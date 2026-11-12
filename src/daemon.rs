//! Vigilancia automatica de simuladores mientras ETOL esta abierto.
//!
//! La logica es pura (sin Tauri): cada `tick` mira que juegos estan corriendo
//! de verdad y devuelve las acciones que la interfaz debe ejecutar. Asi el
//! estado (juego actual, cierre pendiente, parada manual) se puede probar y
//! razonar sin depender de la ventana.

use crate::config::Config;
use crate::detect::{self, GameDef};
use crate::launch::AppKind;
use crate::state;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Margen antes de dar un juego por cerrado (reinicios de sesion).
pub const CLOSE_GRACE: Duration = Duration::from_secs(8);

/// Ventana en la que un arranque manual manda y el vigilante no duplica.
pub const MANUAL_GRACE: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Para el puente del juego indicado.
    StopBridge(String),
    /// Cierra las apps que usa ese juego (no todas: ETS2 solo usa SimHub).
    StopApps(String),
    /// Levanta el puente del juego (si no estaba ya).
    StartBridge(String),
    /// Abre una app; el ejecutor deja pasar unos segundos entre apps.
    StartApp(String, AppKind),
}

#[derive(Debug, Default)]
pub struct Watcher {
    /// El vigilante estaba activo en el tick anterior.
    armed: bool,
    /// Juego cuyo puente estamos siguiendo.
    current: Option<String>,
    /// Momento en el que se dara por cerrado `current`.
    pending_close: Option<Instant>,
    /// Juego parado a mano: no se rearma hasta que se cierre.
    suppressed: Option<String>,
    /// Arranque manual reciente: el vigilante no debe levantar lo mismo.
    manual: Option<(String, Instant)>,
}

impl Watcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// El usuario ha pulsado «Parar» (ficha, bandeja o CLI).
    ///
    /// El juego actual queda ignorado hasta que su proceso desaparezca; si
    /// vuelve a abrirse, el vigilante actua de nuevo.
    pub fn note_manual_stop(&mut self, game: &str) {
        self.suppressed = Some(game.to_string());
        self.current = None;
        self.pending_close = None;
        self.manual = None;
    }

    /// El usuario ha lanzado el juego o el puente a mano: se rearma y, durante
    /// unos segundos, el vigilante deja de arrancar el puente y las apps para
    /// no duplicar el trabajo del script.
    pub fn note_manual_start(&mut self, game: &str) {
        if self.suppressed.as_deref() == Some(game) {
            self.suppressed = None;
        }
        self.manual = Some((game.to_string(), Instant::now() + MANUAL_GRACE));
    }

    /// El arranque manual fallo: se retira la ventana de gracia.
    pub fn cancel_manual_start(&mut self) {
        self.manual = None;
    }

    /// ¿Hay un arranque manual reciente de este juego?
    pub fn manual_active(&self, game: &str) -> bool {
        matches!(&self.manual, Some((id, until)) if id == game && Instant::now() < *until)
    }

    /// Juego que el vigilante esta siguiendo (para diagnostico/tests).
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Juego ignorado por una parada manual (para diagnostico/tests).
    pub fn suppressed(&self) -> Option<&str> {
        self.suppressed.as_deref()
    }

    fn reset(&mut self) {
        self.armed = false;
        self.current = None;
        self.pending_close = None;
        self.suppressed = None;
        self.manual = None;
    }

    /// Calcula las acciones para este instante.
    pub fn tick(&mut self, cfg: &Config, now: Instant) -> Vec<Action> {
        let running = running_games(cfg);
        self.tick_with(cfg, now, &running)
    }

    /// Igual que `tick`, pero reutiliza la lista de juegos en marcha que ya ha
    /// calculado el llamante. Evita recorrer /proc una vez por juego.
    pub fn tick_with(
        &mut self,
        cfg: &Config,
        now: Instant,
        running: &[&'static GameDef],
    ) -> Vec<Action> {
        if !cfg.daemon.enabled {
            self.reset();
            return Vec::new();
        }

        if !self.armed {
            // Al activar el vigilante (o al abrir ETOL con el juego en marcha)
            // se pone al dia como si el juego acabara de abrirse.
            self.armed = true;
            self.current = None;
            self.pending_close = None;
            self.suppressed = None;
            self.manual = None;
        }

        let primary = running.first().map(|def| def.id.to_string());
        let mut actions = Vec::new();

        // Arranque manual reciente: el script ya esta levantando el puente y
        // las apps. Se adopta el juego como actual (para seguir su cierre)
        // pero no se emite ninguna accion de arranque.
        if let Some((game, until)) = self.manual.clone() {
            if now >= until {
                self.manual = None;
            } else if primary.as_deref() == Some(game.as_str()) {
                if self.current.is_none() {
                    self.current = Some(game);
                    self.pending_close = None;
                }
                return Vec::new();
            } else if primary.is_none() && self.current.is_none() {
                return Vec::new();
            }
        }

        // Parada manual: se respeta mientras ese juego siga abierto.
        if let Some(game) = self.suppressed.clone() {
            if primary.as_deref() == Some(game.as_str()) {
                self.current = None;
                self.pending_close = None;
                return Vec::new();
            }
            self.suppressed = None;
        }

        if let Some(current) = self.current.clone() {
            match primary.as_deref() {
                Some(game) if game == current => {
                    // Sigue abierto (o ha vuelto tras un reinicio de sesion).
                    self.pending_close = None;
                }
                Some(_) => {
                    // Cambio directo de simulador: se para el anterior ya.
                    self.current = None;
                    self.pending_close = None;
                    actions.push(Action::StopBridge(current.clone()));
                    if cfg.daemon.close_apps {
                        actions.push(Action::StopApps(current));
                    }
                }
                None => {
                    // Se ha cerrado: esperar el margen por si vuelve.
                    match self.pending_close {
                        Some(deadline) if now >= deadline => {
                            self.current = None;
                            self.pending_close = None;
                            actions.push(Action::StopBridge(current.clone()));
                            if cfg.daemon.close_apps {
                                actions.push(Action::StopApps(current));
                            }
                        }
                        Some(_) => {}
                        None => self.pending_close = Some(now + CLOSE_GRACE),
                    }
                }
            }
        }

        // Juego nuevo (y sin cierre pendiente): levantar su puente y sus apps.
        if self.current.is_none() && self.pending_close.is_none() {
            if let Some(id) = primary {
                self.current = Some(id.clone());
                for def in detect::GAMES {
                    if def.id != id
                        && state::any_matching(&def.bridge_needle())
                    {
                        actions.push(Action::StopBridge(def.id.to_string()));
                        if cfg.daemon.close_apps {
                            actions.push(Action::StopApps(def.id.to_string()));
                        }
                    }
                }
                actions.push(Action::StartBridge(id.clone()));
                let choice = cfg.daemon.game(&id);
                if choice.crewchief {
                    actions.push(Action::StartApp(id.clone(), AppKind::CrewChief));
                }
                if choice.simhub {
                    actions.push(Action::StartApp(id.clone(), AppKind::SimHub));
                }
            }
        }

        actions
    }
}

/// Juegos instalados que tienen su proceso real en marcha.
pub fn running_games(cfg: &Config) -> Vec<&'static GameDef> {
    detect::GAMES
        .iter()
        .filter(|def| game_installed(cfg, def) && game_running(cfg, def))
        .collect()
}

pub fn game_installed(cfg: &Config, def: &GameDef) -> bool {
    let Some(game) = cfg.games.get(def.id) else {
        return false;
    };
    game.installed
        && game
            .path
            .as_ref()
            .map(|path| path.is_dir())
            .unwrap_or(false)
        && game
            .exe
            .as_ref()
            .map(|path| path.is_file())
            .unwrap_or(false)
}

/// Procesos que llevan el nombre del ejecutable de un juego pero que
/// `game_running` no reconoce. Solo se usa para diagnostico: si un juego no
/// arranca solo, el log dice que proceso se esta viendo.
pub fn undiscovered(cfg: &Config) -> Vec<(String, String)> {
    let stubs = stub_needles(cfg);
    detect::GAMES
        .iter()
        .filter_map(|def| {
            let game = cfg.games.get(def.id)?;
            if !game.installed || game_running(cfg, def) {
                return None;
            }
            let name = game
                .exe
                .as_ref()?
                .file_name()?
                .to_str()?
                .to_ascii_lowercase();
            if !name.ends_with(".exe") {
                return None;
            }
            let proc = state::matching(&name).into_iter().find(|proc| {
                let cmdline = proc.cmdline.to_lowercase();
                !stubs.iter().any(|needle| cmdline.contains(needle))
                    && !cmdline.contains("start.exe")
                    && !is_shell_or_wrapper(&cmdline)
            })?;
            Some((def.id.to_string(), proc.cmdline))
        })
        .collect()
}

/// Envoltorios que solo mencionan el ejecutable en sus argumentos (una
/// shell, un `pgrep`, un `wine`...). No sirven para diagnosticar nada.
fn is_shell_or_wrapper(cmdline: &str) -> bool {
    const WRAPPERS: &[&str] = &[
        "bash", "sh -c", "zsh", "fish", "grep", "pgrep", "ps ", "python",
        "wine", "wineserver", "proton", "steam.exe",
    ];
    WRAPPERS.iter().any(|wrapper| cmdline.contains(wrapper))
}

/// Proceso real del juego, excluyendo los stubs inertes que crea el puente.
///
/// Los simuladores de Windows se detectan por su ruta de unidad Wine. Los
/// juegos nativos de Linux (ETS2) llegan como una ruta Unix normal, sin `.exe`.
pub fn game_running(cfg: &Config, def: &GameDef) -> bool {
    let Some(game) = cfg.games.get(def.id) else {
        return false;
    };
    let Some(exe) = game.exe.as_ref() else {
        return false;
    };
    let Some(name) = exe.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    let stubs = stub_needles(cfg);
    let real = |proc: &state::Proc| {
        let cmdline = proc.cmdline.to_lowercase();
        !stubs.iter().any(|needle| cmdline.contains(needle))
    };

    if name.to_ascii_lowercase().ends_with(".exe") {
        // Algunos juegos abren antes un lanzador con otro nombre (Assetto
        // Corsa: `AssettoCorsa.exe` -> `acs.exe`). Mientras cualquiera de
        // los ejecutables del juego siga vivo, el juego esta en marcha.
        return def.exes.iter().any(|candidate| {
            let Some(name) = Path::new(candidate)
                .file_name()
                .and_then(|name| name.to_str())
            else {
                return false;
            };
            state::running_drive_exe(name).iter().any(&real)
        });
    }

    // Juego nativo: el proceso se llama igual que el binario, sin extension.
    state::matching(name)
        .iter()
        .filter(|proc| real(proc))
        .any(|proc| native_cmd_matches(&proc.cmdline, name))
}

/// ¿Algun argumento de la linea de comandos es ese ejecutable nativo?
///
/// Steam a veces envuelve el juego en otros procesos, asi que no basta con
/// mirar solo el primer argumento.
fn native_cmd_matches(cmdline: &str, name: &str) -> bool {
    cmdline.split_whitespace().any(|token| {
        let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
        base.eq_ignore_ascii_case(name)
    })
}

fn run_dir(cfg: &Config, def: &GameDef) -> PathBuf {
    cfg.games
        .get(def.id)
        .map(|game| game.run_dir.clone())
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| cfg.bridge.run_base.join(def.run_subdir))
}

/// Rutas de todos los stubs, en forma Unix y en forma de unidad Wine.
fn stub_needles(cfg: &Config) -> Vec<String> {
    detect::GAMES
        .iter()
        .flat_map(|def| {
            let dir = run_dir(cfg, def);
            let unix = dir.to_string_lossy().to_lowercase();
            let windows = windows_form(&dir);
            [unix, windows]
        })
        .filter(|needle| !needle.is_empty())
        .collect()
}

/// `/tmp/etol/rf2_mirror` -> `z:\tmp\etol\rf2_mirror`.
fn windows_form(path: &Path) -> String {
    format!(
        "z:{}",
        path.to_string_lossy().replace('/', "\\").to_lowercase()
    )
}

#[cfg(test)]
#[allow(clippy::zombie_processes)] // los tests matan sus procesos a mano
mod tests {
    use super::*;
    use crate::config::{Daemon, DaemonGame, Game};
    use std::collections::BTreeMap;

    fn cfg_with(enabled: bool) -> Config {
        let mut cfg = Config {
            daemon: Daemon {
                enabled,
                close_apps: true,
                games: BTreeMap::new(),
            },
            ..Config::default()
        };
        for def in detect::GAMES {
            cfg.games.insert(
                def.id.to_string(),
                Game {
                    id: def.id.to_string(),
                    name: def.name.to_string(),
                    installed: true,
                    path: Some(PathBuf::from("/juego")),
                    exe: Some(PathBuf::from("/juego/juego.exe")),
                    run_dir: PathBuf::from(format!("/tmp/etol/{}_mirror", def.id)),
                    ..Game::default()
                },
            );
        }
        cfg
    }

    #[test]
    fn desactivado_no_hace_nada() {
        let cfg = cfg_with(false);
        let mut watcher = Watcher::new();
        assert!(watcher.tick(&cfg, Instant::now()).is_empty());
        assert!(watcher.current().is_none());
    }

    #[test]
    fn eleccion_por_defecto_es_ambas() {
        let cfg = Config::default();
        let choice = cfg.daemon.game("rf2");
        assert!(choice.crewchief && choice.simhub);
    }

    #[test]
    fn eleccion_configurada() {
        let mut cfg = Config::default();
        cfg.daemon
            .games
            .insert("rf2".into(), DaemonGame { crewchief: true, simhub: false });
        let choice = cfg.daemon.game("rf2");
        assert!(choice.crewchief && !choice.simhub);
    }

    #[test]
    fn parada_manual_se_respeta() {
        let mut watcher = Watcher::new();
        watcher.note_manual_stop("rf2");
        assert_eq!(watcher.suppressed(), Some("rf2"));
        watcher.note_manual_start("rf2");
        assert_eq!(watcher.suppressed(), None);
    }

    #[test]
    fn arranque_manual_abre_ventana_de_gracia() {
        let mut watcher = Watcher::new();
        watcher.note_manual_start("rf2");
        assert!(watcher.manual_active("rf2"));
        assert!(!watcher.manual_active("lmu"));
        // Parar a mano cancela la ventana de arranque.
        watcher.note_manual_stop("rf2");
        assert!(!watcher.manual_active("rf2"));
        // Y un arranque fallido tambien.
        watcher.note_manual_start("rf2");
        watcher.cancel_manual_start();
        assert!(!watcher.manual_active("rf2"));
    }

    #[test]
    fn beamng_usa_el_stub_de_deteccion() {
        // El puente de BeamNG es el proceso inerte que SimHub detecta; el
        // juego real es nativo y no lleva `.exe`.
        assert_eq!(
            detect::game_def("beamng").unwrap().bridge_needle(),
            "BeamNG.drive.x64.exe"
        );
    }

    #[test]
    fn stub_no_cuenta_como_juego() {
        let cfg = cfg_with(true);
        let def = detect::game_def("rf2").unwrap();
        assert!(stub_needles(&cfg).iter().any(|needle| {
            needle.contains("rf2_mirror")
        }));
        // Sin procesos reales no hay juego detectado.
        let _ = def;
    }

    #[test]
    fn ets2_solo_usa_simhub() {
        let mut cfg = Config::default();
        let choice = cfg.daemon.game("ets2");
        assert!(!choice.crewchief, "CrewChief no tiene lector de camiones");
        assert!(choice.simhub);

        // Aunque la configuracion diga lo contrario, se recorta al juego.
        cfg.daemon
            .games
            .insert("ets2".into(), DaemonGame { crewchief: true, simhub: true });
        let choice = cfg.daemon.game("ets2");
        assert!(!choice.crewchief);
        assert!(choice.simhub);
    }

    #[cfg(unix)]
    #[test]
    fn detecta_el_juego_nativo_de_linux() {
        let mut cfg = cfg_with(true);
        let def = detect::game_def("ets2").unwrap();
        let game = cfg.games.get_mut("ets2").unwrap();
        game.exe = Some(PathBuf::from("/juego/bin/linux_x64/eurotrucks2"));
        game.run_dir = PathBuf::from("/tmp/etol/ets2_bridge");

        let real = spawn_con_arg0(
            "/mnt/GAMES/SteamLibrary/steamapps/common/Euro Truck Simulator 2/bin/linux_x64/eurotrucks2",
        );
        std::thread::sleep(Duration::from_millis(150));
        let detected = game_running(&cfg, def);
        let _ = std::process::Command::new("kill")
            .arg(real.id().to_string())
            .status();

        assert!(detected, "el proceso nativo deberia contar como juego");
    }

    #[cfg(unix)]
    fn spawn_con_arg0(arg0: &str) -> std::process::Child {
        use std::os::unix::process::CommandExt;
        std::process::Command::new("sleep")
            .arg("30")
            .arg0(arg0)
            .spawn()
            .expect("no se pudo lanzar el proceso de prueba")
    }

    #[cfg(unix)]
    fn spawn_con_arg0_y_arg(arg0: &str, arg: &str) -> std::process::Child {
        use std::os::unix::process::CommandExt;
        // `sh` conserva su cmdline mientras espera a `sleep`; asi el proceso
        // de prueba tiene el nombre de Steam y la ruta del juego como args.
        std::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .arg0(arg0)
            .arg(arg)
            .spawn()
            .expect("no se pudo lanzar el proceso de prueba")
    }

    #[test]
    fn cierra_al_desaparecer_el_juego() {
        let cfg = cfg_with(true);
        let def = detect::game_def("rf2").unwrap();
        let mut watcher = Watcher::new();
        let t0 = Instant::now();

        let actions = watcher.tick_with(&cfg, t0, &[def]);
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::StartBridge(id) if id == "rf2")),
            "el primer tick debe arrancar el puente"
        );

        // El juego desaparece: se espera el margen antes de parar nada.
        let actions = watcher.tick_with(&cfg, t0 + Duration::from_secs(1), &[]);
        assert!(actions.is_empty(), "todavia no debe parar");

        // Pasado el margen, se para el puente y las apps.
        let actions = watcher.tick_with(
            &cfg,
            t0 + CLOSE_GRACE + Duration::from_secs(2),
            &[],
        );
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::StopBridge(id) if id == "rf2")),
            "debe parar el puente"
        );
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::StopApps(id) if id == "rf2")),
            "debe cerrar las apps"
        );
    }

    #[cfg(unix)]
    #[test]
    fn el_envoltorio_steam_no_cuenta_como_juego() {
        let mut cfg = cfg_with(true);
        let def = detect::game_def("acevo").unwrap();
        let game = cfg.games.get_mut("acevo").unwrap();
        game.exe = Some(PathBuf::from("/juego/AssettoCorsaEVO.exe"));
        game.run_dir = PathBuf::from("/tmp/etol/acevo_mirror");

        // `steam.exe` lleva la ruta del juego como argumento y empieza por
        // `c:\`, pero no es el juego.
        let fake = spawn_con_arg0_y_arg(
            "c:\\windows\\system32\\steam.exe",
            "S:\\steamapps\\common\\Assetto Corsa EVO\\AssettoCorsaEVO.exe",
        );
        std::thread::sleep(Duration::from_millis(150));
        let detected = game_running(&cfg, def);
        let _ = std::process::Command::new("kill")
            .arg(fake.id().to_string())
            .status();

        assert!(!detected, "el steam.exe del prefijo no debe contar como juego");
    }

    #[cfg(unix)]
    #[test]
    fn detecta_el_juego_real_y_ignora_su_stub() {
        let mut cfg = cfg_with(true);
        let def = detect::game_def("rf2").unwrap();
        let game = cfg.games.get_mut("rf2").unwrap();
        game.exe = Some(PathBuf::from("/juego/Bin64/rFactor2.exe"));
        game.run_dir = PathBuf::from("/tmp/etol/rf2_mirror");

        let real = spawn_con_arg0("S:\\steamapps\\common\\rFactor 2\\Bin64\\rFactor2.exe");
        std::thread::sleep(Duration::from_millis(150));
        let detected = game_running(&cfg, def);
        let _ = std::process::Command::new("kill")
            .arg(real.id().to_string())
            .status();

        assert!(detected, "el proceso real deberia contar como juego");

        let stub = spawn_con_arg0("Z:\\tmp\\etol\\rf2_mirror\\Bin64\\rFactor2.exe");
        std::thread::sleep(Duration::from_millis(150));
        let detected = game_running(&cfg, def);
        let _ = std::process::Command::new("kill")
            .arg(stub.id().to_string())
            .status();

        assert!(!detected, "el stub del puente no debe contar como juego");
    }
}

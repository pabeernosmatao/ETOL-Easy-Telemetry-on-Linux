//! Instantanea serializable del estado en vivo. La usan tanto la CLI como la
//! interfaz grafica, para que ambas muestren exactamente lo mismo.

use crate::config::Config;
use crate::detect;
use crate::state;
use crate::util;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub generated: String,
    pub apps: AppsStatus,
    pub hf8: Hf8Status,
    /// Acceso del usuario a los puertos serie USB (HF8, LEDs, Arduino...).
    pub usb: crate::usbaccess::UsbAccess,
    pub setup: SetupStatus,
    /// Vigilancia automatica activa (cambia el banner de MESSAGES).
    pub daemon_enabled: bool,
    pub games: Vec<GameStatus>,
}

/// Estado de la instalacion que alimenta los avisos de la interfaz.
#[derive(Debug, Clone, Serialize)]
pub struct SetupStatus {
    pub flatpak: detect::bottles::FlatpakState,
    /// La botella/prefijo configurados existen y tienen `drive_c`.
    pub bottle_ready: bool,
    pub bottle_path: Option<String>,
    /// Hay que reconfigurar un dispositivo (p. ej. HF8 sin COM33).
    pub device_needs_fix: bool,
    pub device_hint: Option<String>,
    /// Carpeta donde viven los scripts de sistema.
    pub scripts_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppsStatus {
    pub bottle: Option<String>,
    pub bottle_path: Option<String>,
    pub bottle_origin: Option<String>,
    pub wine: Option<String>,
    pub wine_source: Option<String>,
    pub simhub: ProcessStatus,
    pub crewchief: ProcessStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessStatus {
    pub running: bool,
    pub pids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hf8Status {
    pub enabled: bool,
    pub usb_scan: bool,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub com_port: String,
    pub com_target: Option<String>,
    pub com_ok: bool,
    pub holders: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub proton: Option<String>,
    pub prefix: Option<String>,
    pub exe: Option<String>,
    pub bridge_running: bool,
    pub bridge_pids: Vec<u32>,
    pub watchdog_running: bool,
    pub port: u16,
    pub port_in_use: bool,
    pub lock: Option<String>,
    pub lock_held: bool,
    /// Apps que usa el juego (`simhub`, `crewchief`).
    pub apps: Vec<String>,
    /// Juego nativo de Linux (ETS2): sin prefijo Wine propio.
    pub native: bool,
    /// Plugin del puente instalado en el juego, si el juego usa uno.
    pub plugin_ok: Option<bool>,
    /// `ok` (configurado), `pending` (nunca configurado) o `bad` (mal).
    pub config_state: String,
    /// Motivo del problema de configuracion, si lo hay.
    pub config_detail: Option<String>,
}

/// Carpeta donde ETOL deja la marca de "juego ya configurado".
pub fn configured_dir() -> std::path::PathBuf {
    crate::config::root().join("configured")
}

pub fn configured_marker(game: &str) -> bool {
    configured_dir().join(game).is_file()
}

/// Huella del runner que se uso al configurar: Proton/wine o "native".
fn game_fingerprint(game: &crate::config::Game) -> String {
    if let Some(path) = &game.proton {
        return path.display().to_string();
    }
    if let Some(path) = &game.wine {
        return path.display().to_string();
    }
    "native".to_string()
}

/// Segunda linea de la marca (la huella). Las marcas antiguas solo tienen la
/// fecha, asi que no se comparan.
fn configured_fingerprint(game: &str) -> Option<String> {
    let text = std::fs::read_to_string(configured_dir().join(game)).ok()?;
    let fingerprint = text.lines().nth(1)?.trim().to_string();
    (!fingerprint.is_empty()).then_some(fingerprint)
}

/// Deja constancia de que el juego se ha configurado sin errores.
pub fn mark_configured(cfg: &Config, game: &str) {
    let dir = configured_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        let fingerprint = cfg
            .games
            .get(game)
            .map(game_fingerprint)
            .unwrap_or_default();
        let text = format!("{}\n{fingerprint}\n", util::now_utc());
        let _ = std::fs::write(dir.join(game), text);
    }
}

/// Problemas de configuracion que ETOL sabe comprobar.
fn config_problem(
    cfg: &Config,
    def: &detect::GameDef,
    game: &crate::config::Game,
) -> Option<String> {
    if let Some(stored) = configured_fingerprint(def.id) {
        if stored != game_fingerprint(game) {
            return Some(
                "el Proton o wine del juego ha cambiado; vuelve a configurarlo".to_string(),
            );
        }
    }
    if def.id == "ets2" {
        if let Some(dir) = game.path.as_ref() {
            if let Some(path) = def.plugin_path(dir) {
                if !path.is_file() {
                    return Some("falta el plugin SCS del juego".to_string());
                }
            }
        }
    }
    if def.id == "beamng" {
        if let Some(bottle) = cfg.apps.bottle.as_ref() {
            let path = bottle.join(
                "drive_c/Program Files (x86)/SimHub/PluginsData/GameSettings.json",
            );
            if let Ok(text) = std::fs::read_to_string(&path) {
                let json: serde_json::Value =
                    serde_json::from_str(&text).unwrap_or_default();
                let reader = json
                    .get("BeamNgDrive")
                    .and_then(|entry| entry.get("CurrentReader"))
                    .and_then(|value| value.as_str());
                if reader != Some("BeamNgManager") {
                    return Some(
                        "SimHub no esta en modo Outgauge + motionsim".to_string(),
                    );
                }
            }
        }
    }
    None
}

pub fn snapshot(cfg: &Config) -> Snapshot {
    let device = if cfg.devices.scan_usb {
        state::hf8_devices().into_iter().find(|device| {
            device.vendor.eq_ignore_ascii_case(&cfg.hf8.vendor_id)
                && device.product.eq_ignore_ascii_case(&cfg.hf8.product_id)
        })
    } else {
        None
    };

    let (com_target, com_ok) = match (&cfg.apps.bottle, &device) {
        (Some(bottle), Some(device)) => {
            let wanted = cfg.hf8.com_port.to_lowercase();
            let link = state::com_links(bottle)
                .into_iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&wanted));
            match link {
                Some((_, target)) => {
                    let ok = target == device.dev;
                    (Some(target.display().to_string()), ok)
                }
                None => (None, false),
            }
        }
        _ => (None, false),
    };

    let holders = device
        .as_ref()
        .map(|device| {
            state::holders(&device.dev)
                .into_iter()
                .map(|(pid, comm)| format!("{comm} ({pid})"))
                .collect()
        })
        .unwrap_or_default();

    let games = detect::GAMES
        .iter()
        .filter_map(|def| {
            let game = cfg.games.get(def.id)?;
            let bridge_pids: Vec<u32> = state::matching(&def.bridge_needle())
                .iter()
                .map(|proc| proc.pid)
                .collect();
            let lock = state::arranque_lock(&game.run_dir);
            let lock_held = lock
                .as_ref()
                .map(|path| !state::fd_holders(path).is_empty())
                .unwrap_or(false);

            let plugin_ok = def.plugin.and_then(|_| {
                let dir = game.path.as_ref()?;
                Some(def.plugin_path(dir).map(|path| path.is_file()).unwrap_or(false))
            });

            // Estado de configuracion: rojo si algo esta mal o si el juego
            // acaba de detectarse y nunca se ha configurado.
            let problem = config_problem(cfg, def, game);
            let configured = configured_marker(def.id);
            let config_state = if problem.is_some() {
                "bad"
            } else if configured {
                "ok"
            } else {
                "pending"
            };

            Some(GameStatus {
                config_state: config_state.to_string(),
                config_detail: problem,
                id: def.id.to_string(),
                name: game.name.clone(),
                installed: game.installed
                    && game
                        .path
                        .as_ref()
                        .map(|path| path.is_dir())
                        .unwrap_or(false)
                    && game
                        .exe
                        .as_ref()
                        .map(|path| path.is_file())
                        .unwrap_or(false),
                proton: game.proton.as_ref().map(|path| path.display().to_string()),
                prefix: game.prefix.as_ref().map(|path| path.display().to_string()),
                exe: game.exe.as_ref().map(|path| path.display().to_string()),
                bridge_running: !bridge_pids.is_empty(),
                bridge_pids,
                watchdog_running: state::any_matching(&format!("{}_bridge_watchdog", def.id)),
                port: def.port,
                // Sin puerto UDP (ETS2) no hay nada que comprobar.
                port_in_use: def.port != 0 && state::udp_in_use(def.port),
                // El fichero de lock queda en disco tras un arranque normal
                // (flock no lo borra). Solo interesa mientras alguien lo tiene
                // abierto: un lock sin duenos es el estado de reposo.
                lock: lock
                    .filter(|_| lock_held)
                    .map(|path| path.display().to_string()),
                lock_held,
                apps: def.apps.iter().map(|app| app.to_string()).collect(),
                native: def.native,
                plugin_ok,
            })
        })
        .collect();

    let mut extra_paths: Vec<std::path::PathBuf> = detect::steam::libraries();
    for def in detect::GAMES {
        if let Some(game) = cfg.games.get(def.id) {
            if let Some(path) = &game.path {
                extra_paths.push(path.clone());
            }
        }
    }
    let flatpak = detect::bottles::flatpak_state(&extra_paths);
    let bottle_ready = cfg
        .apps
        .bottle
        .as_deref()
        .map(detect::bottles::bottle_ready)
        .unwrap_or(false);
    let any_bridge = detect::GAMES
        .iter()
        .any(|def| state::any_matching(&def.bridge_needle()));
    let device_needs_fix = cfg.devices.scan_usb
        && cfg.hf8.enabled
        && device.is_some()
        && !com_ok
        && any_bridge;
    let device_hint = if device_needs_fix {
        Some(format!(
            "El pad está conectado, pero {} no apunta a {}.",
            cfg.hf8.com_port,
            device
                .as_ref()
                .map(|device| device.dev.display().to_string())
                .unwrap_or_default()
        ))
    } else {
        None
    };
    let scripts_dir = cfg
        .app_dir()
        .map(|dir| dir.join("sistema").display().to_string());
    let usb = crate::usbaccess::inspect();

    Snapshot {
        generated: util::now_utc(),
        apps: AppsStatus {
            bottle: cfg.apps.bottle_name.clone(),
            bottle_path: cfg.apps.bottle.as_ref().map(|path| path.display().to_string()),
            bottle_origin: cfg
                .apps
                .bottle
                .as_deref()
                .and_then(crate::detect::bottles::origin_of)
                .map(|origin| origin.label().to_string()),
            wine: cfg.apps.wine.as_ref().map(|path| path.display().to_string()),
            wine_source: cfg.apps.wine_source.clone(),
            simhub: process_status("SimHubWPF.exe"),
            crewchief: process_status("CrewChiefV4.exe"),
        },
        hf8: Hf8Status {
            enabled: cfg.hf8.enabled,
            usb_scan: cfg.devices.scan_usb,
            device: device.as_ref().map(|device| device.dev.display().to_string()),
            device_name: device.as_ref().map(|device| device.name.clone()),
            com_port: cfg.hf8.com_port.clone(),
            com_target,
            com_ok,
            holders,
        },
        usb,
        setup: SetupStatus {
            flatpak,
            bottle_ready,
            bottle_path: cfg
                .apps
                .bottle
                .as_ref()
                .map(|path| path.display().to_string()),
            device_needs_fix,
            device_hint,
            scripts_dir,
        },
        daemon_enabled: cfg.daemon.enabled,
        games,
    }
}

fn process_status(needle: &str) -> ProcessStatus {
    let pids: Vec<u32> = state::running_windows_exe(needle)
        .iter()
        .map(|proc| proc.pid)
        .collect();
    ProcessStatus {
        running: !pids.is_empty(),
        pids,
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Interfaz grafica de etol.
//!
//! Toda la logica vive en la libreria `etol`; aqui solo se exponen los
//! comandos que usa la pagina de `ui/`.

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use etol::config::{self, Config};
use etol::daemon::{self, Action};
use etol::detect;
use etol::doctor::{self, Check};
use etol::envgen;
use etol::i18n;
use etol::launch::{self, AppKind, Launched, Mode, StopReport};
use etol::logs;
use etol::kwin;
use etol::state;
use etol::status::{self, Snapshot};

mod payload;
mod portcheck;

/// Intervalo entre comprobaciones del vigilante.
const DAEMON_TICK: Duration = Duration::from_secs(1);
/// Separacion entre CrewChief y SimHub, como en los scripts de arranque.
const DAEMON_APP_GAP: Duration = Duration::from_secs(8);
/// Espera maxima a que el puente publique su puerto.
const DAEMON_BRIDGE_TIMEOUT: Duration = Duration::from_secs(25);

/// Se activa al salir para que el vigilante no reabra nada mientras se limpia.
static QUITTING: AtomicBool = AtomicBool::new(false);

/// Estado compartido del vigilante (hilo de fondo, comandos y bandeja).
fn watcher() -> &'static Arc<Mutex<daemon::Watcher>> {
    static WATCHER: OnceLock<Arc<Mutex<daemon::Watcher>>> = OnceLock::new();
    WATCHER.get_or_init(|| Arc::new(Mutex::new(daemon::Watcher::new())))
}

fn with_watcher<T>(f: impl FnOnce(&mut daemon::Watcher) -> T) -> T {
    let mut guard = watcher().lock().unwrap_or_else(|error| error.into_inner());
    f(&mut guard)
}

fn bridge_running(game: &str) -> bool {
    let needle = detect::game_def(game)
        .map(|def| def.bridge_needle())
        .unwrap_or_else(|| format!("{game}_shm_mirror"));
    if game.eq_ignore_ascii_case("dirt2") {
        // El relay UDP es un proceso Linux, no un .exe de Wine.
        state::any_matching("dirt2_udp_relay.py")
    } else {
        // Solo procesos Wine reales del espejo: evita que una shell, un editor
        // o un log con el nombre del binario se confundan con el puente.
        !state::running_drive_exe(&needle).is_empty()
    }
}

fn wait_until(timeout: Duration, step: Duration, mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if check() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(step);
    }
}

#[derive(Serialize)]
struct Bootstrap {
    config: Config,
    config_path: String,
    env_path: String,
    logs_path: String,
    first_run: bool,
    payload: payload::PayloadInfo,
    language: String,
}

#[derive(Serialize)]
struct DoctorReport {
    checks: Vec<Check>,
    failures: usize,
    warnings: usize,
}

/// Ejecuta trabajo bloqueante fuera del hilo de la interfaz.
async fn blocking<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|error| format!("tarea interrumpida: {error}"))?
        .map_err(|error| format!("{error:#}"))
}

fn bootstrap_inner() -> Result<Bootstrap> {
    let first_run = !config::config_path().exists();
    let mut cfg = if first_run {
        detect::detect()
    } else {
        Config::load()?
    };

    // Simuladores nuevos o que acaban de instalarse: si falta alguno o la
    // configuracion guardada lo tiene como no instalado, se vuelve a detectar
    // para actualizar rutas/prefijos sin pedir "Redetectar".
    let missing_game = !first_run
        && detect::GAMES.iter().any(|def| {
            match cfg.games.get(def.id) {
                None => true,
                Some(game) => {
                    !game.installed
                        || !game
                            .path
                            .as_ref()
                            .map(|path| path.is_dir())
                            .unwrap_or(false)
                        || !game
                            .exe
                            .as_ref()
                            .map(|path| path.is_file())
                            .unwrap_or(false)
                }
            }
        });
    if missing_game {
        let mut fresh = detect::detect();
        detect::preserve_manual_wine(&mut fresh);
        cfg = fresh;
    }

    // Si la carpeta de scripts desaparecio (por ejemplo tras reinstalar el
    // sistema), usa la copia incluida en el AppImage o cualquier ruta valida.
    let mut changed = first_run || missing_game;
    let current_valid = cfg
        .app_dir()
        .map(detect::is_app_dir)
        .unwrap_or(false);

    if let Some(dir) = detect::app_dir_override() {
        // El AppImage instalo su payload: es la copia de trabajo.
        if cfg.bridge.app_dir.as_deref() != Some(dir.as_path()) {
            cfg.bridge.app_dir = Some(dir.clone());
            changed = true;
        }
    } else if !current_valid {
        if let Some(dir) = detect::find_app_dir() {
            cfg.bridge.app_dir = Some(dir);
            changed = true;
        }
    }

    // Enlaces de Steam dentro de la botella (tambien al arrancar, no solo al
    // pulsar Redetectar).
    let links = detect::links::ensure(&cfg);
    detect::links::log_report(&links);

    if changed {
        cfg.save()?;
    }

    let env_path = envgen::default_path();
    if changed || !env_path.exists() {
        envgen::write(&cfg)?;
    }

    Ok(Bootstrap {
        config: cfg,
        config_path: config::config_path().display().to_string(),
        env_path: env_path.display().to_string(),
        logs_path: logs::dir().display().to_string(),
        first_run,
        payload: payload::info(),
        language: i18n::code(i18n::current()).to_string(),
    })
}

#[tauri::command]
async fn bootstrap() -> Result<Bootstrap, String> {
    // Marca de que la pagina ha cargado y el IPC funciona.
    logs::info("gui", "interfaz conectada");
    blocking(bootstrap_inner).await
}

#[tauri::command]
async fn detect(app: tauri::AppHandle) -> Result<Bootstrap, String> {
    blocking(move || {
        let mut cfg = detect::detect();
        detect::preserve_manual_wine(&mut cfg);
        cfg.save()?;
        i18n::set_from_setting(&cfg.ui.language);
        envgen::write(&cfg)?;
        refresh_tray(&app);
        Ok(Bootstrap {
            config: cfg,
            config_path: config::config_path().display().to_string(),
            env_path: envgen::default_path().display().to_string(),
            logs_path: logs::dir().display().to_string(),
            first_run: false,
            payload: payload::info(),
            language: i18n::code(i18n::current()).to_string(),
        })
    })
    .await
}

#[tauri::command]
async fn get_config() -> Result<Config, String> {
    blocking(Config::load).await
}

#[tauri::command]
async fn set_config(app: tauri::AppHandle, key: String, value: String) -> Result<Bootstrap, String> {
    blocking(move || {
        let mut cfg = Config::load()?;
        cfg.apply_override(&key, &value)?;
        cfg.save()?;
        i18n::set_from_setting(&cfg.ui.language);
        if key == "logs.enabled" {
            logs::set_enabled(cfg.logs.enabled);
        } else if key == "logs.retention_days" {
            logs::set_retention(cfg.logs.retention_days);
        }
        logs::info("config", format!("{key}={value}"));
        envgen::write(&cfg)?;
        refresh_tray(&app);
        Ok(Bootstrap {
            config: cfg,
            config_path: config::config_path().display().to_string(),
            env_path: envgen::default_path().display().to_string(),
            logs_path: logs::dir().display().to_string(),
            first_run: false,
            payload: payload::info(),
            language: i18n::code(i18n::current()).to_string(),
        })
    })
    .await
}

#[tauri::command]
async fn status(app: tauri::AppHandle) -> Result<Snapshot, String> {
    blocking(move || {
        let cfg = Config::load()?;
        refresh_tray(&app);
        Ok(status::snapshot(&cfg))
    })
    .await
}

#[tauri::command]
async fn doctor_report() -> Result<DoctorReport, String> {
    blocking(|| {
        let cfg = Config::load()?;
        let checks = doctor::run(&cfg);
        let failures = checks
            .iter()
            .filter(|check| check.level == doctor::Level::Fail)
            .count();
        let warnings = checks
            .iter()
            .filter(|check| check.level == doctor::Level::Warn)
            .count();
        Ok(DoctorReport {
            checks,
            failures,
            warnings,
        })
    })
    .await
}

#[tauri::command]
async fn fix_locks() -> Result<Vec<String>, String> {
    blocking(|| {
        let cfg = Config::load()?;
        Ok(doctor::fix_stale_locks(&cfg)
            .into_iter()
            .map(|path| path.display().to_string())
            .collect())
    })
    .await
}

#[tauri::command]
async fn launch(
    game: String,
    mode: String,
    no_hf8: Option<bool>,
) -> Result<Launched, String> {
    blocking(move || {
        let cfg = Config::load()?;
        let mode = match mode.as_str() {
            "todo" => Mode::Todo,
            "puente" => Mode::Puente,
            "stop" | "parar" => Mode::Stop,
            other => return Err(anyhow!("modo desconocido: {other}")),
        };
        if detect::game_def(&game).is_none() {
            return Err(anyhow!("juego desconocido: {game}"));
        }
        let no_hf8 = no_hf8.unwrap_or(false) || cfg.hf8_disabled();
        let stopped = matches!(mode, Mode::Stop);
        if stopped {
            with_watcher(|watcher| watcher.note_manual_stop(&game));
        } else {
            // Se marca ANTES de lanzar: el vigilante no debe arrancar el
            // puente ni las apps mientras el script todavia lo esta haciendo.
            with_watcher(|watcher| watcher.note_manual_start(&game));
        }
        let result = launch::spawn(&cfg, &game, mode, no_hf8, &[]);
        if !stopped && result.is_err() {
            with_watcher(|watcher| watcher.cancel_manual_start());
        }
        result
    })
    .await
}

#[tauri::command]
async fn stop(game: String) -> Result<StopReport, String> {
    blocking(move || {
        let cfg = Config::load()?;
        if detect::game_def(&game).is_none() {
            return Err(anyhow!("juego desconocido: {game}"));
        }
        if !cfg.games.contains_key(&game) {
            return Err(anyhow!(
                "el juego {game} no esta en la configuracion; pulsa Detectar"
            ));
        }
        let report = launch::stop_all(&cfg, &game);
        with_watcher(|watcher| watcher.note_manual_stop(&game));
        Ok(report)
    })
    .await
}

#[tauri::command]
async fn start_app(app: String) -> Result<Launched, String> {
    blocking(move || {
        let kind = AppKind::parse(&app).ok_or_else(|| anyhow!("aplicacion desconocida: {app}"))?;
        let cfg = Config::load()?;
        match launch::start_app(&cfg, kind) {
            Ok(value) => Ok(value),
            Err(error) => {
                logs::error("apps", format!("{}: {error:#}", kind.tag()));
                Err(error)
            }
        }
    })
    .await
}

#[tauri::command]
async fn stop_app(app: String) -> Result<launch::AppReport, String> {
    blocking(move || {
        let kind = AppKind::parse(&app).ok_or_else(|| anyhow!("aplicacion desconocida: {app}"))?;
        let cfg = Config::load()?;
        let report = launch::stop_app(&cfg, kind);
        logs::info(
            "apps",
            format!(
                "{}: {} ({})",
                kind.tag(),
                if report.closed { "cerrado" } else { "sigue abierto" },
                report.detail
            ),
        );
        Ok(report)
    })
    .await
}

#[derive(Serialize)]
struct GameAppStart {
    launched: Launched,
    bridge_started: bool,
    bridge_ok: bool,
}

/// Arranca una app sola, levantando antes el puente del juego si hace falta.
#[tauri::command]
async fn start_app_for_game(game: String, app: String) -> Result<GameAppStart, String> {
    blocking(move || {
        let cfg = Config::load()?;
        let def = detect::game_def(&game)
            .with_context(|| format!("juego desconocido: {game}"))?;
        let kind = AppKind::parse(&app).ok_or_else(|| anyhow!("aplicacion desconocida: {app}"))?;

        for other in detect::GAMES {
            if other.id != def.id
                && state::any_matching(&other.bridge_needle())
            {
                bail!(
                    "hay un puente activo de {}; pulsa Parar en su ficha antes",
                    other.name
                );
            }
        }

        let mut bridge_started = false;
        if !state::any_matching(&def.bridge_needle()) {
            launch::spawn(&cfg, &game, Mode::Puente, cfg.hf8_disabled(), &[])?;
            bridge_started = true;
            for _ in 0..30 {
                let ready = state::any_matching(&def.bridge_needle())
                    && (def.port == 0 || state::udp_in_use(def.port));
                if ready {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }

        let launched = launch::start_app(&cfg, kind)?;
        with_watcher(|watcher| watcher.note_manual_start(&game));
        let bridge_ok = state::any_matching(&def.bridge_needle());
        logs::info(
            "apps",
            format!(
                "{}: arrancado con puente {} ({})",
                kind.tag(),
                game,
                if bridge_started { "nuevo" } else { "ya activo" }
            ),
        );
        Ok(GameAppStart {
            launched,
            bridge_started,
            bridge_ok,
        })
    })
    .await
}

#[tauri::command]
async fn configure_hf8() -> Result<String, String> {
    blocking(|| {
        let cfg = Config::load()?;
        launch::run_hf8(&cfg)
    })
    .await
}

#[tauri::command]
async fn env_preview() -> Result<String, String> {
    blocking(|| {
        let cfg = Config::load()?;
        Ok(envgen::render(&cfg))
    })
    .await
}

#[tauri::command]
async fn open_path(which: String) -> Result<String, String> {
    blocking(move || {
        let path: PathBuf = match which.as_str() {
            "config" => config::config_path()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(config::root),
            "env" => envgen::default_path(),
            "logs" => logs::dir(),
            other => return Err(anyhow!("ruta desconocida: {other}")),
        };

        let target = if path.is_dir() {
            path
        } else {
            path.parent().map(PathBuf::from).unwrap_or(path)
        };

        if !target.exists() {
            return Err(anyhow!("no existe {}", target.display()));
        }

        Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .with_context(|| format!("no se pudo abrir {}", target.display()))?;

        Ok(target.display().to_string())
    })
    .await
}

// ---------------------------------------------------------------------------
// Bandeja del sistema
// ---------------------------------------------------------------------------

/// Estado de la bandeja. `key` evita reconstruir el menu en cada refresco.
struct TrayState {
    ready: bool,
    key: Mutex<String>,
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn tray_launch(game: &str) -> Result<()> {
    let cfg = Config::load()?;
    if !cfg.games.get(game).map(|game| game.installed).unwrap_or(false) {
        bail!("el juego {game} no esta instalado");
    }
    with_watcher(|watcher| watcher.note_manual_start(game));
    if let Err(error) = launch::spawn(&cfg, game, Mode::Todo, cfg.hf8_disabled(), &[]) {
        with_watcher(|watcher| watcher.cancel_manual_start());
        return Err(error);
    }
    Ok(())
}

fn tray_stop(game: &str) -> Result<()> {
    let cfg = Config::load()?;
    if !cfg.games.get(game).map(|game| game.installed).unwrap_or(false) {
        bail!("el juego {game} no esta instalado");
    }
    let report = launch::stop_all(&cfg, game);
    with_watcher(|watcher| watcher.note_manual_stop(game));
    if !report.all_ok() {
        bail!("el puente o alguna aplicacion no se pudo cerrar");
    }
    Ok(())
}

/// Juego al que afecta una accion del vigilante (si tiene uno).
fn action_game(action: &Action) -> Option<&str> {
    match action {
        Action::StopBridge(game) | Action::StartBridge(game) => Some(game),
        Action::StartApp(game, _) => Some(game),
        Action::StopApps(game) => Some(game),
    }
}

/// Ejecuta las acciones del vigilante y deja constancia en el log.
fn run_daemon_actions(app: &tauri::AppHandle, actions: Vec<Action>) {
    let Ok(cfg) = Config::load() else {
        logs::warn("daemon", "no se pudo leer la configuracion; acciones descartadas");
        return;
    };

    let mut last_app: Option<Instant> = None;
    for action in actions {
        // Si el usuario acaba de pulsar «Lanzar», el script ya esta levantando
        // el puente y las apps: el vigilante no debe duplicarlo.
        let manual = action_game(&action)
            .map(|game| with_watcher(|watcher| watcher.manual_active(game)))
            .unwrap_or(false);
        if manual && !matches!(action, Action::StopBridge(_)) {
            logs::info(
                "daemon",
                format!(
                    "{}: arranque manual en curso; no duplico",
                    action_game(&action).unwrap_or("etol")
                ),
            );
            continue;
        }
        match action {
            Action::StopBridge(game) => {
                if !bridge_running(&game) {
                    logs::info("daemon", format!("{game}: el puente ya estaba parado"));
                } else {
                    match launch::stop_bridge(&cfg, &game) {
                        Ok(detail) => logs::info(
                            "daemon",
                            format!(
                                "{game}: {}",
                                detail.lines().next().unwrap_or("puente parado")
                            ),
                        ),
                        Err(error) => logs::warn("daemon", format!("{game}: {error:#}")),
                    }
                    wait_until(Duration::from_secs(10), Duration::from_millis(300), || {
                        !bridge_running(&game)
                    });
                }
            }
            Action::StopApps(game) => {
                for report in launch::stop_apps_for(&cfg, &game) {
                    logs::info(
                        "daemon",
                        format!(
                            "{}: {} ({})",
                            report.app,
                            if report.closed {
                                "cerrado"
                            } else {
                                "sigue abierto"
                            },
                            report.detail
                        ),
                    );
                }
                last_app = None;
            }
            Action::StartBridge(game) => {
                // Un solo puente a la vez: retirar cualquier otro que SIGA vivo.
                for def in detect::GAMES {
                    if def.id != game && bridge_running(def.id) {
                        if let Err(error) = launch::stop_bridge(&cfg, def.id) {
                            logs::warn("daemon", format!("{}: {error:#}", def.id));
                        }
                        wait_until(Duration::from_secs(10), Duration::from_millis(300), || {
                            !bridge_running(def.id)
                        });
                    }
                }

                if bridge_running(&game) {
                    logs::info("daemon", format!("{game}: puente ya activo"));
                } else {
                    match launch::spawn(&cfg, &game, Mode::Puente, cfg.hf8_disabled(), &[]) {
                        Ok(launched) => logs::info(
                            "daemon",
                            format!(
                                "{game}: puente lanzado (pid {}, log {})",
                                launched.pid, launched.log
                            ),
                        ),
                        Err(error) => logs::error("daemon", format!("{game}: {error:#}")),
                    }
                }

                let port = detect::game_def(&game).map(|def| def.port).unwrap_or(0);
                let ready = wait_until(
                    DAEMON_BRIDGE_TIMEOUT,
                    Duration::from_millis(500),
                    || bridge_running(&game) && (port == 0 || state::udp_in_use(port)),
                );
                if ready {
                    logs::info("daemon", format!("{game}: puente listo"));
                } else {
                    logs::warn(
                        "daemon",
                        format!(
                            "{game}: el puente no publico el puerto {port} a tiempo; sigo con las apps"
                        ),
                    );
                }
            }
            Action::StartApp(game, kind) => {
                if let Some(last) = last_app {
                    let gap = DAEMON_APP_GAP.saturating_sub(last.elapsed());
                    if !gap.is_zero() {
                        std::thread::sleep(gap);
                    }
                }
                match launch::start_app(&cfg, kind) {
                    Ok(_) => logs::info("daemon", format!("{game}: {} abierto", kind.name())),
                    Err(error) => {
                        let text = format!("{error:#}");
                        if text.contains("ya esta en marcha") {
                            logs::info(
                                "daemon",
                                format!("{game}: {} ya estaba abierto", kind.name()),
                            );
                        } else {
                            logs::error(
                                "daemon",
                                format!("{game}: {}: {text}", kind.name()),
                            );
                        }
                    }
                }
                last_app = Some(Instant::now());
            }
        }
        refresh_tray(app);
    }
}

/// Hilo de fondo: cada segundo mira si hay un simulador abierto y reacciona.
fn spawn_daemon(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        logs::info("daemon", "vigilante iniciado");
        let mut overlay_on = kwin::status() == kwin::Status::Enabled;
        let mut overlay_error = false;
        // La configuracion solo se relee cuando cambia el fichero.
        let mut cached: Option<(Option<std::time::SystemTime>, Arc<Config>)> = None;
        loop {
            std::thread::sleep(DAEMON_TICK);
            if QUITTING.load(Ordering::Relaxed) {
                break;
            }
            let stamp = std::fs::metadata(config::config_path())
                .and_then(|meta| meta.modified())
                .ok();
            let cfg: Arc<Config> = match &cached {
                Some((cached_stamp, cfg)) if *cached_stamp == stamp => Arc::clone(cfg),
                _ => match Config::load() {
                    Ok(cfg) => {
                        let cfg = Arc::new(cfg);
                        cached = Some((stamp, Arc::clone(&cfg)));
                        cfg
                    }
                    Err(error) => {
                        logs::warn("daemon", format!("config: {error:#}"));
                        continue;
                    }
                },
            };

            // Un solo barrido de /proc por tick: lo usan KWin y el vigilante.
            let running = daemon::running_games(&cfg);
            let desired = cfg.overlays.kwin && !running.is_empty();
            if desired != overlay_on {
                match kwin::set_enabled(desired) {
                    Ok(()) => {
                        overlay_on = desired;
                        overlay_error = false;
                        logs::info(
                            "kwin",
                            format!(
                                "dashboards sobre el juego: {}",
                                if desired { "activados" } else { "desactivados" }
                            ),
                        );
                    }
                    Err(error) => {
                        if !overlay_error {
                            logs::warn(
                                "kwin",
                                format!(
                                    "no se pudo {} la regla de overlays: {error:#}",
                                    if desired { "activar" } else { "desactivar" }
                                ),
                            );
                            overlay_error = true;
                        }
                    }
                }
            }

            let actions = with_watcher(|watcher| {
                watcher.tick_with(&cfg, Instant::now(), &running)
            });
            if actions.is_empty() {
                continue;
            }
            logs::info("daemon", format!("acciones: {actions:?}"));
            run_daemon_actions(&app, actions);
        }
    });
}

/// Para los puentes y cierra SimHub/CrewChief antes de salir de la app.
fn cleanup_before_exit() {
    // La regla de overlays no debe quedar activa sin ETOL.
    kwin::disable_if_exists();

    let Ok(cfg) = Config::load() else {
        logs::warn("quit", "no se pudo leer la configuracion; salgo sin limpiar");
        return;
    };

    logs::capture_run(&cfg.bridge.run_base);

    for def in detect::GAMES {
        let running = state::any_matching(&def.bridge_needle())
            || state::any_matching(&format!("{}_bridge_watchdog", def.id));
        if !running {
            continue;
        }
        match launch::stop_bridge(&cfg, def.id) {
            Ok(detail) => logs::info(
                "quit",
                format!(
                    "{}: {}",
                    def.id,
                    detail.lines().next().unwrap_or("puente parado")
                ),
            ),
            Err(error) => logs::warn("quit", format!("{}: {error:#}", def.id)),
        }
    }

    for report in launch::stop_apps(&cfg) {
        if !report.closed {
            logs::warn(
                "quit",
                format!("{}: sigue abierto ({})", report.app, report.detail),
            );
        }
    }
}

/// Clave del menu: cambia cuando cambia el conjunto de juegos instalados.
fn tray_key(cfg: &Config) -> String {
    detect::GAMES
        .iter()
        .filter_map(|def| {
            let game = cfg.games.get(def.id)?;
            game.installed.then(|| format!("{}:{}", def.id, game.name))
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// Construye el menu de la bandeja mostrando solo los juegos instalados.
fn build_menu(app: &tauri::AppHandle, cfg: &Config) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(
        app,
        "show",
        i18n::t("Mostrar ventana", "Mostra la finestra", "Show window"),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        i18n::t("Salir", "Sortir", "Quit"),
        true,
        None::<&str>,
    )?;

    let menu = Menu::new(app)?;
    menu.append(&show)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let installed: Vec<_> = detect::GAMES
        .iter()
        .filter_map(|def| {
            let game = cfg.games.get(def.id)?;
            game.installed.then_some((def, game))
        })
        .collect();

    if installed.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "no-games",
            i18n::t(
                "No hay simuladores instalados",
                "No hi ha simuladors instal·lats",
                "No simulators installed",
            ),
            false,
            None::<&str>,
        )?;
        menu.append(&empty)?;
    } else {
        for (def, game) in installed {
            let start = MenuItem::with_id(
                app,
                format!("launch:{}", def.id),
                i18n::t("Lanzar", "Llança", "Launch"),
                true,
                None::<&str>,
            )?;
            let stop = MenuItem::with_id(
                app,
                format!("stop:{}", def.id),
                i18n::t("Parar todo", "Atura-ho tot", "Stop everything"),
                true,
                None::<&str>,
            )?;
            let submenu =
                Submenu::with_items(app, game.name.clone(), true, &[&start, &stop])?;
            menu.append(&submenu)?;
        }
    }

    let crew_start = MenuItem::with_id(
        app,
        "app:crewchief:start",
        i18n::t("Abrir CrewChief", "Obre CrewChief", "Open CrewChief"),
        true,
        None::<&str>,
    )?;
    let crew_stop = MenuItem::with_id(
        app,
        "app:crewchief:stop",
        i18n::t("Cerrar CrewChief", "Tanca CrewChief", "Close CrewChief"),
        true,
        None::<&str>,
    )?;
    let sim_start = MenuItem::with_id(
        app,
        "app:simhub:start",
        i18n::t("Abrir SimHub", "Obre SimHub", "Open SimHub"),
        true,
        None::<&str>,
    )?;
    let sim_stop = MenuItem::with_id(
        app,
        "app:simhub:stop",
        i18n::t("Cerrar SimHub", "Tanca SimHub", "Close SimHub"),
        true,
        None::<&str>,
    )?;
    let apps_menu = Submenu::with_items(
        app,
        i18n::t("Aplicaciones", "Aplicacions", "Applications"),
        true,
        &[&crew_start, &crew_stop, &sim_start, &sim_stop],
    )?;
    menu.append(&apps_menu)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&quit)?;

    Ok(menu)
}

/// Reconstruye el menu si cambio el conjunto de juegos instalados.
fn refresh_tray(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    if !state.ready {
        return;
    }

    let cfg = Config::load().unwrap_or_else(|_| detect::detect());
    let key = format!("{}|{}", tray_key(&cfg), i18n::code(i18n::current()));
    let mut current = match state.key.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if *current == key {
        return;
    }

    let Some(tray) = app.tray_by_id("etol") else {
        return;
    };
    match build_menu(app, &cfg) {
        Ok(menu) => {
            if tray.set_menu(Some(menu)).is_ok() {
                *current = key;
            }
        }
        Err(error) => eprintln!("bandeja: no se pudo reconstruir el menu: {error}"),
    }
}

/// Crea el icono de bandeja: mostrar ventana, un submenu por juego instalado
/// y salir.
fn build_tray(app: &tauri::AppHandle, cfg: &Config) -> tauri::Result<()> {
    let menu = build_menu(app, cfg)?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

    TrayIconBuilder::with_id("etol")
        .icon(icon)
        .tooltip("ETOL")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "show" => show_main(app),
                "quit" => {
                    QUITTING.store(true, Ordering::Relaxed);
                    let app = app.clone();
                    std::thread::spawn(move || {
                        cleanup_before_exit();
                        release_single_instance();
                        logs::info("quit", "saliendo de ETOL");
                        app.exit(0);
                    });
                }
                other => {
                    if let Some(game) = other.strip_prefix("launch:") {
                        let game = game.to_string();
                        std::thread::spawn(move || {
                            if let Err(error) = tray_launch(&game) {
                                eprintln!("bandeja: no se pudo arrancar {game}: {error:#}");
                            }
                        });
                    } else if let Some(game) = other.strip_prefix("stop:") {
                        let game = game.to_string();
                        std::thread::spawn(move || {
                            if let Err(error) = tray_stop(&game) {
                                eprintln!("bandeja: no se pudo parar {game}: {error:#}");
                            }
                        });
                    } else if let Some(rest) = other.strip_prefix("app:") {
                        let mut parts = rest.splitn(2, ':');
                        let kind = parts.next().unwrap_or("").to_string();
                        let action = parts.next().unwrap_or("").to_string();
                        std::thread::spawn(move || {
                            let Some(kind) = AppKind::parse(&kind) else {
                                return;
                            };
                            let cfg = Config::load().unwrap_or_else(|_| detect::detect());
                            let result = if action == "start" {
                                launch::start_app(&cfg, kind).map(|_| ())
                            } else {
                                let report = launch::stop_app(&cfg, kind);
                                if report.closed {
                                    Ok(())
                                } else {
                                    Err(anyhow!("no se pudo cerrar {}", kind.name()))
                                }
                            };
                            if let Err(error) = result {
                                eprintln!("bandeja: {}: {error:#}", kind.name());
                            }
                        });
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

#[derive(Serialize)]
struct ScriptPreview {
    key: String,
    path: String,
    content: String,
}

#[derive(Serialize)]
struct ScriptRun {
    ok: bool,
    code: Option<i32>,
    path: String,
    output: String,
}

/// Devuelve el script de sistema tal y como se ejecutaria (para inspeccionarlo).
#[tauri::command]
async fn script_preview(key: String) -> Result<ScriptPreview, String> {
    blocking(move || {
        let path = etol::payload::system_script(&key)
            .with_context(|| format!("no existe el script «{key}»; pulsa Redetectar"))?;
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("no se pudo leer {}", path.display()))?;
        Ok(ScriptPreview {
            key,
            path: path.display().to_string(),
            content,
        })
    })
    .await
}

/// Ejecuta un script de sistema y devuelve su salida completa.
///
/// Los instaladores abren su propia ventana; el comando espera a que el
/// script termine (puede tardar varios minutos).
#[tauri::command]
async fn run_setup_script(key: String) -> Result<ScriptRun, String> {
    blocking(move || {
        let path = etol::payload::system_script(&key)
            .with_context(|| format!("no existe el script «{key}»; pulsa Redetectar"))?;
        logs::info(
            "setup",
            format!("ejecutando {} ({key})", path.display()),
        );
        let output = Command::new("bash")
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .output()
            .with_context(|| format!("no se pudo ejecutar {}", path.display()))?;
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        let ok = output.status.success();
        logs::info(
            "setup",
            format!("{key}: {}", if ok { "ok" } else { "error" }),
        );
        Ok(ScriptRun {
            ok,
            code: output.status.code(),
            path: path.display().to_string(),
            output: text,
        })
    })
    .await
}

#[derive(Serialize)]
struct GameConfigRun {
    ok: bool,
    code: Option<i32>,
    script: String,
    output: String,
}

/// Aplica solo la configuracion del juego (sin arrancar el puente ni apps).
#[tauri::command]
async fn configure_game(game: String) -> Result<GameConfigRun, String> {
    blocking(move || {
        let cfg = Config::load()?;
        detect::game_def(&game)
            .with_context(|| format!("juego desconocido: {game}"))?;
        let app_dir = cfg
            .app_dir()
            .context("no se conoce la carpeta de scripts; usa Redetectar")?;
        let script = app_dir.join("configurar_juego.sh");
        if !script.is_file() {
            bail!("no existe {}", script.display());
        }

        logs::info(
            "config",
            format!("{game}: {}", script.display()),
        );
        let mut command = Command::new("bash");
        command
            .arg(&script)
            .arg(&game)
            .stdin(Stdio::null());
        for (key, value) in launch::env_for(&cfg, &game) {
            command.env(key, value);
        }

        let output = command
            .output()
            .with_context(|| format!("no se pudo ejecutar {}", script.display()))?;
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        let ok = output.status.success();
        if ok {
            // El engranaje deja de estar rojo en cuanto la configuracion pasa.
            status::mark_configured(&game);
        }
        logs::info(
            "config",
            format!("{game}: {}", if ok { "ok" } else { "error" }),
        );
        Ok(GameConfigRun {
            ok,
            code: output.status.code(),
            script: script.display().to_string(),
            output: text,
        })
    })
    .await
}

#[tauri::command]
async fn port_check_plan(port: u16) -> Result<portcheck::PortCheckPlan, String> {
    blocking(move || portcheck::plan(port)).await
}

#[tauri::command]
async fn open_port_check(port: u16) -> Result<portcheck::PortCheckPlan, String> {
    blocking(move || portcheck::open(port)).await
}

#[tauri::command]
async fn port_check_result(port: u16) -> Result<Option<portcheck::PortCheckResult>, String> {
    blocking(move || Ok(portcheck::result(port))).await
}

/// Evita dos instancias de la interfaz: cada una tendria su propio vigilante.
///
/// Usa un fichero con el PID. Si el proceso anterior ya no existe (o reciclo
/// el PID), se considera rancio y se reescribe.
fn acquire_single_instance() -> bool {
    let path = config::root().join("etol-gui.lock");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                let _ = writeln!(file, "{}", std::process::id());
                return true;
            }
            Err(_) => {
                let alive = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| text.trim().parse::<u32>().ok())
                    .map(|pid| {
                        let dir = format!("/proc/{pid}");
                        std::path::Path::new(&dir).is_dir()
                            && std::fs::read(format!("{dir}/cmdline"))
                                .map(|raw| {
                                    String::from_utf8_lossy(&raw).contains("etol-gui")
                                })
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if alive {
                    return false;
                }
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    true
}

fn release_single_instance() {
    let _ = std::fs::remove_file(config::root().join("etol-gui.lock"));
}

fn main() {
    etol::sanitize_env();
    logs::init();
    if !acquire_single_instance() {
        eprintln!("ETOL ya esta abierto: usando la ventana existente.");
        logs::info("gui", "segunda instancia ignorada");
        return;
    }
    logs::info("gui", "arranque de la interfaz");
    tauri::Builder::default()
        .setup(|app| {
            let info = payload::ensure();
            if info.ok {
                if let Some(dir) = &info.dir {
                    eprintln!("usando scripts en {dir}");
                    logs::info("payload", format!("usando scripts en {dir}"));
                    detect::set_app_dir_override(PathBuf::from(dir));
                }
            } else {
                eprintln!("error: {}", info.detail);
                logs::error("payload", &info.detail);
                if let Some(fix) = &info.fix {
                    eprintln!("  -> {fix}");
                    logs::info("payload", format!("arreglo: {fix}"));
                }
            }

            let cfg = Config::load().unwrap_or_else(|_| detect::detect());
            logs::capture_run(&cfg.bridge.run_base);
            let key = tray_key(&cfg);
            let ready = match build_tray(app.handle(), &cfg) {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("aviso: no se pudo crear el icono de bandeja: {error}");
                    false
                }
            };
            app.manage(TrayState {
                ready,
                key: Mutex::new(if ready { key } else { String::new() }),
            });

            spawn_daemon(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let has_tray = window
                    .app_handle()
                    .try_state::<TrayState>()
                    .map(|state| state.ready)
                    .unwrap_or(false);
                if has_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            detect,
            get_config,
            set_config,
            status,
            doctor_report,
            fix_locks,
            launch,
            stop,
            start_app,
            start_app_for_game,
            stop_app,
            configure_hf8,
            env_preview,
            open_path,
            script_preview,
            run_setup_script,
            configure_game,
            port_check_plan,
            open_port_check,
            port_check_result
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la interfaz de etol");
    release_single_instance();
}

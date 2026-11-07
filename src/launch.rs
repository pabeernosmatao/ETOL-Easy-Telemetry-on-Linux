//! Lanzamiento de los scripts existentes con el entorno ya resuelto.

use crate::config::Config;
use crate::state;
use crate::util;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Puente + CrewChief + SimHub, con pausas.
    Todo,
    /// Solo el ciclo del puente.
    Puente,
    /// Para el puente (no cierra las apps).
    Stop,
}

impl Mode {
    pub fn script_name(self, game_id: &str) -> String {
        match self {
            Mode::Todo => format!("arrancar_todo_{game_id}.sh"),
            Mode::Puente => format!("activar_puente_{game_id}.sh"),
            Mode::Stop => format!("desactivar_puente_{game_id}.sh"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Todo => "todo",
            Mode::Puente => "puente",
            Mode::Stop => "parar",
        }
    }
}

/// Variables que exporta el AppImage y que no deben llegar a los scripts.
///
/// Los hijos usan el `python3`, el `wine` y el `xdg-open` del sistema: si
/// heredan `LD_LIBRARY_PATH` o los modulos de GTK del montaje del AppImage,
/// cargan librerias equivocadas y fallan (ya paso con PYTHONHOME).
const APPIMAGE_ENV: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "GI_TYPELIB_PATH",
    "GDK_PIXBUF_MODULE_FILE",
    "GIO_MODULE_DIR",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_PATH",
    "PYTHONHOME",
    "PYTHONPATH",
];

/// Limpia el entorno de un proceso hijo.
pub fn clean_child_env(command: &mut Command) {
    for key in APPIMAGE_ENV {
        command.env_remove(key);
    }
}

/// Variables de entorno que esperan los scripts de SIMHUB_LINUX.
pub fn env_for(cfg: &Config, game_id: &str) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = Vec::new();

    if let Some(path) = &cfg.apps.bottle {
        env.push(("SIMHUB_BOTTLE".into(), path.display().to_string()));
    }
    if let Some(path) = &cfg.apps.wine {
        env.push(("BOTTLE_WINE".into(), path.display().to_string()));
    }

    // El setup del HF8 se ejecuta al activar cada puente; necesita el puerto
    // y el VID/PID configurados.
    env.push(("HF8_COM".into(), cfg.hf8.com_port.clone()));
    env.push(("HF8_VID".into(), cfg.hf8.vendor_id.clone()));
    env.push(("HF8_PID".into(), cfg.hf8.product_id.clone()));

    let Some(game) = cfg.games.get(game_id) else {
        return env;
    };
    let upper = game.id.to_uppercase();

    if let Some(path) = &game.prefix {
        env.push((format!("{upper}_PFX"), path.display().to_string()));
    }

    // LMU usa el directorio de Proton; AMS2 y AC EVO, el binario wine.
    if game_id.eq_ignore_ascii_case("lmu") {
        if let Some(path) = &game.proton {
            env.push((format!("{upper}_PROTON"), path.display().to_string()));
        }
    } else if let Some(path) = game.wine.as_ref().or_else(|| {
        let native = crate::detect::game_def(game_id)
            .map(|def| def.native)
            .unwrap_or(false);
        if native {
            None
        } else {
            cfg.apps.wine.as_ref()
        }
    }) {
        // Si Steam no expone el Proton del juego (p. ej. AC EVO con un runner
        // que no aparece en compatibilitytools.d), el lanzador necesita algo.
        // El wine de Bottles sirve para configurar; el puente avisara si no
        // puede leer la memoria del juego. Los juegos nativos no usan wine.
        if game.wine.is_none() {
            crate::logs::warn(
                "launch",
                format!("{game_id}: sin wine de Steam; uso el wine de Bottles"),
            );
        }
        env.push((format!("{upper}_WINE"), path.display().to_string()));
    }

    if !game.run_dir.as_os_str().is_empty() {
        env.push((
            format!("{upper}_MIRROR_RUN_DIR"),
            game.run_dir.display().to_string(),
        ));
    }

    // Carpeta del juego y plugin del puente (ETS2 instala el .so del SDK).
    if let Some(dir) = &game.path {
        env.push((format!("{upper}_GAME_DIR"), dir.display().to_string()));
    }
    if let (Some(def), Some(dir)) = (crate::detect::game_def(game_id), &game.path) {
        if let Some(plugin) = def.plugin_path(dir) {
            env.push((format!("{upper}_PLUGIN"), plugin.display().to_string()));
        }
    }

    env
}

pub fn script_path(cfg: &Config, game_id: &str, mode: Mode) -> Result<PathBuf> {
    let app_dir = cfg
        .app_dir()
        .context("no se conoce la carpeta de scripts; usa `etol config set bridge.app_dir <ruta>`")?;
    Ok(app_dir.join(mode.script_name(game_id)))
}

pub fn launch(
    cfg: &Config,
    game_id: &str,
    mode: Mode,
    dry_run: bool,
    no_hf8: bool,
    extra: &[String],
) -> Result<()> {
    let script = script_path(cfg, game_id, mode)?;
    if !script.is_file() {
        bail!("no existe el script {}", script.display());
    }

    let mut args: Vec<String> = Vec::new();
    if no_hf8 && mode != Mode::Stop {
        args.push("--no-hf8".to_string());
    }
    args.extend(extra.iter().cloned());

    let env = env_for(cfg, game_id);
    let app_dir = cfg.app_dir().map(Path::to_path_buf);

    if dry_run {
        println!("{}", util::bold("Simulacion (no se ejecuta nada)"));
        if let Some(dir) = &app_dir {
            println!("  cd {}", dir.display());
        }
        for (key, value) in &env {
            println!("  {key}={}", util::sh_quote(value));
        }
        println!(
            "  bash {} {}",
            script.display(),
            args.join(" ")
        );
        return Ok(());
    }

    println!(
        "{} {}",
        util::cyan("Ejecutando"),
        script.display()
    );
    crate::logs::info(
        "launch",
        format!("{game_id} {} {}", mode.label(), script.display()),
    );

    let mut command = Command::new("bash");
    command.arg(&script).args(&args);
    for (key, value) in &env {
        command.env(key, value);
    }
    clean_child_env(&mut command);
    if let Some(dir) = &app_dir {
        command.current_dir(dir);
    }

    let status = command
        .status()
        .with_context(|| format!("no se pudo ejecutar {}", script.display()))?;

    if !status.success() {
        bail!("el script termino con {}", status);
    }
    Ok(())
}

/// Entorno especifico del configurador de HF8.
pub fn hf8_env(cfg: &Config) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(path) = &cfg.apps.bottle {
        env.push(("SIMHUB_BOTTLE".into(), path.display().to_string()));
    }
    if let Some(path) = &cfg.apps.wine {
        env.push(("BOTTLE_WINE".into(), path.display().to_string()));
    }
    env.push(("HF8_COM".into(), cfg.hf8.com_port.clone()));
    env.push(("HF8_VID".into(), cfg.hf8.vendor_id.clone()));
    env.push(("HF8_PID".into(), cfg.hf8.product_id.clone()));
    env.push((
        "LMU_MIRROR_RUN_DIR".into(),
        cfg.bridge.run_base.join("lmu_mirror").display().to_string(),
    ));
    env
}

// ---------------------------------------------------------------------------
// Lanzamiento en segundo plano (interfaz grafica)
// ---------------------------------------------------------------------------

/// Resultado de un lanzamiento en segundo plano.
#[derive(Debug, Clone, Serialize)]
pub struct Launched {
    pub pid: u32,
    pub script: String,
    pub log: String,
    pub game: String,
    pub mode: String,
}

/// Aplicaciones que se pueden abrir por separado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKind {
    SimHub,
    CrewChief,
}

impl AppKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "simhub" => Some(AppKind::SimHub),
            "crewchief" | "crew" => Some(AppKind::CrewChief),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            AppKind::SimHub => "simhub",
            AppKind::CrewChief => "crewchief",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            AppKind::SimHub => "SimHub",
            AppKind::CrewChief => "CrewChief",
        }
    }

    pub fn process_needle(self) -> &'static str {
        match self {
            AppKind::SimHub => "SimHubWPF.exe",
            AppKind::CrewChief => "CrewChiefV4.exe",
        }
    }
}

/// Lanza un script de SIMHUB_LINUX sin bloquear y con la salida en un log.
///
/// Se usa `setsid` para separar el proceso del grupo de la interfaz: cerrar la
/// ventana no detiene el puente ni las aplicaciones.
pub fn spawn(
    cfg: &Config,
    game_id: &str,
    mode: Mode,
    no_hf8: bool,
    extra: &[String],
) -> Result<Launched> {
    let script = script_path(cfg, game_id, mode)?;
    if !script.is_file() {
        bail!("no existe el script {}", script.display());
    }

    let game = cfg
        .games
        .get(game_id)
        .with_context(|| format!("el juego {game_id} no esta en la configuracion"))?;

    let run_dir = if game.run_dir.as_os_str().is_empty() {
        cfg.bridge.run_base.join(format!("{game_id}_mirror"))
    } else {
        game.run_dir.clone()
    };
    std::fs::create_dir_all(&run_dir)
        .with_context(|| format!("no se pudo crear {}", run_dir.display()))?;

    let log = run_dir.join(format!("etol_{}.log", mode.label()));
    let (stdout, stderr) = log_files(&log)?;

    let mut args: Vec<String> = Vec::new();
    if no_hf8 && mode != Mode::Stop {
        args.push("--no-hf8".to_string());
    }
    args.extend(extra.iter().cloned());

    let mut command = detached("bash");
    command.arg(&script).args(&args);
    for (key, value) in env_for(cfg, game_id) {
        command.env(key, value);
    }
    clean_child_env(&mut command);
    if let Some(dir) = cfg.app_dir() {
        command.current_dir(dir);
    }

    let child = command
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .with_context(|| format!("no se pudo ejecutar {}", script.display()))?;

    crate::logs::info(
        "launch",
        format!(
            "{game_id} {} pid {} log {}",
            mode.label(),
            child.id(),
            log.display()
        ),
    );

    Ok(Launched {
        pid: child.id(),
        script: script.display().to_string(),
        log: log.display().to_string(),
        game: game_id.to_string(),
        mode: mode.label().to_string(),
    })
}

/// Abre SimHub o CrewChief dentro de la botella, sin bloquear.
///
/// No hace nada si el proceso ya esta en marcha: los scripts de arranque
/// tampoco reinician aplicaciones abiertas.
pub fn start_app(cfg: &Config, kind: AppKind) -> Result<Launched> {
    let exe = match kind {
        AppKind::SimHub => cfg.apps.simhub_exe.clone(),
        AppKind::CrewChief => cfg.apps.crewchief_exe.clone(),
    }
    .with_context(|| format!("no se conoce el ejecutable de {}", kind.tag()))?;

    if !exe.is_file() {
        bail!("no existe {}", exe.display());
    }
    if !state::running_windows_exe(kind.process_needle()).is_empty() {
        bail!("{} ya esta en marcha", kind.tag());
    }

    let bottle = cfg
        .apps
        .bottle
        .as_ref()
        .context("no se conoce la botella de SimHub")?;
    let wine = cfg
        .apps
        .wine
        .as_ref()
        .context("no se conoce el wine de Bottles")?;
    if !wine.is_file() {
        bail!("no existe {}", wine.display());
    }

    let exe_dir = exe
        .parent()
        .context("el ejecutable no tiene carpeta padre")?;
    let dir_win = windows_path(bottle, exe_dir);
    let exe_win = windows_path(bottle, &exe);

    let run_dir = cfg.bridge.run_base.join("etol_apps");
    std::fs::create_dir_all(&run_dir)
        .with_context(|| format!("no se pudo crear {}", run_dir.display()))?;
    let log = run_dir.join(format!("{}.log", kind.tag()));
    let (stdout, stderr) = log_files(&log)?;

    let mut command = detached(wine.to_string_lossy().as_ref());
    command
        .arg("start")
        .arg("/wait")
        .arg("/d")
        .arg(&dir_win)
        .arg(&exe_win)
        .env("WINEPREFIX", bottle)
        .env("WINEDEBUG", "-all")
        .current_dir(exe_dir)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    clean_child_env(&mut command);

    let child = command
        .spawn()
        .with_context(|| format!("no se pudo ejecutar {}", wine.display()))?;

    crate::logs::info(
        "apps",
        format!("{} pid {} log {}", kind.tag(), child.id(), log.display()),
    );

    Ok(Launched {
        pid: child.id(),
        script: exe_win,
        log: log.display().to_string(),
        game: kind.tag().to_string(),
        mode: "app".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Parada: puente y aplicaciones
// ---------------------------------------------------------------------------

/// Resultado de una etapa de la parada.
#[derive(Debug, Clone, Serialize)]
pub struct StepReport {
    pub ok: bool,
    pub detail: String,
}

impl StepReport {
    /// Detalle en una sola linea, para la consola.
    pub fn one_line(&self) -> String {
        self.detail
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// Resultado del cierre de una aplicacion.
#[derive(Debug, Clone, Serialize)]
pub struct AppReport {
    pub app: String,
    pub closed: bool,
    pub detail: String,
}

/// Resultado de parar el puente y cerrar SimHub y CrewChief.
#[derive(Debug, Clone, Serialize)]
pub struct StopReport {
    pub game: String,
    pub bridge: StepReport,
    pub apps: Vec<AppReport>,
}

impl StopReport {
    pub fn all_ok(&self) -> bool {
        self.bridge.ok && self.apps.iter().all(|app| app.closed)
    }
}

/// Ejecuta `desactivar_puente_<juego>.sh` y devuelve su salida.
///
/// No toca SimHub ni CrewChief: para eso esta `stop_all`.
pub fn stop_bridge(cfg: &Config, game_id: &str) -> Result<String> {
    let script = script_path(cfg, game_id, Mode::Stop)?;
    if !script.is_file() {
        bail!("no existe el script {}", script.display());
    }

    let mut command = Command::new("bash");
    command
        .arg(&script)
        .envs(env_for(cfg, game_id))
        .stdin(Stdio::null());
    clean_child_env(&mut command);
    if let Some(dir) = cfg.app_dir() {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .with_context(|| format!("no se pudo ejecutar {}", script.display()))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let text = text.trim().to_string();

    if !output.status.success() {
        crate::logs::error("stop", format!("{game_id}: {text}"));
        bail!(
            "el script {} termino con {}{}",
            script.display(),
            output.status,
            if text.is_empty() {
                String::new()
            } else {
                format!(": {text}")
            }
        );
    }

    crate::logs::info(
        "stop",
        format!(
            "{game_id}: {}",
            text.lines().next().unwrap_or("puente parado")
        ),
    );
    Ok(text)
}

/// Cierra SimHub y CrewChief con `wine taskkill` y, si no basta, con senales.
///
/// Primero se pide un cierre ordenado (WM_CLOSE), luego `taskkill /F` y por
/// ultimo SIGTERM/SIGKILL a los procesos Linux. Nunca se usa `wineserver -k`:
/// la botella se comparte con otras aplicaciones.
pub fn stop_apps(cfg: &Config) -> Vec<AppReport> {
    close_apps(cfg, &[AppKind::SimHub, AppKind::CrewChief])
}

/// Apps que usa un juego, segun su definicion (`simhub`, `crewchief`).
pub fn apps_for(game_id: &str) -> Vec<AppKind> {
    match crate::detect::game_def(game_id) {
        Some(def) => def
            .apps
            .iter()
            .filter_map(|name| AppKind::parse(name))
            .collect(),
        None => vec![AppKind::SimHub, AppKind::CrewChief],
    }
}

/// Cierra solo las apps que usa ese juego.
///
/// Euro Truck Simulator 2 solo usa SimHub: al pararlo no se toca CrewChief,
/// que puede estar abierto para otro simulador.
pub fn stop_apps_for(cfg: &Config, game_id: &str) -> Vec<AppReport> {
    close_apps(cfg, &apps_for(game_id))
}

fn close_apps(cfg: &Config, kinds: &[AppKind]) -> Vec<AppReport> {
    let reports: Vec<AppReport> = kinds
        .iter()
        .copied()
        .map(|kind| stop_app(cfg, kind))
        .collect();
    for report in &reports {
        crate::logs::info(
            "apps",
            format!(
                "{}: {} ({})",
                report.app,
                if report.closed { "cerrado" } else { "sigue abierto" },
                report.detail
            ),
        );
    }
    reports
}

pub fn stop_app(cfg: &Config, kind: AppKind) -> AppReport {
    let app = kind.name().to_string();

    if app_pids(kind).is_empty() {
        return AppReport {
            app,
            closed: true,
            detail: "no estaba en marcha".to_string(),
        };
    }

    let mut notes: Vec<String> = Vec::new();

    if cfg.apps.wine.is_some() && cfg.apps.bottle.is_some() {
        match taskkill(cfg, kind, false) {
            Ok(()) => notes.push("taskkill".to_string()),
            Err(error) => notes.push(format!("taskkill: {error:#}")),
        }
        if wait_gone(kind, Duration::from_secs(4)) {
            return AppReport {
                app,
                closed: true,
                detail: notes.join(" · "),
            };
        }

        match taskkill(cfg, kind, true) {
            Ok(()) => notes.push("taskkill /F".to_string()),
            Err(error) => notes.push(format!("taskkill /F: {error:#}")),
        }
        if wait_gone(kind, Duration::from_secs(2)) {
            return AppReport {
                app,
                closed: true,
                detail: notes.join(" · "),
            };
        }
    } else {
        notes.push("sin wine configurado".to_string());
    }

    let pids = app_pids(kind);
    signal_all(&pids, "TERM");
    notes.push(format!("SIGTERM a {}", join_pids(&pids)));
    if wait_gone(kind, Duration::from_secs(2)) {
        return AppReport {
            app,
            closed: true,
            detail: notes.join(" · "),
        };
    }

    let pids = app_pids(kind);
    signal_all(&pids, "KILL");
    notes.push(format!("SIGKILL a {}", join_pids(&pids)));
    let closed = wait_gone(kind, Duration::from_secs(1));

    AppReport {
        app,
        closed,
        detail: notes.join(" · "),
    }
}

/// Para el puente y despues cierra SimHub y CrewChief.
///
/// Si el puente falla, las aplicaciones se cierran igualmente y el fallo queda
/// reflejado en el informe.
pub fn stop_all(cfg: &Config, game_id: &str) -> StopReport {
    let bridge = match stop_bridge(cfg, game_id) {
        Ok(detail) => StepReport { ok: true, detail },
        Err(error) => StepReport {
            ok: false,
            detail: format!("{error:#}"),
        },
    };

    StopReport {
        game: game_id.to_string(),
        bridge,
        apps: stop_apps_for(cfg, game_id),
    }
}

fn app_pids(kind: AppKind) -> Vec<u32> {
    state::running_windows_exe(kind.process_needle())
        .iter()
        .map(|proc| proc.pid)
        .collect()
}

fn wait_gone(kind: AppKind, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if app_pids(kind).is_empty() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn taskkill(cfg: &Config, kind: AppKind, force: bool) -> Result<()> {
    let wine = cfg
        .apps
        .wine
        .as_ref()
        .context("no se conoce el wine de Bottles")?;
    let bottle = cfg
        .apps
        .bottle
        .as_ref()
        .context("no se conoce la botella de SimHub")?;

    let mut command = wine_with_timeout(wine);
    command
        .arg("taskkill")
        .arg("/IM")
        .arg(kind.process_needle());
    if force {
        command.arg("/F");
    }
    command
        .env("WINEPREFIX", bottle)
        .env("WINEDEBUG", "-all")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    clean_child_env(&mut command);

    let status = command
        .status()
        .with_context(|| format!("no se pudo ejecutar {}", wine.display()))?;

    // taskkill devuelve error si el proceso ya no existe; el estado real se
    // comprueba despues con `wait_gone`.
    let _ = status;
    Ok(())
}

/// Limita la llamada a wine para que una botella atascada no bloquee la GUI.
fn wine_with_timeout(wine: &Path) -> Command {
    if Path::new("/usr/bin/timeout").exists() {
        let mut command = Command::new("/usr/bin/timeout");
        command.arg("8").arg(wine);
        command
    } else {
        Command::new(wine)
    }
}

fn signal_all(pids: &[u32], signal: &str) {
    for pid in pids {
        let _ = Command::new("kill")
            .arg(format!("-{signal}"))
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn join_pids(pids: &[u32]) -> String {
    pids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Ejecuta el configurador del pad HF8 (bloquea hasta terminar).
pub fn run_hf8(cfg: &Config) -> Result<String> {
    let app_dir = cfg.app_dir().context(
        "no se conoce la carpeta de scripts; usa `etol config set bridge.app_dir <ruta>`",
    )?;
    let script = app_dir.join("bridge").join("setup_hf8.sh");
    if !script.is_file() {
        bail!("no existe {}", script.display());
    }

    let mut command = Command::new("bash");
    command.arg(&script).envs(hf8_env(cfg));
    clean_child_env(&mut command);
    let output = command
        .output()
        .with_context(|| format!("no se pudo ejecutar {}", script.display()))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        bail!("el configurador de HF8 termino con {}", output.status);
    }
    Ok(text.trim().to_string())
}

/// `setsid` desacopla el proceso del grupo de la interfaz grafica.
fn detached(program: &str) -> Command {
    if Path::new("/usr/bin/setsid").exists() {
        let mut command = Command::new("/usr/bin/setsid");
        command.arg(program);
        command
    } else {
        Command::new(program)
    }
}

fn log_files(log: &Path) -> Result<(Stdio, Stdio)> {
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .with_context(|| format!("no se pudo abrir {}", log.display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("no se pudo duplicar {}", log.display()))?;
    Ok((Stdio::from(stdout), Stdio::from(stderr)))
}

/// Convierte una ruta Unix dentro de la botella a una ruta Windows.
pub fn windows_path(bottle: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(bottle.join("drive_c")) {
        let tail = relative.to_string_lossy().replace('/', "\\");
        return format!("C:\\{tail}");
    }
    format!("Z:{}", path.to_string_lossy().replace('/', "\\"))
}

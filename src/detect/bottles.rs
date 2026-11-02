//! Deteccion de botellas de Bottles, de sus runners y de los programas
//! instalados dentro.
//!
//! La resolucion del wine es de solo lectura: nunca crea prefijos, nunca
//! modifica `bottle.yml` y nunca toca el Proton del juego.

use crate::util;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Bottle {
    pub name: String,
    pub path: PathBuf,
    /// Runner declarado en `bottle.yml`.
    pub runner: Option<String>,
    /// `RunnerPath` declarado en `bottle.yml` (puede estar vacio).
    pub runner_path: Option<String>,
    /// `Update_Date` de `bottle.yml`, para desempatar de forma determinista.
    pub update_date: String,
    /// Wine del runner dentro de Bottles, si existe.
    pub wine: Option<PathBuf>,
}

/// De donde ha salido el wine resuelto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WineSource {
    BottleRunner,
    CompatTool,
    System,
    Manual,
}

impl WineSource {
    pub fn as_str(self) -> &'static str {
        match self {
            WineSource::BottleRunner => "bottle-runner",
            WineSource::CompatTool => "compat-tool",
            WineSource::System => "system",
            WineSource::Manual => "manual",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WineSource::BottleRunner => "runner de la botella",
            WineSource::CompatTool => "compatibility tool de Steam",
            WineSource::System => "wine del sistema",
            WineSource::Manual => "configurado a mano",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "bottle-runner" => Some(WineSource::BottleRunner),
            "compat-tool" => Some(WineSource::CompatTool),
            "system" => Some(WineSource::System),
            "manual" => Some(WineSource::Manual),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WineResolution {
    pub wine: Option<PathBuf>,
    pub source: Option<WineSource>,
    pub runner: Option<String>,
    /// Aviso que debe mostrarse antes de usar este wine.
    pub note: Option<String>,
    pub searched: Vec<PathBuf>,
}

/// De donde sale la instalacion de Bottles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Paquete del sistema (Arch, CachyOS, Debian...).
    Native,
    /// Flatpak de usuario (`com.usebottles.bottles`).
    Flatpak,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Origin::Native => "nativa",
            Origin::Flatpak => "Flatpak",
        }
    }
}

/// Ruta de datos de Bottles nativo.
pub fn native_base() -> PathBuf {
    util::home().join(".local/share/bottles")
}

/// Ruta de datos de Bottles Flatpak (instalacion de usuario).
pub fn flatpak_base() -> PathBuf {
    util::home().join(".var/app/com.usebottles.bottles/data/bottles")
}

/// Raices posibles de Bottles (nativo y Flatpak).
pub fn bases() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for candidate in [native_base(), flatpak_base()] {
        if candidate.is_dir() {
            out.push(candidate);
        }
    }
    out
}

/// Instalaciones de Bottles presentes en el equipo.
pub fn installed_origins() -> Vec<Origin> {
    let mut out = Vec::new();
    if native_base().is_dir() {
        out.push(Origin::Native);
    }
    if flatpak_base().is_dir() {
        out.push(Origin::Flatpak);
    }
    out
}

/// Indica de que instalacion sale una ruta (botella o runner).
pub fn origin_of(path: &Path) -> Option<Origin> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if path.starts_with(flatpak_base()) {
        Some(Origin::Flatpak)
    } else if path.starts_with(native_base()) {
        Some(Origin::Native)
    } else {
        None
    }
}

/// Permisos de sistema de ficheros del Flatpak de Bottles, si esta instalado.
///
/// Devuelve `None` cuando no hay Flatpak o no se pueden consultar. Cada entrada
/// viene sin los sufijos `:ro`/`:create`.
pub fn flatpak_filesystems() -> Option<Vec<String>> {
    let output = std::process::Command::new("flatpak")
        .args([
            "info",
            "--user",
            "--show-permissions",
            "com.usebottles.bottles",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|line| line.starts_with("filesystems="))?;
    Some(
        line.trim_start_matches("filesystems=")
            .split(';')
            .filter(|item| !item.is_empty())
            .map(|item| item.split(':').next().unwrap_or(item).to_string())
            .collect(),
    )
}

/// Estado de Bottles como Flatpak (usuario, sistema y permisos).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct FlatpakState {
    /// El binario `flatpak` existe.
    pub available: bool,
    /// `com.usebottles.bottles` instalado para el usuario.
    pub user: bool,
    /// `com.usebottles.bottles` instalado para todo el sistema.
    pub system: bool,
    /// Solo esta la copia de sistema: hay que instalar la de usuario.
    pub needs_user_install: bool,
    /// Permisos `filesystems=` efectivos (sin `:ro`/`:create`).
    pub filesystems: Vec<String>,
    /// Permisos `devices=` efectivos.
    pub devices: Vec<String>,
    /// Rutas externas (Steam/juegos) que los permisos no cubren.
    pub missing_paths: Vec<String>,
    /// Falta el permiso `home` (o su equivalente `host`).
    pub missing_home: bool,
    /// Falta `host-etc`.
    pub missing_host_etc: bool,
    /// Falta `devices=all` (necesario para el pad HF8).
    pub missing_devices: bool,
    /// Todos los permisos necesarios estan puestos.
    pub permissions_ok: bool,
}

fn flatpak_app_installed(scope: &str) -> bool {
    let Ok(output) = std::process::Command::new("flatpak")
        .args(["list", scope, "--app", "--columns=application"])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.trim() == "com.usebottles.bottles")
}

fn flatpak_version_ok() -> bool {
    std::process::Command::new("flatpak")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Lee `filesystems=` y `devices=` del Flatpak de Bottles.
fn flatpak_context(user: bool) -> (Vec<String>, Vec<String>) {
    let scope = if user { "--user" } else { "--system" };
    let Ok(output) = std::process::Command::new("flatpak")
        .args(["info", scope, "--show-permissions", "com.usebottles.bottles"])
        .output()
    else {
        return (Vec::new(), Vec::new());
    };
    if !output.status.success() {
        return (Vec::new(), Vec::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let parse = |prefix: &str| -> Vec<String> {
        text.lines()
            .find(|line| line.starts_with(prefix))
            .map(|line| {
                line.trim_start_matches(prefix)
                    .split(';')
                    .filter(|item| !item.is_empty())
                    .map(|item| item.split(':').next().unwrap_or(item).to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    (parse("filesystems="), parse("devices="))
}

/// Estado completo de Bottles Flatpak para los avisos de la interfaz.
///
/// `extra_paths` son las rutas que Bottles tiene que poder leer ademas del
/// home (bibliotecas de Steam y carpetas de juegos).
pub fn flatpak_state(extra_paths: &[PathBuf]) -> FlatpakState {
    let available = flatpak_version_ok();
    let user = available && flatpak_app_installed("--user");
    let system = available && flatpak_app_installed("--system");
    let (filesystems, devices) = if user {
        flatpak_context(true)
    } else if system {
        flatpak_context(false)
    } else {
        (Vec::new(), Vec::new())
    };

    let home = util::home();
    let missing_home = !filesystems.iter().any(|item| item == "home" || item == "host");
    let missing_host_etc = !filesystems.iter().any(|item| item == "host-etc");
    let missing_devices = !devices.iter().any(|item| item == "all");

    let mut missing_paths = Vec::new();
    for path in extra_paths {
        if !path.is_dir() {
            continue;
        }
        // Dentro del home lo cubre `home` (o `host`).
        if path.starts_with(&home) {
            continue;
        }
        if !flatpak_covers(&filesystems, path) {
            let text = path.display().to_string();
            if !missing_paths.contains(&text) {
                missing_paths.push(text);
            }
        }
    }

    // Si ya falta una biblioteca, sus subcarpetas no aportan nada al aviso.
    missing_paths.sort();
    missing_paths.dedup();
    let mut filtered: Vec<String> = Vec::new();
    for path in &missing_paths {
        if filtered
            .iter()
            .any(|parent| path == parent || path.starts_with(&format!("{parent}/")))
        {
            continue;
        }
        filtered.push(path.clone());
    }
    let missing_paths = filtered;

    let permissions_ok = user
        && !missing_home
        && !missing_host_etc
        && !missing_devices
        && missing_paths.is_empty();

    FlatpakState {
        available,
        user,
        system,
        needs_user_install: system && !user,
        filesystems,
        devices,
        missing_paths,
        missing_home,
        missing_host_etc,
        missing_devices,
        permissions_ok,
    }
}

/// Un prefijo de Botella esta listo si tiene `drive_c` y `system.reg`.
pub fn bottle_ready(path: &Path) -> bool {
    path.join("drive_c").is_dir() && path.join("system.reg").is_file()
}

/// Comprueba si los permisos del Flatpak cubren una ruta del host.
pub fn flatpak_covers(filesystems: &[String], path: &Path) -> bool {
    let home = util::home();
    let path = path.to_string_lossy();
    filesystems.iter().any(|entry| {
        let entry = match entry.as_str() {
            "home" => home.to_string_lossy().to_string(),
            "host" => return true,
            other => other.to_string(),
        };
        path == entry || path.starts_with(&format!("{entry}/"))
    })
}

/// Todas las botellas encontradas.
pub fn all() -> Vec<Bottle> {
    let mut out = Vec::new();

    for base in bases() {
        let bottles_dir = base.join("bottles");
        let Ok(entries) = std::fs::read_dir(&bottles_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let path = entry.path();
            let yml = path.join("bottle.yml");
            let runner = read_field(&yml, "Runner");
            let runner_path = read_field(&yml, "RunnerPath");
            let update_date = read_field(&yml, "Update_Date").unwrap_or_default();
            let wine = runner.as_ref().and_then(|runner| {
                let wine = base
                    .join("runners")
                    .join(runner)
                    .join("files/bin/wine");
                wine.is_file().then_some(wine)
            });
            out.push(Bottle {
                name: entry.file_name().to_string_lossy().to_string(),
                path,
                runner,
                runner_path,
                update_date,
                wine,
            });
        }
    }

    out
}

/// Reconstruye una botella a partir de su ruta (para configuraciones manuales).
pub fn from_path(path: &Path) -> Bottle {
    let yml = path.join("bottle.yml");
    let runner = read_field(&yml, "Runner");
    let runner_path = read_field(&yml, "RunnerPath");
    let update_date = read_field(&yml, "Update_Date").unwrap_or_default();
    let wine = runner.as_ref().and_then(|runner| {
        bases().into_iter().find_map(|base| {
            let wine = base.join("runners").join(runner).join("files/bin/wine");
            wine.is_file().then_some(wine)
        })
    });
    Bottle {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: path.to_path_buf(),
        runner,
        runner_path,
        update_date,
        wine,
    }
}

/// Busca un ejecutable dentro de la botella: primero en las rutas tipicas de
/// SimHub/CrewChief y despues con una busqueda acotada.
pub fn find_program(bottle: &Bottle, exe_name: &str) -> Option<PathBuf> {
    let drive_c = bottle.path.join("drive_c");

    for relative in [
        format!("Program Files (x86)/SimHub/{exe_name}"),
        format!("Program Files/SimHub/{exe_name}"),
        format!("Program Files (x86)/Britton IT Ltd/CrewChiefV4/{exe_name}"),
        format!("Program Files/Britton IT Ltd/CrewChiefV4/{exe_name}"),
    ] {
        let candidate = drive_c.join(relative);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    util::find_file(&drive_c, exe_name, 6)
}

/// Raices de compatibility tools de Steam (nativo y Flatpak).
fn compat_roots() -> Vec<PathBuf> {
    let home = util::home();
    [
        home.join(".steam/root/compatibilitytools.d"),
        home.join(".steam/steam/compatibilitytools.d"),
        home.join(".local/share/Steam/compatibilitytools.d"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d"),
        PathBuf::from("/usr/share/steam/compatibilitytools.d"),
        PathBuf::from("/usr/local/share/steam/compatibilitytools.d"),
        PathBuf::from("/usr/share/Steam/compatibilitytools.d"),
    ]
    .into_iter()
    .filter(|path| path.is_dir())
    .collect()
}

/// Resuelve el wine de la botella sin crear ni modificar nada.
///
/// Orden: `manual` (apps.wine) -> `RunnerPath` -> runners de Bottles ->
/// compatibility tools -> wine del sistema.
pub fn resolve_wine(bottle: Option<&Bottle>, manual: Option<&Path>) -> WineResolution {
    let mut searched: Vec<PathBuf> = Vec::new();
    let runner = bottle.and_then(|bottle| bottle.runner.clone());

    if let Some(path) = manual {
        searched.push(path.to_path_buf());
        if path.is_file() {
            let note = mismatch_note(path, runner.as_deref());
            return WineResolution {
                wine: Some(path.to_path_buf()),
                source: Some(WineSource::Manual),
                runner,
                note,
                searched,
            };
        }
    }

    if let Some(bottle) = bottle {
        // 1. RunnerPath explicito de bottle.yml.
        if let Some(raw) = bottle.runner_path.as_deref() {
            let raw = raw.trim();
            if !raw.is_empty() {
                let path = util::expand(raw);
                let candidates = if path.is_dir() {
                    vec![path.join("files/bin/wine"), path]
                } else {
                    vec![path]
                };
                for candidate in candidates {
                    searched.push(candidate.clone());
                    if candidate.is_file() {
                        let note = mismatch_note(&candidate, runner.as_deref());
                        return WineResolution {
                            wine: Some(candidate),
                            source: Some(WineSource::BottleRunner),
                            runner,
                            note,
                            searched,
                        };
                    }
                }
            }
        }

        // 2. Runners instalados dentro de Bottles.
        if let Some(name) = runner.as_deref() {
            for base in bases() {
                let candidate = base
                    .join("runners")
                    .join(name)
                    .join("files/bin/wine");
                searched.push(candidate.clone());
                if candidate.is_file() {
                    return WineResolution {
                        wine: Some(candidate),
                        source: Some(WineSource::BottleRunner),
                        runner,
                        note: None,
                        searched,
                    };
                }
            }

            // 3. El mismo runner instalado como compatibility tool de Steam.
            for root in compat_roots() {
                let candidate = root.join(name).join("files/bin/wine");
                searched.push(candidate.clone());
                if candidate.is_file() {
                    return WineResolution {
                        wine: Some(candidate),
                        source: Some(WineSource::CompatTool),
                        runner,
                        note: None,
                        searched,
                    };
                }
            }
        }
    }

    // 4. Wine del sistema: ultimo recurso y siempre con aviso.
    for candidate in [
        PathBuf::from("/usr/bin/wine"),
        PathBuf::from("/usr/local/bin/wine"),
        PathBuf::from("/bin/wine"),
    ] {
        searched.push(candidate.clone());
        if candidate.is_file() {
            return WineResolution {
                wine: Some(candidate.clone()),
                source: Some(WineSource::System),
                runner,
                note: Some(crate::messages::wine_system(&candidate)),
                searched,
            };
        }
    }

    WineResolution {
        wine: None,
        source: None,
        runner,
        note: None,
        searched,
    }
}

/// Rutas que se han probado para el wine (para el doctor).
pub fn wine_search_paths(bottle: Option<&Bottle>) -> Vec<PathBuf> {
    resolve_wine(bottle, None).searched
}

fn mismatch_note(path: &Path, runner: Option<&str>) -> Option<String> {
    let runner = runner?;
    let text = path.to_string_lossy();
    if text.contains(runner) {
        None
    } else {
        Some(crate::messages::wine_mismatch(path, runner))
    }
}

/// Lee un campo simple de `bottle.yml` (`Clave: valor`).
fn read_field(path: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let prefix = format!("{key}:");
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix(&prefix) else {
            continue;
        };
        let value = rest.trim().trim_matches('"').trim_matches('\'');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

//! Enlaces de las bibliotecas de Steam dentro de la botella de Bottles.
//!
//! SimHub y CrewChief buscan los juegos en
//! `C:\Program Files (x86)\Steam\SteamApps\common\<juego>`. La botella no
//! comparte el sistema de ficheros de Steam, asi que creamos enlaces por juego
//! (tambien si el juego esta en otra biblioteca o en otro disco).
//!
//! Si ya existe un enlace manual `Steam -> ~/.local/share/Steam`, se migra a
//! una carpeta gestionada y se guarda el enlace anterior como copia.

use crate::config::Config;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub migrated: Option<PathBuf>,
    pub created: Vec<PathBuf>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
    pub metadata: bool,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.migrated.is_some() || !self.created.is_empty() || self.metadata
    }

    pub fn detail(&self) -> String {
        let mut parts = Vec::new();
        if let Some(path) = &self.migrated {
            parts.push(format!("enlace manual guardado como {}", path.display()));
        }
        if !self.created.is_empty() {
            parts.push(format!("{} juego(s) enlazados", self.created.len()));
        }
        if self.metadata {
            parts.push("metadatos de Steam actualizados".to_string());
        }
        if !self.skipped.is_empty() {
            parts.push(format!("{} omitido(s)", self.skipped.len()));
        }
        if !self.errors.is_empty() {
            parts.push(format!("{} error(es)", self.errors.len()));
        }
        if parts.is_empty() {
            "sin cambios".to_string()
        } else {
            parts.join(" · ")
        }
    }
}

/// Ruta de `Program Files (x86)` dentro de la botella.
fn program_files(bottle: &Path) -> PathBuf {
    bottle.join("drive_c").join("Program Files (x86)")
}

/// Devuelve la carpeta Steam existente (Steam/steam) o la que se debe crear.
fn steam_dir(program_files: &Path) -> PathBuf {
    for name in ["Steam", "steam"] {
        let candidate = program_files.join(name);
        if fs::symlink_metadata(&candidate).is_ok() {
            return candidate;
        }
    }
    program_files.join("Steam")
}

/// Juegos instalados con su ruta real, sin duplicados por nombre.
fn installed_games(cfg: &Config) -> Vec<(String, PathBuf, u32)> {
    let mut out: Vec<(String, PathBuf, u32)> = Vec::new();
    for def in crate::detect::GAMES {
        let Some(game) = cfg.games.get(def.id) else {
            continue;
        };
        if !game.installed {
            continue;
        }
        let Some(path) = &game.path else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty()
            || out
                .iter()
                .any(|(existing, _, _)| existing == &name)
        {
            continue;
        }
        out.push((name, path.clone(), def.app_id));
    }
    out
}

/// Escribe los metadatos que SteamHelper.dll (SimHub) usa para detectar los
/// juegos: `libraryfolders.vdf` y un `appmanifest_<id>.acf` por juego.
///
/// Sin estos ficheros SimHub no encuentra la ruta de instalación y muestra los
/// juegos como no configurados, aunque existan los enlaces en `steamapps/common`.
fn write_steam_metadata(steam: &Path, games: &[(String, PathBuf, u32)]) -> std::io::Result<()> {
    let steamapps = steam.join("steamapps");
    fs::create_dir_all(&steamapps)?;

    let steam_name = steam
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "Steam".to_string());
    let win_path = format!("C:\\\\Program Files (x86)\\\\{steam_name}");

    let mut vdf = String::from("\"libraryfolders\"\n{\n\t\"0\"\n\t{\n");
    vdf.push_str(&format!("\t\t\"path\"\t\t\"{win_path}\"\n"));
    vdf.push_str("\t\t\"label\"\t\t\"\"\n");
    vdf.push_str("\t\t\"contentid\"\t\t\"0\"\n");
    vdf.push_str("\t\t\"totalsize\"\t\t\"0\"\n");
    vdf.push_str("\t\t\"update_clean_bytes_tally\"\t\t\"0\"\n");
    vdf.push_str("\t\t\"time_last_update_verified\"\t\t\"0\"\n");
    vdf.push_str("\t\t\"apps\"\n\t\t{\n");
    for (_, _, app_id) in games {
        vdf.push_str(&format!("\t\t\t\"{app_id}\"\t\t\"0\"\n"));
    }
    vdf.push_str("\t\t}\n\t}\n}\n");

    let vdf_path = steamapps.join("libraryfolders.vdf");
    if let Ok(meta) = fs::symlink_metadata(&vdf_path) {
        if meta.file_type().is_symlink() {
            let _ = fs::remove_file(&vdf_path);
        }
    }
    if fs::read_to_string(&vdf_path).ok().as_deref() != Some(vdf.as_str()) {
        fs::write(&vdf_path, vdf)?;
    }

    for (name, path, app_id) in games {
        let manifest = steamapps.join(format!("appmanifest_{app_id}.acf"));
        let real = path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|library| {
                library
                    .join("steamapps")
                    .join(format!("appmanifest_{app_id}.acf"))
            });

        if let Some(real) = real.filter(|manifest| manifest.is_file()) {
            match fs::read_link(&manifest) {
                Ok(current) if current == real => continue,
                Ok(_) => {
                    let _ = fs::remove_file(&manifest);
                }
                Err(_) => {
                    if manifest.exists() {
                        // Un ACF real: no se toca.
                        continue;
                    }
                }
            }
            let _ = symlink(real, &manifest);
        } else {
            if let Ok(meta) = fs::symlink_metadata(&manifest) {
                if meta.file_type().is_symlink() {
                    let _ = fs::remove_file(&manifest);
                } else {
                    continue;
                }
            }
            let acf = format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"{app_id}\"\n\t\"name\"\t\t\"{name}\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"{name}\"\n}}\n"
            );
            fs::write(&manifest, acf)?;
        }
    }

    Ok(())
}

/// Crea (o corrige) los enlaces de Steam dentro de la botella.
pub fn ensure(cfg: &Config) -> Report {
    let mut report = Report::default();
    let Some(bottle) = cfg.apps.bottle.as_deref() else {
        return report;
    };
    if !bottle.is_dir() {
        report
            .errors
            .push(format!("no existe la botella {}", bottle.display()));
        return report;
    }

    let games = installed_games(cfg);
    if games.is_empty() {
        return report;
    }

    let pf = program_files(bottle);
    if let Err(error) = fs::create_dir_all(&pf) {
        report
            .errors
            .push(format!("no se pudo crear {}: {error}", pf.display()));
        return report;
    }

    let steam = steam_dir(&pf);

    // Un enlace manual (por ejemplo steam -> ~/.local/share/Steam) se guarda
    // y se sustituye por una carpeta gestionada con un enlace por juego.
    if let Ok(meta) = fs::symlink_metadata(&steam) {
        if meta.file_type().is_symlink() {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_secs())
                .unwrap_or(0);
            let backup = pf.join(format!("{}.enlace-{stamp}", steam_name(&steam)));
            match fs::rename(&steam, &backup) {
                Ok(()) => {
                    report.migrated = Some(backup);
                    if let Err(error) = fs::create_dir_all(&steam) {
                        report.errors.push(format!(
                            "no se pudo crear {}: {error}",
                            steam.display()
                        ));
                        return report;
                    }
                }
                Err(error) => {
                    report.errors.push(format!(
                        "no se pudo migrar {}: {error}",
                        steam.display()
                    ));
                    return report;
                }
            }
        }
    }

    let common = steam.join("steamapps").join("common");
    if let Err(error) = fs::create_dir_all(&common) {
        report
            .errors
            .push(format!("no se pudo crear {}: {error}", common.display()));
        return report;
    }

    if let Err(error) = write_steam_metadata(&steam, &games) {
        report
            .errors
            .push(format!("no se pudieron escribir los metadatos de Steam: {error}"));
    } else {
        report.metadata = true;
    }

    for (name, real, _) in &games {
        let target = common.join(name);
        match fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let current = fs::read_link(&target).unwrap_or_default();
                if current == *real {
                    continue;
                }
                if fs::remove_file(&target).is_err() {
                    report
                        .errors
                        .push(format!("no se pudo reemplazar {}", target.display()));
                    continue;
                }
                match symlink(real, &target) {
                    Ok(()) => report.created.push(target),
                    Err(error) => report
                        .errors
                        .push(format!("{}: {error}", target.display())),
                }
            }
            Ok(_) => {
                report
                    .skipped
                    .push(format!("{} ya existe y no es un enlace", target.display()));
            }
            Err(_) => match symlink(real, &target) {
                Ok(()) => report.created.push(target),
                Err(error) => report
                    .errors
                    .push(format!("{}: {error}", target.display())),
            },
        }
    }

    report
}

fn steam_name(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "Steam".to_string())
}

/// Comprueba los enlaces sin crear nada (para el doctor).
pub fn check(cfg: &Config) -> (usize, usize, Option<PathBuf>) {
    let Some(bottle) = cfg.apps.bottle.as_deref() else {
        return (0, 0, None);
    };
    let games = installed_games(cfg);
    if games.is_empty() {
        return (0, 0, None);
    }
    let steam = steam_dir(&program_files(bottle));
    let manual = fs::symlink_metadata(&steam)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
        .then_some(steam.clone());
    let mut ok = 0usize;
    let mut missing = 0usize;
    for (name, real, _) in &games {
        let target = steam.join("steamapps").join("common").join(name);
        match fs::read_link(&target) {
            Ok(current) if current == *real => ok += 1,
            _ => missing += 1,
        }
    }
    (ok, missing, manual)
}

/// Resumen legible para logs.
pub fn log_report(report: &Report) {
    if let Some(path) = &report.migrated {
        crate::logs::info(
            "bottle-links",
            format!("enlace manual guardado en {}", path.display()),
        );
    }
    for path in &report.created {
        crate::logs::info("bottle-links", format!("creado {}", path.display()));
    }
    if report.metadata {
        crate::logs::info("bottle-links", "metadatos de Steam actualizados");
    }
    for item in &report.skipped {
        crate::logs::warn("bottle-links", item);
    }
    for item in &report.errors {
        crate::logs::error("bottle-links", item);
    }
}

/// Utilidad para mostrar la ruta de Steam de la botella en mensajes.
pub fn steam_hint(cfg: &Config) -> Option<PathBuf> {
    let bottle = cfg.apps.bottle.as_deref()?;
    Some(steam_dir(&program_files(bottle)))
}

/// Expande una ruta de Steam tal como la ve el juego (para depuracion).
pub fn windows_game_path(name: &str) -> String {
    format!(
        "C:\\Program Files (x86)\\Steam\\SteamApps\\common\\{}",
        name
    )
}

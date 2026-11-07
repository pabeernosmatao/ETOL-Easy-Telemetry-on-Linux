//! Deteccion de bibliotecas de Steam, manifiestos de juego y version de Proton.

use crate::util;
use crate::vdf::{self, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub app_id: u32,
    pub install_dir: PathBuf,
    pub library: PathBuf,
}

/// Raices de Steam conocidas, sin duplicados.
pub fn roots() -> Vec<PathBuf> {
    let home = util::home();
    let candidates = [
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ];
    let mut out: Vec<PathBuf> = Vec::new();
    for candidate in candidates {
        if !candidate.is_dir() {
            continue;
        }
        push_unique(&mut out, candidate);
    }
    out
}

/// Todas las bibliotecas: la raiz de Steam y las de `libraryfolders.vdf`.
pub fn libraries() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    for root in roots() {
        push_unique(&mut out, root.clone());

        let vdf_path = root.join("steamapps/libraryfolders.vdf");
        let Some(value) = vdf::parse_file(&vdf_path) else {
            continue;
        };
        let Some(folders) = value.get("libraryfolders").and_then(Value::as_map) else {
            continue;
        };
        for entry in folders.values() {
            let path = entry
                .get_str("path")
                .map(PathBuf::from)
                .or_else(|| entry.as_str().map(PathBuf::from));
            if let Some(path) = path {
                if path.is_dir() {
                    push_unique(&mut out, path);
                }
            }
        }
    }

    out
}

/// Manifiestos instalados en todas las bibliotecas.
pub fn apps() -> Vec<AppEntry> {
    let mut out = Vec::new();

    for library in libraries() {
        let steamapps = library.join("steamapps");
        let Ok(entries) = std::fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("appmanifest_") || !file_name.ends_with(".acf") {
                continue;
            }
            let Some(value) = vdf::parse_file(&entry.path()) else {
                continue;
            };
            let Some(state) = value.get("AppState") else {
                continue;
            };
            let Some(app_id) = state.get_str("appid").and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let Some(install_dir_name) = state.get_str("installdir") else {
                continue;
            };
            out.push(AppEntry {
                app_id,
                install_dir: steamapps.join("common").join(install_dir_name),
                library: library.clone(),
            });
        }
    }

    out
}

/// Prefijo de Proton del juego, buscando en su biblioteca y en el resto.
pub fn compat_prefix(app_id: u32, preferred_library: &Path) -> Option<PathBuf> {
    let mut candidates = vec![preferred_library.to_path_buf()];
    candidates.extend(libraries());

    for library in candidates {
        let prefix = library
            .join("steamapps")
            .join("compatdata")
            .join(app_id.to_string())
            .join("pfx");
        if prefix.is_dir() {
            return Some(prefix);
        }
    }
    None
}

/// Nombre de la herramienta de compatibilidad usada por el juego.
///
/// Orden de prioridad:
///   1. `CompatToolMapping` del juego en `config/config.vdf` (nombre real del
///      runner elegido en Steam)
///   2. `compatdata/<appid>/config_info` (puede ser una version, no un nombre)
///   3. mapping global (`0`) de Steam
///
/// En Steam 2026 el `config_info` de algunos juegos contiene algo como
/// `11.0-100` (version) en vez del nombre del runner; si se usa primero,
/// `resolve_proton` no encuentra nada y ETOL acaba usando el wine de Bottles.
pub fn compat_tool(app_id: u32, preferred_library: &Path) -> Option<String> {
    if let Some(tool) = compat_tool_from_mapping(app_id) {
        return Some(tool);
    }

    let mut candidates = vec![preferred_library.to_path_buf()];
    candidates.extend(libraries());

    for library in candidates {
        let info = library
            .join("steamapps")
            .join("compatdata")
            .join(app_id.to_string())
            .join("config_info");
        if let Some(tool) = util::first_line(&info) {
            return Some(tool);
        }
    }

    compat_tool_from_mapping(0)
}

/// Directorio de Proton que usa el juego, combinando todas las fuentes.
///
/// `config_info` guarda, ademas de la version, las rutas reales del runner que
/// se ejecuto (`.../Proton - Experimental/files/share/fonts/`). Es la pista mas
/// fiable cuando `CompatToolMapping` todavia no tiene la entrada del juego.
pub fn proton_for(app_id: u32, preferred_library: &Path) -> Option<PathBuf> {
    if let Some(tool) = compat_tool_from_mapping(app_id) {
        if let Some(dir) = resolve_proton(&tool) {
            return Some(dir);
        }
    }

    let mut candidates = vec![preferred_library.to_path_buf()];
    candidates.extend(libraries());
    for library in candidates {
        let info = library
            .join("steamapps")
            .join("compatdata")
            .join(app_id.to_string())
            .join("config_info");
        let Ok(text) = std::fs::read_to_string(&info) else {
            continue;
        };
        if let Some(dir) = proton_from_runner_paths(&text) {
            return Some(dir);
        }
        if let Some(tool) = text.lines().next().map(str::trim).filter(|t| !t.is_empty()) {
            if let Some(dir) = resolve_proton(tool) {
                return Some(dir);
            }
        }
    }

    compat_tool_from_mapping(0).and_then(|tool| resolve_proton(&tool))
}

/// Extrae el directorio de Proton de las rutas de `config_info`.
fn proton_from_runner_paths(text: &str) -> Option<PathBuf> {
    for line in text.lines() {
        let line = line.trim();
        let Some(index) = line.find("/files/") else {
            continue;
        };
        let path = PathBuf::from(&line[..index]);
        if path.join("files/bin/wine").is_file() || path.join("proton").is_file() {
            return Some(path);
        }
    }
    None
}

fn compat_tool_from_mapping(app_id: u32) -> Option<String> {
    for root in roots() {
        let Some(value) = vdf::parse_file(&root.join("config/config.vdf")) else {
            continue;
        };
        let Some(store) = value.get("InstallConfigStore") else {
            continue;
        };
        let Some(mapping) = store
            .get("Software")
            .and_then(|v| v.get("Valve"))
            .and_then(|v| v.get("Steam"))
            .and_then(|v| v.get("CompatToolMapping"))
        else {
            continue;
        };
        if let Some(name) = mapping
            .get(&app_id.to_string())
            .and_then(|entry| entry.get_str("name"))
        {
            return Some(name.to_string());
        }
    }
    None
}

/// Resuelve el directorio de Proton a partir del nombre de herramienta.
///
/// Steam guarda en `config_info` el nombre exacto (por ejemplo `GE-Proton11-7`),
/// que puede no coincidir con el nombre de la carpeta. Se compara tambien con
/// el fichero `version` de cada Proton, que es la fuente fiable.
pub fn resolve_proton(tool: &str) -> Option<PathBuf> {
    let tool = tool.trim();
    if tool.is_empty() {
        return None;
    }

    let mut best: Option<(u8, usize, PathBuf)> = None;

    for dir in proton_dirs() {
        if !(dir.join("files/bin/wine").exists() || dir.join("proton").exists()) {
            continue;
        }

        let mut score: u8 = 0;
        let dir_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if dir_name == tool {
            score = score.max(3);
        } else if dir_name.contains(tool) {
            score = score.max(2);
        }

        let version = util::first_line(&dir.join("version")).unwrap_or_default();
        if version.split_whitespace().any(|token| token == tool) {
            score = score.max(4);
        } else if version.contains(tool) {
            score = score.max(3);
        } else if score == 0 {
            // Steam a veces deja en config_info una version ("11.0-100") en
            // vez del nombre del runner. Se compara el prefijo numerico con
            // el fichero version (Proton Experimental: experimental-11.0-...).
            if let Some(prefix) = tool.split('-').next() {
                let numeric = !prefix.is_empty()
                    && prefix
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '.');
                if numeric && version.contains(prefix) {
                    score = 1;
                }
            }
        }

        // Steam a veces anota un nombre de build (por ejemplo `CachyOS-11.0-100`)
        // que no aparece ni en el nombre de la carpeta ni en `version`. En ese
        // caso se compara por tokens ("cachyos", "11", "0"...).
        let overlap = if score == 0 {
            token_overlap(tool, &dir_name).max(token_overlap(tool, &version))
        } else {
            0
        };

        if score == 0 && overlap < 3 {
            continue;
        }

        let better = match &best {
            Some((best_score, best_overlap, _)) => {
                score > *best_score || (score == *best_score && overlap > *best_overlap)
            }
            None => true,
        };
        if better {
            best = Some((score, overlap, dir));
        }
    }

    best.map(|(_, _, dir)| dir)
}

fn token_overlap(left: &str, right: &str) -> usize {
    let left = tokens(left);
    let right = tokens(right);
    left.intersection(&right).count()
}

fn tokens(text: &str) -> std::collections::BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn proton_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    for library in libraries() {
        push_subdirs(&mut out, &library.join("steamapps/common"));
        push_subdirs(&mut out, &library.join("compatibilitytools.d"));
    }

    push_subdirs(
        &mut out,
        &util::home().join(".steam/root/compatibilitytools.d"),
    );
    push_subdirs(&mut out, Path::new("/usr/share/steam/compatibilitytools.d"));

    out
}

fn push_subdirs(out: &mut Vec<PathBuf>, base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            push_unique(out, entry.path());
        }
    }
}

fn push_unique(out: &mut Vec<PathBuf>, path: PathBuf) {
    let canonical = std::fs::canonicalize(&path).unwrap_or(path);
    if !out.contains(&canonical) {
        out.push(canonical);
    }
}



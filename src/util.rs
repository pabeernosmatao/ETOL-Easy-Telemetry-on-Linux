//! Utilidades comunes: rutas, colores de terminal, busqueda de ficheros y
//! escapado para shell.

use std::collections::VecDeque;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Directorios que nunca merece la pena recorrer al buscar un ejecutable.
const SKIP_DIRS: &[&str] = &[
    "windows",
    "dosdevices",
    "system32",
    "syswow64",
    "winsxs",
    "$recycle.bin",
    "system volume information",
    "node_modules",
    "__pycache__",
    "temp",
    "tmp",
];

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Expande un `~` inicial. El resto de la ruta se deja igual.
pub fn expand(raw: &str) -> PathBuf {
    if raw == "~" {
        return home();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home().join(rest);
    }
    PathBuf::from(raw)
}

pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Busca un fichero por nombre dentro de un arbol, con profundidad limitada.
/// Devuelve la primera coincidencia (sin orden garantizado).
pub fn find_file(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let file_name = entry.file_name().to_string_lossy().to_string();

            if file_type.is_file() && file_name.eq_ignore_ascii_case(name) {
                return Some(path);
            }
            if file_type.is_dir() && depth < max_depth {
                let skip = SKIP_DIRS
                    .iter()
                    .any(|s| file_name.eq_ignore_ascii_case(s));
                if !skip {
                    queue.push_back((path, depth + 1));
                }
            }
        }
    }
    None
}

/// Escapa una cadena para asignarla entre comillas simples en bash.
pub fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Fecha y hora actuales en UTC, sin dependencias externas.
pub fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Algoritmo de Howard Hinnant: dias desde 1970-01-01 -> (año, mes, dia).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Primera linea no vacia de un fichero, recortada.
pub fn first_line(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// Salida por terminal
// ---------------------------------------------------------------------------

fn colors_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn paint(text: &str, code: &str) -> String {
    if colors_enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn green(text: &str) -> String {
    paint(text, "32")
}
pub fn red(text: &str) -> String {
    paint(text, "31")
}
pub fn yellow(text: &str) -> String {
    paint(text, "33")
}
pub fn cyan(text: &str) -> String {
    paint(text, "36")
}
pub fn dim(text: &str) -> String {
    paint(text, "2")
}
pub fn bold(text: &str) -> String {
    paint(text, "1")
}

pub fn section(title: &str) {
    println!();
    println!("{}", bold(title));
}

/// Imprime una etiqueta y un valor alineados.
pub fn field(label: &str, value: &str) {
    println!("  {:<20} {}", label, value);
}

/// Imprime una etiqueta con el valor en verde si existe, rojo si falta.
pub fn field_opt(label: &str, value: Option<&Path>) {
    match value {
        Some(p) => field(label, &p.display().to_string()),
        None => field(label, &red("no detectado")),
    }
}

pub fn field_opt_str(label: &str, value: Option<&str>) {
    match value {
        Some(v) => field(label, v),
        None => field(label, &red("no detectado")),
    }
}

/// Igual que `field`, pero con sangria extra (para subcampos).
pub fn subfield(label: &str, value: &str) {
    println!("    {:<16} {}", label, value);
}

pub fn subfield_opt(label: &str, value: Option<&Path>) {
    match value {
        Some(p) => subfield(label, &p.display().to_string()),
        None => subfield(label, &red("no detectado")),
    }
}

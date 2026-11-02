//! Logs de la aplicacion y del puente.
//!
//! Se guardan en `<raiz>/logs/` (en el AppImage, `~/.config/etol/logs`):
//! - `etol-AAAA-MM-DD.log`: eventos de la app y de los lanzamientos.
//! - `run-AAAA-MM-DD/`: copia de los logs del puente y de SimHub/CrewChief.
//!
//! La captura se puede desactivar y la retencion se limita a 1, 3, 5 o 7 dias.

use crate::config::{self, Config};
use crate::util;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

static ENABLED: AtomicBool = AtomicBool::new(true);
static RETENTION_DAYS: AtomicU32 = AtomicU32::new(7);

pub fn dir() -> PathBuf {
    config::root().join("logs")
}

pub fn file_for_today() -> PathBuf {
    dir().join(format!("etol-{}.log", today()))
}

fn today() -> String {
    util::now_utc().chars().take(10).collect()
}

fn now() -> String {
    util::now_utc()
}

/// Carga la configuracion, crea la carpeta y limpia logs antiguos.
pub fn init() {
    let cfg = Config::load().unwrap_or_default();
    ENABLED.store(cfg.logs.enabled, Ordering::Relaxed);
    RETENTION_DAYS.store(cfg.logs.retention_days.clamp(1, 7), Ordering::Relaxed);
    if cfg.logs.enabled {
        let _ = fs::create_dir_all(dir());
        prune();
    }
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(value: bool) {
    ENABLED.store(value, Ordering::Relaxed);
    if value {
        let _ = fs::create_dir_all(dir());
        prune();
        info("logs", "captura de logs activada");
    }
}

pub fn set_retention(days: u32) {
    RETENTION_DAYS.store(days.clamp(1, 7), Ordering::Relaxed);
    prune();
}

pub fn line(level: &str, module: &str, message: impl AsRef<str>) {
    if !enabled() {
        return;
    }
    let dir = dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(format!("etol-{}.log", today()));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = writeln!(
        file,
        "{} {:<5} [{}] {}",
        now(),
        level,
        module,
        message.as_ref().replace('\n', " | ")
    );
}

pub fn info(module: &str, message: impl AsRef<str>) {
    line("INFO", module, message);
}

pub fn warn(module: &str, message: impl AsRef<str>) {
    line("AVISO", module, message);
}

pub fn error(module: &str, message: impl AsRef<str>) {
    line("ERROR", module, message);
}

/// Copia los logs del puente (y de SimHub/CrewChief) a `logs/run-AAAA-MM-DD/`.
pub fn capture_run(run_base: &Path) {
    if !enabled() || !run_base.is_dir() {
        return;
    }
    let dest = dir().join(format!("run-{}", today()));
    if fs::create_dir_all(&dest).is_err() {
        return;
    }
    let mut copied = 0usize;
    copy_logs(run_base, run_base, &dest, 0, &mut copied);
    if copied > 0 {
        info(
            "logs",
            format!(
                "capturados {copied} ficheros de {} en {}",
                run_base.display(),
                dest.display()
            ),
        );
    }
}

fn copy_logs(src_root: &Path, dir: &Path, dest_root: &Path, depth: usize, copied: &mut usize) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ty) = entry.file_type() else {
            continue;
        };
        if ty.is_dir() {
            copy_logs(src_root, &path, dest_root, depth + 1, copied);
            continue;
        }
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !matches!(ext, "log" | "txt") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() > 8 * 1024 * 1024 {
            continue;
        }
        let Ok(relative) = path.strip_prefix(src_root) else {
            continue;
        };
        let target = dest_root.join(relative);
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::copy(&path, &target).is_ok() {
            *copied += 1;
        }
    }
}

/// Borra logs mas antiguos que `retention_days`.
pub fn prune() {
    let days = RETENTION_DAYS.load(Ordering::Relaxed).clamp(1, 7) as u64;
    let max_age = Duration::from_secs(days * 86_400);
    let Ok(entries) = fs::read_dir(dir()) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age <= max_age {
            continue;
        }
        if meta.is_dir() {
            let _ = fs::remove_dir_all(&path);
        } else if meta.is_file() {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Resumen para el doctor y la interfaz.
pub fn summary() -> String {
    if !enabled() {
        return format!("desactivados ({} no se actualiza)", dir().display());
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    if let Ok(entries) = fs::read_dir(dir()) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    files += 1;
                    bytes += meta.len();
                }
            }
        }
    }
    format!(
        "{} · {} ficheros · {} KB · retencion {} dias",
        dir().display(),
        files,
        bytes / 1024,
        RETENTION_DAYS.load(Ordering::Relaxed)
    )
}

//! Instala el payload incluido en el AppImage y expone su estado a la GUI.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::OnceLock;
use etol::config::Config;
use etol::detect;
use etol::messages;
use etol::payload;

#[derive(Debug, Clone, Serialize)]
pub struct PayloadInfo {
    pub ok: bool,
    pub installed: bool,
    pub dir: Option<String>,
    pub source: Option<String>,
    pub searched: Vec<String>,
    pub detail: String,
    pub fix: Option<String>,
}

static INFO: OnceLock<PayloadInfo> = OnceLock::new();

pub fn info() -> PayloadInfo {
    INFO.get().cloned().unwrap_or_else(|| {
        let report = payload::report();
        let searched: Vec<PathBuf> = report.searched.iter().map(PathBuf::from).collect();
        PayloadInfo {
            ok: report.ok,
            installed: report.installed,
            dir: report.dir.clone(),
            source: report.source.clone(),
            searched: report.searched.clone(),
            fix: (!report.ok).then(|| messages::payload_fix(&searched)),
            detail: report.detail,
        }
    })
}

fn set(info: PayloadInfo) -> PayloadInfo {
    let _ = INFO.set(info.clone());
    info
}

fn searched_strings(searched: &[PathBuf]) -> Vec<String> {
    searched
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

/// Resuelve, verifica e instala el payload. Nunca crea prefijos ni toca
/// Bottles: solo copia scripts a la carpeta de datos de la aplicacion.
pub fn ensure() -> PayloadInfo {
    let search = payload::find_source();
    let searched = searched_strings(&search.searched);

    // Opt-out explicito: el usuario indica su propia carpeta de scripts.
    if let Some(explicit) = std::env::var_os("ETOL_APP_DIR") {
        let dir = PathBuf::from(explicit);
        if detect::is_app_dir(&dir) {
            return set(PayloadInfo {
                ok: true,
                installed: false,
                dir: Some(dir.display().to_string()),
                source: Some("ETOL_APP_DIR".to_string()),
                searched,
                detail: format!("usando la carpeta indicada en ETOL_APP_DIR: {}", dir.display()),
                fix: None,
            });
        }
    }

    let dest = payload::dest();

    // Una carpeta propia configurada a mano se respeta.
    if let Ok(cfg) = Config::load() {
        if let Some(dir) = cfg.bridge.app_dir {
            if dir != dest && detect::is_app_dir(&dir) && !detect::is_legacy_app_dir(&dir) {
                return set(PayloadInfo {
                    ok: true,
                    installed: false,
                    dir: Some(dir.display().to_string()),
                    source: Some("config".to_string()),
                    searched,
                    detail: format!("usando la carpeta de scripts configurada: {}", dir.display()),
                    fix: None,
                });
            }
        }
    }

    let Some(source) = search.found else {
        if payload::is_complete(&dest) {
            let id = payload::read_id(&dest).unwrap_or_else(|| "sin id".to_string());
            return set(PayloadInfo {
                ok: true,
                installed: true,
                dir: Some(dest.display().to_string()),
                source: None,
                searched,
                detail: format!("payload instalado en {} (id {})", dest.display(), short_id(&id)),
                fix: None,
            });
        }

        return set(PayloadInfo {
            ok: false,
            installed: false,
            dir: None,
            source: None,
            searched: searched.clone(),
            detail: messages::payload_missing(&search.searched),
            fix: Some(messages::payload_fix(&search.searched)),
        });
    };

    match payload::install(&source, &dest) {
        Ok(copied) => set(PayloadInfo {
            ok: true,
            installed: true,
            dir: Some(dest.display().to_string()),
            source: Some(source.display().to_string()),
            searched,
            detail: if copied {
                format!("payload instalado en {}", dest.display())
            } else {
                format!("payload ya actualizado en {}", dest.display())
            },
            fix: None,
        }),
        Err(error) => {
            let searched_paths: Vec<PathBuf> = search.searched.to_vec();
            set(PayloadInfo {
                ok: false,
                installed: payload::is_complete(&dest),
                dir: None,
                source: Some(source.display().to_string()),
                searched,
                detail: format!("no se pudo instalar el payload: {error:#}"),
                fix: Some(messages::payload_fix(&searched_paths)),
            })
        }
    }
}

fn short_id(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

//! Localizacion, verificacion e instalacion del payload de scripts.
//!
//! El AppImage es de solo lectura. Los scripts viajan como recurso y se copian
//! a `~/.config/etol/payload` en el primer arranque. Este modulo es el
//! unico sitio donde se decide de donde salen y como se validan.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Archivos que deben existir para considerar valida una carpeta de scripts.
pub const KEY_FILES: &[&str] = &[
    "arrancar_todo_lmu.sh",
    "activar_puente_lmu.sh",
    "desactivar_puente_lmu.sh",
    "arrancar_todo_assetto.sh",
    "activar_puente_assetto.sh",
    "desactivar_puente_assetto.sh",
    "arrancar_todo_rf2.sh",
    "activar_puente_rf2.sh",
    "desactivar_puente_rf2.sh",
    "arrancar_todo_dirt2.sh",
    "activar_puente_dirt2.sh",
    "desactivar_puente_dirt2.sh",
    "arrancar_todo_ets2.sh",
    "activar_puente_ets2.sh",
    "desactivar_puente_ets2.sh",
    "arrancar_todo_beamng.sh",
    "activar_puente_beamng.sh",
    "desactivar_puente_beamng.sh",
    "configurar_juego.sh",
    "bridge/setup_hf8.sh",
    "bridge/lmu_shm_mirror.exe",
    "bridge/assetto_shm_mirror.exe",
    "bridge/rf2_shm_mirror.exe",
    "bridge/dirt2_udp_relay.py",
    "bridge/configure_apps_dirt2.py",
    "bridge/dirt2_process_stub.exe",
    "bridge/ets2_shm_mirror.exe",
    "bridge/ets2_process_stub.exe",
    "bridge/libscs-telemetry.so",
    "bridge/configure_apps_ets2.py",
    "bridge/beamng_process_stub.exe",
    "bridge/configure_apps_beamng.py",
];

/// Scripts de sistema que la interfaz puede inspeccionar y ejecutar.
pub const SYSTEM_SCRIPTS: &[(&str, &str)] = &[
    ("bottles-install", "01-instalar-bottles-flatpak.sh"),
    ("bottles-user", "02-bottles-flatpak-a-usuario.sh"),
    ("prefix", "03-preparar-prefijo-simhub.sh"),
];

pub fn is_complete(dir: &Path) -> bool {
    dir.is_dir() && KEY_FILES.iter().all(|name| dir.join(name).is_file())
}

/// Ruta del script de sistema pedido, tal y como se ejecutaria.
///
/// Prioridad: copia del payload que usa la app, `bridge.app_dir` de la
/// configuracion y, por ultimo, el arbol de fuentes (util para desarrollo).
pub fn system_script(key: &str) -> Option<PathBuf> {
    let file = SYSTEM_SCRIPTS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, file)| *file)?;

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(dir) = crate::detect::app_dir_override() {
        roots.push(dir.clone());
    }
    if let Ok(cfg) = crate::config::Config::load() {
        if let Some(dir) = cfg.app_dir() {
            roots.push(dir.to_path_buf());
        }
    }
    roots.push(dest());
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"));

    for root in roots {
        let candidate = root.join("sistema").join(file);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn read_id(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join(".payload-id")).ok()?;
    let id = text.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Indica si el proceso corre desde un AppImage o desde un AppDir extraido.
pub fn running_bundled() -> bool {
    if std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some() {
        return true;
    }

    let Some(exe) = std::env::current_exe().ok() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    // <AppDir>/usr/bin/etol-gui
    if dir.file_name().and_then(|name| name.to_str()) == Some("bin") {
        if let Some(usr) = dir.parent() {
            if usr.file_name().and_then(|name| name.to_str()) == Some("usr") {
                if let Some(root) = usr.parent() {
                    return root.join("usr/lib/ETOL").is_dir();
                }
            }
        }
    }
    false
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

/// Raices donde puede vivir el directorio de recursos `payload`.
///
/// Orden: `$APPDIR/usr/lib/ETOL`, `<exe_dir>/../lib/ETOL`,
/// `<exe_dir>` y `usr/lib/ETOL` relativo al AppDir.
pub fn resource_roots() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    if let Some(appdir) = std::env::var_os("APPDIR") {
        push_unique(&mut out, PathBuf::from(appdir).join("usr/lib/ETOL"));
    }

    if let Some(dir) = exe_dir() {
        push_unique(&mut out, dir.join("../lib/ETOL"));
        push_unique(&mut out, dir.clone());
    }

    out
}

fn push_unique(out: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !out.contains(&candidate) {
        out.push(candidate);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Search {
    pub found: Option<PathBuf>,
    pub searched: Vec<PathBuf>,
}

/// Busca la carpeta de scripts incluida en el AppImage.
pub fn find_source() -> Search {
    let mut searched = Vec::new();

    for root in resource_roots() {
        for candidate in [root.join("payload"), root] {
            if searched.contains(&candidate) {
                continue;
            }
            searched.push(candidate.clone());
            if is_complete(&candidate) {
                return Search {
                    found: Some(candidate),
                    searched,
                };
            }
        }
    }

    Search {
        found: None,
        searched,
    }
}

/// Carpeta donde la aplicacion instala su copia de trabajo.
pub fn dest() -> PathBuf {
    crate::config::root().join("payload")
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub ok: bool,
    pub installed: bool,
    pub dir: Option<String>,
    pub source: Option<String>,
    pub searched: Vec<String>,
    pub detail: String,
}

/// Estado del payload para el doctor y para el arranque de la GUI.
pub fn report() -> Report {
    let search = find_source();
    let dest = dest();
    let installed = is_complete(&dest);

    let searched: Vec<String> = search
        .searched
        .iter()
        .map(|path| path.display().to_string())
        .collect();

    let (ok, detail) = if installed {
        let id = read_id(&dest).unwrap_or_else(|| "sin id".to_string());
        (
            true,
            format!("instalado en {} (id {})", dest.display(), short_id(&id)),
        )
    } else if let Some(src) = &search.found {
        (
            true,
            format!(
                "disponible en {}; se instalara en el proximo arranque",
                src.display()
            ),
        )
    } else {
        (
            false,
            crate::messages::payload_missing(&search.searched),
        )
    };

    Report {
        ok,
        installed,
        dir: if installed {
            Some(dest.display().to_string())
        } else {
            search.found.as_ref().map(|path| path.display().to_string())
        },
        source: search.found.as_ref().map(|path| path.display().to_string()),
        searched,
        detail,
    }
}

fn short_id(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

/// Instala el payload si hay una copia disponible y todavia no existe.
///
/// La usa la CLI cuando corre dentro de un AppImage: asi `etol detect`
/// deja `bridge.app_dir` apuntando a una copia escribible, igual que la GUI.
pub fn ensure_installed() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("ETOL_APP_DIR") {
        let dir = PathBuf::from(explicit);
        if is_complete(&dir) {
            return Some(dir);
        }
    }

    let dest = dest();
    if is_complete(&dest) {
        return Some(dest);
    }

    if let Ok(cfg) = crate::config::Config::load() {
        if let Some(dir) = cfg.bridge.app_dir {
            if dir != dest && is_complete(&dir) && !crate::detect::is_legacy_app_dir(&dir) {
                return Some(dir);
            }
        }
    }

    let source = find_source().found?;
    match install(&source, &dest) {
        Ok(_) => Some(dest),
        Err(error) => {
            eprintln!("aviso: no se pudo instalar el payload: {error:#}");
            None
        }
    }
}

/// Copia el payload incluido a `dest` si hace falta.
///
/// Devuelve `true` cuando ha copiado algo. La verificacion es doble: se
/// compara `.payload-id` despues de copiar y, si no coincide, se conserva el
/// payload anterior y se aborta el intercambio.
pub fn install(src: &Path, dest: &Path) -> Result<bool> {
    let src_id = read_id(src);

    if is_complete(dest) {
        match (&src_id, read_id(dest)) {
            (Some(src_id), Some(dest_id)) if src_id == &dest_id => return Ok(false),
            _ => {}
        }
    }

    let parent = dest
        .parent()
        .context("la carpeta de destino no tiene padre")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("no se pudo crear {}", parent.display()))?;

    let tmp = dest.with_file_name("payload.tmp");
    let old = dest.with_file_name("payload.old");

    if tmp.exists() {
        fs::remove_dir_all(&tmp)
            .with_context(|| format!("no se pudo limpiar {}", tmp.display()))?;
    }

    copy_tree(src, &tmp)?;

    if let Some(id) = &src_id {
        fs::write(tmp.join(".payload-id"), format!("{id}\n"))
            .with_context(|| format!("no se pudo escribir en {}", tmp.display()))?;
    }

    // Verificacion antes de tocar la copia buena.
    if let Some(id) = &src_id {
        if read_id(&tmp).as_deref() != Some(id.as_str()) {
            let _ = fs::remove_dir_all(&tmp);
            bail!(
                "el payload copiado no coincide con el original (id {} esperado)",
                short_id(id)
            );
        }
    } else if !is_complete(&tmp) {
        let _ = fs::remove_dir_all(&tmp);
        bail!("el payload copiado esta incompleto");
    }

    if dest.exists() {
        if old.exists() {
            fs::remove_dir_all(&old)
                .with_context(|| format!("no se pudo limpiar {}", old.display()))?;
        }
        fs::rename(dest, &old)
            .with_context(|| format!("no se pudo apartar {}", dest.display()))?;
    }

    if let Err(error) = fs::rename(&tmp, dest) {
        if old.exists() {
            let _ = fs::rename(&old, dest);
        }
        let _ = fs::remove_dir_all(&tmp);
        return Err(error).with_context(|| format!("no se pudo instalar {}", dest.display()));
    }

    // Verificacion final: si algo se corrompio, se restaura la copia anterior.
    if let Some(id) = &src_id {
        if read_id(dest).as_deref() != Some(id.as_str()) {
            let _ = fs::remove_dir_all(dest);
            if old.exists() {
                let _ = fs::rename(&old, dest);
            }
            bail!(
                "la copia instalada no coincide con el original (id {} esperado); se conserva la anterior",
                short_id(id)
            );
        }
    }

    if old.exists() {
        let _ = fs::remove_dir_all(&old);
    }

    Ok(true)
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("no se pudo crear {}", dst.display()))?;
    fs::set_permissions(dst, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("no se pudieron ajustar los permisos de {}", dst.display()))?;

    for entry in fs::read_dir(src).with_context(|| format!("no se pudo leer {}", src.display()))? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ty = entry.file_type()?;

        if ty.is_dir() {
            copy_tree(&from, &to)?;
        } else if ty.is_file() || ty.is_symlink() {
            fs::copy(&from, &to)
                .with_context(|| format!("no se pudo copiar {}", from.display()))?;
            fs::set_permissions(&to, fs::Permissions::from_mode(file_mode(&from)))
                .with_context(|| format!("no se pudieron ajustar los permisos de {}", to.display()))?;
        } else {
            bail!("tipo de archivo no soportado: {}", from.display());
        }
    }

    Ok(())
}

/// Los scripts y los ejecutables del puente deben poder ejecutarse siempre.
fn file_mode(path: &Path) -> u32 {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("sh" | "py" | "exe") => 0o755,
        _ => 0o644,
    }
}

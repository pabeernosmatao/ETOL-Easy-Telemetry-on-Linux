//! Icono de grupo para las ventanas de SimHub y CrewChief.
//!
//! Las dos apps corren bajo Wine y comparten la clase de ventana
//! `steam_proton`, asi que KDE las agrupa en una sola entrada del panel. Si no
//! hay un `.desktop` que capture esa clase, KDE usa el icono del `.exe` de la
//! ventana que llega primero (CrewChief) y la mitad de SimHub desaparece.
//!
//! Por eso ETOL instala al arrancar el icono fusionado (CrewChief arriba,
//! SimHub abajo) y su entrada `.desktop`. Antes habia que ejecutar
//! `scripts/icono-apps.py` a mano, y en un equipo nuevo el icono se quedaba
//! como el de CrewChief. Los PNG ya vienen dentro del binario: no hace falta
//! Python, PIL ni pefile en el equipo de destino.
//!
//! No modifica ni SimHub ni CrewChief: solo escribe en `~/.local/share`. Si
//! algo falla (permisos, HOME raro), se anota en el log y ETOL sigue igual.

use anyhow::{Context, Result};
use etol::logs;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Nombre del icono y del `.desktop` que lo usa.
const ICON_NAME: &str = "etol-apps";
/// Clase de ventana que comparten las apps de Wine en el prefijo de Bottles.
const WINDOW_CLASS: &str = "steam_proton";

/// PNG del icono en los tamanos que usa el escritorio.
const ICONS: &[(u32, &[u8])] = &[
    (512, include_bytes!("../icono-grupo/512.png")),
    (256, include_bytes!("../icono-grupo/256.png")),
    (128, include_bytes!("../icono-grupo/128.png")),
    (64, include_bytes!("../icono-grupo/64.png")),
    (32, include_bytes!("../icono-grupo/32.png")),
];

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

/// Ruta del AppImage en ejecucion; si no hay, la del propio binario.
fn exec_path() -> String {
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| std::env::current_exe().ok())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "etol-gui".to_string())
}

/// Escribe el fichero solo si cambia. Devuelve si hubo cambios.
fn write_if_changed(path: &Path, data: &[u8]) -> Result<bool> {
    if let Ok(existing) = fs::read(path) {
        if existing == data {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("no se pudo crear {}", parent.display()))?;
    }
    fs::write(path, data).with_context(|| format!("no se pudo escribir {}", path.display()))?;
    Ok(true)
}

fn desktop_entry() -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=SimHub + CrewChief\n\
         Comment=Apps de telemetria que lanza ETOL (icono de grupo)\n\
         Exec={}\n\
         Icon={ICON_NAME}\n\
         StartupWMClass={WINDOW_CLASS}\n\
         NoDisplay=true\n\
         Terminal=false\n\
         Categories=Utility;\n",
        exec_path()
    )
}

/// Pide a KDE que relea los lanzadores. No se espera: el arranque de ETOL no
/// depende de ello y kbuildsycoca puede tardar un par de segundos.
fn refresh_kde_cache() {
    for tool in ["kbuildsycoca6", "kbuildsycoca5"] {
        let ok = Command::new(tool)
            .arg("--noincremental")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok();
        if ok {
            return;
        }
    }
}

fn ensure_inner() -> Result<bool> {
    let home = home().context("no hay HOME")?;
    let mut changed = false;

    for (size, png) in ICONS {
        let path = home.join(format!(
            ".local/share/icons/hicolor/{size}x{size}/apps/{ICON_NAME}.png"
        ));
        changed |= write_if_changed(&path, png)?;
    }

    let desktop = home.join(format!(".local/share/applications/{ICON_NAME}.desktop"));
    changed |= write_if_changed(&desktop, desktop_entry().as_bytes())?;

    if changed {
        refresh_kde_cache();
        logs::info(
            "icono",
            format!("icono de grupo instalado ({})", desktop.display()),
        );
    }
    Ok(changed)
}

/// Instala el icono si falta o cambio. Nunca hace fallar el arranque.
pub fn ensure() {
    match ensure_inner() {
        Ok(true) => {}
        Ok(false) => logs::info("icono", "icono de grupo al dia"),
        Err(error) => logs::warn(
            "icono",
            format!("no se pudo instalar el icono de grupo: {error:#}"),
        ),
    }
}

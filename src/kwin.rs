//! Regla de KWin para poner los dashboards de SimHub en la capa Overlay.
//!
//! KWin 6 permite forzar la capa de una ventana con una regla de ventanas.
//! La capa Overlay (9) queda por encima incluso de un juego a pantalla
//! completa, tambien si el juego usa Wayland nativo.
//!
//! ETOL deja la regla instalada pero desactivada, y solo la activa mientras
//! hay un simulador en marcha. Asi los dashboards no tapan el escritorio
//! cuando no se esta jugando.

use anyhow::{bail, Context, Result};
use std::process::Command;

pub const RULE_ID: &str = "e701a110-0001-4000-8000-000000000001";
const DESCRIPTION: &str = "ETOL: dashboards SimHub sobre el juego";
const TITLE_REGEX: &str = r".*\(WPF Renderer\)$";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// No hay KWin (por ejemplo, otra sesion grafica).
    Unavailable,
    /// Hay KWin pero ETOL aun no ha creado su regla.
    Missing,
    Disabled,
    Enabled,
}

fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

fn find_tool(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| which(name))
}

fn kwrite(group: &str, key: &str, value: &str) -> Result<()> {
    let tool = find_tool(&["kwriteconfig6", "kwriteconfig5"])
        .context("no se encontro kwriteconfig6/kwriteconfig5")?;
    let output = Command::new(tool)
        .args([
            "--file",
            "kwinrulesrc",
            "--group",
            group,
            "--key",
            key,
            value,
        ])
        .output()
        .context("no se pudo ejecutar kwriteconfig")?;
    if !output.status.success() {
        bail!(
            "kwriteconfig fallo: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn kread(group: &str, key: &str) -> Option<String> {
    let tool = find_tool(&["kreadconfig6", "kreadconfig5"])?;
    let output = Command::new(tool)
        .args(["--file", "kwinrulesrc", "--group", group, "--key", key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Hay una sesion KWin viva a la que pedirle la recarga.
pub fn available() -> bool {
    let Some(tool) = find_tool(&["qdbus6", "qdbus"]) else {
        return false;
    };
    Command::new(tool)
        .args(["org.kde.KWin", "/KWin", "supportInformation"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn reconfigure() -> Result<()> {
    let tool = find_tool(&["qdbus6", "qdbus"]).context("no se encontro qdbus6/qdbus")?;
    let output = Command::new(tool)
        .args(["org.kde.KWin", "/KWin", "reconfigure"])
        .output()
        .context("no se pudo recargar KWin")?;
    if !output.status.success() {
        bail!(
            "KWin no recargo la configuracion: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn ensure_order() -> Result<()> {
    let current = kread("General", "Order").unwrap_or_default();
    if current
        .split(',')
        .any(|item| item.trim() == RULE_ID)
    {
        return Ok(());
    }
    let value = if current.trim().is_empty() {
        RULE_ID.to_string()
    } else {
        format!("{current},{RULE_ID}")
    };
    kwrite("General", "Order", &value)
}

/// Crea la regla si falta y asegura sus valores. No toca `enabled`.
pub fn ensure_rule() -> Result<()> {
    if !available() {
        return Ok(());
    }
    kwrite(RULE_ID, "Description", DESCRIPTION)?;
    kwrite(RULE_ID, "layer", "9")?;
    kwrite(RULE_ID, "layerrule", "2")?;
    kwrite(RULE_ID, "title", TITLE_REGEX)?;
    kwrite(RULE_ID, "titlematch", "3")?;
    kwrite(RULE_ID, "wmclass", "steam_proton")?;
    kwrite(RULE_ID, "wmclassmatch", "1")?;
    if kread(RULE_ID, "enabled").is_none() {
        kwrite(RULE_ID, "enabled", "false")?;
    }
    ensure_order()
}

/// Activa o desactiva la regla y recarga KWin.
pub fn set_enabled(enabled: bool) -> Result<()> {
    if !available() {
        bail!("no hay una sesion KWin disponible");
    }
    ensure_rule()?;
    kwrite(RULE_ID, "enabled", if enabled { "true" } else { "false" })?;
    reconfigure()
}

/// Desactiva la regla solo si existe y esta activa (para salir limpio).
pub fn disable_if_exists() {
    if !available() {
        return;
    }
    if status() != Status::Enabled {
        return;
    }
    if let Err(error) = set_enabled(false) {
        eprintln!("kwin: no se pudo desactivar la regla: {error:#}");
    }
}

pub fn status() -> Status {
    if !available() {
        return Status::Unavailable;
    }
    // Una regla sin clave `enabled` esta activa por defecto en KWin.
    let exists = kread(RULE_ID, "Description").is_some() || kread(RULE_ID, "layer").is_some();
    if !exists {
        return Status::Missing;
    }
    match kread(RULE_ID, "enabled").as_deref() {
        Some("false") => Status::Disabled,
        _ => Status::Enabled,
    }
}

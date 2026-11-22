//! Arranque automatico de ETOL con la sesion del escritorio.
//!
//! Se usa el mecanismo estandar XDG: un fichero `etol.desktop` en
//! `$XDG_CONFIG_HOME/autostart` (normalmente `~/.config/autostart`). GNOME,
//! KDE, XFCE y la mayoria de escritorios lo leen al iniciar sesion.
//!
//! El estado no se guarda en `config.toml`: el interruptor refleja si el
//! fichero existe. Asi no hay deriva entre configuracion y realidad si el
//! usuario borra o edita la entrada a mano.
//!
//! El `Exec=` apunta al AppImage real (`APPIMAGE`) y no a
//! `current_exe()`, que dentro de un AppImage es el binario del montaje
//! temporal. Al arrancar con `--autostart`, la interfaz se queda en la
//! bandeja en vez de abrir la ventana.

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::logs;
use crate::util;

/// Nombre del fichero dentro de la carpeta `autostart`.
pub const FILE_NAME: &str = "etol.desktop";

/// Estado que muestra la pestana Opciones.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// Hay un `etol.desktop` en la carpeta de autostart.
    pub enabled: bool,
    /// Ruta completa del fichero.
    pub path: String,
    /// `Exec=` que contiene ahora mismo, si el fichero existe.
    pub exec: Option<String>,
    /// Lanzador que usaria ETOL si se activara ahora.
    pub launcher: Option<String>,
}

/// Carpeta XDG de autostart.
///
/// `ETOL_AUTOSTART_DIR` permite aislarla en pruebas sin tocar la sesion real.
pub fn dir() -> PathBuf {
    if let Some(value) = std::env::var_os("ETOL_AUTOSTART_DIR") {
        return PathBuf::from(value);
    }
    if let Some(value) = std::env::var_os("XDG_CONFIG_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return path.join("autostart");
        }
    }
    util::home().join(".config").join("autostart")
}

/// Ruta del fichero de autostart de ETOL.
pub fn path() -> PathBuf {
    dir().join(FILE_NAME)
}

/// Ejecutable que debe lanzar el escritorio.
///
/// Dentro de un AppImage se prefiere `APPIMAGE`: es la ruta estable que el
/// usuario ejecuto. `current_exe()` seria `/tmp/.mount_.../usr/bin/etol-gui`,
/// que desaparece al cerrar la aplicacion.
fn launcher() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("APPIMAGE") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
    }

    let exe = std::env::current_exe().context("no se pudo resolver el ejecutable de ETOL")?;
    if !exe.is_file() {
        anyhow::bail!("el ejecutable de ETOL no existe: {}", exe.display());
    }
    Ok(exe)
}

/// Escapa una ruta para la clave `Exec` de un `.desktop`.
///
/// La especificacion XDG pide entrecomillar las rutas con espacios y escapar
/// `"`, `` ` ``, `$` y `\` dentro de las comillas. `%` es un codigo de campo,
/// asi que se duplica para que se trate como texto literal.
fn exec_quote(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let needs_quotes = raw
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\'' | '\\' | '$' | '`' | '%'));
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('%', "%%");
    if needs_quotes {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

/// Contenido del fichero para un lanzador concreto.
fn entry_for(launcher: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=ETOL\n\
         Comment=ETOL · Easy Telemetry On Linux\n\
         Exec={} --autostart\n\
         Icon=etol\n\
         Terminal=false\n\
         Categories=Utility;\n\
         StartupWMClass=etol-gui\n\
         X-GNOME-Autostart-enabled=true\n",
        exec_quote(launcher)
    )
}

/// Lee el `Exec=` actual del fichero, si existe.
fn current_exec(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("Exec=").map(|value| value.to_string()))
}

/// Estado actual, sin modificar nada.
pub fn status() -> Status {
    let path = path();
    let enabled = path.is_file();
    Status {
        enabled,
        path: path.display().to_string(),
        exec: if enabled { current_exec(&path) } else { None },
        launcher: launcher().ok().map(|value| value.display().to_string()),
    }
}

/// Escribe la entrada de forma atomica y con permisos legibles.
fn write_entry(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension("desktop.tmp");
    fs::write(&tmp, text)
        .with_context(|| format!("no se pudo escribir {}", tmp.display()))?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644))
        .with_context(|| format!("no se pudieron ajustar los permisos de {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("no se pudo instalar {}", path.display()))?;
    Ok(())
}

/// Activa el arranque automatico, reescribiendo la entrada si ya existia.
pub fn enable() -> Result<Status> {
    let launcher = launcher()?;
    let dir = dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("no se pudo crear {}", dir.display()))?;

    let path = path();
    write_entry(&path, &entry_for(&launcher))?;

    logs::info(
        "autostart",
        format!("activado: {} -> {}", path.display(), launcher.display()),
    );
    Ok(status())
}

/// Extrae el ejecutable de una linea `Exec=` (admite `env VAR=valor ...`).
fn exec_command(exec: &str) -> Option<PathBuf> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;

    for ch in exec.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ch if ch.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    let mut tokens = tokens.into_iter();
    let mut command = tokens.next()?;
    if command == "env" {
        command = tokens.find(|token| !token.contains('='))?;
    }
    Some(PathBuf::from(command))
}

/// Mantiene las entradas antiguas al dia.
///
/// Las versiones anteriores de ETOL (o la integracion del AppImage) podian
/// dejar un `etol.desktop` sin `--autostart` o apuntando a una copia que ya no
/// existe. Se anade el argumento para que el arranque automatico empiece en la
/// bandeja y, si la ruta desaparecio, se reescribe con el lanzador actual.
pub fn upgrade_if_needed() {
    let path = path();
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    if !text.contains("Name=ETOL") {
        return;
    }
    let Some(line) = text.lines().find(|line| line.starts_with("Exec=")) else {
        return;
    };

    let command = exec_command(line.trim_start_matches("Exec="));
    let missing = command
        .as_ref()
        .map(|value| !value.is_absolute() || !value.is_file())
        .unwrap_or(true);
    let has_flag = line.contains("--autostart");

    let updated = if missing {
        let Ok(launcher) = launcher() else {
            return;
        };
        entry_for(&launcher)
    } else if has_flag {
        return;
    } else {
        text.replacen(line, &format!("{line} --autostart"), 1)
    };

    match write_entry(&path, &updated) {
        Ok(()) => logs::info(
            "autostart",
            format!("entrada existente actualizada: {}", path.display()),
        ),
        Err(error) => logs::warn(
            "autostart",
            format!("no se pudo actualizar la entrada: {error:#}"),
        ),
    }
}

/// Desactiva el arranque automatico eliminando la entrada.
pub fn disable() -> Result<Status> {
    let path = path();
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("no se pudo borrar {}", path.display()))?;
        logs::info("autostart", format!("desactivado: {}", path.display()));
    }
    Ok(status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_with_spaces_and_fields() {
        assert_eq!(exec_quote(Path::new("/home/me/ETOL.AppImage")), "/home/me/ETOL.AppImage");
        assert_eq!(
            exec_quote(Path::new("/home/me/Juegos/ETOL 0.5.AppImage")),
            "\"/home/me/Juegos/ETOL 0.5.AppImage\""
        );
        assert_eq!(
            exec_quote(Path::new("/home/me/100% ETOL.AppImage")),
            "\"/home/me/100%% ETOL.AppImage\""
        );
    }

    #[test]
    fn entry_starts_hidden_and_uses_the_launcher() {
        let text = entry_for(Path::new("/home/me/ETOL.AppImage"));
        assert!(text.contains("Exec=/home/me/ETOL.AppImage --autostart"));
        assert!(text.contains("StartupWMClass=etol-gui"));
        assert!(text.contains("X-GNOME-Autostart-enabled=true"));
    }

    #[test]
    fn finds_the_command_after_env() {
        assert_eq!(
            exec_command("env DESKTOPINTEGRATION=1 /home/me/ETOL.AppImage --autostart"),
            Some(PathBuf::from("/home/me/ETOL.AppImage"))
        );
        assert_eq!(
            exec_command("\"/home/me/Juegos/ETOL AppImage\" --autostart"),
            Some(PathBuf::from("/home/me/Juegos/ETOL AppImage"))
        );
    }
}

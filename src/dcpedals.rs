//! DC Simracing pedals (DC2/DC3): deteccion HID y acceso del usuario.
//!
//! El software oficial de calibracion corre en la botella de ETOL y habla HID
//! con el pedal (VID `0444`, PID `20DC`). En Linux el nodo `/dev/hidrawN`
//! suele pertenecer a `root:root` con modo `0600`, asi que Wine no puede
//! abrirlo hasta instalar una regla udev `uaccess`.
//!
//! Este modulo solo lee sysfs y prepara permisos: nunca abre ni escribe en el
//! dispositivo.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Debe ir antes de `73-seat-late.rules`, que ejecuta el builtin `uaccess`.
/// Un fichero `99-...` anade la etiqueta demasiado tarde y no crea la ACL.
pub const RULE_PATH: &str = "/etc/udev/rules.d/70-etol-dcpedals.rules";
pub const VENDOR_ID: u32 = 0x0444;
pub const PRODUCT_ID: u32 = 0x20dc;

#[derive(Debug, Clone, Serialize)]
pub struct DcPedals {
    /// Hay al menos un hidraw con el VID/PID de DC Pedals.
    pub present: bool,
    pub devices: Vec<Device>,
    pub rule_present: bool,
    /// La regla contiene el VID/PID y `uaccess`.
    pub rule_ok: bool,
    pub needs_fix: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Device {
    pub dev: String,
    pub name: Option<String>,
    pub serial: Option<String>,
    pub accessible: bool,
}

pub fn inspect() -> DcPedals {
    let devices = devices();
    let (rule_present, rule_ok) = rule_status();
    let any_blocked = devices.iter().any(|device| !device.accessible);
    let present = !devices.is_empty();
    let needs_fix = !rule_ok || (present && any_blocked);

    let detail = if !present {
        Some("Conecta los pedales por USB para verlos aqui.".to_string())
    } else if rule_ok && any_blocked {
        Some("Regla instalada, pero el dispositivo sigue sin acceso. Desconecta y vuelve a conectar los pedales.".to_string())
    } else if !rule_ok {
        Some("Falta la regla udev: Wine no puede abrir el HID.".to_string())
    } else {
        None
    };

    DcPedals {
        present,
        devices,
        rule_present,
        rule_ok,
        needs_fix,
        detail,
    }
}

/// Instala la regla udev con pkexec. No toca el dispositivo.
pub fn fix() -> Result<String> {
    if !command_exists("pkexec") {
        bail!(
            "no se encontro pkexec. Ejecuta esto en una terminal con sudo:\n\n{}",
            FIX_SCRIPT
        );
    }

    let mut command = Command::new("pkexec");
    command.arg("/bin/sh").arg("-c").arg(FIX_SCRIPT);
    crate::launch::clean_child_env(&mut command);

    let output = command.output().context("no se pudo ejecutar pkexec")?;
    let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&stderr);
    }

    if output.status.success() {
        return Ok(text);
    }

    match output.status.code() {
        Some(126) => bail!("autorizacion cancelada"),
        Some(127) => bail!("pkexec no pudo ejecutar el arreglo"),
        Some(code) => bail!("el arreglo fallo (codigo {code})\n{text}"),
        None => bail!("el arreglo fallo\n{text}"),
    }
}

fn devices() -> Vec<Device> {
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for entry in entries.flatten() {
        let node = entry.file_name();
        let node = node.to_string_lossy();
        let uevent = entry.path().join("device/uevent");
        let Ok(text) = fs::read_to_string(&uevent) else {
            continue;
        };
        if !is_dc_pedals(&text) {
            continue;
        }

        let dev = PathBuf::from("/dev").join(node.as_ref());
        found.push(Device {
            dev: dev.display().to_string(),
            name: value_of(&text, "HID_NAME"),
            serial: value_of(&text, "HID_UNIQ").filter(|value| !value.is_empty()),
            accessible: can_access(&dev),
        });
    }
    found.sort_by(|a, b| a.dev.cmp(&b.dev));
    found
}

fn is_dc_pedals(uevent: &str) -> bool {
    let Some(value) = value_of(uevent, "HID_ID") else {
        return false;
    };
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 {
        return false;
    }
    let vendor = u32::from_str_radix(parts[1], 16).ok();
    let product = u32::from_str_radix(parts[2], 16).ok();
    vendor == Some(VENDOR_ID) && product == Some(PRODUCT_ID)
}

fn value_of(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim().to_string())
    })
}

fn rule_status() -> (bool, bool) {
    match fs::read_to_string(RULE_PATH) {
        Ok(text) => {
            let lower = text.to_lowercase();
            let ok = lower.contains("hidraw")
                && lower.contains("0444")
                && lower.contains("20dc")
                && lower.contains("uaccess");
            (true, ok)
        }
        Err(_) => (false, false),
    }
}

/// `access(2)` respeta permisos de grupo y ACL de `uaccess`.
fn can_access(path: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { libc::access(path.as_ptr(), libc::R_OK | libc::W_OK) == 0 }
}

fn command_exists(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

/// Script fijo, sin datos del usuario interpolados.
const FIX_SCRIPT: &str = r#"set -eu
mkdir -p /etc/udev/rules.d
# Limpia la version antigua (99-), que llegaba tarde al builtin uaccess.
rm -f /etc/udev/rules.d/99-etol-dcpedals.rules
cat > /etc/udev/rules.d/70-etol-dcpedals.rules <<'EOF'
# ETOL: DC Simracing pedals (DC2/DC3) - acceso HID para Wine.
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0444", ATTRS{idProduct}=="20dc", TAG+="uaccess"
EOF
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw
udevadm settle --timeout=5 >/dev/null 2>&1 || true
echo "Regla udev de DC Pedals instalada y aplicada."
echo "Si el acceso sigue denegado, desconecta y vuelve a conectar los pedales."
"#;

//! Fanatec (CSL DD y otras bases): ajustes del menu de tuning.
//!
//! El driver `hid-fanatecff` expone el menu de tuning por sysfs
//! (`/sys/class/ftec_tuning`). Esta ficha solo **lee** esos valores: no abre
//! el HID ni escribe en el volante. El firmware queda fuera de ETOL.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const VENDOR_ID: u32 = 0x0eb7;
const TUNING_DIR: &str = "/sys/class/ftec_tuning";
const DRIVER_DIR: &str = "/sys/bus/hid/drivers/fanatec";
const RULE_PATH: &str = "/etc/udev/rules.d/70-etol-fanatec.rules";
/// Atributos editables; `RESET` y `SLOT` quedan fuera.
const EDITABLE: &str = "SEN FF FEI FOR SPR DPR NDP NFR NIN INT FFS FUL SHO BLI brF ACP DRI advanced_mode";

#[derive(Debug, Clone, Serialize)]
pub struct Fanatec {
    /// Hay una base Fanatec o el driver expone el menu de tuning.
    pub present: bool,
    /// El modulo `hid_fanatec` esta cargado.
    pub driver: bool,
    /// El driver expone `/sys/class/ftec_tuning`.
    pub tuning: bool,
    pub device: Option<String>,
    pub name: Option<String>,
    /// Modelo conocido por el PID (p. ej. CSL DD).
    pub model: Option<String>,
    /// El usuario puede escribir los ajustes del driver.
    pub writable: bool,
    /// Valor crudo de `SLOT` (identificador interno del firmware).
    pub slot: Option<i64>,
    /// Etiqueta amigable del SET activo.
    pub slot_label: Option<String>,
    /// Rango de giro configurado (grados).
    pub range: Option<i64>,
    pub firmware: Option<String>,
    pub wheel_id: Option<String>,
    pub settings: Vec<Setting>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Setting {
    pub key: String,
    pub label: String,
    pub value: i64,
    /// `degrees`, `percent`, `toggle` o `number`.
    pub kind: String,
    pub min: i64,
    /// `0` = maximo dependiente del dispositivo (SEN).
    pub max: i64,
}

struct Attr {
    key: &'static str,
    label: &'static str,
    kind: &'static str,
    min: i64,
    max: i64,
}

/// Atributos del menu de tuning, en el orden en que se muestran.
///
/// `SLOT` y `RESET` no entran en la lista: el primero se muestra aparte y el
/// segundo es solo escritura.
const ATTRS: &[Attr] = &[
    // 1080 es el maximo fisico del CSL DD; el driver admite hasta 1090.
    Attr { key: "SEN", label: "Sensibilidad", kind: "degrees", min: 90, max: 1080 },
    Attr { key: "FF", label: "Fuerza FFB", kind: "percent", min: 0, max: 100 },
    Attr { key: "FEI", label: "Intensidad de efectos", kind: "percent", min: 0, max: 100 },
    Attr { key: "FOR", label: "Fuerza de efectos", kind: "percent", min: 0, max: 120 },
    Attr { key: "SPR", label: "Muelle", kind: "percent", min: 0, max: 120 },
    Attr { key: "DPR", label: "Amortiguación", kind: "percent", min: 0, max: 120 },
    Attr { key: "NDP", label: "Amortiguación natural", kind: "percent", min: 0, max: 100 },
    Attr { key: "NFR", label: "Fricción natural", kind: "percent", min: 0, max: 100 },
    Attr { key: "NIN", label: "Inercia natural", kind: "percent", min: 0, max: 100 },
    Attr { key: "INT", label: "Interpolación FFB", kind: "number", min: 0, max: 20 },
    Attr { key: "FFS", label: "Escalado FFB", kind: "toggle", min: 0, max: 1 },
    Attr { key: "FUL", label: "FullForce", kind: "percent", min: 0, max: 100 },
    Attr { key: "SHO", label: "Vibración del volante", kind: "percent", min: 0, max: 100 },
    Attr { key: "BLI", label: "Indicador de freno", kind: "number", min: 0, max: 101 },
    Attr { key: "brF", label: "Fuerza de freno", kind: "percent", min: 0, max: 100 },
    Attr { key: "ACP", label: "Levas analógicas", kind: "number", min: 1, max: 4 },
    Attr { key: "DRI", label: "Modo drift", kind: "number", min: -5, max: 3 },
    Attr { key: "advanced_mode", label: "Modo avanzado", kind: "toggle", min: 0, max: 1 },
];

struct Wheel {
    dev: Option<String>,
    name: Option<String>,
    pid: Option<u32>,
}

pub fn inspect() -> Fanatec {
    let wheel = wheel_hidraw();
    let tuning = tuning_node();
    let driver = Path::new(DRIVER_DIR).is_dir() || Path::new("/sys/module/hid_fanatec").is_dir();
    let present = wheel.is_some() || tuning.is_some();

    let mut settings = Vec::new();
    let mut slot = None;
    let mut writable = false;

    if let Some(dir) = &tuning {
        slot = read_i64(dir, "SLOT");
        writable = ATTRS
            .iter()
            .map(|attr| dir.join(attr.key))
            .find(|path| path.is_file())
            .map(|path| can_write(&path))
            .unwrap_or(false);
        for attr in ATTRS {
            if let Some(value) = read_i64(dir, attr.key) {
                settings.push(Setting {
                    key: attr.key.to_string(),
                    label: attr.label.to_string(),
                    value,
                    kind: attr.kind.to_string(),
                    min: attr.min,
                    max: attr.max,
                });
            }
        }
    }

    let info = wheel.or_else(|| tuning.as_deref().and_then(tuning_info));
    let device_dir = tuning.as_ref().map(|dir| dir.join("device"));
    let range = device_dir.as_ref().and_then(|dir| read_i64(dir, "range"));
    let firmware = device_dir.as_ref().and_then(|dir| read_text(dir, "fw_version"));
    let wheel_id = device_dir.as_ref().and_then(|dir| read_text(dir, "wheel_id"));
    let slot_label = slot.map(slot_label);
    let model = info
        .as_ref()
        .and_then(|wheel| wheel.pid)
        .and_then(model_for_pid);

    let detail = if !present {
        Some("Conecta la base Fanatec por USB.".to_string())
    } else if !driver {
        Some("Falta el driver hid-fanatecff.".to_string())
    } else if tuning.is_none() {
        Some("El driver no expone el menú de tuning.".to_string())
    } else if settings.is_empty() {
        Some("No se pudieron leer los ajustes.".to_string())
    } else {
        None
    };

    Fanatec {
        present,
        driver,
        tuning: tuning.is_some(),
        device: info.as_ref().and_then(|wheel| wheel.dev.clone()),
        name: info.as_ref().and_then(|wheel| wheel.name.clone()),
        model,
        writable,
        slot,
        slot_label,
        range,
        firmware,
        wheel_id,
        settings,
        detail,
    }
}

/// Escribe un ajuste del menu de tuning en el SET activo del volante.
///
/// Devuelve el valor que queda realmente en el driver (por ejemplo, `brF` y
/// `FEI` redondean a pasos de 10).
pub fn set(key: &str, value: i64) -> Result<i64> {
    let dir = tuning_node().context("no se detecto el menu de tuning del volante")?;
    let attr = ATTRS
        .iter()
        .find(|attr| attr.key == key)
        .with_context(|| format!("ajuste desconocido: {key}"))?;
    if value < attr.min || value > attr.max {
        bail!("valor fuera de rango ({}..{})", attr.min, attr.max);
    }

    let path = dir.join(attr.key);
    if !path.is_file() {
        bail!("el volante no expone el ajuste {key}");
    }
    if !can_write(&path) {
        bail!("sin permiso de escritura sobre los ajustes del volante; prepara el acceso desde ETOL");
    }

    fs::write(&path, format!("{value}\n"))
        .with_context(|| format!("no se pudo aplicar {key}"))?;
    read_i64(&dir, attr.key).context("el driver no devolvio el valor aplicado")
}

/// Crea la regla udev que da al usuario escritura sobre `ftec_tuning`.
///
/// Solo ajustes: no toca el firmware ni instala drivers. El cambio se aplica
/// al instante con `udevadm trigger`.
pub fn fix_access() -> Result<String> {
    let user = current_user().context("no se pudo determinar el usuario")?;
    if !valid_user(&user) {
        bail!("nombre de usuario no valido: {user}");
    }
    let script = access_script(&user);

    if !command_exists("pkexec") {
        bail!(
            "no se encontro pkexec. Ejecuta esto en una terminal con sudo:\n\n{}",
            script
        );
    }

    let mut command = Command::new("pkexec");
    command.arg("/bin/sh").arg("-c").arg(&script);
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

/// Primer `hidraw` con el VID de Fanatec.
fn wheel_hidraw() -> Option<Wheel> {
    let entries = fs::read_dir("/sys/class/hidraw").ok()?;
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let Ok(text) = fs::read_to_string(entry.path().join("device/uevent")) else {
            continue;
        };
        let Some((vendor, pid)) = hid_ids(&text) else {
            continue;
        };
        if vendor != VENDOR_ID {
            continue;
        }
        found.push(Wheel {
            dev: Some(format!("/dev/{}", entry.file_name().to_string_lossy())),
            name: value_of(&text, "HID_NAME"),
            pid: Some(pid),
        });
    }
    found.sort_by(|a, b| a.dev.cmp(&b.dev));
    found.into_iter().next()
}

/// Datos de la base a partir del propio nodo de tuning.
fn tuning_info(dir: &Path) -> Option<Wheel> {
    let text = fs::read_to_string(dir.join("device/uevent")).ok()?;
    let (_, pid) = hid_ids(&text)?;
    Some(Wheel {
        dev: None,
        name: value_of(&text, "HID_NAME"),
        pid: Some(pid),
    })
}

fn tuning_node() -> Option<PathBuf> {
    let entries = fs::read_dir(TUNING_DIR).ok()?;
    let mut nodes: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    nodes.sort();
    nodes.into_iter().next()
}

fn read_i64(dir: &Path, key: &str) -> Option<i64> {
    fs::read_to_string(dir.join(key))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn read_text(dir: &Path, key: &str) -> Option<String> {
    let value = fs::read_to_string(dir.join(key)).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// 2 -> SET 1, 6 -> SET 3 (identificador interno = 2 * SET).
fn slot_label(slot: i64) -> String {
    if slot > 0 && slot % 2 == 0 && slot <= 10 {
        format!("SET {}", slot / 2)
    } else {
        format!("Slot {slot}")
    }
}

fn model_for_pid(pid: u32) -> Option<String> {
    match pid {
        0x0020 => Some("CSL DD".to_string()),
        _ => None,
    }
}

fn hid_ids(text: &str) -> Option<(u32, u32)> {
    let value = value_of(text, "HID_ID")?;
    let mut parts = value.split(':');
    let _bus = parts.next()?;
    let vendor = u32::from_str_radix(parts.next()?, 16).ok()?;
    let product = u32::from_str_radix(parts.next()?, 16).ok()?;
    Some((vendor, product))
}

fn value_of(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim().to_string())
    })
}

/// `access(2)` respeta permisos de grupo y ACL.
fn can_write(path: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

fn current_user() -> Option<String> {
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            return Some(user);
        }
    }
    let output = Command::new("id").arg("-un").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let user = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!user.is_empty()).then_some(user)
}

fn valid_user(user: &str) -> bool {
    !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn access_script(user: &str) -> String {
    format!(
        r#"set -eu
mkdir -p /etc/udev/rules.d
cat > {rule} <<'EOF'
# ETOL: Fanatec CSL DD - escritura de ajustes para {user}.
ACTION!="add|change", GOTO="etol_fanatec_end"
SUBSYSTEM=="ftec_tuning", RUN+="/bin/sh -c 'cd %S%p && chown -f {user} {editable} 2>/dev/null || true'"
LABEL="etol_fanatec_end"
EOF
udevadm control --reload-rules
udevadm trigger --subsystem-match=ftec_tuning
udevadm settle --timeout=5 >/dev/null 2>&1 || true
echo "Acceso de escritura de Fanatec preparado para {user}."
"#,
        rule = RULE_PATH,
        user = user,
        editable = EDITABLE
    )
}

fn command_exists(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_script_is_valid_shell() {
        let script = access_script("barbas");
        assert!(script.contains(RULE_PATH));
        assert!(script.contains("chown -f barbas"));
        let status = Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&script)
            .status()
            .expect("sh");
        assert!(status.success());
    }
}

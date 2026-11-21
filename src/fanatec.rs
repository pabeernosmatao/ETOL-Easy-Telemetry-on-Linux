//! Fanatec (CSL DD y otras bases): ajustes del menu de tuning.
//!
//! El driver `hid-fanatecff` expone el menu de tuning por sysfs
//! (`/sys/class/ftec_tuning`). Esta ficha solo **lee** esos valores: no abre
//! el HID ni escribe en el volante. El firmware queda fuera de ETOL.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const VENDOR_ID: u32 = 0x0eb7;
const TUNING_DIR: &str = "/sys/class/ftec_tuning";
const DRIVER_DIR: &str = "/sys/bus/hid/drivers/fanatec";

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
    Attr { key: "SEN", label: "Sensibilidad", kind: "degrees", min: 90, max: 0 },
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
        settings,
        detail,
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

//! Inspeccion del estado en vivo: procesos, dispositivos serie, puertos y locks.
//! Todo es de solo lectura sobre /proc y /sys.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub cmdline: String,
}

#[derive(Debug, Clone)]
pub struct Hf8Device {
    pub dev: PathBuf,
    pub vendor: String,
    pub product: String,
    pub name: String,
}

/// Procesos con linea de comandos no vacia.
pub fn processes() -> Vec<Proc> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let raw = std::fs::read(entry.path().join("cmdline")).unwrap_or_default();
        if raw.is_empty() {
            continue;
        }
        let cmdline = String::from_utf8_lossy(&raw)
            .replace('\0', " ")
            .trim()
            .to_string();
        out.push(Proc { pid, cmdline });
    }

    out
}

/// Procesos cuya linea de comandos contiene el texto (sin distinguir mayusculas).
pub fn matching(needle: &str) -> Vec<Proc> {
    let needle = needle.to_lowercase();
    processes()
        .into_iter()
        .filter(|proc| proc.cmdline.to_lowercase().contains(&needle))
        .collect()
}

pub fn any_matching(needle: &str) -> bool {
    !matching(needle).is_empty()
}

/// Procesos Windows reales: ignora los envoltorios `start.exe` de wine, que
/// mantienen el nombre del ejecutable en su linea de comandos, y cualquier
/// proceso que solo mencione el nombre (una shell, un editor...). Los
/// ejecutables de wine empiezan por una unidad `C:\` o `Z:\`.
pub fn running_windows_exe(needle: &str) -> Vec<Proc> {
    matching(needle)
        .into_iter()
        .filter(|proc| !proc.cmdline.contains("start.exe"))
        .filter(|proc| {
            let cmdline = proc.cmdline.trim_start();
            cmdline.starts_with("C:\\") || cmdline.starts_with("Z:\\")
        })
        .collect()
}

/// Como `running_windows_exe`, pero acepta cualquier unidad de Wine (`S:\`,
/// `D:\`...). Los juegos de Steam suelen arrancar desde una unidad propia.
pub fn running_drive_exe(needle: &str) -> Vec<Proc> {
    matching(needle)
        .into_iter()
        .filter(|proc| !proc.cmdline.contains("start.exe"))
        .filter(|proc| is_windows_path(proc.cmdline.trim_start()))
        .collect()
}

fn is_windows_path(cmdline: &str) -> bool {
    let bytes = cmdline.as_bytes();
    bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// Dispositivos ForceFeel/HF8 conectados por USB CDC.
pub fn hf8_devices() -> Vec<Hf8Device> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/tty") else {
        return out;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.starts_with("ttyACM") || name.starts_with("ttyUSB")) {
            continue;
        }
        let device = entry.path().join("device");
        let usb = device.join("..");
        let vendor = read_trimmed(&usb.join("idVendor")).unwrap_or_default();
        let product = read_trimmed(&usb.join("idProduct")).unwrap_or_default();
        if vendor.is_empty() && product.is_empty() {
            continue;
        }
        out.push(Hf8Device {
            dev: PathBuf::from("/dev").join(&name),
            vendor,
            product,
            name: read_trimmed(&usb.join("product")).unwrap_or_default(),
        });
    }

    out.sort_by(|a, b| a.dev.cmp(&b.dev));
    out
}

/// Enlaces `dosdevices/comNN -> /dev/ttyXXX` de un prefijo de Wine.
pub fn com_links(prefix: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(prefix.join("dosdevices")) else {
        return out;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.to_lowercase().starts_with("com") {
            continue;
        }
        if !entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
            continue;
        }
        if let Ok(target) = std::fs::read_link(entry.path()) {
            out.push((name.to_lowercase(), target));
        }
    }

    out.sort_by_key(|(name, _)| {
        name.trim_start_matches("com")
            .parse::<u32>()
            .unwrap_or(u32::MAX)
    });
    out
}

/// Procesos que tienen abierto un dispositivo o fichero concreto.
pub fn fd_holders(target: &Path) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if std::fs::read_link(fd.path()).ok().as_deref() != Some(target) {
                continue;
            }
            let comm = std::fs::read_to_string(entry.path().join("comm"))
                .unwrap_or_default()
                .trim()
                .to_string();
            out.push((pid, comm));
            break;
        }
    }

    out
}

/// Alias para dispositivos.
pub fn holders(dev: &Path) -> Vec<(u32, String)> {
    fd_holders(dev)
}

/// Comprueba si un puerto UDP local esta en uso.
pub fn udp_in_use(port: u16) -> bool {
    ["/proc/net/udp", "/proc/net/udp6"]
        .iter()
        .any(|path| udp_table_has_port(path, port))
}

/// Comprueba si un puerto TCP esta escuchando.
pub fn tcp_listening(port: u16) -> bool {
    ["/proc/net/tcp", "/proc/net/tcp6"]
        .iter()
        .any(|path| tcp_table_listens(path, port))
}

fn tcp_table_listens(path: &str, port: u16) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    for line in text.lines().skip(1) {
        let mut fields = line.split_whitespace();
        let _sl = fields.next();
        let Some(local) = fields.next() else {
            continue;
        };
        let _remote = fields.next();
        let Some(state) = fields.next() else {
            continue;
        };
        if state != "0A" {
            continue;
        }
        let Some((_, raw_port)) = local.split_once(':') else {
            continue;
        };
        if u16::from_str_radix(raw_port, 16).ok() == Some(port) {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallState {
    Inactive,
    Allowed,
    Blocked,
    Unknown,
}

/// Intenta saber si el firewall permite un puerto TCP.
///
/// No pide contrasena: usa `ufw status` / `firewall-cmd` si el usuario puede
/// consultarlos. Si no hay permisos, devuelve `Unknown` para que el doctor
/// muestre el comando que hay que ejecutar.
pub fn firewall_port(port: u16) -> FirewallState {
    if command_exists("ufw") {
        if let Ok(output) = std::process::Command::new("ufw").arg("status").output() {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let lower = text.to_lowercase();
            if lower.contains("status: active") || lower.contains("estado: activo") {
                let needle = format!("{port}/tcp");
                let allowed = text.lines().any(|line| {
                    let line = line.to_lowercase();
                    line.contains(&needle)
                        && (line.contains("allow") || line.contains("permit"))
                });
                return if allowed {
                    FirewallState::Allowed
                } else {
                    FirewallState::Blocked
                };
            }
            if lower.contains("status: inactive") || lower.contains("estado: inactivo") {
                return FirewallState::Inactive;
            }
        }
        // ufw activo pero sin permisos para leer las reglas.
        if std::process::Command::new("systemctl")
            .args(["is-active", "ufw"])
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "active")
            .unwrap_or(false)
        {
            return FirewallState::Unknown;
        }
        return FirewallState::Inactive;
    }

    if command_exists("firewall-cmd") {
        let running = std::process::Command::new("firewall-cmd")
            .arg("--state")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "running")
            .unwrap_or(false);
        if !running {
            return FirewallState::Inactive;
        }
        let query = std::process::Command::new("firewall-cmd")
            .arg(format!("--query-port={port}/tcp"))
            .output();
        return match query {
            Ok(out) if out.status.success() => FirewallState::Allowed,
            Ok(_) => FirewallState::Blocked,
            Err(_) => FirewallState::Unknown,
        };
    }

    FirewallState::Unknown
}

fn command_exists(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

fn udp_table_has_port(path: &str, port: u16) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };

    for line in text.lines().skip(1) {
        let Some(local) = line.split_whitespace().nth(1) else {
            continue;
        };
        let Some((_, raw_port)) = local.split_once(':') else {
            continue;
        };
        if u16::from_str_radix(raw_port, 16).ok() == Some(port) {
            return true;
        }
    }

    false
}

/// Fichero de lock del arranque ordenado, si existe.
pub fn arranque_lock(run_dir: &Path) -> Option<PathBuf> {
    let lock = run_dir.join("arranque.lock");
    lock.is_file().then_some(lock)
}

fn read_trimmed(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

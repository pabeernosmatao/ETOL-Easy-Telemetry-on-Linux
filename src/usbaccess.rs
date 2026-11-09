//! Acceso del usuario a los puertos serie USB (HF8, LEDs, Arduino...).
//!
//! En Linux, `/dev/ttyACM*` y `/dev/ttyUSB*` suelen pertenecer a `root:uucp`
//! (Arch/CachyOS) o `root:dialout` (Debian/Fedora). Si el usuario no esta en
//! ese grupo, Wine/SimHub no puede abrir el HF8 aunque el enlace COM exista.
//! Ademas, `brltty` puede reclamar dispositivos CDC-ACM.
//!
//! La interfaz muestra el estado y ofrece un boton que aplica el arreglo con
//! pkexec: anade el usuario al grupo, instala una regla udev `uaccess` y
//! enmascara brltty si esta instalado.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

pub const RULE_PATH: &str = "/etc/udev/rules.d/99-etol-serial.rules";

#[derive(Debug, Clone, Serialize)]
pub struct UsbDevice {
    pub dev: String,
    pub name: String,
    pub accessible: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsbAccess {
    pub user: String,
    pub serial_group: Option<String>,
    pub in_group: bool,
    pub brltty_present: bool,
    pub brltty_masked: bool,
    pub brltty_active: bool,
    pub rule_present: bool,
    pub rule_ok: bool,
    pub devices: Vec<UsbDevice>,
    pub needs_fix: bool,
    pub needs_relogin: bool,
}

pub fn inspect() -> UsbAccess {
    let user = current_user();
    let serial_group = serial_device_group().or_else(default_serial_group);
    let in_group = serial_group
        .as_deref()
        .map(|group| user_in_group(&user, group))
        .unwrap_or(false);

    let brltty_present = brltty_installed();
    let brltty_masked = brltty_present
        && systemctl_state("is-enabled", "brltty-udev.service").as_deref() == Some("masked");
    let brltty_active = brltty_present
        && systemctl_state("is-active", "brltty-udev.service").as_deref() == Some("active");

    let (rule_present, rule_ok) = rule_status();
    let devices = devices();

    let access_ok = if devices.is_empty() {
        in_group || rule_ok
    } else {
        devices.iter().all(|device| device.accessible)
    };
    let brltty_problem = brltty_present && (brltty_active || !brltty_masked);
    let needs_relogin = !in_group && serial_group.is_some();

    UsbAccess {
        user,
        serial_group,
        in_group,
        brltty_present,
        brltty_masked,
        brltty_active,
        rule_present,
        rule_ok,
        devices,
        needs_fix: !access_ok || brltty_problem,
        needs_relogin,
    }
}

/// Aplica el arreglo con pkexec. Devuelve la salida para mostrarla en la UI.
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

fn current_user() -> String {
    std::env::var("USER")
        .ok()
        .filter(|user| !user.trim().is_empty())
        .or_else(|| command_text("id", &["-un"]))
        .unwrap_or_else(|| "usuario".to_string())
}

fn default_serial_group() -> Option<String> {
    ["uucp", "dialout"]
        .iter()
        .find(|group| group_exists(group))
        .map(|group| (*group).to_string())
}

fn serial_device_group() -> Option<String> {
    for device in crate::state::hf8_devices() {
        let Ok(meta) = fs::metadata(&device.dev) else {
            continue;
        };
        let Some(name) = group_name(meta.gid()) else {
            continue;
        };
        if name != "root" {
            return Some(name);
        }
    }
    None
}

fn group_name(gid: u32) -> Option<String> {
    if let Some(text) = command_text("getent", &["group", &gid.to_string()]) {
        if let Some(name) = text.split(':').next() {
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    let text = fs::read_to_string("/etc/group").ok()?;
    text.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        let _password = parts.next()?;
        let id = parts.next()?;
        (id == gid.to_string()).then(|| name.to_string())
    })
}

fn group_exists(name: &str) -> bool {
    if Command::new("getent")
        .args(["group", name])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return true;
    }
    fs::read_to_string("/etc/group")
        .map(|text| text.lines().any(|line| line.split(':').next() == Some(name)))
        .unwrap_or(false)
}

fn user_in_group(user: &str, group: &str) -> bool {
    command_text("id", &["-nG", user])
        .map(|text| text.split_whitespace().any(|name| name == group))
        .unwrap_or(false)
}

fn brltty_installed() -> bool {
    Path::new("/usr/lib/systemd/system/brltty-udev.service").exists()
        || Path::new("/lib/systemd/system/brltty-udev.service").exists()
        || command_exists("brltty")
}

fn systemctl_state(verb: &str, unit: &str) -> Option<String> {
    command_text("systemctl", &[verb, unit])
}

fn rule_status() -> (bool, bool) {
    match fs::read_to_string(RULE_PATH) {
        Ok(text) => {
            let ok = text.contains("ttyACM")
                && text.contains("ttyUSB")
                && text.contains("uaccess");
            (true, ok)
        }
        Err(_) => (false, false),
    }
}

fn devices() -> Vec<UsbDevice> {
    crate::state::hf8_devices()
        .into_iter()
        .map(|device| UsbDevice {
            dev: device.dev.display().to_string(),
            name: device.name,
            accessible: can_access(&device.dev),
        })
        .collect()
}

/// `access(2)` respeta los permisos del grupo y las ACL de `uaccess`.
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

fn command_text(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Script fijo (sin datos del usuario interpolados) que se ejecuta como root.
/// `PKEXEC_UID` lo pone pkexec; `SUDO_UID`, sudo; si no, se usa el uid actual.
const FIX_SCRIPT: &str = r#"set -eu
uid="${PKEXEC_UID:-${SUDO_UID:-$(id -u)}}"
user="$(getent passwd "$uid" 2>/dev/null | cut -d: -f1)"
[ -n "$user" ] || user="$(id -nu "$uid" 2>/dev/null || true)"
if [ -z "$user" ]; then
  echo "No se pudo identificar al usuario." >&2
  exit 1
fi

group=""
for dev in /dev/ttyACM* /dev/ttyUSB*; do
  [ -e "$dev" ] || continue
  g="$(stat -c '%G' "$dev" 2>/dev/null || true)"
  if [ -n "$g" ] && [ "$g" != "root" ]; then
    group="$g"
    break
  fi
done
if [ -z "$group" ]; then
  if getent group uucp >/dev/null 2>&1; then
    group=uucp
  elif getent group dialout >/dev/null 2>&1; then
    group=dialout
  fi
fi

relogin=0
if [ -n "$group" ]; then
  if id -nG "$user" | tr ' ' '\n' | grep -qx "$group"; then
    echo "El usuario $user ya pertenece al grupo $group."
  else
    usermod -aG "$group" "$user"
    echo "Usuario $user anadido al grupo $group."
    relogin=1
  fi
else
  echo "No se encontro el grupo de los puertos serie (uucp/dialout)."
fi

if [ -e /usr/lib/systemd/system/brltty-udev.service ] || [ -e /lib/systemd/system/brltty-udev.service ] || command -v brltty >/dev/null 2>&1; then
  systemctl mask --now brltty-udev.service >/dev/null 2>&1 || true
  systemctl mask --now brltty.service >/dev/null 2>&1 || true
  echo "brltty enmascarado: no reclamara los puertos serie."
fi

mkdir -p /etc/udev/rules.d
cat > /etc/udev/rules.d/99-etol-serial.rules <<'EOF'
# ETOL: acceso del usuario a los puertos serie USB (HF8, LEDs, Arduino...).
KERNEL=="ttyACM[0-9]*", SUBSYSTEM=="tty", TAG+="uaccess"
KERNEL=="ttyUSB[0-9]*", SUBSYSTEM=="tty", TAG+="uaccess"
EOF
udevadm control --reload-rules
udevadm trigger --subsystem-match=tty
udevadm settle --timeout=5 >/dev/null 2>&1 || true
echo "Regla udev instalada y aplicada."

if [ "$relogin" = 1 ]; then
  echo "El acceso por udev ya deberia funcionar. Para el grupo, cierra sesion y vuelve a entrar."
else
  echo "Acceso USB preparado."
fi
"#;

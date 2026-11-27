//! Sonda del entorno donde corre ETOL.
//!
//! Distingue sistemas clasicos (Arch, Debian, Fedora...) de sistemas Atomic
//! (Bazzite, Fedora Silverblue, bootc...). El resultado se guarda en
//! `[system]` de la configuracion y lo usa el resto de ETOL para elegir el
//! gestor de paquetes, el ambito de Flatpak y las rutas reales del home.
//!
//! En Bazzite `HOME=/home/barbas` es un enlace a `/var/home/barbas`; comparar
//! rutas literales marca como "fuera del home" bibliotecas que si estan dentro.
//! Por eso se guarda tambien `home_real`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Botella que ETOL busca en Flatpak.
const BOTTLES_ID: &str = "com.usebottles.bottles";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SystemInfo {
    /// `atomic` o `classic`.
    pub kind: String,
    /// Identificador de `/etc/os-release` (`bazzite`, `cachyos`, `ubuntu`...).
    pub distro: String,
    /// Nombre legible de la distribucion.
    pub distro_name: String,
    /// `$HOME` literal.
    pub home: String,
    /// `$HOME` canonicalizado (`/var/home/...` en Bazzite).
    pub home_real: String,
    /// Ambito activo de Bottles Flatpak: `user`, `system` o `none`.
    pub flatpak_scope: String,
    /// `rpm-ostree`, `pacman`, `apt`, `dnf` o `unknown`.
    pub package_backend: String,
    /// `$HOME` no coincide con `home_real`.
    pub needs_path_normalization: bool,
    /// Existe `/run/ostree-booted`.
    pub ostree_booted: bool,
    /// Existen `/usr/lib/group` y `/usr/lib/passwd` (NSS altfiles).
    pub altfiles: bool,
    /// Fecha de la ultima sonda.
    pub detected: String,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            kind: "unknown".to_string(),
            distro: "unknown".to_string(),
            distro_name: "Desconocido".to_string(),
            home: String::new(),
            home_real: String::new(),
            flatpak_scope: "none".to_string(),
            package_backend: "unknown".to_string(),
            needs_path_normalization: false,
            ostree_booted: false,
            altfiles: false,
            detected: String::new(),
        }
    }
}

impl SystemInfo {
    pub fn is_atomic(&self) -> bool {
        self.kind == "atomic"
    }

    /// Nombre del paquete que proporciona un comando en este sistema.
    pub fn package_for(&self, command: &str) -> String {
        match (self.package_backend.as_str(), command) {
            ("pacman", "python3") => "python".to_string(),
            ("pacman", "bsdtar") => "libarchive".to_string(),
            ("apt", "bsdtar") => "libarchive-tools".to_string(),
            _ => command.to_string(),
        }
    }

    /// Comando que puede pegar el usuario en una terminal (con `sudo` si hace
    /// falta) para instalar un comando.
    pub fn install_command(&self, package: &str) -> String {
        match self.package_backend.as_str() {
            "rpm-ostree" => format!("rpm-ostree install --apply-live {package}"),
            "pacman" => format!("sudo pacman -S {package}"),
            "apt" => format!("sudo apt install {package}"),
            "dnf" => format!("sudo dnf install {package}"),
            _ => format!(
                "instala {package} con el gestor de paquetes de tu distribucion"
            ),
        }
    }

    /// Igual que `install_command`, pero sin `sudo`: para ejecutarlo con
    /// `pkexec` (que ya eleva privilegios).
    pub fn install_command_root(&self, package: &str) -> String {
        match self.package_backend.as_str() {
            "rpm-ostree" => format!("rpm-ostree install --apply-live {package}"),
            "pacman" => format!("pacman -S --noconfirm {package}"),
            "apt" => format!("apt-get install -y {package}"),
            "dnf" => format!("dnf install -y {package}"),
            _ => format!("instala {package} con el gestor de paquetes de tu distribucion"),
        }
    }

    /// Pista para instalar el paquete que da un comando (`python3`, `curl`...).
    pub fn install_hint(&self, command: &str) -> String {
        self.install_command(&self.package_for(command))
    }

    /// Primer extractor de RAR disponible.
    pub fn archive_tool(&self) -> Option<&'static str> {
        ["bsdtar", "7z", "7za", "unrar", "unar"]
            .into_iter()
            .find(|tool| crate::util::which(tool).is_some())
    }

    /// Paquete que da `bsdtar` en este sistema.
    pub fn bsdtar_package(&self) -> String {
        self.package_for("bsdtar")
    }
}

/// Sondea el sistema actual.
pub fn detect() -> SystemInfo {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let home_real = fs::canonicalize(&home).unwrap_or_else(|_| home.clone());

    let (distro, distro_name) = os_release();
    let ostree_booted = Path::new("/run/ostree-booted").exists();
    let rpm_ostree = command_ok("rpm-ostree", &["status", "--json"]);
    let atomic = ostree_booted || rpm_ostree;

    let package_backend = if atomic && crate::util::which("rpm-ostree").is_some() {
        "rpm-ostree"
    } else if crate::util::which("pacman").is_some() {
        "pacman"
    } else if crate::util::which("apt-get").is_some() || crate::util::which("apt").is_some() {
        "apt"
    } else if crate::util::which("dnf").is_some() {
        "dnf"
    } else {
        "unknown"
    };

    SystemInfo {
        kind: if atomic { "atomic" } else { "classic" }.to_string(),
        distro,
        distro_name,
        home: home.display().to_string(),
        home_real: home_real.display().to_string(),
        flatpak_scope: flatpak_scope(),
        package_backend: package_backend.to_string(),
        needs_path_normalization: home != home_real,
        ostree_booted,
        altfiles: Path::new("/usr/lib/group").is_file()
            && Path::new("/usr/lib/passwd").is_file(),
        detected: crate::util::now_utc(),
    }
}

fn command_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn flatpak_scope() -> String {
    if crate::util::which("flatpak").is_none() {
        return "none".to_string();
    }
    if flatpak_has("--user") {
        "user".to_string()
    } else if flatpak_has("--system") {
        "system".to_string()
    } else {
        "none".to_string()
    }
}

fn flatpak_has(scope: &str) -> bool {
    let Ok(output) = Command::new("flatpak")
        .args(["list", scope, "--app", "--columns=application"])
        .output()
    else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.trim() == BOTTLES_ID)
}

fn os_release() -> (String, String) {
    let text = fs::read_to_string("/etc/os-release").unwrap_or_default();
    parse_os_release(&text)
}

/// Extrae `ID` y el nombre legible de un `/etc/os-release`.
fn parse_os_release(text: &str) -> (String, String) {
    let mut id = String::new();
    let mut name = String::new();
    let mut pretty = String::new();
    let mut like = String::new();

    for line in text.lines() {
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        let value = raw.trim().trim_matches('"').trim_matches('\'');
        match key.trim() {
            "ID" => id = value.to_lowercase(),
            "NAME" => name = value.to_string(),
            "PRETTY_NAME" => pretty = value.to_string(),
            "ID_LIKE" => like = value.to_lowercase(),
            _ => {}
        }
    }

    if id.is_empty() {
        id = like
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_string();
    }
    let display = if !name.is_empty() {
        name
    } else if !pretty.is_empty() {
        pretty
    } else {
        id.clone()
    };
    (id, display)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bazzite_os_release() {
        let text = r#"
NAME="Bazzite"
PRETTY_NAME="Bazzite 45.20261127"
ID=bazzite
ID_LIKE="fedora"
"#;
        assert_eq!(parse_os_release(text), ("bazzite".into(), "Bazzite".into()));
    }

    #[test]
    fn parses_cachyos_os_release() {
        let text = r#"
NAME="CachyOS Linux"
PRETTY_NAME="CachyOS"
ID=cachyos
ID_LIKE=arch
"#;
        assert_eq!(
            parse_os_release(text),
            ("cachyos".into(), "CachyOS Linux".into())
        );
    }

    #[test]
    fn detect_fills_the_basics() {
        let info = detect();
        assert!(!info.home.is_empty());
        assert!(!info.home_real.is_empty());
        assert!(!info.kind.is_empty());
        assert!(!info.detected.is_empty());
        println!("{info:?}");
    }

    #[test]
    fn package_hints_change_with_the_backend() {
        let mut system = SystemInfo::default();
        system.package_backend = "rpm-ostree".into();
        assert_eq!(system.package_for("bsdtar"), "bsdtar");
        assert_eq!(
            system.install_hint("bsdtar"),
            "rpm-ostree install --apply-live bsdtar"
        );

        system.package_backend = "pacman".into();
        assert_eq!(system.package_for("bsdtar"), "libarchive");
        assert_eq!(system.package_for("python3"), "python");
        assert_eq!(system.install_hint("bsdtar"), "sudo pacman -S libarchive");

        system.package_backend = "apt".into();
        assert_eq!(system.package_for("bsdtar"), "libarchive-tools");
        assert_eq!(
            system.install_hint("bsdtar"),
            "sudo apt install libarchive-tools"
        );
    }
}

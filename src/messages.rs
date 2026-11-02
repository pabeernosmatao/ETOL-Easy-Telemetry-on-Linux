//! Textos de diagnostico compartidos por la CLI y la interfaz grafica.
//!
//! Aqui vive el unico origen de verdad de los mensajes accionables: las rutas
//! reales, el comando concreto y la tecla de configuracion que hay que tocar.
//! La GUI recibe estos mismos textos a traves del doctor, sin duplicarlos.

use std::path::{Path, PathBuf};

/// Comando con el que el usuario puede ejecutar la CLI.
///
/// - Si `etol` esta en `PATH` y es este mismo binario, se usa el nombre.
/// - Si la GUI viaja con el CLI al lado (AppImage), se usa la ruta absoluta.
/// - En cualquier otro caso se asume que el nombre esta en `PATH`.
pub fn cli() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(found) = which("etol") {
            if let (Ok(a), Ok(b)) = (found.canonicalize(), exe.canonicalize()) {
                if a == b {
                    return "etol".to_string();
                }
            }
        }
        if exe.file_name().and_then(|name| name.to_str()) == Some("etol") {
            return exe.display().to_string();
        }
        let sibling = exe.with_file_name("etol");
        if sibling.is_file() {
            return sibling.display().to_string();
        }
    }
    "etol".to_string()
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn detect_fix() -> String {
    format!("{} detect --save", cli())
}

pub fn set_fix(key: &str, value: &str) -> String {
    format!("{} config set {} {}", cli(), key, value)
}

pub fn doctor_fix() -> String {
    format!("{} doctor", cli())
}

pub fn payload_missing(searched: &[PathBuf]) -> String {
    let list = searched
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("no se encontro el payload de scripts; buscado en: {list}")
}

pub fn payload_fix(searched: &[PathBuf]) -> String {
    let hint = searched
        .first()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<APPDIR>/usr/lib/ETOL/payload".to_string());
    crate::tr!(
        "reinstala la aplicacion completa (falta {hint}); si prefieres usar tus propios scripts, ejecuta: {}",
        "reinstal·la l'aplicació completa (falta {hint}); si prefereixes usar els teus propis scripts, executa: {}",
        "reinstall the full application (missing {hint}); if you prefer your own scripts, run: {}",
        set_fix("bridge.app_dir", "<ruta>")
    )
}

pub fn app_dir_missing(dir: &Path) -> String {
    crate::tr!(
        "no existe la carpeta de scripts {}",
        "no existeix la carpeta de scripts {}",
        "scripts folder does not exist: {}",
        dir.display()
    )
}

pub fn app_dir_unknown() -> String {
    crate::tr!(
        "no se encontro la carpeta de scripts",
        "no s'ha trobat la carpeta de scripts",
        "scripts folder not found"
    )
    .to_string()
}

pub fn app_dir_fix() -> String {
    set_fix("bridge.app_dir", "<ruta>")
}

pub fn bottle_missing() -> String {
    crate::tr!(
        "no se encontro ninguna botella con SimHub",
        "no s'ha trobat cap ampolla amb SimHub",
        "no bottle with SimHub found"
    )
    .to_string()
}

pub fn bottle_fix() -> String {
    set_fix("apps.bottle", "<ruta>")
}

/// Como instalar Bottles cuando no hay ninguna instalacion.
pub fn bottles_install_fix() -> String {
    format!("{} setup bottles-install", cli())
}

/// Bottles instalado solo para el sistema: instalar la copia de usuario.
pub fn bottles_user_fix() -> String {
    format!("{} setup bottles-user", cli())
}

/// Permisos minimos del Flatpak (home, /etc, Steam y HF8).
pub fn flatpak_permissions_fix() -> String {
    format!("{} setup bottles-user", cli())
}

pub fn bottle_gone(path: &Path) -> String {
    crate::tr!(
        "la botella configurada ya no existe: {}",
        "l'ampolla configurada ja no existeix: {}",
        "the configured bottle no longer exists: {}",
        path.display()
    )
}

pub fn wine_missing(searched: &[PathBuf]) -> String {
    let list = searched
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if list.is_empty() {
        crate::tr!(
            "no se encontro ningun wine para la botella",
            "no s'ha trobat cap wine per a l'ampolla",
            "no wine found for the bottle"
        )
        .to_string()
    } else {
        crate::tr!(
            "no se encontro ningun wine; buscado en: {}",
            "no s'ha trobat cap wine; cercat a: {}",
            "no wine found; searched in: {}",
            list
        )
    }
}

pub fn wine_fix() -> String {
    set_fix("apps.wine", "<ruta>")
}

pub fn wine_system(path: &Path) -> String {
    crate::tr!(
        "se usara el wine del sistema ({}): puede actualizar el prefijo de la botella y romper dotnet48/wine-mono de SimHub; configura el runner exacto con {}",
        "s'usarà el wine del sistema ({}): pot actualitzar el prefix de l'ampolla i trencar dotnet48/wine-mono de SimHub; configura el runner exacte amb {}",
        "the system wine will be used ({}): it may update the bottle prefix and break SimHub's dotnet48/wine-mono; set the exact runner with {}",
        path.display(),
        wine_fix()
    )
}

pub fn wine_mismatch(path: &Path, runner: &str) -> String {
    crate::tr!(
        "el wine {} no parece el runner '{}' que declara la botella; usarlo puede actualizar el prefijo",
        "el wine {} no sembla el runner '{}' que declara l'ampolla; usar-lo pot actualitzar el prefix",
        "wine {} does not look like the '{}' runner declared by the bottle; using it may update the prefix",
        path.display(),
        runner
    )
}

pub fn hf8_missing(vendor: &str, product: &str) -> String {
    crate::tr!(
        "no hay ningun dispositivo {}:{} conectado; el pad es opcional, conectalo y ejecuta {} si quieres usarlo",
        "no hi ha cap dispositiu {}:{} connectat; el pad és opcional, connecta'l i executa {} si vols usar-lo",
        "no {}:{} device connected; the pad is optional, connect it and run {} if you want to use it",
        vendor,
        product,
        detect_fix()
    )
}

pub fn com_missing(com: &str) -> String {
    crate::tr!(
        "no existe el enlace {} en dosdevices; ejecuta {} si quieres configurar el pad",
        "no existeix l'enllaç {} a dosdevices; executa {} si vols configurar el pad",
        "the {} link does not exist in dosdevices; run {} if you want to configure the pad",
        com,
        format!("{} hf8", cli())
    )
}

pub fn port_busy(port: u16) -> String {
    crate::tr!(
        "el puerto UDP {} esta ocupado por otro proceso",
        "el port UDP {} està ocupat per un altre procés",
        "UDP port {} is used by another process",
        port
    )
}

pub fn port_busy_fix(port: u16) -> String {
    format!("ss -lunp | grep {port}")
}

pub fn firewall_fix(port: u16) -> String {
    format!(
        "ufw: sudo ufw allow {port}/tcp  |  firewalld: sudo firewall-cmd --add-port={port}/tcp --permanent && sudo firewall-cmd --reload"
    )
}

pub fn game_missing(name: &str) -> String {
    crate::tr!(
        "{} no esta instalado o no se ha detectado",
        "{} no està instal·lat o no s'ha detectat",
        "{} is not installed or was not detected",
        name
    )
}

pub fn game_incomplete(name: &str, missing: &[&str]) -> String {
    let missing: Vec<String> = missing
        .iter()
        .map(|item| crate::i18n::text(item))
        .collect();
    crate::tr!(
        "{}: falta {}",
        "{}: falta {}",
        "{}: missing {}",
        name,
        missing.join(", ")
    )
}

pub fn script_missing(name: &str) -> String {
    crate::tr!(
        "falta el script {} en la carpeta de scripts",
        "falta el script {} a la carpeta de scripts",
        "script {} is missing from the scripts folder",
        name
    )
}

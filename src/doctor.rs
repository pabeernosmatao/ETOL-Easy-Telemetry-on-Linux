//! Comprobaciones de la instalacion con sugerencias de arreglo.
//!
//! Severidades:
//! - FALLO: payload ausente/incompleto, botella configurada que ya no existe,
//!   wine configurado a mano que no existe y puertos ocupados por otro proceso.
//! - AVISO: juegos no instalados, HF8 desconectado (hardware opcional), wine
//!   resuelto por fallback del sistema, locks huerfanos, etc.
//!
//! El pad HF8 nunca es un fallo: es hardware opcional.

use crate::config::{self, Config};
use crate::detect;
use crate::messages;
use crate::payload;
use crate::state;
use crate::util;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub fix: Option<String>,
}

fn ok(title: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        level: Level::Ok,
        title: title.into(),
        detail: detail.into(),
        fix: None,
    }
}

fn warn(title: impl Into<String>, detail: impl Into<String>, fix: Option<String>) -> Check {
    Check {
        level: Level::Warn,
        title: title.into(),
        detail: detail.into(),
        fix,
    }
}

fn fail(title: impl Into<String>, detail: impl Into<String>, fix: Option<String>) -> Check {
    Check {
        level: Level::Fail,
        title: title.into(),
        detail: detail.into(),
        fix,
    }
}

/// Indica si la carpeta de scripts activa es la copia instalada del payload.
fn uses_payload(cfg: &Config) -> bool {
    match cfg.app_dir() {
        Some(dir) => dir == payload::dest(),
        None => true,
    }
}

pub fn run(cfg: &Config) -> Vec<Check> {
    let mut checks = Vec::new();

    // --- Payload incluido ---
    let report = payload::report();
    if !uses_payload(cfg) {
        checks.push(ok(
            "Payload",
            "no se usa: la configuracion apunta a una carpeta de scripts propia",
        ));
    } else if report.ok {
        checks.push(ok("Payload", report.detail.clone()));
    } else {
        let searched: Vec<PathBuf> = report.searched.iter().map(PathBuf::from).collect();
        checks.push(fail(
            "Payload",
            report.detail.clone(),
            Some(messages::payload_fix(&searched)),
        ));
    }

    // --- Logs ---
    if crate::logs::enabled() {
        checks.push(ok("Logs", crate::logs::summary()));
    } else {
        checks.push(ok("Logs", "captura desactivada en Opciones"));
    }

    // --- Dependencias externas ---
    match util::which("python3") {
        Some(path) => checks.push(ok("Python", path.display().to_string())),
        None => checks.push(fail(
            "Python",
            "no se encontró python3; los puentes de telemetría lo necesitan",
            Some(
                "Arch/CachyOS: sudo pacman -S python · Ubuntu: sudo apt install python3 · Fedora: sudo dnf install python3"
                    .to_string(),
            ),
        )),
    }
    if util::which("curl").is_none() {
        checks.push(warn(
            "curl",
            "no se encontró curl; la preparación del prefijo lo necesita para descargar los instaladores",
            Some(
                "Arch/CachyOS: sudo pacman -S curl · Ubuntu: sudo apt install curl · Fedora: sudo dnf install curl"
                    .to_string(),
            ),
        ));
    }

    // --- Carpeta de scripts ---
    match cfg.app_dir() {
        Some(dir) if dir.is_dir() => checks.push(ok("Carpeta de scripts", dir.display().to_string())),
        Some(dir) => checks.push(fail(
            "Carpeta de scripts",
            messages::app_dir_missing(dir),
            Some(messages::app_dir_fix()),
        )),
        None => checks.push(fail(
            "Carpeta de scripts",
            messages::app_dir_unknown(),
            Some(messages::app_dir_fix()),
        )),
    }

    if let Some(dir) = cfg.app_dir() {
        for def in detect::GAMES {
            for name in [
                format!("activar_puente_{}.sh", def.id),
                format!("arrancar_todo_{}.sh", def.id),
            ] {
                let script = dir.join(&name);
                if !script.is_file() {
                    checks.push(fail(
                        format!("Script {}", name),
                        messages::script_missing(&name),
                        None,
                    ));
                } else if !util::is_executable(&script) {
                    checks.push(warn(
                        format!("Script {}", name),
                        "no es ejecutable",
                        Some(format!("chmod +x {}", script.display())),
                    ));
                }
            }
        }
    }

    // --- Juegos (instalados o no) ---
    for def in detect::GAMES {
        let Some(game) = cfg.games.get(def.id) else {
            checks.push(warn(
                def.name,
                "no esta en la configuracion",
                Some(messages::detect_fix()),
            ));
            continue;
        };

        if !game.installed {
            checks.push(warn(
                def.name,
                messages::game_missing(def.name),
                Some(messages::detect_fix()),
            ));
            continue;
        }

        let mut missing = Vec::new();
        if !game.path.as_deref().map(Path::is_dir).unwrap_or(false) {
            missing.push("ruta del juego");
        }
        if !game.exe.as_deref().map(Path::is_file).unwrap_or(false) {
            missing.push("ejecutable");
        }
        if !game.prefix.as_deref().map(Path::is_dir).unwrap_or(false) {
            missing.push("prefijo de Proton");
        }
        if !game.wine.as_deref().map(Path::is_file).unwrap_or(false) {
            missing.push("wine de Proton");
        }

        if missing.is_empty() {
            let proton = game
                .proton
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "Proton".to_string());
            checks.push(ok(def.name, format!("{proton} · prefijo listo")));
        } else {
            checks.push(warn(
                def.name,
                messages::game_incomplete(def.name, &missing),
                Some(messages::detect_fix()),
            ));
        }
    }

    // --- SimHub / CrewChief ---
    match &cfg.apps.bottle {
        Some(bottle) if bottle.is_dir() => {
            let origin = detect::bottles::origin_of(bottle)
                .map(|origin| format!(" · Bottles {}", origin.label()))
                .unwrap_or_default();
            checks.push(ok(
                "Botella de SimHub",
                format!("{}{origin}", bottle.display()),
            ))
        }
        Some(bottle) => checks.push(fail(
            "Botella de SimHub",
            messages::bottle_gone(bottle),
            Some(messages::bottle_fix()),
        )),
        None => checks.push(warn(
            "Botella de SimHub",
            messages::bottle_missing(),
            Some(messages::bottle_fix()),
        )),
    }

    // --- Bottles nativo o Flatpak ---
    let origins = detect::bottles::installed_origins();
    let mut extra_paths: Vec<PathBuf> = detect::steam::libraries();
    for def in detect::GAMES {
        if let Some(game) = cfg.games.get(def.id) {
            if let Some(path) = &game.path {
                extra_paths.push(path.clone());
            }
        }
    }
    let flatpak = detect::bottles::flatpak_state(&extra_paths);

    if flatpak.available {
        if flatpak.needs_user_install {
            checks.push(warn(
                "Bottles",
                "solo esta la instalacion de sistema (admin); falta la de usuario",
                Some(messages::bottles_user_fix()),
            ));
        } else if flatpak.user {
            checks.push(ok("Bottles", "instalacion Flatpak de usuario"));
        } else if flatpak.system {
            checks.push(ok("Bottles", "instalacion Flatpak de sistema"));
        }
    }

    if origins.contains(&detect::bottles::Origin::Native) {
        checks.push(ok(
            "Bottles nativa",
            format!(
                "tambien hay una instalacion del sistema en {}",
                detect::bottles::native_base().display()
            ),
        ));
    }
    if !flatpak.available && !origins.contains(&detect::bottles::Origin::Native) {
        checks.push(warn(
            "Bottles",
            "no se encontro Bottles (ni nativo ni Flatpak)",
            Some(messages::bottles_install_fix()),
        ));
    }

    // Con Bottles Flatpak, los permisos tienen que cubrir el home, /etc, las
    // bibliotecas de Steam de otros discos y el pad HF8 (devices=all).
    if flatpak.available && (flatpak.user || flatpak.system) {
        if flatpak.permissions_ok {
            checks.push(ok(
                "Permisos Bottles Flatpak",
                "home, host-etc, Steam y devices=all",
            ));
        } else {
            let only_libraries = !flatpak.missing_paths.is_empty()
                && !flatpak.missing_home
                && !flatpak.missing_host_etc
                && !flatpak.missing_devices;
            if only_libraries {
                checks.push(warn(
                    crate::tr!(
                        "Nueva biblioteca de Steam",
                        "Nova biblioteca de Steam",
                        "New Steam library"
                    ),
                    crate::tr!(
                        "se ha detectado una nueva biblioteca de Steam: {}; Bottles necesita permiso para acceder. Aplica los permisos y reinicia Bottles/SimHub.",
                        "s'ha detectat una nova biblioteca de Steam: {}; Bottles necessita permís per accedir-hi. Aplica els permisos i reinicia Bottles/SimHub.",
                        "a new Steam library was detected: {}; Bottles needs permission to access it. Apply the permissions and restart Bottles/SimHub.",
                        flatpak.missing_paths.join(", ")
                    ),
                    Some(messages::flatpak_permissions_fix()),
                ));
            } else {
                let mut missing = Vec::new();
                if flatpak.missing_home {
                    missing.push("home".to_string());
                }
                if flatpak.missing_host_etc {
                    missing.push("host-etc".to_string());
                }
                if flatpak.missing_devices {
                    missing.push("devices=all (pad HF8)".to_string());
                }
                missing.extend(flatpak.missing_paths.clone());
                checks.push(warn(
                    "Permisos Bottles Flatpak",
                    format!("faltan permisos: {}", missing.join(", ")),
                    Some(messages::flatpak_permissions_fix()),
                ));
            }
        }
    }

    checks.extend(check_wine(cfg));

    // --- Enlaces de Steam dentro de la botella ---
    let (linked, missing, manual) = crate::detect::links::check(cfg);
    if linked + missing > 0 {
        if let Some(path) = manual {
            checks.push(warn(
                "Enlaces de Steam",
                format!(
                    "hay un enlace manual {}; redetecta para migrarlo a enlaces por juego",
                    path.display()
                ),
                Some(messages::detect_fix()),
            ));
        } else if missing > 0 {
            checks.push(warn(
                "Enlaces de Steam",
                format!("{linked} juego(s) enlazados, {missing} sin enlace en la botella"),
                Some(messages::detect_fix()),
            ));
        } else {
            checks.push(ok(
                "Enlaces de Steam",
                format!("{linked} juego(s) visibles en la botella"),
            ));
        }
    }

    if !cfg
        .apps
        .simhub_exe
        .as_deref()
        .map(Path::is_file)
        .unwrap_or(false)
    {
        checks.push(warn(
            "SimHub",
            "no se encontro SimHubWPF.exe",
            Some(messages::set_fix("apps.simhub_exe", "<ruta>")),
        ));
    }

    if !cfg
        .apps
        .crewchief_exe
        .as_deref()
        .map(Path::is_file)
        .unwrap_or(false)
    {
        checks.push(warn(
            "CrewChief",
            "no se encontro CrewChiefV4.exe",
            Some(messages::set_fix("apps.crewchief_exe", "<ruta>")),
        ));
    }

    // --- HF8 (opcional) ---
    if cfg.hf8.enabled && !cfg.devices.scan_usb {
        checks.push(ok(
            "Dispositivos USB",
            "escaneo desactivado en Opciones; no se busca el pad",
        ));
    } else if cfg.hf8.enabled {
        let devices = state::hf8_devices();
        let matching_device = devices.iter().find(|device| {
            device.vendor.eq_ignore_ascii_case(&cfg.hf8.vendor_id)
                && device.product.eq_ignore_ascii_case(&cfg.hf8.product_id)
        });

        match matching_device {
            Some(device) => checks.push(ok(
                "HF8 conectado",
                format!(
                    "{} · {}",
                    device.dev.display(),
                    if device.name.is_empty() {
                        "ForceFeel"
                    } else {
                        device.name.as_str()
                    }
                ),
            )),
            None => checks.push(warn(
                "HF8 conectado",
                messages::hf8_missing(&cfg.hf8.vendor_id, &cfg.hf8.product_id),
                None,
            )),
        }

        if let Some(bottle) = &cfg.apps.bottle {
            let links = state::com_links(bottle);
            let wanted = cfg.hf8.com_port.to_lowercase();
            match links
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&wanted))
            {
                Some((_, target)) => match matching_device {
                    Some(device) if target == &device.dev => checks.push(ok(
                        cfg.hf8.com_port.clone(),
                        format!("-> {}", target.display()),
                    )),
                    Some(device) => checks.push(warn(
                        cfg.hf8.com_port.clone(),
                        format!(
                            "apunta a {} en vez de {}",
                            target.display(),
                            device.dev.display()
                        ),
                        Some(format!("{} hf8", messages::cli())),
                    )),
                    None => checks.push(warn(
                        cfg.hf8.com_port.clone(),
                        format!("-> {}", target.display()),
                        None,
                    )),
                },
                None => checks.push(warn(
                    cfg.hf8.com_port.clone(),
                    messages::com_missing(&cfg.hf8.com_port),
                    Some(format!("{} hf8", messages::cli())),
                )),
            }
        }

        if let Some(device) = matching_device {
            let holders = state::holders(&device.dev);
            if holders.iter().any(|(_, comm)| comm.contains("SimHub")) {
                checks.push(ok(
                    "SimHub y el pad",
                    "SimHub tiene abierto el dispositivo",
                ));
            } else if holders.is_empty() {
                checks.push(warn(
                    "SimHub y el pad",
                    "nadie tiene abierto el dispositivo",
                    Some("abre SimHub o lanza el juego".into()),
                ));
            } else {
                let names: Vec<String> = holders
                    .iter()
                    .map(|(pid, comm)| format!("{comm} ({pid})"))
                    .collect();
                checks.push(warn(
                    "SimHub y el pad",
                    format!("abierto por: {}", names.join(", ")),
                    None,
                ));
            }
        }
    } else {
        checks.push(ok("HF8", "desactivado en la configuracion"));
    }

    // --- Binarios del puente ---
    if let Some(dir) = cfg.app_dir() {
        for def in detect::GAMES {
            if def.id == "dirt2" {
                for name in ["dirt2_udp_relay.py", "dirt2_process_stub.exe"] {
                    let file = dir.join("bridge").join(name);
                    if !file.is_file() {
                        checks.push(warn(
                            "Relay DiRT Rally 2.0",
                            format!("falta {}", file.display()),
                            None,
                        ));
                    }
                }
                continue;
            }
            if def.id == "beamng" {
                let stub = dir.join("bridge/beamng_process_stub.exe");
                if !stub.is_file() {
                    checks.push(warn(
                        "Puente BeamNG.drive",
                        format!("falta {}", stub.display()),
                        None,
                    ));
                }
                continue;
            }
            let exe = dir
                .join("bridge")
                .join(format!("{}_shm_mirror.exe", def.id));
            if !exe.is_file() {
                checks.push(warn(
                    format!("Espejo {}", def.id),
                    format!("falta {}", exe.display()),
                    None,
                ));
            }
            if def.id == "ets2" {
                for name in ["ets2_process_stub.exe", "libscs-telemetry.so"] {
                    let file = dir.join("bridge").join(name);
                    if !file.is_file() {
                        checks.push(warn(
                            "Puente Euro Truck Simulator 2",
                            format!("falta {}", file.display()),
                            None,
                        ));
                    }
                }
            }
        }
    }

    // --- Plugin SCS de Euro Truck Simulator 2 (version nativa) ---
    if let Some(game) = cfg.games.get("ets2") {
        if game.installed && game.exe.as_ref().map(|exe| exe.is_file()).unwrap_or(false) {
            let def = detect::game_def("ets2").expect("ets2 esta en GAMES");
            let plugin = game
                .path
                .as_ref()
                .and_then(|dir| def.plugin_path(dir));
            match plugin {
                Some(path) if path.is_file() => {
                    checks.push(ok("Plugin SCS", "instalado en el juego"));
                }
                Some(path) => {
                    checks.push(warn(
                        "Plugin SCS",
                        format!(
                            "falta {}; ETS2 no publicara telemetria",
                            path.display()
                        ),
                        None,
                    ));
                }
                None => {}
            }
        }
    }

    // --- UDP de DiRT Rally 2.0 (extradata=3 para SimHub y CrewChief) ---
    if let Some(game) = cfg.games.get("dirt2") {
        if game.installed {
            if let Some(prefix) = game.prefix.as_ref() {
                let base = prefix.join(
                    "drive_c/users/steamuser/Documents/My Games/DiRT Rally 2.0/hardwaresettings",
                );
                let mut checked = 0;
                let mut wrong = 0;
                for name in ["hardware_settings_config.xml", "hardware_settings_config_vr.xml"] {
                    let path = base.join(name);
                    if !path.is_file() {
                        continue;
                    }
                    checked += 1;
                    let text = std::fs::read_to_string(&path).unwrap_or_default();
                    let udp = text
                        .lines()
                        .find(|line| line.trim_start().starts_with("<udp"))
                        .unwrap_or("");
                    let ok_line = udp.contains("enabled=\"true\"")
                        && udp.contains("extradata=\"3\"")
                        && udp.contains("port=\"20777\"");
                    if !ok_line {
                        wrong += 1;
                    }
                }
                if checked == 0 {
                    checks.push(warn(
                        "DiRT Rally 2.0 UDP",
                        "no se encontro hardware_settings_config.xml (abre el juego una vez)",
                        None,
                    ));
                } else if wrong == 0 {
                    checks.push(ok(
                        "DiRT Rally 2.0 UDP",
                        "extradata=3 en 127.0.0.1:20777",
                    ));
                } else {
                    checks.push(warn(
                        "DiRT Rally 2.0 UDP",
                        "hace falta enabled=true, extradata=3 y puerto 20777",
                        Some("Pulsa Lanzar con el juego cerrado".to_string()),
                    ));
                }
            }
        }
    }

    // --- SimHub WebSocket y firewall (moviles por wifi) ---
    if state::tcp_listening(8888) {
        checks.push(ok("SimHub WebSocket", "escuchando en TCP 8888"));
    } else {
        checks.push(warn(
            "SimHub WebSocket",
            "SimHub no escucha en TCP 8888 (abre SimHub o revisa su configuracion)",
            None,
        ));
    }

    let manual_port = manual_port_check(cfg, 8888);
    match state::firewall_port(8888) {
        state::FirewallState::Allowed => {
            checks.push(ok("Firewall 8888", "permite TCP 8888"))
        }
        state::FirewallState::Inactive => {
            checks.push(ok("Firewall 8888", "no hay firewall activo"))
        }
        state::FirewallState::Blocked => checks.push(warn(
            "Firewall 8888",
            "el firewall bloquea TCP 8888 para los dispositivos de la wifi",
            Some(messages::firewall_fix(8888)),
        )),
        state::FirewallState::Unknown => {
            if manual_port.is_none() {
                checks.push(warn(
                    "Firewall 8888",
                    "no se pudo comprobar el firewall (prueba ufw o firewalld)",
                    Some(messages::firewall_fix(8888)),
                ));
            }
        }
    }

    if let Some(check) = manual_port {
        checks.push(check);
    }

    // --- Puertos y locks ---
    for def in detect::GAMES {
        // Sin puerto propio (ETS2 y BeamNG): no hay nada que comprobar.
        if def.port == 0 {
            continue;
        }
        let running = state::any_matching(&def.bridge_needle());
        let in_use = state::udp_in_use(def.port);
        let title = format!("Puerto {}", def.port);

        if running && !in_use {
            checks.push(warn(
                title,
                "el espejo corre pero el puerto no responde",
                None,
            ));
        } else if !running && in_use {
            checks.push(fail(
                title,
                messages::port_busy(def.port),
                Some(messages::port_busy_fix(def.port)),
            ));
        } else {
            checks.push(ok(
                title,
                if running { "en uso por el espejo" } else { "libre" },
            ));
        }

        if let Some(game) = cfg.games.get(def.id) {
            if let Some(lock) = state::arranque_lock(&game.run_dir) {
                let holders = state::fd_holders(&lock);
                // Un lock sin duenos es el estado normal: flock deja el fichero
                // en disco. Solo se informa mientras hay un arranque en curso.
                if !holders.is_empty() {
                    checks.push(ok(
                        format!("Lock {}", def.id),
                        format!("arranque en curso ({} procesos)", holders.len()),
                    ));
                }
            }
        }
    }

    for check in &mut checks {
        check.title = crate::i18n::text(&check.title);
        check.detail = crate::i18n::text(&check.detail);
        if let Some(fix) = check.fix.as_mut() {
            *fix = crate::i18n::text(fix);
        }
    }
    checks
}

/// Comprueba el wine configurado y el que se resolveria por fallback.
fn manual_port_check(cfg: &Config, port: u16) -> Option<Check> {
    let path = cfg.bridge.run_base.join(format!("comprobar_{port}.txt"));
    let text = std::fs::read_to_string(path).ok()?;
    let mut status = String::new();
    let mut detail = String::new();
    let mut fix = String::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("status=") {
            status = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("detail=") {
            detail = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("fix=") {
            fix = value.trim().to_string();
        }
    }
    if status.is_empty() {
        return None;
    }
    if status == "ok" {
        Some(ok("Puerto 8888 (comprobado)", detail))
    } else {
        Some(warn(
            "Puerto 8888 (comprobado)",
            detail,
            if fix.is_empty() { None } else { Some(fix) },
        ))
    }
}

fn check_wine(cfg: &Config) -> Vec<Check> {
    let mut checks = Vec::new();
    let source = cfg
        .apps
        .wine_source
        .as_deref()
        .and_then(detect::bottles::WineSource::parse);

    match cfg.apps.wine.as_deref() {
        Some(wine) if wine.is_file() => {
            if source == Some(detect::bottles::WineSource::System) {
                checks.push(warn(
                    "Wine de Bottles",
                    messages::wine_system(wine),
                    Some(messages::wine_fix()),
                ));
            } else {
                let label = source.map(|source| source.label()).unwrap_or("detectado");
                checks.push(ok(
                    "Wine de Bottles",
                    format!("{} · {}", wine.display(), label),
                ));

                if let Some(bottle_path) = cfg.apps.bottle.as_deref() {
                    if bottle_path.is_dir() {
                        let bottle = detect::bottles::from_path(bottle_path);
                        let resolution =
                            detect::bottles::resolve_wine(Some(&bottle), Some(wine));
                        if let Some(note) = resolution.note {
                            checks.push(warn("Wine y botella", note, Some(messages::wine_fix())));
                        }
                    }
                }
            }
        }
        Some(wine) => {
            let detail = format!("el wine configurado no existe: {}", wine.display());
            if source == Some(detect::bottles::WineSource::Manual) {
                checks.push(fail("Wine de Bottles", detail, Some(messages::wine_fix())));
            } else {
                checks.push(warn("Wine de Bottles", detail, Some(messages::wine_fix())));
            }
        }
        None => {
            if let Some(bottle_path) = cfg.apps.bottle.as_deref() {
                if bottle_path.is_dir() {
                    let bottle = detect::bottles::from_path(bottle_path);
                    let searched = detect::bottles::wine_search_paths(Some(&bottle));
                    checks.push(warn(
                        "Wine de Bottles",
                        messages::wine_missing(&searched),
                        Some(messages::wine_fix()),
                    ));
                } else {
                    checks.push(warn(
                        "Wine de Bottles",
                        "no hay una botella valida de la que resolver el wine",
                        Some(messages::bottle_fix()),
                    ));
                }
            } else {
                checks.push(warn(
                    "Wine de Bottles",
                    "no hay botella configurada",
                    Some(messages::bottle_fix()),
                ));
            }
        }
    }

    checks
}

/// Borra los locks de arranque que ya no tiene abiertos ningun proceso.
pub fn fix_stale_locks(cfg: &Config) -> Vec<std::path::PathBuf> {
    let mut removed = Vec::new();
    for def in detect::GAMES {
        let Some(game) = cfg.games.get(def.id) else {
            continue;
        };
        let Some(lock) = state::arranque_lock(&game.run_dir) else {
            continue;
        };
        if state::fd_holders(&lock).is_empty() && std::fs::remove_file(&lock).is_ok() {
            removed.push(lock);
        }
    }
    removed
}

/// Imprime los resultados. Devuelve el numero de fallos.
pub fn print(checks: &[Check]) -> usize {
    let mut failures = 0;

    for check in checks {
        let (tag, paint): (&str, fn(&str) -> String) = match check.level {
            Level::Ok => ("OK", util::green),
            Level::Warn => ("AVISO", util::yellow),
            Level::Fail => {
                failures += 1;
                ("FALLO", util::red)
            }
        };
        println!(
            "  {} {:<28} {}",
            paint(&format!("{tag:<6}")),
            check.title,
            util::dim(&check.detail)
        );
        if let Some(fix) = &check.fix {
            println!("         {:<28} {}", "", util::cyan(&format!("-> {fix}")));
        }
    }

    failures
}

/// Resumen para la GUI.
pub fn summary(checks: &[Check]) -> (usize, usize) {
    let failures = checks
        .iter()
        .filter(|check| check.level == Level::Fail)
        .count();
    let warnings = checks
        .iter()
        .filter(|check| check.level == Level::Warn)
        .count();
    (failures, warnings)
}

/// Ruta de la configuracion activa, para mensajes.
pub fn config_path() -> PathBuf {
    config::config_path()
}

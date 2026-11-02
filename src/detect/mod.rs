//! Orquesta la deteccion y construye la configuracion.

pub mod bottles;
pub mod links;
pub mod steam;

use crate::config::{Apps, Config, Game};
use crate::state;
use crate::util;
use std::path::{Path, PathBuf};

pub struct GameDef {
    pub id: &'static str,
    pub name: &'static str,
    pub app_id: u32,
    /// Ejecutables candidatos, en orden de preferencia.
    pub exes: &'static [&'static str],
    /// Subcarpeta de RUN_DIR usada por el puente.
    pub run_subdir: &'static str,
    pub port: u16,
    /// Aplicaciones que usa este juego (`simhub`, `crewchief`).
    pub apps: &'static [&'static str],
    /// El juego corre nativo en Linux: su telemetria no esta en un prefijo Wine.
    pub native: bool,
    /// Plugin que instala el puente, relativo a la carpeta del juego.
    pub plugin: Option<&'static str>,
}

/// Apps que usan todos los simuladores menos los camiones.
pub const APPS_UDP: &[&str] = &["crewchief", "simhub"];
/// Juegos con un solo lector en SimHub (camiones y BeamNG): sin CrewChief.
pub const APPS_SIMHUB_ONLY: &[&str] = &["simhub"];

pub const GAMES: &[GameDef] = &[
    GameDef {
        id: "lmu",
        name: "Le Mans Ultimate",
        app_id: 2_399_420,
        exes: &["Le Mans Ultimate.exe"],
        run_subdir: "lmu_mirror",
        port: 8772,
        apps: APPS_UDP,
        native: false,
        plugin: None,
    },
    GameDef {
        id: "ams2",
        name: "Automobilista 2",
        app_id: 1_066_890,
        exes: &["AMS2.exe", "AMS2AVX.exe"],
        run_subdir: "ams2_mirror",
        port: 8773,
        apps: APPS_UDP,
        native: false,
        plugin: None,
    },
    GameDef {
        id: "acevo",
        name: "Assetto Corsa EVO",
        app_id: 3_058_630,
        exes: &["AssettoCorsaEVO.exe"],
        run_subdir: "acevo_mirror",
        port: 8774,
        apps: APPS_UDP,
        native: false,
        plugin: None,
    },
    GameDef {
        id: "assetto",
        name: "Assetto Corsa",
        app_id: 244_210,
        exes: &["acs.exe", "AssettoCorsa.exe"],
        run_subdir: "assetto_mirror",
        port: 8775,
        apps: APPS_UDP,
        native: false,
        plugin: None,
    },
    GameDef {
        id: "rf2",
        name: "rFactor 2",
        app_id: 365_960,
        exes: &["Bin64/rFactor2.exe", "rFactor2.exe", "Bin32/rFactor2.exe"],
        run_subdir: "rf2_mirror",
        port: 8776,
        apps: APPS_UDP,
        native: false,
        plugin: None,
    },
    GameDef {
        id: "dirt2",
        name: "DiRT Rally 2.0",
        app_id: 690_790,
        exes: &["dirtrally2.exe"],
        run_subdir: "dirt2_udp",
        port: 20777,
        apps: APPS_UDP,
        native: false,
        plugin: None,
    },
    GameDef {
        id: "ets2",
        name: "Euro Truck Simulator 2",
        app_id: 227_300,
        // Version nativa Linux primero; si algun dia se pasa a Proton, el
        // ejecutable Windows queda como alternativa.
        exes: &["bin/linux_x64/eurotrucks2", "bin/win_x64/eurotrucks2.exe"],
        run_subdir: "ets2_bridge",
        // Sin puerto: la telemetria va por memoria compartida, no por UDP.
        port: 0,
        apps: APPS_SIMHUB_ONLY,
        native: true,
        plugin: Some("bin/linux_x64/plugins/libscs-telemetry.so"),
    },
    GameDef {
        id: "beamng",
        name: "BeamNG.drive",
        app_id: 284_160,
        // Version nativa Linux primero; el .exe de Windows queda como respaldo.
        exes: &["BinLinux/BeamNG.drive.x64", "Bin64/BeamNG.drive.x64.exe"],
        run_subdir: "beamng_udp",
        // Sin puerto propio: BeamNG manda OutGauge a SimHub, que es quien escucha.
        port: 0,
        apps: APPS_SIMHUB_ONLY,
        native: true,
        plugin: None,
    },
];

pub fn game_def(id: &str) -> Option<&'static GameDef> {
    GAMES.iter().find(|game| game.id.eq_ignore_ascii_case(id))
}

impl GameDef {
    /// Nombre del proceso que representa el puente de este juego.
    pub fn bridge_needle(&self) -> String {
        match self.id {
            // Puentes por UDP: el proceso es el relay o el stub del juego.
            "dirt2" => "dirt2_udp_relay".to_string(),
            "beamng" => "beamng_udp".to_string(),
            _ => format!("{}_shm_mirror", self.id),
        }
    }

    /// ¿Este juego usa la app indicada (`simhub`, `crewchief`)?
    pub fn has_app(&self, app: &str) -> bool {
        self.apps.iter().any(|name| name.eq_ignore_ascii_case(app))
    }

    /// Ruta del plugin que instala el puente, dentro de la carpeta del juego.
    pub fn plugin_path(&self, game_dir: &Path) -> Option<PathBuf> {
        self.plugin.map(|name| game_dir.join(name))
    }
}

/// Rutas donde historicamente vivia Simhub_Linux_APP.
pub fn legacy_app_dirs() -> [PathBuf; 3] {
    let home = util::home();
    [
        home.join("Escritorio/SIMHUB_LINUX/Simhub_Linux_APP"),
        home.join("SIMHUB_LINUX/Simhub_Linux_APP"),
        home.join("Simhub_Linux_APP"),
    ]
}

/// Comprueba si una carpeta es una de las rutas historicas de SIMHUB_LINUX.
///
/// Sirve para distinguir una ruta detectada automaticamente por versiones
/// anteriores de una carpeta elegida a mano por el usuario.
pub fn is_legacy_app_dir(dir: &Path) -> bool {
    legacy_app_dirs().iter().any(|candidate| candidate == dir)
}

/// Carpeta de los scripts de SIMHUB_LINUX.
///
/// Prioridad: `ETOL_APP_DIR`, la copia incluida en el AppImage, las
/// rutas historicas y, por ultimo, el payload de la configuracion.
pub fn find_app_dir() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("ETOL_APP_DIR") {
        let candidate = PathBuf::from(value);
        if is_app_dir(&candidate) {
            return Some(candidate);
        }
    }

    if let Some(candidate) = APP_DIR_OVERRIDE.get() {
        if is_app_dir(candidate) {
            return Some(candidate.clone());
        }
    }

    legacy_app_dirs()
        .into_iter()
        .chain([crate::config::root().join("payload")])
        .find(|candidate| is_app_dir(candidate))
}

/// Comprueba que una carpeta contiene los scripts del puente.
pub fn is_app_dir(dir: &Path) -> bool {
    dir.join("arrancar_todo_lmu.sh").is_file()
}

/// Fija la carpeta instalada por la interfaz (payload del AppImage).
///
/// Se usa en lugar de una variable de entorno para no tocar el entorno del
/// proceso desde varios hilos.
pub fn set_app_dir_override(dir: PathBuf) {
    let _ = APP_DIR_OVERRIDE.set(dir);
}

/// Carpeta fijada por la interfaz, si ya se instalo el payload.
pub fn app_dir_override() -> Option<&'static PathBuf> {
    APP_DIR_OVERRIDE.get()
}

static APP_DIR_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();


pub fn detect() -> Config {
    let previous = Config::load().ok();
    let scan_usb = previous
        .as_ref()
        .map(|cfg| cfg.devices.scan_usb)
        .unwrap_or(true);

    let mut cfg = Config::default();
    if let Some(prev) = &previous {
        cfg.hf8.enabled = prev.hf8.enabled;
        cfg.hf8.com_port = prev.hf8.com_port.clone();
        cfg.hf8.vendor_id = prev.hf8.vendor_id.clone();
        cfg.hf8.product_id = prev.hf8.product_id.clone();
        cfg.logs = prev.logs.clone();
        cfg.devices = prev.devices.clone();
        cfg.daemon = prev.daemon.clone();
        cfg.overlays = prev.overlays.clone();
        cfg.ui = prev.ui.clone();
    }
    let _ = cfg.ensure_run_base();
    let steam_apps = steam::apps();

    for def in GAMES {
        let mut game = Game {
            id: def.id.to_string(),
            name: def.name.to_string(),
            app_id: def.app_id,
            run_dir: cfg.bridge.run_base.join(def.run_subdir),
            ..Game::default()
        };

        if let Some(entry) = steam_apps.iter().find(|app| app.app_id == def.app_id) {
            game.installed = entry.install_dir.is_dir();
            game.path = Some(entry.install_dir.clone());
            game.exe = def
                .exes
                .iter()
                .map(|exe| entry.install_dir.join(exe))
                .find(|candidate| candidate.is_file());

            game.prefix = steam::compat_prefix(def.app_id, &entry.library);

            if let Some(tool) = steam::compat_tool(def.app_id, &entry.library) {
                game.proton = steam::resolve_proton(&tool);
            }
            if let Some(proton) = &game.proton {
                let wine = proton.join("files/bin/wine");
                if wine.is_file() {
                    game.wine = Some(wine);
                }
            }
        }

        cfg.games.insert(def.id.to_string(), game);
    }

    if let Some(apps) = detect_apps() {
        cfg.apps = apps;
    }

    cfg.bridge.app_dir = find_app_dir();
    cfg.hf8.device = if scan_usb {
        state::hf8_devices().first().map(|device| device.dev.clone())
    } else {
        None
    };

    let links = links::ensure(&cfg);
    links::log_report(&links);

    cfg
}

/// Detecta SimHub/CrewChief y el wine de la botella elegida.
///
/// Cuando hay varias botellas con SimHub la eleccion es determinista:
/// primero la que tambien tiene CrewChiefV4.exe, despues la de `Update_Date`
/// mas reciente y, por ultimo, el nombre. El motivo queda en el log.
pub fn detect_apps() -> Option<Apps> {
    let mut candidates: Vec<(bottles::Bottle, PathBuf, Option<PathBuf>)> = bottles::all()
        .into_iter()
        .filter_map(|bottle| {
            let simhub = bottles::find_program(&bottle, "SimHubWPF.exe")?;
            let crewchief = bottles::find_program(&bottle, "CrewChiefV4.exe");
            Some((bottle, simhub, crewchief))
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| {
        b.2.is_some()
            .cmp(&a.2.is_some())
            .then_with(|| b.0.update_date.cmp(&a.0.update_date))
            .then_with(|| a.0.name.cmp(&b.0.name))
    });

    let (bottle, simhub, crewchief) = candidates.remove(0);
    let reason = if candidates.is_empty() {
        "unica botella con SimHub".to_string()
    } else {
        format!(
            "{} y Update_Date {}",
            if crewchief.is_some() {
                "tiene CrewChiefV4"
            } else {
                "sin CrewChiefV4"
            },
            if bottle.update_date.is_empty() {
                "desconocida"
            } else {
                bottle.update_date.as_str()
            }
        )
    };
    eprintln!(
        "etol: botella elegida '{}' ({})",
        bottle.name, reason
    );

    let resolution = bottles::resolve_wine(Some(&bottle), None);
    Some(Apps {
        bottle: Some(bottle.path.clone()),
        bottle_name: Some(bottle.name),
        prefix: Some(bottle.path),
        wine: resolution.wine,
        wine_source: resolution.source.map(|source| source.as_str().to_string()),
        simhub_exe: Some(simhub),
        crewchief_exe: crewchief,
    })
}

/// Conserva un wine elegido a mano al volver a detectar.
/// Conserva las elecciones manuales al volver a detectar.
///
/// - Un wine elegido a mano que siga existiendo.
/// - Una carpeta de scripts propia (ni la del payload ni una ruta historica).
pub fn preserve_manual_wine(new: &mut Config) {
    let Ok(old) = Config::load() else {
        return;
    };

    if old.apps.wine_source.as_deref() == Some("manual") {
        if let Some(wine) = &old.apps.wine {
            if wine.is_file() {
                new.apps.wine = Some(wine.clone());
                new.apps.wine_source = Some("manual".to_string());
            }
        }
    }

    if let Some(dir) = &old.bridge.app_dir {
        let payload_dir = crate::payload::dest();
        if dir != &payload_dir && is_app_dir(dir) && !is_legacy_app_dir(dir) {
            new.bridge.app_dir = Some(dir.clone());
        }
    }
}

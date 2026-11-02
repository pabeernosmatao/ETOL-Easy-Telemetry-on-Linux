//! Modelo de configuracion y persistencia en TOML.

use crate::i18n;
use crate::util;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Raiz de datos de la aplicacion.
///
/// - Si existe `ETOL_HOME`, se usa esa ruta.
/// - Si corre empaquetado (AppImage/AppDir), `~/.config/etol`, porque el
///   AppImage es de solo lectura.
/// - Si corre desde el arbol de fuentes, la carpeta del proyecto (todo queda
///   junto, como hasta ahora). Se detecta subiendo desde el ejecutable, sin
///   incrustar la ruta del desarrollador en el binario.
pub fn root() -> PathBuf {
    if let Some(value) = std::env::var_os("ETOL_HOME") {
        return PathBuf::from(value);
    }

    if crate::payload::running_bundled() {
        return config_home().join("etol");
    }

    if let Some(dir) = dev_root() {
        return dir;
    }

    config_home().join("etol")
}

/// Raiz del proyecto cuando el binario vive dentro de `target/`.
fn dev_root() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("ETOL_DEV_ROOT") {
        return Some(PathBuf::from(value));
    }

    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?;
    for _ in 0..5 {
        if dir.join("Cargo.toml").is_file() && dir.join("src-tauri/Cargo.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
    None
}

fn config_home() -> PathBuf {
    if let Some(value) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(value);
    }
    util::home().join(".config")
}

pub fn config_path() -> PathBuf {
    root().join("config").join("config.toml")
}

pub fn generated_dir() -> PathBuf {
    root().join("generated")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub meta: Meta,
    pub apps: Apps,
    pub hf8: Hf8,
    pub bridge: Bridge,
    pub logs: Logs,
    pub devices: Devices,
    pub daemon: Daemon,
    pub overlays: Overlays,
    pub ui: Ui,
    pub games: BTreeMap<String, Game>,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub version: u32,
    pub generated: String,
    pub tool: String,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            version: 1,
            generated: util::now_utc(),
            tool: format!("etol {}", env!("CARGO_PKG_VERSION")),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Apps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottle: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottle_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wine: Option<PathBuf>,
    /// Origen del wine: bottle-runner, compat-tool, system o manual.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wine_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simhub_exe: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crewchief_exe: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Hf8 {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<PathBuf>,
    pub com_port: String,
    pub vendor_id: String,
    pub product_id: String,
}

impl Default for Hf8 {
    fn default() -> Self {
        Self {
            enabled: true,
            device: None,
            com_port: "COM33".to_string(),
            vendor_id: "04d8".to_string(),
            product_id: "ee96".to_string(),
        }
    }
}

/// Directorio de trabajo por defecto del puente.
pub const DEFAULT_RUN_BASE: &str = "/tmp/etol";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Logs {
    /// Captura de logs de la aplicacion y del puente.
    pub enabled: bool,
    /// Dias que se conservan los logs (1..=7).
    pub retention_days: u32,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Devices {
    /// Busca dispositivos USB (HF8 y futuros) al detectar/arrancar.
    pub scan_usb: bool,
}

impl Default for Devices {
    fn default() -> Self {
        Self { scan_usb: true }
    }
}

/// Vigilancia automatica de simuladores mientras ETOL esta abierto.
///
/// Cada juego puede arrancar CrewChief, SimHub, ambos o ninguno (solo puente).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Daemon {
    pub enabled: bool,
    /// Al cerrar el simulador se cierran siempre CrewChief y SimHub.
    pub close_apps: bool,
    pub games: BTreeMap<String, DaemonGame>,
}

impl Default for Daemon {
    fn default() -> Self {
        Self {
            enabled: false,
            close_apps: true,
            games: BTreeMap::new(),
        }
    }
}

impl Daemon {
    /// Apps elegidas para un juego; si no hay entrada, ambas.
    ///
    /// Se recortan a las que el juego admite: Euro Truck Simulator 2, por
    /// ejemplo, solo usa SimHub.
    pub fn game(&self, id: &str) -> DaemonGame {
        let mut choice = self.games.get(id).copied().unwrap_or_default();
        if let Some(def) = crate::detect::game_def(id) {
            choice.crewchief &= def.has_app("crewchief");
            choice.simhub &= def.has_app("simhub");
        }
        choice
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonGame {
    pub crewchief: bool,
    pub simhub: bool,
}

impl Default for DaemonGame {
    fn default() -> Self {
        Self {
            crewchief: true,
            simhub: true,
        }
    }
}

/// Overlays de SimHub por encima del simulador.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Overlays {
    /// Activa la regla de KWin mientras hay un simulador en marcha.
    pub kwin: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Self { kwin: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Ui {
    /// `auto`, `es`, `ca` o `en`.
    pub language: String,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            language: "auto".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Bridge {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_dir: Option<PathBuf>,
    pub run_base: PathBuf,
}

impl Default for Bridge {
    fn default() -> Self {
        Self {
            app_dir: None,
            run_base: PathBuf::from(DEFAULT_RUN_BASE),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub app_id: u32,
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proton: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wine: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe: Option<PathBuf>,
    pub run_dir: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            let cfg = Config::default();
            let _ = cfg.ensure_run_base();
            i18n::set(i18n::resolve(&cfg.ui.language));
            return Ok(cfg);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("no se pudo leer {}", path.display()))?;
        let mut cfg: Config = toml::from_str(&text)
            .with_context(|| format!("configuracion invalida en {}", path.display()))?;
        i18n::set(i18n::resolve(&cfg.ui.language));
        if cfg.meta.generated.is_empty() {
            cfg.meta.generated = util::now_utc();
        }

        // Migraciones silenciosas: run_base historico, run_dir derivados,
        // origen del wine y botellas que ya no existen.
        if cfg.normalize() {
            let _ = cfg.save();
        }

        Ok(cfg)
    }

    pub fn save(&mut self) -> Result<PathBuf> {
        self.ensure_run_base()?;
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("no se pudo crear {}", parent.display()))?;
        }
        self.meta.generated = util::now_utc();
        self.meta.tool = format!("etol {}", env!("CARGO_PKG_VERSION"));
        let text = toml::to_string_pretty(self).context("no se pudo serializar la configuracion")?;
        std::fs::write(&path, text)
            .with_context(|| format!("no se pudo escribir {}", path.display()))?;
        Ok(path)
    }

    /// Crea `run_base` con permisos 0700 si todavia no existe.
    pub fn ensure_run_base(&self) -> Result<()> {
        let base = &self.bridge.run_base;
        if base.as_os_str().is_empty() {
            return Ok(());
        }
        if !base.exists() {
            std::fs::create_dir_all(base)
                .with_context(|| format!("no se pudo crear {}", base.display()))?;
            std::fs::set_permissions(base, std::fs::Permissions::from_mode(0o700))
                .with_context(|| format!("no se pudieron ajustar los permisos de {}", base.display()))?;
        }
        Ok(())
    }

    /// Corrige valores heredados del entorno de desarrollo.
    ///
    /// Devuelve `true` si ha cambiado algo (para guardarlo).
    fn normalize(&mut self) -> bool {
        let mut changed = false;

        let base_bad = self.bridge.run_base.as_os_str().is_empty()
            || !self.bridge.run_base.is_absolute()
            || is_legacy_run_path(&self.bridge.run_base);
        if base_bad {
            self.bridge.run_base = PathBuf::from(DEFAULT_RUN_BASE);
            changed = true;
        }

        for def in crate::detect::GAMES {
            let Some(game) = self.games.get_mut(def.id) else {
                continue;
            };
            let expected = self.bridge.run_base.join(def.run_subdir);
            let bad = game.run_dir.as_os_str().is_empty()
                || is_legacy_run_path(&game.run_dir)
                || !game.run_dir.starts_with(&self.bridge.run_base);
            if bad && game.run_dir != expected {
                game.run_dir = expected;
                changed = true;
            }
            if game.id.is_empty() {
                game.id = def.id.to_string();
                changed = true;
            }
            if game.name.is_empty() {
                game.name = def.name.to_string();
                changed = true;
            }
            if game.app_id == 0 {
                game.app_id = def.app_id;
                changed = true;
            }
        }

        if self.apps.wine_source.is_none() {
            if let Some(wine) = &self.apps.wine {
                self.apps.wine_source = Some(infer_wine_source(wine).to_string());
                changed = true;
            }
        }

        // Una botella borrada o renombrada se vuelve a detectar en lugar de
        // dejar la configuracion apuntando a una ruta inexistente.
        if self
            .apps
            .bottle
            .as_deref()
            .map(|path| !path.is_dir())
            .unwrap_or(false)
        {
            if let Some(apps) = crate::detect::detect_apps() {
                if let Some(name) = &apps.bottle_name {
                    eprintln!("etol: la botella configurada ya no existe; re-detectada {name}");
                }
                self.apps = apps;
                changed = true;
            }
        }

        if !(1..=7).contains(&self.logs.retention_days) {
            self.logs.retention_days = if self.logs.retention_days == 0 {
                7
            } else {
                self.logs.retention_days.clamp(1, 7)
            };
            changed = true;
        }

        changed
    }

    /// Indica si hay que lanzar los scripts con `--no-hf8`.
    pub fn hf8_disabled(&self) -> bool {
        !self.hf8.enabled || !self.devices.scan_usb
    }

    pub fn app_dir(&self) -> Option<&Path> {
        self.bridge.app_dir.as_deref()
    }

    /// Cambia un valor concreto usando una clave con puntos.
    pub fn apply_override(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.split('.').collect();
        match parts.as_slice() {
            ["apps", "bottle"] => self.apps.bottle = Some(util::expand(value)),
            ["apps", "bottle_name"] => self.apps.bottle_name = Some(value.to_string()),
            ["apps", "prefix"] => self.apps.prefix = Some(util::expand(value)),
            ["apps", "wine"] => {
                self.apps.wine = Some(util::expand(value));
                self.apps.wine_source = Some("manual".to_string());
            }
            ["apps", "wine_source"] => self.apps.wine_source = Some(value.to_string()),
            ["apps", "simhub_exe"] => self.apps.simhub_exe = Some(util::expand(value)),
            ["apps", "crewchief_exe"] => self.apps.crewchief_exe = Some(util::expand(value)),
            ["hf8", "enabled"] => self.hf8.enabled = parse_bool(value)?,
            ["hf8", "device"] => self.hf8.device = Some(util::expand(value)),
            ["hf8", "com_port"] => self.hf8.com_port = value.trim().to_uppercase(),
            ["hf8", "vendor_id"] => self.hf8.vendor_id = value.trim().to_string(),
            ["hf8", "product_id"] => self.hf8.product_id = value.trim().to_string(),
            ["logs", "enabled"] => self.logs.enabled = parse_bool(value)?,
            ["logs", "retention_days"] => {
                let days: u32 = value
                    .trim()
                    .parse()
                    .with_context(|| format!("dias de retencion invalidos: {value}"))?;
                if !(1..=7).contains(&days) {
                    bail!("los dias de retencion deben estar entre 1 y 7");
                }
                self.logs.retention_days = days;
            }
            ["devices", "scan_usb"] => self.devices.scan_usb = parse_bool(value)?,
            ["daemon", "enabled"] => self.daemon.enabled = parse_bool(value)?,
            ["daemon", "close_apps"] => self.daemon.close_apps = parse_bool(value)?,
            ["overlays", "kwin"] => self.overlays.kwin = parse_bool(value)?,
            ["ui", "language"] => {
                let value = value.trim().to_lowercase();
                match value.as_str() {
                    "auto" | "es" | "ca" | "en" => self.ui.language = value,
                    other => bail!("idioma no valido: {other} (auto, es, ca, en)"),
                }
            }
            ["daemon", "games", id, field] => {
                let id = (*id).to_string();
                let entry = self.daemon.games.entry(id).or_default();
                match *field {
                    "crewchief" => entry.crewchief = parse_bool(value)?,
                    "simhub" => entry.simhub = parse_bool(value)?,
                    other => bail!("campo de vigilancia desconocido: {other}"),
                }
            }
            ["bridge", "app_dir"] => self.bridge.app_dir = Some(util::expand(value)),
            ["bridge", "run_base"] => self.bridge.run_base = util::expand(value),
            ["games", id, field] => {
                let id = (*id).to_string();
                let field = *field;
                let game = self.games.entry(id.clone()).or_insert_with(|| Game {
                    id: id.clone(),
                    ..Game::default()
                });
                match field {
                    "path" => game.path = Some(util::expand(value)),
                    "prefix" => game.prefix = Some(util::expand(value)),
                    "proton" => game.proton = Some(util::expand(value)),
                    "wine" => game.wine = Some(util::expand(value)),
                    "exe" => game.exe = Some(util::expand(value)),
                    "run_dir" => game.run_dir = util::expand(value),
                    "installed" => game.installed = parse_bool(value)?,
                    "app_id" => {
                        game.app_id = value
                            .trim()
                            .parse()
                            .with_context(|| format!("app_id invalido: {value}"))?
                    }
                    other => bail!("campo de juego desconocido: {other}"),
                }
            }
            _ => bail!("clave desconocida: {key}"),
        }
        Ok(())
    }
}

/// Marca del directorio de trabajo de versiones de desarrollo.
fn is_legacy_run_path(path: &Path) -> bool {
    let marker = ["open", "code"].concat();
    path.to_string_lossy().contains(&marker)
}

/// Deduce de donde salio un wine ya configurado.
pub fn infer_wine_source(path: &Path) -> &'static str {
    let text = path.to_string_lossy();
    if text.contains("/bottles/runners/") {
        "bottle-runner"
    } else if text.contains("compatibilitytools.d") {
        "compat-tool"
    } else if text == "/usr/bin/wine"
        || text == "/usr/local/bin/wine"
        || text == "/bin/wine"
        || text.ends_with("/bin/wine")
    {
        "system"
    } else {
        "manual"
    }
}

fn parse_bool(raw: &str) -> Result<bool> {
    match raw.trim().to_lowercase().as_str() {
        "1" | "true" | "si" | "sí" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => bail!("valor booleano invalido: {other}"),
    }
}

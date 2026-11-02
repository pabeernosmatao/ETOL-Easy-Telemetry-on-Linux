//! Interfaz de linea de comandos.

use crate::config::{self, Config};
use crate::detect;
use crate::doctor;
use crate::envgen;
use crate::launch::{self, Mode};
use crate::payload;
use crate::state;
use crate::status;
use crate::util;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

#[derive(Parser)]
#[command(
    name = "etol",
    version,
    about = "Detector, configurador y lanzador de la pila de etol",
    long_about = "Detecta juegos de Steam, su version de Proton, el prefijo de SimHub/CrewChief \
y el pad HF8. Genera las rutas en generated/entorno.sh y lanza los scripts existentes."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Detecta juegos, Proton, prefijos, SimHub y CrewChief
    Detect {
        /// Guarda el resultado en config/config.toml
        #[arg(long)]
        save: bool,
        /// Salida en JSON (para futuras interfaces)
        #[arg(long)]
        json: bool,
    },
    /// Detecta, guarda la configuracion y genera el entorno
    Init,
    /// Gestiona la configuracion
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Genera generated/entorno.sh
    Env {
        /// Escribe el entorno en otra ruta
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Muestra el estado en vivo
    Status,
    /// Comprueba la instalacion y propone arreglos
    Doctor {
        /// Borra locks de arranque huerfanos
        #[arg(long)]
        fix: bool,
    },
    /// Lanza los scripts de un juego
    Launch {
        /// lmu, ams2, acevo, assetto, rf2, dirt2, ets2 o beamng
        #[arg(value_parser = ["lmu", "ams2", "acevo", "assetto", "rf2", "dirt2", "ets2", "beamng"])]
        game: String,
        /// Solo el ciclo del puente
        #[arg(long)]
        puente: bool,
        /// Para el puente (no cierra las apps)
        #[arg(long)]
        stop: bool,
        /// Muestra lo que haria, sin ejecutar
        #[arg(long)]
        dry_run: bool,
        /// No configura el pad HF8
        #[arg(long)]
        no_hf8: bool,
        /// Argumentos extra para el script
        #[arg(last = true)]
        extra: Vec<String>,
    },
    /// Configura el pad HF8 (COM33)
    Hf8 {
        /// Muestra lo que haria, sin ejecutar
        #[arg(long)]
        dry_run: bool,
    },
    /// Ejecuta un script de instalacion (Bottles Flatpak o prefijo)
    Setup {
        /// bottles-install, bottles-user o prefix
        #[arg(value_parser = ["bottles-install", "bottles-user", "prefix"])]
        script: String,
        /// Solo muestra la ruta del script, sin ejecutarlo
        #[arg(long)]
        print: bool,
    },
    /// Para el puente y cierra las apps del juego
    Stop {
        /// lmu, ams2, acevo, assetto, rf2, dirt2, ets2 o beamng
        #[arg(value_parser = ["lmu", "ams2", "acevo", "assetto", "rf2", "dirt2", "ets2", "beamng"])]
        game: String,
        /// Solo para el puente, sin cerrar las aplicaciones
        #[arg(long)]
        solo_puente: bool,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Muestra la configuracion actual
    Show,
    /// Muestra la ruta del fichero de configuracion
    Path,
    /// Cambia un valor: etol config set apps.bottle /ruta
    Set { key: String, value: String },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Detect { save, json } => cmd_detect(save, json),
        Command::Init => cmd_init(),
        Command::Config { action } => cmd_config(action),
        Command::Env { out } => cmd_env(out),
        Command::Status => cmd_status(),
        Command::Doctor { fix } => cmd_doctor(fix),
        Command::Launch {
            game,
            puente,
            stop,
            dry_run,
            no_hf8,
            extra,
        } => cmd_launch(game, puente, stop, dry_run, no_hf8, extra),
        Command::Hf8 { dry_run } => cmd_hf8(dry_run),
        Command::Setup { script, print } => cmd_setup(script, print),
        Command::Stop { game, solo_puente } => cmd_stop(game, solo_puente),
    }
}

/// Deja la copia de scripts lista antes de detectar.
///
/// Dentro del AppImage el payload se instala en `~/.config/etol/payload`
/// y se usa como carpeta de trabajo. Fuera del AppImage no hace nada.
fn prepare_payload() {
    if let Some(dir) = payload::ensure_installed() {
        detect::set_app_dir_override(dir);
    }
}

fn cmd_detect(save: bool, json: bool) -> Result<()> {
    prepare_payload();
    let mut cfg = detect::detect();
    detect::preserve_manual_wine(&mut cfg);

    if json {
        println!("{}", serde_json::to_string_pretty(&cfg)?);
        return Ok(());
    }

    print_detected(&cfg);

    if save {
        let path = cfg.save()?;
        let env = envgen::write(&cfg)?;
        println!();
        println!("  {} {}", util::green("Configuracion:"), path.display());
        println!("  {} {}", util::green("Entorno:      "), env.display());
    }

    Ok(())
}

fn cmd_init() -> Result<()> {
    prepare_payload();
    let mut cfg = detect::detect();
    detect::preserve_manual_wine(&mut cfg);
    print_detected(&cfg);

    let path = cfg.save()?;
    let env = envgen::write(&cfg)?;

    println!();
    println!("  {} {}", util::green("Configuracion:"), path.display());
    println!("  {} {}", util::green("Entorno:      "), env.display());
    println!();
    println!(
        "  Siguiente paso: {}",
        util::cyan("etol doctor")
    );

    Ok(())
}

fn cmd_config(action: ConfigCmd) -> Result<()> {
    match action {
        ConfigCmd::Show => {
            let cfg = Config::load()?;
            print!("{}", toml::to_string_pretty(&cfg)?);
        }
        ConfigCmd::Path => println!("{}", config::config_path().display()),
        ConfigCmd::Set { key, value } => {
            let mut cfg = Config::load()?;
            cfg.apply_override(&key, &value)?;
            let path = cfg.save()?;
            let env = envgen::write(&cfg)?;
            println!("{} {} = {}", util::green("Actualizado"), key, value);
            println!("  config:  {}", path.display());
            println!("  entorno: {}", env.display());
        }
    }
    Ok(())
}

fn cmd_env(out: Option<PathBuf>) -> Result<()> {
    let cfg = Config::load()?;

    if !config::config_path().exists() {
        bail!("no hay configuracion guardada; ejecuta primero `etol init`");
    }

    match out {
        Some(path) => {
            envgen::write_to(&cfg, &path)?;
            println!("{}", path.display());
        }
        None => {
            let path = envgen::write(&cfg)?;
            println!("{}", path.display());
        }
    }
    Ok(())
}

fn cmd_setup(script: String, print: bool) -> Result<()> {
    prepare_payload();
    let path = payload::system_script(&script)
        .with_context(|| format!("no existe el script «{script}»"))?;
    println!("{} {}", util::cyan("Script:"), path.display());
    if print {
        return Ok(());
    }
    println!();
    let status = ProcessCommand::new("bash")
        .arg(&path)
        .status()
        .with_context(|| format!("no se pudo ejecutar {}", path.display()))?;
    if !status.success() {
        bail!("el script termino con {status}");
    }
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = Config::load()?;

    if !config::config_path().exists() {
        println!(
            "{}",
            util::yellow("Aviso: no hay configuracion guardada; se usa deteccion en memoria.")
        );
        println!("Ejecuta `etol init` para guardarla.\n");
    }

    print_status(&cfg);
    Ok(())
}

fn cmd_doctor(fix: bool) -> Result<()> {
    if !config::config_path().exists() {
        prepare_payload();
    }
    let mut cfg = Config::load()?;
    if cfg.bridge.app_dir.is_none() {
        if let Some(dir) = payload::ensure_installed() {
            cfg.bridge.app_dir = Some(dir);
        }
    }
    if !config::config_path().exists() {
        println!(
            "{}",
            util::yellow("Aviso: no hay configuracion guardada; ejecuta `etol init`.")
        );
    }

    if fix {
        let removed = doctor::fix_stale_locks(&cfg);
        util::section("Limpieza");
        if removed.is_empty() {
            println!("  {}", util::dim("No hay locks huerfanos."));
        } else {
            for lock in removed {
                println!("  {} {}", util::green("Borrado"), lock.display());
            }
        }
    }

    util::section("Comprobaciones");
    let checks = doctor::run(&cfg);
    let failures = doctor::print(&checks);

    println!();
    if failures == 0 {
        println!("  {}", util::green("Todo correcto."));
        Ok(())
    } else {
        println!(
            "  {} {} comprobacion(es) con fallo.",
            util::red("Atencion:"),
            failures
        );
        std::process::exit(1);
    }
}

fn cmd_launch(
    game: String,
    puente: bool,
    stop: bool,
    dry_run: bool,
    no_hf8: bool,
    extra: Vec<String>,
) -> Result<()> {
    let cfg = Config::load()?;

    if detect::game_def(&game).is_none() {
        bail!("juego desconocido: {game} (usa lmu, ams2, acevo, assetto o rf2)");
    }
    if !cfg.games.contains_key(&game) {
        bail!("el juego {game} no esta en la configuracion; ejecuta `etol init`");
    }

    let mode = if stop {
        Mode::Stop
    } else if puente {
        Mode::Puente
    } else {
        Mode::Todo
    };

    let no_hf8 = no_hf8 || cfg.hf8_disabled();
    launch::launch(&cfg, &game, mode, dry_run, no_hf8, &extra)
}

fn cmd_stop(game: String, solo_puente: bool) -> Result<()> {
    let cfg = Config::load()?;

    if detect::game_def(&game).is_none() {
        bail!("juego desconocido: {game} (usa lmu, ams2, acevo, assetto o rf2)");
    }
    if !cfg.games.contains_key(&game) {
        bail!("el juego {game} no esta en la configuracion; ejecuta `etol init`");
    }

    if solo_puente {
        let detail = launch::stop_bridge(&cfg, &game)?;
        util::section("Puente");
        if detail.is_empty() {
            util::field(&game, &util::dim("parado"));
        } else {
            for line in detail.lines().filter(|line| !line.trim().is_empty()) {
                println!("  {line}");
            }
        }
        return Ok(());
    }

    let report = launch::stop_all(&cfg, &game);
    print_stop_report(&report);

    if !report.all_ok() {
        std::process::exit(1);
    }
    Ok(())
}

fn print_stop_report(report: &launch::StopReport) {
    util::section(&format!("Parando {}", report.game.to_uppercase()));

    let bridge = if report.bridge.ok {
        util::green("parado")
    } else {
        util::red("fallo")
    };
    let detail = bridge_summary(&report.bridge.detail);
    if detail.is_empty() {
        util::field("puente", &bridge);
    } else {
        util::field("puente", &format!("{bridge}  {}", util::dim(&detail)));
    }

    for app in &report.apps {
        let state = if app.closed {
            util::green("cerrado")
        } else {
            util::red("sigue abierto")
        };
        util::field(&app.app, &format!("{state}  {}", util::dim(&app.detail)));
    }
}

/// Resume la salida del script y omite la frase que avisa de que el script no
/// toca las aplicaciones: `etol stop` si las cierra despues.
fn bridge_summary(detail: &str) -> String {
    detail
        .lines()
        .flat_map(|line| line.split(". "))
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.contains("no se han tocado"))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn cmd_hf8(dry_run: bool) -> Result<()> {
    let cfg = Config::load()?;
    let app_dir = cfg
        .app_dir()
        .context("no se conoce la carpeta de scripts; usa `etol config set bridge.app_dir <ruta>`")?;
    let script = app_dir.join("bridge").join("setup_hf8.sh");

    if !script.is_file() {
        bail!("no existe {}", script.display());
    }

    let env = launch::hf8_env(&cfg);

    if dry_run {
        println!("{}", util::bold("Simulacion (no se ejecuta nada)"));
        for (key, value) in &env {
            println!("  {key}={}", util::sh_quote(value));
        }
        println!("  bash {}", script.display());
        return Ok(());
    }

    println!("{} {}", util::cyan("Configurando HF8"), script.display());
    let status = ProcessCommand::new("bash")
        .arg(&script)
        .envs(env)
        .status()
        .with_context(|| format!("no se pudo ejecutar {}", script.display()))?;

    if !status.success() {
        bail!("el configurador de HF8 termino con {status}");
    }

    println!();
    println!("  Comprueba con: {}", util::cyan("etol doctor"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Salida
// ---------------------------------------------------------------------------

fn print_detected(cfg: &Config) {
    util::section("Juegos");

    for def in detect::GAMES {
        let Some(game) = cfg.games.get(def.id) else {
            continue;
        };
        println!(
            "  {}",
            util::bold(&format!("{} ({})", game.name, game.app_id))
        );
        util::subfield("instalado", &yes_no(game.installed));
        subfield_path("ruta", game.path.as_deref());
        subfield_path("ejecutable", game.exe.as_deref());
        subfield_path("prefijo", game.prefix.as_deref());
        subfield_path("proton", game.proton.as_deref());
        subfield_path("puente", Some(game.run_dir.as_path()));
    }

    util::section("SimHub y CrewChief");
    util::field_opt_str("botella", cfg.apps.bottle_name.as_deref());
    field_path("ruta", cfg.apps.bottle.as_deref());
    field_path("wine", cfg.apps.wine.as_deref());
    if let Some(source) = &cfg.apps.wine_source {
        util::field("origen wine", source);
    }
    field_path("SimHub", cfg.apps.simhub_exe.as_deref());
    field_path("CrewChief", cfg.apps.crewchief_exe.as_deref());

    util::section("Pad HF8");
    if cfg.hf8.enabled {
        let devices = state::hf8_devices();
        let device = devices.iter().find(|device| {
            device.vendor.eq_ignore_ascii_case(&cfg.hf8.vendor_id)
                && device.product.eq_ignore_ascii_case(&cfg.hf8.product_id)
        });
        match device {
            Some(device) => util::field(
                "dispositivo",
                &format!(
                    "{} · {}",
                    device.dev.display(),
                    if device.name.is_empty() {
                        "ForceFeel"
                    } else {
                        device.name.as_str()
                    }
                ),
            ),
            None => util::field(
                "dispositivo",
                &util::red("no conectado"),
            ),
        }
        util::field("puerto", &cfg.hf8.com_port);
    } else {
        util::field("estado", &util::dim("desactivado"));
    }

    util::section("Scripts");
    field_path("carpeta", cfg.app_dir());
}

fn print_status(cfg: &Config) {
    let snapshot = status::snapshot(cfg);

    println!();
    println!(
        "{}  {}",
        util::bold("Estado en vivo"),
        util::dim(&snapshot.generated)
    );

    util::section("Aplicaciones");
    match &snapshot.apps.bottle {
        Some(name) => util::field("botella", name),
        None => util::field("botella", &util::red("no detectada")),
    }
    match &snapshot.apps.wine {
        Some(wine) => util::field("wine", wine),
        None => util::field("wine", &util::red("no detectado")),
    }
    if let Some(source) = &snapshot.apps.wine_source {
        util::field("origen wine", source);
    }
    process_field("SimHub", &snapshot.apps.simhub);
    process_field("CrewChief", &snapshot.apps.crewchief);

    util::section("Pad HF8");
    if !snapshot.hf8.enabled {
        util::field("estado", &util::dim("desactivado"));
    } else {
        match &snapshot.hf8.device {
            Some(device) => {
                util::field("dispositivo", device);
                let target = snapshot
                    .hf8
                    .com_target
                    .clone()
                    .unwrap_or_else(|| "sin enlace".to_string());
                let suffix = if snapshot.hf8.com_ok {
                    util::green("correcto")
                } else {
                    util::red("incorrecto (etol hf8)")
                };
                util::field(
                    &snapshot.hf8.com_port,
                    &format!("-> {target}  {suffix}"),
                );
                if snapshot.hf8.holders.is_empty() {
                    util::field("abierto por", &util::dim("nadie"));
                } else {
                    util::field("abierto por", &snapshot.hf8.holders.join(", "));
                }
            }
            None => util::field("dispositivo", &util::red("no conectado")),
        }
    }

    util::section("Juegos");
    for game in &snapshot.games {
        println!("  {}", util::bold(&game.name));
        util::subfield("instalado", &yes_no(game.installed));

        if game.bridge_running {
            let pids: Vec<String> = game.bridge_pids.iter().map(|pid| pid.to_string()).collect();
            util::subfield(
                "puente",
                &format!("{} (pid {})", util::green("activo"), pids.join(", ")),
            );
        } else {
            util::subfield("puente", &util::dim("parado"));
        }

        if game.watchdog_running {
            util::subfield("vigilante", &util::green("activo"));
        }

        let port = if game.port_in_use {
            util::green("en uso")
        } else {
            util::dim("libre")
        };
        util::subfield(&format!("puerto {}", game.port), &port);

        if let Some(lock) = &game.lock {
            let text = if game.lock_held {
                format!("lock activo {lock}")
            } else {
                format!("lock huerfano {lock}")
            };
            util::subfield("arranque", &text);
        }
    }
}

fn process_field(label: &str, status: &status::ProcessStatus) {
    if status.running {
        let pids: Vec<String> = status.pids.iter().map(|pid| pid.to_string()).collect();
        util::field(
            label,
            &format!("{} (pid {})", util::green("en marcha"), pids.join(", ")),
        );
    } else {
        util::field(label, &util::dim("parado"));
    }
}

fn field_path(label: &str, value: Option<&Path>) {
    util::field_opt(label, value);
}

fn subfield_path(label: &str, value: Option<&Path>) {
    util::subfield_opt(label, value);
}

fn yes_no(value: bool) -> String {
    if value {
        util::green("si")
    } else {
        util::red("no")
    }
}

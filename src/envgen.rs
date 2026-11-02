//! Genera `generated/entorno.sh`, el unico punto donde viven las rutas.
//!
//! Los scripts de SIMHUB_LINUX ya aceptan estas variables por entorno, asi que
//! no hace falta parchearlos: basta con hacer `source` de este fichero.

use crate::config::{self, Config};
use crate::util;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn default_path() -> PathBuf {
    config::generated_dir().join("entorno.sh")
}

pub fn render(cfg: &Config) -> String {
    let mut out = String::new();

    out.push_str("#!/usr/bin/env bash\n");
    out.push_str("# entorno.sh - rutas de la pila de etol\n");
    out.push_str(&format!("# Generado por etol el {}\n", util::now_utc()));
    out.push_str("#\n");
    out.push_str("# NO editar a mano. Cambia config/config.toml o usa\n");
    out.push_str("#   etol config set <clave> <valor>\n");
    out.push_str("# y regenera con: etol env\n");
    out.push_str("#\n");
    out.push_str("# Uso:\n");
    out.push_str("#   source generated/entorno.sh\n");
    out.push_str("#   ./activar_puente_ams2.sh\n\n");

    out.push_str("# --- SimHub / CrewChief (botella de Bottles) ---\n");
    export_path(&mut out, "SIMHUB_BOTTLE", cfg.apps.bottle.as_deref());
    export_path(&mut out, "BOTTLE_WINE", cfg.apps.wine.as_deref());
    if let Some(source) = &cfg.apps.wine_source {
        out.push_str(&format!(
            "export BOTTLE_WINE_SOURCE={}\n",
            util::sh_quote(source)
        ));
    }

    out.push_str("\n# --- Directorio de trabajo del puente ---\n");
    export_path(&mut out, "ETOL_RUN_BASE", Some(&cfg.bridge.run_base));

    out.push_str("\n# --- Juegos ---\n");
    export_game(&mut out, cfg, "lmu", true);
    export_game(&mut out, cfg, "ams2", false);
    export_game(&mut out, cfg, "acevo", false);
    export_game(&mut out, cfg, "assetto", false);
    export_game(&mut out, cfg, "rf2", false);

    out.push_str("\n# --- Scripts de SIMHUB_LINUX ---\n");
    export_path(&mut out, "ETOL_APP_DIR", cfg.app_dir());

    out.push_str("\n# --- Pad HF8 (ForceFeel) ---\n");
    out.push_str(&format!(
        "export HF8_COM={}\n",
        util::sh_quote(&cfg.hf8.com_port)
    ));
    out.push_str(&format!(
        "export HF8_VID={}\n",
        util::sh_quote(&cfg.hf8.vendor_id)
    ));
    out.push_str(&format!(
        "export HF8_PID={}\n",
        util::sh_quote(&cfg.hf8.product_id)
    ));
    if cfg.hf8.enabled {
        export_path(&mut out, "HF8_DEV", cfg.hf8.device.as_deref());
    } else {
        out.push_str("# HF8 desactivado en la configuracion\n");
    }

    out
}

pub fn write(cfg: &Config) -> Result<PathBuf> {
    let path = default_path();
    write_to(cfg, &path)?;
    Ok(path)
}

pub fn write_to(cfg: &Config, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("no se pudo crear {}", parent.display()))?;
    }
    std::fs::write(path, render(cfg))
        .with_context(|| format!("no se pudo escribir {}", path.display()))?;
    Ok(())
}

fn export_game(out: &mut String, cfg: &Config, id: &str, uses_proton_dir: bool) {
    let Some(game) = cfg.games.get(id) else {
        out.push_str(&format!("# {id}: no detectado\n"));
        return;
    };

    let upper = game.id.to_uppercase();
    export_path(out, &format!("{upper}_PFX"), game.prefix.as_deref());

    if uses_proton_dir {
        export_path(out, &format!("{upper}_PROTON"), game.proton.as_deref());
    } else {
        export_path(out, &format!("{upper}_WINE"), game.wine.as_deref());
    }

    if !game.run_dir.as_os_str().is_empty() {
        export_path(out, &format!("{upper}_MIRROR_RUN_DIR"), Some(&game.run_dir));
    }
}

fn export_path(out: &mut String, name: &str, value: Option<&Path>) {
    match value {
        Some(path) => out.push_str(&format!(
            "export {name}={}\n",
            util::sh_quote(&path.display().to_string())
        )),
        None => out.push_str(&format!("# {name}: no detectado\n")),
    }
}

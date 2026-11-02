//! Comprobacion manual del puerto TCP de SimHub (8888).
//!
//! El doctor no puede leer las reglas de ufw/firewalld sin permisos, asi que
//! ofrece un boton que abre una terminal: el usuario escribe su contrasena y
//! el script deja el resultado en un fichero que ETOL lee despues.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize)]
pub struct PortCheckPlan {
    pub port: u16,
    /// Comando que se ejecuta en la terminal.
    pub command: String,
    /// Comprobaciones internas que hace el script.
    pub checks: Vec<String>,
    pub terminal: String,
    pub script: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortCheckResult {
    pub ok: bool,
    pub status: String,
    pub detail: String,
    pub fix: Option<String>,
}

fn run_base() -> PathBuf {
    crate::config::Config::load()
        .map(|cfg| cfg.bridge.run_base)
        .unwrap_or_else(|_| PathBuf::from("/tmp/etol"))
}

fn script_path(port: u16) -> PathBuf {
    run_base().join(format!("comprobar_{port}.sh"))
}

fn result_path(port: u16) -> PathBuf {
    run_base().join(format!("comprobar_{port}.txt"))
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(name);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

fn terminal() -> String {
    for name in [
        "konsole",
        "xdg-terminal-exec",
        "gnome-terminal",
        "alacritty",
        "xterm",
    ] {
        if command_exists(name) {
            return name.to_string();
        }
    }
    String::new()
}

fn checks(port: u16) -> Vec<String> {
    let listen = format!("ss -ltn | grep :{port}");
    if command_exists("ufw") {
        vec![listen, format!("sudo ufw status | grep {port}/tcp")]
    } else if command_exists("firewall-cmd") {
        vec![
            listen,
            format!("sudo firewall-cmd --query-port={port}/tcp"),
        ]
    } else {
        vec![listen, format!("sudo ss -ltnp | grep :{port}")]
    }
}

pub fn plan(port: u16) -> Result<PortCheckPlan> {
    if port == 0 {
        bail!("puerto invalido");
    }
    let script = script_path(port);
    Ok(PortCheckPlan {
        port,
        command: format!("bash {}", script.display()),
        checks: checks(port),
        terminal: terminal(),
        script: script.display().to_string(),
    })
}

fn write_script(port: u16) -> Result<PathBuf> {
    let path = script_path(port);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("no se pudo crear {}", parent.display()))?;
    }
    let result = result_path(port);
    let body = format!(
        r#"#!/usr/bin/env bash
set -u

PORT={port}
RESULT={result}

echo "ETOL - comprobacion del puerto $PORT"
echo

local_ok=0
if ss -ltn 2>/dev/null | grep -qE ":$PORT[[:space:]]"; then
  local_ok=1
  echo "SimHub escucha en TCP $PORT."
else
  echo "SimHub NO escucha en TCP $PORT."
fi
echo

fw=unknown
if command -v ufw >/dev/null 2>&1; then
  out="$(sudo ufw status 2>&1 || true)"
  echo "$out"
  if echo "$out" | grep -qiE "password is required|incorrect password|contrase"; then
    fw=auth
  elif echo "$out" | grep -qiE "status: active|estado: activo"; then
    if echo "$out" | grep -E "$PORT/tcp" | grep -qiE "allow|permit"; then
      fw=ok
    else
      fw=blocked
    fi
  else
    fw=inactive
  fi
elif command -v firewall-cmd >/dev/null 2>&1; then
  if sudo firewall-cmd --query-port="$PORT/tcp" >/dev/null 2>&1; then
    fw=ok
  else
    fw=blocked
  fi
else
  out="$(sudo ss -ltnp 2>/dev/null | grep -E ":$PORT[[:space:]]" || true)"
  echo "$out"
  fw=unknown
fi

if [ "$local_ok" != 1 ]; then
  status=fail
  detail="SimHub no escucha en TCP $PORT."
  fix="Abre SimHub y vuelve a comprobar."
elif [ "$fw" = blocked ]; then
  status=fail
  detail="SimHub escucha, pero el firewall bloquea TCP $PORT."
  fix="ufw: sudo ufw allow $PORT/tcp  |  firewalld: sudo firewall-cmd --add-port=$PORT/tcp --permanent && sudo firewall-cmd --reload"
elif [ "$fw" = auth ]; then
  status=fail
  detail="No se pudo comprobar el firewall (sin contrasena o cancelado)."
  fix="Vuelve a intentarlo y escribe tu contrasena cuando la pida."
else
  status=ok
  detail="SimHub escucha en TCP $PORT y el firewall no lo bloquea ($fw)."
  fix=""
fi

printf 'status=%s\n' "$status" > "$RESULT"
printf 'detail=%s\n' "$detail" >> "$RESULT"
printf 'fix=%s\n' "$fix" >> "$RESULT"

echo
echo "Resultado: $detail"
if [ -n "$fix" ]; then
  echo "Sugerencia: $fix"
fi
echo
read -r -p "Pulsa Enter para cerrar..." _
"#,
        port = port,
        result = result.display()
    );
    fs::write(&path, body)
        .with_context(|| format!("no se pudo escribir {}", path.display()))?;
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
    Ok(path)
}

pub fn open(port: u16) -> Result<PortCheckPlan> {
    let plan = plan(port)?;
    if plan.terminal.is_empty() {
        bail!(
            "no encuentro una terminal (konsole, gnome-terminal, alacritty o xterm). Ejecuta a mano: {}",
            plan.command
        );
    }
    let script = write_script(port)?;
    let _ = fs::remove_file(result_path(port));

    let mut command = match plan.terminal.as_str() {
        "konsole" => {
            let mut c = Command::new("konsole");
            c.arg("-e").arg("bash").arg(&script);
            c
        }
        "alacritty" => {
            let mut c = Command::new("alacritty");
            c.arg("-e").arg("bash").arg(&script);
            c
        }
        "gnome-terminal" => {
            let mut c = Command::new("gnome-terminal");
            c.arg("--").arg("bash").arg(&script);
            c
        }
        "xdg-terminal-exec" => {
            let mut c = Command::new("xdg-terminal-exec");
            c.arg("bash").arg(&script);
            c
        }
        other => {
            let mut c = Command::new(other);
            c.arg("-e").arg("bash").arg(&script);
            c
        }
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("no se pudo abrir {}", plan.terminal))?;

    Ok(plan)
}

pub fn result(port: u16) -> Option<PortCheckResult> {
    let text = fs::read_to_string(result_path(port)).ok()?;
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
    Some(PortCheckResult {
        ok: status == "ok",
        status,
        detail,
        fix: if fix.is_empty() { None } else { Some(fix) },
    })
}

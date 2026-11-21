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

#[derive(Debug, Clone, Serialize)]
pub struct FirewallOpenPlan {
    /// `ufw`, `firewalld`, `inactive` o `none`.
    pub firewall: String,
    pub lan: Option<String>,
    /// Comando manual (con sudo) que se muestra al usuario.
    pub command: Option<String>,
    pub available: bool,
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

fn service_active(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn firewall_kind() -> &'static str {
    if command_exists("ufw") && service_active("ufw") {
        "ufw"
    } else if command_exists("firewall-cmd") && service_active("firewalld") {
        "firewalld"
    } else if command_exists("ufw") || command_exists("firewall-cmd") {
        "inactive"
    } else {
        "none"
    }
}

/// Plan de apertura del puerto, restringido a la red local.
pub fn open_plan(port: u16) -> Result<FirewallOpenPlan> {
    if port == 0 {
        bail!("puerto invalido");
    }
    let kind = firewall_kind();
    if kind != "ufw" && kind != "firewalld" {
        return Ok(FirewallOpenPlan {
            firewall: kind.to_string(),
            lan: None,
            command: None,
            available: false,
        });
    }

    let lan = etol::util::local_lan_cidr().unwrap_or_else(|| "192.168.0.0/16".to_string());
    let command = match kind {
        "ufw" => format!("sudo ufw allow from {lan} to any port {port} proto tcp"),
        "firewalld" => format!(
            "sudo firewall-cmd --permanent --add-rich-rule='rule family=\"ipv4\" source address=\"{lan}\" port port=\"{port}\" protocol=\"tcp\" accept' && sudo firewall-cmd --reload"
        ),
        _ => unreachable!(),
    };
    Ok(FirewallOpenPlan {
        firewall: kind.to_string(),
        lan: Some(lan),
        command: Some(command),
        available: true,
    })
}

/// Aplica la regla con pkexec. Solo abre el puerto para la red local.
pub fn open_firewall(port: u16) -> Result<String> {
    let plan = open_plan(port)?;
    if !plan.available {
        let detail = match plan.firewall.as_str() {
            "inactive" => "No hay un firewall activo; el puerto ya no esta bloqueado.",
            "none" => "No se detecto ufw ni firewalld; configura el firewall que uses.",
            _ => "No se pudo detectar el firewall.",
        };
        bail!(detail);
    }

    let lan = plan.lan.context("no se detecto la red local")?;
    if !valid_lan(&lan) {
        bail!("red local no valida: {lan}");
    }
    let script = match plan.firewall.as_str() {
        "ufw" => format!("ufw allow from {lan} to any port {port} proto tcp"),
        "firewalld" => format!(
            "firewall-cmd --permanent --add-rich-rule='rule family=\"ipv4\" source address=\"{lan}\" port port=\"{port}\" protocol=\"tcp\" accept' && firewall-cmd --reload"
        ),
        _ => unreachable!(),
    };

    if !command_exists("pkexec") {
        bail!("no se encontro pkexec para aplicar la regla");
    }
    let mut command = Command::new("pkexec");
    command.arg("/bin/sh").arg("-c").arg(&script);
    etol::launch::clean_child_env(&mut command);

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
        return Ok(format!("Puerto {port} abierto solo para la red local {lan}."));
    }
    match output.status.code() {
        Some(126) => bail!("autorizacion cancelada"),
        Some(127) => bail!("pkexec no pudo ejecutar la regla"),
        Some(code) => bail!("la regla fallo (codigo {code})\n{text}"),
        None => bail!("la regla fallo\n{text}"),
    }
}

fn valid_lan(lan: &str) -> bool {
    let Some((ip, prefix)) = lan.split_once('/') else {
        return false;
    };
    ip.parse::<std::net::Ipv4Addr>().is_ok()
        && prefix
            .parse::<u8>()
            .map(|value| value <= 32)
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

# Red local para las reglas del firewall (no abrir el puerto a Internet).
LAN_CIDR="$(ip -o -4 addr show scope global 2>/dev/null \
  | grep -vE ' (docker|br-|virbr|veth|tun|tap|wg|tailscale|zt)' \
  | grep -E 'inet (10\.|192\.168\.|172\.(1[6-9]|2[0-9]|3[01])\.)' \
  | head -1 | sed -E 's/.* inet ([^ ]+).*/\1/')"
LAN=""
if [ -n "$LAN_CIDR" ]; then
  LAN="$(python3 -c 'import ipaddress,sys; print(ipaddress.ip_interface(sys.argv[1]).network)' "$LAN_CIDR" 2>/dev/null || true)"
fi
if [ -z "$LAN" ]; then
  LAN="192.168.0.0/16"
fi

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
  fix="ufw: sudo ufw allow from $LAN to any port $PORT proto tcp  |  firewalld: sudo firewall-cmd --permanent --add-rich-rule='rule family=\"ipv4\" source address=\"$LAN\" port port=\"$PORT\" protocol=\"tcp\" accept' && sudo firewall-cmd --reload"
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

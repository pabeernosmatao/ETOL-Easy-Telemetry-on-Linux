#!/usr/bin/env python3
"""Configura SimHub y CrewChief para consumir el espejo de Assetto Corsa
Competizione.

- SimHub: Assetto Corsa Competizione visible y como juego preferente, lector
  por memoria compartida (CurrentReader null).
- CrewChief: juego activo ACC.
- Wine: salida de audio por PulseAudio, para que CrewChief se oiga.

No toca ninguna opcion de otros juegos, no arranca ni reinicia aplicaciones y
no modifica archivos de una app que este abierta (esa app los reescribiria al
cerrarse). Hace copia de seguridad antes de cada cambio.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from xml.sax.saxutils import escape

HOME = Path.home()
BOTTLE = Path(os.environ.get(
    "SIMHUB_BOTTLE",
    HOME / ".local/share/bottles/bottles/simhub",
)).expanduser()
BACKUP_DIR = Path(os.environ.get(
    "ACC_MIRROR_BACKUP_DIR",
    HOME / ".local/share/acc_mirror/backups",
)).expanduser()
RUN_DIR = os.environ.get("ACC_MIRROR_RUN_DIR", "/tmp/etol/acc_mirror")
ACC_PFX = os.environ.get("ACC_PFX", "").strip()
STAMP = datetime.now().strftime("%Y%m%d-%H%M%S")
CHANGES: list[str] = []
WARNINGS: list[str] = []


def app_running(exe_name: str) -> bool:
    """True si el .exe real de la app corre en algun wineserver."""
    needle = exe_name.encode()
    try:
        entries = list(Path("/proc").iterdir())
    except OSError:
        return False
    for entry in entries:
        if not entry.name.isdigit():
            continue
        try:
            raw = (entry / "cmdline").read_bytes()
        except OSError:
            continue
        if needle not in raw:
            continue
        cmd = raw.replace(b"\0", b" ").strip().lower()
        if any(marker in cmd for marker in (
            b"start.exe", b"wine", b"pv-adverb", b"pressure-vessel",
            b"proton", b"reaper", b"steamlaunch",
        )):
            continue
        return True
    return False


def backup(path: Path) -> None:
    if not path.exists():
        return
    BACKUP_DIR.mkdir(parents=True, exist_ok=True)
    dest = BACKUP_DIR / f"{path.name}.{STAMP}.bak"
    shutil.copy2(path, dest)
    CHANGES.append(f"backup: {dest}")


def write_atomic(path: Path, text: str) -> None:
    tmp = path.with_name(path.name + ".tmp-acc-mirror")
    tmp.write_text(text, encoding="utf-8", newline="")
    os.replace(tmp, path)


def load_json(path: Path, default):
    if not path.exists():
        return default
    try:
        with path.open("r", encoding="utf-8-sig") as fh:
            return json.load(fh)
    except Exception as exc:  # noqa: BLE001
        print(f"AVISO: no se pudo leer {path}: {exc}", file=sys.stderr)
        return default


def save_json(path: Path, data) -> None:
    backup(path)
    write_atomic(path, json.dumps(data, indent=2, ensure_ascii=False) + "\n")


def load_acc_json(path: Path, default):
    """ACC escribe este JSON en UTF-16, pero puede quedar a medias."""
    if not path.exists():
        return default
    raw = path.read_bytes()
    for encoding in ("utf-16", "utf-8-sig", "utf-8"):
        try:
            return json.loads(raw.decode(encoding))
        except Exception:
            continue
    if raw.strip(b"\xff\xfe\x00"):
        print(f"AVISO: no se pudo leer {path}; se reescribe", file=sys.stderr)
    return default


def write_utf16(path: Path, text: str) -> None:
    tmp = path.with_name(path.name + ".tmp-acc-mirror")
    tmp.write_text(text, encoding="utf-16", newline="")
    os.replace(tmp, path)


def configure_wine_audio() -> None:
    """Deja la salida de audio de Wine en PulseAudio (PipeWire-Pulse).

    Este runner no tiene clave Audio y arranca `winealsa`, que en este equipo
    no inicializa ningun dispositivo: la botella expone cero salidas y
    CrewChief reproduce sus avisos por dentro pero no se oye nada. Con
    `pulse` los avisos salen por el dispositivo por defecto del sistema.
    """
    wine = os.environ.get("BOTTLE_WINE", "").strip()
    if not wine or not Path(wine).is_file():
        WARNINGS.append(
            "falta BOTTLE_WINE; no puedo revisar el driver de audio de Wine"
        )
        return

    socket = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")) / "pulse/native"
    if not socket.exists() and not os.environ.get("PULSE_SERVER"):
        WARNINGS.append(
            "no encuentro PulseAudio/PipeWire; CrewChief podria quedarse sin audio"
        )
        return

    env = dict(os.environ)
    env["WINEPREFIX"] = str(BOTTLE)
    env["WINEDEBUG"] = "-all"

    def reg(args: list[str]) -> subprocess.CompletedProcess:
        return subprocess.run(
            [wine, "reg", *args],
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
        )

    query = reg(["query", r"HKCU\Software\Wine\Drivers", "/v", "Audio"])
    if query.returncode == 0 and re.search(r"\bpulse\b", query.stdout, re.I):
        CHANGES.append("Wine: audio ya estaba en PulseAudio")
        return

    backup(BOTTLE / "user.reg")
    add = reg([
        "add", r"HKCU\Software\Wine\Drivers", "/v", "Audio",
        "/t", "REG_SZ", "/d", "pulse", "/f",
    ])
    if add.returncode != 0:
        WARNINGS.append(
            "no se pudo activar el audio PulseAudio de Wine: "
            + (add.stderr or add.stdout).strip()
        )
        return
    CHANGES.append("Wine: audio por PulseAudio (CrewChief se oye)")


def configure_acc_broadcasting() -> None:
    """Activa el servidor de broadcasting de ACC en 127.0.0.1:9000.

    CrewChief y SimHub escuchan en ese puerto y usan la contrasena "asd".
    El juego lee el fichero al arrancar: si ACC esta abierto hay que
    reiniciarlo una vez.
    """
    if not ACC_PFX:
        WARNINGS.append("falta ACC_PFX; no puedo escribir el broadcasting.json de ACC")
        return
    configs = list(
        (Path(ACC_PFX) / "drive_c/users").glob(
            "*/Documents/Assetto Corsa Competizione/Config"
        )
    )
    if not configs:
        WARNINGS.append("no encuentro la carpeta Config de ACC en el prefijo")
        return
    desired = {
        "updListenerPort": 9000,
        "connectionPassword": "asd",
        "commandPassword": "",
    }
    for config_dir in configs:
        path = config_dir / "broadcasting.json"
        data = load_acc_json(path, {})
        if not isinstance(data, dict):
            data = {}
        changed = any(data.get(key) != value for key, value in desired.items())
        if not changed:
            CHANGES.append(
                f"ACC: broadcasting ya estaba activo en 127.0.0.1:9000 ({path})"
            )
            continue
        data.update(desired)
        backup(path)
        write_utf16(path, json.dumps(data, indent=4, ensure_ascii=False) + "\n")
        CHANGES.append(f"ACC: broadcasting activado en 127.0.0.1:9000 ({path})")

    if app_running("AC2-Win64-Shipping.exe"):
        WARNINGS.append(
            "ACC esta abierto: reinicialo una vez para que lea broadcasting.json"
        )


def configure_simhub() -> None:
    simhub_running = app_running("SimHubWPF.exe")
    plugins = BOTTLE / "drive_c/Program Files (x86)/SimHub/PluginsData"

    hidden_path = plugins / "HiddenGames.json"
    hidden = load_json(hidden_path, {})
    if hidden.get("AssettoCorsaCompetizione") is not False:
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco HiddenGames.json; "
                "Assetto Corsa Competizione deberia estar visible."
            )
        else:
            hidden["AssettoCorsaCompetizione"] = False
            save_json(hidden_path, hidden)
            CHANGES.append("SimHub: Assetto Corsa Competizione visible en HiddenGames.json")
    else:
        CHANGES.append("SimHub: Assetto Corsa Competizione ya estaba visible")

    game_path = plugins / "GameSettings.json"
    games = load_json(game_path, {})
    entry = games.get("AssettoCorsaCompetizione")
    if not isinstance(entry, dict):
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto y no tiene entrada AssettoCorsaCompetizione; "
                "se creara en el proximo arranque."
            )
        else:
            games["AssettoCorsaCompetizione"] = {
                "UDPPort": None,
                "UDPForwardPort": None,
                "UDPForwardIpAddress": "127.0.0.1",
                "TyreWearMinimum": None,
                "UDPForwardActive": False,
                "DisableConfigAlert": False,
                "OrderEx": 1,
                "LauncherSetting": None,
                "CurrentReader": None,
                "ManualConfigurationDismissed": False,
                "AddictionnalUDPRedirects": [],
                "AutomaticConfigurationDismissed": False,
                "DisplayUnitSettings": None,
                "TargetIP": None,
                "LastActivations": [],
            }
            save_json(game_path, games)
            CHANGES.append("SimHub: entrada AssettoCorsaCompetizione creada")
    else:
        changed = False
        if entry.get("UDPPort") != 9000:
            if simhub_running:
                WARNINGS.append(
                    "SimHub esta abierto: no cambio el puerto UDP de ACC; "
                    "deberia usar 9000 por defecto."
                )
            else:
                entry["UDPPort"] = 9000
                changed = True
        if entry.get("OrderEx") != 1:
            if simhub_running:
                WARNINGS.append(
                    "SimHub esta abierto: no cambio OrderEx; selecciona "
                    "Assetto Corsa Competizione manualmente si hace falta."
                )
            else:
                entry["OrderEx"] = 1
                changed = True
        if entry.get("CurrentReader") is not None:
            WARNINGS.append(
                "SimHub tiene un lector explicito para AssettoCorsaCompetizione; "
                "el espejo usa memoria compartida (acpmf_*)."
            )
        if changed:
            save_json(game_path, games)
            CHANGES.append(
                "SimHub: Assetto Corsa Competizione marcado como preferente (OrderEx=1)"
            )
        else:
            CHANGES.append("SimHub: Assetto Corsa Competizione ya estaba configurado")


def set_xml_setting(text: str, name: str, value: str) -> tuple[str, bool]:
    pattern = re.compile(
        r'(<setting\s+name="' + re.escape(name) +
        r'"\s+serializeAs="String">\s*)'
        r'(<value\s*/>|<value>.*?</value>)',
        re.S,
    )
    replacement = r'\1<value>' + escape(value) + r'</value>'
    new_text, count = pattern.subn(replacement, text, count=1)
    if count:
        return new_text, True

    marker = "    </CrewChiefV4.Properties.Settings>"
    if marker not in new_text:
        return new_text, False
    block = (
        f'      <setting name="{name}" serializeAs="String">\n'
        f'        <value>{escape(value)}</value>\n'
        f'      </setting>\n'
    )
    return new_text.replace(marker, block + marker, 1), True


def configure_crewchief() -> None:
    exe_dir = BOTTLE / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
    config_path = exe_dir / "CrewChiefV4.exe.config"
    if not config_path.exists():
        WARNINGS.append(f"no existe {config_path}")
        return

    if app_running("CrewChiefV4.exe"):
        WARNINGS.append(
            "CrewChief esta abierto: no toco su configuracion (la reescribe al "
            "cerrarse). Si no esta ya en ACC, cambialo en la app cuando no "
            "estes en pista."
        )
        return

    text = config_path.read_text(encoding="utf-8-sig")
    original = text
    desired = {
        "last_game_definition": "ACC",
        "enable_auto_detect": "False",
        "run_immediately": "True",
    }
    for name, value in desired.items():
        text, ok = set_xml_setting(text, name, value)
        if not ok:
            WARNINGS.append(f"no se pudo ajustar {name}")

    m = re.search(
        r'(<setting\s+name="limit_available_games"\s+serializeAs="String">\s*'
        r'<value>)(.*?)(</value>)',
        text,
        re.S,
    )
    if m:
        games = [x.strip() for x in m.group(2).split(",") if x.strip()]
        if "ACC" not in games:
            games.append("ACC")
            text = text[:m.start(2)] + ",".join(games) + text[m.end(2):]

    if text != original:
        backup(config_path)
        write_atomic(config_path, text)
        CHANGES.append("CrewChief: juego activo ACC")
    else:
        CHANGES.append("CrewChief: ya estaba configurado para ACC")


def main() -> int:
    if not BOTTLE.is_dir():
        print(f"ERROR: no existe la botella {BOTTLE}", file=sys.stderr)
        return 1

    configure_wine_audio()
    configure_acc_broadcasting()
    configure_simhub()
    configure_crewchief()

    for line in CHANGES:
        print(f"  · {line}")
    for line in WARNINGS:
        print(f"  AVISO: {line}", file=sys.stderr)
    if not CHANGES and not WARNINGS:
        print("  · sin cambios")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

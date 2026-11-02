#!/usr/bin/env python3
"""Configura DiRT Rally 2.0, SimHub y CrewChief para la telemetria UDP.

Reparto:

  juego -> 127.0.0.1:20777 -> relay ETOL -> SimHub 20778
                                          -> CrewChief 20779

El relay lo arranca `activar_puente_dirt2.sh`. Este script solo deja la
configuracion escrita y hace copia de seguridad de cada archivo.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path
from xml.sax.saxutils import escape

HOME = Path.home()
BOTTLE = Path(
    os.environ.get("SIMHUB_BOTTLE", HOME / ".local/share/bottles/bottles/simhub")
).expanduser()
PFX = Path(os.environ.get("DIRT2_PFX", "")).expanduser()
RUN_DIR = os.environ.get("DIRT2_UDP_RUN_DIR", "/tmp/etol/dirt2_udp")
BACKUP_DIR = Path(
    os.environ.get("DIRT2_UDP_BACKUP_DIR", HOME / ".local/share/dirt2_udp/backups")
).expanduser()
STAMP = datetime.now().strftime("%Y%m%d-%H%M%S")

GAME_PORT = int(os.environ.get("DIRT2_LISTEN_PORT", "20777"))
SIMHUB_PORT = int(os.environ.get("DIRT2_SIMHUB_PORT", "20778"))
CREWCHIEF_PORT = int(os.environ.get("DIRT2_CREWCHIEF_PORT", "20779"))

CHANGES: list[str] = []
WARNINGS: list[str] = []


def app_running(exe_name: str) -> bool:
    """True si el .exe real de la app corre en algun wineserver."""
    needle = exe_name.encode()
    proc = Path("/proc")
    try:
        entries = list(proc.iterdir())
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
        if any(
            marker in cmd
            for marker in (
                b"start.exe",
                b"wine",
                b"pv-adverb",
                b"pressure-vessel",
                b"proton",
                b"reaper",
                b"steamlaunch",
            )
        ):
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
    tmp = path.with_name(path.name + ".tmp-etol-dirt2")
    tmp.write_text(text, encoding="utf-8", newline="")
    os.replace(tmp, path)


def load_json(path: Path, default):
    if not path.exists():
        return default
    try:
        with path.open("r", encoding="utf-8-sig") as fh:
            return json.load(fh)
    except Exception as exc:  # noqa: BLE001
        WARNINGS.append(f"no se pudo leer {path}: {exc}")
        return default


def save_json(path: Path, data) -> None:
    backup(path)
    write_atomic(path, json.dumps(data, indent=2, ensure_ascii=False) + "\n")


def set_xml_setting(text: str, name: str, value: str) -> tuple[str, bool]:
    pattern = re.compile(
        r'(<setting\s+name="' + re.escape(name) +
        r'"\s+serializeAs="String">\s*)'
        r'(<value\s*/>|<value>.*?</value>)',
        re.S,
    )
    replacement = r"\1<value>" + escape(value) + r"</value>"
    new_text, count = pattern.subn(replacement, text, count=1)
    if count:
        return new_text, True

    marker = "    </CrewChiefV4.Properties.Settings>"
    if marker not in new_text:
        return new_text, False
    block = (
        f'      <setting name="{name}" serializeAs="String">\n'
        f'        <value>{escape(value)}</value>\n'
        f"      </setting>\n"
    )
    return new_text.replace(marker, block + marker, 1), True


def game_config_files() -> list[Path]:
    """XML de pantalla normal y de VR, si existen."""
    base = (
        PFX
        / "drive_c/users/steamuser/Documents/My Games/DiRT Rally 2.0"
        / "hardwaresettings"
    )
    files = [
        base / name
        for name in (
            "hardware_settings_config.xml",
            "hardware_settings_config_vr.xml",
        )
        if (base / name).exists()
    ]
    if not files:
        files = sorted(PFX.rglob("hardware_settings_config*.xml"))
    return files


def configure_game() -> None:
    configs = game_config_files()
    if not configs:
        WARNINGS.append(
            "no se encontro hardware_settings_config.xml de DiRT Rally 2.0"
        )
        return

    # extradata=3 es el paquete completo de 264 bytes (66 floats). Con 1 o 2
    # el juego manda paquetes cortos que SimHub y CrewChief no saben leer.
    tag = (
        f'<udp enabled="true" extradata="3" ip="127.0.0.1" '
        f'port="{GAME_PORT}" delay="1" />'
    )

    for config in configs:
        text = config.read_text(encoding="utf-8-sig")
        original = text
        text, count = re.subn(r"<udp\b[^>]*/>", tag, text, count=1)
        if count == 0:
            WARNINGS.append(
                f"no se encontro la etiqueta <udp> en {config.name}"
            )
            continue

        if text != original:
            backup(config)
            write_atomic(config, text)
            CHANGES.append(
                f"DiRT Rally 2.0 ({config.name}): UDP extradata=3 en "
                f"127.0.0.1:{GAME_PORT}"
            )
        else:
            CHANGES.append(
                f"DiRT Rally 2.0 ({config.name}): UDP ya estaba configurado"
            )

    if app_running("dirtrally2.exe"):
        WARNINGS.append(
            "DiRT Rally 2.0 esta abierto: cierra el juego y vuelve a abrirlo "
            "para que aplique el UDP completo (extradata=3)"
        )


def configure_simhub() -> None:
    simhub_running = app_running("SimHubWPF.exe")
    plugins = BOTTLE / "drive_c/Program Files (x86)/SimHub/PluginsData"

    hidden_path = plugins / "HiddenGames.json"
    hidden = load_json(hidden_path, {})
    if hidden.get("CodemastersDirtRally2") is not False:
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco HiddenGames.json; "
                "DiRT Rally 2.0 deberia estar visible."
            )
        else:
            hidden["CodemastersDirtRally2"] = False
            save_json(hidden_path, hidden)
            CHANGES.append("SimHub: DiRT Rally 2.0 visible en HiddenGames.json")
    else:
        CHANGES.append("SimHub: DiRT Rally 2.0 ya estaba visible")

    game_path = plugins / "GameSettings.json"
    games = load_json(game_path, {})
    entry = games.get("CodemastersDirtRally2")
    if not isinstance(entry, dict):
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto y no tiene entrada CodemastersDirtRally2; "
                "se creara en el proximo arranque sin configurar."
            )
            return
        entry = {
            "UDPPort": SIMHUB_PORT,
            "UDPForwardPort": None,
            "UDPForwardIpAddress": "127.0.0.1",
            "TyreWearMinimum": None,
            "UDPForwardActive": False,
            "DisableConfigAlert": True,
            "OrderEx": 1,
            "LauncherSetting": None,
            "CurrentReader": None,
            "ManualConfigurationDismissed": True,
            "AddictionnalUDPRedirects": [],
            "AutomaticConfigurationDismissed": True,
            "TargetIP": "127.0.0.1",
            "LastActivations": [],
        }
        games["CodemastersDirtRally2"] = entry
        save_json(game_path, games)
        CHANGES.append("SimHub: entrada CodemastersDirtRally2 creada")
        return

    if simhub_running:
        WARNINGS.append(
            "SimHub esta abierto: no toco GameSettings.json; "
            "reinicialo para aplicar el puerto UDP."
        )
        return

    desired = {
        "UDPPort": SIMHUB_PORT,
        "UDPForwardPort": None,
        "UDPForwardIpAddress": "127.0.0.1",
        "UDPForwardActive": False,
        "DisableConfigAlert": True,
        "OrderEx": 1,
        "LauncherSetting": None,
        "CurrentReader": None,
        "ManualConfigurationDismissed": True,
        "AutomaticConfigurationDismissed": True,
        "TargetIP": "127.0.0.1",
    }
    changed = False
    for key, value in desired.items():
        if entry.get(key) != value:
            entry[key] = value
            changed = True
    if not isinstance(entry.get("AddictionnalUDPRedirects"), list):
        entry["AddictionnalUDPRedirects"] = []
        changed = True

    if changed:
        save_json(game_path, games)
        CHANGES.append(
            f"SimHub: DiRT Rally 2.0 escucha en UDP {SIMHUB_PORT}"
        )
    else:
        CHANGES.append("SimHub: DiRT Rally 2.0 ya estaba configurado")


def configure_crewchief() -> None:
    exe_dir = BOTTLE / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
    config_path = exe_dir / "CrewChiefV4.exe.config"
    if not config_path.exists():
        WARNINGS.append(f"no existe {config_path}")
        return

    if app_running("CrewChiefV4.exe"):
        WARNINGS.append(
            "CrewChief esta abierto: no toco su configuracion; "
            "reinicialo para aplicar DIRT_2 y el puerto UDP."
        )
        return

    text = config_path.read_text(encoding="utf-8-sig")
    original = text
    desired = {
        "last_game_definition": "DIRT_2",
        "dirt_rally_2_udp_data_port": str(CREWCHIEF_PORT),
        "dirt_rally_udp_data_port": str(CREWCHIEF_PORT),
        "enable_auto_detect": "False",
        "run_immediately": "True",
        "launch_dirt2": "False",
    }
    for name, value in desired.items():
        text, ok = set_xml_setting(text, name, value)
        if not ok:
            WARNINGS.append(f"no se pudo ajustar {name}")

    match = re.search(
        r'(<setting\s+name="limit_available_games"\s+serializeAs="String">\s*'
        r"<value>)(.*?)(</value>)",
        text,
        re.S,
    )
    if match:
        games = [item.strip() for item in match.group(2).split(",") if item.strip()]
        games = [item for item in games if item != "DIRT2"]
        if "DIRT_2" not in games:
            games.append("DIRT_2")
            text = text[: match.start(2)] + ",".join(games) + text[match.end(2):]

    if text != original:
        backup(config_path)
        write_atomic(config_path, text)
        CHANGES.append("CrewChief: juego activo DIRT_2")
    else:
        CHANGES.append("CrewChief: ya estaba configurado para DIRT_2")


def configure_crewchief_profile() -> None:
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        return
    for path in sorted(profiles.glob("*.json")):
        data = load_json(path, None)
        if not isinstance(data, dict):
            continue
        settings = data.get("userSettings")
        if not isinstance(settings, dict):
            continue

        changed = False
        if settings.get("last_game_definition") != "DIRT_2":
            settings["last_game_definition"] = "DIRT_2"
            changed = True
        if settings.get("enable_auto_detect") is not False:
            settings["enable_auto_detect"] = False
            changed = True
        if settings.get("run_immediately") is not True:
            settings["run_immediately"] = True
            changed = True
        if settings.get("launch_dirt2") is not False:
            settings["launch_dirt2"] = False
            changed = True

        for key in ("dirt_rally_2_udp_data_port", "dirt_rally_udp_data_port"):
            current = settings.get(key)
            value = str(CREWCHIEF_PORT) if isinstance(current, str) else CREWCHIEF_PORT
            if current != value:
                settings[key] = value
                changed = True

        if changed:
            save_json(path, data)
            CHANGES.append(f"CrewChief: perfil {path.name} en DIRT_2")


def configure_crewchief_user_config() -> None:
    root = BOTTLE / "drive_c/users/steamuser/AppData/Local/Britton_IT_Ltd"
    if not root.exists():
        return
    for path in sorted(root.rglob("user.config")):
        text = path.read_text(encoding="utf-8-sig")
        original = text
        desired = {
            "last_game_definition": "DIRT_2",
            "dirt_rally_2_udp_data_port": str(CREWCHIEF_PORT),
            "dirt_rally_udp_data_port": str(CREWCHIEF_PORT),
            "enable_auto_detect": "False",
            "run_immediately": "True",
            "launch_dirt2": "False",
        }
        for name, value in desired.items():
            text, ok = set_xml_setting(text, name, value)
            if not ok:
                WARNINGS.append(f"no se pudo ajustar {name} en {path.name}")
        if text != original:
            backup(path)
            write_atomic(path, text)
            CHANGES.append(f"CrewChief: user.config {path.name} en DIRT_2")


def main() -> int:
    if not PFX.exists():
        print(f"ERROR: no existe el prefijo {PFX}", file=sys.stderr)
        return 1
    if not BOTTLE.exists():
        print(f"ERROR: no existe la botella {BOTTLE}", file=sys.stderr)
        return 1

    configure_game()
    configure_simhub()
    configure_crewchief()
    configure_crewchief_profile()
    configure_crewchief_user_config()

    if CHANGES:
        print("Configuracion aplicada:")
        for item in CHANGES:
            print(f"  - {item}")
    else:
        print("No habia nada que cambiar.")
    if WARNINGS:
        print("Avisos:", file=sys.stderr)
        for item in WARNINGS:
            print(f"  - {item}", file=sys.stderr)
    print(f"Backups: {BACKUP_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

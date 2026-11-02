#!/usr/bin/env python3
"""Configura BeamNG.drive y SimHub para la telemetria OutGauge (UDP).

SimHub escucha el protocolo OutGauge de BeamNG en 127.0.0.1:63392 (su valor
por defecto), asi que aqui solo hay que:

  - activar OutGauge y motionSim en la configuracion de BeamNG;
  - decirle a SimHub el puerto y quitarle los avisos de configuracion.

No se instala ningun mod ni se toca la carpeta del juego. Si BeamNG esta
abierto, la configuracion no se aplicara hasta reiniciarlo (el juego reescribe
su settings.json al salir).
"""

from __future__ import annotations

import json
import os
import shutil
import sys
from datetime import datetime
from pathlib import Path

HOME = Path.home()
BOTTLE = Path(
    os.environ.get("SIMHUB_BOTTLE", HOME / ".local/share/bottles/bottles/simhub")
).expanduser()
RUN_DIR = os.environ.get("BEAMNG_RUN_DIR", "/tmp/etol/beamng_udp")
UDP_PORT = int(os.environ.get("BEAMNG_UDP_PORT", "63392"))
BACKUP_DIR = Path(
    os.environ.get("BEAMNG_BACKUP_DIR", HOME / ".local/share/etol/beamng/backups")
).expanduser()
STAMP = datetime.now().strftime("%Y%m%d-%H%M%S")

BEAMNG_USER = HOME / ".local/share/BeamNG/BeamNG.drive"
PLUGINS = BOTTLE / "drive_c/Program Files (x86)/SimHub/PluginsData"
GAME_KEY = "BeamNgDrive"

CHANGES: list[str] = []
WARNINGS: list[str] = []


def app_running(exe_name: str) -> bool:
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
        if needle in raw:
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
    tmp = path.with_name(path.name + ".tmp-etol-beamng")
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


def beamng_settings() -> list[Path]:
    """settings.json local y el de la nube (Steam arranca con -forceCloud on)."""
    paths: list[Path] = []
    current = BEAMNG_USER / "current/settings/settings.json"
    if current.exists():
        paths.append(current)
    for path in sorted(BEAMNG_USER.glob("*/settings/settings.json")):
        if path not in paths:
            paths.append(path)
    cloud = BEAMNG_USER / "current/settings/cloud/settings.json"
    if cloud.exists():
        paths.append(cloud)
    return paths


def configure_game() -> None:
    files = beamng_settings()
    if not files:
        WARNINGS.append(
            f"no encuentro settings.json de BeamNG en {BEAMNG_USER}; "
            "abre el juego una vez para que cree el perfil"
        )
        return

    running = app_running("BeamNG.drive.x64")
    # Solo OutGauge: con MotionSim en el mismo puerto, sus paquetes se mezclan
    # con los de OutGauge y SimHub los descarta (lo avisa la propia
    # documentacion de BeamNG y de SimHub).
    desired = {
        "protocols_outgauge_enabled": True,
        "protocols_outgauge_address": "127.0.0.1",
        "protocols_outgauge_port": UDP_PORT,
        "protocols_outgauge_maxUpdateRate": 60,
        "protocols_motionSim_enabled": False,
    }
    for path in files:
        data = load_json(path, {})
        changed = False
        for key, value in desired.items():
            if data.get(key) != value:
                data[key] = value
                changed = True
        if changed:
            save_json(path, data)
            CHANGES.append(
                f"BeamNG: OutGauge en 127.0.0.1:{UDP_PORT} (motionSim desactivado) "
                f"({path.parent.parent.name})"
            )
        else:
            CHANGES.append("BeamNG: los protocolos ya estaban configurados")

    if running:
        WARNINGS.append(
            "BeamNG esta abierto: reinicialo para que aplique la configuracion "
            "(el juego reescribe su settings.json al salir)"
        )


def configure_simhub() -> None:
    if not PLUGINS.is_dir():
        WARNINGS.append(f"no existe {PLUGINS}")
        return

    simhub_running = app_running("SimHubWPF.exe")

    hidden_path = PLUGINS / "HiddenGames.json"
    hidden = load_json(hidden_path, {})
    if hidden.get(GAME_KEY) is not False:
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco HiddenGames.json; "
                "BeamNG.drive deberia estar visible."
            )
        else:
            hidden[GAME_KEY] = False
            save_json(hidden_path, hidden)
            CHANGES.append("SimHub: BeamNG.drive visible en HiddenGames.json")
    else:
        CHANGES.append("SimHub: BeamNG.drive ya estaba visible")

    game_path = PLUGINS / "GameSettings.json"
    games = load_json(game_path, {})
    if not isinstance(games, dict) or not games:
        WARNINGS.append(f"no se pudo leer {game_path}")
        return

    entry = games.get(GAME_KEY)
    if not isinstance(entry, dict):
        WARNINGS.append(
            "SimHub no tiene la entrada BeamNgDrive en GameSettings.json"
        )
        return

    if simhub_running:
        WARNINGS.append(
            "SimHub esta abierto: no toco GameSettings.json; "
            "reinicialo para aplicar el puerto UDP."
        )
        return

    desired = {
        "UDPPort": UDP_PORT,
        "UDPForwardActive": False,
        "UDPForwardPort": None,
        "UDPForwardIpAddress": "127.0.0.1",
        # Modo "Outgauge + motionsim": es el que lee los paquetes OutGauge del
        # juego. Con "SH Telemetry mod" SimHub espera su propio mod y descarta
        # los de BeamNG.
        "CurrentReader": "BeamNgManager",
        "DisableConfigAlert": True,
        "AutomaticConfigurationDismissed": True,
        "ManualConfigurationDismissed": True,
    }
    changed = False
    for key, value in desired.items():
        if entry.get(key) != value:
            entry[key] = value
            changed = True
    if changed:
        save_json(game_path, games)
        CHANGES.append(f"SimHub: BeamNG.drive escucha en UDP {UDP_PORT}")
    else:
        CHANGES.append("SimHub: BeamNG.drive ya estaba configurado")


def main() -> int:
    configure_game()
    configure_simhub()

    print(f"BeamNG.drive (Linux nativo) -> SimHub por UDP {UDP_PORT}")
    print(f"  logs: {RUN_DIR}")
    for line in CHANGES:
        print(f"  · {line}")
    for line in WARNINGS:
        print(f"  ! {line}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

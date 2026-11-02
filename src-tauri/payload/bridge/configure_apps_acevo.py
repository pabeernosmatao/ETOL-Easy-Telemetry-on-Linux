#!/usr/bin/env python3
"""Configura SimHub y CrewChief para consumir el espejo AC EVO.

- SimHub: Assetto Corsa EVO visible y como juego preferente, lector por memoria
  compartida (CurrentReader null).
- CrewChief: juego activo ASSETTO_EVO y ASSETTO_EVO en la lista de juegos
  disponibles.

No toca ninguna opcion de LMU/AMS2, no arranca ni reinicia aplicaciones y no
modifica archivos de una app que este abierta (esa app los reescribiria al
cerrarse). Hace copia de seguridad antes de cada cambio.
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
_BOTTLE = os.environ.get("SIMHUB_BOTTLE", "").strip()
if not _BOTTLE:
    sys.exit(
        "ERROR: falta SIMHUB_BOTTLE. Ejecuta: etol detect --save "
        "o etol config set apps.bottle <ruta>"
    )
BOTTLE = Path(_BOTTLE).expanduser()
BACKUP_DIR = Path(os.environ.get(
    "ACEVO_MIRROR_BACKUP_DIR",
    HOME / ".local/share/acevo_mirror/backups",
)).expanduser()
STAMP = datetime.now().strftime("%Y%m%d-%H%M%S")
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
    tmp = path.with_name(path.name + ".tmp-acevo-mirror")
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


def configure_simhub() -> None:
    simhub_running = app_running("SimHubWPF.exe")
    plugins = BOTTLE / "drive_c/Program Files (x86)/SimHub/PluginsData"

    hidden_path = plugins / "HiddenGames.json"
    hidden = load_json(hidden_path, {})
    if hidden.get("AssettoCorsaEVO") is True:
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco HiddenGames.json; "
                "AssettoCorsaEVO deberia estar visible."
            )
        else:
            hidden["AssettoCorsaEVO"] = False
            save_json(hidden_path, hidden)
            CHANGES.append("SimHub: AssettoCorsaEVO visible en HiddenGames.json")
    else:
        CHANGES.append("SimHub: AssettoCorsaEVO ya estaba visible")

    game_path = plugins / "GameSettings.json"
    games = load_json(game_path, {})
    entry = games.get("AssettoCorsaEVO")
    if not isinstance(entry, dict):
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto y no tiene entrada AssettoCorsaEVO; "
                "se creara en el proximo arranque con --no-config desactivado."
            )
        else:
            games["AssettoCorsaEVO"] = {
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
                "TargetIP": None,
                "LastActivations": [],
            }
            save_json(game_path, games)
            CHANGES.append("SimHub: entrada AssettoCorsaEVO creada")
    else:
        changed = False
        if entry.get("OrderEx") != 1:
            if simhub_running:
                WARNINGS.append(
                    "SimHub esta abierto: no cambio OrderEx; selecciona "
                    "Assetto Corsa EVO manualmente si hace falta."
                )
            else:
                entry["OrderEx"] = 1
                changed = True
        if entry.get("CurrentReader") is not None:
            WARNINGS.append(
                "SimHub tiene un lector explicito para AssettoCorsaEVO; "
                "el espejo usa memoria compartida (acevo_pmf_*)."
            )
        if changed:
            save_json(game_path, games)
            CHANGES.append("SimHub: AssettoCorsaEVO marcado como preferente (OrderEx=1)")
        else:
            CHANGES.append("SimHub: AssettoCorsaEVO ya estaba configurado")


def set_xml_setting(text: str, name: str, value: str) -> tuple[str, bool]:
    pattern = re.compile(
        r'(<setting\s+name="' + re.escape(name) +
        r'"\s+serializeAs="String">\s*)'
        r'(<value\s*/>|<value>.*?</value>)',
        re.S,
    )
    literal = "<value>" + escape(value) + "</value>"
    new_text, count = pattern.subn(
        lambda match: match.group(1) + literal, text, count=1
    )
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


def configure_crewchief_profile(game_id: str = "") -> None:
    """Deja el perfil listo para el juego: autodetectado, arranque y lista."""
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        return
    desired = {"enable_auto_detect": False, "run_immediately": True}
    if game_id:
        desired["last_game_definition"] = game_id
    for path in sorted(profiles.glob("*.json")):
        data = load_json(path, None)
        if not isinstance(data, dict):
            continue
        user_settings = data.get("userSettings")
        if not isinstance(user_settings, dict):
            continue
        changed = False
        for name, value in desired.items():
            if user_settings.get(name) != value:
                user_settings[name] = value
                changed = True
        if game_id:
            available = [
                item.strip()
                for item in str(user_settings.get("limit_available_games", "")).split(",")
                if item.strip()
            ]
            if game_id not in available:
                available.append(game_id)
                user_settings["limit_available_games"] = ",".join(available)
                changed = True
        if changed:
            save_json(path, data)
            CHANGES.append(
                f"CrewChief: perfil {path.name} preparado para {game_id or 'el juego'}"
            )


def configure_crewchief_user_config() -> None:
    """Aplica el juego activo al user.config de CrewChief.

    El user.config de AppData/Local tiene prioridad sobre
    CrewChiefV4.exe.config. Sin esto, CrewChief arranca en el ultimo juego que
    se selecciono a mano aunque el .config diga otra cosa.
    """
    exe_dir = BOTTLE / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
    config_path = exe_dir / "CrewChiefV4.exe.config"
    if not config_path.exists():
        return

    text = config_path.read_text(encoding="utf-8-sig")
    values = {}
    for name in (
        "last_game_definition",
        "lmu_install_path",
        "enable_auto_detect",
        "run_immediately",
    ):
        match = re.search(
            r'<setting\s+name="' + re.escape(name) + r'"\s+serializeAs="String">\s*'
            r"<value>(.*?)</value>",
            text,
            re.S,
        )
        if match:
            values[name] = match.group(1).strip()
    if not values:
        return

    users = BOTTLE / "drive_c/users"
    for user_config in sorted(
        users.glob(
            "*/AppData/Local/Britton_IT_Ltd/CrewChiefV4.exe_Url_*/*/user.config"
        )
    ):
        text = user_config.read_text(encoding="utf-8-sig")
        new_text = text
        for name, value in values.items():
            new_text, _ = set_xml_setting(new_text, name, value)
        if new_text == text:
            continue
        backup(user_config)
        write_atomic(user_config, new_text)
        CHANGES.append(
            f"CrewChief: user.config de {user_config.parent.name} "
            f"({', '.join(values)})"
        )


def configure_simhub_plugins(enable_lmu: bool) -> None:
    """Activa o desactiva el plugin LMU de SimHub."""
    path = (
        BOTTLE
        / "drive_c/Program Files (x86)/SimHub/PluginsData/PluginsActivation.json"
    )
    if not path.exists():
        return
    data = load_json(path, None)
    if not isinstance(data, list):
        return
    changed = False
    for entry in data:
        if not isinstance(entry, dict):
            continue
        if entry.get("ClassName") == "LMU_SessionDataPlugin.LMU_SessionDataPlugin":
            if entry.get("IsEnabled") is not enable_lmu:
                entry["IsEnabled"] = enable_lmu
                changed = True
    if changed:
        save_json(path, data)
        estado = "activado" if enable_lmu else "desactivado"
        CHANGES.append(f"SimHub: LMU SessionDataPlugin {estado}")


def configure_crewchief() -> None:
    exe_dir = BOTTLE / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
    config_path = exe_dir / "CrewChiefV4.exe.config"
    if not config_path.exists():
        WARNINGS.append(f"no existe {config_path}")
        return

    if app_running("CrewChiefV4.exe"):
        WARNINGS.append(
            "CrewChief esta abierto: no toco su configuracion (la reescribe al "
            "cerrarse). Si no esta ya en Assetto Corsa EVO, cambialo en la app "
            "cuando no estes en pista."
        )
        return

    text = config_path.read_text(encoding="utf-8-sig")
    original = text
    desired = {
        "last_game_definition": "ASSETTO_EVO",
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
        if "ASSETTO_EVO" not in games:
            games.append("ASSETTO_EVO")
            text = text[:m.start(2)] + ",".join(games) + text[m.end(2):]

    if text != original:
        backup(config_path)
        write_atomic(config_path, text)
        CHANGES.append("CrewChief: juego activo ASSETTO_EVO")
    else:
        CHANGES.append("CrewChief: ya estaba configurado para ASSETTO_EVO")

    configure_crewchief_profile("ASSETTO_EVO")

    configure_crewchief_user_config()


def main() -> int:
    if not BOTTLE.exists():
        print(f"ERROR: no existe la botella: {BOTTLE}", file=sys.stderr)
        return 1
    configure_simhub()
    configure_simhub_plugins(False)
    configure_crewchief()

    if CHANGES:
        print("Configuracion aplicada:")
        for item in CHANGES:
            print(f"  - {item}")
    else:
        print("No habia nada que cambiar.")
    if WARNINGS:
        print("Avisos:")
        for item in WARNINGS:
            print(f"  - {item}")
    print(f"Copias de seguridad: {BACKUP_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

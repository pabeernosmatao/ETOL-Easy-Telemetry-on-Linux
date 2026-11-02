#!/usr/bin/env python3
"""Configura SimHub y CrewChief para consumir el espejo LMU.

- SimHub: LMU visible en el selector de juegos.
- CrewChief: juego activo LMU, sin comprobacion/instalacion del plugin en el
  juego (el puente ya publica los buffers SMMP) y sin pit manager de escritura.

Hace copia de seguridad antes de tocar cada archivo. No arranca ni reinicia
ninguna aplicacion.
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
    "LMU_MIRROR_BACKUP_DIR",
    HOME / ".local/share/lmu_mirror/backups",
)).expanduser()
STAMP = datetime.now().strftime("%Y%m%d-%H%M%S")
CHANGES: list[str] = []


def backup(path: Path) -> None:
    if not path.exists():
        return
    BACKUP_DIR.mkdir(parents=True, exist_ok=True)
    dest = BACKUP_DIR / f"{path.name}.{STAMP}.bak"
    shutil.copy2(path, dest)
    CHANGES.append(f"backup: {dest}")


def write_atomic(path: Path, text: str) -> None:
    tmp = path.with_name(path.name + ".tmp-lmu-mirror")
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
    plugins = BOTTLE / "drive_c/Program Files (x86)/SimHub/PluginsData"
    hidden_path = plugins / "HiddenGames.json"
    hidden = load_json(hidden_path, {})
    if hidden.get("LMU") is not False:
        hidden["LMU"] = False
        save_json(hidden_path, hidden)
        CHANGES.append("SimHub: LMU visible en HiddenGames.json")

    game_path = plugins / "GameSettings.json"
    games = load_json(game_path, {})
    if "LMU" not in games:
        games["LMU"] = {
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
        CHANGES.append("SimHub: entrada LMU creada en GameSettings.json")
    elif games["LMU"].get("OrderEx") != 1:
        games["LMU"]["OrderEx"] = 1
        save_json(game_path, games)
        CHANGES.append("SimHub: LMU marcado como juego preferente (OrderEx=1)")


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


def configure_crewchief_profile() -> None:
    """Ajusta el perfil activo de CrewChief.

    CrewChief guarda muchos ajustes en
    Documents/CrewChiefV4/Profiles/<perfil>.json y esos valores tienen
    prioridad sobre CrewChiefV4.exe.config. Si no se toca el perfil, el
    comprobador del plugin y el pit manager siguen activos aunque el .config
    diga lo contrario.
    """
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        print(f"AVISO: no existe el directorio de perfiles {profiles}",
              file=sys.stderr)
        return

    desired = {
        "last_game_definition": "LMU",
        "enable_lmu_rf2_plugin_dll_check": False,
        "use_legacy_rf2_shared_memory_filenames": False,
        "enable_auto_detect": False,
        # Arranca CrewChief al abrir la app. Sin esto, el proceso queda en la
        # pantalla inicial y no conecta con la memoria compartida hasta pulsar
        # "Start Crew Chief".
        "run_immediately": True,
    }
    if os.environ.get("LMU_MIRROR_KEEP_PIT_MANAGER") != "1":
        desired["lmu_enable_pit_manager"] = False

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
        if changed:
            save_json(path, data)
            CHANGES.append(
                f"CrewChief: perfil {path.name} sin plugin-check ni pit manager"
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
        print(f"AVISO: no existe {config_path}", file=sys.stderr)
        return

    text = config_path.read_text(encoding="utf-8-sig")
    original = text
    desired = {
        "last_game_definition": "LMU",
        "lmu_install_path": "Z:" + os.environ.get(
            "LMU_MIRROR_RUN_DIR", "/tmp/etol/lmu_mirror"
        ).replace("/", "\\"),
        "use_legacy_rf2_shared_memory_filenames": "False",
        "enable_lmu_rf2_plugin_dll_check": "False",
        "enable_auto_detect": "False",
        "run_immediately": "True",
    }
    if os.environ.get("LMU_MIRROR_KEEP_PIT_MANAGER") != "1":
        desired["lmu_enable_pit_manager"] = "False"

    for name, value in desired.items():
        text, ok = set_xml_setting(text, name, value)
        if not ok:
            print(f"AVISO: no se pudo ajustar {name}", file=sys.stderr)

    # Asegura que LMU siga en la lista de juegos disponibles.
    m = re.search(
        r'(<setting\s+name="limit_available_games"\s+serializeAs="String">\s*'
        r'<value>)(.*?)(</value>)',
        text,
        re.S,
    )
    if m:
        games = [x.strip() for x in m.group(2).split(",") if x.strip()]
        if "LMU" not in games:
            games.append("LMU")
            text = text[:m.start(2)] + ",".join(games) + text[m.end(2):]

    if text != original:
        backup(config_path)
        write_atomic(config_path, text)
        CHANGES.append("CrewChief: juego activo LMU y modo de solo lectura")
    else:
        CHANGES.append("CrewChief: ya estaba configurado")

    configure_crewchief_profile()

    configure_crewchief_user_config()


def main() -> int:
    if not BOTTLE.exists():
        print(f"ERROR: no existe la botella: {BOTTLE}", file=sys.stderr)
        return 1
    configure_simhub()
    configure_simhub_plugins(True)
    configure_crewchief()

    if CHANGES:
        print("Configuracion aplicada:")
        for item in CHANGES:
            print(f"  - {item}")
    else:
        print("No habia nada que cambiar.")
    print(f"Copias de seguridad: {BACKUP_DIR}")
    print("Si CrewChief estaba abierto, cierralo y vuelve a abrirlo para que "
          "tome el juego LMU.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

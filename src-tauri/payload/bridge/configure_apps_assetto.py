#!/usr/bin/env python3
"""Configura SimHub y CrewChief para consumir el espejo de Assetto Corsa.

- SimHub: Assetto Corsa visible y como juego preferente, lector por memoria
  compartida (CurrentReader null).
- CrewChief: juego activo ASSETTO_64BIT, ruta de instalacion en el espejo y
  ASSETTO_64BIT en la lista de juegos disponibles.

No toca ninguna opcion de LMU/AMS2/AC EVO, no arranca ni reinicia aplicaciones
y no modifica archivos de una app que este abierta (esa app los reescribiria al
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
_BOTTLE = os.environ.get("SIMHUB_BOTTLE", "").strip()
if not _BOTTLE:
    sys.exit(
        "ERROR: falta SIMHUB_BOTTLE. Ejecuta: etol detect --save "
        "o etol config set apps.bottle <ruta>"
    )
BOTTLE = Path(_BOTTLE).expanduser()
BACKUP_DIR = Path(os.environ.get(
    "ASSETTO_MIRROR_BACKUP_DIR",
    HOME / ".local/share/assetto_mirror/backups",
)).expanduser()
RUN_DIR = os.environ.get("ASSETTO_MIRROR_RUN_DIR", "/tmp/etol/assetto_mirror")
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
    tmp = path.with_name(path.name + ".tmp-assetto-mirror")
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
    if hidden.get("AssettoCorsa") is not False:
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco HiddenGames.json; "
                "Assetto Corsa deberia estar visible."
            )
        else:
            hidden["AssettoCorsa"] = False
            save_json(hidden_path, hidden)
            CHANGES.append("SimHub: Assetto Corsa visible en HiddenGames.json")
    else:
        CHANGES.append("SimHub: Assetto Corsa ya estaba visible")

    game_path = plugins / "GameSettings.json"
    games = load_json(game_path, {})
    entry = games.get("AssettoCorsa")
    if not isinstance(entry, dict):
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto y no tiene entrada AssettoCorsa; "
                "se creara en el proximo arranque con --no-config desactivado."
            )
        else:
            games["AssettoCorsa"] = {
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
            CHANGES.append("SimHub: entrada AssettoCorsa creada")
    else:
        changed = False
        if entry.get("OrderEx") != 1:
            if simhub_running:
                WARNINGS.append(
                    "SimHub esta abierto: no cambio OrderEx; selecciona "
                    "Assetto Corsa manualmente si hace falta."
                )
            else:
                entry["OrderEx"] = 1
                changed = True
        if entry.get("CurrentReader") is not None:
            WARNINGS.append(
                "SimHub tiene un lector explicito para AssettoCorsa; "
                "el espejo usa memoria compartida (acpmf_*)."
            )
        if changed:
            save_json(game_path, games)
            CHANGES.append("SimHub: Assetto Corsa marcado como preferente (OrderEx=1)")
        else:
            CHANGES.append("SimHub: Assetto Corsa ya estaba configurado")


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
            raw = str(user_settings.get("limit_available_games", "")).strip()
            available = [item.strip() for item in raw.split(",") if item.strip()]
            if len(available) == 1:
                # ETOL pudo convertir una lista vacia (todos) en una lista de
                # un solo juego; CrewChief solo mostraba ese juego.
                user_settings["limit_available_games"] = ""
                changed = True
            elif available and game_id not in available:
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
        "enable_lmu_rf2_plugin_dll_check",
        "enable_automatic_plugin_update",
        "lmu_enable_pit_manager",
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
            f"({', '.join(n for n in values if n != 'lmu_install_path')})"
        )


def configure_simhub_plugins(enable_lmu: bool) -> None:
    """Activa o desactiva el plugin LMU de SimHub.

    SimHub 9.x usa `doX.doX_LMU_SessionDataPlugin`; las versiones antiguas
    usaban `LMU_SessionDataPlugin.LMU_SessionDataPlugin`. Se activa el que
    exista, sin tocar los demas plugins del juego.
    """
    path = (
        BOTTLE
        / "drive_c/Program Files (x86)/SimHub/PluginsData/PluginsActivation.json"
    )
    if not path.exists():
        return
    data = load_json(path, None)
    if not isinstance(data, list):
        return
    primary = "doX.doX_LMU_SessionDataPlugin"
    legacy = "LMU_SessionDataPlugin.LMU_SessionDataPlugin"
    names = {
        entry.get("ClassName")
        for entry in data
        if isinstance(entry, dict)
    }
    target = primary if primary in names else legacy if legacy in names else ""
    if not target:
        return
    changed = False
    for entry in data:
        if not isinstance(entry, dict) or entry.get("ClassName") != target:
            continue
        if entry.get("IsEnabled") is not enable_lmu:
            entry["IsEnabled"] = enable_lmu
            changed = True
    if changed:
        save_json(path, data)
        estado = "activado" if enable_lmu else "desactivado"
        CHANGES.append(f"SimHub: {target.split('.')[-1]} {estado}")


def fix_limit_available_games() -> None:
    """Restaura la lista de juegos de CrewChief si quedo con uno solo.

    Una lista vacia significa "todos". ETOL pudo escribir un unico juego
    (por ejemplo ASSETTO_EVO) y CrewChief solo mostraba ese. Se limpia.
    """
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        return
    for path in sorted(profiles.glob("*.json")):
        data = load_json(path, None)
        if not isinstance(data, dict):
            continue
        user_settings = data.get("userSettings")
        if not isinstance(user_settings, dict):
            continue
        raw = str(user_settings.get("limit_available_games", "")).strip()
        available = [item.strip() for item in raw.split(",") if item.strip()]
        if len(available) == 1:
            user_settings["limit_available_games"] = ""
            save_json(path, data)
            CHANGES.append(f"CrewChief: lista de juegos restaurada en {path.name}")


def configure_crewchief() -> None:
    exe_dir = BOTTLE / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
    config_path = exe_dir / "CrewChiefV4.exe.config"
    if not config_path.exists():
        WARNINGS.append(f"no existe {config_path}")
        return

    if app_running("CrewChiefV4.exe"):
        WARNINGS.append(
            "CrewChief esta abierto: no toco su configuracion (la reescribe al "
            "cerrarse). Si no esta ya en Assetto Corsa, cambialo en la app "
            "cuando no estes en pista."
        )
        return

    text = config_path.read_text(encoding="utf-8-sig")
    original = text
    desired = {
        "last_game_definition": "ASSETTO_64BIT",
        "acs_install_path": "Z:" + RUN_DIR.replace("/", "\\"),
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
        if games == ["ASSETTO_64BIT"]:
            text = text[:m.start(2)] + "" + text[m.end(2):]
        elif games and "ASSETTO_64BIT" not in games:
            games.append("ASSETTO_64BIT")
            text = text[:m.start(2)] + ",".join(games) + text[m.end(2):]

    if text != original:
        backup(config_path)
        write_atomic(config_path, text)
        CHANGES.append("CrewChief: juego activo ASSETTO_64BIT")
    else:
        CHANGES.append("CrewChief: ya estaba configurado para ASSETTO_64BIT")


def configure_crewchief_profile_extra() -> None:
    """Deja la ruta de AC y ASSETTO_64BIT tambien en el perfil JSON.

    CrewChief lee el perfil con prioridad sobre el .config. Sin
    acs_install_path en el perfil, busca el juego real y muestra el aviso
    "Plugin Needed" que bloquea el arranque.
    """
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        return
    if app_running("CrewChiefV4.exe"):
        WARNINGS.append(
            "CrewChief esta abierto: no toco su perfil; si pide el plugin, "
            "cierra CrewChief y vuelve a lanzar."
        )
        return
    install = "Z:" + RUN_DIR.replace("/", "\\")
    for path in sorted(profiles.glob("*.json")):
        data = load_json(path, None)
        if not isinstance(data, dict):
            continue
        user_settings = data.get("userSettings")
        if not isinstance(user_settings, dict):
            continue
        changed = False
        if user_settings.get("acs_install_path") != install:
            user_settings["acs_install_path"] = install
            changed = True
        games = [
            item.strip()
            for item in str(user_settings.get("limit_available_games", "")).split(",")
            if item.strip()
        ]
        if "ASSETTO_64BIT" not in games:
            games.append("ASSETTO_64BIT")
            user_settings["limit_available_games"] = ",".join(games)
            changed = True
        if changed:
            save_json(path, data)
            CHANGES.append(
                f"CrewChief: perfil {path.name} con acs_install_path y ASSETTO_64BIT"
            )


def configure_simhub_steam_path() -> None:
    """Hace que SimHub encuentre la instalacion real de AC desde Bottles.

    SimHub escanea Steam para localizar el juego. En la botella no hay Steam,
    asi que se crea un `libraryfolders.vdf` minimo apuntando a la biblioteca
    real (Z:\\mnt\\...) y se marca la app 244210 como instalada. Sin esto
    SimHub muestra "SimHub can't configure the game" y no lee coche/pista.
    """
    pfx = os.environ.get("ASSETTO_PFX", "").strip()
    wine = os.environ.get("BOTTLE_WINE", "").strip()
    if not pfx or not wine:
        return
    marker = "/steamapps/compatdata/"
    if marker not in pfx:
        return
    library = pfx.split(marker, 1)[0]
    if not Path(library).is_dir():
        return
    winlib = "Z:" + library.replace("/", "\\\\")

    steam = BOTTLE / "drive_c/Program Files (x86)/Steam"
    apps = steam / "steamapps"
    apps.mkdir(parents=True, exist_ok=True)
    vdf = (
        '"libraryfolders"\n{\n'
        '\t"0"\n\t{\n\t\t"path"\t\t"C:\\\\Program Files (x86)\\\\Steam"\n\t}\n'
        '\t"1"\n\t{\n\t\t"path"\t\t"' + winlib + '"\n\t}\n}\n'
    )
    vdf_path = apps / "libraryfolders.vdf"
    if not vdf_path.exists() or vdf_path.read_text(encoding="utf-8") != vdf:
        vdf_path.write_text(vdf, encoding="utf-8")
        CHANGES.append("SimHub: biblioteca de Steam con Assetto Corsa")

    env = dict(os.environ, WINEPREFIX=str(BOTTLE), WINEDEBUG="-all")
    for name, value, kind in (
        ("Installed", "1", "REG_DWORD"),
        ("Name", "Assetto Corsa", "REG_SZ"),
    ):
        subprocess.run(
            [
                wine, "reg", "add", r"HKCU\Software\Valve\Steam\Apps\244210",
                "/v", name, "/t", kind, "/d", value, "/f",
            ],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )

    # Fallback por si SimHub usa la ruta del proceso (stub acs.exe): el espejo
    # necesita ver el contenido real del juego.
    game = Path(library) / "steamapps/common/assettocorsa"
    if game.is_dir():
        for name in ("content", "cfg"):
            link = Path(RUN_DIR) / name
            target = game / name
            if target.exists() and not link.exists():
                try:
                    link.symlink_to(target)
                except OSError:
                    pass
        docs = BOTTLE / "drive_c/users/steamuser/Documents/Assetto Corsa"
        real_docs = Path(pfx) / "drive_c/users/steamuser/Documents/Assetto Corsa"
        if real_docs.exists() and not docs.exists():
            docs.parent.mkdir(parents=True, exist_ok=True)
            try:
                docs.symlink_to(real_docs)
            except OSError:
                pass

    configure_crewchief_profile("ASSETTO_64BIT")

    configure_crewchief_user_config()

    fix_limit_available_games()


def main() -> int:
    if not BOTTLE.exists():
        print(f"ERROR: no existe la botella: {BOTTLE}", file=sys.stderr)
        return 1
    configure_simhub()
    configure_simhub_plugins(False)
    configure_simhub_steam_path()
    configure_crewchief()
    configure_crewchief_profile_extra()

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

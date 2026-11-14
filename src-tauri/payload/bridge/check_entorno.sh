#!/usr/bin/env bash
# Comprueba, sin modificar nada, que el entorno del puente LMU -> Bottles
# existe y esta en su sitio. Sirve antes de activar el puente y, sobre todo,
# despues de reinstalar el sistema o mover carpetas.
#
# Uso: ./bridge/check_entorno.sh [-v]
#   -v   muestra tambien los detalles de lo que si esta bien
#
# Sale con 0 si lo esencial esta, 1 si falta algo imprescindible.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
VERBOSE=0
[[ "${1:-}" == "-v" ]] && VERBOSE=1

LMU_PROTON="${LMU_PROTON:-}"
if [[ -z "$LMU_PROTON" ]]; then
  echo "ERROR: falta LMU_PROTON. Ejecuta: etol detect --save o etol config set games.lmu.proton <ruta>" >&2
  exit 1
fi
LMU_PFX="${LMU_PFX:-}"
if [[ -z "$LMU_PFX" ]]; then
  echo "ERROR: falta LMU_PFX. Ejecuta: etol detect --save o etol config set games.lmu.prefix <ruta>" >&2
  exit 1
fi
LMU_WINE="$LMU_PROTON/files/bin/wine"

LMU_GAME_DIR="${LMU_GAME_DIR:-}"
if [[ -z "$LMU_GAME_DIR" ]]; then
  _lmu_lib="${LMU_PFX%/steamapps/compatdata/*}"
  if [[ "$_lmu_lib" != "$LMU_PFX" ]]; then
    LMU_GAME_DIR="$_lmu_lib/steamapps/common/Le Mans Ultimate"
  fi
fi
if [[ -n "$LMU_GAME_DIR" && -d "$LMU_GAME_DIR" ]]; then
  mkdir -p "$RUN_DIR/Plugins" "$RUN_DIR/UserData/player"
  cp -f "$LMU_GAME_DIR/Plugins/"*.dll "$RUN_DIR/Plugins/" 2>/dev/null || true
  cp -f "$LMU_GAME_DIR/UserData/player/CustomPluginVariables.JSON" \
        "$RUN_DIR/UserData/player/CustomPluginVariables.JSON" 2>/dev/null || true
fi
LMU_GAME="${LMU_GAME:-$HOME/.local/share/Steam/steamapps/common/Le Mans Ultimate}"

BOTTLE="${SIMHUB_BOTTLE:-}"
if [[ -z "$BOTTLE" ]]; then
  echo "ERROR: falta SIMHUB_BOTTLE. Ejecuta: etol detect --save o etol config set apps.bottle <ruta>" >&2
  exit 1
fi
BOTTLE_WINE="${BOTTLE_WINE:-}"
if [[ -z "$BOTTLE_WINE" ]]; then
  echo "ERROR: falta BOTTLE_WINE. Ejecuta: etol detect --save o etol config set apps.wine <ruta>" >&2
  exit 1
fi
SIMHUB_DIR="$BOTTLE/drive_c/Program Files (x86)/SimHub"
CREW_DIR="$BOTTLE/drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
STEAM_LINK="$BOTTLE/drive_c/Program Files (x86)/steam"

ZIG="${ZIG:-$ROOT/../tools/zig/zig}"
SDK="$LMU_GAME/Support/SharedMemoryInterface"

FAILS=0
WARNS=0

ok()   { printf '  OK    %s\n' "$*"; }
bad()  { printf '  FALLA %s\n' "$*"; FAILS=$((FAILS + 1)); }
warn() { printf '  AVISO %s\n' "$*"; WARNS=$((WARNS + 1)); }

need_file() {
  local path="$1" label="$2"
  if [[ -f "$path" ]]; then
    [[ "$VERBOSE" == 1 ]] && ok "$label: $path"
    return 0
  fi
  bad "$label no existe: $path"
  return 1
}

need_exec() {
  local path="$1" label="$2"
  if [[ -x "$path" ]]; then
    [[ "$VERBOSE" == 1 ]] && ok "$label: $path"
    return 0
  fi
  bad "$label no es ejecutable o no existe: $path"
  return 1
}

echo "== Prefijo de LMU =="
need_exec "$LMU_WINE" "wine de LMU" || true
need_file "$LMU_PFX/system.reg" "prefijo de LMU" || true
if [[ -d "$LMU_GAME" ]]; then
  [[ "$VERBOSE" == 1 ]] && ok "juego: $LMU_GAME"
else
  bad "no encuentro el juego en: $LMU_GAME"
fi

echo "== Botella de Bottles =="
need_file "$BOTTLE/system.reg" "botella" || true
need_exec "$BOTTLE_WINE" "wine de Bottles" || true
need_file "$SIMHUB_DIR/SimHubWPF.exe" "SimHub" || true
need_file "$CREW_DIR/CrewChiefV4.exe" "CrewChief" || true

echo "== Ruta del juego dentro de la botella =="
if [[ -e "$STEAM_LINK" ]]; then
  target="$(readlink -f "$STEAM_LINK" 2>/dev/null || true)"
  if [[ -n "$target" && -d "$target/steamapps/common/Le Mans Ultimate" ]]; then
    ok "enlace steam -> $target"
  else
    bad "el enlace $STEAM_LINK no apunta a una instalacion de Steam con LMU"
  fi
else
  bad "falta el enlace: $STEAM_LINK -> ~/.local/share/Steam"
  echo "        SimHub/CrewChief buscan 'C:\\Program Files (x86)\\Steam\\SteamApps\\common\\Le Mans Ultimate'."
  echo "        Crear con: ln -s \"\$HOME/.local/share/Steam\" \"$STEAM_LINK\""
fi

echo "== Binarios del puente =="
if need_file "$HERE/lmu_shm_mirror.exe" "espejo compilado"; then
  :
fi
if need_file "$HERE/lmu_process_stub.exe" "stub compilado"; then
  :
fi
if [[ -x "$ZIG" && -f "$SDK/SharedMemoryInterface.hpp" ]]; then
  [[ "$VERBOSE" == 1 ]] && ok "se puede recompilar (Zig + SDK)"
elif [[ -f "$HERE/lmu_shm_mirror.exe" && -f "$HERE/lmu_process_stub.exe" ]]; then
  warn "no se puede recompilar (falta Zig o el SDK); los .exe actuales sirven"
else
  bad "faltan los .exe y no se pueden compilar (falta Zig o $SDK)"
fi

echo "== Configuracion de las apps =="
PROFILE="$BOTTLE/drive_c/users/steamuser/Documents/CrewChiefV4/Profiles/defaultSettings.json"
if [[ -f "$PROFILE" ]]; then
  grep -q '"enable_lmu_rf2_plugin_dll_check": *false' "$PROFILE" \
    && ok "CrewChief: plugin check desactivado" \
    || warn "CrewChief: enable_lmu_rf2_plugin_dll_check no esta en false"
  grep -q '"lmu_enable_pit_manager": *false' "$PROFILE" \
    && ok "CrewChief: pit manager desactivado" \
    || warn "CrewChief: lmu_enable_pit_manager no esta en false (escribe en LMU)"
else
  warn "no encuentro el perfil de CrewChief: $PROFILE"
fi

echo "== Procesos =="
for mode in receiver sender; do
  if pgrep -f -- "lmu_shm_mirror\\.exe --$mode" >/dev/null 2>&1; then
    ok "espejo --$mode en marcha"
  else
    warn "espejo --$mode parado (normal si aun no has activado el puente)"
  fi
done
pgrep -f -- 'Le Mans Ultimate[.]exe' | while read -r pid; do
  [[ -r "/proc/$pid/cmdline" ]] || continue
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline")"
  norm="${cmd//\\//}"
  [[ "$norm" == *"/tmp/etol/lmu_mirror/Le Mans Ultimate.exe"* ]] && echo "  OK    stub de deteccion en marcha"
done
if ! pgrep -f -- 'lmu_bridge_watchdog\.sh' >/dev/null 2>&1; then
  warn "vigilante parado"
else
  ok "vigilante en marcha"
fi

echo
if [[ "$FAILS" -gt 0 ]]; then
  echo "Resultado: faltan $FAILS cosa(s) imprescindible(s)."
  exit 1
fi
if [[ "$WARNS" -gt 0 ]]; then
  echo "Resultado: entorno listo con $WARNS aviso(s). Ejecuta ./activar_puente_lmu.sh"
else
  echo "Resultado: entorno listo. Ejecuta ./activar_puente_lmu.sh"
fi

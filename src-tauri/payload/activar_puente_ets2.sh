#!/usr/bin/env bash
# Activa el puente de Euro Truck Simulator 2 (version nativa Linux).
#
# ETS2 para Linux escribe la telemetria del SDK de SCS en /dev/shm/SCSTelemetry.
# SimHub corre bajo Wine en el prefijo de Bottles y espera un mapping con
# nombre. El espejo de ETOL copia los bytes tal cual (mismo layout en Windows
# y Linux) y publica `Local\SCSTelemetry`. Es de solo lectura: no escribe en el
# juego ni en su memoria.
#
# Este puente solo alimenta SimHub. CrewChief no tiene lector de camiones.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
MIRROR="$BRIDGE/ets2_shm_mirror.exe"
PLUGIN="$BRIDGE/libscs-telemetry.so"
CONFIG_PY="$BRIDGE/configure_apps_ets2.py"
STUB_SRC="$BRIDGE/ets2_process_stub.exe"
RUN_DIR="${ETS2_MIRROR_RUN_DIR:-/tmp/etol/ets2_bridge}"
STUB_EXE="$RUN_DIR/eurotrucks2.exe"

BOTTLE="${SIMHUB_BOTTLE:-}"
if [[ -z "$BOTTLE" ]]; then
  echo "ERROR: falta SIMHUB_BOTTLE. Ejecuta: etol detect --save" >&2
  exit 1
fi
BOTTLE_WINE="${BOTTLE_WINE:-}"
if [[ -z "$BOTTLE_WINE" ]]; then
  echo "ERROR: falta BOTTLE_WINE. Ejecuta: etol detect --save" >&2
  exit 1
fi

# Carpeta del juego. ETOL la pasa en ETS2_GAME_DIR; si no, se deduce del
# ejecutable nativo que este mismo script puede encontrar en Steam.
GAME_DIR="${ETS2_GAME_DIR:-}"
if [[ -z "$GAME_DIR" ]]; then
  for candidate in \
    "/mnt/GAMES/SteamLibrary/steamapps/common/Euro Truck Simulator 2" \
    "$HOME/.local/share/Steam/steamapps/common/Euro Truck Simulator 2"; do
    if [[ -x "$candidate/bin/linux_x64/eurotrucks2" ]]; then
      GAME_DIR="$candidate"
      break
    fi
  done
fi

SHM_PATH="/dev/shm/SCSTelemetry"
SOURCE_WIN='Z:\dev\shm\SCSTelemetry'
MAP_NAME='Local\SCSTelemetry'
ALIAS_NAME="${ETS2_ALIAS_MAP:-Local\SHSCSTelemetry}"

DO_CONFIG=1
SOLO_CONFIG=0
DO_HF8=1
DO_MIRROR=1
DO_PLUGIN=1
DO_STUB=1

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --no-config    no tocar la configuracion de SimHub
  --solo-config  aplica la configuracion y sale, sin arrancar el espejo
  --no-hf8       no configurar el pad haptico HF8
  --no-plugin    no instalar el plugin SCS en el juego
  --no-mirror    no arrancar el espejo (solo plugin y configuracion)
  --no-stub      no arrancar el proceso inerte eurotrucks2.exe
  -h, --help     esta ayuda

Origen: $SHM_PATH -> $MAP_NAME
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-config) DO_CONFIG=0; shift ;;
    --solo-config) SOLO_CONFIG=1; shift ;;
    --no-hf8) DO_HF8=0; shift ;;
    --no-plugin) DO_PLUGIN=0; shift ;;
    --no-mirror) DO_MIRROR=0; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$RUN_DIR"
export SIMHUB_BOTTLE="$BOTTLE" BOTTLE_WINE="$BOTTLE_WINE"
export ETS2_MIRROR_RUN_DIR="$RUN_DIR"
[[ -n "$GAME_DIR" ]] && export ETS2_GAME_DIR="$GAME_DIR"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

game_running() {
  pgrep -x eurotrucks2 >/dev/null 2>&1
}

# ---------------------------------------------------------------------------
# 1. Plugin SCS en el juego (necesario para que el juego publique la memoria)
# ---------------------------------------------------------------------------
install_plugin() {
  if [[ "$DO_PLUGIN" != 1 ]]; then
    echo "plugin SCS omitido (--no-plugin)."
    return 0
  fi
  if [[ -z "$GAME_DIR" ]]; then
    echo "AVISO: no encuentro la carpeta de ETS2; no instalo el plugin SCS." >&2
    echo "       Copia a mano $PLUGIN en bin/linux_x64/plugins/." >&2
    return 0
  fi
  if [[ ! -f "$PLUGIN" ]]; then
    echo "AVISO: falta $PLUGIN" >&2
    echo "       Sin el, ETS2 no publicara telemetria." >&2
    return 1
  fi

  local dir="$GAME_DIR/bin/linux_x64/plugins"
  local dest="$dir/libscs-telemetry.so"
  if [[ -f "$dest" ]] && cmp -s "$PLUGIN" "$dest"; then
    echo "Plugin SCS ya instalado en $dest"
    return 0
  fi

  if ! mkdir -p "$dir" 2>/dev/null; then
    echo "AVISO: no puedo crear $dir" >&2
    echo "       Instala el plugin a mano (hace falta permiso de escritura)." >&2
    return 1
  fi
  if ! install -m 644 "$PLUGIN" "$dest" 2>/dev/null; then
    echo "AVISO: no puedo escribir $dest" >&2
    return 1
  fi

  echo "Plugin SCS instalado: $dest"
  if game_running; then
    echo "AVISO: ETS2 esta abierto; cierra el juego y vuelve a abrirlo para" >&2
    echo "       que cargue el plugin y empiece a publicar telemetria." >&2
  fi
}

# ---------------------------------------------------------------------------
# 2. Espejo /dev/shm/SCSTelemetry -> Local\SCSTelemetry
# ---------------------------------------------------------------------------
mirror_pid() {
  local pidfile="$RUN_DIR/mirror.pid" pid
  [[ -f "$pidfile" ]] || return 1
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  kill -0 "$pid" 2>/dev/null || return 1
  printf '%s\n' "$pid"
}

find_mirror_pids() {
  local pid base args
  for pid in $(pgrep -f -- 'ets2_shm_mirror[.]exe' 2>/dev/null || true); do
    [[ -r "/proc/$pid/cmdline" ]] || continue
    base="$(tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | head -n1)"
    base="${base//\\//}"
    base="${base##*/}"
    # Solo el espejo (o su envoltorio start.exe), nunca una shell que
    # mencione la ruta.
    if [[ "$base" == "ets2_shm_mirror.exe" ]]; then
      printf '%s\n' "$pid"
      continue
    fi
    if [[ "$base" == "start.exe" ]]; then
      args="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
      [[ "${args//\\//}" == *"ets2_shm_mirror.exe"* ]] && printf '%s\n' "$pid"
    fi
  done
}

start_mirror() {
  if [[ "$DO_MIRROR" != 1 ]]; then
    echo "espejo omitido (--no-mirror)."
    return 0
  fi
  if [[ ! -x "$MIRROR" ]]; then
    fail "falta $MIRROR (compilalo con scripts/juegos/ets2/bridge/build_ets2_mirror.sh)"
  fi
  if [[ ! -x "$BOTTLE_WINE" ]]; then
    fail "no existe el wine de Bottles: $BOTTLE_WINE"
  fi

  local running
  running="$(mirror_pid || true)"
  if [[ -n "$running" ]]; then
    echo "Espejo ya en marcha (PID $running)."
    return 0
  fi
  rm -f "$RUN_DIR/mirror.pid"

  local args=(
    --source "$SOURCE_WIN"
    --map "$MAP_NAME"
    --size 32768
    --interval 16
  )
  [[ -n "$ALIAS_NAME" ]] && args+=(--alias "$ALIAS_NAME")

  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all \
    "$BOTTLE_WINE" "$MIRROR" "${args[@]}" \
    >"$RUN_DIR/mirror.log" 2>&1 </dev/null &
  echo "$!" > "$RUN_DIR/mirror.pid"

  local ready=0 i
  for i in $(seq 1 80); do
    if grep -q "mapping .* listo" "$RUN_DIR/mirror.log" 2>/dev/null; then
      ready=1
      break
    fi
    if ! kill -0 "$(cat "$RUN_DIR/mirror.pid")" 2>/dev/null; then
      break
    fi
    sleep 0.25
  done
  if [[ "$ready" != 1 ]]; then
    tail -n 20 "$RUN_DIR/mirror.log" >&2 || true
    fail "el espejo no publico $MAP_NAME"
  fi

  echo "Espejo listo: $SHM_PATH -> $MAP_NAME"
  [[ -n "$ALIAS_NAME" ]] && echo "  alias: $ALIAS_NAME"

  if [[ ! -e "$SHM_PATH" ]]; then
    echo "AVISO: $SHM_PATH todavia no existe; el espejo espera al juego." >&2
    echo "       Si ETS2 esta abierto, reinicialo para que cargue el plugin SCS." >&2
  fi
}

# ---------------------------------------------------------------------------
# 3. Stub de deteccion para SimHub
# ---------------------------------------------------------------------------
# SimHub elige el simulador activo por el proceso que ve en su propio prefijo.
# ETS2 corre nativo en Linux, fuera de Bottles, asi que SimHub no lo ve y no
# cambia solo. Este stub inerte solo aporta el nombre "eurotrucks2.exe".
# Vive en RUN_DIR, asi que ETOL no lo confunde con el juego real.
stub_cmd_matches() {
  local pid="$1" base args
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  base="$(tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | head -n1)"
  base="${base//\\//}"
  base="${base##*/}"
  [[ "$base" == "eurotrucks2.exe" ]] || return 1
  args="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "${args//\\//}" == *"$STUB_EXE"* ]]
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'eurotrucks2[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

start_stub() {
  if [[ "$DO_STUB" != 1 ]]; then
    echo "stub de deteccion eurotrucks2.exe omitido (--no-stub)."
    return 0
  fi
  if [[ ! -x "$STUB_SRC" ]]; then
    echo "AVISO: falta $STUB_SRC; SimHub no cambiara solo a ETS2." >&2
    echo "       Compilalo con scripts/juegos/ets2/bridge/build_ets2_stub.sh" >&2
    return 0
  fi

  # Copia atomica: si el stub anterior sigue en marcha no se puede truncar su
  # binario (ETXTBSY); renombrar deja el inodo viejo intacto.
  cp -f "$STUB_SRC" "$STUB_EXE.tmp"
  chmod +x "$STUB_EXE.tmp" 2>/dev/null || true
  mv -f "$STUB_EXE.tmp" "$STUB_EXE"

  local existing
  existing="$(find_stub_pids | head -n1 || true)"
  if [[ -n "$existing" ]]; then
    echo "$existing" > "$RUN_DIR/stub.pid"
    echo "Stub de deteccion ya en marcha (PID $existing)."
    return 0
  fi
  rm -f "$RUN_DIR/stub.pid"

  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all "$BOTTLE_WINE" "$STUB_EXE" \
    >"$RUN_DIR/stub.log" 2>&1 </dev/null &
  echo "$!" > "$RUN_DIR/stub.pid"

  local found="" i
  for i in $(seq 1 50); do
    found="$(find_stub_pids | head -n1 || true)"
    [[ -n "$found" ]] && break
    sleep 0.1
  done
  if [[ -z "$found" ]]; then
    echo "AVISO: el stub eurotrucks2.exe no arranco; SimHub no cambiara solo." >&2
    tail -n 20 "$RUN_DIR/stub.log" >&2 || true
    return 1
  fi
  echo "Stub de deteccion listo: eurotrucks2.exe (PID $found)."
}

# ---------------------------------------------------------------------------
# 4. Configuracion de SimHub
# ---------------------------------------------------------------------------
apply_config() {
  if [[ "$DO_CONFIG" != 1 ]]; then
    echo "configuracion de SimHub omitida (--no-config)."
    return 0
  fi
  if [[ ! -f "$CONFIG_PY" ]]; then
    echo "AVISO: falta $CONFIG_PY" >&2
    return 0
  fi
  echo "Configurando SimHub para Euro Truck Simulator 2..."
  python3 "$CONFIG_PY" || echo "AVISO: no se pudo configurar SimHub." >&2
}

install_plugin || true
apply_config

if [[ "$SOLO_CONFIG" == 1 ]]; then
  echo "Configuracion aplicada (sin arrancar el espejo)."
  exit 0
fi

start_mirror || true
start_stub || true

# El HF8 se configura al final: el espejo y el stub son procesos Wine y, al
# arrancar, pueden regenerar dosdevices y reasignar el COM del pad.
if [[ "$DO_HF8" == 1 ]]; then
  HF8_SETUP="$BRIDGE/setup_hf8.sh"
  if [[ -x "$HF8_SETUP" ]]; then
    echo "Configurando pad haptico HF8 (ForceFeel)..."
    SIMHUB_BOTTLE="$BOTTLE" BOTTLE_WINE="$BOTTLE_WINE" \
      LMU_MIRROR_RUN_DIR="$RUN_DIR" "$HF8_SETUP" || \
      echo "AVISO: no se pudo configurar el HF8; SimHub seguira sin el pad." >&2
  fi
fi

cat <<EOF

Siguiente paso:
  - Arranque ordenado: ./arrancar_todo_ets2.sh
  - Abre Euro Truck Simulator 2 desde Steam (version Linux).
  - SimHub debe recibir la telemetria en cuanto el juego cargue una partida.
  - Solo SimHub: CrewChief no tiene lector de camiones.
Para parar el espejo: ./desactivar_puente_ets2.sh
EOF

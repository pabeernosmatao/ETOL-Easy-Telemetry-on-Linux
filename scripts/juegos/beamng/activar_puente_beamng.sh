#!/usr/bin/env bash
# Activa el puente de BeamNG.drive (version nativa Linux).
#
# BeamNG publica su telemetria por UDP con el protocolo OutGauge. SimHub
# escucha en 127.0.0.1:63392, asi que no hay memoria compartida ni espejo:
# este script solo deja la configuracion escrita y arranca un stub de proceso
# para que SimHub vea el juego dentro de su prefijo.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
CONFIG_PY="$BRIDGE/configure_apps_beamng.py"
STUB_SRC="$BRIDGE/beamng_process_stub.exe"
RUN_DIR="${BEAMNG_RUN_DIR:-/tmp/etol/beamng_udp}"
STUB_EXE="$RUN_DIR/BeamNG.drive.x64.exe"

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

GAME_DIR="${BEAMNG_GAME_DIR:-}"
if [[ -z "$GAME_DIR" ]]; then
  for candidate in \
    "/mnt/GAMES/SteamLibrary/steamapps/common/BeamNG.drive" \
    "$HOME/.local/share/Steam/steamapps/common/BeamNG.drive"; do
    if [[ -x "$candidate/BinLinux/BeamNG.drive.x64" ]]; then
      GAME_DIR="$candidate"
      break
    fi
  done
fi

UDP_PORT="${BEAMNG_UDP_PORT:-63392}"

DO_CONFIG=1
SOLO_CONFIG=0
DO_HF8=1
DO_STUB=1

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --no-config    no tocar la configuracion de BeamNG ni de SimHub
  --solo-config  aplica la configuracion y sale, sin arrancar el stub
  --no-hf8       no configurar el pad haptico HF8
  --no-stub      no arrancar el proceso inerte BeamNG.drive.x64.exe
  -h, --help     esta ayuda

Telemetria: OutGauge UDP 127.0.0.1:$UDP_PORT -> SimHub
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-config) DO_CONFIG=0; shift ;;
    --solo-config) SOLO_CONFIG=1; shift ;;
    --no-hf8) DO_HF8=0; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$RUN_DIR"
export SIMHUB_BOTTLE="$BOTTLE" BOTTLE_WINE="$BOTTLE_WINE"
export BEAMNG_RUN_DIR="$RUN_DIR" BEAMNG_UDP_PORT="$UDP_PORT"
[[ -n "$GAME_DIR" ]] && export BEAMNG_GAME_DIR="$GAME_DIR"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

# SimHub elige el simulador por el proceso que ve en su prefijo. BeamNG corre
# nativo en Linux, asi que este stub inerte le da el nombre que busca.
stub_cmd_matches() {
  local pid="$1" base args
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  base="$(tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | head -n1)"
  base="${base//\\//}"
  base="${base##*/}"
  [[ "$base" == "BeamNG.drive.x64.exe" ]] || return 1
  args="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "${args//\\//}" == *"$STUB_EXE"* ]]
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'BeamNG[.]drive[.]x64[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

start_stub() {
  if [[ "$DO_STUB" != 1 ]]; then
    echo "stub de deteccion BeamNG.drive.x64.exe omitido (--no-stub)."
    return 0
  fi
  if [[ ! -x "$STUB_SRC" ]]; then
    echo "AVISO: falta $STUB_SRC; SimHub no cambiara solo a BeamNG." >&2
    echo "       Compilalo con scripts/juegos/beamng/bridge/build_beamng_stub.sh" >&2
    return 0
  fi

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
    echo "AVISO: el stub BeamNG.drive.x64.exe no arranco; SimHub no cambiara solo." >&2
    tail -n 20 "$RUN_DIR/stub.log" >&2 || true
    return 1
  fi
  echo "Stub de deteccion listo: BeamNG.drive.x64.exe (PID $found)."
}

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando BeamNG.drive y SimHub..."
  python3 "$CONFIG_PY" || echo "AVISO: no se pudo aplicar la configuracion." >&2
fi

if [[ "$SOLO_CONFIG" == 1 ]]; then
  echo "Configuracion aplicada (sin arrancar el stub)."
  exit 0
fi

start_stub || true

# El HF8 se configura al final: el stub es un proceso Wine y, al arrancar,
# puede regenerar dosdevices y reasignar el COM del pad.
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
  - Arranque ordenado: ./arrancar_todo_beamng.sh
  - Abre BeamNG.drive desde Steam (version Linux).
  - SimHub debe recibir datos en cuanto entres en un vehiculo.
  - Solo SimHub: CrewChief no se usa con BeamNG.
Para parar el stub: ./desactivar_puente_beamng.sh
EOF

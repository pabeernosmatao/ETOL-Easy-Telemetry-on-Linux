#!/usr/bin/env bash
# Activa el reparto UDP de DiRT Rally 2.0 -> SimHub/CrewChief.
#
# El juego envia a 127.0.0.1:20777. El relay de ETOL escucha ahi y reenvia una
# copia a SimHub (20778) y otra a CrewChief (20779). El puente es de solo
# lectura: no escribe en el juego ni reinicia las aplicaciones.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
RELAY="$BRIDGE/dirt2_udp_relay.py"
CONFIG_PY="$BRIDGE/configure_apps_dirt2.py"
STUB_SRC="$BRIDGE/dirt2_process_stub.exe"
RUN_DIR="${DIRT2_UDP_RUN_DIR:-/tmp/etol/dirt2_udp}"
STUB_EXE="$RUN_DIR/dirtrally2.exe"

DIRT2_PFX="${DIRT2_PFX:-}"
if [[ -z "$DIRT2_PFX" ]]; then
  echo "ERROR: falta DIRT2_PFX. Ejecuta: etol detect --save" >&2
  exit 1
fi
DIRT2_WINE="${DIRT2_WINE:-}"
if [[ -z "$DIRT2_WINE" ]]; then
  echo "ERROR: falta DIRT2_WINE. Ejecuta: etol detect --save" >&2
  exit 1
fi
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

PORT="${DIRT2_LISTEN_PORT:-20777}"
SIMHUB_PORT="${DIRT2_SIMHUB_PORT:-20778}"
CREWCHIEF_PORT="${DIRT2_CREWCHIEF_PORT:-20779}"
DO_CONFIG=1
SOLO_CONFIG=0
DO_HF8=1
DO_RELAY=1
DO_STUB=1

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --no-config    no tocar la configuracion de juego/SimHub/CrewChief
  --solo-config  aplica la configuracion y sale, sin arrancar el relay
  --no-hf8       no configurar el pad haptico HF8
  --no-stub      no arrancar el proceso inerte dirtrally2.exe
  -h, --help     esta ayuda

Puertos: juego $PORT, SimHub $SIMHUB_PORT, CrewChief $CREWCHIEF_PORT.
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
export DIRT2_PFX DIRT2_WINE DIRT2_UDP_RUN_DIR="$RUN_DIR"
export DIRT2_LISTEN_PORT="$PORT" DIRT2_SIMHUB_PORT="$SIMHUB_PORT"
export DIRT2_CREWCHIEF_PORT="$CREWCHIEF_PORT"
export SIMHUB_BOTTLE="$BOTTLE" BOTTLE_WINE="$BOTTLE_WINE"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando DiRT Rally 2.0, SimHub y CrewChief..."
  python3 "$CONFIG_PY" || fail "no se pudo aplicar la configuracion"
fi

if [[ "$SOLO_CONFIG" == 1 ]]; then
  echo "Configuracion aplicada (sin arrancar el relay)."
  exit 0
fi

relay_running() {
  local pidfile="$RUN_DIR/relay.pid" pid
  [[ -f "$pidfile" ]] || return 1
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

# CrewChief solo abre su lector UDP de DiRT Rally 2.0 cuando ve un proceso
# llamado "dirtrally2" en SU prefijo. El juego real corre en el prefijo de
# Proton de Steam, que es otro wineserver: no lo ve. Este stub inerte le da el
# nombre que busca. Vive en RUN_DIR, asi que ETOL no lo confunde con el juego.
stub_cmd_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  local cmd norm
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"dirtrally2.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'dirtrally2[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

start_stub() {
  if [[ "$DO_STUB" != 1 ]]; then
    echo "stub de deteccion dirtrally2.exe omitido (--no-stub)."
    return 0
  fi
  if [[ ! -x "$STUB_SRC" ]]; then
    echo "AVISO: falta $STUB_SRC; CrewChief no vera DiRT Rally 2.0." >&2
    echo "       Compilalo con scripts/juegos/dirt2/bridge/build_dirt2_stub.sh" >&2
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
    echo "AVISO: el stub dirtrally2.exe no arranco; CrewChief no vera el juego." >&2
    tail -n 20 "$RUN_DIR/stub.log" >&2 || true
    return 1
  fi
  echo "Stub de deteccion listo: dirtrally2.exe (PID $found)."
}

if [[ "$DO_RELAY" == 1 ]]; then
  if relay_running; then
    echo "Relay UDP ya en marcha (PID $(cat "$RUN_DIR/relay.pid"))."
  else
    rm -f "$RUN_DIR/relay.pid"
    setsid env \
      DIRT2_LISTEN_PORT="$PORT" \
      DIRT2_SIMHUB_PORT="$SIMHUB_PORT" \
      DIRT2_CREWCHIEF_PORT="$CREWCHIEF_PORT" \
      python3 "$RELAY" >>"$RUN_DIR/relay.log" 2>&1 </dev/null &
    echo "$!" > "$RUN_DIR/relay.pid"

    ready=0
    for _ in $(seq 1 40); do
      if ss -lun 2>/dev/null | grep -q "127.0.0.1:$PORT"; then
        ready=1
        break
      fi
      sleep 0.25
    done
    if [[ "$ready" != 1 ]]; then
      tail -n 20 "$RUN_DIR/relay.log" >&2 || true
      fail "el relay no abrio UDP $PORT"
    fi
    echo "Relay UDP listo: juego $PORT -> SimHub $SIMHUB_PORT, CrewChief $CREWCHIEF_PORT."
  fi
fi

start_stub || true

# El pad HF8 se configura AL FINAL: el stub es un proceso Wine que, al
# arrancar, regenera dosdevices y reasigna COM33 a otro tty. Configurarlo
# antes dejaba el enlace pisado y SimHub abria el puerto equivocado.
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
  - Arranque ordenado de todo: ./arrancar_todo_dirt2.sh
  - SimHub: deberia detectar DiRT Rally 2.0 por UDP sin reiniciarlo.
  - CrewChief: tarda unos segundos en ver el stub y abrir UDP $CREWCHIEF_PORT.
  - El juego debe estar cerrado al aplicar la configuracion y reiniciarse
    despues: necesita leer <udp extradata="3"> al arrancar.
Para parar el relay y el stub: ./desactivar_puente_dirt2.sh
EOF

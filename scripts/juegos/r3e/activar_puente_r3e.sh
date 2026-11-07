#!/usr/bin/env bash
# Activa el espejo de telemetria de RaceRoom ($R3E) -> prefijo Bottles.
#
# El emisor corre en el prefijo de RaceRoom y copia el mapping $R3E por UDP.
# El receptor lo recrea en la botella de Bottles para que SimHub y CrewChief
# lo lean. Es de solo lectura: no modifica el juego.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
EXE="$BRIDGE/r3e_shm_mirror.exe"
STUB_SRC="$BRIDGE/r3e_process_stub.exe"
CONFIG_PY="$BRIDGE/configure_apps_r3e.py"
RUN_DIR="${R3E_MIRROR_RUN_DIR:-/tmp/etol/r3e_mirror}"
STUB_EXE="$RUN_DIR/RRRE64.exe"
MAP='$R3E'

R3E_PFX="${R3E_PFX:-}"
if [[ -z "$R3E_PFX" ]]; then
  echo "ERROR: falta R3E_PFX. Ejecuta: etol detect --save o etol config set games.r3e.prefix <ruta>" >&2
  exit 1
fi
R3E_WINE="${R3E_WINE:-}"
if [[ -z "$R3E_WINE" ]]; then
  echo "ERROR: falta R3E_WINE. Ejecuta: etol detect --save o etol config set games.r3e.wine <ruta>" >&2
  exit 1
fi

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

PORT="${R3E_MIRROR_PORT:-8777}"
SIZE="${R3E_MIRROR_SIZE:-45056}"
HZ="${R3E_MIRROR_HZ:-30}"
DO_CONFIG=1
SOLO_CONFIG=0
DO_STUB=1
DO_HF8=1
VERBOSE=0

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --port N        puerto UDP local (defecto: $PORT)
  --size N        tamano del mapping en Bottles (defecto: $SIZE)
  --hz N          frecuencia del espejo (defecto: $HZ)
  --no-config     no tocar la configuracion de SimHub/CrewChief
  --solo-config   solo configura y sale (boton de engranaje)
  --no-stub       no arrancar el proceso inerte RRRE64.exe
  --no-hf8        compatibilidad (el HF8 lo configura SimHub)
  --verbose       logs detallados
  -h, --help      esta ayuda
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="${2:?falta valor para --port}"; shift 2 ;;
    --size) SIZE="${2:?falta valor para --size}"; shift 2 ;;
    --hz) HZ="${2:?falta valor para --hz}"; shift 2 ;;
    --no-config) DO_CONFIG=0; shift ;;
    --solo-config) SOLO_CONFIG=1; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    --no-hf8) DO_HF8=0; shift ;;
    --verbose) VERBOSE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$RUN_DIR"
export R3E_MIRROR_RUN_DIR="$RUN_DIR" R3E_MIRROR_PORT="$PORT" R3E_MIRROR_SIZE="$SIZE" R3E_MIRROR_HZ="$HZ"
export R3E_PFX R3E_WINE BOTTLE_WINE SIMHUB_BOTTLE="$BOTTLE"

fail() { echo "ERROR: $*" >&2; exit 1; }

if [[ "$R3E_WINE" == "$BOTTLE_WINE" ]]; then
  echo "AVISO: el wine de RaceRoom no se detecto; se usa el de Bottles." >&2
  echo "       Si el emisor no puede leer \$R3E, configura el Proton del juego en Steam/ETOL." >&2
fi

# Stub: proceso inerte llamado RRRE64.exe para que SimHub/CrewChief vean el
# juego dentro de Bottles (el proceso real esta en el prefijo de Proton).
stub_cmd_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  local cmd norm
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline")"
  [[ "$cmd" == *"RRRE64.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'RRRE64[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

start_stub() {
  [[ "$DO_STUB" == 1 ]] || { echo "stub de deteccion omitido."; return 0; }
  [[ -x "$STUB_SRC" ]] || { echo "AVISO: falta $STUB_SRC; sin stub." >&2; return 0; }
  local existing
  existing="$(find_stub_pids | head -n1 || true)"
  if [[ -n "$existing" ]]; then
    echo "$existing" > "$RUN_DIR/stub.pid"
    echo "stub ya estaba en marcha (PID $existing)."
    return 0
  fi
  cp -f "$STUB_SRC" "$STUB_EXE.tmp"
  chmod +x "$STUB_EXE.tmp" 2>/dev/null || true
  mv -f "$STUB_EXE.tmp" "$STUB_EXE"
  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all "$BOTTLE_WINE" "$STUB_EXE" \
    >"$RUN_DIR/stub.log" 2>&1 < /dev/null &
  echo "$!" > "$RUN_DIR/stub.pid"
  for _ in $(seq 1 50); do
    existing="$(find_stub_pids | head -n1 || true)"
    [[ -n "$existing" ]] && break
    sleep 0.1
  done
  if [[ -n "$existing" ]]; then
    echo "$existing" > "$RUN_DIR/stub.pid"
    echo "stub de deteccion arrancado (PID $existing)."
  else
    echo "AVISO: el stub no arranco; revisa $RUN_DIR/stub.log" >&2
  fi
}

start_process() {
  local name="$1" pfx="$2" wine="$3"; shift 3
  local pidfile="$RUN_DIR/$name.pid" log="$RUN_DIR/$name.log"
  setsid env WINEPREFIX="$pfx" WINEDEBUG=-all "$wine" "$EXE" "$@" \
    >"$log" 2>&1 < /dev/null &
  echo "$!" > "$pidfile"
  echo "$name arrancado (PID $!, log $log)."
}

wait_for_log() {
  local file="$1" pattern="$2" timeout="${3:-20}" i
  for ((i = 0; i < timeout * 5; i++)); do
    if [[ -f "$file" ]] && grep -Fq "$pattern" "$file"; then
      return 0
    fi
    sleep 0.2
  done
  return 1
}

[[ -x "$EXE" ]] || fail "falta $EXE"
[[ -f "$CONFIG_PY" ]] || fail "falta $CONFIG_PY"

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando SimHub y CrewChief para RaceRoom (con copia de seguridad)..."
  SIMHUB_BOTTLE="$BOTTLE" \
    R3E_MIRROR_BACKUP_DIR="$HOME/.local/share/r3e_mirror/backups" \
    python3 "$CONFIG_PY" || \
    echo "AVISO: la configuracion automatica fallo; el puente seguira funcionando." >&2
fi

if [[ "${SOLO_CONFIG:-0}" == 1 ]]; then
  echo "Configuracion aplicada."
  exit 0
fi

# El receptor debe existir antes de SimHub/CrewChief.
if ! pgrep -f -- 'r3e_shm_mirror\.exe --receiver' >/dev/null 2>&1; then
  local_args=(--receiver --name "$MAP" --bind "127.0.0.1:$PORT" --size "$SIZE")
  [[ "$VERBOSE" == 1 ]] && local_args+=(--verbose)
  start_process receiver "$BOTTLE" "$BOTTLE_WINE" "${local_args[@]}"
  if ! wait_for_log "$RUN_DIR/receiver.log" "receptor" 10; then
    fail "el receptor no arranco; revisa $RUN_DIR/receiver.log"
  fi
else
  echo "receptor ya estaba en marcha."
fi

if ! pgrep -f -- 'r3e_shm_mirror\.exe --sender' >/dev/null 2>&1; then
  sender_args=(--sender --name "$MAP" --udp "127.0.0.1:$PORT" --hz "$HZ")
  [[ "$VERBOSE" == 1 ]] && sender_args+=(--verbose)
  start_process sender "$R3E_PFX" "$R3E_WINE" "${sender_args[@]}"
  if wait_for_log "$RUN_DIR/sender.log" "abierto" 10; then
    echo "RaceRoom: mapping \$R3E enlazado."
  else
    echo "AVISO: RaceRoom aun no publica \$R3E; el emisor esperara." >&2
  fi
else
  echo "emisor ya estaba en marcha."
fi

start_stub

cat <<EOF

Espejo RaceRoom:
  mapping:  \$R3E ($SIZE bytes)
  UDP:      127.0.0.1:$PORT
  logs:     $RUN_DIR/receiver.log, $RUN_DIR/sender.log
  para parar: ./desactivar_puente_r3e.sh
EOF

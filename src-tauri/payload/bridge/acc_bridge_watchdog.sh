#!/usr/bin/env bash
# Vigila el espejo Assetto Corsa Competizione -> Bottles y lo levanta si
# desaparece. No toca el juego, SimHub, CrewChief ni sus configuraciones.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXE="$HERE/acc_shm_mirror.exe"
STUB_SRC="$HERE/acc_process_stub.exe"

# En el arbol de fuentes los binarios viven en vendor/; en el payload, en bridge/.
if [[ ! -x "$EXE" && -x "$HERE/../../../../vendor/acc_shm_mirror.exe" ]]; then
  EXE="$HERE/../../../../vendor/acc_shm_mirror.exe"
fi
if [[ ! -x "$STUB_SRC" && -x "$HERE/../../../../vendor/acc_process_stub.exe" ]]; then
  STUB_SRC="$HERE/../../../../vendor/acc_process_stub.exe"
fi

RUN_DIR="${ACC_MIRROR_RUN_DIR:-/tmp/etol/acc_mirror}"
STUB_GAME="$RUN_DIR/AC2-Win64-Shipping.exe"
STUB_ACC="$RUN_DIR/acc.exe"
LOG="$RUN_DIR/watchdog.log"
INTERVAL="${ACC_MIRROR_WATCH_INTERVAL:-2}"
DO_STUB="${ACC_MIRROR_STUB:-1}"

ACC_PFX="${ACC_PFX:-}"
ACC_WINE="${ACC_WINE:-}"
BOTTLE="${SIMHUB_BOTTLE:-}"
BOTTLE_WINE="${BOTTLE_WINE:-}"

PORT="${ACC_MIRROR_PORT:-8779}"
HZ="${ACC_MIRROR_HZ:-60}"
SIZES="${ACC_MIRROR_SIZES:-4096,4096,4096}"

if [[ -z "$ACC_PFX" || -z "$ACC_WINE" || -z "$BOTTLE" || -z "$BOTTLE_WINE" ]]; then
  echo "ERROR: faltan ACC_PFX, ACC_WINE, SIMHUB_BOTTLE o BOTTLE_WINE." >&2
  exit 1
fi

mkdir -p "$RUN_DIR"

log() {
  printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >>"$LOG"
}

normalize() {
  printf '%s' "${1//\\//}"
}

mirror_pid() {
  local mode="$1" pid cmd
  for pid in $(pgrep -f -- "acc_shm_mirror\\.exe --$mode" 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == start.exe* ]] && continue
    printf '%s\n' "$pid"
    return 0
  done
  return 0
}

stub_pids() {
  local name="$1" pattern pid cmd
  case "$name" in
    AC2*) pattern='AC2-Win64-Shipping[.]exe' ;;
    *) pattern='acc[.]exe' ;;
  esac
  for pid in $(pgrep -f -- "$pattern" 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$(normalize "$cmd")" == *"$RUN_DIR/$name"* ]] && printf '%s\n' "$pid"
  done
  return 0
}

start_mirror() {
  local mode="$1" prefix="$2" wine="$3"
  shift 3
  local pid
  setsid env WINEPREFIX="$prefix" WINEDEBUG=-all "$wine" "$EXE" "$@" \
    >>"$RUN_DIR/$mode.log" 2>&1 </dev/null &
  pid=$!
  echo "$pid" >"$RUN_DIR/$mode.pid"
  log "$mode relanzado (launcher PID $pid)."
}

start_stub() {
  local name="$1" target="$RUN_DIR/$1"
  if [[ -x "$STUB_SRC" ]]; then
    cp -f "$STUB_SRC" "$target.tmp" 2>/dev/null && mv -f "$target.tmp" "$target"
  fi
  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all "$BOTTLE_WINE" "$target" \
    >>"$RUN_DIR/stub-$name.log" 2>&1 </dev/null &
  echo "$!" >"$RUN_DIR/stub-$name.pid"
  log "stub $name relanzado."
}

[[ -x "$EXE" ]] || { echo "ERROR: falta $EXE" >&2; exit 1; }
[[ -x "$BOTTLE_WINE" ]] || { echo "ERROR: falta el wine de Bottles: $BOTTLE_WINE" >&2; exit 1; }

echo $$ >"$RUN_DIR/watchdog.pid"
log "vigilante iniciado (PID $$, intervalo ${INTERVAL}s)."
trap 'log "vigilante detenido."; rm -f "$RUN_DIR/watchdog.pid"; exit 0' TERM INT

while true; do
  if [[ -z "$(mirror_pid receiver)" ]]; then
    start_mirror receiver "$BOTTLE" "$BOTTLE_WINE" \
      --receiver --bind "127.0.0.1:$PORT" --sizes "$SIZES"
    sleep 1
  fi
  if [[ -z "$(mirror_pid sender)" ]]; then
    start_mirror sender "$ACC_PFX" "$ACC_WINE" \
      --sender --udp "127.0.0.1:$PORT" --hz "$HZ"
    sleep 1
  fi
  if [[ "$DO_STUB" == 1 ]]; then
    if [[ -z "$(stub_pids acc.exe)" ]]; then
      start_stub acc.exe
      sleep 1
    fi
    if [[ -z "$(stub_pids AC2-Win64-Shipping.exe)" ]]; then
      start_stub AC2-Win64-Shipping.exe
      sleep 1
    fi
  fi
  sleep "$INTERVAL"
done

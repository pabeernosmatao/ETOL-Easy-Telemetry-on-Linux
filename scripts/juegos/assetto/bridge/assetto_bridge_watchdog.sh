#!/usr/bin/env bash
# Vigila el espejo Assetto Corsa -> Bottles y lo levanta si desaparece.
#
# Motivo: los procesos del puente (receptor, emisor y stub) corren bajo Wine.
# Si el wineserver del prefijo de Bottles se reinicia, Wine mata todos sus
# procesos y el puente queda "vivo" solo en apariencia: el emisor sigue
# enviando UDP pero nadie publica acpmf_*, y las apps dejan de ver Assetto Corsa
# sin dar error.
#
# Este vigilante solo relanza procesos del puente. No toca Assetto Corsa, SimHub,
# CrewChief ni sus archivos de configuracion.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXE="$HERE/assetto_shm_mirror.exe"
STUB_SRC="$HERE/ac_process_stub.exe"

RUN_DIR="${ASSETTO_MIRROR_RUN_DIR:-/tmp/etol/assetto_mirror}"
STUB_EXE="$RUN_DIR/acs.exe"
LOG="$RUN_DIR/watchdog.log"
INTERVAL="${ASSETTO_MIRROR_WATCH_INTERVAL:-2}"
DO_STUB="${ASSETTO_MIRROR_STUB:-1}"

ASSETTO_PFX="${ASSETTO_PFX:-}"
ASSETTO_WINE="${ASSETTO_WINE:-}"
BOTTLE="${SIMHUB_BOTTLE:-}"
BOTTLE_WINE="${BOTTLE_WINE:-}"

PORT="${ASSETTO_MIRROR_PORT:-8775}"
HZ="${ASSETTO_MIRROR_HZ:-60}"
SIZES="${ASSETTO_MIRROR_SIZES:-4096,4096,4096}"

if [[ -z "$ASSETTO_PFX" || -z "$ASSETTO_WINE" || -z "$BOTTLE" || -z "$BOTTLE_WINE" ]]; then
  echo "ERROR: faltan ASSETTO_PFX, ASSETTO_WINE, SIMHUB_BOTTLE o BOTTLE_WINE." >&2
  exit 1
fi

mkdir -p "$RUN_DIR"

log() {
  printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >>"$LOG"
}

stub_cmd_matches() {
  local pid="$1" cmd norm
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"acs.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'acs[.]exe' 2>/dev/null || true); do
    stub_cmd_matches "$pid" && printf '%s\n' "$pid"
  done
  return 0
}

mirror_pid() {
  local mode="$1" pid cmd
  for pid in $(pgrep -f -- "assetto_shm_mirror\\.exe --$mode" 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    # Wine arranca el .exe a traves de "start.exe /exec ...", que tambien
    # aparece en pgrep. Solo cuenta el proceso real del espejo.
    [[ "$cmd" == start.exe* ]] && continue
    printf '%s\n' "$pid"
    return 0
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
  if [[ -x "$STUB_SRC" ]]; then
    cp -f "$STUB_SRC" "$STUB_EXE.tmp" 2>/dev/null && mv -f "$STUB_EXE.tmp" "$STUB_EXE"
  fi
  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all "$BOTTLE_WINE" "$STUB_EXE" \
    >>"$RUN_DIR/stub.log" 2>&1 </dev/null &
  echo "$!" >"$RUN_DIR/stub.pid"
  log "stub de deteccion relanzado."
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
    start_mirror sender "$ASSETTO_PFX" "$ASSETTO_WINE" \
      --sender --udp "127.0.0.1:$PORT" --hz "$HZ"
    sleep 1
  fi
  if [[ "$DO_STUB" == 1 && -z "$(find_stub_pids)" ]]; then
    start_stub
    sleep 1
  fi
  sleep "$INTERVAL"
done

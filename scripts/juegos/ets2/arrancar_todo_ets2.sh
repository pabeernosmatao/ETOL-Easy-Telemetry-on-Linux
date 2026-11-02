#!/usr/bin/env bash
# Arranca todo para Euro Truck Simulator 2: espejo -> SimHub.
#
# No arranca ETS2 (lo abres tu desde Steam) ni reinicia SimHub si ya esta en
# marcha. Solo SimHub: CrewChief no tiene lector de camiones.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACTIVAR="$HERE/activar_puente_ets2.sh"
RUN_DIR="${ETS2_MIRROR_RUN_DIR:-/tmp/etol/ets2_bridge}"
LOG="$RUN_DIR/arranque.log"

BOTTLE="${SIMHUB_BOTTLE:-$HOME/.local/share/bottles/bottles/simhub}"
BOTTLE_WINE="${BOTTLE_WINE:-$HOME/.local/share/bottles/runners/ge-proton11-11/files/bin/wine}"

SIMHUB_DIR_WIN='C:\Program Files (x86)\SimHub'
SIMHUB_EXE_WIN='C:\Program Files (x86)\SimHub\SimHubWPF.exe'

DO_CONFIG=1
DO_HF8=1
DO_PLUGIN=1
DRY=0
ACTIVAR_ARGS=()

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --no-config    no reconfigura SimHub
  --no-hf8       no configura el pad HF8
  --no-plugin    no instala el plugin SCS en el juego
  --dry-run      muestra lo que haria, sin arrancar nada
  -h, --help     esta ayuda
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-config) DO_CONFIG=0; shift ;;
    --no-hf8) DO_HF8=0; shift ;;
    --no-plugin) DO_PLUGIN=0; shift ;;
    --dry-run) DRY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ "$DO_CONFIG" == 1 ]] || ACTIVAR_ARGS+=(--no-config)
[[ "$DO_HF8" == 1 ]] || ACTIVAR_ARGS+=(--no-hf8)
[[ "$DO_PLUGIN" == 1 ]] || ACTIVAR_ARGS+=(--no-plugin)

mkdir -p "$RUN_DIR"

log() {
  printf '%s %s\n' "$(date '+%H:%M:%S')" "$*" | tee -a "$LOG"
}

if command -v flock >/dev/null 2>&1; then
  exec 9>"$RUN_DIR/arranque.lock"
  if ! flock -n 9; then
    echo "Ya hay un arranque en curso ($RUN_DIR/arranque.lock)." >&2
    exit 0
  fi
fi

app_pid() {
  local name="$1" pid cmd
  for pid in $(pgrep -f -- "$name" 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == *start.exe* ]] && continue
    [[ "$cmd" == [CZ]:\\* ]] && { printf '%s\n' "$pid"; return 0; }
  done
  return 0
}

start_app() {
  local name="$1" dir_win="$2" exe_win="$3" logfile="$4"
  local pid
  pid="$(app_pid "$name")"
  if [[ -n "$pid" ]]; then
    log "  $name ya esta en marcha (PID $pid); no lo toco."
    return 0
  fi
  if [[ "$DRY" == 1 ]]; then
    log "  [dry-run] arrancaria $name"
    return 0
  fi
  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all \
    "$BOTTLE_WINE" start /wait /d "$dir_win" "$exe_win" \
    >>"$logfile" 2>&1 </dev/null 9>&- &
  log "  $name lanzado (log $logfile)."
}

log "=== arranque Euro Truck Simulator 2 $(date '+%F %T') ==="

if [[ "$DRY" == 1 ]]; then
  log "  [dry-run] $ACTIVAR ${ACTIVAR_ARGS[*]}"
else
  set +e
  "$ACTIVAR" "${ACTIVAR_ARGS[@]}" 9>&- 2>&1 | tee -a "$LOG"
  status=${PIPESTATUS[0]}
  set -e
  if [[ "$status" != 0 ]]; then
    log "AVISO: activar_puente_ets2.sh fallo; sigo con SimHub."
  fi
fi

log "1/2 espejo de telemetria listo"
log "2/2 SimHub"
start_app SimHubWPF.exe "$SIMHUB_DIR_WIN" "$SIMHUB_EXE_WIN" "$RUN_DIR/simhub.log"

log "estado final:"
log "  espejo:  $(cat "$RUN_DIR/mirror.pid" 2>/dev/null || echo 'sin PID')"
log "  SimHub:  $(app_pid SimHubWPF.exe | head -n1 || true)"
log "  shm:     $( [[ -e /dev/shm/SCSTelemetry ]] && echo presente || echo 'esperando al juego' )"
log "  logs:    $RUN_DIR"
log "Para parar el espejo: ./desactivar_puente_ets2.sh (no cierra SimHub)."

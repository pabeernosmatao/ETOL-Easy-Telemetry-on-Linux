#!/usr/bin/env bash
# Arranca todo para DiRT Rally 2.0: relay UDP -> CrewChief -> SimHub.
#
# No arranca DiRT Rally 2.0 (lo abres tu desde Steam) ni reinicia apps que ya
# esten en marcha. El relay es de solo lectura.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACTIVAR="$HERE/activar_puente_dirt2.sh"
RUN_DIR="${DIRT2_UDP_RUN_DIR:-/tmp/etol/dirt2_udp}"
LOG="$RUN_DIR/arranque.log"

BOTTLE="${SIMHUB_BOTTLE:-$HOME/.local/share/bottles/bottles/simhub}"
BOTTLE_WINE="${BOTTLE_WINE:-$HOME/.local/share/bottles/runners/ge-proton11-11/files/bin/wine}"

CREW_DIR_WIN='C:\Program Files (x86)\Britton IT Ltd\CrewChiefV4'
CREW_EXE_WIN='C:\Program Files (x86)\Britton IT Ltd\CrewChiefV4\CrewChiefV4.exe'
SIMHUB_DIR_WIN='C:\Program Files (x86)\SimHub'
SIMHUB_EXE_WIN='C:\Program Files (x86)\SimHub\SimHubWPF.exe'

ESPERA_APPS="${DIRT2_ARRANQUE_ESPERA_APPS:-8}"
DO_CONFIG=1
DO_HF8=1
DO_STUB=1
DRY=0
ACTIVAR_ARGS=()

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --espera N     segundos entre CrewChief y SimHub (defecto: $ESPERA_APPS)
  --no-config    no reconfigura juego/SimHub/CrewChief
  --no-hf8       no configura el pad HF8
  --no-stub      no arranca el proceso inerte dirtrally2.exe
  --dry-run      muestra lo que haria, sin arrancar nada
  -h, --help     esta ayuda
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --espera) ESPERA_APPS="${2:?falta valor para --espera}"; shift 2 ;;
    --no-config) DO_CONFIG=0; shift ;;
    --no-hf8) DO_HF8=0; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    --dry-run) DRY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ "$DO_CONFIG" == 1 ]] || ACTIVAR_ARGS+=(--no-config)
[[ "$DO_HF8" == 1 ]] || ACTIVAR_ARGS+=(--no-hf8)
[[ "$DO_STUB" == 1 ]] || ACTIVAR_ARGS+=(--no-stub)

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
    >>"$logfile" 2>&1 </dev/null &
  log "  $name lanzado (log $logfile)."
}

log "=== arranque DiRT Rally 2.0 $(date '+%F %T') ==="

if [[ "$DRY" == 1 ]]; then
  log "  [dry-run] $ACTIVAR ${ACTIVAR_ARGS[*]}"
else
  set +e
  "$ACTIVAR" "${ACTIVAR_ARGS[@]}" 9>&- 2>&1 | tee -a "$LOG"
  status=${PIPESTATUS[0]}
  set -e
  if [[ "$status" != 0 ]]; then
    log "AVISO: activar_puente_dirt2.sh fallo; sigo con CrewChief y SimHub."
  fi
fi

log "1/3 relay UDP listo"
log "2/3 CrewChief"
start_app CrewChiefV4.exe "$CREW_DIR_WIN" "$CREW_EXE_WIN" "$RUN_DIR/crewchief.log"
if [[ "$DRY" != 1 ]]; then
  log "  esperando ${ESPERA_APPS}s antes de SimHub..."
  sleep "$ESPERA_APPS"
fi
log "3/3 SimHub"
start_app SimHubWPF.exe "$SIMHUB_DIR_WIN" "$SIMHUB_EXE_WIN" "$RUN_DIR/simhub.log"

log "estado final:"
log "  relay:     $(cat "$RUN_DIR/relay.pid" 2>/dev/null || echo 'sin PID')"
log "  CrewChief: $(app_pid CrewChiefV4.exe | head -n1 || true)"
log "  SimHub:    $(app_pid SimHubWPF.exe | head -n1 || true)"
log "  logs:      $RUN_DIR"
log "Para parar el relay: ./desactivar_puente_dirt2.sh (no cierra las apps)."

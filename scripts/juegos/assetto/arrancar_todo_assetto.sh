#!/usr/bin/env bash
# Arranca todo en orden y con pausas: espejo Assetto Corsa -> CrewChief -> SimHub.
#
# Existe porque las piezas compiten al arrancar: CrewChief carga miles de
# sonidos y SimHub inicializa plugins y lectores. Si se abren a la vez, el
# arranque se alarga y es mas facil que alguno se quede a medias.
#
# No arranca Assetto Corsa (lo abres tu desde Steam) ni reinicia apps que ya esten en
# marcha. El espejo es de solo lectura.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACTIVAR="$HERE/activar_puente_assetto.sh"
RUN_DIR="${ASSETTO_MIRROR_RUN_DIR:-/tmp/etol/assetto_mirror}"
LOG="$RUN_DIR/arranque.log"

BOTTLE="${SIMHUB_BOTTLE:-$HOME/.local/share/bottles/bottles/simhub}"
BOTTLE_WINE="${BOTTLE_WINE:-$HOME/.local/share/bottles/runners/ge-proton11-11/files/bin/wine}"

CREW_DIR_WIN='C:\Program Files (x86)\Britton IT Ltd\CrewChiefV4'
CREW_EXE_WIN='C:\Program Files (x86)\Britton IT Ltd\CrewChiefV4\CrewChiefV4.exe'
SIMHUB_DIR_WIN='C:\Program Files (x86)\SimHub'
SIMHUB_EXE_WIN='C:\Program Files (x86)\SimHub\SimHubWPF.exe'

ESPERA_PUENTE="${ASSETTO_ARRANQUE_ESPERA_PUENTE:-3}"   # tras publicar los mappings
ESPERA_APPS="${ASSETTO_ARRANQUE_ESPERA_APPS:-8}"      # entre CrewChief y SimHub
TIMEOUT_APP="${ASSETTO_ARRANQUE_TIMEOUT_APP:-120}"     # espera maxima de ventana
DO_CONFIG=1
DO_STUB=1
DRY=0
ACTIVAR_ARGS=()

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --espera N         segundos entre CrewChief y SimHub (defecto: $ESPERA_APPS)
  --espera-puente N  segundos extra tras publicar los mappings (defecto: $ESPERA_PUENTE)
  --timeout N        segundos maximos esperando la ventana de cada app (defecto: $TIMEOUT_APP)
  --no-config        no reconfigura SimHub/CrewChief (pasa --no-config a activar)
  --no-stub          no arranca el proceso inerte acs.exe
  --dry-run          muestra lo que haria, sin arrancar nada
  -h, --help         esta ayuda

Variables equivalentes: ASSETTO_ARRANQUE_ESPERA_APPS, ASSETTO_ARRANQUE_ESPERA_PUENTE,
ASSETTO_ARRANQUE_TIMEOUT_APP.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --espera) ESPERA_APPS="${2:?falta valor para --espera}"; shift 2 ;;
    --espera-puente) ESPERA_PUENTE="${2:?falta valor para --espera-puente}"; shift 2 ;;
    --timeout) TIMEOUT_APP="${2:?falta valor para --timeout}"; shift 2 ;;
    --no-config) DO_CONFIG=0; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    --dry-run) DRY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ "$DO_CONFIG" == 1 ]] || ACTIVAR_ARGS+=(--no-config)
[[ "$DO_STUB" == 1 ]] || ACTIVAR_ARGS+=(--no-stub)

mkdir -p "$RUN_DIR"

log() {
  printf '%s %s\n' "$(date '+%H:%M:%S')" "$*" | tee -a "$LOG"
}

# Evita dos arranques simultaneos.
if command -v flock >/dev/null 2>&1; then
  exec 9>"$RUN_DIR/arranque.lock"
  if ! flock -n 9; then
    echo "Ya hay un arranque en curso ($RUN_DIR/arranque.lock)." >&2
    exit 0
  fi
fi

# PID del proceso Windows real (no del envoltorio start.exe/pv-adverb).
app_pid() {
  local name="$1" pid cmd
  for pid in $(pgrep -f -- "$name" 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == *start.exe* ]] && continue
    [[ "$cmd" == [CZ]:\\* ]] && { printf '%s\n' "$pid"; return 0; }
  done
  return 0
}

assetto_pid() {
  local pid cmd
  for pid in $(pgrep -f -- 'acs[.]exe' 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == *"assetto_mirror/acs.exe"* ]] && continue
    [[ "$cmd" == *"assetto_mirror\\acs.exe"* ]] && continue
    [[ "$cmd" == *start.exe* ]] && continue
    # Evita el reaper de Steam, pressure-vessel y el steam.exe que solo
    # envuelven al juego; el proceso real es S:\common\...\acs.exe.
    [[ "$cmd" == *SteamLaunch* ]] && continue
    [[ "$cmd" == *reaper* ]] && continue
    [[ "$cmd" == *pv-adverb* ]] && continue
    [[ "$cmd" == *waitforexitandrun* ]] && continue
    [[ "$cmd" == *"steam.exe"* ]] && continue
    printf '%s\n' "$pid"
    return 0
  done
  return 0
}

# Limpia envoltorios 'start /wait' de sesiones anteriores.
limpiar_envoltorios() {
  local pid cmd app newest wetime aetime
  for pid in $(pgrep -f -- 'start.exe' 2>/dev/null || true); do
    [[ "$pid" == "$$" ]] && continue
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    case "$cmd" in
      "start.exe "*|"C:\\windows\\system32\\start.exe "*) ;;
      *) continue ;;
    esac
    case "$cmd" in
      *CrewChiefV4*) app=CrewChiefV4.exe ;;
      *SimHub*) app=SimHubWPF.exe ;;
      *) continue ;;
    esac
    newest="$(pgrep -n -x "$app" 2>/dev/null || true)"
    if [[ -z "$newest" ]]; then
      kill "$pid" 2>/dev/null || true
      continue
    fi
    wetime="$(ps -o etimes= -p "$pid" 2>/dev/null | tr -d ' ')"
    aetime="$(ps -o etimes= -p "$newest" 2>/dev/null | tr -d ' ')"
    if [[ -n "$wetime" && -n "$aetime" ]] && (( wetime > aetime + 120 )); then
      kill "$pid" 2>/dev/null || true
    fi
  done
}

receiver_alive() {
  local pid cmd
  for pid in $(pgrep -f -- 'assetto_shm_mirror\.exe --receiver' 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == start.exe* ]] && continue
    return 0
  done
  return 1
}

wait_mappings() {
  local timeout="${1:-30}" i
  for ((i = 0; i < timeout * 2; i++)); do
    grep -Fq "Mappings iniciales creados" "$RUN_DIR/receiver.log" 2>/dev/null && return 0
    sleep 0.5
  done
  return 1
}

wait_window() {
  local pattern="$1" timeout="$2" i
  if ! command -v wmctrl >/dev/null 2>&1; then
    sleep 5
    return 0
  fi
  for ((i = 0; i < timeout; i++)); do
    if wmctrl -l 2>/dev/null | grep -i -- "$pattern" >/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

# Si el wineserver del prefijo se reinicia al abrir una app, Wine mata el
# espejo; el vigilante lo relanza en unos segundos.
ensure_bridge() {
  local i
  for i in $(seq 1 30); do
    receiver_alive && return 0
    sleep 1
  done
  return 1
}

launch_app() {
  local dir="$1" exe="$2" tag="$3"
  if [[ "$DRY" == 1 ]]; then
    log "  (dry-run) wine start /wait /d '$dir' '$exe'"
    return 0
  fi
  log "  lanzando $tag"
  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all "$BOTTLE_WINE" start /wait /d "$dir" "$exe" \
    >>"$RUN_DIR/$tag.log" 2>&1 </dev/null &
  printf '%s\n' "$!" >"$RUN_DIR/$tag.launcher.pid"
}

log "=== arranque $(date '+%F %T') ==="
limpiar_envoltorios

if [[ -n "$(assetto_pid)" ]]; then
  log "Assetto Corsa en marcha."
else
  log "AVISO: Assetto Corsa no esta en marcha; las apps arrancaran igual y se conectaran cuando lo abras."
fi

log "1/3 espejo"
if [[ "$DRY" == 1 ]]; then
  log "  (dry-run) $ACTIVAR ${ACTIVAR_ARGS[*]}"
else
  if ! "$ACTIVAR" "${ACTIVAR_ARGS[@]}" 9>&- 2>&1 | tee -a "$LOG"; then
    log "ERROR: activar_puente_assetto.sh fallo. Revisa $LOG"
    log "AVISO: sigo con CrewChief y SimHub; el juego se quedara sin datos del espejo."
  fi
fi

if [[ "$DRY" == 0 ]]; then
  if wait_mappings 30; then
    log "  mappings acpmf_* publicados."
  else
    log "AVISO: no veo 'Mappings iniciales creados' en receiver.log."
  fi
  if ! ensure_bridge; then
    log "AVISO: el receptor no esta vivo; abro las apps igualmente y el espejo se puede recuperar despues."
  fi
fi
[[ "$ESPERA_PUENTE" -gt 0 && "$DRY" == 0 ]] && sleep "$ESPERA_PUENTE"

crew_pid="$(app_pid 'CrewChiefV4\.exe')"
if [[ -n "$crew_pid" ]]; then
  log "2/3 CrewChief ya esta en marcha (PID $crew_pid); no lo toco."
else
  log "2/3 CrewChief"
  launch_app "$CREW_DIR_WIN" "$CREW_EXE_WIN" crewchief
  if [[ "$DRY" == 0 ]]; then
    if wait_window "Crew Chief" "$TIMEOUT_APP"; then
      log "  ventana de CrewChief lista."
    else
      log "AVISO: no aparecio la ventana de CrewChief en ${TIMEOUT_APP}s; sigue cargando o fallo (ver $RUN_DIR/crewchief.log)."
    fi
    ensure_bridge || log "AVISO: el espejo no responde tras abrir CrewChief."
  fi
  if [[ "$ESPERA_APPS" -gt 0 && "$DRY" == 0 ]]; then
    log "  esperando ${ESPERA_APPS}s antes de SimHub..."
    sleep "$ESPERA_APPS"
  fi
fi

sim_pid="$(app_pid 'SimHubWPF\.exe')"
if [[ -n "$sim_pid" ]]; then
  log "3/3 SimHub ya esta en marcha (PID $sim_pid); no lo toco."
else
  log "3/3 SimHub"
  launch_app "$SIMHUB_DIR_WIN" "$SIMHUB_EXE_WIN" simhub
  if [[ "$DRY" == 0 ]]; then
    if wait_window "SimHub" "$TIMEOUT_APP"; then
      log "  ventana de SimHub lista."
    else
      log "AVISO: no aparecio la ventana de SimHub en ${TIMEOUT_APP}s (ver $RUN_DIR/simhub.log)."
    fi
    ensure_bridge || log "AVISO: el espejo no responde tras abrir SimHub."
  fi
fi

app_state() {
  local pid
  pid="$(app_pid "$1")"
  if [[ -n "$pid" ]]; then printf 'PID %s' "$pid"; else printf 'parado'; fi
}

mirror_count() {
  local pid cmd n=0
  for pid in $(pgrep -f -- 'assetto_shm_mirror\.exe --' 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == start.exe* ]] && continue
    n=$((n + 1))
  done
  printf '%s' "$n"
}

log "estado final:"
log "  espejo:    $(mirror_count) proceso(s)"
log "  Assetto Corsa:    $(if [[ -n "$(assetto_pid)" ]]; then printf 'PID %s' "$(assetto_pid)"; else printf 'parado'; fi)"
log "  CrewChief: $(app_state 'CrewChiefV4\.exe')"
log "  SimHub:    $(app_state 'SimHubWPF\.exe')"
log "  logs:      $LOG"
log "Para parar el puente: ./desactivar_puente_assetto.sh (no cierra las apps)."

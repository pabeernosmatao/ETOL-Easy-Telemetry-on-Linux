#!/usr/bin/env bash
# Activa el espejo de telemetria Assetto Corsa Competizione -> prefijo Bottles
# (SimHub/CrewChief).
#
# El espejo es de solo lectura: ACC -> UDP -> Bottles. No modifica el juego,
# no reenvia escrituras y no reinicia SimHub ni CrewChief.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
EXE="$BRIDGE/acc_shm_mirror.exe"
STUB_SRC="$BRIDGE/acc_process_stub.exe"
CONFIG_PY="$BRIDGE/configure_apps_acc.py"
RUN_DIR="${ACC_MIRROR_RUN_DIR:-/tmp/etol/acc_mirror}"
STUB_GAME="$RUN_DIR/AC2-Win64-Shipping.exe"
STUB_ACC="$RUN_DIR/acc.exe"

# En el arbol de fuentes los binarios viven en vendor/; en el payload, en bridge/.
if [[ ! -x "$EXE" && -x "$HERE/../../../vendor/acc_shm_mirror.exe" ]]; then
  EXE="$HERE/../../../vendor/acc_shm_mirror.exe"
fi
if [[ ! -x "$STUB_SRC" && -x "$HERE/../../../vendor/acc_process_stub.exe" ]]; then
  STUB_SRC="$HERE/../../../vendor/acc_process_stub.exe"
fi
if [[ ! -x "$STUB_SRC" && -x "$HERE/../../../payload/bridge/ac_process_stub.exe" ]]; then
  STUB_SRC="$HERE/../../../payload/bridge/ac_process_stub.exe"
fi

ACC_PFX="${ACC_PFX:-}"
if [[ -z "$ACC_PFX" ]]; then
  echo "ERROR: falta ACC_PFX. Ejecuta: etol detect --save o etol config set games.acc.prefix <ruta>" >&2
  exit 1
fi
ACC_WINE="${ACC_WINE:-}"
if [[ -z "$ACC_WINE" ]]; then
  echo "ERROR: falta ACC_WINE. Ejecuta: etol detect --save o etol config set games.acc.wine <ruta>" >&2
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

PORT="${ACC_MIRROR_PORT:-8779}"
HZ="${ACC_MIRROR_HZ:-60}"
SIZES="${ACC_MIRROR_SIZES:-4096,4096,4096}"
DO_CONFIG=1
SOLO_CONFIG=0
DO_WATCH=1
DO_STUB=1
VERBOSE=0

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --port N        puerto UDP local (defecto: $PORT)
  --hz N          frecuencia del espejo acpmf_* (defecto: $HZ)
  --sizes A,B,C   tamanos physics,graphics,static (defecto: $SIZES)
  --no-config     no tocar la configuracion de SimHub/CrewChief
  --solo-config   aplica la configuracion y sale, sin arrancar el puente
  --no-watch      no arrancar el vigilante que relanza el puente
  --no-stub       no arrancar los procesos inertes de deteccion
  --no-hf8        compatibilidad con ETOL; no se usa aqui
  --verbose       logs detallados del emisor/receptor
  -h, --help      esta ayuda
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="${2:?falta valor para --port}"; shift 2 ;;
    --hz) HZ="${2:?falta valor para --hz}"; shift 2 ;;
    --sizes) SIZES="${2:?falta valor para --sizes}"; shift 2 ;;
    --no-config) DO_CONFIG=0; shift ;;
    --solo-config) SOLO_CONFIG=1; shift ;;
    --no-watch) DO_WATCH=0; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    --no-hf8) shift ;;
    --verbose) VERBOSE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$RUN_DIR"

# El vigilante hereda estos valores para relanzar con la misma configuracion.
export ACC_MIRROR_RUN_DIR="$RUN_DIR"
export ACC_MIRROR_PORT="$PORT" ACC_MIRROR_HZ="$HZ" ACC_MIRROR_SIZES="$SIZES"
export ACC_PFX ACC_WINE BOTTLE_WINE
export SIMHUB_BOTTLE="$BOTTLE"
if [[ "$DO_STUB" != 1 ]]; then
  export ACC_MIRROR_STUB=0
fi

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

normalize() {
  printf '%s' "${1//\\//}"
}

# Stubs inertes: SimHub/CrewChief detectan el juego por proceso. ACC corre en
# el prefijo de Steam, asi que en Bottles solo ven estos stubs.
stub_cmd_matches() {
  local pid="$1" name="$2" cmd
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$(normalize "$cmd")" == *"$RUN_DIR/$name"* ]] || return 1
  [[ "$(normalize "$cmd")" == *"$name"* ]]
}

find_stub_pids() {
  local name="$1" pattern pid
  case "$name" in
    AC2*) pattern='AC2-Win64-Shipping[.]exe' ;;
    *) pattern='acc[.]exe' ;;
  esac
  for pid in $(pgrep -f -- "$pattern" 2>/dev/null || true); do
    if stub_cmd_matches "$pid" "$name"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

real_acc_running() {
  local pid cmd
  for pid in $(pgrep -f -- 'AC2-Win64-Shipping[.]exe' 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$(normalize "$cmd")" == *"$RUN_DIR"* ]] && continue
    return 0
  done
  return 1
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

[[ -x "$ACC_WINE" ]] || fail "no encuentro el wine de ACC: $ACC_WINE"
[[ -d "$ACC_PFX" ]] || fail "no encuentro el prefijo de ACC: $ACC_PFX"
[[ -x "$BOTTLE_WINE" ]] || fail "no encuentro el wine de Bottles: $BOTTLE_WINE"
[[ -d "$BOTTLE" ]] || fail "no encuentro la botella: $BOTTLE"
[[ -f "$BOTTLE/system.reg" ]] || fail "la botella no parece valida: $BOTTLE"
[[ -x "$EXE" ]] || fail "falta $EXE; compila scripts/juegos/acc/bridge/build_acc_mirror.sh"
if [[ "$DO_STUB" == 1 ]]; then
  [[ -x "$STUB_SRC" ]] || fail "falta $STUB_SRC"
fi

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando SimHub y CrewChief para Assetto Corsa Competizione (con copia de seguridad)..."
  SIMHUB_BOTTLE="$BOTTLE" python3 "$CONFIG_PY" || \
    echo "AVISO: la configuracion automatica fallo; el puente seguira funcionando." >&2
fi

if [[ "${SOLO_CONFIG:-0}" == 1 ]]; then
  echo "Configuracion aplicada para Assetto Corsa Competizione."
  exit 0
fi

if real_acc_running; then
  echo "Assetto Corsa Competizione detectado en marcha."
else
  echo "AVISO: Assetto Corsa Competizione no parece estar en marcha; el emisor esperara a acpmf_*."
fi

pid_alive() {
  local pidfile="$1"
  [[ -f "$pidfile" ]] || return 1
  local pid
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

is_our_process() {
  local pid="$1" cmd
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"acc_shm_mirror.exe"* ]]
}

find_existing() {
  local mode="$1" pid cmd
  for pid in $(pgrep -f -- "acc_shm_mirror\\.exe --${mode}" 2>/dev/null || true); do
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == start.exe* ]] && continue
    [[ "$cmd" == *"/acc_shm_mirror.exe"* || "$cmd" == *"\\acc_shm_mirror.exe"* ]] && {
      printf '%s\n' "$pid"
      return 0
    }
  done
  return 0
}

start_process() {
  local name="$1" prefix="$2" wine="$3"
  shift 3
  local pidfile="$RUN_DIR/$name.pid"
  local logfile="$RUN_DIR/$name.log"

  if pid_alive "$pidfile"; then
    echo "$name ya esta en marcha (PID $(cat "$pidfile"))."
    return 0
  fi
  rm -f "$pidfile"

  local existing
  existing="$(find_existing "$name")"
  if [[ -n "$existing" ]]; then
    echo "$existing" > "$pidfile"
    echo "$name ya estaba en marcha (PID $existing)."
    return 0
  fi

  setsid env WINEPREFIX="$prefix" WINEDEBUG=-all "$wine" "$EXE" "$@" \
    >"$logfile" 2>&1 < /dev/null &
  local pid=$!
  echo "$pid" > "$pidfile"
  sleep 1

  if ! kill -0 "$pid" 2>/dev/null; then
    echo "ERROR: $name no arranco. Ultimas lineas de $logfile:" >&2
    tail -n 20 "$logfile" >&2 || true
    rm -f "$pidfile"
    return 1
  fi
  echo "$name arrancado (PID $pid, log $logfile)."
}

start_one_stub() {
  local name="$1" target="$RUN_DIR/$1" existing
  existing="$(find_stub_pids "$name" | head -n1 || true)"
  if [[ -n "$existing" ]]; then
    echo "stub $name ya estaba en marcha (PID $existing)."
    return 0
  fi

  cp -f "$STUB_SRC" "$target.tmp"
  chmod +x "$target.tmp" 2>/dev/null || true
  mv -f "$target.tmp" "$target"

  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all "$BOTTLE_WINE" "$target" \
    >"$RUN_DIR/stub-$name.log" 2>&1 < /dev/null &
  local launcher=$!
  local found="" i
  for i in $(seq 1 50); do
    found="$(find_stub_pids "$name" | head -n1 || true)"
    [[ -n "$found" ]] && break
    sleep 0.1
  done
  if [[ -z "$found" ]]; then
    echo "ERROR: el stub $name no arranco. Ultimas lineas:" >&2
    tail -n 20 "$RUN_DIR/stub-$name.log" >&2 || true
    kill "$launcher" 2>/dev/null || true
    return 1
  fi
  echo "$found" > "$RUN_DIR/stub-$name.pid"
  echo "stub $name arrancado (PID $found)."
}

start_stub() {
  if [[ "$DO_STUB" != 1 ]]; then
    echo "stubs de deteccion omitidos (--no-stub)."
    return 0
  fi
  start_one_stub "acc.exe" || return 1
  start_one_stub "AC2-Win64-Shipping.exe" || return 1
}

RECEIVER_ARGS=(--receiver --bind "127.0.0.1:$PORT" --sizes "$SIZES")
SENDER_ARGS=(--sender --udp "127.0.0.1:$PORT" --hz "$HZ")
if [[ "$VERBOSE" == 1 ]]; then
  RECEIVER_ARGS+=(--verbose)
  SENDER_ARGS+=(--verbose)
fi

start_process receiver "$BOTTLE" "$BOTTLE_WINE" "${RECEIVER_ARGS[@]}" || fail "no se pudo arrancar el receptor"

if ! wait_for_log "$RUN_DIR/receiver.log" "Mappings iniciales creados" 20; then
  echo "ERROR: el receptor no publico los mappings iniciales." >&2
  tail -n 30 "$RUN_DIR/receiver.log" 2>/dev/null >&2 || true
  fail "revisa el log del receptor"
fi
echo "Mappings acpmf_* del receptor listos."

start_process sender "$ACC_PFX" "$ACC_WINE" "${SENDER_ARGS[@]}" || fail "no se pudo arrancar el emisor"

if real_acc_running; then
  if wait_for_log "$RUN_DIR/receiver.log" "publicado" 15; then
    echo "Primeros datos de Assetto Corsa Competizione recibidos."
  else
    echo "AVISO: aun no llegan datos de ACC; el espejo seguira intentandolo." >&2
  fi
fi

if ! start_stub; then
  echo "AVISO: SimHub/CrewChief podrian no detectar ACC hasta que arranquen los stubs." >&2
fi

WATCHDOG="$BRIDGE/acc_bridge_watchdog.sh"
if [[ "$DO_WATCH" == 1 && -x "$WATCHDOG" ]]; then
  if [[ -f "$RUN_DIR/watchdog.pid" ]] && kill -0 "$(cat "$RUN_DIR/watchdog.pid" 2>/dev/null)" 2>/dev/null; then
    echo "vigilante ya estaba en marcha (PID $(cat "$RUN_DIR/watchdog.pid"))."
  else
    setsid "$WATCHDOG" >/dev/null 2>&1 </dev/null &
    sleep 0.5
    if [[ -f "$RUN_DIR/watchdog.pid" ]] && kill -0 "$(cat "$RUN_DIR/watchdog.pid")" 2>/dev/null; then
      echo "vigilante arrancado (PID $(cat "$RUN_DIR/watchdog.pid"), log $RUN_DIR/watchdog.log)."
    else
      echo "AVISO: el vigilante no arranco; el puente seguira sin autorecuperacion." >&2
    fi
  fi
fi

sleep 1
echo
echo "Estado del espejo:"
pgrep -af 'acc_shm_mirror\.exe' || true
for name in acc.exe AC2-Win64-Shipping.exe; do
  pids="$(find_stub_pids "$name" | tr '\n' ' ' || true)"
  echo "Stub $name: ${pids:-no esta en marcha}"
done
echo
echo "Logs: $RUN_DIR/receiver.log, $RUN_DIR/sender.log y $RUN_DIR/stub-*.log"
tail -n 5 "$RUN_DIR/receiver.log" 2>/dev/null || true
tail -n 5 "$RUN_DIR/sender.log" 2>/dev/null || true
echo
cat <<'EOF'
Siguiente paso:
  - Arranque ordenado de todo: ./arrancar_todo_acc.sh
  - SimHub: si ya estaba en Assetto Corsa Competizione, deberia conectar sin
    reiniciar. Si no, selecciona ACC cuando no estes en pista.
  - CrewChief: si estaba en otro juego, cámbialo a ACC cuando no estes en
    pista. La configuracion automatica solo se aplica con CrewChief cerrado.
  - El puente no reinicia apps ni escribe en el juego (solo lee acpmf_*).
  - El vigilante (bridge/acc_bridge_watchdog.sh) relanza el puente si Wine lo
    mata; su log es watchdog.log. Se desactiva con --no-watch.
Para parar el puente: ./desactivar_puente_acc.sh
EOF

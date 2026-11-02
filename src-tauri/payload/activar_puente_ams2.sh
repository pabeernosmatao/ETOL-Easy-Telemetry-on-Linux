#!/usr/bin/env bash
# Activa el espejo de telemetria AMS2 -> prefijo Bottles (SimHub/CrewChief).
#
# El espejo es de solo lectura: AMS2 -> UDP -> Bottles. No modifica el juego,
# no reenvia escrituras y no reinicia SimHub ni CrewChief.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
EXE="$BRIDGE/ams2_shm_mirror.exe"
STUB_SRC="$BRIDGE/ams2_process_stub.exe"
CONFIG_PY="$BRIDGE/configure_apps_ams2.py"
RUN_DIR="${AMS2_MIRROR_RUN_DIR:-/tmp/etol/ams2_mirror}"
STUB_EXE="$RUN_DIR/AMS2AVX.exe"

AMS2_PFX="${AMS2_PFX:-}"
if [[ -z "$AMS2_PFX" ]]; then
  echo "ERROR: falta AMS2_PFX. Ejecuta: etol detect --save o etol config set games.ams2.prefix <ruta>" >&2
  exit 1
fi
AMS2_WINE="${AMS2_WINE:-}"
if [[ -z "$AMS2_WINE" ]]; then
  echo "ERROR: falta AMS2_WINE. Ejecuta: etol detect --save o etol config set games.ams2.wine <ruta>" >&2
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

PORT="${AMS2_MIRROR_PORT:-8773}"
HZ="${AMS2_MIRROR_HZ:-60}"
SIZE="${AMS2_MIRROR_SIZE:-24576}"
DO_CONFIG=1
SOLO_CONFIG=0
DO_BUILD=1
DO_WATCH=1
DO_STUB=1
DO_HF8=1
VERBOSE=0

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --port N        puerto UDP local (defecto: $PORT)
  --hz N          frecuencia del espejo \$pcars2\$ (defecto: $HZ)
  --size N        tamano del mapping en Bottles (defecto: $SIZE)
  --no-config     no tocar la configuracion de SimHub/CrewChief
  --no-build      no recompilar aunque falte el .exe
  --no-watch      no arrancar el vigilante que relanza el puente
  --no-stub       no arrancar el proceso inerte AMS2AVX.exe
  --no-hf8        no configurar el pad haptico HF8 (ForceFeel)
  --verbose       logs detallados del emisor/receptor
  -h, --help      esta ayuda
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="${2:?falta valor para --port}"; shift 2 ;;
    --hz) HZ="${2:?falta valor para --hz}"; shift 2 ;;
    --size) SIZE="${2:?falta valor para --size}"; shift 2 ;;
    --no-config) DO_CONFIG=0; shift ;;
    --solo-config) SOLO_CONFIG=1; shift ;;
    --no-build) DO_BUILD=0; shift ;;
    --no-watch) DO_WATCH=0; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    --no-hf8) DO_HF8=0; shift ;;
    --verbose) VERBOSE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$RUN_DIR"

# El vigilante hereda estos valores para relanzar con la misma configuracion.
export AMS2_MIRROR_RUN_DIR="$RUN_DIR"
export AMS2_MIRROR_PORT="$PORT" AMS2_MIRROR_HZ="$HZ" AMS2_MIRROR_SIZE="$SIZE"
export AMS2_PFX AMS2_WINE BOTTLE_WINE
export SIMHUB_BOTTLE="$BOTTLE"
if [[ "$DO_STUB" != 1 ]]; then
  export AMS2_MIRROR_STUB=0
fi

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

# El stub es un .exe inerte llamado AMS2AVX.exe que solo sirve para que
# CrewChief (y SimHub, si detecta por proceso) vean AMS2 dentro de Bottles.
stub_cmd_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  local cmd norm
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline")"
  [[ "$cmd" == *"AMS2AVX.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'AMS2AVX[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

real_ams2_running() {
  local pid
  for pid in $(pgrep -f -- 'AMS2AVX[.]exe' 2>/dev/null || true); do
    if ! stub_cmd_matches "$pid"; then
      return 0
    fi
  done
  # Tambien cuenta el lanzador AMS2.exe, aunque el proceso real sea AMS2AVX.
  for pid in $(pgrep -f -- 'AMS2[.]exe' 2>/dev/null || true); do
    local cmd
    cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ "$cmd" == *"steamapps/common/Automobilista 2"* ]] && return 0
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

[[ -x "$AMS2_WINE" ]] || fail "no encuentro el wine de AMS2: $AMS2_WINE"
[[ -d "$AMS2_PFX" ]] || fail "no encuentro el prefijo de AMS2: $AMS2_PFX"
[[ -x "$BOTTLE_WINE" ]] || fail "no encuentro el wine de Bottles: $BOTTLE_WINE"
[[ -d "$BOTTLE" ]] || fail "no encuentro la botella: $BOTTLE"
[[ -f "$BOTTLE/system.reg" ]] || fail "la botella no parece valida: $BOTTLE"

if [[ "$DO_BUILD" == 1 ]]; then
  if [[ ! -x "$EXE" || "$BRIDGE/ams2_shm_mirror.cpp" -nt "$EXE" ]]; then
    echo "Compilando espejo AMS2..."
    "$BRIDGE/build_ams2_mirror.sh"
  fi
  if [[ "$DO_STUB" == 1 && ( ! -x "$STUB_SRC" || "$BRIDGE/ams2_process_stub.cpp" -nt "$STUB_SRC" ) ]]; then
    echo "Compilando stub de deteccion AMS2AVX.exe..."
    "$BRIDGE/build_ams2_stub.sh"
  fi
fi
[[ -x "$EXE" ]] || fail "falta $EXE; ejecuta ./bridge/build_ams2_mirror.sh"
if [[ "$DO_STUB" == 1 ]]; then
  [[ -x "$STUB_SRC" ]] || fail "falta $STUB_SRC; ejecuta ./bridge/build_ams2_stub.sh"
fi

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando SimHub y CrewChief para AMS2 (con copia de seguridad)..."
  SIMHUB_BOTTLE="$BOTTLE" python3 "$CONFIG_PY" || \
    echo "AVISO: la configuracion automatica fallo; el puente seguira funcionando." >&2
fi

if [[ "${SOLO_CONFIG:-0}" == 1 ]]; then
  echo "Configuracion aplicada."
  exit 0
fi

if real_ams2_running; then
  echo "AMS2 detectado en marcha."
else
  echo "AVISO: AMS2 no parece estar en marcha; el emisor esperara a \$pcars2\$."
fi

pid_alive() {
  local pidfile="$1"
  [[ -f "$pidfile" ]] || return 1
  local pid
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

is_our_process() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  tr '\0' ' ' < "/proc/$pid/cmdline" | grep -q 'ams2_shm_mirror\.exe'
}

find_existing() {
  local mode="$1"
  pgrep -f -- "ams2_shm_mirror\.exe --${mode}" 2>/dev/null | head -n1 || true
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
    if grep -qi "version mismatch" "$logfile" 2>/dev/null; then
      echo "PISTA: el juego se abrio con otro Proton. Su wineserver no es compatible con el de Bottles." >&2
      echo "       Cierra el juego, abrelo con el Proton configurado en ETOL y vuelve a lanzar." >&2
    fi
    rm -f "$pidfile"
    return 1
  fi
  echo "$name arrancado (PID $pid, log $logfile)."
}

start_stub() {
  if [[ "$DO_STUB" != 1 ]]; then
    echo "stub de deteccion AMS2AVX.exe omitido (--no-stub)."
    return 0
  fi

  # Copia atomica: si el stub anterior sigue en marcha, no se puede truncar su
  # binario (ETXTBSY); renombrar deja el inodo viejo intacto.
  cp -f "$STUB_SRC" "$STUB_EXE.tmp"
  chmod +x "$STUB_EXE.tmp" 2>/dev/null || true
  mv -f "$STUB_EXE.tmp" "$STUB_EXE"

  local existing
  existing="$(find_stub_pids | head -n1 || true)"
  if [[ -n "$existing" ]]; then
    echo "$existing" > "$RUN_DIR/stub.pid"
    echo "stub ya estaba en marcha (PID $existing)."
    return 0
  fi
  rm -f "$RUN_DIR/stub.pid"

  setsid env WINEPREFIX="$BOTTLE" WINEDEBUG=-all "$BOTTLE_WINE" "$STUB_EXE" \
    >"$RUN_DIR/stub.log" 2>&1 < /dev/null &
  local launcher_pid=$!
  echo "$launcher_pid" > "$RUN_DIR/stub.pid"

  local found="" i
  for i in $(seq 1 50); do
    found="$(find_stub_pids | head -n1 || true)"
    [[ -n "$found" ]] && break
    sleep 0.1
  done
  if [[ -z "$found" ]]; then
    echo "ERROR: el stub no arranco. Ultimas lineas de $RUN_DIR/stub.log:" >&2
    tail -n 20 "$RUN_DIR/stub.log" >&2 || true
    rm -f "$RUN_DIR/stub.pid"
    return 1
  fi
  echo "$found" > "$RUN_DIR/stub.pid"
  echo "stub de deteccion arrancado (PID $found, log $RUN_DIR/stub.log)."
}

RECEIVER_ARGS=(--receiver --bind "127.0.0.1:$PORT" --size "$SIZE")
SENDER_ARGS=(--sender --udp "127.0.0.1:$PORT" --hz "$HZ")
if [[ "$VERBOSE" == 1 ]]; then
  RECEIVER_ARGS+=(--verbose)
  SENDER_ARGS+=(--verbose)
fi

start_process receiver "$BOTTLE" "$BOTTLE_WINE" "${RECEIVER_ARGS[@]}" || fail "no se pudo arrancar el receptor"

# El mapping $pcars2$ debe existir antes de que CrewChief/SimHub lo abran. Si
# ya estaban abiertos, sus lectores reintentan solos; no se reinicia nada.
if ! wait_for_log "$RUN_DIR/receiver.log" "Mappings iniciales creados" 20; then
  echo "ERROR: el receptor no publico \$pcars2\$ inicial." >&2
  tail -n 30 "$RUN_DIR/receiver.log" 2>/dev/null >&2 || true
  fail "revisa el log del receptor"
fi
echo "Mapping \$pcars2\$ del receptor listo."

# El pad HF8 necesita que el wineserver de Bottles ya este vivo (lo arranca el
# receptor) para que sus claves y el enlace COM queden en la sesion correcta.
# Solo se toca el registro y dosdevices; no se abre el puerto ni se reinicia nada.
if [[ "$DO_HF8" == 1 ]]; then
  HF8_SETUP="$BRIDGE/setup_hf8.sh"
  if [[ -x "$HF8_SETUP" ]]; then
    echo "Configurando pad haptico HF8 (ForceFeel)..."
    SIMHUB_BOTTLE="$BOTTLE" BOTTLE_WINE="$BOTTLE_WINE" \
      LMU_MIRROR_RUN_DIR="$RUN_DIR" "$HF8_SETUP" || \
      echo "AVISO: no se pudo configurar el HF8; SimHub seguira sin el pad." >&2
  fi
fi

start_process sender "$AMS2_PFX" "$AMS2_WINE" "${SENDER_ARGS[@]}" || fail "no se pudo arrancar el emisor"

if real_ams2_running; then
  if wait_for_log "$RUN_DIR/receiver.log" '$pcars2$ publicado' 15; then
    echo "Primeros datos de AMS2 recibidos."
  else
    echo "AVISO: aun no llegan datos de AMS2; el espejo seguira intentandolo." >&2
  fi
fi

# El stub se arranca DESPUES del mapping: si CrewChief/SimHub lo detectan, el
# lector ya encuentra $pcars2$. No abre el juego ni toca su memoria.
if ! start_stub; then
  echo "AVISO: CrewChief podria no detectar AMS2 hasta que el stub arranque." >&2
fi

# Vigilante: si el wineserver de Bottles se reinicia, Wine mata receptor,
# emisor y stub. El vigilante solo relanza procesos del puente.
WATCHDOG="$BRIDGE/ams2_bridge_watchdog.sh"
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
pgrep -af 'ams2_shm_mirror\.exe' || true
local_stub_pids="$(find_stub_pids | tr '\n' ' ' || true)"
if [[ -n "$local_stub_pids" ]]; then
  echo "Stub de deteccion: $local_stub_pids"
else
  echo "Stub de deteccion: no esta en marcha."
fi
echo
echo "Logs: $RUN_DIR/receiver.log, $RUN_DIR/sender.log y $RUN_DIR/stub.log"
tail -n 5 "$RUN_DIR/receiver.log" 2>/dev/null || true
tail -n 5 "$RUN_DIR/sender.log" 2>/dev/null || true
echo
cat <<'EOF'
Siguiente paso:
  - Arranque ordenado de todo: ./arrancar_todo_ams2.sh
  - SimHub: si ya estaba en Automobilista 2, deberia conectar sin reiniciar.
    Si no, selecciona Automobilista 2 cuando no estes en pista.
  - CrewChief: si ya estaba en Automobilista 2, deberia conectar sin reiniciar.
    Si estaba en LMU, cámbialo a Automobilista 2 cuando no estes en pista.
  - El puente no reinicia apps ni escribe en el juego (solo lee $pcars2$).
  - El vigilante (bridge/ams2_bridge_watchdog.sh) relanza el puente si Wine lo
    mata; su log es watchdog.log. Se desactiva con --no-watch.
Para parar el puente: ./desactivar_puente_ams2.sh
EOF

#!/usr/bin/env bash
# Activa el espejo de telemetria AC EVO -> prefijo Bottles (SimHub/CrewChief).
#
# El espejo es de solo lectura: AC EVO -> UDP -> Bottles. No modifica el juego,
# no reenvia escrituras y no reinicia SimHub ni CrewChief.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
EXE="$BRIDGE/acevo_shm_mirror.exe"
STUB_SRC="$BRIDGE/acevo_process_stub.exe"
CONFIG_PY="$BRIDGE/configure_apps_acevo.py"
RUN_DIR="${ACEVO_MIRROR_RUN_DIR:-/tmp/etol/acevo_mirror}"
STUB_EXE="$RUN_DIR/AssettoCorsaEVO.exe"

ACEVO_PFX="${ACEVO_PFX:-}"
if [[ -z "$ACEVO_PFX" ]]; then
  echo "ERROR: falta ACEVO_PFX. Ejecuta: etol detect --save o etol config set games.acevo.prefix <ruta>" >&2
  exit 1
fi
ACEVO_WINE="${ACEVO_WINE:-}"
if [[ -z "$ACEVO_WINE" ]]; then
  echo "ERROR: falta ACEVO_WINE. Ejecuta: etol detect --save o etol config set games.acevo.wine <ruta>" >&2
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

PORT="${ACEVO_MIRROR_PORT:-8774}"
HZ="${ACEVO_MIRROR_HZ:-60}"
SIZES="${ACEVO_MIRROR_SIZES:-4096,8192,4096}"
DO_CONFIG=1
DO_HF8=1
SOLO_CONFIG=0
DO_BUILD=1
DO_WATCH=1
DO_STUB=1
VERBOSE=0

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --port N        puerto UDP local (defecto: $PORT)
  --hz N          frecuencia del espejo acevo_pmf_* (defecto: $HZ)
  --sizes A,B,C   tamanos physics,graphics,static (defecto: $SIZES)
  --no-config     no tocar la configuracion de SimHub/CrewChief
  --no-build      no recompilar aunque falte el .exe
  --no-watch      no arrancar el vigilante que relanza el puente
  --no-stub       no arrancar el proceso inerte AssettoCorsaEVO.exe
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
    --no-hf8) DO_HF8=0; shift ;;
    --solo-config) SOLO_CONFIG=1; shift ;;
    --no-build) DO_BUILD=0; shift ;;
    --no-watch) DO_WATCH=0; shift ;;
    --no-stub) DO_STUB=0; shift ;;
    --verbose) VERBOSE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$RUN_DIR"

# El vigilante hereda estos valores para relanzar con la misma configuracion.
export ACEVO_MIRROR_RUN_DIR="$RUN_DIR"
export ACEVO_MIRROR_PORT="$PORT" ACEVO_MIRROR_HZ="$HZ" ACEVO_MIRROR_SIZES="$SIZES"
export ACEVO_PFX ACEVO_WINE BOTTLE_WINE
export SIMHUB_BOTTLE="$BOTTLE"
if [[ "$DO_STUB" != 1 ]]; then
  export ACEVO_MIRROR_STUB=0
fi

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

# El stub es un .exe inerte llamado AssettoCorsaEVO.exe que solo sirve para que
# CrewChief (y SimHub, si detecta por proceso) vean AC EVO dentro de Bottles.
stub_cmd_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  local cmd norm
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"AssettoCorsaEVO.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'AssettoCorsaEVO[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

real_acevo_running() {
  local pid
  for pid in $(pgrep -f -- 'AssettoCorsaEVO[.]exe' 2>/dev/null || true); do
    if ! stub_cmd_matches "$pid"; then
      return 0
    fi
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

[[ -x "$ACEVO_WINE" ]] || fail "no encuentro el wine de AC EVO: $ACEVO_WINE"
[[ -d "$ACEVO_PFX" ]] || fail "no encuentro el prefijo de AC EVO: $ACEVO_PFX"
[[ -x "$BOTTLE_WINE" ]] || fail "no encuentro el wine de Bottles: $BOTTLE_WINE"
[[ -d "$BOTTLE" ]] || fail "no encuentro la botella: $BOTTLE"
[[ -f "$BOTTLE/system.reg" ]] || fail "la botella no parece valida: $BOTTLE"

if [[ "$DO_BUILD" == 1 ]]; then
  if [[ ! -x "$EXE" || "$BRIDGE/acevo_shm_mirror.cpp" -nt "$EXE" ]]; then
    echo "Compilando espejo AC EVO..."
    "$BRIDGE/build_acevo_mirror.sh"
  fi
  if [[ "$DO_STUB" == 1 && ( ! -x "$STUB_SRC" || "$BRIDGE/acevo_process_stub.cpp" -nt "$STUB_SRC" ) ]]; then
    echo "Compilando stub de deteccion AssettoCorsaEVO.exe..."
    "$BRIDGE/build_acevo_stub.sh"
  fi
fi
[[ -x "$EXE" ]] || fail "falta $EXE; ejecuta ./bridge/build_acevo_mirror.sh"
if [[ "$DO_STUB" == 1 ]]; then
  [[ -x "$STUB_SRC" ]] || fail "falta $STUB_SRC; ejecuta ./bridge/build_acevo_stub.sh"
fi

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando SimHub y CrewChief para AC EVO (con copia de seguridad)..."
  SIMHUB_BOTTLE="$BOTTLE" python3 "$CONFIG_PY" || \
    echo "AVISO: la configuracion automatica fallo; el puente seguira funcionando." >&2
fi

if [[ "${SOLO_CONFIG:-0}" == 1 ]]; then
  echo "Configuracion aplicada."
  exit 0
fi

if real_acevo_running; then
  echo "AC EVO detectado en marcha."
else
  echo "AVISO: AC EVO no parece estar en marcha; el emisor esperara a acevo_pmf_*."
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
  [[ "$cmd" == *"acevo_shm_mirror.exe"* ]]
}

find_existing() {
  local mode="$1"
  pgrep -f -- "acevo_shm_mirror\.exe --${mode}" 2>/dev/null | head -n1 || true
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
    echo "stub de deteccion AssettoCorsaEVO.exe omitido (--no-stub)."
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

RECEIVER_ARGS=(--receiver --bind "127.0.0.1:$PORT" --sizes "$SIZES")
SENDER_ARGS=(--sender --udp "127.0.0.1:$PORT" --hz "$HZ")
if [[ "$VERBOSE" == 1 ]]; then
  RECEIVER_ARGS+=(--verbose)
  SENDER_ARGS+=(--verbose)
fi

start_process receiver "$BOTTLE" "$BOTTLE_WINE" "${RECEIVER_ARGS[@]}" || fail "no se pudo arrancar el receptor"

# Los mappings deben existir antes de que CrewChief/SimHub los abran. Si ya
# estaban abiertos, sus lectores reintentan solos; no se reinicia nada.
if ! wait_for_log "$RUN_DIR/receiver.log" "Mappings iniciales creados" 20; then
  echo "ERROR: el receptor no publico los mappings iniciales." >&2
  tail -n 30 "$RUN_DIR/receiver.log" 2>/dev/null >&2 || true
  fail "revisa el log del receptor"
fi
echo "Mappings acevo_pmf_* del receptor listos."

# El HF8 necesita el wineserver de Bottles vivo; el receptor ya lo arranco.
if [[ "$DO_HF8" == 1 ]]; then
  HF8_SETUP="$BRIDGE/setup_hf8.sh"
  if [[ -x "$HF8_SETUP" ]]; then
    echo "Configurando pad haptico HF8 (ForceFeel)..."
    SIMHUB_BOTTLE="$BOTTLE" BOTTLE_WINE="$BOTTLE_WINE" \
      LMU_MIRROR_RUN_DIR="$RUN_DIR" "$HF8_SETUP" || \
      echo "AVISO: no se pudo configurar el HF8; SimHub seguira sin el pad." >&2
  fi
fi

start_process sender "$ACEVO_PFX" "$ACEVO_WINE" "${SENDER_ARGS[@]}" || fail "no se pudo arrancar el emisor"

if real_acevo_running; then
  if wait_for_log "$RUN_DIR/receiver.log" "publicado" 15; then
    echo "Primeros datos de AC EVO recibidos."
  else
    echo "AVISO: aun no llegan datos de AC EVO; el espejo seguira intentandolo." >&2
  fi
fi

# El stub se arranca DESPUES de los mappings: si CrewChief/SimHub lo detectan,
# el lector ya encuentra acevo_pmf_*. No abre el juego ni toca su memoria.
if ! start_stub; then
  echo "AVISO: CrewChief podria no detectar AC EVO hasta que el stub arranque." >&2
fi

# Vigilante: si el wineserver de Bottles se reinicia, Wine mata receptor,
# emisor y stub. El vigilante solo relanza procesos del puente.
WATCHDOG="$BRIDGE/acevo_bridge_watchdog.sh"
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
pgrep -af 'acevo_shm_mirror\.exe' || true
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
  - Arranque ordenado de todo: ./arrancar_todo_acevo.sh
  - SimHub: si ya estaba en Assetto Corsa EVO, deberia conectar sin reiniciar.
    Si no, selecciona Assetto Corsa EVO cuando no estes en pista.
  - CrewChief: si ya estaba en Assetto Corsa EVO, deberia conectar sin
    reiniciar. Si estaba en LMU/AMS2, cámbialo a Assetto Corsa EVO cuando no
    estes en pista. La configuracion automatica solo se aplica con CrewChief
    cerrado.
  - El puente no reinicia apps ni escribe en el juego (solo lee acevo_pmf_*).
  - El vigilante (bridge/acevo_bridge_watchdog.sh) relanza el puente si Wine lo
    mata; su log es watchdog.log. Se desactiva con --no-watch.
Para parar el puente: ./desactivar_puente_acevo.sh
EOF

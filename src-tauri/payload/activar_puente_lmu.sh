#!/usr/bin/env bash
# Activa el espejo de telemetria LMU -> prefijo Bottles (SimHub/CrewChief).
#
# El espejo es de solo lectura: LMU -> UDP -> Bottles. No modifica el juego,
# no reenvia escrituras y no reinicia SimHub ni CrewChief.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
EXE="$BRIDGE/lmu_shm_mirror.exe"
SENDER_EXE="$BRIDGE/lmu_shm_sender.exe"
STUB_SRC="$BRIDGE/lmu_process_stub.exe"
CONFIG_PY="$BRIDGE/configure_apps.py"
RUN_DIR="${LMU_MIRROR_RUN_DIR:-/tmp/etol/lmu_mirror}"
STUB_EXE="$RUN_DIR/Le Mans Ultimate.exe"

LMU_PROTON="${LMU_PROTON:-}"
if [[ -z "$LMU_PROTON" ]]; then
  echo "ERROR: falta LMU_PROTON. Ejecuta: etol detect --save o etol config set games.lmu.proton <ruta>" >&2
  exit 1
fi
LMU_PFX="${LMU_PFX:-}"
if [[ -z "$LMU_PFX" ]]; then
  echo "ERROR: falta LMU_PFX. Ejecuta: etol detect --save o etol config set games.lmu.prefix <ruta>" >&2
  exit 1
fi
LMU_WINE="$LMU_PROTON/files/bin/wine"

LMU_GAME_DIR="${LMU_GAME_DIR:-}"
if [[ -z "$LMU_GAME_DIR" ]]; then
  _lmu_lib="${LMU_PFX%/steamapps/compatdata/*}"
  if [[ "$_lmu_lib" != "$LMU_PFX" ]]; then
    LMU_GAME_DIR="$_lmu_lib/steamapps/common/Le Mans Ultimate"
  fi
fi
if [[ -n "$LMU_GAME_DIR" && -d "$LMU_GAME_DIR" ]]; then
  mkdir -p "$RUN_DIR/Plugins" "$RUN_DIR/UserData/player"
  cp -f "$LMU_GAME_DIR/Plugins/"*.dll "$RUN_DIR/Plugins/" 2>/dev/null || true
  cp -f "$LMU_GAME_DIR/UserData/player/CustomPluginVariables.JSON" \
        "$RUN_DIR/UserData/player/CustomPluginVariables.JSON" 2>/dev/null || true
fi

LMU_GAME_DIR="${LMU_GAME_DIR:-}"
if [[ -z "$LMU_GAME_DIR" ]]; then
  _lmu_lib="${LMU_PFX%/steamapps/compatdata/*}"
  if [[ "$_lmu_lib" != "$LMU_PFX" ]]; then
    LMU_GAME_DIR="$_lmu_lib/steamapps/common/Le Mans Ultimate"
  fi
fi
if [[ -n "$LMU_GAME_DIR" && -d "$LMU_GAME_DIR" ]]; then
  mkdir -p "$RUN_DIR/Plugins" "$RUN_DIR/UserData/player"
  cp -f "$LMU_GAME_DIR/Plugins/"*.dll "$RUN_DIR/Plugins/" 2>/dev/null || true
  cp -f "$LMU_GAME_DIR/UserData/player/CustomPluginVariables.JSON" \
        "$RUN_DIR/UserData/player/CustomPluginVariables.JSON" 2>/dev/null || true
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

PORT="${LMU_MIRROR_PORT:-8772}"
HZ="${LMU_MIRROR_HZ:-30}"
SMMP_HZ="${LMU_MIRROR_SMMP_HZ:-10}"
DO_CONFIG=1
SOLO_CONFIG=0
DO_BUILD=1
DO_WATCH=1
DO_HF8=1
VERBOSE=0

usage() {
  cat <<EOF
Uso: $0 [opciones]

  --port N        puerto UDP local (defecto: $PORT)
  --hz N          frecuencia de LMU_Data (defecto: $HZ)
  --smmp-hz N     frecuencia de los buffers SMMP (defecto: $SMMP_HZ)
  --no-config     no tocar la configuracion de SimHub/CrewChief
  --no-build      no recompilar aunque falte el .exe
  --no-watch      no arrancar el vigilante que relanza el puente
  --no-hf8        no configurar el pad haptico HF8 (ForceFeel)
  --verbose       logs detallados del emisor/receptor
  -h, --help      esta ayuda
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="${2:?falta valor para --port}"; shift 2 ;;
    --hz) HZ="${2:?falta valor para --hz}"; shift 2 ;;
    --smmp-hz) SMMP_HZ="${2:?falta valor para --smmp-hz}"; shift 2 ;;
    --no-config) DO_CONFIG=0; shift ;;
    --solo-config) SOLO_CONFIG=1; shift ;;
    --no-build) DO_BUILD=0; shift ;;
    --no-watch) DO_WATCH=0; shift ;;
    --no-hf8) DO_HF8=0; shift ;;
    --verbose) VERBOSE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opcion desconocida: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$RUN_DIR"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

# El stub es un .exe inerte llamado "Le Mans Ultimate.exe" que solo sirve para
# que SimHub detecte el juego dentro del prefijo de Bottles. No lee memoria.
stub_cmd_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  local cmd norm
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline")"
  [[ "$cmd" == *"Le Mans Ultimate.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'Le Mans Ultimate[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

real_lmu_running() {
  local pid
  for pid in $(pgrep -f -- 'Le Mans Ultimate[.]exe' 2>/dev/null || true); do
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

[[ -x "$LMU_WINE" ]] || fail "no encuentro el wine de LMU: $LMU_WINE"
[[ -d "$LMU_PFX" ]] || fail "no encuentro el prefijo de LMU: $LMU_PFX"
[[ -x "$BOTTLE_WINE" ]] || fail "no encuentro el wine de Bottles: $BOTTLE_WINE"
[[ -d "$BOTTLE" ]] || fail "no encuentro la botella: $BOTTLE"
[[ -f "$BOTTLE/system.reg" ]] || fail "la botella no parece valida: $BOTTLE"

if [[ "$DO_BUILD" == 1 ]]; then
  if [[ ! -x "$EXE" || "$BRIDGE/lmu_shm_mirror.cpp" -nt "$EXE" ]]; then
    echo "Compilando espejo..."
    "$BRIDGE/build_mirror.sh"
  fi
  if [[ ! -x "$STUB_SRC" || "$BRIDGE/lmu_process_stub.cpp" -nt "$STUB_SRC" ]]; then
    echo "Compilando stub de deteccion..."
    "$BRIDGE/build_stub.sh"
  fi
fi
[[ -x "$EXE" ]] || fail "falta $EXE; ejecuta ./bridge/build_mirror.sh"
[[ -x "$STUB_SRC" ]] || fail "falta $STUB_SRC; ejecuta ./bridge/build_stub.sh"

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando SimHub y CrewChief (con copia de seguridad)..."
  SIMHUB_BOTTLE="$BOTTLE" python3 "$CONFIG_PY" || \
    echo "AVISO: la configuracion automatica fallo; el puente seguira funcionando." >&2
fi

# SimHub solo da LMU por configurado si el plugin SMMP esta activo en
# UserData/player/CustomPluginVariables.JSON; CrewChief ademas lo busca
# en Plugins/. Se hace con copia de seguridad y sin tocar el resto.
if [[ -n "$LMU_GAME_DIR" && -d "$LMU_GAME_DIR" ]]; then
  # SimHub valida el MD5 contra su copia de _Addons; se prefiere esa.
  LMU_PLUGIN_SRC="$BOTTLE/drive_c/Program Files (x86)/SimHub/_Addons/GamePlugins/RFactor2/Bin64/Plugins/rFactor2SharedMemoryMapPlugin64.dll"
  [[ -f "$LMU_PLUGIN_SRC" ]] || LMU_PLUGIN_SRC="$BRIDGE/rFactor2SharedMemoryMapPlugin64.dll"
  python3 "$BRIDGE/ensure_lmu_plugin.py" "$LMU_GAME_DIR" "$LMU_PLUGIN_SRC" \
    "$BOTTLE/drive_c/Program Files (x86)/SimHub" || \
    echo "AVISO: no se pudo activar el plugin SMMP en LMU." >&2
fi

if [[ "${SOLO_CONFIG:-0}" == 1 ]]; then
  echo "Configuracion aplicada."
  exit 0
fi

if real_lmu_running; then
  echo "LMU detectado en marcha."
else
  echo "AVISO: LMU no parece estar en marcha; el emisor esperara a LMU_Data."
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
  tr '\0' ' ' < "/proc/$pid/cmdline" | grep -q 'lmu_shm_mirror\.exe'
}

find_existing() {
  local mode="$1"
  pgrep -f -- "lmu_shm_mirror\.exe --${mode}" 2>/dev/null | head -n1 || true
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

RECEIVER_ARGS=(--receiver --bind "127.0.0.1:$PORT")
SENDER_ARGS=(--sender --udp "127.0.0.1:$PORT" --hz "$HZ" --smmp-hz "$SMMP_HZ")
if [[ "$VERBOSE" == 1 ]]; then
  RECEIVER_ARGS+=(--verbose)
  SENDER_ARGS+=(--verbose)
fi

start_process receiver "$BOTTLE" "$BOTTLE_WINE" "${RECEIVER_ARGS[@]}" || fail "no se pudo arrancar el receptor"

# Los mappings deben existir antes de que SimHub detecte el proceso: su lector
# rF2 abre los seis $rFactor2SMMP_* al inicializarse y falla si falta alguno.
if ! wait_for_log "$RUN_DIR/receiver.log" "Mappings iniciales creados" 20; then
  echo "ERROR: el receptor no publico los mappings iniciales." >&2
  tail -n 30 "$RUN_DIR/receiver.log" 2>/dev/null >&2 || true
  fail "revisa el log del receptor"
fi
echo "Mappings del receptor listos (LMU_Data, LMU y rFactor2 SMMP)."

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

start_raw_sender() {
  local pidfile="$RUN_DIR/raw.pid"
  if pid_alive "$pidfile"; then
    echo "emisor crudo ya esta en marcha (PID $(cat "$pidfile"))."
    return 0
  fi
  rm -f "$pidfile"
  if [[ ! -x "$SENDER_EXE" ]]; then
    echo "AVISO: falta $SENDER_EXE; se usara solo el emisor clasico." >&2
    return 0
  fi
  setsid env WINEPREFIX="$LMU_PFX" WINEDEBUG=-all "$LMU_WINE" "$SENDER_EXE" \
    --sender --udp "127.0.0.1:$PORT" --hz "$HZ" --smmp-hz "$SMMP_HZ" \
    >"$RUN_DIR/raw.log" 2>&1 < /dev/null &
  echo "$!" > "$pidfile"
  echo "emisor crudo arrancado (PID $!, log $RUN_DIR/raw.log)."
}

start_process sender "$LMU_PFX" "$LMU_WINE" "${SENDER_ARGS[@]}" || fail "no se pudo arrancar el emisor"
start_raw_sender

if real_lmu_running; then
  if wait_for_log "$RUN_DIR/receiver.log" "LMU_Data publicado" 15; then
    echo "Primeros datos de LMU recibidos."
  else
    echo "AVISO: aun no llegan datos de LMU; el espejo seguira intentandolo." >&2
  fi
fi

# Arranca el stub DESPUES de los mappings: SimHub lo detecta y su lector ya
# encuentra todos los buffers. El stub no abre el juego ni toca su memoria.
if ! start_stub; then
  echo "AVISO: SimHub no detectara LMU hasta que el stub arranque." >&2
fi

# Vigilante: si el wineserver de Bottles se reinicia (pasa al abrir CrewChief o
# SimHub con otro runner), Wine mata receptor, emisor y stub. Sin vigilante el
# puente queda muerto en silencio. Solo relanza procesos del puente.
WATCHDOG="$BRIDGE/lmu_bridge_watchdog.sh"
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
pgrep -af 'lmu_shm_mirror\.exe' || true
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
  - Arranque ordenado de todo: ./arrancar_todo_lmu.sh
  - SimHub: deberia detectar LMU y recibir datos sin reiniciarlo. Si no,
    reinícialo cuando no estes en pista.
  - CrewChief: si ya estaba abierto, ciérralo y vuelve a abrirlo para que tome
    el juego LMU (last_game_definition=LMU). El puente no reinicia apps.
  - El pit manager de CrewChief queda desactivado: el puente es de solo lectura.
  - El vigilante (bridge/lmu_bridge_watchdog.sh) relanza el puente si Wine lo
    mata; su log es watchdog.log. Se desactiva con --no-watch.
Para parar el puente: ./desactivar_puente_lmu.sh
EOF

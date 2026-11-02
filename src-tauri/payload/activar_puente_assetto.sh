#!/usr/bin/env bash
# Activa el espejo de telemetria Assetto Corsa -> prefijo Bottles (SimHub/CrewChief).
#
# El espejo es de solo lectura: Assetto Corsa -> UDP -> Bottles. No modifica el juego,
# no reenvia escrituras y no reinicia SimHub ni CrewChief.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRIDGE="$HERE/bridge"
EXE="$BRIDGE/assetto_shm_mirror.exe"
STUB_SRC="$BRIDGE/ac_process_stub.exe"
CONFIG_PY="$BRIDGE/configure_apps_assetto.py"
RUN_DIR="${ASSETTO_MIRROR_RUN_DIR:-/tmp/etol/assetto_mirror}"
STUB_EXE="$RUN_DIR/acs.exe"

ASSETTO_PFX="${ASSETTO_PFX:-}"
if [[ -z "$ASSETTO_PFX" ]]; then
  echo "ERROR: falta ASSETTO_PFX. Ejecuta: etol detect --save o etol config set games.assetto.prefix <ruta>" >&2
  exit 1
fi
ASSETTO_WINE="${ASSETTO_WINE:-}"
if [[ -z "$ASSETTO_WINE" ]]; then
  echo "ERROR: falta ASSETTO_WINE. Ejecuta: etol detect --save o etol config set games.assetto.wine <ruta>" >&2
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

PORT="${ASSETTO_MIRROR_PORT:-8775}"
HZ="${ASSETTO_MIRROR_HZ:-60}"
SIZES="${ASSETTO_MIRROR_SIZES:-4096,4096,4096,16384,32768}"
DO_CONFIG=1
DO_HF8=1
SOLO_CONFIG=0
DO_BUILD=0
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
  --no-build      no recompilar aunque falte el .exe
  --no-watch      no arrancar el vigilante que relanza el puente
  --no-stub       no arrancar el proceso inerte acs.exe
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
export ASSETTO_MIRROR_RUN_DIR="$RUN_DIR"
export ASSETTO_MIRROR_PORT="$PORT" ASSETTO_MIRROR_HZ="$HZ" ASSETTO_MIRROR_SIZES="$SIZES"
export ASSETTO_PFX ASSETTO_WINE BOTTLE_WINE
export SIMHUB_BOTTLE="$BOTTLE"
if [[ "$DO_STUB" != 1 ]]; then
  export ASSETTO_MIRROR_STUB=0
fi

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

# El stub es un .exe inerte llamado acs.exe que solo sirve para que
# CrewChief (y SimHub, si detecta por proceso) vean Assetto Corsa dentro de Bottles.
stub_cmd_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  local cmd norm
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"acs.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'acs[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

real_assetto_running() {
  local pid
  for pid in $(pgrep -f -- 'acs[.]exe' 2>/dev/null || true); do
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

[[ -x "$ASSETTO_WINE" ]] || fail "no encuentro el wine de Assetto Corsa: $ASSETTO_WINE"
[[ -d "$ASSETTO_PFX" ]] || fail "no encuentro el prefijo de Assetto Corsa: $ASSETTO_PFX"
[[ -x "$BOTTLE_WINE" ]] || fail "no encuentro el wine de Bottles: $BOTTLE_WINE"
[[ -d "$BOTTLE" ]] || fail "no encuentro la botella: $BOTTLE"
[[ -f "$BOTTLE/system.reg" ]] || fail "la botella no parece valida: $BOTTLE"

[[ -x "$EXE" ]] || fail "falta $EXE; ejecuta scripts/juegos/assetto/bridge/build_ac_mirror.sh"
if [[ "$DO_STUB" == 1 ]]; then
  [[ -x "$STUB_SRC" ]] || fail "falta $STUB_SRC; ejecuta scripts/juegos/assetto/bridge/build_ac_stub.sh"
fi

# CrewChief necesita su app Python (CrewChiefEx) dentro de Assetto Corsa para
# publicar acpmf_crewchief (rivales y estado de sesion). Se copia al juego real
# y se activa en python.ini (nombres de rivales); ademas se copia al espejo
# para que CrewChief no muestre el aviso "Plugin Needed".
preparar_plugin_crewchief() {
  local src="$BOTTLE/drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4/plugins/assettocorsa/apps/python/CrewChiefEx"
  if [[ ! -d "$src" ]]; then
    echo "AVISO: no encuentro el plugin CrewChiefEx en la botella; CrewChief pedira instalarlo." >&2
    return 0
  fi

  mkdir -p "$RUN_DIR/apps/python/CrewChiefEx"
  cp -a "$src/." "$RUN_DIR/apps/python/CrewChiefEx/" 2>/dev/null || true

  local lib="${ASSETTO_PFX%/steamapps/compatdata/*}"
  local game=""
  if [[ "$lib" != "$ASSETTO_PFX" ]]; then
    game="$lib/steamapps/common/assettocorsa"
  fi
  if [[ -z "$game" || ! -d "$game/apps/python" ]]; then
    echo "AVISO: no encuentro apps/python de Assetto Corsa; el plugin solo queda en el espejo." >&2
    return 0
  fi

  mkdir -p "$game/apps/python/CrewChiefEx"
  cp -a "$src/." "$game/apps/python/CrewChiefEx/" 2>/dev/null || true

  # Se activa por defecto para que CrewChief reciba rivales y estado de
  # sesion. Con ASSETTO_CREWCHIEF_PLUGIN=0 se puede desactivar si AC falla.
  local active="${ASSETTO_CREWCHIEF_PLUGIN:-1}"
  if [[ "$active" != "1" ]]; then active=0; fi
  local ini
  for ini in \
    "$game/cfg/python.ini" \
    "$ASSETTO_PFX/drive_c/users/steamuser/Documents/Assetto Corsa/cfg/python.ini"; do
    mkdir -p "$(dirname "$ini")" 2>/dev/null || true
    touch "$ini" 2>/dev/null || true
    if grep -qi '^\[CREWCHIEFEX\]' "$ini" 2>/dev/null; then
      sed -i '/^\[CREWCHIEFEX\]/,/^\[/ s/^ACTIVE=.*/ACTIVE='"$active"'/' "$ini"
    else
      printf '\n[CREWCHIEFEX]\nACTIVE=%s\n' "$active" >>"$ini"
    fi
  done
  if [[ "$active" == "1" ]]; then
    echo "Plugin CrewChiefEx instalado y activado en python.ini (nombres de rivales)."
  else
    echo "Plugin CrewChiefEx instalado; app Python desactivada (receptor cubre acpmf_crewchief)."
  fi
}

preparar_plugin_simhub() {
  local src="$BOTTLE/drive_c/Program Files (x86)/SimHub/_Addons/GamePlugins/assettocorsa/apps/python/SimHub"
  if [[ ! -d "$src" ]]; then
    echo "AVISO: no encuentro la app SimHub para AC; el relative no vera rivales." >&2
    return 0
  fi
  local lib="${ASSETTO_PFX%/steamapps/compatdata/*}"
  local game=""
  if [[ "$lib" != "$ASSETTO_PFX" ]]; then
    game="$lib/steamapps/common/assettocorsa"
  fi
  if [[ -z "$game" || ! -d "$game/apps/python" ]]; then
    echo "AVISO: no encuentro apps/python de Assetto Corsa; app SimHub omitida." >&2
    return 0
  fi
  mkdir -p "$game/apps/python/SimHub"
  cp -a "$src/." "$game/apps/python/SimHub/" 2>/dev/null || true
  local ini
  for ini in \
    "$game/cfg/python.ini" \
    "$ASSETTO_PFX/drive_c/users/steamuser/Documents/Assetto Corsa/cfg/python.ini"; do
    mkdir -p "$(dirname "$ini")" 2>/dev/null || true
    touch "$ini" 2>/dev/null || true
    if grep -qi '^\[SIMHUB\]' "$ini" 2>/dev/null; then
      sed -i '/^\[SIMHUB\]/,/^\[/ s/^ACTIVE=.*/ACTIVE=1/' "$ini"
    else
      printf '\n[SIMHUB]\nACTIVE=1\n' >>"$ini"
    fi
  done
  echo "App SimHub instalada y activada (rivales para RELATIVE_GENERAL)."
}

if [[ "$DO_CONFIG" == 1 ]]; then
  echo "Configurando SimHub y CrewChief para Assetto Corsa (con copia de seguridad)..."
  SIMHUB_BOTTLE="$BOTTLE" python3 "$CONFIG_PY" || \
    echo "AVISO: la configuracion automatica fallo; el puente seguira funcionando." >&2
  preparar_plugin_crewchief
  preparar_plugin_simhub
fi

if [[ "${SOLO_CONFIG:-0}" == 1 ]]; then
  echo "Configuracion aplicada para Assetto Corsa."
  exit 0
fi

if real_assetto_running; then
  echo "Assetto Corsa detectado en marcha."
else
  echo "AVISO: Assetto Corsa no parece estar en marcha; el emisor esperara a acpmf_*."
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
  [[ "$cmd" == *"assetto_shm_mirror.exe"* ]]
}

find_existing() {
  local mode="$1"
  pgrep -f -- "assetto_shm_mirror\.exe --${mode}" 2>/dev/null | head -n1 || true
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
      echo "       Cierra Assetto Corsa, abrelo con el Proton configurado en ETOL y vuelve a lanzar." >&2
    fi
    rm -f "$pidfile"
    return 1
  fi
  echo "$name arrancado (PID $pid, log $logfile)."
}

start_stub() {
  if [[ "$DO_STUB" != 1 ]]; then
    echo "stub de deteccion acs.exe omitido (--no-stub)."
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
echo "Mappings acpmf_* del receptor listos."

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

start_process sender "$ASSETTO_PFX" "$ASSETTO_WINE" "${SENDER_ARGS[@]}" || fail "no se pudo arrancar el emisor"

if real_assetto_running; then
  if wait_for_log "$RUN_DIR/receiver.log" "publicado" 15; then
    echo "Primeros datos de Assetto Corsa recibidos."
  else
    echo "AVISO: aun no llegan datos de Assetto Corsa; el espejo seguira intentandolo." >&2
  fi
fi

# El stub se arranca DESPUES de los mappings: si CrewChief/SimHub lo detectan,
# el lector ya encuentra acpmf_*. No abre el juego ni toca su memoria.
if ! start_stub; then
  echo "AVISO: CrewChief podria no detectar Assetto Corsa hasta que el stub arranque." >&2
fi

# Vigilante: si el wineserver de Bottles se reinicia, Wine mata receptor,
# emisor y stub. El vigilante solo relanza procesos del puente.
WATCHDOG="$BRIDGE/assetto_bridge_watchdog.sh"
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
pgrep -af 'assetto_shm_mirror\.exe' || true
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
  - Arranque ordenado de todo: ./arrancar_todo_assetto.sh
  - SimHub: si ya estaba en Assetto Corsa, deberia conectar sin reiniciar.
    Si no, selecciona Assetto Corsa cuando no estes en pista.
  - CrewChief: si ya estaba en Assetto Corsa, deberia conectar sin
    reiniciar. Si estaba en LMU/AMS2, cámbialo a Assetto Corsa cuando no
    estes en pista. La configuracion automatica solo se aplica con CrewChief
    cerrado.
  - El puente no reinicia apps ni escribe en el juego (solo lee acpmf_*).
  - El vigilante (bridge/assetto_bridge_watchdog.sh) relanza el puente si Wine lo
    mata; su log es watchdog.log. Se desactiva con --no-watch.
Para parar el puente: ./desactivar_puente_assetto.sh
EOF

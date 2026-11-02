#!/usr/bin/env bash
# Vigila el espejo LMU -> Bottles y lo levanta si desaparece.
#
# Motivo: los tres procesos del puente (receptor, emisor y stub) corren bajo
# Wine. Si el wineserver del prefijo de Bottles se reinicia (por ejemplo al
# abrir CrewChief o SimHub con otro runner), Wine mata todos sus procesos y el
# puente se queda "vivo" solo en apariencia: el emisor sigue enviando UDP pero
# nadie publica los mappings, y las apps dejan de ver LMU sin dar error.
#
# Este vigilante solo relanza procesos del puente. No toca LMU, SimHub,
# CrewChief ni sus archivos de configuracion.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXE="$HERE/lmu_shm_mirror.exe"
STUB_SRC="$HERE/lmu_process_stub.exe"

RUN_DIR="${LMU_MIRROR_RUN_DIR:-/tmp/etol/lmu_mirror}"
STUB_EXE="$RUN_DIR/Le Mans Ultimate.exe"
LOG="$RUN_DIR/watchdog.log"
INTERVAL="${LMU_MIRROR_WATCH_INTERVAL:-2}"

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

mkdir -p "$RUN_DIR"

log() {
  printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >>"$LOG"
}

stub_cmd_matches() {
  local pid="$1" cmd norm
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline")"
  [[ "$cmd" == *"Le Mans Ultimate.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'Le Mans Ultimate[.]exe' 2>/dev/null || true); do
    stub_cmd_matches "$pid" && printf '%s\n' "$pid"
  done
  return 0
}

mirror_pid() {
  local mode="$1" pid cmd
  for pid in $(pgrep -f -- "lmu_shm_mirror\\.exe --$mode" 2>/dev/null || true); do
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
    start_mirror receiver "$BOTTLE" "$BOTTLE_WINE" --receiver --bind "127.0.0.1:$PORT"
    sleep 1
  fi
  if [[ -z "$(mirror_pid sender)" ]]; then
    start_mirror sender "$LMU_PFX" "$LMU_WINE" \
      --sender --udp "127.0.0.1:$PORT" --hz "$HZ" --smmp-hz "$SMMP_HZ"
    sleep 1
  fi
  if [[ -z "$(find_stub_pids)" ]]; then
    start_stub
    sleep 1
  fi
  sleep "$INTERVAL"
done

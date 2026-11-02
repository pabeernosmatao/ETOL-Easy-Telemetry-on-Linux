#!/usr/bin/env bash
# Detiene el espejo LMU -> Bottles. No toca LMU, SimHub, CrewChief ni sus
# configuraciones. Solo termina los procesos lmu_shm_mirror.exe.
set -euo pipefail

RUN_DIR="${LMU_MIRROR_RUN_DIR:-/tmp/etol/lmu_mirror}"
STUB_EXE="$RUN_DIR/Le Mans Ultimate.exe"

# El stub es el unico "Le Mans Ultimate.exe" que vive bajo RUN_DIR. Nunca se
# toca el proceso real del juego, que esta en el prefijo de Steam.
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

stop_stub() {
  local pids
  pids="$(find_stub_pids)"
  if [[ -z "$pids" ]]; then
    rm -f "$RUN_DIR/stub.pid"
    echo "stub de deteccion no estaba en marcha."
    return 0
  fi

  # El stub no guarda estado; se puede terminar directamente. Se envian TERM a
  # todos sus procesos (lanzador Wine y .exe) y se espera antes de usar KILL.
  for pid in $pids; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  for _ in $(seq 1 30); do
    [[ -z "$(find_stub_pids)" ]] && break
    sleep 0.1
  done
  pids="$(find_stub_pids)"
  if [[ -n "$pids" ]]; then
    for pid in $pids; do
      kill -KILL "$pid" 2>/dev/null || true
    done
  fi
  rm -f "$RUN_DIR/stub.pid"

  if [[ -z "$(find_stub_pids)" ]]; then
    echo "stub de deteccion detenido."
  else
    echo "AVISO: aun queda algun proceso del stub; revisa pgrep -af 'Le Mans Ultimate'." >&2
  fi
}

is_our_process() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  tr '\0' ' ' < "/proc/$pid/cmdline" | grep -q 'lmu_shm_mirror\.exe'
}

stop_pidfile() {
  local name="$1"
  local pidfile="$RUN_DIR/$name.pid"
  [[ -f "$pidfile" ]] || return 0
  local pid
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  rm -f "$pidfile"
  [[ -n "$pid" ]] || return 0
  if ! kill -0 "$pid" 2>/dev/null; then
    return 0
  fi
  if ! is_our_process "$pid"; then
    echo "AVISO: el PID $pid de $name ya no es el espejo; no lo toco." >&2
    return 0
  fi
  kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  if kill -0 "$pid" 2>/dev/null; then
    kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
  fi
  echo "$name detenido."
}

stop_watchdog() {
  local pidfile="$RUN_DIR/watchdog.pid" pid
  [[ -f "$pidfile" ]] || { echo "vigilante no estaba en marcha."; return 0; }
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  rm -f "$pidfile"
  [[ -n "$pid" ]] || return 0
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "vigilante no estaba en marcha."
    return 0
  fi
  if ! tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'lmu_bridge_watchdog\.sh'; then
    echo "AVISO: el PID $pid del vigilante ya no es el vigilante; no lo toco." >&2
    return 0
  fi
  kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 30); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill -KILL "$pid" 2>/dev/null || true
  echo "vigilante detenido."
}

stop_watchdog
stop_stub
stop_pidfile receiver
stop_pidfile sender

# Limpieza de procesos huerfanos (por ejemplo si se perdio el PID file).
for mode in receiver sender; do
  pids="$(pgrep -f -- "lmu_shm_mirror\.exe --${mode}" 2>/dev/null || true)"
  for pid in $pids; do
    if is_our_process "$pid"; then
      kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
    fi
  done
done

sleep 0.5
if pgrep -f -- 'lmu_shm_mirror\.exe' >/dev/null 2>&1; then
  echo "AVISO: aun queda algun proceso lmu_shm_mirror.exe; revisa pgrep -af lmu_shm_mirror." >&2
else
  echo "Espejo, vigilante y stub de deteccion detenidos. LMU, SimHub y CrewChief no se han tocado."
fi

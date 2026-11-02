#!/usr/bin/env bash
# Para el relay UDP y el stub de deteccion de DiRT Rally 2.0.
# No cierra SimHub ni CrewChief.
set -euo pipefail

RUN_DIR="${DIRT2_UDP_RUN_DIR:-/tmp/etol/dirt2_udp}"
PIDFILE="$RUN_DIR/relay.pid"
STUB_EXE="$RUN_DIR/dirtrally2.exe"
LOG="$RUN_DIR/relay.log"

stopped=0
if [[ -f "$PIDFILE" ]]; then
  pid="$(cat "$PIDFILE" 2>/dev/null || true)"
  if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 40); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
    stopped=1
  fi
  rm -f "$PIDFILE"
fi

# Por si el PID se perdio: solo el relay de esta carpeta.
for pid in $(pgrep -f -- 'dirt2_udp_relay[.]py' 2>/dev/null || true); do
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"$RUN_DIR"* || "$cmd" == *"$(dirname "$0")"* ]] || continue
  kill "$pid" 2>/dev/null || true
  stopped=1
done

# Stub inerte dirtrally2.exe. Se comprueba la ruta para no tocar el juego real
# (que tambien se llama dirtrally2.exe pero corre desde el prefijo de Steam).
stub_cmd_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 1
  local cmd norm
  cmd="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "$cmd" == *"dirtrally2.exe"* ]] || return 1
  norm="${cmd//\\//}"
  [[ "$norm" == *"$STUB_EXE"* ]] || return 1
}

find_stub_pids() {
  local pid
  for pid in $(pgrep -f -- 'dirtrally2[.]exe' 2>/dev/null || true); do
    if stub_cmd_matches "$pid"; then
      printf '%s\n' "$pid"
    fi
  done
  return 0
}

stub_pids="$(find_stub_pids)"
if [[ -n "$stub_pids" ]]; then
  for pid in $stub_pids; do
    kill "$pid" 2>/dev/null || true
  done
  for _ in $(seq 1 30); do
    [[ -z "$(find_stub_pids)" ]] && break
    sleep 0.1
  done
  for pid in $(find_stub_pids); do
    kill -9 "$pid" 2>/dev/null || true
  done
  rm -f "$RUN_DIR/stub.pid"
  echo "Stub de deteccion detenido."
  stopped=1
else
  rm -f "$RUN_DIR/stub.pid"
  echo "El stub de deteccion ya estaba parado."
fi

if [[ "$stopped" == 1 ]]; then
  echo "Relay UDP detenido."
else
  echo "El relay UDP ya estaba parado."
fi
echo "Logs: $LOG"

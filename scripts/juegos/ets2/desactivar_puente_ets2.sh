#!/usr/bin/env bash
# Para el espejo de Euro Truck Simulator 2. No cierra SimHub.
# No toca el juego ni el plugin instalado en su carpeta.
set -euo pipefail

RUN_DIR="${ETS2_MIRROR_RUN_DIR:-/tmp/etol/ets2_bridge}"
PIDFILE="$RUN_DIR/mirror.pid"
LOG="$RUN_DIR/mirror.log"

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

# Por si el PID se perdio: solo el espejo real de esta carpeta, nunca una
# shell que mencione la ruta.
for pid in $(pgrep -f -- 'ets2_shm_mirror[.]exe' 2>/dev/null || true); do
  [[ -r "/proc/$pid/cmdline" ]] || continue
  base="$(tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | head -n1)"
  base="${base//\\//}"
  base="${base##*/}"
  if [[ "$base" == "ets2_shm_mirror.exe" ]]; then
    kill "$pid" 2>/dev/null || true
    stopped=1
    continue
  fi
  if [[ "$base" == "start.exe" ]]; then
    args="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    if [[ "${args//\\//}" == *"ets2_shm_mirror.exe"* ]]; then
      kill "$pid" 2>/dev/null || true
      stopped=1
    fi
  fi
done

# Stub inerte eurotrucks2.exe: solo el de esta carpeta, nunca el juego nativo
# (que se llama eurotrucks2, sin .exe y en otra ruta).
stub_pids=""
for pid in $(pgrep -f -- 'eurotrucks2[.]exe' 2>/dev/null || true); do
  [[ -r "/proc/$pid/cmdline" ]] || continue
  base="$(tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | head -n1)"
  base="${base//\\//}"
  base="${base##*/}"
  [[ "$base" == "eurotrucks2.exe" ]] || continue
  args="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ "${args//\\//}" == *"$RUN_DIR"* ]] || continue
  stub_pids+="$pid "
  kill "$pid" 2>/dev/null || true
done
if [[ -n "$stub_pids" ]]; then
  for _ in $(seq 1 30); do
    alive=0
    for pid in $stub_pids; do kill -0 "$pid" 2>/dev/null && alive=1; done
    [[ "$alive" == 0 ]] && break
    sleep 0.1
  done
  for pid in $stub_pids; do kill -9 "$pid" 2>/dev/null || true; done
  echo "Stub de deteccion eurotrucks2.exe detenido."
else
  echo "El stub de deteccion ya estaba parado."
fi
rm -f "$RUN_DIR/stub.pid"

if [[ "$stopped" == 1 ]]; then
  echo "Espejo de Euro Truck Simulator 2 detenido."
else
  echo "El espejo ya estaba parado."
fi
echo "Logs: $LOG"

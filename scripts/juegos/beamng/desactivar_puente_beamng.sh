#!/usr/bin/env bash
# Para el stub de deteccion de BeamNG.drive. No cierra SimHub ni toca el juego.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN_DIR="${BEAMNG_RUN_DIR:-/tmp/etol/beamng_udp}"

stub_pids=""
for pid in $(pgrep -f -- 'BeamNG[.]drive[.]x64[.]exe' 2>/dev/null || true); do
  [[ -r "/proc/$pid/cmdline" ]] || continue
  base="$(tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | head -n1)"
  base="${base//\\//}"
  base="${base##*/}"
  [[ "$base" == "BeamNG.drive.x64.exe" ]] || continue
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
  echo "Stub de deteccion de BeamNG.drive detenido."
else
  echo "El stub de deteccion ya estaba parado."
fi
rm -f "$RUN_DIR/stub.pid"

# BeamNG reescribe su settings.json mientras corre. Al cerrar el juego se
# reaplica la configuracion para que el proximo arranque ya la tenga.
if [[ -f "$HERE/bridge/configure_apps_beamng.py" ]] && \
   ! pgrep -f "BinLinux/BeamNG[.]drive[.]x64" >/dev/null 2>&1; then
  SIMHUB_BOTTLE="${SIMHUB_BOTTLE:-}" BEAMNG_RUN_DIR="$RUN_DIR" \
    python3 "$HERE/bridge/configure_apps_beamng.py" >/dev/null 2>&1 || true
  echo "Configuracion de BeamNG reaplicada para el proximo arranque."
fi

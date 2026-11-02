#!/usr/bin/env bash
# Mantiene el puente vivo: si LMU está en pausa/menú y el puente sale tras
# ~10 s sin frames, lo vuelve a intentar. Cuando vuelve la sesión, reconecta.
#
# Uso:
#   nohup ./bridge/run_bridge_loop.sh > /tmp/etol/lmu_shm_bridge.log 2>&1 &
#   ./bridge/stop_bridge.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

while true; do
  "$HERE/run_bridge.sh" "$@" || true
  sleep 2
done

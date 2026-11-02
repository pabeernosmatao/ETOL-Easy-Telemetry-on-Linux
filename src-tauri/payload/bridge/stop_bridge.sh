#!/usr/bin/env bash
# Detiene el puente LMU SHM y el bucle de reintento; no toca el juego ni el daemon.
set -euo pipefail
pkill -f 'run_bridge_loop\.sh' || true
if pkill -f 'lmu_shm_bridge\.exe'; then
  echo "Puente LMU SHM detenido."
else
  echo "El puente LMU SHM no estaba en marcha."
fi

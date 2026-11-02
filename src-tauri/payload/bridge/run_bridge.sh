#!/usr/bin/env bash
# Ejecuta el puente LMU SHM con el mismo Proton/Wine y prefijo que el juego.
#
# Uso normal (en primer plano):
#   ./bridge/run_bridge.sh
#
# En segundo plano:
#   nohup ./bridge/run_bridge.sh > /tmp/etol/lmu_shm_bridge.log 2>&1 &
#
# Snapshot unico para evidencia:
#   ./bridge/run_bridge.sh --once --no-udp --out /tmp/etol/shm_once.json
#
# Variables opcionales:
#   PROTON=...  PFX=...  UDP=127.0.0.1:8771  HZ=30
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PROTON="${PROTON:-}"
if [[ -z "$PROTON" ]]; then
  echo "ERROR: falta PROTON. Ejecuta: etol detect --save o etol config set games.lmu.proton <ruta>" >&2
  exit 1
fi
PFX="${PFX:-}"
if [[ -z "$PFX" ]]; then
  echo "ERROR: falta PFX. Ejecuta: etol detect --save o etol config set games.lmu.prefix <ruta>" >&2
  exit 1
fi
EXE="${EXE:-$HERE/lmu_shm_bridge.exe}"
UDP="${UDP:-127.0.0.1:8771}"
HZ="${HZ:-30}"
WINE="$PROTON/files/bin/wine"

if [[ ! -x "$WINE" ]]; then
  echo "ERROR: no encuentro el wine de Proton en: $WINE" >&2
  echo "Ajusta PROTON=/ruta/a/GE-Proton..." >&2
  exit 1
fi
if [[ ! -f "$PFX/system.reg" ]]; then
  echo "ERROR: no encuentro el prefijo de LMU en: $PFX" >&2
  echo "Ajusta PFX=/ruta/a/compatdata/2399420/pfx" >&2
  exit 1
fi
if [[ ! -f "$EXE" ]]; then
  echo "ERROR: no encuentro el puente: $EXE" >&2
  echo "Compilalo con: ./bridge/build_bridge.sh" >&2
  exit 1
fi

if [[ $# -gt 0 ]]; then
  ARGS=("$@")
else
  ARGS=(--udp "$UDP" --hz "$HZ" --verbose)
fi

exec env WINEPREFIX="$PFX" WINEDEBUG=-all "$WINE" "$EXE" "${ARGS[@]}"

#!/usr/bin/env bash
# Aplica solo la configuracion de un juego, sin arrancar el puente ni las apps.
#
# Lo usa el boton de engranaje de cada ficha. El script de activacion de cada
# juego acepta --solo-config: hace su bloque de configuracion (SimHub,
# CrewChief, plugins, enlaces) y sale antes de crear procesos.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
game="${1:-}"

case "$game" in
  lmu|ams2|acevo|assetto|rf2|dirt2|ets2) ;;
  *)
    echo "uso: $0 <lmu|ams2|acevo|assetto|rf2|dirt2|ets2>" >&2
    exit 2
    ;;
esac

script="$HERE/activar_puente_${game}.sh"
if [[ ! -f "$script" ]]; then
  echo "ERROR: no existe $script" >&2
  exit 1
fi

exec bash "$script" --solo-config

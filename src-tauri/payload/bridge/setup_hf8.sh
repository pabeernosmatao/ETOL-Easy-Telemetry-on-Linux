#!/usr/bin/env bash
# Configura el pad haptico Next Level Racing HF8 (ForceFeel) en Bottles.
#
# Solo toca el prefijo de Bottles:
#   - enlace dosdevices/comNN -> /dev/ttyACMn
#   - claves Enum\USB del pad, incluida DEVPKEY_NAME con tipo 0x12
# No abre el puerto, no reinicia SimHub y no modifica el juego.
#
# El puerto debe existir en SerialPort.GetPortNames() para que SimHub lo vea.
# COM33 es el ultimo puerto estandar de Wine y ya aparece siempre; por eso se
# usa por defecto en lugar de un COM40 que Wine no enumera.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BOTTLE="${SIMHUB_BOTTLE:-}"
if [[ -z "$BOTTLE" ]]; then
  echo "ERROR: falta SIMHUB_BOTTLE. Ejecuta: etol detect --save o etol config set apps.bottle <ruta>" >&2
  exit 1
fi
WINE="${BOTTLE_WINE:-}"
if [[ -z "$WINE" ]]; then
  echo "ERROR: falta BOTTLE_WINE. Ejecuta: etol detect --save o etol config set apps.wine <ruta>" >&2
  exit 1
fi
RUN_DIR="${LMU_MIRROR_RUN_DIR:-/tmp/etol/lmu_mirror}"

COM="${HF8_COM:-COM33}"
VID="${HF8_VID:-04d8}"
PID="${HF8_PID:-ee96}"
DEFAULT_SUFFIX="${HF8_INSTANCE_SUFFIX:-513&256&1&10}"

PROP_NAME='{b725f130-47ef-101a-a5f1-02608c9eebac}'

say() { echo "$*"; }
warn() { echo "AVISO: $*" >&2; }

[[ -d "$BOTTLE" ]] || { warn "no existe la botella $BOTTLE"; exit 0; }
[[ -x "$WINE" ]] || { warn "no existe wine de Bottles: $WINE"; exit 0; }

mkdir -p "$RUN_DIR"

# 1. Localizar el tty del HF8 por VID:PID.
TTY=""
shopt -s nullglob
for sys in /sys/class/tty/ttyACM*; do
  vendor="$(cat "$sys/device/../idVendor" 2>/dev/null || true)"
  product="$(cat "$sys/device/../idProduct" 2>/dev/null || true)"
  if [[ "${vendor,,}" == "$VID" && "${product,,}" == "$PID" ]]; then
    TTY="/dev/$(basename "$sys")"
    break
  fi
done
shopt -u nullglob

if [[ -z "$TTY" ]]; then
  warn "no encuentro el HF8 ($VID:$PID). Conectalo y vuelve a ejecutar."
  exit 0
fi
say "HF8 detectado en $TTY"

# 2. Enlace DOS. El puerto elegido debe ser uno de los que Wine enumera.
COM_LOWER="${COM,,}"

export WINEPREFIX="$BOTTLE"
export WINEDEBUG=-all

# 2a. Wine regenera dosdevices al inicializar el prefijo y puede
# asignar el COM elegido a otro tty. Se despierta el prefijo ANTES de
# enlazar y se deja el wineserver vivo para que el siguiente arranque
# no vuelva a pisar el enlace.
HF8_LINK_LATE=1
"$WINE" reg query 'HKLM\HARDWARE\DEVICEMAP\SERIALCOMM' >/dev/null 2>&1 || true
WINESERVER="$(dirname "$WINE")/wineserver"
if [[ -x "$WINESERVER" ]]; then
  "$WINESERVER" -p >/dev/null 2>&1 || true
fi

ln -sfn "$TTY" "$BOTTLE/dosdevices/$COM_LOWER"
say "Enlace $BOTTLE/dosdevices/$COM_LOWER -> $TTY"

# 2b. El COM elegido debe estar en SERIALCOMM: SerialPort.GetPortNames() solo
# devuelve esos puertos y SimHub ignora cualquier otro aunque el enlace exista.
if ! "$WINE" reg query 'HKLM\HARDWARE\DEVICEMAP\SERIALCOMM' 2>/dev/null \
  | tr -d '\r' | grep -qE "REG_SZ[[:space:]]+${COM}$"; then
  warn "$COM no aparece en SERIALCOMM; SimHub no lo vera. Prueba otro HF8_COM (p. ej. COM33)."
fi

# 3. Descubrir los sufijos de instancia que Wine ya creo para MI_00.
mapfile -t SUFFIXES < <(
  "$WINE" reg query 'HKLM\SYSTEM\CurrentControlSet\Enum\USB\VID_04D8&PID_EE96&MI_00' 2>/dev/null \
    | sed -n 's/.*MI_00\\//p' \
    | tr -d '\r' \
    | sort -u
)
if [[ "${#SUFFIXES[@]}" -eq 0 ]]; then
  SUFFIXES=("$DEFAULT_SUFFIX")
  warn "no encontre la instancia USB del pad; uso $DEFAULT_SUFFIX"
fi

# 4. Generar e importar el .reg (tipo 0x12 solo en MI_00).
REG_FILE="$RUN_DIR/hf8.reg"
REG_ARGS=(--com "$COM" --out "$REG_FILE")
for suffix in "${SUFFIXES[@]}"; do
  REG_ARGS+=(--suffix "$suffix")
done
python3 "$HERE/setup_hf8.py" "${REG_ARGS[@]}"

WIN_REG="Z:${REG_FILE//\//\\}"
"$WINE" regedit /S "$WIN_REG" >/dev/null 2>&1 || {
  warn "no se pudo importar $REG_FILE"
  exit 0
}
say "Registro del HF8 importado ($REG_FILE)."

# 5. Verificacion basica: nombre con el COM y tipo distinto de REG_SZ.
ok=1
for suffix in "${SUFFIXES[@]}"; do
  key="HKLM\\SYSTEM\\CurrentControlSet\\Enum\\USB\\VID_04D8&PID_EE96&MI_00\\$suffix"
  friendly="$("$WINE" reg query "$key" /v FriendlyName 2>/dev/null | tr -d '\r' || true)"
  if [[ "$friendly" != *"($COM)"* ]]; then
    warn "MI_00\\$suffix no muestra ($COM) en FriendlyName"
    ok=0
  fi
  prop="$("$WINE" reg query "$key\\Properties\\$PROP_NAME\\000A" 2>/dev/null | tr -d '\r' || true)"
  if [[ "$prop" == *"REG_SZ"* ]]; then
    warn "MI_00\\$suffix tiene DEVPKEY_NAME como REG_SZ (SimHub lo ignorara)"
    ok=0
  fi
done

ln -sfn "$TTY" "$BOTTLE/dosdevices/$COM_LOWER"
if [[ "$(readlink -f "$BOTTLE/dosdevices/$COM_LOWER")" != "$(readlink -f "$TTY")" ]]; then
  warn "el enlace $COM no apunta a $TTY"
  ok=0
fi

if [[ "$ok" == 1 ]]; then
  say "HF8 listo para SimHub: $COM -> $TTY (DEVPKEY_NAME tipo 0x12)."
else
  warn "la configuracion del HF8 quedo incompleta."
fi

#!/usr/bin/env bash
# El stub inerte de ACC es el mismo binario que el de Assetto Corsa: solo
# cambia el nombre con el que se copia en RUN_DIR (acc.exe y
# AC2-Win64-Shipping.exe). Se mantiene una copia con nombre propio para que el
# payload de ACC no dependa de los archivos de AC.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"

SRC="$ROOT/vendor/ac_process_stub.exe"
OUT="$ROOT/vendor/acc_process_stub.exe"

if [[ ! -f "$SRC" ]]; then
  echo "ERROR: falta $SRC" >&2
  exit 1
fi

cp -f "$SRC" "$OUT"
chmod +x "$OUT"
file "$OUT"
sha256sum "$OUT"

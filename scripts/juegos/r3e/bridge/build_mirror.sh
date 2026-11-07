#!/usr/bin/env bash
# Compila r3e_shm_mirror.exe (emisor/receptor crudo del mapping $R3E).
# El AppImage usa el binario ya compilado de vendor/; esto es para desarrollo.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
ZIG="${ZIG:-$ROOT/../SIMHUB_LINUX/tools/zig-x86_64-linux-0.15.2/zig}"
SRC="$HERE/r3e_shm_mirror.cpp"
OUT="$ROOT/vendor/r3e_shm_mirror.exe"

if [[ ! -x "$ZIG" ]]; then
  echo "ERROR: no encuentro zig (define ZIG=/ruta/zig)" >&2
  exit 1
fi
"$ZIG" c++ -target x86_64-windows-gnu -std=c++17 -O2 \
  -o "$OUT" "$SRC" -lws2_32
echo "compilado: $OUT"

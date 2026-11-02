#!/usr/bin/env bash
# Compila el proceso inerte "eurotrucks2.exe" para la deteccion de SimHub.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
ZIG="${ZIG:-$HOME/Escritorio/SIMHUB_LINUX/tools/zig/zig}"

if [[ ! -x "$ZIG" ]]; then
  echo "ERROR: no encuentro Zig en: $ZIG" >&2
  echo "Ajusta ZIG=/ruta/a/zig" >&2
  exit 1
fi

OUT="$ROOT/vendor/ets2_process_stub.exe"
mkdir -p "$ROOT/vendor"

"$ZIG" c++ -target x86_64-windows-gnu -std=c++17 -O2 \
  -o "$OUT" \
  "$HERE/ets2_process_stub.cpp"

file "$OUT"
sha256sum "$OUT"

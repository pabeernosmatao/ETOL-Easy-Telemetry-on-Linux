#!/usr/bin/env bash
# Compila el proceso inerte "rFactor2.exe" para la deteccion de CrewChief/SimHub.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
ZIG="${ZIG:-$ROOT/../SIMHUB_LINUX/tools/zig/zig}"

if [[ ! -x "$ZIG" ]]; then
  echo "ERROR: no encuentro Zig en: $ZIG" >&2
  echo "Ajusta ZIG=/ruta/a/zig" >&2
  exit 1
fi

OUT="$ROOT/vendor/rf2_process_stub.exe"

"$ZIG" c++ -target x86_64-windows-gnu -std=c++17 -O2 \
  -o "$OUT" \
  "$HERE/rf2_process_stub.cpp"

file "$OUT"
sha256sum "$OUT"

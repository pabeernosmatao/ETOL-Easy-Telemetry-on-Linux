#!/usr/bin/env bash
# Envoltorio de cargo que usa el toolchain local del proyecto.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export RUSTUP_HOME="$ROOT/.toolchain/rustup"
export CARGO_HOME="$ROOT/.toolchain/cargo"

# No filtrar la ruta del desarrollador en mensajes de panic ni en el binario.
if [[ " $* " == *" --release "* ]]; then
  export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$ROOT=/etol --remap-path-prefix=$CARGO_HOME=/toolchain"
fi

if [[ ! -x "$CARGO_HOME/bin/cargo" ]]; then
  echo "No hay toolchain. Ejecuta primero: $ROOT/scripts/instalar-toolchain.sh" >&2
  exit 1
fi

exec "$CARGO_HOME/bin/cargo" "$@"

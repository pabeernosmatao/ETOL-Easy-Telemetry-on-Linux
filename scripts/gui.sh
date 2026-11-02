#!/usr/bin/env bash
# Compila y ejecuta (o empaqueta) la interfaz grafica con el toolchain local.
#
#   ./scripts/gui.sh            compila en debug y abre la ventana
#   ./scripts/gui.sh dev        modo desarrollo de Tauri
#   ./scripts/gui.sh build      AppImage en target/release/bundle/appimage/
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export RUSTUP_HOME="$ROOT/.toolchain/rustup"
export CARGO_HOME="$ROOT/.toolchain/cargo"
export CARGO_TARGET_DIR="$ROOT/target"
export PATH="$CARGO_HOME/bin:$ROOT/node_modules/.bin:$PATH"

# No filtrar la ruta del desarrollador en mensajes de panic ni en el binario.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$ROOT=/etol --remap-path-prefix=$CARGO_HOME=/toolchain"

if [[ ! -x "$CARGO_HOME/bin/cargo" ]]; then
  echo "No hay toolchain. Ejecuta primero: $ROOT/scripts/instalar-toolchain.sh" >&2
  exit 1
fi

tauri_cli() {
  if [[ -x "$ROOT/node_modules/.bin/tauri" ]]; then
    "$ROOT/node_modules/.bin/tauri" "$@"
  else
    npx --no-install tauri "$@"
  fi
}

case "${1:-run}" in
  run)
    shift
    exec "$CARGO_HOME/bin/cargo" run --manifest-path "$ROOT/src-tauri/Cargo.toml" "$@"
    ;;
  dev)
    shift
    tauri_cli dev "$@"
    ;;
  build)
    shift
    "$ROOT/scripts/preparar-payload.sh"
    mkdir -p "$ROOT/src-tauri/binaries"
    "$CARGO_HOME/bin/cargo" build --release --manifest-path "$ROOT/Cargo.toml"
    install -m 755 \
      "$ROOT/target/release/etol" \
      "$ROOT/src-tauri/binaries/etol-x86_64-unknown-linux-gnu"
    tauri_cli build "$@"
    ;;
  *)
    echo "uso: gui.sh [run|dev|build] [argumentos]" >&2
    exit 2
    ;;
esac

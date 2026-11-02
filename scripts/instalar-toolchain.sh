#!/usr/bin/env bash
# Instala un toolchain de Rust aislado dentro del propio proyecto.
#
# No usa sudo ni toca ~/.cargo ni ~/.rustup: todo queda en .toolchain/ y se
# puede borrar con `rm -rf .toolchain` sin dejar rastro.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export RUSTUP_HOME="$ROOT/.toolchain/rustup"
export CARGO_HOME="$ROOT/.toolchain/cargo"

if [[ -x "$CARGO_HOME/bin/rustc" && -x "$CARGO_HOME/bin/cargo" ]]; then
  echo "Toolchain ya instalado:"
  "$CARGO_HOME/bin/rustc" --version
  "$CARGO_HOME/bin/cargo" --version
  exit 0
fi

echo "Instalando Rust (perfil minimo) en $ROOT/.toolchain ..."
mkdir -p "$ROOT/.toolchain"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$TMP/rustup-init.sh"
sh "$TMP/rustup-init.sh" -y --profile minimal --default-toolchain stable --no-modify-path

echo
echo "Listo:"
"$CARGO_HOME/bin/rustc" --version
"$CARGO_HOME/bin/cargo" --version
echo
echo "Compila con: ./scripts/cargo.sh build --release"

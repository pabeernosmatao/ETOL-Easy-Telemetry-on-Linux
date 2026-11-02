#!/usr/bin/env bash
# Prepara los dos AppImage con el nombre final de la release en dist/.
#
#   ETOL-<version>-linux-x86_64-portable.AppImage   WebKit del sistema (~6 MB)
#   ETOL-<version>-linux-x86_64-full.AppImage       todo incluido (~111 MB)
#
# Uso: ./scripts/preparar-release.sh   (despues de ./scripts/gui.sh build)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUNDLE="$ROOT/target/release/bundle/appimage"
DIST="$ROOT/dist"
VERSION="$(python3 - "$ROOT/src-tauri/tauri.conf.json" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["version"])
PY
)"

FULL_SRC="$BUNDLE/ETOL_${VERSION}_amd64.AppImage"
PORTABLE_SRC="$BUNDLE/ETOL-${VERSION}-linux-x86_64-portable.AppImage"

if [[ ! -f "$FULL_SRC" ]]; then
  echo "ERROR: falta $FULL_SRC; ejecuta ./scripts/gui.sh build" >&2
  exit 1
fi
if [[ ! -f "$PORTABLE_SRC" ]]; then
  echo "aviso: falta la variante portable; la genero..."
  "$ROOT/scripts/empaquetar-portable.sh" >/dev/null
fi

mkdir -p "$DIST"
FULL_OUT="$DIST/ETOL-${VERSION}-linux-x86_64-full.AppImage"
PORT_OUT="$DIST/ETOL-${VERSION}-linux-x86_64-portable.AppImage"
install -m 755 "$FULL_SRC" "$FULL_OUT"
install -m 755 "$PORTABLE_SRC" "$PORT_OUT"

echo "release ${VERSION}:"
ls -lh "$FULL_OUT" "$PORT_OUT" | sed 's/^/  /'
echo
sha256sum "$FULL_OUT" "$PORT_OUT"

#!/usr/bin/env bash
# Crea una variante "portable" del AppImage que usa el WebKit/GTK del sistema.
#
# El AppImage que genera Tauri empaqueta WebKit y GTK de la maquina de
# compilacion. Si esa maquina es mas nueva que el destino (por ejemplo
# CachyOS con glibc 2.44 frente a Ubuntu 26.04 con 2.43), el binario incluido
# exige una glibc que el destino no tiene y el AppImage no arranca.
#
# Esta variante quita las librerias empaquetadas: el ejecutable sigue pidiendo
# glibc 2.39 y usa el WebKit del sistema (libwebkit2gtk-4.1 + libgtk-3), que
# es lo que traen Ubuntu 26.04, Fedora, Arch y CachyOS.
#
# Uso:
#   ./scripts/empaquetar-portable.sh            usa el AppImage mas reciente
#   ./scripts/empaquetar-portable.sh <entrada>  usa un AppImage concreto
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUNDLE="$ROOT/target/release/bundle/appimage"
VERSION="$(python3 - "$ROOT/src-tauri/tauri.conf.json" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["version"])
PY
)"
OUT="$BUNDLE/ETOL-${VERSION}-linux-x86_64-portable.AppImage"

SRC="${1:-}"
if [[ -z "$SRC" ]]; then
  SRC="$(ls -t "$BUNDLE"/ETOL_*_amd64.AppImage 2>/dev/null | head -n1 || true)"
fi
if [[ -z "$SRC" || ! -f "$SRC" ]]; then
  echo "ERROR: no encuentro el AppImage de entrada. Compila antes con:" >&2
  echo "       ./scripts/gui.sh build" >&2
  exit 1
fi

APPDIR="$BUNDLE/ETOL.AppDir"
if [[ ! -d "$APPDIR" ]]; then
  echo "ERROR: falta $APPDIR (lo genera tauri build)" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# 1. Runtime: es la cabecera ELF del AppImage hasta el superbloque squashfs.
python3 - "$SRC" "$TMP/runtime" <<'PY'
import struct, sys
src, dest = sys.argv[1], sys.argv[2]
data = open(src, "rb").read(4_000_000)
pos = 0
while True:
    off = data.find(b"hsqs", pos)
    if off < 0:
        sys.exit("ERROR: no encuentro el squashfs dentro del AppImage")
    block_size = struct.unpack_from("<I", data, off + 12)[0]
    comp = struct.unpack_from("<H", data, off + 20)[0]
    if block_size == 131072 and 1 <= comp <= 6:
        with open(dest, "wb") as out:
            out.write(data[:off])
        print(f"runtime: {off} bytes (compresion {comp})")
        break
    pos = off + 1
PY

# 2. mksquashfs: viene dentro del plugin de linuxdeploy que usa Tauri.
PLUGIN="$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage"
if [[ ! -x "$PLUGIN" ]]; then
  echo "ERROR: falta $PLUGIN (lo descarga tauri build)" >&2
  exit 1
fi
( cd "$TMP" && "$PLUGIN" --appimage-extract >/dev/null 2>&1 )
MKSQUASHFS="$TMP/squashfs-root/appimagetool-prefix/usr/bin/mksquashfs"
if [[ ! -x "$MKSQUASHFS" ]]; then
  echo "ERROR: no encuentro mksquashfs en el plugin de linuxdeploy" >&2
  exit 1
fi

# 3. Copia del AppDir sin las librerias empaquetadas.
LITE="$TMP/AppDir"
cp -a "$APPDIR" "$LITE"

rm -f "$LITE"/usr/lib/*.so*
rm -rf "$LITE"/usr/lib/webkit2gtk-4.1 \
       "$LITE"/usr/lib/girepository-1.0 \
       "$LITE"/usr/lib/gio \
       "$LITE"/usr/lib/gtk-3.0 \
       "$LITE"/usr/lib/gdk-pixbuf-2.0 \
       "$LITE"/usr/share/glib-2.0
# AppRun hace `source` del hook con `set -e`: se deja uno minimo.
mkdir -p "$LITE/apprun-hooks"
cat > "$LITE/apprun-hooks/linuxdeploy-plugin-gtk.sh" <<'HOOK'
#! /usr/bin/env bash
# Variante portable: GTK y WebKit del sistema.
export APPDIR="${APPDIR:-"$(dirname "$(realpath "$0")")"}"
HOOK
chmod +x "$LITE/apprun-hooks/linuxdeploy-plugin-gtk.sh"

# 4. Empaquetar y unir runtime + squashfs.
"$MKSQUASHFS" "$LITE" "$TMP/app.squashfs" -root-owned -noappend -comp zstd -no-progress >/dev/null
cat "$TMP/runtime" "$TMP/app.squashfs" > "$OUT"
chmod +x "$OUT"

echo "variante portable: $OUT"
ls -lh "$OUT"
echo "requiere en el sistema: libwebkit2gtk-4.1 y libgtk-3"

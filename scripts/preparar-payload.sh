#!/usr/bin/env bash
# Prepara el payload que se empaqueta dentro del AppImage.
#
# Copia los scripts y el puente de SIMHUB_LINUX a src-tauri/payload, sin los
# artefactos de desarrollo (.pdb, .cpp, __pycache__). El AppImage lo instala en
# ~/.config/etol/payload en el primer arranque.
#
# Origen: ETOL_APP_DIR o las rutas habituales.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/src-tauri/payload"

find_src() {
  if [[ -n "${ETOL_APP_DIR:-}" && -f "$ETOL_APP_DIR/arrancar_todo_lmu.sh" ]]; then
    printf '%s\n' "$ETOL_APP_DIR"
    return 0
  fi
  for candidate in \
    "$HOME/Escritorio/SIMHUB_LINUX/Simhub_Linux_APP" \
    "$HOME/SIMHUB_LINUX/Simhub_Linux_APP" \
    "$HOME/Simhub_Linux_APP"; do
    if [[ -f "$candidate/arrancar_todo_lmu.sh" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

if ! SRC="$(find_src)"; then
  echo "aviso: no encuentro Simhub_Linux_APP; el AppImage se compilara sin scripts incluidos" >&2
  mkdir -p "$DEST"
  exit 0
fi

echo "payload: $SRC -> $DEST"
rm -rf "$DEST"
mkdir -p "$DEST"

tar -C "$SRC" \
  --exclude='*.pdb' \
  --exclude='*.cpp' \
  --exclude='__pycache__' \
  --exclude='*.pyc' \
  --exclude='build_*.sh' \
  -cf - . | tar -C "$DEST" -xf -

# Puente LMU con los tamanos SMMP que espera CrewChief. Se sobrescribe solo en
# la copia: el arbol original de SIMHUB_LINUX no se toca.
if [[ -f "$ROOT/vendor/lmu_shm_mirror.exe" ]]; then
  install -m 755 "$ROOT/vendor/lmu_shm_mirror.exe" "$DEST/bridge/lmu_shm_mirror.exe"
  echo "puente LMU: binario con tamanos SMMP de CrewChief"
fi

# Assetto Corsa: scripts propios de ETOL (el arbol de SIMHUB_LINUX no se toca).
if [[ -d "$ROOT/scripts/juegos/assetto" ]]; then
  cp -a "$ROOT/scripts/juegos/assetto/." "$DEST/"
  rm -f "$DEST/bridge/ac_shm_mirror.cpp" \
        "$DEST/bridge/ac_process_stub.cpp" \
        "$DEST/bridge/build_ac_mirror.sh" \
        "$DEST/bridge/build_ac_stub.sh"
  echo "payload: scripts de Assetto Corsa"
fi
if [[ -f "$ROOT/vendor/assetto_shm_mirror.exe" ]]; then
  install -m 755 "$ROOT/vendor/assetto_shm_mirror.exe" "$DEST/bridge/assetto_shm_mirror.exe"
fi
if [[ -f "$ROOT/vendor/ac_process_stub.exe" ]]; then
  install -m 755 "$ROOT/vendor/ac_process_stub.exe" "$DEST/bridge/ac_process_stub.exe"
fi

# rFactor 2: scripts propios y plugin SMMP (el arbol de SIMHUB_LINUX no se toca).
if [[ -d "$ROOT/scripts/juegos/rf2" ]]; then
  cp -a "$ROOT/scripts/juegos/rf2/." "$DEST/"
  rm -f "$DEST/bridge/rf2_shm_mirror.cpp" \
        "$DEST/bridge/rf2_process_stub.cpp" \
        "$DEST/bridge/build_rf2_mirror.sh" \
        "$DEST/bridge/build_rf2_stub.sh"
  echo "payload: scripts de rFactor 2"
fi
if [[ -f "$ROOT/vendor/rf2_shm_mirror.exe" ]]; then
  install -m 755 "$ROOT/vendor/rf2_shm_mirror.exe" "$DEST/bridge/rf2_shm_mirror.exe"
fi
if [[ -f "$ROOT/vendor/rf2_process_stub.exe" ]]; then
  install -m 755 "$ROOT/vendor/rf2_process_stub.exe" "$DEST/bridge/rf2_process_stub.exe"
fi
# DiRT Rally 2.0: relay UDP y stub de deteccion propios.
if [[ -d "$ROOT/scripts/juegos/dirt2" ]]; then
  cp -a "$ROOT/scripts/juegos/dirt2/." "$DEST/"
  rm -f "$DEST/bridge/dirt2_process_stub.cpp" \
        "$DEST/bridge/build_dirt2_stub.sh"
  echo "payload: scripts de DiRT Rally 2.0"
fi
if [[ -f "$ROOT/vendor/dirt2_process_stub.exe" ]]; then
  install -m 755 "$ROOT/vendor/dirt2_process_stub.exe" "$DEST/bridge/dirt2_process_stub.exe"
fi

# Euro Truck Simulator 2 (nativo Linux): plugin SCS y espejo /dev/shm.
if [[ -d "$ROOT/scripts/juegos/ets2" ]]; then
  cp -a "$ROOT/scripts/juegos/ets2/." "$DEST/"
  rm -f "$DEST/bridge/ets2_shm_mirror.cpp" \
        "$DEST/bridge/ets2_process_stub.cpp" \
        "$DEST/bridge/build_ets2_mirror.sh" \
        "$DEST/bridge/build_ets2_stub.sh"
  echo "payload: scripts de Euro Truck Simulator 2"
fi
if [[ -f "$ROOT/vendor/ets2_shm_mirror.exe" ]]; then
  install -m 755 "$ROOT/vendor/ets2_shm_mirror.exe" "$DEST/bridge/ets2_shm_mirror.exe"
fi
if [[ -f "$ROOT/vendor/ets2_process_stub.exe" ]]; then
  install -m 755 "$ROOT/vendor/ets2_process_stub.exe" "$DEST/bridge/ets2_process_stub.exe"
fi
if [[ -f "$ROOT/vendor/libscs-telemetry.so" ]]; then
  install -m 644 "$ROOT/vendor/libscs-telemetry.so" "$DEST/bridge/libscs-telemetry.so"
fi
if [[ -f "$ROOT/vendor/libscs-telemetry.LICENSE.txt" ]]; then
  install -m 644 "$ROOT/vendor/libscs-telemetry.LICENSE.txt" \
    "$DEST/bridge/libscs-telemetry.LICENSE.txt"
fi

# BeamNG.drive (nativo Linux): solo stub de deteccion, la telemetria va por UDP.
if [[ -d "$ROOT/scripts/juegos/beamng" ]]; then
  cp -a "$ROOT/scripts/juegos/beamng/." "$DEST/"
  rm -f "$DEST/bridge/beamng_process_stub.cpp" \
        "$DEST/bridge/build_beamng_stub.sh"
  echo "payload: scripts de BeamNG.drive"
fi
if [[ -f "$ROOT/vendor/beamng_process_stub.exe" ]]; then
  install -m 755 "$ROOT/vendor/beamng_process_stub.exe" \
    "$DEST/bridge/beamng_process_stub.exe"
fi

if [[ -f "$ROOT/vendor/rFactor2SharedMemoryMapPlugin64.dll" ]]; then
  install -m 755 "$ROOT/vendor/rFactor2SharedMemoryMapPlugin64.dll" \
    "$DEST/bridge/rFactor2SharedMemoryMapPlugin64.dll"
fi
if [[ -f "$ROOT/vendor/rFactor2SharedMemoryMapPlugin64.LICENSE.txt" ]]; then
  install -m 644 "$ROOT/vendor/rFactor2SharedMemoryMapPlugin64.LICENSE.txt" \
    "$DEST/bridge/rFactor2SharedMemoryMapPlugin64.LICENSE.txt"
fi

# Scripts propios de ETOL (instalacion de Bottles Flatpak y del prefijo).
if [[ -d "$ROOT/scripts/sistema" ]]; then
  mkdir -p "$DEST/sistema"
  cp -a "$ROOT/scripts/sistema/." "$DEST/sistema/"
fi

# Licencias: el AppImage distribuido debe llevar los avisos.
if [[ -f "$ROOT/LICENSE" ]]; then
  install -m 644 "$ROOT/LICENSE" "$DEST/LICENSE"
fi
# Guía de uso en los tres idiomas de la interfaz.
for lang in es ca en; do
  if [[ -f "$ROOT/docs/guia-$lang.html" ]]; then
    install -m 644 "$ROOT/docs/guia-$lang.html" "$DEST/guia-$lang.html"
  fi
done
if [[ -f "$ROOT/THIRD-PARTY.md" ]]; then
  install -m 644 "$ROOT/THIRD-PARTY.md" "$DEST/THIRD-PARTY.md"
fi

# Quita rutas y nombres del entorno de desarrollo SOLO de la copia.
python3 "$ROOT/scripts/parchear-payload.py" "$DEST"

# Los scripts y binarios deben poder ejecutarse desde ~/.config.
find "$DEST" -type f \( -name '*.sh' -o -name '*.exe' -o -name '*.py' \) -exec chmod 755 {} +
find "$DEST" -type d -exec chmod 755 {} +

# Identificador de contenido: si cambia, el AppImage refresca la copia.
ID="$(
  cd "$DEST"
  find . -type f ! -name '.payload-id' -print0 \
    | sort -z \
    | xargs -0 sha256sum \
    | sha256sum \
    | awk '{print $1}'
)"
printf '%s\n' "$ID" > "$DEST/.payload-id"

echo "payload listo: $(du -sh "$DEST" | cut -f1), id ${ID:0:12}"

#!/usr/bin/env bash
# Recoge todo lo necesario para diagnosticar ETOL (Easy Telemetry On Linux).
#
# Uso, desde la carpeta donde esta el AppImage:
#
#   ./scripts/recoger-logs.sh
#   ./scripts/recoger-logs.sh --gui
#   ./scripts/recoger-logs.sh --juego lmu
#   ./scripts/recoger-logs.sh /ruta/ETOL-0.3.3-linux-x86_64-portable.AppImage
#
#   --gui            abre la aplicacion para que reproduzcas el fallo
#   --juego ID       arranca puente + CrewChief + SimHub de ID
#                    (lmu, ams2, acevo, assetto, rf2, dirt2, ets2, beamng), espera y recoge los logs
#   --espera N       segundos de espera con --juego (defecto: 60)
#   --help           muestra esta ayuda
#
# No necesita sudo, ni Rust, ni la carpeta del proyecto.

set -euo pipefail

GUI=0
JUEGO=""
ESPERA=60
APP=""

uso() {
  cat <<'EOF'
Recoge todo lo necesario para diagnosticar ETOL.

Uso, desde la carpeta donde esta el AppImage:

  ./scripts/recoger-logs.sh
  ./scripts/recoger-logs.sh --gui
  ./scripts/recoger-logs.sh --juego lmu
  ./scripts/recoger-logs.sh /ruta/ETOL-0.3.3-linux-x86_64-portable.AppImage

  --gui            abre la aplicacion para que reproduzcas el fallo
  --juego ID       arranca puente + CrewChief + SimHub de ID
                   (lmu, ams2, acevo, assetto, rf2, dirt2, ets2, beamng), espera y recoge los logs
  --espera N       segundos de espera con --juego (defecto: 60)
  --help           muestra esta ayuda

No necesita sudo, ni Rust, ni la carpeta del proyecto.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --gui) GUI=1; shift ;;
    --juego) JUEGO="${2:-}"; [[ -n "$JUEGO" ]] || { echo "Falta el ID del juego" >&2; exit 2; }; shift 2 ;;
    --espera) ESPERA="${2:-}"; [[ "$ESPERA" =~ ^[0-9]+$ ]] || { echo "--espera necesita un numero" >&2; exit 2; }; shift 2 ;;
    -h|--help) uso; exit 0 ;;
    -*) echo "Opcion desconocida: $1" >&2; uso; exit 2 ;;
    *) APP="$1"; shift ;;
  esac
done

if [[ -n "$JUEGO" ]]; then
  case "$JUEGO" in
    lmu|ams2|acevo) ;;
    *) echo "ERROR: --juego debe ser lmu, ams2 o acevo" >&2; exit 2 ;;
  esac
  GUI=0
fi

# --- localizar el AppImage ---------------------------------------------------

if [[ -z "$APP" ]]; then
  mapfile -t candidatos < <(
    find "$PWD" -maxdepth 1 -type f -iname 'ETOL*.AppImage' -printf '%T@ %p\n' 2>/dev/null \
      | sort -nr | cut -d' ' -f2-
  )
  if [[ ${#candidatos[@]} -eq 0 ]]; then
    echo "ERROR: no encuentro ningun ETOL*.AppImage en $PWD" >&2
    echo "Pasa la ruta como argumento: $0 /ruta/ETOL-0.3.3-linux-x86_64-portable.AppImage" >&2
    exit 1
  fi
  APP="${candidatos[0]}"
  if [[ ${#candidatos[@]} -gt 1 ]]; then
    echo "Aviso: hay varios AppImage; uso el mas reciente:"
    printf '  %s\n' "${candidatos[@]}"
  fi
fi

if [[ ! -f "$APP" ]]; then
  echo "ERROR: no existe $APP" >&2
  exit 1
fi

APP="$(readlink -f "$APP")"
chmod +x "$APP" 2>/dev/null || true

# --- carpetas de trabajo -----------------------------------------------------

STAMP="$(date '+%Y%m%d-%H%M%S')"
HOST="$(hostname 2>/dev/null || echo equipo)"
OUT="$PWD/etol-logs-$HOST-$STAMP"
TAR="$OUT.tar.gz"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/etol-extract.XXXXXX")"

CFG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/etol"
CFG="$CFG_DIR/config/config.toml"

limpiar() { rm -rf "$TMP"; }
trap limpiar EXIT

mkdir -p "$OUT"

echo "AppImage: $APP"
echo "Salida:   $TAR"
echo

# --- 1. extraer el CLI y preparar la configuracion ---------------------------

echo "[1/6] Extrayendo el CLI..."
(
  cd "$TMP"
  "$APP" --appimage-extract >/dev/null 2>&1
) || true
CLI="$TMP/squashfs-root/usr/bin/etol"

# En un equipo recien estrenado aun no hay configuracion. Se crea igual que
# en el primer arranque de la GUI para que el doctor no de un falso fallo.
if [[ -x "$CLI" && ! -f "$CFG" ]]; then
  echo "  No hay configuracion guardada; creandola con 'detect --save'..."
  "$CLI" detect --save > "$OUT/primer-arranque.txt" 2>&1 || true
fi

# --- 2. arrancar la pila o abrir la GUI -------------------------------------

if [[ -n "$JUEGO" ]]; then
  echo
  echo "[2/6] Arrancando puente + CrewChief + SimHub de $JUEGO..."
  if [[ -x "$CLI" ]]; then
    # Es el mismo script que ejecuta el boton "Arrancar todo" de la GUI:
    # arrancar_todo_<juego>.sh (puente, luego CrewChief y luego SimHub).
    "$CLI" launch "$JUEGO" > "$OUT/arranque-$JUEGO.txt" 2>&1 || true
    echo "  Esperando ${ESPERA}s a que arranque todo..."
    for ((i = ESPERA; i > 0; i--)); do
      printf '\r  %3ds ' "$i"
      sleep 1
    done
    printf '\r  listo.   \n'
  else
    echo "  AVISO: no se pudo extraer el CLI; no se arranca nada." | tee "$OUT/arranque-$JUEGO.txt"
  fi
elif [[ "$GUI" -eq 1 ]]; then
  echo
  echo "[2/6] Abriendo la aplicacion."
  echo "      Reproduce el fallo: pulsa 'Redetectar' y luego 'Arrancar todo'."
  echo "      'Arrancar todo' lanza puente + CrewChief + SimHub."
  echo "      Cuando termines, elige 'Salir' en la bandeja o cierra la"
  echo "      ventana, y pulsa Enter aqui."
  echo
  if [[ -e /dev/fuse ]] && { command -v fusermount3 >/dev/null 2>&1 || command -v fusermount >/dev/null 2>&1; }; then
    "$APP" </dev/null >"$OUT/gui.txt" 2>&1 &
  else
    "$APP" --appimage-extract-and-run </dev/null >"$OUT/gui.txt" 2>&1 &
  fi
  GUI_PID=$!
  if [[ -t 0 ]]; then
    read -r -p "Pulsa Enter cuando hayas terminado... " _ || true
  else
    echo "(sin terminal interactiva; continuo sin esperar)"
  fi
  sleep 1
  if kill -0 "$GUI_PID" 2>/dev/null; then
    echo "La app sigue abierta; los logs ya estan guardados."
    echo "Puedes cerrarla con 'Salir' en la bandeja."
  fi
else
  echo "[2/6] Sin arrancar nada (usa --gui o --juego lmu/ams2/acevo)."
fi

# --- 3. doctor ---------------------------------------------------------------

echo "[3/6] Ejecutando el doctor..."
if [[ -x "$CLI" ]]; then
  "$CLI" --version > "$OUT/version.txt" 2>&1 || true
  "$CLI" doctor > "$OUT/doctor.txt" 2>&1 || true
  echo "  doctor.txt listo"
else
  echo "AVISO: no se pudo extraer usr/bin/etol" > "$OUT/doctor.txt"
  echo "  AVISO: no se pudo ejecutar el doctor" >&2
fi

# --- 4. configuracion, entorno y logs ---------------------------------------

echo "[4/6] Copiando configuracion y logs..."
if [[ -f "$CFG" ]]; then
  cp "$CFG" "$OUT/config.toml"
  cp "$CFG_DIR/generated/entorno.sh" "$OUT/entorno.sh" 2>/dev/null || true
  cp "$CFG_DIR/payload/.payload-id" "$OUT/payload-id.txt" 2>/dev/null || true
  cp -a "$CFG_DIR/logs" "$OUT/app-logs" 2>/dev/null || true
  RUN_BASE="$(sed -n 's/^[[:space:]]*run_base[[:space:]]*=[[:space:]]*"\(.*\)".*/\1/p' "$CFG" | head -n1)"
else
  echo "AVISO: no existe $CFG" > "$OUT/config-aviso.txt"
  RUN_BASE=""
fi

RUN_BASE="${RUN_BASE:-/tmp/etol}"
echo "  run_base: $RUN_BASE"

if [[ -d "$RUN_BASE" ]]; then
  cp -a "$RUN_BASE" "$OUT/run"
  find "$RUN_BASE" -maxdepth 2 -type f -name '*.log' -printf '%TY-%Tm-%Td %TH:%TM  %p\n' \
    2>/dev/null | sort > "$OUT/indice-logs.txt" || true
else
  echo "AVISO: no existe $RUN_BASE (no se ha arrancado el puente?)" > "$OUT/run-aviso.txt"
fi

# Logs de SimHub/CrewChief dentro de la botella (por si se arrancaron a mano).
if [[ -f "$OUT/config.toml" ]]; then
  BOTTLE="$(sed -n 's/^[[:space:]]*bottle[[:space:]]*=[[:space:]]*"\(.*\)".*/\1/p' "$OUT/config.toml" | head -n1)"
  if [[ -n "$BOTTLE" && -d "$BOTTLE/drive_c" ]]; then
    mkdir -p "$OUT/windows-logs"
    (
      cd "$BOTTLE/drive_c"
      find . -maxdepth 8 -type f -iname '*.log' -mtime -2 -size -10M -print0 2>/dev/null \
        | xargs -0 -r cp --parents -t "$OUT/windows-logs" 2>/dev/null
      # Consolas de CrewChief y logs de SimHub (suelen ser .txt).
      find . -maxdepth 12 -type f \
        \( -ipath '*CrewChiefV4/DebugLogs*' -o -ipath '*SimHub/Logs*' \) \
        \( -iname '*.log' -o -iname '*.txt' \) -mtime -2 -size -20M -print0 2>/dev/null \
        | xargs -0 -r cp --parents -t "$OUT/windows-logs" 2>/dev/null
    ) || true
    echo "  logs de Windows: $(find "$OUT/windows-logs" -type f 2>/dev/null | wc -l) ficheros"
  fi
fi

# --- 5. datos del sistema ----------------------------------------------------

echo "[5/6] Recogiendo datos del sistema..."
{
  echo "Fecha: $(date)"
  echo
  echo "--- uname ---"
  uname -a
  echo
  echo "--- sistema ---"
  cat /etc/os-release 2>/dev/null || true
  echo
  echo "--- sesion ---"
  echo "XDG_SESSION_TYPE=${XDG_SESSION_TYPE:-?}"
  echo "DISPLAY=${DISPLAY:-?}"
  echo "WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-?}"
  echo
  echo "--- wine ---"
  if command -v wine >/dev/null 2>&1; then
    command -v wine
    wine --version 2>/dev/null || true
  else
    echo "wine no esta en PATH"
  fi
  echo
  echo "--- HF8 (04d8:ee96) ---"
  if command -v lsusb >/dev/null 2>&1; then
    lsusb | grep -i '04d8:ee96' || echo "no visible con lsusb"
  else
    echo "lsusb no instalado"
  fi
  echo
  echo "--- puertos UDP 8771-8774 ---"
  ss -lunp 2>/dev/null | grep -E ':(8771|8772|8773|8774)\b' || echo "ninguno ocupado"
} > "$OUT/sistema.txt" 2>&1

{
  ps -ef 2>/dev/null | grep -i -E 'etol|SimHub|CrewChief|shm_mirror|wine' \
    | grep -v -E 'grep|recoger-logs' || true
} > "$OUT/procesos.txt" 2>&1

if command -v journalctl >/dev/null 2>&1; then
  journalctl --user -b -o cat 2>/dev/null \
    | grep -i -E 'etol|shm_mirror' | tail -n 300 > "$OUT/journal.txt" || true
fi

# --- 6. empaquetar -----------------------------------------------------------

echo "[6/6] Empaquetando..."
tar -czf "$TAR" -C "$(dirname "$OUT")" "$(basename "$OUT")"

echo
echo "Listo."
echo
echo "  Enviar:  $TAR"
echo "  Tamano:  $(du -h "$TAR" | cut -f1)"
echo "  Carpeta: $OUT"
echo
echo "Contenido principal:"
find "$OUT" -maxdepth 1 -mindepth 1 -printf '  %f\n' | sort
if [[ -f "$OUT/doctor.txt" ]]; then
  echo
  echo "Resumen del doctor:"
  grep -E 'FALLO|AVISO' "$OUT/doctor.txt" | head -n 15 || echo "  (sin FALLO ni AVISO)"
fi
if [[ -n "$JUEGO" ]]; then
  echo
  echo "La pila de $JUEGO sigue en marcha. Para pararla:"
  echo "  $CLI stop $JUEGO"
  echo "o pulsa 'Parar' en la aplicacion."
fi

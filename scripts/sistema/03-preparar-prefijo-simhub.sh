#!/usr/bin/env bash
# Crea y prepara la botella de SimHub/CrewChief dentro de Bottles Flatpak.
#
#   ./03-preparar-prefijo-simhub.sh
#   ./03-preparar-prefijo-simhub.sh --dry-run
#   ./03-preparar-prefijo-simhub.sh --sin-instaladores
#   ./03-preparar-prefijo-simhub.sh --bottle simhub --runner ge-proton11-11
#
# Qué hace:
#   1. Comprueba que Bottles Flatpak está instalado en modo usuario.
#   2. Descarga el último GE-Proton (GitHub) y lo registra como runner.
#   3. Crea la botella `simhub` (win64, win11) con ese runner.
#   4. Instala las dependencias que necesitan SimHub y CrewChief:
#      dotnet48, vcrun2019, vcrun2022, d3dcompiler_43, d3dx9 y fuentes.
#   5. Descarga la última versión de CrewChief (MSI) y de SimHub (ZIP) y lanza
#      sus instaladores dentro de la botella.
#
# En los instaladores:
#   CrewChief: al terminar, NO marques «Run CrewChiefV4».
#   SimHub:    desmarca «Install .NET Runtime», «Install USB Display Drivers»
#              y «Install Visual C++ Redistributables»; al terminar, NO lo
#              ejecutes.
#
# No usa sudo. No toca los juegos ni sus prefijos de Proton.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib-bottles-flatpak.sh
source "$HERE/lib-bottles-flatpak.sh"

BOTTLE="${ETOL_BOTTLE_NAME:-simhub}"
RUNNER="${ETOL_RUNNER_NAME:-}"
DOWNLOADS="${ETOL_DOWNLOADS:-$HOME/Descargas/ETOL}"
DRY=0
WITH_DEPS=1
WITH_INSTALLERS=1
UA="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36"

usage() {
  sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY=1; shift ;;
    --bottle) BOTTLE="${2:-}"; [[ -n "$BOTTLE" ]] || die "falta el nombre"; shift 2 ;;
    --runner) RUNNER="${2:-}"; [[ -n "$RUNNER" ]] || die "falta el runner"; shift 2 ;;
    --downloads) DOWNLOADS="${2:-}"; [[ -n "$DOWNLOADS" ]] || die "falta la carpeta"; shift 2 ;;
    --sin-dependencias) WITH_DEPS=0; shift ;;
    --sin-instaladores) WITH_INSTALLERS=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "opción desconocida: $1" ;;
  esac
done

have_flatpak || die "no se encontró flatpak."
flatpak_user_installed || die "Bottles no está instalado para el usuario. Ejecuta 01-instalar-bottles-flatpak.sh"

RUNNER_DIR=""
BOTTLE_DIR="$ETOL_BOTTLES_DATA/bottles/$BOTTLE"
WIN_CREW="Z:\\home"
TMP_DIR=""

cleanup() {
  if [[ -n "${TMP_DIR:-}" && -d "$TMP_DIR" ]]; then
    rm -rf "$TMP_DIR"
  fi
  return 0
}
trap cleanup EXIT

win_path() { printf 'Z:%s' "${1//\//\\}"; }

api_latest_runner() {
  curl -fsSL --max-time 30 \
    'https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/latest' \
    | python3 -c '
import json, sys
d = json.load(sys.stdin)
tag = d.get("tag_name") or ""
for a in d.get("assets", []):
    if a.get("name", "").endswith("-x86_64.tar.gz"):
        print(tag)
        print(a["browser_download_url"])
        break
'
}

resolve_runner() {
  local tag url
  if [[ -n "$RUNNER" ]]; then
    # Nombre ya elegido: si existe, se usa; si no, se busca esa etiqueta.
    if [[ -x "$ETOL_BOTTLES_DATA/runners/$RUNNER/files/bin/wine" ]]; then
      info "runner $RUNNER ya instalado."
      RUNNER_DIR="$ETOL_BOTTLES_DATA/runners/$RUNNER"
      return 0
    fi
    tag="${RUNNER/#ge-proton/GE-Proton}"
  else
    local pair
    if ! pair="$(api_latest_runner)"; then
      # Sin red: usa el runner instalado más reciente, si hay alguno.
      local installed
      installed="$(find "$ETOL_BOTTLES_DATA/runners" -maxdepth 1 -type d -name 'ge-proton*' 2>/dev/null | sort -V | tail -1)"
      if [[ -n "$installed" && -x "$installed/files/bin/wine" ]]; then
        warn "no se pudo consultar GitHub; uso el runner instalado $(basename "$installed")."
        RUNNER="$(basename "$installed")"
        RUNNER_DIR="$installed"
        return 0
      fi
      die "no se pudo consultar la última versión de GE-Proton y no hay ningún runner instalado."
    fi
    tag="$(printf '%s\n' "$pair" | sed -n '1p')"
    url="$(printf '%s\n' "$pair" | sed -n '2p')"
    [[ -n "$tag" && -n "$url" ]] || die "respuesta inesperada de GitHub al buscar GE-Proton."
    RUNNER="$(printf '%s' "$tag" | tr '[:upper:]' '[:lower:]')"
  fi

  if [[ -x "$ETOL_BOTTLES_DATA/runners/$RUNNER/files/bin/wine" ]]; then
    info "runner $RUNNER ya instalado."
    RUNNER_DIR="$ETOL_BOTTLES_DATA/runners/$RUNNER"
    return 0
  fi

  if [[ -z "${url:-}" ]]; then
    url="https://github.com/GloriousEggroll/proton-ge-custom/releases/download/$tag/$tag-x86_64.tar.gz"
  fi

  step "Descargando runner $tag"
  info "$url"
  mkdir -p "$ETOL_BOTTLES_DATA/runners"
  TMP_DIR="$(mktemp -d "$ETOL_BOTTLES_DATA/runners/.etol-runner-XXXXXX")"
  curl -fL --progress-bar "$url" -o "$TMP_DIR/runner.tar.gz"
  info "extrayendo…"
  tar -xzf "$TMP_DIR/runner.tar.gz" -C "$TMP_DIR"
  local src
  src="$(find "$TMP_DIR" -maxdepth 1 -type d -name 'GE-Proton*' | head -1)"
  [[ -n "$src" ]] || die "el tar.gz no contiene una carpeta GE-Proton*."
  mkdir -p "$ETOL_BOTTLES_DATA/runners/$RUNNER"
  shopt -s dotglob
  mv "$src"/* "$ETOL_BOTTLES_DATA/runners/$RUNNER"/
  shopt -u dotglob
  rm -rf "$TMP_DIR"
  TMP_DIR=""
  RUNNER_DIR="$ETOL_BOTTLES_DATA/runners/$RUNNER"
  [[ -x "$RUNNER_DIR/files/bin/wine" ]] || die "el runner no quedó bien instalado."
  info "runner instalado en $RUNNER_DIR"
}

bottle_exists() {
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" list bottles 2>/dev/null \
    | grep -q -- "- $BOTTLE\$"
}

wait_for_prefix() {
  local tries=0
  while (( tries < 300 )); do
    if [[ -f "$BOTTLE_DIR/system.reg" && -d "$BOTTLE_DIR/drive_c" ]]; then
      return 0
    fi
    sleep 1
    tries=$((tries + 1))
  done
  return 1
}

create_bottle() {
  if bottle_exists; then
    info "la botella $BOTTLE ya existe."
    wait_for_prefix || die "la botella existe pero el prefijo no está listo."
    return 0
  fi
  step "Creando la botella $BOTTLE"
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" new \
    --bottle-name "$BOTTLE" \
    --environment application \
    --arch win64 \
    --runner "$RUNNER"
  wait_for_prefix || die "la botella se creó pero el prefijo no está listo."
  info "prefijo listo en $BOTTLE_DIR"
}

configure_bottle() {
  step "Configurando la botella"
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" edit \
    -b "$BOTTLE" --win win11 >/dev/null 2>&1 || warn "no se pudo fijar Windows 11."
  info "Windows 11 y runner $RUNNER."
}

install_dependencies() {
  [[ "$WITH_DEPS" == 1 ]] || { info "dependencias omitidas (--sin-dependencias)."; return 0; }
  step "Instalando dependencias (puede tardar)"
  cat <<'EOF'
  dotnet48 · vcrun2019 · vcrun2022 · d3dcompiler_43 · d3dx9 · corefonts

  winetricks descarga los paquetes de Microsoft y los instala en el prefijo.
  Si alguna ventana pide confirmación, acéptala.
EOF
  local wine="$RUNNER_DIR/files/bin/wine"
  local -a envargs=(
    "--env=WINEPREFIX=$BOTTLE_DIR"
    "--env=WINEDEBUG=-all"
  )
  if [[ -x "$wine" ]]; then
    envargs+=("--env=WINE=$wine" "--env=WINELOADER=$wine" "--env=WINESERVER=$RUNNER_DIR/files/bin/wineserver")
  fi
  flatpak run "${envargs[@]}" --command=winetricks "$ETOL_BOTTLES_ID" -q \
    dotnet48 vcrun2019 vcrun2022 d3dcompiler_43 d3dx9 corefonts \
    || die "winetricks falló; revisa el log y vuelve a ejecutar el script."
  info "dependencias instaladas."
}

crewchief_installer() {
  local out="$DOWNLOADS/CrewChiefV4.msi"
  CREW_MSI=""
  if [[ -s "$out" ]] && file "$out" | grep -qi 'Composite Document'; then
    info "reutilizo $out"
    CREW_MSI="$out"
    return 0
  fi

  mkdir -p "$DOWNLOADS"
  local xml url version
  if [[ -n "${CREWCHIEF_URL:-}" ]]; then
    url="$CREWCHIEF_URL"
    version="personalizada"
  else
    # Fuente principal (puede pedir verificación de Cloudflare).
    xml="$(curl -fsSL -A "$UA" --max-time 25 \
      'https://thecrewchief.org/downloads.php?do=downloadxml' 2>/dev/null || true)"
    url="$(printf '%s' "$xml" | sed -n 's:.*<url>\(.*\)</url>.*:\1:p' | head -1)"
    version="$(printf '%s' "$xml" | sed -n 's:.*<version>\(.*\)</version>.*:\1:p' | head -1)"
    # La fuente principal a veces apunta a una página, no al MSI directo.
    if [[ -n "$url" && "$url" != *.msi ]]; then
      warn "la fuente principal no da un MSI directo ($url); uso el espejo oficial."
      url=""
    fi

    if [[ -z "$url" ]]; then
      warn "la web de CrewChief no respondió (Cloudflare); uso el espejo oficial."
      xml="$(curl -fsSL --max-time 25 \
        'https://gitlab.com/mr_belowski/CrewChiefV4/-/raw/gitlab-update-files/auto_update_data_files/secondary/auto_update_data.xml?ref_type=heads' \
        2>/dev/null || true)"
      url="$(printf '%s' "$xml" | sed -n 's:.*<url>\(.*\)</url>.*:\1:p' | head -1)"
      version="$(printf '%s' "$xml" | sed -n 's:.*<version>\(.*\)</version>.*:\1:p' | head -1)"
      warn "el espejo puede ser anterior; CrewChief se actualizará solo al abrirlo."
    fi
  fi

  [[ -n "$url" ]] || die "no se pudo averiguar la URL del instalador de CrewChief.
Pasa una URL manualmente: CREWCHIEF_URL=https://… ./03-preparar-prefijo-simhub.sh"

  step "Descargando CrewChief ${version:-}"
  info "$url"
  curl -fL --progress-bar "$url" -o "$out" || die "no se pudo descargar CrewChief."
  file "$out" | grep -qi 'Composite Document' \
    || die "el archivo descargado no es un MSI válido: $out"
  CREW_MSI="$out"
}

simhub_installer() {
  local json tag url zip outdir
  SIMHUB_SETUP=""
  json="$(curl -fsSL --max-time 30 \
    'https://api.github.com/repos/SHWotever/SimHub/releases/latest')" \
    || die "no se pudo consultar la última versión de SimHub."
  tag="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name",""))')"
  url="$(printf '%s' "$json" | python3 -c '
import json, sys
d = json.load(sys.stdin)
for a in d.get("assets", []):
    if a.get("name", "").lower().endswith(".zip"):
        print(a["browser_download_url"]); break
')"
  [[ -n "$tag" && -n "$url" ]] || die "respuesta inesperada de GitHub al buscar SimHub."

  zip="$DOWNLOADS/SimHub-$tag.zip"
  outdir="$DOWNLOADS/simhub-$tag"
  mkdir -p "$DOWNLOADS"
  if [[ ! -s "$zip" ]]; then
    step "Descargando SimHub $tag"
    info "$url"
    curl -fL --progress-bar "$url" -o "$zip" || die "no se pudo descargar SimHub."
  else
    info "reutilizo $zip"
  fi

  if [[ ! -d "$outdir" ]] || ! find "$outdir" -maxdepth 1 -iname 'SimHubSetup*.exe' | grep -q .; then
    step "Extrayendo SimHub"
    rm -rf "$outdir"
    mkdir -p "$outdir"
    unzip -q "$zip" -d "$outdir" || die "no se pudo extraer $zip"
  fi
  local setup
  setup="$(find "$outdir" -maxdepth 2 -iname 'SimHubSetup*.exe' | head -1)"
  [[ -n "$setup" ]] || die "el ZIP de SimHub no contiene SimHubSetup*.exe"
  SIMHUB_SETUP="$setup"
}

launch_crewchief_installer() {
  local msi="$1" cmd
  step "Instalador de CrewChief"
  cat <<'EOF'
  Se va a abrir el instalador de CrewChief.

    - Pulsa Siguiente/Aceptar hasta el final.
    - Si al terminar ofrece «Run CrewChiefV4» o «Launch CrewChief»,
      DESMÁRCALO antes de cerrar.
    - No hace falta instalar plugins de juegos desde aquí.

EOF
  cmd="msiexec /i \"$(win_path "$msi")\""
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" shell -b "$BOTTLE" -i "$cmd"
}

launch_simhub_installer() {
  local setup="$1"
  step "Instalador de SimHub"
  cat <<'EOF'
  Se va a abrir el instalador de SimHub. DESMARCA:

    [ ] Install .NET Runtime
    [ ] Install USB Display Drivers
    [ ] Install Visual C++ Redistributables

  Las dependencias ya las instala este script dentro del prefijo. Al terminar,
  NO marques «Run SimHub».

EOF
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" run \
    -b "$BOTTLE" -e "$(win_path "$setup")"
}

check_installed() {
  step "Comprobando la instalación"
  local ok=0
  if [[ -f "$BOTTLE_DIR/drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4/CrewChiefV4.exe" ]]; then
    info "CrewChief: instalado."
    ok=1
  else
    warn "CrewChief no aparece en la botella."
  fi
  if [[ -f "$BOTTLE_DIR/drive_c/Program Files (x86)/SimHub/SimHubWPF.exe" ]]; then
    info "SimHub: instalado."
    ok=1
  else
    warn "SimHub no aparece en la botella."
  fi
  [[ "$ok" == 1 ]] || warn "los instaladores no dejaron los ejecutables esperados."
}

if [[ "$DRY" == 1 ]]; then
  step "Simulación"
  info "runner: ${RUNNER:-el último GE-Proton}"
  info "botella: $BOTTLE (win64, win11) en $BOTTLE_DIR"
  info "dependencias: dotnet48 vcrun2019 vcrun2022 d3dcompiler_43 d3dx9 corefonts"
  info "CrewChief: se descargará el MSI más reciente."
  info "SimHub: se descargará el ZIP más reciente de GitHub."
  info "instaladores: se lanzarán dentro de la botella salvo --sin-instaladores."
  exit 0
fi

step "Bottles Flatpak"
info "usuario: instalado · botella: $BOTTLE · descargas: $DOWNLOADS"

resolve_runner
create_bottle
configure_bottle
install_dependencies

crewchief_installer
simhub_installer
[[ -n "$CREW_MSI" && -n "$SIMHUB_SETUP" ]] || die "no se pudieron preparar los instaladores."

if [[ "$WITH_INSTALLERS" == 1 ]]; then
  launch_crewchief_installer "$CREW_MSI"
  launch_simhub_installer "$SIMHUB_SETUP"
else
  step "Instaladores descargados"
  info "CrewChief: $CREW_MSI"
  info "SimHub:    $SIMHUB_SETUP"
  info "Ejecuta este script sin --sin-instaladores para lanzarlos."
fi

check_installed

step "Listo"
cat <<EOF
  Botella: $BOTTLE_DIR
  Runner:  $RUNNER

  Abre ETOL y pulsa «Redetectar» para que tome la botella y los ejecutables.
  Si CrewChief o SimHub piden actualizarse al abrirlos, acepta la actualización.
EOF

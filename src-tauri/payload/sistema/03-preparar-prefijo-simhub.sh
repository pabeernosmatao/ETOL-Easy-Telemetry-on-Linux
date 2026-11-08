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
#      dotnet48, vcrun2022, d3dcompiler_43, d3dx9 y fuentes.
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

need() {
  command -v "$1" >/dev/null 2>&1 && return 0
  die "falta «$1» en el sistema. Instálalo antes de continuar (${2:-usa el gestor de paquetes de tu distribución})."
}

have_flatpak || die "no se encontró flatpak."
flatpak_user_installed || die "Bottles no está instalado para el usuario. Ejecuta 01-instalar-bottles-flatpak.sh"

need curl "Arch/CachyOS: sudo pacman -S curl · Ubuntu: sudo apt install curl · Fedora: sudo dnf install curl"
need tar "Arch/CachyOS: sudo pacman -S tar · Ubuntu: sudo apt install tar · Fedora: sudo dnf install tar"
need unzip "Arch/CachyOS: sudo pacman -S unzip · Ubuntu: sudo apt install unzip · Fedora: sudo dnf install unzip"
need file "Arch/CachyOS: sudo pacman -S file · Ubuntu: sudo apt install file · Fedora: sudo dnf install file"

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

# Ultima etiqueta de una release. Primero usa la redireccion de GitHub, que no
# consume el limite de la API; si esa ruta falla (a veces responde 504), se
# consulta api.github.com como respaldo.
github_latest_tag() {
  local repo="$1" url tag
  url="$(curl -fsSL -A "$UA" -o /dev/null -w '%{url_effective}' --max-time 30 \
    "https://github.com/$repo/releases/latest" 2>/dev/null || true)"
  tag="$(printf '%s\n' "$url" | sed -n 's#.*/releases/tag/\([^/]*\)$#\1#p')"
  if [[ -z "$tag" ]]; then
    tag="$(curl -fsSL -A "$UA" --max-time 30 \
      "https://api.github.com/repos/$repo/releases/latest" 2>/dev/null \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -n1 || true)"
  fi
  [[ -n "$tag" ]] || return 1
  printf '%s\n' "$tag"
}

api_latest_runner() {
  github_latest_tag GloriousEggroll/proton-ge-custom
}
api_latest_runner() {
  github_latest_tag GloriousEggroll/proton-ge-custom
}

# URL de un asset concreto de una release, consultando la API de GitHub.
github_asset_api_url() {
  local repo="$1" tag="$2" name="$3"
  curl -fsSL -A "$UA" --max-time 30 \
    "https://api.github.com/repos/$repo/releases/tags/$tag" 2>/dev/null \
    | python3 -c 'import json, sys
name = sys.argv[1]
try:
    data = json.load(sys.stdin)
except Exception:
    sys.exit(1)
for asset in data.get("assets", []):
    if asset.get("name") == name:
        print(asset.get("url", ""))
        break
' "$name"
}

# Descarga un asset de una release. La ruta /releases/download/ de github.com
# puede responder 504 (incidencias del CDN); api.github.com sigue funcionando y
# redirige al mismo fichero, asi que se usa como respaldo.
github_download_asset() {
  local repo="$1" tag="$2" name="$3" out="$4" url api
  url="https://github.com/$repo/releases/download/$tag/$name"
  if curl -fL --progress-bar --max-time 1800 "$url" -o "$out"; then
    return 0
  fi
  warn "la descarga directa de GitHub fallo; probando la API de GitHub."
  api="$(github_asset_api_url "$repo" "$tag" "$name" || true)"
  [[ -n "$api" ]] || die "no se encontro $name en la release $tag de $repo."
  curl -fL --progress-bar --max-time 1800 \
    -H "Accept: application/octet-stream" "$api" -o "$out"
}

resolve_runner() {
  local tag url
  progress 10 "Buscando la última versión de GE-Proton"
  if [[ -n "$RUNNER" ]]; then
    # Nombre ya elegido: si existe, se usa; si no, se busca esa etiqueta.
    if [[ -x "$ETOL_BOTTLES_DATA/runners/$RUNNER/files/bin/wine" ]]; then
      info "runner $RUNNER ya instalado."
      RUNNER_DIR="$ETOL_BOTTLES_DATA/runners/$RUNNER"
      return 0
    fi
    tag="${RUNNER/#ge-proton/GE-Proton}"
  else
    if ! tag="$(api_latest_runner)" || [[ -z "$tag" ]]; then
      # Sin red: usa el runner instalado más reciente, si hay alguno.
      local installed=""
      # En una instalacion limpia la carpeta runners aun no existe; sin esta
      # guarda, `find` falla, pipefail propaga el error y el script termina
      # sin mensaje (solo "codigo 1" en la interfaz).
      if [[ -d "$ETOL_BOTTLES_DATA/runners" ]]; then
        installed="$(find "$ETOL_BOTTLES_DATA/runners" -maxdepth 1 -type d -name 'ge-proton*' | sort -V | tail -1)"
      fi
      if [[ -n "$installed" && -x "$installed/files/bin/wine" ]]; then
        warn "no se pudo consultar GitHub; uso el runner instalado $(basename "$installed")."
        RUNNER="$(basename "$installed")"
        RUNNER_DIR="$installed"
        return 0
      fi
      die "no se pudo consultar la última versión de GE-Proton y no hay ningún runner instalado.
Comprueba la conexión (o pasa una versión con --runner, por ejemplo --runner ge-proton11-11)."
    fi
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
  progress 20 "Descargando GE-Proton (puede tardar)"
  info "$url"
  mkdir -p "$ETOL_BOTTLES_DATA/runners"
  TMP_DIR="$(mktemp -d "$ETOL_BOTTLES_DATA/runners/.etol-runner-XXXXXX")"
  github_download_asset GloriousEggroll/proton-ge-custom "$tag" \
    "$tag-x86_64.tar.gz" "$TMP_DIR/runner.tar.gz"
  info "extrayendo…"
  tar -xzf "$TMP_DIR/runner.tar.gz" -C "$TMP_DIR"
  local src
  src="$(find "$TMP_DIR" -maxdepth 1 -type d -name 'GE-Proton*' | head -1 || true)"
  [[ -n "$src" ]] || die "el tar.gz no contiene una carpeta GE-Proton*."
  mkdir -p "$ETOL_BOTTLES_DATA/runners/$RUNNER"
  shopt -s dotglob
  mv "$src"/* "$ETOL_BOTTLES_DATA/runners/$RUNNER"/
  shopt -u dotglob
  rm -rf "$TMP_DIR"
  TMP_DIR=""
  RUNNER_DIR="$ETOL_BOTTLES_DATA/runners/$RUNNER"
  [[ -x "$RUNNER_DIR/files/bin/wine" ]] || die "el runner no quedó bien instalado."
  progress 40 "Runner GE-Proton instalado"
  info "runner instalado en $RUNNER_DIR"
}

bottle_exists() {
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" list bottles 2>/dev/null \
    | grep -q -- "- $BOTTLE\$"
}

# Bottles descarga sus componentes (dxvk, vkd3d, nvapi, latencyflex,
# winebridge) directamente de github.com/releases/download, que a veces
# responde 504. Cuando falla la creacion de la botella, ETOL descarga esos
# mismos ficheros por la API de GitHub y los deja en la cache de Bottles
# (~/.var/.../bottles/temp). Bottles comprueba el MD5 y, si coinciden, no
# vuelve a descargarlos.
precache_bottles_components() {
  python3 - "$ETOL_BOTTLES_DATA" <<'PY_COMPONENTS'
import hashlib
import json
import os
import re
import sys
import urllib.error
import urllib.request

base = sys.argv[1]
temp = os.path.join(base, "temp")
repos = os.path.join(temp, "repositories", "components")
if not os.path.isdir(repos):
    sys.exit(0)

UA = "ETOL/1.0"
GH_RELEASE = re.compile(
    r"^https://github\.com/([^/]+)/([^/]+)/releases/download/([^/]+)/(.+)$"
)


def get(url, accept=None):
    headers = {"User-Agent": UA}
    if accept:
        headers["Accept"] = accept
    return urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=300)


def download(url, dest):
    try:
        with get(url) as resp, open(dest, "wb") as fh:
            while True:
                chunk = resp.read(1 << 20)
                if not chunk:
                    break
                fh.write(chunk)
        return True
    except urllib.error.HTTPError as error:
        match = GH_RELEASE.match(url)
        if error.code not in (403, 404, 429, 500, 502, 503, 504) or not match:
            return False
    except Exception:
        return False

    owner, repo, tag, name = match.groups()
    try:
        with get(f"https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}") as resp:
            data = json.load(resp)
        asset = next((a for a in data.get("assets", []) if a.get("name") == name), None)
        if not asset:
            return False
        with get(asset["url"], "application/octet-stream") as resp, open(dest, "wb") as fh:
            while True:
                chunk = resp.read(1 << 20)
                if not chunk:
                    break
                fh.write(chunk)
        return True
    except Exception:
        return False


prepared = 0
for root, _dirs, files in os.walk(repos):
    for entry in files:
        if not entry.endswith(".yml") or entry == "catalog.yml":
            continue
        text = open(os.path.join(root, entry), encoding="utf-8", errors="replace").read()
        name = re.search(r"^Name:\s*(\S+)", text, re.M)
        file_name = re.search(r"^\s*-?\s*file_name:\s*(\S+)", text, re.M)
        url = re.search(r"^\s*url:\s*(\S+)", text, re.M)
        checksum = re.search(r"^\s*file_checksum:\s*([0-9a-fA-F]{32})", text, re.M)
        rename = re.search(r"^\s*rename:\s*(\S+)", text, re.M)
        if not (name and file_name and url and checksum):
            continue
        target = os.path.join(temp, (rename.group(1) if rename else file_name.group(1)))
        md5 = checksum.group(1).lower()
        if os.path.isfile(target):
            with open(target, "rb") as fh:
                if hashlib.md5(fh.read()).hexdigest() == md5:
                    continue
            os.remove(target)
        print(f"  componente {name.group(1)}: descargando…", flush=True)
        if not download(url.group(1), target):
            print(f"AVISO: no se pudo descargar {name.group(1)}", file=sys.stderr)
            continue
        with open(target, "rb") as fh:
            got = hashlib.md5(fh.read()).hexdigest()
        if got != md5:
            os.remove(target)
            print(f"AVISO: {name.group(1)} no coincide con su MD5", file=sys.stderr)
            continue
        prepared += 1
print(f"  componentes preparados: {prepared}", flush=True)
PY_COMPONENTS
}

bottle_new() {
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" new \
    --bottle-name "$BOTTLE" \
    --environment application \
    --arch win64 \
    --runner "$RUNNER"
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
  progress 45 "Creando la botella $BOTTLE"
  if ! bottle_new; then
    warn "Bottles no pudo crearla; preparando sus componentes desde la API de GitHub."
    progress 46 "Preparando componentes de Bottles"
    precache_bottles_components || true
    if ! bottle_new; then
      die "Bottles no pudo crear la botella.
Si la salida muestra «504», es una caída temporal de las descargas de GitHub:
reintenta en unos minutos."
    fi
  fi
  wait_for_prefix || die "la botella se creó pero el prefijo no está listo."
  progress 50 "Prefijo creado"
  info "prefijo listo en $BOTTLE_DIR"
}

configure_bottle() {
  progress 52 "Configurando la botella (Windows 11)"
  step "Configurando la botella"
  flatpak run --command=bottles-cli "$ETOL_BOTTLES_ID" edit \
    -b "$BOTTLE" --win win11 >/dev/null 2>&1 || warn "no se pudo fijar Windows 11."
  info "Windows 11 y runner $RUNNER."
}

install_dependencies() {
  [[ "$WITH_DEPS" == 1 ]] || { info "dependencias omitidas (--sin-dependencias)."; return 0; }
  step "Instalando dependencias (puede tardar)"
  progress 55 "Instalando dependencias con winetricks (puede tardar)"
  cat <<'EOF'
  dotnet48 · vcrun2022 · d3dcompiler_43 · d3dx9 · corefonts

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
  # vcrun2022 (2015-2022) ya cubre vcrun2019. Instalar las dos hace que
  # winetricks aborte con "vcrun2022 conflicts with vcrun2019"; --force
  # permite actualizar la que ya estuviera puesta en una botella anterior.
  flatpak run "${envargs[@]}" --command=winetricks "$ETOL_BOTTLES_ID" -q --force \
    dotnet48 vcrun2022 d3dcompiler_43 d3dx9 corefonts \
    || die "winetricks falló; revisa el log y vuelve a ejecutar el script."
  progress 80 "Dependencias instaladas"
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

  progress 82 "Descargando CrewChief"
  step "Descargando CrewChief ${version:-}"
  info "$url"
  curl -fL --progress-bar "$url" -o "$out" || die "no se pudo descargar CrewChief."
  file "$out" | grep -qi 'Composite Document' \
    || die "el archivo descargado no es un MSI válido: $out"
  CREW_MSI="$out"
}

simhub_installer() {
  local page tag url zip outdir
  SIMHUB_SETUP=""
  tag="$(github_latest_tag SHWotever/SimHub)" \
    || die "no se pudo consultar la última versión de SimHub (GitHub no responde)."
  [[ -n "$tag" ]] || die "respuesta inesperada de GitHub al buscar SimHub."
  # La pagina de assets no usa la API y no tiene limite de peticiones.
  page="$(curl -fsSL -A "$UA" --max-time 30 \
    "https://github.com/SHWotever/SimHub/releases/expanded_assets/$tag" 2>/dev/null || true)"
  url="$(printf '%s' "$page" | grep -oE 'href="[^"]+/releases/download/[^"]+\.zip"' | head -n1 \
    | sed 's/^href="//; s/"$//')"
  if [[ -z "$url" ]]; then
    url="https://github.com/SHWotever/SimHub/releases/download/$tag/SimHub.$tag.zip"
  elif [[ "$url" != http* ]]; then
    url="https://github.com$url"
  fi

  zip="$DOWNLOADS/SimHub-$tag.zip"
  outdir="$DOWNLOADS/simhub-$tag"
  mkdir -p "$DOWNLOADS"
  progress 90 "Descargando SimHub"
  if [[ ! -s "$zip" ]]; then
    step "Descargando SimHub $tag"
    info "$url"
    local asset
    asset="$(basename "${url%%\?*}")"
    [[ -n "$asset" ]] || asset="SimHub.$tag.zip"
    github_download_asset SHWotever/SimHub "$tag" "$asset" "$zip" \
      || die "no se pudo descargar SimHub."
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
  setup="$(find "$outdir" -maxdepth 2 -iname 'SimHubSetup*.exe' | head -1 || true)"
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
    - El MSI viene del espejo oficial y puede ser una version anterior:
      abre CrewChief una vez al terminar y acepta la actualizacion.
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
  info "dependencias: dotnet48 vcrun2022 d3dcompiler_43 d3dx9 corefonts"
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
  progress 96 "Abriendo los instaladores de CrewChief y SimHub"
  launch_crewchief_installer "$CREW_MSI"
  launch_simhub_installer "$SIMHUB_SETUP"
else
  step "Instaladores descargados"
  info "CrewChief: $CREW_MSI"
  info "SimHub:    $SIMHUB_SETUP"
  info "Ejecuta este script sin --sin-instaladores para lanzarlos."
fi

check_installed

progress 100 "Listo"
step "Listo"
cat <<EOF
  Botella: $BOTTLE_DIR
  Runner:  $RUNNER

  Abre ETOL y pulsa «Redetectar» para que tome la botella y los ejecutables.
  Si CrewChief o SimHub piden actualizarse al abrirlos, acepta la actualización.
EOF

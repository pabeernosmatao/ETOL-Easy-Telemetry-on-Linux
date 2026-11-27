#!/usr/bin/env bash
# Utilidades compartidas por los scripts de Bottles Flatpak de ETOL.
#
# No se ejecuta directamente: los scripts 01, 02 y 03 hacen `source` de este
# fichero. Lee el ámbito real de Bottles (usuario o sistema), normaliza las
# rutas del home (`/home` -> `/var/home` en Bazzite) y elige el gestor de
# paquetes correcto (rpm-ostree, pacman, apt o dnf).

ETOL_BOTTLES_ID="com.usebottles.bottles"
HOME_REAL="$(realpath -m "${HOME:-/}" 2>/dev/null || printf '%s' "${HOME:-/}")"
# Para rutas que se pasan a `flatpak run` se usa $HOME literal: dentro del
# sandbox el home es /home/<usuario>, no /var/home/<usuario>. HOME_REAL se usa
# solo para comparar rutas del host.
ETOL_BOTTLES_DATA="$HOME/.var/app/$ETOL_BOTTLES_ID/data/bottles"

step() { printf '\n== %s ==\n' "$*"; }
info() { printf '  %s\n' "$*"; }
warn() { printf 'AVISO: %s\n' "$*" >&2; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

# Si una orden falla sin pasar por die(), deja el motivo en la salida.
# Sin esto, `set -e` termina el script y la interfaz solo muestra
# "El script termino con errores (codigo 1)" sin ninguna pista.
trap 'code=$?; printf "ERROR: fallo inesperado en la linea %s (codigo %s).\n" "$LINENO" "$code" >&2' ERR

# Escribe el progreso (0-100) y el texto de la etapa para la barra de ETOL.
# La interfaz pasa ETOL_PROGRESS_FILE; si no existe, no hace nada.
progress() {
  local percent="$1"; shift
  local text="${*:-}"
  [[ -n "${ETOL_PROGRESS_FILE:-}" ]] || return 0
  [[ "$percent" =~ ^[0-9]+$ ]] || return 0
  printf '%s|%s\n' "$percent" "$text" > "$ETOL_PROGRESS_FILE.tmp" 2>/dev/null || return 0
  mv -f "$ETOL_PROGRESS_FILE.tmp" "$ETOL_PROGRESS_FILE" 2>/dev/null || true
  return 0
}

have_flatpak() {
  command -v flatpak >/dev/null 2>&1
}

# Gestor de paquetes del sistema. En Atomic se usa rpm-ostree aunque tambien
# exista dnf, porque /usr es de solo lectura.
package_backend() {
  if [[ -e /run/ostree-booted ]] || command -v rpm-ostree >/dev/null 2>&1; then
    printf 'rpm-ostree'
  elif command -v pacman >/dev/null 2>&1; then
    printf 'pacman'
  elif command -v apt-get >/dev/null 2>&1 || command -v apt >/dev/null 2>&1; then
    printf 'apt'
  elif command -v dnf >/dev/null 2>&1; then
    printf 'dnf'
  else
    printf 'unknown'
  fi
}

# Comando que puede pegar el usuario en una terminal para instalar un paquete.
package_install_hint() {
  local pkg
  pkg="$(package_for_command "$1")"
  case "$(package_backend)" in
    rpm-ostree) printf 'rpm-ostree install --apply-live %s' "$pkg" ;;
    pacman) printf 'sudo pacman -S %s' "$pkg" ;;
    apt) printf 'sudo apt install %s' "$pkg" ;;
    dnf) printf 'sudo dnf install %s' "$pkg" ;;
    *) printf 'instala %s con el gestor de paquetes de tu distribucion' "$pkg" ;;
  esac
}

# Paquete que da un comando en cada familia.
package_for_command() {
  case "$(package_backend):$1" in
    pacman:python3) printf 'python' ;;
    pacman:bsdtar) printf 'libarchive' ;;
    apt:bsdtar) printf 'libarchive-tools' ;;
    *) printf '%s' "$1" ;;
  esac
}

# Paquete que da `bsdtar` en cada familia.
bsdtar_package() {
  package_for_command bsdtar
}

flatpak_user_installed() {
  flatpak list --user --app --columns=application 2>/dev/null \
    | grep -qx "$ETOL_BOTTLES_ID"
}

flatpak_system_installed() {
  flatpak list --system --app --columns=application 2>/dev/null \
    | grep -qx "$ETOL_BOTTLES_ID"
}

# Ámbito que usa ETOL: la copia de usuario si existe; si no, la de sistema.
# Bazzite trae Bottles como Flatpak de sistema y es perfectamente válido.
flatpak_scope() {
  if flatpak_user_installed; then
    printf 'user'
  elif flatpak_system_installed; then
    printf 'system'
  else
    printf 'none'
  fi
}

flatpak_scope_arg() {
  case "$(flatpak_scope)" in
    user) printf -- '--user' ;;
    system) printf -- '--system' ;;
    *) printf '' ;;
  esac
}

# Ejecuta Bottles en el ámbito activo, no siempre en el de usuario.
flatpak_run() {
  case "$(flatpak_scope)" in
    system) flatpak run --system "$@" ;;
    *) flatpak run --user "$@" ;;
  esac
}

ensure_flathub_user() {
  step "Remoto flathub (usuario)"
  if flatpak remotes --user --columns=name 2>/dev/null | grep -qx flathub; then
    info "flathub ya está dado de alta para este usuario."
    return 0
  fi
  flatpak remote-add --user --if-not-exists flathub \
    https://flathub.org/repo/flathub.flatpakrepo
  info "flathub añadido."
}

# Bibliotecas de Steam que están fuera del home real. Dentro del home el
# permiso `home` ya las cubre. Se canonicalizan las dos partes para que
# $HOME y /var$HOME se comparen como la misma carpeta.
steam_libraries_outside_home() {
  local vdf lib lib_real
  for vdf in \
    "$HOME/.local/share/Steam/steamapps/libraryfolders.vdf" \
    "$HOME/.steam/steam/steamapps/libraryfolders.vdf" \
    "$HOME/.steam/root/steamapps/libraryfolders.vdf" \
    "$HOME/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/libraryfolders.vdf"; do
    [[ -f "$vdf" ]] || continue
    grep -oE '"path"[[:space:]]+"[^"]+"' "$vdf" 2>/dev/null \
      | sed -E 's/.*"path"[[:space:]]+"([^"]+)"/\1/'
  done | sort -u | while IFS= read -r lib; do
    lib_real="$(realpath -m "$lib" 2>/dev/null || printf '%s' "$lib")"
    [[ -d "$lib_real" ]] || continue
    case "$lib_real" in
      "$HOME_REAL"|"$HOME_REAL"/*) ;;
      *) printf '%s\n' "$lib_real" ;;
    esac
  done
}

print_permissions() {
  local scope_arg
  scope_arg="$(flatpak_scope_arg)"
  step "Permisos de Bottles Flatpak ($(flatpak_scope))"
  flatpak info "$scope_arg" --show-permissions "$ETOL_BOTTLES_ID" 2>/dev/null \
    | sed -n '/^\[Context\]/,/^$/p' || warn "no se pudieron leer los permisos."
}

# Comprueba la postcondición: no basta con que `flatpak override` salga 0.
verify_bottles_permissions() {
  local scope_arg info
  scope_arg="$(flatpak_scope_arg)"
  info="$(flatpak info "$scope_arg" --show-permissions "$ETOL_BOTTLES_ID" 2>/dev/null || true)"
  [[ -n "$info" ]] || return 1
  printf '%s\n' "$info" | grep -q '^filesystems=.*home' || return 1
  printf '%s\n' "$info" | grep -q '^filesystems=.*host-etc' || return 1
  printf '%s\n' "$info" | grep -q '^devices=.*all' || return 1

  local lib
  while IFS= read -r lib; do
    [[ -n "$lib" ]] || continue
    printf '%s\n' "$info" | grep -Fq "$lib" || return 1
  done < <(steam_libraries_outside_home)
  return 0
}

# Permisos mínimos para que Bottles vea las bibliotecas de Steam del equipo y
# el pad HF8 (/dev/ttyACM0). Se quita `host` y `host-os`: no hace falta dar
# acceso a todo el sistema.
#
# En un Flatpak de sistema el override se escribe en /etc y necesita
# autorización; se usa pkexec (o sudo) y después se verifica.
apply_bottles_permissions() {
  local scope
  scope="$(flatpak_scope)"
  [[ "$scope" != "none" ]] || die "Bottles no está instalado en ningún ámbito."

  step "Aplicando permisos mínimos ($scope)"
  local -a args=(
    "$(flatpak_scope_arg)"
    --nofilesystem=host
    --nofilesystem=host-os
    --filesystem=home
    --filesystem=host-etc
    --filesystem=xdg-data/Steam
    --filesystem=~/.steam
    --device=all
  )
  local lib
  while IFS= read -r lib; do
    [[ -n "$lib" ]] && args+=(--filesystem="$lib")
  done < <(steam_libraries_outside_home)

  if [[ "$scope" == "system" ]]; then
    info "Bottles es un Flatpak de sistema; se pedirá autorización de administrador."
    if command -v pkexec >/dev/null 2>&1; then
      pkexec "$(command -v flatpak)" override "${args[@]}" "$ETOL_BOTTLES_ID"
    elif command -v sudo >/dev/null 2>&1; then
      sudo "$(command -v flatpak)" override "${args[@]}" "$ETOL_BOTTLES_ID"
    else
      die "no se encontró pkexec ni sudo para aplicar permisos al Flatpak de sistema."
    fi
  else
    flatpak override "${args[@]}" "$ETOL_BOTTLES_ID"
  fi

  verify_bottles_permissions \
    || die "el override terminó bien pero los permisos no se aplicaron. Revisa 'flatpak info $(flatpak_scope_arg) --show-permissions $ETOL_BOTTLES_ID'."

  info "home, host-etc, Steam y devices=all."
  local external
  external="$(steam_libraries_outside_home | tr '\n' ' ')"
  if [[ -n "$external" ]]; then
    info "bibliotecas fuera del home: $external"
  else
    info "no hay bibliotecas de Steam fuera del home."
  fi
  info "host y host-os quedan denegados."
}

bottles_running() {
  pgrep -f "$ETOL_BOTTLES_ID" >/dev/null 2>&1
}

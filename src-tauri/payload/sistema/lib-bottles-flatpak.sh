#!/usr/bin/env bash
# Utilidades compartidas por los scripts de Bottles Flatpak de ETOL.
#
# No se ejecuta directamente: los scripts 01 y 02 hacen `source` de este
# fichero. Todo lo que hace es leer rutas y aplicar permisos con `flatpak
# override --user`; nunca usa sudo.

ETOL_BOTTLES_ID="com.usebottles.bottles"
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

flatpak_user_installed() {
  flatpak list --user --app --columns=application 2>/dev/null \
    | grep -qx "$ETOL_BOTTLES_ID"
}

flatpak_system_installed() {
  flatpak list --system --app --columns=application 2>/dev/null \
    | grep -qx "$ETOL_BOTTLES_ID"
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

# Bibliotecas de Steam que están fuera del home. Dentro del home el permiso
# `home` ya las cubre.
steam_libraries_outside_home() {
  local vdf lib
  for vdf in \
    "$HOME/.local/share/Steam/steamapps/libraryfolders.vdf" \
    "$HOME/.steam/steam/steamapps/libraryfolders.vdf" \
    "$HOME/.steam/root/steamapps/libraryfolders.vdf" \
    "$HOME/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/libraryfolders.vdf"; do
    [[ -f "$vdf" ]] || continue
    grep -oE '"path"[[:space:]]+"[^"]+"' "$vdf" 2>/dev/null \
      | sed -E 's/.*"path"[[:space:]]+"([^"]+)"/\1/'
  done | sort -u | while IFS= read -r lib; do
    [[ -d "$lib" ]] || continue
    case "$lib" in
      "$HOME"|"$HOME"/*) ;;
      *) printf '%s\n' "$lib" ;;
    esac
  done
}

print_permissions() {
  step "Permisos de Bottles Flatpak"
  flatpak info --user --show-permissions "$ETOL_BOTTLES_ID" 2>/dev/null \
    | sed -n '/^\[Context\]/,/^$/p' || warn "no se pudieron leer los permisos."
}

# Permisos mínimos para que Bottles vea las bibliotecas de Steam del equipo y
# el pad HF8 (/dev/ttyACM0). Se quita `host` y `host-os`: no hace falta dar
# acceso a todo el sistema.
apply_bottles_permissions() {
  step "Aplicando permisos mínimos"
  local -a args=(
    --user
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

  flatpak override "${args[@]}" "$ETOL_BOTTLES_ID"

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

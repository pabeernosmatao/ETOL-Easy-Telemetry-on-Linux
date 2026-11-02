#!/usr/bin/env bash
# Instala Bottles como Flatpak en modo usuario y le da los permisos mínimos
# que necesita ETOL (Steam + pad HF8). No usa sudo.
#
#   ./01-instalar-bottles-flatpak.sh
#   ./01-instalar-bottles-flatpak.sh --dry-run
#
# Qué hace:
#   1. Comprueba que `flatpak` está instalado.
#   2. Añade flathub al usuario si falta.
#   3. Instala com.usebottles.bottles con `flatpak install --user`.
#   4. Aplica `flatpak override --user`: home, host-etc, bibliotecas de Steam
#      fuera del home y devices=all (HF8). Quita host y host-os.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib-bottles-flatpak.sh
source "$HERE/lib-bottles-flatpak.sh"

DRY=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    -h|--help)
      sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      die "opción desconocida: $arg"
      ;;
  esac
done

if ! have_flatpak; then
  cat >&2 <<'EOF'
ERROR: no se encontró `flatpak`.

Instálalo primero con el gestor de tu distribución:
  Ubuntu/Debian:  sudo apt install flatpak
  Arch/CachyOS:   sudo pacman -S flatpak
  Fedora:         sudo dnf install flatpak

Después vuelve a ejecutar este script.
EOF
  exit 1
fi

step "Comprobando Bottles Flatpak"
if flatpak_user_installed; then
  info "Bottles ya está instalado para este usuario."
  INSTALL=0
else
  info "Bottles no está instalado para este usuario."
  INSTALL=1
fi

if [[ "$DRY" == 1 ]]; then
  step "Simulación"
  info "flatpak remote-add --user --if-not-exists flathub …"
  if [[ "$INSTALL" == 1 ]]; then
    info "flatpak install --user -y flathub $ETOL_BOTTLES_ID"
  fi
  info "flatpak override --user --nofilesystem=host --nofilesystem=host-os \\"
  info "  --filesystem=home --filesystem=host-etc --filesystem=xdg-data/Steam \\"
  info "  --filesystem=~/.steam --device=all … $ETOL_BOTTLES_ID"
  exit 0
fi

ensure_flathub_user

if [[ "$INSTALL" == 1 ]]; then
  step "Instalando Bottles (usuario)"
  flatpak install --user -y flathub "$ETOL_BOTTLES_ID"
  info "instalado."
else
  info "no hace falta instalar."
fi

flatpak_user_installed || die "la instalación no dejó Bottles disponible para el usuario."

apply_bottles_permissions
print_permissions

step "Listo"
info "Bottles Flatpak está instalado en modo usuario."
info "Si Bottles estaba abierto, ciérralo y vuelve a abrirlo para que tome los permisos."

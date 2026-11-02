#!/usr/bin/env bash
# Pasa Bottles de una instalación de sistema (admin) a una instalación de
# usuario y aplica los permisos que necesita ETOL.
#
#   ./02-bottles-flatpak-a-usuario.sh
#   ./02-bottles-flatpak-a-usuario.sh --dry-run
#
# Qué hace:
#   1. Si Bottles solo está instalado para todo el sistema, instala la copia de
#      usuario (flathub). La copia de usuario tiene prioridad y comparte las
#      mismas botellas y datos (~/.var/app/com.usebottles.bottles).
#   2. Aplica `flatpak override --user` con los permisos mínimos.
#   3. Explica cómo quitar la copia de sistema cuando el usuario quiera:
#      `sudo flatpak uninstall --system com.usebottles.bottles`.
#
# No borra nada del sistema: para eso haría falta sudo y este script no lo usa.

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

have_flatpak || die "no se encontró flatpak; ejecuta antes 01-instalar-bottles-flatpak.sh"

step "Estado actual"
USER=0
SYSTEM=0
flatpak_user_installed && USER=1
flatpak_system_installed && SYSTEM=1

if [[ "$USER" == 1 ]]; then
  info "Bottles de usuario: instalado."
else
  info "Bottles de usuario: no instalado."
fi
if [[ "$SYSTEM" == 1 ]]; then
  info "Bottles de sistema (admin): instalado."
else
  info "Bottles de sistema (admin): no instalado."
fi

if [[ "$USER" == 0 && "$SYSTEM" == 0 ]]; then
  die "no hay ninguna instalación de Bottles. Ejecuta 01-instalar-bottles-flatpak.sh"
fi

if [[ "$DRY" == 1 ]]; then
  step "Simulación"
  if [[ "$USER" == 0 ]]; then
    info "flatpak remote-add --user --if-not-exists flathub …"
    info "flatpak install --user -y flathub $ETOL_BOTTLES_ID"
  fi
  info "flatpak override --user … $ETOL_BOTTLES_ID"
  if [[ "$SYSTEM" == 1 ]]; then
    info "La copia de sistema NO se toca. Para quitarla:"
    info "  sudo flatpak uninstall --system $ETOL_BOTTLES_ID"
  fi
  exit 0
fi

if [[ "$SYSTEM" == 1 && "$USER" == 0 ]]; then
  step "Instalando la copia de usuario"
  cat <<'EOF'
  Bottles estaba instalado para todo el sistema. ETOL necesita la copia de
  usuario porque:

    - se actualiza sin permisos de administrador,
    - sus permisos (Steam, HF8) se aplican solo a tu usuario,
    - las botellas y los datos viven en ~/.var/app/com.usebottles.bottles,
      así que la copia de usuario reutiliza todo lo que ya tienes.

  La copia de sistema no se borra en este paso.
EOF
  ensure_flathub_user
  flatpak install --user -y flathub "$ETOL_BOTTLES_ID"
  flatpak_user_installed || die "no se pudo instalar la copia de usuario."
  info "copia de usuario instalada."
fi

if [[ "$SYSTEM" == 1 ]]; then
  step "Copias instaladas"
  cat <<EOF
  Conviven dos instalaciones de Bottles:

    usuario  (la que usará ETOL)
    sistema  (la antigua, ya no hace falta)

  Las botellas, los runners y la configuración están en:
    ~/.var/app/$ETOL_BOTTLES_ID

  Cuando quieras quitar la de sistema (pide contraseña):

    sudo flatpak uninstall --system $ETOL_BOTTLES_ID

  Este script no lo hace solo para no tocar nada del sistema sin avisar.
EOF
fi

apply_bottles_permissions
print_permissions

step "Listo"
info "Bottles Flatpak queda en modo usuario y con los permisos de ETOL."
info "Cierra Bottles si estaba abierto y vuelve a abrirlo."

#!/usr/bin/env bash
# Aplica los permisos que necesita ETOL a Bottles Flatpak.
#
# En CachyOS/Arch lo normal es una copia de usuario. En Bazzite (Atomic)
# Bottles viene como Flatpak de sistema: también es válido y no hace falta
# instalar una copia de usuario solo para los permisos.
#
#   ./02-bottles-flatpak-a-usuario.sh
#   ./02-bottles-flatpak-a-usuario.sh --dry-run
#
# Qué hace:
#   1. Detecta si la copia activa es de usuario o de sistema.
#   2. Aplica `flatpak override` al ámbito correcto: `--user` o `--system`.
#      Con Bottles de sistema pide autorización con pkexec (o sudo).
#   3. Verifica que home, host-etc, Steam y devices=all quedaron aplicados.
#
# No instala ni borra copias de Bottles. No toca botellas, juegos ni prefijos.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib-bottles-flatpak.sh
source "$HERE/lib-bottles-flatpak.sh"

DRY=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    -h|--help)
      sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      die "opción desconocida: $arg"
      ;;
  esac
done

have_flatpak || die "no se encontró flatpak; instálalo con: $(package_install_hint flatpak)"

progress 5 "Comprobando el estado de Bottles"
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

SCOPE="$(flatpak_scope)"
if [[ "$SCOPE" == "none" ]]; then
  die "no se pudo determinar el ámbito activo de Bottles."
fi

if [[ "$DRY" == 1 ]]; then
  step "Simulación"
  info "ámbito activo: $SCOPE"
  info "flatpak override $(flatpak_scope_arg) --nofilesystem=host --nofilesystem=host-os \\"
  info "  --filesystem=home --filesystem=host-etc --filesystem=xdg-data/Steam \\"
  info "  --filesystem=~/.steam --device=all … $ETOL_BOTTLES_ID"
  if [[ "$SCOPE" == "system" ]]; then
    info "Se pedirá autorización de administrador (pkexec o sudo)."
  fi
  exit 0
fi

if [[ "$SCOPE" == "system" && "$USER" == 0 ]]; then
  step "Bottles de sistema"
  cat <<'EOF'
  Bottles está instalado para todo el sistema. En sistemas Atomic como
  Bazzite esto es lo normal y ETOL puede usarlo sin instalar una copia de
  usuario:

    - las botellas y los datos siguen en ~/.var/app/com.usebottles.bottles,
    - los permisos se aplican al Flatpak de sistema con autorización,
    - no se duplica la aplicación ni se toca la copia del sistema.
EOF
fi

if [[ "$SYSTEM" == 1 && "$USER" == 1 ]]; then
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

progress 85 "Aplicando permisos"
apply_bottles_permissions
progress 95 "Comprobando permisos"
print_permissions

progress 100 "Listo"
step "Listo"
info "Bottles Flatpak queda con los permisos de ETOL en el ámbito $SCOPE."
info "Cierra Bottles si estaba abierto y vuelve a abrirlo."

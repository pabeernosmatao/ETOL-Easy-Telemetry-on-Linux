# ETOL 0.3.23

Un arreglo para el pad HF8: acceso a los puertos serie USB sin comandos
manuales.

## Acceso USB del HF8

En una instalación limpia, SimHub podía ver el HF8 pero no abrirlo. El motivo
habitual es que el usuario no pertenece al grupo propietario de
`/dev/ttyACM*` y `/dev/ttyUSB*`:

- Arch/CachyOS: `uucp`.
- Debian, Ubuntu, Fedora: `dialout`.

Además, `brltty` puede reclamar dispositivos USB CDC-ACM (como el HF8).

La pestaña **Dispositivos** tiene ahora una tarjeta **Acceso a dispositivos
USB** como primera ficha y plegada por defecto. La cabecera muestra el estado
(«Correcto» o «Revisar») sin ocupar espacio. Al desplegarla se ve:

- el grupo serie detectado y si el usuario pertenece a él;
- el estado de `brltty` (no instalado, enmascarado o activo);
- el estado de la regla udev;
- los puertos serie detectados y si el usuario puede abrirlos.

El botón **Preparar acceso USB** pide autorización con `pkexec` y:

1. añade el usuario al grupo que posee los puertos serie (`uucp` o `dialout`);
2. instala `/etc/udev/rules.d/99-etol-serial.rules` con `TAG+="uaccess"` para
   `ttyACM*` y `ttyUSB*`, de modo que el acceso funciona sin depender del grupo;
3. enmascara `brltty-udev.service` y `brltty.service` si están instalados;
4. recarga y aplica las reglas udev.

La regla udev da acceso inmediato al usuario de la sesión. El grupo es el
respaldo persistente: si se añade, hay que cerrar sesión o reiniciar para que
la sesión lo reciba.

En el equipo de pruebas el único cambio necesario fue añadir el usuario a
`uucp`: `brltty` no estaba instalado y la regla de `ttyUSB` que se usaba para
los LEDs no afecta al HF8, que es `ttyACM`.

## Autocierre al cerrar el juego

El vigilante daba por abiertos algunos juegos después de cerrarlos. Proton deja
un proceso `steam.exe` cuyo primer argumento es
`c:\windows\system32\steam.exe` y que lleva la ruta del juego en la línea de
comandos; la detección lo confundía con el ejecutable real.

Ahora solo cuenta el proceso cuyo primer argumento (o `comm`) es el ejecutable
del juego. El `steam.exe` del prefijo ya no impide que se paren el puente,
CrewChief y SimHub al cerrar el simulador.

Tambien se corrigio el autocierre de LMU: el nombre del puente ahora incluye
`.exe` (antes no se reconocia el proceso) y la ventana de arranque manual ya no
impide cerrar CrewChief y SimHub si el juego se cierra dentro de esos 90 s.

Ademas, al arrancar ETOL se paran los puentes que hayan quedado de una sesion
anterior si su juego no esta abierto.

## Diagnóstico

La pestaña **Diagnóstico** incluye una comprobación **Acceso USB**. Si el
usuario no puede abrir el dispositivo o `brltty` puede reclamarlo, aparece un
aviso con el arreglo.

## Notas de la versión

- Nueva tarjeta en Dispositivos y nuevo comando `fix_usb_access`.
- Nuevo módulo `usbaccess` con la detección y el arreglo.
- Traducciones en español, catalán e inglés.
- Guías actualizadas en los tres idiomas.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.23-linux-x86_64-portable.AppImage` | 6.6 MB | `e9019519b70b99a5cfaf4d78420fc689602fe442bb6b9d1ad2bc795de859cf03` |
| `ETOL-0.3.23-linux-x86_64-full.AppImage` | 112 MB | `13f76a9bb56eb674ff673ecb42521f28e7cb06671d7365a3e73c6ba9b2d9b4b7` |

## Instalación

```bash
chmod +x ETOL-0.3.23-linux-x86_64-portable.AppImage
./ETOL-0.3.23-linux-x86_64-portable.AppImage
```

## Requisitos

- Linux x86_64, glibc **2.39+** (portable) o **2.44+** (full).
- `python3` y `curl` en el sistema.
- Portable: `libwebkit2gtk-4.1` + `libgtk-3`.
- `pkexec` y un agente polkit para aplicar el arreglo USB (KDE lo incluye).
- Steam con los juegos; Bottles con SimHub (ETOL puede instalar y preparar
  ambos).
- KDE Plasma para los dashboards sobre el juego.
- Un host StatusNotifier para el icono de bandeja.

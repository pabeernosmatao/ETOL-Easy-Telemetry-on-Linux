# ETOL · Guía de uso

[English](GUIDE.md) · **Español** · Versión 0.3.22

Todo lo que hace ETOL, cómo preparar SimHub y CrewChief desde cero y cómo
sacarle partido a cada simulador.

---

## Índice

1. [Lo primero](#1-lo-primero)
2. [La ventana](#2-la-ventana)
3. [La pestaña Juegos](#3-la-pestaña-juegos)
4. [Jugar](#4-jugar)
5. [Bottles, SimHub y CrewChief desde cero](#5-bottles-simhub-y-crewchief-desde-cero)
6. [Simuladores compatibles](#6-simuladores-compatibles)
7. [Escritorio: qué funciona en cada uno](#7-escritorio-qué-funciona-en-cada-uno)
8. [Pad HF8](#8-pad-hf8-opcional)
9. [La bandeja](#9-la-bandeja)
10. [Logs y diagnóstico](#10-logs-y-diagnóstico)
11. [Problemas frecuentes](#11-problemas-frecuentes)
12. [Dónde está cada cosa](#12-dónde-está-cada-cosa)

---

## 1. Lo primero

ETOL es una única aplicación: detecta tus simuladores de Steam, prepara SimHub
(y CrewChief si lo quieres) dentro de una botella de Bottles y lanza todo con un
clic. No modifica los juegos.

### Qué necesitas

- Linux x86_64 con **glibc 2.39 o superior**.
- `python3` (los puentes de telemetría) y `curl` (descargas).
- **Steam** con los juegos instalados.
- **Bottles** con una botella que tenga SimHub. Si no la tienes, ETOL la crea y
  la prepara por ti.
- Opcional: el pad **HF8** y KDE Plasma para los dashboards encima del juego.

### Abrir el AppImage

```bash
chmod +x ETOL-0.3.22-linux-x86_64-portable.AppImage
./ETOL-0.3.22-linux-x86_64-portable.AppImage
```

La variante **portable** usa el WebKit del sistema y necesita
`libwebkit2gtk-4.1` y `libgtk-3`. La variante **full** lo lleva todo dentro y
pide glibc 2.44 o superior.

### El primer arranque

La primera vez ETOL mira qué falta y muestra un aviso arriba:

- **Falta Bottles Flatpak**: pulsa el aviso y acepta. ETOL instala Bottles para
  tu usuario y le da los permisos mínimos (home, host-etc, bibliotecas de Steam
  y HF8).
- **Falta la botella de SimHub**: ETOL descarga GE-Proton, crea la botella
  `simhub`, instala las dependencias y abre los instaladores de CrewChief y
  SimHub.

Durante la instalación se abre una ventana con una **barra de progreso** y el
paso actual. Los instaladores de Windows son interactivos: pulsa «Siguiente» y
desmarca lo que se indica en el paso 5.

---

## 2. La ventana

| Pestaña | Para qué sirve |
|---|---|
| **Juegos** | Una ficha por simulador instalado: estado, puerto, prefijo, botones de lanzar/apps y el engranaje de configuración. |
| **Aplicaciones** | SimHub y CrewChief, con sus procesos, la botella elegida y las rutas. |
| **Dispositivos** | El pad HF8, su puerto COM y el enlace dentro de la botella. |
| **Diagnóstico** | Comprobaciones con el arreglo exacto al lado. |
| **Opciones** | Idioma, vigilancia automática, apps por juego, dashboards, logs, USB y pantallas. |

El idioma por defecto es **Automático**: usa español o catalán si el sistema
está en esos idiomas y, para el resto, inglés.

---

## 3. La pestaña Juegos

Cada simulador instalado tiene su ficha:

- **Puerto**: libre, ocupado o en uso por el puente activo.
- **Prefijo**: el prefijo de Proton del juego, o el plugin en los juegos nativos.
- **Arranque**: el lock que usa ETOL para saber que el puente está en marcha.

### El engranaje

Configura ese juego: copia plugins, ajusta SimHub, CrewChief y los puentes. Está
en **rojo** cuando el juego nunca se ha configurado o cuando ETOL ve algo mal.
Púlsalo una vez por juego.

### Los botones

- **Lanzar**: arranca el puente y las apps del juego. Si ya está en marcha, se
  convierte en **Parar**.
- **CrewChief** / **SimHub**: abren o cierran solo esa aplicación.

En los juegos que solo usan SimHub (ETS2 y BeamNG.drive) los dos botones ocupan
**la mitad cada uno**.

---

## 4. Jugar

### Vigilancia automática (recomendada)

Con la vigilancia activa (Opciones → Vigilancia automática) solo tienes que
abrir el simulador. ETOL lo detecta, levanta su puente y abre las apps elegidas.
Al cerrar el juego lo para todo. Si cambias de juego, cambia solo.

### Lanzamiento manual

Desactiva la vigilancia y usa los botones de la ficha o el menú de la bandeja.

ETOL tiene que seguir abierto para vigilar. Puedes dejarlo en la bandeja: al
cerrar la ventana sigue en segundo plano.

Al salir de ETOL se paran los puentes, se cierran las apps que abrió y se
desactiva la regla de KWin de los dashboards.

---

## 5. Bottles, SimHub y CrewChief desde cero

1. **Instala Bottles** como Flatpak de usuario si no está. No usa `sudo`.
2. **Crea la botella `simhub`** con prefijo Windows 11 de 64 bits y GE-Proton.
   Si ya existe, la reutiliza.
3. **Prepara el prefijo** con `dotnet48`, `vcrun2022`, `d3dcompiler_43`,
   `d3dx9` y `corefonts` mediante winetricks. Puede tardar varios minutos.
4. **Descarga y abre los instaladores**: CrewChief (MSI) y SimHub (ZIP).

> **En los instaladores:** en CrewChief, al terminar **no marques** «Run
> CrewChiefV4». En SimHub, desmarca *Install .NET Runtime*, *Install USB Display
> Drivers* e *Install Visual C++ Redistributables*, y no lo ejecutes al final.

> **CrewChief** se descarga del espejo oficial y puede ser una versión anterior:
> ábrelo una vez después y acepta la actualización antes de usarlo con los juegos.

---

## 6. Simuladores compatibles

| Juego | Telemetría | Apps | Notas |
|---|---|---|---|
| Le Mans Ultimate | memoria compartida | SimHub + CrewChief | |
| Automobilista 2 | memoria compartida | SimHub + CrewChief | |
| Assetto Corsa EVO | memoria compartida | SimHub + CrewChief | |
| Assetto Corsa | memoria compartida | SimHub + CrewChief | |
| rFactor 2 | memoria compartida | SimHub + CrewChief | incluye el plugin SMMP |
| RaceRoom Racing Experience | memoria compartida (`$R3E`) | SimHub + CrewChief | espejo crudo por UDP 8777 |
| DiRT Rally 2.0 | UDP (relay 20777) | SimHub + CrewChief | `extradata=3` |
| Euro Truck Simulator 2 | Linux nativo, `/dev/shm` | SimHub | instala el plugin SCS |
| BeamNG.drive | Linux nativo, OutGauge UDP | SimHub | |

Los puentes son de solo lectura: leen la telemetría y la publican para SimHub.
Nunca escriben en el juego.

---

## 7. Escritorio: qué funciona en cada uno

| Función | KDE Plasma | GNOME | niri / otros Wayland |
|---|---|---|---|
| App, detección y telemetría | Sí | Sí | Sí |
| Dashboards sobre el juego | **Sí**, regla de KWin | No | No |
| ¿Hace falta gamescope? | **No: funciona con o sin gamescope** | No aplica | No aplica |
| Icono de bandeja | Nativo | Extensión AppIndicator | Barra con bandeja |
| Cerrar a la bandeja | Sí | Con la extensión | Con la barra |

Los dashboards sobre el juego son una **regla de KWin** que coloca la ventana de
SimHub encima del simulador. En Plasma funciona **con o sin gamescope**; no es
necesario usar gamescope. GNOME y niri no implementan esa regla, así que allí
ETOL ofrece telemetría y apps, pero no el overlay.

ETOL instala solo un **icono de grupo** para SimHub y CrewChief (CrewChief
arriba, SimHub abajo) y su entrada `.desktop` para la clase de ventana
`steam_proton`, de modo que el panel muestra un único icono con las dos mitades
en lugar del icono de CrewChief. Se crea al arrancar; no hay que ejecutar nada a
mano.

---

## 8. Pad HF8 (opcional)

El HF8 (ForceFeel) es un pad háptico USB. ETOL lo detecta, enlaza su puerto COM
dentro de la botella y escribe las claves que necesita SimHub. Sin él ETOL
funciona igual y el diagnóstico solo muestra un aviso.

En **Dispositivos** puedes activarlo, ver el puerto y reconfigurarlo.

---

## 9. La bandeja

ETOL usa **StatusNotifierItem** (el estándar de bandeja de Linux), así que el
icono lo pinta tu escritorio: Plasma de forma nativa, la barra de niri si tiene
módulo de bandeja, o GNOME con la extensión AppIndicator.

El menú tiene: Mostrar ventana, un submenú por simulador (Lanzar / Parar todo),
Aplicaciones (abrir/cerrar CrewChief y SimHub) y Salir.

Si no hay host de bandeja, ETOL abre igual y lo deja en el log; en ese caso
cerrar la ventana cierra la aplicación.

---

## 10. Logs y diagnóstico

En **Diagnóstico** tienes las comprobaciones con el arreglo al lado. En
**Opciones** puedes activar la captura de logs y elegir la retención.

Los logs viven en `~/.config/etol/logs/`:

- `etol-AAAA-MM-DD.log`: eventos de la app.
- `setup-*.log` y `config-*.log`: salida completa de instalaciones y
  configuraciones.
- `run-AAAA-MM-DD/`: copia de los logs del puente y de SimHub/CrewChief.

Si algo falla, envía la carpeta de logs completa. `recoger-logs.sh` la empaqueta
en un `.tar.gz`.

---

## 11. Problemas frecuentes

**No aparece ningún simulador.** Comprueba que Steam los tiene instalados y
pulsa **Redetectar**. ETOL lee todas las bibliotecas de Steam, también las de
otros discos.

**El engranaje sigue rojo.** Ese juego no se ha configurado o ETOL ve algo mal.
Pulsa el engranaje y mira el diagnóstico.

**SimHub no arranca.** Revisa **Aplicaciones** y el diagnóstico; si la botella
no tiene SimHub, vuelve a preparar el prefijo.

**No hay icono de bandeja.** Tu escritorio no tiene host StatusNotifier. En
GNOME instala la extensión AppIndicator; en niri añade un módulo de bandeja.

**El AppImage no arranca.** La portable necesita `libwebkit2gtk-4.1` y
`libgtk-3`. Con glibc 2.44 o superior prueba la **full**. Sin FUSE2:
`./ETOL-*.AppImage --appimage-extract-and-run`.

**Los dashboards no salen encima del juego.** Es exclusivo de KDE Plasma (regla
de KWin). En Plasma no hace falta gamescope.

**Quiero empezar de cero.** Cierra ETOL y borra `~/.config/etol`. Las botellas
de Bottles y los juegos no se tocan.

---

## 12. Dónde está cada cosa

| Qué | Dónde |
|---|---|
| Configuración | `~/.config/etol/config.toml` |
| Scripts y puentes | `~/.config/etol/payload/` |
| Logs | `~/.config/etol/logs/` |
| Entorno generado | `~/.config/etol/generated/entorno.sh` |
| Botellas, runners y prefijos | `~/.var/app/com.usebottles.bottles/` |
| Descargas de instaladores | `~/Descargas/ETOL/` |

ETOL no guarda nada dentro de la carpeta de Steam ni de los juegos, salvo los
plugins que el usuario pide instalar (SCS en ETS2 y SMMP en rFactor 2 y
Le Mans Ultimate) y los ficheros de configuración que cada juego espera.

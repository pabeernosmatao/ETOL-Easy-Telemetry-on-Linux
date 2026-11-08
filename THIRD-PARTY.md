# Componentes de terceros

ETOL se distribuye bajo licencia **MIT** (ver `LICENSE`). Ese permiso cubre el
código propio del proyecto: la aplicación Tauri, el núcleo Rust, la interfaz y
los scripts de `scripts/`.

El AppImage y el payload incluyen además componentes de terceros que mantienen
**sus propias licencias**. Esta página los recoge.

---

## 1. Plugin SCS para ETS2 (`libscs-telemetry.so`)

| | |
|---|---|
| Fichero | `vendor/libscs-telemetry.so` (se copia a `bridge/libscs-telemetry.so`) |
| SHA-256 | `ca18a9b32b99c79942f5cdd35e0236b070a5ff10fe3d17b3edf8ac2436b2aecc` |
| Licencia | **MIT** |
| Copyright | © 2014 Hans |
| Origen | `truckermudgeon/scs-sdk-plugin` (fork de `RenCloud/scs-sdk-plugin`, a su vez basado en `nlhans/ets2-sdk-plugin`) |
| Texto | `vendor/libscs-telemetry.LICENSE.txt` |

Se instala en `bin/linux_x64/plugins/` del juego cuando el usuario lo pide desde
ETOL. Solo lee la telemetría del SDK de SCS.

---

## 2. rFactor 2 Shared Memory Map Plugin (`rFactor2SharedMemoryMapPlugin64.dll`)

| | |
|---|---|
| Fichero | `vendor/rFactor2SharedMemoryMapPlugin64.dll` |
| SHA-256 | `9d98d77b767812dca5afeb6663f486b3ad5be090c3e10783f36bc73a470ab5e6` |
| Licencia | **GNU GPL v3** |
| Origen | `TheIronWolfModding/rF2SharedMemoryMapPlugin` (https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin) |
| Texto | `vendor/rFactor2SharedMemoryMapPlugin64.LICENSE.txt` |

El plugin se distribuye **sin modificar** dentro del payload (y por tanto del
AppImage). ETOL lo copia a la carpeta del juego cuando el usuario configura
rFactor 2 o Le Mans Ultimate. Su código fuente está disponible en el repositorio
indicado.

Si redistribuyes ETOL o el AppImage, debes:

- conservar el texto de la GPL v3 que acompaña al plugin,
- mantener el aviso de que el plugin es GPL v3 y enlazar su código fuente.

### Plugin de Le Mans Ultimate (`LMU.3.8.SharedMemoryMapPlugin.dll`)

| | |
|---|---|
| Fichero | `vendor/LMU.3.8.SharedMemoryMapPlugin.dll` |
| SHA-256 | `3c79c9df5b132c833363aefbfb986b8e60cd35a403d887a114bf97b62d35c618` |
| Versión | 3.8.0.1 (`LMU Shared Memory Map Plugin`) |
| Licencia | **GNU GPL v3** |
| Origen | TheIronWolfModding (mismo proyecto que el plugin de rFactor 2) |
| Texto | `vendor/rFactor2SharedMemoryMapPlugin64.LICENSE.txt` |

CrewChief 4.18 y posteriores ya no incluyen este plugin. ETOL lleva una copia
**sin modificar** y la copia a `<juego>/Plugins/` al configurar Le Mans
Ultimate, para que el juego publique los buffers `$LMU.3.8.SMMP_*` que leen
SimHub, CrewChief y el espejo de ETOL. Se aplican las mismas obligaciones GPL v3
descritas arriba.

---

## 3. Tauri y bibliotecas del sistema

El ejecutable gráfico usa Tauri v2 y WebKitGTK. El AppImage completo empaqueta
estas bibliotecas; la variante `-linux-x86_64` usa las del sistema.

| Componente | Licencia |
|---|---|
| Tauri (`tauri`, `tauri-build`, `tauri-runtime`, `wry`, `tao`) | MIT o Apache-2.0 (dual) |
| WebKitGTK / JavaScriptCoreGTK | LGPL-2.1-or-later y BSD-2-Clause |
| GTK 3, GLib, GObject, Pango, GDK-Pixbuf | LGPL-2.1-or-later |
| libsoup 3 | LGPL-2.1-or-later |
| glibc | LGPL-2.1-or-later |
| Fuentes y utilidades de linuxdeploy | MIT y otras licencias libres |

La bandeja se implementa con **StatusNotifierItem sobre D-Bus** (crate `ksni`,
licencia Unlicense). ETOL no enlaza ni empaqueta `libappindicator`; el icono lo
pinta el propio escritorio si tiene un host SNI (Plasma, barras de niri con
bandeja, GNOME con la extensión AppIndicator). Si no lo hay, ETOL registra un
aviso y la ventana funciona igual.

Las licencias completas de cada paquete están en el propio sistema
(`/usr/share/doc/<paquete>/copyright`) o en el repositorio del proyecto.

---

## 4. Dependencias Rust y JavaScript

Las dependencias de Rust se resuelven con `Cargo.lock` y `src-tauri/Cargo.lock`:
todas son permisivas (MIT, Apache-2.0 o BSD). La interfaz no usa dependencias de
JavaScript en tiempo de ejecución; `node_modules` solo contiene la CLI de Tauri
(`@tauri-apps/cli`, MIT/Apache-2.0).

Puedes listar el detalle con:

```bash
cargo license            # si está instalado
cargo tree --format "{p} {l}"
```

---

## 5. Lo que **no** se distribuye

SimHub, CrewChief, Bottles, Steam, Proton y los propios simuladores **no** van
dentro de ETOL. El usuario los instala por su cuenta y ETOL solo los detecta,
configura y lanza. Sus licencias son las de cada fabricante.

---

## 6. Cómo cumplir al redistribuir

Si publicas un fork o un AppImage modificado:

1. Conserva `LICENSE` (MIT, con el aviso de copyright de ETOL).
2. Conserva `THIRD-PARTY.md` y los dos ficheros `*.LICENSE.txt` de `vendor/`.
3. No cambies la licencia del plugin de rF2: es GPL v3.
4. Si quitas el plugin de rF2 del paquete, indica dónde conseguirlo.

Para cualquier duda sobre compatibilidad de licencias, lo más seguro es
mantener las licencias originales de cada componente tal y como se distribuyen
aquí.

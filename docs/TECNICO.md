# ETOL · documentación técnica

Este documento recoge la parte técnica que antes vivía en el README: la CLI, las
claves de configuración, cómo detecta ETOL y cómo se compila.

## CLI

El AppImage incluye el binario `etol` al lado de la interfaz
(`usr/bin/etol`). Si compilas desde el repositorio:

```bash
./etol init          # detecta todo, guarda config y genera el entorno
./etol doctor        # comprueba que todo cuadra
./etol status        # estado en vivo
./etol launch ams2   # puente + CrewChief + SimHub
./etol stop ams2     # parar puente y cerrar SimHub/CrewChief
```

### `detect`

Escanea y muestra lo que encuentra, sin escribir nada. `--save` guarda el
resultado.

### `init`

`detect --save` + generación de `generated/entorno.sh`. Es lo primero que hay
que ejecutar en una instalación nueva.

### `config`

```bash
./etol config show
./etol config path
./etol config set apps.bottle /ruta/a/la/botella
./etol config set games.acevo.proton "/ruta/Proton-CachyOS Latest"
```

| Clave | Ejemplo |
|---|---|
| `apps.bottle` | `~/.local/share/bottles/bottles/simhub` |
| `apps.wine` | `.../runners/ge-proton11-11/files/bin/wine` |
| `apps.simhub_exe` | `.../SimHub/SimHubWPF.exe` |
| `apps.crewchief_exe` | `.../CrewChiefV4/CrewChiefV4.exe` |
| `bridge.app_dir` | carpeta de scripts (payload instalado) |
| `bridge.run_base` | `/tmp/etol` |
| `hf8.enabled` | `true` / `false` |
| `hf8.com_port` | `COM33` |
| `games.<id>.path` | ruta de instalación del juego |
| `games.<id>.prefix` | `compatdata/<appid>/pfx` |
| `games.<id>.proton` | carpeta de Proton |
| `games.<id>.wine` | binario `wine` |
| `games.<id>.exe` | ejecutable del juego |

Cada `config set` regenera el entorno automáticamente.

### `env`

Muestra `generated/entorno.sh`, el archivo con todas las rutas resueltas que
usan los scripts.

### `status`

Estado en vivo de apps, pad, juegos, puertos y locks.

### `doctor`

Comprobaciones con el arreglo exacto al lado. Códigos: `0` todo correcto,
`1` avisos, `2` fallos. `--fix` borra locks huérfanos.

### `launch` / `stop`

```bash
./etol launch lmu            # puente + apps
./etol launch lmu --puente   # solo el ciclo del puente
./etol launch lmu --dry-run  # muestra el comando, sin ejecutar
./etol stop lmu              # para el puente y cierra las apps del juego
./etol stop lmu --solo-puente
```

Juegos: `lmu`, `ams2`, `acevo`, `assetto`, `rf2`, `dirt2`, `ets2`, `beamng`.

### `hf8` y `setup`

```bash
./etol hf8                       # configura el pad en la botella
./etol setup prefix              # botella, dependencias e instaladores
./etol setup bottles-install     # Bottles Flatpak
./etol setup bottles-user        # pasar Bottles de sistema a usuario
```

## Detección

1. **Steam**: `.steam/steam`, `.local/share/Steam` y la versión Flatpak;
   bibliotecas de `libraryfolders.vdf`.
2. **Proton**: `config_info` de `compatdata/<appid>`, con respaldo en
   `compatibilitytools.d`.
3. **Bottles**: Flatpak de usuario, Flatpak de sistema y nativo; elige la
   botella que tenga SimHub.
4. **HF8**: USB `04d8:ee96` y el enlace de su COM en `dosdevices`.

## Estructura del proyecto

```
src/                 núcleo Rust (detección, configuración, lanzador, doctor)
src-tauri/           interfaz Tauri y comandos
ui/                  HTML, CSS y JavaScript (sin bundler)
scripts/juegos/*/    puentes y scripts por simulador
vendor/              binarios de los puentes y plugins de terceros
docs/                documentación
```

El payload (scripts y binarios) viaja dentro del AppImage y se instala en
`~/.config/etol/payload`. El repositorio incluye una copia ya generada en
`src-tauri/payload/`, así que compila sin árboles externos.

## Compilar

```bash
./scripts/instalar-toolchain.sh   # Rust aislado en .toolchain/ (una vez)
npm install                       # CLI de Tauri (una vez)
./scripts/gui.sh build            # AppImage en target/release/bundle/appimage/
./scripts/empaquetar-portable.sh  # variante que usa el WebKit del sistema
./scripts/preparar-release.sh     # los dos con el nombre final, en dist/
```

`preparar-release.sh` deja los assets listos para subir:

- `ETOL-<version>-linux-x86_64-portable.AppImage` (WebKit del sistema)
- `ETOL-<version>-linux-x86_64-full.AppImage` (todo incluido)

`amd64` y `x86_64` son la misma arquitectura; el sufijo lo pone Tauri.

La bandeja usa `ksni` (StatusNotifierItem por D-Bus), no `libappindicator`. Eso
sube el mínimo de Rust a **1.80** y añade `zbus`/`tokio` a las dependencias.

## Configuración y logs

- Configuración: `~/.config/etol/config/config.toml`
- Payload instalado: `~/.config/etol/payload`
- Logs: `~/.config/etol/logs` (retención configurable, 1–7 días)
- Entorno generado: `~/.config/etol/generated/entorno.sh`

## Seguridad

- Solo lectura sobre juegos, prefijos, `/proc` y `/sys`.
- No escribe en memoria compartida ni en archivos de los juegos.
- No usa `sudo`.
- `doctor --fix` únicamente borra `arranque.lock` cuando ningún proceso lo tiene
  abierto.
- Permisos de Tauri mínimos (`core:default`) y CSP activa.

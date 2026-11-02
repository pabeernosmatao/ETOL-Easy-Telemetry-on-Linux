# Puentes de Le Mans Ultimate, Automobilista 2 y Assetto Corsa EVO

Cuatro puentes independientes, todos **de solo lectura** sobre la memoria
compartida oficial de cada juego. Ninguno modifica el juego, sus archivos ni su
memoria.

| Puente | Destino | Transporte |
|---|---|---|
| `lmu_shm_mirror.exe` | SimHub y CrewChief en el prefijo de Bottles | UDP `127.0.0.1:8772` |
| `ams2_shm_mirror.exe` | SimHub y CrewChief en el prefijo de Bottles | UDP `127.0.0.1:8773` |
| `acevo_shm_mirror.exe` | SimHub y CrewChief en el prefijo de Bottles | UDP `127.0.0.1:8774` |
| `lmu_shm_bridge.exe` | dashboards nativos (`telemetry_daemon.py`) | UDP `127.0.0.1:8771` (JSON) |

Este documento cubre primero el **espejo LMU**, después el **espejo AMS2**,
después el **espejo AC EVO** y al final el puente JSON.

---

# 1. Espejo LMU → Bottles

## Por qué

`LMU_Data` y los `$LMU.3.8.SMMP_*` son named objects de Wine. Cada prefijo tiene
su propio `wineserver`, así que un proceso del prefijo de Bottles no puede abrir
la memoria de un proceso del prefijo de Steam. El espejo copia los buffers por
UDP loopback entre los dos prefijos.

```
LMU (GE-Proton10-34-LMU-hid_fixes, pfx de Steam 2399420)
  LMU_Data + $LMU.3.8.SMMP_*   (FILE_MAP_READ, lock oficial)
        │  emisor:  lmu_shm_mirror.exe --sender
        │  UDP 127.0.0.1:8772, fragmentado (30 Hz LMU_Data / 10 Hz SMMP)
        ▼
  receptor: lmu_shm_mirror.exe --receiver (prefijo de Bottles)
  publica LMU_Data + $LMU.3.8.SMMP_* + alias $rFactor2SMMP_*
        ▼
  SimHub / CrewChief
```

## Qué se espeja

| Buffer | Bytes |
|---|---:|
| `LMU_Data` | 327680 (copia 324824) |
| `$LMU.3.8.SMMP_Telemetry$` | 245760 |
| `$LMU.3.8.SMMP_Scoring$` | 77824 |
| `$LMU.3.8.SMMP_Rules$` | 49152 |
| `$LMU.3.8.SMMP_Extended$` | 12288 |
| `$LMU.3.8.SMMP_ForceFeedback$` | 4096 |
| `$LMU.3.8.SMMP_Graphics$` | 4096 |

- El receptor crea además los alias `$rFactor2SMMP_Telemetry$`, `Scoring`,
  `Rules`, `Extended`, `ForceFeedback` y `Graphics`. SimHub usa
  `CrewChiefV4.rFactor2_V2.RF2SharedMemoryReader`, que exige los seis; sin
  `ForceFeedback` o `Graphics` no inicializa.
- En la copia de `LMU_Data` se anulan `mVehicle` y `mResultsStream`: son
  punteros absolutos del proceso del juego, inválidos en otro proceso.
- Se publica `LMU_Data_Event` para que los lectores esperen datos.
- Los mappings se crean **antes** de arrancar el stub, para que SimHub
  encuentre los seis buffers al inicializar su lector.

## Qué NO se hace

- No se abren buffers de control (`HWControl`, `WeatherControl`,
  `RulesControl`, `PluginControl`, etc.).
- No se reenvía ninguna escritura de CrewChief hacia LMU.
- No se toca el juego, su prefijo, sus plugins ni sus archivos.

## Uso

```bash
cd $HOME/.config/etol/payload

./bridge/check_entorno.sh            # comprueba el entorno (no modifica nada)
./arrancar_todo_lmu.sh               # espejo -> CrewChief -> SimHub, con pausas
./activar_puente_lmu.sh              # solo el ciclo del espejo
./activar_puente_lmu.sh --no-config  # sin tocar SimHub/CrewChief
./activar_puente_lmu.sh --no-watch   # sin vigilante
./activar_puente_lmu.sh --verbose    # logs detallados
./desactivar_puente_lmu.sh           # para vigilante, stub y espejo
```

`arrancar_todo_lmu.sh` es idempotente (no toca apps ya abiertas), espera a la
ventana de cada app antes de seguir y comprueba que el espejo siga vivo tras
abrirla. Opciones: `--espera N` (defecto 25 s entre apps), `--espera-puente N`,
`--timeout N`, `--no-config`, `--dry-run`. Log: `arranque.log` en `RUN_DIR`.

Orden recomendado: activar el puente, arrancar LMU, CrewChief y SimHub. El
receptor debe existir antes que las apps; si no, crean sus propios mappings con
otro tamaño y el receptor no puede publicar. Al terminar, cerrar las apps y
desactivar.

`check_entorno.sh` valida prefijo de LMU, botella, runners, SimHub, CrewChief,
el enlace `steam`, los `.exe`, los flags de CrewChief y los procesos; devuelve 1
si falta algo imprescindible. Es lo primero que hay que ejecutar tras reinstalar
el sistema o mover carpetas (ver la tabla de reposición en el README raíz).

Qué hace `activar_puente_lmu.sh` por dentro:

1. Compila si falta el `.exe` (Zig; ver más abajo).
2. Ajusta SimHub y CrewChief con `configure_apps.py` (backups e idempotente).
3. Arranca el receptor y espera `Mappings iniciales creados`.
4. Arranca el emisor (o reutiliza el que ya estaba).
5. Arranca el stub `Le Mans Ultimate.exe`.
6. Arranca el vigilante.

No reinicia LMU, SimHub ni CrewChief.

## Stub de detección

`lmu_process_stub.cpp` se compila como un `.exe` inerte llamado
`Le Mans Ultimate.exe` y se ejecuta en el prefijo de Bottles desde
`/tmp/etol/lmu_mirror/`. SimHub y CrewChief lo usan para detectar que el
juego está en marcha. No abre el juego, no lee memoria y no tiene ventana.
Se para con `desactivar_puente_lmu.sh`, que solo mata procesos cuyo ejecutable
sea exactamente ese stub (nunca el `Le Mans Ultimate.exe` real del prefijo de
Steam).

## Vigilante

`lmu_bridge_watchdog.sh` comprueba cada 2 s que existen receptor, emisor y stub,
y relanza el que falte (`watchdog.log`). Es necesario porque al abrir CrewChief
o SimHub con otro runner, Wine reinicia el `wineserver` del prefijo y mata todos
sus procesos: el emisor seguía enviando UDP pero nadie publicaba los mappings, y
las apps dejaban de ver LMU sin ningún error visible.

Variables: `LMU_MIRROR_WATCH_INTERVAL` (segundos, 2 por defecto).

## Compilar

```bash
./bridge/build_mirror.sh   # lmu_shm_mirror.exe
./bridge/build_stub.sh     # lmu_process_stub.exe
```

Requiere Zig 0.15.x en `../tools/zig/zig` (o `ZIG=/ruta/zig`) y los headers del
SDK en `Support/SharedMemoryInterface/` de la instalación de LMU. El build
compila a un temporal y renombra, para poder recompilar con el espejo en marcha
(si no, falla con `text file busy`).

## Configuración de las apps

`configure_apps.py` escribe, con copia de seguridad en
`~/.local/share/lmu_mirror/backups/` (`LMU_MIRROR_BACKUP_DIR`):

- CrewChief, perfil JSON (`Documents/CrewChiefV4/Profiles/*.json`) y
  `CrewChiefV4.exe.config`:
  - `enable_lmu_rf2_plugin_dll_check=False` (los plugins viven en el prefijo de
    Steam; comprobarlos desde Bottles no aporta nada).
  - `use_legacy_rf2_shared_memory_filenames=False`.
  - `lmu_enable_pit_manager=False` (el pit manager escribe en la memoria del
    juego). Para conservarlo: `LMU_MIRROR_KEEP_PIT_MANAGER=1`.
  - `run_immediately=True`.
- SimHub: LMU visible en `HiddenGames.json` y `OrderEx=1` en
  `GameSettings.json`.

No modifica `lmu_install_path`, `GamePath` ni rutas de instalación: las rutas
las fijó el usuario y se respetan. SimHub y CrewChief apuntan a
`C:\Program Files (x86)\Steam\SteamApps\common\Le Mans Ultimate`, que en el
prefijo de Bottles resuelve por el enlace
`drive_c/Program Files (x86)/steam -> ~/.local/share/Steam`.

## Comprobar

```bash
pgrep -af 'lmu_shm_mirror|lmu_bridge_watchdog'
tail -f /tmp/etol/lmu_mirror/receiver.log   # "LMU_Data publicado (...)"
tail -f /tmp/etol/lmu_mirror/sender.log
cat /tmp/etol/lmu_mirror/watchdog.log
```

CrewChief (consola de su ventana): `Using game definition Le Mans Ultimate`,
`Creating buffer: $LMU.3.8.SMMP_*` y la sesión con circuito y pilotos. Su log se
escribe al cerrar la app (`Documents/CrewChiefV4/DebugLogs/console_*.txt`);
`console_flush=True` en el perfil lo escribe en vivo.

SimHub: `GameRunning=true` y `NewData` con datos en su WebSocket `8888`.

## Limitaciones

- Si el receptor arranca después de que SimHub/CrewChief hayan creado sus
  propios mappings, los tamaños no coinciden y el receptor no puede publicar
  (lo deja en `receiver.log`). Solución: reiniciar la app que los creó.
- Tras un reinicio del `wineserver` hay unos segundos sin datos hasta que el
  vigilante relanza el puente.
- SimHub en Wine puede caer en su watchdog (`PssCaptureSnapshot` no existe) y
  perder el lector del juego; hay que reiniciar SimHub.
- El espejo no implementa plugins WPF/WinForms ni ShakeIt: solo publica datos.

## Pad HF8 (ForceFeel)

El pad Next Level Racing HF8 es un CDC ACM (`04d8:ee96`) que Linux expone como
`/dev/ttyACM0`. `setup_hf8.sh` lo publica en la botella como **`COM33`**:

```bash
bridge/setup_hf8.sh          # tty por VID:PID + enlace + registro
bridge/setup_hf8.py ...      # generador del .reg (lo llama el .sh)
bridge/check_hf8.sh          # "HF8: COM33" si SimHub lo ve; devuelve 1 si no
```

Por qué no basta con un enlace `comNN`:

- SimHub busca el pad con `GetSerialPortFromPid(0xEE96)`, que recorre
  `SerialPort.GetPortNames()`. Solo aparecen los puertos de
  `HARDWARE\DEVICEMAP\SERIALCOMM`; un COM "inventado" como `COM40` se ignora
  aunque `dosdevices/com40` exista. `COM33` es el último puerto estándar de
  Wine y siempre está en la lista.
- SimHub lee el nombre de `DEVPKEY_NAME`
  (`Enum\USB\VID_04D8&PID_EE96&MI_00\<instancia>\Properties\{b725f130-...}\000A`)
  y espera `DEVPROP_TYPE_STRING` (**0x12**). Wine lo devuelve como `REG_SZ`
  (1) y SimHub lo descarta. El `.reg` fuerza el tipo con `hex(12):`; `wine reg
  add` no puede.
- Solo `MI_00` lleva `"ForceFeel Pad (COM33)"` en `FriendlyName`/`Name`. El
  padre y `MI_01` se quedan sin `(COMn)` para que `GetSerialPortFromPid` no
  devuelva una entrada sin puerto o un duplicado.

Verificación (sin abrir el puerto):

```bash
wine reg query 'HKLM\SYSTEM\CurrentControlSet\Enum\USB\VID_04D8&PID_EE96&MI_00' /s
# FriendlyName = ForceFeel Pad (COM33), Device Parameters\PortName = COM33
# Properties\{b725f130-...}\000A debe salir con tipo (null), no REG_SZ
```

Limitaciones:

- `SerialPort.Close()` se cuelga bajo Wine; no lo llames desde scripts. Si
  SimHub se queda colgado al cerrar el pad, es esa llamada.
- El COM se lee al abrir el pad. Si SimHub ya estaba abierto con otro COM,
  reinícialo cuando no estés en pista.
- Si el pad se mueve a otro puerto USB, Wine puede crear otro sufijo de
  instancia; el script aplica el `.reg` a todos y pueden aparecer varias
  entradas `COM33`, todas válidas.

---

# 1b. Espejo AMS2 → Bottles

## Por qué

`$pcars2$` es un named object de Wine del prefijo de AMS2. SimHub
(`PCarsSharedMemory.AMS2.Automobilista2Manager`) y CrewChief
(`CrewChiefV4.AMS2.AMS2SharedMemoryReader`) lo abren con
`MemoryMappedFile.OpenExisting("$pcars2$", ...)`, pero cada prefijo tiene su
propio wineserver. El espejo copia el mapping por UDP loopback.

```
AMS2 (Proton-GE, pfx de Steam 1066890)
  $pcars2$ (24.576 B, AMS2APIStruct 20.700 B)   FILE_MAP_READ
        │  emisor:  ams2_shm_mirror.exe --sender
        │  UDP 127.0.0.1:8773, 24 KB por frame, 60 Hz
        ▼
  receptor: ams2_shm_mirror.exe --receiver (prefijo de Bottles)
  crea/publica $pcars2$ (24.576 B)
        ▼
  SimHub / CrewChief
```

## Qué se espeja

| Mapping | Bytes | Detalle |
|---|---:|---|
| `$pcars2$` | 24576 | `AMS2APIStruct` 20700 B; lo abren SimHub y CrewChief |
| `$pcars$` | 8192 | no se espeja; los lectores de AMS2 no lo usan |

- El receptor crea `$pcars2$` con `PAGE_READWRITE`; el emisor lo abre con
  `FILE_MAP_READ`.
- `mSequenceNumber` está en el offset **7.320**. El emisor lee la secuencia antes
  y después del `memcpy` y solo envía si es par y no cambió; el receptor la marca
  impar antes de copiar y la publica par al final.
- No se publica ningún evento: los lectores de AMS2 hacen polling.

## Qué NO se hace

- No se abre `$pcars2$` con permisos de escritura en el prefijo de AMS2.
- No se reenvía ninguna escritura de CrewChief/SimHub hacia AMS2.
- No se toca el juego, su prefijo, sus plugins ni sus archivos.

## Uso

```bash
cd $HOME/.config/etol/payload

./arrancar_todo_ams2.sh               # espejo -> CrewChief -> SimHub, con pausas
./activar_puente_ams2.sh              # solo el ciclo del espejo
./activar_puente_ams2.sh --no-config  # sin tocar SimHub/CrewChief
./activar_puente_ams2.sh --no-watch   # sin vigilante
./activar_puente_ams2.sh --no-stub    # sin el stub AMS2AVX.exe
./activar_puente_ams2.sh --no-hf8     # sin configurar el pad HF8
./activar_puente_ams2.sh --verbose    # logs detallados
./desactivar_puente_ams2.sh           # para vigilante, stub y espejo
```

`arrancar_todo_ams2.sh` es idempotente (no toca apps ya abiertas), espera a la
ventana de cada app antes de seguir y comprueba que el espejo siga vivo tras
abrirla. Opciones: `--espera N` (defecto 25 s entre apps), `--espera-puente N`,
`--timeout N`, `--no-config`, `--no-stub`, `--no-hf8`, `--dry-run`. Log:
`arranque.log` en `RUN_DIR`.

Orden recomendado: activar el puente, arrancar AMS2, CrewChief y SimHub. Si las
apps ya estaban abiertas, sus lectores reintentan solos; el receptor crea el
mapping. No hace falta reiniciar las apps.

Qué hace `activar_puente_ams2.sh` por dentro:

1. Compila si falta el `.exe` (Zig; ver más abajo).
2. Ajusta SimHub y CrewChief con `configure_apps_ams2.py` (backups e idempotente;
   no toca apps abiertas).
3. Arranca el receptor y espera `Mappings iniciales creados`.
4. Configura el pad HF8 (`COM33`) para que SimHub lo vea.
5. Arranca el emisor (o reutiliza el que ya estaba).
6. Arranca el stub `AMS2AVX.exe`.
7. Arranca el vigilante.

No reinicia AMS2, SimHub ni CrewChief. El HF8 se refresca en cada activación
porque Bottles puede reasignar `COM33` a otro dispositivo serie (por ejemplo,
un adaptador CH341); LMU ya lo hacía y AMS2 ahora también. Si no tienes HF8,
usa `--no-hf8`.

## Stub de detección

`ams2_process_stub.cpp` se compila como un `.exe` inerte llamado `AMS2AVX.exe`
y se ejecuta en el prefijo de Bottles desde `/tmp/etol/ams2_mirror/`.
CrewChief lo usa para detectar que el juego está en marcha: su `GameDefinition`
de AMS2 declara los nombres de proceso `AMS2` y `AMS2AVX`. No abre el juego, no
lee memoria y no tiene ventana. Se para con `desactivar_puente_ams2.sh`, que
solo mata procesos cuyo ejecutable sea exactamente ese stub (nunca el
`AMS2AVX.exe` real del prefijo de Proton).

SimHub no necesita el stub: su `Automobilista2Manager` abre `$pcars2$` y su
entrada `Automobilista2` está visible con `OrderEx=1`.

## Vigilante

`ams2_bridge_watchdog.sh` comprueba cada 2 s que existen receptor, emisor y
stub, y relanza el que falte (`watchdog.log`). Es necesario porque al abrir
CrewChief o SimHub con otro runner, Wine puede reiniciar el `wineserver` del
prefijo y matar todos sus procesos.

Variables: `AMS2_MIRROR_WATCH_INTERVAL` (segundos, 2 por defecto),
`AMS2_MIRROR_STUB=0` para no relanzar el stub.

## Compilar

```bash
./bridge/build_ams2_mirror.sh   # ams2_shm_mirror.exe
./bridge/build_ams2_stub.sh     # ams2_process_stub.exe
```

Requiere Zig 0.15.x en `../tools/zig/zig` (o `ZIG=/ruta/zig`). No necesita el
SDK de LMU: el espejo AMS2 trabaja con el mapping crudo. El build compila a un
temporal y renombra, para poder recompilar con el espejo en marcha.

## Configuración de las apps

`configure_apps_ams2.py` escribe, con copia de seguridad en
`~/.local/share/ams2_mirror/backups/` (`AMS2_MIRROR_BACKUP_DIR`):

- CrewChief: `last_game_definition=AMS2` y `AMS2` en
  `limit_available_games`, solo si CrewChief está cerrado (si está abierto, la
  app reescribe su configuración al salir).
- SimHub: `Automobilista2` visible en `HiddenGames.json` y `OrderEx=1` en
  `GameSettings.json`; no cambia `CurrentReader` (debe ser `null` para usar
  memoria compartida).

No modifica ninguna opción de LMU.

## Comprobar

```bash
pgrep -af 'ams2_shm_mirror|ams2_bridge_watchdog'
tail -f /tmp/etol/ams2_mirror/receiver.log   # "$pcars2$ publicado (24576 bytes)"
tail -f /tmp/etol/ams2_mirror/sender.log
cat /tmp/etol/ams2_mirror/watchdog.log
```

En Bottles, `tasklist` debe mostrar `AMS2AVX.exe` (stub), `ams2_shm_mirror.exe`,
`CrewChiefV4.exe` y `SimHubWPF.exe`. SimHub escribe en su log
`Active car model changed` / `Active track changed` / `Game connected`.
CrewChief escribe `Initialised ams2 shared memory` en su consola; su log puede
estar bufferizado hasta que se cierra la app.

## Limitaciones

- Solo se espeja `$pcars2$`; no se publica `$pcars`.
- Si AMS2 cambia el tamaño del struct o el offset de `mSequenceNumber`, hay que
  actualizar `kStructSize`/`kSeqOffset` en `ams2_shm_mirror.cpp`.
- El receptor crea el mapping; si cambias `--size`, para el puente y cierra las
  apps que lo tengan abierto antes de volver a activar (el mapping no se puede
  redimensionar).
- El stub no tiene ventana; si una app exigiera título de ventana además del
  proceso, habría que ampliarlo.

---

# 1c. Espejo AC EVO → Bottles

## Por qué

AC EVO publica tres named objects de Wine en su prefijo de Steam/Proton:

| Mapping | Bytes | Struct | `packetId` |
|---|---:|---|---|
| `Local\acevo_pmf_physics` | 4096 | `SPageFilePhysics` | offset 0 |
| `Local\acevo_pmf_graphics` | 8192 | `SPageFileGraphicEvo` | offset 0 |
| `Local\acevo_pmf_static` | 4096 | `SPageFileStaticEvo` | no |

SimHub (`ACSharedMemory.ACEVOManager`) y CrewChief los abren con
`MemoryMappedFile.OpenExisting`, pero cada prefijo tiene su propio wineserver.
El espejo copia los tres mappings por UDP loopback.

```
AC EVO (Proton-CachyOS, pfx de Steam 3058630)
  acevo_pmf_physics (4 KB) / graphics (8 KB) / static (4 KB)   FILE_MAP_READ
        │  emisor:  acevo_shm_mirror.exe --sender
        │  UDP 127.0.0.1:8774, 16 KB por frame, 60 Hz
        ▼
  receptor: acevo_shm_mirror.exe --receiver (prefijo de Bottles)
  crea/publica los tres mappings
        ▼
  SimHub / CrewChief
```

## Qué se espeja

- Los tres mappings completos. No se publica ningún otro mapping de AC EVO.
- El receptor crea los mappings con `PAGE_READWRITE`; el emisor los abre con
  `FILE_MAP_READ`.
- En `physics` y `graphics`, `packetId` está en el offset 0. El emisor lee el
  `packetId` antes y después del `memcpy` y solo envía si no cambió; el receptor
  copia el resto y publica `packetId` al final, para que un lector que compara
  paquetes no acepte una copia a medias.
- `static` no tiene `packetId`; se copia completo. Sus cambios son poco
  frecuentes (sesión, coche, pista, clima).
- No se publica ningún evento: los lectores de AC EVO hacen polling.

## Qué NO se hace

- No se abre ningún `acevo_pmf_*` con permisos de escritura en el prefijo de AC
  EVO.
- No se reenvía ninguna escritura de CrewChief/SimHub hacia AC EVO.
- No se toca el juego, su prefijo, sus plugins ni sus archivos.

## Uso

```bash
cd $HOME/.config/etol/payload

./arrancar_todo_acevo.sh               # espejo -> CrewChief -> SimHub, con pausas
./activar_puente_acevo.sh              # solo el ciclo del espejo
./activar_puente_acevo.sh --no-config  # sin tocar SimHub/CrewChief
./activar_puente_acevo.sh --no-watch   # sin vigilante
./activar_puente_acevo.sh --no-stub    # sin el stub AssettoCorsaEVO.exe
./activar_puente_acevo.sh --verbose    # logs detallados
./desactivar_puente_acevo.sh           # para vigilante, stub y espejo
```

`arrancar_todo_acevo.sh` es idempotente (no toca apps ya abiertas), espera a la
ventana de cada app antes de seguir y comprueba que el espejo siga vivo tras
abrirla. Opciones: `--espera N` (defecto 25 s entre apps), `--espera-puente N`,
`--timeout N`, `--no-config`, `--no-stub`, `--dry-run`. Log: `arranque.log` en
`RUN_DIR`.

Orden recomendado: activar el puente, arrancar AC EVO, CrewChief y SimHub. Si
las apps ya estaban abiertas, sus lectores reintentan solos; el receptor crea
los mappings. No hace falta reiniciar las apps.

Qué hace `activar_puente_acevo.sh` por dentro:

1. Compila si falta el `.exe` (Zig; ver más abajo).
2. Ajusta SimHub y CrewChief con `configure_apps_acevo.py` (backups e
   idempotente; no toca apps abiertas).
3. Arranca el receptor y espera `Mappings iniciales creados`.
4. Arranca el emisor (o reutiliza el que ya estaba).
5. Arranca el stub `AssettoCorsaEVO.exe`.
6. Arranca el vigilante.

No reinicia AC EVO, SimHub ni CrewChief.

## Stub de detección

`acevo_process_stub.cpp` se compila como un `.exe` inerte llamado
`AssettoCorsaEVO.exe` y se ejecuta en el prefijo de Bottles desde
`/tmp/etol/acevo_mirror/`. CrewChief lo usa para detectar que el juego está
en marcha: su `GameDefinition` de AC EVO declara el proceso
`AssettoCorsaEVO`. No abre el juego, no lee memoria y no tiene ventana. Se para
con `desactivar_puente_acevo.sh`, que solo mata procesos cuyo ejecutable sea
exactamente ese stub (nunca el `AssettoCorsaEVO.exe` real del prefijo de
Proton).

SimHub conectó sin depender del stub: su `ACEVOManager` abre `acevo_pmf_*` y su
entrada `AssettoCorsaEVO` está visible con `OrderEx=1`. El stub es sobre todo
para la detección de proceso de CrewChief; SimHub también lo detecta si está
presente.

## Vigilante

`acevo_bridge_watchdog.sh` comprueba cada 2 s que existen receptor, emisor y
stub, y relanza el que falte (`watchdog.log`). Es necesario porque al abrir
CrewChief o SimHub con otro runner, Wine puede reiniciar el `wineserver` del
prefijo y matar todos sus procesos.

Variables: `ACEVO_MIRROR_WATCH_INTERVAL` (segundos, 2 por defecto),
`ACEVO_MIRROR_STUB=0` para no relanzar el stub.

## Compilar

```bash
./bridge/build_acevo_mirror.sh   # acevo_shm_mirror.exe
./bridge/build_acevo_stub.sh     # acevo_process_stub.exe
```

Requiere Zig 0.15.x en `../tools/zig/zig` (o `ZIG=/ruta/zig`). No necesita el
SDK de AC EVO: el espejo trabaja con los mappings crudos. El build compila a un
temporal y renombra, para poder recompilar con el espejo en marcha.

## Configuración de las apps

`configure_apps_acevo.py` escribe, con copia de seguridad en
`~/.local/share/acevo_mirror/backups/` (`ACEVO_MIRROR_BACKUP_DIR`):

- CrewChief: `last_game_definition=ASSETTO_EVO` y `ASSETTO_EVO` en
  `limit_available_games`, solo si CrewChief está cerrado (si está abierto, la
  app reescribe su configuración al salir).
- SimHub: `AssettoCorsaEVO` visible en `HiddenGames.json` y `OrderEx=1` en
  `GameSettings.json`; no cambia `CurrentReader` (debe ser `null` para usar
  memoria compartida).

No modifica ninguna opción de LMU ni AMS2. CrewChief exige reiniciarse para
cambiar entre circuit y rally; si está abierto en otro juego, selecciona
**Assetto Corsa EVO** cuando no estés en pista.

## Comprobar

```bash
pgrep -af 'acevo_shm_mirror|acevo_bridge_watchdog'
tail -f /tmp/etol/acevo_mirror/receiver.log   # "Local\acevo_pmf_* publicado"
tail -f /tmp/etol/acevo_mirror/sender.log
cat /tmp/etol/acevo_mirror/watchdog.log
```

En Bottles, `tasklist` debe mostrar `AssettoCorsaEVO.exe` (stub),
`acevo_shm_mirror.exe`, `CrewChiefV4.exe` y `SimHubWPF.exe`. SimHub escribe en
su log `Assetto Corsa EVO game process detected`, `Active car model changed`,
`Active track changed` y `Game connected`. CrewChief puede necesitar que
selecciones **Assetto Corsa EVO** en la app si estaba en LMU/AMS2; el cambio de
configuración automática solo se aplica con CrewChief cerrado.

## Limitaciones

- Solo se espejan los tres mappings `acevo_pmf_*`; no se publica ningún mapping
  adicional de AC EVO.
- Los tamaños observados (4096/8192/4096) y los nombres pueden cambiar con una
  actualización del juego; en ese caso hay que actualizar
  `kDefaultSizes`/`kMappingNames` en `acevo_shm_mirror.cpp`.
- El receptor crea los mappings; si cambias `--sizes`, para el puente y cierra
  las apps que los tengan abiertos antes de volver a activar (un mapping no se
  puede redimensionar).
- El stub no tiene ventana; si una app exigiera título de ventana además del
  proceso, habría que ampliarlo.

---

# 2. Puente JSON/UDP para dashboards nativos

Lector de solo lectura que envía snapshots JSON por UDP al daemon Linux.

```
LMU (Proton) ──LMU_Data──> lmu_shm_bridge.exe ──UDP 8771──> telemetry_daemon.py
```

## Compilar

```bash
./bridge/build_bridge.sh
# o:
LMU="/ruta/a/Le Mans Ultimate" ./bridge/build_bridge.sh
```

Comando equivalente:

```bash
tools/zig/zig c++ -target x86_64-windows-gnu -std=c++17 -O2 \
  -I"$HOME/.local/share/Steam/steamapps/common/Le Mans Ultimate/Support/SharedMemoryInterface" \
  -o bridge/lmu_shm_bridge.exe bridge/lmu_shm_bridge.cpp -lws2_32
```

## Ejecutar

Con LMU en marcha:

```bash
./bridge/run_bridge.sh                 # primer plano
nohup ./bridge/run_bridge_loop.sh > /tmp/etol/lmu_shm_bridge.log 2>&1 &
./bridge/stop_bridge.sh                # detener (no toca el juego)
```

Variables opcionales:

```bash
PROTON="$HOME/.local/share/Steam/compatibilitytools.d/GE-Proton10-34-LMU-hid_fixes" \
PFX="$HOME/.local/share/Steam/steamapps/compatdata/2399420/pfx" \
UDP=127.0.0.1:8771 HZ=30 ./bridge/run_bridge.sh
```

Snapshot único (sin dejar proceso; requiere frames de LMU):

```bash
./bridge/run_bridge.sh --once --no-udp --out /tmp/etol/shm_once.json
python3 -m json.tool /tmp/etol/shm_once.json | head
```

## Formato JSON (resumen)

```json
{
  "type": "lmu_shm",
  "v": 1,
  "source": "lmu_shm_bridge",
  "seq": 1234,
  "ts": 123456789,
  "game": {
    "track": "Michelin Raceway Road Atlanta",
    "session": 1,
    "sessionType": "Practice",
    "phase": 5,
    "currentET": 2278.0,
    "endET": 3605.0,
    "lapDist": 4083.454,
    "numVehicles": 54,
    "sessionBestLap": 70.921,
    "events": ["FFB", "UPDATE_TELEMETRY"],
    "generic": { "gameVersion": 14200, "appWidth": 7680, "appHeight": 1440 }
  },
  "player": {
    "id": 0, "name": "Javier Barbero", "car": "Alpine Endurance Team 2026 #35:LM",
    "class": "Hyper", "place": 36, "laps": 24, "bestLap": 77.2554,
    "lastLap": 99.5787, "inPits": false,
    "telemetry": { "speedKmh": 278.7, "gear": 6, "rpm": 8235.4, "fuel": 89.8, "...": "..." }
  },
  "vehicles": [
    { "id": 1, "name": "Sébastien Buemi", "place": 1, "class": "Hyper",
      "bestLap": 70.921, "lastLap": 72.093, "gap": 0.0, "inPits": false, "...": "..." }
  ]
}
```

- `player.telemetry` incluye todos los campos de `TelemInfoV01` y las 4 ruedas
  (`tempC`, `wear`, `pressureKpa`, `brakeTempC`, `tireLoad`, `camber`, ...).
- `vehicles` es compacto (31 campos) para standings/relative/track map.
- `game` incluye sesión, clima, banderas, tiempos y eventos.
- `seq` es monótono; el daemon lo usa para diagnosticar.

## Detalles técnicos

- Abre `LMU_Data` con `OpenFileMappingA` y `MapViewOfFile`.
- Usa `LMU_SharedMemoryLockData` / `LMU_SharedMemoryLockEvent` para leer bajo el
  lock oficial.
- Espera frames con `LMU_Data_HoldEvent` / `LMU_Data_DataEvent`.
- **Scoring:** `CopySharedMemoryObj` solo copia `scoring` cuando el evento
  `UPDATE_SCORING` está en la cola. En muchos frames solo llegan
  `FFB`/`UPDATE_TELEMETRY`, y `copied.scoring` queda vacío. El puente copia
  `scoringInfo` + `vehScoringInfo[0..numVehicles-1]` **directamente bajo el
  lock**, por eso los 54 coches están siempre disponibles.
- Solo copia la telemetría del jugador; los rivales van por scoring compacto.
- Envía a 30 Hz (configurable con `--hz`), ~37 KB por snapshot con 54 coches.
- El puente sale tras ~10 s sin frames (juego cerrado/menú); el daemon vuelve a
  SimHub WS como fallback.

## Evidencia

Rutas relativas a este `bridge/`:

- `../../evidencia/shm_bridge/shm_snapshot_once.json`: snapshot de 37 KB, 54 coches.
- `../../evidencia/shm_bridge/probe_shm2.txt`: prueba de que el scoring crudo está
  poblado y `CopySharedMemoryObj` lo dejó vacío en ese frame.
- `../../evidencia/shm_bridge/bridge.log`: log del puente en vivo.
- `../../evidencia/prototipo_nativo.md`: informe completo.

## Limitaciones

- Es específico de LMU/rFactor2 (estructuras del SDK oficial).
- Requiere ejecutarse bajo el mismo prefijo/wineserver que el juego.
- No sustituye a SimHub para plugins WPF/WinForms ni ShakeIt; esos requieren
  reimplementación nativa.

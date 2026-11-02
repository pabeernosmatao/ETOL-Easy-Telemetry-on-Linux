# SimHub Linux — telemetría nativa y Dash Studio

Prototipo de app nativa Linux para leer telemetría de simuladores y renderizar
dashboards/overlays tipo SimHub Dash Studio, sin depender de SimHub para los
datos de pista.

Estado actual: **Le Mans Ultimate funciona con memoria compartida nativa**
(telemetría + scoring de todos los coches, 62 en la última sesión), y el daemon
mantiene SimHub WebSocket como fuente de respaldo. El renderer `.djson`, el
dashboard simple y el relative nativo ya consumen la fuente nativa.

## Arquitectura

```
┌──────────────────────────────┐
│ Le Mans Ultimate (Proton)    │
│  LMU_Data + eventos + lock   │
└──────────────┬───────────────┘
               │ named shared memory (Wine)
┌──────────────▼───────────────┐
│ lmu_shm_bridge.exe (Win x64) │  solo lectura
│  telemetría jugador + scoring│
└──────────────┬───────────────┘
               │ UDP 127.0.0.1:8771 (JSON ~37 KB, 30 Hz)
┌──────────────▼───────────────┐
│ telemetry_daemon.py (Linux)  │
│  normaliza a nombres SimHub  │
│  fallback: SimHub WS 8888    │
│  sesión/clima: LMU REST 6397 │
└──────────────┬───────────────┘
               │ HTTP + SSE 0.0.0.0:8770 (LAN opcional)
┌──────────────▼───────────────┐
│ dash.html / relative.html    │
│ djson.html                   │
│  dashboards nativos          │
└──────────────────────────────┘
```

Los espejos para SimHub/CrewChief (LMU, AMS2 y AC EVO) son independientes de
este diagrama: copian la memoria compartida de cada juego al prefijo de Bottles
por UDP. Están descritos en las secciones siguientes.

## Inicio rápido

Con LMU ya en marcha:

> Para SimHub y CrewChief (Bottles) el camino es otro: ver
> [Puente para SimHub y CrewChief](#puente-para-simhub-y-crewchief-espejo-lmu--bottles).
> Lo de aquí es el puente JSON para los dashboards nativos.
>
> Para Automobilista 2, el puente equivalente es
> [Puente para Automobilista 2](#puente-para-automobilista-2-espejo-pcars2--bottles)
> (`./arrancar_todo_ams2.sh`).
>
> Para Assetto Corsa EVO, el puente equivalente es
> [Puente para Assetto Corsa EVO](#puente-para-assetto-corsa-evo-espejo-acevo_pmf_--bottles)
> (`./arrancar_todo_acevo.sh`).

```bash
cd $HOME/.config/etol/payload

# 1) compilar el puente una vez (Zig; no hace falta MinGW)
./bridge/build_bridge.sh

# 2) arrancar el puente en segundo plano (reintenta si LMU está en pausa/menú)
nohup ./bridge/run_bridge_loop.sh > /tmp/etol/lmu_shm_bridge.log 2>&1 &

# 3) arrancar el daemon (0.0.0.0 para acceder desde el móvil en la LAN)
nohup python3 telemetry_daemon.py --port 8770 --host 0.0.0.0 \
  > /tmp/etol/telemetry_daemon.log 2>&1 &

# 4) comprobar que SHM manda
curl -s http://127.0.0.1:8770/api/health
# {"ok":true,"simhub":true,"lmu":true,"shm":true,"source":"shm+lmu",...}
```

Abrir:

- Dashboard simple: <http://127.0.0.1:8770/>
- Relative nativo inspirado en `NEW_5_RELATIVE`:
  <http://127.0.0.1:8770/relative>
- Relative desde el móvil en la LAN:
  <http://192.168.1.170:8770/relative>
- Dash Studio `.djson`:
  <http://127.0.0.1:8770/djson?path=SimHub%20-%20SimpleDash%2FSimHub%20-%20SimpleDash.djson>

Si solo se quiere acceso local, arrancar el daemon sin `--host 0.0.0.0` (por
defecto escucha en `127.0.0.1`). Para la LAN, el cortafuegos debe permitir TCP
`8770` desde la subred local; en este equipo:

```bash
sudo ufw allow from 192.168.1.0/24 to any port 8770 proto tcp comment 'SimHub Linux LAN'
```

Detener el puente y su bucle de reintento (no toca el juego):

```bash
./bridge/stop_bridge.sh
```

El daemon, al no recibir SHM durante 2 s, vuelve automáticamente a SimHub WS.
Si tampoco hay SimHub, congela el último estado y lo indica en `/api/health`.

## Puente para SimHub y CrewChief (espejo LMU → Bottles)

SimHub y CrewChief no pueden leer `LMU_Data` directamente porque corren en el
prefijo de Bottles (`simhub`) y la memoria compartida con nombre de Wine es
privada de cada prefijo/wineserver. El espejo copia los buffers por UDP local
entre los dos prefijos:

```
LMU (GE-Proton10-34-LMU-hid_fixes, pfx de Steam 2399420)
  LMU_Data + $LMU.3.8.SMMP_*        (solo lectura, lock oficial)
        │  lmu_shm_mirror.exe --sender   (emisor, prefijo de LMU)
        │  UDP 127.0.0.1:8772 (fragmentado, 30 Hz / 10 Hz)
        ▼
  lmu_shm_mirror.exe --receiver        (receptor, prefijo de Bottles)
  publica LMU_Data + $LMU.3.8.SMMP_* + alias $rFactor2SMMP_*
        ▼
  SimHub / CrewChief
```

Es estrictamente de solo lectura:

- El emisor abre los buffers con `FILE_MAP_READ` y copia `LMU_Data` bajo el lock
  oficial (`LMU_SharedMemoryLockData`).
- El receptor solo escribe sus propios mappings en el prefijo de Bottles.
- No se espejan buffers de control (`HWControl`, `WeatherControl`,
  `RulesControl`, `PluginControl`, etc.) y no se reenvía ninguna escritura de
  CrewChief hacia LMU.

Buffers espejados (tamaños reales):

| Buffer | Bytes |
|---|---:|
| `LMU_Data` | 327680 (copia 324824) |
| `$LMU.3.8.SMMP_Telemetry$` | 245760 |
| `$LMU.3.8.SMMP_Scoring$` | 77824 |
| `$LMU.3.8.SMMP_Rules$` | 49152 |
| `$LMU.3.8.SMMP_Extended$` | 12288 |
| `$LMU.3.8.SMMP_ForceFeedback$` | 4096 |
| `$LMU.3.8.SMMP_Graphics$` | 4096 |

Además del nombre LMU, el receptor crea los seis alias legados
`$rFactor2SMMP_Telemetry$`, `Scoring`, `Rules`, `Extended`, `ForceFeedback` y
`Graphics`: el lector `CrewChiefV4.rFactor2_V2.RF2SharedMemoryReader` que usa
SimHub los exige todos; si falta `ForceFeedback` o `Graphics` no inicializa.

En la copia de `LMU_Data` se anulan `mVehicle` y `mResultsStream`: son punteros
absolutos del proceso del juego y no son válidos en otro proceso.

### Activar y desactivar

```bash
cd $HOME/.config/etol/payload

./activar_puente_lmu.sh              # espejo + stub + configuracion + vigilante
./activar_puente_lmu.sh --no-config  # no toca SimHub/CrewChief
./activar_puente_lmu.sh --no-hf8     # no configura el pad haptico HF8
./activar_puente_lmu.sh --verbose    # logs detallados
./desactivar_puente_lmu.sh           # para espejo, vigilante y stub
```

`activar_puente_lmu.sh` compila si hace falta, configura SimHub/CrewChief (con
copia de seguridad), arranca el receptor, espera a que publique los mappings,
arranca el emisor y después el stub. Nunca reinicia LMU, SimHub ni CrewChief.

El **stub** es un `.exe` inerte llamado `Le Mans Ultimate.exe` que corre en el
prefijo de Bottles. Solo sirve para que SimHub y CrewChief detecten el juego;
no abre LMU ni toca su memoria. Se arranca después de los mappings y se para
con el script de desactivación.

El **vigilante** (`bridge/lmu_bridge_watchdog.sh`) relanza receptor, emisor y
stub si desaparecen. Hace falta porque al abrir CrewChief o SimHub con otro
runner, Wine reinicia el wineserver del prefijo y mata todos sus procesos: el
puente se quedaba muerto en silencio y las apps dejaban de ver LMU. Se arranca
solo desde `activar_puente_lmu.sh` (o con `--no-watch` para no usarlo) y se para
desde `desactivar_puente_lmu.sh`.

### Orden de arranque

El receptor tiene que existir **antes** de SimHub/CrewChief: si arrancan con el
puente parado, crean sus propios mappings con otro tamaño y el receptor ya no
puede publicar (habría que reiniciar la app).

La forma cómoda es el script de arranque ordenado, que ya mete las pausas:

```bash
./arrancar_todo_lmu.sh                 # espejo -> CrewChief -> SimHub
./arrancar_todo_lmu.sh --espera 40     # más margen entre apps
./arrancar_todo_lmu.sh --no-config     # sin reconfigurar las apps
./arrancar_todo_lmu.sh --no-hf8        # sin configurar el pad HF8
./arrancar_todo_lmu.sh --dry-run       # ver qué haría
```

Es idempotente: si CrewChief o SimHub ya están abiertos, los deja como están.
Espera a que aparezca la ventana de cada app antes de seguir, comprueba que el
espejo siga vivo después de abrirlas (por si el `wineserver` se reinicia) y
escribe todo en `/tmp/etol/lmu_mirror/arranque.log`. No arranca LMU.

Equivale a hacerlo a mano:

1. `./activar_puente_lmu.sh` (da igual que LMU esté abierto o no; el emisor
   espera a `LMU_Data` reintentando cada 2 s).
2. LMU desde Steam.
3. CrewChief.
4. SimHub.

Al terminar: cerrar CrewChief y SimHub, después `./desactivar_puente_lmu.sh` y
por último LMU. El script de desactivación no toca las apps, así que también se
puede ejecutar antes de cerrarlas sin riesgo.

Antes de activar (útil tras reinstalar o mover carpetas):

```bash
./bridge/check_entorno.sh      # -v para ver también lo que está bien
```

Comprueba prefijo de LMU, botella, runners de Wine, SimHub, CrewChief, el enlace
`steam`, los `.exe` del puente, los flags de CrewChief y los procesos. No
modifica nada y devuelve 1 si falta algo imprescindible.

### Pad háptico HF8 (ForceFeel)

El Next Level Racing HF8 es un dispositivo **CDC ACM** (`04d8:ee96`) que Linux
expone como `/dev/ttyACM0` con el driver `cdc_acm`. No es HID: SimHub lo abre
como puerto serie a 115200 8N1 y le manda los 8 canales de vibración.

La botella lo publica como **`COM33`** mediante el enlace:

```bash
~/.local/share/bottles/bottles/simhub/dosdevices/com33 -> /dev/ttyACM0
```

`COM33` es el último puerto estándar de Wine y siempre aparece en
`SerialPort.GetPortNames()`. **No uses un `COM40` "inventado"**: SimHub busca el
pad con `SerialPortDescriptionHelper.GetSerialPortFromPid(0xEE96)`, que solo
recorre los puertos que devuelve `GetPortNames()`; un COM que no esté en
`HARDWARE\DEVICEMAP\SERIALCOMM` se ignora aunque el enlace `dosdevices` exista.

El otro requisito es el tipo del registro. SimHub lee el nombre del dispositivo
de `DEVPKEY_NAME`
(`...\Enum\USB\VID_04D8&PID_EE96&MI_00\<instancia>\Properties\{b725f130-...}\000A`)
y espera **`DEVPROP_TYPE_STRING` (0x12)**. Wine lo devuelve como `REG_SZ` (tipo
1) y el conversor de SimHub lo descarta, así que el pad no aparece. La solución
es importar un `.reg` con `hex(12):` (Wine `reg add` no permite ese tipo;
`regedit /S` sí).

El script `bridge/setup_hf8.sh` lo automatiza:

```bash
./bridge/setup_hf8.sh                 # detecta el tty, crea el enlace e importa el .reg
HF8_COM=COM33 ./bridge/setup_hf8.sh   # puerto estandar alternativo (opcional)
./bridge/check_hf8.sh                 # verifica lo que ve SimHub: "HF8: COM33"
```

- Localiza el tty por VID:PID (no asume `/dev/ttyACM0`).
- Crea `dosdevices/com33` apuntando al tty real.
- Descubre el sufijo de instancia USB que Wine ya creó y genera el `.reg` con
  `bridge/setup_hf8.py`.
- Aplica `"ForceFeel Pad (COM33)"` **solo a `MI_00`** (la interfaz CDC de
  control). El dispositivo padre y `MI_01` se quedan como `"ForceFeel Pad"` sin
  el `(COMn)`, para que `GetSerialPortFromPid` no los confunda con un segundo
  puerto.
- Verifica que `FriendlyName` lleva el COM y que `DEVPKEY_NAME` **no** es
  `REG_SZ`.

`activar_puente_lmu.sh` lo ejecuta automáticamente después de que el receptor
publique los mappings (el `wineserver` de Bottles ya está vivo) y antes de que
arranques SimHub. `arrancar_todo_lmu.sh` lo hereda; se puede saltar con
`--no-hf8` en cualquiera de los dos.

Notas:

- Si mueves el pad a otro puerto USB, Wine puede crear otro sufijo de instancia.
  El script aplica el `.reg` a todos los sufijos que encuentre; por eso puede
  aparecer más de una entrada `COM33` en la lista de puertos. El pad funciona
  igual porque ambos apuntan al mismo tty.
- `SerialPort.Close()` se cuelga bajo Wine. SimHub lo llama al cerrar el
  plugin/pad; si la app se queda colgada al salir, es esa llamada, no el
  puente. Evita cerrar el puerto desde scripts.
- El cambio de puerto se lee al **abrir** el pad. Si SimHub ya estaba abierto
  con otro COM, ciérralo y vuelve a abrirlo cuando no estés en pista.

### Reinstalar el sistema

Los scripts **no son un instalador**: dan por hecho que la botella y el juego ya
existen. Lo que hay que reponer es el entorno, no el puente:

| Pieza | Dónde / cómo |
|---|---|
| LMU + `compatdata/2399420` | Steam; el prefijo se recrea al arrancar el juego |
| Runner `GE-Proton10-34-LMU-hid_fixes` | `~/.local/share/Steam/compatibilitytools.d/` |
| Botella `simhub` + runner `ge-proton11-11` | Bottles, con SimHub y CrewChief instalados en sus rutas habituales |
| Enlace `steam` en la botella | `ln -s "$HOME/.local/share/Steam" "$HOME/.local/share/bottles/bottles/simhub/drive_c/Program Files (x86)/steam"` |
| Proyecto completo | Conserva los `.exe` ya compilados (si no, hacen falta Zig 0.15 y los headers del SDK de LMU) |

Si cambia alguna ruta o versión, no hace falta editar los scripts: se
sobrescriben con variables de entorno.

```bash
LMU_PROTON="$HOME/.local/share/Steam/compatibilitytools.d/otro-runner" \
LMU_PFX="$HOME/.local/share/Steam/steamapps/compatdata/2399420/pfx" \
LMU_GAME="$HOME/.local/share/Steam/steamapps/common/Le Mans Ultimate" \
SIMHUB_BOTTLE="$HOME/.local/share/bottles/bottles/simhub" \
BOTTLE_WINE="$HOME/.local/share/bottles/runners/ge-proton11-11/files/bin/wine" \
./activar_puente_lmu.sh
```

`configure_apps.py` también acepta `SIMHUB_BOTTLE` y `LMU_MIRROR_BACKUP_DIR`;
las rutas de instalación de las apps no se tocan nunca. `LMU_GAME` y `ZIG` solo
los usa `check_entorno.sh` (para localizar el SDK y comprobar si se puede
recompilar).

### Comprobar

```bash
pgrep -af 'lmu_shm_mirror|lmu_bridge_watchdog'
tail -f /tmp/etol/lmu_mirror/receiver.log   # "LMU_Data publicado (324824 bytes, N coches)"
tail -f /tmp/etol/lmu_mirror/sender.log
tail -f /tmp/etol/lmu_mirror/watchdog.log   # solo escribe cuando relanza algo
```

En CrewChief, la consola de la ventana muestra `Using game definition Le Mans
Ultimate`, `Creating buffer: $LMU.3.8.SMMP_*` y después la sesión (tipo, circuito,
pilotos). En SimHub, `http://127.0.0.1:8888` debe dar `GameRunning=true` y
`NewData` con datos.

### Rutas de las apps

SimHub y CrewChief apuntan a
`C:\Program Files (x86)\Steam\SteamApps\common\Le Mans Ultimate`. Dentro del
prefijo de Bottles eso se resuelve por el enlace
`drive_c/Program Files (x86)/steam -> ~/.local/share/Steam`, que ya existía en
la configuración del usuario. `bridge/configure_apps.py` **no** toca
`lmu_install_path`, `GamePath` ni rutas equivalentes: solo ajusta los flags del
puente y la visibilidad/orden de LMU en SimHub.

### Limitaciones

- El espejo depende de que el emisor y el receptor sigan vivos; por eso está el
  vigilante. Si el wineserver se reinicia, hay unos segundos sin datos.
- Si el receptor arranca cuando SimHub/CrewChief ya crearon sus propios
  mappings, los tamaños no coinciden y el receptor no puede publicar; en ese
  caso hay que reiniciar la app que los creó (el receptor lo deja en su log).
- CrewChief queda con `enable_lmu_rf2_plugin_dll_check=False` y
  `lmu_enable_pit_manager=False`: el pit manager escribe en la memoria del
  juego y el puente es de solo lectura. Para conservarlo:
  `LMU_MIRROR_KEEP_PIT_MANAGER=1`.
- SimHub en Wine puede caer en su watchdog (`PssCaptureSnapshot` no existe en
  Wine) y perder el lector del juego; entonces hay que reiniciar SimHub.

## Puente para Automobilista 2 (espejo `$pcars2$` → Bottles)

SimHub y CrewChief leen AMS2 con el struct `AMS2APIStruct` (20.700 bytes) desde
el mapping `$pcars2$` (24.576 bytes). Ese mapping vive en el prefijo Proton de
AMS2 (`compatdata/1066890/pfx`) y no es visible desde Bottles, así que se copia
por UDP local con el mismo patrón que LMU:

```
AMS2 (Proton-GE, pfx de Steam 1066890)
  $pcars2$ (24.576 B)                (FILE_MAP_READ)
        │  ams2_shm_mirror.exe --sender   (emisor, prefijo de AMS2)
        │  UDP 127.0.0.1:8773 (24 KB por frame, 60 Hz)
        ▼
  ams2_shm_mirror.exe --receiver       (receptor, prefijo de Bottles)
  crea/publica $pcars2$ (24.576 B)
        ▼
  SimHub / CrewChief
```

- El emisor abre `$pcars2$` con `FILE_MAP_READ` y **nunca** escribe en el juego.
- El receptor crea `$pcars2$` en Bottles y copia los bytes. Marca
  `mSequenceNumber` (offset **7.320**) como impar antes de copiar y lo publica
  par al final, para que CrewChief descarte frames rasgados.
- Solo se espeja `$pcars2$`. El mapping legado `$pcars` (8.192 B) no lo usan
  los lectores de AMS2 de SimHub/CrewChief.
- No se reenvía ninguna escritura de las apps hacia AMS2.

### Activar y desactivar

```bash
cd $HOME/.config/etol/payload

./activar_puente_ams2.sh              # espejo + stub + configuracion + vigilante
./activar_puente_ams2.sh --no-config  # no toca SimHub/CrewChief
./activar_puente_ams2.sh --no-watch   # sin vigilante
./activar_puente_ams2.sh --no-stub    # sin el proceso inerte AMS2AVX.exe
./activar_puente_ams2.sh --no-hf8     # sin configurar el pad HF8 (ForceFeel)
./desactivar_puente_ams2.sh           # para espejo, vigilante y stub
```

`activar_puente_ams2.sh` compila si hace falta, configura las apps (con copia de
seguridad), arranca el receptor, espera a que cree `$pcars2$`, configura el pad
HF8, arranca el emisor y después el stub. No reinicia AMS2, SimHub ni CrewChief.

El **HF8** se refresca en cada activación porque Bottles puede reasignar
`COM33` a otro dispositivo serie (por ejemplo, un adaptador CH341) y SimHub
dejaría de encontrar el pad. LMU ya lo hacía; AMS2 ahora también. Si no tienes
HF8, usa `--no-hf8`.

El **stub** es un `.exe` inerte llamado `AMS2AVX.exe` que corre en Bottles. Solo
sirve para que CrewChief detecte el juego: su `GameDefinition` de AMS2 usa los
nombres de proceso `AMS2` y `AMS2AVX`. No abre AMS2 ni toca su memoria. SimHub
detecta AMS2 por su propio lector (`Automobilista2` visible, `OrderEx=1`).

El **vigilante** (`bridge/ams2_bridge_watchdog.sh`) relanza receptor, emisor y
stub si el wineserver de Bottles los mata. Se arranca solo desde
`activar_puente_ams2.sh` (o con `--no-watch` para no usarlo).

### Orden de arranque

El receptor debe crear `$pcars2$` antes de que las apps lo abran. Si SimHub o
CrewChief ya estaban abiertos, sus lectores reintentan solos y conectan en
cuanto aparece el mapping; no hace falta reiniciarlos. El receptor es el único
que crea `$pcars2$`; las apps solo lo abren. Si cambias `--size`, para el puente
y cierra las apps que lo tengan abierto antes de volver a activar, porque el
mapping no se puede redimensionar.

```bash
./arrancar_todo_ams2.sh                 # espejo -> CrewChief -> SimHub
./arrancar_todo_ams2.sh --espera 40     # más margen entre apps
./arrancar_todo_ams2.sh --no-config     # sin reconfigurar las apps
./arrancar_todo_ams2.sh --dry-run       # ver qué haría
```

Es idempotente: si CrewChief o SimHub ya están abiertos, los deja como están.
No arranca AMS2 (lo abres desde Steam). Log:
`/tmp/etol/ams2_mirror/arranque.log`.

### Configuración de las apps

`bridge/configure_apps_ams2.py` (backups en
`~/.local/share/ams2_mirror/backups/`):

- CrewChief: `last_game_definition=AMS2` y `AMS2` en
  `limit_available_games`, **solo si CrewChief está cerrado** (si está abierto,
  la app reescribe su configuración al salir).
- SimHub: `Automobilista2` visible (`HiddenGames.json`) y `OrderEx=1`
  (`GameSettings.json`), sin tocar el lector por memoria compartida.
- No modifica ninguna opción de LMU.

### Comprobar

```bash
pgrep -af 'ams2_shm_mirror|ams2_bridge_watchdog'
tail -f /tmp/etol/ams2_mirror/receiver.log   # "$pcars2$ publicado (24576 bytes)"
tail -f /tmp/etol/ams2_mirror/sender.log
cat /tmp/etol/ams2_mirror/watchdog.log
```

En Bottles, `tasklist` debe mostrar `AMS2AVX.exe` (stub), `ams2_shm_mirror.exe`,
`CrewChiefV4.exe` y `SimHubWPF.exe`. SimHub escribe en su log
`Active car model changed` / `Active track changed` / `Game connected`.
CrewChief escribe `Initialised ams2 shared memory` en su consola, pero su log
puede estar bufferizado hasta que se cierra la app.

### Limitaciones

- Solo se espeja `$pcars2$`; no se publica `$pcars`.
- Si AMS2 cambia el tamaño del struct o el offset de `mSequenceNumber`, hay que
  actualizar `kStructSize`/`kSeqOffset` en `bridge/ams2_shm_mirror.cpp`.
- El puente no reinicia apps ni implementa plugins de SimHub: solo publica el
  mapping que leen los lectores de AMS2.
- El stub no tiene ventana: si una app exigiera título de ventana además del
  proceso, habría que ampliarlo (CrewChief y SimHub no lo necesitan en esta
  versión).

## Puente para Assetto Corsa EVO (espejo `acevo_pmf_*` → Bottles)

SimHub (`ACSharedMemory.ACEVOManager`) y CrewChief leen AC EVO desde tres
mappings de memoria compartida:

| Mapping | Bytes | Contenido |
|---|---:|---|
| `Local\acevo_pmf_physics` | 4096 | `SPageFilePhysics`; `packetId` en offset 0 |
| `Local\acevo_pmf_graphics` | 8192 | `SPageFileGraphicEvo`; `packetId` en offset 0 |
| `Local\acevo_pmf_static` | 4096 | `SPageFileStaticEvo`; sin `packetId` |

AC EVO corre en el prefijo Proton de Steam (`compatdata/3058630/pfx`) y
SimHub/CrewChief en la botella `simhub`; los named objects de Wine no cruzan de
prefijo. El espejo copia los tres mappings por UDP loopback:

```
AC EVO (Proton-CachyOS, pfx de Steam 3058630)
  acevo_pmf_physics (4 KB) / graphics (8 KB) / static (4 KB)   FILE_MAP_READ
        │  acevo_shm_mirror.exe --sender   (emisor, prefijo de AC EVO)
        │  UDP 127.0.0.1:8774 (16 KB por frame, 60 Hz)
        ▼
  acevo_shm_mirror.exe --receiver         (receptor, prefijo de Bottles)
  crea/publica los tres mappings en Bottles
        ▼
  SimHub / CrewChief
```

- El emisor abre los tres mappings con `FILE_MAP_READ` y **nunca** escribe en el
  juego.
- El receptor crea los mappings con `PAGE_READWRITE` y copia los bytes. En
  physics/graphics publica `packetId` al final de la copia para que un lector
  que compara el paquete antes/después no acepte una copia a medias. `static`
  se copia completo.
- No se reenvía ninguna escritura de las apps hacia AC EVO.

### Activar y desactivar

```bash
cd $HOME/.config/etol/payload

./activar_puente_acevo.sh              # espejo + stub + configuracion + vigilante
./activar_puente_acevo.sh --no-config  # no toca SimHub/CrewChief
./activar_puente_acevo.sh --no-watch   # sin vigilante
./activar_puente_acevo.sh --no-stub    # sin el proceso inerte AssettoCorsaEVO.exe
./desactivar_puente_acevo.sh           # para espejo, vigilante y stub
```

`activar_puente_acevo.sh` compila si hace falta, configura las apps (con copia
de seguridad), arranca el receptor, espera a que cree los tres mappings,
arranca el emisor y después el stub. No reinicia AC EVO, SimHub ni CrewChief.

El **stub** es un `.exe` inerte llamado `AssettoCorsaEVO.exe` que corre en
Bottles. Solo sirve para que CrewChief detecte el juego: su `GameDefinition`
declara el proceso `AssettoCorsaEVO`. No abre AC EVO ni toca su memoria. SimHub
detecta AC EVO por su propio lector (`AssettoCorsaEVO` visible, `OrderEx=1`).

El **vigilante** (`bridge/acevo_bridge_watchdog.sh`) relanza receptor, emisor y
stub si el wineserver de Bottles los mata. Se arranca solo desde
`activar_puente_acevo.sh` (o con `--no-watch` para no usarlo).

### Orden de arranque

El receptor debe crear los mappings antes de que las apps los abran. Si SimHub o
CrewChief ya estaban abiertos, sus lectores reintentan solos y conectan en
cuanto aparecen; no hace falta reiniciarlos. El receptor es el único que crea
los mappings; las apps solo los abren. Si cambias `--sizes`, para el puente y
cierra las apps que los tengan abiertos antes de volver a activar, porque un
mapping no se puede redimensionar.

```bash
./arrancar_todo_acevo.sh                 # espejo -> CrewChief -> SimHub
./arrancar_todo_acevo.sh --espera 40     # más margen entre apps
./arrancar_todo_acevo.sh --no-config     # sin reconfigurar las apps
./arrancar_todo_acevo.sh --dry-run       # ver qué haría
```

Es idempotente: si CrewChief o SimHub ya están abiertos, los deja como están.
No arranca AC EVO (lo abres desde Steam). Log:
`/tmp/etol/acevo_mirror/arranque.log`.

### Configuración de las apps

`bridge/configure_apps_acevo.py` (backups en
`~/.local/share/acevo_mirror/backups/`):

- CrewChief: `last_game_definition=ASSETTO_EVO` y `ASSETTO_EVO` en
  `limit_available_games`, **solo si CrewChief está cerrado** (si está abierto,
  la app reescribe su configuración al salir).
- SimHub: `AssettoCorsaEVO` visible (`HiddenGames.json`) y `OrderEx=1`
  (`GameSettings.json`), sin tocar el lector por memoria compartida.
- No modifica ninguna opción de LMU ni AMS2.

CrewChief exige reiniciarse para cambiar entre modos circuit y rally. Si la app
está abierta en otro juego, selecciona **Assetto Corsa EVO** cuando no estés en
pista; no cambies la selección con el coche en movimiento.

### Comprobar

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

### Limitaciones

- Solo se espejan los tres mappings `acevo_pmf_*`; no se publica ningún mapping
  adicional de AC EVO.
- Los tamaños observados (4096/8192/4096) y los nombres pueden cambiar con una
  actualización del juego; en ese caso hay que actualizar
  `kDefaultSizes`/`kMappingNames` en `bridge/acevo_shm_mirror.cpp`.
- El puente no reinicia apps ni implementa plugins de SimHub: solo publica los
  mappings que leen los lectores de AC EVO.
- El stub no tiene ventana: si una app exigiera título de ventana además del
  proceso, habría que ampliarlo (CrewChief y SimHub no lo necesitan en esta
  versión).

## Puertos y procesos

| Puerto | Proceso | Uso |
|---:|---|---|
| 8770 | `telemetry_daemon.py` | HTTP + SSE para dashboards (LAN opcional) |
| 8771/UDP | `telemetry_daemon.py` | snapshots del puente LMU SHM |
| 8772/UDP | `lmu_shm_mirror.exe` | espejo LMU → Bottles (SimHub/CrewChief) |
| 8773/UDP | `ams2_shm_mirror.exe` | espejo AMS2 `$pcars2$` → Bottles (SimHub/CrewChief) |
| 8774/UDP | `acevo_shm_mirror.exe` | espejo AC EVO `acevo_pmf_*` → Bottles (SimHub/CrewChief) |
| 6397 | LMU | REST de sesión/standings/clima |
| 6398 | LMU | WebSocket de LMU (protocolo pendiente) |
| 8888 | SimHub (Proton) | WS `GetGameDataUpdates`, fallback |

## API

| Ruta | Contenido |
|---|---|
| `/api/health` | estado de SimHub, LMU y SHM + `source` + `shm_seq` |
| `/api/light` | estado ligero: `meta`, `simhub` normalizado, `lmu` |
| `/api/state` | estado completo, incluye `native` crudo |
| `/api/native` | último snapshot crudo del puente (37 KB) |
| `/api/standings` | standings; SHM ordenado por posición, o LMU REST |
| `/api/stream` | SSE con el estado ligero a ~10 Hz |
| `/djson` | renderer de plantillas `.djson` |
| `/relative` | relative nativo: 4+4, `REST`/`SB`, `LAST`, `BOX`/`OUT`, neumático cuando está disponible, energía virtual y vueltas de combustible |
| `/` | dashboard simple nativo |

## Normalización SHM → nombres SimHub

El puente entrega nombres nativos (`speedKmh`, `rpm`, `gear`, ...); el daemon
los convierte a los nombres que ya usan los dashboards:

| SHM | Propiedad tipo SimHub |
|---|---|
| `telemetry.speedKmh` | `SpeedKmh` |
| `telemetry.rpm` / `maxRpm` | `Rpms` / `MaxRpm` |
| `telemetry.gear` | `Gear` (`R`/`N`/`1`...) |
| `telemetry.throttle/brake` | `Throttle` / `Brake` (0–100) |
| `player.bestLap` | `BestLapTime` |
| `game.sessionBestLap` | `SessionBestTime` |
| `player.lastLap` | `LastLapTime` |
| `telemetry.currentLapTime` | `CurrentLapTime` / `LapTime` |
| `player.lapDist` / `game.lapDist` | `LapDistance` / `TrackLength` |
| `player.lapDist / game.lapDist` | `TrackPositionPercent` |
| `player.place` | `Position` |
| `player.laps` | `CompletedLaps` |
| `telemetry.fuel` / `fuelPct` | `Fuel` / `FuelPercent` |
| `telemetry.virtualEnergy` | `VirtualEnergy` (fracción 0–1; el relative la muestra en %) |
| `wheels[i].tempC[1]` | `TyreTemperatureFrontLeft`... |
| `wheels[i].wear * 100` | `TyreWearFrontLeft`... |
| `wheels[i].pressureKpa` | `TyrePressureFrontLeft`... |
| `wheels[i].brakeTempC` | `BrakeTemperatureFrontLeft`... |
| `wheels[i].flat` | `TyreFlatFrontLeft`... (dashboard simple) |
| `game.track` | `TrackName` |
| `game.sessionType` | `SessionTypeName` / `Session` |
| `player.car` / `class` | `CarName` / `CarClass` |
| `game.ambientTemp/trackTemp` | `AirTemp` / `TrackTemp` |
| `game.raining/avgPathWetness` | `Rain` / `Wetness` |

Detalles importantes:

- `maxLaps` de LMU puede ser `2147483647` (carrera sin límite). El daemon lo
  normaliza a `0` para que el dashboard muestre solo las vueltas completadas.
- `lastLap` puede ser `-1` (vuelta inválida/aún sin completar); el renderer
  muestra `-:--.---`.
- `mWear` se interpreta como fracción **restante** (1.0 = nuevo). Si un
  neumático está pinchado, LMU puede reportar `flat=true`, presión 0 y wear 0;
  el dashboard simple muestra `FLAT` en rojo.
- `SessionBestTime` usa el mejor tiempo de todos los coches
  (`game.sessionBestLap`), no solo el del jugador.

## Relative nativo

`web/relative.html` es un dashboard propio inspirado en `NEW_5_RELATIVE`, sin
depender de `WidgetItem`, JavaScript de doX ni plugins de SimHub. Consume el
estado ligero por SSE (`/api/stream`) y los standings por HTTP
(`/api/standings`, 1 Hz). Para las vueltas de salida consulta cada 2 s
`/api/lmu/rest/watch/standings` (solo lectura; no toca el puente ni el juego).

- Calcula la distancia relativa con `lapDist` y hace *wrap* sobre
  `TrackLength`, para mostrar coches físicamente delante/detrás y no solo el
  orden de carrera.
- Convierte la distancia a segundos con `estimatedLapTime`; es una estimación,
  no el gap oficial de doX.
- Muestra siempre 4 coches delante + el jugador + 4 coches detrás, con fuente
  más grande y el jugador centrado. Si hay menos coches disponibles, muestra los
  que haya.
- La cabecera muestra `REST` (tiempo restante de sesión) y `SB` (mejor vuelta
  de sesión) al lado de pista/sesión. En móvil estrecho el bloque pasa a una
  segunda línea, pero sigue en la parte superior.
- Cada fila muestra, después del piloto, `LAST` (última vuelta), el estado
  `BOX` / `OUT` / `GARAGE` y el compuesto de neumático cuando está disponible.
- `BOX` sale de `inPits`/`pitting` o de `pitState` `ENTERING`/`STOPPED`/
  `EXITING`; `OUT` sale de `countLapFlag === "COUNT_LAP_ONLY"` en LMU REST. Si
  REST no responde, se usa un fallback por transición de entrada/salida de
  boxes. `COUNT_LAP_ONLY` puede incluir alguna vuelta invalidada, así que `OUT`
  es una aproximación útil, no un dato oficial de estrategia.
- Neumáticos: LMU SHM solo expone el compuesto del jugador
  (`TyreCompoundFront`/`TyreCompoundRear`); los rivales no lo publican. Por eso
  no se inventa: el jugador muestra `S`/`M`/`H`/`I`/`W` y el rival solo si el
  puente/API lo aporta en el futuro (`frontTyreCompoundName`/`tyreCompound`).
- Excluye coches en garaje de la lista de rivales; el jugador se mantiene.
- Colores por clase (`Hyper`, `LMP2`, `GT3`), posición, número, piloto, hueco,
  combustible, tiempos, bandera y luces de salida.
- Muestra `VIRTUAL ENERGY` en porcentaje (`0.971` → `97.1 %`) con barra.
- Estima las vueltas de combustible restantes con el combustible consumido
  entre vueltas completadas. Usa una media móvil de las últimas 5 vueltas y
  cachea el consumo por circuito+coche en `localStorage`; hasta tener una vuelta
  válida (o una caché previa) muestra `— LAPS`.
- En móvil (<560 px) el panel lateral pasa debajo en una rejilla **3×2** y las
  filas usan dos líneas (piloto arriba; `LAST`/estado/neumático abajo) para
  aprovechar todo el alto y no dejar los nombres ni los datos pequeños. En
  tablet vertical (561–820 px con alto ≥700 px) se usa la misma idea con filas
  más grandes. En móvil horizontal se mantiene el panel lateral derecho.
- Enlaces de navegación entre `/`, `/relative` y `/djson` en escritorio; en móvil
  se ocultan para dejar sitio al estado y a las luces.

## Renderer `.djson`

`web/djson.js` compone las pantallas del `.djson` (fondo, activa, foreground) y
soporta:

- `Layer`/`Group`, `TextItem`, `RectangleItem`/`EllipseItem`, `GradientItem`.
- `LinearGaugeItem` / `VerticalLinearGaugeItem`.
- `ImageItem`.
- Behaviors: `LastLapTime`, `SessionBestTime`, `LapTime`, `CurrentGear`,
  `Speed`, `RPM`, `LapPercentage`.
- Bindings simples tipo `[DataCorePlugin.GameData.NewData.SpeedKmh]`.

Pendiente de ampliar: `ScreenEnabledExpression`, `DialGauge`, overlays,
leaderboard controls, más behaviors y bindings, imágenes/asset paths.

## Pruebas y evidencia

```bash
python3 -m py_compile telemetry_daemon.py
node --check web/djson.js

# jsdom (renderer + dashboard simple)
cd /tmp/etol/jsdomtest
node test_renderer.js
node test_dash.js

# capturas Playwright de escritorio y móvil
cd /tmp/etol/pwtest
node capture_djson.js
node capture_simple.js
node capture_relative_final2.js
```

Evidencia guardada en `../evidencia/`:

- `shm_bridge/shm_snapshot_once.json` — snapshot de 37 KB, 54 coches.
- `shm_bridge/native.json` — snapshot crudo en vivo.
- `shm_bridge/health.json` — `shm:true`, `source:"shm+lmu"`.
- `shm_bridge/light.json` — valores normalizados (278 km/h, marcha 6, ...).
- `shm_bridge/standings.json` — 54 coches ordenados por posición.
- `shm_bridge/probe_shm2.txt` — prueba del scoring crudo y del evento ausente.
- `shm_bridge/dash_*.png` — capturas de ambos dashboards.
- `shm_bridge/bridge.log`, `daemon.log`, `toolchain.txt`.
- `native_relative/relative_850x450.png`, `relative_1280x720.png`,
  `relative_1920x1080.png`, `relative_390x844.png`, `relative_844x390.png`,
  `relative_768x1024.png`, `relative_560x800.png` — relative nativo en vivo,
  incluido móvil vertical/horizontal y tablet vertical.
- `native_relative/relative_lan_390x844.png` — la misma página cargada por la
  URL LAN (`192.168.1.170:8770`), no por `127.0.0.1`.
- `native_relative/relative_mock_850x450.png`,
  `relative_mock_844x390.png`, `relative_mock_390x844.png` — validación con
  datos sintéticos de vueltas de combustible, `REST`/`SB` y energía virtual.
- `native_relative/relative_mock_rows_850x450.png`,
  `relative_mock_rows_844x390.png`, `relative_mock_rows_390x844.png` —
  validación sintética de `LAST`, `BOX` y compuesto de neumático.
- `native_relative/health.json`, `light.json`, `standings.json` — estado
  capturado durante la prueba.
- `native_relative/playwright_final.json` — resumen de las capturas responsive,
  combustible/VE y filas enriquecidas.
- `native_relative/resultado.md` — validación y limitaciones del relative.
- `prototipo_nativo.md` — informe técnico en español.

## Estado y limitaciones

Funciona:

- LMU: telemetría nativa (velocidad, RPM, marcha, pedales, combustible,
  temperaturas, desgaste, presiones, frenos, suspensiones, posición, vueltas,
  tiempos, clima) + scoring de todos los coches.
- Dashboards `.djson` y dashboard simple con SHM como fuente primaria.
- Relative nativo (`/relative`) con 4+4 coches, colores de clase, boxes, huecos
  estimados por distancia de pista, energía virtual y vueltas de combustible.
- Fallback automático a SimHub WS si el puente se detiene.
- Standings nativos ordenados por posición.
- SimHub y CrewChief en Bottles leen LMU por el espejo UDP (telemetría,
  scoring, reglas, extended, force feedback y graphics), sin reiniciar el juego.
- AMS2: SimHub y CrewChief leen `$pcars2$` (24.576 B) por el espejo UDP
  `8773`, con `AMS2APIStruct` de 20.700 B, sin reiniciar el juego.
- AC EVO: SimHub lee `acevo_pmf_physics` (4 KB), `acevo_pmf_graphics` (8 KB) y
  `acevo_pmf_static` (4 KB) por el espejo UDP `8774`. CrewChief usa el mismo
  espejo y el stub `AssettoCorsaEVO.exe`; si estaba en otro juego hay que
  seleccionar **Assetto Corsa EVO** en la app (la configuración automática solo
  se aplica con CrewChief cerrado).

Pendiente:

- LMU WebSocket 6398: falta el protocolo de suscripción; REST cubre sesión y
  standings.
- Adaptador nativo JSON para AMS2 y AC EVO (los espejos `$pcars2$` y
  `acevo_pmf_*` para SimHub/CrewChief ya funcionan); iRacing (no instalado).
- Plugins de terceros WPF/WinForms (NeoRed, doX, etc.) y ShakeIt: no pueden
  ejecutarse nativamente; hay que reimplementar sus efectos/lógica.
- Gap exacto de doX y datos por coche (compuesto, combustible, desgaste): el
  relative actual estima el hueco con distancia/tiempo de vuelta y solo muestra
  el estado global del jugador.
- Las vueltas de combustible son una estimación propia: necesitan al menos una
  vuelta completa válida o una caché previa del mismo circuito+coche; no es el
  cálculo interno de LMU/SimHub y puede desviarse con repostajes o cambios de
  consumo.
- Overlays/leaderboard avanzados y más cobertura `.djson`.
- Fragmentar/compactar el UDP si una sesión supera ~60 KB de JSON.
- Empaquetado (servicio de usuario, lanzador, instalador).

## Archivos clave

- `telemetry_daemon.py` — daemon HTTP/SSE, SHM UDP, normalización, fallback.
- `bridge/lmu_shm_bridge.cpp` — puente LMU SHM → JSON/UDP.
- `bridge/lmu_shm_mirror.cpp` — espejo LMU → Bottles para SimHub/CrewChief.
- `bridge/lmu_process_stub.cpp` — stub inerte `Le Mans Ultimate.exe`.
- `bridge/configure_apps.py` — flags de SimHub/CrewChief (con backups).
- `bridge/lmu_bridge_watchdog.sh` — relanza el espejo si Wine lo mata.
- `activar_puente_lmu.sh`, `desactivar_puente_lmu.sh` — ciclo del espejo.
- `arrancar_todo_lmu.sh` — arranque ordenado: espejo → CrewChief → SimHub.
- `bridge/ams2_shm_mirror.cpp` — espejo AMS2 `$pcars2$` → Bottles.
- `bridge/ams2_process_stub.cpp` — stub inerte `AMS2AVX.exe`.
- `bridge/configure_apps_ams2.py` — configuración AMS2 de SimHub/CrewChief.
- `bridge/ams2_bridge_watchdog.sh` — relanza el espejo AMS2 si Wine lo mata.
- `activar_puente_ams2.sh`, `desactivar_puente_ams2.sh` — ciclo del espejo AMS2.
- `arrancar_todo_ams2.sh` — arranque ordenado: espejo AMS2 → CrewChief → SimHub.
- `bridge/acevo_shm_mirror.cpp` — espejo AC EVO `acevo_pmf_*` → Bottles.
- `bridge/acevo_process_stub.cpp` — stub inerte `AssettoCorsaEVO.exe`.
- `bridge/configure_apps_acevo.py` — configuración AC EVO de SimHub/CrewChief.
- `bridge/acevo_bridge_watchdog.sh` — relanza el espejo AC EVO si Wine lo mata.
- `activar_puente_acevo.sh`, `desactivar_puente_acevo.sh` — ciclo del espejo AC EVO.
- `arrancar_todo_acevo.sh` — arranque ordenado: espejo AC EVO → CrewChief → SimHub.
- `bridge/check_entorno.sh` — comprobación del entorno (tras reinstalar).
- `bridge/build_bridge.sh`, `bridge/run_bridge.sh`, `bridge/stop_bridge.sh`.
- `bridge/README.md` — compilación/ejecución/detalles del puente.
- `web/dash.html` — dashboard simple nativo.
- `web/relative.html` — relative nativo inspirado en `NEW_5_RELATIVE`.
- `web/djson.js`, `web/djson.html` — renderer Dash Studio.
- `../evidencia/prototipo_nativo.md` — informe y evidencias.

## Nota legal / SDK

El puente incluye en tiempo de compilación los headers oficiales
`SharedMemoryInterface.hpp` / `InternalsPlugin.hpp` de la instalación de LMU.
No se copian ni redistribuyen en este repositorio. El puente solo lee memoria
compartida; no modifica el juego.

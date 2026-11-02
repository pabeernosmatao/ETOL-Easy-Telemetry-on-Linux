# Binarios de terceros

## `lmu_shm_mirror.exe`

Puente LMU de `SIMHUB_LINUX` con un unico cambio: los tamanos de reserva de los
buffers SMMP pasan a ser los que espera CrewChief.

| Buffer | Original | Parcheado |
| --- | --- | --- |
| `SMMP_Telemetry` | 245760 | 241680 |
| `SMMP_Scoring` | 77824 | 75312 |
| `SMMP_Rules` | 49152 | 45272 |
| `SMMP_Extended` | 12288 | 10152 |

Sin el parche, CrewChief puede responder con `Destination array was not long
enough` cuando abre los buffers antes de que el juego publique los suyos.

- Origen: variante parcheada de LMU (PARCHE_LMU 0.1.1) sobre el puente original de SIMHUB_LINUX
- SHA-256: `236c9c9e61464a93427e6dbe472aebfc8e991a4ee1d77d3142b2ab138d0c8446`
- El original de `SIMHUB_LINUX` no se toca: `preparar-payload.sh` copia todo y
  despues sobrescribe solo `bridge/lmu_shm_mirror.exe` en la copia del payload.

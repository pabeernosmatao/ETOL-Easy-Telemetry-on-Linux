// Proceso inerte con nombre "acs.exe" para que CrewChief y SimHub vean
// Assetto Corsa dentro del prefijo de Bottles.
//
// No abre el juego, no lee ni escribe memoria compartida. Solo mantiene vivo un
// proceso con el nombre que buscan las apps (SimHub usa acs, acs_x86, ...).
//
// Uso:
//   "acs.exe"          espera hasta que se senalice el evento de parada
//   "acs.exe" --stop   senaliza el evento de parada y sale
//
// El nombre del proceso lo da el nombre del archivo .exe. El script de
// activacion copia este binario como "acs.exe" en RUN_DIR.

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstring>

int main(int argc, char** argv) {
    const char* kStopEvent = "AC_MirrorStubStop";
    const char* kReadyEvent = "AC_MirrorStubReady";

    if (argc > 1 && std::strcmp(argv[1], "--stop") == 0) {
        HANDLE stop = OpenEventA(EVENT_MODIFY_STATE, FALSE, kStopEvent);
        if (!stop) return 1;
        SetEvent(stop);
        CloseHandle(stop);
        return 0;
    }

    HANDLE stop = CreateEventA(nullptr, TRUE, FALSE, kStopEvent);
    if (!stop) return 1;
    ResetEvent(stop);

    HANDLE ready = CreateEventA(nullptr, TRUE, FALSE, kReadyEvent);
    if (ready) {
        SetEvent(ready);
        CloseHandle(ready);
    }

    WaitForSingleObject(stop, INFINITE);
    CloseHandle(stop);
    return 0;
}

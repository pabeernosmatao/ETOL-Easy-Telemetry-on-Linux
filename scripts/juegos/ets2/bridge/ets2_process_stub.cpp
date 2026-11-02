// Proceso inerte con nombre "eurotrucks2.exe" para que SimHub vea Euro Truck
// Simulator 2 dentro del prefijo de Bottles.
//
// SimHub elige el simulador activo por el proceso que encuentra en su propio
// prefijo. ETS2 corre nativo en Linux, fuera de Bottles, asi que SimHub no lo
// ve y no cambia solo. Este stub solo mantiene vivo ese nombre: no abre el
// juego, no lee ni escribe memoria y no tiene ventana.
//
// Uso:
//   "eurotrucks2.exe"          espera hasta que se senalice el evento de parada
//   "eurotrucks2.exe" --stop   senaliza el evento de parada y sale

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstring>

int main(int argc, char** argv) {
    const char* kStopEvent = "Ets2_MirrorStubStop";
    const char* kReadyEvent = "Ets2_MirrorStubReady";

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

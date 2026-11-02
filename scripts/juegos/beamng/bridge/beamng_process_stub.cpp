// Proceso inerte con nombre "BeamNG.drive.x64.exe" para que SimHub vea
// BeamNG.drive dentro del prefijo de Bottles.
//
// BeamNG corre nativo en Linux, fuera de Bottles. Este stub solo mantiene vivo
// ese nombre: no abre el juego, no lee ni escribe memoria y no tiene ventana.
//
// Uso:
//   "BeamNG.drive.x64.exe"          espera hasta que se senalice la parada
//   "BeamNG.drive.x64.exe" --stop   senaliza la parada y sale

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstring>

int main(int argc, char** argv) {
    const char* kStopEvent = "BeamNg_MirrorStubStop";
    const char* kReadyEvent = "BeamNg_MirrorStubReady";

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

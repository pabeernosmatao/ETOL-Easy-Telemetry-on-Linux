// Proceso inerte con nombre "rFactor2.exe" para que CrewChief y SimHub vean
// rFactor 2 dentro del prefijo de Bottles.
//
// No abre el juego, no lee ni escribe memoria compartida. Solo mantiene vivo un
// proceso con el nombre que buscan las apps.
//
// Uso:
//   "rFactor2.exe"          espera hasta que se senalice el evento de parada
//   "rFactor2.exe" --stop   senaliza el evento de parada y sale

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstring>

int main(int argc, char** argv) {
    const char* kStopEvent = "RF2_MirrorStubStop";
    const char* kReadyEvent = "RF2_MirrorStubReady";

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

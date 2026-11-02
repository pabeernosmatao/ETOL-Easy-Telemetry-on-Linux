// Proceso inerte con nombre "dirtrally2.exe" para que CrewChief vea DiRT Rally
// 2.0 dentro del prefijo de Bottles.
//
// CrewChief solo abre su lector UDP del juego cuando encuentra un proceso
// llamado "dirtrally2" en su propio prefijo. El juego real corre en el prefijo
// de Proton de Steam, que es otro wineserver, asi que no lo ve. Este stub no
// abre el juego, no lee ni escribe memoria: solo mantiene vivo ese nombre.
//
// Uso:
//   "dirtrally2.exe"          espera hasta que se senalice el evento de parada
//   "dirtrally2.exe" --stop   senaliza el evento de parada y sale

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstring>

int main(int argc, char** argv) {
    const char* kStopEvent = "Dirt2_MirrorStubStop";
    const char* kReadyEvent = "Dirt2_MirrorStubReady";

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

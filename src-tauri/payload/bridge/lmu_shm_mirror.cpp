// Espejo de solo lectura de la telemetria de Le Mans Ultimate entre prefijos Wine.
//
// Uso:
//   lmu_shm_mirror.exe --sender   --udp 127.0.0.1:8772 --hz 30 --smmp-hz 10
//   lmu_shm_mirror.exe --receiver --bind 127.0.0.1:8772
//
// El emisor se ejecuta en el prefijo de LMU y solo abre LMU_Data y los buffers
// $LMU.3.8.SMMP_* con FILE_MAP_READ. El receptor se ejecuta en el prefijo de
// Bottles (SimHub/CrewChief), crea las copias locales y publica LMU_Data_Event.
//
// No se reenvia ninguna escritura hacia LMU: el sentido es siempre
// LMU -> UDP -> Bottles. Los buffers de control (HWControl, WeatherControl,
// RulesControl, PluginControl) no se copian.
//
// Compilacion:
//   zig c++ -target x86_64-windows-gnu -std=c++17 -O2 \
//     -I"<LMU>/Support/SharedMemoryInterface" \
//     -o lmu_shm_mirror.exe lmu_shm_mirror.cpp -lws2_32

#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>

#include <algorithm>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <string>
#include <vector>

#include "SharedMemoryInterface.hpp"

namespace {

constexpr uint32_t kMagic = 0x4C4D5542u; // "LMUB"
constexpr uint16_t kVersion = 1;
constexpr uint8_t kKindLmu = 1;
constexpr uint8_t kKindSmmp = 2;
constexpr size_t kMaxPayload = 60000;
constexpr int kDefaultHz = 30;
constexpr int kDefaultSmmpHz = 10;

#pragma pack(push, 1)
struct PacketHeader {
    uint32_t magic;
    uint16_t version;
    uint8_t kind;
    uint8_t flags;
    uint32_t frame;
    uint32_t total;
    uint32_t offset;
    uint32_t payload;
    char name[64];
};
#pragma pack(pop)

static_assert(sizeof(PacketHeader) == 88, "cabecera UDP inesperada");

struct SmmpSpec {
    const char* name;
    bool partial;
    /// Tamano que reserva CrewChief para $LMU.3.8.* (menor que LMU 3.8).
    size_t crewchiefSize;
    // SimHub (RfactorReader.LMUManager) usa el lector rF2 legado, que abre los
    // nombres $rFactor2SMMP_*. Se publica el buffer completo de LMU 3.8.
    const char* alias;
    size_t simhubSize;
};

// CrewChief dimensiona sus arrays con el layout LMU 3.8 que el conoce:
// 241680/75312/45272/10152. LMU publica 4080/2512/3880/2136 bytes mas; si el
// mapping de $LMU.3.8.* tiene el tamano grande, CrewChief falla con
// "Destination array was not long enough" y no lee la telemetria.
// SimHub sigue recibiendo el buffer completo bajo $rFactor2SMMP_*.
const SmmpSpec kSmmpSpecs[] = {
    {"$LMU.3.8.SMMP_Telemetry$", true, 241680, "$rFactor2SMMP_Telemetry$", 245760},
    {"$LMU.3.8.SMMP_Scoring$", true, 75312, "$rFactor2SMMP_Scoring$", 77824},
    {"$LMU.3.8.SMMP_Rules$", true, 45272, "$rFactor2SMMP_Rules$", 49152},
    {"$LMU.3.8.SMMP_Extended$", false, 10152, "$rFactor2SMMP_Extended$", 12288},
    {"$LMU.3.8.SMMP_ForceFeedback$", false, 4096, "$rFactor2SMMP_ForceFeedback$", 4096},
    {"$LMU.3.8.SMMP_Graphics$", false, 4096, "$rFactor2SMMP_Graphics$", 4096},
};

uint64_t nowMs() {
    return static_cast<uint64_t>(GetTickCount64());
}

void logLine(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    std::vfprintf(stderr, fmt, ap);
    va_end(ap);
    std::fputc('\n', stderr);
    std::fflush(stderr);
}

bool parseHostPort(const std::string& text, std::string& host, int& port) {
    auto pos = text.rfind(':');
    if (pos == std::string::npos) return false;
    host = text.substr(0, pos);
    if (host.empty()) host = "127.0.0.1";
    char* end = nullptr;
    long p = std::strtol(text.c_str() + pos + 1, &end, 10);
    if (!end || *end != '\0' || p < 1 || p > 65535) return false;
    port = static_cast<int>(p);
    return true;
}

struct ReadMapping {
    HANDLE handle = nullptr;
    const uint8_t* view = nullptr;
    size_t size = 0;

    bool open(const char* name) {
        close();
        handle = OpenFileMappingA(FILE_MAP_READ, FALSE, name);
        if (!handle) return false;
        view = static_cast<const uint8_t*>(MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0));
        if (!view) {
            close();
            return false;
        }
        MEMORY_BASIC_INFORMATION mbi{};
        if (VirtualQuery(view, &mbi, sizeof(mbi)) == 0) {
            close();
            return false;
        }
        size = static_cast<size_t>(mbi.RegionSize);
        return size > 0;
    }

    void close() {
        if (view) {
            UnmapViewOfFile(view);
            view = nullptr;
        }
        if (handle) {
            CloseHandle(handle);
            handle = nullptr;
        }
        size = 0;
    }

    ~ReadMapping() { close(); }
};

struct WriteMapping {
    HANDLE handle = nullptr;
    uint8_t* view = nullptr;
    size_t size = 0;
    uint32_t version = 0;

    bool ensure(const char* name, size_t wanted) {
        if (handle && view && size >= wanted) return true;
        close();
        if (wanted == 0 || wanted > 16u * 1024u * 1024u) return false;
        handle = CreateFileMappingA(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
                                    0, static_cast<DWORD>(wanted), name);
        if (!handle) return false;
        view = static_cast<uint8_t*>(MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, wanted));
        if (!view) {
            close();
            return false;
        }
        size = wanted;
        return true;
    }

    /// Como ensure, pero el mapping debe tener exactamente `wanted` bytes.
    /// Los buffers de CrewChief y SimHub comparten contenido pero no tamano.
    bool ensureExact(const char* name, size_t wanted) {
        if (handle && view && size == wanted) return true;
        close();
        if (wanted == 0 || wanted > 16u * 1024u * 1024u) return false;
        handle = CreateFileMappingA(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
                                    0, static_cast<DWORD>(wanted), name);
        if (!handle) return false;
        view = static_cast<uint8_t*>(MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, wanted));
        if (!view) {
            close();
            return false;
        }
        size = wanted;
        return true;
    }

    void close() {
        if (view) {
            UnmapViewOfFile(view);
            view = nullptr;
        }
        if (handle) {
            CloseHandle(handle);
            handle = nullptr;
        }
        size = 0;
        version = 0;
    }

    ~WriteMapping() { close(); }
};

void stampInitialHeader(WriteMapping& wm, bool partial, size_t size) {
    if (!wm.view || wm.version != 0) return;
    wm.version = 1;
    uint32_t* hdr = reinterpret_cast<uint32_t*>(wm.view);
    hdr[0] = 1;
    hdr[1] = 1;
    if (partial && size >= 12) {
        // Tamano maximo del bloque parcial. El primer frame real lo sustituye
        // por el tamano que publica LMU.
        hdr[2] = static_cast<uint32_t>(size);
    }
}

struct SenderOptions {
    std::string host = "127.0.0.1";
    int port = 8772;
    int hz = kDefaultHz;
    int smmpHz = kDefaultSmmpHz;
    bool verbose = false;
};

class UdpSender {
public:
    bool init(const SenderOptions& opt) {
        opt_ = opt;
        WSADATA wsa{};
        if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
            logLine("ERROR: WSAStartup fallo: %d", WSAGetLastError());
            return false;
        }
        wsaStarted_ = true;
        sock_ = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
        if (sock_ == INVALID_SOCKET) {
            logLine("ERROR: no se pudo crear socket UDP: %d", WSAGetLastError());
            return false;
        }
        int sndbuf = 8 * 1024 * 1024;
        setsockopt(sock_, SOL_SOCKET, SO_SNDBUF,
                   reinterpret_cast<const char*>(&sndbuf), sizeof(sndbuf));

        addr_.sin_family = AF_INET;
        addr_.sin_port = htons(static_cast<u_short>(opt.port));
        addr_.sin_addr.s_addr = inet_addr(opt.host.c_str());
        if (addr_.sin_addr.s_addr == INADDR_NONE) {
            logLine("ERROR: direccion UDP invalida: %s", opt.host.c_str());
            return false;
        }
        packet_.reserve(sizeof(PacketHeader) + kMaxPayload);
        return true;
    }

    ~UdpSender() {
        if (sock_ != INVALID_SOCKET) closesocket(sock_);
        if (wsaStarted_) WSACleanup();
    }

    bool sendFrame(uint8_t kind, const char* name, const uint8_t* data,
                   size_t size, uint32_t frame) {
        if (!data || size == 0) return false;
        size_t off = 0;
        bool ok = true;
        while (off < size) {
            size_t chunk = std::min(kMaxPayload, size - off);
            PacketHeader h{};
            h.magic = kMagic;
            h.version = kVersion;
            h.kind = kind;
            h.flags = (off + chunk == size) ? 1 : 0;
            h.frame = frame;
            h.total = static_cast<uint32_t>(size);
            h.offset = static_cast<uint32_t>(off);
            h.payload = static_cast<uint32_t>(chunk);
            if (name) {
                std::strncpy(h.name, name, sizeof(h.name) - 1);
                h.name[sizeof(h.name) - 1] = '\0';
            }

            packet_.resize(sizeof(PacketHeader) + chunk);
            std::memcpy(packet_.data(), &h, sizeof(h));
            std::memcpy(packet_.data() + sizeof(h), data + off, chunk);
            int sent = sendto(sock_, reinterpret_cast<const char*>(packet_.data()),
                              static_cast<int>(packet_.size()), 0,
                              reinterpret_cast<const sockaddr*>(&addr_), sizeof(addr_));
            if (sent == SOCKET_ERROR) {
                ok = false;
                if (opt_.verbose) {
                    logLine("WARN: sendto fallo: %d", WSAGetLastError());
                }
                break;
            }
            off += chunk;
        }
        return ok;
    }

private:
    SenderOptions opt_;
    SOCKET sock_ = INVALID_SOCKET;
    bool wsaStarted_ = false;
    sockaddr_in addr_{};
    std::vector<uint8_t> packet_;
};

int runSender(const SenderOptions& opt) {
    UdpSender udp;
    if (!udp.init(opt)) return 1;

    logLine("Emisor LMU -> udp://%s:%d (LMU %.1f Hz, SMMP %.1f Hz)",
            opt.host.c_str(), opt.port,
            static_cast<double>(opt.hz), static_cast<double>(opt.smmpHz));

    std::vector<uint8_t> lmuBuffer(sizeof(SharedMemoryObjectOut));
    std::vector<std::vector<uint8_t>> smmpBuffers(
        sizeof(kSmmpSpecs) / sizeof(kSmmpSpecs[0]));
    uint32_t frame = 0;
    std::map<std::string, bool> announced;

    while (true) {
        ReadMapping lmu;
        uint64_t lastWaitLog = 0;
        while (!lmu.open(LMU_SHARED_MEMORY_FILE)) {
            // Sin el juego el emisor espera indefinidamente: se avisa cada 30 s
            // en vez de cada 2 s para no llenar el log si queda encendido.
            const uint64_t t = nowMs();
            if (lastWaitLog == 0 || t - lastWaitLog >= 30000) {
                logLine("Esperando a que LMU cree %s...", LMU_SHARED_MEMORY_FILE);
                lastWaitLog = t;
            }
            Sleep(2000);
        }

        auto lock = SharedMemoryLock::MakeSharedMemoryLock();
        if (!lock.has_value()) {
            logLine("WARN: no se pudo abrir el lock de LMU; reintentando");
            lmu.close();
            Sleep(2000);
            continue;
        }
        auto waiter = SharedMemoryUpdateWaiter::MakeSharedMemoryUpdateWaiter();
        if (!waiter.has_value()) {
            logLine("WARN: no se pudieron abrir los eventos de LMU; reintentando");
            lmu.close();
            Sleep(2000);
            continue;
        }

        const auto* shared = reinterpret_cast<const SharedMemoryLayout*>(lmu.view);
        std::vector<ReadMapping> smmp(sizeof(kSmmpSpecs) / sizeof(kSmmpSpecs[0]));
        uint64_t lastFrame = nowMs();
        uint64_t lastLmu = 0;
        uint64_t lastSmmp = 0;
        uint64_t frames = 0;
        uint64_t lmuFrames = 0;
        uint64_t smmpFrames = 0;

        logLine("LMU_Data abierto (mapping %llu bytes); espejando.",
                static_cast<unsigned long long>(lmu.size));

        while (true) {
            bool got = waiter->WaitForNextFrame(250);
            if (!got) {
                if (nowMs() - lastFrame > 5000) {
                    logLine("Sin frames de LMU durante 5 s; cierro handles y espero.");
                    break;
                }
                continue;
            }
            lastFrame = nowMs();
            ++frames;

            uint64_t now = nowMs();
            const uint64_t lmuInterval = static_cast<uint64_t>(1000 / std::max(1, opt.hz));
            const uint64_t smmpInterval = static_cast<uint64_t>(1000 / std::max(1, opt.smmpHz));

            if (now - lastLmu >= lmuInterval) {
                if (lock->Lock(1000)) {
                    std::memcpy(lmuBuffer.data(), &shared->data, lmuBuffer.size());
                    lock->Unlock();
                    ++frame;
                    if (udp.sendFrame(kKindLmu, nullptr, lmuBuffer.data(), lmuBuffer.size(), frame)) {
                        ++lmuFrames;
                        if (opt.verbose && (lmuFrames == 1 || lmuFrames % 300 == 0)) {
                            logLine("LMU_Data: frame %llu enviado (%zu bytes).",
                                    static_cast<unsigned long long>(lmuFrames),
                                    lmuBuffer.size());
                        }
                    }
                }
                lastLmu = now;
            }

            if (now - lastSmmp >= smmpInterval) {
                for (size_t i = 0; i < smmp.size(); ++i) {
                    const SmmpSpec& spec = kSmmpSpecs[i];
                    if (!smmp[i].open(spec.name)) {
                        if (!announced[spec.name]) {
                            announced[spec.name] = true;
                            logLine("Aviso: %s no existe todavia; se omitira.", spec.name);
                        }
                        continue;
                    }
                    announced[spec.name] = true;

                    size_t size = smmp[i].size ? smmp[i].size : spec.simhubSize;
                    if (size < 8) continue;
                    const uint32_t* srcHdr = reinterpret_cast<const uint32_t*>(smmp[i].view);
                    if (srcHdr[0] != srcHdr[1]) continue;

                    std::vector<uint8_t>& buf = smmpBuffers[i];
                    buf.resize(size);
                    std::memcpy(buf.data(), smmp[i].view, size);
                    const uint32_t* copyHdr = reinterpret_cast<const uint32_t*>(buf.data());
                    if (copyHdr[0] != copyHdr[1]) continue;

                    ++frame;
                    if (udp.sendFrame(kKindSmmp, spec.name, buf.data(), buf.size(), frame)) {
                        ++smmpFrames;
                        if (opt.verbose && (smmpFrames == 1 || smmpFrames % 100 == 0)) {
                            logLine("SMMP %s: frame %llu enviado (%zu bytes).",
                                    spec.name,
                                    static_cast<unsigned long long>(smmpFrames), size);
                        }
                    }
                }
                lastSmmp = now;
            }
        }

        logLine("Cierro espejo tras %llu frames; esperando a LMU.",
                static_cast<unsigned long long>(frames));
    }
}

struct NativeLockData {
    volatile LONG waiters;
    volatile LONG busy;
};

struct Assembly {
    bool active = false;
    uint32_t frame = 0;
    size_t total = 0;
    size_t received = 0;
    std::vector<uint8_t> data;
    std::vector<uint8_t> got;
};

struct ReceiverOptions {
    std::string bindHost = "127.0.0.1";
    int port = 8772;
    bool verbose = false;
};

int runReceiver(const ReceiverOptions& opt) {
    WSADATA wsa{};
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        logLine("ERROR: WSAStartup fallo: %d", WSAGetLastError());
        return 1;
    }
    SOCKET sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (sock == INVALID_SOCKET) {
        logLine("ERROR: no se pudo crear socket UDP: %d", WSAGetLastError());
        WSACleanup();
        return 1;
    }

    int rcvbuf = 8 * 1024 * 1024;
    setsockopt(sock, SOL_SOCKET, SO_RCVBUF,
               reinterpret_cast<const char*>(&rcvbuf), sizeof(rcvbuf));

    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(static_cast<u_short>(opt.port));
    addr.sin_addr.s_addr = inet_addr(opt.bindHost.c_str());
    if (addr.sin_addr.s_addr == INADDR_NONE) {
        logLine("ERROR: direccion de escucha invalida: %s", opt.bindHost.c_str());
        closesocket(sock);
        WSACleanup();
        return 1;
    }
    if (bind(sock, reinterpret_cast<const sockaddr*>(&addr), sizeof(addr)) == SOCKET_ERROR) {
        logLine("ERROR: no se pudo escuchar en %s:%d: %d",
                opt.bindHost.c_str(), opt.port, WSAGetLastError());
        closesocket(sock);
        WSACleanup();
        return 1;
    }

    HANDLE dataEvent = CreateEventA(nullptr, FALSE, FALSE, "LMU_Data_Event");
    if (!dataEvent) {
        logLine("ERROR: no se pudo crear LMU_Data_Event: %lu",
                static_cast<unsigned long>(GetLastError()));
    } else if (GetLastError() == ERROR_ALREADY_EXISTS) {
        logLine("LMU_Data_Event ya existia; se reutiliza.");
    }

    // Interfaz oficial de LMU: FFB-HUB y el SDK esperan Hold/Data y el lock.
    HANDLE holdEvent = CreateEventA(nullptr, FALSE, FALSE, "LMU_Data_HoldEvent");
    HANDLE nativeDataEvent = CreateEventA(nullptr, FALSE, FALSE, "LMU_Data_DataEvent");
    HANDLE lockEvent = CreateEventA(nullptr, FALSE, FALSE, "LMU_SharedMemoryLockEvent");
    HANDLE lockMapping = CreateFileMappingA(
        INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0, sizeof(NativeLockData),
        "LMU_SharedMemoryLockData");
    bool lockCreated = false;
    if (lockMapping) {
        lockCreated = GetLastError() != ERROR_ALREADY_EXISTS;
    }
    NativeLockData* lockData = nullptr;
    if (lockMapping) {
        lockData = static_cast<NativeLockData*>(
            MapViewOfFile(lockMapping, FILE_MAP_ALL_ACCESS, 0, 0, sizeof(NativeLockData)));
        if (lockData && lockCreated) {
            lockData->waiters = 0;
            lockData->busy = 0;
        }
    }
    if (!holdEvent || !nativeDataEvent || !lockData) {
        logLine("AVISO: no se pudieron crear los eventos/lock nativos de LMU: %lu",
                static_cast<unsigned long>(GetLastError()));
    } else {
        logLine("Eventos nativos de LMU listos (Hold/Data/lock).");
    }

    logLine("Receptor escuchando en udp://%s:%d; publicando LMU_Data y SMMP.",
            opt.bindHost.c_str(), opt.port);
    // FFB-HUB comprueba IsWindow(mAppWindow): el handle del juego no es valido
    // en el wineserver de Bottles. Publicamos una ventana oculta propia para
    // que el espejo pase esa comprobacion.
    HWND nativeWindow = CreateWindowExW(
        0, L"STATIC", L"Le Mans Ultimate", WS_OVERLAPPEDWINDOW,
        0, 0, 1280, 720, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
    if (!nativeWindow) {
        logLine("AVISO: no se pudo crear la ventana del espejo: %lu",
                static_cast<unsigned long>(GetLastError()));
    }

    WriteMapping lmuMapping;
    std::map<std::string, WriteMapping> smmpMappings;
    std::map<std::string, Assembly> assemblies;
    std::map<std::string, bool> announced;
    std::vector<uint8_t> packet(65536);
    uint64_t frames = 0;

    // Deja todos los mappings creados antes de que SimHub/CrewChief abran sus
    // lectores. Si se crean solo al llegar el primer frame, el lector rF2 de
    // SimHub puede fallar al abrir $rFactor2SMMP_ForceFeedback$ o Graphics.
    if (!lmuMapping.ensure("LMU_Data", sizeof(SharedMemoryObjectOut))) {
        logLine("ERROR: no se pudo crear LMU_Data inicial: %lu",
                static_cast<unsigned long>(GetLastError()));
    }
    for (const SmmpSpec& spec : kSmmpSpecs) {
        WriteMapping& primary = smmpMappings[spec.name];
        if (!primary.ensureExact(spec.name, spec.crewchiefSize)) {
            logLine("ERROR: no se pudo crear %s inicial: %lu", spec.name,
                    static_cast<unsigned long>(GetLastError()));
        } else {
            stampInitialHeader(primary, spec.partial, spec.crewchiefSize);
        }
        if (spec.alias && std::strcmp(spec.alias, spec.name) != 0) {
            WriteMapping& alias = smmpMappings[spec.alias];
            if (!alias.ensureExact(spec.alias, spec.simhubSize)) {
                logLine("ERROR: no se pudo crear %s inicial: %lu", spec.alias,
                        static_cast<unsigned long>(GetLastError()));
            } else {
                stampInitialHeader(alias, spec.partial, spec.simhubSize);
            }
        }
    }
    logLine("Mappings iniciales creados (LMU_Data, LMU y rFactor2 SMMP).");

    auto lockNative = [&]() {
        if (!lockData) return;
        for (int spins = 0; spins < 4000; ++spins) {
            if (InterlockedCompareExchange(&lockData->busy, 1, 0) == 0) return;
            YieldProcessor();
        }
        InterlockedIncrement(&lockData->waiters);
        while (InterlockedCompareExchange(&lockData->busy, 1, 0) != 0) {
            if (!lockEvent || WaitForSingleObject(lockEvent, 100) != WAIT_OBJECT_0) break;
        }
        InterlockedDecrement(&lockData->waiters);
    };
    auto unlockNative = [&]() {
        if (!lockData) return;
        InterlockedExchange(&lockData->busy, 0);
        if (lockData->waiters > 0 && lockEvent) SetEvent(lockEvent);
    };

    while (true) {
        sockaddr_in from{};
        int fromLen = sizeof(from);
        int n = recvfrom(sock, reinterpret_cast<char*>(packet.data()),
                         static_cast<int>(packet.size()), 0,
                         reinterpret_cast<sockaddr*>(&from), &fromLen);
        if (n < static_cast<int>(sizeof(PacketHeader))) continue;

        PacketHeader h{};
        std::memcpy(&h, packet.data(), sizeof(h));
        if (h.magic != kMagic || h.version != kVersion) continue;
        if (h.kind != kKindLmu && h.kind != kKindSmmp) continue;
        if (h.total == 0 || h.total > 16u * 1024u * 1024u) continue;
        if (h.offset > h.total || h.payload > h.total - h.offset) continue;
        if (static_cast<size_t>(n) < sizeof(PacketHeader) + h.payload) continue;

        std::string name;
        if (h.kind == kKindSmmp) {
            h.name[sizeof(h.name) - 1] = '\0';
            name = h.name;
            if (name.empty()) continue;
        }
        std::string key = std::string(1, static_cast<char>(h.kind)) + name;
        Assembly& a = assemblies[key];
        if (!a.active || a.frame != h.frame || a.total != h.total) {
            a.active = true;
            a.frame = h.frame;
            a.total = h.total;
            a.received = 0;
            a.data.assign(h.total, 0);
            a.got.assign(h.total, 0);
        }

        const uint8_t* payload = packet.data() + sizeof(PacketHeader);
        for (uint32_t i = 0; i < h.payload; ++i) {
            size_t pos = static_cast<size_t>(h.offset) + i;
            if (!a.got[pos]) {
                a.got[pos] = 1;
                a.data[pos] = payload[i];
                ++a.received;
            }
        }
        if (a.received != a.total) continue;

        if (h.kind == kKindLmu) {
            if (!lmuMapping.ensure("LMU_Data", a.total)) {
                logLine("ERROR: no se pudo crear/mapear LMU_Data (%zu bytes): %lu",
                        a.total, static_cast<unsigned long>(GetLastError()));
                a.active = false;
                continue;
            }
            if (holdEvent) SetEvent(holdEvent);
            lockNative();
            std::memcpy(lmuMapping.view, a.data.data(), a.total);
            auto* dst = reinterpret_cast<SharedMemoryObjectOut*>(lmuMapping.view);
            long num = dst->scoring.scoringInfo.mNumVehicles;
            if (num < 0) num = 0;
            if (num > 104) num = 104;
            dst->scoring.scoringInfo.mNumVehicles = num;
            // Los punteros del layout de LMU son direcciones absolutas del
            // proceso del juego. En otro proceso/prefijo no son validas, y
            // SimHub accede al array vehScoringInfo directamente. Se anulan
            // para que ningun consumidor intente desreferenciarlos.
            dst->scoring.scoringInfo.mVehicle = nullptr;
            dst->scoring.scoringInfo.mResultsStream = nullptr;
            if (nativeWindow) {
                dst->generic.appInfo.mAppWindow = nativeWindow;
            }
            unlockNative();
            if (nativeDataEvent) SetEvent(nativeDataEvent);
            if (dataEvent) SetEvent(dataEvent);
            ++frames;
            if (!announced["LMU_Data"]) {
                announced["LMU_Data"] = true;
                logLine("LMU_Data publicado (%zu bytes, %ld coches).",
                        a.total, num);
            } else if (opt.verbose && frames % 300 == 0) {
                logLine("LMU_Data frames=%llu, coches=%ld.",
                        static_cast<unsigned long long>(frames), num);
            }
        } else {
            if (a.total < 8) {
                a.active = false;
                continue;
            }
            const SmmpSpec* spec = nullptr;
            for (const SmmpSpec& candidate : kSmmpSpecs) {
                if (name == candidate.name) {
                    spec = &candidate;
                    break;
                }
            }
            const bool partial = spec ? spec->partial : true;

            auto publish = [&](const char* target, size_t wanted) -> bool {
                WriteMapping& wm = smmpMappings[target];
                if (!wm.ensureExact(target, wanted)) {
                    logLine("ERROR: no se pudo crear/mapear %s (%zu bytes): %lu",
                            target, wanted,
                            static_cast<unsigned long>(GetLastError()));
                    return false;
                }
                const size_t copy = std::min(a.total, wm.size);
                if (copy < 8) return false;
                uint32_t v = ++wm.version;
                if (v == 0) v = ++wm.version;
                uint32_t* hdr = reinterpret_cast<uint32_t*>(wm.view);
                hdr[0] = v;
                hdr[1] = v - 1;
                std::memcpy(wm.view + 8, a.data.data() + 8, copy - 8);
                hdr[1] = v;
                return true;
            };

            bool ok = publish(name.c_str(), spec ? spec->crewchiefSize : a.total);
            if (ok && spec && spec->alias && name != spec->alias) {
                ok = publish(spec->alias, std::max(spec->simhubSize, a.total));
            }
            if (!ok) {
                a.active = false;
                continue;
            }
            if (!announced[name]) {
                announced[name] = true;
                if (spec && spec->alias) {
                    logLine("%s publicado (%zu bytes; alias %s %zu).",
                            name.c_str(), spec->crewchiefSize, spec->alias,
                            spec->simhubSize);
                } else {
                    logLine("%s publicado (%zu bytes).", name.c_str(), a.total);
                }
            }
            (void)partial;
        }
        a.active = false;
    }

    if (dataEvent) CloseHandle(dataEvent);
    closesocket(sock);
    WSACleanup();
    return 0;
}

void usage(const char* argv0) {
    std::printf(
        "Uso:\n"
        "  %s --sender   [--udp 127.0.0.1:8772] [--hz 30] [--smmp-hz 10] [--verbose]\n"
        "  %s --receiver [--bind 127.0.0.1:8772] [--verbose]\n"
        "\n"
        "El emisor debe ejecutarse en el prefijo de LMU y el receptor en el de Bottles.\n"
        "Solo lectura: el emisor abre LMU_Data y $LMU.3.8.SMMP_* con FILE_MAP_READ.\n",
        argv0, argv0);
}

} // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        usage(argv[0]);
        return 1;
    }
    std::string mode = argv[1];
    if (mode == "--help" || mode == "-h") {
        usage(argv[0]);
        return 0;
    }

    if (mode == "--sender") {
        SenderOptions opt;
        for (int i = 2; i < argc; ++i) {
            std::string a = argv[i];
            if (a == "--udp" && i + 1 < argc) {
                if (!parseHostPort(argv[++i], opt.host, opt.port)) {
                    logLine("ERROR: --udp invalido");
                    return 1;
                }
            } else if (a == "--hz" && i + 1 < argc) {
                opt.hz = std::max(1, std::min(120, std::atoi(argv[++i])));
            } else if (a == "--smmp-hz" && i + 1 < argc) {
                opt.smmpHz = std::max(1, std::min(60, std::atoi(argv[++i])));
            } else if (a == "--verbose") {
                opt.verbose = true;
            } else if (a == "--help" || a == "-h") {
                usage(argv[0]);
                return 0;
            } else {
                logLine("ERROR: argumento desconocido: %s", a.c_str());
                return 1;
            }
        }
        return runSender(opt);
    }

    if (mode == "--receiver") {
        ReceiverOptions opt;
        for (int i = 2; i < argc; ++i) {
            std::string a = argv[i];
            if (a == "--bind" && i + 1 < argc) {
                if (!parseHostPort(argv[++i], opt.bindHost, opt.port)) {
                    logLine("ERROR: --bind invalido");
                    return 1;
                }
            } else if (a == "--verbose") {
                opt.verbose = true;
            } else if (a == "--help" || a == "-h") {
                usage(argv[0]);
                return 0;
            } else {
                logLine("ERROR: argumento desconocido: %s", a.c_str());
                return 1;
            }
        }
        return runReceiver(opt);
    }

    usage(argv[0]);
    return 1;
}

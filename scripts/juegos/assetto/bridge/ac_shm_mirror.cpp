// Espejo de solo lectura de la telemetria de Assetto Corsa entre prefijos Wine.
//
// Uso:
//   assetto_shm_mirror.exe --sender   --udp 127.0.0.1:8775 --hz 60
//   assetto_shm_mirror.exe --receiver --bind 127.0.0.1:8775
//
// El emisor se ejecuta en el prefijo de Assetto Corsa (Steam/Proton) y abre los
// cuatro mappings de telemetria con FILE_MAP_READ:
//   Local\acpmf_physics   (4096 bytes)
//   Local\acpmf_graphics  (4096 bytes)
//   Local\acpmf_static    (4096 bytes)
//   Local\acpmf_crewchief (16384 bytes, lo crea el plugin CrewChiefEx)
//   Local\acpmf_simhub_v2 (32768 bytes, lo crea la app SimHub)
//
// El receptor se ejecuta en el prefijo de Bottles (SimHub/CrewChief), crea sus
// propios mappings con los mismos nombres y tamanos y copia los bytes
// recibidos. El sentido es siempre Assetto Corsa -> UDP -> Bottles. No se reenvia
// ninguna escritura hacia el juego: el emisor nunca abre los mappings con
// permisos de escritura.
//
// packetId esta en el offset 0 de physics y graphics. El receptor escribe esos
// cuatro bytes al final de cada copia para que un lector que compara packetId
// antes/despues no acepte una copia a medias. El mapping static no tiene
// packetId; se copia completo.
//
// Compilacion:
//   zig c++ -target x86_64-windows-gnu -std=c++17 -O2 \
//     -o assetto_shm_mirror.exe ac_shm_mirror.cpp -lws2_32

#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <tlhelp32.h>

#include <algorithm>
#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

namespace {

constexpr uint32_t kMagic = 0x41435331u; // "ACS1"
constexpr uint16_t kVersion = 1;
constexpr size_t kMaxPayload = 60000;
constexpr size_t kMaxMapping = 16u * 1024u * 1024u;
constexpr int kMappingCount = 5;
constexpr int kDefaultHz = 60;

const char* const kMappingNames[kMappingCount] = {
    "Local\\acpmf_physics",
    "Local\\acpmf_graphics",
    "Local\\acpmf_static",
    "Local\\acpmf_crewchief",
    "Local\\acpmf_simhub_v2",
};

constexpr size_t kDefaultSizes[kMappingCount] = {
    4096,   // Local\acpmf_physics
    4096,   // Local\acpmf_graphics
    4096,   // Local\acpmf_static
    16384,  // Local\acpmf_crewchief (15660 bytes redondeados a pagina)
    32768,  // Local\acpmf_simhub_v2 (app SimHub: 30252 bytes)
};

constexpr bool kHasPacketId[kMappingCount] = {true, true, false, false, false};

#pragma pack(push, 1)
struct PacketHeader {
    uint32_t magic;
    uint16_t version;
    uint8_t mapping;
    uint8_t flags;
    uint32_t frame;
    uint32_t total;
    uint32_t offset;
    uint32_t payload;
};
#pragma pack(pop)

static_assert(sizeof(PacketHeader) == 24, "cabecera UDP inesperada");

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

// Proceso real de Assetto Corsa dentro del prefijo de Steam/Proton.
bool gameProcessAlive() {
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) return true;
    PROCESSENTRY32 pe{};
    pe.dwSize = sizeof(pe);
    bool found = false;
    if (Process32First(snap, &pe)) {
        do {
            if (_stricmp(pe.szExeFile, "acs.exe") == 0 ||
                _stricmp(pe.szExeFile, "acs_x86.exe") == 0 ||
                _stricmp(pe.szExeFile, "acs_dmm.exe") == 0 ||
                _stricmp(pe.szExeFile, "acs_pro.exe") == 0 ||
                _stricmp(pe.szExeFile, "acs") == 0) {
                found = true;
                break;
            }
        } while (Process32Next(snap, &pe));
    }
    CloseHandle(snap);
    return found;
}

struct ReadMapping {
    HANDLE handle = nullptr;
    const uint8_t* view = nullptr;
    size_t size = 0;

    bool open(const char* name) {
        close();
        handle = OpenFileMappingA(FILE_MAP_READ, FALSE, name);
        if (!handle) return false;
        view = static_cast<const uint8_t*>(
            MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0));
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
        if (size == 0 || size > kMaxMapping) {
            close();
            return false;
        }
        return true;
    }

    bool isOpen() const { return handle && view && size > 0; }

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

    bool ensure(const char* name, size_t wanted) {
        if (handle && view && size >= wanted) return true;
        close();
        if (wanted == 0 || wanted > kMaxMapping) return false;

        handle = CreateFileMappingA(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
                                    0, static_cast<DWORD>(wanted), name);
        if (!handle) return false;
        const bool existed = (GetLastError() == ERROR_ALREADY_EXISTS);
        view = static_cast<uint8_t*>(
            MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, 0));
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
        if (size < wanted) {
            logLine("ERROR: %s ya existe con %zu bytes, menos de %zu.",
                    name, size, wanted);
            close();
            return false;
        }
        if (existed) {
            logLine("Aviso: %s ya existia (%zu bytes); se reutiliza.",
                    name, size);
        }
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
    }

    ~WriteMapping() { close(); }
};

struct SenderOptions {
    std::string host = "127.0.0.1";
    int port = 8775;
    int hz = kDefaultHz;
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

    bool sendFrame(int mapping, const uint8_t* data, size_t size, uint32_t frame) {
        if (!data || size == 0 || mapping < 0 || mapping >= kMappingCount) return false;
        size_t off = 0;
        bool ok = true;
        while (off < size) {
            size_t chunk = std::min(kMaxPayload, size - off);
            PacketHeader h{};
            h.magic = kMagic;
            h.version = kVersion;
            h.mapping = static_cast<uint8_t>(mapping);
            h.flags = (off + chunk == size) ? 1 : 0;
            h.frame = frame;
            h.total = static_cast<uint32_t>(size);
            h.offset = static_cast<uint32_t>(off);
            h.payload = static_cast<uint32_t>(chunk);

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

    logLine("Emisor Assetto Corsa -> udp://%s:%d (%d Hz, mappings acpmf_*).",
            opt.host.c_str(), opt.port, opt.hz);

    ReadMapping mappings[kMappingCount];
    std::vector<uint8_t> buffers[kMappingCount];
    uint64_t lastWaitLog[kMappingCount] = {0, 0, 0};
    uint64_t lastGameCheck = 0;
    uint64_t lastSend = 0;
    uint32_t frame = 0;
    uint64_t sentFrames = 0;

    const int intervalMs = std::max(1, 1000 / std::max(1, opt.hz));

    while (true) {
        const uint64_t now = nowMs();

        if (now - lastGameCheck >= 2000) {
            lastGameCheck = now;
            if (!gameProcessAlive()) {
                bool any = false;
                for (int i = 0; i < kMappingCount; ++i) {
                    if (mappings[i].isOpen()) {
                        mappings[i].close();
                        any = true;
                    }
                }
                if (any) {
                    logLine("Assetto Corsa ya no esta en marcha; cierro mappings y espero.");
                }
                Sleep(2000);
                continue;
            }
        }

        if (now - lastSend < static_cast<uint64_t>(intervalMs)) {
            Sleep(1);
            continue;
        }
        lastSend = now;

        bool sentAny = false;
        for (int i = 0; i < kMappingCount; ++i) {
            if (!mappings[i].isOpen()) {
                if (!mappings[i].open(kMappingNames[i])) {
                    if (lastWaitLog[i] == 0 || now - lastWaitLog[i] >= 30000) {
                        logLine("Esperando a que Assetto Corsa cree %s...", kMappingNames[i]);
                        lastWaitLog[i] = now;
                    }
                    continue;
                }
                const size_t size = mappings[i].size;
                buffers[i].resize(size);
                logLine("%s abierto (mapping %zu bytes); espejando.",
                        kMappingNames[i], size);
            }

            const size_t size = mappings[i].size;
            if (size < 4 || size > kMaxMapping) {
                logLine("WARN: %s tiene un tamano inesperado (%zu); se cierra.",
                        kMappingNames[i], size);
                mappings[i].close();
                continue;
            }
            if (buffers[i].size() != size) buffers[i].resize(size);

            uint32_t seq1 = 0;
            uint32_t seq2 = 0;
            bool consistent = false;
            for (int attempt = 0; attempt < 3; ++attempt) {
                if (kHasPacketId[i]) {
                    seq1 = *reinterpret_cast<const uint32_t*>(mappings[i].view);
                }
                std::memcpy(buffers[i].data(), mappings[i].view, size);
                if (kHasPacketId[i]) {
                    seq2 = *reinterpret_cast<const uint32_t*>(mappings[i].view);
                } else {
                    seq2 = seq1;
                }
                if (seq1 == seq2) {
                    consistent = true;
                    break;
                }
            }
            if (!consistent) continue;

            if (udp.sendFrame(i, buffers[i].data(), size, frame)) {
                sentAny = true;
                if (opt.verbose && (sentFrames == 0 || (sentFrames % 600) == 0)) {
                    logLine("%s: frame %llu enviado (%zu bytes).",
                            kMappingNames[i],
                            static_cast<unsigned long long>(sentFrames), size);
                }
            }
        }

        if (sentAny) ++sentFrames;
        ++frame;
    }
}

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
    int port = 8775;
    size_t sizes[kMappingCount] = {kDefaultSizes[0], kDefaultSizes[1],
                                   kDefaultSizes[2], kDefaultSizes[3],
                                   kDefaultSizes[4]};
    bool verbose = false;
};

void publishMapping(int index, WriteMapping& mapping, const uint8_t* data, size_t total) {
    if (!mapping.view || mapping.size == 0) return;
    const size_t copySize = std::min(total, mapping.size);
    if (copySize == 0) return;

    if (kHasPacketId[index] && copySize >= sizeof(uint32_t)) {
        const uint32_t packetId =
            *reinterpret_cast<const uint32_t*>(data);
        // El packetId se publica al final: mientras se copia el resto, los
        // lectores siguen viendo el paquete anterior.
        std::memcpy(mapping.view + sizeof(uint32_t),
                    data + sizeof(uint32_t),
                    copySize - sizeof(uint32_t));
        *reinterpret_cast<uint32_t*>(mapping.view) = packetId;
    } else {
        std::memcpy(mapping.view, data, copySize);
    }
    if (copySize < mapping.size) {
        std::memset(mapping.view + copySize, 0, mapping.size - copySize);
    }
}

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

    WriteMapping mappings[kMappingCount];
    for (int i = 0; i < kMappingCount; ++i) {
        if (!mappings[i].ensure(kMappingNames[i], opt.sizes[i])) {
            logLine("ERROR: no se pudo crear/mapear %s: %lu",
                    kMappingNames[i],
                    static_cast<unsigned long>(GetLastError()));
            closesocket(sock);
            WSACleanup();
            return 1;
        }
        std::memset(mappings[i].view, 0, mappings[i].size);
    }
    for (int i = 0; i < kMappingCount; ++i) {
        logLine("Mapping %s creado (%zu bytes).", kMappingNames[i], mappings[i].size);
    }
    logLine("Mappings iniciales creados (%d).", kMappingCount);
    logLine("Receptor escuchando en udp://%s:%d; publicando acpmf_*.",
            opt.bindHost.c_str(), opt.port);

    Assembly assemblies[kMappingCount];
    std::vector<uint8_t> packet(65536);
    uint64_t frames[kMappingCount] = {0, 0, 0};
    bool announced[kMappingCount] = {false, false, false};

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
        if (h.mapping >= kMappingCount) continue;
        if (h.total == 0 || h.total > kMaxMapping) continue;
        if (h.offset > h.total || h.payload > h.total - h.offset) continue;
        if (static_cast<size_t>(n) < sizeof(PacketHeader) + h.payload) continue;

        const int idx = h.mapping;
        Assembly& a = assemblies[idx];
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

        if (a.total > mappings[idx].size) {
            if (!announced[idx]) {
                announced[idx] = true;
                logLine("Aviso: %s trae %zu bytes y el mapping %zu; se copian %zu.",
                        kMappingNames[idx], a.total, mappings[idx].size,
                        mappings[idx].size);
            }
        }
        publishMapping(idx, mappings[idx], a.data.data(),
                       std::min(a.total, mappings[idx].size));

        ++frames[idx];
        if (!announced[idx]) {
            announced[idx] = true;
            logLine("%s publicado (%zu bytes).", kMappingNames[idx],
                    std::min(a.total, mappings[idx].size));
        } else if (opt.verbose && frames[idx] % 600 == 0) {
            logLine("%s frames=%llu (%zu bytes).", kMappingNames[idx],
                    static_cast<unsigned long long>(frames[idx]),
                    std::min(a.total, mappings[idx].size));
        }
        a.active = false;
    }

    closesocket(sock);
    WSACleanup();
    return 0;
}

void usage(const char* argv0) {
    std::printf(
        "Uso:\n"
        "  %s --sender   [--udp 127.0.0.1:8775] [--hz 60] [--verbose]\n"
        "  %s --receiver [--bind 127.0.0.1:8775] [--sizes 4096,4096,4096,16384,32768] [--verbose]\n"
        "\n"
        "El emisor debe ejecutarse en el prefijo de Assetto Corsa y el receptor en el de Bottles.\n"
        "Solo lectura: el emisor abre Local\\acpmf_* con FILE_MAP_READ.\n",
        argv0, argv0);
}

bool parseSizes(const std::string& text, size_t out[kMappingCount]) {
    size_t start = 0;
    for (int i = 0; i < kMappingCount; ++i) {
        size_t comma = text.find(',', start);
        std::string part = text.substr(start, comma == std::string::npos
                                                  ? std::string::npos
                                                  : comma - start);
        if (part.empty()) return false;
        char* end = nullptr;
        unsigned long v = std::strtoul(part.c_str(), &end, 10);
        if (!end || *end != '\0' || v == 0 || v > kMaxMapping) return false;
        out[i] = static_cast<size_t>(v);
        if (i + 1 < kMappingCount) {
            if (comma == std::string::npos) return false;
            start = comma + 1;
        } else if (comma != std::string::npos) {
            return false;
        }
    }
    return true;
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
            } else if (a == "--sizes" && i + 1 < argc) {
                if (!parseSizes(argv[++i], opt.sizes)) {
                    logLine("ERROR: --sizes invalido");
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

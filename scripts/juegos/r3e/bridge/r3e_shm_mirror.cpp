// Espejo crudo de un mapping de Windows entre dos prefijos Wine.
// Para RaceRoom: nombre "$R3E", 45056 bytes.
//
//   r3e_shm_mirror.exe --sender   --name "$R3E" --udp 127.0.0.1:8777 --hz 30
//   r3e_shm_mirror.exe --receiver --name "$R3E" --bind 127.0.0.1:8777 --size 45056
//
// No interpreta los datos: copia bytes. El receptor crea el mapping en el
// prefijo de Bottles para que SimHub/CrewChief lo lean.
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <thread>
#include <vector>

namespace {
constexpr uint32_t kMagic = 0x52334542u; // "R3EB"
constexpr uint16_t kVersion = 1;
constexpr size_t kMaxPayload = 60000;

#pragma pack(push, 1)
struct Header {
    uint32_t magic;
    uint16_t version;
    uint16_t flags;
    uint32_t frame;
    uint32_t total;
    uint32_t offset;
    uint32_t payload;
    char name[64];
};
#pragma pack(pop)
static_assert(sizeof(Header) == 88, "cabecera inesperada");

void logLine(const char* fmt, ...) {
    va_list ap; va_start(ap, fmt); std::vfprintf(stderr, fmt, ap); va_end(ap);
    std::fputc('\n', stderr); std::fflush(stderr);
}

struct ReadMapping {
    HANDLE handle = nullptr;
    const uint8_t* view = nullptr;
    size_t size = 0;

    bool open(const std::string& name) {
        close();
        handle = OpenFileMappingA(FILE_MAP_READ, FALSE, name.c_str());
        if (!handle) return false;
        view = static_cast<const uint8_t*>(MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0));
        if (!view) { close(); return false; }
        MEMORY_BASIC_INFORMATION mbi{};
        if (!VirtualQuery(view, &mbi, sizeof(mbi))) { close(); return false; }
        size = static_cast<size_t>(mbi.RegionSize);
        return size > 0;
    }
    void close() {
        if (view) { UnmapViewOfFile(view); view = nullptr; }
        if (handle) { CloseHandle(handle); handle = nullptr; }
        size = 0;
    }
    ~ReadMapping() { close(); }
};

struct WriteMapping {
    HANDLE handle = nullptr;
    uint8_t* view = nullptr;
    size_t size = 0;

    bool ensure(const std::string& name, size_t wanted) {
        if (view && size >= wanted) return true;
        close();
        if (wanted == 0 || wanted > 16u * 1024u * 1024u) return false;
        // Si SimHub/CrewChief ya lo crearon, abrirlo; si no, crearlo.
        handle = OpenFileMappingA(FILE_MAP_ALL_ACCESS, FALSE, name.c_str());
        if (handle) {
            MEMORY_BASIC_INFORMATION mbi{};
            view = static_cast<uint8_t*>(MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, 0));
            if (view && VirtualQuery(view, &mbi, sizeof(mbi))) {
                size = static_cast<size_t>(mbi.RegionSize);
                if (size >= wanted) return true;
            }
            close();
        }
        handle = CreateFileMappingA(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
                                    0, static_cast<DWORD>(wanted), name.c_str());
        if (!handle) return false;
        view = static_cast<uint8_t*>(MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, wanted));
        if (!view) { close(); return false; }
        size = wanted;
        return true;
    }
    void close() {
        if (view) { UnmapViewOfFile(view); view = nullptr; }
        if (handle) { CloseHandle(handle); handle = nullptr; }
        size = 0;
    }
    ~WriteMapping() { close(); }
};

struct Options {
    std::string mode, name = "$R3E", host = "127.0.0.1", udp = "127.0.0.1:8777";
    int port = 8777, hz = 30;
    size_t size = 0;
    bool once = false, verbose = false;
};

bool parseHostPort(const std::string& text, std::string& host, int& port) {
    auto pos = text.rfind(':');
    if (pos == std::string::npos) return false;
    host = text.substr(0, pos);
    if (host.empty()) host = "127.0.0.1";
    int p = std::atoi(text.c_str() + pos + 1);
    if (p <= 0 || p > 65535) return false;
    port = p;
    return true;
}

int runSender(const Options& o) {
    WSADATA wsa{};
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) { logLine("WSAStartup fallo"); return 1; }
    SOCKET sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (sock == INVALID_SOCKET) { logLine("socket fallo"); return 1; }
    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(static_cast<u_short>(o.port));
    addr.sin_addr.s_addr = inet_addr(o.host.c_str());
    ReadMapping map;
    uint32_t frame = 0;
    bool warned = false;
    const int sleepMs = std::max(1, 1000 / std::max(1, o.hz));
    for (;;) {
        if (!map.open(o.name)) {
            if (!warned) { logLine("esperando al mapping %s", o.name.c_str()); warned = true; }
            std::this_thread::sleep_for(std::chrono::milliseconds(500));
            continue;
        }
        warned = false;
        if (o.verbose || frame == 0) logLine("%s abierto (%zu bytes)", o.name.c_str(), map.size);
        size_t offset = 0;
        while (offset < map.size) {
            size_t chunk = std::min(kMaxPayload, map.size - offset);
            Header h{};
            h.magic = kMagic; h.version = kVersion; h.frame = frame;
            h.total = static_cast<uint32_t>(map.size);
            h.offset = static_cast<uint32_t>(offset);
            h.payload = static_cast<uint32_t>(chunk);
            std::strncpy(h.name, o.name.c_str(), sizeof(h.name) - 1);
            std::vector<uint8_t> pkt(sizeof(h) + chunk);
            std::memcpy(pkt.data(), &h, sizeof(h));
            std::memcpy(pkt.data() + sizeof(h), map.view + offset, chunk);
            sendto(sock, reinterpret_cast<const char*>(pkt.data()),
                   static_cast<int>(pkt.size()), 0,
                   reinterpret_cast<const sockaddr*>(&addr), sizeof(addr));
            offset += chunk;
        }
        ++frame;
        if (o.once) break;
        std::this_thread::sleep_for(std::chrono::milliseconds(sleepMs));
    }
    return 0;
}

int runReceiver(const Options& o) {
    WSADATA wsa{};
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) { logLine("WSAStartup fallo"); return 1; }
    SOCKET sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (sock == INVALID_SOCKET) { logLine("socket fallo"); return 1; }
    int rcvbuf = 8 * 1024 * 1024;
    setsockopt(sock, SOL_SOCKET, SO_RCVBUF, reinterpret_cast<const char*>(&rcvbuf), sizeof(rcvbuf));
    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(static_cast<u_short>(o.port));
    addr.sin_addr.s_addr = inet_addr(o.host.c_str());
    if (bind(sock, reinterpret_cast<const sockaddr*>(&addr), sizeof(addr)) == SOCKET_ERROR) {
        logLine("bind fallo: %d", WSAGetLastError()); return 1;
    }
    logLine("receptor %s en udp://%s:%d", o.name.c_str(), o.host.c_str(), o.port);
    WriteMapping map;
    if (o.size && !map.ensure(o.name, o.size)) {
        logLine("no se pudo crear %s (%zu)", o.name.c_str(), o.size);
        return 1;
    }
    if (o.size) {
        logLine("Mappings iniciales creados (%s, %zu bytes)", o.name.c_str(), o.size);
    }
    std::vector<uint8_t> pkt(65536);
    uint32_t frames = 0;
    for (;;) {
        int n = recvfrom(sock, reinterpret_cast<char*>(pkt.data()),
                         static_cast<int>(pkt.size()), 0, nullptr, nullptr);
        if (n < static_cast<int>(sizeof(Header))) continue;
        Header h{};
        std::memcpy(&h, pkt.data(), sizeof(h));
        if (h.magic != kMagic || h.version != kVersion) continue;
        if (h.total == 0 || h.total > 16u * 1024u * 1024u) continue;
        if (h.payload > static_cast<uint32_t>(n - sizeof(h))) continue;
        if (!map.ensure(o.name, h.total)) {
            logLine("no se pudo crear %s (%u)", o.name.c_str(), h.total);
            continue;
        }
        size_t dest = h.offset;
        size_t copy = std::min<size_t>(h.payload, map.size > dest ? map.size - dest : 0);
        if (copy) std::memcpy(map.view + dest, pkt.data() + sizeof(h), copy);
        if (frames == 0) logLine("%s publicado (%u bytes)", o.name.c_str(), h.total);
        ++frames;
    }
    return 0;
}
} // namespace

int main(int argc, char** argv) {
    Options o;
    for (int i = 1; i < argc; ++i) {
        std::string a = argv[i];
        if (a == "--sender") o.mode = "sender";
        else if (a == "--receiver") o.mode = "receiver";
        else if (a == "--name" && i + 1 < argc) o.name = argv[++i];
        else if (a == "--udp" && i + 1 < argc) { o.udp = argv[++i]; parseHostPort(o.udp, o.host, o.port); }
        else if (a == "--bind" && i + 1 < argc) { o.udp = argv[++i]; parseHostPort(o.udp, o.host, o.port); }
        else if (a == "--hz" && i + 1 < argc) o.hz = std::atoi(argv[++i]);
        else if (a == "--size" && i + 1 < argc) o.size = static_cast<size_t>(std::strtoull(argv[++i], nullptr, 10));
        else if (a == "--once") o.once = true;
        else if (a == "--verbose") o.verbose = true;
        else if (a == "--help" || a == "-h") {
            std::printf("uso: %s --sender|--receiver [--name $R3E] [--udp host:port|--bind host:port] [--size N] [--hz N]\n", argv[0]);
            return 0;
        }
    }
    if (o.mode == "sender") return runSender(o);
    if (o.mode == "receiver") return runReceiver(o);
    std::fprintf(stderr, "falta --sender o --receiver\n");
    return 2;
}

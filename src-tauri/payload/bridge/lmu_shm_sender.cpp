// Enviador crudo de LMU_Data y $LMU.3.8.SMMP_* por UDP (protocolo del espejo).
// No interpreta la estructura de LMU: copia bytes, por lo que sobrevive a
// cambios de version del SDK. Sustituye al --sender de lmu_shm_mirror.exe.
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <thread>
#include <vector>

namespace {
constexpr uint32_t kMagic = 0x4C4D5542u;
constexpr uint16_t kVersion = 1;
constexpr uint8_t kKindLmu = 1;
constexpr uint8_t kKindSmmp = 2;
constexpr size_t kMaxPayload = 60000;

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
static_assert(sizeof(PacketHeader) == 88, "cabecera inesperada");

struct Mapping {
    HANDLE handle = nullptr;
    const uint8_t* view = nullptr;
    size_t size = 0;

    bool open(const char* name) {
        close();
        handle = OpenFileMappingA(FILE_MAP_READ, FALSE, name);
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
    ~Mapping() { close(); }
};

struct SmmpSpec { const char* name; const char* alias; };

const SmmpSpec kSmmp[] = {
    {"$LMU.3.8.SMMP_Telemetry$", "$rFactor2SMMP_Telemetry$"},
    {"$LMU.3.8.SMMP_Scoring$", "$rFactor2SMMP_Scoring$"},
    {"$LMU.3.8.SMMP_Rules$", "$rFactor2SMMP_Rules$"},
    {"$LMU.3.8.SMMP_Extended$", "$rFactor2SMMP_Extended$"},
    {"$LMU.3.8.SMMP_ForceFeedback$", "$rFactor2SMMP_ForceFeedback$"},
    {"$LMU.3.8.SMMP_Graphics$", "$rFactor2SMMP_Graphics$"},
};

void logLine(const char* fmt, ...) {
    va_list ap; va_start(ap, fmt); std::vfprintf(stderr, fmt, ap); va_end(ap);
    std::fputc('\n', stderr); std::fflush(stderr);
}

bool sendBuffer(SOCKET sock, const sockaddr_in& addr, uint8_t kind, const char* name,
                uint32_t frame, const uint8_t* data, size_t total) {
    size_t offset = 0;
    while (offset < total) {
        const size_t chunk = std::min(kMaxPayload, total - offset);
        PacketHeader h{};
        h.magic = kMagic; h.version = kVersion; h.kind = kind; h.flags = 0;
        h.frame = frame; h.total = static_cast<uint32_t>(total);
        h.offset = static_cast<uint32_t>(offset);
        h.payload = static_cast<uint32_t>(chunk);
        if (name) {
            std::strncpy(h.name, name, sizeof(h.name) - 1);
        }
        std::vector<uint8_t> packet(sizeof(h) + chunk);
        std::memcpy(packet.data(), &h, sizeof(h));
        std::memcpy(packet.data() + sizeof(h), data + offset, chunk);
        if (sendto(sock, reinterpret_cast<const char*>(packet.data()),
                   static_cast<int>(packet.size()), 0,
                   reinterpret_cast<const sockaddr*>(&addr), sizeof(addr)) == SOCKET_ERROR) {
            return false;
        }
        offset += chunk;
    }
    return true;
}

bool parseUdp(const std::string& text, sockaddr_in& addr) {
    auto pos = text.rfind(':');
    if (pos == std::string::npos) return false;
    std::string host = text.substr(0, pos);
    if (host.empty()) host = "127.0.0.1";
    int port = std::atoi(text.c_str() + pos + 1);
    if (port <= 0 || port > 65535) return false;
    addr = {};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(static_cast<u_short>(port));
    addr.sin_addr.s_addr = inet_addr(host.c_str());
    return addr.sin_addr.s_addr != INADDR_NONE;
}
} // namespace

int main(int argc, char** argv) {
    std::string udp = "127.0.0.1:8772";
    int hz = 30, smmpHz = 10;
    bool verbose = false, once = false;
    for (int i = 1; i < argc; ++i) {
        const std::string a = argv[i];
        if (a == "--udp" && i + 1 < argc) udp = argv[++i];
        else if (a == "--hz" && i + 1 < argc) hz = std::atoi(argv[++i]);
        else if (a == "--smmp-hz" && i + 1 < argc) smmpHz = std::atoi(argv[++i]);
        else if (a == "--verbose") verbose = true;
        else if (a == "--once") once = true;
        else if (a == "--sender") { /* compatibilidad con el script */ }
        else if (a == "--help" || a == "-h") {
            std::printf("Uso: %s --sender --udp host:port --hz N --smmp-hz N [--verbose] [--once]\n", argv[0]);
            return 0;
        }
    }
    if (hz < 1) hz = 30;
    if (smmpHz < 1) smmpHz = 10;

    WSADATA wsa{};
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) { logLine("ERROR: WSAStartup"); return 1; }
    SOCKET sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (sock == INVALID_SOCKET) { logLine("ERROR: socket"); return 1; }
    int sndbuf = 8 * 1024 * 1024;
    setsockopt(sock, SOL_SOCKET, SO_SNDBUF, reinterpret_cast<const char*>(&sndbuf), sizeof(sndbuf));
    sockaddr_in addr{};
    if (!parseUdp(udp, addr)) { logLine("ERROR: --udp invalido"); return 1; }
    logLine("Emisor crudo LMU -> udp://%s (LMU %d Hz, SMMP %d Hz)", udp.c_str(), hz, smmpHz);

    Mapping lmu; Mapping smmp[6];
    uint32_t frame = 0;
    uint64_t lmuFrames = 0;
    uint64_t smmpFrames[6] = {0};
    auto lastLmu = std::chrono::steady_clock::now();
    auto lastSmmp = lastLmu;
    auto lastTry = lastLmu;
    const auto lmuPeriod = std::chrono::milliseconds(1000 / hz);
    const auto smmpPeriod = std::chrono::milliseconds(1000 / smmpHz);

    while (true) {
        const auto now = std::chrono::steady_clock::now();
        if (now - lastTry > std::chrono::seconds(2)) {
            lastTry = now;
            if (!lmu.view) {
                if (lmu.open("LMU_Data") && verbose) logLine("LMU_Data abierto (%zu bytes)", lmu.size);
            }
            for (int i = 0; i < 6; ++i) {
                if (!smmp[i].view) smmp[i].open(kSmmp[i].name);
            }
        }
        if (lmu.view && now - lastLmu >= lmuPeriod) {
            lastLmu = now;
            ++frame;
            if (sendBuffer(sock, addr, kKindLmu, nullptr, frame, lmu.view, lmu.size)) {
                ++lmuFrames;
                if (verbose && (lmuFrames <= 3 || lmuFrames % 30 == 0))
                    logLine("LMU_Data frames=%llu bytes=%zu", static_cast<unsigned long long>(lmuFrames), lmu.size);
            } else {
                logLine("ERROR: sendto LMU_Data");
            }
        }
        if (now - lastSmmp >= smmpPeriod) {
            lastSmmp = now;
            ++frame;
            for (int i = 0; i < 6; ++i) {
                if (!smmp[i].view) continue;
                if (sendBuffer(sock, addr, kKindSmmp, kSmmp[i].name, frame, smmp[i].view, smmp[i].size)) {
                    ++smmpFrames[i];
                    if (verbose && (smmpFrames[i] <= 2 || smmpFrames[i] % 20 == 0))
                        logLine("SMMP %s frames=%llu bytes=%zu", kSmmp[i].name,
                                static_cast<unsigned long long>(smmpFrames[i]), smmp[i].size);
                }
                // El receptor publica tambien el alias $rFactor2SMMP_*.
                if (sendBuffer(sock, addr, kKindSmmp, kSmmp[i].alias, frame, smmp[i].view, smmp[i].size)) {
                    if (verbose && smmpFrames[i] == 1)
                        logLine("SMMP alias %s enviado", kSmmp[i].alias);
                }
            }
        }
        if (once) break;
        std::this_thread::sleep_for(std::chrono::milliseconds(2));
    }
    return 0;
}

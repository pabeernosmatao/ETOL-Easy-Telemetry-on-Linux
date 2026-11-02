// Espejo de solo lectura: /dev/shm/SCSTelemetry (juego nativo Linux) ->
// memoria con nombre de Windows dentro del prefijo de Bottles.
//
// Euro Truck Simulator 2 en Linux escribe la telemetria del SDK de SCS en un
// objeto de memoria compartida POSIX llamado /SCSTelemetry. SimHub, que corre
// bajo Wine en el prefijo de Bottles, espera un named mapping `Local\...`.
// Este proceso puente copia los bytes tal cual: el layout es identico en
// Windows y Linux (scs-telemetry-common.hpp), asi que no hay que convertir
// nada.
//
// Nunca escribe en el objeto del juego. Solo lee.
//
// Compilacion: build_ets2_mirror.sh (Zig, target x86_64-windows-gnu).

#include <windows.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

namespace {

struct Options {
    std::string source = "Z:\\dev\\shm\\SCSTelemetry";
    std::string map = "Local\\SCSTelemetry";
    std::string alias;
    DWORD size = 32 * 1024;
    DWORD interval_ms = 16;
};

void usage() {
    std::printf(
        "uso: ets2_shm_mirror.exe [opciones]\n"
        "\n"
        "  --source RUTA      fichero de memoria compartida del juego\n"
        "                     (defecto: Z:\\dev\\shm\\SCSTelemetry)\n"
        "  --map NOMBRE       mapping de Windows que se publica\n"
        "                     (defecto: Local\\SCSTelemetry)\n"
        "  --alias NOMBRE     publica una segunda copia con ese nombre\n"
        "                     (util para Local\\SHSCSTelemetry)\n"
        "  --size BYTES       tamano del mapping (defecto: 32768)\n"
        "  --interval MS      periodo de copia (defecto: 16)\n"
        "  -h, --help         esta ayuda\n");
}

bool parse(DWORD* target, const char* raw) {
    char* end = nullptr;
    unsigned long value = std::strtoul(raw, &end, 10);
    if (end == raw || *end != '\0' || value == 0) {
        return false;
    }
    *target = static_cast<DWORD>(value);
    return true;
}

Options parse_args(int argc, char** argv) {
    Options options;
    for (int i = 1; i < argc; ++i) {
        const std::string arg = argv[i];
        const bool has_value = i + 1 < argc;
        if (arg == "-h" || arg == "--help") {
            usage();
            std::exit(0);
        } else if (arg == "--source" && has_value) {
            options.source = argv[++i];
        } else if (arg == "--map" && has_value) {
            options.map = argv[++i];
        } else if (arg == "--alias" && has_value) {
            options.alias = argv[++i];
        } else if (arg == "--size" && has_value) {
            if (!parse(&options.size, argv[++i])) {
                std::fprintf(stderr, "ERROR: --size no valido\n");
                std::exit(2);
            }
        } else if (arg == "--interval" && has_value) {
            if (!parse(&options.interval_ms, argv[++i])) {
                std::fprintf(stderr, "ERROR: --interval no valido\n");
                std::exit(2);
            }
        } else {
            std::fprintf(stderr, "ERROR: opcion desconocida: %s\n", arg.c_str());
            usage();
            std::exit(2);
        }
    }
    return options;
}

struct Mapping {
    HANDLE handle = nullptr;
    void* view = nullptr;
    SIZE_T bytes = 0;
    bool created = false;
};

// Crea el mapping (o se engancha al que ya exista) y reserva una vista.
// Si SimHub lo creo antes, se usa su tamano real en lugar del pedido.
bool open_mapping(const std::string& name, DWORD size, Mapping* out) {
    SetLastError(0);
    HANDLE handle = CreateFileMappingA(
        INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0, size, name.c_str());
    if (handle == nullptr && GetLastError() == ERROR_ACCESS_DENIED) {
        // Ya existe con otros permisos: basta con abrirlo.
        handle = OpenFileMappingA(FILE_MAP_WRITE, FALSE, name.c_str());
    }
    if (handle == nullptr) {
        std::fprintf(stderr, "ERROR: no se pudo crear %s (codigo %lu)\n",
                     name.c_str(), GetLastError());
        return false;
    }

    out->created = GetLastError() != ERROR_ALREADY_EXISTS;
    // 0 = mapear hasta el final del objeto, sea cual sea su tamano.
    void* view = MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, 0);
    if (view == nullptr) {
        std::fprintf(stderr, "ERROR: no se pudo mapear %s (codigo %lu)\n",
                     name.c_str(), GetLastError());
        CloseHandle(handle);
        return false;
    }

    MEMORY_BASIC_INFORMATION info;
    SIZE_T bytes = size;
    if (VirtualQuery(view, &info, sizeof(info)) != 0) {
        bytes = info.RegionSize;
    }

    out->handle = handle;
    out->view = view;
    out->bytes = bytes;
    return true;
}

HANDLE open_source(const std::string& path) {
    return CreateFileA(
        path.c_str(), GENERIC_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
        OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
}

}  // namespace

int main(int argc, char** argv) {
    const Options options = parse_args(argc, argv);

    Mapping primary;
    if (!open_mapping(options.map, options.size, &primary)) {
        return 1;
    }
    std::printf("mapping %s listo (%zu bytes)\n", options.map.c_str(),
                static_cast<size_t>(primary.bytes));

    Mapping alias;
    if (!options.alias.empty() &&
        open_mapping(options.alias, options.size, &alias)) {
        std::printf("mapping %s listo (%zu bytes)\n", options.alias.c_str(),
                    static_cast<size_t>(alias.bytes));
    }

    std::printf("origen %s; esperando datos del juego...\n",
                options.source.c_str());
    std::fflush(stdout);

    auto* buffer = static_cast<unsigned char*>(std::malloc(primary.bytes));
    auto* previous = static_cast<unsigned char*>(std::malloc(primary.bytes));
    if (buffer == nullptr || previous == nullptr) {
        std::fprintf(stderr, "ERROR: sin memoria\n");
        return 1;
    }
    std::memset(previous, 0, primary.bytes);

    bool source_open = false;
    bool has_data = false;
    unsigned long empty_polls = 0;

    for (;;) {
        HANDLE file = open_source(options.source);
        if (file == INVALID_HANDLE_VALUE) {
            if (source_open) {
                std::printf("origen cerrado; esperando a que vuelva el juego\n");
                std::fflush(stdout);
                source_open = false;
            }
            Sleep(options.interval_ms);
            continue;
        }

        if (!source_open) {
            std::printf("origen abierto: %s\n", options.source.c_str());
            std::fflush(stdout);
            source_open = true;
        }

        DWORD read = 0;
        const BOOL read_ok = ReadFile(file, buffer, static_cast<DWORD>(primary.bytes),
                                      &read, nullptr);
        CloseHandle(file);

        if (!read_ok || read == 0) {
            ++empty_polls;
            // El fichero existe pero aun no tiene datos: no es un error.
            if (empty_polls == 200) {
                std::printf("origen sin datos todavia; sigo esperando\n");
                std::fflush(stdout);
            }
            Sleep(options.interval_ms);
            continue;
        }
        empty_polls = 0;

        const SIZE_T copy = read < primary.bytes ? read : primary.bytes;
        if (std::memcmp(buffer, previous, copy) != 0) {
            std::memcpy(primary.view, buffer, copy);
            if (alias.view != nullptr) {
                const SIZE_T alias_copy = copy < alias.bytes ? copy : alias.bytes;
                std::memcpy(alias.view, buffer, alias_copy);
            }
            std::memcpy(previous, buffer, copy);
            if (!has_data) {
                std::printf("primera copia de %zu bytes\n", static_cast<size_t>(copy));
                std::fflush(stdout);
                has_data = true;
            }
        }

        Sleep(options.interval_ms);
    }

    return 0;
}

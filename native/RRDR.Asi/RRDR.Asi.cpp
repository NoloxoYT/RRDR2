// RRDR.asi - native ScriptHookRDR2 loader for RRDR cores.
//
// This module is loaded by the ASI loader (dinput8.dll) next to RDR2.exe.
// It registers a script with ScriptHookRDR2, scans <game>/scripts for RRDR
// cores and drives them through the C ABI.
//
// No .NET, no ScriptHookRDR2DotNet: the core ABI is the same one the managed
// host uses, so a core DLL works with either loader.

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdarg.h>
#include <stdint.h>
// <string.h> is deliberately not included. With /GF and the CRT-less link used
// here, clang would replace memcpy/strrchr with intrinsics it then has no
// symbol for; the compiler still emits the calls on its own.

// The linker is driven without a default entry point and without the CRT. On
// its own it treats every COMDAT as unused and drops DllMain, which produced a
// 12 KB DLL with no code and no strings: the loader never ran. build_asi.sh
// pins the real entry with /entry:RustDllEntry /include:RustDllEntry; Main()
// stays reachable through DllAttach, which passes &Main to scriptRegister.

// ScriptHookRDR2 exports (C++ mangled names).
//
// The import library is generated from the DLL, so we resolve the symbols at
// runtime through GetProcAddress. That keeps the loader independent of the
// C++ mangling and of the link-time import library.
static void LogToFile(const char* message);

typedef void (*NativeInitFn)(uint64_t hash);
typedef void (*NativePush64Fn)(uint64_t value);
typedef uint64_t* (*NativeCallFn)();
typedef void (*ScriptRegisterFn)(HMODULE module, void (*fn)());
typedef void (*ScriptUnregisterFn)(void (*fn)());
typedef void (*ScriptWaitFn)(uint32_t waitTime);
// World pool exports (see PoolHandles below).
typedef int32_t (*WorldGetAllFn)(int32_t* output, int32_t capacity);

static NativeInitFn g_native_init = nullptr;
static NativePush64Fn g_native_push64 = nullptr;
static NativeCallFn g_native_call = nullptr;
static ScriptRegisterFn g_script_register = nullptr;
static ScriptUnregisterFn g_script_unregister = nullptr;
static ScriptWaitFn g_script_wait = nullptr;
static WorldGetAllFn g_world_get_all_peds = nullptr;
static WorldGetAllFn g_world_get_all_vehicles = nullptr;
static WorldGetAllFn g_world_get_all_objects = nullptr;

static void ResolveScriptHook() {
    HMODULE hook = GetModuleHandleA("ScriptHookRDR2.dll");
    if (!hook) {
        hook = LoadLibraryA("ScriptHookRDR2.dll");
    }
    if (!hook) {
        LogToFile("ScriptHookRDR2.dll not found");
        return;
    }

    g_native_init = reinterpret_cast<NativeInitFn>(
        GetProcAddress(hook, "?nativeInit@@YAX_K@Z"));
    g_native_push64 = reinterpret_cast<NativePush64Fn>(
        GetProcAddress(hook, "?nativePush64@@YAX_K@Z"));
    g_native_call = reinterpret_cast<NativeCallFn>(
        GetProcAddress(hook, "?nativeCall@@YAPEA_KXZ"));
    g_script_register = reinterpret_cast<ScriptRegisterFn>(
        GetProcAddress(hook, "?scriptRegister@@YAXPEAUHINSTANCE__@@P6AXXZ@Z"));
    g_script_unregister = reinterpret_cast<ScriptUnregisterFn>(
        GetProcAddress(hook, "?scriptUnregister@@YAXP6AXXZ@Z"));
    g_script_wait = reinterpret_cast<ScriptWaitFn>(
        GetProcAddress(hook, "?scriptWait@@YAXK@Z"));

    // World pool exports, used by PoolHandles (ABI v3). They are optional:
    // without them the cores fall back to nearby queries only, like a v2 host.
    g_world_get_all_peds = reinterpret_cast<WorldGetAllFn>(
        GetProcAddress(hook, "?worldGetAllPeds@@YAHPEAHH@Z"));
    g_world_get_all_vehicles = reinterpret_cast<WorldGetAllFn>(
        GetProcAddress(hook, "?worldGetAllVehicles@@YAHPEAHH@Z"));
    g_world_get_all_objects = reinterpret_cast<WorldGetAllFn>(
        GetProcAddress(hook, "?worldGetAllObjects@@YAHPEAHH@Z"));

    if (!g_native_init || !g_native_push64 || !g_native_call ||
        !g_script_register || !g_script_wait) {
        LogToFile("ScriptHookRDR2 exports missing");
    }
}

// ---- Shared ABI with rrdr-bridge -------------------------------------------

// Error codes as defined by rrdr-bridge's ffi.rs (docs/ABI.md): negative, so
// that `status != RD_OK` is the only comparison a loader ever needs. Do not
// test for a specific code here without updating that table in sync.
enum : int32_t {
    RD_OK = 0,
    RD_ERR_HOST = -1,
    RD_ERR_CALLBACK = -2,
    RD_ERR_INIT = -3,
    RD_ERR_NOT_INIT = -4,
    RD_ERR_PANIC = -5,
};

enum : uint32_t {
    ARG_I32 = 0,
    ARG_U32 = 1,
    ARG_F32 = 2,
    ARG_BOOL = 3,
    ARG_U64 = 4,
    ARG_STRING = 5,
    ARG_POINTER = 6,
};

enum : uint32_t {
    RETURN_U64 = 0,
    RETURN_VEC3 = 1,
    RETURN_STRING = 2,
};

#pragma pack(push, 8)
struct RdArg {
    uint32_t kind;
    uint32_t pad;
    uint64_t bits;
};

struct RdHost {
    void* user;
    uint64_t (*call_native)(void* user, uint64_t hash, const RdArg* args,
                            uint32_t arg_count, uint32_t return_kind,
                            void* return_buffer, uint32_t return_capacity);
    void (*log)(void* user, uint32_t level, const char* message, uint32_t length);
};

struct RdHostV3 {
    void* user;
    uint64_t (*call_native)(void* user, uint64_t hash, const RdArg* args,
                            uint32_t arg_count, uint32_t return_kind,
                            void* return_buffer, uint32_t return_capacity);
    void (*log)(void* user, uint32_t level, const char* message, uint32_t length);
    uint32_t (*pool_handles)(void* user, uint32_t kind, int32_t* output,
                             uint32_t capacity);
};
#pragma pack(pop)

typedef uint32_t (*RdAbiVersionFn)();
typedef int32_t (*RdInitFn)(const RdHost*, const char*);
typedef int32_t (*RdInitV3Fn)(const RdHostV3*, const char*);
typedef void (*RdTickFn)(float);
typedef void (*RdShutdownFn)();
typedef int32_t (*RdReloadFn)(const char*);

// ---- Loaded core -----------------------------------------------------------

struct Core {
    HMODULE module;
    RdTickFn tick;
    RdReloadFn reload;
    RdShutdownFn shutdown;
    char config_path[MAX_PATH];
    FILETIME config_time;
    char name[MAX_PATH];
};

static const int MAX_CORES = 32;
static Core g_cores[MAX_CORES];
static int g_core_count = 0;
static HMODULE g_self = nullptr;
static char g_mods_dir[MAX_PATH];

// ---- Minimal string helpers ------------------------------------------------

// Everything here is hand-written on purpose: calling the CRT versions
// (strlen, strncpy_s, _stricmp) pulls api-ms-win-crt-*.dll imports, and on Wine
// a single failed import makes the ASI load fail without any message.

static size_t OwnTextLength(const char* text) {
    size_t length = 0;
    while (text && text[length] != '\0') {
        ++length;
    }
    return length;
}

static void OwnCopyText(char* destination, size_t capacity, const char* source) {
    if (capacity == 0) {
        return;
    }

    size_t index = 0;
    for (; source && source[index] != '\0' && index + 1 < capacity; ++index) {
        destination[index] = source[index];
    }
    destination[index] = '\0';
}

static void OwnCopyBytes(void* destination, const void* source, size_t count) {
    unsigned char* out = static_cast<unsigned char*>(destination);
    const unsigned char* in = static_cast<const unsigned char*>(source);
    for (size_t index = 0; index < count; ++index) {
        out[index] = in[index];
    }
}

// Last occurrence of `needle`, or nullptr.
static char* OwnLastOf(char* text, char needle) {
    char* found = nullptr;
    for (size_t index = 0; text && text[index] != '\0'; ++index) {
        if (text[index] == needle) {
            found = text + index;
        }
    }
    return found;
}

static char OwnLowerChar(char value) {
    return (value >= 'A' && value <= 'Z') ? static_cast<char>(value - 'A' + 'a') : value;
}

static bool OwnEqualsIgnoreCase(const char* left, const char* right) {
    size_t index = 0;
    for (;;) {
        const char a = OwnLowerChar(left ? left[index] : '\0');
        const char b = OwnLowerChar(right ? right[index] : '\0');
        if (a != b) {
            return false;
        }
        if (a == '\0') {
            return true;
        }
        ++index;
    }
}

// Minimal formatter: only the specifiers this file uses (%s, %u, %02u, %lu).
// Returns the number of characters written, excluding the terminator.
static size_t OwnFormat(char* destination, size_t capacity, const char* format, ...) {
    if (capacity == 0) {
        return 0;
    }

    va_list args;
    va_start(args, format);

    size_t written = 0;
    for (size_t index = 0; format[index] != '\0' && written + 1 < capacity; ++index) {
        if (format[index] != '%') {
            destination[written++] = format[index];
            continue;
        }

        ++index;
        bool pad_zero = false;
        if (format[index] == '0') {
            pad_zero = true;
            ++index;
        }

        int width = 0;
        while (format[index] >= '0' && format[index] <= '9') {
            width = width * 10 + (format[index] - '0');
            ++index;
        }

        if (format[index] == 'l') {
            ++index;
        }

        if (format[index] == 's') {
            const char* text = va_arg(args, const char*);
            for (size_t position = 0; text && text[position] != '\0' && written + 1 < capacity;
                 ++position) {
                destination[written++] = text[position];
            }
        } else if (format[index] == 'u') {
            const unsigned long value = va_arg(args, unsigned long);
            char digits[24];
            int count = 0;
            unsigned long remaining = value;
            do {
                digits[count++] = static_cast<char>('0' + (remaining % 10));
                remaining /= 10;
            } while (remaining != 0 && count < 23);

            for (int pad = count; pad < width && written + 1 < capacity; ++pad) {
                destination[written++] = pad_zero ? '0' : ' ';
            }
            while (count > 0 && written + 1 < capacity) {
                destination[written++] = digits[--count];
            }
        } else if (format[index] == '%') {
            destination[written++] = '%';
        }
    }

    va_end(args);
    destination[written] = '\0';
    return written;
}

// ---- Logging ---------------------------------------------------------------

// Opens a log file next to the cores. Win32 only: the CRT (fopen) pulls
// api-ms-win-crt-*.dll imports, and on Wine a failed import makes the whole ASI
// fail to load silently: no log, no script, nothing to debug with.
//
// The log is capped: a mod that logs every tick would otherwise grow the file
// without bound over a long session. When it passes the cap it is deleted and
// starts fresh (no rename dance: the file is a diagnostic, not a record).
static const DWORD LOG_MAX_BYTES = 8 * 1024 * 1024;

static void AppendToFile(const char* path, const char* text) {
    WIN32_FILE_ATTRIBUTE_DATA data;
    if (GetFileAttributesExA(path, GetFileExInfoStandard, &data)) {
        ULARGE_INTEGER size;
        size.HighPart = data.nFileSizeHigh;
        size.LowPart = data.nFileSizeLow;
        if (size.QuadPart > LOG_MAX_BYTES) {
            DeleteFileA(path);
        }
    }

    HANDLE file = CreateFileA(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
                              nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        return;
    }

    SetFilePointer(file, 0, nullptr, FILE_END);

    DWORD written = 0;
    const DWORD length = static_cast<DWORD>(OwnTextLength(text));
    WriteFile(file, text, length, &written, nullptr);
    CloseHandle(file);
}

static void LogToFile(const char* message) {
    if (g_mods_dir[0] == '\0' || message == nullptr) {
        return;
    }

    char path[MAX_PATH];
    OwnFormat(path, sizeof(path), "%s\\RRDR.asi.log", g_mods_dir);

    SYSTEMTIME now;
    GetLocalTime(&now);

    char line[2300];
    OwnFormat(line, sizeof(line), "[%02u:%02u:%02u.%03u] %s\r\n",
                now.wHour, now.wMinute, now.wSecond, now.wMilliseconds, message);

    AppendToFile(path, line);
}

// The core passes a slice, not a C string: copying `length` bytes avoids
// reading past the end of the message (which used to print garbage and could
// fault on unlucky layouts).
static void LogLevel(void* user, uint32_t level, const char* message, uint32_t length) {
    (void)user;
    (void)level;

    if (!message) {
        LogToFile("(null)");
        return;
    }

    char buffer[2048];
    uint32_t copy = length < sizeof(buffer) - 1 ? length : static_cast<uint32_t>(sizeof(buffer) - 1);
    OwnCopyBytes(buffer, message, copy);
    buffer[copy] = '\0';

    LogToFile(buffer);
}

static void LogMessage(const char* prefix, const char* detail) {
    char buffer[2048];
    OwnFormat(buffer, sizeof(buffer), "%s%s", prefix, detail ? detail : "");
    LogToFile(buffer);
}

// ---- Native bridge ---------------------------------------------------------

static uint64_t CallNative(void* user, uint64_t hash, const RdArg* args,
                           uint32_t arg_count, uint32_t return_kind,
                           void* return_buffer, uint32_t return_capacity) {
    (void)user;

    // ScriptHookRDR2 has no per-call return type: every native is invoked by
    // pushing its hash and arguments, then reading the return slot. Typed
    // results are reconstructed here from that raw 64-bit value.
    if (!g_native_init || !g_native_push64 || !g_native_call) {
        return 0;
    }

    g_native_init(hash);

    for (uint32_t index = 0; index < arg_count; ++index) {
        const RdArg& arg = args[index];
        g_native_push64(arg.bits);
    }

    uint64_t* result = g_native_call();
    uint64_t raw = result ? *result : 0;

    if (return_kind == RETURN_VEC3 && return_buffer && return_capacity >= 12) {
        // ScriptHookRDR2 stores a Vector3 in three 64-bit slots: x, y and z are
        // at offsets 0, 8 and 16 (see ScriptHookRDR2DotNet ObjectFromNative).
        // Copy them as floats into the 12-byte buffer the core allocated.
        uint64_t slots[3] = {0, 0, 0};
        if (result) {
            OwnCopyBytes(slots, result, sizeof(slots));
        }

        float values[3];
        for (int axis = 0; axis < 3; ++axis) {
            // Each slot holds the float in its low 32 bits.
            uint32_t bits = static_cast<uint32_t>(slots[axis] & 0xFFFFFFFFu);
            OwnCopyBytes(&values[axis], &bits, sizeof(uint32_t));
        }
        OwnCopyBytes(return_buffer, values, sizeof(values));
        return 12;
    } else if (return_kind == RETURN_STRING && return_buffer && return_capacity > 0) {
        // The native returns a string pointer in the return slot.
        const char* text = reinterpret_cast<const char*>(static_cast<uintptr_t>(raw));
        if (text) {
            uint32_t length = 0;
            while (length + 1 < return_capacity && text[length] != '\0') {
                ++length;
            }
            OwnCopyBytes(return_buffer, text, length);
            static_cast<char*>(return_buffer)[length] = '\0';
            return length;
        }
        static_cast<char*>(return_buffer)[0] = '\0';
    }

    return raw;
}

static uint32_t PoolHandles(void* user, uint32_t kind, int32_t* output,
                            uint32_t capacity) {
    (void)user;

    if (!output || capacity == 0) {
        return 0;
    }

    // The ScriptHookRDR2 world exports have a fixed-size internal buffer, so
    // cap the request to a sane pool size.
    const int32_t limit = capacity > 4096 ? 4096 : static_cast<int32_t>(capacity);

    WorldGetAllFn getter = nullptr;
    switch (kind) {
        case 0:  // PoolKind::Ped
            getter = g_world_get_all_peds;
            break;
        case 1:  // PoolKind::Vehicle
            getter = g_world_get_all_vehicles;
            break;
        case 2:  // PoolKind::Object
            getter = g_world_get_all_objects;
            break;
        default:
            return 0;
    }

    if (!getter) {
        // Exports missing (older ScriptHookRDR2): no global pools, the cores
        // fall back to nearby queries only.
        return 0;
    }

    const int32_t count = getter(output, limit);
    return count > 0 ? static_cast<uint32_t>(count) : 0;
}

// ---- Core loading ----------------------------------------------------------

static char g_game_dir[MAX_PATH] = {0};
static bool g_reload_configs = true;

// Comma-separated DLL names from RRDR.asi.ini `disable =`, upper-cased and
// NUL-separated for OwnEqualsIgnoreCase comparisons.
static const int MAX_DISABLED = 32;
static char g_disabled[MAX_DISABLED][64];
static int g_disabled_count = 0;

// Reads RRDR.asi.ini next to the game exe, so keys stay optional.
static void LoadAsiConfig() {
    char ini[MAX_PATH];
    OwnFormat(ini, sizeof(ini), "%sRRDR.asi.ini", g_game_dir);

    char value[MAX_PATH];

    DWORD length = GetPrivateProfileStringA(
        "RRDR", "mods_dir", "", value, sizeof(value), ini);
    if (length > 0) {
        // Absolute when it starts with a drive letter (either separator) or a
        // UNC/leading-slash form. Forward slashes work on Windows too, so
        // `mods_dir = C:/Games/...` must not be glued onto the game dir.
        const bool drive =
            value[1] == ':' && (value[2] == '\\' || value[2] == '/');
        if (drive || value[0] == '\\' || value[0] == '/') {
            OwnCopyText(g_mods_dir, sizeof(g_mods_dir), value);
        } else {
            OwnFormat(g_mods_dir, sizeof(g_mods_dir), "%s%s", g_game_dir, value);
        }
    } else {
        OwnFormat(g_mods_dir, sizeof(g_mods_dir), "%sscripts\\Rscripts", g_game_dir);
    }

    // The result is only read as a flag; the length is not needed.
    GetPrivateProfileStringA(
        "RRDR", "reload_config", "true", value, sizeof(value), ini);
    g_reload_configs = !OwnEqualsIgnoreCase(value, "false") && !OwnEqualsIgnoreCase(value, "0");

    // disable = Foo.dll,Bar.dll: comma-separated DLL names to skip.
    g_disabled_count = 0;
    GetPrivateProfileStringA("RRDR", "disable", "", value, sizeof(value), ini);
    char* cursor = value;
    while (*cursor != '\0' && g_disabled_count < MAX_DISABLED) {
        while (*cursor == ' ' || *cursor == ',') {
            ++cursor;
        }
        if (*cursor == '\0') {
            break;
        }

        char* entry = g_disabled[g_disabled_count];
        size_t index = 0;
        while (*cursor != '\0' && *cursor != ',' && index + 1 < sizeof(g_disabled[0])) {
            entry[index++] = OwnLowerChar(*cursor++);
        }
        entry[index] = '\0';
        if (index > 0) {
            ++g_disabled_count;
        }
    }
}

// True when `name` (a DLL file name) is on the disable list.
static bool IsDisabled(const char* name) {
    for (int index = 0; index < g_disabled_count; ++index) {
        if (OwnEqualsIgnoreCase(name, g_disabled[index])) {
            return true;
        }
    }
    return false;
}

static void BuildModsDir() {
    GetModuleFileNameA(nullptr, g_game_dir, MAX_PATH);

    char* slash = OwnLastOf(g_game_dir, '\\');
    if (slash) {
        *(slash + 1) = '\0';
    }

    // Default location, then let the ini override it.
    OwnFormat(g_mods_dir, sizeof(g_mods_dir), "%sscripts\\Rscripts", g_game_dir);
    LoadAsiConfig();
    CreateDirectoryA(g_mods_dir, nullptr);
}

static void RememberConfigTime(Core& core) {
    WIN32_FILE_ATTRIBUTE_DATA data;
    if (GetFileAttributesExA(core.config_path, GetFileExInfoStandard, &data)) {
        core.config_time = data.ftLastWriteTime;
    } else {
        core.config_time.dwLowDateTime = 0;
        core.config_time.dwHighDateTime = 0;
    }
}

static void LoadCore(const char* path) {
    if (g_core_count >= MAX_CORES) {
        LogMessage("too many cores, skipping ", path);
        return;
    }

    HMODULE module = LoadLibraryA(path);
    if (!module) {
        LogMessage("LoadLibrary failed: ", path);
        return;
    }

    RdAbiVersionFn abi_version = reinterpret_cast<RdAbiVersionFn>(
        GetProcAddress(module, "rd_abi_version"));
    RdTickFn tick = reinterpret_cast<RdTickFn>(
        GetProcAddress(module, "rd_tick"));
    RdShutdownFn shutdown = reinterpret_cast<RdShutdownFn>(
        GetProcAddress(module, "rd_shutdown"));
    RdReloadFn reload = reinterpret_cast<RdReloadFn>(
        GetProcAddress(module, "rd_reload_config"));

    // rd_abi_version doubles as the "is this an RRDR core" probe: a DLL
    // without it is not ours and is released silently. A core that has the ABI
    // but is missing tick/shutdown is a broken build worth a log line.
    if (!abi_version) {
        FreeLibrary(module);
        return;
    }

    if (!tick || !shutdown) {
        LogMessage("core missing exports: ", path);
        FreeLibrary(module);
        return;
    }

    uint32_t abi = abi_version();
    if (abi < 2 || abi > 3) {
        LogMessage("unsupported core ABI: ", path);
        FreeLibrary(module);
        return;
    }

    Core& core = g_cores[g_core_count];
    memset(&core, 0, sizeof(core));
    core.module = module;
    core.tick = tick;
    core.reload = reload;
    core.shutdown = shutdown;

    // <core>.ini next to the DLL; empty string disables the config path.
    char* separator = OwnLastOf(const_cast<char*>(path), '\\');
    const char* file_name = separator ? separator + 1 : path;
    char stem[MAX_PATH];
    OwnFormat(stem, sizeof(stem), "%s", file_name);
    char* dot = OwnLastOf(stem, '.');
    if (dot) {
        *dot = '\0';
    }
    OwnFormat(core.config_path, sizeof(core.config_path), "%s\\%s.ini", g_mods_dir, stem);

    const char* config = GetFileAttributesA(core.config_path) != INVALID_FILE_ATTRIBUTES
                             ? core.config_path
                             : "";

    RdHostV3 host = {};
    host.user = nullptr;
    host.call_native = &CallNative;
    host.log = &LogLevel;
    host.pool_handles = &PoolHandles;

    int32_t status = RD_ERR_INIT;

    if (abi >= 3) {
        RdInitV3Fn init = reinterpret_cast<RdInitV3Fn>(
            GetProcAddress(module, "rd_init_v3"));
        if (init) {
            status = init(&host, config);
        }
    } else {
        RdInitFn init = reinterpret_cast<RdInitFn>(
            GetProcAddress(module, "rd_init"));
        if (init) {
            RdHost host2 = {};
            host2.user = nullptr;
            host2.call_native = &CallNative;
            host2.log = &LogLevel;
            status = init(&host2, config);
        }
    }

    if (status != RD_OK) {
        LogMessage("core init failed: ", path);
        FreeLibrary(module);
        return;
    }

    RememberConfigTime(core);
    OwnCopyText(core.name, sizeof(core.name), file_name);
    ++g_core_count;

    LogMessage("loaded core: ", file_name);
}

static void ScanMods() {
    char pattern[MAX_PATH];
    OwnFormat(pattern, sizeof(pattern), "%s\\*.dll", g_mods_dir);

    WIN32_FIND_DATAA data;
    HANDLE find = FindFirstFileA(pattern, &data);
    if (find == INVALID_HANDLE_VALUE) {
        return;
    }

    do {
        if (data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            continue;
        }

        char path[MAX_PATH];
        OwnFormat(path, sizeof(path), "%s\\%s", g_mods_dir, data.cFileName);

        if (IsDisabled(data.cFileName)) {
            LogMessage("disabled, skipping: ", data.cFileName);
            continue;
        }

        LoadCore(path);
    } while (FindNextFileA(find, &data));

    FindClose(find);
}

// Polls the per-core .ini files for changes.
//
// The check is throttled to once every 2 seconds, like CoreHost.CheckConfigReload
// on the managed side: a per-frame GetFileAttributesExA on every core's ini is a
// classic micro-stutter source on HDDs, network drives and Wine.
static void ReloadConfigs() {
    if (!g_reload_configs) {
        return;
    }

    static LARGE_INTEGER frequency = {};
    static LONGLONG next_check = 0;
    if (frequency.QuadPart == 0) {
        QueryPerformanceFrequency(&frequency);
        next_check = 0;
    }

    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    if (next_check > now.QuadPart) {
        return;
    }
    next_check = now.QuadPart + frequency.QuadPart * 2;

    for (int index = 0; index < g_core_count; ++index) {
        Core& core = g_cores[index];
        if (!core.reload || core.config_path[0] == '\0') {
            continue;
        }

        WIN32_FILE_ATTRIBUTE_DATA data;
        if (!GetFileAttributesExA(core.config_path, GetFileExInfoStandard, &data)) {
            continue;
        }

        if (data.ftLastWriteTime.dwLowDateTime == core.config_time.dwLowDateTime &&
            data.ftLastWriteTime.dwHighDateTime == core.config_time.dwHighDateTime) {
            continue;
        }

        core.config_time = data.ftLastWriteTime;
        core.reload(core.config_path);
    }
}

// ---- Script entry ----------------------------------------------------------

// Seconds since the previous call, measured with the high-resolution counter.
//
// A core that receives 0 for ever never advances its timers: the same natives
// end up called every frame and the game ends up hitching. The delta is
// clamped so a long stall (loading screen, alt-tab) does not hand the cores a
// multi-second step either.
static double SecondsSinceLastTick() {
    static LARGE_INTEGER frequency = {};
    if (frequency.QuadPart == 0) {
        QueryPerformanceFrequency(&frequency);
    }

    static LONGLONG last = 0;

    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);

    if (last == 0) {
        last = now.QuadPart;
        return 0.0;
    }

    const double seconds = frequency.QuadPart > 0
                               ? static_cast<double>(now.QuadPart - last) /
                                     static_cast<double>(frequency.QuadPart)
                               : 0.0;
    last = now.QuadPart;

    if (seconds < 0.0) {
        return 0.0;
    }
    return seconds > 0.25 ? 0.25 : seconds;
}

static void Main() {
    LogToFile("RRDR.asi main() entered");

    // BuildModsDir already ran in DLL_PROCESS_ATTACH; running it again here
    // would repeat the ini reads and directory creation every session.
    ScanMods();

    char count[16];
    OwnFormat(count, sizeof(count), "%d", g_core_count);
    LogMessage("cores loaded: ", count);

    for (;;) {
        const float dt = static_cast<float>(SecondsSinceLastTick());

        for (int index = 0; index < g_core_count; ++index) {
            if (g_cores[index].tick) {
                g_cores[index].tick(dt);
            }
        }

        ReloadConfigs();
        if (g_script_wait) {
            g_script_wait(0);
        }
    }
}

// ---- ASI entry -------------------------------------------------------------

static BOOL WINAPI DllAttach(HMODULE module, DWORD reason, LPVOID reserved);

// Real entry point. With /noentry the linker sets the address of entry 0 and
// Windows never calls DllMain: the ASI "loads" and does nothing. This stub is
// what the linker is told to start at, and it forwards to DllMain.
extern "C" BOOL WINAPI RustDllEntry(HMODULE module, DWORD reason, LPVOID reserved) {
    return DllAttach(module, reason, reserved);
}

static BOOL WINAPI DllAttach(HMODULE module, DWORD reason, LPVOID reserved) {
    (void)reserved;

    switch (reason) {
        case DLL_PROCESS_ATTACH:
            g_self = module;
            DisableThreadLibraryCalls(module);
            BuildModsDir();
            ResolveScriptHook();
            if (g_script_register) {
                g_script_register(module, &Main);
            }
            break;

        case DLL_PROCESS_DETACH:
            if (g_script_unregister) {
                g_script_unregister(&Main);
            }
            for (int index = 0; index < g_core_count; ++index) {
                if (g_cores[index].shutdown) {
                    g_cores[index].shutdown();
                }
                if (g_cores[index].module) {
                    FreeLibrary(g_cores[index].module);
                }
            }
            g_core_count = 0;
            break;
    }

    return TRUE;
}

// Conventional name, in case something calls it by name.
BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID reserved) {
    return DllAttach(module, reason, reserved);
}

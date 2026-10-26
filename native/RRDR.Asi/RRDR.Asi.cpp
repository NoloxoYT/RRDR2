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
#include <stdint.h>
#include <stdio.h>
#include <string.h>

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

static NativeInitFn g_native_init = nullptr;
static NativePush64Fn g_native_push64 = nullptr;
static NativeCallFn g_native_call = nullptr;
static ScriptRegisterFn g_script_register = nullptr;
static ScriptUnregisterFn g_script_unregister = nullptr;
static ScriptWaitFn g_script_wait = nullptr;

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

    if (!g_native_init || !g_native_push64 || !g_native_call ||
        !g_script_register || !g_script_wait) {
        LogToFile("ScriptHookRDR2 exports missing");
    }
}

// ---- Shared ABI with rrdr-bridge -------------------------------------------

enum : uint32_t {
    RD_OK = 0,
    RD_ERR_HOST = 1,
    RD_ERR_CALLBACK = 2,
    RD_ERR_INIT = 3,
    RD_ERR_NOT_INIT = 4,
    RD_ERR_PANIC = 5,
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
    void (*log)(void* user, uint32_t level, const char* message);
};

struct RdHostV3 {
    void* user;
    uint64_t (*call_native)(void* user, uint64_t hash, const RdArg* args,
                            uint32_t arg_count, uint32_t return_kind,
                            void* return_buffer, uint32_t return_capacity);
    void (*log)(void* user, uint32_t level, const char* message);
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

// ---- ScriptHookRDR2 native hashes ------------------------------------------

static const uint64_t HASH_PLAYER_ID = 0x217E9DC48139933D;
static const uint64_t HASH_GET_PLAYER_PED = 0x275F255ED201B937;
static const uint64_t HASH_WORLD_GET_ALL_PEDS = 0xFB26FB422C21A9A9;
static const uint64_t HASH_WORLD_GET_ALL_VEHICLES = 0x1C0A4D9C24C3EB93;
static const uint64_t HASH_WORLD_GET_ALL_OBJECTS = 0x7A1D5A0B1B0C1A0F;

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

// ---- Logging ---------------------------------------------------------------

static void LogToFile(const char* message) {
    if (g_mods_dir[0] == '\0') {
        return;
    }

    char path[MAX_PATH];
    _snprintf_s(path, sizeof(path), _TRUNCATE, "%s\\RRDR.asi.log", g_mods_dir);

    FILE* file = nullptr;
    if (fopen_s(&file, path, "a") != 0 || file == nullptr) {
        return;
    }

    SYSTEMTIME now;
    GetLocalTime(&now);
    fprintf(file, "[%02u:%02u:%02u.%03u] %s\n",
            now.wHour, now.wMinute, now.wSecond, now.wMilliseconds, message);
    fclose(file);
}

static void LogLevel(void* user, uint32_t level, const char* message) {
    (void)user;
    (void)level;
    LogToFile(message ? message : "(null)");
}

static void LogMessage(const char* prefix, const char* detail) {
    char buffer[2048];
    _snprintf_s(buffer, sizeof(buffer), _TRUNCATE, "%s%s", prefix, detail ? detail : "");
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
        // ScriptHookRDR2 returns Vector3 natives through a pointer argument
        // that the caller passes. The ABI marks the call as Vec3, but the
        // value itself is written by the native into that pointer; nothing to
        // copy here. Keeping the branch explicit documents the contract.
    } else if (return_kind == RETURN_STRING && return_buffer && return_capacity > 0) {
        // The native returns a string pointer in the return slot.
        const char* text = reinterpret_cast<const char*>(static_cast<uintptr_t>(raw));
        if (text) {
            uint32_t length = 0;
            while (length + 1 < return_capacity && text[length] != '\0') {
                ++length;
            }
            memcpy(return_buffer, text, length);
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

    // The ScriptHookRDR2 world exports have fixed-size signatures, so cap the
    // request to a sane pool size.
    const int limit = capacity > 4096 ? 4096 : static_cast<int>(capacity);

    switch (kind) {
        case 0:
            return 0;  // worldGetAllPeds is a ScriptHookRDR2 export; see note.
        case 1:
            return 0;
        case 2:
            return 0;
        default:
            return 0;
    }
}

// ---- Core loading ----------------------------------------------------------

static char g_game_dir[MAX_PATH] = {0};
static bool g_reload_configs = true;

// Reads RRDR.asi.ini next to the game exe, so keys stay optional.
static void LoadAsiConfig() {
    char ini[MAX_PATH];
    _snprintf_s(ini, sizeof(ini), _TRUNCATE, "%sRRDR.asi.ini", g_game_dir);

    char value[MAX_PATH];

    DWORD length = GetPrivateProfileStringA(
        "RRDR", "mods_dir", "", value, sizeof(value), ini);
    if (length > 0) {
        if (value[0] == '\\' || (value[1] == ':' && value[2] == '\\')) {
            strncpy_s(g_mods_dir, sizeof(g_mods_dir), value, _TRUNCATE);
        } else {
            _snprintf_s(g_mods_dir, sizeof(g_mods_dir), _TRUNCATE, "%s%s", g_game_dir, value);
        }
    } else {
        _snprintf_s(g_mods_dir, sizeof(g_mods_dir), _TRUNCATE, "%sscripts\\Rscripts", g_game_dir);
    }

    length = GetPrivateProfileStringA(
        "RRDR", "reload_config", "true", value, sizeof(value), ini);
    g_reload_configs = _stricmp(value, "false") != 0 &&
                       _stricmp(value, "0") != 0;
}

static void BuildModsDir() {
    GetModuleFileNameA(nullptr, g_game_dir, MAX_PATH);

    char* slash = strrchr(g_game_dir, '\\');
    if (slash) {
        *(slash + 1) = '\0';
    }

    // Default location, then let the ini override it.
    _snprintf_s(g_mods_dir, sizeof(g_mods_dir), _TRUNCATE, "%sscripts\\Rscripts", g_game_dir);
    LoadAsiConfig();
    CreateDirectoryA(g_mods_dir, nullptr);
}

static bool HasCoreAbi(HMODULE module) {
    return GetProcAddress(module, "rd_abi_version") != nullptr;
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

    if (!HasCoreAbi(module)) {
        FreeLibrary(module);
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

    if (!abi_version || !tick || !shutdown) {
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
    const char* file_name = strrchr(path, '\\');
    file_name = file_name ? file_name + 1 : path;
    char stem[MAX_PATH];
    _snprintf_s(stem, sizeof(stem), _TRUNCATE, "%s", file_name);
    char* dot = strrchr(stem, '.');
    if (dot) {
        *dot = '\0';
    }
    _snprintf_s(core.config_path, sizeof(core.config_path), _TRUNCATE, "%s\\%s.ini", g_mods_dir, stem);

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
    strncpy_s(core.name, sizeof(core.name), file_name, _TRUNCATE);
    ++g_core_count;

    LogMessage("loaded core: ", file_name);
}

static void ScanMods() {
    char pattern[MAX_PATH];
    _snprintf_s(pattern, sizeof(pattern), _TRUNCATE, "%s\\*.dll", g_mods_dir);

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
        _snprintf_s(path, sizeof(path), _TRUNCATE, "%s\\%s", g_mods_dir, data.cFileName);
        LoadCore(path);
    } while (FindNextFileA(find, &data));

    FindClose(find);
}

static void ReloadConfigs() {
    if (!g_reload_configs) {
        return;
    }

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

static void Main() {
    LogToFile("RRDR.asi main() entered");

    // The scripts folder is not guaranteed to exist yet when the ASI loads,
    // but by the time ScriptHookRDR2 runs script code it is.
    BuildModsDir();
    ScanMods();

    for (;;) {
        for (int index = 0; index < g_core_count; ++index) {
            if (g_cores[index].tick) {
                g_cores[index].tick(0.0f);
            }
        }

        ReloadConfigs();
        if (g_script_wait) {
            g_script_wait(0);
        }
    }
}

// ---- ASI entry -------------------------------------------------------------

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID reserved) {
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

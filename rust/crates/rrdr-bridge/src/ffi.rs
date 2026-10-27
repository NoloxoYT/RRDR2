//! C ABI shared with `RRDR.Host.dll`.
//!
//! The ABI is intentionally tiny and versioned. Adding a native call never
//! changes the ABI: everything goes through `call_native(hash, args)`.

use crate::game::{Arg, GameApi, LogLevel, PoolKind, Vec3};
use crate::Mod;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

pub const ABI_VERSION: u32 = 3;

pub const ARG_I32: u32 = 0;
pub const ARG_U32: u32 = 1;
pub const ARG_F32: u32 = 2;
pub const ARG_BOOL: u32 = 3;
pub const ARG_U64: u32 = 4;
pub const ARG_STR: u32 = 5;
pub const ARG_PTR: u32 = 6;

pub const RD_OK: i32 = 0;
pub const RD_ERR_HOST: i32 = -1;
pub const RD_ERR_CALLBACK: i32 = -2;
pub const RD_ERR_INIT: i32 = -3;
pub const RD_ERR_PANIC: i32 = -4;
pub const RD_ERR_NOT_INIT: i32 = -5;

/// Return kinds understood by the managed host.
pub const RET_U64: u32 = 0;
/// Vector3 return: the loader writes 12 bytes (three f32) into the caller's
/// buffer and returns 12. ScriptHookRDR2 hands vectors back as three 64-bit
/// slots (1.0, 2.0, 3.0 read at offsets 0, 8 and 16), not as packed floats.
pub const RET_VEC3: u32 = 1;
pub const RET_STRING: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RdArg {
    pub kind: u32,
    pub pad: u32,
    pub bits: u64,
}

pub type CallNativeFn = unsafe extern "C" fn(
    user: *mut c_void,
    hash: u64,
    args: *const RdArg,
    argc: u32,
    return_kind: u32,
    return_buffer: *mut u8,
    return_capacity: u32,
) -> u64;

pub type LogFn = unsafe extern "C" fn(user: *mut c_void, level: u32, message: *const u8, len: u32);

/// Optional global-pool callback added in ABI v3.
///
/// `output` points to an array of `capacity` i32 handles. The callback returns
/// the number of handles written (clamped by the caller).
pub type PoolHandlesFn =
    unsafe extern "C" fn(user: *mut c_void, kind: u32, output: *mut i32, capacity: u32) -> u32;

/// Host vtable passed by the managed shim to `rd_init` (ABI v2).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RdHost {
    pub user: *mut c_void,
    pub call_native: Option<CallNativeFn>,
    pub log: Option<LogFn>,
}

/// Host vtable passed by the managed shim to `rd_init_v3` (ABI v3).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RdHostV3 {
    pub user: *mut c_void,
    pub call_native: Option<CallNativeFn>,
    pub log: Option<LogFn>,
    /// Optional global pool provider. `None` falls back to nearby queries only.
    pub pool_handles: Option<PoolHandlesFn>,
}

/// Size of the buffer the host copies returned strings into. Shared with
/// `docs/ABI.md`; change both sides together.
pub const MAX_STRING_RETURN: usize = 4096;

impl From<RdHost> for RdHostV3 {
    fn from(host: RdHost) -> Self {
        Self {
            user: host.user,
            call_native: host.call_native,
            log: host.log,
            pool_handles: None,
        }
    }
}

/// [`GameApi`] implementation that forwards to the managed host.
pub struct FfiHost {
    host: RdHostV3,
    // Scratch reused across calls (§: the host is only used from the script
    // tick thread and always behind `&mut`, so reuse is safe). Keeping them in
    // the struct avoids one allocation per native call on hot paths.
    strings: Vec<CString>,
    raw: Vec<RdArg>,
    string_buffer: Vec<u8>,
}

// The host object is only used from the script tick thread. The pointer is
// opaque to Rust.
unsafe impl Send for FfiHost {}

impl FfiHost {
    pub fn new(host: RdHostV3) -> Self {
        Self {
            host,
            strings: Vec::new(),
            raw: Vec::new(),
            string_buffer: Vec::new(),
        }
    }

    fn invoke(
        &mut self,
        hash: u64,
        args: &[Arg<'_>],
        return_kind: u32,
        return_buffer: *mut u8,
        return_capacity: u32,
    ) -> u64 {
        let Some(call_native) = self.host.call_native else {
            return 0;
        };

        // CString heap buffers stay alive for the whole native call. The Vec is
        // reused across calls; moving a CString inside it does not move its
        // heap allocation.
        self.strings.clear();
        self.raw.clear();

        for argument in args {
            let (kind, bits) = match argument {
                Arg::I32(value) => (ARG_I32, *value as u32 as u64),
                Arg::U32(value) => (ARG_U32, *value as u64),
                Arg::F32(value) => (ARG_F32, value.to_bits() as u64),
                Arg::Bool(value) => (ARG_BOOL, *value as u64),
                Arg::U64(value) => (ARG_U64, *value),
                Arg::Ptr(value) => (ARG_PTR, *value),
                Arg::Str(value) => {
                    let string = match CString::new(*value) {
                        Ok(string) => string,
                        Err(_) => {
                            // Interior NUL: the native would see a truncated
                            // string. Refuse rather than silently sending an
                            // empty one.
                            self.log(
                                LogLevel::Warn,
                                "string argument contains an interior NUL and was dropped",
                            );
                            continue;
                        }
                    };
                    let pointer = string.as_ptr() as u64;
                    self.strings.push(string);
                    (ARG_STR, pointer)
                }
            };

            self.raw.push(RdArg { kind, pad: 0, bits });
        }

        // SAFETY: the function pointer comes from the managed host, and all
        // argument pointers are valid for the duration of the call.
        unsafe {
            call_native(
                self.host.user,
                hash,
                self.raw.as_ptr(),
                self.raw.len() as u32,
                return_kind,
                return_buffer,
                return_capacity,
            )
        }
    }
}

impl GameApi for FfiHost {
    fn call(&mut self, hash: u64, args: &[Arg<'_>]) -> u64 {
        self.invoke(hash, args, RET_U64, std::ptr::null_mut(), 0)
    }

    fn call_vec3(&mut self, hash: u64, args: &[Arg<'_>]) -> Vec3 {
        let mut value = Vec3::default();
        self.invoke(
            hash,
            args,
            RET_VEC3,
            &mut value as *mut Vec3 as *mut u8,
            std::mem::size_of::<Vec3>() as u32,
        );
        value
    }

    fn call_string(&mut self, hash: u64, args: &[Arg<'_>]) -> String {
        // Take the buffer out of `self` for the call: `invoke` needs `&mut
        // self`, and the raw pointer must not alias it while borrowed.
        let mut buffer = std::mem::take(&mut self.string_buffer);
        buffer.clear();
        buffer.resize(MAX_STRING_RETURN, 0);

        let length = self.invoke(
            hash,
            args,
            RET_STRING,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        );
        let length = (length as usize).min(buffer.len());
        let text = String::from_utf8_lossy(&buffer[..length]).into_owned();
        self.string_buffer = buffer;
        text
    }

    fn pool_handles(&mut self, kind: PoolKind, capacity: usize) -> Option<Vec<i32>> {
        let provider = self.host.pool_handles?;
        let capacity = capacity.clamp(1, 4096);
        let mut buffer = vec![0i32; capacity];

        // SAFETY: the provider comes from the managed host and writes at most
        // `capacity` i32 values into the live buffer.
        let count = unsafe {
            provider(
                self.host.user,
                kind as u32,
                buffer.as_mut_ptr(),
                capacity as u32,
            )
        };

        let count = (count as usize).min(capacity);
        buffer.truncate(count);
        Some(buffer)
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        if let Some(log) = self.host.log {
            // SAFETY: message points to a live UTF-8 slice for the duration of
            // the callback.
            unsafe {
                log(
                    self.host.user,
                    level as u32,
                    message.as_ptr(),
                    message.len() as u32,
                );
            }
        } else {
            eprintln!("[rrdr-bridge] {level:?}: {message}");
        }
    }
}

pub struct State<T: Mod> {
    host: FfiHost,
    module: T,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

unsafe fn read_path(pointer: *const c_char) -> Option<PathBuf> {
    if pointer.is_null() {
        return None;
    }

    let text = CStr::from_ptr(pointer).to_string_lossy();
    let text = text.trim();

    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    }
}

/// Shared init implementation for ABI v2 and v3.
///
/// # Safety
/// `config_path` must be null or a valid NUL-terminated UTF-8 string.
unsafe fn init_with_host<T: Mod>(
    state: &Mutex<Option<State<T>>>,
    host_snapshot: Option<RdHostV3>,
    config_path: *const c_char,
) -> i32 {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(host_value) = host_snapshot else {
            return RD_ERR_HOST;
        };
        if host_value.call_native.is_none() {
            return RD_ERR_CALLBACK;
        }

        let mut ffi = FfiHost::new(host_value);
        let path = read_path(config_path);

        // Shut down a previous instance outside the lock: shutdown calls back
        // into managed code, which must not re-enter the mutex.
        let previous = {
            let mut guard = lock(state);
            guard.take()
        };
        if let Some(mut previous) = previous {
            previous.module.shutdown(&mut previous.host);
        }

        match T::init(&mut ffi, path.as_deref()) {
            Ok(module) => {
                ffi.log(
                    LogLevel::Info,
                    &format!("Rust core initialized (bridge ABI v{ABI_VERSION})"),
                );
                *lock(state) = Some(State { host: ffi, module });
                RD_OK
            }
            Err(error) => {
                ffi.log(LogLevel::Error, &format!("mod init failed: {error}"));
                RD_ERR_INIT
            }
        }
    }));

    match outcome {
        Ok(code) => code,
        Err(_) => {
            if let Some(host) = host_snapshot {
                let mut ffi = FfiHost::new(host);
                ffi.log(LogLevel::Error, "panic during rd_init");
            }
            RD_ERR_PANIC
        }
    }
}

/// Implementation behind the generated `rd_init` (ABI v2, no pool provider).
///
/// # Safety
/// `host` must point to a valid `RdHost` and `config_path` must be null or a
/// valid NUL-terminated UTF-8 string.
pub unsafe fn init<T: Mod>(
    state: &Mutex<Option<State<T>>>,
    host: *const RdHost,
    config_path: *const c_char,
) -> i32 {
    let snapshot = if host.is_null() {
        None
    } else {
        Some(RdHostV3::from(*host))
    };
    init_with_host(state, snapshot, config_path)
}

/// Implementation behind the generated `rd_init_v3`.
///
/// # Safety
/// `host` must point to a valid `RdHostV3` and `config_path` must be null or a
/// valid NUL-terminated UTF-8 string.
pub unsafe fn init_v3<T: Mod>(
    state: &Mutex<Option<State<T>>>,
    host: *const RdHostV3,
    config_path: *const c_char,
) -> i32 {
    let snapshot = if host.is_null() { None } else { Some(*host) };
    init_with_host(state, snapshot, config_path)
}

/// Implementation behind the generated `rd_tick`.
pub fn tick<T: Mod>(state: &Mutex<Option<State<T>>>, dt: f32) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut guard = lock(state);
        if let Some(state) = guard.as_mut() {
            state.module.tick(&mut state.host, dt);
        }
    }));

    if outcome.is_err() {
        let mut guard = lock(state);
        if let Some(state) = guard.as_mut() {
            state.host.log(LogLevel::Error, "panic during rd_tick");
        }
    }
}

/// Implementation behind the generated `rd_shutdown`.
pub fn shutdown<T: Mod>(state: &Mutex<Option<State<T>>>) {
    let previous = {
        let mut guard = lock(state);
        guard.take()
    };

    if let Some(mut state) = previous {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state.module.shutdown(&mut state.host);
        }));

        if outcome.is_err() {
            state.host.log(LogLevel::Error, "panic during rd_shutdown");
        }

        state.host.log(LogLevel::Info, "Rust core shut down");
    }
}

/// Implementation behind the generated `rd_reload_config`.
///
/// # Safety
/// `config_path` must be null or a valid NUL-terminated UTF-8 string.
pub unsafe fn reload<T: Mod>(state: &Mutex<Option<State<T>>>, config_path: *const c_char) -> i32 {
    let path = read_path(config_path);

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut guard = lock(state);
        let Some(state) = guard.as_mut() else {
            return RD_ERR_NOT_INIT;
        };

        state.module.reload(&mut state.host, path.as_deref());
        state.host.log(LogLevel::Info, "configuration reloaded");
        RD_OK
    }));

    match outcome {
        Ok(code) => code,
        Err(_) => {
            let mut guard = lock(state);
            if let Some(state) = guard.as_mut() {
                state
                    .host
                    .log(LogLevel::Error, "panic during rd_reload_config");
            }
            RD_ERR_PANIC
        }
    }
}

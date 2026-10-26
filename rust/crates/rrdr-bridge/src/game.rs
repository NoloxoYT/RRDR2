//! Minimal game abstraction.
//!
//! The core never talks to ScriptHook directly. The managed shim implements a
//! native-call callback, and tests / the simulator implement the same trait with
//! a fake game. That is the whole "mock" contract.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
}

/// Global entity pool kind for [`GameApi::pool_handles`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PoolKind {
    /// Ped pool (`ScriptHookRDR2.dll!worldGetAllPeds`).
    Ped = 0,
    /// Vehicle pool (`worldGetAllVehicles`).
    Vehicle = 1,
    /// Object/prop pool (`worldGetAllObjects`).
    Object = 2,
}

/// One argument to a native call.
///
/// Values are kept typed so the managed side can build the correct
/// ScriptHook `InputArgument` (float bits are *not* interchangeable with ints).
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    I32(i32),
    U32(u32),
    F32(f32),
    Bool(bool),
    U64(u64),
    Str(&'a str),
    /// Raw address, used for output parameters. The callee writes through it
    /// synchronously before the native call returns.
    Ptr(u64),
}

pub trait GameApi {
    /// Invoke a native by hash. Returns the raw 64-bit result slot.
    fn call(&mut self, hash: u64, args: &[Arg<'_>]) -> u64;

    fn log(&mut self, level: LogLevel, message: &str);

    fn call_i32(&mut self, hash: u64, args: &[Arg<'_>]) -> i32 {
        self.call(hash, args) as u32 as i32
    }

    fn call_u32(&mut self, hash: u64, args: &[Arg<'_>]) -> u32 {
        self.call(hash, args) as u32
    }

    fn call_f32(&mut self, hash: u64, args: &[Arg<'_>]) -> f32 {
        f32::from_bits(self.call(hash, args) as u32)
    }

    fn call_bool(&mut self, hash: u64, args: &[Arg<'_>]) -> bool {
        self.call(hash, args) != 0
    }

    /// Call a native that returns a Vector3 (entity coords, rotations, ...).
    ///
    /// A Vector3 does not fit in the 64-bit result slot, so the managed host
    /// copies it into a 12-byte buffer owned by the caller.
    fn call_vec3(&mut self, hash: u64, args: &[Arg<'_>]) -> Vec3;

    /// Call a native that returns a UTF-8 string.
    ///
    /// The host copies up to a fixed-size buffer (4 KiB in the current
    /// implementation); longer strings are truncated.
    fn call_string(&mut self, hash: u64, args: &[Arg<'_>]) -> String;

    /// Query a global entity pool from the managed host.
    ///
    /// Returns `None` when the host does not provide a pool provider (ABI v2
    /// hosts and the offline mock do not). The returned handles are raw and
    /// still need to be validated by the caller.
    fn pool_handles(&mut self, _kind: PoolKind, _capacity: usize) -> Option<Vec<i32>> {
        None
    }
}

/// 3D vector used by native bindings.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

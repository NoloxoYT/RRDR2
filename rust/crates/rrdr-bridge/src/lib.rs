//! Rust side of the ScriptHookRDR2DotNet bridge.
//!
//! A Rust mod implements [`Mod`] and calls [`export_mod!`]. The generated
//! `rd_*` exports are loaded by the generic `RRDR.Host.dll` managed script.
//!
//! All natives are invoked through the [`GameApi`] callback provided by the
//! managed host, so this crate never links against ScriptHookRDR2 or .NET.

pub mod ffi;
pub mod game;
pub mod memory;

#[macro_use]
mod macros;

pub use ffi::{RdArg, RdHost, RdHostV3, ABI_VERSION};
pub use game::{Arg, GameApi, LogLevel, PoolKind, Vec3};

use std::path::Path;

/// A Rust mod loaded by `RRDR.Host.dll`.
///
/// Implementations must be `Send` because the host stores them in a static
/// mutex. In practice mods are plain data structures, so this is automatic.
pub trait Mod: Sized + Send {
    /// Called once when `rd_init` is invoked. `config_path` is the optional path
    /// passed by the managed host.
    fn init(api: &mut dyn GameApi, config_path: Option<&Path>) -> Result<Self, String>;

    /// Called every game tick. `dt` is the frame delta in seconds.
    fn tick(&mut self, api: &mut dyn GameApi, dt: f32);

    /// Called when the managed host detects a config file change.
    fn reload(&mut self, api: &mut dyn GameApi, config_path: Option<&Path>) {
        let _ = (api, config_path);
    }

    /// Called before the DLL is unloaded. Restore any global game state here.
    fn shutdown(&mut self, api: &mut dyn GameApi) {
        let _ = api;
    }
}

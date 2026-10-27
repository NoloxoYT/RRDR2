//! High-level Rust API for Red Dead Redemption 2.
//!
//! This crate sits on top of `rrdr-natives` and provides typed, non-owning
//! wrappers around the game's entities:
//!
//! ```ignore
//! use rrdr_game::prelude::*;
//!
//! fn tick(api: &mut dyn GameApi) {
//!     let Some(player) = Player::local(api) else { return };
//!     let Some(ped) = player.ped(api) else { return };
//!
//!     if ped.health(api) < 25 {
//!         ped.set_health(api, 25);
//!     }
//! }
//! ```
//!
//! Handles are `Copy` weak references, not ownership tokens. They can become
//! invalid or be recycled by the game at any time. Revalidate the player ped
//! every tick and avoid holding handles across long periods when identity
//! matters. Cleanup is explicit: `delete`, `release`, and `remove` take
//! `&mut dyn GameApi`; there is no `Drop` that calls natives.
//!
//! All natives must be called on the script thread. `GameApi` is passed as
//! `&mut dyn GameApi` and must not be stored in wrappers or sent to another
//! thread.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod anim_scene;
pub mod audio;
pub mod blip;
pub mod camera;
pub mod entity;
pub mod environment;
pub mod error;
pub mod game;
pub mod math;
pub mod model;
pub mod ped;
pub mod pickup;
pub mod player;
pub mod pool;
pub mod prelude;
pub mod prop;
pub mod raycast;
pub mod rope;
// `scratch` is an internal detail; it is deliberately not in the prelude.
mod scratch;
pub mod task;
pub mod traits;
pub mod ui;
pub mod vehicle;
pub mod volume;
pub mod weapon;
pub mod world;

#[cfg(test)]
mod test_support;

pub use error::{Error, Result};
pub use math::Vec3Ops;
pub use pool::PoolOptions;

pub use rrdr_bridge::{Arg, GameApi, LogLevel, PoolKind, Vec3};

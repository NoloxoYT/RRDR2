//! Rope wrapper.

use crate::entity::Entity;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::physics as n_physics;

/// A physics rope handle.
///
/// Non-owning: the game may break or delete the rope and recycle the handle.
/// Use [`Rope::exists`] before relying on a stored handle.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rope(i32);

impl Rope {
    /// Wrap a raw rope handle. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Wrap a raw rope handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(raw)
    }

    /// Wrap a raw handle after checking `DOES_ROPE_EXIST`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        let rope = Self::from_raw(raw)?;
        rope.exists(api).then_some(rope)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// `PHYSICS::DOES_ROPE_EXIST`.
    pub fn exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_physics::DOES_ROPE_EXIST(api, self.0)
    }

    /// Number of physics vertices (`PHYSICS::GET_ROPE_VERTEX_COUNT`).
    pub fn vertex_count(self, api: &mut dyn GameApi) -> i32 {
        if self.0 == 0 {
            return 0;
        }
        n_physics::GET_ROPE_VERTEX_COUNT(api, self.0)
    }

    /// `PHYSICS::_IS_ROPE_BROKEN`.
    pub fn is_broken(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_physics::_IS_ROPE_BROKEN(api, self.0)
    }

    /// Current rope length in metres.
    ///
    /// The RDR2 native set has no documented rope-length getter.
    /// `PHYSICS::_0x3D69537039F8D824` is the candidate used (commented out in
    /// ScriptHookRDR2DotNet's `Rope.Length` getter); this wrapper passes the
    /// handle and reinterprets the low 32 bits of the `u64` result as `f32`.
    /// Treat the value as unverified until confirmed in-game.
    pub fn length(self, api: &mut dyn GameApi) -> f32 {
        if self.0 == 0 {
            return 0.0;
        }

        let raw = n_physics::_0x3D69537039F8D824(api, self.0 as u64);
        f32::from_bits(raw as u32)
    }

    /// Force the rope to a length (`PHYSICS::ROPE_FORCE_LENGTH`). Non-finite
    /// and negative lengths are ignored.
    pub fn set_length(self, api: &mut dyn GameApi, length: f32) {
        if self.0 != 0 && length.is_finite() && length >= 0.0 {
            n_physics::ROPE_FORCE_LENGTH(api, self.0, length);
        }
    }

    /// Activate physics for the rope (`PHYSICS::ACTIVATE_PHYSICS`).
    pub fn activate_physics(self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            n_physics::ACTIVATE_PHYSICS(api, self.0);
        }
    }

    /// World position of one rope vertex (`PHYSICS::GET_ROPE_VERTEX_COORD`).
    pub fn vertex_coord(self, api: &mut dyn GameApi, vertex: i32) -> Vec3 {
        if self.0 == 0 || vertex < 0 {
            return Vec3::default();
        }
        n_physics::GET_ROPE_VERTEX_COORD(api, self.0, vertex)
    }

    /// `PHYSICS::_IS_ROPE_ATTACHED_TO_ENTITY`.
    pub fn is_attached_to(self, api: &mut dyn GameApi, entity: Entity) -> bool {
        self.0 != 0 && n_physics::_IS_ROPE_ATTACHED_TO_ENTITY(api, self.0, entity.raw())
    }

    /// Detach the rope from an entity (`PHYSICS::DETACH_ROPE_FROM_ENTITY`).
    pub fn detach(self, api: &mut dyn GameApi, entity: Entity) {
        if self.0 != 0 {
            n_physics::DETACH_ROPE_FROM_ENTITY(api, self.0, entity.raw());
        }
    }

    /// Release the rope back to the game (`PHYSICS::_RELEASE_ROPE`) and
    /// invalidate this handle.
    ///
    /// The native does not take a handle pointer, so whether the rope itself
    /// is destroyed is game-version dependent. Re-wrap the old raw handle with
    /// [`Rope::from_raw`] if it is still valid.
    pub fn release(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            n_physics::_RELEASE_ROPE(api, self.0);
            self.0 = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn zero_handle_is_rejected() {
        assert!(Rope::from_raw(0).is_none());
        assert!(!Rope::from_raw_unchecked(0).exists(&mut TestGame::new()));
    }

    #[test]
    fn release_clears_the_handle() {
        let mut game = TestGame::new();
        let mut rope = Rope::from_raw(1).unwrap();

        rope.release(&mut game);

        assert_eq!(rope.raw(), 0);
        assert!(!rope.exists(&mut game));
    }

    #[test]
    fn invalid_vertex_and_length_are_ignored() {
        let mut game = TestGame::new();
        let rope = Rope::from_raw(1).unwrap();

        assert_eq!(rope.vertex_coord(&mut game, -1), Vec3::default());
        rope.set_length(&mut game, f32::NAN);
        rope.set_length(&mut game, -1.0);

        assert!(game.calls.is_empty());
    }
}

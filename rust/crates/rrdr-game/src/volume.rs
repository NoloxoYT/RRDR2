//! Volume (trigger/shape) wrapper.
//!
//! Volumes live in their own pool and are not entities. They are used for
//! scripted triggers, audio, and gameplay regions.

use crate::Vec3Ops;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::volume as n_volume;

/// A volume handle.
///
/// Non-owning: the game may delete the volume and recycle the handle. Use
/// [`Volume::exists`] before relying on a stored handle.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Volume(i32);

impl Volume {
    /// Wrap a raw volume handle. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Wrap a raw volume handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(raw)
    }

    /// Wrap a raw handle after checking `DOES_VOLUME_EXIST`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        let volume = Self::from_raw(raw)?;
        volume.exists(api).then_some(volume)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// `VOLUME::DOES_VOLUME_EXIST`.
    pub fn exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_volume::DOES_VOLUME_EXIST(api, self.0)
    }

    /// World position (`VOLUME::GET_VOLUME_COORDS`).
    pub fn position(self, api: &mut dyn GameApi) -> Vec3 {
        if self.0 == 0 {
            return Vec3::default();
        }
        n_volume::GET_VOLUME_COORDS(api, self.0)
    }

    /// Set the world position (`VOLUME::SET_VOLUME_COORDS`).
    ///
    /// The native returns a success flag; it is ignored, matching the C#
    /// property semantics. Non-finite positions are rejected.
    pub fn set_position(self, api: &mut dyn GameApi, position: Vec3) {
        if self.0 != 0 && position.is_finite() {
            n_volume::SET_VOLUME_COORDS(api, self.0, position.x, position.y, position.z);
        }
    }

    /// Rotation in degrees (`VOLUME::GET_VOLUME_ROTATION`).
    pub fn rotation(self, api: &mut dyn GameApi) -> Vec3 {
        if self.0 == 0 {
            return Vec3::default();
        }
        n_volume::GET_VOLUME_ROTATION(api, self.0)
    }

    /// Set the rotation in degrees (`VOLUME::SET_VOLUME_ROTATION`).
    ///
    /// Non-finite rotations are rejected.
    pub fn set_rotation(self, api: &mut dyn GameApi, rotation: Vec3) {
        if self.0 != 0 && rotation.is_finite() {
            n_volume::SET_VOLUME_ROTATION(api, self.0, rotation.x, rotation.y, rotation.z);
        }
    }

    /// Scale (`VOLUME::GET_VOLUME_SCALE`).
    pub fn scale(self, api: &mut dyn GameApi) -> Vec3 {
        if self.0 == 0 {
            return Vec3::default();
        }
        n_volume::GET_VOLUME_SCALE(api, self.0)
    }

    /// Set the scale (`VOLUME::SET_VOLUME_SCALE`). Non-finite values are
    /// rejected.
    pub fn set_scale(self, api: &mut dyn GameApi, scale: Vec3) {
        if self.0 != 0 && scale.is_finite() {
            n_volume::SET_VOLUME_SCALE(api, self.0, scale.x, scale.y, scale.z);
        }
    }

    /// `VOLUME::_IS_AGGREGATE_VOLUME`.
    pub fn is_aggregate(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_volume::_IS_AGGREGATE_VOLUME(api, self.0)
    }

    /// Relationship group hash (`VOLUME::_GET_VOLUME_RELATIONSHIP`).
    pub fn relationship_group(self, api: &mut dyn GameApi) -> u32 {
        if self.0 == 0 {
            return 0;
        }
        n_volume::_GET_VOLUME_RELATIONSHIP(api, self.0)
    }

    /// Set the relationship group hash (`VOLUME::_SET_VOLUME_RELATIONSHIP`).
    pub fn set_relationship_group(self, api: &mut dyn GameApi, relationship_group: u32) {
        if self.0 != 0 {
            n_volume::_SET_VOLUME_RELATIONSHIP(api, self.0, relationship_group);
        }
    }

    /// Delete the volume and invalidate this handle (`VOLUME::DELETE_VOLUME`).
    pub fn delete(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            n_volume::DELETE_VOLUME(api, self.0);
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
        assert!(Volume::from_raw(0).is_none());
        assert!(!Volume::from_raw_unchecked(0).exists(&mut TestGame::new()));
    }

    #[test]
    fn delete_clears_the_handle() {
        let mut game = TestGame::new();
        let mut volume = Volume::from_raw(1).unwrap();

        volume.delete(&mut game);

        assert_eq!(volume.raw(), 0);
        assert!(!volume.exists(&mut game));
    }

    #[test]
    fn non_finite_spatial_values_are_ignored() {
        let mut game = TestGame::new();
        let volume = Volume::from_raw(1).unwrap();

        volume.set_position(&mut game, Vec3::new(0.0, f32::NAN, 0.0));
        volume.set_rotation(&mut game, Vec3::new(0.0, 0.0, f32::INFINITY));
        volume.set_scale(&mut game, Vec3::new(f32::NEG_INFINITY, 0.0, 0.0));

        assert!(game.calls.is_empty());
    }
}

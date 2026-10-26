//! Pickup wrapper.

use crate::prop::Prop;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::object as n_object;

/// A world pickup handle.
///
/// Pickups are not entities; the visual object they spawn is exposed through
/// [`Pickup::object`]. Handles are non-owning and may be recycled after
/// collection or deletion.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pickup(i32);

impl Pickup {
    /// Wrap a raw pickup handle. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Wrap a raw pickup handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(raw)
    }

    /// Wrap a raw handle after checking `DOES_PICKUP_EXIST`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        let pickup = Self::from_raw(raw)?;
        pickup.exists(api).then_some(pickup)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// `OBJECT::DOES_PICKUP_EXIST`.
    pub fn exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_object::DOES_PICKUP_EXIST(api, self.0)
    }

    /// World position (`OBJECT::GET_PICKUP_COORDS`).
    pub fn position(self, api: &mut dyn GameApi) -> Vec3 {
        if self.0 == 0 {
            return Vec3::default();
        }
        n_object::GET_PICKUP_COORDS(api, self.0)
    }

    /// The prop spawned for this pickup (`OBJECT::GET_PICKUP_OBJECT`), if it
    /// currently exists.
    pub fn object(self, api: &mut dyn GameApi) -> Option<Prop> {
        if self.0 == 0 {
            return None;
        }
        Prop::from_raw(n_object::GET_PICKUP_OBJECT(api, self.0))
    }

    /// `OBJECT::HAS_PICKUP_BEEN_COLLECTED`.
    pub fn is_collected(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_object::HAS_PICKUP_BEEN_COLLECTED(api, self.0)
    }

    /// `OBJECT::DOES_PICKUP_OBJECT_EXIST` (the spawned prop, not the pickup
    /// itself).
    pub fn object_exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_object::DOES_PICKUP_OBJECT_EXIST(api, self.0)
    }

    /// Remove the pickup and invalidate this handle (`OBJECT::REMOVE_PICKUP`).
    pub fn delete(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            n_object::REMOVE_PICKUP(api, self.0);
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
        assert!(Pickup::from_raw(0).is_none());
        assert!(!Pickup::from_raw_unchecked(0).exists(&mut TestGame::new()));
    }

    #[test]
    fn delete_clears_the_handle() {
        let mut game = TestGame::new();
        let mut pickup = Pickup::from_raw(1).unwrap();

        pickup.delete(&mut game);

        assert_eq!(pickup.raw(), 0);
        assert!(!pickup.exists(&mut game));
        assert!(pickup.object(&mut game).is_none());
    }

    #[test]
    fn missing_object_returns_none() {
        let mut game = TestGame::new();
        let pickup = Pickup::from_raw(1).unwrap();

        assert!(pickup.object(&mut game).is_none());
        assert!(!pickup.object_exists(&mut game));
        assert!(!pickup.is_collected(&mut game));
    }
}

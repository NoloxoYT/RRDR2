//! Blip wrapper.

use crate::entity::Entity;
use rrdr_bridge::GameApi;
use rrdr_natives::map as n_map;

/// A map blip. Blips live in a separate pool from entities.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Blip(i32);

impl Blip {
    /// Wrap a raw blip handle. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Wrap a raw blip handle after checking `DOES_BLIP_EXIST`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        let blip = Self::from_raw(raw)?;
        blip.exists(api).then_some(blip)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// `DOES_BLIP_EXIST`.
    pub fn exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_map::DOES_BLIP_EXIST(api, self.0)
    }

    /// Remove the blip and invalidate the handle.
    pub fn remove(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            let mut raw = self.0;
            n_map::REMOVE_BLIP(api, &mut raw);
            self.0 = 0;
        }
    }

    /// `SET_BLIP_SPRITE`.
    pub fn set_sprite(self, api: &mut dyn GameApi, sprite: u32) {
        n_map::SET_BLIP_SPRITE(api, self.0, sprite, true);
    }

    /// `SET_BLIP_SCALE`.
    pub fn set_scale(self, api: &mut dyn GameApi, scale: f32) {
        if scale.is_finite() {
            n_map::SET_BLIP_SCALE(api, self.0, scale.max(0.0));
        }
    }
}

/// Add a blip for an entity.
pub fn add_for_entity(api: &mut dyn GameApi, sprite: u32, entity: Entity) -> Option<Blip> {
    Blip::from_raw(n_map::BLIP_ADD_FOR_ENTITY(api, sprite, entity.raw()))
}

/// Add a blip at a world position.
pub fn add_for_coords(api: &mut dyn GameApi, sprite: u32, position: crate::Vec3) -> Option<Blip> {
    if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
        return None;
    }

    Blip::from_raw(n_map::BLIP_ADD_FOR_COORDS(
        api, sprite, position.x, position.y, position.z,
    ))
}

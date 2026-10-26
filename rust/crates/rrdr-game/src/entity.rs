//! Type-erased entity handle and entity kind dispatch.

use crate::error::{Error, Result};
use crate::ped::Ped;
use crate::prop::Prop;
use crate::traits::{HasEntity, HasHandle};
use crate::vehicle::Vehicle;
use rrdr_bridge::GameApi;
use rrdr_natives::entity as n_entity;

/// Value returned by `ENTITY::GET_ENTITY_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityKind {
    /// No entity / invalid type.
    None,
    /// `Ped`.
    Ped,
    /// `Vehicle`.
    Vehicle,
    /// `Object` (ScriptHook calls it `Prop`): props, doors, pickups.
    Prop,
    /// A type added by a future game version.
    Other(i32),
}

impl EntityKind {
    /// Map the raw value returned by `GET_ENTITY_TYPE`.
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::Ped,
            2 => Self::Vehicle,
            3 => Self::Prop,
            other => Self::Other(other),
        }
    }

    /// Raw value understood by the game.
    pub const fn raw(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Ped => 1,
            Self::Vehicle => 2,
            Self::Prop => 3,
            Self::Other(other) => other,
        }
    }
}

/// Type-erased entity handle.
///
/// This is a non-owning weak reference: the game can delete the entity and
/// recycle the handle. Use [`Entity::exists`] or [`Entity::try_from_raw`] when
/// freshness matters.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity(i32);

impl Entity {
    /// The invalid handle (`0`).
    pub const INVALID: Self = Self(0);

    /// Wrap a raw handle without validating it. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Wrap a raw handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(raw)
    }

    /// Wrap a raw handle after checking `DOES_ENTITY_EXIST`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        let entity = Self::from_raw(raw)?;
        entity.exists(api).then_some(entity)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// `DOES_ENTITY_EXIST`.
    pub fn exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_entity::DOES_ENTITY_EXIST(api, self.0)
    }

    /// `GET_ENTITY_TYPE`.
    pub fn kind(self, api: &mut dyn GameApi) -> EntityKind {
        if self.0 == 0 {
            return EntityKind::None;
        }
        EntityKind::from_raw(n_entity::GET_ENTITY_TYPE(api, self.0))
    }

    /// Downcast to a ped, checking the entity type.
    pub fn as_ped(self, api: &mut dyn GameApi) -> Option<Ped> {
        (self.kind(api) == EntityKind::Ped).then_some(Ped::from_raw_unchecked(self.0))
    }

    /// Downcast to a vehicle, checking the entity type.
    pub fn as_vehicle(self, api: &mut dyn GameApi) -> Option<Vehicle> {
        (self.kind(api) == EntityKind::Vehicle).then_some(Vehicle::from_raw_unchecked(self.0))
    }

    /// Downcast to a prop/object, checking the entity type.
    pub fn as_prop(self, api: &mut dyn GameApi) -> Option<Prop> {
        (self.kind(api) == EntityKind::Prop).then_some(Prop::from_raw_unchecked(self.0))
    }

    /// Resolve the concrete entity type.
    pub fn resolve(self, api: &mut dyn GameApi) -> Option<EntityRef> {
        match self.kind(api) {
            EntityKind::Ped => Some(EntityRef::Ped(Ped::from_raw_unchecked(self.0))),
            EntityKind::Vehicle => Some(EntityRef::Vehicle(Vehicle::from_raw_unchecked(self.0))),
            EntityKind::Prop => Some(EntityRef::Prop(Prop::from_raw_unchecked(self.0))),
            _ => None,
        }
    }

    /// Require this entity to be a ped.
    pub fn require_ped(self, api: &mut dyn GameApi) -> Result<Ped> {
        match self.kind(api) {
            EntityKind::Ped => Ok(Ped::from_raw_unchecked(self.0)),
            actual => Err(Error::WrongEntityType {
                expected: EntityKind::Ped,
                actual,
                handle: self.0,
            }),
        }
    }

    /// Require this entity to be a vehicle.
    pub fn require_vehicle(self, api: &mut dyn GameApi) -> Result<Vehicle> {
        match self.kind(api) {
            EntityKind::Vehicle => Ok(Vehicle::from_raw_unchecked(self.0)),
            actual => Err(Error::WrongEntityType {
                expected: EntityKind::Vehicle,
                actual,
                handle: self.0,
            }),
        }
    }

    /// Require this entity to be a prop.
    pub fn require_prop(self, api: &mut dyn GameApi) -> Result<Prop> {
        match self.kind(api) {
            EntityKind::Prop => Ok(Prop::from_raw_unchecked(self.0)),
            actual => Err(Error::WrongEntityType {
                expected: EntityKind::Prop,
                actual,
                handle: self.0,
            }),
        }
    }

    /// `GET_ENTITY_MAX_HEALTH` for non-ped entities.
    pub fn max_health(self, api: &mut dyn GameApi) -> i32 {
        n_entity::GET_ENTITY_MAX_HEALTH(api, self.0, false)
    }

    /// `SET_ENTITY_MAX_HEALTH` for non-ped entities.
    pub fn set_max_health(self, api: &mut dyn GameApi, value: i32) {
        if value > 0 {
            n_entity::SET_ENTITY_MAX_HEALTH(api, self.0, value);
        }
    }

    /// `IS_ENTITY_A_PED`.
    pub fn is_ped(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_A_PED(api, self.0)
    }

    /// `IS_ENTITY_A_VEHICLE`.
    pub fn is_vehicle(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_A_VEHICLE(api, self.0)
    }

    /// `IS_ENTITY_AN_OBJECT`.
    pub fn is_object(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_AN_OBJECT(api, self.0)
    }

    /// `IS_ENTITY_ON_FIRE`.
    pub fn is_on_fire(self, api: &mut dyn GameApi) -> bool {
        rrdr_natives::fire::IS_ENTITY_ON_FIRE(api, self.0)
    }

    /// `IS_ENTITY_A_MISSION_ENTITY`.
    pub fn is_mission_entity(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_A_MISSION_ENTITY(api, self.0)
    }

    /// Mark this entity as a mission entity so it survives script cleanup.
    pub fn mark_as_mission_entity(self, api: &mut dyn GameApi) {
        n_entity::SET_ENTITY_AS_MISSION_ENTITY(api, self.0, true, true);
    }

    /// Release the entity back to the game.
    pub fn release(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            let mut raw = self.0;
            n_entity::SET_ENTITY_AS_NO_LONGER_NEEDED(api, &mut raw);
            self.0 = 0;
        }
    }

    /// Delete the entity.
    pub fn delete(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            self.mark_as_mission_entity(api);
            let mut raw = self.0;
            n_entity::DELETE_ENTITY(api, &mut raw);
            self.0 = 0;
        }
    }

    /// `SET_ENTITY_INVINCIBLE`. There is no getter in the native API.
    pub fn set_invincible(self, api: &mut dyn GameApi, invincible: bool) {
        n_entity::SET_ENTITY_INVINCIBLE(api, self.0, invincible);
    }

    /// `_GET_ENTITY_CAN_BE_DAMAGED`.
    pub fn can_be_damaged(self, api: &mut dyn GameApi) -> bool {
        n_entity::_GET_ENTITY_CAN_BE_DAMAGED(api, self.0)
    }

    /// `SET_ENTITY_CAN_BE_DAMAGED`.
    pub fn set_can_be_damaged(self, api: &mut dyn GameApi, can_be_damaged: bool) {
        n_entity::SET_ENTITY_CAN_BE_DAMAGED(api, self.0, can_be_damaged);
    }

    /// Raw proof bitmask.
    pub fn proof_bits(self, api: &mut dyn GameApi) -> i32 {
        n_entity::_GET_ENTITY_PROOFS(api, self.0)
    }

    /// Set the raw proof bitmask.
    pub fn set_proof_bits(self, api: &mut dyn GameApi, bits: i32) {
        n_entity::SET_ENTITY_PROOFS(api, self.0, bits, false);
    }

    /// Clear all proofs.
    pub fn clear_proofs(self, api: &mut dyn GameApi) {
        self.set_proof_bits(api, 0);
    }

    /// Check one proof flag.
    pub fn is_proof(self, api: &mut dyn GameApi, proof: Proof) -> bool {
        (self.proof_bits(api) & (1 << proof.bit())) != 0
    }

    /// Enable or disable one proof flag.
    pub fn set_proof(self, api: &mut dyn GameApi, proof: Proof, enabled: bool) {
        let mut bits = self.proof_bits(api);
        if enabled {
            bits |= 1 << proof.bit();
        } else {
            bits &= !(1 << proof.bit());
        }
        self.set_proof_bits(api, bits);
    }

    /// `IS_ENTITY_VISIBLE`.
    pub fn is_visible(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_VISIBLE(api, self.0)
    }

    /// `SET_ENTITY_VISIBLE`.
    pub fn set_visible(self, api: &mut dyn GameApi, visible: bool) {
        n_entity::SET_ENTITY_VISIBLE(api, self.0, visible);
    }

    /// `IS_ENTITY_ON_SCREEN`.
    pub fn is_on_screen(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_ON_SCREEN(api, self.0)
    }

    /// `IS_ENTITY_IN_AIR`.
    pub fn is_in_air(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_IN_AIR(api, self.0, 0)
    }

    /// `IS_ENTITY_IN_WATER`.
    pub fn is_in_water(self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_IN_WATER(api, self.0)
    }

    /// `GET_ENTITY_HEIGHT_ABOVE_GROUND`.
    pub fn height_above_ground(self, api: &mut dyn GameApi) -> f32 {
        n_entity::GET_ENTITY_HEIGHT_ABOVE_GROUND(api, self.0)
    }
}

impl HasHandle for Entity {
    fn handle(&self) -> i32 {
        self.0
    }
}

impl HasEntity for Entity {
    fn entity(&self) -> Entity {
        *self
    }
}

/// Concrete entity wrapper after type dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityRef {
    /// Ped.
    Ped(Ped),
    /// Vehicle.
    Vehicle(Vehicle),
    /// Prop/object.
    Prop(Prop),
}

impl EntityRef {
    /// The underlying type-erased entity.
    pub const fn entity(self) -> Entity {
        match self {
            Self::Ped(ped) => ped.entity(),
            Self::Vehicle(vehicle) => vehicle.entity(),
            Self::Prop(prop) => prop.entity(),
        }
    }
}

impl HasHandle for EntityRef {
    fn handle(&self) -> i32 {
        EntityRef::entity(*self).raw()
    }
}

impl HasEntity for EntityRef {
    fn entity(&self) -> Entity {
        EntityRef::entity(*self)
    }
}

/// Entity proof flags, matching the bit order used by ScriptHookRDR2DotNet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Proof {
    /// Bullets cannot damage the entity.
    Bullet,
    /// Fire cannot damage the entity.
    Flame,
    /// Explosions cannot damage the entity.
    Explosion,
    /// Collisions cannot damage the entity.
    Collision,
    /// Melee cannot damage the entity.
    Melee,
    /// Steam cannot damage the entity.
    Steam,
    /// Smoke cannot damage the entity.
    Smoke,
    /// Headshots cannot damage the entity.
    Headshot,
    /// Projectiles cannot damage the entity.
    Projectile,
}

impl Proof {
    /// Bit index used by `SET_ENTITY_PROOFS`.
    pub const fn bit(self) -> u8 {
        match self {
            Self::Bullet => 0,
            Self::Flame => 1,
            Self::Explosion => 2,
            Self::Collision => 3,
            Self::Melee => 4,
            Self::Steam => 5,
            Self::Smoke => 6,
            Self::Headshot => 7,
            Self::Projectile => 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;
    use crate::traits::{Damageable, Spatial};
    use rrdr_bridge::Vec3;

    #[test]
    fn zero_handle_is_rejected() {
        assert!(Entity::from_raw(0).is_none());
        assert!(!Entity::from_raw_unchecked(0).exists(&mut TestGame::new()));
    }

    #[test]
    fn entity_kind_maps_and_dispatches() {
        let mut game = TestGame::new();
        game.kind = 2;

        let entity = Entity::from_raw(5).expect("non-zero handle");
        assert_eq!(entity.kind(&mut game), EntityKind::Vehicle);
        assert!(entity.is_vehicle(&mut game));
        assert!(entity.as_vehicle(&mut game).is_some());
        assert!(entity.as_ped(&mut game).is_none());

        match entity.require_ped(&mut game) {
            Err(Error::WrongEntityType {
                expected,
                actual,
                handle,
            }) => {
                assert_eq!(expected, EntityKind::Ped);
                assert_eq!(actual, EntityKind::Vehicle);
                assert_eq!(handle, 5);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn health_and_position_roundtrip() {
        let mut game = TestGame::new();
        let entity = Entity::from_raw(1).unwrap();

        assert_eq!(entity.health(&mut game), 100);
        entity.set_health(&mut game, 40);
        assert_eq!(entity.health(&mut game), 40);

        let position = Vec3::new(-1.0, 2.5, 300.0);
        entity.set_position(&mut game, position);
        assert_eq!(entity.position(&mut game), position);
    }

    #[test]
    fn non_finite_position_is_ignored() {
        let mut game = TestGame::new();
        let entity = Entity::from_raw(1).unwrap();
        let original = entity.position(&mut game);

        entity.set_position(&mut game, Vec3::new(f32::NAN, 0.0, 0.0));
        assert_eq!(entity.position(&mut game), original);
    }

    #[test]
    fn delete_clears_the_handle() {
        let mut game = TestGame::new();
        let mut entity = Entity::from_raw(1).unwrap();

        entity.delete(&mut game);

        assert_eq!(entity.raw(), 0);
        assert!(game.deleted);
        assert!(game.mission);
    }

    #[test]
    fn proofs_can_be_toggled() {
        let mut game = TestGame::new();
        let entity = Entity::from_raw(1).unwrap();

        assert!(!entity.is_proof(&mut game, Proof::Bullet));
        entity.set_proof(&mut game, Proof::Bullet, true);
        assert!(entity.is_proof(&mut game, Proof::Bullet));
        assert_eq!(entity.proof_bits(&mut game), 1);

        entity.clear_proofs(&mut game);
        assert!(!entity.is_proof(&mut game, Proof::Bullet));
    }
}

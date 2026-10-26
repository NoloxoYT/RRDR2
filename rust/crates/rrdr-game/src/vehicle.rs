//! Vehicle wrapper.

use crate::entity::Entity;
use crate::ped::Ped;
use crate::traits::{Damageable, HasEntity, HasHandle};
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::vehicle as n_vehicle;

/// A vehicle.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Vehicle(Entity);

impl Vehicle {
    /// Wrap a raw handle without validating it. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Entity::from_raw(raw).map(Self)
    }

    /// Wrap a raw handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(Entity::from_raw_unchecked(raw))
    }

    /// Wrap a raw handle after checking existence and entity type.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        Entity::try_from_raw(api, raw)?.as_vehicle(api)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0.raw()
    }

    /// Type-erased entity handle.
    pub const fn entity(self) -> Entity {
        self.0
    }

    /// Vehicle body health (`GET_VEHICLE_BODY_HEALTH`), in the game's float range.
    pub fn body_health(self, api: &mut dyn GameApi) -> f32 {
        n_vehicle::GET_VEHICLE_BODY_HEALTH(api, self.raw())
    }

    /// Set vehicle body health.
    pub fn set_body_health(self, api: &mut dyn GameApi, health: f32) {
        if health.is_finite() {
            n_vehicle::SET_VEHICLE_BODY_HEALTH(api, self.raw(), health.max(0.0));
        }
    }

    /// Entity health is below entity max health.
    pub fn is_damaged(self, api: &mut dyn GameApi) -> bool {
        Damageable::health(&self, api) < self.0.max_health(api)
    }

    /// `IS_VEHICLE_DRIVEABLE`.
    pub fn is_driveable(self, api: &mut dyn GameApi) -> bool {
        n_vehicle::IS_VEHICLE_DRIVEABLE(api, self.raw(), false, false)
    }

    /// Apply localized vehicle damage.
    pub fn apply_damage(self, api: &mut dyn GameApi, point: Vec3, damage: f32, radius: f32) {
        if point.x.is_finite()
            && point.y.is_finite()
            && point.z.is_finite()
            && damage.is_finite()
            && radius.is_finite()
        {
            n_vehicle::SET_VEHICLE_DAMAGE(
                api,
                self.raw(),
                point.x,
                point.y,
                point.z,
                damage,
                radius,
                true,
            );
        }
    }

    /// Driver, if any.
    pub fn driver(self, api: &mut dyn GameApi) -> Option<Ped> {
        Ped::from_raw(n_vehicle::GET_DRIVER_OF_VEHICLE(api, self.raw()))
    }

    /// Ped in a specific seat, if any.
    pub fn get_ped_in_seat(self, api: &mut dyn GameApi, seat: i32) -> Option<Ped> {
        Ped::from_raw(n_vehicle::GET_PED_IN_VEHICLE_SEAT(api, self.raw(), seat))
    }

    /// Repair the vehicle.
    pub fn repair(self, api: &mut dyn GameApi) {
        n_vehicle::SET_VEHICLE_FIXED(api, self.raw());
    }

    /// Explode the vehicle.
    pub fn explode(self, api: &mut dyn GameApi) {
        n_vehicle::EXPLODE_VEHICLE(api, self.raw(), true, false, 0, 0);
    }

    /// `IS_VEHICLE_WRECKED`.
    pub fn is_wrecked(self, api: &mut dyn GameApi) -> bool {
        n_vehicle::IS_VEHICLE_WRECKED(api, self.raw())
    }
}

impl HasHandle for Vehicle {
    fn handle(&self) -> i32 {
        self.raw()
    }
}

impl HasEntity for Vehicle {
    fn entity(&self) -> Entity {
        self.0
    }
}

impl From<Vehicle> for Entity {
    fn from(vehicle: Vehicle) -> Self {
        vehicle.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;
    use crate::traits::Damageable;

    #[test]
    fn body_health_roundtrip() {
        let mut game = TestGame::new();
        game.vehicle_body_health = 750.0;
        let vehicle = Vehicle::from_raw(2).unwrap();

        assert_eq!(vehicle.body_health(&mut game), 750.0);
        vehicle.set_body_health(&mut game, 500.0);
        assert_eq!(vehicle.body_health(&mut game), 500.0);
    }

    #[test]
    fn damage_and_occupants() {
        let mut game = TestGame::new();
        game.driver = 10;
        game.seat_ped = 11;
        let vehicle = Vehicle::from_raw(2).unwrap();

        vehicle.apply_damage(&mut game, Vec3::new(1.0, 2.0, 3.0), 25.0, 0.5);
        assert_eq!(
            game.vehicle_damage,
            Some((Vec3::new(1.0, 2.0, 3.0), 25.0, 0.5))
        );
        assert_eq!(vehicle.driver(&mut game), Ped::from_raw(10));
        assert_eq!(vehicle.get_ped_in_seat(&mut game, 1), Ped::from_raw(11));
    }

    #[test]
    fn damage_state_and_actions() {
        let mut game = TestGame::new();
        let vehicle = Vehicle::from_raw(2).unwrap();

        assert!(!vehicle.is_damaged(&mut game));
        vehicle.set_health(&mut game, 50);
        assert!(vehicle.is_damaged(&mut game));

        vehicle.repair(&mut game);
        vehicle.explode(&mut game);
        assert!(game.repaired);
        assert!(game.exploded);
    }
}

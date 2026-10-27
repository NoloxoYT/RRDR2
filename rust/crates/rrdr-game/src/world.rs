//! World queries and effects.

use crate::entity::Entity;
use crate::ped::Ped;
use crate::player::Player;
use crate::pool::{self, PoolOptions};
use crate::prop::Prop;
use crate::traits::{Damageable, Spatial};
use crate::vehicle::Vehicle;
use crate::Vec3Ops;
use rrdr_bridge::{GameApi, PoolKind, Vec3};
use rrdr_natives::{fire as n_fire, misc as n_misc};

/// The local player.
pub fn local_player(api: &mut dyn GameApi) -> Option<Player> {
    Player::local(api)
}

/// Checked entity lookup.
pub fn entity(api: &mut dyn GameApi, raw: i32) -> Option<Entity> {
    Entity::try_from_raw(api, raw)
}

/// Checked ped lookup.
pub fn ped(api: &mut dyn GameApi, raw: i32) -> Option<Ped> {
    Ped::try_from_raw(api, raw)
}

/// Checked vehicle lookup.
pub fn vehicle(api: &mut dyn GameApi, raw: i32) -> Option<Vehicle> {
    Vehicle::try_from_raw(api, raw)
}

/// Peds near a source ped.
///
/// Deduplication uses a `HashSet` over raw handles: a linear `contains` per
/// candidate was O(n²) on the 256..1024 entries a pool can return.
pub fn nearby_peds(api: &mut dyn GameApi, source: Ped, options: PoolOptions) -> Vec<Ped> {
    let mut peds = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw in pool::nearby_ped_handles(api, source.raw(), options.capacity) {
        if raw == 0 || raw == source.raw() || !seen.insert(raw) {
            continue;
        }

        let Some(ped) = Ped::try_from_raw(api, raw) else {
            continue;
        };

        if !options.include_dead && ped.is_dead(api) {
            continue;
        }

        peds.push(ped);
    }

    peds
}

/// Peds near the local player.
pub fn nearby_peds_from_player(api: &mut dyn GameApi, options: PoolOptions) -> Vec<Ped> {
    let Some(player) = Player::local(api) else {
        return Vec::new();
    };
    let Some(ped) = player.ped(api) else {
        return Vec::new();
    };

    nearby_peds(api, ped, options)
}

/// Vehicles near a source ped. Deduplicated with a `HashSet` (see
/// [`nearby_peds`]).
pub fn nearby_vehicles(api: &mut dyn GameApi, source: Ped, options: PoolOptions) -> Vec<Vehicle> {
    let mut vehicles = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw in pool::nearby_vehicle_handles(api, source.raw(), options.capacity) {
        if raw == 0 || !seen.insert(raw) {
            continue;
        }

        let Some(vehicle) = Vehicle::try_from_raw(api, raw) else {
            continue;
        };

        if !options.include_dead && vehicle.is_dead(api) {
            continue;
        }

        vehicles.push(vehicle);
    }

    vehicles
}

/// Global ped pool.
///
/// Returns `None` when the host does not expose a pool provider (ABI v2 or the
/// offline mock). Requires the managed host's `worldGetAllPeds` callback.
///
/// The per-handle validation costs 2-4 marshaled native calls each; do not
/// call this every tick for a large capacity.
pub fn all_peds(api: &mut dyn GameApi, options: PoolOptions) -> Option<Vec<Ped>> {
    let handles = api.pool_handles(PoolKind::Ped, options.capacity)?;
    let mut peds = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw in handles {
        if raw == 0 || !seen.insert(raw) {
            continue;
        }

        let Some(ped) = Ped::try_from_raw(api, raw) else {
            continue;
        };

        if !options.include_dead && ped.is_dead(api) {
            continue;
        }

        peds.push(ped);
    }

    Some(peds)
}

/// Global vehicle pool. Deduplicated with a `HashSet` (see [`all_peds`]).
pub fn all_vehicles(api: &mut dyn GameApi, options: PoolOptions) -> Option<Vec<Vehicle>> {
    let handles = api.pool_handles(PoolKind::Vehicle, options.capacity)?;
    let mut vehicles = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw in handles {
        if raw == 0 || !seen.insert(raw) {
            continue;
        }

        let Some(vehicle) = Vehicle::try_from_raw(api, raw) else {
            continue;
        };

        if !options.include_dead && vehicle.is_dead(api) {
            continue;
        }

        vehicles.push(vehicle);
    }

    Some(vehicles)
}

/// Global object/prop pool. Deduplicated with a `HashSet` (see [`all_peds`]).
pub fn all_objects(api: &mut dyn GameApi, options: PoolOptions) -> Option<Vec<Prop>> {
    let handles = api.pool_handles(PoolKind::Object, options.capacity)?;
    let mut props = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw in handles {
        if raw == 0 || !seen.insert(raw) {
            continue;
        }

        if let Some(prop) = Prop::try_from_raw(api, raw) {
            props.push(prop);
        }
    }

    Some(props)
}

/// Closest ped to a source ped among the nearby snapshot.
pub fn nearest_ped(api: &mut dyn GameApi, source: Ped, options: PoolOptions) -> Option<Ped> {
    let origin = source.position(api);
    nearby_peds(api, source, options)
        .into_iter()
        .min_by(|left, right| {
            let left_distance = left.position(api).distance_squared(origin);
            let right_distance = right.position(api).distance_squared(origin);
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Closest vehicle to a source ped among the nearby snapshot.
pub fn nearest_vehicle(
    api: &mut dyn GameApi,
    source: Ped,
    options: PoolOptions,
) -> Option<Vehicle> {
    let origin = source.position(api);
    nearby_vehicles(api, source, options)
        .into_iter()
        .min_by(|left, right| {
            let left_distance = left.position(api).distance_squared(origin);
            let right_distance = right.position(api).distance_squared(origin);
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Ground Z at a world position.
pub fn ground_z(api: &mut dyn GameApi, position: Vec3) -> Option<f32> {
    if !position.is_finite() {
        return None;
    }

    let mut ground = 0.0f32;
    n_misc::GET_GROUND_Z_FOR_3D_COORD(api, position.x, position.y, position.z, &mut ground, false)
        .then_some(ground)
}

/// Create an explosion.
pub fn add_explosion(
    api: &mut dyn GameApi,
    position: Vec3,
    explosion_type: i32,
    damage_scale: f32,
    camera_shake: f32,
) {
    if position.is_finite() && damage_scale.is_finite() && camera_shake.is_finite() {
        n_fire::ADD_EXPLOSION(
            api,
            position.x,
            position.y,
            position.z,
            explosion_type,
            damage_scale,
            true,
            false,
            camera_shake,
        );
    }
}

/// Shoot a single bullet between two points.
pub fn shoot_bullet(
    api: &mut dyn GameApi,
    source: Vec3,
    target: Vec3,
    damage: i32,
    weapon_hash: u32,
    owner: Option<Ped>,
) {
    if !source.is_finite() || !target.is_finite() || damage <= 0 {
        return;
    }

    n_misc::SHOOT_SINGLE_BULLET_BETWEEN_COORDS(
        api,
        source.x,
        source.y,
        source.z,
        target.x,
        target.y,
        target.z,
        damage,
        true,
        weapon_hash,
        owner.map(Ped::raw).unwrap_or(0),
        true,
        false,
        -1.0,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn checked_lookup_uses_existence_and_kind() {
        let mut game = TestGame::new();
        game.kind = 1;
        game.exists = true;

        assert_eq!(
            local_player(&mut game),
            Some(Player::from_index(0).unwrap())
        );
        assert!(ped(&mut game, 1).is_some());
        assert!(entity(&mut game, 1).is_some());
        assert!(vehicle(&mut game, 1).is_none());

        game.exists = false;
        assert!(entity(&mut game, 1).is_none());
    }

    #[test]
    fn global_pools_filter_invalid_and_duplicate_handles() {
        let mut game = TestGame::new();
        game.kind = 1;
        game.pool_peds = vec![1, 1, 0];

        let peds = all_peds(&mut game, PoolOptions::default()).expect("pool provider");
        assert_eq!(peds, vec![Ped::from_raw(1).unwrap()]);

        game.kind = 2;
        game.pool_vehicles = vec![2, 0, 2];
        let vehicles = all_vehicles(&mut game, PoolOptions::default()).expect("pool provider");
        assert_eq!(vehicles, vec![Vehicle::from_raw(2).unwrap()]);

        game.kind = 3;
        game.pool_objects = vec![3, 0, 3];
        let props = all_objects(&mut game, PoolOptions::default()).expect("pool provider");
        assert_eq!(props, vec![Prop::from_raw(3).unwrap()]);
    }

    #[test]
    fn global_pools_return_none_without_provider() {
        let mut game = TestGame::new();
        game.pools_available = false;

        assert!(all_peds(&mut game, PoolOptions::default()).is_none());
        assert!(all_vehicles(&mut game, PoolOptions::default()).is_none());
        assert!(all_objects(&mut game, PoolOptions::default()).is_none());
    }
}

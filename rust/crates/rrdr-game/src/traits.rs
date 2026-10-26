//! Shared behavior for handle types.
//!
//! The traits are implemented for every type that implements [`HasEntity`], so
//! `Ped`, `Vehicle`, and `Prop` get the same spatial/damage helpers as `Entity`.

use crate::entity::Entity;
use crate::model::Model;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::entity as n_entity;

/// Anything with a raw RAGE handle.
pub trait HasHandle: Copy {
    /// Raw handle value. `0` is invalid.
    fn handle(&self) -> i32;
}

/// Anything that can be viewed as an [`Entity`].
pub trait HasEntity: HasHandle {
    /// The type-erased entity handle.
    fn entity(&self) -> Entity;
}

/// Entity existence checks.
pub trait Exists: HasEntity {
    /// `DOES_ENTITY_EXIST`. A `true` result is best-effort: the handle can be
    /// recycled by the game.
    fn exists(&self, api: &mut dyn GameApi) -> bool {
        n_entity::DOES_ENTITY_EXIST(api, self.handle())
    }
}

/// Position, rotation, heading, and velocity helpers.
pub trait Spatial: HasEntity {
    /// World position.
    fn position(&self, api: &mut dyn GameApi) -> Vec3 {
        n_entity::GET_ENTITY_COORDS(api, self.handle(), true, true)
    }

    /// Teleport the entity. Rejects non-finite positions.
    fn set_position(&self, api: &mut dyn GameApi, position: Vec3) {
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return;
        }

        n_entity::SET_ENTITY_COORDS(
            api,
            self.handle(),
            position.x,
            position.y,
            position.z,
            true,
            true,
            true,
            true,
        );
    }

    /// World rotation in degrees (rotation order 2, matching the C# API).
    fn rotation(&self, api: &mut dyn GameApi) -> Vec3 {
        n_entity::GET_ENTITY_ROTATION(api, self.handle(), 2)
    }

    /// Set world rotation in degrees (rotation order 2).
    fn set_rotation(&self, api: &mut dyn GameApi, rotation: Vec3) {
        if !rotation.x.is_finite() || !rotation.y.is_finite() || !rotation.z.is_finite() {
            return;
        }

        n_entity::SET_ENTITY_ROTATION(
            api,
            self.handle(),
            rotation.x,
            rotation.y,
            rotation.z,
            2,
            true,
        );
    }

    /// Heading in degrees.
    fn heading(&self, api: &mut dyn GameApi) -> f32 {
        n_entity::GET_ENTITY_HEADING(api, self.handle())
    }

    /// Set heading in degrees.
    fn set_heading(&self, api: &mut dyn GameApi, heading: f32) {
        if heading.is_finite() {
            n_entity::SET_ENTITY_HEADING(api, self.handle(), heading);
        }
    }

    /// Velocity vector.
    fn velocity(&self, api: &mut dyn GameApi) -> Vec3 {
        n_entity::GET_ENTITY_VELOCITY(api, self.handle(), -1)
    }

    /// Set velocity vector.
    fn set_velocity(&self, api: &mut dyn GameApi, velocity: Vec3) {
        if velocity.x.is_finite() && velocity.y.is_finite() && velocity.z.is_finite() {
            n_entity::SET_ENTITY_VELOCITY(api, self.handle(), velocity.x, velocity.y, velocity.z);
        }
    }

    /// Speed in metres per second.
    fn speed(&self, api: &mut dyn GameApi) -> f32 {
        n_entity::GET_ENTITY_SPEED(api, self.handle())
    }

    /// Unit forward vector.
    fn forward_vector(&self, api: &mut dyn GameApi) -> Vec3 {
        n_entity::GET_ENTITY_FORWARD_VECTOR(api, self.handle())
    }

    /// Distance to another entity.
    fn distance_to(&self, api: &mut dyn GameApi, other: Entity) -> f32 {
        let a = self.position(api);
        let b = other.position(api);
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Health and death helpers shared by all damageable entities.
///
/// `max_health` is deliberately not part of this trait: peds use
/// `GET_PED_MAX_HEALTH` while other entities use `GET_ENTITY_MAX_HEALTH`, and
/// the two values are not guaranteed to agree.
pub trait Damageable: HasEntity {
    /// Current health.
    fn health(&self, api: &mut dyn GameApi) -> i32 {
        n_entity::GET_ENTITY_HEALTH(api, self.handle())
    }

    /// Set health. `killer` is the entity credited with the damage, or `0`.
    fn set_health(&self, api: &mut dyn GameApi, health: i32) {
        if health >= 0 {
            n_entity::SET_ENTITY_HEALTH(api, self.handle(), health, 0);
        }
    }

    /// Set health with an explicit killer.
    fn set_health_with_killer(&self, api: &mut dyn GameApi, health: i32, killer: Entity) {
        if health >= 0 {
            n_entity::SET_ENTITY_HEALTH(api, self.handle(), health, killer.raw());
        }
    }

    /// `IS_ENTITY_DEAD`.
    fn is_dead(&self, api: &mut dyn GameApi) -> bool {
        n_entity::IS_ENTITY_DEAD(api, self.handle())
    }

    /// Set health to zero. This is the generic entity kill; peds have
    /// [`crate::ped::Ped::kill`] for the ped-specific death path.
    fn kill(&self, api: &mut dyn GameApi) {
        self.set_health(api, 0);
    }
}

/// Model helpers.
pub trait HasModel: HasEntity {
    /// The entity's model hash.
    fn model(&self, api: &mut dyn GameApi) -> Model {
        Model::from_hash(n_entity::GET_ENTITY_MODEL(api, self.handle()))
    }

    /// Compare the entity's model against a model hash.
    fn is_model(&self, api: &mut dyn GameApi, model: Model) -> bool {
        self.model(api) == model
    }
}

/// Explicit cleanup helpers.
///
/// Handles are `Copy` weak references, so these methods cannot invalidate every
/// alias. After `delete`/`release`, treat all copies of the handle as stale.
pub trait Deletable: HasEntity {
    /// Release the entity back to the game.
    fn release(&self, api: &mut dyn GameApi) {
        let mut entity = self.entity();
        Entity::release(&mut entity, api);
    }

    /// Delete the entity.
    fn delete(&self, api: &mut dyn GameApi) {
        let mut entity = self.entity();
        Entity::delete(&mut entity, api);
    }
}

impl<T: HasEntity> Exists for T {}
impl<T: HasEntity> Spatial for T {}
impl<T: HasEntity> Damageable for T {}
impl<T: HasEntity> HasModel for T {}
impl<T: HasEntity> Deletable for T {}

//! Raycast helpers.

use crate::entity::Entity;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::shapetest as n_shapetest;

/// Result of a raycast shape test.
#[derive(Clone, Copy, Debug)]
pub struct RaycastResult {
    /// Raw shape-test handle.
    pub handle: i32,
    /// Raw result code from `GET_SHAPE_TEST_RESULT` (`0` invalid, `1` pending,
    /// `2` complete).
    pub result: i32,
    /// Whether the ray hit something.
    pub did_hit: bool,
    /// Hit position.
    pub hit_position: Vec3,
    /// Hit surface normal.
    pub surface_normal: Vec3,
    /// Entity hit, if any.
    pub entity: Option<Entity>,
}

/// Cast a line-of-sight probe from `source` to `target`.
///
/// The shape test may still be pending on the first call; inspect
/// [`RaycastResult::result`] and call again next tick if needed. This mirrors
/// the single-shot behavior of the C# `RaycastResult` constructor.
pub fn raycast(api: &mut dyn GameApi, source: Vec3, target: Vec3) -> RaycastResult {
    if !source.x.is_finite()
        || !source.y.is_finite()
        || !source.z.is_finite()
        || !target.x.is_finite()
        || !target.y.is_finite()
        || !target.z.is_finite()
    {
        return RaycastResult {
            handle: 0,
            result: 0,
            did_hit: false,
            hit_position: Vec3::default(),
            surface_normal: Vec3::default(),
            entity: None,
        };
    }

    let handle = n_shapetest::START_SHAPE_TEST_LOS_PROBE(
        api, source.x, source.y, source.z, target.x, target.y, target.z, 511, 0, 0,
    );

    let mut did_hit = false;
    let mut hit_position = Vec3::default();
    let mut surface_normal = Vec3::default();
    let mut entity_hit = 0i32;
    let result = n_shapetest::GET_SHAPE_TEST_RESULT(
        api,
        handle,
        &mut did_hit,
        &mut hit_position,
        &mut surface_normal,
        &mut entity_hit,
    );

    RaycastResult {
        handle,
        result,
        did_hit,
        hit_position,
        surface_normal,
        entity: Entity::from_raw(entity_hit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn raycast_returns_shape_test_result() {
        let mut game = TestGame::new();
        let result = raycast(
            &mut game,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
        );

        assert_eq!(result.handle, 7);
        assert_eq!(result.result, 2);
        assert!(!result.did_hit);
    }

    #[test]
    fn non_finite_ray_is_rejected_without_native_call() {
        let mut game = TestGame::new();
        let result = raycast(
            &mut game,
            Vec3::new(f32::NAN, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
        );

        assert_eq!(result.result, 0);
        assert!(game.calls.is_empty());
    }
}

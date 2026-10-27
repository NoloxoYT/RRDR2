//! Raycast helpers.

use crate::entity::Entity;
use crate::scratch::{BoolOut, I32Out, Vec3Out};
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::shapetest as n_shapetest;

/// Result of a raycast shape test.
#[derive(Clone, Copy, Debug)]
pub struct RaycastResult {
    /// Raw shape-test handle.
    ///
    /// The game keeps a probe alive until its result is consumed; there is no
    /// documented `RELEASE_SHAPE_TEST` native in RDR2. When polling, save the
    /// handle and re-query with [`raycast_result`] instead of starting a new
    /// probe every tick.
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
///
/// # Do not poll with fresh probes
///
/// Every call allocates a new shape test inside the game. A mod that calls
/// this every tick accumulates probes and eventually hitches the script
/// thread. Poll with [`raycast_result`] on the saved [`RaycastResult::handle`]
/// instead.
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

    shape_test_result(api, handle)
}

/// Re-query an existing shape test by handle without starting a new probe.
///
/// Use this to poll a pending raycast: `result.result` becomes `2` when the
/// probe has completed. Returns an invalid result (`result = 0`) for a zero
/// handle.
pub fn raycast_result(api: &mut dyn GameApi, handle: i32) -> RaycastResult {
    if handle == 0 {
        return RaycastResult {
            handle: 0,
            result: 0,
            did_hit: false,
            hit_position: Vec3::default(),
            surface_normal: Vec3::default(),
            entity: None,
        };
    }

    shape_test_result(api, handle)
}

/// Shared body: read back the result of an existing probe.
///
/// The out-params go through padded scratch blocks ([`crate::scratch`]) so a
/// native that writes a 4-byte `BOOL` or a 24-byte three-slot vector cannot
/// corrupt the stack (issues §4.1).
fn shape_test_result(api: &mut dyn GameApi, handle: i32) -> RaycastResult {
    let mut did_hit = BoolOut::default();
    let mut hit_position = Vec3Out::default();
    let mut surface_normal = Vec3Out::default();
    let mut entity_hit = I32Out::default();

    let result = n_shapetest::GET_SHAPE_TEST_RESULT(
        api,
        handle,
        &mut did_hit.value,
        &mut hit_position.value,
        &mut surface_normal.value,
        &mut entity_hit.value,
    );

    RaycastResult {
        handle,
        result,
        did_hit: did_hit.value,
        hit_position: hit_position.value,
        surface_normal: surface_normal.value,
        entity: Entity::from_raw(entity_hit.value),
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

    #[test]
    fn requery_reuses_the_handle_without_a_new_probe() {
        let mut game = TestGame::new();
        let result = raycast(
            &mut game,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
        );
        let calls_after_start = game.calls.len();

        let polled = raycast_result(&mut game, result.handle);

        assert_eq!(polled.handle, result.handle);
        // Only the GET_SHAPE_TEST_RESULT call, no new START_SHAPE_TEST probe.
        assert_eq!(game.calls.len(), calls_after_start + 1);
        assert_eq!(
            game.calls[calls_after_start],
            n_shapetest::HASH_GET_SHAPE_TEST_RESULT
        );
    }
}

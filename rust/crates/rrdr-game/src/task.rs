//! Task helpers for a ped.
//!
//! `TaskInvoker` is a short-lived view over a `Ped`; it borrows the ped for the
//! duration of the call chain and never stores `GameApi`.

use crate::entity::Entity;
use crate::ped::Ped;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::task as n_task;

/// Task helpers for one ped.
#[derive(Clone, Copy, Debug)]
pub struct TaskInvoker<'a> {
    ped: &'a Ped,
}

impl<'a> TaskInvoker<'a> {
    pub(crate) const fn new(ped: &'a Ped) -> Self {
        Self { ped }
    }

    /// Combat a target ped.
    pub fn combat(&self, api: &mut dyn GameApi, target: Ped) {
        n_task::TASK_COMBAT_PED(api, self.ped.raw(), target.raw(), 0, 0);
    }

    /// Shoot at a world position for a duration in milliseconds.
    pub fn shoot_at(&self, api: &mut dyn GameApi, position: Vec3, duration_ms: i32) {
        if position.x.is_finite() && position.y.is_finite() && position.z.is_finite() {
            n_task::TASK_SHOOT_AT_COORD(
                api,
                self.ped.raw(),
                position.x,
                position.y,
                position.z,
                duration_ms,
                0,
                0,
            );
        }
    }

    /// Aim at a world position for a duration in milliseconds.
    pub fn aim_at(&self, api: &mut dyn GameApi, position: Vec3, duration_ms: i32) {
        if position.x.is_finite() && position.y.is_finite() && position.z.is_finite() {
            n_task::TASK_AIM_GUN_AT_COORD(
                api,
                self.ped.raw(),
                position.x,
                position.y,
                position.z,
                duration_ms,
                true,
                false,
            );
        }
    }

    /// Walk/run to a world position.
    pub fn go_to(&self, api: &mut dyn GameApi, position: Vec3, speed: f32) {
        if position.x.is_finite()
            && position.y.is_finite()
            && position.z.is_finite()
            && speed.is_finite()
        {
            n_task::TASK_GO_TO_COORD_ANY_MEANS(
                api,
                self.ped.raw(),
                position.x,
                position.y,
                position.z,
                speed,
                0,
                false,
                0,
                0.0,
            );
        }
    }

    /// Look at an entity for a duration in milliseconds.
    pub fn look_at(&self, api: &mut dyn GameApi, target: Entity, duration_ms: i32) {
        n_task::TASK_LOOK_AT_ENTITY(api, self.ped.raw(), target.raw(), duration_ms, 0, 0, 0);
    }

    /// Flee from a world position.
    pub fn flee(&self, api: &mut dyn GameApi, position: Vec3) {
        if position.x.is_finite() && position.y.is_finite() && position.z.is_finite() {
            n_task::TASK_FLEE_COORD(
                api,
                self.ped.raw(),
                position.x,
                position.y,
                position.z,
                0,
                0,
                0.0,
                -1,
                0,
            );
        }
    }

    /// Clear all tasks.
    pub fn clear(&self, api: &mut dyn GameApi, immediately: bool) {
        if immediately {
            n_task::CLEAR_PED_TASKS_IMMEDIATELY(api, self.ped.raw(), true, true);
        } else {
            n_task::CLEAR_PED_TASKS(api, self.ped.raw(), true, true);
        }
    }

    /// Clear secondary tasks (look-at, ambient).
    pub fn clear_secondary(&self, api: &mut dyn GameApi) {
        n_task::CLEAR_PED_SECONDARY_TASK(api, self.ped.raw());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;
    use rrdr_natives::task as n_task;

    #[test]
    fn task_calls_are_forwarded() {
        let mut game = TestGame::new();
        let ped = Ped::from_raw(1).unwrap();
        let target = Ped::from_raw(2).unwrap();
        let position = Vec3::new(1.0, 2.0, 3.0);

        ped.tasks().combat(&mut game, target);
        ped.tasks().shoot_at(&mut game, position, 500);
        ped.tasks().aim_at(&mut game, position, 500);
        ped.tasks().go_to(&mut game, position, 1.0);
        ped.tasks().look_at(&mut game, target.entity(), 500);
        ped.tasks().flee(&mut game, position);
        ped.tasks().clear(&mut game, true);
        ped.tasks().clear(&mut game, false);
        ped.tasks().clear_secondary(&mut game);

        for hash in [
            n_task::HASH_TASK_COMBAT_PED,
            n_task::HASH_TASK_SHOOT_AT_COORD,
            n_task::HASH_TASK_AIM_GUN_AT_COORD,
            n_task::HASH_TASK_GO_TO_COORD_ANY_MEANS,
            n_task::HASH_TASK_LOOK_AT_ENTITY,
            n_task::HASH_TASK_FLEE_COORD,
            n_task::HASH_CLEAR_PED_TASKS_IMMEDIATELY,
            n_task::HASH_CLEAR_PED_TASKS,
            n_task::HASH_CLEAR_PED_SECONDARY_TASK,
        ] {
            assert!(game.calls.contains(&hash), "missing task call 0x{hash:X}");
        }
    }
}

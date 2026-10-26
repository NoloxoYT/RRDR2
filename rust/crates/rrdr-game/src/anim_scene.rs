//! Animation scene wrapper.
//!
//! An animation scene is a script-controlled cutscene or synchronized
//! animation. Loading and starting are non-blocking: [`AnimScene::load`] and
//! [`AnimScene::start`] only issue the request. Poll [`AnimScene::is_loaded`],
//! [`AnimScene::is_running`], and [`AnimScene::is_finished`] from the script
//! tick instead of waiting inside the call.

use crate::entity::Entity;
use crate::Vec3Ops;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::animscene as n_animscene;

/// A handle to an animation scene.
///
/// This is a non-owning weak reference. The game can delete the scene and
/// recycle the handle, so revalidate with [`AnimScene::exists`] when identity
/// matters.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnimScene(i32);

impl AnimScene {
    /// Wrap a raw animation scene handle. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Wrap a raw animation scene handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(raw)
    }

    /// Wrap a raw handle after checking `DOES_ANIM_SCENE_EXIST`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        let scene = Self::from_raw(raw)?;
        scene.exists(api).then_some(scene)
    }

    /// Raw handle.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// `ANIMSCENE::DOES_ANIM_SCENE_EXIST`.
    pub fn exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::DOES_ANIM_SCENE_EXIST(api, self.0)
    }

    /// `ANIMSCENE::IS_ANIM_SCENE_LOADED` with `p1 = true`, `p2 = false`.
    pub fn is_loaded(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::IS_ANIM_SCENE_LOADED(api, self.0, true, false)
    }

    /// `ANIMSCENE::IS_ANIM_SCENE_METADATA_LOADED` with `p1 = false`.
    pub fn is_metadata_loaded(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::IS_ANIM_SCENE_METADATA_LOADED(api, self.0, false)
    }

    /// `ANIMSCENE::_IS_ANIM_SCENE_LOADING` with `p1 = true`.
    pub fn is_loading(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::_IS_ANIM_SCENE_LOADING(api, self.0, true)
    }

    /// `ANIMSCENE::IS_ANIM_SCENE_RUNNING` with `p1 = false`.
    pub fn is_running(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::IS_ANIM_SCENE_RUNNING(api, self.0, false)
    }

    /// `ANIMSCENE::IS_ANIM_SCENE_FINISHED` with `p1 = false`.
    pub fn is_finished(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::IS_ANIM_SCENE_FINISHED(api, self.0, false)
    }

    /// `ANIMSCENE::HAS_ANIM_SCENE_EXITED` with `p1 = false`.
    pub fn is_exited(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::HAS_ANIM_SCENE_EXITED(api, self.0, false)
    }

    /// `ANIMSCENE::_IS_ANIM_SCENE_ABORTED`.
    pub fn is_aborted(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::_IS_ANIM_SCENE_ABORTED(api, self.0)
    }

    /// `ANIMSCENE::_IS_ANIM_SCENE_SKIPPABLE`.
    pub fn is_skippable(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::_IS_ANIM_SCENE_SKIPPABLE(api, self.0)
    }

    /// `ANIMSCENE::_IS_ANIM_SCENE_PAUSED`.
    pub fn is_paused(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_animscene::_IS_ANIM_SCENE_PAUSED(api, self.0)
    }

    /// `ANIMSCENE::SET_ANIM_SCENE_PAUSED`.
    pub fn set_paused(self, api: &mut dyn GameApi, paused: bool) {
        if self.0 != 0 {
            n_animscene::SET_ANIM_SCENE_PAUSED(api, self.0, paused);
        }
    }

    /// Animation dictionary hash (`ANIMSCENE::_GET_ANIM_SCENE_DICT`).
    pub fn dictionary(self, api: &mut dyn GameApi) -> u32 {
        if self.0 == 0 {
            return 0;
        }
        n_animscene::_GET_ANIM_SCENE_DICT(api, self.0)
    }

    /// Normalized playback phase (`ANIMSCENE::GET_ANIM_SCENE_PHASE`).
    pub fn phase(self, api: &mut dyn GameApi) -> f32 {
        if self.0 == 0 {
            return 0.0;
        }
        n_animscene::GET_ANIM_SCENE_PHASE(api, self.0)
    }

    /// Total scene duration in seconds (`ANIMSCENE::_GET_ANIM_SCENE_DURATION`).
    pub fn duration(self, api: &mut dyn GameApi) -> f32 {
        if self.0 == 0 {
            return 0.0;
        }
        n_animscene::_GET_ANIM_SCENE_DURATION(api, self.0)
    }

    /// Current playback time in seconds (`ANIMSCENE::_GET_ANIM_SCENE_TIME`).
    pub fn time(self, api: &mut dyn GameApi) -> f32 {
        if self.0 == 0 {
            return 0.0;
        }
        n_animscene::_GET_ANIM_SCENE_TIME(api, self.0)
    }

    /// Playback rate (`ANIMSCENE::_GET_ANIM_SCENE_RATE`).
    pub fn rate(self, api: &mut dyn GameApi) -> f32 {
        if self.0 == 0 {
            return 0.0;
        }
        n_animscene::_GET_ANIM_SCENE_RATE(api, self.0)
    }

    /// Set the playback rate (`ANIMSCENE::SET_ANIM_SCENE_RATE`). Non-finite
    /// values are ignored.
    pub fn set_rate(self, api: &mut dyn GameApi, rate: f32) {
        if self.0 != 0 && rate.is_finite() {
            n_animscene::SET_ANIM_SCENE_RATE(api, self.0, rate);
        }
    }

    /// Request loading, unless the scene is already loaded or loading.
    ///
    /// Non-blocking: poll [`AnimScene::is_loaded`] from the tick.
    pub fn load(self, api: &mut dyn GameApi) {
        if self.0 != 0 && !self.is_loaded(api) && !self.is_loading(api) {
            n_animscene::LOAD_ANIM_SCENE(api, self.0);
        }
    }

    /// Start the scene when it is loaded and not already running.
    ///
    /// Non-blocking: poll [`AnimScene::is_finished`] or
    /// [`AnimScene::is_exited`] from the tick.
    pub fn start(self, api: &mut dyn GameApi) {
        if self.0 != 0 && self.is_loaded(api) && !self.is_running(api) {
            n_animscene::START_ANIM_SCENE(api, self.0);
        }
    }

    /// Abort the scene (`ANIMSCENE::ABORT_ANIM_SCENE` with `p1 = true`).
    pub fn abort(self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            n_animscene::ABORT_ANIM_SCENE(api, self.0, true);
        }
    }

    /// Trigger a skip when the scene reports itself as skippable.
    pub fn skip(self, api: &mut dyn GameApi) {
        if self.0 != 0 && self.is_skippable(api) {
            n_animscene::TRIGGER_ANIM_SCENE_SKIP(api, self.0);
        }
    }

    /// Bind an entity to a named slot in the scene
    /// (`ANIMSCENE::SET_ANIM_SCENE_ENTITY` with `flags = 0`, matching the C#
    /// API).
    pub fn add_entity(self, api: &mut dyn GameApi, entity_id: &str, entity: Entity) {
        if self.0 != 0 {
            n_animscene::SET_ANIM_SCENE_ENTITY(api, self.0, entity_id, entity.raw(), 0);
        }
    }

    /// Set the scene origin and rotation. Rotation order `2` matches the C# API.
    /// Non-finite values are ignored.
    pub fn set_origin(self, api: &mut dyn GameApi, position: Vec3, rotation: Vec3) {
        if self.0 == 0 || !position.is_finite() || !rotation.is_finite() {
            return;
        }

        n_animscene::SET_ANIM_SCENE_ORIGIN_VEC3(api, self.0, position, rotation, 2);
    }

    /// Delete the scene and invalidate this handle.
    pub fn delete(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            n_animscene::_DELETE_ANIM_SCENE(api, self.0);
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
        assert!(AnimScene::from_raw(0).is_none());
        assert!(!AnimScene::from_raw_unchecked(0).exists(&mut TestGame::new()));
    }

    #[test]
    fn delete_clears_the_handle() {
        let mut game = TestGame::new();
        let mut scene = AnimScene::from_raw(1).unwrap();

        scene.delete(&mut game);

        assert_eq!(scene.raw(), 0);
        assert!(!scene.exists(&mut game));
    }

    #[test]
    fn non_finite_origin_is_ignored() {
        let mut game = TestGame::new();
        let scene = AnimScene::from_raw(1).unwrap();

        scene.set_origin(&mut game, Vec3::new(f32::NAN, 0.0, 0.0), Vec3::default());
        assert!(game.calls.is_empty());
    }
}

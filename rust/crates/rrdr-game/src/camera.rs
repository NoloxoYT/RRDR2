//! Scripted cameras and gameplay-camera queries.
//!
//! [`Camera`] wraps a scripted camera handle from the `CAM` namespace. Like
//! entity handles, a camera handle is a weak reference: the game can destroy
//! the camera and recycle the handle, so revalidate with [`Camera::exists`]
//! when freshness matters.
//!
//! The native API exposes setters but no getters for the near and far clip
//! planes, so only [`Camera::set_near_clip`] and [`Camera::set_far_clip`] are
//! available. RDR2 also has no native that sets the normal gameplay camera's
//! FOV; [`GameplayCamera::set_fov`] documents the closest verified native.

use crate::entity::Entity;
use crate::ped::{BoneId, Ped};
use crate::Vec3Ops;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::cam as n_cam;

/// Euler rotation order used by the ScriptHook camera API (`2`).
const ROTATION_ORDER: i32 = 2;

/// Ease time used by ScriptHook's `Camera.Render` and `World.RenderingCamera`
/// defaults, in milliseconds.
const DEFAULT_EASE_TIME_MS: i32 = 3000;

/// Value ScriptHook's `World.CreateCamera` passes as the final
/// `CREATE_CAM_WITH_PARAMS` argument.
const CREATE_CAMERA_P9: i32 = 2;

/// A scripted camera.
///
/// This is a non-owning weak reference, not an ownership token. Call
/// [`Camera::delete`] for explicit cleanup; there is no `Drop` that calls
/// natives.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Camera(i32);

impl Camera {
    /// Wrap a raw camera handle without validating it. Rejects zero.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Wrap a raw camera handle known to be valid by construction.
    pub const fn from_raw_unchecked(raw: i32) -> Self {
        Self(raw)
    }

    /// Wrap a raw camera handle after checking `DOES_CAM_EXIST`.
    pub fn try_from_raw(api: &mut dyn GameApi, raw: i32) -> Option<Self> {
        let camera = Self::from_raw(raw)?;
        camera.exists(api).then_some(camera)
    }

    /// Raw camera handle.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// `DOES_CAM_EXIST`.
    pub fn exists(self, api: &mut dyn GameApi) -> bool {
        self.0 != 0 && n_cam::DOES_CAM_EXIST(api, self.0)
    }

    /// Destroy the camera and invalidate this handle.
    ///
    /// Any other copies of the handle become stale.
    pub fn delete(&mut self, api: &mut dyn GameApi) {
        if self.0 != 0 {
            n_cam::DESTROY_CAM(api, self.0, false);
            self.0 = 0;
        }
    }

    /// `IS_CAM_INTERPOLATING`.
    pub fn is_interpolating(self, api: &mut dyn GameApi) -> bool {
        n_cam::IS_CAM_INTERPOLATING(api, self.0)
    }

    /// `IS_CAM_SHAKING`.
    pub fn is_shaking(self, api: &mut dyn GameApi) -> bool {
        n_cam::IS_CAM_SHAKING(api, self.0)
    }

    /// `IS_CAM_RENDERING`.
    pub fn is_rendering(self, api: &mut dyn GameApi) -> bool {
        n_cam::IS_CAM_RENDERING(api, self.0)
    }

    /// `IS_CAM_ACTIVE`.
    pub fn is_active(self, api: &mut dyn GameApi) -> bool {
        n_cam::IS_CAM_ACTIVE(api, self.0)
    }

    /// `SET_CAM_ACTIVE`.
    pub fn set_active(self, api: &mut dyn GameApi, active: bool) {
        n_cam::SET_CAM_ACTIVE(api, self.0, active);
    }

    /// `SET_CAM_NEAR_CLIP`.
    ///
    /// There is no getter native; non-finite or non-positive values are
    /// ignored.
    pub fn set_near_clip(self, api: &mut dyn GameApi, near_clip: f32) {
        if near_clip.is_finite() && near_clip > 0.0 {
            n_cam::SET_CAM_NEAR_CLIP(api, self.0, near_clip);
        }
    }

    /// `SET_CAM_FAR_CLIP`.
    ///
    /// There is no getter native; non-finite or non-positive values are
    /// ignored.
    pub fn set_far_clip(self, api: &mut dyn GameApi, far_clip: f32) {
        if far_clip.is_finite() && far_clip > 0.0 {
            n_cam::SET_CAM_FAR_CLIP(api, self.0, far_clip);
        }
    }

    /// `GET_CAM_FOV`.
    pub fn fov(self, api: &mut dyn GameApi) -> f32 {
        n_cam::GET_CAM_FOV(api, self.0)
    }

    /// `SET_CAM_FOV`.
    ///
    /// Non-finite or non-positive values are ignored.
    pub fn set_fov(self, api: &mut dyn GameApi, fov: f32) {
        if fov.is_finite() && fov > 0.0 {
            n_cam::SET_CAM_FOV(api, self.0, fov);
        }
    }

    /// `GET_CAM_COORD`.
    pub fn position(self, api: &mut dyn GameApi) -> Vec3 {
        n_cam::GET_CAM_COORD(api, self.0)
    }

    /// `SET_CAM_COORD`. Rejects non-finite positions.
    pub fn set_position(self, api: &mut dyn GameApi, position: Vec3) {
        if position.is_finite() {
            n_cam::SET_CAM_COORD_VEC3(api, self.0, position);
        }
    }

    /// World rotation in degrees (`GET_CAM_ROT` with rotation order 2).
    pub fn rotation(self, api: &mut dyn GameApi) -> Vec3 {
        n_cam::GET_CAM_ROT(api, self.0, ROTATION_ORDER)
    }

    /// Set world rotation in degrees (`SET_CAM_ROT` with rotation order 2).
    pub fn set_rotation(self, api: &mut dyn GameApi, rotation: Vec3) {
        if rotation.is_finite() {
            n_cam::SET_CAM_ROT_VEC3(api, self.0, rotation, ROTATION_ORDER);
        }
    }

    /// Unit direction the camera is facing, computed from its rotation.
    ///
    /// Returns a zero vector if the game reports a non-finite rotation.
    pub fn direction(self, api: &mut dyn GameApi) -> Vec3 {
        let rotation = self.rotation(api);
        if rotation.is_finite() {
            direction_from_rotation(rotation)
        } else {
            Vec3::default()
        }
    }

    /// Forward vector; alias of [`Camera::direction`].
    pub fn forward_vector(self, api: &mut dyn GameApi) -> Vec3 {
        self.direction(api)
    }

    /// Convert a camera-local offset to world coordinates.
    ///
    /// The offset is interpreted as `x` right, `y` forward, `z` up. Roll is
    /// applied around the forward axis. Returns `None` if the offset or the
    /// camera transform is non-finite.
    pub fn offset_in_world_coords(self, api: &mut dyn GameApi, offset: Vec3) -> Option<Vec3> {
        if !offset.is_finite() {
            return None;
        }

        let position = self.position(api);
        let rotation = self.rotation(api);
        (position.is_finite() && rotation.is_finite())
            .then(|| offset_in_camera_space(position, rotation, offset))
    }

    /// Render this camera as the active scripted camera.
    ///
    /// Calls `RENDER_SCRIPT_CAMS` with the same parameters as ScriptHook's
    /// `Camera.Render()` defaults: no easing and a 3000 ms ease time.
    pub fn render(self, api: &mut dyn GameApi) {
        render_script_cams(api, true, false, DEFAULT_EASE_TIME_MS);
    }

    /// `SHAKE_CAM`.
    ///
    /// `shake_name` must be a valid camera shake name; the game ignores
    /// unknown names. Empty names, negative amplitudes, and non-finite
    /// amplitudes are ignored.
    pub fn shake(self, api: &mut dyn GameApi, shake_name: &str, amplitude: f32) {
        if !shake_name.is_empty() && amplitude.is_finite() && amplitude >= 0.0 {
            n_cam::SHAKE_CAM(api, self.0, shake_name, amplitude);
        }
    }

    /// `POINT_CAM_AT_COORD`. Rejects non-finite positions.
    pub fn point_at(self, api: &mut dyn GameApi, position: Vec3) {
        if position.is_finite() {
            n_cam::POINT_CAM_AT_COORD_VEC3(api, self.0, position);
        }
    }

    /// `ATTACH_CAM_TO_ENTITY`, relative to the entity.
    ///
    /// Does nothing for a zero entity handle or a non-finite offset.
    pub fn attach_to_entity(self, api: &mut dyn GameApi, entity: Entity, offset: Vec3) {
        if entity.raw() != 0 && offset.is_finite() {
            n_cam::ATTACH_CAM_TO_ENTITY_VEC3(api, self.0, entity.raw(), offset, true);
        }
    }

    /// `ATTACH_CAM_TO_PED_BONE`, relative to the ped bone.
    ///
    /// Does nothing for a zero ped handle, an invalid bone, or a non-finite
    /// offset.
    pub fn attach_to_ped_bone(self, api: &mut dyn GameApi, ped: Ped, bone: BoneId, offset: Vec3) {
        if ped.raw() != 0 && bone.raw() >= 0 && offset.is_finite() {
            n_cam::ATTACH_CAM_TO_PED_BONE_VEC3(api, self.0, ped.raw(), bone.raw(), offset, true);
        }
    }
}

/// Create a scripted camera.
///
/// `name` is a camera type such as `DEFAULT_SCRIPTED_CAMERA`. The native is
/// `CREATE_CAM_WITH_PARAMS`, called with the same flags as ScriptHook's
/// `World.CreateCamera`. Returns `None` when an argument is invalid or the
/// game rejects the request.
pub fn create_camera(
    api: &mut dyn GameApi,
    name: &str,
    position: Vec3,
    rotation: Vec3,
    fov: f32,
) -> Option<Camera> {
    if name.is_empty()
        || !position.is_finite()
        || !rotation.is_finite()
        || !fov.is_finite()
        || fov <= 0.0
    {
        return None;
    }

    let raw = n_cam::CREATE_CAM_WITH_PARAMS_VEC3(
        api,
        name,
        position,
        rotation,
        fov,
        true,
        CREATE_CAMERA_P9,
    );
    Camera::from_raw(raw)
}

/// The camera currently being rendered (`GET_RENDERING_CAM`).
///
/// The handle is not validated with `DOES_CAM_EXIST`, because the gameplay
/// camera is not a scripted camera and is not guaranteed to pass that check.
pub fn rendering_camera(api: &mut dyn GameApi) -> Option<Camera> {
    Camera::from_raw(n_cam::GET_RENDERING_CAM(api))
}

/// Make `camera` the rendering camera, or return to the gameplay camera.
///
/// A `Some` camera is activated first, then scripted-camera rendering starts;
/// `None` stops scripted-camera rendering. This matches ScriptHook's
/// `World.RenderingCamera` setter.
pub fn set_rendering_camera(api: &mut dyn GameApi, camera: Option<Camera>) {
    match camera {
        Some(camera) => {
            camera.set_active(api, true);
            render_script_cams(api, true, false, DEFAULT_EASE_TIME_MS);
        }
        None => render_script_cams(api, false, false, DEFAULT_EASE_TIME_MS),
    }
}

/// Queries for the game's gameplay camera.
///
/// This type has no handle: the gameplay camera is always present while the
/// game is running.
pub struct GameplayCamera;

impl GameplayCamera {
    /// `GET_GAMEPLAY_CAM_COORD`.
    pub fn position(api: &mut dyn GameApi) -> Vec3 {
        n_cam::GET_GAMEPLAY_CAM_COORD(api)
    }

    /// `GET_GAMEPLAY_CAM_ROT` with rotation order 2, in degrees.
    pub fn rotation(api: &mut dyn GameApi) -> Vec3 {
        n_cam::GET_GAMEPLAY_CAM_ROT(api, ROTATION_ORDER)
    }

    /// Unit direction the gameplay camera is facing, computed from its
    /// rotation.
    ///
    /// Returns a zero vector if the game reports a non-finite rotation.
    pub fn direction(api: &mut dyn GameApi) -> Vec3 {
        let rotation = Self::rotation(api);
        if rotation.is_finite() {
            direction_from_rotation(rotation)
        } else {
            Vec3::default()
        }
    }

    /// `GET_GAMEPLAY_CAM_FOV`.
    pub fn fov(api: &mut dyn GameApi) -> f32 {
        n_cam::GET_GAMEPLAY_CAM_FOV(api)
    }

    /// Set the gameplay hint camera FOV.
    ///
    /// RDR2 has no native that changes the normal gameplay camera's FOV. This
    /// calls `SET_GAMEPLAY_HINT_FOV`, which only has an effect while a
    /// gameplay hint camera is active. For a persistent FOV, create a
    /// [`Camera`] and make it the rendering camera. Non-finite or non-positive
    /// values are ignored.
    pub fn set_fov(api: &mut dyn GameApi, fov: f32) {
        if fov.is_finite() && fov > 0.0 {
            n_cam::SET_GAMEPLAY_HINT_FOV(api, fov);
        }
    }

    /// `IS_AIM_CAM_ACTIVE`.
    pub fn is_aim_cam_active(api: &mut dyn GameApi) -> bool {
        n_cam::IS_AIM_CAM_ACTIVE(api)
    }

    /// `IS_FIRST_PERSON_AIM_CAM_ACTIVE`.
    pub fn is_first_person_aim_cam_active(api: &mut dyn GameApi) -> bool {
        n_cam::IS_FIRST_PERSON_AIM_CAM_ACTIVE(api)
    }

    /// `IS_GAMEPLAY_CAM_LOOKING_BEHIND`.
    pub fn is_looking_behind(api: &mut dyn GameApi) -> bool {
        n_cam::IS_GAMEPLAY_CAM_LOOKING_BEHIND(api)
    }

    /// `IS_GAMEPLAY_CAM_SHAKING`.
    pub fn is_shaking(api: &mut dyn GameApi) -> bool {
        n_cam::IS_GAMEPLAY_CAM_SHAKING(api)
    }

    /// `_IS_IN_FULL_FIRST_PERSON_MODE`.
    pub fn is_in_first_person(api: &mut dyn GameApi) -> bool {
        n_cam::_IS_IN_FULL_FIRST_PERSON_MODE(api)
    }

    /// Whether the gameplay camera is in third person.
    ///
    /// This is the inverse of [`GameplayCamera::is_in_first_person`], matching
    /// ScriptHook's `GameplayCamera.IsInThirdPerson`.
    pub fn is_in_third_person(api: &mut dyn GameApi) -> bool {
        !Self::is_in_first_person(api)
    }
}

/// Shared `RENDER_SCRIPT_CAMS` boilerplate from the C# API.
fn render_script_cams(api: &mut dyn GameApi, render: bool, ease: bool, ease_time_ms: i32) {
    n_cam::RENDER_SCRIPT_CAMS(api, render, ease, ease_time_ms, true, false, 0);
}

/// Convert an Euler rotation in degrees (`x` pitch, `y` roll, `z` yaw) to a
/// unit direction vector.
///
/// `pitch.cos().abs()` mirrors ScriptHookRDR2DotNet's own
/// `DirectionFromRotation`, where the horizontal component is taken as an
/// absolute value. Consequence (documented, not a bug to fix blindly): for
/// |pitch| > 90° the forward vector flips horizontally instead of pointing
/// backwards. Scripted cameras stay within ±90° pitch, which is why the C#
/// API gets away with it.
fn direction_from_rotation(rotation: Vec3) -> Vec3 {
    let pitch = rotation.x.to_radians();
    let yaw = rotation.z.to_radians();
    let horizontal = pitch.cos().abs();

    Vec3::new(-yaw.sin() * horizontal, yaw.cos() * horizontal, pitch.sin())
}

/// Transform a camera-local offset into world space.
fn offset_in_camera_space(position: Vec3, rotation: Vec3, offset: Vec3) -> Vec3 {
    let forward = direction_from_rotation(rotation);
    let yaw = rotation.z.to_radians();
    let roll = rotation.y.to_radians();

    // Level right/up basis, then rotate it around the forward axis by roll.
    let right_level = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let up_level = right_level.cross(forward);
    let (roll_sin, roll_cos) = roll.sin_cos();

    let right = Vec3::new(
        right_level.x * roll_cos + up_level.x * roll_sin,
        right_level.y * roll_cos + up_level.y * roll_sin,
        right_level.z * roll_cos + up_level.z * roll_sin,
    );
    let up = Vec3::new(
        up_level.x * roll_cos - right_level.x * roll_sin,
        up_level.y * roll_cos - right_level.y * roll_sin,
        up_level.z * roll_cos - right_level.z * roll_sin,
    );

    Vec3::new(
        position.x + right.x * offset.x + forward.x * offset.y + up.x * offset.z,
        position.y + right.y * offset.x + forward.y * offset.y + up.y * offset.z,
        position.z + right.z * offset.x + forward.z * offset.y + up.z * offset.z,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: Vec3, expected: Vec3) {
        let epsilon = 1.0e-4;
        assert!(
            (actual.x - expected.x).abs() < epsilon
                && (actual.y - expected.y).abs() < epsilon
                && (actual.z - expected.z).abs() < epsilon,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn rotation_to_direction() {
        assert_near(
            direction_from_rotation(Vec3::new(0.0, 0.0, 0.0)),
            Vec3::new(0.0, 1.0, 0.0),
        );
        assert_near(
            direction_from_rotation(Vec3::new(0.0, 0.0, 90.0)),
            Vec3::new(-1.0, 0.0, 0.0),
        );
        assert_near(
            direction_from_rotation(Vec3::new(90.0, 0.0, 0.0)),
            Vec3::new(0.0, 0.0, 1.0),
        );
    }

    #[test]
    fn local_offsets_are_rotated_into_world_space() {
        let origin = Vec3::default();
        let no_rotation = Vec3::default();

        assert_near(
            offset_in_camera_space(origin, no_rotation, Vec3::new(1.0, 2.0, 3.0)),
            Vec3::new(1.0, 2.0, 3.0),
        );
        assert_near(
            offset_in_camera_space(origin, Vec3::new(0.0, 0.0, 90.0), Vec3::new(1.0, 2.0, 3.0)),
            Vec3::new(-2.0, 1.0, 3.0),
        );
    }

    #[test]
    fn raw_handles_reject_zero() {
        assert!(Camera::from_raw(0).is_none());
        assert_eq!(Camera::from_raw_unchecked(7).raw(), 7);
    }
}

//! Soundset and sound helpers around the `AUDIO` namespace.
//!
//! All functions are fire-and-forget: the game reports completion of a
//! prepared sound through its own state, and this layer never blocks. Play
//! helpers return the sound id allocated for the request, matching
//! ScriptHookRDR2DotNet's `Audio` class.

use crate::Vec3Ops;
use rrdr_bridge::{GameApi, Vec3};
use rrdr_natives::audio as n_audio;

/// Prepare a soundset (`AUDIO::PREPARE_SOUNDSET` with `p1 = false`).
pub fn prepare_soundset(api: &mut dyn GameApi, soundset: &str) -> bool {
    n_audio::PREPARE_SOUNDSET(api, soundset, false)
}

/// Prepare a sound in a previously prepared soundset (`AUDIO::PREPARE_SOUND`
/// with the C# API's `-2` placeholder sound id).
pub fn prepare_sound(api: &mut dyn GameApi, sound_name: &str, soundset: &str) -> bool {
    n_audio::PREPARE_SOUND(api, sound_name, soundset, -2)
}

/// Play a frontend (UI) sound and return the sound id allocated by
/// `AUDIO::GET_SOUND_ID`.
///
/// This mirrors `Audio.PlaySoundFrontend`: the play native is called with
/// `p2 = true`, `p3 = 0`, then a fresh id is fetched.
pub fn play_sound_frontend(api: &mut dyn GameApi, sound_name: &str, soundset: &str) -> i32 {
    n_audio::PLAY_SOUND_FRONTEND(api, sound_name, soundset, true, 0);
    n_audio::GET_SOUND_ID(api)
}

/// Play a non-positional world sound and return the sound id allocated by
/// `AUDIO::GET_SOUND_ID`.
///
/// This mirrors `Audio.PlaySound`: `AUDIO::PLAY_SOUND` is called with
/// `p2 = false`, `p3 = 0`, `p4 = true`, `p5 = 0`.
pub fn play_sound(api: &mut dyn GameApi, sound_name: &str, soundset: &str) -> i32 {
    n_audio::PLAY_SOUND(api, sound_name, soundset, false, 0, true, 0);
    n_audio::GET_SOUND_ID(api)
}

/// Play a sound at a world position and return its sound id.
///
/// Uses `AUDIO::_PLAY_SOUND_FROM_POSITION` (the C# `Audio` class has no
/// positional overload). Non-finite positions are rejected and no native is
/// called, so the result is `None`.
pub fn play_sound_from_position(
    api: &mut dyn GameApi,
    sound_name: &str,
    soundset: &str,
    position: Vec3,
) -> Option<i32> {
    if !position.is_finite() {
        return None;
    }

    n_audio::_PLAY_SOUND_FROM_POSITION(
        api, sound_name, position.x, position.y, position.z, soundset, false, 0, true, 0,
    );
    Some(n_audio::GET_SOUND_ID(api))
}

/// Stop a sound by name and soundset (`AUDIO::_STOP_SOUND_WITH_NAME`).
pub fn stop_sound(api: &mut dyn GameApi, sound_name: &str, soundset: &str) {
    n_audio::_STOP_SOUND_WITH_NAME(api, sound_name, soundset);
}

/// Stop a sound by id (`AUDIO::_STOP_SOUND_WITH_ID`).
pub fn stop_sound_by_id(api: &mut dyn GameApi, sound_id: i32) {
    n_audio::_STOP_SOUND_WITH_ID(api, sound_id);
}

/// Release a sound id back to the audio system (`AUDIO::RELEASE_SOUND_ID`).
///
/// Call this after the sound has finished, not immediately after starting it.
pub fn release_sound(api: &mut dyn GameApi, sound_id: i32) {
    n_audio::RELEASE_SOUND_ID(api, sound_id);
}

/// Set an audio flag by name (`AUDIO::SET_AUDIO_FLAG`).
pub fn set_audio_flag(api: &mut dyn GameApi, flag_name: &str, toggle: bool) {
    n_audio::SET_AUDIO_FLAG(api, flag_name, toggle);
}

/// Set a variable on a named sound
/// (`AUDIO::_SET_VARIABLE_ON_SOUND_WITH_NAME`).
///
/// Non-finite values are ignored.
pub fn set_audio_variable(
    api: &mut dyn GameApi,
    sound_name: &str,
    soundset: &str,
    variable_name: &str,
    value: f32,
) {
    if value.is_finite() {
        n_audio::_SET_VARIABLE_ON_SOUND_WITH_NAME(api, variable_name, value, sound_name, soundset);
    }
}

/// Set a variable on a sound id (`AUDIO::_SET_VARIABLE_ON_SOUND_WITH_ID`).
///
/// Non-finite values are ignored.
pub fn set_audio_variable_by_id(
    api: &mut dyn GameApi,
    sound_id: i32,
    variable_name: &str,
    value: f32,
) {
    if value.is_finite() {
        n_audio::_SET_VARIABLE_ON_SOUND_WITH_ID(api, sound_id, variable_name, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn play_helpers_allocate_a_sound_id() {
        let mut game = TestGame::new();

        assert_eq!(play_sound(&mut game, "sound", "set"), 0);
        assert!(game.calls.contains(&n_audio::HASH_PLAY_SOUND));
        assert!(game.calls.contains(&n_audio::HASH_GET_SOUND_ID));

        game.calls.clear();
        assert_eq!(play_sound_frontend(&mut game, "sound", "set"), 0);
        assert!(game.calls.contains(&n_audio::HASH_PLAY_SOUND_FRONTEND));
        assert!(game.calls.contains(&n_audio::HASH_GET_SOUND_ID));
    }

    #[test]
    fn non_finite_position_is_rejected() {
        let mut game = TestGame::new();

        let result = play_sound_from_position(
            &mut game,
            "sound",
            "set",
            Vec3::new(0.0, f32::INFINITY, 0.0),
        );

        assert_eq!(result, None);
        assert!(game.calls.is_empty());
    }

    #[test]
    fn finite_position_dispatches_positional_play() {
        let mut game = TestGame::new();

        assert_eq!(
            play_sound_from_position(&mut game, "sound", "set", Vec3::new(1.0, 2.0, 3.0)),
            Some(0)
        );
        assert!(game
            .calls
            .contains(&n_audio::HASH__PLAY_SOUND_FROM_POSITION));
    }

    #[test]
    fn non_finite_variable_is_ignored() {
        let mut game = TestGame::new();

        set_audio_variable(&mut game, "sound", "set", "volume", f32::NAN);
        set_audio_variable_by_id(&mut game, 1, "volume", f32::NAN);

        assert!(game.calls.is_empty());
    }
}

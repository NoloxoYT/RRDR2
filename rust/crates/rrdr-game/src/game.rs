//! Global game state, timing, input, and hash helpers.

use rrdr_bridge::GameApi;
use rrdr_natives::{debug as n_debug, hud as n_hud, misc as n_misc, pad as n_pad, ped as n_ped};

/// Frame delta in seconds.
pub fn frame_time(api: &mut dyn GameApi) -> f32 {
    n_misc::GET_FRAME_TIME(api)
}

/// Total rendered frames.
pub fn frame_count(api: &mut dyn GameApi) -> i32 {
    n_misc::GET_FRAME_COUNT(api)
}

/// Game build version string.
pub fn version(api: &mut dyn GameApi) -> String {
    n_debug::GET_GAME_VERSION_NAME(api)
}

/// Pause menu active.
pub fn is_paused(api: &mut dyn GameApi) -> bool {
    n_hud::IS_PAUSE_MENU_ACTIVE(api)
}

/// Set the game pause state.
pub fn set_paused(api: &mut dyn GameApi, paused: bool) {
    n_misc::SET_GAME_PAUSED(api, paused);
}

/// Control pressed this frame. Uses input group 0 (keyboard/controller).
pub fn control_pressed(api: &mut dyn GameApi, control: u32) -> bool {
    n_pad::IS_CONTROL_PRESSED(api, 0, control)
}

/// Control just pressed this frame.
pub fn control_just_pressed(api: &mut dyn GameApi, control: u32) -> bool {
    n_pad::IS_CONTROL_JUST_PRESSED(api, 0, control)
}

/// Analog control value (0.0..1.0).
pub fn control_normal(api: &mut dyn GameApi, control: u32) -> f32 {
    n_pad::GET_CONTROL_NORMAL(api, 0, control)
}

/// Hash a string with the game's `GET_HASH_KEY`.
pub fn hash(api: &mut dyn GameApi, text: &str) -> u32 {
    n_misc::GET_HASH_KEY(api, text)
}

/// Global AI weapon damage multiplier.
pub fn set_ai_weapon_damage_multiplier(api: &mut dyn GameApi, multiplier: f32) {
    if multiplier.is_finite() {
        n_ped::SET_AI_WEAPON_DAMAGE_MODIFIER(api, multiplier.max(0.0));
    }
}

/// Restore the vanilla AI weapon damage multiplier.
pub fn reset_ai_weapon_damage_modifier(api: &mut dyn GameApi) {
    n_ped::RESET_AI_WEAPON_DAMAGE_MODIFIER(api);
}

/// Global AI melee damage multiplier.
pub fn set_ai_melee_weapon_damage_multiplier(api: &mut dyn GameApi, multiplier: f32) {
    if multiplier.is_finite() {
        n_ped::SET_AI_MELEE_WEAPON_DAMAGE_MODIFIER(api, multiplier.max(0.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn time_version_and_pause() {
        let mut game = TestGame::new();

        assert!((frame_time(&mut game) - 0.016).abs() < f32::EPSILON);
        assert_eq!(frame_count(&mut game), 123);
        assert_eq!(version(&mut game), "test-version");
        assert!(!is_paused(&mut game));

        set_paused(&mut game, true);
        assert!(is_paused(&mut game));
    }

    #[test]
    fn controls_and_hash() {
        let mut game = TestGame::new();
        game.control_pressed = true;
        game.control_just_pressed = true;
        game.control_normal = 0.75;
        game.hash = 42;

        assert!(control_pressed(&mut game, 1));
        assert!(control_just_pressed(&mut game, 1));
        assert!((control_normal(&mut game, 1) - 0.75).abs() < f32::EPSILON);
        assert_eq!(hash(&mut game, "test"), 42);
    }

    #[test]
    fn global_damage_modifiers() {
        let mut game = TestGame::new();

        set_ai_weapon_damage_multiplier(&mut game, 1.5);
        set_ai_melee_weapon_damage_multiplier(&mut game, 2.0);
        assert_eq!(game.ai_weapon, Some(1.5));
        assert_eq!(game.ai_melee, Some(2.0));

        reset_ai_weapon_damage_modifier(&mut game);
        assert_eq!(game.ai_weapon, None);
    }
}

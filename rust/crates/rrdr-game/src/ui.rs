//! Screen, drawing, subtitle, and HUD helpers.
//!
//! Screen coordinates are normalized: `(0.0, 0.0)` is the top-left corner and
//! `(1.0, 1.0)` is the bottom-right corner, independent of the render
//! resolution. [`resolution`] returns the pixel dimensions when a mod needs to
//! convert between the two.
//!
//! # Custom text
//!
//! RDR2 text drawing uses an opaque 64-bit string id instead of a normal Rust
//! string. The id is produced by the variadic `MISC::VAR_STRING` native
//! (`0xFA925AC00EB830B9`), which is intentionally not generated in
//! `rrdr-natives` because variadic natives cannot be expressed as a typed Rust
//! function. [`draw_text`] uses the fixed three-argument form that
//! ScriptHookRDR2DotNet uses: flags `10`, the `"LITERAL_STRING"` label, then
//! the text itself.
//!
//! ScriptHookRDR2DotNet runs a one-time text-pool warm-up (`TextPoolInit`)
//! when a script starts. Without it the first custom text draw calls can be
//! dropped by the game. [`warm_up_text`] performs the same warm-up from Rust;
//! call it once after the script has started if custom text is used. The
//! warm-up is a workaround in the managed API rather than a documented native
//! contract, so it is exposed explicitly instead of running implicitly.

use rrdr_bridge::{Arg, GameApi, Vec3};
use rrdr_natives::{
    cam as n_cam, graphics as n_graphics, hud as n_hud, interaction as n_interaction, map as n_map,
    uidebug as n_uidebug, uilog as n_uilog,
};

/// Hash of `MISC::VAR_STRING`, skipped by the bindings because it is variadic.
const HASH_VAR_STRING: u64 = 0xFA925AC00EB830B9;

/// Flags used by ScriptHookRDR2DotNet for a literal, non-localized string.
const VAR_STRING_LITERAL_FLAGS: i32 = 10;

/// Label required by `VAR_STRING` for a literal string.
const VAR_STRING_LITERAL_LABEL: &str = "LITERAL_STRING";

/// Radar zoom lower bound accepted by `MAP::SET_RADAR_ZOOM`.
const RADAR_ZOOM_MIN: i32 = 0;

/// Radar zoom upper bound accepted by `MAP::SET_RADAR_ZOOM`.
const RADAR_ZOOM_MAX: i32 = 200;

/// Current render resolution in pixels.
///
/// Calls `GRAPHICS::GET_SCREEN_RESOLUTION` (`0x66773C92835D0909`). The values
/// are the resolution the game is currently rendering at, which can differ
/// from the desktop resolution when resolution scaling is enabled.
pub fn resolution(api: &mut dyn GameApi) -> (i32, i32) {
    let mut width = 0i32;
    let mut height = 0i32;

    n_graphics::GET_SCREEN_RESOLUTION(api, &mut width, &mut height);

    (width, height)
}

/// Project a world position onto normalized screen coordinates.
///
/// Calls `GRAPHICS::GET_SCREEN_COORD_FROM_WORLD_COORD` (`0xCB50D7AFCC8B0EC6`).
/// Returns `None` when the native reports that the position is not on screen
/// or when it produces a non-finite coordinate. The returned coordinates use
/// the same normalized space as [`draw_rect`] and [`draw_text`].
///
/// The C# `RDR2.UI.Screen.WorldToScreen` helper scales the native result by
/// the 1920x1080 UI reference size; this function returns the native values
/// unscaled so they can be passed directly to the drawing helpers.
pub fn world_to_screen(api: &mut dyn GameApi, position: Vec3) -> Option<(f32, f32)> {
    if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
        return None;
    }

    let mut screen_x = 0.0f32;
    let mut screen_y = 0.0f32;

    let visible = n_graphics::GET_SCREEN_COORD_FROM_WORLD_COORD(
        api,
        position.x,
        position.y,
        position.z,
        &mut screen_x,
        &mut screen_y,
    );

    if visible && screen_x.is_finite() && screen_y.is_finite() {
        Some((screen_x, screen_y))
    } else {
        None
    }
}

/// Show a subtitle through the game's objective log.
///
/// This mirrors ScriptHookRDR2DotNet's `RDR2.UI.Screen.PrintSubtitle`, which
/// calls `UILOG::_UILOG_SET_CACHED_OBJECTIVE` (`0xFA233F8FE190514C`), prints it,
/// then clears the cached objective. The UILOG path has no duration argument,
/// so `duration_ms` is only a validity gate: a non-positive value or empty text
/// does nothing, while the game controls how long the line remains visible.
///
/// The C# API calls this a subtitle, but visually it is the objective banner
/// used by the game rather than a dialogue subtitle tied to an audio line.
pub fn print_subtitle(api: &mut dyn GameApi, text: &str, duration_ms: i32) {
    if text.is_empty() || duration_ms <= 0 {
        return;
    }

    n_uilog::_UILOG_SET_CACHED_OBJECTIVE(api, text);
    n_uilog::_UILOG_PRINT_CACHED_OBJECTIVE(api);
    n_uilog::_UILOG_CLEAR_HAS_DISPLAYED_CACHED_OBJECTIVE(api);
    n_uilog::_UILOG_CLEAR_CACHED_OBJECTIVE(api);
}

/// Fade the screen to black over `duration_ms` milliseconds.
///
/// Calls `CAM::DO_SCREEN_FADE_OUT` (`0x40C719A5E410B9E4`). Negative durations
/// are clamped to zero.
pub fn fade_out(api: &mut dyn GameApi, duration_ms: i32) {
    n_cam::DO_SCREEN_FADE_OUT(api, duration_ms.max(0));
}

/// Fade the screen back in from black over `duration_ms` milliseconds.
///
/// Calls `CAM::DO_SCREEN_FADE_IN` (`0x6A053CF596F67DF7`). Negative durations
/// are clamped to zero.
pub fn fade_in(api: &mut dyn GameApi, duration_ms: i32) {
    n_cam::DO_SCREEN_FADE_IN(api, duration_ms.max(0));
}

/// Draw a solid rectangle centered on `(center_x, center_y)`.
///
/// Calls `GRAPHICS::DRAW_RECT` (`0x405224591DF02025`) with the trailing
/// parameters set to `false`, matching ScriptHookRDR2DotNet. Coordinates and
/// size are normalized to the screen, so a full-screen rectangle is
/// `(0.5, 0.5, 1.0, 1.0, ...)`. Color components are clamped to `0..=255`.
///
/// Non-finite coordinates or dimensions are ignored.
#[allow(clippy::too_many_arguments)]
pub fn draw_rect(
    api: &mut dyn GameApi,
    center_x: f32,
    center_y: f32,
    width: f32,
    height: f32,
    red: i32,
    green: i32,
    blue: i32,
    alpha: i32,
) {
    if !center_x.is_finite() || !center_y.is_finite() || !width.is_finite() || !height.is_finite() {
        return;
    }

    n_graphics::DRAW_RECT(
        api,
        center_x,
        center_y,
        width,
        height,
        color_component(red),
        color_component(green),
        color_component(blue),
        color_component(alpha),
        false,
        false,
    );
}

/// Draw a line of custom text at normalized screen coordinates.
///
/// The text is registered with `MISC::VAR_STRING` and displayed with
/// `UIDEBUG::_BG_DISPLAY_TEXT` (`0x16794E044C9EFB58`). `scale` is applied to
/// both axes via `UIDEBUG::_BG_SET_TEXT_SCALE` (`0xA1253A3C870B6843`), and the
/// color goes through `UIDEBUG::_BG_SET_TEXT_COLOR` (`0x16FA5CE47F184F1E`).
/// Color components are clamped to `0..=255`.
///
/// Non-finite coordinates or scale are ignored. Custom text may not appear
/// until [`warm_up_text`] has been called once; see the module documentation.
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    api: &mut dyn GameApi,
    text: &str,
    x: f32,
    y: f32,
    scale: f32,
    red: i32,
    green: i32,
    blue: i32,
    alpha: i32,
) {
    if !x.is_finite() || !y.is_finite() || !scale.is_finite() {
        return;
    }

    let string_id = text_string_id(api, text);

    n_uidebug::_BG_SET_TEXT_SCALE(api, scale, scale);
    n_uidebug::_BG_SET_TEXT_COLOR(
        api,
        color_component(red),
        color_component(green),
        color_component(blue),
        color_component(alpha),
    );

    // `_BG_DISPLAY_TEXT` takes the raw `VAR_STRING` id. Passing it as
    // `Arg::U64` is required; the generated wrapper only accepts `&str` and
    // cannot forward the raw id.
    api.call(
        n_uidebug::HASH__BG_DISPLAY_TEXT,
        &[Arg::U64(string_id), Arg::F32(x), Arg::F32(y)],
    );
}

/// Initialize the game's custom-text pool.
///
/// ScriptHookRDR2DotNet's core runs this loop once when a script starts
/// because `UIDEBUG::_BG_DISPLAY_TEXT` can silently drop early draws otherwise.
/// It makes 1024 blank text draws at increasing coordinates, so call it once
/// during script initialization, not every tick.
///
/// The warm-up is a ScriptHookRDR2DotNet workaround rather than a documented
/// native requirement. It is harmless if another script already initialized
/// the pool, but wasteful.
///
/// This is the one-shot form: 4096 marshaled native calls in a single tick,
/// which is a guaranteed multi-hundred-millisecond hitch at init. Prefer
/// [`TextWarmUp`], which spreads the same loop over several ticks.
pub fn warm_up_text(api: &mut dyn GameApi) {
    let mut warm_up = TextWarmUp::new();
    while !warm_up.advance(api) {}
}

/// Chunked version of [`warm_up_text`].
///
/// Drives the same 1024 blank draws but only `DEFAULT_DRAWS_PER_TICK` (64) of
/// them per call, so no single frame eats the whole hitch. Call
/// [`TextWarmUp::advance`] every tick until it returns `true`:
///
/// ```ignore
/// let mut warm_up = TextWarmUp::new();
/// // in the tick:
/// if !warm_up.advance(api) {
///     return; // not warmed up yet, skip drawing this frame
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextWarmUp {
    total: u32,
    per_tick: u32,
    next: u32,
}

/// Blank draws one [`TextWarmUp`] performs per tick by default (16 ticks).
pub const TEXT_WARM_UP_DRAWS_PER_TICK: u32 = 64;

/// Total blank draws one warm-up performs (ScriptHookRDR2DotNet's loop count).
pub const TEXT_WARM_UP_TOTAL: u32 = 1024;

impl Default for TextWarmUp {
    fn default() -> Self {
        Self::new()
    }
}

impl TextWarmUp {
    /// A warm-up of the standard 1024 draws, 64 per tick.
    pub fn new() -> Self {
        Self {
            total: TEXT_WARM_UP_TOTAL,
            per_tick: TEXT_WARM_UP_DRAWS_PER_TICK,
            next: 0,
        }
    }

    /// A warm-up with custom pacing. `per_tick` is clamped to at least 1 so a
    /// zero cannot turn [`TextWarmUp::advance`] into an infinite loop.
    pub fn with_pacing(per_tick: u32) -> Self {
        Self {
            total: TEXT_WARM_UP_TOTAL,
            per_tick: per_tick.clamp(1, TEXT_WARM_UP_TOTAL),
            next: 0,
        }
    }

    /// True once every draw has been issued.
    pub fn finished(&self) -> bool {
        self.next >= self.total
    }

    /// Issue the next chunk of blank draws. Returns `true` when the warm-up is
    /// finished and custom text can be drawn.
    pub fn advance(&mut self, api: &mut dyn GameApi) -> bool {
        if self.finished() {
            return true;
        }

        let end = self.next.saturating_add(self.per_tick).min(self.total);
        for step in self.next..end {
            let offset = step as f32 * 0.01;

            n_uidebug::_BG_SET_TEXT_SCALE(api, 0.1, 0.1);
            n_uidebug::_BG_SET_TEXT_COLOR(api, 255, 255, 255, 255);

            let string_id = text_string_id(api, " \r\n\t ");

            api.call(
                n_uidebug::HASH__BG_DISPLAY_TEXT,
                &[Arg::U64(string_id), Arg::F32(offset), Arg::F32(offset)],
            );
        }
        self.next = end;

        self.finished()
    }
}

/// One argument of a variadic `MISC::VAR_STRING` call.
///
/// Only the literal-string form (`flags = 10`, label `LITERAL_STRING`) is
/// verified against ScriptHookRDR2DotNet's behaviour; the localized/format
/// variants are not exercised by this helper. See [`var_string`].
#[derive(Debug, Clone, Copy)]
pub enum VarStringArg<'a> {
    /// Integer argument.
    I32(i32),
    /// Float argument.
    F32(f32),
    /// Literal string argument (passed as a `LITERAL_STRING` label).
    Text(&'a str),
    /// Raw 64-bit value (hash, string id, ...).
    Hash(u64),
}

/// Call variadic `MISC::VAR_STRING` (`0xFA925AC00EB830B9`) with an arbitrary
/// argument list and return the opaque 64-bit string id.
///
/// [`warm_up_text`]/[`draw_text`] use the fixed three-argument literal form;
/// this helper is the supported path for N-argument forms that the generated
/// bindings cannot express. The label argument required for literal strings is
/// inserted automatically.
pub fn var_string(api: &mut dyn GameApi, args: &[VarStringArg<'_>]) -> u64 {
    let mut native_args: Vec<Arg<'_>> = Vec::with_capacity(args.len() + 2);
    native_args.push(Arg::I32(VAR_STRING_LITERAL_FLAGS));
    native_args.push(Arg::Str(VAR_STRING_LITERAL_LABEL));

    for argument in args {
        match *argument {
            VarStringArg::I32(value) => native_args.push(Arg::I32(value)),
            VarStringArg::F32(value) => native_args.push(Arg::F32(value)),
            VarStringArg::Text(text) => native_args.push(Arg::Str(text)),
            VarStringArg::Hash(value) => native_args.push(Arg::U64(value)),
        }
    }

    api.call(HASH_VAR_STRING, &native_args)
}

/// Whether the HUD is currently visible.
///
/// `HUD::IS_HUD_HIDDEN` (`0x71B72B478F8189DC`) is inverted, matching
/// ScriptHookRDR2DotNet's `RDR2.UI.Hud.IsVisible`.
pub fn is_visible(api: &mut dyn GameApi) -> bool {
    !n_hud::IS_HUD_HIDDEN(api)
}

/// Show or hide the HUD.
///
/// Calls `HUD::DISPLAY_HUD` (`0xD63FE3AF9FB3D53F`).
pub fn set_visible(api: &mut dyn GameApi, visible: bool) {
    n_hud::DISPLAY_HUD(api, visible);
}

/// Whether the radar is currently visible.
///
/// `HUD::IS_RADAR_HIDDEN` (`0x1B82FD5FFA4D666E`) is inverted, matching
/// ScriptHookRDR2DotNet's `RDR2.UI.Hud.IsRadarVisible`.
pub fn is_radar_visible(api: &mut dyn GameApi) -> bool {
    !n_hud::IS_RADAR_HIDDEN(api)
}

/// Show or hide the radar.
///
/// Calls `MAP::DISPLAY_RADAR` (`0x1B3DA717B9AFF828`).
pub fn set_radar_visible(api: &mut dyn GameApi, visible: bool) {
    n_map::DISPLAY_RADAR(api, visible);
}

/// Show the mouse cursor for the current frame.
///
/// Calls `INTERACTION::SET_MOUSE_CURSOR_THIS_FRAME` (`0xF12E4CCAF249DC10`).
/// The call must be repeated every frame the cursor should be visible.
pub fn show_cursor_this_frame(api: &mut dyn GameApi) {
    n_interaction::SET_MOUSE_CURSOR_THIS_FRAME(api);
}

/// Set the radar zoom level.
///
/// Calls `MAP::SET_RADAR_ZOOM` (`0xCAF6489DA2C8DD9E`). Values are clamped to
/// the `0..=200` range documented by ScriptHookRDR2DotNet.
pub fn set_radar_zoom(api: &mut dyn GameApi, zoom_level: i32) {
    n_map::SET_RADAR_ZOOM(api, zoom_level.clamp(RADAR_ZOOM_MIN, RADAR_ZOOM_MAX));
}

/// Whether the pause menu is active.
///
/// Calls `HUD::IS_PAUSE_MENU_ACTIVE` (`0x535384D6067BA42E`).
pub fn is_pause_menu_active(api: &mut dyn GameApi) -> bool {
    n_hud::IS_PAUSE_MENU_ACTIVE(api)
}

/// Create the opaque string id used by the UI text natives.
///
/// `MISC::VAR_STRING` (`0xFA925AC00EB830B9`) is variadic, so it has no typed
/// binding in `rrdr-natives`. The return value is raw: `api.call` must be used
/// instead of `call_string`, because the UI natives need the 64-bit id rather
/// than a decoded Rust `String`.
fn text_string_id(api: &mut dyn GameApi, text: &str) -> u64 {
    api.call(
        HASH_VAR_STRING,
        &[
            Arg::I32(VAR_STRING_LITERAL_FLAGS),
            Arg::Str(VAR_STRING_LITERAL_LABEL),
            Arg::Str(text),
        ],
    )
}

/// Clamp one color component to the native's `0..=255` range.
fn color_component(value: i32) -> i32 {
    value.clamp(0, 255)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestGame;

    #[test]
    fn non_finite_drawing_is_ignored() {
        let mut game = TestGame::new();

        draw_rect(&mut game, f32::NAN, 0.5, 0.1, 0.1, 255, 0, 0, 255);
        draw_text(
            &mut game,
            "test",
            0.5,
            f32::INFINITY,
            0.3,
            255,
            255,
            255,
            255,
        );

        assert!(game.calls.is_empty());
    }

    #[test]
    fn text_draw_uses_var_string_then_display() {
        let mut game = TestGame::new();

        draw_text(&mut game, "test", 0.5, 0.5, 0.3, 10, 20, 30, 40);

        assert_eq!(
            game.calls,
            vec![
                HASH_VAR_STRING,
                n_uidebug::HASH__BG_SET_TEXT_SCALE,
                n_uidebug::HASH__BG_SET_TEXT_COLOR,
                n_uidebug::HASH__BG_DISPLAY_TEXT,
            ]
        );
    }

    #[test]
    fn radar_zoom_is_clamped() {
        let mut game = TestGame::new();

        set_radar_zoom(&mut game, -5);
        set_radar_zoom(&mut game, 500);

        assert_eq!(game.calls.len(), 2);
    }

    #[test]
    fn subtitle_needs_positive_duration() {
        let mut game = TestGame::new();

        print_subtitle(&mut game, "", 1000);
        print_subtitle(&mut game, "text", 0);

        assert!(game.calls.is_empty());
    }
}

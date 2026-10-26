//! In-game clock and weather controls.
//!
//! Time uses the `CLOCK` natives. Weather uses the `MISC` weather-state
//! natives, following the semantics of ScriptHook's `World` class:
//! [`current`] reports the visually dominant weather and [`next`] reports the
//! transition target.
//!
//! Snow has two independent controls: [`set_snow_coverage`] selects one of the
//! game's coverage presets, while [`set_snow_level`] overrides the amount of
//! snow on the ground.

use rrdr_bridge::GameApi;
use rrdr_natives::{clock as n_clock, graphics as n_graphics, misc as n_misc};

/// Hours in a game day.
const HOURS_PER_DAY: i32 = 24;

/// Minutes in an hour.
const MINUTES_PER_HOUR: i32 = 60;

/// Seconds in a minute.
const SECONDS_PER_MINUTE: i32 = 60;

/// Blend percentage at which ScriptHook considers the second weather type to
/// be the current one.
const CURRENT_WEATHER_BLEND_THRESHOLD: f32 = 0.5;

/// First `p1` argument used by the `rdr3_discoveries` weather example.
const WEATHER_TRANSITION_P1: bool = true;

/// Second `p2` argument used by the `rdr3_discoveries` weather example.
const WEATHER_TRANSITION_P2: bool = true;

/// Final `p5` argument used by the `rdr3_discoveries` weather example.
const WEATHER_TRANSITION_P5: bool = false;

/// The in-game date as `(day, month, year)`.
pub fn current_date(api: &mut dyn GameApi) -> (i32, i32, i32) {
    (
        n_clock::GET_CLOCK_DAY_OF_MONTH(api),
        n_clock::GET_CLOCK_MONTH(api),
        n_clock::GET_CLOCK_YEAR(api),
    )
}

/// The in-game time as `(hour, minute, second)`.
pub fn current_time(api: &mut dyn GameApi) -> (i32, i32, i32) {
    (
        n_clock::GET_CLOCK_HOURS(api),
        n_clock::GET_CLOCK_MINUTES(api),
        n_clock::GET_CLOCK_SECONDS(api),
    )
}

/// Set the in-game time.
///
/// The hour must be `0..=23`, the minute and second `0..=59`. Out-of-range
/// values are ignored.
pub fn set_time(api: &mut dyn GameApi, hour: i32, minute: i32, second: i32) {
    let valid = (0..HOURS_PER_DAY).contains(&hour)
        && (0..MINUTES_PER_HOUR).contains(&minute)
        && (0..SECONDS_PER_MINUTE).contains(&second);

    if valid {
        n_clock::SET_CLOCK_TIME(api, hour, minute, second);
    }
}

/// A weather type, identified by its game hash.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Weather(u32);

impl Weather {
    /// Sunny weather (`0x614A1F91`).
    pub const SUNNY: Self = Self(0x614A_1F91);

    /// Overcast weather (`0xBB898D2D`).
    pub const OVERCAST: Self = Self(0xBB89_8D2D);

    /// Clearing weather (`0x6DB1A50D`).
    pub const CLEARING: Self = Self(0x6DB1_A50D);

    /// Rainy weather (`0x54A69840`).
    pub const RAIN: Self = Self(0x54A6_9840);

    /// Foggy weather (`0xD61BDE01`).
    pub const FOG: Self = Self(0xD61B_DE01);

    /// Snowy weather (`0xEFB6EFF6`).
    pub const SNOW: Self = Self(0xEFB6_EFF6);

    /// Blizzard weather (`0x27EA2814`).
    pub const BLIZZARD: Self = Self(0x27EA_2814);

    /// Thunderstorm weather (`0x7C1C4A13`).
    pub const THUNDERSTORM: Self = Self(0x7C1C_4A13);

    /// Wrap a weather hash.
    pub const fn from_hash(hash: u32) -> Self {
        Self(hash)
    }

    /// Hash a weather name such as `"SUNNY"` with `GET_HASH_KEY`.
    pub fn from_name(api: &mut dyn GameApi, name: &str) -> Self {
        Self(n_misc::GET_HASH_KEY(api, name))
    }

    /// Raw weather hash.
    pub const fn hash(self) -> u32 {
        self.0
    }

    /// Whether this is the currently dominant weather.
    ///
    /// Uses the same majority-blend rule as ScriptHook's
    /// `World.CurrentWeather`.
    pub fn is_current(self, api: &mut dyn GameApi) -> bool {
        self == current(api)
    }

    /// Change the weather, blending in over `transition_seconds`.
    ///
    /// A zero duration forces the weather immediately with
    /// `SET_CURR_WEATHER_STATE`, matching ScriptHook's `World.CurrentWeather`
    /// setter. A positive duration uses `SET_WEATHER_TYPE` with the transition
    /// flag and duration set, matching the `rdr3_discoveries` weather example.
    ///
    /// Negative durations are treated as zero; non-finite durations are
    /// ignored.
    pub fn set(self, api: &mut dyn GameApi, transition_seconds: f32) {
        if !transition_seconds.is_finite() {
            return;
        }

        let seconds = transition_seconds.max(0.0);
        if seconds == 0.0 {
            let previous = current(api);
            // percentWeather2 = 1.0, enabled = true: blend fully to `self`.
            n_misc::SET_CURR_WEATHER_STATE(api, previous.hash(), self.hash(), 1.0, true);
        } else {
            set_weather_type(api, self, seconds, true);
        }
    }

    /// Make this the next weather type, blending in over `transition_seconds`.
    ///
    /// RDR2 has no native that stores a pending weather type without starting
    /// the transition, so this uses `SET_WEATHER_TYPE`. While the blend is in
    /// progress the new type is reported by [`next`]. Negative durations are
    /// treated as zero; non-finite durations are ignored.
    pub fn set_next(self, api: &mut dyn GameApi, transition_seconds: f32) {
        if !transition_seconds.is_finite() {
            return;
        }

        let seconds = transition_seconds.max(0.0);
        set_weather_type(api, self, seconds, true);
    }
}

/// The currently dominant weather.
pub fn current(api: &mut dyn GameApi) -> Weather {
    let (first, second, percent) = weather_state(api);
    if percent >= CURRENT_WEATHER_BLEND_THRESHOLD {
        Weather(second)
    } else {
        Weather(first)
    }
}

/// The weather transition target reported by the game.
///
/// This is the second type returned by `GET_CURR_WEATHER_STATE`, matching
/// ScriptHook's `World.NextWeather`.
pub fn next(api: &mut dyn GameApi) -> Weather {
    Weather(weather_state(api).1)
}

/// Snow coverage presets understood by `GRAPHICS::_SET_SNOW_COVERAGE_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SnowCoverage {
    /// Primary snow coverage.
    Primary = 0,
    /// Secondary snow coverage.
    Secondary = 1,
    /// Christmas snow coverage.
    Xmas = 2,
    /// Christmas secondary snow coverage.
    XmasSecondary = 3,
}

impl SnowCoverage {
    /// Raw value understood by the native.
    pub const fn raw(self) -> i32 {
        self as i32
    }
}

/// Set the snow coverage preset.
pub fn set_snow_coverage(api: &mut dyn GameApi, coverage: SnowCoverage) {
    n_graphics::_SET_SNOW_COVERAGE_TYPE(api, coverage.raw());
}

/// Current ground snow level (`GET_SNOW_LEVEL`).
pub fn snow_level(api: &mut dyn GameApi) -> f32 {
    n_misc::GET_SNOW_LEVEL(api)
}

/// Override the ground snow level (`_SET_SNOW_LEVEL`).
///
/// Non-finite values are ignored. The game's expected range is not documented
/// by the native, so the value is passed through unchanged.
pub fn set_snow_level(api: &mut dyn GameApi, level: f32) {
    if level.is_finite() {
        n_misc::_SET_SNOW_LEVEL(api, level);
    }
}

/// `SET_WEATHER_TYPE` wrapper with the fixed flags from Rockstar's example.
fn set_weather_type(
    api: &mut dyn GameApi,
    weather: Weather,
    transition_seconds: f32,
    transition: bool,
) {
    n_misc::SET_WEATHER_TYPE(
        api,
        weather.hash(),
        WEATHER_TRANSITION_P1,
        WEATHER_TRANSITION_P2,
        transition,
        transition_seconds,
        WEATHER_TRANSITION_P5,
    );
}

/// Read the raw two-type weather blend from `GET_CURR_WEATHER_STATE`.
fn weather_state(api: &mut dyn GameApi) -> (u32, u32, f32) {
    let mut first = 0u32;
    let mut second = 0u32;
    let mut percent = 0.0f32;
    n_misc::GET_CURR_WEATHER_STATE(api, &mut first, &mut second, &mut percent);
    (first, second, percent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_hashes_round_trip() {
        assert_eq!(Weather::from_hash(Weather::SUNNY.hash()), Weather::SUNNY);
        assert_eq!(Weather::SUNNY.hash(), 0x614A_1F91);
        assert_ne!(Weather::SUNNY, Weather::RAIN);
    }

    #[test]
    fn snow_coverage_discriminants_match_the_native() {
        assert_eq!(SnowCoverage::Primary.raw(), 0);
        assert_eq!(SnowCoverage::Secondary.raw(), 1);
        assert_eq!(SnowCoverage::Xmas.raw(), 2);
        assert_eq!(SnowCoverage::XmasSecondary.raw(), 3);
    }
}

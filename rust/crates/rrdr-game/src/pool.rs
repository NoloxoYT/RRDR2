//! Pool/buffer helpers for nearby entity enumeration.
//!
//! ScriptHookRDR2DotNet's `World.GetAllPeds` uses a host C++ export, not a
//! native. Until the host grows a global-pool callback, the portable API is
//! proximity enumeration with `GET_PED_NEARBY_PEDS` / `GET_PED_NEARBY_VEHICLES`.

use rrdr_bridge::GameApi;
use rrdr_natives::ped as n_ped;

/// Default capacity for nearby queries.
pub const DEFAULT_CAPACITY: usize = 256;

/// Maximum capacity accepted by this layer.
pub const MAX_CAPACITY: usize = 1024;

/// Options for nearby entity queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolOptions {
    /// Maximum number of handles to request.
    pub capacity: usize,
    /// Include dead peds/vehicles.
    pub include_dead: bool,
}

impl Default for PoolOptions {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CAPACITY,
            include_dead: false,
        }
    }
}

/// Low-level nearby ped handles.
///
/// `GET_PED_NEARBY_PEDS` takes a 64-bit slot array where slot 0 carries the
/// capacity in its low 32 bits and slots `1..=count` hold handles. That layout
/// is the one documented on FiveM's native DB (`pedsToStore: "the first index
/// is the size"`), so it is community-documented rather than guessed; the
/// trailing `-1` (ignored ped type) and `0` arguments and the
/// count-from-return-value assumption are still unverified in-game.
///
/// Before trusting the layout, validate it once with a debug build: request
/// capacity 1, assert the returned count is ≤ 1, and check every handle
/// against `DOES_ENTITY_EXIST` (the wrappers in [`crate::world`] already do
/// this through `try_from_raw`). A native that writes more entries than
/// `capacity + 1` slots would be a game-side heap overrun, so keep the buffer
/// at `capacity + 1` and never request more than [`MAX_CAPACITY`].
pub(crate) fn nearby_ped_handles(api: &mut dyn GameApi, source: i32, capacity: usize) -> Vec<i32> {
    let capacity = capacity.clamp(1, MAX_CAPACITY);
    let mut buffer = vec![0u64; capacity + 1];
    buffer[0] = capacity as u64;

    let count = n_ped::GET_PED_NEARBY_PEDS(api, source, buffer.as_mut_ptr(), -1, 0);
    let count = count.max(0) as usize;
    let count = count.min(capacity);

    buffer
        .iter()
        .skip(1)
        .take(count)
        .map(|slot| *slot as u32 as i32)
        .collect()
}

/// Low-level nearby vehicle handles. Uses the same slot layout as
/// [`nearby_ped_handles`] (same validation caveat applies).
pub(crate) fn nearby_vehicle_handles(
    api: &mut dyn GameApi,
    source: i32,
    capacity: usize,
) -> Vec<i32> {
    let capacity = capacity.clamp(1, MAX_CAPACITY);
    let mut buffer = vec![0u64; capacity + 1];
    buffer[0] = capacity as u64;

    let count = n_ped::GET_PED_NEARBY_VEHICLES(api, source, buffer.as_mut_ptr());
    let count = count.max(0) as usize;
    let count = count.min(capacity);

    buffer
        .iter()
        .skip(1)
        .take(count)
        .map(|slot| *slot as u32 as i32)
        .collect()
}

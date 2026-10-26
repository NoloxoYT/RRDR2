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
/// The native uses an array of 64-bit slots: slot 0 carries the capacity in its
/// low 32 bits, slots 1..=count contain handles. This layout is community
/// documented and must be validated in-game.
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

/// Low-level nearby vehicle handles. Uses the same slot layout.
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

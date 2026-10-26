//! Error types for high-level game operations.

use crate::entity::EntityKind;
use std::fmt;

/// Errors returned by high-level operations that can fail for a reason the
/// caller may want to handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The handle is zero or no longer points to an existing entity.
    InvalidHandle(i32),
    /// The handle points to a different entity type than expected.
    WrongEntityType {
        /// Type the caller asked for.
        expected: EntityKind,
        /// Type actually returned by the game.
        actual: EntityKind,
        /// Raw handle involved.
        handle: i32,
    },
    /// The game is not ready yet (loading, respawn, player ped not spawned).
    NotReady(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandle(handle) => write!(formatter, "invalid entity handle {handle}"),
            Self::WrongEntityType {
                expected,
                actual,
                handle,
            } => write!(
                formatter,
                "handle {handle} is {actual:?}, expected {expected:?}"
            ),
            Self::NotReady(reason) => write!(formatter, "not ready: {reason}"),
        }
    }
}

impl std::error::Error for Error {}

/// Convenient result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

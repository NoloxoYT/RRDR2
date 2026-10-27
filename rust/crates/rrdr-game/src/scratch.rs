//! Padded scratch blocks for natives that write through `Arg::Ptr` out-params.
//!
//! `Arg::Ptr` hands the game a raw Rust address; the native writes into it
//! synchronously before returning. What it writes is community-documented for
//! several natives and unvalidated in-game. Two documented mismatches would
//! corrupt unrelated stack memory:
//!
//! * a 4-byte `BOOL` written through a `*mut bool` (Rust `bool` is 1 byte);
//! * a 24-byte three-slot vector written through a `*mut Vec3` (Rust `Vec3` is
//!   12 bytes).
//!
//! Every block here pads its out field with slack, so an over-writing native
//! eats padding instead of the stack. The padding also documents that these
//! layouts still need in-game validation (issues §4.1): the values read from
//! the typed field are only correct if the native writes them at the Rust
//! offsets, which is what the oversized block lets us verify safely.
//!
//! This module is a private implementation detail of `rrdr-game` and must stay
//! safe Rust (`#![forbid(unsafe_code)]` holds): the padded fields are passed as
//! ordinary references, which the generated natives take as raw pointers.

use rrdr_bridge::Vec3;

/// A `bool` out-param with 31 bytes of slack after it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BoolOut {
    /// The value, read as the 1-byte Rust `bool`.
    pub value: bool,
    /// Slack for a native that writes a 4-byte `BOOL` (or more).
    _pad: [u8; 31],
}

/// A `Vec3` out-param with 20 bytes of slack after it (32-byte total block).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Vec3Out {
    /// The value, read as the 12-byte packed `Vec3`.
    pub value: Vec3,
    /// Slack for a native that writes the 24-byte three-slot vector format.
    _pad: [u8; 20],
}

/// An `i32` out-param with 28 bytes of slack after it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct I32Out {
    /// The value.
    pub value: i32,
    /// Slack.
    _pad: [u8; 28],
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn blocks_leave_room_for_over_writes() {
        assert_eq!(size_of::<BoolOut>(), 32);
        assert_eq!(size_of::<Vec3Out>(), 32);
        assert_eq!(size_of::<I32Out>(), 32);
        assert!(align_of::<Vec3Out>() >= align_of::<Vec3>());
    }

    #[test]
    fn typed_fields_are_zero_initialised() {
        let boolean = BoolOut::default();
        assert!(!boolean.value);

        let vector = Vec3Out::default();
        assert_eq!(vector.value, Vec3::default());
    }
}

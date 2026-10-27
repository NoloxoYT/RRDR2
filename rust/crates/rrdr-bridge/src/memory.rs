//! Safe, validated access to the game's own process memory.
//!
//! This module exists because some game values (shop prices, for instance) are
//! not reachable through a native and can only be read or patched in memory.
//!
//! Every entry point validates the target range with `VirtualQuery` before
//! touching it: null pointers, integer overflow, uncommitted pages, guard pages
//! and pages without the required access are rejected with a [`MemoryError`]
//! instead of crashing the game. Writes temporarily change the page protection
//! and always restore it, including when the copy fails.
//!
//! The API is intentionally narrow: no raw pointers are handed back to callers.
//! Reads copy into caller-provided buffers, writes take a slice, and patching
//! returns a guard that restores the original bytes on drop.
//!
//! Only Windows has an implementation; other targets return
//! [`MemoryError::Unsupported`] so the crate keeps building for tests.
//!
//! # Patching shop prices
//!
//! RDR2 exposes no native to change prices, so a multiplier has to be applied
//! where the game computes them. The shape of the code is:
//!
//! ```ignore
//! use rrdr_bridge::memory::{self, Patch, Pattern};
//!
//! // Find the site once, at init. The pattern has to come from reverse
//! // engineering this build of RDR2.exe; there is no stable public signature.
//! let exe = memory::main_module()?;
//! let site = memory::scan_first(&exe, &Pattern::parse("F3 0F 10 ?? ?? F3 0F 59")?)?
//!     .ok_or("price signature not found")?;
//!
//! // Replace the operand with your multiplier; the guard puts the original
//! // bytes back when it is dropped.
//! let patch = Patch::apply(site + 4, &0.75f32.to_le_bytes())?;
//! // ... keep `patch` alive, or call `patch.restore()` explicitly.
//! ```
//!
//! Two rules make this safe in practice:
//!
//! 1. Keep every [`Patch`] in the mod state and drop it in `Mod::shutdown`, so
//!    the bytes are restored before the DLL is unloaded.
//! 2. Re-scan on every launch and treat a missing match as a hard error; never
//!    hardcode an absolute address, ASLR moves the module every run.

use crate::Vec3;
use std::fmt;

/// Failure modes of a memory operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    /// The target does not have a memory implementation (non-Windows build).
    Unsupported,
    /// The address was zero.
    NullAddress,
    /// `address + length` overflowed.
    Overflow,
    /// The length was zero.
    EmptyRange,
    /// The module is not loaded, or its PE headers are not what we expect.
    InvalidModule,
    /// `VirtualQuery` failed at this address.
    QueryFailed { address: usize, code: u32 },
    /// The page is not committed.
    NotCommitted(usize),
    /// The page is a guard page or has no access rights.
    Inaccessible(usize),
    /// The range is not readable.
    NotReadable(usize),
    /// The range is not writable, even after changing its protection.
    NotWritable(usize),
    /// The range spans more than one memory region, which this API refuses to
    /// patch in one go.
    SpansRegions { address: usize, length: usize },
    /// `VirtualProtect` failed.
    ProtectFailed { address: usize, code: u32 },
    /// The previous page protection could not be restored after a write.
    RestoreFailed { address: usize, code: u32 },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::Unsupported => write!(f, "memory access is not supported on this target"),
            MemoryError::NullAddress => write!(f, "address is null"),
            MemoryError::Overflow => write!(f, "address + length overflows"),
            MemoryError::EmptyRange => write!(f, "length is zero"),
            MemoryError::InvalidModule => write!(f, "module not loaded or PE headers invalid"),
            MemoryError::QueryFailed { address, code } => {
                write!(f, "VirtualQuery failed at {address:#X} (error {code})")
            }
            MemoryError::NotCommitted(address) => {
                write!(f, "no committed memory at {address:#X}")
            }
            MemoryError::Inaccessible(address) => {
                write!(f, "memory at {address:#X} is a guard page or has no access")
            }
            MemoryError::NotReadable(address) => {
                write!(f, "memory at {address:#X} is not readable")
            }
            MemoryError::NotWritable(address) => {
                write!(f, "memory at {address:#X} is not writable")
            }
            MemoryError::SpansRegions { address, length } => write!(
                f,
                "range {address:#X}+{length} spans several memory regions"
            ),
            MemoryError::ProtectFailed { address, code } => {
                write!(f, "VirtualProtect failed at {address:#X} (error {code})")
            }
            MemoryError::RestoreFailed { address, code } => write!(
                f,
                "failed to restore page protection at {address:#X} (error {code})"
            ),
        }
    }
}

impl std::error::Error for MemoryError {}

pub type Result<T> = std::result::Result<T, MemoryError>;

/// A loaded module: its base address and its full mapped size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Module {
    pub base: usize,
    pub size: usize,
}

impl Module {
    pub fn end(&self) -> usize {
        self.base.wrapping_add(self.size)
    }
}

/// Types whose every bit pattern is a valid value, so copying raw bytes into
/// them cannot create an invalid value.
///
/// # Safety
/// Implementors must guarantee that any bit pattern is valid, and that the type
/// has no padding whose contents would be observable.
pub unsafe trait Pod: Copy {}

macro_rules! impl_pod {
    ($($ty:ty),* $(,)?) => {
        $(unsafe impl Pod for $ty {})*
    };
}

impl_pod!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

// `Vec3` is three `f32`, `#[repr(C)]`, so every bit pattern is valid.
unsafe impl Pod for Vec3 {}

/// Resolves a loaded module by file name (for example `"RDR2.exe"`).
///
/// An empty name resolves the main executable.
pub fn module(name: &str) -> Result<Module> {
    imp::module(name)
}

/// Resolves the main executable of the process.
pub fn main_module() -> Result<Module> {
    imp::module("")
}

/// Checks the arguments that are invalid on every platform, so the failure does
/// not depend on the target having a memory implementation.
fn validate_arguments(address: usize, length: usize) -> Result<()> {
    if address == 0 {
        return Err(MemoryError::NullAddress);
    }
    if length == 0 {
        return Err(MemoryError::EmptyRange);
    }
    address.checked_add(length).ok_or(MemoryError::Overflow)?;
    Ok(())
}

/// Copies `out.len()` bytes from `address` into `out`.
pub fn read(address: usize, out: &mut [u8]) -> Result<()> {
    validate_arguments(address, out.len())?;
    imp::read(address, out)
}

/// Copies `bytes` to `address`, temporarily changing the page protection when
/// needed and restoring it afterwards.
pub fn write(address: usize, bytes: &[u8]) -> Result<()> {
    validate_arguments(address, bytes.len())?;
    imp::write(address, bytes)
}

/// Reads a single [`Pod`] value.
pub fn read_value<T: Pod>(address: usize) -> Result<T> {
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let size = std::mem::size_of::<T>();
    // SAFETY: `value` is a live local with room for `T`, and `read` only writes
    // `size` bytes into the buffer we hand it.
    let buffer = unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr() as *mut u8, size) };
    read(address, buffer)?;
    // SAFETY: `Pod` guarantees every bit pattern is a valid `T`.
    Ok(unsafe { value.assume_init() })
}

/// Writes a single [`Pod`] value.
pub fn write_value<T: Pod>(address: usize, value: T) -> Result<()> {
    let size = std::mem::size_of::<T>();
    // SAFETY: `&value` is a live local with `size` readable bytes.
    let bytes = unsafe { std::slice::from_raw_parts(&value as *const T as *const u8, size) };
    write(address, bytes)
}

pub fn read_vec3(address: usize) -> Result<Vec3> {
    read_value::<Vec3>(address)
}

pub fn write_vec3(address: usize, value: Vec3) -> Result<()> {
    write_value(address, value)
}

/// A byte written into the process, restored automatically on drop.
///
/// The guard stores the original bytes, so a panic or an early return still puts
/// the game back the way it was.
#[derive(Debug)]
pub struct Patch {
    address: usize,
    original: Vec<u8>,
    active: bool,
}

impl Patch {
    /// Writes `bytes` at `address` and remembers what was there.
    pub fn apply(address: usize, bytes: &[u8]) -> Result<Patch> {
        if bytes.is_empty() {
            return Err(MemoryError::EmptyRange);
        }

        let mut original = vec![0u8; bytes.len()];
        read(address, &mut original)?;
        write(address, bytes)?;

        Ok(Patch {
            address,
            original,
            active: true,
        })
    }

    /// Restores the original bytes. Idempotent.
    pub fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }

        write(self.address, &self.original)?;
        self.active = false;
        Ok(())
    }

    pub fn address(&self) -> usize {
        self.address
    }

    pub fn len(&self) -> usize {
        self.original.len()
    }

    pub fn is_empty(&self) -> bool {
        self.original.is_empty()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for Patch {
    fn drop(&mut self) {
        // Best effort: the game is going away anyway if this fails.
        let _ = self.restore();
    }
}

/// A byte pattern with wildcards, parsed from `"48 8B ?? E8 ??"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    bytes: Vec<u8>,
    /// `true` where the byte must match, `false` for a wildcard.
    exact: Vec<bool>,
}

/// Why a pattern string could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError {
    pub token: String,
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid pattern token: {:?}", self.token)
    }
}

impl std::error::Error for PatternError {}

impl Pattern {
    /// Parses a space-separated pattern. `?` and `??` are wildcards.
    pub fn parse(text: &str) -> std::result::Result<Pattern, PatternError> {
        let mut bytes = Vec::new();
        let mut exact = Vec::new();

        for token in text.split_whitespace() {
            match token {
                "?" | "??" => {
                    bytes.push(0);
                    exact.push(false);
                }
                _ => {
                    if token.len() != 2 {
                        return Err(PatternError {
                            token: token.to_string(),
                        });
                    }
                    let byte = u8::from_str_radix(token, 16).map_err(|_| PatternError {
                        token: token.to_string(),
                    })?;
                    bytes.push(byte);
                    exact.push(true);
                }
            }
        }

        if bytes.is_empty() {
            return Err(PatternError {
                token: String::new(),
            });
        }

        Ok(Pattern { bytes, exact })
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn matches_at(&self, haystack: &[u8], offset: usize) -> bool {
        for index in 0..self.bytes.len() {
            if self.exact[index] && haystack[offset + index] != self.bytes[index] {
                return false;
            }
        }
        true
    }
}

/// Largest chunk read while scanning, kept small so a scan does not allocate a
/// copy of the whole module.
#[cfg_attr(not(windows), allow(dead_code))]
const SCAN_CHUNK: usize = 1 << 20;

/// Finds every match of `pattern` in `module`, in address order.
///
/// Only committed, readable regions are scanned; gaps and guard pages are
/// skipped instead of failing the whole scan.
pub fn scan(module: &Module, pattern: &Pattern) -> Result<Vec<usize>> {
    imp::scan(module, pattern)
}

/// Finds the first match of `pattern` in `module`.
pub fn scan_first(module: &Module, pattern: &Pattern) -> Result<Option<usize>> {
    Ok(scan(module, pattern)?.into_iter().next())
}

/// Scans a byte buffer, used by the platform implementation and by tests.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn scan_buffer(buffer: &[u8], pattern: &Pattern, mut report: impl FnMut(usize)) {
    if pattern.len() > buffer.len() {
        return;
    }

    for offset in 0..=(buffer.len() - pattern.len()) {
        if pattern.matches_at(buffer, offset) {
            report(offset);
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::CString;

    const MEM_COMMIT: u32 = 0x1000;
    const PAGE_NOACCESS: u32 = 0x01;
    const PAGE_READONLY: u32 = 0x02;
    const PAGE_READWRITE: u32 = 0x04;
    const PAGE_WRITECOPY: u32 = 0x08;
    const PAGE_EXECUTE_READ: u32 = 0x20;
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
    const PAGE_GUARD: u32 = 0x100;

    const READABLE: u32 = PAGE_READONLY
        | PAGE_READWRITE
        | PAGE_WRITECOPY
        | PAGE_EXECUTE_READ
        | PAGE_EXECUTE_READWRITE
        | PAGE_EXECUTE_WRITECOPY;
    const WRITABLE: u32 =
        PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY;

    /// Matches the x64 `MEMORY_BASIC_INFORMATION`. The two `u32` paddings make
    /// the `SIZE_T` land on its 8-byte boundary, which is what Windows expects
    /// for the `PartitionId` variant too.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    struct MemoryBasicInformation {
        base_address: usize,
        allocation_base: usize,
        allocation_protect: u32,
        _padding1: u32,
        region_size: usize,
        state: u32,
        protect: u32,
        _type: u32,
        _padding2: u32,
    }

    const _: () = assert!(std::mem::size_of::<MemoryBasicInformation>() == 48);

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleA(name: *const u8) -> *mut std::ffi::c_void;
        fn VirtualQuery(
            address: *const std::ffi::c_void,
            information: *mut MemoryBasicInformation,
            length: usize,
        ) -> usize;
        fn VirtualProtect(
            address: *mut std::ffi::c_void,
            length: usize,
            protection: u32,
            old_protection: *mut u32,
        ) -> i32;
        fn GetLastError() -> u32;
    }

    fn last_error() -> u32 {
        // SAFETY: no arguments, no memory involved.
        unsafe { GetLastError() }
    }

    fn query(address: usize) -> Result<MemoryBasicInformation> {
        let mut information = MemoryBasicInformation::default();
        // SAFETY: `information` is a live, correctly sized struct.
        let written = unsafe {
            VirtualQuery(
                address as *const std::ffi::c_void,
                &mut information,
                std::mem::size_of::<MemoryBasicInformation>(),
            )
        };

        if written == 0 {
            return Err(MemoryError::QueryFailed {
                address,
                code: last_error(),
            });
        }

        Ok(information)
    }

    /// Validates that `[address, address + length)` is committed and accessible.
    fn validate(address: usize, length: usize, need_write: bool) -> Result<()> {
        if address == 0 {
            return Err(MemoryError::NullAddress);
        }
        if length == 0 {
            return Err(MemoryError::EmptyRange);
        }
        let end = address.checked_add(length).ok_or(MemoryError::Overflow)?;

        let mut cursor = address;
        while cursor < end {
            let information = query(cursor)?;

            if information.state != MEM_COMMIT {
                return Err(MemoryError::NotCommitted(cursor));
            }
            if information.protect & PAGE_GUARD != 0 || information.protect & PAGE_NOACCESS != 0 {
                return Err(MemoryError::Inaccessible(cursor));
            }

            let allowed = if need_write { WRITABLE } else { READABLE };
            if information.protect & allowed == 0 {
                return Err(if need_write {
                    MemoryError::NotWritable(cursor)
                } else {
                    MemoryError::NotReadable(cursor)
                });
            }

            let region_end = information
                .base_address
                .checked_add(information.region_size)
                .ok_or(MemoryError::Overflow)?;

            // A zero-sized or non-advancing region would loop forever.
            if region_end <= cursor {
                return Err(MemoryError::QueryFailed {
                    address: cursor,
                    code: 0,
                });
            }

            cursor = region_end;
        }

        Ok(())
    }

    pub fn read(address: usize, out: &mut [u8]) -> Result<()> {
        validate(address, out.len(), false)?;

        // SAFETY: the range was just validated as committed and readable, and
        // `out` is a live buffer of the same length.
        unsafe {
            std::ptr::copy_nonoverlapping(address as *const u8, out.as_mut_ptr(), out.len());
        }

        Ok(())
    }

    pub fn write(address: usize, bytes: &[u8]) -> Result<()> {
        if address == 0 {
            return Err(MemoryError::NullAddress);
        }
        if bytes.is_empty() {
            return Err(MemoryError::EmptyRange);
        }
        let end = address
            .checked_add(bytes.len())
            .ok_or(MemoryError::Overflow)?;

        // Read validation first: writing into a hole must never be attempted.
        validate(address, bytes.len(), false)?;

        // Patching across two regions is refused: the two may need different
        // protections, and restoring them correctly gets ambiguous.
        let information = query(address)?;
        let region_end = information
            .base_address
            .checked_add(information.region_size)
            .ok_or(MemoryError::Overflow)?;
        if end > region_end {
            return Err(MemoryError::SpansRegions {
                address,
                length: bytes.len(),
            });
        }

        let mut old_protection = 0u32;
        // SAFETY: the range was validated above; `old_protection` is a live u32.
        let changed = unsafe {
            VirtualProtect(
                address as *mut std::ffi::c_void,
                bytes.len(),
                PAGE_EXECUTE_READWRITE,
                &mut old_protection,
            )
        };
        if changed == 0 {
            return Err(MemoryError::ProtectFailed {
                address,
                code: last_error(),
            });
        }

        // SAFETY: the range is now writable for `bytes.len()` bytes and does not
        // alias `bytes`, which lives in Rust-owned memory.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len());
        }

        let mut restored = 0u32;
        // SAFETY: same range, restoring the protection captured above.
        let restore = unsafe {
            VirtualProtect(
                address as *mut std::ffi::c_void,
                bytes.len(),
                old_protection,
                &mut restored,
            )
        };
        if restore == 0 {
            return Err(MemoryError::RestoreFailed {
                address,
                code: last_error(),
            });
        }

        Ok(())
    }

    pub fn module(name: &str) -> Result<Module> {
        let c_name = if name.is_empty() {
            None
        } else {
            Some(CString::new(name).map_err(|_| MemoryError::InvalidModule)?)
        };

        let pointer = match c_name {
            Some(name) => {
                // SAFETY: the CString is NUL-terminated and lives for the call.
                unsafe { GetModuleHandleA(name.as_ptr() as *const u8) }
            }
            // SAFETY: a null name asks for the main executable.
            None => unsafe { GetModuleHandleA(std::ptr::null()) },
        };

        if pointer.is_null() {
            return Err(MemoryError::InvalidModule);
        }

        let base = pointer as usize;
        let size = image_size(base)?;

        Ok(Module { base, size })
    }

    /// Reads `SizeOfImage` out of the PE headers, checking the signatures and
    /// the offsets before using them.
    fn image_size(base: usize) -> Result<usize> {
        let dos_signature = read_value::<u16>(base)?;
        if dos_signature != 0x5A4D {
            // "MZ"
            return Err(MemoryError::InvalidModule);
        }

        let lfanew = read_value::<u32>(base + 0x3C)? as usize;
        // A sane DOS header points inside the first page(s).
        if lfanew == 0 || lfanew > 0x1000 {
            return Err(MemoryError::InvalidModule);
        }

        let nt = base.checked_add(lfanew).ok_or(MemoryError::Overflow)?;
        let nt_signature = read_value::<u32>(nt)?;
        if nt_signature != 0x0000_4550 {
            // "PE\0\0"
            return Err(MemoryError::InvalidModule);
        }

        // PE32+ layout: OptionalHeader starts at nt + 0x18, and SizeOfImage is
        // at offset 0x38 inside it.
        let size_of_image = read_value::<u32>(nt + 0x18 + 0x38)? as usize;
        if size_of_image == 0 {
            return Err(MemoryError::InvalidModule);
        }

        Ok(size_of_image)
    }

    pub fn scan(module: &Module, pattern: &Pattern) -> Result<Vec<usize>> {
        if pattern.is_empty() {
            return Err(MemoryError::EmptyRange);
        }

        let end = module
            .base
            .checked_add(module.size)
            .ok_or(MemoryError::Overflow)?;
        let mut cursor = module.base;
        let mut found = Vec::new();

        // Bytes carried over from the previous chunk so a match straddling two
        // chunks is still found once.
        let mut overlap: Vec<u8> = Vec::new();

        while cursor < end {
            let information = match query(cursor) {
                Ok(information) => information,
                Err(MemoryError::QueryFailed { .. }) => break,
                Err(other) => return Err(other),
            };

            let region_end = information
                .base_address
                .checked_add(information.region_size)
                .ok_or(MemoryError::Overflow)?
                .min(end);

            if region_end <= cursor {
                break;
            }

            let readable = information.state == MEM_COMMIT
                && information.protect & PAGE_GUARD == 0
                && information.protect & PAGE_NOACCESS == 0
                && information.protect & READABLE != 0;

            if readable {
                let mut offset = cursor;
                while offset < region_end {
                    let take = SCAN_CHUNK.min(region_end - offset);
                    let mut buffer = vec![0u8; overlap.len() + take];

                    buffer[..overlap.len()].copy_from_slice(&overlap);
                    read(offset, &mut buffer[overlap.len()..])?;

                    let base_of_buffer = offset - overlap.len();
                    {
                        let mut report = |local: usize| {
                            let address = base_of_buffer + local;
                            if found.last() != Some(&address) {
                                found.push(address);
                            }
                        };
                        scan_buffer(&buffer, pattern, &mut report);
                    }

                    let keep = pattern.len().saturating_sub(1);
                    overlap.clear();
                    if keep > 0 {
                        let start = buffer.len().saturating_sub(keep);
                        overlap.extend_from_slice(&buffer[start..]);
                    }

                    offset += take;
                }
            }

            cursor = region_end;
        }

        Ok(found)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn module(_name: &str) -> Result<Module> {
        Err(MemoryError::Unsupported)
    }

    pub fn read(_address: usize, _out: &mut [u8]) -> Result<()> {
        Err(MemoryError::Unsupported)
    }

    pub fn write(_address: usize, _bytes: &[u8]) -> Result<()> {
        Err(MemoryError::Unsupported)
    }

    pub fn scan(_module: &Module, _pattern: &Pattern) -> Result<Vec<usize>> {
        Err(MemoryError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_parses_bytes_and_wildcards() {
        let pattern = Pattern::parse("48 8B ?? E8 ??").expect("valid pattern");
        assert_eq!(pattern.len(), 5);
        assert_eq!(pattern.bytes, vec![0x48, 0x8B, 0x00, 0xE8, 0x00]);
        assert_eq!(pattern.exact, vec![true, true, false, true, false]);
    }

    #[test]
    fn pattern_rejects_garbage() {
        assert!(Pattern::parse("4").is_err());
        assert!(Pattern::parse("ZZ").is_err());
        assert!(Pattern::parse("").is_err());
        assert!(Pattern::parse("   ").is_err());
    }

    #[test]
    fn scan_buffer_finds_every_match() {
        let buffer = [0x90, 0x48, 0x8B, 0x00, 0x48, 0x8B, 0x01];
        let pattern = Pattern::parse("48 8B").expect("valid");
        let mut offsets = Vec::new();
        scan_buffer(&buffer, &pattern, |offset| offsets.push(offset));
        assert_eq!(offsets, vec![1, 4]);
    }

    #[test]
    fn scan_buffer_respects_wildcards() {
        let buffer = [0x48, 0x8B, 0x11, 0x48, 0x8B, 0x22];
        let pattern = Pattern::parse("48 8B ??").expect("valid");
        let mut offsets = Vec::new();
        scan_buffer(&buffer, &pattern, |offset| offsets.push(offset));
        assert_eq!(offsets, vec![0, 3]);
    }

    #[test]
    fn scan_buffer_ignores_short_buffers() {
        let pattern = Pattern::parse("48 8B 00").expect("valid");
        let mut offsets = Vec::new();
        scan_buffer(&[0x48, 0x8B], &pattern, |offset| offsets.push(offset));
        assert!(offsets.is_empty());
    }

    #[test]
    fn null_and_empty_ranges_are_rejected_before_touching_memory() {
        assert_eq!(read(0, &mut [0u8; 4]), Err(MemoryError::NullAddress));
        assert_eq!(write(0, &[0u8; 4]), Err(MemoryError::NullAddress));
        assert_eq!(read(0x1000, &mut []), Err(MemoryError::EmptyRange));
        assert_eq!(write(0x1000, &[]), Err(MemoryError::EmptyRange));
    }

    #[test]
    fn overflow_is_rejected() {
        let mut buffer = [0u8; 4];
        assert_eq!(
            read(usize::MAX - 1, &mut buffer),
            Err(MemoryError::Overflow)
        );
    }

    #[test]
    fn patch_guard_rejects_empty_bytes() {
        assert!(matches!(
            Patch::apply(0x1000, &[]),
            Err(MemoryError::EmptyRange)
        ));
    }
}

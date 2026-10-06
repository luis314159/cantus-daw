//! Cache padding: a value alone in its own 128 bytes.
//!
//! Two threads that write two values on one cache line make the line ping-pong between their
//! cores ("false sharing"). On x86-64 the line is 64 bytes but Intel's spatial prefetcher
//! fetches lines in **pairs** (since Sandy Bridge), so neighbours within 128 bytes still
//! collide; Apple silicon's lines are 128 bytes. Hence 128 on both (ADR 0031 R11, the same
//! choice as crossbeam's `CachePadded`). Buffers written concurrently by two workers are
//! rounded to [`round_up`] for the same reason.

use std::ops::{Deref, DerefMut};

/// The padding and alignment, bytes.
pub const CACHE_PAD: usize = 128;

/// `T` aligned to, and padded to a multiple of, 128 bytes.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
#[repr(C, align(128))]
pub struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    #[inline]
    pub const fn new(value: T) -> CachePadded<T> {
        CachePadded { value }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for CachePadded<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for CachePadded<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T> From<T> for CachePadded<T> {
    fn from(value: T) -> Self {
        CachePadded::new(value)
    }
}

/// `bytes` rounded up to a whole number of 128-byte pairs of lines.
#[inline]
pub const fn round_up(bytes: usize) -> usize {
    bytes.div_ceil(CACHE_PAD) * CACHE_PAD
}

/// How many `T` fill [`round_up`]`(n · size_of::<T>())` bytes: the length to give a buffer
/// of `n` elements so the next one starts on its own pair of lines.
#[inline]
pub const fn padded_len<T>(n: usize) -> usize {
    let size = std::mem::size_of::<T>();
    if size == 0 {
        return n;
    }
    round_up(n * size) / size
}

const _: () = assert!(std::mem::align_of::<CachePadded<u8>>() == CACHE_PAD);
const _: () = assert!(std::mem::size_of::<CachePadded<u8>>() == CACHE_PAD);
const _: () = assert!(std::mem::size_of::<CachePadded<[u64; 17]>>() == 2 * CACHE_PAD);

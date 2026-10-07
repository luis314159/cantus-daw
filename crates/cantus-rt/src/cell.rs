//! Cells for state the engine's pool shares between threads (ADR 0031 §2.7, R16): an
//! `AtomicRefCell` with **try-only** borrows.
//!
//! The engine keeps each graph node's state, its output and its per-worker scratch behind
//! one of these. The schedule guarantees every borrow succeeds — a node runs after its
//! producers finished and before its consumers start, and the callback touches node state
//! only between blocks — so a failed borrow is a scheduler bug: the caller counts it
//! (`EngineStats::schedule_conflicts`) and treats the node as silent, never panics and never
//! waits (ADR 0005 rule 3). There is deliberately **no** panicking `borrow`/`borrow_mut`:
//! R16's rule ("only `try_*`") holds by construction rather than by a lint.
//!
//! - [`NodeCell`]: exclusive borrows only, like a `Mutex` that is only ever `try_lock`ed.
//!   `Sync` when `T: Send` — enough for units that are `Send` but not `Sync` (a hosted
//!   plug-in's instance).
//! - [`ShareCell`]: many readers or one writer, like `RefCell` across threads. `Sync` when
//!   `T: Send + Sync` (buffers that several consumers read at once).
//!
//! Neither can block: a borrow is one compare-and-swap, its release one store (or one
//! `fetch_sub`). Both are `#[repr(align(128))]`-free on purpose — wrap them in
//! [`crate::CachePadded`] where two threads write neighbours.

use std::cell::UnsafeCell;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A value one thread at a time may hold, borrowed with [`NodeCell::try_borrow_mut`].
pub struct NodeCell<T> {
    busy: AtomicBool,
    value: UnsafeCell<T>,
}

// SAFETY: the value is only reached through `NodeMut`, of which at most one exists at a
// time (the `busy` flag, taken with an Acquire CAS and released with a Release store,
// orders every access of one holder before the next holder's). That is exactly a mutex,
// which is `Sync` for `T: Send`.
unsafe impl<T: Send> Sync for NodeCell<T> {}

impl<T> NodeCell<T> {
    pub const fn new(value: T) -> NodeCell<T> {
        NodeCell {
            busy: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// The value, exclusively, or `None` when someone holds it now. Never blocks.
    #[inline]
    pub fn try_borrow_mut(&self) -> Option<NodeMut<'_, T>> {
        self.busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| NodeMut {
                cell: self,
                _not_sync: PhantomData,
            })
    }

    /// Whether someone holds the value right now (a hint: it may change at once).
    #[inline]
    pub fn is_borrowed(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    /// The value, through a unique reference to the cell: no check needed.
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: Default> Default for NodeCell<T> {
    fn default() -> Self {
        NodeCell::new(T::default())
    }
}

impl<T> fmt::Debug for NodeCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeCell")
            .field("borrowed", &self.is_borrowed())
            .finish_non_exhaustive()
    }
}

/// The exclusive borrow of a [`NodeCell`]; released when dropped.
pub struct NodeMut<'a, T> {
    cell: &'a NodeCell<T>,
    // `&mut T`'s auto traits: `Sync` only with `T: Sync`.
    _not_sync: PhantomData<&'a mut T>,
}

impl<T> Deref for NodeMut<'_, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: this guard is the only holder (module docs), for its whole life.
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> DerefMut for NodeMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: as above; `&mut self` makes the reference unique among this guard's.
        unsafe { &mut *self.cell.value.get() }
    }
}

impl<T> Drop for NodeMut<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.busy.store(false, Ordering::Release);
    }
}

impl<T: fmt::Debug> fmt::Debug for NodeMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// The high bit of a [`ShareCell`]'s state: a writer holds it.
const WRITER: usize = 1 << (usize::BITS - 1);
/// Readers at once, at most (far below the writer bit, so a count can never reach it).
const MAX_READERS: usize = WRITER >> 1;

/// A value many threads may read at once, or one may write, borrowed with
/// [`ShareCell::try_borrow`] and [`ShareCell::try_borrow_mut`].
pub struct ShareCell<T> {
    state: AtomicUsize,
    value: UnsafeCell<T>,
}

// SAFETY: shared borrows hand out `&T` on several threads at once (needs `T: Sync`), and an
// exclusive borrow hands the value to one thread at a time (needs `T: Send`); the state word
// (readers counted with Acquire CAS / Release `fetch_sub`, the writer bit taken with an
// Acquire CAS from 0 and released with a Release store) orders a writer's accesses against
// every reader's — `std::sync::RwLock`'s bounds.
unsafe impl<T: Send + Sync> Sync for ShareCell<T> {}

impl<T> ShareCell<T> {
    pub const fn new(value: T) -> ShareCell<T> {
        ShareCell {
            state: AtomicUsize::new(0),
            value: UnsafeCell::new(value),
        }
    }

    /// Read the value, or `None` while a writer holds it. Never blocks.
    #[inline]
    pub fn try_borrow(&self) -> Option<ShareRef<'_, T>> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s & WRITER != 0 || s >= MAX_READERS {
                return None;
            }
            match self
                .state
                .compare_exchange_weak(s, s + 1, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return Some(ShareRef { cell: self }),
                Err(now) => s = now,
            }
        }
    }

    /// The value, exclusively, or `None` while anyone reads or writes it. Never blocks.
    #[inline]
    pub fn try_borrow_mut(&self) -> Option<ShareMut<'_, T>> {
        self.state
            .compare_exchange(0, WRITER, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| ShareMut {
                cell: self,
                _not_sync: PhantomData,
            })
    }

    /// The value, through a unique reference to the cell: no check needed.
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: Default> Default for ShareCell<T> {
    fn default() -> Self {
        ShareCell::new(T::default())
    }
}

impl<T> fmt::Debug for ShareCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareCell")
            .field("state", &self.state.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// A shared borrow of a [`ShareCell`]; released when dropped.
pub struct ShareRef<'a, T> {
    cell: &'a ShareCell<T>,
}

impl<T> Deref for ShareRef<'_, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: while any `ShareRef` lives the writer bit cannot be set (a writer's CAS
        // needs the state at 0), so nobody holds `&mut T`.
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> Drop for ShareRef<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.state.fetch_sub(1, Ordering::Release);
    }
}

impl<T> Clone for ShareRef<'_, T> {
    fn clone(&self) -> Self {
        // Cannot overflow in practice (MAX_READERS is 2^62); counted all the same.
        self.cell.state.fetch_add(1, Ordering::Relaxed);
        ShareRef { cell: self.cell }
    }
}

impl<T: fmt::Debug> fmt::Debug for ShareRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// The exclusive borrow of a [`ShareCell`]; released when dropped.
pub struct ShareMut<'a, T> {
    cell: &'a ShareCell<T>,
    _not_sync: PhantomData<&'a mut T>,
}

impl<T> Deref for ShareMut<'_, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: the writer bit is ours: no reader and no other writer exists.
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> DerefMut for ShareMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: as above.
        unsafe { &mut *self.cell.value.get() }
    }
}

impl<T> Drop for ShareMut<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.state.store(0, Ordering::Release);
    }
}

impl<T: fmt::Debug> fmt::Debug for ShareMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_node_cell_has_one_holder_at_a_time() {
        let c = NodeCell::new(5);
        let mut a = c.try_borrow_mut().expect("free");
        assert!(c.try_borrow_mut().is_none(), "held");
        *a += 1;
        drop(a);
        assert_eq!(*c.try_borrow_mut().expect("free again"), 6);
        assert!(!c.is_borrowed());
    }

    #[test]
    fn a_share_cell_has_readers_or_one_writer() {
        let c = ShareCell::new(vec![1, 2]);
        let r1 = c.try_borrow().expect("readable");
        let r2 = r1.clone();
        assert!(c.try_borrow_mut().is_none(), "read, so not writable");
        assert_eq!(r1.len() + r2.len(), 4);
        drop((r1, r2));
        let mut w = c.try_borrow_mut().expect("writable");
        assert!(c.try_borrow().is_none(), "written, so not readable");
        assert!(c.try_borrow_mut().is_none());
        w.push(3);
        drop(w);
        assert_eq!(c.try_borrow().expect("readable").len(), 3);
    }

    #[test]
    fn exclusive_borrows_serialise_threads() {
        // Four threads each add 1 000 times under the cell; a failed borrow retries. Any
        // lost update would mean two holders at once.
        let c = Arc::new(NodeCell::new(0u64));
        let s = Arc::new(ShareCell::new(0u64));
        let n = if cfg!(miri) { 50 } else { 1_000 };
        let hs: Vec<_> = (0..4)
            .map(|_| {
                let (c, s) = (c.clone(), s.clone());
                std::thread::spawn(move || {
                    for _ in 0..n {
                        loop {
                            if let Some(mut g) = c.try_borrow_mut() {
                                *g += 1;
                                break;
                            }
                            std::hint::spin_loop();
                        }
                        loop {
                            if let Some(mut g) = s.try_borrow_mut() {
                                *g += 1;
                                break;
                            }
                            if let Some(r) = s.try_borrow() {
                                assert!(*r <= 4 * n);
                            }
                            std::hint::spin_loop();
                        }
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(*c.try_borrow_mut().unwrap(), 4 * n);
        assert_eq!(*s.try_borrow().unwrap(), 4 * n);
    }
}

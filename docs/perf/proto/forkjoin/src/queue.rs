//! Dmitry Vyukov's bounded MPMC queue, carrying `u32` node indices, in safe Rust: every
//! cell is a pair of atomics, so no `unsafe` is needed. Fixed capacity, allocated once;
//! push and pop never allocate, never lock, never make a system call.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering::*};

struct Cell {
    seq: AtomicUsize,
    val: AtomicU32,
}

pub struct Queue {
    cells: Box<[Cell]>,
    mask: usize,
    enq: AtomicUsize,
    deq: AtomicUsize,
}

impl Queue {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two().max(2);
        Queue {
            cells: (0..cap)
                .map(|i| Cell {
                    seq: AtomicUsize::new(i),
                    val: AtomicU32::new(0),
                })
                .collect(),
            mask: cap - 1,
            enq: AtomicUsize::new(0),
            deq: AtomicUsize::new(0),
        }
    }

    pub fn push(&self, v: u32) -> bool {
        let mut pos = self.enq.load(Relaxed);
        loop {
            let cell = &self.cells[pos & self.mask];
            let seq = cell.seq.load(Acquire);
            let dif = seq as isize - pos as isize;
            if dif == 0 {
                match self.enq.compare_exchange_weak(pos, pos + 1, Relaxed, Relaxed) {
                    Ok(_) => {
                        cell.val.store(v, Relaxed);
                        cell.seq.store(pos + 1, Release);
                        return true;
                    }
                    Err(p) => pos = p,
                }
            } else if dif < 0 {
                return false; // full
            } else {
                pos = self.enq.load(Relaxed);
            }
        }
    }

    pub fn pop(&self) -> Option<u32> {
        let mut pos = self.deq.load(Relaxed);
        loop {
            let cell = &self.cells[pos & self.mask];
            let seq = cell.seq.load(Acquire);
            let dif = seq as isize - (pos + 1) as isize;
            if dif == 0 {
                match self.deq.compare_exchange_weak(pos, pos + 1, Relaxed, Relaxed) {
                    Ok(_) => {
                        let v = cell.val.load(Relaxed);
                        cell.seq.store(pos + self.mask + 1, Release);
                        return Some(v);
                    }
                    Err(p) => pos = p,
                }
            } else if dif < 0 {
                return None; // empty
            } else {
                pos = self.deq.load(Relaxed);
            }
        }
    }
}

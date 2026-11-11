//! An effect's memory request in a mailbox of its own (ADR 0112, integration of round 39):
//! the event that asks for it stays small (ADR 0107's 64 B), and the request itself —
//! Aula's response by value, 84 B — waits here for the interface's thread.
//!
//! Each Aula unit owns one mailbox, made with it off the audio thread. The audio thread
//! writes the request in (atomics only: no allocation, no lock) and sends a clone of the
//! mailbox's `Arc` in the event — a count, never a free there, since the unit holds the
//! mailbox until it is dropped off the audio thread. The reader takes the latest request
//! written: a newer one written meanwhile is the one the unit wants now, so it is the one
//! worth building (the unit compares what arrives against what it wants either way).
//!
//! A sequence lock: the writer makes the count odd, writes, makes it even; a reader that
//! saw an odd count or two different counts reads again. One writer per mailbox (the unit's
//! audio thread), readers anywhere.

use crate::fx::aula::AulaNeed;
use cantus_dsp::aula_kernel::{Layout, LEVELS};
use std::sync::atomic::{fence, AtomicU64, Ordering};

/// The request's words: the source, the shape's nine `u32`s in five, the room's levels and
/// its partitions per level.
const WORDS: usize = 1 + 5 + 1 + LEVELS;

/// One unit's latest request.
#[derive(Debug, Default)]
pub struct NeedMailbox {
    seq: AtomicU64,
    words: [AtomicU64; WORDS],
}

fn pack(n: &AulaNeed) -> [u64; WORDS] {
    let mut w = [0u64; WORDS];
    w[0] = n.source;
    for (i, pair) in n.shape.chunks(2).enumerate() {
        let hi = pair.get(1).copied().unwrap_or(0);
        w[1 + i] = u64::from(pair[0]) | (u64::from(hi) << 32);
    }
    w[6] = n.cap.levels as u64;
    for (k, p) in n.cap.parts.iter().enumerate() {
        w[7 + k] = *p as u64;
    }
    w
}

fn unpack(w: &[u64; WORDS]) -> AulaNeed {
    let mut shape = [0u32; 9];
    for (i, s) in shape.iter_mut().enumerate() {
        let word = w[1 + i / 2];
        *s = if i % 2 == 0 {
            word as u32
        } else {
            (word >> 32) as u32
        };
    }
    let mut cap = Layout {
        levels: w[6] as usize,
        ..Layout::default()
    };
    for (k, p) in cap.parts.iter_mut().enumerate() {
        *p = w[7 + k] as usize;
    }
    AulaNeed {
        source: w[0],
        shape,
        cap,
    }
}

impl NeedMailbox {
    /// Put `need` in (the audio thread; allocation-free).
    pub fn write(&self, need: &AulaNeed) {
        let w = pack(need);
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        for (slot, v) in self.words.iter().zip(w) {
            slot.store(v, Ordering::Relaxed);
        }
        self.seq.store(s.wrapping_add(2), Ordering::Release);
    }

    /// The latest request written (any thread). Before any write: the zero request.
    pub fn read(&self) -> AulaNeed {
        loop {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let mut w = [0u64; WORDS];
            for (v, slot) in w.iter_mut().zip(&self.words) {
                *v = slot.load(Ordering::Relaxed);
            }
            fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == s1 {
                return unpack(&w);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn need(source: u64, k: u32) -> AulaNeed {
        let mut shape = [0u32; 9];
        for (i, s) in shape.iter_mut().enumerate() {
            *s = (0.25f32 * (i as f32 + 1.0) * k as f32).to_bits() ^ (i as u32 * 7919);
        }
        AulaNeed {
            source,
            shape,
            cap: Layout::for_taps(70_000 + k as usize),
        }
    }

    #[test]
    fn a_request_comes_out_as_it_went_in() {
        let m = NeedMailbox::default();
        for (s, k) in [(0, 1), (u64::MAX - 3, 2), (1 << 40, 3)] {
            let n = need(s, k);
            m.write(&n);
            assert_eq!(m.read(), n);
        }
    }

    #[test]
    fn a_reader_beside_the_writer_never_sees_half_a_request() {
        let m = std::sync::Arc::new(NeedMailbox::default());
        m.write(&need(0, 0));
        let w = m.clone();
        let writer = std::thread::spawn(move || {
            for k in 1..20_000u32 {
                w.write(&need(u64::from(k), k));
            }
        });
        for _ in 0..20_000 {
            let got = m.read();
            assert_eq!(got, need(got.source, got.source as u32), "torn: {got:?}");
        }
        writer.join().unwrap();
    }
}

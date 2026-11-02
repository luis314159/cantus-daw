//! The master's samples for a display (round 35, ADR 0091 §4): what leaves the engine,
//! copied into a lock-free ring for the stage's light visualiser (its spectrum and level).
//!
//! The audio thread only **copies**: at the end of each block, while a reader has turned the
//! tap on, the block's first two channels go into an `rtrb` ring as interleaved pairs (a
//! mono output is paired with itself). No analysis, no allocation, no lock, no wait: a full
//! ring drops what does not fit and counts it. The interface drains the ring once a frame
//! and analyses there. Off (the default, and whenever nothing shows it), the tap costs one
//! relaxed load a block.

use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Stereo frames the ring holds: about a third of a second at 48 kHz, ample for a reader
/// that drains it every display frame.
pub const TAP_FRAMES: usize = 16_384;

#[derive(Debug, Default)]
struct Shared {
    on: AtomicBool,
    /// Frames a full ring could not take.
    lost: AtomicU64,
}

/// The audio thread's end.
#[derive(Debug, Default)]
pub struct MasterTap {
    ring: Option<Producer<f32>>,
    shared: Arc<Shared>,
}

/// The interface's end.
#[derive(Debug)]
pub struct MasterTapReader {
    ring: Consumer<f32>,
    shared: Arc<Shared>,
}

/// A tap and its reader, the ring allocated here (off the audio thread).
pub fn pair() -> (MasterTap, MasterTapReader) {
    let (tx, rx) = RingBuffer::new(TAP_FRAMES * 2);
    let shared = Arc::new(Shared::default());
    (
        MasterTap {
            ring: Some(tx),
            shared: shared.clone(),
        },
        MasterTapReader { ring: rx, shared },
    )
}

impl MasterTap {
    /// Copy a block's output (`channels` interleaved) while the tap is on. Real-time safe.
    pub fn push(&mut self, output: &[f32], channels: usize) {
        let Some(ring) = self.ring.as_mut() else {
            return;
        };
        if channels == 0 || !self.shared.on.load(Ordering::Relaxed) {
            return;
        }
        let frames = output.len() / channels;
        let room = ring.slots() / 2;
        let n = frames.min(room);
        if n < frames {
            self.shared
                .lost
                .fetch_add((frames - n) as u64, Ordering::Relaxed);
        }
        if n == 0 {
            return;
        }
        let Ok(mut chunk) = ring.write_chunk(n * 2) else {
            return;
        };
        let (a, b) = chunk.as_mut_slices();
        let mut k = 0;
        for f in 0..n {
            let l = output[f * channels];
            let r = if channels > 1 {
                output[f * channels + 1]
            } else {
                l
            };
            for v in [l, r] {
                if k < a.len() {
                    a[k] = v;
                } else {
                    b[k - a.len()] = v;
                }
                k += 1;
            }
        }
        chunk.commit_all();
    }
}

impl crate::processor::Processor {
    /// Give the processor the tap's writing end (made with the handle, off the audio thread).
    pub fn attach_master_tap(&mut self, tap: MasterTap) {
        self.master_tap = tap;
    }
}

impl crate::backend::EngineHandle {
    /// The master's samples for a display (the stage's visualiser), when there is an
    /// engine to give them: turn it on while it is shown, drain it once a frame.
    pub fn master_tap(&mut self) -> Option<&mut MasterTapReader> {
        self.master_tap.as_mut()
    }
}

impl MasterTapReader {
    /// Turn the copying on or off. Turned off, what the ring holds is thrown away, so the
    /// next reader starts from now.
    pub fn set_on(&mut self, on: bool) {
        self.shared.on.store(on, Ordering::Relaxed);
        if !on {
            self.clear();
        }
    }

    pub fn is_on(&self) -> bool {
        self.shared.on.load(Ordering::Relaxed)
    }

    /// Hand every pair the ring holds to `f` (interleaved left, right), oldest first, in
    /// at most two slices. Returns the frames handed over.
    pub fn drain(&mut self, mut f: impl FnMut(&[f32])) -> usize {
        let n = self.ring.slots() & !1;
        if n == 0 {
            return 0;
        }
        let Ok(chunk) = self.ring.read_chunk(n) else {
            return 0;
        };
        let (a, b) = chunk.as_slices();
        if !a.is_empty() {
            f(a);
        }
        if !b.is_empty() {
            f(b);
        }
        chunk.commit_all();
        n / 2
    }

    /// Frames lost to a full ring since the tap was made.
    pub fn lost(&self) -> u64 {
        self.shared.lost.load(Ordering::Relaxed)
    }

    fn clear(&mut self) {
        let n = self.ring.slots();
        if let Ok(c) = self.ring.read_chunk(n) {
            c.commit_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_it_copies_nothing_on_it_copies_pairs() {
        let (mut tap, mut rx) = pair();
        let block: Vec<f32> = (0..8).map(|i| i as f32).collect();
        tap.push(&block, 2);
        assert_eq!(rx.drain(|_| {}), 0, "off: nothing");
        rx.set_on(true);
        tap.push(&block, 2);
        let mut got = Vec::new();
        assert_eq!(rx.drain(|s| got.extend_from_slice(s)), 4);
        assert_eq!(got, block);
        // Mono pairs itself; more channels keep the first two.
        tap.push(&[1.0, 2.0], 1);
        tap.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3);
        got.clear();
        rx.drain(|s| got.extend_from_slice(s));
        assert_eq!(got, vec![1.0, 1.0, 2.0, 2.0, 1.0, 2.0, 4.0, 5.0]);
    }

    #[test]
    fn a_full_ring_drops_and_counts_and_off_clears() {
        let (mut tap, mut rx) = pair();
        rx.set_on(true);
        let block = vec![0.5f32; 1024 * 2];
        for _ in 0..(TAP_FRAMES / 1024 + 3) {
            tap.push(&block, 2);
        }
        assert_eq!(rx.lost(), 3 * 1024);
        rx.set_on(false);
        assert_eq!(rx.drain(|_| {}), 0, "turned off, the ring is emptied");
    }
}

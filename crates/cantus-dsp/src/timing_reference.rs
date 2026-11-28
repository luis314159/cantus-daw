//! The reference the crate's timing tests compare with (docs/RELEASING.md L3): a fixed
//! loop of plain DSP whose cost is the machine's speed and its load, nothing of ours. A
//! test times its own work and this reference in turn, in short windows, the best of many
//! of each — a preempted window only takes longer, and a burst of load falls on both
//! alike — and bounds the ratio. A runner several times slower than the owner's PC, or a
//! machine running the whole suite beside the test, slows both; the ratio stays.
//!
//! The workload: a block of 256 samples of white noise (a xorshift, refilled each block,
//! so nothing decays into denormals) through four biquad low-passes in series (direct form
//! II transposed, fixed coefficients: a 2 kHz Butterworth at 48 kHz). One unit is one
//! block; on the owner's machine (2026-11-27) a unit takes about 4.8 µs (the crate builds
//! optimised in tests too).

use std::time::Instant;

/// The coefficients of a 2 kHz Butterworth low-pass at 48 kHz (b0, b1, b2, a1, a2).
const LP: [f32; 5] = [
    0.014_401_44,
    0.028_802_88,
    0.014_401_44,
    -1.632_993_2,
    0.690_598_9,
];

/// The reference workload and its state.
pub(crate) struct Reference {
    buf: [f32; 256],
    z: [[f32; 2]; 4],
    seed: u32,
}

impl Reference {
    pub(crate) fn new() -> Reference {
        Reference {
            buf: [0.0; 256],
            z: [[0.0; 2]; 4],
            seed: 0x9E37_79B9,
        }
    }

    /// One unit of work: a block of noise through the four sections.
    fn block(&mut self) {
        for v in self.buf.iter_mut() {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 17;
            self.seed ^= self.seed << 5;
            *v = (self.seed >> 8) as f32 * (1.0 / 8_388_608.0) - 1.0;
        }
        let [b0, b1, b2, a1, a2] = LP;
        for z in self.z.iter_mut() {
            for v in self.buf.iter_mut() {
                let x = *v;
                let y = b0 * x + z[0];
                z[0] = b1 * x - a1 * y + z[1];
                z[1] = b2 * x - a2 * y;
                *v = y;
            }
        }
        std::hint::black_box(&mut self.buf);
    }

    /// Seconds `units` blocks of the reference take.
    pub(crate) fn time(&mut self, units: usize) -> f64 {
        let t = Instant::now();
        for _ in 0..units {
            self.block();
        }
        t.elapsed().as_secs_f64()
    }
}

/// Seconds `work` takes, once.
pub(crate) fn secs(work: impl FnOnce()) -> f64 {
    let t = Instant::now();
    work();
    t.elapsed().as_secs_f64()
}

/// `work` and `units` blocks of the reference timed in turn, `rounds` times: the best of
/// each, in seconds (work, reference). Size `units` so a reference window lasts about as
/// long as a window of the work, and both short (a few milliseconds), so that on a loaded
/// machine some windows of each run undisturbed.
pub(crate) fn best_of_interleaved(
    rounds: usize,
    units: usize,
    mut work: impl FnMut() -> f64,
) -> (f64, f64) {
    let mut reference = Reference::new();
    // Warm the reference's code and the clock's frequency up before the first window.
    reference.time(units);
    let (mut w, mut r) = (f64::INFINITY, f64::INFINITY);
    for _ in 0..rounds {
        w = w.min(work());
        r = r.min(reference.time(units));
    }
    (w, r)
}

#[test]
fn the_reference_is_a_steady_measure() {
    // Its result is finite (the filters are stable) and its cost scales with the units:
    // the best of 20 windows of 400 units against the best of 20 of 100, near 4×.
    let _turn = crate::timing_turn();
    let mut r = Reference::new();
    r.time(1_000);
    assert!(r.buf.iter().all(|v| v.is_finite() && v.abs() < 4.0));
    assert!(r.buf.iter().any(|v| *v != 0.0));
    let (four, one) = best_of_interleaved(20, 100, || Reference::new().time(400));
    let ratio = four / one;
    eprintln!(
        "reference: {:.2} µs a unit, 400 units / 100 units = {ratio:.2}",
        one / 100.0 * 1e6
    );
    assert!((2.0..8.0).contains(&ratio), "{ratio}");
}

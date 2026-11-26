//! r42 fiddle (ADR 0164) — the open strings in sympathy (a child of `arcus`; the strings
//! survey's item 5): the instrument's four strings, undamped while no finger and no bow is
//! on them, driven by the bridge — the force every voice puts on it — and ringing into it.
//! A note at an open string's pitch, an octave or a twelfth of it, sets that string ringing,
//! and it rings on after the bow has lifted: the fiddle's ring, the violin's resonance.
//!
//! **The model.** Each string a loop of its open period (a fractional delay, a one-pole
//! loss per trip, its open T60 from the instrument's table), fed by the bridge's force and
//! heard there. A string a voice is on (stopped or bowed open: the voice's own string is
//! that string) is damped and not fed. The coupling is one-way: what the bridge's motion
//! gives back to the bowed strings is far under their own wave and is left out.
//!
//! **The level.** At `sym` 1 a string's steady answer at a partial it shares with the
//! drive is [`COUPLE_DB`] under that partial in the drive; the ring after the drive stops
//! decays at the string's own T60.
//!
//! **Null and rest.** At `sym` 0 nothing is fed and nothing here runs (the bank is never
//! live): the instance's samples are untouched, to the bit. The bank rests (exact zeros)
//! once its strings have been quieter than [`QUIET`] for a whole longest period with nothing
//! fed. Which strings are taken changes only at a note's event, so the samples never depend
//! on where the blocks were cut.

use super::tables::InstrumentSpec;
use crate::math;

/// The strings' steady answer at a shared partial at `sym` 1, dB re the drive's.
pub const COUPLE_DB: f32 = -20.0;
/// The one-pole loss per trip (the bridge's and the air's upper loss on an open string).
const LOSS_A: f32 = 0.15;
/// A taken string's T60, s (a finger or the bow on it).
const TAKEN_T60: f64 = 0.04;
/// Under this a string is quiet (−140 dB re the bridge's force at full scale).
const QUIET: f32 = 1e-7;
/// The longest a string rings in sympathy, s: an open string's own T60 in the tables is its
/// decay on a rigid support (the violin's G 11.5 s), but in sympathy it rings through the
/// bridge into the body, which takes its energy (players hear 1–3 s).
const MAX_T60: f64 = 3.0;
/// The lowest open string the lines hold, Hz (the bass's E1 bent down).
const LOWEST_HZ: f64 = 25.0;

/// The sympathetic strings.
#[derive(Clone, Debug)]
pub struct Sympathy {
    sr: f64,
    line: Vec<f32>,
    /// Each string's part of `line` (the same length each).
    len: usize,
    w: usize,
    /// The read delay (samples), the loop gain while free and while taken, the drive's
    /// scale (free), the one-pole's state.
    d: [f32; 4],
    g_free: [f32; 4],
    g_taken: [f32; 4],
    k: [f32; 4],
    lp: [f32; 4],
    /// The strings a voice is on (a mask of four).
    taken: u8,
    /// Fed: `sym` above 0.
    fed: bool,
    live: bool,
    quiet: usize,
    longest: usize,
}

impl Sympathy {
    /// Allocates: never on the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        let len = (sample_rate as f64 / LOWEST_HZ).ceil() as usize + 4;
        Sympathy {
            sr: sample_rate as f64,
            line: vec![0.0; 4 * len],
            len,
            w: 0,
            d: [100.0; 4],
            g_free: [0.0; 4],
            g_taken: [0.0; 4],
            k: [0.0; 4],
            lp: [0.0; 4],
            taken: 0,
            fed: false,
            live: false,
            quiet: 0,
            longest: len,
        }
    }

    /// The strings' tuning and coupling: `spec`'s open strings at `offsets` semitones from
    /// their keys (the track's tuning, a cross-tuning), `sym` the coupling (0 off), `decay`
    /// × the strings' T60 (`ring`: a steel set's longer ring, 1 otherwise).
    pub fn set(&mut self, spec: &InstrumentSpec, offsets: [f32; 4], sym: f32, decay: f64) {
        self.fed = sym > 0.0;
        let mut longest = 0usize;
        for i in 0..4 {
            let st = spec.strings[i];
            let f = 440.0 * math::exp2((st.open as f64 + offsets[i] as f64 - 69.0) / 12.0);
            let f = f.max(LOWEST_HZ * 1.02);
            let p = self.sr / f;
            let w0 = std::f64::consts::TAU * f / self.sr;
            let a = LOSS_A as f64;
            let (sn, cs) = math::sin_cos(w0);
            let lag = math::atan2(a * sn, 1.0 - a * cs) / w0;
            self.d[i] = (p - lag).clamp(2.0, (self.len - 2) as f64) as f32;
            let t60 = (st.t60_s.min(MAX_T60) * decay).max(0.05);
            let g = math::powf(10.0, -3.0 / (f * t60));
            self.g_free[i] = g as f32;
            self.g_taken[i] = math::powf(10.0, -3.0 / (f * TAKEN_T60)) as f32;
            self.k[i] = sym.clamp(0.0, 2.0) * math::db_to_gain(COUPLE_DB) * (1.0 - g as f32);
            longest = longest.max(p.ceil() as usize + 2);
        }
        self.longest = longest;
    }

    /// The strings voices are on now (a mask of four): those are damped and not fed.
    pub fn set_taken(&mut self, mask: u8) {
        self.taken = mask;
    }

    pub fn is_still(&self) -> bool {
        !self.live
    }

    pub fn reset(&mut self) {
        if self.live {
            self.line.fill(0.0);
        }
        self.lp = [0.0; 4];
        self.w = 0;
        self.live = false;
        self.quiet = 0;
    }

    /// The bridge's force in `l` (the voices' sum, the same in `r`): the strings driven by
    /// it and their answer **added** into both.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        if !self.live {
            if !self.fed || l[..n].iter().all(|x| *x == 0.0) {
                return;
            }
            self.live = true;
            self.quiet = 0;
        }
        let len = self.len;
        for i in 0..n {
            let x = l[i];
            let mut out = 0.0f32;
            let mut loud = 0.0f32;
            for s in 0..4 {
                let base = s * len;
                let d = self.d[s];
                let di = d as usize;
                let fr = d - di as f32;
                let j0 = (self.w + len - di) % len;
                let j1 = (j0 + len - 1) % len;
                let y = self.line[base + j0] + fr * (self.line[base + j1] - self.line[base + j0]);
                let lp = (1.0 - LOSS_A) * y + LOSS_A * self.lp[s];
                self.lp[s] = lp;
                let taken = self.taken & (1 << s) != 0;
                let (g, k) = if taken {
                    (self.g_taken[s], 0.0)
                } else {
                    (self.g_free[s], if self.fed { self.k[s] } else { 0.0 })
                };
                self.line[base + self.w] = g * lp + k * x;
                out += lp;
                loud = loud.max(lp.abs());
            }
            self.w = if self.w + 1 == len { 0 } else { self.w + 1 };
            l[i] += out;
            r[i] += out;
            if x == 0.0 && loud < QUIET {
                self.quiet += 1;
            } else {
                self.quiet = 0;
            }
            if self.quiet > self.longest {
                self.reset();
                if l[i + 1..n].iter().all(|x| *x == 0.0) {
                    return;
                }
                self.live = true; // fed again within this slice: on from rest, exactly
            }
        }
    }
}

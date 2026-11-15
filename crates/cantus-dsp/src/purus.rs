//! **Purus**, the real-time repair unit (fx side round, wave E; DSP phase; the name is
//! provisional — [`NAME`]). An insert, not the offline audio editor (ADR 0096): it cleans
//! a track as it plays.
//!
//! ```text
//!  in ─┬─ de-hum (IIR, 0 latency) ─ mix · out ─┬─ de-reverb (STFT, N − 1) ─ mix · out ─ out level ─ out
//!      │                                       └──── its input, delayed ───┘            │
//!      └──────────────── the input, delayed as the chain is ───────────── delta ────────┘
//! ```
//!
//! - **De-hum** ([`hum`]): mains hum at 50/60 Hz, or followed, or found, with its harmonics
//!   up to the count set; a notch per harmonic (depth, width), or the gentle mode, which
//!   subtracts the hum it has learned and holds the learning under a bass note.
//! - **De-reverb** ([`verb`]): the late tail predicted from a statistical room model with
//!   a decay set or estimated, the early part kept, taken out by a smoothed spectral gain.
//! - **De-click / de-crackle**: not in this phase (see `docs/round-fx/purus.md`).
//!
//! Each module has its **mix** (0 returns its input bit for bit), its **output** level and
//! its **delta** (listen to what it takes out). With every module that is on listening to
//! its delta, the output is exactly *the input, delayed as the chain is, minus what the
//! chain would have sent* — the bits of `x(t − L) − y(t)`. With some, the sum of what those
//! modules take out, each aligned to the output.
//!
//! **Latency**: [`latency_for`] — 0 while the de-reverb is off (the de-hum is IIR), `N − 1`
//! (1 023 samples at 48 kHz, 21.3 ms) while it is on, everything else delayed to match.
//! **Null**: with every module off (the defaults), the output is the input, bit for bit
//! and undelayed; a module at mix 0 hands its input on bit for bit (the de-reverb's
//! delayed by its latency).
//!
//! **Rest** (ADR 0031): once the input has been +0.0 for long enough that the de-reverb's
//! frames hold only silence and every de-hum state is under 10⁻¹⁵, the unit **drains** —
//! every state back to a new unit's, the glides landed — and is at rest: silence in is
//! silence out with nothing advancing, so [`Purus::skip`] has nothing to count. The decay
//! the de-reverb estimated survives the drain and [`Purus::reset`] (what was learned of
//! the room); [`Purus::forget`] clears it.
//!
//! Every per-sample step is on the sample count's grid, never the block's: the output does
//! not depend on the block size. Deterministic maths (`crate::math`). Nothing allocates
//! after [`Purus::new`].

#[path = "purus_hum.rs"]
pub mod hum;
#[path = "purus_testkit.rs"]
pub mod testkit;
#[path = "purus_verb.rs"]
pub mod verb;

use crate::math;
pub use hum::{Dehum, HumMode, HumSettings};
pub use verb::{DecayMode, Dereverb, VerbSettings};

/// The unit's name (provisional).
pub const NAME: &str = "Purus";

/// Bands of the reduction readout, log-spaced from 20 Hz to 20 kHz.
pub const BANDS: usize = 48;

/// Purus's settings. **`Default` is the null**: every module off, the output at 0 dB.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PurusSettings {
    pub hum: HumSettings,
    pub verb: VerbSettings,
    /// The unit's output level, dB (−24…+24).
    pub out_db: f32,
}

/// The unit's latency for a set of settings at a sample rate, samples.
pub fn latency_for(s: &PurusSettings, sample_rate: f32) -> usize {
    if s.verb.on {
        verb::latency_for(sample_rate)
    } else {
        0
    }
}

/// What the display draws, read once a block.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PurusMeter {
    /// The hum's fundamental now and the mains frequency it centres on, Hz.
    pub hum_hz: f32,
    pub hum_nominal_hz: f32,
    /// Each harmonic's level in the input (dBFS, its 8 Hz band) and its depth now (dB, ≤ 0).
    pub hum_level_db: [f32; hum::MAX_HARMONICS],
    pub hum_depth_db: [f32; hum::MAX_HARMONICS],
    /// Gentle: the harmonics whose learning a note is holding (either channel).
    pub hum_held: [bool; hum::MAX_HARMONICS],
    /// The decay time in force and the one estimated so far (0 before the first), s.
    pub decay_s: f32,
    pub decay_estimated_s: f32,
    /// The de-reverb's reduction per band, dB (≤ 0), the last frame's.
    pub verb_reduction_db: [f32; BANDS],
    /// The last block's input and output peaks and the level of what was taken out (RMS),
    /// dBFS (−160 for silence).
    pub in_peak_db: f32,
    pub out_peak_db: f32,
    pub removed_db: f32,
}

/// A parameter's glide (10 ms, landing exactly).
#[derive(Copy, Clone, Debug)]
struct Glide {
    v: f32,
    t: f32,
}

impl Glide {
    fn new(v: f32) -> Self {
        Glide { v, t: v }
    }
    #[inline]
    fn next(&mut self, k: f32) -> f32 {
        if self.v != self.t {
            self.v += (self.t - self.v) * k;
            if (self.v - self.t).abs() < 1e-6 {
                self.v = self.t;
            }
        }
        self.v
    }
    fn land(&mut self) {
        self.v = self.t;
    }
    fn landed(&self) -> bool {
        self.v == self.t
    }
}

/// `x + m·(y − x)`, exactly `x` at 0 and exactly `y` at 1.
#[inline]
fn blend(x: f32, y: f32, m: f32) -> f32 {
    if m == 0.0 {
        x
    } else if m == 1.0 {
        y
    } else {
        x + m * (y - x)
    }
}

#[inline]
fn scaled(x: f32, g: f32) -> f32 {
    if g == 1.0 {
        x
    } else {
        x * g
    }
}

/// Zero-run samples between the drain's looks.
const DRAIN_EVERY: usize = 256;
/// What the de-hum's states must be under before the drain empties them.
const DRAIN_BELOW: f64 = 1e-15;

/// The unit.
#[derive(Clone, Debug)]
pub struct Purus {
    fs: f32,
    s: PurusSettings,
    hum: Dehum,
    verb: Dereverb,
    /// Whether the de-hum runs (on, or gliding out).
    hum_live: bool,
    hum_mix: Glide,
    hum_out: Glide,
    verb_mix: Glide,
    verb_out: Glide,
    out: Glide,
    k: f32,
    /// The input delayed as the chain is (for the delta), indexed by the sample count.
    xring: [Vec<f32>; 2],
    xpos: usize,
    rest: bool,
    zero_run: usize,
    /// The last block's peaks and what was taken out.
    in_peak: f32,
    out_peak: f32,
    removed_sq: f64,
    removed_n: usize,
}

impl Purus {
    /// A new unit at `sample_rate`, with the null settings. Allocates here only.
    pub fn new(sample_rate: f32) -> Self {
        let verb = Dereverb::new(sample_rate);
        let n = verb.size();
        Purus {
            fs: sample_rate,
            s: PurusSettings::default(),
            hum: Dehum::new(sample_rate),
            verb,
            hum_live: false,
            hum_mix: Glide::new(0.0),
            hum_out: Glide::new(1.0),
            verb_mix: Glide::new(1.0),
            verb_out: Glide::new(1.0),
            out: Glide::new(1.0),
            k: 1.0 - math::exp(-1.0 / (0.01 * sample_rate)),
            xring: [vec![0.0; n], vec![0.0; n]],
            xpos: 0,
            rest: true,
            zero_run: 0,
            in_peak: 0.0,
            out_peak: 0.0,
            removed_sq: 0.0,
            removed_n: 0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.fs
    }

    pub fn settings(&self) -> &PurusSettings {
        &self.s
    }

    /// The modules, for their readouts (the hum's response, the de-reverb's gains).
    pub fn dehum(&self) -> &Dehum {
        &self.hum
    }

    pub fn dereverb(&self) -> &Dereverb {
        &self.verb
    }

    /// Measurement only (see [`Dereverb::set_shadow`]).
    #[doc(hidden)]
    pub fn dereverb_mut(&mut self) -> &mut Dereverb {
        &mut self.verb
    }

    /// New settings, between blocks. Levels and mixes glide (10 ms); the de-reverb switched
    /// on starts empty (and the latency becomes its own).
    pub fn set(&mut self, s: &PurusSettings) {
        let hum = s.hum.sane();
        let verb = s.verb.sane();
        let out_db = if s.out_db.is_finite() {
            s.out_db.clamp(-24.0, 24.0)
        } else {
            0.0
        };
        let was = self.s;
        self.s = PurusSettings { hum, verb, out_db };
        self.hum.set(&hum);
        if hum.on && !self.hum_live {
            self.hum.reset();
            self.hum_live = true;
        } else if self.rest {
            self.hum.land();
        }
        if verb.on && !was.verb.on {
            self.verb.reset();
            self.xring[0].fill(0.0);
            self.xring[1].fill(0.0);
        }
        self.verb.set(&verb);
        self.hum_mix.t = if hum.on { hum.mix } else { 0.0 };
        self.hum_out.t = gain(hum.out_db);
        self.verb_mix.t = verb.mix;
        self.verb_out.t = gain(verb.out_db);
        self.out.t = gain(out_db);
        if self.rest {
            self.land();
        }
    }

    fn land(&mut self) {
        for g in [
            &mut self.hum_mix,
            &mut self.hum_out,
            &mut self.verb_mix,
            &mut self.verb_out,
            &mut self.out,
        ] {
            g.land();
        }
        if !self.s.hum.on {
            self.hum_live = false;
        }
    }

    /// Samples of delay the unit adds now.
    pub fn latency(&self) -> usize {
        latency_for(&self.s, self.fs)
    }

    /// Whether silence in gives silence out with nothing advancing (ADR 0031).
    pub fn at_rest(&self) -> bool {
        self.rest
    }

    /// Why the unit is not at rest (for the engine's diagnostics).
    pub fn rest_blocker(&self) -> Option<&'static str> {
        (!self.rest).then_some("Purus: a tail or a filter still sounds")
    }

    /// The engine skipped `frames` samples of silence while the unit was at rest: nothing
    /// counts. (Not at rest, the silence is processed.)
    pub fn skip(&mut self, frames: usize) {
        for _ in 0..frames {
            if self.rest {
                return;
            }
            let _ = self.sample(0.0, 0.0);
        }
    }

    /// Every state to a new unit's with the settings in force, keeping the decay the
    /// de-reverb estimated.
    pub fn reset(&mut self) {
        self.hum.reset();
        self.verb.reset();
        self.xring[0].fill(0.0);
        self.xring[1].fill(0.0);
        self.xpos = 0;
        self.land();
        self.rest = true;
        self.zero_run = 0;
    }

    /// [`Purus::reset`], and the decay estimated forgotten too (a new unit's state).
    pub fn forget(&mut self) {
        self.reset();
        self.verb.forget();
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        self.in_peak = 0.0;
        self.out_peak = 0.0;
        self.removed_sq = 0.0;
        self.removed_n = 0;
        for i in 0..n {
            let (l, r) = (left[i], right[i]);
            self.in_peak = self.in_peak.max(l.abs()).max(r.abs());
            let (yl, yr) = self.sample(l, r);
            self.out_peak = self.out_peak.max(yl.abs()).max(yr.abs());
            left[i] = yl;
            right[i] = yr;
        }
    }

    /// One stereo sample.
    #[inline]
    fn sample(&mut self, l: f32, r: f32) -> (f32, f32) {
        let silent = l.to_bits() == 0 && r.to_bits() == 0;
        if self.rest {
            if silent {
                return (0.0, 0.0);
            }
            self.rest = false;
            self.zero_run = 0;
        }
        let k = self.k;
        // De-hum.
        let (mut hl, mut hr) = (l, r);
        if self.hum_live {
            let (cl, cr) = self.hum.process(l, r);
            let m = self.hum_mix.next(k);
            let g = self.hum_out.next(k);
            hl = scaled(blend(l, cl, m), g);
            hr = scaled(blend(r, cr, m), g);
            if !self.s.hum.on && self.hum_mix.landed() {
                self.hum_live = false;
            }
        }
        // De-reverb, and the input delayed to match.
        let (xd, hd, (vl, vr)) = if self.s.verb.on {
            let ((wl, wr), (dl, dr)) = self.verb.process(hl, hr);
            let m = self.verb_mix.next(k);
            let g = self.verb_out.next(k);
            let mask = self.xring[0].len() - 1;
            self.xring[0][self.xpos] = l;
            self.xring[1][self.xpos] = r;
            let rp = (self.xpos + 1) & mask;
            let xd = (self.xring[0][rp], self.xring[1][rp]);
            self.xpos = rp;
            let v = (scaled(blend(dl, wl, m), g), scaled(blend(dr, wr, m), g));
            (xd, (dl, dr), v)
        } else {
            ((l, r), (hl, hr), (hl, hr))
        };
        let g = self.out.next(k);
        let hum_d = self.s.hum.on && self.s.hum.delta;
        let verb_d = self.s.verb.on && self.s.verb.delta;
        let all = (hum_d || !self.s.hum.on) && (verb_d || !self.s.verb.on);
        let (yl, yr) = if !hum_d && !verb_d {
            (vl, vr)
        } else if all {
            (xd.0 - vl, xd.1 - vr)
        } else if hum_d {
            (xd.0 - hd.0, xd.1 - hd.1)
        } else {
            (hd.0 - vl, hd.1 - vr)
        };
        let (dl, dr) = (xd.0 - vl, xd.1 - vr);
        self.removed_sq += (dl as f64) * (dl as f64) + (dr as f64) * (dr as f64);
        self.removed_n += 2;
        let out = (scaled(yl, g), scaled(yr, g));
        // The drain.
        if silent {
            self.zero_run += 1;
            if self.zero_run % DRAIN_EVERY == 0 {
                self.try_drain();
            }
        } else {
            self.zero_run = 0;
        }
        out
    }

    fn try_drain(&mut self) {
        let verb_clear = !self.s.verb.on || self.verb.drained();
        let hum_clear = !self.hum_live || self.hum.quiet_below(DRAIN_BELOW);
        if verb_clear && hum_clear {
            self.reset();
        }
    }

    /// The display's readout.
    pub fn meter(&self) -> PurusMeter {
        let db = |v: f64| {
            if v > 1e-8 {
                (20.0 * math::log10(v)) as f32
            } else {
                -160.0
            }
        };
        let mut m = PurusMeter {
            hum_hz: self.hum.f0() as f32,
            hum_nominal_hz: self.hum.nominal() as f32,
            hum_level_db: [-160.0; hum::MAX_HARMONICS],
            hum_depth_db: [0.0; hum::MAX_HARMONICS],
            hum_held: [false; hum::MAX_HARMONICS],
            decay_s: self.verb.decay_s(),
            decay_estimated_s: self.verb.estimated_decay_s(),
            verb_reduction_db: [0.0; BANDS],
            in_peak_db: db(self.in_peak as f64),
            out_peak_db: db(self.out_peak as f64),
            removed_db: if self.removed_n > 0 {
                db((self.removed_sq / self.removed_n as f64).sqrt())
            } else {
                -160.0
            },
        };
        for k in 0..hum::MAX_HARMONICS {
            m.hum_level_db[k] = db(self.hum.level(k));
            m.hum_depth_db[k] = if self.hum_live {
                self.hum.depth_now_db(k) as f32
            } else {
                0.0
            };
            m.hum_held[k] = self.hum_live && (self.hum.held(0, k) || self.hum.held(1, k));
        }
        if self.s.verb.on {
            let gains = self.verb.gains();
            let bin_hz = self.fs / self.verb.size() as f32;
            for (band, out) in m.verb_reduction_db.iter_mut().enumerate() {
                let lo = band_edge(band);
                let hi = band_edge(band + 1);
                let b0 = ((lo / bin_hz).round() as usize).min(gains.len() - 1);
                let b1 = ((hi / bin_hz).round() as usize).clamp(b0 + 1, gains.len());
                let p: f32 = gains[b0..b1].iter().map(|g| g * g).sum::<f32>() / (b1 - b0) as f32;
                *out = (10.0 * math::log10(p.max(1e-16))).min(0.0);
            }
        }
        m
    }
}

/// The lower edge of reduction band `b`, Hz (20 Hz × 1000^(b/BANDS)).
pub fn band_edge(b: usize) -> f32 {
    20.0 * math::powf(1000.0f32, b as f32 / BANDS as f32)
}

fn gain(db: f32) -> f32 {
    if db == 0.0 {
        1.0
    } else {
        math::db_to_gain(db)
    }
}

#[cfg(test)]
#[path = "purus_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "purus_tests_measure.rs"]
mod tests_measure;
#[cfg(test)]
#[path = "purus_tests_shared.rs"]
mod tests_shared;
#[cfg(test)]
#[path = "purus_tests_verb.rs"]
mod tests_verb;

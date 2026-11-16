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

#[path = "purus_click.rs"]
pub mod click;
#[path = "purus_hum.rs"]
pub mod hum;
#[path = "purus_testkit.rs"]
pub mod testkit;
#[path = "purus_verb.rs"]
pub mod verb;

use crate::math;
pub use click::{ClickSettings, Declick};
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
    pub click: ClickSettings,
    pub verb: VerbSettings,
    /// The unit's output level, dB (−24…+24).
    pub out_db: f32,
}

/// The unit's latency for a set of settings at a sample rate, samples.
pub fn latency_for(s: &PurusSettings, sample_rate: f32) -> usize {
    let (h, c, v) = latencies(s, sample_rate);
    h + c + v
}

/// Each module's latency: the de-hum's (its gentle mode's look-ahead), the de-click's, the
/// de-reverb's.
fn latencies(s: &PurusSettings, sample_rate: f32) -> (usize, usize, usize) {
    let h = if s.hum.on && s.hum.gentle {
        hum::ahead_for(sample_rate)
    } else {
        0
    };
    let c = if s.click.on { click::LATENCY } else { 0 };
    let v = if s.verb.on {
        verb::latency_for(sample_rate)
    } else {
        0
    };
    (h, c, v)
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
    /// Clicks rebuilt in the last block judged (both channels) and since the reset.
    pub clicks_last: u32,
    pub clicks_total: u64,
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
    click: Declick,
    verb: Dereverb,
    /// Whether the de-hum runs (on, or gliding out).
    hum_live: bool,
    hum_mix: Glide,
    hum_out: Glide,
    click_mix: Glide,
    click_out: Glide,
    verb_mix: Glide,
    verb_out: Glide,
    out: Glide,
    k: f32,
    /// The modules' latencies in force (de-hum, de-click, de-reverb).
    lat: (usize, usize, usize),
    /// The input, the de-hum's output and the de-click's, kept to be read delayed as the
    /// chain is (the deltas), indexed by the sample count.
    xr: [Vec<f32>; 2],
    hr: [Vec<f32>; 2],
    cr: [Vec<f32>; 2],
    pos: usize,
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
        let most = hum::ahead_for(sample_rate) + click::LATENCY + verb.latency();
        let n = (most + 1).next_power_of_two();
        Purus {
            fs: sample_rate,
            s: PurusSettings::default(),
            hum: Dehum::new(sample_rate),
            click: Declick::new(),
            verb,
            hum_live: false,
            hum_mix: Glide::new(0.0),
            hum_out: Glide::new(1.0),
            click_mix: Glide::new(1.0),
            click_out: Glide::new(1.0),
            verb_mix: Glide::new(1.0),
            verb_out: Glide::new(1.0),
            out: Glide::new(1.0),
            k: 1.0 - math::exp(-1.0 / (0.01 * sample_rate)),
            lat: (0, 0, 0),
            xr: [vec![0.0; n], vec![0.0; n]],
            hr: [vec![0.0; n], vec![0.0; n]],
            cr: [vec![0.0; n], vec![0.0; n]],
            pos: 0,
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

    pub fn declick(&self) -> &Declick {
        &self.click
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
        let click = s.click.sane();
        let verb = s.verb.sane();
        let out_db = if s.out_db.is_finite() {
            s.out_db.clamp(-24.0, 24.0)
        } else {
            0.0
        };
        let was = self.s;
        self.s = PurusSettings {
            hum,
            click,
            verb,
            out_db,
        };
        self.hum.set(&hum);
        // The gentle mode's look-ahead goes with it at once (no glide out of a delay).
        if !hum.on && (hum.gentle || was.hum.gentle) {
            self.hum_live = false;
        }
        if hum.on && !self.hum_live {
            self.hum.reset();
            self.hum_live = true;
        } else if self.rest {
            self.hum.land();
        }
        if click.on && !was.click.on {
            self.click.reset();
        }
        self.click.set(&click);
        if verb.on && !was.verb.on {
            self.verb.reset();
        }
        self.verb.set(&verb);
        self.lat = latencies(&self.s, self.fs);
        self.click_mix.t = click.mix;
        self.click_out.t = gain(click.out_db);
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
            &mut self.click_mix,
            &mut self.click_out,
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
        self.click.reset();
        self.verb.reset();
        for ring in [&mut self.xr, &mut self.hr, &mut self.cr] {
            ring[0].fill(0.0);
            ring[1].fill(0.0);
        }
        self.pos = 0;
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
        let mut h = (l, r);
        if self.hum_live {
            let ((dl, dr), (cl, cr)) = self.hum.process(l, r);
            let m = self.hum_mix.next(k);
            let g = self.hum_out.next(k);
            h = (scaled(blend(dl, cl, m), g), scaled(blend(dr, cr, m), g));
            if !self.s.hum.on && self.hum_mix.landed() {
                self.hum_live = false;
            }
        }
        // De-click.
        let mut c = h;
        if self.s.click.on {
            let ((wl, wr), (dl, dr)) = self.click.process(h.0, h.1);
            let m = self.click_mix.next(k);
            let g = self.click_out.next(k);
            c = (scaled(blend(dl, wl, m), g), scaled(blend(dr, wr, m), g));
        }
        // De-reverb.
        let mut v = c;
        if self.s.verb.on {
            let ((wl, wr), (dl, dr)) = self.verb.process(c.0, c.1);
            let m = self.verb_mix.next(k);
            let g = self.verb_out.next(k);
            v = (scaled(blend(dl, wl, m), g), scaled(blend(dr, wr, m), g));
        }
        // The input and each module's output, delayed to meet the output.
        let (lh, lc, lv) = self.lat;
        let mask = self.xr[0].len() - 1;
        let p = self.pos;
        self.xr[0][p] = l;
        self.xr[1][p] = r;
        self.hr[0][p] = h.0;
        self.hr[1][p] = h.1;
        self.cr[0][p] = c.0;
        self.cr[1][p] = c.1;
        let read = |ring: &[Vec<f32>; 2], d: usize| {
            let i = p.wrapping_sub(d) & mask;
            (ring[0][i], ring[1][i])
        };
        let xd = read(&self.xr, lh + lc + lv);
        let hd = read(&self.hr, lc + lv);
        let cd = read(&self.cr, lv);
        self.pos = (p + 1) & mask;
        let g = self.out.next(k);
        let s = &self.s;
        let hum_d = s.hum.on && s.hum.delta;
        let click_d = s.click.on && s.click.delta;
        let verb_d = s.verb.on && s.verb.delta;
        let all = (hum_d || !s.hum.on) && (click_d || !s.click.on) && (verb_d || !s.verb.on);
        let (yl, yr) = if !hum_d && !click_d && !verb_d {
            v
        } else if all {
            (xd.0 - v.0, xd.1 - v.1)
        } else {
            // The sum of what the modules listened to take out (the first as it is).
            let mut acc: Option<(f32, f32)> = None;
            let mut add = |d: (f32, f32)| {
                acc = Some(match acc {
                    None => d,
                    Some(a) => (a.0 + d.0, a.1 + d.1),
                });
            };
            if hum_d {
                add((xd.0 - hd.0, xd.1 - hd.1));
            }
            if click_d {
                add((hd.0 - cd.0, hd.1 - cd.1));
            }
            if verb_d {
                add((cd.0 - v.0, cd.1 - v.1));
            }
            acc.unwrap_or(v)
        };
        let (dl, dr) = (xd.0 - v.0, xd.1 - v.1);
        self.removed_sq += (dl as f64) * (dl as f64) + (dr as f64) * (dr as f64);
        self.removed_n += 2;
        let out = (scaled(yl, g), scaled(yr, g));
        // The drain.
        if silent {
            self.zero_run += 1;
            if self.zero_run.is_multiple_of(DRAIN_EVERY) {
                self.try_drain();
            }
        } else {
            self.zero_run = 0;
        }
        out
    }

    fn try_drain(&mut self) {
        let verb_clear = !self.s.verb.on || self.verb.drained();
        let click_clear = !self.s.click.on || self.click.drained();
        let hum_clear = !self.hum_live || self.hum.quiet_below(DRAIN_BELOW);
        let delays_clear = self.zero_run > self.latency();
        if verb_clear && click_clear && hum_clear && delays_clear {
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
            clicks_last: self.click.repaired().0 as u32,
            clicks_total: self.click.repaired().1,
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
#[path = "purus_tests_click.rs"]
mod tests_click;
#[cfg(test)]
#[path = "purus_tests_measure.rs"]
mod tests_measure;
#[cfg(test)]
#[path = "purus_tests_shared.rs"]
mod tests_shared;
#[cfg(test)]
#[path = "purus_tests_verb.rs"]
mod tests_verb;

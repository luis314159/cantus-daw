//! Arcus's upper strings (round 42, `arcus-upper`; a child of `arcus`): the violin and the
//! viola on the cello's engine, and what their short bow–bridge segment needs.
//!
//! **Why the cello's engine was not enough.** The bow's junction splits the string's loop at
//! the contact point: the bridge loop is `β·P` samples long (`P` the period at the rate).
//! The junction's lines read it by a Lagrange interpolator that needs two samples at least,
//! and the motion between bow and bridge (the Helmholtz corner's return, the ripple the
//! lower Schelleng limit is made of) needs several more to exist at all. So `voice::min_beta`
//! keeps the segment five samples long — on a violin at 48 kHz that forces β above the
//! player's from E5 up (β ≥ 0.09 at A5, 0.14 at E6, 0.27 at E7, where a player bows near
//! 0.06 at ff): the upper register could not be bowed where it is bowed.
//!
//! **The choice, by measurement** (`docs/round42/arcus-upper.md`, `upper_tests::probe_rate`):
//! the whole voice — its string, its bow, its pluck, its pitch loop — runs at `os ×` the
//! rate (the violin 4×, the viola 2×), and a linear-phase half-band cascade brings it back ([`Decimator`]). The body,
//! the radiation, the match and the output stage stay at the rate, after the voices' sum.
//! A locally oversampled bridge segment was considered and set aside: the junction couples
//! both loops every sample, so a rate change inside the loop needs an interpolator and a
//! decimator *inside* the feedback path — their delay and their passband ripple go round
//! the loop every period and move its pitch and its losses, which is what the segment was
//! too short to model in the first place. A fractional junction placement is what the cello
//! already has (both loops are fractional delays): it keeps the pitch but not the motion
//! between bow and bridge.
//!
//! The cello and the double bass run at `os = 1`, through the code they always ran: their
//! output is the same to the bit (`upper_tests::the_cello_and_the_bass_keep_their_bits`).

use super::tables::Instrument;
use crate::math;

/// The largest factor a voice runs at.
pub const MAX_OS: usize = 4;

/// The factor an instrument's voices run at, by measurement (`upper_tests::probe_rate`).
pub fn factor(inst: Instrument) -> usize {
    match inst {
        Instrument::Cello | Instrument::Bass => 1,
        // The violin's E string bowed near the bridge leaves the segment 2.2 samples at 48
        // kHz (the study's §3.13: `d·f_s/(L·f_open)`, the note's own pitch does not enter);
        // at 2× its top fifth still breaks (−20 … −33 cents), at 4× every note holds.
        Instrument::Violin => 4,
        // The viola's A string at 2× keeps every note Helmholtz and within 0.03 cent, β at
        // its target to E6 (`probe_rate`), and costs 0.78 × what 4× does.
        Instrument::Viola => 2,
    }
}

/// The factor a voice of `inst` runs at with the tools' override (`ArcusSettings::oversample`,
/// `None` for the instrument's own): 1, 2 or 4.
pub fn factor_with(inst: Instrument, over: Option<u8>) -> usize {
    match over {
        Some(1) => 1,
        Some(2) => 2,
        Some(4) => 4,
        _ => factor(inst),
    }
}

/// Taps of the last stage (2× → 1×) and of the first (4× → 2×), both `4K + 3`.
const TAPS_LAST: usize = 75;
const TAPS_FIRST: usize = 23;
/// Kaiser's β for about 96 dB of stop band (β = 0.1102·(A − 8.7)).
const KAISER: f64 = 9.62;

/// The modified Bessel function of the first kind, order 0 (its series).
fn bessel_i0(x: f64) -> f64 {
    let q = 0.25 * x * x;
    let (mut sum, mut term) = (1.0f64, 1.0f64);
    for k in 1..200 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < 1e-18 * sum {
            break;
        }
    }
    sum
}

/// A half-band low-pass of `taps` (`4K + 3`) taps at a quarter of its rate, Kaiser-windowed.
/// Its side taps are not rescaled to make the DC gain exactly one: a uniform scale on them
/// moves the stop band's nulls (a scale of 1 + 2·10⁻⁴ costs 20 dB of it); the window leaves
/// the DC gain within its own ripple, 10⁻⁵.
pub fn half_band(taps: usize) -> Vec<f64> {
    let c = (taps - 1) / 2;
    let norm = bessel_i0(KAISER);
    (0..taps)
        .map(|n| {
            let t = n as f64 - c as f64;
            if n == c {
                return 0.5;
            }
            let ideal = math::sin(0.5 * std::f64::consts::PI * t) / (std::f64::consts::PI * t);
            let r = t / c as f64;
            ideal * bessel_i0(KAISER * (1.0 - r * r).max(0.0).sqrt()) / norm
        })
        .collect()
}

/// One half-band stage taking two samples to one. With `xe[k] = x[2k]`, `xo[k] = x[2k+1]`
/// and the centre tap `c = 2K + 1` (odd), `y[m] = ½·xo[m − K − 1] + Σᵢ h[2i]·xe[m − i]`:
/// the odd taps are zero but the centre, so the even phase is a plain FIR of `2K + 2` taps
/// and the odd phase a delay.
#[derive(Clone, Debug)]
struct Stage {
    even: Vec<f32>,
    /// The even phase's last `2K + 2` samples, written twice (a window that never wraps).
    xe: Vec<f32>,
    pe: usize,
    /// The odd phase's delay line (`K + 2` long) and its write position.
    xo: Vec<f32>,
    po: usize,
    k: usize,
    still: bool,
}

impl Stage {
    fn new(taps: usize) -> Self {
        let h = half_band(taps);
        let k = (taps - 3) / 4;
        // The window holds xe[m − (2K+1)] … xe[m], oldest first: tap i reads xe[m − i].
        let even: Vec<f32> = (0..2 * k + 2).rev().map(|i| h[2 * i] as f32).collect();
        Stage {
            even,
            xe: vec![0.0; 2 * (2 * k + 2)],
            pe: 0,
            xo: vec![0.0; k + 2],
            po: 0,
            k,
            still: true,
        }
    }

    fn reset(&mut self) {
        if !self.still {
            self.xe.fill(0.0);
            self.xo.fill(0.0);
        }
        self.pe = 0;
        self.po = 0;
        self.still = true;
    }

    /// One output from the next pair `(x[2m], x[2m+1])`.
    #[inline]
    fn tick(&mut self, e: f32, o: f32) -> f32 {
        self.still &= e == 0.0 && o == 0.0;
        let n = self.even.len();
        self.xe[self.pe] = e;
        self.xe[self.pe + n] = e;
        self.pe = if self.pe + 1 == n { 0 } else { self.pe + 1 };
        let w = &self.xe[self.pe..self.pe + n];
        let acc = self
            .even
            .iter()
            .zip(w)
            .fold(0.0f32, |acc, (h, x)| acc + h * x);
        // xo[m − K − 1]: read before this pair's odd sample is written (`K + 2` long).
        let len = self.xo.len();
        let back = self.k + 1;
        let idx = if self.po >= back {
            self.po - back
        } else {
            self.po + len - back
        };
        let delayed = self.xo[idx];
        self.xo[self.po] = o;
        self.po = if self.po + 1 == len { 0 } else { self.po + 1 };
        acc + 0.5 * delayed
    }
}

/// The voice's way back to the rate: one half-band stage for 2×, two for 4× (a short one
/// from 4× to 2×, the long one from 2× to 1× — flat to 20 kHz at 48 kHz and about 96 dB
/// down from 28 kHz, where an alias would fold under 20 kHz).
#[derive(Clone, Debug)]
pub struct Decimator {
    first: Stage,
    last: Stage,
    mid: Vec<f32>,
}

impl Decimator {
    /// Allocates: never on the audio thread.
    pub fn new() -> Self {
        Decimator {
            first: Stage::new(TAPS_FIRST),
            last: Stage::new(TAPS_LAST),
            mid: vec![0.0; 2 * super::CHUNK],
        }
    }

    pub fn reset(&mut self) {
        self.first.reset();
        self.last.reset();
    }

    /// The delay it adds at `os`, in samples at the rate.
    pub fn latency(os: usize) -> f64 {
        let c_last = ((TAPS_LAST - 1) / 2) as f64;
        let c_first = ((TAPS_FIRST - 1) / 2) as f64;
        match os {
            2 => c_last / 2.0,
            4 => c_last / 2.0 + c_first / 4.0,
            _ => 0.0,
        }
    }

    /// `x` (`os · out.len()` samples at `os ×` the rate) brought to the rate, **added** into
    /// `out`; the largest magnitude added is returned.
    pub fn run(&mut self, os: usize, x: &[f32], out: &mut [f32]) -> f32 {
        let n = out.len();
        let mut peak = 0f32;
        match os {
            2 => {
                for (m, o) in out.iter_mut().enumerate() {
                    let y = self.last.tick(x[2 * m], x[2 * m + 1]);
                    *o += y;
                    peak = peak.max(y.abs());
                }
            }
            4 => {
                let mid = &mut self.mid[..2 * n];
                for (j, v) in mid.iter_mut().enumerate() {
                    *v = self.first.tick(x[2 * j], x[2 * j + 1]);
                }
                for (m, o) in out.iter_mut().enumerate() {
                    let y = self.last.tick(self.mid[2 * m], self.mid[2 * m + 1]);
                    *o += y;
                    peak = peak.max(y.abs());
                }
            }
            _ => {
                for (o, v) in out.iter_mut().zip(x) {
                    *o += *v;
                    peak = peak.max(v.abs());
                }
            }
        }
        peak
    }
}

impl Default for Decimator {
    fn default() -> Self {
        Self::new()
    }
}

#[path = "arcus_upper_body.rs"]
pub mod body;
#[path = "arcus_upper_tables.rs"]
pub mod tables;

/// The violin's and the viola's bowed-pitch table (`examples/arcus_upper_pitch.rs`), read by
/// `pitch::TABLE`.
#[path = "arcus_upper_pitch_data.rs"]
pub mod pitch_data;

/// The bridge's corner rounding a bowed note of `inst` at `f0` Hz is drawn with
/// (`string::StringStep::corner_hz`): the instrument's, and for the violin and the viola at
/// least [`CORNER_PER_F0`] × the note's fundamental — a one-pole whose time constant is a
/// fixed share of the period, so the top notes (a slip a few samples long even at 4×) keep the
/// Helmholtz corner sharper than the slip itself (measured: `upper_tests_rate::probe_corner`).
/// The cello's and the bass's are their own, untouched.
pub fn corner_hz(inst: Instrument, spec_hz: f64, f0: f64) -> f64 {
    match inst {
        Instrument::Cello | Instrument::Bass => spec_hz,
        Instrument::Violin | Instrument::Viola => spec_hz.max(corner_ratio() * f0),
    }
}

/// The corner's least ratio to the fundamental on the upper strings.
pub const CORNER_PER_F0: f64 = 6.0;

#[cfg(not(test))]
fn corner_ratio() -> f64 {
    CORNER_PER_F0
}

#[cfg(test)]
thread_local! {
    /// The probes' override of [`CORNER_PER_F0`].
    pub static CORNER_RATIO: std::cell::Cell<f64> = const { std::cell::Cell::new(CORNER_PER_F0) };
}

#[cfg(test)]
fn corner_ratio() -> f64 {
    CORNER_RATIO.with(|c| c.get())
}

/// How a violinist or a violist bows high on a string: lighter and slower as the stopped
/// length shortens (the force's place in its region, in octaves; the speed's scale), so the
/// Helmholtz motion holds to the top of the range — on this model the top fifth of the
/// violin's E string bowed ff with the open string's bowing goes raucous (−25 to −40 cents,
/// the energy between the harmonics: `upper_tests_rate::probe_top`), and half an octave
/// lighter or a third slower brings it back. Nothing for the cello and the bass.
pub fn force_trim(inst: Instrument, open_hz: f64, f0: f64) -> f64 {
    match inst {
        Instrument::Cello | Instrument::Bass => 0.0,
        Instrument::Violin | Instrument::Viola => {
            0.35 * math::log2((open_hz / f0.max(1.0)).clamp(0.1, 1.0))
        }
    }
}

/// The bow speed's scale high on a string (see [`force_trim`]).
pub fn speed_scale(inst: Instrument, open_hz: f64, f0: f64) -> f64 {
    match inst {
        Instrument::Cello | Instrument::Bass => 1.0,
        Instrument::Violin | Instrument::Viola => {
            math::powf((open_hz / f0.max(1.0)).clamp(0.1, 1.0), 0.25)
        }
    }
}

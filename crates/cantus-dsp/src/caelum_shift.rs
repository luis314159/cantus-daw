//! Caelum's pitch shifter: the one that lives inside the reverb's feedback.
//!
//! ## Why not PSOLA or the phase vocoder
//!
//! Cantus has two shifters already ([`crate::psola`], Saltus's poly engine). PSOLA needs a
//! pitch to lock its grains to, and a reverb's tail has none: it is a sum of thousands of
//! decaying modes. The phase vocoder handles any sound but costs 85 ms and a few hundred
//! operations a sample — in a loop that runs the tail through itself every pass, its
//! latency is only more pre-delay, but its cost is four times what the whole tank costs.
//! A diffuse signal needs neither: no grain boundary can be heard in noise. So this is the
//! oldest shifter there is — a delay line read by moving heads — made for diffuse feedback:
//!
//! - **Three heads**, a third of a turn apart, each reading the line at a delay that sweeps
//!   once per grain (80 ms) — down for a shift up, up for a shift down — through a Hann
//!   window. Three Hann windows a third apart **sum to exactly one**, so a coherent signal
//!   (a held chord) keeps its amplitude with no ripple; and **their squares sum to exactly
//!   one half**, so a diffuse signal (where the heads read uncorrelated samples) keeps a
//!   constant power with no ripple either. Two heads give one or the other, never both —
//!   the flutter of cheap shimmers.
//! - **An energy normaliser** brings the output's power to the input's: the power of the
//!   input read at the heads' mean delay against the power of what the heads made, over
//!   150 ms, as a gain between ½ and 2 (√2 for a diffuse tail, where the three heads
//!   lose 3 dB). In the loop, this is what makes the shimmer neither grow nor sink: the
//!   energy taken out of the tank is the energy put back.
//! - The phase is an integer count (2³² a turn), so a skipped shifter's phase is a
//!   processed one's to the bit, and a new one skipped as far is the same.

use super::parts::SineTable;
use crate::math;
use crate::rest::ZeroRun;

/// The grain: one sweep of a head, ms.
pub const GRAIN_MS: f32 = 80.0;
/// The heads' phases, a third of a turn apart.
const OFFSETS: [u32; 3] = [0, 1_431_655_765, 2_863_311_531];
/// The normaliser's gain range.
pub const NORM_MIN: f32 = 0.5;
pub const NORM_MAX: f32 = 2.0;
/// The normaliser's time constant, s.
const NORM_S: f32 = 0.15;

/// One shifter: its line, its heads, its normaliser.
#[derive(Clone, Debug)]
pub struct Shifter {
    pub buf: Vec<f32>,
    mask: usize,
    w: usize,
    phase: u32,
    inc: u32,
    grain: f32,
    /// The normaliser's two powers: the input at the heads' mean delay, and the heads' sum.
    pub p_in: f32,
    pub p_out: f32,
    k_pow: f32,
    pub zeros: ZeroRun,
    ratio: f32,
    /// Where the heads start (two shifters of one voice start apart, so the two sides of
    /// the shimmer are not the same grains).
    origin: u32,
}

impl Shifter {
    pub fn new(sample_rate: f32) -> Self {
        Self::with_origin(sample_rate, 0)
    }

    /// A shifter whose heads start at `origin` (2³² a turn).
    pub fn with_origin(sample_rate: f32, origin: u32) -> Self {
        let grain = (GRAIN_MS * 0.001 * sample_rate).round();
        let size = (grain as usize + 8).next_power_of_two();
        Shifter {
            buf: vec![0.0; size],
            mask: size - 1,
            w: 0,
            phase: origin,
            inc: 0,
            grain,
            p_in: 0.0,
            p_out: 0.0,
            k_pow: 1.0 - math::exp(-1.0 / (NORM_S * sample_rate)),
            zeros: ZeroRun::FULL,
            ratio: 1.0,
            origin,
        }
    }

    /// The pitch ratio for `semitones`.
    pub fn ratio_of(semitones: f32) -> f32 {
        math::powf(2f32, semitones / 12.0)
    }

    /// Shift by `semitones`. The heads keep their places; only their speed changes.
    pub fn set_semitones(&mut self, semitones: f32) {
        let r = Self::ratio_of(semitones.clamp(-36.0, 36.0));
        self.ratio = r;
        // The delay moves by (1 − r) samples a sample: (1 − r) / grain turns of the phase.
        let turns = (1.0 - r as f64) / self.grain as f64;
        let v = (turns - turns.floor()) * 4_294_967_296.0;
        self.inc = (v as u64 & 0xFFFF_FFFF) as u32;
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// The heads' mean delay, samples: what the shimmer adds to a pass of the loop.
    pub fn mean_delay(&self) -> f32 {
        1.0 + 0.5 * self.grain
    }

    /// The normaliser's gain now.
    pub fn gain(&self) -> f32 {
        if self.p_out > 1e-30 {
            (self.p_in / self.p_out).sqrt().clamp(NORM_MIN, NORM_MAX)
        } else {
            1.0
        }
    }

    /// One sample in, one shifted sample out.
    #[inline]
    pub fn tick(&mut self, x: f32, t: &SineTable) -> f32 {
        let mask = self.mask;
        let w = self.w;
        self.buf[w] = x;
        let mut y = 0.0f32;
        for off in OFFSETS {
            let ph = self.phase.wrapping_add(off);
            let win = (1.0 - t.cos(ph)) * (1.0 / 3.0);
            let d = 1.0 + self.grain * (ph as f32 * (1.0 / 4_294_967_296.0));
            let n = d as usize;
            let f = d - n as f32;
            let a = self.buf[w.wrapping_sub(n) & mask];
            let b = self.buf[w.wrapping_sub(n + 1) & mask];
            y += win * (a + (b - a) * f);
        }
        let mid = self.buf[w.wrapping_sub(self.mean_delay() as usize) & mask];
        self.p_in += (mid * mid - self.p_in) * self.k_pow;
        self.p_out += (y * y - self.p_out) * self.k_pow;
        if self.p_in < 1e-30 {
            self.p_in = 0.0;
        }
        if self.p_out < 1e-30 {
            self.p_out = 0.0;
        }
        let out = y * self.gain();
        self.phase = self.phase.wrapping_add(self.inc);
        self.w = (w + 1) & mask;
        out
    }

    /// After a block of `n` ticks: the line's run of zeros.
    pub fn wrote(&mut self, n: usize) {
        self.zeros.wrote(&self.buf, self.w, n);
    }

    /// At rest: the line all +0.0 and the normaliser empty.
    pub fn is_zero(&self) -> bool {
        self.zeros.covers(self.buf.len())
            && self.p_in.to_bits() == 0
            && self.p_out.to_bits() == 0
    }

    /// `n` samples of silence not run: the write position and the heads' phase move as
    /// ticks would move them.
    pub fn skip(&mut self, n: usize) {
        self.w = (self.w + n) & self.mask;
        self.phase = self.phase.wrapping_add(self.inc.wrapping_mul(n as u32));
    }

    /// Everything cleared but the count (the phase and the write position, which a new
    /// shifter skipped as far would have too).
    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.p_in = 0.0;
        self.p_out = 0.0;
        self.zeros = ZeroRun::FULL;
    }

    /// As new: the count too.
    pub fn reset(&mut self) {
        self.clear();
        self.w = 0;
        self.phase = self.origin;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn sine(hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| 0.5 * math::sin(std::f32::consts::TAU * hz * i as f32 / FS))
            .collect()
    }

    /// The power-weighted mean frequency of a signal between `lo` and `hi`, and the share
    /// of its power there that lies within ±25 cents of `want`: a scan of its correlation
    /// with sines, a cent apart.
    fn centroid(x: &[f32], lo: f32, hi: f32, want: f32) -> (f32, f32) {
        let (mut acc, mut sum, mut near) = (0.0f64, 0.0f64, 0.0f64);
        let mut hz = lo;
        while hz < hi {
            let (mut c, mut s) = (0.0f64, 0.0f64);
            for (i, v) in x.iter().enumerate() {
                let p = std::f64::consts::TAU * hz as f64 * i as f64 / FS as f64;
                c += *v as f64 * math::cos(p);
                s += *v as f64 * math::sin(p);
            }
            let m = c * c + s * s;
            acc += m * hz as f64;
            sum += m;
            if (1200.0 * math::log2(hz / want)).abs() <= 25.0 {
                near += m;
            }
            hz *= 1.000_578;
        }
        ((acc / sum) as f32, (near / sum) as f32)
    }

    #[test]
    fn an_octave_up_doubles_the_frequency() {
        let t = SineTable::new();
        for (st, want) in [(12.0f32, 880.0f32), (7.0, 659.26), (-12.0, 220.0), (5.0, 587.33)] {
            let mut s = Shifter::new(FS);
            s.set_semitones(st);
            let x = sine(440.0, 48_000);
            let y: Vec<f32> = x.iter().map(|v| s.tick(*v, &t)).collect();
            let (got, share) = centroid(&y[12_000..24_000], want * 0.8, want * 1.25, want);
            let cents = 1200.0 * math::log2(got / want);
            eprintln!(
                "{st:+} st: centre {got:.1} Hz for {want:.1} ({cents:+.1} cents), {:.0} % within 25 cents",
                share * 100.0
            );
            assert!(cents.abs() < 10.0, "{st}: {got} Hz");
        }
    }

    /// Noise in: the power out equals the power in within 0.3 dB, with no ripple at the
    /// grain's rate over 20 ms windows beyond 1.5 dB (two heads ripple by 3 dB).
    #[test]
    fn a_diffuse_signal_keeps_its_power_without_ripple() {
        let t = SineTable::new();
        let mut s = Shifter::new(FS);
        s.set_semitones(12.0);
        let mut seed = 7u32;
        let x: Vec<f32> = (0..96_000)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();
        let y: Vec<f32> = x.iter().map(|v| s.tick(*v, &t)).collect();
        let p = |v: &[f32]| v.iter().map(|a| (*a as f64) * (*a as f64)).sum::<f64>() / v.len() as f64;
        let ratio_db = 10.0 * math::log10(p(&y[24_000..]) / p(&x[24_000..]));
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for c in y[24_000..].chunks(960) {
            let d = 10.0 * math::log10(p(c));
            lo = lo.min(d);
            hi = hi.max(d);
        }
        eprintln!("power {ratio_db:+.2} dB; 20 ms windows within {:.2} dB", hi - lo);
        assert!(ratio_db.abs() < 0.3);
        assert!(hi - lo < 1.5);
    }

    #[test]
    fn a_skipped_shifter_counts_as_a_processed_one() {
        let t = SineTable::new();
        let mut a = Shifter::new(FS);
        let mut b = Shifter::new(FS);
        a.set_semitones(7.0);
        b.set_semitones(7.0);
        for _ in 0..1234 {
            a.tick(0.0, &t);
        }
        b.skip(1234);
        assert_eq!(a.phase, b.phase);
        assert_eq!(a.w, b.w);
    }
}

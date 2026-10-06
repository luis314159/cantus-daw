//! Measuring a reverb: what the tests hold Spatium to, and what its editor draws from the
//! engine's real output — decay time per octave by Schroeder's backward integration, the
//! envelope of a tail, echo density, periodicity and stereo correlation.
//!
//! These run off the audio thread (in tests and on the UI thread) and allocate freely.

// The DSP here walks several parallel arrays (lines, sections, grid points) with one
// index; that reads clearer than zipping five iterators.
#![allow(clippy::needless_range_loop)]

use crate::aequalis::{design, Coefs, MAX_SECTIONS};
use crate::math;
use crate::EqShape;

/// Octave centres the displays and tests read decay at, Hz.
pub const OCTAVES: [f32; 8] = [
    63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0,
];

fn run(c: &Coefs, x: &mut [f32]) {
    let (mut z1, mut z2) = (0.0f64, 0.0f64);
    let (b0, b1, b2, a1, a2) = (
        c.b0 as f64,
        c.b1 as f64,
        c.b2 as f64,
        c.a1 as f64,
        c.a2 as f64,
    );
    for v in x.iter_mut() {
        let xin = *v as f64;
        let y = b0 * xin + z1;
        z1 = b1 * xin - a1 * y + z2;
        z2 = b2 * xin - a2 * y;
        *v = y as f32;
    }
}

/// The signal through an octave band-pass centred on `hz`: 24 dB/octave on each side.
pub fn octave_band(signal: &[f32], sample_rate: f32, hz: f32) -> Vec<f32> {
    octave_band_sloped(signal, sample_rate, hz, 24.0)
}

/// [`octave_band`] with skirts of `slope` dB/octave (6 … 96).
pub fn octave_band_sloped(signal: &[f32], sample_rate: f32, hz: f32, slope: f32) -> Vec<f32> {
    let mut out = signal.to_vec();
    let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
    let lo = hz / std::f32::consts::SQRT_2;
    let hi = (hz * std::f32::consts::SQRT_2).min(0.45 * sample_rate);
    let n = design(EqShape::LowCut, sample_rate, lo, 0.0, 0.707, slope, &mut c);
    for s in &c[..n] {
        run(s, &mut out);
    }
    let n = design(EqShape::HighCut, sample_rate, hi, 0.0, 0.707, slope, &mut c);
    for s in &c[..n] {
        run(s, &mut out);
    }
    out
}

/// Schroeder's energy decay curve, dB, 0 at the start.
pub fn schroeder_db(signal: &[f32]) -> Vec<f32> {
    let mut acc = 0.0f64;
    let mut edc = vec![0.0f64; signal.len()];
    for (i, x) in signal.iter().enumerate().rev() {
        acc += (*x as f64) * (*x as f64);
        edc[i] = acc;
    }
    let total = edc.first().copied().unwrap_or(0.0).max(1e-300);
    edc.iter()
        .map(|e| (10.0 * math::log10((e / total).max(1e-30))) as f32)
        .collect()
}

/// RT60 from a decay curve: a least-squares line through the part between `from_db` and
/// `to_db` (e.g. −5 and −25 for T20), extrapolated to 60 dB.
pub fn fit_decay(edc_db: &[f32], sample_rate: f32, from_db: f32, to_db: f32) -> Option<f32> {
    let start = edc_db.iter().position(|d| *d <= from_db)?;
    let end = edc_db.iter().position(|d| *d <= to_db)?;
    if end <= start + 8 {
        return None;
    }
    let n = (end - start) as f64;
    let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
    for (k, d) in edc_db[start..end].iter().enumerate() {
        let x = k as f64 / sample_rate as f64;
        let y = *d as f64;
        sx += x;
        sy += y;
        sxx += x * x;
        sxy += x * y;
    }
    let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    if slope >= 0.0 {
        return None;
    }
    Some((-60.0 / slope) as f32)
}

/// RT60 of `signal` in the octave around `hz`, by T20 (−5 to −25 dB).
pub fn band_t60(signal: &[f32], sample_rate: f32, hz: f32) -> Option<f32> {
    let band = octave_band(signal, sample_rate, hz);
    fit_decay(&schroeder_db(&band), sample_rate, -5.0, -25.0)
}

/// How far a band must fall, from where T20's fit starts to the end of the capture, for
/// its T20 to be *measured*: the fit's 20 dB and a 10 dB margin. Less, and part of the fit
/// rests on the energy the capture did not hold — estimated, not heard.
pub const COMPLETE_DB: f32 = 30.0;

/// A decay as measured on a capture that may have been cut short.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Decay {
    /// RT60 by T20, seconds, with the energy after the end of the capture accounted for.
    pub t60: Option<f32>,
    /// How far the level fell from where the fit starts (the decay curve's −5 dB point) to
    /// the end of the capture, dB.
    pub fell_db: f32,
}

impl Decay {
    /// The capture held the decay the fit reads ([`COMPLETE_DB`]); otherwise the number is
    /// an extrapolation from a tail that was cut, and a display should say so.
    pub fn complete(&self) -> bool {
        self.fell_db >= COMPLETE_DB
    }
}

/// The skirts of the octave filters [`band_decay`] measures through, dB/octave.
pub const DECAY_BAND_SLOPE: f32 = 96.0;

/// The decay of the octave around `hz`, as the editor shows it: on whatever the reverb was
/// fed, with a capture that may stop before the tail has died away.
///
/// - **What the octave hears.** Music is not a flat test signal: a hit with nothing below
///   100 Hz, through a post EQ that cuts at 140 Hz, leaves the 63 Hz octave almost empty,
///   and a 24 dB/octave band then measures its louder neighbours' faster decay through its
///   skirts (the round-16 demo: 3.43 s against 4.46 s, whether 2.9 s or 13 s of tail were
///   captured). The band is filtered at [`DECAY_BAND_SLOPE`] instead, which leaves the
///   neighbours 100 dB down; its own ringing is over in a few tens of milliseconds.
/// - **A cut tail.** Schroeder's backward integration of a tail cut before it has died
///   away misses the energy after the cut, so its curve bends down early and T20 reads
///   short. The missing energy is estimated (ISO 3382-1's correction for truncation,
///   iterated to agreement) and added back; [`Decay::complete`] says whether the capture
///   held the decay at all.
pub fn band_decay(signal: &[f32], sample_rate: f32, hz: f32) -> Decay {
    decay_of(
        &octave_band_sloped(signal, sample_rate, hz, DECAY_BAND_SLOPE),
        sample_rate,
    )
}

/// [`band_decay`] for the whole signal, unfiltered.
pub fn decay(signal: &[f32], sample_rate: f32) -> Decay {
    decay_of(signal, sample_rate)
}

fn decay_of(x: &[f32], fs: f32) -> Decay {
    let n = x.len();
    let win = ((0.05 * fs) as usize).clamp(1, n.max(1));
    let power: Vec<f64> = x.iter().map(|v| (*v as f64) * (*v as f64)).collect();
    if n < 2 * win {
        return Decay {
            t60: None,
            fell_db: 0.0,
        };
    }
    let mut tail = vec![0.0f64; n];
    let mut acc = 0.0f64;
    for i in (0..n).rev() {
        acc += power[i];
        tail[i] = acc;
    }
    // How far the level fell over the part the fit reads: from where the decay curve
    // passes −5 dB (after any early burst that is not the decay — strong reflections in an
    // otherwise quiet octave) to the end, each level the mean power over up to 200 ms so
    // that even the lowest octave's is steady.
    let span = ((0.2 * fs) as usize).min(n / 4).max(1);
    let mean = |a: usize| power[a..a + span].iter().sum::<f64>() / span as f64;
    let from = tail
        .iter()
        .position(|e| *e <= tail[0] * math::powf(10f64, -0.5))
        .unwrap_or(0)
        .min(n - span);
    let (start, end) = (mean(from), mean(n - span));
    let fell_db = if start <= 0.0 {
        0.0
    } else {
        (10.0 * math::log10(start / end.max(start * 1e-30))) as f32
    };
    // The end window's power, for whether any energy follows the cut.
    let end = power[n - win..].iter().sum::<f64>() / win as f64;
    let fit = |extra: f64| -> Option<f32> {
        let total = (tail[0] + extra).max(1e-300);
        let db: Vec<f32> = tail
            .iter()
            .map(|e| (10.0 * math::log10(((e + extra) / total).max(1e-30))) as f32)
            .collect();
        fit_decay(&db, fs, -5.0, -25.0)
    };
    let mut t60 = fit(0.0);
    if end > 0.0 {
        // The energy after the cut, from the energy of the capture's last quarter: for a
        // decay of power rate k, what follows the cut is that energy / (e^(kW) − 1). A
        // quarter of the capture is many cycles even of the lowest octave, so the estimate
        // is steady where one short window's level would swing by decibels. Each better
        // rate gives a better sum.
        let w = (n / 4).max(win);
        let last = tail[n - w];
        for _ in 0..20 {
            let Some(t) = t60 else { break };
            let k = 13.815_5 / (t as f64 * fs as f64);
            let extra = last / (math::exp(k * w as f64) - 1.0).max(1e-12);
            let next = fit(extra);
            let settled = next.is_some_and(|v| (v - t).abs() <= t * 1e-4);
            t60 = next;
            if settled {
                break;
            }
        }
    }
    Decay { t60, fell_db }
}

/// RT60 in the octave around `hz`, energy-averaged over several responses (several
/// source and receiver positions, in acoustics' terms — ISO 3382 averages them the same
/// way): the band-filtered energies are summed before the backward integration, so a
/// band with few modes in one response does not decide the number alone.
pub fn band_t60_averaged(signals: &[&[f32]], sample_rate: f32, hz: f32) -> Option<f32> {
    let n = signals.iter().map(|s| s.len()).min()?;
    let mut energy = vec![0.0f64; n];
    for s in signals {
        let band = octave_band(&s[..n], sample_rate, hz);
        for (e, x) in energy.iter_mut().zip(&band) {
            *e += (*x as f64) * (*x as f64);
        }
    }
    let mut acc = 0.0f64;
    let mut edc = vec![0.0f64; n];
    for i in (0..n).rev() {
        acc += energy[i];
        edc[i] = acc;
    }
    let total = edc.first().copied().unwrap_or(0.0).max(1e-300);
    let db: Vec<f32> = edc
        .iter()
        .map(|e| (10.0 * math::log10((e / total).max(1e-30))) as f32)
        .collect();
    fit_decay(&db, sample_rate, -5.0, -25.0)
}

/// What the octave band around `hz` would measure on an ideal reverb whose decay follows
/// `t60` exactly: the band's energy decay is a sum of exponentials, one per frequency,
/// weighted by the band filter, and where the curve is steep the slowest of them
/// dominates the fit. Comparing a measurement with this, rather than with the curve at
/// the band's centre, compares like with like.
pub fn expected_band_t60(t60: impl Fn(f32) -> f32, sample_rate: f32, hz: f32) -> Option<f32> {
    let mut lo_c = [Coefs::IDENTITY; MAX_SECTIONS];
    let mut hi_c = [Coefs::IDENTITY; MAX_SECTIONS];
    let lo = hz / std::f32::consts::SQRT_2;
    let hi = (hz * std::f32::consts::SQRT_2).min(0.45 * sample_rate);
    let nl = design(
        EqShape::LowCut,
        sample_rate,
        lo,
        0.0,
        0.707,
        24.0,
        &mut lo_c,
    );
    let nh = design(
        EqShape::HighCut,
        sample_rate,
        hi,
        0.0,
        0.707,
        24.0,
        &mut hi_c,
    );
    // Components on a 1/24-octave grid over ±3 octaves: (weight, T60).
    let mut comps = Vec::new();
    for k in -72..=72 {
        let f = hz * math::powf(2f32, k as f32 / 24.0);
        if f < 10.0 || f > 0.49 * sample_rate {
            continue;
        }
        let db: f32 = lo_c[..nl]
            .iter()
            .chain(&hi_c[..nh])
            .map(|c| c.magnitude_db(sample_rate, f))
            .sum();
        // Flat energy per hertz: on a log grid each point stands for a width ∝ f.
        let w = math::powf(10f64, db as f64 / 10.0) * f as f64;
        comps.push((w, t60(f).max(1e-3) as f64));
    }
    let longest = comps.iter().map(|c| c.1).fold(0.0, f64::max);
    let rate = 1_000.0;
    let steps = (longest * 2.0 * rate) as usize + 2;
    let edc: Vec<f64> = (0..steps)
        .map(|i| {
            let t = i as f64 / rate;
            // The backward integral of w·e^(−13.8 t/T) is w·T/13.8·e^(−13.8 t/T).
            comps
                .iter()
                .map(|(w, tt)| w * tt * math::exp(-13.815_5 * t / tt))
                .sum()
        })
        .collect();
    let total = edc[0].max(1e-300);
    let db: Vec<f32> = edc
        .iter()
        .map(|e| (10.0 * math::log10((e / total).max(1e-30))) as f32)
        .collect();
    fit_decay(&db, rate as f32, -5.0, -25.0)
}

/// Broadband RT60 by T20.
pub fn t60(signal: &[f32], sample_rate: f32) -> Option<f32> {
    fit_decay(&schroeder_db(signal), sample_rate, -5.0, -25.0)
}

/// The envelope of a signal: RMS in bins of `bin_ms`, dB.
pub fn envelope_db(signal: &[f32], sample_rate: f32, bin_ms: f32) -> Vec<f32> {
    let bin = ((bin_ms * 0.001 * sample_rate) as usize).max(1);
    signal
        .chunks(bin)
        .map(|c| {
            let e = c.iter().map(|x| math::powi(*x as f64, 2)).sum::<f64>() / c.len() as f64;
            (10.0 * math::log10(e.max(1e-20))) as f32
        })
        .collect()
}

/// Normalised echo density (Abel & Huang, 2006) in windows of `win_ms`: the fraction of
/// samples outside one standard deviation, over what Gaussian noise would give. About 1 is
/// a dense, noise-like tail; well under 1 is sparse echoes.
pub fn echo_density(signal: &[f32], sample_rate: f32, win_ms: f32) -> Vec<f32> {
    let win = ((win_ms * 0.001 * sample_rate) as usize).max(16);
    const GAUSS_OUTSIDE: f64 = 0.317_310_5;
    signal
        .chunks_exact(win)
        .map(|c| {
            let var = c.iter().map(|x| math::powi(*x as f64, 2)).sum::<f64>() / c.len() as f64;
            let sd = var.sqrt();
            if sd <= 0.0 {
                return 0.0;
            }
            let outside = c.iter().filter(|x| (**x as f64).abs() > sd).count() as f64;
            (outside / c.len() as f64 / GAUSS_OUTSIDE) as f32
        })
        .collect()
}

/// Correlation coefficient of two signals (−1…1).
pub fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        ab += (*x as f64) * (*y as f64);
        aa += math::powi(*x as f64, 2);
        bb += math::powi(*y as f64, 2);
    }
    if aa <= 0.0 || bb <= 0.0 {
        return 0.0;
    }
    (ab / (aa * bb).sqrt()) as f32
}

/// The strongest repetition in a signal: the largest normalised autocorrelation at a lag
/// between `min_ms` and `max_ms`, and that lag. A tail that rings or flutters at a loop's
/// period shows a peak near 1 at that period; a dense tail stays low everywhere.
pub fn max_autocorrelation(
    signal: &[f32],
    sample_rate: f32,
    min_ms: f32,
    max_ms: f32,
) -> (f32, f32) {
    let n = signal.len();
    let size = (2 * n).next_power_of_two();
    let mut re = vec![0.0f32; size];
    let mut im = vec![0.0f32; size];
    re[..n].copy_from_slice(signal);
    crate::spectrum::fft(&mut re, &mut im);
    for k in 0..size {
        re[k] = re[k] * re[k] + im[k] * im[k];
        im[k] = 0.0;
    }
    // The inverse through the forward transform: conjugate symmetry makes it real.
    crate::spectrum::fft(&mut re, &mut im);
    let zero = re[0].max(1e-30);
    let lo = (min_ms * 0.001 * sample_rate) as usize;
    let hi = ((max_ms * 0.001 * sample_rate) as usize).min(n - 1);
    let mut best = (0.0f32, 0.0f32);
    for lag in lo.max(1)..=hi {
        // Unbiased: each lag has fewer overlapping samples.
        let r = re[lag] / zero * n as f32 / (n - lag) as f32;
        if r > best.0 {
            best = (r, lag as f32 * 1000.0 / sample_rate);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_exponential_decay_measures_its_rt60() {
        let fs = 48_000.0;
        let t60 = 1.5f32;
        let mut seed = 1u32;
        let sig: Vec<f32> = (0..(fs as usize * 2))
            .map(|i| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
                noise * math::powf(10f32, -3.0 * i as f32 / (fs * t60))
            })
            .collect();
        let got = t60_of(&sig, fs);
        assert!((got / t60 - 1.0).abs() < 0.03, "{got}");
        let band = band_t60(&sig, fs, 1_000.0).unwrap();
        assert!((band / t60 - 1.0).abs() < 0.05, "{band}");
        let ned = echo_density(&sig[4800..24_000], fs, 20.0);
        assert!(ned.iter().all(|d| *d > 0.8), "{ned:?}");
    }

    fn t60_of(sig: &[f32], fs: f32) -> f32 {
        t60(sig, fs).unwrap()
    }

    /// The demo's real case: a nearly empty low octave (4.46 s) beside a loud tone an octave
    /// and a half up that decays faster (3.2 s). Through 24 dB/octave skirts the low octave
    /// reads the tone's decay; through the editor's steep ones it reads its own.
    #[test]
    fn a_quiet_octave_is_not_read_off_its_loud_neighbour() {
        let fs = 48_000.0f32;
        let mut s = 3u32;
        let n = (8.0 * fs) as usize;
        let white: Vec<f32> = (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();
        let env = |t60: f32, i: usize| math::powf(10f32, -3.0 * i as f32 / (fs * t60));
        let low: Vec<f32> = octave_band(
            &white
                .iter()
                .enumerate()
                .map(|(i, v)| v * env(4.46, i))
                .collect::<Vec<_>>(),
            fs,
            63.0,
        )
        .iter()
        .map(|v| v * 0.3)
        .collect();
        // The tone fades in over 20 ms, as a reverb's tail builds: no click to leak.
        let tone: Vec<f32> = (0..n)
            .map(|i| {
                let fade = (i as f32 / (0.02 * fs)).min(1.0);
                math::sin(i as f32 / fs * 190.0 * std::f32::consts::TAU) * 0.5 * fade * env(3.2, i)
            })
            .collect();
        let mix: Vec<f32> = low.iter().zip(&tone).map(|(a, b)| a + b).collect();
        let own = band_decay(&low, fs, 63.0).t60.unwrap();
        let skirted = band_t60(&mix, fs, 63.0).unwrap();
        let steep = band_decay(&mix, fs, 63.0).t60.unwrap();
        eprintln!(
            "63 Hz alone {own:.2} s; beside the tone: 24 dB/oct {skirted:.2} s, steep {steep:.2} s"
        );
        assert!(
            skirted < own * 0.85,
            "the premise: 24 dB/oct skirts read the tone ({skirted} s against {own} s)"
        );
        assert!(
            (steep / own - 1.0).abs() < 0.02,
            "the editor's measure read {steep} s for an octave that decays over {own} s"
        );
    }

    /// A low octave decaying over 4.46 s under a louder mid octave decaying over 1.5 s —
    /// the demo's case. Cut after 2 s (when the whole tail has fallen far enough to be
    /// measured), plain Schroeder reads the low octave short; the corrected measure reads
    /// it right and says the capture was cut there, while the mid octave, captured whole,
    /// reads right and complete.
    #[test]
    fn a_cut_tail_is_measured_right_and_said_to_be_cut() {
        let fs = 48_000.0f32;
        let noise = |seed: u32, n: usize| {
            let mut s = seed;
            (0..n)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 17;
                    s ^= s << 5;
                    (s as f32 / u32::MAX as f32) * 2.0 - 1.0
                })
                .collect::<Vec<f32>>()
        };
        let decaying = |seed: u32, t60: f32, hz: f32, gain: f32, n: usize| {
            let x: Vec<f32> = noise(seed, n)
                .iter()
                .enumerate()
                .map(|(i, v)| v * gain * math::powf(10f32, -3.0 * i as f32 / (fs * t60)))
                .collect();
            octave_band(&x, fs, hz)
        };
        let whole = (fs * 12.0) as usize;
        let low = decaying(7, 4.46, 63.0, 1.0, whole);
        let mid = decaying(11, 1.5, 1_000.0, 3.0, whole);
        let full: Vec<f32> = low.iter().zip(&mid).map(|(a, b)| a + b).collect();
        let cut = &full[..(fs * 2.0) as usize];

        let plain = band_t60(cut, fs, 63.0).unwrap();
        let low_cut = band_decay(cut, fs, 63.0);
        let low_whole = band_decay(&full, fs, 63.0);
        let mid_cut = band_decay(cut, fs, 1_000.0);
        eprintln!(
            "63 Hz: plain {plain:.2} s, corrected {:?} s (fell {:.1} dB), whole {:?} s; \
             1 kHz cut {:?} s (fell {:.1} dB)",
            low_cut.t60, low_cut.fell_db, low_whole.t60, mid_cut.t60, mid_cut.fell_db
        );
        // One realisation of a narrow low octave reads a few per cent off its nominal
        // decay whatever the method; what the cut must not do is move it. So the cut is
        // compared with the whole tail of the same noise.
        let whole_t = low_whole.t60.unwrap();
        assert!((whole_t / 4.46 - 1.0).abs() < 0.08, "whole: {whole_t}");
        assert!(low_whole.complete());
        assert!(
            plain < whole_t * 0.93,
            "the plain measure should read short: {plain} against {whole_t}"
        );
        let got = low_cut.t60.unwrap();
        assert!(
            (got / whole_t - 1.0).abs() < 0.03,
            "the corrected measure of the cut tail: {got} s against {whole_t}"
        );
        assert!(
            !low_cut.complete(),
            "the low octave's tail was cut: {low_cut:?}"
        );
        let mid_t = mid_cut.t60.unwrap();
        assert!((mid_t / 1.5 - 1.0).abs() < 0.08, "mid: {mid_t}");
        assert!(
            mid_cut.complete(),
            "the mid octave fell far enough: {mid_cut:?}"
        );
    }

    #[test]
    fn a_comb_shows_its_period() {
        let fs = 48_000.0;
        let period = 480; // 10 ms
        let mut sig = vec![0.0f32; 24_000];
        let mut seed = 5u32;
        for i in 0..sig.len() {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            sig[i] = noise * 0.1
                + if i >= period {
                    0.9 * sig[i - period]
                } else {
                    0.0
                };
        }
        let (r, lag) = max_autocorrelation(&sig, fs, 2.0, 50.0);
        assert!(r > 0.5 && (lag - 10.0).abs() < 0.1, "{r} at {lag} ms");
    }
}

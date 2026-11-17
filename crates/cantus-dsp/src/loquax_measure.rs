//! Loquax's measurements and test material: a speech-like phrase made of our own pieces,
//! and two intelligibility proxies. Not for the audio thread (it allocates).
//!
//! - [`phrase`] — syllables at about four a second: a consonant (sibilant, fricative or
//!   stop burst, [`crate::pitch::testkit`]'s noise) then a vowel (a glottal pulse train
//!   through four formants, Peterson and Barney's averages) on a falling, inflected f0,
//!   with gaps between the words. Its spectral envelope moves the way speech's does —
//!   formant to formant, voiced to unvoiced, at the syllable rate — which is what a vocoder
//!   has to carry and what these measures listen for.
//! - [`spectral_correlation`] — how well the output's *spectral envelope* follows the
//!   voice's: per 21 ms frame, the Pearson correlation across third-octave bands (125 Hz …
//!   8 kHz) of the two spectra in dB, averaged over the frames where the voice is within
//!   40 dB of its loudest. 1: the same shape in every frame.
//! - [`ncm`] — the **normalised covariance measure** (Goldsworthy and Greenberg, *J. Acoust.
//!   Soc. Am.* 116, 2004; Ma, Hu and Loizou 2009), a speech-based relative of the speech
//!   transmission index (IEC 60268-16): in each of the STI's seven octave bands (125 Hz …
//!   8 kHz), the correlation `r` of the voice's and the output's intensity envelopes
//!   (modulations under 12.5 Hz — the syllables) is read as an apparent signal-to-noise
//!   ratio `10 log₁₀(r² / (1 − r²))`, clipped to ±15 dB, mapped to 0 … 1, and averaged with
//!   the STI's male-speech octave weights. 1: every band's envelope carried perfectly; a
//!   carrier that ignores the voice scores near 0. It is a proxy for intelligibility, not a
//!   listening test: it shows that the words' rhythm and spectral motion come through, and
//!   it ranks settings the way listeners rank them (more bands, consonants passed: higher).

use crate::aula_fft::Fft;
use crate::math;
use crate::pitch::testkit::{sibilant, voice, Noise, Vowel};

/// The STFT the measures use: 1024 points (21 ms at 48 kHz), hop 256, Hann.
pub const FRAME: usize = 1024;
pub const HOP: usize = 256;

/// More vowels (Peterson and Barney, adult male): /ɛ/ as in "bet", /ɔ/ as in "bought".
pub const E: Vowel = Vowel {
    formants: [
        (530.0, 80.0),
        (1_840.0, 100.0),
        (2_480.0, 170.0),
        (3_500.0, 250.0),
    ],
};
pub const O: Vowel = Vowel {
    formants: [
        (570.0, 80.0),
        (840.0, 90.0),
        (2_410.0, 170.0),
        (3_400.0, 250.0),
    ],
};

/// A consonant before a vowel.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Consonant {
    None,
    /// /s/: noise above 4 kHz.
    S,
    /// /f/, /h/: broadband noise, quieter.
    F,
    /// /t/, /k/: a short burst.
    T,
}

/// A speech-like phrase, `seconds` long (it repeats its words), peak-normalised to `peak`.
pub fn phrase(sr: f32, seconds: f32, peak: f32) -> Vec<f32> {
    use Consonant::*;
    let words: [&[(Consonant, Vowel, f32)]; 4] = [
        &[(S, Vowel::A, 0.20), (T, Vowel::I, 0.16), (None, O, 0.22)],
        &[(F, E, 0.18), (None, Vowel::U, 0.20)],
        &[(T, Vowel::A, 0.15), (S, Vowel::I, 0.18), (None, E, 0.25)],
        &[(None, O, 0.16), (S, Vowel::U, 0.22)],
    ];
    let n = (seconds * sr) as usize;
    let mut out = vec![0.0f32; n];
    let mut at = (0.05 * sr) as usize;
    let mut w = 0usize;
    let mut seed = 1u64;
    while at < n {
        let word = words[w % words.len()];
        let start = at;
        for (k, &(c, v, dur)) in word.iter().enumerate() {
            seed += 1;
            let cons = match c {
                None => Vec::new(),
                S => sibilant(sr, 0.09, 0.30, seed),
                F => broadband(sr, 0.08, 0.12, seed),
                T => broadband(sr, 0.025, 0.35, seed),
            };
            add_shaped(&mut out, &cons, at, sr, 0.008, 0.015);
            at += cons.len();
            // The f0 falls across the phrase and rises a little on each word's first vowel.
            let t0 = at as f32 / sr;
            let lift = if k == 0 { 12.0 } else { 0.0 };
            let f0 = move |t: f64| {
                let tt = t0 + t as f32;
                (135.0 - 4.0 * (tt % 8.0) + lift * (1.0 - (t as f32 / dur).min(1.0))).max(70.0)
            };
            let vow = voice(sr, dur, v, 0.9, f0);
            add_shaped(&mut out, &vow, at, sr, 0.02, 0.04);
            at += vow.len() - (0.02 * sr) as usize;
        }
        let _ = start;
        at += (0.18 * sr) as usize;
        w += 1;
    }
    crate::pitch::testkit::normalise(&mut out, peak);
    out
}

/// White noise, lightly low-passed (a fricative's or a stop's burst).
fn broadband(sr: f32, seconds: f32, peak: f32, seed: u64) -> Vec<f32> {
    let mut ns = Noise::new(seed ^ 0xF00D);
    let c = math::exp(-core::f32::consts::TAU * 6_000.0 / sr);
    let mut lp = 0.0f32;
    let mut v: Vec<f32> = (0..(seconds * sr) as usize)
        .map(|_| {
            let x = ns.next();
            lp = x + (lp - x) * c;
            lp
        })
        .collect();
    crate::pitch::testkit::normalise(&mut v, peak);
    v
}

/// `src` added into `dst` at `at`, faded in and out (seconds).
fn add_shaped(dst: &mut [f32], src: &[f32], at: usize, sr: f32, fade_in: f32, fade_out: f32) {
    let (fi, fo) = ((fade_in * sr) as usize, (fade_out * sr) as usize);
    let len = src.len();
    for (j, &x) in src.iter().enumerate() {
        let Some(d) = dst.get_mut(at + j) else { break };
        let mut g = 1.0f32;
        if j < fi {
            g = j as f32 / fi as f32;
        }
        if len - j < fo {
            g = g.min((len - j) as f32 / fo as f32);
        }
        *d += x * g;
    }
}

/// The power spectra of `x`'s frames (Hann, [`FRAME`] points, hop [`HOP`]).
pub fn power_frames(x: &[f32]) -> Vec<Vec<f32>> {
    let fft = Fft::new(FRAME);
    let win: Vec<f32> = (0..FRAME)
        .map(|i| 0.5 - 0.5 * math::cos(core::f32::consts::TAU * i as f32 / FRAME as f32))
        .collect();
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + FRAME <= x.len() {
        let mut re: Vec<f32> = (0..FRAME).map(|i| x[at + i] * win[i]).collect();
        let mut im = vec![0.0f32; FRAME];
        fft.run(&mut re, &mut im, false);
        out.push(
            (0..=FRAME / 2)
                .map(|k| re[k] * re[k] + im[k] * im[k])
                .collect(),
        );
        at += HOP;
    }
    out
}

/// Each band's energy in each frame: `[band][frame]`, for bands given as (low, high) Hz.
pub fn band_energies(frames: &[Vec<f32>], sr: f32, bands: &[(f32, f32)]) -> Vec<Vec<f64>> {
    let bin = sr / FRAME as f32;
    bands
        .iter()
        .map(|&(lo, hi)| {
            let (a, b) = (
                (lo / bin).ceil() as usize,
                ((hi / bin).floor() as usize).max(1),
            );
            frames
                .iter()
                .map(|f| {
                    f[a.min(f.len() - 1)..=b.min(f.len() - 1)]
                        .iter()
                        .map(|&p| f64::from(p))
                        .sum::<f64>()
                })
                .collect()
        })
        .collect()
}

/// Third-octave bands from 125 Hz to 8 kHz (19 bands).
pub fn third_octaves() -> Vec<(f32, f32)> {
    (0..19)
        .map(|i| {
            let c = 125.0 * math::exp2(i as f32 / 3.0);
            (c * math::exp2(-1.0 / 6.0), c * math::exp2(1.0 / 6.0))
        })
        .collect()
}

/// The STI's octave bands, 125 Hz … 8 kHz.
pub fn octaves() -> Vec<(f32, f32)> {
    (0..7)
        .map(|i| {
            let c = 125.0 * math::exp2(i as f32);
            (c * math::exp2(-0.5), c * math::exp2(0.5))
        })
        .collect()
}

/// IEC 60268-16's octave weights for male speech (the α factors), 125 Hz … 8 kHz.
pub const STI_WEIGHTS: [f64; 7] = [0.085, 0.127, 0.230, 0.233, 0.309, 0.224, 0.173];

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    if n < 2.0 {
        return 0.0;
    }
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    if saa <= 0.0 || sbb <= 0.0 {
        0.0
    } else {
        sab / (saa * sbb).sqrt()
    }
}

/// The spectral-envelope correlation of `test` with `reference` (see the module docs).
pub fn spectral_correlation(reference: &[f32], test: &[f32], sr: f32) -> f64 {
    let bands = third_octaves();
    let er = band_energies(&power_frames(reference), sr, &bands);
    let et = band_energies(&power_frames(test), sr, &bands);
    let frames = er[0].len().min(et[0].len());
    let total: Vec<f64> = (0..frames).map(|f| er.iter().map(|b| b[f]).sum()).collect();
    let loudest = total.iter().cloned().fold(0.0f64, f64::max);
    let db = |e: f64| 10.0 * math::log10(e + 1e-12);
    let (mut sum, mut count) = (0.0, 0usize);
    for f in 0..frames {
        if total[f] < loudest * 1e-4 {
            continue;
        }
        let a: Vec<f64> = er.iter().map(|b| db(b[f])).collect();
        let b: Vec<f64> = et.iter().map(|b| db(b[f])).collect();
        sum += pearson(&a, &b);
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

/// Each octave band's envelope correlation and the NCM (see the module docs).
pub fn ncm_bands(reference: &[f32], test: &[f32], sr: f32) -> ([f64; 7], f64) {
    let bands = octaves();
    let er = band_energies(&power_frames(reference), sr, &bands);
    let et = band_energies(&power_frames(test), sr, &bands);
    // The envelopes' modulations above 12.5 Hz are smoothed away (a one-pole at the
    // frame rate).
    let rate = sr / HOP as f32;
    let k = 1.0 - math::exp(-core::f64::consts::TAU * 12.5 / f64::from(rate));
    let smooth = |v: &[f64]| -> Vec<f64> {
        let mut s = 0.0;
        v.iter()
            .map(|&x| {
                s += (x - s) * k;
                s
            })
            .collect()
    };
    let mut r = [0.0f64; 7];
    let (mut num, mut den) = (0.0, 0.0);
    for b in 0..7 {
        let rb = pearson(&smooth(&er[b]), &smooth(&et[b]));
        r[b] = rb;
        let r2 = (rb * rb).min(0.999_999);
        let snr = if rb <= 0.0 {
            -15.0
        } else {
            (10.0 * math::log10(r2 / (1.0 - r2))).clamp(-15.0, 15.0)
        };
        num += STI_WEIGHTS[b] * (snr + 15.0) / 30.0;
        den += STI_WEIGHTS[b];
    }
    (r, num / den)
}

/// The normalised covariance measure, 0 … 1.
pub fn ncm(reference: &[f32], test: &[f32], sr: f32) -> f64 {
    ncm_bands(reference, test, sr).1
}

/// The RMS of `x`, dBFS.
pub fn rms_db(x: &[f32]) -> f32 {
    let e: f64 = x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>() / x.len() as f64;
    (10.0 * math::log10(e + 1e-30)) as f32
}

/// A band's response, dB, at `hz`: the Goertzel magnitude of an impulse response.
pub fn response_db(ir: &[f32], hz: f32, sr: f32) -> f32 {
    let w = core::f64::consts::TAU * f64::from(hz) / f64::from(sr);
    let (s, c) = math::sin_cos(w);
    let (mut re, mut im) = (0.0f64, 0.0f64);
    // A direct DFT at one frequency, rotating a phasor (exact enough over 2¹⁶ samples).
    let (mut pr, mut pi) = (1.0f64, 0.0f64);
    for &x in ir {
        re += f64::from(x) * pr;
        im -= f64::from(x) * pi;
        let npr = pr * c - pi * s;
        pi = pr * s + pi * c;
        pr = npr;
    }
    (10.0 * math::log10(re * re + im * im + 1e-300)) as f32
}

//! Loudness to ITU-R BS.1770-4 / EBU R128: K-weighting, momentary (400 ms), short-term
//! (3 s), integrated with the two gates, loudness range (EBU Tech 3342) and true peak.
//!
//! ## One code path for the meter and the editor
//!
//! Everything is built on **10 ms columns**: the K-weighted energy of each channel, summed
//! (every channel weighs 1, as BS.1770 gives left and right). [`ColumnLoudness`] turns a
//! stream of columns into momentary and short-term loudness and feeds the gates every
//! 100 ms (BS.1770's gating step). [`LoudnessMeter`] makes the columns from samples; the
//! limiter's editor gets the very same columns from the audio thread and runs the same
//! [`ColumnLoudness`] — so the conformance tests below check what the user reads.
//!
//! ## Gating without allocating
//!
//! The integrated measurement and the loudness range keep histograms in 0.01 LU bins from
//! −70 to +10 LUFS (8 000 bins, allocated once): each bin counts its blocks *and* sums their
//! exact energies, so the mean is exact and only the gate decision is rounded — to 0.005 LU.

use super::os::TruePeak;

/// Energy → LUFS. Silence is −∞.
#[inline]
pub fn lufs(energy: f64) -> f64 {
    if energy > 0.0 {
        -0.691 + 10.0 * energy.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// LUFS → energy.
#[inline]
pub fn energy_of(lufs: f64) -> f64 {
    10f64.powf((lufs + 0.691) / 10.0)
}

/// The absolute gate, LUFS.
pub const ABSOLUTE_GATE: f64 = -70.0;
/// The relative gate of the integrated measurement, LU below the absolute-gated level.
pub const RELATIVE_GATE: f64 = -10.0;
/// The relative gate of the loudness range, LU (EBU Tech 3342).
pub const RANGE_GATE: f64 = -20.0;

/// One biquad, transposed direct form II, in f64.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Section {
    pub b: [f64; 3],
    /// a1, a2 (a0 = 1).
    pub a: [f64; 2],
    z: [f64; 2],
}

impl Section {
    #[inline]
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        // Both at once: flushing one alone kicks the other, and the RLB high-pass (poles
        // near 1) then hovers at the threshold for ever instead of coming to rest (found by
        // the at-rest tests, ADR 0031 §5.1; Porta's sections do the same).
        if self.z[0].abs() < 1e-30 && self.z[1].abs() < 1e-30 {
            self.z = [0.0; 2];
        }
        y
    }
}

/// The K-weighting of BS.1770: a high shelf (+4 dB above ~1.7 kHz, the head) and the RLB
/// high-pass (~38 Hz). Designed from the analog prototype for any sample rate; at 48 kHz it
/// reproduces the standard's Tables 1 and 2 (the test holds it).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct KWeighting {
    pub shelf: Section,
    pub highpass: Section,
}

impl KWeighting {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate as f64;
        // Stage 1: the shelf.
        let f0 = 1681.974450955533;
        let g = 3.999843853973347;
        let q = 0.7071752369554196;
        let k = (std::f64::consts::PI * f0 / fs).tan();
        let vh = 10f64.powf(g / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let a0 = 1.0 + k / q + k * k;
        let shelf = Section {
            b: [
                (vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0,
            ],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [0.0; 2],
        };
        // Stage 2: the RLB high-pass.
        let f0 = 38.13547087602444;
        let q = 0.5003270373238773;
        let k = (std::f64::consts::PI * f0 / fs).tan();
        let a0 = 1.0 + k / q + k * k;
        let highpass = Section {
            b: [1.0, -2.0, 1.0],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [0.0; 2],
        };
        KWeighting { shelf, highpass }
    }

    pub fn reset(&mut self) {
        self.shelf.z = [0.0; 2];
        self.highpass.z = [0.0; 2];
    }

    /// Empty, as [`Self::reset`] leaves it (ADR 0031 §5.1).
    pub fn is_clear(&self) -> bool {
        crate::rest::all_zero_f64(&self.shelf.z) && crate::rest::all_zero_f64(&self.highpass.z)
    }

    #[inline]
    pub fn tick(&mut self, x: f64) -> f64 {
        self.highpass.tick(self.shelf.tick(x))
    }
}

const BINS: usize = 8_000;
const BIN_LO: f64 = -70.0;
const BINS_PER_LU: f64 = 100.0;

/// Blocks by loudness, in 0.01 LU bins: counts and exact energy sums.
#[derive(Clone, Debug)]
pub struct Histogram {
    count: Box<[u32]>,
    energy: Box<[f64]>,
    total: u64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Histogram {
    pub fn new() -> Self {
        Histogram {
            count: vec![0; BINS].into_boxed_slice(),
            energy: vec![0.0; BINS].into_boxed_slice(),
            total: 0,
        }
    }

    pub fn clear(&mut self) {
        self.count.iter_mut().for_each(|c| *c = 0);
        self.energy.iter_mut().for_each(|e| *e = 0.0);
        self.total = 0;
    }

    /// Add a block by its energy; blocks at or below the absolute gate are not kept.
    pub fn add(&mut self, energy: f64) {
        let l = lufs(energy);
        if l <= ABSOLUTE_GATE {
            return;
        }
        let i = (((l - BIN_LO) * BINS_PER_LU) as usize).min(BINS - 1);
        self.count[i] += 1;
        self.energy[i] += energy;
        self.total += 1;
    }

    /// Blocks kept (above the absolute gate).
    pub fn len(&self) -> u64 {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    fn centre(i: usize) -> f64 {
        BIN_LO + (i as f64 + 0.5) / BINS_PER_LU
    }

    /// The first bin whose centre is above `gate` LUFS.
    fn first_above(gate: f64) -> usize {
        let i = ((gate - BIN_LO) * BINS_PER_LU - 0.5).ceil();
        if i <= 0.0 {
            0
        } else {
            (i as usize).min(BINS)
        }
    }

    /// The mean energy of the blocks in bins from `from` up, and how many there were.
    fn mean_from(&self, from: usize) -> (f64, u64) {
        let mut e = 0.0;
        let mut n = 0u64;
        for i in from..BINS {
            e += self.energy[i];
            n += self.count[i] as u64;
        }
        if n == 0 {
            (0.0, 0)
        } else {
            (e / n as f64, n)
        }
    }

    /// Integrated loudness: the mean of the blocks above the absolute gate and above the
    /// relative gate (−10 LU under the absolute-gated mean). `None` before any block counts.
    pub fn integrated(&self) -> Option<f64> {
        let (mean, n) = self.mean_from(0);
        if n == 0 {
            return None;
        }
        let gate = lufs(mean) + RELATIVE_GATE;
        let (mean, n) = self.mean_from(Self::first_above(gate));
        (n > 0).then(|| lufs(mean))
    }

    /// Loudness range (EBU Tech 3342): the spread between the 10th and the 95th percentile
    /// of the blocks above the absolute gate and above a relative gate 20 LU under their
    /// mean. `None` before any block counts.
    pub fn range(&self) -> Option<f64> {
        let (mean, n) = self.mean_from(0);
        if n == 0 {
            return None;
        }
        let from = Self::first_above(lufs(mean) + RANGE_GATE);
        let n: u64 = self.count[from..].iter().map(|c| *c as u64).sum();
        if n == 0 {
            return None;
        }
        let rank = |p: f64| ((n - 1) as f64 * p).round() as u64;
        let (lo_rank, hi_rank) = (rank(0.10), rank(0.95));
        let (mut lo, mut hi) = (None, None);
        let mut seen = 0u64;
        for i in from..BINS {
            let c = self.count[i] as u64;
            if c == 0 {
                continue;
            }
            if lo.is_none() && seen + c > lo_rank {
                lo = Some(Self::centre(i));
            }
            if seen + c > hi_rank {
                hi = Some(Self::centre(i));
                break;
            }
            seen += c;
        }
        Some(hi? - lo?)
    }
}

/// Columns in the short-term window.
pub const SHORT_COLUMNS: usize = 300;
/// Columns in the momentary window.
pub const MOMENTARY_COLUMNS: usize = 40;
/// Columns per gating step (100 ms).
pub const STEP_COLUMNS: usize = 10;

/// Loudness from a stream of 10 ms columns of K-weighted energy (see the module notes).
#[derive(Clone, Debug)]
pub struct ColumnLoudness {
    ring: [f64; SHORT_COLUMNS],
    pos: usize,
    /// Columns seen since the last reset.
    seen: u64,
    /// Momentary and short-term loudness at the last column, LUFS (−∞ until they exist).
    pub momentary: f64,
    pub short_term: f64,
    pub max_momentary: f64,
    pub max_short_term: f64,
    /// Gating blocks (400 ms every 100 ms) for the integrated measurement.
    pub blocks: Histogram,
    /// Short-term values every 100 ms, for the loudness range.
    pub shorts: Histogram,
}

impl Default for ColumnLoudness {
    fn default() -> Self {
        Self::new()
    }
}

impl ColumnLoudness {
    pub fn new() -> Self {
        ColumnLoudness {
            ring: [0.0; SHORT_COLUMNS],
            pos: 0,
            seen: 0,
            momentary: f64::NEG_INFINITY,
            short_term: f64::NEG_INFINITY,
            max_momentary: f64::NEG_INFINITY,
            max_short_term: f64::NEG_INFINITY,
            blocks: Histogram::new(),
            shorts: Histogram::new(),
        }
    }

    pub fn reset(&mut self) {
        self.ring = [0.0; SHORT_COLUMNS];
        self.pos = 0;
        self.seen = 0;
        self.momentary = f64::NEG_INFINITY;
        self.short_term = f64::NEG_INFINITY;
        self.max_momentary = f64::NEG_INFINITY;
        self.max_short_term = f64::NEG_INFINITY;
        self.blocks.clear();
        self.shorts.clear();
    }

    fn mean_of_last(&self, n: usize) -> f64 {
        let mut s = 0.0;
        for k in 0..n {
            s += self.ring[(self.pos + SHORT_COLUMNS - 1 - k) % SHORT_COLUMNS];
        }
        s / n as f64
    }

    /// One column: the mean over its samples of the summed K-weighted squares.
    pub fn push(&mut self, energy: f64) {
        self.ring[self.pos] = energy.max(0.0);
        self.pos = (self.pos + 1) % SHORT_COLUMNS;
        self.seen += 1;
        let step = self.seen.is_multiple_of(STEP_COLUMNS as u64);
        if self.seen >= MOMENTARY_COLUMNS as u64 {
            let m = self.mean_of_last(MOMENTARY_COLUMNS);
            self.momentary = lufs(m);
            self.max_momentary = self.max_momentary.max(self.momentary);
            if step {
                self.blocks.add(m);
            }
        }
        if self.seen >= SHORT_COLUMNS as u64 {
            let s = self.mean_of_last(SHORT_COLUMNS);
            self.short_term = lufs(s);
            self.max_short_term = self.max_short_term.max(self.short_term);
            if step {
                self.shorts.add(s);
            }
        }
    }

    pub fn integrated(&self) -> Option<f64> {
        self.blocks.integrated()
    }

    pub fn range(&self) -> Option<f64> {
        self.shorts.range()
    }

    /// Seconds measured.
    pub fn seconds(&self) -> f64 {
        self.seen as f64 / 100.0
    }
}

/// Samples in one 10 ms column at a sample rate.
pub fn column_len(sample_rate: f32) -> usize {
    ((sample_rate as f64 / 100.0).round() as usize).max(1)
}

/// A complete stereo loudness meter: samples in, BS.1770-4 / R128 readings out.
#[derive(Clone, Debug)]
pub struct LoudnessMeter {
    k: [KWeighting; 2],
    col_len: usize,
    acc: f64,
    acc_n: usize,
    pub columns: ColumnLoudness,
    tp: [TruePeak; 2],
    /// The largest true peak so far, linear.
    pub max_true_peak: f32,
}

impl LoudnessMeter {
    pub fn new(sample_rate: f32) -> Self {
        LoudnessMeter {
            k: [KWeighting::new(sample_rate); 2],
            col_len: column_len(sample_rate),
            acc: 0.0,
            acc_n: 0,
            columns: ColumnLoudness::new(),
            tp: [TruePeak::new(), TruePeak::new()],
            max_true_peak: 0.0,
        }
    }

    pub fn process(&mut self, left: &[f32], right: &[f32]) {
        for (&l, &r) in left.iter().zip(right) {
            let kl = self.k[0].tick(l as f64);
            let kr = self.k[1].tick(r as f64);
            self.acc += kl * kl + kr * kr;
            self.acc_n += 1;
            if self.acc_n == self.col_len {
                self.columns.push(self.acc / self.col_len as f64);
                self.acc = 0.0;
                self.acc_n = 0;
            }
            self.max_true_peak = self
                .max_true_peak
                .max(self.tp[0].push(l))
                .max(self.tp[1].push(r));
        }
    }

    /// Mono: one channel, weighed as one channel (BS.1770 sums channels; a mono file is one).
    pub fn process_mono(&mut self, x: &[f32]) {
        for &v in x {
            let k = self.k[0].tick(v as f64);
            self.acc += k * k;
            self.acc_n += 1;
            if self.acc_n == self.col_len {
                self.columns.push(self.acc / self.col_len as f64);
                self.acc = 0.0;
                self.acc_n = 0;
            }
            self.max_true_peak = self.max_true_peak.max(self.tp[0].push(v));
        }
    }

    pub fn momentary(&self) -> f64 {
        self.columns.momentary
    }

    pub fn short_term(&self) -> f64 {
        self.columns.short_term
    }

    pub fn integrated(&self) -> Option<f64> {
        self.columns.integrated()
    }

    pub fn range(&self) -> Option<f64> {
        self.columns.range()
    }

    /// The largest true peak so far, dBTP.
    pub fn true_peak_db(&self) -> f32 {
        if self.max_true_peak > 0.0 {
            20.0 * self.max_true_peak.log10()
        } else {
            f32::NEG_INFINITY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const FS: f32 = 48_000.0;

    /// A stereo 1 kHz sine at `dbfs` peak, `secs` long, both channels.
    fn sine(dbfs: f64, secs: f64) -> Vec<f32> {
        let a = 10f64.powf(dbfs / 20.0);
        let n = (secs * FS as f64).round() as usize;
        (0..n)
            .map(|i| (a * (2.0 * PI * 1_000.0 * i as f64 / FS as f64).sin()) as f32)
            .collect()
    }

    fn measure(segments: &[(f64, f64)]) -> LoudnessMeter {
        let mut m = LoudnessMeter::new(FS);
        for &(db, secs) in segments {
            let x = sine(db, secs);
            m.process(&x, &x);
        }
        m
    }

    /// BS.1770-4 Tables 1 and 2: the published coefficients at 48 kHz.
    #[test]
    fn the_k_weighting_is_the_standards_at_48k() {
        let k = KWeighting::new(48_000.0);
        let shelf_b = [1.53512485958697, -2.69169618940638, 1.19839281085285];
        let shelf_a = [-1.69065929318241, 0.73248077421585];
        let hp_a = [-1.99004745483398, 0.99007225036621];
        for (i, (got, want)) in k.shelf.b.iter().zip(shelf_b).enumerate() {
            assert!((got - want).abs() < 1e-7, "shelf b{i}");
        }
        for i in 0..2 {
            assert!((k.shelf.a[i] - shelf_a[i]).abs() < 1e-7, "shelf a{}", i + 1);
            assert!((k.highpass.a[i] - hp_a[i]).abs() < 1e-7, "hp a{}", i + 1);
        }
        assert_eq!(k.highpass.b, [1.0, -2.0, 1.0]);
    }

    /// EBU Tech 3341 cases 1 and 2: a stereo 1 kHz sine at −23 (−33) dBFS reads −23 (−33)
    /// LUFS momentary, short-term and integrated, ±0.1 LU. Both channels count: the same
    /// sine in one channel alone reads 3 dB less.
    #[test]
    fn tech_3341_cases_1_and_2_a_stereo_sine_reads_its_level() {
        for db in [-23.0, -33.0] {
            let m = measure(&[(db, 20.0)]);
            let i = m.integrated().unwrap();
            eprintln!(
                "3341 sine {db}: I {i:.3} M {:.3} S {:.3}",
                m.momentary(),
                m.short_term()
            );
            assert!((i - db).abs() < 0.1, "I = {i} for {db}");
            assert!((m.momentary() - db).abs() < 0.1, "M = {}", m.momentary());
            assert!((m.short_term() - db).abs() < 0.1, "S = {}", m.short_term());
        }
        let mut mono = LoudnessMeter::new(FS);
        mono.process_mono(&sine(-23.0, 20.0));
        let i = mono.integrated().unwrap();
        eprintln!("3341 one channel: I {i:.3}");
        assert!((i - -26.01).abs() < 0.1, "one channel alone: {i}");
    }

    /// Cases 3 and 4: the relative gate removes the quiet sections (−36 dBFS, 13 LU under),
    /// the absolute gate the near-silent ones (−72 dBFS).
    #[test]
    fn tech_3341_cases_3_and_4_the_gates_remove_what_they_should() {
        let m = measure(&[(-36.0, 10.0), (-23.0, 60.0), (-36.0, 10.0)]);
        eprintln!("3341 case 3: I {:.3}", m.integrated().unwrap());
        assert!((m.integrated().unwrap() - -23.0).abs() < 0.1);
        let m = measure(&[
            (-72.0, 10.0),
            (-36.0, 10.0),
            (-23.0, 60.0),
            (-36.0, 10.0),
            (-72.0, 10.0),
        ]);
        eprintln!("3341 case 4: I {:.3}", m.integrated().unwrap());
        assert!((m.integrated().unwrap() - -23.0).abs() < 0.1);
    }

    /// Case 5: −26, −20, −26 dBFS; nothing is gated out and the energy mean is −23.0.
    #[test]
    fn tech_3341_case_5_the_energy_mean() {
        let m = measure(&[(-26.0, 20.0), (-20.0, 20.1), (-26.0, 20.0)]);
        let i = m.integrated().unwrap();
        eprintln!("3341 case 5: I {i:.3}");
        assert!((i - -23.0).abs() < 0.1, "I = {i}");
    }

    /// EBU Tech 3342 cases 1–4: loudness range, ±1 LU.
    #[test]
    fn tech_3342_cases_1_to_4_loudness_range() {
        let cases: [(&[(f64, f64)], f64); 4] = [
            (&[(-20.0, 20.0), (-30.0, 20.0)], 10.0),
            (&[(-20.0, 20.0), (-15.0, 20.0)], 5.0),
            (&[(-40.0, 20.0), (-20.0, 20.0)], 20.0),
            (
                &[
                    (-50.0, 20.0),
                    (-35.0, 20.0),
                    (-20.0, 20.0),
                    (-35.0, 20.0),
                    (-50.0, 20.0),
                ],
                15.0,
            ),
        ];
        for (segments, want) in cases {
            let lra = measure(segments).range().unwrap();
            eprintln!("3342 LRA {lra:.3} (want {want})");
            assert!((lra - want).abs() < 1.0, "LRA {lra} for {want}");
        }
    }

    /// True peak (EBU Tech 3341 tolerance +0.2 / −0.4 dB): a 0 dBFS sine at fs/4 whose
    /// samples sit at ±45° reads 0 dBTP though every sample is −3 dB.
    #[test]
    fn true_peak_reads_between_the_samples() {
        let x: Vec<f32> = (0..48_000)
            .map(|n| (PI / 2.0 * n as f64 + PI / 4.0).sin() as f32)
            .collect();
        // Faded in and out: a tone switched on abruptly overshoots between its first samples.
        let x = crate::terminus::os::tests::faded(x, 4_800);
        let mut m = LoudnessMeter::new(FS);
        m.process(&x, &x);
        let tp = m.true_peak_db();
        eprintln!("true peak of the 45 degree quarter-rate sine: {tp:.4} dBTP");
        assert!((-0.4..=0.2).contains(&tp), "TP {tp}");
        assert!(tp.abs() < 0.02, "and much closer than the tolerance: {tp}");
    }

    #[test]
    fn silence_is_minus_infinity_and_ungated() {
        let mut m = LoudnessMeter::new(FS);
        let z = vec![0.0f32; 96_000];
        m.process(&z, &z);
        assert_eq!(m.integrated(), None);
        assert_eq!(m.momentary(), f64::NEG_INFINITY);
        assert_eq!(m.range(), None);
    }
}

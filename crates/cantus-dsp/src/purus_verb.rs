//! Purus's de-reverb: the room's late tail taken out of dialogue or a close microphone, in
//! a streaming short-time Fourier transform of about 21 ms.
//!
//! **The transform** is the audio editor's (ADR 0096, `denoise::stft`) made streaming: the
//! same square-root-Hann window (`denoise::stft::sqrt_hann`, shared) at 75 % overlap, and
//! the same radix-2 transform (`aequalis::fft`), 1024 points at 44.1–48 kHz (twice that
//! at 88.2–96, four times at 176.4–192), so a frame is about 21 ms at every rate. Each hop
//! the newest frame is analysed, given its gains and overlap-added; a sample is final once
//! the last frame over it has been added, `N − 1` samples after it came in — **the
//! latency, fixed**. The two channels share one set of gains (from their mean power), so
//! the image never wanders.
//!
//! **The model** is Habets's generalisation of Lebart's (Lebart, Boucher & Denbigh 2001;
//! Habets 2007, *Single- and multi-microphone speech dereverberation using spectral
//! enhancement*, ch. 3): a room whose reverberant energy decays as `e^{−2δt}`,
//! `δ = 3·ln 10 / T60`, excited by a direct sound `DRR` times stronger than all its
//! reverberation. Per bin, frame to frame (hop `R` samples, `a = e^{−2δR/fs}`):
//!
//! ```text
//! λ_rev(n)  = (1 − κ)·a·λ_rev(n−1) + κ·a·λ_y(n−1),   κ = (1 − a) / (a · DRR)  (≤ 1)
//! λ_late(n) = a^(N_e − 1) · λ_rev(n − N_e + 1)        (N_e frames = the early part kept)
//! ```
//!
//! `λ_y` the observed power, lightly smoothed. `κ = 1` is Lebart's model (no direct sound:
//! everything heard is assumed to decay); the direct-sound setting keeps a sustained note
//! from being taken for its own reverberation. **The gain** is the Wiener gain on the
//! decision-directed a-priori ratio (Ephraim & Malah 1984), `ξ = α·|Ŝ(n−1)|²/λ_late +
//! (1 − α)·max(γ − 1, 0)`, `G = ξ/(1 + ξ)`, never under the amount's floor, then smoothed
//! over three bins and, falling, over time — the editor's denoiser's smoothing
//! (`denoise::smooth_gains`, shared). Both are what keep musical noise down.
//!
//! **The decay time** is set, or **estimated** from the signal (after Löllmann & Vary's
//! idea of measuring only where the sound decays freely): the frame energy from 300 Hz to
//! 5 kHz is watched for runs where it only falls (each frame within 0.5 dB of falling),
//! at least 80 ms long and 10 dB deep; a least-squares line through each run's level (its
//! first two frames skipped — the direct sound's own end) gives a T60; the estimate is the
//! median of the last sixteen. Until the first, the decay set is used.
//!
//! What the editor's denoiser does *not* share: its noise model (a learned, stationary
//! profile — a reverb's is predicted frame by frame from the signal itself) and its
//! offline transform (it allocates per run; this one allocates only in `new`).

use crate::aequalis::fft::Fft;
use crate::denoise::{smooth_gains, stft::sqrt_hann};
use crate::math;

/// The most early part kept, ms.
pub const MAX_KEEP_MS: f32 = 200.0;
/// A frame whose every sample is under this (−180 dBFS) counts toward the drain.
pub const QUIET: f32 = 1e-9;
/// Estimates the decay's median is taken over.
const ESTIMATES: usize = 16;

/// Where the decay time comes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DecayMode {
    /// The decay set.
    Set,
    /// Measured from the signal's free decays (the decay set until the first).
    Estimate,
}

/// The de-reverb module's settings (real units).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VerbSettings {
    pub on: bool,
    /// The most any bin is turned down, dB, 0–30.
    pub amount_db: f32,
    pub decay: DecayMode,
    /// The room's T60, s, 0.1–10 (Set; Estimate's starting point).
    pub decay_s: f32,
    /// The direct-to-reverberant ratio assumed, dB, −10…+20: higher for a close microphone.
    /// Default +6 dB (dialogue first: a dry voice keeps its level within 1 dB); a far
    /// microphone's presets set 0 dB.
    pub direct_db: f32,
    /// 0–1: the gains' smoothing (and the decision-directed weight).
    pub smoothing: f32,
    /// The early part kept (reflections), ms, 5–200: the tail is taken from there on.
    pub keep_ms: f32,
    pub mix: f32,
    pub out_db: f32,
    pub delta: bool,
}

impl Default for VerbSettings {
    fn default() -> Self {
        VerbSettings {
            on: false,
            amount_db: 12.0,
            decay: DecayMode::Set,
            decay_s: 0.8,
            direct_db: 6.0,
            smoothing: 0.5,
            keep_ms: 50.0,
            mix: 1.0,
            out_db: 0.0,
            delta: false,
        }
    }
}

impl VerbSettings {
    /// Clamped into range.
    pub fn sane(&self) -> VerbSettings {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        VerbSettings {
            amount_db: f(self.amount_db, 0.0, 30.0, 12.0),
            decay_s: f(self.decay_s, 0.1, 10.0, 0.8),
            direct_db: f(self.direct_db, -10.0, 20.0, 6.0),
            smoothing: f(self.smoothing, 0.0, 1.0, 0.5),
            keep_ms: f(self.keep_ms, 5.0, MAX_KEEP_MS, 50.0),
            mix: f(self.mix, 0.0, 1.0, 1.0),
            out_db: f(self.out_db, -24.0, 24.0, 0.0),
            ..*self
        }
    }
}

/// The transform's size at a sample rate: about 21 ms.
pub fn size_for(sample_rate: f32) -> usize {
    let r = (sample_rate / 48_000.0).max(1e-3);
    let oct = math::log2(r).round() as i32;
    (1024.0 * math::exp2(oct as f32))
        .round()
        .clamp(256.0, 4096.0) as usize
}

/// The module's latency at a sample rate, samples.
pub fn latency_for(sample_rate: f32) -> usize {
    size_for(sample_rate) - 1
}

/// The free-decay estimator (see the module's header).
#[derive(Clone, Debug)]
struct DecayEstimate {
    frame_s: f32,
    lo: usize,
    hi: usize,
    /// The current run's levels (dB), up to [`RUN_FRAMES`], and the lowest so far.
    run: [f32; RUN_FRAMES],
    len: usize,
    low: f32,
    smooth: f32,
    k: f32,
    found: [f32; ESTIMATES],
    count: usize,
    next: usize,
    median: f32,
}

/// The longest run kept, frames (about 1.4 s at 48 kHz).
const RUN_FRAMES: usize = 256;
/// A run goes on while no frame is this far above the lowest of the run, dB.
const RUN_RISE_DB: f32 = 2.0;
/// The start of each run not fitted (the direct sound's end and the early reflections), s.
const FIT_SKIP_S: f32 = 0.06;
/// What a run must give after that: a length, s, and a fall, dB.
const FIT_MIN_S: f32 = 0.08;
const FIT_MIN_DB: f32 = 6.0;

impl DecayEstimate {
    fn new(fs: f32, n: usize, hop: usize) -> Self {
        let bin_hz = fs / n as f32;
        DecayEstimate {
            frame_s: hop as f32 / fs,
            lo: (300.0 / bin_hz).round() as usize,
            hi: ((5_000.0 / bin_hz).round() as usize).min(n / 2),
            run: [0.0; RUN_FRAMES],
            len: 0,
            low: 0.0,
            smooth: 0.0,
            k: 1.0 - math::exp(-(hop as f32 / fs) / 0.015),
            found: [0.0; ESTIMATES],
            count: 0,
            next: 0,
            median: 0.0,
        }
    }

    fn clear_run(&mut self) {
        self.len = 0;
        self.smooth = 0.0;
    }

    /// The estimate, s (0 until the first).
    fn value(&self) -> f32 {
        if self.count == 0 {
            0.0
        } else {
            self.median
        }
    }

    fn push(&mut self, power: &[f32]) {
        let e: f32 = power[self.lo..=self.hi].iter().sum();
        if e < 1e-12 {
            // Into silence: what fell so far is a run, if it is one.
            self.close();
            self.len = 0;
            self.smooth = 0.0;
            return;
        }
        // A light smoothing of the frame energy (about 15 ms): a decaying tail's own
        // fluctuations must not break its run.
        self.smooth = if self.len == 0 && self.smooth <= 0.0 {
            e
        } else {
            self.smooth + (e - self.smooth) * self.k
        };
        let db = 10.0 * math::log10(self.smooth);
        if self.len > 0 && db < self.low + RUN_RISE_DB && self.len < RUN_FRAMES {
            self.run[self.len] = db;
            self.len += 1;
            self.low = self.low.min(db);
            return;
        }
        if self.len > 0 {
            self.close();
        }
        self.run[0] = db;
        self.len = 1;
        self.low = db;
    }

    /// A run has ended: fit it, if it is long and deep enough.
    fn close(&mut self) {
        let skip = (FIT_SKIP_S / self.frame_s).round() as usize;
        let min = (FIT_MIN_S / self.frame_s).ceil() as usize;
        // The last frames, near the floor the run ended on, are left out too.
        let end = self.len.saturating_sub(3);
        if end < skip + min {
            return;
        }
        let pts = &self.run[skip..end];
        if pts[0] - pts[pts.len() - 1] < FIT_MIN_DB {
            return;
        }
        let m = pts.len() as f32;
        let (mut sx, mut sy, mut sxx, mut sxy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for (i, y) in pts.iter().enumerate() {
            let x = i as f32 * self.frame_s;
            sx += x;
            sy += y;
            sxx += x * x;
            sxy += x * y;
        }
        let den = m * sxx - sx * sx;
        if den <= 0.0 {
            return;
        }
        let slope = (m * sxy - sx * sy) / den;
        if slope >= 0.0 {
            return;
        }
        let t60 = -60.0 / slope;
        if !(0.1..=10.0).contains(&t60) {
            return;
        }
        self.found[self.next] = t60;
        self.next = (self.next + 1) % ESTIMATES;
        self.count = (self.count + 1).min(ESTIMATES);
        let mut sorted = [0.0f32; ESTIMATES];
        sorted[..self.count].copy_from_slice(&self.found[..self.count]);
        let s = &mut sorted[..self.count];
        for i in 1..s.len() {
            let mut j = i;
            while j > 0 && s[j - 1] > s[j] {
                s.swap(j - 1, j);
                j -= 1;
            }
        }
        let c = s.len();
        self.median = if c % 2 == 1 {
            s[c / 2]
        } else {
            0.5 * (s[c / 2 - 1] + s[c / 2])
        };
    }
}

/// The de-reverb module.
#[derive(Clone, Debug)]
pub struct Dereverb {
    fs: f32,
    s: VerbSettings,
    n: usize,
    hop: usize,
    bins: usize,
    fft: Fft,
    window: Vec<f32>,
    /// The input (the last `n` samples) and the overlap-add, per channel, indexed by the
    /// sample number modulo `n`.
    ring: [Vec<f32>; 2],
    ola: [Vec<f32>; 2],
    pos: usize,
    phase: usize,
    re: [Vec<f32>; 2],
    im: [Vec<f32>; 2],
    /// The model, per bin: the observed power (smoothed), the reverberant power's history
    /// (a ring of frames), the last frame's estimate of the clean power, the gains.
    lam_y: Vec<f32>,
    lam_rev: Vec<f32>,
    rev_frames: usize,
    rev_at: usize,
    clean_prev: Vec<f32>,
    raw: Vec<f32>,
    prev: Vec<f32>,
    gain: Vec<f32>,
    power: Vec<f32>,
    est: DecayEstimate,
    /// Frames in a row under [`QUIET`] (the drain asks).
    quiet_frames: usize,
    /// Measurement only: the gains from the left channel alone, so the right carries any
    /// signal through the left's gains (a component of a mixture, for its own share).
    shadow: bool,
}

impl Dereverb {
    pub fn new(sample_rate: f32) -> Self {
        let n = size_for(sample_rate);
        let hop = n / 4;
        let bins = n / 2 + 1;
        let frames_per_s = sample_rate / hop as f32;
        let rev_frames = (MAX_KEEP_MS * 0.001 * frames_per_s).ceil() as usize + 2;
        Dereverb {
            fs: sample_rate,
            s: VerbSettings::default(),
            n,
            hop,
            bins,
            fft: Fft::new(n),
            window: sqrt_hann(n),
            ring: [vec![0.0; n], vec![0.0; n]],
            ola: [vec![0.0; n], vec![0.0; n]],
            pos: 0,
            phase: 0,
            re: [vec![0.0; n], vec![0.0; n]],
            im: [vec![0.0; n], vec![0.0; n]],
            lam_y: vec![0.0; bins],
            lam_rev: vec![0.0; bins * rev_frames],
            rev_frames,
            rev_at: 0,
            clean_prev: vec![0.0; bins],
            raw: vec![1.0; bins],
            prev: vec![1.0; bins],
            gain: vec![1.0; bins],
            power: vec![0.0; bins],
            est: DecayEstimate::new(sample_rate, n, hop),
            quiet_frames: usize::MAX,
            shadow: false,
        }
    }

    pub fn set(&mut self, s: &VerbSettings) {
        self.s = s.sane();
    }

    /// Measurement only: compute the gains from the left channel alone and apply them to
    /// both, so a component of the left's mixture can be sent on the right and measured
    /// as the mixture's processing treats it.
    #[doc(hidden)]
    pub fn set_shadow(&mut self, on: bool) {
        self.shadow = on;
    }

    pub fn settings(&self) -> &VerbSettings {
        &self.s
    }

    /// Samples of delay, fixed for the sample rate.
    pub fn latency(&self) -> usize {
        self.n - 1
    }

    /// The transform's size.
    pub fn size(&self) -> usize {
        self.n
    }

    /// The frequency of bin `b`, Hz.
    pub fn bin_hz(&self, b: usize) -> f32 {
        b as f32 * self.fs / self.n as f32
    }

    /// The gain each bin was given in the last frame (linear; 1 = untouched).
    pub fn gains(&self) -> &[f32] {
        &self.gain
    }

    /// The decay time in force, s: the one set, or the estimate once there is one.
    pub fn decay_s(&self) -> f32 {
        match self.s.decay {
            DecayMode::Set => self.s.decay_s,
            DecayMode::Estimate => {
                let e = self.est.value();
                if e > 0.0 {
                    e
                } else {
                    self.s.decay_s
                }
            }
        }
    }

    /// The decay estimated from the signal so far, s (0 before the first free decay).
    pub fn estimated_decay_s(&self) -> f32 {
        self.est.value()
    }

    /// Everything to a new unit's state, **keeping the decay estimated** (what was learned
    /// of the room, as the editor's denoiser keeps its profile).
    pub fn reset(&mut self) {
        for c in 0..2 {
            self.ring[c].fill(0.0);
            self.ola[c].fill(0.0);
        }
        self.pos = 0;
        self.phase = 0;
        self.lam_y.fill(0.0);
        self.lam_rev.fill(0.0);
        self.rev_at = 0;
        self.clean_prev.fill(0.0);
        self.raw.fill(1.0);
        self.prev.fill(1.0);
        self.gain.fill(1.0);
        self.power.fill(0.0);
        self.est.clear_run();
        self.quiet_frames = usize::MAX;
    }

    /// Forget the decay estimated too (a new unit).
    pub fn forget(&mut self) {
        self.est = DecayEstimate::new(self.fs, self.n, self.hop);
    }

    /// Whether the last four frames (every frame a sample still to come out was in) were
    /// under [`QUIET`]: what is left to come out is under it too, and the unit's drain may
    /// empty the module.
    pub fn drained(&self) -> bool {
        self.quiet_frames >= 4
    }

    /// One stereo sample in; out, the processed sample and the input `latency()` ago.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> ((f32, f32), (f32, f32)) {
        let mask = self.n - 1;
        self.ring[0][self.pos] = l;
        self.ring[1][self.pos] = r;
        self.phase += 1;
        if self.phase == self.hop {
            self.phase = 0;
            self.frame();
        }
        let rp = (self.pos + 1) & mask;
        let wet = (self.ola[0][rp], self.ola[1][rp]);
        self.ola[0][rp] = 0.0;
        self.ola[1][rp] = 0.0;
        let dry = (self.ring[0][rp], self.ring[1][rp]);
        self.pos = rp;
        (wet, dry)
    }

    // The harmonics' (bins') arrays run in step: an index reads clearest.
    #[allow(clippy::needless_range_loop)]
    fn frame(&mut self) {
        let n = self.n;
        let mask = n - 1;
        let start = (self.pos + 1) & mask;
        let mut sound = false;
        let mut peak = 0.0f32;
        for c in 0..2 {
            for i in 0..n {
                let v = self.ring[c][(start + i) & mask];
                sound |= v != 0.0;
                peak = peak.max(v.abs());
                self.re[c][i] = v * self.window[i];
                self.im[c][i] = 0.0;
            }
        }
        // Frames under −180 dBFS count toward the drain (a de-hum's ring before it can come
        // to the very end of its decay).
        if peak < QUIET {
            self.quiet_frames = self.quiet_frames.saturating_add(1);
        } else {
            self.quiet_frames = 0;
        }
        if !sound {
            // A frame of silence transforms to exact zeros: nothing to add; the model
            // still decays as it would on zeros.
            self.power.fill(0.0);
            self.model();
            return;
        }
        for c in 0..2 {
            self.fft.run(&mut self.re[c], &mut self.im[c], false);
        }
        for b in 0..self.bins {
            let (lr, li) = (self.re[0][b], self.im[0][b]);
            let (rr, ri) = (self.re[1][b], self.im[1][b]);
            self.power[b] = if self.shadow {
                lr * lr + li * li
            } else {
                0.5 * (lr * lr + li * li + rr * rr + ri * ri)
            };
        }
        self.model();
        let half = n / 2;
        for c in 0..2 {
            let (re, im) = (&mut self.re[c], &mut self.im[c]);
            for b in 0..self.bins {
                re[b] *= self.gain[b];
                im[b] *= self.gain[b];
            }
            for b in 1..half {
                re[n - b] = re[b];
                im[n - b] = -im[b];
            }
            im[0] = 0.0;
            im[half] = 0.0;
            self.fft.run(re, im, true);
            let scale = 0.5 / n as f32;
            for i in 0..n {
                let at = (start + i) & mask;
                self.ola[c][at] += re[i] * self.window[i] * scale;
            }
        }
    }

    /// The model's step for this frame's `power`: the late reverberation predicted, the gains.
    fn model(&mut self) {
        let s = self.s;
        self.est.push(&self.power);
        let t60 = self.decay_s().max(0.05);
        let delta = 3.0 * std::f32::consts::LN_10 / t60;
        let a = math::exp(-2.0 * delta * self.hop as f32 / self.fs);
        let drr = math::db_to_gain(s.direct_db) * math::db_to_gain(s.direct_db);
        let kappa = ((1.0 - a) / (a * drr)).min(1.0);
        let frames_per_ms = self.fs / self.hop as f32 * 0.001;
        let ne = ((s.keep_ms * frames_per_ms).round() as usize).clamp(1, self.rev_frames - 1);
        let decay_e = math::powi(a, ne as i32 - 1);
        let bins = self.bins;
        // The reverberant power's recursion, into the ring's next slot.
        let last = self.rev_at;
        let next = (self.rev_at + 1) % self.rev_frames;
        for b in 0..bins {
            let prev = self.lam_rev[last * bins + b];
            self.lam_rev[next * bins + b] = (1.0 - kappa) * a * prev + kappa * a * self.lam_y[b];
        }
        self.rev_at = next;
        let from = (next + self.rev_frames - (ne - 1)) % self.rev_frames;
        let floor = math::db_to_gain(-s.amount_db);
        // The amount also leans on the prediction: up to twice the tail at 24 dB.
        let over = 1.0 + s.amount_db / 24.0;
        let dd = 0.85 + 0.13 * s.smoothing;
        for b in 0..bins {
            let p = self.power[b];
            let late = over * decay_e * self.lam_rev[from * bins + b];
            let g = if late > 1e-24 && p > 0.0 {
                let gamma = p / late;
                let xi = dd * self.clean_prev[b] / late + (1.0 - dd) * (gamma - 1.0).max(0.0);
                xi / (1.0 + xi)
            } else {
                1.0
            };
            self.raw[b] = g.max(floor);
            // The observed power, lightly smoothed (for the next frame's recursion).
            self.lam_y[b] = 0.5 * self.lam_y[b] + 0.5 * p;
        }
        let gain = &mut self.gain;
        smooth_gains(&self.raw, &mut self.prev, s.smoothing, |b, g| gain[b] = g);
        for b in 0..bins {
            self.clean_prev[b] = self.gain[b] * self.gain[b] * self.power[b];
        }
    }
}

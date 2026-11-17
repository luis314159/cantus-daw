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
//! **The room's share is measured** (`Share`): the late tail predicted is taken out only as
//! far as the signal's own free decays show a tail is there — none in a dry voice, which
//! then passes untouched whatever the Direct sound (the owner's null standard), the whole
//! prediction in a room.
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
    /// Default 0 dB: since the room's share is measured (`Share`), a dry voice passes
    /// untouched at 0 dB too (the owner's null standard, 2026-11-17).
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
            direct_db: 0.0,
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
            direct_db: f(self.direct_db, -10.0, 20.0, 0.0),
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

/// Octave bands the room's share is measured in (63 Hz … 16 kHz and above).
const SHARE_BANDS: usize = 10;
/// A band's frames count once it has fallen this long without rising 3 dB (the direct
/// sound's own end and the early part are not the room's tail), at least.
const SHARE_MIN_FRAMES: usize = 4;
/// The presence (the share of decay frames where the predicted tail is heard) under which
/// no room is taken out, and over which the whole predicted tail is.
const SHARE_FROM: f32 = 0.6;
const SHARE_TO: f32 = 0.75;
/// The most of the predicted tail taken where the evidence is strongest (a room whose tail
/// is plainly there: the prediction leans a little further, as the amount's does).
const SHARE_MOST: f32 = 1.2;
/// The decay frames of no tail the evidence starts with (about 0.1 s of decays at 48 kHz).
const SHARE_PRIOR: f64 = 20.0;
/// How far under its start a decay must have fallen to count (−6 dB, power).
const SHARE_FALL: f32 = 0.25;
/// The measure's memory, s of counted frames.
const SHARE_MEMORY_S: f32 = 2.0;

/// **The room's share, measured** (phase B, the owner's null standard, 2026-11-17): how
/// much of the late tail the model predicts is taken out, from the evidence that a tail is
/// there at all. In each octave band's free decays — once a band has fallen for the early
/// part kept without rising 3 dB, and lies 6 dB under where it began (a held sound is no
/// decay) — a frame where at least half the predicted tail is heard counts for a room, one
/// where less is heard counts against it. The **presence** is the share of such frames
/// over about two seconds of decays (the octaves 125 Hz … 8 kHz pooled: a room is
/// broadband), counted from a prior of [`SHARE_PRIOR`] frames that showed none. A room's
/// tails are heard where predicted (presence 0.75–0.92 on the measured scenes); a dry
/// voice's decays fall to nothing long before the prediction does (0.36–0.48 on a real dry
/// voice). Under [`SHARE_FROM`] nothing is taken — the gains are 1 and the input passes
/// untouched (the transform's round trip, within 2·10⁻⁶) — and from there to [`SHARE_TO`]
/// the share rises to the whole prediction, to [`SHARE_MOST`] where a tail is plainest.
#[derive(Clone, Debug)]
struct Share {
    run_min: [f32; SHARE_BANDS],
    run_top: [f32; SHARE_BANDS],
    run_len: [usize; SHARE_BANDS],
    heard: [f64; SHARE_BANDS],
    predicted: [f64; SHARE_BANDS],
    share: [f32; SHARE_BANDS],
    presence: f32,
    keep: f64,
}

impl Share {
    fn new(fs: f32, hop: usize) -> Self {
        let frames = (SHARE_MEMORY_S * fs / hop as f32).max(1.0);
        Share {
            run_min: [0.0; SHARE_BANDS],
            run_top: [0.0; SHARE_BANDS],
            run_len: [0; SHARE_BANDS],
            heard: [0.0; SHARE_BANDS],
            predicted: [0.0; SHARE_BANDS],
            share: [0.0; SHARE_BANDS],
            presence: 0.0,
            keep: 1.0 - 1.0 / f64::from(frames),
        }
    }

    fn clear(&mut self) {
        let keep = self.keep;
        *self = Share {
            keep,
            ..Share {
                run_min: [0.0; SHARE_BANDS],
                run_top: [0.0; SHARE_BANDS],
                run_len: [0; SHARE_BANDS],
                heard: [0.0; SHARE_BANDS],
                predicted: [0.0; SHARE_BANDS],
                share: [0.0; SHARE_BANDS],
                presence: 0.0,
                keep,
            }
        };
    }

    /// One frame: each band's power heard and late power predicted; `early` the frames a
    /// decay must have run before it counts.
    fn push(&mut self, heard: &[f32; SHARE_BANDS], late: &[f32; SHARE_BANDS], early: usize) {
        let need = early.max(SHARE_MIN_FRAMES);
        for j in 0..SHARE_BANDS {
            let p = heard[j];
            if p > 2.0 * self.run_min[j] || self.run_len[j] == 0 {
                // A rise (or the first frame): a new decay may start here.
                self.run_min[j] = p;
                self.run_top[j] = p;
                self.run_len[j] = 1;
            } else {
                self.run_min[j] = self.run_min[j].min(p);
                self.run_len[j] += 1;
            }
            // A decay, not a held sound: 6 dB under where the run began.
            if self.run_len[j] > need && p <= SHARE_FALL * self.run_top[j] && late[j] > 1e-20 {
                // Is the tail there? A frame where at least half the predicted tail is heard
                // says yes; the share of such frames is the evidence (`presence`).
                let hit = if p >= 0.5 * late[j] { 1.0 } else { 0.0 };
                self.heard[j] = self.keep * self.heard[j] + hit;
                self.predicted[j] = self.keep * self.predicted[j] + 1.0;
            }
        }
        // The room is broadband: the evidence of the octaves 125 Hz … 8 kHz pooled (each
        // octave by the frames it counted), so one band's few bins do not decide alone.
        let (mut hits, mut frames) = (0.0f64, 0.0f64);
        for j in 1..=7 {
            hits += self.heard[j];
            frames += self.predicted[j];
        }
        // Until there is evidence, no room: the count starts as if [`SHARE_PRIOR`] decay
        // frames had shown no tail, so a few early frames cannot decide alone.
        self.presence = (hits / (frames + SHARE_PRIOR)) as f32;
        let sh = ((self.presence - SHARE_FROM) / (SHARE_TO - SHARE_FROM)).clamp(0.0, SHARE_MOST);
        self.share = [sh; SHARE_BANDS];
    }
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
    /// The room's share measured, and each bin's octave band.
    share: Share,
    band_of: Vec<u8>,
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
            share: Share::new(sample_rate, hop),
            band_of: (0..bins)
                .map(|b| {
                    let hz = b as f32 * sample_rate / n as f32;
                    if hz < 88.4 {
                        0
                    } else {
                        ((math::log2(hz / 88.4)).floor() as usize + 1).min(SHARE_BANDS - 1) as u8
                    }
                })
                .collect(),
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

    /// The share of the predicted tail taken now (0: no room evidenced, the input passes
    /// untouched; 1: the model's whole tail; up to 1.2), and the evidence behind it.
    pub fn room_share(&self) -> (f32, f32) {
        (self.share.share[1], self.share.presence)
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
        self.share.clear();
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
        // The room's share, measured in each octave's free decays (`Share`).
        let mut heard = [0.0f32; SHARE_BANDS];
        let mut predicted = [0.0f32; SHARE_BANDS];
        for b in 0..bins {
            let j = usize::from(self.band_of[b]);
            heard[j] += self.power[b];
            predicted[j] += decay_e * self.lam_rev[from * bins + b];
        }
        self.share.push(&heard, &predicted, ne);
        // The amount also leans on the prediction: up to twice the tail at 24 dB.
        let over = 1.0 + s.amount_db / 24.0;
        let dd = 0.85 + 0.13 * s.smoothing;
        for b in 0..bins {
            let p = self.power[b];
            let share = self.share.share[usize::from(self.band_of[b])];
            let late = share * over * decay_e * self.lam_rev[from * bins + b];
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

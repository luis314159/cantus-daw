//! **Comparatio** — the reference-track comparator, for the master (DSP phase; the name is
//! provisional, [`NAME`]). Latin *comparatio*: "a comparing, a setting side by side".
//!
//! You load reference tracks, switch between your mix and a reference **at the same
//! loudness**, and see how they differ — in tonal balance, dynamics, stereo and peaks. Its
//! references for *function* (inspired by, never copied) are the reference plug-ins of
//! the mastering world: ADPTR Metric AB, Mastering The Mix's REFERENCE, Melda's MCompare.
//!
//! ## What is where
//!
//! | Module | What |
//! |---|---|
//! | this file | the unit: A/B with the crossfade, the loudness match, the blind buttons, rest |
//! | [`measure`] | one source's loudness, dynamics, true peak and stereo, on the audio thread |
//! | [`stereo`] | correlation and width, whole and in three bands |
//! | [`tonal`] | the long-term fractional-octave spectrum, the difference, its confidence |
//! | [`reader`] | where the reference plays (free, locked to the song, a loop) without a click |
//! | [`blind`] | ABX and blind preference: seeded, reproducible, an exact score |
//! | [`match_eq`] | the match-EQ suggestion, a curve for Aequalis's fit |
//!
//! ## The unit
//!
//! ```text
//! mix in ──┬───────────────────────────── × cos ──┐
//!          └─ measure (mix)                       ├──▶ out
//! reference (reader) ─┬─ × match gain ── × sin ───┘
//!                     └─ measure (reference)
//! ```
//!
//! - **The mix is never touched**: while the mix is heard (and not fading), the output is
//!   the input, to the bit — with no reference loaded, or bypassed, always.
//! - **The match gain goes on the reference only**, toward the mix's loudness:
//!   - *integrated* — `I_mix − I_ref` (the reference's integrated loudness over what has
//!     played beside the mix, or over its whole file as the engine measured it at load),
//!     held until both exist;
//!   - *short-term* — `S_mix − S_ref` every 100 ms, held unless both short-term windows
//!     are whole ([`measure::Analyser::whole_window`]: every momentary value of the last
//!     three seconds above the absolute gate and within 20 LU of the integrated loudness),
//!     so neither a quiet intro nor the edge of a quiet passage is raised;
//!   - *manual* — no match, only the offset;
//!
//!   plus the **offset** in every mode, limited to −48 … +24 dB, and gliding (a one-pole
//!   in dB, in `f64`: 0.5 s integrated, 0.3 s short-term, 50 ms manual) whether the
//!   reference is heard or not, so it is where it should be when it is.
//! - **The switch is a crossfade** of equal power (`cos θ`/`sin θ`, which keeps the
//!   loudness of two unrelated signals level through it) over `fade_ms`, counted in
//!   samples, with `θ` itself eased (`π/4 · (1 − cos πx)`) so neither gain starts or
//!   ends with a corner: a corner is a click's spectrum, falling only 12 dB an octave.
//! - **Everything steps on one grid**: the analysers' 100 ms steps counted in samples from
//!   the measurement's start; each pass is cut at the step's end, so the match gain moves
//!   at the same sample whatever the block size.

use crate::math;
use crate::modfx_clock::{Clock, Transport};

#[path = "comparatio_blind.rs"]
pub mod blind;
#[path = "comparatio_match.rs"]
pub mod match_eq;
#[path = "comparatio_measure.rs"]
pub mod measure;
#[path = "comparatio_reader.rs"]
pub mod reader;
#[path = "comparatio_stereo.rs"]
pub mod stereo;
#[path = "comparatio_tonal.rs"]
pub mod tonal;

use measure::{Analyser, Readings};
use reader::{Reader, RefAudio, Sync};

/// The provisional name.
pub const NAME: &str = "Comparatio";
/// The longest piece a pass is cut into (the reference's scratch buffers).
pub const CHUNK: usize = 512;
/// The match gain's limits, dB.
pub const MAX_CUT_DB: f32 = -48.0;
pub const MAX_BOOST_DB: f32 = 24.0;

const ZEROS: [f32; CHUNK] = [0.0; CHUNK];

/// What is heard.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Listen {
    /// The mix (ABX's A).
    #[default]
    Mix,
    /// The reference (ABX's B).
    Reference,
    /// ABX's X: the mix or the reference, hidden ([`blind::x_is_reference`]).
    X,
    /// Blind preference: button One or Two ([`blind::one_is_reference`]).
    One,
    Two,
}

impl Listen {
    pub fn is_blind(self) -> bool {
        matches!(self, Listen::X | Listen::One | Listen::Two)
    }
}

/// How the reference's level is matched to the mix's.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum MatchMode {
    #[default]
    Integrated,
    ShortTerm,
    Manual,
}

/// Which integrated loudness of the reference the integrated match reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum RefLoudness {
    /// What has played beside the mix (the whole file's until then).
    #[default]
    Played,
    /// The whole file, as measured at load (what has played until then).
    WholeFile,
}

/// Comparatio's settings. `Default` is the null: the mix heard, untouched.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ComparatioSettings {
    pub bypass: bool,
    pub listen: Listen,
    pub matching: MatchMode,
    /// Added to the match in every mode, dB (−24 … +24).
    pub offset_db: f32,
    pub ref_loudness: RefLoudness,
    /// The A/B crossfade, ms (1 … 500).
    pub fade_ms: f32,
    pub sync: Sync,
    /// The blind test's seed and the trial under way.
    pub blind_seed: u64,
    pub blind_trial: u32,
}

impl Default for ComparatioSettings {
    fn default() -> Self {
        ComparatioSettings {
            bypass: false,
            listen: Listen::Mix,
            matching: MatchMode::Integrated,
            offset_db: 0.0,
            ref_loudness: RefLoudness::Played,
            fade_ms: 30.0,
            sync: Sync::Free,
            blind_seed: 0,
            blind_trial: 0,
        }
    }
}

/// What is heard, as the panel may show it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Heard {
    Mix,
    Reference,
    /// Between the two (`fade` says how far).
    Fading,
    /// A blind button: not shown.
    Hidden,
}

/// What the unit tells its panel.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ComparatioMeter {
    pub heard: Heard,
    /// 0 (the mix) … 1 (the reference); 0 while blind.
    pub fade: f32,
    /// The match gain on the reference now, and where it is going, dB.
    pub gain_db: f32,
    pub target_db: f32,
    /// The reference's position (its next sample), or `None` while it is silent.
    pub reference_at: Option<i64>,
    pub mix: Readings,
    pub reference: Readings,
}

/// The four streams of a piece, for the engine's tonal worker (Phase B).
#[derive(Copy, Clone, Debug)]
pub struct Tap<'a> {
    pub mix: (&'a [f32], &'a [f32]),
    pub reference: (&'a [f32], &'a [f32]),
}

/// The unit.
#[derive(Clone, Debug)]
pub struct Comparatio {
    sr: f32,
    s: ComparatioSettings,
    clock: Clock,
    reader: Reader,
    mix: Analyser,
    reference: Analyser,
    ref_l: Box<[f32]>,
    ref_r: Box<[f32]>,
    /// The crossfade: 0 (the mix) … `fade_len` (the reference), one step a sample.
    fade: usize,
    fade_len: usize,
    /// The match: the automatic part, the target with the offset, the applied gain.
    match_db: f32,
    target_db: f64,
    gain_db: f64,
    gain: f32,
    glide: f64,
    whole_file: Option<f64>,
    /// The last pass had a reference, of this length.
    ref_len: Option<i64>,
}

/// The equal-power gains at `x` (0 … 1) of a fade, eased at both ends: `(sin θ, cos θ)`
/// with `θ = π/4 · (1 − cos πx)`.
#[inline]
pub fn eased(x: f32) -> (f32, f32) {
    let theta = std::f32::consts::FRAC_PI_4 * (1.0 - math::cos(std::f32::consts::PI * x));
    (math::sin(theta), math::cos(theta))
}

fn fade_len_of(ms: f32, sr: f32) -> usize {
    ((ms.clamp(1.0, 500.0) / 1_000.0 * sr).round() as usize).max(1)
}

fn glide_of(mode: MatchMode, sr: f32) -> f64 {
    let tau = match mode {
        MatchMode::Integrated => 0.5,
        MatchMode::ShortTerm => 0.3,
        MatchMode::Manual => 0.05,
    };
    1.0 - math::exp(-1.0 / (tau * sr.max(1.0) as f64))
}

impl Comparatio {
    /// Allocates here only.
    pub fn new(sample_rate: f32) -> Self {
        let s = ComparatioSettings::default();
        Comparatio {
            sr: sample_rate,
            s,
            clock: Clock::new(),
            reader: Reader::new(sample_rate),
            mix: Analyser::new(sample_rate),
            reference: Analyser::new(sample_rate),
            ref_l: vec![0.0; CHUNK].into(),
            ref_r: vec![0.0; CHUNK].into(),
            fade: 0,
            fade_len: fade_len_of(s.fade_ms, sample_rate),
            match_db: 0.0,
            target_db: 0.0,
            gain_db: 0.0,
            gain: 1.0,
            glide: glide_of(s.matching, sample_rate),
            whole_file: None,
            ref_len: None,
        }
    }

    pub fn settings(&self) -> &ComparatioSettings {
        &self.s
    }

    /// Between passes.
    pub fn set(&mut self, s: &ComparatioSettings) {
        let len = fade_len_of(s.fade_ms, self.sr);
        if len != self.fade_len {
            // The fade keeps where it is, in proportion.
            self.fade = (self.fade * len + self.fade_len / 2) / self.fade_len;
            self.fade_len = len;
        }
        if s.matching != self.s.matching {
            self.glide = glide_of(s.matching, self.sr);
            if s.matching == MatchMode::Manual {
                self.match_db = 0.0;
            }
        }
        if s.sync != self.s.sync {
            self.reader.set_sync(s.sync);
        }
        self.s = *s;
        self.retarget();
    }

    /// The song's transport for the next pass.
    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
    }

    /// The reference's integrated loudness over its whole file, measured off the audio
    /// thread at load ([`analyse_whole`]), or `None`.
    pub fn set_whole_file_loudness(&mut self, lufs: Option<f64>) {
        self.whole_file = lufs;
    }

    /// Put the reference's free (or loop) playhead at `pos` samples.
    pub fn cue(&mut self, pos: i64) {
        self.reader.cue(pos);
    }

    fn retarget(&mut self) {
        let t = (self.match_db + self.s.offset_db).clamp(MAX_CUT_DB, MAX_BOOST_DB);
        self.target_db = t as f64;
    }

    /// Is the reference what should be heard?
    fn wants_reference(&self) -> bool {
        if self.s.bypass || self.ref_len.is_none() {
            return false;
        }
        let (seed, trial) = (self.s.blind_seed, self.s.blind_trial);
        match self.s.listen {
            Listen::Mix => false,
            Listen::Reference => true,
            Listen::X => blind::x_is_reference(seed, trial),
            Listen::One => blind::one_is_reference(seed, trial),
            Listen::Two => !blind::one_is_reference(seed, trial),
        }
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], reference: Option<RefAudio>) {
        self.process_tapped(left, right, reference, |_| {});
    }

    /// [`Self::process`], handing each piece's four streams to `tap` (the mix as it came
    /// in, the reference as read, before its gain).
    pub fn process_tapped<F: FnMut(Tap)>(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        reference: Option<RefAudio>,
        mut tap: F,
    ) {
        let n = left.len().min(right.len());
        self.ref_len = reference.map(|a| a.len());
        let mut i = 0;
        while i < n {
            let m = (n - i).min(CHUNK).min(self.mix.to_step());
            let steps = self.mix.steps();
            self.reader.read(
                reference,
                &self.clock,
                i,
                &mut self.ref_l[..m],
                &mut self.ref_r[..m],
            );
            let (l, r) = (&mut left[i..i + m], &mut right[i..i + m]);
            self.mix.process(l, r);
            self.reference.process(&self.ref_l[..m], &self.ref_r[..m]);
            tap(Tap {
                mix: (l, r),
                reference: (&self.ref_l[..m], &self.ref_r[..m]),
            });
            self.output(l, r, m);
            if self.mix.steps() != steps {
                self.end_step();
            }
            i += m;
        }
        self.clock.advance(n);
    }

    /// The output of a piece: the crossfade and the glide, sample by sample.
    fn output(&mut self, l: &mut [f32], r: &mut [f32], m: usize) {
        if self.s.bypass {
            self.fade = 0;
            self.glide_n(m);
            return;
        }
        let to_ref = self.wants_reference();
        if self.fade == 0 && !to_ref {
            self.glide_n(m);
            return;
        }
        for k in 0..m {
            self.glide_n(1);
            if to_ref && self.fade < self.fade_len {
                self.fade += 1;
            } else if !to_ref && self.fade > 0 {
                self.fade -= 1;
            }
            if self.fade == 0 {
                continue;
            }
            let (rl, rr) = (self.ref_l[k] * self.gain, self.ref_r[k] * self.gain);
            if self.fade == self.fade_len {
                l[k] = rl;
                r[k] = rr;
            } else {
                let (s, c) = eased(self.fade as f32 / self.fade_len as f32);
                l[k] = l[k] * c + rl * s;
                r[k] = r[k] * c + rr * s;
            }
        }
    }

    /// `n` samples of the match gain's glide.
    fn glide_n(&mut self, n: usize) {
        for _ in 0..n {
            if self.gain_db == self.target_db {
                return;
            }
            self.gain_db += (self.target_db - self.gain_db) * self.glide;
            if (self.target_db - self.gain_db).abs() < 1e-3 {
                self.gain_db = self.target_db;
            }
            self.gain = math::db_to_gain(self.gain_db as f32);
        }
    }

    /// A 100 ms step ended: the match follows the readings.
    fn end_step(&mut self) {
        match self.s.matching {
            MatchMode::Manual => self.match_db = 0.0,
            MatchMode::Integrated => {
                let played = self.reference.integrated();
                let r = match self.s.ref_loudness {
                    RefLoudness::Played => played.or(self.whole_file),
                    RefLoudness::WholeFile => self.whole_file.or(played),
                };
                if let (Some(m), Some(r)) = (self.mix.integrated(), r) {
                    self.match_db = (m - r) as f32;
                }
            }
            MatchMode::ShortTerm => {
                if self.mix.whole_window() && self.reference.whole_window() {
                    self.match_db = (self.mix.short_term() - self.reference.short_term()) as f32;
                }
            }
        }
        self.retarget();
    }

    /// Start a new measurement of both, here.
    pub fn restart_measurement(&mut self) {
        self.mix.restart();
        self.reference.restart();
    }

    pub fn meter(&self) -> ComparatioMeter {
        let blind = self.s.listen.is_blind();
        let heard = if blind {
            Heard::Hidden
        } else if self.fade == 0 {
            Heard::Mix
        } else if self.fade == self.fade_len {
            Heard::Reference
        } else {
            Heard::Fading
        };
        ComparatioMeter {
            heard,
            fade: if blind {
                0.0
            } else {
                self.fade as f32 / self.fade_len as f32
            },
            gain_db: self.gain_db as f32,
            target_db: self.target_db as f32,
            reference_at: self.reader.position(),
            mix: self.mix.readings(),
            reference: self.reference.readings(),
        }
    }

    /// The short-term histograms of the mix and the reference ([`Analyser::histogram`]).
    pub fn histograms(&self) -> (&[u32; measure::HIST_BINS], &[u32; measure::HIST_BINS]) {
        (self.mix.histogram(), self.reference.histogram())
    }

    /// Latency: none.
    pub fn latency(&self) -> usize {
        0
    }

    /// At rest (ADR 0031 §5.1): silence in gives silence out and nothing that holds
    /// sound moves — the mix heard (or the reference, silent), no fade, the reference
    /// silent ahead, every filter empty. (The match gain may still glide: what it
    /// multiplies is silence, and [`Self::skip`] glides it as processing would.)
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// What keeps it from resting, by name.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        if self.fade != 0 && self.fade != self.fade_len {
            return Some("crossfade");
        }
        if !self.s.bypass && (self.fade == 0) == self.wants_reference() {
            return Some("switch pending");
        }
        if !self.reference_silent() {
            return Some("reference playing");
        }
        if !self.mix.is_clear() || !self.reference.is_clear() {
            return Some("analysers");
        }
        None
    }

    /// The reference makes no sound now and will not without a jump.
    fn reference_silent(&self) -> bool {
        let Some(len) = self.ref_len else {
            return self.reader.is_silent();
        };
        if self.reader.is_silent() {
            return !self.clock.playing();
        }
        matches!(
            (self.reader.sync(), self.reader.position()),
            (Sync::Locked { .. }, Some(p)) if p >= len
        ) && self.reader.fade_done()
    }

    /// `n` samples of silence skipped at rest: exactly what processing them leaves.
    pub fn skip(&mut self, n: usize) {
        let mut i = 0;
        while i < n {
            let m = (n - i).min(CHUNK).min(self.mix.to_step());
            let steps = self.mix.steps();
            self.mix.process(&ZEROS[..m], &ZEROS[..m]);
            self.reference.process(&ZEROS[..m], &ZEROS[..m]);
            // At rest the output is silence whatever the gain; the glide goes on as in
            // `output`, sample by sample.
            self.glide_n(m);
            if self.mix.steps() != steps {
                self.end_step();
            }
            i += m;
        }
        self.reader.skip(&self.clock, n);
        self.clock.skip(n);
    }

    /// As new (the settings kept), allocating nothing.
    pub fn reset(&mut self) {
        self.clock.reset();
        self.reader.reset();
        self.mix.reset();
        self.reference.reset();
        self.fade = 0;
        self.match_db = 0.0;
        self.retarget();
        self.gain_db = self.target_db;
        self.gain = math::db_to_gain(self.gain_db as f32);
        self.ref_len = None;
    }
}

/// A whole file analysed off the audio thread (at load): its readings, its short-term
/// histogram and its long-term curve.
#[derive(Clone, Debug)]
pub struct WholeAnalysis {
    pub readings: Readings,
    pub histogram: [u32; measure::HIST_BINS],
    pub curve: tonal::Curve,
}

/// Analyse a whole file (allocates; never on the audio thread). The step in progress and one
/// more of silence after it close the last step and the true peak's interpolator.
pub fn analyse_whole(
    left: &[f32],
    right: &[f32],
    sample_rate: f32,
    resolution: tonal::Resolution,
) -> WholeAnalysis {
    let mut a = Analyser::new(sample_rate);
    let mut t = tonal::Tonal::new(sample_rate, resolution);
    for (l, r) in left.chunks(4_096).zip(right.chunks(4_096)) {
        a.process(l, r);
        t.push(l, r);
    }
    for _ in 0..2 {
        let tail = vec![0.0f32; a.to_step()];
        a.process(&tail, &tail);
    }
    WholeAnalysis {
        readings: a.readings(),
        histogram: *a.histogram(),
        curve: t.curve(),
    }
}

#[cfg(test)]
#[path = "comparatio_testkit.rs"]
mod testkit;
#[cfg(test)]
#[path = "comparatio_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "comparatio_tests_match.rs"]
mod tests_match;
#[cfg(test)]
#[path = "comparatio_tests_shared.rs"]
mod tests_shared;

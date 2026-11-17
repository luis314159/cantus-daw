//! What Comparatio's console shows, held between frames (design sheet v1): the engine's
//! telemetry (`cantus_engine::fx::comparatio::t`) read every frame — what is heard, the
//! switch's fade, the match gain, both sources' loudness, peaks, dynamics and stereo, and
//! the two short-term histograms assembled from their ten pages — and the tonal curves:
//! the reference's from its whole file (analysed off the audio thread when it was read,
//! `comparatio_store`), your mix's from the slot's analyser tap ([`Live`], phase B) — both
//! the signal's middle, so like meets like — with their difference and the match EQ's
//! suggestion.

use cantus_dsp::comparatio::match_eq::{MatchCurve, MatchOptions};
use cantus_dsp::comparatio::measure::HIST_BINS;
use cantus_dsp::comparatio::tonal::{Curve, Difference, Resolution, Tonal};
use cantus_engine::fx::comparatio_store::Summary;
use cantus_engine::fx::comparatio::{t, R_COUNT};
use cantus_engine::fx::TELEMETRY_LEN;

/// One source's readings, as the telemetry carries them (`R_*` order; NaN: not yet).
pub type Readings = [f32; R_COUNT];

/// The stereo bands: whole, lows, mids, highs — each (correlation, width).
pub type Stereo = [(f32, f32); 4];

/// What is heard.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Heard {
    #[default]
    Mix,
    Reference,
    Fading,
    Hidden,
}

#[derive(Clone, Debug)]
pub struct Picture {
    pub heard: Heard,
    pub fade: f32,
    pub gain: f32,
    pub target: f32,
    pub ref_at: f32,
    pub ref_len: f32,
    pub whole: f32,
    pub mix: Readings,
    pub reference: Readings,
    pub mix_stereo: Stereo,
    pub ref_stereo: Stereo,
    pub mix_hist: [f32; HIST_BINS],
    pub ref_hist: [f32; HIST_BINS],
    /// The tonal curves and their difference, when there are both.
    pub mix_curve: Option<Box<Curve>>,
    pub ref_curve: Option<Box<Curve>>,
    pub diff: Option<Box<Difference>>,
    /// The match EQ's suggestion (phase B): what Send to Aequalis puts before the unit.
    pub match_eq: Option<Box<MatchCurve>>,
    /// A rough picture of the reference's level over its length (0 … 1, 240 points).
    pub overview: Vec<f32>,
    pub got: bool,
}

impl Default for Picture {
    fn default() -> Self {
        Picture {
            heard: Heard::Mix,
            fade: 0.0,
            gain: 0.0,
            target: 0.0,
            ref_at: f32::NAN,
            ref_len: 0.0,
            whole: f32::NAN,
            mix: [f32::NAN; R_COUNT],
            reference: [f32::NAN; R_COUNT],
            mix_stereo: [(f32::NAN, f32::NAN); 4],
            ref_stereo: [(f32::NAN, f32::NAN); 4],
            mix_hist: [0.0; HIST_BINS],
            ref_hist: [0.0; HIST_BINS],
            mix_curve: None,
            ref_curve: None,
            diff: None,
            match_eq: None,
            overview: Vec::new(),
            got: false,
        }
    }
}

impl Picture {
    /// Read a block's telemetry (every frame).
    pub fn update(&mut self, tel: &[f32; TELEMETRY_LEN]) {
        if tel[t::SAMPLE_RATE] <= 0.0 {
            return;
        }
        self.got = true;
        self.heard = match tel[t::HEARD] as i32 {
            1 => Heard::Reference,
            2 => Heard::Fading,
            3 => Heard::Hidden,
            _ => Heard::Mix,
        };
        self.fade = tel[t::FADE];
        self.gain = tel[t::GAIN];
        self.target = tel[t::TARGET];
        self.ref_at = tel[t::REF_AT];
        self.ref_len = tel[t::REF_LEN];
        self.whole = tel[t::WHOLE];
        self.mix.copy_from_slice(&tel[t::MIX..t::MIX + R_COUNT]);
        self.reference
            .copy_from_slice(&tel[t::REF..t::REF + R_COUNT]);
        for b in 0..4 {
            self.mix_stereo[b] = (tel[t::MIX_STEREO + 2 * b], tel[t::MIX_STEREO + 2 * b + 1]);
            self.ref_stereo[b] = (tel[t::REF_STEREO + 2 * b], tel[t::REF_STEREO + 2 * b + 1]);
        }
        let page = tel[t::HIST_PAGE] as usize;
        let from = (page % t::PAGES_PER_SOURCE) * t::PER_PAGE;
        let hist = if page < t::PAGES_PER_SOURCE {
            &mut self.mix_hist
        } else {
            &mut self.ref_hist
        };
        if from + t::PER_PAGE <= HIST_BINS {
            hist[from..from + t::PER_PAGE].copy_from_slice(&tel[t::HIST..t::HIST + t::PER_PAGE]);
        }
    }

    /// Whether a reference is loaded.
    pub fn loaded(&self) -> bool {
        self.ref_len > 0.0
    }

    /// Set both tonal curves (the difference with them).
    pub fn set_curves(&mut self, mix: Curve, reference: Curve) {
        let d = cantus_dsp::comparatio::tonal::compare(&mix, &reference);
        self.mix_curve = Some(Box::new(mix));
        self.ref_curve = Some(Box::new(reference));
        self.diff = Some(Box::new(d));
    }
}

/// A reading as words with its unit, or a dash when it is not there yet: "−14.2".
pub fn num(v: f32, digits: usize, b: &mut String) {
    use std::fmt::Write as _;
    b.clear();
    if v.is_finite() {
        let _ = write!(b, "{v:.digits$}");
    } else {
        b.push('\u{2014}');
    }
}

/// Your mix's long-term tonal curves, live (phase B): the slot's analyser tap carries the
/// unit's input — the mix, whatever is heard — as its middle `(L + R) / 2`
/// (`FxHost::take_pairs`, the pair's first), measured here on the editor's thread in thirds
/// and sixths of an octave by the DSP's own analyser; the reference's curves are the same
/// middle of its whole file (`comparatio_store::Summary::mid`), so like meets like. The
/// measurement starts again when the unit's does (its seconds measured go back). The
/// demo feeds the same path. Allocates only in [`Live::new`].
pub struct Live {
    rate: f32,
    tonal: [Tonal; 2],
    mid: Vec<f32>,
    pairs: Vec<(f32, f32)>,
    /// The unit's seconds measured at the last pull.
    seconds: f32,
    /// Frames pushed since the curves were last made.
    fresh: bool,
}

impl std::fmt::Debug for Live {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "comparatio::Live({} Hz)", self.rate)
    }
}

impl Live {
    pub fn new(rate: f32) -> Live {
        Live {
            rate,
            tonal: [
                Tonal::new(rate, Resolution::Third),
                Tonal::new(rate, Resolution::Sixth),
            ],
            mid: Vec::with_capacity(16_384),
            pairs: Vec::with_capacity(16_384),
            seconds: 0.0,
            fresh: false,
        }
    }

    /// Take what the host has of the slot's pairs; `measured` is the unit's seconds of the
    /// mix measured (a restart of the unit's measurement restarts the curves).
    pub fn pull(&mut self, host: &mut dyn super::super::host::FxHost, measured: f32) {
        let sr = host.sample_rate();
        if (sr - self.rate).abs() > 0.5 && sr > 1_000.0 {
            *self = Live::new(sr);
        }
        if measured.is_finite() && measured + 0.5 < self.seconds {
            self.restart();
        }
        if measured.is_finite() {
            self.seconds = measured;
        }
        let mut pairs = std::mem::take(&mut self.pairs);
        pairs.clear();
        host.take_pairs(&mut pairs);
        let mut mid = std::mem::take(&mut self.mid);
        mid.clear();
        mid.extend(pairs.iter().map(|p| p.0));
        self.feed(&mid);
        self.mid = mid;
        self.pairs = pairs;
    }

    /// The mix's middle, as the tap carries it.
    pub fn feed(&mut self, mid: &[f32]) {
        if mid.is_empty() {
            return;
        }
        for t in &mut self.tonal {
            t.push(mid, mid);
        }
        self.fresh = true;
    }

    pub fn restart(&mut self) {
        for t in &mut self.tonal {
            t.restart();
        }
        self.fresh = true;
    }

    /// Whether new frames came since the last [`Live::take_fresh`].
    pub fn take_fresh(&mut self) -> bool {
        std::mem::take(&mut self.fresh)
    }

    /// The curve at `res` (0 thirds, 1 sixths), once enough was heard.
    pub fn curve(&self, res: usize) -> Option<Curve> {
        let t = &self.tonal[res.min(1)];
        (t.frames() >= cantus_dsp::comparatio::tonal::MIN_FRAMES).then(|| t.curve())
    }
}

/// The reference's side of the picture from the store's summary of the one compared: its
/// curve at `res` and its overview. Whether one was there.
pub fn reference_of(pic: &mut Picture, summary: &Summary, res: usize) -> Curve {
    if pic.overview.len() != summary.overview.len() {
        pic.overview.clone_from(&summary.overview);
    } else {
        pic.overview.copy_from_slice(&summary.overview);
    }
    summary.mid[res.min(1)].clone()
}

impl Picture {
    /// The curves, their difference and the match EQ's suggestion, made again (no
    /// allocation once the boxes exist).
    pub fn set_all(&mut self, mix: Curve, reference: Curve, opts: &MatchOptions) {
        let d = cantus_dsp::comparatio::tonal::compare(&mix, &reference);
        let m = cantus_dsp::comparatio::match_eq::suggest(&d, opts);
        put_box(&mut self.mix_curve, mix);
        put_box(&mut self.ref_curve, reference);
        put_box(&mut self.diff, d);
        put_box(&mut self.match_eq, m);
    }

    /// No curves (no reference, or not enough heard).
    pub fn clear_curves(&mut self) {
        self.mix_curve = None;
        self.ref_curve = None;
        self.diff = None;
        self.match_eq = None;
    }
}

fn put_box<T>(slot: &mut Option<Box<T>>, v: T) {
    match slot {
        Some(b) => **b = v,
        None => *slot = Some(Box::new(v)),
    }
}

/// The match EQ's options from the unit's keys (`eqamt`, `eqmax`).
pub fn match_options(spec: &cantus_core::EffectSpec) -> MatchOptions {
    MatchOptions {
        amount: spec.get("eqamt").clamp(0.0, 1.0),
        max_db: spec.get("eqmax").clamp(1.0, 12.0),
        ..MatchOptions::default()
    }
}

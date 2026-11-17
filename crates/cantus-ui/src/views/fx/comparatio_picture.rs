//! What Comparatio's console shows, held between frames (design sheet v1): the engine's
//! telemetry (`cantus_engine::fx::comparatio::t`) read every frame — what is heard, the
//! switch's fade, the match gain, both sources' loudness, peaks, dynamics and stereo, and
//! the two short-term histograms assembled from their ten pages — and the tonal curves:
//! the reference's from its whole file (analysed off the audio thread when it was read,
//! `comparatio_store`), your mix's from the slot's signal (step 3, the slot's scope as
//! Speculum's; in the design sheet the demo's own analysis).

use cantus_dsp::comparatio::measure::HIST_BINS;
use cantus_dsp::comparatio::tonal::{Curve, Difference};
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

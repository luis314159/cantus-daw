//! Comparatio, the reference comparator (ADR 0146, provisional), behind the engine's plug-in
//! contract. Owned by the wave-C builder.
//!
//! **The mix is never touched**: with no reference, or the mix heard and no fade running,
//! the output is the input, to the bit. The reference (a file by path, `EffectSpec::ir`)
//! comes off the audio thread from [`super::comparatio_store`] through the memory path
//! (`MemoryNeed::Comparatio`): the unit asks for it when the snapshot names it and holds an
//! `Arc` the store has a sibling of, so letting it go never frees here.
//!
//! - **A swap or a removal while the reference is heard fades to the mix first**: the new
//!   reference waits (or the old one is kept) with the DSP set to the mix until it hears
//!   the mix, then the swap happens and the settings come back — no click, no cut.
//! - **The transport** is the song's real samples (`SongPosition::project_samples`): a
//!   locked reference follows the song in time, through tempo changes.
//! - **Renders never hear it**: the engine's compile for a render takes the reference away
//!   (`comparatio_store::strip_for_render`).
//! - Latency 0. Rest and skip are the DSP's (exact).
//!
//! The telemetry carries what is heard, the match, both sources' readings and stereo
//! bands, and the two short-term histograms a page at a time (ten pages of twelve bins).

use super::comparatio_store::RefTrack;
use super::{FxUnit, SongPosition, Telemetry, TELEMETRY_LEN};
use crate::effects::{EffectMemory, MemoryNeed};
use cantus_core::EffectSpec;
use cantus_dsp::comparatio::measure::{Readings, HIST_BINS};
use cantus_dsp::comparatio::reader::{RefAudio, Sync};
use cantus_dsp::comparatio::stereo::BANDS as STEREO_BANDS;
use cantus_dsp::comparatio::{
    Comparatio, ComparatioSettings, Heard, Listen, MatchMode, RefLoudness,
};
use cantus_dsp::modfx_clock::Transport;
use std::sync::Arc;

/// What a unit asks the store for: the reference by its path's key.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ComparatioNeed {
    pub key: u64,
}

/// The telemetry's layout (what the editor draws).
pub mod t {
    use super::{HIST_BINS, STEREO_BANDS};
    /// What is heard: 0 the mix, 1 the reference, 2 between, 3 hidden (a blind test).
    pub const HEARD: usize = 0;
    /// How far the switch has gone toward the reference, 0 … 1 (0 when hidden).
    pub const FADE: usize = 1;
    /// The match gain on the reference now and where it is going, dB.
    pub const GAIN: usize = 2;
    pub const TARGET: usize = 3;
    /// Where the reference plays, s (NaN: nowhere), and its length, s (0: none loaded).
    pub const REF_AT: usize = 4;
    pub const REF_LEN: usize = 5;
    /// The reference's whole-file integrated loudness, LUFS (NaN until analysed).
    pub const WHOLE: usize = 6;
    pub const SAMPLE_RATE: usize = 7;
    /// Each source's readings ([`super::R_COUNT`] of them, `R_*` order): the mix's, the
    /// reference's. A reading not there yet is NaN; silence is −∞.
    pub const MIX: usize = 8;
    pub const REF: usize = MIX + super::R_COUNT;
    /// Each source's stereo bands over the whole measurement: whole, lows, mids, highs,
    /// each a correlation then a width (NaN: no signal in the band).
    pub const MIX_STEREO: usize = REF + super::R_COUNT;
    pub const REF_STEREO: usize = MIX_STEREO + 2 * STEREO_BANDS;
    /// The histogram page this block carries, and its bins: page `p` is source `p / 5`
    /// (0 the mix), bins `(p % 5)·12 …` — steps counted at that short-term loudness.
    pub const HIST_PAGE: usize = REF_STEREO + 2 * STEREO_BANDS;
    pub const HIST: usize = HIST_PAGE + 1;
    pub const PER_PAGE: usize = 12;
    pub const PAGES_PER_SOURCE: usize = HIST_BINS / PER_PAGE;
    pub const PAGES: usize = 2 * PAGES_PER_SOURCE;
    pub const END: usize = HIST + PER_PAGE;
}

/// The readings' order in the telemetry.
pub const R_MOMENTARY: usize = 0;
pub const R_SHORT: usize = 1;
pub const R_INTEGRATED: usize = 2;
pub const R_RANGE: usize = 3;
pub const R_MAX_MOMENTARY: usize = 4;
pub const R_MAX_SHORT: usize = 5;
pub const R_TRUE_PEAK: usize = 6;
pub const R_SAMPLE_PEAK: usize = 7;
pub const R_CREST: usize = 8;
pub const R_PLR: usize = 9;
pub const R_PSR: usize = 10;
pub const R_PSR_MEAN: usize = 11;
pub const R_SECONDS: usize = 12;
pub const R_COUNT: usize = 13;

/// Comparatio's parameters as the DSP wants them, for a reference `frames` long at `rate`.
pub fn settings(spec: &EffectSpec, rate: f32, frames: usize) -> ComparatioSettings {
    Raw::read(spec).settings(rate, frames)
}

/// The spec's numbers the settings are made from, kept by the unit (copying the spec itself
/// would allocate on the audio thread): the loop's end depends on the reference's length,
/// which can change with no new spec.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Raw {
    base: ComparatioSettings,
    sync: usize,
    soff: f32,
    lstart: f32,
    lend: f32,
}

impl Raw {
    pub fn read(spec: &EffectSpec) -> Raw {
        let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
        Raw {
            base: base(spec),
            sync: idx("sync"),
            soff: spec.get("soff"),
            lstart: spec.get("lstart"),
            lend: spec.get("lend"),
        }
    }

    pub fn settings(&self, rate: f32, frames: usize) -> ComparatioSettings {
        let at = |s: f32| (f64::from(s) * f64::from(rate)).round() as i64;
        let sync = match self.sync {
            1 => Sync::Locked {
                offset: at(self.soff),
            },
            2 => {
                let start = at(self.lstart).clamp(0, frames as i64);
                let end = at(self.lend);
                Sync::Loop {
                    start,
                    end: if end > start {
                        end.min(frames as i64)
                    } else {
                        frames as i64
                    },
                }
            }
            _ => Sync::Free,
        };
        ComparatioSettings { sync, ..self.base }
    }
}

fn base(spec: &EffectSpec) -> ComparatioSettings {
    let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
    ComparatioSettings {
        bypass: false,
        listen: match idx("listen") {
            1 => Listen::Reference,
            2 => Listen::X,
            3 => Listen::One,
            4 => Listen::Two,
            _ => Listen::Mix,
        },
        matching: match idx("match") {
            1 => MatchMode::ShortTerm,
            2 => MatchMode::Manual,
            _ => MatchMode::Integrated,
        },
        offset_db: spec.get("offset"),
        ref_loudness: if idx("refl") >= 1 {
            RefLoudness::WholeFile
        } else {
            RefLoudness::Played
        },
        fade_ms: spec.get("fade"),
        sync: Sync::Free,
        blind_seed: idx("seed") as u64,
        blind_trial: idx("trial") as u32,
    }
}

#[derive(Clone)]
pub struct Unit {
    dsp: Box<Comparatio>,
    sample_rate: f32,
    /// The settings the spec asks for (the DSP may be held on the mix for a swap).
    asked: ComparatioSettings,
    /// The spec's numbers (the loop's end depends on the reference's length).
    raw: Raw,
    /// The reference the snapshot names, by its path's key.
    want: Option<u64>,
    reference: Option<Arc<RefTrack>>,
    /// A reference that arrived while the old one was heard: it waits for the mix.
    next: Option<Arc<RefTrack>>,
    /// The DSP is held on the mix until it hears it (a swap or a removal under way).
    holding: bool,
    /// The reference asked for and answered with none (a render's answer): not asked again
    /// until the snapshot names another.
    refused: Option<u64>,
    blocks: u64,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "comparatio::Unit({})",
            if self.reference.is_some() {
                "a reference"
            } else {
                "no reference"
            }
        )
    }
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Comparatio::new(sample_rate)),
            sample_rate,
            asked: ComparatioSettings::default(),
            raw: Raw::read(&EffectSpec::new(cantus_core::EffectKind::Reference)),
            want: None,
            reference: None,
            next: None,
            holding: false,
            refused: None,
            blocks: 0,
        }
    }

    /// The DSP, for the editor and the tests.
    pub fn dsp(&self) -> &Comparatio {
        &self.dsp
    }

    /// The reference held, for the tests.
    pub fn reference(&self) -> Option<&Arc<RefTrack>> {
        self.reference.as_ref()
    }

    fn frames(&self) -> usize {
        self.reference.as_ref().map_or(0, |r| r.frames())
    }

    /// The DSP's settings: the spec's, or the mix while a swap waits.
    fn apply(&mut self) {
        self.asked = self.raw.settings(self.sample_rate, self.frames());
        let mut s = self.asked;
        let stale = self.reference.is_some()
            && (self.next.is_some() || self.want != self.reference.as_ref().map(|r| r.key));
        self.holding = stale;
        if stale {
            s.listen = Listen::Mix;
        }
        self.dsp.set(&s);
        self.dsp
            .set_whole_file_loudness(self.reference.as_ref().and_then(|r| r.whole_lufs()));
    }

    /// The swap waited for: once the mix is heard, the reference the snapshot names.
    fn settle(&mut self) {
        if !self.holding || self.dsp.meter().heard != Heard::Mix {
            return;
        }
        let keep = self.want.is_some() && self.next.is_none();
        if !keep || self.want != self.reference.as_ref().map(|r| r.key) {
            // Let go here: the store holds a sibling, so this only counts down.
            self.reference = self.next.take();
        }
        self.apply();
    }
}

fn opt(v: Option<f64>) -> f32 {
    v.map_or(f32::NAN, |x| x as f32)
}

fn put_readings(out: &mut Telemetry, at: usize, r: &Readings) {
    out[at + R_MOMENTARY] = r.momentary as f32;
    out[at + R_SHORT] = r.short_term as f32;
    out[at + R_INTEGRATED] = opt(r.integrated);
    out[at + R_RANGE] = opt(r.range);
    out[at + R_MAX_MOMENTARY] = r.max_momentary as f32;
    out[at + R_MAX_SHORT] = r.max_short_term as f32;
    out[at + R_TRUE_PEAK] = r.true_peak_db as f32;
    out[at + R_SAMPLE_PEAK] = r.sample_peak_db as f32;
    out[at + R_CREST] = opt(r.crest_db);
    out[at + R_PLR] = opt(r.plr);
    out[at + R_PSR] = opt(r.psr);
    out[at + R_PSR_MEAN] = opt(r.psr_mean);
    out[at + R_SECONDS] = r.seconds as f32;
}

fn put_stereo(out: &mut Telemetry, at: usize, r: &Readings) {
    for (b, band) in r.stereo.iter().enumerate() {
        out[at + 2 * b] = opt(band.correlation);
        out[at + 2 * b + 1] = opt(band.width);
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.raw = Raw::read(spec);
        self.want = spec
            .ir
            .as_deref()
            .map(|f| super::aula_store::file_key(&f.path));
        self.apply();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp
            .set_whole_file_loudness(self.reference.as_ref().and_then(|r| r.whole_lufs()));
        let audio = self.reference.as_deref().map(|r| RefAudio {
            left: &r.left,
            right: &r.right,
            id: r.id,
        });
        self.dsp.process(left, right, audio);
        self.blocks = self.blocks.wrapping_add(1);
        self.settle();
    }

    fn reset(&mut self) {
        self.dsp.reset();
        self.settle();
    }

    fn at_rest(&self) -> bool {
        !self.holding && self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        if self.holding {
            return Some("Comparatio: a reference waits for the mix");
        }
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn set_position(&mut self, p: &SongPosition) {
        self.dsp.set_transport(&Transport {
            playing: p.playing,
            tempo_bpm: p.tempo_bpm,
            samples: p.project_samples,
        });
    }

    fn follows_song(&self) -> bool {
        true
    }

    fn memory_need(&self) -> Option<MemoryNeed> {
        let key = self.want?;
        if self.refused == Some(key) {
            return None;
        }
        let have = |r: &Option<Arc<RefTrack>>| r.as_ref().map(|r| r.key) == Some(key);
        (!have(&self.reference) && !have(&self.next))
            .then_some(MemoryNeed::Comparatio(ComparatioNeed { key }))
    }

    fn supply(&mut self, memory: EffectMemory) -> Option<EffectMemory> {
        let track = match memory {
            EffectMemory::Comparatio(Some(track)) => track,
            EffectMemory::Comparatio(None) => {
                self.refused = self.want;
                return None;
            }
            other => return Some(other),
        };
        self.refused = None;
        if self.want != Some(track.key) {
            return Some(EffectMemory::Comparatio(Some(track)));
        }
        if self.reference.is_some() && self.dsp.meter().heard != Heard::Mix {
            // Heard (or hidden): it waits for the mix.
            let old = self.next.replace(track);
            self.apply();
            return old.map(|t| EffectMemory::Comparatio(Some(t)));
        }
        let old = self.reference.replace(track);
        self.next = None;
        self.apply();
        old.map(|t| EffectMemory::Comparatio(Some(t)))
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut out = [0.0; TELEMETRY_LEN];
        out[t::HEARD] = match m.heard {
            Heard::Mix => 0.0,
            Heard::Reference => 1.0,
            Heard::Fading => 2.0,
            Heard::Hidden => 3.0,
        };
        out[t::FADE] = m.fade;
        out[t::GAIN] = m.gain_db;
        out[t::TARGET] = m.target_db;
        let fs = f64::from(self.sample_rate.max(1.0));
        out[t::REF_AT] = m.reference_at.map_or(f32::NAN, |s| (s as f64 / fs) as f32);
        out[t::REF_LEN] = (self.frames() as f64 / fs) as f32;
        out[t::WHOLE] = opt(self.reference.as_ref().and_then(|r| r.whole_lufs()));
        out[t::SAMPLE_RATE] = self.sample_rate;
        put_readings(&mut out, t::MIX, &m.mix);
        put_readings(&mut out, t::REF, &m.reference);
        put_stereo(&mut out, t::MIX_STEREO, &m.mix);
        put_stereo(&mut out, t::REF_STEREO, &m.reference);
        let page = (self.blocks % t::PAGES as u64) as usize;
        out[t::HIST_PAGE] = page as f32;
        let (mix, rf) = self.dsp.histograms();
        let hist = if page < t::PAGES_PER_SOURCE { mix } else { rf };
        let from = (page % t::PAGES_PER_SOURCE) * t::PER_PAGE;
        for j in 0..t::PER_PAGE {
            out[t::HIST + j] = hist[from + j] as f32;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_fits_and_the_pages_cover_both_histograms() {
        const { assert!(t::END <= TELEMETRY_LEN) };
        assert_eq!(t::PAGES_PER_SOURCE * t::PER_PAGE, HIST_BINS);
    }

    #[test]
    fn the_defaults_are_the_mix_heard() {
        let spec = EffectSpec::new(cantus_core::EffectKind::Reference);
        assert_eq!(settings(&spec, 48_000.0, 0), ComparatioSettings::default());
    }
}

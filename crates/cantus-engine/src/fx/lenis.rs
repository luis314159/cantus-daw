//! Lenis, the resonance suppressor (ADR 0140), behind the engine's plug-in
//! contract. Owned by the wave-B builder.
//!
//! [`settings`] reads the table (`cantus_core::fx::lenis`) into the DSP's settings. The
//! latency is the quality's frame (255 / 511 / 1 023 samples at 48 kHz), read live by the
//! engine each pass, so its delay compensation follows a **quality change**: the DSP takes
//! it at its next hop and starts again from silence (a gap of one latency — two outputs
//! delayed differently cannot crossfade; ADR 0140), the strip's latency changes with it and
//! the other paths are re-aligned. At depth 0 the output is the input delayed by that
//! latency, to the bit. `reset` makes the unit as new, its frame grid's clock at 0 (the
//! engine skips it to where the stream is after), so a reset unit and a new one agree to
//! the bit.
//!
//! The telemetry carries the meter's 128 display points a page at a time (the page is the
//! frame count's, so it changes with every hop): the editor keeps the whole picture and
//! refreshes each page as it comes.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::lenis::BANDS;
use cantus_core::EffectSpec;
use cantus_dsp::lenis::{Channels, FocusBand, Lenis, LenisSettings, Mode, Quality, DISPLAY};

/// The telemetry's layout (what the editor draws).
pub mod t {
    /// Display points a page carries, and the pages: page `p` carries the points
    /// `p, p + PAGES, p + 2·PAGES, …` (an older page leaves an interleaved, slightly older
    /// value, never a hole).
    pub const PER_PAGE: usize = 26;
    pub const PAGES: usize = 5;
    /// The input's level at the page's points, dBFS.
    pub const LEVEL: usize = 0;
    /// The reduction at the page's points, dB (≥ 0).
    pub const REDUCTION: usize = PER_PAGE;
    /// Which page this is, 0 … PAGES − 1.
    pub const PAGE: usize = 2 * PER_PAGE;
    /// The most any bin is turned down now, dB.
    pub const MAX_REDUCTION: usize = PAGE + 1;
    /// The latency, samples; the quality in force, 0 … 2.
    pub const LATENCY: usize = PAGE + 2;
    pub const QUALITY: usize = PAGE + 3;
    /// Frames analysed since the start, modulo 2²⁰ (the editor sees when it moved).
    pub const FRAMES: usize = PAGE + 4;
    pub const SAMPLE_RATE: usize = PAGE + 5;
    /// The input's and the output's peaks in the block, linear.
    pub const IN_PEAK: usize = PAGE + 6;
    pub const OUT_PEAK: usize = PAGE + 7;
    /// The end of Lenis's numbers.
    pub const END: usize = PAGE + 8;
}

/// The display point the `j`th slot of page `page` carries (`None` past the display).
pub fn page_point(page: usize, j: usize) -> Option<usize> {
    let i = page + j * t::PAGES;
    (i < DISPLAY).then_some(i)
}

/// Lenis's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> LenisSettings {
    let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
    let mut bands = [FocusBand::default(); BANDS];
    for (n, b) in bands.iter_mut().enumerate() {
        let key = |w: &str| cantus_core::fx::lenis::band_key(n, w);
        *b = FocusBand {
            on: spec.get(key("on")) >= 0.5,
            hz: spec.get(key("hz")),
            amount: spec.get(key("amt")),
            width_oct: spec.get(key("w")),
        };
    }
    LenisSettings {
        depth: spec.get("depth"),
        sharpness: spec.get("sharp"),
        selectivity: spec.get("select"),
        attack_ms: spec.get("att"),
        release_ms: spec.get("rel"),
        low_hz: spec.get("lo"),
        high_hz: spec.get("hi"),
        bands,
        channels: match idx("chan") {
            1 => Channels::Independent,
            2 => Channels::MidSide,
            _ => Channels::Linked,
        },
        mode: if idx("curve") >= 1 {
            Mode::Hard
        } else {
            Mode::Soft
        },
        quality: match idx("qual") {
            0 => Quality::Fast,
            2 => Quality::Precise,
            _ => Quality::Standard,
        },
        delta: spec.get("delta") >= 0.5,
        mix: spec.get("mix"),
        out_db: spec.get("out"),
    }
}

#[derive(Clone)]
pub struct Unit {
    dsp: Box<Lenis>,
    sample_rate: f32,
    peaks: [f32; 2],
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lenis::Unit({} samples late)", self.dsp.latency())
    }
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Lenis::new(sample_rate)),
            sample_rate,
            peaks: [0.0; 2],
        }
    }

    /// The DSP, for the editor's drawing and the tests.
    pub fn dsp(&self) -> &Lenis {
        &self.dsp
    }
}

fn peak(l: &[f32], r: &[f32]) -> f32 {
    l.iter().chain(r).fold(0f32, |m, v| m.max(v.abs()))
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.peaks[0] = peak(left, right);
        self.dsp.process(left, right);
        self.peaks[1] = peak(left, right);
    }

    /// As new: the DSP's clock back at 0 too (the engine skips the unit to where the
    /// stream is after a reset, so the frame grid lands where a new unit's does).
    fn reset(&mut self) {
        self.dsp.restart();
        self.peaks = [0.0; 2];
    }

    fn at_rest(&self) -> bool {
        self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
        self.peaks = [0.0; 2];
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn gain_reduction_db(&self) -> Option<f32> {
        Some(self.dsp.meter().max_reduction_db)
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut out = [0.0; TELEMETRY_LEN];
        let page = (m.frames % t::PAGES as u64) as usize;
        for j in 0..t::PER_PAGE {
            if let Some(i) = page_point(page, j) {
                out[t::LEVEL + j] = m.level_db[i];
                out[t::REDUCTION + j] = m.reduction_db[i];
            }
        }
        out[t::PAGE] = page as f32;
        out[t::MAX_REDUCTION] = m.max_reduction_db;
        out[t::LATENCY] = self.dsp.latency() as f32;
        out[t::QUALITY] = match self.dsp.settings().quality {
            Quality::Fast => 0.0,
            Quality::Standard => 1.0,
            Quality::Precise => 2.0,
        };
        out[t::FRAMES] = (m.frames % (1 << 20)) as f32;
        out[t::SAMPLE_RATE] = self.sample_rate;
        out[t::IN_PEAK] = self.peaks[0];
        out[t::OUT_PEAK] = self.peaks[1];
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pages_cover_every_display_point_once() {
        let mut seen = [0u8; DISPLAY];
        for p in 0..t::PAGES {
            for j in 0..t::PER_PAGE {
                if let Some(i) = page_point(p, j) {
                    seen[i] += 1;
                }
            }
        }
        assert!(seen.iter().all(|n| *n == 1));
        const { assert!(t::END <= TELEMETRY_LEN) };
    }
}

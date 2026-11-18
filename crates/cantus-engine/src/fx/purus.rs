//! Purus, the real-time repair unit (ADR 0142), behind the engine's plug-in
//! contract. Owned by the wave-C builder.
//!
//! [`settings`] reads the table (`cantus_core::fx::purus`) into the DSP's settings. The
//! latency is the sum of the modules that are on (`cantus_dsp::purus::latency_for`: the
//! gentle de-hum's 50 ms look-ahead, the de-click's 767 samples, the de-reverb's `N − 1`),
//! read live by the engine each pass, so its delay compensation follows a module switched
//! on or off. With every module off (the defaults) the output is the input, to the bit and
//! undelayed.
//!
//! The telemetry carries the hum's readings whole and the de-reverb's 48 reduction bands a
//! page at a time (three pages of sixteen, the page the block count's): the editor keeps the
//! whole picture and refreshes each page as it comes.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::purus::{
    ClickSettings, DecayMode, HumMode, HumSettings, Purus, PurusSettings, VerbSettings, BANDS,
};

/// The telemetry's layout (what the editor draws).
pub mod t {
    use cantus_dsp::purus::hum::MAX_HARMONICS;
    /// The hum's fundamental now, and the nominal it centres on, Hz.
    pub const HUM_HZ: usize = 0;
    pub const HUM_NOMINAL: usize = 1;
    /// Which harmonics the gentle mode holds, a bit each (bit 0 the fundamental).
    pub const HELD: usize = 2;
    /// The decay in force and the decay estimated, s.
    pub const DECAY: usize = 3;
    pub const DECAY_ESTIMATED: usize = 4;
    /// Clicks rebuilt in the last block, and since the start modulo 2²⁰.
    pub const CLICKS_LAST: usize = 5;
    pub const CLICKS_TOTAL: usize = 6;
    /// The input's and the output's peaks in the block, and the level of what was taken
    /// out, dBFS.
    pub const IN_PEAK: usize = 7;
    pub const OUT_PEAK: usize = 8;
    pub const REMOVED: usize = 9;
    /// The latency, samples, and the rate.
    pub const LATENCY: usize = 10;
    pub const SAMPLE_RATE: usize = 11;
    /// The page of reduction bands this block carries, 0 … PAGES − 1.
    pub const PAGE: usize = 12;
    /// Each harmonic's level in the input (its 3 Hz band), dBFS, and its depth now, dB.
    pub const HUM_LEVEL: usize = 13;
    pub const HUM_DEPTH: usize = HUM_LEVEL + MAX_HARMONICS;
    /// The de-reverb's reduction (dB, ≥ 0) at the page's bands: band `PAGE + j·PAGES`.
    pub const VERB: usize = HUM_DEPTH + MAX_HARMONICS;
    pub const PER_PAGE: usize = 16;
    pub const PAGES: usize = 3;
    /// The end of Purus's numbers.
    pub const END: usize = VERB + PER_PAGE;
}

/// The reduction band the `j`th slot of page `page` carries.
pub fn page_band(page: usize, j: usize) -> Option<usize> {
    let i = page + j * t::PAGES;
    (i < BANDS).then_some(i)
}

/// Purus's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> PurusSettings {
    let on = |k: &str| spec.get(k) >= 0.5;
    PurusSettings {
        hum: HumSettings {
            on: on("hon"),
            mode: match spec.get("hmode").round() as i32 {
                1 => HumMode::Track,
                2 => HumMode::Auto,
                _ => HumMode::Fixed,
            },
            freq_hz: spec.get("hhz"),
            harmonics: spec.get("hharm").round().clamp(1.0, 16.0) as usize,
            depth_db: spec.get("hdepth"),
            width_hz: spec.get("hwidth"),
            gentle: on("hgentle"),
            mix: spec.get("hmix"),
            out_db: spec.get("hout"),
            delta: on("hdelta"),
        },
        click: ClickSettings {
            on: on("con"),
            // The table stops the knob at 0.7; a hand-edited file cannot pass it either.
            sensitivity: spec
                .get("csens")
                .min(cantus_core::fx::purus::SENSITIVITY_MAX),
            crackle: spec.get("crackle"),
            mix: spec.get("cmix"),
            out_db: spec.get("cout"),
            delta: on("cdelta"),
        },
        verb: VerbSettings {
            on: on("von"),
            amount_db: spec.get("vamount"),
            decay: if on("vdecay") {
                DecayMode::Estimate
            } else {
                DecayMode::Set
            },
            decay_s: spec.get("vt60"),
            direct_db: spec.get("vdirect"),
            smoothing: spec.get("vsmooth"),
            keep_ms: spec.get("vkeep"),
            mix: spec.get("vmix"),
            out_db: spec.get("vout"),
            delta: on("vdelta"),
        },
        out_db: spec.get("out"),
    }
}

#[derive(Clone)]
pub struct Unit {
    dsp: Box<Purus>,
    sample_rate: f32,
    /// Blocks processed: the reduction page the telemetry carries.
    blocks: u64,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "purus::Unit({} samples late)", self.dsp.latency())
    }
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Purus::new(sample_rate)),
            sample_rate,
            blocks: 0,
        }
    }

    /// The DSP, for the editor's drawing and the tests.
    pub fn dsp(&self) -> &Purus {
        &self.dsp
    }

    /// The de-reverb estimates its decay and has learned one.
    fn learned(&self) -> bool {
        let v = self.dsp.dereverb();
        let s = v.settings();
        s.on && s.decay == DecayMode::Estimate && v.estimated_decay_s() > 0.0
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
        self.blocks = self.blocks.wrapping_add(1);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    /// The DSP's rest, but not while the de-reverb estimates and has learned a decay: what
    /// it learned of the room outlives silence (the DSP keeps it on purpose), so a resting
    /// unit would not be as good as new (the contract's third condition, ADR 0031 §5.1).
    fn at_rest(&self) -> bool {
        self.dsp.at_rest() && !self.learned()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        if self.learned() {
            return Some("Purus: the room's decay it estimated outlives silence");
        }
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut out = [0.0; TELEMETRY_LEN];
        out[t::HUM_HZ] = m.hum_hz;
        out[t::HUM_NOMINAL] = m.hum_nominal_hz;
        out[t::HELD] = m
            .hum_held
            .iter()
            .enumerate()
            .fold(0u32, |b, (i, h)| b | (u32::from(*h) << i)) as f32;
        out[t::DECAY] = m.decay_s;
        out[t::DECAY_ESTIMATED] = m.decay_estimated_s;
        out[t::CLICKS_LAST] = m.clicks_last as f32;
        out[t::CLICKS_TOTAL] = (m.clicks_total % (1 << 20)) as f32;
        out[t::IN_PEAK] = m.in_peak_db;
        out[t::OUT_PEAK] = m.out_peak_db;
        out[t::REMOVED] = m.removed_db;
        out[t::LATENCY] = self.dsp.latency() as f32;
        out[t::SAMPLE_RATE] = self.sample_rate;
        let page = (self.blocks % t::PAGES as u64) as usize;
        out[t::PAGE] = page as f32;
        out[t::HUM_LEVEL..t::HUM_LEVEL + m.hum_level_db.len()].copy_from_slice(&m.hum_level_db);
        out[t::HUM_DEPTH..t::HUM_DEPTH + m.hum_depth_db.len()].copy_from_slice(&m.hum_depth_db);
        for j in 0..t::PER_PAGE {
            if let Some(b) = page_band(page, j) {
                out[t::VERB + j] = m.verb_reduction_db[b];
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pages_cover_every_band_once_and_fit() {
        let mut seen = [0u8; BANDS];
        for p in 0..t::PAGES {
            for j in 0..t::PER_PAGE {
                if let Some(b) = page_band(p, j) {
                    seen[b] += 1;
                }
            }
        }
        assert!(seen.iter().all(|n| *n == 1));
        const { assert!(t::END <= TELEMETRY_LEN) };
    }

    #[test]
    fn the_defaults_are_every_module_off() {
        let spec = EffectSpec::new(cantus_core::EffectKind::Repair);
        assert_eq!(settings(&spec), PurusSettings::default());
    }
}

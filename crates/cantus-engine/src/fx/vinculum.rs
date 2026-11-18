//! Vinculum, the bus compressor (ADR 0139), behind the engine's plug-in
//! contract. Owned by the wave-B builder.
//!
//! [`settings`] reads the table (`cantus_core::fx::vinculum`) into the DSP's settings: the
//! VCA's panel positions into its times and ratio, `rel` 4 into Auto, an `hpf` at 20 Hz
//! into off. The key is the slot's sidechain when one is routed (the engine's ordinary
//! sidechain path, `TAKES_SIDECHAIN`: the strip runner passes another strip's post-fader
//! output, or silence when it made none), the input otherwise. `out` is applied after the
//! DSP, gliding, and not at all at 0 dB (the null). Latency 0; at rest exactly when the DSP
//! is and the output gain has landed.
//!
//! The opto's Limit attack is 0.9 ms to 63 % at 16 dB over (the published average is
//! ~10 ms): the DSP is kept as measured until the owner has listened (ADR 0139).

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::vinculum::{ATTACKS_MS, HPF_OFF_HZ, RATIOS, RELEASES_MS, REL_AUTO};
use cantus_core::EffectSpec;
use cantus_dsp::modfx_line::Glide;
use cantus_dsp::vinculum::{Character, Model, Vinculum, VinculumSettings};

/// The telemetry's layout (what the editor draws), in the meter's units.
pub mod t {
    /// The reduction at the block's end and its deepest in the block, dB (≥ 0).
    pub const GR: usize = 0;
    pub const GR_PEAK: usize = 1;
    /// The input's and the output's peaks in the block, left and right, linear.
    pub const IN_L: usize = 2;
    pub const IN_R: usize = 3;
    pub const OUT_L: usize = 4;
    pub const OUT_R: usize = 5;
    /// The side chain's peak in the block (the key's, when routed), linear.
    pub const KEY: usize = 6;
    /// 1 while a key is routed.
    pub const KEYED: usize = 7;
    /// The model in force, 0 … 2.
    pub const MODEL: usize = 8;
    /// The sample rate.
    pub const SAMPLE_RATE: usize = 9;
    /// The end of Vinculum's numbers.
    pub const END: usize = 10;
}

fn pick(table: &[f32], index: f32) -> f32 {
    table[(index.round().max(0.0) as usize).min(table.len() - 1)]
}

/// Vinculum's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> VinculumSettings {
    let rel = spec.get("rel").round();
    let hpf = spec.get("hpf");
    VinculumSettings {
        model: Model::from_index(spec.get("model")),
        threshold_db: spec.get("thr"),
        ratio: pick(&RATIOS, spec.get("ratio")),
        attack_ms: pick(&ATTACKS_MS, spec.get("att")),
        release_ms: pick(&RELEASES_MS, rel.min(3.0)),
        auto_release: rel >= REL_AUTO,
        character: if spec.get("char") >= 0.5 {
            Character::Limit
        } else {
            Character::Compress
        },
        mu_time: spec.get("time").round().clamp(0.0, 5.0) as usize,
        sc_hpf_hz: if hpf <= HPF_OFF_HZ { 0.0 } else { hpf },
        mix: spec.get("mix"),
        makeup_db: spec.get("makeup"),
        colour: spec.get("colour"),
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Box<Vinculum>,
    out: Glide,
    sample_rate: f32,
    /// The last block's meter (the DSP's peak is taken each block).
    gr: f32,
    gr_peak: f32,
    peaks: [f32; 5],
    keyed: bool,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Vinculum::new(sample_rate)),
            out: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            sample_rate,
            gr: 0.0,
            gr_peak: 0.0,
            peaks: [0.0; 5],
            keyed: false,
        }
    }

    /// The DSP, for the editor's drawing and the tests.
    pub fn dsp(&self) -> &Vinculum {
        &self.dsp
    }
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs()))
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
        let db = spec.get("out");
        self.out.target = if db == 0.0 {
            1.0
        } else {
            cantus_dsp::math::db_to_gain(db.clamp(-24.0, 24.0))
        };
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        self.peaks[0] = peak(left);
        self.peaks[1] = peak(right);
        self.peaks[4] = match key {
            Some((kl, kr)) => peak(kl).max(peak(kr)),
            None => self.peaks[0].max(self.peaks[1]),
        };
        self.keyed = key.is_some();
        self.dsp.process_keyed(left, right, key);
        let m = self.dsp.take_meter();
        self.gr = m.gr_db;
        self.gr_peak = m.gr_peak_db;
        if !(self.out.still() && self.out.value == 1.0) {
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                let g = self.out.next();
                *l *= g;
                *r *= g;
            }
        }
        self.peaks[2] = peak(left);
        self.peaks[3] = peak(right);
    }

    /// fx/final — at a start, a seek or a loop's wrap: the output at its target (the only
    /// glide: the DSP takes its settings at once). The detector and the cell are kept.
    fn snap(&mut self) {
        self.out.jump(self.out.target);
    }

    fn reset(&mut self) {
        self.out.jump(self.out.target);
        self.dsp.reset();
        self.gr = 0.0;
        self.gr_peak = 0.0;
        self.peaks = [0.0; 5];
    }

    fn at_rest(&self) -> bool {
        self.out.still() && self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        if !self.out.still() {
            return Some("the output level is gliding");
        }
        (!self.dsp.at_rest()).then_some("a reduction or the colour's filters")
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
        self.peaks = [0.0; 5];
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn gain_reduction_db(&self) -> Option<f32> {
        Some(self.gr)
    }

    fn telemetry(&self) -> Telemetry {
        let mut out = [0.0; TELEMETRY_LEN];
        out[t::GR] = self.gr;
        out[t::GR_PEAK] = self.gr_peak;
        out[t::IN_L] = self.peaks[0];
        out[t::IN_R] = self.peaks[1];
        out[t::OUT_L] = self.peaks[2];
        out[t::OUT_R] = self.peaks[3];
        out[t::KEY] = self.peaks[4];
        out[t::KEYED] = if self.keyed { 1.0 } else { 0.0 };
        out[t::MODEL] = match self.dsp.settings().model {
            Model::Vca => 0.0,
            Model::Opto => 1.0,
            Model::VariMu => 2.0,
        };
        out[t::SAMPLE_RATE] = self.sample_rate;
        out
    }
}

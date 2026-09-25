//! Spatium (the reverb) behind the engine's plug-in contract. Owned by the reverb.
//!
//! [`settings`] maps the parameters (`cantus_core::fx::space`) onto the DSP's settings
//! (`cantus_dsp::spatium`); the editor calls the same function, so what it draws is what
//! the audio thread is told. The DSP re-fits its late field only when something that
//! decides it changed, so a snapshot that moves another strip's fader costs nothing here.
//!
//! ## Telemetry (64 floats)
//!
//! | Index | What |
//! |---|---|
//! | [`tel::WET_L`], [`tel::WET_R`] | wet peaks, linear, held and falling ~20 dB/s |
//! | [`tel::INPUT`] | the input's (or key's) peak, same ballistics |
//! | [`tel::DUCK_DB`] | ducking's gain reduction now, dB (positive) |
//! | [`tel::FREEZE`] | how frozen the tank is, 0…1 |
//! | [`tel::CORRELATION`] | the wet's L/R correlation, −1…1 |
//! | [`tel::PREDELAY_MS`] | the pre-delay in force (it glides), ms |
//! | [`tel::LINE_MS`] | the late lines' mean length in force, ms |
//! | [`tel::MOD_MS`] | the modulation depth in force, ms |
//! | [`tel::KEYED`] | 1 when a sidechain key is routed |
//! | [`tel::ALIVE`] | always 1: tells a running engine from no engine |
//! | [`tel::ACHIEVED`] … +8 | the decay the late field achieves at 63 Hz … 8 kHz (octaves), s |

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::space::{DECAY_BANDS, POST_BANDS};
use cantus_core::{EffectKind, EffectSpec};
use cantus_dsp::spatium::model::{DecayShape, Style};
use cantus_dsp::spatium::{Spatium, SpatiumSettings, OCTAVES};
use cantus_dsp::{EqShape, Placement};

/// Where each number sits in the telemetry.
pub mod tel {
    pub const WET_L: usize = 0;
    pub const WET_R: usize = 1;
    pub const INPUT: usize = 2;
    pub const DUCK_DB: usize = 3;
    pub const FREEZE: usize = 4;
    pub const CORRELATION: usize = 5;
    pub const PREDELAY_MS: usize = 6;
    pub const LINE_MS: usize = 7;
    pub const MOD_MS: usize = 8;
    pub const KEYED: usize = 9;
    pub const ALIVE: usize = 10;
    pub const ACHIEVED: usize = 16;
}

/// The post EQ's shapes, by the index its `shape` parameter stores.
pub const POST_SHAPES: [EqShape; 5] = [
    EqShape::Bell,
    EqShape::LowShelf,
    EqShape::HighShelf,
    EqShape::LowCut,
    EqShape::HighCut,
];

struct Raw {
    s: SpatiumSettings,
    d_used: [bool; DECAY_BANDS],
    d_on: [bool; DECAY_BANDS],
    e_used: [bool; POST_BANDS],
    e_on: [bool; POST_BANDS],
}

fn flag(v: f32) -> bool {
    v >= 0.5
}

fn apply(raw: &mut Raw, key: &str, v: f32) {
    let s = &mut raw.s;
    match key {
        "style" => s.style = Style::from_index(v.round().max(0.0) as usize),
        "space" => s.space = v,
        "decay" => s.decay_s = v,
        "predelay" => s.predelay_ms = v,
        "distance" => s.distance = v,
        "character" => s.character = v,
        "bright" => s.brightness = v,
        "width" => s.width = v,
        "mix" => s.mix = v,
        "freeze" => s.freeze = flag(v),
        "er" => s.er_db = v,
        "er_size" => s.er_size = v,
        "duck" => s.duck_db = v,
        "duck_thr" => s.duck_thr_db = v,
        "duck_att" => s.duck_att_ms = v,
        "duck_rel" => s.duck_rel_ms = v,
        "deq_on" => s.decay_eq_on = flag(v),
        "peq_on" => s.post_eq_on = flag(v),
        "peq_auto" => s.post_auto_gain = flag(v),
        _ => {
            let Some((head, field)) = key.split_once('.') else {
                return;
            };
            let (kind, n) = head.split_at(1.min(head.len()));
            let Ok(n) = n.parse::<usize>() else {
                return;
            };
            match kind {
                "d" if n < DECAY_BANDS => {
                    let b = &mut s.decay_bands[n];
                    match field {
                        "used" => raw.d_used[n] = flag(v),
                        "on" => raw.d_on[n] = flag(v),
                        "shape" => b.shape = DecayShape::from_index(v.round().max(0.0) as usize),
                        "freq" => b.freq = v,
                        "mult" => b.mult = v,
                        "q" => b.q = v,
                        _ => {}
                    }
                }
                "e" if n < POST_BANDS => {
                    let b = &mut s.post_bands[n];
                    match field {
                        "used" => raw.e_used[n] = flag(v),
                        "on" => raw.e_on[n] = flag(v),
                        "shape" => {
                            b.shape = POST_SHAPES[(v.round().max(0.0) as usize).min(4)];
                        }
                        "freq" => b.freq = v,
                        "gain" => b.gain_db = v,
                        "q" => b.q = v,
                        "slope" => b.slope = v,
                        "place" => b.placement = Placement::from_index(v.round().max(0.0) as usize),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

/// The DSP's settings for a spec: every parameter's default, then what the spec stores.
/// Allocation-free (it runs on the audio thread with every snapshot).
pub fn settings(spec: &EffectSpec) -> SpatiumSettings {
    let mut raw = Raw {
        s: SpatiumSettings::default(),
        d_used: [false; DECAY_BANDS],
        d_on: [true; DECAY_BANDS],
        e_used: [false; POST_BANDS],
        e_on: [true; POST_BANDS],
    };
    for info in EffectKind::Space.params() {
        apply(&mut raw, info.key, info.default);
    }
    for (k, v) in &spec.params {
        apply(&mut raw, k, *v);
    }
    for n in 0..DECAY_BANDS {
        raw.s.decay_bands[n].active = raw.d_used[n] && raw.d_on[n];
    }
    for n in 0..POST_BANDS {
        raw.s.post_bands[n].active = raw.e_used[n] && raw.e_on[n];
    }
    raw.s
}

#[derive(Debug)]
pub struct Unit {
    dsp: Spatium,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Spatium::new(sample_rate),
        }
    }
}

impl FxUnit for Unit {
    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right, key);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut t = [0.0; TELEMETRY_LEN];
        t[tel::WET_L] = m.wet_peak[0];
        t[tel::WET_R] = m.wet_peak[1];
        t[tel::INPUT] = m.in_peak;
        t[tel::DUCK_DB] = m.duck_db;
        t[tel::FREEZE] = m.freeze;
        t[tel::CORRELATION] = m.correlation;
        t[tel::PREDELAY_MS] = m.predelay_ms;
        t[tel::LINE_MS] = m.line_ms;
        t[tel::MOD_MS] = m.mod_depth_ms;
        t[tel::KEYED] = if m.keyed { 1.0 } else { 0.0 };
        t[tel::ALIVE] = 1.0;
        for (i, a) in m.achieved.iter().enumerate().take(OCTAVES.len()) {
            t[tel::ACHIEVED + i] = *a;
        }
        t
    }
}

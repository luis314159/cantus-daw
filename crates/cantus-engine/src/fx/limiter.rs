//! Terminus (the limiter) behind the engine's plug-in contract. Owned by the limiter.
//!
//! [`settings`] is the one reading of the parameters (`cantus_core::fx::limiter`) into
//! the DSP's terms (`cantus_dsp::terminus::LimSettings`); the editor uses it too, so what
//! it draws and what plays come from the same numbers.
//!
//! ## Telemetry (64 floats, `Meters::fx` while the editor is open)
//!
//! | Index | What |
//! |---|---|
//! | 0 | columns published since the stream began (wraps at `T_WRAP`) |
//! | 1 | latency, samples |
//! | 2 | sample rate |
//! | 3 | the Match monitor's gain, dB (0 when off) |
//! | 4, 5 | the last column's output true peak, left and right, dBTP |
//! | 6, 7 | the last column's largest gain reduction, left and right, dB |
//! | 8 … 62 | a ring of the last 11 columns, 5 floats each: K-weighted output energy, input peak after the gain (dBFS), output true peak (dBTP), gain reduction (dB), clipper reduction (dB) |
//!
//! A column is 10 ms, counted from the start of the stream (`cantus_dsp::terminus::Column`
//! reads one). The ring holds 110 ms, so the editor loses nothing unless a frame comes
//! that late; it tells a gap from the counter.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::terminus::{LimSettings, LimStyle, Monitor, Shaping, Terminus, TELEMETRY};

const _: () = assert!(
    TELEMETRY == TELEMETRY_LEN,
    "the telemetry block is 64 floats"
);

/// The limiter's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> LimSettings {
    let index = |key: &str| spec.get(key).round().max(0.0) as usize;
    LimSettings {
        gain_db: spec.get("gain"),
        ceiling_db: spec.get("ceiling"),
        style: LimStyle::from_index(index("style")),
        lookahead_ms: spec.get("look"),
        attack_ms: spec.get("att"),
        release_ms: spec.get("rel"),
        link_transients: spec.get("link_t"),
        link_release: spec.get("link_r"),
        knee_db: spec.get("knee"),
        true_peak: spec.get("tp") >= 0.5,
        clip: spec.get("clip"),
        dc_filter: spec.get("dc") >= 0.5,
        monitor: Monitor::from_index(index("monitor")),
        delta: spec.get("delta") >= 0.5,
        dither: index("dither"),
        shaping: Shaping::from_index(index("shape")),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Box<Terminus>,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Terminus::new(sample_rate)),
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

    fn at_rest(&self) -> bool {
        self.dsp.at_rest()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    /// The deeper of the reduction now and the last column's largest (telemetry 6, 7), so
    /// a meter read once a block does not miss a peak between reads.
    fn gain_reduction_db(&self) -> Option<f32> {
        let [l, r] = self.dsp.gain_reduction_db();
        let t = self.dsp.telemetry();
        Some(l.abs().max(r.abs()).max(t[6].abs()).max(t[7].abs()))
    }

    fn telemetry(&self) -> Telemetry {
        self.dsp.telemetry()
    }
}

//! Multiplex (the multiband) behind the engine's plug-in contract. Owned by the multiband;
//! nothing else edits this file.
//!
//! [`settings`] reads the sparse parameter map (`cantus_core::fx::multiband`) into the
//! DSP's [`Settings`] without allocating — it walks the keys that are stored and parses
//! `b{n}.{field}` in place — so `configure` is safe on the audio thread. The editor draws
//! from the same function, so the picture and the sound come from one reading of the spec.
//!
//! ## Telemetry layout (64 floats)
//!
//! | Index | What |
//! |---|---|
//! | 0–7 | each band's dynamic gain, dB, by band number (held: the larger in size of the last one or two 1024-sample windows) |
//! | 8–15 | each band's detector level, dB (held maximum) |
//! | 16, 17 | input peak, left and right, linear (held) |
//! | 18, 19 | output peak, left and right, linear (held) |
//! | 20 | latency, samples |
//! | 21 | how much of the output comes from the split, 0…1 |
//! | 22 | the sample rate |

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::multiband::{parse_key, MB_BAND_FIELDS};
use cantus_core::EffectSpec;
use cantus_dsp::multiplex::{
    BandSettings, Listen, Mode, Multiplex, Placement, Settings, Slope, MAX_BANDS,
};

/// Where each group of numbers starts in the telemetry.
pub const T_GAIN: usize = 0;
pub const T_LEVEL: usize = 8;
pub const T_INPUT: usize = 16;
pub const T_OUTPUT: usize = 18;
pub const T_LATENCY: usize = 20;
pub const T_SPLIT: usize = 21;
pub const T_RATE: usize = 22;

/// A Multiplex's parameters as the DSP wants them. No allocation.
pub fn settings(spec: &EffectSpec) -> Settings {
    let mut s = Settings::default();
    for (key, value) in &spec.params {
        let v = *value;
        if !v.is_finite() {
            continue;
        }
        if let Some((b, f)) = parse_key(key) {
            let (name, _, min, max, _, _, _, _) = MB_BAND_FIELDS[f];
            let v = v.clamp(min, max);
            let band = &mut s.bands[b];
            let on = v >= 0.5;
            match name {
                "used" => band.used = on,
                "from" => band.from_hz = v,
                "on" => band.on = on,
                "solo" => band.solo = on,
                "mute" => band.mute = on,
                "thr" => band.thr_db = v,
                "ratio" => band.ratio = v,
                "range" => band.range_db = v,
                "thr_lo" => band.thr_lo_db = v,
                "ratio_lo" => band.ratio_lo = v,
                "range_lo" => band.range_lo_db = v,
                "knee" => band.knee_db = v,
                "att" => band.attack_ms = v,
                "rel" => band.release_ms = v,
                "la" => band.lookahead_ms = v,
                "gain" => band.gain_db = v,
                "place" => band.placement = Placement::from_index(v.round() as usize),
                "key" => band.key_ext = on,
                "key_free" => band.key_free = on,
                "key_lo" => band.key_lo_hz = v,
                "key_hi" => band.key_hi_hz = v,
                _ => {}
            }
            continue;
        }
        match key.as_str() {
            "mode" => s.mode = Mode::from_index(v.round().max(0.0) as usize),
            "slope" => s.slope = Slope::from_db(v),
            "out" => s.output_db = v.clamp(-24.0, 24.0),
            "listen" => s.listen = Listen::from_index(v.round().max(0.0) as usize),
            _ => {}
        }
    }
    s
}

/// One band's settings, for the editor.
pub fn band(spec: &EffectSpec, b: usize) -> BandSettings {
    settings(spec).bands[b.min(MAX_BANDS - 1)]
}

#[derive(Clone, Debug)] // r34:listen: Clone for fork
pub struct Unit {
    dsp: Multiplex,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Multiplex::new(sample_rate),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    } // r34:listen
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

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        telemetry_of(&self.dsp)
    }
}

/// The telemetry of a running Multiplex (also what the editor's demo feeds itself).
pub fn telemetry_of(dsp: &Multiplex) -> Telemetry {
    let m = dsp.meter();
    let mut t = [0.0; TELEMETRY_LEN];
    t[T_GAIN..T_GAIN + MAX_BANDS].copy_from_slice(&m.gain_db);
    t[T_LEVEL..T_LEVEL + MAX_BANDS].copy_from_slice(&m.level_db);
    t[T_INPUT..T_INPUT + 2].copy_from_slice(&m.input);
    t[T_OUTPUT..T_OUTPUT + 2].copy_from_slice(&m.output);
    t[T_LATENCY] = dsp.latency() as f32;
    t[T_SPLIT] = m.split;
    t[T_RATE] = dsp.sample_rate();
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_spec_reads_into_settings_field_by_field() {
        let mut spec = EffectSpec::new(EffectKind::Multiband);
        let s = settings(&spec);
        assert_eq!(s, Settings::default(), "an empty spec is the DSP's default");
        for (k, v) in [
            ("b2.used", 1.0),
            ("b2.from", 250.0),
            ("b2.thr_lo", -45.0),
            ("b2.ratio_lo", 3.0),
            ("b2.place", 2.0),
            ("b2.key", 1.0),
            ("mode", 1.0),
            ("slope", 48.0),
            ("out", -3.0),
            ("listen", 1.0),
        ] {
            spec.set(k, v).unwrap();
        }
        let s = settings(&spec);
        let b = s.bands[2];
        assert!(b.used && b.key_ext);
        assert_eq!(b.from_hz, 250.0);
        assert_eq!(b.thr_lo_db, -45.0);
        assert_eq!(b.ratio_lo, 3.0);
        assert_eq!(b.placement, Placement::Side);
        assert_eq!(s.mode, Mode::Split);
        assert_eq!(s.slope, Slope::S48);
        assert_eq!(s.output_db, -3.0);
        assert_eq!(s.listen, Listen::Delta);
        // Every default in the parameter table is the DSP's default.
        let d = BandSettings::default();
        let get = |f: &str| {
            MB_BAND_FIELDS
                .iter()
                .find(|x| x.0 == f)
                .map(|x| x.4)
                .unwrap()
        };
        assert_eq!(get("from"), d.from_hz);
        assert_eq!(get("thr"), d.thr_db);
        assert_eq!(get("ratio"), d.ratio);
        assert_eq!(get("range"), d.range_db);
        assert_eq!(get("thr_lo"), d.thr_lo_db);
        assert_eq!(get("ratio_lo"), d.ratio_lo);
        assert_eq!(get("range_lo"), d.range_lo_db);
        assert_eq!(get("knee"), d.knee_db);
        assert_eq!(get("att"), d.attack_ms);
        assert_eq!(get("rel"), d.release_ms);
        assert_eq!(get("la"), d.lookahead_ms);
        assert_eq!(get("gain"), d.gain_db);
        assert_eq!(get("key_lo"), d.key_lo_hz);
        assert_eq!(get("key_hi"), d.key_hi_hz);
        assert_eq!(get("on"), 1.0);
    }
}

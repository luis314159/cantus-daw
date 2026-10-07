//! Culmen (the maximizer) behind the engine's plug-in contract. Owned by Culmen.
//!
//! [`settings`] is the one reading of the parameters (`cantus_core::fx::maximizer`) into
//! the DSP's terms (`cantus_dsp::culmen::MaxSettings`), without allocating — it walks the
//! keys that are stored and parses `{band}.{field}` in place — so `configure` is safe on
//! the audio thread. The editor draws from the same function, so the picture and the
//! sound come from one reading of the spec.
//!
//! ## Telemetry (64 floats, `Meters::fx` while the editor is open)
//!
//! The layout is `cantus_dsp::culmen::telemetry`'s: a header (columns published, latency,
//! sample rate, the match gain, the clipper, each band's detector level now, the
//! brickwall's reduction now — slot 9, the number a strip's GR meter wants) and a ring of
//! the last six 10 ms columns: output and input loudness energy, input and output peak,
//! the brickwall's reduction and each band's gain.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::maximizer::{parse_key, BAND_FIELDS};
use cantus_core::EffectSpec;
use cantus_dsp::culmen::{telemetry, Culmen, MaxSettings};
use cantus_dsp::multiplex::{Mode, Placement, Slope};
use cantus_dsp::terminus::{LimStyle, Monitor, Shaping};

const _: () = assert!(
    telemetry::LEN == TELEMETRY_LEN,
    "the telemetry block is 64 floats"
);

/// Culmen's parameters as the DSP wants them. No allocation.
pub fn settings(spec: &EffectSpec) -> MaxSettings {
    let mut s = MaxSettings::default();
    for (key, value) in &spec.params {
        let v = *value;
        if !v.is_finite() {
            continue;
        }
        if let Some((b, f)) = parse_key(key) {
            let (name, _, min, max, _, _, _, _) = BAND_FIELDS[f];
            let v = v.clamp(min, max);
            let band = &mut s.bands[b];
            let on = v >= 0.5;
            match name {
                "on" => band.on = on,
                "solo" => band.solo = on,
                "thr" => band.thr_db = v,
                "ratio" => band.ratio = v,
                "range" => band.range_db = v,
                "thr_lo" => band.thr_lo_db = v,
                "ratio_lo" => band.ratio_lo = v,
                "range_lo" => band.range_lo_db = v,
                "knee" => band.knee_db = v,
                "att" => band.attack_ms = v,
                "rel" => band.release_ms = v,
                "gain" => band.gain_db = v,
                "place" => band.placement = Placement::from_index(v.round() as usize),
                _ => {}
            }
            continue;
        }
        let Some(info) = spec.kind.param(key) else {
            continue;
        };
        let v = v.clamp(info.min, info.max);
        let index = v.round().max(0.0) as usize;
        let on = v >= 0.5;
        let lim = &mut s.limiter;
        match key.as_str() {
            "gain" => s.drive_db = v,
            "ceiling" => lim.ceiling_db = v,
            "style" => lim.style = LimStyle::from_index(index),
            "look" => lim.lookahead_ms = v,
            "lim_att" => lim.attack_ms = v,
            "lim_rel" => lim.release_ms = v,
            "link_t" => lim.link_transients = v,
            "link_r" => lim.link_release = v,
            "lim_knee" => lim.knee_db = v,
            "tp" => lim.true_peak = on,
            "clip" => lim.clip = v,
            "dc" => lim.dc_filter = on,
            "bands" => s.count = index.clamp(1, 3),
            "x1" => s.x1_hz = v,
            "x2" => s.x2_hz = v,
            "xmode" => s.mode = Mode::from_index(index),
            "slope" => s.slope = Slope::from_db(v),
            "delta" => s.delta = on,
            "monitor" => s.monitor = Monitor::from_index(index),
            "dither" => s.dither = index,
            "shape" => s.shaping = Shaping::from_index(index),
            _ => {}
        }
    }
    s
}

#[derive(Debug)]
pub struct Unit {
    dsp: Box<Culmen>,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Culmen::new(sample_rate)),
        }
    }
}

impl FxUnit for Unit {
    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
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
        self.dsp.telemetry()
    }

    /// The master's reduction meter (ADR 0026), as Terminus answers it: the deepest of
    /// the brickwall's reduction now, the last 10 ms column's largest, and the deepest
    /// band cut in that column (a band lifting is not a reduction). Positive dB.
    fn gain_reduction_db(&self) -> Option<f32> {
        let t = self.dsp.telemetry();
        let mut gr = self.dsp.reduction_now_db().abs();
        // The column before the counter (before any column, a ring of zeros).
        let seq = t[telemetry::SEQ] as u32;
        let last = (seq + telemetry::WRAP - 1) % telemetry::WRAP;
        let c = telemetry::Column::read(&t, last);
        gr = gr.max(c.limit_db.abs());
        for b in c.bands_db {
            gr = gr.max(-b);
        }
        Some(gr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::fx::maximizer::{BANDS, GLOBALS};
    use cantus_core::EffectKind;
    use cantus_dsp::culmen::{FULL, HI, LO, MID};

    #[test]
    fn an_empty_spec_is_the_dsps_default_and_every_key_lands() {
        let mut spec = EffectSpec::new(EffectKind::Maximizer);
        assert_eq!(settings(&spec), MaxSettings::default());
        for (k, v) in [
            ("gain", 6.0),
            ("ceiling", -0.3),
            ("style", 2.0),
            ("look", 1.0),
            ("lim_att", 3.0),
            ("lim_rel", 90.0),
            ("link_t", 0.2),
            ("link_r", 0.5),
            ("lim_knee", 2.0),
            ("tp", 0.0),
            ("clip", 0.5),
            ("dc", 1.0),
            ("bands", 2.0),
            ("x1", 200.0),
            ("x2", 6_000.0),
            ("xmode", 2.0),
            ("slope", 48.0),
            ("delta", 1.0),
            ("monitor", 2.0),
            ("dither", 5.0),
            ("shape", 1.0),
            ("mid.thr", -20.0),
            ("hi.ratio_lo", 3.0),
            ("lo.place", 1.0),
            ("all.gain", -2.0),
            ("lo.solo", 1.0),
        ] {
            spec.set(k, v).unwrap();
        }
        let s = settings(&spec);
        assert_eq!(s.drive_db, 6.0);
        assert_eq!(s.limiter.ceiling_db, -0.3);
        assert_eq!(s.limiter.style, LimStyle::Loud);
        assert_eq!(s.limiter.lookahead_ms, 1.0);
        assert_eq!(s.limiter.attack_ms, 3.0);
        assert_eq!(s.limiter.release_ms, 90.0);
        assert_eq!(s.limiter.link_transients, 0.2);
        assert_eq!(s.limiter.link_release, 0.5);
        assert_eq!(s.limiter.knee_db, 2.0);
        assert!(!s.limiter.true_peak && s.limiter.dc_filter);
        assert_eq!(s.limiter.clip, 0.5);
        assert_eq!((s.count, s.x1_hz, s.x2_hz), (2, 200.0, 6_000.0));
        assert_eq!((s.mode, s.slope), (Mode::Linear, Slope::S48));
        assert!(s.delta);
        assert_eq!(s.monitor, Monitor::Match);
        assert_eq!((s.dither, s.shaping), (5, Shaping::Basic));
        assert_eq!(s.bands[MID].thr_db, -20.0);
        assert_eq!(s.bands[HI].ratio_lo, 3.0);
        assert_eq!(s.bands[LO].placement, Placement::Mid);
        assert!(s.bands[LO].solo);
        assert_eq!(s.bands[FULL].gain_db, -2.0);
        // Every default in the table is the DSP's default.
        let d = MaxSettings::default();
        let get = |k: &str| GLOBALS.iter().find(|p| p.key == k).unwrap().default;
        assert_eq!(get("gain"), d.drive_db);
        assert_eq!(get("ceiling"), d.limiter.ceiling_db);
        assert_eq!(get("look"), d.limiter.lookahead_ms);
        assert_eq!(get("lim_att"), d.limiter.attack_ms);
        assert_eq!(get("lim_rel"), d.limiter.release_ms);
        assert_eq!(get("link_t"), d.limiter.link_transients);
        assert_eq!(get("link_r"), d.limiter.link_release);
        assert_eq!(get("x1"), d.x1_hz);
        assert_eq!(get("x2"), d.x2_hz);
        assert_eq!(get("bands") as usize, d.count);
        for (b, _) in BANDS.iter().enumerate() {
            for f in BAND_FIELDS {
                let (name, default) = (f.0, f.4);
                let band = &d.bands[b];
                let dsp = match name {
                    "on" => band.on as u8 as f32,
                    "solo" => band.solo as u8 as f32,
                    "thr" => band.thr_db,
                    "ratio" => band.ratio,
                    "range" => band.range_db,
                    "thr_lo" => band.thr_lo_db,
                    "ratio_lo" => band.ratio_lo,
                    "range_lo" => band.range_lo_db,
                    "knee" => band.knee_db,
                    "att" => band.attack_ms,
                    "rel" => band.release_ms,
                    "gain" => band.gain_db,
                    "place" => band.placement.index() as f32,
                    other => panic!("{other} is not read"),
                };
                assert_eq!(dsp, default, "{name}");
            }
        }
    }
}

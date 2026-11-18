//! Caelum, the shimmer reverb (ADR 0138), behind the engine's plug-in
//! contract. Owned by the wave-A builder.
//!
//! [`settings`] reads the table (`cantus_core::fx::caelum`) into the DSP's settings; a
//! synced pre-delay (`psync`) is its note value at the song's tempo, from the position the
//! engine tells the unit. Ducking listens to the key when one is routed (the engine's
//! ordinary sidechain path, `TAKES_SIDECHAIN`), to the input otherwise. `out` is applied
//! after the DSP, gliding, and not at all at 0 dB (the null). Latency 0; at rest exactly
//! when the DSP is (its tank drained) and the output gain has landed.

use super::{FxUnit, SongPosition, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::caelum::SYNC_BEATS;
use cantus_core::EffectSpec;
use cantus_dsp::caelum::{sync_ms, Caelum, CaelumSettings, Lines, OCTAVES};
use cantus_dsp::modfx_line::Glide;

/// The telemetry's layout (what the editor draws), in the meter's units.
pub mod t {
    /// The wet's peaks, left and right, linear.
    pub const WET_L: usize = 0;
    pub const WET_R: usize = 1;
    /// The input's (or the key's) peak, linear.
    pub const INPUT: usize = 2;
    /// The ducking's reduction now, dB (positive).
    pub const DUCK: usize = 3;
    /// Freeze 0 … 1, the bloom's share 0 … 1.
    pub const FREEZE: usize = 4;
    pub const BLOOM: usize = 5;
    /// The tank's energy, dB; the shimmer's share of it, 0 … 1; the hold's and the
    /// governor's trim, dB.
    pub const ENERGY: usize = 6;
    pub const SHIMMER: usize = 7;
    pub const TRIM: usize = 8;
    /// The pre-delay now, ms; the lines' mean length and the modulation's depth, ms.
    pub const PREDELAY: usize = 9;
    pub const LINE: usize = 10;
    pub const DEPTH: usize = 11;
    /// 1 while a key is routed.
    pub const KEYED: usize = 12;
    /// The decay the tank achieves at each of the eight octaves (63 Hz … 8 kHz), s
    /// (`f32::INFINITY` while infinite).
    pub const ACHIEVED: usize = 13;
    /// The sample rate.
    pub const SAMPLE_RATE: usize = 21;
    /// The end of Caelum's numbers.
    pub const END: usize = 22;
}

/// Caelum's parameters as the DSP wants them (the pre-delay as set; [`Unit`] syncs it).
pub fn settings(spec: &EffectSpec) -> CaelumSettings {
    let on = |k: &str| spec.get(k) >= 0.5;
    CaelumSettings {
        size: spec.get("size"),
        decay_s: spec.get("decay"),
        infinite: on("inf"),
        sustain: spec.get("isus"),
        predelay_ms: spec.get("pre"),
        diffusion: spec.get("diff"),
        high_hz: spec.get("hhz"),
        high_decay: spec.get("hdec"),
        low_hz: spec.get("lhz"),
        low_decay: spec.get("ldec"),
        mod_depth: spec.get("mdepth"),
        mod_rate_hz: spec.get("mrate"),
        shimmer: spec.get("shim"),
        pitch_a: spec.get("pa"),
        pitch_b: spec.get("pb"),
        pitch_b_level: spec.get("pblvl"),
        shimmer_low_hz: spec.get("slo"),
        shimmer_high_hz: spec.get("shi"),
        bloom_ms: spec.get("bloom"),
        freeze: on("freeze"),
        duck_db: spec.get("duck"),
        duck_thr_db: spec.get("dthr"),
        duck_att_ms: spec.get("datt"),
        duck_rel_ms: spec.get("drel"),
        low_cut_hz: spec.get("lcut"),
        high_cut_hz: spec.get("hcut"),
        width: spec.get("width"),
        lines: if spec.get("lines") >= 0.5 {
            Lines::Sixteen
        } else {
            Lines::Eight
        },
        mix: spec.get("mix"),
    }
}

/// The note value a `psync` index names, quarter notes (`None`: free).
pub fn sync_beats(index: f32) -> Option<f32> {
    let i = index.round();
    (i >= 1.0).then(|| SYNC_BEATS[(i as usize - 1).min(SYNC_BEATS.len() - 1)])
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Box<Caelum>,
    set: CaelumSettings,
    sync: Option<f32>,
    tempo: f64,
    out: Glide,
    sample_rate: f32,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let set = CaelumSettings::default();
        let mut dsp = Box::new(Caelum::new(sample_rate));
        dsp.set(&set);
        dsp.reset();
        Unit {
            dsp,
            set,
            sync: None,
            tempo: 120.0,
            out: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            sample_rate,
        }
    }

    /// The DSP, for the editor's drawing and the tests.
    pub fn dsp(&self) -> &Caelum {
        &self.dsp
    }

    /// The settings as the DSP has them, the pre-delay synced.
    fn apply(&mut self) {
        let mut s = self.set;
        if let Some(beats) = self.sync {
            s.predelay_ms = sync_ms(beats, self.tempo as f32);
        }
        self.dsp.set(&s);
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.set = settings(spec);
        self.sync = sync_beats(spec.get("psync"));
        let db = spec.get("out");
        self.out.target = if db == 0.0 {
            1.0
        } else {
            cantus_dsp::math::db_to_gain(db.clamp(-24.0, 24.0))
        };
        self.apply();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right, key);
        if self.out.still() && self.out.value == 1.0 {
            return;
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let g = self.out.next();
            *l *= g;
            *r *= g;
        }
    }

    /// fx/final — at a start, a seek or a loop's wrap: every glide at its target.
    fn snap(&mut self) {
        self.out.jump(self.out.target);
        self.dsp.snap();
    }

    fn reset(&mut self) {
        self.out.jump(self.out.target);
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.out.still() && self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        if !self.out.still() {
            return Some("the output level is gliding");
        }
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn follows_song(&self) -> bool {
        self.sync.is_some()
    }

    fn set_position(&mut self, position: &SongPosition) {
        if self.sync.is_some() && position.tempo_bpm > 0.0 && position.tempo_bpm != self.tempo {
            self.tempo = position.tempo_bpm;
            // A tank at rest holds nothing a glide would carry: the synced pre-delay lands at
            // once (a resting unit and a reset one are the same to the bit), so a song — or a
            // host — that starts at its own tempo plays its note from the first sample.
            let resting = self.dsp.at_rest();
            self.apply();
            if resting {
                self.dsp.reset();
            }
        }
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut out = [0.0; TELEMETRY_LEN];
        out[t::WET_L] = m.wet_peak[0];
        out[t::WET_R] = m.wet_peak[1];
        out[t::INPUT] = m.in_peak;
        out[t::DUCK] = m.duck_db;
        out[t::FREEZE] = m.freeze;
        out[t::BLOOM] = m.bloom;
        out[t::ENERGY] = m.energy_db;
        out[t::SHIMMER] = m.shimmer;
        out[t::TRIM] = m.trim_db;
        out[t::PREDELAY] = m.predelay_ms;
        out[t::LINE] = m.line_ms;
        out[t::DEPTH] = m.mod_depth_ms;
        out[t::KEYED] = if m.keyed { 1.0 } else { 0.0 };
        for (k, a) in m.achieved.iter().enumerate().take(OCTAVES.len()) {
            out[t::ACHIEVED + k] = *a;
        }
        out[t::SAMPLE_RATE] = self.sample_rate;
        out
    }
}

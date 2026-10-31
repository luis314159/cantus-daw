//! Organum — the tonewheel organ and rotary speaker: the engine's unit. Owned by this
//! instrument's builder (ADR 0023).
//!
//! [`settings`] maps the parameters (`cantus_core::inst::organ`) onto the DSP's settings
//! (`cantus_dsp::organum`); the editor calls the same function, so what it draws is what
//! the audio thread is told.
//!
//! **Modulation.** The organ's sources are all global and all move only at events — the
//! macros with a snapshot, the mod wheel, expression, aftertouch and sustain with a
//! controller — so the matrix is evaluated then, not per block: the effective values are
//! the stored ones plus each route's depth × the target's range, clamped, and the DSP
//! glides where a jump would click. A route at depth 0 is never evaluated.
//!
//! **Controllers, natively**: the mod wheel past half way turns the rotor to fast (the
//! half-moon switch organists expect under the right hand); expression (CC 11) is the swell
//! pedal, multiplying the editor's swell.
//!
//! ## Telemetry
//!
//! | Index | What |
//! |---|---|
//! | 0, 1, 2 | keys down, keys the organ has (154), 0 (nothing is ever stolen) |
//! | [`tel::HORN_ANGLE`], [`tel::DRUM_ANGLE`] | rotor angles, turns (0 = at the listener) |
//! | [`tel::HORN_RPM`], [`tel::DRUM_RPM`] | rotor speeds now, rpm |
//! | [`tel::SCANNER`] | the vibrato pickup's place on the line, 0…1 |
//! | [`tel::PERC`] | the percussion envelope, 0…1 |
//! | [`tel::PEAK_L`], [`tel::PEAK_R`] | output peaks, linear |
//! | [`tel::SPEED`] | the speed the rotors are heading for: 0 stop, 1 slow, 2 fast |
//! | [`tel::HELD`] … +2 | keys down on the upper, lower and pedal divisions |
//! | [`tel::ALIVE`] | always 1 |

use super::{Controller, InstrumentUnit, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::modulation::{MACRO_KEYS, SLOTS};
use cantus_core::inst::organ::PARAMS;
use cantus_core::NativeSpec;
use cantus_dsp::modmatrix::{ModCurve, ModMatrix, ModRoute};
use cantus_dsp::organum::rotary::RotorSpeed;
use cantus_dsp::organum::scanner::VibMode;
use cantus_dsp::organum::wheels::Tuning;
use cantus_dsp::organum::{OrganSettings, Organum};

pub mod tel {
    pub const HORN_ANGLE: usize = 8;
    pub const DRUM_ANGLE: usize = 9;
    pub const HORN_RPM: usize = 10;
    pub const DRUM_RPM: usize = 11;
    pub const SCANNER: usize = 12;
    pub const PERC: usize = 13;
    pub const PEAK_L: usize = 14;
    pub const PEAK_R: usize = 15;
    pub const SPEED: usize = 16;
    pub const HELD: usize = 17;
    pub const ALIVE: usize = 20;
}

/// Keys the organ has: two manuals of 61 and 32 pedals.
pub const KEYS: usize = 61 + 61 + 32;

/// How many parameters there are (the index space of values and modulation targets).
pub const N: usize = PARAMS.len();

/// The sources the organ produces, in its own index order.
pub const SOURCE_TAGS: [&str; 12] = [
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "mw", "expr",
    "at", "sus",
];
const SRC_MW: usize = 8;
const SRC_EXPR: usize = 9;
const SRC_AT: usize = 10;
const SRC_SUS: usize = 11;

fn flag(v: f32) -> bool {
    v >= 0.5
}

fn apply(s: &mut OrganSettings, key: &str, v: f32) {
    let r = &mut s.rotary;
    match key {
        "split.upper" => s.split_upper = v.round().clamp(0.0, 127.0) as u8,
        "split.pedal" => s.split_pedal = v.round().clamp(0.0, 127.0) as u8,
        "perc" => s.perc_on = flag(v),
        "perc.harm" => s.perc_third = flag(v),
        "perc.decay" => s.perc_slow = flag(v),
        "perc.vol" => s.perc_soft = flag(v),
        "perc.fast" => s.perc_fast_ms = v,
        "perc.slow" => s.perc_slow_ms = v,
        "perc.soft" => s.perc_soft_db = v,
        "perc.drop" => s.perc_drop_db = v,
        "perc.steal" => s.perc_steal = flag(v),
        "vib" => s.vib_mode = VibMode::from_index(v),
        "vib.upper" => s.vib_upper = flag(v),
        "vib.lower" => s.vib_lower = flag(v),
        "vib.rate" => s.vib_rate_hz = v,
        "click" => s.click_db = v,
        "click.rel" => s.release_click_db = v,
        "leak" => s.leak_db = v,
        "leak.bar" => s.bar_leak_db = v,
        "bright" => s.brilliance_db = v,
        "tuning" => s.tuning = Tuning::from_index(v),
        "a4" => s.a4_hz = v,
        "swell" => s.swell = v,
        "drive" => s.drive = v,
        "lvl.upper" => s.upper_db = v,
        "lvl.lower" => s.lower_db = v,
        "lvl.pedal" => s.pedal_db = v,
        "rot.out" => s.rotary_on = !flag(v),
        "rot.speed" => r.speed = RotorSpeed::from_index(v),
        "rot.horn_slow" => r.horn_slow_rpm = v,
        "rot.horn_fast" => r.horn_fast_rpm = v,
        "rot.drum_slow" => r.drum_slow_rpm = v,
        "rot.drum_fast" => r.drum_fast_rpm = v,
        "rot.horn_up" => r.horn_up_s = v,
        "rot.horn_down" => r.horn_down_s = v,
        "rot.drum_up" => r.drum_up_s = v,
        "rot.drum_down" => r.drum_down_s = v,
        "rot.balance" => r.balance_db = v,
        "rot.xover" => r.crossover_hz = v,
        "rot.cab" => r.open = flag(v),
        "rot.dist" => r.mic_distance_cm = v,
        "rot.spread" => r.mic_spread_deg = v,
        "rot.brake" => r.brake_deg = v,
        "gain" => s.gain_db = v,
        _ => {
            // Drawbars: u1…u9, l1…l9, p1, p2.
            let (div, n) = key.split_at(1);
            let Ok(n) = n.parse::<usize>() else {
                return;
            };
            match div {
                "u" if (1..=9).contains(&n) => s.upper[n - 1] = v,
                "l" if (1..=9).contains(&n) => s.lower[n - 1] = v,
                "p" if (1..=2).contains(&n) => s.pedal[n - 1] = v,
                _ => {}
            }
        }
    }
}

/// The DSP's settings for a list of values in `PARAMS` order. Allocation-free.
pub fn settings_from(values: &[f32; N]) -> OrganSettings {
    let mut s = OrganSettings::default();
    for (info, v) in PARAMS.iter().zip(values) {
        apply(&mut s, info.key, *v);
    }
    s
}

/// The stored values of a spec (defaults where nothing is stored), in `PARAMS` order.
pub fn values_of(spec: &NativeSpec) -> [f32; N] {
    let mut v = [0.0; N];
    for (i, info) in PARAMS.iter().enumerate() {
        v[i] = spec.get(info.key);
    }
    v
}

/// The DSP's settings for a spec, without modulation — what the editor draws at rest.
pub fn settings(spec: &NativeSpec) -> OrganSettings {
    settings_from(&values_of(spec))
}

fn source_index(tag: &str) -> Option<u16> {
    SOURCE_TAGS.iter().position(|t| *t == tag).map(|i| i as u16)
}

/// The spec's live connections, resolved to the organ's indices.
pub fn routes(spec: &NativeSpec, matrix: &mut ModMatrix) {
    matrix.clear();
    for k in SLOTS.iter() {
        let (Some(src), Some(dst)) = (spec.text(k.src), spec.text(k.dst)) else {
            continue;
        };
        if spec.get(k.on) < 0.5 {
            continue;
        }
        let (Some(src), Some(dst)) = (source_index(src), PARAMS.iter().position(|i| i.key == dst))
        else {
            continue;
        };
        let via = match spec.text(k.via) {
            Some(v) => match source_index(v) {
                Some(i) => Some(i),
                None => continue,
            },
            None => None,
        };
        matrix.push(ModRoute {
            src,
            dst: dst as u16,
            depth: spec.get(k.amt),
            via,
            curve: ModCurve::from_index(spec.get(k.curve)),
        });
    }
}

/// Stored values moved by the matrix: each route adds `shape(source) · depth · via` of the
/// target's range; the result is clamped to the range.
pub fn modulated(base: &[f32; N], matrix: &ModMatrix, sources: &[f32]) -> [f32; N] {
    let mut eff = *base;
    if matrix.is_empty() {
        return eff;
    }
    let mut off = [0.0f32; N];
    matrix.apply(sources, &mut off);
    for (i, info) in PARAMS.iter().enumerate() {
        if off[i] != 0.0 {
            eff[i] = (base[i] + off[i] * (info.max - info.min)).clamp(info.min, info.max);
        }
    }
    eff
}

#[derive(Clone, Debug)] // r34:listen: Clone for fork
pub struct Unit {
    dsp: Organum,
    base: [f32; N],
    matrix: ModMatrix,
    sources: [f32; SOURCE_TAGS.len()],
    expression: f32,
    mod_wheel: f32,
    speed: RotorSpeed,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let mut base = [0.0; N];
        for (i, info) in PARAMS.iter().enumerate() {
            base[i] = info.default;
        }
        Unit {
            dsp: Organum::new(sample_rate),
            base,
            matrix: ModMatrix::default(),
            sources: [0.0; SOURCE_TAGS.len()],
            expression: 1.0,
            mod_wheel: 0.0,
            speed: RotorSpeed::Slow,
        }
    }

    /// Evaluate the matrix and the controllers, and hand the DSP its settings.
    fn update(&mut self) {
        let eff = modulated(&self.base, &self.matrix, &self.sources);
        let mut s = settings_from(&eff);
        s.swell *= self.expression;
        if self.mod_wheel >= 0.5 && s.rotary.speed == RotorSpeed::Slow {
            s.rotary.speed = RotorSpeed::Fast;
        }
        self.speed = s.rotary.speed;
        self.dsp.set(&s);
    }
}

impl InstrumentUnit for Unit {
    fn fork(&self) -> Option<Box<dyn InstrumentUnit>> {
        Some(Box::new(self.clone()))
    } // r34:listen
    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.base = values_of(spec);
        routes(spec, &mut self.matrix);
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.sources[m] = spec.get(key);
        }
        self.update();
    }

    fn note_on(&mut self, note: NoteOn) {
        self.dsp.note_on(note.key, note.id, note.channel);
    }

    fn note_off(&mut self, _key: u8, id: u64, _velocity: f32) {
        self.dsp.note_off(id);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        let (src, native) = match controller {
            Controller::ModWheel => (SRC_MW, true),
            Controller::Expression => (SRC_EXPR, true),
            Controller::Pressure => (SRC_AT, false),
            Controller::Sustain => (SRC_SUS, false),
            _ => return,
        };
        let value = value.clamp(0.0, 1.0);
        let old = self.sources[src];
        self.sources[src] = value;
        match controller {
            Controller::ModWheel => self.mod_wheel = value,
            Controller::Expression => self.expression = value,
            _ => {}
        }
        let routed = self
            .matrix
            .routes()
            .iter()
            .any(|r| r.src as usize == src || r.via.is_some_and(|v| v as usize == src));
        if old != value && (native || routed) {
            self.update();
        }
    }

    fn all_notes_off(&mut self) {
        self.dsp.all_notes_off();
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.dsp.process(left, right)
    }

    fn active_voices(&self) -> usize {
        self.dsp.keys_down()
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut t = [0.0; TELEMETRY_LEN];
        t[0] = self.dsp.keys_down() as f32;
        t[1] = KEYS as f32;
        t[tel::HORN_ANGLE] = m.horn_angle;
        t[tel::DRUM_ANGLE] = m.drum_angle;
        t[tel::HORN_RPM] = m.horn_rpm;
        t[tel::DRUM_RPM] = m.drum_rpm;
        t[tel::SCANNER] = m.scanner;
        t[tel::PERC] = m.perc;
        t[tel::PEAK_L] = m.peak_l;
        t[tel::PEAK_R] = m.peak_r;
        t[tel::SPEED] = self.speed as usize as f32;
        for d in 0..3 {
            t[tel::HELD + d] = m.held[d] as f32;
        }
        t[tel::ALIVE] = 1.0;
        t
    }
}

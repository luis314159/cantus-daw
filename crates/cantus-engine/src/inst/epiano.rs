//! Lamina — the modelled electric piano: the engine's unit. Owned by this instrument's
//! builder (ADR 0023).
//!
//! [`settings`] maps the parameters (`cantus_core::inst::epiano`) onto the DSP's settings
//! (`cantus_dsp::lamina`); the editor calls the same function.
//!
//! **Modulation.** Global sources (macros, mod wheel, aftertouch, pitch bend, expression,
//! sustain) move the instrument's settings when they change; per-note sources (velocity,
//! key) move the settings a note is struck with, latched at its strike — a note's hammer,
//! decay and pickup are its own for its life, as on the instrument. A route at depth 0 is
//! never evaluated.
//!
//! **Controllers, natively**: sustain is the damper pedal, continuously (half-pedalling);
//! pitch bend bends every tine (±2 semitones as the processor scales it); expression is a
//! volume pedal before the preamp.
//!
//! ## Telemetry
//!
//! | Index | What |
//! |---|---|
//! | 0, 1, 2 | voices sounding, polyphony, voices stolen so far |
//! | [`tel::KEY`], [`tel::VELOCITY`] | the last struck key and its velocity (after the curve) |
//! | [`tel::X`], [`tel::SWING_LO`], [`tel::SWING_HI`] | its tip now and its swing over the last block, mm |
//! | [`tel::PEDAL`], [`tel::ENGAGEMENT`] | the pedal 0…1 and how far the dampers rest on the tines |
//! | [`tel::TREM`] | the tremolo's position, −1…1 |
//! | [`tel::PEAK_L`], [`tel::PEAK_R`] | output peaks |
//! | [`tel::ALIVE`] | always 1 |

use super::{Controller, InstrumentUnit, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::epiano::PARAMS;
use cantus_core::inst::modulation::{MACRO_KEYS, SLOTS};
use cantus_core::inst::TRIM_KEY;
use cantus_core::NativeSpec;
use cantus_dsp::lamina::geometry::Engine;
use cantus_dsp::lamina::{Lamina, LaminaSettings, Range, Trem};
use cantus_dsp::modmatrix::{ModCurve, ModMatrix, ModRoute};

pub mod tel {
    pub const KEY: usize = 8;
    pub const VELOCITY: usize = 9;
    pub const X: usize = 10;
    pub const SWING_LO: usize = 11;
    pub const SWING_HI: usize = 12;
    pub const PEDAL: usize = 13;
    pub const ENGAGEMENT: usize = 14;
    pub const TREM: usize = 15;
    pub const PEAK_L: usize = 16;
    pub const PEAK_R: usize = 17;
    pub const ALIVE: usize = 18;
}

pub const N: usize = PARAMS.len();

/// The sources Lamina produces, in its own index order.
pub const SOURCE_TAGS: [&str; 15] = [
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "vel", "key",
    "mw", "at", "pb", "expr", "sus",
];
const SRC_VEL: usize = 8;
const SRC_KEY: usize = 9;
const SRC_MW: usize = 10;
const SRC_AT: usize = 11;
const SRC_PB: usize = 12;
const SRC_EXPR: usize = 13;
const SRC_SUS: usize = 14;

fn flag(v: f32) -> bool {
    v >= 0.5
}

fn apply(s: &mut LaminaSettings, key: &str, v: f32) {
    match key {
        "engine" => s.engine = Engine::from_index(v),
        "range" => s.range = Range::from_index(v),
        "a4" => s.a4_hz = v,
        "spread" => s.spread_ct = v,
        "age" => s.age = v,
        "hard" => s.hardness = v,
        "hard.vel" => s.hardness_vel = v,
        "force" => s.force_db = v,
        "strike" => s.strike = v,
        "hammer.noise" => s.hammer_noise_db = v,
        "tine.decay" => s.tine_decay_s = v,
        "tine.couple" => s.couple = v,
        "tine.bell" => s.bell_db = v,
        "tine.bell_decay" => s.bell_decay_s = v,
        "tine.gap" => s.tine_gap_mm = v,
        "tine.align" => s.tine_align_mm = v,
        "reed.decay" => s.reed_decay_s = v,
        "reed.overtones" => s.overtones_db = v,
        "reed.gap" => s.reed_gap_mm = v,
        "reed.align" => s.reed_align_mm = v,
        "pickup.level" => s.pickup_db = v,
        "damper.time" => s.damper_ms = v,
        "damper.tone" => s.damper_tone = v,
        "damper.noise" => s.damper_noise_db = v,
        "release.noise" => s.release_noise_db = v,
        "pedal.lift" => s.pedal_lift = v,
        "drive" => s.drive = v,
        "bass" => s.bass_db = v,
        "treble" => s.treble_db = v,
        "output" => s.speaker = flag(v),
        "trem.mode" => s.trem = Trem::from_index(v),
        "trem.rate" => s.trem_rate_hz = v,
        "trem.depth" => s.trem_depth = v,
        "dynamics" => s.dynamics_db = v,
        "vel.curve" => s.vel_curve = v,
        "polyphony" => s.polyphony = v.round().clamp(1.0, 64.0) as usize,
        "gain" => s.gain_db = v,
        _ => {
            let Some(rest) = key.strip_prefix("curve.") else {
                return;
            };
            let Some((which, n)) = rest.split_once('.') else {
                return;
            };
            let Ok(n) = n.parse::<usize>() else {
                return;
            };
            if !(1..=8).contains(&n) {
                return;
            }
            match which {
                "hard" => s.curve_hard[n - 1] = v,
                "decay" => s.curve_decay[n - 1] = v,
                "level" => s.curve_level[n - 1] = v,
                _ => {}
            }
        }
    }
}

pub fn settings_from(values: &[f32; N]) -> LaminaSettings {
    let mut s = LaminaSettings::default();
    for (info, v) in PARAMS.iter().zip(values) {
        apply(&mut s, info.key, *v);
    }
    s
}

pub fn values_of(spec: &NativeSpec) -> [f32; N] {
    let mut v = [0.0; N];
    for (i, info) in PARAMS.iter().enumerate() {
        v[i] = spec.get(info.key);
    }
    v
}

/// The DSP's settings for a spec, without modulation — what the editor draws at rest.
pub fn settings(spec: &NativeSpec) -> LaminaSettings {
    trimmed(settings_from(&values_of(spec)), spec.get(TRIM_KEY))
}

/// The output trim (ADR 0039) added to the gain stage, the last one: a project saved
/// before the calibration has a trim of 0, and `x + 0.0` is `x`, so it renders as it did.
fn trimmed(mut s: LaminaSettings, trim_db: f32) -> LaminaSettings {
    s.gain_db += trim_db;
    s
}

fn source_index(tag: &str) -> Option<u16> {
    SOURCE_TAGS.iter().position(|t| *t == tag).map(|i| i as u16)
}

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
    dsp: Lamina,
    base: [f32; N],
    matrix: ModMatrix,
    sources: [f32; SOURCE_TAGS.len()],
    /// Whether any route reads velocity or key (then a note gets settings of its own).
    per_note: bool,
    /// The output trim, dB (ADR 0039).
    trim_db: f32,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let mut base = [0.0; N];
        for (i, info) in PARAMS.iter().enumerate() {
            base[i] = info.default;
        }
        Unit {
            dsp: Lamina::new(sample_rate),
            base,
            matrix: ModMatrix::default(),
            sources: [0.0; SOURCE_TAGS.len()],
            per_note: false,
            trim_db: cantus_core::InstrumentKind::ElectricPiano.calibration_db(),
        }
    }

    fn update(&mut self) {
        let mut src = self.sources;
        src[SRC_VEL] = 0.0;
        src[SRC_KEY] = 0.0;
        let eff = modulated(&self.base, &self.matrix, &src);
        self.dsp.set(&trimmed(settings_from(&eff), self.trim_db));
    }

    fn reads(&self, src: usize) -> bool {
        self.matrix
            .routes()
            .iter()
            .any(|r| r.src as usize == src || r.via.is_some_and(|v| v as usize == src))
    }

    pub fn dsp(&self) -> &Lamina {
        &self.dsp
    }
}

impl InstrumentUnit for Unit {
    fn fork(&self) -> Option<Box<dyn InstrumentUnit>> {
        Some(Box::new(self.clone()))
    } // r34:listen
    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.base = values_of(spec);
        self.trim_db = spec.get(TRIM_KEY);
        routes(spec, &mut self.matrix);
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.sources[m] = spec.get(key);
        }
        self.per_note = self.reads(SRC_VEL) || self.reads(SRC_KEY);
        self.update();
    }

    fn note_on(&mut self, note: NoteOn) {
        if self.per_note {
            let mut src = self.sources;
            src[SRC_VEL] = note.velocity;
            src[SRC_KEY] = ((note.key as f32 - 60.0) / 48.0).clamp(-1.0, 1.0);
            let s = trimmed(
                settings_from(&modulated(&self.base, &self.matrix, &src)),
                self.trim_db,
            );
            self.dsp.note_on(note.key, note.id, note.velocity, Some(&s));
        } else {
            self.dsp.note_on(note.key, note.id, note.velocity, None);
        }
    }

    fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        self.dsp.note_off(key, id, velocity);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        let src = match controller {
            Controller::ModWheel => SRC_MW,
            Controller::Pressure => SRC_AT,
            Controller::PitchBend => {
                self.dsp.set_bend(value);
                SRC_PB
            }
            Controller::Expression => {
                self.dsp.set_expression(value);
                SRC_EXPR
            }
            Controller::Sustain => {
                self.dsp.set_pedal(value);
                SRC_SUS
            }
            Controller::Cc(_) => return,
        };
        let v = if src == SRC_PB {
            (value / 2.0).clamp(-1.0, 1.0)
        } else {
            value.clamp(0.0, 1.0)
        };
        let old = self.sources[src];
        self.sources[src] = v;
        if old != v && self.reads(src) {
            self.update();
        }
    }

    fn all_notes_off(&mut self) {
        self.dsp.all_notes_off();
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    /// Lamina at rest, and every played source (wheel, pressure, bend, expression, pedal)
    /// where a new unit has it, so the modulated settings are a new unit's too.
    fn at_rest(&self) -> bool {
        self.sources[SRC_MW..=SRC_SUS].iter().all(|v| *v == 0.0) && self.dsp.at_rest()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.dsp.process(left, right)
    }

    fn active_voices(&self) -> usize {
        self.dsp.active_voices()
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut t = [0.0; TELEMETRY_LEN];
        t[0] = m.voices as f32;
        t[1] = self.dsp.polyphony() as f32;
        t[2] = m.stolen as f32;
        t[tel::KEY] = m.key as f32;
        t[tel::VELOCITY] = m.velocity;
        t[tel::X] = m.x;
        t[tel::SWING_LO] = m.swing_lo;
        t[tel::SWING_HI] = m.swing_hi;
        t[tel::PEDAL] = m.pedal;
        t[tel::ENGAGEMENT] = m.engagement;
        t[tel::TREM] = m.trem;
        t[tel::PEAK_L] = m.peak_l;
        t[tel::PEAK_R] = m.peak_r;
        t[tel::ALIVE] = 1.0;
        t
    }
}

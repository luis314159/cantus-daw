//! Calor — the analog-modelling polysynth: the engine's unit (ADR 0023). Owned by this
//! instrument's builder.
//!
//! `configure` reads the spec into [`CalorSettings`] (real units) and resolves the shared
//! modulation matrix — source tags to Calor's source slots, target keys to its
//! [`Target`]s, each depth (a fraction of the target's range) scaled into the target's own
//! unit — without allocating. Everything else is the DSP's.

use super::{Controller, InstrumentUnit, NoteExpression, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::analog::CARD_ROLL_KEYS;
use cantus_core::inst::modulation::{connections, MACRO_KEYS};
use cantus_core::{InstrumentKind, NativeSpec};
use cantus_dsp::calor::chorus::ChorusMode;
use cantus_dsp::calor::{
    src, Assign, Calor, CalorSettings, Expression, HpfPos, LfoRestart, LfoSettings, LfoShape,
    OscRev, OscSettings, Quality, Target, CARDS,
};
use cantus_dsp::math;
use cantus_dsp::modmatrix::{ModCurve, ModRoute, MAX_ROUTES};
use cantus_dsp::va::math::db_gain;
use cantus_dsp::va::{EnvModel, EnvSettings, FilterSettings, Model, Revision, SubShape};
use cantus_dsp::voices::{PlayMode, Priority};

#[derive(Debug)]
pub struct Unit {
    calor: Box<Calor>,
    routes: [ModRoute; MAX_ROUTES],
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            calor: Box::new(Calor::new(sample_rate)),
            routes: [ModRoute::default(); MAX_ROUTES],
        }
    }
}

/// A time knob at its minimum is "off" (glide, LFO delay).
fn off_at_min(v: f32) -> f32 {
    if v <= 0.0011 {
        0.0
    } else {
        v
    }
}

/// The spec in real units.
pub fn settings(spec: &NativeSpec) -> CalorSettings {
    let g = |k: &str| spec.get(k);
    let on = |k: &str| spec.get(k) >= 0.5;
    let osc = |n: usize| {
        let k = |f: usize| g(OSC_KEYS[n][f]);
        OscSettings {
            octave: k(0),
            semi: k(1),
            fine: k(2),
            saw: k(3),
            pulse: k(4),
            tri: k(5),
            pw: k(6),
            pwm: k(7),
            rev: OscRev::from_index(k(8)),
            sync: n > 0 && k(9) >= 0.5,
        }
    };
    let env = |e: usize| {
        let k = |f: usize| g(ENV_KEYS[e][f]);
        EnvSettings {
            attack: k(0) * 0.001,
            decay: k(1) * 0.001,
            sustain: k(2),
            release: k(3) * 0.001,
            model: EnvModel::from_index(k(4)),
        }
    };
    let lfo = |l: usize| {
        let k = |f: usize| g(LFO_KEYS[l][f]);
        LfoSettings {
            rate: k(0),
            shape: LfoShape::from_index(k(1)),
            delay: off_at_min(k(2)),
            restart: LfoRestart::from_index(k(3)),
            phase: k(4) / 360.0,
            unipolar: k(5) >= 0.5,
        }
    };
    let mut rolls = [0u32; CARDS];
    for (c, r) in rolls.iter_mut().enumerate() {
        *r = g(CARD_ROLL_KEYS[c]).max(0.0) as u32;
    }
    CalorSettings {
        mode: PlayMode::from_index(g("voice.mode")),
        priority: Priority::from_index(g("voice.priority")),
        voices: g("voice.count").round().clamp(1.0, 16.0) as usize,
        stack: g("voice.stack").round().clamp(1.0, 8.0) as usize,
        stack_detune: g("voice.stack_detune"),
        stack_spread: g("voice.stack_spread"),
        assign: Assign::from_index(g("voice.assign")),
        glide: off_at_min(g("glide.time")),
        glide_rate: on("glide.mode"),
        glide_linear: on("glide.curve"),
        glide_legato: on("glide.legato"),
        transpose: g("tune.transpose"),
        fine: g("tune.fine"),
        bend_up: g("bend.up"),
        bend_down: g("bend.down"),
        osc: [osc(0), osc(1), osc(2)],
        osc3_low: on("osc3.low"),
        osc3_kbd: on("osc3.kbd"),
        xmod: g("xmod.amount"),
        polymod_env: g("polymod.env"),
        polymod_osc3: g("polymod.osc3"),
        polymod_freq: on("polymod.to_freq"),
        polymod_pw: on("polymod.to_pw"),
        polymod_vcf: on("polymod.to_vcf"),
        sub_shape: SubShape::from_index(g("sub.shape")),
        noise_pink: on("noise.color"),
        mix: [
            g("mix.osc1"),
            g("mix.osc2"),
            g("mix.osc3"),
            g("mix.sub"),
            g("mix.noise"),
        ],
        feedback: g("mix.feedback"),
        mix_drive: db_gain(g("mix.drive")),
        hpf: g("hpf.cutoff"),
        hpf_pos: HpfPos::from_index(g("hpf.pos")),
        filter: FilterSettings {
            model: Model::from_index(g("vcf.model")),
            reso: g("vcf.reso"),
            poles: g("vcf.slope").round().clamp(1.0, 4.0) as u8,
            morph: g("vcf.morph"),
            band: on("vcf.mid"),
            bass_comp: g("vcf.bass_comp"),
            drive: db_gain(g("vcf.drive")),
            rev: Revision::from_index(g("vcf.rev")),
        },
        cutoff: g("vcf.cutoff"),
        key_track: g("vcf.key"),
        env_amount: g("vcf.env"),
        env_velocity: g("vcf.vel"),
        lfo_amount: g("vcf.lfo"),
        filter_fm: g("vcf.fm"),
        vca_gate: on("vca.source"),
        vca_velocity: g("vca.vel"),
        vca_drive: g("vca.drive"),
        pan: g("vca.pan"),
        env: [env(0), env(1)],
        env_kbd: [g("env1.kbd"), g("env2.kbd")],
        env_vel: [g("env1.vel"), g("env2.vel")],
        env_reset: [on("env1.retrig"), on("env2.retrig")],
        lfo: [lfo(0), lfo(1)],
        lfo1_pitch: g("lfo1.pitch"),
        wheel_vibrato: g("wheel.vibrato"),
        wheel_mix: g("wheel.mix"),
        wheel_pw: g("wheel.to_pw"),
        wheel_vcf: g("wheel.to_vcf"),
        condition: g("analog.condition"),
        seed: g("analog.seed").max(0.0) as u32,
        drift: g("analog.drift"),
        drift_rate: g("analog.drift_rate"),
        variance: cantus_dsp::va::Variance {
            detune_cents: g("analog.detune"),
            cutoff_st: g("analog.var_cutoff"),
            env: g("analog.var_env"),
            pw: g("analog.var_pw"),
            glide: g("analog.var_glide"),
            tracking_cents: g("analog.tracking"),
            pan: 0.3,
        },
        phase_reset: on("analog.phase"),
        rolls,
        quality: Quality::from_index(g("quality")),
        chorus: ChorusMode::from_index(g("chorus.mode")),
        chorus_mix: g("chorus.mix"),
        gain: db_gain(g("gain")),
    }
}

// Key tables, so `configure` reads the spec without formatting a string. Oscillator
// fields: octave, semi, fine, saw, pulse, tri, pw, pwm, rev, sync (oscillator 1 has none).
const OSC_KEYS: [[&str; 10]; 3] = [
    [
        "osc1.octave",
        "osc1.semi",
        "osc1.fine",
        "osc1.saw",
        "osc1.pulse",
        "osc1.tri",
        "osc1.pw",
        "osc1.pwm",
        "osc1.rev",
        "osc1.rev",
    ],
    [
        "osc2.octave",
        "osc2.semi",
        "osc2.fine",
        "osc2.saw",
        "osc2.pulse",
        "osc2.tri",
        "osc2.pw",
        "osc2.pwm",
        "osc2.rev",
        "osc2.sync",
    ],
    [
        "osc3.octave",
        "osc3.semi",
        "osc3.fine",
        "osc3.saw",
        "osc3.pulse",
        "osc3.tri",
        "osc3.pw",
        "osc3.pwm",
        "osc3.rev",
        "osc3.sync",
    ],
];
const ENV_KEYS: [[&str; 5]; 2] = [
    ["env1.a", "env1.d", "env1.s", "env1.r", "env1.model"],
    ["env2.a", "env2.d", "env2.s", "env2.r", "env2.model"],
];
const LFO_KEYS: [[&str; 6]; 2] = [
    [
        "lfo1.rate",
        "lfo1.shape",
        "lfo1.delay",
        "lfo1.restart",
        "lfo1.phase",
        "lfo1.unipolar",
    ],
    [
        "lfo2.rate",
        "lfo2.shape",
        "lfo2.delay",
        "lfo2.restart",
        "lfo2.phase",
        "lfo2.unipolar",
    ],
];

/// A modulation source's slot in Calor, by its tag.
pub fn route_source(tag: &str) -> Option<u16> {
    let i = match tag {
        "env1" => src::ENV1,
        "env2" => src::ENV2,
        "lfo1" => src::LFO1,
        "lfo2" => src::LFO2,
        "vel" => src::VEL,
        "relvel" => src::RELVEL,
        "key" => src::KEY,
        "rnd" => src::RND,
        "mpe.bend" => src::MPE_BEND,
        "mpe.press" => src::MPE_PRESS,
        "mpe.slide" => src::MPE_SLIDE,
        "mw" => src::MW,
        "at" => src::AT,
        "pb" => src::PB,
        "expr" => src::EXPR,
        "sus" => src::SUS,
        _ => {
            let m = MACRO_KEYS.iter().position(|k| *k == tag)?;
            src::MACRO1 + m
        }
    };
    Some(i as u16)
}

/// Where a target key lands in Calor, and what one unit of the parameter is in the
/// target's unit (semitones to cents, octaves to cents).
const TARGETS: &[(&str, Target, f32)] = &[
    ("tune.fine", Target::Pitch, 1.0),
    ("tune.transpose", Target::Pitch, 100.0),
    ("osc1.octave", Target::Osc1Pitch, 1200.0),
    ("osc1.semi", Target::Osc1Pitch, 100.0),
    ("osc1.fine", Target::Osc1Pitch, 1.0),
    ("osc2.octave", Target::Osc2Pitch, 1200.0),
    ("osc2.semi", Target::Osc2Pitch, 100.0),
    ("osc2.fine", Target::Osc2Pitch, 1.0),
    ("osc3.octave", Target::Osc3Pitch, 1200.0),
    ("osc3.semi", Target::Osc3Pitch, 100.0),
    ("osc3.fine", Target::Osc3Pitch, 1.0),
    ("osc1.pw", Target::Pw1, 1.0),
    ("osc2.pw", Target::Pw2, 1.0),
    ("osc3.pw", Target::Pw3, 1.0),
    ("mix.osc1", Target::Mix1, 1.0),
    ("mix.osc2", Target::Mix2, 1.0),
    ("mix.osc3", Target::Mix3, 1.0),
    ("mix.sub", Target::MixSub, 1.0),
    ("mix.noise", Target::MixNoise, 1.0),
    ("mix.feedback", Target::Feedback, 1.0),
    ("mix.drive", Target::MixDrive, 1.0),
    ("vcf.cutoff", Target::Cutoff, 1.0),
    ("vcf.reso", Target::Reso, 1.0),
    ("vcf.drive", Target::VcfDrive, 1.0),
    ("vcf.morph", Target::Morph, 1.0),
    ("vcf.fm", Target::FilterFm, 1.0),
    ("vcf.env", Target::EnvAmount, 1.0),
    ("vcf.bass_comp", Target::BassComp, 1.0),
    ("hpf.cutoff", Target::Hpf, 1.0),
    ("vca.drive", Target::VcaDrive, 1.0),
    ("vca.pan", Target::Pan, 1.0),
    ("gain", Target::Gain, 1.0),
    ("lfo1.rate", Target::Lfo1Rate, 1.0),
    ("lfo2.rate", Target::Lfo2Rate, 1.0),
    ("xmod.amount", Target::Xmod, 1.0),
    ("polymod.env", Target::PolyEnv, 1.0),
    ("polymod.osc3", Target::PolyOsc3, 1.0),
    ("env1.a", Target::Env1A, 1.0),
    ("env1.d", Target::Env1D, 1.0),
    ("env1.s", Target::Env1S, 1.0),
    ("env1.r", Target::Env1R, 1.0),
    ("env2.a", Target::Env2A, 1.0),
    ("env2.d", Target::Env2D, 1.0),
    ("env2.s", Target::Env2S, 1.0),
    ("env2.r", Target::Env2R, 1.0),
    ("wheel.vibrato", Target::Vibrato, 1.0),
    ("lfo1.pitch", Target::Vibrato, 1.0),
];

/// A target key's slot in Calor and the depth, in the target's unit, of a route at full
/// depth: the parameter's whole range (in octaves for a logarithmic one), in its unit.
pub fn route_target(key: &str) -> Option<(u16, f32)> {
    let (_, target, unit) = TARGETS.iter().find(|(k, _, _)| *k == key)?;
    let info = InstrumentKind::Analog.param(key)?;
    let span = if info.logarithmic && info.min > 0.0 {
        math::log2(info.max / info.min)
    } else {
        info.max - info.min
    };
    Some((*target as u16, span * unit))
}

impl InstrumentUnit for Unit {
    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.calor.set(&settings(spec));
        let mut n = 0;
        for c in connections(spec).filter(|c| c.on) {
            let (Some(src), Some((dst, span))) = (route_source(c.src), route_target(c.dst)) else {
                continue;
            };
            if n < MAX_ROUTES && c.amount != 0.0 {
                self.routes[n] = ModRoute {
                    src,
                    dst,
                    depth: c.amount * span,
                    via: c.via.and_then(route_source),
                    curve: ModCurve::from_index(c.curve as usize as f32),
                };
                n += 1;
            }
        }
        self.calor.set_routes(&self.routes[..n]);
        let mut macros = [0f32; 8];
        for (m, k) in MACRO_KEYS.iter().enumerate() {
            macros[m] = spec.get(k);
        }
        self.calor.set_macros(&macros);
    }

    fn note_on(&mut self, note: NoteOn) {
        self.calor.note_on(note.key, note.velocity, note.id);
    }

    fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        self.calor.note_off(key, id, velocity);
    }

    fn choke(&mut self, key: u8, id: u64) {
        self.calor.choke(key, id);
    }

    fn expression(&mut self, id: u64, expression: NoteExpression, value: f32) {
        let e = match expression {
            NoteExpression::Tuning => Expression::Tuning,
            NoteExpression::Pressure => Expression::Pressure,
            NoteExpression::Brightness => Expression::Brightness,
            NoteExpression::Pan => Expression::Pan,
            NoteExpression::Volume => Expression::Volume,
            NoteExpression::Vibrato => Expression::Vibrato,
            NoteExpression::Expression => Expression::Expr,
        };
        self.calor.expression(id, e, value);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        match controller {
            Controller::PitchBend => self.calor.pitch_bend(value),
            Controller::ModWheel => self.calor.mod_wheel(value),
            Controller::Sustain => self.calor.sustain(value),
            Controller::Pressure => self.calor.pressure(value),
            Controller::Expression => self.calor.expression_pedal(value),
            Controller::Cc(_) => {}
        }
    }

    fn all_notes_off(&mut self) {
        self.calor.all_notes_off();
    }

    fn reset(&mut self) {
        self.calor.reset();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.calor.process(left, right)
    }

    fn active_voices(&self) -> usize {
        self.calor.active_voices()
    }

    fn latency(&self) -> usize {
        self.calor.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        self.calor.telemetry(&mut t);
        t
    }
}

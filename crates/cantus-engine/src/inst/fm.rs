//! Modulus — the FM synth: the engine's unit (ADR 0023). Owned by this instrument's builder.
//!
//! `configure` turns the sparse spec into the DSP's plain [`Patch`] without allocating: it
//! starts from the patch every default describes (built once, in `new`) and writes only the
//! numbers the spec stores, parsing each key's parts in place; the shared modulation slots
//! are resolved to the DSP's source and target indices, with each route's depth scaled from
//! "a fraction of the target's range" into the target's own unit.

use super::{Assets, Controller, InstrumentUnit, NoteExpression, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::fm::{self as model, OPS};
use cantus_core::inst::modulation::{connections, MACRO_KEYS};
use cantus_core::NativeSpec;
use cantus_dsp::modmatrix::{ModCurve, ModRoute};
use cantus_dsp::modulus::{source, target, Modulus, OpSettings, Patch};
use cantus_dsp::voices::{PlayMode, Priority};

#[derive(Debug)]
pub struct Unit {
    synth: Modulus,
    defaults: Box<Patch>,
    scratch: Box<Patch>,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let defaults = Box::new(default_patch());
        let mut synth = Modulus::new(sample_rate);
        synth.set_patch(&defaults);
        Unit {
            synth,
            scratch: defaults.clone(),
            defaults,
        }
    }

    pub fn synth(&self) -> &Modulus {
        &self.synth
    }
}

/// The patch every default of the model describes.
pub fn default_patch() -> Patch {
    let mut p = Patch::default();
    for info in model::params() {
        apply(&mut p, info.key, info.default);
    }
    p
}

/// The patch a spec describes (allocates a patch: for the editor and tests; the unit fills
/// its own without allocating).
pub fn patch_of(spec: &NativeSpec) -> Patch {
    let d = default_patch();
    let mut p = d.clone();
    patch_into(spec, &d, &mut p);
    p
}

pub fn patch_into(spec: &NativeSpec, defaults: &Patch, out: &mut Patch) {
    out.clone_from(defaults);
    for (k, v) in &spec.params {
        apply(out, k, *v);
    }
    apply_morph(spec, out);
    for (i, k) in MACRO_KEYS.iter().enumerate() {
        out.macros[i] = spec.get(k);
    }
    out.routes.clear();
    for c in connections(spec).filter(|c| c.on) {
        let (Some(src), Some((dst, scale))) = (source_index(c.src), target_of(c.dst)) else {
            continue;
        };
        out.routes.push(ModRoute {
            src: src as u16,
            dst: dst as u16,
            depth: c.amount * scale,
            via: c.via.and_then(source_index).map(|v| v as u16),
            curve: ModCurve::from_index(c.curve as i32 as f32),
        });
    }
}

/// The morph pad: the morphable values as the bilinear mix of the four corners at the pad's
/// position (a corner never stored is the patch as it is). Nothing happens while no corner
/// is stored. Stack arrays only: this runs in `configure`.
fn apply_morph(spec: &NativeSpec, out: &mut Patch) {
    use model::{morph_pairs, morph_slot, morph_weights, MORPH_CORNERS, MORPH_FIELDS, MORPH_KEYS};
    let stored = MORPH_CORNERS.map(|k| spec.text(k));
    if stored.iter().all(Option::is_none) {
        return;
    }
    let w = morph_weights(spec.get("morph.x"), spec.get("morph.y"));
    let mut mixed = [0f32; MORPH_FIELDS];
    let mut corner = [0f32; MORPH_FIELDS];
    for (i, text) in stored.iter().enumerate() {
        if w[i] == 0.0 {
            continue;
        }
        match text {
            None => {
                for (s, k) in MORPH_KEYS.iter().enumerate() {
                    corner[s] = spec.get(k);
                }
            }
            Some(text) => {
                for (s, k) in MORPH_KEYS.iter().enumerate() {
                    corner[s] = model::param(k).map(|p| p.default).unwrap_or(0.0);
                }
                for (k, v) in morph_pairs(text) {
                    if let Some(s) = morph_slot(k) {
                        corner[s] = v;
                    }
                }
            }
        }
        for (m, c) in mixed.iter_mut().zip(&corner) {
            *m += w[i] * c;
        }
    }
    for (s, k) in MORPH_KEYS.iter().enumerate() {
        apply(out, k, mixed[s]);
    }
}

/// A source tag's index among the DSP's sources (the model lists them in the same order).
pub fn source_index(tag: &str) -> Option<usize> {
    model::SOURCES.iter().position(|s| *s == tag)
}

/// The DSP target a key drives, and how many of the target's units a full-depth route is
/// (its range, in octaves for the logarithmic ones). `None`: not modulatable.
pub fn target_of(key: &str) -> Option<(usize, f32)> {
    let info = model::param(key)?;
    let scale = if info.logarithmic {
        (info.max / info.min).log2()
    } else {
        info.max - info.min
    };
    let mut it = key.split('.');
    let head = it.next()?;
    let op = it.clone().next().and_then(model::op_index);
    let t = match (head, op) {
        ("gain", _) => target::GAIN,
        ("tune", _) => target::TUNE,
        ("m", Some(s)) => target::M + s * OPS + model::op_index(it.nth(1)?)?,
        ("fb", Some(o)) => target::FB + o,
        ("out", Some(o)) => target::OUT + o,
        ("pan", Some(o)) => target::PAN + o,
        ("op", Some(o)) => match it.nth(1)? {
            "level" => target::LEVEL + o,
            "detune" => target::DETUNE + o,
            "ratio" => target::RATIO + o,
            _ => return None,
        },
        _ => match key {
            "lfo1.rate" => target::LFO1_RATE,
            "lfo1.pitch" => target::LFO1_PITCH,
            "lfo1.amp" => target::LFO1_AMP,
            "lfo2.rate" => target::LFO2_RATE,
            "macro.bright" => target::BRIGHT,
            "macro.harm" => target::HARM,
            "macro.detune" => target::MDETUNE,
            _ => return None,
        },
    };
    Some((t, scale))
}

fn on(v: f32) -> bool {
    v >= 0.5
}

fn index(field: &str, prefix: char) -> Option<usize> {
    let n: usize = field.strip_prefix(prefix)?.parse().ok()?;
    (1..=model::ENV_POINTS).contains(&n).then(|| n - 1)
}

fn op_field(s: &mut OpSettings, field: &str, v: f32) {
    match field {
        "on" => s.on = on(v),
        "wave" => s.wave = v.round().clamp(0.0, 3.0) as u8,
        "freq_mode" => s.fixed = on(v),
        "ratio" => s.ratio = v,
        "detune" => s.detune = v,
        "offset" => s.offset = v,
        "fixed" => s.fixed_hz = v,
        "level" => s.level_db = v,
        "vel" => s.vel = v,
        "phase" => s.phase = v,
        "sync" => s.key_sync = on(v),
        "penv" => s.penv = on(v),
        "ams" => s.ams = v.round().clamp(0.0, 3.0) as u8,
        _ => {}
    }
}

fn env_field(s: &mut OpSettings, field: &str, v: f32) {
    let e = &mut s.time;
    let step = |x: f32| x.round().clamp(0.0, 99.0) as u8;
    match field {
        "mode" => s.dx = on(v),
        "n" => e.n = (v.round() as usize).clamp(1, model::ENV_POINTS),
        "sus" => e.sus = (v.round() as usize).min(model::ENV_POINTS),
        "loop" => e.lp = (v.round() as usize).min(model::ENV_POINTS),
        "l0" => e.l0 = v,
        "key_time" => e.key_time = v,
        "vel_time" => e.vel_time = v,
        _ => {
            if let Some(k) = field
                .strip_prefix("dr")
                .and_then(|n| n.parse::<usize>().ok())
            {
                if (1..=4).contains(&k) {
                    s.dxenv.rates[k - 1] = step(v);
                }
            } else if let Some(k) = field
                .strip_prefix("dl")
                .and_then(|n| n.parse::<usize>().ok())
            {
                if (1..=4).contains(&k) {
                    s.dxenv.levels[k - 1] = step(v);
                }
            } else if let Some(k) = index(field, 't') {
                e.t[k] = v / 1000.0;
            } else if let Some(k) = index(field, 'l') {
                e.l[k] = v;
            } else if let Some(k) = index(field, 'c') {
                e.c[k] = v;
            }
        }
    }
}

fn ks_field(s: &mut OpSettings, field: &str, v: f32) {
    match field {
        "break" => s.ks_break = v.round().clamp(0.0, 127.0) as u8,
        "ldepth" => s.ks_ldepth = v,
        "rdepth" => s.ks_rdepth = v,
        "lcurve" => s.ks_lcurve = v.round().clamp(0.0, 3.0) as u8,
        "rcurve" => s.ks_rcurve = v.round().clamp(0.0, 3.0) as u8,
        "rate" => s.ks_rate = v.round().clamp(0.0, 7.0) as u8,
        _ => {}
    }
}

/// Write one number into the patch. Keys the patch does not hold are ignored.
pub fn apply(p: &mut Patch, key: &str, v: f32) {
    let mut it = key.split('.');
    let head = it.next().unwrap_or("");
    let second = it.next();
    let third = it.next().unwrap_or("");
    let op = second.and_then(model::op_index);
    match (head, op) {
        ("op", Some(o)) => op_field(&mut p.ops[o], third, v),
        ("out", Some(o)) => p.ops[o].out_db = v,
        ("pan", Some(o)) => p.ops[o].pan = v,
        ("fb", Some(o)) => p.ops[o].fb = v,
        ("env", Some(o)) => env_field(&mut p.ops[o], third, v),
        ("ks", Some(o)) => ks_field(&mut p.ops[o], third, v),
        ("m", Some(s)) => {
            if let Some(t) = model::op_index(third) {
                if s != t {
                    p.m[s][t] = v;
                }
            }
        }
        _ => global(p, key, v),
    }
}

fn global(p: &mut Patch, key: &str, v: f32) {
    let ms = v / 1000.0;
    let n = v.round();
    match key {
        "gain" => p.gain_db = v,
        "voices" => p.voices = n.clamp(1.0, 64.0) as usize,
        "mode" => p.mode = PlayMode::from_index(v),
        "priority" => p.priority = Priority::from_index(v),
        "glide" => p.glide = ms,
        "unison" => p.unison = n.clamp(1.0, 8.0) as usize,
        "uni.detune" => p.uni_detune = v,
        "uni.spread" => p.uni_spread = v,
        "transpose" => p.transpose = v,
        "tune" => p.tune = v,
        "pb_range" => p.pb_range = v,
        "quality" => p.quality = n.clamp(0.0, 4.0) as u8,
        "guard" => p.guard = n.clamp(0.0, 2.0) as u8,
        "fb_avg" => p.fb_avg = on(v),
        "drift" => p.drift = v,
        "penv.l0" => p.penv_l[0] = v,
        "penv.l1" => p.penv_l[1] = v,
        "penv.l2" => p.penv_l[2] = v,
        "penv.l3" => p.penv_l[3] = v,
        "penv.t1" => p.penv_t[0] = ms,
        "penv.t2" => p.penv_t[1] = ms,
        "penv.t3" => p.penv_t[2] = ms,
        "lfo1.rate" => p.lfo_rate[0] = v,
        "lfo1.wave" => p.lfo_wave[0] = n.clamp(0.0, 5.0) as u8,
        "lfo1.delay" => p.lfo_delay[0] = ms,
        "lfo1.sync" => p.lfo_sync[0] = on(v),
        "lfo1.pitch" => p.lfo1_pitch = v,
        "lfo1.amp" => p.lfo1_amp = v,
        "lfo2.rate" => p.lfo_rate[1] = v,
        "lfo2.wave" => p.lfo_wave[1] = n.clamp(0.0, 5.0) as u8,
        "lfo2.delay" => p.lfo_delay[1] = ms,
        "lfo2.sync" => p.lfo_sync[1] = on(v),
        "menv.a" => p.menv[0] = ms,
        "menv.d" => p.menv[1] = ms,
        "menv.s" => p.menv[2] = v,
        "menv.r" => p.menv[3] = ms,
        "macro.bright" => p.bright = v,
        "macro.harm" => p.harm = v,
        "macro.detune" => p.detune_macro = v,
        "macro.a" => p.env_macro[0] = v,
        "macro.d" => p.env_macro[1] = v,
        "macro.s" => p.env_macro[2] = v,
        "macro.r" => p.env_macro[3] = v,
        _ => {}
    }
}

impl InstrumentUnit for Unit {
    fn configure(&mut self, spec: &NativeSpec, _assets: &Assets) {
        patch_into(spec, &self.defaults, &mut self.scratch);
        self.synth.set_patch(&self.scratch);
    }

    fn note_on(&mut self, note: NoteOn) {
        self.synth.note_on(note.key, note.id, note.velocity);
    }

    fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        self.synth.note_off(key, id, velocity);
    }

    fn expression(&mut self, id: u64, expression: NoteExpression, value: f32) {
        let which = match expression {
            NoteExpression::Tuning => 0,
            NoteExpression::Pressure => 1,
            NoteExpression::Brightness => 2,
            NoteExpression::Pan => 3,
            NoteExpression::Volume => 4,
            NoteExpression::Vibrato | NoteExpression::Expression => return,
        };
        self.synth.expression(id, which, value);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        match controller {
            Controller::PitchBend => self.synth.set_bend(value),
            Controller::ModWheel => self.synth.set_source(source::MW, value),
            Controller::Pressure => self.synth.set_source(source::AT, value),
            Controller::Expression => self.synth.set_source(source::EXPR, value),
            Controller::Sustain => self.synth.set_sustain(value >= 0.5),
            Controller::Cc(_) => {}
        }
    }

    fn all_notes_off(&mut self) {
        self.synth.all_notes_off();
    }

    fn reset(&mut self) {
        self.synth.reset();
    }

    fn at_rest(&self) -> bool {
        self.synth.at_rest()
    }

    fn skip(&mut self, frames: usize) {
        self.synth.skip(frames);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.synth.process(left, right)
    }

    fn active_voices(&self) -> usize {
        self.synth.active_voices()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        let unison = self.synth.patch().unison.max(1);
        t[0] = (self.synth.active_voices() / unison) as f32;
        t[1] = self.synth.polyphony() as f32;
        t[2] = self.synth.stolen() as f32;
        self.synth.telemetry(&mut t);
        t
    }
}

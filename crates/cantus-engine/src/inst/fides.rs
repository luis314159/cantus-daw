//! Fides's unit (round 41, ADR 0150): the resonator instrument on the audio thread. Owned
//! by the `fides` builder. The DSP is `cantus_dsp::fides`; this reads the spec into its
//! settings and routes, hands it the notes, the expressions and the controllers, splits its
//! voices across the pool (ADR 0038) and reports telemetry.
//!
//! **Modulation.** A route to a key the voices move (a pitch, a level, the bow, a decay, a
//! position …) is the DSP's own, evaluated per voice on its control grid with every source
//! — velocity, key, MPE's pressure and slide, the exciter's envelope, the vibrato's LFO.
//! A route to any other key (a body's size, a cut) is evaluated here, from the instance's
//! sources (the macros and the controllers), on a grid of [`CTRL`] samples counted from the
//! start of the stream, and the settings rebuilt; a voice source on such a key reads 0.
//!
//! **MPE.** Each note's tuning, pressure and slide reach its voice: the pressure presses the
//! bow (`mpe.press`), the slide moves where it is played (`mpe.slide`).

use super::{Controller, InstrumentUnit, NoteExpression, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::fides::{self as model, ix, COUNT, PARAMS};
use cantus_core::inst::modulation::{MACRO_KEYS, SLOTS};
use cantus_core::NativeSpec;
use cantus_dsp::fides::body::{BodyKind, BodySettings};
use cantus_dsp::fides::exciter::{ExciterKind, ExciterSettings, NoiseSettings};
use cantus_dsp::fides::tables::ResKind;
use cantus_dsp::fides::{target as tg, Fides, FidesSettings, ResSettings};
use cantus_dsp::math;
use cantus_dsp::modmatrix::{ModCurve, ModRoute, MAX_ROUTES};
use cantus_dsp::voices::PlayMode;

/// The grid the instance-wide routes are evaluated on, in samples from the stream's start.
pub const CTRL: u64 = cantus_dsp::fides::CTRL as u64;

/// Every number of the spec, by index of [`PARAMS`].
pub fn values(spec: &NativeSpec) -> [f32; COUNT] {
    let mut v = [0.0; COUNT];
    for (o, info) in v.iter_mut().zip(PARAMS) {
        *o = spec.get(info.key);
    }
    v
}

/// The DSP's settings from a spec (the editor draws with these too).
pub fn settings(spec: &NativeSpec) -> FidesSettings {
    settings_from(&values(spec))
}

fn res(v: &[f32; COUNT], base: usize) -> ResSettings {
    // `a.*` and `b.*` lie in the same order after their type (the model's test holds it).
    let at = |k: usize| v[base + (k - ix::A_TYPE)];
    ResSettings {
        kind: ResKind::from_index(at(ix::A_TYPE)),
        decay_s: at(ix::A_DECAY),
        material: at(ix::A_MATERIAL),
        decay_key: at(ix::A_DECAY_KEY),
        inharm: at(ix::A_INHARM),
        listen: at(ix::A_LISTEN),
        modes: at(ix::A_MODES).round().clamp(1.0, 48.0) as usize,
        tune_st: at(ix::A_TUNE),
        fine_ct: at(ix::A_FINE),
        keytrack: at(ix::A_KEYTRACK),
        level_db: at(ix::A_LEVEL),
    }
}

/// The DSP's settings from a value array.
pub fn settings_from(v: &[f32; COUNT]) -> FidesSettings {
    let round = |i: usize| v[i].round();
    let on = |i: usize| v[i] >= 0.5;
    FidesSettings {
        poly: round(ix::POLY).clamp(1.0, 32.0) as usize,
        mode: PlayMode::from_index(v[ix::MODE]),
        retrig: on(ix::RETRIG),
        glide_ms: v[ix::GLIDE],
        bend_st: v[ix::BEND],
        transpose_st: 12.0 * round(ix::OCTAVE) + round(ix::TRANSPOSE),
        fine_ct: v[ix::FINE],
        a4: v[ix::A4],
        velocity: v[ix::VELOCITY],
        release_s: v[ix::RELEASE],
        vib_depth_ct: v[ix::VIB_DEPTH],
        vib_rate_hz: v[ix::VIB_RATE],
        vib_delay_ms: v[ix::VIB_DELAY],
        trem_depth: v[ix::TREM_DEPTH],
        trem_rate_hz: v[ix::TREM_RATE],
        seed: round(ix::SEED).max(0.0) as u32,
        exc: ExciterSettings {
            kind: ExciterKind::from_index(v[ix::EXC]),
            level_db: v[ix::EXC_LEVEL],
            contact_ms: v[ix::EXC_CONTACT],
            vel_tone: v[ix::EXC_VEL_TONE],
            tone_hz: v[ix::EXC_TONE],
            noise: v[ix::EXC_NOISE],
            pos: v[ix::EXC_POS],
            bow_pressure: v[ix::BOW_PRESSURE],
            bow_speed: v[ix::BOW_SPEED],
            bow_attack_ms: v[ix::BOW_ATTACK],
            bow_release_ms: v[ix::BOW_RELEASE],
        },
        noise: NoiseSettings {
            level: v[ix::NOISE_LEVEL],
            color_hz: v[ix::NOISE_COLOR],
            width_oct: v[ix::NOISE_WIDTH],
            density: v[ix::NOISE_DENSITY],
            attack_ms: v[ix::NOISE_ATTACK],
            decay_ms: v[ix::NOISE_DECAY],
            sustain: v[ix::NOISE_SUSTAIN],
            release_ms: v[ix::NOISE_RELEASE],
        },
        mpe_press: v[ix::MPE_PRESS],
        mpe_slide: v[ix::MPE_SLIDE],
        a: res(v, ix::A_TYPE),
        b_on: on(ix::B_ON),
        b: res(v, ix::B_TYPE),
        serial: on(ix::COUPLE),
        mix: v[ix::COUPLE_MIX],
        body: BodySettings {
            kind: BodyKind::from_index(v[ix::BODY]),
            mix: v[ix::BODY_MIX],
            size: v[ix::BODY_SIZE],
            decay: v[ix::BODY_DECAY],
        },
        width: v[ix::WIDTH],
        lowcut_hz: v[ix::LOWCUT],
        highcut_hz: v[ix::HIGHCUT],
        gain_db: v[ix::GAIN],
    }
}

/// The DSP target a key's route moves, and the knob's whole travel in that target's units
/// (octaves for a logarithmic knob, its own unit otherwise; cents as semitones).
pub fn voice_target(key: &str) -> Option<(u16, f32)> {
    let i = model::index(key)?;
    let info = &PARAMS[i];
    let travel = if info.logarithmic {
        math::log2(info.max / info.min)
    } else {
        info.max - info.min
    };
    let t = match i {
        ix::TRANSPOSE => tg::PITCH,
        ix::FINE => return Some((tg::PITCH, travel * 0.01)),
        ix::EXC_LEVEL => tg::EXC_LEVEL,
        ix::EXC_CONTACT => tg::EXC_CONTACT,
        ix::EXC_TONE => tg::EXC_TONE,
        ix::EXC_NOISE => tg::EXC_NOISE,
        ix::EXC_POS => tg::EXC_POS,
        ix::BOW_PRESSURE => tg::BOW_PRESSURE,
        ix::BOW_SPEED => tg::BOW_SPEED,
        ix::NOISE_LEVEL => tg::NOISE_LEVEL,
        ix::NOISE_COLOR => tg::NOISE_COLOR,
        ix::A_DECAY => tg::A_DECAY,
        ix::A_MATERIAL => tg::A_MATERIAL,
        ix::A_TUNE => tg::A_TUNE,
        ix::A_FINE => return Some((tg::A_TUNE, travel * 0.01)),
        ix::A_LEVEL => tg::A_LEVEL,
        ix::B_DECAY => tg::B_DECAY,
        ix::B_MATERIAL => tg::B_MATERIAL,
        ix::B_TUNE => tg::B_TUNE,
        ix::B_FINE => return Some((tg::B_TUNE, travel * 0.01)),
        ix::B_LEVEL => tg::B_LEVEL,
        ix::COUPLE_MIX => tg::MIX,
        ix::VIB_DEPTH => tg::VIB_DEPTH,
        ix::GAIN => tg::GAIN,
        ix::BODY_MIX => tg::BODY_MIX,
        ix::TREM_DEPTH => tg::TREM_DEPTH,
        _ => return None,
    };
    Some((t, travel))
}

/// A route the unit evaluates itself (a key the voices do not move), from the instance's
/// sources: `src` indexes `[macro1…8, mw, at, pb, expr, sus]` (a voice source: none).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct SlowRoute {
    dst: u16,
    src: Option<u16>,
    via: Option<Option<u16>>,
    depth: f32,
    curve: ModCurve,
}

#[derive(Clone, Debug)] // Clone for fork (a range render's checkpoint)
pub struct Unit {
    f: Fides,
    sample_rate: f32,
    base: [f32; COUNT],
    slow: [SlowRoute; MAX_ROUTES],
    n_slow: usize,
    /// The instance's sources for the slow routes: the macros, mw, at, pb, expr, sus.
    globals: [f32; 13],
    t: u64,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            f: Fides::new(sample_rate),
            sample_rate,
            base: values(&NativeSpec::new(cantus_core::InstrumentKind::Resonator)),
            slow: [SlowRoute::default(); MAX_ROUTES],
            n_slow: 0,
            globals: [0.0; 13],
            t: 0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The DSP, for tests and the editor's previews.
    pub fn dsp(&self) -> &Fides {
        &self.f
    }

    /// The slow routes applied to the spec's values: the settings for the next `CTRL`.
    fn modulate(&mut self) {
        let mut v = self.base;
        for r in &self.slow[..self.n_slow] {
            let x = r.src.map_or(0.0, |s| self.globals[s as usize]);
            let via = match r.via {
                None => 1.0,
                Some(None) => 0.0,
                Some(Some(s)) => self.globals[s as usize],
            };
            let o = r.curve.shape(x) * r.depth * via;
            if o == 0.0 {
                continue;
            }
            let i = r.dst as usize;
            let info = &PARAMS[i];
            // A fraction of the knob's travel in its taper: a logarithmic knob in octaves.
            let x = if info.logarithmic {
                let span = math::ln(info.max / info.min);
                let pos = math::ln(v[i] / info.min) / span + o;
                info.min * math::exp(pos.clamp(0.0, 1.0) * span)
            } else {
                (v[i] + o * (info.max - info.min)).clamp(info.min, info.max)
            };
            v[i] = if info.stepped { x.round() } else { x };
        }
        self.f.set(&settings_from(&v));
    }

    /// r41:fides-input — `input`, the live input for the Input exciter (ADR 0154): the
    /// strip the track is keyed from, mono, as long as the call; `None` when unkeyed (the
    /// render as it always was, to the bit).
    fn run(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        input: Option<&[f32]>,
        par: &dyn cantus_dsp::par::Parallel,
    ) -> bool {
        let n = left.len().min(right.len());
        let call = |f: &mut Fides, l: &mut [f32], r: &mut [f32], at: usize| match input {
            Some(x) => f.process_input(l, r, &x[at.min(x.len())..(at + l.len()).min(x.len())], par),
            None => f.process_par(l, r, par),
        };
        if self.n_slow == 0 {
            self.t += n as u64;
            return call(&mut self.f, &mut left[..n], &mut right[..n], 0);
        }
        let mut wrote = false;
        let mut at = 0;
        while at < n {
            if self.t.is_multiple_of(CTRL) {
                self.modulate();
            }
            let k = ((CTRL - self.t % CTRL) as usize).min(n - at);
            wrote |= call(
                &mut self.f,
                &mut left[at..at + k],
                &mut right[at..at + k],
                at,
            );
            at += k;
            self.t += k as u64;
        }
        wrote
    }
}

/// Where a source tag lies among the instance's sources (`None`: a voice's own).
fn global_index(tag_index: usize) -> Option<u16> {
    tag_index
        .checked_sub(cantus_dsp::fides::VOICE_SOURCES)
        .map(|i| i as u16)
}

impl InstrumentUnit for Unit {
    /// r42:tuning — the track's tuning (ADR 0155), at the DSP's insertion point.
    fn set_tuning(&mut self, table: &cantus_core::tuning::TuningTable) {
        self.f.set_tuning(&table.0);
    }

    fn fork(&self) -> Option<Box<dyn InstrumentUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.base = values(spec);
        let mut macros = [0f32; 8];
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            macros[m] = spec.get(key);
        }
        self.globals[..8].copy_from_slice(&macros);
        let mut routes = [ModRoute::default(); MAX_ROUTES];
        let (mut n, mut ns) = (0, 0);
        for keys in SLOTS.iter() {
            let (Some(src), Some(dst)) = (spec.text(keys.src), spec.text(keys.dst)) else {
                continue;
            };
            let amount = spec.get(keys.amt);
            if spec.get(keys.on) < 0.5 || amount == 0.0 {
                continue;
            }
            let Some(s) = model::SOURCES.iter().position(|t| *t == src) else {
                continue;
            };
            let via = spec
                .text(keys.via)
                .and_then(|v| model::SOURCES.iter().position(|t| *t == v));
            let curve = ModCurve::from_index(spec.get(keys.curve));
            if let Some((t, travel)) = voice_target(dst) {
                if n < MAX_ROUTES {
                    routes[n] = ModRoute {
                        src: s as u16,
                        dst: t,
                        depth: amount * travel,
                        via: via.map(|v| v as u16),
                        curve,
                    };
                    n += 1;
                }
            } else if let Some(i) = model::index(dst) {
                let kind = cantus_core::InstrumentKind::Resonator;
                if kind.modulatable(dst) && ns < MAX_ROUTES {
                    self.slow[ns] = SlowRoute {
                        dst: i as u16,
                        src: global_index(s),
                        via: via.map(global_index),
                        depth: amount,
                        curve,
                    };
                    ns += 1;
                }
            }
        }
        self.n_slow = ns;
        self.f.set_routes(&routes[..n]);
        self.f.set_macros(&macros);
        if ns == 0 {
            self.f.set(&settings_from(&self.base));
        } else {
            self.modulate();
        }
    }

    fn note_on(&mut self, note: NoteOn) {
        self.f.note_on(note.key, note.velocity, note.id);
    }

    fn note_off(&mut self, key: u8, id: u64, _velocity: f32) {
        self.f.note_off(key, id);
    }

    fn choke(&mut self, key: u8, id: u64) {
        self.f.choke(key, id);
    }

    fn expression(&mut self, id: u64, expression: NoteExpression, value: f32) {
        match expression {
            NoteExpression::Tuning => self.f.expression(id, 0, value),
            NoteExpression::Pressure => self.f.expression(id, 1, value),
            NoteExpression::Brightness => self.f.expression(id, 2, value),
            _ => {}
        }
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        match controller {
            Controller::PitchBend => {
                self.f.pitch_bend(value);
                self.globals[10] = (value / 2.0).clamp(-1.0, 1.0);
            }
            Controller::ModWheel => {
                self.f.set_controller(8, value);
                self.globals[8] = value;
            }
            Controller::Pressure => {
                self.f.set_controller(9, value);
                self.globals[9] = value;
            }
            Controller::Expression => {
                self.f.set_controller(11, value);
                self.globals[11] = value;
            }
            Controller::Sustain => {
                self.f.sustain(value >= 0.5);
                self.globals[12] = if value >= 0.5 { 1.0 } else { 0.0 };
            }
            Controller::Cc(_) => {}
        }
    }

    fn all_notes_off(&mut self) {
        self.f.all_notes_off();
        self.globals[12] = 0.0;
    }

    fn reset(&mut self) {
        self.f.reset();
        self.globals[8..].fill(0.0);
        // The grid restarts with the unit, as a new one's starts at 0.
        self.t = 0;
        if self.n_slow > 0 {
            self.modulate();
        }
    }

    /// The DSP at rest (no voice; the body, the cuts and the played controllers home): the
    /// slow routes read only the macros and the controllers, so nothing moves until a
    /// note comes.
    fn at_rest(&self) -> bool {
        self.f.at_rest()
    }

    fn skip(&mut self, frames: usize) {
        self.f.skip(frames);
        self.t += frames as u64;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.run(left, right, None, &cantus_dsp::par::Serial)
    }

    fn process_par(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        par: &dyn cantus_dsp::par::Parallel,
    ) -> bool {
        self.run(left, right, None, par)
    }

    // r41:fides-input — the Input exciter's live input (ADR 0154).
    fn process_input(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        input: &[f32],
        par: &dyn cantus_dsp::par::Parallel,
    ) -> bool {
        self.run(left, right, Some(input), par)
    }

    fn splits_voices(&self) -> bool {
        true
    }

    fn active_voices(&self) -> usize {
        self.f.active_voices()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        self.f.telemetry(&mut t);
        t
    }
}

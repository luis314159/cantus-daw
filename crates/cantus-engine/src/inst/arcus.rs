//! Arcus's unit (round 42, ADR 0157): the bowed string on the audio thread. Owned by the
//! `arcus` builder. The DSP is `cantus_dsp::arcus`; this reads the spec into its settings,
//! hands it the notes (each with its own articulation and bow when it has one — ADR 0158),
//! the expressions, the controllers and the track's tuning (ADR 0155), splits its voices
//! across the pool (ADR 0038) and reports telemetry.
//!
//! **Controllers.** CC1, CC11, breath (CC2) and aftertouch move the dynamic as `dyn.src`
//! says (by default whichever of CC1, breath and aftertouch moved last, CC11 scaling it);
//! CC21 (or CC2, or aftertouch, as `vib.src` says) the vibrato; CC65 switches portamento on.
//! The velocity alone always plays (pp at the softest — never silent).
//!
//! **Modulation.** Every route is evaluated here from the instance's sources (the macros and
//! the controllers) on a grid of [`CTRL`] samples counted from the stream's start, and the
//! settings rebuilt: no voice source exists, so nothing differs from voice to voice.

use super::{Controller, InstrumentUnit, NoteExpression, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::arcus::{self as model, ix, COUNT, PARAMS};
use cantus_core::inst::modulation::{MACRO_KEYS, SLOTS};
use cantus_core::note_art::{Articulation, Bow, BowPart, BowStart, NoteArt};
use cantus_core::NativeSpec;
use cantus_dsp::arcus::string::Friction;
use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{
    bow, Arcus, ArcusSettings, Art, BowDir, DirMode, DynSource, NoteHow, Technique, VibSource,
};
use cantus_dsp::math;
use cantus_dsp::modmatrix::{ModCurve, MAX_ROUTES};
use cantus_dsp::voices::PlayMode;

/// The grid the routes are evaluated on, in samples from the stream's start.
pub const CTRL: u64 = cantus_dsp::arcus::CTRL as u64;

/// Every number of the spec, by index of [`PARAMS`].
pub fn values(spec: &NativeSpec) -> [f32; COUNT] {
    let mut v = [0.0; COUNT];
    for (o, info) in v.iter_mut().zip(PARAMS) {
        *o = spec.get(info.key);
    }
    v
}

/// The DSP's settings from a spec (the editor draws with these too).
pub fn settings(spec: &NativeSpec) -> ArcusSettings {
    settings_from(&values(spec))
}

/// The DSP's settings from a value array.
pub fn settings_from(v: &[f32; COUNT]) -> ArcusSettings {
    let round = |i: usize| v[i].round();
    ArcusSettings {
        model: round(ix::MODEL).clamp(1.0, 2.0) as u8,
        matched: None,
        instrument: Instrument::from_index(v[ix::INST]),
        technique: Technique::from_index(v[ix::TECH]),
        poly: round(ix::POLY).clamp(1.0, 8.0) as usize,
        mode: PlayMode::from_index(v[ix::MODE]),
        finger_ms: v[ix::FINGER],
        porta_ms: v[ix::PORTA],
        bend_st: round(ix::BEND),
        transpose_st: 12.0 * round(ix::OCTAVE) + round(ix::TRANSPOSE),
        fine_ct: v[ix::FINE],
        dynamic: v[ix::DYN],
        vel_dyn: v[ix::VEL_DYN],
        vel_attack: v[ix::VEL_ATTACK],
        dyn_src: DynSource::from_index(v[ix::DYN_SRC]),
        // The knobs read × (the read-out's units); the DSP takes octaves: β × 2^(−pos/2),
        // the force × 2^force.
        bow_pos: (-2.0 * math::log2(v[ix::BOW_POS].max(1e-3))).clamp(-1.0, 1.0),
        bow_force: math::log2(v[ix::BOW_FORCE].max(1e-3)).clamp(-1.0, 1.0),
        bow_speed: v[ix::BOW_SPEED],
        bow_attack: v[ix::BOW_ATTACK],
        bow_release_ms: v[ix::BOW_RELEASE],
        bow_noise_db: v[ix::BOW_NOISE],
        friction: Friction::from_index(v[ix::FRICTION]),
        dir: DirMode::from_index(v[ix::BOW_DIR]),
        change_ms: v[ix::BOW_CHANGE],
        trem_rate: v[ix::TREM_RATE],
        vib_depth: v[ix::VIB_DEPTH],
        vib_rate: v[ix::VIB_RATE],
        vib_delay_ms: v[ix::VIB_DELAY],
        vib_attack_ms: v[ix::VIB_ATTACK],
        vib_var: v[ix::VIB_VAR],
        vib_src: VibSource::from_index(v[ix::VIB_SRC]),
        vib_dyn: v[ix::VIB_DYN],       // r42:arcus-p5
        player: v[ix::PLAYER],         // r42 arcus-expr
        bow_weight: v[ix::BOW_WEIGHT], // r42 arcus-expr
        pair: round(ix::PAIR),         // r42 fiddle
        tilt: v[ix::TILT],
        just: v[ix::JUST],
        sym: v[ix::SYM],
        grit: v[ix::GRIT],
        bite: v[ix::BITE],
        steel: v[ix::STEEL],
        fiddle: v[ix::FIDDLE], // r42 arcus-classical (schema 87)
        cross: round(ix::CROSS).clamp(0.0, 5.0) as u8,
        mic: round(ix::MIC),
        bridge: v[ix::BRIDGE],
        pizz_pos: v[ix::PIZZ_POS],
        pizz_soft_ms: v[ix::PIZZ_SOFT],
        pizz_decay: v[ix::PIZZ_DECAY],
        decay: v[ix::DECAY],
        ring_s: v[ix::RING],
        mpe_press: v[ix::MPE_PRESS],
        mpe_slide: v[ix::MPE_SLIDE],
        body_mix: v[ix::BODY_MIX],
        body_size: v[ix::BODY_SIZE],
        body_ring: v[ix::BODY_RING],
        lowcut_hz: v[ix::LOWCUT],
        highcut_hz: v[ix::HIGHCUT],
        gain_db: v[ix::GAIN],
        seed: round(ix::SEED).max(0.0) as u32,
        oversample: None, // r42:arcus-upper
        shape_try: None,  // r42 arcus-classical
    }
}

/// r42 arcus-expr — everything a note says about how it is played, as the DSP reads it
/// (ADR 0158's articulation and bow, ADR 0162's bow start, part and string — the string from
/// MusicXML's count, 1 the highest, to the DSP's, 0 the lowest of the instrument's four).
pub fn how_of(art: NoteArt) -> NoteHow {
    let (a, b) = art_of(art);
    NoteHow {
        art: a,
        bow: b,
        start: art.start.map(|s| match s {
            BowStart::String => bow::Start::String,
            BowStart::Air => bow::Start::Air,
        }),
        part: art.part.map(|p| match p {
            BowPart::Frog => bow::Part::Frog,
            BowPart::Middle => bow::Part::Middle,
            BowPart::Tip => bow::Part::Tip,
            BowPart::Whole => bow::Part::Whole,
        }),
        string: art.string.filter(|n| (1..=4).contains(n)).map(|n| 4 - n),
        ..NoteHow::default()
    }
}

/// The DSP's articulation and bow for a note's own (ADR 0158).
pub fn art_of(art: NoteArt) -> (Option<Art>, Option<BowDir>) {
    let a = match art.articulation {
        Articulation::Sustain => Art::Sustain,
        Articulation::Legato => Art::Legato,
        Articulation::Staccato => Art::Staccato,
        Articulation::Spiccato => Art::Spiccato,
        Articulation::Pizzicato => Art::Pizzicato,
        Articulation::Tremolo => Art::Tremolo,
        Articulation::Chop => Art::Chop, // r42 fiddle
    };
    let b = art.bow.map(|b| match b {
        Bow::Down => BowDir::Down,
        Bow::Up => BowDir::Up,
    });
    (Some(a), b)
}

/// A route, evaluated here from the instance's sources: `src` indexes
/// `[macro1…8, mw, at, pb, expr, sus]`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Route {
    dst: u16,
    src: u16,
    via: Option<u16>,
    depth: f32,
    curve: ModCurve,
}

#[derive(Clone, Debug)] // Clone for fork (a range render's checkpoint)
pub struct Unit {
    a: Arcus,
    sample_rate: f32,
    base: [f32; COUNT],
    routes: [Route; MAX_ROUTES],
    n_routes: usize,
    /// The instance's sources: the macros, mw, at, pb, expr, sus.
    globals: [f32; 13],
    t: u64,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            a: Arcus::new(sample_rate),
            sample_rate,
            base: values(&NativeSpec::new(cantus_core::InstrumentKind::BowedString)),
            routes: [Route::default(); MAX_ROUTES],
            n_routes: 0,
            globals: [0.0; 13],
            t: 0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The DSP, for tests and the editor's previews.
    pub fn dsp(&self) -> &Arcus {
        &self.a
    }

    /// The routes applied to the spec's values: the settings for the next `CTRL`.
    fn modulate(&mut self) {
        let mut v = self.base;
        for r in &self.routes[..self.n_routes] {
            let x = self.globals[r.src as usize];
            let via = r.via.map_or(1.0, |s| self.globals[s as usize]);
            let o = r.curve.shape(x) * r.depth * via;
            if o == 0.0 {
                continue;
            }
            let i = r.dst as usize;
            let info = &PARAMS[i];
            let x = if info.logarithmic {
                let span = math::ln(info.max / info.min);
                let pos = math::ln(v[i] / info.min) / span + o;
                info.min * math::exp(pos.clamp(0.0, 1.0) * span)
            } else {
                (v[i] + o * (info.max - info.min)).clamp(info.min, info.max)
            };
            v[i] = if info.stepped { x.round() } else { x };
        }
        self.a.set(&settings_from(&v));
    }

    fn run(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        par: &dyn cantus_dsp::par::Parallel,
    ) -> bool {
        let n = left.len().min(right.len());
        if self.n_routes == 0 {
            self.t += n as u64;
            return self.a.process_par(&mut left[..n], &mut right[..n], par);
        }
        let mut wrote = false;
        let mut at = 0;
        while at < n {
            if self.t.is_multiple_of(CTRL) {
                self.modulate();
            }
            let k = ((CTRL - self.t % CTRL) as usize).min(n - at);
            wrote |= self
                .a
                .process_par(&mut left[at..at + k], &mut right[at..at + k], par);
            at += k;
            self.t += k as u64;
        }
        wrote
    }
}

impl InstrumentUnit for Unit {
    fn fork(&self) -> Option<Box<dyn InstrumentUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.base = values(spec);
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.globals[m] = spec.get(key);
        }
        let kind = cantus_core::InstrumentKind::BowedString;
        let mut n = 0;
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
            if let Some(i) = model::index(dst) {
                if kind.modulatable(dst) && n < MAX_ROUTES {
                    self.routes[n] = Route {
                        dst: i as u16,
                        src: s as u16,
                        via: via.map(|v| v as u16),
                        depth: amount,
                        curve: ModCurve::from_index(spec.get(keys.curve)),
                    };
                    n += 1;
                }
            }
        }
        self.n_routes = n;
        if n == 0 {
            self.a.set(&settings_from(&self.base));
        } else {
            self.modulate();
        }
    }

    fn note_on(&mut self, note: NoteOn) {
        self.a.note_on(note.key, note.velocity, note.id);
    }

    fn note_off(&mut self, key: u8, id: u64, _velocity: f32) {
        self.a.note_off(key, id);
    }

    fn choke(&mut self, key: u8, id: u64) {
        self.a.choke(key, id);
    }

    fn expression(&mut self, id: u64, expression: NoteExpression, value: f32) {
        match expression {
            NoteExpression::Tuning => self.a.expression(id, 0, value),
            NoteExpression::Pressure => self.a.expression(id, 1, value),
            NoteExpression::Brightness => self.a.expression(id, 2, value),
            _ => {}
        }
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        match controller {
            Controller::PitchBend => {
                self.a.pitch_bend(value);
                self.globals[10] = (value / 2.0).clamp(-1.0, 1.0);
            }
            Controller::ModWheel => {
                self.a.controller(0, value);
                self.globals[8] = value;
            }
            Controller::Pressure => {
                self.a.controller(3, value);
                self.globals[9] = value;
            }
            Controller::Expression => {
                self.a.controller(1, value);
                self.globals[11] = value;
            }
            Controller::Sustain => {
                self.a.sustain(value >= 0.5);
                self.globals[12] = if value >= 0.5 { 1.0 } else { 0.0 };
            }
            Controller::Cc(2) => self.a.controller(2, value),
            Controller::Cc(21) => self.a.controller(4, value),
            Controller::Cc(65) => self.a.controller(5, value),
            Controller::Cc(_) => {}
        }
    }

    fn set_tuning(&mut self, table: &cantus_core::tuning::TuningTable) {
        self.a.set_tuning(&table.0);
    }

    fn note_art(&mut self, id: u64, art: NoteArt) {
        self.a.note_how(id, how_of(art)); // r42 arcus-expr
    }

    fn note_len(&mut self, id: u64, len: u32, next: Option<i32>) {
        self.a.note_len(id, len, next); // r42 arcus-classical (ADR 0166)
    }

    fn all_notes_off(&mut self) {
        self.a.all_notes_off();
        self.globals[12] = 0.0;
    }

    fn reset(&mut self) {
        self.a.reset();
        self.globals[8..].fill(0.0);
        self.t = 0;
        if self.n_routes > 0 {
            self.modulate();
        }
    }

    fn at_rest(&self) -> bool {
        self.a.at_rest()
    }

    fn skip(&mut self, frames: usize) {
        self.a.skip(frames);
        self.t += frames as u64;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.run(left, right, &cantus_dsp::par::Serial)
    }

    fn process_par(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        par: &dyn cantus_dsp::par::Parallel,
    ) -> bool {
        self.run(left, right, par)
    }

    fn splits_voices(&self) -> bool {
        true
    }

    fn active_voices(&self) -> usize {
        self.a.active_voices()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        self.a.telemetry(&mut t);
        t
    }
}

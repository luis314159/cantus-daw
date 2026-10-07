//! Unda — the wavetable / hybrid flagship synth behind the engine's instrument contract.
//! Owned by this instrument's builder (ADR 0023).
//!
//! [`settings`] maps the parameters (`cantus_core::inst::wavetable`) onto the DSP's
//! settings (`cantus_dsp::unda`); the editor calls the same function, so what it draws is
//! what the audio thread is told.
//!
//! **Imported tables.** An `osc{n}.table` asset arrives decoded (`SampleData`); its mip
//! levels are built by [`prepare`] *off the audio thread*, before the snapshot that needs
//! them is sent (`inst::units_for` calls it), and kept in a process-wide registry keyed by
//! the decoded audio and the frame size. `configure` only looks them up — lock-free, no
//! allocation. Like the sample cache, the registry never evicts, so a table the audio
//! thread holds is never freed there. A table that is not in the registry plays silence.
//!
//! ## Telemetry
//!
//! | Index | What |
//! |---|---|
//! | 0, 1, 2 | voices sounding, polyphony, voices stolen (the frame's) |
//! | [`tel::POS`] +0…2 | the newest voice's table positions, 0…1 |
//! | [`tel::ENV`] +0…3 | its envelope levels |
//! | [`tel::LFO`] +0…7 | its LFO values, −1…1 |
//! | [`tel::CUTOFF`] +0…1 | its filter cutoffs, Hz (modulated) |
//! | [`tel::RES`] +0…1 | its filter resonances |
//! | [`tel::PEAK`] +0…1 | output peaks, linear |
//! | [`tel::LANES`] | unison lanes running |
//! | [`tel::ALIVE`] | 1: a running engine (or a demo) |
//! | [`tel::OVERSAMPLE`] | the oversampling factor |

use super::{Assets, Controller, InstrumentUnit, NoteExpression, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::modulation::{self, connections, Curve, MACRO_KEYS};
use cantus_core::inst::wavetable::{self as model, parse_shape, LFO_POINTS, TABLE_KEYS};
use cantus_core::{InstrumentKind, NativeSpec, SampleData};
use cantus_dsp::modmatrix::{ModCurve, ModRoute};
use cantus_dsp::pulsus::SampleRef;
use cantus_dsp::unda::filter::FilterType;
use cantus_dsp::unda::grana::Window;
use cantus_dsp::unda::modsrc::{LfoMode, LfoWave, Shape};
use cantus_dsp::unda::osc::Warp;
use cantus_dsp::unda::Engine;
use cantus_dsp::unda::{source, target, Range, Route, Settings, Table, TableChoice, Unda};
use cantus_dsp::voices::{PlayMode, Priority};
use std::sync::{Arc, Mutex, OnceLock};

/// Where each number sits in the telemetry.
pub mod tel {
    pub const POS: usize = 8;
    pub const ENV: usize = 11;
    pub const LFO: usize = 15;
    pub const CUTOFF: usize = 23;
    pub const RES: usize = 25;
    pub const PEAK: usize = 27;
    pub const LANES: usize = 29;
    pub const ALIVE: usize = 30;
    pub const OVERSAMPLE: usize = 31;
}

/// The parameter key of every modulatable value, in `cantus_dsp::unda::target` order.
pub const TARGET_KEYS: [&str; target::COUNT] = [
    "gain",
    "flt.routing",
    "osc1.pos",
    "osc1.pitch",
    "osc1.fine",
    "osc1.level",
    "osc1.pan",
    "osc1.detune",
    "osc1.blend",
    "osc1.width",
    "osc1.spread",
    "osc1.warp1.amt",
    "osc1.warp2.amt",
    "osc1.tilt",
    "osc1.stretch",
    "osc1.oddeven",
    "osc1.density",
    "osc1.glen",
    "osc1.spray",
    "osc2.pos",
    "osc2.pitch",
    "osc2.fine",
    "osc2.level",
    "osc2.pan",
    "osc2.detune",
    "osc2.blend",
    "osc2.width",
    "osc2.spread",
    "osc2.warp1.amt",
    "osc2.warp2.amt",
    "osc2.tilt",
    "osc2.stretch",
    "osc2.oddeven",
    "osc2.density",
    "osc2.glen",
    "osc2.spray",
    "osc3.pos",
    "osc3.pitch",
    "osc3.fine",
    "osc3.level",
    "osc3.pan",
    "osc3.detune",
    "osc3.blend",
    "osc3.width",
    "osc3.spread",
    "osc3.warp1.amt",
    "osc3.warp2.amt",
    "osc3.tilt",
    "osc3.stretch",
    "osc3.oddeven",
    "osc3.density",
    "osc3.glen",
    "osc3.spray",
    "sub.level",
    "sub.pan",
    "noise.level",
    "noise.pan",
    "noise.color",
    "noise.width",
    "flt1.cutoff",
    "flt1.res",
    "flt1.drive",
    "flt1.key",
    "flt1.stereo",
    "flt1.mix",
    "flt2.cutoff",
    "flt2.res",
    "flt2.drive",
    "flt2.key",
    "flt2.stereo",
    "flt2.mix",
    "env1.delay",
    "env1.a",
    "env1.h",
    "env1.d",
    "env1.s",
    "env1.r",
    "env2.delay",
    "env2.a",
    "env2.h",
    "env2.d",
    "env2.s",
    "env2.r",
    "env3.delay",
    "env3.a",
    "env3.h",
    "env3.d",
    "env3.s",
    "env3.r",
    "env4.delay",
    "env4.a",
    "env4.h",
    "env4.d",
    "env4.s",
    "env4.r",
    "lfo1.rate",
    "lfo2.rate",
    "lfo3.rate",
    "lfo4.rate",
    "lfo5.rate",
    "lfo6.rate",
    "lfo7.rate",
    "lfo8.rate",
];

/// The target a parameter key modulates, if it is modulatable.
pub fn target_of(key: &str) -> Option<usize> {
    TARGET_KEYS.iter().position(|k| *k == key)
}

/// Every target's range, from the parameter model (computed once).
pub fn ranges() -> &'static [Range; target::COUNT] {
    static R: OnceLock<[Range; target::COUNT]> = OnceLock::new();
    R.get_or_init(|| {
        let mut r = [Range::default(); target::COUNT];
        for (t, key) in TARGET_KEYS.iter().enumerate() {
            let info = InstrumentKind::Wavetable
                .param(key)
                .unwrap_or_else(|| panic!("{key} is not a Unda parameter"));
            r[t] = Range {
                min: info.min,
                max: info.max,
                log: info.logarithmic,
            };
        }
        r
    })
}

// ---- the table registry ----------------------------------------------------------------------

struct Entry {
    audio: Arc<SampleData>,
    frame: usize,
    table: Arc<Table>,
}

const REGISTRY: usize = 1024;
static ENTRIES: [OnceLock<Entry>; REGISTRY] = [const { OnceLock::new() }; REGISTRY];
static NEXT: Mutex<usize> = Mutex::new(0);

/// The imported table for this audio and frame size, if [`prepare`] built it. Lock-free
/// and allocation-free: what `configure` calls on the audio thread.
pub fn lookup(audio: &Arc<SampleData>, frame: usize) -> Option<Arc<Table>> {
    ENTRIES
        .iter()
        .map_while(|e| e.get())
        .find(|e| e.frame == frame && Arc::ptr_eq(&e.audio, audio))
        .map(|e| e.table.clone())
}

/// Build the mip levels of every imported table `spec` names that the registry does not
/// hold yet. Off the audio thread (it allocates and takes a lock): the backend and the
/// offline renderer call it, through `inst::units_for`, before sending the snapshot.
pub fn prepare(spec: &NativeSpec, assets: &Assets) {
    for (o, key) in TABLE_KEYS.iter().enumerate() {
        let Some(audio) = super::asset(assets, key) else {
            continue;
        };
        let frame = frame_of(spec, o);
        if lookup(audio, frame).is_some() {
            continue;
        }
        let Ok(mut next) = NEXT.lock() else {
            return;
        };
        if lookup(audio, frame).is_some() {
            continue;
        }
        if *next >= REGISTRY {
            log::warn!("Unda: the table registry is full; {key} stays silent");
            return;
        }
        let table = Table::from_audio(&audio.frames, audio.channels as usize, frame);
        let _ = ENTRIES[*next].set(Entry {
            audio: audio.clone(),
            frame,
            table: Arc::new(table),
        });
        *next += 1;
    }
}

fn frame_of(spec: &NativeSpec, o: usize) -> usize {
    const FRAME_KEYS: [&str; 3] = ["osc1.frame", "osc2.frame", "osc3.frame"];
    spec.get(FRAME_KEYS[o]).round().max(1.0) as usize
}

// ---- parameters → settings --------------------------------------------------------------------

fn flag(v: f32) -> bool {
    v >= 0.5
}

fn index(v: f32) -> usize {
    v.round().max(0.0) as usize
}

/// One parameter onto the settings. Every key the model has lands somewhere; the ones
/// that are modulatable land in `base` as well.
fn apply(s: &mut Settings, key: &str, v: f32) {
    if let Some(t) = target_of(key) {
        s.base[t] = v;
    }
    match key {
        "quality" => s.oversample = 1 << index(v).min(2),
        "voice.poly" => s.polyphony = index(v).clamp(1, 32),
        "voice.mode" => s.mode = PlayMode::from_index(v),
        "voice.prio" => s.priority = Priority::from_index(v),
        "voice.glide" => s.glide_s = v,
        "voice.glide_mode" => s.glide_always = flag(v),
        "voice.bend" => s.bend_st = v,
        "voice.vel" => s.vel_depth = v,
        "voice.drift" => s.drift = v,
        "voice.seed" => s.seed = v.round().max(0.0) as u64,
        "sub.on" => s.sub_on = flag(v),
        "sub.wave" => s.sub_wave = index(v).min(4),
        "sub.oct" => s.sub_oct = v.round(),
        "sub.route" => s.sub_route = Route::from_index(v),
        "noise.on" => s.noise_on = flag(v),
        "noise.route" => s.noise_route = Route::from_index(v),
        _ => {
            let Some((head, field)) = key.split_once('.') else {
                return;
            };
            let (name, n) = head.split_at(head.len() - 1);
            let Some(n) = n.parse::<usize>().ok().and_then(|n| n.checked_sub(1)) else {
                return;
            };
            match name {
                "osc" if n < 3 => {
                    let o = &mut s.osc[n];
                    match field {
                        "on" => o.on = flag(v),
                        "wt" => o.table = TableChoice::Factory(index(v)),
                        "oct" => o.oct = v.round(),
                        "semi" => o.semi = v.round(),
                        "phase" => o.phase = v / 360.0,
                        "rand" => o.rand = v,
                        "route" => o.route = Route::from_index(v),
                        "uni" => o.unison = index(v).clamp(1, 16),
                        "ucurve" => o.curve = index(v),
                        "stack" => o.stack = index(v),
                        "warp1" => o.warp[0] = Warp::from_index(v),
                        "warp2" => o.warp[1] = Warp::from_index(v),
                        "warp1.src" => o.warp_src[0] = index(v).min(5),
                        "warp2.src" => o.warp_src[1] = index(v).min(5),
                        "engine" => o.engine = Engine::from_index(v),
                        "partials" => o.partials = index(v).clamp(1, 256),
                        "gwin" => o.window = Window::from_index(v),
                        "gpitch" => o.pitch_rand = v,
                        "greverse" => o.reverse = v,
                        _ => {}
                    }
                }
                "flt" if n < 2 => match field {
                    "on" => s.filters[n].on = flag(v),
                    "type" => s.filters[n].ty = FilterType::from_index(v),
                    _ => {}
                },
                "env" if n < 4 => match field {
                    "ac" => s.env_curves[n].attack = v,
                    "dc" => s.env_curves[n].decay = v,
                    "rc" => s.env_curves[n].release = v,
                    _ => {}
                },
                "lfo" if n < 8 => {
                    let l = &mut s.lfo[n];
                    match field {
                        "wave" => l.wave = LfoWave::from_index(v),
                        "mode" => l.mode = LfoMode::from_index(v),
                        "phase" => l.phase = v / 360.0,
                        "delay" => l.delay = v,
                        "rise" => l.rise = v,
                        "smooth" => l.smooth = v,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

fn curve(c: Curve) -> ModCurve {
    match c {
        Curve::Linear => ModCurve::Linear,
        Curve::Exp => ModCurve::Exp,
        Curve::Log => ModCurve::Log,
        Curve::S => ModCurve::S,
        Curve::Steps => ModCurve::Steps,
    }
}

fn source_index(tag: &str) -> Option<u16> {
    modulation::SOURCES
        .iter()
        .position(|s| s.tag == tag)
        .map(|i| i as u16)
}

/// The DSP's settings for a spec and its decoded assets: every parameter's default, then
/// what the spec stores; the tables looked up; the connections resolved. Allocation-free
/// (it runs on the audio thread with every snapshot).
pub fn settings(spec: &NativeSpec, assets: &Assets) -> Settings {
    let mut s = Settings::default();
    for info in model::PARAMS {
        apply(&mut s, info.key, info.default);
    }
    for (k, v) in &spec.params {
        apply(&mut s, k, *v);
    }
    // An imported table replaces the factory one; for Grana the file is also its sample
    // (an `Arc` clone of the decoded audio: no copy, no allocation).
    for (o, key) in TABLE_KEYS.iter().enumerate() {
        if spec.text(key).is_some() {
            let audio = super::asset(assets, key);
            s.osc[o].table = match audio {
                Some(audio) => lookup(audio, frame_of(spec, o))
                    .map(TableChoice::Imported)
                    .unwrap_or(TableChoice::Missing),
                None => TableChoice::Missing,
            };
            if s.osc[o].engine == Engine::Grana {
                s.osc[o].sample = audio.map(|a| SampleRef {
                    frames: a.frames.clone(),
                    rate: a.rate as f32,
                    channels: a.channels.max(1) as usize,
                });
            }
        }
    }
    s.range = *ranges();
    for (l, key) in model::SHAPE_KEYS.iter().enumerate() {
        if let Some(text) = spec.text(key) {
            let mut pts = [(0.0, 0.0); LFO_POINTS];
            let n = parse_shape(text, &mut pts);
            if n > 0 {
                s.lfo[l].shape = Shape {
                    points: pts,
                    len: n,
                };
            }
        }
    }
    for c in connections(spec).filter(|c| c.on && c.amount != 0.0) {
        let (Some(src), Some(dst)) = (source_index(c.src), target_of(c.dst)) else {
            continue;
        };
        let via = c.via.and_then(source_index);
        let pushed = s.routes.push(ModRoute {
            src,
            dst: dst as u16,
            depth: c.amount,
            via,
            curve: curve(c.curve),
        });
        if pushed {
            s.used_sources[src as usize] = true;
            if let Some(v) = via {
                s.used_sources[v as usize] = true;
            }
        }
    }
    s
}

#[derive(Debug)]
pub struct Unit {
    dsp: Unda,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let _ = ranges();
        let mut dsp = Unda::new(sample_rate);
        dsp.set(&settings(&NativeSpec::new(InstrumentKind::Wavetable), &[]));
        Unit { dsp }
    }
}

impl InstrumentUnit for Unit {
    fn configure(&mut self, spec: &NativeSpec, assets: &Assets) {
        self.dsp.set(&settings(spec, assets));
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.dsp.set_global(source::MACRO + m, spec.get(key));
        }
    }

    fn note_on(&mut self, note: NoteOn) {
        self.dsp.note_on(note.key, note.id, note.velocity);
    }

    fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        self.dsp.note_off(key, id, velocity);
    }

    fn expression(&mut self, id: u64, expression: NoteExpression, value: f32) {
        let which = match expression {
            NoteExpression::Tuning => 0,
            NoteExpression::Pressure => 1,
            NoteExpression::Brightness => 2,
            NoteExpression::Volume => 3,
            NoteExpression::Pan => 4,
            _ => return,
        };
        self.dsp.expression(id, which, value);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        match controller {
            Controller::PitchBend => self
                .dsp
                .set_global(source::PB, (value / 2.0).clamp(-1.0, 1.0)),
            Controller::ModWheel => self.dsp.set_global(source::MW, value),
            Controller::Pressure => self.dsp.set_global(source::AT, value),
            Controller::Expression => self.dsp.set_global(source::EXPR, value),
            Controller::Sustain => self.dsp.sustain(value >= 0.5),
            Controller::Cc(_) => {}
        }
    }

    fn all_notes_off(&mut self) {
        self.dsp.all_notes_off();
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
        t[1] = self.dsp.settings().polyphony as f32;
        t[2] = m.stolen as f32;
        t[tel::POS..tel::POS + 3].copy_from_slice(&m.pos);
        t[tel::ENV..tel::ENV + 4].copy_from_slice(&m.env);
        t[tel::LFO..tel::LFO + 8].copy_from_slice(&m.lfo);
        t[tel::CUTOFF..tel::CUTOFF + 2].copy_from_slice(&m.cutoff);
        t[tel::RES..tel::RES + 2].copy_from_slice(&m.res);
        t[tel::PEAK..tel::PEAK + 2].copy_from_slice(&m.peak);
        t[tel::LANES] = m.lanes as f32;
        t[tel::ALIVE] = 1.0;
        t[tel::OVERSAMPLE] = self.dsp.settings().oversample as f32;
        t
    }
}

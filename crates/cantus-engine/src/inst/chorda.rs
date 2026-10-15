//! Chorda — the modelled grand piano: the engine's unit, and the keyboards its strings are
//! designed for (ADR 0059).
//!
//! **Designs off the audio thread.** A keyboard — every key's string loop fitted to its
//! partials — depends on the tuning (`a4`, `stretch`, `stiffness`) and the sample rate, and
//! takes about a second to compute on the machine's cores. They live in a registry that
//! never evicts (32 tunings at most in a session): [`prepare`] asks for the ones a snapshot
//! needs, on a thread of their own, before the snapshot is sent ([`super::units_for`]); the
//! unit looks its tuning up on the audio thread with a lock-free scan and takes it the
//! moment it is there. A unit is made with the reference keyboard (440 Hz, the model's
//! stretch and stiffness), computed once per session and sample rate — at start-up beside
//! the window ([`warm`]), or by the first unit made, which waits for it. While another
//! tuning is being designed the instrument keeps playing the one it had; an **offline**
//! render waits for it, so an export always plays the tuning asked for.
//!
//! **Controllers, natively**: sustain is the damper pedal, continuously (half-pedalling);
//! CC 67 is the soft pedal (una corda). A piano has no pitch bend.
//!
//! ## Telemetry
//!
//! | Index | What |
//! |---|---|
//! | 0, 1, 2 | voices sounding, polyphony (88), voices stolen (0: one voice per key) |
//! | [`tel::KEY`], [`tel::VELOCITY`] | the last struck key and its velocity |
//! | [`tel::PEDAL`], [`tel::SOFT`] | the sustain and soft pedals, 0…1 |
//! | [`tel::PEAK_L`], [`tel::PEAK_R`] | output peaks |
//! | [`tel::DESIGNED`] | 1 when the tuning asked for is the one playing |
//! | [`tel::ALIVE`] | always 1 |

use super::{Controller, InstrumentUnit, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::chorda::{DESIGN_KEYS, PARAMS};
use cantus_core::inst::modulation::{MACRO_KEYS, SLOTS};
use cantus_core::NativeSpec;
use cantus_dsp::chorda::design::{Keyboard, Tuning};
use cantus_dsp::chorda::{Chorda, ChordaSettings};
use cantus_dsp::modmatrix::{ModCurve, ModMatrix, ModRoute};
use std::sync::{Arc, Mutex, OnceLock};

pub mod tel {
    pub const KEY: usize = 8;
    pub const VELOCITY: usize = 9;
    pub const PEDAL: usize = 10;
    pub const SOFT: usize = 11;
    pub const PEAK_L: usize = 12;
    pub const PEAK_R: usize = 13;
    pub const DESIGNED: usize = 14;
    pub const ALIVE: usize = 15;
}

pub const N: usize = PARAMS.len();

/// The sources Chorda produces, in its own index order.
pub const SOURCE_TAGS: [&str; 14] = [
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "vel", "key",
    "mw", "at", "expr", "sus",
];
const SRC_VEL: usize = 8;
const SRC_KEY: usize = 9;
const SRC_MW: usize = 10;
const SRC_AT: usize = 11;
const SRC_EXPR: usize = 12;
const SRC_SUS: usize = 13;

/// The soft pedal's controller.
pub const SOFT_CC: u8 = 67;

// ---- the keyboards' registry ------------------------------------------------------------

/// A keyboard's identity: the sample rate and the three tuning numbers, as bits.
pub type DesignKey = [u32; 4];

const REGISTRY: usize = 32;

struct Entry {
    key: DesignKey,
    keyboard: Arc<Keyboard>,
}

static ENTRIES: [OnceLock<Entry>; REGISTRY] = [const { OnceLock::new() }; REGISTRY];
static NEXT: Mutex<usize> = Mutex::new(0);
static PENDING: Mutex<Vec<DesignKey>> = Mutex::new(Vec::new());

fn tuning_of(values: &[f32; N]) -> Tuning {
    let get = |k: &str| {
        PARAMS
            .iter()
            .position(|i| i.key == k)
            .map_or(0.0, |i| values[i])
    };
    Tuning {
        a4: get("a4") as f64,
        stretch: get("stretch") as f64 / 100.0,
        stiffness: get("stiffness") as f64 / 100.0,
    }
}

fn key_of(tuning: Tuning, fs: f32) -> DesignKey {
    [
        fs.to_bits(),
        (tuning.a4 as f32).to_bits(),
        (tuning.stretch as f32).to_bits(),
        (tuning.stiffness as f32).to_bits(),
    ]
}

/// The keyboard for `key`, if designed: a lock-free scan (the audio thread may call it).
pub fn lookup(key: DesignKey) -> Option<Arc<Keyboard>> {
    ENTRIES
        .iter()
        .filter_map(OnceLock::get)
        .find(|e| e.key == key)
        .map(|e| e.keyboard.clone())
}

fn insert(key: DesignKey, keyboard: Keyboard) {
    let Ok(mut next) = NEXT.lock() else {
        return;
    };
    if lookup(key).is_some() {
        return;
    }
    if *next >= REGISTRY {
        log::warn!("Chorda: the keyboard registry is full; this tuning keeps the last one");
        return;
    }
    let _ = ENTRIES[*next].set(Entry {
        key,
        keyboard: Arc::new(keyboard),
    });
    *next += 1;
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |n| n.get().saturating_sub(1))
        .clamp(1, 8)
}

/// Design `tuning` at `fs` on a thread of its own, unless it is designed or on its way.
pub fn request(tuning: Tuning, fs: f32) {
    let key = key_of(tuning, fs);
    if lookup(key).is_some() {
        return;
    }
    {
        let Ok(mut pending) = PENDING.lock() else {
            return;
        };
        if pending.contains(&key) {
            return;
        }
        pending.push(key);
    }
    let spawned = std::thread::Builder::new()
        .name("cantus-chorda-design".into())
        .spawn(move || {
            insert(key, Keyboard::compute(tuning, fs as f64, threads()));
            if let Ok(mut pending) = PENDING.lock() {
                pending.retain(|k| *k != key);
            }
        });
    if spawned.is_err() {
        if let Ok(mut pending) = PENDING.lock() {
            pending.retain(|k| *k != key);
        }
    }
}

/// The keyboard for `tuning` at `fs`, waiting for it (or computing it here) if need be. Off
/// the audio thread only.
pub fn ensure(tuning: Tuning, fs: f32) -> Arc<Keyboard> {
    let key = key_of(tuning, fs);
    request(tuning, fs);
    loop {
        if let Some(k) = lookup(key) {
            return k;
        }
        let gone = PENDING.lock().map(|p| !p.contains(&key)).unwrap_or(true);
        if gone && lookup(key).is_none() {
            // The registry is full, or its thread could not start: design it here.
            return Arc::new(Keyboard::compute(tuning, fs as f64, threads()));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Ask for the keyboards a snapshot's Chorda tracks need (from [`super::units_for`]).
pub fn prepare(snapshot: &cantus_core::Snapshot, fs: f32) {
    for t in &snapshot.tracks {
        if let Some(n) = t.instrument.native() {
            if n.kind == cantus_core::InstrumentKind::Chorda {
                request(tuning_of(&values_of(n)), fs);
            }
        }
    }
}

/// Start designing the reference keyboard for `fs` (at start-up, beside the window).
pub fn warm(fs: f32) {
    request(Tuning::default(), fs);
}

// ---- parameters → settings --------------------------------------------------------------

pub fn values_of(spec: &NativeSpec) -> [f32; N] {
    let mut v = [0.0; N];
    for (i, info) in PARAMS.iter().enumerate() {
        v[i] = spec.get(info.key);
    }
    v
}

/// The DSP's settings and the felt's hardness (−1…1) for parameter values.
pub fn settings_from(values: &[f32; N]) -> (ChordaSettings, f64) {
    let mut s = ChordaSettings::default();
    let mut hardness = 0.0;
    // The hammer's slowest strike against its fastest: 15.7 (24 dB) at 100 %.
    let span = (5.5f64 / 0.35).ln();
    for (info, v) in PARAMS.iter().zip(values) {
        let v = *v as f64;
        match info.key {
            "hardness" => hardness = v / 100.0,
            "hysteresis" => s.voice.felt.eps = v / 100.0,
            "dynamics" => {
                s.voice.v_max = 5.5;
                s.voice.v_min = 5.5 * cantus_dsp::math::exp(-span * v / 100.0);
            }
            "vel.curve" => s.voice.vel_curve = v,
            "decay" => s.voice.decay = v / 100.0,
            "unison" => s.voice.detune_ct = v,
            "coupling" => s.voice.coupling = v / 100.0,
            "sympathy" => s.sympathetic = v / 100.0,
            "phantom" => s.voice.phantom = v / 100.0,
            "damper" => s.voice.damper = v / 100.0,
            "una" => s.voice.una_depth = v / 100.0,
            "body" => s.body = v / 100.0,
            "tilt" => s.tilt_db = v,
            "width" => s.width = v / 100.0,
            "gain" => s.gain_db = v,
            _ => {}
        }
    }
    (s, hardness)
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
        if spec.get(k.on) < 0.5 || DESIGN_KEYS.contains(&dst) {
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

#[derive(Debug)]
pub struct Unit {
    dsp: Chorda,
    base: [f32; N],
    matrix: ModMatrix,
    sources: [f32; SOURCE_TAGS.len()],
    /// Whether any route reads velocity or key.
    per_note: bool,
    /// The keyboard asked for and the one playing.
    want: DesignKey,
    have: DesignKey,
    fs: f32,
    offline: bool,
}

impl Unit {
    /// Allocates the instrument and takes the reference keyboard (computed once per session
    /// and sample rate, waited for if it is not ready).
    pub fn new(sample_rate: f32) -> Self {
        let fs = if sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        let mut base = [0.0; N];
        for (i, info) in PARAMS.iter().enumerate() {
            base[i] = info.default;
        }
        let mut dsp = Chorda::new(fs);
        let key = key_of(Tuning::default(), fs);
        dsp.set_keyboard(ensure(Tuning::default(), fs));
        let mut u = Unit {
            dsp,
            base,
            matrix: ModMatrix::default(),
            sources: [0.0; SOURCE_TAGS.len()],
            per_note: false,
            want: key,
            have: key,
            fs,
            offline: false,
        };
        u.update();
        u
    }

    fn update(&mut self) {
        let mut src = self.sources;
        src[SRC_VEL] = 0.0;
        src[SRC_KEY] = 0.0;
        let eff = modulated(&self.base, &self.matrix, &src);
        let (s, h) = settings_from(&eff);
        self.dsp.set(&s, h);
    }

    fn reads(&self, src: usize) -> bool {
        self.matrix
            .routes()
            .iter()
            .any(|r| r.src as usize == src || r.via.is_some_and(|v| v as usize == src))
    }

    /// Take the keyboard asked for if it is there (an offline render waits for it).
    fn follow_design(&mut self) {
        if self.want == self.have {
            return;
        }
        if let Some(k) = lookup(self.want) {
            self.dsp.set_keyboard(k);
            self.have = self.want;
            return;
        }
        if !self.offline {
            return;
        }
        // Offline (not the audio thread): wait for the design, two minutes at most.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(1));
            if let Some(k) = lookup(self.want) {
                self.dsp.set_keyboard(k);
                self.have = self.want;
                return;
            }
        }
    }

    pub fn dsp(&self) -> &Chorda {
        &self.dsp
    }
}

impl InstrumentUnit for Unit {
    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.base = values_of(spec);
        self.want = key_of(tuning_of(&self.base), self.fs);
        routes(spec, &mut self.matrix);
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.sources[m] = spec.get(key);
        }
        self.per_note = self.reads(SRC_VEL) || self.reads(SRC_KEY);
        self.update();
        if let Some(k) = lookup(self.want) {
            self.dsp.set_keyboard(k);
            self.have = self.want;
        }
    }

    fn note_on(&mut self, note: NoteOn) {
        self.follow_design();
        if self.per_note {
            let mut src = self.sources;
            src[SRC_VEL] = note.velocity;
            src[SRC_KEY] = ((note.key as f32 - 60.0) / 48.0).clamp(-1.0, 1.0);
            let (s, h) = settings_from(&modulated(&self.base, &self.matrix, &src));
            self.dsp.set(&s, h);
            self.dsp.note_on(note.key, note.velocity, note.id);
            self.update();
        } else {
            self.dsp.note_on(note.key, note.velocity, note.id);
        }
    }

    fn note_off(&mut self, key: u8, id: u64, _velocity: f32) {
        self.dsp.note_off(key, id);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        let src = match controller {
            Controller::ModWheel => SRC_MW,
            Controller::Pressure => SRC_AT,
            Controller::Expression => SRC_EXPR,
            Controller::Sustain => {
                self.dsp.set_pedal(value);
                SRC_SUS
            }
            Controller::Cc(n) if n == SOFT_CC => {
                self.dsp.set_soft(value);
                return;
            }
            Controller::PitchBend | Controller::Cc(_) => return,
        };
        let v = value.clamp(0.0, 1.0);
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
        for s in self.sources[SRC_MW..=SRC_SUS].iter_mut() {
            *s = 0.0;
        }
        self.update();
    }

    fn at_rest(&self) -> bool {
        self.sources[SRC_MW..=SRC_SUS].iter().all(|v| *v == 0.0) && self.dsp.at_rest()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.follow_design();
        self.dsp.process(left, right)
    }

    fn process_par(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        par: &dyn cantus_dsp::par::Parallel,
    ) -> bool {
        self.follow_design();
        self.dsp.process_par(left, right, par)
    }

    fn splits_voices(&self) -> bool {
        true
    }

    fn set_offline(&mut self, offline: bool) {
        self.offline = offline;
    }

    fn active_voices(&self) -> usize {
        self.dsp.active_voices()
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut t = [0.0; TELEMETRY_LEN];
        t[0] = m.voices as f32;
        t[1] = 88.0;
        t[tel::KEY] = m.key as f32;
        t[tel::VELOCITY] = m.velocity;
        t[tel::PEDAL] = m.pedal;
        t[tel::SOFT] = m.soft;
        t[tel::PEAK_L] = m.peak_l;
        t[tel::PEAK_R] = m.peak_r;
        t[tel::DESIGNED] = (self.want == self.have) as u8 as f32;
        t[tel::ALIVE] = 1.0;
        t
    }
}

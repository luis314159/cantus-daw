//! Chorda — the modelled grand piano: the engine's unit, and the keyboards its strings are
//! designed for (ADR 0059).
//!
//! **Designs off every thread that must not wait.** A keyboard — every key's string loop
//! fitted to its partials — depends on the tuning (`a4`, `stretch`, `stiffness`) and the
//! sample rate, and takes a second or two on the machine's cores. One designer thread makes
//! them, one at a time: the reference keyboard at start-up and after a device change
//! ([`warm`]), and each track's tuning once it has *settled* ([`prepare`], on every snapshot:
//! a knob being dragged asks for a tuning a frame, and only the one let go is designed).
//! They live in a registry that never evicts (Unda's rule: what the audio thread holds is
//! never freed there); the unit looks its tuning up with a lock-free scan and takes it the
//! moment it is there. Nothing waits on the interface thread: a unit made before its
//! keyboard exists is **silent**, and the editor says *designing the strings* with the
//! design's progress, until it arrives; while another tuning is designed the instrument
//! plays the one it had. An **offline** render waits for its own (designing it on the spot
//! if it must), so an export always plays the tuning asked for. The three tuning keys are
//! set, never automated or modulated (`InstrumentKind::automatable`).
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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

// ---- the keyboards' registry and their designer ------------------------------------------

/// A keyboard's identity: the sample rate and the three tuning numbers (rounded as
/// [`settled`] rounds them), as bits.
pub type DesignKey = [u32; 4];

/// Keyboards a session keeps. Never evicted (as Unda's tables): a keyboard the audio thread
/// holds is never freed there. Designs are made only of tunings that have settled, so a
/// session rarely holds more than a handful.
const REGISTRY: usize = 128;
/// How long a track's tuning must stay before it is designed (a knob being dragged asks for
/// one tuning a frame; only the one it is let go at is designed).
const SETTLE: std::time::Duration = std::time::Duration::from_millis(250);

struct Entry {
    key: DesignKey,
    keyboard: Arc<Keyboard>,
}

static ENTRIES: [OnceLock<Entry>; REGISTRY] = [const { OnceLock::new() }; REGISTRY];
static NEXT: Mutex<usize> = Mutex::new(0);

/// A tuning asked for: by a track at a sample rate (the newest replaces the track's last),
/// or at once.
struct Want {
    owner: Option<(u64, u32)>,
    key: DesignKey,
    tuning: Tuning,
    fs: f32,
    since: std::time::Instant,
    now: bool,
}

#[derive(Default)]
struct Designer {
    wants: Vec<Want>,
    /// The design under way and its keys done (0…88).
    busy: Option<(DesignKey, Arc<std::sync::atomic::AtomicUsize>)>,
    /// Designs the registry had no room for: never designed again this session.
    refused: Vec<DesignKey>,
    running: bool,
}

static DESIGNER: Mutex<Designer> = Mutex::new(Designer {
    wants: Vec::new(),
    busy: None,
    refused: Vec::new(),
    running: false,
});
static WAKE: std::sync::Condvar = std::sync::Condvar::new();

// What the audio thread may know of the refusals without the designer's lock (review of
// fix3): a fresh unit waits silent only for a tuning that can still arrive.
/// The registry has no room (or too many designs failed to keep count): no tuning not
/// designed yet will arrive this session.
static NONE_ARRIVES: AtomicBool = AtomicBool::new(false);
/// The designer's thread could not start: nothing arrives until it does.
static NO_DESIGNER: AtomicBool = AtomicBool::new(false);
/// Designs that failed, written under the designer's lock, read lock-free.
const FAILED_KEPT: usize = 64;
static FAILED: [OnceLock<DesignKey>; FAILED_KEPT] = [const { OnceLock::new() }; FAILED_KEPT];
static FAILED_N: AtomicUsize = AtomicUsize::new(0);

/// Refuse `key` for the session (`d` is the designer, locked): never designed again, and
/// said where the audio thread can read it.
fn refuse(d: &mut Designer, key: DesignKey, full: bool) {
    if !d.refused.contains(&key) {
        d.refused.push(key);
    }
    if full {
        NONE_ARRIVES.store(true, Ordering::Release);
        return;
    }
    let n = FAILED_N.load(Ordering::Acquire);
    if FAILED.iter().take(n).any(|k| k.get() == Some(&key)) {
        return;
    }
    if n < FAILED_KEPT {
        let _ = FAILED[n].set(key);
        FAILED_N.store(n + 1, Ordering::Release);
    } else {
        NONE_ARRIVES.store(true, Ordering::Release);
    }
}

/// Whether the keyboard for `key` is designed or can still be: neither refused nor waiting
/// on a designer that could not start. Lock-free and allocation-free: the audio thread
/// asks it (a fresh unit drops the reference keyboard only for a tuning on its way).
pub fn can_arrive(key: DesignKey) -> bool {
    if lookup(key).is_some() {
        return true;
    }
    if NONE_ARRIVES.load(Ordering::Acquire) || NO_DESIGNER.load(Ordering::Acquire) {
        return false;
    }
    let n = FAILED_N.load(Ordering::Acquire);
    !FAILED.iter().take(n).any(|k| k.get() == Some(&key))
}

/// The tuning parameter values ask for.
pub fn tuning_of(values: &[f32; N]) -> Tuning {
    let get = |k: &str| {
        PARAMS
            .iter()
            .position(|i| i.key == k)
            .map_or(0.0, |i| values[i])
    };
    settled(Tuning {
        a4: get("a4") as f64,
        stretch: get("stretch") as f64,
        stiffness: get("stiffness") as f64,
    })
}

/// A tuning as it is designed: A4 to a hundredth of a hertz, the stretch and the stiffness
/// to a thousandth (below anything heard; two knob positions that close share a design).
pub fn settled(t: Tuning) -> Tuning {
    let round = |v: f64, step: f64| (v / step).round() * step;
    Tuning {
        a4: round(t.a4, 0.01),
        stretch: round(t.stretch, 0.001),
        stiffness: round(t.stiffness, 0.001),
    }
}

pub fn key_of(tuning: Tuning, fs: f32) -> DesignKey {
    let t = settled(tuning);
    [
        fs.to_bits(),
        (t.a4 as f32).to_bits(),
        (t.stretch as f32).to_bits(),
        (t.stiffness as f32).to_bits(),
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

/// Keep a design: whether the registry had room.
fn insert(key: DesignKey, keyboard: Arc<Keyboard>) -> bool {
    let Ok(mut next) = NEXT.lock() else {
        return false;
    };
    if lookup(key).is_some() {
        return true;
    }
    if *next >= REGISTRY {
        return false;
    }
    let _ = ENTRIES[*next].set(Entry { key, keyboard });
    *next += 1;
    true
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |n| n.get().saturating_sub(1))
        .clamp(1, 8)
}

/// Ask for `tuning` at `fs`: for `owner` (a track: designed once it has settled, replacing
/// what the track asked for before) or, with no owner, at once. Never waits.
fn ask(owner: Option<(u64, u32)>, tuning: Tuning, fs: f32) {
    let tuning = settled(tuning);
    let key = key_of(tuning, fs);
    if lookup(key).is_some() {
        if let (Some(o), Ok(mut d)) = (owner, DESIGNER.lock()) {
            d.wants.retain(|w| w.owner != Some(o));
        }
        return;
    }
    let Ok(mut d) = DESIGNER.lock() else {
        return;
    };
    if d.refused.contains(&key) {
        return;
    }
    let now = std::time::Instant::now();
    match owner {
        Some(o) => {
            if let Some(w) = d.wants.iter_mut().find(|w| w.owner == Some(o)) {
                if w.key != key {
                    *w = Want {
                        owner,
                        key,
                        tuning,
                        fs,
                        since: now,
                        now: false,
                    };
                }
            } else {
                d.wants.push(Want {
                    owner,
                    key,
                    tuning,
                    fs,
                    since: now,
                    now: false,
                });
            }
        }
        None => {
            if !d.wants.iter().any(|w| w.owner.is_none() && w.key == key) {
                d.wants.push(Want {
                    owner,
                    key,
                    tuning,
                    fs,
                    since: now,
                    now: true,
                });
            }
        }
    }
    if !d.running {
        let spawned = if faults::spawn_fails() {
            Err(std::io::Error::other("no thread (a test)"))
        } else {
            std::thread::Builder::new()
                .name("cantus-chorda-design".into())
                .spawn(design_loop)
        };
        d.running = spawned.is_ok();
        NO_DESIGNER.store(!d.running, Ordering::Release);
        if !d.running {
            faults::spawn_failed();
        }
    }
    WAKE.notify_all();
}

/// Faults the tests put in the designer's way (never set outside them).
mod faults {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    static SPAWN_FAILS: AtomicBool = AtomicBool::new(false);
    /// A design of this A4 (its bits as f32) panics on the designer's thread.
    static PANICS_AT: AtomicU32 = AtomicU32::new(0);

    /// Spawns that failed (the tests prove they took that path).
    static SPAWN_FAILED: AtomicU32 = AtomicU32::new(0);

    pub fn spawn_fails() -> bool {
        SPAWN_FAILS.load(Ordering::Relaxed)
    }

    pub fn spawn_failed() {
        SPAWN_FAILED.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub fn spawns_failed() -> u32 {
        SPAWN_FAILED.load(Ordering::Relaxed)
    }

    pub fn panics(a4: f64) -> bool {
        let p = PANICS_AT.load(Ordering::Relaxed);
        p != 0 && p == (a4 as f32).to_bits()
    }

    #[cfg(test)]
    pub fn set_spawn_fails(on: bool) {
        SPAWN_FAILS.store(on, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub fn set_panics_at(a4: Option<f64>) {
        PANICS_AT.store(a4.map_or(0, |a| (a as f32).to_bits()), Ordering::Relaxed);
    }

    /// Every fault off again, when a test ends — passed or not.
    #[cfg(test)]
    pub struct Reset;

    #[cfg(test)]
    impl Drop for Reset {
        fn drop(&mut self) {
            set_spawn_fails(false);
            set_panics_at(None);
            super::NO_DESIGNER.store(false, Ordering::Release);
        }
    }
}

/// The designer's thread: one design at a time, the wanted ones first, a track's only once
/// it has settled; it ends when nothing is left to design.
fn design_loop() {
    loop {
        let job = {
            let Ok(mut d) = DESIGNER.lock() else {
                return;
            };
            loop {
                d.wants.retain(|w| lookup(w.key).is_none());
                if d.wants.is_empty() {
                    d.running = false;
                    return;
                }
                let ready = d
                    .wants
                    .iter()
                    .position(|w| w.now)
                    .or_else(|| d.wants.iter().position(|w| w.since.elapsed() >= SETTLE));
                if let Some(i) = ready {
                    let w = &d.wants[i];
                    let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let job = (w.key, w.tuning, w.fs, done.clone());
                    d.busy = Some((w.key, done));
                    break job;
                }
                let Ok((g, _)) = WAKE.wait_timeout(d, std::time::Duration::from_millis(50)) else {
                    return;
                };
                d = g;
            }
        };
        let (key, tuning, fs, done) = job;
        // A design that fails (panics) must not leave the designer marked busy and
        // running for good — an export waiting on it would never end (sound review 2): it
        // is refused, and whoever waits designs it on their own thread.
        let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if faults::panics(tuning.a4) {
                panic!("a design that fails (a test)");
            }
            Keyboard::compute_counted(tuning, fs as f64, threads(), &done)
        }));
        let kept = made.map(|k| insert(key, Arc::new(k)));
        let mut d = DESIGNER.lock().unwrap_or_else(|p| p.into_inner());
        d.busy = None;
        match kept {
            Ok(true) => {}
            Ok(false) => {
                log::warn!("Chorda: no room for another tuning this session; keeping the last");
                refuse(&mut d, key, true);
                d.wants.retain(|w| w.key != key);
            }
            Err(_) => {
                log::error!("Chorda: a string design failed; it is made where it is needed");
                refuse(&mut d, key, false);
                d.wants.retain(|w| w.key != key);
            }
        }
        drop(d);
        WAKE.notify_all();
    }
}

/// How long [`ensure`] waits for the designer before it designs on its own thread.
const ENSURE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(120);

/// The keyboard for `tuning` at `fs`, waiting for the designer — or designing it here when
/// the registry has no room, the design failed there, the designer's thread could not
/// start, or it has not delivered in two minutes (sound review 2: an export must always
/// end). Never on the audio thread: an offline render, a test.
pub fn ensure(tuning: Tuning, fs: f32) -> Arc<Keyboard> {
    let tuning = settled(tuning);
    let key = key_of(tuning, fs);
    let here = || {
        let k = Arc::new(Keyboard::compute(tuning, fs as f64, threads()));
        insert(key, k.clone());
        k
    };
    let since = std::time::Instant::now();
    loop {
        if let Some(k) = lookup(key) {
            return k;
        }
        let refused = DESIGNER.lock().map_or(true, |d| d.refused.contains(&key));
        if refused {
            return here();
        }
        ask(None, tuning, fs);
        let running = DESIGNER.lock().is_ok_and(|d| d.running);
        if !running || since.elapsed() > ENSURE_PATIENCE {
            if let Some(k) = lookup(key) {
                return k;
            }
            return here();
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Whether the design of `tuning` at `fs` was refused this session (the registry full, or
/// its design failed): a live unit keeps the keyboard it had (an export designs its own).
pub fn refused(tuning: Tuning, fs: f32) -> bool {
    let key = key_of(tuning, fs);
    lookup(key).is_none() && DESIGNER.lock().is_ok_and(|d| d.refused.contains(&key))
}

/// For tests (the application's): as if the registry had had no room for `tuning` at
/// `fs`.
#[doc(hidden)]
pub fn refuse_for_tests(tuning: Tuning, fs: f32) {
    if let Ok(mut d) = DESIGNER.lock() {
        refuse(&mut d, key_of(tuning, fs), false);
    }
}

/// How far the design `tuning` at `fs` has come: `None` when it is designed (or never will
/// be), otherwise 0…1 (0 while it waits its turn or for the knob to settle).
pub fn progress(tuning: Tuning, fs: f32) -> Option<f32> {
    let key = key_of(tuning, fs);
    if lookup(key).is_some() {
        return None;
    }
    let d = DESIGNER.lock().ok()?;
    if d.refused.contains(&key) {
        return None;
    }
    match &d.busy {
        Some((k, done)) if *k == key => {
            Some(done.load(std::sync::atomic::Ordering::Relaxed) as f32 / 88.0)
        }
        _ => Some(0.0),
    }
}

/// Ask for the keyboards a snapshot's Chorda tracks need (from [`super::units_for`], on
/// every snapshot): each track's tuning, designed once it has settled.
pub fn prepare(snapshot: &cantus_core::Snapshot, fs: f32) {
    for t in &snapshot.tracks {
        if let Some(n) = t.instrument.native() {
            if n.kind == cantus_core::InstrumentKind::Chorda {
                ask(Some((t.id.0, fs.to_bits())), tuning_of(&values_of(n)), fs);
            }
        }
    }
}

/// Start designing the reference keyboard for `fs` (at start-up, after a device change).
pub fn warm(fs: f32) {
    ask(None, Tuning::default(), fs);
}

/// Whether the reference keyboard for `fs` is designed or asked for.
pub fn is_warm(fs: f32) -> bool {
    let key = key_of(Tuning::default(), fs);
    lookup(key).is_some()
        || DESIGNER.lock().is_ok_and(|d| {
            d.wants.iter().any(|w| w.key == key) || d.busy.as_ref().is_some_and(|b| b.0 == key)
        })
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
    // The hammer's slowest strike against its fastest: 15.7 (24 dB) at 1.
    let span = cantus_dsp::math::ln(5.5f64 / 0.35);
    for (info, v) in PARAMS.iter().zip(values) {
        let v = *v as f64;
        match info.key {
            "hardness" => hardness = v,
            "hysteresis" => s.voice.felt.eps = v,
            "dynamics" => {
                s.voice.v_max = 5.5;
                s.voice.v_min = 5.5 * cantus_dsp::math::exp(-span * v);
            }
            "vel.curve" => s.voice.vel_curve = v,
            "decay" => s.voice.decay = v,
            "unison" => s.voice.detune_ct = v,
            "coupling" => s.voice.coupling = v,
            "sympathy" => s.sympathetic = v,
            "phantom" => s.voice.phantom = v,
            "damper" => s.voice.damper = v,
            "una" => s.voice.una_depth = v,
            "body" => s.body = v,
            "tilt" => s.tilt_db = v,
            "width" => s.width = v,
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
    /// The keyboard asked for (and its tuning) and the one playing (none yet: silent).
    want: DesignKey,
    want_tuning: Tuning,
    have: Option<DesignKey>,
    fs: f32,
    offline: bool,
    /// A note was struck: from then on a tuning on its way waits behind the one playing.
    /// Before, a unit holding only the reference keyboard is silent until its own tuning
    /// arrives (sound review 2: it played A4 440 meanwhile).
    struck: bool,
}

impl Unit {
    /// Allocates the instrument and takes the reference keyboard if it is designed — never
    /// waiting for it (it is asked for, and taken when it arrives).
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
        let have = match lookup(key) {
            Some(k) => {
                dsp.set_keyboard(k);
                Some(key)
            }
            None => {
                warm(fs);
                None
            }
        };
        let mut u = Unit {
            dsp,
            base,
            matrix: ModMatrix::default(),
            sources: [0.0; SOURCE_TAGS.len()],
            per_note: false,
            want: key,
            want_tuning: Tuning::default(),
            have,
            fs,
            offline: false,
            struck: false,
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

    /// Take the keyboard asked for if it is there. Offline (never the audio thread) it is
    /// waited for — designed on the spot if the designer cannot keep it — once.
    fn follow_design(&mut self) {
        if self.have == Some(self.want) {
            return;
        }
        if let Some(k) = lookup(self.want) {
            self.dsp.set_keyboard(k);
            self.have = Some(self.want);
            return;
        }
        if self.offline {
            self.dsp.set_keyboard(ensure(self.want_tuning, self.fs));
            self.have = Some(self.want);
            return;
        }
        // Its own tuning will not come (refused this session, or no designer): a unit with
        // nothing keeps playing — the reference keyboard, as the editor says (review of
        // fix3: it stayed silent for the session).
        if self.have.is_none() && !can_arrive(self.want) {
            let key = key_of(Tuning::default(), self.fs);
            if let Some(k) = lookup(key) {
                self.dsp.set_keyboard(k);
                self.have = Some(key);
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
        self.want_tuning = tuning_of(&self.base);
        self.want = key_of(self.want_tuning, self.fs);
        routes(spec, &mut self.matrix);
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.sources[m] = spec.get(key);
        }
        self.per_note = self.reads(SRC_VEL) || self.reads(SRC_KEY);
        self.update();
        if let Some(k) = lookup(self.want) {
            self.dsp.set_keyboard(k);
            self.have = Some(self.want);
        } else if !self.struck && self.have.is_some() && can_arrive(self.want) {
            // Nothing played yet: silent until its own tuning is designed, never another —
            // unless it will not come, then the keyboard it has plays (lock-free: the audio
            // thread configures).
            self.dsp.clear_keyboard();
            self.have = None;
        }
    }

    fn note_on(&mut self, note: NoteOn) {
        self.follow_design();
        self.struck |= self.have.is_some();
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
        t[tel::DESIGNED] = (self.have == Some(self.want)) as u8 as f32;
        t[tel::ALIVE] = 1.0;
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ensure` on a worker, failed if it has not returned in a minute (it used to spin
    /// for ever).
    fn ensure_within_a_minute(tuning: Tuning) -> Arc<Keyboard> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ensure(tuning, 48_000.0));
        });
        rx.recv_timeout(std::time::Duration::from_secs(60))
            .expect("ensure returned")
    }

    fn tuning(a4: f64) -> Tuning {
        Tuning {
            a4,
            stretch: 1.0,
            stiffness: 1.0,
        }
    }

    /// The designer and its faults are the process's: the tests that use them run one at a
    /// time (review of fix3: in parallel, one test's fault hid another's path), and every
    /// fault is off again when each ends, passed or not.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    fn alone() -> (std::sync::MutexGuard<'static, ()>, faults::Reset) {
        let g = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
        (g, faults::Reset)
    }

    /// Wait until the designer's thread has ended (nothing left to design).
    fn designer_idle() {
        let since = std::time::Instant::now();
        while DESIGNER.lock().is_ok_and(|d| d.running) {
            assert!(since.elapsed().as_secs() < 60, "the designer ends");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn an_export_ends_when_the_designers_thread_cannot_start() {
        let _alone = alone();
        designer_idle();
        let failed = faults::spawns_failed();
        faults::set_spawn_fails(true);
        let k = ensure_within_a_minute(tuning(431.17));
        assert!(
            faults::spawns_failed() > failed,
            "the export took the path where the designer could not start"
        );
        assert_eq!(k.fs, 48_000.0);
    }

    #[test]
    fn an_export_ends_when_a_design_fails_on_the_designers_thread() {
        let _alone = alone();
        faults::set_panics_at(Some(433.33));
        let k = ensure_within_a_minute(tuning(433.33));
        faults::set_panics_at(None);
        assert_eq!(k.fs, 48_000.0);
        assert!(
            DESIGNER
                .lock()
                .is_ok_and(|d| d.refused.contains(&key_of(tuning(433.33), 48_000.0))),
            "the design failed on the designer's thread, and was refused there"
        );
        // The designer is not left busy: another design still comes from it.
        let other = ensure_within_a_minute(tuning(434.44));
        assert_eq!(other.fs, 48_000.0);
    }

    /// Sound review 2: a unit made with the reference keyboard but set to a tuning not
    /// designed yet (a song opened with the Baroque preset) is silent until its own
    /// keyboard arrives — never playing A4 440 meanwhile (ADR 0059 §6).
    #[test]
    fn a_fresh_unit_waits_silent_for_its_own_tuning() {
        let _alone = alone();
        let fs = 48_000.0;
        ensure(Tuning::default(), fs);
        let mut u = Unit::new(fs);
        let mut spec = NativeSpec::new(cantus_core::InstrumentKind::Chorda);
        spec.set("a4", 437.77);
        u.configure(&spec, &[]);
        let on = |u: &mut Unit, id| {
            u.note_on(NoteOn {
                key: 60,
                velocity: 0.8,
                id,
                channel: 0,
            })
        };
        on(&mut u, 1);
        let (mut l, mut r) = (vec![0.0f32; 4_800], vec![0.0f32; 4_800]);
        u.process(&mut l, &mut r);
        assert!(l.iter().all(|x| *x == 0.0), "silent before its keyboard");
        assert_eq!(u.telemetry()[tel::DESIGNED], 0.0);
        ensure(tuning(437.77), fs);
        on(&mut u, 2);
        u.process(&mut l, &mut r);
        assert!(l.iter().any(|x| *x != 0.0), "its own keyboard plays");
        assert_eq!(u.telemetry()[tel::DESIGNED], 1.0);
    }

    /// Review of fix3: a fresh unit set to a tuning the session refuses (the registry full,
    /// a design that failed) keeps the reference keyboard and plays — the editor's caption
    /// says "keeping this one" — and one that dropped it while the tuning was on its way
    /// takes it back when the tuning is refused, as when the designer cannot start.
    #[test]
    fn a_fresh_unit_whose_tuning_is_refused_keeps_playing() {
        let _alone = alone();
        let fs = 48_000.0;
        ensure(Tuning::default(), fs);
        let play = |u: &mut Unit, id| {
            u.note_on(NoteOn {
                key: 60,
                velocity: 0.8,
                id,
                channel: 0,
            });
            let (mut l, mut r) = (vec![0.0f32; 4_800], vec![0.0f32; 4_800]);
            u.process(&mut l, &mut r);
            l.iter().any(|x| *x != 0.0)
        };
        let set = |u: &mut Unit, a4: f64| {
            let mut spec = NativeSpec::new(cantus_core::InstrumentKind::Chorda);
            spec.set("a4", a4 as f32);
            u.configure(&spec, &[]);
        };
        // Refused before it is set.
        refuse_for_tests(tuning(441.23), fs);
        let mut u = Unit::new(fs);
        set(&mut u, 441.23);
        assert!(
            play(&mut u, 1),
            "a refused tuning keeps the reference keyboard"
        );
        // Refused after the unit went silent for it.
        let mut u = Unit::new(fs);
        set(&mut u, 442.34);
        refuse_for_tests(tuning(442.34), fs);
        assert!(play(&mut u, 2), "the reference keyboard comes back");
        // No designer: the same.
        designer_idle();
        faults::set_spawn_fails(true);
        let mut u = Unit::new(fs);
        // The snapshot asks for the track's tuning (`prepare`); the thread cannot start.
        ask(Some((99, fs.to_bits())), tuning(443.45), fs);
        set(&mut u, 443.45);
        assert!(
            play(&mut u, 3),
            "nothing will design it: the reference keyboard plays"
        );
    }
}

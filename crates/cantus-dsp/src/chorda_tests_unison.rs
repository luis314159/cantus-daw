//! Every unison in tune, string by string (round 41, `docs/round41/chorda-tune.md`). The
//! owner, 2026-11-20, on beta.15: *"Chorda: some notes sound like some of their strings are
//! slightly out of tune … E4, C#3, D3, C3, B2, E5. Alone, they sound like one of the strings
//! inside the key is out of tune."*
//!
//! Each key of the shipped voicing is struck and its strings' bridge force — the voice alone,
//! before the sympathetic strings, the board and the room, its action's noises and phantom
//! partials off (none of them moves a string's frequency) — is read with `modekit`: near each
//! of its partials the unison rings as a few damped modes (one per loop, as the bridge
//! couples them), each found to a thousandth of a hertz. Measured:
//!
//! - **each loop alone** (the others muted on the bridge, [`Chorda::set_loops_kept`]): its
//!   one mode per partial, against the key's designed curve (`tests_tuning::curve_of`, the
//!   keyboard without the technician's table) — where a tuner's mute leaves each string;
//! - **the unison sounding together**: its modes per partial, their power-weighted pitch, and
//!   the fastest beat audible between them — what the ear hears as "a string out of tune".

use super::modekit::{analyse, fastest_beat, most_heard_beat, Beat, PartialModes};
use super::string::MAX_STRINGS;
use super::tables;
use super::tests_tuning::{curve_of, keyboard_of};
use super::voicing::Voicing;
use super::*;
use std::sync::Mutex;

const FS: f64 = 48_000.0;
/// The render's length (s): a mid-register unison's slow beat needs seconds to show.
const SECONDS: f64 = 4.0;
/// The fit's start (s): the hammer has long left the strings.
const T0: f64 = 0.1;
/// The partials read per key (below 5 kHz).
pub const PARTIALS: u32 = 8;
/// A beat is judged while its partial sounds within this many dB of its peak and it swings
/// the partial's level by at least [`MIN_DEPTH_DB`], over the first [`BEAT_SECONDS`].
pub const FLOOR_DB: f64 = 40.0;
pub const MIN_DEPTH_DB: f64 = 1.0;
pub const BEAT_SECONDS: f64 = 5.0;
/// The velocities every key is struck at.
pub const VELOCITIES: [f32; 3] = [0.3, 0.6, 0.9];
const CENTS: f64 = 1_200.0 / std::f64::consts::LN_2;

/// A model-2 unit voiced `v` on its own keyboard, its action's noises and phantom partials
/// off (neither touches a string's frequency; both put sound near the partials that is not a
/// string's mode).
pub fn dry_unit(v: &Voicing) -> Chorda {
    let mut c = Chorda::new(FS as f32);
    c.set_keyboard(keyboard_of(v));
    let mut s = ChordaSettings {
        model: 2,
        noise: 0.0,
        ..ChordaSettings::default()
    };
    s.voice.phantom = 0.0;
    c.set(&s, 0.0);
    c.set_voicing(v);
    c
}

/// Key `m` struck at `vel` with its loops kept as `keep`: its voice's bridge force (N),
/// [`SECONDS`] long, rendered alone.
pub fn dry(v: &Voicing, m: u8, vel: f32, keep: [f64; MAX_STRINGS]) -> Vec<f64> {
    let mut c = dry_unit(v);
    c.set_loops_kept(m, keep);
    c.note_on(m, vel, 1);
    let mut voice = c.voice((m - tables::LOWEST) as usize).clone();
    let n = (SECONDS * FS) as usize;
    let (mut out, mut noise) = (vec![0.0; n], vec![0.0; n]);
    voice.render(&mut out, &mut noise);
    out
}

/// One key at one velocity, as its modes show it.
#[derive(Clone, Debug)]
pub struct KeyModes {
    pub key: u8,
    pub vel: f32,
    /// Its loops (vertical strings first, then the horizontal loop) and vertical strings.
    pub loops: usize,
    pub vertical: usize,
    /// The curve's partials (Hz, `[n − 1]`).
    pub curve: Vec<f64>,
    /// Each loop alone: its mode per partial (Hz, `[loop][n − 1]`; NaN if none).
    pub alone: Vec<Vec<f64>>,
    /// The unison together, per partial.
    pub together: Vec<PartialModes>,
    /// What the voice laid: each loop's detune (cents).
    pub detune_ct: [f64; MAX_STRINGS],
}

impl KeyModes {
    /// Loop `i` alone, partial `n`, against the curve (cents).
    pub fn alone_cents(&self, i: usize, n: u32) -> f64 {
        let k = (n - 1) as usize;
        match (self.alone[i].get(k), self.curve.get(k)) {
            (Some(f), Some(c)) => CENTS * crate::math::ln(f / c),
            _ => f64::NAN,
        }
    }

    /// The largest difference (Hz) between two vertical strings alone at partial `n`.
    pub fn alone_spread_hz(&self, n: u32) -> f64 {
        let k = (n - 1) as usize;
        let fs: Vec<f64> = self.alone[..self.vertical]
            .iter()
            .filter_map(|a| a.get(k).copied())
            .filter(|f| f.is_finite())
            .collect();
        let lo = fs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = fs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if fs.len() < 2 {
            0.0
        } else {
            hi - lo
        }
    }

    /// The largest difference (cents) between two vertical strings alone at partial `n` (the
    /// horizontal loop is the same wire's other polarisation, set its designed `polar_ct`
    /// apart on purpose, and judged with the unison it rings in).
    pub fn alone_spread_ct(&self, n: u32) -> f64 {
        let cs: Vec<f64> = (0..self.vertical)
            .map(|l| self.alone_cents(l, n))
            .filter(|c| c.is_finite())
            .collect();
        if cs.len() < 2 {
            return 0.0;
        }
        let lo = cs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = cs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        hi - lo
    }

    /// The unison's partial `n`.
    pub fn partial(&self, n: u32) -> Option<&PartialModes> {
        self.together.iter().find(|p| p.n == n)
    }

    /// The unison together at partial `n`: its power-weighted pitch against the curve (cents).
    pub fn together_cents(&self, n: u32) -> f64 {
        match self.partial(n) {
            Some(p) => CENTS * crate::math::ln(p.mean_hz() / p.near_hz),
            None => f64::NAN,
        }
    }

    /// The fastest audible beat of the unison's partial `n`.
    pub fn beat(&self, n: u32) -> Option<Beat> {
        fastest_beat(self.partial(n)?, BEAT_SECONDS, FLOOR_DB, MIN_DEPTH_DB)
    }

    /// The audible beat of the unison's partial `n` the ear hears most swells of in its
    /// first [`BEAT_SECONDS`] ([`most_heard_beat`]).
    pub fn heard(&self, n: u32) -> Option<Beat> {
        most_heard_beat(self.partial(n)?, BEAT_SECONDS, FLOOR_DB, MIN_DEPTH_DB)
    }
}

/// Key `m` of `v` at `vel`: each loop alone and the unison together (its first
/// [`PARTIALS`]).
pub fn key_modes(v: &Voicing, m: u8, vel: f32) -> KeyModes {
    key_modes_to(v, m, vel, PARTIALS)
}

/// [`key_modes`] on the unison's first `partials` only (the gates read 1–3).
pub fn key_modes_to(v: &Voicing, m: u8, vel: f32, partials: u32) -> KeyModes {
    let i = (m - tables::LOWEST) as usize;
    let curve_key = curve_of(v)[i];
    let curve: Vec<f64> = (1..=partials)
        .map(|n| curve_key.partial(n))
        .take_while(|f| *f < 5_000.0)
        .collect();
    let curve = if curve.is_empty() {
        vec![curve_key.partial(1)]
    } else {
        curve
    };
    // How many loops it lays (and what): one silent strike's lay.
    let mut c = dry_unit(v);
    c.note_on(m, vel, 1);
    let laid = c.loops_laid(m);
    let f0 = curve_key.f0();
    let together_x = dry(v, m, vel, [1.0; MAX_STRINGS]);
    let together = analyse(&together_x, FS, f0, &curve, T0, SECONDS, laid.loops + 2);
    let mut alone = Vec::new();
    for l in 0..laid.loops {
        let mut keep = [0.0; MAX_STRINGS];
        keep[l] = 1.0;
        let x = dry(v, m, vel, keep);
        let near = &curve[..curve.len().min(3)];
        let p = analyse(&x, FS, f0, near, T0, SECONDS, 2);
        alone.push(
            (0..near.len())
                .map(|k| {
                    p.iter()
                        .find(|q| q.n as usize == k + 1)
                        .and_then(|q| q.modes.first())
                        .map_or(f64::NAN, |md| md.hz)
                })
                .collect(),
        );
    }
    KeyModes {
        key: m,
        vel,
        loops: laid.loops,
        vertical: laid.vertical,
        curve,
        alone,
        together,
        detune_ct: laid.detune_ct,
    }
}

/// Every key of `keys` at every velocity of [`VELOCITIES`], on `threads` threads, sorted.
pub fn map_keys(v: &Voicing, keys: &[u8], threads: usize) -> Vec<KeyModes> {
    map_keys_to(v, keys, threads, PARTIALS)
}

/// [`map_keys`] on the first `partials` only.
pub fn map_keys_to(v: &Voicing, keys: &[u8], threads: usize, partials: u32) -> Vec<KeyModes> {
    let _ = keyboard_of(v);
    let jobs: Vec<(u8, f32)> = keys
        .iter()
        .flat_map(|m| VELOCITIES.iter().map(move |x| (*m, *x)))
        .collect();
    let out = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for t in 0..threads {
            let (out, jobs) = (&out, &jobs);
            s.spawn(move || {
                for (m, x) in jobs.iter().skip(t).step_by(threads) {
                    let r = key_modes_to(v, *m, *x, partials);
                    out.lock().unwrap().push(r);
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by(|a, b| a.key.cmp(&b.key).then(a.vel.total_cmp(&b.vel)));
    o
}

/// The keys `CHORDA_TUNE_KEYS` picks (`lo-hi` or a comma list; every key if unset).
pub fn diag_keys() -> Vec<u8> {
    let Ok(s) = std::env::var("CHORDA_TUNE_KEYS") else {
        return (tables::LOWEST..=tables::HIGHEST).collect();
    };
    if let Some((a, b)) = s.split_once('-') {
        if let (Ok(a), Ok(b)) = (a.trim().parse::<u8>(), b.trim().parse::<u8>()) {
            return (a..=b).collect();
        }
    }
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

/// The voicing a diagnostic reads: the shipped one, `CHORDA_TUNE_SPEC` applied over it.
pub fn diag_voicing() -> Voicing {
    let mut v = Voicing::default();
    if let Ok(spec) = std::env::var("CHORDA_TUNE_SPEC") {
        assert!(v.apply(&spec), "a key of CHORDA_TUNE_SPEC is not one");
    }
    v
}

/// How many loops key `m` lays on `v`: its strings, and the horizontal loop on a key of one
/// or two strings when model 2's polarisation is on.
pub fn loops_of(v: &Voicing, m: u8) -> usize {
    let n = tables::strings(m) as usize;
    n + usize::from(v.polar != 0.0 && n < 3)
}

/// The threads a map runs on: `CHORDA_THREADS`, else the machine's (at most 16).
pub fn threads() -> usize {
    std::env::var("CHORDA_THREADS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get().min(16)))
}

/// The kit itself on the real model: a key of one string and no horizontal loop rings one
/// mode per partial; muting every loop but one leaves that loop's mode.
#[test]
fn a_single_loop_rings_one_mode_per_partial() {
    let v = Voicing::default();
    let k = key_modes(&v, 60, 0.6);
    for l in 0..k.loops {
        for n in 1..=3 {
            let c = k.alone_cents(l, n);
            assert!(c.is_finite() && c.abs() < 10.0, "C4 loop {l} p{n}: {c}");
        }
    }
    assert!(k.beat(1).is_some() || k.partial(1).is_some_and(|p| p.modes.len() == 1));
}

fn fmt(x: f64, w: usize, p: usize) -> String {
    if x.is_finite() {
        format!("{x:+w$.p$}")
    } else {
        format!("{:>w$}", "-")
    }
}

/// Diagnostic: every key of `CHORDA_TUNE_KEYS` (all if unset) on the voicing
/// `CHORDA_TUNE_SPEC` over the shipped one — each loop's laid detune, each loop alone on
/// partials 1–3 against the curve (cents), the unison together's pitch on partials 1–3
/// (cents) and the fastest audible beat on partials 1–3 (Hz, its depth dB), and the fastest
/// on 4–8, at the velocity `CHORDA_TUNE_VEL` (0.6). Run with
/// `cargo test -p cantus-dsp --lib unison_map -- --ignored --nocapture`.
#[test]
#[ignore]
fn unison_map() {
    let v = diag_voicing();
    let keys = diag_keys();
    let vel: f32 = std::env::var("CHORDA_TUNE_VEL")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0.6);
    let _ = keyboard_of(&v);
    let out = Mutex::new(Vec::new());
    let th = threads();
    std::thread::scope(|s| {
        for t in 0..th {
            let (out, keys, v) = (&out, &keys, &v);
            s.spawn(move || {
                for m in keys.iter().skip(t).step_by(th) {
                    let r = key_modes(v, *m, vel);
                    out.lock().unwrap().push(r);
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by_key(|k| k.key);
    eprintln!("key  detunes ct | alone p1 / p2 / p3 (cents, per loop) | together p1 p2 p3 ct | beat p1 p2 p3 Hz (dB) | p4-8 Hz");
    for k in &o {
        let det: Vec<String> = (0..k.loops).map(|i| fmt(k.detune_ct[i], 5, 2)).collect();
        let alone: Vec<String> = (1..=3u32)
            .map(|n| {
                (0..k.loops)
                    .map(|i| fmt(k.alone_cents(i, n), 5, 2))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        let tog: Vec<String> = (1..=3u32).map(|n| fmt(k.together_cents(n), 5, 2)).collect();
        let beats: Vec<String> = (1..=3u32)
            .map(|n| match k.beat(n) {
                Some(b) => format!("{:.3}({:.1})", b.rate, b.depth_db),
                None => "-".into(),
            })
            .collect();
        let upper = (4..=PARTIALS)
            .filter_map(|n| k.beat(n).map(|b| b.rate))
            .fold(0.0f64, f64::max);
        eprintln!(
            "{:3} {} | {} | {} | {} | {:.3}",
            k.key,
            det.join(" "),
            alone.join(" / "),
            tog.join(" "),
            beats.join(" "),
            upper
        );
    }
}

/// Diagnostic: each partial's modes of the keys `CHORDA_TUNE_KEYS` at `CHORDA_TUNE_VEL`
/// (0.6): frequency against the curve (cents), decay (T60 s) and level (dB).
#[test]
#[ignore]
fn unison_modes() {
    let v = diag_voicing();
    let vel: f32 = std::env::var("CHORDA_TUNE_VEL")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0.6);
    for m in diag_keys() {
        let k = key_modes(&v, m, vel);
        eprintln!("key {m}: {} loops", k.loops);
        for p in &k.together {
            let ms: Vec<String> = p
                .modes
                .iter()
                .map(|md| {
                    format!(
                        "{:+.3}ct T60 {:.2}s {:.1}dB",
                        CENTS * crate::math::ln(md.hz / p.near_hz),
                        6.907_755 / md.decay.max(1e-9),
                        md.level_db(0.0) - p.peak_db
                    )
                })
                .collect();
            eprintln!("  p{} {:.2} Hz: {}", p.n, p.near_hz, ms.join(" | "));
        }
    }
}

/// One key's summary over [`VELOCITIES`]: the fastest audible beat on partials 1–3 (Hz, its
/// depth dB, its partial), the farthest any loop alone sits from the curve on partials 1–3
/// (cents), the widest two strings alone stand apart there (Hz), the unison together's
/// partials 1–3 against the curve at velocity 0.6 (cents) and the fastest beat on 4–8 (Hz).
#[derive(Clone, Debug, Default)]
pub struct KeySummary {
    pub key: u8,
    pub strings: usize,
    pub beat: f64,
    pub beat_depth: f64,
    pub beat_partial: u32,
    pub alone_ct: f64,
    pub alone_spread_hz: f64,
    pub together_ct: [f64; 3],
    pub upper_beat: f64,
    /// The most swells heard of a beat on partials 1–3 in the first five seconds, its rate.
    pub cycles: f64,
    pub cycles_rate: f64,
}

/// [`KeySummary`] of each key in `maps` (every velocity of a key together).
pub fn summarise(maps: &[KeyModes]) -> Vec<KeySummary> {
    let mut out: Vec<KeySummary> = Vec::new();
    for k in maps {
        if out.last().is_none_or(|s| s.key != k.key) {
            out.push(KeySummary {
                key: k.key,
                strings: k.vertical,
                together_ct: [f64::NAN; 3],
                ..KeySummary::default()
            });
        }
        let s = out.last_mut().expect("a key");
        for n in 1..=3u32 {
            if let Some(b) = k.beat(n) {
                if b.rate > s.beat {
                    (s.beat, s.beat_depth, s.beat_partial) = (b.rate, b.depth_db, n);
                }
            }
            if let Some(b) = k.heard(n) {
                if b.cycles > s.cycles {
                    (s.cycles, s.cycles_rate) = (b.cycles, b.rate);
                }
            }
            for l in 0..k.loops {
                let c = k.alone_cents(l, n);
                if c.is_finite() && c.abs() > s.alone_ct.abs() {
                    s.alone_ct = c;
                }
            }
            s.alone_spread_hz = s.alone_spread_hz.max(k.alone_spread_hz(n));
            if k.vel == 0.6 {
                s.together_ct[(n - 1) as usize] = k.together_cents(n);
            }
        }
        for n in 4..=PARTIALS {
            if let Some(b) = k.beat(n) {
                s.upper_beat = s.upper_beat.max(b.rate);
            }
        }
    }
    out
}

/// Diagnostic: every key of `CHORDA_TUNE_KEYS` (all if unset) at velocities 0.3, 0.6 and 0.9
/// on `CHORDA_TUNE_SPEC` over the shipped voicing, one line each ([`KeySummary`]); with
/// `CHORDA_UNISON_JSON` the same as JSON (`tools/chorda-voicing/unison_page.py`). Run with
/// `cargo test -p cantus-dsp --lib unison_table -- --ignored --nocapture`.
#[test]
#[ignore]
fn unison_table() {
    let v = diag_voicing();
    let maps = map_keys(&v, &diag_keys(), threads());
    let sums = summarise(&maps);
    eprintln!("key n | beat p1-3 Hz (dB, partial) | alone worst ct | alone spread Hz | together p1 p2 p3 ct | beat p4-8 Hz | swells in 5 s (Hz)");
    let mut json = String::from("[\n");
    for (i, s) in sums.iter().enumerate() {
        eprintln!(
            "{:3} {} | {:.3} ({:4.1}, p{}) | {} | {:.3} | {} {} {} | {:.3} | {:.2} ({:.3})",
            s.key,
            s.strings,
            s.beat,
            s.beat_depth,
            s.beat_partial,
            fmt(s.alone_ct, 5, 2),
            s.alone_spread_hz,
            fmt(s.together_ct[0], 5, 2),
            fmt(s.together_ct[1], 5, 2),
            fmt(s.together_ct[2], 5, 2),
            s.upper_beat,
            s.cycles,
            s.cycles_rate
        );
        let num = |x: f64| {
            if x.is_finite() {
                format!("{x:.4}")
            } else {
                "null".into()
            }
        };
        json += &format!(
            "{{\"key\":{},\"cycles\":{},\"strings\":{},\"beat\":{},\"depth\":{},\"partial\":{},\"alone\":{},\"spread\":{},\"together\":[{},{},{}],\"upper\":{}}}{}\n",
            s.key,
            num(s.cycles),
            s.strings,
            num(s.beat),
            num(s.beat_depth),
            s.beat_partial,
            num(s.alone_ct),
            num(s.alone_spread_hz),
            num(s.together_ct[0]),
            num(s.together_ct[1]),
            num(s.together_ct[2]),
            num(s.upper_beat),
            if i + 1 < sums.len() { "," } else { "" }
        );
    }
    json += "]\n";
    if let Ok(path) = std::env::var("CHORDA_UNISON_JSON") {
        std::fs::write(&path, json).expect("the JSON written");
        eprintln!("wrote {path}");
    }
}

/// Key `m`'s level on a unit voiced `v` (the whole instrument, as played) struck at the lab's
/// mezzo-forte (velocity 75), over 0–0.5, 0.25–1.25 and 1.25–3.4 s (dB, both channels' mean
/// power) — round 40's measure of what a retune did to a key's colour of decay.
pub fn key_levels(v: &Voicing, m: u8) -> [f64; 3] {
    use super::tests_engine::{perform, Ev};
    let frames = (3.4 * FS) as usize;
    let (l, r) = perform(
        &mut super::tests_tuning::unit(v),
        &[(0, Ev::On(m, 75.0 / 127.0, 1))],
        frames,
        512,
        1,
    );
    let lv = |a: f64, b: f64| {
        let (a, b) = ((a * FS) as usize, ((b * FS) as usize).min(l.len()));
        let p: f64 = l[a..b]
            .iter()
            .chain(&r[a..b])
            .map(|x| (*x as f64) * (*x as f64))
            .sum::<f64>()
            / (2 * (b - a)) as f64;
        10.0 * crate::math::log10(p.max(1e-30))
    };
    [lv(0.0, 0.5), lv(0.25, 1.25), lv(1.25, 3.4)]
}

/// Diagnostic: each key of `CHORDA_TUNE_KEYS` — its level over the three windows of
/// [`key_levels`] on `DSP_VERSION` 10's voicing ([`Voicing::technician`]) and on `CHORDA_TUNE_SPEC` over the shipped one, and the change
/// (dB). `CHORDA_TUNE_BASE` (a spec) sets the comparison's first voicing instead.
#[test]
#[ignore]
fn unison_levels() {
    let mut a = Voicing::technician();
    if let Ok(spec) = std::env::var("CHORDA_TUNE_BASE") {
        assert!(a.apply(&spec), "a key of CHORDA_TUNE_BASE is not one");
    }
    let b = diag_voicing();
    let keys = diag_keys();
    let out = Mutex::new(Vec::new());
    let th = threads();
    std::thread::scope(|s| {
        for t in 0..th {
            let (out, keys, a, b) = (&out, &keys, &a, &b);
            s.spawn(move || {
                for m in keys.iter().skip(t).step_by(th) {
                    let r = (*m, key_levels(a, *m), key_levels(b, *m));
                    out.lock().unwrap().push(r);
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by_key(|e| e.0);
    eprintln!("key | before 0-0.5 0.25-1.25 1.25-3.4 dB | after | change");
    let mut sq = [0.0f64; 3];
    for (m, x, y) in &o {
        let d = [y[0] - x[0], y[1] - x[1], y[2] - x[2]];
        for (s, v) in sq.iter_mut().zip(d) {
            *s += v * v;
        }
        eprintln!(
            "{m:3} | {:6.1} {:6.1} {:6.1} | {:6.1} {:6.1} {:6.1} | {:+5.2} {:+5.2} {:+5.2}",
            x[0], x[1], x[2], y[0], y[1], y[2], d[0], d[1], d[2]
        );
    }
    let n = o.len().max(1) as f64;
    eprintln!(
        "rms change: {:.2} {:.2} {:.2} dB",
        (sq[0] / n).sqrt(),
        (sq[1] / n).sqrt(),
        (sq[2] / n).sqrt()
    );
}

/// A key's name (C4 = 60).
pub fn key_name(m: u8) -> String {
    const N: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", N[(m % 12) as usize], m as i32 / 12 - 1)
}

/// The listening set for the owner (`CHORDA_RENDER_OUT` the folder): for each key of
/// `CHORDA_LISTEN_KEYS` (the six he named if unset) `<key>-held` — struck alone at mezzo-forte
/// and held 7 s, then at piano and at forte, 5 s each — and `<key>-phrase`: the key, its
/// octave below, a chord holding it, a short line around it ending on it; and `six-keys`, the
/// named keys one after another. Each rendered on the voicing before (`CHORDA_TUNE_BASE` over
/// `DSP_VERSION` 10's, [`Voicing::technician`]) and after (`CHORDA_TUNE_SPEC` over it), 24-bit,
/// as `-before.wav` / `-after.wav`. Run with
/// `cargo test -p cantus-dsp --lib tune_listen -- --ignored --nocapture`.
#[test]
#[ignore]
fn tune_listen() {
    use super::tests_tuning::{play, write_wav};
    let Ok(out) = std::env::var("CHORDA_RENDER_OUT") else {
        return;
    };
    let dir = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&dir).unwrap();
    let mut before = Voicing::technician();
    if let Ok(spec) = std::env::var("CHORDA_TUNE_BASE") {
        assert!(before.apply(&spec), "a key of CHORDA_TUNE_BASE is not one");
    }
    let after = diag_voicing();
    let keys: Vec<u8> = std::env::var("CHORDA_LISTEN_KEYS")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![48, 49, 50, 47, 64, 76]);
    let (pp, mf, ff) = (40.0 / 127.0, 75.0 / 127.0, 110.0 / 127.0);
    type Notes = Vec<(f64, f64, u8, f32)>;
    let mut jobs: Vec<(String, Notes, f64)> = Vec::new();
    for &k in &keys {
        let name = key_name(k).replace('#', "s");
        jobs.push((
            format!("{name}-held"),
            vec![(0.0, 7.0, k, mf), (7.5, 5.0, k, pp), (13.0, 5.0, k, ff)],
            1.5,
        ));
        // A major or minor triad holding the key, under it where there is room.
        let chord = if k >= 52 {
            vec![k - 7, k - 3, k]
        } else {
            vec![k, k + 4, k + 7]
        };
        let mut ph = vec![(0.0, 3.5, k, mf), (4.0, 3.5, k - 12, mf), (4.0, 3.5, k, mf)];
        for c in &chord {
            ph.push((8.0, 3.5, *c, mf));
        }
        for (i, d) in [-2i32, 0, 2, 4, 2, 0].iter().enumerate() {
            let t = 12.0 + 0.45 * i as f64;
            ph.push((t, 0.42, (k as i32 + d) as u8, mf));
        }
        ph.push((12.0 + 0.45 * 6.0, 4.0, k, mf));
        jobs.push((format!("{name}-phrase"), ph, 2.0));
    }
    let six: Vec<_> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (i as f64 * 4.0, 3.6, *k, mf))
        .collect();
    jobs.push(("named-keys".into(), six, 2.0));
    // The blind kit's level-matched copies, outside the kit's own folder (`abx/`), so no file
    // a listener receives carries a name (`tools/chorda-voicing/tune_abx.py` draws them).
    let abx = dir.join("matched");
    std::fs::create_dir_all(&abx).unwrap();
    std::thread::scope(|s| {
        for (name, notes, tail) in &jobs {
            let (dir, abx, before, after) = (&dir, &abx, &before, &after);
            s.spawn(move || {
                let pair = [play(before, notes, *tail), play(after, notes, *tail)];
                for ((l, r), which) in pair.iter().zip(["before", "after"]) {
                    let peak = l.iter().chain(r).fold(0.0f32, |m, x| m.max(x.abs()));
                    eprintln!(
                        "{name}-{which}: peak {peak:.3}, rms {:.1} dBFS",
                        rms_db(l, r)
                    );
                    write_wav(&dir.join(format!("{name}-{which}.wav")), l, r);
                }
                // The blind test's copies: both at one RMS (−24 dBFS, lowered together if a
                // peak would pass −1 dBFS), so a level never tells them apart.
                let gains: Vec<f64> = pair.iter().map(|(l, r)| -24.0 - rms_db(l, r)).collect();
                let over = pair
                    .iter()
                    .zip(&gains)
                    .map(|((l, r), g)| {
                        let p = l.iter().chain(r).fold(0.0f32, |m, x| m.max(x.abs())) as f64;
                        20.0 * crate::math::log10(p.max(1e-12)) + g + 1.0
                    })
                    .fold(0.0f64, f64::max);
                for (((l, r), g), which) in pair.iter().zip(&gains).zip(["before", "after"]) {
                    let k = crate::math::powf(10.0f64, (g - over) / 20.0) as f32;
                    let (l, r): (Vec<f32>, Vec<f32>) = (
                        l.iter().map(|x| x * k).collect(),
                        r.iter().map(|x| x * k).collect(),
                    );
                    write_wav(&abx.join(format!("{name}-{which}.wav")), &l, &r);
                }
            });
        }
    });
}

/// The RMS of a stereo render (dBFS, both channels).
fn rms_db(l: &[f32], r: &[f32]) -> f64 {
    let p = l
        .iter()
        .chain(r)
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        / (l.len() + r.len()).max(1) as f64;
    10.0 * crate::math::log10(p.max(1e-30))
}

/// The most swells (whole beats) the ear hears on partials 1–3 of key `m`'s unison at `vel` on
/// `v` in the first [`BEAT_SECONDS`] — the together render alone (no loop muted), for a search.
pub fn together_beat(v: &Voicing, m: u8, vel: f32) -> f64 {
    let i = (m - tables::LOWEST) as usize;
    let key = curve_of(v)[i];
    let near: Vec<f64> = (1..=3)
        .map(|n| key.partial(n))
        .enumerate()
        .filter(|(n, f)| *n == 0 || *f < 5_000.0)
        .map(|(_, f)| f)
        .collect();
    let x = dry(v, m, vel, [1.0; MAX_STRINGS]);
    let loops = tables::strings(m) as usize + usize::from(v.polar != 0.0 && tables::strings(m) < 3);
    analyse(&x, FS, key.f0(), &near, T0, SECONDS, loops + 2)
        .iter()
        .filter_map(|p| most_heard_beat(p, BEAT_SECONDS, FLOOR_DB, MIN_DEPTH_DB))
        .map(|b| b.cycles)
        .fold(0.0, f64::max)
}

/// The search's target (swells heard in the first five seconds): under the gate's one, with
/// room for what the search does not see (`CHORDA_SEARCH` overrides it).
pub fn search_target() -> f64 {
    std::env::var("CHORDA_SEARCH")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0.75)
}

/// The gate: no audible beat on partials 1–3 swells and falls a whole time in the first five
/// seconds — no beat faster than one every five seconds while it sounds.
pub const BEAT_GATE_CYCLES: f64 = 1.0;

/// Diagnostic, the technician's search (round 41): for each key of `CHORDA_TUNE_KEYS` with a
/// unison, on `CHORDA_TUNE_SPEC` over the shipped voicing, the least `uni_tight` (0: the
/// designed spread … 1: none) whose fastest audible beat on partials 1–3, at velocities 0.3,
/// 0.6 and 0.9, is under [`search_target()`] — each unison kept as wide as it can be without a beat
/// the ear counts (a slightly detuned unison locked on its bridge rings the slow second stage
/// of its decay, Weinreich 1977; a tuned-dead one does not). Bisection, eight steps. Prints
/// the table as Rust.
#[test]
#[ignore]
fn unison_search() {
    let base = diag_voicing();
    let keys: Vec<u8> = diag_keys()
        .into_iter()
        .filter(|m| tables::strings(*m) > 1)
        .collect();
    let out = Mutex::new(Vec::new());
    let th = threads();
    let worst = |v: &Voicing, m: u8| {
        VELOCITIES
            .iter()
            .map(|x| together_beat(v, m, *x))
            .fold(0.0f64, f64::max)
    };
    std::thread::scope(|s| {
        for t in 0..th {
            let (out, keys, base) = (&out, &keys, &base);
            s.spawn(move || {
                for m in keys.iter().skip(t).step_by(th) {
                    let i = (*m - tables::LOWEST) as usize;
                    let at = |x: f32| {
                        let mut v = *base;
                        v.uni_tight[i] = x;
                        worst(&v, *m)
                    };
                    let (mut lo, mut hi) = (0.0f32, 1.0f32);
                    let first = at(0.0);
                    let tight = if first <= search_target() {
                        0.0
                    } else {
                        for _ in 0..8 {
                            let mid = 0.5 * (lo + hi);
                            if at(mid) <= search_target() {
                                hi = mid;
                            } else {
                                lo = mid;
                            }
                        }
                        hi
                    };
                    let last = at(tight);
                    out.lock().unwrap().push((*m, tight, first, last));
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by_key(|e| e.0);
    for (m, t, a, b) in &o {
        eprintln!("key {m}: uni_tight {t:.4}  beat {a:.3} -> {b:.3} Hz");
    }
    let spec: Vec<String> = o
        .iter()
        .map(|(m, t, _, _)| format!("uni_tight.{m}={t}"))
        .collect();
    eprintln!("SPEC {}", spec.join(","));
}

/// Diagnostic, the technician's last pass re-measured (round 41): each key A0–E6 of
/// `CHORDA_TUNE_SPEC` over the shipped voicing struck at velocity 0.6 through the whole
/// instrument and measured as round 40's gate measures it (`tunekit`), its table entry moved
/// by what it still needs (secant steps from the second round; `CHORDA_TUNE_ROUNDS`, 3), and
/// the table printed as Rust and as a spec. Keys above E6 keep no table (round 40: there the
/// board's tail is as much of the measure as the strings, and a table does not converge).
#[test]
#[ignore]
fn unison_retable() {
    use super::tests_tuning::map;
    let mut v = diag_voicing();
    let rounds: usize = std::env::var("CHORDA_TUNE_ROUNDS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(3);
    let keys: Vec<u8> = (21u8..=88).collect();
    let mut prev: Vec<(f32, f64)> = Vec::new();
    for r in 0..rounds {
        let m = map(&v, &keys, 0.6, threads());
        let mut worst = 0.0f64;
        let mut next = v.tune;
        for (j, (k, t)) in m.iter().enumerate() {
            let i = (*k - tables::LOWEST) as usize;
            let (now, err) = (v.tune[i], t.pitch);
            worst = worst.max(err.abs());
            // A secant step once two rounds are in (the table's slope is not exactly 1: the
            // board's share moves with the key), else the whole error.
            let step = match prev.get(j) {
                Some((was, e0)) if (err - e0).abs() > 1e-3 && (now - was).abs() > 1e-4 => {
                    let slope = (err - e0) / (now - was) as f64;
                    if (0.3..=3.0).contains(&slope) {
                        err / slope
                    } else {
                        err
                    }
                }
                _ => err,
            };
            next[i] = ((now as f64 - step) * 100.0).round() as f32 / 100.0;
        }
        eprintln!("round {r}: worst {worst:.2} cents");
        prev = m
            .iter()
            .map(|(k, t)| (v.tune[(*k - tables::LOWEST) as usize], t.pitch))
            .collect();
        v.tune = next;
    }
    let m = map(&v, &keys, 0.6, threads());
    let worst = m.iter().map(|(_, t)| t.pitch.abs()).fold(0.0f64, f64::max);
    eprintln!("final: worst {worst:.2} cents");
    let rows: Vec<String> = v
        .tune
        .chunks(8)
        .enumerate()
        .map(|(c, ch)| {
            let vals: Vec<String> = ch.iter().map(|x| format!("{x:.2}")).collect();
            format!(
                "    {}, // {}–{}",
                vals.join(", "),
                21 + c * 8,
                21 + c * 8 + ch.len() - 1
            )
        })
        .collect();
    eprintln!("TABLE\n{}", rows.join("\n"));
    let spec: Vec<String> = (21u8..=108)
        .map(|k| format!("tune.{k}={}", v.tune[(k - 21) as usize]))
        .collect();
    eprintln!("SPEC {}", spec.join(","));
}

/// The most the loops of a unison, each alone, may stand apart on partials 1–3 (cents): a
/// fraction of a cent, as a fine technician leaves a unison — 0.5 cent is a beat every 7 s
/// at C4's fundamental, every 2.3 s at its third partial.
pub const ALONE_SPREAD_CT: f64 = 0.5;

/// What the gate allows past [`ALONE_SPREAD_CT`] (cents): the search read the strings alone
/// at velocity 0.6; a soft or hard strike's fit reads a partial a few hundredths of a cent
/// apart (G2's third partial, 0.06 at 0.3).
pub const ALONE_KIT_CT: f64 = 0.1;

/// The least `uni_tight` for key `m` on `v` (bisection, eight steps) whose unison, at every
/// velocity of [`VELOCITIES`], the ear hears beat fewer than [`search_target()`] times on
/// partials 1–3 and whose loops alone stand within [`ALONE_SPREAD_CT`] of each other there —
/// and the swells heard (past the target when no width meets both).
pub fn least_tight(v: &Voicing, m: u8) -> (f32, f64) {
    let i = (m - tables::LOWEST) as usize;
    let at = |x: f32| {
        let mut w = *v;
        w.uni_tight[i] = x;
        let swells = VELOCITIES
            .iter()
            .map(|vel| together_beat(&w, m, *vel))
            .fold(0.0f64, f64::max);
        // The loops alone (their frequencies do not depend on the velocity).
        let k = key_modes_to(&w, m, 0.6, 3);
        let apart = (1..=3).map(|n| k.alone_spread_ct(n)).fold(0.0f64, f64::max);
        if apart > ALONE_SPREAD_CT {
            swells.max(search_target() + apart)
        } else {
            swells
        }
    };
    let first = at(0.0);
    if first <= search_target() {
        return (0.0, first);
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut best = at(1.0);
    for _ in 0..8 {
        let mid = 0.5 * (lo + hi);
        let b = at(mid);
        if b <= search_target() {
            (hi, best) = (mid, b);
        } else {
            lo = mid;
        }
    }
    (hi, best)
}

/// Diagnostic, the technician's search with the bridge re-voiced (round 41): for each key of
/// `CHORDA_TUNE_KEYS` with a unison, on `CHORDA_TUNE_SPEC` over the shipped voicing — its
/// unison as wide as it can be without a beat the ear counts ([`least_tight`]), and its
/// bridge's share of the strings' loss (`coupling.<key>`) lowered until its sustain is the
/// shipped voicing's again: the mean of its level's change over 0.25–1.25 s and 1.25–3.4 s
/// ([`key_levels`]) within a tenth of a decibel of none (bisection on the share's log, from
/// a tenth to one). A unison tuned closer rings less of the slow second stage a detuned one
/// rang (Weinreich 1977), so less of the strings' loss goes through the bridge. Prints the
/// spec.
#[test]
#[ignore]
fn unison_voice_search() {
    let base = diag_voicing();
    let shipped = Voicing::technician();
    let keys: Vec<u8> = diag_keys()
        .into_iter()
        .filter(|m| tables::strings(*m) > 1)
        .collect();
    let out = Mutex::new(Vec::new());
    let th = threads();
    std::thread::scope(|s| {
        for t in 0..th {
            let (out, keys, base, shipped) = (&out, &keys, &base, &shipped);
            s.spawn(move || {
                for m in keys.iter().skip(t).step_by(th) {
                    let i = (*m - tables::LOWEST) as usize;
                    let want = key_levels(shipped, *m);
                    let trial = |c: f32| {
                        let mut v = *base;
                        v.couplings[i] = c;
                        let (tight, beat) = least_tight(&v, *m);
                        v.uni_tight[i] = tight;
                        let got = key_levels(&v, *m);
                        let d = 0.5 * ((got[1] - want[1]) + (got[2] - want[2]));
                        (tight, beat, d, got)
                    };
                    let at_one = trial(1.0);
                    let mut pick = (1.0f32, at_one);
                    if at_one.2 < -0.1 {
                        // Lower the share (log bisection): the sustain rises as it falls.
                        let (mut lo, mut hi) = (crate::math::ln(0.1f64), 0.0f64);
                        for _ in 0..7 {
                            let mid = 0.5 * (lo + hi);
                            let c = crate::math::exp(mid) as f32;
                            let r = trial(c);
                            if (r.2).abs() < (pick.1 .2).abs() {
                                pick = (c, r);
                            }
                            if r.2 < 0.0 {
                                hi = mid;
                            } else {
                                lo = mid;
                            }
                            if r.2.abs() <= 0.1 {
                                break;
                            }
                        }
                    }
                    let (c, (tight, beat, d, got)) = pick;
                    eprintln!(
                        "key {m}: coupling {c:.3} uni_tight {tight:.3} beat {beat:.3} Hz, level change {:+.2} {:+.2} {:+.2} dB",
                        got[0] - want[0],
                        got[1] - want[1],
                        got[2] - want[2]
                    );
                    let _ = d;
                    out.lock().unwrap().push((*m, c, tight));
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by_key(|e| e.0);
    let spec: Vec<String> = o
        .iter()
        .flat_map(|(m, c, t)| [format!("coupling.{m}={c}"), format!("uni_tight.{m}={t}")])
        .collect();
    eprintln!("SPEC {}", spec.join(","));
}

/// The stiffness scatter's scales the optimiser tries (against `uni_b`'s).
pub const UNI_B_TRIES: [f32; 5] = [0.02, 0.05, 0.1, 0.2, 0.35];

/// The bridge's shares of the strings' loss the optimiser tries (against the register's).
pub const COUPLING_TRIES: [f32; 4] = [1.0, 0.6, 0.35, 0.2];

/// Diagnostic, the technician's pass per key (round 41): for each key of `CHORDA_TUNE_KEYS`
/// with a unison, on `CHORDA_TUNE_SPEC` over the shipped voicing, every way of laying it the
/// pass knows — the fan or the pair (three strings), the stiffness scatter at each of
/// [`UNI_B_TRIES`], the bridge's share of the strings' loss at each of [`COUPLING_TRIES`] (a
/// unison tuned closer rings less of a detuned one's slow second stage; less of the loss
/// through the bridge lets it sustain as it did) — each as wide as it can be without a beat the ear counts
/// ([`least_tight`]), and the one that keeps the key's level over 0–0.5, 0.25–1.25 and
/// 1.25–3.4 s ([`key_levels`]) closest to the shipped voicing's (the sum of the three
/// changes' sizes, the attack's weighed double). Prints each key's choice and the spec.
#[test]
#[ignore]
fn unison_optimise() {
    let base = diag_voicing();
    let shipped = Voicing::technician();
    let keys: Vec<u8> = diag_keys()
        .into_iter()
        .filter(|m| loops_of(&base, *m) > 1)
        .collect();
    let out = Mutex::new(Vec::new());
    let th = threads();
    std::thread::scope(|s| {
        for t in 0..th {
            let (out, keys, base, shipped) = (&out, &keys, &base, &shipped);
            s.spawn(move || {
                for m in keys.iter().skip(t).step_by(th) {
                    let i = (*m - tables::LOWEST) as usize;
                    let want = key_levels(shipped, *m);
                    let pairs: &[f32] = if tables::strings(*m) == 3 { &[0.0, 1.0] } else { &[0.0] };
                    // (score, pair, scatter, width, swells, level changes, bridge share)
                    type Pick = (f64, f32, f32, f32, f64, [f64; 3], f32);
                    let mut best: Option<Pick> = None;
                    for &pair in pairs {
                        for (&b, &c) in UNI_B_TRIES
                            .iter()
                            .flat_map(|b| COUPLING_TRIES.iter().map(move |c| (b, c)))
                        {
                            let mut v = *base;
                            v.uni_pair[i] = pair;
                            v.uni_b_keys[i] = b;
                            v.couplings[i] = c;
                            let (tight, swells) = least_tight(&v, *m);
                            if swells > search_target() {
                                continue;
                            }
                            v.uni_tight[i] = tight;
                            let got = key_levels(&v, *m);
                            let d = [got[0] - want[0], got[1] - want[1], got[2] - want[2]];
                            let score = 2.0 * d[0].abs() + d[1].abs() + d[2].abs();
                            if best.is_none_or(|x| score < x.0) {
                                best = Some((score, pair, b, tight, swells, d, c));
                            }
                        }
                    }
                    if let Some((score, pair, b, tight, swells, d, c)) = best {
                        eprintln!(
                            "key {m}: pair {pair} uni_b x{b} coupling x{c} uni_tight {tight:.3} swells {swells:.2}; level {:+.2} {:+.2} {:+.2} dB (score {score:.2})",
                            d[0], d[1], d[2]
                        );
                        out.lock().unwrap().push((*m, pair, b, tight, c));
                    } else {
                        eprintln!("key {m}: nothing under the target");
                    }
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by_key(|e| e.0);
    let spec: Vec<String> = o
        .iter()
        .flat_map(|(m, p, b, t, c)| {
            let mut v = vec![format!("uni_b.{m}={b}"), format!("uni_tight.{m}={t}")];
            if *p != 0.0 {
                v.push(format!("uni_pair.{m}={p}"));
            }
            if *c != 1.0 {
                v.push(format!("coupling.{m}={c}"));
            }
            v
        })
        .collect();
    eprintln!("SPEC {}", spec.join(","));
}

/// The shipped voicing's every key A0–C8 at every velocity of [`VELOCITIES`], measured once
/// for every gate here.
fn shipped_modes() -> &'static [KeyModes] {
    static M: std::sync::OnceLock<Vec<KeyModes>> = std::sync::OnceLock::new();
    M.get_or_init(|| {
        let keys: Vec<u8> = (tables::LOWEST..=tables::HIGHEST).collect();
        map_keys_to(&Voicing::default(), &keys, threads(), 3)
    })
}

/// The bound on a string alone's ear's pitch against its key's pitch — the curve and the
/// key's table (cents): half a cent of its own (its place in a unison whose strings stand
/// within [`ALONE_SPREAD_CT`] of each other; a beat every 7 s at C4's fundamental against a
/// string on the key's pitch) and a quarter cent of what its bridge leaves on a string
/// sounding by itself (the in-phase mode's pull is the unison's, not the string's alone).
pub const ALONE_PITCH_CT: f64 = 0.75;

/// The most (cents) the technician's table moves a key, A0–C8: what lies between the strings
/// on the curve and the whole key as the instrument sounds it (the board's and the room's
/// share of its measure, the in-phase mode's pull). `DSP_VERSION` 10's reached 1.21 (C3), laid
/// over strings that beat; with the unisons tuned it is under 0.65 everywhere.
pub const TABLE_CT: f64 = 0.65;

/// Every unison is nearly beatless, as a fine technician leaves it: on partials 1–3, at
/// any velocity, no audible beat swells and falls a whole time in its first five seconds
/// ([`most_heard_beat`]) — no beat faster than one in five seconds while it sounds. The owner
/// heard C#3's third partials beating 5.7 Hz, E4's 5.1 and D3's 3.9 as strings out of tune
/// (beta.15).
#[test]
fn every_unison_is_beatless_on_its_first_three_partials() {
    let mut worst = (0.0f64, 0.0f64, 0u8, 0u32, 0.0f32);
    let mut out = Vec::new();
    for k in shipped_modes() {
        for n in 1..=3 {
            let Some(b) = k.heard(n) else {
                continue;
            };
            if b.cycles >= BEAT_GATE_CYCLES {
                out.push(format!(
                    "key {} ({}) at {}: partial {n} beats {:.3} Hz, {:.2} swells heard, {:.1} dB deep",
                    k.key,
                    key_name(k.key),
                    k.vel,
                    b.rate,
                    b.cycles,
                    b.depth_db
                ));
            }
            if b.cycles > worst.0 {
                worst = (b.cycles, b.rate, k.key, n, k.vel);
            }
        }
    }
    eprintln!(
        "most swells heard on partials 1-3: key {} p{} at {}: {:.2} ({:.3} Hz)",
        worst.2, worst.3, worst.4, worst.0, worst.1
    );
    assert!(
        out.is_empty(),
        "beats the ear counts:
{}",
        out.join(
            "
"
        )
    );
}

/// The strings of each unison, each alone (the others muted on the bridge), stand within a
/// fraction of a cent of each other on partials 1–3 ([`ALONE_SPREAD_CT`]) — a technician
/// tunes each string to the others by their beats.
#[test]
fn the_strings_of_each_unison_alone_agree_within_half_a_cent() {
    let mut worst = (0.0f64, 0u8, 0u32);
    let mut out = Vec::new();
    for k in shipped_modes().iter().filter(|k| k.vertical > 1) {
        for n in 1..=3u32 {
            let d = k.alone_spread_ct(n);
            if d > ALONE_SPREAD_CT + ALONE_KIT_CT {
                out.push(format!(
                    "key {} ({}) at {}: its strings alone {d:.3} cents apart on partial {n}",
                    k.key,
                    key_name(k.key),
                    k.vel
                ));
            }
            if d > worst.0 {
                worst = (d, k.key, n);
            }
        }
    }
    eprintln!(
        "widest strings alone: key {} p{}: {:.3} cents",
        worst.1, worst.2, worst.0
    );
    assert!(
        out.is_empty(),
        "strings alone apart:
{}",
        out.join(
            "
"
        )
    );
}

/// The ear's pitch (cents) of loop `l` of `k` alone: its partials 1–3 against the curve,
/// weighed by a struck string's amplitude and the ear's dominance region
/// ([`super::voice::ear_weight`]).
pub fn alone_pitch(k: &KeyModes, curve: &tables::Key, l: usize) -> f64 {
    let (mut sw, mut sc) = (0.0, 0.0);
    for n in 1..=3u32 {
        let c = k.alone_cents(l, n);
        if c.is_finite() {
            let w = super::voice::ear_weight(curve, n);
            sw += w;
            sc += w * c;
        }
    }
    if sw > 0.0 {
        sc / sw
    } else {
        f64::NAN
    }
}

/// Each string alone sits on the key's designed pitch: its ear's pitch on partials 1–3
/// within [`ALONE_PITCH_CT`] of the curve and the key's table, and the table itself within
/// [`TABLE_CT`] — folded so that a string alone and the unison together (`tests_tune_gate`,
/// the whole key through the instrument) both land.
#[test]
fn every_string_alone_sounds_its_keys_pitch() {
    let v = Voicing::default();
    for (i, t) in v.tune.iter().enumerate() {
        assert!(
            (*t as f64).abs() <= TABLE_CT,
            "key {}: the table moves it {t:+.2} cents",
            i + 21
        );
    }
    let curve = curve_of(&v);
    let mut worst = (0.0f64, 0u8, 0usize);
    let mut out = Vec::new();
    for k in shipped_modes() {
        let key = curve[(k.key - tables::LOWEST) as usize];
        // A key of one string has no unison: its string alone is the key, held to the whole
        // key's cent (`tests_tune_gate`).
        let bound = if k.vertical == 1 { 1.0 } else { ALONE_PITCH_CT };
        let table = v.tune[(k.key - tables::LOWEST) as usize] as f64;
        for l in 0..k.vertical {
            let p = alone_pitch(k, &key, l) - table;
            if p.abs() > bound {
                out.push(format!(
                    "key {} ({}) s{} at {}: alone {p:+.2} cents from its key's pitch (bound {bound})",
                    k.key,
                    key_name(k.key),
                    l + 1,
                    k.vel
                ));
            }
            if p.abs() > worst.0 {
                worst = (p.abs(), k.key, l);
            }
        }
    }
    eprintln!(
        "worst string alone off its key's pitch: key {} s{}: {:.2} cents",
        worst.1,
        worst.2 + 1,
        worst.0
    );
    assert!(
        out.is_empty(),
        "strings alone off their pitch:
{}",
        out.join(
            "
"
        )
    );
}

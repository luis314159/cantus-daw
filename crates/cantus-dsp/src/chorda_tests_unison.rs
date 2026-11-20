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

use super::modekit::{analyse, fastest_beat, Beat, PartialModes};
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
pub const BEAT_SECONDS: f64 = 3.0;
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
    /// What the voice laid: each loop's detune (cents) and stiffness scatter.
    pub detune_ct: [f64; MAX_STRINGS],
    pub eps: [f64; MAX_STRINGS],
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

    /// The partial's level under the key's strongest partial (dB, ≤ 0).
    pub fn below_top(&self, n: u32) -> f64 {
        let top = self
            .together
            .iter()
            .map(|p| p.peak_db)
            .fold(f64::MIN, f64::max);
        self.partial(n).map_or(f64::NEG_INFINITY, |p| p.peak_db - top)
    }
}

/// Key `m` of `v` at `vel`: each loop alone and the unison together.
pub fn key_modes(v: &Voicing, m: u8, vel: f32) -> KeyModes {
    let i = (m - tables::LOWEST) as usize;
    let curve_key = curve_of(v)[i];
    let curve: Vec<f64> = (1..=PARTIALS)
        .map(|n| curve_key.partial(n))
        .take_while(|f| *f < 5_000.0 && *f < 0.45 * FS)
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
        eps: laid.eps,
    }
}

/// Every key of `keys` at every velocity of [`VELOCITIES`], on `threads` threads, sorted.
pub fn map_keys(v: &Voicing, keys: &[u8], threads: usize) -> Vec<KeyModes> {
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
                    let r = key_modes(v, *m, *x);
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

pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get().min(16))
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

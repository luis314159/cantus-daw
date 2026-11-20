//! Every string of every key in tune, each measured alone inside the instrument (round 41,
//! BACKLOG §3e; `docs/round41/chorda-strings.md`). The owner, 2026-11-19: *"en Chorda
//! deberíamos validar afinación de cada cuerda, cuerdas de una nota para ver que suenen
//! afinadas también."*
//!
//! Round 40's gate (`chorda_tests_tune_gate.rs`) measures each key whole, its strings mixed;
//! `chorda_tests_string.rs` one string in the bare string model. Here each loop of each key
//! A0–C8 of the shipped voicing sounds alone through the whole model-2 unit — hammer,
//! bridge and its resonances, phantom partials, sympathetic strings, board, body and room —
//! the others muted as a technician's felt mutes them ([`Chorda::set_loops_kept`]: still on
//! the bridge, loading it, returning nothing): the single strings of A0–F#1 (21–30), both
//! strings of G1–F2 (31–41), the three of F#2–C8 (42–108), and model 2's horizontal
//! polarisation on the one- and two-string keys, each struck at velocities 0.3, 0.6 and 0.9
//! and measured with `tunekit` against its designed pitch: the curve the keyboard is drawn
//! to (`tests_tuning::curve_of`: without the technician's table) moved by the loop's
//! designed detune, and on its upper partials by its designed stiffness scatter.
//!
//! Each loop is also *predicted*: its partials solved from its laid loop's phase and the
//! phase the bridge gives a loop sounding alone ([`super::string::KeyStrings::solo_phase`]).
//! What the measurement shows past the prediction is what no formula here models (the
//! hammer, the phantom partials, the board's and room's colouring of a decaying tone, the
//! kit's own error).

use super::design::{first_phase, second_phase, LoopDesign};
use super::string::{LaidLoops, MAX_STRINGS};
use super::tables::{self, Key};
use super::tests_engine::{perform, Ev};
use super::tests_tuning::{curve_of, keyboard_of, unit};
use super::tunekit::{measure, KeyTune};
use super::voicing::Voicing;
use std::f64::consts::TAU;
use std::sync::{Mutex, OnceLock};

const FS: f64 = 48_000.0;
/// 1.25 s: the kit's pitch window ends at 1.1 s and A0's Hann (4 periods) reaches 73 ms past
/// it. A loop alone has no unison to beat against, so the beat window's 3.3 s is not needed.
const FRAMES: usize = 60_000;
/// The velocities every loop is struck at.
pub const VELOCITIES: [f32; 3] = [0.3, 0.6, 0.9];
/// Partials measured per loop.
const PARTIALS: u32 = 12;
const CENTS: f64 = 1_200.0 / std::f64::consts::LN_2;

/// What a loop is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A vertical string of the key's unison (index among them).
    String(usize),
    /// Model 2's horizontal-polarisation loop.
    Polar,
}

/// One loop of one key, struck alone at one velocity.
#[derive(Clone, Debug)]
pub struct LoopTune {
    pub key: u8,
    /// Its index among the key's loops, and what it is.
    pub index: usize,
    pub kind: Kind,
    /// The key's vertical strings.
    pub strings: usize,
    pub vel: f32,
    /// What the voice laid: its detune (cents) and stiffness scatter.
    pub detune_ct: f64,
    pub eps: f64,
    /// The key's technician's-table correction (cents; every loop of the key carries it).
    pub table_ct: f64,
    /// Partial n's designed offset from the curve (cents; `[n − 1]`).
    pub designed_ct: Vec<f64>,
    /// Partial n's predicted error against its designed pitch (cents): the table, the
    /// bridge's tuning of the unison and the bridge's pull on the loop alone.
    pub predicted_ct: Vec<f64>,
    /// The measurement against the designed partials: each partial's `cents` is its error.
    pub tune: KeyTune,
    /// The prediction weighed as the ear weighs the measured partials (cents).
    pub predicted_pitch: f64,
}

impl LoopTune {
    /// The ear's pitch error (cents) against the designed pitch.
    pub fn error(&self) -> f64 {
        self.tune.pitch
    }

    /// Partial `n`'s error (cents) and its level under the loop's strongest (dB, ≤ 0).
    pub fn partial(&self, n: u32) -> Option<(f64, f64)> {
        let top = self
            .tune
            .partials
            .iter()
            .map(|p| p.level_db)
            .fold(f64::MIN, f64::max);
        self.tune
            .partials
            .iter()
            .find(|p| p.n == n)
            .map(|p| (p.cents, p.level_db - top))
    }

    /// The ear's pitch as measured against the curve (cents): the error plus the designed
    /// offset, weighed alike.
    pub fn pitch_on_curve(&self) -> f64 {
        self.tune.pitch + self.weighed(&self.designed_ct)
    }

    /// `per_partial` (cents, `[n − 1]`) weighed as the kit weighs the measured partials.
    pub fn weighed(&self, per_partial: &[f64]) -> f64 {
        let (mut sw, mut sc) = (0.0, 0.0);
        for p in self
            .tune
            .partials
            .iter()
            .filter(|p| p.n <= 8 && p.near_hz < 5_000.0)
        {
            let w = crate::math::powf(10.0f64, p.level_db / 20.0) * dominance(p.near_hz);
            sw += w;
            sc += w * per_partial[(p.n - 1) as usize];
        }
        if sw > 0.0 {
            sc / sw
        } else {
            0.0
        }
    }
}

/// The kit's dominance weight (`tunekit::dominance`, the same formula).
fn dominance(hz: f64) -> f64 {
    let o = crate::math::log2(hz / 700.0) / 1.5;
    0.1 + 0.9 * crate::math::exp(-0.5 * o * o)
}

/// A loop's own phase at `w` with fraction `frac` and its sections' pole angles scaled by
/// `1 + eps`, as [`super::string::KeyStrings::set_string_stiffness_set`] scales them.
fn laid_phase(design: &LoopDesign, w: f64, frac: f64, eps: f64) -> f64 {
    let total = design.delay.max(3) as f64;
    let mut p = -w * (total + 1.0) + first_phase(w, frac);
    for (r, t) in &design.poles[..design.sections] {
        let t2 = if eps == 0.0 {
            *t
        } else {
            (*t * (1.0 + eps)).clamp(1e-6, std::f64::consts::PI - 1e-6)
        };
        p += second_phase(w, *r, t2);
    }
    p
}

/// The `k`-th resonance (Hz) of a loop whose phase is `phase`: Newton on `phase(ω) = −2πk`
/// from `hz`, with the design's group delay for the slope.
fn resonance(design: &LoopDesign, phase: &dyn Fn(f64) -> f64, k: u32, hz: f64) -> f64 {
    let target = -TAU * k as f64;
    let mut w = TAU * hz / FS;
    for _ in 0..60 {
        let e = phase(w) - target;
        w = (w + e / design.group_delay(w)).clamp(1e-9, std::f64::consts::PI);
    }
    w * FS / TAU
}

/// Key `m`'s loop `index` struck alone at `vel` on a unit voiced `v`, measured against its
/// designed partials and predicted.
pub fn loop_tune(v: &Voicing, m: u8, index: usize, vel: f32) -> LoopTune {
    let i = (m - tables::LOWEST) as usize;
    let mut c = unit(v);
    let mut keep = [0.0; MAX_STRINGS];
    keep[index] = 1.0;
    c.set_loops_kept(m, keep);
    let (l, r) = perform(&mut c, &[(0, Ev::On(m, vel, 1))], FRAMES, 512, 1);
    let laid: LaidLoops = c.loops_laid(m);
    assert!(index < laid.loops, "key {m} has no loop {index}");
    let kb = keyboard_of(v);
    let design = kb.loops[i];
    let curve: Key = curve_of(v)[i];
    let ks = c.key_strings(i);
    let (detune, eps) = (laid.detune_ct[index], laid.eps[index]);
    // The same detune with no stiffness scatter: what the scatter alone moves.
    let plain = (design.frac + detune * design.frac_per_cent).clamp(-0.99, 0.99);
    let (mut near, mut designed_ct, mut predicted_ct) = (Vec::new(), Vec::new(), Vec::new());
    for n in 1..=PARTIALS {
        let hz = curve.partial(n);
        if hz > 0.45 * FS {
            break;
        }
        // The scatter's hold keeps the first partial where the detune put it (the shipped
        // `uni_b_hold`): by design it moves only the upper ones.
        let stiff = if n == 1 {
            0.0
        } else {
            let scattered = |w: f64| laid_phase(&design, w, laid.designed_frac[index], eps);
            let flat = |w: f64| laid_phase(&design, w, plain, 0.0);
            CENTS
                * crate::math::ln(
                    resonance(&design, &scattered, n, hz) / resonance(&design, &flat, n, hz),
                )
        };
        let want = detune + stiff;
        let hz_want = hz * crate::math::exp(want / CENTS);
        let now = |w: f64| laid_phase(&design, w, laid.frac[index], eps) + ks.solo_phase(index, w);
        let pred = resonance(&design, &now, n, hz);
        near.push(hz_want);
        designed_ct.push(want);
        predicted_ct.push(CENTS * crate::math::ln(pred / hz_want));
    }
    let f = |x: &[f32]| x.iter().map(|s| *s as f64).collect::<Vec<f64>>();
    let tune = measure(&f(&l), &f(&r), FS, curve.f0(), &near);
    let mut t = LoopTune {
        key: m,
        index,
        kind: if index < laid.vertical {
            Kind::String(index)
        } else {
            Kind::Polar
        },
        strings: laid.vertical,
        vel,
        detune_ct: detune,
        eps,
        table_ct: v.tune[i] as f64,
        designed_ct,
        predicted_ct,
        tune,
        predicted_pitch: 0.0,
    };
    t.predicted_pitch = t.weighed(&t.predicted_ct);
    t
}

/// How many loops key `m` lays on `v`: its strings, and the horizontal loop on a key of one
/// or two strings when model 2's polarisation is on.
pub fn loops_of(v: &Voicing, m: u8) -> usize {
    let n = tables::strings(m) as usize;
    n + usize::from(v.polar != 0.0 && n < 3)
}

/// Every loop of `keys` at every velocity of [`VELOCITIES`], on `threads` threads, sorted
/// by key, loop and velocity.
pub fn map_loops(v: &Voicing, keys: &[u8], threads: usize) -> Vec<LoopTune> {
    let _ = keyboard_of(v);
    let jobs: Vec<(u8, usize, f32)> = keys
        .iter()
        .flat_map(|m| {
            (0..loops_of(v, *m)).flat_map(move |i| VELOCITIES.iter().map(move |x| (*m, i, *x)))
        })
        .collect();
    let out = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for t in 0..threads {
            let (out, jobs) = (&out, &jobs);
            s.spawn(move || {
                for (m, i, x) in jobs.iter().skip(t).step_by(threads) {
                    let r = loop_tune(v, *m, *i, *x);
                    out.lock().unwrap().push(r);
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by(|a, b| {
        (a.key, a.index)
            .cmp(&(b.key, b.index))
            .then(a.vel.total_cmp(&b.vel))
    });
    o
}

/// The shipped voicing's every loop A0–C8 at every velocity, measured once for every test.
fn shipped() -> &'static [LoopTune] {
    static M: OnceLock<Vec<LoopTune>> = OnceLock::new();
    M.get_or_init(|| {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(16));
        let keys: Vec<u8> = (tables::LOWEST..=tables::HIGHEST).collect();
        map_loops(&Voicing::default(), &keys, threads)
    })
}

/// The felt at its default is no felt: a phrase over keys of one, two and three strings
/// (the horizontal loops among them), the pedals moving, renders to the same bits whether
/// the hook was never touched or every loop of every key was set kept, `[1; 3]` — a product
/// by 1 is exact in IEEE 754. (That the hook's code itself moved nothing against the code
/// before it is the model-2 hashes' to hold: `chorda_tests_voicing3.rs`'s three, unchanged.)
#[test]
fn every_loop_kept_changes_not_a_bit() {
    let mut ev = vec![
        (0, Ev::On(21, 0.95, 1)),
        (30, Ev::On(26, 0.5, 2)),
        (60, Ev::On(33, 0.72, 3)),
        (90, Ev::On(41, 0.4, 4)),
        (2_000, Ev::Pedal(0.6)),
        (3_000, Ev::On(42, 1.0, 5)),
        (3_004, Ev::On(60, 0.8, 6)),
        (3_008, Ev::On(88, 0.97, 7)),
        (9_000, Ev::Off(33, 3)),
        (11_000, Ev::On(30, 0.9, 8)),
        (12_000, Ev::Soft(1.0)),
        (12_500, Ev::On(31, 0.45, 9)),
        (15_000, Ev::Pedal(0.0)),
        (15_001, Ev::Off(21, 1)),
        (16_000, Ev::Soft(0.0)),
        (18_000, Ev::On(100, 0.85, 10)),
        (18_400, Ev::Off(100, 10)),
    ];
    ev.sort_by_key(|e| e.0);
    let v = Voicing::default();
    let (l0, r0) = perform(&mut unit(&v), &ev, 24_000, 256, 1);
    let mut kept = unit(&v);
    for m in tables::LOWEST..=tables::HIGHEST {
        kept.set_loops_kept(m, [1.0; MAX_STRINGS]);
    }
    let (l1, r1) = perform(&mut kept, &ev, 24_000, 256, 1);
    assert!(l0.iter().chain(&r0).any(|x| *x != 0.0));
    let bits = |x: &[f32]| x.iter().map(|s| s.to_bits()).collect::<Vec<u32>>();
    assert!(bits(&l0) == bits(&l1) && bits(&r0) == bits(&r1));
}

/// A muted loop holds nothing — its hammer never met it and it returned nothing — while
/// the loop left sounding sounds, and every loop stays laid on the bridge.
#[test]
fn a_muted_loop_holds_nothing_and_the_kept_one_sounds() {
    let v = Voicing::default();
    for (m, solo) in [(60u8, 1usize), (33, 0), (33, 2), (25, 1), (25, 0)] {
        let mut c = unit(&v);
        let mut keep = [0.0; MAX_STRINGS];
        keep[solo] = 1.0;
        c.set_loops_kept(m, keep);
        let _ = perform(&mut c, &[(0, Ev::On(m, 0.6, 1))], 4_800, 512, 1);
        let laid = c.loops_laid(m);
        assert_eq!(laid.loops, loops_of(&v, m), "key {m}");
        assert_eq!(laid.vertical, tables::strings(m) as usize, "key {m}");
        let ks = c.key_strings((m - tables::LOWEST) as usize);
        assert_eq!(ks.strings(), laid.loops, "key {m}");
        for i in 0..laid.loops {
            let p = ks.loop_peak(i);
            if i == solo {
                assert!(p > 0.0, "key {m}: loop {i} kept but silent");
            } else {
                assert_eq!(p, 0.0, "key {m}: loop {i} muted but holds {p:e}");
            }
        }
    }
}

/// Diagnostic: every loop A0–C8 alone at velocities 0.3, 0.6 and 0.9 — its designed offset,
/// the ear's pitch error and the first three partials' errors (cents), the prediction —
/// and, with `CHORDA_STRINGS_JSON` set, the whole table as JSON for the owner's page
/// (`tools/chorda-voicing/strings_page.py`). `CHORDA_TUNE_KEYS=lo-hi` picks the keys. Run
/// with `cargo test --release -p cantus-dsp --lib strings_map -- --ignored --nocapture`.
#[test]
#[ignore]
fn strings_map() {
    let (lo, hi) = std::env::var("CHORDA_TUNE_KEYS")
        .ok()
        .and_then(|s| {
            let (a, b) = s.split_once('-')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((tables::LOWEST, tables::HIGHEST));
    let keys: Vec<u8> = (lo..=hi).collect();
    let v = Voicing::default();
    let all = map_loops(&v, &keys, 16);
    eprintln!("key loop  vel  detune   eps     table | err   pred  | p1    p2    p3   (dB p1 p2 p3)");
    let p = |t: &LoopTune, n: u32| t.partial(n).map_or((f64::NAN, f64::NAN), |x| x);
    for t in &all {
        let name = match t.kind {
            Kind::String(i) => format!("s{}", i + 1),
            Kind::Polar => "pol".into(),
        };
        let (a, b, c) = (p(t, 1), p(t, 2), p(t, 3));
        eprintln!(
            "{:3} {name:>4} {:.1} {:+6.2} {:+8.5} {:+5.2} | {:+5.2} {:+5.2} | {:+5.2} {:+5.2} {:+5.2} ({:5.1} {:5.1} {:5.1})",
            t.key, t.vel, t.detune_ct, t.eps, t.table_ct, t.error(), t.predicted_pitch,
            a.0, b.0, c.0, a.1, b.1, c.1
        );
    }
    if let Ok(path) = std::env::var("CHORDA_STRINGS_JSON") {
        std::fs::write(&path, json(&all)).expect("the JSON written");
        eprintln!("wrote {path}");
    }
}

/// The table as JSON (no serde here: numbers and short names only).
fn json(all: &[LoopTune]) -> String {
    let num = |x: f64| {
        if x.is_finite() {
            format!("{:.3}", x)
        } else {
            "null".into()
        }
    };
    let mut s = String::from("[\n");
    for (j, t) in all.iter().enumerate() {
        let kind = match t.kind {
            Kind::String(i) => format!("\"s{}\"", i + 1),
            Kind::Polar => "\"polar\"".into(),
        };
        let parts: Vec<String> = t
            .tune
            .partials
            .iter()
            .map(|p| {
                format!(
                    "[{},{},{},{},{}]",
                    p.n,
                    num(p.cents),
                    num(p.level_db),
                    num(t.designed_ct[(p.n - 1) as usize]),
                    num(t.predicted_ct[(p.n - 1) as usize])
                )
            })
            .collect();
        s += &format!(
            "{{\"key\":{},\"loop\":{kind},\"strings\":{},\"vel\":{},\"detune\":{},\"eps\":{},\"table\":{},\"err\":{},\"pred\":{},\"curve\":{},\"partials\":[{}]}}{}\n",
            t.key,
            t.strings,
            t.vel,
            num(t.detune_ct),
            format!("{:.6}", t.eps),
            num(t.table_ct),
            num(t.error()),
            num(t.predicted_pitch),
            num(t.pitch_on_curve()),
            parts.join(","),
            if j + 1 < all.len() { "," } else { "" }
        );
    }
    s + "]\n"
}

/// The unison's sizes as they are laid (round 41: `tables::strings`' comment said three
/// strings from D2, the code has laid them from F#2 since round 28 — the code is what every
/// voicing, gate and recording was made on, so the comment was corrected, not the keys).
#[test]
fn one_string_to_f_sharp_1_two_to_f2_three_from_f_sharp_2() {
    for m in tables::LOWEST..=tables::HIGHEST {
        let want = match m {
            21..=30 => 1,
            31..=41 => 2,
            _ => 3,
        };
        assert_eq!(tables::strings(m), want, "key {m}");
    }
}

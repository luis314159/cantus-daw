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
use super::tests_tuning::{curve_of, keyboard_of, unit, unit_with};
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
    /// Of it, what the loop as laid gives on a rigid bridge (cents): the table and the
    /// bridge's tuning of the unison, no pull.
    pub rigid_ct: Vec<f64>,
    /// The measurement against the designed partials: each partial's `cents` is its error.
    pub tune: KeyTune,
    /// Whether the action's noises were off (see [`quiet`]).
    pub quiet: bool,
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
pub fn loop_tune(v: &Voicing, m: u8, index: usize, vel: f32, quiet: bool) -> LoopTune {
    let i = (m - tables::LOWEST) as usize;
    // Round 42 (candidate 17): the action's noises off in both runs — the knock now rings
    // through the case at a recorded grand's level, and read as a loop's phase it moved two
    // bass loops past their tolerance; a loop's pitch is the strings'. `quiet` stays as the
    // record of which run asked for it.
    let _ = quiet;
    let mut c = unit_with(v, Some(0.0));
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
    let (mut near, mut designed_ct) = (Vec::new(), Vec::new());
    let (mut predicted_ct, mut rigid_ct) = (Vec::new(), Vec::new());
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
        let rigid = |w: f64| laid_phase(&design, w, laid.frac[index], eps);
        let rig = resonance(&design, &rigid, n, hz);
        near.push(hz_want);
        designed_ct.push(want);
        predicted_ct.push(CENTS * crate::math::ln(pred / hz_want));
        rigid_ct.push(CENTS * crate::math::ln(rig / hz_want));
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
        rigid_ct,
        tune,
        quiet,
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
pub fn map_loops(v: &Voicing, keys: &[u8], threads: usize, quiet: bool) -> Vec<LoopTune> {
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
                    let r = loop_tune(v, *m, *i, *x, quiet);
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

fn all_loops(quiet: bool) -> Vec<LoopTune> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(16));
    let keys: Vec<u8> = (tables::LOWEST..=tables::HIGHEST).collect();
    map_loops(&Voicing::default(), &keys, threads, quiet)
}

/// The shipped voicing's every loop A0–C8 at every velocity, as the instrument plays it,
/// measured once for every test.
fn shipped() -> &'static [LoopTune] {
    static M: OnceLock<Vec<LoopTune>> = OnceLock::new();
    M.get_or_init(|| all_loops(false))
}

/// [`shipped`] with the action's noises off (`ChordaSettings::noise` 0: the key's thud, the
/// blow in the frame, the damper's landing), every string path as it ships. The thud is the
/// key's, the same whichever loop sounds, so a loop alone hears it louder against itself
/// than its key does: the horizontal loop, struck at 7 % of the blow, under a thud some
/// 20 dB stronger for it. Its 72 and 185 Hz modes, rung on by the board and the room, then
/// sit on the weak fundamentals of G#1–C2 (−17 to −24 dB under their strongest partial,
/// under the bass's 55 Hz high-pass) and pull their measure by up to +63 cents at velocity
/// 0.9 — the thud's, not the string's (round 41: off, the same fundamentals measure within
/// 1.9 cents). The partials are judged here; the ear's pitch in both.
fn quiet() -> &'static [LoopTune] {
    static M: OnceLock<Vec<LoopTune>> = OnceLock::new();
    M.get_or_init(|| all_loops(true))
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

/// The keys `CHORDA_TUNE_KEYS` picks (`lo-hi` or a comma list; every key if unset).
fn diag_keys() -> Vec<u8> {
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

/// Diagnostic: every loop A0–C8 alone at velocities 0.3, 0.6 and 0.9, as the instrument
/// plays it and with the action's noises off — its designed offset, the ear's pitch error,
/// the prediction and its tolerance, the fundamental's error in both — and, with
/// `CHORDA_STRINGS_JSON` set, the whole table and each key whole (`tests_tuning::key_tune`)
/// as JSON for the owner's page (`tools/chorda-voicing/strings_page.py`).
/// `CHORDA_TUNE_KEYS` picks the keys. Run with
/// `cargo test --release -p cantus-dsp --lib strings_map -- --ignored --nocapture`.
#[test]
#[ignore]
fn strings_map() {
    let keys = diag_keys();
    let v = Voicing::default();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(16));
    let loud = map_loops(&v, &keys, threads, false);
    let calm = map_loops(&v, &keys, threads, true);
    eprintln!(
        "key loop vel  detune  eps      table |  err  quiet  pred   tol | p1 err (quiet)  dB"
    );
    for (t, q) in loud.iter().zip(&calm) {
        let p1 = |t: &LoopTune| t.partial(1).map_or((f64::NAN, f64::NAN), |x| x);
        eprintln!(
            "{:3} {:>4} {:.1} {:+6.2} {:+8.5} {:+5.2} | {:+5.2} {:+5.2} {:+5.2} {:5.2} | {:+6.2} ({:+5.2}) {:5.1}",
            t.key,
            name(t.kind),
            t.vel,
            t.detune_ct,
            t.eps,
            t.table_ct,
            t.error(),
            q.error(),
            q.predicted_pitch,
            pitch_tolerance(q),
            p1(t).0,
            p1(q).0,
            p1(q).1
        );
    }
    if let Ok(path) = std::env::var("CHORDA_STRINGS_JSON") {
        let whole: Vec<(u8, f32, KeyTune)> = keys
            .iter()
            .flat_map(|m| VELOCITIES.iter().map(move |x| (*m, *x)))
            .collect::<Vec<_>>()
            .chunks(threads)
            .flat_map(|c| {
                std::thread::scope(|s| {
                    let hs: Vec<_> = c
                        .iter()
                        .map(|(m, x)| {
                            let v = &v;
                            s.spawn(move || (*m, *x, super::tests_tuning::key_tune(v, *m, *x)))
                        })
                        .collect();
                    hs.into_iter()
                        .map(|h| h.join().unwrap())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        std::fs::write(&path, json(&loud, &calm, &whole)).expect("the JSON written");
        eprintln!("wrote {path}");
    }
}

/// Diagnostic: each partial of the loops of the keys `CHORDA_TUNE_KEYS`, alone at velocity
/// 0.6 with the action's noises off — its designed offset from the curve, what the loop
/// as laid gives on a rigid bridge, the prediction with the bridge's pull, and the
/// measurement (all but the first against the designed partial, cents), and its level.
#[test]
#[ignore]
fn strings_partials() {
    let v = Voicing::default();
    for m in diag_keys() {
        for i in 0..loops_of(&v, m) {
            let t = loop_tune(&v, m, i, 0.6, true);
            eprintln!(
                "key {m} {} detune {:+.2} eps {:+.5} table {:+.2}: error {:+.2} predicted {:+.2}",
                name(t.kind),
                t.detune_ct,
                t.eps,
                t.table_ct,
                t.error(),
                t.predicted_pitch
            );
            for p in &t.tune.partials {
                let k = (p.n - 1) as usize;
                eprintln!(
                    "   p{:2} {:7.1} Hz designed {:+6.2} rigid {:+6.2} predicted {:+6.2} measured {:+6.2} {:6.1} dB",
                    p.n, p.near_hz, t.designed_ct[k], t.rigid_ct[k], t.predicted_ct[k], p.cents, p.level_db
                );
            }
        }
    }
}

fn name(k: Kind) -> String {
    match k {
        Kind::String(i) => format!("s{}", i + 1),
        Kind::Polar => "polar".into(),
    }
}

/// The table as JSON (no serde here: numbers and short names only): each loop and velocity
/// as played (`err`, `p1`) and with the action's noises off (`quiet`, `p1q`, the partials),
/// and each key whole.
fn json(loud: &[LoopTune], calm: &[LoopTune], whole: &[(u8, f32, KeyTune)]) -> String {
    let num = |x: f64| {
        if x.is_finite() {
            format!("{:.3}", x)
        } else {
            "null".into()
        }
    };
    let p1 = |t: &LoopTune| num(t.partial(1).map_or(f64::NAN, |x| x.0));
    let mut s = String::from("{\"loops\":[\n");
    for (j, (t, q)) in loud.iter().zip(calm).enumerate() {
        let parts: Vec<String> = q
            .tune
            .partials
            .iter()
            .map(|p| {
                let k = (p.n - 1) as usize;
                format!(
                    "[{},{},{},{},{},{}]",
                    p.n,
                    num(p.near_hz),
                    num(p.cents),
                    num(p.level_db),
                    num(q.designed_ct[k]),
                    num(q.predicted_ct[k])
                )
            })
            .collect();
        s += &format!(
            "{{\"key\":{},\"loop\":\"{}\",\"strings\":{},\"vel\":{},\"detune\":{},\"eps\":{:.6},\"table\":{},\"err\":{},\"quiet\":{},\"pred\":{},\"tol\":{},\"p1\":{},\"p1q\":{},\"curve\":{},\"partials\":[{}]}}{}\n",
            t.key,
            name(t.kind),
            t.strings,
            t.vel,
            num(t.detune_ct),
            t.eps,
            num(t.table_ct),
            num(t.error()),
            num(q.error()),
            num(q.predicted_pitch),
            num(pitch_tolerance(q)),
            p1(t),
            p1(q),
            num(t.pitch_on_curve()),
            parts.join(","),
            if j + 1 < loud.len() { "," } else { "" }
        );
    }
    s += "],\"keys\":[\n";
    for (j, (m, x, k)) in whole.iter().enumerate() {
        let p1 = k
            .partials
            .iter()
            .find(|p| p.n == 1)
            .map_or(f64::NAN, |p| p.cents);
        s += &format!(
            "{{\"key\":{m},\"vel\":{x},\"pitch\":{},\"p1\":{},\"beat\":{}}}{}\n",
            num(k.pitch),
            num(p1),
            num(k.beat_worst),
            if j + 1 < whole.len() { "," } else { "" }
        );
    }
    s + "]}\n"
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

/// The last key with a damper (E6), as the whole-key gate has it: above, a key's own
/// sympathetic string rings with it undamped and the board's high band takes a larger share.
const TOP_DAMPED: u8 = 88;

/// The whole key's gate (`chorda_tests_tune_gate.rs`'s `pitch_tolerance`, the same reasons):
/// a cent to E6 — what a technician holds a unison and its octaves to by counting beats —
/// and 2.5 above, where a cent is 2–4 Hz and the top is set by ear.
fn key_gate(m: u8) -> f64 {
    if m <= TOP_DAMPED {
        1.0
    } else {
        2.5
    }
}

/// The most (cents) the technician's pass lets a bridge resonance pull one of a key's first
/// three partials (the shipped `bridge.pull`).
fn bridge_cap() -> f64 {
    Voicing::default().bridge_pull
}

/// What the design itself puts between a loop alone and its key (cents, the ear's), and so
/// may stand between the loop alone and its designed pitch with nothing wrong:
/// - **the technician's table** (`voicing::tune::TUNE`): measured on each key whole and laid
///   on every one of its loops alike, it moves each loop by the shift the unison sounding
///   together has and a loop alone does not (the polarisation loop beside its string
///   carries it as well);
/// - **the bridge's tuning** (`KeyStrings::set_bridge_tuned`): every string of a unison is
///   moved against the pull the bridge's resonances give the unison moving together; a
///   string alone meets `1/n` of that pull ([`super::string::KeyStrings::solo_phase`]), so
///   `1 − 1/n` of the correction is left on it — at most that share of the cap, the pull
///   being held under it on the first three partials, the ones the ear weighs most.
///
/// A single string's key has neither: its table is its own string's, its bridge tuned for
/// it alone. It has no unison to hide in and is held to the whole key's gate itself.
fn alone_allowance(t: &LoopTune) -> f64 {
    match t.kind {
        Kind::String(_) if t.strings == 1 => 0.0,
        Kind::String(_) => t.table_ct.abs() + (1.0 - 1.0 / t.strings as f64) * bridge_cap(),
        Kind::Polar => t.table_ct.abs(),
    }
}

/// A loop alone's tolerance on the ear's pitch against its designed pitch: the whole key's
/// gate and [`alone_allowance`].
fn pitch_tolerance(t: &LoopTune) -> f64 {
    key_gate(t.key) + alone_allowance(t)
}

/// The partials a loop is judged on: the first three below 5 kHz (the loop design's own
/// checked band, `design::CHECK_HZ` — above it the design promises nothing and the whole
/// key's gate skips them too) within 30 dB of the loop's strongest (under, the measure is
/// the board's and the room's as much as the string's).
fn judged(t: &LoopTune) -> impl Iterator<Item = (u32, f64)> + '_ {
    (1..=3u32).filter_map(move |n| {
        let p = t.tune.partials.iter().find(|p| p.n == n)?;
        let (cents, below) = t.partial(n)?;
        (p.near_hz < super::design::CHECK_HZ && below > -30.0).then_some((n, cents))
    })
}

/// Every vertical string of a one-string key (A0–F#1), alone — its horizontal loop muted —
/// is held to the whole key's gate: its pitch within a cent of the curve as the ear weighs
/// it, as played and with the action's noises off, at every velocity; its fundamental
/// within 3 cents (the whole key's limit for it: the bridge's pull left on the
/// fundamental and the stiffness scatter's remainder).
#[test]
fn every_single_string_alone_is_held_to_the_whole_keys_gate() {
    let mut worst = (0.0f64, 0u8, 0.0f32);
    for t in shipped().iter().chain(quiet()) {
        if t.strings != 1 || t.kind != Kind::String(0) {
            continue;
        }
        let tol = key_gate(t.key);
        assert!(
            t.error().abs() <= tol,
            "key {} alone at {}: {:+.2} cents from its curve (tolerance {tol})",
            t.key,
            t.vel,
            t.error()
        );
        if t.error().abs() > worst.0 {
            worst = (t.error().abs(), t.key, t.vel);
        }
    }
    for t in quiet()
        .iter()
        .filter(|t| t.strings == 1 && t.kind == Kind::String(0))
    {
        let (p1, _) = t
            .partial(1)
            .expect("every string's fundamental is measured");
        assert!(
            p1.abs() <= 3.0,
            "key {} at {}: fundamental {p1:+.2} cents",
            t.key,
            t.vel
        );
    }
    eprintln!(
        "worst single string: key {} at {} {:.2} cents",
        worst.1, worst.2, worst.0
    );
}

/// Every string of every unison, and every horizontal loop, alone sounds its designed
/// pitch — the curve and its designed detune — within the whole key's gate and what the
/// design puts between a loop alone and its key ([`alone_allowance`]), as played and with
/// the action's noises off, at every velocity.
#[test]
fn every_loop_alone_sounds_its_designed_pitch() {
    let mut worst = (0.0f64, 0u8, String::new(), 0.0f32);
    for t in shipped().iter().chain(quiet()) {
        let tol = pitch_tolerance(t);
        assert!(
            t.error().abs() <= tol,
            "key {} {} alone at {} (quiet {}): {:+.2} cents from its designed pitch (tolerance {tol:.2})",
            t.key,
            name(t.kind),
            t.vel,
            t.quiet,
            t.error()
        );
        if t.error().abs() > worst.0 {
            worst = (t.error().abs(), t.key, name(t.kind), t.vel);
        }
    }
    eprintln!(
        "worst loop alone: key {} {} at {} {:.2} cents",
        worst.1, worst.2, worst.3, worst.0
    );
}

/// Each loop alone is where its laid loop and its bridge put it: the measured pitch within
/// half a cent of the prediction to E6 (what no formula here models — the hammer's contact,
/// the phantom partials, the board's and the room's colouring of a decaying tone, the kit's
/// own error — is a few tenths; half a cent is a beat every 4 s at C4's fundamental), and
/// within the whole key's 2.5 cents above, where the board's high band's tail is a larger
/// share of the measure. With the action's noises off (the thud is not the string's;
/// [`quiet`]).
#[test]
fn every_loop_alone_is_where_its_loop_and_its_bridge_put_it() {
    let mut worst = (0.0f64, 0u8, String::new());
    for t in quiet() {
        let tol = if t.key <= TOP_DAMPED {
            0.5
        } else {
            key_gate(t.key)
        };
        let off = t.error() - t.predicted_pitch;
        assert!(
            off.abs() <= tol,
            "key {} {} at {}: measured {:+.2}, predicted {:+.2} cents",
            t.key,
            name(t.kind),
            t.vel,
            t.error(),
            t.predicted_pitch
        );
        if t.key <= TOP_DAMPED && off.abs() > worst.0 {
            worst = (off.abs(), t.key, name(t.kind));
        }
    }
    eprintln!(
        "worst off its prediction to E6: key {} {} {:.2} cents",
        worst.1, worst.2, worst.0
    );
}

/// Each loop's first partials alone lie on its design: each judged partial ([`judged`])
/// within 3 cents — the whole key's limit for its fundamental — and what the design puts
/// between a loop alone and its key. With the action's noises off ([`quiet`]: as played,
/// the thud pulls the weak fundamentals of G#1–C2 by up to 63 cents).
#[test]
fn every_loops_first_partials_lie_on_its_design() {
    let mut worst = (0.0f64, 0u8, String::new(), 0u32);
    for t in quiet() {
        let tol = 3.0 + alone_allowance(t);
        for (n, c) in judged(t) {
            assert!(
                c.abs() <= tol,
                "key {} {} at {}: partial {n} {c:+.2} cents (tolerance {tol:.2})",
                t.key,
                name(t.kind),
                t.vel
            );
            if c.abs() > worst.0 {
                worst = (c.abs(), t.key, name(t.kind), n);
            }
        }
    }
    eprintln!(
        "worst first partial: key {} {} partial {} {:.2} cents",
        worst.1, worst.2, worst.3, worst.0
    );
}

/// The offsets inside a unison are as designed: a unison's strings share their bridge,
/// their table and the bridge's correction, so each partial of one string alone stands from
/// the same partial of another by their designed difference — the fundamentals within a
/// cent (what a technician holds a unison to by its beats), the second and third partials
/// within 1.5 (a string's detune and the bridge's correction are laid on its fraction,
/// whose delay is not flat: on a fraction far from zero they move the upper partials up to a
/// cent more or less than the fundamental — round 41 measured 1.1 at worst). With the
/// action's noises off.
#[test]
fn the_offsets_inside_each_unison_are_as_designed() {
    let q = quiet();
    let mut worst = [(0.0f64, 0u8); 3];
    for a in q
        .iter()
        .filter(|t| t.kind == Kind::String(0) && t.strings > 1)
    {
        for b in q.iter().filter(|b| {
            b.key == a.key && b.vel == a.vel && matches!(b.kind, Kind::String(i) if i > 0)
        }) {
            let bj: Vec<(u32, f64)> = judged(b).collect();
            for (n, ca) in judged(a) {
                let Some((_, cb)) = bj.iter().find(|x| x.0 == n) else {
                    continue;
                };
                let d = ca - cb;
                let tol = if n == 1 { 1.0 } else { 1.5 };
                assert!(
                    d.abs() <= tol,
                    "key {} at {}: s1 and {} partial {n} {d:+.2} cents off their designed offset",
                    a.key,
                    a.vel,
                    name(b.kind)
                );
                let w = &mut worst[(n - 1) as usize];
                if d.abs() > w.0 {
                    *w = (d.abs(), a.key);
                }
            }
        }
    }
    eprintln!("worst unison offsets off design (cents, key), partials 1-3: {worst:?}");
}

/// The horizontal loop stands from its own vertical string by its designed offset
/// (`polar_ct`, and the string's detune): the ear's pitch of each alone within the whole
/// key's gate of their designed difference and what the bridge leaves on the string alone
/// and not on the loop, whose side of the bridge is a resistance (`1 − 1/n` of the cap).
/// As played and with the action's noises off.
#[test]
fn the_horizontal_loop_stands_its_designed_offset_from_its_string() {
    let mut n = 0;
    let mut worst = (0.0f64, 0u8);
    for set in [shipped(), quiet()] {
        for p in set.iter().filter(|t| t.kind == Kind::Polar) {
            let s = set
                .iter()
                .find(|t| t.key == p.key && t.vel == p.vel && t.kind == Kind::String(0))
                .expect("the loop's string is measured");
            let d = p.error() - s.error();
            let tol = key_gate(p.key) + (1.0 - 1.0 / p.strings as f64) * bridge_cap();
            assert!(
                d.abs() <= tol,
                "key {} at {}: the horizontal loop {d:+.2} cents off its designed offset from its string (tolerance {tol:.2})",
                p.key,
                p.vel
            );
            if d.abs() > worst.0 {
                worst = (d.abs(), p.key);
            }
            n += 1;
        }
    }
    // Every one- and two-string key, three velocities, both ways.
    assert_eq!(n, 2 * 3 * (41 - 21 + 1));
    eprintln!(
        "worst horizontal loop against its string: key {} {:.2} cents",
        worst.1, worst.0
    );
}

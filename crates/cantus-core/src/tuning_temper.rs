//! The named temperaments' tables (round 42, ADR 0155): twelve pitches in cents above C,
//! each built from its **construction rule** — which fifths of the circle are narrowed (or
//! widened) by which fraction of a comma — or, for the just tunings, from its ratios. The
//! rules and the archive files they agree with (to 0.0000 c) are in
//! `research/instruments/microtuning-verified.md` §5; the tests hold every row against that
//! note's published table (three decimals) and the rows' own intervals.
//!
//! A row's "C" is its best key: [`cents_in`] rotates it so that key sits on any of the
//! twelve, and `Tuning::offsets` anchors A to the diapason (as Pianoteq and Logic do).

use crate::tuning::Temperament;
use cantus_dsp::math;

/// The pure fifth, `1200·log2(3/2)`.
pub fn fifth() -> f64 {
    ratio_cents(3, 2)
}

/// The Pythagorean comma, `1200·log2(3^12 / 2^19)` ≈ 23.460 c.
pub fn pythagorean_comma() -> f64 {
    12.0 * fifth() - 7.0 * 1200.0
}

/// The syntonic comma, `1200·log2(81/80)` ≈ 21.506 c.
pub fn syntonic_comma() -> f64 {
    ratio_cents(81, 80)
}

/// The schisma: the Pythagorean comma less the syntonic ≈ 1.954 c.
pub fn schisma() -> f64 {
    pythagorean_comma() - syntonic_comma()
}

/// `1200·log2(n/d)`, deterministically.
pub fn ratio_cents(n: u64, d: u64) -> f64 {
    1200.0 * math::log2(n as f64 / d as f64)
}

/// The circle of fifths from C, as pitch classes: C G D A E B F♯ C♯ G♯ D♯ A♯ F.
const CIRCLE: [usize; 12] = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];

/// Twelve pitches from the twelve fifths of the circle starting at C, each `fifth −
/// temper[i]` (a positive value narrows it). The tempering sums to the Pythagorean comma for
/// the circle to close on C.
fn from_fifths(temper: [f64; 12]) -> [f64; 12] {
    let p = fifth();
    let mut out = [0.0; 12];
    let mut at = 0.0f64;
    for (i, &t) in temper.iter().enumerate().take(11) {
        at += p - t;
        at = at.rem_euclid(1200.0);
        out[CIRCLE[i + 1]] = at;
    }
    out
}

/// Twelve pitches from a chain of equal fifths `f` from E♭ to G♯ (the wolf between G♯ and
/// E♭) — the Pythagorean and meantone tunings.
fn chain(f: f64) -> [f64; 12] {
    let mut out = [0.0; 12];
    for n in -3i32..=8 {
        let pc = (n * 7).rem_euclid(12) as usize;
        out[pc] = (f64::from(n) * f).rem_euclid(1200.0);
    }
    out
}

/// Twelve pitches from ratios above C (index 0 is 1/1).
fn ratios(r: [(u64, u64); 12]) -> [f64; 12] {
    let mut out = [0.0; 12];
    for (o, (n, d)) in out.iter_mut().zip(r) {
        *o = ratio_cents(n, d);
    }
    out
}

/// The temperament's twelve pitches in cents above C (C = 0), with C its best key.
pub fn cents(id: Temperament) -> [f64; 12] {
    let pc = pythagorean_comma();
    let sc = syntonic_comma();
    let sch = schisma();
    // The fifths of the circle, in order: C–G, G–D, D–A, A–E, E–B, B–F♯, F♯–C♯, C♯–G♯,
    // G♯–D♯, D♯–A♯, A♯–F, F–C.
    match id {
        Temperament::Pythagorean => chain(fifth()),
        Temperament::MeantoneQuarter => chain(fifth() - sc / 4.0),
        Temperament::MeantoneSixth => chain(fifth() - sc / 6.0),
        // ¼ PC on C–G, G–D, D–A and B–F♯.
        Temperament::WerckmeisterIii => {
            let q = pc / 4.0;
            from_fifths([q, q, q, 0.0, 0.0, q, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        }
        // ⅓ PC narrow on C–G, D–A, E–B, F♯–C♯, A♯–F; ⅓ PC wide on G♯–D♯ and D♯–A♯.
        Temperament::WerckmeisterIv => {
            let t = pc / 3.0;
            from_fifths([t, 0.0, t, 0.0, t, 0.0, t, 0.0, -t, -t, t, 0.0])
        }
        // ½ SC on D–A and A–E, the schisma on F♯–C♯ (the standard Db/Ab spelling).
        Temperament::KirnbergerIi => {
            let h = sc / 2.0;
            from_fifths([0.0, 0.0, h, h, 0.0, 0.0, sch, 0.0, 0.0, 0.0, 0.0, 0.0])
        }
        // ¼ SC on C–G, G–D, D–A, A–E, the schisma on F♯–C♯ (the letter to Forkel, 1779).
        Temperament::KirnbergerIii => {
            let q = sc / 4.0;
            from_fifths([q, q, q, q, 0.0, 0.0, sch, 0.0, 0.0, 0.0, 0.0, 0.0])
        }
        // ⅙ PC on F–C–G–D–A–E–B (the modern Vallotti).
        Temperament::Vallotti => {
            let s = pc / 6.0;
            from_fifths([s, s, s, s, s, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, s])
        }
        // ⅙ PC on C–G–D–A–E–B–F♯ (Young's second, "Young II").
        Temperament::Young1799 => {
            let s = pc / 6.0;
            from_fifths([s, s, s, s, s, s, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        }
        // ⅕ PC on C–G–D–A–E and B–F♯.
        Temperament::Kellner => {
            let f = pc / 5.0;
            from_fifths([f, f, f, f, 0.0, f, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        }
        // Neidhardt III, "große Stadt" (1724), in twelfths of the PC.
        Temperament::Neidhardt => {
            let u = pc / 12.0;
            from_fifths([
                2.0 * u,
                2.0 * u,
                2.0 * u,
                u,
                0.0,
                u,
                u,
                u,
                0.0,
                u,
                u,
                0.0,
            ])
        }
        // Lehman (2005): ⅙ SC on F–C–G–D–A–E, E–B–F♯–C♯ pure, 1/12 SC on C♯–G♯ and
        // D♯–A♯, the schisma on G♯–D♯, A♯–F pure.
        Temperament::BachLehman => {
            let s = sc / 6.0;
            let t = sc / 12.0;
            from_fifths([s, s, s, s, 0.0, 0.0, 0.0, t, sch, t, 0.0, s])
        }
        // 5-limit just major (Malcolm 1721: B♭ = 16/9).
        Temperament::JustMajor => ratios([
            (1, 1),
            (16, 15),
            (9, 8),
            (6, 5),
            (5, 4),
            (4, 3),
            (45, 32),
            (3, 2),
            (8, 5),
            (5, 3),
            (16, 9),
            (15, 8),
        ]),
        // 5-limit just, minor as well (Ellis's duodene: B♭ = 9/5).
        Temperament::JustMinor => ratios([
            (1, 1),
            (16, 15),
            (9, 8),
            (6, 5),
            (5, 4),
            (4, 3),
            (45, 32),
            (3, 2),
            (8, 5),
            (5, 3),
            (9, 5),
            (15, 8),
        ]),
        // 7-limit (C7 = 4:5:6:7 pure): 7/6, 7/5, 12/7, 7/4.
        Temperament::Barbershop7 => ratios([
            (1, 1),
            (16, 15),
            (9, 8),
            (7, 6),
            (5, 4),
            (4, 3),
            (7, 5),
            (3, 2),
            (8, 5),
            (12, 7),
            (7, 4),
            (15, 8),
        ]),
    }
}

/// Each pitch class's deviation from 12-TET in cents, with the temperament's best key on
/// `key` (0 = C … 11 = B): `D_k(p) = D[(p − k) mod 12]` — every interval kept.
pub fn deviation_in(id: Temperament, key: u8) -> [f64; 12] {
    let c = cents(id);
    let k = usize::from(key % 12);
    let mut out = [0.0; 12];
    for (p, o) in out.iter_mut().enumerate() {
        let i = (p + 12 - k) % 12;
        *o = c[i] - 100.0 * i as f64;
    }
    out
}

/// The twelve pitches in cents above C, the temperament rotated to `key`.
pub fn cents_in(id: Temperament, key: u8) -> [f64; 12] {
    let d = deviation_in(id, key);
    let mut out = [0.0; 12];
    for (p, o) in out.iter_mut().enumerate() {
        *o = 100.0 * p as f64 + d[p] - d[0];
    }
    out
}

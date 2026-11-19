//! The tuning gate (`DSP_VERSION` 10, ADR 0098's fourth amendment,
//! `docs/round40/chorda-tune.md`): every key A0–C8 of the shipped voicing, struck alone at
//! velocity 0.6 and measured as a tuner hears it (`tunekit`: each partial's power-weighted
//! frequency over 0.1–1.1 s, the key's pitch the partials weighed by their level and the
//! ear's dominance region, each partial's beat the 3–97 % swing of its level about its decay
//! over 0.15–3.3 s), against the smooth curve its keyboard is designed to (the stretch the
//! design draws, without the technician's table).
//!
//! The owner, 2026-11-18: "En Chorda hay notas desafinadas, detecté Bb2, B2, C3 por lo
//! menos" — candidate 16 had Bb2 13 cents flat, B0 18 sharp, ten keys 4–18 cents off, and
//! nine unisons beating 20–29 dB deep. Each tolerance below says why it is what it is.

use super::tests_tuning::map;
use super::tunekit::KeyTune;
use super::voicing::Voicing;
use std::sync::OnceLock;

/// The shipped voicing's 88 keys at velocity 0.6, measured once for every test here.
fn shipped() -> &'static [(u8, KeyTune)] {
    static M: OnceLock<Vec<(u8, KeyTune)>> = OnceLock::new();
    M.get_or_init(|| {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(16));
        let keys: Vec<u8> = (21u8..=108).collect();
        map(&Voicing::default(), &keys, 0.6, threads)
    })
}

/// A partial's offset from the curve (cents) and level (dB), if it was measured.
fn partial(k: &KeyTune, n: u32) -> Option<(f64, f64)> {
    k.partials
        .iter()
        .find(|p| p.n == n)
        .map(|p| (p.cents, p.level_db))
}

/// The last key with a damper (E6): above it a key's own sympathetic string rings with it
/// undamped and the board's high band's tail takes a larger share of what a partial's
/// pitch is measured from.
const TOP_DAMPED: u8 = 88;

/// The pitch tolerance (cents) of key `m` as the ear weighs its partials: a cent to E6 —
/// what a technician holds a unison and its octaves to by counting beats (a cent at C4 is a
/// beat every 6 s against a pure octave) — and 2.5 cents above, where a cent is 2–4 Hz, the
/// ear's pitch discrimination is 5–9 cents (0.3–0.5 %) and a tuner sets the top by ear.
fn pitch_tolerance(m: u8) -> f64 {
    if m <= TOP_DAMPED {
        1.0
    } else {
        2.5
    }
}

#[test]
fn every_key_sounds_on_the_curve() {
    let mut worst: (f64, u8) = (0.0, 0);
    for (m, k) in shipped() {
        let tol = pitch_tolerance(*m);
        assert!(
            k.pitch.abs() <= tol,
            "key {m}: {:+.2} cents from the curve (tolerance {tol})",
            k.pitch
        );
        // The fundamental alone may sit up to 3 cents off the ladder the ear hears: the
        // in-phase mode's pull the bridge's resonances are allowed (2 cents, by construction)
        // and what the strings' stiffness scatter leaves (a cent).
        let (p1, _) = partial(k, 1).expect("every key's fundamental is measured");
        assert!(p1.abs() <= 3.0, "key {m}: fundamental {p1:+.2} cents");
        if k.pitch.abs() > worst.0 {
            worst = (k.pitch.abs(), *m);
        }
    }
    eprintln!("worst key {} at {:.2} cents", worst.1, worst.0);
}

#[test]
fn neighbouring_keys_lie_on_one_smooth_curve() {
    let m = shipped();
    for w in m.windows(3) {
        let (k, a, b, c) = (w[1].0, w[0].1.pitch, w[1].1.pitch, w[2].1.pitch);
        // A key against the mean of its neighbours: the curve's own bend over two
        // semitones is under a hundredth of a cent, so this is the key's raggedness.
        let rag = b - 0.5 * (a + c);
        let tol = pitch_tolerance(k).max(pitch_tolerance(w[2].0));
        assert!(
            rag.abs() <= tol,
            "key {k}: {rag:+.2} cents off its neighbours' mean"
        );
    }
}

/// The interval error (cents) between key `lo`'s partial `p` and key `hi`'s partial `q`
/// against the curve's own (the coincident partials of a tuned interval), and the beat it
/// adds (Hz), when both partials sound within 30 dB of their key's strongest.
fn interval(lo: &KeyTune, p: u32, hi: &KeyTune, q: u32) -> Option<(f64, f64)> {
    let ok = |k: &KeyTune, n: u32| {
        let top = k
            .partials
            .iter()
            .map(|x| x.level_db)
            .fold(f64::MIN, f64::max);
        k.partials
            .iter()
            .find(|x| x.n == n && x.level_db > top - 30.0)
            .map(|x| (x.cents, x.near_hz))
    };
    let ((a, hz), (b, _)) = (ok(lo, p)?, ok(hi, q)?);
    Some((a - b, hz * (a - b) / 1_731.234))
}

/// Whether an interval's coincident partials beat as the curve's own do: within a third of a
/// beat a second (slower than one beat in a held note's three seconds — what a technician
/// leaves when counting an octave or a fifth; a cent at C4's second partial is 0.3 Hz), or,
/// where cents are many hertz, within 3 cents to E6 and 3.5 above for the octave's 2:1 and
/// the fifth's 3:2. The octave's 4:2 (its upper partial weaker, where the strings'
/// stiffness scatter lives) may be 4.5 cents or half a beat a second off: candidate 16 had
/// its 2:1s and 3:2s at 2.6 cents rms, 9.8 at worst; the Salamander Grand's own (a sampled
/// set, three keys a sample) are 5.8 rms, 34 at worst.
fn interval_ok(cents: f64, hz: f64, hi: u8, p: u32) -> bool {
    let (tol, slow) = if p == 4 {
        (4.5, 0.5)
    } else if hi <= TOP_DAMPED {
        (3.0, 0.33)
    } else {
        (3.5, 0.33)
    };
    hz.abs() <= slow || cents.abs() <= tol
}

#[test]
fn octaves_and_fifths_beat_as_the_curve_draws_them() {
    let m = shipped();
    let key = |n: u8| &m[(n - 21) as usize].1;
    let mut all = Vec::new();
    for lo in 21u8..=101 {
        // The octave's 2:1 and 4:2, the fifth's 3:2.
        for (hi, p, q) in [(lo + 12, 2u32, 1u32), (lo + 12, 4, 2), (lo + 7, 3, 2)] {
            if hi > 108 {
                continue;
            }
            // Partials up to 5 kHz: above, few sound 30 dB from their key's strongest and
            // their measure is a hiss's.
            if key(lo)
                .partials
                .iter()
                .any(|x| x.n == p && x.near_hz > 5_000.0)
            {
                continue;
            }
            if let Some((c, hz)) = interval(key(lo), p, key(hi), q) {
                all.push((lo, hi, p, q, c, hz));
            }
        }
    }
    let bad: Vec<_> = all
        .iter()
        .filter(|(_, hi, p, _, c, hz)| !interval_ok(*c, *hz, *hi, *p))
        .collect();
    let worst = all
        .iter()
        .max_by(|a, b| a.5.abs().total_cmp(&b.5.abs()))
        .expect("intervals measured");
    eprintln!(
        "{} intervals; the fastest beat off the curve {}–{} {}:{} {:+.2} cents ({:+.2} Hz)",
        all.len(),
        worst.0,
        worst.1,
        worst.2,
        worst.3,
        worst.4,
        worst.5
    );
    for (lo, hi, p, q, c, hz) in &bad {
        eprintln!("  {lo}–{hi} {p}:{q} {c:+.2} cents ({hz:+.2} Hz)");
    }
    assert!(
        bad.is_empty(),
        "{} octaves or fifths beat off the curve",
        bad.len()
    );
}

/// The deepest beat (dB) of a key's three strongest partials a register allows: a recorded
/// grand's own (the Salamander Grand's keys at mezzo-forte, measured with the same kit —
/// `tuning_wav`: A0–A#1 at most 7.5, B1–A#2 16.3, the middle 12–18 but for one sample set
/// at 27.8, the treble 16–19, the top octave 20.3) with a little room. Candidate 16 had
/// nine keys at 20–29.
fn beat_ceiling(m: u8) -> f64 {
    match m {
        21..=34 => 10.0,
        35..=46 => 16.0,
        47..=95 => 20.0,
        _ => 22.0,
    }
}

#[test]
fn no_unison_beats_deeper_than_a_recorded_grands() {
    for (m, k) in shipped() {
        let c = beat_ceiling(*m);
        assert!(
            k.beat_worst <= c,
            "key {m}: its unison beats {:.1} dB deep (ceiling {c})",
            k.beat_worst
        );
    }
}

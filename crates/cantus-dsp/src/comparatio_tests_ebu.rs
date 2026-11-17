//! The published conformance cases, through what Comparatio's panel reads
//! ([`super::measure::Analyser`]): EBU Tech 3341 (loudness metering, v4 2016) cases 1–5, 9,
//! 12 and the true-peak cases 15–19; EBU Tech 3342 (loudness range, v4 2016) cases 1–4;
//! and ITU-R BS.1770-4 Annex 2's own example 4× interpolator, run beside ours on the same
//! signals as a second, independent reading.
//!
//! The signals are generated from the documents' definitions (a 1 kHz sine at 48 kHz in
//! both channels, at the stated dBFS for the stated seconds; the true-peak sines at a
//! fraction of the rate with a phase offset). Not run here, because they are recordings
//! the EBU distributes rather than definitions: 3341 cases 6 (5.0 channels — Comparatio
//! meters stereo), 7, 8, 10, 11, 13, 14 and 20–23, and 3342 cases 5 and 6.

use super::measure::{Analyser, Readings};
use super::testkit::*;
use crate::math;

/// Run a stereo signal (the same in both channels) through an analyser, a reading after
/// every 100 ms step.
fn steps(x: &[f32]) -> (Analyser, Vec<Readings>) {
    let mut a = Analyser::new(FS);
    let mut out = Vec::new();
    let mut i = 0;
    while i < x.len() {
        let m = a.to_step().min(x.len() - i);
        a.process(&x[i..i + m], &x[i..i + m]);
        if a.to_step() == 4_800 {
            out.push(a.readings());
        }
        i += m;
    }
    (a, out)
}

fn close(got: f64, want: f64, tol: f64, what: &str) {
    eprintln!("{what}: {got:.3} (want {want} ±{tol})");
    assert!((got - want).abs() <= tol, "{what}: {got} for {want}");
}

/// 3341 cases 1 and 2: −23 and −33 dBFS read −23.0 and −33.0 LUFS, momentary, short-term
/// and integrated, ±0.1.
#[test]
fn tech_3341_cases_1_and_2() {
    for db in [-23.0, -33.0] {
        let (a, _) = steps(&segments(&[(db, 20.0)]));
        let r = a.readings();
        close(r.integrated.unwrap(), db, 0.1, "3341 I");
        close(r.momentary, db, 0.1, "3341 M");
        close(r.short_term, db, 0.1, "3341 S");
    }
}

/// Cases 3, 4 and 5: the gates and the energy mean.
#[test]
fn tech_3341_cases_3_4_5() {
    let cases: [(&[(f64, f64)], &str); 3] = [
        (
            &[(-36.0, 10.0), (-23.0, 60.0), (-36.0, 10.0)],
            "3341 case 3",
        ),
        (
            &[
                (-72.0, 10.0),
                (-36.0, 10.0),
                (-23.0, 60.0),
                (-36.0, 10.0),
                (-72.0, 10.0),
            ],
            "3341 case 4",
        ),
        (
            &[(-26.0, 20.0), (-20.0, 20.1), (-26.0, 20.0)],
            "3341 case 5",
        ),
    ];
    for (seg, what) in cases {
        let (a, _) = steps(&segments(seg));
        close(a.readings().integrated.unwrap(), -23.0, 0.1, what);
    }
}

/// Case 9: 1.34 s at −20 dBFS and 1.66 s at −30, five times: the short-term loudness is
/// −23.0 ±0.1 at every step once 3 s are in. Case 12: 0.18 s at −20 and 0.22 s at −30,
/// 25 times: the momentary loudness is −23.0 ±0.1 at every step after 1 s.
#[test]
fn tech_3341_cases_9_and_12_constant_short_term_and_momentary() {
    let mut seg = Vec::new();
    for _ in 0..5 {
        seg.extend([(-20.0, 1.34), (-30.0, 1.66)]);
    }
    let (_, rs) = steps(&segments(&seg));
    let mut worst = 0f64;
    for r in rs.iter().skip(30) {
        worst = worst.max((r.short_term + 23.0).abs());
    }
    close(
        -23.0 + worst,
        -23.0,
        0.1,
        "3341 case 9, the worst short-term step",
    );
    let mut seg = Vec::new();
    for _ in 0..25 {
        seg.extend([(-20.0, 0.18), (-30.0, 0.22)]);
    }
    let (_, rs) = steps(&segments(&seg));
    let mut worst = 0f64;
    for r in rs.iter().skip(10) {
        worst = worst.max((r.momentary + 23.0).abs());
    }
    close(
        -23.0 + worst,
        -23.0,
        0.1,
        "3341 case 12, the worst momentary step",
    );
}

/// 3342 cases 1–4: loudness range ±1 LU.
#[test]
fn tech_3342_cases_1_to_4() {
    let cases: [(&[(f64, f64)], f64); 4] = [
        (&[(-20.0, 20.0), (-30.0, 20.0)], 10.0),
        (&[(-20.0, 20.0), (-15.0, 20.0)], 5.0),
        (&[(-40.0, 20.0), (-20.0, 20.0)], 20.0),
        (
            &[
                (-50.0, 20.0),
                (-35.0, 20.0),
                (-20.0, 20.0),
                (-35.0, 20.0),
                (-50.0, 20.0),
            ],
            15.0,
        ),
    ];
    for (seg, want) in cases {
        let (a, _) = steps(&segments(seg));
        close(a.readings().range.unwrap(), want, 1.0, "3342 LRA");
    }
}

/// BS.1770-4 Annex 2, Table 1: the example 4× interpolator's 48 taps, phase by phase.
const ANNEX2: [[f64; 12]; 4] = [
    [
        0.0017089843750,
        0.0109863281250,
        -0.0196533203125,
        0.0332031250000,
        -0.0594482421875,
        0.1373291015625,
        0.9721679687500,
        -0.1022949218750,
        0.0476074218750,
        -0.0266113281250,
        0.0148925781250,
        -0.0083007812500,
    ],
    [
        -0.0291748046875,
        0.0292968750000,
        -0.0517578125000,
        0.0891113281250,
        -0.1665039062500,
        0.4650878906250,
        0.7797851562500,
        -0.2003173828125,
        0.1015625000000,
        -0.0582275390625,
        0.0330810546875,
        -0.0189208984375,
    ],
    [
        -0.0189208984375,
        0.0330810546875,
        -0.0582275390625,
        0.1015625000000,
        -0.2003173828125,
        0.7797851562500,
        0.4650878906250,
        -0.1665039062500,
        0.0891113281250,
        -0.0517578125000,
        0.0292968750000,
        -0.0291748046875,
    ],
    [
        -0.0083007812500,
        0.0148925781250,
        -0.0266113281250,
        0.0476074218750,
        -0.1022949218750,
        0.9721679687500,
        0.1373291015625,
        -0.0594482421875,
        0.0332031250000,
        -0.0196533203125,
        0.0109863281250,
        0.0017089843750,
    ],
];

/// The true peak by Annex 2's interpolator, dBTP: the largest of the four phases.
fn annex2_true_peak(x: &[f32]) -> f64 {
    let mut peak = 0f64;
    for n in 11..x.len() {
        for phase in &ANNEX2 {
            let mut y = 0.0;
            for (k, h) in phase.iter().enumerate() {
                y += h * x[n - k] as f64;
            }
            peak = peak.max(y.abs());
        }
    }
    20.0 * math::log10(peak)
}

/// 3341 cases 15–19 (true peak, +0.2 / −0.4 dB): sines at fs/4, fs/6 and fs/8 whose
/// samples miss the peak by the phase offset, at −6 dBTP, and at +3 dBTP with samples
/// under 0 dBFS. Faded in and out over 100 ms (a sine switched on abruptly overshoots
/// between its first samples, which is not what these cases test). Ours and Annex 2's
/// interpolator both read within the tolerance; ours within 0.02 dB of the exact peak.
#[test]
fn tech_3341_cases_15_to_19_true_peak() {
    let cases = [
        ("15", 4.0, 0.0, -6.0),
        ("16", 4.0, 45.0, -6.0),
        ("17", 6.0, 60.0, -6.0),
        ("18", 8.0, 67.5, -6.0),
        ("19", 4.0, 45.0, 3.0),
    ];
    for (name, div, deg, db) in cases {
        let hz = FS as f64 / div;
        let x = faded(sine(hz, db, 2.0, f64::to_radians(deg)), 4_800);
        let (a, _) = steps(&x);
        let ours = a.readings().true_peak_db;
        let annex = annex2_true_peak(&x);
        let sample = a.readings().sample_peak_db;
        eprintln!(
            "3341 case {name}: ours {ours:.3} dBTP, Annex 2 {annex:.3}, sample peak {sample:.3} dBFS"
        );
        assert!(
            (db - 0.4..=db + 0.2).contains(&ours),
            "case {name}: ours {ours}"
        );
        assert!(
            (db - 0.4..=db + 0.2).contains(&annex),
            "case {name}: Annex 2 {annex}"
        );
        // The peak is known exactly (the sine's amplitude): ours, with its refinement
        // between the 4× points, is within 0.02 dB of it; Annex 2's plain 4× reads up to
        // 0.3 dB under it here, as a plain 4× meter does.
        assert!(
            (ours - db).abs() < 0.02,
            "case {name}: ours {ours} for {db}"
        );
    }
}

/// The weighting's gain at 997 Hz is the reason for the standard's −0.691 constant (a
/// 997 Hz sine reads its level): the tonal curve's K-weights, computed from the
/// coefficients, read +0.691 dB there.
#[test]
fn the_k_weight_at_997_hz_is_the_standards_constant() {
    let g = 10.0 * math::log10(super::tonal::k_weight(FS, 997.0));
    close(g, 0.691, 0.002, "|K(997 Hz)|², dB");
}

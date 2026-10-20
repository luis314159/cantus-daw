//! The tempo and beat tracker's tests (ADR 0061), and the accuracy corpus: generated
//! click and drum tracks at known tempos from 60 to 200 BPM, with swing.

use super::testkit::{click_track, drum_loop, Material};
use super::*;
use crate::onset::detect;

fn grid(m: &Material) -> Grid {
    analyse(&detect(&m.audio, m.rate), m.rate, m.audio.len(), 4)
}

#[test]
fn a_click_track_gives_its_tempo_and_beats() {
    for bpm in [80.0, 100.0, 120.0, 140.0, 170.0] {
        let m = click_track(48_000.0, bpm, 8, 0.25);
        let g = grid(&m);
        assert!((g.bpm - bpm).abs() < 0.5, "{bpm}: {}", g.bpm);
        assert!(g.confidence > 0.5, "{bpm}: confidence {}", g.confidence);
        // Every true beat has a tracked beat within 10 ms.
        for b in &m.beats {
            let near = g
                .beats
                .iter()
                .map(|x| (x - b).abs())
                .fold(f64::MAX, f64::min);
            assert!(near < 480.0, "{bpm}: beat at {b} missed by {near}");
        }
        assert_eq!(
            g.first_downbeat().map(|d| (d - m.beats[0]).abs() < 480.0),
            Some(true),
            "{bpm}: downbeat"
        );
    }
}

#[test]
fn a_swung_drum_loop_gives_its_tempo_and_downbeat() {
    for (bpm, swing) in [(92.0, 0.0), (92.0, 0.6), (128.0, 0.4), (75.0, 0.2)] {
        let m = drum_loop(44_100.0, bpm, 8, swing, 0.37, 11);
        let g = grid(&m);
        assert!((g.bpm - bpm).abs() < 0.5, "{bpm}/{swing}: {}", g.bpm);
        let d = g.first_downbeat().unwrap();
        // On a bar line: a multiple of four beats from the first kick.
        let per = 60.0 / bpm * m.rate as f64;
        let bars = (d - m.beats[0]) / (4.0 * per);
        assert!(
            (bars - bars.round()).abs() * 4.0 * per < 441.0,
            "{bpm}/{swing}: downbeat at {d}, first kick {}",
            m.beats[0]
        );
    }
}

#[test]
fn a_loop_of_whole_bars_is_told_and_its_tempo_made_exact() {
    for (bpm, bars) in [(92.0, 4usize), (127.0, 2), (85.0, 8)] {
        let mut m = drum_loop(48_000.0, bpm, bars, 0.0, 0.0, 5);
        let exact = (60.0 / bpm * 4.0 * bars as f64 * 48_000.0).round() as usize;
        m.audio.truncate(exact);
        let g = grid(&m);
        assert_eq!(
            g.bars,
            Some(bars as u32),
            "{bpm}: {} bpm, beats {:?}, downbeat {}",
            g.bpm,
            &g.beats[..4],
            g.downbeat
        );
        assert!((g.bpm - bpm).abs() < 0.01, "{bpm}: {}", g.bpm);
    }
}

#[test]
fn silence_has_no_tempo() {
    let o = detect(&vec![0.0; 96_000], 48_000.0);
    let g = analyse(&o, 48_000.0, 96_000, 4);
    assert_eq!(g.bpm, 0.0);
    assert_eq!(g.confidence, 0.0);
    assert!(g.beats.is_empty());
}

/// The corpus (ADR 0061 §9): every file from 60 to 200 BPM, metronomic and played. What
/// is asserted, and reported:
/// - within ±1 BPM: of the metronomic files with a tempo in 70–180, at least 97 %; of the
///   humanised ones (timing jitter, a slow drift, a pad under them, a pick-up, 3/4, 6/8),
///   at least 90 % — and every miss is an octave (×2 or ×½), never another tempo;
/// - a tempo outside 70–180 comes back as its octave inside it (×2 or ×½, within 1 BPM) —
///   by design, the user corrects it with ×2 / ½ — every one of them, asserted;
/// - ×1.5 and ×⅔ answers are metric errors, counted apart (a 6/8 groove's dotted-quarter
///   pulse is one of its two right answers, not an error).
#[test]
fn the_corpus_is_within_one_bpm() {
    use super::testkit::{groove, Feel, Metre};
    #[derive(Default)]
    struct Tally {
        files: usize,
        inside: usize,
        inside_ok: usize,
        outside: usize,
        outside_octave: usize,
        metric: usize,
        /// Misses inside 70–180 that are an octave of the truth.
        inside_octave: usize,
        misses: Vec<(f64, usize, f64)>,
    }
    let near = |a: f64, b: f64| (a - b).abs() <= 1.0;
    let feel = |metre, jitter_ms, pickup| Feel {
        metre,
        jitter_ms,
        drift: 0.01,
        pad_db: Some(-20.0),
        pickup,
    };
    let (mut metro, mut played) = (Tally::default(), Tally::default());
    let mut bpm = 60.0;
    while bpm <= 200.0 {
        for kind in 0..7 {
            let seed = bpm as u64 * 7 + kind as u64;
            let (m, metre) = match kind {
                0 => (click_track(44_100.0, bpm, 8, 0.1), Metre::FourFour),
                1 => (drum_loop(44_100.0, bpm, 6, 0.0, 0.2, seed), Metre::FourFour),
                2 => (
                    drum_loop(48_000.0, bpm, 6, 0.6, 0.05, seed),
                    Metre::FourFour,
                ),
                3 => (
                    groove(48_000.0, bpm, 6, 0.1, feel(Metre::FourFour, 8.0, 0), seed),
                    Metre::FourFour,
                ),
                4 => (
                    groove(44_100.0, bpm, 6, 0.3, feel(Metre::FourFour, 15.0, 1), seed),
                    Metre::FourFour,
                ),
                5 => (
                    groove(48_000.0, bpm, 8, 0.1, feel(Metre::ThreeFour, 8.0, 0), seed),
                    Metre::ThreeFour,
                ),
                _ => (
                    groove(48_000.0, bpm, 8, 0.2, feel(Metre::SixEight, 8.0, 0), seed),
                    Metre::SixEight,
                ),
            };
            let got = grid(&m).bpm;
            // A 6/8 groove's pulse may be its quarters or its dotted quarters.
            let truths: Vec<f64> = if metre == Metre::SixEight {
                vec![bpm, bpm * 2.0 / 3.0]
            } else {
                vec![bpm]
            };
            let t = if kind < 3 { &mut metro } else { &mut played };
            t.files += 1;
            let in_range = truths.iter().any(|b| (PREFER.0..=PREFER.1).contains(b));
            let ok = truths.iter().any(|b| near(got, *b));
            let octave = truths
                .iter()
                .any(|b| near(got, b * 2.0) || near(got, b / 2.0));
            if !ok
                && !octave
                && truths
                    .iter()
                    .any(|b| near(got, b * 1.5) || near(got, b * 2.0 / 3.0))
            {
                t.metric += 1;
            }
            if in_range {
                t.inside += 1;
                if ok {
                    t.inside_ok += 1;
                } else {
                    t.inside_octave += usize::from(octave);
                    t.misses.push((bpm, kind, got));
                }
            } else {
                t.outside += 1;
                if octave && (PREFER.0..=PREFER.1).contains(&got) {
                    t.outside_octave += 1;
                } else {
                    t.misses.push((bpm, kind, got));
                }
            }
        }
        bpm += 5.0;
    }
    for (name, t) in [("metronomic", &metro), ("played", &played)] {
        eprintln!(
            "corpus {name}: {} files; {}/{} within ±1 BPM in 70–180 ({} of the misses an \
             octave); {}/{} outside it answered by their octave inside it; {} metric (×1.5, ×⅔) \
             errors; misses {:?}",
            t.files,
            t.inside_ok,
            t.inside,
            t.inside_octave,
            t.outside_octave,
            t.outside,
            t.metric,
            t.misses
        );
    }
    assert!(
        metro.inside_ok * 100 >= metro.inside * 97,
        "{:?}",
        metro.misses
    );
    assert!(
        played.inside_ok * 100 >= played.inside * 90,
        "{:?}",
        played.misses
    );
    // Every miss is an octave (the ×2 / ½ the user corrects), never another tempo.
    for t in [&metro, &played] {
        assert_eq!(t.inside_ok + t.inside_octave, t.inside, "{:?}", t.misses);
        assert_eq!(t.metric, 0, "{:?}", t.misses);
    }
    assert_eq!(metro.outside_octave, metro.outside, "{:?}", metro.misses);
    assert_eq!(played.outside_octave, played.outside, "{:?}", played.misses);
}

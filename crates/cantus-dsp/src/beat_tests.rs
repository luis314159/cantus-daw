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
            let near = g.beats.iter().map(|x| (x - b).abs()).fold(f64::MAX, f64::min);
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
        assert_eq!(g.bars, Some(bars as u32), "{bpm}");
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

/// The corpus (ADR 0061's measurement): within ±1 BPM, and the octave errors, over click
/// and drum tracks from 60 to 200 BPM. A tempo outside 70–180 is reported in that range
/// by design (an octave "error" the user corrects with ×2 / ÷2), so it is counted apart.
#[test]
fn the_corpus_is_within_one_bpm() {
    let (mut inside, mut inside_ok, mut outside, mut octave) = (0, 0, 0, 0);
    let mut misses = Vec::new();
    let mut bpm = 60.0;
    while bpm <= 200.0 {
        for kind in 0..3 {
            let m = match kind {
                0 => click_track(44_100.0, bpm, 8, 0.1),
                1 => drum_loop(44_100.0, bpm, 6, 0.0, 0.2, bpm as u64),
                _ => drum_loop(48_000.0, bpm, 6, 0.6, 0.05, bpm as u64 + 1),
            };
            let g = grid(&m);
            let in_range = (PREFER.0..=PREFER.1).contains(&bpm);
            let ok = (g.bpm - bpm).abs() <= 1.0;
            let oct = [0.5, 2.0, 1.5, 2.0 / 3.0]
                .iter()
                .any(|k| (g.bpm - bpm * k).abs() <= 1.0);
            if in_range {
                inside += 1;
                if ok {
                    inside_ok += 1;
                } else {
                    misses.push((bpm, kind, g.bpm));
                }
            } else {
                outside += 1;
            }
            if !ok && oct {
                octave += 1;
            }
        }
        bpm += 5.0;
    }
    eprintln!(
        "corpus: {inside_ok}/{inside} within ±1 BPM in 70–180; {outside} outside; {octave} octave errors; misses {misses:?}"
    );
    assert!(inside_ok * 100 >= inside * 97, "{inside_ok}/{inside}: {misses:?}");
}

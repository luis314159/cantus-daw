//! Drums to notes on our own drum instrument (round 34, ADR 0086): a two-minute pattern
//! of known MIDI — kicks, snares, three toms, closed, pedal and open hats, crashes and a
//! ride — rendered through Pulsus's factory kits by the offline render, transcribed by
//! `cantus_dsp::onset::drums`, and scored class by class with mir_eval's rules (an onset
//! within 50 ms). Prints every kit's F-measures and holds each class to its floor.

use cantus_core::*;
use cantus_dsp::onset::drums::{transcribe, Class};
use cantus_dsp::onset::eval::{onsets, Score};
use cantus_engine::{render, RenderOptions};
use std::sync::Arc;

const BPM: f64 = 110.0;

/// The pattern, in beats: `(beat, GM key, velocity)`. Sixteen bars of a groove that
/// changes every four: eighth hats with an open one, a ride bar, tom fills, crashes on the
/// fours' downbeats, pedal hats under the ride, ghost-free.
fn pattern() -> Vec<(f64, u8, u8)> {
    let mut v = Vec::new();
    for bar in 0..16 {
        let b0 = bar as f64 * 4.0;
        let section = bar / 4;
        let fill = bar % 4 == 3;
        if bar % 4 == 0 {
            v.push((b0, 49, 115));
        }
        for e in 0..8 {
            let at = b0 + e as f64 * 0.5;
            if fill && e >= 4 {
                continue;
            }
            match section {
                1 => v.push((at, 51, 90)),
                _ if !(bar % 4 == 0 && e == 0) => {
                    v.push((at, if e == 7 && section == 2 { 46 } else { 42 }, 85))
                }
                _ => {}
            }
        }
        if section == 1 {
            for q in [1.0, 3.0] {
                v.push((b0 + q, 44, 80));
            }
        }
        v.push((b0, 36, 120));
        v.push((b0 + 2.5, 36, 105));
        if section == 3 {
            v.push((b0 + 1.5, 36, 100));
        }
        v.push((b0 + 1.0, 38, 115));
        if fill {
            for (i, key) in [50u8, 50, 47, 47, 45, 45].iter().enumerate() {
                v.push((b0 + 2.0 + 0.33 * i as f64, *key, 105));
            }
        } else {
            v.push((b0 + 3.0, 38, 115));
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

fn class_of(key: u8) -> Option<Class> {
    match key {
        35 | 36 => Some(Class::Kick),
        38 | 40 => Some(Class::Snare),
        41 | 43 | 45 | 47 | 48 | 50 => Some(Class::Toms),
        42 | 44 | 46 => Some(Class::HiHat),
        49 | 51 | 52 | 55 | 57 | 59 => Some(Class::Cymbals),
        _ => None,
    }
}

/// The pattern rendered through `kit`, mono at 44.1 kHz.
fn render_kit(kit: PulsusParams) -> Vec<f32> {
    let mut p = Project {
        tempo_bpm: BPM,
        ..Project::default()
    };
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(kit);
    let bars = Ticks::beats(16 * 4);
    let mut c = p.make_clip("Groove", Ticks::ZERO, bars);
    for (beat, key, vel) in pattern() {
        let start = Ticks((beat * Ticks::QUARTER.0 as f64).round() as i64);
        let n = p.make_note(start, Ticks::QUARTER / 4, key, vel);
        c.notes.push(n);
    }
    t.clips.push(c);
    p.tracks.push(t);
    let r = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            sample_rate: 44_100,
            channels: 2,
            tail_seconds: 2.0,
            ..Default::default()
        },
    )
    .expect("render");
    r.samples.chunks(2).map(|c| 0.5 * (c[0] + c[1])).collect()
}

fn score(found: &[cantus_dsp::onset::drums::Hit]) -> Vec<Score> {
    let spb = 60.0 / BPM;
    Class::ALL
        .iter()
        .map(|c| {
            let truth: Vec<f64> = pattern()
                .iter()
                .filter(|h| class_of(h.1) == Some(*c))
                .map(|h| h.0 * spb)
                .collect();
            let f: Vec<f64> = found
                .iter()
                .filter(|h| h.class == *c)
                .map(|h| h.time)
                .collect();
            onsets(&truth, &f, 0.05)
        })
        .collect()
}

#[test]
fn pulsus_kits_transcribe_class_by_class() {
    let mut per: Vec<Vec<Score>> = vec![Vec::new(); 5];
    for kit in PulsusParams::factory() {
        if kit.label_key.ends_with("percussion") {
            // Congas and shakers, no kit: nothing of the pattern sounds as a drum kit.
            continue;
        }
        let x = render_kit(kit.kit.clone());
        if let Some(dir) = std::env::var_os("CANTUS_DRUMS_WRITE") {
            // The renders and their truth in MDB Drums' layout, for the tuning search
            // (`cantus-dsp/tests/onset_drums_mdb.rs`, `CANTUS_DRUMS_EXTRA`).
            let dir = std::path::PathBuf::from(dir);
            let name = kit.label_key.replace('.', "_");
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 44_100,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let mut w =
                hound::WavWriter::create(dir.join(format!("{name}_Drum.wav")), spec).unwrap();
            for v in &x {
                w.write_sample(*v).unwrap();
            }
            w.finalize().unwrap();
            let code = |c: Class| match c {
                Class::Kick => "KD",
                Class::Snare => "SD",
                Class::Toms => "TT",
                Class::HiHat => "HH",
                Class::Cymbals => "CY",
            };
            let spb = 60.0 / BPM;
            let text: String = pattern()
                .iter()
                .filter_map(|h| {
                    Some(format!(
                        "{:.6} 	 {} 
",
                        h.0 * spb,
                        code(class_of(h.1)?)
                    ))
                })
                .collect();
            std::fs::write(dir.join(format!("{name}_class.txt")), text).unwrap();
        }
        let found = transcribe(&x, 44_100.0, 0.5);
        let s = score(&found);
        let mut line = format!("{:22}", kit.label_key);
        for (i, c) in Class::ALL.iter().enumerate() {
            line += &format!(
                " {c:?} {:.2} ({}/{}/{})",
                s[i].f, s[i].matched, s[i].truth, s[i].found
            );
            per[i].push(s[i]);
        }
        println!("{line}");
        if std::env::var_os("CANTUS_DRUMS_CONFUSION").is_some() {
            let spb = 60.0 / BPM;
            let truth: Vec<(f64, Class)> = pattern()
                .iter()
                .filter_map(|h| Some((h.0 * spb, class_of(h.1)?)))
                .collect();
            let mut conf = [[0usize; 6]; 5];
            for h in &found {
                if !truth
                    .iter()
                    .any(|t| t.1 == h.class && (t.0 - h.time).abs() <= 0.05)
                {
                    let near = truth
                        .iter()
                        .filter(|t| (t.0 - h.time).abs() <= 0.05)
                        .map(|t| t.1 as usize)
                        .next()
                        .unwrap_or(5);
                    conf[h.class as usize][near] += 1;
                }
            }
            let mut missed = [[0usize; 6]; 5];
            for t in &truth {
                if !found
                    .iter()
                    .any(|h| h.class == t.1 && (t.0 - h.time).abs() <= 0.05)
                {
                    let got = found
                        .iter()
                        .filter(|h| (t.0 - h.time).abs() <= 0.05)
                        .map(|h| h.class as usize)
                        .next()
                        .unwrap_or(5);
                    missed[t.1 as usize][got] += 1;
                }
            }
            println!("   false (found → truly there K S T H C none): {conf:?}");
            println!("   missed (truth → found there instead):      {missed:?}");
        }
    }
    let total: Vec<Score> = per.iter().map(|v| Score::sum(v)).collect();
    println!(
        "all kits: {}",
        Class::ALL
            .iter()
            .zip(&total)
            .map(|(c, s)| format!("{c:?} F {:.3} P {:.3} R {:.3}", s.f, s.precision, s.recall))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    // The floors this build holds (ADR 0086 §3 gives the measured numbers).
    for (i, c) in Class::ALL.iter().enumerate() {
        assert!(total[i].f >= FLOOR[i], "{c:?} {:?}", total[i]);
    }
}

/// The F-measures below which this test fails: kick, snare, toms, hi-hat, cymbals (just
/// under what this build measures: 0.986, 0.833, 0.286, 0.847, 0.250).
const FLOOR: [f64; 5] = [0.95, 0.78, 0.2, 0.8, 0.18];

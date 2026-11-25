//! Arcus's renders for the owner's ears (round 42, ADR 0157): the cello and the double bass —
//! a solo note at pp, mf and ff, a four-bar legato phrase with a CC1 swell, pizzicato and
//! tremolo — as 48 kHz 24-bit WAVs. Run by `tools/arcus/listen.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_listen -- <out dir> [levels]
//! ```
//!
//! `levels` prints each render's peak and RMS instead of only writing it.

use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{Arcus, ArcusSettings, Technique};
use cantus_dsp::voices::PlayMode;
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;

/// A score's event at a time in seconds.
#[derive(Copy, Clone, Debug)]
enum Ev {
    On(u8, f32),
    Off(u8),
    /// CC1 (the dynamic), 0 … 1.
    Cc1(f32),
}

/// A render: its file's name, the settings, the score, its length (s).
type Job = (String, ArcusSettings, Vec<(f64, Ev)>, f64);

fn id(k: u8) -> u64 {
    k as u64 + 7_000
}

/// Render `score` (seconds, event) for `secs` seconds, in blocks of 256, cut at each event.
fn render(s: &ArcusSettings, score: &[(f64, Ev)], secs: f64) -> (Vec<f32>, Vec<f32>) {
    let mut a = Arcus::new(SR);
    a.set(s);
    let frames = (secs * SR as f64) as usize;
    let mut ev: Vec<(usize, Ev)> = score
        .iter()
        .map(|(t, e)| ((t * SR as f64) as usize, *e))
        .collect();
    ev.sort_by_key(|e| e.0);
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    let (mut at, mut i) = (0usize, 0usize);
    while at < frames {
        while i < ev.len() && ev[i].0 <= at {
            match ev[i].1 {
                Ev::On(k, v) => a.note_on(k, v, id(k)),
                Ev::Off(k) => a.note_off(k, id(k)),
                Ev::Cc1(v) => a.controller(0, v),
            }
            i += 1;
        }
        let next = ev.get(i).map_or(frames, |e| e.0);
        let n = 256.min(next.max(at + 1) - at).min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l, r)
}

fn write(path: &Path, l: &[f32], r: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create");
    for (a, b) in l.iter().zip(r) {
        for x in [a, b] {
            let v = (x.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
            w.write_sample(v).expect("write");
        }
    }
    w.finalize().expect("finalize");
}

fn db(x: f64) -> f64 {
    20.0 * cantus_dsp::math::log10(x.max(1e-12))
}

/// r42:arcus-p6 — one held note swelled by CC1 from pp through mf to ff over 6 s and back
/// over 2 (where the brightness following the bow's force is easiest to hear).
fn swell(key: u8) -> (Vec<(f64, Ev)>, f64) {
    let mut ev = vec![(0.0, Ev::Cc1(0.05)), (0.1, Ev::On(key, 0.5))];
    let steps = 160;
    for j in 0..=steps {
        let u = j as f64 / steps as f64;
        let v = if u < 0.75 {
            0.05 + 0.95 * u / 0.75
        } else {
            1.0 - 0.95 * (u - 0.75) / 0.25
        };
        ev.push((0.4 + 8.0 * u, Ev::Cc1(v as f32)));
    }
    ev.push((8.6, Ev::Off(key)));
    (ev, 10.0)
}

/// A legato line: (key, beats), each overlapping the next by 30 ms, at `bpm`, with a CC1
/// swell from `lo` up to `hi` over bars 2–3 and back over bar 4.
fn legato(line: &[(u8, f64)], bpm: f64, vel: f32, lo: f32, hi: f32) -> (Vec<(f64, Ev)>, f64) {
    let beat = 60.0 / bpm;
    let mut ev = vec![(0.0, Ev::Cc1(lo))];
    let mut t = 0.2;
    for (i, (k, beats)) in line.iter().enumerate() {
        ev.push((t, Ev::On(*k, vel)));
        let end = t + beats * beat;
        let off = if i + 1 < line.len() { end + 0.03 } else { end };
        ev.push((off, Ev::Off(*k)));
        t = end;
    }
    let bar = 4.0 * beat;
    let steps = 80;
    for j in 0..=steps {
        let u = j as f64 / steps as f64;
        // Bars 2 and 3 up, bar 4 down.
        let (time, v) = if u < 2.0 / 3.0 {
            (
                0.2 + bar + u * 1.5 * 2.0 * bar,
                lo + (hi - lo) * (u * 1.5) as f32,
            )
        } else {
            let w = (u - 2.0 / 3.0) * 3.0;
            (0.2 + 3.0 * bar + w * bar, hi + (lo - hi) * w as f32)
        };
        ev.push((time, Ev::Cc1(v)));
    }
    (ev, t + 2.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(
        args.get(1)
            .cloned()
            .unwrap_or_else(|| "arcus-listen".into()),
    );
    let levels = args.iter().any(|a| a == "levels");
    std::fs::create_dir_all(&out).expect("out dir");
    for (name, inst, note, phrase_shift) in [
        ("cello", Instrument::Cello, 50u8, 0i32),
        ("bass", Instrument::Bass, 38u8, -12i32),
    ] {
        // r42:arcus-p6: `ARCUS_FRICTION=memory` renders with the friction's memory.
        let memory = std::env::var("ARCUS_FRICTION").is_ok_and(|f| f == "memory");
        let base = ArcusSettings {
            instrument: inst,
            friction: if memory {
                cantus_dsp::arcus::string::Friction::Memory
            } else {
                cantus_dsp::arcus::string::Friction::Hyperbolic
            },
            ..ArcusSettings::default()
        };
        let mut jobs: Vec<Job> = Vec::new();
        // A solo note at three dynamics (the velocity alone: pp, mf, ff).
        for (dyn_name, vel) in [("pp", 0.22f32), ("mf", 0.6), ("ff", 0.98)] {
            jobs.push((
                format!("{name}-note-{dyn_name}"),
                base,
                vec![(0.1, Ev::On(note, vel)), (4.1, Ev::Off(note))],
                5.5,
            ));
        }
        // r42:arcus-p6: the swell, an octave over the lowest string's fifth.
        let (ev, secs) = swell(note + 7);
        jobs.push((format!("{name}-swell"), base, ev, secs));
        // Beside the ABX references: the Karoryfer cello's A3 at p, mf and f (its files
        // `A2_*_d.wav`, named an octave low), the Smolken bass's D2.
        let abx = if inst == Instrument::Cello {
            57u8
        } else {
            38u8
        };
        for (dyn_name, vel) in [("p", 0.35f32), ("mf", 0.6), ("f", 0.85)] {
            jobs.push((
                format!("{name}-abx-{abx}-{dyn_name}"),
                base,
                vec![(0.05, Ev::On(abx, vel)), (4.05, Ev::Off(abx))],
                5.0,
            ));
        }
        // Four bars, legato, with a CC1 swell (a simple line in D minor).
        let line: Vec<(u8, f64)> = [
            (50, 2.0),
            (52, 1.0),
            (53, 1.0),
            (55, 2.0),
            (57, 2.0),
            (58, 1.0),
            (57, 1.0),
            (55, 1.0),
            (53, 1.0),
            (52, 4.0),
        ]
        .iter()
        .map(|(k, b)| ((*k + phrase_shift) as u8, *b))
        .collect();
        let (ev, secs) = legato(&line, 72.0, 0.55, 0.3, 0.85);
        jobs.push((format!("{name}-phrase-legato-cc1"), base, ev, secs));
        // Pizzicato: a walking line, quarter notes at 100.
        let pizz = ArcusSettings {
            technique: Technique::Pizzicato,
            mode: PlayMode::Poly,
            ..base
        };
        let walk = [50u8, 53, 57, 55, 53, 52, 50, 45];
        let mut ev = Vec::new();
        for (i, k) in walk.iter().enumerate() {
            let k = (*k as i32 + phrase_shift) as u8;
            let t = 0.1 + i as f64 * 0.6;
            ev.push((t, Ev::On(k, 0.55 + 0.05 * (i % 3) as f32)));
            ev.push((t + 0.5, Ev::Off(k)));
        }
        jobs.push((format!("{name}-pizzicato"), pizz, ev, 7.0));
        // Tremolo: two held notes.
        let trem = ArcusSettings {
            technique: Technique::Tremolo,
            ..base
        };
        let (a, b) = ((note as i32) as u8, (note as i32 + 5) as u8);
        jobs.push((
            format!("{name}-tremolo"),
            trem,
            vec![
                (0.1, Ev::On(a, 0.6)),
                (2.6, Ev::Off(a)),
                (2.7, Ev::On(b, 0.8)),
                (5.2, Ev::Off(b)),
            ],
            6.5,
        ));
        for (file, s, ev, secs) in jobs {
            let (l, r) = render(&s, &ev, secs);
            let peak = l.iter().chain(&r).fold(0f32, |m, x| m.max(x.abs())) as f64;
            let rms =
                (l.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>() / l.len() as f64).sqrt();
            if levels {
                println!(
                    "{file:28} peak {:6.1} dBFS  RMS {:6.1} dBFS",
                    db(peak),
                    db(rms)
                );
            }
            write(&out.join(format!("{file}.wav")), &l, &r);
        }
    }
    println!("written to {}", out.display());
}

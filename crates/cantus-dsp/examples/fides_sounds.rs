//! Fides's lab (round 41, ADR 0150): its levels and its sounds, straight from the DSP.
//!
//! `cargo run --release -p cantus-dsp --example fides_sounds -- levels` prints every
//! exciter on every resonator at C4, velocity 100 (peak and RMS over the first second, dB):
//! the table the resonators' trims are fitted to.
//!
//! `… -- wav <dir>` writes one WAV per exciter × resonator (a short phrase), for the ear.

use cantus_dsp::fides::body::BodyKind;
use cantus_dsp::fides::exciter::ExciterKind;
use cantus_dsp::fides::tables::ResKind;
use cantus_dsp::fides::{Fides, FidesSettings};
use cantus_dsp::math;

const SR: f32 = 48_000.0;

fn db(x: f32) -> f32 {
    20.0 * math::log10(x.max(1e-9))
}

/// One note (or a phrase) through an instance: (left, right).
fn play(
    s: &FidesSettings,
    notes: &[(usize, u8, f32, usize)],
    frames: usize,
) -> (Vec<f32>, Vec<f32>) {
    let mut f = Fides::new(SR);
    f.set(s);
    let mut l = vec![0.0; frames];
    let mut r = vec![0.0; frames];
    let mut events: Vec<(usize, bool, u8, f32)> = Vec::new();
    for &(at, key, vel, len) in notes {
        events.push((at, true, key, vel));
        events.push((at + len, false, key, 0.0));
    }
    events.sort_by_key(|e| (e.0, e.1));
    let mut at = 0;
    let mut ev = 0;
    while at < frames {
        while ev < events.len() && events[ev].0 <= at {
            let (_, on, key, vel) = events[ev];
            if on {
                f.note_on(key, vel, key as u64);
            } else {
                f.note_off(key, key as u64);
            }
            ev += 1;
        }
        let next = events.get(ev).map_or(frames, |e| e.0).max(at + 1);
        let n = (next - at).min(256).min(frames - at);
        f.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l, r)
}

fn settings(e: ExciterKind, r: ResKind) -> FidesSettings {
    let mut s = FidesSettings::default();
    s.exc.kind = e;
    s.a.kind = r;
    s.body.kind = BodyKind::None;
    if e == ExciterKind::Noise {
        s.noise.sustain = 0.6;
    }
    s
}

fn levels() {
    print!("{:<9}", "");
    for r in ResKind::ALL {
        print!("{:>14}", format!("{r:?}"));
    }
    println!();
    for e in ExciterKind::ALL {
        if e == ExciterKind::Input {
            continue;
        }
        print!("{:<9}", format!("{e:?}"));
        for r in ResKind::ALL {
            let (l, rr) = play(&settings(e, r), &[(0, 60, 100.0 / 127.0, 36_000)], 48_000);
            let peak = l.iter().chain(&rr).fold(0f32, |m, x| m.max(x.abs()));
            let rms = (l.iter().map(|x| x * x).sum::<f32>() / l.len() as f32).sqrt();
            print!("{:>14}", format!("{:.1}/{:.1}", db(peak), db(rms)));
        }
        println!();
    }
}

fn write(path: &std::path::Path, l: &[f32], r: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("a wav");
    for (a, b) in l.iter().zip(r) {
        for x in [a, b] {
            w.write_sample((x.clamp(-1.0, 1.0) * 8_388_607.0) as i32)
                .unwrap();
        }
    }
    w.finalize().unwrap();
}

fn wavs(dir: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let phrase = [
        (0, 48u8, 0.8f32, 20_000usize),
        (24_000, 55, 0.6, 20_000),
        (48_000, 60, 0.9, 20_000),
        (72_000, 64, 0.5, 20_000),
        (96_000, 67, 1.0, 40_000),
    ];
    for e in ExciterKind::ALL {
        if e == ExciterKind::Input {
            continue;
        }
        for r in ResKind::ALL {
            let mut s = settings(e, r);
            s.body.kind = if r == ResKind::String {
                BodyKind::Guitar
            } else {
                BodyKind::None
            };
            let (l, rr) = play(&s, &phrase, 192_000);
            let p = std::path::Path::new(dir).join(format!("{e:?}-{r:?}.wav").to_lowercase());
            write(&p, &l, &rr);
        }
    }
    println!("written to {dir}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("wav") => wavs(args.get(1).map_or("fides-lab", String::as_str)),
        _ => levels(),
    }
}

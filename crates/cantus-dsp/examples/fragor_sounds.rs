//! Fragor's listening examples: a synthesized DI guitar (a plucked string of our own —
//! extended Karplus–Strong, no recording) and a saw lead through each pedal, a chain, and
//! the germanium fuzz's battery running down.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example fragor_sounds -- <out-dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. Each file is scaled to peak at −3 dBFS (the pedals' own
//! levels differ; the scaling is said in the README written beside them).

use cantus_dsp::fragor::{BoardSettings, Fragor, Model};
use cantus_dsp::math;
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;

fn hash(i: u64) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

/// One plucked string: `hz`, `secs`, `vel` 0…1, palm-muted or ringing.
fn string(hz: f32, secs: f32, vel: f32, seed: u64, palm: bool) -> Vec<f32> {
    let n = (secs * SR) as usize;
    let period = SR / hz;
    let len = period.floor() as usize + 2;
    let frac = period - period.floor();
    // The excitation: a noise burst, low-passed by how hard it is picked, with the pick a
    // fifth of the way along the string (a comb).
    let bright = 0.25 + 0.6 * vel;
    let mut ex = vec![0.0f32; len];
    let mut lp = 0.0f32;
    for (i, e) in ex.iter_mut().enumerate() {
        lp += bright * (hash(seed * 7_919 + i as u64) - lp);
        *e = lp;
    }
    let pick = (len as f32 * 0.2) as usize;
    let ex: Vec<f32> = (0..len)
        .map(|i| ex[i] - if i >= pick { ex[i - pick] } else { 0.0 })
        .collect();
    let mut line = ex;
    let (loss, damp) = if palm { (0.985, 0.35) } else { (0.9975, 0.5) };
    let mut out = Vec::with_capacity(n);
    let mut p = 0usize;
    let mut z = 0.0f32;
    for _ in 0..n {
        let a = line[p];
        let b = line[(p + 1) % len];
        // Fractional delay by linear interpolation, then the loop's loss and a gentle
        // low-pass (the string's damping).
        let v = a + (b - a) * frac;
        z = damp * v + (1.0 - damp) * z;
        line[p] = loss * z;
        out.push(a);
        p = (p + 1) % len;
    }
    // The pickup: the string's displacement with its DC taken away (a 40 Hz high-pass),
    // and a gentle low-pass for the pickup's own resonance and the cable (about 5 kHz).
    let (mut x1, mut y1, mut lp) = (0.0f32, 0.0f32, 0.0f32);
    let r = 1.0 - std::f32::consts::TAU * 40.0 / SR;
    let a = 1.0 - math::exp(-std::f32::consts::TAU * 5_000.0 / SR);
    out.iter_mut().for_each(|v| {
        let y = *v - x1 + r * y1;
        x1 = *v;
        y1 = y;
        lp += a * (y - lp);
        *v = lp;
    });
    out
}

/// Add `src` into `dst` at `at` seconds, scaled.
fn mix(dst: &mut Vec<f32>, src: &[f32], at: f32, gain: f32) {
    let start = (at * SR) as usize;
    if dst.len() < start + src.len() {
        dst.resize(start + src.len(), 0.0);
    }
    for (i, s) in src.iter().enumerate() {
        dst[start + i] += s * gain;
    }
}

/// A riff: an E power chord ringing, palm-muted eighths, G and A chords, a lick, E again.
fn riff() -> Vec<f32> {
    let e2 = 82.41;
    let mut v = vec![];
    let chord = |v: &mut Vec<f32>, root: f32, at: f32, secs: f32, seed: u64| {
        for (j, m) in [1.0f32, 1.4983, 2.0].iter().enumerate() {
            let s = string(root * m, secs, 0.9, seed + j as u64, false);
            mix(v, &s, at + 0.008 * j as f32, 1.0);
        }
    };
    chord(&mut v, e2, 0.0, 1.0, 1);
    for i in 0..4 {
        let s = string(e2, 0.24, 0.8, 10 + i, true);
        mix(&mut v, &s, 1.0 + 0.25 * i as f32, 1.0);
    }
    chord(&mut v, 98.0, 2.0, 0.5, 20);
    chord(&mut v, 110.0, 2.5, 0.5, 30);
    for (i, hz) in [164.81f32, 196.0, 220.0, 246.94, 293.66, 329.63]
        .iter()
        .enumerate()
    {
        let s = string(*hz, 0.4, 0.7, 40 + i as u64, false);
        mix(&mut v, &s, 3.0 + 0.25 * i as f32, 1.0);
    }
    chord(&mut v, e2, 4.5, 2.5, 50);
    normalise(&mut v, 0.35);
    v
}

/// One ringing note for the battery's sweep.
fn note() -> Vec<f32> {
    let mut v = string(164.81, 2.4, 0.8, 99, false);
    normalise(&mut v, 0.3);
    v
}

/// A saw lead: a phrase of eight notes, each a naive saw with a soft attack.
fn synth() -> Vec<f32> {
    let notes = [
        220.0f32, 261.63, 329.63, 392.0, 329.63, 293.66, 261.63, 220.0,
    ];
    let mut v = vec![];
    for (i, hz) in notes.iter().enumerate() {
        let n = (0.375 * SR) as usize;
        let s: Vec<f32> = (0..n)
            .map(|j| {
                let ph = (j as f32 * hz / SR).fract();
                let env = (j as f32 / 200.0).min(1.0) * (1.0 - j as f32 / n as f32).max(0.0).sqrt();
                (2.0 * ph - 1.0) * env
            })
            .collect();
        mix(&mut v, &s, 0.375 * i as f32, 1.0);
    }
    normalise(&mut v, 0.25);
    v
}

fn normalise(v: &mut [f32], peak: f32) {
    let m = v.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-9);
    v.iter_mut().for_each(|x| *x *= peak / m);
}

/// One render: the file, the chain, its knobs, what to say of it.
type Run = (
    &'static str,
    Vec<Model>,
    Vec<(Model, [f32; 3])>,
    &'static str,
);

fn through(s: &BoardSettings, x: &[f32]) -> Vec<f32> {
    let mut f = Fragor::new(SR);
    f.set(s);
    f.reset();
    let tail = vec![0.0; (SR * 0.5) as usize];
    let input: Vec<f32> = x.iter().chain(&tail).copied().collect();
    let (mut l, mut r) = (input.clone(), input);
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        f.process(a, b);
    }
    l
}

fn board(chain: &[Model], knobs: &[(Model, [f32; 3])]) -> BoardSettings {
    let mut s = BoardSettings::default();
    for (i, m) in chain.iter().enumerate() {
        s.chain[i] = Some(*m);
    }
    for (m, k) in knobs {
        s.knobs[m.index()] = *k;
    }
    s.mono = true;
    s
}

fn write(dir: &Path, name: &str, v: &[f32], notes: &mut String, what: &str) -> f32 {
    let m = v.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-9);
    let g = math::db_to_gain(-3.0) / m;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(dir.join(name), spec).expect("wav");
    for x in v {
        let s = ((x * g).clamp(-1.0, 1.0) * 8_388_607.0) as i32;
        w.write_sample(s).unwrap();
        w.write_sample(s).unwrap();
    }
    w.finalize().unwrap();
    let db = 20.0 * math::log10(g);
    notes.push_str(&format!(
        "{name}\n  {what}\n  scaled {db:+.1} dB to peak at -3 dBFS\n\n"
    ));
    println!("{}", dir.join(name).display());
    db
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/fragor-sounds"));
    std::fs::create_dir_all(&dir).expect("dir");
    let mut notes = String::from(
        "Fragor's listening examples (fx side round). The guitar is synthesized \
         (a plucked string of our own, extended Karplus-Strong: no recording, nothing \
         licensed). 48 kHz, 24-bit, the same signal on both sides.\n\n",
    );
    let g = riff();
    write(
        &dir,
        "00-di-guitar.wav",
        &g,
        &mut notes,
        "The DI guitar, dry: the riff every file plays.",
    );
    use Model::*;
    let runs: [Run; 6] = [
        ("01-green-overdrive.wav", vec![Overdrive], vec![(Overdrive, [0.6, 0.55, 0.5])], "Green overdrive: Drive 60 %, Tone 55 %, Level 50 %."),
        ("02-op-amp-distortion.wav", vec![OpAmp], vec![(OpAmp, [0.7, 0.35, 0.5])], "Op-amp distortion: Distortion 70 %, Filter 35 %, Volume 50 %."),
        ("03-silicon-distortion.wav", vec![Silicon], vec![(Silicon, [0.6, 0.5, 0.5])], "Silicon distortion: Dist 60 %, Tone 50 %, Level 50 %."),
        ("04-germanium-fuzz.wav", vec![Fuzz], vec![(Fuzz, [0.85, 1.0, 0.5])], "Germanium fuzz: Fuzz 85 %, Bias 100 % (a fresh battery), Volume 50 %."),
        ("05-triangle-muff.wav", vec![Muff], vec![(Muff, [0.65, 0.45, 0.5])], "Triangle muff: Sustain 65 %, Tone 45 %, Volume 50 %."),
        (
            "06-chain-overdrive-fuzz-muff.wav",
            vec![Overdrive, Fuzz, Muff],
            vec![(Overdrive, [0.3, 0.6, 0.7]), (Fuzz, [0.6, 0.8, 0.45]), (Muff, [0.5, 0.4, 0.5])],
            "The chain of the design sheet: green overdrive into germanium fuzz into triangle muff.",
        ),
    ];
    for (name, chain, knobs, what) in runs.iter() {
        let s = board(chain, knobs);
        write(&dir, name, &through(&s, &g), &mut notes, what);
    }
    // The battery running down: the same note at Bias 100, 60, 35, 20 and 10 %.
    let n = note();
    let mut sweep = vec![];
    for (i, bias) in [1.0f32, 0.6, 0.35, 0.2, 0.1].iter().enumerate() {
        let s = board(&[Fuzz], &[(Fuzz, [1.0, *bias, 0.5])]);
        let y = through(&s, &n);
        let at = i as f32 * 2.6;
        mix(&mut sweep, &y, at, 1.0);
    }
    write(
        &dir,
        "07-germanium-fuzz-battery-sweep.wav",
        &sweep,
        &mut notes,
        "The germanium fuzz at full Fuzz, one ringing note five times: Bias 100 % (fresh \
         battery), 60 %, 35 %, 20 %, 10 %. Listen to the end of each note: fresh, it holds and \
         fades; starved, it sputters and gates.",
    );
    let sy = synth();
    let mut both = sy.clone();
    let s = board(&[OpAmp], &[(OpAmp, [0.6, 0.45, 0.5])]);
    let y = through(&s, &sy);
    mix(&mut both, &y, 3.2, 1.0);
    write(
        &dir,
        "08-saw-lead-dry-then-op-amp-distortion.wav",
        &both,
        &mut notes,
        "A saw lead, dry, then through the op-amp distortion (Distortion 60 %, Filter 45 %).",
    );
    std::fs::write(dir.join("README.txt"), notes).expect("readme");
}

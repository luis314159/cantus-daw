//! r42 fiddle — the fiddle lab (ADR 0164): three short phrases, before and after, for the
//! owner's ears and the study's measures (`tools/arcus/fiddle/fiddle_measure.py`):
//!
//! - **dobles**: a bluegrass double-stop lick (thirds and sixths against the open E and A);
//! - **rapidas**: a fast run at 160 BPM crossing strings (two slurred, two separate);
//! - **bordon**: a tune on the E string over the open A, re-bowed together.
//!
//! *Before* is the fiddle as round 42's `arcus-expr` left it: the *Fiddle* preset's numbers,
//! the lower note of each double stop or drone on a second Arcus. *After* is one Arcus with
//! one bow on two strings, the sympathetic strings, the grit and the steel set. 48 kHz 24-bit
//! WAVs, each normalised to the same RMS.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example fiddle_lab -- <out dir> [variant=value ...]
//! ```

use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{Arcus, ArcusSettings, Art, BowDir};
use cantus_dsp::math;
use cantus_dsp::voices::PlayMode;
use std::path::Path;

const SR: f32 = 48_000.0;

/// A note: start (s), length (s), key, velocity, its own articulation and bow, and whether
/// it is the lower note of a double stop or a drone (a second instance before).
#[derive(Clone, Copy)]
struct N {
    at: f64,
    len: f64,
    key: u8,
    vel: f32,
    art: Art,
    dir: BowDir,
    low: bool,
}

/// The *Fiddle* preset's numbers (`arcus.preset.violin_fiddle`) as the DSP reads them.
fn preset() -> ArcusSettings {
    ArcusSettings {
        instrument: Instrument::Violin,
        mode: PlayMode::Legato,
        bow_pos: (-2.0 * math::log2(0.82f32)).clamp(-1.0, 1.0),
        bow_force: math::log2(1.15f32),
        bow_noise_db: 4.0,
        bow_attack: 0.7,
        bow_release_ms: 70.0,
        change_ms: 14.0,
        finger_ms: 14.0,
        vib_depth: 0.35,
        vib_delay_ms: 450.0,
        vib_var: 0.35,
        ring_s: 1.8,
        body_ring: 0.9,
        gain_db: 5.4,
        ..ArcusSettings::default()
    }
}

fn after(args: &[(String, f32)]) -> ArcusSettings {
    let mut s = ArcusSettings {
        pair: 1.0,
        sym: 1.0,
        grit: 1.0,
        steel: 1.0,
        just: 0.6,
        ..preset()
    };
    for (k, v) in args {
        match k.as_str() {
            "sym" => s.sym = *v,
            "grit" => s.grit = *v,
            "steel" => s.steel = *v,
            "just" => s.just = *v,
            "tilt" => s.tilt = *v,
            "noise" => s.bow_noise_db = *v,
            "pos" => s.bow_pos = *v,
            "force" => s.bow_force = *v,
            "ring" => s.ring_s = *v,
            _ => panic!("unknown setting {k}"),
        }
    }
    s
}

fn render(insts: &[(ArcusSettings, Vec<N>)], secs: f64) -> Vec<f32> {
    let frames = (secs * SR as f64) as usize;
    let mut sum = vec![0.0f32; frames];
    for (s, notes) in insts {
        let mut a = Arcus::new(SR);
        a.set(s);
        // Events: (sample, order, what) — note-offs before note-ons at one sample.
        let mut ev: Vec<(usize, u8, usize)> = Vec::new();
        for (i, n) in notes.iter().enumerate() {
            ev.push(((n.at * SR as f64).round() as usize, 1, i));
            ev.push((((n.at + n.len) * SR as f64).round() as usize, 0, i));
        }
        ev.sort();
        let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
        let (mut at, mut i) = (0usize, 0usize);
        while at < frames {
            while i < ev.len() && ev[i].0 <= at {
                let (_, on, k) = ev[i];
                let n = notes[k];
                let id = 1000 + k as u64;
                if on == 1 {
                    a.note_art(id, Some(n.art), Some(n.dir));
                    a.note_on(n.key, n.vel, id);
                } else {
                    a.note_off(n.key, id);
                }
                i += 1;
            }
            let next = ev.get(i).map_or(frames, |e| e.0.min(frames));
            let k = 256.min(next.max(at + 1) - at).min(frames - at);
            a.process(&mut l[at..at + k], &mut r[at..at + k]);
            at += k;
        }
        for (o, x) in sum.iter_mut().zip(&l) {
            *o += x;
        }
    }
    let rms = (sum.iter().map(|x| (x * x) as f64).sum::<f64>() / frames as f64).sqrt();
    let g = (0.1 / rms.max(1e-9)) as f32;
    sum.iter().map(|x| x * g).collect()
}

fn write(path: &Path, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create");
    for s in x {
        w.write_sample((s.clamp(-1.0, 1.0) * 8_388_607.0) as i32)
            .expect("write");
    }
    w.finalize().expect("finalize");
}

/// Two notes struck together (`hi` over `lo`) for `len`, one bow.
fn dstop(v: &mut Vec<N>, at: f64, len: f64, lo: u8, hi: u8, vel: f32, art: Art, dir: BowDir) {
    v.push(N { at, len, key: hi, vel, art, dir, low: false });
    v.push(N { at, len, key: lo, vel, art, dir, low: true });
}

fn flip(d: BowDir) -> BowDir {
    match d {
        BowDir::Down => BowDir::Up,
        BowDir::Up => BowDir::Down,
    }
}

/// The double-stop lick (A major, 120 BPM: an eighth 0.25 s).
fn dobles() -> Vec<N> {
    let mut v = Vec::new();
    let e = 0.25;
    let mut dir = BowDir::Down;
    let mut t = 0.1;
    // (lower, upper, eighths)
    let lick: [(u8, u8, f64); 12] = [
        (73, 76, 1.0), (71, 76, 1.0), (69, 76, 2.0), (69, 78, 1.0), (69, 76, 1.0),
        (68, 76, 1.0), (66, 73, 1.0), (64, 73, 2.0), (69, 73, 1.0), (69, 76, 1.0),
        (68, 71, 1.0), (69, 73, 4.0),
    ];
    for (lo, hi, n) in lick {
        let len = n * e - 0.02;
        dstop(&mut v, t, len, lo, hi, 0.72, Art::Sustain, dir);
        dir = flip(dir);
        t += n * e;
    }
    v
}

/// The fast run (160 BPM: an eighth 0.1875 s), two slurred then two separate.
fn rapidas() -> Vec<N> {
    let mut v = Vec::new();
    let e = 60.0 / 160.0 / 2.0;
    let run: [u8; 32] = [
        81, 76, 73, 76, 69, 73, 76, 81, 80, 76, 71, 76, 68, 71, 76, 80, 81, 78, 74, 78, 69, 74,
        78, 81, 83, 81, 80, 76, 73, 71, 69, 69,
    ];
    let mut dir = BowDir::Down;
    let mut t = 0.1;
    for (i, k) in run.iter().enumerate() {
        let slurred = i % 4 == 1;
        let art = if slurred { Art::Legato } else { Art::Sustain };
        if i % 4 != 1 && i > 0 {
            dir = flip(dir);
        }
        let last = i + 1 == run.len();
        let len = if last { 0.6 } else if i % 4 == 0 { e } else { e - 0.012 };
        let vel = if i % 2 == 0 { 0.74 } else { 0.68 };
        v.push(N { at: t, len, key: *k, vel, art, dir, low: false });
        t += e;
    }
    v
}

/// A tune on the E string over the open A (re-bowed together), 116 BPM.
fn bordon() -> Vec<N> {
    let mut v = Vec::new();
    let e = 60.0 / 116.0 / 2.0;
    let tune: [(u8, f64); 10] = [
        (76, 1.0), (78, 1.0), (80, 2.0), (81, 1.0), (80, 1.0), (78, 1.0), (76, 1.0),
        (78, 2.0), (76, 2.0), (76, 4.0),
    ];
    let mut dir = BowDir::Down;
    let mut t = 0.1;
    for (k, n) in tune {
        dstop(&mut v, t, n * e - 0.02, 69, k, 0.72, Art::Sustain, dir);
        dir = flip(dir);
        t += n * e;
    }
    v
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = Path::new(args.first().expect("an output folder"));
    std::fs::create_dir_all(out).expect("the folder");
    let extra: Vec<(String, f32)> = args[1..]
        .iter()
        .map(|a| {
            let (k, v) = a.split_once('=').expect("key=value");
            (k.to_string(), v.parse().expect("a number"))
        })
        .collect();
    let tag = if extra.is_empty() {
        String::new()
    } else {
        format!(
            "-{}",
            extra
                .iter()
                .map(|(k, v)| format!("{k}{v}"))
                .collect::<Vec<_>>()
                .join("-")
        )
    };
    for (name, notes) in [("dobles", dobles()), ("rapidas", rapidas()), ("bordon", bordon())] {
        let secs = notes.iter().map(|n| n.at + n.len).fold(0.0, f64::max) + 1.5;
        let (hi, lo): (Vec<N>, Vec<N>) = notes.iter().partition(|n| !n.low);
        let before = render(&[(preset(), hi), (preset(), lo)], secs);
        write(&out.join(format!("{name}-antes.wav")), &before);
        let now = render(&[(after(&extra), notes.clone())], secs);
        write(&out.join(format!("{name}-ahora{tag}.wav")), &now);
        println!("{name}: {secs:.1} s");
    }
}

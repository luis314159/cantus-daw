//! Vinculum's listening examples: a drum bus (our own Pulsus kit) and a short mix bus (the
//! CC0 DI guitar of Carbonthief, Pulsus drums, a Gravis bass), each through the three
//! models, gentle and slammed, slammed in parallel, and the VCA's releases side by side.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example vinculum_sounds -- <guitar-wav-dir> <out-dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. Every processed file is level-matched (RMS) to its dry twin
//! afterwards, so what differs is the compression, not the level; the README beside them
//! gives each file's settings, its reduction and the gain the match applied.

use cantus_dsp::exemplar::resample::{Kernel, MAX_TAPS};
use cantus_dsp::gravis::{Engine, Gravis, GravisSettings};
use cantus_dsp::math;
use cantus_dsp::pulsus::{DrumVoice, PadSettings, Pulsus};
use cantus_dsp::vinculum::{Character, Model, Vinculum, VinculumSettings};
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;
const BPM: f32 = 90.0;
const BARS: usize = 8;

type Stereo = (Vec<f32>, Vec<f32>);

fn frames() -> usize {
    (BARS as f32 * 4.0 * 60.0 / BPM * SR) as usize
}

fn peak(s: &Stereo) -> f32 {
    s.0.iter().chain(&s.1).fold(0f32, |m, x| m.max(x.abs()))
}

fn rms(s: &Stereo) -> f32 {
    let e: f64 =
        s.0.iter()
            .chain(&s.1)
            .map(|x| (*x as f64) * (*x as f64))
            .sum();
    (e / (2 * s.0.len()) as f64).sqrt() as f32
}

fn scale(s: &mut Stereo, g: f32) {
    for x in s.0.iter_mut().chain(s.1.iter_mut()) {
        *x *= g;
    }
}

/// The drum loop: kick, snare and eighth-note hats, 90 BPM, 16 steps a bar.
fn drums() -> Stereo {
    let mut p = Pulsus::new(SR);
    p.set_kit(2, 11, 4, false);
    p.set_pad_count(3);
    for (i, (pitch, voice)) in [
        (36u8, DrumVoice::Kick),
        (38, DrumVoice::Snare),
        (42, DrumVoice::HiHat),
    ]
    .iter()
    .enumerate()
    {
        let s = PadSettings {
            id: i as u16 + 1,
            ..PadSettings::of(*voice, 2)
        };
        p.set_pad(i, *pitch, &s);
    }
    let step = SR * 60.0 / BPM / 4.0;
    let mut events = Vec::new();
    for bar in 0..BARS {
        for k in 0..16 {
            let at = ((bar * 16 + k) as f32 * step) as usize;
            if [0, 7, 8, 10].contains(&k) || (bar % 4 == 3 && k == 15) {
                events.push((at, 36u8, if k == 0 { 122u8 } else { 104 }));
            }
            if [4, 12].contains(&k) {
                events.push((at, 38, 118));
            }
            if bar % 4 == 3 && [13, 14].contains(&k) {
                events.push((at, 38, 70 + 20 * (k as u8 - 13)));
            }
            if k % 2 == 0 {
                events.push((at, 42, if k % 4 == 0 { 92 } else { 66 }));
            }
        }
    }
    let n = frames();
    let mut out = (vec![0f32; n], vec![0f32; n]);
    let mut pos = 0;
    for (tag, (at, pitch, vel)) in events.iter().copied().chain([(n, 0, 0)]).enumerate() {
        while pos < at {
            let end = (pos + 256).min(at);
            p.process_add(&mut out.0[pos..end], &mut out.1[pos..end]);
            pos = end;
        }
        if pitch > 0 {
            p.note_on(pitch, vel, tag as u64 + 1);
        }
    }
    out
}

/// The bass: the chords' roots on beats 1 and 3, a synth bass from Gravis.
fn bass() -> Stereo {
    let mut g = Gravis::new(SR);
    g.set(&GravisSettings {
        engine: Engine::Synth,
        ..GravisSettings::default()
    });
    let roots = [33u8, 29, 36, 31]; // A1, F1, C2, G1
    let half = SR * 60.0 / BPM * 2.0;
    let n = frames();
    let mut out = (vec![0f32; n], vec![0f32; n]);
    let mut events = Vec::new();
    for bar in 0..BARS {
        for h in 0..2 {
            let at = ((bar * 2 + h) as f32 * half) as usize;
            let key = roots[bar % 4] + if h == 1 && bar % 2 == 1 { 12 } else { 0 };
            events.push((at, true, key, (bar * 2 + h) as u64));
            events.push((
                at + (half * 0.85) as usize,
                false,
                key,
                (bar * 2 + h) as u64,
            ));
        }
    }
    events.sort_by_key(|e| (e.0, e.1));
    let mut pos = 0;
    for (at, on, key, id) in events.iter().copied().chain([(n, false, 0, u64::MAX)]) {
        while pos < at.min(n) {
            let end = (pos + 256).min(at.min(n));
            g.process(&mut out.0[pos..end], &mut out.1[pos..end]);
            pos = end;
        }
        if id == u64::MAX {
            break;
        }
        if on {
            g.note_on(key, id, 0.8);
        } else {
            g.note_off(key, id);
        }
    }
    out
}

/// A 44.1 kHz (or any rate) float WAV, mono, at 48 kHz through our own sinc kernel.
fn read_resampled(path: &Path) -> Vec<f32> {
    let mut r = hound::WavReader::open(path).expect("guitar wav");
    let spec = r.spec();
    let ch = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.unwrap() as f32 / full)
                .collect()
        }
    };
    let mono: Vec<f32> = raw
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    let ratio = spec.sample_rate as f64 / SR as f64;
    if spec.sample_rate as f32 == SR {
        return mono;
    }
    let k = Kernel::new(32, 8.0);
    let mut c = [0f32; MAX_TAPS];
    let n = (mono.len() as f64 / ratio) as usize;
    (0..n)
        .map(|j| {
            let pos = j as f64 * ratio;
            let i = pos.floor() as isize;
            k.coefs(pos - pos.floor(), &mut c);
            (0..32)
                .map(|t| {
                    let at = i - 15 + t as isize;
                    if at >= 0 && (at as usize) < mono.len() {
                        mono[at as usize] * c[t]
                    } else {
                        0.0
                    }
                })
                .sum()
        })
        .collect()
}

/// The owner's progression: Am – F – C – G, twice, a strum on beats 1 and 3, each faded
/// over 20 ms before the next.
fn guitar(dir: &Path) -> Stereo {
    let files = [
        "Sample07 (A Minor).wav",
        "Sample15 (F Major).wav",
        "Sample22 (C Major).wav",
        "Sample17 (G Major).wav",
    ];
    let chords: Vec<Vec<f32>> = files.iter().map(|f| read_resampled(&dir.join(f))).collect();
    let n = frames();
    let half = (SR * 60.0 / BPM * 2.0) as usize;
    let fade = (0.02 * SR) as usize;
    let mut m = vec![0f32; n];
    for k in 0..BARS * 2 {
        let at = k * half;
        let src = &chords[(k / 2) % 4];
        let len = half.min(src.len()).min(n - at);
        for i in 0..len {
            let f = if i + fade >= half {
                (half - i) as f32 / fade as f32
            } else {
                1.0
            };
            m[at + i] += src[i] * f;
        }
    }
    (m.clone(), m)
}

struct Take {
    name: &'static str,
    what: String,
    s: VinculumSettings,
}

fn render(dry: &Stereo, s: &VinculumSettings) -> (Stereo, f32, f32) {
    let mut v = Vinculum::new(SR);
    v.set(s);
    let mut out = dry.clone();
    let (mut sum, mut count, mut top) = (0f64, 0usize, 0f32);
    for (a, b) in out.0.chunks_mut(480).zip(out.1.chunks_mut(480)) {
        v.process(a, b);
        let m = v.take_meter();
        sum += m.gr_peak_db as f64;
        count += 1;
        top = top.max(m.gr_peak_db);
    }
    (out, (sum / count as f64) as f32, top)
}

fn write(dir: &Path, name: &str, s: &Stereo) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(dir.join(name), spec).expect("wav");
    for (l, r) in s.0.iter().zip(&s.1) {
        for x in [l, r] {
            w.write_sample((x.clamp(-1.0, 1.0) * 8_388_607.0) as i32)
                .unwrap();
        }
    }
    w.finalize().unwrap();
    println!("{}", dir.join(name).display());
}

fn vca(thr: f32, ratio: f32, attack: f32, release: f32, auto: bool) -> VinculumSettings {
    VinculumSettings {
        model: Model::Vca,
        threshold_db: thr,
        ratio,
        attack_ms: attack,
        release_ms: release,
        auto_release: auto,
        ..VinculumSettings::default()
    }
}

fn opto(thr: f32, character: Character) -> VinculumSettings {
    VinculumSettings {
        model: Model::Opto,
        threshold_db: thr,
        character,
        ..VinculumSettings::default()
    }
}

fn mu(thr: f32, character: Character, time: usize, colour: f32) -> VinculumSettings {
    VinculumSettings {
        model: Model::VariMu,
        threshold_db: thr,
        character,
        mu_time: time,
        colour,
        ..VinculumSettings::default()
    }
}

fn takes(
    prefix: &'static [&'static str; 12],
    gentle: [VinculumSettings; 3],
    slam: [VinculumSettings; 3],
) -> Vec<Take> {
    let names = ["VCA", "Opto", "Vari-mu"];
    let mut out = Vec::new();
    for m in 0..3 {
        out.push(Take {
            name: prefix[m * 3],
            what: format!("{} gentle", names[m]),
            s: gentle[m],
        });
        out.push(Take {
            name: prefix[m * 3 + 1],
            what: format!("{} slammed", names[m]),
            s: slam[m],
        });
        // Parallel: the slammed setting, its make-up filled in from the render, at 50 %.
        out.push(Take {
            name: prefix[m * 3 + 2],
            what: format!("{} slammed, in parallel (mix 50 %)", names[m]),
            s: VinculumSettings {
                mix: 0.5,
                ..slam[m]
            },
        });
    }
    out
}

fn describe(s: &VinculumSettings) -> String {
    let base = match s.model {
        Model::Vca => {
            let rel = if s.auto_release {
                "Auto".to_string()
            } else {
                format!("{} s", s.release_ms / 1000.0)
            };
            format!(
                "threshold {} dB, ratio {}:1, attack {} ms, release {rel}",
                s.threshold_db, s.ratio, s.attack_ms
            )
        }
        Model::Opto => format!("threshold {} dB, {:?}", s.threshold_db, s.character),
        Model::VariMu => format!(
            "threshold {} dB, {:?}, time position {}, valve colour {:.0} %",
            s.threshold_db,
            s.character,
            s.mu_time + 1,
            s.colour * 100.0
        ),
    };
    format!(
        "{base}, make-up {} dB, mix {:.0} %",
        s.makeup_db,
        s.mix * 100.0
    )
}

fn bus(dir: &Path, notes: &mut String, title: &str, dry_name: &str, dry: &Stereo, list: Vec<Take>) {
    notes.push_str(&format!("## {title}\n\n`{dry_name}` — the dry twin.\n\n"));
    notes.push_str("| File | What | Settings | Reduction (mean of 10 ms peaks / deepest) | Level match |\n|---|---|---|---|---|\n");
    write(dir, dry_name, dry);
    let target = rms(dry);
    let mut slam_gr = 0.0;
    for mut t in list {
        if t.s.mix < 1.0 {
            t.s.makeup_db = slam_gr;
        }
        let (mut out, mean, top) = render(dry, &t.s);
        if t.s.mix == 1.0 && t.what.ends_with("slammed") {
            slam_gr = mean.round();
        }
        let g = target / rms(&out).max(1e-9);
        scale(&mut out, g);
        let p = peak(&out);
        if p > 0.99 {
            scale(&mut out, 0.99 / p);
        }
        write(dir, t.name, &out);
        let off = 20.0 * math::log10(rms(&out) / target);
        notes.push_str(&format!(
            "| `{}` | {} | {} | {mean:.1} / {top:.1} dB | {:+.1} dB{} |\n",
            t.name,
            t.what,
            describe(&t.s),
            20.0 * math::log10(g),
            if p > 0.99 {
                format!(", then lowered to keep the peak under 0 dBFS: {off:+.1} dB against the dry twin")
            } else {
                String::new()
            }
        ));
    }
    notes.push('\n');
}

fn main() {
    let mut args = std::env::args().skip(1);
    let guitar_dir = PathBuf::from(args.next().expect("guitar wav dir"));
    let dir = PathBuf::from(args.next().expect("out dir"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut notes = String::new();

    // The drum bus, peaking at −6 dBFS.
    let mut d = drums();
    let pd = peak(&d);
    scale(&mut d, 0.5 / pd);
    use Character::{Compress, Limit};
    let drum_takes = takes(
        &[
            "01-drums-vca-gentle.wav",
            "02-drums-vca-slammed.wav",
            "03-drums-vca-parallel.wav",
            "04-drums-opto-gentle.wav",
            "05-drums-opto-slammed.wav",
            "06-drums-opto-parallel.wav",
            "07-drums-varimu-gentle.wav",
            "08-drums-varimu-slammed.wav",
            "09-drums-varimu-parallel.wav",
            "",
            "",
            "",
        ],
        [
            vca(-20.0, 4.0, 10.0, 300.0, true),
            opto(-24.0, Compress),
            mu(-20.0, Compress, 1, 0.4),
        ],
        [
            vca(-32.0, 10.0, 0.3, 100.0, false),
            opto(-32.0, Limit),
            mu(-32.0, Limit, 0, 1.0),
        ],
    );
    bus(
        &dir,
        &mut notes,
        "Drum bus (Pulsus, 90 BPM)",
        "00-drums-dry.wav",
        &d,
        drum_takes,
    );
    let releases = vec![
        Take {
            name: "10-drums-vca-release-0.1s.wav",
            what: "VCA, release 0.1 s".into(),
            s: vca(-24.0, 4.0, 3.0, 100.0, false),
        },
        Take {
            name: "11-drums-vca-release-1.2s.wav",
            what: "VCA, release 1.2 s".into(),
            s: vca(-24.0, 4.0, 3.0, 1200.0, false),
        },
        Take {
            name: "12-drums-vca-release-auto.wav",
            what: "VCA, release Auto".into(),
            s: vca(-24.0, 4.0, 3.0, 300.0, true),
        },
    ];
    bus(
        &dir,
        &mut notes,
        "Drum bus: the VCA's releases",
        "00-drums-dry.wav",
        &d,
        releases,
    );

    // The mix bus: guitar, drums and bass, peaking at −6 dBFS.
    let g = guitar(&guitar_dir);
    let b = bass();
    let gp = peak(&g).max(1e-9);
    let bp = peak(&b).max(1e-9);
    let mut m = (vec![0f32; frames()], vec![0f32; frames()]);
    for i in 0..frames() {
        let gt = g.0[i] / gp * 0.55;
        m.0[i] = gt + d.0[i] * 0.8 + b.0[i] / bp * 0.45;
        m.1[i] = gt + d.1[i] * 0.8 + b.1[i] / bp * 0.45;
    }
    let pm = peak(&m);
    scale(&mut m, 0.5 / pm);
    let mix_takes = takes(
        &[
            "21-mix-vca-gentle.wav",
            "22-mix-vca-slammed.wav",
            "23-mix-vca-parallel.wav",
            "24-mix-opto-gentle.wav",
            "25-mix-opto-slammed.wav",
            "26-mix-opto-parallel.wav",
            "27-mix-varimu-gentle.wav",
            "28-mix-varimu-slammed.wav",
            "29-mix-varimu-parallel.wav",
            "",
            "",
            "",
        ],
        [
            vca(-24.0, 2.0, 30.0, 300.0, true),
            opto(-26.0, Compress),
            mu(-22.0, Compress, 1, 0.4),
        ],
        [
            vca(-28.0, 10.0, 1.0, 100.0, false),
            opto(-28.0, Limit),
            mu(-28.0, Limit, 0, 1.0),
        ],
    );
    bus(
        &dir,
        &mut notes,
        "Mix bus (guitar, drums, bass)",
        "20-mix-dry.wav",
        &m,
        mix_takes,
    );
    memory(&dir, &mut notes, &m);
    std::fs::write(dir.join("table.md"), notes).unwrap();
}

/// fx-b:vinculum (opto, 2026-11-17) — the photocell's memory: the same quiet phrase (the
/// mix's first two bars, 10 dB down) after two seconds of silence, and again straight after
/// twelve loud seconds of the mix (6 dB up). The opto (Compress) is level-matched on the first
/// phrase, so the second one is heard against it: it comes in held down and recovers over
/// seconds. The dry twin has the same timeline.
fn memory(dir: &Path, notes: &mut String, m: &Stereo) {
    let bar = (SR * 60.0 / BPM * 4.0) as usize;
    let (phrase, loud) = (2 * bar, (12.0 * SR) as usize);
    let fade = (0.01 * SR) as usize;
    let quiet = math::db_to_gain(-10.0);
    let mut dry: Stereo = (Vec::new(), Vec::new());
    let piece = |dry: &mut Stereo, len: usize, gain: f32| {
        for i in 0..len {
            let f = (i.min(len - 1 - i) as f32 / fade as f32).min(1.0) * gain;
            dry.0.push(m.0[i] * f);
            dry.1.push(m.1[i] * f);
        }
    };
    piece(&mut dry, (2.0 * SR) as usize, 0.0);
    let a = (dry.0.len(), dry.0.len() + phrase);
    piece(&mut dry, phrase, quiet);
    piece(&mut dry, loud, 2.0);
    let b = (dry.0.len(), dry.0.len() + phrase);
    piece(&mut dry, phrase, quiet);
    piece(&mut dry, (3.0 * SR) as usize, 0.0);
    let s = opto(-34.0, Character::Compress);
    let mut v = Vinculum::new(SR);
    v.set(&s);
    let mut out = dry.clone();
    let mut gr = Vec::new();
    for (x, y) in out.0.chunks_mut(480).zip(out.1.chunks_mut(480)) {
        v.process(x, y);
        gr.push(v.take_meter().gr_db);
    }
    let sec = SR as usize;
    let mean = |from: usize, to: usize| {
        let (i, j) = (from / 480, to / 480);
        gr[i..j].iter().sum::<f32>() / (j - i) as f32
    };
    let seg = |x: &Stereo, r: (usize, usize)| -> Stereo {
        (x.0[r.0..r.1].to_vec(), x.1[r.0..r.1].to_vec())
    };
    let g = rms(&seg(&dry, a)) / rms(&seg(&out, a)).max(1e-9);
    scale(&mut out, g);
    let p = peak(&out);
    if p > 0.99 {
        scale(&mut out, 0.99 / p);
    }
    write(dir, "30-mix-opto-memory-dry.wav", &dry);
    write(dir, "31-mix-opto-memory.wav", &out);
    // Second by second: the phrase's reduction after silence and after the passage.
    let per_sec = |r: (usize, usize)| -> Vec<String> {
        (0..phrase / sec)
            .map(|k| format!("{:.1}", mean(r.0 + k * sec, r.0 + (k + 1) * sec)))
            .collect()
    };
    notes.push_str("## The opto's memory\n\n");
    notes.push_str(
        "`30-mix-opto-memory-dry.wav` — the dry twin: 2 s of silence, the mix's first two \
         bars 10 dB down (the phrase), twelve loud seconds of the mix (6 dB up), the phrase \
         again, 3 s of silence.\n\n",
    );
    notes.push_str(&format!(
        "`31-mix-opto-memory.wav` — Opto, {}; level-matched on the first phrase ({:+.1} dB). \
         The loud passage: {:.1} dB on average. The phrase's reduction, second by second \
         (dB): after silence {}; after the loud passage {}.\n\n",
        describe(&s),
        20.0 * math::log10(g),
        mean(a.1, b.0),
        per_sec(a).join(", "),
        per_sec(b).join(", "),
    ));
}

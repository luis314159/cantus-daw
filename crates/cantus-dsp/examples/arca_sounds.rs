//! Arca's, Rota's and the Uni-Vibe's listening examples, DSP only (no engine): the owner's
//! reference progression on a real DI guitar (Carbonthief's CC0 strums, decoded to WAV
//! beforehand: Cantus's FLAC reader does not read their 32-bit files) through each amp,
//! clean and driven, with and without its cabinet; the rotary speaker on the guitar and
//! on Organum's dry organ; the Uni-Vibe.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arca_sounds -- <guitar-wav-dir> <out-dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. Every processed file is level-matched (RMS) to its dry twin
//! unless its README line says otherwise.

use cantus_dsp::arca::cab::{impulse, CabSettings, Cabinet, Mic};
use cantus_dsp::arca::{AmpModel, Arca, ArcaSettings};
use cantus_dsp::aula_kernel::{Format, Kernel, Layout};
use cantus_dsp::exemplar::resample::{Kernel as Sinc, MAX_TAPS};
use cantus_dsp::organum::rotary::RotorSpeed;
use cantus_dsp::organum::{OrganSettings, Organum};
use cantus_dsp::phasis::{Phasis, PhasisSettings, PhasisStyle};
use cantus_dsp::rota::{Rota, RotaSettings};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SR: f32 = 48_000.0;
const BPM: f32 = 90.0;

fn read_mono(p: &Path) -> (Vec<f32>, u32) {
    let mut r = hound::WavReader::open(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
    let spec = r.spec();
    let ch = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
    };
    let mono = raw
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    (mono, spec.sample_rate)
}

/// To 48 kHz with Exemplar's windowed-sinc kernel.
fn to_48k(x: &[f32], from: u32) -> Vec<f32> {
    if from == 48_000 {
        return x.to_vec();
    }
    let k = Sinc::new(32, 9.0);
    let step = from as f64 / 48_000.0;
    let n = (x.len() as f64 / step) as usize;
    let mut c = [0.0f32; MAX_TAPS];
    (0..n)
        .map(|j| {
            let pos = j as f64 * step;
            let i = pos.floor() as isize;
            k.coefs(pos - i as f64, &mut c);
            let mut acc = 0.0f32;
            for (t, ct) in c.iter().enumerate().take(32) {
                let idx = i - 15 + t as isize;
                if idx >= 0 && (idx as usize) < x.len() {
                    acc += x[idx as usize] * ct;
                }
            }
            acc
        })
        .collect()
}

/// The reference progression: Am F C G twice, a strum on beats 1 and 3, each faded out
/// over 20 ms before the next.
fn progression(dir: &Path) -> Vec<f32> {
    let names = [
        "Sample07 (A Minor).wav",
        "Sample15 (F Major).wav",
        "Sample22 (C Major).wav",
        "Sample17 (G Major).wav",
    ];
    let chords: Vec<Vec<f32>> = names
        .iter()
        .map(|n| {
            let (x, sr) = read_mono(&dir.join(n));
            to_48k(&x, sr)
        })
        .collect();
    let beat = (60.0 / BPM * SR) as usize;
    let total = 8 * 4 * beat + SR as usize;
    let mut out = vec![0.0f32; total];
    let fade = (0.02 * SR) as usize;
    for bar in 0..8 {
        for half in 0..2 {
            let start = (bar * 4 + half * 2) * beat;
            let len = 2 * beat;
            let c = &chords[bar % 4];
            for i in 0..len.min(c.len()) {
                let g = if i + fade >= len {
                    (len - i) as f32 / fade as f32
                } else {
                    1.0
                };
                out[start + i] += c[i] * g;
            }
        }
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let g = 0.501 / peak; // −6 dBFS
    out.iter().map(|v| v * g).collect()
}

/// Organum's organ, its own rotary off: the progression's chords held, a bar each.
fn organ() -> Vec<f32> {
    let mut o = Organum::new(SR);
    o.set(&OrganSettings {
        rotary_on: false,
        ..OrganSettings::default()
    });
    let chords: [[u8; 4]; 4] = [
        [57, 60, 64, 69],
        [53, 57, 60, 65],
        [48, 55, 60, 64],
        [55, 59, 62, 67],
    ];
    let bar = (4.0 * 60.0 / BPM * SR) as usize;
    let mut l = vec![0.0f32; 8 * bar + SR as usize];
    let mut r = l.clone();
    let mut id = 0u64;
    for b in 0..8 {
        let c = chords[b % 4];
        let ids: Vec<u64> = c
            .iter()
            .map(|&k| {
                id += 1;
                o.note_on(k, id, 0);
                id
            })
            .collect();
        let s = b * bar;
        o.process(&mut l[s..s + bar - 2_400], &mut r[s..s + bar - 2_400]);
        for i in ids {
            o.note_off(i);
        }
        o.process(
            &mut l[s + bar - 2_400..s + bar],
            &mut r[s + bar - 2_400..s + bar],
        );
    }
    let n = l.len();
    o.process(&mut l[8 * bar..n], &mut r[8 * bar..n]);
    let mono: Vec<f32> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
    let peak = mono.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    mono.iter().map(|v| v * 0.5 / peak).collect()
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt()
}

fn write(path: &Path, l: &[f32], r: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (a, b) in l.iter().zip(r) {
        for v in [a, b] {
            w.write_sample((v.clamp(-1.0, 1.0) * 8_388_607.0) as i32)
                .unwrap();
        }
    }
    w.finalize().unwrap();
}

/// Level-match `y` (stereo) to `dry`'s RMS; if that would clip, scale to −1 dBFS peak and
/// say so. Returns the note for the README.
fn matched(dry: &[f32], l: &mut [f32], r: &mut [f32]) -> String {
    let target = rms(dry);
    let now = 0.5 * (rms(l) + rms(r));
    let mut g = target / now.max(1e-9);
    let peak = l.iter().chain(r.iter()).fold(0.0f32, |m, v| m.max(v.abs())) * g;
    let mut note = String::new();
    if peak > 0.891 {
        let back = 0.891 / peak;
        g *= back;
        let _ = write!(
            note,
            " (peaks held at −1 dBFS: {:.1} dB under the dry's level)",
            -20.0 * cantus_dsp::math::log10(back)
        );
    }
    for v in l.iter_mut().chain(r.iter_mut()) {
        *v *= g;
    }
    note
}

fn cab_for(m: AmpModel) -> Cabinet {
    match m {
        AmpModel::Clean => Cabinet::OneTwelveOpen,
        AmpModel::Crunch => Cabinet::FourTwelveVintage,
        AmpModel::Chime => Cabinet::TwoTwelveOpen,
        AmpModel::HighGain => Cabinet::FourTwelveModern,
        AmpModel::Bass => Cabinet::FourTenBass,
    }
}

fn name_of(m: AmpModel) -> &'static str {
    match m {
        AmpModel::Clean => "clean",
        AmpModel::Crunch => "crunch",
        AmpModel::Chime => "chime",
        AmpModel::HighGain => "high-gain",
        AmpModel::Bass => "bass",
    }
}

/// `(gain, master)` for the model played clean and driven.
fn voicing(m: AmpModel) -> [(f32, f32); 2] {
    match m {
        AmpModel::Clean => [(0.25, 1.0), (0.85, 1.0)],
        AmpModel::Crunch => [(0.15, 0.45), (0.7, 1.0)],
        AmpModel::Chime => [(0.25, 0.6), (0.8, 1.0)],
        AmpModel::HighGain => [(0.12, 0.5), (0.75, 0.7)],
        AmpModel::Bass => [(0.3, 1.0), (0.8, 1.0)],
    }
}

fn kernel(c: Cabinet) -> Arc<Kernel> {
    let h = impulse(
        &CabSettings {
            cabinet: c,
            mic: Mic::Dynamic,
            position: 0.3,
            distance_cm: 3.0,
        },
        SR,
    );
    Arc::new(Kernel::new(
        Format::Mono,
        std::slice::from_ref(&h),
        Layout::for_taps(h.len()),
    ))
}

fn arca(x: &[f32], s: &ArcaSettings, cab: Option<Cabinet>) -> (Vec<f32>, Vec<f32>) {
    let mut a = match cab {
        Some(c) => Arca::with_cab(SR, kernel(c)),
        None => Arca::new(SR),
    };
    a.set(s);
    a.reset();
    let (mut l, mut r) = (x.to_vec(), x.to_vec());
    for (bl, br) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        a.process(bl, br);
    }
    // The latency, removed (as the engine's delay compensation does).
    let lat = a.latency();
    l.drain(..lat);
    r.drain(..lat);
    l.extend(std::iter::repeat_n(0.0, lat));
    r.extend(std::iter::repeat_n(0.0, lat));
    (l, r)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).expect("the guitar WAVs' folder"));
    let out = PathBuf::from(args.get(2).expect("an output folder"));
    std::fs::create_dir_all(&out).unwrap();
    let mut readme = String::from(
        "Arca (amp and cabinet), Rota (the rotary speaker) and Phasis's Uni-Vibe style - phase A, DSP only.\n\
         Guitar: Carbonthief, CC0 (DI chord strums, the Renoise forum's library; decoded to WAV,\n\
         resampled 44.1 -> 48 kHz with Exemplar's sinc kernel). The owner's reference progression:\n\
         Am - F - C - G, 90 BPM, 8 bars, a strum on beats 1 and 3. 48 kHz, 24-bit WAV. Each processed\n\
         file is level-matched (RMS) to its dry twin (00 for the guitar, 30 for the organ) unless said.\n\
         Every cabinet is one of Arca's own generated responses (no recording), a dynamic mic\n\
         30 % off the cap, 3 cm from the grille. Oversampling 4x (the default).\n\n",
    );
    let di = progression(&dir);
    write(&out.join("00-guitar-di.wav"), &di, &di);
    readme +=
        "00-guitar-di.wav\n  The guitar, dry (peak -6 dBFS): the twin of every guitar file.\n\n";
    let mut k = 1;
    for m in AmpModel::ALL {
        for (which, (gain, master)) in ["clean", "driven"].iter().zip(voicing(m)) {
            for cab in [false, true] {
                let s = ArcaSettings {
                    model: Some(m),
                    gain,
                    master,
                    cab,
                    ..ArcaSettings::default()
                };
                let (mut l, mut r) = arca(&di, &s, cab.then(|| cab_for(m)));
                let note = matched(&di, &mut l, &mut r);
                let file = format!(
                    "{k:02}-{}-{which}-{}.wav",
                    name_of(m),
                    if cab { "with-cabinet" } else { "amp-only" }
                );
                write(&out.join(&file), &l, &r);
                let _ = writeln!(
                    readme,
                    "{file}\n  The {} amp {which}: gain {:.0} %, master {:.0} %, tone at noon{}{}.\n",
                    name_of(m),
                    gain * 100.0,
                    master * 100.0,
                    if cab {
                        format!(", into its cabinet ({:?})", cab_for(m))
                    } else {
                        ", no cabinet (the amp's speaker output: harsh by nature)".into()
                    },
                    note
                );
                println!("{file}");
                k += 1;
            }
        }
    }
    // The rotary on the guitar: slow, fast, and the switch thrown.
    let rota = |x: &[f32], speed: RotorSpeed, drive: f32, switch: bool| {
        let mut r = Rota::new(SR);
        r.set(&RotaSettings {
            speed,
            drive,
            mix: 1.0,
            ..RotaSettings::default()
        });
        r.reset();
        let (mut l, mut rr) = (x.to_vec(), x.to_vec());
        let bar = (4.0 * 60.0 / BPM * SR) as usize;
        let mut i = 0;
        while i < l.len() {
            let end = (i + 256).min(l.len());
            if switch && (i / bar) != (end / bar) && [2, 4, 6].contains(&(end / bar)) {
                r.toggle();
            }
            r.process(&mut l[i..end], &mut rr[i..end]);
            i = end;
        }
        (l, rr)
    };
    let og = organ();
    write(&out.join("30-organ-dry.wav"), &og, &og);
    readme += "30-organ-dry.wav\n  Organum's organ (drawbars 888000000, its own rotary off), the progression's chords held: the twin of the organ files.\n\n";
    type Job<'a> = (&'a str, &'a [f32], RotorSpeed, f32, bool, &'a str);
    let jobs: [Job; 6] = [
        ("21-rotary-guitar-slow.wav", &di, RotorSpeed::Slow, 0.0, false, "Rota on the guitar, slow (chorale: horn 40, drum 36 rpm)."),
        ("22-rotary-guitar-fast.wav", &di, RotorSpeed::Fast, 0.0, false, "Rota on the guitar, fast (tremolo: horn 423, drum 357 rpm)."),
        ("23-rotary-guitar-switching.wav", &di, RotorSpeed::Slow, 0.2, true, "Rota on the guitar, the switch thrown at bars 3, 5 and 7 (slow, fast, slow, fast): listen to the horn catch up in a fraction of a second and the drum take seconds. Drive 20 %."),
        ("31-rotary-organ-slow.wav", &og, RotorSpeed::Slow, 0.0, false, "Rota on the organ, slow."),
        ("32-rotary-organ-fast.wav", &og, RotorSpeed::Fast, 0.0, false, "Rota on the organ, fast."),
        ("33-rotary-organ-switching-driven.wav", &og, RotorSpeed::Slow, 0.5, true, "Rota on the organ, the switch thrown at bars 3, 5 and 7, its amp driven (50 %)."),
    ];
    for (file, x, speed, drive, switch, what) in jobs {
        let (mut l, mut r) = rota(x, speed, drive, switch);
        let note = matched(x, &mut l, &mut r);
        write(&out.join(file), &l, &r);
        let _ = writeln!(readme, "{file}\n  {what}{note}\n");
        println!("{file}");
    }
    // The Uni-Vibe.
    for (file, mix, rate, what) in [
        ("41-univibe-chorus.wav", 0.5, 2.0, "Phasis's Uni-Vibe style, chorus (dry and wet equal), 2 Hz, intensity 100 %."),
        ("42-univibe-vibrato.wav", 1.0, 5.0, "Phasis's Uni-Vibe style, vibrato (wet only), 5 Hz, intensity 100 %."),
        ("43-univibe-chorus-slow.wav", 0.5, 0.8, "Phasis's Uni-Vibe style, chorus, 0.8 Hz: the slow throb, the sweep rising quickly and falling slowly."),
    ] {
        let mut p = Phasis::new(SR);
        let mut s = PhasisSettings {
            style: PhasisStyle::Vibe,
            mix,
            intensity: 1.0,
            ..PhasisSettings::default()
        };
        s.lfo.rate = cantus_dsp::modfx_lfo::Rate::Hz(rate);
        p.set(&s);
        p.reset();
        let (mut l, mut r) = (di.clone(), di.clone());
        for (bl, br) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            p.process(bl, br);
        }
        let note = matched(&di, &mut l, &mut r);
        write(&out.join(file), &l, &r);
        let _ = writeln!(readme, "{file}\n  {what}{note}\n");
        println!("{file}");
    }
    std::fs::write(out.join("README.txt"), readme).unwrap();
}

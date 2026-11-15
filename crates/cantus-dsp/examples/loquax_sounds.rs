//! Loquax's listening examples and its CPU measure, through the DSP alone (no engine).
//!
//! ```text
//! cargo run -p cantus-dsp --release --example loquax_sounds -- <voice.wav> <out-dir>
//! cargo run -p cantus-dsp --release --example loquax_sounds -- --cpu
//! ```
//!
//! `<voice.wav>`: a spoken voice, mono or stereo, 48 kHz (the README beside the files says
//! which recording and its licence; no recording is in the repository). The sung voice is
//! our own: [`cantus_dsp::pitch::testkit`]'s glottal pulse through formants, singing a
//! melody on five vowels. The carrier is Loquax's own synthesiser playing a progression
//! (Am, F, C, G at 90 BPM). 48 kHz, 24-bit WAV, stereo; each file made up to the dry
//! voice's loudness (RMS), the make-up said in the README.

use cantus_dsp::loquax::carrier::SynthParams;
use cantus_dsp::loquax::measure::{self, ncm, rms_db, spectral_correlation};
use cantus_dsp::loquax::{Loquax, LoquaxSettings, NoteEvent, NoteKind};
use cantus_dsp::math;
use cantus_dsp::pitch::testkit::{vibrato, voice, Vowel};
use std::path::Path;
use std::time::Instant;

const SR: f32 = 48_000.0;
const BAR: f32 = 4.0 * 60.0 / 90.0;

/// The progression, bar by bar, from `t0` seconds, for `bars` bars: (keys) per bar.
fn progression(t0: f32, bars: usize, octave_up: bool) -> Vec<NoteEvent> {
    let chords: [&[u8]; 4] = [
        &[45, 57, 60, 64],
        &[41, 57, 60, 65],
        &[48, 55, 60, 64],
        &[43, 55, 59, 62],
    ];
    let lift = if octave_up { 12 } else { 0 };
    let mut ev = Vec::new();
    for b in 0..bars {
        let at = ((t0 + b as f32 * BAR) * SR) as u32;
        if b > 0 {
            ev.push(NoteEvent {
                frame: at.saturating_sub(1),
                kind: NoteKind::AllOff,
            });
        }
        for &k in chords[b % 4] {
            ev.push(NoteEvent {
                frame: at,
                kind: NoteKind::On {
                    key: k + lift,
                    velocity: 0.8,
                },
            });
        }
    }
    ev
}

/// `l`/`r` through `q` in 256-frame blocks with `notes` (absolute frames); `change` is
/// called before each block with its start frame, to move settings mid-render.
fn render(
    q: &mut Loquax,
    l: &mut [f32],
    r: &mut [f32],
    notes: &[NoteEvent],
    mut change: impl FnMut(&mut Loquax, usize),
) {
    let n = l.len();
    let mut at = 0usize;
    let mut ev = Vec::new();
    while at < n {
        let len = 256.min(n - at);
        change(q, at);
        ev.clear();
        for e in notes {
            let f = e.frame as usize;
            if f >= at && f < at + len {
                ev.push(NoteEvent {
                    frame: (f - at) as u32,
                    kind: e.kind,
                });
            }
        }
        q.process(&mut l[at..at + len], &mut r[at..at + len], None, &ev);
        at += len;
    }
}

fn read_voice(path: &Path, seconds: f32) -> Vec<f32> {
    let mut rd = hound::WavReader::open(path).expect("the voice WAV");
    let spec = rd.spec();
    assert_eq!(spec.sample_rate, 48_000, "the voice must be 48 kHz");
    let ch = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => rd.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            rd.samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
    };
    let mono: Vec<f32> = raw
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .take((seconds * SR) as usize)
        .collect();
    mono
}

/// A sung melody on five vowels (our own synthetic singer), `bars` bars at 90 BPM.
fn sung(bars: usize) -> Vec<f32> {
    let notes: [(Vowel, f32, f32); 8] = [
        (Vowel::A, 57.0, 2.0),
        (Vowel::I, 60.0, 1.0),
        (Vowel::U, 64.0, 1.0),
        (measure::E, 65.0, 2.0),
        (measure::O, 64.0, 2.0),
        (Vowel::A, 62.0, 1.5),
        (Vowel::I, 60.0, 0.5),
        (Vowel::U, 59.0, 2.0),
    ];
    let beat = BAR / 4.0;
    let mut out = Vec::new();
    let mut i = 0usize;
    while out.len() < (bars as f32 * BAR * SR) as usize {
        let (v, key, beats) = notes[i % notes.len()];
        let hz = 440.0 * math::exp2((key - 12.0 - 69.0) / 12.0);
        let mut seg = voice(SR, beats * beat, v, 0.5, vibrato(hz, 25.0, 5.2));
        let fade = (0.03 * SR) as usize;
        let len = seg.len();
        for (j, x) in seg.iter_mut().enumerate() {
            let g = (j.min(len - 1 - j) as f32 / fade as f32).min(1.0);
            *x *= g;
        }
        out.extend(seg);
        i += 1;
    }
    out
}

struct Out<'a> {
    dir: &'a Path,
    dry_rms: f32,
    notes: String,
}

impl Out<'_> {
    fn write(&mut self, name: &str, l: &[f32], r: &[f32], what: &str, reference: Option<&[f32]>) {
        let level = 0.5 * (rms_db(l) + rms_db(r));
        let mut make = self.dry_rms - level;
        let peak = l.iter().chain(r).fold(0.0f32, |m, x| m.max(x.abs()));
        // Never past −1 dBFS.
        let room = -1.0 - 20.0 * math::log10(peak.max(1e-9));
        if make > room {
            make = room;
        }
        let g = math::db_to_gain(make);
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(self.dir.join(name), spec).unwrap();
        for (a, b) in l.iter().zip(r) {
            for x in [a, b] {
                let v = (x * g).clamp(-1.0, 1.0) * 8_388_607.0;
                w.write_sample(v as i32).unwrap();
            }
        }
        w.finalize().unwrap();
        let score = reference
            .map(|v| {
                format!(
                    " NCM {:.2}, spectral-envelope r {:.2}.",
                    ncm(v, l, SR),
                    spectral_correlation(v, l, SR)
                )
            })
            .unwrap_or_default();
        println!("{name}: made up {make:+.1} dB.{score}");
        self.notes += &format!("- `{name}` — {what} (made up {make:+.1} dB).{score}\n");
    }
}

fn through(s: &LoquaxSettings, x: &[f32], notes: &[NoteEvent]) -> (Vec<f32>, Vec<f32>) {
    let mut q = Loquax::new(SR);
    q.set(s);
    q.reset();
    let (mut l, mut r) = (x.to_vec(), x.to_vec());
    render(&mut q, &mut l, &mut r, notes, |_, _| {});
    (l, r)
}

fn clear(bands: usize) -> LoquaxSettings {
    LoquaxSettings {
        mix: 1.0,
        bands,
        ..LoquaxSettings::default()
    }
}

fn choir_synth() -> SynthParams {
    SynthParams {
        saw: 0.8,
        pulse: 0.35,
        width: 0.22,
        noise: 0.06,
        unison: 7,
        detune: 22.0,
        spread: 1.0,
        attack_ms: 60.0,
        release_ms: 400.0,
        ..SynthParams::default()
    }
}

fn cpu() {
    let speech = measure::phrase(SR, 10.0, 0.5);
    let notes = progression(0.0, 4, false);
    println!("Loquax CPU, 10 s of speech and a four-note chord (unison 3), one core, 48 kHz:");
    for &order in &[4usize, 6, 8] {
        for &bands in &[16usize, 32, 64] {
            let s = LoquaxSettings {
                order,
                ..clear(bands)
            };
            let mut best = f64::MAX;
            for _ in 0..3 {
                let t = Instant::now();
                let _ = through(&s, &speech, &notes);
                best = best.min(t.elapsed().as_secs_f64());
            }
            println!(
                "  order {order}, {bands:2} bands: {:.2} % of real time ({:.1} µs per 64-frame block)",
                100.0 * best / 10.0,
                best / (10.0 * SR as f64 / 64.0) * 1e6
            );
        }
    }
    // The worst case: the formant shift and the range automated every block, so the
    // synthesis bank is redesigned every 32 samples.
    for &bands in &[32usize, 64] {
        let s = LoquaxSettings {
            order: 8,
            ..clear(bands)
        };
        let mut best = f64::MAX;
        for _ in 0..3 {
            let mut q = Loquax::new(SR);
            q.set(&s);
            q.reset();
            let (mut l, mut r) = (speech.clone(), speech.clone());
            let t = Instant::now();
            render(&mut q, &mut l, &mut r, &notes, |q, at| {
                let mut s = *q.settings();
                s.formant = 12.0 * math::sin(at as f32 / SR);
                s.lo_hz = 100.0 + 50.0 * math::sin(at as f32 / SR * 0.7);
                q.set(&s);
            });
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!(
            "  order 8, {bands:2} bands, formant and range automated every block: {:.2} % of real time",
            100.0 * best / 10.0
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--cpu") {
        cpu();
        return;
    }
    let (voice_path, dir) = (Path::new(&args[0]), Path::new(&args[1]));
    std::fs::create_dir_all(dir).unwrap();
    let bars = 8;
    let secs = bars as f32 * BAR;
    let spoken = read_voice(voice_path, secs);
    let sung = sung(bars);
    let mut out = Out {
        dir,
        dry_rms: rms_db(&spoken),
        notes: String::new(),
    };
    let pad = progression(0.0, bars, false);
    let pad_up = progression(0.0, bars, true);

    out.write(
        "00-voice-dry.wav",
        &spoken,
        &spoken,
        "the spoken voice, dry",
        None,
    );
    out.write(
        "00-sung-dry.wav",
        &sung,
        &sung,
        "the sung voice (synthetic), dry",
        None,
    );
    // The carrier alone: the synthesiser heard through the carrier pass-through only.
    let carrier_only = LoquaxSettings {
        carrier_pass: 1.0,
        sibilance: 0.0,
        unvoiced: 0.0,
        out_db: 0.0,
        ..clear(32)
    };
    let silent = vec![0.0f32; spoken.len()];
    let (l, r) = through(&carrier_only, &silent, &pad);
    out.write(
        "00-carrier-dry.wav",
        &l,
        &r,
        "the carrier: the synthesiser's default patch playing Am, F, C, G",
        None,
    );

    // 1. The classic robot: 16 bands, order 4, a monotone C3 drone, no notes.
    let mut robot = clear(16);
    robot.order = 4;
    robot.robot = true;
    robot.robot_note = 48.0;
    robot.release_ms = 25.0;
    robot.synth = SynthParams {
        saw: 0.5,
        pulse: 0.8,
        width: 0.4,
        noise: 0.0,
        unison: 1,
        ..SynthParams::default()
    };
    let (l, r) = through(&robot, &spoken, &[]);
    out.write(
        "01-robot-16-bands.wav",
        &l,
        &r,
        "the classic robot: 16 bands, 4th order, one C3 drone, no notes played",
        Some(&spoken),
    );

    // 2. The clear voice: 32 bands, order 6, the progression.
    let s = clear(32);
    let (l, r) = through(&s, &spoken, &pad);
    out.write("02-clear-32-bands.wav", &l, &r, "a clear voice: 32 bands, 6th order, the default patch playing the progression, consonants passed", Some(&spoken));
    let mut bare = s;
    bare.sibilance = 0.0;
    bare.unvoiced = 0.0;
    let (l, r) = through(&bare, &spoken, &pad);
    out.write("02b-clear-no-consonants.wav", &l, &r, "the same with the sibilance pass-through and the unvoiced noise off (compare the s and t sounds)", Some(&spoken));

    // 3. Formant shift.
    let mut up = clear(32);
    up.formant = 5.0;
    let (l, r) = through(&up, &spoken, &pad);
    out.write(
        "03-formant-up-5.wav",
        &l,
        &r,
        "formants up 5 semitones: a smaller throat, the same notes",
        Some(&spoken),
    );
    let mut down = clear(32);
    down.formant = -6.0;
    let (l, r) = through(&down, &spoken, &pad);
    out.write(
        "03b-formant-down-6.wav",
        &l,
        &r,
        "formants down 6 semitones: a giant's throat",
        Some(&spoken),
    );

    // 4. The choir: 40 bands, a seven-voice unison pad an octave up, bands spread.
    let choir = LoquaxSettings {
        spread: 0.5,
        tilt: 1.0,
        attack_ms: 4.0,
        release_ms: 70.0,
        synth: choir_synth(),
        ..clear(40)
    };
    let (l, r) = through(&choir, &spoken, &pad_up);
    out.write("04-choir-spoken.wav", &l, &r, "a choir: 40 bands, seven detuned voices an octave up, the bands spread across the stereo field", Some(&spoken));
    let (l, r) = through(&choir, &sung, &pad);
    out.write(
        "05-choir-sung.wav",
        &l,
        &r,
        "the sung voice through the same choir, on the progression",
        Some(&sung),
    );

    // 6. Hold: the voice speaks; from bar 3 the bands freeze for two bars, a pad of its last
    //    vowel, then speak again.
    let s = clear(32);
    let mut q = Loquax::new(SR);
    q.set(&s);
    q.reset();
    let (mut l, mut r) = (spoken.clone(), spoken.clone());
    let (h0, h1) = ((2.0 * BAR * SR) as usize, (4.0 * BAR * SR) as usize);
    render(&mut q, &mut l, &mut r, &pad, |q, at| {
        let hold = (h0..h1).contains(&at);
        if q.settings().hold != hold {
            let mut s = *q.settings();
            s.hold = hold;
            q.set(&s);
        }
    });
    out.write(
        "06-hold-bars-3-4.wav",
        &l,
        &r,
        "hold: the bands frozen through bars 3 and 4 (the last vowel sustained as a pad)",
        None,
    );

    let readme = format!(
        "# Loquax — sound examples (DSP only)\n\n\
         Rendered by `cargo run -p cantus-dsp --release --example loquax_sounds` through Loquax's \
         DSP alone (no engine), 48 kHz, 24-bit WAV, stereo, eight bars at 90 BPM. Each file is \
         made up to the dry voice's loudness (RMS, never past −1 dBFS). NCM: the normalised \
         covariance measure against the dry voice (0 … 1, an intelligibility proxy); r: the \
         spectral-envelope correlation (see `loquax_measure.rs`).\n\n{}\n\
         ## Sources\n\n\
         - The spoken voice: *Gettysburg Address*, read by Britton Rea (Enfaze Studios), \
         Wikimedia Commons `File:Gettysburg_by_Britton.ogg`, released into the public domain \
         by its author. The first {secs:.0} seconds, mixed to mono and resampled to 48 kHz.\n\
         - The sung voice: synthetic, ours (`cantus_dsp::pitch::testkit`: a Rosenberg glottal \
         pulse through four formants), no recording.\n\
         - The carrier: Loquax's own synthesiser.\n",
        out.notes
    );
    std::fs::write(dir.join("README.md"), readme).unwrap();
}

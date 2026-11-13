//! Fragor on a real guitar: the owner's reference progression (Am – F – C – G, 90 BPM,
//! 8 bars, a strum on beats 1 and 3) built from Carbonthief's CC0 DI chord strums, dry and
//! through the pedals.
//!
//! ```text
//! cargo run -p cantus-ui --release --example fragor_guitar -- <samples dir> <out dir>
//! ```
//!
//! `<samples dir>` holds `Sample07 (A Minor).flac` and the others (never in the
//! repository). Read with the engine's own loader (decoded and resampled to 48 kHz),
//! written as 24-bit FLAC with the export dialog's encoder. The dry file peaks at −6 dBFS;
//! each processed one is level-matched to it (RMS) except where the level is the point.

use cantus_dsp::fragor::{BoardSettings, Fragor, Model};
use std::path::{Path, PathBuf};

const SR: u32 = 48_000;
const BPM: f64 = 90.0;

/// The chord `stem` from `dir`: its FLAC, or a WAV beside it (these CC0 files are 32-bit
/// integer FLAC, which Cantus's reader does not take: decoded to WAV for listening).
fn load(dir: &Path, stem: &str, cache: &mut cantus_engine::samples::SampleCache) -> Vec<f32> {
    let flac = dir.join(format!("{stem}.flac"));
    let path = if flac.exists() {
        flac
    } else {
        dir.join(format!("{stem}.wav"))
    };
    let d = cache
        .load_audio(&path.to_string_lossy(), SR)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let ch = d.channels.max(1) as usize;
    d.frames
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Place `src` at `at` samples into `dst`, faded out over 20 ms before `until`.
fn place(dst: &mut Vec<f32>, src: &[f32], at: usize, until: usize) {
    let fade = (0.02 * SR as f64) as usize;
    let len = src.len().min(until.saturating_sub(at));
    if dst.len() < at + len {
        dst.resize(at + len, 0.0);
    }
    for i in 0..len {
        let g = if i + fade > len {
            (len - i) as f32 / fade as f32
        } else {
            1.0
        };
        dst[at + i] += src[i] * g;
    }
}

fn beats(n: f64) -> usize {
    (n * 60.0 / BPM * SR as f64).round() as usize
}

fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn through(s: &BoardSettings, x: &[f32]) -> Vec<f32> {
    let mut f = Fragor::new(SR as f32);
    f.set(s);
    f.reset();
    let tail = vec![0.0; SR as usize];
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

fn write(path: &Path, v: &[f32]) {
    use cantus_export::settings::{Depth, Dither, Tags};
    let mut samples = Vec::with_capacity(v.len() * 2);
    for x in v {
        samples.push(*x);
        samples.push(*x);
    }
    let audio = cantus_export::Audio {
        rate: SR,
        channels: 2,
        samples,
    };
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).expect("create"));
    let mut progress = |_: f32| true;
    cantus_export::encode::flac(
        &mut out,
        &audio,
        Depth::Int24,
        Dither::Off,
        &Tags::default(),
        "",
        &mut progress,
    )
    .expect("flac");
    use std::io::Write as _;
    out.flush().expect("flush");
    println!("{}", path.display());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let samples = PathBuf::from(args.first().expect("<samples dir> <out dir>"));
    let out = PathBuf::from(args.get(1).expect("<samples dir> <out dir>"));
    std::fs::create_dir_all(&out).expect("out dir");
    let mut cache = cantus_engine::samples::SampleCache::new();
    let chords = [
        load(&samples, "Sample07 (A Minor)", &mut cache),
        load(&samples, "Sample15 (F Major)", &mut cache),
        load(&samples, "Sample22 (C Major)", &mut cache),
        load(&samples, "Sample17 (G Major)", &mut cache),
    ];
    // The progression: eight bars, a strum on beats 1 and 3.
    let mut di = vec![];
    for bar in 0..8 {
        for half in 0..2 {
            let at = beats(bar as f64 * 4.0 + half as f64 * 2.0);
            place(&mut di, &chords[bar % 4], at, at + beats(2.0));
        }
    }
    di.resize(beats(32.0), 0.0);
    let g = cantus_dsp::math::db_to_gain(-6.0) / peak(&di);
    di.iter_mut().for_each(|x| *x *= g);
    let target = rms(&di);
    let mut notes = String::from(
        "Fragor on a real guitar. Guitar: Carbonthief, CC0 (DI chord strums, the Renoise \
         forum's library; its 32-bit FLACs decoded to WAV for this, since Cantus's FLAC reader
         does not read 32-bit files). The owner's reference progression: Am - F - C - G, 90 BPM, 8 bars, \
         a strum on beats 1 and 3. 48 kHz, 24-bit FLAC. The dry file peaks at -6 dBFS; every \
         processed file is level-matched to it (RMS) unless said otherwise.\n\n",
    );
    write(&out.join("00-guitar-di.flac"), &di);
    notes.push_str("00-guitar-di.flac\n  The guitar, dry.\n\n");
    let matched = |v: Vec<f32>| -> Vec<f32> {
        let g = target / rms(&v).max(1e-9);
        let v: Vec<f32> = v.iter().map(|x| x * g).collect();
        let p = peak(&v);
        // Never clip the file: the level match gives way to a −0.3 dBFS ceiling.
        let c = if p > 0.966 { 0.966 / p } else { 1.0 };
        v.iter().map(|x| x * c).collect()
    };
    use Model::*;
    type Run = (
        &'static str,
        Vec<Model>,
        Vec<(Model, [f32; 3])>,
        &'static str,
    );
    let runs: Vec<Run> = vec![
        (
            "01-green-overdrive.flac",
            vec![Overdrive],
            vec![(Overdrive, [0.6, 0.55, 0.5])],
            "Green overdrive: Drive 60 %, Tone 55 %.",
        ),
        (
            "02-op-amp-distortion.flac",
            vec![OpAmp],
            vec![(OpAmp, [0.65, 0.4, 0.5])],
            "Op-amp distortion: Distortion 65 %, Filter 40 %.",
        ),
        (
            "03-silicon-distortion.flac",
            vec![Silicon],
            vec![(Silicon, [0.6, 0.5, 0.5])],
            "Silicon distortion: Dist 60 %, Tone 50 %.",
        ),
        (
            "04-germanium-fuzz.flac",
            vec![Fuzz],
            vec![(Fuzz, [0.85, 1.0, 0.5])],
            "Germanium fuzz: Fuzz 85 %, Bias 100 % (a fresh battery).",
        ),
        (
            "05-triangle-muff.flac",
            vec![Muff],
            vec![(Muff, [0.65, 0.45, 0.5])],
            "Triangle muff: Sustain 65 %, Tone 45 %.",
        ),
        (
            "06-chain-overdrive-fuzz-muff.flac",
            vec![Overdrive, Fuzz, Muff],
            vec![
                (Overdrive, [0.3, 0.6, 0.7]),
                (Fuzz, [0.6, 0.8, 0.45]),
                (Muff, [0.5, 0.4, 0.5]),
            ],
            "The design sheet's chain: green overdrive into germanium fuzz into triangle muff.",
        ),
        (
            "08-preset-stacked-lead.flac",
            vec![Overdrive, OpAmp],
            vec![(Overdrive, [0.3, 0.6, 0.7]), (OpAmp, [0.55, 0.35, 0.5])],
            "Factory preset Stacked lead: the overdrive tightening the op-amp distortion.",
        ),
        (
            "09-preset-vintage-fuzz.flac",
            vec![Fuzz],
            vec![(Fuzz, [0.8, 1.0, 0.5])],
            "Factory preset Vintage fuzz: the germanium fuzz on a fresh battery.",
        ),
    ];
    for (name, chain, knobs, what) in runs {
        let y = matched(through(&board(&chain, &knobs), &di));
        write(&out.join(name), &y);
        notes.push_str(&format!("{name}\n  {what}\n\n"));
    }
    // The battery running down on held chords: an A minor strum left to ring a whole bar,
    // at Bias 100, 60, 35, 20 and 10 %.
    let mut sweep = vec![];
    let held: Vec<f32> = chords[0].iter().map(|x| x * g).collect();
    for (i, bias) in [1.0f32, 0.6, 0.35, 0.2, 0.1].iter().enumerate() {
        let mut note = vec![];
        place(&mut note, &held, 0, beats(4.0));
        note.resize(beats(4.0), 0.0);
        let y = through(&board(&[Fuzz], &[(Fuzz, [1.0, *bias, 0.5])]), &note);
        place(
            &mut sweep,
            &y[..beats(4.0)],
            i * beats(4.0),
            (i + 1) * beats(4.0),
        );
    }
    let sp = peak(&sweep);
    let sweep: Vec<f32> = sweep.iter().map(|x| x * 0.7 / sp).collect();
    write(&out.join("07-germanium-fuzz-battery-sweep.flac"), &sweep);
    notes.push_str(
        "07-germanium-fuzz-battery-sweep.flac\n  The germanium fuzz at full Fuzz on an A minor \
         chord left to ring a bar, five times: Bias 100 % (fresh battery), 60 %, 35 %, 20 %, \
         10 %. Not level-matched (the dying battery is quieter, as a real one is): scaled once \
         to peak at -3 dBFS. Listen to the end of each chord.\n\n",
    );
    std::fs::write(out.join("README.txt"), notes).expect("readme");
}

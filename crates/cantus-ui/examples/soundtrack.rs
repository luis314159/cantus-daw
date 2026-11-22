//! The launch video's soundtrack (round 41, `docs/round41/video/README.md` §4), mixed **in
//! Cantus**: one project per language, rendered through Cantus's own export path.
//!
//! ```text
//! cargo run --release -p cantus-ui --example soundtrack -- --lang en [--out <dir>]
//!     [--voice <dir>] [--depth <dB>] [--margin <dB>]
//! ```
//!
//! 1. *With All Your Heart* rendered by Cantus (`cantus_export::export_mix`, the export
//!    dialog's path) to `heart.wav`, once.
//! 2. A project of two audio tracks, each on a strip of its own:
//!    - **Music**: the Heart render from bar 1 at 0:00; at bar 12 (24.83 s) one equal-power
//!      crossfade of 2 s ([`XFADE_S`], the clips' overlap, ADR 0070) jumps 44 bars ahead, to
//!      the last chorus's pickup, so the song's last chord rings out at the film's end;
//!    - **Voice**: the eight chosen lines (`<voice>/<lang>/chosen.json`), each at its
//!      `start`, its lead trimmed to 60 ms.
//! 3. **The carve** (the owner, draft 3): Aequalis on the music's strip, four dynamic bells
//!    keyed by the voice's strip, cut 200 Hz – 5 kHz while the voice speaks (40 ms attack,
//!    450 ms release); the music's level elsewhere stays at draft 2's bed (its fader at
//!    −3 dB). The carve is deepened until the music sits at least `--margin` dB (10) under
//!    the voice in the speech band (300 Hz–3 kHz) during every line, measured on the two
//!    strips' stems; the bed is lowered only if the deepest carve (30 dB) cannot do it.
//! 4. The master: the export's loudness stage — BS.1770-4 to −14 LUFS integrated under a
//!    −1 dBTP ceiling, Terminus's true-peak limiter holding the peaks — written as 48 kHz
//!    24-bit WAV.
//!
//! Out (in `--out`, default `F:\projects\video-work\soundtrack`): `heart.wav`,
//! `soundtrack-<lang>.cantus`, `soundtrack-<lang>.wav`, the stems
//! `soundtrack-<lang>-music.wav` and `-voice.wav`, and `soundtrack-<lang>.json` (the edit,
//! the ducking per line, the loudness as Cantus measured it).

use cantus_core::audio::AudioSource;
use cantus_core::crossfade::XfadeCurve;
use cantus_core::{AudioSourceId, EffectKind, Project, Snapshot, StripId, Ticks};
use cantus_engine::{RenderOptions, SampleCache, StemOptions, StemSource, StemSpec};
use cantus_export::{Control, Depth, ExportSettings, Format, Loudness, Tags};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 48_000;
/// The Heart's tempo (`demos::heart::BPM`): the project runs at it, so ticks are its bars.
const BPM: f64 = 116.0;
/// The film's length, seconds.
const FILM_S: f64 = 80.0;
/// Where the jump is centred: the film's (and the song's) bar 12, a four-bar phrase's start.
pub const XFADE_BAR: f64 = 12.0;
/// How far it jumps: bar 12 becomes bar 56, the last chorus's first bar.
pub const JUMP_BARS: f64 = 44.0;
/// The crossfade's length, seconds.
const XFADE_S: f64 = 2.0;
/// The deepest carve tried (Aequalis's dynamic range is ±30 dB).
const MAX_DEPTH: f32 = 30.0;
/// The carve's bells: centre and Q, together 200 Hz – 5 kHz (each about 1.4 octaves wide).
const CARVE: [(f32, f32); 4] = [(320.0, 1.0), (850.0, 1.0), (2_100.0, 1.0), (4_300.0, 1.2)];
/// The carve's threshold on the key (the voice in each band's region), its attack.
const CARVE_THR: f32 = -60.0;
const CARVE_ATT: f32 = 40.0;
/// Draft 2's bed: the music's fader.
const DRAFT2_BED: f32 = -3.0;
/// The lead kept before each line's speech.
const LEAD_S: f64 = 0.06;

fn bar_s(bars: f64) -> f64 {
    bars * 4.0 * 60.0 / BPM
}

fn ticks(seconds: f64) -> Ticks {
    Ticks::from_seconds(seconds, BPM)
}

#[derive(Clone, Debug)]
struct Opts {
    lang: String,
    out: PathBuf,
    voice: PathBuf,
    depth: f32,
    margin: f32,
    /// The music's fader: the bed's level under everything (lowered until every line
    /// stands clear).
    bed: f32,
    /// The carve's release, ms.
    release: f32,
}

impl Opts {
    fn parse() -> Opts {
        let mut o = Opts {
            lang: "en".into(),
            out: PathBuf::from(r"F:\projects\video-work\soundtrack"),
            voice: PathBuf::from(r"F:\projects\video-work\voice\chosen"),
            depth: 9.0,
            margin: 10.0,
            bed: DRAFT2_BED,
            release: 450.0,
        };
        let a: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i + 1 < a.len() {
            let v = a[i + 1].clone();
            match a[i].as_str() {
                "--lang" => o.lang = v,
                "--out" => o.out = PathBuf::from(v),
                "--voice" => o.voice = PathBuf::from(v),
                "--depth" => o.depth = v.parse().expect("--depth dB"),
                "--bed" => o.bed = v.parse().expect("--bed dB"),
                "--release" => o.release = v.parse().expect("--release ms"),
                "--margin" => o.margin = v.parse().expect("--margin dB"),
                x => panic!("unknown option {x}"),
            }
            i += 2;
        }
        o
    }
}

/// One narration line from `chosen.json`.
#[derive(Clone, Debug)]
struct Line {
    n: u32,
    file: PathBuf,
    start: f64,
    speech_from: f64,
    speech_s: f64,
}

fn lines(o: &Opts) -> Vec<Line> {
    let dir = o.voice.join(&o.lang);
    let text = std::fs::read_to_string(dir.join("chosen.json")).expect("chosen.json");
    let v: serde_json::Value = serde_json::from_str(&text).expect("the manifest");
    v.as_array()
        .expect("a list of lines")
        .iter()
        .map(|l| {
            let n = l["line"].as_u64().expect("line") as u32;
            Line {
                n,
                file: dir.join(format!("{n:02}.wav")),
                start: l["start"].as_f64().expect("start"),
                speech_from: l["speech_from"].as_f64().expect("speech_from"),
                speech_s: l["speech_s"].as_f64().expect("speech_s"),
            }
        })
        .collect()
}

/// *With All Your Heart*, rendered by Cantus's export (32-bit float, as mixed), once.
fn heart(o: &Opts) -> PathBuf {
    let path = o.out.join("heart.wav");
    if path.exists() {
        return path;
    }
    let demo = cantus_ui::demos::find("heart").expect("the Heart demo");
    let p = (demo.build)();
    let mut snap = Snapshot::compile(&p, 1);
    let missing = SampleCache::new().attach_audio(&mut snap, RATE);
    assert!(missing.is_empty(), "the Heart's audio: {missing:?}");
    let job = cantus_export::MixJob {
        snapshot: Arc::new(snap),
        render: RenderOptions {
            sample_rate: RATE,
            tail_seconds: 4.0,
            ..Default::default()
        },
        units: Vec::new(),
        instruments: Vec::new(),
    };
    let s = ExportSettings {
        format: Format::Wav,
        depth: Depth::Float32,
        tail_seconds: 4.0,
        ..Default::default()
    };
    let done = cantus_export::export_mix(job, &s, &Tags::default(), &path, &Control::new())
        .expect("the Heart is rendered");
    println!(
        "heart.wav: {:.2} s, peak {:.1} dBFS",
        done.seconds, done.peak_db
    );
    path
}

/// A file of the pool, as an import leaves it.
fn source(p: &mut Project, path: &Path, name: &str) -> AudioSourceId {
    let r = hound::WavReader::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let spec = r.spec();
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: name.into(),
        path: path.display().to_string(),
        sample_rate: spec.sample_rate,
        channels: spec.channels as u8,
        frames: r.duration() as u64,
        sha256: None,
        recorded: false,
    });
    id
}

/// The soundtrack's project: its music and voice strips.
struct Mix {
    p: Project,
    music: StripId,
    voice: StripId,
}

fn strip_mut(p: &mut Project, id: StripId) -> &mut cantus_core::MixerStrip {
    p.strips.iter_mut().find(|s| s.id == id).expect("the strip")
}

/// The project: the Heart edited (one crossfade), the eight lines, the carve on the music
/// keyed by the voice, `depth` dB deep, the music's fader at `bed`.
fn project(o: &Opts, heart: &Path, lines: &[Line], depth: f32, bed: f32) -> Mix {
    let mut p = Project::new(format!(
        "Cantus launch soundtrack ({})",
        o.lang.to_uppercase()
    ));
    p.tempo_bpm = BPM;
    let song = source(&mut p, heart, "With All Your Heart (rendered by Cantus)");
    // The music: bar 1 at 0:00; at bar 12 a 2-s equal-power crossfade into bar 56.
    let mut music = p.make_track("Music");
    let music_strip = music.strip.expect("a free strip for the music");
    let (xc, jump, half) = (bar_s(XFADE_BAR), bar_s(JUMP_BARS), XFADE_S / 2.0);
    let a = p.make_audio_clip("Heart from bar 1", song, Ticks::ZERO, ticks(xc + half));
    let b_start = xc - half;
    let mut b = p.make_audio_clip(
        "Heart from bar 56",
        song,
        ticks(b_start),
        ticks(FILM_S - b_start),
    );
    b.offset = ticks(b_start + jump);
    if let Some(r) = b.audio.as_mut() {
        r.cross = Some(XfadeCurve::EqualPower);
    }
    music.clips.push(a);
    music.clips.push(b);
    p.tracks.push(music);
    // The voice: each line at its start, 60 ms of its lead kept.
    let mut voice = p.make_track("Voice");
    let voice_strip = voice.strip.expect("a free strip for the voice");
    for l in lines {
        let id = source(&mut p, &l.file, &format!("Line {}", l.n));
        let lead = LEAD_S.min(l.speech_from);
        let start = l.start - lead;
        let mut c = p.make_audio_clip(
            format!("Line {}", l.n),
            id,
            ticks(start),
            ticks(lead + l.speech_s + 0.35),
        );
        c.offset = ticks(l.speech_from - lead);
        voice.clips.push(c);
    }
    p.tracks.push(voice);
    // The carve (the owner, draft 3): Aequalis on the music, four dynamic bells keyed by
    // the voice's strip (each band's detector hears the key in its own region), cutting
    // 200 Hz – 5 kHz by `depth` dB while the voice speaks; the rest of the music untouched.
    let mut carve = p.make_effect(EffectKind::Eq);
    carve.sidechain = Some(voice_strip);
    for (i, (hz, q)) in CARVE.iter().enumerate() {
        for (k, v) in [
            ("used", 1.0),
            ("on", 1.0),
            ("shape", 0.0),
            ("freq", *hz),
            ("q", *q),
            ("gain", 0.0),
            ("dyn", 1.0),
            ("key", 1.0),
            ("dir", 0.0),
            ("thr", CARVE_THR),
            ("range", -depth),
            ("ratio", 20.0),
            ("att", CARVE_ATT),
            ("rel", o.release),
        ] {
            carve.spec.set(&cantus_core::fx::eq::eq_key(i, k), v);
        }
    }
    let m = strip_mut(&mut p, music_strip);
    m.name = "Music".into();
    m.gain_db = bed;
    m.effects.push(carve);
    strip_mut(&mut p, voice_strip).name = "Voice".into();
    Mix {
        p,
        music: music_strip,
        voice: voice_strip,
    }
}

fn snapshot(p: &Project) -> Snapshot {
    let mut s = Snapshot::compile(p, 1);
    let missing = SampleCache::new().attach_audio(&mut s, RATE);
    assert!(missing.is_empty(), "the soundtrack's audio: {missing:?}");
    s
}

fn film_range() -> RenderOptions {
    RenderOptions {
        sample_rate: RATE,
        start: Ticks::ZERO,
        end: Some(ticks(FILM_S)),
        tail_seconds: 0.0,
        compensate_latency: true,
        ..Default::default()
    }
}

/// The music's and the voice's strips, as mixed (after the duck and the faders).
fn stems(mix: &Mix) -> (Vec<f32>, Vec<f32>) {
    let s = snapshot(&mix.p);
    let at = |id: StripId| s.strips.iter().position(|x| x.id == id).expect("a stem");
    let specs = [
        StemSpec {
            source: StemSource::Strip(at(mix.music)),
            file_name: "music.wav".into(),
        },
        StemSpec {
            source: StemSource::Strip(at(mix.voice)),
            file_name: "voice.wav".into(),
        },
    ];
    let opts = StemOptions {
        render: film_range(),
        keep_silent: true,
        through_master: false,
    };
    let mut out = cantus_engine::render_stems(Arc::new(s), &opts, &specs).expect("the stems");
    let voice = out.pop().expect("voice").samples;
    let music = out.pop().expect("music").samples;
    (music, voice)
}

/// A second-order section (RBJ), run over a signal.
fn biquad(x: &mut [f32], b: [f64; 3], a: [f64; 3]) {
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0, 0.0, 0.0);
    for s in x.iter_mut() {
        let x0 = *s as f64;
        let y0 = (b[0] * x0 + b[1] * x1 + b[2] * x2 - a[1] * y1 - a[2] * y2) / a[0];
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        *s = y0 as f32;
    }
}

/// The speech band: the mid (L+R)/2 through a 300 Hz high-pass and a 3 kHz low-pass
/// (Butterworth, 12 dB/oct each).
fn speech_band(inter: &[f32]) -> Vec<f32> {
    use cantus_dsp::math;
    let mut x: Vec<f32> = inter.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect();
    let q = std::f64::consts::FRAC_1_SQRT_2;
    let w = |hz: f64| 2.0 * std::f64::consts::PI * hz / RATE as f64;
    let (c, s) = (math::cos(w(300.0)), math::sin(w(300.0)));
    let al = s / (2.0 * q);
    biquad(
        &mut x,
        [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
        [1.0 + al, -2.0 * c, 1.0 - al],
    );
    let (c, s) = (math::cos(w(3000.0)), math::sin(w(3000.0)));
    let al = s / (2.0 * q);
    biquad(
        &mut x,
        [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
        [1.0 + al, -2.0 * c, 1.0 - al],
    );
    x
}

/// RMS of `x` (mono) from `a` to `b` seconds, dBFS.
fn rms_db(x: &[f32], a: f64, b: f64) -> f32 {
    let i0 = ((a.max(0.0)) * RATE as f64) as usize;
    let i1 = ((b * RATE as f64) as usize).min(x.len());
    if i1 <= i0 {
        return f32::NEG_INFINITY;
    }
    let e: f64 = x[i0..i1]
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        / (i1 - i0) as f64;
    (10.0 * cantus_dsp::math::log10(e.max(1e-20))) as f32
}

/// Per line: the music and the voice in the speech band while it speaks, and the music
/// in the gaps around it (a second after the last line, until just before the next).
struct Duck {
    n: u32,
    from: f64,
    to: f64,
    music_db: f32,
    voice_db: f32,
    margin_db: f32,
    music_between_db: f32,
    /// The music's full-band RMS, 400 ms before the line against 400 ms once it has begun.
    entry_drop_db: f32,
}

fn measure(music: &[f32], voice: &[f32], lines: &[Line]) -> Vec<Duck> {
    let (mb, vb) = (speech_band(music), speech_band(voice));
    let full: Vec<f32> = music.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let (a, b) = (l.start, l.start + l.speech_s);
        let music_db = rms_db(&mb, a, b);
        let voice_db = rms_db(&vb, a, b);
        // The gap before the line: from a second after the previous one (or 1 s in)
        // to 0.3 s before this one.
        let gap_from = if i == 0 {
            1.0
        } else {
            lines[i - 1].start + lines[i - 1].speech_s + 1.0
        };
        let gap_to = a - 0.3;
        let music_between_db = if gap_to - gap_from >= 0.5 {
            rms_db(&mb, gap_from, gap_to)
        } else {
            f32::NAN
        };
        out.push(Duck {
            n: l.n,
            from: a,
            to: b,
            music_db,
            voice_db,
            margin_db: voice_db - music_db,
            music_between_db,
            entry_drop_db: rms_db(&full, a - 0.6, a - 0.2) - rms_db(&full, a + 0.2, a + 0.6),
        });
    }
    out
}

/// The largest step between 100-ms RMS windows (hop 50 ms) of the full mix's mid from
/// `a` to `b` seconds: the crossfade's continuity.
fn steps(mix: &[f32], a: f64, b: f64) -> (f32, f64) {
    let x: Vec<f32> = mix.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect();
    let mut worst = (0.0f32, a);
    let mut t = a;
    let mut last = rms_db(&x, t, t + 0.1);
    while t + 0.15 <= b {
        t += 0.05;
        let now = rms_db(&x, t, t + 0.1);
        let d = (now - last).abs();
        if d.is_finite() && d > worst.0 {
            worst = (d, t);
        }
        last = now;
    }
    worst
}

/// The music alone where no line speaks (a second after each line until 0.3 s before the
/// next), as one BS.1770 integrated reading (Terminus's meter), LUFS.
fn bed_lufs(music: &[f32], lines: &[Line]) -> f64 {
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(RATE as f32);
    let mut from = 1.0;
    let mut spans = Vec::new();
    for l in lines {
        spans.push((from, l.start - 0.3));
        from = l.start + l.speech_s + 1.0;
    }
    spans.push((from, FILM_S - 1.0));
    for (a, b) in spans {
        if b - a < 0.5 {
            continue;
        }
        let i0 = (a * RATE as f64) as usize * 2;
        let i1 = ((b * RATE as f64) as usize * 2).min(music.len());
        let (l, r): (Vec<f32>, Vec<f32>) =
            music[i0..i1].chunks_exact(2).map(|f| (f[0], f[1])).unzip();
        m.process(&l, &r);
    }
    m.integrated().unwrap_or(f64::NEG_INFINITY)
}

fn write_stem(path: &Path, samples: Vec<f32>) {
    let peak = samples.iter().fold(0f32, |m, v| m.max(v.abs()));
    let r = cantus_engine::Rendered {
        sample_rate: RATE,
        channels: 2,
        samples,
        peak,
        cut_notes: 0,
    };
    cantus_engine::write_wav(&r, path).expect("a stem is written");
}

fn main() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(run)
        .expect("a thread")
        .join()
        .expect("the soundtrack");
}

fn run() {
    let o = Opts::parse();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    std::fs::create_dir_all(&o.out).expect("the output folder");
    let heart = heart(&o);
    let lines = lines(&o);
    println!("cantus_dsp::DSP_VERSION {}", cantus_dsp::DSP_VERSION);
    // The carve deepened until every line stands `margin` dB over the music in the speech
    // band; the bed lowered only if the deepest carve cannot do it (the owner, draft 3).
    let mut depth = o.depth.min(MAX_DEPTH);
    let mut bed = o.bed;
    let (mix, ducks, music_bed) = loop {
        let mix = project(&o, &heart, &lines, depth, bed);
        let (music, voice) = stems(&mix);
        let ducks = measure(&music, &voice, &lines);
        let worst = ducks.iter().map(|d| d.margin_db).fold(f32::MAX, f32::min);
        println!("bed {bed:.1} dB, carve {depth:.1} dB: the closest line stands {worst:.1} dB over the music");
        if worst >= o.margin || bed <= -30.0 {
            let music_bed = bed_lufs(&music, &lines);
            let stem = |w: &str| o.out.join(format!("soundtrack-{}-{w}.wav", o.lang));
            write_stem(&stem("music"), music);
            write_stem(&stem("voice"), voice);
            break (mix, ducks, music_bed);
        }
        if depth < MAX_DEPTH {
            depth = (depth + (o.margin - worst).max(1.5)).min(MAX_DEPTH);
        } else {
            bed = (bed - (o.margin - worst).max(0.5) - 0.3).max(-30.0);
        }
    };
    // The project, saved.
    let cantus = o.out.join(format!("soundtrack-{}.cantus", o.lang));
    cantus_project::save(&mix.p, &cantus, env!("CARGO_PKG_VERSION")).expect("the project is saved");
    // The master: Cantus's export, −14 LUFS under −1 dBTP, 48 kHz 24-bit WAV.
    let wav = o.out.join(format!("soundtrack-{}.wav", o.lang));
    let job = cantus_export::MixJob {
        snapshot: Arc::new(snapshot(&mix.p)),
        render: film_range(),
        units: Vec::new(),
        instruments: Vec::new(),
    };
    let s = ExportSettings {
        format: Format::Wav,
        depth: Depth::Int24,
        loudness: Loudness {
            target_lufs: Some(-14.0),
            ceiling_dbtp: -1.0,
            limit: true,
        },
        tail_seconds: 0.0,
        ..Default::default()
    };
    let done = cantus_export::export_mix(job, &s, &Tags::default(), &wav, &Control::new())
        .expect("the soundtrack is exported");
    let n = done.loudness.expect("the loudness stage ran");
    // For the web's lossy encode (AAC overshoots a -1 dBTP master by about a dB): the same
    // mix at -14 LUFS under a -2 dBTP ceiling, so the video's sound stays under -1 dBTP.
    let web = o.out.join(format!("soundtrack-{}-web.wav", o.lang));
    let job = cantus_export::MixJob {
        snapshot: Arc::new(snapshot(&mix.p)),
        render: film_range(),
        units: Vec::new(),
        instruments: Vec::new(),
    };
    let mut s2 = s.clone();
    s2.loudness.ceiling_dbtp = -2.0;
    let web_done = cantus_export::export_mix(job, &s2, &Tags::default(), &web, &Control::new())
        .expect("the web soundtrack is exported");
    let nw = web_done.loudness.expect("the loudness stage ran");
    println!(
        "{}: written {:.1} LUFS {:.1} dBTP (for the lossy encode)",
        web.display(),
        nw.lufs.unwrap_or(f64::NAN),
        nw.dbtp
    );
    // The crossfade's continuity, read back from the file.
    let r = hound::WavReader::open(&wav).expect("the export");
    let full = 1.0 / (1u32 << 23) as f32;
    let samples: Vec<f32> = r
        .into_samples::<i32>()
        .map(|v| v.expect("a sample") as f32 * full)
        .collect();
    let xc = bar_s(XFADE_BAR);
    let (step_mix, step_mix_at) = steps(&samples, xc - 2.0, xc + 2.0);
    // The crossfade alone: the music before the duck (depth 0) over it, against the
    // song's own bars on either side (its own hits, unedited).
    let (dry, _) = stems(&project(&o, &heart, &lines, 0.0, bed));
    let (step, step_at) = steps(&dry, xc - 1.5, xc + 1.5);
    let song: Vec<f32> = hound::WavReader::open(&heart)
        .expect("heart.wav")
        .into_samples::<f32>()
        .map(|v| v.expect("a sample"))
        .collect();
    let (step_a, _) = steps(&song, xc - 1.5, xc + 1.5);
    let jump = bar_s(JUMP_BARS);
    let (step_b, _) = steps(&song, xc + jump - 1.5, xc + jump + 1.5);
    // Said, and kept beside the files.
    println!(
        "{}: {:.2} s; as mixed {:.1} LUFS {:.1} dBTP; gain {:+.1} dB{}; written {:.1} LUFS {:.1} dBTP",
        wav.display(),
        done.seconds,
        n.measured_lufs.unwrap_or(f64::NAN),
        n.measured_dbtp,
        n.gain_db,
        if n.limited { ", limited" } else { "" },
        n.lufs.unwrap_or(f64::NAN),
        n.dbtp
    );
    println!(
        "the bed: fader {bed:.1} dB; the music alone between the lines {:.1} LUFS as mixed, {:.1} LUFS as heard (after the export's {:+.1} dB)",
        music_bed,
        music_bed + n.gain_db as f64,
        n.gain_db
    );
    println!(
        "line  speech (s)      music  voice  margin  music between  carve  full-band drop at entry"
    );
    for d in &ducks {
        println!(
            "{:>4}  {:5.2}–{:5.2}  {:6.1} {:6.1} {:7.1} {:14.1} {:6.1} {:6.1}",
            d.n,
            d.from,
            d.to,
            d.music_db,
            d.voice_db,
            d.margin_db,
            d.music_between_db,
            d.music_between_db - d.music_db,
            d.entry_drop_db
        );
    }
    println!(
        "crossfade at {xc:.2} s: the music's largest 100-ms RMS step within ±1.5 s {step:.1} dB at {step_at:.2} s \
         (the song's own there: {step_a:.1} dB at bar 12, {step_b:.1} dB at bar 56); the mix's within ±2 s \
         {step_mix:.1} dB at {step_mix_at:.2} s"
    );
    let json = serde_json::json!({
        "lang": o.lang,
        "song": "With All Your Heart (demos::heart), rendered by cantus_export::export_mix",
        "edit": {
            "from_bar_1_at": 0.0,
            "crossfade_centre_s": xc,
            "crossfade_s": XFADE_S,
            "curve": "equal power (the clips' overlap, ADR 0070)",
            "jump_bars": JUMP_BARS,
            "film_bar_12_becomes_song_bar": XFADE_BAR + JUMP_BARS,
            "song_offset_after_s": bar_s(JUMP_BARS),
        },
        "dsp_version": cantus_dsp::DSP_VERSION,
        "carve": {"effect": "Aequalis (EffectKind::Eq), dynamic bells keyed by the voice strip",
                  "bells_hz_q": CARVE, "range_db": -depth, "threshold_db": CARVE_THR, "ratio": 20.0,
                  "attack_ms": CARVE_ATT, "release_ms": o.release, "music_fader_db": bed,
                  "bed_lowered_db": DRAFT2_BED - bed},
        "bed": {"music_fader_db": bed, "music_between_lines_lufs_as_mixed": music_bed,
                "music_between_lines_lufs_as_heard": music_bed + n.gain_db as f64},
        "lines": ducks.iter().map(|d| serde_json::json!({
            "line": d.n, "from": d.from, "to": d.to,
            "music_speech_band_db": d.music_db, "voice_speech_band_db": d.voice_db,
            "margin_db": d.margin_db, "music_between_db": d.music_between_db,
            "carve_db": d.music_between_db - d.music_db,
            "full_band_drop_at_entry_db": d.entry_drop_db,
        })).collect::<Vec<_>>(),
        "loudness_cantus": {
            "as_mixed_lufs": n.measured_lufs, "as_mixed_dbtp": n.measured_dbtp,
            "gain_db": n.gain_db, "limited": n.limited,
            "written_lufs": n.lufs, "written_dbtp": n.dbtp,
            "meter": "cantus_export's loudness stage: BS.1770-4, Terminus's meter and true-peak limiter",
        },
        "crossfade_check": {
            "music_before_duck": {"largest_step_db": step, "at_s": step_at, "span_s": 1.5},
            "song_unedited_bar_12": step_a, "song_unedited_bar_56": step_b,
            "mix": {"largest_step_db": step_mix, "at_s": step_mix_at, "span_s": 2.0},
            "window_ms": 100, "hop_ms": 50},
        "web": {"wav": web.display().to_string(), "written_lufs": nw.lufs, "written_dbtp": nw.dbtp, "ceiling_dbtp": -2.0},
        "files": {"wav": wav.display().to_string(), "project": cantus.display().to_string()},
    });
    let report = o.out.join(format!("soundtrack-{}.json", o.lang));
    std::fs::write(&report, serde_json::to_string_pretty(&json).expect("json"))
        .expect("the report");
}

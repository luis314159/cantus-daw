//! Vetus's sound examples (fx side round, ADR 0130): each beside its dry twin,
//! level-matched within 1 dB, written as FLAC.
//!
//! - **The guitar**: the owner's reference progression (Am, F, C, G at 90 BPM, struck on
//!   beats 1 and 3, eight bars) from Carbonthief's CC0 DI strums — read with Cantus's own
//!   decoder from WAVs made from the 32-bit FLACs (which it cannot read yet), resampled to
//!   48 kHz with its resampler.
//! - **The drums and the keys** of the *Paper Lanterns* demo (lo-fi hip-hop, 80 BPM), each
//!   soloed and rendered by the engine from bar 5 for eight bars.
//!
//! Every example runs Vetus's DSP on the song's clock (the transport told each block), as
//! the engine does. The samples are not in the repository: pass their folder.
//!
//! ```text
//! cargo run --release -p cantus-ui --example render_vetus -- <guitar WAV dir> <out dir>
//! ```

use cantus_core::{EffectKind, EffectSpec, Project, Ticks};
use cantus_dsp::modfx_clock::Transport;
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 48_000;

/// One strum, stereo interleaved at 48 kHz.
fn strum(dir: &Path, stem: &str) -> Vec<f32> {
    let d = ["flac", "wav"]
        .iter()
        .find_map(|ext| {
            let p = dir.join(format!("{stem}.{ext}"));
            p.exists()
                .then(|| cantus_engine::SampleCache::decode_source(&p).ok())
                .flatten()
        })
        .expect("decoded");
    let ch = d.channels as usize;
    let mut x: Vec<f32> = d.frames.to_vec();
    if d.rate != RATE {
        x = cantus_engine::samples::resample::resample(&x, ch, d.rate, RATE);
    }
    if ch == 1 {
        x.iter().flat_map(|v| [*v, *v]).collect()
    } else {
        x
    }
}

/// The owner's progression (`refs/audio/README.md`), peak −6 dBFS.
fn guitar(dir: &Path) -> (Vec<f32>, Vec<f32>) {
    const CHORDS: [&str; 4] = [
        "Sample07 (A Minor)",
        "Sample15 (F Major)",
        "Sample22 (C Major)",
        "Sample17 (G Major)",
    ];
    let beat = (60.0 / 90.0 * f64::from(RATE)) as usize;
    let n = 8 * 4 * beat + RATE as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let fade = (0.02 * f64::from(RATE)) as usize;
    let strums: Vec<Vec<f32>> = CHORDS.iter().map(|c| strum(dir, c)).collect();
    for bar in 0..8 {
        for half in 0..2 {
            let at = (bar * 4 + half * 2) * beat;
            let len = 2 * beat;
            let s = &strums[bar % 4];
            for i in 0..len.min(s.len() / 2) {
                let g = if i + fade >= len {
                    (len - i) as f32 / fade as f32
                } else {
                    1.0
                };
                l[at + i] += s[2 * i] * g;
                r[at + i] += s[2 * i + 1] * g;
            }
        }
    }
    peak_to(&mut l, &mut r, -6.0);
    (l, r)
}

fn peak_to(l: &mut [f32], r: &mut [f32], db: f32) {
    let peak = l
        .iter()
        .chain(r.iter())
        .fold(0f32, |m, v| m.max(v.abs()))
        .max(1e-9);
    let g = cantus_dsp::math::db_to_gain(db) / peak;
    for v in l.iter_mut().chain(r.iter_mut()) {
        *v *= g;
    }
}

/// The lo-fi demo with only the tracks `keep` names, rendered from bar 5 for eight bars.
fn demo_part(keep: &dyn Fn(&str) -> bool) -> (Vec<f32>, Vec<f32>) {
    let mut p: Project = (cantus_ui::demos::find("lofi")
        .expect("the lo-fi demo")
        .build)();
    for t in &mut p.tracks {
        t.muted = !keep(&t.name.to_lowercase());
    }
    let opts = cantus_engine::RenderOptions {
        start: Ticks::beats(16),
        end: Some(Ticks::beats(48)),
        tail_seconds: 0.5,
        ..Default::default()
    };
    let out = cantus_engine::render(Arc::new(cantus_core::Snapshot::compile(&p, 1)), &opts)
        .expect("rendered")
        .samples;
    let (mut l, mut r): (Vec<f32>, Vec<f32>) = out.chunks_exact(2).map(|c| (c[0], c[1])).unzip();
    peak_to(&mut l, &mut r, -6.0);
    (l, r)
}

fn rms(l: &[f32], r: &[f32]) -> f32 {
    let s: f64 = l
        .iter()
        .chain(r)
        .map(|v| f64::from(*v) * f64::from(*v))
        .sum();
    (s / (l.len() + r.len()) as f64).sqrt() as f32
}

/// Through Vetus with `params` (Character rising from `char0` to `char1` across the take),
/// the song playing from `start` at `bpm`.
fn through(
    x: &(Vec<f32>, Vec<f32>),
    params: &[(&str, f32)],
    bpm: f64,
    start: i64,
    sweep: Option<(f32, f32)>,
) -> (Vec<f32>, Vec<f32>) {
    let mut spec = EffectSpec::new(EffectKind::LoFi);
    for (k, v) in params {
        spec.set(k, *v).unwrap_or_else(|| panic!("{k}"));
    }
    let mut u = cantus_dsp::vetus::Vetus::new(RATE as f32);
    u.set(&cantus_engine::fx::vetus::settings(&spec));
    u.reset();
    let (mut l, mut r) = x.clone();
    let len = l.len();
    let mut at = 0usize;
    while at < len {
        let n = 480.min(len - at);
        if let Some((a, b)) = sweep {
            spec.set("char", a + (b - a) * at as f32 / len as f32);
            u.set(&cantus_engine::fx::vetus::settings(&spec));
        }
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: bpm,
            samples: start + at as i64,
        });
        u.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l, r)
}

fn write(l: &[f32], r: &[f32], path: &Path, title: &str, album: &str) {
    let samples: Vec<f32> = l.iter().zip(r).flat_map(|(a, b)| [*a, *b]).collect();
    let s = ExportSettings {
        format: Format::Flac,
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus".into(),
        album: album.into(),
        ..Default::default()
    };
    export_rendered(
        Audio {
            rate: RATE,
            channels: 2,
            samples,
        },
        &s,
        &tags,
        path,
        &Control::new(),
    )
    .expect("written");
    println!("{}", path.display());
}

type Example = (
    &'static str,
    &'static str,
    Vec<(&'static str, f32)>,
    Option<(f32, f32)>,
);

/// Writes `examples` through `x` beside its dry twin, level-matched, and their lines for
/// the README.
#[allow(clippy::too_many_arguments)]
fn set_of(
    out: &Path,
    x: &(Vec<f32>, Vec<f32>),
    dry_name: &str,
    dry_title: &str,
    bpm: f64,
    start: i64,
    examples: Vec<Example>,
    readme: &mut String,
) {
    std::fs::create_dir_all(out).expect("out dir");
    write(
        &x.0,
        &x.1,
        &out.join(format!("{dry_name}.flac")),
        dry_title,
        "Vetus",
    );
    let _ = writeln!(readme, "- `{dry_name}.flac` — {dry_title} (dry)");
    let dry = rms(&x.0, &x.1);
    for (name, title, params, sweep) in examples {
        let (mut l, mut r) = through(x, &params, bpm, start, sweep);
        let g = dry / rms(&l, &r).max(1e-9);
        for v in l.iter_mut().chain(r.iter_mut()) {
            *v *= g;
        }
        let matched = 20.0 * (rms(&l, &r) / dry).log10();
        assert!(matched.abs() < 1.0, "{name}: {matched:.2} dB");
        write(&l, &r, &out.join(format!("{name}.flac")), title, "Vetus");
        let _ = writeln!(
            readme,
            "- `{name}.flac` — {title} (made up {:+.1} dB to the dry level)",
            20.0 * g.log10()
        );
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let samples = PathBuf::from(args.next().expect("the guitar WAV folder"));
    let out = PathBuf::from(args.next().unwrap_or_else(|| "target/sounds/vetus".into()));
    let mut readme = String::from(
        "# Vetus — sound examples\n\nEach example beside its dry twin, level-matched to it \
         within 1 dB (RMS), 48 kHz FLAC. Rendered by `cargo run --release -p cantus-ui \
         --example render_vetus` through Vetus's DSP on the song's clock.\n\n",
    );
    let g = guitar(&samples);
    readme.push_str("## guitar/ — the owner's progression (Am, F, C, G at 90 BPM)\n\n");
    set_of(
        &out.join("guitar"),
        &g,
        "00-guitar-di",
        "The DI guitar",
        90.0,
        0,
        vec![
            (
                "01-cassette",
                "A worn cassette: wow and flutter, tape hiss, the tape's band, a little age",
                vec![
                    ("wob.on", 1.0),
                    ("wow", 14.0),
                    ("flut", 5.0),
                    ("noise.on", 1.0),
                    ("crackle", 0.0),
                    ("hiss", 0.35),
                    ("tone.on", 1.0),
                    ("tone.hi", 10_000.0),
                    ("age.on", 1.0),
                    ("age.drop", 0.25),
                    ("age.wear", 0.3),
                ],
                None,
            ),
            (
                "02-8-bit-sampler",
                "An 8-bit sampler at 11 kHz, raw: the bits' grit and the aliasing",
                vec![
                    ("bits.on", 1.0),
                    ("bits.depth", 8.0),
                    ("rate.on", 1.0),
                    ("rate.hz", 11_025.0),
                ],
                None,
            ),
            (
                "03-am-radio",
                "AM radio: the broadcast band, μ-law grit, a little hum and hiss",
                vec![
                    ("tone.on", 1.0),
                    ("tone.medium", 1.0),
                    ("tone.lo", 120.0),
                    ("tone.hi", 4_500.0),
                    ("tone.res", 0.35),
                    ("bits.on", 1.0),
                    ("bits.depth", 9.0),
                    ("bits.mode", 2.0),
                    ("noise.on", 1.0),
                    ("crackle", 0.0),
                    ("hiss", 0.3),
                    ("hum", 0.25),
                ],
                None,
            ),
            (
                "04-gramophone",
                "A gramophone: the horn's band and resonances, crackle and pops, a slow wow",
                vec![
                    ("tone.on", 1.0),
                    ("tone.medium", 3.0),
                    ("tone.lo", 180.0),
                    ("tone.hi", 5_000.0),
                    ("noise.on", 1.0),
                    ("crackle", 0.6),
                    ("hiss", 0.3),
                    ("hiss.kind", 1.0),
                    ("wob.on", 1.0),
                    ("wow", 10.0),
                    ("wow.rate", 0.55),
                    ("flut", 2.0),
                ],
                None,
            ),
            (
                "05-telephone",
                "A telephone: its band and its carbon microphone, 8 kHz filtered",
                vec![
                    ("tone.on", 1.0),
                    ("tone.medium", 2.0),
                    ("tone.lo", 300.0),
                    ("tone.hi", 3_400.0),
                    ("tone.res", 0.3),
                    ("rate.on", 1.0),
                    ("rate.hz", 8_000.0),
                    ("rate.filter", 1.0),
                    ("bits.on", 1.0),
                    ("bits.depth", 8.0),
                    ("bits.mode", 2.0),
                ],
                None,
            ),
            (
                "06-character-sweep",
                "Character from 0 to 100 % over the eight bars, every module on: the record ages",
                vec![
                    ("bits.on", 1.0),
                    ("bits.depth", 16.0),
                    ("rate.on", 1.0),
                    ("rate.hz", 44_100.0),
                    ("rate.filter", 2.0),
                    ("wob.on", 1.0),
                    ("wow", 4.0),
                    ("flut", 1.0),
                    ("age.on", 1.0),
                    ("age.drop", 0.0),
                    ("age.wear", 0.0),
                    ("noise.on", 1.0),
                    ("crackle", 0.1),
                    ("hiss", 0.1),
                    ("tone.on", 1.0),
                    ("tone.lo", 30.0),
                    ("tone.hi", 18_000.0),
                ],
                Some((0.0, 1.0)),
            ),
        ],
        &mut readme,
    );
    readme.push_str("\nGuitar: Carbonthief, CC0.\n\n");
    let start = 16 * 36_000; // bar 5 at 80 BPM, 48 kHz (36 000 samples a beat)
    let drums =
        demo_part(&|n: &str| n.contains("kick") || n.contains("snare") || n.contains("hats"));
    readme.push_str("## drums/ — the Paper Lanterns demo's kit (80 BPM, bars 5–12)\n\n");
    set_of(
        &out.join("drums"),
        &drums,
        "00-drums-dry",
        "The demo's drums",
        80.0,
        start,
        vec![
            (
                "01-dusty-12-bit",
                "A 12-bit sampler at 26 kHz, truncating, filtered: the dusty drums of a 90s sampler",
                vec![
                    ("bits.on", 1.0),
                    ("bits.depth", 12.0),
                    ("bits.mode", 1.0),
                    ("rate.on", 1.0),
                    ("rate.hz", 26_040.0),
                    ("rate.filter", 1.0),
                    ("noise.on", 1.0),
                    ("crackle", 0.35),
                    ("hiss", 0.0),
                    ("hiss.kind", 1.0),
                ],
                None,
            ),
            (
                "02-crushed",
                "Crushed: 6 bits at 6 kHz, raw",
                vec![
                    ("bits.on", 1.0),
                    ("bits.depth", 6.0),
                    ("rate.on", 1.0),
                    ("rate.hz", 6_000.0),
                ],
                None,
            ),
        ],
        &mut readme,
    );
    let keys = demo_part(&|n: &str| n.contains("key") || n.contains("piano") || n.contains("tine"));
    readme.push_str("\n## keys/ — the Paper Lanterns demo's keys (80 BPM, bars 5–12)\n\n");
    set_of(
        &out.join("keys"),
        &keys,
        "00-keys-dry",
        "The demo's keys",
        80.0,
        start,
        vec![(
            "01-lofi-hip-hop",
            "Lo-fi hip-hop: a warbling tape, vinyl dust, the cassette's band, a little Character",
            vec![
                ("char", 0.25),
                ("wob.on", 1.0),
                ("wow", 16.0),
                ("wow.drift", 0.7),
                ("flut", 4.0),
                ("noise.on", 1.0),
                ("crackle", 0.45),
                ("hiss", 0.2),
                ("hiss.kind", 1.0),
                ("tone.on", 1.0),
                ("tone.lo", 70.0),
                ("tone.hi", 8_000.0),
                ("age.on", 1.0),
                ("age.drop", 0.15),
            ],
            None,
        )],
        &mut readme,
    );
    std::fs::write(out.join("README.md"), readme).expect("README");
    println!("{}", out.join("README.md").display());
}

//! Cribrum on a real guitar (fx side round): the owner's reference progression — Am, F, C,
//! G, one chord a bar at 90 BPM, struck on beats 1 and 3, eight bars — built from DI strums
//! read with Cantus's own decoder and resampled to 48 kHz with its resampler, then played
//! through Cribrum's DSP on the song's clock and written as FLAC beside the dry guitar
//! (`00-guitar-di.flac`, peak −6 dBFS), each processed file level-matched to it within 1 dB.
//! The samples are not in the repository: pass their folder.
//!
//! ```text
//! cargo run -p cantus-ui --example render_cribrum_guitar -- <SampleData dir> <out dir>
//! ```

use cantus_core::{EffectKind, EffectSpec};
use cantus_dsp::modfx_clock::Transport;
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format};
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;
const BPM: f64 = 90.0;
const BARS: usize = 8;

/// The chord of each bar (`README.md`).
const CHORDS: [&str; 4] = [
    "Sample07 (A Minor).flac",
    "Sample15 (F Major).flac",
    "Sample22 (C Major).flac",
    "Sample17 (G Major).flac",
];

/// One strum, stereo interleaved at 48 kHz.
fn strum(dir: &Path, file: &str) -> Vec<f32> {
    let d = cantus_engine::SampleCache::decode_source(&dir.join(file)).expect("decoded");
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

/// The progression: two strums a bar, each faded over 20 ms before the next.
fn progression(dir: &Path) -> (Vec<f32>, Vec<f32>) {
    let beat = (60.0 / BPM * f64::from(RATE)) as usize;
    let n = BARS * 4 * beat + RATE as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let fade = (0.02 * f64::from(RATE)) as usize;
    let strums: Vec<Vec<f32>> = CHORDS.iter().map(|c| strum(dir, c)).collect();
    for bar in 0..BARS {
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
    let peak = l.iter().chain(&r).fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    let g = cantus_dsp::math::db_to_gain(-6.0) / peak;
    for v in l.iter_mut().chain(r.iter_mut()) {
        *v *= g;
    }
    (l, r)
}

fn rms(l: &[f32], r: &[f32]) -> f32 {
    let s: f64 = l.iter().chain(r).map(|v| f64::from(*v) * f64::from(*v)).sum();
    (s / (l.len() + r.len()) as f64).sqrt() as f32
}

/// Through Cribrum with `params`, the song playing from bar 1.
fn through(l: &[f32], r: &[f32], params: &[(&str, f32)]) -> (Vec<f32>, Vec<f32>) {
    let mut spec = EffectSpec::new(EffectKind::AutoFilter);
    for (k, v) in params {
        spec.set(k, *v).unwrap_or_else(|| panic!("{k}"));
    }
    let mut u = cantus_dsp::cribrum::Cribrum::new(RATE as f32, cantus_engine::fx::fluctus::shape_at);
    u.set(&cantus_engine::fx::cribrum::settings(&spec));
    u.reset();
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    let mut at = 0usize;
    while at < ol.len() {
        let n = 480.min(ol.len() - at);
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: BPM,
            samples: at as i64,
        });
        u.process(&mut ol[at..at + n], &mut or[at..at + n], None);
        at += n;
    }
    (ol, or)
}

fn write(l: &[f32], r: &[f32], path: &Path, title: &str) {
    let samples: Vec<f32> = l.iter().zip(r).flat_map(|(a, b)| [*a, *b]).collect();
    let s = ExportSettings {
        format: Format::Flac,
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus (guitar: Carbonthief, CC0)".into(),
        album: "Cribrum on a guitar".into(),
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

fn main() {
    let mut args = std::env::args().skip(1);
    let samples = PathBuf::from(args.next().expect("the SampleData folder"));
    let out = PathBuf::from(args.next().unwrap_or_else(|| "target/sounds/cribrum-guitar".into()));
    std::fs::create_dir_all(&out).expect("out dir");
    let (l, r) = progression(&samples);
    write(&l, &r, &out.join("00-guitar-di.flac"), "The DI guitar");
    let dry = rms(&l, &r);
    let examples: [(&str, &str, Vec<(&str, f32)>); 4] = [
        (
            "01-auto-wah",
            "Auto-wah: the follower opens a narrow band-pass as the strums ring",
            vec![
                ("mix", 1.0),
                ("a.mode", 1.0),
                ("a.cut", 450.0),
                ("a.res", 0.72),
                ("env.att", 4.0),
                ("env.rel", 90.0),
                ("env.gain", 6.0),
                ("env.cut", 2.4),
            ],
        ),
        (
            "02-synced-sweep",
            "A resonant ladder swept once a bar, on the song",
            vec![
                ("mix", 1.0),
                ("a.model", 1.0),
                ("a.slope", 1.0),
                ("a.cut", 1_200.0),
                ("a.res", 0.55),
                ("os", 1.0),
                ("lfo.div", 3.0),
                ("lfo.cut", 1.8),
            ],
        ),
        (
            "03-vowel",
            "The vowel filter talking A-E-I-O on the half bar",
            vec![
                ("mix", 1.0),
                ("a.mode", 5.0),
                ("a.vowel", 2.0),
                ("a.res", 0.45),
                ("lfo.div", 5.0),
                ("lfo.vowel", 2.0),
            ],
        ),
        (
            "04-gated-steps",
            "The Gated steps preset: held steps on the bar, never a click",
            cantus_ui::views::fx::factory_presets(EffectKind::AutoFilter)
                .iter()
                .find(|(k, _)| *k == "preset.cribrum.gated_steps")
                .map(|(_, v)| v.iter().copied().filter(|(k, _)| *k != "out").collect())
                .expect("the preset"),
        ),
    ];
    for (name, title, params) in examples {
        // Level-matched to the dry guitar: rendered, measured, its output set, rendered again.
        let (wl, wr) = through(&l, &r, &params);
        let db = 20.0 * cantus_dsp::math::log10(dry / rms(&wl, &wr).max(1e-9));
        let mut p = params.clone();
        p.push(("out", db.clamp(-24.0, 24.0)));
        let (wl, wr) = through(&l, &r, &p);
        let off = 20.0 * cantus_dsp::math::log10(rms(&wl, &wr) / dry);
        println!("  {name}: out {db:+.1} dB, {off:+.2} dB from the dry level");
        write(&wl, &wr, &out.join(format!("{name}.flac")), title);
    }
    std::fs::write(
        out.join("README.md"),
        "# Cribrum on a guitar\n\nGuitar: Carbonthief, CC0 (*Strummed Guitar Chords*, DI).\n\n\
         The owner's reference progression: Am - F - C - G, one chord a bar at 90 BPM, struck on\n\
         beats 1 and 3, eight bars. `00-guitar-di.flac` is the dry guitar (peak -6 dBFS); each\n\
         other file is Cribrum on it, level-matched to the dry guitar within 1 dB:\n\n\
         - `01-auto-wah`: the follower opening a narrow band-pass (the *Cry wah* preset's settings)\n\
         - `02-synced-sweep`: a resonant ladder swept once a bar on the song\n\
         - `03-vowel`: the vowel filter talking A-E-I-O on the half bar (*Talking lead*)\n\
         - `04-gated-steps`: the *Gated steps* preset\n",
    )
    .expect("readme");
}

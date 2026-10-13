//! Renders "without / with" pairs for the website's effects gallery
//! (`cargo run --release -p cantus-ui --example render_fx_ab -- <out-dir>`).
//!
//! For each effect kind the demo songs already use it in context. The "without" file is the
//! same song, same window, with every slot of that kind bypassed; the "with" file is the song
//! as shipped. Among the demos that contain the kind, the one where the effect changes the
//! sound the most is kept (the difference, in dB below the dry signal, is printed), so the
//! pair is one a listener can actually hear.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cantus_core::{EffectKind, Project, Snapshot};
use cantus_engine::{render, RenderOptions};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format, Mp3Mode, Rate};
use cantus_ui::demos::{self, Demo};

/// (file stem on the site, kind)
const KINDS: [(&str, EffectKind); 7] = [
    ("eq", EffectKind::Eq),
    ("comp", EffectKind::Compressor),
    ("deesser", EffectKind::DeEsser),
    ("gate", EffectKind::Gate),
    ("limiter", EffectKind::Limiter),
    ("multiband", EffectKind::Multiband),
    ("space", EffectKind::Space),
];

/// Where each demo's whole band plays, in bars (start, end): the same windows the demo tests use.
fn window(id: &str) -> (i64, i64) {
    match id {
        "ballad" => (12, 20),
        "house" => (16, 24),
        "funk" => (12, 20),
        "lofi" => (20, 28),
        "ambient" => (16, 24),
        "trap" => (4, 12),
        _ => (0, 8),
    }
}

fn count(p: &Project, kind: EffectKind) -> usize {
    p.strips
        .iter()
        .flat_map(|s| s.effects.iter())
        .chain(p.master.effects.iter())
        .filter(|e| e.spec.kind == kind)
        .count()
}

fn bypass(p: &Project, kind: EffectKind) -> Project {
    let mut q = p.clone();
    for slot in q
        .strips
        .iter_mut()
        .flat_map(|s| s.effects.iter_mut())
        .chain(q.master.effects.iter_mut())
    {
        if slot.spec.kind == kind {
            slot.bypass = true;
        }
    }
    q
}

fn render_window(p: &Project, id: &str) -> Vec<f32> {
    let bar = p.time_signature.ticks_per_bar();
    let (a, b) = window(id);
    let r = render(
        Arc::new(Snapshot::compile(p, 0)),
        &RenderOptions {
            start: bar * a,
            end: Some(bar * b),
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .expect("the demo renders");
    assert_eq!(r.channels, 2);
    r.samples
}

/// How far the effect moves the sound: the residual's level against the dry signal, dB.
fn change_db(dry: &[f32], wet: &[f32]) -> f64 {
    let n = dry.len().min(wet.len());
    let (mut d, mut e) = (0.0f64, 0.0f64);
    for i in 0..n {
        let x = dry[i] as f64;
        let y = (wet[i] - dry[i]) as f64;
        d += x * x;
        e += y * y;
    }
    10.0 * ((e + 1e-20) / (d + 1e-20)).log10()
}

/// The shift, in sample frames, that best lines `wet` up with `dry` (searched over ±2000):
/// anything but 0 means bypassing the effect moved the song in time, and the pair would
/// flam when switched.
fn lag_frames(dry: &[f32], wet: &[f32]) -> i32 {
    let mono = |v: &[f32]| -> Vec<f32> { v.chunks(2).map(|c| c[0] + c[1]).take(96_000).collect() };
    let (a, b) = (mono(dry), mono(wet));
    let n = a.len().min(b.len());
    let mut best = (0i32, f64::MIN);
    for lag in -2000i32..=2000 {
        let mut acc = 0.0f64;
        for i in 2000..n.saturating_sub(2000) {
            acc += a[i] as f64 * b[(i as i32 + lag) as usize] as f64;
        }
        if acc > best.1 {
            best = (lag, acc);
        }
    }
    best.0
}

fn write(samples: Vec<f32>, path: &Path, title: &str) {
    let audio = Audio {
        rate: 48_000,
        channels: 2,
        samples,
    };
    let s = ExportSettings {
        format: Format::Mp3,
        mp3: Mp3Mode::Cbr(128),
        rate: Rate::Hz(44_100),
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus".into(),
        album: "Effects, with and without".into(),
        ..Default::default()
    };
    export_rendered(audio, &s, &tags, path, &Control::new()).expect("the file is written");
}

fn main() {
    let out: PathBuf = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: render_fx_ab <out-dir>");
    std::fs::create_dir_all(&out).expect("create the output folder");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let built: Vec<(&Demo, Project)> = demos::ALL.iter().map(|d| (d, (d.build)())).collect();

    std::thread::scope(|scope| {
        for (stem, kind) in KINDS {
            // An optional second argument limits the run to some stems (`eq,gate`).
            if let Some(only) = std::env::args().nth(2) {
                if !only.split(',').any(|s| s == stem) {
                    continue;
                }
            }
            let (built, out) = (&built, &out);
            scope.spawn(move || {
                let mut best: Option<(f64, &str, Vec<f32>, Vec<f32>)> = None;
                for (d, p) in built.iter().filter(|(_, p)| count(p, kind) > 0) {
                    let with = render_window(p, d.id);
                    let without = render_window(&bypass(p, kind), d.id);
                    let db = change_db(&without, &with);
                    println!(
                        "{stem}: {} -> change {:.1} dB below the dry mix, lag {} frames",
                        d.id,
                        db,
                        lag_frames(&without, &with)
                    );
                    // A pair that is out of step in time would flam when switched: skip it.
                    let lag = lag_frames(&without, &with);
                    if lag.abs() <= 2 && best.as_ref().is_none_or(|b| db > b.0) {
                        best = Some((db, d.id, without, with));
                    }
                }
                let (db, id, without, with) =
                    best.unwrap_or_else(|| panic!("no demo uses {kind:?}"));
                println!("{stem}: kept {id} ({db:.1} dB)");
                write(
                    without,
                    &out.join(format!("{stem}-without.mp3")),
                    &format!("{stem} without ({id})"),
                );
                write(
                    with,
                    &out.join(format!("{stem}-with.mp3")),
                    &format!("{stem} with ({id})"),
                );
            });
        }
    });
}

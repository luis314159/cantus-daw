//! Renders a short excerpt of each demo song to MP3, for the website's player
//! (`cargo run --release -p cantus-ui --example render_demos -- <out-dir>`).
//!
//! The excerpt is the window where the whole band plays (the same windows the demo tests
//! measure). Nothing is added to the mix: the songs are already mastered on their own
//! master channel, so the file is what the program plays.

use std::path::PathBuf;
use std::sync::Arc;

use cantus_core::Snapshot;
use cantus_engine::{render, RenderOptions};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format, Mp3Mode, Rate};
use cantus_ui::demos;

/// Where each demo's whole band plays, in bars (start, end).
fn window(id: &str) -> (i64, i64) {
    match id {
        "ballad" => (12, 20),
        "house" => (16, 24),
        "funk" => (12, 20),
        "lofi" => (20, 28),
        "ambient" => (16, 24),
        "trap" => (4, 12),
        "heart" => (32, 40),
        _ => (0, 8),
    }
}

fn main() {
    let out: PathBuf = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: render_demos <out-dir>");
    std::fs::create_dir_all(&out).expect("create the output folder");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);

    std::thread::scope(|scope| {
        for d in demos::ALL.iter() {
            let out = out.clone();
            scope.spawn(move || {
                let p = (d.build)();
                let bar = p.time_signature.ticks_per_bar();
                let (a, b) = window(d.id);
                let r = render(
                    Arc::new(Snapshot::compile(&p, 0)),
                    &RenderOptions {
                        start: bar * a,
                        end: Some(bar * b),
                        tail_seconds: 1.5,
                        ..Default::default()
                    },
                )
                .expect("the demo renders");
                let audio = Audio {
                    rate: r.sample_rate,
                    channels: r.channels,
                    samples: r.samples,
                };
                let s = ExportSettings {
                    format: Format::Mp3,
                    mp3: Mp3Mode::Cbr(128),
                    rate: Rate::Hz(44_100),
                    ..Default::default()
                };
                let tags = Tags {
                    title: d.id.to_string(),
                    artist: "Cantus".into(),
                    album: "Demo songs".into(),
                    comment: "Rendered by Cantus; excerpt of a demo song.".into(),
                    ..Default::default()
                };
                let path = out.join(format!("{}.mp3", d.id));
                let done = export_rendered(audio, &s, &tags, &path, &Control::new())
                    .expect("the file is written");
                println!(
                    "{}: {:.1} s, peak {:.1} dBFS -> {}",
                    d.id,
                    done.seconds,
                    done.peak_db,
                    path.display()
                );
            });
        }
    });
}

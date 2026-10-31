//! Screenshots of *Separate stems…* (round 34, ADR 0085) without a screen:
//! `cargo run -p cantus-ui --example shoot_stems -- <out-dir> [scale] [locale]`.
//! The panel before the model's first download (its size, licence and provenance), while a
//! separation runs, *Settings ▸ Models*, and the arrangement after a separation — the new
//! tracks under the original, their clips wearing the "AI" badge — in Cantus Night and
//! Paper. Nothing is downloaded: the stems are the welcome song's own files, renamed.

use cantus_content::models::ReadyModel;
use cantus_core::{AudioSource, AudioSourceId, Command, CommandBus, Origin, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::stems_ui::{self, run, Card, Separated, Stem, Target};
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

fn wav(path: &Path, seconds: f32, f: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let n = (seconds * 44_100.0) as usize;
    for i in 0..n {
        let t = i as f32 / 44_100.0;
        let env = (1.0 - (t * 2.0).fract()).powi(3);
        let v = ((2.0 * std::f32::consts::PI * f * t).sin() * env * 12_000.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

fn model() -> ReadyModel {
    ReadyModel {
        id: "htdemucs".into(),
        family: "ht_demucs".into(),
        graph: PathBuf::new(),
        library: PathBuf::new(),
        library_sha256: String::new(),
        files: Vec::new(),
        sources: ["drums", "bass", "other", "vocals"]
            .map(String::from)
            .to_vec(),
        sample_rate: 44_100,
        segment: 343_980,
        name: "HT-Demucs — 4 stems".into(),
        version: 1,
        licence: "MIT".into(),
    }
}

/// A song on one track, a 16-bar clip of it.
fn song(
    dir: &Path,
) -> (
    cantus_core::Project,
    cantus_core::TrackId,
    cantus_core::ClipId,
) {
    let file = dir.join("Song.wav");
    wav(&file, 32.0, 110.0);
    let mut bus = CommandBus::default();
    let mut track = bus.new_track("Song");
    track.color = cantus_core::Color::PALETTE[1];
    let tid = track.id;
    let id = AudioSourceId(bus.alloc_id());
    let mut clip = bus.new_clip("Song", Ticks::ZERO, Ticks::beats(32));
    clip.audio = Some(cantus_core::AudioRegion::of(id));
    let cid = clip.id;
    bus.execute(
        Command::Batch {
            label_key: "x".into(),
            commands: vec![
                Command::InsertTrack {
                    index: 0,
                    track: Box::new(track),
                },
                Command::InsertAudioSource {
                    source: AudioSource {
                        id,
                        name: "Song.wav".into(),
                        path: file.display().to_string(),
                        sample_rate: 44_100,
                        channels: 2,
                        frames: 32 * 44_100,
                        sha256: None,
                        recorded: false,
                    },
                },
                Command::InsertClip {
                    track: tid,
                    clip: Box::new(clip),
                },
            ],
        },
        Origin::User,
    )
    .unwrap();
    (bus.project().clone(), tid, cid)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let locale = match args.next().as_deref() {
        Some("es") => cantus_ui::i18n::Locale::Es,
        _ => cantus_ui::i18n::Locale::En,
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(locale);
    let code = locale.code();
    let pct = (scale * 100.0).round() as u32;
    let tmp = std::env::temp_dir().join(format!("cantus-shoot-stems-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    std::env::set_var("CANTUS_DATA_DIR", &tmp);
    let themes = [
        ("night", cantus_ui::theme::default_theme()),
        ("paper", cantus_ui::theme::paper()),
    ];
    for (tn, theme) in &themes {
        let context = || {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(theme, &ctx);
            ctx
        };
        let name = |what: &str| dir.join(format!("stems-{what}-{tn}-{code}-{pct}.png"));
        let (p, tid, cid) = song(&tmp);
        let new_app = || {
            let mut app = CantusApp::from_parts(Some(p.clone()), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app
        };
        // The panel before the first download, and while it separates.
        for what in ["panel-before", "panel-running"] {
            let mut app = new_app();
            let target = Target::Clips(vec![(tid, cid)]);
            if what == "panel-running" {
                let words = target.words(app.project());
                app.stems.runs.push(run::shown(
                    target.clone(),
                    model(),
                    run::Phase::Separating(0),
                    0.42,
                    words,
                ));
            }
            app.stems.dialog = Some(target);
            let ctx = context();
            let path = name(what);
            let mut frame = 0;
            shoot(
                &ctx,
                egui::vec2(900.0, 700.0),
                scale,
                6,
                Some(&path),
                |root| {
                    frame += 1;
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                        .show(root, |ui| {
                            if frame > 1 {
                                let ctx = ui.ctx().clone();
                                stems_ui::view::dialog(&mut app, &ctx);
                            }
                        });
                },
            );
            println!("wrote {}", path.display());
        }
        // Settings ▸ Models.
        {
            let mut app = new_app();
            cantus_ui::views::workspace_settings::open(
                &mut app,
                cantus_ui::views::workspace_settings::SettingsPage::Models,
            );
            let ctx = context();
            let path = name("settings-models");
            let mut frame = 0;
            shoot(
                &ctx,
                egui::vec2(1000.0, 760.0),
                scale,
                6,
                Some(&path),
                |root| {
                    frame += 1;
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                        .show(root, |ui| {
                            if frame > 1 {
                                let ctx = ui.ctx().clone();
                                cantus_ui::views::workspace_settings::window(&mut app, &ctx);
                            }
                        });
                },
            );
            println!("wrote {}", path.display());
        }
        // After a separation: the whole window.
        {
            let mut app = new_app();
            let stems_dir = tmp.join("Stems");
            std::fs::create_dir_all(&stems_dir).unwrap();
            let parts = ["drums", "bass", "other", "vocals"];
            let stems: Vec<Stem> = parts
                .iter()
                .enumerate()
                .map(|(i, part)| {
                    let path = stems_dir.join(format!("{part}.wav"));
                    wav(&path, 32.0, 55.0 * (i + 1) as f32);
                    Stem {
                        part: part.to_string(),
                        path,
                        frames: 32 * 44_100,
                        rate: 44_100,
                    }
                })
                .collect();
            let source = app.project().audio_pool[0].id;
            let card = Card {
                id: "htdemucs".into(),
                name: "HT-Demucs — 4 stems".into(),
                version: 1,
                licence: "MIT".into(),
            };
            let bus_name = Target::Clips(vec![(tid, cid)]).bus_name(app.project());
            let part = |p: &str| run::part_name(p);
            let planned = stems_ui::plan(
                &mut app.bus,
                &[(tid, cid)],
                &[Separated { source, stems }],
                &card,
                None,
                "2026-10-30",
                &part,
                &bus_name,
            )
            .unwrap();
            assert!(app.exec(planned.command));
            let ctx = context();
            let path = name("arrangement");
            shoot(
                &ctx,
                egui::vec2(1400.0, 860.0),
                scale,
                12,
                Some(&path),
                |root| {
                    app.frame(root);
                },
            );
            println!("wrote {}", path.display());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

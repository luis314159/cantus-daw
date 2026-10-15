//! Comping (round 28, ADR 0058), screenshotted without a screen:
//!
//! - `lanes`: a lead vocal with three takes, comped from all three — the seams' crossfades
//!   drawn as an X over the stretch they cross, each seam's handle, each window's take
//!   number, the comped ranges lit on the take lanes, the comp's chip (*Comp B*: a second
//!   comp was made) in the first lane's header; and a drum pair comped as a group;
//! - `menu`: the comp's menu open (the comps, new, rename, delete, flatten, merge, quick
//!   swipe, grouped comping, every seam's crossfade).
//!
//! Each at 75, 100 and 200 %, in English and Spanish, in Cantus Night and Paper.
//!
//! `cargo run --release -p cantus-ui --example shoot_comping -- <out-dir> [filter…]`
//! writes `comp-<what>-<theme>-<lang>-<scale>.png`.

use cantus_core::audio::AudioSource;
use cantus_core::{AudioSourceId, Command, Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::{raster, views, welcome_project, CantusApp};
use std::path::PathBuf;

fn bars(n: i64) -> Ticks {
    Ticks::beats(4 * n)
}

fn takes(app: &mut CantusApp, track: TrackId, name: &str, n: u8) {
    for k in 1..=n {
        let id = AudioSourceId(app.bus.alloc_id());
        app.exec(Command::InsertAudioSource {
            source: AudioSource {
                id,
                name: format!("{name} take {k}"),
                path: format!("C:/demo/Audio/{name} take {k}.wav"),
                sample_rate: 48_000,
                channels: 1,
                frames: 48_000 * 10,
                sha256: None,
                recorded: true,
            },
        });
        let mut clip = app
            .bus
            .new_clip(format!("{name} \u{b7} Take {k}"), bars(1), bars(4));
        clip.audio = Some(cantus_core::AudioRegion::of(id));
        clip.lane = k;
        app.exec(Command::InsertClip {
            track,
            clip: Box::new(clip),
        });
    }
    app.exec(Command::SetTrackTakesOpen {
        id: track,
        open: true,
    });
}

fn setup(app: &mut CantusApp) -> TrackId {
    app.add_audio_track();
    let vocal = app.selected_track.expect("the audio track");
    app.exec(Command::SetTrackName {
        id: vocal,
        name: "Lead vocal".into(),
    });
    app.exec(Command::SetTrackHeight {
        id: vocal,
        height: 1.4,
    });
    takes(app, vocal, "Lead vocal", 3);
    app.comp_swipe(vocal, 1, bars(1), bars(2) + Ticks::beats(2));
    app.comp_swipe(vocal, 3, bars(2) + Ticks::beats(2), bars(4));
    app.comp_swipe(vocal, 2, bars(4), bars(5));
    // A wider crossfade on the second seam, and a second comp playing.
    if let Some((_, r, _, _)) =
        cantus_ui::comping::seams(app.project().track(vocal).unwrap()).get(1)
    {
        let r = *r;
        app.set_seam_xfade(vocal, Some(r), Ticks(240));
    }
    app.duplicate_comp(vocal);
    app.arrangement.scroll_beats = 2.0;
    app.arrangement.px_per_beat = 34.0;
    vocal
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for what in ["lanes", "menu"] {
        for (theme, dark) in [("night", true), ("paper", false)] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [0.75f32, 1.0, 2.0] {
                    let name = format!(
                        "comp-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = egui::Context::default();
                    cantus_ui::design::install_fonts(&ctx);
                    cantus_ui::design::install_style(&ctx);
                    let t = if dark {
                        cantus_ui::theme_family::cantus_night()
                    } else {
                        cantus_ui::theme::paper()
                    };
                    cantus_ui::theme::apply(&t, &ctx);
                    let tmp = std::env::temp_dir().join("cantus-shoot-comp");
                    let mut app = CantusApp::from_parts(
                        Some(welcome_project()),
                        EngineHandle::dummy(),
                        false,
                    );
                    app.settings_path = tmp.join("settings.json");
                    app.layouts_path = tmp.join("layouts.json");
                    app.keymap_path = tmp.join("keymap.toml");
                    app.appearance_path = tmp.join("appearance.json");
                    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
                    app.appearance.assign(
                        app.appearance.mode,
                        if dark {
                            "preset:cantus_night".into()
                        } else {
                            "preset:paper".into()
                        },
                    );
                    app.attach(&ctx);
                    let vocal = setup(&mut app);
                    let path = dir.join(format!("{name}.png"));
                    raster::shoot(
                        &ctx,
                        egui::vec2(1280.0 / scale.max(1.0), 720.0 / scale.max(1.0)),
                        scale,
                        4,
                        Some(&path),
                        |root| {
                            egui::CentralPanel::default()
                                .frame(egui::Frame::NONE)
                                .show(root, |ui| views::arrangement::show(&mut app, ui));
                            if what == "menu" {
                                egui::Area::new(egui::Id::new("shoot-comp-menu"))
                                    .order(egui::Order::Foreground)
                                    .fixed_pos(egui::pos2(520.0, 120.0))
                                    .show(root.ctx(), |ui| {
                                        views::arrangement::comp::comp_menu(&mut app, ui, vocal);
                                    });
                            }
                        },
                    );
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}

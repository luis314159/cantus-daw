//! Screenshots of recording (ADR 0025), headless: armed tracks with their inputs, monitor
//! and input meters, take lanes open with a comp lit on them, the punch brace, a take
//! being recorded in red with its live waveform, the record options, the inspector's
//! input rows and the desk's record dots.
//!
//! `cargo run -p cantus-ui --example shoot_rec -- <out-dir> [filter…]`
//!
//! Writes `rec-arrangement-<theme>.png` (the arrangement alone, 1280 × 720),
//! `rec-window-<theme>.png` (the whole window, 1920 × 1080, the record options open) and
//! `rec-window-<theme>-es.png`; `rec-audio-track-<theme>.png` (the whole window with the
//! lead vocal, an audio track, in the inspector, and a pass that is losing frames).

use cantus_core::audio::{AudioSource, MonitorMode, TrackInput};
use cantus_core::{AudioSourceId, Command, LoopRegion, Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::{raster, views, welcome_project};
use std::path::PathBuf;

fn bars(n: i64) -> Ticks {
    Ticks::beats(4 * n)
}

/// A voice-like envelope of min/max peaks.
fn peaks(n: usize, seed: u32) -> Vec<(f32, f32)> {
    let mut x = seed;
    (0..n)
        .map(|i| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let noise = (x as f32 / u32::MAX as f32) * 0.25;
            let phrase = (i as f32 * 0.012).sin().abs();
            let syl = 0.5 + 0.5 * (i as f32 * 0.21).sin();
            let a = (0.75 * phrase * syl + noise * phrase).min(0.98);
            (-a * (0.85 + noise), a)
        })
        .collect()
}

/// A track with three takes on lanes 1–3 and a comp of all three on lane 0.
fn takes(app: &mut CantusApp, track: TrackId) {
    let mut sources = Vec::new();
    for k in 1..=3u8 {
        let id = AudioSourceId(app.bus.alloc_id());
        sources.push(id);
        app.exec(Command::InsertAudioSource {
            source: AudioSource {
                id,
                name: format!("Lead vocal take {k}"),
                path: format!("C:/demo/Audio/Lead vocal take {k}.wav"),
                sample_rate: 48_000,
                channels: 1,
                frames: 48_000 * 10,
                sha256: None,
                recorded: true,
            },
        });
        let mut clip = app
            .bus
            .new_clip(format!("Lead vocal \u{b7} Take {k}"), bars(1), bars(4));
        clip.audio = Some(cantus_core::AudioRegion::of(id));
        clip.lane = k;
        app.exec(Command::InsertClip {
            track,
            clip: Box::new(clip),
        });
    }
    app.comp_swipe(track, 1, bars(1), bars(2) + Ticks::beats(2));
    app.comp_swipe(track, 3, bars(2) + Ticks::beats(2), bars(4));
    app.comp_swipe(track, 2, bars(4), bars(5));
    app.exec(Command::SetTrackTakesOpen {
        id: track,
        open: true,
    });
}

fn setup(app: &mut CantusApp) {
    app.rec.demo_inputs = Some(vec![0.45, 0.12, 0.62, 0.55]);
    // The welcome project's first track records notes.
    if let Some(keys) = app.project().tracks.first().map(|t| t.id) {
        app.select_track(keys);
        app.exec(Command::SetTrackArmed {
            id: keys,
            armed: true,
        });
    }
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
    takes(app, vocal);
    app.add_audio_track();
    let guitar = app.selected_track.expect("the second audio track");
    app.exec(Command::SetTrackName {
        id: guitar,
        name: "Guitar DI".into(),
    });
    app.exec(Command::SetTrackHeight {
        id: guitar,
        height: 1.4,
    });
    app.set_track_input(guitar, Some(TrackInput::stereo(2)));
    app.set_monitor(guitar, MonitorMode::On);
    app.toggle_arm(guitar, false);
    // Punch over bars 9–12, two bars of count-in.
    let mut rs = app.project().recording.clone();
    rs.punch = Some(LoopRegion {
        start: bars(8),
        end: bars(12),
    });
    rs.punch_enabled = true;
    rs.count_in_bars = 2;
    app.set_record_settings(rs);
    // A take of the guitar being recorded from bar 7 (two bars of count-in before bar 9).
    let n = (8.0 * 2.0 * 48_000.0 / cantus_engine::PEAK_FRAMES as f64) as usize;
    app.demo_pass(bars(6), vec![(guitar, peaks(n, 7))]);
    // The inspector on the guitar's channel: its input rows.
    if let Some(strip) = app.project().track(guitar).and_then(|t| t.strip) {
        app.select_strip(strip);
    }
    app.arrangement.scroll_beats = 0.0;
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let keep = |name: &str| filters.iter().all(|f| name.contains(f.as_str()));
    for (theme, dark) in [("slate", true), ("paper", false)] {
        for (what, locale) in [
            ("arrangement", Locale::En),
            ("window", Locale::En),
            ("window-es", Locale::Es),
            ("audio-track", Locale::En),
        ] {
            let name = format!("rec-{}-{theme}", what.replace("-es", ""))
                + if locale == Locale::Es { "-es" } else { "" };
            if !keep(&name) {
                continue;
            }
            i18n::set_locale(locale);
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let tmp = std::env::temp_dir().join("cantus-shoot-rec");
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app.layouts_path = tmp.join("layouts.json");
            app.keymap_path = tmp.join("keymap.toml");
            app.appearance_path = tmp.join("appearance.json");
            app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
            app.attach(&ctx);
            setup(&mut app);
            if what == "audio-track" {
                // The audio track in the inspector (its input and its audio, no
                // instrument), and the clock warning of frames lost while recording.
                let vocal = app
                    .project()
                    .tracks
                    .iter()
                    .find(|t| t.name == "Lead vocal")
                    .map(|t| t.id);
                if let Some(v) = vocal {
                    app.select_track(v);
                }
                app.rec.dropped = 9_600;
            }
            let path = dir.join(format!("{name}.png"));
            if what == "arrangement" {
                raster::shoot(
                    &ctx,
                    egui::vec2(1280.0, 720.0),
                    1.0,
                    4,
                    Some(&path),
                    |root| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(root, |ui| views::arrangement::show(&mut app, ui));
                    },
                );
            } else {
                if what != "audio-track" {
                    views::record_options::open(&ctx);
                }
                raster::shoot(
                    &ctx,
                    egui::vec2(1920.0, 1080.0),
                    1.0,
                    6,
                    Some(&path),
                    |root| app.frame(root),
                );
            }
            println!("{}", path.display());
        }
    }
    i18n::set_locale(Locale::En);
}

//! Round 31's editing (ADRs 0070, 0071), screenshotted without a screen:
//!
//! - `xfade`: a vocal track of four clips — two crossing over an overlap on an S-curve (the
//!   later one selected), two overlapping summed (no crossfade: dashed edges);
//! - `envelope`: the clips' gain envelopes shown, one clip's line with a dip, a hold and a
//!   bend, the clip selected (its points and bend handles);
//! - `editor`: the crossfade editor open over the overlap;
//! - `settings`: Settings ▸ Editing;
//! - `insert`: Concentus inserted from the slot menu — its editor names *Juno I*.
//!
//! In Cantus Night and Paper, English and Spanish, at 75, 100 and 200 %.
//!
//! `cargo run -p cantus-ui --example shoot_clipfx -- <out-dir> [filter…]` writes
//! `clipfx-<what>-<theme>-<lang>-<scale>.png`.

use cantus_core::clip_gain::{GainEnvelope, GainPoint};
use cantus_core::crossfade::XfadeCurve;
use cantus_core::{AudioSource, AudioSourceId, Command, EffectKind, Shape, Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::{raster, views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

/// A take that reads as a voice: phrases of a decaying tone with breaths between.
fn take(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let n = 48_000 * 16;
    for i in 0..n {
        let t = i as f32 / 48_000.0;
        let phrase = (t * 1.3).fract();
        let env = if phrase < 0.8 {
            (1.0 - phrase / 0.8).powf(0.6) * (0.6 + 0.4 * (t * 0.7).sin().abs())
        } else {
            0.04
        };
        let s = (t * 220.0 * std::f32::consts::TAU).sin() * 0.7
            + (t * 440.0 * std::f32::consts::TAU).sin() * 0.2;
        w.write_sample((s * env * 0.9 * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
}

fn clip(
    app: &mut CantusApp,
    track: TrackId,
    src: AudioSourceId,
    at: i64,
    len: i64,
    off: i64,
) -> cantus_core::ClipId {
    let mut c = app
        .bus
        .new_clip("Lead vocal", Ticks::beats(at), Ticks::beats(len));
    c.audio = Some(cantus_core::AudioRegion::of(src));
    c.offset = Ticks::beats(off);
    let id = c.id;
    app.exec(Command::InsertClip {
        track,
        clip: Box::new(c),
    });
    id
}

struct Scene {
    vocal: TrackId,
    crossed: cantus_core::ClipId,
    enveloped: cantus_core::ClipId,
}

fn setup(app: &mut CantusApp, dir: &Path) -> Scene {
    let file = dir.join("lead.wav");
    if !file.exists() {
        take(&file);
    }
    app.add_audio_track();
    let vocal = app.selected_track.expect("the audio track");
    app.exec(Command::SetTrackName {
        id: vocal,
        name: "Lead vocal".into(),
    });
    app.exec(Command::SetTrackHeight {
        id: vocal,
        height: 2.2,
    });
    let src = AudioSourceId(app.bus.alloc_id());
    app.exec(Command::InsertAudioSource {
        source: AudioSource {
            id: src,
            name: "Lead vocal".into(),
            path: file.to_string_lossy().into_owned(),
            sample_rate: 48_000,
            channels: 1,
            frames: 48_000 * 16,
            sha256: None,
            recorded: true,
        },
    });
    // Two crossed by the automatic crossfade (S-curve), two summed.
    app.set_auto_crossfade_curve(XfadeCurve::SCurve);
    let enveloped = clip(app, vocal, src, 1, 6, 0);
    let crossed = clip(app, vocal, src, 6, 6, 9);
    app.set_auto_crossfade(false);
    clip(app, vocal, src, 13, 5, 2);
    let summed = clip(app, vocal, src, 17, 5, 12);
    // A fade in on the summed one, under its overlap's square (review 1: kept apart).
    if let Some(mut a) = app
        .project()
        .track(vocal)
        .and_then(|t| t.clip(summed))
        .and_then(|c| c.audio.clone())
    {
        a.fade_in = Ticks(480);
        app.exec(Command::SetClipAudio {
            track: vocal,
            clip: summed,
            audio: Some(a),
        });
    }
    app.settings.auto_crossfade = true;
    // A line on the first: a dip, a hold, a bend up.
    let mut pts = vec![
        GainPoint::new(Ticks::beats(0), 0.0),
        GainPoint::new(Ticks::beats(1), -3.0),
        GainPoint::new(Ticks(1_920 + 480), -14.0),
        GainPoint::new(Ticks::beats(3), -14.0),
        GainPoint::new(Ticks::beats(4), 2.0),
        GainPoint::new(Ticks::beats(5), 0.0),
    ];
    pts[2].shape = Shape::Hold;
    pts[3].tension = 0.55;
    let mut a = app
        .project()
        .track(vocal)
        .and_then(|t| t.clip(enveloped))
        .and_then(|c| c.audio.clone())
        .unwrap();
    a.gain_env = Some(Box::new(GainEnvelope::new(pts)));
    app.exec(Command::SetClipAudio {
        track: vocal,
        clip: enveloped,
        audio: Some(a),
    });
    app.arrangement.scroll_beats = 0.0;
    app.arrangement.px_per_beat = 52.0;
    // The waveform's peaks read before the first frame.
    if let Some(s) = app.project().audio_source(src).cloned() {
        let base = app.audio_base().map(Path::to_path_buf);
        let _ = app.peaks.lookup(&s, base.as_deref(), None);
    }
    let _ = app.peaks.wait_idle(std::time::Duration::from_secs(30));
    Scene {
        vocal,
        crossed,
        enveloped,
    }
}

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = if dark {
        cantus_ui::theme_family::cantus_night()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&t, &ctx);
    ctx
}

fn app_for(ctx: &egui::Context, dark: bool) -> CantusApp {
    let tmp = std::env::temp_dir().join("cantus-shoot-clipfx");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
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
    app.attach(ctx);
    app
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let media = std::env::temp_dir().join("cantus-shoot-clipfx-media");
    std::fs::create_dir_all(&media).expect("media dir");
    for what in ["xfade", "envelope", "editor", "settings", "insert"] {
        for (theme, dark) in [("night", true), ("paper", false)] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [0.75f32, 1.0, 2.0] {
                    let name = format!(
                        "clipfx-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(dark);
                    let mut app = app_for(&ctx, dark);
                    let scene = setup(&mut app, &media);
                    app.settings.show_clip_gain = what == "envelope";
                    if what == "envelope" {
                        app.select_clip(scene.vocal, scene.enveloped);
                    }
                    if what == "settings" {
                        views::workspace_settings::open(
                            &mut app,
                            views::workspace_settings::SettingsPage::Editing,
                        );
                    }
                    if what == "insert" {
                        if let Some(strip) = app.project().strips.first().map(|s| s.id) {
                            app.add_effect(strip, EffectKind::Chorus);
                            if let Some(e) = app
                                .project()
                                .effects_of(strip)
                                .and_then(|e| e.last())
                                .map(|e| e.id)
                            {
                                app.open_effect_editor(strip, e);
                            }
                        }
                        app.fx.pos = Some(egui::pos2(40.0, 40.0));
                    }
                    let size = egui::vec2(1280.0 / scale.max(1.0), 720.0 / scale.max(1.0));
                    if what == "xfade" || what == "editor" {
                        app.select_clip(scene.vocal, scene.crossed);
                    }
                    let path = dir.join(format!("{name}.png"));
                    let crossed = (scene.vocal, scene.crossed);
                    raster::shoot(&ctx, size, scale, 4, Some(&path), |root| {
                        if what == "editor" && app.arrangement.xfade_editor().is_none() {
                            if let Some(x) =
                                app.arrangement.audio_handles(crossed).and_then(|h| h.xfade)
                            {
                                app.arrangement
                                    .open_xfade_editor(crossed, x.square.left_bottom());
                            }
                        }
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(root, |ui| views::arrangement::show(&mut app, ui));
                        let ctx = root.ctx().clone();
                        if what == "settings" {
                            views::workspace_settings::window(&mut app, &ctx);
                        }
                        if what == "insert" {
                            views::fx::window(&mut app, &ctx);
                        }
                    });
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}

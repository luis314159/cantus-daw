//! The stage drawn without a screen (round 35, ADR 0091), on the program's opening song
//! played by the real processor: Trino floating over the whole window mid-dance (the hero
//! picture; also in Spanish); the same with the pointer on it (its frame and grips); the
//! Stage tab with a dancer docked under the inspector, and empty — in Slate (Cantus Night)
//! and Paper, at 100 % (and 200 % with a third argument `200`). And the companion window's
//! own pass (`companion`): its sizes, backdrops (transparent over a picture, the theme's
//! stage, a chroma key), visualisers and streaming mode, hovered, with its menu open.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_stage -- [out-dir] [case] [200]
//! ```

use cantus_core::Ticks;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::raster::shoot_at;
use cantus_ui::stage::{Surface, ViewMode};
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

const SIZE: egui::Vec2 = egui::vec2(1600.0, 900.0);

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme_family::cantus_night()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    cantus_ui::views::workspace_style::install(&ctx);
    ctx
}

/// Where a dancer shows and stands: mode, x, y, size.
type Place = (ViewMode, f32, f32, f32);

/// How it dances: its move (0: to the beat, else `Move::ALL[i - 1]` — 13 is *Arms up*),
/// energy and mood (2: excited).
const DANCE: [(&str, f32); 3] = [("move", 13.0), ("energy", 1.0), ("mood", 2.0)];

/// A number from the environment (`CANTUS_SHOT_<name>`), else `default`: to try a pose or a
/// place without building again.
fn tune(name: &str, default: f32) -> f32 {
    std::env::var(format!("CANTUS_SHOT_{name}"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The program's own opening song (the house demo's drop, its whole desk) played by the real
/// processor to bar 5, beat 2 and a half — its meters live — with a dancer track added
/// (Trino, the default character) placed as `place` says (none: no dancer). The status line
/// is cleared and the audio chip shows an interface (none is open here).
fn app(place: Option<Place>, docked_tab: bool) -> (CantusApp, Processor) {
    use cantus_core::inst::InstrumentKind;
    let p = cantus_ui::demos::startup_project();
    // The song's own dancer (the demo has one: Trino), placed and dancing as the shot
    // wants; hidden for the empty stage.
    let dancer = p
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Dancer)
        })
        .map(|t| t.id)
        .expect("the opening song has its dancer");
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(p), engine, false);
    let values: Vec<(&str, f32)> = match place {
        Some((mode, x, y, size)) => [
            ("x", x),
            ("y", y),
            ("size", size),
            ("view.mode", mode.value()),
        ]
        .into_iter()
        .chain(DANCE.map(|(k, v)| (k, tune(&k.to_uppercase(), v))))
        .collect(),
        None => vec![("view.mode", ViewMode::Hidden.value())],
    };
    for (k, v) in values {
        app.set_inst_value(dancer, k, v, false);
    }
    if place.is_some() {
        app.select_track(dancer);
    }
    app.sync_engine();
    app.status.text.clear();
    // A running interface, as on a desk (no "no audio" chip in a picture of the program).
    app.audio_setup.demo = Some(cantus_ui::audio_setup::demo::live_on_umc());
    // Played for half a second up to the moment: the meters and the dancer as they are
    // while the song runs.
    let beat = tune("BEAT", 17.5) as f64;
    let bpm = app.project().tempo_bpm.max(1.0);
    let from = beat - BLOCK as f64 * PRE_ROLL as f64 / 48_000.0 * bpm / 60.0;
    app.engine
        .seek(Ticks((cantus_core::PPQ as f64 * from).round() as i64));
    app.engine.play();
    let mut buf = vec![0f32; BLOCK * 2];
    for _ in 0..PRE_ROLL {
        proc.process(&mut buf, 2);
        app.engine.poll();
    }
    if docked_tab {
        app.workspace.reveal(Pane::Stage);
    }
    (app, proc)
}

/// The processor's block in the shots, and how many play before the picture.
const BLOCK: usize = 512;
const PRE_ROLL: usize = 47;

fn window(
    out: &Path,
    name: &str,
    dark: bool,
    ppp: f32,
    (mut app, mut proc): (CantusApp, Processor),
    hover: bool,
) {
    let mut buf = vec![0f32; BLOCK * 2];
    let ctx = context(dark);
    let theme = if dark { "slate" } else { "paper" };
    let path = out.join(format!("stage-{name}-{theme}-{}.png", (ppp * 100.0) as u32));
    let at = std::cell::Cell::new(None);
    shoot_at(
        &ctx,
        SIZE,
        ppp,
        6,
        Some(&path),
        |frame| if hover && frame >= 3 { at.get() } else { None },
        |root| {
            // A block a frame: the meters stay lit, the song hardly moves.
            proc.process(&mut buf, 2);
            app.frame(root);
            if hover {
                at.set(
                    app.stage
                        .dancers()
                        .first()
                        .and_then(|d| {
                            app.stage
                                .body(d.track, Surface::Float)
                                .or_else(|| app.stage.body(d.track, Surface::Dock))
                        })
                        .map(|b| b.center()),
                );
            }
        },
    );
    println!("{}", path.display());
}

/// The hero shot's dancer: floating over the mixer's empty middle, large enough to read on
/// a landing page.
fn hero() -> Place {
    (
        ViewMode::Floating,
        tune("X", 0.54),
        tune("Y", 0.95),
        tune("SIZE", 1.6),
    )
}

fn main() {
    // The app and its processor are passed around by value (a shot's convenience): a
    // thread with room for them.
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(run)
        .expect("a thread")
        .join()
        .expect("the shots");
}

fn run() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-stage"));
    let case = args.next();
    std::fs::create_dir_all(&out).expect("the output folder");
    let wants = |c: &str| case.as_deref().is_none_or(|w| w == c);
    let ppps: &[f32] = if args.next().as_deref() == Some("200") {
        &[1.0, 2.0]
    } else {
        &[1.0]
    };
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for &dark in &[true, false] {
        for &ppp in ppps {
            if wants("float") {
                window(&out, "float", dark, ppp, app(Some(hero()), false), false);
            }
            if wants("hover") {
                let a = app(Some((ViewMode::Floating, 0.55, 0.9, 1.2)), false);
                window(&out, "hover", dark, ppp, a, true);
            }
            if wants("dock") {
                let a = app(Some((ViewMode::Docked, 0.5, 0.86, 1.0)), true);
                window(&out, "dock", dark, ppp, a, false);
            }
            if wants("empty") {
                window(&out, "empty", dark, ppp, app(None, true), false);
            }
        }
    }
    // One in Spanish: the hero, as the Spanish README and landing page show it.
    if wants("float") || wants("es") {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
        window(&out, "float-es", true, 1.0, app(Some(hero()), false), false);
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    }
    if wants("companion") {
        for dark in [true, false] {
            companion_shots(&out, dark);
        }
    }
    if wants("video") {
        video_shots(&out);
    }
    if wants("native") {
        native_shots(&out);
    }
}

/// The companion as Windows shows it: its own pixels (true alpha), here laid over a picture
/// to see the soft shadow and the glass through, hovered and not.
fn native_shots(out: &Path) {
    use cantus_ui::stage::companion::{Backdrop, StageSettings, VisSnap};
    use cantus_ui::stage::companion_paint::{draw, Scene};
    use cantus_ui::stage::vis_video::Setup;
    let _ = context(true);
    let ch = cantus_dance::cast::default_character();
    let frame = cantus_dance::DanceFrame {
        playing: true,
        beats: 9.5,
        seconds: 4.75,
        bar: 2,
        bar_start_beats: 8.0,
        ..Default::default()
    };
    let pose = cantus_dance::pose_at(&frame, ch.as_ref(), 0.0);
    let mut vis = VisSnap {
        bar: 3,
        beat: 1,
        beat_phase: 0.2,
        playing: true,
        song: std::sync::Arc::from("Morning Light"),
        ..Default::default()
    };
    let nb = vis.bands.len() as f32;
    for (i, b) in vis.bands.iter_mut().enumerate() {
        let x = i as f32 / nb;
        *b = (0.85 - 0.6 * x + 0.18 * (x * 19.0).sin()).clamp(0.05, 1.0);
    }
    for (name, backdrop, hover) in [
        ("transparent", Backdrop::Transparent, false),
        ("transparent-hover", Backdrop::Transparent, true),
        ("theme", Backdrop::Theme, false),
    ] {
        let st = StageSettings {
            companion_open: true,
            backdrop,
            ..Default::default()
        };
        let setup = Setup::from_theme("Morning Light", false, false);
        let dancers = vec![(ch.clone(), pose)];
        let scene = Scene {
            settings: &st,
            setup: &setup,
            dancers: &dancers,
            place_feet: true,
            vis: &vis,
            hover,
            scale: 1.25,
        };
        let mut pm = tiny_skia::Pixmap::new(500, 575).unwrap();
        draw(&mut pm, &scene);
        // Its own pixels too (true alpha), to lay over any picture (the README's, the
        // site's). r37:chars
        let alpha = out.join(format!("native-{name}-alpha.png"));
        pm.save_png(&alpha).expect("written");
        // Over a picture: a desktop or a game under the window.
        let mut under = tiny_skia::Pixmap::new(500, 575).unwrap();
        for y in 0..575u32 {
            for x in 0..500u32 {
                let c = if ((x / 25) + (y / 25)) % 2 == 0 {
                    [70u8, 110, 150]
                } else {
                    [120u8, 80, 160]
                };
                let i = ((y * 500 + x) * 4) as usize;
                under.data_mut()[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        under.draw_pixmap(
            0,
            0,
            pm.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        let path = out.join(format!("native-{name}.png"));
        under.save_png(&path).expect("written");
        println!("{}", path.display());
    }
}

/// Video frames with the light visualiser: a chord's spectrum, vertical on the theme's
/// stage with the title line, wide and transparent without it (the song's chip instead).
fn video_shots(out: &Path) {
    use cantus_dance::render::{Background, Stage};
    use cantus_ui::stage::vis_video::{analyse, with_title, Setup, VideoVis};
    use cantus_video::dance::{Dancer, SongFrames};
    use cantus_video::export::FrameSource;
    let _ = context(true);
    let sr = 48_000u32;
    let samples: Vec<f32> = (0..sr as usize * 4)
        .flat_map(|i| {
            let t = i as f32 / sr as f32;
            let mut v = 0.0;
            for (k, hz) in [
                55.0f32, 110.0, 220.0, 277.2, 329.6, 659.3, 1318.5, 2637.0, 5274.0,
            ]
            .iter()
            .enumerate()
            {
                v += (0.3 / (k as f32 + 1.0)) * (2.0 * std::f32::consts::PI * hz * t).sin();
            }
            [v, v]
        })
        .collect();
    let raw = analyse(&samples, 2, sr);
    let tempo = cantus_core::tempo::TempoMap::constant(120.0, Default::default());
    let theme = cantus_ui::kit::Style::current().t.panel;
    let cases = [
        (
            "vertical-stage",
            1080u32,
            1920u32,
            true,
            Background::Solid([theme.r(), theme.g(), theme.b(), 255]),
        ),
        (
            "wide-transparent",
            1920,
            1080,
            false,
            Background::Transparent,
        ),
    ];
    for (name, w, h, title_on, bg) in cases {
        let setup = Setup::from_theme("Morning Light", title_on, false);
        let vis = VideoVis::new(setup, raw.clone(), tempo.clone(), 0.0);
        let title = title_on.then(|| {
            let font = cantus_ui::design::score_faces().bold;
            Box::new(move |_n: u32, _t: f64, pm: &mut tiny_skia::Pixmap| {
                let (w, h) = (pm.width() as f32, pm.height() as f32);
                let px = (w.min(h) * 0.045).max(8.0);
                cantus_video::title::draw(
                    pm,
                    font,
                    "Morning Light · 120 BPM",
                    px,
                    h - px * 1.4,
                    px,
                    w - 2.0 * px,
                    [245, 245, 248, 235],
                    None,
                );
            }) as Box<cantus_video::dance::Overlay>
        });
        let frames = SongFrames {
            dancers: vec![Dancer {
                character: cantus_dance::cast::default_character(),
                timeline: Vec::new(),
            }],
            stage: Stage::new(w, h, bg),
            start_seconds: 0.0,
            fps: 30,
            overlay: with_title(Some(vis), title),
        };
        let mut pm = tiny_skia::Pixmap::new(w, h).expect("a frame");
        frames.draw(45, &mut pm);
        let path = out.join(format!("video-{name}.png"));
        pm.save_png(&path).expect("written");
        println!("{}", path.display());
    }
}

/// A picture behind a see-through window: what a stream shows under the overlay.
fn backdrop_picture(ui: &egui::Ui, r: egui::Rect) {
    let p = ui.painter();
    let n = 12;
    for i in 0..n {
        for j in 0..n * 2 {
            let cell = egui::Rect::from_min_size(
                egui::pos2(
                    r.left() + r.width() * i as f32 / n as f32,
                    r.top() + r.height() * j as f32 / (2 * n) as f32,
                ),
                egui::vec2(
                    r.width() / n as f32 + 1.0,
                    r.height() / (2 * n) as f32 + 1.0,
                ),
            );
            let t = (i + j) as f32 / (3 * n) as f32;
            let c = egui::Color32::from_rgb(
                (40.0 + 120.0 * t) as u8,
                (70.0 + 60.0 * (1.0 - t)) as u8,
                (110.0 + 90.0 * t) as u8,
            );
            let c = if (i + j) % 2 == 0 {
                c
            } else {
                c.gamma_multiply(0.85)
            };
            p.rect_filled(cell, 0.0, c);
        }
    }
}

fn companion_shots(out: &Path, dark: bool) {
    use cantus_ui::stage::companion::{Backdrop, Shared, StageSettings, VisMode, VisSnap};
    use std::sync::{Arc, Mutex};
    let theme = if dark { "slate" } else { "paper" };
    let character = cantus_dance::cast::default_character();
    let cases: [(&str, [f32; 2], Backdrop, VisMode, bool); 8] = [
        (
            "transparent",
            [400.0, 460.0],
            Backdrop::Transparent,
            VisMode::Spectrum,
            false,
        ),
        (
            "theme",
            [400.0, 460.0],
            Backdrop::Theme,
            VisMode::Spectrum,
            false,
        ),
        (
            "green",
            [400.0, 460.0],
            Backdrop::Green,
            VisMode::Level,
            false,
        ),
        (
            "vertical",
            [540.0, 960.0],
            Backdrop::Transparent,
            VisMode::Spectrum,
            false,
        ),
        (
            "hover",
            [400.0, 460.0],
            Backdrop::Theme,
            VisMode::Spectrum,
            true,
        ),
        (
            "menu",
            [400.0, 460.0],
            Backdrop::Theme,
            VisMode::Spectrum,
            true,
        ),
        // Streaming mode: nothing but the stage, over what the stream shows under it, and
        // on a chroma key for a capture that keeps no alpha.
        (
            "streaming-transparent",
            [540.0, 600.0],
            Backdrop::Transparent,
            VisMode::Spectrum,
            false,
        ),
        (
            "streaming-green",
            [540.0, 600.0],
            Backdrop::Green,
            VisMode::Spectrum,
            false,
        ),
    ];
    for (name, size, backdrop, vis, hover) in cases {
        for ppp in [1.0f32] {
            let ctx = context(dark);
            let mut bands = [0.0f32; cantus_ui::stage::vis::BANDS];
            let nb = bands.len() as f32;
            for (i, b) in bands.iter_mut().enumerate() {
                let x = i as f32 / nb;
                *b = (0.85 - 0.6 * x + 0.18 * (x * 19.0).sin()).clamp(0.05, 1.0);
            }
            let mut frame = cantus_dance::DanceFrame {
                playing: true,
                beats: 9.5,
                seconds: 4.75,
                bar: 2,
                bar_start_beats: 8.0,
                ..Default::default()
            };
            let ix = |k: &str| {
                cantus_core::inst::dancer::KEYS
                    .iter()
                    .position(|x| *x == k)
                    .unwrap()
            };
            frame.values[ix("y")] = if vis == VisMode::Off { 0.92 } else { 0.80 };
            for (k, v) in DANCE {
                frame.values[ix(k)] = v;
            }
            let pose = cantus_dance::pose_at(&frame, character.as_ref(), 0.0);
            let mut sh = Shared::default();
            sh.settings = StageSettings {
                companion_open: true,
                streaming: name.starts_with("streaming"),
                size,
                backdrop,
                vis,
                ..Default::default()
            };
            sh.frame = frame;
            sh.dancers = vec![(character.clone(), pose)];
            sh.vis = VisSnap {
                bands,
                level: 0.72,
                bar: 3,
                beat: 1,
                beat_phase: 0.2,
                playing: true,
                song: Arc::from("Morning Light"),
                bpm: 120.0,
                ..Default::default()
            };
            sh.character_name = Arc::from(character.info().name.as_str());
            let shared = Arc::new(Mutex::new(sh));
            let path = out.join(format!(
                "companion-{name}-{theme}-{}.png",
                (ppp * 100.0) as u32
            ));
            let centre = egui::pos2(size[0] * 0.5, size[1] * 0.5);
            cantus_ui::raster::shoot_events(
                &ctx,
                egui::vec2(size[0], size[1]),
                ppp,
                5,
                Some(&path),
                |f, _| {
                    let mut ev = Vec::new();
                    if hover {
                        ev.push(egui::Event::PointerMoved(centre));
                    }
                    if name == "menu" && f == 2 {
                        for pressed in [true, false] {
                            ev.push(egui::Event::PointerButton {
                                pos: centre,
                                button: egui::PointerButton::Secondary,
                                pressed,
                                modifiers: Default::default(),
                            });
                        }
                    }
                    ev
                },
                |root| {
                    let r = root.max_rect();
                    if backdrop == Backdrop::Transparent {
                        backdrop_picture(root, r);
                    }
                    cantus_ui::stage::companion_view::show(
                        root,
                        egui::ViewportClass::Deferred,
                        &shared,
                    );
                },
            );
            println!("{}", path.display());
        }
    }
}

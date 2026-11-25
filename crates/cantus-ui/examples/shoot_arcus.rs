//! Arcus's window without a screen (round 42, ADR 0157): its design sheet — Play with the
//! cello bowed, the double bass bowed, the cello plucked and its tremolo, in both pictures
//! (Drawn, Photographic), Night and Paper, at 100 % and 200 % (`--edit` adds Edit's
//! Schelleng diagram); `--strip` the live part frame by frame; `--scene` the picture
//! alone (`--zoom=1,3`, `--only=<part of a name>[+<another>]`). `CANTUS_SHOOT_LANG=es` for Spanish.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_arcus -- <dir>
//! ```

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::shell::Shell;
use cantus_ui::raster::shoot;
use cantus_ui::scene::bowed_hold::{Hold, Holder}; // r42:arcus-upper
use cantus_ui::views::inst::View;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::default_theme()
    } else {
        cantus_ui::theme::default_light_theme()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

/// r42:arcus-look — `--scene`: the instrument's picture alone (the cello and the bass, Drawn
/// and Photographic, Night and Paper) at the Play view's size, for the design sheet.
fn scenes(dir: &std::path::Path, zoom: f32, only: Option<&str>, holds: &[Hold]) {
    use cantus_ui::scene::{self, bowed::Bowed, Room, SceneBackend, SceneView, Subject};
    cantus_ui::views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    let size = egui::vec2(590.0, 300.0);
    for (name, b) in [
        ("cello", Bowed::Cello),
        ("bass", Bowed::Bass),
        ("violin", Bowed::Violin), // r42:arcus-upper
        ("viola", Bowed::Viola),   // r42:arcus-upper
    ] {
        for (look, be, &hold) in [
            ("drawn", SceneBackend::Vector),
            ("photo", SceneBackend::PerPixel),
        ]
        .into_iter()
        .flat_map(|(l, be)| holds.iter().map(move |h| (l, be, h)))
        {
            for (theme, dark) in [("night", true), ("paper", false)] {
                let tag = format!("{name}{}-{look}-{theme}", hold_tag(hold));
                if only.is_some_and(|o| !o.split('+').all(|p| tag.contains(p))) {
                    continue;
                }
                scene::backend::set_default(be);
                let ctx = context(dark);
                let view = SceneView {
                    subject: Subject::Bowed {
                        instrument: b,
                        holder: Holder::of(hold), // r42:arcus-upper
                    },
                    room: Room::Studio,
                    look: Default::default(),
                    dark,
                    selected: None,
                    hovered: None,
                    sounding: Vec::new(),
                    accent: egui::Color32::TRANSPARENT,
                    ink: egui::Color32::TRANSPARENT,
                };
                let mut sc = scene::Scene::default();
                let path = dir.join(format!("scene-{tag}-{}.png", (zoom * 100.0) as u32));
                let t0 = std::time::Instant::now();
                shoot(&ctx, size, zoom, 3, Some(&path), |ui| {
                    let r = ui.max_rect();
                    let _ = sc.paint(ui.painter(), r, &view);
                });
                println!(
                    "{} · {:.0} ms",
                    path.display(),
                    t0.elapsed().as_secs_f64() * 1e3
                );
            }
        }
    }
    scene::backend::set_default(SceneBackend::Vector);
}

/// r42:arcus-look — `--strip`: the live part moving, frame by frame: a bowed note (its
/// attack, the bow sliding down, an up-bow) and a pizzicato (the pluck and its ring dying),
/// six frames each, the picture alone, side by side.
fn strips(dir: &std::path::Path) {
    use cantus_dsp::arcus::tel;
    cantus_ui::views::inst::chorda::scene::hybrid::register();
    cantus_ui::scene::set_blocking(true);
    let looks = [
        ("drawn", cantus_ui::scene::SceneBackend::Vector),
        ("photo", cantus_ui::scene::SceneBackend::PerPixel),
    ];
    // (level, speed m/s, β, force N, where the bow meets the hair) a frame.
    let bowed: [(f32, f32, f32, f32, f32); 6] = [
        (0.04, 0.06, 0.10, 1.2, 0.18),
        (0.18, 0.12, 0.10, 1.5, 0.30),
        (0.34, 0.15, 0.11, 1.7, 0.46),
        (0.36, 0.16, 0.12, 1.9, 0.62),
        (0.30, -0.10, 0.10, 1.4, 0.70),
        (0.33, -0.15, 0.09, 1.6, 0.56),
    ];
    let plucked: [f32; 6] = [0.9, 0.7, 0.5, 0.33, 0.2, 0.1];
    for (look, be) in looks {
        cantus_ui::scene::backend::set_default(be);
        for (name, tech) in [("bowed", 0.0f32), ("pizz", 1.0)] {
            let size = views::inst::editor_size(InstrumentKind::BowedString);
            let mut tiles = Vec::new();
            // A window a frame (a run of frames keeps only the textures it uploaded).
            for j in 0..6 {
                let ctx = context(true);
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.demo_editor("arcus");
                let Some(t) = app.inst.open else {
                    panic!("no Arcus editor")
                };
                app.set_inst_value(t, "tech", tech, false);
                views::inst::arcus::demo_feed(&mut app);
                app.inst.view = View::Play;
                if let Some(tele) = app.inst.injected.as_mut() {
                    if tech > 0.5 {
                        tele[tel::LEVEL] = plucked[j];
                        tele[tel::STRING] = 1.0;
                        tele[tel::KEY] = 50.0;
                        tele[tel::BETA] = 0.16;
                    } else {
                        let (lv, sp, beta, f, _) = bowed[j];
                        tele[tel::LEVEL] = lv;
                        tele[tel::SPEED] = sp;
                        tele[tel::BETA] = beta;
                        tele[tel::FORCE] = f;
                        tele[tel::VIBRATO] = 0.5;
                    }
                }
                app.inst
                    .arcus_scenes
                    .insert(t.0, (Default::default(), bowed[j].4, 0.0));
                let c = shoot(&ctx, size, 1.0, 3, None, |ui| {
                    views::inst::window_in(&mut app, ui)
                });
                let r = app
                    .inst
                    .arcus_scenes
                    .get(&t.0)
                    .and_then(|e| e.0.rect())
                    .expect("the picture was drawn");
                tiles.push((c, r));
            }
            let (tw, th) = (tiles[0].1.width() as u32, tiles[0].1.height() as u32);
            let gap = 8u32;
            let mut pix = tiny_skia::Pixmap::new(6 * tw + 7 * gap, th + 2 * gap).expect("a strip");
            pix.fill(tiny_skia::Color::from_rgba8(24, 24, 26, 255));
            let w = pix.width();
            let data = pix.pixels_mut();
            for (n, (c, r)) in tiles.iter().enumerate() {
                for y in 0..th {
                    for x in 0..tw {
                        let (sx, sy) =
                            (r.min.x as usize + x as usize, r.min.y as usize + y as usize);
                        if sx >= c.width || sy >= c.height {
                            continue;
                        }
                        let p = c.pixel(sx, sy);
                        let (dx, dy) = (gap + n as u32 * (tw + gap) + x, gap + y);
                        data[(dy * w + dx) as usize] =
                            tiny_skia::PremultipliedColorU8::from_rgba(p[0], p[1], p[2], 255)
                                .expect("opaque");
                    }
                }
            }
            let path = dir.join(format!("arcus-strip-{name}-{look}-night.png"));
            pix.save_png(&path).expect("a png");
            println!("{}", path.display());
        }
    }
    cantus_ui::scene::backend::set_default(cantus_ui::scene::SceneBackend::Vector);
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/shots/arcus".into()),
    );
    std::fs::create_dir_all(&dir).expect("out dir");
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--strip") {
        strips(&dir);
        return;
    }
    if args.iter().any(|a| a == "--scene") {
        let only = args.iter().find_map(|a| a.strip_prefix("--only="));
        let zooms: Vec<f32> = args
            .iter()
            .find_map(|a| a.strip_prefix("--zoom="))
            .map(|z| z.split(',').filter_map(|v| v.parse().ok()).collect())
            .unwrap_or_else(|| vec![1.0, 2.0]);
        for z in zooms {
            scenes(&dir, z, only, &holds(&args));
        }
        return;
    }
    let es = std::env::var("CANTUS_SHOOT_LANG").is_ok_and(|l| l == "es");
    cantus_ui::i18n::set_locale(if es {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    let lang = if es { "es" } else { "en" };
    // (name, view, instrument, technique, the telemetry's string and key)
    // r42:arcus-look — the design sheet v3: the picture in Play (the cello bowed, the bass
    // bowed, the cello plucked, the cello's tremolo) in both looks (Drawn, Photographic),
    // both themes and both scales; `--edit` adds Edit's Schelleng diagram.
    cantus_ui::views::inst::chorda::scene::hybrid::register();
    cantus_ui::scene::set_blocking(true);
    let only = args.iter().find_map(|a| a.strip_prefix("--only="));
    let mut states: Vec<(&str, View, f32, f32, f32, f32)> = vec![
        // r42:arcus-upper — on the G string (D3), the bow tilted toward its tip as the
        // reference's is.
        ("cello-play", View::Play, 0.0, 0.0, 1.0, 50.0),
        ("bass-play", View::Play, 1.0, 0.0, 2.0, 45.0),
        ("cello-pizz", View::Play, 0.0, 1.0, 1.0, 50.0),
        ("cello-trem", View::Play, 0.0, 2.0, 3.0, 64.0),
        ("violin-play", View::Play, 2.0, 0.0, 2.0, 76.0), // r42:arcus-upper
        ("viola-play", View::Play, 3.0, 0.0, 2.0, 67.0),  // r42:arcus-upper
        ("violin-pizz", View::Play, 2.0, 1.0, 2.0, 74.0), // r42:arcus-upper
        // r42:arcus-upper — a second hand position: an octave up the same string.
        ("violin-high", View::Play, 2.0, 0.0, 2.0, 81.0),
        ("cello-high", View::Play, 0.0, 0.0, 2.0, 62.0),
    ];
    if args.iter().any(|a| a == "--edit") {
        states.push(("cello-edit", View::Edit, 0.0, 0.0, 2.0, 57.0));
    }
    let looks = [
        ("drawn", cantus_ui::scene::SceneBackend::Vector),
        ("photo", cantus_ui::scene::SceneBackend::PerPixel),
    ];
    for (name, view, inst, tech, string, key) in states {
        for (look, be, hold) in looks
            .into_iter()
            .flat_map(|(l, be)| holds(&args).into_iter().map(move |h| (l, be, h)))
        {
            for dark in [true, false] {
                // r42:arcus-upper — `--scales=1,2,4`: which window scales (a 400 % shot
                // for the hands' crops).
                for k in scales(&args) {
                    let theme = if dark { "night" } else { "paper" };
                    let tag = format!(
                        "{name}{}{}-{look}-{theme}-{lang}-{}",
                        hold_tag(hold),
                        stop_tag(&args),
                        (k * 100.0) as u32
                    );
                    if only.is_some_and(|o| !o.split('+').all(|p| tag.contains(p))) {
                        continue;
                    }
                    cantus_ui::scene::backend::set_default(be);
                    let ctx = context(dark);
                    let mut app = CantusApp::from_parts(
                        Some(welcome_project()),
                        EngineHandle::dummy(),
                        false,
                    );
                    app.settings.arcus_holder = Holder::of(hold); // r42:arcus-upper
                    app.settings.arcus_stop = stop_show(&args); // r42:arcus-upper
                    app.demo_editor("arcus");
                    let Some(t) = app.inst.open else {
                        panic!("no Arcus editor")
                    };
                    app.set_inst_value(t, "inst", inst, false);
                    app.set_inst_value(t, "tech", tech, false);
                    views::inst::arcus::demo_feed(&mut app);
                    if let Some(tele) = app.inst.injected.as_mut() {
                        tele[cantus_dsp::arcus::tel::STRING] = string;
                        tele[cantus_dsp::arcus::tel::KEY] = key;
                        tele[cantus_dsp::arcus::tel::VIBRATO] = 0.4;
                        if (inst - 1.0).abs() < 0.5 {
                            // r42:arcus-upper (the bass alone)
                            tele[cantus_dsp::arcus::tel::FORCE] = 2.6;
                            tele[cantus_dsp::arcus::tel::FMIN] = 0.9;
                            tele[cantus_dsp::arcus::tel::FMAX] = 3.6;
                        }
                    }
                    app.inst.view = view;
                    Shell::new(("inst", t.0)).set_scale(&ctx, k);
                    let size = views::inst::editor_size(InstrumentKind::BowedString) * k;
                    let path = dir.join(format!("arcus-{tag}.png"));
                    shoot(&ctx, size, 1.0, 8, Some(&path), |ui| {
                        views::inst::window_in(&mut app, ui)
                    });
                    println!("{}", path.display());
                }
            }
        }
    }
    cantus_ui::scene::backend::set_default(cantus_ui::scene::SceneBackend::Vector);
}

/// r42:arcus-upper — `--scales=1,2,4` (default 1 and 2): the window's scales.
fn scales(args: &[String]) -> Vec<f32> {
    args.iter()
        .find_map(|a| a.strip_prefix("--scales="))
        .map(|z| z.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_else(|| vec![1.0, 2.0])
}

/// r42:arcus-upper — `--hold=air,stand,player,mannequin` (or `all`): what holds the
/// instrument in the shots; nothing by default (the names then as before).
fn holds(args: &[String]) -> Vec<Hold> {
    match args.iter().find_map(|a| a.strip_prefix("--hold=")) {
        None => vec![Hold::Air],
        Some("all") => Hold::ALL.to_vec(),
        Some(list) => list.split(',').filter_map(Hold::from_key).collect(),
    }
}

/// A shot's name for `h`: none for nothing (as before), else `-stand`, `-player`, ….
fn hold_tag(h: Hold) -> String {
    if h == Hold::Air {
        String::new()
    } else {
        format!("-{}", h.key())
    }
}

/// r42:arcus-upper — `--stop=finger|marker|none`: how the stopped note is shown (the
/// finger by default, the names then as before).
fn stop_show(args: &[String]) -> cantus_ui::scene::bowed_hold::StopShow {
    use cantus_ui::scene::bowed_hold::StopShow;
    match args.iter().find_map(|a| a.strip_prefix("--stop=")) {
        Some("marker") => StopShow::Marker,
        Some("none") => StopShow::None,
        _ => StopShow::Finger,
    }
}

fn stop_tag(args: &[String]) -> String {
    match args.iter().find_map(|a| a.strip_prefix("--stop=")) {
        Some(s) if s != "finger" => format!("-{s}"),
        _ => String::new(),
    }
}

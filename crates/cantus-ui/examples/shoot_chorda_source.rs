//! Chorda's source list and Mixer (round 40 phase 3, ADR 0125; `chorda-src`), drawn to
//! look at beside the approved design (`design-p3`): the editor at the design's 1280 × 720
//! in Night (Slate) and Paper, 1× and 1.5×, English and Spanish —
//! `chorda-play-*` (the source list beside the piano: the model playing),
//! `chorda-mixer-*` (the model's five on the shared desk) and, when a content folder with
//! bitKlavier's images is given, `chorda-mixer-pack-*` (its nine images, those on disk
//! loaded, the rest as placeholders). With `--beside <design-dir>`, each shot is also put
//! beside the design's sheet of the same name (`side-*.png`, the design at the left).
//!
//! `cargo run --release -p cantus-ui --example shoot_chorda_source -- <dir> [<content>]
//!  [--beside <design-dir>]`

use cantus_core::inst::chorda_source::{RecordedPiano, SourceChoice};
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::raster;
use std::path::{Path, PathBuf};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 720.0);

#[derive(Copy, Clone, PartialEq)]
enum What {
    Play,
    Mixer,
    Pack,
}

fn app(paper: bool, content: Option<&Path>) -> (CantusApp, cantus_core::TrackId) {
    let mut p = Project::new("Chorda");
    let mut t = p.make_track("Piano");
    t.strip = Some(p.strips[0].id);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
    let clip = p.make_clip("Play", Ticks::ZERO, Ticks(3840));
    t.clips.push(clip);
    let id = t.id;
    p.tracks.push(t);
    let tmp = std::env::temp_dir().join("cantus-shoot-chorda-source");
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if paper {
        cantus_ui::kit::appearance::Mode::Light
    } else {
        cantus_ui::kit::appearance::Mode::Dark
    };
    // Slate, as the design's sheets.
    let mode = app.appearance.mode;
    let preset = if paper { "slate_paper" } else { "slate_night" };
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.samples.content_root = Some(
        content
            .map(Path::to_path_buf)
            .unwrap_or_else(|| tmp.join("Content")),
    );
    (app, id)
}

fn shoot(
    dir: &Path,
    content: Option<&Path>,
    paper: bool,
    ppp: f32,
    what: What,
    es: bool,
) -> PathBuf {
    i18n::set_locale(if es { Locale::Es } else { Locale::En });
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (mut app, track) = app(paper, content);
    // As the live engine loads: long samples stream (ADR 0050), in the background.
    if let Some(c) = content {
        let dir = c.join(".stream-cache");
        let _ = std::fs::create_dir_all(&dir);
        cantus_engine::inst::stream::hub().set_cache_dir(dir);
    }
    app.engine.samples.set_streaming(true);
    app.engine.samples.set_background(true);
    app.attach(&ctx);
    cantus_ui::scene::set_blocking(true);
    // The Play view's picture (the baked scene until `scene` switches it) built in place too.
    cantus_ui::views::inst::chorda::scene::set_blocking(true);
    cantus_ui::views::inst::demo_setup(&mut app, track, InstrumentKind::Chorda);
    // Round 40 polish — `CHORDA_SHOT_BODY=upright` (or `grand`, `parlour`, `concert`): the
    // piano's body.
    if let Ok(b) = std::env::var("CHORDA_SHOT_BODY") {
        app.set_inst_text(
            track,
            cantus_core::inst::chorda_look::BODY_KEY,
            Some(b.clone()),
        );
    }
    // Round 40 polish — `CHORDA_SHOT_ROOM_OPEN=1`: the room card open from its chip.
    if std::env::var("CHORDA_SHOT_ROOM_OPEN").as_deref() == Ok("1") {
        cantus_ui::views::inst::chorda::view::set_room_open(&mut app, track, true);
    }
    if what == What::Pack {
        // bitKlavier: the images on disk loaded, the rest offered.
        let set = SourceChoice::Recorded(RecordedPiano {
            pack: "bitklavier-grand".into(),
            instrument: "grand".into(),
        });
        let _ = app.chorda_switch(track, &set);
    }
    if what != What::Play {
        app.inst.view = cantus_ui::views::inst::View::Mixer;
    }
    app.open_inst_editor(track);
    app.inst.chorda_source_open_previous = what == What::Play;
    let theme = if paper { "paper" } else { "night" };
    let name = match what {
        What::Play => "play",
        What::Mixer => "mixer",
        What::Pack => "mixer-pack",
    };
    let lang = if es { "-es" } else { "" };
    let path = dir.join(format!(
        "chorda-{name}-{theme}-{}x{lang}.png",
        if ppp > 1.0 { "1.5" } else { "1" }
    ));
    // The editor alone, at the design's size (as `shoot_tympanum`).
    raster::shoot(&ctx, SIZE, ppp, 12, Some(&path), |ui| {
        cantus_ui::views::inst::window_in_sized(&mut app, ui, SIZE)
    });
    println!("{}", path.display());
    path
}

/// An 8-bit PNG as RGB rows.
fn read_rgb(path: &Path) -> Option<(usize, usize, Vec<u8>)> {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let ch = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return None,
    };
    let mut rgb = Vec::with_capacity(w * h * 3);
    for px in buf[..w * h * ch].chunks(ch) {
        rgb.extend_from_slice(&px[..3]);
    }
    Some((w, h, rgb))
}

/// The design's sheet at the left, ours at the right, a gap between.
fn beside(design: &Path, ours: &Path, out: &Path) {
    let (Some(a), Some(b)) = (read_rgb(design), read_rgb(ours)) else {
        eprintln!("cannot put {} beside {}", ours.display(), design.display());
        return;
    };
    let gap = 24;
    let (w, h) = (a.0 + gap + b.0, a.1.max(b.1));
    let mut img = vec![128u8; w * h * 3];
    for (x0, (iw, ih, px)) in [(0, &a), (a.0 + gap, &b)] {
        for y in 0..*ih {
            let src = &px[y * iw * 3..(y + 1) * iw * 3];
            let at = (y * w + x0) * 3;
            img[at..at + iw * 3].copy_from_slice(src);
        }
    }
    let f = std::io::BufWriter::new(std::fs::File::create(out).expect("the side file"));
    let mut enc = png::Encoder::new(f, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(&img))
        .expect("the side image");
    println!("{}", out.display());
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let design = args.iter().position(|a| a == "--beside").map(|i| {
        let d = PathBuf::from(args[i + 1].clone());
        args.drain(i..i + 2);
        d
    });
    let mut args = args.into_iter();
    let dir = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "target/shots/chorda-source".into()),
    );
    let content = args.next().map(PathBuf::from);
    // r40 integration — `CANTUS_SHOOT_PICTURES=photo`: the instruments' pictures by the
    // photographic renderer (Settings ▸ Appearance ▸ Instrument pictures ▸ Photographic).
    if std::env::var("CANTUS_SHOOT_PICTURES").as_deref() == Ok("photo") {
        cantus_ui::views::inst::chorda::scene::hybrid::register();
        cantus_ui::scene::backend::set_default(cantus_ui::scene::SceneBackend::PerPixel);
    }
    std::fs::create_dir_all(&dir).expect("the folder");
    // Round 40 polish — `CHORDA_SHOT_PLAY=1`: the Play view only, in English and Spanish
    // at every theme and scale.
    let play_only = std::env::var("CHORDA_SHOT_PLAY").as_deref() == Ok("1");
    let mut whats = if play_only {
        vec![What::Play]
    } else {
        vec![What::Play, What::Mixer]
    };
    if content.is_some() {
        whats.push(What::Pack);
    }
    for paper in [false, true] {
        for ppp in [1.0, 1.5] {
            for &what in &whats {
                let ours = shoot(&dir, content.as_deref(), paper, ppp, what, false);
                if let Some(d) = &design {
                    let name = ours.file_name().unwrap().to_string_lossy().to_string();
                    let sheet = d.join(&name);
                    if sheet.exists() {
                        beside(&sheet, &ours, &dir.join(format!("side-{name}")));
                    }
                }
            }
        }
    }
    if play_only {
        for paper in [false, true] {
            for ppp in [1.0, 1.5] {
                shoot(&dir, content.as_deref(), paper, ppp, What::Play, true);
            }
        }
        return;
    }
    for &what in &whats {
        shoot(&dir, content.as_deref(), false, 1.0, what, true);
    }
}

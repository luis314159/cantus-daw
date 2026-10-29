//! Every picture in the README, in one command, headless (round 26):
//!
//! ```text
//! cargo run --release -p cantus-ui --example shoot_readme                 # into docs/images/
//! cargo run --release -p cantus-ui --example shoot_readme -- <dir> [name…]  # some, elsewhere
//! ```
//!
//! The pictures are the program on a real song — the demo it opens on, *Neon Tide*'s second
//! drop — with a working studio: the demo interface of the audio setup (an ASIO driver at
//! 48 kHz and 256 samples), so no picture carries the "no audio device" line a machine
//! without one shows. The engine is the manual one, run a few blocks so meters, clock and
//! playhead move. Everything shown is what the program draws; nothing is painted over it.
//!
//! - `window-night` (1920 × 1080, the hero): the whole song across the arrangement (intro,
//!   build, drop, break, drop, outro), the desk Extra large with the selected strip's
//!   cables, the sample browser and the inspector, in Cantus Night (the default);
//!   `window-night-720` / `window-paper-720`: the window as it opens, Night and Paper.
//! - `mixer`: the desk Extra large with the selected strip's cables, and the inspector.
//! - `matrix`: the Routing tab (Alt+F9) on the song's desk.
//! - `linked`: the arrangement with linked copies and the clip browser (Alt+F5), the paint
//!   tool out.
//! - `pianoroll`, `rack`: the piano roll and the channel rack on the song.
//! - `fx-<kind>`: each effect's editor alone, at its designed size, fed as a playing engine
//!   would; `fx-hosted`: a CLAP plug-in's generic editor (the test plug-in, built from
//!   source).
//! - `inst-<kind>`: each instrument's editor, cropped from the window it docks in.
//!   Exemplar plays the **Salamander Grand Piano** when `CANTUS_SALAMANDER_ARCHIVE` names
//!   the pack's `.tar.gz` (installed once into `cantus-readme-content/` beside it, its
//!   sample cache in `cantus-readme-cache/`); without it, Exemplar's demo zones. The run
//!   says which.
//! - `appearance`: the Appearance page; `themes`: every shipped theme's preview, named;
//!   `mark`: the mark in its own colours and in each icon colour, on a dark and a light band.
//! - `packs`: *Sound packs…* on the Salamander Grand Piano, installed, with its credit.
//! - `score-roll`, `score-pane`, `score-page` (round 32): the piano roll with the staff
//!   under it (Night), the Score pane (Paper), and the top of the printed A4 page.
//! - Every effect and instrument wears its own face (*Units wear their own faces*, ADR
//!   0074, on by default).
//!
//! PNGs are written as 8-bit RGB with the encoder's best compression, then optimised by
//! `oxipng -o max` when it is installed (`cargo install oxipng`); both are lossless.

use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster::{self, Canvas};
use cantus_ui::{demos, CantusApp};
use egui::{Rect, Vec2};
use std::path::{Path, PathBuf};

/// A context with the program's fonts and style.
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The application on `project`, in a shipped theme, with a working studio, attached to
/// `ctx`. It never touches the user's files.
fn studio(
    ctx: &egui::Context,
    project: cantus_core::Project,
    preset: &str,
    engine: EngineHandle,
) -> CantusApp {
    let dark = preset != "cantus_paper";
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-readme");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(ctx);
    // The demo interface of the audio setup, as on a machine with one plugged in.
    cantus_ui::audio_setup::demo::setup(&mut app);
    if let Some(live) = app.audio_setup.demo.as_mut() {
        live.driver = "ASIO".into();
        live.device = "ASIO4ALL v2".into();
        live.wanted = live.device.clone();
        live.block = 256;
    }
    app.audio_setup.scan = None;
    // No sound packs but the shot's own: nothing of this machine's shows.
    app.samples.content_root = Some(tmp.join("Content"));
    app.settings.sample_folders.clear();
    app.settings.audio.device = Some("ASIO4ALL v2".into());
    app.set_status(String::new());
    app
}

/// The song the program opens on.
fn song() -> cantus_core::Project {
    demos::startup_project()
}

/// The desk ends cleanly: the strips nothing feeds (the empty slots) are removed.
fn tidy(app: &mut CantusApp) {
    let empty: Vec<_> = app
        .project()
        .strips
        .iter()
        .filter(|s| s.name.is_empty() && s.effects.is_empty())
        .map(|s| s.id)
        .collect();
    for id in empty {
        app.exec(cantus_core::Command::RemoveStrip { id });
    }
}

/// Playing a few bars in: the engine run so the clock, the playhead and the meters move.
fn play(app: &mut CantusApp, processor: &mut cantus_engine::Processor, bar: i64) {
    app.sync_engine();
    app.seek(cantus_core::Ticks::beats(bar * 4));
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    for _ in 0..40 {
        processor.process(&mut audio, 2);
    }
}

/// Write `canvas` (or the part of it in `crop`, in pixels) as a compressed RGB PNG.
fn save(canvas: &Canvas, crop: Option<Rect>, path: &Path) {
    let full = Rect::from_min_size(
        egui::Pos2::ZERO,
        Vec2::new(canvas.width as f32, canvas.height as f32),
    );
    let r = crop.map_or(full, |c| c.intersect(full));
    let (x0, y0) = (r.min.x.round() as usize, r.min.y.round() as usize);
    let (w, h) = (r.width().round() as usize, r.height().round() as usize);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            rgb.extend_from_slice(&canvas.pixel(x, y)[..3]);
        }
    }
    write_png(w, h, &rgb, path);
}

/// An 8-bit RGB PNG, the encoder's best compression (lossless).
fn write_png(w: usize, h: usize, rgb: &[u8], path: &Path) {
    let file = std::fs::File::create(path).expect("create the picture");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(rgb).expect("png data");
    println!("wrote {} ({w} x {h})", path.display());
}

/// The application on the song, playing, with the chords' strip selected (sidechained from
/// the kick, sent to the reverb), as `setup` leaves it; and the engine that plays it.
fn playing(
    ctx: &egui::Context,
    preset: &str,
    project: cantus_core::Project,
    bar: i64,
    setup: fn(&mut CantusApp),
) -> (CantusApp, cantus_engine::Processor) {
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = studio(ctx, project, preset, engine);
    tidy(&mut app);
    if let Some(strip) = app.project().tracks.get(5).and_then(|t| t.strip) {
        app.select_strip(strip);
    }
    setup(&mut app);
    play(&mut app, &mut processor, bar);
    (app, processor)
}

/// The whole window, `size` pixels, in `preset`.
fn window(preset: &str, size: (f32, f32), setup: fn(&mut CantusApp)) -> Canvas {
    window_with(preset, size, song(), 6, setup, |_, _| {})
}

/// [`window`] on `project`, playing from `bar`, with `each` run before every frame.
fn window_with(
    preset: &str,
    size: (f32, f32),
    project: cantus_core::Project,
    bar: i64,
    setup: fn(&mut CantusApp),
    each: fn(usize, &mut CantusApp),
) -> Canvas {
    let ctx = context();
    let (mut app, mut processor) = playing(&ctx, preset, project, bar, setup);
    let mut audio = vec![0f32; 800 * 2];
    let mut frame = 0;
    raster::shoot(&ctx, egui::vec2(size.0, size.1), 1.0, 14, None, |root| {
        each(frame, &mut app);
        frame += 1;
        processor.process(&mut audio, 2);
        app.frame(root);
    })
}

/// One view alone, filling `size` points, in Cantus Night.
fn alone(
    size: (f32, f32),
    setup: fn(&mut CantusApp),
    draw: fn(&mut CantusApp, &mut egui::Ui),
) -> Canvas {
    let ctx = context();
    let (mut app, mut processor) = playing(&ctx, "cantus_night", song(), 6, setup);
    let mut audio = vec![0f32; 800 * 2];
    raster::shoot(&ctx, egui::vec2(size.0, size.1), 1.0, 10, None, |root| {
        processor.process(&mut audio, 2);
        // What the window's frame does first: the engine's meters, latency and clock.
        app.engine.poll();
        let fill = cantus_ui::kit::Style::current().t.panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(fill))
            .show(root, |ui| draw(&mut app, ui));
    })
}

fn nothing(_: &mut CantusApp) {}

/// The arrangement's height in the linked-clips picture, which shows the window above it.
const LINKED_ARRANGEMENT: f32 = 460.0;

/// The arrangement shows the whole loop: sixteen bars across its width.
fn sixteen_bars(app: &mut CantusApp, width: f32) {
    app.arrangement.px_per_beat = width / 64.0;
    app.arrangement.scroll_beats = 0.0;
}

/// The whole of the song the program opens on (*Neon Tide*: intro, build, drop, break,
/// drop, outro), for the hero.
fn whole_song() -> cantus_core::Project {
    (demos::find("house").expect("the house demo").build)()
}

/// The hero: the whole song across the arrangement, the desk Extra large under it with the
/// selected strip's cables, the sample browser docked at the left.
fn hero(app: &mut CantusApp) {
    use cantus_ui::views::mixer::layout::{CableView, WidthMode};
    cantus_ui::views::mixer::set_width_mode(app, WidthMode::from_index(2));
    let mut prefs = app.mixer.prefs();
    prefs.cables = CableView::Selected;
    app.mixer.set_prefs(prefs);
    let beats = app.project().content_end().0 as f32 / cantus_core::Ticks::beats(1).0 as f32;
    app.arrangement.px_per_beat = HERO_LANES / beats.max(1.0);
    app.arrangement.scroll_beats = 0.0;
}

/// The hero's arrangement lanes, in points, and its browser's and inspector's widths.
const HERO_LANES: f32 = 1060.0;
const HERO_BROWSER: f32 = 300.0;
const HERO_INSPECTOR: f32 = 330.0;

/// The browser opens once the window's width is known; then it and the inspector take
/// their widths.
fn hero_frames(frame: usize, app: &mut CantusApp) {
    if frame == 1 {
        app.toggle_pane(cantus_ui::views::workspace::Pane::Browser);
    }
    if frame == 3 {
        let main = app.workspace.dock.main_surface_mut();
        if let egui_dock::Node::Horizontal(split) = &mut main[egui_dock::NodeIndex::root()] {
            split.fraction = HERO_BROWSER / 1920.0;
        }
        app.workspace.impose_sizes(HERO_INSPECTOR, 0.0);
    }
}

fn compact(app: &mut CantusApp) {
    sixteen_bars(app, 760.0);
}

/// The desk alone, Extra large, every cable drawn.
fn desk(app: &mut CantusApp) {
    use cantus_ui::views::mixer::layout::{CableView, WidthMode};
    cantus_ui::views::mixer::set_width_mode(app, WidthMode::from_index(2));
    let mut prefs = app.mixer.prefs();
    prefs.cables = CableView::All;
    app.mixer.set_prefs(prefs);
}

fn draw_mixer(app: &mut CantusApp, ui: &mut egui::Ui) {
    cantus_ui::views::mixer::show(app, ui);
}

fn draw_inspector(app: &mut CantusApp, ui: &mut egui::Ui) {
    cantus_ui::views::inspector::show(app, ui);
}

fn draw_routing(app: &mut CantusApp, ui: &mut egui::Ui) {
    cantus_ui::views::routing::show(app, ui);
}

/// The lead's clip under the playhead, for the piano roll.
fn lead(app: &mut CantusApp) {
    let pick = app
        .project()
        .tracks
        .get(7)
        .and_then(|t| t.clips.first().map(|c| (t.id, c.id)));
    if let Some((t, c)) = pick {
        app.select_clip(t, c);
    }
}

fn draw_piano_roll(app: &mut CantusApp, ui: &mut egui::Ui) {
    cantus_ui::views::piano_roll::show(app, ui);
}

/// The drums' clip, for the channel rack.
fn drums(app: &mut CantusApp) {
    let pick = app
        .project()
        .tracks
        .first()
        .and_then(|t| t.clips.first().map(|c| (t.id, c.id)));
    if let Some((t, c)) = pick {
        app.select_clip(t, c);
    }
}

fn draw_rack(app: &mut CantusApp, ui: &mut egui::Ui) {
    cantus_ui::views::channel_rack::show(app, ui);
}

/// The Appearance page.
fn appearance(app: &mut CantusApp) {
    app.show_theme_editor = true;
}

fn draw_appearance(app: &mut CantusApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    cantus_ui::views::appearance::window(app, &ctx);
}

/// Linked copies the way a musician makes them, on the song: each part's clips that are the
/// same music linked into one group (*Link selection to this clip*), the clip browser open,
/// the arrangement tall, and the paint tool out with the bass line.
fn linked(app: &mut CantusApp) {
    let tracks: Vec<_> = app.project().tracks.iter().map(|t| t.id).collect();
    for t in tracks {
        type Music = Vec<(cantus_core::Ticks, cantus_core::Ticks, u8)>;
        let clips: Vec<(cantus_core::ClipId, Music)> = app
            .project()
            .track(t)
            .map(|tr| {
                tr.clips
                    .iter()
                    .filter(|c| c.audio.is_none() && c.automation.is_none())
                    .map(|c| {
                        (
                            c.id,
                            c.notes
                                .iter()
                                .map(|n| (n.start, n.length, n.pitch))
                                .collect(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut done: Vec<usize> = Vec::new();
        for i in 0..clips.len() {
            if done.contains(&i) {
                continue;
            }
            let same: Vec<usize> = (i..clips.len())
                .filter(|&j| clips[j].1 == clips[i].1)
                .collect();
            done.extend(&same);
            if same.len() < 2 {
                continue;
            }
            app.set_clip_selection(same.iter().map(|&j| (t, clips[j].0)).collect());
            app.link_clips_to(Some((t, clips[same[0]].0)));
        }
    }
    app.show_pane(cantus_ui::views::workspace::Pane::Clips);
    app.workspace.impose_sizes(0.0, LINKED_ARRANGEMENT);
    let bass = app
        .project()
        .tracks
        .get(4)
        .and_then(|t| t.clips.first().map(|c| (t.id, c.id)));
    if let Some((t, c)) = bass {
        app.select_clip(t, c);
        app.clip_browser.set_brush(Some((t, c)));
    }
    app.focus_arrangement();
    app.toggle_arrangement_paint();
    sixteen_bars(app, 1000.0);
    app.set_status(String::new());
}

/// Trim a uniform border (the colour of the top-left pixel) to `margin` pixels.
fn trimmed(canvas: &Canvas, margin: f32) -> Rect {
    let bg = canvas.pixel(0, 0);
    let differs = |x: usize, y: usize| {
        let p = canvas.pixel(x, y);
        (0..3).any(|k| (p[k] as i32 - bg[k] as i32).abs() > 6)
    };
    let (mut x0, mut y0, mut x1, mut y1) = (canvas.width, canvas.height, 0, 0);
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            if differs(x, y) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    Rect::from_min_max(
        egui::pos2(x0 as f32 - margin, y0 as f32 - margin),
        egui::pos2(x1 as f32 + 1.0 + margin, y1 as f32 + 1.0 + margin),
    )
}

/// A context in Cantus Night (the default theme), for the views drawn alone.
fn night() -> egui::Context {
    let ctx = context();
    cantus_ui::theme::apply(&cantus_ui::theme_family::cantus_night(), &ctx);
    ctx
}

/// The test CLAP plug-in's library, built from source the first time (as `shoot` does).
fn test_plugin() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let file = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    });
    if !file.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let _ = std::process::Command::new(cargo)
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status();
    }
    file
}

/// An effect's editor alone, at its designed size, fed as a playing engine would.
fn effect(kind: &str) -> Canvas {
    use cantus_core::EffectKind as K;
    use cantus_ui::views;
    let fx = match kind {
        "eq" => K::Eq,
        "comp" => K::Compressor,
        "limiter" => K::Limiter,
        "space" => K::Space,
        "multiband" => K::Multiband,
        "deesser" => K::DeEsser,
        "gate" => K::Gate,
        "saturator" => K::Saturator,
        "stereo" => K::Stereo,
        "maximizer" => K::Maximizer,
        "tonsor" => K::Clipper,
        "impetus" => K::Transient,
        "discrimen" => K::TransientSplit,
        "speculum" => K::Analyzer,
        "tonus" => K::AutoTune,
        "concentus" => K::Chorus,
        "flexus" => K::Flanger,
        "phasis" => K::Phaser,
        "tremor" => K::Tremolo,
        "vibrans" => K::Vibrato,
        "anulus" => K::RingMod,
        "translatio" => K::FreqShift,
        "fluctus" => K::VolumeShaper,
        _ => K::External,
    };
    let ctx = night();
    let mut app = CantusApp::from_parts(
        Some(cantus_ui::welcome_project()),
        EngineHandle::dummy(),
        false,
    );
    if kind == "hosted" {
        views::plugins::demo_setup(&mut app, &test_plugin(), true);
    } else {
        app.demo_editor(kind);
        let k = match kind {
            "eq" => Some(K::Eq),
            "comp" => Some(K::Compressor),
            _ => app.fx.open.map(|o| o.2),
        };
        if let Some(k) = k {
            views::fx::demo_feed(&mut app, k);
        }
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let mut size = views::fx::frame::designed_size(fx);
    if kind == "hosted" {
        // The generic editor fits its parameters: learn its height in a context of its own.
        let probe = night();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            };
            probe
                .run_ui(input, |ui| views::fx::window(&mut app, ui.ctx()))
                .textures_delta
                .clear();
        }
        if let Some(h) = views::plugins::wanted_height(&app) {
            size.y = h;
        }
    }
    raster::shoot(&ctx, size, 1.0, 6, None, |root| {
        let ctx = root.ctx().clone();
        views::eq_editor::window(&mut app, &ctx);
        views::comp_editor::window(&mut app, &ctx);
        views::fx::window(&mut app, &ctx);
    })
}

/// The score's passage (round 32, ADR 0075): the two-hand piano piece, as a clip on a
/// track, with dynamics from velocity.
fn score_project() -> cantus_core::Project {
    let p = cantus_score::samples::piano();
    let mut pr = cantus_core::Project::new("Sonatina");
    pr.key = p.key;
    pr.tempo_bpm = p.tempo.bpm_at(0.0);
    let part = &p.parts[0];
    let mut t = pr.make_track("Piano");
    t.score = part.settings.clone();
    let mut c = pr.make_clip(
        "Sonatina",
        cantus_core::Ticks(0),
        cantus_core::Ticks(6 * 3840),
    );
    for n in &part.notes {
        let note = pr.make_note(
            cantus_core::Ticks(n.start),
            cantus_core::Ticks(n.len),
            n.pitch,
            n.velocity,
        );
        c.notes.push(note);
    }
    t.clips.push(c);
    pr.tracks.push(t);
    pr
}

fn score_app(ctx: &egui::Context, preset: &str) -> CantusApp {
    let mut app = studio(ctx, score_project(), preset, EngineHandle::dummy());
    let t = app.project().tracks[0].id;
    let c = app.project().tracks[0].clips[0].id;
    app.selected_track = Some(t);
    app.selected_clip = Some(c);
    app.selected_notes = vec![app.project().tracks[0].clips[0].notes[4].id];
    app
}

/// The piano roll with the staff under it (Alt+S), in Cantus Night.
fn score_roll() -> Canvas {
    let ctx = night();
    let mut app = score_app(&ctx, "cantus_night");
    app.score.split_open = true;
    app.score.split_h = 290.0;
    app.score.split_space = 8.5;
    raster::shoot(&ctx, egui::vec2(1400.0, 760.0), 1.0, 4, None, |ui| {
        cantus_ui::views::piano_roll::show(&mut app, ui)
    })
}

/// The Score pane, in Cantus Paper.
fn score_pane() -> Canvas {
    let ctx = context();
    cantus_ui::theme::apply(&cantus_ui::theme::paper(), &ctx);
    let mut app = score_app(&ctx, "cantus_paper");
    app.score.pane_space = 8.0;
    raster::shoot(&ctx, egui::vec2(900.0, 400.0), 1.0, 4, None, |ui| {
        cantus_ui::views::score_view::show(&mut app, ui)
    })
}

/// The printed page: A4 as the PDF lays it out, black on white.
fn score_page() -> Canvas {
    use cantus_score::layout::{engrave, Format, LayoutOptions, Paper};
    use cantus_ui::views::score_paint::{paint, Placement, ScoreInk};
    let ctx = context();
    let mut app = score_app(&ctx, "cantus_paper");
    app.settings.export.tags.artist = "Cantus".into();
    let score = cantus_ui::score::score_of(
        &app,
        cantus_ui::score::Scope::Track(app.project().tracks[0].id),
    );
    let e = engrave(
        &score,
        &LayoutOptions {
            format: Format::Pages(Paper::A4),
            text: cantus_ui::score::measure(),
            header: true,
        },
    );
    let list = &e.pages[0].list;
    let space = 7.5;
    raster::shoot(
        &ctx,
        egui::vec2(list.width * space, list.height * space * 0.42),
        1.0,
        3,
        None,
        |ui| {
            let r = ui.max_rect();
            // A page: white paper, black ink, whatever the theme.
            ui.painter().rect_filled(r, 0.0, egui::Color32::WHITE);
            let pl = Placement {
                origin: r.min,
                space,
            };
            let ink = ScoreInk {
                ink: egui::Color32::BLACK,
                selected: egui::Color32::BLACK,
            };
            paint(ui.painter(), list, &pl, &ink, &|_| false, r);
        },
    )
}

/// An instrument's editor as the program docks it, cropped from the whole window.
fn instrument(kind: &str) -> (Canvas, Option<Rect>) {
    use cantus_ui::views;
    let ctx = context();
    let mut app = studio(
        &ctx,
        cantus_ui::welcome_project(),
        "cantus_night",
        EngineHandle::dummy(),
    );
    // Exemplar plays the Salamander Grand Piano where its archive is on this machine.
    let piano = kind == "sampler" && salamander(&mut app);
    if kind == "sampler" {
        println!(
            "Exemplar: {}",
            if piano {
                "the Salamander Grand Piano, installed from CANTUS_SALAMANDER_ARCHIVE"
            } else {
                "its demo zones (set CANTUS_SALAMANDER_ARCHIVE to the pack's .tar.gz for the piano)"
            }
        );
    }
    if !piano {
        app.demo_editor(kind);
        if let Some(k) = views::inst::kind_from_name(kind) {
            views::inst::demo_feed(&mut app, k);
        }
    }
    // A pack's samples load beside the window: wait for them (two minutes at most).
    let frames = if piano { 1200 } else { 10 };
    let canvas = raster::shoot(
        &ctx,
        egui::vec2(1760.0, 1060.0),
        1.0,
        frames,
        None,
        |root| {
            if piano
                && (app.engine.samples.progress().is_some()
                    || views::inst::sampler::wave_file_reading(&app))
            {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            app.frame(root)
        },
    );
    let mut keys = Vec::new();
    views::workspace_device::wanted(&ctx, &mut keys);
    let crop = keys
        .iter()
        .find_map(|&k| views::workspace_device::shown(&ctx, k))
        .map(|(whole, _)| whole)
        // The piano's footer reads the engine's telemetry, which a shot has not got.
        .map(|r| {
            if piano {
                r.with_max_y(r.max.y - FOOTER)
            } else {
                r
            }
        });
    (canvas, crop)
}

/// The height of an instrument editor's footer.
const FOOTER: f32 = 25.0;

/// The Salamander Grand Piano installed from the archive `CANTUS_SALAMANDER_ARCHIVE` names
/// (the pack's own `.tar.gz`, as *Install from a file…* takes it) into a folder beside it,
/// once, and a track playing it in Exemplar with its editor open. False when the archive
/// is not there or the install fails: the picture is then Exemplar's demo.
fn salamander(app: &mut CantusApp) -> bool {
    use cantus_content::{install, Manager, Manifest};
    const ID: &str = "salamander-grand-piano";
    let Some(archive) = std::env::var_os("CANTUS_SALAMANDER_ARCHIVE").map(PathBuf::from) else {
        return false;
    };
    if !archive.is_file() {
        return false;
    }
    let beside = archive.parent().unwrap_or(Path::new(".")).to_path_buf();
    let root = beside.join("cantus-readme-content");
    let bundled = Manifest::bundled();
    let have = install::installed(&install::Layout::new(&root))
        .iter()
        .any(|r| r.id == ID);
    if !have {
        let mut m = Manager::new(&root, bundled.clone());
        if !m.install_from_file(ID, &archive) {
            return false;
        }
        let done = m.wait_idle(std::time::Duration::from_secs(1800));
        if done.iter().any(|f| f.result.is_err()) {
            return false;
        }
    }
    // The decoded samples' cache beside it too, never the user's.
    cantus_engine::inst::stream::hub().set_cache_dir(beside.join("cantus-readme-cache"));
    app.samples.content_root = Some(root);
    app.samples.manifest = Some(bundled);
    let found = app.samples.content().instruments();
    app.picker_view.refresh(&app.plugins.db.clone(), &found);
    let Some(choice) = app
        .picker_view
        .device(&format!("pack:{ID}/grand"))
        .map(|d| d.choice.clone())
    else {
        return false;
    };
    app.apply_pick(
        cantus_ui::picker::PickPurpose::AddInstrument { index: None },
        choice,
    );
    let Some(track) = app.project().tracks.last().map(|t| t.id) else {
        return false;
    };
    app.select_track(track);
    app.open_inst_editor(track);
    // Its middle C at the loudest layer selected: the waveform shows that sample.
    if let Some(cantus_core::InstrumentSpec::Native(n)) =
        app.project().track(track).map(|t| t.instrument.clone())
    {
        use cantus_core::inst::sampler::{zf, zones};
        app.inst.sampler.selected = zones(&n)
            .into_iter()
            .filter(|(_, z)| {
                let v = &z.values;
                v[zf::TRIG] == 0.0 && v[zf::LO] <= 60.0 && v[zf::HI] >= 60.0
            })
            .max_by(|a, b| a.1.values[zf::VHI].total_cmp(&b.1.values[zf::VHI]))
            .map(|(i, _)| i);
    }
    true
}

/// Every shipped theme's preview, named, in rows.
fn themes() -> Canvas {
    use cantus_ui::kit::appearance::PRESETS;
    use cantus_ui::kit::text::{self, Elide, Policy, Role, Text};
    use cantus_ui::kit::{with_brush, Style};
    use cantus_ui::views::appearance_look;
    let ctx = context();
    let mut app = studio(
        &ctx,
        cantus_ui::welcome_project(),
        "cantus_night",
        EngineHandle::dummy(),
    );
    let (cols, chip, gap) = (6usize, Vec2::new(200.0, 92.0), 14.0);
    let rows = PRESETS.len().div_ceil(cols);
    let size = Vec2::new(
        cols as f32 * (chip.x + gap) + gap,
        rows as f32 * (chip.y + 26.0 + gap) + gap,
    );
    raster::shoot(&ctx, size, 1.0, 4, None, |root| {
        let s = Style::current();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(s.t.panel))
            .show(root, |ui| {
                for (i, (preset, _)) in PRESETS.iter().enumerate() {
                    let (c, r) = ((i % cols) as f32, (i / cols) as f32);
                    let min = egui::pos2(gap + c * (chip.x + gap), gap + r * (chip.y + 26.0 + gap));
                    let swatch = Rect::from_min_size(min, chip);
                    let label = Rect::from_min_size(
                        egui::pos2(min.x, swatch.bottom() + 4.0),
                        Vec2::new(chip.x, 22.0),
                    );
                    let t = appearance_look::preview_tokens(&mut app, &format!("preset:{preset}"));
                    with_brush(ui.painter(), |b| {
                        appearance_look::paint_preview(b, swatch, &t, s.k);
                        let name = cantus_ui::i18n::t(
                            &cantus_ui::kit::appearance::preset_name_key(preset),
                        );
                        let _ = text::paint(
                            b,
                            label,
                            &Text::new(&name, Role::Label, s.t.text)
                                .policy(Policy::Elide(Elide::End)),
                            s.tk(),
                        );
                    });
                }
            });
    })
}

/// The mark in its own colours and in each icon colour, at 96 px, on a dark and a light band.
fn mark() -> (usize, usize, Vec<u8>) {
    use cantus_ui::kit::appearance::IconPreset;
    use cantus_ui::kit::mark::MarkColours;
    let mut colours = vec![MarkColours::ORIGINAL];
    for p in IconPreset::ALL {
        colours.push(MarkColours::from_one(p.rgb()));
    }
    let (px, pad) = (96usize, 24usize);
    let w = colours.len() * (px + pad) + pad;
    let h = 2 * (px + 2 * pad);
    let mut rgb = vec![0u8; w * h * 3];
    let bands = [[0x17u8, 0x15, 0x13], [0xee, 0xec, 0xe8]];
    for (bi, bg) in bands.iter().enumerate() {
        for y in bi * (px + 2 * pad)..(bi + 1) * (px + 2 * pad) {
            for x in 0..w {
                rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(bg);
            }
        }
        for (ci, c) in colours.iter().enumerate() {
            let pix = cantus_ui::window_icon::render(c, px as u32).expect("the mark");
            let (x0, y0) = (pad + ci * (px + pad), bi * (px + 2 * pad) + pad);
            for y in 0..px {
                for x in 0..px {
                    let p = pix.pixel(x as u32, y as u32).expect("pixel").demultiply();
                    let a = p.alpha() as f32 / 255.0;
                    let i = ((y0 + y) * w + x0 + x) * 3;
                    for (k, v) in [p.red(), p.green(), p.blue()].into_iter().enumerate() {
                        rgb[i + k] = (v as f32 * a + rgb[i + k] as f32 * (1.0 - a)).round() as u8;
                    }
                }
            }
        }
    }
    (w, h, rgb)
}

/// *Sound packs…* on the Salamander Grand Piano, installed (its receipt and licence file
/// in a folder of the shot's own), with its credit.
fn packs() -> Canvas {
    use cantus_content::{Manifest, Receipt};
    let bundled = Manifest::bundled();
    let pack = bundled
        .pack("salamander-grand-piano")
        .expect("the Salamander");
    let root =
        std::env::temp_dir().join(format!("cantus-shoot-readme-packs-{}", std::process::id()));
    let dir = root.join(&pack.id);
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join(&pack.licence_file), "CC BY 3.0").expect("licence");
    let receipt = Receipt {
        id: pack.id.clone(),
        version: pack.version,
        name: pack.name.clone(),
        licence: pack.licence.clone(),
        licence_url: pack.licence_url.clone(),
        licence_file: pack.licence_file.clone(),
        credits: pack.credits.clone(),
        attribution: pack.attribution.clone(),
        instruments: pack.instruments.clone(),
        installed_at: 0,
        files: 0,
        bytes: pack.installed_size,
    };
    std::fs::write(
        dir.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&receipt).expect("json"),
    )
    .expect("receipt");
    let ctx = night();
    let mut app = CantusApp::from_parts(
        Some(cantus_ui::welcome_project()),
        EngineHandle::dummy(),
        false,
    );
    app.settings_path = root.join("settings.json");
    app.samples.content_root = Some(root.clone());
    app.samples.manifest = Some(bundled.clone());
    app.settings.sample_folders.clear();
    app.samples.window.open = true;
    app.samples.window.selected = Some("salamander-grand-piano".into());
    let mut frame = 0;
    raster::shoot(&ctx, egui::vec2(1100.0, 760.0), 1.0, 6, None, |root| {
        frame += 1;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
            .show(root, |ui| {
                if frame > 1 {
                    let ctx = ui.ctx().clone();
                    cantus_ui::views::samples_content::window(&mut app, &ctx);
                }
            });
    })
}

/// Every picture, by its file name (without `.png`).
const ALL: &[&str] = &[
    "window-night",
    "window-night-720",
    "window-paper-720",
    "mixer",
    "inspector",
    "matrix",
    "linked",
    "pianoroll",
    "rack",
    "appearance",
    "themes",
    "mark",
    "packs",
    "fx-eq",
    "fx-comp",
    "fx-limiter",
    "fx-space",
    "fx-multiband",
    "fx-deesser",
    "fx-gate",
    "fx-saturator",
    "fx-stereo",
    "fx-maximizer",
    "fx-tonsor",
    "fx-impetus",
    "fx-discrimen",
    "fx-speculum",
    "fx-tonus",
    "fx-concentus",
    "fx-flexus",
    "fx-phasis",
    "fx-tremor",
    "fx-vibrans",
    "fx-anulus",
    "fx-translatio",
    "fx-fluctus",
    "fx-hosted",
    "inst-wavetable",
    "inst-analog",
    "inst-fm",
    "inst-sampler",
    "inst-bass",
    "inst-organ",
    "inst-epiano",
    "inst-pulsus",
    "inst-chorda",
    "score-roll",
    "score-pane",
    "score-page",
];

fn shoot_one(name: &str, dir: &Path) {
    let path = dir.join(format!("{name}.png"));
    match name {
        "window-night" => save(
            &window_with(
                "cantus_night",
                (1920.0, 1080.0),
                whole_song(),
                22,
                hero,
                hero_frames,
            ),
            None,
            &path,
        ),
        "window-night-720" => save(
            &window("cantus_night", (1280.0, 720.0), compact),
            None,
            &path,
        ),
        "window-paper-720" => save(
            &window("cantus_paper", (1280.0, 720.0), compact),
            None,
            &path,
        ),
        "mixer" => save(&alone((1240.0, 560.0), desk, draw_mixer), None, &path),
        "inspector" => save(
            &alone((290.0, 1000.0), nothing, draw_inspector),
            None,
            &path,
        ),
        "matrix" => save(&alone((700.0, 600.0), nothing, draw_routing), None, &path),
        // The top of the window: the clip browser, the arrangement and the inspector.
        "linked" => {
            let c = window("cantus_night", (1760.0, 1000.0), linked);
            let top = Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1760.0, LINKED_ARRANGEMENT + 52.0),
            );
            save(&c, Some(top), &path);
        }
        "pianoroll" => save(&alone((1240.0, 640.0), lead, draw_piano_roll), None, &path),
        "rack" => save(&alone((1240.0, 640.0), drums, draw_rack), None, &path),
        "appearance" => {
            let c = alone((1100.0, 900.0), appearance, draw_appearance);
            save(&c, Some(trimmed(&c, 0.0)), &path);
        }
        "themes" => save(&themes(), None, &path),
        "mark" => {
            let (w, h, rgb) = mark();
            write_png(w, h, &rgb, &path);
        }
        "packs" => {
            let c = packs();
            save(&c, Some(trimmed(&c, 0.0)), &path);
        }
        "score-roll" => save(&score_roll(), None, &path),
        "score-pane" => {
            save(&score_pane(), None, &path);
        }
        "score-page" => {
            let c = score_page();
            save(&c, Some(trimmed(&c, 30.0)), &path);
        }
        n if n.starts_with("fx-") => save(&effect(&n[3..]), None, &path),
        n if n.starts_with("inst-") => {
            let (canvas, crop) = instrument(&n[5..]);
            save(&canvas, crop, &path);
        }
        n => eprintln!("no picture named {n}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/images"));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut written = Vec::new();
    for name in ALL {
        if only.is_empty() || only.iter().any(|o| name.starts_with(o.as_str())) {
            shoot_one(name, &dir);
            written.push(dir.join(format!("{name}.png")));
        }
    }
    // Smaller still, losslessly, where oxipng is installed (`cargo install oxipng`): about
    // a fifth off what the encoder writes.
    let oxipng = std::process::Command::new("oxipng")
        .args(["-o", "max", "--strip", "safe", "-q"])
        .args(&written)
        .status();
    match oxipng {
        Ok(s) if s.success() => println!("optimised {} pictures with oxipng", written.len()),
        _ => println!("oxipng not found: the pictures are as the encoder wrote them"),
    }
}

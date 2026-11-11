//! Round 39, `el-views`: des-views' views reached the way a user reaches them — the app
//! booted through `CantusApp::assemble` on the demo song, the piano roll, the channel
//! rack, the browser and the arrangement at its four zooms brought to the front, the whole
//! window rendered by the CPU renderer and its pixels read.
//!
//! - The keyboard is ivory and ebony with a 1-px gap between white keys (the bed shows in
//!   one-pixel runs down a white key's column), in Slate Night and Slate Paper.
//! - The rack's lit steps carry their channel's hue.
//! - The browser, the rack and the clips at every zoom render.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CentralView, Files};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster::{self, Canvas};
use cantus_ui::views::room::Piano;
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
    M.lock().unwrap_or_else(|e| e.into_inner())
}

fn boot(dir: &std::path::Path, preset: &str, dark: bool) -> (CantusApp, egui::Context) {
    let (app, ctx, _proc) = boot_with(dir, preset, dark);
    (app, ctx)
}

/// [`boot`], keeping the engine's processor (a test that plays runs it).
fn boot_with(
    dir: &std::path::Path,
    preset: &str,
    dark: bool,
) -> (CantusApp, egui::Context, cantus_engine::Processor) {
    std::fs::write(
        dir.join("settings.json"),
        // No start page over the window: the tests click and read the views under it.
        r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
    )
    .unwrap();
    std::env::set_var("CANTUS_DATA_DIR", dir);
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::demos::startup_project()),
        engine,
        None,
        None,
        None,
        false,
    );
    let mode = if dark { Mode::Dark } else { Mode::Light };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.apply_appearance();
    (app, ctx, proc)
}

fn shot(app: &mut CantusApp, ctx: &egui::Context) -> Canvas {
    raster::shoot(ctx, egui::vec2(1280.0, 720.0), 1.0, 6, None, |root| {
        app.frame(root)
    })
}

fn near(p: [u8; 4], c: egui::Color32, tol: i32) -> bool {
    let [r, g, b, _] = c.to_array();
    (p[0] as i32 - r as i32).abs() <= tol
        && (p[1] as i32 - g as i32).abs() <= tol
        && (p[2] as i32 - b as i32).abs() <= tol
}

/// The column through the white keys' fronts: the one with the most ivory.
fn ivory_column(c: &Canvas, k: &Piano) -> (usize, usize) {
    (0..c.width / 2)
        .map(|x| {
            let n = (0..c.height)
                .filter(|&y| near(c.pixel(x, y), k.ivory, 10))
                .count();
            (x, n)
        })
        .max_by_key(|(_, n)| *n)
        .unwrap()
}

#[test]
fn the_piano_roll_s_keyboard_is_ivory_and_ebony_with_a_gap_between_white_keys() {
    let _one = serial();
    for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, ctx) = boot(dir.path(), preset, dark);
        // A melodic clip (a kit's clip shows named pads instead of keys).
        let (t, c) = app
            .project()
            .tracks
            .iter()
            .filter(|t| !matches!(t.instrument, cantus_core::InstrumentSpec::Pulsus(_)))
            .find_map(|t| {
                t.clips
                    .iter()
                    .find(|c| c.notes.len() > 8)
                    .map(|c| (t.id, c.id))
            })
            .expect("the demo song has a melodic clip");
        app.selected_track = Some(t);
        app.selected_clip = Some(c);
        app.piano_roll.key_height = 12.0;
        app.show_central(CentralView::PianoRoll);
        let canvas = shot(&mut app, &ctx);
        let k = Piano::of(&cantus_ui::kit::tokens::current());
        let (x, n) = ivory_column(&canvas, &k);
        assert!(
            n > 120,
            "{preset}: white keys in ivory ({n} pixels at x {x})"
        );
        // Down a column through the white keys: the bed in short runs (the 1-px gap
        // under each white key, a pixel of its shade above it) between ivory. In Paper the
        // ivory is near the panel, so the column is the one with the most such gaps.
        let gaps_at = |x: usize| {
            let mut gaps = 0;
            let mut y = 2;
            while y + 3 < canvas.height {
                if near(canvas.pixel(x, y), k.bed, 16) {
                    let mut end = y;
                    while end + 1 < canvas.height && near(canvas.pixel(x, end + 1), k.bed, 16) {
                        end += 1;
                    }
                    if end - y < 2
                        && near(canvas.pixel(x, y - 2), k.ivory, 45)
                        && near(canvas.pixel(x, end + 2), k.ivory, 45)
                    {
                        gaps += 1;
                    }
                    y = end + 1;
                } else {
                    y += 1;
                }
            }
            gaps
        };
        let gaps = (0..canvas.width / 3).map(gaps_at).max().unwrap_or(0);
        assert!(
            gaps >= 6,
            "{preset}: one-pixel gaps between white keys: {gaps}"
        );
        // The black keys: ebony beside the ivory, toward the back.
        let ebony = (0..canvas.height)
            .filter(|&y| (0..canvas.width / 3).any(|xx| near(canvas.pixel(xx, y), k.ebony, 12)))
            .count();
        assert!(ebony > 40, "{preset}: black keys ({ebony} rows)");
    }
}

#[test]
fn the_rack_s_lit_steps_carry_their_channel_s_hue_and_every_view_renders() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, ctx) = boot(dir.path(), "slate_night", true);
    app.show_central(CentralView::ChannelRack);
    let canvas = shot(&mut app, &ctx);
    // Some track's own colour, mixed most of the way into a step's well, shows.
    let room = cantus_ui::views::room::Room::of(&cantus_ui::kit::tokens::current());
    let hues: Vec<egui::Color32> = app
        .project()
        .tracks
        .iter()
        .map(|t| cantus_ui::theme::track_color(t.color))
        .collect();
    let lit = hues.iter().any(|h| {
        let c = cantus_ui::kit::color::mix(room.well, *h, 0.80);
        let mut n = 0;
        for y in 0..canvas.height {
            for x in 0..canvas.width {
                if near(canvas.pixel(x, y), c, 6) {
                    n += 1;
                }
            }
        }
        n > 60
    });
    assert!(lit, "a lit step in its channel's hue");
    // The browser and the arrangement's four zooms render through the boot path.
    app.toggle_pane(Pane::Browser);
    let _ = shot(&mut app, &ctx);
    for ppb in [4.0, 12.0, 36.0, 90.0] {
        app.show_central(CentralView::Mixer);
        app.arrangement.px_per_beat = ppb;
        let _ = shot(&mut app, &ctx);
    }
}

/// dir-a's toolbar folds (`b-toolbar-*`), through the boot path: the piano roll's row shows
/// everything in a wide window and, in a narrow one, folds as far as its name's glyph and
/// its tools to the chosen one — and unfolds when the window widens again.
#[test]
fn the_piano_roll_s_toolbar_folds_in_a_narrow_window_and_unfolds_in_a_wide_one() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, ctx) = boot(dir.path(), "slate_night", true);
    let (t, c) = app
        .project()
        .tracks
        .iter()
        .find_map(|t| {
            t.clips
                .iter()
                .find(|c| !c.notes.is_empty())
                .map(|c| (t.id, c.id))
        })
        .expect("the demo song has notes");
    app.selected_track = Some(t);
    app.selected_clip = Some(c);
    app.show_central(CentralView::PianoRoll);
    let mut at = |w: f32| {
        let _ = raster::shoot(&ctx, egui::vec2(w, 720.0), 1.0, 6, None, |root| {
            app.frame(root)
        });
        app.piano_roll.bar_fold
    };
    let wide = at(2400.0);
    let narrow = at(620.0);
    let again = at(2400.0);
    assert_eq!(wide, 0, "a wide window folds nothing");
    assert!(
        narrow >= 2,
        "a narrow window folds the name and more: {narrow}"
    );
    assert_eq!(again, 0, "widened, it unfolds");
}

// ---- session 2: the rack's map and audio rows, the browser's search and preview, clip
// states, each through the boot path ----------------------------------------------------

/// A clock for [`frame_with`]: egui tells a click from a press by the time between them.
static FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// One frame of the whole window at `size` with `events`: what it painted.
fn frame_with(
    app: &mut CantusApp,
    ctx: &egui::Context,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        time: Some(FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as f64 / 60.0),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |root| app.frame(root));
    out.textures_delta.clear();
    out.shapes
}

/// Every text a frame painted: its words, where, and in what colour.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect, egui::Color32)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(String, egui::Rect, egui::Color32)>) {
        match s {
            egui::Shape::Text(t) => {
                let section = t.galley.job.sections.first().map(|s| s.format.color);
                let mut c = t
                    .override_text_color
                    .or(section)
                    .unwrap_or(t.fallback_color);
                if c == egui::Color32::PLACEHOLDER {
                    c = t.fallback_color;
                }
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                out.push((t.galley.job.text.clone(), rect, c));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// A press and a release of the primary button at `at`, over two frames.
fn click(app: &mut CantusApp, ctx: &egui::Context, size: egui::Vec2, at: egui::Pos2) {
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame_with(app, ctx, size, vec![egui::Event::PointerMoved(at)]);
    let _ = frame_with(app, ctx, size, vec![press(true)]);
    let _ = frame_with(app, ctx, size, vec![press(false)]);
    let _ = frame_with(app, ctx, size, vec![]);
}

/// A sound file: `seconds` of a 220-Hz tone at 48 kHz, mono.
fn tone(path: &std::path::Path, seconds: f32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..(48_000.0 * seconds) as i32 {
        let v = (i as f32 * 220.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.6;
        w.write_sample((v * i16::MAX as f32) as i16).unwrap();
    }
    w.finalize().unwrap();
}

/// Frames until `done` says so, at most `secs` seconds (sounds load off the UI thread).
fn until(
    app: &mut CantusApp,
    ctx: &egui::Context,
    size: egui::Vec2,
    secs: u64,
    mut done: impl FnMut(&CantusApp, &[egui::epaint::ClippedShape]) -> bool,
) -> bool {
    let t = std::time::Instant::now();
    while t.elapsed().as_secs() < secs {
        let shapes = frame_with(app, ctx, size, vec![]);
        if done(app, &shapes) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

/// The rack's head carries the pattern's map — one cell a bar — and a click on a bar pages
/// the steps there; an audio channel's row draws its sound over the page.
#[test]
fn the_rack_s_map_pages_to_a_bar_and_an_audio_channel_draws_its_sound() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, ctx) = boot(dir.path(), "slate_night", true);
    // An audio channel: a tone on a new track under the pattern.
    let wav = dir.path().join("tone.wav");
    tone(&wav, 2.0);
    assert_eq!(app.import_audio(&[wav], cantus_core::Ticks::ZERO, None), 1);
    // A melodic clip's pattern, eight bars of sixteenths (the demo song's Drop).
    let (t, c) = app
        .project()
        .tracks
        .iter()
        .find_map(|t| {
            t.clips
                .iter()
                .find(|c| !c.notes.is_empty() && c.length >= cantus_core::Ticks::beats(32))
                .map(|c| (t.id, c.id))
        })
        .expect("an eight-bar clip");
    app.select_clip(t, c);
    app.show_central(CentralView::ChannelRack);
    let size = egui::vec2(1600.0, 1800.0);
    for _ in 0..4 {
        let _ = frame_with(&mut app, &ctx, size, vec![]);
    }
    let (map, bars) = app.rack.map.expect("the pattern map is in the rack's head");
    let per_bar = 16.0; // sixteenths in a 4/4 bar
    let clip = app.project().track(t).unwrap().clip(c).unwrap();
    let want = ((clip.length.0 as f32 / cantus_core::Ticks::SIXTEENTH.0 as f32) / per_bar)
        .ceil()
        .min(8.0) as usize;
    assert_eq!(bars, want, "one cell a bar, eight at most");
    assert!((map.width() / bars as f32 - 22.0).abs() < 1.0, "{map:?}");
    // The audio row: its peaks once the file is read (off the interface's thread), on the
    // first page, where the tone is.
    let drew = until(&mut app, &ctx, size, 20, |a, _| {
        a.rack.audio_columns.get() > 20
    });
    assert!(drew, "the audio channel's row drew its sound");
    // A click on the third bar pages there.
    let (map, bars) = app.rack.map.expect("the map, still");
    let cell = map.width() / bars as f32;
    click(
        &mut app,
        &ctx,
        size,
        egui::pos2(map.left() + cell * 2.5, map.center().y),
    );
    assert!(!app.rack.fit, "a page chosen leaves the fit");
    assert_eq!(
        app.rack.scroll_steps,
        2.0 * per_bar,
        "the third bar's first step"
    );
}

/// The browser's search lights the match inside a row's name, in the accent; a chosen
/// sound's preview shows how long it is.
#[test]
fn the_browser_lights_a_search_s_match_and_the_preview_shows_its_time() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, ctx) = boot(dir.path(), "slate_night", true);
    let sounds = dir.path().join("sounds");
    std::fs::create_dir_all(&sounds).unwrap();
    let wav = sounds.join("Deep Kick 01.wav");
    tone(&wav, 0.5);
    app.settings.sample_folders = vec![sounds.clone()];
    if !app.workspace.is_visible(Pane::Browser) {
        app.toggle_pane(Pane::Browser);
    }
    app.samples.query = "kick".to_string();
    let size = egui::vec2(1600.0, 900.0);
    let accent = cantus_ui::views::room::Room::of(&cantus_ui::kit::tokens::current()).accent;
    let mut seen = Vec::new();
    let lit = until(&mut app, &ctx, size, 20, |_, shapes| {
        seen = texts(shapes);
        seen.iter().any(|(t, _, c)| t == "Kick" && *c == accent)
    });
    assert!(
        lit,
        "the match painted in the accent: {:?}",
        seen.iter()
            .filter(|(t, _, _)| t.contains("Kick") || t.contains("Deep"))
            .collect::<Vec<_>>()
    );
    // The rest of the name in the row's own ink, beside it.
    assert!(seen
        .iter()
        .any(|(t, _, c)| t.starts_with("Deep") && *c != accent));
    // The preview: chosen, it reads its length ("0:00.5").
    app.samples.selected = Some(wav);
    let timed = until(&mut app, &ctx, size, 20, |_, shapes| {
        texts(shapes)
            .iter()
            .any(|(t, _, _)| t.starts_with("0:00.5"))
    });
    assert!(timed, "the preview shows the sound's time");
}

/// Clip states (des-views): a name strip in lanes of 44 and taller, none in a short lane
/// at the far zoom; a playing clip lit along its top; a muted clip hatched.
#[test]
fn clips_show_their_name_strip_their_playing_light_and_their_muted_hatch() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, ctx, mut proc) = boot_with(dir.path(), "slate_night", true);
    app.show_central(CentralView::Mixer);
    app.clear_clip_selection();
    app.selected_clip = None; // nothing selected: a selected clip names itself anywhere
    let size = egui::vec2(1600.0, 1000.0);
    // The first track's first clip, from the song's start.
    let (t, c, name) = {
        let tr = &app.project().tracks[0];
        let clip = tr
            .clips
            .iter()
            .find(|c| c.start.0 == 0)
            .expect("a clip at the start");
        (tr.id, clip.id, clip.name.clone())
    };
    let named_in_row = |app: &mut CantusApp| {
        let shapes = frame_with(app, &ctx, size, vec![]);
        let row = app.arrangement.geometry().unwrap().rows[0];
        texts(&shapes).iter().any(|(s, r, _)| {
            *s == name && r.top() >= row.top - 0.5 && r.bottom() <= row.top + row.height + 0.5
        })
    };
    app.arrangement.px_per_beat = 4.0;
    app.arrangement.track_height = 44.0;
    for _ in 0..3 {
        let _ = frame_with(&mut app, &ctx, size, vec![]);
    }
    assert!(
        named_in_row(&mut app),
        "a 44-px lane names its clip in the strip"
    );
    let row = app.arrangement.geometry().unwrap().rows[0];
    let strip_top = texts(&frame_with(&mut app, &ctx, size, vec![]))
        .into_iter()
        .find(|(s, r, _)| *s == name && r.top() >= row.top - 0.5)
        .map(|(_, r, _)| r);
    let strip = strip_top.expect("the name");
    assert!(
        strip.center().y <= row.top + 2.0 + 13.0,
        "in the 13-px strip: {strip:?}"
    );
    app.arrangement.track_height = 30.0;
    for _ in 0..3 {
        let _ = frame_with(&mut app, &ctx, size, vec![]);
    }
    assert!(
        !named_in_row(&mut app),
        "a short lane at the far zoom leaves the name out"
    );

    // Playing and muted, read from the pixels of the clip's body.
    app.arrangement.track_height = 44.0;
    app.arrangement.px_per_beat = 30.0;
    app.arrangement.scroll_beats = 0.0;
    let body = |app: &CantusApp| {
        let geo = app.arrangement.geometry().unwrap();
        let row = geo.rows[0];
        let inset = (row.height * 0.04).clamp(1.0, 2.0);
        let clip = app.project().track(t).unwrap().clip(c).unwrap();
        egui::Rect::from_min_max(
            egui::pos2(geo.x(clip.start.as_beats_f64() as f32), row.top + inset),
            egui::pos2(
                geo.x(clip.end().as_beats_f64() as f32),
                row.top + row.height - inset,
            ),
        )
    };
    let lum = |p: [u8; 4]| p[0] as u32 + p[1] as u32 + p[2] as u32;
    let still = shot(&mut app, &ctx);
    let b = body(&app);
    // The lit top: a pixel row under the snapped body's edge, past the name's start.
    let (x, y) = ((b.left() + 40.0) as usize, (b.top().round() + 1.0) as usize);
    app.toggle_play();
    let mut audio = vec![0.0f32; 800 * 2];
    proc.process(&mut audio, 2);
    let _ = frame_with(&mut app, &ctx, size, vec![]);
    assert!(app.engine.is_playing(), "the transport plays");
    let playing = shot(&mut app, &ctx);
    assert!(
        lum(playing.pixel(x, y)) > lum(still.pixel(x, y)) + 20,
        "the playing clip's top is lit: {:?} against {:?}",
        playing.pixel(x, y),
        still.pixel(x, y)
    );
    app.toggle_play();
    proc.process(&mut audio, 2);
    // Muted: a 1-px hatch across the body, under the notes' row.
    let line = |c: &Canvas, b: egui::Rect| {
        let y = (b.bottom() - 3.0) as usize;
        let xs = (b.left() as usize + 4)..(b.right().min(c.width as f32 - 1.0) as usize - 4);
        let base = c.pixel(xs.start, y);
        xs.filter(|&x| c.pixel(x, y) != base).count()
    };
    let open = line(&shot(&mut app, &ctx), body(&app));
    assert!(app.exec(cantus_core::Command::SetClipMute {
        track: t,
        clip: c,
        muted: true
    }));
    let muted = shot(&mut app, &ctx);
    let hatched = line(&muted, body(&app));
    assert!(
        hatched > open + 20,
        "the muted clip is hatched: {hatched} marked pixels against {open}"
    );
}

//! Screenshots of linked clips (ADR 0037) without a screen: the arrangement with a drum
//! pattern linked four times (one copy selected, its siblings outlined), a bass line
//! linked across two tracks, one copy made unique, two linked audio clips; and the clip's
//! right-click menu on a linked copy with *Make unique*, *Select linked copies* and *Link
//! selection to this clip*; and the empty timeline's menu with its *Copies are linked*
//! switch. Round 26: the *Link to…* picker (all of it, and filtered), and the inspector
//! naming a linked clip's group.
//!
//! `cargo run -p cantus-ui --example shoot_links -- <out-dir> [es] [light] [z0.75|z2] [kind=…]`

use cantus_core::{AudioRegion, AudioSource, AudioSourceId, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::keymap::Action;
use cantus_ui::raster::shoot_events;
use cantus_ui::{starter_project, CantusApp};
use std::cell::Cell;
use std::path::PathBuf;

const W: f32 = 1400.0;
const H: f32 = 820.0;

/// Drums, a bass on two tracks, and a vocal track with an audio clip.
fn project() -> cantus_core::Project {
    let mut p = starter_project();
    p.tracks[0].clips.clear();
    p.tracks[0].name = "Drums".into();
    let mut beat = p.make_clip("Beat A", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [36u8, 42, 38, 42, 36, 36, 38, 42].into_iter().enumerate() {
        let n = p.make_note(Ticks::EIGHTH * i as i64, Ticks::SIXTEENTH, pitch, 100);
        beat.notes.push(n);
    }
    p.tracks[0].clips.push(beat);
    for name in ["Bass", "Bass double"] {
        let mut t = p.make_track(name);
        if name == "Bass" {
            let mut c = p.make_clip("Bass line", Ticks::beats(4), Ticks::beats(8));
            for (i, pitch) in [36u8, 36, 43, 41, 36, 39].into_iter().enumerate() {
                let n = p.make_note(Ticks::beats(i as i64), Ticks::beats(1), pitch, 100);
                c.notes.push(n);
            }
            t.clips.push(c);
        }
        p.tracks.push(t);
    }
    let src = AudioSource {
        id: AudioSourceId(p.alloc_id()),
        name: "Vocal".into(),
        path: "C:/nowhere/vocal.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 8,
        sha256: None,
        recorded: false,
    };
    let mut vocal = p.make_track("Vocal");
    let mut take = p.make_clip("Chorus vocal", Ticks::beats(0), Ticks::beats(6));
    let mut region = AudioRegion::of(src.id);
    region.fade_in = Ticks::QUARTER;
    region.fade_out = Ticks::beats(1);
    take.audio = Some(region);
    vocal.clips.push(take);
    p.audio_pool.push(src);
    p.tracks.push(vocal);
    p
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The app with the copies made the way a musician makes them: Duplicate three times on
/// the beat, an Alt-drag of the bass onto the next track, a fourth beat made unique, and
/// Duplicate on the vocal.
fn app(ctx: &egui::Context, light: bool) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut a = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
    // Never touch the user's files; Cantus Night, or Cantus Paper for `light`.
    let tmp = std::env::temp_dir().join("cantus-shoot-links");
    a.settings_path = tmp.join("settings.json");
    a.layouts_path = tmp.join("layouts.json");
    a.keymap_path = tmp.join("keymap.toml");
    a.appearance_path = tmp.join("appearance.json");
    a.appearance.mode = if light { Mode::Light } else { Mode::Dark };
    let preset = if light {
        "cantus_paper"
    } else {
        "cantus_night"
    };
    a.appearance
        .assign(a.appearance.mode, format!("preset:{preset}"));
    a.attach(ctx);
    a.focus_arrangement();
    let t = |a: &CantusApp, i: usize| a.project().tracks[i].id;
    let c = |a: &CantusApp, i: usize| a.project().tracks[i].clips[0].id;
    let (t0, c0) = (t(&a, 0), c(&a, 0));
    a.select_clip(t0, c0);
    for _ in 0..4 {
        a.run_action(Action::Duplicate);
    }
    // The last copy becomes a variation of its own.
    a.run_action(Action::MakeUnique);
    let last = a.clip_selection()[0];
    a.rename_clip(last, "Beat B".into());
    let (t1, c1) = (t(&a, 1), c(&a, 1));
    a.select_clip(t1, c1);
    a.commit_clip_drag(
        (t1, c1),
        cantus_ui::views::arrangement::ClipDragResult::Move {
            delta: Ticks::beats(8),
            rows: 1,
            copy: true,
        },
    );
    let (t3, c3) = (t(&a, 3), c(&a, 3));
    a.select_clip(t3, c3);
    a.run_action(Action::Duplicate);
    // One beat selected: its three siblings are outlined.
    let second = a.project().tracks[0].clips[1].id;
    a.select_clip(t0, second);
    a.set_status(String::new());
    a
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "shots".into()));
    let spanish = args.iter().any(|a| a == "es");
    // Round 26: a light theme (Cantus Paper) besides Cantus Night, and the UI scale.
    let light = args.iter().any(|a| a == "light");
    let zoom: f32 = args
        .iter()
        .find_map(|a| a.strip_prefix('z').and_then(|z| z.parse().ok()))
        .unwrap_or(1.0);
    let only: Option<&str> = args
        .iter()
        .find_map(|a| a.strip_prefix("kind=").map(|k| k as &str));
    std::fs::create_dir_all(&dir).expect("output dir");
    let (locale, mut suffix) = if spanish {
        (cantus_ui::i18n::Locale::Es, "-es".to_string())
    } else {
        (cantus_ui::i18n::Locale::En, String::new())
    };
    if light {
        suffix.push_str("-paper");
    }
    if zoom != 1.0 {
        suffix.push_str(&format!("-z{zoom}"));
    }
    cantus_ui::i18n::set_locale(locale);
    for kind in [
        "arrangement",
        "menu",
        "timeline-menu",
        "picker",
        "picker-filter",
        "inspector",
        "browser",
        "browser-drag",
        "browser-paint",
    ] {
        if only.is_some_and(|o| o != kind) {
            continue;
        }
        let menu = kind.ends_with("menu");
        let ctx = context();
        let mut a = app(&ctx, light);
        let t0 = a.project().tracks[0].id;
        let (b1, b4) = (
            a.project().tracks[0].clips[1].id,
            a.project().tracks[0].clips[4].id,
        );
        match kind {
            // Two beats selected, so the menu also offers to link the selection here.
            "menu" => a.set_clip_selection(vec![(t0, b4), (t0, b1)]),
            // "Beat B" (made unique) asks to become a group chosen by name.
            "picker" | "picker-filter" => {
                a.select_clip(t0, b4);
                a.run_action(Action::LinkTo);
            }
            // The clip browser beside the arrangement (FL's picker panel).
            "browser" | "browser-drag" | "browser-paint" => {
                a.show_pane(cantus_ui::views::workspace::Pane::Clips);
                vocal_peaks(&a);
                if kind == "browser-paint" {
                    // The bass line chosen in the browser, the paint tool out (B).
                    let t1 = a.project().tracks[1].id;
                    let bass = a.project().tracks[1].clips[0].id;
                    a.clip_browser.set_brush(Some((t1, bass)));
                    a.focus_arrangement();
                    a.toggle_arrangement_paint();
                }
            }
            _ => {}
        }
        let aim: Cell<Option<egui::Pos2>> = Cell::new(None);
        // `browser-drag`: the bass line's tile, held over bar 7 of the vocal track.
        let tile: Cell<Option<egui::Pos2>> = Cell::new(None);
        let drop: Cell<Option<egui::Pos2>> = Cell::new(None);
        let path = dir.join(format!("links-{kind}{suffix}.png"));
        shoot_events(
            &ctx,
            egui::vec2(W, H),
            zoom,
            18,
            Some(&path),
            |frame, _| match (menu, aim.get()) {
                (true, Some(p)) if frame == 6 => vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Secondary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Secondary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                _ if kind == "picker-filter" && frame == 4 => {
                    vec![egui::Event::Text("ba".into())]
                }
                // Pressed on the tile at frame 6, moved to the drop point, still held.
                _ if kind.starts_with("browser-") && (6..=16).contains(&frame) => {
                    match (tile.get(), drop.get()) {
                        (Some(from), Some(to)) => {
                            let p = from + (to - from) * ((frame - 6) as f32 / 10.0);
                            let mut ev = vec![egui::Event::PointerMoved(p)];
                            if frame == 6 {
                                ev.push(egui::Event::PointerButton {
                                    pos: p,
                                    button: egui::PointerButton::Primary,
                                    pressed: true,
                                    modifiers: egui::Modifiers::NONE,
                                });
                            }
                            ev
                        }
                        _ => Vec::new(),
                    }
                }
                _ => Vec::new(),
            },
            |root| {
                let mut frame = eframe::Frame::_new_kittest();
                eframe::App::ui(&mut a, root, &mut frame);
                if tile.get().is_none() && kind == "browser-drag" {
                    let t1 = a.project().tracks[1].id;
                    let bass = a.project().tracks[1].clips[0].id;
                    let r = a
                        .clip_browser
                        .tile_rects()
                        .iter()
                        .find(|(r, _)| *r == (t1, bass));
                    tile.set(r.map(|(_, r)| r.center()));
                }
                if let Some(g) = a.arrangement.geometry() {
                    drop.set(Some(egui::pos2(g.x(24.5), g.row_y(3))));
                    if kind == "browser-paint" {
                        // A stroke along the bass row from bar 5, still held: its ghosts.
                        tile.set(Some(egui::pos2(g.x(16.3), g.row_y(1))));
                        drop.set(Some(egui::pos2(g.x(30.0), g.row_y(1))));
                    }
                    // The second beat (bar 2), in its title bar; or empty timeline at bar
                    // 7 on the vocal track.
                    aim.set(Some(if kind == "menu" {
                        egui::pos2(g.x(6.0), g.rows[0].top + 8.0)
                    } else {
                        egui::pos2(g.x(24.0), g.row_y(3))
                    }));
                }
            },
        );
        println!("wrote {}", path.display());
    }
}

/// Peaks for the vocal take (its file is not on this machine): eight seconds of a voice-like
/// envelope over noise, handed to the store as an import would, so the clip browser's
/// tile and the arrangement draw a waveform.
fn vocal_peaks(a: &CantusApp) {
    let mut seed = 0x2545_f491_u32;
    let frames: Vec<f32> = (0..48_000 * 8)
        .flat_map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let phrase = (i % 48_000) as f32 / 48_000.0;
            let env = if phrase < 0.7 {
                0.9 * (1.0 - phrase)
            } else {
                0.05
            };
            [noise * env, noise * env * 0.9]
        })
        .collect();
    a.peaks.insert_data(
        std::path::Path::new("C:/nowhere/vocal.wav"),
        std::sync::Arc::new(cantus_core::SampleData {
            stream: None,
            rate: 48_000,
            channels: 2,
            frames: frames.into(),
        }),
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
}

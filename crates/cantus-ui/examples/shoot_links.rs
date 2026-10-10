//! Screenshots of linked clips (ADR 0037) without a screen: the arrangement with a drum
//! pattern linked four times (one copy selected, its siblings outlined), a bass line
//! linked across two tracks, one copy made unique, two linked audio clips; and the clip's
//! right-click menu on a linked copy with *Make unique*, *Select linked copies* and *Link
//! selection to this clip*; and the empty timeline's menu with its *Copies are linked*
//! switch.
//!
//! `cargo run -p cantus-ui --example shoot_links -- <out-dir> [es]`

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
fn app(ctx: &egui::Context) -> CantusApp {
    let mut a = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
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
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let spanish = args.any(|a| a == "es");
    std::fs::create_dir_all(&dir).expect("output dir");
    let (locale, suffix) = if spanish {
        (cantus_ui::i18n::Locale::Es, "-es")
    } else {
        (cantus_ui::i18n::Locale::En, "")
    };
    cantus_ui::i18n::set_locale(locale);

    for kind in ["arrangement", "menu", "timeline-menu"] {
        let menu = kind != "arrangement";
        let ctx = context();
        let mut a = app(&ctx);
        if kind == "menu" {
            // Two beats selected, so the menu also offers to link the selection here.
            let t0 = a.project().tracks[0].id;
            let (b1, b4) = (
                a.project().tracks[0].clips[1].id,
                a.project().tracks[0].clips[4].id,
            );
            a.set_clip_selection(vec![(t0, b4), (t0, b1)]);
        }
        let aim: Cell<Option<egui::Pos2>> = Cell::new(None);
        let path = dir.join(format!("links-{kind}{suffix}.png"));
        shoot_events(
            &ctx,
            egui::vec2(W, H),
            1.0,
            12,
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
                _ => Vec::new(),
            },
            |root| {
                let mut frame = eframe::Frame::_new_kittest();
                eframe::App::ui(&mut a, root, &mut frame);
                if let Some(g) = a.arrangement.geometry() {
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

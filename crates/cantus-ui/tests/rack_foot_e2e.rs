//! The channel rack's last measures (round 39, r39:lo-rest; des-views' *Channel rack*),
//! through `CantusApp::assemble` on the demo song the program opens with, driven as eframe
//! drives it:
//! - the name plates sit at 104, 150 × 20, a kit's pads stepped in 14, the target marks at
//!   262, 3 × 14 — read from the rows the rack drew;
//! - a kit's chevron folds its pads under its own row and opens them again;
//! - the foot under the last row: "Add a channel" (150 × 24) opens the instrument picker
//!   for a new channel at the end, and the drop zone (56 high) is drawn with its words.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::CantusApp;
use egui::{vec2, Event, Pos2, Rect};
use std::path::Path;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    proc_: Processor,
    t: f64,
}

fn boot(dir: &Path) -> Rig {
    std::fs::write(
        dir.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    std::env::set_var("CANTUS_DATA_DIR", dir);
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc_) = EngineHandle::manual(48_000.0);
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
    app.show_central(CentralView::ChannelRack);
    let mut r = Rig {
        app,
        ctx,
        proc_,
        t: 0.0,
    };
    r.frames(4, None);
    r
}

impl Rig {
    /// One frame as eframe runs it; every text drawn, with where it is seen.
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let mut audio = vec![0f32; 800 * 2];
        self.proc_.process(&mut audio, 2);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(app, ui, &mut frame);
        });
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()).intersect(clip),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
                _ => {}
            }
        }
        let mut v = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, c.clip_rect, &mut v);
        }
        v
    }

    fn frames(&mut self, n: usize, at: Option<Pos2>) -> Vec<(String, Rect)> {
        let mut last = Vec::new();
        for _ in 0..n {
            last = self.frame(at.map(Event::PointerMoved).into_iter().collect());
        }
        last
    }

    fn click(&mut self, at: Pos2) {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![Event::PointerMoved(at), button(true)]);
        self.frame(vec![Event::PointerMoved(at), button(false)]);
        self.frame(vec![]);
    }

    /// The rows the rack laid out on the last frame.
    fn rows(&self) -> usize {
        self.app.rack.geometry().map_or(0, |g| g.rows.len())
    }
}

/// Where a text is seen, if it is.
fn seen<'a>(texts: &'a [(String, Rect)], s: &str) -> Option<&'a Rect> {
    texts
        .iter()
        .find(|(t, r)| t == s && r.is_positive())
        .map(|(_, r)| r)
}

#[test]
fn the_plates_the_marks_a_kits_fold_and_the_foot() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = boot(dir.path());
    let texts = r.frames(2, None);
    // Only what the channel column shows.
    let g = r.app.rack.geometry().unwrap();
    let (grid_left, grid_top) = (g.grid_left, g.grid_top);
    let col_left = grid_left - 268.0;
    let column: Vec<(String, Rect)> = texts
        .iter()
        .filter(|(_, r)| r.left() >= col_left && r.right() <= grid_left && r.top() >= grid_top)
        .cloned()
        .collect();
    // A plate's name starts 9 into its plate at 104: a channel's at 113 from the column's
    // left, a pad's 14 further in. The demo's kick is a channel, its clap kit has pads.
    let kick = *seen(&column, "Kick").expect("the kick's plate");
    let snare = *seen(&column, "Snare").expect("a pad's plate");
    let k = cantus_ui::kit::Style::current().k;
    assert!(
        (kick.left() - (col_left + 113.0 * k)).abs() <= 1.5,
        "a channel's name at 104 + 9: {kick:?}, the column at {col_left}"
    );
    assert!(
        (snare.left() - (col_left + 127.0 * k)).abs() <= 1.5,
        "a pad's name stepped in 14: {snare:?}"
    );
    // A kit's own row says how many pads it has.
    assert!(
        column.iter().any(|(t, _)| t.ends_with(" pads")),
        "a kit's row counts its pads"
    );
    // The chevron folds the kit's pads under its row, and opens them again.
    let before = r.rows();
    let (kit, fold) = r.app.rack.kit_folds[0];
    assert!(
        (fold.left() - (col_left + 104.0 * k)).abs() <= 1.0,
        "{fold:?}"
    );
    r.click(fold.center());
    assert!(r.app.rack.folded.contains(&kit), "folded");
    let folded = r.rows();
    assert!(folded < before, "the pads leave: {before} → {folded}");
    let (_, fold) = r
        .app
        .rack
        .kit_folds
        .iter()
        .copied()
        .find(|(t, _)| *t == kit)
        .unwrap();
    r.click(fold.center());
    assert_eq!(r.rows(), before, "and come back");
    // The foot: scrolled to the end, "Add a channel" and the drop zone under the last row.
    r.app.rack.scroll_y = 10_000.0;
    let texts = r.frames(3, None);
    let (add, zone) = r.app.rack.foot.expect("the foot");
    let last = r.app.rack.geometry().unwrap().rows.last().copied().unwrap();
    assert!(add.top() >= last.top + last.height, "under the last row");
    assert!((add.width() - 150.0 * k).abs() < 0.5 && (add.height() - 24.0 * k).abs() < 0.5);
    assert!((zone.height() - 56.0 * k).abs() < 0.5, "{zone:?}");
    let words = seen(&texts, "Add a channel").expect("its words");
    assert!(add.contains_rect(words.shrink(0.5)), "{words:?} in {add:?}");
    let drop = seen(&texts, "Drop a sound here for a new channel").expect("the zone's words");
    assert!(zone.contains_rect(drop.shrink(0.5)), "{drop:?} in {zone:?}");
    assert!(seen(&texts, "or onto a channel to replace its sample").is_some());
    // A click on "Add a channel" opens the instrument picker for a new channel at the end.
    assert!(r.app.picker.is_none());
    r.click(add.center());
    assert_eq!(
        r.app.picker,
        Some(cantus_ui::picker::PickPurpose::AddInstrument { index: None }),
        "the picker, for a channel at the end"
    );
}

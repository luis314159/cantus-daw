//! Screenshots of cut, copy and paste (ADR 0036) without a screen: the Edit menu open with
//! Cut, Copy and Paste and their keys; the arrangement after a two-track block was copied
//! and pasted at bar 5 (both copies selected).
//!
//! `cargo run -p cantus-ui --example shoot_clipboard -- <out-dir> [es]`

use cantus_core::{AutoPoint, AutoTarget, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::keymap::Action;
use cantus_ui::raster::shoot;
use cantus_ui::{starter_project, CantusApp};
use std::path::{Path, PathBuf};

const W: f32 = 1400.0;
const H: f32 = 820.0;

/// Two instrument tracks with a clip each, and an automation clip under the second.
fn project() -> cantus_core::Project {
    let mut p = starter_project();
    let mut t = p.make_track("Bass");
    let mut c = p.make_clip("Bass line", Ticks::beats(4), Ticks::beats(12));
    for (i, pitch) in [36u8, 36, 43, 41, 36, 39].into_iter().enumerate() {
        let n = p.make_note(Ticks::beats(i as i64 * 2), Ticks::beats(1), pitch, 100);
        c.notes.push(n);
    }
    t.clips.push(c);
    let gain = t.id;
    p.tracks.push(t);
    let auto = p.make_automation_clip(
        "Bass gain",
        AutoTarget::TrackGain(gain),
        Ticks::ZERO,
        Ticks::beats(16),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.3),
            AutoPoint::new(Ticks::beats(16), 0.8),
        ],
    );
    p.tracks[1].clips.push(auto);
    p
}

fn app(ctx: &egui::Context) -> CantusApp {
    let mut a = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
    a.attach(ctx);
    a
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

fn window(ctx: &egui::Context, a: &mut CantusApp, path: &Path, before: impl Fn(&egui::Context)) {
    shoot(ctx, egui::vec2(W, H), 1.0, 12, Some(path), |root| {
        before(root.ctx());
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::ui(a, root, &mut frame);
    });
    println!("wrote {}", path.display());
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

    // The Edit menu, after Ctrl+C in the piano roll: the status line says what was copied.
    let ctx = context();
    let mut a = app(&ctx);
    a.focus_piano_roll();
    a.run_action(Action::Copy);
    // The menu word's id is what `views::transport` gives `ui.interact`; its popup is
    // `Popup::default_response_id` of that.
    let edit = egui::Id::new(("toolbar-menu", 1usize)).with("popup");
    window(
        &ctx,
        &mut a,
        &dir.join(format!("clipboard-edit-menu{suffix}.png")),
        |c| egui::Popup::open_id(c, edit),
    );

    // The arrangement after copying both tracks' clips and pasting at bar 5.
    let ctx = context();
    let mut a = app(&ctx);
    a.focus_arrangement();
    let sel: Vec<_> = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(|c| c.automation.is_none())
                .map(move |c| (t.id, c.id))
        })
        .collect();
    a.set_clip_selection(sel);
    a.run_action(Action::Copy);
    let first = a.project().tracks[0].id;
    a.select_track(first);
    a.seek(Ticks::beats(16));
    a.paste_now();
    window(
        &ctx,
        &mut a,
        &dir.join(format!("clipboard-pasted{suffix}.png")),
        |_| {},
    );
}

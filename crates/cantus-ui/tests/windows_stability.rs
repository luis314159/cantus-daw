//! Several screens hold still and leak nothing (ADR 0151; the owner, 2026-11-17: *"make
//! sure it looks excellent on different screens and that it stays stable and works well"*).
//! Through `CantusApp::assemble`, run as eframe runs windows (`windows_rig`):
//!
//! - 500 scripted cycles of opening a window, moving an area into it and out, sending it to
//!   the other screen and closing it: no panic, the textures egui holds and the viewports it
//!   keeps come back to where they were, every area in exactly one dock.
//! - Screens unplugged and plugged in again, at start and while the song plays, 50 times:
//!   the window comes and goes with its screen, the transport never stops.
//! - No window's scale flips between two values: over a long idle stretch each window's
//!   pixels per point is one value, and nothing asks for a frame.
//! - Layout files: corrupt, empty, from before several screens, with a window whose dock
//!   names nothing — each loads into a usable main window.

mod windows_rig;
use cantus_ui::views::workspace::Pane;
use cantus_ui::windows::Target;
use egui::ViewportId;
use std::time::Duration;
use windows_rig::*;

/// Every area of the workspace lives in at most one dock.
fn every_area_once(r: &mut Rig) {
    let app = r.app();
    let mut seen = Vec::new();
    for (_, ws) in app.docks() {
        for p in ws.panes() {
            assert!(!seen.contains(&p), "{p:?} in two docks");
            seen.push(p);
        }
    }
}

fn textures(r: &Rig) -> usize {
    r.ctx.tex_manager().read().num_allocated()
}

#[test]
fn five_hundred_windows_opened_moved_and_closed_leave_nothing_behind() {
    run(|| {
        let mut r = Rig::new(vec![laptop(), external()]);
        r.app().reveal_pane(Pane::Mixer);
        r.app().reveal_pane(Pane::Browser);
        r.idle(10);
        let base_textures = textures(&r);
        let ctx = r.ctx.clone();
        let areas = [Pane::Mixer, Pane::Browser, Pane::PianoRoll, Pane::Inspector];
        for n in 0..500 {
            let a = areas[n % areas.len()];
            let b = areas[(n + 1) % areas.len()];
            if !r.app().pane_shown(a) {
                r.app().reveal_pane(a);
            }
            if !r.app().pane_shown(b) {
                r.app().reveal_pane(b);
            }
            let screen = Some(n % 2);
            r.app().move_pane(&ctx, a, Target::NewWindow { screen });
            r.frame();
            let key = r.app().windows.list.last().map(|w| w.key);
            if let Some(k) = key {
                r.app().move_pane(&ctx, b, Target::Window(k));
                r.frame();
                r.app().move_pane(&ctx, b, Target::Main);
                r.frame();
            }
            every_area_once(&mut r);
            r.app().move_pane(&ctx, a, Target::Main);
            r.frame();
            assert!(
                r.windows().is_empty(),
                "cycle {n}: the window closed with its last area"
            );
        }
        r.idle(10);
        assert!(
            r.cbs.is_empty(),
            "no viewport kept for a window that is gone"
        );
        let now = textures(&r);
        assert!(
            now <= base_textures + 2,
            "textures grew from {base_textures} to {now} over 500 windows"
        );
    });
}

#[test]
fn screens_unplugged_and_plugged_while_playing_never_stop_the_song() {
    run(|| {
        let mut r = Rig::new(vec![laptop(), external()]);
        r.app().reveal_pane(Pane::Mixer);
        r.idle(4);
        let ctx = r.ctx.clone();
        r.app()
            .move_pane(&ctx, Pane::Mixer, Target::NewWindow { screen: Some(1) });
        r.idle(4);
        r.app().engine.play();
        r.idle(2);
        for n in 0..50 {
            r.set_screens(vec![laptop()]);
            r.idle(2);
            assert!(r.windows().is_empty(), "unplug {n}");
            assert!(r.app().workspace.contains(Pane::Mixer), "unplug {n}");
            r.set_screens(vec![laptop(), external()]);
            r.idle(2);
            assert_eq!(r.windows().len(), 1, "replug {n}");
            assert!(!r.app().workspace.contains(Pane::Mixer), "replug {n}");
            every_area_once(&mut r);
            assert!(r.app().engine.is_playing(), "the song plays on ({n})");
        }
        assert!(r.app().windows.parked.is_empty());
    });
}

#[test]
fn no_scale_flips_and_nothing_draws_at_rest() {
    run(|| {
        let mut r = Rig::new(vec![laptop(), external()]);
        r.app().reveal_pane(Pane::Mixer);
        r.idle(4);
        let ctx = r.ctx.clone();
        r.app()
            .move_pane(&ctx, Pane::Mixer, Target::NewWindow { screen: Some(1) });
        r.idle(30);
        let id = r.windows()[0];
        let (main, win) = (r.passes[&ViewportId::ROOT].ppp, r.passes[&id].ppp);
        r.clear_asked();
        for _ in 0..300 {
            r.frame();
            assert_eq!(
                r.passes[&ViewportId::ROOT].ppp,
                main,
                "the main window's scale moved"
            );
            assert_eq!(r.passes[&id].ppp, win, "the window's scale moved");
        }
        assert!(
            !r.was_asked(id, Duration::from_secs(1)),
            "the window asked for frames at rest"
        );
        let w = &r.app().windows.list[0];
        assert!(!w.watch.tripped(), "its watchdog saw a loop");
    });
}

#[test]
fn layout_files_corrupt_old_or_strange_load_into_a_usable_window() {
    run(|| {
        let files: [&str; 5] = [
            "{ this is not json",
            "",
            // Before several screens: no windows, no places.
            r#"{"current":{"name":"Mixing","inspector":320.0,"arrangement":200.0,
               "show_inspector":true,"show_arrangement":true,"central":"mixer"},"saved":[]}"#,
            // A window whose dock names nothing, another with a nonsense place.
            r#"{"current":{"name":"@default","inspector":280.0,"arrangement":0.0,
               "show_inspector":true,"show_arrangement":true,"central":"mixer",
               "windows":[{"key":1,"dock":{"nothing":true}},
                          {"key":2,"dock":null,"place":{"screen":{"size":[0,0],"pos":[0,0]},
                           "rect":{"x":0,"y":0,"w":-5,"h":-5}}}]},"saved":[]}"#,
            // A window for a screen that never existed.
            r#"{"current":{"name":"@default","inspector":280.0,"arrangement":0.0,
               "show_inspector":true,"show_arrangement":true,"central":"mixer",
               "screens":"X@1x1+0+0"},"saved":[],
               "by_screens":[{"screens":"nowhere","layout":{"name":"x","inspector":1.0,
               "arrangement":1.0,"show_inspector":true,"show_arrangement":true,"central":"mixer"}}]}"#,
        ];
        for text in files {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::write(tmp.path().join("layouts.json"), text).unwrap();
            let mut r = Rig::in_dir(
                tmp.path(),
                vec![laptop(), external()],
                Some(cantus_ui::welcome_project()),
            );
            r.idle(6);
            assert!(r.app().workspace.contains(Pane::Arrangement), "{text}");
            every_area_once(&mut r);
            drop(r);
        }
    });
}

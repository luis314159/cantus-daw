//! Several screens, end to end (ADR 0151): through `CantusApp::assemble`, run as eframe runs
//! windows (`windows_rig`), on the issue-#1 reporter's desk — his 2 880 × 1 800 laptop at
//! the system's 200 % holding the main window, a 1 920 × 1 080 screen at 100 % beside it.
//!
//! - A tab's menu sends the mixer to a window of its own: a deferred viewport whose pass
//!   draws the mixer, gone from the main window; the window lands on the other screen.
//! - Work in it is work on the project: a strip muted from it is one command, undone with
//!   Ctrl+Z typed in it; the space bar typed in it plays; the main window is asked to draw.
//! - Each window draws at its own screen's scale: the laptop's 0.9 × 2.0, the screen's
//!   1.25 × 1.0 — the follow rule for each screen, egui's zoom unmoved.
//! - At rest no window asks for a frame; while the song plays the mixer's window draws and a
//!   window holding only the browser does not.
//! - The layout keeps the window: a fresh start reopens it on its screen; with that screen
//!   unplugged its mixer is back in the main window (the status line says so) and comes back
//!   to its window when the screen does — at start and while running.

mod windows_rig;
use cantus_ui::views::workspace::Pane;
use egui::{Key, Modifiers, PointerButton, ViewportId};
use std::time::Duration;
use windows_rig::*;

/// Right-click the tab titled `title` in the main window and choose `row`.
fn tab_menu(r: &mut Rig, title: &str, row: &str) {
    let tabs = r.text_in(ViewportId::ROOT, title);
    assert!(!tabs.is_empty(), "the {title} tab is drawn");
    for tab in tabs {
        r.click(ViewportId::ROOT, tab.center(), PointerButton::Secondary);
        r.root(Vec::new());
        if let Some(at) = r.text_in(ViewportId::ROOT, row).first().copied() {
            r.click(ViewportId::ROOT, at.center(), PointerButton::Primary);
            r.idle(3);
            return;
        }
        r.key(ViewportId::ROOT, Key::Escape, Modifiers::NONE);
    }
    panic!("no tab titled {title} opened a menu with {row}");
}

/// The desk with the mixer moved to a window of its own.
fn with_mixer_window() -> Rig {
    with_mixer_window_in(Rig::new(vec![laptop(), external()]))
}

fn with_mixer_window_in(mut r: Rig) -> Rig {
    r.app().reveal_pane(Pane::Mixer);
    r.idle(6);
    tab_menu(&mut r, "Mixer", "Move to a new window");
    r
}

#[test]
fn the_mixer_moves_to_a_window_of_its_own_on_the_other_screen() {
    run(|| {
        let mut r = with_mixer_window();
        let wins = r.windows();
        assert_eq!(wins.len(), 1, "one window of its own");
        let app = r.app();
        assert!(app.windows.list[0].workspace.contains(Pane::Mixer));
        assert!(
            !app.workspace.contains(Pane::Mixer),
            "gone from the main window"
        );
        let place = app.windows.list[0].place.clone().expect("placed");
        assert_eq!(
            place.screen.size,
            [1920, 1080],
            "on the screen the main window is not on"
        );
        assert!(place.maximised);
        // Its pass draws the mixer: the master strip's name is painted there, not in the
        // main window.
        let id = wins[0];
        assert!(r.cbs.contains_key(&id), "a deferred viewport");
        assert!(
            !r.text_in(id, "Master").is_empty(),
            "the mixer is drawn in its window"
        );
    });
}

#[test]
fn each_window_draws_at_its_own_screens_scale() {
    run(|| {
        let mut r = with_mixer_window();
        r.idle(4);
        let id = r.windows()[0];
        let main = r.passes[&ViewportId::ROOT].ppp;
        let win = r.passes[&id].ppp;
        // The follow rule: 2 880 px at 200 % → 0.9; 1 920 px at 100 % → 1.25.
        assert!((main - 1.8).abs() < 1e-3, "the main window at {main}");
        assert!((win - 1.25).abs() < 1e-3, "the mixer's window at {win}");
        assert!(
            (r.ctx.zoom_factor() - 0.9).abs() < 1e-3,
            "egui's zoom unmoved"
        );
        // Its words sit inside its window.
        let size = r.passes[&id].size;
        for (w, rect, _) in r.texts(id) {
            assert!(
                rect.max.x <= size.x + 0.5 && rect.max.y <= size.y + 0.5,
                "{w:?} at {rect:?} outside the window's {size:?}"
            );
        }
    });
}

#[test]
fn keys_typed_in_a_window_reach_the_project_and_the_transport() {
    run(|| {
        let mut r = with_mixer_window();
        let id = r.windows()[0];
        // The space bar typed in the mixer's window plays.
        r.key(id, Key::Space, Modifiers::NONE);
        r.idle(2);
        assert!(
            r.app().engine.is_playing(),
            "the space bar in the window plays"
        );
        r.key(id, Key::Space, Modifiers::NONE);
        r.idle(2);
        assert!(!r.app().engine.is_playing(), "and stops");
        // A strip muted from its window is a command; Ctrl+Z typed there undoes it.
        let strip = {
            let app = r.app();
            app.project()
                .strips
                .iter()
                .find(|s| !s.id.is_master())
                .map(|s| s.id)
        }
        .expect("a strip");
        let before = r.app().bus.revision();
        r.app().select_strip(strip);
        r.app().focus = cantus_ui::app::EditFocus::Mixer;
        r.idle(2);
        r.clear_asked();
        r.key(id, Key::M, Modifiers::NONE);
        let muted = r.app().project().strip(strip).map(|s| s.muted);
        assert_eq!(
            muted,
            Some(true),
            "M typed in the mixer's window mutes the strip"
        );
        assert!(r.app().bus.revision() > before, "one command");
        assert!(
            r.ctx.has_requested_repaint_for(&ViewportId::ROOT),
            "the main window is asked to draw the change"
        );
        r.key(id, Key::Z, Modifiers::COMMAND);
        let muted = r.app().project().strip(strip).map(|s| s.muted);
        assert_eq!(muted, Some(false), "Ctrl+Z typed in the window undoes it");
    });
}

#[test]
fn at_rest_no_window_draws_and_playing_draws_only_what_moves() {
    run(|| {
        let mut r = with_mixer_window();
        // A second window with only the browser.
        r.app().reveal_pane(Pane::Browser);
        r.idle(2);
        let ctx = r.ctx.clone();
        r.app().move_pane(
            &ctx,
            Pane::Browser,
            cantus_ui::windows::Target::NewWindow { screen: Some(0) },
        );
        r.idle(30);
        let wins = r.windows();
        assert_eq!(wins.len(), 2);
        r.clear_asked();
        r.idle(20);
        for id in &wins {
            assert!(
                !r.was_asked(*id, Duration::from_secs(1)),
                "window {id:?} asks for a frame at rest"
            );
        }
        // Playing: the mixer's window draws, the browser's does not.
        r.app().engine.play();
        r.idle(3);
        r.clear_asked();
        r.root(Vec::new());
        let mixer = wins
            .iter()
            .copied()
            .find(|id| {
                let app = r.app.as_ref().unwrap();
                let i = app.windows.index_of(*id).unwrap();
                app.windows.list[i].workspace.contains(Pane::Mixer)
            })
            .unwrap();
        let browser = wins.iter().copied().find(|id| *id != mixer).unwrap();
        assert!(
            r.was_asked(mixer, Duration::from_millis(100)),
            "the mixer's meters move"
        );
        assert!(
            !r.was_asked(browser, Duration::from_secs(1)),
            "the browser does not"
        );
    });
}

#[test]
fn the_window_is_kept_and_follows_its_screen_in_and_out() {
    run(|| {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let fresh = Rig::in_dir(
            &dir,
            vec![laptop(), external()],
            Some(cantus_ui::welcome_project()),
        );
        let mut r = with_mixer_window_in(fresh);
        let ctx = r.ctx.clone();
        r.app().store_layouts(&ctx);
        drop(r);
        // A fresh start on the same desk: the window is back on its screen.
        let mut r = Rig::in_dir(
            &dir,
            vec![laptop(), external()],
            Some(cantus_ui::welcome_project()),
        );
        r.idle(6);
        assert_eq!(r.windows().len(), 1, "reopened");
        assert!(r.app().windows.list[0].workspace.contains(Pane::Mixer));
        assert!(!r.app().workspace.contains(Pane::Mixer));
        drop(r);
        // A start with the screen unplugged: the mixer in the main window, said so.
        let mut r = Rig::in_dir(&dir, vec![laptop()], Some(cantus_ui::welcome_project()));
        r.idle(6);
        assert!(
            r.windows().is_empty(),
            "no window on a screen that is not there"
        );
        assert!(
            r.app().workspace.contains(Pane::Mixer),
            "the mixer came back"
        );
        let status = r.app().status.text.clone();
        assert!(
            status.contains("DISPLAY2"),
            "the status line names the screen: {status}"
        );
        // The screen plugged in while running: the window returns.
        r.set_screens(vec![laptop(), external()]);
        r.idle(4);
        assert_eq!(r.windows().len(), 1, "the window returns with its screen");
        assert!(r.app().windows.list[0].workspace.contains(Pane::Mixer));
        assert!(!r.app().workspace.contains(Pane::Mixer));
        // And unplugged while playing: back into the main window, the song plays on.
        r.app().engine.play();
        r.idle(2);
        r.set_screens(vec![laptop()]);
        r.idle(4);
        assert!(r.windows().is_empty());
        assert!(r.app().workspace.contains(Pane::Mixer));
        assert!(r.app().engine.is_playing(), "the transport never stopped");
    });
}

#[test]
fn f9_hides_the_mixer_in_its_window_and_brings_it_back_there() {
    run(|| {
        let mut r = with_mixer_window();
        r.key(ViewportId::ROOT, Key::F9, Modifiers::NONE);
        r.idle(2);
        assert!(
            r.windows().is_empty(),
            "its window closed with its only area"
        );
        assert!(!r.app().pane_shown(Pane::Mixer));
        r.key(ViewportId::ROOT, Key::F9, Modifiers::NONE);
        r.idle(2);
        assert_eq!(r.windows().len(), 1, "F9 brings it back to its window");
        assert!(!r.app().workspace.contains(Pane::Mixer));
    });
}

#[test]
fn ctrl_plus_in_a_window_scales_that_window_alone() {
    run(|| {
        let mut r = with_mixer_window();
        r.idle(4);
        let id = r.windows()[0];
        let main = r.passes[&ViewportId::ROOT].ppp;
        let before = r.passes[&id].ppp;
        r.key(id, Key::Equals, Modifiers::COMMAND);
        r.idle(3);
        let after = r.passes[&id].ppp;
        assert!(after > before + 0.05, "the window grew: {before} → {after}");
        assert_eq!(
            r.passes[&ViewportId::ROOT].ppp,
            main,
            "the main window did not"
        );
        r.key(id, Key::Num0, Modifiers::COMMAND);
        r.idle(3);
        assert_eq!(
            r.passes[&id].ppp, before,
            "Ctrl+0 gives it back its screen's scale"
        );
    });
}

#[test]
fn a_tab_dragged_out_of_the_window_becomes_a_window_where_it_fell() {
    run(|| {
        let mut r = Rig::new(vec![laptop(), external()]);
        r.app().reveal_pane(Pane::Mixer);
        r.idle(8);
        let size = r.passes[&ViewportId::ROOT].size;
        // The mixer's tab (the one in a tab bar: the lowest "Mixer" in the lower zone's bar).
        let tab = r
            .text_in(ViewportId::ROOT, "Mixer")
            .into_iter()
            .filter(|t| t.min.x < size.x * 0.7)
            .max_by(|a, b| a.min.y.total_cmp(&b.min.y))
            .expect("the mixer's tab");
        // Let go to the right of the laptop's window: over the external screen.
        let to = egui::Pos2::new(size.x + 300.0, tab.center().y);
        r.drag(ViewportId::ROOT, tab.center(), to);
        r.idle(3);
        assert_eq!(r.windows().len(), 1, "a window of its own");
        let app = r.app();
        assert!(app.windows.list[0].workspace.contains(Pane::Mixer));
        assert!(!app.pane_shown(Pane::Mixer) || !app.workspace.contains(Pane::Mixer));
        let place = app.windows.list[0]
            .place
            .clone()
            .expect("placed where it fell");
        assert_eq!(
            place.screen.size,
            [1920, 1080],
            "on the screen it was let go over"
        );
    });
}

/// A stand-in for a window's native handle (no OS window in the rig).
fn fake_native(n: isize) -> raw_window_handle::RawWindowHandle {
    raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
        std::num::NonZeroIsize::new(n).unwrap(),
    ))
}

/// ADR 0151 §10: a plug-in editor opened from a window of its own belongs to that window
/// and opens on its screen; before that window goes — closed, docked back, its screen
/// unplugged, a layout applied — its editors are given to the main window first (Windows
/// destroys an owned window with its owner). The re-owning itself is held by
/// cantus-host's `owner_tests` on real Win32 windows.
#[test]
fn a_windows_plug_in_editors_are_its_own_and_outlive_it() {
    run(|| {
        let mut r = with_mixer_window();
        let id = r.windows()[0];
        r.app().windows.list[0].native = Some(fake_native(0x5150));
        // Working in the mixer's window: an editor opened now is owned by it, placed a
        // little in from its corner on its screen.
        r.click(id, egui::Pos2::new(400.0, 400.0), PointerButton::Primary);
        assert_eq!(r.app().windows.host, id);
        let owner = r.app().editor_owner();
        assert_eq!(owner, Some(fake_native(0x5150)));
        let at = cantus_host::window::next_place().expect("a place on its screen");
        assert!(
            at.0 >= 2880 && at.0 < 2880 + 1920,
            "on the external screen: {at:?}"
        );
        cantus_host::window::place_next(None);
        // From the main window: the main window's.
        r.click(
            ViewportId::ROOT,
            egui::Pos2::new(400.0, 300.0),
            PointerButton::Primary,
        );
        let main = r.app().plugins.owner_window;
        assert_eq!(r.app().editor_owner(), main);
        // The window docks back: its editors were given away before it went.
        let ctx = r.ctx.clone();
        r.app()
            .move_pane(&ctx, Pane::Mixer, cantus_ui::windows::Target::Main);
        r.idle(2);
        assert!(r.windows().is_empty());
        assert_eq!(r.app().windows.released, vec![0x5150]);
        // Its screen unplugged, and a layout applied: the same, every time.
        r.app().move_pane(
            &ctx,
            Pane::Mixer,
            cantus_ui::windows::Target::NewWindow { screen: Some(1) },
        );
        r.idle(2);
        r.app().windows.list[0].native = Some(fake_native(0x6160));
        r.set_screens(vec![laptop()]);
        r.idle(2);
        assert_eq!(r.app().windows.released.last(), Some(&0x6160));
        r.set_screens(vec![laptop(), external()]);
        r.idle(2);
        r.app().windows.list[0].native = Some(fake_native(0x7170));
        r.app().reset_layout();
        r.idle(2);
        assert_eq!(r.app().windows.released.last(), Some(&0x7170));
    });
}

/// ADR 0151 §4: *Choose what the bar shows…* and *Rename the window…* in a window's menu,
/// kept with the layout (user settings: no undo, the project is untouched).
#[test]
fn a_windows_bar_and_name_are_chosen_from_its_menu_and_kept() {
    run(|| {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let fresh = Rig::in_dir(
            &dir,
            vec![laptop(), external()],
            Some(cantus_ui::welcome_project()),
        );
        let mut r = with_mixer_window_in(fresh);
        let id = r.windows()[0];
        let revision = r.app().bus.revision();
        let open_menu = |r: &mut Rig| {
            r.click(id, egui::Pos2::new(24.0, 22.0), PointerButton::Primary);
            r.child(id, Vec::new());
        };
        // The bar's groups: the metronome added.
        open_menu(&mut r);
        assert!(r.app().windows.menu.is_some(), "the window's menu opened");
        let row = r.text_in(id, "Choose what the bar shows…")[0];
        {
            let p = row.center();
            let ev = |pressed| egui::Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            r.pointers.insert(id, p);
            r.pass_in(id, vec![]);
            r.pass_in(id, vec![ev(true)]);
            r.pass_in(id, vec![ev(false)]);
        }
        assert!(
            r.app().windows.groups_menu.is_some(),
            "the groups' list opened (menu {:?}, row {row:?})",
            r.app().windows.menu
        );
        r.idle(2);
        let metronome = r.text_in(id, "Metronome");
        let texts: Vec<String> = r.texts(id).into_iter().map(|t| t.0).collect();
        assert_eq!(
            metronome.len(),
            1,
            "the groups' list drawn in the window: {:?}; painted {texts:?}",
            r.app().windows.groups_menu
        );
        r.click(id, metronome[0].center(), PointerButton::Primary);
        r.child(id, Vec::new());
        use cantus_ui::views::transport::layout::Group;
        assert!(r.app().windows.list[0].groups.contains(&Group::Metronome));
        r.key(id, Key::Escape, Modifiers::NONE);
        // Its name: typed in the kit's dialog, Enter keeps it.
        open_menu(&mut r);
        let row = r.text_in(id, "Rename the window…")[0];
        r.click(id, row.center(), PointerButton::Primary);
        r.idle(2);
        r.pass_in(
            id,
            vec![egui::Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        r.pass_in(id, vec![egui::Event::Text("Drums".into())]);
        r.key(id, Key::Enter, Modifiers::NONE);
        r.idle(2);
        assert_eq!(r.app().window_name(0), "Drums");
        assert_eq!(
            r.app().bus.revision(),
            revision,
            "not the project: nothing to undo"
        );
        // Kept with the layout.
        let ctx = r.ctx.clone();
        r.app().store_layouts(&ctx);
        drop(r);
        let mut r = Rig::in_dir(
            &dir,
            vec![laptop(), external()],
            Some(cantus_ui::welcome_project()),
        );
        r.idle(6);
        assert_eq!(r.windows().len(), 1);
        assert_eq!(r.app().window_name(0), "Drums");
        assert!(r.app().windows.list[0].groups.contains(&Group::Metronome));
    });
}

//! Bringing a hidden dancer back, end to end (round 40; the owner, 2026-11-18: "once the
//! dancer is hidden, I don't know how to bring it back in an easy way"), in the app
//! `CantusApp::assemble` puts together, drawn by its own frame:
//!
//! - a dancer added the way the instrument picker adds one floats over the workspace, and
//!   *View ▸ Stage* is ticked;
//! - its pill's *Hide* (clicked) hides it — the tick goes, and a card in the corner says how
//!   to bring it back with the key the keymap gives the stage, and *Show again* (clicked)
//!   brings it back, the hide undone;
//! - hidden again, the stage's key (pressed) brings it back where it was, in one undo step:
//!   one undo hides it again;
//! - the track's menu offers *Show the dancer* while it is hidden, and its row (clicked)
//!   shows it; the editor's *Show* and the Stage tab's *Show the dancers* do the same.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::keymap::Action;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::stage::chrome::{self, Button};
use cantus_ui::stage::{show, Surface, ViewMode};
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;
use egui::{Event, Pos2, Rect, Vec2};
use std::path::Path;

const SCREEN: Vec2 = Vec2::new(1600.0, 900.0);

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        // A configured copy: no first-run audio setup or start page over the window.
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
}

impl Rig {
    fn new() -> Rig {
        Rig::in_language(cantus_ui::i18n::Locale::En)
    }

    fn in_language(loc: cantus_ui::i18n::Locale) -> Rig {
        let files = Files::read(data(), Some(loc));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, _proc) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
        let mut r = Rig { app, ctx, t: 0.0 };
        r.frames(3);
        r
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.t += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| app.frame(ui))
            .textures_delta
            .clear();
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    /// A click at `p`: the pointer there, pressed, released.
    fn click(&mut self, p: Pos2) {
        let button = |pressed| Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(p)]);
        self.frame(vec![Event::PointerMoved(p)]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.frames(2);
    }

    /// The stage's key as the keymap binds it, pressed.
    fn press_stage_key(&mut self) {
        let b = *self
            .app
            .keymap
            .bindings_for(Action::ToggleStage)
            .first()
            .expect("the stage has a key");
        let modifiers = egui::Modifiers {
            alt: b.alt,
            shift: b.shift,
            command: b.command,
            ctrl: b.command && !cfg!(target_os = "macos"),
            mac_cmd: b.command && cfg!(target_os = "macos"),
        };
        // As a window reports it: the modifiers held, then the key.
        self.frame(vec![
            Event::ModifiersChanged(modifiers),
            Event::Key {
                key: b.key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ]);
        self.frame(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
        self.frames(2);
    }

    fn mode(&self, track: TrackId) -> ViewMode {
        show::mode_of(&self.app, track).expect("a dancer")
    }

    /// *View ▸ Stage*'s tick, as the menu draws it.
    fn ticked(&self) -> bool {
        cantus_ui::views::transport::view_menu_ticks(&self.app)
            .into_iter()
            .find(|(k, _)| *k == "stage.view.stage")
            .expect("the menu has the stage's row")
            .1
    }

    /// A dancer added as the instrument picker adds one, floating at the middle.
    fn add_dancer(&mut self) -> TrackId {
        self.app.apply_pick(
            PickPurpose::AddInstrument { index: None },
            PickChoice::Instrument(InstrumentKind::Dancer),
        );
        let track = self
            .app
            .project()
            .tracks
            .iter()
            .rev()
            .find(|t| {
                t.instrument
                    .native()
                    .is_some_and(|n| n.kind == InstrumentKind::Dancer)
            })
            .expect("the dancer")
            .id;
        // The picker opened its window; closed, as a user does to see the stage.
        self.app.close_inst_editor();
        self.app.set_inst_value(track, "x", 0.5, false);
        self.app.set_inst_value(track, "y", 0.8, false);
        self.app.sync_engine();
        self.frames(4);
        track
    }

    /// Its pill's *Hide*, clicked as the user does: the pointer on the dancer, then on the
    /// pill's last button.
    fn hide_with_its_button(&mut self, track: TrackId) {
        let body = self
            .app
            .stage
            .body(track, Surface::Float)
            .expect("the dancer floats");
        self.frame(vec![Event::PointerMoved(body.center())]);
        self.frame(vec![Event::PointerMoved(body.center())]);
        let name = self.app.project().track(track).unwrap().name.clone();
        let s = cantus_ui::kit::Style::current();
        let pill = chrome::layout(
            &self.ctx,
            body,
            &name,
            &[Button::Pin, Button::Dock, Button::Hide],
            &s,
            Rect::from_min_size(Pos2::ZERO, SCREEN),
        );
        assert_eq!(
            self.app.stage.hovered(self.t).map(|(t, _)| t),
            Some(track),
            "the pointer on the dancer shows its pill"
        );
        let hide = pill.buttons[2].1.center();
        self.click(hide);
    }
}

#[test]
fn a_hidden_dancer_comes_back_from_the_notice_the_key_and_one_undo_hides_it_again() {
    let mut r = Rig::new();
    let track = r.add_dancer();
    assert_eq!(r.mode(track), ViewMode::Floating);
    assert!(show::on_screen(&r.app) && r.ticked(), "on screen: ticked");

    // Its own Hide: hidden, the tick gone, the card up with the key the keymap gives.
    r.hide_with_its_button(track);
    assert_eq!(r.mode(track), ViewMode::Hidden, "its Hide hid it");
    assert!(!r.ticked(), "nothing on screen: not ticked");
    let key = r.app.keymap.display_for(Action::ToggleStage);
    let now = std::time::Instant::now();
    let n = show::notice(&r.app, now)
        .expect("the card after a Hide")
        .clone();
    let (title, body) = show::notice_words(&r.app, &n);
    assert!(title.contains(&r.app.project().track(track).unwrap().name));
    assert!(body.contains(&key), "{body:?} names {key}");
    assert!(body.contains("View"), "{body:?}");

    // Its Show again (clicked in the corner stack): back, the hide undone.
    let cards = cantus_ui::views::audio_notice::card_layout(&r.app, &r.ctx);
    // The bottom card: no plug-in, agent or freeze card waits here.
    let (_, buttons) = cards.first().expect("the card is drawn").clone();
    r.click(buttons[0].center());
    assert_eq!(
        r.mode(track),
        ViewMode::Floating,
        "Show again brought it back"
    );
    assert!(r.ticked());
    assert!(show::notice(&r.app, std::time::Instant::now()).is_none());
    assert_ne!(
        r.app.bus.undo_label_key(),
        Some(show::HIDE_ONE),
        "the hide was undone, not stacked on"
    );

    // Hidden again, the stage's key brings it back, in one step.
    r.hide_with_its_button(track);
    assert_eq!(r.mode(track), ViewMode::Hidden);
    assert!(!r.ticked());
    r.press_stage_key();
    assert_eq!(r.mode(track), ViewMode::Floating, "the key brought it back");
    assert!(r.ticked(), "on screen again: ticked");
    assert!(r.app.stage.body(track, Surface::Float).is_some(), "drawn");
    assert_eq!(r.app.bus.undo_label_key(), Some(show::SHOW_ALL));
    r.app.undo();
    r.frames(2);
    assert_eq!(r.mode(track), ViewMode::Hidden, "one undo hides it again");
    assert!(!r.ticked());

    // The key again: back; and once more, every dancer off (the switch), the tick gone.
    r.press_stage_key();
    assert_eq!(r.mode(track), ViewMode::Floating);
    r.press_stage_key();
    assert!(
        r.app.settings.stage.hidden && !r.ticked(),
        "the key hides them"
    );
    assert_eq!(r.mode(track), ViewMode::Floating, "its own mode untouched");
    r.app.run_action(Action::ToggleStage);
    assert!(!r.app.settings.stage.hidden && r.ticked(), "and shows them");
}

#[test]
fn a_docked_dancer_goes_back_to_the_stage_tab() {
    let mut r = Rig::new();
    let track = r.add_dancer();
    r.app
        .set_inst_value(track, "view.mode", ViewMode::Docked.value(), false);
    r.app.reveal_pane(Pane::Stage);
    r.frames(3);
    assert!(r.ticked(), "docked on an open tab: ticked");
    let body = r.app.stage.body(track, Surface::Dock).expect("on the tab");
    r.frame(vec![Event::PointerMoved(body.center())]);
    r.frame(vec![Event::PointerMoved(body.center())]);
    // Hidden by the stage's own action (the docked pill's Hide).
    cantus_ui::stage::float::apply(
        &mut r.app,
        &[cantus_ui::stage::float::Action::Mode {
            track,
            mode: ViewMode::Hidden,
        }],
    );
    r.frames(2);
    assert_eq!(r.mode(track), ViewMode::Hidden);
    assert!(!r.ticked());
    // The Stage tab, empty now, offers to show them; its button (clicked) does.
    let b = r.app.stage.empty_show.expect("the empty tab's button");
    r.click(b.center());
    assert_eq!(r.mode(track), ViewMode::Docked, "back where it was: docked");
    assert!(r.app.pane_shown(Pane::Stage));
    assert!(r.ticked());
}

#[test]
fn the_tracks_menu_and_the_editor_show_a_hidden_dancer() {
    let mut r = Rig::new();
    let track = r.add_dancer();
    r.hide_with_its_button(track);
    assert_eq!(r.mode(track), ViewMode::Hidden);
    // The track's menu, as a right-click opens it: a row of its own, "Show the dancer".
    let s = cantus_ui::kit::Style::current();
    let row = [
        cantus_ui::kit::menu::Item::Separator,
        cantus_ui::kit::menu::Item::action(cantus_ui::kit::text::Src::Key(
            "stage.menu.show_dancer",
        )),
    ];
    let row_h = cantus_ui::kit::menu::height_in(&row, &s);
    let menu = |r: &mut Rig, events: Vec<Event>| -> Vec<Rect> {
        cantus_ui::kit::widgets::record_lists(true);
        r.t += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(r.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut r.app;
        let mut gone = None;
        r.ctx
            .run_ui(input, |ui| {
                egui::Area::new(egui::Id::new("test-track-menu"))
                    .fixed_pos(Pos2::new(100.0, 100.0))
                    .show(ui.ctx(), |ui| {
                        cantus_ui::views::arrangement::menus::track(app, ui, track, &mut gone)
                    });
            })
            .textures_delta
            .clear();
        let lists = cantus_ui::kit::widgets::take_lists();
        cantus_ui::kit::widgets::record_lists(false);
        lists.into_iter().map(|(rect, _)| rect).collect()
    };
    let lists = menu(&mut r, Vec::new());
    let ours = lists
        .iter()
        .find(|l| (l.height() - row_h).abs() < 0.5)
        .copied()
        .expect("the menu offers Show the dancer while it is hidden");
    let at = cantus_ui::kit::menu::row_in(ours, &row, &s, 1).center();
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    menu(&mut r, vec![Event::PointerMoved(at)]);
    menu(&mut r, vec![Event::PointerMoved(at)]);
    menu(&mut r, vec![press(true)]);
    menu(&mut r, vec![press(false)]);
    assert_eq!(
        r.mode(track),
        ViewMode::Floating,
        "the menu's row showed it"
    );
    r.frames(2);
    let lists = menu(&mut r, Vec::new());
    assert!(
        !lists.iter().any(|l| (l.height() - row_h).abs() < 0.5),
        "on screen, the menu has no such row"
    );
    // The editor's banner: Show.
    r.hide_with_its_button(track);
    assert_eq!(r.mode(track), ViewMode::Hidden);
    r.app.open_inst_editor(track);
    let editor = |r: &mut Rig, events: Vec<Event>| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..Default::default()
        };
        let app = &mut r.app;
        r.ctx
            .run_ui(input, |ui| cantus_ui::views::inst::window_in(app, ui))
            .textures_delta
            .clear();
    };
    editor(&mut r, Vec::new());
    editor(&mut r, Vec::new());
    let b = *r
        .app
        .inst
        .dancer
        .placed
        .get("show_hidden")
        .expect("the editor says it is hidden, with Show");
    let press = |pressed| Event::PointerButton {
        pos: b.center(),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    editor(&mut r, vec![Event::PointerMoved(b.center())]);
    editor(&mut r, vec![press(true)]);
    editor(&mut r, vec![press(false)]);
    assert_eq!(
        r.mode(track),
        ViewMode::Floating,
        "the editor's Show showed it"
    );
}

#[test]
fn the_palette_finds_it_as_show_the_dancers_in_both_languages() {
    for (loc, words) in [
        (cantus_ui::i18n::Locale::En, "show the dancers"),
        (cantus_ui::i18n::Locale::Es, "mostrar los bailarines"),
    ] {
        let mut r = Rig::in_language(loc);
        cantus_ui::i18n::set_thread_locale(loc);
        let entries = cantus_ui::views::palette::entries(&mut r.app);
        let ranked = cantus_ui::palette_index::rank(&entries, words, &Default::default());
        let first = &entries[*ranked.first().expect("found")];
        assert_eq!(
            first.target,
            cantus_ui::palette_index::Target::Action(Action::ToggleStage),
            "{loc:?}: {words:?} finds the stage first"
        );
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

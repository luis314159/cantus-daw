//! Round 26, third review: every text field of the app is the kit's (`kit::field`), with
//! one set of rules — select-all on open, Enter commits, Esc cancels and closes, a click
//! elsewhere commits, closes and lets the keyboard go, and nothing else reads the keys
//! while a field holds them. Driven through the whole window, gesture by gesture, for
//! every rename and the layout's name.

use cantus_core::{InstrumentSpec, Project, PulsusParams, Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::{starter_project, CantusApp};
use egui::{pos2, vec2, Event, Key, Modifiers, Pos2};

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new(project: Project) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        let dir = tempfile::tempdir().unwrap();
        let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
        app.settings_path = dir.path().join("settings.json");
        app.layouts_path = dir.path().join("layouts.json");
        Rig {
            ctx,
            app,
            size: vec2(1600.0, 900.0),
            frame: 0,
            pointer: None,
            _dir: dir,
        }
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame += 1;
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.idle(2);
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.into())]);
        self.idle(1);
    }

    /// A click on the status bar: somewhere that is no field and does nothing else.
    fn click_elsewhere(&mut self) {
        let p = pos2(300.0, self.size.y - 8.0);
        self.pointer = Some(p);
        self.frame(Vec::new());
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.idle(2);
    }

    fn keyboard_free(&self) -> bool {
        !self.ctx.egui_wants_keyboard_input() && !cantus_ui::kit::field::active(&self.ctx)
    }
}

/// One rename in the window: how to open it, whether it is open, and the name it edits.
struct Editor {
    what: &'static str,
    make: fn() -> Rig,
    open: fn(&mut Rig),
    is_open: fn(&Rig) -> bool,
    name: fn(&Rig) -> String,
}

fn track(r: &Rig) -> TrackId {
    r.app.project().tracks[0].id
}

fn editors() -> Vec<Editor> {
    vec![
        Editor {
            what: "track",
            make: || Rig::new(starter_project()),
            open: |r| {
                let t = track(r);
                let name = r.app.project().tracks[0].name.clone();
                r.app.arrangement.begin_rename_track(t, name);
            },
            is_open: |r| r.app.arrangement.renaming_track().is_some(),
            name: |r| r.app.project().tracks[0].name.clone(),
        },
        Editor {
            what: "clip",
            make: || Rig::new(starter_project()),
            open: |r| {
                let t = &r.app.project().tracks[0];
                let (t, c) = (t.id, t.clips[0].id);
                r.app.select_clip(t, c);
                r.app.begin_rename_clip();
            },
            is_open: |r| r.app.arrangement.renaming_clip(),
            name: |r| r.app.project().tracks[0].clips[0].name.clone(),
        },
        Editor {
            what: "clip in the inspector",
            make: || {
                let mut r = Rig::new(starter_project());
                let t = &r.app.project().tracks[0];
                let (t, c) = (t.id, t.clips[0].id);
                r.app.select_clip(t, c);
                r
            },
            open: |r| {
                let at = r
                    .ctx
                    .read_response(egui::Id::new("insp-clip-name"))
                    .expect("the inspector's name row")
                    .rect
                    .center();
                r.pointer = Some(at);
                r.frame(Vec::new());
                for pressed in [true, false] {
                    r.frame(vec![Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }]);
                }
                r.pointer = None;
            },
            is_open: |r| {
                let edit = cantus_ui::kit::field::edit_id(egui::Id::new("insp-clip-name"));
                r.ctx.memory(|m| m.has_focus(edit))
            },
            name: |r| r.app.project().tracks[0].clips[0].name.clone(),
        },
        Editor {
            what: "pad",
            make: || {
                let mut p = Project::new("kit");
                let mut t = p.make_track("Drums");
                t.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
                let clip = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(8));
                t.clips.push(clip);
                p.tracks.push(t);
                let mut r = Rig::new(p);
                let t = &r.app.project().tracks[0];
                let (t, c) = (t.id, t.clips[0].id);
                r.app.select_clip(t, c);
                r.app.show_central(cantus_ui::app::CentralView::ChannelRack);
                r
            },
            open: |r| {
                let t = track(r);
                r.app.rack.begin_rename_pad(t, 0, "Kick".into());
            },
            is_open: |r| r.app.rack.renaming_pad(),
            name: |r| {
                r.app.project().tracks[0]
                    .instrument
                    .lane_name(36)
                    .unwrap_or_default()
                    .to_string()
            },
        },
        Editor {
            what: "strip",
            make: || {
                let mut r = Rig::new(starter_project());
                r.app.central = cantus_ui::app::CentralView::Mixer;
                r
            },
            open: |r| {
                let st = r.app.project().strips[0].clone();
                r.app.mixer.begin_rename(&st);
            },
            is_open: |r| r.app.mixer.renaming(),
            name: |r| r.app.project().strips[0].name.clone(),
        },
    ]
}

fn opened(e: &Editor) -> Rig {
    let mut r = (e.make)();
    r.idle(3);
    (e.open)(&mut r);
    r.idle(3);
    assert!((e.is_open)(&r), "{}: the editor opened", e.what);
    assert!(
        !r.keyboard_free(),
        "{}: the field holds the keyboard",
        e.what
    );
    r
}

/// Opening selects the name, so what is typed replaces it; Enter commits and closes.
#[test]
fn enter_commits_what_was_typed_over_the_selected_name() {
    for e in editors() {
        let mut r = opened(&e);
        r.type_text("Zed");
        r.key(Key::Enter, Modifiers::NONE);
        assert_eq!(
            (e.name)(&r),
            "Zed",
            "{}: typed over the selected name",
            e.what
        );
        assert!(!(e.is_open)(&r), "{}: Enter closed it", e.what);
        assert!(r.keyboard_free(), "{}: the keyboard let go", e.what);
    }
}

/// Esc cancels and closes, and is the field's alone: the selection the app's Esc would
/// clear stays.
#[test]
fn esc_cancels_closes_and_reaches_nothing_else() {
    for e in editors() {
        let mut r = opened(&e);
        let old = (e.name)(&r);
        let selected = (r.app.selected_notes.len(), r.app.selected_clips.len());
        r.type_text("Q");
        r.key(Key::Escape, Modifiers::NONE);
        assert_eq!((e.name)(&r), old, "{}: the name as it was", e.what);
        assert!(!(e.is_open)(&r), "{}: Esc closed it", e.what);
        assert!(r.keyboard_free(), "{}: the keyboard let go", e.what);
        assert_eq!(
            (r.app.selected_notes.len(), r.app.selected_clips.len()),
            selected,
            "{}: the closing Esc reached the app",
            e.what
        );
    }
}

/// A click elsewhere commits, closes and lets the keyboard go.
#[test]
fn a_click_elsewhere_commits_and_closes() {
    for e in editors() {
        let mut r = opened(&e);
        r.type_text("Wren");
        r.click_elsewhere();
        assert_eq!((e.name)(&r), "Wren", "{}: committed", e.what);
        assert!(!(e.is_open)(&r), "{}: closed", e.what);
        assert!(r.keyboard_free(), "{}: the keyboard let go", e.what);
    }
}

/// While a field holds the keyboard, the app's shortcuts wait: Ctrl+Z undoes nothing.
#[test]
fn shortcuts_wait_while_a_field_holds_the_keyboard() {
    for e in editors() {
        let mut r = opened(&e);
        let rev = r.app.bus.revision();
        r.key(Key::Z, Modifiers::COMMAND);
        r.key(Key::M, Modifiers::NONE);
        assert_eq!(r.app.bus.revision(), rev, "{}: a shortcut acted", e.what);
        assert!((e.is_open)(&r), "{}: still open", e.what);
    }
}

/// The layout's name: the dialog's field opens with the name selected; Enter saves, Esc
/// leaves the dialog, a click elsewhere keeps the words and lets the keyboard go.
#[test]
fn the_layout_name_dialog_follows_the_rules() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    r.app.naming_layout = Some("Mix A".into());
    r.idle(3);
    assert!(!r.keyboard_free(), "the field took the keyboard");
    r.type_text("Wide desk");
    assert_eq!(
        r.app.naming_layout.as_deref(),
        Some("Wide desk"),
        "typed over"
    );
    r.key(Key::Enter, Modifiers::NONE);
    assert!(r.app.naming_layout.is_none(), "Enter saved and closed");
    assert_eq!(r.app.layouts.current.name, "Wide desk");
    r.app.naming_layout = Some("Mix B".into());
    r.idle(3);
    r.key(Key::Escape, Modifiers::NONE);
    assert!(r.app.naming_layout.is_none(), "Esc left the dialog");
    assert!(r.keyboard_free());
    r.app.naming_layout = Some("Mix C".into());
    r.idle(3);
    r.type_text("Mix D");
    r.click_elsewhere();
    assert_eq!(
        r.app.naming_layout.as_deref(),
        Some("Mix D"),
        "kept the words"
    );
    assert!(r.keyboard_free(), "the keyboard let go");
}

/// Final review, fields 6: a rename opened and left as it was — Enter or a click elsewhere
/// — runs no command (no empty undo step), and a name typed with spaces round it is kept
/// without them.
#[test]
fn an_untouched_rename_runs_no_command_and_names_are_trimmed() {
    for e in editors() {
        for close in ["enter", "away"] {
            let mut r = opened(&e);
            let rev = r.app.bus.revision();
            if close == "enter" {
                r.key(Key::Enter, Modifiers::NONE);
            } else {
                r.click_elsewhere();
            }
            assert!(!(e.is_open)(&r), "{} {close}: closed", e.what);
            assert_eq!(
                r.app.bus.revision(),
                rev,
                "{} {close}: a command ran ({:?})",
                e.what,
                r.app.bus.undo_label_key()
            );
        }
        let mut r = opened(&e);
        r.type_text("  Zed  ");
        r.key(Key::Enter, Modifiers::NONE);
        assert_eq!((e.name)(&r), "Zed", "{}: trimmed", e.what);
    }
}

/// Final review, fields 5: Esc in an export tag cancels the tag only — the dialog stays.
#[test]
fn esc_in_an_export_tag_cancels_the_tag_and_keeps_the_dialog() {
    let mut r = Rig::new(starter_project());
    r.idle(2);
    cantus_ui::views::export::open(&mut r.app, None);
    r.idle(3);
    let artist =
        cantus_ui::kit::field::edit_id(egui::Id::new(("export-tag", "export_audio.tag.artist")));
    let at = r
        .ctx
        .read_response(artist)
        .expect("the artist tag")
        .rect
        .center();
    r.pointer = Some(at);
    r.frame(Vec::new());
    for pressed in [true, false] {
        r.frame(vec![Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }]);
    }
    r.idle(1);
    let before = r.app.export.dialog.as_ref().unwrap().tags.artist.clone();
    r.type_text("Coro");
    r.key(Key::Escape, Modifiers::NONE);
    let d = r.app.export.dialog.as_ref().expect("the dialog stays open");
    assert_eq!(d.tags.artist, before, "the tag as it was");
    r.key(Key::Escape, Modifiers::NONE);
    assert!(
        r.app.export.dialog.is_none(),
        "a second Esc closes the dialog"
    );
}

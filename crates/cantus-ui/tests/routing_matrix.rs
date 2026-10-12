//! The Routing tab's matrix (round 25, design §5, test 13), driven the way a user drives
//! it: frames with pointer and key events, and what the project, the undo history and the
//! hint bar say afterwards. Also the kit's `POST | PRE` switch and Alt+F9.

use cantus_core::{EffectKind, Project, Send, SendTap, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::kit::hint;
use cantus_ui::views::routing::{self, Col, Row};
use cantus_ui::views::workspace::Pane;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{pos2, vec2, Event, Key, Modifiers, Pos2};

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    frame: u64,
    pointer: Option<Pos2>,
    mods: Modifiers,
    /// The whole window (`CantusApp::frame`) instead of the matrix alone.
    window: Option<tempfile::TempDir>,
    size: egui::Vec2,
}

impl Rig {
    fn new(project: Project) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
        app.routing_view.set_in_use(false);
        let mut r = Rig {
            ctx: ctx(),
            app,
            frame: 0,
            pointer: None,
            mods: Modifiers::NONE,
            window: None,
            size: vec2(1200.0, 800.0),
        };
        r.idle(2);
        r
    }

    /// The whole window, 1600 × 1000, its settings kept out of the user's.
    fn window(project: Project) -> Rig {
        let mut r = Rig::new(project);
        let dir = tempfile::tempdir().unwrap();
        r.app.settings_path = dir.path().join("settings.json");
        r.app.layouts_path = dir.path().join("layouts.json");
        r.window = Some(dir);
        r.size = vec2(1600.0, 1000.0);
        r.idle(2);
        r
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame += 1;
        let mut ev = vec![Event::ModifiersChanged(self.mods)];
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
        let window = self.window.is_some();
        let mut out = self.ctx.run_ui(input, |ui| {
            if window {
                app.frame(ui);
            } else {
                egui::CentralPanel::default().show(ui, |ui| views::routing::show(app, ui));
            }
        });
        out.textures_delta.clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    /// The centre of the cell `row` → `col`.
    fn cell(&self, row: Row, col: Col) -> Pos2 {
        let v = &self.app.routing_view;
        let g = v.geometry().expect("the matrix was drawn");
        let r = v.grid().row_of(row).expect("the row is shown");
        let c = v.grid().col_of(col).expect("the column is shown");
        g.cell_rect(r, c).center()
    }

    fn hover(&mut self, p: Pos2) {
        self.pointer = Some(p);
        self.frame(Vec::new());
    }

    fn click(&mut self, p: Pos2, modifiers: Modifiers) {
        self.hover(p);
        self.mods = modifiers;
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
            }]);
        }
        self.mods = Modifiers::NONE;
        self.idle(2);
    }

    /// Press at `from`, move to `to` in steps, release.
    fn drag(&mut self, from: Pos2, to: Pos2, modifiers: Modifiers) {
        self.hover(from);
        self.mods = modifiers;
        self.frame(vec![Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers,
        }]);
        for i in 1..=8 {
            self.pointer = Some(from + (to - from) * (i as f32 / 8.0));
            self.frame(Vec::new());
        }
        self.frame(vec![Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers,
        }]);
        self.mods = Modifiers::NONE;
        self.idle(2);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.mods = modifiers;
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }]);
        self.mods = Modifiers::NONE;
        self.idle(1);
    }

    fn strips(&self) -> Vec<StripId> {
        self.app.project().strips.iter().map(|s| s.id).collect()
    }

    fn undo_steps(&self) -> usize {
        self.app.bus.history().len()
    }

    fn last_label(&self) -> Option<String> {
        self.app.bus.undo_label_key().map(str::to_string)
    }
}

/// Click an empty cell: a unity post-fader send; Ctrl+click: the only output; Alt+click on
/// a strip with one keyable insert: keyed at once — one undo step each. The master column
/// takes the output back.
#[test]
fn a_click_follows_the_desks_grammar() {
    let mut r = Rig::new(welcome_project());
    let ids = r.strips();
    let (a, ret, bus, comp_host) = (ids[0], ids[9], ids[10], ids[11]);
    let steps = r.undo_steps();
    r.click(r.cell(Row::Strip(a), Col::Strip(ret)), Modifiers::NONE);
    let send = r.app.project().strip(a).unwrap().send_to(ret).copied();
    assert_eq!(send.map(|s| (s.level, s.tap)), Some((1.0, SendTap::Post)));
    let labels: Vec<&str> = r
        .app
        .bus
        .history()
        .iter()
        .map(|h| h.label_key.as_str())
        .collect();
    assert_eq!(r.undo_steps(), steps + 1, "{labels:?}");
    assert_eq!(r.last_label().as_deref(), Some("cmd.add_send"));
    // The same cell clicked again only focuses it: no second send, no new step.
    r.click(r.cell(Row::Strip(a), Col::Strip(ret)), Modifiers::NONE);
    assert_eq!(r.undo_steps(), steps + 1);
    assert_eq!(
        r.app.routing_view.focus(),
        Some((Row::Strip(a), Col::Strip(ret)))
    );
    r.click(r.cell(Row::Strip(a), Col::Strip(bus)), Modifiers::COMMAND);
    assert_eq!(r.app.project().strip(a).unwrap().output, Some(bus));
    assert_eq!(r.last_label().as_deref(), Some("cmd.replace_output"));
    assert!(
        r.app.project().strip(a).unwrap().send_to(ret).is_some(),
        "Ctrl never touches the sends"
    );
    r.app.add_effect(comp_host, EffectKind::Compressor);
    r.idle(1);
    r.click(r.cell(Row::Strip(a), Col::Strip(comp_host)), Modifiers::ALT);
    let keyed = r.app.project().strip(comp_host).unwrap().effects[0].sidechain;
    assert_eq!(keyed, Some(a));
    assert_eq!(r.last_label().as_deref(), Some("cmd.key_sidechain"));
    r.click(r.cell(Row::Strip(a), Col::Master), Modifiers::NONE);
    assert_eq!(r.app.project().strip(a).unwrap().output, None);
}

/// Alt+click on a strip with nothing to key opens the duck / gate offers; a pick adds the
/// gate keyed from the row, one step.
#[test]
fn alt_on_a_strip_with_nothing_to_key_offers_a_duck_or_a_gate() {
    let mut r = Rig::new(welcome_project());
    let ids = r.strips();
    let (a, host) = (ids[1], ids[12]);
    let at = r.cell(Row::Strip(a), Col::Strip(host));
    r.click(at, Modifiers::ALT);
    assert!(routing::key_menu_open(&r.app));
    // The menu opens below and right of the pointer: its band, then the duck, the gate.
    let s = 1.0;
    let gate = at + vec2(4.0 * s + 40.0, 4.0 * s + 18.0 + 22.0 + 11.0);
    r.click(gate, Modifiers::NONE);
    assert!(!routing::key_menu_open(&r.app));
    let chain = &r.app.project().strip(host).unwrap().effects;
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].spec.kind, EffectKind::Gate);
    assert_eq!(chain[0].sidechain, Some(a));
}

/// A cell a send would close a loop through is refused before the click — the hint says
/// why and names the strip — and the click issues nothing.
#[test]
fn a_loop_cell_refuses_and_says_why() {
    let mut p = welcome_project();
    let (a, b) = (p.strips[0].id, p.strips[9].id);
    p.strip_mut(a).unwrap().output = Some(b);
    let mut r = Rig::new(p);
    let cell = r.cell(Row::Strip(b), Col::Strip(a));
    {
        let v = &r.app.routing_view;
        let g = v.grid();
        let (ri, ci) = (
            g.row_of(Row::Strip(b)).unwrap(),
            g.col_of(Col::Strip(a)).unwrap(),
        );
        assert_eq!(g.cell(ri, ci).refused, Some(routing::Refused::Loop));
    }
    r.hover(cell);
    let h = hint::current(&r.ctx).expect("the hint bar says something");
    assert!(h.name.contains("loop"), "{h:?}");
    let steps = r.undo_steps();
    let revision = r.app.bus.revision();
    for m in [Modifiers::NONE, Modifiers::COMMAND] {
        r.click(cell, m);
    }
    assert_eq!(r.undo_steps(), steps, "a refused click issues nothing");
    assert_eq!(r.app.bus.revision(), revision);
    assert!(r.app.project().strip(b).unwrap().sends.is_empty());
}

/// The keys of §5 while the matrix has the focus: arrows move it, Enter sends, + raises
/// the level a decibel, Delete removes, Ctrl+Z still undoes (passed on to the keymap).
#[test]
fn the_keys_move_the_focus_and_act() {
    let mut r = Rig::new(welcome_project());
    let ids = r.strips();
    let (a, c0) = (ids[2], ids[8]);
    r.click(r.cell(Row::Strip(a), Col::Strip(c0)), Modifiers::NONE);
    r.app.undo();
    r.idle(1);
    assert!(r.app.project().strip(a).unwrap().sends.is_empty());
    r.key(Key::ArrowRight, Modifiers::NONE);
    let to = ids[9];
    assert_eq!(
        r.app.routing_view.focus(),
        Some((Row::Strip(a), Col::Strip(to)))
    );
    r.key(Key::Enter, Modifiers::NONE);
    let level = |r: &Rig| {
        r.app
            .project()
            .strip(a)
            .unwrap()
            .send_to(to)
            .map(|s| s.level)
    };
    assert_eq!(level(&r), Some(1.0));
    r.key(Key::Minus, Modifiers::NONE);
    let down = level(&r).unwrap();
    assert!(
        (cantus_core::linear_to_db(down) + 1.0).abs() < 1e-3,
        "{down}"
    );
    r.key(Key::Plus, Modifiers::SHIFT);
    let up = level(&r).unwrap();
    assert!((cantus_core::linear_to_db(up) + 0.9).abs() < 1e-3, "{up}");
    r.key(Key::Delete, Modifiers::NONE);
    assert_eq!(level(&r), None);
    r.key(Key::Z, Modifiers::COMMAND);
    assert!(level(&r).is_some(), "Ctrl+Z went through to the keymap");
    // Ctrl+Enter routes the output; Esc gives the focus back.
    r.key(Key::ArrowDown, Modifiers::NONE);
    let b = ids[3];
    r.key(Key::Enter, Modifiers::COMMAND);
    assert_eq!(r.app.project().strip(b).unwrap().output, Some(to));
    r.key(Key::Escape, Modifiers::NONE);
    assert!(r.ctx.memory(|m| m.focused()).is_none());
}

/// Dragging a send's cell moves its level (200 pt the whole travel), a double-click puts
/// it back to 0 dB, and the drag is one undo step.
#[test]
fn a_drag_on_a_send_is_its_level_and_a_double_click_is_0_db() {
    let mut p = welcome_project();
    let (a, ret) = (p.strips[0].id, p.strips[9].id);
    p.strip_mut(a).unwrap().sends.push(Send::new(ret));
    let mut r = Rig::new(p);
    let steps = r.undo_steps();
    let at = r.cell(Row::Strip(a), Col::Strip(ret));
    r.drag(at, at + vec2(0.0, 50.0), Modifiers::NONE);
    let level = |r: &Rig| {
        r.app
            .project()
            .strip(a)
            .unwrap()
            .send_to(ret)
            .unwrap()
            .level
    };
    assert!(level(&r) < 0.85 && level(&r) > 0.6, "{}", level(&r));
    assert_eq!(r.undo_steps(), steps + 1, "one gesture, one step");
    r.hover(at);
    for _ in 0..2 {
        for pressed in [true, false] {
            r.frame(vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
    }
    r.idle(2);
    assert_eq!(level(&r), 1.0);
}

/// Shift+drag down a column copies the send (level, switch, tap) onto the strips it
/// crosses, one undo step that undoes exactly.
#[test]
fn ctrl_drag_down_a_column_copies_the_send() {
    let mut p = welcome_project();
    let ret = p.strips[9].id;
    let src = p.strips[0].id;
    p.strip_mut(src).unwrap().sends.push(Send {
        level: 0.5,
        tap: SendTap::Pre,
        ..Send::new(ret)
    });
    let before = p.clone();
    let mut r = Rig::new(p);
    let steps = r.undo_steps();
    let from = r.cell(Row::Strip(src), Col::Strip(ret));
    let to = r.cell(Row::Strip(r.strips()[3]), Col::Strip(ret));
    r.drag(from, to, Modifiers::COMMAND);
    for i in 1..=3 {
        let s = r.app.project().strips[i].send_to(ret).copied();
        assert_eq!(
            s.map(|s| (s.level, s.tap, s.enabled)),
            Some((0.5, SendTap::Pre, true)),
            "row {i}"
        );
    }
    assert!(r.app.project().strips[4].send_to(ret).is_none());
    assert_eq!(r.undo_steps(), steps + 1);
    assert_eq!(r.last_label().as_deref(), Some("cmd.copy_send"));
    r.app.undo();
    assert_eq!(r.app.project().strips, before.strips);
}

/// A track row is a radio: a click plays the track into that strip (or the master).
#[test]
fn a_track_row_routes_the_track() {
    let mut r = Rig::new(welcome_project());
    let track = r.app.project().tracks[0].id;
    let to = r.strips()[12];
    r.click(r.cell(Row::Track(track), Col::Strip(to)), Modifiers::NONE);
    assert_eq!(r.app.project().track(track).unwrap().strip, Some(to));
    r.click(r.cell(Row::Track(track), Col::Master), Modifiers::NONE);
    assert_eq!(r.app.project().track(track).unwrap().strip, None);
}

/// *In use* opens by itself above 32 strips and hides what routes nowhere; *All* brings
/// every strip back.
#[test]
fn in_use_hides_the_strips_nothing_routes() {
    let mut p = welcome_project();
    while p.strips.len() < 40 {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    assert!(app.routing_view.in_use(app.project().strips.len()));
    app.routing_view.set_in_use(true);
    let mut r = Rig {
        ctx: ctx(),
        app,
        frame: 0,
        pointer: None,
        mods: Modifiers::NONE,
        window: None,
        size: vec2(1200.0, 800.0),
    };
    r.idle(2);
    let shown = r.app.routing_view.grid().strip_rows;
    assert_eq!(shown, 4, "the four strips the tracks play into");
    assert_eq!(r.app.routing_view.grid().hidden, 36);
    r.app.routing_view.set_in_use(false);
    r.idle(1);
    assert_eq!(r.app.routing_view.grid().strip_rows, 40);
}

/// Alt+F9 (every preset) shows and hides the Routing tab, in the lower zone.
#[test]
fn alt_f9_opens_the_routing_tab() {
    use cantus_ui::keymap::{Action, Binding, Preset};
    for preset in Preset::ALL {
        let map = preset.keymap();
        assert!(
            map.bindings_for(Action::ViewRouting)
                .contains(&Binding::alt(Key::F9)),
            "{preset:?}"
        );
        assert!(map.conflicts().is_empty(), "{preset:?}");
    }
    let mut r = Rig::new(welcome_project());
    assert!(!r.app.workspace.contains(Pane::Routing));
    r.app.run_action(Action::ViewRouting);
    assert!(r.app.workspace.is_visible(Pane::Routing));
    assert!(Pane::Routing.is_lower());
    r.app.run_action(Action::ViewRouting);
    assert!(!r.app.workspace.contains(Pane::Routing));
}

/// The kit's `POST | PRE` switch: a click on the other segment picks it, a click on the lit
/// one picks nothing, and the hint says what the tap means.
#[test]
fn the_tap_switch_picks_and_explains() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let ctx = ctx();
    let rect = egui::Rect::from_min_size(pos2(20.0, 20.0), vec2(80.0, 22.0));
    let mut picked = Vec::new();
    let run = |events: Vec<Event>, pointer: Pos2, tap: SendTap| {
        let mut ev = vec![Event::PointerMoved(pointer)];
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0))),
            events: ev,
            ..Default::default()
        };
        let mut got = None;
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let s = cantus_ui::kit::Style::current();
                cantus_ui::kit::with_brush(ui.painter(), |b| {
                    got = cantus_ui::kit::tap::switch(ui, b, &s, rect, ui.id().with("t"), tap);
                });
            });
        });
        out.textures_delta.clear();
        got
    };
    let pre = pos2(rect.right() - 10.0, rect.center().y);
    let post = pos2(rect.left() + 10.0, rect.center().y);
    for (p, tap) in [(pre, SendTap::Post), (post, SendTap::Post)] {
        run(Vec::new(), p, tap);
        let press = |pressed| Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(vec![press(true)], p, tap);
        picked.push(run(vec![press(false)], p, tap));
    }
    assert_eq!(picked, vec![Some(SendTap::Pre), None]);
    run(Vec::new(), pre, SendTap::Post);
    let h = hint::current(&ctx).expect("hovered");
    assert!(h.name.contains("Pre-fader"), "{h:?}");
    assert!(h.value.contains("mute"), "{h:?}");
}

/// Integration finding: *Show the destination* from the matrix waited unseen while the
/// desk was behind the Routing tab. It brings the desk to the front and scrolls the
/// destination into view.
#[test]
fn show_the_destination_from_the_matrix_brings_the_desk_forward() {
    let mut p = welcome_project();
    while p.strips.len() < 64 {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    let mut r = Rig::window(p);
    let ids = r.strips();
    let (from, to) = (ids[1], ids[60]);
    r.app.set_strip_send(from, Send::new(to));
    r.app.end_gesture();
    r.app.run_action(cantus_ui::keymap::Action::ViewRouting);
    r.idle(3);
    assert!(r.app.workspace.is_visible(Pane::Routing));
    let at = r.cell(Row::Strip(from), Col::Strip(to));
    r.hover(at);
    for pressed in [true, false] {
        r.frame(vec![Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        }]);
    }
    r.idle(2);
    let owner = routing::owner();
    let row = cantus_ui::views::route_menu::row_rect(&r.ctx, owner, "routing.menu.show_dest")
        .or_else(|| cantus_ui::views::route_menu::row_rect(&r.ctx, owner, "Show the destination"))
        .unwrap_or_else(|| {
            panic!(
                "no Show row: {:?}",
                cantus_ui::views::route_menu::row_labels(&r.ctx, owner)
            )
        });
    r.click(row.center(), Modifiers::NONE);
    r.idle(3);
    assert!(
        r.app.workspace.is_visible(Pane::Mixer),
        "the desk is in front"
    );
    assert!(r.app.mixer.scroll().0 > 0.0, "strip 61 scrolled into view");
}

/// Integration finding: with the matrix holding the focus the keymap skipped the typing
/// piano, so a note held while the matrix took the focus never ended, and no note could
/// be played. The matrix forwards through the one helper the keymap uses
/// (`CantusApp::dispatch_keys`).
#[test]
fn the_typing_piano_plays_and_releases_while_the_matrix_has_the_focus() {
    let p = welcome_project();
    let mut r = Rig::new(p);
    let ids = r.strips();
    let track = r.app.project().tracks[0].id;
    r.app.selected_track = Some(track);
    r.app.keyboard_piano = true;
    // A click on a cell gives the matrix the keyboard focus.
    r.click(
        r.cell(Row::Strip(ids[0]), Col::Strip(ids[5])),
        Modifiers::NONE,
    );
    r.app.undo();
    r.idle(1);
    assert!(
        r.ctx.memory(|m| m.focused()).is_some(),
        "the matrix has the focus"
    );
    let ev = |pressed| Event::Key {
        key: Key::A,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    r.frame(vec![ev(true)]);
    assert_eq!(r.app.keyboard.held_count(), 1, "A plays a note");
    r.idle(2);
    r.frame(vec![ev(false)]);
    assert_eq!(r.app.keyboard.held_count(), 0, "its release ends it");
}

/// Integration finding: Shift at the start of a drag copied the send instead of the
/// house's fine drag. Shift+drag is fine (a tenth of the travel), copies nothing; the
/// copy is Ctrl+drag.
#[test]
fn shift_drag_on_a_send_is_the_fine_level() {
    let mut p = welcome_project();
    let (a, ret) = (p.strips[0].id, p.strips[9].id);
    p.strip_mut(a).unwrap().sends.push(Send {
        level: 0.8,
        ..Send::new(ret)
    });
    let mut r = Rig::new(p);
    let at = r.cell(Row::Strip(a), Col::Strip(ret));
    let level = |r: &Rig| {
        r.app
            .project()
            .strip(a)
            .unwrap()
            .send_to(ret)
            .unwrap()
            .level
    };
    r.drag(at, at + vec2(0.0, 50.0), Modifiers::NONE);
    let coarse = 0.8 - level(&r);
    r.app.undo();
    r.idle(1);
    r.drag(at, at + vec2(0.0, 50.0), Modifiers::SHIFT);
    let fine = 0.8 - level(&r);
    assert!(coarse > 0.0 && fine > 0.0, "{coarse} {fine}");
    assert!(
        (fine * 10.0 - coarse).abs() < coarse * 0.2,
        "a tenth of the travel: {fine} vs {coarse}"
    );
    let others = r.app.project().strips[1..9]
        .iter()
        .filter(|s| s.send_to(ret).is_some())
        .count();
    assert_eq!(others, 0, "nothing copied");
    assert_eq!(r.last_label().as_deref(), Some("cmd.set_strip_send"));
}

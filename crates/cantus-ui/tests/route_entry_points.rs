//! Round 25's integration: every way into a route, pressed on the **real** control a
//! musician presses — a strip's port in the cable band, a lit cable's end mark, the "+"
//! row of Extra large, the tap chip of a send row, the selected cable's chip, the
//! inspector's `POST | PRE` switch, the route menu's rows — through whole frames, and
//! what the project and the undo stack say afterwards. (The reviewers found the cable
//! gestures were driven only by seeding the press the desk would record.)

use cantus_core::routing::Route;
use cantus_core::{Dock, EffectKind, Project, Send, SendTap, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::views::mixer::{self, layout::WidthMode, Control};
use cantus_ui::views::route_menu;
use cantus_ui::{starter_project, views, CantusApp};
use egui::{vec2, Event, Modifiers, Pos2, Rect};

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
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    window: Option<tempfile::TempDir>,
}

impl Rig {
    fn desk(project: Project) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
        Rig {
            ctx: ctx(),
            app,
            size: vec2(1600.0, 1000.0),
            frame: 0,
            pointer: None,
            window: None,
        }
    }

    /// The whole window, tall enough for the inspector's sends.
    fn window(project: Project) -> Rig {
        let mut r = Rig::desk(project);
        let dir = tempfile::tempdir().unwrap();
        r.app.settings_path = dir.path().join("settings.json");
        r.app.layouts_path = dir.path().join("layouts.json");
        r.app.central = cantus_ui::app::CentralView::Mixer;
        r.window = Some(dir);
        r.size = vec2(1600.0, 2400.0);
        r
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame += 1;
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
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
                egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
            }
        });
        out.textures_delta.clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn ids(&self) -> Vec<StripId> {
        self.app.project().strips.iter().map(|s| s.id).collect()
    }

    fn rect(&self, strip: Option<StripId>, c: Control) -> Rect {
        mixer::control_rect(&self.ctx, strip, c)
            .unwrap_or_else(|| panic!("{c:?} of {strip:?} is not on the desk"))
    }

    fn button(&mut self, at: Pos2, pressed: bool, button: egui::PointerButton) {
        self.pointer = Some(at);
        self.frame(vec![Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        }]);
    }

    fn click(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(Vec::new());
        self.button(at, true, egui::PointerButton::Primary);
        self.button(at, false, egui::PointerButton::Primary);
        self.idle(2);
    }

    fn right_click(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(Vec::new());
        self.button(at, true, egui::PointerButton::Secondary);
        self.button(at, false, egui::PointerButton::Secondary);
        self.idle(2);
    }

    /// Press at `a`, travel past the drag threshold to `z` in steps, release there.
    fn drag(&mut self, a: Pos2, z: Pos2) {
        self.pointer = Some(a);
        self.frame(Vec::new());
        self.button(a, true, egui::PointerButton::Primary);
        for k in 1..=8 {
            self.pointer = Some(a + (z - a) * (k as f32 / 8.0));
            self.frame(Vec::new());
        }
        self.frame(Vec::new());
        self.button(z, false, egui::PointerButton::Primary);
        self.idle(2);
    }

    fn label(&self) -> Option<&str> {
        self.app.bus.undo_label_key()
    }

    /// Click the row `key` of the route menu `owner` opened (hovered first, as a pointer
    /// passing on its way opens a submenu).
    fn row(&mut self, owner: egui::Id, key: &str) {
        let r = route_menu::row_rect(&self.ctx, owner, key).unwrap_or_else(|| {
            panic!(
                "no row {key:?}; rows: {:?}",
                route_menu::row_labels(&self.ctx, owner)
            )
        });
        self.pointer = Some(r.center());
        self.idle(2);
        self.click(r.center());
    }
}

fn desk_owner() -> egui::Id {
    egui::Id::new("mixer.route_menu")
}

fn send_of(app: &CantusApp, from: StripId, to: StripId) -> Option<Send> {
    app.project().strip(from)?.send_to(to).copied()
}

// ---- the cable gestures from the real controls ------------------------------------------

/// A drag from strip 2's real port onto strip 5's column: a unity post-fader send, the
/// output untouched, one undo step.
#[test]
fn a_cable_dragged_from_a_real_port_sends_at_0_db() {
    let mut r = Rig::desk(starter_project());
    r.idle(3);
    let s = r.ids();
    let port = r.rect(Some(s[1]), Control::Port).center();
    let to = r.rect(Some(s[4]), Control::Fader).center();
    r.drag(port, to);
    assert_eq!(send_of(&r.app, s[1], s[4]), Some(Send::new(s[4])));
    assert_eq!(r.app.project().strip(s[1]).unwrap().output, None);
    assert_eq!(r.label(), Some("cmd.add_send"));
}

/// A lit send cable's destination end mark dragged onto another column: the route moves,
/// its level and tap kept (`cmd.move_route`).
#[test]
fn a_real_end_mark_dragged_moves_the_route() {
    let mut r = Rig::desk(starter_project());
    let s = r.ids();
    r.app.set_strip_send(
        s[1],
        Send {
            level: 0.5,
            tap: SendTap::Pre,
            ..Send::new(s[3])
        },
    );
    r.app.end_gesture();
    r.app.select_strip(s[1]);
    r.idle(3);
    let cable = *r
        .app
        .mixer
        .cables()
        .iter()
        .find(|d| d.route == Route::send(s[1], s[3]))
        .expect("the send's cable is lit");
    // The pointer over the destination lights its cables too (lanes may shift): find the
    // mark where it is drawn with the pointer on it.
    let mut mark = mixer::end_mark_rect(&r.ctx, cable.edge, false).expect("its end mark");
    for _ in 0..4 {
        r.pointer = Some(mark.center());
        r.idle(2);
        let cable = *r
            .app
            .mixer
            .cables()
            .iter()
            .find(|d| d.route == Route::send(s[1], s[3]))
            .expect("still lit");
        mark = mixer::end_mark_rect(&r.ctx, cable.edge, false).expect("its end mark");
    }
    let to = r.rect(Some(s[6]), Control::Fader).center();
    r.drag(mark.center(), to);
    let moved = send_of(&r.app, s[1], s[6]).expect("the send moved");
    assert_eq!((moved.level, moved.tap), (0.5, SendTap::Pre));
    assert!(send_of(&r.app, s[1], s[3]).is_none());
    assert_eq!(r.label(), Some("cmd.move_route"));
}

/// Extra large's "+" row is a port too: dragged onto a column, it sends there.
#[test]
fn the_plus_row_dragged_onto_a_strip_sends() {
    let mut r = Rig::desk(starter_project());
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    r.idle(3);
    let s = r.ids();
    let plus = r.rect(Some(s[2]), Control::SendAdd).center();
    let to = r.rect(Some(s[7]), Control::Fader).center();
    r.drag(plus, to);
    assert_eq!(send_of(&r.app, s[2], s[7]), Some(Send::new(s[7])));
    assert_eq!(r.label(), Some("cmd.add_send"));
}

// ---- POST | PRE from every entry point ---------------------------------------------------

/// The tap chip of an Extra large send row: a click makes the send pre-fader.
#[test]
fn the_send_rows_chip_makes_it_pre() {
    let mut r = Rig::desk(starter_project());
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    let s = r.ids();
    r.app.set_strip_send(s[1], Send::new(s[5]));
    r.app.end_gesture();
    r.idle(3);
    let chip = mixer::tap_chip_rect(&r.ctx, s[1], 0).expect("the row's chip");
    r.click(chip.center());
    assert_eq!(send_of(&r.app, s[1], s[5]).unwrap().tap, SendTap::Pre);
    assert_eq!(r.label(), Some("cmd.set_send_tap"));
    // And back.
    r.click(chip.center());
    assert_eq!(send_of(&r.app, s[1], s[5]).unwrap().tap, SendTap::Post);
}

/// The selected send cable's chip beside its pill.
#[test]
fn the_selected_pills_chip_makes_it_pre() {
    let mut r = Rig::desk(starter_project());
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let s = r.ids();
    let ret = r
        .app
        .project()
        .strips
        .iter()
        .find(|x| x.dock == Dock::Right)
        .map(|x| x.id)
        .unwrap();
    r.app.set_strip_send(s[1], Send::new(ret));
    r.app.end_gesture();
    r.app.select_strip(s[1]);
    r.idle(3);
    let pill = mixer::pill_rect(&r.ctx, s[1], ret).expect("the send's pill");
    r.click(pill.center());
    assert_eq!(r.app.mixer.selected_cable(), Some(Route::send(s[1], ret)));
    let chip = mixer::pill_tap_rect(&r.ctx, s[1], ret).expect("the chip beside the pill");
    r.click(chip.center());
    assert_eq!(send_of(&r.app, s[1], ret).unwrap().tap, SendTap::Pre);
    assert_eq!(r.label(), Some("cmd.set_send_tap"));
}

/// The inspector's send row: the `PRE` segment of its switch.
#[test]
fn the_inspectors_switch_makes_it_pre() {
    let mut p = starter_project();
    let (src, ret) = (p.strips[1].id, p.strips[5].id);
    p.strips[1].sends.push(Send::new(ret));
    let mut r = Rig::window(p);
    r.app.select_strip(src);
    r.idle(4);
    let seg = r
        .ctx
        .read_response(egui::Id::new(("insp-send-tap", src, ret)))
        .expect("the inspector's switch")
        .rect;
    // Two segments, POST then PRE: the right half is PRE.
    r.click(seg.center() + vec2(seg.width() * 0.25, 0.0));
    assert_eq!(send_of(&r.app, src, ret).unwrap().tap, SendTap::Pre);
    assert_eq!(r.label(), Some("cmd.set_send_tap"));
}

/// The route menu's *Pre-fader* radio, opened from a send row's right-click.
#[test]
fn the_route_menus_radio_makes_it_pre() {
    let mut r = Rig::desk(starter_project());
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    let s = r.ids();
    r.app.set_strip_send(s[1], Send::new(s[5]));
    r.app.end_gesture();
    r.idle(3);
    r.right_click(r.rect(Some(s[1]), Control::SendRow(0)).center());
    assert!(route_menu::is_open(&r.ctx, desk_owner()));
    r.row(desk_owner(), "routing.menu.pre");
    assert_eq!(send_of(&r.app, s[1], s[5]).unwrap().tap, SendTap::Pre);
    assert_eq!(r.label(), Some("cmd.set_send_tap"));
}

// ---- every row of the route menu (finding 20) ---------------------------------------------

/// Open the route menu on `route` at the middle of the desk.
fn open(r: &mut Rig, route: Route) {
    route_menu::open(&r.ctx, desk_owner(), route, egui::pos2(500.0, 400.0));
    r.idle(2);
    assert!(route_menu::is_open(&r.ctx, desk_owner()));
}

/// A send's menu: Level ▸ a step, Send on, Post / Pre, Make this the output, Select the
/// strips routed here / it feeds, Remove — each the project change and the undo label it
/// names.
#[test]
fn a_sends_route_menu_rows_each_do_what_they_say() {
    let mut r = Rig::desk(starter_project());
    let s = r.ids();
    let route = Route::send(s[1], s[5]);
    r.app.set_strip_send(s[1], Send::new(s[5]));
    r.app.end_gesture();
    r.idle(2);
    // Level ▸ −6 dB.
    open(&mut r, route);
    let level = route_menu::row_rect(&r.ctx, desk_owner(), "routing.menu.level").unwrap();
    r.pointer = Some(level.center());
    r.idle(3);
    let step = "\u{2212}6.0 dB";
    r.row(desk_owner(), step);
    let l = send_of(&r.app, s[1], s[5]).unwrap().level;
    assert!((cantus_core::linear_to_db(l) + 6.0).abs() < 0.01, "{l}");
    // Send on (off now).
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.send_on");
    assert!(!send_of(&r.app, s[1], s[5]).unwrap().enabled);
    // Pre, then Post.
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.pre");
    assert_eq!(send_of(&r.app, s[1], s[5]).unwrap().tap, SendTap::Pre);
    assert_eq!(r.label(), Some("cmd.set_send_tap"));
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.post");
    assert_eq!(send_of(&r.app, s[1], s[5]).unwrap().tap, SendTap::Post);
    // Select the strips routed here / the strip it comes from.
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.select_into");
    assert_eq!(r.app.selected_strip, Some(s[1]));
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.select_from");
    assert!(r.app.selected_strip.is_some());
    // Make this the output: the send goes, the destination is the output.
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.make_output");
    let st = r.app.project().strip(s[1]).unwrap();
    assert_eq!((st.output, st.send_to(s[5]).is_some()), (Some(s[5]), false));
    assert_eq!(r.label(), Some("cmd.send_to_output"));
    r.app.undo();
    // Remove.
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.remove");
    assert!(send_of(&r.app, s[1], s[5]).is_none());
    assert_eq!(r.label(), Some("cmd.remove_route"));
}

/// An output's menu: Make this a send, Show the destination, Remove (back to the master).
#[test]
fn an_outputs_route_menu_rows_each_do_what_they_say() {
    let mut r = Rig::desk(starter_project());
    let s = r.ids();
    r.app.exec(cantus_core::Command::SetStripOutput {
        id: s[1],
        output: Some(s[5]),
    });
    r.idle(2);
    let route = Route::output(s[1], s[5]);
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.make_send");
    let st = r.app.project().strip(s[1]).unwrap();
    assert_eq!(
        (st.output, st.send_to(s[5]).copied()),
        (None, Some(Send::new(s[5])))
    );
    assert_eq!(r.label(), Some("cmd.output_to_send"));
    r.app.undo();
    r.idle(1);
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.show_dest");
    assert!(
        !route_menu::is_open(&r.ctx, desk_owner()),
        "the menu closed"
    );
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.remove_output");
    assert_eq!(r.app.project().strip(s[1]).unwrap().output, None);
    assert_eq!(r.label(), Some("cmd.remove_route"));
}

/// A key's menu: Open the keyed effect, Remove.
#[test]
fn a_keys_route_menu_rows_each_do_what_they_say() {
    let mut r = Rig::desk(starter_project());
    let s = r.ids();
    r.app.add_effect(s[6], EffectKind::Compressor);
    let fx = r.app.project().strip(s[6]).unwrap().effects[0].id;
    r.app.exec(cantus_core::Command::SetEffectSidechain {
        strip: s[6],
        effect: fx,
        source: Some(s[2]),
    });
    r.idle(2);
    let route = Route::key(s[2], s[6], fx);
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.open_keyed");
    assert_eq!(
        r.app.comp_editor.open,
        Some((s[6], fx)),
        "the keyed compressor's editor opened"
    );
    open(&mut r, route);
    r.row(desk_owner(), "routing.menu.remove");
    let e = &r.app.project().strip(s[6]).unwrap().effects[0];
    assert_eq!(e.sidechain, None);
    assert_eq!(r.label(), Some("cmd.remove_route"));
}

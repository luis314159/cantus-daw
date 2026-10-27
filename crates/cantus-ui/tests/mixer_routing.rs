//! Round 25's routing gestures on the desk, driven the way a musician drives them: frames
//! with pointer and key events, and what the project, the undo stack, the menus and the
//! hint bar say afterwards (design §3.3, §4.1–4.4; the owner's answers 2, 11, 12).
//!
//! - strips and the selection dragged by their number cells, a group by its separator —
//!   one undo step each, docks respected, Esc cancels, a click still selects;
//! - a track (or a channel-rack channel) dropped on a strip routes it;
//! - the *Route here* plate on a strip nothing feeds (and not on a return fed by sends),
//!   its menu, *New return ▸*, the route menu's key (Shift+F10);
//! - the inspector's *Receives*.
//!
//! The cable gestures from a port and an end (whose controls the desk builder draws) are
//! driven in `views/mixer_drag.rs`'s own tests.

use cantus_core::{Dock, Project, Send, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::kit::hint;
use cantus_ui::routing_actions::TrackDrag;
use cantus_ui::views::mixer::{self, Control};
use cantus_ui::views::route_menu;
use cantus_ui::{starter_project, views, CantusApp};
use egui::{pos2, vec2, Event, Key, Modifiers, Pos2};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

/// The desk's own route menus (`mixer::route_owner`).
fn desk_owner() -> egui::Id {
    egui::Id::new("mixer.route_menu")
}

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    mods: Modifiers,
    /// The whole window (the inspector's tests) or the mixer alone.
    window: bool,
    _dir: Option<tempfile::TempDir>,
}

impl Rig {
    fn new(project: Project) -> Rig {
        Rig {
            ctx: ctx(),
            app: app(project),
            size: vec2(1600.0, 900.0),
            frame: 0,
            pointer: None,
            mods: Modifiers::NONE,
            window: false,
            _dir: None,
        }
    }

    /// The whole window (transport, arrangement, inspector, the mixer below).
    fn window(project: Project) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rig::new(project);
        r.app.settings_path = dir.path().join("settings.json");
        r.app.layouts_path = dir.path().join("layouts.json");
        r.app.central = cantus_ui::app::CentralView::Mixer;
        r.window = true;
        r._dir = Some(dir);
        r
    }

    fn frame(&mut self, events: Vec<Event>) -> String {
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
        let window = self.window;
        let mut out = self.ctx.run_ui(input, |ui| {
            if window {
                app.frame(ui);
            } else {
                egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
            }
        });
        out.textures_delta.clear();
        painted_text(&out.shapes)
    }

    fn idle(&mut self, n: usize) -> String {
        let mut text = String::new();
        for _ in 0..n {
            text = self.frame(Vec::new());
        }
        text
    }

    fn rect(&self, strip: Option<StripId>, c: Control) -> egui::Rect {
        mixer::control_rect(&self.ctx, strip, c)
            .unwrap_or_else(|| panic!("{c:?} of {strip:?} is not on the desk"))
    }

    fn id_rect(&self, id: egui::Id) -> egui::Rect {
        self.ctx
            .read_response(id)
            .map(|r| r.rect)
            .unwrap_or_else(|| panic!("{id:?} was not drawn"))
    }

    fn button(&mut self, at: Pos2, pressed: bool) -> String {
        self.pointer = Some(at);
        let m = self.mods;
        self.frame(vec![Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: m,
        }])
    }

    fn click(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(Vec::new());
        self.button(at, true);
        self.button(at, false);
        self.idle(2);
    }

    fn right_click(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(Vec::new());
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.idle(2);
    }

    /// Press at `a`, travel to `z` in steps (each a frame), and stay there pressed.
    fn press_and_move(&mut self, a: Pos2, z: Pos2) -> String {
        self.pointer = Some(a);
        self.frame(Vec::new());
        self.button(a, true);
        let mut text = String::new();
        for k in 1..=8 {
            let u = k as f32 / 8.0;
            self.pointer = Some(a + (z - a) * u);
            text = self.frame(Vec::new());
        }
        // Let the frame after the last move draw what the pointer is over.
        self.frame(Vec::new());
        text
    }

    fn drag(&mut self, a: Pos2, z: Pos2) {
        self.press_and_move(a, z);
        self.button(z, false);
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
        self.mods = Modifiers::NONE;
        self.idle(2);
    }

    fn ids(&self) -> Vec<StripId> {
        self.app.project().strips.iter().map(|s| s.id).collect()
    }

    fn order(&self) -> Vec<StripId> {
        self.ids()
    }

    fn hint(&self) -> (String, String) {
        hint::current(&self.ctx)
            .map(|h| (h.name, h.value))
            .unwrap_or_default()
    }

    /// Click the row `label` (its key, or its words) of a route menu `owner` opened.
    fn click_row(&mut self, owner: egui::Id, label: &str) {
        let r = route_menu::row_rect(&self.ctx, owner, label).unwrap_or_else(|| {
            panic!(
                "no row {label:?}; rows: {:?}",
                route_menu::row_labels(&self.ctx, owner)
            )
        });
        // Hover first: a submenu opens on hover, as a musician's pointer passes.
        self.pointer = Some(r.center());
        self.idle(2);
        self.click(r.center());
    }
}

fn painted_text(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn walk(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push_str(" | ");
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    for s in shapes {
        walk(&s.shape, &mut out);
    }
    out
}

/// One undo step brings the project back (ids handed out stay handed out).
fn undo_restores(app: &mut CantusApp, before: &Project) {
    app.undo();
    let mut now = app.project().clone();
    now.next_id = before.next_id;
    assert!(&now == before, "one undo step restores the project");
}

// ---- strips and groups along the desk (answer 12) ---------------------------------------

#[test]
fn a_number_cell_dragged_moves_the_strip_in_one_undo_step() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let before = r.app.project().clone();
    // Strip 2 dragged past the middle of strip 5: it lands between 5 and 6.
    let from = r.rect(Some(s[1]), Control::Head).center();
    let to = r.rect(Some(s[4]), Control::Head).right_center() - vec2(4.0, 0.0);
    let text = r.press_and_move(from, to);
    assert!(
        text.contains("Move"),
        "the chip says what releasing does: {text}"
    );
    r.button(to, false);
    r.idle(2);
    let now = r.order();
    assert_eq!(&now[..6], &[s[0], s[2], s[3], s[4], s[1], s[5]]);
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.move_strips"));
    assert_eq!(
        r.app.selected_strip,
        Some(s[1]),
        "the moved strip is the selection"
    );
    undo_restores(&mut r.app, &before);
}

#[test]
fn a_click_on_a_number_cell_still_selects_and_moves_nothing() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let before = r.order();
    let at = r.rect(Some(s[3]), Control::Head).center();
    r.click(at);
    assert_eq!(r.app.selected_strip, Some(s[3]));
    assert_eq!(r.order(), before);
}

#[test]
fn escape_cancels_a_strip_drag() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let before = r.app.project().clone();
    let from = r.rect(Some(s[1]), Control::Head).center();
    let to = r.rect(Some(s[6]), Control::Head).center();
    r.press_and_move(from, to);
    r.frame(vec![Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    r.button(to, false);
    r.idle(2);
    assert!(r.app.project() == &before, "nothing moved");
}

#[test]
fn the_selection_moves_as_one_block() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    r.app.select_strip(s[1]);
    r.app.toggle_strip_selected(s[2]);
    r.idle(2);
    let from = r.rect(Some(s[2]), Control::Head).center();
    let to = r.rect(Some(s[6]), Control::Head).right_center() - vec2(3.0, 0.0);
    r.drag(from, to);
    let now = r.order();
    assert_eq!(&now[..7], &[s[0], s[3], s[4], s[5], s[6], s[1], s[2]]);
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.move_strips"));
}

#[test]
fn a_strip_dropped_on_the_right_dock_is_pinned_there() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let ret = *r
        .app
        .project()
        .strips
        .iter()
        .find(|x| x.dock == Dock::Right)
        .map(|x| &x.id)
        .unwrap();
    let from = r.rect(Some(s[2]), Control::Head).center();
    let to = r.rect(Some(ret), Control::Head).left_center() + vec2(3.0, 0.0);
    r.drag(from, to);
    let p = r.app.project();
    assert_eq!(p.strip(s[2]).unwrap().dock, Dock::Right);
    let right: Vec<StripId> = p
        .strips
        .iter()
        .filter(|x| x.dock == Dock::Right)
        .map(|x| x.id)
        .collect();
    assert_eq!(right[0], s[2], "it lands before the first return");
}

#[test]
fn a_group_dragged_by_its_separator_moves_every_member() {
    let mut r = Rig::new(starter_project());
    let s = r.ids();
    r.app.create_strip_group(&[s[1], s[2]]);
    r.app.end_gesture();
    r.idle(3);
    let gid = r.app.project().strip(s[1]).unwrap().group.unwrap();
    let sep = r.id_rect(egui::Id::new(("mixer-group", gid.0)));
    let before = r.app.project().clone();
    let to = r.rect(Some(s[6]), Control::Head).right_center() - vec2(3.0, 0.0);
    r.drag(sep.center(), to);
    let now = r.order();
    assert_eq!(&now[..7], &[s[0], s[3], s[4], s[5], s[6], s[1], s[2]]);
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.move_group"));
    let g = r.app.project().strip_group(gid).unwrap();
    assert!(!g.collapsed, "the drag did not fold the group");
    undo_restores(&mut r.app, &before);
}

// ---- a track dropped on a strip (answer 11) ---------------------------------------------

#[test]
fn a_track_dropped_on_a_strip_routes_it() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let track = r.app.project().tracks[0].id;
    let target = r.rect(Some(s[7]), Control::Fader).center();
    // The arrangement (or the channel rack) set the payload when the drag began.
    r.pointer = Some(target - vec2(200.0, 0.0));
    r.button(target - vec2(200.0, 0.0), true);
    egui::DragAndDrop::set_payload(&r.ctx, TrackDrag(track));
    for k in 1..=6 {
        r.pointer = Some(target - vec2(200.0 - 200.0 * k as f32 / 6.0, 0.0));
        r.frame(Vec::new());
    }
    r.frame(Vec::new());
    let (name, value) = r.hint();
    assert!(
        value.starts_with("Route ") && value.contains("8"),
        "the hint names the drop: {name} / {value}"
    );
    // Its own words, not the cable gesture's modifiers (integration finding).
    assert!(name.starts_with("Drop on a strip: the track"), "{name}");
    assert!(!name.contains("Ctrl"), "{name}");
    r.button(target, false);
    r.idle(2);
    assert_eq!(r.app.project().tracks[0].strip, Some(s[7]));
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.route_tracks"));
    assert_eq!(r.app.selected_strip, Some(s[7]));
    assert!(egui::DragAndDrop::payload::<TrackDrag>(&r.ctx).is_none());
}

#[test]
fn an_arrangement_header_dragged_carries_its_track() {
    let mut r = Rig::window(starter_project());
    r.idle(4);
    let track = r.app.project().tracks[0].id;
    let chip = cantus_ui::views::device_chrome::spot(
        &r.ctx,
        cantus_ui::views::device_chrome::Spot::Chip(track),
    )
    .expect("the header is drawn");
    // On the header's name, right of its instrument chip.
    let a = chip.right_center() + vec2(40.0, 0.0);
    r.pointer = Some(a);
    r.frame(Vec::new());
    r.button(a, true);
    for k in 1..=4 {
        r.pointer = Some(a + vec2(0.0, 6.0 * k as f32));
        r.frame(Vec::new());
    }
    assert_eq!(
        egui::DragAndDrop::payload::<TrackDrag>(&r.ctx).map(|p| p.0),
        Some(track)
    );
    r.button(r.pointer.unwrap(), false);
}

// ---- Route here (design §4.2) -------------------------------------------------------------

#[test]
fn the_plate_shows_on_a_strip_nothing_feeds_and_not_on_a_return_fed_by_sends() {
    let mut p = starter_project();
    let ret = p.strips.iter().find(|x| x.dock == Dock::Right).unwrap().id;
    let src = p.strips[0].id;
    p.strips[0].sends.push(Send::new(ret));
    let mut r = Rig::new(p);
    r.idle(3);
    let s = r.ids();
    let empty = s[5];
    assert!(
        r.ctx.read_response(route_menu::plate_id(empty)).is_some(),
        "an unfed strip shows the plate"
    );
    assert!(
        r.ctx.read_response(route_menu::plate_id(ret)).is_none(),
        "a return fed by a send is fed"
    );
    assert!(
        r.ctx.read_response(route_menu::plate_id(src)).is_none(),
        "the strip a track feeds is fed"
    );
    // Its hint says what it does.
    let plate = r.id_rect(route_menu::plate_id(empty));
    r.pointer = Some(plate.center());
    r.idle(2);
    assert_eq!(r.hint().0, "Route here");
}

#[test]
fn the_plate_opens_route_here_and_routes_the_selected_track() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let track = r.app.project().tracks[0].id;
    r.app.select_track(track);
    r.idle(2);
    let before = r.app.project().clone();
    let plate = r.id_rect(route_menu::plate_id(s[5]));
    r.click(plate.center());
    assert!(route_menu::is_open(&r.ctx, desk_owner()), "the menu opened");
    let name = r.app.project().tracks[0].name.clone();
    r.click_row(desk_owner(), &format!("Route “{name}” here"));
    assert_eq!(r.app.project().tracks[0].strip, Some(s[5]));
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.route_tracks"));
    assert!(
        !route_menu::is_open(&r.ctx, desk_owner()),
        "a pick closes it"
    );
    r.idle(2);
    assert!(
        r.ctx.read_response(route_menu::plate_id(s[5])).is_none(),
        "the plate goes the frame the strip is fed"
    );
    undo_restores(&mut r.app, &before);
}

#[test]
fn a_return_plate_offers_the_sends_first() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let ret = r
        .app
        .project()
        .strips
        .iter()
        .find(|x| x.dock == Dock::Right)
        .unwrap()
        .id;
    r.app.select_strip(s[1]);
    r.idle(2);
    let plate = r.id_rect(route_menu::plate_id(ret));
    r.click(plate.center());
    let rows = route_menu::row_labels(&r.ctx, desk_owner());
    assert!(rows[0].starts_with("Send 2"), "the send first: {rows:?}");
    r.click_row(desk_owner(), &rows[0]);
    let send = r.app.project().strip(s[1]).unwrap().send_to(ret).copied();
    assert_eq!(send, Some(Send::new(ret)), "a unity, post-fader send");
    assert_eq!(r.app.project().strip(s[1]).unwrap().output, None);
}

// ---- New return, the route menu's key -----------------------------------------------------

#[test]
fn a_new_return_with_a_reverb_from_the_add_send_list() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let before = r.app.project().clone();
    route_menu::open_add_send(&r.ctx, desk_owner(), s[0], pos2(400.0, 200.0));
    r.idle(2);
    r.click_row(desk_owner(), "routing.menu.new_return");
    // The submenu opened on the row; pick the reverb.
    r.click_row(desk_owner(), "routing.menu.return_reverb");
    let p = r.app.project();
    let new = p.strips.last().unwrap();
    assert_eq!(new.dock, Dock::Right);
    assert_eq!(new.effects[0].spec.kind, cantus_core::EffectKind::Space);
    assert!(p.strip(s[0]).unwrap().send_to(new.id).is_some());
    assert_eq!(r.app.selected_strip, Some(new.id));
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.new_return"));
    undo_restores(&mut r.app, &before);
}

#[test]
fn shift_f10_opens_the_selected_strips_route_menu_and_the_strip_menu_says_so() {
    // The whole window: Shift+F10 is the keymap's *Route menu* (rebindable), acting while
    // the mixer is what is being edited.
    let mut r = Rig::window(starter_project());
    r.idle(3);
    let s = r.ids();
    r.app.select_strip(s[2]);
    r.app.focus = cantus_ui::app::EditFocus::Arrangement;
    r.idle(2);
    r.key(Key::F10, Modifiers::SHIFT);
    assert!(
        !r.idle(1).contains("NEW RETURN"),
        "not while the arrangement is being edited"
    );
    r.app.focus = cantus_ui::app::EditFocus::Mixer;
    r.key(Key::F10, Modifiers::SHIFT);
    let text = r.idle(1);
    assert!(
        text.contains("OUTPUT") && text.contains("NEW RETURN"),
        "the route menu shows its output and sends: {text}"
    );
    r.key(Key::Escape, Modifiers::NONE);
    let head = r.rect(Some(s[2]), Control::Head).center();
    r.right_click(head);
    let text = r.idle(1);
    assert!(
        text.contains("Where it goes…") && text.contains("Shift+F10"),
        "the strip menu shows the key: {text}"
    );
    // Rebound, the menus show the new key and the old one does nothing.
    use cantus_ui::keymap::{Action, Binding};
    r.key(Key::Escape, Modifiers::NONE);
    r.app
        .keymap
        .unbind(Action::RouteMenu, Binding::shift(Key::F10));
    r.app.keymap.bind(Action::RouteMenu, Binding::alt(Key::F10));
    r.right_click(head);
    let text = r.idle(1);
    // Alt is Option on a Mac's keyboard, and its menus say so (ADR 0066).
    let alt_f10 = if cfg!(target_os = "macos") {
        "Option+F10"
    } else {
        "Alt+F10"
    };
    assert!(
        text.contains(alt_f10),
        "the strip menu shows the new key: {text}"
    );
}

#[test]
fn a_channel_rack_channel_dragged_carries_its_track() {
    let mut r = Rig::window(starter_project());
    r.app.central = cantus_ui::app::CentralView::ChannelRack;
    r.idle(4);
    let track = r.app.project().tracks[0].id;
    let button = cantus_ui::views::device_chrome::spot(
        &r.ctx,
        cantus_ui::views::device_chrome::Spot::RackButton(track),
    )
    .expect("the channel button is drawn");
    let a = button.center();
    r.pointer = Some(a);
    r.frame(Vec::new());
    r.button(a, true);
    for k in 1..=4 {
        r.pointer = Some(a + vec2(0.0, 8.0 * k as f32));
        r.frame(Vec::new());
    }
    assert_eq!(
        egui::DragAndDrop::payload::<TrackDrag>(&r.ctx).map(|p| p.0),
        Some(track)
    );
    r.button(r.pointer.unwrap(), false);
}

// ---- Receives (design §4.4) ---------------------------------------------------------------

/// A return fed by strip 1's send at −6 dB, its sheet in the inspector of a tall window
/// (so the whole sheet shows without scrolling).
fn receiving() -> (Rig, StripId, StripId) {
    let mut p = starter_project();
    let ret = p.strips.iter().find(|x| x.dock == Dock::Right).unwrap().id;
    let src = p.strips[0].id;
    let mut send = Send::new(ret);
    send.level = 0.5;
    p.strips[0].sends.push(send);
    let mut r = Rig::window(p);
    r.size = vec2(1600.0, 2400.0);
    r.app.select_strip(ret);
    r.idle(4);
    (r, src, ret)
}

#[test]
fn receives_lists_the_senders_and_mixes_them_from_the_return() {
    let (mut r, src, ret) = receiving();
    let level = r.id_rect(egui::Id::new(("insp-recv-level", ret.0, src.0)));
    let before = r.app.project().clone();
    r.drag(level.center(), level.center() + vec2(0.0, -40.0));
    let now = r
        .app
        .project()
        .strip(src)
        .unwrap()
        .send_to(ret)
        .unwrap()
        .level;
    assert!(now > 0.5, "dragged up, the send is louder: {now}");
    // One gesture, one step.
    undo_restores(&mut r.app, &before);
    r.idle(2);
    // The lamp switches it, the level kept.
    let lamp = r.id_rect(egui::Id::new(("insp-recv-on", ret.0, src.0)));
    r.click(lamp.center());
    let send = *r.app.project().strip(src).unwrap().send_to(ret).unwrap();
    assert!(!send.enabled && send.level == 0.5);
}

#[test]
fn a_receives_name_selects_its_source() {
    let (mut r, src, ret) = receiving();
    let name = r.id_rect(egui::Id::new(("insp-recv-name", ret.0, src.0, 1u8)));
    r.click(name.center());
    assert_eq!(r.app.selected_strip, Some(src));
}

#[test]
fn an_empty_receives_opens_route_here() {
    let mut r = Rig::window(starter_project());
    r.size = vec2(1600.0, 2400.0);
    let s = r.ids();
    r.app.select_strip(s[5]);
    r.idle(4);
    let well = r.id_rect(egui::Id::new(("insp-recv-empty", s[5].0)));
    r.click(well.center());
    let owner = route_menu::inspector_owner();
    assert!(route_menu::is_open(&r.ctx, owner));
    let rows = route_menu::row_labels(&r.ctx, owner);
    assert!(
        rows.iter().any(|l| l == "routing.menu.route_a_track"),
        "{rows:?}"
    );
    r.click_row(owner, "routing.menu.route_a_track");
    let name = r.app.project().tracks[0].name.clone();
    r.click_row(owner, &name);
    assert_eq!(r.app.project().tracks[0].strip, Some(s[5]));
}

// ---- click mode: press on the source, click on the destination (answer 2) --------------

/// A click on a strip's real port arms a route (the port shows it, the hint says what the
/// next click does); a click on another strip's fader makes a unity post-fader send — the
/// fader under that click does not move — and lets the route go.
#[test]
fn a_click_on_a_port_then_on_a_strip_sends() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let before = r.app.project().clone();
    let gain = r.app.project().strip(s[4]).unwrap().gain_db;
    let port = r.rect(Some(s[1]), Control::Port).center();
    r.click(port);
    assert_eq!(r.app.mixer.armed(), Some(s[1]), "the click armed a route");
    // Over a strip that takes it: the words say what the click will do.
    let fader = r.rect(Some(s[4]), Control::Fader).center();
    r.pointer = Some(fader);
    r.idle(2);
    let (name, value) = r.hint();
    assert!(name.starts_with("Click: send"), "{name}");
    assert!(value.starts_with("Send to 5"), "{value}");
    r.click(fader);
    assert_eq!(r.app.mixer.armed(), None, "the route was made and let go");
    let st = r.app.project().strip(s[1]).unwrap();
    assert_eq!(st.send_to(s[4]), Some(&Send::new(s[4])), "0 dB, post-fader");
    assert_eq!(st.output, None, "the output untouched");
    assert_eq!(r.app.project().strip(s[4]).unwrap().gain_db, gain);
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.add_send"));
    undo_restores(&mut r.app, &before);
}

/// The hint bar reads whole in both languages while a route is armed or carried: the
/// modifiers ("…· Esc: cancel") are what only the bar says — the chip beside the pointer
/// already carries the destination's words — so the bar's value gives way to them, never
/// the other way round. (Round 25: in Spanish the bar cut "Esc: cancelar".)
#[test]
fn the_hint_bar_keeps_the_modifiers_whole_while_routing() {
    for locale in cantus_ui::i18n::Locale::ALL {
        let mut r = Rig::window(starter_project());
        cantus_ui::i18n::set_thread_locale(locale);
        r.idle(3);
        let s = r.ids();
        r.click(r.rect(Some(s[1]), Control::Port).center());
        assert_eq!(r.app.mixer.armed(), Some(s[1]));
        r.pointer = Some(r.rect(Some(s[5]), Control::Fader).center());
        let text = r.idle(3);
        let keys = cantus_ui::i18n::t("routing.arm.hint");
        assert!(
            text.contains(&keys),
            "{locale:?}: the hint bar cut {keys:?}"
        );
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Ctrl held at the second click: the destination becomes the only output.
#[test]
fn ctrl_at_the_second_click_routes_the_output() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    r.click(r.rect(Some(s[1]), Control::Port).center());
    r.mods = Modifiers::COMMAND;
    let fader = r.rect(Some(s[5]), Control::Fader).center();
    r.click(fader);
    r.mods = Modifiers::NONE;
    let st = r.app.project().strip(s[1]).unwrap();
    assert_eq!(st.output, Some(s[5]));
    assert!(st.sends.is_empty(), "sends untouched");
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.replace_output"));
}

/// Esc, a click on empty space, or a click on its own strip lets an armed route go; a
/// refused strip keeps it armed (the chip says why) and issues nothing.
#[test]
fn an_armed_route_is_let_go_by_esc_empty_space_or_its_own_strip() {
    let mut r = Rig::new(starter_project());
    r.idle(3);
    let s = r.ids();
    let port = r.rect(Some(s[1]), Control::Port).center();
    let before = r.app.project().clone();
    r.click(port);
    r.key(Key::Escape, Modifiers::NONE);
    assert_eq!(r.app.mixer.armed(), None, "Esc");
    r.click(port);
    // The title row: no column under the click.
    r.click(pos2(port.x, 6.0));
    assert_eq!(r.app.mixer.armed(), None, "a click on empty space");
    r.click(port);
    r.click(r.rect(Some(s[1]), Control::Fader).center());
    assert_eq!(r.app.mixer.armed(), None, "a click on its own strip");
    assert!(r.app.project() == &before, "nothing was issued");
    // Strip 1 feeds strip 2: from strip 2, strip 1 would loop.
    r.app.exec(cantus_core::Command::SetStripOutput {
        id: s[0],
        output: Some(s[1]),
    });
    r.idle(1);
    let fed = r.app.project().clone();
    r.click(port);
    let f0 = r.rect(Some(s[0]), Control::Fader).center();
    r.pointer = Some(f0);
    r.idle(2);
    r.click(f0);
    assert_eq!(
        r.app.mixer.armed(),
        Some(s[1]),
        "a refused strip keeps it armed"
    );
    assert!(r.app.project() == &fed, "a refused click issues nothing");
    assert!(r.hint().1.contains("Would make a loop"), "{:?}", r.hint());
    r.click(r.rect(Some(s[6]), Control::Fader).center());
    assert!(r.app.project().strip(s[1]).unwrap().send_to(s[6]).is_some());
}

/// Round 26 (ADR 0044 §10): a route armed by a click lets go when the desk leaves the
/// screen — its pane closed, or another tab brought over it — so it is never found still
/// armed when the desk comes back; a selected cable is kept (Delete removes it once the
/// desk shows it again). The signal is the desk itself: it was not drawn on the pass
/// before this one.
#[test]
fn an_armed_route_lets_go_when_the_desk_is_hidden_and_a_selected_cable_stays() {
    use cantus_core::routing::Route;
    use cantus_ui::views::workspace::Pane;
    let mut r = Rig::window(starter_project());
    r.idle(3);
    let s = r.ids();
    r.app.exec(cantus_core::Command::SetStripOutput {
        id: s[1],
        output: Some(s[9]),
    });
    r.app.select_strip(s[1]);
    r.idle(3);
    assert!(r.app.workspace.is_visible(Pane::Mixer), "the desk shows");
    let route = Route::output(s[1], s[9]);
    for hide in [Pane::Mixer, Pane::Routing] {
        // Armed from strip 3's port, with strip 2's output cable selected.
        r.click(r.rect(Some(s[2]), Control::Port).center());
        assert_eq!(r.app.mixer.armed(), Some(s[2]), "{hide:?}: armed");
        r.app.mixer.select_cable(Some(route));
        r.idle(1);
        assert_eq!(r.app.mixer.selected_cable(), Some(route));
        // The desk leaves the screen: its pane closed, or the Routing tab over it.
        r.app.toggle_pane(hide);
        r.idle(3);
        assert!(!r.app.workspace.is_visible(Pane::Mixer), "{hide:?}: hidden");
        // And comes back.
        if hide == Pane::Mixer {
            r.app.toggle_pane(Pane::Mixer);
        } else {
            r.app.show_pane(Pane::Mixer);
        }
        r.idle(3);
        assert!(r.app.workspace.is_visible(Pane::Mixer), "{hide:?}: back");
        assert_eq!(
            r.app.mixer.armed(),
            None,
            "{hide:?}: the armed route let go"
        );
        assert_eq!(
            r.app.mixer.selected_cable(),
            Some(route),
            "{hide:?}: the selected cable is kept"
        );
    }
    // While the desk stays on screen nothing lets the route go by itself.
    r.click(r.rect(Some(s[2]), Control::Port).center());
    r.idle(5);
    assert_eq!(
        r.app.mixer.armed(),
        Some(s[2]),
        "still armed on a shown desk"
    );
}

/// The selected strip's ▼ chip arms the same way (right-click still lists every
/// destination), and a click on a strip's mute while armed routes instead of muting.
#[test]
fn the_selected_strips_chip_arms_a_route_too() {
    let mut r = Rig::new(starter_project());
    let s = r.ids();
    r.app.select_strip(s[2]);
    r.idle(3);
    let chip = r.rect(Some(s[2]), Control::Send).center();
    r.click(chip);
    assert_eq!(r.app.mixer.armed(), Some(s[2]));
    assert!(
        !route_menu::is_open(&r.ctx, desk_owner()),
        "no menu on a click"
    );
    let mute = r.rect(Some(s[6]), Control::Mute).center();
    r.click(mute);
    let p = r.app.project();
    assert!(p.strip(s[2]).unwrap().send_to(s[6]).is_some(), "routed");
    assert!(
        !p.strip(s[6]).unwrap().muted,
        "the mute under the click did not act"
    );
    r.right_click(chip);
    assert_eq!(r.app.mixer.armed(), None);
}

/// Integration finding: a separator stood for its group's first member in project order —
/// wrong for a group split across docks. A strip dropped just before the right dock's
/// separator of a group whose first member is in the middle lands in the right dock,
/// before that run's first member.
#[test]
fn a_drop_before_a_separator_lands_before_that_runs_first_member() {
    let mut r = Rig::new(starter_project());
    let s = r.ids();
    let right: Vec<StripId> = r
        .app
        .project()
        .strips
        .iter()
        .filter(|x| x.dock == Dock::Right)
        .map(|x| x.id)
        .collect();
    assert!(
        right.len() >= 2,
        "the starter desk has returns on the right"
    );
    let gid = r.app.create_strip_group(&[s[2], s[3]]).expect("a group");
    r.app.exec(cantus_core::Command::SetStripGroup {
        id: right[1],
        group: Some(gid),
    });
    r.idle(3);
    // The right dock's separator sits between right[0] and right[1].
    let a = r.rect(Some(right[0]), Control::Head).right_center();
    let b = r.rect(Some(right[1]), Control::Head).left_center();
    assert!(b.x - a.x > 4.0, "a separator between them");
    let to = pos2(a.x + 2.0, a.y);
    let from = r.rect(Some(s[6]), Control::Head).center();
    r.drag(from, to);
    let p = r.app.project();
    assert_eq!(
        p.strip(s[6]).unwrap().dock,
        Dock::Right,
        "it went to the right dock"
    );
    let now: Vec<StripId> = p
        .strips
        .iter()
        .filter(|x| x.dock == Dock::Right)
        .map(|x| x.id)
        .collect();
    let at = now.iter().position(|x| *x == s[6]).unwrap();
    assert_eq!(
        now.get(at + 1),
        Some(&right[1]),
        "before the run's first member"
    );
    assert_eq!(p.strip(s[6]).unwrap().group, None);
}

/// Integration finding: a view drawn inside a tab toggled the dock while it was checked
/// out, so Extra large's chain line never opened a hidden inspector. The request now waits
/// for the dock (`CantusApp::show_pane`).
#[test]
fn the_chain_line_opens_a_hidden_inspector() {
    use cantus_ui::views::workspace::Pane;
    let mut r = Rig::window(starter_project());
    mixer::set_width_mode(&mut r.app, mixer::layout::WidthMode::Xl);
    r.idle(3);
    if r.app.workspace.is_visible(Pane::Inspector) {
        r.app.toggle_pane(Pane::Inspector);
    }
    r.idle(3);
    assert!(!r.app.workspace.is_visible(Pane::Inspector));
    let s = r.ids();
    let chain = r.rect(Some(s[2]), Control::Chain).center();
    r.click(chain);
    assert!(
        r.app.workspace.is_visible(Pane::Inspector),
        "the inspector opened"
    );
    assert_eq!(r.app.selected_strip, Some(s[2]));
}

/// Integration finding: the master's pan law stood as a bare "-3" under OUTPUT. It reads
/// "−3 dB law" from Wide up (its short form where only that fits), in both languages.
#[test]
fn the_masters_pan_law_is_labelled() {
    for (locale, word) in [
        (cantus_ui::i18n::Locale::En, "dB law"),
        (cantus_ui::i18n::Locale::Es, "ley "),
    ] {
        let mut r = Rig::new(starter_project());
        cantus_ui::i18n::set_thread_locale(locale);
        mixer::set_width_mode(&mut r.app, mixer::layout::WidthMode::Wide);
        let text = r.idle(3);
        let law = r.app.project().pan_law.id().replace('-', "\u{2212}");
        assert!(
            text.contains(word) && text.contains(&law),
            "{locale:?}: {text}"
        );
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Whole window (design test 11 with answer 11): an arrangement header dragged onto a
/// mixer strip routes its track there; the arrangement shows no drop line while the
/// track is held over the desk, and the track order is unchanged.
#[test]
fn an_arrangement_header_dropped_on_a_strip_routes_it_and_keeps_the_order() {
    let mut r = Rig::window(cantus_ui::welcome_project());
    r.idle(4);
    let order: Vec<_> = r.app.project().tracks.iter().map(|t| t.id).collect();
    assert!(order.len() >= 3, "several tracks, so a move would show");
    let track = order[1];
    let s = r.ids();
    let chip = cantus_ui::views::device_chrome::spot(
        &r.ctx,
        cantus_ui::views::device_chrome::Spot::Chip(track),
    )
    .expect("the header is drawn");
    let a = chip.right_center() + vec2(40.0, 0.0);
    let z = r.rect(Some(s[6]), Control::Fader).center();
    r.pointer = Some(a);
    r.frame(Vec::new());
    r.button(a, true);
    for k in 1..=8 {
        r.pointer = Some(a + (z - a) * (k as f32 / 8.0));
        r.frame(Vec::new());
    }
    r.frame(Vec::new());
    assert_eq!(
        r.app.arrangement.reorder_target(),
        Some((track, 1)),
        "held over the desk: no drop line in the arrangement"
    );
    r.button(z, false);
    r.idle(2);
    let p = r.app.project();
    assert_eq!(p.track(track).unwrap().strip, Some(s[6]), "routed");
    let now: Vec<_> = p.tracks.iter().map(|t| t.id).collect();
    assert_eq!(now, order, "the track order is unchanged");
}

/// Integration finding: the Receives key row read "2 · Compressor ← key", cut in its
/// middle on a narrow sheet. It reads like the chain line — "Comp, slot 1 ← key" — and a
/// narrow sheet cuts its end.
#[test]
fn a_receives_key_row_names_the_insert_as_the_chain_line_does() {
    let (mut r, src, ret) = receiving();
    let comp = r.app.duck_strip_from(ret, src).expect("a keyed compressor");
    let _ = comp;
    let text = r.idle(3);
    assert!(text.contains("Comp, slot 1 \u{2190} key"), "{text}");
}

// ---- round 26: typing a send's level in the route menu ------------------------------

/// Round 26 (ADR 0044 §10, amended): *Level ▸ Type a level…* turns the route menu into a
/// field — the kit's numeric entry, prefilled with the level and selected so typing
/// replaces it. Enter sets it in one undo step; a Spanish decimal comma reads (the kit's
/// parser accepts it everywhere); above 0 dB clamps to 0 dB; −∞ (or anything at or below
/// the floor) is silence; words that are not a level keep the field open and change
/// nothing; Esc cancels.
#[test]
fn a_level_typed_in_the_route_menu_sets_the_send_in_one_step() {
    use cantus_core::routing::Route;
    let mut r = Rig::new(starter_project());
    let s = r.ids();
    r.app.set_strip_send(
        s[1],
        Send {
            level: 0.5,
            ..Send::new(s[5])
        },
    );
    r.app.end_gesture();
    r.idle(2);
    let route = Route::send(s[1], s[5]);
    let level = |r: &Rig| {
        r.app
            .project()
            .strip(s[1])
            .and_then(|x| x.send_to(s[5]))
            .unwrap()
            .level
    };
    let open_field = |r: &mut Rig| {
        route_menu::open(&r.ctx, desk_owner(), route, pos2(500.0, 300.0));
        r.idle(2);
        let row = route_menu::row_rect(&r.ctx, desk_owner(), "routing.menu.level").unwrap();
        r.pointer = Some(row.center());
        r.idle(3);
        r.click_row(desk_owner(), "routing.menu.level_type");
        r.idle(1);
    };
    let typed = |r: &Rig| route_menu::typed_level(&r.ctx, desk_owner());
    let type_text = |r: &mut Rig, text: &str| {
        r.frame(vec![Event::Text(text.into())]);
        r.idle(1);
    };
    // Prefilled with the level now, selected: what is typed replaces it.
    open_field(&mut r);
    let now = typed(&r).expect("the field is open");
    assert!(now.contains("6.0"), "prefilled with −6.0 dB: {now:?}");
    let before = r.app.project().clone();
    type_text(&mut r, "-4,5");
    assert_eq!(
        typed(&r).as_deref(),
        Some("-4,5"),
        "typing replaced the value"
    );
    r.key(Key::Enter, Modifiers::NONE);
    assert!(typed(&r).is_none(), "Enter closed the field");
    assert!(!route_menu::is_open(&r.ctx, desk_owner()));
    let db = cantus_core::linear_to_db(level(&r));
    assert!((db + 4.5).abs() < 0.01, "−4,5 read as −4.5 dB: {db}");
    r.app.undo();
    assert!(r.app.project() == &before, "one undo step");
    // Above 0 dB: clamped to unity.
    open_field(&mut r);
    type_text(&mut r, "+6 dB");
    r.key(Key::Enter, Modifiers::NONE);
    assert_eq!(level(&r), 1.0, "clamped to 0 dB");
    // −∞ and the floor: silence.
    for words in ["-inf", "\u{2212}\u{221e}", "-120"] {
        open_field(&mut r);
        type_text(&mut r, words);
        r.key(Key::Enter, Modifiers::NONE);
        assert_eq!(level(&r), 0.0, "{words:?} is silence");
        r.app.undo();
    }
    // Not a level: the field stays, nothing changes; Esc cancels.
    let kept = level(&r);
    open_field(&mut r);
    type_text(&mut r, "loud");
    r.key(Key::Enter, Modifiers::NONE);
    assert!(
        typed(&r).is_some(),
        "still open after words that are not a level"
    );
    assert_eq!(level(&r), kept);
    r.key(Key::Escape, Modifiers::NONE);
    assert!(typed(&r).is_none() && !route_menu::is_open(&r.ctx, desk_owner()));
    open_field(&mut r);
    type_text(&mut r, "-12");
    r.key(Key::Escape, Modifiers::NONE);
    assert_eq!(level(&r), kept, "Esc cancels");
    // Review finding 5: Enter on the prefill, untouched, changes nothing and adds no step
    // — even for a level the prefill's one decimal cannot say exactly (−10.457 dB), or
    // one below the faders' floor.
    for odd in [0.3f32, 1e-5] {
        r.app.set_strip_send(
            s[1],
            Send {
                level: odd,
                ..Send::new(s[5])
            },
        );
        r.app.end_gesture();
        r.idle(1);
        let before = r.app.project().clone();
        let steps = r.app.bus.revision();
        open_field(&mut r);
        r.key(Key::Enter, Modifiers::NONE);
        assert!(typed(&r).is_none(), "{odd}: Enter closed the field");
        assert!(r.app.project() == &before, "{odd}: the level is untouched");
        assert_eq!(
            r.app.bus.revision(),
            steps,
            "{odd}: no command, no undo step"
        );
    }
}

/// Review finding 3, under the app-wide field rules (third review): while the level field
/// is open the app's shortcuts never act (Ctrl+Z here, which would have removed the send
/// being edited); typed words land in the field; Esc closes it with nothing changed; and a
/// click anywhere off the text box — the panel's own band included — commits and closes
/// it, letting the keyboard go. The whole window, as a musician has it.
#[test]
fn the_level_field_keeps_the_keyboard_while_it_is_open() {
    let mut r = Rig::window(starter_project());
    r.idle(3);
    let s = r.ids();
    r.app.set_strip_send(
        s[1],
        Send {
            level: 0.5,
            ..Send::new(s[5])
        },
    );
    r.app.end_gesture();
    r.idle(2);
    let at = pos2(500.0, 300.0);
    let app = &r.app;
    route_menu::open_level(&r.ctx, desk_owner(), app, s[1], s[5], at);
    r.idle(3);
    assert!(route_menu::typed_level(&r.ctx, desk_owner()).is_some());
    let panel = route_menu::level_panel(&r.ctx, desk_owner()).expect("the panel was drawn");
    r.key(Key::Z, Modifiers::COMMAND);
    assert!(
        r.app.project().strip(s[1]).unwrap().send_to(s[5]).is_some(),
        "Ctrl+Z reached the app's undo while the field was open"
    );
    assert!(route_menu::typed_level(&r.ctx, desk_owner()).is_some());
    // Typed words still land in the field, and Esc closes it with nothing changed.
    r.frame(vec![Event::Text("9".into())]);
    r.idle(1);
    let now = route_menu::typed_level(&r.ctx, desk_owner()).unwrap();
    assert!(now.contains('9'), "typing reaches the field: {now:?}");
    r.key(Key::Escape, Modifiers::NONE);
    assert!(route_menu::typed_level(&r.ctx, desk_owner()).is_none());
    let l = r
        .app
        .project()
        .strip(s[1])
        .unwrap()
        .send_to(s[5])
        .unwrap()
        .level;
    assert_eq!(l, 0.5, "nothing changed");
    // Open again: a click on the panel's own band, off the text box, commits (the
    // prefill untouched: nothing set) and closes, and the keyboard is free.
    let app = &r.app;
    route_menu::open_level(&r.ctx, desk_owner(), app, s[1], s[5], at);
    r.idle(3);
    r.click(pos2(panel.left() + 20.0, panel.top() + 10.0));
    assert!(
        route_menu::typed_level(&r.ctx, desk_owner()).is_none(),
        "closed"
    );
    assert!(!r.ctx.egui_wants_keyboard_input(), "the keyboard let go");
}

/// Round 26: the inspector's *Add send* — the empty Sends row and the band's + — opens
/// the desk's own add-send list (`route_menu::open_add_send`, one code path): *New return
/// ▸* first, then every strip by dock; a reverb return from there is one undo step.
#[test]
fn the_inspectors_add_send_offers_a_new_return() {
    let mut r = Rig::window(starter_project());
    r.size = vec2(1600.0, 2400.0);
    let s = r.ids();
    r.app.select_strip(s[0]);
    r.idle(4);
    let owner = route_menu::inspector_owner();
    // No sends yet: the empty row opens the list.
    let empty = r.id_rect(egui::Id::new("insp-send-empty"));
    r.click(empty.center());
    assert!(
        route_menu::is_open(&r.ctx, owner),
        "the route menu's add-send list"
    );
    let rows = route_menu::row_labels(&r.ctx, owner);
    assert_eq!(
        rows.first().map(String::as_str),
        Some("routing.menu.new_return"),
        "New return first: {rows:?}"
    );
    let before = r.app.project().clone();
    r.click_row(owner, "routing.menu.new_return");
    r.click_row(owner, "routing.menu.return_reverb");
    let p = r.app.project();
    let new = p.strips.last().unwrap();
    assert_eq!(new.dock, Dock::Right);
    assert_eq!(new.effects[0].spec.kind, cantus_core::EffectKind::Space);
    assert!(
        p.strip(s[0]).unwrap().send_to(new.id).is_some(),
        "it sends there"
    );
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.new_return"));
    undo_restores(&mut r.app, &before);
    // With a send already, the band's + opens the same list.
    r.app.set_strip_send(s[0], Send::new(s[5]));
    r.app.end_gesture();
    r.app.select_strip(s[0]);
    r.idle(4);
    let plus = r.id_rect(egui::Id::new(("insp-band-plus", "kit.inspector.sends")));
    r.click(plus.center());
    assert!(route_menu::is_open(&r.ctx, owner));
    let rows = route_menu::row_labels(&r.ctx, owner);
    assert_eq!(
        rows.first().map(String::as_str),
        Some("routing.menu.new_return")
    );
    // A strip from the list adds the send.
    let label = route_menu::row_labels(&r.ctx, owner)
        .into_iter()
        .find(|l| l.starts_with("8 "))
        .unwrap_or_else(|| "8".to_string());
    r.click_row(owner, &label);
    assert!(r.app.project().strip(s[0]).unwrap().send_to(s[7]).is_some());
}

/// Round 26 (ADR 0043, amended): the cable legend is never invisible. At 1280 × 720 the
/// band sits at its least (20 pt) and the three sample lines with their words do not fit
/// under ROUTING; a compact key does — the three samples (output thick, send thin, key
/// dashed in the dynamics hue) beside the row's name, inside its cell, touching neither
/// the name nor each other. The whole window, English and Spanish, 100 % and 200 %.
#[test]
fn the_cable_legend_shows_when_the_band_is_at_its_least() {
    use mixer::layout::WidthMode;
    for (locale, ppp, mode) in cantus_ui::i18n::Locale::ALL
        .into_iter()
        .flat_map(|l| [1.0f32, 2.0].map(|p| (l, p)))
        .flat_map(|(l, p)| [None, Some(WidthMode::Wide)].map(|m| (l, p, m)))
    {
        {
            let mut r = Rig::window(starter_project());
            cantus_ui::i18n::set_thread_locale(locale);
            r.ctx.set_pixels_per_point(ppp);
            r.size = vec2(1280.0, 720.0) / ppp;
            // The default width (Compact) and Wide.
            if let Some(m) = mode {
                mixer::set_width_mode(&mut r.app, m);
            }
            let s = r.ids();
            r.app.select_strip(s[1]);
            r.idle(4);
            let key = r
                .app
                .mixer
                .cable_key()
                .unwrap_or_else(|| panic!("{locale:?} {ppp}: the band's row is on the desk"));
            // Review finding 11: on screen — inside the desk's visible rows and the window.
            let rows = r.app.mixer.rows_seen();
            let window = egui::Rect::from_min_size(Pos2::ZERO, r.size);
            assert!(
                rows.expand(0.5).contains_rect(key.cell) && window.contains_rect(key.cell),
                "{locale:?} {ppp} {mode:?}: the band's cell {:?} is off the desk's view {rows:?}",
                key.cell
            );
            let marks = key.marks.expect("the legend's samples are drawn");
            assert!(
                key.compact || key.cell.height() >= 40.0,
                "{locale:?} {ppp} {mode:?}: a short band ({:?}) has the compact key",
                key.cell
            );
            // Review finding 12: the compact key is a row of three samples, never a stack
            // that reads as a ≡ menu glyph.
            if key.compact {
                assert!(
                    marks[0].right() < marks[1].left() && marks[1].right() < marks[2].left(),
                    "{locale:?} {ppp} {mode:?}: the samples side by side: {marks:?}"
                );
            }
            // Review finding 13: ROUTING where every row's name starts.
            assert!(
                (key.name.left() - (key.cell.left() + 8.0)).abs() < 1.0,
                "{locale:?} {ppp} {mode:?}: the name {:?} not aligned with the column",
                key.name
            );
            for (i, m) in marks.iter().enumerate() {
                assert!(
                    key.cell.contains_rect(*m),
                    "{locale:?} {ppp}: sample {i} {m:?} outside its cell {:?}",
                    key.cell
                );
                assert!(
                    !m.intersects(key.name),
                    "{locale:?} {ppp}: sample {i} {m:?} over the name {:?}",
                    key.name
                );
                for o in &marks[i + 1..] {
                    assert!(!m.intersects(*o), "{locale:?} {ppp}: samples overlap");
                }
                assert!(
                    !m.intersects(key.handle),
                    "{locale:?} {ppp}: sample {i} {m:?} against the band's handle"
                );
            }
            assert!(key.cell.contains_rect(key.name), "the name in its cell");
            assert!(
                !key.name.intersects(key.handle),
                "{locale:?} {ppp}: the name {:?} under the band's handle {:?}",
                key.name,
                key.handle
            );
            let words = cantus_ui::i18n::t("cables.legend").to_uppercase();
            let text = r.idle(1);
            assert!(
                text.contains(&format!("| {words} |")),
                "{locale:?} {ppp}: ROUTING whole ({words})"
            );
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Review findings 1 and 2: whatever the desk was carrying when it left the screen lets
/// go — a cable dragged from a port, a press on a port that had not travelled yet, a strip
/// carried by its number cell. Released while the desk is hidden (or after it is back), it
/// makes nothing: no route, no move, no command.
#[test]
fn what_the_desk_carried_lets_go_when_it_is_hidden() {
    use cantus_ui::views::workspace::Pane;
    for case in ["cable", "press", "strip"] {
        let mut r = Rig::window(starter_project());
        r.idle(3);
        let s = r.ids();
        r.app.select_strip(s[1]);
        r.idle(2);
        let rev = r.app.bus.revision();
        let order = r.order();
        let from = match case {
            "strip" => r.rect(Some(s[2]), Control::Head).center(),
            _ => r.rect(Some(s[2]), Control::Port).center(),
        };
        let to = r.rect(Some(s[6]), Control::Fader).center();
        r.pointer = Some(from);
        r.idle(1);
        r.button(from, true);
        if case != "press" {
            // Carried: past the 6-pt threshold, over strip 7.
            for k in 1..=6 {
                r.pointer = Some(from + (to - from) * (k as f32 / 6.0));
                r.idle(1);
            }
        }
        // The desk leaves the screen with the button still held.
        r.app.toggle_pane(Pane::Routing);
        r.idle(2);
        assert!(!r.app.workspace.is_visible(Pane::Mixer), "{case}: hidden");
        r.pointer = Some(to);
        r.idle(2);
        r.app.show_pane(Pane::Mixer);
        r.idle(3);
        assert!(r.app.workspace.is_visible(Pane::Mixer), "{case}: back");
        r.button(to, false);
        r.idle(3);
        assert_eq!(
            r.app.bus.revision(),
            rev,
            "{case}: a release after the desk was hidden made something: {:?}",
            r.app.bus.undo_label_key()
        );
        assert_eq!(r.order(), order, "{case}: no strip moved");
        assert!(
            r.app.project().strip(s[2]).unwrap().sends.is_empty(),
            "{case}: no send"
        );
        assert_eq!(r.app.mixer.armed(), None, "{case}: nothing armed");
    }
}

// ---- round 26, second review: the pinned band (findings 7–11) -------------------------

/// The whole window at 1280 × 720 and 200 %, Wide, strip 2 selected: the desk scrolls and
/// the band is pinned to its foot.
fn pinned_rig() -> Rig {
    let mut r = Rig::window(starter_project());
    r.ctx.set_pixels_per_point(2.0);
    r.size = vec2(640.0, 360.0);
    mixer::set_width_mode(&mut r.app, mixer::layout::WidthMode::Wide);
    let s = r.ids();
    r.app.select_strip(s[1]);
    r.idle(4);
    r
}

fn wheel(r: &mut Rig, at: Pos2, dy: f32, modifiers: Modifiers) {
    r.pointer = Some(at);
    r.mods = modifiers;
    r.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, dy),
        phase: egui::TouchPhase::Move,
        modifiers,
    }]);
    for _ in 0..20 {
        r.frame(Vec::new());
    }
    r.mods = Modifiers::NONE;
    r.idle(1);
}

/// Second review, findings 7 and 10 (and the third's 6): nothing hidden under the pinned
/// band is grabbed through it (the meter/fader ratio handle), and the band's own height
/// handle, while the desk holds the band at less than asked, grows it no further — a drag
/// up writes no preference above what shows, and the band does not move.
#[test]
fn the_pinned_bands_legend_cell_grabs_nothing_hidden() {
    let mut r = pinned_rig();
    let key = r.app.mixer.cable_key().expect("the band's cell");
    let before = r.app.mixer.prefs();
    for p in [
        pos2(key.handle.center().x, key.cell.bottom() - 3.0),
        key.handle.center(),
    ] {
        r.drag(p, p + vec2(0.0, -15.0));
        let now = r.app.mixer.prefs();
        assert_eq!(
            now.ratio, before.ratio,
            "the hidden ratio handle moved ({p:?})"
        );
        assert!(
            now.band == before.band || now.band <= key.cell.height() + 0.5,
            "a preference above what shows ({p:?}): {}",
            now.band
        );
        let k = r.app.mixer.cable_key().unwrap();
        assert_eq!(k.cell, key.cell, "the band's cell moved");
    }
}

/// Second review, finding 9: the pinned band stops at the vertical scrollbar — a click on
/// the bar's track beside the band pages the desk.
#[test]
fn the_scrollbar_track_beside_a_pinned_band_pages() {
    let mut r = pinned_rig();
    let key = r.app.mixer.cable_key().unwrap();
    let rows = r.app.mixer.rows_seen();
    assert_eq!(r.app.mixer.scroll().1, 0.0);
    r.click(pos2(rows.right() - 5.0, key.cell.center().y + 2.0));
    assert!(
        r.app.mixer.scroll().1 > 0.0,
        "the track's click paged the desk"
    );
}

/// Second review, finding 8: the wheel over the pinned band does what it does over the
/// band at the desk's foot — sideways, never down — and Ctrl+wheel steps the width.
#[test]
fn the_wheel_over_a_pinned_band_acts_as_over_the_band() {
    let mut r = pinned_rig();
    // Down a page first (the scrollbar's track), so the band is pinned with the desk
    // scrolled.
    let rows = r.app.mixer.rows_seen();
    r.click(pos2(rows.right() - 5.0, rows.top() + rows.height() * 0.6));
    let (x0, y0) = r.app.mixer.scroll();
    assert!(y0 > 0.0, "the desk scrolled down");
    let key = r.app.mixer.cable_key().unwrap();
    let on_band = pos2(key.cell.right() + 70.0, key.cell.bottom() - 2.0);
    wheel(&mut r, on_band, -2.0, Modifiers::NONE);
    let (x1, y1) = r.app.mixer.scroll();
    assert_eq!(y1, y0, "the wheel over the band scrolled the desk down");
    assert!(x1 > x0, "the wheel over the band scrolls sideways");
    let mode = r.app.mixer.prefs().width;
    wheel(&mut r, on_band, 1.0, Modifiers::COMMAND);
    assert_ne!(
        r.app.mixer.prefs().width,
        mode,
        "Ctrl+wheel over the band steps the width"
    );
}

/// Second review, finding 11: Esc that closes the typed-level field keeps the cable the
/// right-click selected.
#[test]
fn esc_on_the_level_field_keeps_the_selected_cable() {
    use cantus_core::routing::Route;
    let mut r = Rig::window(starter_project());
    r.idle(3);
    let s = r.ids();
    r.app.set_strip_send(s[1], Send::new(s[5]));
    r.app.end_gesture();
    r.app.select_strip(s[1]);
    r.idle(2);
    let route = Route::send(s[1], s[5]);
    r.app.mixer.select_cable(Some(route));
    let app = &r.app;
    route_menu::open_level(&r.ctx, desk_owner(), app, s[1], s[5], pos2(500.0, 300.0));
    r.idle(3);
    r.key(Key::Escape, Modifiers::NONE);
    assert!(
        route_menu::typed_level(&r.ctx, desk_owner()).is_none(),
        "Esc closed the field"
    );
    assert_eq!(
        r.app.mixer.selected_cable(),
        Some(route),
        "the cable stays selected"
    );
}

/// Review 2: a send's level is a parameter — its route menu (a send knob's right-click)
/// offers its automation clip and *Modulate…*.
#[test]
fn a_sends_menu_automates_and_modulates_its_level() {
    let mut r = Rig::new(starter_project());
    let s = r.ids();
    let ret = r
        .app
        .project()
        .strips
        .iter()
        .find(|x| x.dock == Dock::Right)
        .unwrap()
        .id;
    assert!(r.app.add_sends(&[s[1]], ret));
    r.app.select_strip(s[1]);
    r.idle(3);
    let target = cantus_core::AutoTarget::Send {
        strip: s[1],
        to: ret,
    };
    let knob = r.rect(Some(ret), Control::Send).center();
    r.right_click(knob);
    r.click_row(desk_owner(), "mods.menu.send");
    assert_eq!(
        r.app.modulators.popup.as_ref().map(|p| p.0.clone()),
        Some(target.clone()),
        "the Modulate… list for the send"
    );
    r.app.modulators.popup = None;
    r.idle(2);
    r.right_click(knob);
    r.click_row(desk_owner(), "auto.send.level");
    assert!(
        r.app.project().tracks.iter().any(|t| t
            .clips
            .iter()
            .any(|c| c.automation.as_ref().is_some_and(|a| a.target == target))),
        "an automation clip on the send's level"
    );
}

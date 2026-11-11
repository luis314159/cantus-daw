//! The mixer desk (ADR 0024 §6), driven the way a user drives it: frames with pointer,
//! wheel and key events, and what the desk, the hint bar and the project say afterwards.

use cantus_core::{Command, Project, Send, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::kit::hint;
use cantus_ui::views::mixer::{self, layout::WidthMode, Control};
use cantus_ui::{starter_project, views, welcome_project, CantusApp};
use egui::{pos2, vec2, Event, Modifiers, MouseWheelUnit, Pos2, TouchPhase};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    cantus_ui::views::mixer::with_cables(CantusApp::from_parts(
        Some(project),
        EngineHandle::dummy(),
        false,
    ))
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

/// The welcome project grown to `n` strips.
fn grown(n: usize) -> Project {
    let mut p = welcome_project();
    while p.strips.len() < n {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    p
}

/// A frame counter per context, so egui's clock moves.
struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    /// The keys held, as eframe reports them with every frame.
    mods: Modifiers,
}

impl Rig {
    fn new(project: Project, size: egui::Vec2) -> Rig {
        Rig {
            ctx: ctx(),
            app: app(project),
            size,
            frame: 0,
            pointer: None,
            mods: Modifiers::NONE,
        }
    }

    /// One frame of the mixer alone with `events`; returns the painted text.
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
        let mut out = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
        });
        out.textures_delta.clear();
        painted_text(&out.shapes)
    }

    /// One idle frame; every painted text with where it lies.
    fn texts(&mut self) -> Vec<(String, egui::Rect)> {
        self.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
        });
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut v = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut v);
        }
        v
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
            .unwrap_or_else(|| panic!("{c:?} is not on the desk"))
    }

    /// Move the pointer to `p` (and let a frame see it).
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
        self.frame(Vec::new());
    }

    /// One wheel notch at the pointer (`dy` lines; positive scrolls up).
    fn wheel(&mut self, dy: f32, modifiers: Modifiers) {
        self.mods = modifiers;
        self.frame(vec![Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, dy),
            phase: TouchPhase::Move,
            modifiers,
        }]);
        // Let egui's smoothing hand the rest of the notch over.
        for _ in 0..20 {
            self.frame(Vec::new());
        }
        self.mods = Modifiers::NONE;
        self.frame(Vec::new());
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

fn hint_now(ctx: &egui::Context) -> (String, String) {
    hint::current(ctx)
        .map(|h| (h.name, h.value))
        .unwrap_or_default()
}

/// Round 39 (des-mixer): no label column — the strips carry their values and the desk
/// names nothing forty times; no "−∞" anywhere at rest; the master first, every strip
/// numbered.
#[test]
fn the_desk_has_no_label_column_and_no_minus_infinity() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 900.0));
    let text = r.idle(2);
    for word in [
        "NUMBER",
        "NAME",
        "LEVEL",
        "VOLUME",
        "FROM SELECTED",
        "ROUTING",
    ] {
        assert!(!text.contains(&format!("| {word} |")), "{word}: {text}");
    }
    assert!(
        !text.contains('\u{221e}'),
        "no minus infinity at rest: {text}"
    );
    let m = text.find("| Master |").expect("the master");
    let one = text.find("| 1 |").expect("strip 1");
    assert!(m < one, "the master is at the left: {text}");
    assert!(text.contains("| 16 |"), "sixteen strips: {text}");
}

/// At 720 px of window the desk fits first (dropping rows) and scrolls when even that is
/// too tall: the sends row is always reachable.
#[test]
fn a_short_desk_fits_first_then_scrolls() {
    let mut r = Rig::new(starter_project(), vec2(1280.0, 330.0));
    r.idle(2);
    assert!(
        mixer::control_rect(&r.ctx, None, Control::Width).is_none(),
        "the width knob gives way first"
    );
    let fader = r.rect(None, Control::Fader);
    r.hover(fader.center());
    r.wheel(-3.0, Modifiers::NONE);
    let (_, y) = r.app.mixer.scroll();
    assert!(y > 0.0, "the wheel over the faders scrolls the desk down");
    // The master's gain did not move: a plain wheel never changes a fader.
    assert_eq!(r.app.project().master.gain_db, 0.0);
    // Scrolled far enough, the foot of the strips shows.
    for _ in 0..6 {
        r.wheel(-3.0, Modifiers::NONE);
    }
    let s1 = r.app.project().strips[0].id;
    r.idle(1);
    let fader = r.rect(Some(s1), Control::Fader);
    assert!(
        fader.bottom() <= 330.0,
        "the fader's foot is reachable: {fader:?}"
    );
}

/// A tall desk shows every row and needs no scrolling.
#[test]
fn a_tall_desk_shows_every_row() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    // Polarity and width are Wide's and Large's rows (des-mixer).
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    r.idle(2);
    for c in [Control::Width, Control::Phase, Control::Eq(0), Control::Fx] {
        assert!(
            mixer::control_rect(&r.ctx, None, c).is_some() || c != Control::Width,
            "{c:?}"
        );
    }
    let s1 = r.app.project().strips[0].id;
    for c in [
        Control::Pan,
        Control::Fader,
        Control::Width,
        Control::Phase,
        Control::Mute,
    ] {
        assert!(mixer::control_rect(&r.ctx, Some(s1), c).is_some(), "{c:?}");
    }
    assert_eq!(r.app.mixer.scroll().1, 0.0);
}

/// Only the strips on screen are laid out (ADR 0024 §13), and the plain wheel over the
/// names scrolls the strips sideways.
#[test]
fn only_visible_strips_are_laid_out_and_the_wheel_scrolls_sideways() {
    let mut r = Rig::new(grown(64), vec2(1000.0, 800.0));
    r.idle(2);
    let laid = r.app.mixer.strips_laid_out();
    assert!(laid < 64, "{laid} strips laid out");
    assert!(laid >= 20, "{laid} strips laid out");
    let s1 = r.app.project().strips[0].id;
    let head = r.rect(Some(s1), Control::Head);
    r.hover(head.center());
    r.wheel(-3.0, Modifiers::NONE);
    let (x, y) = r.app.mixer.scroll();
    assert!(x > 0.0, "the wheel over the names scrolls sideways");
    assert_eq!(y, 0.0);
}

/// Shift+wheel moves the strip under the pointer along the desk.
#[test]
fn shift_wheel_reorders_the_strip_under_the_pointer() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 900.0));
    r.idle(2);
    let s3 = r.app.project().strips[2].id;
    let head = r.rect(Some(s3), Control::Head);
    r.hover(head.center());
    r.wheel(-1.0, Modifiers::SHIFT);
    assert_eq!(r.app.project().strip_number(s3), Some(4), "moved right");
    r.hover(r.rect(Some(s3), Control::Head).center());
    r.wheel(1.0, Modifiers::SHIFT);
    assert_eq!(r.app.project().strip_number(s3), Some(3), "and back");
}

/// Ctrl+wheel over a fader moves it; elsewhere it steps the width mode; the plain wheel
/// never moves a fader.
#[test]
fn ctrl_wheel_moves_a_fader_or_steps_the_width() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 900.0));
    r.idle(2);
    let s1 = r.app.project().strips[0].id;
    let gain = |r: &Rig| r.app.project().strip(s1).unwrap().gain_db;
    let fader = r.rect(Some(s1), Control::Fader);
    r.hover(fader.center());
    r.wheel(1.0, Modifiers::NONE);
    assert_eq!(gain(&r), 0.0, "a plain wheel leaves the fader alone");
    r.wheel(1.0, Modifiers::COMMAND);
    assert!(gain(&r) > 0.5, "Ctrl+wheel moves it: {}", gain(&r));
    assert_eq!(r.app.mixer.prefs().width, WidthMode::Compact);
    let head = r.rect(Some(s1), Control::Head);
    r.hover(head.center());
    r.wheel(1.0, Modifiers::COMMAND);
    assert_eq!(r.app.mixer.prefs().width, WidthMode::Wide);
    r.wheel(1.0, Modifiers::COMMAND);
    assert_eq!(r.app.mixer.prefs().width, WidthMode::Xl);
    r.wheel(-1.0, Modifiers::COMMAND);
    assert_eq!(r.app.mixer.prefs().width, WidthMode::Wide);
}

/// The hint bar names what the pointer is on, with its value: pan, width, polarity, the
/// fader in dB.
#[test]
fn the_hint_bar_reads_the_strip() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    // Width is Wide's and Large's (des-mixer).
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let id = r.app.project().tracks[0].strip.unwrap();
    r.app.exec(Command::SetStripPan { id, pan: -0.5 });
    r.app.exec(Command::SetStripWidth { id, width: 0.0 });
    r.app.exec(Command::SetStripGain { id, gain_db: -6.0 });
    r.idle(2);
    r.hover(r.rect(Some(id), Control::Pan).center());
    assert_eq!(hint_now(&r.ctx), ("Pan".into(), "L50".into()));
    r.hover(r.rect(Some(id), Control::Width).center());
    assert_eq!(hint_now(&r.ctx), ("Width".into(), "0 %".into()));
    r.hover(r.rect(Some(id), Control::Fader).center());
    assert_eq!(hint_now(&r.ctx).1, "\u{2212}6.0 dB");
    r.hover(r.rect(Some(id), Control::Phase).center());
    assert!(hint_now(&r.ctx).0.contains("polarity"));
}

/// FL's send row: with a strip selected, a click on another strip's arrow sends it
/// there, Ctrl+click routes it there only, and an arrow that would loop does nothing.
#[test]
fn the_send_row_routes_the_selected_strip() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    // FL's row is the Rows menu's to show (off by default since round 39).
    mixer::set_row(&mut r.app, mixer::layout::Row::Sends, true);
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    r.app.select_strip(ids[0]);
    r.idle(2);
    r.click(
        r.rect(Some(ids[3]), Control::Send).center(),
        Modifiers::NONE,
    );
    let s1 = r.app.project().strip(ids[0]).unwrap().clone();
    assert!(s1.send_to(ids[3]).is_some(), "a send to strip 4");
    r.click(
        r.rect(Some(ids[4]), Control::Send).center(),
        Modifiers::COMMAND,
    );
    assert_eq!(r.app.project().strip(ids[0]).unwrap().output, Some(ids[4]));
    // Strip 5 now takes strip 1; from strip 5, strip 1's arrow would loop.
    r.app.select_strip(ids[4]);
    r.idle(1);
    r.click(
        r.rect(Some(ids[0]), Control::Send).center(),
        Modifiers::NONE,
    );
    assert!(r.app.project().strip(ids[4]).unwrap().sends.is_empty());
    assert_eq!(r.app.project().strip(ids[4]).unwrap().output, None);
}

/// A strip selected elsewhere (the arrangement, the inspector) is scrolled into view.
#[test]
fn the_selected_strip_is_scrolled_into_view() {
    let mut r = Rig::new(grown(64), vec2(1000.0, 800.0));
    r.idle(2);
    assert_eq!(r.app.mixer.scroll().0, 0.0);
    let far = r.app.project().strips[55].id;
    r.app.select_strip(far);
    r.idle(2);
    assert!(r.app.mixer.scroll().0 > 0.0);
    let head = r.rect(Some(far), Control::Head);
    assert!(head.width() > 10.0, "the strip is on screen: {head:?}");
}

/// Clicks select, Ctrl+click adds, Shift+click selects a run; a fader dragged inside the
/// selection moves them all.
#[test]
fn selection_and_ganged_faders() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    r.idle(2);
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    r.click(
        r.rect(Some(ids[1]), Control::Head).center(),
        Modifiers::NONE,
    );
    assert_eq!(r.app.selected_strip, Some(ids[1]));
    r.click(
        r.rect(Some(ids[4]), Control::Head).center(),
        Modifiers::SHIFT,
    );
    assert_eq!(r.app.strip_selection().len(), 4, "strips 2 to 5");
    r.click(
        r.rect(Some(ids[2]), Control::Head).center(),
        Modifiers::COMMAND,
    );
    assert_eq!(r.app.strip_selection().len(), 3, "Ctrl+click took 3 out");
    // Drag strip 2's fader down: 2, 4 and 5 move together.
    let f = r.rect(Some(ids[1]), Control::Fader);
    let cap = pos2(f.center().x, f.top() + f.height() * 0.25);
    r.hover(cap);
    r.frame(vec![Event::PointerButton {
        pos: cap,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    for i in 1..=6 {
        r.pointer = Some(cap + vec2(0.0, 6.0 * i as f32));
        r.frame(Vec::new());
    }
    r.frame(vec![Event::PointerButton {
        pos: cap + vec2(0.0, 36.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    let g = |i: usize| r.app.project().strips[i].gain_db;
    assert!(g(1) < 0.0, "{}", g(1));
    assert_eq!(g(1), g(3));
    assert_eq!(g(1), g(4));
    assert_eq!(g(2), 0.0, "strip 3 is out of the selection");
}

/// The width mode is kept per window layout.
#[test]
fn the_width_mode_is_kept_per_layout() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = Rig::new(starter_project(), vec2(1400.0, 900.0));
    r.app.mixer.load_prefs(&dir.path().join("layouts.json"));
    r.idle(1);
    let mut p = r.app.mixer.prefs();
    p.width = WidthMode::Wide;
    r.app.mixer.set_prefs(p);
    // Saved as a new layout: it starts from the set-up in use, then goes its own way.
    r.app.layouts.current.name = "Mixing".into();
    r.idle(1);
    assert_eq!(r.app.mixer.prefs().width, WidthMode::Wide);
    let mut p = r.app.mixer.prefs();
    p.width = WidthMode::Xl;
    r.app.mixer.set_prefs(p);
    r.app.layouts.current.name = cantus_ui::layout::DEFAULT_NAME.into();
    r.idle(1);
    assert_eq!(r.app.mixer.prefs().width, WidthMode::Wide);
    // And it survives a restart.
    let mut again = Rig::new(starter_project(), vec2(1400.0, 900.0));
    again.app.mixer.load_prefs(&dir.path().join("layouts.json"));
    again.app.layouts.current.name = "Mixing".into();
    again.idle(1);
    assert_eq!(again.app.mixer.prefs().width, WidthMode::Xl);
}

#[test]
fn wider_modes_show_more() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    let compact = r.idle(2);
    assert!(
        !compact.contains("| 0.0 |"),
        "Compact has no level row: {compact}"
    );
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let wide = r.idle(2);
    assert!(wide.contains("| 0.0 |"), "the level: {wide}");
    assert!(wide.contains("| Master |"), "where it goes: {wide}");
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    let s0 = r.app.project().strips[0].id;
    r.app.add_effect(s0, cantus_core::EffectKind::Eq);
    r.app.add_effect(s0, cantus_core::EffectKind::Compressor);
    r.app.add_effect(s0, cantus_core::EffectKind::Limiter);
    let xl = r.idle(2);
    // Large lists the inserts by their units' names; Wide by their kinds.
    assert!(
        xl.contains("| Aequalis |") && xl.contains("| Comprimo |"),
        "{xl}"
    );
    assert!(mixer::control_rect(&r.ctx, Some(s0), Control::Chain).is_some());
    assert!(mixer::control_rect(&r.ctx, Some(s0), Control::SendAdd).is_some());
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let wide = r.idle(2);
    assert!(
        wide.contains("| EQ |") && wide.contains("| Comp |"),
        "{wide}"
    );
}

/// Rename in place: a double-click on the name, type, Enter.
#[test]
fn a_strip_is_renamed_in_place() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 900.0));
    r.idle(2);
    let s2 = r.app.project().strips[1].id;
    let head = r.rect(Some(s2), Control::Head).center();
    r.hover(head);
    for _ in 0..2 {
        for pressed in [true, false] {
            r.frame(vec![Event::PointerButton {
                pos: head,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
    }
    r.idle(2);
    r.frame(vec![Event::Text("Bus A".into())]);
    r.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    r.idle(1);
    assert_eq!(r.app.project().strip(s2).unwrap().name, "Bus A");
}

/// Spanish, and the UI at 75 % and 200 %: the desk paints, with every row named.
#[test]
fn the_desk_paints_in_spanish_and_at_every_scale() {
    for ppp in [0.75f32, 1.0, 2.0] {
        let mut r = Rig::new(starter_project(), vec2(1280.0, 720.0) / ppp);
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
        r.ctx.set_pixels_per_point(ppp);
        let text = r.idle(2);
        assert!(text.contains("| Máster |"), "{text}");
        assert!(!text.contains('\u{2026}'), "nothing elided: {text}");
        assert!(text.contains("| 1 |"));
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// The pan law lives in the master's menu (des-mixer: no chip on the strip).
#[test]
fn the_master_carries_the_pan_law() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    r.idle(2);
    mixer::open_menu(&mut r.app, Some(StripId::MASTER), pos2(60.0, 300.0));
    let text = r.idle(2);
    let law = cantus_ui::i18n::t("mixer.pan_law");
    assert!(text.contains(&format!("| {law} |")), "{text}");
    let _ = Send::new;
}

// ---- round 25: the cable band and Extra large's rows (ADR 0041, design §8 tests 6–8, 15)

use cantus_core::routing::Route;
use cantus_core::{Dock, SendTap};
use cantus_ui::views::mixer::layout::CableView;

/// Strips 15 and 16 docked at the right as returns; strip 2 sends to 15.
fn with_returns(r: &mut Rig) -> Vec<StripId> {
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    for id in [ids[14], ids[15]] {
        r.app.exec(Command::SetStripDock {
            id,
            dock: Dock::Right,
        });
    }
    r.app.set_strip_send(ids[1], Send::new(ids[14]));
    r.app.end_gesture();
    ids
}

fn set_cables(r: &mut Rig, view: CableView) {
    let mut p = r.app.mixer.prefs();
    p.cables = view;
    r.app.mixer.set_prefs(p);
}

fn drawn(r: &Rig, route: Route) -> Option<cantus_ui::views::mixer::DrawnCable> {
    r.app
        .mixer
        .cables()
        .iter()
        .find(|d| d.route == route)
        .copied()
}

/// A point on a drawn cable's own run, a quarter of the way from its source (clear of
/// its jacks and of a level pill at its lowest point).
fn on_run(r: &Rig, d: &cantus_ui::views::mixer::DrawnCable) -> Pos2 {
    let p = r.app.mixer.cable_path(d);
    p[p.len() / 4]
}

/// Design test 8: a send from strip 2 to a right-dock return paints a cable from strip
/// 2's port to the return's; the selection's output to the master is drawn too, and the
/// cable view decides the rest — the selection's (the default), all, or none.
#[test]
fn the_band_draws_the_selections_cables_port_to_port() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    let ids = with_returns(&mut r);
    r.app.select_strip(ids[1]);
    r.idle(3);
    assert_eq!(r.app.mixer.prefs().cables, CableView::Selected, "answer 1");
    let send = drawn(&r, Route::send(ids[1], ids[14])).expect("the send's cable");
    let port = |r: &Rig, id| r.rect(Some(id), Control::Port).center();
    // r39:el-cables — from the source's output jack (the port) into the target's input
    // jack, left of its output.
    assert!((send.from.x - port(&r, ids[1]).x).abs() < 0.5, "{send:?}");
    let into = port(&r, ids[14]).x - send.to.x;
    assert!(into > 8.0 && into <= 22.5, "{send:?} {into}");
    assert!(send.lit && send.chips == [false, false]);
    let master = drawn(&r, Route::output(ids[1], StripId::MASTER));
    assert!(master.is_some(), "the selection's master route is drawn");
    assert_eq!(r.app.mixer.cables_drawn(), 2);
    // Another strip selected: its master route alone; strip 2's send is not drawn.
    r.app.select_strip(ids[5]);
    r.idle(2);
    assert!(drawn(&r, Route::send(ids[1], ids[14])).is_none());
    assert_eq!(r.app.mixer.cables_drawn(), 1);
    // All: every route but the implied master ones, those away from the selection at
    // 45 %; a strip that receives (the return) draws its way to the master.
    set_cables(&mut r, CableView::All);
    r.idle(2);
    let send = drawn(&r, Route::send(ids[1], ids[14])).expect("drawn in All");
    assert!(
        !send.lit && send.alpha < 0.5,
        "away from the selection: faint"
    );
    assert!(drawn(&r, Route::output(ids[1], StripId::MASTER)).is_none());
    assert!(drawn(&r, Route::output(ids[14], StripId::MASTER)).is_some());
    assert_eq!(r.app.mixer.cables_drawn(), 3);
    // Off: no band at all.
    set_cables(&mut r, CableView::Off);
    r.idle(2);
    assert_eq!(r.app.mixer.cables_drawn(), 0);
    assert!(mixer::control_rect(&r.ctx, Some(ids[1]), Control::Port).is_none());
}

/// Answers 3, 4 and 5: a send's level is on its cable, a pre-fader send's pill says PRE,
/// and a doubled signal (the output already reaches the destination) is named as such
/// in the hint bar.
#[test]
fn a_cable_carries_its_level_pre_and_the_doubled_warning() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    // The level pills are an option of the cable view since round 39 (des-mixer).
    {
        let mut p = r.app.mixer.prefs();
        p.cable_opts.levels = true;
        r.app.mixer.set_prefs(p);
    }
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids = with_returns(&mut r);
    r.app.set_strip_send(
        ids[1],
        Send {
            level: 0.5,
            tap: SendTap::Pre,
            ..Send::new(ids[14])
        },
    );
    // Strip 2's output goes to strip 9, and it also sends there: doubled.
    r.app.exec(Command::SetStripOutput {
        id: ids[1],
        output: Some(ids[8]),
    });
    r.app.set_strip_send(ids[1], Send::new(ids[8]));
    r.app.end_gesture();
    r.app.select_strip(ids[1]);
    let text = r.idle(3);
    assert!(
        text.contains("| PRE |"),
        "the pre-fader send is marked: {text}"
    );
    // The value's unit is set in its own run: the digits are what is checked.
    assert!(text.contains("| \u{2212}6.0"), "its level: {text}");
    let doubled = drawn(&r, Route::send(ids[1], ids[8])).expect("the doubled send");
    // Hover the doubled cable's run, away from its pill.
    let at = on_run(&r, &doubled);
    r.hover(at);
    let (name, value) = hint_now(&r.ctx);
    assert!(
        name.contains('\u{2192}'),
        "the hint names both ends: {name}"
    );
    assert!(
        value.contains("twice"),
        "the doubled signal is said: {value}"
    );
}

/// A click selects a cable (its halo), Delete removes it in one undo step, Esc lets it
/// go; a drag on a pill sets the send's level.
#[test]
fn a_cable_is_selected_removed_and_its_pill_dragged() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    // The level pills are an option of the cable view since round 39 (des-mixer).
    {
        let mut p = r.app.mixer.prefs();
        p.cable_opts.levels = true;
        r.app.mixer.set_prefs(p);
    }
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids = with_returns(&mut r);
    r.app.exec(Command::SetStripOutput {
        id: ids[1],
        output: Some(ids[9]),
    });
    r.app.select_strip(ids[1]);
    r.app.focus = cantus_ui::app::EditFocus::Mixer;
    r.idle(3);
    let out = drawn(&r, Route::output(ids[1], ids[9])).expect("the output's cable");
    r.click(on_run(&r, &out), Modifiers::NONE);
    assert_eq!(
        r.app.mixer.selected_cable(),
        Some(Route::output(ids[1], ids[9]))
    );
    let before = r.app.project().clone();
    r.frame(vec![Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    r.idle(1);
    assert_eq!(r.app.project().strip(ids[1]).unwrap().output, None);
    assert_eq!(r.app.mixer.selected_cable(), None);
    r.app.undo();
    assert_eq!(*r.app.project(), before, "one undo step brings it back");
    // The send's pill: a drag down lowers the level (unity is the top).
    r.idle(2);
    let pill = *r.app.mixer.cable_pills().first().expect("the send's pill");
    let level = |r: &Rig| r.app.project().strip(ids[1]).unwrap().sends[0].level;
    let start = level(&r);
    r.hover(pill.center());
    r.frame(vec![Event::PointerButton {
        pos: pill.center(),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    for k in 1..=6 {
        r.pointer = Some(pill.center() + vec2(0.0, 6.0 * k as f32));
        r.frame(Vec::new());
    }
    r.frame(vec![Event::PointerButton {
        pos: r.pointer.unwrap(),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    assert!(level(&r) < start, "the pill lowered the level ({start})");
}

/// Delete with a cable selected and the mixer focused removes the cable — never the
/// notes still selected in the piano roll (the keymap's Delete runs before the desk).
#[test]
fn delete_on_a_selected_cable_removes_the_cable_not_the_notes() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids = with_returns(&mut r);
    r.app.exec(Command::SetStripOutput {
        id: ids[1],
        output: Some(ids[9]),
    });
    // Notes selected in the piano roll's clip.
    let (track, clip) = {
        let t = &r.app.project().tracks[0];
        (t.id, t.clips[0].id)
    };
    r.app.select_clip(track, clip);
    let notes: Vec<_> = r.app.project().tracks[0].clips[0]
        .notes
        .iter()
        .map(|n| n.id)
        .collect();
    assert!(!notes.is_empty());
    r.app.selected_notes = notes.to_vec();
    r.app.select_strip(ids[1]);
    r.idle(3);
    let out = drawn(&r, Route::output(ids[1], ids[9])).expect("the output's cable");
    r.click(on_run(&r, &out), Modifiers::NONE);
    r.app.focus = cantus_ui::app::EditFocus::Mixer;
    r.app.selected_notes = notes.to_vec();
    assert!(r.app.mixer.selected_cable().is_some());
    // As the application's frame runs: the keymap first, then the panels.
    r.frame += 1;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, r.size)),
        time: Some(r.frame as f64 / 60.0),
        events: vec![Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..Default::default()
    };
    let app = &mut r.app;
    r.ctx
        .run_ui(input, |ui| {
            app.global_shortcuts(ui.ctx());
            egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
        })
        .textures_delta
        .clear();
    r.idle(1);
    assert_eq!(
        r.app.project().strip(ids[1]).unwrap().output,
        None,
        "the cable went"
    );
    assert_eq!(
        r.app.project().tracks[0].clips[0].notes.len(),
        notes.len(),
        "the notes stayed"
    );
}

/// An end scrolled out of the middle becomes an edge chip with the far strip's number; a
/// click on it scrolls that strip into view.
#[test]
fn an_end_out_of_view_is_a_chip_that_scrolls_to_it() {
    let mut r = Rig::new(grown(64), vec2(1000.0, 800.0));
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    r.app.set_strip_send(ids[1], Send::new(ids[50]));
    r.app.end_gesture();
    r.app.select_strip(ids[1]);
    let text = r.idle(3);
    let d = drawn(&r, Route::send(ids[1], ids[50])).expect("drawn to its chip");
    assert_eq!(d.chips, [false, true], "{d:?}");
    assert!(
        text.contains("| 51 \u{203a} |"),
        "the chip names strip 51: {text}"
    );
    assert!(mixer::control_rect(&r.ctx, Some(ids[50]), Control::Head).is_none());
    r.click(d.to, Modifiers::NONE);
    r.idle(1);
    assert!(
        mixer::control_rect(&r.ctx, Some(ids[50]), Control::Head).is_some(),
        "the chip scrolled strip 51 into view"
    );
}

/// Extra large's own send rows: the lamp switches a send, a drag sets its level, "+"
/// follows the last send; the output chip says where the strip goes.
#[test]
fn extra_large_send_rows_switch_and_set_their_sends() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    let ids = with_returns(&mut r);
    r.app.exec(Command::SetStripName {
        id: ids[14],
        name: "Reverb".into(),
    });
    // Round 37: the band has as many rows as the strip with the most sends; a strip with
    // two sends makes room for "+" after strip 2's one.
    r.app.set_strip_send(ids[3], Send::new(ids[14]));
    r.app.set_strip_send(ids[3], Send::new(ids[15]));
    r.app.end_gesture();
    let text = r.idle(3);
    assert!(text.contains("| Reverb |"), "the send row names it: {text}");
    let row = r.rect(Some(ids[1]), Control::SendRow(0));
    assert!(mixer::control_rect(&r.ctx, Some(ids[1]), Control::SendAdd).is_some());
    let send = |r: &Rig| r.app.project().strip(ids[1]).unwrap().sends[0];
    r.click(
        egui::pos2(row.left() + 5.0, row.center().y),
        Modifiers::NONE,
    );
    assert!(!send(&r).enabled, "the lamp switched it off");
    r.app.undo();
    assert!(send(&r).enabled);
    let start = send(&r).level;
    r.hover(row.center());
    r.frame(vec![Event::PointerButton {
        pos: row.center(),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    for k in 1..=5 {
        r.pointer = Some(row.center() + vec2(0.0, 6.0 * k as f32));
        r.frame(Vec::new());
    }
    r.frame(vec![Event::PointerButton {
        pos: r.pointer.unwrap(),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    assert!(send(&r).level < start, "dragged down: quieter");
    assert!(text.contains("| Master |"), "where it goes: {text}");
}

/// Design test 15: in Spanish and English, at 75 % and 200 %, Wide and Extra large, the
/// band's pills overlap neither each other nor the ports, and the band's words fit.
#[test]
fn the_band_reads_without_overlaps_at_every_scale_and_language() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for ppp in [0.75f32, 2.0] {
            for mode in [WidthMode::Wide, WidthMode::Xl] {
                let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0) / ppp);
                cantus_ui::i18n::set_thread_locale(locale);
                r.ctx.set_pixels_per_point(ppp);
                mixer::set_width_mode(&mut r.app, mode);
                let ids = with_returns(&mut r);
                for (k, to) in [(2usize, 14usize), (3, 15), (4, 14), (2, 8)] {
                    r.app.set_strip_send(ids[k], Send::new(ids[to]));
                }
                r.app.end_gesture();
                r.app.selected_strips = vec![ids[1], ids[2], ids[3], ids[4]];
                r.app.selected_strip = Some(ids[2]);
                set_cables(&mut r, CableView::All);
                // The level pills are an option since round 39 (des-mixer's band is the
                // sockets' row).
                let mut p = r.app.mixer.prefs();
                p.cable_opts.levels = true;
                r.app.mixer.set_prefs(p);
                r.idle(3);
                let pills = r.app.mixer.cable_pills().to_vec();
                assert!(!pills.is_empty(), "{locale:?} {ppp} {mode:?}: no pills");
                // The band's words (pills, PRE tags, edge chips, tap chips) are never cut
                // (integration finding 24); nor is a send row's tap word, and no name
                // runs under its chip.
                let texts = r.texts();
                let ports: Vec<egui::Rect> = ids
                    .iter()
                    .filter_map(|id| mixer::control_rect(&r.ctx, Some(*id), Control::Port))
                    .collect();
                let band_top = ports.iter().map(|p| p.top()).fold(f32::MAX, f32::min) - 3.0;
                let band_left = ports.iter().map(|p| p.left()).fold(f32::MAX, f32::min) - 4.0;
                let mut band_words = 0;
                for (t, rect) in &texts {
                    if rect.center().y >= band_top && rect.center().x >= band_left {
                        band_words += 1;
                        assert!(
                            !t.contains('\u{2026}'),
                            "{locale:?} {ppp} {mode:?}: band word cut: {t:?}"
                        );
                    }
                }
                assert!(
                    band_words > 0,
                    "{locale:?} {ppp} {mode:?}: the band's words read"
                );
                for (i, a) in pills.iter().enumerate() {
                    for b in &pills[i + 1..] {
                        assert!(!a.intersects(*b), "{locale:?} {ppp}: {a:?} on {b:?}");
                    }
                    for id in &ids {
                        if let Some(port) = mixer::control_rect(&r.ctx, Some(*id), Control::Port) {
                            let ring = egui::Rect::from_center_size(port.center(), vec2(7.0, 7.0));
                            assert!(!a.intersects(ring), "{locale:?} {ppp}: a pill on a port");
                        }
                    }
                }
            }
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// One pass as the application runs it: the keymap first, then (when `desk`) the panels.
fn app_pass(r: &mut Rig, events: Vec<Event>, desk: bool) {
    r.frame += 1;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, r.size)),
        time: Some(r.frame as f64 / 60.0),
        events,
        ..Default::default()
    };
    let app = &mut r.app;
    r.ctx
        .run_ui(input, |ui| {
            app.global_shortcuts(ui.ctx());
            if desk {
                egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
            }
        })
        .textures_delta
        .clear();
}

fn delete_key() -> Event {
    Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Integration finding: Delete removed a selected cable the musician could no longer see.
/// The *Off* view lets the selection go, and a desk that is not on screen (its band drew
/// nothing on the last pass) never has its cable removed by Delete.
#[test]
fn delete_never_removes_a_cable_that_is_not_drawn() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids = with_returns(&mut r);
    r.app.exec(Command::SetStripOutput {
        id: ids[1],
        output: Some(ids[9]),
    });
    r.app.select_strip(ids[1]);
    r.app.focus = cantus_ui::app::EditFocus::Mixer;
    r.idle(3);
    let route = Route::output(ids[1], ids[9]);
    let out = drawn(&r, route).expect("the output's cable");
    r.click(on_run(&r, &out), Modifiers::NONE);
    assert_eq!(r.app.mixer.selected_cable(), Some(route));
    // The view switched Off: nothing shows, nothing stays selected, Delete does nothing.
    let mut prefs = r.app.mixer.prefs();
    prefs.cables = mixer::layout::CableView::Off;
    r.app.mixer.set_prefs(prefs);
    r.idle(1);
    assert_eq!(r.app.mixer.selected_cable(), None, "Off lets it go");
    app_pass(&mut r, vec![delete_key()], true);
    assert_eq!(r.app.project().strip(ids[1]).unwrap().output, Some(ids[9]));
    // Back to Selected, selected again, then the desk leaves the screen: Delete on the
    // next passes (the keymap only) keeps the cable.
    prefs.cables = mixer::layout::CableView::Selected;
    r.app.mixer.set_prefs(prefs);
    r.idle(2);
    r.click(on_run(&r, &out), Modifiers::NONE);
    assert_eq!(r.app.mixer.selected_cable(), Some(route));
    app_pass(&mut r, Vec::new(), false);
    app_pass(&mut r, vec![delete_key()], false);
    assert_eq!(
        r.app.project().strip(ids[1]).unwrap().output,
        Some(ids[9]),
        "a desk not on screen keeps its cable"
    );
    // On screen again: Delete removes it.
    r.idle(1);
    app_pass(&mut r, vec![delete_key()], true);
    assert_eq!(r.app.project().strip(ids[1]).unwrap().output, None);
}

/// Integration finding: Esc during a desk drag (or with a cable selected) also ran the
/// keymap's *Clear selection*, dropping the notes selected elsewhere. The desk takes Esc
/// for itself then; with nothing of the desk's to cancel, Esc clears as before.
#[test]
fn esc_on_the_desk_cancels_its_own_things_and_clears_nothing_else() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids = with_returns(&mut r);
    r.app.exec(Command::SetStripOutput {
        id: ids[1],
        output: Some(ids[9]),
    });
    r.app.select_strip(ids[1]);
    r.app.focus = cantus_ui::app::EditFocus::Mixer;
    let notes: std::collections::BTreeSet<_> = r.app.project().tracks[0].clips[0]
        .notes
        .iter()
        .map(|n| n.id)
        .collect();
    assert!(!notes.is_empty());
    r.idle(3);
    let esc = || Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    // A cable selected: Esc lets it go, the notes stay.
    let out = drawn(&r, Route::output(ids[1], ids[9])).expect("the output's cable");
    r.click(on_run(&r, &out), Modifiers::NONE);
    assert!(r.app.mixer.selected_cable().is_some());
    r.app.selected_notes = notes.iter().copied().collect();
    app_pass(&mut r, vec![esc()], true);
    r.idle(1);
    assert_eq!(r.app.mixer.selected_cable(), None, "Esc let the cable go");
    assert_eq!(r.app.selected_notes.len(), notes.len(), "the notes stayed");
    // A strip carried by its number cell: Esc cancels the drag, the notes stay.
    let head = r.rect(Some(ids[2]), Control::Head).center();
    r.pointer = Some(head);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    app_pass(
        &mut r,
        vec![Event::PointerMoved(head), press(head, true)],
        true,
    );
    for k in 1..=4 {
        let at = head + vec2(15.0 * k as f32, 0.0);
        app_pass(&mut r, vec![Event::PointerMoved(at)], true);
    }
    assert!(r.app.mixer.claims_escape(), "the drag is on");
    app_pass(&mut r, vec![esc()], true);
    assert!(!r.app.mixer.claims_escape(), "Esc cancelled the drag");
    assert_eq!(r.app.selected_notes.len(), notes.len(), "the notes stayed");
    let at = head + vec2(60.0, 0.0);
    app_pass(&mut r, vec![press(at, false)], true);
    // Nothing of the desk's left: Esc clears the selection as it always did.
    app_pass(&mut r, vec![esc()], true);
    assert!(r.app.selected_notes.is_empty(), "Esc clears as before");
    // A cable selected on a desk that then leaves the screen: Esc is not the hidden
    // desk's to take. (The drag picked strip 3 up, and selected it: strip 2 again, so its
    // cable hangs where `out` was drawn — a cable only the hover lights keeps
    // its span's depth, round 26.)
    r.app.select_strip(ids[1]);
    r.idle(1);
    r.click(on_run(&r, &out), Modifiers::NONE);
    assert!(r.app.mixer.selected_cable().is_some());
    r.app.selected_notes = notes.iter().copied().collect();
    app_pass(&mut r, Vec::new(), false);
    app_pass(&mut r, vec![esc()], false);
    assert!(
        r.app.selected_notes.is_empty(),
        "a hidden desk does not eat Esc"
    );
}

/// Integration finding: FL's from-selected arrow acted for the selected strip alone and
/// a refused click did nothing silently. It is the desk's one gesture now: the whole
/// selection sends (one undo step), and a refusal is said in the hint bar.
#[test]
fn the_from_selected_arrow_acts_for_the_selection_and_says_a_refusal() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    // FL's row is the Rows menu's to show (off by default since round 39).
    mixer::set_row(&mut r.app, mixer::layout::Row::Sends, true);
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    r.app.select_strip(ids[0]);
    r.app.toggle_strip_selected(ids[1]);
    r.idle(2);
    r.click(
        r.rect(Some(ids[5]), Control::Send).center(),
        Modifiers::NONE,
    );
    for from in [ids[0], ids[1]] {
        let st = r.app.project().strip(from).unwrap();
        assert_eq!(st.send_to(ids[5]), Some(&Send::new(ids[5])), "{from:?}");
    }
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.add_send"));
    r.app.undo();
    assert!(
        r.app.project().strip(ids[1]).unwrap().sends.is_empty(),
        "one step"
    );
    // Strip 1 feeds strip 6; from strip 6, strip 1's hollow arrow refuses — and says why.
    r.app.exec(Command::SetStripOutput {
        id: ids[0],
        output: Some(ids[5]),
    });
    r.app.select_strip(ids[5]);
    r.idle(2);
    let arrow = r.rect(Some(ids[0]), Control::Send).center();
    r.click(arrow, Modifiers::NONE);
    r.hover(arrow);
    let (_, value) = hint::current(&r.ctx)
        .map(|h| (h.name, h.value))
        .unwrap_or_default();
    assert!(
        value.contains("Would make a loop"),
        "the refusal is said: {value}"
    );
    assert!(r.app.project().strip(ids[5]).unwrap().sends.is_empty());
}

/// Integration finding 25: a cable whose two ends are both scrolled out on the same side
/// draws nothing (no chip pair, no stub); one with an end on screen draws, its far end a
/// chip.
#[test]
fn a_cable_with_both_ends_off_on_one_side_draws_nothing() {
    let mut r = Rig::new(grown(64), vec2(1000.0, 800.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    r.app.set_strip_send(ids[40], Send::new(ids[50]));
    r.app.set_strip_send(ids[1], Send::new(ids[52]));
    r.app.end_gesture();
    r.app.select_strip(ids[0]);
    set_cables(&mut r, CableView::All);
    r.idle(3);
    assert_eq!(r.app.mixer.scroll().0, 0.0);
    assert!(
        drawn(&r, Route::send(ids[40], ids[50])).is_none(),
        "both ends off to the right: nothing drawn"
    );
    let one = drawn(&r, Route::send(ids[1], ids[52])).expect("one end on screen draws");
    assert_eq!(one.chips, [false, true], "its far end is a chip");
}

/// Round 26 (ADR 0043 §2), as round 39 draws the cables: a cable's shape comes from its
/// two jacks, never from the hover. Hovering a strip lights its routes, but the
/// selection's cables keep their place — so an input jack never moves under a pointer
/// approaching it, whether the pointer is on another strip or on the jack itself; and
/// pulling the plug out of that jack moves the selection's route, never another's.
#[test]
fn hovering_a_strip_never_moves_the_selections_lanes() {
    for view in [CableView::Selected, CableView::All] {
        let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
        // The level pills are an option of the cable view since round 39 (des-mixer).
        {
            let mut p = r.app.mixer.prefs();
            p.cable_opts.levels = true;
            r.app.mixer.set_prefs(p);
        }
        mixer::set_width_mode(&mut r.app, WidthMode::Wide);
        let ids = with_returns(&mut r);
        r.app.exec(Command::SetStripOutput {
            id: ids[1],
            output: Some(ids[9]),
        });
        r.app.set_strip_send(ids[1], Send::new(ids[6]));
        r.app.set_strip_send(ids[4], Send::new(ids[14]));
        r.app.set_strip_send(ids[4], Send::new(ids[15]));
        r.app.exec(Command::SetStripOutput {
            id: ids[4],
            output: Some(ids[8]),
        });
        r.app.end_gesture();
        set_cables(&mut r, view);
        r.app.select_strip(ids[1]);
        r.pointer = Some(pos2(-50.0, -50.0));
        r.idle(3);
        let mine = [
            Route::output(ids[1], ids[9]),
            Route::send(ids[1], ids[14]),
            Route::send(ids[1], ids[6]),
        ];
        let at_rest: Vec<_> = mine
            .iter()
            .map(|rt| drawn(&r, *rt).unwrap_or_else(|| panic!("{view:?}: {rt:?} drawn")))
            .collect();
        // The pointer on strip 5's fader (its cables cross the selection's), then under
        // strip 10's jacks, then on the input jack strip 2's output plugs into.
        let fader = r.rect(Some(ids[4]), Control::Fader).center();
        let mark = at_rest[0].to;
        let below = r.rect(Some(ids[9]), Control::Port).center() + vec2(0.0, 10.0);
        for (what, p) in [
            ("strip 5", fader),
            ("strip 10's column", below),
            ("the jack", mark),
        ] {
            r.hover(p);
            r.idle(1);
            for (rt, before) in mine.iter().zip(&at_rest) {
                let now = drawn(&r, *rt).unwrap_or_else(|| panic!("{view:?} {what}: {rt:?}"));
                assert!(
                    (now.depth - before.depth).abs() < 0.01
                        && (now.from - before.from).length() < 0.01
                        && (now.to - before.to).length() < 0.01,
                    "{view:?}, pointer on {what}: {rt:?} moved from {before:?} to {now:?}"
                );
            }
        }
        // Pulling the plug out of strip 10's input jack moves the selection's route, never
        // another strip's.
        let target = r.rect(Some(ids[12]), Control::Fader).center();
        r.pointer = Some(mark);
        r.idle(2);
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        r.frame(vec![press(mark, true)]);
        for k in 1..=8 {
            r.pointer = Some(mark + (target - mark) * (k as f32 / 8.0));
            r.frame(Vec::new());
        }
        r.frame(vec![press(target, false)]);
        r.idle(2);
        let p = r.app.project();
        assert_eq!(
            p.strip(ids[1]).unwrap().output,
            Some(ids[12]),
            "{view:?}: the selection's output moved to strip 13"
        );
        assert_eq!(
            p.strip(ids[4]).unwrap().output,
            Some(ids[8]),
            "{view:?}: strip 5's kept"
        );
        assert_eq!(
            p.strip(ids[9]).unwrap().output,
            None,
            "{view:?}: strip 10's kept"
        );
        r.app.undo();
        r.pointer = Some(pos2(-50.0, -50.0));
        r.idle(2);
        // The hover did light something: strip 5's own send.
        r.hover(fader);
        assert!(
            drawn(&r, Route::send(ids[4], ids[14])).is_some_and(|d| d.lit),
            "{view:?}: hovering strip 5 lights its routes"
        );
    }
}

/// Round 26: no two words of the legend column read the same — the row names (and their
/// short forms), ROUTING and its three line samples — and no word a strip or the band
/// paints (the plate, the "+" rows, the tap chips, the PRE tag, MASTER) reads as one of
/// them, in either language (round 25's Spanish legend said SALIDA under ROUTING beside
/// Wide's SALIDA row). Then the painted desk, Wide and Extra large with the cables on:
/// each legend word once.
#[test]
fn the_legend_words_never_read_the_same() {
    use cantus_ui::views::mixer::layout::Row;
    for locale in cantus_ui::i18n::Locale::ALL {
        cantus_ui::i18n::set_thread_locale(locale);
        let tr = |k: &str| cantus_ui::i18n::t(k).to_uppercase();
        let mut legend: Vec<(String, String)> = Vec::new();
        for row in Row::ALL {
            let key = row.legend_key();
            legend.push((key.to_string(), tr(key)));
            let short = format!("{key}.short");
            if cantus_ui::i18n::t(&short) != short {
                legend.push((short.clone(), tr(&short)));
            }
        }
        for (key, kind, kinds) in [
            (
                "cables.legend.output",
                "cables.kind.output",
                "cables.kinds.outputs",
            ),
            (
                "cables.legend.send",
                "cables.kind.send",
                "cables.kinds.sends",
            ),
            ("cables.legend.key", "cables.kind.key", "cables.kinds.keys"),
        ] {
            legend.push((key.to_string(), tr(key)));
            // Review finding 10: the legend names a kind of line as everything else
            // names that kind — the cable's hover and the options menu's kinds (the
            // legend's word may be the short form of it, as English "out").
            let word = tr(key);
            for other in [kind, kinds] {
                assert!(
                    tr(other).contains(&word),
                    "{locale:?}: the legend says {word:?}, {other} says {:?}",
                    tr(other)
                );
            }
        }
        for (i, (ka, a)) in legend.iter().enumerate() {
            for (kb, b) in &legend[i + 1..] {
                let same_row = kb.strip_suffix(".short") == Some(ka.as_str());
                assert!(
                    same_row || a != b,
                    "{locale:?}: {ka} and {kb} both read {a:?}"
                );
            }
        }
        for key in [
            "routing.here.plate",
            "mixer.send_add",
            "mixer.chain.add",
            "matrix.tap.post.name",
            "matrix.tap.pre.name",
            "routing.tap.pre",
            "mixer.master",
        ] {
            let w = tr(key);
            for (k, l) in &legend {
                assert!(
                    &w != l,
                    "{locale:?}: {key} reads {w:?} like the legend's {k}"
                );
            }
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Second review, findings 1–3, as round 39 draws the cables (Wide, 1600 × 1000): a cable
/// the hover lights can be pointed at on its own run (the selection's beside it does not
/// take the point), and a lone hover cable hangs where its span puts it, with its level
/// pill.
#[test]
fn the_hovers_cables_are_pointable_and_hung_by_their_span() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    {
        let mut p = r.app.mixer.prefs();
        p.cable_opts.levels = true;
        r.app.mixer.set_prefs(p);
    }
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids = with_returns(&mut r);
    r.app.exec(Command::SetStripOutput {
        id: ids[1],
        output: Some(ids[9]),
    });
    r.app.exec(Command::SetStripOutput {
        id: ids[4],
        output: Some(ids[9]),
    });
    r.app.select_strip(ids[1]);
    r.idle(3);
    // Strip 5 hovered: 5 → 10 lights, beside the selection's 2 → 10.
    let fader = r.rect(Some(ids[4]), Control::Fader).center();
    r.hover(fader);
    r.idle(1);
    let theirs = drawn(&r, Route::output(ids[4], ids[9])).expect("5 → 10 lit by the hover");
    let mine = drawn(&r, Route::output(ids[1], ids[9])).expect("2 → 10");
    assert!(theirs.lit && mine.lit, "{theirs:?} {mine:?}");
    assert_eq!(theirs.to, mine.to, "both plug into strip 10's input jack");
    // Near its source, still in strip 5's column (so strip 5 stays hovered).
    let on_run = r.app.mixer.cable_path(&theirs)[2];
    r.click(on_run, Modifiers::NONE);
    assert_eq!(
        r.app.mixer.selected_cable(),
        Some(Route::output(ids[4], ids[9])),
        "the hover's cable is the one under the pointer on its run"
    );
    // A lone hover cable: strip 13 selected, strip 5's long send to return 15 hovered.
    r.app.mixer.select_cable(None);
    r.app.exec(Command::SetStripOutput {
        id: ids[4],
        output: None,
    });
    r.app.set_strip_send(ids[4], Send::new(ids[14]));
    r.app.end_gesture();
    r.app.select_strip(ids[12]);
    // At rest in All, it hangs at 45 % where its span puts it…
    set_cables(&mut r, CableView::All);
    r.pointer = Some(pos2(-50.0, -50.0));
    r.idle(2);
    let faint = drawn(&r, Route::send(ids[4], ids[14])).expect("5 → 15 faint in All");
    assert!(!faint.lit && faint.alpha < 0.5);
    // …and lit by the hover, it hangs there still.
    r.hover(fader);
    r.idle(1);
    let send = drawn(&r, Route::send(ids[4], ids[14])).expect("5 → 15 lit by the hover");
    assert!(send.lit && send.alpha == 1.0, "{send:?}");
    assert_eq!(send.depth, faint.depth, "a cable hangs by its span");
    assert!(send.depth >= 16.0, "not on the floor");
    let low = r.app.mixer.cable_point(&send); // a symmetric curve: its lowest point
    assert!(
        r.app.mixer.cable_pills().iter().any(|p| p.contains(low)),
        "the hover's send shows its level pill: {:?}",
        r.app.mixer.cable_pills()
    );
}

/// Final review, visual 1: the right dock never covers the master. Three returns docked at
/// the right on a narrow desk (1280 × 720 at 200 % leaves the mixer some 360 pt): where the
/// master, the left dock and the right dock do not fit side by side, the right dock
/// scrolls with the middle instead of lying over the master.
#[test]
fn the_right_dock_never_covers_the_master() {
    for w in [300.0f32, 360.0, 420.0, 520.0, 800.0] {
        for mode in WidthMode::ALL {
            let mut r = Rig::new(starter_project(), vec2(w, 340.0));
            mixer::set_width_mode(&mut r.app, mode);
            let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
            for id in [ids[13], ids[14], ids[15]] {
                r.app.exec(Command::SetStripDock {
                    id,
                    dock: Dock::Right,
                });
            }
            r.idle(4);
            let master = r.rect(None, Control::Head);
            for id in [ids[13], ids[14], ids[15]] {
                let Some(head) = mixer::control_rect(&r.ctx, Some(id), Control::Head) else {
                    continue;
                };
                assert!(
                    !head.shrink(0.5).intersects(master.shrink(0.5)),
                    "{w} pt, {mode:?}: a docked return {head:?} over the master {master:?}"
                );
            }
        }
    }
}

/// Final review, visual 6: renaming a strip that shows its track's name (Compact): the
/// field opens with that name selected, as a popover under the number row — the next
/// strips' numbers stay in sight — and an untouched name keeps following its track.
#[test]
fn a_strip_rename_opens_with_the_name_it_shows_clear_of_the_numbers() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 700.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Compact);
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    assert!(
        r.app.project().strips[0].name.is_empty(),
        "named after its track"
    );
    let shown = r.app.project().tracks[0].name.clone();
    r.idle(3);
    let head = r.rect(Some(ids[0]), Control::Head);
    let at = pos2(head.center().x, head.top() + 6.0);
    r.click(at, Modifiers::NONE);
    r.click(at, Modifiers::NONE);
    r.idle(2);
    assert!(r.app.mixer.renaming(), "the rename opened");
    let field = r
        .ctx
        .memory(|m| m.focused())
        .and_then(|id| r.ctx.read_response(id))
        .expect("the field holds the keyboard")
        .rect;
    let texts = r.texts();
    assert!(
        texts
            .iter()
            .any(|(s, at)| *s == shown && field.expand(1.0).contains_rect(*at)),
        "the field shows {shown:?}: {:?}",
        texts
            .iter()
            .filter(|(_, at)| field.intersects(*at))
            .collect::<Vec<_>>()
    );
    for n in 2..=6 {
        let num = n.to_string();
        let cell = texts
            .iter()
            .find(|(s, at)| *s == num && at.top() < field.bottom() + 40.0)
            .unwrap_or_else(|| panic!("strip {n}'s number painted"))
            .1;
        assert!(
            !cell.intersects(field),
            "strip {n}'s number {cell:?} under the field {field:?}"
        );
    }
    let rev = r.app.bus.revision();
    r.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    r.idle(2);
    assert!(!r.app.mixer.renaming());
    assert_eq!(r.app.bus.revision(), rev, "untouched: no command");
    assert!(
        r.app.project().strips[0].name.is_empty(),
        "still its track's"
    );
}

/// Every colour painted inside `area` (shapes and mesh vertices), for a look test.
fn colours_in(shapes: &[egui::epaint::ClippedShape], area: egui::Rect) -> Vec<egui::Color32> {
    fn walk(shape: &egui::Shape, area: egui::Rect, out: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, area, out)),
            egui::Shape::Mesh(m) => out.extend(
                m.vertices
                    .iter()
                    .filter(|v| area.contains(v.pos))
                    .map(|v| v.color),
            ),
            egui::Shape::Rect(r) if area.intersects(r.rect) => {
                out.push(r.fill);
                out.push(r.stroke.color);
            }
            egui::Shape::Path(p) if p.points.iter().any(|q| area.contains(*q)) => {
                out.push(p.fill);
                if let egui::epaint::PathStroke {
                    color: egui::epaint::ColorMode::Solid(c),
                    ..
                } = p.stroke
                {
                    out.push(c);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, area, &mut out);
    }
    out
}

/// Review of fix3: a send's cable pill that a modulator drives shows it — its outline in
/// the modulation hue — as the desk's send knob and row do.
#[test]
fn a_modulated_sends_pill_shows_its_modulation() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    // The level pills are an option of the cable view since round 39 (des-mixer).
    {
        let mut p = r.app.mixer.prefs();
        p.cable_opts.levels = true;
        r.app.mixer.set_prefs(p);
    }
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let ids = with_returns(&mut r);
    r.app.select_strip(ids[1]);
    r.idle(3);
    let pill = *r.app.mixer.cable_pills().first().expect("the send's pill");
    let hue = cantus_ui::kit::Style::current().t.hue_mod;
    let paint = |r: &mut Rig| {
        r.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, r.size)),
            time: Some(r.frame as f64 / 60.0),
            ..Default::default()
        };
        let app = &mut r.app;
        let mut out = r.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
        });
        out.textures_delta.clear();
        colours_in(&out.shapes, pill)
    };
    assert!(!paint(&mut r).contains(&hue), "nothing modulates it yet");
    let id = cantus_core::ModulatorId(r.app.bus.alloc_id());
    let mut m = cantus_core::modulator::Modulator::new(id, cantus_core::modulator::ModKind::Lfo);
    m.routes.push(cantus_core::modulator::ModRoute {
        target: cantus_core::AutoTarget::Send {
            strip: ids[1],
            to: ids[14],
        },
        depth: 0.3,
    });
    r.app.exec(Command::InsertModulator {
        host: cantus_core::modulator::ModHost::Strip(ids[1]),
        index: 0,
        modulator: Box::new(m),
    });
    r.idle(1);
    assert!(
        paint(&mut r).contains(&hue),
        "the pill shows the modulation"
    );
}

/// Round 37 (review of the finish's deliveries): where the middle scrolls, the right dock
/// starts after whole strips, but never past the window's edge — a narrow desk with many
/// returns docked right keeps every docked strip on screen, at every width.
#[test]
fn the_right_dock_never_leaves_the_window() {
    let mut r = Rig::new(grown(24), vec2(1400.0, 700.0));
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    for id in &ids[18..24] {
        r.app.exec(Command::SetStripDock {
            id: *id,
            dock: Dock::Right,
        });
    }
    let mut w = 1400.0;
    while w >= 700.0 {
        r.size = vec2(w, 700.0);
        r.frame(Vec::new());
        r.frame(Vec::new());
        let dock = r.app.mixer.right_dock_seen();
        let rows = r.app.mixer.rows_seen();
        assert!(dock.is_positive(), "{w}: a right dock");
        assert!(
            dock.right() <= rows.right() + 0.5,
            "{w}: the dock {dock:?} inside the desk {rows:?}"
        );
        w -= 37.0;
    }
}

/// Round 37 (finish, ADR 0103 §48): the insert panel, as the desk's ▾ menu turns it on,
/// shows the selected strip's inserts by name and its empty slots, at the desk's right
/// beside the strips; a click on an insert opens its editor.
#[test]
fn the_insert_panel_shows_the_selected_strips_inserts_and_opens_them() {
    let mut r = Rig::new(grown(12), vec2(1500.0, 760.0));
    let strip = r.app.project().strips[2].id;
    let slot = r.app.bus.new_effect(cantus_core::EffectKind::Saturator);
    let fx = slot.id;
    assert!(r.app.exec(Command::InsertEffect {
        strip,
        index: 0,
        slot: Box::new(slot),
    }));
    r.app.select_strip(strip);
    r.frame(Vec::new());
    let before = r.app.mixer.rows_seen();
    mixer::set_inserts_panel(&mut r.app, true);
    r.frame(Vec::new());
    let texts = r.texts();
    let after = r.app.mixer.rows_seen();
    assert!(
        after.right() < before.right() - 150.0,
        "the desk gives the panel its room: {before:?} → {after:?}"
    );
    let name = cantus_ui::history_text::effect_name(
        r.app
            .project()
            .strip(strip)
            .unwrap()
            .effects
            .first()
            .unwrap(),
    );
    let (_, at) = texts
        .iter()
        .find(|(t, _)| *t == name)
        .unwrap_or_else(|| panic!("the insert {name:?} in the panel: {texts:?}"));
    assert!(
        at.left() > after.right(),
        "the panel is right of the strips"
    );
    assert!(
        texts.iter().any(|(t, _)| t == "Slot 10"),
        "the empty slots named as places"
    );
    r.click(at.center(), Modifiers::NONE);
    assert!(
        r.app.fx.open.map(|(s, e, _)| (s, e)) == Some((strip, fx))
            || r.app.eq_editor.open == Some((strip, fx))
            || r.app.comp_editor.open == Some((strip, fx)),
        "a click opens the insert's editor"
    );
}

/// r39:lo-mixer — the overflow's pill ("5 ›") sits in a slot of its own beside the
/// scrolling middle, never over a strip (it covered the last strip's meter in Large): in
/// each density, scrolled to either end, no strip's column runs under either pill.
#[test]
fn the_overflow_pill_never_covers_a_strip() {
    let mut r = Rig::new(grown(64), vec2(1400.0, 700.0));
    for mode in [WidthMode::Compact, WidthMode::Wide, WidthMode::Xl] {
        mixer::set_width_mode(&mut r.app, mode);
        for _ in 0..2 {
            r.frame(Vec::new());
        }
        let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
        let mut pills = 0;
        for _ in 0..2 {
            for side in ["more-right", "more-left"] {
                let Some(pill) = r
                    .ctx
                    .read_response(egui::Id::new(("mixer-title", side)))
                    .map(|p| p.rect)
                else {
                    continue;
                };
                pills += 1;
                for s in &ids {
                    if let Some(head) = mixer::control_rect(&r.ctx, Some(*s), Control::Head) {
                        assert!(
                            pill.right() <= head.left() + 0.5 || pill.left() >= head.right() - 0.5,
                            "{mode:?}: the {side} pill {pill:?} over a strip {head:?}"
                        );
                    }
                }
            }
            // Paged to the far end by the right pill: the left pill shows.
            for _ in 0..12 {
                let Some(pill) = r
                    .ctx
                    .read_response(egui::Id::new(("mixer-title", "more-right")))
                    .map(|p| p.rect)
                else {
                    break;
                };
                r.click(pill.center(), Modifiers::NONE);
                r.idle(1);
            }
        }
        assert!(pills >= 2, "{mode:?}: the desk overflows both ways in turn");
        // Back to the start for the next density.
        for _ in 0..12 {
            let Some(pill) = r
                .ctx
                .read_response(egui::Id::new(("mixer-title", "more-left")))
                .map(|p| p.rect)
            else {
                break;
            };
            r.click(pill.center(), Modifiers::NONE);
            r.idle(1);
        }
    }
}

/// r39:lo-mixer — FL's next inserts: the desk fills its width with the next empty strips,
/// numbered on from the last, at 1920 and at 1280 wide, in every density (Compact most of
/// all); they are only painted (the project and the engine keep their strips), and a click
/// on one makes the strips up to it in one undo step, the last selected.
#[test]
fn the_desk_fills_its_width_with_the_next_empty_strips() {
    for w in [1920.0f32, 1280.0] {
        for mode in [WidthMode::Compact, WidthMode::Wide, WidthMode::Xl] {
            let mut r = Rig::new(welcome_project(), vec2(w, 700.0));
            mixer::set_width_mode(&mut r.app, mode);
            let strips = r.app.project().strips.len();
            r.idle(2);
            let shown = r.app.mixer.next_strips_shown();
            let first = format!("{}", strips + 1);
            let texts = r.texts();
            let painted = texts.iter().any(|(t, _)| *t == first);
            if mode == WidthMode::Compact {
                assert!(
                    shown >= 3,
                    "{w} {mode:?}: the next strips fill the desk ({shown})"
                );
            }
            if shown > 0 {
                assert!(painted, "{w} {mode:?}: numbered on from {strips}");
                // Filled to the edge: less than a strip's room is left after them.
                let rows = r.app.mixer.rows_seen();
                let last = texts
                    .iter()
                    .filter(|(t, _)| *t == format!("{}", strips + shown))
                    .map(|(_, at)| *at)
                    .next()
                    .expect("the last one numbered");
                assert!(
                    rows.right() - last.center().x < 2.0 * mode.pitch(),
                    "{w} {mode:?}: filled to the edge ({last:?} in {rows:?})"
                );
            }
            assert_eq!(
                r.app.project().strips.len(),
                strips,
                "{w} {mode:?}: painted only"
            );
        }
    }
    // A click on the third makes the strips up to it, one undo step.
    let mut r = Rig::new(welcome_project(), vec2(1920.0, 700.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Compact);
    let strips = r.app.project().strips.len();
    r.idle(2);
    let third = r
        .ctx
        .read_response(egui::Id::new(("mixer-next-strip", 2usize)))
        .expect("the third next strip")
        .rect;
    let steps = r.app.bus.history().len();
    r.click(third.center(), Modifiers::NONE);
    r.idle(1);
    let p = r.app.project();
    assert_eq!(p.strips.len(), strips + 3, "made up to it");
    assert_eq!(
        r.app.selected_strip,
        p.strips.last().map(|s| s.id),
        "selected"
    );
    assert_eq!(r.app.bus.history().len(), steps + 1, "one undo step");
    r.app.undo();
    assert_eq!(r.app.project().strips.len(), strips, "undone");
}

/// r39:fix2 — the desk shows every strip that fits and folds only what does not: an
/// overflow slot only on a side with strips hidden beyond it, and no gap a strip could
/// fill. At the start the middle begins where it begins on a desk that fits (lo-mixer's
/// left slot was an empty 36 pt there, and at 1280 × 720 in Wide it folded a third strip
/// into "3 ›"); the right dock stays at the desk's edge, and what is left beside the "n ›"
/// pill is less than a strip. Paged to the end: no right slot, the middle runs to the dock.
#[test]
fn the_desk_folds_only_what_does_not_fit() {
    for (w, mode) in [
        (1920.0f32, WidthMode::Xl),
        (1280.0, WidthMode::Wide),
        (1280.0, WidthMode::Compact),
        (1600.0, WidthMode::Wide),
    ] {
        let fits = {
            let mut r = Rig::new(welcome_project(), vec2(w, 700.0));
            mixer::set_width_mode(&mut r.app, mode);
            r.idle(3);
            r.app.mixer.middle_seen()
        };
        let mut r = Rig::new(grown(96), vec2(w, 700.0));
        mixer::set_width_mode(&mut r.app, mode);
        r.idle(3);
        let pill = |r: &Rig, side: &str| {
            r.ctx
                .read_response(egui::Id::new(("mixer-title", side)))
                .map(|p| p.rect)
        };
        let mid = r.app.mixer.middle_seen();
        let dock = r.app.mixer.right_dock_seen();
        let rows = r.app.mixer.rows_seen();
        let pitch = mode.pitch();
        assert!(
            pill(&r, "more-right").is_some(),
            "{w} {mode:?}: it overflows"
        );
        assert!(
            pill(&r, "more-left").is_none(),
            "{w} {mode:?}: nothing hidden on the left"
        );
        assert!(
            (mid.left() - fits.left()).abs() < 0.5,
            "{w} {mode:?}: the middle starts at {} as on a desk that fits ({}), no empty slot",
            mid.left(),
            fits.left()
        );
        let whole = mid.width() / pitch;
        assert!(
            (whole - whole.round()).abs() < 0.01,
            "{w} {mode:?}: whole strips ({whole})"
        );
        assert!(dock.width() > 0.0, "{w} {mode:?}: the returns docked right");
        // The pill whole in its slot, 2 pt clear of the strips and the dock; beside it and
        // its clearance, less than a strip's room.
        let p = pill(&r, "more-right").unwrap();
        let slot_r = dock.left() - 8.0 - mid.right();
        assert!(
            p.left() >= mid.right() + 1.5 && p.right() <= dock.left() - 8.0 - 1.5,
            "{w} {mode:?}: the pill {p:?} out of its slot ({mid:?}, {dock:?})"
        );
        let spare = slot_r - p.width() - 4.0;
        assert!(
            spare < pitch - 0.5,
            "{w} {mode:?}: {spare} pt beside the pill would hold another strip"
        );
        assert!(
            rows.right() - dock.right() < 16.0,
            "{w} {mode:?}: the returns at the desk's edge ({dock:?} in {rows:?})"
        );
        // Paged to the end: the left pill shows, the right one goes, the middle runs to
        // the dock.
        for _ in 0..40 {
            let Some(p) = pill(&r, "more-right") else {
                break;
            };
            r.click(p.center(), Modifiers::NONE);
            r.idle(1);
        }
        assert!(pill(&r, "more-right").is_none(), "{w} {mode:?}: at the end");
        assert!(
            pill(&r, "more-left").is_some(),
            "{w} {mode:?}: the left pill"
        );
        let mid = r.app.mixer.middle_seen();
        let dock = r.app.mixer.right_dock_seen();
        assert!(
            dock.left() - mid.right() <= 8.5,
            "{w} {mode:?}: no gap before the dock at the end ({mid:?}, {dock:?})"
        );
    }
}

/// r39:fix2 — the overflow's slot is as wide as its count needs: a one-digit count's slot
/// is narrower than a two-digit one's (30 pt and 36 in the shipped face), so a strip that
/// missed by a few points beside a 36-pt slot fits beside a one-digit pill. Across a run of
/// widths (fails before: at some of them the room beside a one-digit pill held a strip).
#[test]
fn the_overflow_slot_fits_its_count() {
    let mode = WidthMode::Wide;
    let pitch = mode.pitch();
    let slot = |n_strips: usize, w: f32| -> Option<(usize, f32, egui::Rect)> {
        let mut r = Rig::new(grown(n_strips), vec2(w, 700.0));
        mixer::set_width_mode(&mut r.app, mode);
        r.idle(3);
        let p = r
            .ctx
            .read_response(egui::Id::new(("mixer-title", "more-right")))
            .map(|p| p.rect)?;
        let texts = r.texts();
        let n: usize = texts
            .iter()
            .filter(|(_, at)| p.contains(at.center()))
            .find_map(|(t, _)| t.trim().parse().ok())?;
        let mid = r.app.mixer.middle_seen();
        let dock = r.app.mixer.right_dock_seen();
        Some((n, dock.left() - 8.0 - mid.right(), p))
    };
    let k = (17..40)
        .find(|k| slot(*k, 1280.0).is_some_and(|(n, _, _)| n < 10))
        .expect("a desk with a one-digit overflow");
    let mut one = None;
    for w in (1240..1320).map(|w| w as f32) {
        let Some((n, room, p)) = slot(k, w) else {
            continue;
        };
        if n >= 10 {
            continue;
        }
        let need = p.width() + 4.0;
        assert!(
            room >= need - 0.5,
            "{w}: the slot {room} under its pill {p:?}"
        );
        assert!(
            room - need < pitch - 0.5,
            "{w}: {room} pt beside a {n} pill would hold another strip"
        );
        one = Some(need);
    }
    let one = one.expect("one-digit overflows across the widths");
    let (n, _, p) = slot(96, 1280.0).expect("a desk with a two-digit overflow");
    assert!(n >= 10);
    let two = p.width() + 4.0;
    assert!(
        one + 3.0 < two,
        "a one-digit slot ({one}) under a two-digit one ({two})"
    );
    assert!(one <= 31.0, "one digit needs about 30 pt: {one}");
}

//! The mixer desk (ADR 0024 §6), driven the way a user drives it: frames with pointer,
//! wheel and key events, and what the desk, the hint bar and the project say afterwards.

use cantus_core::{Command, Project, Send, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::kit::hint;
use cantus_ui::views::mixer::{self, layout::WidthMode, Control};
use cantus_ui::{starter_project, views, welcome_project, CantusApp};
use egui::{pos2, vec2, Event, Modifiers, MouseWheelUnit, Pos2, TouchPhase};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
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

/// The legend names every row once; the strips carry no repeated words.
#[test]
fn the_legend_names_each_row_once_and_strips_carry_no_words() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 900.0));
    let text = r.idle(2);
    for word in [
        "NUMBER",
        "NAME",
        "LEVEL",
        "PAN",
        "VOLUME",
        "FROM SELECTED",
        "WIDTH",
        "ROUTING",
    ] {
        let n = text.matches(&format!("| {word} |")).count()
            + usize::from(text.starts_with(&format!("{word} |")));
        assert_eq!(n, 1, "{word} is named {n} times: {text}");
    }
    for gone in ["| M |", "| S |", "| FX |", "| Mono |", "| SENDS |"] {
        let n = text.matches(gone).count();
        // "M" is the master's number cell, once.
        assert!(n <= 1, "{gone} appears {n} times: {text}");
    }
    // Every strip is numbered, the master first.
    let m = text.find("| MASTER |").expect("the master");
    let one = text.find("| 1 |").expect("strip 1");
    assert!(m < one, "the master is at the left: {text}");
    assert!(text.contains("| 16 |"), "sixteen strips: {text}");
    // The fader scale is printed once, in the legend.
    for mark in [
        "+12",
        "\u{2212}12",
        "\u{2212}30",
        "\u{2212}60",
        "\u{2212}\u{221e}",
    ] {
        assert_eq!(
            text.matches(&format!("| {mark} |")).count(),
            1,
            "{mark}: {text}"
        );
    }
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
    // Scrolled far enough, the sends row shows.
    for _ in 0..6 {
        r.wheel(-3.0, Modifiers::NONE);
    }
    let s1 = r.app.project().strips[0].id;
    r.app.select_strip(r.app.project().strips[1].id);
    r.idle(1);
    let send = r.rect(Some(s1), Control::Send);
    assert!(send.bottom() <= 330.0, "the sends are reachable: {send:?}");
}

/// A tall desk shows every row and needs no scrolling.
#[test]
fn a_tall_desk_shows_every_row() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
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

/// Wide shows each fader's value and its output; Extra large the chain in one line and
/// the strip's own sends — no insert slots (round 25, ADR 0041 §9: they are the
/// inspector's).
#[test]
fn wider_modes_show_more() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    let compact = r.idle(2);
    assert!(!compact.contains("| VALUE |") && !compact.contains("| OUTPUT |"));
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let wide = r.idle(2);
    assert!(wide.contains("| VALUE |"), "{wide}");
    assert!(wide.contains("| +0.0 |"), "the fader's value: {wide}");
    assert!(wide.contains("| \u{2192} M |"), "the output chip: {wide}");
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    let s0 = r.app.project().strips[0].id;
    r.app.add_effect(s0, cantus_core::EffectKind::Eq);
    r.app.add_effect(s0, cantus_core::EffectKind::Compressor);
    r.app.add_effect(s0, cantus_core::EffectKind::Limiter);
    let xl = r.idle(2);
    assert!(
        xl.contains("| FX CHAIN |") && xl.contains("| SENDS |"),
        "{xl}"
    );
    assert!(
        xl.contains("| EQ\u{2009}\u{b7}\u{2009}Comp\u{2009}\u{b7}\u{2009}+1 |"),
        "the owner's chain line: {xl}"
    );
    assert!(!xl.contains("Aequalis"), "no slot names the insert: {xl}");
    assert!(mixer::control_rect(&r.ctx, Some(s0), Control::Chain).is_some());
    assert!(mixer::control_rect(&r.ctx, Some(s0), Control::SendAdd).is_some());
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
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
        r.ctx.set_pixels_per_point(ppp);
        let text = r.idle(2);
        assert!(text.contains("| PANEO |"), "{text}");
        assert!(!text.contains('\u{2026}'), "nothing elided: {text}");
        assert!(text.contains("| 1 |"));
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// The master strip's pan law is on the desk, as a value with its unit (Compact's narrow
/// master: "−3 dB"; Wide and up: "−3 dB law" — never a bare number).
#[test]
fn the_master_carries_the_pan_law() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    let text = r.idle(2);
    assert!(text.contains("| \u{2212}3 dB |"), "{text}");
    r.app.exec(Command::SetPanLaw {
        law: cantus_core::PanLaw::Minus6dB,
    });
    assert!(r.idle(1).contains("| \u{2212}6 dB |"));
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
    assert!((send.from.x - port(&r, ids[1]).x).abs() < 0.5, "{send:?}");
    assert!((send.to.x - port(&r, ids[14]).x).abs() < 0.5, "{send:?}");
    assert!(send.lit && send.chips == [false, false]);
    let master = drawn(&r, Route::output(ids[1], StripId::MASTER));
    assert!(master.is_some(), "the selection's master route is drawn");
    assert_eq!(r.app.mixer.cables_drawn(), 2);
    // Another strip selected: its master route alone; strip 2's send is not drawn.
    r.app.select_strip(ids[5]);
    r.idle(2);
    assert!(drawn(&r, Route::send(ids[1], ids[14])).is_none());
    assert_eq!(r.app.mixer.cables_drawn(), 1);
    // All: every route but the implied master ones, those away from the selection faint.
    set_cables(&mut r, CableView::All);
    r.idle(2);
    let send = drawn(&r, Route::send(ids[1], ids[14])).expect("drawn in All");
    assert!(!send.lit, "away from the selection: faint");
    assert!(drawn(&r, Route::output(ids[1], StripId::MASTER)).is_none());
    assert_eq!(r.app.mixer.cables_drawn(), 2);
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
    let low = cantus_ui::kit::cable::path(doubled.from, doubled.to, doubled.depth);
    // Hover the doubled cable's flat run, away from its pill.
    r.hover(egui::pos2(low.run_start.x + 6.0, low.run_start.y));
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
    let p = cantus_ui::kit::cable::path(out.from, out.to, out.depth);
    r.click(p.run_start + vec2(4.0, 0.0), Modifiers::NONE);
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
    let p = cantus_ui::kit::cable::path(out.from, out.to, out.depth);
    r.click(p.run_start + vec2(4.0, 0.0), Modifiers::NONE);
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
    assert!(text.contains("| \u{2192} M |"), "the output chip: {text}");
}

/// Design test 15: in Spanish and English, at 75 % and 200 %, Wide and Extra large, the
/// band's pills overlap neither each other nor the ports, and the band's words fit.
#[test]
fn the_band_reads_without_overlaps_at_every_scale_and_language() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for ppp in [0.75f32, 2.0] {
            for mode in [WidthMode::Wide, WidthMode::Xl] {
                let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0) / ppp);
                cantus_ui::i18n::set_locale(locale);
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
                let mut rows = 0;
                if mode == WidthMode::Xl {
                    for id in &ids {
                        for k in 0..2u8 {
                            let (Some(row), Some(chip)) = (
                                mixer::control_rect(&r.ctx, Some(*id), Control::SendRow(k)),
                                mixer::tap_chip_rect(&r.ctx, *id, k),
                            ) else {
                                continue;
                            };
                            rows += 1;
                            let inside: Vec<&(String, egui::Rect)> = texts
                                .iter()
                                .filter(|(_, tr)| row.contains(tr.center()))
                                .collect();
                            assert!(
                                inside.iter().any(|(t, _)| t == "PRE" || t == "POST"),
                                "{locale:?} {ppp}: the tap word whole in {inside:?}"
                            );
                            for (t, tr) in &inside {
                                if t != "PRE" && t != "POST" {
                                    assert!(
                                        tr.right() <= chip.left() + 0.5,
                                        "{locale:?} {ppp}: {t:?} runs under the chip"
                                    );
                                }
                            }
                        }
                    }
                    assert!(rows > 0 || ppp > 1.0, "{locale:?} {ppp}: send rows checked");
                }
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
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
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
    let p = cantus_ui::kit::cable::path(out.from, out.to, out.depth);
    r.click(p.run_start + vec2(4.0, 0.0), Modifiers::NONE);
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
    r.click(p.run_start + vec2(4.0, 0.0), Modifiers::NONE);
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
    let p = cantus_ui::kit::cable::path(out.from, out.to, out.depth);
    r.click(p.run_start + vec2(4.0, 0.0), Modifiers::NONE);
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
    // desk's to take.
    r.idle(1);
    r.click(p.run_start + vec2(4.0, 0.0), Modifiers::NONE);
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

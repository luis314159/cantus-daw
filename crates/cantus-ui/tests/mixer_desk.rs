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
    for word in ["NUMBER", "NAME", "LEVEL", "PAN", "VOLUME", "SENDS", "WIDTH"] {
        let n = text.matches(&format!("| {word} |")).count()
            + usize::from(text.starts_with(&format!("{word} |")));
        assert_eq!(n, 1, "{word} is named {n} times: {text}");
    }
    for gone in ["| M |", "| S |", "| FX |", "| Mono |", "| SENDS | SENDS"] {
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

/// Wide shows each fader's value; Extra large the insert slots, empty ones as wells.
#[test]
fn wider_modes_show_more() {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    let compact = r.idle(2);
    assert!(!compact.contains("| VALUE |") && !compact.contains("| INSERTS |"));
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let wide = r.idle(2);
    assert!(wide.contains("| VALUE |"), "{wide}");
    assert!(wide.contains("| +0.0 |"), "the fader's value: {wide}");
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    let s0 = r.app.project().strips[0].id;
    r.app.add_effect(s0, cantus_core::EffectKind::Eq);
    let xl = r.idle(2);
    assert!(xl.contains("| INSERTS |"), "{xl}");
    assert!(xl.contains("Aequalis"), "the insert's name: {xl}");
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

/// The master strip's pan law is on the desk, as a value.
#[test]
fn the_master_carries_the_pan_law() {
    let mut r = Rig::new(starter_project(), vec2(1400.0, 1000.0));
    let text = r.idle(2);
    assert!(text.contains("| -3 |"), "{text}");
    r.app.exec(Command::SetPanLaw {
        law: cantus_core::PanLaw::Minus6dB,
    });
    assert!(r.idle(1).contains("| -6 |"));
    let _ = Send::new;
}

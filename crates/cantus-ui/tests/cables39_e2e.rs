//! The routing cables' three looks (round 39, r39:el-cables; des-mixer's `options-cables-*`,
//! the owner's choice of 2026-11-09: *Flow* by default, *Patch* and *Subway* selectable),
//! held through `CantusApp::assemble` on the demo song the program opens with, the mixer
//! maximised as a user shows it:
//! - each style chosen in the Cables key's menu draws the selection's cables in its shape;
//!   *Show all* draws every route (the others at 45 %) and a bus's way to the master;
//! - hovering a cable lights its whole route to the master and dims the rest;
//! - dragging from an output jack: a valid target says "Route Chords to Drum bus", a loop
//!   "Refused: Kick already feeds Drum bus (a loop)" and a release does nothing;
//! - Flow's gleam moves only while the transport plays with signal, and asks for no frame
//!   of its own (none at rest);
//! - the style is a setting: chosen in Settings ▸ Appearance too, kept across a restart.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::routing::Route;
use cantus_core::StripId;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::settings::CableStyle;
use cantus_ui::views::mixer::{self, layout::CableView, layout::WidthMode, Control, DrawnCable};
use cantus_ui::views::workspace_ticker::{Reason, Tick};
use cantus_ui::CantusApp;
use egui::{vec2, Event, Modifiers, Pos2, Rect};
use std::path::Path;
use std::time::Duration;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    proc: Processor,
    t: f64,
    pointer: Option<Pos2>,
    mods: Modifiers,
    /// The repaint the last frame asked for.
    repaint: Duration,
    /// What the ticker decided at the last frame's end (its reasons, its request).
    tick: Tick,
}

/// Nothing moves: no reason for a frame this tick (a deadline far off — the autosave's —
/// is not a frame).
fn quiet(t: &Tick) -> bool {
    t.reasons == 0 && t.requested.is_none_or(|d| d >= Duration::from_secs(1))
}

fn boot(dir: &Path, first: bool) -> Rig {
    if first {
        std::fs::write(
            dir.join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
    }
    std::env::set_var("CANTUS_DATA_DIR", dir);
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::demos::startup_project()),
        engine,
        None,
        None,
        None,
        false,
    );
    app.show_central(CentralView::Mixer);
    app.workspace
        .toggle_fold(cantus_ui::views::workspace::Pane::Arrangement);
    let mut r = Rig {
        app,
        ctx,
        proc,
        t: 0.0,
        pointer: None,
        mods: Modifiers::NONE,
        repaint: Duration::ZERO,
        tick: Tick::default(),
    };
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    r.frames(4);
    r
}

impl Rig {
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        // The engine runs offline: a block of audio a frame (silence while stopped).
        let mut audio = vec![0f32; 800 * 2];
        self.proc.process(&mut audio, 2);
        let mut ev = vec![Event::ModifiersChanged(self.mods)];
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        // As the program runs it: eframe's `ui`, whose end asks the ticker for the one
        // repaint the frame needs (none at rest).
        let mut out = self.ctx.run_ui(input, |ui| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(app, ui, &mut frame);
        });
        out.textures_delta.clear();
        self.tick = cantus_ui::views::workspace_ticker::last(&self.ctx);
        self.repaint = out
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(Duration::MAX);
        // Where each text is seen: its rect inside its clip (a row scrolled out of a page
        // is painted but not seen — integration of round 39, the Settings page grew).
        fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()).intersect(clip),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
                _ => {}
            }
        }
        let mut v = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, c.clip_rect, &mut v);
        }
        v
    }

    fn frames(&mut self, n: usize) -> Vec<(String, Rect)> {
        let mut t = Vec::new();
        for _ in 0..n {
            t = self.frame(Vec::new());
        }
        t
    }

    fn button(&mut self, at: Pos2, pressed: bool) {
        self.pointer = Some(at);
        let modifiers = self.mods;
        self.frame(vec![Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        }]);
    }

    fn click(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(Vec::new());
        self.button(at, true);
        self.button(at, false);
        self.frames(2);
    }

    /// Click the painted words `word` (a menu row, a segment) where they are seen; a word
    /// painted but scrolled out of its page is wheeled into view first (from the middle of
    /// the window, where a dialog's page is).
    fn click_word(&mut self, word: &str) {
        for _ in 0..30 {
            let texts = self.frames(1);
            // The topmost (painted last): a dialog's word over the window's same word.
            if let Some(at) = texts
                .iter()
                .rev()
                .find(|(t, r)| t == word && r.is_positive())
                .map(|(_, r)| r.center())
            {
                self.click(at);
                return;
            }
            assert!(
                texts.iter().any(|(t, _)| t == word),
                "{word:?} is not painted: {texts:?}"
            );
            self.pointer = Some(Pos2::new(960.0, 540.0));
            self.frame(vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: vec2(0.0, -3.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }]);
            // The page glides to its new place: let it settle before looking again (round
            // 39, themes: Settings ▸ Appearance grew its theme pictures, and the cables'
            // row is now below the fold).
            self.frames(12);
        }
        panic!("{word:?} never came into view");
    }

    fn strip(&self, name: &str) -> StripId {
        let p = self.app.project();
        p.strips
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.id)
            .or_else(|| {
                p.tracks
                    .iter()
                    .find(|t| t.name == name)
                    .and_then(|t| t.strip)
            })
            .unwrap_or_else(|| panic!("no strip {name}"))
    }

    fn name(&self, id: StripId) -> String {
        let p = self.app.project();
        p.tracks
            .iter()
            .find(|t| t.strip == Some(id))
            .map(|t| t.name.clone())
            .or_else(|| p.strip(id).map(|s| s.name.clone()))
            .unwrap_or_default()
    }

    fn hint(&self) -> String {
        cantus_ui::kit::hint::current(&self.ctx)
            .map(|h| format!("{} {}", h.name, h.value))
            .unwrap_or_default()
    }

    fn drawn(&self, route: Route) -> Option<DrawnCable> {
        self.app
            .mixer
            .cables()
            .iter()
            .find(|d| d.route == route)
            .copied()
    }

    /// The Cables key on (the selection's cables), then `style` from its options menu.
    fn cables_in(&mut self, style: CableStyle) {
        if self.app.mixer.prefs().cables == CableView::Off {
            let key = mixer::title_rect(&self.ctx, "cables").expect("the Cables key");
            self.click(key.center());
        }
        let opts = mixer::title_rect(&self.ctx, "cable-options").expect("the options key");
        self.click(opts.center());
        self.click_word(&cantus_ui::i18n::t(style.key()));
        // The options menu stays open for its switches; a second click on its key closes it.
        self.click(opts.center());
        self.frames(2);
    }
}

/// The point of a drawn cable farthest from every other one (where a user would point at
/// it), away from its two ends.
fn clear_point(r: &Rig, d: &DrawnCable) -> Pos2 {
    let path = r.app.mixer.cable_path(d);
    let n = path.len();
    let mut best = (path[n / 2], f32::MIN);
    for &p in &path[n / 5..n - n / 5] {
        let near = r
            .app
            .mixer
            .cables()
            .iter()
            .filter(|o| o.route != d.route)
            .flat_map(|o| r.app.mixer.cable_path(o).windows(2))
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                let ab = b - a;
                let u = ((p - a).dot(ab) / ab.length_sq().max(1e-9)).clamp(0.0, 1.0);
                p.distance(a + ab * u)
            })
            .fold(f32::MAX, f32::min);
        if near > best.1 {
            best = (p, near);
        }
    }
    best.0
}

/// The Kick's routes on the demo song: its output to the drum bus, keys into the pumped
/// strips (Chords, Bass, Choir).
fn kick_selected(r: &mut Rig) -> (StripId, StripId) {
    let kick = r.strip("Kick");
    let bus = r
        .app
        .project()
        .strip(kick)
        .and_then(|s| s.output)
        .expect("the kick goes to the drum bus");
    r.app.select_strip(kick);
    r.frames(2);
    (kick, bus)
}

/// Each style, chosen in the Cables key's menu: the selection's cables drawn in its shape
/// (a hanging curve whose deepest point is 0.75·k below the jacks; a subway route that
/// runs down, across in its lane and up); Show all draws every route, the others at 45 %,
/// and the drum bus's way to the master.
#[test]
fn each_style_draws_the_selections_cables_and_show_all() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = boot(dir.path(), true);
    assert_eq!(
        r.app.settings.cable_style, None,
        "none chosen: the theme's own"
    );
    let sys = cantus_ui::kit::Style::current().t.system;
    assert_eq!(sys.cables, CableStyle::Flow, "Slate's own: Flow");
    assert_eq!(
        r.app.settings.cable_style_in(&sys),
        CableStyle::Flow,
        "the default"
    );
    assert!(r.app.mixer.cables().is_empty(), "nothing at rest");
    let (kick, bus) = kick_selected(&mut r);
    for style in CableStyle::ALL {
        r.cables_in(style);
        assert_eq!(
            r.app.settings.cable_style,
            Some(style),
            "chosen in the menu"
        );
        assert_eq!(r.app.mixer.prefs().cables, CableView::Selected);
        let out = r
            .drawn(Route::output(kick, bus))
            .unwrap_or_else(|| panic!("{style:?}: the kick's output drawn"));
        assert!(out.lit && out.alpha == 1.0, "{style:?}: {out:?}");
        assert!(
            r.app
                .mixer
                .cables()
                .iter()
                .all(|d| d.route.from == kick || d.route.to == kick),
            "{style:?}: only the kick's cables"
        );
        let keys = r
            .app
            .mixer
            .cables()
            .iter()
            .filter(|d| matches!(d.route.kind, cantus_core::routing::RouteKind::Key(_)))
            .count();
        assert!(keys >= 2, "{style:?}: the kick's keys drawn: {keys}");
        let path = r.app.mixer.cable_path(&out).to_vec();
        assert_eq!((path[0], *path.last().unwrap()), (out.from, out.to));
        let low = path.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        match style {
            CableStyle::Patch | CableStyle::Flow => {
                assert!(
                    (low - (out.from.y + 0.75 * out.depth)).abs() < 0.05,
                    "{style:?}"
                );
                assert!(path.len() == 25, "a cubic in 24 segments");
            }
            CableStyle::Jack => {
                // A catenary from each seated plug's boot, its lowest point 3 + s below.
                assert!(
                    (low - (out.from.y + 3.0 + out.depth)).abs() < 0.5,
                    "{style:?} {low}"
                );
                let nut = mixer::control_rect(&r.ctx, Some(kick), Control::Port).unwrap();
                assert!(
                    (out.from.y - nut.center().y - 22.0).abs() < 0.5,
                    "from the boot"
                );
            }
            CableStyle::Subway => {
                assert!((low - (out.from.y + out.depth)).abs() < 0.01, "at its lane");
                let flat = path
                    .windows(2)
                    .filter(|w| (w[0].y - w[1].y).abs() < 1e-3)
                    .count();
                let upright = path
                    .windows(2)
                    .filter(|w| (w[0].x - w[1].x).abs() < 1e-3)
                    .count();
                assert!(flat >= 1 && upright >= 2, "orthogonal runs: {path:?}");
            }
        }
        // Show all: every route; the others at 45 %; the bus's way to the master.
        let opts = mixer::title_rect(&r.ctx, "cable-options").unwrap();
        r.click(opts.center());
        r.click_word(&cantus_ui::i18n::t("cables.show_all"));
        r.click(opts.center());
        r.frames(2);
        assert_eq!(r.app.mixer.prefs().cables, CableView::All);
        let all = r.app.mixer.cables();
        assert!(all.len() > 8, "{style:?}: every route: {}", all.len());
        assert!(all.iter().any(|d| !d.lit && (d.alpha - 0.45).abs() < 1e-3));
        assert!(
            r.drawn(Route::output(bus, StripId::MASTER)).is_some(),
            "{style:?}: the drum bus's way to the master"
        );
        let mut p = r.app.mixer.prefs();
        p.cables = CableView::Selected;
        r.app.mixer.set_prefs(p);
        r.frames(2);
    }
}

/// Hovering the Kick → Drum bus cable lights its whole route (Kick → Drum bus → Master),
/// drops the rest to 22 % and names the route; in every style.
#[test]
fn hovering_a_cable_lights_its_route_to_the_master() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = boot(dir.path(), true);
    let (kick, bus) = kick_selected(&mut r);
    for style in CableStyle::ALL {
        r.cables_in(style);
        let out = r
            .drawn(Route::output(kick, bus))
            .expect("the kick's output");
        // The point of it farthest from every other cable drawn.
        r.pointer = Some(clear_point(&r, &out));
        r.frames(3);
        let chain = r
            .drawn(Route::output(bus, StripId::MASTER))
            .unwrap_or_else(|| panic!("{style:?}: the bus's way to the master drawn"));
        assert!(chain.chain && chain.alpha == 1.0, "{style:?}: {chain:?}");
        let out = r.drawn(Route::output(kick, bus)).unwrap();
        assert_eq!(out.alpha, 1.0, "{style:?}: the hovered cable whole");
        let others: Vec<_> = r
            .app
            .mixer
            .cables()
            .iter()
            .filter(|d| !d.chain && d.route != out.route)
            .collect();
        assert!(!others.is_empty() && others.iter().all(|d| (d.alpha - 0.22).abs() < 1e-3));
        let words = format!(
            "{} \u{2192} {} \u{2192} {}",
            r.name(kick),
            r.name(bus),
            cantus_ui::i18n::t("mixer.master")
        );
        let h = r.hint();
        assert!(
            h.contains(&words),
            "{style:?}: the hint names the route: {h} / {words}"
        );
        let painted = r.frames(1);
        assert!(
            painted.iter().any(|(t, _)| *t == words),
            "{style:?}: the tip by the pointer"
        );
        r.pointer = Some(Pos2::new(-50.0, -50.0));
        r.frames(3);
        assert!(r.app.mixer.cables().iter().all(|d| d.alpha >= 0.45));
    }
}

/// Carry a cable from `from`'s output jack to `to`'s input jack with Ctrl (the output),
/// held there; returns the words beside the pointer (the hint).
fn carry(r: &mut Rig, from: StripId, to: StripId) -> String {
    let a = mixer::control_rect(&r.ctx, Some(from), Control::Port)
        .expect("the output jack")
        .center();
    let z = mixer::control_rect(&r.ctx, Some(to), Control::Port)
        .expect("the target's jacks")
        .center()
        - vec2(22.0, 0.0);
    r.mods = Modifiers::CTRL;
    r.pointer = Some(a);
    r.frame(Vec::new());
    r.button(a, true);
    for k in 1..=8 {
        r.pointer = Some(a + (z - a) * (k as f32 / 8.0));
        r.frame(Vec::new());
    }
    r.frames(2);
    r.hint()
}

/// Drag to connect, in every style: from the Chords' output jack to the drum bus's input
/// jack routes the Chords there ("Route Chords to Drum bus"); from the drum bus to the
/// kick is a loop — refused, said, and a release issues nothing.
#[test]
fn dragging_from_a_jack_routes_or_says_why_not() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = boot(dir.path(), true);
    let (kick, bus) = kick_selected(&mut r);
    let chords = r.strip("Chords");
    for style in CableStyle::ALL {
        r.cables_in(style);
        let before = r.app.project().clone();
        let words = carry(&mut r, chords, bus);
        let want = format!("Route {} to {}", r.name(chords), r.name(bus));
        assert!(words.contains(&want), "{style:?}: {words}");
        r.button(r.pointer.unwrap(), false);
        r.mods = Modifiers::NONE;
        r.frames(2);
        assert_eq!(
            r.app.project().strip(chords).unwrap().output,
            Some(bus),
            "{style:?}: routed"
        );
        r.app.undo();
        assert!(*r.app.project() == before, "{style:?}: one undo step");
        r.frames(2);
        let words = carry(&mut r, bus, kick);
        let want = format!(
            "Refused: {} already feeds {} (a loop)",
            r.name(kick),
            r.name(bus)
        );
        assert!(words.contains(&want), "{style:?}: {words}");
        r.button(r.pointer.unwrap(), false);
        r.mods = Modifiers::NONE;
        r.frames(2);
        assert!(
            *r.app.project() == before,
            "{style:?}: a refused drop issues nothing"
        );
    }
}

/// Flow: the gleam travels only while the transport plays with signal (Patch and Subway
/// never draw one); it asks for no frame of its own — the frames it rides are the ones
/// the playing desk asks for with the cables off — and the desk at rest asks for none.
#[test]
fn flow_moves_only_while_playing_and_asks_for_no_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = boot(dir.path(), true);
    kick_selected(&mut r);
    r.cables_in(CableStyle::Flow);
    // The pointer leaves the window (an event every frame would ask egui for frames).
    r.pointer = None;
    r.frame(vec![Event::PointerGone]);
    let mut p = r.app.mixer.prefs();
    p.cables = CableView::All;
    r.app.mixer.set_prefs(p);
    r.frames(40);
    assert_eq!(r.app.mixer.cable_gleams(), 0, "stopped: nothing moves");
    assert!(quiet(&r.tick), "at rest: no frame asked for: {:?}", r.tick);
    r.app.toggle_play();
    r.frames(30);
    assert!(
        r.app.mixer.cable_gleams() > 0,
        "playing with signal: the gleams"
    );
    let playing_on = r.tick.requested;
    assert!(
        playing_on.is_some_and(|d| d >= Duration::from_millis(15)),
        "at the ticker's rate, not the screen's: {:?}",
        r.tick
    );
    // The same desk with the cables off asks for the same frames.
    let mut p = r.app.mixer.prefs();
    p.cables = CableView::Off;
    r.app.mixer.set_prefs(p);
    r.frames(2);
    assert_eq!(
        r.tick.requested, playing_on,
        "the cables ask for no frame of their own"
    );
    p.cables = CableView::All;
    r.app.mixer.set_prefs(p);
    // Patch and Subway: no gleam while playing.
    for style in [CableStyle::Patch, CableStyle::Subway] {
        r.app.settings.cable_style = Some(style);
        r.frames(2);
        assert_eq!(r.app.mixer.cable_gleams(), 0, "{style:?}");
    }
    r.app.settings.cable_style = Some(CableStyle::Flow);
    r.app.toggle_play();
    r.frames(2);
    assert_eq!(r.app.mixer.cable_gleams(), 0, "stopped again");
    // The meters fall, then the desk is quiet with every cable shown.
    r.frames(300);
    assert!(
        quiet(&r.tick),
        "the stopped desk asks for nothing: {:?}",
        r.tick
    );
}

/// The style is a setting of this machine: chosen in Settings ▸ Appearance as in the
/// Cables menu, and the same after a restart.
#[test]
fn the_style_is_a_setting_kept_across_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut r = boot(dir.path(), true);
        r.cables_in(CableStyle::Subway);
        assert_eq!(r.app.settings.cable_style, Some(CableStyle::Subway));
    }
    {
        let mut r = boot(dir.path(), false);
        assert_eq!(r.app.settings.cable_style, Some(CableStyle::Subway), "kept");
        cantus_ui::views::workspace_settings::open(
            &mut r.app,
            cantus_ui::views::workspace_settings::SettingsPage::Appearance,
        );
        r.frames(3);
        r.click_word(&cantus_ui::i18n::t("cables.style.patch"));
        assert_eq!(
            r.app.settings.cable_style,
            Some(CableStyle::Patch),
            "Settings ▸ Appearance"
        );
    }
    let r = boot(dir.path(), false);
    assert_eq!(
        r.app.settings.cable_style,
        Some(CableStyle::Patch),
        "kept again"
    );
    let saved = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
    assert!(saved.contains("\"cable_style\""), "{saved}");
}

/// D, "Jack": a cord dropped on its jack settles — its sag rings down for 300 ms, the
/// display's rate asked for meanwhile — then is exactly its sag and still, and the desk
/// asks for nothing more; each route's catenary is solved once (the same shape every
/// frame at rest).
#[test]
fn a_dropped_jack_cord_settles_then_rests() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = boot(dir.path(), true);
    let (_, bus) = kick_selected(&mut r);
    let chords = r.strip("Chords");
    r.app.select_strip(chords);
    r.cables_in(CableStyle::Jack);
    r.pointer = None;
    r.frame(vec![Event::PointerGone]);
    r.frames(40);
    assert!(quiet(&r.tick), "at rest before: {:?}", r.tick);
    carry(&mut r, chords, bus);
    r.button(r.pointer.unwrap(), false);
    r.mods = Modifiers::NONE;
    r.pointer = None;
    let route = Route::output(chords, bus);
    let mut sags = Vec::new();
    let mut asked = Vec::new();
    for _ in 0..30 {
        r.frame(vec![Event::PointerGone]);
        sags.push(r.drawn(route).expect("the new cord drawn").depth);
        asked.push(r.tick);
    }
    let rest = *sags.last().unwrap();
    assert!(
        sags[..12].iter().any(|s| (s - rest).abs() > 0.01 * rest),
        "it settles: {sags:?}"
    );
    assert!(
        asked[..12].iter().all(|t| t.has(Reason::Animation)),
        "a frame each tick while it settles: {asked:?}"
    );
    assert_eq!(
        sags[25..].iter().copied().fold(rest, f32::max),
        rest,
        "still from 300 ms"
    );
    let still = r.app.mixer.cable_path(&r.drawn(route).unwrap()).to_vec();
    for _ in 0..30 {
        r.frame(Vec::new());
    }
    assert!(quiet(&r.tick), "at rest: no frame asked for: {:?}", r.tick);
    assert_eq!(
        r.app.mixer.cable_path(&r.drawn(route).unwrap()),
        &still[..],
        "the same cord at rest"
    );
}

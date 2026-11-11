//! The owner's four choices for the strip (round 39, 2026-11-09; des-mixer's
//! `options-keys-cables-*`), held through `CantusApp::assemble` on the demo song the program
//! opens with, the mixer shown as a user shows it:
//! 1. keys, option C — M|S one pill, every target 24 tall at least; the arm a dot in the head;
//!    polarity and swap one pill, the width knob in the pan row;
//! 2. cables, option C with B on demand — nothing drawn at rest, "N routes" under a strip
//!    with several, the lit strip's destinations tagged by kind; the header's Cables key
//!    shows the curves;
//! 3. a send's level is a small knob (Wide, Large), a dot that opens one (Compact);
//! 4. a keyed insert's key says "← Kick" and lights its source.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::{Command, EffectKind, Send, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::app::{CentralView, Files};
use cantus_ui::views::mixer::{self, layout::CableView, layout::WidthMode, Control};
use cantus_ui::CantusApp;
use egui::{vec2, Event, Pos2, Rect};

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    pointer: Option<Pos2>,
    _dir: tempfile::TempDir,
}

fn boot() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    std::env::set_var("CANTUS_DATA_DIR", dir.path());
    let files = Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, _proc) = EngineHandle::manual(48_000.0);
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
    // The mixer takes the window under the top bar, as the pictures show it.
    app.workspace
        .toggle_fold(cantus_ui::views::workspace::Pane::Arrangement);
    let mut r = Rig {
        app,
        ctx,
        t: 0.0,
        pointer: None,
        _dir: dir,
    };
    r.frames(4);
    r
}

impl Rig {
    fn frame(&mut self, events: Vec<Event>) -> String {
        self.t += 1.0 / 60.0;
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1600.0))),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        let mut text = String::new();
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
        for c in &out.shapes {
            walk(&c.shape, &mut text);
        }
        text
    }

    fn frames(&mut self, n: usize) -> String {
        let mut t = String::new();
        for _ in 0..n {
            t = self.frame(Vec::new());
        }
        t
    }

    fn rect(&self, strip: Option<StripId>, c: Control) -> Rect {
        mixer::control_rect(&self.ctx, strip, c).unwrap_or_else(|| panic!("{c:?} of {strip:?}"))
    }

    fn click(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(Vec::new());
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        self.frames(2);
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

    fn hint(&self) -> String {
        cantus_ui::kit::hint::current(&self.ctx)
            .map(|h| format!("{} {}", h.name, h.value))
            .unwrap_or_default()
    }
}

/// The owner's choice 1, option C's keys: M|S one pill of two 24-tall targets side by side (Compact stacked);
/// the arm a 24 × 24 dot in the head; polarity and swap one pill; width in the pan row.
#[test]
fn keys_are_option_c() {
    let mut r = boot();
    let chords = r.strip("Chords");
    for mode in [WidthMode::Wide, WidthMode::Xl, WidthMode::Compact] {
        mixer::set_width_mode(&mut r.app, mode);
        r.frames(3);
        let (m, s) = (
            r.rect(Some(chords), Control::Mute),
            r.rect(Some(chords), Control::Solo),
        );
        assert!(
            m.height() >= 23.0 && s.height() >= 23.0,
            "{mode:?}: {m:?} {s:?}"
        );
        assert!(
            m.width() >= 24.0 && s.width() >= 24.0,
            "{mode:?}: {m:?} {s:?}"
        );
        if mode == WidthMode::Compact {
            assert!(s.top() >= m.bottom() - 0.5, "Compact: M over S");
        } else {
            assert!(
                (m.top() - s.top()).abs() < 0.5 && (s.left() - m.right()).abs() < 0.5,
                "{mode:?}: one pill"
            );
            let (ph, sw) = (
                r.rect(Some(chords), Control::Phase),
                r.rect(Some(chords), Control::Swap),
            );
            assert!(
                (ph.top() - sw.top()).abs() < 0.5 && (sw.left() - ph.right()).abs() < 1.5,
                "{mode:?}: polarity and swap one pill"
            );
            let (pan, width) = (
                r.rect(Some(chords), Control::Pan),
                r.rect(Some(chords), Control::Width),
            );
            assert!(
                (pan.center().y - width.center().y).abs() < 0.5,
                "{mode:?}: width in the pan row"
            );
        }
    }
    // The arm: in the head of every strip a track plays into, always there (grey), red
    // and armed after a click.
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    r.frames(3);
    for name in ["Kick", "Chords", "Lead"] {
        let id = r.strip(name);
        let head = r.rect(Some(id), Control::Head);
        let arm = r.rect(Some(id), Control::Record);
        assert!(
            arm.width() >= 24.0 && arm.height() >= 24.0,
            "{name}: {arm:?}"
        );
        assert!(
            head.intersects(arm),
            "{name}: the arm is in the head: {arm:?} {head:?}"
        );
    }
    let kick = r.strip("Kick");
    let armed = |r: &Rig| r.app.project().tracks_on_strip(kick).all(|t| t.armed);
    assert!(!armed(&r));
    let arm = r.rect(Some(kick), Control::Record);
    r.click(arm.center());
    assert!(armed(&r), "a click arms the kick's track");
}

/// The owner's choice 2, option C's cables, B on demand: nothing at rest, "N routes" under a strip with
/// several, its destinations tagged by kind when it is selected; the Cables key draws them.
#[test]
fn cables_are_option_c_with_b_on_demand() {
    let mut r = boot();
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    assert_eq!(
        r.app.mixer.prefs().cables,
        CableView::Off,
        "nothing drawn at rest"
    );
    let (kick, bass, reverb) = (r.strip("Kick"), r.strip("Bass"), r.strip("Reverb"));
    r.app.set_strip_send(kick, Send::new(reverb));
    r.app.end_gesture();
    // Bass's compressor keyed from the kick.
    r.app.add_effect(bass, EffectKind::Compressor);
    let fx = r
        .app
        .project()
        .strip(bass)
        .unwrap()
        .effects
        .last()
        .unwrap()
        .id;
    r.app.exec(Command::SetEffectSidechain {
        strip: bass,
        effect: fx,
        source: Some(kick),
    });
    let text = r.frames(3);
    assert!(r.app.mixer.cables().is_empty(), "no cable at rest");
    let p = r.app.project();
    let k = p.strip(kick).unwrap();
    let keys = p
        .strips
        .iter()
        .filter(|o| o.effects.iter().any(|e| e.sidechain == Some(kick)))
        .count();
    let n = usize::from(k.output.is_some()) + k.sends.len() + keys;
    assert!(n >= 3, "out, a send and a key: {n}");
    let words = format!("{n} routes");
    assert!(text.contains(&words), "under the kick, {words}: {text}");
    r.app.select_strip(kick);
    let text = r.frames(3);
    assert!(text.contains("> send"), "the reverb tagged: {text}");
    assert!(text.contains("> key"), "the bass tagged: {text}");
    assert!(text.contains("> out"), "the drum bus tagged: {text}");
    // The Cables key: the selection's curves.
    let key = mixer::title_rect(&r.ctx, "cables").expect("the Cables key");
    r.click(key.center());
    assert_eq!(r.app.mixer.prefs().cables, CableView::Selected);
    r.frames(2);
    assert!(
        !r.app.mixer.cables().is_empty(),
        "the selection's cables drawn"
    );
}

/// The owner's choice 3: a send's level is a small knob: dragging it moves the level (Wide); Compact's dot
/// opens one.
#[test]
fn a_send_is_a_small_knob() {
    let mut r = boot();
    let (kick, reverb) = (r.strip("Kick"), r.strip("Reverb"));
    let sends = r.app.project().strip(kick).unwrap().sends.len();
    if sends == 0 {
        r.app.set_strip_send(kick, Send::new(reverb));
        r.app.end_gesture();
    }
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    r.frames(3);
    let row = r.rect(Some(kick), Control::SendRow(0));
    let start = r.app.project().strip(kick).unwrap().sends[0].level;
    let at = row.center();
    r.pointer = Some(at);
    r.frame(vec![Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    for k in 1..=5 {
        r.pointer = Some(at + vec2(0.0, 6.0 * k as f32));
        r.frame(Vec::new());
    }
    r.frame(vec![Event::PointerButton {
        pos: r.pointer.unwrap(),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(
        r.app.project().strip(kick).unwrap().sends[0].level < start,
        "dragged down: quieter"
    );
    // Compact: the dot opens the knob.
    mixer::set_width_mode(&mut r.app, WidthMode::Compact);
    r.frames(3);
    assert!(
        mixer::control_rect(&r.ctx, Some(kick), Control::SendRow(0)).is_none(),
        "closed at rest"
    );
    let dot = mixer::send_dot_rect(&r.ctx, kick, 0).expect("the send's dot");
    r.click(dot.center());
    assert!(
        mixer::control_rect(&r.ctx, Some(kick), Control::SendRow(0)).is_some(),
        "its knob opened"
    );
}

/// The owner's choice 4: a keyed insert's key says where it listens and lights that strip.
#[test]
fn a_keyed_insert_names_its_source() {
    let mut r = boot();
    let (kick, bass) = (r.strip("Kick"), r.strip("Bass"));
    r.app.add_effect(bass, EffectKind::Compressor);
    let fx = r
        .app
        .project()
        .strip(bass)
        .unwrap()
        .effects
        .last()
        .unwrap()
        .id;
    r.app.exec(Command::SetEffectSidechain {
        strip: bass,
        effect: fx,
        source: Some(kick),
    });
    mixer::set_width_mode(&mut r.app, WidthMode::Xl);
    let text = r.frames(3);
    let kick_name = r
        .app
        .project()
        .tracks
        .iter()
        .find(|t| t.strip == Some(kick))
        .map(|t| t.name.clone())
        .unwrap();
    assert!(text.contains(&kick_name), "Large names the source: {text}");
    let k = r
        .app
        .project()
        .strip(bass)
        .unwrap()
        .effects
        .iter()
        .position(|e| e.sidechain == Some(kick))
        .expect("the keyed insert");
    let key = mixer::key_rect(&r.ctx, bass, k).expect("the key glyph");
    r.pointer = Some(key.center());
    r.frames(3);
    let h = r.hint();
    assert!(
        h.contains(&format!("\u{2190} {kick_name}")),
        "the hint: {h}"
    );
    assert_eq!(r.app.mixer.key_lit_strip(), Some(kick), "its source lit");
}

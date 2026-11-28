//! Round 40, `look`: the owner's answers on the look, each a setting of this machine with
//! his answer as its default (Settings ▸ Appearance ▸ *The window*), held through
//! `CantusApp::assemble` on the demo song the program opens with, chosen the way a user
//! chooses them — a click on the segment's words — and kept across a restart:
//! - **maximise keeps** the inspector beside the area by default; *Nothing* gives the area
//!   the whole workspace;
//! - **the top bar** brings back the automation arm and the typing keyboard's key, each
//!   doing what its key does; the default bar has neither; in Spanish at 1920 × 1080 the
//!   bar with both keeps every group whole (on it or in its panel);
//! - **the mixer's right dock** sits at the window's edge with the next empty strips
//!   before it by default, or just after the last strip;
//! - **the inspector's values** are value bars by default, round 39's slim bars by choice;
//! - **Mute on** lights a muted strip's M in the accent by default, grey by choice.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::views::owner_look::{DockPlace, MaximiseKeeps, MuteOn, ValueStyle};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_settings::{open, SettingsPage};
use cantus_ui::CantusApp;
use egui::{vec2, Color32, Event, Pos2, Rect};
use std::path::Path;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    proc: Processor,
    t: f64,
    pointer: Option<Pos2>,
    size: egui::Vec2,
    /// The colours of the last frame's shapes.
    colours: Vec<Color32>,
}

fn data() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    d
}

/// One test at a time: the data folder (`CANTUS_DATA_DIR`), the locale and the installed
/// theme are the process's.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn boot(dir: &Path, locale: cantus_ui::i18n::Locale) -> Rig {
    std::env::set_var("CANTUS_DATA_DIR", dir);
    let files = Files::read(dir, Some(locale));
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
    app.workflow.start.close();
    let mut r = Rig {
        app,
        ctx,
        proc,
        t: 0.0,
        pointer: None,
        size: vec2(1920.0, 1080.0),
        colours: Vec::new(),
    };
    r.frames(4);
    r
}

/// The colours a shape paints with.
fn colours_of(shape: &egui::Shape, out: &mut Vec<Color32>) {
    use egui::Shape;
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| colours_of(s, out)),
        Shape::Mesh(m) => out.extend(m.vertices.iter().map(|v| v.color)),
        Shape::Rect(r) => out.extend([r.fill, r.stroke.color]),
        Shape::Circle(c) => out.extend([c.fill, c.stroke.color]),
        Shape::Path(p) => out.push(p.fill),
        _ => {}
    }
}

impl Rig {
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let mut audio = vec![0f32; 800 * 2];
        self.proc.process(&mut audio, 2);
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        // As the program runs it: eframe's `ui`.
        let mut out = self.ctx.run_ui(input, |ui| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(app, ui, &mut frame);
        });
        out.textures_delta.clear();
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
        self.colours.clear();
        for c in &out.shapes {
            walk(&c.shape, c.clip_rect, &mut v);
            colours_of(&c.shape, &mut self.colours);
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

    fn click(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(Vec::new());
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        self.pointer = None;
        self.frames(2);
    }

    /// Click the painted words `word` where they are seen (the topmost: a dialog's over
    /// the window's); a word scrolled out of Settings' page is wheeled into view first.
    fn click_word(&mut self, word: &str) {
        let mut last: Option<Pos2> = None;
        for _ in 0..40 {
            let texts = self.frames(1);
            if let Some(at) = texts
                .iter()
                .rev()
                .find(|(t, r)| t == word && r.is_positive())
                .map(|(_, r)| r.center())
            {
                // Clicked once it stands still (the page settles its scroll first).
                if last == Some(at) {
                    self.click(at);
                    return;
                }
                last = Some(at);
                continue;
            }
            assert!(
                texts.iter().any(|(t, _)| t == word),
                "{word:?} is not painted: {:?}",
                texts.iter().map(|t| &t.0).collect::<Vec<_>>()
            );
            self.pointer = Some(Pos2::new(self.size.x * 0.5, self.size.y * 0.5));
            self.frame(vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: vec2(0.0, -3.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        panic!("{word:?} never came into view");
    }

    /// Settings ▸ Appearance open, `word` clicked, Settings closed.
    fn choose(&mut self, word: &str) {
        open(&mut self.app, SettingsPage::Appearance);
        self.frames(30);
        let was = self.app.settings.look.clone();
        // The headless pointer's first press on a freshly shown page is sometimes taken by
        // the window coming forward (a click on the segment again is the same choice). The
        // macOS runner, slower, once let all of three tries pass with no change and the test
        // read the old setting later (rehearsal 46147025464): more tries, and a choice that
        // never lands fails here, where it happened.
        let mut changed = false;
        for _ in 0..8 {
            self.click_word(word);
            self.frames(3);
            if self.app.settings.look != was {
                changed = true;
                break;
            }
            let texts = self.frames(1);
            let at: Vec<_> = texts.iter().filter(|(t, _)| t == word).collect();
            eprintln!("choose({word:?}): no change yet; seen at {at:?}");
        }
        assert!(changed, "choose({word:?}): eight clicks changed nothing");
        self.app.show_settings = false;
        self.frames(3);
    }

    /// The areas on the main surface now.
    fn areas(&self) -> Vec<Pane> {
        self.app
            .workspace
            .areas_in_order()
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    }
}

#[test]
fn maximise_keeps_the_inspector_unless_the_setting_gives_the_whole_workspace() {
    let _one = serial();
    let dir = data();
    let mut r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.maximise, MaximiseKeeps::Inspector);
    let before = r.areas();
    assert!(before.contains(&Pane::Arrangement) && before.contains(&Pane::Inspector));
    assert!(before.len() > 2, "more areas than the two: {before:?}");
    let insp_w = r
        .app
        .workspace
        .areas_in_order()
        .into_iter()
        .find(|(p, _)| *p == Pane::Inspector)
        .map(|(_, rect)| rect.width())
        .unwrap();

    // Ctrl+Alt+Enter's action on the arrangement: the inspector stays beside it, at its
    // width, on the same side; the rest fold to the docks' row.
    r.app.workspace.reveal(Pane::Arrangement);
    r.app.run_action(cantus_ui::keymap::Action::MaximizeArea);
    r.frames(3);
    assert_eq!(r.app.workspace.maximised(), Some(Pane::Arrangement));
    assert!(r.app.workspace.inspector_kept());
    let now = r.app.workspace.areas_in_order();
    let panes: Vec<Pane> = now.iter().map(|a| a.0).collect();
    assert_eq!(panes, vec![Pane::Arrangement, Pane::Inspector], "{panes:?}");
    let (arr, insp) = (now[0].1, now[1].1);
    assert!(
        insp.left() >= arr.right() - 1.0,
        "the inspector at its side"
    );
    assert!(
        (insp.width() - insp_w).abs() < 24.0,
        "the inspector keeps its width: {} for {insp_w}",
        insp.width()
    );
    let hidden = r.app.workspace.hidden_by_maximise();
    assert!(!hidden.contains(&Pane::Inspector), "{hidden:?}");
    assert!(!hidden.is_empty());
    // The inspector's key hides it alone: the arrangement stays maximised.
    r.app.run_action(cantus_ui::keymap::Action::ToggleInspector);
    r.frames(2);
    assert_eq!(r.app.workspace.maximised(), Some(Pane::Arrangement));
    assert_eq!(r.areas(), vec![Pane::Arrangement]);
    // The same again gives everything back as it was, but the inspector the user hid.
    r.app.run_action(cantus_ui::keymap::Action::MaximizeArea);
    r.frames(2);
    assert_eq!(r.app.workspace.maximised(), None);
    r.app.run_action(cantus_ui::keymap::Action::ToggleInspector);
    r.frames(2);
    let mut back = r.areas();
    back.sort_by_key(|p| format!("{p:?}"));
    let mut was = before.clone();
    was.sort_by_key(|p| format!("{p:?}"));
    assert_eq!(back, was);

    // Settings ▸ Appearance ▸ Maximise keeps: Nothing — the whole workspace.
    r.choose("Nothing: the whole workspace");
    assert_eq!(r.app.settings.look.maximise, MaximiseKeeps::Nothing);
    r.app.workspace.reveal(Pane::Arrangement);
    r.app.run_action(cantus_ui::keymap::Action::MaximizeArea);
    r.frames(3);
    assert_eq!(r.areas(), vec![Pane::Arrangement]);
    assert!(r
        .app
        .workspace
        .hidden_by_maximise()
        .contains(&Pane::Inspector));
    // Chosen back while maximised: the inspector comes back beside it at once.
    r.choose("The inspector beside it");
    assert_eq!(r.app.workspace.maximised(), Some(Pane::Arrangement));
    assert_eq!(r.areas(), vec![Pane::Arrangement, Pane::Inspector]);
    r.choose("Nothing: the whole workspace");
    r.app.workspace.restore_maximised();

    // Kept across a restart.
    drop(r);
    let r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.maximise, MaximiseKeeps::Nothing);
    assert_eq!(r.app.workspace.maximise_keeps(), MaximiseKeeps::Nothing);
}

#[test]
fn the_top_bar_brings_back_the_automation_arm_and_the_typing_key_by_the_setting() {
    let _one = serial();
    let dir = data();
    let mut r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    let arm = egui::Id::new("toolbar-automation-arm");
    let typing = egui::Id::new(("toolbar-key", "typing"));
    // The default bar: neither.
    assert!(!r.app.settings.look.topbar_automation && !r.app.settings.look.topbar_typing);
    r.frames(2);
    assert!(r.ctx.read_response(arm).is_none());
    assert!(r.ctx.read_response(typing).is_none());

    // Settings ▸ Appearance ▸ Top bar extras: Both.
    r.choose("Both");
    assert!(r.app.settings.look.topbar_automation && r.app.settings.look.topbar_typing);
    r.frames(3);
    let a = r
        .ctx
        .read_response(arm)
        .expect("the arm key is on the bar")
        .rect;
    let bar = cantus_ui::views::transport::HEIGHT;
    assert!(a.bottom() <= bar + 1.0, "{a:?}");
    // Each does what its key does.
    let armed = r.app.auto_armed();
    r.click(a.center());
    assert_ne!(r.app.auto_armed(), armed, "a click arms the automation");
    // The typing key leads the inputs: on the bar, or — where the bar folds the inputs at
    // this width — in More's Inputs section.
    let (_, layout) = cantus_ui::views::transport::last_layout(&r.ctx).expect("the bar");
    eprintln!(
        "1920 × 1080, English, both extras: folded {:?}",
        layout.folded()
    );
    let k = match r.ctx.read_response(typing) {
        Some(k) => {
            assert!(k.rect.bottom() <= bar + 1.0);
            k.rect
        }
        None => {
            assert!(
                layout
                    .folded()
                    .contains(&cantus_ui::views::transport::layout::Group::Inputs),
                "the typing key is on the bar or folded with the inputs"
            );
            cantus_ui::views::transport::open_overflow(&mut r.app);
            r.frames(3);
            r.ctx
                .read_response(typing)
                .expect("the typing key in More's Inputs")
                .rect
        }
    };
    let on = r.app.keyboard_piano;
    r.click(k.center());
    assert_ne!(
        r.app.keyboard_piano, on,
        "a click turns the typing keyboard on"
    );
    r.app
        .run_action(cantus_ui::keymap::Action::ToggleTypingPiano);

    // Only the arm, then only the typing key.
    r.choose("Automation arm");
    assert!(r.app.settings.look.topbar_automation && !r.app.settings.look.topbar_typing);
    r.frames(2);
    assert!(r.ctx.read_response(arm).is_some());
    assert!(r.ctx.read_response(typing).is_none());
    r.choose("Typing keyboard");
    r.frames(2);
    assert!(r.ctx.read_response(arm).is_none());
    assert!(r.ctx.read_response(typing).is_some());
    r.choose("Both");

    // Kept across a restart — and in Spanish at 1920 × 1080, 100 %, the bar with both
    // keeps every group whole: on the bar, or folded into its panel.
    drop(r);
    let mut r = boot(dir.path(), cantus_ui::i18n::Locale::Es);
    assert!(r.app.settings.look.topbar_automation && r.app.settings.look.topbar_typing);
    r.frames(3);
    let (rect, layout) = cantus_ui::views::transport::last_layout(&r.ctx).expect("the bar");
    assert!(rect.width() >= 1900.0);
    let placed = layout.placed();
    for (i, p) in placed.iter().enumerate() {
        assert!(p.span.width() > 0.0, "{p:?}");
        for q in &placed[i + 1..] {
            assert!(
                p.span.x1 <= q.span.x0 + 0.5 || q.span.x1 <= p.span.x0 + 0.5,
                "{p:?} overlaps {q:?}"
            );
        }
    }
    let on_bar: Vec<_> = placed.iter().map(|p| p.group).collect();
    assert!(
        on_bar.contains(&cantus_ui::views::transport::layout::Group::Transport),
        "the transport (and its arm) stays on the bar: {on_bar:?}"
    );
    let a = r
        .ctx
        .read_response(arm)
        .expect("the arm key in Spanish")
        .rect;
    assert!(a.right() <= rect.right());
}

#[test]
fn the_mixers_right_dock_sits_at_the_edge_or_after_the_last_strip() {
    let _one = serial();
    let dir = data();
    let mut r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.mixer_dock, DockPlace::Edge);
    r.app.show_central(CentralView::Mixer);
    r.app.workspace.toggle_fold(Pane::Arrangement);
    r.frames(4);
    // The demo song has its returns docked right.
    let dock = r.app.mixer.right_dock_seen();
    let rows = r.app.mixer.rows_seen();
    assert!(
        dock.width() > 0.0,
        "the demo song docks its returns at the right"
    );
    assert!(
        rows.right() - dock.right() < 24.0,
        "pinned at the window's edge: {dock:?} in {rows:?}"
    );
    let middle = r.app.mixer.middle_seen();
    assert!(
        dock.left() - middle.right() > 60.0,
        "room before it: {middle:?} {dock:?}"
    );
    assert!(
        r.app.mixer.next_strips_shown() > 0,
        "filled with the next strips"
    );

    r.choose("After the last strip");
    assert_eq!(r.app.settings.look.mixer_dock, DockPlace::AfterStrips);
    r.frames(3);
    let after = r.app.mixer.right_dock_seen();
    let middle = r.app.mixer.middle_seen();
    assert!(after.left() < dock.left() - 60.0, "{after:?} for {dock:?}");
    assert!(after.left() - middle.right() < 24.0, "{middle:?} {after:?}");

    drop(r);
    let r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.mixer_dock, DockPlace::AfterStrips);
}

/// How many value bars the inspector painted last frame (inspect mode's registry).
fn value_bars(r: &Rig) -> usize {
    cantus_ui::kit::inspect::items(&r.ctx)
        .iter()
        .filter(|i| i.part == "value_bar")
        .count()
}

#[test]
fn the_inspector_shows_value_bars_unless_the_setting_keeps_the_slim_ones() {
    let _one = serial();
    let dir = data();
    let mut r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.inspector_values, ValueStyle::Bars);
    cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
    // A strip chosen: the inspector's Level shows its volume, pan and width.
    cantus_ui::views::inspector::unfold(&r.ctx, "inspector.level");
    let strip = r.app.project().tracks[0].strip.expect("a strip");
    r.app.select_strip(strip);
    r.frames(4);
    assert!(
        value_bars(&r) >= 2,
        "the Level section's values are value bars"
    );
    r.choose("Slim bars");
    assert_eq!(r.app.settings.look.inspector_values, ValueStyle::Slim);
    cantus_ui::views::inspector::unfold(&r.ctx, "inspector.level");
    r.app.select_strip(strip);
    r.frames(4);
    assert_eq!(value_bars(&r), 0, "round 39's slim bars");
    drop(r);
    let r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.inspector_values, ValueStyle::Slim);
}

#[test]
fn a_muted_strips_m_is_lit_in_colour_unless_the_setting_says_grey() {
    let _one = serial();
    let dir = data();
    let mut r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.mute_on, MuteOn::Colour);
    r.app.show_central(CentralView::Mixer);
    r.app.workspace.toggle_fold(Pane::Arrangement);
    r.frames(4);
    let accent = cantus_ui::kit::tokens::current().highlight;
    let count = |r: &Rig, c: Color32| r.colours.iter().filter(|x| **x == c).count();
    let lit_before = count(&r, accent);
    // Two strips muted, as the M key mutes them.
    let ids: Vec<_> = r
        .app
        .project()
        .strips
        .iter()
        .skip(1)
        .take(2)
        .map(|s| s.id)
        .collect();
    for id in &ids {
        r.app.exec(cantus_core::Command::SetStripMute {
            id: *id,
            muted: true,
        });
    }
    r.frames(3);
    assert!(
        count(&r, accent) >= lit_before + 2,
        "each muted strip's M lit in the accent"
    );
    r.choose("Grey");
    assert_eq!(r.app.settings.look.mute_on, MuteOn::Grey);
    r.frames(3);
    let lit_grey = count(&r, accent);
    assert!(
        lit_grey < lit_before + 2,
        "grey: the accent is not on the M keys"
    );
    drop(r);
    let r = boot(dir.path(), cantus_ui::i18n::Locale::En);
    assert_eq!(r.app.settings.look.mute_on, MuteOn::Grey);
}

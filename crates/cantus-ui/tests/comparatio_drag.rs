//! Comparatio's editor answers the pointer as its picture promises (ADR 0146 provisional,
//! phase B), driven frame by frame with real pointer events through the window the app
//! draws:
//!
//! - the big selector and the lamps switch what is heard, the blind drawer's A, B and X
//!   keys too — each press one undo step;
//! - the reference's numbered keys compare each reference of the list, *Remove* takes the
//!   one compared off it, *Load reference…* adds a file (a WAV the test writes, picked as
//!   the dialog would) and compares it — each one undo step, undone to the list it was;
//! - a file dropped on the console is added and compared, one undo step;
//! - a drag across the overview loops that stretch of the reference, one undo step;
//! - *Send to Aequalis* puts the match EQ just before the unit (one undo step), and a second
//!   send updates that Aequalis instead of adding another. // fxc:comparatio

use cantus_core::fx::comparatio::{Reference, References};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::views::fx::comparatio::Act;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const KIND: EffectKind = EffectKind::Reference;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    views::fx::frame::shell_of(KIND).set_scale(&ctx, 1.0);
    ctx
}

fn open(kv: &[(&str, f32)]) -> (CantusApp, egui::Vec2) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("comparatio");
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    views::fx::demo_feed(&mut app, KIND);
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, "comparatio")
            .set_many(kv, "cmd.fx_preset");
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let size = views::fx::frame::designed_size(KIND);
    settle(&mut app, size);
    (app, size)
}

fn window(app: &mut CantusApp, ui: &mut egui::Ui) {
    let c = ui.ctx().clone();
    views::fx::window(app, &c);
}

/// A few quiet frames in a context of their own: the window's rects are known after.
fn settle(app: &mut CantusApp, size: egui::Vec2) {
    shoot_events(&ctx(), size, 1.0, 6, None, |_, _| Vec::new(), |ui| window(app, ui));
}

fn press(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// A click at `at`: pressed and let go a frame apart.
fn click(app: &mut CantusApp, size: egui::Vec2, at: Pos2) {
    shoot_events(
        &ctx(),
        size,
        1.0,
        10,
        None,
        |f, _| match f {
            2 => vec![Event::PointerMoved(at)],
            3 => vec![press(at, true)],
            4 => vec![press(at, false)],
            _ => Vec::new(),
        },
        |ui| window(app, ui),
    );
}

/// Press at `from`, move to `to` in steps, let go — in a settled context of its own.
fn drag(app: &mut CantusApp, size: egui::Vec2, from: Pos2, to: Pos2) {
    let (steps, q) = (12, 4);
    shoot_events(
        &ctx(),
        size,
        1.0,
        q + steps + 6,
        None,
        |f, _| match f {
            f if f < q => Vec::new(),
            f if f == q => vec![Event::PointerMoved(from)],
            f if f == q + 1 => vec![press(from, true)],
            f if f < q + steps + 2 => {
                let t = (f - q - 1) as f32 / steps as f32;
                vec![Event::PointerMoved(from + (to - from) * t)]
            }
            f if f == q + steps + 2 => vec![press(to, false)],
            _ => Vec::new(),
        },
        |ui| window(app, ui),
    );
}

/// A file dropped by the system at `at` (what the window's backend hands egui).
#[derive(Debug)]
struct Dropped(PathBuf);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        Err("not read".to_string())
    }
}

fn drop_file(app: &mut CantusApp, size: egui::Vec2, at: Pos2, file: &Path) {
    let c = ctx();
    for f in 0..6 {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(f as f64 / 60.0),
            ..Default::default()
        };
        if f == 2 {
            input.events.push(Event::PointerMoved(at));
        }
        if f == 3 {
            input.dropped_files.push(Arc::new(Dropped(file.to_path_buf())));
        }
        let _ = c.run_ui(input, |ui| window(app, ui));
    }
}

fn spec(app: &CantusApp) -> EffectSpec {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.clone()
}

fn compared(app: &CantusApp) -> Option<String> {
    spec(app).ir.map(|f| f.path)
}

fn listed(app: &CantusApp) -> Vec<String> {
    let s = spec(app);
    cantus_core::fx::comparatio::listed(s.refs.as_deref(), s.ir.as_deref())
        .into_iter()
        .map(|r| r.path)
        .collect()
}

/// The console's spot doing `act`.
fn spot(app: &CantusApp, act: Act) -> Pos2 {
    app.fx
        .comparatio
        .spots
        .iter()
        .find(|s| s.act == act)
        .unwrap_or_else(|| panic!("no spot for {act:?}"))
        .rect
        .center()
}

fn wav(dir: &Path, name: &str, hz: f32) -> PathBuf {
    let path = dir.join(name);
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..48_000 {
        let v = (0.2
            * cantus_dsp::math::sin(i as f32 / 48_000.0 * hz * std::f32::consts::TAU)
            * 32_767.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
    path
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantus-comparatio-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_selector_lamps_and_blind_keys_each_one_undo_step() {
    let (mut app, size) = open(&[]);
    assert_eq!(spec(&app).get("listen"), 0.0, "your mix, at its defaults");
    let cases: [(Act, f32); 6] = [
        (Act::Swap, 1.0),
        (Act::Listen(1.0), 1.0),
        (Act::Listen(2.0), 2.0),
        (Act::Listen(0.0), 0.0),
        (Act::Listen(1.0), 1.0),
        (Act::Listen(2.0), 2.0),
    ];
    for (i, (act, want)) in cases.iter().enumerate() {
        // The other value first where the press would not move anything.
        if (spec(&app).get("listen") - want).abs() < 0.5 {
            let (strip, effect, _) = app.fx.open.unwrap();
            app.exec(Command::SetEffectParam {
                strip,
                effect,
                key: "listen".into(),
                value: if *want == 0.0 { 1.0 } else { 0.0 },
            });
        }
        let before = spec(&app).get("listen");
        settle(&mut app, size);
        let at = match (i, act) {
            // The lamps' spots and the keys share a value: the first Listen(1) is the lamp.
            (1, Act::Listen(_)) => app.fx.comparatio.spots[2].rect.center(),
            _ => spot(&app, *act),
        };
        click(&mut app, size, at);
        assert_eq!(spec(&app).get("listen"), *want, "{act:?}");
        app.undo();
        assert_eq!(spec(&app).get("listen"), before, "{act:?}: one press, one undo");
    }
}

#[test]
fn the_reference_list_answers_its_keys_each_one_undo_step() {
    let (mut app, size) = open(&[]);
    let three = listed(&app);
    assert_eq!(three.len(), 3, "the demo's list: {three:?}");
    assert_eq!(compared(&app).as_deref(), Some(three[0].as_str()));
    // The numbered keys: the third, then the second.
    for i in [2usize, 1] {
        settle(&mut app, size);
        let before = compared(&app);
        let at = spot(&app, Act::Select(i));
    click(&mut app, size, at);
        assert_eq!(compared(&app).as_deref(), Some(three[i].as_str()), "key {}", i + 1);
        assert_eq!(listed(&app), three, "the list kept");
        app.undo();
        assert_eq!(compared(&app), before, "one press, one undo");
    }
    // Remove: the one compared off the list, the next compared in its place.
    settle(&mut app, size);
    let at = spot(&app, Act::Remove);
    click(&mut app, size, at);
    assert_eq!(listed(&app), vec![three[1].clone(), three[2].clone()]);
    assert_eq!(compared(&app).as_deref(), Some(three[1].as_str()));
    app.undo();
    assert_eq!(listed(&app), three);
    assert_eq!(compared(&app).as_deref(), Some(three[0].as_str()));
    // Load reference…: a file picked (as the dialog would give it) added and compared.
    let dir = temp("load");
    let file = wav(&dir, "Picked master.wav", 220.0);
    views::fx::comparatio::pick_for_tests(vec![file.clone()]);
    settle(&mut app, size);
    let at = spot(&app, Act::Load);
    click(&mut app, size, at);
    let now = listed(&app);
    assert_eq!(now.len(), 4);
    let stored = now[3].clone();
    assert!(stored.ends_with("Picked master.wav"), "{stored}");
    assert_eq!(compared(&app).as_deref(), Some(stored.as_str()));
    // Its print taken as it was loaded.
    let s = spec(&app);
    let entry: &Reference = &s.refs.as_ref().unwrap().list[3];
    let (bytes, hash) = cantus_engine::fx::comparatio_store::fingerprint(&file).unwrap();
    assert_eq!((entry.bytes, entry.hash.as_str()), (bytes, hash.as_str()));
    app.undo();
    assert_eq!(listed(&app), three, "one load, one undo");
    // A file dropped on the console: added and compared, one undo step.
    let dropped = wav(&dir, "Dropped master.wav", 330.0);
    settle(&mut app, size);
    let inside = app.fx.comparatio.display.expect("drawn").center();
    drop_file(&mut app, size, inside, &dropped);
    let now = listed(&app);
    assert_eq!(now.len(), 4, "{now:?}");
    assert!(compared(&app).unwrap().ends_with("Dropped master.wav"));
    app.undo();
    assert_eq!(listed(&app), three, "one drop, one undo");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_drag_across_the_overview_loops_that_stretch_one_undo_step() {
    let (mut app, size) = open(&[]);
    assert_eq!(spec(&app).get("sync"), 0.0);
    // The overview's window: under the name line, along the strip.
    let r = app
        .fx
        .comparatio
        .overview
        .expect("the overview is laid out");
    let from = egui::pos2(r.left() + r.width() * 0.25, r.center().y);
    let to = egui::pos2(r.left() + r.width() * 0.75, r.center().y);
    drag(&mut app, size, from, to);
    let s = spec(&app);
    assert_eq!(s.get("sync"), 2.0, "the loop's playback");
    // The demo's reference is 12 s long: about 3 s to 9 s (the glass's margins aside).
    assert!((s.get("lstart") - 3.0).abs() < 0.4, "{}", s.get("lstart"));
    assert!((s.get("lend") - 9.0).abs() < 0.4, "{}", s.get("lend"));
    app.undo();
    let s = spec(&app);
    assert_eq!(
        (s.get("sync"), s.get("lstart"), s.get("lend")),
        (0.0, 0.0, 0.0),
        "one drag, one undo"
    );
}

#[test]
fn send_to_aequalis_puts_the_match_eq_before_the_unit_and_updates_it() {
    let (mut app, size) = open(&[]);
    let (strip, effect, _) = app.fx.open.unwrap();
    let chain = |app: &CantusApp| -> Vec<(EffectId, EffectKind)> {
        app.project()
            .effects_of(strip)
            .unwrap()
            .iter()
            .map(|e| (e.id, e.spec.kind))
            .collect()
    };
    let before = chain(&app);
    assert!(app.fx.comparatio.pic.match_eq.is_some(), "the suggestion is drawn");
    let at = spot(&app, Act::SendEq);
    click(&mut app, size, at);
    let after = chain(&app);
    assert_eq!(after.len(), before.len() + 1);
    let at = after.iter().position(|(id, _)| *id == effect).unwrap();
    assert_eq!(after[at - 1].1, EffectKind::Eq, "an Aequalis just before");
    let eq = after[at - 1].0;
    let refs: References = *spec(&app).refs.unwrap();
    assert_eq!(refs.eq, Some(eq), "the unit knows where it went");
    let s = app.project().effect(strip, eq).unwrap().spec.clone();
    assert!(s.get("b0.used") >= 0.5, "fitted bands: {:?}", s.params);
    // The list kept as it was.
    assert_eq!(refs.list.len(), 3);
    // A second send updates that Aequalis.
    settle(&mut app, size);
    {
        let mut h = views::fx::host::DawHost::new(&mut app, strip, effect, "comparatio");
        h.set("eqamt", 1.0, None);
    }
    settle(&mut app, size);
    let at = spot(&app, Act::SendEq);
    click(&mut app, size, at);
    assert_eq!(chain(&app), after, "updated in place, not added");
    let s2 = app.project().effect(strip, eq).unwrap().spec.clone();
    assert_ne!(s2.params, s.params, "the full amount draws another curve");
    app.undo();
    assert_eq!(app.project().effect(strip, eq).unwrap().spec, s, "one undo step");
    app.undo(); // the amount
    app.undo(); // the first send
    assert_eq!(chain(&app), before, "the first send undone at once");
    assert!(spec(&app).refs.unwrap().eq.is_none());
}

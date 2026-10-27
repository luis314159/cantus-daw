//! Gravis's editor (on the kit's shell, ADR 0024): every engine opens in Play, Edit and
//! Matrix and paints its name and its own picture; a knob gesture, a drag on a cell, a drag
//! on the drawing and an engine switch are one undo step each; the factory presets set what
//! they name, name their macros, and come back in one undo; every label has its words in
//! English and Spanish; and no word is cut, elided or painted over another at 75 % and
//! 200 %, in English and Spanish.

mod support;
use cantus_core::inst::bass::PARAMS;
use cantus_core::*;
use cantus_ui::kit::shell::{self, Bands};
use cantus_ui::kit::{Function, Style};
use cantus_ui::views::inst::{bass, preset_spec, View};
use egui::{Pos2, Rect, Vec2};
use support::*;

/// The locale is global: these tests take turns.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn with_bass() -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Bass)),
    );
    (a, track)
}

fn spec(a: &cantus_ui::CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("a native instrument")
}

fn paint_inst(app: &mut cantus_ui::CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(1400.0, 1000.0), 3, |ui| {
        cantus_ui::views::inst::window_in(app, ui);
    })
}

// ---- what a frame painted ------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Word {
    text: String,
    rect: Rect,
    clip: Rect,
}

fn words(shapes: &[egui::epaint::ClippedShape]) -> Vec<Word> {
    fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<Word>) {
        match shape {
            egui::Shape::Text(t) if !t.galley.text().trim().is_empty() => out.push(Word {
                text: t.galley.text().to_string(),
                rect: t.visual_bounding_rect(),
                clip,
            }),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut out);
    }
    out
}

fn frame(
    ctx: &egui::Context,
    size: Vec2,
    time: f64,
    events: Vec<egui::Event>,
    mut draw: impl FnMut(&mut egui::Ui),
) -> Vec<egui::epaint::ClippedShape> {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        time: Some(time),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| draw(ui));
    out.textures_delta.clear();
    out.shapes
}

fn window_frames(
    ctx: &egui::Context,
    a: &mut cantus_ui::CantusApp,
    size: Vec2,
    steps: Vec<Vec<egui::Event>>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut shapes = Vec::new();
    for (i, events) in steps.into_iter().enumerate() {
        shapes = frame(ctx, size, i as f64 * 0.05, events, |ui| {
            cantus_ui::views::inst::window_in(a, ui);
        });
    }
    shapes
}

fn view_area(k: f32, view: View) -> Rect {
    let s = Style::current().scaled(k);
    let win = Rect::from_min_size(Pos2::ZERO, shell::INSTRUMENT * k);
    let body = Rect::from_min_max(
        Pos2::new(win.left(), win.top() + shell::HEADER * k),
        Pos2::new(win.right(), win.bottom() - shell::FOOTER * k),
    );
    let mut g = shell::Grid::new(body, &s);
    let _ = shell::layout_bands(
        &mut g,
        &s,
        &Bands {
            macros_open: true,
            mods_open: true,
            keyboard: view == View::Play,
        },
    );
    g.rest()
}

fn faults(all: &[Word], area: Rect) -> Vec<String> {
    let inside: Vec<&Word> = all
        .iter()
        .filter(|w| area.expand(0.5).contains_rect(w.rect))
        .collect();
    let mut bad = Vec::new();
    for w in &inside {
        if w.text.contains('\u{2026}') {
            bad.push(format!("elided: {:?} at {:?}", w.text, w.rect.min));
        }
        if !w.clip.expand(0.75).contains_rect(w.rect) {
            bad.push(format!("cut by its clip: {:?} at {:?}", w.text, w.rect.min));
        }
    }
    for (i, a) in inside.iter().enumerate() {
        for b in &inside[i + 1..] {
            let x = a.rect.intersect(b.rect);
            if x.width() > 1.0 && x.height() > 1.0 {
                bad.push(format!("{:?} over {:?} at {:?}", a.text, b.text, x.min));
            }
        }
    }
    bad
}

// ---- the tests -----------------------------------------------------------------------------

#[test]
fn every_engine_opens_in_every_view_with_its_picture() {
    let _turn = turn();
    let pictures = [
        (0.0, "Pitch over time"),
        (1.0, "Filter response"),
        (2.0, "From above, to scale"),
    ];
    for (engine, picture) in pictures {
        let (mut a, track) = with_bass();
        a.set_inst_value(track, "engine", engine, false);
        a.open_inst_editor(track);
        for view in [View::Play, View::Edit, View::Matrix] {
            a.inst.view = view;
            let text = paint_inst(&mut a);
            assert!(text.contains("GRAVIS"), "engine {engine} {view:?}: {text}");
            if view != View::Matrix {
                assert!(
                    text.contains(picture) || text.contains(&picture.to_uppercase()),
                    "engine {engine} in {view:?} did not draw its picture: {text}"
                );
                for seg in ["808", "Synth", "String", "Legato", "Mono"] {
                    assert!(text.contains(seg), "engine {engine} {view:?}: no {seg}");
                }
            }
        }
        a.close_inst_editor();
    }
}

#[test]
fn a_knob_gesture_is_one_undo_step() {
    let _turn = turn();
    let (mut a, track) = with_bass();
    for v in [300.0, 450.0, 700.0] {
        a.set_inst_value(track, "syn.cutoff", v, true);
    }
    a.end_gesture();
    assert_eq!(spec(&a, track).get("syn.cutoff"), 700.0);
    a.undo();
    assert!(
        !spec(&a, track).is_stored("syn.cutoff"),
        "one undo step back to the default"
    );
}

#[test]
fn dragging_a_cell_is_one_undo_step() {
    let _turn = turn();
    let (mut a, track) = with_bass();
    a.open_inst_editor(track);
    a.inst.view = View::Play;
    let ctx = ctx();
    let size = Vec2::new(1200.0, 820.0);
    let shapes = window_frames(&ctx, &mut a, size, vec![vec![]; 3]);
    let all = words(&shapes);
    // The 808's knock, in its Pitch card.
    let label = all
        .iter()
        .find(|w| w.text == "KNOCK")
        .unwrap_or_else(|| panic!("no knock cell in {all:#?}"));
    let at = label.rect.center();
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let up = |dy: f32| at - Vec2::new(0.0, dy);
    let steps = vec![
        vec![egui::Event::PointerMoved(at)],
        vec![press(at, true)],
        vec![egui::Event::PointerMoved(up(20.0))],
        vec![egui::Event::PointerMoved(up(40.0))],
        vec![egui::Event::PointerMoved(up(60.0))],
        vec![press(up(60.0), false)],
        vec![],
    ];
    let before = spec(&a, track).get("808.knock");
    window_frames(&ctx, &mut a, size, steps);
    let after = spec(&a, track).get("808.knock");
    assert!(
        after > before + 0.1,
        "the drag raised the knock: {before} to {after}"
    );
    a.undo();
    assert!(
        !spec(&a, track).is_stored("808.knock"),
        "one undo takes the whole drag back"
    );
}

#[test]
fn dragging_the_pluck_marker_sets_millimetres_in_one_undo_step() {
    let _turn = turn();
    let (mut a, track) = with_bass();
    a.set_inst_value(track, "engine", 2.0, false);
    let s = spec(&a, track);
    let ctx = ctx();
    let size = egui::vec2(1200.0, 360.0);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    let [pluck, _, _] = bass::string_handles(rect, &s);
    let to = pluck + egui::vec2(-120.0, 0.0);
    let want = bass::string_mm_at(rect, &s, to.x).round();
    let steps: Vec<Vec<egui::Event>> = vec![
        vec![egui::Event::PointerMoved(pluck)],
        vec![egui::Event::PointerButton {
            pos: pluck,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
        vec![egui::Event::PointerMoved(pluck + egui::vec2(-40.0, 0.0))],
        vec![egui::Event::PointerMoved(pluck + egui::vec2(-80.0, 0.0))],
        vec![egui::Event::PointerMoved(to)],
        vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
        vec![],
    ];
    for (i, events) in steps.into_iter().enumerate() {
        let current = spec(&a, track);
        frame(&ctx, size, i as f64 * 0.05, events, |ui| {
            // No margins: the display at the origin.
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    bass::picture(&mut a, ui, track, &current, size);
                });
        });
    }
    let got = spec(&a, track).get("str.pluck");
    assert!(
        (got - want).abs() <= 2.0,
        "the pluck marker landed at {got} mm, the pointer at {want} mm"
    );
    assert!(got < 180.0, "dragged towards the bridge: {got}");
    a.undo();
    assert_eq!(
        spec(&a, track).get("str.pluck"),
        180.0,
        "the whole drag is one undo step"
    );
}

#[test]
fn switching_engines_is_one_step_and_keeps_every_engines_settings() {
    let _turn = turn();
    let (mut a, track) = with_bass();
    a.set_inst_value(track, "str.pluck", 300.0, false);
    a.set_inst_value(track, "syn.cutoff", 1234.0, false);
    a.set_inst_value(track, "engine", 2.0, false);
    a.set_inst_value(track, "engine", 0.0, false);
    let s = spec(&a, track);
    assert_eq!(s.get("str.pluck"), 300.0);
    assert_eq!(s.get("syn.cutoff"), 1234.0);
    a.undo();
    assert_eq!(spec(&a, track).get("engine"), 2.0);
}

#[test]
fn no_word_is_cut_elided_or_overlapped_at_75_and_200_percent_in_both_languages() {
    let _turn = turn();
    let mut bad = Vec::new();
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for engine in [0.0f32, 1.0, 2.0] {
            for k in [0.75f32, 2.0] {
                let (mut a, track) = with_bass();
                cantus_ui::i18n::set_thread_locale(locale);
                a.set_inst_value(track, "engine", engine, false);
                a.open_inst_editor(track);
                bass::demo_feed(&mut a);
                a.inst.view = View::Play;
                let ctx = ctx();
                shell::Shell::new(("inst", track.0)).set_scale(&ctx, k);
                let size = shell::INSTRUMENT * k + Vec2::splat(40.0);
                let shapes = window_frames(&ctx, &mut a, size, vec![vec![]; 3]);
                for f in faults(&words(&shapes), view_area(k, View::Play)) {
                    bad.push(format!("{locale:?} engine {engine} {k} Play: {f}"));
                }
                // Edit, whole: the window would scroll it.
                let s = spec(&a, track);
                let st = Style::current().scaled(k).function(Function::Source);
                let w = view_area(k, View::Edit).width();
                let size = Vec2::new(w + 40.0, 1800.0 * k);
                let ctx = support::ctx();
                let mut shapes = Vec::new();
                for i in 0..3 {
                    shapes = frame(&ctx, size, i as f64 * 0.25, vec![], |root| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(root, |ui| {
                                ui.allocate_ui(Vec2::new(w, size.y), |ui| {
                                    bass::body(&mut a, ui, track, &s, &st)
                                });
                            });
                    });
                }
                let area = Rect::from_min_size(Pos2::ZERO, size);
                for f in faults(&words(&shapes), area) {
                    bad.push(format!("{locale:?} engine {engine} {k} Edit: {f}"));
                }
                cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
            }
        }
    }
    assert!(bad.is_empty(), "{} faults: {bad:#?}", bad.len());
}

#[test]
fn presets_set_what_they_name_and_name_their_macros() {
    let _turn = turn();
    assert!((8..=16).contains(&bass::FACTORY_PRESETS.len()));
    for preset in bass::FACTORY_PRESETS {
        let (mut a, track) = with_bass();
        a.set_inst_value(track, "gain", -20.0, false);
        let target = preset_spec(InstrumentKind::Bass, preset);
        a.apply_inst_spec(track, &target, "cmd.fx_preset");
        let s = spec(&a, track);
        for (k, v) in preset.values {
            let info = InstrumentKind::Bass
                .param(k)
                .unwrap_or_else(|| panic!("{k}"));
            let want = if info.stepped { v.round() } else { *v };
            assert_eq!(
                s.get(k),
                want.clamp(info.min, info.max),
                "{} {k}",
                preset.label_key
            );
        }
        for (k, v) in preset.text {
            assert_eq!(s.text(k), Some(*v), "{} {k}", preset.label_key);
        }
        assert!(!preset.tags.is_empty(), "{}", preset.label_key);
        assert!(
            s.text("macro1.name").is_some(),
            "{} names what its first macro does",
            preset.label_key
        );
        // Every connection a preset makes reaches a real target from a real source.
        for c in cantus_core::inst::modulation::connections(&s) {
            assert!(InstrumentKind::Bass.param(c.dst).is_some(), "{}", c.dst);
            assert!(InstrumentKind::Bass.sources().contains(&c.src), "{}", c.src);
        }
        a.undo();
        assert_eq!(
            spec(&a, track).get("gain"),
            -20.0,
            "{}: one undo step",
            preset.label_key
        );
    }
}

#[test]
fn every_label_is_translated_in_both_languages() {
    let _turn = turn();
    use cantus_ui::i18n::{set_thread_locale, t, Locale};
    let mut keys: Vec<&str> = PARAMS.iter().map(|p| p.label_key).collect();
    keys.extend(bass::FACTORY_PRESETS.iter().map(|p| p.label_key));
    keys.extend(
        bass::FACTORY_PRESETS
            .iter()
            .flat_map(|p| p.tags.iter().copied()),
    );
    keys.extend([
        "bass.engine.808",
        "bass.engine.synth",
        "bass.engine.string",
        "bass.host_note",
        "bass.host_short",
        "bass.title.808",
        "bass.title.synth",
        "bass.title.string",
    ]);
    for locale in [Locale::En, Locale::Es] {
        set_thread_locale(locale);
        for k in &keys {
            assert_ne!(t(k), *k, "{k} has no words in {locale:?}");
        }
    }
    set_thread_locale(Locale::En);
    // And nothing painted is a key.
    for locale in [Locale::En, Locale::Es] {
        for engine in [0.0, 1.0, 2.0] {
            let (mut a, track) = with_bass();
            set_thread_locale(locale);
            a.set_inst_value(track, "engine", engine, false);
            a.open_inst_editor(track);
            bass::demo_feed(&mut a);
            for view in [View::Play, View::Edit] {
                a.inst.view = view;
                let text = paint_inst(&mut a);
                assert!(
                    !text.contains("bass."),
                    "{locale:?} engine {engine} {view:?} painted a key: {text}"
                );
            }
            set_thread_locale(Locale::En);
        }
    }
}

/// Every engine's Play and Edit views in the window, Slate and Paper: into
/// `$GRAVIS_SHOTS` when it is set, else only rendered.
#[test]
fn every_engine_renders_in_both_themes() {
    let _turn = turn();
    let dir = std::env::var_os("GRAVIS_SHOTS").map(std::path::PathBuf::from);
    for (tn, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        for (engine, en) in [(0.0, "808"), (1.0, "synth"), (2.0, "string")] {
            for (view, vn) in [(View::Play, "play"), (View::Edit, "edit")] {
                let (mut a, track) = with_bass();
                a.set_inst_value(track, "engine", engine, false);
                a.open_inst_editor(track);
                bass::demo_feed(&mut a);
                a.inst.view = view;
                let ctx = ctx();
                cantus_ui::theme::apply(&theme, &ctx);
                let path = dir
                    .as_ref()
                    .map(|d| d.join(format!("gravis-{en}-{vn}-{tn}.png")));
                cantus_ui::raster::shoot(
                    &ctx,
                    egui::vec2(1100.0, 720.0),
                    1.0,
                    5,
                    path.as_deref(),
                    |ui| {
                        cantus_ui::views::inst::window_in(&mut a, ui);
                    },
                );
            }
        }
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx());
}

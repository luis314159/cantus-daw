//! Calor's editor (ADR 0023, on the kit's shell of ADR 0024): it opens in Play, Edit and
//! Matrix and paints what it is; a gesture on the filter display and a drag on a cell are
//! one undo step each; presets set what they name and name their macros; re-rolling a card
//! is one undo step and moves that card alone; every string is in both locales; and no
//! word is cut, elided or painted over another at 75 % and 200 %, in English and Spanish.

mod support;
use cantus_core::inst::modulation::connections;
use cantus_core::*;
use cantus_ui::kit::shell::{self, Bands};
use cantus_ui::kit::{Function, Style};
use cantus_ui::views::inst::{analog, preset_spec, View};
use egui::{Pos2, Rect, Vec2};
use support::*;

const KIND: InstrumentKind = InstrumentKind::Analog;

/// The locale is global: these tests take turns, so the one that paints in Spanish never
/// paints under another's feet.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn with_calor() -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(KIND)));
    (a, track)
}

fn spec(a: &cantus_ui::CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("Calor")
}

fn paint_editor(a: &mut cantus_ui::CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(1500.0, 1100.0), 3, |ui| {
        cantus_ui::views::inst::window_in(a, ui);
    })
}

// ---- what a frame painted ------------------------------------------------------------------

/// A text a frame painted: its words, where its letters are, the clip it was painted in.
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

/// One frame of `draw` at `size` with `events`, returning its shapes.
fn frame(
    ctx: &egui::Context,
    size: Vec2,
    time: f64,
    events: Vec<egui::Event>,
    draw: impl FnMut(&mut egui::Ui),
) -> Vec<egui::epaint::ClippedShape> {
    let mut draw = draw;
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

/// The area the instrument fills in its window at the origin, at scale `k`, in `view`.
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

/// What is wrong with the words painted inside `area`: elided, cut by their clip, or over
/// another word.
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

fn locales() -> [cantus_ui::i18n::Locale; 2] {
    [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es]
}

// ---- the tests -----------------------------------------------------------------------------

#[test]
fn it_opens_in_every_view_and_paints_what_it_is() {
    let _turn = turn();
    let (mut a, track) = with_calor();
    a.open_inst_editor(track);
    analog::demo_feed(&mut a);
    for (view, expect) in [
        (
            View::Play,
            vec![
                "Calor",
                "Ladder",
                "Filter",
                "Voice cards",
                "Settled",
                "Amp envelope",
            ],
        ),
        (
            View::Edit,
            vec!["Calor", "Osc 1", "Osc 2", "Osc 3", "Mixer", "Amplifier"],
        ),
        (View::Matrix, vec!["Calor"]),
    ] {
        a.inst.view = view;
        let text = paint_editor(&mut a);
        for e in expect {
            assert!(
                text.contains(e) || text.contains(&e.to_uppercase()),
                "{view:?} never painted {e:?}: {text}"
            );
        }
    }
}

#[test]
fn every_string_it_paints_is_translated() {
    let _turn = turn();
    for locale in locales() {
        let (mut a, track) = with_calor();
        cantus_ui::i18n::set_locale(locale);
        a.open_inst_editor(track);
        analog::demo_feed(&mut a);
        // A self-oscillating state paints the extra labels too.
        a.set_inst_value(track, "vcf.reso", 1.1, false);
        a.set_inst_value(track, "vcf.model", 3.0, false);
        for view in [View::Play, View::Edit] {
            a.inst.view = view;
            let text = paint_editor(&mut a);
            assert!(
                !text.contains("calor.") && !text.contains("preset.tag"),
                "{locale:?} {view:?} painted an untranslated key: {text}"
            );
        }
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    }
    // Every parameter and preset has a label in both locales.
    for key in KIND
        .params()
        .iter()
        .map(|i| i.label_key)
        .chain(analog::FACTORY_PRESETS.iter().map(|p| p.label_key))
        .chain(
            analog::FACTORY_PRESETS
                .iter()
                .flat_map(|p| p.tags.iter().copied()),
        )
    {
        for locale in locales() {
            cantus_ui::i18n::set_locale(locale);
            assert_ne!(cantus_ui::i18n::t(key), key, "{locale:?} has no {key}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn a_drag_on_the_filter_is_one_undo_step() {
    let _turn = turn();
    let (mut a, track) = with_calor();
    let before = spec(&a, track);
    for (cut, reso) in [(1500.0, 0.3), (1200.0, 0.5), (900.0, 0.8)] {
        analog::set_filter(&mut a, track, cut, reso);
    }
    a.end_gesture();
    let s = spec(&a, track);
    assert_eq!((s.get("vcf.cutoff"), s.get("vcf.reso")), (900.0, 0.8));
    a.undo();
    assert_eq!(
        spec(&a, track),
        before,
        "one undo takes the whole drag back"
    );
}

#[test]
fn a_knob_gesture_is_one_undo_step() {
    let _turn = turn();
    let (mut a, track) = with_calor();
    for v in [0.3, 0.5, 0.9] {
        a.set_inst_value(track, "analog.condition", v, true);
    }
    a.end_gesture();
    assert_eq!(spec(&a, track).get("analog.condition"), 0.9);
    a.undo();
    assert!(!spec(&a, track).is_stored("analog.condition"));
}

/// The window at the origin: one frame per step, each with its events.
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

#[test]
fn dragging_a_cell_is_one_undo_step() {
    let _turn = turn();
    let (mut a, track) = with_calor();
    a.open_inst_editor(track);
    a.inst.view = View::Play;
    analog::demo_feed(&mut a);
    let ctx = ctx();
    let size = Vec2::new(1200.0, 820.0);
    let shapes = window_frames(&ctx, &mut a, size, vec![vec![], vec![], vec![]]);
    let all = words(&shapes);
    let label = all
        .iter()
        .find(|w| w.text == "RESONANCE" || w.text == "RES")
        .unwrap_or_else(|| panic!("no resonance cell in {all:#?}"));
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
    let before = spec(&a, track).get("vcf.reso");
    window_frames(&ctx, &mut a, size, steps);
    let after = spec(&a, track).get("vcf.reso");
    assert!(
        after > before + 0.1,
        "the drag raised the resonance: {before} to {after}"
    );
    a.undo();
    assert!(
        !spec(&a, track).is_stored("vcf.reso"),
        "one undo takes the whole drag back"
    );
}

#[test]
fn no_word_is_cut_elided_or_overlapped_at_75_and_200_percent_in_both_languages() {
    let _turn = turn();
    let mut bad = Vec::new();
    for locale in locales() {
        for (k, model) in [(0.75f32, 0.0), (2.0, 0.0), (1.0, 3.0), (1.0, 4.0)] {
            let (mut a, track) = with_calor();
            cantus_ui::i18n::set_locale(locale);
            a.open_inst_editor(track);
            analog::demo_setup(&mut a, track);
            a.set_inst_value(track, "vcf.model", model, false);
            a.inst.view = View::Play;
            // Play, in the window.
            let ctx = ctx();
            shell::Shell::new(("inst", track.0)).set_scale(&ctx, k);
            let size = shell::INSTRUMENT * k + Vec2::splat(40.0);
            let shapes = window_frames(&ctx, &mut a, size, vec![vec![]; 3]);
            for f in faults(&words(&shapes), view_area(k, View::Play)) {
                bad.push(format!("{locale:?} {k} model {model} Play: {f}"));
            }
            // Edit, whole: the window would scroll it.
            let s = spec(&a, track);
            let st = Style::current().scaled(k).function(Function::Tone);
            let w = view_area(k, View::Edit).width();
            let size = Vec2::new(w + 40.0, 2400.0 * k);
            let ctx = support::ctx();
            let mut shapes = Vec::new();
            for i in 0..3 {
                shapes = frame(&ctx, size, i as f64 * 0.25, vec![], |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| {
                            ui.allocate_ui(Vec2::new(w, size.y), |ui| {
                                analog::body(&mut a, ui, track, &s, &st)
                            });
                        });
                });
            }
            let area = Rect::from_min_size(Pos2::ZERO, size);
            for f in faults(&words(&shapes), area) {
                bad.push(format!("{locale:?} {k} model {model} Edit: {f}"));
            }
            cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        }
    }
    assert!(bad.is_empty(), "{} faults: {bad:#?}", bad.len());
}

#[test]
fn presets_set_what_they_name_and_name_their_macros() {
    let _turn = turn();
    assert!(
        (8..=16).contains(&analog::FACTORY_PRESETS.len()),
        "8 to 16 factory presets"
    );
    for preset in analog::FACTORY_PRESETS {
        let target = preset_spec(KIND, preset);
        for (k, v) in preset.values {
            assert_eq!(
                target.get(k),
                *v,
                "{}: {k} is not a Calor key or is out of range",
                preset.label_key
            );
        }
        assert!(
            target.text("macro1.name").is_some(),
            "{} names no macro",
            preset.label_key
        );
        assert!(
            connections(&target).any(|c| c.src == "macro1"),
            "{}: macro 1 drives nothing",
            preset.label_key
        );
        assert!(!preset.tags.is_empty(), "{} has no tags", preset.label_key);
        let (mut a, track) = with_calor();
        a.set_inst_value(track, "gain", -20.0, false);
        a.apply_inst_spec(track, &target, "cmd.fx_preset");
        assert_eq!(spec(&a, track), target, "{}", preset.label_key);
        a.undo();
        assert_eq!(spec(&a, track).get("gain"), -20.0, "one undo step back");
    }
}

#[test]
fn rerolling_a_card_is_one_undo_step_and_moves_that_card_alone() {
    let _turn = turn();
    let (mut a, track) = with_calor();
    let before = analog_cards(&spec(&a, track));
    let s = spec(&a, track);
    a.set_inst_value(track, "card5.roll", 1.0, false);
    let after = analog_cards(&spec(&a, track));
    for c in 0..16 {
        if c == 4 {
            assert_ne!(before[c], after[c], "card 5 was drawn again");
        } else {
            assert_eq!(before[c], after[c], "card {} moved too", c + 1);
        }
    }
    a.undo();
    assert_eq!(spec(&a, track), s);
}

fn analog_cards(s: &NativeSpec) -> Vec<[f32; 3]> {
    let set = cantus_engine::inst::analog::settings(s);
    let v = set.card_variance();
    (0..16)
        .map(|c| cantus_dsp::va::card_trims(set.seed, c, set.rolls[c], &v).osc_cents)
        .collect()
}

/// The whole Edit view through the software renderer, Slate and Paper: into
/// `$CALOR_SHOTS` when it is set, else only rendered.
#[test]
fn the_edit_view_renders() {
    let _turn = turn();
    let dir = std::env::var_os("CALOR_SHOTS").map(std::path::PathBuf::from);
    for (name, theme) in [
        ("edit-slate", cantus_ui::theme::slate()),
        ("edit-paper", cantus_ui::theme::paper()),
    ] {
        let (mut a, track) = with_calor();
        analog::demo_setup(&mut a, track);
        let ctx = ctx();
        cantus_ui::theme::apply(&theme, &ctx);
        let path = dir.as_ref().map(|d| d.join(format!("calor-{name}.png")));
        // The whole view, unscrolled (the window would show its top).
        cantus_ui::raster::shoot(
            &ctx,
            egui::vec2(1040.0, 1900.0),
            1.0,
            4,
            path.as_deref(),
            |root| {
                let ground = Style::current().t.device_ground;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(ground).inner_margin(8.0))
                    .show(root, |ui| {
                        let s = spec(&a, track);
                        let st = Style::current().function(Function::Tone);
                        analog::body(&mut a, ui, track, &s, &st);
                    });
            },
        );
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx());
}

//! Pulsus II's editor, in the kit's device shell (ADR 0024 §8): it opens in both views and
//! paints its name, its pads and the kick's note; a knob gesture, a cell drag, a layer
//! edit, an upgrade and a kit load are one undo step each; the selection follows a pad by
//! identity; every word fits its box at 75 % and 200 % and in Spanish; the grid's words
//! and edges pass contrast in every shipped theme. With `CANTUS_INST_SHOTS=<dir>` it
//! writes a PNG per view and theme — the design review.

mod support;
use cantus_core::*;
use cantus_ui::views::pulsus_editor::{self, View};
use support::*;

fn paint_editor(app: &mut cantus_ui::CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(1400.0, 900.0), 3, |ui| {
        pulsus_editor::window_in(app, ui);
    })
}

fn kit_app(kit: PulsusParams) -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(track, InstrumentSpec::Pulsus(kit));
    (a, track)
}

fn kit(a: &cantus_ui::CantusApp, track: TrackId) -> PulsusParams {
    a.project()
        .track(track)
        .unwrap()
        .instrument
        .pulsus()
        .unwrap()
        .clone()
}

#[test]
fn the_editor_opens_in_both_views_and_paints_its_pads() {
    let (mut a, track) = kit_app(PulsusParams::studio());
    pulsus_editor::open(&mut a, track);
    for view in [View::Play, View::Edit] {
        a.pulsus.view = view;
        let text = paint_editor(&mut a);
        assert!(
            text.contains("PULSUS II"),
            "{view:?}: no name painted: {text}"
        );
        for name in ["Kick", "Crash", "Ride", "Cowbell"] {
            assert!(
                text.contains(name),
                "{view:?}: the {name} pad is not on the grid"
            );
        }
        // The selected kick's tail, as a note: Tune 0 on a model 2 kick is F1.
        assert!(
            text.contains("F1"),
            "{view:?}: the kick's note is not shown: {text}"
        );
    }
    pulsus_editor::close(&mut a);
    assert!(a.pulsus.open.is_none());
}

#[test]
fn a_model_one_kit_offers_the_upgrade_and_it_is_one_undo_step() {
    let mut old = PulsusParams {
        version: 1,
        ..PulsusParams::default()
    };
    old.pads[2].drive = 0.2; // Pulsus I stored 0.2 on every pad
    let (mut a, track) = kit_app(old.clone());
    let before = kit(&a, track);
    pulsus_editor::open(&mut a, track);
    let text = paint_editor(&mut a);
    let upgrade = cantus_ui::i18n::t("pulsus.upgrade").to_uppercase();
    assert!(text.to_uppercase().contains(&upgrade), "{text}");
    pulsus_editor::upgrade(&mut a, track);
    let after = kit(&a, track);
    assert_eq!(after.version, PULSUS_MODEL);
    assert_eq!(
        after.pads[2].drive, 0.0,
        "the drive model 1 never heard is taken back"
    );
    assert_eq!(
        after.pads[0].drive, before.pads[0].drive,
        "the kick keeps its drive"
    );
    a.undo();
    assert_eq!(
        kit(&a, track),
        before,
        "one undo step brings model 1 back exactly"
    );
}

#[test]
fn a_knob_gesture_and_a_layer_edit_are_one_undo_step_each() {
    let (mut a, track) = kit_app(PulsusParams::studio());
    let before = kit(&a, track);
    let snare = before.pads[2].id;
    for v in [0.3f32, 0.35, 0.4, 0.45] {
        pulsus_editor::set_field(&mut a, track, snare, PadField::Decay, v, true);
    }
    a.end_gesture();
    assert_eq!(kit(&a, track).by_id(snare).unwrap().decay, 0.45);
    a.undo();
    assert_eq!(kit(&a, track), before, "a drag is one step");

    let layers = vec![PadLayer::new("a.wav"), PadLayer::new("b.wav")];
    assert!(a.exec(Command::SetPadLayers {
        track,
        pad: snare,
        layers: layers.clone()
    }));
    assert_eq!(kit(&a, track).by_id(snare).unwrap().layers, layers);
    a.undo();
    assert_eq!(kit(&a, track), before);
}

#[test]
fn loading_a_factory_kit_sets_what_it_names_in_one_step() {
    let (mut a, track) = kit_app(PulsusParams::default());
    let before = kit(&a, track);
    let factory = PulsusParams::factory();
    assert!(factory.len() >= 8);
    let eight = factory
        .iter()
        .find(|f| f.label_key == "pulsus.kit.808")
        .unwrap();
    pulsus_editor::load_kit(&mut a, track, eight.kit.clone());
    let now = kit(&a, track);
    assert_eq!(now.pads.len(), eight.kit.pads.len());
    assert_eq!(now.pads[0].model, 1, "the 808 kit's kick is the 808 model");
    a.undo();
    assert_eq!(kit(&a, track), before);
}

/// The selection is a pad, not a row: adding a pad below it keeps it on the same drum.
#[test]
fn the_selection_follows_the_pad_when_a_lower_one_arrives() {
    let (mut a, track) = kit_app(PulsusParams::default());
    let k = kit(&a, track);
    let snare = k.pads[2].id;
    pulsus_editor::open(&mut a, track);
    a.pulsus.selected = Some(snare);
    let mut low = DrumPad::kick_909();
    low.pitch = 35;
    low.id = k.next_id();
    assert!(a.exec(Command::InsertPad {
        track,
        pad: Box::new(low)
    }));
    let k = kit(&a, track);
    assert_eq!(k.index_of(snare), Some(3), "the snare moved down a row");
    assert_eq!(a.pulsus.selected, Some(snare));
    // And an edit addressed to it still lands on the snare.
    pulsus_editor::set_field(&mut a, track, snare, PadField::Tone, 2_222.0, false);
    assert_eq!(kit(&a, track).by_id(snare).unwrap().tone, 2_222.0);
    assert_eq!(kit(&a, track).pads[3].pitch, 38);
}

// ---- the look: words in their boxes, a drag on a cell, contrast, screenshots ----------------

/// Every straight text a frame painted: its string, where its letters are, and the clip it
/// was painted under.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect, egui::Rect)> {
    fn walk(
        shape: &egui::Shape,
        clip: egui::Rect,
        out: &mut Vec<(String, egui::Rect, egui::Rect)>,
    ) {
        match shape {
            egui::Shape::Text(t) if t.angle == 0.0 => {
                let r = egui::Rect::from_min_size(t.pos, t.galley.size());
                out.push((t.galley.text().to_string(), r, clip));
            }
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

/// Run frames of the editor with `events` in the first ones, and return the last frame's shapes.
fn run(
    ctx: &egui::Context,
    app: &mut cantus_ui::CantusApp,
    size: egui::Vec2,
    frames: Vec<Vec<egui::Event>>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut shapes = Vec::new();
    for (i, events) in frames.into_iter().enumerate() {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 * 0.05),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            pulsus_editor::window_in(app, ui);
        });
        out.textures_delta.clear();
        shapes = out.shapes;
    }
    shapes
}

/// Nothing elided, nothing cut by its clip, no two words on top of each other: the
/// texts whose centre lies in `area`.
fn assert_fits(texts: &[(String, egui::Rect, egui::Rect)], area: egui::Rect, what: &str) {
    let inside: Vec<&(String, egui::Rect, egui::Rect)> = texts
        .iter()
        .filter(|(s, r, clip)| {
            !s.trim().is_empty() && area.contains(r.center()) && clip.contains(r.center())
        })
        .collect();
    assert!(
        inside.len() > 20,
        "{what}: only {} texts painted",
        inside.len()
    );
    for (s, r, clip) in &inside {
        // A label, a title or a value that did not fit loses its end; a name is elided in
        // the middle by design (ADR 0024 §3), and may be.
        assert!(
            !s.ends_with('\u{2026}') || opens_a_dialog(s),
            "{what}: \"{s}\" was cut short"
        );
        assert!(
            r.left() >= clip.left() - 0.5 && r.right() <= clip.right() + 0.5,
            "{what}: \"{s}\" at {r:?} is cut by its clip {clip:?}"
        );
    }
    for (i, (a, ra, _)) in inside.iter().enumerate() {
        for (b, rb, _) in &inside[i + 1..] {
            let o = ra.intersect(*rb);
            assert!(
                !(o.width() > 0.5 && o.height() > 0.5),
                "{what}: \"{a}\" {ra:?} runs into \"{b}\" {rb:?}"
            );
        }
    }
}

fn open_at_origin(view: View) -> (cantus_ui::CantusApp, TrackId) {
    let (mut a, track) = kit_app(PulsusParams::studio());
    pulsus_editor::open(&mut a, track);
    a.pulsus.kit_name.insert(track, "pulsus.kit.studio");
    a.pulsus.selected = kit(&a, track)
        .pads
        .iter()
        .find(|p| p.pitch == 36)
        .map(|p| p.id);
    a.pulsus.view = view;
    (a, track)
}

#[test]
fn every_word_fits_its_box_at_three_quarters_double_size_and_in_spanish() {
    use cantus_ui::i18n::{set_locale, Locale};
    for (locale, k) in [
        (Locale::En, 1.0),
        (Locale::En, 0.75),
        (Locale::En, 2.0),
        (Locale::Es, 1.0),
        (Locale::Es, 0.75),
    ] {
        for view in [View::Play, View::Edit] {
            let (mut a, track) = open_at_origin(view);
            let c = ctx();
            set_locale(locale);
            cantus_ui::theme::apply(&cantus_ui::theme::slate(), &c);
            cantus_ui::kit::shell::Shell::new(("pulsus", track.0)).set_scale(&c, k);
            let size = cantus_ui::kit::shell::INSTRUMENT * k + egui::vec2(40.0, 40.0);
            let shapes = run(&c, &mut a, size, vec![Vec::new(); 3]);
            let area = egui::Rect::from_min_max(
                egui::pos2(0.0, 36.0 * k),
                egui::pos2(1040.0 * k, (680.0 - 32.0) * k),
            );
            assert_fits(
                &texts(&shapes),
                area,
                &format!("{locale:?} {view:?} at {k}"),
            );
            set_locale(Locale::En);
        }
    }
}

#[test]
fn a_drag_on_a_cell_is_one_undo_step() {
    let (mut a, track) = open_at_origin(View::Play);
    let before = kit(&a, track);
    let c = ctx();
    let size = egui::vec2(1100.0, 720.0);
    let shapes = run(&c, &mut a, size, vec![Vec::new(); 3]);
    let decay = cantus_ui::i18n::t("pad.decay").to_uppercase();
    let (_, r, _) = texts(&shapes)
        .into_iter()
        .find(|(s, _, _)| *s == decay)
        .expect("the voice card's decay cell is painted");
    let p = r.center();
    let mut frames = vec![
        vec![egui::Event::PointerMoved(p)],
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    ];
    for i in 1..=8 {
        frames.push(vec![egui::Event::PointerMoved(
            p - egui::vec2(0.0, 6.0 * i as f32),
        )]);
    }
    frames.push(vec![egui::Event::PointerButton {
        pos: p - egui::vec2(0.0, 48.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    frames.push(Vec::new());
    run(&c, &mut a, size, frames);
    let kick = before.pads.iter().find(|p| p.pitch == 36).unwrap().id;
    let now = kit(&a, track);
    assert!(
        now.by_id(kick).unwrap().decay > before.by_id(kick).unwrap().decay,
        "dragging the cell up lengthens the decay"
    );
    a.undo();
    assert_eq!(kit(&a, track), before, "the whole drag undoes at once");
}

/// The grid's words at 4.5:1 and its marks at 3:1 on the cells, in every shipped theme.
#[test]
fn the_pad_grid_passes_contrast_in_every_theme() {
    use cantus_ui::kit::color::{contrast, mix};
    use cantus_ui::theme;
    for (name, th) in [
        ("slate", theme::slate()),
        ("paper", theme::paper()),
        ("parchment", theme::parchment()),
        ("ink & gold", theme::ink_and_gold()),
    ] {
        let t = th.kit_tokens();
        let hue = t.hue_source;
        let selected = mix(t.deck, hue, if t.dark { 0.22 } else { 0.14 });
        let checks = [
            ("pad name", t.text, t.deck, 4.5),
            ("note and model", t.text_dim, t.deck, 4.5),
            ("name on the selected pad", t.text, selected, 4.5),
            ("choke chip", t.highlight, t.deck, 3.0),
            ("the hit", hue, t.deck, 3.0),
        ];
        for (what, fg, bg, min) in checks {
            let c = contrast(fg, bg);
            assert!(c >= min, "{name}: {what} is {c:.2}:1, under {min}:1");
        }
    }
}

#[test]
fn screenshots_in_slate_and_paper() {
    let Some(dir) = std::env::var_os("CANTUS_INST_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    for (theme_name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        for (view, tag) in [(View::Play, "play"), (View::Edit, "edit")] {
            let (mut a, _) = open_at_origin(view);
            let c = ctx();
            cantus_ui::theme::apply(&theme, &c);
            let path = dir.join(format!("pulsus-{tag}-{theme_name}.png"));
            cantus_ui::raster::shoot(&c, egui::vec2(1080.0, 720.0), 1.0, 6, Some(&path), |ui| {
                pulsus_editor::window_in(&mut a, ui);
            });
        }
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx());
}

/// A label that ends in "…" by the convention for an action that opens a dialog
/// ("Export SFZ…", "+ Layer…"), as the locale spells it — not a cut.
fn opens_a_dialog(s: &str) -> bool {
    [
        "smp.export_sfz",
        "smp.import_samples",
        "smp.import_folder",
        "smp.import_instrument",
        "pulsus.layer.add",
        "pad.sample_load",
    ]
    .iter()
    .map(|k| cantus_ui::i18n::t(k))
    .any(|l| l == s || l.to_uppercase() == s)
}

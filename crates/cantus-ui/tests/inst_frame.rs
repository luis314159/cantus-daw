//! The instrument frame (ADR 0023), for every native kind: it opens in each of its three
//! views and paints the instrument's name; A/B, presets and modulation connections are one
//! undo step each and come back exactly.

mod support;
use cantus_core::*;
use cantus_ui::views::inst::View;
use support::*;

fn paint_inst(app: &mut cantus_ui::CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(1400.0, 900.0), 3, |ui| {
        cantus_ui::views::inst::window_in(app, ui);
    })
}

fn with(kind: InstrumentKind) -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
    (a, track)
}

fn spec(a: &cantus_ui::CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("a native instrument")
}

#[test]
fn every_instrument_opens_in_the_shared_frame_in_every_view() {
    for kind in InstrumentKind::ALL {
        let (mut a, track) = with(kind);
        a.open_inst_editor(track);
        let name = cantus_ui::i18n::t(kind.name_key());
        for view in [View::Play, View::Edit, View::Matrix] {
            a.inst.view = view;
            let text = paint_inst(&mut a);
            assert!(
                text.to_uppercase().contains(&name.to_uppercase()),
                "{kind:?} in {view:?}: the editor never painted its name: {text}"
            );
        }
        a.close_inst_editor();
        assert!(a.inst.open.is_none());
    }
}

#[test]
fn a_b_is_one_undo_step_and_comes_back_exactly() {
    let (mut a, track) = with(InstrumentKind::Organ);
    let a_side = spec(&a, track);
    a.toggle_inst_ab(track);
    assert!(a.inst.on_b(track));
    a.set_inst_value(track, "gain", -12.0, false);
    let b_side = spec(&a, track);
    a.toggle_inst_ab(track);
    assert_eq!(spec(&a, track), a_side);
    a.toggle_inst_ab(track);
    assert_eq!(spec(&a, track), b_side);
    a.undo();
    assert_eq!(spec(&a, track), a_side);
}

#[test]
fn a_preset_sets_what_it_names_and_resets_the_rest() {
    let (mut a, track) = with(InstrumentKind::Organ);
    a.set_inst_value(track, "macro2", 0.7, false);
    a.set_inst_text(track, "macro2.name", Some("Tone".into()));
    let mut preset = NativeSpec::new(InstrumentKind::Organ);
    preset.set("gain", -6.0);
    a.apply_inst_spec(track, &preset, "cmd.fx_preset");
    let s = spec(&a, track);
    assert_eq!(s.get("gain"), -6.0);
    assert!(
        !s.is_stored("macro2"),
        "a value the preset does not name is reset"
    );
    assert_eq!(s.text("macro2.name"), None, "and so is a text");
    a.undo();
    assert_eq!(
        spec(&a, track).get("macro2"),
        0.7,
        "one undo step takes it all back"
    );
}

#[test]
fn connecting_and_removing_modulation_are_one_undo_step_each() {
    use cantus_core::inst::modulation::connections;
    let (mut a, track) = with(InstrumentKind::Wavetable);
    a.connect_modulation(track, "vel", "gain");
    let s = spec(&a, track);
    let c: Vec<_> = connections(&s).collect();
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].src, c[0].dst), ("vel", "gain"));
    a.undo();
    assert_eq!(connections(&spec(&a, track)).count(), 0);
    a.redo();
    assert_eq!(connections(&spec(&a, track)).count(), 1);
    a.remove_modulation(track, 1);
    let s = spec(&a, track);
    assert_eq!(connections(&s).count(), 0);
    assert!(
        s.params.is_empty() && s.text.is_empty(),
        "removing leaves nothing behind"
    );
    a.undo();
    assert_eq!(connections(&spec(&a, track)).count(), 1);
}

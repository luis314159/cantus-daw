//! Every instrument editor in the kit's device shell (ADR 0024 §8), in Slate and in Paper:
//! it opens in each of its three views and paints its name, and the bands' cards paint
//! their titles. With `CANTUS_INST_SHOTS=<dir>` it also writes a PNG per kind, theme and
//! view — the design review for the instruments' migration.

mod support;
use cantus_core::*;
use cantus_ui::views::inst::View;
use support::*;

fn open(kind: InstrumentKind) -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
    cantus_ui::views::inst::demo_setup(&mut a, track, kind);
    cantus_ui::views::inst::demo_feed(&mut a, kind);
    a.open_inst_editor(track);
    (a, track)
}

fn draw(a: &mut cantus_ui::CantusApp, ui: &mut egui::Ui) {
    cantus_ui::views::inst::window_in(a, ui);
}

#[test]
fn every_instrument_opens_in_the_shell_in_both_themes() {
    let shots = std::env::var_os("CANTUS_INST_SHOTS").map(std::path::PathBuf::from);
    if let Some(d) = &shots {
        let _ = std::fs::create_dir_all(d);
    }
    for (theme_name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        for kind in InstrumentKind::ALL {
            let (mut a, _) = open(kind);
            let name = cantus_ui::i18n::t(kind.name_key());
            for (view, tag) in [
                (View::Play, "play"),
                (View::Edit, "edit"),
                (View::Matrix, "matrix"),
            ] {
                a.inst.view = view;
                let c = ctx();
                cantus_ui::theme::apply(&theme, &c);
                let text = paint(&c, egui::vec2(1200.0, 820.0), 3, |ui| draw(&mut a, ui));
                assert!(
                    text.to_uppercase().contains(&name.to_uppercase()),
                    "{kind:?} in {view:?} ({theme_name}): the shell never painted the name"
                );
                if let Some(d) = &shots {
                    let path = d.join(format!("{}-{tag}-{theme_name}.png", kind_file(kind)));
                    let c = ctx();
                    cantus_ui::theme::apply(&theme, &c);
                    cantus_ui::raster::shoot(
                        &c,
                        egui::vec2(1200.0, 820.0),
                        1.0,
                        6,
                        Some(&path),
                        |ui| draw(&mut a, ui),
                    );
                }
            }
        }
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx());
}

fn kind_file(kind: InstrumentKind) -> &'static str {
    match kind {
        InstrumentKind::Wavetable => "unda",
        InstrumentKind::Analog => "calor",
        InstrumentKind::Fm => "modulus",
        InstrumentKind::Sampler => "exemplar",
        InstrumentKind::Bass => "gravis",
        InstrumentKind::ElectricPiano => "lamina",
        InstrumentKind::Organ => "organum",
        InstrumentKind::Chorda => "chorda",
    }
}

#[test]
fn the_keyboard_follows_the_view_until_it_is_chosen() {
    let (mut a, _) = open(InstrumentKind::Organ);
    a.inst.view = View::Play;
    assert!(cantus_ui::views::inst::keyboard_shown(&a));
    a.inst.view = View::Edit;
    assert!(!cantus_ui::views::inst::keyboard_shown(&a));
    a.inst.keyboard = Some(true);
    assert!(cantus_ui::views::inst::keyboard_shown(&a));
}

#[test]
fn alt_k_toggles_the_keyboard_card_of_the_editor_in_front() {
    let (mut a, _) = open(InstrumentKind::Analog);
    a.inst.view = View::Play;
    assert!(cantus_ui::views::inst::keyboard_shown(&a));
    let c = ctx();
    let _ = paint(&c, egui::vec2(1200.0, 820.0), 1, |ui| {
        cantus_ui::views::workspace_device::request_keyboard_toggle(ui.ctx());
        draw(&mut a, ui);
    });
    assert!(
        !cantus_ui::views::inst::keyboard_shown(&a),
        "Alt+K hid nothing"
    );
    // Once per press: the next frames leave it alone.
    let _ = paint(&c, egui::vec2(1200.0, 820.0), 3, |ui| draw(&mut a, ui));
    assert!(!cantus_ui::views::inst::keyboard_shown(&a));
}

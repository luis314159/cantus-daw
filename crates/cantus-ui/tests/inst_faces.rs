//! The instruments' faces (round 32, ADR 0074), as the review of the instruments builder's
//! delivery asked:
//!
//! - every card title of a unit is set in its sheet's lettering — all capitals or none, never
//!   "PREAMP | TREMOLO | Output" side by side — and on the plain look all capitals;
//! - four instruments in four faces of their own, drawn in one frame, Chorda in Play and
//!   Modulus in Edit among them, make no skin tokens, worn styles or display colours again
//!   once they have settled (a face's caches are not cycled by its neighbours').

mod inst_keys;
mod support;
use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::t;
use cantus_ui::kit::skin::{self, Unit};
use cantus_ui::views::inst::View;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect};
use inst_keys::frame;

fn opened(kind: InstrumentKind, view: View) -> CantusApp {
    let mut a = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let name = match kind {
        InstrumentKind::Wavetable => "unda",
        InstrumentKind::Analog => "calor",
        InstrumentKind::Fm => "modulus",
        InstrumentKind::Sampler => "exemplar",
        InstrumentKind::Bass => "gravis",
        InstrumentKind::ElectricPiano => "lamina",
        InstrumentKind::Organ => "organum",
        InstrumentKind::Chorda => "chorda",
    };
    a.demo_editor(name);
    a.inst.view = view;
    a
}

/// The card titles each sentence-case face shows in Play, by key.
fn titles(kind: InstrumentKind) -> &'static [&'static str] {
    match kind {
        InstrumentKind::Chorda => &[
            "chorda.strings",
            "chorda.pedals",
            "chorda.hammer",
            "chorda.resonance",
            "chorda.output",
        ],
        InstrumentKind::ElectricPiano => &[
            "epiano.pickup",
            "epiano.resonator_tine",
            "epiano.damper",
            "epiano.preamp",
            "epiano.tremolo",
        ],
        InstrumentKind::Sampler => &["smp.card.map", "smp.card.seam"],
        _ => &[],
    }
}

#[test]
fn every_card_title_of_a_unit_is_set_one_way() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for on in [true, false] {
        skin::set_enabled(on);
        for kind in [
            InstrumentKind::Chorda,
            InstrumentKind::ElectricPiano,
            InstrumentKind::Sampler,
        ] {
            let caps = skin::of(Unit::Instrument(kind)).lettering.title().1;
            let view = if kind == InstrumentKind::Sampler {
                View::Edit
            } else {
                View::Play
            };
            let mut a = opened(kind, view);
            let ctx = support::ctx();
            let mut painted = Vec::new();
            for _ in 0..3 {
                painted = frame(&mut a, &ctx, vec![]);
            }
            let has = |s: &str| painted.iter().any(|p| p.text == s);
            for key in titles(kind) {
                let word = t(key);
                let upper = word.to_uppercase();
                let want = if caps { upper } else { word };
                // Only titles are asked for: a control's label (a macro named like a card)
                // is in capitals by the kit's own rule, whatever the face.
                assert!(
                    has(&want),
                    "{kind:?} (faces {on}): {key} not painted as {want:?}"
                );
            }
            // The shell's own bands follow the same lettering.
            let macros = t("shell.macros");
            let band = if caps { macros.to_uppercase() } else { macros };
            assert!(
                has(&band),
                "{kind:?} (faces {on}): the macros band as {band:?}"
            );
        }
    }
    skin::set_enabled(true);
}

#[test]
fn four_instrument_faces_in_one_frame_settle_their_caches() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    skin::set_enabled(true);
    let ctx = support::ctx();
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let mut apps = [
        opened(InstrumentKind::Chorda, View::Play),
        opened(InstrumentKind::Fm, View::Edit),
        opened(InstrumentKind::Organ, View::Play),
        opened(InstrumentKind::Wavetable, View::Play),
    ];
    views::inst::demo_feed(&mut apps[3], InstrumentKind::Wavetable);
    let size = cantus_ui::kit::shell::INSTRUMENT;
    let mut run = |i: usize| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size * 2.0)),
            time: Some(i as f64 / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            for (n, a) in apps.iter_mut().enumerate() {
                let at = Pos2::new((n % 2) as f32 * size.x, (n / 2) as f32 * size.y);
                let mut c = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(at, size))
                        .id_salt(("face", n)),
                );
                views::inst::window_in(a, &mut c);
            }
        });
        out.textures_delta.clear();
        out.shapes.len()
    };
    for i in 0..8 {
        run(i);
    }
    let settled = skin::counts();
    let mut shapes = 0;
    for i in 8..40 {
        shapes = run(i);
    }
    assert!(shapes > 0);
    assert_eq!(
        skin::counts(),
        settled,
        "(skin tokens, worn misses, display colours) made again in steady state"
    );
}

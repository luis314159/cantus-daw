//! Nothing truncated in Fides's window (round 41, ADR 0109's rule): its Play view with each
//! of its six exciters (each draws its own knobs), its Edit view whole and its Matrix, of a
//! Fides added the way the picker adds it and on the marimba preset its `--editor` shows —
//! at 100 % and 150 % of a 1920 × 1080 screen, Slate Night and Paper, English and Spanish,
//! with the kit's text probe on: any text elided with "…" or dropped for want of room
//! fails (a designed short form, `<key>.short`, is the house's way, ADR 0024). And the
//! factory presets' macro names are said in the window's language (they are locale keys,
//! `fides.macro.*`).
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

#[path = "ux_sweep/mod.rs"]
mod sweep;

use cantus_core::InstrumentKind;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::text::probe;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::inst::View;
use sweep::{Look, Rig};

#[derive(Copy, Clone, Debug)]
enum Case {
    /// The Play view with exciter `n` (0 Pluck … 5 Input).
    Play(u8),
    Edit,
    Matrix,
}

const CASES: [Case; 8] = [
    Case::Play(0),
    Case::Play(1),
    Case::Play(2),
    Case::Play(3),
    Case::Play(4),
    Case::Play(5),
    Case::Edit,
    Case::Matrix,
];

/// A Fides added through the picker, on its demo (the marimba), in `case`.
fn show(r: &mut Rig, case: Case) {
    r.reset();
    r.app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Resonator),
    );
    let track = r.app.project().tracks.last().map(|t| t.id).unwrap();
    cantus_ui::views::inst::demo_setup(&mut r.app, track, InstrumentKind::Resonator);
    r.app.open_inst_editor(track);
    r.app.inst.view = match case {
        Case::Play(exciter) => {
            r.app
                .set_inst_value(track, "exc.type", exciter as f32, false);
            View::Play
        }
        Case::Edit => View::Edit,
        Case::Matrix => View::Matrix,
    };
    r.idle(4);
}

#[test]
fn no_word_is_cut_in_fides_s_window_in_either_language() {
    let looks = match std::env::var("CANTUS_UX_LOOKS").as_deref() {
        Ok("plain") => vec![Look::PLAIN],
        _ => Look::all(),
    };
    let mut cuts = Vec::new();
    for look in looks {
        let mut r = Rig::new(look);
        for case in CASES {
            show(&mut r, case);
            probe::start();
            let texts = r.frame(None);
            let p = r.app.project();
            let mut names: Vec<String> = p.tracks.iter().map(|t| t.name.clone()).collect();
            names.extend(p.strips.iter().map(|s| s.name.clone()));
            names.retain(|n| n.chars().count() >= 3);
            // The Matrix's slot switch (`mod.on`) is the frame's, every instrument's: not
            // Fides's words (reported to the integrator; "Activa" is cut in Spanish there).
            let shared = cantus_ui::i18n::t("mod.on").to_uppercase();
            let ours = |full: &str| {
                !names.iter().any(|n| full.contains(n.as_str())) && full.to_uppercase() != shared
            };
            for c in probe::take().into_iter().filter(|c| ours(&c.full)) {
                cuts.push(format!(
                    "{look:?} {case:?}: {:?} → {:?} ({}, {:.0} pt)",
                    c.full,
                    c.shown,
                    c.how,
                    c.rect.width()
                ));
            }
            for t in texts.into_iter().filter(|t| t.elided && ours(&t.text)) {
                cuts.push(format!("{look:?} {case:?}: {:?} (egui)", t.text));
            }
        }
    }
    for c in cuts.iter().take(60) {
        eprintln!("CUT {c}");
    }
    assert!(cuts.is_empty(), "{} cuts (listed above)", cuts.len());
}

#[test]
fn the_presets_macro_names_are_said_in_the_window_s_language() {
    for (locale, said, not) in [
        (
            Locale::En,
            ["HARDNESS", "DECAY", "BODY", "POSITION"],
            "DUREZA",
        ),
        (
            Locale::Es,
            ["DUREZA", "CAÍDA", "CUERPO", "POSICIÓN"],
            "HARDNESS",
        ),
    ] {
        let look = Look {
            locale,
            dark: true,
            scale: 1.0,
        };
        let mut r = Rig::new(look);
        show(&mut r, Case::Play(3));
        let painted: Vec<String> = r.frame(None).into_iter().map(|t| t.text).collect();
        for word in said {
            assert!(
                painted.iter().any(|t| t == word),
                "{locale:?}: the macro {word} is not painted ({painted:?})"
            );
        }
        assert!(
            !painted
                .iter()
                .any(|t| t == not || t.starts_with("FIDES.MACRO")),
            "{locale:?}: a macro named in the other language or by its key"
        );
    }
}

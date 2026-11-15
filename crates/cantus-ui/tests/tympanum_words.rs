//! Nothing truncated in Tympanum's views or the mixer's grouped strips (round 40 phase 3,
//! ADR 0109's rule): the Kit and Mixer views of a Tympanum added the way the picker adds it,
//! and Cantus's mixer with its mics on strips of their own under the family band — at 100 %
//! and 150 % of a 1920 × 1080 screen, Slate Night and Paper, English and Spanish, with the
//! kit's text probe on. `Tympanum` is not in `InstrumentKind::ALL` until it sounds, so
//! `ux_whole_words` does not reach it yet: this does.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

#[path = "ux_sweep/mod.rs"]
mod sweep;

use cantus_core::inst_out::{self, OutputId};
use cantus_core::InstrumentKind;
use cantus_ui::kit::text::probe;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::inst::View;
use sweep::{Look, Rig};

#[derive(Copy, Clone, Debug)]
enum Case {
    Kit,
    Mixer,
    Desk,
}

fn show(r: &mut Rig, case: Case) {
    r.reset();
    r.app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::AcousticDrums),
    );
    let track = r.app.project().tracks.last().map(|t| t.id).unwrap();
    match case {
        Case::Kit | Case::Mixer => {
            cantus_ui::views::inst::demo_setup(&mut r.app, track, InstrumentKind::AcousticDrums);
            r.app.open_inst_editor(track);
            r.app.inst.view = if matches!(case, Case::Kit) {
                View::Play
            } else {
                View::Mixer
            };
        }
        Case::Desk => {
            // Three mics on strips of the desk: the family band over them.
            for id in [0u16, 1, 3] {
                let p = r.app.project().clone();
                let Some(free) = p.first_free_strip() else {
                    break;
                };
                if let Ok(cmd) = inst_out::set_route(&p, track, OutputId(id), Some(free)) {
                    r.app.exec(cmd);
                }
            }
            r.app.show_central(cantus_ui::app::CentralView::Mixer);
        }
    }
    r.idle(4);
}

#[test]
fn no_word_is_cut_in_tympanums_views_or_the_grouped_strips() {
    let looks = match std::env::var("CANTUS_UX_LOOKS").as_deref() {
        Ok("plain") => vec![Look::PLAIN],
        _ => Look::all(),
    };
    let mut cuts = Vec::new();
    for look in looks {
        let mut r = Rig::new(look);
        for case in [Case::Kit, Case::Mixer, Case::Desk] {
            show(&mut r, case);
            probe::start();
            let texts = r.frame(None);
            let p = r.app.project();
            let mut names: Vec<String> = p.tracks.iter().map(|t| t.name.clone()).collect();
            names.extend(p.strips.iter().map(|s| s.name.clone()));
            names.retain(|n| n.chars().count() >= 3);
            let ours = |full: &str| !names.iter().any(|n| full.contains(n.as_str()));
            for c in probe::take().into_iter().filter(|c| ours(&c.full)) {
                cuts.push(format!(
                    "{look:?} {case:?}: {:?} → {:?} ({})",
                    c.full, c.shown, c.how
                ));
            }
            // egui's own warning: two widgets with one id (its red "First use / Second use
            // of widget ID" overlays, on in debug builds).
            for t in texts.iter().filter(|t| t.text.contains("use of widget ID")) {
                cuts.push(format!("{look:?} {case:?}: id clash {:?}", t.text));
            }
            for t in texts.into_iter().filter(|t| t.elided && ours(&t.text)) {
                cuts.push(format!("{look:?} {case:?}: {:?} (egui)", t.text));
            }
        }
    }
    for c in cuts.iter().take(40) {
        eprintln!("CUT {c}");
    }
    assert!(cuts.is_empty(), "{} cuts (listed above)", cuts.len());
}

/// Tympanum in Spanish says its words in Spanish (r40 integration, the no-leftovers rule):
/// every Spanish string of the area differs from the English one but the words a Spanish
/// drummer says as they are — names (the instrument, its kits), the pieces Spanish keeps
/// (Tom, Ride, Crash, Splash, China), the rudiments (Flam, Drag), Kit, Omni, Pedal, sizes —
/// and no two articulations share a name in either language.
#[test]
fn tympanums_spanish_is_spanish_and_its_articulations_are_told_apart() {
    fn load(lang: &str) -> Vec<(String, String)> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("locales/areas/tympanum.{lang}.txt"));
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect()
    }
    let (en, es) = (load("en"), load("es"));
    let es_of = |k: &str| es.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
    const KEPT: &[&str] = &[
        "instrument.tympanum",
        "tympanum.view.kit",
        "tympanum.kits",
        "tympanum.kit_card",
        "tympanum.strip.kit",
        "tympanum.kit.studio",
        "tympanum.kit.studio.about",
        "tympanum.kit.jazz",
        "tympanum.kit.sixties",
        "tympanum.kit.arena",
        "tympanum.kit.funk",
        "tympanum.piece.tom1",
        "tympanum.piece.tom2",
        "tympanum.piece.ride",
        "tympanum.piece.crash1",
        "tympanum.piece.crash2",
        "tympanum.piece.splash",
        "tympanum.piece.china",
        "tympanum.mic_name.tom_1",
        "tympanum.mic_name.tom_2",
        "tympanum.mic_name.ride",
        "tympanum.art.flam",
        "tympanum.art.drag",
        "tympanum.art.pedal",
        "tympanum.mic.omni",
        "tympanum.mic.kind.sub",
        "tympanum.size_in",
        "tympanum.size_cym",
    ];
    let english: Vec<String> = en
        .iter()
        .filter(|(k, v)| !KEPT.contains(&k.as_str()) && es_of(k).as_deref() == Some(v.as_str()))
        .map(|(k, v)| format!("{k} = {v}"))
        .collect();
    assert!(english.is_empty(), "English left in Spanish: {english:#?}");
    for list in [&en, &es] {
        let arts: Vec<&(String, String)> = list
            .iter()
            .filter(|(k, _)| k.starts_with("tympanum.art."))
            .collect();
        for (i, a) in arts.iter().enumerate() {
            for b in &arts[i + 1..] {
                assert_ne!(a.1, b.1, "{} and {} share a name", a.0, b.0);
            }
        }
    }
}

//! Saltator in a shipped demo (round 35, ADR 0087): the house song has a dancer — Trino on
//! the stage over the workspace, dancing its routine, hearing the kick, jumping, spinning
//! and raising its arms in the drops on notes of its move map, swaying through the break —
//! and the song sounds bit for bit as without it (the dancer is silent and was added last).

use cantus_core::inst::dancer::{self, Move};
use cantus_core::*;
use cantus_engine::{render, RenderOptions};
use std::sync::Arc;

fn house() -> Project {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    (cantus_ui::demos::find("house").unwrap().build)()
}

fn the_dancer(p: &Project) -> &Track {
    p.tracks
        .iter()
        .find(|t| dancer::is_dancer(&t.instrument))
        .expect("the house demo has a dancer")
}

#[test]
fn the_house_demo_has_a_dancer_on_the_stage() {
    let p = house();
    let t = the_dancer(&p);
    let n = t.instrument.native().unwrap();
    assert_eq!(
        n.text(dancer::CHARACTER_KEY),
        Some(dancer::DEFAULT_CHARACTER)
    );
    assert_eq!(n.get("view.mode"), 0.0, "floating over the workspace");
    assert!(n.get("react") > 0.0);
    assert_eq!(t.icon(), TrackIcon::Dance);
    // It hears the kick.
    let kick = p
        .tracks
        .iter()
        .find(|x| x.name == "Kick")
        .and_then(|x| x.strip);
    assert_eq!(
        dancer::reacting_to(&p, t.id),
        kick.map(cantus_core::modulator::Listen::Strip)
    );
    // Moves on its map in the drops; `move` automated in the break.
    let keys: Vec<u8> = t
        .clips
        .iter()
        .flat_map(|c| &c.notes)
        .map(|x| x.pitch)
        .collect();
    for m in [Move::Jump, Move::Spin, Move::ArmsUp] {
        assert!(
            keys.contains(&(dancer::BASE_KEY + m.index() as u8)),
            "{m:?}"
        );
    }
    assert!(t
        .clips
        .iter()
        .filter_map(|c| c.automation.as_ref())
        .any(|a| a.target
            == AutoTarget::Instrument {
                track: t.id,
                key: "move".into()
            }));
    // What the program opens on (the second drop) has none (the owner's decision,
    // 2026-11-08: nothing dances at start); the excerpt with it is the stage's.
    let start = cantus_ui::demos::startup_project();
    assert!(!start
        .tracks
        .iter()
        .any(|t| dancer::is_dancer(&t.instrument)));
    assert!(cantus_ui::demos::startup_project_with_dancer()
        .tracks
        .iter()
        .any(|t| dancer::is_dancer(&t.instrument)));
}

/// Bars 30 to 46 (the end of the drop, the break, the second drop's start): the same bits
/// with the dancer as with its track taken away (its strip, an empty one of the desk before
/// the dancer took it, stays).
#[test]
fn the_house_demo_sounds_bit_for_bit_as_without_its_dancer() {
    let with = house();
    let mut without = with.clone();
    let id = the_dancer(&with).id;
    without.tracks.retain(|t| t.id != id);
    let bar = with.time_signature.ticks_per_bar();
    let opts = RenderOptions {
        start: bar * 30,
        end: Some(bar * 46),
        tail_seconds: 0.5,
        ..Default::default()
    };
    let a = render(Arc::new(Snapshot::compile(&with, 0)), &opts).unwrap();
    let b = render(Arc::new(Snapshot::compile(&without, 0)), &opts).unwrap();
    assert!(a.peak > 0.01, "the song sounds");
    assert_eq!(a.samples.len(), b.samples.len());
    let first = a
        .samples
        .iter()
        .zip(&b.samples)
        .position(|(x, y)| x.to_bits() != y.to_bits());
    assert_eq!(first, None, "the dancer moved a sample");
}

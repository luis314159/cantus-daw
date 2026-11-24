//! Every other instrument reads whole at every editor scale and at the narrowest window
//! (r42 fix4-narrow): `instruments_scale_e2e` held Chorda and Tympanum to it, and nothing
//! held the rest — Exemplar's Edit view at the narrowest window at 150 %, in Spanish, cut
//! "Liberació…", "Transposic…", "Triá…", "R…" and its map card's title "Mapa tecla ×
//! veloc…". Its own test binary: the editors paint on this thread.
//!
//! The app made by `CantusApp::assemble`, a track for each native kind in
//! `InstrumentKind::ALL` but Chorda and Tympanum (`instruments_scale_e2e`'s), and Arcus
//! (`BowedString`, not in `ALL` yet), each in its demo's sound; every view its header offers
//! (Play, Edit, Matrix; the dancer's Play and Edit) at the editor's 100 %, 150 % and 200 %,
//! at the shell's size, and at the narrowest window the editor allows at each scale —
//! `inst_scale`'s rules: no word cut (elided, dropped, or painted past its card where
//! nothing scrolls), none overlapping another or running under a button, every word read at
//! 100 % read at the other scales; in English and in Spanish.

mod inst_scale;
mod paths_rig;

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project};
use cantus_ui::views::inst::{self, View};
use egui::Vec2;
use inst_scale::*;
use paths_rig::*;

/// The kinds this binary reads, and a short name for each one's pictures
/// (`CANTUS_INST_ONLY=vetus,bass` reads only those, to look at a few while fixing them).
fn kinds() -> Vec<(InstrumentKind, &'static str)> {
    let mut v: Vec<(InstrumentKind, &'static str)> = InstrumentKind::ALL
        .into_iter()
        .filter(|k| !matches!(k, InstrumentKind::Chorda | InstrumentKind::AcousticDrums))
        .map(|k| (k, short(k)))
        .collect();
    if !v.iter().any(|(k, _)| *k == InstrumentKind::BowedString) {
        v.push((
            InstrumentKind::BowedString,
            short(InstrumentKind::BowedString),
        ));
    }
    if let Ok(only) = std::env::var("CANTUS_INST_ONLY") {
        v.retain(|(_, name)| only.split(',').any(|o| o.trim() == *name));
    }
    v
}

fn short(kind: InstrumentKind) -> &'static str {
    match kind {
        InstrumentKind::Wavetable => "unda",
        InstrumentKind::Analog => "vetus",
        InstrumentKind::Fm => "fm",
        InstrumentKind::Sampler => "exemplar",
        InstrumentKind::Bass => "bass",
        InstrumentKind::ElectricPiano => "epiano",
        InstrumentKind::Organ => "organum",
        InstrumentKind::Chorda => "chorda",
        InstrumentKind::AcousticDrums => "tympanum",
        InstrumentKind::Resonator => "fides",
        InstrumentKind::BowedString => "arcus",
        InstrumentKind::Dancer => "dancer",
    }
}

/// A song with a track for each of `kinds`, each with its instrument.
fn song(kinds: &[(InstrumentKind, &'static str)]) -> (Project, Vec<cantus_core::TrackId>) {
    let mut p = cantus_ui::welcome_project();
    let mut ids = Vec::new();
    for &(kind, name) in kinds {
        let mut t = p.make_track(name);
        let strip = p.make_strip(name);
        t.strip = Some(strip.id);
        p.strips.push(strip);
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        ids.push(t.id);
        p.tracks.push(t);
    }
    (p, ids)
}

/// The editor's size at 100 % where `view` lays itself out whole: the shell's for Play; the
/// Edit and Matrix views scroll in the shell's height, so they are read in one tall enough
/// for every card (a row half out of sight in a scrolled view is no cut) — Exemplar's
/// taller, its map above its zone card and its cards over more rows at the narrowest width.
fn whole(kind: InstrumentKind, view: View) -> Vec2 {
    let least = match view {
        View::Edit if kind == InstrumentKind::Sampler => Vec2::new(0.0, 2300.0),
        View::Edit => Vec2::new(0.0, 1700.0),
        View::Matrix => Vec2::new(0.0, 1200.0),
        _ => Vec2::ZERO,
    };
    inst::size_for(least)
}

/// Read `kinds` whole, every view, every scale, the narrowest window, En and Es.
fn read(kinds: &[(InstrumentKind, &'static str)], least_words: usize) {
    let _theme_turn = theme_turn();
    let _turn = turn();
    let (p, tracks) = song(kinds);
    let mut r = Rig::new(p);
    for (&(kind, _), &track) in kinds.iter().zip(&tracks) {
        inst::demo_setup(&mut r.app, track, kind);
    }
    r.idle(4);
    let mut h = Host::new();
    let cases: Vec<Case> = kinds
        .iter()
        .zip(&tracks)
        .map(|(&(kind, name), &track)| Case {
            track,
            kind,
            name,
            views: views_of(kind),
        })
        .collect();
    let mut bad = Vec::new();
    let read = read_views(&mut r, &mut h, &cases, whole, &mut bad);
    assert!(
        read > least_words,
        "only {read} words were read:\n{}",
        bad.join("\n")
    );
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn every_other_instrument_reads_whole_at_every_scale_and_the_narrowest_window() {
    let kinds = kinds();
    let all = std::env::var("CANTUS_INST_ONLY").is_err();
    assert!(!all || kinds.len() >= 10, "only {} kinds", kinds.len());
    read(&kinds, if all { 1500 } else { 0 });
}

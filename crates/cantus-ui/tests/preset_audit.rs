//! The preset audit (round 37, ADR 0101): how many factory presets each unit ships, how
//! many say what style and role they are for, and the floor every unit must reach (Chorda's
//! and the kits' too, since piano's and drums' presets landed).
//!
//! `cargo test -p cantus-ui --test preset_audit -- --nocapture` prints the table.

use cantus_core::{EffectKind, InstrumentKind};
use cantus_ui::preset_catalog::{entries, units, Unit};
use cantus_ui::preset_tags::{Role, Style};
use std::collections::HashSet;

/// The floor for each unit (ADR 0101 §1): an instrument that sounds 24, a drum machine 16
/// kits, an effect 12, the analyser 8 (its presets are views). The dancer makes no sound
/// (its presets are dances, not counted); a hosted plug-in keeps its maker's.
fn floor(unit: Unit) -> usize {
    match unit {
        Unit::Instrument(InstrumentKind::Dancer) => 0,
        Unit::Instrument(_) => 24,
        Unit::Kit => 16,
        Unit::Effect(EffectKind::Analyzer) => 8,
        Unit::Effect(EffectKind::External) => 0,
        // The older Reverb is no longer offered (Spatium is the reverb): old projects keep it.
        Unit::Effect(EffectKind::Reverb) => 0,
        Unit::Effect(_) => 12,
    }
}

/// Across the library: every style at least this many instrument presets, every
/// instrument role too, every effect role this many effect presets.
const PER_STYLE: usize = 12;
const PER_ROLE: usize = 12;

fn name(unit: Unit) -> String {
    cantus_ui::i18n::t_in(cantus_ui::i18n::Locale::En, unit.name_key())
}

#[test]
fn every_unit_s_presets_are_counted_and_their_ids_are_unique() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut ids = HashSet::new();
    println!("| Unit | Presets | With a style | With a role | Floor |");
    println!("|---|---|---|---|---|");
    let (mut total, mut styled) = (0, 0);
    for unit in units() {
        let all = entries(unit, false);
        for e in &all {
            assert!(
                ids.insert((format!("{unit:?}"), e.id.clone())),
                "{unit:?}: {} twice",
                e.id
            );
            assert!(!e.name.is_empty() && e.name != e.id, "{}: no name", e.id);
        }
        let s = all.iter().filter(|e| !e.styles.is_empty()).count();
        let r = all.iter().filter(|e| !e.roles.is_empty()).count();
        total += all.len();
        styled += s;
        println!(
            "| {} | {} | {s} | {r} | {} |",
            name(unit),
            all.len(),
            floor(unit)
        );
    }
    println!("| **All** | **{total}** | **{styled}** | | |");
}

#[test]
fn every_unit_reaches_its_floor_and_every_preset_says_what_it_is_for() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut short = Vec::new();
    let mut untagged = Vec::new();
    let mut per_style = vec![0usize; Style::ALL.len()];
    let mut per_role = vec![0usize; Role::ALL.len()];
    for unit in units() {
        let all = entries(unit, false);
        if all.len() < floor(unit) {
            short.push(format!("{}: {} of {}", name(unit), all.len(), floor(unit)));
        }
        if floor(unit) == 0 {
            continue;
        }
        for e in &all {
            if e.styles.is_empty() || e.roles.is_empty() {
                untagged.push(format!("{} {}", name(unit), e.id));
            }
            for s in &e.styles {
                if unit.is_instrument() {
                    per_style[Style::ALL.iter().position(|x| x == s).unwrap()] += 1;
                }
            }
            for r in &e.roles {
                per_role[Role::ALL.iter().position(|x| x == r).unwrap()] += 1;
            }
        }
    }
    let thin_styles: Vec<String> = Style::ALL
        .iter()
        .zip(&per_style)
        .filter(|(_, n)| **n < PER_STYLE)
        .map(|(s, n)| format!("{}: {n}", s.id()))
        .collect();
    let thin_roles: Vec<String> = Role::ALL
        .iter()
        .zip(&per_role)
        .filter(|(_, n)| **n < PER_ROLE)
        .map(|(r, n)| format!("{}: {n}", r.id()))
        .collect();
    assert!(
        short.is_empty() && untagged.is_empty() && thin_styles.is_empty() && thin_roles.is_empty(),
        "below the floor: {short:#?}\nwithout a style or a role: {} presets {untagged:#?}\n\
         styles under {PER_STYLE}: {thin_styles:?}\nroles under {PER_ROLE}: {thin_roles:?}",
        untagged.len()
    );
}

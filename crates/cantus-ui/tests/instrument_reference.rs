//! Instrument gain staging (round 37, ADR 0101 §6, BACKLOG §5): a new instance of every
//! instrument that sounds — its default patch, on the staging material of
//! `instrument_levels.rs` (a C-major triad at velocity 100 for two seconds; Gravis its root
//! an octave down, as a bass plays) — sits at **one reference level**, −18.7 LUFS (the
//! median of the default patches when round 37 began), within ±4.5 LU: the spread a held
//! saw and a struck electric piano have between them at the same velocity. Lamina sits at
//! the bottom of it on purpose (ADR 0039: more would pass full scale at velocity 127).
//!
//! Chorda's model 1 (−30.1 LUFS) is listed: its level is the piano builder's model 2 to set
//! (schema 54 gives every new Chorda model 2); the test fails once a new Chorda is level and
//! the list still names it.

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, Snapshot};
use cantus_engine::{render, RenderOptions};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use std::sync::Arc;

mod measure;
use measure::lufs;

/// The reference level, LUFS, and how far a kind's default patch may sit from it.
const REFERENCE: f64 = -18.7;
const SPREAD: f64 = 4.5;

/// Kinds whose new instance is not yet level, with the builder that owns the fix.
const PENDING: &[(InstrumentKind, &str)] = &[]; // r37:piano: Chorda's model 2 is level (ADR 0098 §3)

fn triad_project(kind: InstrumentKind) -> Project {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let evs: Vec<Ev> = if kind == InstrumentKind::Bass {
        vec![Ev::new(0.0, 4.0, 48, 100)]
    } else {
        [60u8, 64, 67]
            .iter()
            .map(|&n| Ev::new(0.0, 4.0, n, 100))
            .collect()
    };
    let mut s = Song::new("level", 120.0, 1);
    let part = s.part(
        "part",
        InstrumentSpec::Native(NativeSpec::new(kind)),
        colors::KEYS,
        cantus_core::TrackIcon::Keys,
    );
    s.feel(part, Feel::TIGHT);
    s.clip(part, "triad", 0.0, 1.0, &evs);
    s.finish()
}

#[test]
fn every_instrument_s_new_instance_sits_at_the_reference_level() {
    let kinds: Vec<InstrumentKind> = InstrumentKind::ALL
        .into_iter()
        .filter(|k| *k != InstrumentKind::Dancer)
        .collect();
    let rows: Vec<(InstrumentKind, f64)> = std::thread::scope(|scope| {
        let hs: Vec<_> = kinds
            .iter()
            .map(|&k| {
                scope.spawn(move || {
                    let p = triad_project(k);
                    let r = render(
                        Arc::new(Snapshot::compile(&p, 0)),
                        &RenderOptions {
                            tail_seconds: 0.0,
                            ..Default::default()
                        },
                    )
                    .expect("it renders");
                    (k, lufs(&r))
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut bad = Vec::new();
    for (kind, l) in &rows {
        let off = l - REFERENCE;
        println!("{kind:?}: {l:.1} LUFS ({off:+.1} LU)");
        let inside = off.abs() <= SPREAD;
        let pending = PENDING.iter().find(|(k, _)| k == kind);
        match (inside, pending) {
            (false, None) => bad.push(format!("{kind:?}: {off:+.1} LU from the reference")),
            (true, Some((_, who))) => bad.push(format!(
                "{kind:?} is level now ({off:+.1} LU): strike it from PENDING ({who})"
            )),
            _ => {}
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

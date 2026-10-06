//! The engine's performance instruments in words (round 20, `docs/perf/benchmark.md`):
//! who a dropout is blamed on ("Spatium on Vocal took 3.1 ms of 2.7 ms") and what the
//! callback histogram says, for the status bar — and for the benchmark's hot-spot report,
//! which names nodes the same way.

use crate::i18n::{t, tf};
use cantus_core::{EffectId, Project, StripId};
use cantus_engine::{Culprit, Dropout, EngineStats};

/// A strip's name as the desk shows it: its own, else its first track's, else "Insert n".
fn strip_name(p: &Project, id: StripId) -> String {
    let Some(s) = p.strip(id) else {
        return t("mixer.master");
    };
    if !s.name.is_empty() {
        return s.name.clone();
    }
    if let Some(track) = p.tracks_on_strip(id).next() {
        return track.name.clone();
    }
    tf("perf.strip_n", &[("n", &p.strip_number(id).unwrap_or(0))])
}

/// An effect's name as its slot shows it: the hosted plug-in's own, else the kind's.
fn effect_name(p: &Project, id: EffectId) -> String {
    let slot = p
        .strips
        .iter()
        .flat_map(|s| s.effects.iter())
        .chain(p.master.effects.iter())
        .find(|e| e.id == id);
    match slot {
        Some(e) => match e.spec.external.as_deref() {
            Some(x) => x.name.clone(),
            None => t(e.spec.kind.name_key()),
        },
        None => t("perf.engine"),
    }
}

/// A profiler node in the project's words: "Calor on Lead", "Spatium on Vocal", "mixing
/// on Drum bus", "the engine".
pub fn culprit_text(p: &Project, c: Culprit) -> String {
    match c {
        Culprit::Engine => t("perf.engine"),
        Culprit::Instrument(id) => match p.track(id) {
            Some(track) => tf(
                "perf.instrument",
                &[
                    ("instrument", &t(track.instrument.name_key())),
                    ("track", &track.name),
                ],
            ),
            None => t("perf.engine"),
        },
        Culprit::Strip(id) => tf("perf.strip", &[("strip", &strip_name(p, id))]),
        Culprit::Insert { strip, effect, .. } => tf(
            "perf.insert",
            &[
                ("effect", &effect_name(p, effect)),
                ("strip", &strip_name(p, strip)),
            ],
        ),
        Culprit::MasterInsert { effect, .. } => tf(
            "perf.insert",
            &[
                ("effect", &effect_name(p, effect)),
                ("strip", &t("mixer.master")),
            ],
        ),
    }
}

/// Milliseconds as a person reads a block: one decimal, two under a millisecond (a
/// 64-sample buffer at 96 kHz is 0.67 ms).
fn ms(us: f64) -> String {
    if us < 1_000.0 {
        format!("{:.2}", us / 1e3)
    } else {
        format!("{:.1}", us / 1e3)
    }
}

fn ms2(us: f64) -> String {
    format!("{:.2}", us / 1e3)
}

/// The status bar's dropout readout: "Dropout: Spatium on Vocal took 3.1 ms of 2.7 ms".
pub fn dropout_text(p: &Project, d: &Dropout) -> String {
    tf(
        "perf.dropout",
        &[
            ("who", &culprit_text(p, d.culprit)),
            ("ms", &ms(d.culprit_us as f64)),
            ("budget", &ms(d.budget_us as f64)),
        ],
    )
}

/// Its tooltip: the whole block, and what to do about it.
pub fn dropout_tip(p: &Project, d: &Dropout) -> String {
    tf(
        "perf.dropout_tip",
        &[
            ("block", &ms2(d.block_us as f64)),
            ("budget", &ms2(d.budget_us as f64)),
            ("n", &d.block),
            ("who", &culprit_text(p, d.culprit)),
            ("ms", &ms2(d.culprit_us as f64)),
        ],
    )
}

/// The callback histogram in one line, for the engine readout's tooltip.
pub fn histogram_text(stats: &EngineStats) -> String {
    let h = &stats.histogram;
    let budget = if stats.sample_rate > 0.0 {
        stats.block_frames as f64 / stats.sample_rate * 1e6
    } else {
        0.0
    };
    tf(
        "perf.histogram",
        &[
            ("blocks", &h.count()),
            ("p50", &ms2(h.quantile(0.5))),
            ("p99", &ms2(h.quantile(0.99))),
            ("p999", &ms2(h.quantile(0.999))),
            ("max", &ms2(stats.max_block_us)),
            ("budget", &ms2(budget)),
            ("overruns", &stats.overruns),
        ],
    )
}

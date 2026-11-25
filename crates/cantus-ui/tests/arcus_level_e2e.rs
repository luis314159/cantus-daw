//! r42:arcus-p5 — Arcus's presets at one level in music (`docs/round42/arcus-match.md`,
//! *Pass 5*): every bowed factory preset — the violin's, the viola's, the cello's and the
//! bass's — plays the common phrase with CC1 dynamics in its instrument's register
//! (`cantus_ui::arcus_level`) through the real engine's render, as the preset browser loads
//! it, and lands within [`arcus_level::PARITY_LU`] of the others and of the target. The demo
//! heard the violin and the viola 6–7 LU under the cello and the bass at equal faders: the
//! presets' gains had been fitted on velocity-only audition phrases outside the instruments'
//! registers.

use cantus_core::{InstrumentKind, Ticks, PPQ};
use cantus_engine::{render, RenderOptions};
use cantus_ui::arcus_level;
use cantus_ui::preset_catalog::{entries, Payload, Unit};
use cantus_ui::preset_measure::lufs;
use std::sync::Arc;

const RATE: u32 = 48_000;

#[test]
fn every_bowed_preset_is_at_one_level_in_music() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut levels = Vec::new();
    for e in entries(Unit::Instrument(InstrumentKind::BowedString), false) {
        let Payload::Native(spec) = &e.payload else {
            continue;
        };
        if !arcus_level::is_bowed(spec) {
            continue;
        }
        let p = arcus_level::project(spec);
        let snap = Arc::new(cantus_core::Snapshot::compile(&p, 0));
        let out = render(
            snap,
            &RenderOptions {
                sample_rate: RATE,
                start: Ticks::ZERO,
                end: Some(Ticks((arcus_level::BEATS * PPQ as f64) as i64)),
                tail_seconds: 1.0,
                compensate_latency: true,
                ..Default::default()
            },
        )
        .expect("a render")
        .samples;
        let l = lufs(&out, 2, RATE as f32);
        println!("{:40} {l:6.1} LUFS", e.id);
        levels.push((l, e.id.clone()));
    }
    assert!(levels.len() >= 28, "{} bowed presets", levels.len());
    levels.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (lo, hi) = (&levels[0], &levels[levels.len() - 1]);
    println!(
        "spread {:.2} LU ({} {:.1} … {} {:.1})",
        hi.0 - lo.0,
        lo.1,
        lo.0,
        hi.1,
        hi.0
    );
    assert!(
        hi.0 - lo.0 <= arcus_level::PARITY_LU,
        "the presets spread {:.2} LU: {} at {:.1}, {} at {:.1}",
        hi.0 - lo.0,
        lo.1,
        lo.0,
        hi.1,
        hi.0
    );
    for (l, id) in &levels {
        assert!(
            (l - arcus_level::TARGET).abs() <= arcus_level::PARITY_LU,
            "{id}: {l:.1} LUFS against {:.1}",
            arcus_level::TARGET
        );
    }
}

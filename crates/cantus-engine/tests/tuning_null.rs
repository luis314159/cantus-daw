//! Round 42 (ADR 0155) — the null proof of the tuning: every pitched instrument and effect,
//! rendered at the default tuning, dumped byte for byte for `tools/test/ab-compare.sh`, which
//! builds this file once against the contract's code (`--base r42/contract`, the DSP and
//! engine files as they were) and once against the round's: the two dumps are equal when not
//! one sample of a song without a tuning moved. It calls nothing the contract did not have,
//! so it compiles against both.
//!
//!   bash tools/test/ab-compare.sh --crate cantus-engine --bin-test tuning_null \
//!        --test tuning_null_dump --base r42/contract --files "<the round's DSP and engine files>"

use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

fn render_of(p: &Project) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.5,
        block_frames: 256,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("render")
        .samples
}

/// A line with leaps, a held chord and repeated keys (legato and glides where an
/// instrument has them), on `kind` (`None`: Firmus) with `set` applied.
fn line(kind: Option<InstrumentKind>, set: &[(&str, f32)]) -> Project {
    let mut p = Project::new("null");
    p.tempo_bpm = 132.0;
    let mut t = p.make_track("line");
    if let Some(k) = kind {
        let mut spec = NativeSpec::new(k);
        for (key, v) in set {
            assert!(spec.set(key, *v).is_some(), "{k:?} has {key}");
        }
        t.instrument = InstrumentSpec::Native(spec);
    }
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(8));
    let keys = [36u8, 48, 55, 60, 64, 67, 72, 84, 61, 63, 66, 70];
    for (i, &k) in keys.iter().enumerate() {
        // Overlapping notes: legato and glides where the instrument has them.
        let n = p.make_note(
            Ticks::EIGHTH * i as i64,
            Ticks::QUARTER,
            k,
            60 + (i as u8 * 5),
        );
        clip.notes.push(n);
    }
    for k in [48u8, 55, 64, 71] {
        let n = p.make_note(Ticks::beats(4), Ticks::beats(3), k, 100);
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

/// A name, the instrument (`None`: Firmus) and the settings it plays with.
type Played = (
    &'static str,
    Option<InstrumentKind>,
    Vec<(&'static str, f32)>,
);

#[test]
#[ignore = "a dump for tools/test/ab-compare.sh"]
fn tuning_null_dump() {
    let instruments: Vec<Played> = vec![
        ("firmus", None, vec![]),
        ("calor", Some(InstrumentKind::Analog), vec![]),
        (
            "calor-mono-glide",
            Some(InstrumentKind::Analog),
            vec![("voice.mode", 1.0), ("glide.time", 0.2)],
        ),
        ("unda", Some(InstrumentKind::Wavetable), vec![]),
        ("modulus", Some(InstrumentKind::Fm), vec![]),
        ("gravis", Some(InstrumentKind::Bass), vec![]),
        (
            "gravis-string",
            Some(InstrumentKind::Bass),
            vec![("engine", 2.0)],
        ),
        (
            "gravis-strings",
            Some(InstrumentKind::Bass),
            vec![("engine", 2.0), ("mode", 2.0)],
        ),
        ("lamina", Some(InstrumentKind::ElectricPiano), vec![]),
        ("organum", Some(InstrumentKind::Organ), vec![]),
        ("exemplar", Some(InstrumentKind::Sampler), vec![]),
        ("chorda", Some(InstrumentKind::Chorda), vec![]),
        ("fides", Some(InstrumentKind::Resonator), vec![]),
    ];
    let mut bytes = Vec::new();
    let mut put = |name: &str, x: &[f32]| {
        for s in x {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        let peak = x.iter().fold(0f32, |a, s| a.max(s.abs()));
        println!("{name}: {} samples, peak {peak:.3}", x.len());
    };
    for (name, kind, set) in &instruments {
        put(name, &render_of(&line(*kind, set)));
    }
    // The pitched effects on the line's strip, at their defaults.
    for kind in [
        EffectKind::AutoTune,
        EffectKind::PitchShift,
        EffectKind::Enhancer,
        EffectKind::Vocoder,
    ] {
        let mut p = line(Some(InstrumentKind::Analog), &[]);
        let slot = p.make_effect(kind);
        let strip = p.tracks[0].strip.expect("a strip");
        p.strip_mut(strip).unwrap().effects.push(slot);
        put(&format!("{kind:?}"), &render_of(&p));
    }
    if let Ok(path) = std::env::var("CANTUS_AB_DUMP") {
        std::fs::write(path, &bytes).unwrap();
    }
}

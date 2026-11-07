//! What a preset is held to (round 37, ADR 0101 §5): its loudness on its role's material and
//! the window around it, the units that make level by design, each unit's CPU budget — and
//! the integrated loudness they are measured with (ITU-R BS.1770-4, the meters' K-weighting).
//! Shared by the render check, the library's tool and the website's excerpts.

use crate::preset_catalog::{Entry, Unit};
use crate::preset_tags::Role;
use cantus_core::{EffectKind, InstrumentKind};
use cantus_dsp::terminus::loudness::KWeighting;

/// How far a preset may sit from its target, LU.
pub const WINDOW: f64 = 2.0;

/// The peak a preset may reach on the headroom test's dense riff at velocity 127, dBFS: a
/// preset under its window because a louder one would pass this is limited by its own
/// dynamics, not out of level (the render check says so; the fit stops there).
pub const HEADROOM_DB: f64 = -1.0;

/// Each instrument role's loudness on its phrase, LUFS integrated: the instruments'
/// reference level (−18.7 LUFS, the median new instance) for the parts that fill a mix, a
/// little under it for the sparse ones — a pluck's sixteenths and a lead's single line read
/// louder than their integrated figure.
pub fn role_target(role: Role) -> f64 {
    match role {
        Role::Bass => -16.0,
        Role::Lead => -18.0,
        Role::Pad => -19.0,
        // Struck and plucked parts carry the most crest: a level that leaves them their
        // velocity-127 headroom (the headroom test's dense riff under full scale).
        Role::Keys | Role::Pluck => -21.0,
        Role::Fx => -20.0,
        // Drums carry the most crest of all: a kit's groove at the family's level, a little
        // under it — matching −16 drove single hits near full scale (the integrator's
        // decision, round 37; the single pad's 0.6-peak rule holds as before).
        Role::Drums => -19.0,
        _ => -18.0,
    }
}

/// What a preset's level is compared with: an instrument's LUFS on its phrase against its
/// role's target; an effect's change over its dry source against no change.
pub fn target(e: &Entry) -> f64 {
    if e.unit.is_instrument() {
        role_target(e.role())
    } else {
        0.0
    }
}

/// Effects whose job is the level (a limiter, a maximizer, a clipper): no window.
pub fn levels_by_design(unit: Unit) -> bool {
    matches!(
        unit,
        Unit::Effect(EffectKind::Limiter | EffectKind::Maximizer | EffectKind::Clipper)
    )
}

/// Presets whose level is not what they are for, with why: a listen (one part of the
/// signal alone) or a duck that needs its key — no window for them (the clipping check holds).
pub fn by_design(id: &str) -> Option<&'static str> {
    match id {
        "preset.gate.duck" => Some("ducks under its key: alone on a source it ducks itself"),
        "preset.st.side_solo" => Some("a listen: the sides alone"),
        "preset.discrimen.only_transients" => Some("a listen: the transients alone"),
        "preset.discrimen.only_body" => Some("a listen: the body alone"),
        "preset.tonus.octave_down" => Some("an octave down reads quieter by its K-weighting"),
        _ => None,
    }
}

/// The share of one core a render of a preset's phrase may take, in a release build: a
/// tenth for every unit, a fifth for the modelled piano (ADR 0059 holds 64 pedal-down voices
/// within a quarter).
pub fn budget(unit: Unit) -> f64 {
    match unit {
        Unit::Instrument(InstrumentKind::Chorda) => 0.20,
        // A sampler on many overlapping tails at its alias-free quality, and a mastering
        // chain (multiband into a limiter) that runs once per song.
        Unit::Instrument(InstrumentKind::Sampler) => 0.15,
        Unit::Effect(EffectKind::Maximizer) => 0.20,
        _ => 0.10,
    }
}

/// Integrated loudness of interleaved samples (ITU-R BS.1770-4: K-weighting, 400 ms blocks
/// overlapping by 75 %, the absolute gate at −70 LUFS and the relative one 10 LU under).
pub fn lufs(samples: &[f32], ch: usize, sr: f32) -> f64 {
    let ch = ch.max(1);
    let frames = samples.len() / ch;
    let mut k: Vec<KWeighting> = (0..ch).map(|_| KWeighting::new(sr)).collect();
    let mut energy = vec![0f64; frames];
    for (f, slot) in energy.iter_mut().enumerate() {
        let mut e = 0.0;
        for (c, kw) in k.iter_mut().enumerate() {
            let y = kw.tick(f64::from(samples[f * ch + c]));
            e += y * y;
        }
        *slot = e;
    }
    let block = (0.4 * f64::from(sr)) as usize;
    let hop = (block / 4).max(1);
    let mut blocks = Vec::new();
    let mut at = 0;
    while block > 0 && at + block <= frames {
        blocks.push(energy[at..at + block].iter().sum::<f64>() / block as f64);
        at += hop;
    }
    let l = |e: f64| -0.691 + 10.0 * e.max(1e-30).log10();
    let gated: Vec<f64> = blocks.iter().copied().filter(|e| l(*e) > -70.0).collect();
    if gated.is_empty() {
        return -100.0;
    }
    let mean = gated.iter().sum::<f64>() / gated.len() as f64;
    let rel = l(mean) - 10.0;
    let kept: Vec<f64> = gated.into_iter().filter(|e| l(*e) > rel).collect();
    l(kept.iter().sum::<f64>() / kept.len().max(1) as f64)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_full_scale_sine_at_one_kilohertz_reads_minus_three_lufs() {
        // BS.1770: a 1 kHz sine at 0 dBFS on one channel of two reads −3.01 LUFS.
        let sr = 48_000.0f32;
        let mut s = Vec::new();
        for i in 0..(sr as usize * 3) {
            let x = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin();
            s.push(x);
            s.push(0.0);
        }
        let l = super::lufs(&s, 2, sr);
        assert!((l + 3.01).abs() < 0.1, "{l}");
    }
}

//! What `SetProjectTuning` and `SetTrackTuning` refuse (round 42, ADR 0155): a tuning no
//! unit could play. Owned by the `tuning` builder (the contract's checks are the floor).

use crate::command::CommandError;
use crate::tuning::{ScaleDegree, TrackTuning, Tuning, TuningSystem, A4_MAX_HZ, A4_MIN_HZ};

/// The most degrees a scale may have (the Scala archive's largest is under 700).
pub const MAX_DEGREES: usize = 4096;

/// Refuse a diapason out of range, a temperament key past B, a scale with no degree, more
/// than [`MAX_DEGREES`], a degree that is not a positive finite interval, and a keyboard
/// map whose keys or degrees point outside it.
pub fn check(t: &Tuning) -> Result<(), CommandError> {
    let bad = |what: &str| Err(CommandError::InvalidValue(format!("tuning: {what}")));
    if !(A4_MIN_HZ..=A4_MAX_HZ).contains(&t.a4_hz) {
        return bad("diapason out of range");
    }
    match &t.system {
        TuningSystem::Equal | TuningSystem::External => {}
        TuningSystem::Temperament { key, .. } => {
            if *key > 11 {
                return bad("temperament key");
            }
        }
        TuningSystem::Scale(s) => {
            if s.degrees.is_empty() || s.degrees.len() > MAX_DEGREES {
                return bad("scale degrees");
            }
            for d in &s.degrees {
                let ok = match *d {
                    ScaleDegree::Cents(c) => c.is_finite(),
                    ScaleDegree::Ratio(n, d) => n > 0 && d > 0,
                };
                if !ok {
                    return bad("scale degree");
                }
            }
        }
    }
    if let Some(m) = &t.map {
        if m.first_key > m.last_key || m.first_key > 127 || m.last_key > 127 {
            return bad("keyboard map keys");
        }
        if !(m.reference_hz.is_finite() && m.reference_hz > 0.0) {
            return bad("keyboard map reference");
        }
        if m.mapping.len() > m.size as usize {
            return bad("keyboard map size");
        }
    }
    Ok(())
}

/// [`check`] for a track's own tuning.
pub fn check_track(t: &TrackTuning) -> Result<(), CommandError> {
    match t {
        TrackTuning::Own { tuning } => check(tuning),
        _ => Ok(()),
    }
}

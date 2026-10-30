//! What an instrument of ours does with pressure and timbre **out of the box** (ADR 0080).
//!
//! Per-note bend is wired into every listening instrument's pitch (no route: a bend that
//! did not bend would be a bug). Pressure and timbre are modulation sources (`mpe.press`,
//! `mpe.slide`, ADR 0023) that a patch routes where it likes; a freshly inserted Unda, Calor
//! or Modulus comes with two routes in its last matrix slots so an MPE controller is
//! expressive at once — the classic choices: pressure opens the sound (Calor's filter, Unda's
//! and Modulus's level), timbre moves its colour (Unda's wavetable position, Calor's filter,
//! Modulus's brightness — heard once its patch has modulators). Ordinary matrix slots:
//! shown, editable,
//! removable. Without per-note expression the sources are 0 and the routes add nothing —
//! and a project made before keeps its patches as they were (only new instances get them).

use crate::inst::{InstrumentKind, NativeSpec};

/// The matrix slots the defaults use (the last two of 32: the user's start at 1).
pub const SLOTS: [usize; 2] = [31, 32];

/// The default routes of `kind`: `(source, target key, depth)` for pressure and timbre.
pub fn routes(kind: InstrumentKind) -> Option<[(&'static str, &'static str, f32); 2]> {
    Some(match kind {
        InstrumentKind::Wavetable => [
            ("mpe.press", "osc1.level", 0.15),
            ("mpe.slide", "osc1.pos", 0.6),
        ],
        InstrumentKind::Analog => [
            ("mpe.press", "vcf.cutoff", 0.25),
            ("mpe.slide", "vcf.cutoff", 0.2),
        ],
        InstrumentKind::Fm => [
            ("mpe.press", "gain", 0.15),
            ("mpe.slide", "macro.bright", 0.5),
        ],
        _ => return None,
    })
}

/// `spec` with its kind's default routes in [`SLOTS`] (only where those slots are empty).
pub fn with_routes(mut spec: NativeSpec) -> NativeSpec {
    let Some(r) = routes(spec.kind) else {
        return spec;
    };
    for (slot, (src, dst, amt)) in SLOTS.iter().zip(r) {
        let s = format!("mod{slot}.src");
        if spec.text(&s).is_some() {
            continue;
        }
        let _ = spec.set_text(&s, Some(src.to_string()));
        let _ = spec.set_text(&format!("mod{slot}.dst"), Some(dst.to_string()));
        spec.set(&format!("mod{slot}.amt"), amt);
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_name_real_targets_and_fill_only_empty_slots() {
        for kind in [
            InstrumentKind::Wavetable,
            InstrumentKind::Analog,
            InstrumentKind::Fm,
        ] {
            let s = with_routes(NativeSpec::new(kind));
            for (slot, (src, dst, amt)) in SLOTS.iter().zip(routes(kind).unwrap()) {
                assert_eq!(s.text(&format!("mod{slot}.src")), Some(src), "{kind:?}");
                assert_eq!(s.text(&format!("mod{slot}.dst")), Some(dst), "{kind:?}");
                assert_eq!(s.get(&format!("mod{slot}.amt")), amt, "{kind:?}");
                // The target is one of the kind's parameters.
                let mut probe = NativeSpec::new(kind);
                assert!(probe.set(dst, 0.5).is_some(), "{kind:?}: {dst}");
            }
        }
        let mut busy = NativeSpec::new(InstrumentKind::Fm);
        busy.set_text("mod31.src", Some("lfo1".into())).unwrap();
        let s = with_routes(busy);
        assert_eq!(s.text("mod31.src"), Some("lfo1"), "a slot in use is kept");
        assert_eq!(
            with_routes(NativeSpec::new(InstrumentKind::Bass)),
            NativeSpec::new(InstrumentKind::Bass)
        );
    }
}

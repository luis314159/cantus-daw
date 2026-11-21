//! Fragor's five circuits as pedals of their own on Tabula's board (round 41, ADR 0152): their
//! parameter models. Owned by the `pedals` builder; the contract declares the five kinds,
//! their stubs and their names.
//!
//! Fragor (`EffectKind::Pedal`, ADR 0128) enters a board as one pedal — the whole unit, three
//! slots. The owner (2026-11-18, BACKLOG §3b): the green overdrive, the op-amp distortion, the
//! silicon distortion, the germanium fuzz and the triangle muff each a stompbox of its own,
//! so several drives can sit anywhere in a chain, between other pedals, beyond Fragor's three
//! slots. Each is the same DK circuit and **equals Fragor with only that slot filled, to the
//! bit**. They are **board-only** ([`crate::EffectKind::offered`] is false for them: never in
//! an insert menu; Fragor is the insert) and are not exported as plug-ins.
//!
//! Keys are forever once shipped. The builder writes each pedal's table — its circuit's
//! three knobs under Fragor's keys (`od.drive` …), its footswitch, `in`, `out`, `mix`,
//! `gate`, `mono`, `os` as Fragor names them, so a pedal and a one-slot Fragor read the same
//! values — **every default a null to the bit**.
//!
//! **The contract's stub** is a wire times `out` (dB, default 0).

use crate::effect::{p, EffectParamInfo};

/// The five circuits, in Fragor's model order (`fragor_circuits::Model`: Overdrive, OpAmp,
/// Silicon, Fuzz, Muff) and `EffectKind::ALL`'s.
pub const STEMS: [&str; 5] = [
    "fragor_od",
    "fragor_op",
    "fragor_si",
    "fragor_ge",
    "fragor_mf",
];

/// Each pedal's name key, by circuit.
pub const NAME_KEYS: [&str; 5] = [
    "effect.fragor_od",
    "effect.fragor_op",
    "effect.fragor_si",
    "effect.fragor_ge",
    "effect.fragor_mf",
];

pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key.
pub const CHOICES: &[(&str, &str)] = &[];

/// The stub's table, one for all five until the builder writes each circuit's.
pub const PARAMS: &[EffectParamInfo] = &[p(
    "out",
    "fx.fragor_pedal.out",
    -24.0,
    24.0,
    0.0,
    "dB",
    false,
    false,
)];

/// The parameters of circuit `i` (0 … 4, in [`STEMS`]' order).
pub fn params(i: usize) -> &'static [EffectParamInfo] {
    let _ = i;
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_is_a_null() {
        for i in 0..5 {
            for p in params(i) {
                if p.key == "out" {
                    assert_eq!(p.default, 0.0);
                }
            }
        }
    }
}

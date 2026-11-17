//! The palette of Libra's balance (the fx side round, ADR 0130): a brass pair of
//! scales as reviewed data, like a unit's sheet (ADR 0074 §1) — the painter
//! (`views/fx/libra_balance.rs`) lights these and names no colour of its own. An object, so
//! — like a display (ADR 0074 §7) — it keeps its colours in every theme, on the dark screen.
//!
//! Every value is Cantus's own.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// Polished brass: its body, the light along it, the shade under it, its darkest edge.
pub const BRASS: Hsl = hsl(42.0, 0.58, 0.50);
pub const BRASS_LIGHT: Hsl = hsl(46.0, 0.72, 0.78);
pub const BRASS_SHADE: Hsl = hsl(36.0, 0.52, 0.30);
pub const BRASS_EDGE: Hsl = hsl(32.0, 0.45, 0.15);
/// The engraving on the brass and the scale's marks.
pub const ENGRAVE: Hsl = hsl(30.0, 0.40, 0.12);
/// The chains: darker, finer.
pub const CHAIN: Hsl = hsl(38.0, 0.40, 0.42);
/// The slot a pan's level runs in.
pub const SLOT: Hsl = hsl(220.0, 0.12, 0.07);

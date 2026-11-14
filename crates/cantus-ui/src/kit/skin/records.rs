//! The palette of Vetus's record (the fx side round, ADR 0133 provisional): the turntable,
//! the vinyl, its label and the marks of age as reviewed data, like a unit's sheet (ADR 0074
//! §1) — the painter (`views/fx/vetus_record.rs`) lights these and names no colour of its
//! own. A record is a physical object, so — like a display (ADR 0074 §7) — it keeps its
//! colours in every theme: it lies on a dark platter in Slate and in Paper alike.
//!
//! Every value is Cantus's own; no product's colours, label or lettering.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// The well the turntable sits in, darkest at its edges.
pub const PLINTH: Hsl = hsl(30.0, 0.10, 0.075);
pub const PLINTH_EDGE: Hsl = hsl(30.0, 0.08, 0.035);
/// The platter's machined rim and its strobe marks.
pub const PLATTER: Hsl = hsl(210.0, 0.05, 0.22);
pub const PLATTER_LIGHT: Hsl = hsl(210.0, 0.08, 0.62);
pub const STROBE: Hsl = hsl(210.0, 0.10, 0.80);
/// The vinyl: nearly black, a breath of warmth; its grooves a little lighter.
pub const VINYL: Hsl = hsl(240.0, 0.08, 0.045);
pub const VINYL_EDGE: Hsl = hsl(240.0, 0.06, 0.16);
pub const GROOVE: Hsl = hsl(220.0, 0.10, 0.55);
/// The light the vinyl reflects (two soft wedges).
pub const SHEEN: Hsl = hsl(40.0, 0.25, 0.92);
/// The label's paper new and yellowed with age, its print and its ring.
pub const LABEL_NEW: Hsl = hsl(42.0, 0.45, 0.84);
pub const LABEL_OLD: Hsl = hsl(36.0, 0.50, 0.58);
pub const LABEL_INK: Hsl = hsl(12.0, 0.55, 0.22);
pub const LABEL_ACCENT: Hsl = hsl(8.0, 0.70, 0.46);
pub const SPINDLE: Hsl = hsl(210.0, 0.06, 0.70);
/// Dust on the vinyl, a fresh tick's flash, a scratch.
pub const DUST: Hsl = hsl(40.0, 0.15, 0.82);
pub const FLASH: Hsl = hsl(44.0, 0.95, 0.70);
pub const SCRATCH: Hsl = hsl(40.0, 0.10, 0.75);
/// The tonearm: brushed metal, its shadow, the cartridge's body.
pub const ARM: Hsl = hsl(210.0, 0.06, 0.78);
pub const ARM_DARK: Hsl = hsl(210.0, 0.06, 0.36);
pub const ARM_SHADOW: Hsl = hsl(0.0, 0.0, 0.0);
pub const CARTRIDGE: Hsl = hsl(8.0, 0.55, 0.38);

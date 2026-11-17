//! The palette of the objects Arca and Rota draw (the fx side round, wave 3, ADRs 0136 and
//! 0137 provisional): the amp's chassis and valves' surroundings, a speaker's cone, its
//! cabinet's cloth and covering, a microphone, and the rotary speaker's walnut cabinet,
//! horn and drum — reviewed data, like a unit's sheet (ADR 0074 §1). The painters
//! (`views/fx/arca_*.rs`, `views/fx/rota_*.rs`) light these and name no colour of their
//! own. Objects, so — like a display (ADR 0074 §7) — they keep their colours in every theme,
//! on the dark screen.
//!
//! Every value is Cantus's own: no product's colours, cloth pattern or covering.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// The chassis seen through the display: dark steel, its light edge, its shadow.
pub const CHASSIS: Hsl = hsl(30.0, 0.08, 0.10);
pub const CHASSIS_TOP: Hsl = hsl(30.0, 0.07, 0.16);
pub const CHASSIS_EDGE: Hsl = hsl(30.0, 0.06, 0.30);
/// A valve's socket ring on the chassis.
pub const SOCKET: Hsl = hsl(24.0, 0.10, 0.06);

/// A speaker's cone: paper, its ribs' shade, its rim's light; the surround; the dust cap.
pub const CONE: Hsl = hsl(30.0, 0.14, 0.17);
pub const CONE_LIGHT: Hsl = hsl(34.0, 0.16, 0.30);
pub const CONE_SHADE: Hsl = hsl(26.0, 0.12, 0.08);
pub const SURROUND: Hsl = hsl(28.0, 0.10, 0.12);
pub const DUST_CAP: Hsl = hsl(32.0, 0.12, 0.24);
pub const DUST_CAP_LIGHT: Hsl = hsl(36.0, 0.16, 0.42);
/// The speaker's steel basket and its gasket.
pub const BASKET: Hsl = hsl(210.0, 0.06, 0.34);
pub const GASKET: Hsl = hsl(30.0, 0.06, 0.07);

/// The cabinet: its covering (a black vinyl), the cloth over the speakers (an oatmeal weave),
/// the cloth's threads, the piping round its edge.
pub const COVERING: Hsl = hsl(30.0, 0.05, 0.09);
pub const COVERING_TOP: Hsl = hsl(30.0, 0.05, 0.15);
pub const CLOTH: Hsl = hsl(40.0, 0.14, 0.36);
pub const CLOTH_THREAD: Hsl = hsl(40.0, 0.18, 0.50);
pub const PIPING: Hsl = hsl(44.0, 0.20, 0.70);

/// A microphone: its body (satin steel), the grille's mesh, the mesh's light.
pub const MIC_BODY: Hsl = hsl(210.0, 0.06, 0.44);
pub const MIC_BODY_LIGHT: Hsl = hsl(210.0, 0.08, 0.74);
pub const MIC_MESH: Hsl = hsl(210.0, 0.05, 0.22);

/// The rotary cabinet's walnut: its body, its grain's light, its inside (unfinished
/// plywood in the shadow), the louvres' slats.
pub const WALNUT: Hsl = hsl(24.0, 0.42, 0.24);
pub const WALNUT_LIGHT: Hsl = hsl(28.0, 0.46, 0.38);
pub const WALNUT_DARK: Hsl = hsl(20.0, 0.40, 0.12);
pub const PLYWOOD: Hsl = hsl(34.0, 0.24, 0.18);
pub const PLYWOOD_LIGHT: Hsl = hsl(36.0, 0.28, 0.30);
pub const SLAT: Hsl = hsl(26.0, 0.40, 0.30);

/// The horn: black bakelite bells on a chrome rotor; the drum: a dark fibre cylinder with its
/// scoop; the motors' grey castings.
pub const HORN: Hsl = hsl(220.0, 0.06, 0.12);
pub const HORN_LIGHT: Hsl = hsl(220.0, 0.08, 0.34);
pub const CHROME: Hsl = hsl(210.0, 0.08, 0.70);
pub const CHROME_SHADE: Hsl = hsl(215.0, 0.08, 0.32);
pub const DRUM: Hsl = hsl(26.0, 0.16, 0.16);
pub const DRUM_LIGHT: Hsl = hsl(30.0, 0.18, 0.30);
pub const CASTING: Hsl = hsl(210.0, 0.04, 0.28);

/// The amp's copper-wound transformers and the rotary's motor windings.
pub const COPPER: Hsl = hsl(22.0, 0.62, 0.40);
pub const COPPER_LIGHT: Hsl = hsl(26.0, 0.70, 0.60);

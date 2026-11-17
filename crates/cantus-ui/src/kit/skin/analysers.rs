//! fxb:lenis — the palette of the objects Lenis draws (the fx side round, wave B, ADR 0143
//! provisional): the bench analyser the resonance suppressor is — its case and front panel
//! in three finishes (a grey-green laboratory enamel, an ivory enamel, a graphite
//! anodising), the screen-printed legend, the moulded bezel round the screen, the carrying
//! handles, the push keys and their lamps, the screen's phosphors (the spectrum's trace, the
//! cut's veil, the focus curve), the readouts' LED digits — and the black epoxy bench it
//! stands on, its rubber feet, the BNC sockets. Reviewed data, like a unit's sheet (ADR 0074
//! §1): the painters (`views/fx/lenis_*.rs`) light these and name no colour of their own.
//! Objects, so they keep their colours in every theme; the screen stays dark.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// One finish of the instrument: its case, its panel, its print, its bezel, its handles,
/// its keys, and what its screen and readouts glow in.
pub struct Finish {
    /// The case's wrap (the shell round the front panel): its lit top, face and foot.
    pub wrap_top: Hsl,
    pub wrap: Hsl,
    pub wrap_foot: Hsl,
    /// The front panel's enamel: lit top, face, foot; the enamel's fine speckle; the
    /// chamfer's light along its edge.
    pub panel_top: Hsl,
    pub panel: Hsl,
    pub panel_foot: Hsl,
    pub speckle: Hsl,
    pub chamfer: Hsl,
    /// The screen-printed legend, and the finer ink of its rules and scales.
    pub print: Hsl,
    pub rule: Hsl,
    /// The moulded bezel round the screen: its lit lip, face and shadowed foot.
    pub bezel_top: Hsl,
    pub bezel: Hsl,
    pub bezel_foot: Hsl,
    /// The carrying handles: the tube's highlight, its body, its shade.
    pub handle_light: Hsl,
    pub handle: Hsl,
    pub handle_dark: Hsl,
    /// The push keys' caps: lit top, face, foot.
    pub key_top: Hsl,
    pub key: Hsl,
    pub key_foot: Hsl,
    /// The screen: the spectrum's trace, the cut's veil, the focus curve and its nodes.
    pub trace: Hsl,
    pub cut: Hsl,
    pub focus: Hsl,
    /// The readouts' LED digits, lit.
    pub led: Hsl,
    /// The phosphor glass: its ground at the centre and at the rim, its graticule.
    pub glass: Hsl,
    pub glass_rim: Hsl,
    pub graticule: Hsl,
}

/// **Laboratory**: a pale grey-green enamel panel in a blue-grey case, chrome handles, a
/// dark green-grey print; the spectrum an aqua phosphor, the cuts a rose, the focus sand.
pub const LABORATORY: Finish = Finish {
    wrap_top: hsl(200.0, 0.10, 0.46),
    wrap: hsl(202.0, 0.12, 0.32),
    wrap_foot: hsl(206.0, 0.14, 0.17),
    panel_top: hsl(150.0, 0.10, 0.80),
    panel: hsl(152.0, 0.09, 0.71),
    panel_foot: hsl(156.0, 0.09, 0.58),
    speckle: hsl(150.0, 0.08, 0.86),
    chamfer: hsl(140.0, 0.12, 0.92),
    print: hsl(170.0, 0.24, 0.14),
    rule: hsl(168.0, 0.16, 0.30),
    bezel_top: hsl(200.0, 0.06, 0.26),
    bezel: hsl(204.0, 0.08, 0.12),
    bezel_foot: hsl(210.0, 0.10, 0.05),
    handle_light: hsl(200.0, 0.10, 0.96),
    handle: hsl(204.0, 0.07, 0.70),
    handle_dark: hsl(210.0, 0.08, 0.34),
    key_top: hsl(150.0, 0.06, 0.90),
    key: hsl(154.0, 0.06, 0.78),
    key_foot: hsl(160.0, 0.07, 0.52),
    trace: hsl(168.0, 0.70, 0.62),
    cut: hsl(348.0, 0.78, 0.66),
    focus: hsl(44.0, 0.62, 0.72),
    led: hsl(354.0, 0.92, 0.60),
    glass: hsl(172.0, 0.32, 0.065),
    glass_rim: hsl(176.0, 0.30, 0.025),
    graticule: hsl(166.0, 0.22, 0.62),
};

/// **Ivory**: an ivory enamel panel in a warm taupe case, satin nickel handles, a deep
/// brown print; the spectrum an amber phosphor, the cuts a cobalt violet, the focus a pale
/// aqua.
pub const IVORY: Finish = Finish {
    wrap_top: hsl(30.0, 0.12, 0.44),
    wrap: hsl(28.0, 0.14, 0.30),
    wrap_foot: hsl(24.0, 0.16, 0.15),
    panel_top: hsl(44.0, 0.42, 0.92),
    panel: hsl(42.0, 0.34, 0.84),
    panel_foot: hsl(38.0, 0.26, 0.70),
    speckle: hsl(46.0, 0.46, 0.95),
    chamfer: hsl(46.0, 0.60, 0.97),
    print: hsl(22.0, 0.40, 0.15),
    rule: hsl(26.0, 0.26, 0.34),
    bezel_top: hsl(28.0, 0.10, 0.28),
    bezel: hsl(26.0, 0.12, 0.13),
    bezel_foot: hsl(24.0, 0.14, 0.05),
    handle_light: hsl(44.0, 0.24, 0.95),
    handle: hsl(40.0, 0.12, 0.68),
    handle_dark: hsl(34.0, 0.12, 0.34),
    key_top: hsl(44.0, 0.40, 0.95),
    key: hsl(42.0, 0.30, 0.86),
    key_foot: hsl(36.0, 0.22, 0.60),
    trace: hsl(42.0, 0.90, 0.62),
    cut: hsl(258.0, 0.62, 0.76),
    focus: hsl(170.0, 0.46, 0.70),
    led: hsl(38.0, 0.96, 0.58),
    glass: hsl(30.0, 0.28, 0.06),
    glass_rim: hsl(26.0, 0.30, 0.022),
    graticule: hsl(40.0, 0.30, 0.62),
};

/// **Graphite**: a graphite anodised panel in a black case, black satin handles, a white
/// print; the spectrum a white phosphor, the cuts an orange, the focus a pale cyan.
pub const GRAPHITE: Finish = Finish {
    wrap_top: hsl(220.0, 0.06, 0.20),
    wrap: hsl(222.0, 0.07, 0.11),
    wrap_foot: hsl(226.0, 0.08, 0.05),
    panel_top: hsl(218.0, 0.06, 0.27),
    panel: hsl(220.0, 0.06, 0.19),
    panel_foot: hsl(224.0, 0.07, 0.12),
    speckle: hsl(214.0, 0.06, 0.34),
    chamfer: hsl(210.0, 0.08, 0.52),
    print: hsl(210.0, 0.10, 0.90),
    rule: hsl(212.0, 0.08, 0.62),
    bezel_top: hsl(220.0, 0.06, 0.22),
    bezel: hsl(224.0, 0.08, 0.08),
    bezel_foot: hsl(228.0, 0.10, 0.03),
    handle_light: hsl(214.0, 0.08, 0.62),
    handle: hsl(220.0, 0.06, 0.24),
    handle_dark: hsl(226.0, 0.08, 0.07),
    key_top: hsl(218.0, 0.06, 0.36),
    key: hsl(220.0, 0.06, 0.25),
    key_foot: hsl(224.0, 0.07, 0.12),
    trace: hsl(228.0, 0.34, 0.88),
    cut: hsl(24.0, 0.90, 0.62),
    focus: hsl(188.0, 0.56, 0.66),
    led: hsl(22.0, 0.96, 0.58),
    glass: hsl(218.0, 0.22, 0.06),
    glass_rim: hsl(222.0, 0.24, 0.022),
    graticule: hsl(212.0, 0.14, 0.66),
};

/// The bench: black epoxy resin, a fine speckle, the light along its front edge.
pub const BENCH: Hsl = hsl(210.0, 0.08, 0.075);
pub const BENCH_FAR: Hsl = hsl(214.0, 0.10, 0.04);
pub const BENCH_SPECK: Hsl = hsl(205.0, 0.06, 0.24);
/// The instrument's rubber feet.
pub const FOOT: Hsl = hsl(220.0, 0.05, 0.10);
pub const FOOT_LIGHT: Hsl = hsl(214.0, 0.05, 0.30);
/// The panel's screws: a nickel pan head.
pub const SCREW: Hsl = hsl(206.0, 0.06, 0.62);
pub const SCREW_LIGHT: Hsl = hsl(200.0, 0.08, 0.92);
pub const SCREW_DARK: Hsl = hsl(212.0, 0.08, 0.26);
/// A BNC socket: its nickel shell, the bayonet pins, the white insulator, the gold pin.
pub const NICKEL: Hsl = hsl(204.0, 0.06, 0.74);
pub const NICKEL_LIGHT: Hsl = hsl(200.0, 0.10, 0.95);
pub const NICKEL_DARK: Hsl = hsl(210.0, 0.08, 0.30);
pub const INSULATOR: Hsl = hsl(48.0, 0.18, 0.90);
pub const PIN: Hsl = hsl(44.0, 0.70, 0.58);
/// The maker's badge: black enamel in the nickel plate's stamping.
pub const BADGE_INK: Hsl = hsl(222.0, 0.12, 0.10);
/// The readouts' smoked window, and an unlit LED segment.
pub const SMOKE: Hsl = hsl(350.0, 0.14, 0.035);
pub const LED_OFF: Hsl = hsl(350.0, 0.10, 0.11);
/// A key's lamp window unlit.
pub const LAMP_OFF: Hsl = hsl(210.0, 0.06, 0.20);
/// The signal lamps beside the sockets: a green LED.
pub const SIGNAL: Hsl = hsl(132.0, 0.80, 0.52);
/// Shadow under a part.
pub const SHADOW: Hsl = hsl(0.0, 0.0, 0.0);
/// Light on glass.
pub const GLINT: Hsl = hsl(190.0, 0.30, 0.96);

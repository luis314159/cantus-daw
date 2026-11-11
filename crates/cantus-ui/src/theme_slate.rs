//! **Slate** (round 39, ADR 0111) — the default theme family, from `dir-a`'s system
//! (`docs/round39/design/dir-a/`): a cool, quiet grey-blue room. Panels are separated by
//! value, never by borders — a dark gutter between panels, a lighter bar for headers and
//! strips, a darker body for lanes. Keys are recessed wells a shade darker than their bar.
//! One accent, a calm azure (Night) or a deep blue (Paper), spent only on state. Set in
//! Barlow Semi Condensed with tabular figures ([`crate::kit::system::Face::Barlow`]).
//!
//! The values are the design's, token by token (`src/base.py` `NIGHT`/`PAPER`): gutter →
//! `ink`, base → `panel` and the lanes, surface → bars, strips and decks, raised → menus
//! and header cells, well/edge/lip → the keys' and fields' wells, ink 1–3 → text,
//! secondary and placeholder. Where the design's colour falls short of a promise the kit
//! makes it is moved the least that holds it, and said here:
//! - Paper's record red is `#c93434` (the design's `#d33b3b` is 4.3:1 on the base, and a
//!   warning written in it must read at 4.5:1).
//! - Paper's meters run in a white groove (`raised`): its green and amber are 2.2:1 on the
//!   design's grey well, and a meter must read at 3:1.
//! - The function hues (device sections, ADR 0024 §2), which the design does not draw, are
//!   picked at 4.5:1 on a card, its title band and a display, in both lights.

use crate::kit::system::System;
use crate::kit::tokens::Tokens;
use crate::theme::Theme;
use crate::theme_family::palette_of;
use egui::Color32;

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// A theme of the family: its tokens (and so its system), the palette taken from them.
fn family(name: &str, t: Tokens, accent: Color32, accent_bright: Color32) -> Theme {
    let mut theme = Theme::from_palette(name, &palette_of(&t, accent, accent_bright)).with_kit(&t);
    theme.system = t.system;
    theme
}

/// Slate Night — the default (round 39). Its tokens, as designed.
pub const fn slate_night_tokens() -> Tokens {
    Tokens {
        ink: hex(0x14191e),
        well: hex(0x1a1f25),
        panel: hex(0x1d2329),
        toolbar: hex(0x262d34),
        panel_raised: hex(0x262d34),
        panel_light: hex(0x1d2329),
        grid: hex(0x21282e),
        grid_strong: hex(0x262e35),
        grid_bar: hex(0x323b44),
        edge: hex(0x7a8692),
        text: hex(0xe3e8ed),
        text_dim: hex(0xa6b0ba),
        label: hex(0xa6b0ba),
        placeholder: hex(0x7a8692),
        text_select: hex(0x1f3a52),
        brand: hex(0x5cafe9),
        brand_bright: hex(0x8cc8f2),
        highlight: hex(0x5cafe9),
        highlight_band: hex(0x9fd0f4),
        highlight_glyph: hex(0x0d1822),
        selection: hex(0x5cafe9),
        focus: hex(0x5cafe9),
        led: hex(0x5dce8a),
        danger: hex(0xee5c5a),
        fx_on: hex(0x5dce8a),
        meter_well: hex(0x11151a),
        meter_low: hex(0x5dce8a),
        meter_high: hex(0xebb64d),
        meter_clip: hex(0xff5d50),
        strip: hex(0x262d34),
        strip_selected: hex(0x2f373f),
        header_cell: hex(0x2f373f),
        cap: hex(0x434c56),
        knob_body: hex(0x2f373f),
        knob_rim: hex(0x76818d),
        knob_track: hex(0x707b87),
        knob_pointer: hex(0xe3e8ed),
        device_ground: hex(0x14191e),
        deck: hex(0x262d34),
        deck_title: hex(0x2b333b),
        value_pill: hex(0x1a1f25),
        value_pill_text: hex(0xe3e8ed),
        step_odd: hex(0x2f373f),
        step_even: hex(0x38414a),
        step_odd_lit: hex(0x5cafe9),
        step_even_lit: hex(0x8cc8f2),
        key_white: hex(0xdde2e8),
        key_black: hex(0x1a1f25),
        row_white: hex(0x20272d),
        row_black: hex(0x1d2329),
        playhead: hex(0x5cafe9),
        menu: hex(0x2f373f),
        menu_band: hex(0x293038),
        menu_hover: hex(0x3c5b72),
        menu_text: hex(0xe3e8ed),
        menu_text_hover: hex(0xf4f7fa),
        hue_source: hex(0x86b6f2),
        hue_tone: hex(0x4fc6c0),
        hue_dynamics: hex(0xebb64d),
        hue_mod: hex(0xb99af5),
        hue_space: hex(0xec9ac6),
        hue_output: hex(0xa3d277),
        disabled: hex(0x47515b),    // r39:el-controls
        auto: hex(0xb48df4),        // r39:el-controls
        modulated: hex(0xee86c8),   // r39:el-controls
        destructive: hex(0xc8443f), // r39:el-controls
        primary_ink: hex(0x0d1822), // r39:el-controls
        meter_mid: hex(0xc9d36a),   // r39:el-controls
        fcap_hi: hex(0x8a95a1),     // r39:el-controls
        fcap_mid: hex(0x66707b),    // r39:el-controls
        fcap_lo: hex(0x525b65),     // r39:el-controls
        cap_hi: hex(0x4a545e),      // r39:el-controls
        cap_mid: hex(0x353e47),     // r39:el-controls
        cap_lo: hex(0x272e35),      // r39:el-controls
        scrim: Color32::from_rgba_unmultiplied_const(8, 11, 14, 158), // r39:el-controls
        shadow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 140),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::from_rgba_unmultiplied_const(0x1d, 0x23, 0x29, 150), // r39: the user's colour a tint at rest
        readout: Color32::TRANSPARENT, // r39:lapis2 — each readout in its own ink
        readout_soft: Color32::TRANSPARENT,
        selection_tint: Color32::TRANSPARENT,
        loop_edge: Color32::TRANSPARENT,
        sheen: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow_cool: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        bevel: Color32::from_rgba_unmultiplied_const(0xe3, 0xe8, 0xed, 20),
        bevel_shade: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 70),
        inner_shadow: Color32::from_rgba_unmultiplied_const(0x11, 0x15, 0x1a, 255),
        lip: Color32::from_rgba_unmultiplied_const(0x2e, 0x36, 0x3e, 255),
        specular: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 20),
        drop: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 90),
        dark: true,
        radius: 4.0,
        round: 1.0,
        depth: 0.5,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
        system: System::SLATE,
    }
}

/// Slate Night — the default (round 39).
pub fn slate_night() -> Theme {
    family(
        "Slate Night",
        slate_night_tokens(),
        hex(0x5cafe9),
        hex(0x8cc8f2),
    )
}

/// Slate Paper — Slate in daylight. Its tokens, as designed.
pub const fn slate_paper_tokens() -> Tokens {
    Tokens {
        ink: hex(0xfbfcfd),
        well: hex(0xd6dde3),
        panel: hex(0xf4f6f8),
        toolbar: hex(0xe4e9ee),
        panel_raised: hex(0xe4e9ee),
        panel_light: hex(0xf4f6f8),
        grid: hex(0xe8ecf0),
        grid_strong: hex(0xd9dfe5),
        grid_bar: hex(0xc8d0d8),
        edge: hex(0x6a7684),
        text: hex(0x17202a),
        text_dim: hex(0x4a5867),
        label: hex(0x4a5867),
        placeholder: hex(0x6a7684),
        text_select: hex(0x8fb4e0),
        brand: hex(0x1f6ec2),
        brand_bright: hex(0x3d8ad8),
        highlight: hex(0x1f6ec2),
        highlight_band: hex(0x8db8e6),
        highlight_glyph: hex(0xffffff),
        selection: hex(0x1f6ec2),
        focus: hex(0x1f6ec2),
        led: hex(0x1f8f55),
        danger: hex(0xc93434),
        fx_on: hex(0x1f8f55),
        meter_well: hex(0xfbfcfd),
        meter_low: hex(0x1f8f55),
        meter_high: hex(0xa86e06),
        meter_clip: hex(0xd6322b),
        strip: hex(0xe4e9ee),
        strip_selected: hex(0xd9e0e7),
        header_cell: hex(0xfbfcfd),
        cap: hex(0xf3f6f8),
        knob_body: hex(0xf6f8fa),
        knob_rim: hex(0x6a7684),
        knob_track: hex(0x737f8c),
        knob_pointer: hex(0x17202a),
        device_ground: hex(0xc5cdd5),
        deck: hex(0xe4e9ee),
        deck_title: hex(0xdfe5ea),
        value_pill: hex(0xfbfcfd),
        value_pill_text: hex(0x17202a),
        step_odd: hex(0xd6dde3),
        step_even: hex(0xccd4db),
        step_odd_lit: hex(0x1f6ec2),
        step_even_lit: hex(0x4d8fd6),
        key_white: hex(0xffffff),
        key_black: hex(0x2a333d),
        row_white: hex(0xeef1f4),
        row_black: hex(0xf4f6f8),
        playhead: hex(0x1f6ec2),
        menu: hex(0xfbfcfd),
        menu_band: hex(0xeef1f4),
        menu_hover: hex(0x1f6ec2),
        menu_text: hex(0x17202a),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x2a62b8),
        hue_tone: hex(0x0d6c68),
        hue_dynamics: hex(0x7c5000),
        hue_mod: hex(0x6a40c4),
        hue_space: hex(0xa8336f),
        hue_output: hex(0x456a1c),
        disabled: hex(0xa9b3bd),    // r39:el-controls
        auto: hex(0x784dd2), // r39:el-controls — the design's #7a4fd6 moved the least to 4.5:1 on a bar
        modulated: hex(0xb23886), // r39:el-controls — the design's #b83a8a moved the least to 4.5:1 on a bar
        destructive: hex(0xc8352f), // r39:el-controls
        primary_ink: hex(0xffffff), // r39:el-controls
        meter_mid: hex(0x8e9928), // r39:el-controls — the design's #9aa62c is 2.6:1 in the white groove
        fcap_hi: hex(0xffffff),   // r39:el-controls
        fcap_mid: hex(0xf3f6f8),  // r39:el-controls
        fcap_lo: hex(0xdae1e7),   // r39:el-controls
        cap_hi: hex(0xffffff),    // r39:el-controls
        cap_mid: hex(0xf5f7f9),   // r39:el-controls
        cap_lo: hex(0xdde3e9),    // r39:el-controls
        scrim: Color32::from_rgba_unmultiplied_const(34, 46, 58, 87), // r39:el-controls
        shadow: Color32::from_rgba_unmultiplied_const(0x28, 0x37, 0x46, 56),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::from_rgba_unmultiplied_const(0xf4, 0xf6, 0xf8, 150), // r39: the user's colour a tint at rest
        readout: Color32::TRANSPARENT, // r39:lapis2 — each readout in its own ink
        readout_soft: Color32::TRANSPARENT,
        selection_tint: Color32::TRANSPARENT,
        loop_edge: Color32::TRANSPARENT,
        sheen: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow_cool: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        bevel: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 150),
        bevel_shade: Color32::from_rgba_unmultiplied_const(0x3c, 0x4b, 0x5a, 40),
        inner_shadow: Color32::from_rgba_unmultiplied_const(0xb9, 0xc3, 0xcc, 255),
        lip: Color32::from_rgba_unmultiplied_const(0xee, 0xf2, 0xf5, 255),
        specular: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 60),
        drop: Color32::from_rgba_unmultiplied_const(0x28, 0x37, 0x46, 40),
        dark: false,
        radius: 4.0,
        round: 1.0,
        depth: 0.5,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
        system: System::SLATE,
    }
}

/// Slate Paper — Slate in daylight.
pub fn slate_paper() -> Theme {
    family(
        "Slate Paper",
        slate_paper_tokens(),
        hex(0x1f6ec2),
        hex(0x3d8ad8),
    )
}

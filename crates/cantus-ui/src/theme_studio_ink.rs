//! **Studio ink** (round 39, an optional theme family, ADR 0111) — `dir-c`'s system
//! (`docs/round39/design/dir-c/`): the chrome is ink and the music is light. Night's
//! surfaces are near-blacks with a cold breath of blue-green, set in three inks and
//! hairlines; Paper turns the metaphor over. One pen, a calm azure, marks what is on or
//! selected. Set in IBM Plex Sans Condensed with Plex Mono for values and an ExtraLight
//! clock ([`crate::kit::system::Face::PlexInk`]).
//!
//! The design's values, token by token (`src/tokens.py`); keys and fields use its `field`
//! (Night) and `track` (Paper) as their well. The function hues, which the design does not
//! draw, are picked at 4.5:1 on cards and displays.

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

/// Studio ink, Night. Its tokens, as designed.
pub const fn studio_ink_night_tokens() -> Tokens {
    Tokens {
        ink: hex(0x08090b),
        well: hex(0x20252b),
        panel: hex(0x0e1013),
        toolbar: hex(0x13161a),
        panel_raised: hex(0x1a1e23),
        panel_light: hex(0x0e1013),
        grid: hex(0x101318),
        grid_strong: hex(0x181c22),
        grid_bar: hex(0x22272e),
        edge: hex(0x7a838e),
        text: hex(0xe7ebef),
        text_dim: hex(0xa2abb5),
        label: hex(0xa2abb5),
        placeholder: hex(0x7a838e),
        text_select: hex(0x1d4a7a),
        brand: hex(0x62b4ff),
        brand_bright: hex(0x9ccfff),
        highlight: hex(0x62b4ff),
        highlight_band: hex(0x9ccfff),
        highlight_glyph: hex(0x061423),
        selection: hex(0x62b4ff),
        focus: hex(0x62b4ff),
        led: hex(0x42dd8f),
        danger: hex(0xff5a4e),
        fx_on: hex(0x42dd8f),
        meter_well: hex(0x08090b),
        meter_low: hex(0x3fd39b),
        meter_high: hex(0xd9e264),
        meter_clip: hex(0xff4a3d),
        strip: hex(0x13161a),
        strip_selected: hex(0x1a1e23),
        header_cell: hex(0x1a1e23),
        cap: hex(0x3a414a),
        knob_body: hex(0x1a1e23),
        knob_rim: hex(0x6f7883),
        knob_track: hex(0x6c7580),
        knob_pointer: hex(0xe7ebef),
        device_ground: hex(0x08090b),
        deck: hex(0x13161a),
        deck_title: hex(0x1a1e23),
        value_pill: hex(0x20252b),
        value_pill_text: hex(0xe7ebef),
        step_odd: hex(0x1a1e23),
        step_even: hex(0x22272e),
        step_odd_lit: hex(0x62b4ff),
        step_even_lit: hex(0x9ccfff),
        key_white: hex(0xe2e6ea),
        key_black: hex(0x0e1013),
        row_white: hex(0x0b0c0f),
        row_black: hex(0x08090b),
        playhead: hex(0x62b4ff),
        menu: hex(0x171a1f),
        menu_band: hex(0x13161a),
        menu_hover: hex(0x1d4a7a),
        menu_text: hex(0xe7ebef),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x6cb6ff),
        hue_tone: hex(0x3fd3c0),
        hue_dynamics: hex(0xf2c24f),
        hue_mod: hex(0xb495f7),
        hue_space: hex(0xf08cc4),
        hue_output: hex(0xa6d96c),
        disabled: hex(0x51565c),    // r39:el-controls
        auto: hex(0xb495f7),        // r39:el-controls
        modulated: hex(0xf08cc4),   // r39:el-controls
        destructive: hex(0xcc483e), // r39:el-controls
        primary_ink: hex(0x061423), // r39:el-controls
        meter_mid: hex(0x8cda80),   // r39:el-controls
        fcap_hi: hex(0x83878b),     // r39:el-controls
        fcap_mid: hex(0x5b5e62),    // r39:el-controls
        fcap_lo: hex(0x44474b),     // r39:el-controls
        cap_hi: hex(0x3b3e42),      // r39:el-controls
        cap_mid: hex(0x24272b),     // r39:el-controls
        cap_lo: hex(0x15181c),      // r39:el-controls
        scrim: Color32::from_rgba_unmultiplied_const(8, 11, 14, 158), // r39:el-controls
        shadow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 140),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::from_rgba_unmultiplied_const(0x0e, 0x10, 0x13, 150), // r39: the user's colour a tint at rest
        readout: Color32::TRANSPARENT, // r39:lapis2 — each readout in its own ink
        readout_soft: Color32::TRANSPARENT,
        selection_tint: Color32::TRANSPARENT,
        loop_edge: Color32::TRANSPARENT,
        sheen: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow_cool: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        bevel: Color32::from_rgba_unmultiplied_const(0xe7, 0xeb, 0xef, 14),
        bevel_shade: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 60),
        inner_shadow: Color32::from_rgba_unmultiplied_const(0x08, 0x09, 0x0b, 255),
        lip: Color32::from_rgba_unmultiplied_const(0x1d, 0x21, 0x27, 255),
        specular: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 14),
        drop: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 90),
        dark: true,
        radius: 3.0,
        round: 1.0,
        depth: 0.4,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
        system: System::STUDIO_INK,
    }
}

/// Studio ink, Night.
pub fn studio_ink_night() -> Theme {
    family(
        "Studio Ink Night",
        studio_ink_night_tokens(),
        hex(0x62b4ff),
        hex(0x9ccfff),
    )
}

/// Studio ink, Paper. Its tokens, as designed.
pub const fn studio_ink_paper_tokens() -> Tokens {
    Tokens {
        ink: hex(0xffffff),
        well: hex(0xe2e2dd),
        panel: hex(0xf6f6f4),
        toolbar: hex(0xeeeeeb),
        panel_raised: hex(0xe7e7e3),
        panel_light: hex(0xf6f6f4),
        grid: hex(0xf3f3f1),
        grid_strong: hex(0xe6e6e2),
        grid_bar: hex(0xd6d6d1),
        edge: hex(0x6a707a),
        text: hex(0x16191d),
        text_dim: hex(0x454b53),
        label: hex(0x454b53),
        placeholder: hex(0x6a707a),
        text_select: hex(0x9dbdec),
        brand: hex(0x1e64d0),
        brand_bright: hex(0x4a86e0),
        highlight: hex(0x1e64d0),
        highlight_band: hex(0x9fbdf0),
        highlight_glyph: hex(0xffffff),
        selection: hex(0x1e64d0),
        focus: hex(0x1e64d0),
        led: hex(0x0b8a47),
        danger: hex(0xcc2b20),
        fx_on: hex(0x0b8a47),
        meter_well: hex(0xffffff),
        meter_low: hex(0x0a8457),
        meter_high: hex(0x857400),
        meter_clip: hex(0xc9381c),
        strip: hex(0xeeeeeb),
        strip_selected: hex(0xe2e2dd),
        header_cell: hex(0xffffff),
        cap: hex(0xfbfbfa),
        knob_body: hex(0xfbfbfa),
        knob_rim: hex(0x6a707a),
        knob_track: hex(0x7b818a),
        knob_pointer: hex(0x16191d),
        device_ground: hex(0xdcdcd7),
        deck: hex(0xeeeeeb),
        deck_title: hex(0xe7e7e3),
        value_pill: hex(0xffffff),
        value_pill_text: hex(0x16191d),
        step_odd: hex(0xe2e2dd),
        step_even: hex(0xd6d6d1),
        step_odd_lit: hex(0x1e64d0),
        step_even_lit: hex(0x4a86e0),
        key_white: hex(0xffffff),
        key_black: hex(0x22262b),
        row_white: hex(0xf6f6f4),
        row_black: hex(0xffffff),
        playhead: hex(0x1e64d0),
        menu: hex(0xfbfbfa),
        menu_band: hex(0xf3f3f1),
        menu_hover: hex(0x1e64d0),
        menu_text: hex(0x16191d),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x1b5abd),
        hue_tone: hex(0x086c66),
        hue_dynamics: hex(0x7f5800),
        hue_mod: hex(0x6a44c8),
        hue_space: hex(0xa8326b),
        hue_output: hex(0x4a7020),
        disabled: hex(0xa6a9ac),    // r39:el-controls
        auto: hex(0x6a44c8),        // r39:el-controls
        modulated: hex(0xa8326b),   // r39:el-controls
        destructive: hex(0xcc2b20), // r39:el-controls
        primary_ink: hex(0xffffff), // r39:el-controls
        meter_mid: hex(0x487c2c),   // r39:el-controls
        fcap_hi: hex(0xffffff),     // r39:el-controls
        fcap_mid: hex(0xf8f8f7),    // r39:el-controls
        fcap_lo: hex(0xe1e1df),     // r39:el-controls
        cap_hi: hex(0xffffff),      // r39:el-controls
        cap_mid: hex(0xfafaf9),     // r39:el-controls
        cap_lo: hex(0xe5e5e3),      // r39:el-controls
        scrim: Color32::from_rgba_unmultiplied_const(34, 46, 58, 87), // r39:el-controls
        shadow: Color32::from_rgba_unmultiplied_const(0x28, 0x28, 0x1e, 46),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::from_rgba_unmultiplied_const(0xf6, 0xf6, 0xf4, 150), // r39: the user's colour a tint at rest
        readout: Color32::TRANSPARENT, // r39:lapis2 — each readout in its own ink
        readout_soft: Color32::TRANSPARENT,
        selection_tint: Color32::TRANSPARENT,
        loop_edge: Color32::TRANSPARENT,
        sheen: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow_cool: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        bevel: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 150),
        bevel_shade: Color32::from_rgba_unmultiplied_const(0x28, 0x28, 0x1e, 36),
        inner_shadow: Color32::from_rgba_unmultiplied_const(0xc6, 0xc6, 0xc0, 255),
        lip: Color32::from_rgba_unmultiplied_const(0xfb, 0xfb, 0xfa, 255),
        specular: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 60),
        drop: Color32::from_rgba_unmultiplied_const(0x28, 0x28, 0x1e, 36),
        dark: false,
        radius: 3.0,
        round: 1.0,
        depth: 0.4,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
        system: System::STUDIO_INK,
    }
}

/// Studio ink, Paper.
pub fn studio_ink_paper() -> Theme {
    family(
        "Studio Ink Paper",
        studio_ink_paper_tokens(),
        hex(0x1e64d0),
        hex(0x4a86e0),
    )
}

//! **Graphite and gold** (round 39, an optional theme family, ADR 0111) — `dir-b`'s system
//! (`docs/round39/design/dir-b/`): a neutral graphite workspace (every grey nearly R = G =
//! B) lit by Cantus's own metal. Gold means on, selected, focus, the playhead; teal means
//! signal and an effect on; red is kept for record and clip. Set in Sofia Sans in three
//! widths ([`crate::kit::system::Face::Sofia`]); keys are dark wells with a light lip.
//!
//! The design's values, token by token (`src/core.py` `NIGHT`/`PAPER`); moved the least
//! that holds a promise of the kit's, and said here:
//! - Paper's red is `#bf3220` (the design's `#cf3a27` is 4.3:1 on the panel).
//! - Paper's meters are a step deeper (`#2a9078`, `#9c760c`) in a white groove, at 3:1.
//! - The function hues, which the design does not draw, at 4.5:1 on cards and displays.

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

/// Graphite and gold, Night. Its tokens, as designed.
pub const fn graphite_night_tokens() -> Tokens {
    Tokens {
        ink: hex(0x111111),
        well: hex(0x1a1a19),
        panel: hex(0x1e1e1d),
        toolbar: hex(0x242423),
        panel_raised: hex(0x242423),
        panel_light: hex(0x1e1e1d),
        grid: hex(0x262625),
        grid_strong: hex(0x323230),
        grid_bar: hex(0x44433f),
        edge: hex(0x8e8a83),
        text: hex(0xecebe4),
        text_dim: hex(0xb3afa7),
        label: hex(0xb3afa7),
        placeholder: hex(0x8e8a83),
        text_select: hex(0x4a3d22),
        brand: hex(0xe6b450),
        brand_bright: hex(0xf5d58e),
        highlight: hex(0xe6b450),
        highlight_band: hex(0xf5d58e),
        highlight_glyph: hex(0x1a1406),
        selection: hex(0xe6b450),
        focus: hex(0xe6b450),
        led: hex(0x5cc2a9),
        danger: hex(0xee5a45),
        fx_on: hex(0x5cc2a9),
        meter_well: hex(0x0b0b0b),
        meter_low: hex(0x5cc2a9),
        meter_high: hex(0xd9c25a),
        meter_clip: hex(0xee5a45),
        strip: hex(0x242423),
        strip_selected: hex(0x323231),
        header_cell: hex(0x2b2b2a),
        cap: hex(0x4a4945),
        knob_body: hex(0x2b2b2a),
        knob_rim: hex(0x85817a),
        knob_track: hex(0x7a766f),
        knob_pointer: hex(0xecebe4),
        device_ground: hex(0x111111),
        deck: hex(0x242423),
        deck_title: hex(0x2b2b2a),
        value_pill: hex(0x1a1a19),
        value_pill_text: hex(0xecebe4),
        step_odd: hex(0x2b2b2a),
        step_even: hex(0x353533),
        step_odd_lit: hex(0xe6b450),
        step_even_lit: hex(0xf5d58e),
        key_white: hex(0xe8e5df),
        key_black: hex(0x1a1a19),
        row_white: hex(0x1c1c1b),
        row_black: hex(0x181818),
        playhead: hex(0xe6b450),
        menu: hex(0x2b2b2a),
        menu_band: hex(0x262625),
        menu_hover: hex(0x5f5135),
        menu_text: hex(0xecebe4),
        menu_text_hover: hex(0xfff8ea),
        hue_source: hex(0x8ab4f0),
        hue_tone: hex(0x5cc2a9),
        hue_dynamics: hex(0xe6b450),
        hue_mod: hex(0xb9a0f0),
        hue_space: hex(0xec9ac0),
        hue_output: hex(0xa8cf7a),
        disabled: hex(0x615f5b),    // r39:el-controls
        auto: hex(0xb9a0f0),        // r39:el-controls
        modulated: hex(0xec9ac0),   // r39:el-controls
        destructive: hex(0xca4c3b), // r39:el-controls
        primary_ink: hex(0x1a1406), // r39:el-controls
        meter_mid: hex(0x9ac282),   // r39:el-controls
        fcap_hi: hex(0x8e8d89),     // r39:el-controls
        fcap_mid: hex(0x686865),    // r39:el-controls
        fcap_lo: hex(0x52524f),     // r39:el-controls
        cap_hi: hex(0x4a4a48),      // r39:el-controls
        cap_mid: hex(0x343432),     // r39:el-controls
        cap_lo: hex(0x262625),      // r39:el-controls
        scrim: Color32::from_rgba_unmultiplied_const(8, 11, 14, 158), // r39:el-controls
        shadow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 140),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::from_rgba_unmultiplied_const(0x18, 0x18, 0x18, 150), // r39: the user's colour a tint at rest
        readout: Color32::TRANSPARENT, // r39:lapis2 — each readout in its own ink
        readout_soft: Color32::TRANSPARENT,
        selection_tint: Color32::TRANSPARENT,
        loop_edge: Color32::TRANSPARENT,
        sheen: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow_cool: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        bevel: Color32::from_rgba_unmultiplied_const(0xec, 0xeb, 0xe4, 18),
        bevel_shade: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 70),
        inner_shadow: Color32::from_rgba_unmultiplied_const(0x0b, 0x0b, 0x0b, 255),
        lip: Color32::from_rgba_unmultiplied_const(0x2e, 0x2e, 0x2d, 255),
        specular: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 16),
        drop: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 90),
        dark: true,
        radius: 3.0,
        round: 1.0,
        depth: 0.5,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
        system: System::GRAPHITE,
    }
}

/// Graphite and gold, Night.
pub fn graphite_night() -> Theme {
    family(
        "Graphite Night",
        graphite_night_tokens(),
        hex(0xe6b450),
        hex(0xf5d58e),
    )
}

/// Graphite and gold, Paper. Its tokens, as designed.
pub const fn graphite_paper_tokens() -> Tokens {
    Tokens {
        ink: hex(0xfcfbf9),
        well: hex(0xd9d6d0),
        panel: hex(0xeceae6),
        toolbar: hex(0xe4e2dd),
        panel_raised: hex(0xe4e2dd),
        panel_light: hex(0xeceae6),
        grid: hex(0xe3e1dc),
        grid_strong: hex(0xd2cfc8),
        grid_bar: hex(0xb4b0a6),
        edge: hex(0x67635c),
        text: hex(0x1d1c1a),
        text_dim: hex(0x4a4741),
        label: hex(0x4a4741),
        placeholder: hex(0x67635c),
        text_select: hex(0xd0ad55),
        brand: hex(0x7c5307),
        brand_bright: hex(0xa8740e),
        highlight: hex(0x7c5307),
        highlight_band: hex(0xe3ae45),
        highlight_glyph: hex(0xffffff),
        selection: hex(0x7c5307),
        focus: hex(0x7c5307),
        led: hex(0x136e5c),
        danger: hex(0xbf3220),
        fx_on: hex(0x136e5c),
        meter_well: hex(0xfcfbf9),
        meter_low: hex(0x2a9078),
        meter_high: hex(0x9c760c),
        meter_clip: hex(0xd23c2a),
        strip: hex(0xe4e2dd),
        strip_selected: hex(0xdcdad4),
        header_cell: hex(0xfaf9f7),
        cap: hex(0xfaf9f7),
        knob_body: hex(0xf5f4f1),
        knob_rim: hex(0x67635c),
        knob_track: hex(0x7a766f),
        knob_pointer: hex(0x1d1c1a),
        device_ground: hex(0xd2cfc8),
        deck: hex(0xe4e2dd),
        deck_title: hex(0xdcdad4),
        value_pill: hex(0xfcfbf9),
        value_pill_text: hex(0x1d1c1a),
        step_odd: hex(0xd9d6d0),
        step_even: hex(0xcfcbc4),
        step_odd_lit: hex(0x7c5307),
        step_even_lit: hex(0xa8740e),
        key_white: hex(0xffffff),
        key_black: hex(0x2b2a27),
        row_white: hex(0xeeede9),
        row_black: hex(0xf5f4f1),
        playhead: hex(0x7c5307),
        menu: hex(0xfaf9f7),
        menu_band: hex(0xeeede9),
        menu_hover: hex(0x7c5307),
        menu_text: hex(0x1d1c1a),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x2858a4),
        hue_tone: hex(0x10645a),
        hue_dynamics: hex(0x6a4604),
        hue_mod: hex(0x6a45bd),
        hue_space: hex(0xa33568),
        hue_output: hex(0x42641b),
        disabled: hex(0xa3a19c),    // r39:el-controls
        auto: hex(0x6a45bd),        // r39:el-controls
        modulated: hex(0xa33568),   // r39:el-controls
        destructive: hex(0xbf3220), // r39:el-controls
        primary_ink: hex(0xffffff), // r39:el-controls
        meter_mid: hex(0x638342),   // r39:el-controls
        fcap_hi: hex(0xffffff),     // r39:el-controls
        fcap_mid: hex(0xf4f3f1),    // r39:el-controls
        fcap_lo: hex(0xd8d6d1),     // r39:el-controls
        cap_hi: hex(0xffffff),      // r39:el-controls
        cap_mid: hex(0xf7f6f5),     // r39:el-controls
        cap_lo: hex(0xdcdad5),      // r39:el-controls
        scrim: Color32::from_rgba_unmultiplied_const(34, 46, 58, 87), // r39:el-controls
        shadow: Color32::from_rgba_unmultiplied_const(0x5a, 0x50, 0x40, 50),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::from_rgba_unmultiplied_const(0xf5, 0xf4, 0xf1, 150), // r39: the user's colour a tint at rest
        readout: Color32::TRANSPARENT, // r39:lapis2 — each readout in its own ink
        readout_soft: Color32::TRANSPARENT,
        selection_tint: Color32::TRANSPARENT,
        loop_edge: Color32::TRANSPARENT,
        sheen: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        glow_cool: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0),
        bevel: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 150),
        bevel_shade: Color32::from_rgba_unmultiplied_const(0x5a, 0x50, 0x40, 40),
        inner_shadow: Color32::from_rgba_unmultiplied_const(0xbd, 0xb9, 0xb0, 255),
        lip: Color32::from_rgba_unmultiplied_const(0xec, 0xea, 0xe5, 255),
        specular: Color32::from_rgba_unmultiplied_const(0xff, 0xff, 0xff, 60),
        drop: Color32::from_rgba_unmultiplied_const(0x4a, 0x42, 0x34, 40),
        dark: false,
        radius: 3.0,
        round: 1.0,
        depth: 0.5,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
        system: System::GRAPHITE,
    }
}

/// Graphite and gold, Paper.
pub fn graphite_paper() -> Theme {
    family(
        "Graphite Paper",
        graphite_paper_tokens(),
        hex(0x7c5307),
        hex(0xa8740e),
    )
}

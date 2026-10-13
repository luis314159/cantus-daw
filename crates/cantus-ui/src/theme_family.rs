//! Round 25's theme family (ADR 0046): **Cantus Night**, the default, and **Cantus
//! Paper**, its light twin, made from the website's palette as one design in two lights;
//! then six more for variety — **Daylight** and **Blue Hour** (white with blue details and
//! its dark twin, BACKLOG §6d.4), **Lagoon**, **Ultraviolet**, and **High Contrast** dark
//! and light for low vision.
//!
//! Each theme is written once, as the kit's tokens (every colour role, ADR 0024 §2); the
//! older palette that today's views still read is taken from those tokens plus a text-
//! legible accent, so the two can never disagree. Every pairing the kit promises holds
//! as written, without the contrast floor's help (`tests` below), and round 25's own
//! floor on top: meters on their well, the playhead on both rows, the glyph on a
//! highlighted key (`theme::tests::every_theme_meets_the_contrast_floor`).

use crate::kit::tokens::Tokens;
use crate::theme::{Palette, Theme};
use egui::Color32;

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The palette today's views read, from a family theme's tokens: the same surfaces, text,
/// keys and meters, with `accent` as the text-legible accent (4.5:1 on the panel and the
/// canvas) and `accent_bright` for the large readouts (3:1).
pub fn palette_of(t: &Tokens, accent: Color32, accent_bright: Color32) -> Palette {
    Palette {
        ink: t.ink,
        panel: t.panel,
        panel_raised: t.panel_raised,
        grid: t.grid,
        grid_strong: t.grid_strong,
        grid_bar: t.grid_bar,
        text: t.text,
        text_dim: t.text_dim,
        gold: accent,
        gold_bright: accent_bright,
        focus: t.focus,
        shadow: t.shadow,
        selection: t.selection,
        danger: t.danger,
        ok: t.led,
        black_key: t.key_black,
        white_key: t.key_white,
        row_black: t.row_black,
        row_white: t.row_white,
        meter_low: t.meter_low,
        meter_high: t.meter_high,
        meter_clip: t.meter_clip,
        corner_radius: t.radius as u8,
        dark: t.dark,
    }
}

fn family(name: &str, t: Tokens, accent: Color32, accent_bright: Color32) -> Theme {
    Theme::from_palette(name, &palette_of(&t, accent, accent_bright)).with_kit(&t)
}

/// **Cantus Night** — the default (ADR 0046): the website's night stage. Espresso
/// surfaces (`night` #120d0a … `night-3` #2a1d15), warm ivory text, the logo's copper
/// and amber as the accent, its verdigris patina as the selection and the meters' green.
///
/// Refined for long sessions (round 25, *night*; ADR 0046 §8): the surfaces lifted a
/// step off the near-black and a touch less saturated; primary text ≈ 87 % ivory
/// (12.7:1 on the panel rather than 15.5:1), secondary 6.6:1; the large track-colour
/// fills under an espresso veil (their bright hue stays on the colour cell); the fader
/// caps, white keys and amber keys a step softer; and the website's light — a warm sheen
/// on bars, a copper glow and a verdigris counter-glow in the big grounds.
pub const fn cantus_night_tokens() -> Tokens {
    Tokens {
        ink: hex(0x120d0a),
        well: hex(0x110c09),
        panel: hex(0x1f1712),
        toolbar: hex(0x251b15),
        panel_raised: hex(0x2b2019),
        panel_light: hex(0x33261e),
        grid: hex(0x2a1f18),
        grid_strong: hex(0x382a20),
        grid_bar: hex(0x4f3d30),
        edge: hex(0x786351),
        text: hex(0xe6d9ca),
        text_dim: hex(0xb09a86),
        label: hex(0xcbbaa8),
        placeholder: hex(0x6f5c4d),
        brand: hex(0xd4622a),
        brand_bright: hex(0xf5ad3d),
        highlight: hex(0xe09b48),
        highlight_band: hex(0xf3cd92),
        highlight_glyph: hex(0x3a1a06),
        selection: hex(0x58b8a3),
        focus: hex(0xf0cf98),
        led: hex(0x74c482),
        danger: hex(0xf0604c),
        fx_on: hex(0xf2b85a),
        meter_well: hex(0x0f0b09),
        meter_low: hex(0x4fbfa3),
        meter_high: hex(0xeaa543),
        meter_clip: hex(0xff5a3c),
        strip: hex(0x1b1411),
        strip_selected: hex(0x2a1f18),
        header_cell: hex(0x382a20),
        cap: hex(0xb9a997),
        knob_body: hex(0x2a1e17),
        knob_rim: hex(0x85705e),
        knob_track: hex(0x786351),
        knob_pointer: hex(0xe6d9ca),
        device_ground: hex(0x110c0a),
        deck: hex(0x1e1612),
        deck_title: hex(0x271c16),
        value_pill: hex(0x2e221a),
        value_pill_text: hex(0xe6d9ca),
        step_odd: hex(0x3b2c22),
        step_even: hex(0x4a3029),
        step_odd_lit: hex(0xf0cf98),
        step_even_lit: hex(0xe3a07a),
        key_white: hex(0xddd0c2),
        key_black: hex(0x1a1310),
        row_white: hex(0x1c1511),
        row_black: hex(0x17110d),
        playhead: hex(0xf5c860),
        menu: hex(0x2b2019),
        menu_band: hex(0x352820),
        menu_hover: hex(0xa94617),
        menu_text: hex(0xe6d9ca),
        menu_text_hover: hex(0xfff4ea),
        hue_source: hex(0x8fb8ff),
        hue_tone: hex(0x4dc7bd),
        hue_dynamics: hex(0xf0b45a),
        hue_mod: hex(0xc1acfc),
        hue_space: hex(0xf49aca),
        hue_output: hex(0xa8d27a),
        shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x99),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::from_rgba_unmultiplied_const(0x2b, 0x22, 0x1c, 110),
        sheen: Color32::from_rgba_unmultiplied_const(0xff, 0xd9, 0xb0, 18),
        glow: Color32::from_rgba_unmultiplied_const(0xd4, 0x62, 0x2a, 30),
        glow_cool: Color32::from_rgba_unmultiplied_const(0x3e, 0x8e, 0x7e, 20),
        dark: true,
        radius: 3.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// Cantus Night as a theme: its tokens, and the palette taken from them.
pub fn cantus_night() -> Theme {
    family(
        "Cantus Night",
        cantus_night_tokens(),
        hex(0xe8783e),
        hex(0xf5ad3d),
    )
}

/// **Cantus Paper** — Cantus Night's light twin: the website's cool stone sections
/// (`paper` #f3f5f4, `mist` #e5eae8), ink text, copper-deep for accents, verdigris-deep
/// for selection and focus; the meters stay dark inside, as on the night stage.
pub const fn cantus_paper_tokens() -> Tokens {
    Tokens {
        ink: hex(0xfbfcfc),
        well: hex(0xd6dedb),
        panel: hex(0xe5eae8),
        toolbar: hex(0xdce3e0),
        panel_raised: hex(0xf8faf9),
        panel_light: hex(0xeef2f0),
        grid: hex(0xdde4e1),
        grid_strong: hex(0xcbd5d2),
        grid_bar: hex(0xa3b0ac),
        edge: hex(0x7a8682),
        text: hex(0x14100d),
        text_dim: hex(0x4a4540),
        label: hex(0x3a342f),
        placeholder: hex(0x8f8a84),
        brand: hex(0xd4622a),
        brand_bright: hex(0xf5ad3d),
        highlight: hex(0xe07a3a),
        highlight_band: hex(0xf8c08a),
        highlight_glyph: hex(0x3a1404),
        selection: hex(0x1f6f62),
        focus: hex(0x1f5f54),
        led: hex(0x2a8f58),
        danger: hex(0xb8321f),
        fx_on: hex(0xf0a020),
        meter_well: hex(0x1d1410),
        meter_low: hex(0x4fbfa3),
        meter_high: hex(0xf5ad3d),
        meter_clip: hex(0xff6a3c),
        strip: hex(0xe9eeec),
        strip_selected: hex(0xdbe3e0),
        header_cell: hex(0xcbd5d2),
        cap: hex(0xf6f8f7),
        knob_body: hex(0x3b2a1f),
        knob_rim: hex(0x3b2a1f),
        knob_track: hex(0x7a8682),
        knob_pointer: hex(0xffffff),
        device_ground: hex(0xd6dedb),
        deck: hex(0xf6f8f7),
        deck_title: hex(0xe8edeb),
        value_pill: hex(0xe3e9e6),
        value_pill_text: hex(0x14100d),
        step_odd: hex(0xe1e7e4),
        step_even: hex(0xefdcd2),
        step_odd_lit: hex(0xf5c98a),
        step_even_lit: hex(0xe9a07a),
        key_white: hex(0xffffff),
        key_black: hex(0x2a1d15),
        row_white: hex(0xf6f8f7),
        row_black: hex(0xebf0ee),
        playhead: hex(0xc8561f),
        menu: hex(0xffffff),
        menu_band: hex(0xeef2f0),
        menu_hover: hex(0xa94617),
        menu_text: hex(0x14100d),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x2f62c8),
        hue_tone: hex(0x0d736d),
        hue_dynamics: hex(0x8a5800),
        hue_mod: hex(0x7a4cc2),
        hue_space: hex(0xa94275),
        hue_output: hex(0x4f6b2c),
        shadow: Color32::from_rgba_premultiplied(0x04, 0x04, 0x03, 0x38),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::TRANSPARENT,
        sheen: Color32::TRANSPARENT,
        glow: Color32::TRANSPARENT,
        glow_cool: Color32::TRANSPARENT,
        dark: false,
        radius: 3.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// Cantus Paper as a theme: its tokens, and the palette taken from them.
pub fn cantus_paper() -> Theme {
    family(
        "Cantus Paper",
        cantus_paper_tokens(),
        hex(0xa94617),
        hex(0xc8561f),
    )
}

/// **Daylight** — white with blue details (BACKLOG §6d.4, asked 2026-10-09): the old
/// light look sharpened, near-white surfaces, cool grey-blue headers, one saturated
/// blue for every active and selected state, dark-slate meters.
pub const fn daylight_tokens() -> Tokens {
    Tokens {
        ink: hex(0xffffff),
        well: hex(0xdfe5ec),
        panel: hex(0xeef1f5),
        toolbar: hex(0xe6eaf0),
        panel_raised: hex(0xfbfcfd),
        panel_light: hex(0xf3f5f8),
        grid: hex(0xe3e8ee),
        grid_strong: hex(0xcfd7e1),
        grid_bar: hex(0xa7b3c2),
        edge: hex(0x7a8696),
        text: hex(0x121a24),
        text_dim: hex(0x4b5869),
        label: hex(0x344152),
        placeholder: hex(0x8a96a5),
        brand: hex(0x1f6fe5),
        brand_bright: hex(0x4b8dff),
        highlight: hex(0x1f6fe5),
        highlight_band: hex(0x9cc3ff),
        highlight_glyph: hex(0xffffff),
        selection: hex(0x1f6fe5),
        focus: hex(0x0b5bd3),
        led: hex(0x1e9450),
        danger: hex(0xcc2f27),
        fx_on: hex(0xf2b01e),
        meter_well: hex(0x26303b),
        meter_low: hex(0x5fd38a),
        meter_high: hex(0xffd34d),
        meter_clip: hex(0xff6b4a),
        strip: hex(0xf2f5f8),
        strip_selected: hex(0xe1eaf6),
        header_cell: hex(0xd5dde7),
        cap: hex(0xf7f9fb),
        knob_body: hex(0x2b3542),
        knob_rim: hex(0x2b3542),
        knob_track: hex(0x7f8c9c),
        knob_pointer: hex(0xffffff),
        device_ground: hex(0xe1e6ec),
        deck: hex(0xfafbfc),
        deck_title: hex(0xedf1f6),
        value_pill: hex(0xe6ebf1),
        value_pill_text: hex(0x121a24),
        step_odd: hex(0xe4e9ef),
        step_even: hex(0xecdfe3),
        step_odd_lit: hex(0x8fb8f5),
        step_even_lit: hex(0xf2a6b5),
        key_white: hex(0xffffff),
        key_black: hex(0x2b3038),
        row_white: hex(0xf7f9fb),
        row_black: hex(0xedf1f5),
        playhead: hex(0x1f6fe5),
        menu: hex(0xffffff),
        menu_band: hex(0xeef2f7),
        menu_hover: hex(0x1f6fe5),
        menu_text: hex(0x121a24),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x2559c4),
        hue_tone: hex(0x0c7068),
        hue_dynamics: hex(0x865700),
        hue_mod: hex(0x7347bf),
        hue_space: hex(0xa63e70),
        hue_output: hex(0x4c682a),
        shadow: Color32::from_rgba_premultiplied(0x03, 0x05, 0x08, 0x33),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::TRANSPARENT,
        sheen: Color32::TRANSPARENT,
        glow: Color32::TRANSPARENT,
        glow_cool: Color32::TRANSPARENT,
        dark: false,
        radius: 3.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// Daylight as a theme: its tokens, and the palette taken from them.
pub fn daylight() -> Theme {
    family("Daylight", daylight_tokens(), hex(0x1a5fd0), hex(0x2f7cf6))
}

/// **Blue Hour** — Daylight's dark twin: navy slate, the same blue family lit for a
/// dark ground.
pub const fn blue_hour_tokens() -> Tokens {
    Tokens {
        ink: hex(0x0d1117),
        well: hex(0x0a0e13),
        panel: hex(0x151b23),
        toolbar: hex(0x19202a),
        panel_raised: hex(0x1c2430),
        panel_light: hex(0x232d3a),
        grid: hex(0x1c2430),
        grid_strong: hex(0x263142),
        grid_bar: hex(0x364459),
        edge: hex(0x64748b),
        text: hex(0xe8eef6),
        text_dim: hex(0x9aa9bc),
        label: hex(0xc3cedc),
        placeholder: hex(0x5c6a7c),
        brand: hex(0x4c8dff),
        brand_bright: hex(0x8ec0ff),
        highlight: hex(0x5b9bff),
        highlight_band: hex(0xb5d1ff),
        highlight_glyph: hex(0x061a3d),
        selection: hex(0x6aa6ff),
        focus: hex(0xa8cdff),
        led: hex(0x3fd17a),
        danger: hex(0xff6464),
        fx_on: hex(0xffcf4a),
        meter_well: hex(0x080b0f),
        meter_low: hex(0x4fd18b),
        meter_high: hex(0xffd34d),
        meter_clip: hex(0xff6b4a),
        strip: hex(0x121820),
        strip_selected: hex(0x1b2635),
        header_cell: hex(0x2b3747),
        cap: hex(0xc7d2e0),
        knob_body: hex(0x222c39),
        knob_rim: hex(0x6b7c93),
        knob_track: hex(0x64748b),
        knob_pointer: hex(0xe8eef6),
        device_ground: hex(0x0a0d12),
        deck: hex(0x141a22),
        deck_title: hex(0x1b2330),
        value_pill: hex(0x1f2935),
        value_pill_text: hex(0xe8eef6),
        step_odd: hex(0x263142),
        step_even: hex(0x352a33),
        step_odd_lit: hex(0xb6d0f7),
        step_even_lit: hex(0xf2b9c4),
        key_white: hex(0xd9e1ec),
        key_black: hex(0x10151c),
        row_white: hex(0x161d26),
        row_black: hex(0x11171f),
        playhead: hex(0x6aa6ff),
        menu: hex(0x1c2430),
        menu_band: hex(0x263142),
        menu_hover: hex(0x2a63cc),
        menu_text: hex(0xe8eef6),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x8fb8ff),
        hue_tone: hex(0x4dc7bd),
        hue_dynamics: hex(0xe8b25a),
        hue_mod: hex(0xb9a4fa),
        hue_space: hex(0xf093c4),
        hue_output: hex(0x9ccf6a),
        shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x99),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::TRANSPARENT,
        sheen: Color32::TRANSPARENT,
        glow: Color32::TRANSPARENT,
        glow_cool: Color32::TRANSPARENT,
        dark: true,
        radius: 3.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// Blue Hour as a theme: its tokens, and the palette taken from them.
pub fn blue_hour() -> Theme {
    family(
        "Blue Hour",
        blue_hour_tokens(),
        hex(0x6aa6ff),
        hex(0x8ec0ff),
    )
}

/// **Ultraviolet** — deep violet with neon: magenta keys, cyan selection and playhead.
pub const fn ultraviolet_tokens() -> Tokens {
    Tokens {
        ink: hex(0x0f0a1a),
        well: hex(0x0b0714),
        panel: hex(0x181126),
        toolbar: hex(0x1d152e),
        panel_raised: hex(0x231a36),
        panel_light: hex(0x2c2142),
        grid: hex(0x221936),
        grid_strong: hex(0x2e2347),
        grid_bar: hex(0x42335f),
        edge: hex(0x76669b),
        text: hex(0xf1ecff),
        text_dim: hex(0xab9fcc),
        label: hex(0xcfc5ea),
        placeholder: hex(0x625584),
        brand: hex(0xb26bff),
        brand_bright: hex(0xe0a8ff),
        highlight: hex(0xff4fd8),
        highlight_band: hex(0xffb3f0),
        highlight_glyph: hex(0x2b0026),
        selection: hex(0x33e6ff),
        focus: hex(0x7cf3ff),
        led: hex(0x59ff9b),
        danger: hex(0xff5a76),
        fx_on: hex(0xffe14f),
        meter_well: hex(0x08050f),
        meter_low: hex(0x3ff2b0),
        meter_high: hex(0xffd84f),
        meter_clip: hex(0xff4f6d),
        strip: hex(0x140e21),
        strip_selected: hex(0x241a3a),
        header_cell: hex(0x362a50),
        cap: hex(0xd7cdef),
        knob_body: hex(0x241a38),
        knob_rim: hex(0x7d6aa8),
        knob_track: hex(0x76669b),
        knob_pointer: hex(0xf1ecff),
        device_ground: hex(0x0b0714),
        deck: hex(0x171024),
        deck_title: hex(0x201733),
        value_pill: hex(0x261c3a),
        value_pill_text: hex(0xf1ecff),
        step_odd: hex(0x2e2347),
        step_even: hex(0x3d2346),
        step_odd_lit: hex(0xb9f6ff),
        step_even_lit: hex(0xffc2f1),
        key_white: hex(0xe6def7),
        key_black: hex(0x120c1e),
        row_white: hex(0x1a1229),
        row_black: hex(0x150f22),
        playhead: hex(0x33e6ff),
        menu: hex(0x231a36),
        menu_band: hex(0x2e2347),
        menu_hover: hex(0x6f35c8),
        menu_text: hex(0xf1ecff),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x8fb0ff),
        hue_tone: hex(0x45dccb),
        hue_dynamics: hex(0xffc35c),
        hue_mod: hex(0xd29bff),
        hue_space: hex(0xff8fd6),
        hue_output: hex(0xa6e36f),
        shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0xa0),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::TRANSPARENT,
        sheen: Color32::TRANSPARENT,
        glow: Color32::TRANSPARENT,
        glow_cool: Color32::TRANSPARENT,
        dark: true,
        radius: 4.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// Ultraviolet as a theme: its tokens, and the palette taken from them.
pub fn ultraviolet() -> Theme {
    family(
        "Ultraviolet",
        ultraviolet_tokens(),
        hex(0xc48bff),
        hex(0xe0a8ff),
    )
}

/// **Lagoon** — cool teal slate with an amber selection, its complement.
pub const fn lagoon_tokens() -> Tokens {
    Tokens {
        ink: hex(0x0c1517),
        well: hex(0x091113),
        panel: hex(0x142124),
        toolbar: hex(0x182729),
        panel_raised: hex(0x1c2d30),
        panel_light: hex(0x23373a),
        grid: hex(0x1b2b2e),
        grid_strong: hex(0x25393c),
        grid_bar: hex(0x355054),
        edge: hex(0x5f7e82),
        text: hex(0xe6f2f0),
        text_dim: hex(0x93b0ad),
        label: hex(0xbfd6d3),
        placeholder: hex(0x56706d),
        brand: hex(0x3ec9b0),
        brand_bright: hex(0x8af0dc),
        highlight: hex(0x2fd3b5),
        highlight_band: hex(0xa8f5e6),
        highlight_glyph: hex(0x032a24),
        selection: hex(0xffb454),
        focus: hex(0xffd28a),
        led: hex(0x5ce08a),
        danger: hex(0xff6b5b),
        fx_on: hex(0xffd24f),
        meter_well: hex(0x060d0e),
        meter_low: hex(0x3fd6a8),
        meter_high: hex(0xffd24f),
        meter_clip: hex(0xff6b4a),
        strip: hex(0x111d20),
        strip_selected: hex(0x1b3034),
        header_cell: hex(0x2b4245),
        cap: hex(0xc5d8d6),
        knob_body: hex(0x1b2a2d),
        knob_rim: hex(0x648a8e),
        knob_track: hex(0x5f7e82),
        knob_pointer: hex(0xe6f2f0),
        device_ground: hex(0x091113),
        deck: hex(0x132124),
        deck_title: hex(0x1a2b2e),
        value_pill: hex(0x1d3033),
        value_pill_text: hex(0xe6f2f0),
        step_odd: hex(0x25393c),
        step_even: hex(0x3a3436),
        step_odd_lit: hex(0xb4ece2),
        step_even_lit: hex(0xffd0b0),
        key_white: hex(0xd6e6e4),
        key_black: hex(0x0e1719),
        row_white: hex(0x152326),
        row_black: hex(0x111d20),
        playhead: hex(0xffb454),
        menu: hex(0x1c2d30),
        menu_band: hex(0x25393c),
        menu_hover: hex(0x17705f),
        menu_text: hex(0xe6f2f0),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x8fb8ff),
        hue_tone: hex(0x4dd6c4),
        hue_dynamics: hex(0xf0b45a),
        hue_mod: hex(0xc1acfc),
        hue_space: hex(0xf49aca),
        hue_output: hex(0xa8d27a),
        shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x99),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::TRANSPARENT,
        sheen: Color32::TRANSPARENT,
        glow: Color32::TRANSPARENT,
        glow_cool: Color32::TRANSPARENT,
        dark: true,
        radius: 3.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// Lagoon as a theme: its tokens, and the palette taken from them.
pub fn lagoon() -> Theme {
    family("Lagoon", lagoon_tokens(), hex(0x3ec9b0), hex(0x8af0dc))
}

/// **High Contrast** — for low vision: black, white text (13:1 at the least), yellow
/// keys and focus, cyan selection, bright edges round every control.
pub const fn high_contrast_tokens() -> Tokens {
    Tokens {
        ink: hex(0x000000),
        well: hex(0x000000),
        panel: hex(0x121212),
        toolbar: hex(0x161616),
        panel_raised: hex(0x1e1e1e),
        panel_light: hex(0x262626),
        grid: hex(0x333333),
        grid_strong: hex(0x555555),
        grid_bar: hex(0x8a8a8a),
        edge: hex(0xc8c8c8),
        text: hex(0xffffff),
        text_dim: hex(0xe2e2e2),
        label: hex(0xf0f0f0),
        placeholder: hex(0xa0a0a0),
        brand: hex(0xffd400),
        brand_bright: hex(0xffe866),
        highlight: hex(0xffd400),
        highlight_band: hex(0xfff2a0),
        highlight_glyph: hex(0x000000),
        selection: hex(0x00e5ff),
        focus: hex(0xffff00),
        led: hex(0x3dff6e),
        danger: hex(0xff6060),
        fx_on: hex(0xffd400),
        meter_well: hex(0x000000),
        meter_low: hex(0x3dff6e),
        meter_high: hex(0xffd400),
        meter_clip: hex(0xff3b3b),
        strip: hex(0x0d0d0d),
        strip_selected: hex(0x1f2a2c),
        header_cell: hex(0x303030),
        cap: hex(0xffffff),
        knob_body: hex(0x000000),
        knob_rim: hex(0xffffff),
        knob_track: hex(0xa0a0a0),
        knob_pointer: hex(0xffffff),
        device_ground: hex(0x000000),
        deck: hex(0x0f0f0f),
        deck_title: hex(0x1a1a1a),
        value_pill: hex(0x1a1a1a),
        value_pill_text: hex(0xffffff),
        step_odd: hex(0x3a3a3a),
        step_even: hex(0x4a3434),
        step_odd_lit: hex(0xffffff),
        step_even_lit: hex(0xffd400),
        key_white: hex(0xffffff),
        key_black: hex(0x000000),
        row_white: hex(0x0d0d0d),
        row_black: hex(0x000000),
        playhead: hex(0xffff00),
        menu: hex(0x000000),
        menu_band: hex(0x1a1a1a),
        menu_hover: hex(0xffd400),
        menu_text: hex(0xffffff),
        menu_text_hover: hex(0x000000),
        hue_source: hex(0x9cc4ff),
        hue_tone: hex(0x4df0e0),
        hue_dynamics: hex(0xffc94d),
        hue_mod: hex(0xd6b8ff),
        hue_space: hex(0xff9ed6),
        hue_output: hex(0xb6f07a),
        shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0xc0),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::TRANSPARENT,
        sheen: Color32::TRANSPARENT,
        glow: Color32::TRANSPARENT,
        glow_cool: Color32::TRANSPARENT,
        dark: true,
        radius: 2.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// High Contrast as a theme: its tokens, and the palette taken from them.
pub fn high_contrast() -> Theme {
    family(
        "High Contrast",
        high_contrast_tokens(),
        hex(0xffd400),
        hex(0xffe866),
    )
}

/// **High Contrast Light** — white, black text (9:1 at the least), deep blue keys
/// and selection, a magenta focus ring that no other role uses.
pub const fn high_contrast_light_tokens() -> Tokens {
    Tokens {
        ink: hex(0xffffff),
        well: hex(0xffffff),
        panel: hex(0xf0f0f0),
        toolbar: hex(0xe8e8e8),
        panel_raised: hex(0xffffff),
        panel_light: hex(0xf7f7f7),
        grid: hex(0xcfcfcf),
        grid_strong: hex(0x9e9e9e),
        grid_bar: hex(0x5c5c5c),
        edge: hex(0x404040),
        text: hex(0x000000),
        text_dim: hex(0x262626),
        label: hex(0x141414),
        placeholder: hex(0x6e6e6e),
        brand: hex(0x0033cc),
        brand_bright: hex(0x1a4fe6),
        highlight: hex(0x0033cc),
        highlight_band: hex(0x9db0ff),
        highlight_glyph: hex(0xffffff),
        selection: hex(0x0033cc),
        focus: hex(0xb0006b),
        led: hex(0x007a1f),
        danger: hex(0xc00000),
        fx_on: hex(0xa35f00),
        meter_well: hex(0x000000),
        meter_low: hex(0x33e070),
        meter_high: hex(0xffd400),
        meter_clip: hex(0xff3b3b),
        strip: hex(0xf7f7f7),
        strip_selected: hex(0xdfe6ff),
        header_cell: hex(0xd6d6d6),
        cap: hex(0xffffff),
        knob_body: hex(0x000000),
        knob_rim: hex(0x000000),
        knob_track: hex(0x595959),
        knob_pointer: hex(0xffffff),
        device_ground: hex(0xe3e3e3),
        deck: hex(0xffffff),
        deck_title: hex(0xebebeb),
        value_pill: hex(0xe6e6e6),
        value_pill_text: hex(0x000000),
        step_odd: hex(0xdedede),
        step_even: hex(0xf0d6d6),
        step_odd_lit: hex(0x5577ff),
        step_even_lit: hex(0xff5599),
        key_white: hex(0xffffff),
        key_black: hex(0x000000),
        row_white: hex(0xffffff),
        row_black: hex(0xefefef),
        playhead: hex(0xb0006b),
        menu: hex(0xffffff),
        menu_band: hex(0xebebeb),
        menu_hover: hex(0x0033cc),
        menu_text: hex(0x000000),
        menu_text_hover: hex(0xffffff),
        hue_source: hex(0x0039b3),
        hue_tone: hex(0x005c56),
        hue_dynamics: hex(0x6b4300),
        hue_mod: hex(0x5a2aa8),
        hue_space: hex(0x8a1f55),
        hue_output: hex(0x375010),
        shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x40),
        light: hex(0xffffff),
        dark_shade: hex(0x000000),
        track_veil: Color32::TRANSPARENT,
        sheen: Color32::TRANSPARENT,
        glow: Color32::TRANSPARENT,
        glow_cool: Color32::TRANSPARENT,
        dark: false,
        radius: 2.0,
        round: 1.0,
        depth: 1.0,
        density: 1.0,
        text_scale: 1.0,
        generation: 0,
    }
}

/// High Contrast Light as a theme: its tokens, and the palette taken from them.
pub fn high_contrast_light() -> Theme {
    family(
        "High Contrast Light",
        high_contrast_light_tokens(),
        hex(0x0033cc),
        hex(0x1a4fe6),
    )
}

/// The family, by the id an appearance file names each with (`preset:<id>`), in the
/// order the Appearance page lists them.
pub const FAMILY: [crate::theme::Preset; 8] = [
    ("cantus_night", cantus_night),
    ("cantus_paper", cantus_paper),
    ("daylight", daylight),
    ("blue_hour", blue_hour),
    ("ultraviolet", ultraviolet),
    ("lagoon", lagoon),
    ("contrast", high_contrast),
    ("contrast_light", high_contrast_light),
];

/// A family theme's id and its tokens.
pub type TokensEntry = (&'static str, fn() -> Tokens);

/// Every family theme's tokens, by id (for tests and the previews).
pub const FAMILY_TOKENS: [TokensEntry; 8] = [
    ("cantus_night", cantus_night_tokens),
    ("cantus_paper", cantus_paper_tokens),
    ("daylight", daylight_tokens),
    ("blue_hour", blue_hour_tokens),
    ("ultraviolet", ultraviolet_tokens),
    ("lagoon", lagoon_tokens),
    ("contrast", high_contrast_tokens),
    ("contrast_light", high_contrast_light_tokens),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_theme_keeps_every_promise_as_written() {
        // The raw tokens, before any floor: what the file says is what is shown.
        for (id, make) in FAMILY_TOKENS {
            let bad = make().legibility_failures();
            assert!(bad.is_empty(), "{id}: {bad:#?}");
        }
        // And the theme resolves to exactly those tokens.
        for ((id, theme), (_, tokens)) in FAMILY.iter().zip(FAMILY_TOKENS) {
            let mut want = tokens();
            let got = theme().kit_tokens();
            want.generation = got.generation;
            assert_eq!(got, want, "{id}");
        }
    }
}

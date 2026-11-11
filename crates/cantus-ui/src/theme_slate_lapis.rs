//! **Slate Lapis** (round 39, an optional theme of the Slate family, ADR 0111 §2) — made
//! to go with the app's logo #03, *the metal* (a lapis-blue tile with a gold C). It is
//! Slate's system — the same structure ([`System::SLATE`]), text, faces, keys, meters,
//! hues and corners, Night and Paper — with three things of its own, so it is told from
//! Slate at a glance (round 39, `lapis2`, approved by the owner):
//!
//! - **Lapis on the selection.** The accent is lapis — `#7ea2f6` at night, `#2443a8` on
//!   paper (Slate's azure is `#5cafe9` / `#1f6ec2`), its tones derived as Slate derives
//!   its own (the bright tone and lit step 70 % white→accent at night, the band 58.7 %,
//!   the ink on it the accent at 14.3 % of black, the menu's hover 30 % toward it, the
//!   text selection the accent at 35 % of black; on paper the selection and band half
//!   way, the bright tone 84 %, the lit step 78 %, white on it). And the
//!   **selection tint** (`accent.selection_tint`) lays the lapis over every selected
//!   fill — a clip's body, a note, a mixer strip, the selected track's header and lane,
//!   a list's row — and marks the active tab, where Slate keeps its neutral and
//!   track-coloured selections.
//! - **Gold on the readouts and marks**, from the logo's metal: the top bar's position,
//!   time and tempo and the rulers' bar numbers (`text.readout`, `#e2b85a` at night,
//!   `#6e5008` on paper — 4.5:1 and more on the bar, its wells and the panel), the
//!   playhead (`#e2b85a` / `#9a6e1c`), the loop region's ends and its edges along its
//!   whole length (`editor.loop_edge`). No body text, label or button is gold.
//! - **A night-blue room.** Every surface, well and line of Slate's ladder — the window's
//!   ground, the panels, the bars, the wells, the desk, the lanes, the menus, the grid —
//!   is Slate's own colour at **the same luminance**, its hue turned to the lapis's
//!   (222° at night, 226° on paper) and its saturation × 1.7 (night) / × 1.8 (paper):
//!   deep night blue in Night, a cool blue-grey paper in Paper. The same luminance keeps
//!   every contrast Slate holds; the text, the keys, the knob and fader caps (the metal),
//!   the meters, the cables' and the status colours are Slate's.
//!
//! Slate stays the default and does not change ([`crate::theme::default_theme`]).

use crate::kit::system::System;
use crate::kit::tokens::Tokens;
use crate::theme::Theme;
use crate::theme_family::palette_of;
use crate::theme_slate::{slate_night_tokens, slate_paper_tokens};
use egui::Color32;

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

const fn alpha(v: u32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const((v >> 16) as u8, (v >> 8) as u8, v as u8, a)
}

/// The lapis accent at night, and its bright tone.
const NIGHT_LAPIS: Color32 = hex(0x7ea2f6);
const NIGHT_LAPIS_BRIGHT: Color32 = hex(0xa5bef9);
/// The logo's metal at night: the playhead, the loop's edges and the readouts.
const NIGHT_GOLD: Color32 = hex(0xe2b85a);
/// The readouts' second gold at night (6:1 on the bar).
const NIGHT_GOLD_SOFT: Color32 = hex(0xc9a55a);
/// The lapis accent on paper, and its bright tone.
const PAPER_LAPIS: Color32 = hex(0x2443a8);
const PAPER_LAPIS_BRIGHT: Color32 = hex(0x4761b6);
/// The logo's metal on paper: the playhead and the loop's edges.
const PAPER_GOLD: Color32 = hex(0x9a6e1c);
/// The metal as text on paper, deepened to read on the bar and its wells: the readouts.
const PAPER_GOLD_INK: Color32 = hex(0x6e5008);
const PAPER_GOLD_INK_SOFT: Color32 = hex(0x7a5a04);
/// The hue the room turns to (the lapis's own), and how much more colour it takes.
pub const NIGHT_HUE: f32 = 222.0;
pub const PAPER_HUE: f32 = 226.0;
pub const NIGHT_GAIN: f32 = 1.7;
pub const PAPER_GAIN: f32 = 1.8;

/// A theme of the family: its tokens (and so its system), the palette taken from them.
fn family(name: &str, t: Tokens, accent: Color32, accent_bright: Color32) -> Theme {
    let mut theme = Theme::from_palette(name, &palette_of(&t, accent, accent_bright)).with_kit(&t);
    theme.system = t.system;
    theme
}

/// The selection tint's strength on a large fill (its alpha), by light.
const NIGHT_TINT: u8 = 46;
const PAPER_TINT: u8 = 26;

/// Slate Lapis, Night: Slate Night in a night-blue room, lapis on the selection, gold on the readouts.
pub const fn slate_lapis_night_tokens() -> Tokens {
    Tokens {
        // The room: Slate's ladder at its own luminance, turned to the lapis.
        ink: hex(0x131826),
        well: hex(0x181e2d),
        panel: hex(0x1c2232),
        toolbar: hex(0x242c3f),
        panel_raised: hex(0x242c3f),
        panel_light: hex(0x1c2232),
        grid: hex(0x202738),
        grid_strong: hex(0x242d40),
        grid_bar: hex(0x303a51),
        edge: hex(0x7985a0),
        placeholder: hex(0x7985a0),
        meter_well: hex(0x0f1520),
        strip: hex(0x242c3f),
        strip_selected: hex(0x2d364a),
        header_cell: hex(0x2d364a),
        device_ground: hex(0x131826),
        deck: hex(0x242c3f),
        deck_title: hex(0x293246),
        value_pill: hex(0x181e2d),
        step_odd: hex(0x2d364a),
        step_even: hex(0x364057),
        row_white: hex(0x1f2637),
        row_black: hex(0x1c2232),
        menu: hex(0x2d364a),
        menu_band: hex(0x272f43),
        disabled: hex(0x455069),
        inner_shadow: hex(0x0f1520),
        lip: hex(0x2c3549),
        track_veil: alpha(0x1c2232, 150),
        // The accent, lapis, and its tones as Slate derives its own.
        text_select: hex(0x2c3956),
        brand: NIGHT_LAPIS,
        brand_bright: NIGHT_LAPIS_BRIGHT,
        highlight: NIGHT_LAPIS,
        highlight_band: hex(0xb3c8fa),
        highlight_glyph: hex(0x121723),
        selection: NIGHT_LAPIS,
        focus: NIGHT_LAPIS,
        step_odd_lit: NIGHT_LAPIS,
        step_even_lit: NIGHT_LAPIS_BRIGHT,
        menu_hover: hex(0x475776),
        primary_ink: hex(0x121723),
        // The lapis over every selected fill.
        selection_tint: alpha(0x7ea2f6, NIGHT_TINT),
        // The metal: the readouts, the playhead, the loop's edges.
        readout: NIGHT_GOLD,
        readout_soft: NIGHT_GOLD_SOFT,
        playhead: NIGHT_GOLD,
        loop_edge: NIGHT_GOLD,
        system: System::SLATE,
        ..slate_night_tokens()
    }
}

/// Slate Lapis, Night.
pub fn slate_lapis_night() -> Theme {
    family(
        "Slate Lapis Night",
        slate_lapis_night_tokens(),
        NIGHT_LAPIS,
        NIGHT_LAPIS_BRIGHT,
    )
}

/// Slate Lapis, Paper: Slate Paper on a cool blue-grey paper, lapis on the selection, gold on the readouts.
pub const fn slate_lapis_paper_tokens() -> Tokens {
    Tokens {
        // The room: Slate's ladder at its own luminance, turned to the lapis.
        ink: hex(0xfbfcfe),
        well: hex(0xd7dceb),
        panel: hex(0xf4f6fa),
        toolbar: hex(0xe5e8f4),
        panel_raised: hex(0xe5e8f4),
        panel_light: hex(0xf4f6fa),
        grid: hex(0xe8ebf5),
        grid_strong: hex(0xdadeed),
        grid_bar: hex(0xc9cfe2),
        edge: hex(0x68749a),
        placeholder: hex(0x68749a),
        meter_well: hex(0xfbfcfe),
        strip: hex(0xe5e8f4),
        strip_selected: hex(0xdadfef),
        header_cell: hex(0xfbfcfe),
        device_ground: hex(0xc6ccdf),
        deck: hex(0xe5e8f4),
        deck_title: hex(0xe0e4f1),
        value_pill: hex(0xfbfcfe),
        step_odd: hex(0xd7dceb),
        step_even: hex(0xcdd3e5),
        row_white: hex(0xeef1f8),
        row_black: hex(0xf4f6fa),
        menu: hex(0xfbfcfe),
        menu_band: hex(0xeef1f8),
        disabled: hex(0xaab1ca),
        inner_shadow: hex(0xbac1d9),
        lip: hex(0xeff1f9),
        track_veil: alpha(0xf4f6fa, 150),
        // The accent, lapis, and its tones as Slate derives its own.
        text_select: hex(0x90a0d3),
        brand: PAPER_LAPIS,
        brand_bright: PAPER_LAPIS_BRIGHT,
        highlight: PAPER_LAPIS,
        highlight_band: hex(0x92a1d4),
        highlight_glyph: hex(0xffffff),
        selection: PAPER_LAPIS,
        focus: PAPER_LAPIS,
        step_odd_lit: PAPER_LAPIS,
        step_even_lit: hex(0x546cbb),
        menu_hover: PAPER_LAPIS,
        primary_ink: hex(0xffffff),
        // The lapis over every selected fill.
        selection_tint: alpha(0x2443a8, PAPER_TINT),
        // The metal: the readouts, the playhead, the loop's edges.
        readout: PAPER_GOLD_INK,
        readout_soft: PAPER_GOLD_INK_SOFT,
        playhead: PAPER_GOLD,
        loop_edge: PAPER_GOLD,
        system: System::SLATE,
        ..slate_paper_tokens()
    }
}

/// Slate Lapis, Paper.
pub fn slate_lapis_paper() -> Theme {
    family(
        "Slate Lapis Paper",
        slate_lapis_paper_tokens(),
        PAPER_LAPIS,
        PAPER_LAPIS_BRIGHT,
    )
}

/// The gold of the playhead and the loop's edges in a Slate Lapis theme, by its light.
pub const fn gold(dark: bool) -> Color32 {
    if dark {
        NIGHT_GOLD
    } else {
        PAPER_GOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::color::{contrast, luminance, Hsl};
    use crate::kit::tokens::Tok;

    /// The room: the ladder's surfaces, wells and lines that turn to the lapis.
    const ROOM: [&str; 28] = [
        "ink",
        "well",
        "panel",
        "toolbar",
        "panel_raised",
        "panel_light",
        "grid",
        "grid_strong",
        "grid_bar",
        "edge",
        "placeholder",
        "meter_well",
        "strip",
        "strip_selected",
        "header_cell",
        "device_ground",
        "deck",
        "deck_title",
        "value_pill",
        "step_odd",
        "step_even",
        "row_white",
        "row_black",
        "menu",
        "menu_band",
        "disabled",
        "inner_shadow",
        "lip",
    ];

    /// The accent's tones, the selection tint, and the metal.
    const OWN: [&str; 18] = [
        "text_select",
        "brand",
        "brand_bright",
        "highlight",
        "highlight_band",
        "highlight_glyph",
        "selection",
        "focus",
        "step_odd_lit",
        "step_even_lit",
        "menu_hover",
        "primary_ink",
        "selection_tint",
        "readout",
        "readout_soft",
        "playhead",
        "loop_edge",
        "track_veil",
    ];

    /// Which tokens differ between two sets, by their field.
    fn differ(a: &Tokens, b: &Tokens) -> Vec<&'static str> {
        Tok::ALL
            .iter()
            .filter(|tok| a.get(**tok) != b.get(**tok))
            .map(|tok| tok.field())
            .collect()
    }

    fn field(t: &Tokens, name: &str) -> Color32 {
        let tok = Tok::ALL.iter().find(|k| k.field() == name).expect(name);
        t.get(*tok)
    }

    fn pairs() -> [(Tokens, Tokens); 2] {
        [
            (slate_lapis_night_tokens(), slate_night_tokens()),
            (slate_lapis_paper_tokens(), slate_paper_tokens()),
        ]
    }

    #[test]
    fn lapis_is_slate_but_for_its_room_its_selection_and_its_metal() {
        // Slate's text, keys, caps, meters, hues, status colours, corners and face.
        for (lapis, slate) in pairs() {
            for key in differ(&lapis, &slate) {
                assert!(
                    ROOM.contains(&key) || OWN.contains(&key),
                    "{key} differs from Slate"
                );
            }
            let always = [
                "brand",
                "highlight",
                "selection",
                "selection_tint",
                "readout",
                "readout_soft",
                "playhead",
                "loop_edge",
            ];
            for key in ROOM.iter().chain(always.iter()) {
                assert_ne!(field(&lapis, key), field(&slate, key), "{key} is Slate's");
            }
            assert_eq!(lapis.system, System::SLATE);
            assert_eq!(lapis.dark, slate.dark);
            assert_eq!(
                (lapis.radius, lapis.depth, lapis.density),
                (slate.radius, slate.depth, slate.density)
            );
            assert_eq!(lapis.primary_ink, lapis.highlight_glyph);
            assert_eq!(lapis.playhead, gold(lapis.dark));
            assert_eq!(lapis.loop_edge, gold(lapis.dark));
            // Slate itself has none of the three: its selections, readouts and loop are
            // its views' own.
            for c in [
                slate.selection_tint,
                slate.readout,
                slate.readout_soft,
                slate.loop_edge,
            ] {
                assert_eq!(c, Color32::TRANSPARENT);
            }
        }
        assert_eq!(slate_lapis_night_tokens().highlight, hex(0x7ea2f6));
        assert_eq!(slate_lapis_paper_tokens().highlight, hex(0x2443a8));
    }

    #[test]
    fn the_room_is_slates_own_turned_to_lapis_at_the_same_luminance() {
        for ((lapis, slate), (hue, gain)) in pairs()
            .into_iter()
            .zip([(NIGHT_HUE, NIGHT_GAIN), (PAPER_HUE, PAPER_GAIN)])
        {
            for key in ROOM {
                let (l, s) = (field(&lapis, key), field(&slate, key));
                let (ll, ls) = (luminance(l), luminance(s));
                // The same luminance, to the rounding of 8-bit channels.
                assert!(
                    (ll - ls).abs() <= 0.006 + 0.01 * ls,
                    "{key}: luminance {ls} became {ll}"
                );
                let (hl, hs) = (Hsl::of(l), Hsl::of(s));
                let off = ((hl.h - hue + 540.0) % 360.0 - 180.0).abs();
                assert!(off <= 8.0, "{key}: hue {} is not the lapis's {hue}", hl.h);
                // More colour, by about the gain (rounding moves a pale one most).
                assert!(
                    hl.s >= hs.s * (gain - 0.4),
                    "{key}: saturation {} for Slate's {} × {gain}",
                    hl.s,
                    hs.s
                );
            }
            // So every contrast Slate holds between its own surfaces and text, the room
            // holds within a few per cent.
            for key in ROOM {
                let (l, s) = (field(&lapis, key), field(&slate, key));
                let (cl, cs) = (contrast(lapis.text, l), contrast(slate.text, s));
                assert!((cl - cs).abs() <= 0.05 * cs, "{key}: text {cs} became {cl}");
            }
        }
    }

    #[test]
    fn the_readouts_are_gold_and_read_on_the_bar_its_wells_and_the_panel() {
        for (lapis, _) in pairs() {
            for c in [lapis.readout, lapis.readout_soft] {
                let h = Hsl::of(c);
                assert!((35.0..=50.0).contains(&h.h), "{c:?} is not gold");
                for bg in [lapis.toolbar, lapis.well, lapis.panel] {
                    assert!(contrast(c, bg) >= 4.5, "{c:?} on {bg:?}");
                }
            }
            // The first reads stronger than the second.
            assert!(
                contrast(lapis.readout, lapis.toolbar)
                    > contrast(lapis.readout_soft, lapis.toolbar)
            );
        }
    }

    #[test]
    fn lapis_keeps_every_promise_as_written() {
        for t in [slate_lapis_night_tokens(), slate_lapis_paper_tokens()] {
            let bad = t.legibility_failures();
            assert!(bad.is_empty(), "{bad:#?}");
        }
        // The theme resolves to exactly its colours, and to Slate's resolved shape.
        for (theme, tokens, slate) in [
            (
                slate_lapis_night(),
                slate_lapis_night_tokens(),
                crate::theme::slate_night(),
            ),
            (
                slate_lapis_paper(),
                slate_lapis_paper_tokens(),
                crate::theme::slate_paper(),
            ),
        ] {
            let got = theme.kit_tokens();
            assert!(differ(&got, &tokens).is_empty(), "{}", theme.name);
            let slate = slate.kit_tokens();
            assert_eq!(
                (got.radius, got.round, got.depth, got.density, got.system),
                (
                    slate.radius,
                    slate.round,
                    slate.depth,
                    slate.density,
                    slate.system
                ),
                "{}",
                theme.name
            );
        }
    }
}

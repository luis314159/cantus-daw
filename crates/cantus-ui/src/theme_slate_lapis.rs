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
//!   (222° at night, 226° on paper) and its saturation × 4.2 (night) / × 5.5 (paper):
//!   a deep lapis blue in Night, a clear blue paper in Paper. (Round 39 shipped × 1.7 /
//!   × 1.8; the owner found it too close to Slate and chose the bluest of three stronger
//!   rooms, round 40 — `docs/round40/look/lapis/`.) The same luminance keeps
//!   every contrast Slate holds; the text, the keys, the knob and fader caps (the metal),
//!   the meters, the cables' and the status colours are Slate's.
//!
//! Slate Lapis has been the default since 2026-11-18 (ADR 0111, amended;
//! [`crate::theme::default_theme`]); Slate, round 39's default, does not change.

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
/// The hue the room turns to (the lapis's own), and how much more colour it takes (the
/// owner's choice, round 40: "C", the bluest).
pub const NIGHT_HUE: f32 = 222.0;
pub const PAPER_HUE: f32 = 226.0;
pub const NIGHT_GAIN: f32 = 4.2;
pub const PAPER_GAIN: f32 = 5.5;

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
        ink: hex(0x05163d),
        well: hex(0x0b1c45),
        panel: hex(0x0c204c),
        toolbar: hex(0x13295d),
        panel_raised: hex(0x13295d),
        panel_light: hex(0x0c204c),
        grid: hex(0x102455),
        grid_strong: hex(0x122a61),
        grid_bar: hex(0x1a3678),
        edge: hex(0x6984c1),
        placeholder: hex(0x6984c1),
        meter_well: hex(0x031236),
        strip: hex(0x13295d),
        strip_selected: hex(0x1a336d),
        header_cell: hex(0x1a336d),
        device_ground: hex(0x05163d),
        deck: hex(0x13295d),
        deck_title: hex(0x162f69),
        value_pill: hex(0x0b1c45),
        step_odd: hex(0x1a336d),
        step_even: hex(0x213d7d),
        row_white: hex(0x0e2355),
        row_black: hex(0x0c204c),
        menu: hex(0x1a336d),
        menu_band: hex(0x152c63),
        disabled: hex(0x2e4d93),
        inner_shadow: hex(0x031236),
        lip: hex(0x19326c),
        track_veil: alpha(0x0c204c, 150),
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
        ink: hex(0xfafcff),
        well: hex(0xd1dbff),
        panel: hex(0xf3f6ff),
        toolbar: hex(0xe1e8ff),
        panel_raised: hex(0xe1e8ff),
        panel_light: hex(0xf3f6ff),
        grid: hex(0xe6ebff),
        grid_strong: hex(0xd3deff),
        grid_bar: hex(0xbfcefd),
        edge: hex(0x506fd3),
        placeholder: hex(0x506fd3),
        meter_well: hex(0xfafcff),
        strip: hex(0xe1e8ff),
        strip_selected: hex(0xd5deff),
        header_cell: hex(0xfafcff),
        device_ground: hex(0xbccbfb),
        deck: hex(0xe1e8ff),
        deck_title: hex(0xdbe4ff),
        value_pill: hex(0xfafcff),
        step_odd: hex(0xd1dbff),
        step_even: hex(0xc5d2fd),
        row_white: hex(0xecf0ff),
        row_black: hex(0xf3f6ff),
        menu: hex(0xfafcff),
        menu_band: hex(0xecf0ff),
        disabled: hex(0x9cb0ef),
        inner_shadow: hex(0xafc0f9),
        lip: hex(0xedf1ff),
        // r40:look — the veil stays round 39's paper (`#f4f6fa`), not the bluer room's: a
        // veil as blue as the room laid the blue tracks' bodies at chroma 20.4, past the
        // calm of 20 (`arrangement_legibility`).
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

/// The room's rule for one colour: Slate's `c` at **its own WCAG luminance**, its hue
/// turned to `hue` and its HSL saturation × `gain` (at most full). The lightness that
/// keeps the luminance is found by bisection, and of the 8-bit colours around it the one
/// whose luminance is nearest wins — so every contrast Slate holds on that surface, the
/// room holds (round 39 `lapis2`; round 40 `look`, the owner's stronger room).
pub fn turned(c: Color32, hue: f32, gain: f32) -> Color32 {
    use crate::kit::color::{luminance, opaque, Hsl};
    let a = c.a();
    let base = opaque(c);
    let target = luminance(base);
    let s = (Hsl::of(base).s * gain).min(1.0);
    let at = |l: f32| Hsl { h: hue, s, l }.rgb();
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..32 {
        let mid = 0.5 * (lo + hi);
        if luminance(at(mid)) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let near = [at(lo), at(hi), at(lo - 0.002), at(hi + 0.002)];
    let best = near
        .into_iter()
        .min_by(|x, y| {
            let dx = (luminance(*x) - target).abs();
            let dy = (luminance(*y) - target).abs();
            dx.total_cmp(&dy)
        })
        .unwrap_or(base);
    if a == 255 {
        best
    } else {
        let [r, g, b, _] = best.to_srgba_unmultiplied();
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }
}

/// The room's tokens by field: the surfaces, wells and lines of Slate's ladder that turn
/// to the lapis (the track veil follows the panel).
pub const ROOM_FIELDS: [&str; 28] = [
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

/// `lapis` with its room re-derived from `slate` at `hue` × `gain` — the written tokens'
/// rule, for checking them and for shooting other strengths.
pub fn with_room(lapis: Tokens, slate: &Tokens, hue: f32, gain: f32) -> Tokens {
    use crate::kit::tokens::Tok;
    let mut t = lapis;
    for tok in Tok::ALL {
        if ROOM_FIELDS.contains(&tok.field()) {
            t.set(*tok, turned(slate.get(*tok), hue, gain));
        }
    }
    let [r, g, b, _] = t.panel.to_srgba_unmultiplied();
    t.track_veil = Color32::from_rgba_unmultiplied(r, g, b, slate.track_veil.a());
    t
}

/// Slate Lapis with its room at `gain` instead of the written one, as a theme named
/// `name` — for the shots the owner chooses a strength from (round 40, `look`).
pub fn at_gain(dark: bool, gain: f32, name: &str) -> Theme {
    if dark {
        let t = with_room(
            slate_lapis_night_tokens(),
            &slate_night_tokens(),
            NIGHT_HUE,
            gain,
        );
        family(name, t, NIGHT_LAPIS, NIGHT_LAPIS_BRIGHT)
    } else {
        let t = with_room(
            slate_lapis_paper_tokens(),
            &slate_paper_tokens(),
            PAPER_HUE,
            gain,
        );
        family(name, t, PAPER_LAPIS, PAPER_LAPIS_BRIGHT)
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
                // Near white (Paper's ink) 8-bit rounding moves the hue most.
                let room = if hl.l > 0.97 { 14.0 } else { 8.0 };
                assert!(off <= room, "{key}: hue {} is not the lapis's {hue}", hl.h);
                // More colour, by about the gain (rounding moves a pale one most).
                assert!(
                    hl.s >= (hs.s * (gain - 0.4)).min(0.9),
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
    fn the_written_room_is_the_rule_at_its_gain() {
        // The rule re-derives the written tokens to a count or two per channel.
        for ((lapis, slate), (hue, gain)) in pairs()
            .into_iter()
            .zip([(NIGHT_HUE, NIGHT_GAIN), (PAPER_HUE, PAPER_GAIN)])
        {
            let derived = with_room(lapis, &slate, hue, gain);
            for key in ROOM_FIELDS {
                let (a, b) = (field(&lapis, key), field(&derived, key));
                let d = a
                    .to_array()
                    .iter()
                    .zip(b.to_array())
                    .map(|(x, y)| (*x as i32 - y as i32).abs())
                    .max()
                    .unwrap_or(0);
                assert!(d <= 3, "{key}: written {a:?}, the rule {b:?}");
            }
        }
    }

    #[test]
    fn every_stronger_room_keeps_every_promise() {
        // The strengths the owner chooses from (round 40): more colour each, the same
        // luminance, every kit promise (ADR 0111: text 4.5:1, non-text 3:1).
        for gain in [1.7f32, 2.5, 2.8, 3.3, 4.0, 4.2, 5.5] {
            for dark in [true, false] {
                let theme = at_gain(dark, gain, "Lapis room");
                let t = theme.kit_tokens();
                let bad = t.legibility_failures();
                assert!(bad.is_empty(), "x{gain} dark {dark}: {bad:#?}");
                let slate = if dark {
                    slate_night_tokens()
                } else {
                    slate_paper_tokens()
                };
                for key in ROOM_FIELDS {
                    let (l, s) = (field(&t, key), field(&slate, key));
                    let (ll, ls) = (luminance(l), luminance(s));
                    assert!((ll - ls).abs() <= 0.006 + 0.01 * ls, "{key} x{gain}");
                }
            }
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

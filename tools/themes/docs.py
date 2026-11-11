"""The two generated modules' documentation and preamble."""

PRELUDE = """
use crate::kit::system::{CableStyle, System};
use crate::kit::tokens::Tokens;
use crate::theme::Theme;
use crate::theme_family::palette_of;
use egui::Color32;

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

const fn alpha(v: u32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const((v >> 16) as u8, (v >> 8) as u8, v as u8, a)
}

/// A theme of round 39's design system: its tokens (and so its system), the palette taken
/// from them.
fn family(name: &str, t: Tokens, accent: Color32, accent_bright: Color32) -> Theme {
    let mut theme = Theme::from_palette(name, &palette_of(&t, accent, accent_bright)).with_kit(&t);
    theme.system = t.system;
    theme
}
"""

RESTORED = """//! **The themes of rounds 24–38, redrawn in round 39's design system** (the owner,
//! 2026-11-10: "Recover all of them"; ADR 0111 §2). Round 39 retired them when the owner
//! chose Slate; they come back here, each in its own light and with its own character, on
//! Slate's structure: a dark (or, on paper, light) gutter between panels, the base under
//! the lanes, a lighter surface for bars, strips and cards, raised menus and number cells,
//! recessed wells for keys and fields with a 1-px edge and lip, three inks, one accent,
//! no gradient and no glow, the user's colours under the lane veil.
//!
//! How each is made (`tools/themes/themes.py`, which wrote this file and measured every
//! promise the kit makes before writing it): Slate's ladder turned to the theme's hue with
//! its own amount of colour, at Slate's luminance scaled in WCAG's terms — (L + 0.05) × f −
//! 0.05, which keeps every contrast between two of its surfaces as Slate has it — then the
//! theme's own colours on top, from its old constructor's palette (`theme_family.rs`,
//! `kit::tokens::Tokens::{slate, paper, ink_and_gold, parchment}`, kept unchanged for the
//! tests that hold the kit's machinery). Where a colour falls short of a promise it is
//! moved the least that holds it.
//!
//! - **Cantus Night** (dark, Graphite's system): the website's night stage — an espresso
//!   room, ivory inks, the logo's copper on the keys, its verdigris patina on the
//!   selection (and laid over every selected fill), amber on the clock, the rulers'
//!   numbers, the playhead and the loop's edges.
//! - **Cantus Paper** (light, Graphite's system): its twin on the website's cool stone —
//!   copper-deep keys, verdigris-deep selection, copper readouts.
//! - **Daylight** (light, Studio ink's system): white with blue details — a whiter, bluer
//!   paper than Slate's, one vivid blue on keys, selection, readouts and the loop, and the
//!   meters in a dark slate well, as Daylight always had them.
//! - **Blue Hour** (dark, Studio ink's system): Daylight's night — a deep navy room,
//!   electric-blue keys and selection, ice-blue readouts.
//! - **Ultraviolet** (dark, Slate's system): deep violet with neon — magenta keys and loop
//!   edges, cyan selection, playhead and readouts.
//! - **Lagoon** (dark, Slate's system): a cool teal room, teal keys, its complement amber
//!   on the selection, the readouts and the playhead.
//! - **Pewter** (dark, Slate's system; preset id `slate`, round 24's FL-derived *Slate*,
//!   renamed so it collides with neither the new Slate nor anyone's mark): a lighter, bluer
//!   slate grey than Slate's over a dark gutter, orange keys, a lime selection and playhead,
//!   lime meters (round 24's mid-grey darkened so the user's colour cells read on its bars).
//! - **Chalk** (light, Slate's system; preset id `paper`, its twin): a grey chalk paper,
//!   deep orange keys, a green selection, lime meters in a dark well.
//! - **Ink & Gold** (dark, Studio ink's system): the first theme — a blue-black ink room,
//!   warm ivory text, gold on the keys, the readouts and the playhead, a soft blue
//!   selection.
//! - **Parchment** (light, Studio ink's system): a warm cream sheet, gold-brown keys and
//!   selection, sepia readouts.
//!
//! No restored theme keeps Classic's raised keys: none of them was about them (each was
//! its colours; Pewter's FL character is its grey chrome and lime, not the relief), so
//! every one takes the recessed keys that are round 39's standard.
"""

ADDED = """//! **Themes new in round 39** (the owner, 2026-11-10: "add more if you want"; ADR 0111
//! §2): each a character of its own on Slate's structure, made and measured as the
//! restored ones are (`tools/themes/themes.py`, which wrote this file;
//! `crate::theme_redrawn` says how).
//!
//! - **Evergreen Night** (dark, Graphite's system with Jack cables): a deep forest-green
//!   room with brass — brass keys, selection, readouts and loop, cream inks; real patch
//!   cords on the mixer, as on a console of that era.
//! - **Evergreen Paper** (light, the same system): a pale sage sheet; the forest green
//!   carries the keys and the selection (brass is muddy as a key on paper), brass the
//!   readouts, the playhead and the loop.
//! - **Bordeaux** (dark, Graphite's system): a deep wine room, warm ivory inks, rose gold
//!   on the keys and the selection, champagne on the readouts, the playhead and the loop.
//! - **Print** (light, Studio ink's system with Subway cables): black ink on white paper
//!   and one spot of vermilion — every surface a neutral grey, the keys and the selection
//!   in ink, vermilion for the playhead, the loop, record and clip; the meters in greys;
//!   the user's colours under a heavier veil; the cables drawn as a printed diagram.
"""

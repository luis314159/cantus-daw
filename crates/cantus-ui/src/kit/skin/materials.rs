//! How a unit's face is made (round 32, ADR 0074): the [`Material`] of the device ground,
//! its cards and its header bar, laid over the colour the part already painted — through
//! the lighting recipe's light and shade (the theme's `light` and `dark` tokens, scaled by
//! its depth), never a colour of its own. The surface's colour is the skin's tint.

use super::super::brush::{Brush, Fill};
use super::super::color::{contrast, ensure_contrast_all, over, with_alpha};
use super::super::Tokens;
use super::super::{Style, Tok};
use super::textures::{self, Tile};
use super::Material;
use egui::{Color32, Pos2, Rect};

/// The tokens a material paints with, over its surface's own: the recipe's light and
/// shade — and the panel and the strip, which the material's contrast floor reads
/// ([`floor`]: the text it lifts over the header's darkest and lightest is held on them
/// too).
pub const TOKENS: &[Tok] = &[Tok::Light, Tok::Dark, Tok::Panel, Tok::Strip];

/// The part of the face being finished.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The device ground behind the cards: the material whole.
    Ground,
    /// A card: the material lighter, so the controls on it stay the eye's first stop.
    Card,
    /// The header bar (the window's title).
    Header,
}

impl Material {
    /// Every material, the kit's first.
    pub const ALL: [Material; 10] = [
        Material::Plain,
        Material::Matte,
        Material::Brushed,
        Material::Anodised,
        Material::Lacquer,
        Material::Wood,
        Material::Tolex,
        Material::Glass,
        Material::Paper,
        Material::Carbon,
    ];

    /// The name the gallery and the shots give it.
    pub fn name(self) -> &'static str {
        match self {
            Material::Plain => "plain",
            Material::Matte => "matte",
            Material::Brushed => "brushed",
            Material::Anodised => "anodised",
            Material::Lacquer => "lacquer",
            Material::Wood => "wood",
            Material::Tolex => "vinyl",
            Material::Glass => "glass",
            Material::Paper => "paper",
            Material::Carbon => "carbon",
        }
    }
}

/// Finish `rect` (corner `r`), already filled with `base`, in the skin's material for
/// `surface`, at `alpha` (a disabled part's). The plain material paints nothing.
pub fn finish(
    b: &mut Brush,
    rect: Rect,
    r: f32,
    base: Color32,
    s: &Style,
    surface: Surface,
    alpha: f32,
) {
    let m = s.skin.material;
    if m == Material::Plain || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    b.register(rect, "material", TOKENS, &[]);
    let t = &s.t;
    let d = t.depth.clamp(0.0, 1.5);
    // How much of it: the ground whole, a card less, the header between.
    let amount = match surface {
        Surface::Ground => 1.0,
        Surface::Card => 0.55,
        Surface::Header => 0.8,
    } * alpha;
    let light = |a: f32| with_alpha(t.light, (a * amount * d.max(0.35)).clamp(0.0, 1.0));
    let shade = |a: f32| with_alpha(t.dark_shade, (a * amount * d.max(0.35)).clamp(0.0, 1.0));
    let rect = b.snap_rect(rect);
    let px = b.px();
    // The tiles' strength: a card's texture quieter than the ground's, so the controls on
    // it stay the eye's first stop.
    let tex = match surface {
        Surface::Card => 0.32 * alpha,
        _ => amount,
    };
    match m {
        Material::Plain => {}
        Material::Matte => {
            // A flat paint, soft-lit: the recipe's ramp flattened, a broad glow above.
            b.rrect(rect, r, Fill::Solid(base.gamma_multiply(alpha)));
            let top = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.center().y));
            b.rrect(top, r, Fill::V(light(0.07), light(0.0)));
        }
        Material::Brushed => {
            textures::lay_rounded(b, rect, r, Tile::Brushed, t.light, 0.10 * tex);
            // A sheen band across, a third of the way down.
            let stops = [
                (0.0, light(0.0)),
                (0.28, light(0.09)),
                (0.42, light(0.02)),
                (0.75, light(0.0)),
                (1.0, shade(0.10)),
            ];
            b.rrect(rect, r, Fill::Stops(&stops));
        }
        Material::Anodised => {
            // A deep colour with a metallic ramp: bright at the top, a soft band below the
            // middle, darker at the foot; a whisper of grain.
            let stops = [
                (0.0, light(0.16)),
                (0.18, light(0.03)),
                (0.55, light(0.07)),
                (0.62, light(0.0)),
                (1.0, shade(0.22)),
            ];
            b.rrect(rect, r, Fill::Stops(&stops));
            textures::lay_rounded(b, rect, r, Tile::Brushed, t.light, 0.045 * tex);
            b.hline(
                rect.left() + r,
                rect.right() - r,
                rect.top() + px * 0.5,
                px,
                light(0.35),
            );
        }
        Material::Lacquer => {
            // Gloss: a strong reflection over the upper part, a crisp edge to it, the lower
            // edge dark.
            let cut = rect.top() + rect.height() * 0.42;
            let upper = Rect::from_min_max(rect.min, Pos2::new(rect.right(), cut));
            b.rrect(upper, r, Fill::V(light(0.20), light(0.05)));
            b.hline(rect.left() + r, rect.right() - r, cut, px, light(0.10));
            let foot = Rect::from_min_max(
                Pos2::new(rect.left(), rect.bottom() - rect.height() * 0.2),
                rect.max,
            );
            b.rrect(foot, r, Fill::V(shade(0.0), shade(0.28)));
            b.hline(
                rect.left() + r,
                rect.right() - r,
                rect.top() + px * 0.5,
                px,
                light(0.45),
            );
        }
        Material::Wood => {
            if surface == Surface::Card {
                // A panel set in a wooden case: the card stays a painted panel.
                b.rrect(rect, r, Fill::V(light(0.05), light(0.0)));
                return;
            }
            // The grain and paper's fibre (a fifth as strong) in one tile.
            textures::lay_rounded(b, rect, r, Tile::Varnished, t.light, 0.30 * tex);
            // Varnish: a soft light above, a little depth below.
            b.rrect(rect, r, Fill::V(light(0.08), shade(0.10)));
        }
        Material::Tolex => {
            textures::lay_rounded(b, rect, r, Tile::Tolex, t.light, 0.16 * tex);
            b.rrect(rect, r, Fill::V(light(0.03), shade(0.08)));
        }
        Material::Glass => {
            // Smoked glass over the colour: darker, a diagonal reflection, a lit lip.
            b.rrect(rect, r, Fill::V(shade(0.18), shade(0.36)));
            let (w, h) = (rect.width(), rect.height());
            let x0 = rect.left() + w * 0.08;
            let band = (w * 0.22).min(h * 1.2);
            let pts = [
                Pos2::new(x0, rect.top() + px),
                Pos2::new(x0 + band, rect.top() + px),
                Pos2::new((x0 + band - h * 0.6).max(rect.left()), rect.bottom() - px),
                Pos2::new((x0 - h * 0.6).max(rect.left()), rect.bottom() - px),
            ];
            b.polygon(&pts, light(0.05));
            b.hline(
                rect.left() + r,
                rect.right() - r,
                rect.top() + px * 0.5,
                px,
                light(0.40),
            );
        }
        Material::Paper => {
            textures::lay_rounded(b, rect, r, Tile::Paper, t.light, 0.16 * tex);
            b.rrect(rect, r, Fill::V(light(0.04), shade(0.04)));
        }
        Material::Carbon => {
            // A dark weave: on a light theme only its shadows read, and softly.
            let k = if t.dark { 0.26 } else { 0.10 };
            textures::lay_rounded(b, rect, r, Tile::Carbon, t.light, k * tex);
            b.rrect(rect, r, Fill::V(light(0.08), shade(0.12)));
        }
    }
}

/// How much `finish` lays at most, anywhere text can sit on `surface` (its one-pixel
/// lips aside — no text is read on a hairline): the gradients' light and shade (before
/// the surface's amount and the theme's depth) and the tiles with their strengths (before
/// the surface's texture amount; a tile's own reach is `textures::reach`). Kept beside
/// `finish`, recipe by recipe: change one, change the other.
fn peaks(m: Material, surface: Surface, dark: bool) -> (f32, f32, &'static [(Tile, f32)]) {
    match m {
        Material::Plain => (0.0, 0.0, &[]),
        Material::Matte => (0.07, 0.0, &[]),
        Material::Brushed => (0.09, 0.10, &[(Tile::Brushed, 0.10)]),
        Material::Anodised => (0.16, 0.22, &[(Tile::Brushed, 0.045)]),
        Material::Lacquer => (0.20, 0.28, &[]),
        Material::Wood if surface == Surface::Card => (0.05, 0.0, &[]),
        Material::Wood => (0.08, 0.10, &[(Tile::Varnished, 0.30)]),
        Material::Tolex => (0.03, 0.08, &[(Tile::Tolex, 0.16)]),
        // The reflection lies over the smoke: counting it alone is the lighter bound.
        Material::Glass => (0.05, 0.36, &[]),
        Material::Paper => (0.04, 0.04, &[(Tile::Paper, 0.16)]),
        Material::Carbon if dark => (0.08, 0.12, &[(Tile::Carbon, 0.26)]),
        Material::Carbon => (0.08, 0.12, &[(Tile::Carbon, 0.10)]),
    }
}

/// The darkest and the lightest colour `finish` makes of `base` on `surface` in tokens
/// `t`: what the text read there must keep its contrast on.
pub fn extremes(m: Material, surface: Surface, t: &Tokens, base: Color32) -> [Color32; 2] {
    if m == Material::Plain {
        return [base, base];
    }
    let d = t.depth.clamp(0.0, 1.5).max(0.35);
    let amount = match surface {
        Surface::Ground => 1.0,
        Surface::Card => 0.55,
        Surface::Header => 0.8,
    };
    let tex = match surface {
        Surface::Card => 0.32,
        _ => amount,
    };
    let (gl, gs, tiles) = peaks(m, surface, t.dark);
    let mut darkest = over(
        with_alpha(t.dark_shade, (gs * amount * d).clamp(0.0, 1.0)),
        base,
    );
    let mut lightest = over(with_alpha(t.light, (gl * amount * d).clamp(0.0, 1.0)), base);
    for (tile, k) in tiles {
        let (up, down) = textures::reach(*tile);
        let a = k * tex;
        // A tile's shade is black by alpha: the recipe's shade token, black in every preset.
        darkest = over(
            with_alpha(t.dark_shade, (down * a).clamp(0.0, 1.0)),
            darkest,
        );
        lightest = over(with_alpha(t.light, (up * a).clamp(0.0, 1.0)), lightest);
    }
    [darkest, lightest]
}

/// The contrast floor over a material (the integrator's review, 4): text and dim text
/// pushed to 4.5:1 on the darkest and the lightest colour the material makes of a card
/// and of the header, labels and the function hues on those of a card — as well as on
/// every surface `Tokens::floor` already holds them to.
pub(crate) fn floor(t: &mut Tokens, m: Material) {
    if m == Material::Plain {
        return;
    }
    let [cd, cl] = extremes(m, Surface::Card, t, t.deck);
    let [hd, hl] = extremes(m, Surface::Header, t, t.panel);
    // A colour moves only when the material takes it below the floor, and then against
    // the material's colours and the surfaces it was already floored on together.
    let lift = |fg: Color32, over: &[Color32], also: &[Color32]| {
        if over.iter().all(|bg| contrast(fg, *bg) >= 4.5) {
            return fg;
        }
        // On the stack: the painters allocate nothing (`tests/kit_alloc.rs`).
        let mut all = [Color32::TRANSPARENT; 12];
        let n = over.len() + also.len();
        all[..over.len()].copy_from_slice(over);
        all[over.len()..n].copy_from_slice(also);
        ensure_contrast_all(fg, &all[..n], 4.5)
    };
    t.text = lift(
        t.text,
        &[cd, cl, hd, hl],
        &[
            t.panel,
            t.strip,
            t.strip_selected,
            t.panel_raised,
            t.deck,
            t.deck_title,
            t.well,
            t.ink,
        ],
    );
    t.text_dim = lift(t.text_dim, &[cd, cl, hd, hl], &[t.panel, t.strip, t.deck]);
    t.label = lift(t.label, &[cd, cl], &[t.strip, t.deck, t.deck_title]);
    // The hues on cards only: the kit promises none on the panel the header is painted in.
    let on = [t.deck, t.deck_title, t.ink];
    t.hue_source = lift(t.hue_source, &[cd, cl], &on);
    t.hue_tone = lift(t.hue_tone, &[cd, cl], &on);
    t.hue_dynamics = lift(t.hue_dynamics, &[cd, cl], &on);
    t.hue_mod = lift(t.hue_mod, &[cd, cl], &on);
    t.hue_space = lift(t.hue_space, &[cd, cl], &on);
    t.hue_output = lift(t.hue_output, &[cd, cl], &on);
}

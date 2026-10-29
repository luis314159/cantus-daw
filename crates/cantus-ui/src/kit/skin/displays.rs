//! One way to light a display (round 32, ADR 0074): the colours and the ground of a
//! unit's characteristic display in its skin's [`Display`] look — a CRT's phosphor, an
//! LCD, a VFD, glass over the face, ink on paper, a scope's graticule — and the strokes
//! that live on it ([`trace`], [`graticule`], [`needle_meter`]). The families' editors
//! call these (the effects through `views::fx::canvas::Ink`, which reads [`ink`]); nothing
//! here is one unit's.
//!
//! A display's colours come from the theme's tokens in the plain look and from the look's
//! own data (an LCD is olive in Slate and in Paper alike) in the others, with its text
//! pushed to WCAG AA against its ground.

use super::super::brush::{Brush, Fill};
use super::super::cache;
use super::super::color::{ensure_contrast, mix, shade, with_alpha, Hsl};
use super::super::text::{self, Role, Text};
use super::super::{Style, Tok, Tokens};
use super::textures::{self, Tile};
use super::Display;
use egui::{Align2, Color32, Pos2, Rect, Vec2};
use std::cell::Cell;

/// A CRT's phosphor when the skin names no accent: P31's green.
const P31: Hsl = Hsl {
    h: 135.0,
    s: 0.72,
    l: 0.56,
};
/// An LCD's glass, lit from behind (dark theme) and by the room (light theme).
const LCD_DARK: Hsl = Hsl {
    h: 82.0,
    s: 0.17,
    l: 0.62,
};
const LCD_LIGHT: Hsl = Hsl {
    h: 82.0,
    s: 0.20,
    l: 0.71,
};
/// An LCD's segments.
const LCD_INK: Hsl = Hsl {
    h: 100.0,
    s: 0.28,
    l: 0.11,
};
/// A VFD's light: cyan-green.
const VFD: Hsl = Hsl {
    h: 168.0,
    s: 0.88,
    l: 0.60,
};
/// Printed chart paper, and its ink.
const PAPER_LIGHT: Hsl = Hsl {
    h: 44.0,
    s: 0.40,
    l: 0.93,
};
const PAPER_DARK: Hsl = Hsl {
    h: 42.0,
    s: 0.24,
    l: 0.83,
};
const PAPER_RULE: Hsl = Hsl {
    h: 205.0,
    s: 0.50,
    l: 0.50,
};
const PAPER_INK: Hsl = Hsl {
    h: 226.0,
    s: 0.55,
    l: 0.26,
};
const PAPER_TEXT: Hsl = Hsl {
    h: 30.0,
    s: 0.16,
    l: 0.14,
};
/// A scope's beam, its glass and its graticule.
const BEAM: Hsl = Hsl {
    h: 186.0,
    s: 0.88,
    l: 0.60,
};
const SCOPE_GROUND: Hsl = Hsl {
    h: 200.0,
    s: 0.28,
    l: 0.07,
};
const SCOPE_LINE: Hsl = Hsl {
    h: 190.0,
    s: 0.14,
    l: 0.62,
};
/// A VU meter's backlit face.
const VU_FACE: Hsl = Hsl {
    h: 43.0,
    s: 0.62,
    l: 0.80,
};
const VU_RED: Hsl = Hsl {
    h: 2.0,
    s: 0.70,
    l: 0.44,
};

impl Display {
    /// Every look, the kit's first.
    pub const ALL: [Display; 7] = [
        Display::Plain,
        Display::Phosphor,
        Display::Lcd,
        Display::Vfd,
        Display::Glass,
        Display::Paper,
        Display::Scope,
    ];

    /// The name the gallery and the shots give it.
    pub fn name(self) -> &'static str {
        match self {
            Display::Plain => "plain",
            Display::Phosphor => "phosphor",
            Display::Lcd => "lcd",
            Display::Vfd => "vfd",
            Display::Glass => "glass",
            Display::Paper => "paper",
            Display::Scope => "scope",
        }
    }
}

/// The colours of a display in its look.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DisplayInk {
    pub look: Display,
    pub ground: Color32,
    /// The ground's top (it is lit a hair from above).
    pub ground_top: Color32,
    /// The grid: opaque, as the theme's grid token is (a painter that thins it with an
    /// alpha thins the same faint line in every look).
    pub grid: Color32,
    pub grid_strong: Color32,
    pub edge: Color32,
    /// Words on it: AA (4.5:1) on the ground.
    pub text: Color32,
    /// Dim words: 3:1 on the ground.
    pub dim: Color32,
    /// The main trace (the unit's accent in the plain look).
    pub trace: Color32,
    /// A second voice.
    pub trace2: Color32,
    /// Behind a label over data.
    pub plate: Color32,
    /// A secondary line.
    pub faint: Color32,
    /// How much a trace glows (0: none).
    pub glow: f32,
    /// Whether the display is dark (a CRT is dark on a light theme too).
    pub dark: bool,
    /// The theme's light, for reflections.
    pub light: Color32,
    /// The theme's shade.
    pub shade: Color32,
    /// Gain reduction, clipping, a warning — words: 4.5:1 on the ground in a look of its
    /// own (the integrator's review of dynamics: the theme's red was 1.3–3.5:1 on an LCD,
    /// paper or a CRT); the theme's own in the plain look.
    pub danger: Color32,
    /// Something open or passing (a gate's open lane): 3:1 on the ground in a look of its
    /// own; the theme's in the plain look.
    pub ok: Color32,
}

/// The tokens a display paints with in `look` (for inspect mode), beside its hue.
pub fn tokens(look: Display) -> &'static [Tok] {
    match look {
        Display::Plain => &[
            Tok::Ink,
            Tok::Grid,
            Tok::GridStrong,
            Tok::Edge,
            Tok::Text,
            Tok::TextDim,
        ],
        Display::Glass => &[Tok::Deck, Tok::Text, Tok::TextDim, Tok::Light],
        Display::Lcd => &[Tok::Dark],
        Display::Paper => &[Tok::Light],
        _ => &[Tok::Light, Tok::Dark],
    }
}

thread_local! {
    /// The last few display inks made (one per open unit's look and hue): a look's
    /// colours cost contrast searches, done once — three editors side by side must not
    /// make each other's again every frame.
    static LAST: Cell<[Option<(u64, DisplayInk)>; 8]> = const { Cell::new([None; 8]) };
}

/// A display's colours in the style's skin.
pub fn ink(s: &Style) -> DisplayInk {
    let t = &s.t;
    let look = s.skin.display;
    let key = cache::key((
        (look, s.skin.key, cache::ckey(s.hue), t.dark),
        (
            cache::ckey(t.ink),
            cache::ckey(t.deck),
            cache::ckey(t.text),
            cache::ckey(t.text_dim),
            cache::ckey(t.grid),
            cache::ckey(t.grid_strong),
        ),
        (
            cache::ckey(t.edge),
            cache::ckey(t.light),
            cache::ckey(t.dark_shade),
            cache::ckey(t.hue_mod),
        ),
        (
            cache::ckey(t.danger),
            cache::ckey(t.meter_low),
            s.skin.colour_key(),
        ),
    ));
    let last = LAST.with(|l| l.get());
    if let Some((_, ink)) = last.iter().flatten().find(|(k, _)| *k == key) {
        return *ink;
    }
    super::count_display();
    let mut ink = make(s);
    // Every look's voices readable on its ground: the main trace and a second voice at
    // 3:1, and the second never the main one's colour.
    ink.trace = ensure_contrast(ink.trace, ink.ground, 3.0);
    ink.trace2 = ensure_contrast(ink.trace2, ink.ground, 3.0);
    if ink.trace2 == ink.trace {
        ink.trace2 = ensure_contrast(mix(ink.trace, ink.text, 0.5), ink.ground, 3.0);
    }
    if ink.look != Display::Plain {
        ink.danger = ensure_contrast(ink.danger, ink.ground, 4.5);
        ink.ok = ensure_contrast(ink.ok, ink.ground, 3.0);
    }
    // The newest first; the oldest falls off.
    let mut next = [None; 8];
    next[0] = Some((key, ink));
    next[1..].copy_from_slice(&last[..7]);
    LAST.with(|l| l.set(next));
    ink
}

fn make(s: &Style) -> DisplayInk {
    let t = &s.t;
    // A look's colours follow its own polarity, not the theme's: a CRT, a VFD and a scope
    // are dark in Paper too, chart paper is light in Slate.
    let on_dark = s.skin.accent_in(true);
    let on_light = s.skin.accent_in(false);
    let second = s.skin.accent2_in(t.dark);
    let plain = DisplayInk {
        look: Display::Plain,
        ground: t.ink,
        ground_top: if t.dark { shade(t.ink, 0.025) } else { t.ink },
        grid: t.grid,
        grid_strong: if t.dark {
            shade(t.grid, 0.07)
        } else {
            t.grid_strong
        },
        edge: with_alpha(t.edge, 0.55),
        text: t.text,
        dim: t.text_dim,
        trace: s.hue,
        trace2: second.unwrap_or(t.hue_mod),
        plate: with_alpha(t.ink, 0.86),
        faint: with_alpha(t.text_dim, if t.dark { 0.30 } else { 0.40 }),
        glow: 0.0,
        dark: t.dark,
        light: t.light,
        shade: t.dark_shade,
        danger: t.danger,
        ok: t.meter_low,
    };
    // A look of its own: a ground, a light on it, words pushed to AA.
    let own = |ground: Color32, top: Color32, light: Color32, line: Color32, glow: f32| {
        let dark = super::super::color::luminance(ground) < 0.18;
        let text = ensure_contrast(if dark { shade(light, 0.16) } else { light }, ground, 4.5);
        let dim = ensure_contrast(mix(text, ground, 0.42), ground, 3.0);
        DisplayInk {
            look: s.skin.display,
            ground,
            ground_top: top,
            grid: mix(ground, line, 0.16),
            grid_strong: mix(ground, line, 0.30),
            edge: if dark {
                shade(ground, 0.14)
            } else {
                shade(ground, -0.22)
            },
            text,
            dim,
            trace: light,
            // The second accent as the display's own polarity wants it (a CRT is dark in
            // Paper too; an LCD light in Slate): the integrator's review of modulation.
            trace2: s
                .skin
                .accent2_in(dark)
                .unwrap_or_else(|| shade(light, if dark { 0.22 } else { -0.16 })),
            plate: with_alpha(ground, 0.88),
            faint: mix(ground, dim, 0.45),
            glow,
            dark,
            light: t.light,
            shade: t.dark_shade,
            danger: t.danger,
            ok: t.meter_low,
        }
    };
    match s.skin.display {
        Display::Plain => plain,
        Display::Phosphor => {
            let p = Hsl {
                l: 0.60,
                ..on_dark.map_or(P31, Hsl::of)
            }
            .rgb();
            let h = Hsl::of(p).h;
            let g = Hsl {
                h,
                s: 0.40,
                l: 0.05,
            }
            .rgb();
            let top = Hsl {
                h,
                s: 0.36,
                l: 0.075,
            }
            .rgb();
            own(g, top, p, p, 1.0)
        }
        Display::Lcd => {
            let g = if t.dark { LCD_DARK } else { LCD_LIGHT }.rgb();
            let ink = LCD_INK.rgb();
            let mut d = own(g, shade(g, 0.02), ink, ink, 0.0);
            d.trace2 = s
                .skin
                .accent2_in(false)
                .map(|c| ensure_contrast(c, g, 3.0))
                .unwrap_or_else(|| mix(ink, g, 0.35));
            d
        }
        Display::Vfd => {
            // The light is the accent's (the VFD's cyan-green without one); the second
            // accent stays the second voice.
            let v = on_dark.map_or(VFD, |c| Hsl {
                l: 0.62,
                s: Hsl::of(c).s.max(0.6),
                ..Hsl::of(c)
            });
            let v = v.rgb();
            let g = Hsl {
                h: Hsl::of(v).h,
                s: 0.30,
                l: 0.04,
            }
            .rgb();
            own(g, g, v, v, 0.7)
        }
        Display::Glass => {
            let g = shade(t.deck, if t.dark { -0.12 } else { -0.08 });
            let text = ensure_contrast(t.text, g, 4.5);
            DisplayInk {
                look: Display::Glass,
                ground: g,
                ground_top: shade(g, 0.03),
                text,
                dim: ensure_contrast(t.text_dim, g, 3.0),
                plate: with_alpha(g, 0.88),
                edge: with_alpha(t.edge, 0.7),
                ..plain
            }
        }
        Display::Paper => {
            let g = if t.dark { PAPER_DARK } else { PAPER_LIGHT }.rgb();
            let pen = on_light.map_or(PAPER_INK.rgb(), |c| {
                Hsl {
                    l: Hsl::of(c).l.min(0.34),
                    s: Hsl::of(c).s.max(0.5),
                    ..Hsl::of(c)
                }
                .rgb()
            });
            let pen = ensure_contrast(pen, g, 4.5);
            let mut d = own(g, g, PAPER_TEXT.rgb(), PAPER_RULE.rgb(), 0.0);
            d.trace = pen;
            d.grid = mix(g, PAPER_RULE.rgb(), 0.14);
            d.grid_strong = mix(g, PAPER_RULE.rgb(), 0.30);
            d
        }
        Display::Scope => {
            let beam = on_dark.map_or(BEAM, |c| Hsl {
                l: 0.62,
                ..Hsl::of(c)
            });
            let g = SCOPE_GROUND.rgb();
            let mut d = own(g, shade(g, 0.02), beam.rgb(), SCOPE_LINE.rgb(), 0.5);
            d.text = ensure_contrast(SCOPE_LINE.rgb(), g, 4.5);
            d.dim = ensure_contrast(mix(d.text, g, 0.4), g, 3.0);
            d.grid = mix(g, SCOPE_LINE.rgb(), 0.10);
            d.grid_strong = mix(g, SCOPE_LINE.rgb(), 0.24);
            d
        }
    }
}

thread_local! {
    /// The last few display styles' tokens (one per open unit's look).
    static STYLED: Cell<[Option<(u64, Tokens)>; 6]> = const { Cell::new([None; 6]) };
}

/// The style to draw controls and words **inside** a display in (the integrator's review
/// of the instruments: each family wrote its own `on_display`): every surface a shade of
/// the display's ground — cards, title bands, panels, value pills, wells, knob bodies —
/// its `dark` the display's (a CRT is dark in Paper, an LCD light in Slate), its words
/// the display's, its hue the display's main trace (modulation's own colour kept apart),
/// then the theme's contrast floor run on all of it. In the plain look it is the theme's
/// style as it is. Cached per look and theme.
pub fn style(s: &Style) -> Style {
    let d = ink(s);
    if d.look == Display::Plain {
        return *s;
    }
    let t0 = &s.t;
    let key = cache::key((
        (
            cache::ckey(d.ground),
            cache::ckey(d.text),
            cache::ckey(d.trace),
        ),
        (
            s.skin.colour_key(),
            t0.generation,
            t0.dark,
            cache::ckey(t0.hue_mod),
        ),
        (
            cache::ckey(t0.text),
            cache::ckey(t0.focus),
            cache::ckey(t0.selection),
        ),
    ));
    let last = STYLED.with(|c| c.get());
    let t = match last.iter().flatten().find(|(k, _)| *k == key) {
        Some((_, t)) => *t,
        None => {
            let t = style_tokens(t0, &d);
            let mut next = [None; 6];
            next[0] = Some((key, t));
            next[1..].copy_from_slice(&last[..5]);
            STYLED.with(|c| c.set(next));
            t
        }
    };
    let mut out = Style::new(t);
    out.k = s.k;
    out.skin = s.skin;
    out.cues = s.cues;
    out
}

fn style_tokens(base: &Tokens, d: &DisplayInk) -> Tokens {
    let g = d.ground;
    let mut t = *base;
    // Raised parts a little lighter than the ground, wells a little darker — in either
    // polarity, as the theme's ladder is.
    let up = |x: f32| shade(g, x);
    t.dark = d.dark;
    t.ink = g;
    t.device_ground = g;
    t.strip = g;
    t.panel = up(0.03);
    t.toolbar = up(0.03);
    // No room lighting inside a display: the theme's sheen and glows are off.
    t.sheen = with_alpha(t.sheen, 0.0);
    t.glow = with_alpha(t.glow, 0.0);
    t.glow_cool = with_alpha(t.glow_cool, 0.0);
    t.panel_raised = up(0.07);
    t.panel_light = up(0.11);
    t.deck = up(0.05);
    t.deck_title = up(0.08);
    t.strip_selected = up(0.09);
    t.header_cell = up(0.06);
    t.cap = up(0.10);
    t.well = up(-0.04);
    t.meter_well = up(-0.06);
    t.value_pill = up(-0.05);
    t.knob_body = up(if d.dark { 0.12 } else { -0.10 });
    t.knob_rim = up(if d.dark { 0.20 } else { -0.18 });
    t.knob_track = mix(g, d.text, 0.35);
    t.knob_pointer = d.text;
    t.text = d.text;
    t.text_dim = d.dim;
    t.label = d.dim;
    t.value_pill_text = d.text;
    t.placeholder = d.faint;
    t.grid = d.grid;
    t.grid_strong = d.grid_strong;
    t.edge = d.edge;
    let h = d.trace;
    t.hue_source = h;
    t.hue_tone = h;
    t.hue_dynamics = h;
    t.hue_space = h;
    t.hue_output = h;
    t.hue_mod = super::apart_from(base.hue_mod, h);
    t.floor();
    t
}

/// Paint a display's ground in `rect`, in the style's look ([`ink`]).
pub fn ground(b: &mut Brush, rect: Rect, s: &Style) {
    let d = ink(s);
    b.register(rect, "display", tokens(d.look), &[s.hue_tok]);
    ground_with(b, rect, &d);
}

/// [`ground`] with its colours made.
pub fn ground_with(b: &mut Brush, rect: Rect, d: &DisplayInk) {
    let rect = b.snap_rect(rect);
    b.rect(rect, Fill::V(d.ground_top, d.ground));
    let px = b.px();
    match d.look {
        Display::Plain | Display::Glass => {}
        Display::Phosphor => {
            // The beam's own light in the glass, brighter at the middle; the scan lines.
            let stops = [
                (0.0, with_alpha(d.trace, 0.07)),
                (0.6, with_alpha(d.trace, 0.025)),
                (1.0, with_alpha(d.trace, 0.0)),
            ];
            b.radial(rect, rect.center(), rect.size() * 0.75, &stops, 8);
            textures::lay(b, rect, Tile::Scan, d.light, 0.45);
        }
        Display::Lcd => {
            // The glass sits below the bezel: its shadow along the top and the left.
            let sh = with_alpha(d.shade, 0.28);
            b.rect(
                Rect::from_min_size(rect.min, Vec2::new(rect.width(), 2.0 * px)),
                Fill::V(sh, with_alpha(d.shade, 0.0)),
            );
            b.vline(rect.left() + px * 0.5, rect.top(), rect.bottom(), px, sh);
        }
        Display::Vfd => {
            textures::lay(b, rect, Tile::Mesh, d.light, 0.30);
        }
        Display::Paper => {
            textures::lay(b, rect, Tile::Paper, d.light, 0.07);
            // Printed chart paper: a fine grid, every fifth line stronger.
            let step = 8.0 * (rect.height() / 64.0).clamp(0.75, 2.0);
            let (cols, rows) = (
                (rect.width() / step).round().max(1.0) as u32,
                (rect.height() / step).round().max(1.0) as u32,
            );
            for i in 1..cols {
                let x = b.snap_mid(rect.left() + rect.width() * i as f32 / cols as f32);
                let c = if i % 5 == 0 { d.grid_strong } else { d.grid };
                b.vline(x, rect.top(), rect.bottom(), px, c);
            }
            for j in 1..rows {
                let y = b.snap_mid(rect.top() + rect.height() * j as f32 / rows as f32);
                let c = if j % 5 == 0 { d.grid_strong } else { d.grid };
                b.hline(rect.left(), rect.right(), y, px, c);
            }
        }
        Display::Scope => {
            graticule(b, rect, d, 10, 8);
        }
    }
}

/// What lies over a display, painted after its content (`canvas::edge_b` calls it): the
/// glass's reflection, a CRT's darker corners.
pub fn cover(b: &mut Brush, rect: Rect, d: &DisplayInk) {
    let rect = b.snap_rect(rect);
    let (w, h) = (rect.width(), rect.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let reflection = |b: &mut Brush, a: f32| {
        let x0 = rect.left() + w * 0.55;
        let band = (w * 0.16).min(h);
        let pts = [
            Pos2::new(x0, rect.top()),
            Pos2::new((x0 + band).min(rect.right()), rect.top()),
            Pos2::new((x0 + band - h * 0.55).max(rect.left()), rect.bottom()),
            Pos2::new((x0 - h * 0.55).max(rect.left()), rect.bottom()),
        ];
        b.polygon(&pts, with_alpha(d.light, a));
    };
    match d.look {
        Display::Plain | Display::Lcd | Display::Paper => {}
        Display::Glass => {
            reflection(b, 0.055);
            b.hline(
                rect.left(),
                rect.right(),
                rect.top() + b.px() * 1.5,
                b.px(),
                with_alpha(d.light, 0.22),
            );
        }
        Display::Phosphor | Display::Vfd | Display::Scope => {
            let edge = (h * 0.14).min(24.0);
            let sh = with_alpha(d.shade, 0.32);
            b.rect(
                Rect::from_min_size(rect.min, Vec2::new(w, edge)),
                Fill::V(sh, with_alpha(d.shade, 0.0)),
            );
            b.rect(
                Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - edge), rect.max),
                Fill::V(with_alpha(d.shade, 0.0), sh),
            );
            reflection(b, 0.018);
        }
    }
}

/// A trace through `pts`, `w` points wide, in `color`, glowing as the look does (a CRT's
/// beam, a VFD's segments).
pub fn trace(b: &mut Brush, pts: &[Pos2], w: f32, color: Color32, d: &DisplayInk) {
    if pts.len() < 2 {
        return;
    }
    if d.glow > 0.0 {
        // One strip: the core a touch brighter, its halo fading out to twice its width
        // (the two wide layers it replaces summed to a quarter of the colour at the
        // core's edge).
        b.glow_polyline(
            pts,
            w,
            shade(color, 0.10 * d.glow),
            with_alpha(color, 0.25 * d.glow),
            w * 1.5,
        );
    } else {
        b.polyline(pts, w, color);
    }
}

/// A graticule over `rect`: `cols` × `rows` divisions, the middle lines stronger with
/// five sub-ticks per division (an oscilloscope's).
pub fn graticule(b: &mut Brush, rect: Rect, d: &DisplayInk, cols: u32, rows: u32) {
    let rect = b.snap_rect(rect);
    let px = b.px();
    let (cols, rows) = (cols.max(1), rows.max(1));
    for i in 1..cols {
        let x = b.snap_mid(rect.left() + rect.width() * i as f32 / cols as f32);
        let c = if i * 2 == cols { d.grid_strong } else { d.grid };
        b.vline(x, rect.top(), rect.bottom(), px, c);
    }
    for j in 1..rows {
        let y = b.snap_mid(rect.top() + rect.height() * j as f32 / rows as f32);
        let c = if j * 2 == rows { d.grid_strong } else { d.grid };
        b.hline(rect.left(), rect.right(), y, px, c);
    }
    // Sub-ticks on the middle lines.
    let (cx, cy) = (rect.center().x, rect.center().y);
    let tick = (rect.height() / rows as f32 * 0.12).clamp(2.0 * px, 4.0);
    for i in 0..cols * 5 {
        let x = rect.left() + rect.width() * i as f32 / (cols * 5) as f32;
        b.vline(b.snap_mid(x), cy - tick, cy + tick, px, d.grid_strong);
    }
    for j in 0..rows * 5 {
        let y = rect.top() + rect.height() * j as f32 / (rows * 5) as f32;
        b.hline(cx - tick, cx + tick, b.snap_mid(y), px, d.grid_strong);
    }
}

/// Where a VU needle points for `vu` (−20…+3 VU): the classic scale's spacing, 0…1.
pub fn vu_position(vu: f32) -> f32 {
    // The face's marks, as a VU meter prints them (IEC 60268-17's scale, by eye).
    const MARKS: [(f32, f32); 11] = [
        (-20.0, 0.0),
        (-10.0, 0.25),
        (-7.0, 0.36),
        (-5.0, 0.46),
        (-3.0, 0.58),
        (-2.0, 0.64),
        (-1.0, 0.71),
        (0.0, 0.78),
        (1.0, 0.85),
        (2.0, 0.92),
        (3.0, 1.0),
    ];
    let v = vu.clamp(-20.0, 3.0);
    for w in MARKS.windows(2) {
        let ((a, pa), (z, pz)) = (w[0], w[1]);
        if v <= z {
            return pa + (pz - pa) * (v - a) / (z - a);
        }
    }
    1.0
}

/// A VU needle meter in `rect` (a dynamics unit's, a hardware compressor's face): the
/// backlit face, the scale with its red zone from 0 VU, the needle at `vu` (−20…+3), the
/// peak lamp lit when `peak`. `s` gives the scale `k` and the theme's light and shade.
pub fn needle_meter(b: &mut Brush, rect: Rect, s: &Style, vu: f32, peak: bool) {
    b.register(rect, "needle_meter", &[Tok::Light, Tok::Dark], &[]);
    let t = &s.t;
    let rect = b.snap_rect(rect);
    let k = s.k;
    let face = VU_FACE.rgb();
    let ink = PAPER_TEXT.rgb();
    let red = VU_RED.rgb();
    b.rrect(rect, s.r(3.0), Fill::Solid(shade(face, -0.30)));
    let glass = rect.shrink(2.0 * k);
    // Lit from behind: brighter in the middle.
    b.rrect(
        glass,
        s.r(2.0),
        Fill::V(shade(face, -0.06), shade(face, -0.12)),
    );
    let stops = [
        (0.0, with_alpha(t.light, 0.35)),
        (1.0, with_alpha(t.light, 0.0)),
    ];
    b.radial(
        glass,
        glass.center_bottom(),
        glass.size() * Vec2::new(0.7, 1.1),
        &stops,
        8,
    );
    // The arc: its pivot below the face.
    let pivot = Pos2::new(glass.center().x, glass.bottom() + glass.height() * 0.35);
    let radius = glass.height() * 1.0;
    let (a0, a1) = (
        -std::f32::consts::FRAC_PI_2 - 0.78,
        -std::f32::consts::FRAC_PI_2 + 0.78,
    );
    let ang = |p: f32| a0 + (a1 - a0) * p.clamp(0.0, 1.0);
    let zero = vu_position(0.0);
    b.arc(pivot, radius, ang(0.0), ang(zero), 1.2 * k, ink, false);
    b.arc(pivot, radius, ang(zero), ang(1.0), 3.0 * k, red, false);
    for (v, major) in [
        (-20.0, true),
        (-10.0, true),
        (-7.0, false),
        (-5.0, true),
        (-3.0, false),
        (-2.0, false),
        (-1.0, false),
        (0.0, true),
        (1.0, false),
        (2.0, false),
        (3.0, true),
    ] {
        let p = vu_position(v);
        let dir = Vec2::angled(ang(p));
        let len = if major { 6.0 } else { 3.5 } * k;
        let c = if v > 0.0 { red } else { ink };
        b.segment(
            pivot + dir * radius,
            pivot + dir * (radius + len),
            1.0 * k,
            c,
            false,
        );
    }
    // The words: VU in the middle.
    let label = Rect::from_center_size(
        Pos2::new(glass.center().x, glass.bottom() - glass.height() * 0.28),
        Vec2::new(30.0 * k, 12.0 * k),
    );
    let _ = text::paint(
        b,
        label,
        &Text::new("VU", Role::Micro, ink).align(Align2::CENTER_CENTER),
        s.tk(),
    );
    // The needle, and its pivot's cover.
    let dir = Vec2::angled(ang(vu_position(vu)));
    let tip = pivot + dir * (radius + 4.0 * k);
    // From where it shows above the pivot's cover.
    let from = pivot + dir * (radius * 0.42);
    b.segment(from, tip, 1.4 * k, shade(ink, 0.05), true);
    b.rect(
        Rect::from_min_max(Pos2::new(glass.left(), glass.bottom() - 5.0 * k), glass.max),
        Fill::V(shade(face, -0.25), shade(face, -0.35)),
    );
    // The peak lamp, top right.
    let lamp = Pos2::new(glass.right() - 7.0 * k, glass.top() + 7.0 * k);
    if peak {
        super::super::light::glow(b, lamp, 8.0 * k, red, 0.8);
        b.circle(lamp, 2.6 * k, Fill::Solid(shade(red, 0.12)));
    } else {
        b.circle(lamp, 2.6 * k, Fill::Solid(shade(red, -0.25)));
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Pair, Skin};
    use super::*;
    use crate::kit::color::contrast;

    /// The integrator's review of modulation: a look's voices were picked by the theme's
    /// polarity, not the display's (a CRT is dark in Paper too), so the second voice came
    /// out muddy in one theme. For every unit with a look of its own and a second accent —
    /// and a probe sheet in every such look — the second voice is the accent for the
    /// display's polarity (then 3:1 on its ground), the same in Slate and in Paper where
    /// the ground is (CRT, VFD, scope), and a hue apart from the main trace.
    #[test]
    fn a_look_s_voices_follow_the_display_s_polarity_not_the_theme_s() {
        use super::super::{of, Unit};
        let probe = |look: Display| -> &'static Skin {
            Box::leak(Box::new(Skin {
                display: look,
                accent: Some(Pair {
                    dark: Hsl {
                        h: 10.0,
                        s: 0.8,
                        l: 0.65,
                    },
                    light: Hsl {
                        h: 14.0,
                        s: 0.8,
                        l: 0.36,
                    },
                }),
                accent2: Some(Pair {
                    dark: Hsl {
                        h: 200.0,
                        s: 0.8,
                        l: 0.62,
                    },
                    light: Hsl {
                        h: 204.0,
                        s: 0.85,
                        l: 0.26,
                    },
                }),
                ..Skin::plain("test.voices")
            }))
        };
        let mut sheets: Vec<&'static Skin> = Unit::all().into_iter().map(of).collect();
        sheets.extend(
            [
                Display::Phosphor,
                Display::Lcd,
                Display::Vfd,
                Display::Paper,
                Display::Scope,
            ]
            .map(probe),
        );
        let mut bad = Vec::new();
        for skin in sheets {
            let own = !matches!(skin.display, Display::Plain | Display::Glass);
            if !own || skin.accent2.is_none() {
                continue;
            }
            let at = |t: Tokens| ink(&Style::new(t).skinned(skin));
            let (slate, paper) = (
                at(crate::theme::slate().kit_tokens()),
                at(crate::theme::paper().kit_tokens()),
            );
            for (theme, d) in [("slate", slate), ("paper", paper)] {
                let want = ensure_contrast(skin.accent2_in(d.dark).unwrap(), d.ground, 3.0);
                if d.trace2 != want {
                    bad.push(format!(
                        "{} {theme}: the second voice is not the display's",
                        skin.key
                    ));
                }
                let (a, b) = (Hsl::of(d.trace), Hsl::of(d.trace2));
                let g = (a.h - b.h).rem_euclid(360.0);
                if a.s > 0.2 && b.s > 0.2 && g.min(360.0 - g) < 20.0 {
                    bad.push(format!("{} {theme}: the voices share a hue", skin.key));
                }
            }
            if matches!(
                skin.display,
                Display::Phosphor | Display::Vfd | Display::Scope
            ) && (slate.trace2 != paper.trace2 || slate.trace != paper.trace)
            {
                bad.push(format!(
                    "{}: its CRT's voices change with the theme",
                    skin.key
                ));
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// The integrator's review of dynamics: danger words and the open lane, the theme's own
    /// colours, were 1.3–3.5:1 on an LCD, paper or a CRT. In every look and preset, danger
    /// reads at 4.5:1 and ok at 3:1 on the ground (and on a label's plate over it); plain,
    /// they are the theme's.
    #[test]
    fn danger_and_ok_read_on_every_look_s_ground() {
        let mut bad = Vec::new();
        for look in Display::ALL {
            let skin: &'static Skin = Box::leak(Box::new(Skin {
                display: look,
                ..Skin::plain("test.danger")
            }));
            for (key, make) in crate::theme::PRESETS {
                let base = make().kit_tokens();
                let d = ink(&Style::new(base).skinned(skin));
                if look == Display::Plain {
                    assert_eq!((d.danger, d.ok), (base.danger, base.meter_low), "{key}");
                    continue;
                }
                let plate = crate::kit::color::over(d.plate, d.ground);
                for (what, c, bg, need) in [
                    ("danger", d.danger, d.ground, 4.5),
                    ("danger on a plate", d.danger, plate, 4.5),
                    ("ok", d.ok, d.ground, 3.0),
                    ("ok on a plate", d.ok, plate, 3.0),
                ] {
                    let got = contrast(c, bg);
                    if got < need - 0.01 {
                        bad.push(format!("{} {key}: {what} at {got:.2}", look.name()));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// The integrator's review of the instruments: drawing inside a display in the theme's
    /// fills with the display's words gave dark letters on dark nodes (Slate on an LCD) and
    /// light text on white cards (Paper on a scope). `display_style` makes every fill a
    /// shade of the display's ground and its `dark` the display's: every look, every
    /// preset, with and without an accent, keeps every pairing the kit promises — and the
    /// plain look is the theme's style untouched.
    #[test]
    fn a_display_s_style_keeps_every_promise_in_every_look_and_preset() {
        let accent = Pair {
            dark: Hsl {
                h: 50.0,
                s: 0.9,
                l: 0.75,
            },
            light: Hsl {
                h: 220.0,
                s: 0.7,
                l: 0.35,
            },
        };
        let mut bad = Vec::new();
        for look in Display::ALL {
            for a in [None, Some(accent)] {
                let skin: &'static Skin = Box::leak(Box::new(Skin {
                    display: look,
                    accent: a,
                    ..Skin::plain("test.display_style")
                }));
                for (key, make) in crate::theme::PRESETS {
                    let s = Style::new(make().kit_tokens()).skinned(skin);
                    let ds = style(&s);
                    if look == Display::Plain {
                        assert_eq!(ds, s, "the plain look is the theme's style");
                        continue;
                    }
                    let d = ink(&s);
                    if ds.t.dark != d.dark {
                        bad.push(format!("{} {key}: dark is not the display's", look.name()));
                    }
                    for (what, got) in ds.t.legibility_failures() {
                        bad.push(format!(
                            "{} {key} accent {}: {what} at {got:.2}",
                            look.name(),
                            a.is_some()
                        ));
                    }
                    for (what, fg, bg) in [
                        ("text on a card", ds.t.text, ds.t.deck),
                        (
                            "text on a value pill",
                            ds.t.value_pill_text,
                            ds.t.value_pill,
                        ),
                        ("label on a card", ds.t.label, ds.t.deck),
                        ("text on the ground", ds.t.text, d.ground),
                        ("hue on a card", ds.hue, ds.t.deck),
                    ] {
                        let got = contrast(fg, bg);
                        if got < 4.5 {
                            bad.push(format!("{} {key}: {what} at {got:.2}", look.name()));
                        }
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} failures: {:#?}",
            bad.len(),
            &bad[..bad.len().min(12)]
        );
    }

    /// The integrator's findings from the modulation family: a VFD's second voice came out
    /// in its main one's colour, and the second voice was not pushed to contrast on Paper,
    /// Phosphor and Scope. Every look, in every preset, with no accent, one, or two: the
    /// words at AA, the dim words and both voices at 3:1 on the ground, two distinct voices.
    #[test]
    fn every_look_keeps_its_voices_readable_and_apart_in_every_preset() {
        let accent = Pair {
            dark: Hsl {
                h: 32.0,
                s: 0.85,
                l: 0.58,
            },
            light: Hsl {
                h: 28.0,
                s: 0.85,
                l: 0.40,
            },
        };
        let second = Pair {
            dark: Hsl {
                h: 200.0,
                s: 0.7,
                l: 0.6,
            },
            light: Hsl {
                h: 205.0,
                s: 0.7,
                l: 0.38,
            },
        };
        let mut bad = Vec::new();
        for look in Display::ALL {
            for (a, a2) in [
                (None, None),
                (Some(accent), None),
                (Some(accent), Some(second)),
                (None, Some(second)),
            ] {
                let skin: &'static Skin = Box::leak(Box::new(Skin {
                    display: look,
                    accent: a,
                    accent2: a2,
                    ..Skin::plain("test.display")
                }));
                for (key, make) in crate::theme::PRESETS {
                    let s = Style::new(make().kit_tokens()).skinned(skin);
                    let d = ink(&s);
                    let tag = format!(
                        "{look:?} {key} accent {} second {}",
                        a.is_some(),
                        a2.is_some()
                    );
                    for (what, fg, need) in [
                        ("text", d.text, 4.5),
                        ("dim", d.dim, 3.0),
                        ("trace", d.trace, 3.0),
                        ("second voice", d.trace2, 3.0),
                    ] {
                        let got = contrast(fg, d.ground);
                        if got + 1e-3 < need {
                            bad.push(format!("{tag}: {what} at {got:.2}"));
                        }
                    }
                    if d.trace2 == d.trace {
                        bad.push(format!("{tag}: the second voice is the first's colour"));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} failures: {:#?}",
            bad.len(),
            &bad[..bad.len().min(12)]
        );
    }
}

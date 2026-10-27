//! The audio cores as engines (the owner's idea, 2026-10-27): Settings ▸ Audio's *Audio
//! cores* shows, beside each number of cores, an engine and a model it is known from — by
//! the model's name only, never its maker's — with an illustration of the model (an SVG
//! of our own in the theme's colours; no logos, no text). The number of cores stays the
//! value ("12 · V12 · Enzo"); a count between two tiers takes the one below it. The model
//! names are in **one table**, [`TABLE`], so a legal review can swap them in one place.

use crate::i18n::t;
use crate::kit::color::{opaque, Hsl};
use crate::kit::{Brush, Tokens};
use egui::{pos2, Color32, Rect};

/// An engine tier, from a single cylinder to a turbo-ramjet.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Engine {
    Single,
    Twin,
    Inline3,
    V4,
    V6,
    V8,
    V10,
    V12,
    W16,
    Ramjet,
}

/// The table: from how many cores each tier starts, the tier, its stable id (its engine
/// word's locale key: `workflow.engine.<id>`) and the model it shows. The only place the
/// model names are written.
pub const TABLE: [(u32, Engine, &str, &str); 10] = [
    (1, Engine::Single, "single", "Grom"),
    (2, Engine::Twin, "twin", "Scrambler"),
    (3, Engine::Inline3, "inline3", "GR Yaris"),
    (4, Engine::V4, "v4", "Fulvia"),
    (6, Engine::V6, "v6", "GT-R"),
    (8, Engine::V8, "v8", "Mustang"),
    (10, Engine::V10, "v10", "Viper"),
    (12, Engine::V12, "v12", "Enzo"),
    (16, Engine::W16, "w16", "Chiron"),
    (17, Engine::Ramjet, "ramjet", "SR-71 Blackbird"),
];

impl Engine {
    pub const ALL: [Engine; 10] = [
        Engine::Single,
        Engine::Twin,
        Engine::Inline3,
        Engine::V4,
        Engine::V6,
        Engine::V8,
        Engine::V10,
        Engine::V12,
        Engine::W16,
        Engine::Ramjet,
    ];

    /// The tier of `cores` (the audio thread alone, 0, is one core): the last whose start
    /// is at or below the count.
    pub fn of(cores: u32) -> Engine {
        let cores = cores.max(1);
        TABLE
            .iter()
            .rev()
            .find(|(from, ..)| *from <= cores)
            .map_or(Engine::Single, |row| row.1)
    }

    fn row(self) -> &'static (u32, Engine, &'static str, &'static str) {
        TABLE
            .iter()
            .find(|row| row.1 == self)
            .expect("every tier is in the table")
    }

    /// The i18n key of the engine's word ("V12", "single").
    pub fn name_key(self) -> String {
        format!("workflow.engine.{}", self.row().2)
    }

    pub fn name(self) -> String {
        t(&self.name_key())
    }

    /// The model it shows (a proper name, the same in every language).
    pub fn model(self) -> &'static str {
        self.row().3
    }
}

/// "12 · V12 · Enzo": the count, its engine, its model.
pub fn with_engine(count: &str, cores: u32) -> String {
    let e = Engine::of(cores);
    format!("{count} · {} · {}", e.name(), e.model())
}

// ---- the illustrations -----------------------------------------------------------------
//
// Each model is an SVG of our own (`assets/vehicles/`, side profile facing right, drawn
// from photographs of the real thing for its proportions — wheelbase, overhangs, roof,
// glasshouse — never traced). Its colours are **placeholders** (`{paint}`, `{glass}`, …)
// filled from the theme's tokens when it is drawn, so a model keeps its character (the
// red single, the yellow twin, the green fastback) in the theme's own hues, and its
// lightness as designed so it reads on any ground. `{detail}` and `{simple}` are
// opacities: below [`SMALL_PX`] the fine detail (panel lines, spokes, vents) gives way to
// a simpler drawing that still reads at a list row's height. The SVG is rasterised by
// `resvg` at the exact physical size it is shown at, into the kit's layer atlas (ADR
// 0024 §1, §12), and redrawn only when the theme, the size or the model changes.

/// The SVG sources, in [`Engine::ALL`]'s order.
pub const SOURCES: [&str; 10] = [
    include_str!("../assets/vehicles/grom.svg"),
    include_str!("../assets/vehicles/scrambler.svg"),
    include_str!("../assets/vehicles/gr-yaris.svg"),
    include_str!("../assets/vehicles/fulvia.svg"),
    include_str!("../assets/vehicles/gt-r.svg"),
    include_str!("../assets/vehicles/mustang.svg"),
    include_str!("../assets/vehicles/viper.svg"),
    include_str!("../assets/vehicles/enzo.svg"),
    include_str!("../assets/vehicles/chiron.svg"),
    include_str!("../assets/vehicles/sr-71.svg"),
];

/// The drawings' box: `0 0 560 200` (so [`ASPECT`] wide for each unit of height).
pub const VIEW: (f32, f32) = (560.0, 200.0);

/// Below this many physical pixels of height a drawing shows its simple level of detail.
pub const SMALL_PX: u32 = 40;

/// The box a drawing is shown in has this width for each unit of height.
pub const ASPECT: f32 = 2.8;

/// What a drawing is coloured from: the theme's tokens and the ground it sits on (its
/// outline and shadow answer the ground's lightness).
#[derive(Copy, Clone, Debug)]
pub struct Look {
    pub t: Tokens,
    pub ground: Color32,
}

impl Look {
    pub fn of(t: &Tokens, ground: Color32) -> Look {
        Look { t: *t, ground }
    }
}

/// Every placeholder a drawing may use, in the order [`palette`] returns them.
pub const SLOTS: [&str; 20] = [
    "paint_hi",
    "paint_lo",
    "paint_deep",
    "paint",
    "accent_lo",
    "accent",
    "trim_hi",
    "trim",
    "glass_hi",
    "glass",
    "tyre_hi",
    "tyre",
    "rim_hi",
    "rim_lo",
    "rim",
    "chrome",
    "lamp",
    "tail",
    "edge",
    "shadow",
];

impl Engine {
    fn index(self) -> usize {
        Engine::ALL.iter().position(|x| *x == self).unwrap_or(0)
    }

    /// Its drawing's SVG source, with placeholders.
    pub fn source(self) -> &'static str {
        SOURCES[self.index()]
    }
}

/// A colour of the hue of `c` at the designed saturation `s` and lightness `l`: the
/// theme gives the hue, the drawing keeps its depth whatever the theme's own lightness.
fn tone(c: Color32, s: f32, l: f32) -> Color32 {
    Hsl {
        h: Hsl::of(opaque(c)).h,
        s,
        l,
    }
    .rgb()
}

/// The theme's neutral (its panels' hue, nearly grey) at lightness `l`.
fn neutral(t: &Tokens, l: f32) -> Color32 {
    let h = Hsl::of(opaque(t.panel));
    Hsl {
        h: h.h,
        s: h.s.min(0.08),
        l,
    }
    .rgb()
}

/// The colours of `e`'s drawing on `look`, one for each of [`SLOTS`].
pub fn palette(e: Engine, look: &Look) -> [Color32; 20] {
    let t = &look.t;
    // Each model's paint and its second colour, in the theme's hues at a designed
    // lightness: the red single, the yellow twin with its brown seat, the white hatch
    // with red, the dark-blue coupé, the gunmetal coupé with orange brakes, the green
    // fastback, the blue roadster with white stripes, the red berlinetta, the two blues
    // of the hypercar, the black plane with its burners' glow.
    let (paint, accent) = match e {
        Engine::Single => (tone(t.danger, 0.72, 0.5), tone(t.brand_bright, 0.7, 0.55)),
        Engine::Twin => (tone(t.brand_bright, 0.8, 0.54), tone(t.brand, 0.35, 0.26)),
        Engine::Inline3 => (neutral(t, 0.9), tone(t.danger, 0.75, 0.47)),
        Engine::V4 => (tone(t.hue_source, 0.5, 0.33), neutral(t, 0.86)),
        Engine::V6 => (neutral(t, 0.5), tone(t.hue_dynamics, 0.8, 0.52)),
        Engine::V8 => (tone(t.hue_output, 0.4, 0.3), neutral(t, 0.86)),
        Engine::V10 => (tone(t.hue_source, 0.75, 0.42), neutral(t, 0.95)),
        Engine::V12 => (tone(t.danger, 0.82, 0.47), neutral(t, 0.14)),
        Engine::W16 => (tone(t.hue_source, 0.8, 0.47), tone(t.hue_source, 0.5, 0.17)),
        Engine::Ramjet => (
            tone(t.hue_source, 0.12, 0.2),
            tone(t.brand_bright, 0.85, 0.6),
        ),
    };
    let p = Hsl::of(paint);
    let a = Hsl::of(accent);
    let at = |h: Hsl, dl: f32| {
        Hsl {
            l: (h.l + dl).clamp(0.02, 0.97),
            ..h
        }
        .rgb()
    };
    // Glass: dark, a little of the theme's cool hue, never saturated.
    let glass = tone(t.hue_source, 0.22, 0.13);
    let dark_ground = Hsl::of(opaque(look.ground)).l < 0.5;
    [
        at(p, 0.2),
        at(p, -0.12),
        at(p, -0.24),
        paint,
        at(a, -0.14),
        accent,
        neutral(t, if dark_ground { 0.34 } else { 0.3 }),
        neutral(t, if dark_ground { 0.16 } else { 0.1 }),
        Hsl {
            l: 0.8,
            ..Hsl::of(glass)
        }
        .rgb(),
        glass,
        neutral(t, if dark_ground { 0.3 } else { 0.24 }),
        neutral(t, if dark_ground { 0.13 } else { 0.08 }),
        neutral(t, 0.9),
        neutral(t, 0.42),
        neutral(t, 0.68),
        neutral(t, 0.88),
        tone(t.brand_bright, 0.4, 0.9),
        tone(t.danger, 0.85, 0.48),
        if dark_ground {
            neutral(t, 0.82)
        } else {
            neutral(t, 0.18)
        },
        neutral(t, 0.03),
    ]
}

fn hex_of(c: Color32) -> String {
    let [r, g, b, _] = c.to_array();
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// `e`'s SVG with every placeholder filled: its colours from `look`, its detail for a
/// drawing `px_h` physical pixels tall.
pub fn svg(e: Engine, look: &Look, px_h: u32) -> String {
    let colours = palette(e, look);
    let small = px_h < SMALL_PX;
    let mut out = String::with_capacity(e.source().len() + 64);
    let mut rest = e.source();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(len) = rest[open..].find('}') else {
            break;
        };
        let name = &rest[open + 1..open + len];
        match name {
            "detail" => out.push_str(if small { "0" } else { "1" }),
            "simple" => out.push_str(if small { "1" } else { "0" }),
            _ => match SLOTS.iter().position(|s| *s == name) {
                Some(i) => out.push_str(&hex_of(colours[i])),
                None => out.push_str(&rest[open..open + len + 1]),
            },
        }
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out
}

/// Rasterise `svg` into `pix` (its whole size).
pub fn render_into(svg: &str, pix: &mut resvg::tiny_skia::Pixmap) -> bool {
    use resvg::{tiny_skia, usvg};
    let Ok(tree) = usvg::Tree::from_str(svg, &usvg::Options::default()) else {
        return false;
    };
    let size = tree.size();
    let (sx, sy) = (
        pix.width() as f32 / size.width(),
        pix.height() as f32 / size.height(),
    );
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(sx, sy),
        &mut pix.as_mut(),
    );
    true
}

/// The model's illustration in `r` (centred at [`ASPECT`] : 1), facing right, at the
/// exact pixel size it covers, from the layer atlas.
pub fn paint(b: &mut Brush, r: Rect, e: Engine, look: Look) {
    let (w, h) = if r.width() / r.height() > ASPECT {
        (r.height() * ASPECT, r.height())
    } else {
        (r.width(), r.width() / ASPECT)
    };
    let ppp = b.ppp();
    let (pw, ph) = ((w * ppp).round().max(1.0), (h * ppp).round().max(1.0));
    let min = pos2(
        b.snap(r.center().x - w * 0.5),
        b.snap(r.center().y - h * 0.5),
    );
    let rect = Rect::from_min_size(min, egui::vec2(pw / ppp, ph / ppp));
    let colours = palette(e, &look);
    let small = (ph as u32) < SMALL_PX;
    let key = crate::kit::cache::key((
        "cantus-vehicle",
        e.index(),
        colours.map(crate::kit::cache::ckey),
        pw as u32,
        ph as u32,
        small,
    ));
    let uv = b.layer(key, pw as usize, ph as usize, |pix| {
        render_into(&svg(e, &look, ph as u32), pix);
    });
    if let Some(uv) = uv {
        b.layer_quad(rect, uv, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{has, Locale};

    fn looks() -> [Look; 2] {
        let night = crate::theme_family::cantus_night_tokens();
        let paper = Tokens::paper();
        [Look::of(&night, night.menu), Look::of(&paper, paper.menu)]
    }

    /// Every count from 1 to 64 has an engine with a word in both languages, a model and a
    /// drawing; the tiers rise with the count, and a count between two takes the one below.
    #[test]
    fn every_count_has_an_engine_and_a_drawing() {
        let ctx = egui::Context::default();
        let mut last = 0;
        for n in 1..=64u32 {
            let e = Engine::of(n);
            let rank = Engine::ALL.iter().position(|x| *x == e).unwrap();
            assert!(rank >= last, "{n}: tiers rise");
            last = rank;
            for l in Locale::ALL {
                assert!(has(l, &e.name_key()), "{n} {l:?}: {}", e.name_key());
            }
            assert!(!e.model().is_empty());
            let mut drawn = 0;
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                crate::kit::with_brush(ui.painter(), |b| {
                    let before = b.vertex_count();
                    paint(
                        b,
                        Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(34.0, 14.0)),
                        e,
                        looks()[0],
                    );
                    drawn = b.vertex_count() - before;
                });
            });
            out.textures_delta.clear();
            assert!(drawn > 0, "{n}: drawn");
        }
        assert_eq!(Engine::of(0), Engine::Single, "the audio thread alone");
        assert_eq!(Engine::of(4), Engine::V4);
        assert_eq!(Engine::of(5), Engine::V4, "between tiers: the one below");
        assert_eq!(Engine::of(12), Engine::V12);
        assert_eq!(Engine::of(15), Engine::V12);
        assert_eq!(Engine::of(16), Engine::W16);
        assert_eq!(Engine::of(17), Engine::Ramjet);
        assert_eq!(with_engine("12", 12), "12 · V12 · Enzo");
    }

    /// The shapes in `g` filled with SVG's default black: a shape with no `fill` of its own
    /// or from an ancestor (a stroked line without `fill="none"` fills its triangle). No
    /// palette colour is pure black, so a black fill can only be the default.
    fn default_black(g: &resvg::usvg::Group, found: &mut Vec<String>) {
        use resvg::usvg::{Node, Paint};
        for n in g.children() {
            match n {
                Node::Group(g) => default_black(g, found),
                Node::Path(p) => {
                    if let Some(Paint::Color(c)) = p.fill().map(|f| f.paint()) {
                        if (c.red, c.green, c.blue) == (0, 0, 0) {
                            found.push(format!("{:?}", p.data().bounds()));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// The share of `pix` that is drawn (alpha above a quarter).
    fn coverage(pix: &resvg::tiny_skia::Pixmap) -> f32 {
        let drawn = pix.pixels().iter().filter(|p| p.alpha() > 64).count();
        drawn as f32 / pix.pixels().len() as f32
    }

    /// Every model's SVG parses, every placeholder is filled from the theme (none left,
    /// none unknown), it holds no text and no raster image, and it renders something of
    /// a vehicle's size at a list row's height and at the large picture's, in a dark and
    /// a light theme.
    #[test]
    fn every_drawing_parses_fills_its_colours_and_renders() {
        for e in Engine::ALL {
            let src = e.source();
            assert!(src.contains("viewBox=\"0 0 560 200\""), "{e:?}: the box");
            for bad in ["<text", "<image", "<style", "data:"] {
                assert!(!src.contains(bad), "{e:?}: {bad}");
            }
            for look in looks() {
                for (w, h) in [(53u32, 19u32), (106, 38), (235, 84), (470, 168)] {
                    let svg = svg(e, &look, h);
                    assert!(
                        !svg.contains('{') && !svg.contains('}'),
                        "{e:?}: a slot left"
                    );
                    let mut pix = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
                    assert!(render_into(&svg, &mut pix), "{e:?} parses");
                    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
                        .unwrap();
                    let mut black = Vec::new();
                    default_black(tree.root(), &mut black);
                    assert!(
                        black.is_empty(),
                        "{e:?}: filled with the default black: {black:?}"
                    );
                    let c = coverage(&pix);
                    assert!((0.1..0.8).contains(&c), "{e:?} at {w}×{h}: covers {c}");
                }
            }
        }
    }

    /// The colours follow the theme: a dark and a light theme fill the same drawing
    /// differently, and the small drawing differs from the large one only by its detail.
    #[test]
    fn the_colours_come_from_the_theme() {
        let [night, paper] = looks();
        for e in Engine::ALL {
            assert_ne!(svg(e, &night, 100), svg(e, &paper, 100), "{e:?}");
            assert_ne!(
                svg(e, &night, 100),
                svg(e, &night, 20),
                "{e:?}: two details"
            );
            let fill =
                hex_of(palette(e, &night)[SLOTS.iter().position(|s| *s == "paint").unwrap()]);
            assert!(
                svg(e, &night, 100).contains(&fill),
                "{e:?}: its paint is used"
            );
            for look in [night, paper] {
                assert!(
                    !palette(e, &look).contains(&Color32::BLACK),
                    "{e:?}: pure black is kept for the default-fill check"
                );
            }
        }
    }
}

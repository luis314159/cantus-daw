//! The mixer strip's and the inspector's glyphs — set 3 of the icon family (round 39,
//! `icons`, approved by the owner on 2026-11-09; `spec-mixer.md`).
//!
//! Each glyph is drawn for each device size des-mixer places it at (12–30 px), never one
//! drawing scaled: a size is picked from what the designer drew ([`size_for`]), rasterised
//! once per (glyph, size, inks) into the layer atlas and shown as one textured quad on the
//! device's pixel grid — what the sheet shows is what the screen shows, to the pixel. At a
//! scale between the drawn ones (125 %) the nearest drawing that fits is used, centred.
//!
//! **Inks** (`spec-mixer.md`): *quiet* (a bare glyph at rest: ink 3, ink 4), *rest* in a
//! well (ink 2, ink 3), *hover* (ink 1, ink 2), *on* (the state's colour, and that colour
//! mixed 45 % into the well), *off* (ink 4).

use super::brush::Brush;
use super::color::{mix, opaque};
use super::icons::Inks;
use super::Style;
use egui::{Color32, Pos2, Rect};

#[path = "glyph_mixer_data.rs"]
mod data;

/// A glyph of the set.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MGlyph {
    Phase,
    Swap,
    Mono,
    Link,
    Arm,
    Enable,
    Fx,
    Send,
    Out,
    Into,
    Ret,
    Key,
    Routing,
    Freeze,
    Vca,
    Lamp,
    LampOff,
    Grip,
    MiniView,
    More,
    Add,
    FoldOpen,
    FoldShut,
}

impl MGlyph {
    pub const ALL: [MGlyph; 23] = [
        MGlyph::Phase,
        MGlyph::Swap,
        MGlyph::Mono,
        MGlyph::Link,
        MGlyph::Arm,
        MGlyph::Enable,
        MGlyph::Fx,
        MGlyph::Send,
        MGlyph::Out,
        MGlyph::Into,
        MGlyph::Ret,
        MGlyph::Key,
        MGlyph::Routing,
        MGlyph::Freeze,
        MGlyph::Vca,
        MGlyph::Lamp,
        MGlyph::LampOff,
        MGlyph::Grip,
        MGlyph::MiniView,
        MGlyph::More,
        MGlyph::Add,
        MGlyph::FoldOpen,
        MGlyph::FoldShut,
    ];

    /// Its name in the design source.
    pub fn name(self) -> &'static str {
        match self {
            MGlyph::Phase => "phase",
            MGlyph::Swap => "swap",
            MGlyph::Mono => "mono",
            MGlyph::Link => "link",
            MGlyph::Arm => "arm",
            MGlyph::Enable => "enable",
            MGlyph::Fx => "fx",
            MGlyph::Send => "send",
            MGlyph::Out => "out",
            MGlyph::Into => "into",
            MGlyph::Ret => "ret",
            MGlyph::Key => "key",
            MGlyph::Routing => "routing",
            MGlyph::Freeze => "freeze",
            MGlyph::Vca => "vca",
            MGlyph::Lamp => "lamp",
            MGlyph::LampOff => "lampOff",
            MGlyph::Grip => "grip",
            MGlyph::MiniView => "miniView",
            MGlyph::More => "more",
            MGlyph::Add => "add",
            MGlyph::FoldOpen => "foldOpen",
            MGlyph::FoldShut => "foldShut",
        }
    }
}

/// The device sizes `g` is drawn at, ascending.
pub fn sizes(g: MGlyph) -> impl Iterator<Item = u16> {
    let n = g.name();
    data::DRAWINGS
        .iter()
        .filter(move |(name, _, _)| *name == n)
        .map(|(_, s, _)| *s)
}

/// The drawing of `g` at `size` device pixels.
pub fn drawing(g: MGlyph, size: u16) -> Option<&'static str> {
    let n = g.name();
    data::DRAWINGS
        .iter()
        .find(|(name, s, _)| *name == n && *s == size)
        .map(|(_, _, svg)| *svg)
}

/// The drawing to use for a glyph des-mixer places at `px` device pixels: that size when
/// it is drawn, else the largest drawn below it, else the smallest drawn.
pub fn size_for(g: MGlyph, px: f32) -> Option<u16> {
    let want = px.round() as i32;
    let mut best: Option<u16> = None;
    let mut least: Option<u16> = None;
    for s in sizes(g) {
        if i32::from(s) <= want && best.is_none_or(|b| s > b) {
            best = Some(s);
        }
        if least.is_none_or(|l| s < l) {
            least = Some(s);
        }
    }
    best.or(least)
}

/// How a glyph is inked.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ink {
    /// A bare glyph at rest, no well: ink 3 and ink 4.
    Quiet,
    /// In a well at rest: ink 2 and ink 3.
    Rest,
    /// Under the pointer: ink 1 and ink 2.
    Hover,
    /// On, in `Color32` (the accent, `play`, `rec`): that colour and it mixed into the well.
    On(Color32),
    /// Unavailable or bypassed: ink 4.
    Off,
    /// One colour for both inks (a glyph set in a state's own colour, as the snowflake).
    Solid(Color32),
}

/// The fourth ink: Slate's quietest (`#47515b` Night, `#a9b3bd` Paper), derived from the
/// placeholder ink and the panel so every theme family has one.
pub fn ink4(s: &Style) -> Color32 {
    let t = &s.t;
    mix(t.placeholder, t.panel, 0.5)
}

/// The inks of `ink` on a ground of `well`.
pub fn inks(s: &Style, ink: Ink, well: Color32) -> Inks {
    let t = &s.t;
    let i4 = ink4(s);
    let (main, second) = match ink {
        Ink::Quiet => (t.placeholder, i4),
        Ink::Rest => (t.text_dim, t.placeholder),
        Ink::Hover => (t.text, t.text_dim),
        Ink::On(c) => (c, mix(c, well, 0.45)),
        Ink::Off => (i4, i4),
        Ink::Solid(c) => (c, c),
    };
    Inks {
        main: opaque(main),
        second: opaque(second),
        rec: opaque(t.danger),
        cut: opaque(well),
    }
}

fn raster(doc: &str, pix: &mut tiny_skia::Pixmap) {
    use resvg::usvg;
    let Ok(tree) = usvg::Tree::from_str(doc, &usvg::Options::default()) else {
        return;
    };
    let Some(mut out) = resvg::tiny_skia::Pixmap::new(pix.width(), pix.height()) else {
        return;
    };
    resvg::render(&tree, Default::default(), &mut out.as_mut());
    pix.data_mut().copy_from_slice(out.data());
}

/// Paint `g`, des-mixer's `size` points across, centred at `c`, in `inks`. The canvas sits
/// on whole device pixels (the designer's half pixels land where drawn). Returns the
/// canvas's rect, or `None` when nothing was painted.
pub fn paint(
    b: &mut Brush,
    c: Pos2,
    size: f32,
    g: MGlyph,
    inks: &Inks,
    alpha: f32,
) -> Option<Rect> {
    let ppp = b.ppp();
    let px = size_for(g, size * ppp)?;
    let body = drawing(g, px)?;
    let side = f32::from(px) / ppp;
    let snap = |v: f32| (v * ppp).round() / ppp;
    let at = Rect::from_min_size(
        egui::pos2(snap(c.x - side * 0.5), snap(c.y - side * 0.5)),
        egui::Vec2::splat(side),
    );
    let id = super::cache::key(("mglyph39", g.name(), px, *inks));
    let n = usize::from(px);
    let uv = b.layer(id, n, n, |pix| {
        raster(&super::icons::document(body, px, inks), pix)
    })?;
    b.layer_quad(at, uv, alpha);
    Some(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every glyph of set 3 is drawn at every size the spec places it at, parses, inks
    /// something, and names its inks.
    #[test]
    fn every_glyph_of_set_3_is_drawn_at_its_sizes_and_parses() {
        let t = crate::theme_slate::slate_night_tokens();
        let ink = Inks {
            main: t.text,
            second: t.text_dim,
            rec: t.danger,
            cut: t.well,
        };
        let spec: &[(MGlyph, &[u16])] = &[
            (MGlyph::Phase, &[12, 16, 18, 24, 27]),
            (MGlyph::Swap, &[12, 16, 18, 24, 27]),
            (MGlyph::Mono, &[12, 16, 18, 24, 27]),
            (MGlyph::Arm, &[14, 16, 18, 21, 24, 27]),
            (MGlyph::Enable, &[12, 14, 16, 18, 21, 24]),
            (MGlyph::Key, &[12, 14, 18, 21]),
            (MGlyph::Freeze, &[12, 14, 18, 21]),
            (MGlyph::Routing, &[16, 20, 24, 30]),
            (MGlyph::Lamp, &[10, 15]),
            (MGlyph::FoldOpen, &[8, 10, 12, 15]),
        ];
        for (g, want) in spec {
            let have: Vec<u16> = sizes(*g).collect();
            assert_eq!(&have, want, "{g:?}");
        }
        for g in MGlyph::ALL {
            assert!(sizes(g).count() >= 2, "{g:?}: a 100 % and a 150 % drawing");
            for s in sizes(g) {
                let doc = super::super::icons::document(drawing(g, s).unwrap(), s, &ink);
                assert!(!doc.contains("var("), "{g:?} {s}: an ink left unnamed");
                let mut pix = tiny_skia::Pixmap::new(s.into(), s.into()).unwrap();
                raster(&doc, &mut pix);
                let inked = pix.pixels().iter().filter(|p| p.alpha() > 0).count();
                assert!(inked >= 4, "{g:?} {s}: {inked} pixels inked");
            }
        }
    }

    #[test]
    fn a_place_takes_its_drawn_size_or_the_largest_below() {
        assert_eq!(size_for(MGlyph::Phase, 16.0), Some(16));
        assert_eq!(size_for(MGlyph::Phase, 24.0), Some(24));
        // 125 %: 16 pt is 20 px — the 18-px drawing, never a scaled one.
        assert_eq!(size_for(MGlyph::Phase, 20.0), Some(18));
        assert_eq!(size_for(MGlyph::Lamp, 8.0), Some(10));
    }
}

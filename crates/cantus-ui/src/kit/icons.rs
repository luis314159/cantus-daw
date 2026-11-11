//! The icon family on a key (round 39, `icons`' second pass, approved 2026-11-09): each
//! glyph drawn for the key height it sits in — 16, 20, 24, 30 and 36 device pixels — never
//! one drawing scaled (ADR 0108).
//!
//! The drawings ([`super::icons_data`]) are the designer's own, made on the key's pixel
//! grid: straight edges on whole pixels, odd strokes on half pixels, some set pixel by
//! pixel. They are rasterised once per (glyph, size, inks) into the layer atlas and shown as
//! one textured quad on the key's pixel grid, so what the design sheet shows is what the
//! screen shows, to the pixel.
//!
//! **Inks per state** (`family.js`): at rest the glyph is ink 2 with ink 3 as its second
//! ink; under the pointer ink 1 and ink 2; on, the accent (green for play, red for record)
//! with the accent mixed 45 % into the well as its second ink; unavailable, ink 4.

use super::brush::Brush;
use super::color::{mix, opaque};
use super::glyph::Glyph;
use super::{State, Style};
use egui::{Color32, Rect};

/// The key heights each glyph is drawn for, in device pixels.
pub const SIZES: [u16; 5] = [16, 20, 24, 30, 36];

/// The family's name of a kit glyph, where the family draws it.
pub fn name(g: Glyph) -> Option<&'static str> {
    // r39:el-controls — the whole map (set 5, spec-rest.md), where a drawing exists.
    let family = super::glyph::family_name(g);
    if SIZES.iter().all(|&k| drawing(family, k).is_some()) {
        return Some(family);
    }
    Some(match g {
        Glyph::Play => "play",
        Glyph::Pause => "pause",
        Glyph::Stop => "stop",
        Glyph::Record => "record",
        Glyph::ToStart => "toStart",
        Glyph::Loop => "loop",
        Glyph::Metronome => "metronome",
        Glyph::Magnet => "snap",
        Glyph::Undo => "undo",
        Glyph::Redo => "redo",
        Glyph::Gear => "gear",
        Glyph::Pick => "pick",
        Glyph::Zoom => "zoom",
        Glyph::Arrangement => "arrangement",
        Glyph::PianoRoll => "pianoRoll",
        Glyph::Rack => "rack",
        Glyph::Mixer => "mixer",
        Glyph::Browser => "browser",
        Glyph::Inspector => "inspector",
        Glyph::Device => "plugins",
        _ => return None,
    })
}

/// The drawing of `name` for a key `size` device pixels high.
pub fn drawing(name: &str, size: u16) -> Option<&'static str> {
    super::icons_data::DRAWINGS
        .iter()
        .find(|(n, s, _)| *n == name && *s == size)
        .map(|(_, _, svg)| *svg)
        .or_else(|| super::glyph::rest_drawing(name, size)) // r39:el-controls
}

/// The largest drawn key height that fits `px` device pixels.
pub fn size_for(px: f32) -> Option<u16> {
    SIZES
        .iter()
        .rev()
        .copied()
        .find(|s| f32::from(*s) <= px + 0.5)
}

/// A key's inks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Inks {
    pub main: Color32,
    pub second: Color32,
    pub rec: Color32,
    /// The key's own well: what a cut out of a solid shows.
    pub cut: Color32,
}

/// What a key in `st` inks its glyph with, on its `well`; `on` is the colour of its on
/// state (the accent, or green for play and red for record).
pub fn inks(s: &Style, st: State, on: Color32, well: Color32) -> Inks {
    let t = &s.t;
    let (main, second) = if st.disabled {
        (t.placeholder, t.placeholder)
    } else if st.lit {
        (on, mix(on, well, 0.45))
    } else if st.hovered || st.active {
        (t.text, t.text_dim)
    } else {
        (t.text_dim, t.placeholder)
    };
    Inks {
        main: opaque(main),
        second: opaque(second),
        rec: opaque(t.danger),
        cut: opaque(well),
    }
}

fn css(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// The whole SVG document of a drawing in `inks`, on a `size`-pixel canvas.
pub fn document(body: &str, size: u16, inks: &Inks) -> String {
    let (m, s2, r, c) = (
        css(inks.main),
        css(inks.second),
        css(inks.rec),
        css(inks.cut),
    );
    let body = body
        .replace("var(--main)", &m)
        .replace("var(--second)", &s2)
        .replace("var(--rec)", &r)
        .replace("var(--cut)", &c);
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{size}' height='{size}' \
         viewBox='0 0 {size} {size}'><style>.f1{{fill:{m}}}.f2{{fill:{s2}}}.fr{{fill:{r}}}\
         .s1{{fill:none;stroke:{m}}}.s2{{fill:none;stroke:{s2}}}.cut{{fill:{c}}}\
         .scut{{fill:none;stroke:{c}}}</style>{body}</svg>"
    )
}

/// Rasterise a document into `pix` (premultiplied RGBA).
pub(crate) fn raster(doc: &str, pix: &mut tiny_skia::Pixmap) {
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

/// Paint glyph `g` in `key` (its rect in points) in `inks`, from the family's drawing for
/// the key's height on the device. Returns false when the family has no drawing for it (the
/// caller paints the kit's own glyph instead).
pub fn paint(b: &mut Brush, key: Rect, g: Glyph, inks: &Inks, alpha: f32) -> bool {
    let Some(name) = name(g) else {
        return false;
    };
    paint_name(b, key, name, inks, alpha)
}

/// [`paint`] by the family's own name of a drawing (r39:lo-arrange — the tools and Follow,
/// which the kit's `Glyph` does not list: "select", "draw", "slice", "erase", "muteTool",
/// "zoom", "follow"). Returns false when the family has no drawing of that name.
pub fn paint_name(b: &mut Brush, key: Rect, name: &'static str, inks: &Inks, alpha: f32) -> bool {
    let ppp = b.ppp();
    let Some(size) = size_for(key.height() * ppp) else {
        return false;
    };
    let Some(body) = drawing(name, size) else {
        return false;
    };
    let side = f32::from(size) / ppp;
    // The canvas centred in the key, on whole device pixels.
    let snap = |v: f32| (v * ppp).round() / ppp;
    let x = snap(key.center().x - side * 0.5);
    let y = snap(key.center().y - side * 0.5);
    let at = Rect::from_min_size(egui::pos2(x, y), egui::Vec2::splat(side));
    let id = super::cache::key(("icon39", name, size, *inks));
    let px = usize::from(size);
    let Some(uv) = b.layer(id, px, px, |pix| raster(&document(body, size, inks), pix)) else {
        return false;
    };
    b.layer_quad(at, uv, alpha);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_glyph_is_drawn_at_every_size_and_parses() {
        let t = crate::theme_slate::slate_night_tokens();
        let ink = Inks {
            main: t.text,
            second: t.text_dim,
            rec: t.danger,
            cut: t.well,
        };
        for g in Glyph::ALL.iter() {
            let Some(n) = name(*g) else { continue };
            for s in SIZES {
                let body = drawing(n, s).unwrap_or_else(|| panic!("{n} at {s} px"));
                let doc = document(body, s, &ink);
                assert!(!doc.contains("var("), "{n} {s}: an ink left unnamed");
                let tree = resvg::usvg::Tree::from_str(&doc, &Default::default())
                    .unwrap_or_else(|e| panic!("{n} {s}: {e}"));
                assert!(tree.root().has_children(), "{n} {s}: draws nothing");
                let mut pix = tiny_skia::Pixmap::new(s.into(), s.into()).unwrap();
                raster(&doc, &mut pix);
                let inked = pix.pixels().iter().filter(|p| p.alpha() > 0).count();
                assert!(inked > 4, "{n} {s}: {inked} pixels inked");
            }
        }
    }

    #[test]
    fn a_key_takes_the_largest_drawing_that_fits() {
        assert_eq!(size_for(15.0), None);
        assert_eq!(size_for(16.0), Some(16));
        assert_eq!(size_for(23.6), Some(24));
        assert_eq!(size_for(29.0), Some(24));
        assert_eq!(size_for(36.0), Some(36));
        assert_eq!(size_for(48.0), Some(36));
    }
}

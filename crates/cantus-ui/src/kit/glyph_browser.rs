//! The browser's glyphs (round 39, the icon family's set 4, `docs/round39/design/icons/`,
//! `spec-browser.md`): folder and open folder, file, audio, MIDI, preset, instrument and
//! effect, pack, star and favourite, search, history, clear, the sounding speaker, the
//! installed check and download — each drawn by its designer for every size it is shown
//! at (12, 14, 16 at 100 %; 18, 21, 24 at 150 %), never one drawing scaled (ADR 0108 §3).
//!
//! As [`super::icons`] does for the keys: each (glyph, size, inks) is rasterised once into
//! the layer atlas and drawn as one quad on the device's pixel grid. Bare glyphs in a row
//! take one ink (`main`) with a quieter second; the table at the bottom is generated from
//! the design source (`glyphs-b-browser*.js`, evaluated with the sheets' own helpers) —
//! edit the design, not the table.

use super::icons::{document, Inks};
use super::{Brush, Style};
use egui::{Color32, Rect};

/// A glyph of the browser set.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    Folder,
    FolderOpen,
    File,
    Audio,
    Midi,
    Preset,
    Instrument,
    Effect,
    Pack,
    Star,
    StarFilled,
    Search,
    History,
    Clear,
    Speaker,
    Check,
    Download,
}

impl Glyph {
    pub const ALL: [Glyph; 17] = [
        Glyph::Folder,
        Glyph::FolderOpen,
        Glyph::File,
        Glyph::Audio,
        Glyph::Midi,
        Glyph::Preset,
        Glyph::Instrument,
        Glyph::Effect,
        Glyph::Pack,
        Glyph::Star,
        Glyph::StarFilled,
        Glyph::Search,
        Glyph::History,
        Glyph::Clear,
        Glyph::Speaker,
        Glyph::Check,
        Glyph::Download,
    ];

    /// The design source's name.
    pub fn name(self) -> &'static str {
        match self {
            Glyph::Folder => "folder",
            Glyph::FolderOpen => "folderOpen",
            Glyph::File => "file",
            Glyph::Audio => "audio",
            Glyph::Midi => "midi",
            Glyph::Preset => "preset",
            Glyph::Instrument => "instrument",
            Glyph::Effect => "effect",
            Glyph::Pack => "pack",
            Glyph::Star => "star",
            Glyph::StarFilled => "starFilled",
            Glyph::Search => "search",
            Glyph::History => "history",
            Glyph::Clear => "clear",
            Glyph::Speaker => "speaker",
            Glyph::Check => "check",
            Glyph::Download => "download",
        }
    }
}

/// The drawn sizes of `g`, smallest first.
pub fn sizes(g: Glyph) -> impl Iterator<Item = u16> {
    DRAWINGS
        .iter()
        .filter(move |(n, _, _)| *n == g.name())
        .map(|(_, s, _)| *s)
}

/// The drawing of `g` for the largest size that fits `px` device pixels (the smallest
/// when none does).
pub fn drawing_for(g: Glyph, px: f32) -> Option<(u16, &'static str)> {
    let fit = DRAWINGS
        .iter()
        .filter(|(n, s, _)| *n == g.name() && f32::from(*s) <= px + 0.5)
        .max_by_key(|(_, s, _)| *s);
    let any = DRAWINGS
        .iter()
        .filter(|(n, _, _)| *n == g.name())
        .min_by_key(|(_, s, _)| *s);
    fit.or(any).map(|(_, s, body)| (*s, *body))
}

/// The inks of a bare glyph in `main`, its second ink a quieter step of it.
pub fn inks(main: Color32, s: &Style) -> Inks {
    let second = super::color::mix(main, s.t.panel, 0.45);
    Inks {
        main,
        second,
        rec: s.t.danger,
        cut: s.t.panel,
    }
}

/// Paint `g` centred in `rect` in `main`. The drawing is the one made for the rect's
/// height on the device. Returns false when nothing could be drawn.
pub fn paint(b: &mut Brush, rect: Rect, g: Glyph, main: Color32) -> bool {
    let s = Style::current();
    paint_inks(b, rect, g, &inks(main, &s))
}

/// [`paint`] with the inks given.
pub fn paint_inks(b: &mut Brush, rect: Rect, g: Glyph, inks: &Inks) -> bool {
    let ppp = b.ppp();
    let Some((size, body)) = drawing_for(g, rect.height().min(rect.width()) * ppp) else {
        return false;
    };
    let side = f32::from(size) / ppp;
    let snap = |v: f32| (v * ppp).round() / ppp;
    let at = Rect::from_min_size(
        egui::pos2(
            snap(rect.center().x - side * 0.5),
            snap(rect.center().y - side * 0.5),
        ),
        egui::Vec2::splat(side),
    );
    let id = super::cache::key(("glyph_browser", g.name(), size, *inks));
    let px = usize::from(size);
    let Some(uv) = b.layer(id, px, px, |pix| raster(&document(body, size, inks), pix)) else {
        return false;
    };
    b.layer_quad(at, uv, 1.0);
    true
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    /// Every glyph of the set is drawn at its 100 % and its 150 % sizes, and every drawing
    /// parses and inks pixels.
    #[test]
    fn every_browser_glyph_is_drawn_at_both_scales_and_parses() {
        let t = crate::theme_slate::slate_night_tokens();
        let ink = Inks {
            main: t.text,
            second: t.text_dim,
            rec: t.danger,
            cut: t.panel,
        };
        for g in Glyph::ALL {
            let sizes: Vec<u16> = sizes(g).collect();
            assert!(sizes.iter().any(|s| *s <= 16), "{g:?}: a 100 % size");
            assert!(sizes.iter().any(|s| *s >= 15), "{g:?}: a 150 % size");
            for size in sizes {
                let (_, body) = drawing_for(g, f32::from(size)).unwrap();
                let doc = document(body, size, &ink);
                let mut pix = tiny_skia::Pixmap::new(u32::from(size), u32::from(size)).unwrap();
                raster(&doc, &mut pix);
                let inked = pix.data().chunks(4).filter(|p| p[3] > 0).count();
                assert!(inked > 4, "{g:?} at {size}: {inked} pixels");
            }
        }
    }

    /// A size is never scaled: the drawing picked is the one made for the pixels at hand.
    #[test]
    fn the_drawing_picked_is_the_one_made_for_the_size() {
        assert_eq!(drawing_for(Glyph::Folder, 12.0).map(|d| d.0), Some(12));
        assert_eq!(drawing_for(Glyph::Folder, 18.0).map(|d| d.0), Some(18));
        assert_eq!(drawing_for(Glyph::Folder, 20.0).map(|d| d.0), Some(18));
        assert_eq!(drawing_for(Glyph::Folder, 6.0).map(|d| d.0), Some(12));
    }
}

/// (name, size in device pixels, SVG body on a canvas of that size). Generated.
pub const DRAWINGS: &[(&str, u16, &str)] = &[
    (
        "folder",
        12,
        r##"<path class="s1" stroke-width="1" d="M1.5 2.5 H4.5 L5.5 3.5 H10.5 V9.5 H1.5 Z M2 4.5 H10"/>"##,
    ),
    (
        "folder",
        14,
        r##"<path class="s1" stroke-width="1" d="M1.5 3.5 H5.5 L6.5 4.5 H12.5 V11.5 H1.5 Z M2 5.5 H12"/>"##,
    ),
    (
        "folder",
        18,
        r##"<path class="s1" stroke-width="1" d="M2.5 4.5 H7.5 L8.5 5.5 H15.5 V14.5 H2.5 Z M3 7.5 H15"/>"##,
    ),
    (
        "folder",
        21,
        r##"<path class="s1" stroke-width="1" d="M2.5 5.5 H8.5 L9.5 6.5 H18.5 V16.5 H2.5 Z M3 8.5 H18"/>"##,
    ),
    (
        "folderOpen",
        12,
        r##"<path class="s1" stroke-width="1" d="M1.5 9.5 V2.5 H4.5 L5.5 3.5 H9.5 V5"/><path class="s1" stroke-width="1" d="M1.5 9.5 L3 5.5 H11.5 L10 9.5 Z"/>"##,
    ),
    (
        "folderOpen",
        14,
        r##"<path class="s1" stroke-width="1" d="M1.5 11.5 V3.5 H5.5 L6.5 4.5 H11.5 V6.5"/><path class="s1" stroke-width="1" d="M1.5 11.5 L3.5 6.5 H13.5 L11.5 11.5 Z"/>"##,
    ),
    (
        "folderOpen",
        18,
        r##"<path class="s1" stroke-width="1" d="M2.5 14.5 V4.5 H7.5 L8.5 5.5 H14.5 V8.5"/><path class="s1" stroke-width="1" d="M2.5 14.5 L5 8.5 H16.5 L14 14.5 Z"/>"##,
    ),
    (
        "folderOpen",
        21,
        r##"<path class="s1" stroke-width="1" d="M2.5 16.5 V5.5 H8.5 L9.5 6.5 H16.5 V9.5"/><path class="s1" stroke-width="1" d="M2.5 16.5 L5.5 9.5 H19.5 L16.5 16.5 Z"/>"##,
    ),
    (
        "file",
        12,
        r##"<path class="s1" stroke-width="1" d="M2.5 1.5 H7 L9.5 4 V10.5 H2.5 Z"/><path class="s2" stroke-width="1" d="M7 1.5 V4 H9.5"/>"##,
    ),
    (
        "file",
        14,
        r##"<path class="s1" stroke-width="1" d="M3.5 1.5 H8.5 L11.5 4.5 V12.5 H3.5 Z"/><path class="s2" stroke-width="1" d="M8.5 1.5 V4.5 H11.5"/>"##,
    ),
    (
        "file",
        18,
        r##"<path class="s1" stroke-width="1" d="M4.5 2.5 H10.5 L14.5 6.5 V15.5 H4.5 Z"/><path class="s2" stroke-width="1" d="M10.5 2.5 V6.5 H14.5"/>"##,
    ),
    (
        "file",
        21,
        r##"<path class="s1" stroke-width="1" d="M5.5 2.5 H12.5 L16.5 6.5 V18.5 H5.5 Z"/><path class="s2" stroke-width="1" d="M12.5 2.5 V6.5 H16.5"/>"##,
    ),
    (
        "audio",
        12,
        r##"<rect class="f1" x="1" y="5" width="1" height="2"/><rect class="f1" x="3" y="3" width="1" height="6"/><rect class="f1" x="5" y="1" width="1" height="10"/><rect class="f1" x="7" y="3" width="1" height="6"/><rect class="f1" x="9" y="4" width="1" height="4"/><rect class="f1" x="11" y="5" width="1" height="2"/>"##,
    ),
    (
        "audio",
        14,
        r##"<rect class="f1" x="2" y="5" width="1" height="4"/><rect class="f1" x="4" y="3" width="1" height="8"/><rect class="f1" x="6" y="1" width="1" height="12"/><rect class="f1" x="8" y="4" width="1" height="6"/><rect class="f1" x="10" y="3" width="1" height="8"/><rect class="f1" x="12" y="6" width="1" height="2"/>"##,
    ),
    (
        "audio",
        16,
        r##"<rect class="f1" x="2" y="7" width="1" height="2"/><rect class="f1" x="4" y="5" width="1" height="6"/><rect class="f1" x="6" y="3" width="1" height="10"/><rect class="f1" x="8" y="2" width="1" height="12"/><rect class="f1" x="10" y="5" width="1" height="6"/><rect class="f1" x="12" y="4" width="1" height="8"/><rect class="f1" x="14" y="7" width="1" height="2"/>"##,
    ),
    (
        "audio",
        18,
        r##"<rect class="f1" x="3" y="7" width="1" height="4"/><rect class="f1" x="5" y="5" width="1" height="8"/><rect class="f1" x="7" y="3" width="1" height="12"/><rect class="f1" x="9" y="2" width="1" height="14"/><rect class="f1" x="11" y="5" width="1" height="8"/><rect class="f1" x="13" y="4" width="1" height="10"/><rect class="f1" x="15" y="7" width="1" height="4"/>"##,
    ),
    (
        "audio",
        21,
        r##"<rect class="f1" x="3" y="8" width="1" height="4"/><rect class="f1" x="5" y="6" width="1" height="8"/><rect class="f1" x="7" y="4" width="1" height="12"/><rect class="f1" x="9" y="2" width="1" height="16"/><rect class="f1" x="11" y="5" width="1" height="10"/><rect class="f1" x="13" y="4" width="1" height="12"/><rect class="f1" x="15" y="7" width="1" height="6"/><rect class="f1" x="17" y="9" width="1" height="2"/>"##,
    ),
    (
        "audio",
        24,
        r##"<rect class="f1" x="4" y="10" width="1" height="4"/><rect class="f1" x="6" y="8" width="1" height="8"/><rect class="f1" x="8" y="6" width="1" height="12"/><rect class="f1" x="10" y="4" width="1" height="16"/><rect class="f1" x="12" y="3" width="1" height="18"/><rect class="f1" x="14" y="7" width="1" height="10"/><rect class="f1" x="16" y="5" width="1" height="14"/><rect class="f1" x="18" y="9" width="1" height="6"/><rect class="f1" x="20" y="11" width="1" height="2"/>"##,
    ),
    (
        "midi",
        12,
        r##"<rect class="f1" x="4" y="2" width="7" height="1"/><rect class="f1" x="4" y="3" width="1" height="6"/><rect class="f1" x="10" y="3" width="1" height="5"/><rect class="f1" x="2" y="8" width="3" height="2" rx="0.5"/><rect class="f1" x="8" y="7" width="3" height="2" rx="0.5"/>"##,
    ),
    (
        "midi",
        14,
        r##"<rect class="f1" x="5" y="2" width="8" height="1"/><rect class="f1" x="5" y="3" width="1" height="7"/><rect class="f1" x="12" y="3" width="1" height="6"/><rect class="f1" x="2" y="9" width="4" height="2" rx="0.8"/><rect class="f1" x="9" y="8" width="4" height="2" rx="0.8"/>"##,
    ),
    (
        "midi",
        16,
        r##"<rect class="f1" x="6" y="2" width="8" height="2"/><rect class="f1" x="6" y="4" width="1" height="8"/><rect class="f1" x="13" y="4" width="1" height="7"/><rect class="f1" x="2" y="11" width="5" height="3" rx="1.2"/><rect class="f1" x="9" y="10" width="5" height="3" rx="1.2"/>"##,
    ),
    (
        "midi",
        18,
        r##"<rect class="f1" x="7" y="3" width="9" height="2"/><rect class="f1" x="7" y="5" width="1" height="8"/><rect class="f1" x="15" y="5" width="1" height="7"/><rect class="f1" x="3" y="12" width="5" height="3" rx="1.2"/><rect class="f1" x="11" y="11" width="5" height="3" rx="1.2"/>"##,
    ),
    (
        "midi",
        21,
        r##"<rect class="f1" x="8" y="3" width="10" height="2"/><rect class="f1" x="8" y="5" width="1" height="10"/><rect class="f1" x="17" y="5" width="1" height="9"/><rect class="f1" x="3" y="14" width="6" height="3" rx="1.4"/><rect class="f1" x="12" y="13" width="6" height="3" rx="1.4"/>"##,
    ),
    (
        "midi",
        24,
        r##"<rect class="f1" x="9" y="4" width="11" height="2"/><rect class="f1" x="9" y="6" width="1" height="11"/><rect class="f1" x="19" y="6" width="1" height="10"/><rect class="f1" x="4" y="16" width="6" height="4" rx="1.8"/><rect class="f1" x="14" y="15" width="6" height="4" rx="1.8"/>"##,
    ),
    (
        "preset",
        12,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="6" y="5" width="1" height="1"/><rect x="7" y="4" width="1" height="1"/><rect x="8" y="3" width="1" height="1"/></g><g shape-rendering="crispEdges" transform="translate(2,2)"><rect x="2" y="0" width="4" height="1" fill="var(--main)"/><rect x="1" y="1" width="1" height="1" fill="var(--main)"/><rect x="6" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="7" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="7" y="3" width="1" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/><rect x="7" y="4" width="1" height="1" fill="var(--main)"/><rect x="0" y="5" width="1" height="1" fill="var(--main)"/><rect x="7" y="5" width="1" height="1" fill="var(--main)"/><rect x="1" y="6" width="1" height="1" fill="var(--main)"/><rect x="6" y="6" width="1" height="1" fill="var(--main)"/><rect x="2" y="7" width="4" height="1" fill="var(--main)"/></g>"##,
    ),
    (
        "preset",
        16,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="8" y="7" width="1" height="1"/><rect x="9" y="6" width="1" height="1"/><rect x="10" y="5" width="1" height="1"/><rect x="11" y="4" width="1" height="1"/></g><g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="3" y="0" width="4" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="7" y="1" width="2" height="1" fill="var(--main)"/><rect x="1" y="2" width="1" height="1" fill="var(--main)"/><rect x="8" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="9" y="3" width="1" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/><rect x="9" y="4" width="1" height="1" fill="var(--main)"/><rect x="0" y="5" width="1" height="1" fill="var(--main)"/><rect x="9" y="5" width="1" height="1" fill="var(--main)"/><rect x="0" y="6" width="1" height="1" fill="var(--main)"/><rect x="9" y="6" width="1" height="1" fill="var(--main)"/><rect x="1" y="7" width="1" height="1" fill="var(--main)"/><rect x="8" y="7" width="1" height="1" fill="var(--main)"/><rect x="1" y="8" width="2" height="1" fill="var(--main)"/><rect x="7" y="8" width="2" height="1" fill="var(--main)"/><rect x="3" y="9" width="4" height="1" fill="var(--main)"/></g>"##,
    ),
    (
        "preset",
        18,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="9" y="8" width="1" height="1"/><rect x="10" y="7" width="1" height="1"/><rect x="11" y="6" width="1" height="1"/><rect x="12" y="5" width="1" height="1"/></g><g shape-rendering="crispEdges" transform="translate(4,4)"><rect x="3" y="0" width="4" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="7" y="1" width="2" height="1" fill="var(--main)"/><rect x="1" y="2" width="1" height="1" fill="var(--main)"/><rect x="8" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="9" y="3" width="1" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/><rect x="9" y="4" width="1" height="1" fill="var(--main)"/><rect x="0" y="5" width="1" height="1" fill="var(--main)"/><rect x="9" y="5" width="1" height="1" fill="var(--main)"/><rect x="0" y="6" width="1" height="1" fill="var(--main)"/><rect x="9" y="6" width="1" height="1" fill="var(--main)"/><rect x="1" y="7" width="1" height="1" fill="var(--main)"/><rect x="8" y="7" width="1" height="1" fill="var(--main)"/><rect x="1" y="8" width="2" height="1" fill="var(--main)"/><rect x="7" y="8" width="2" height="1" fill="var(--main)"/><rect x="3" y="9" width="4" height="1" fill="var(--main)"/></g>"##,
    ),
    (
        "preset",
        24,
        r##"<circle class="s1" stroke-width="1" cx="12" cy="12" r="6.5"/><g class="f1" shape-rendering="crispEdges"><rect x="12" y="11" width="1" height="1"/><rect x="13" y="10" width="1" height="1"/><rect x="14" y="9" width="1" height="1"/><rect x="15" y="8" width="1" height="1"/><rect x="16" y="7" width="1" height="1"/></g>"##,
    ),
    (
        "instrument",
        16,
        r##"<path class="s1" stroke-width="1" d="M3.5 4.5 H12.5 V11.5 H3.5 Z"/><rect class="f1" x="6" y="8" width="1" height="3"/><rect class="f1" x="9" y="8" width="1" height="3"/><rect class="f1" x="5" y="5" width="2" height="3"/><rect class="f1" x="9" y="5" width="2" height="3"/>"##,
    ),
    (
        "instrument",
        24,
        r##"<path class="s1" stroke-width="1" d="M4.5 6.5 H19.5 V17.5 H4.5 Z"/><rect class="f1" x="9" y="12" width="1" height="5"/><rect class="f1" x="14" y="12" width="1" height="5"/><rect class="f1" x="8" y="7" width="3" height="5"/><rect class="f1" x="13" y="7" width="3" height="5"/>"##,
    ),
    (
        "effect",
        16,
        r##"<path class="s1" stroke-width="1" d="M3 6.5 H8 C10 6.5 11 9 12.5 12.5"/>"##,
    ),
    (
        "effect",
        24,
        r##"<path class="s1" stroke-width="1" d="M4.5 9.5 H12 C15 9.5 16.5 13 18.5 18.5"/>"##,
    ),
    (
        "pack",
        12,
        r##"<path class="s1" stroke-width="1" d="M1.5 2.5 H10.5 V4.5 H1.5 Z M2.5 4.5 V10.5 H9.5 V4.5"/><rect class="f1" x="5" y="6" width="2" height="1"/>"##,
    ),
    (
        "pack",
        14,
        r##"<path class="s1" stroke-width="1" d="M1.5 2.5 H12.5 V5.5 H1.5 Z M2.5 5.5 V12.5 H11.5 V5.5"/><rect class="f1" x="5" y="7" width="4" height="1"/>"##,
    ),
    (
        "pack",
        18,
        r##"<path class="s1" stroke-width="1" d="M2.5 3.5 H15.5 V6.5 H2.5 Z M3.5 6.5 V15.5 H14.5 V6.5"/><rect class="f1" x="7" y="9" width="4" height="1"/>"##,
    ),
    (
        "pack",
        21,
        r##"<path class="s1" stroke-width="1" d="M2.5 3.5 H18.5 V7.5 H2.5 Z M3.5 7.5 V18.5 H17.5 V7.5"/><rect class="f1" x="8" y="10" width="5" height="1"/>"##,
    ),
    (
        "star",
        12,
        r##"<path class="s1" stroke-width="1" stroke-linejoin="round" d="M6.00 1.40 L7.26 4.66 L10.76 4.85 L8.04 7.06 L8.94 10.45 L6.00 8.55 L3.06 10.45 L3.96 7.06 L1.24 4.85 L4.74 4.66 Z"/>"##,
    ),
    (
        "star",
        14,
        r##"<path class="s1" stroke-width="1" stroke-linejoin="round" d="M7.00 1.40 L8.52 5.31 L12.71 5.55 L9.45 8.20 L10.53 12.25 L7.00 9.98 L3.47 12.25 L4.55 8.20 L1.29 5.55 L5.48 5.31 Z"/>"##,
    ),
    (
        "star",
        18,
        r##"<path class="s1" stroke-width="1" stroke-linejoin="round" d="M9.00 2.00 L10.90 6.89 L16.13 7.18 L12.07 10.50 L13.41 15.57 L9.00 12.72 L4.59 15.57 L5.93 10.50 L1.87 7.18 L7.10 6.89 Z"/>"##,
    ),
    (
        "star",
        21,
        r##"<path class="s1" stroke-width="1" stroke-linejoin="round" d="M10.50 2.00 L12.77 7.87 L19.06 8.22 L14.18 12.20 L15.79 18.28 L10.50 14.87 L5.21 18.28 L6.82 12.20 L1.94 8.22 L8.23 7.87 Z"/>"##,
    ),
    (
        "starFilled",
        12,
        r##"<path class="f1" stroke-width="1" stroke-linejoin="round" d="M6.00 1.20 L7.31 4.59 L10.95 4.79 L8.13 7.09 L9.06 10.61 L6.00 8.64 L2.94 10.61 L3.87 7.09 L1.05 4.79 L4.69 4.59 Z"/>"##,
    ),
    (
        "starFilled",
        14,
        r##"<path class="f1" stroke-width="1" stroke-linejoin="round" d="M7.00 1.20 L8.57 5.24 L12.90 5.48 L9.54 8.22 L10.64 12.42 L7.00 10.07 L3.36 12.42 L4.46 8.22 L1.10 5.48 L5.43 5.24 Z"/>"##,
    ),
    (
        "starFilled",
        18,
        r##"<path class="f1" stroke-width="1" stroke-linejoin="round" d="M9.00 1.70 L10.97 6.79 L16.42 7.09 L12.19 10.54 L13.58 15.81 L9.00 12.85 L4.42 15.81 L5.81 10.54 L1.58 7.09 L7.03 6.79 Z"/>"##,
    ),
    (
        "starFilled",
        21,
        r##"<path class="f1" stroke-width="1" stroke-linejoin="round" d="M10.50 1.70 L12.85 7.76 L19.34 8.13 L14.30 12.24 L15.97 18.52 L10.50 15.00 L5.03 18.52 L6.70 12.24 L1.66 8.13 L8.15 7.76 Z"/>"##,
    ),
    (
        "search",
        12,
        r##"<g transform="translate(-2,-2)"><g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="2" y="0" width="3" height="1" fill="var(--main)"/><rect x="1" y="1" width="1" height="1" fill="var(--main)"/><rect x="5" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="6" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="6" y="3" width="1" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/><rect x="6" y="4" width="1" height="1" fill="var(--main)"/><rect x="1" y="5" width="1" height="1" fill="var(--main)"/><rect x="5" y="5" width="1" height="1" fill="var(--main)"/><rect x="2" y="6" width="3" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(9,9)"><rect x="0" y="0" width="2" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="2" y="2" width="2" height="1" fill="var(--main)"/><rect x="3" y="3" width="1" height="1" fill="var(--main)"/></g></g>"##,
    ),
    (
        "search",
        14,
        r##"<g transform="translate(-1,-1)"><g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="2" y="0" width="3" height="1" fill="var(--main)"/><rect x="1" y="1" width="1" height="1" fill="var(--main)"/><rect x="5" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="6" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="6" y="3" width="1" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/><rect x="6" y="4" width="1" height="1" fill="var(--main)"/><rect x="1" y="5" width="1" height="1" fill="var(--main)"/><rect x="5" y="5" width="1" height="1" fill="var(--main)"/><rect x="2" y="6" width="3" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(9,9)"><rect x="0" y="0" width="2" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="2" y="2" width="2" height="1" fill="var(--main)"/><rect x="3" y="3" width="1" height="1" fill="var(--main)"/></g></g>"##,
    ),
    (
        "search",
        18,
        r##"<g transform="translate(1,1)"><g shape-rendering="crispEdges"><rect x="5" y="2" width="3" height="1" fill="var(--main)"/><rect x="3" y="3" width="2" height="1" fill="var(--main)"/><rect x="8" y="3" width="2" height="1" fill="var(--main)"/><rect x="3" y="4" width="1" height="1" fill="var(--main)"/><rect x="5" y="4" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="9" y="4" width="1" height="1" fill="var(--main)"/><rect x="2" y="5" width="1" height="1" fill="var(--main)"/><rect x="4" y="5" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="10" y="5" width="1" height="1" fill="var(--main)"/><rect x="2" y="6" width="1" height="1" fill="var(--main)"/><rect x="10" y="6" width="1" height="1" fill="var(--main)"/><rect x="2" y="7" width="1" height="1" fill="var(--main)"/><rect x="10" y="7" width="1" height="1" fill="var(--main)"/><rect x="3" y="8" width="1" height="1" fill="var(--main)"/><rect x="9" y="8" width="1" height="1" fill="var(--main)"/><rect x="3" y="9" width="2" height="1" fill="var(--main)"/><rect x="8" y="9" width="2" height="1" fill="var(--main)"/><rect x="5" y="10" width="3" height="1" fill="var(--main)"/><rect x="9" y="10" width="2" height="1" fill="var(--main)"/><rect x="10" y="11" width="2" height="1" fill="var(--main)"/><rect x="11" y="12" width="2" height="1" fill="var(--main)"/><rect x="12" y="13" width="2" height="1" fill="var(--main)"/></g></g>"##,
    ),
    (
        "search",
        21,
        r##"<g transform="translate(1,1)"><g shape-rendering="crispEdges"><rect x="7" y="3" width="3" height="1" fill="var(--main)"/><rect x="5" y="4" width="2" height="1" fill="var(--main)"/><rect x="10" y="4" width="2" height="1" fill="var(--main)"/><rect x="4" y="5" width="1" height="1" fill="var(--main)"/><rect x="12" y="5" width="1" height="1" fill="var(--main)"/><rect x="4" y="6" width="1" height="1" fill="var(--main)"/><rect x="6" y="6" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="12" y="6" width="1" height="1" fill="var(--main)"/><rect x="3" y="7" width="1" height="1" fill="var(--main)"/><rect x="5" y="7" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="13" y="7" width="1" height="1" fill="var(--main)"/><rect x="3" y="8" width="1" height="1" fill="var(--main)"/><rect x="13" y="8" width="1" height="1" fill="var(--main)"/><rect x="3" y="9" width="1" height="1" fill="var(--main)"/><rect x="13" y="9" width="1" height="1" fill="var(--main)"/><rect x="4" y="10" width="1" height="1" fill="var(--main)"/><rect x="12" y="10" width="1" height="1" fill="var(--main)"/><rect x="4" y="11" width="1" height="1" fill="var(--main)"/><rect x="12" y="11" width="1" height="1" fill="var(--main)"/><rect x="5" y="12" width="2" height="1" fill="var(--main)"/><rect x="10" y="12" width="2" height="1" fill="var(--main)"/><rect x="7" y="13" width="3" height="1" fill="var(--main)"/><rect x="12" y="13" width="2" height="1" fill="var(--main)"/><rect x="13" y="14" width="2" height="1" fill="var(--main)"/><rect x="14" y="15" width="2" height="1" fill="var(--main)"/><rect x="15" y="16" width="2" height="1" fill="var(--main)"/></g></g>"##,
    ),
    (
        "history",
        12,
        r##"<path class="s1" stroke-width="1" d="M2.74 5.13 A4 4 0 1 1 2.74 7.87"/><path class="f1" d="M0.8 7.4 L4.6 7.4 L2.7 9.8 Z"/><rect class="f1" x="6" y="4" width="1" height="3"/><rect class="f1" x="6" y="6" width="2" height="1"/>"##,
    ),
    (
        "history",
        14,
        r##"<path class="s1" stroke-width="1" d="M2.80 5.79 A5 5 0 1 1 2.80 9.21"/><path class="f1" d="M0.9 8.8 L4.7 8.8 L2.8 11.4 Z"/><rect class="f1" x="7" y="4" width="1" height="4"/><rect class="f1" x="7" y="7" width="3" height="1"/>"##,
    ),
    (
        "history",
        18,
        r##"<path class="s1" stroke-width="1" d="M3.39 7.28 A6.5 6.5 0 1 1 3.39 11.72"/><path class="f1" d="M1.2 11.2 L5.6 11.2 L3.4 14.2 Z"/><rect class="f1" x="9" y="5" width="1" height="5"/><rect class="f1" x="9" y="9" width="4" height="1"/>"##,
    ),
    (
        "history",
        21,
        r##"<path class="s1" stroke-width="1" d="M3.45 7.93 A7.5 7.5 0 1 1 3.45 13.07"/><path class="f1" d="M1 12.4 L5.9 12.4 L3.45 15.8 Z"/><rect class="f1" x="10" y="5" width="1" height="6"/><rect class="f1" x="10" y="10" width="4" height="1"/>"##,
    ),
    (
        "clear",
        10,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="1" y="1" width="1" height="1"/><rect x="2" y="2" width="1" height="1"/><rect x="3" y="3" width="1" height="1"/><rect x="4" y="4" width="1" height="1"/><rect x="5" y="5" width="1" height="1"/><rect x="6" y="6" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="1" y="7" width="1" height="1"/><rect x="2" y="6" width="1" height="1"/><rect x="3" y="5" width="1" height="1"/><rect x="4" y="4" width="1" height="1"/><rect x="5" y="3" width="1" height="1"/><rect x="6" y="2" width="1" height="1"/><rect x="7" y="1" width="1" height="1"/></g>"##,
    ),
    (
        "clear",
        12,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="2" y="2" width="1" height="1"/><rect x="3" y="3" width="1" height="1"/><rect x="4" y="4" width="1" height="1"/><rect x="5" y="5" width="1" height="1"/><rect x="6" y="6" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="2" y="8" width="1" height="1"/><rect x="3" y="7" width="1" height="1"/><rect x="4" y="6" width="1" height="1"/><rect x="5" y="5" width="1" height="1"/><rect x="6" y="4" width="1" height="1"/><rect x="7" y="3" width="1" height="1"/><rect x="8" y="2" width="1" height="1"/></g>"##,
    ),
    (
        "clear",
        15,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="3" y="3" width="1" height="1"/><rect x="4" y="4" width="1" height="1"/><rect x="5" y="5" width="1" height="1"/><rect x="6" y="6" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="9" y="9" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="11" y="11" width="1" height="1"/><rect x="3" y="11" width="1" height="1"/><rect x="4" y="10" width="1" height="1"/><rect x="5" y="9" width="1" height="1"/><rect x="6" y="8" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="8" y="6" width="1" height="1"/><rect x="9" y="5" width="1" height="1"/><rect x="10" y="4" width="1" height="1"/><rect x="11" y="3" width="1" height="1"/></g>"##,
    ),
    (
        "clear",
        18,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="4" y="4" width="1" height="1"/><rect x="5" y="5" width="1" height="1"/><rect x="6" y="6" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="9" y="9" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="11" y="11" width="1" height="1"/><rect x="12" y="12" width="1" height="1"/><rect x="4" y="12" width="1" height="1"/><rect x="5" y="11" width="1" height="1"/><rect x="6" y="10" width="1" height="1"/><rect x="7" y="9" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="9" y="7" width="1" height="1"/><rect x="10" y="6" width="1" height="1"/><rect x="11" y="5" width="1" height="1"/><rect x="12" y="4" width="1" height="1"/></g>"##,
    ),
    (
        "speaker",
        12,
        r##"<rect class="f1" x="1" y="5" width="2" height="3"/><rect class="f1" x="4" y="4" width="1" height="5"/><rect class="f1" x="5" y="3" width="1" height="7"/><path class="s1" stroke-width="1" d="M7.27 4.73 A2.5 2.5 0 0 1 7.27 8.27"/><path class="s1" stroke-width="1" d="M8.68 3.32 A4.5 4.5 0 0 1 8.68 9.68"/>"##,
    ),
    (
        "speaker",
        18,
        r##"<rect class="f1" x="2" y="7" width="3" height="4"/><rect class="f1" x="6" y="6" width="1" height="6"/><rect class="f1" x="7" y="5" width="1" height="8"/><rect class="f1" x="8" y="4" width="1" height="10"/><path class="s1" stroke-width="1" d="M10.97 6.53 A3.5 3.5 0 0 1 10.97 11.47"/><path class="s1" stroke-width="1" d="M12.74 4.76 A6 6 0 0 1 12.74 13.24"/>"##,
    ),
    (
        "check",
        10,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="1" y="5" width="1" height="1"/><rect x="2" y="6" width="1" height="1"/><rect x="3" y="7" width="1" height="1"/><rect x="4" y="6" width="1" height="1"/><rect x="5" y="5" width="1" height="1"/><rect x="6" y="4" width="1" height="1"/><rect x="7" y="3" width="1" height="1"/><rect x="8" y="2" width="1" height="1"/></g>"##,
    ),
    (
        "check",
        12,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="2" y="6" width="1" height="1"/><rect x="3" y="7" width="1" height="1"/><rect x="4" y="8" width="1" height="1"/><rect x="5" y="7" width="1" height="1"/><rect x="6" y="6" width="1" height="1"/><rect x="7" y="5" width="1" height="1"/><rect x="8" y="4" width="1" height="1"/><rect x="9" y="3" width="1" height="1"/><rect x="10" y="2" width="1" height="1"/></g>"##,
    ),
    (
        "check",
        15,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="2" y="8" width="1" height="1"/><rect x="3" y="9" width="1" height="1"/><rect x="4" y="10" width="1" height="1"/><rect x="5" y="11" width="1" height="1"/><rect x="6" y="10" width="1" height="1"/><rect x="7" y="9" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="9" y="7" width="1" height="1"/><rect x="10" y="6" width="1" height="1"/><rect x="11" y="5" width="1" height="1"/><rect x="12" y="4" width="1" height="1"/></g>"##,
    ),
    (
        "check",
        18,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="3" y="9" width="1" height="1"/><rect x="4" y="10" width="1" height="1"/><rect x="5" y="11" width="1" height="1"/><rect x="6" y="12" width="1" height="1"/><rect x="7" y="13" width="1" height="1"/><rect x="8" y="12" width="1" height="1"/><rect x="9" y="11" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="11" y="9" width="1" height="1"/><rect x="12" y="8" width="1" height="1"/><rect x="13" y="7" width="1" height="1"/><rect x="14" y="6" width="1" height="1"/><rect x="15" y="5" width="1" height="1"/></g>"##,
    ),
    (
        "download",
        12,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="5" y="1" width="1" height="1"/><rect x="5" y="2" width="1" height="1"/><rect x="5" y="3" width="1" height="1"/><rect x="5" y="4" width="1" height="1"/><rect x="5" y="5" width="1" height="1"/><rect x="5" y="6" width="1" height="1"/><rect x="3" y="7" width="1" height="1"/><rect x="4" y="7" width="1" height="1"/><rect x="5" y="7" width="1" height="1"/><rect x="6" y="7" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="4" y="8" width="1" height="1"/><rect x="5" y="8" width="1" height="1"/><rect x="6" y="8" width="1" height="1"/><rect x="5" y="9" width="1" height="1"/><rect x="1" y="8" width="1" height="1"/><rect x="1" y="9" width="1" height="1"/><rect x="1" y="10" width="1" height="1"/><rect x="1" y="10" width="1" height="1"/><rect x="2" y="10" width="1" height="1"/><rect x="3" y="10" width="1" height="1"/><rect x="4" y="10" width="1" height="1"/><rect x="5" y="10" width="1" height="1"/><rect x="6" y="10" width="1" height="1"/><rect x="7" y="10" width="1" height="1"/><rect x="8" y="10" width="1" height="1"/><rect x="9" y="10" width="1" height="1"/><rect x="9" y="8" width="1" height="1"/><rect x="9" y="9" width="1" height="1"/><rect x="9" y="10" width="1" height="1"/></g>"##,
    ),
    (
        "download",
        14,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="6" y="1" width="1" height="1"/><rect x="6" y="2" width="1" height="1"/><rect x="6" y="3" width="1" height="1"/><rect x="6" y="4" width="1" height="1"/><rect x="6" y="5" width="1" height="1"/><rect x="6" y="6" width="1" height="1"/><rect x="4" y="7" width="1" height="1"/><rect x="5" y="7" width="1" height="1"/><rect x="6" y="7" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="8" y="7" width="1" height="1"/><rect x="5" y="8" width="1" height="1"/><rect x="6" y="8" width="1" height="1"/><rect x="7" y="8" width="1" height="1"/><rect x="6" y="9" width="1" height="1"/><rect x="1" y="10" width="1" height="1"/><rect x="1" y="11" width="1" height="1"/><rect x="1" y="12" width="1" height="1"/><rect x="1" y="12" width="1" height="1"/><rect x="2" y="12" width="1" height="1"/><rect x="3" y="12" width="1" height="1"/><rect x="4" y="12" width="1" height="1"/><rect x="5" y="12" width="1" height="1"/><rect x="6" y="12" width="1" height="1"/><rect x="7" y="12" width="1" height="1"/><rect x="8" y="12" width="1" height="1"/><rect x="9" y="12" width="1" height="1"/><rect x="10" y="12" width="1" height="1"/><rect x="11" y="12" width="1" height="1"/><rect x="11" y="10" width="1" height="1"/><rect x="11" y="11" width="1" height="1"/><rect x="11" y="12" width="1" height="1"/></g>"##,
    ),
    (
        "download",
        18,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="8" y="2" width="1" height="1"/><rect x="8" y="3" width="1" height="1"/><rect x="8" y="4" width="1" height="1"/><rect x="8" y="5" width="1" height="1"/><rect x="8" y="6" width="1" height="1"/><rect x="8" y="7" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="8" y="9" width="1" height="1"/><rect x="5" y="10" width="1" height="1"/><rect x="6" y="10" width="1" height="1"/><rect x="7" y="10" width="1" height="1"/><rect x="8" y="10" width="1" height="1"/><rect x="9" y="10" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="11" y="10" width="1" height="1"/><rect x="6" y="11" width="1" height="1"/><rect x="7" y="11" width="1" height="1"/><rect x="8" y="11" width="1" height="1"/><rect x="9" y="11" width="1" height="1"/><rect x="10" y="11" width="1" height="1"/><rect x="7" y="12" width="1" height="1"/><rect x="8" y="12" width="1" height="1"/><rect x="9" y="12" width="1" height="1"/><rect x="8" y="13" width="1" height="1"/><rect x="2" y="12" width="1" height="1"/><rect x="2" y="13" width="1" height="1"/><rect x="2" y="14" width="1" height="1"/><rect x="2" y="15" width="1" height="1"/><rect x="2" y="15" width="1" height="1"/><rect x="3" y="15" width="1" height="1"/><rect x="4" y="15" width="1" height="1"/><rect x="5" y="15" width="1" height="1"/><rect x="6" y="15" width="1" height="1"/><rect x="7" y="15" width="1" height="1"/><rect x="8" y="15" width="1" height="1"/><rect x="9" y="15" width="1" height="1"/><rect x="10" y="15" width="1" height="1"/><rect x="11" y="15" width="1" height="1"/><rect x="12" y="15" width="1" height="1"/><rect x="13" y="15" width="1" height="1"/><rect x="14" y="15" width="1" height="1"/><rect x="14" y="12" width="1" height="1"/><rect x="14" y="13" width="1" height="1"/><rect x="14" y="14" width="1" height="1"/><rect x="14" y="15" width="1" height="1"/></g>"##,
    ),
    (
        "download",
        21,
        r##"<g class="f1" shape-rendering="crispEdges"><rect x="10" y="2" width="1" height="1"/><rect x="10" y="3" width="1" height="1"/><rect x="10" y="4" width="1" height="1"/><rect x="10" y="5" width="1" height="1"/><rect x="10" y="6" width="1" height="1"/><rect x="10" y="7" width="1" height="1"/><rect x="10" y="8" width="1" height="1"/><rect x="10" y="9" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="6" y="11" width="1" height="1"/><rect x="7" y="11" width="1" height="1"/><rect x="8" y="11" width="1" height="1"/><rect x="9" y="11" width="1" height="1"/><rect x="10" y="11" width="1" height="1"/><rect x="11" y="11" width="1" height="1"/><rect x="12" y="11" width="1" height="1"/><rect x="13" y="11" width="1" height="1"/><rect x="14" y="11" width="1" height="1"/><rect x="7" y="12" width="1" height="1"/><rect x="8" y="12" width="1" height="1"/><rect x="9" y="12" width="1" height="1"/><rect x="10" y="12" width="1" height="1"/><rect x="11" y="12" width="1" height="1"/><rect x="12" y="12" width="1" height="1"/><rect x="13" y="12" width="1" height="1"/><rect x="8" y="13" width="1" height="1"/><rect x="9" y="13" width="1" height="1"/><rect x="10" y="13" width="1" height="1"/><rect x="11" y="13" width="1" height="1"/><rect x="12" y="13" width="1" height="1"/><rect x="9" y="14" width="1" height="1"/><rect x="10" y="14" width="1" height="1"/><rect x="11" y="14" width="1" height="1"/><rect x="10" y="15" width="1" height="1"/><rect x="2" y="15" width="1" height="1"/><rect x="2" y="16" width="1" height="1"/><rect x="2" y="17" width="1" height="1"/><rect x="2" y="18" width="1" height="1"/><rect x="2" y="18" width="1" height="1"/><rect x="3" y="18" width="1" height="1"/><rect x="4" y="18" width="1" height="1"/><rect x="5" y="18" width="1" height="1"/><rect x="6" y="18" width="1" height="1"/><rect x="7" y="18" width="1" height="1"/><rect x="8" y="18" width="1" height="1"/><rect x="9" y="18" width="1" height="1"/><rect x="10" y="18" width="1" height="1"/><rect x="11" y="18" width="1" height="1"/><rect x="12" y="18" width="1" height="1"/><rect x="13" y="18" width="1" height="1"/><rect x="14" y="18" width="1" height="1"/><rect x="15" y="18" width="1" height="1"/><rect x="16" y="18" width="1" height="1"/><rect x="17" y="18" width="1" height="1"/><rect x="18" y="18" width="1" height="1"/><rect x="18" y="15" width="1" height="1"/><rect x="18" y="16" width="1" height="1"/><rect x="18" y="17" width="1" height="1"/><rect x="18" y="18" width="1" height="1"/></g>"##,
    ),
];

//! The song's title and tempo drawn into a video's frames as vectors: each glyph's outline
//! from the font (skrifa, the outline reader egui itself uses) filled by tiny-skia, so the
//! words are as crisp at 2 160 pixels as the character beside them. No shaping beyond the
//! font's own advances — a title is a line of Latin text, Spanish accents included.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use tiny_skia::{Color, FillRule, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

struct Pen<'a> {
    pb: &'a mut PathBuilder,
    x: f32,
    y: f32,
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.x + x, self.y - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.x + x, self.y - y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.pb
            .quad_to(self.x + cx, self.y - cy, self.x + x, self.y - y);
    }
    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        let (ox, oy) = (self.x, self.y);
        self.pb
            .cubic_to(ox + c0x, oy - c0y, ox + c1x, oy - c1y, ox + x, oy - y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// The light and the dark ink a title is drawn in.
pub const LIGHT: [u8; 4] = [250, 250, 252, 255];
pub const DARK: [u8; 4] = [18, 18, 22, 255];

/// WCAG's relative luminance of an sRGB colour.
pub fn luminance(c: [u8; 4]) -> f64 {
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG's contrast ratio of two colours (1 … 21).
pub fn contrast(a: [u8; 4], b: [u8; 4]) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The title's ink and its outline over `background` (`None`: transparent — whatever an
/// editor puts behind it): the ink of the two that stands out more, the other round it.
pub fn ink_for(background: Option<[u8; 4]>) -> ([u8; 4], [u8; 4]) {
    match background {
        Some(bg) if contrast(DARK, bg) > contrast(LIGHT, bg) => (DARK, LIGHT),
        _ => (LIGHT, DARK),
    }
}

/// How wide `text` is at `px` pixels a em, with `font`.
pub fn width(font: &[u8], text: &str, px: f32) -> f32 {
    let Ok(f) = FontRef::new(font) else {
        return 0.0;
    };
    let (map, m) = (
        f.charmap(),
        f.glyph_metrics(Size::new(px), LocationRef::default()),
    );
    text.chars()
        .filter_map(|c| map.map(c))
        .filter_map(|g| m.advance_width(g))
        .sum()
}

/// Draw `text` with its baseline's left end at (`x`, `baseline`), `px` pixels a em, in
/// straight `rgba`; shrunk to fit `max_width` when it is longer. Characters the font lacks
/// are left out.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    pm: &mut Pixmap,
    font: &[u8],
    text: &str,
    x: f32,
    baseline: f32,
    px: f32,
    max_width: f32,
    rgba: [u8; 4],
    outline: Option<[u8; 4]>,
) {
    let Ok(f) = FontRef::new(font) else {
        return;
    };
    let w = width(font, text, px);
    let px = if w > max_width && w > 0.0 {
        px * max_width / w
    } else {
        px
    };
    let size = Size::new(px);
    let (map, metrics, outlines) = (
        f.charmap(),
        f.glyph_metrics(size, LocationRef::default()),
        f.outline_glyphs(),
    );
    let mut pb = PathBuilder::new();
    let mut pen_x = x;
    for c in text.chars() {
        let Some(g) = map.map(c) else { continue };
        if let Some(o) = outlines.get(g) {
            let mut pen = Pen {
                pb: &mut pb,
                x: pen_x,
                y: baseline,
            };
            let _ = o.draw(
                DrawSettings::unhinted(size, LocationRef::default()),
                &mut pen,
            );
        }
        pen_x += metrics.advance_width(g).unwrap_or(0.0);
    }
    let Some(path) = pb.finish() else { return };
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    if let Some(o) = outline {
        // A soft edge of the other colour round every glyph: the words read over any
        // picture behind them (a transparent layer's, a busy stage).
        paint.set_color(Color::from_rgba8(o[0], o[1], o[2], o[3]));
        let stroke = Stroke {
            width: (px * 0.14).max(1.0),
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
    paint.set_color(Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A font the tests can read: the system's, else none (the drawing is skipped).
    fn any_font() -> Option<Vec<u8>> {
        [
            "C:/Windows/Fonts/arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ]
        .iter()
        .find_map(|p| std::fs::read(p).ok())
    }

    #[test]
    fn words_are_drawn_where_asked_and_fit_their_width() {
        let Some(font) = any_font() else {
            return;
        };
        let mut pm = Pixmap::new(400, 100).unwrap();
        draw(
            &mut pm,
            &font,
            "Canción",
            10.0,
            70.0,
            60.0,
            120.0,
            [255, 255, 255, 255],
            None,
        );
        let lit: Vec<(u32, u32)> = (0..100)
            .flat_map(|y| (0..400).map(move |x| (x, y)))
            .filter(|&(x, y)| pm.pixel(x, y).unwrap().alpha() > 128)
            .collect();
        assert!(!lit.is_empty());
        assert!(
            lit.iter().all(|&(x, y)| (9..=131).contains(&x) && y <= 72),
            "inside its box"
        );
        assert!(
            width(&font, "Canción", 60.0) > 120.0,
            "it was shrunk to fit"
        );
    }

    #[test]
    fn the_interfaces_own_face_draws() {
        let font = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../cantus-ui/assets/fonts/IBMPlexSans-SemiBold.ttf"
        ))
        .unwrap();
        for (w, h) in [(1080, 1920), (300, 533), (64, 64)] {
            let mut pm = Pixmap::new(w, h).unwrap();
            let (wf, hf) = (w as f32, h as f32);
            let px = (wf.min(hf) * 0.045).max(8.0);
            draw(
                &mut pm,
                &font,
                "Canción · 120 BPM",
                px,
                hf - px * 1.4,
                px,
                wf - 2.0 * px,
                [255; 4],
                Some(DARK),
            );
        }
    }

    #[test]
    fn the_ink_reads_over_every_background_offered() {
        use cantus_dance::render::{CHROMA_BLUE, CHROMA_GREEN, CHROMA_MAGENTA};
        // The keys, black, white, and stage colours light and dark (Paper's #dfe4e7 among
        // them).
        for bg in [
            CHROMA_GREEN,
            CHROMA_BLUE,
            CHROMA_MAGENTA,
            [0, 0, 0, 255],
            [255, 255, 255, 255],
            [0xdf, 0xe4, 0xe7, 255],
            [0x1d, 0x1f, 0x24, 255],
            [0x80, 0x80, 0x80, 255],
        ] {
            let (ink, edge) = ink_for(Some(bg));
            assert!(contrast(ink, bg) >= 4.5, "{bg:?}: {}", contrast(ink, bg));
            assert!(contrast(ink, edge) >= 4.5);
        }
        // Transparent: the outline carries it over whatever is put behind.
        let (ink, edge) = ink_for(None);
        assert!(contrast(ink, edge) >= 4.5);
    }
}

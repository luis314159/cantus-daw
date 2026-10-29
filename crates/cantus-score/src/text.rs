//! The score's words — title, composer, part names, tempo, bar numbers, *accel.* — are set
//! in a text face the caller gives (the interface's IBM Plex, the same bytes the PDF
//! embeds), measured here so the layout centres and clears them exactly.

use skrifa::instance::{LocationRef, Size};
use skrifa::MetadataProvider;

/// Which face a text is set in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Bold,
    Italic,
}

impl Face {
    pub const ALL: [Face; 3] = [Face::Regular, Face::Bold, Face::Italic];
}

/// The text faces' bytes (TrueType or OpenType).
#[derive(Copy, Clone, Debug)]
pub struct TextFaces {
    pub regular: &'static [u8],
    pub bold: &'static [u8],
    pub italic: &'static [u8],
}

impl TextFaces {
    pub fn bytes(&self, face: Face) -> &'static [u8] {
        match face {
            Face::Regular => self.regular,
            Face::Bold => self.bold,
            Face::Italic => self.italic,
        }
    }
}

/// Measures text in the faces given (or, without faces, by a fixed estimate — for tools
/// that only need the notes).
#[derive(Clone, Debug, Default)]
pub struct TextMeasure {
    faces: Option<TextFaces>,
}

impl TextMeasure {
    pub fn new(faces: Option<TextFaces>) -> TextMeasure {
        TextMeasure { faces }
    }

    pub fn faces(&self) -> Option<TextFaces> {
        self.faces
    }

    /// The advance width of `s` at `size` (both in staff spaces).
    pub fn width(&self, s: &str, size: f32, face: Face) -> f32 {
        let Some(f) = self.faces else {
            return s.chars().count() as f32 * size * 0.55;
        };
        let Ok(font) = skrifa::FontRef::new(f.bytes(face)) else {
            return s.chars().count() as f32 * size * 0.55;
        };
        let map = font.charmap();
        let metrics = font.glyph_metrics(Size::new(size), LocationRef::default());
        s.chars()
            .map(|c| {
                map.map(c)
                    .and_then(|g| metrics.advance_width(g))
                    .unwrap_or(size * 0.5)
            })
            .sum()
    }

    /// The height of capitals at `size` (to set words beside glyphs).
    pub fn cap_height(&self, size: f32) -> f32 {
        size * 0.7
    }
}

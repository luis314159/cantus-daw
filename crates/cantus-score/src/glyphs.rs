//! The music font: Bravura (SIL OFL 1.1), and what the engraver reads of its glyphs.
//!
//! SMuFL's metadata is in staff spaces with y **up**; the display list is y **down**. The
//! helpers here turn one into the other, so no other module flips a sign.

pub use crate::glyphs_meta::{defaults, Glyph, Meta, BRAVURA_VERSION};

/// Bravura 1.482 (Steinberg, SIL Open Font License 1.1 — `assets/LICENSE-Bravura-OFL.txt`).
pub static BRAVURA: &[u8] = include_bytes!("../assets/Bravura.otf");

/// A SMuFL font's em is four staff spaces: a glyph drawn at `em = 4 × space`.
pub const EM_IN_SPACES: f32 = 4.0;

const STEM_UP_SE: usize = 0;
const STEM_DOWN_NW: usize = 1;
const STEM_UP_NW: usize = 2;
const STEM_DOWN_SW: usize = 3;

impl Glyph {
    pub fn meta(self) -> &'static Meta {
        &crate::glyphs_meta::META[self as usize]
    }

    pub fn codepoint(self) -> char {
        self.meta().codepoint
    }

    pub fn name(self) -> &'static str {
        self.meta().name
    }

    /// The box in display coordinates (y down) relative to the glyph's origin:
    /// `[left, top, right, bottom]`.
    pub fn bounds(self) -> [f32; 4] {
        let b = self.meta().bbox;
        [b[0], -b[3], b[2], -b[1]]
    }

    pub fn width(self) -> f32 {
        let b = self.meta().bbox;
        b[2] - b[0]
    }

    pub fn advance(self) -> f32 {
        self.meta().advance
    }

    fn anchor(self, i: usize) -> Option<[f32; 2]> {
        let a = self.meta().anchors[i];
        (!a[0].is_nan()).then_some([a[0], -a[1]])
    }

    /// Where an up stem meets a notehead (its right side), y down.
    pub fn stem_up_se(self) -> [f32; 2] {
        self.anchor(STEM_UP_SE).unwrap_or([self.width(), 0.0])
    }

    /// Where a down stem meets a notehead (its left side), y down.
    pub fn stem_down_nw(self) -> [f32; 2] {
        self.anchor(STEM_DOWN_NW).unwrap_or([0.0, 0.0])
    }

    /// Where a flag meets an up stem's end, y down.
    pub fn stem_up_nw(self) -> [f32; 2] {
        self.anchor(STEM_UP_NW).unwrap_or([0.0, 0.0])
    }

    /// Where a flag meets a down stem's end, y down.
    pub fn stem_down_sw(self) -> [f32; 2] {
        self.anchor(STEM_DOWN_SW).unwrap_or([0.0, 0.0])
    }

    /// The glyph for a digit of a time signature.
    pub fn time_digit(d: u32) -> Glyph {
        const D: [Glyph; 10] = [
            Glyph::TimeSig0,
            Glyph::TimeSig1,
            Glyph::TimeSig2,
            Glyph::TimeSig3,
            Glyph::TimeSig4,
            Glyph::TimeSig5,
            Glyph::TimeSig6,
            Glyph::TimeSig7,
            Glyph::TimeSig8,
            Glyph::TimeSig9,
        ];
        D[(d % 10) as usize]
    }

    /// The glyph for a digit of a tuplet number.
    pub fn tuplet_digit(d: u32) -> Glyph {
        const D: [Glyph; 10] = [
            Glyph::Tuplet0,
            Glyph::Tuplet1,
            Glyph::Tuplet2,
            Glyph::Tuplet3,
            Glyph::Tuplet4,
            Glyph::Tuplet5,
            Glyph::Tuplet6,
            Glyph::Tuplet7,
            Glyph::Tuplet8,
            Glyph::Tuplet9,
        ];
        D[(d % 10) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skrifa::MetadataProvider;

    #[test]
    fn every_glyph_the_engraver_uses_is_in_the_font() {
        let font = skrifa::FontRef::new(BRAVURA).expect("Bravura parses");
        let map = font.charmap();
        for g in Glyph::ALL {
            assert!(
                map.map(g.codepoint()).is_some(),
                "{} (U+{:04X}) is missing from Bravura",
                g.name(),
                g.codepoint() as u32
            );
        }
    }

    #[test]
    fn the_table_is_in_the_enum_order() {
        for (i, g) in Glyph::ALL.iter().enumerate() {
            assert_eq!(*g as usize, i);
        }
        assert_eq!(Glyph::NoteheadBlack.name(), "noteheadBlack");
        let se = Glyph::NoteheadBlack.stem_up_se();
        assert!((se[0] - 1.18).abs() < 1e-6 && (se[1] + 0.168).abs() < 1e-6);
    }
}

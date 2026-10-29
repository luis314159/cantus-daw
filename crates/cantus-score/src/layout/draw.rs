//! Drawing one system: each staff's lines, preamble, notes, rests, stems, beams and
//! marks in staff-local y (0 the top line), then — in `system.rs` — stacked by their
//! skylines and joined by bar lines, a brace and names.

use super::chord::y_of;
use super::sky::Skyline;
use crate::display::Item;
use crate::glyphs::Glyph;
use crate::model::{Clef, KeySig};
use cantus_core::time::TimeSignature;

/// What a system shows, and where its bars and columns stand.
#[derive(Clone, Debug)]
pub struct SystemPlan {
    pub first_bar: usize,
    pub last_bar: usize,
    /// The staff lines' left end and right end.
    pub x0: f32,
    pub x1: f32,
    /// The first system: the time signature always, names in full.
    pub first: bool,
    /// Each bar's start x (its bar line, or the preamble's end for the first) and, per
    /// bar, each column's x.
    pub bar_x: Vec<f32>,
    pub bar_end: Vec<f32>,
    pub col_x: Vec<Vec<f32>>,
    /// Where the preamble ends (the first bar's notes begin after it).
    pub preamble_end: f32,
    /// A courtesy time signature after the last bar line (the next system changes metre).
    pub courtesy: Option<TimeSignature>,
}

/// One staff of a system, drawn.
pub struct StaffDrawn {
    pub items: Vec<Item>,
    pub sky: Skyline,
}

/// The clef glyph and the y of its origin.
pub fn clef_glyph(c: Clef) -> (Glyph, f32) {
    match c {
        Clef::Treble => (Glyph::GClef, 3.0),
        Clef::Treble8vb => (Glyph::GClefOttava8vb, 3.0),
        Clef::Treble8va => (Glyph::GClefOttava8va, 3.0),
        Clef::Bass => (Glyph::FClef, 1.0),
        Clef::Bass8vb => (Glyph::FClefOttava8vb, 1.0),
        Clef::Alto => (Glyph::CClef, 2.0),
        Clef::Tenor => (Glyph::CClef, 1.0),
    }
}

/// The staff positions of a key signature's accidentals in a clef.
pub fn key_positions(key: KeySig, clef: Clef) -> Vec<(i32, Glyph)> {
    // In the treble clef: F5 C5 G5 D5 A4 E5 B4 sharps; B4 E5 A4 D5 G4 C5 F4 flats.
    const SHARPS: [i32; 7] = [4, 1, 5, 2, -1, 3, 0];
    const FLATS: [i32; 7] = [0, 3, -1, 2, -2, 1, -3];
    let shift = match clef {
        Clef::Treble | Clef::Treble8va | Clef::Treble8vb => 0,
        Clef::Bass | Clef::Bass8vb => -2,
        Clef::Alto => -1,
        Clef::Tenor => 1,
    };
    let n = key.fifths.unsigned_abs().min(7) as usize;
    if key.fifths > 0 {
        SHARPS[..n]
            .iter()
            .map(|p| {
                let mut q = p + shift;
                if clef == Clef::Tenor && q > 4 {
                    q -= 7;
                }
                (q, Glyph::AccidentalSharp)
            })
            .collect()
    } else {
        FLATS[..n]
            .iter()
            .map(|p| (p + shift, Glyph::AccidentalFlat))
            .collect()
    }
}

const KEY_GAP: f32 = 0.1;

/// A time signature's digits: numerator over denominator, centred on the wider.
pub fn time_width(sig: TimeSignature) -> f32 {
    let w = |n: u8| -> f32 {
        n.to_string()
            .chars()
            .map(|c| Glyph::time_digit(c.to_digit(10).unwrap_or(0)).advance())
            .sum()
    };
    w(sig.numerator).max(w(sig.denominator))
}

pub fn draw_time(sig: TimeSignature, x: f32, out: &mut Vec<Item>) {
    let total = time_width(sig);
    for (n, y) in [(sig.numerator, 1.0f32), (sig.denominator, 3.0)] {
        let digits: Vec<Glyph> = n
            .to_string()
            .chars()
            .map(|c| Glyph::time_digit(c.to_digit(10).unwrap_or(0)))
            .collect();
        let w: f32 = digits.iter().map(|g| g.advance()).sum();
        let mut at = x + (total - w) / 2.0;
        for g in digits {
            out.push(Item::Glyph {
                glyph: g,
                x: at,
                y,
                scale: 1.0,
                owner: None,
            });
            at += g.advance();
        }
    }
}

/// The preamble's pieces and where each starts: clef, key signature, time signature (if
/// shown); returns where the preamble ends.
pub fn preamble(
    clef: Clef,
    key: KeySig,
    sig: Option<TimeSignature>,
    x0: f32,
    out: Option<&mut Vec<Item>>,
) -> f32 {
    let mut items = Vec::new();
    let (g, y) = clef_glyph(clef);
    let mut x = x0 + 0.75;
    items.push(Item::Glyph {
        glyph: g,
        x,
        y,
        scale: 1.0,
        owner: None,
    });
    x += g.width();
    if key.fifths != 0 {
        x += 0.8;
        for (pos, g) in key_positions(key, clef) {
            items.push(Item::Glyph {
                glyph: g,
                x,
                y: y_of(pos),
                scale: 1.0,
                owner: None,
            });
            x += g.width() + KEY_GAP;
        }
        x -= KEY_GAP;
    }
    if let Some(sig) = sig {
        x += 0.9;
        draw_time(sig, x, &mut items);
        x += time_width(sig);
    }
    if let Some(out) = out {
        out.extend(items);
    }
    x
}

/// Glyph of a dynamic.
pub fn dynamic_glyph(d: crate::model::Dynamic) -> Glyph {
    use crate::model::Dynamic::*;
    match d {
        Ppp => Glyph::DynamicPPP,
        Pp => Glyph::DynamicPP,
        P => Glyph::DynamicPiano,
        Mp => Glyph::DynamicMP,
        Mf => Glyph::DynamicMF,
        F => Glyph::DynamicForte,
        Ff => Glyph::DynamicFF,
        Fff => Glyph::DynamicFFF,
    }
}

//! One event's shape around its column: noteheads (a second's upper head beside the stem
//! when the stem is up, the lower when it is down), the stem's place, accidentals stacked
//! in columns without collisions (Gould: the highest nearest the note, then the lowest,
//! inward — a seventh apart may share a column), and augmentation dots in spaces.
//!
//! x is relative to the column (the left edge of a notehead on the stem's normal side);
//! y is relative to the staff's top line, in spaces, down.

use crate::glyphs::{defaults, Glyph};
use crate::model::{Dur, Event, Kind, Stem};

/// The y of staff position `pos` (0 the middle line, +1 a step up).
pub fn y_of(pos: i32) -> f32 {
    2.0 - pos as f32 * 0.5
}

/// The notehead glyph of a value.
pub fn notehead(d: Dur) -> Glyph {
    match d.log {
        i8::MIN..=-1 => Glyph::NoteheadDoubleWhole,
        0 => Glyph::NoteheadWhole,
        1 => Glyph::NoteheadHalf,
        _ => Glyph::NoteheadBlack,
    }
}

/// The rest glyph of a value.
pub fn rest_glyph(d: Dur) -> Glyph {
    match d.log {
        i8::MIN..=-1 => Glyph::RestDoubleWhole,
        0 => Glyph::RestWhole,
        1 => Glyph::RestHalf,
        2 => Glyph::RestQuarter,
        3 => Glyph::RestEighth,
        4 => Glyph::RestSixteenth,
        5 => Glyph::RestThirtySecond,
        _ => Glyph::RestSixtyFourth,
    }
}

/// The flag glyph of a value and stem.
pub fn flag(d: Dur, stem: Stem) -> Option<Glyph> {
    let up = stem == Stem::Up;
    Some(match (d.log, up) {
        (3, true) => Glyph::FlagEighthUp,
        (3, false) => Glyph::FlagEighthDown,
        (4, true) => Glyph::FlagSixteenthUp,
        (4, false) => Glyph::FlagSixteenthDown,
        (5, true) => Glyph::FlagThirtySecondUp,
        (5, false) => Glyph::FlagThirtySecondDown,
        (l, true) if l >= 6 => Glyph::FlagSixtyFourthUp,
        (l, false) if l >= 6 => Glyph::FlagSixtyFourthDown,
        _ => return None,
    })
}

/// The accidental glyph of an alteration.
pub fn accidental(alter: i8) -> Glyph {
    match alter {
        -2 => Glyph::AccidentalDoubleFlat,
        -1 => Glyph::AccidentalFlat,
        1 => Glyph::AccidentalSharp,
        2 => Glyph::AccidentalDoubleSharp,
        _ => Glyph::AccidentalNatural,
    }
}

/// A placed notehead.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadAt {
    pub pos: i32,
    pub dx: f32,
    pub glyph: Glyph,
    pub id: u64,
}

/// A placed accidental (with its parentheses when it is a courtesy one).
#[derive(Clone, Debug, PartialEq)]
pub struct AccAt {
    pub pos: i32,
    pub dx: f32,
    pub glyph: Glyph,
    pub courtesy: bool,
    pub id: u64,
}

/// An event's shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub heads: Vec<HeadAt>,
    pub accidentals: Vec<AccAt>,
    /// Dots: x and y.
    pub dots: Vec<(f32, f32)>,
    /// The stem's centre x, if it has one.
    pub stem_x: Option<f32>,
    /// A rest's glyph and its y.
    pub rest: Option<(Glyph, f32)>,
    /// How far it reaches left (negative) and right of the column.
    pub left: f32,
    pub right: f32,
}

/// The gap between an accidental and its notehead, and between accidental columns.
const ACC_GAP: f32 = 0.2;
const ACC_COL_GAP: f32 = 0.12;
const DOT_GAP: f32 = 0.35;
const DOT_STEP: f32 = 0.5;

/// The y a rest sits at: the middle of the staff, moved up for voice 1 and down for voice
/// 2 when two voices share it.
pub fn rest_y(glyph: Glyph, voice: usize, two: bool) -> f32 {
    let base = match glyph {
        Glyph::RestWhole => 1.0,
        _ => 2.0,
    };
    if !two {
        base
    } else if voice == 0 {
        base - 2.0
    } else {
        base + 2.0
    }
}

/// The shape of `e`, voice `voice`, `two` when two voices share the staff in its bar.
pub fn shape(e: &Event, voice: usize, two: bool) -> Shape {
    match &e.kind {
        Kind::Rest { .. } => {
            let g = rest_glyph(e.dur);
            let y = rest_y(g, voice, two);
            let w = g.advance();
            let mut dots = Vec::new();
            for k in 0..e.dur.dots {
                dots.push((w + DOT_GAP + k as f32 * DOT_STEP, 1.5));
            }
            let right = dots.last().map_or(w, |d| d.0 + 0.4);
            Shape {
                heads: Vec::new(),
                accidentals: Vec::new(),
                dots,
                stem_x: None,
                rest: Some((g, y)),
                left: 0.0,
                right,
            }
        }
        Kind::Chord(hs) => chord_shape(e, hs, two && voice == 1),
    }
}

fn chord_shape(e: &Event, hs: &[crate::model::Head], lower_voice: bool) -> Shape {
    let g = notehead(e.dur);
    let hw = g.width();
    let th = defaults::STEM_THICKNESS;
    let up = e.stem == Stem::Up;
    let stemmed = e.dur.log >= 1;
    // Heads: walk from the stem's far end; a second from the last normal head flips.
    let mut order: Vec<usize> = (0..hs.len()).collect();
    order.sort_by_key(|&i| hs[i].pos);
    if !up {
        order.reverse();
    }
    let flip = if up { hw - th } else { -(hw - th) };
    let mut heads: Vec<HeadAt> = Vec::with_capacity(hs.len());
    let mut last: Option<(i32, bool)> = None;
    for &i in &order {
        let h = &hs[i];
        let flipped = match last {
            Some((p, was_flipped)) => (h.pos - p).abs() == 1 && !was_flipped,
            None => false,
        };
        heads.push(HeadAt {
            pos: h.pos,
            dx: if flipped { flip } else { 0.0 },
            glyph: g,
            id: h.id,
        });
        last = Some((h.pos, flipped));
    }
    // A unison of one value draws once.
    heads.dedup_by(|a, b| a.pos == b.pos && a.dx == b.dx);
    let head_left = heads.iter().map(|h| h.dx).fold(0.0f32, f32::min);
    let head_right = heads.iter().map(|h| h.dx + hw).fold(hw, f32::max);
    let stem_x = stemmed.then(|| {
        if up {
            g.stem_up_se()[0] - th / 2.0
        } else {
            g.stem_down_nw()[0] + th / 2.0
        }
    });
    // Accidentals: highest, lowest, next highest, next lowest…, each in the nearest
    // column where it clears the others.
    let mut want: Vec<(i32, Glyph, bool, u64)> = hs
        .iter()
        .filter_map(|h| {
            h.accidental
                .map(|a| (h.pos, accidental(a.alter), a.courtesy, h.id))
        })
        .collect();
    want.sort_by_key(|w| std::cmp::Reverse(w.0));
    let mut zig: Vec<(i32, Glyph, bool, u64)> = Vec::with_capacity(want.len());
    let (mut a, mut b) = (0usize, want.len());
    while a < b {
        zig.push(want[a]);
        a += 1;
        if a < b {
            b -= 1;
            zig.push(want[b]);
        }
    }
    // Columns: each a list of (top, bottom) extents and its width.
    let mut cols: Vec<(Vec<(f32, f32)>, f32)> = Vec::new();
    let mut placed: Vec<(i32, Glyph, bool, u64, usize)> = Vec::new();
    for (pos, glyph, courtesy, id) in zig {
        let bb = glyph.bounds();
        let y = y_of(pos);
        let ext = (y + bb[1] + 0.1, y + bb[3] - 0.1);
        let mut w = glyph.width();
        if courtesy {
            w += Glyph::AccidentalParensLeft.width() + Glyph::AccidentalParensRight.width();
        }
        let col = cols
            .iter()
            .position(|(spans, _)| spans.iter().all(|s| ext.1 <= s.0 || ext.0 >= s.1))
            .unwrap_or(cols.len());
        if col == cols.len() {
            cols.push((Vec::new(), 0.0));
        }
        cols[col].0.push(ext);
        cols[col].1 = cols[col].1.max(w);
        placed.push((pos, glyph, courtesy, id, col));
    }
    // The heads a column must clear: the leftmost head near its line.
    let mut col_x: Vec<f32> = Vec::with_capacity(cols.len());
    let mut x = head_left - ACC_GAP;
    for (_, w) in &cols {
        col_x.push(x - w);
        x -= w + ACC_COL_GAP;
    }
    let accidentals: Vec<AccAt> = placed
        .into_iter()
        .map(|(pos, glyph, courtesy, id, col)| {
            let w = cols[col].1;
            let own = glyph.width()
                + if courtesy {
                    Glyph::AccidentalParensLeft.width() + Glyph::AccidentalParensRight.width()
                } else {
                    0.0
                };
            AccAt {
                pos,
                // Right-aligned in its column, against the note.
                dx: col_x[col] + (w - own),
                glyph,
                courtesy,
                id,
            }
        })
        .collect();
    // Ledger lines reach past the heads: neighbours keep their ledger lines apart.
    let ledgered = heads.iter().any(|h| h.pos.abs() >= 6);
    let ext = if ledgered {
        defaults::LEGER_LINE_EXTENSION
    } else {
        0.0
    };
    let left = col_x
        .last()
        .copied()
        .unwrap_or(head_left)
        .min(head_left - ext);
    // Dots: in the space of each head (a head on a line moves to the space above, a second
    // voice's below), never two in one space.
    let mut dots = Vec::new();
    if e.dur.dots > 0 {
        let mut spaces: Vec<i32> = Vec::new();
        let mut ps: Vec<i32> = heads.iter().map(|h| h.pos).collect();
        ps.sort_unstable();
        ps.dedup();
        ps.reverse();
        for p in ps {
            // A second voice's dots go below its line, the first's above.
            let mut s = match (p.rem_euclid(2) == 0, lower_voice) {
                (true, false) => p + 1,
                (true, true) => p - 1,
                (false, _) => p,
            };
            while spaces.contains(&s) {
                s -= 2;
            }
            spaces.push(s);
        }
        let x0 = head_right + DOT_GAP;
        for s in spaces {
            for k in 0..e.dur.dots {
                dots.push((x0 + k as f32 * DOT_STEP, y_of(s)));
            }
        }
    }
    let mut right = head_right + ext;
    if let Some(d) = dots.iter().map(|d| d.0).reduce(f32::max) {
        right = right.max(d + Glyph::AugmentationDot.width());
    }
    // An unbeamed up-stem note's flag reaches right of its stem.
    if let (Some(sx), None, true) = (stem_x, e.group, up) {
        if let Some(f) = flag(e.dur, e.stem) {
            right = right.max(sx + f.width());
        }
    }
    Shape {
        heads,
        accidentals,
        dots,
        stem_x,
        rest: None,
        left,
        right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Accidental, Head, Spelled};

    fn head(pos: i32, alter: Option<i8>) -> Head {
        Head {
            pitch: 60,
            spelled: Spelled {
                letter: 0,
                alter: 0,
                octave: 4,
            },
            pos,
            accidental: alter.map(|a| Accidental {
                alter: a,
                courtesy: false,
            }),
            tie: false,
            tied_from: false,
            id: (pos + 100) as u64,
            velocity: 80,
        }
    }

    fn ev(heads: Vec<Head>, stem: Stem, dots: u8) -> Event {
        Event {
            start: 0,
            len: 960,
            dur: Dur::new(2, dots),
            kind: Kind::Chord(heads),
            tuplet: None,
            stem,
            beams: Vec::new(),
            group: None,
            dynamic: None,
        }
    }

    #[test]
    fn a_second_puts_the_upper_head_right_of_an_up_stem() {
        let s = shape(
            &ev(vec![head(0, None), head(1, None)], Stem::Up, 0),
            0,
            false,
        );
        assert_eq!(s.heads[0].dx, 0.0);
        assert!(s.heads[1].dx > 1.0);
        let s = shape(
            &ev(vec![head(0, None), head(1, None)], Stem::Down, 0),
            0,
            false,
        );
        assert!(s.heads.iter().any(|h| h.pos == 0 && h.dx < -1.0));
    }

    #[test]
    fn a_third_of_sharps_takes_two_columns_a_seventh_one() {
        let s = shape(
            &ev(vec![head(0, Some(1)), head(2, Some(1))], Stem::Up, 0),
            0,
            false,
        );
        let xs: Vec<f32> = s.accidentals.iter().map(|a| a.dx).collect();
        assert!((xs[0] - xs[1]).abs() > 0.9, "{xs:?}");
        let s = shape(
            &ev(vec![head(-3, Some(1)), head(3, Some(1))], Stem::Up, 0),
            0,
            false,
        );
        let xs: Vec<f32> = s.accidentals.iter().map(|a| a.dx).collect();
        assert!((xs[0] - xs[1]).abs() < 1e-6, "{xs:?}");
        assert!(s.left < -1.0);
    }

    #[test]
    fn dots_sit_in_spaces() {
        let s = shape(&ev(vec![head(0, None)], Stem::Up, 1), 0, false);
        assert_eq!(s.dots.len(), 1);
        assert_eq!(s.dots[0].1, y_of(1));
        let s = shape(&ev(vec![head(1, None)], Stem::Up, 1), 0, false);
        assert_eq!(s.dots[0].1, y_of(1));
    }
}

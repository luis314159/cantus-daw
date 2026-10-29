//! The engraved score as a display list: what to draw, where, in staff spaces (y down,
//! the origin at the page's top left). The screen, the PDF and the golden tests all read
//! this one list, so what is tested is what is shown and printed.

use crate::glyphs::Glyph;
use crate::text::Face;
use std::fmt::Write;

/// Which note an item draws (for selection and hit testing).
pub type Owner = Option<u64>;

/// Where a text item's `x` is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Anchor {
    Left,
    Centre,
    Right,
}

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A music glyph with its origin at `(x, y)`; `scale` 1 is the staff's size (an em of
    /// four spaces).
    Glyph {
        glyph: Glyph,
        x: f32,
        y: f32,
        scale: f32,
        owner: Owner,
    },
    /// A straight line `w` thick with square ends (staff and ledger lines, stems, bar
    /// lines, brackets).
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        w: f32,
        owner: Owner,
    },
    /// A dashed line (an octave line's).
    Dashed {
        x0: f32,
        y0: f32,
        x1: f32,
        w: f32,
        dash: f32,
        gap: f32,
    },
    /// A beam: the parallelogram whose top edge runs from `(x0, y0)` to `(x1, y1)`, `h`
    /// thick vertically.
    Beam {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        h: f32,
        owner: Owner,
    },
    /// A tie or slur: the cubic `p` is its middle line; it is `end` thick at its ends and
    /// `mid` at its middle, thickening away from the notes (`up` bows upward).
    Curve {
        p: [[f32; 2]; 4],
        end: f32,
        mid: f32,
        up: bool,
        owner: Owner,
    },
    /// Words: `y` is the baseline, `size` the font size in spaces.
    Text {
        text: String,
        x: f32,
        y: f32,
        size: f32,
        face: Face,
        anchor: Anchor,
    },
    /// A brace before a grand staff, from `y0` to `y1`: the glyph stretched to the height
    /// and widened far less (`outline::brace`).
    Brace { x: f32, y0: f32, y1: f32 },
}

impl Item {
    /// The note it draws, if any.
    pub fn owner(&self) -> Owner {
        match self {
            Item::Glyph { owner, .. }
            | Item::Line { owner, .. }
            | Item::Beam { owner, .. }
            | Item::Curve { owner, .. } => *owner,
            _ => None,
        }
    }

    /// Its bounding box `[x0, y0, x1, y1]` (text approximately, without a measure).
    pub fn bounds(&self) -> [f32; 4] {
        match self {
            Item::Glyph {
                glyph, x, y, scale, ..
            } => {
                let b = glyph.bounds();
                [
                    x + b[0] * scale,
                    y + b[1] * scale,
                    x + b[2] * scale,
                    y + b[3] * scale,
                ]
            }
            Item::Line {
                x0, y0, x1, y1, w, ..
            } => [
                x0.min(*x1) - w / 2.0,
                y0.min(*y1) - w / 2.0,
                x0.max(*x1) + w / 2.0,
                y0.max(*y1) + w / 2.0,
            ],
            Item::Dashed { x0, y0, x1, w, .. } => [*x0, y0 - w / 2.0, *x1, y0 + w / 2.0],
            Item::Beam {
                x0, y0, x1, y1, h, ..
            } => [*x0, y0.min(*y1), *x1, y0.max(*y1) + h],
            Item::Curve { p, mid, .. } => {
                let xs = p.iter().map(|q| q[0]);
                let ys = p.iter().map(|q| q[1]);
                let (x0, x1) = xs.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
                let (y0, y1) = ys.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
                [x0, y0 - mid, x1, y1 + mid]
            }
            Item::Text {
                text,
                x,
                y,
                size,
                anchor,
                ..
            } => {
                let w = text.chars().count() as f32 * size * 0.55;
                let x0 = match anchor {
                    Anchor::Left => *x,
                    Anchor::Centre => x - w / 2.0,
                    Anchor::Right => x - w,
                };
                [x0, y - size * 0.75, x0 + w, y + size * 0.25]
            }
            Item::Brace { x, y0, y1 } => [
                *x,
                *y0,
                x + crate::glyphs::Glyph::Brace.width()
                    * crate::outline::brace_width_scale(y1 - y0),
                *y1,
            ],
        }
    }
}

/// A list of items with the size of what holds them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DisplayList {
    pub items: Vec<Item>,
    pub width: f32,
    pub height: f32,
}

fn f(v: f32) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 {
        "0".to_string()
    } else {
        format!("{r}")
    }
}

impl DisplayList {
    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    /// One line per item, coordinates to a thousandth of a space: what the golden tests
    /// compare.
    pub fn dump(&self) -> String {
        let mut o = String::with_capacity(self.items.len() * 48);
        let _ = writeln!(o, "size {} {}", f(self.width), f(self.height));
        for it in &self.items {
            let own = |o: &Owner| o.map(|id| format!(" #{id}")).unwrap_or_default();
            let _ = match it {
                Item::Glyph {
                    glyph,
                    x,
                    y,
                    scale,
                    owner,
                } => {
                    let s = if (*scale - 1.0).abs() > 1e-6 {
                        format!(" x{}", f(*scale))
                    } else {
                        String::new()
                    };
                    writeln!(
                        o,
                        "glyph {} {} {}{s}{}",
                        glyph.name(),
                        f(*x),
                        f(*y),
                        own(owner)
                    )
                }
                Item::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    w,
                    owner,
                } => writeln!(
                    o,
                    "line {} {} {} {} w{}{}",
                    f(*x0),
                    f(*y0),
                    f(*x1),
                    f(*y1),
                    f(*w),
                    own(owner)
                ),
                Item::Dashed {
                    x0,
                    y0,
                    x1,
                    w,
                    dash,
                    gap,
                } => writeln!(
                    o,
                    "dashed {} {} {} w{} {}/{}",
                    f(*x0),
                    f(*y0),
                    f(*x1),
                    f(*w),
                    f(*dash),
                    f(*gap)
                ),
                Item::Beam {
                    x0,
                    y0,
                    x1,
                    y1,
                    h,
                    owner,
                } => writeln!(
                    o,
                    "beam {} {} {} {} h{}{}",
                    f(*x0),
                    f(*y0),
                    f(*x1),
                    f(*y1),
                    f(*h),
                    own(owner)
                ),
                Item::Curve {
                    p,
                    end,
                    mid,
                    up,
                    owner,
                } => writeln!(
                    o,
                    "curve {} {} {} {} {} {} {} {} {}/{} {}{}",
                    f(p[0][0]),
                    f(p[0][1]),
                    f(p[1][0]),
                    f(p[1][1]),
                    f(p[2][0]),
                    f(p[2][1]),
                    f(p[3][0]),
                    f(p[3][1]),
                    f(*end),
                    f(*mid),
                    if *up { "up" } else { "down" },
                    own(owner)
                ),
                Item::Text {
                    text,
                    x,
                    y,
                    size,
                    face,
                    anchor,
                } => writeln!(
                    o,
                    "text {:?} {} {} s{} {:?} {:?}",
                    text,
                    f(*x),
                    f(*y),
                    f(*size),
                    face,
                    anchor
                ),
                Item::Brace { x, y0, y1 } => writeln!(o, "brace {} {} {}", f(*x), f(*y0), f(*y1)),
            };
        }
        o
    }

    /// The items of note `id`.
    pub fn of_note(&self, id: u64) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(move |i| i.owner() == Some(id))
    }

    /// The note whose head is nearest `(x, y)` within `reach` spaces.
    pub fn note_at(&self, x: f32, y: f32, reach: f32) -> Option<u64> {
        let mut best: Option<(f32, u64)> = None;
        for it in &self.items {
            if let Item::Glyph {
                glyph,
                owner: Some(id),
                ..
            } = it
            {
                if !matches!(
                    glyph,
                    Glyph::NoteheadBlack
                        | Glyph::NoteheadHalf
                        | Glyph::NoteheadWhole
                        | Glyph::NoteheadDoubleWhole
                ) {
                    continue;
                }
                let b = it.bounds();
                let cx = x.clamp(b[0], b[2]);
                let cy = y.clamp(b[1], b[3]);
                let d = ((cx - x).powi(2) + (cy - y).powi(2)).sqrt();
                if d <= reach && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, *id));
                }
            }
        }
        best.map(|b| b.1)
    }
}

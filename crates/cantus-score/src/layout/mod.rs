//! The layout: the logical score engraved into display lists — one long system (beside
//! the piano roll), systems wrapped to a width (the Score pane), or pages (PDF and
//! print) — with systems broken where the music spaces best and pages filled and
//! justified (Gould, *Behind Bars*, "Layout").

mod beam;
mod chord;
mod columns;
mod draw;
mod marks;
mod notes;
mod sky;
mod system;

pub use columns::{space, QUARTER_SPACE};

use crate::display::DisplayList;
use crate::model::Score;
use crate::text::TextMeasure;

/// A paper size and the engraving's scale on it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Paper {
    pub width_mm: f32,
    pub height_mm: f32,
    pub margin_mm: f32,
    /// One staff space (Gould's rastral sizes: 1.75 mm is a 7 mm staff, the usual size for
    /// parts and piano music).
    pub space_mm: f32,
}

impl Paper {
    pub const A4: Paper = Paper {
        width_mm: 210.0,
        height_mm: 297.0,
        margin_mm: 15.0,
        space_mm: 1.75,
    };
    pub const LETTER: Paper = Paper {
        width_mm: 215.9,
        height_mm: 279.4,
        margin_mm: 15.0,
        space_mm: 1.75,
    };

    /// Its size in staff spaces.
    pub fn spaces(&self) -> (f32, f32, f32) {
        (
            self.width_mm / self.space_mm,
            self.height_mm / self.space_mm,
            self.margin_mm / self.space_mm,
        )
    }
}

/// How the score is laid out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Format {
    /// One system as long as the music (the staff beside the piano roll).
    Continuous,
    /// Systems wrapped to `width` spaces, one under another (the Score pane).
    Wrap { width: f32 },
    /// Pages of paper (the PDF, print).
    Pages(Paper),
}

#[derive(Clone, Debug)]
pub struct LayoutOptions {
    pub format: Format,
    pub text: TextMeasure,
    /// The title and composer over the first system (pages), page numbers after the first.
    pub header: bool,
}

/// Where one system lies and which ticks fall where in it: what the playhead, the
/// selection and a click need.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemMap {
    /// The first staff's top line and the last staff's bottom line (page y).
    pub top: f32,
    pub bottom: f32,
    pub x0: f32,
    pub x1: f32,
    pub first_bar: usize,
    pub last_bar: usize,
    /// `(absolute tick, x)` at every column and bar line, in order.
    pub ticks: Vec<(i64, f32)>,
    /// Each staff's top line (page y), in the score's staff order.
    pub staff_tops: Vec<f32>,
}

impl SystemMap {
    /// The x of `tick` in this system (between columns, in proportion), if it is in it.
    pub fn x_of(&self, tick: i64) -> Option<f32> {
        let first = self.ticks.first()?;
        let last = self.ticks.last()?;
        if tick < first.0 || tick > last.0 {
            return None;
        }
        let i = self.ticks.partition_point(|t| t.0 <= tick);
        if i == 0 {
            return Some(first.1);
        }
        let (t0, x0) = self.ticks[i - 1];
        match self.ticks.get(i) {
            Some(&(t1, x1)) if t1 > t0 => {
                Some(x0 + (x1 - x0) * (tick - t0) as f32 / (t1 - t0) as f32)
            }
            _ => Some(x0),
        }
    }

    /// The tick at `x` (the inverse of [`x_of`](Self::x_of)).
    pub fn tick_at(&self, x: f32) -> Option<i64> {
        let first = self.ticks.first()?;
        if x <= first.1 {
            return Some(first.0);
        }
        let i = self.ticks.partition_point(|t| t.1 <= x);
        let (t0, x0) = self.ticks[i - 1];
        match self.ticks.get(i) {
            Some(&(t1, x1)) if x1 > x0 => {
                Some(t0 + ((t1 - t0) as f32 * (x - x0) / (x1 - x0)).round() as i64)
            }
            _ => Some(t0),
        }
    }
}

/// One page (or the whole surface, for the screen's formats).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Page {
    pub list: DisplayList,
    pub systems: Vec<SystemMap>,
}

/// The engraved score.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Engraved {
    pub pages: Vec<Page>,
}

impl Engraved {
    /// The page and system holding `tick`, and its x there.
    pub fn locate(&self, tick: i64) -> Option<(usize, usize, f32)> {
        for (pi, p) in self.pages.iter().enumerate() {
            for (si, s) in p.systems.iter().enumerate() {
                if let Some(x) = s.x_of(tick) {
                    return Some((pi, si, x));
                }
            }
        }
        None
    }

    /// Every page's dump, for the golden tests.
    pub fn dump(&self) -> String {
        let mut o = String::new();
        for (i, p) in self.pages.iter().enumerate() {
            o.push_str(&format!("page {}\n", i + 1));
            o.push_str(&p.list.dump());
        }
        o
    }
}

/// Engrave `score`.
pub fn engrave(score: &Score, opts: &LayoutOptions) -> Engraved {
    system::engrave(score, opts)
}

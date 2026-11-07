//! `Meter` — a solid two-bar fill in a sunken well, three stops (lime; yellow from −6 dB;
//! orange from 0 dB), a 2-pt peak-hold line. [`MeterBatch`] paints every meter of a view
//! into the brush's one mesh with the colours worked out once and no feathering (every
//! edge is on the pixel grid) — `rendering-performance.md` §5.5 measured the batched
//! mesh 30× cheaper than eight shapes per meter.

use super::brush::{Brush, Fill};
use super::color::shade;
use super::light::Sunken;
use super::{Style, Tok};

/// The tokens a meter paints with (for inspect mode).
pub const TOKENS: &[Tok] = &[
    Tok::MeterWell,
    Tok::MeterLow,
    Tok::MeterHigh,
    Tok::MeterClip,
    Tok::InnerShadow,
    Tok::Lip,
];
use egui::{Color32, Pos2, Rect};

/// The meter's range, in dB.
pub const MIN_DB: f32 = -60.0;
pub const MAX_DB: f32 = 6.0;

/// Position (0 bottom … 1 top) of a level in dB: linear in dB, so −6 dB sits at 82 %.
pub fn pos_of_db(db: f32) -> f32 {
    ((db - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0)
}

/// The level in dB at a position (0…1), for the hint under the pointer.
pub fn db_at(pos: f32) -> f32 {
    MIN_DB + (MAX_DB - MIN_DB) * pos.clamp(0.0, 1.0)
}

/// What one meter shows, per channel (left, right), in dB.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Levels {
    pub level: [f32; 2],
    pub hold: [f32; 2],
}

impl Levels {
    pub const SILENT: Levels = Levels {
        level: [MIN_DB; 2],
        hold: [MIN_DB; 2],
    };
}

/// The colours of every meter in a view, worked out once.
#[derive(Copy, Clone, Debug)]
pub struct MeterBatch {
    well: Sunken,
    /// The shadow the well's upper lip casts, and the light on its lower lip (round 37).
    shadow: Color32,
    lip: Color32,
    lane: Color32,
    /// The well's rim: a light theme's firmer (judge 7: Paper's meters washed out).
    rim: Color32,
    low: Color32,
    high: Color32,
    clip: Color32,
    /// Meters painted through this batch (for the frame-budget counters).
    pub count: u32,
}

impl MeterBatch {
    pub fn new(s: &Style) -> MeterBatch {
        let t = &s.t;
        let m = super::light::Material::of(t);
        MeterBatch {
            well: Sunken::at(t.meter_well, t.depth),
            shadow: m.inner_shadow,
            // r37:finish — a light theme's lip half as bright: a white rim round a dark well
            // read harsh on paper.
            lip: if t.dark {
                m.lip
            } else {
                m.lip.gamma_multiply(0.5)
            },
            lane: shade(t.meter_well, if t.dark { -0.035 } else { 0.04 }), // r37:finish
            // r37:finish — a light theme's dark well has a firm dark rim on the paper.
            rim: if t.dark {
                Sunken::at(t.meter_well, t.depth).border
            } else {
                shade(t.meter_well, -0.5)
            },
            low: t.meter_low,
            high: t.meter_high,
            clip: t.meter_clip,
            count: 0,
        }
    }

    /// Paint one meter in `rect`: plain quads, straight into the brush's mesh.
    pub fn paint(&mut self, b: &mut Brush, rect: Rect, levels: &Levels) {
        self.count += 1;
        b.register(rect, "meter", TOKENS, &[]);
        let rect = b.snap_rect(rect);
        let px = b.px();
        let w = &self.well;
        // The lower lip catching the light, just under the well.
        b.rect(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.bottom()),
                Pos2::new(rect.right(), rect.bottom() + px),
            ),
            Fill::Solid(self.lip),
        );
        b.rect(rect, Fill::Solid(self.rim));
        let inner = rect.shrink(px);
        b.rect(inner, Fill::V(w.fill_top, w.fill_bottom));
        b.rect(
            Rect::from_min_max(inner.min, Pos2::new(inner.max.x, inner.min.y + px)),
            Fill::Solid(w.inner_top),
        );
        // Two bars, each 40 % of the well, with the gap in the middle.
        let iw = inner.width();
        let bw = b.snap((iw * 0.40).max(px));
        let gap = b.snap((iw - 2.0 * bw) / 3.0).max(0.0);
        let y_top = inner.top() + px;
        let y_bot = inner.bottom();
        let h = y_bot - y_top;
        let y_of = |db: f32| y_bot - h * pos_of_db(db);
        let y6 = b.snap(y_of(-6.0));
        let y0 = b.snap(y_of(0.0));
        for ch in 0..2 {
            let x0 = if ch == 0 {
                inner.left() + gap
            } else {
                inner.right() - gap - bw
            };
            let x1 = x0 + bw;
            b.rect(
                Rect::from_min_max(Pos2::new(x0, y_top), Pos2::new(x1, y_bot)),
                Fill::Solid(self.lane),
            );
            let lv = levels.level[ch];
            if lv > MIN_DB {
                let y = b.snap(y_of(lv));
                // Each zone lit brighter toward its top (round 37): a lit column, not a
                // flat block.
                let mut zone = |from: f32, to: f32, c: Color32| {
                    let (a, z) = (from.max(y), to);
                    if a < z {
                        b.rect(
                            Rect::from_min_max(Pos2::new(x0, a), Pos2::new(x1, z)),
                            Fill::V(shade(c, 0.08), shade(c, -0.07)),
                        );
                    }
                };
                zone(y6, y_bot, self.low);
                zone(y0, y6, self.high);
                zone(y_top, y0, self.clip);
            }
            let hold = levels.hold[ch];
            if hold > MIN_DB {
                let y = b.snap(y_of(hold));
                let c = if hold >= 0.0 {
                    self.clip
                } else if hold >= -6.0 {
                    self.high
                } else {
                    self.low
                };
                // The held peak: a bright line with a softer one under it (round 37).
                let t = b.snap((2.0 * px).max(px * 2.0));
                b.rect(
                    Rect::from_min_max(
                        Pos2::new(x0, y.max(y_top)),
                        Pos2::new(x1, (y + t * 0.5).min(y_bot)),
                    ),
                    Fill::Solid(shade(c, 0.10)),
                );
                b.rect(
                    Rect::from_min_max(
                        Pos2::new(x0, (y + t * 0.5).min(y_bot)),
                        Pos2::new(x1, (y + t).min(y_bot)),
                    ),
                    Fill::Solid(shade(c, -0.12)),
                );
            }
        }
        // The shadow the upper lip casts over the well's top.
        let reach = (3.0 * px).min(inner.height());
        b.rect(
            Rect::from_min_max(inner.min, Pos2::new(inner.max.x, inner.top() + reach)),
            Fill::V(self.shadow, Color32::TRANSPARENT),
        );
    }
}

/// Paint a single meter (a batch of one).
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, levels: &Levels) {
    MeterBatch::new(s).paint(b, rect, levels);
}

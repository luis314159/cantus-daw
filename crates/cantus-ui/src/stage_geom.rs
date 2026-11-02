//! Where a dancer stands on a stage, in pixels (ADR 0091 §1): pure geometry, no egui state.
//!
//! A stage over the workspace (or the docked Stage tab) is an **area** in physical pixels:
//! a dancer's feet stand at its `x`/`y` share of it and it is `size × standard` of its height
//! tall. Drawing the whole area every frame would upload megabytes; instead each dancer is
//! drawn into a **box** of its own — the character's bounds as the pose places them (lift,
//! spin and facing included), rounded out to whole pixels and up to a bucket so the pixmap
//! is not reallocated each frame — on a sub-stage whose numbers put the character exactly
//! where the whole area would (`a_box_draws_what_the_whole_area_draws`). The **body** is
//! the standing box (no lift, no spin), in points: what the pointer grabs, steady while the
//! dancer jumps and spins.

use cantus_dance::character::Character;
use cantus_dance::pose::Pose;
use cantus_dance::render::{placement, Background, Stage};
use egui::{pos2, Pos2, Rect};

/// How tall a dancer of size 1 stands over the workspace, as a share of its height.
pub const FLOAT_STANDARD: f32 = 0.30;
/// How tall it stands on the docked Stage tab, as a share of the tab's height.
pub const DOCK_STANDARD: f32 = 0.50;
/// Boxes grow in steps of this many pixels, so a dancer's pixmap keeps its size while it
/// moves a little.
pub const BUCKET: u32 = 32;
/// Room around a character's bounds, in its units (anti-aliasing, a stroke's width).
const PAD: f32 = 0.03;

/// An area in physical pixels: the workspace, or the Stage tab.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Area {
    /// Its top-left corner and size, in physical pixels (fractional: points × scale).
    pub min: Pos2,
    pub width: f32,
    pub height: f32,
    /// How tall a dancer of size 1 stands, as a share of `height`.
    pub standard: f32,
    /// Whether the characters draw their contact shadows on it: not over a chroma key,
    /// where a shadow is a darker key the capture leaves as a smudge.
    pub shadows: bool,
}

impl Area {
    /// The area of `rect` (points) at `ppp` physical pixels per point.
    pub fn of(rect: Rect, ppp: f32, standard: f32) -> Area {
        Area {
            min: pos2(rect.min.x * ppp, rect.min.y * ppp),
            width: rect.width() * ppp,
            height: rect.height() * ppp,
            standard,
            shadows: true,
        }
    }

    /// The same area with the contact shadows drawn or not (`false` over a chroma key).
    pub fn with_shadows(self, shadows: bool) -> Area {
        Area { shadows, ..self }
    }

    /// The pixels a unit of a character of `size` takes.
    pub fn px_per_unit(&self, size: f32) -> f32 {
        (size * self.standard * self.height).max(0.5)
    }

    /// Where the feet of a pose stand (no lift), in absolute physical pixels.
    pub fn feet(&self, pose: &Pose) -> Pos2 {
        pos2(
            self.min.x + pose.x * self.width,
            self.min.y + pose.y * self.height,
        )
    }

    /// The share of the area a point (physical pixels) is at: what `x`/`y` become.
    pub fn share(&self, p: Pos2) -> (f32, f32) {
        (
            (p.x - self.min.x) / self.width.max(1.0),
            (p.y - self.min.y) / self.height.max(1.0),
        )
    }

    /// A stage as big as the area (whole pixels), for placing on it.
    fn whole(&self) -> Stage {
        let w = self.width.round().max(1.0) as u32;
        let h = self.height.round().max(1.0) as u32;
        Stage {
            width: w,
            height: h,
            background: Background::Transparent,
            standard_height: self.standard * self.height / h as f32,
            shadows: self.shadows,
        }
    }
}

/// A dancer placed: the box to draw (absolute physical pixels, whole), the sub-stage and
/// pose that draw it there, and the body the pointer grabs (points).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Placed {
    /// The box's top-left corner, absolute physical pixels.
    pub x0: i32,
    pub y0: i32,
    /// The sub-stage (its size is the box's), and the pose re-expressed on it.
    pub stage: Stage,
    pub pose: Pose,
    /// The standing box, in points.
    pub body: Rect,
}

impl Placed {
    /// The box in points, at `ppp`.
    pub fn rect(&self, ppp: f32) -> Rect {
        Rect::from_min_size(
            pos2(self.x0 as f32 / ppp, self.y0 as f32 / ppp),
            egui::vec2(
                self.stage.width as f32 / ppp,
                self.stage.height as f32 / ppp,
            ),
        )
    }
}

fn round_up(v: u32, step: u32) -> u32 {
    v.div_ceil(step) * step
}

/// The corners of `ch`'s bounds (padded) through `pose`'s placement on `area`, as a box in
/// absolute physical pixels: (min, max).
fn bounds_px(area: &Area, ch: &dyn Character, pose: &Pose) -> (Pos2, Pos2) {
    let whole = area.whole();
    let b = ch.bounds();
    let corners = [
        (b.left - PAD, b.top - PAD),
        (b.right + PAD, b.top - PAD),
        (b.left - PAD, b.bottom + PAD),
        (b.right + PAD, b.bottom + PAD),
    ];
    let mut lo = pos2(f32::INFINITY, f32::INFINITY);
    let mut hi = pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    // The character as placed (lifted, spun), and its bounds again on the floor under it
    // (unlifted, unspun): a contact shadow, a landing's dust and falling confetti are drawn
    // there (`cantus_dance::rig::ground`), wherever the body is.
    let mut grounded = *pose;
    grounded.lift = 0.0;
    grounded.spin = 0.0;
    for p in [pose, &grounded] {
        let (t, _) = placement(&whole, ch, p);
        for (x, y) in corners {
            let mut q = tiny_skia::Point::from_xy(x, y);
            t.map_point(&mut q);
            lo = pos2(lo.x.min(q.x), lo.y.min(q.y));
            hi = pos2(hi.x.max(q.x), hi.y.max(q.y));
        }
    }
    // The whole stage's pixels start at the area's corner rounded to a pixel.
    let o = pos2(area.min.x.round(), area.min.y.round());
    (
        pos2(lo.x + o.x - 1.0, lo.y + o.y - 1.0),
        pos2(hi.x + o.x + 1.0, hi.y + o.y + 1.0),
    )
}

/// Place `ch` in `pose` on `area`, its box cut to `clip` (absolute physical pixels: what
/// can be seen). `None` when it is invisible, has no size, or is wholly outside `clip`.
pub fn place(area: &Area, clip: Rect, ch: &dyn Character, pose: &Pose, ppp: f32) -> Option<Placed> {
    if !pose.visible
        || pose.size.is_nan()
        || pose.size <= 0.0
        || area.width < 1.0
        || area.height < 1.0
    {
        return None;
    }
    let (lo, hi) = bounds_px(area, ch, pose);
    let lo = pos2(lo.x.max(clip.min.x), lo.y.max(clip.min.y));
    let hi = pos2(hi.x.min(clip.max.x), hi.y.min(clip.max.y));
    if hi.x <= lo.x || hi.y <= lo.y {
        return None;
    }
    let x0 = lo.x.floor() as i32;
    let y0 = lo.y.floor() as i32;
    let w = round_up((hi.x.ceil() as i32 - x0).max(1) as u32, BUCKET);
    let h = round_up((hi.y.ceil() as i32 - y0).max(1) as u32, BUCKET);
    let whole = area.whole();
    let ppu = (pose.size * whole.standard_height * whole.height as f32).max(0.5);
    // The feet in the whole stage's pixels, then in the box's.
    let o = pos2(area.min.x.round(), area.min.y.round());
    let fx = pose.x * whole.width as f32 + o.x - x0 as f32;
    let fy = pose.y * whole.height as f32 + o.y - y0 as f32;
    let stage = Stage {
        width: w,
        height: h,
        background: Background::Transparent,
        standard_height: ppu / (pose.size * h as f32),
        shadows: area.shadows,
    };
    let mut boxed = *pose;
    boxed.x = fx / w as f32;
    boxed.y = fy / h as f32;
    Some(Placed {
        x0,
        y0,
        stage,
        pose: boxed,
        body: body(area, ch, pose, ppp),
    })
}

/// The standing box of `ch` in `pose` on `area` (no lift, no spin), in points.
pub fn body(area: &Area, ch: &dyn Character, pose: &Pose, ppp: f32) -> Rect {
    let mut still = *pose;
    still.lift = 0.0;
    still.spin = 0.0;
    // Standing still, the grounded copy is the same box.
    let (lo, hi) = bounds_px(area, ch, &still);
    Rect::from_min_max(pos2(lo.x / ppp, lo.y / ppp), pos2(hi.x / ppp, hi.y / ppp))
}

/// The size a dancer becomes when its body's top is dragged to `top_y` (physical pixels)
/// with its feet where they are: its standing height follows the pointer.
pub fn size_for_top(area: &Area, ch: &dyn Character, pose: &Pose, top_y: f32) -> f32 {
    let feet = area.feet(pose).y;
    let tall = (-ch.bounds().top).max(0.1);
    let px = (feet - top_y).max(1.0);
    px / (tall * area.standard * area.height.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_dance::render::render;
    use cantus_dance::{cast, pose_at, DanceFrame};

    fn pose(beats: f64) -> Pose {
        let ch = cast::default_character();
        let frame = DanceFrame {
            playing: true,
            beats,
            seconds: beats * 0.5,
            ..DanceFrame::default()
        };
        pose_at(&frame, ch.as_ref(), 0.0)
    }

    /// The box drawn on its own is the whole area's drawing, cut out: the same pixels where
    /// the box is (a few edge pixels anti-aliased a little differently: the box's origin is
    /// another float), and nothing of the dancer outside it.
    #[test]
    fn a_box_draws_what_the_whole_area_draws() {
        let ch = cast::default_character();
        let area = Area {
            min: pos2(37.0, 11.0),
            width: 480.0,
            height: 360.0,
            standard: FLOAT_STANDARD,
            shadows: true,
        };
        let clip = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(2000.0, 2000.0));
        for (i, beats) in [0.0, 0.3, 1.75, 6.5].into_iter().enumerate() {
            let mut p = pose(beats);
            p.x = [0.3, 0.5, 0.71, 0.9][i];
            p.spin = [0.0, 30.0, 0.0, 200.0][i];
            p.lift = [0.0, 0.2, 0.5, 0.1][i];
            p.turn = [1.0, -1.0, 0.0, -0.4][i];
            let placed = place(&area, clip, ch.as_ref(), &p, 1.0).unwrap();
            let whole = area.whole();
            let full = render(&whole, &[(ch.as_ref(), p)]).unwrap();
            let boxed = render(&placed.stage, &[(ch.as_ref(), placed.pose)]).unwrap();
            let ox = placed.x0 - 37;
            let oy = placed.y0 - 11;
            let (mut worst, mut differ, mut outside) = (0u8, 0usize, 0usize);
            for y in 0..whole.height as i32 {
                for x in 0..whole.width as i32 {
                    let a = full.pixel(x as u32, y as u32).unwrap();
                    let (bx, by) = (x - ox, y - oy);
                    let inside = bx >= 0
                        && by >= 0
                        && (bx as u32) < placed.stage.width
                        && (by as u32) < placed.stage.height;
                    if !inside {
                        outside += usize::from(a.alpha() > 0);
                        continue;
                    }
                    let b = boxed.pixel(bx as u32, by as u32).unwrap();
                    let d = [
                        a.red().abs_diff(b.red()),
                        a.green().abs_diff(b.green()),
                        a.blue().abs_diff(b.blue()),
                        a.alpha().abs_diff(b.alpha()),
                    ]
                    .into_iter()
                    .max()
                    .unwrap_or(0);
                    worst = worst.max(d);
                    differ += usize::from(d > 0);
                }
            }
            assert_eq!(outside, 0, "pose {i}: nothing of it outside its box");
            let drawn = full.pixels().iter().filter(|c| c.alpha() > 0).count();
            assert!(
                worst <= 48 && differ * 50 < drawn,
                "pose {i}: the box draws the same pixels (worst {worst}, {differ} of {drawn} differ)"
            );
        }
    }

    /// Every character of the cast, every move, lifted to 0, 0.5 and 1.5 (a Jump, the
    /// `jump` knob): no pixel it draws within its own bounds — as placed, and on the floor
    /// under it where its shadow and dust are — falls outside its box. (A pixel drawn past
    /// the character's own `bounds()` is the cast's to fix: listed, not failed here.)
    #[test]
    fn nothing_is_drawn_outside_the_box_however_high_it_jumps() {
        let area = Area {
            min: pos2(0.0, 0.0),
            width: 400.0,
            height: 600.0,
            standard: FLOAT_STANDARD,
            shadows: true,
        };
        let clip = Rect::from_min_size(pos2(-2000.0, -2000.0), egui::vec2(5000.0, 5000.0));
        let whole = area.whole();
        let mut beyond = Vec::new();
        for ch in cast::builtin() {
            for (i, m) in cantus_dance::Move::ALL.iter().enumerate() {
                for lift in [0.0f32, 0.5, 1.5] {
                    let mut f = DanceFrame {
                        playing: true,
                        beats: 4.0 + i as f64 * 0.37,
                        seconds: 2.0 + i as f64 * 0.2,
                        bar: 1,
                        bar_start_beats: 4.0,
                        ..DanceFrame::default()
                    };
                    let k = |key: &str| {
                        cantus_core::inst::dancer::KEYS
                            .iter()
                            .position(|x| *x == key)
                            .unwrap()
                    };
                    f.values[k("move")] = (m.index() + 1) as f32;
                    f.values[k("energy")] = 1.0;
                    f.values[k("y")] = 0.95;
                    let mut p = pose_at(&f, ch.as_ref(), 0.0);
                    p.lift = lift;
                    let placed = place(&area, clip, ch.as_ref(), &p, 1.0).unwrap();
                    let full = render(&whole, &[(ch.as_ref(), p)]).unwrap();
                    let (x0, y0) = (placed.x0, placed.y0);
                    let (x1, y1) = (
                        x0 + placed.stage.width as i32,
                        y0 + placed.stage.height as i32,
                    );
                    // What the character says it fills (its bounds, unpadded) as placed and
                    // on the floor under it: the stage's box must hold all of it.
                    let mut honest = (
                        f32::INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    );
                    let mut grounded = p;
                    grounded.lift = 0.0;
                    grounded.spin = 0.0;
                    let b = ch.bounds();
                    for q in [p, grounded] {
                        let (t, _) = cantus_dance::render::placement(&whole, ch.as_ref(), &q);
                        for (x, y) in [
                            (b.left, b.top),
                            (b.right, b.top),
                            (b.left, b.bottom),
                            (b.right, b.bottom),
                        ] {
                            let mut pt = tiny_skia::Point::from_xy(x, y);
                            t.map_point(&mut pt);
                            honest = (
                                honest.0.min(pt.x),
                                honest.1.min(pt.y),
                                honest.2.max(pt.x),
                                honest.3.max(pt.y),
                            );
                        }
                    }
                    let (mut stage_misses, mut beyond_bounds) = (0, 0);
                    for y in 0..whole.height as i32 {
                        for x in 0..whole.width as i32 {
                            let a = full.pixel(x as u32, y as u32).unwrap().alpha();
                            if a == 0 || (x >= x0 && x < x1 && y >= y0 && y < y1) {
                                continue;
                            }
                            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                            if fx >= honest.0 && fx <= honest.2 && fy >= honest.1 && fy <= honest.3
                            {
                                stage_misses += 1;
                            } else {
                                beyond_bounds += 1;
                            }
                        }
                    }
                    assert_eq!(
                        stage_misses,
                        0,
                        "{} {m:?} lifted {lift}: pixels within its bounds but outside its box",
                        ch.info().id
                    );
                    if beyond_bounds > 0 {
                        // The character draws past its own `bounds()` (the cast's to fix):
                        // said, not the stage's failure.
                        beyond.push(format!(
                            "{} {m:?} lift {lift}: {beyond_bounds} px",
                            ch.info().id
                        ));
                    }
                }
            }
        }
        if !beyond.is_empty() {
            eprintln!("drawn past their own bounds (the cast's): {beyond:#?}");
        }
    }

    #[test]
    fn boxes_come_in_buckets_and_are_cut_to_what_shows() {
        let ch = cast::default_character();
        let area = Area::of(
            Rect::from_min_size(pos2(0.0, 40.0), egui::vec2(1200.0, 700.0)),
            1.5,
            FLOAT_STANDARD,
        );
        let clip = Rect::from_min_size(pos2(0.0, 60.0), egui::vec2(1800.0, 1050.0));
        let p = pose(0.0);
        let placed = place(&area, clip, ch.as_ref(), &p, 1.5).unwrap();
        assert_eq!(placed.stage.width % BUCKET, 0);
        assert_eq!(placed.stage.height % BUCKET, 0);
        // A dancer pushed off the right edge is cut there, and gone once wholly outside.
        let mut off = p;
        off.x = 1.02;
        let cut = place(&area, clip, ch.as_ref(), &off, 1.5).unwrap();
        assert!(cut.x0 as f32 + 1.0 < clip.max.x);
        assert!((cut.x0 as f32 + cut.stage.width as f32) - clip.max.x < BUCKET as f32 + 1.0);
        off.x = 1.6;
        assert!(place(&area, clip, ch.as_ref(), &off, 1.5).is_none());
        let mut hidden = p;
        hidden.visible = false;
        assert!(place(&area, clip, ch.as_ref(), &hidden, 1.5).is_none());
    }

    #[test]
    fn the_body_stays_put_while_the_dancer_jumps_and_spins() {
        let ch = cast::default_character();
        let area = Area::of(
            Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0)),
            2.0,
            DOCK_STANDARD,
        );
        let p = pose(0.0);
        let mut jumping = p;
        jumping.lift = 0.6;
        jumping.spin = 90.0;
        assert_eq!(
            body(&area, ch.as_ref(), &p, 2.0),
            body(&area, ch.as_ref(), &jumping, 2.0)
        );
        // Its feet are at its x/y share of the area, its height size × standard of it.
        let b = body(&area, ch.as_ref(), &p, 2.0);
        assert!((b.center().x - p.x * 800.0).abs() < 0.2 * 800.0 * DOCK_STANDARD);
        let unit = 600.0 * DOCK_STANDARD * p.size;
        let below = (ch.bounds().bottom + 0.03) * unit + 2.0;
        assert!(
            b.bottom() >= p.y * 600.0 && b.bottom() <= p.y * 600.0 + below,
            "its feet at the bottom: {b:?}"
        );
        // As tall as its bounds say (padded), whatever the character.
        let tall = b.height() / unit;
        let want = ch.bounds().height() + 0.06;
        assert!(
            (tall - want).abs() < 0.05,
            "{tall} units tall, its bounds {want}"
        );
    }

    #[test]
    fn dragging_the_top_sets_the_size_it_shows() {
        let ch = cast::default_character();
        let area = Area::of(
            Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0)),
            1.0,
            FLOAT_STANDARD,
        );
        let p = pose(0.0);
        let top = area.feet(&p).y + ch.bounds().top * area.px_per_unit(1.0) * 2.0;
        let s = size_for_top(&area, ch.as_ref(), &p, top);
        assert!((s - 2.0).abs() < 1e-3, "twice as tall is size 2 ({s})");
    }
}

//! Lapsus's timeline edited by hand (fx side round, ADR 0129): the curves'
//! points dragged (snapped to the grid's steps, Shift for free), a double-click adds a point
//! or takes one away, a drag on a segment bends it (its tension), and the pencil paints
//! held steps a grid cell at a time — every edit one step of undo
//! (`Command::SetLapsusPattern`, one gesture per drag).

use super::super::host::FxHost;
use super::State;
use cantus_core::fx::lapsus::{self as model, Lane, Point, MAX_POINTS, SHAPE_HOLD};
use egui::{Pos2, Rect};

/// A drag on a lane.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Drag {
    /// Point `i` (in place order) of the lane's pattern moves.
    Point(Lane, usize),
    /// The segment from point `i` bends: its tension when the drag began, the pointer then.
    Tension {
        lane: Lane,
        i: usize,
        from: f32,
        y0: f32,
    },
    /// The pencil paints steps.
    Pencil(Lane),
}

/// How near a point the pointer must be to take it, points at 100 %.
const NEAR: f32 = 8.0;

/// One lane's place on screen: its rect, how many turns of its cycle the span holds, the
/// pattern's quarter notes.
#[derive(Copy, Clone, Debug)]
pub struct Place {
    pub lane: Lane,
    pub rect: Rect,
    pub reps: usize,
    pub beats: f64,
}

impl Place {
    /// The pattern's phase (0 … 1) under `px`, and which turn of it.
    fn phase(&self, px: f32) -> f32 {
        let u = ((px - self.rect.left()) / self.rect.width()).clamp(0.0, 1.0) * self.reps as f32;
        let turn = u.floor().min(self.reps as f32 - 1.0);
        (u - turn).clamp(0.0, 1.0)
    }

    fn value(&self, py: f32) -> f32 {
        let v = (py - self.rect.top()) / self.rect.height();
        match self.lane {
            Lane::Time => v.clamp(0.0, 1.0),
            Lane::Vol => (1.0 - v).clamp(0.0, 1.0),
        }
    }

    /// Every place point `q` is drawn (once per turn).
    fn at(&self, q: &Point) -> impl Iterator<Item = Pos2> + '_ {
        let q = *q;
        (0..self.reps).map(move |r| {
            let u = (r as f32 + q.x) / self.reps as f32;
            let y = match self.lane {
                Lane::Time => q.y.clamp(0.0, 1.0),
                Lane::Vol => 1.0 - q.y.clamp(0.0, 1.0),
            };
            Pos2::new(
                self.rect.left() + self.rect.width() * u,
                self.rect.top() + self.rect.height() * y,
            )
        })
    }

    /// The grid's cells in one turn of the pattern.
    fn cells(&self, step: f64) -> usize {
        (self.beats / step.max(0.0625)).round().max(1.0) as usize
    }
}

/// The nearest point within reach of `p`, by place order.
fn nearest(pts: &[Point], place: &Place, p: Pos2, k: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, q) in pts.iter().enumerate() {
        for at in place.at(q) {
            let d = at.distance(p);
            if d <= NEAR * k && best.is_none_or(|(_, b)| d < b) {
                best = Some((i, d));
            }
        }
    }
    best.map(|(i, _)| i)
}

/// Sorted by place, as the model keeps them; where `moved` (a point's values) went.
fn sorted(mut pts: Vec<Point>, moved: Option<Point>) -> (Vec<Point>, Option<usize>) {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x));
    let at = moved.and_then(|m| pts.iter().position(|q| *q == m));
    (pts, at)
}

/// The edits a lane's response asks for, as commands through the host.
#[allow(clippy::too_many_arguments)]
pub fn lane(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    resp: &egui::Response,
    place: Place,
    slot: usize,
    k: f32,
) {
    let lane = place.lane;
    let pattern = model::pattern_of(host.spec().lapsus.as_deref(), lane, slot).clone();
    let fine = ui.input(|i| i.modifiers.shift);
    let cells = place.cells(super::GRIDS[st.grid].1);
    let snap = |u: f32| {
        if fine {
            u
        } else {
            ((u * cells as f32).round() / cells as f32).min(1.0)
        }
    };
    let set = |host: &mut dyn FxHost, p: model::Pattern, gesture: Option<u64>| {
        host.set_lapsus_pattern(lane, slot as u8, Some(p), gesture);
    };
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(resp.interact_pointer_pos()) {
            st.gesture += 1;
            st.drag = if st.tool == 1 {
                Some(Drag::Pencil(lane))
            } else {
                match nearest(&pattern.points, &place, p, k) {
                    Some(i) => Some(Drag::Point(lane, i)),
                    None if !pattern.points.is_empty() => {
                        let u = place.phase(p.x);
                        let n = pattern.points.len();
                        let j = pattern.points.partition_point(|q| q.x <= u);
                        let i = if j == 0 { n - 1 } else { j - 1 };
                        Some(Drag::Tension {
                            lane,
                            i,
                            from: pattern.points[i].t,
                            y0: p.y,
                        })
                    }
                    None => None,
                }
            };
        }
    }
    if let (Some(d), Some(p), true) = (st.drag, resp.interact_pointer_pos(), resp.dragged()) {
        match d {
            Drag::Point(l, i) if l == lane && i < pattern.points.len() => {
                let mut pts = pattern.points.clone();
                pts[i].x = snap(place.phase(p.x));
                pts[i].y = place.value(p.y);
                let moved = pts[i];
                let (pts, at) = sorted(pts, Some(moved));
                if let Some(at) = at {
                    st.drag = Some(Drag::Point(lane, at));
                }
                set(
                    host,
                    model::Pattern {
                        points: pts,
                        ..pattern.clone()
                    },
                    Some(st.gesture),
                );
            }
            Drag::Tension {
                lane: l,
                i,
                from,
                y0,
            } if l == lane && i < pattern.points.len() => {
                // Dragging up bulges the segment up, whichever way it runs.
                let mut pts = pattern.points.clone();
                let next = pts[(i + 1) % pts.len()].y;
                let rising = match lane {
                    Lane::Time => next <= pts[i].y,
                    Lane::Vol => next >= pts[i].y,
                };
                let delta = (y0 - p.y) / (80.0 * k);
                pts[i].t = (if rising { from - delta } else { from + delta }).clamp(-1.0, 1.0);
                if pts[i].s == SHAPE_HOLD {
                    pts[i].s = 0;
                }
                set(
                    host,
                    model::Pattern {
                        points: pts,
                        ..pattern.clone()
                    },
                    Some(st.gesture),
                );
            }
            Drag::Pencil(l) if l == lane => {
                let u = place.phase(p.x);
                let c = ((u * cells as f32).floor() as usize).min(cells - 1);
                let (x0, x1) = (c as f32 / cells as f32, (c + 1) as f32 / cells as f32);
                let mut pts: Vec<Point> = pattern
                    .points
                    .iter()
                    .copied()
                    .filter(|q| q.x < x0 || q.x >= x1)
                    .collect();
                if pts.len() < MAX_POINTS {
                    pts.push(Point::new(x0, place.value(p.y)).shape(SHAPE_HOLD));
                    let (pts, _) = sorted(pts, None);
                    if pts != pattern.points {
                        set(
                            host,
                            model::Pattern {
                                points: pts,
                                ..pattern.clone()
                            },
                            Some(st.gesture),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
    if resp.double_clicked() && st.tool == 0 {
        let Some(p) = resp.interact_pointer_pos() else {
            return;
        };
        let mut pts = pattern.points.clone();
        match nearest(&pts, &place, p, k) {
            Some(i) => {
                pts.remove(i);
            }
            None if pts.len() < MAX_POINTS => {
                pts.push(Point::new(snap(place.phase(p.x)), place.value(p.y)));
            }
            None => return,
        }
        let (pts, _) = sorted(pts, None);
        set(
            host,
            model::Pattern {
                points: pts,
                ..pattern.clone()
            },
            None,
        );
    }
}

/// The pattern card's length: one step.
pub fn set_len(host: &mut dyn FxHost, lane: Lane, slot: usize, len: u8) {
    let p = model::pattern_of(host.spec().lapsus.as_deref(), lane, slot).clone();
    if p.len != len {
        host.set_lapsus_pattern(lane, slot as u8, Some(model::Pattern { len, ..p }), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(lane: Lane, reps: usize) -> Place {
        Place {
            lane,
            rect: Rect::from_min_max(Pos2::new(100.0, 50.0), Pos2::new(500.0, 250.0)),
            reps,
            beats: 4.0,
        }
    }

    #[test]
    fn a_lane_maps_the_pointer_to_the_patterns_phase_and_value() {
        let p = place(Lane::Time, 1);
        assert_eq!(p.phase(300.0), 0.5);
        assert_eq!(p.value(50.0), 0.0, "the top is now");
        assert_eq!(p.value(250.0), 1.0);
        let v = place(Lane::Vol, 4);
        assert!(
            (v.phase(150.0) - 0.5).abs() < 1e-6,
            "the first of four turns"
        );
        assert_eq!(v.value(50.0), 1.0, "the top is full");
        assert_eq!(p.cells(0.25), 16);
    }

    #[test]
    fn a_point_is_found_in_every_turn_it_is_drawn() {
        let v = place(Lane::Vol, 2);
        let pts = [Point::new(0.5, 1.0)];
        // Drawn at a quarter and three quarters of the span, both at the top.
        assert_eq!(nearest(&pts, &v, Pos2::new(200.0, 50.0), 1.0), Some(0));
        assert_eq!(nearest(&pts, &v, Pos2::new(400.0, 52.0), 1.0), Some(0));
        assert_eq!(nearest(&pts, &v, Pos2::new(300.0, 52.0), 1.0), None);
    }
}

//! Cribrum's drawn curve, edited on the movement card's lane as Fluctus's is and as the
//! automation editor's clips are (the same six segment shapes and tension): drag a point
//! (Shift: off the sixteenth grid), drag a segment up or down to bend it, double-click to
//! add a point or take one away; the selected point's segment shape is chosen beside the
//! lane.

use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::State;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::{with_brush, Brush, Style};
use cantus_core::fx::cribrum::{point_keys, POINTS};
use cantus_dsp::cribrum::CribrumSettings;
use cantus_dsp::fluctus::CurvePoint;
use egui::{pos2, vec2, Pos2, Rect, Sense};

/// A drag on the curve.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Grab {
    /// Point `n` moves.
    Point(usize),
    /// The segment that starts at point `n` bends: its tension then, and the pointer's height.
    Tension { n: usize, from: f32, y0: f32 },
}

/// How near a point the pointer must be to take it, points at 100 %.
const NEAR: f32 = 9.0;
/// Where a dragged point lands without Shift: the length's sixteenths.
const GRID: f32 = 16.0;

/// The used points with their numbers, sorted by place (as the DSP sorts them).
pub fn sorted(cs: &CribrumSettings) -> ([(usize, CurvePoint); POINTS], usize) {
    let mut out = [(0usize, CurvePoint::default()); POINTS];
    let mut n = 0;
    for (i, p) in cs.curve.points.iter().enumerate() {
        if let Some(p) = p {
            out[n] = (i, *p);
            n += 1;
        }
    }
    for i in 1..n {
        let mut j = i;
        while j > 0 && out[j - 1].1.x > out[j].1.x {
            out.swap(j - 1, j);
            j -= 1;
        }
    }
    (out, n)
}

/// The plot's mapping.
#[derive(Copy, Clone, Debug)]
pub struct Map {
    pub plot: Rect,
}

impl Map {
    pub fn x(&self, u: f32) -> f32 {
        self.plot.left() + self.plot.width() * u
    }
    pub fn y(&self, v: f32) -> f32 {
        self.plot.bottom() - self.plot.height() * v.clamp(0.0, 1.0)
    }
    fn u(&self, px: f32) -> f32 {
        ((px - self.plot.left()) / self.plot.width()).clamp(0.0, 1.0)
    }
    fn v(&self, py: f32) -> f32 {
        ((self.plot.bottom() - py) / self.plot.height()).clamp(0.0, 1.0)
    }
}

fn nearest(cs: &CribrumSettings, map: Map, p: Pos2, k: f32) -> Option<usize> {
    let (pts, n) = sorted(cs);
    pts[..n]
        .iter()
        .map(|(i, q)| (*i, pos2(map.x(q.x), map.y(q.y)).distance(p)))
        .filter(|(_, d)| *d <= NEAR * k)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn segment_at(cs: &CribrumSettings, u: f32) -> Option<usize> {
    let (pts, n) = sorted(cs);
    if n == 0 {
        return None;
    }
    let i = pts[..n].partition_point(|(_, q)| q.x <= u);
    Some(if i == 0 { pts[n - 1].0 } else { pts[i - 1].0 })
}

/// The point the shape keys edit and the lane rings: the selected one, or the first.
pub fn marked(st: &State, cs: &CribrumSettings) -> Option<usize> {
    st.point
        .filter(|n| cs.curve.points[*n].is_some())
        .or_else(|| cs.curve.points.iter().position(Option::is_some))
}

/// Edit the curve on the lane's plot.
pub fn edit(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    map: Map,
    cs: &CribrumSettings,
    k: f32,
) {
    let resp = ui.interact(
        map.plot,
        ui.id().with("cribrum-curve"),
        Sense::click_and_drag(),
    );
    let fine = ui.input(|i| i.modifiers.shift);
    let place = |px: f32| {
        let u = map.u(px);
        if fine {
            u
        } else {
            ((u * GRID).round() / GRID).min(1.0)
        }
    };
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(resp.interact_pointer_pos()) {
            st.gesture += 1;
            st.grab = match nearest(cs, map, p, k) {
                Some(n) => {
                    st.point = Some(n);
                    Some(Grab::Point(n))
                }
                None => segment_at(cs, map.u(p.x)).map(|n| Grab::Tension {
                    n,
                    from: cs.curve.points[n].map_or(0.0, |q| q.tension),
                    y0: p.y,
                }),
            };
        }
    }
    if let (Some(g), Some(p), true) = (st.grab, resp.interact_pointer_pos(), resp.dragged()) {
        match g {
            Grab::Point(n) => {
                let keys = point_keys()[n];
                host.apply(
                    &[(keys[1], Some(place(p.x))), (keys[2], Some(map.v(p.y)))],
                    "cmd.cribrum_point",
                    Some(st.gesture),
                );
            }
            Grab::Tension { n, from, y0 } => {
                let (pts, count) = sorted(cs);
                let at = pts[..count].iter().position(|(i, _)| *i == n);
                let rising = at.is_none_or(|j| pts[(j + 1) % count].1.y >= pts[j].1.y);
                let delta = (y0 - p.y) / (60.0 * k);
                let t = if rising { from - delta } else { from + delta };
                let key = point_keys()[n][3];
                host.set(key, t.clamp(-1.0, 1.0), Some(key));
            }
        }
    }
    if resp.drag_stopped() && st.grab.take().is_some() {
        host.end_gesture();
    }
    if resp.clicked() {
        if let Some(n) = resp
            .interact_pointer_pos()
            .and_then(|p| nearest(cs, map, p, k))
        {
            st.point = Some(n);
        }
    }
    if resp.double_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let used = cs.curve.points.iter().flatten().count();
            match nearest(cs, map, p, k) {
                Some(n) if used > 1 => {
                    let keys = point_keys()[n];
                    host.apply(
                        &[
                            (keys[0], Some(0.0)),
                            (keys[1], None),
                            (keys[2], None),
                            (keys[3], None),
                            (keys[4], None),
                        ],
                        "cmd.cribrum_point_remove",
                        None,
                    );
                    if st.point == Some(n) {
                        st.point = None;
                    }
                }
                Some(_) => {}
                None => {
                    if let Some(n) = (0..POINTS).find(|n| cs.curve.points[*n].is_none()) {
                        let keys = point_keys()[n];
                        let shape = segment_at(cs, map.u(p.x))
                            .and_then(|s| cs.curve.points[s])
                            .map_or(0.0, |q| f32::from(q.shape));
                        host.apply(
                            &[
                                (keys[0], Some(1.0)),
                                (keys[1], Some(place(p.x))),
                                (keys[2], Some(map.v(p.y))),
                                (keys[3], Some(0.0)),
                                (keys[4], Some(shape)),
                            ],
                            "cmd.cribrum_point_add",
                            None,
                        );
                        st.point = Some(n);
                    }
                }
            }
        }
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("cribrum.curve_tip"),
            frame::tr("cribrum.curve_drag"),
            Caps::NONE,
        );
    }
}

/// The selected point's segment shape as six keys with their shapes drawn on them.
pub fn shape_keys(
    host: &mut dyn FxHost,
    st: &State,
    ui: &mut egui::Ui,
    s: &Style,
    width: f32,
    cs: &CribrumSettings,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(width * s.k, s.d(controls::ROW)), Sense::hover());
    let Some(n) = marked(st, cs) else {
        return;
    };
    let key = point_keys()[n][4];
    let now = host.spec().get(key).round().max(0.0) as usize;
    let gap = s.d(2.0);
    let w = (rect.width() - gap * 5.0) / 6.0;
    let name = frame::tr("fx.cribrum.c.s");
    for (i, seg) in SEGMENT_KEYS.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rect.left() + i as f32 * (w + gap), rect.top()),
            vec2(w, rect.height()),
        );
        let resp = ui.interact(r, ui.id().with(("cribrum-seg", i)), Sense::click());
        let state = crate::kit::State::of(&resp, false).lit(i == now);
        with_brush(ui.painter(), |b| {
            controls::paint_key(b, r, s, state, Src::Str(""));
            let ink = if i == now { s.t.text } else { s.hue };
            glyph(b, r.shrink2(vec2(4.0 * s.k, 5.0 * s.k)), i as u8, ink, s.k);
        });
        let learnt = controls::stepped_offer(ui, s, host, key, name, &resp);
        if resp.clicked() && i != now && !learnt {
            host.set(key, i as f32, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr(seg),
                frame::tr("cribrum.segment_tip"),
                Caps::NONE,
            );
        }
    }
}

const SEGMENT_KEYS: [&str; 6] = [
    "fluctus.shape.0",
    "fluctus.shape.1",
    "fluctus.shape.2",
    "fluctus.shape.3",
    "fluctus.shape.4",
    "fluctus.shape.5",
];

/// A segment of `shape` rising across a small box.
fn glyph(b: &mut Brush, r: Rect, shape: u8, ink: egui::Color32, k: f32) {
    const M: usize = 25;
    let pts: Vec<Pos2> = (0..M)
        .map(|j| {
            let u = j as f32 / (M - 1) as f32;
            let v = cantus_engine::fx::fluctus::shape_at(shape, 0.0, u.min(0.999));
            pos2(r.left() + r.width() * u, r.bottom() - r.height() * v)
        })
        .collect();
    b.polyline(&pts, 1.0 * k, ink);
}

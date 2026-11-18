//! Cedo's display answers the pointer (ADR 0141, phase B). Every edit is one
//! step of undo — a drag one gesture, a click or a double-click one command:
//!
//! - **The shape** (Tempo, MIDI): a point dragged — its place snapped to the screen's fine
//!   grid (Shift for free), its amount where the pointer is; a drag anywhere else on the plot
//!   bends the segment under it (up bulges it up, whichever way it runs); a double-click
//!   adds a point there or takes the one under it away; a right-click on a point turns the
//!   segment it starts from Curve → S → Hold → Curve (the automation editor's shapes) —
//!   `Command::SetCedoShape` through the host.
//! - **The envelope** (Sidechain): the attack's, the hold's and the release's handles
//!   dragged in time (Shift for fine).
//! - **The threshold** (Sidechain): its dashed line across the last four seconds dragged up
//!   and down, on the key's scale (−60 … 0 dBFS).
//! - **The band strip** (Lows, Highs): the crossover dragged along it; the split's word (its
//!   minimum or linear phase) clicked switches the split.
//! - **The trigger's flag** on the bed clicked: the next trigger (Sidechain → Tempo → MIDI).
//! - **The level gauge and its bellows** dragged down: a deeper duck (the depth; Shift for
//!   fine). The pilot lamp shows the hits and does nothing.
//!
//! The flag, the split and the gauge offer their parameter's menu (automation, MIDI learn)
//! as every control does.

use super::super::controls;
use super::super::frame::tr;
use super::super::host::FxHost;
use super::display::Drawn;
use super::State;
use crate::kit::hint::{self, Caps};
use crate::kit::Style;
use cantus_core::fx::cedo::{classic_pump, Shape, MAX_POINTS};
use cantus_core::fx::lapsus::{Point, SHAPE_CURVE, SHAPE_HOLD, SHAPE_S};
use cantus_core::EffectSpec;
use egui::{pos2, Pos2, Rect, Sense};

/// What a drag on the display moves.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Drag {
    /// Point `i` (in place order) of the shape.
    Point(usize),
    /// The segment from point `i` bends: its tension when the drag began, the pointer then.
    Bend {
        i: usize,
        from: f32,
        y0: f32,
    },
    /// An envelope's time: its key, its value and the pointer when the drag began, and the
    /// milliseconds a point was worth then (the scale moves as the times do).
    Env {
        key: &'static str,
        from: f32,
        x0: f32,
        ms_per_pt: f32,
    },
    Threshold,
    Crossover,
    /// The gauge: the depth and the pointer when the drag began.
    Depth {
        from: f32,
        y0: f32,
    },
}

/// How near a point or a handle the pointer must be to take it, points at 100 %.
const NEAR: f32 = 8.0;

/// The shape the spec plays: its own, or the classic pump.
pub fn shape_of(spec: &EffectSpec) -> Shape {
    spec.cedo.as_deref().cloned().unwrap_or_else(classic_pump)
}

/// The fine grid's cells in a cycle: sixteen, twelve for a triplet division.
pub fn cells(spec: &EffectSpec) -> usize {
    if matches!(spec.get("div").round() as i32, 7 | 10 | 13 | 15) {
        12
    } else {
        16
    }
}

/// Where point `q` of the shape is drawn in `plot`.
pub fn point_at(plot: Rect, q: &Point) -> Pos2 {
    pos2(
        plot.left() + plot.width() * q.x.clamp(0.0, 1.0),
        plot.top() + plot.height() * q.y.clamp(0.0, 1.0),
    )
}

fn phase(plot: Rect, x: f32) -> f32 {
    ((x - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0)
}

fn amount(plot: Rect, y: f32) -> f32 {
    ((y - plot.top()) / plot.height().max(1.0)).clamp(0.0, 1.0)
}

/// The nearest point within reach of `p`.
fn nearest(pts: &[Point], plot: Rect, p: Pos2, k: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, q) in pts.iter().enumerate() {
        let d = point_at(plot, q).distance(p);
        if d <= NEAR * k && best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Sorted by place, as the model keeps them; where `moved` went.
fn sorted(mut pts: Vec<Point>, moved: Option<Point>) -> (Vec<Point>, Option<usize>) {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x));
    let at = moved.and_then(|m| pts.iter().position(|q| *q == m));
    (pts, at)
}

/// The envelope's time scale on the plot (as `screen::envelope` draws it): the lead before
/// the hit and the span, ms.
pub fn env_scale(spec: &EffectSpec) -> (f32, f32) {
    let total =
        (spec.get("att").max(0.0) + spec.get("hold").max(0.0) + spec.get("rel").max(0.0)).max(1.0);
    let lead = total * 0.08;
    (lead, lead + total * 1.10)
}

/// The envelope's three handles (attack, hold, release): their keys and places.
pub fn env_handles(spec: &EffectSpec, plot: Rect) -> [(&'static str, Pos2); 3] {
    let (lead, span) = env_scale(spec);
    let x_of = |t: f32| plot.left() + plot.width() * ((t + lead) / span).clamp(0.0, 1.0);
    let (att, hold, rel) = (
        spec.get("att").max(0.0),
        spec.get("hold").max(0.0),
        spec.get("rel").max(0.0),
    );
    [
        ("att", pos2(x_of(att), plot.bottom())),
        ("hold", pos2(x_of(att + hold), plot.bottom())),
        ("rel", pos2(x_of(att + hold + rel), plot.top())),
    ]
}

/// The threshold's line in the history, y.
pub fn threshold_y(spec: &EffectSpec, hist: Rect) -> f32 {
    hist.bottom() - hist.height() * ((spec.get("thr") + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// The band strip's frequency at `x` (20 Hz … 20 kHz, logarithmic) and its x of `hz`.
pub fn strip_hz(strip: Rect, x: f32) -> f32 {
    let t = ((x - strip.left()) / strip.width().max(1.0)).clamp(0.0, 1.0);
    20.0 * cantus_dsp::math::powf(1_000.0, t)
}

pub fn strip_x(strip: Rect, hz: f32) -> f32 {
    strip.left()
        + strip.width() * (cantus_dsp::math::log10(hz.max(20.0) / 20.0) / 3.0).clamp(0.0, 1.0)
}

/// A shape through the host, in the gesture `g` (a drag's steps merge into one).
fn put(host: &mut dyn FxHost, points: Vec<Point>, g: Option<u64>) {
    host.set_cedo_shape(Some(Shape { points }), g);
}

/// The display's edits this frame: what `drawn` says was drawn where.
pub fn display(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, drawn: &Drawn) {
    let k = s.k;
    let spec = host.spec().clone();
    let trig = super::screen::trigger(&spec);
    let plot = drawn.screen.plot;
    let id = ui.id().with("cedo-display");
    let resp = ui.interact(plot, id.with("plot"), Sense::click_and_drag());
    if trig == 0 {
        envelope(host, st, ui, &resp, &spec, plot, k);
    } else {
        shape(host, st, ui, &resp, &spec, plot, k);
    }
    if trig == 0 {
        threshold(host, st, ui, drawn.screen.hist, id.with("thr"), k);
    }
    if let Some(strip) = drawn.screen.band {
        crossover(host, st, ui, strip, id.with("xover"));
    }
    if let Some(r) = drawn.split_word {
        let resp = ui.interact(r.expand(3.0 * k), id.with("split"), Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, "xover", tr("fx.cedo.xover"), &resp);
        if resp.clicked() && !learnt {
            let now = host.spec().get("xover").round();
            host.set("xover", if now >= 1.0 { 0.0 } else { 1.0 }, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                tr("fx.cedo.xover"),
                tr("cedo.tip.split"),
                Caps::NONE,
            );
        }
    }
    if let Some(r) = drawn.trigger_flag {
        let resp = ui.interact(r, id.with("trig"), Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, "trig", tr("fx.cedo.trig"), &resp);
        if resp.clicked() && !learnt {
            host.set("trig", ((trig + 1) % 3) as f32, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                tr("fx.cedo.trig"),
                tr("cedo.tip.trigger"),
                Caps::NONE,
            );
        }
    }
    gauge(host, st, ui, s, drawn.gauge, id.with("gauge"), k);
}

/// The shape's edits (Tempo, MIDI).
fn shape(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    resp: &egui::Response,
    spec: &EffectSpec,
    plot: Rect,
    k: f32,
) {
    let pts = shape_of(spec).points;
    let fine = ui.input(|i| i.modifiers.shift);
    let n = cells(spec) as f32;
    let snap = |u: f32| {
        if fine {
            u
        } else {
            ((u * n).round() / n).min(1.0)
        }
    };
    if resp.hovered() || resp.dragged() {
        hint::set_text(
            ui.ctx(),
            tr("cedo.d.drawn"),
            tr("cedo.tip.shape"),
            Caps::NONE,
        );
    }
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(resp.interact_pointer_pos()) {
            st.gesture += 1;
            st.drag = match nearest(&pts, plot, p, k) {
                Some(i) => Some(Drag::Point(i)),
                None if !pts.is_empty() => {
                    let u = phase(plot, p.x);
                    let j = pts.partition_point(|q| q.x <= u);
                    let i = if j == 0 { pts.len() - 1 } else { j - 1 };
                    Some(Drag::Bend {
                        i,
                        from: pts[i].t,
                        y0: p.y,
                    })
                }
                None => None,
            };
        }
    }
    if let (Some(d), Some(p), true) = (st.drag, resp.interact_pointer_pos(), resp.dragged()) {
        match d {
            Drag::Point(i) if i < pts.len() => {
                let mut v = pts.clone();
                v[i].x = snap(phase(plot, p.x));
                v[i].y = amount(plot, p.y);
                let moved = v[i];
                let (v, at) = sorted(v, Some(moved));
                if let Some(at) = at {
                    st.drag = Some(Drag::Point(at));
                }
                if v != pts {
                    put(host, v, Some(st.gesture));
                }
            }
            Drag::Bend { i, from, y0 } if i < pts.len() => {
                // Up bulges the segment up, whichever way it runs (the screen's amount grows
                // downward, as Lapsus's time lane's does).
                let mut v = pts.clone();
                let next = v[(i + 1) % v.len()].y;
                let rising = next <= v[i].y;
                let delta = (y0 - p.y) / (80.0 * k);
                v[i].t = (if rising { from - delta } else { from + delta }).clamp(-1.0, 1.0);
                if v[i].s == SHAPE_HOLD {
                    v[i].s = SHAPE_CURVE;
                }
                if v != pts {
                    put(host, v, Some(st.gesture));
                }
            }
            _ => {}
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
    if resp.double_clicked() {
        let Some(p) = resp.interact_pointer_pos() else {
            return;
        };
        let mut v = pts.clone();
        match nearest(&v, plot, p, k) {
            Some(i) => {
                v.remove(i);
            }
            None if v.len() < MAX_POINTS => {
                v.push(Point::new(snap(phase(plot, p.x)), amount(plot, p.y)));
            }
            None => return,
        }
        let (v, _) = sorted(v, None);
        put(host, v, None);
    }
    if resp.secondary_clicked() {
        let Some(p) = resp.interact_pointer_pos() else {
            return;
        };
        if let Some(i) = nearest(&pts, plot, p, k) {
            let mut v = pts.clone();
            v[i].s = match v[i].s {
                SHAPE_CURVE => SHAPE_S,
                SHAPE_S => SHAPE_HOLD,
                _ => SHAPE_CURVE,
            };
            put(host, v, None);
        }
    }
}

/// The envelope's handles (Sidechain).
fn envelope(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    resp: &egui::Response,
    spec: &EffectSpec,
    plot: Rect,
    k: f32,
) {
    let handles = env_handles(spec, plot);
    if resp.hovered() || resp.dragged() {
        hint::set_text(
            ui.ctx(),
            tr("cedo.d.envelope"),
            tr("cedo.tip.envelope"),
            Caps::NONE,
        );
    }
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(resp.interact_pointer_pos()) {
            // The nearest handle in time within reach (the release's last: it may sit on
            // the hold's when the hold is short).
            let (_, span) = env_scale(spec);
            st.drag = handles
                .iter()
                .map(|(key, at)| (*key, at.distance(p)))
                .filter(|(_, d)| *d <= 2.0 * NEAR * k)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(key, _)| Drag::Env {
                    key,
                    from: spec.get(key),
                    x0: p.x,
                    ms_per_pt: span / plot.width().max(1.0),
                });
        }
    }
    if let (
        Some(Drag::Env {
            key,
            from,
            x0,
            ms_per_pt,
        }),
        Some(p),
        true,
    ) = (st.drag, resp.interact_pointer_pos(), resp.dragged())
    {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let max = spec.param(key).map_or(5_000.0, |i| i.max);
        let v = (from + (p.x - x0) * ms_per_pt * fine).clamp(0.0, max);
        let v = (v * 10.0).round() / 10.0;
        if v != host.spec().get(key) {
            host.set(key, v, Some(key));
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
}

/// The threshold's line across the last four seconds (Sidechain).
fn threshold(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    hist: Rect,
    id: egui::Id,
    k: f32,
) {
    let y = threshold_y(host.spec(), hist);
    let band = Rect::from_min_max(
        pos2(hist.left(), y - NEAR * k),
        pos2(hist.right(), y + NEAR * k),
    );
    let resp = ui.interact(band, id, Sense::drag());
    if resp.hovered() || resp.dragged() {
        hint::set_text(
            ui.ctx(),
            tr("fx.cedo.thr"),
            tr("cedo.tip.threshold"),
            Caps::NONE,
        );
    }
    if resp.drag_started() {
        st.drag = Some(Drag::Threshold);
    }
    if let (Some(Drag::Threshold), Some(p), true) =
        (st.drag, resp.interact_pointer_pos(), resp.dragged())
    {
        let db = ((hist.bottom() - p.y) / hist.height().max(1.0) * 60.0 - 60.0).clamp(-60.0, 0.0);
        let db = (db * 10.0).round() / 10.0;
        if db != host.spec().get("thr") {
            host.set("thr", db, Some("thr"));
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
}

/// The crossover along the band strip (Lows, Highs).
fn crossover(host: &mut dyn FxHost, st: &mut State, ui: &egui::Ui, strip: Rect, id: egui::Id) {
    let resp = ui.interact(strip, id, Sense::drag());
    if resp.hovered() || resp.dragged() {
        hint::set_text(
            ui.ctx(),
            tr("fx.cedo.xhz"),
            tr("cedo.tip.crossover"),
            Caps::NONE,
        );
    }
    if resp.drag_started() {
        st.drag = Some(Drag::Crossover);
    }
    if let (Some(Drag::Crossover), Some(p), true) =
        (st.drag, resp.interact_pointer_pos(), resp.dragged())
    {
        let hz = strip_hz(strip, p.x).clamp(40.0, 2_000.0).round();
        if hz != host.spec().get("xhz") {
            host.set("xhz", hz, Some("xhz"));
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
}

/// The level gauge and its bellows: dragged down, a deeper duck.
fn gauge(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    id: egui::Id,
    k: f32,
) {
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let learnt = controls::stepped_offer(ui, s, host, "depth", tr("fx.cedo.depth"), &resp);
    if learnt {
        return;
    }
    if resp.hovered() || resp.dragged() {
        let v = host.spec().get("depth");
        hint::set(
            ui.ctx(),
            tr("fx.cedo.depth"),
            Caps::NONE,
            resp.dragged(),
            |b| {
                use std::fmt::Write as _;
                if v >= cantus_core::fx::cedo::DEPTH_INF {
                    write!(b, "\u{221e} \u{b7} {}", tr("cedo.tip.gauge"))
                } else {
                    write!(b, "{v:.1} dB \u{b7} {}", tr("cedo.tip.gauge"))
                }
            },
        );
    }
    if resp.drag_started() {
        if let Some(p) = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
        {
            st.drag = Some(Drag::Depth {
                from: host.spec().get("depth"),
                y0: p.y,
            });
        }
    }
    if let (Some(Drag::Depth { from, y0 }), Some(p), true) =
        (st.drag, resp.interact_pointer_pos(), resp.dragged())
    {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        // A quarter of a decibel a point at 100 %: the gauge's height is about 30 dB.
        let v = (from + (p.y - y0) / k * 0.25 * fine).clamp(0.0, cantus_core::fx::cedo::DEPTH_INF);
        let v = (v * 10.0).round() / 10.0;
        if v != host.spec().get("depth") {
            host.set("depth", v, Some("depth"));
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
}

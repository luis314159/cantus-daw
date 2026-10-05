//! Fervor's curve card: the transfer curve as heard, the signal over it, and — with the
//! *Curve* model — the editor of the drawn waveshaper.

use super::*;
use cantus_core::fx::saturator::{point_keys, POINTS};
use cantus_dsp::fervor::{CurveSpec, Shaper, LUT_N};

/// The display's vertical half-span: a curve may rise a little above full scale.
pub const Y_SPAN: f32 = 1.25;
/// How near a handle the pointer must be to take it, points at 100 %.
const NEAR: f32 = 9.0;

/// The curve as drawn, kept until the settings change.
#[derive(Debug)]
pub struct Drawn {
    settings: SatSettings,
    shaper: Shaper,
    lut: Box<[f32; LUT_N + 1]>,
    /// The drawn curve's knots (x, y, tension), ends included.
    knots: Vec<(f32, f32, f32)>,
    /// Which parameter each knot is: `Some(n)` for point n, `None` for an end.
    owners: Vec<Option<usize>>,
}

impl Drawn {
    fn of(ss: &SatSettings) -> Drawn {
        let (shaper, lut) = fervor::static_curve(ss);
        let (knots, owners) = knots_of(&ss.curve);
        Drawn {
            settings: ss.clone(),
            shaper,
            lut,
            knots,
            owners,
        }
    }

    fn at(&self, x: f32) -> f32 {
        self.shaper.apply(x, &self.lut[..])
    }
}

/// The knots of a drawn curve in x order, with the point each one is.
/// A knot: (x, y, tension of the segment into it).
type Knot = (f32, f32, f32);

fn knots_of(c: &CurveSpec) -> (Vec<Knot>, Vec<Option<usize>>) {
    let start = if c.symmetric { 0.0 } else { -1.0 };
    let mut v: Vec<(f32, f32, f32, Option<usize>)> = c
        .points
        .iter()
        .enumerate()
        .filter_map(|(n, p)| p.map(|p| (p.x, p.y, p.t, Some(n))))
        .filter(|(x, ..)| *x > start && *x < 1.0)
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let first = (start, if c.symmetric { 0.0 } else { c.lo }, 0.0, None);
    let last = (1.0, c.hi, c.end_t, None);
    let all: Vec<_> = std::iter::once(first)
        .chain(v)
        .chain(std::iter::once(last))
        .collect();
    (
        all.iter().map(|k| (k.0, k.1, k.2)).collect(),
        all.iter().map(|k| k.3).collect(),
    )
}

/// What a drag on the curve holds.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Grab {
    /// An inner point, by its number.
    Point(usize),
    /// The end at x = 1 (`true`) or x = −1.
    End(bool),
    /// The bend of the segment that ends at knot `knot`, from where it was and the
    /// pointer's height when the drag began.
    Tension { knot: usize, from: f32, y0: f32 },
    /// The drive, dragged up and down anywhere on an analytic model's curve.
    Drive { from: f32, y0: f32 },
}

/// The display's mapping between the curve's units and the screen.
#[derive(Copy, Clone, Debug)]
struct Map {
    rect: Rect,
}

impl Map {
    fn pos(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.rect.center().x + x * self.rect.width() * 0.5,
            self.rect.center().y - y / Y_SPAN * self.rect.height() * 0.5,
        )
    }

    fn x_of(&self, px: f32) -> f32 {
        (px - self.rect.center().x) / (self.rect.width() * 0.5)
    }

    fn y_of(&self, py: f32) -> f32 {
        -(py - self.rect.center().y) / (self.rect.height() * 0.5) * Y_SPAN
    }
}

/// The curve card: the display, and under it the drawn curve's tools or the numbers.
pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ss: &SatSettings,
) {
    if st.curve.as_ref().is_none_or(|d| d.settings != *ss) {
        st.curve = Some(Drawn::of(ss));
    }
    let k = s.k;
    let tools_h = controls::ROW + 8.0;
    let disp = (rect.height() / k - band::TITLE - tools_h - 1.0).max(40.0);
    controls::card_with(
        ui,
        s,
        rect,
        Src::Key("sat.card.curve"),
        None,
        Some(disp),
        |ui, cr| {
            if let Some(d) = cr.display {
                display(host, st, ui, s, d, tel, ss);
            }
            let w = cr.body.width() / k;
            if ss.model == Model::Curve {
                tools(host, ui, s, w);
            } else {
                numbers(ui, s, w, tel, ss);
            }
        },
    );
}

/// The display: the grid, the line of no change, the signal as a cloud, the curve, the
/// ball, and the drawn curve's handles.
fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    outer: Rect,
    tel: &[f32; 64],
    ss: &SatSettings,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(outer, ui.id().with("sat-curve"), Sense::click_and_drag());
    let side = outer.width().min(outer.height());
    let rect = Rect::from_center_size(outer.center(), vec2(side, side)).shrink(6.0 * k);
    let map = Map { rect };
    st.curve_rect = Some(rect);
    let Some(drawn) = st.curve.as_ref() else {
        return;
    };
    let editing = ss.model == Model::Curve;
    let latency = tel[telemetry::LATENCY].max(0.0) as usize;
    let ball = st.ball;
    let hover = controls::hover_in(ui, outer);
    let handles = if editing {
        handle_points(drawn, &map)
    } else {
        Vec::new()
    };
    let near = hover.and_then(|p| nearest(&handles, p, NEAR * k));
    with_brush(&ui.painter_at(outer), |b| {
        canvas::ground_b(b, outer, &ink, s.hue_tok);
        let px = b.px();
        for g in [-1.0f32, -0.5, 0.5, 1.0] {
            let a = map.pos(g, 0.0).x;
            b.vline(b.snap_mid(a), rect.top(), rect.bottom(), px, ink.grid);
            let c = map.pos(0.0, g).y;
            b.hline(rect.left(), rect.right(), b.snap_mid(c), px, ink.grid);
        }
        let o = map.pos(0.0, 0.0);
        b.vline(
            b.snap_mid(o.x),
            rect.top(),
            rect.bottom(),
            px,
            ink.grid_strong,
        );
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(o.y),
            px,
            ink.grid_strong,
        );
        // The line of no change.
        canvas::dashed(
            b,
            map.pos(-1.0, -1.0),
            map.pos(1.0, 1.0),
            px,
            ink.grid_strong,
            4.0 * k,
            3.0 * k,
        );
        // The signal: (in, out) pairs, the output lined up with the input it came from.
        let pairs = host.pairs();
        let n = pairs.len();
        if n > latency + 2 {
            let take = 1_200.min(n - latency);
            let dot = Ink::a(ink.hue, if ink.dark { 0.16 } else { 0.22 });
            for i in (n - take)..n {
                let (input, _) = pairs[i - latency];
                let (_, output) = pairs[i];
                if input.abs() > 1.0 || output.abs() > Y_SPAN {
                    continue;
                }
                let p = map.pos(input, output);
                b.rect(
                    Rect::from_center_size(p, vec2(2.0, 2.0) * k),
                    Fill::Solid(dot),
                );
            }
        }
        // The curve as heard (drive and bias in it); what it changes — the space between it
        // and the line of no change — filled.
        let mut pts = [Pos2::ZERO; 161];
        let mut same = [Pos2::ZERO; 161];
        for (i, (p, q)) in pts.iter_mut().zip(same.iter_mut()).enumerate() {
            let x = -1.0 + 2.0 * i as f32 / 160.0;
            *p = map.pos(x, drawn.at(x).clamp(-Y_SPAN, Y_SPAN));
            *q = map.pos(x, x);
        }
        b.between(&pts, &same, ink.fill(0.4));
        // With the Curve model and some drive or bias, the curve as drawn, thin, under it.
        if editing && (ss.total_drive_db() > 0.0 || ss.bias != 0.0) {
            let mut dp = [Pos2::ZERO; 161];
            for (i, p) in dp.iter_mut().enumerate() {
                let x = -1.0 + 2.0 * i as f32 / 160.0;
                *p = map.pos(x, ss.curve.eval(x));
            }
            b.polyline(&dp, 1.2 * k, Ink::a(ink.text, 0.45));
        }
        b.polyline(&pts, 2.5 * k, ink.hue);
        // Where the input's peak sits on the curve now.
        if ball > 1e-4 {
            let x = ball.min(1.0);
            for sx in [x, -x] {
                let p = map.pos(sx, drawn.at(sx).clamp(-Y_SPAN, Y_SPAN));
                b.circle(p, 5.0 * k, Fill::Solid(ink.ground));
                b.circle_stroke(p, 5.0 * k, 2.0 * k, ink.text);
            }
        }
        // The drawn curve's handles.
        for h in &handles {
            let lit = near.is_some_and(|n| n.grab == h.grab);
            let c = if lit { ink.text } else { ink.hue };
            match h.grab {
                Grab::Point(_) => b.circle(h.at, if lit { 6.5 } else { 5.0 } * k, Fill::Solid(c)),
                Grab::End(_) => b.rect(
                    b.snap_rect(Rect::from_center_size(h.at, vec2(10.0, 10.0) * k)),
                    Fill::Solid(c),
                ),
                Grab::Tension { .. } => {
                    let r = if lit { 5.0 } else { 3.5 } * k;
                    b.polygon(
                        &[
                            h.at + vec2(0.0, -r),
                            h.at + vec2(r, 0.0),
                            h.at + vec2(0.0, r),
                            h.at + vec2(-r, 0.0),
                        ],
                        Ink::a(c, 0.85),
                    );
                }
                Grab::Drive { .. } => {}
            }
        }
        // Axis words.
        canvas::put_key(
            b,
            pos2(rect.right() - 3.0 * k, o.y - 2.0 * k),
            Align2::RIGHT_BOTTOM,
            "sat.axis.in",
            Role::Micro,
            ink.dim,
            tk,
        );
        canvas::put_key(
            b,
            pos2(o.x + 4.0 * k, rect.top() + 2.0 * k),
            Align2::LEFT_TOP,
            "sat.axis.out",
            Role::Micro,
            ink.dim,
            tk,
        );
    });
    interact(host, st, ui, &resp, &map, &handles, ss, editing, k);
    if resp.hovered() {
        let (name, what) = if editing {
            ("sat.card.curve", "sat.curve_tip")
        } else {
            ("sat.card.curve", "sat.drive_tip")
        };
        hint::set_text(ui.ctx(), frame::tr(name), frame::tr(what), Caps::NONE);
    }
}

/// A handle on the display.
#[derive(Copy, Clone, Debug)]
struct Handle {
    at: Pos2,
    grab: Grab,
}

/// Every handle of the drawn curve: its inner points, its ends (the start only when it is
/// not mirrored), and the middle of every segment (its bend).
fn handle_points(d: &Drawn, map: &Map) -> Vec<Handle> {
    let mut out = Vec::new();
    let sym = d.settings.curve.symmetric;
    for (i, (x, y, _)) in d.knots.iter().enumerate() {
        let grab = match d.owners[i] {
            Some(n) => Grab::Point(n),
            None if i + 1 == d.knots.len() => Grab::End(true),
            None if !sym => Grab::End(false),
            None => continue,
        };
        out.push(Handle {
            at: map.pos(*x, *y),
            grab,
        });
    }
    for i in 1..d.knots.len() {
        let (x0, y0, _) = d.knots[i - 1];
        let (x1, y1, t) = d.knots[i];
        if (x1 - x0) * map.rect.width() < 24.0 || (y1 - y0).abs() < 1e-3 {
            continue;
        }
        let xm = 0.5 * (x0 + x1);
        let ym = y0 + (y1 - y0) * cantus_dsp::fervor::shape::warp(0.5, t as f64) as f32;
        out.push(Handle {
            at: map.pos(xm, ym),
            grab: Grab::Tension {
                knot: i,
                from: t,
                y0: 0.0,
            },
        });
    }
    out
}

fn nearest(handles: &[Handle], p: Pos2, within: f32) -> Option<Handle> {
    handles
        .iter()
        .map(|h| (h.at.distance(p), *h))
        .filter(|(d, _)| *d < within)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, h)| h)
}

/// The knot of point `n` in the drawn curve, if it is one.
fn knot_of(d: &Drawn, n: usize) -> Option<usize> {
    d.owners.iter().position(|o| *o == Some(n))
}

/// Drags, double-clicks and right-clicks on the display.
#[allow(clippy::too_many_arguments)]
fn interact(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    resp: &egui::Response,
    map: &Map,
    handles: &[Handle],
    ss: &SatSettings,
    editing: bool,
    k: f32,
) {
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(resp.interact_pointer_pos()) {
            st.gesture += 1;
            st.drag = if editing {
                nearest(handles, p, NEAR * k).map(|h| match h.grab {
                    Grab::Tension { knot, from, .. } => Grab::Tension {
                        knot,
                        from,
                        y0: p.y,
                    },
                    g => g,
                })
            } else {
                Some(Grab::Drive {
                    from: ss.drive_db,
                    y0: p.y,
                })
            };
        }
    }
    if let (Some(g), Some(p), true) = (st.drag, resp.interact_pointer_pos(), resp.dragged()) {
        let y = map.y_of(p.y).clamp(-1.0, 1.0);
        match g {
            Grab::Point(n) => {
                if let Some(d) = st.curve.as_ref() {
                    if let Some(i) = knot_of(d, n) {
                        let lo = d.knots[i - 1].0 + 0.01;
                        let hi = d.knots[i + 1].0 - 0.01;
                        let x = map.x_of(p.x).clamp(lo.min(hi), hi.max(lo));
                        let keys = point_keys()[n];
                        host.apply(
                            &[(keys[1], Some(x)), (keys[2], Some(y))],
                            "cmd.sat_point",
                            Some(st.gesture),
                        );
                    }
                }
            }
            Grab::End(high) => {
                let key = if high { "c.hi" } else { "c.lo" };
                host.set(key, y, Some(key));
            }
            Grab::Tension { knot, from, y0 } => {
                if let Some(d) = st.curve.as_ref() {
                    let rising = d.knots[knot].1 >= d.knots[knot - 1].1;
                    let dir = if rising { 1.0 } else { -1.0 };
                    let t = (from + (y0 - p.y) / (80.0 * k) * dir).clamp(-1.0, 1.0);
                    let key = d.owners[knot].map_or("c.t", |n| point_keys()[n][3]);
                    host.set(key, t, Some(key));
                }
            }
            Grab::Drive { from, y0 } => {
                let v = (from + (y0 - p.y) / (160.0 * k) * 36.0).clamp(0.0, 36.0);
                host.set("drive", v, Some("drive"));
            }
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
    if !editing {
        return;
    }
    let at = resp.interact_pointer_pos();
    let hit = at.and_then(|p| nearest(handles, p, NEAR * k));
    let remove = |host: &mut dyn FxHost, n: usize| {
        let keys = point_keys()[n];
        host.apply(
            &[
                (keys[0], None),
                (keys[1], None),
                (keys[2], None),
                (keys[3], None),
            ],
            "cmd.sat_point_remove",
            None,
        );
    };
    if resp.double_clicked() {
        match hit.map(|h| h.grab) {
            Some(Grab::Point(n)) => remove(host, n),
            Some(_) => {}
            None => {
                if let Some(p) = at {
                    let start = if ss.curve.symmetric { 0.0 } else { -1.0 };
                    let x = map.x_of(p.x);
                    let free = (0..POINTS).find(|n| ss.curve.points[*n].is_none());
                    if let (true, Some(n)) = (x > start + 0.01 && x < 0.99, free) {
                        let keys = point_keys()[n];
                        host.apply(
                            &[
                                (keys[0], Some(1.0)),
                                (keys[1], Some(x)),
                                (keys[2], Some(map.y_of(p.y).clamp(-1.0, 1.0))),
                                (keys[3], None),
                            ],
                            "cmd.sat_point_add",
                            None,
                        );
                    }
                }
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some(Grab::Point(n)) = hit.map(|h| h.grab) {
            remove(host, n);
        }
    }
}

/// The ready-made curves: a name and the points and ends they set (everything else of the
/// curve goes back to its default).
pub const SHAPES: &[(&str, &[(&str, f32)])] = &[
    ("sat.shape.line", &[]),
    (
        "sat.shape.soft",
        &[
            ("p0.on", 1.0),
            ("p0.x", 0.35),
            ("p0.y", 0.55),
            ("p1.on", 1.0),
            ("p1.x", 0.75),
            ("p1.y", 0.88),
            ("p1.t", 0.3),
            ("c.t", 0.4),
        ],
    ),
    (
        "sat.shape.hard",
        &[("p0.on", 1.0), ("p0.x", 0.5), ("p0.y", 0.5), ("c.hi", 0.5)],
    ),
    (
        "sat.shape.cross",
        &[
            ("p0.on", 1.0),
            ("p0.x", 0.45),
            ("p0.y", 0.25),
            ("p0.t", -0.6),
            ("c.t", 0.5),
        ],
    ),
    (
        "sat.shape.fold",
        &[
            ("p0.on", 1.0),
            ("p0.x", 0.5),
            ("p0.y", 0.9),
            ("p0.t", 0.4),
            ("c.hi", -0.2),
            ("c.t", 0.3),
        ],
    ),
    (
        "sat.shape.stairs",
        &[
            ("p0.on", 1.0),
            ("p0.x", 0.333),
            ("p0.y", 0.333),
            ("p0.t", 1.0),
            ("p1.on", 1.0),
            ("p1.x", 0.667),
            ("p1.y", 0.667),
            ("p1.t", 1.0),
            ("c.t", 1.0),
        ],
    ),
    (
        "sat.shape.rectify",
        &[
            ("c.sym", 0.0),
            ("c.lo", 1.0),
            ("p0.on", 1.0),
            ("p0.x", 0.0),
            ("p0.y", 0.0),
        ],
    ),
    (
        "sat.shape.asym",
        &[
            ("c.sym", 0.0),
            ("c.lo", -0.45),
            ("p0.on", 1.0),
            ("p0.x", -0.4),
            ("p0.y", -0.38),
            ("p0.t", 0.5),
            ("p1.on", 1.0),
            ("p1.x", 0.0),
            ("p1.y", 0.0),
            ("p2.on", 1.0),
            ("p2.x", 0.5),
            ("p2.y", 0.7),
            ("p2.t", 0.3),
            ("c.t", 0.3),
        ],
    ),
];

/// Apply ready-made curve `i`: every point and end the shape does not name is cleared, and
/// the model becomes *Curve* — one step.
pub fn apply_shape(host: &mut dyn FxHost, i: usize) {
    let Some((_, values)) = SHAPES.get(i) else {
        return;
    };
    let mut edits: Vec<(&str, Option<f32>)> = vec![("model", Some(6.0))];
    let curve_keys = ["c.sym", "c.lo", "c.hi", "c.t"]
        .into_iter()
        .chain(point_keys().iter().flatten().copied());
    for key in curve_keys {
        edits.push((key, values.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)));
    }
    // A sparse kind forgets what is cleared: only send what changes.
    let spec = host.spec();
    edits.retain(|(key, v)| match v {
        Some(v) => spec.get(key) != *v || !spec.params.contains_key(*key),
        None => spec.params.contains_key(*key),
    });
    if !edits.is_empty() {
        host.apply(&edits, "cmd.sat_shape", None);
    }
}

/// Under the drawn curve: mirrored or not, the ready-made curves, how many points.
fn tools(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, w: f32) {
    let gap = 4.0;
    let sym_w = 96.0;
    let count_w = 92.0;
    let field_w = (w - sym_w - count_w - 2.0 * gap - 16.0).max(80.0);
    controls::led_param(ui, s, host, "c.sym", Src::Key("fx.sat.c.sym"), sym_w);
    let id = ui.id().with("sat-shapes");
    if let Some(at) = controls::field(
        ui,
        s,
        id,
        frame::tr("sat.shapes"),
        frame::tr("sat.shapes_tip"),
        field_w,
    ) {
        let items: Vec<_> = SHAPES
            .iter()
            .map(|(key, _)| frame::action(Src::Key(key)))
            .collect();
        if let Some(i) = controls::list(ui, s, id, at, field_w, &items) {
            apply_shape(host, i);
        }
    }
    let used = point_keys()
        .iter()
        .filter(|k| host.spec().get(k[0]) >= 0.5)
        .count();
    controls::count(ui, s, "sat.points", used, count_w);
}

/// Under an analytic model's curve: the drive the curve sees, the level match, and where
/// the input peaks.
fn numbers(ui: &mut egui::Ui, s: &Style, w: f32, tel: &[f32; 64], ss: &SatSettings) {
    let k = s.k;
    let (rect, _) = ui.allocate_exact_size(vec2(w * k, s.d(controls::ROW)), Sense::hover());
    let ink = Ink::of(s);
    let tk = s.tk();
    let match_db = if ss.auto_gain {
        tel[telemetry::MATCH]
    } else {
        0.0
    };
    let peak = tel[telemetry::IN_PEAK];
    with_brush(ui.painter(), |b| {
        let mid = rect.center().y;
        let r = canvas::tag(
            b,
            pos2(rect.left() + 6.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("fx.sat.drive"),
            ink.text,
            tk,
            None,
            format_args!("{:+.1} dB", ss.total_drive_db()),
        );
        let r = canvas::tag(
            b,
            pos2(r.right() + 14.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("sat.match"),
            ink.text,
            tk,
            None,
            format_args!("{match_db:+.1} dB"),
        );
        if peak > -119.0 && r.right() + 120.0 * k < rect.right() {
            canvas::tag(
                b,
                pos2(r.right() + 14.0 * k, mid),
                Align2::LEFT_CENTER,
                frame::tr("sat.peak_in"),
                ink.text,
                tk,
                None,
                format_args!("{peak:.1} dBFS"),
            );
        }
    });
}

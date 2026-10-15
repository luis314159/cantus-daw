//! Fluctus's curve editor: the drawn level over one cycle, its points and segments edited
//! as the automation editor's clips are (ADR 0055).

use super::super::canvas::{self, Ink};
use super::super::controls::{self, Size};
use super::super::frame;
use super::super::host::FxHost;
use super::State;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Style};
use cantus_core::fx::fluctus::{point_keys, POINTS};
use cantus_dsp::fluctus::{Curve, CurvePoint, FluctusSettings};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

/// A drag on the curve.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Grab {
    /// Point `n` moves.
    Point(usize),
    /// The segment that starts at point `n` bends: its tension when the drag began, and
    /// where the pointer was.
    Tension { n: usize, from: f32, y0: f32 },
}

/// How near a point the pointer must be to take it, points at 100 %.
const NEAR: f32 = 9.0;
/// Where a dragged point lands without Shift: the cycle's sixteenths.
const GRID: f32 = 16.0;

/// The used points with their numbers, sorted by place (as the DSP sorts them).
pub fn curve_points(fs: &FluctusSettings) -> ([(usize, CurvePoint); POINTS], usize) {
    let mut out = [(0usize, CurvePoint::default()); POINTS];
    let mut n = 0;
    for (i, p) in fs.points.iter().enumerate() {
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

/// The plot's mapping between the curve's units and the screen.
#[derive(Copy, Clone, Debug)]
struct Map {
    plot: Rect,
}

impl Map {
    fn x(&self, u: f32) -> f32 {
        self.plot.left() + self.plot.width() * u
    }
    fn y(&self, v: f32) -> f32 {
        self.plot.bottom() - self.plot.height() * v
    }
    fn u(&self, px: f32) -> f32 {
        ((px - self.plot.left()) / self.plot.width()).clamp(0.0, 1.0)
    }
    fn v(&self, py: f32) -> f32 {
        ((self.plot.bottom() - py) / self.plot.height()).clamp(0.0, 1.0)
    }
}

pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    fs: &FluctusSettings,
) {
    let tempo = if tel[cantus_dsp::modfx_clock::tel::TEMPO] > 0.0 {
        tel[cantus_dsp::modfx_clock::tel::TEMPO]
    } else {
        120.0
    };
    let div = host.spec().get("div");
    if st.title.0 != div || st.title.1 != tempo || st.title.2.is_empty() {
        st.title.0 = div;
        st.title.1 = tempo;
        st.title.2.clear();
        super::title_words(div, tempo, &mut st.title.2);
    }
    controls::card(ui, s, rect, Src::Key("fluctus.card.curve"), None, |ui, cr| {
        let r = cr.body;
        let k = s.k;
        let map = Map {
            // Room above the plot for its words, so none sits on the curve.
            plot: Rect::from_min_max(
                pos2(r.left() + 12.0 * k, r.top() + 22.0 * k),
                pos2(r.right() - 12.0 * k, r.bottom() - 16.0 * k),
            ),
        };
        st.plot = Some(map.plot);
        let resp = ui.interact(map.plot, ui.id().with("fluctus-curve"), Sense::click_and_drag());
        edit(host, st, ui, &resp, map, fs, k);
        // What is drawn is what is now set (the edit above may have moved it).
        let fs = cantus_engine::fx::fluctus::settings(host.spec());
        paint(ui, s, r, map, st, tel, &fs);
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr("fluctus.curve_tip"),
                frame::tr("fluctus.curve_how"),
                Caps::NONE,
            );
        }
        if fs.depth == 0.0 {
            super::modfx::idle_note(ui, s, r, "fluctus.idle");
        }
    });
}

/// The nearest point within reach of `p`, by number.
fn nearest(fs: &FluctusSettings, map: Map, p: Pos2, k: f32) -> Option<usize> {
    let (pts, n) = curve_points(fs);
    pts[..n]
        .iter()
        .map(|(i, q)| (*i, pos2(map.x(q.x), map.y(q.y)).distance(p)))
        .filter(|(_, d)| *d <= NEAR * k)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The point whose segment holds `u`, by number.
fn segment_at(fs: &FluctusSettings, u: f32) -> Option<usize> {
    let (pts, n) = curve_points(fs);
    if n == 0 {
        return None;
    }
    let i = pts[..n].partition_point(|(_, q)| q.x <= u);
    Some(if i == 0 { pts[n - 1].0 } else { pts[i - 1].0 })
}

fn edit(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    resp: &egui::Response,
    map: Map,
    fs: &FluctusSettings,
    k: f32,
) {
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
            st.drag = match nearest(fs, map, p, k) {
                Some(n) => {
                    st.selected = Some(n);
                    Some(Grab::Point(n))
                }
                None => segment_at(fs, map.u(p.x)).map(|n| Grab::Tension {
                    n,
                    from: fs.points[n].map_or(0.0, |q| q.tension),
                    y0: p.y,
                }),
            };
        }
    }
    if let (Some(g), Some(p), true) = (st.drag, resp.interact_pointer_pos(), resp.dragged()) {
        match g {
            Grab::Point(n) => {
                let keys = point_keys()[n];
                host.apply(
                    &[(keys[1], Some(place(p.x))), (keys[2], Some(map.v(p.y)))],
                    "cmd.fluctus_point",
                    Some(st.gesture),
                );
            }
            Grab::Tension { n, from, y0 } => {
                // Dragging up bulges the segment up, whichever way it runs.
                let (pts, count) = curve_points(fs);
                let at = pts[..count].iter().position(|(i, _)| *i == n);
                let rising = at.is_none_or(|j| {
                    let next = pts[(j + 1) % count].1.y;
                    next >= pts[j].1.y
                });
                let delta = (y0 - p.y) / (80.0 * k);
                let t = if rising { from - delta } else { from + delta };
                let key = point_keys()[n][3];
                host.set(key, t.clamp(-1.0, 1.0), Some(key));
            }
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(n) = nearest(fs, map, p, k) {
                st.selected = Some(n);
            }
        }
    }
    if resp.double_clicked() {
        let Some(p) = resp.interact_pointer_pos() else {
            return;
        };
        let used = fs.points.iter().flatten().count();
        match nearest(fs, map, p, k) {
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
                    "cmd.fluctus_point_remove",
                    None,
                );
                if st.selected == Some(n) {
                    st.selected = None;
                }
            }
            Some(_) => {}
            None => {
                if let Some(n) = (0..POINTS).find(|n| fs.points[*n].is_none()) {
                    let keys = point_keys()[n];
                    host.apply(
                        &[
                            (keys[0], Some(1.0)),
                            (keys[1], Some(place(p.x))),
                            (keys[2], Some(map.v(p.y))),
                            (keys[3], Some(0.0)),
                            (keys[4], Some(0.0)),
                        ],
                        "cmd.fluctus_point_add",
                        None,
                    );
                    st.selected = Some(n);
                }
            }
        }
    }
}

fn paint(
    ui: &egui::Ui,
    s: &Style,
    r: Rect,
    map: Map,
    st: &State,
    tel: &[f32; 64],
    fs: &FluctusSettings,
) {
    let ink = Ink::of(s);
    let k = s.k;
    let curve = Curve::new(&fs.points, cantus_engine::fx::fluctus::shape_at);
    const N: usize = 321;
    let mut raw = [Pos2::ZERO; N];
    let mut heard = [Pos2::ZERO; N];
    for j in 0..N {
        let u = j as f32 / (N - 1) as f32;
        let x = map.x(u);
        raw[j] = pos2(x, map.y(curve.at(u.min(0.99999))));
        heard[j] = pos2(x, map.y(super::level_at(fs, &curve, u.min(0.99999))));
    }
    let plot = map.plot;
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        for q in 1..16 {
            let x = map.x(q as f32 / 16.0);
            let c = if q % 4 == 0 { ink.grid_strong } else { ink.grid };
            b.vline(x, plot.top(), plot.bottom(), b.px(), c);
        }
        for q in [0.25f32, 0.5, 0.75] {
            b.hline(plot.left(), plot.right(), map.y(q), b.px(), ink.grid);
        }
        b.hline(plot.left(), plot.right(), plot.top(), b.px(), ink.grid_strong);
        b.hline(plot.left(), plot.right(), plot.bottom(), b.px(), ink.grid_strong);
        // What is heard (with the depth), filled; the curve itself, the line you edit.
        b.area(&heard, plot.bottom(), ink.fill(0.7));
        b.polyline(&raw, 2.0 * k, ink.hue);
        let (pts, n) = curve_points(fs);
        for (i, p) in &pts[..n] {
            let at = pos2(map.x(p.x), map.y(p.y));
            b.dot(at, 4.0 * k, ink.hue);
            if st.selected == Some(*i) {
                b.circle_stroke(at, 7.0 * k, 1.5 * k, ink.text);
            }
        }
        if tel[cantus_dsp::modfx_clock::tel::BLOCKS] > 0.0 {
            let ph = tel[cantus_dsp::modfx_clock::tel::PHASE_L].clamp(0.0, 1.0);
            b.vline(map.x(ph), plot.top(), plot.bottom(), 1.0 * k, ink.text);
        }
        canvas::putf(
            b,
            pos2(r.right() - 6.0 * k, r.top() + 3.0 * k),
            Align2::RIGHT_TOP,
            Role::Value,
            ink.text,
            k,
            Some(ink.plate),
            format_args!("{}", st.title.2),
        );
        for (word, v) in [("100%", 1.0f32), ("0%", 0.0)] {
            let t = Text::new(word, Role::Micro, ink.dim);
            let align = if v > 0.5 { Align2::LEFT_TOP } else { Align2::LEFT_BOTTOM };
            let at = pos2(
                plot.left(),
                if v > 0.5 { r.top() + 4.0 * k } else { r.bottom() - 2.0 * k },
            );
            canvas::put(b, at, align, &t, k, None);
        }
        canvas::edge_b(b, r, &ink);
    });
}

/// The selected point's segment: its shape (the automation editor's six) and tension.
pub fn point_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    fs: &FluctusSettings,
    width: f32,
) {
    let chosen = st
        .selected
        .filter(|n| fs.points[*n].is_some())
        .or_else(|| fs.points.iter().position(Option::is_some));
    controls::card(ui, s, rect, Src::Key("fluctus.card.point"), None, |ui, _| {
        let Some(n) = chosen else {
            controls::caption(ui, s, Src::Key("fluctus.no_point"), width);
            return;
        };
        let keys = point_keys()[n];
        controls::column(ui, s, width, |ui| {
            controls::caption(ui, s, Src::Key("fluctus.segment"), width);
            let labels = SHAPE_KEYS.map(Src::Key);
            let now = host.spec().get(keys[4]).round().max(0.0) as usize;
            let name = frame::tr("fx.fluctus.p.s");
            if let Some(i) = controls::segmented(ui, s, now, &labels, name, width) {
                host.set(keys[4], i as f32, None);
            }
        });
        controls::param(ui, s, host, keys[3], Size::M);
    });
}

/// The segment shapes' short names, by index (`fluctus.shape.{i}`).
const SHAPE_KEYS: [&str; 6] = [
    "fluctus.shape.0",
    "fluctus.shape.1",
    "fluctus.shape.2",
    "fluctus.shape.3",
    "fluctus.shape.4",
    "fluctus.shape.5",
];

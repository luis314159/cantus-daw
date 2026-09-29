//! Unda's modulation sources and voice on the kit: an envelope card (pick 1–4 in its title
//! bar, drag its corners), an LFO card (pick 1–8; choose a shape, or draw one: click to add
//! a point, drag it, double-click to remove it), each with its live value at the display's
//! edge; the voice, and the output with the quality.

use super::parts::{self, c, C};
use super::{count_text, names, set_values};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::with_alpha;
use crate::kit::knob::KnobSize;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::inst::wavetable::{
    self as model, format_shape, parse_shape, LFO_POINTS, SHAPE_KEYS,
};
use cantus_core::{Command, MergeKey, NativeSpec, TrackId};
use cantus_dsp::unda::modsrc::{wave, LfoWave, Shape};
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use std::fmt::Write as _;

/// Where each segment of an envelope ends, in seconds, and the time the display spans.
fn env_points(spec: &NativeSpec, e: usize) -> ([f32; 5], f32, f32) {
    let g = |f: &str| spec.get(&format!("env{}.{f}", e + 1));
    let (dl, a, h, d, s, r) = (g("delay"), g("a"), g("h"), g("d"), g("s"), g("r"));
    let t_a = dl + a;
    let t_h = t_a + h;
    let t_d = t_h + d;
    let span_before = t_d.max(0.05);
    let hold = 0.25 * span_before.max(r * 0.5);
    let t_s = t_d + hold;
    let t_r = t_s + r;
    ([dl, t_a, t_h, t_d, t_s], t_r, s)
}

/// A title bar's segmented picker at its right: `Some(i)` when another is picked.
/// Where a title bar's picker of `n` segments, `per` points each, goes: at its right.
fn title_rect(bar: Rect, s: &Style, n: usize, per: f32) -> Rect {
    let h = s.s(18.0);
    let w = s.s(per) * n as f32;
    Rect::from_min_size(
        Pos2::new(bar.right() - s.s(6.0) - w, bar.center().y - h * 0.5),
        Vec2::new(w, h),
    )
}

const DIGITS: [Src<'static>; 8] = [
    Src::Str("1"),
    Src::Str("2"),
    Src::Str("3"),
    Src::Str("4"),
    Src::Str("5"),
    Src::Str("6"),
    Src::Str("7"),
    Src::Str("8"),
];

/// Envelope `e`'s display: its shape with the curvature the DSP uses, the three corners to
/// drag (A; D·S; R), the newest voice's level at the left edge, the time shown.
#[allow(clippy::too_many_arguments)]
fn env_display(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    d: Rect,
    e: usize,
) {
    let k = |f: &str| format!("env{}.{f}", e + 1);
    let ([dl, t_a, t_h, t_d, t_s], t_r, sus) = env_points(spec, e);
    let plot = d.shrink2(Vec2::new(s.s(10.0), s.s(10.0)));
    let span = t_r.max(1e-3);
    let at = |time: f32, level: f32| {
        Pos2::new(
            plot.left() + plot.width() * time / span,
            plot.bottom() - plot.height() * level,
        )
    };
    let shape = |c: f32, x: f32| {
        let kk = 6.0 * c;
        if kk.abs() < 1e-4 {
            x
        } else {
            ((kk * x).exp() - 1.0) / (kk.exp() - 1.0)
        }
    };
    let (ac, dc, rc) = (spec.get(&k("ac")), spec.get(&k("dc")), spec.get(&k("rc")));
    let mut pts: Vec<Pos2> = Vec::with_capacity(80);
    pts.push(at(0.0, 0.0));
    pts.push(at(dl, 0.0));
    for i in 1..=24 {
        let x = i as f32 / 24.0;
        pts.push(at(dl + (t_a - dl) * x, shape(ac, x)));
    }
    pts.push(at(t_h, 1.0));
    for i in 1..=24 {
        let x = i as f32 / 24.0;
        pts.push(at(t_h + (t_d - t_h) * x, 1.0 + (sus - 1.0) * shape(dc, x)));
    }
    pts.push(at(t_s, sus));
    for i in 1..=24 {
        let x = i as f32 / 24.0;
        pts.push(at(t_s + (t_r - t_s) * x, sus * (1.0 - shape(rc, x))));
    }
    let handles = [at(t_a, 1.0), at(t_d, sus), at(t_r, 0.0)];
    let resp = ui.interact(d, ui.id().with(("unda-env", e)), Sense::click_and_drag());
    let tel = app.inst_telemetry();
    let live = super::live(app);
    with_brush(&ui.painter_at(d), |b| {
        b.hline(
            d.left(),
            d.right(),
            b.snap_mid(plot.bottom()),
            b.px(),
            s.t.grid,
        );
        b.area(
            &pts,
            plot.bottom(),
            with_alpha(s.hue, if s.t.dark { 0.20 } else { 0.14 }),
        );
        b.polyline(&pts, s.s(1.8), s.hue);
        for (i, h) in handles.iter().enumerate() {
            b.circle(*h, s.s(4.5), Fill::Solid(s.hue));
            let label = ["A", "D\u{b7}S", "R"][i];
            // Beside the corner, kept inside the display: left of it near the right edge,
            // below it near the top.
            let (w, lh) = (s.s(28.0), s.s(12.0));
            let right = h.x + s.s(6.0) + w > d.right() - s.s(2.0);
            let x = if right {
                h.x - s.s(6.0) - w
            } else {
                h.x + s.s(6.0)
            };
            let y = if h.y - s.s(16.0) < d.top() + s.s(2.0) {
                h.y + s.s(4.0)
            } else {
                h.y - s.s(16.0)
            };
            parts::say(
                b,
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, lh)),
                s,
                label,
                Role::Micro,
                s.t.text_dim,
                if right {
                    Align2::RIGHT_CENTER
                } else {
                    Align2::LEFT_CENTER
                },
            );
        }
        if live {
            let lv = tel[cantus_engine::inst::wavetable::tel::ENV + e];
            let y = at(0.0, lv).y;
            b.segment(
                Pos2::new(d.left(), y),
                Pos2::new(d.left() + s.s(14.0), y),
                s.s(2.0),
                s.t.highlight,
                false,
            );
        }
        let mut span_txt = String::new();
        crate::views::inst::write_readout(span, "s", &mut span_txt);
        parts::chip(
            b,
            Rect::from_min_size(
                Pos2::new(d.right() - s.s(106.0), d.top() + s.s(5.0)),
                Vec2::new(s.s(100.0), s.s(16.0)),
            ),
            s,
            &tf("unda.env_span", &[("s", &span_txt)]),
            s.t.text_dim,
            Align2::RIGHT_CENTER,
        );
    });
    // Drag the nearest corner.
    let id = ui.id().with(("unda-env-grab", e));
    if resp.drag_started() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let near = (0..3).min_by(|a, b| {
                handles[*a]
                    .distance(pos)
                    .partial_cmp(&handles[*b].distance(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            ui.data_mut(|d| d.insert_temp(id, near.unwrap_or(0)));
        }
    }
    if resp.dragged() {
        let grab: usize = ui.data(|d| d.get_temp(id)).unwrap_or(0);
        let secs_per_px = span / plot.width().max(1.0);
        let dx = resp.drag_delta().x * secs_per_px;
        let g = |f: &str| spec.get(&k(f));
        let gesture = format!("env{e}");
        match grab {
            0 => set_values(
                app,
                track,
                &gesture,
                &[(&k("a"), (g("a") + dx).clamp(0.0005, 20.0))],
            ),
            1 => {
                let sv = (g("s") - resp.drag_delta().y / plot.height()).clamp(0.0, 1.0);
                set_values(
                    app,
                    track,
                    &gesture,
                    &[(&k("d"), (g("d") + dx).clamp(0.001, 30.0)), (&k("s"), sv)],
                );
            }
            _ => set_values(
                app,
                track,
                &gesture,
                &[(&k("r"), (g("r") + dx).clamp(0.001, 30.0))],
            ),
        }
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    parts::display_hint(ui, &resp, "unda.env_tip", "");
}

/// An envelope card: pick 1–4 in the title bar; its shape; its nine cells.
pub fn env_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let e = app.inst.wavetable.env.min(3);
    let cells_h = parts::cell_size(KnobSize::S, s).y;
    let disp = (rect.height() - cells_h - s.s(24.0 + 1.0 + 16.0)) / s.k;
    let title = tf("unda.env_title", &[("n", &(e + 1).to_string())]);
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Str(&title),
        None,
        None,
        Some(disp.max(40.0)),
    );
    let pr = title_rect(cr.title, s, 4, 26.0);
    let name = t("unda.env_pick");
    if let Some(i) = parts::seg(
        ui,
        ui.id().with("unda-env-pick"),
        pr,
        s,
        &DIGITS[..4],
        e,
        &name,
    ) {
        app.inst.wavetable.env = i;
    }
    if let Some(d) = cr.display {
        env_display(app, ui, track, spec, s, d, e);
    }
    let fields = ["delay", "a", "h", "d", "s", "r", "ac", "dc", "rc"];
    let keys: Vec<String> = fields.iter().map(|f| format!("env{}.{f}", e + 1)).collect();
    let list: Vec<C<'_>> = keys.iter().map(|k| c(k)).collect();
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        parts::below_display(&cr, s),
        &list,
    );
}

/// Play's amplifier: envelope 1's shape and its four stages, with the velocity.
pub fn amp_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cells_h = parts::cell_size(KnobSize::M, s).y;
    let disp = (rect.height() - cells_h - s.s(24.0 + 1.0 + 16.0)) / s.k;
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Key("unda.amp_env"),
        None,
        None,
        Some(disp.max(36.0)),
    );
    if let Some(d) = cr.display {
        env_display(app, ui, track, spec, s, d, 0);
    }
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::M,
        parts::below_display(&cr, s),
        &[
            c("env1.a"),
            c("env1.d"),
            c("env1.s"),
            c("env1.r"),
            c("voice.vel"),
        ],
    );
}

/// A LFO's drawn shape, or the default one.
fn shape_of(spec: &NativeSpec, l: usize) -> Shape {
    let mut pts = [(0.0, 0.0); LFO_POINTS];
    match spec.text(SHAPE_KEYS[l]).map(|s| parse_shape(s, &mut pts)) {
        Some(n) if n > 0 => Shape {
            points: pts,
            len: n,
        },
        _ => Shape::default(),
    }
}

/// A LFO card: pick 1–8 in the title bar; its shape (drawn when "Drawn"), its live value at
/// the display's left edge; its rate, phase, delay, rise and smoothing, its shape and mode.
pub fn lfo_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let l = app.inst.wavetable.lfo.min(7);
    let k = |f: &str| format!("lfo{}.{f}", l + 1);
    let cells_h = parts::cell_size(KnobSize::S, s).y;
    let disp = (rect.height() - cells_h - s.s(24.0 + 1.0 + 16.0)) / s.k;
    let title = tf("unda.lfo_title", &[("n", &(l + 1).to_string())]);
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Str(&title),
        None,
        None,
        Some(disp.max(40.0)),
    );
    let pr = title_rect(cr.title, s, DIGITS.len(), 22.0);
    let name = t("unda.lfo_pick");
    if let Some(i) = parts::seg(ui, ui.id().with("unda-lfo-pick"), pr, s, &DIGITS, l, &name) {
        app.inst.wavetable.lfo = i;
    }
    let w = LfoWave::from_index(spec.get(&k("wave")));
    let shape = shape_of(spec, l);
    let drawn = w == LfoWave::Drawn;
    if let Some(d) = cr.display {
        let inner = d.shrink2(Vec2::new(s.s(10.0), s.s(10.0)));
        let resp = ui.interact(d, ui.id().with(("unda-lfo", l)), Sense::click_and_drag());
        let tel = app.inst_telemetry();
        let live = super::live(app);
        with_brush(&ui.painter_at(d), |b| {
            paint_lfo(
                b,
                s,
                d,
                inner,
                w,
                &shape,
                live.then(|| tel[cantus_engine::inst::wavetable::tel::LFO + l]),
            )
        });
        if drawn {
            edit_shape(app, ui, track, l, &shape, &resp, inner);
        }
        parts::display_hint(
            ui,
            &resp,
            if drawn {
                "unda.lfo_draw_tip"
            } else {
                "unda.lfo_pick_tip"
            },
            "",
        );
    }
    let body = parts::below_display(&cr, s);
    let fields = ["rate", "phase", "delay", "rise", "smooth"];
    let keys: Vec<String> = fields.iter().map(|f| k(f)).collect();
    let deg = |v: f32, b: &mut String| {
        let _ = write!(b, "{v:.0}°");
    };
    let list: Vec<C<'_>> = keys
        .iter()
        .zip(fields)
        .map(|(key, f)| match f {
            "rate" => c(key).fmt(&super::rate_s),
            "phase" => c(key).fmt(&deg),
            _ => c(key),
        })
        .collect();
    let cw = parts::row_width(list.len(), KnobSize::S, s);
    let cells_r = Rect::from_min_size(body.min, Vec2::new(cw, body.height()));
    parts::cells(app, ui, track, spec, s, KnobSize::S, cells_r, &list);
    let fx = cells_r.right() + s.s(12.0);
    let fh = s.s(20.0);
    let col = Rect::from_min_max(Pos2::new(fx, body.top()), body.max);
    let y0 = col.center().y - fh - s.s(2.0);
    let r1 = Rect::from_min_size(Pos2::new(col.left(), y0), Vec2::new(col.width(), fh));
    let r2 = Rect::from_min_size(
        Pos2::new(col.left(), y0 + fh + s.s(4.0)),
        Vec2::new(col.width(), fh),
    );
    parts::choice_key(
        app,
        ui,
        track,
        spec,
        &k("wave"),
        r1,
        s,
        &names("unda.lfo", model::LFO_WAVES),
    );
    parts::choice_key(
        app,
        ui,
        track,
        spec,
        &k("mode"),
        r2,
        s,
        &names("unda.lfo_mode", model::LFO_MODES),
    );
}

/// A LFO's picture: quarters, the axis, the curve (random shapes drawn with fixed example
/// values, since each note draws its own), the drawn points, and the live value.
fn paint_lfo(
    b: &mut Brush,
    s: &Style,
    d: Rect,
    inner: Rect,
    w: LfoWave,
    shape: &Shape,
    now: Option<f32>,
) {
    let at = |x: f32, v: f32| {
        Pos2::new(
            inner.left() + inner.width() * x,
            inner.center().y - inner.height() * 0.5 * v,
        )
    };
    for q in 1..4 {
        let x = b.snap_mid(inner.left() + inner.width() * q as f32 / 4.0);
        b.vline(x, d.top(), d.bottom(), b.px(), s.t.grid);
    }
    b.hline(
        d.left(),
        d.right(),
        b.snap_mid(inner.center().y),
        b.px(),
        s.t.grid_strong,
    );
    let mut pts = [Pos2::ZERO; 161];
    for (i, p) in pts.iter_mut().enumerate() {
        let x = i as f32 / 160.0;
        let seg = (x * 4.0).floor();
        let r = |n: f32| ((n * 12.9898).sin() * 43_758.547).fract() * 2.0 - 1.0;
        let v = match w {
            LfoWave::SampleHold | LfoWave::SmoothRandom => {
                let local = (x * 4.0).fract();
                wave(w, local, r(seg + 1.0), r(seg + 2.0), shape)
            }
            _ => wave(w, x.min(0.9999), 0.0, 0.0, shape),
        };
        *p = at(x, v);
    }
    b.area(
        &pts,
        inner.center().y,
        with_alpha(s.hue, if s.t.dark { 0.16 } else { 0.12 }),
    );
    b.polyline(&pts, s.s(1.8), s.hue);
    if w == LfoWave::Drawn {
        for i in 0..shape.len {
            let (x, y) = shape.points[i];
            b.circle(at(x, 2.0 * y - 1.0), s.s(4.0), Fill::Solid(s.hue));
        }
    }
    if let Some(v) = now {
        let y = at(0.0, v).y;
        b.segment(
            Pos2::new(d.left(), y),
            Pos2::new(d.left() + s.s(14.0), y),
            s.s(2.0),
            s.t.highlight,
            false,
        );
    }
}

/// Click to add a point, drag one to move it, double-click to remove it: each gesture is
/// one undo step on the shape's text.
fn edit_shape(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    l: usize,
    shape: &Shape,
    resp: &egui::Response,
    inner: Rect,
) {
    let to_xy = |p: Pos2| {
        (
            ((p.x - inner.left()) / inner.width()).clamp(0.0, 1.0),
            (0.5 - (p.y - inner.center().y) / inner.height()).clamp(0.0, 1.0),
        )
    };
    let mut pts: Vec<(f32, f32)> = shape.points[..shape.len].to_vec();
    let near = |p: Pos2, pts: &[(f32, f32)]| {
        pts.iter().position(|(x, y)| {
            let q = Pos2::new(
                inner.left() + inner.width() * x,
                inner.center().y - inner.height() * (y - 0.5),
            );
            q.distance(p) < 8.0
        })
    };
    let id = ui.id().with(("unda-lfo-grab", l));
    let mut changed = false;
    if resp.double_clicked() {
        if let Some(i) = resp.interact_pointer_pos().and_then(|p| near(p, &pts)) {
            if pts.len() > 2 {
                pts.remove(i);
                changed = true;
            }
        }
    } else if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            if near(p, &pts).is_none() && pts.len() < LFO_POINTS {
                let xy = to_xy(p);
                let at = pts.iter().position(|q| q.0 > xy.0).unwrap_or(pts.len());
                pts.insert(at, xy);
                changed = true;
            }
        }
    }
    if resp.drag_started() {
        let grab = resp.interact_pointer_pos().and_then(|p| near(p, &pts));
        ui.data_mut(|d| d.insert_temp(id, grab));
    }
    if resp.dragged() {
        if let (Some(Some(i)), Some(p)) = (
            ui.data(|d| d.get_temp::<Option<usize>>(id)),
            resp.interact_pointer_pos(),
        ) {
            let (mut x, y) = to_xy(p);
            let lo = if i > 0 { pts[i - 1].0 } else { 0.0 };
            let hi = if i + 1 < pts.len() { pts[i + 1].0 } else { 1.0 };
            x = x.clamp(lo, hi);
            pts[i] = (x, y);
            changed = true;
        }
    }
    if changed {
        app.exec_merging(
            Command::SetInstrumentText {
                track,
                key: SHAPE_KEYS[l].into(),
                text: Some(format_shape(&pts)),
            },
            MergeKey::of(&("unda-shape", track.0, l)),
        );
        if !resp.dragged() {
            app.end_gesture();
        }
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
}

/// The voice: how notes become voices — mode, priority, when to glide, and six cells.
pub fn voice_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("unda.voice"), None, None, None);
    let body = cr.body;
    let fw = s.s(210.0).min(body.width() * 0.4);
    let fh = s.s(20.0);
    let y0 = body.center().y - (fh * 3.0 + s.s(8.0)) * 0.5;
    let rows = [
        (
            "unda.mode",
            "voice.mode",
            names("unda.mode", model::VOICE_MODES),
        ),
        (
            "unda.prio",
            "voice.prio",
            names("unda.prio", &["last", "low", "high"]),
        ),
        (
            "unda.glide",
            "voice.glide_mode",
            names("unda.glide_mode", &["legato", "always"]),
        ),
    ];
    for (i, (label, key, list)) in rows.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(body.left(), y0 + i as f32 * (fh + s.s(4.0))),
            Vec2::new(fw, fh),
        );
        let lw = s.s(62.0);
        with_brush(ui.painter(), |b| {
            parts::row_label(
                b,
                Rect::from_min_size(r.min, Vec2::new(lw - s.s(6.0), fh)),
                s,
                label,
            )
        });
        let f = Rect::from_min_max(Pos2::new(r.left() + lw, r.top()), r.max);
        parts::choice_key(app, ui, track, spec, key, f, s, list);
    }
    let cells_r = Rect::from_min_max(
        Pos2::new(body.left() + fw + s.s(14.0), body.top()),
        body.max,
    );
    let pct = |v: f32, b: &mut String| {
        let _ = write!(b, "{:.0} %", v * 100.0);
    };
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        cells_r,
        &[
            c("voice.poly").fmt(&count_text),
            c("voice.glide"),
            c("voice.bend"),
            c("voice.vel").fmt(&pct),
            c("voice.drift").fmt(&pct),
            c("voice.seed").fmt(&count_text),
        ],
    );
}

/// The output: the gain, the oversampling, and how many unison lanes run.
pub fn output_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("unda.output"), None, None, None);
    let body = cr.body;
    let cw = parts::cell_size(KnobSize::S, s).x;
    parts::cells(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        Rect::from_min_size(body.min, Vec2::new(cw, body.height())),
        &[c("gain").fmt(&super::db_s).label(Src::Key("unda.gain"))],
    );
    let col = Rect::from_min_max(
        Pos2::new(body.left() + cw + s.s(10.0), body.top()),
        body.max,
    );
    let fh = s.s(20.0);
    let lw = s.s(56.0);
    let y0 = col.center().y - fh - s.s(2.0);
    let r1 = Rect::from_min_size(Pos2::new(col.left(), y0), Vec2::new(col.width(), fh));
    let r2 = Rect::from_min_size(
        Pos2::new(col.left(), y0 + fh + s.s(4.0)),
        Vec2::new(col.width(), fh),
    );
    let lanes = app.inst_telemetry()[cantus_engine::inst::wavetable::tel::LANES];
    let n = format!("{}", lanes as u32);
    with_brush(ui.painter(), |b| {
        parts::row_label(
            b,
            Rect::from_min_size(r1.min, Vec2::new(lw - s.s(6.0), fh)),
            s,
            "unda.quality",
        );
        parts::row_label(
            b,
            Rect::from_min_size(r2.min, Vec2::new(lw - s.s(6.0), fh)),
            s,
            "unda.lanes_label",
        );
        crate::kit::well::readout(
            b,
            Rect::from_min_max(
                Pos2::new(r2.left() + lw, r2.top()),
                Pos2::new(r2.left() + lw + s.s(48.0), r2.bottom()),
            ),
            s,
            crate::kit::State::REST,
            &n,
            true,
        );
    });
    let q = [
        Src::Key("unda.q.1x"),
        Src::Key("unda.q.2x"),
        Src::Key("unda.q.4x"),
    ];
    parts::seg_key(
        app,
        ui,
        track,
        spec,
        "quality",
        Rect::from_min_max(Pos2::new(r1.left() + lw, r1.top()), r1.max),
        s,
        &q,
    );
}

//! The level history: input as a dim fill, output over it in the device's hue, the gain
//! reduction hanging from the top in the danger colour, all on one dB scale; bars and
//! beats behind; the threshold as a dashed line you drag. Click to freeze, wheel to scroll
//! back, hover to read any moment. Its own controls — speed, bars, freeze, scale — sit in
//! a strip at its top.

use super::*;
use crate::views::fx::canvas::{self, Ink};

/// The history's display `rect`: its strip of controls, then the plot.
pub(super) fn show(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    cs: &CompSettings,
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let head = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + HISTORY_HEAD * k));
    let plot = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max);
    let resp = ui.interact(plot, ui.id().with("comp-history"), Sense::click_and_drag());
    let range = st.range_db;
    st.geometry.history = Some(plot);
    st.geometry.range_db = range;
    let y = |d: f32| plot.top() + (-d / range).clamp(0.0, 1.0) * plot.height();
    let d_of = |py: f32| -(py - plot.top()) / plot.height() * range;
    let (a, b_end) = st.visible();
    let cols = b_end - a;
    let slots = if st.speed == Speed::Whole {
        cols.max(2)
    } else {
        VISIBLE
    };
    let dx = plot.width() / slots as f32;
    let x_of = |i: usize| plot.right() - dx * (cols - i) as f32;
    let ty = y(cs.threshold_db);
    let hover = resp.hover_pos();
    let near = hover.is_some_and(|pt| (pt.y - ty).abs() < 6.0 * k);
    let held = st.drag == Some(Grab::Threshold);

    // The columns as points, folded to about one a pixel (the whole take can hold four
    // thousand): input, output, reduction.
    let ppp = ui.ctx().pixels_per_point();
    let max_pts = ((plot.width() * ppp) as usize).max(2);
    let stride = cols.div_ceil(max_pts).max(1);
    let n = cols.div_ceil(stride);
    st.pts.clear();
    st.pts.resize(3 * n, Pos2::ZERO);
    let mut max_gr = 0.0f32;
    for j in 0..n {
        let (i0, i1) = (j * stride, ((j + 1) * stride).min(cols));
        let mut c = st.history[a + i0];
        for i in i0 + 1..i1 {
            c = c.fold(st.history[a + i]);
        }
        let x = x_of(i1 - 1);
        st.pts[j] = pos2(x, y(c.inp));
        st.pts[n + j] = pos2(x, y(c.out));
        st.pts[2 * n + j] = pos2(x, y(-c.gr));
        max_gr = max_gr.max(c.gr);
    }
    let last_gr = st.history.back().map_or(0.0, |h| h.gr);
    let step = if range <= 36.0 { 6.0 } else { 12.0 };
    let (bar_len, beat_len) = st.beats;
    // What the history is doing, when it is not simply running.
    let lang = crate::i18n::locale();
    let state_text: Option<&str> = if st.frozen {
        Some(frame::tr("comp.frozen"))
    } else if st.scroll > 0 {
        let n = st.scroll;
        Some(cached(&mut st.words.scrolled, (n, lang), || {
            tf("comp.scrolled", &[("n", &n)])
        }))
    } else if st.speed == Speed::Whole {
        let n = st.whole_frames;
        Some(cached(&mut st.words.whole, (n, lang), || {
            tf("comp.whole_state", &[("n", &n)])
        }))
    } else {
        None
    };
    let history = &st.history;
    let pts = &st.pts;
    let grid = st.grid;
    let dragging = st.drag.is_some();

    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        b.hline(
            head.left(),
            head.right(),
            b.snap_mid(head.bottom()),
            px,
            ink.grid,
        );
        let mut g = -step;
        while g > -range {
            b.hline(plot.left(), plot.right(), b.snap_mid(y(g)), px, ink.grid);
            g -= step;
        }
        // Bars and beats from the transport, behind everything.
        let bar_at = |i: usize| -> Option<i64> {
            let (p0, p1) = (history[a + i - 1].tick?, history[a + i].tick?);
            (p1.div_euclid(bar_len) != p0.div_euclid(bar_len)).then_some(p1.div_euclid(bar_len))
        };
        if grid {
            for i in 1..cols {
                let (Some(p0), Some(p1)) = (history[a + i - 1].tick, history[a + i].tick) else {
                    continue;
                };
                let x = b.snap_mid(x_of(i));
                if p1.div_euclid(bar_len) != p0.div_euclid(bar_len) {
                    b.vline(x, plot.top(), plot.bottom(), px, ink.grid_strong);
                } else if p1.div_euclid(beat_len) != p0.div_euclid(beat_len) {
                    b.vline(x, plot.top(), plot.bottom(), px, ink.grid);
                }
            }
        }
        if n > 1 {
            let (inp, rest) = pts.split_at(n);
            let (out, gr) = rest.split_at(n);
            // Light fills: the lines carry the reading, the fills only say which side
            // is signal (the output over the input's grey).
            b.area(
                inp,
                plot.bottom(),
                Ink::a(ink.dim, if ink.dark { 0.10 } else { 0.12 }),
            );
            b.area(out, plot.bottom(), ink.fill(0.3));
            b.area(
                gr,
                plot.top(),
                Ink::a(ink.danger, if ink.dark { 0.30 } else { 0.22 }),
            );
            b.polyline(out, 1.25 * k, ink.hue);
            b.polyline(gr, 1.5 * k, ink.danger);
        }
        // The threshold: dashed, heavier while near or held.
        canvas::dashed(
            b,
            pos2(plot.left(), ty),
            pos2(plot.right(), ty),
            if held || near { 2.0 } else { 1.25 } * k,
            ink.hue,
            6.0 * k,
            4.0 * k,
        );

        // The words last, so nothing is painted over them; none lands on another.
        let plate = Some(ink.plate);
        let mut g = -step;
        while g > -range {
            canvas::putf(
                b,
                pos2(plot.left() + 4.0 * k, y(g) - 2.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                plate,
                format_args!("{g:.0}"),
            );
            g -= step;
        }
        // The reduction now and at most, at the bottom right — or the top right when the
        // threshold's own tag is down there.
        let low = ty > plot.bottom() - 34.0 * k;
        let tag_at = if low {
            pos2(plot.right() - 8.0 * k, plot.top() + 6.0 * k)
        } else {
            pos2(plot.right() - 8.0 * k, plot.bottom() - 4.0 * k)
        };
        let tag = canvas::putf(
            b,
            tag_at,
            if low {
                Align2::RIGHT_TOP
            } else {
                Align2::RIGHT_BOTTOM
            },
            Role::Body,
            ink.danger,
            tk,
            plate,
            format_args!(
                "{} {last_gr:.1} dB  \u{b7}  {} {max_gr:.1} dB",
                frame::tr("comp.gr"),
                frame::tr("comp.max"),
            ),
        );
        canvas::putf(
            b,
            pos2(plot.right() - 8.0 * k, ty - 3.0 * k),
            Align2::RIGHT_BOTTOM,
            Role::Value,
            ink.hue,
            tk,
            plate,
            format_args!("{:+.1} dB", cs.threshold_db),
        );
        // Bar numbers along the bottom, clear of the reduction's tag.
        if grid {
            let mut ruler = canvas::Ruler::new();
            for i in 1..cols {
                let Some(bar) = bar_at(i) else {
                    continue;
                };
                let at = pos2(x_of(i) + 2.0 * k, plot.bottom() - 2.0 * k);
                if !ruler.fits(at.x, 4.0 * k) || (!low && at.x + 16.0 * k > tag.left()) {
                    continue;
                }
                let r = canvas::putf(
                    b,
                    at,
                    Align2::LEFT_BOTTOM,
                    Role::Micro,
                    ink.dim,
                    tk,
                    None,
                    format_args!("{}", bar + 1),
                );
                ruler.placed(r);
            }
        }
        if let Some(text) = state_text {
            canvas::put(
                b,
                pos2(plot.left() + 28.0 * k, plot.top() + 6.0 * k),
                Align2::LEFT_TOP,
                &Text::new(text, Role::Body, ink.hue).caps(false),
                tk,
                plate,
            );
        }
        if cols == 0 {
            canvas::put_key(
                b,
                plot.center(),
                Align2::CENTER_CENTER,
                "comp.history_empty",
                Role::Body,
                ink.dim,
                tk,
            );
        }
        // Hover: read any moment.
        if let Some(pt) = hover.filter(|p| plot.contains(*p)) {
            if !near && cols > 0 && !dragging {
                let i = (((pt.x - x_of(0)) / dx).round().max(0.0) as usize).min(cols - 1);
                let c = history[a + i];
                b.vline(
                    b.snap_mid(x_of(i)),
                    plot.top(),
                    plot.bottom(),
                    px,
                    ink.grid_strong,
                );
                let right = pt.x > plot.center().x;
                let at = pos2(
                    if right {
                        pt.x - 10.0 * k
                    } else {
                        pt.x + 10.0 * k
                    },
                    plot.top() + if state_text.is_some() { 30.0 } else { 6.0 } * k,
                );
                let align = if right {
                    Align2::RIGHT_TOP
                } else {
                    Align2::LEFT_TOP
                };
                let (inp, out) = (c.inp.max(-99.9), c.out.max(-99.9));
                let (w_in, w_out, w_gr) = (
                    frame::tr("comp.read_in"),
                    frame::tr("comp.read_out"),
                    frame::tr("comp.gr"),
                );
                match c.tick {
                    Some(t) => canvas::putf(
                        b,
                        at,
                        align,
                        Role::Body,
                        ink.text,
                        tk,
                        plate,
                        format_args!(
                            "{w_in} {inp:.1}  {w_out} {out:.1}  {w_gr} {:.1} dB  \u{b7}  {}.{}",
                            c.gr,
                            t.div_euclid(bar_len) + 1,
                            t.rem_euclid(bar_len) / beat_len + 1,
                        ),
                    ),
                    None => canvas::putf(
                        b,
                        at,
                        align,
                        Role::Body,
                        ink.text,
                        tk,
                        plate,
                        format_args!("{w_in} {inp:.1}  {w_out} {out:.1}  {w_gr} {:.1} dB", c.gr),
                    ),
                };
            }
        }
    });
    head_controls(st, ui, s, head);
    gestures(host, st, ui, &resp, (ty, 6.0 * k, near), dx, &d_of);
}
/// The history's own controls, in the strip at its top: how it moves (Fast, Slow, the
/// whole take), bars behind it, freeze, back to live when scrolled, and its scale.
fn head_controls(st: &mut State, ui: &mut egui::Ui, s: &Style, head: Rect) {
    let inner = head.shrink2(vec2(s.s(8.0), s.s(4.0)));
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    gap_x(&mut ui, s);
    let w = inner.width() / s.k;
    // Narrow (a small window): the speed takes less, the scale's legend goes.
    let roomy = w >= 560.0;
    let labels = Speed::KEYS.map(Src::Key);
    if let Some(i) = controls::segmented(
        &mut ui,
        s,
        st.speed.index(),
        &labels,
        frame::tr("comp.history"),
        if roomy { 228.0 } else { 180.0 },
    ) {
        st.set_speed(Speed::ALL[i]);
    }
    if controls::led(
        &mut ui,
        s,
        st.grid,
        Src::Key("comp.grid"),
        frame::tr("comp.grid_tip"),
        88.0,
    )
    .clicked()
    {
        st.grid = !st.grid;
    }
    if controls::led(
        &mut ui,
        s,
        st.frozen,
        Src::Key("comp.freeze"),
        frame::tr("comp.freeze_tip"),
        88.0,
    )
    .clicked()
    {
        st.frozen = !st.frozen;
    }
    if st.scroll > 0
        && controls::key(
            &mut ui,
            s,
            Src::Key("comp.live"),
            frame::tr("comp.live"),
            64.0,
            false,
        )
        .clicked()
    {
        st.scroll = 0;
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        gap_x(ui, s);
        let idx = RANGES
            .iter()
            .position(|r| (*r - st.range_db).abs() < 0.5)
            .unwrap_or(2);
        let id = ui.id().with("comp-scale");
        let at = controls::field(ui, s, id, RANGE_TEXT[idx], frame::tr("comp.scale"), 76.0);
        if roomy {
            controls::caption(ui, s, Src::Key("comp.scale"), 44.0);
        }
        if let Some(at) = at {
            let items: [Item<'_>; 5] =
                std::array::from_fn(|i| frame::radio(Src::Str(RANGE_TEXT[i]), i == idx));
            if let Some(i) = controls::list(ui, s, id, at, 96.0, &items) {
                st.range_db = RANGES[i];
            }
        }
    });
}

/// The history's gestures: drag the threshold line, click to freeze, double-click for
/// live, the wheel to scroll back.
#[allow(clippy::too_many_arguments)]
fn gestures(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    (ty, reach, near): (f32, f32, bool),
    dx: f32,
    d_of: &dyn Fn(f32) -> f32,
) {
    // The wheel scrolls back through what was kept (not in Whole, which shows it all).
    if resp.hovered() && st.speed != Speed::Whole {
        let wheel = ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
        if wheel != 0.0 {
            let max = st.history.len().saturating_sub(VISIBLE);
            let by = (wheel / dx.max(0.1)).round() as i64;
            st.scroll = (st.scroll as i64 + by).clamp(0, max as i64) as usize;
        }
    }
    if resp.clicked() && !near {
        st.frozen = !st.frozen;
    }
    if resp.double_clicked() {
        st.frozen = false;
        st.scroll = 0;
    }
    // What a drag grabs is decided where the press began: by the time egui calls it a
    // drag, the pointer has already moved.
    let origin = ui.input(|i| i.pointer.press_origin());
    if resp.drag_started() && origin.is_some_and(|o| (o.y - ty).abs() < reach) {
        st.drag = Some(Grab::Threshold);
    }
    if st.drag == Some(Grab::Threshold) {
        if let Some(pt) = resp.interact_pointer_pos() {
            if resp.dragged() {
                host.set("thr", d_of(pt.y).clamp(-80.0, 0.0), Some("thr"));
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if near || st.drag == Some(Grab::Threshold) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("comp.history"),
            frame::tr("comp.history_tip"),
            Caps::NONE,
        );
    }
}

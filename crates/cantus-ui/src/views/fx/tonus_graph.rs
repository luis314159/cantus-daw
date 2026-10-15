//! Tonus's display: the pitch graph (what was sung and what comes out, over note lanes)
//! and the detected note's panel (the note, its distance in cents, the target, the shift,
//! a meter centred at zero).

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::{State, HISTORY, KEY_NAMES};
use crate::kit::text::Role;
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_dsp::tonus::{scale, Allowed, Source, TonusSettings};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The pitch classes that are targets and on bypass now: the settings' with a scale, the
/// notes held with MIDI.
pub(super) fn classes(st: &State, ss: &TonusSettings) -> (u16, u16) {
    if ss.source != Source::Scale {
        return (st.header.held, 0);
    }
    let a = scale::resolve(ss.scale, ss.key, &ss.notes);
    let (mut on, mut by) = (0u16, 0u16);
    for (pc, x) in a.iter().enumerate() {
        match x {
            Allowed::Target => on |= 1 << pc,
            Allowed::Bypass => by |= 1 << pc,
            Allowed::Removed => {}
        }
    }
    (on, by)
}

fn is_black(pc: i32) -> bool {
    matches!(pc, 1 | 3 | 6 | 8 | 10)
}

/// Draw the display in `d`; returns the graph's rect and the panel's.
pub(super) fn show(
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    ss: &TonusSettings,
    d: Rect,
) -> (Rect, Rect) {
    let k = s.k;
    let ink = Ink::of(s);
    let panel_w = (172.0 * k).min(d.width() * 0.32);
    let graph = Rect::from_min_max(d.min, pos2(d.right() - panel_w, d.bottom()));
    let panel = Rect::from_min_max(pos2(graph.right(), d.top()), d.max);
    let (on, by) = classes(st, ss);

    // The centre follows the voice: the mean of the last voiced input, eased.
    let (mut sum, mut n) = (0.0f32, 0u32);
    for c in st.history.iter().rev().take(120) {
        if c.input > 0.0 {
            sum += c.input;
            n += 1;
        }
    }
    let want = if n > 0 {
        sum / n as f32
    } else if let Some(t) = st.header.target {
        t as f32
    } else if st.centre > 0.0 {
        st.centre
    } else {
        57.0
    };
    st.centre = if st.centre <= 0.0 {
        want
    } else {
        st.centre + (want - st.centre) * 0.15
    };
    let lanes = ((graph.height() / (16.0 * k)) as i32).clamp(7, 25) | 1;
    let lo = st.centre.round() as i32 - lanes / 2;
    let lane_h = graph.height() / lanes as f32;
    let gutter = 36.0 * k;
    let plot = Rect::from_min_max(pos2(graph.left() + gutter, graph.top()), graph.max);
    let y_of = |p: f32| -> f32 {
        (graph.bottom() - (p - lo as f32 + 0.5) * lane_h).clamp(graph.top(), graph.bottom())
    };
    let mut pts = std::mem::take(&mut st.pts);

    with_brush(&ui.painter_at(d), |b| {
        canvas::ground_b(b, d, &ink, s.hue_tok);
        // Lanes.
        for i in 0..lanes {
            let m = lo + i;
            let pc = m.rem_euclid(12);
            let top = graph.bottom() - (i + 1) as f32 * lane_h;
            let band = Rect::from_min_max(pos2(graph.left(), top), pos2(graph.right(), top + lane_h));
            let target = on & (1 << pc) != 0;
            let bypass = by & (1 << pc) != 0;
            if is_black(pc) {
                b.rect(b.snap_rect(band), Fill::Solid(Ink::a(ink.grid, 0.45)));
            }
            if st.header.target == Some(m) {
                b.rect(b.snap_rect(band), Fill::Solid(Ink::a(ink.hue, 0.24)));
            } else if target {
                b.rect(b.snap_rect(band), Fill::Solid(Ink::a(ink.hue, 0.08)));
            }
            if bypass {
                let y = band.center().y;
                canvas::dashed(b, pos2(plot.left(), y), pos2(plot.right(), y), b.px(), ink.dim, 4.0 * k, 4.0 * k);
            }
            b.hline(graph.left(), graph.right(), b.snap_mid(band.bottom()), b.px(), ink.grid);
            if lane_h >= 11.0 * k {
                let color = if target || bypass { ink.text } else { ink.dim };
                let name = frame::tr(KEY_NAMES[pc as usize]);
                let oct = m.div_euclid(12) - 1;
                canvas::putf(
                    b,
                    pos2(graph.left() + 4.0 * k, band.center().y),
                    Align2::LEFT_CENTER,
                    Role::Micro,
                    color,
                    s.tk(),
                    None,
                    format_args!("{name}{oct}"),
                );
            }
        }
        b.vline(b.snap_mid(plot.left()), graph.top(), graph.bottom(), b.px(), ink.grid_strong);
        // What was sung (dim) and what comes out (the hue).
        let len = st.history.len();
        let dx = plot.width() / HISTORY as f32;
        let x_of = |i: usize| plot.right() - (len - 1 - i) as f32 * dx;
        for (output, color, w) in [(false, ink.dim, 1.5 * k), (true, ink.hue, 2.0 * k)] {
            pts.clear();
            for (i, c) in st.history.iter().enumerate() {
                let v = if output { c.output } else { c.input };
                let x = x_of(i);
                if v > 0.0 && x >= plot.left() {
                    pts.push(pos2(x, y_of(v)));
                } else {
                    stroke(b, &mut pts, w, color);
                }
            }
            stroke(b, &mut pts, w, color);
        }
        // The panel.
        b.vline(b.snap_mid(panel.left()), panel.top(), panel.bottom(), b.px(), ink.edge);
        readout(b, s, &ink, st, panel);
        canvas::edge_b(b, d, &ink);
    });
    st.pts = pts;
    (graph, panel)
}

/// Draw a polyline and empty the buffer.
fn stroke(b: &mut Brush, pts: &mut Vec<Pos2>, w: f32, color: Color32) {
    if pts.len() >= 2 {
        b.polyline(pts, w, color);
    }
    pts.clear();
}

/// The detected note in large type, its cents, the target, the shift and its meter.
fn readout(b: &mut Brush, s: &Style, ink: &Ink, st: &State, r: Rect) {
    let k = s.k;
    let tk = s.tk();
    let h = st.header;
    let cx = r.center().x;
    let row = |f: f32| r.top() + r.height() * f;
    match h.pitch {
        Some(p) => {
            let n = p.round() as i32;
            let dev = (p - n as f32) * 100.0;
            let name = frame::tr(KEY_NAMES[n.rem_euclid(12) as usize]);
            let oct = n.div_euclid(12) - 1;
            canvas::putf(
                b,
                pos2(cx, row(0.2)),
                Align2::CENTER_CENTER,
                Role::Hero,
                ink.text,
                tk,
                None,
                format_args!("{name}{oct}"),
            );
            let warm = if dev.abs() > 15.0 { ink.danger } else { ink.text };
            canvas::tag(
                b,
                pos2(cx, row(0.38)),
                Align2::CENTER_CENTER,
                frame::tr("tonus.sung"),
                warm,
                tk,
                None,
                format_args!("{dev:+.0} ct"),
            );
            if h.on_bypass {
                canvas::put_key(b, pos2(cx, row(0.52)), Align2::CENTER_CENTER, "tonus.bypassed", Role::Label, ink.dim, tk);
            } else if let Some(t) = h.target {
                let tn = frame::tr(KEY_NAMES[t.rem_euclid(12) as usize]);
                let to = t.div_euclid(12) - 1;
                canvas::tag(
                    b,
                    pos2(cx, row(0.52)),
                    Align2::CENTER_CENTER,
                    frame::tr("tonus.target"),
                    ink.hue,
                    tk,
                    None,
                    format_args!("{tn}{to}"),
                );
            } else {
                canvas::put_key(b, pos2(cx, row(0.52)), Align2::CENTER_CENTER, "tonus.no_target", Role::Label, ink.dim, tk);
            }
        }
        None => {
            canvas::putf(b, pos2(cx, row(0.2)), Align2::CENTER_CENTER, Role::Hero, ink.dim, tk, None, format_args!("\u{2014}"));
            canvas::put_key(b, pos2(cx, row(0.38)), Align2::CENTER_CENTER, "tonus.no_voice", Role::Label, ink.dim, tk);
        }
    }
    // The shift and its meter: ±100 cents, centred.
    let shift = h.shift * 100.0;
    canvas::tag(
        b,
        pos2(cx, row(0.68)),
        Align2::CENTER_CENTER,
        frame::tr("tonus.shift"),
        ink.text,
        tk,
        None,
        format_args!("{shift:+.0} ct"),
    );
    let bar = Rect::from_center_size(pos2(cx, row(0.84)), vec2(r.width() - 24.0 * k, 8.0 * k));
    b.rrect(b.snap_rect(bar), 2.0 * k, Fill::Solid(ink.grid));
    let half = bar.width() * 0.5;
    let x = (shift / 100.0).clamp(-1.0, 1.0) * half;
    if x != 0.0 {
        let fill = Rect::from_min_max(
            pos2(cx.min(cx + x), bar.top()),
            pos2(cx.max(cx + x), bar.bottom()),
        );
        b.rect(b.snap_rect(fill), Fill::Solid(ink.hue));
    }
    for f in [-0.5f32, 0.0, 0.5] {
        let tx = b.snap_mid(cx + f * half);
        let t = if f == 0.0 { 4.0 } else { 2.0 } * k;
        b.vline(tx, bar.top() - t, bar.bottom() + t, b.px(), ink.text);
    }
}

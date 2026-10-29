//! The selected operator's card: its envelope, drawn and dragged (breakpoints in time and
//! level with a bend per segment, a sustain point and a loop; or the DX's four rates and
//! levels), its keyboard scaling curve, and its oscillator — pages picked in the title bar.

use super::ops::pill_num;
use super::parts::{self, c};
use super::{batch, drag_values, Detail};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::knob::KnobSize;
use crate::kit::menu::Item;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Brush, Fill, Style};
use crate::views::inst::write_readout;
use cantus_core::inst::fm::{OpKeys, ENV_POINTS, KS_CURVES, NAMES, OP_KEYS, WAVES};
use cantus_core::{pitch_name, NativeSpec, TrackId};
use cantus_dsp::modulus::env::{bend, dx_env_steps, dx_qrate, dx_speed};
use cantus_dsp::modulus::{key_scaling_db, tel, OpSettings};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// A segment of `secs` in the drawing: square-root time, so a 2 ms attack and a 4 s
/// release are both visible and draggable (each segment also gets a fixed 8 pt).
fn seg_s(secs: f32) -> f32 {
    (secs.max(0.0) / 0.25).sqrt()
}

/// The points of a time-mode envelope as drawn: `(x, level)` for the start and each point,
/// and where the sustain plateau sits.
fn time_points(spec: &NativeSpec, k: &OpKeys, rect: Rect) -> (Vec<Pos2>, f32) {
    let n = spec.get(k.env_n).round() as usize;
    let sus = spec.get(k.env_sus).round() as usize;
    let total: f32 = (0..n)
        .map(|i| seg_s(spec.get(k.env_t[i]) / 1000.0))
        .sum::<f32>();
    let plateau = if sus > 0 { rect.width() * 0.12 } else { 0.0 };
    let fixed = (rect.width() * 0.02).max(1.0);
    let unit = (rect.width() - plateau - fixed * 2.0 - fixed * n as f32).max(0.0) / total.max(1e-3);
    let y = |l: f32| rect.bottom() - l.clamp(0.0, 1.0) * rect.height();
    let mut x = rect.left();
    let mut pts = vec![Pos2::new(x, y(spec.get(k.env_l0)))];
    for i in 0..n {
        x += seg_s(spec.get(k.env_t[i]) / 1000.0) * unit + fixed;
        pts.push(Pos2::new(x, y(spec.get(k.env_l[i]))));
        if i + 1 == sus {
            x += plateau;
        }
    }
    (pts, plateau)
}

/// The four DX points (start at L4, then L1…L4) with widths from the measured speeds.
fn dx_points(s: &OpSettings, rect: Rect) -> Vec<Pos2> {
    let l = s.dxenv.levels;
    let r = s.dxenv.rates;
    let steps = |x: u8| dx_env_steps(x).max(16.0);
    let secs = |from: u8, to: u8, rate: u8| {
        let d = (steps(to) - steps(from)).abs();
        let v = dx_speed(dx_qrate(rate, 0)) * if steps(to) > steps(from) { 6.0 } else { 1.0 };
        d / v
    };
    let seq = [
        (l[3], l[0], r[0]),
        (l[0], l[1], r[1]),
        (l[1], l[2], r[2]),
        (l[2], l[3], r[3]),
    ];
    let plateau = rect.width() * 0.12;
    let fixed = (rect.width() * 0.02).max(1.0);
    let total: f32 = seq.iter().map(|(a, b, c)| seg_s(secs(*a, *b, *c))).sum();
    let unit = (rect.width() - plateau - fixed * 6.0).max(0.0) / total.max(1e-3);
    let y = |x: u8| rect.bottom() - x as f32 / 99.0 * rect.height();
    let mut x = rect.left();
    let mut pts = vec![Pos2::new(x, y(l[3]))];
    for (i, (a, b, c)) in seq.iter().enumerate() {
        x += seg_s(secs(*a, *b, *c)) * unit + fixed;
        pts.push(Pos2::new(x, y(*b)));
        if i == 2 {
            x += plateau;
        }
    }
    pts
}

/// The envelope's outline (for the table's thumbnails).
pub(super) fn shape(spec: &NativeSpec, o: usize, rect: Rect) -> Vec<Pos2> {
    let k = &OP_KEYS[o];
    if spec.get(k.env_mode) >= 0.5 {
        let p = cantus_engine::inst::fm::patch_of(spec);
        let mut pts = dx_points(&p.ops[o], rect);
        pts.insert(4, Pos2::new(pts[3].x + rect.width() * 0.12, pts[3].y));
        pts
    } else {
        let (mut pts, plateau) = time_points(spec, k, rect);
        let sus = spec.get(k.env_sus).round() as usize;
        if sus > 0 && sus < pts.len() {
            let p = pts[sus];
            pts.insert(sus + 1, Pos2::new(p.x + plateau, p.y));
        }
        pts
    }
}

/// A display ground inside a card's body (the card's own display bleeds full width; this
/// one sits beside the controls): ink, darker to the bottom, a hairline edge.
fn ground(b: &mut Brush, rect: Rect, s: &Style) {
    let r = b.snap_rect(rect);
    if s.skin.display != crate::kit::skin::Display::Plain {
        // A unit's face: the display's own look (an LCD on Modulus), its edge.
        crate::kit::skin::display_ground(b, r, s);
        b.rrect_stroke(r, s.r(3.0), b.px(), s.t.edge);
        return;
    }
    b.rrect(
        r,
        s.r(3.0),
        Fill::V(
            s.t.ink,
            shade(s.t.ink, if s.t.dark { -0.02 } else { -0.03 }),
        ),
    );
    b.rrect_stroke(r, s.r(3.0), b.px(), with_alpha(s.t.edge, 0.55));
}

/// A breakpoint handle: filled when it is the sustain point, bigger while hot.
fn handle(ui: &egui::Ui, id: egui::Id, at: Pos2, s: &Style) -> egui::Response {
    ui.interact(
        Rect::from_center_size(at, Vec2::splat(s.s(12.0))),
        id,
        Sense::click_and_drag(),
    )
}

fn paint_handle(b: &mut Brush, at: Pos2, s: &Style, marked: bool, hot: bool) {
    let r = if hot { s.s(5.5) } else { s.s(4.0) };
    b.circle(at, r, Fill::Solid(if marked { s.hue } else { s.t.deck }));
    b.circle_stroke(at, r, s.s(1.5), s.hue);
}

// ---- the card ------------------------------------------------------------------------------

/// The selected operator's card: Envelope · Key scaling · Oscillator in the title bar.
pub(super) fn detail_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let o = app.inst.fm.selected.min(7);
    let k = &OP_KEYS[o];
    let title = tf("fm.operator", &[("op", &NAMES[o])]);
    let cr = parts::card(ui, rect, s, Src::Str(&title), None, None, None);
    let pages = [
        Src::Key("fm.page.env"),
        Src::Key("fm.page.scaling"),
        Src::Key("fm.page.osc"),
    ];
    let now = match app.inst.fm.detail {
        Detail::Envelope => 0,
        Detail::Scaling => 1,
        Detail::Oscillator => 2,
    };
    let w = s.s(240.0).min(cr.title.width() * 0.6);
    let h = s.s(18.0);
    let pr = Rect::from_min_size(
        Pos2::new(
            cr.title.right() - s.s(6.0) - w,
            cr.title.center().y - h * 0.5,
        ),
        Vec2::new(w, h),
    );
    if let Some(i) = parts::seg(
        ui,
        ui.id().with("fm-page"),
        pr,
        s,
        &pages,
        now,
        &t("fm.page.tip"),
    ) {
        app.inst.fm.detail = [Detail::Envelope, Detail::Scaling, Detail::Oscillator][i];
    }
    let body = cr.body;
    match app.inst.fm.detail {
        Detail::Oscillator => osc_page(app, ui, track, spec, s, body, o),
        page => {
            let side = s.s(170.0).min(body.width() * 0.34);
            let d = Rect::from_min_max(
                body.min,
                Pos2::new(body.right() - side - s.s(10.0), body.bottom()),
            );
            let col = Rect::from_min_max(Pos2::new(d.right() + s.s(10.0), body.top()), body.max);
            if page == Detail::Envelope {
                let dx = spec.get(k.env_mode) >= 0.5;
                if dx {
                    dx_editor(app, ui, track, spec, s, d, o);
                } else {
                    time_editor(app, ui, track, spec, s, d, o);
                }
                let fh = s.s(20.0);
                let modes = [Src::Key("fm.env.time_mode"), Src::Key("fm.env.dx_mode")];
                let mr = Rect::from_min_size(col.min, Vec2::new(col.width(), fh));
                if let Some(i) = parts::seg_param(
                    app,
                    ui,
                    ui.id().with(("fm-env-mode", o)),
                    mr,
                    s,
                    &modes,
                    dx as usize,
                    &t("fm.env.mode_tip"),
                    track,
                    spec,
                    k.env_mode,
                ) {
                    app.set_inst_value(track, k.env_mode, i as f32, false);
                }
                let rest =
                    Rect::from_min_max(Pos2::new(col.left(), mr.bottom() + s.s(6.0)), col.max);
                if dx {
                    dx_numbers(app, ui, track, spec, s, rest, o);
                } else {
                    parts::cells_spread(
                        app,
                        ui,
                        track,
                        spec,
                        s,
                        KnobSize::M,
                        rest,
                        &[c(k.key_time), c(k.vel_time)],
                    );
                }
            } else {
                scaling_editor(app, ui, track, spec, s, d, o);
                let fh = s.s(20.0);
                let lw = s.s(52.0);
                let curves: Vec<String> = KS_CURVES.iter().map(|x| t(x)).collect();
                for (i, (key, label)) in
                    [(k.ks_lcurve, "fm.ks.lcurve"), (k.ks_rcurve, "fm.ks.rcurve")]
                        .iter()
                        .enumerate()
                {
                    let r = Rect::from_min_size(
                        Pos2::new(col.left(), col.top() + i as f32 * (fh + s.s(4.0))),
                        Vec2::new(col.width(), fh),
                    );
                    with_brush(ui.painter(), |b| {
                        parts::row_label(
                            b,
                            Rect::from_min_size(r.min, Vec2::new(lw - s.s(6.0), fh)),
                            s,
                            label,
                        )
                    });
                    parts::choice_key(
                        app,
                        ui,
                        track,
                        spec,
                        key,
                        Rect::from_min_max(Pos2::new(r.left() + lw, r.top()), r.max),
                        s,
                        &curves,
                    );
                }
                let rest = Rect::from_min_max(
                    Pos2::new(col.left(), col.top() + fh * 2.0 + s.s(10.0)),
                    col.max,
                );
                parts::cells_spread(
                    app,
                    ui,
                    track,
                    spec,
                    s,
                    KnobSize::M,
                    rest,
                    &[c(k.vel), c(k.ks_rate)],
                );
            }
        }
    }
}

/// The oscillator page: wave and frequency mode, three switches, and every number.
#[allow(clippy::too_many_arguments)]
fn osc_page(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    body: Rect,
    o: usize,
) {
    let k = &OP_KEYS[o];
    let fh = s.s(20.0);
    let top = Rect::from_min_size(body.min, Vec2::new(body.width(), fh));
    let cols = parts::columns(top, s, &[0.28, 0.24, 0.16, 0.16, 0.16]);
    let lw = s.s(46.0);
    let waves: Vec<String> = WAVES.iter().map(|x| t(x)).collect();
    with_brush(ui.painter(), |b| {
        parts::row_label(
            b,
            Rect::from_min_size(cols[0].min, Vec2::new(lw - s.s(6.0), fh)),
            s,
            "fm.op.wave",
        );
    });
    parts::choice_key(
        app,
        ui,
        track,
        spec,
        k.wave,
        Rect::from_min_max(Pos2::new(cols[0].left() + lw, cols[0].top()), cols[0].max),
        s,
        &waves,
    );
    let modes = [Src::Key("fm.op.ratio"), Src::Key("fm.op.fixed")];
    parts::seg_key(app, ui, track, spec, k.freq_mode, cols[1], s, &modes);
    parts::led_key(app, ui, track, spec, k.sync, cols[2], s);
    parts::led_key(app, ui, track, spec, k.penv, cols[3], s);
    parts::led_key(app, ui, track, spec, k.on, cols[4], s);
    // Two rows of M cells: the frequency and level, then phase, place and feedback.
    let rest = Rect::from_min_max(Pos2::new(body.left(), top.bottom() + s.s(4.0)), body.max);
    let half = rest.height() * 0.5;
    let r1 = Rect::from_min_size(rest.min, Vec2::new(rest.width(), half));
    let r2 = Rect::from_min_size(
        Pos2::new(rest.left(), rest.top() + half),
        Vec2::new(rest.width(), half),
    );
    let ratio = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "\u{00d7}{v:.2}");
    };
    let fb = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{v:.2}");
    };
    let out = |v: f32, b: &mut String| {
        if v <= cantus_core::inst::fm::OUT_OFF {
            b.push_str(&t("fm.off"));
        } else {
            crate::views::inst::wavetable::db_text(v, b);
        }
    };
    let db = |v: f32, b: &mut String| crate::views::inst::wavetable::db_text(v, b);
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::M,
        r1,
        &[
            c(k.ratio).fmt(&ratio),
            c(k.fixed),
            c(k.detune),
            c(k.offset),
            c(k.level).fmt(&db),
            c(k.vel),
        ],
    );
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::M,
        r2,
        &[
            c(k.phase),
            c(k.pan),
            c(k.out).fmt(&out),
            c(k.fb).fmt(&fb),
            c(k.ams),
        ],
    );
}

/// The DX envelope's rates and levels as eight value pills, R and L per stage.
#[allow(clippy::too_many_arguments)]
fn dx_numbers(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    o: usize,
) {
    let k = &OP_KEYS[o];
    let ph = s.s(18.0);
    let gap = s.s(4.0);
    let w = (rect.width() - gap) * 0.5;
    for i in 0..4 {
        let y = rect.top() + i as f32 * (ph + gap);
        let rr = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(w, ph));
        let lr = Rect::from_min_size(Pos2::new(rect.left() + w + gap, y), Vec2::new(w, ph));
        let rt = format!("R{} {:.0}", i + 1, spec.get(k.dx_r[i]));
        let lt = format!("L{} {:.0}", i + 1, spec.get(k.dx_l[i]));
        pill_num(
            app, ui, track, spec, k.dx_r[i], rr, s, &rt, None, None, None,
        );
        pill_num(
            app, ui, track, spec, k.dx_l[i], lr, s, &lt, None, None, None,
        );
    }
    let rest = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + 4.0 * (ph + gap)),
        rect.max,
    );
    if rest.height() > parts::cell_size(KnobSize::S, s).y {
        parts::cells_spread(app, ui, track, spec, s, KnobSize::S, rest, &[c(k.ks_rate)]);
    }
}

// ---- the editors ---------------------------------------------------------------------------

/// The breakpoint envelope: drag a point in time and level, a segment's middle to bend it;
/// right-click a point for sustain, loop and removal; + adds a point.
#[allow(clippy::too_many_arguments)]
fn time_editor(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    o: usize,
) {
    // The envelope in the display's look (round 32): its ink, its ground.
    let s = &crate::views::inst::face::on_display(s);
    let k = &OP_KEYS[o];
    let plot = rect.shrink2(Vec2::new(s.s(12.0), s.s(16.0)));
    let (pts, plateau) = time_points(spec, k, plot);
    let n = spec.get(k.env_n).round() as usize;
    let sus = spec.get(k.env_sus).round() as usize;
    let lp = spec.get(k.env_loop).round() as usize;
    // The curve itself, bends included.
    let mut line = vec![pts[0]];
    for i in 0..n {
        let (a, z) = (pts[i], pts[i + 1]);
        let a = if sus > 0 && i == sus {
            let flat = Pos2::new(a.x + plateau, a.y);
            line.push(flat);
            flat
        } else {
            a
        };
        let cv = spec.get(k.env_c[i]);
        for j in 1..=16 {
            let x = j as f32 / 16.0;
            line.push(Pos2::new(
                a.x + (z.x - a.x) * x,
                a.y + (z.y - a.y) * bend(x, cv),
            ));
        }
    }
    // Interactions first, so the handles know whether they are hot.
    let r0 = handle(ui, ui.id().with(("fm-env0", o)), pts[0], s);
    if r0.dragged() {
        let v = spec.get(k.env_l0) - r0.drag_delta().y / plot.height();
        app.set_inst_value(track, k.env_l0, v.clamp(0.0, 1.0), true);
    }
    if r0.drag_stopped() {
        app.end_gesture();
    }
    let mut hot = vec![r0.hovered() || r0.dragged()];
    let mut bends = Vec::with_capacity(n);
    for i in 0..n {
        let p = pts[i + 1];
        let resp = handle(ui, ui.id().with(("fm-envp", o, i)), p, s);
        if resp.dragged() {
            let d = resp.drag_delta();
            let lv = (spec.get(k.env_l[i]) - d.y / plot.height()).clamp(0.0, 1.0);
            let tm = spec.get(k.env_t[i]);
            let tm = if tm <= 0.0 && d.x > 0.0 {
                1.0
            } else {
                tm * (d.x / s.k / 40.0).exp2()
            };
            drag_values(
                app,
                track,
                &[(k.env_l[i], lv), (k.env_t[i], tm.clamp(0.0, 30_000.0))],
            );
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        if resp.hovered() || resp.dragged() {
            let mut v = String::new();
            write_readout(spec.get(k.env_t[i]), "ms", &mut v);
            let v = format!("{v} \u{b7} {:.0} %", spec.get(k.env_l[i]) * 100.0);
            crate::kit::hint::set_text(
                ui.ctx(),
                &tf("fm.env.point", &[("n", &(i + 1).to_string())]),
                &v,
                crate::kit::hint::Caps {
                    menu: true,
                    ..crate::kit::hint::Caps::PARAM
                },
            );
        }
        hot.push(resp.hovered() || resp.dragged());
        let menu_id = ui.id().with(("fm-envp-menu", o, i));
        if resp.secondary_clicked() {
            parts::toggle_popup(ui, menu_id);
        }
        let mut items = vec![
            Item::action(Src::Key("fm.env.sus_here")),
            Item::action(Src::Key("fm.env.loop_here")),
            Item::action(Src::Key("fm.env.no_sus")),
        ];
        if n > 1 {
            items.push(Item::action(Src::Key("fm.env.remove")));
        }
        match parts::popup(
            ui,
            menu_id,
            Rect::from_center_size(p, Vec2::splat(s.s(12.0))),
            s,
            &items,
        ) {
            Some(0) => app.set_inst_value(track, k.env_sus, (i + 1) as f32, false),
            Some(1) => app.set_inst_value(track, k.env_loop, (i + 1) as f32, false),
            Some(2) => batch(
                app,
                track,
                spec,
                "cmd.set_instrument_value",
                &[(k.env_sus, 0.0), (k.env_loop, 0.0)],
            ),
            Some(3) => remove_point(app, track, spec, k, i, n),
            _ => {}
        }
        // The bend handle, in the segment's middle.
        let a = pts[i];
        let mid = Pos2::new((a.x + p.x) * 0.5, (a.y + p.y) * 0.5);
        let bresp = ui.interact(
            Rect::from_center_size(mid, Vec2::splat(s.s(10.0))),
            ui.id().with(("fm-envc", o, i)),
            Sense::drag(),
        );
        if bresp.dragged() {
            let cv = (spec.get(k.env_c[i])
                - bresp.drag_delta().y / s.k / 80.0 * if p.y > a.y { -1.0 } else { 1.0 })
            .clamp(-1.0, 1.0);
            app.set_inst_value(track, k.env_c[i], cv, true);
        }
        if bresp.drag_stopped() {
            app.end_gesture();
        }
        bends.push((mid, bresp.hovered() || bresp.dragged()));
    }
    let add = (n < ENV_POINTS).then(|| {
        let at = Rect::from_center_size(
            Pos2::new(rect.right() - s.s(14.0), rect.top() + s.s(12.0)),
            Vec2::splat(s.s(16.0)),
        );
        let resp = ui.interact(at, ui.id().with(("fm-env+", o)), Sense::click());
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &t("fm.env.add"),
                "",
                crate::kit::hint::Caps::NONE,
            );
        }
        if resp.clicked() {
            app.set_inst_value(track, k.env_n, (n + 1) as f32, false);
        }
        (at, resp.hovered())
    });
    let tel_ = app.inst_telemetry();
    with_brush(&ui.painter_at(rect), |b| {
        ground(b, rect, s);
        b.hline(
            plot.left(),
            plot.right(),
            b.snap_mid(plot.bottom()),
            b.px(),
            s.t.grid,
        );
        b.polyline(&line, s.s(2.0), s.hue);
        for (i, p) in pts.iter().enumerate().skip(1) {
            let mut l = String::new();
            write_readout(spec.get(k.env_t[i - 1]), "ms", &mut l);
            parts::say(
                b,
                Rect::from_center_size(
                    Pos2::new(p.x, rect.bottom() - s.s(8.0)),
                    Vec2::new(s.s(52.0), s.s(12.0)),
                ),
                s,
                &l,
                Role::Micro,
                s.t.text_dim,
                Align2::CENTER_CENTER,
            );
        }
        for (mark, text) in [(sus, "S"), (lp, "L")] {
            if mark > 0 && mark < pts.len() {
                let x = b.snap_mid(pts[mark].x);
                let mut y = plot.top();
                while y < plot.bottom() {
                    b.vline(
                        x,
                        y,
                        (y + s.s(3.0)).min(plot.bottom()),
                        b.px(),
                        s.t.text_dim,
                    );
                    y += s.s(6.0);
                }
                parts::say(
                    b,
                    Rect::from_min_size(
                        Pos2::new(x + s.s(3.0), rect.top() + s.s(2.0)),
                        Vec2::new(s.s(14.0), s.s(12.0)),
                    ),
                    s,
                    text,
                    Role::Micro,
                    s.t.text,
                    Align2::LEFT_CENTER,
                );
            }
        }
        // Where the newest voice is.
        if tel_[tel::KEY] > 0.0 && tel_[tel::AMP + o] > 0.0 {
            let stage = (tel_[tel::STAGE + o] as usize).min(n);
            let x = pts[stage.min(pts.len() - 1)].x;
            b.circle(
                Pos2::new(
                    x,
                    plot.bottom() - tel_[tel::AMP + o].min(1.0) * plot.height(),
                ),
                s.s(3.0),
                Fill::Solid(s.t.highlight),
            );
        }
        for (i, p) in pts.iter().enumerate() {
            paint_handle(b, *p, s, i > 0 && i == sus, hot[i]);
        }
        for (mid, h) in &bends {
            let d = s.s(4.0);
            b.polygon(
                &[
                    *mid + Vec2::new(0.0, -d),
                    *mid + Vec2::new(d, 0.0),
                    *mid + Vec2::new(0.0, d),
                    *mid + Vec2::new(-d, 0.0),
                ],
                if *h { s.hue } else { s.t.text_dim },
            );
        }
        if let Some((at, h)) = add {
            parts::say(
                b,
                at,
                s,
                "+",
                Role::Title,
                if h { s.hue } else { s.t.text },
                Align2::CENTER_CENTER,
            );
        }
    });
}

/// Remove point `i` (0-based): the ones after it move down, one undo step.
fn remove_point(
    app: &mut CantusApp,
    track: TrackId,
    spec: &NativeSpec,
    k: &OpKeys,
    i: usize,
    n: usize,
) {
    let mut changes: Vec<(&str, f32)> = Vec::new();
    for j in i..n - 1 {
        changes.push((k.env_t[j], spec.get(k.env_t[j + 1])));
        changes.push((k.env_l[j], spec.get(k.env_l[j + 1])));
        changes.push((k.env_c[j], spec.get(k.env_c[j + 1])));
    }
    changes.push((k.env_n, (n - 1) as f32));
    let sus = spec.get(k.env_sus).round() as usize;
    if sus > i {
        changes.push((k.env_sus, (sus - 1) as f32));
    }
    batch(app, track, spec, "cmd.fm_env_point", &changes);
}

/// The DX envelope: drag a point up and down for its level, sideways for its rate.
#[allow(clippy::too_many_arguments)]
fn dx_editor(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    o: usize,
) {
    // The envelope in the display's look (round 32): its ink, its ground.
    let s = &crate::views::inst::face::on_display(s);
    let k = &OP_KEYS[o];
    let plot = rect.shrink2(Vec2::new(s.s(12.0), s.s(16.0)));
    let p = cantus_engine::inst::fm::patch_of(spec);
    let pts = dx_points(&p.ops[o], plot);
    let mut drawn = pts[..4].to_vec();
    drawn.push(Pos2::new(pts[3].x + plot.width() * 0.12, pts[3].y));
    drawn.push(pts[4]);
    let mut hot = [false; 4];
    for i in 0..4 {
        let resp = handle(ui, ui.id().with(("fm-dxp", o, i)), pts[i + 1], s);
        if resp.dragged() {
            // Whole steps: the fraction a frame's movement leaves is kept for the next one.
            let d = resp.drag_delta() / s.k;
            let acc = ui.data_mut(|m| {
                let a = m.get_temp_mut_or_default::<Vec2>(resp.id);
                *a += Vec2::new(-d.x / 3.0, -d.y / (plot.height() / s.k) * 99.0);
                let whole = Vec2::new(a.x.trunc(), a.y.trunc());
                *a -= whole;
                whole
            });
            let l = (spec.get(k.dx_l[i]) + acc.y).clamp(0.0, 99.0);
            let r = (spec.get(k.dx_r[i]) + acc.x).clamp(0.0, 99.0);
            drag_values(app, track, &[(k.dx_l[i], l), (k.dx_r[i], r)]);
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        hot[i] = resp.hovered() || resp.dragged();
        if hot[i] {
            let v = tf(
                "fm.env.dx_tip",
                &[
                    ("n", &(i + 1).to_string()),
                    ("r", &format!("{}", spec.get(k.dx_r[i]))),
                    ("l", &format!("{}", spec.get(k.dx_l[i]))),
                ],
            );
            crate::kit::hint::set_text(ui.ctx(), &v, "", crate::kit::hint::Caps::PARAM);
        }
    }
    with_brush(&ui.painter_at(rect), |b| {
        ground(b, rect, s);
        b.hline(
            plot.left(),
            plot.right(),
            b.snap_mid(plot.bottom()),
            b.px(),
            s.t.grid,
        );
        b.polyline(&drawn, s.s(2.0), s.hue);
        for i in 0..4 {
            paint_handle(b, pts[i + 1], s, i == 2, hot[i]);
        }
    });
}

/// Keyboard level scaling: the curve over the keyboard, the break point to drag sideways,
/// each side's depth to drag up and down (its value at two octaves).
#[allow(clippy::too_many_arguments)]
fn scaling_editor(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    o: usize,
) {
    // The envelope in the display's look (round 32): its ink, its ground.
    let s = &crate::views::inst::face::on_display(s);
    let k = &OP_KEYS[o];
    let plot = rect.shrink2(Vec2::new(s.s(12.0), s.s(18.0)));
    let p = cantus_engine::inst::fm::patch_of(spec);
    let op = &p.ops[o];
    let (lo, hi) = (21.0f32, 108.0f32);
    let x = |note: f32| plot.left() + (note - lo) / (hi - lo) * plot.width();
    let y = |db: f32| plot.top() + (24.0 - db.clamp(-48.0, 24.0)) / 72.0 * plot.height();
    let line: Vec<Pos2> = (21..=108)
        .map(|n| Pos2::new(x(n as f32), y(key_scaling_db(op, n))))
        .collect();
    let bx = x(op.ks_break as f32);
    let br = ui.interact(
        Rect::from_center_size(
            Pos2::new(bx, plot.center().y),
            Vec2::new(s.s(10.0), plot.height()),
        ),
        ui.id().with(("fm-ksb", o)),
        Sense::drag(),
    );
    if br.dragged() {
        let v = spec.get(k.ks_break) + br.drag_delta().x / plot.width() * (hi - lo);
        app.set_inst_value(track, k.ks_break, v.round().clamp(0.0, 127.0), true);
    }
    if br.drag_stopped() {
        app.end_gesture();
    }
    if br.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("fm.ks.break"),
            &pitch_name(op.ks_break),
            crate::kit::hint::Caps::PARAM,
        );
    }
    let mut sides = Vec::new();
    for (side, key, note) in [
        (-1.0f32, k.ks_ldepth, op.ks_break as f32 - 24.0),
        (1.0, k.ks_rdepth, op.ks_break as f32 + 24.0),
    ] {
        let n = note.clamp(lo, hi);
        let at = Pos2::new(x(n), y(key_scaling_db(op, n as u8)));
        let resp = handle(ui, ui.id().with(("fm-ksd", o, side as i32)), at, s);
        if resp.dragged() {
            let curve = if side < 0.0 {
                op.ks_lcurve
            } else {
                op.ks_rcurve
            };
            let dir = if curve <= 1 { 1.0 } else { -1.0 };
            let v = spec.get(key) + resp.drag_delta().y / s.k * dir * 0.25;
            app.set_inst_value(track, key, v.clamp(0.0, 48.0), true);
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        let mut l = String::new();
        write_readout(key_scaling_db(op, n as u8), "dB", &mut l);
        sides.push((at, resp.hovered() || resp.dragged(), l));
    }
    with_brush(&ui.painter_at(rect), |b| {
        ground(b, rect, s);
        b.hline(
            plot.left(),
            plot.right(),
            b.snap_mid(y(0.0)),
            b.px(),
            s.t.text_dim,
        );
        b.polyline(&line, s.s(2.0), s.hue);
        b.vline(b.snap_mid(bx), plot.top(), plot.bottom(), s.s(1.0), s.hue);
        parts::say(
            b,
            Rect::from_min_size(
                Pos2::new(bx + s.s(3.0), rect.top() + s.s(2.0)),
                Vec2::new(s.s(40.0), s.s(12.0)),
            ),
            s,
            &pitch_name(op.ks_break),
            Role::Micro,
            s.t.text,
            Align2::LEFT_CENTER,
        );
        for (at, h, l) in &sides {
            paint_handle(b, *at, s, false, *h);
            let below = at.y < plot.center().y;
            let lr = Rect::from_center_size(
                *at + Vec2::new(0.0, if below { s.s(12.0) } else { -s.s(12.0) }),
                Vec2::new(s.s(60.0), s.s(12.0)),
            );
            parts::say(b, lr, s, l, Role::Micro, s.t.text, Align2::CENTER_CENTER);
        }
    });
}

//! The transfer curve — input level across, output level up, on the history's scale — and
//! the meters beside it with the measured numbers.

use super::*;
use crate::kit::Fill;
use crate::views::fx::canvas::{self, Ink};
use cantus_dsp::comprimo::{effective_gain_reduction_db, gain_reduction_db};

/// `4.0:1`, `∞:1`.
struct Ratio(f32);

impl std::fmt::Display for Ratio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= RATIO_INF - 0.05 {
            f.write_str("\u{221e}:1")
        } else {
            write!(f, "{:.1}:1", self.0)
        }
    }
}

/// Points along the curves.
const CURVE: usize = 121;

/// The transfer display `outer`: the curve's numbers in a strip at its top, the plot
/// under it. Drag the threshold point, the slope above it (the ratio, with click stops;
/// Shift for free), the knee marks, or the dashed range floor; the wheel widens the knee.
/// A feedback detector's curve is drawn as it really settles, with the set curve dashed
/// behind it.
pub(super) fn show(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    cs: &CompSettings,
    tel: &Tele,
    outer: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let rect = Rect::from_min_max(
        pos2(outer.left(), outer.top() + TRANSFER_HEAD * k),
        outer.max,
    );
    let resp = ui.interact(rect, ui.id().with("comp-transfer"), Sense::click_and_drag());
    st.geometry.transfer = Some(rect);
    let lo = -st.range_db;
    let x = |d: f32| rect.left() + ((d - lo) / -lo).clamp(0.0, 1.0) * rect.width();
    let y = |d: f32| rect.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * rect.height();
    let dx_of = |px: f32| lo + (px - rect.left()) / rect.width() * -lo;
    let dy_of = |py: f32| lo + (rect.bottom() - py) / rect.height() * -lo;
    let mix = cs.mix;
    let out_of = |d: f32| d - mix * effective_gain_reduction_db(d, cs);
    let fb = cs.detector().feedback;
    let at = |i: usize| lo + -lo * i as f32 / (CURVE - 1) as f32;
    let floor_shown = cs.range_db < -lo;
    let t_pt = pos2(x(cs.threshold_db), y(out_of(cs.threshold_db)));
    let k_lo = cs.threshold_db - cs.knee_db * 0.5;
    let k_hi = cs.threshold_db + cs.knee_db * 0.5;
    let k_pts = [
        pos2(x(k_lo), y(out_of(k_lo))),
        pos2(x(k_hi), y(out_of(k_hi))),
    ];
    // Feedback: the ratio it really gives, 12 dB over.
    let lang = crate::i18n::locale();
    let effective = (fb > 0.0 && cs.ratio > 1.0).then(|| {
        cached(&mut st.words.effective, (host.revision(), lang), || {
            let over = 12.0;
            let g = effective_gain_reduction_db(cs.threshold_db + over, cs);
            let eff = if over - g > 0.01 {
                over / (over - g)
            } else {
                RATIO_INF
            };
            tf("comp.effective", &[("ratio", &ratio_text(eff))])
        })
    });
    let held_range = st.drag == Some(Grab::Range);
    let (ni, no) = st.now;
    with_brush(&ui.painter_at(outer), |b| {
        canvas::ground_b(b, outer, &ink, s.hue_tok);
        let px = b.px();
        b.hline(
            outer.left(),
            outer.right(),
            b.snap_mid(rect.top()),
            px,
            ink.grid,
        );
        let step = if -lo <= 36.0 { 6.0 } else { 12.0 };
        let mut g = -step;
        while g > lo {
            b.hline(rect.left(), rect.right(), b.snap_mid(y(g)), px, ink.grid);
            b.vline(b.snap_mid(x(g)), rect.top(), rect.bottom(), px, ink.grid);
            g -= step;
        }
        // 1:1, for reference.
        b.segment(
            pos2(x(lo), y(lo)),
            pos2(x(0.0), y(0.0)),
            px,
            ink.grid_strong,
            false,
        );
        let mut pts = [Pos2::ZERO; CURVE];
        if fb > 0.0 || mix != 1.0 {
            // The curve as set, dashed, behind the one that really happens.
            for (i, p) in pts.iter_mut().enumerate() {
                let d = at(i);
                *p = pos2(x(d), y(d - gain_reduction_db(d, cs)));
            }
            for w in pts.windows(2).step_by(2) {
                b.segment(w[0], w[1], 1.25 * k, Ink::a(ink.hue, 0.6), false);
            }
        }
        // The range floor: the curve never falls further below 1:1 than this.
        if floor_shown {
            let r = cs.range_db * mix.max(0.01);
            canvas::dashed(
                b,
                pos2(x(lo + r), y(lo)),
                pos2(x(0.0), y(-r)),
                if held_range { 2.0 } else { 1.25 } * k,
                ink.danger,
                3.0 * k,
                3.0 * k,
            );
        }
        for (i, p) in pts.iter_mut().enumerate() {
            let d = at(i);
            *p = pos2(x(d), y(out_of(d)));
        }
        b.polyline(&pts, 2.5 * k, ink.hue);
        // The knee marks and the threshold point.
        for kp in k_pts {
            b.circle_stroke(kp, 4.0 * k, 1.5 * k, ink.hue);
        }
        b.circle(t_pt, 6.0 * k, Fill::Solid(ink.hue));
        // Where the key is now, on the input axis, and the operating point.
        if tel.live() {
            let key = tel.get(tele::KEY_LEVEL);
            if key > lo {
                let kx = b.snap_mid(x(key));
                b.vline(
                    kx,
                    rect.bottom() - 10.0 * k,
                    rect.bottom(),
                    3.0 * k,
                    ink.text,
                );
            }
        }
        if ni > lo {
            b.circle(pos2(x(ni), y(no)), 4.0 * k, Fill::Solid(ink.danger));
        }
        // The words last.
        let mut ruler = canvas::Ruler::new();
        let mut g = lo + step;
        while g < 0.0 {
            let at = pos2(x(g) + 2.0 * k, rect.bottom() - 2.0 * k);
            if ruler.fits(at.x, 4.0 * k) {
                let r = canvas::putf(
                    b,
                    at,
                    Align2::LEFT_BOTTOM,
                    Role::Micro,
                    ink.dim,
                    tk,
                    None,
                    format_args!("{g:.0}"),
                );
                ruler.placed(r);
            }
            canvas::putf(
                b,
                pos2(rect.left() + 4.0 * k, y(g) - 2.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                Some(ink.plate),
                format_args!("{g:.0}"),
            );
            g += step;
        }
        canvas::putf(
            b,
            pos2(
                outer.left() + 8.0 * k,
                outer.top() + TRANSFER_HEAD * k * 0.5,
            ),
            Align2::LEFT_CENTER,
            Role::Body,
            ink.text,
            tk,
            None,
            format_args!(
                "{} {}  \u{b7}  {} {:.1} dB",
                frame::tr("fx.comp.ratio"),
                Ratio(cs.ratio),
                frame::tr("fx.comp.knee"),
                cs.knee_db
            ),
        );
        if let Some(e) = effective {
            canvas::put(
                b,
                pos2(rect.left() + 26.0 * k, rect.top() + 6.0 * k),
                Align2::LEFT_TOP,
                &Text::new(e, Role::Body, ink.text).caps(false),
                tk,
                Some(ink.plate),
            );
        }
    });

    // Gestures.
    let shift = ui.input(|i| i.modifiers.shift);
    if resp.drag_started() {
        if let Some(pt) = ui.input(|i| i.pointer.press_origin()) {
            let near = |q: Pos2| pt.distance(q) < 9.0 * k;
            let over_floor = floor_shown && {
                let (di, dout) = (dx_of(pt.x), dy_of(pt.y));
                ((di - dout) - cs.range_db * mix).abs() < -lo * 9.0 * k / rect.width()
            };
            st.drag = if near(t_pt) {
                Some(Grab::Threshold)
            } else if near(k_pts[0]) || near(k_pts[1]) {
                Some(Grab::Knee)
            } else if over_floor {
                Some(Grab::Range)
            } else if dx_of(pt.x) > cs.threshold_db {
                Some(Grab::Ratio)
            } else {
                None
            };
        }
    }
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            match g {
                Grab::Threshold => host.set("thr", dx_of(pt.x).clamp(-80.0, 0.0), Some("thr")),
                Grab::Knee => {
                    let kn = ((dx_of(pt.x) - cs.threshold_db).abs() * 2.0).clamp(0.0, 72.0);
                    host.set("knee", kn, Some("knee"));
                }
                Grab::Range => {
                    let r = ((dx_of(pt.x) - dy_of(pt.y)) / mix.max(0.01)).clamp(0.0, 60.0);
                    host.set("range", r, Some("range"));
                }
                Grab::Ratio => {
                    // The pointer is a point the curve should pass through: its input
                    // level over the threshold, and its output over the threshold.
                    let over_in = (dx_of(pt.x) - cs.threshold_db).max(0.5);
                    let over_out = (dy_of(pt.y) - cs.threshold_db).clamp(0.0, over_in);
                    let ratio = if over_out < 0.01 {
                        RATIO_INF
                    } else {
                        (over_in / over_out).clamp(1.0, RATIO_INF)
                    };
                    let ratio = if shift { ratio } else { snap_ratio(ratio) };
                    host.set("ratio", ratio, Some("ratio"));
                }
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    // The wheel over the curve: the knee, one step of undo per burst.
    if resp.hovered() {
        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
        if wheel != 0.0 {
            let fine = if shift { 0.2 } else { 1.0 };
            let kn = (cs.knee_db + wheel * 0.1 * fine).clamp(0.0, 72.0);
            host.set("knee", kn, Some("knee"));
            st.wheel_at = Some(st.clock);
        }
        hint::set_text(
            ui.ctx(),
            frame::tr("comp.card.transfer"),
            frame::tr("comp.transfer_tip"),
            Caps::NONE,
        );
    }
    if resp.double_clicked() {
        host.set_many(&[("ratio", 4.0), ("knee", 6.0)], "cmd.comp_reset_curve");
    }
}
/// The meters' colour for a level: the kit's three stops (−6 dB, 0 dB).
fn level_color(s: &Style, db: f32) -> egui::Color32 {
    if db >= 0.0 {
        s.t.meter_clip
    } else if db >= -6.0 {
        s.t.meter_high
    } else {
        s.t.meter_low
    }
}

/// Input, reduction and output meters on the displays' scale, with their held peaks and
/// the output's short-term loudness on its meter; beside them the numbers: loudness
/// momentary / short-term / integrated (and its reset), ten seconds of reduction, the
/// crest factor in and out, the auto gain in force, the compare's gain.
pub(super) fn meters(
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    cs: &CompSettings,
    tel: &Tele,
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let range = st.range_db;
    let last = st.live;
    let hold = st.hold;
    let col_w = 24.0 * k;
    let bar_w = 10.0 * k;
    let top = rect.top() + 20.0 * k;
    let bottom = rect.bottom() - 18.0 * k;
    let y = |d: f32| top + (-d / range).clamp(0.0, 1.0) * (bottom - top);
    let live = tel.live();
    let v = |i: usize| {
        if live {
            tel.get(i)
        } else {
            loudness::SILENCE
        }
    };
    // The numbers' column, and where the reset key goes in it.
    let nx = rect.left() + 8.0 * k + 3.0 * col_w + 6.0 * k;
    let nr = rect.right() - 8.0 * k;
    let pitch = 14.0 * k;
    let mut row = rect.top() + 6.0 * k;
    let mut reset_at = Rect::NOTHING;
    let integrated = st.integrated();
    let ok_text = crate::kit::color::ensure_contrast(ink.ok, ink.ground, 4.5);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let well_fill = s.t.meter_well;
        let meters = [
            ("comp.meter_in", last.inp, hold[0], false),
            ("comp.meter_gr", last.gr, hold[1], true),
            ("comp.meter_out", last.out, hold[2], false),
        ];
        for (i, (label, lv, hd, reduction)) in meters.into_iter().enumerate() {
            let mid = rect.left() + 8.0 * k + (i as f32 + 0.5) * col_w;
            let well = b.snap_rect(Rect::from_min_max(
                pos2(mid - bar_w * 0.5, top),
                pos2(mid + bar_w * 0.5, bottom),
            ));
            b.rect(well, Fill::Solid(well_fill));
            let inner = well.shrink(b.px());
            if reduction {
                let yb = y(-lv).max(inner.top());
                if yb > inner.top() {
                    b.rect(
                        Rect::from_min_max(inner.min, pos2(inner.right(), yb)),
                        Fill::Solid(ink.danger),
                    );
                }
                let yh = b.snap(y(-hd));
                b.rect(
                    Rect::from_min_max(pos2(inner.left(), yh), pos2(inner.right(), yh + 2.0 * k)),
                    Fill::Solid(ink.danger),
                );
            } else {
                let yl = y(lv).min(inner.bottom());
                if yl < inner.bottom() {
                    b.rect(
                        Rect::from_min_max(pos2(inner.left(), yl), inner.max),
                        Fill::Solid(level_color(s, lv)),
                    );
                }
                if hd > -range {
                    let yh = b.snap(y(hd));
                    b.rect(
                        Rect::from_min_max(
                            pos2(inner.left(), yh),
                            pos2(inner.right(), yh + 2.0 * k),
                        ),
                        Fill::Solid(level_color(s, hd)),
                    );
                }
            }
            // The output's short-term loudness, on its own meter: a tick in the ok colour.
            if i == 2 && live {
                let lu = tel.get(tele::OUT_S);
                if lu > -range {
                    b.rect(
                        Rect::from_center_size(
                            pos2(mid, b.snap(y(lu))),
                            vec2(bar_w + 6.0 * k, 2.0 * k),
                        ),
                        Fill::Solid(ink.ok),
                    );
                }
            }
            canvas::put_key(
                b,
                pos2(mid, rect.top() + 10.0 * k),
                Align2::CENTER_CENTER,
                label,
                Role::Micro,
                ink.dim,
                tk,
            );
            let color = if !reduction && hd >= 0.0 {
                ink.danger
            } else {
                ink.text
            };
            let at = pos2(mid, bottom + 9.0 * k);
            if reduction {
                canvas::putf(
                    b,
                    at,
                    Align2::CENTER_CENTER,
                    Role::Micro,
                    color,
                    tk,
                    None,
                    format_args!("{:.1}", -hd.max(0.0)),
                );
            } else if hd <= -99.0 {
                canvas::putf(
                    b,
                    at,
                    Align2::CENTER_CENTER,
                    Role::Micro,
                    color,
                    tk,
                    None,
                    format_args!("-\u{221e}"),
                );
            } else {
                canvas::putf(
                    b,
                    at,
                    Align2::CENTER_CENTER,
                    Role::Micro,
                    color,
                    tk,
                    None,
                    format_args!("{hd:.1}"),
                );
            }
        }
        b.vline(
            b.snap_mid(nx - 4.0 * k),
            rect.top() + 6.0 * k,
            rect.bottom() - 6.0 * k,
            b.px(),
            ink.grid,
        );
        // The numbers: a heading, then label | value rows.
        let heading = |b: &mut crate::kit::Brush, key: &str, row: &mut f32| {
            canvas::put_key(
                b,
                pos2(nx, *row + pitch * 0.5),
                Align2::LEFT_CENTER,
                key,
                Role::Micro,
                ink.dim,
                tk,
            );
            *row += pitch;
        };
        let pair = |b: &mut crate::kit::Brush,
                    key: &str,
                    color: egui::Color32,
                    row: &mut f32,
                    args: std::fmt::Arguments<'_>| {
            let mid = *row + pitch * 0.5;
            // The value first; the label takes what is left of the row, elided, so a long
            // word (Spanish) never runs into its number.
            let v = canvas::putf(
                b,
                pos2(nr, mid),
                Align2::RIGHT_CENTER,
                Role::Value,
                color,
                tk,
                None,
                args,
            );
            let room = Rect::from_min_max(
                pos2(nx + 4.0 * k, mid - pitch * 0.5),
                pos2((v.left() - 4.0 * k).max(nx + 4.0 * k), mid + pitch * 0.5),
            );
            let label = Text::key(key, Role::Label, ink.dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = ktext::paint(b, room, &label, tk);
            *row += pitch;
        };
        heading(b, "comp.loudness", &mut row);
        pair(
            b,
            "comp.lufs_m",
            ink.text,
            &mut row,
            format_args!("{}", Lufs(v(tele::OUT_M))),
        );
        pair(
            b,
            "comp.lufs_s",
            ok_text,
            &mut row,
            format_args!("{}", Lufs(v(tele::OUT_S))),
        );
        pair(
            b,
            "comp.lufs_i",
            ink.text,
            &mut row,
            format_args!("{}", Lufs(integrated)),
        );
        reset_at = Rect::from_min_max(pos2(nx, row + 2.0 * k), pos2(nr, row + 22.0 * k));
        row += 26.0 * k;
        heading(b, "comp.gr_stats", &mut row);
        let (avg, peak) = if live {
            (tel.get(tele::GR_AVG), tel.get(tele::GR_PEAK))
        } else {
            (0.0, 0.0)
        };
        pair(
            b,
            "comp.gr_avg",
            ink.danger,
            &mut row,
            format_args!("{avg:.1}"),
        );
        pair(
            b,
            "comp.gr_peak",
            ink.danger,
            &mut row,
            format_args!("{peak:.1}"),
        );
        heading(b, "comp.crest", &mut row);
        let (ci, co) = if live {
            (tel.get(tele::CREST_IN), tel.get(tele::CREST_OUT))
        } else {
            (0.0, 0.0)
        };
        pair(
            b,
            "comp.crest_io",
            ink.text,
            &mut row,
            format_args!("{ci:.1}\u{2192}{co:.1}"),
        );
        if cs.auto_gain {
            let g = if live { tel.get(tele::AUTO_GAIN) } else { 0.0 };
            pair(
                b,
                "fx.comp.auto_gain.short",
                ink.hue,
                &mut row,
                format_args!("{g:+.1} dB"),
            );
        }
        if cs.listen == Listen::Compare && live {
            let g = tel.get(tele::COMPARE);
            pair(
                b,
                "comp.compare",
                ink.hue,
                &mut row,
                format_args!("{g:+.1} dB"),
            );
        }
    });
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(reset_at));
    if controls::key(
        &mut child,
        s,
        Src::Key("comp.lufs_reset"),
        frame::tr("comp.lufs_reset_tip"),
        reset_at.width() / k,
        false,
    )
    .clicked()
    {
        st.reset_loudness();
    }
}

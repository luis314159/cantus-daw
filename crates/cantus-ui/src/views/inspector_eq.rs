//! The strip EQ as a display that is a control (ADR 0024 §7, FL's inspector graph):
//! drag a band's token for its frequency and gain, drag it with the right button for its
//! width, double-click it for 0 dB; under the display, one row per band with its three
//! numbers in wells. The picture is drawn from the coefficients the audio thread runs
//! (`cantus_dsp::ChannelEq`), sampled once per setting.

use super::channel::Chan;
use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Text};
use crate::kit::{cache, Brush, Fill, Function, Style};
use cantus_core::{
    Command, EqField, MergeKey, MixerStrip, StripEq, EQ_MAX_GAIN_DB, EQ_MAX_HZ, EQ_MAX_Q,
    EQ_MIN_HZ, EQ_MIN_Q,
};
use egui::{pos2, Align2, Id, PointerButton, Pos2, Rect, Sense, Vec2};

/// The display's height at 100 %.
pub const EQ_H: f32 = 72.0;
/// Points the response is sampled at, across 20 Hz … 20 kHz.
const CURVE_N: usize = 96;
const BANDS: [&str; 3] = [
    "inspector.eq_band_low",
    "inspector.eq_band_mid",
    "inspector.eq_band_high",
];

/// The strip EQ's response, sampled once per setting.
#[derive(Default)]
pub struct EqCurve {
    key: Option<[u32; 9]>,
    db: Vec<f32>,
}

impl EqCurve {
    fn update(&mut self, eq: &StripEq) {
        let mut k = [0u32; 9];
        for (i, b) in eq.bands.iter().enumerate() {
            k[i * 3] = b.freq_hz.to_bits();
            k[i * 3 + 1] = b.gain_db.to_bits();
            k[i * 3 + 2] = b.q.to_bits();
        }
        if self.key == Some(k) {
            return;
        }
        self.key = Some(k);
        let sr = 48_000.0;
        let mut dsp = cantus_dsp::ChannelEq::default();
        for (i, b) in eq.bands.iter().enumerate() {
            dsp.set_band(i, sr, b.freq_hz, b.gain_db, b.q);
        }
        self.db.clear();
        self.db.extend((0..CURVE_N).map(|i| {
            let f = i as f32 / (CURVE_N - 1) as f32;
            dsp.magnitude_db(sr, EQ_MIN_HZ * (EQ_MAX_HZ / EQ_MIN_HZ).powf(f))
        }));
    }
}

/// Where the plot sits inside the display.
fn plot_of(rect: Rect, s: &Style) -> Rect {
    rect.shrink2(Vec2::new(s.s(6.0), s.s(8.0)))
}

fn x_of(hz: f32, plot: Rect) -> f32 {
    let f = (hz.max(EQ_MIN_HZ) / EQ_MIN_HZ).ln() / (EQ_MAX_HZ / EQ_MIN_HZ).ln();
    plot.left() + f * plot.width()
}

fn hz_at(x: f32, plot: Rect) -> f32 {
    let f = ((x - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0);
    EQ_MIN_HZ * (EQ_MAX_HZ / EQ_MIN_HZ).powf(f)
}

fn y_of(db: f32, plot: Rect) -> f32 {
    plot.center().y - (db / EQ_MAX_GAIN_DB).clamp(-1.0, 1.0) * plot.height() * 0.5
}

fn db_at(y: f32, plot: Rect) -> f32 {
    ((plot.center().y - y) / (plot.height() * 0.5).max(1.0) * EQ_MAX_GAIN_DB)
        .clamp(-EQ_MAX_GAIN_DB, EQ_MAX_GAIN_DB)
}

fn token(eq: &StripEq, i: usize, plot: Rect) -> Pos2 {
    let b = &eq.bands[i];
    pos2(x_of(b.freq_hz, plot), y_of(b.gain_db, plot))
}

/// The display: grid, response (filled toward 0 dB), one numbered token per band.
pub fn eq_display(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    eq: &StripEq,
    curve: &mut EqCurve,
    selected: Option<usize>,
    hovered: Option<usize>,
) {
    b.register(
        rect,
        "eq_display",
        &[
            crate::kit::Tok::Ink,
            crate::kit::Tok::Grid,
            crate::kit::Tok::TextDim,
        ],
        &[s.hue_tok],
    );
    // r39:el-mixer — des-mixer's EQ: the curve in a quiet panel (`strip.panel`), the
    // area under it in the accent at 12 %, the 0-dB line and the decades faint, the bands
    // as small rings in their colours (filled while held or pointed at).
    let p = crate::views::mixer::look::Pal::of(s);
    let rect = b.snap_rect(rect);
    b.rrect(rect, 3.0, Fill::Solid(p.panel));
    let plot = plot_of(rect, s);
    let px = b.px();
    let faint = crate::kit::color::mix(p.panel, p.ink4, 0.45);
    for f in [100.0, 1_000.0, 10_000.0] {
        b.vline(
            b.snap_mid(x_of(f, plot)),
            rect.top() + 2.0,
            rect.bottom() - 2.0,
            px,
            faint,
        );
    }
    b.hline(
        plot.left(),
        plot.right(),
        b.snap_mid(y_of(0.0, plot)),
        px,
        faint,
    );
    curve.update(eq);
    let mut pts = [Pos2::ZERO; CURVE_N];
    for (i, q) in pts.iter_mut().enumerate() {
        let x = plot.left() + plot.width() * i as f32 / (CURVE_N - 1) as f32;
        *q = pos2(x, y_of(curve.db.get(i).copied().unwrap_or(0.0), plot));
    }
    let flat = eq.is_flat();
    if !flat {
        b.area(&pts, y_of(0.0, plot), with_alpha(p.accent, 0.12));
    }
    b.polyline(&pts, px, if flat { p.ink4 } else { p.ink2 });
    let colours = [p.solo, p.play, p.accent];
    for (i, digit) in ["1", "2", "3"].into_iter().enumerate() {
        let c = token(eq, i, plot);
        let col = colours[i];
        if selected == Some(i) || hovered == Some(i) {
            b.circle(c, 5.0, Fill::Solid(col));
            let tx = Text::new(digit, Role::Micro, p.base);
            let _ = text::paint(
                b,
                Rect::from_center_size(c, Vec2::splat(10.0)),
                &tx,
                s.tk() * 0.85,
            );
        } else {
            b.circle(c, 3.0, Fill::Solid(p.panel));
            b.circle_stroke(c, 3.0, px, col);
        }
    }
    for (f, l) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        crate::views::mixer::look::say(
            b,
            s,
            l,
            x_of(f, plot),
            rect.bottom() - 3.0,
            Align2::CENTER_BOTTOM,
            30.0,
            9.0,
            crate::views::mixer::look::W::Regular,
            p.ink3,
        );
    }
}

/// The EQ band and display, then a row per band. `None`: no strip, all dimmed.
pub fn show(sh: &mut Sheet<'_>, st: &mut Chan, ms: Option<&MixerStrip>) {
    let s = sh.s.function(Function::Tone);
    let default = StripEq::default();
    let eq = ms.map_or(&default, |m| &m.eq);
    let flat = eq.is_flat();
    let band = sh.band("kit.inspector.eq", Function::Tone, Some(!flat), false, None);
    if band.power && !flat {
        if let Some(m) = ms {
            sh.acts.push(Act::Flat(m.id));
        }
    }
    let w = sh.width();
    let (rect, _) = sh
        .ui
        .allocate_exact_size(Vec2::new(w, s.s(EQ_H)), Sense::hover());
    // r39:lo-mixer — the graph inside the section's margins (des-mixer: a panel 12 in).
    let rect = rect.shrink2(Vec2::new(s.s(sheet::MARGIN), 0.0));
    let plot = plot_of(rect, &s);
    let mut hovered = None;
    if let Some(m) = ms {
        let id = m.id;
        let r = sh
            .ui
            .interact(rect, Id::new(("insp-eq", id)), Sense::click_and_drag());
        let near = |p: Pos2| {
            (0..3)
                .map(|i| (i, token(eq, i, plot).distance(p)))
                .filter(|(_, d)| *d < s.s(16.0))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        };
        hovered = r.hover_pos().and_then(near);
        if r.drag_started() {
            st.eq_drag = r.interact_pointer_pos().and_then(near);
        }
        if let (Some(i), Some(p)) = (st.eq_drag, r.interact_pointer_pos()) {
            if r.dragged_by(PointerButton::Secondary) {
                let q =
                    (eq.bands[i].q * (-r.drag_delta().y * 0.01).exp()).clamp(EQ_MIN_Q, EQ_MAX_Q);
                push(sh, id, i, EqField::Q, q);
            } else if r.dragged() {
                push(
                    sh,
                    id,
                    i,
                    EqField::Freq,
                    hz_at(p.x, plot).clamp(EQ_MIN_HZ, EQ_MAX_HZ),
                );
                push(sh, id, i, EqField::Gain, db_at(p.y, plot));
            }
        }
        if r.drag_stopped() {
            st.eq_drag = None;
            sh.acts.push(Act::EndGesture);
        }
        if r.double_clicked() {
            if let Some(i) = r.interact_pointer_pos().and_then(near) {
                push(sh, id, i, EqField::Gain, 0.0);
                sh.acts.push(Act::EndGesture);
            }
        }
        if let Some(i) = st.eq_drag.or(hovered) {
            let b = eq.bands[i];
            let name = sheet::name(sh.fmt.tr(BANDS[i]));
            let text = sh.fmt.get(
                cache::key(("insp-eq-hint", id, i)),
                sheet::bits(b.freq_hz)
                    ^ (sheet::bits(b.gain_db) << 32)
                    ^ sheet::bits(b.q).rotate_left(16),
                |t| {
                    sheet::hz(b.freq_hz, t);
                    t.push_str(" · ");
                    sheet::db(b.gain_db, t);
                    t.push_str(" · Q ");
                    sheet::plain(b.q, t);
                },
            );
            hint::set_text(sh.ui.ctx(), &name, text, Caps::PARAM);
        } else if r.hovered() {
            // r39:el-mixer — the display between the bands says what it does.
            let name = sh.fmt.tr("inspector.eq_display_tip");
            hint::set_text(sh.ui.ctx(), name, "", Caps::NONE);
        }
    }
    eq_display(sh.b, rect, &s, eq, &mut st.eq, st.eq_drag, hovered);
    // The band under the pointer reads out in the display's corner, as FL's does.
    if let (Some(m), Some(i)) = (ms, st.eq_drag.or(hovered)) {
        let b = eq.bands[i];
        let stamp = sheet::bits(b.freq_hz) ^ (sheet::bits(b.gain_db) << 32);
        let text = sh
            .fmt
            .get(cache::key(("insp-eq-read", m.id, i)), stamp, |t| {
                short_hz(b.freq_hz, t);
                t.push_str("  ");
                signed(b.gain_db, t);
            });
        let tx = Text::new(text, Role::Body, s.hue)
            .align(egui::Align2::LEFT_TOP)
            .policy(crate::kit::text::Policy::Elide(
                crate::kit::text::Elide::End,
            ));
        let r = Rect::from_min_size(
            rect.min + Vec2::new(s.s(8.0), s.s(4.0)),
            Vec2::new(s.s(120.0), s.s(16.0)),
        );
        let _ = text::paint(sh.b, r, &tx, s.tk());
    }
    if ms.is_none() {
        sh.b.rect(rect, Fill::Solid(with_alpha(s.t.panel_light, 0.55)));
    }
    sh.ui.add_space(s.d(4.0));
    // r39:lo-mixer — des-mixer's band table: a column per band — its colour's mark and its
    // name, then its frequency (ink 2) and gain (ink 1), its width under them; each figure
    // drags (Shift fine, a double-click resets) as the wells did.
    use crate::views::mixer::look::{self, Pal, W};
    let pal = Pal::of(&s);
    let w = sh.width();
    let m = s.s(sheet::MARGIN);
    let (table, _) = sh
        .ui
        .allocate_exact_size(Vec2::new(w, s.s(50.0)), Sense::hover());
    let cw = (table.width() - 2.0 * m) / 3.0;
    let marks = [pal.solo, pal.play, pal.accent];
    let sid = ms.map(|m| m.id);
    let dim = ms.is_none();
    let a = if dim { 0.45 } else { 1.0 };
    for (i, key) in BANDS.iter().enumerate() {
        let x = table.left() + m + cw * i as f32;
        let col = Rect::from_min_max(
            pos2(x, table.top()),
            pos2(x + cw - s.s(8.0), table.bottom()),
        );
        let b = eq.bands[i];
        // The mark and the name.
        sh.b.rect(
            sh.b.snap_rect(Rect::from_min_size(
                pos2(col.left(), col.top() + s.s(8.0)),
                Vec2::new(6.0, 2.0),
            )),
            Fill::Solid(marks[i].gamma_multiply(a)),
        );
        // The band's name, its short form where the column is narrow ("Shelf agudo" →
        // "Agudo").
        let full = sheet::name(sh.fmt.tr(key));
        let fits =
            look::width_of(sh.ui.ctx(), &s, &full, 10.0, W::Regular) <= col.width() - s.s(10.0);
        let name = if fits {
            full
        } else {
            let mut k = String::with_capacity(32);
            k.push_str(key);
            k.push_str(".short");
            sheet::name(sh.fmt.tr(&k))
        };
        look::say(
            sh.b,
            &s,
            &name,
            col.left() + s.s(10.0),
            col.top() + s.s(12.0),
            Align2::LEFT_BOTTOM,
            col.width() - s.s(10.0),
            10.0,
            W::Regular,
            pal.ink3.gamma_multiply(a),
        );
        let fields = [
            (
                EqField::Freq,
                b.freq_hz,
                Range::log(EQ_MIN_HZ, EQ_MAX_HZ, default.bands[i].freq_hz),
                "inspector.eq_freq",
            ),
            (
                EqField::Gain,
                b.gain_db,
                Range::lin(-EQ_MAX_GAIN_DB, EQ_MAX_GAIN_DB, 0.0).bipolar(),
                "inspector.eq_gain",
            ),
            (
                EqField::Q,
                b.q,
                Range::log(EQ_MIN_Q, EQ_MAX_Q, 0.71),
                "inspector.eq_q",
            ),
        ];
        // Frequency and gain on one line where both fit whole, else the gain goes under
        // the frequency, beside the width; a narrow column writes the frequency short.
        let ctx = sh.ui.ctx().clone();
        let stamp = sheet::bits(b.freq_hz) ^ (sheet::bits(b.gain_db) << 32);
        let long = {
            let f = sh.fmt.get(cache::key(("insp-eq-fw", sid, i)), stamp, |t| {
                sheet::hz(b.freq_hz, t)
            });
            look::width_of(&ctx, &s, f, 11.0, W::Regular)
        };
        let gw = {
            let g = sh.fmt.get(cache::key(("insp-eq-gw", sid, i)), stamp, |t| {
                signed(b.gain_db, t)
            });
            look::width_of(&ctx, &s, g, 11.0, W::Regular)
        };
        let short = long + 4.0 > col.width();
        let one_line = long + gw + 8.0 <= col.width();
        let line = |n: f32| col.top() + s.s(16.0) * n;
        let cells = if one_line {
            [
                Rect::from_min_size(
                    pos2(col.left(), line(1.0)),
                    Vec2::new(col.width() - gw - 8.0, s.s(16.0)),
                ),
                Rect::from_min_size(
                    pos2(col.right() - gw - 4.0, line(1.0)),
                    Vec2::new(gw + 4.0, s.s(16.0)),
                ),
                Rect::from_min_size(
                    pos2(col.left(), line(2.0)),
                    Vec2::new(col.width(), s.s(16.0)),
                ),
            ]
        } else {
            let half = col.width() * 0.5;
            [
                Rect::from_min_size(
                    pos2(col.left(), line(1.0)),
                    Vec2::new(col.width(), s.s(16.0)),
                ),
                Rect::from_min_size(pos2(col.left(), line(2.0)), Vec2::new(half, s.s(16.0))),
                Rect::from_min_size(
                    pos2(col.left() + half, line(2.0)),
                    Vec2::new(half, s.s(16.0)),
                ),
            ]
        };
        for (k, (field, v, range, nkey)) in fields.into_iter().enumerate() {
            let cell = cells[k];
            let id = Id::new(("insp-eqv", sid, i, k));
            let d = sheet::drag(sh.ui, id, cell, v, &range, false);
            let shown = d.value.unwrap_or(v);
            let text = sh.fmt.get(
                cache::key(("insp-eqt", sid, i, k, short)),
                sheet::bits(shown),
                |t| match field {
                    EqField::Freq if short => short_hz(shown, t),
                    EqField::Freq => sheet::hz(shown, t),
                    EqField::Gain => signed(shown, t),
                    EqField::Q => {
                        t.push_str("Q ");
                        short_q(shown, t);
                    }
                },
            );
            let hot = !dim && (d.resp.hovered() || d.resp.dragged());
            let (size, ink, align, x) = match field {
                EqField::Freq => (11.0, pal.ink2, Align2::LEFT_BOTTOM, cell.left()),
                EqField::Gain => (11.0, pal.ink1, Align2::RIGHT_BOTTOM, cell.right()),
                EqField::Q => (10.0, pal.ink3, Align2::LEFT_BOTTOM, cell.left()),
            };
            look::say(
                sh.b,
                &s,
                text,
                x,
                cell.center().y + 4.0,
                align,
                cell.width(),
                size,
                W::Regular,
                (if hot { pal.ink1 } else { ink }).gamma_multiply(a),
            );
            if hot {
                let name = sheet::name(sh.fmt.tr(nkey));
                let text = sheet::name(sh.fmt.get(
                    cache::key(("insp-eqt", sid, i, k)),
                    sheet::bits(shown),
                    |_| {},
                ));
                hint::set_text(sh.ui.ctx(), &name, &text, Caps::PARAM);
            }
            if let (Some(nv), Some(id), false) = (d.value, sid, dim) {
                push(sh, id, i, field, nv);
            }
            if d.ended && !dim {
                sh.acts.push(Act::EndGesture);
            }
        }
    }
}

/// "90", "1.2k", "12k": a frequency under the legend's "Hz".
fn short_hz(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = if v >= 10_000.0 {
        write!(b, "{:.0}k", v / 1_000.0)
    } else if v >= 1_000.0 {
        write!(b, "{:.1}k", v / 1_000.0)
    } else {
        write!(b, "{v:.0}")
    };
}

/// "+2.5", "−3.5", "+12": a gain under the legend's "dB", in four characters at most so
/// it fits the narrowest well.
fn signed(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let r = (v * 10.0).round() / 10.0;
    let sign = if r < 0.0 { "\u{2212}" } else { "+" };
    let _ = if r.abs() >= 10.0 {
        write!(b, "{sign}{:.0}", r.abs())
    } else {
        write!(b, "{sign}{:.1}", r.abs())
    };
}

/// "0.71", "10.0": a width in four characters at most.
fn short_q(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = if v >= 10.0 {
        write!(b, "{v:.1}")
    } else {
        write!(b, "{v:.2}")
    };
}

fn push(sh: &mut Sheet<'_>, id: cantus_core::StripId, band: usize, field: EqField, value: f32) {
    sh.acts.push(Act::Merge(
        Command::SetStripEq {
            id,
            band: band as u8,
            field,
            value,
        },
        MergeKey::of(&("seq", id.0, band as u8, field as u8)),
    ));
}

//! The strip EQ as a display that is a control (ADR 0024 §7, FL's inspector graph):
//! drag a band's token for its frequency and gain, drag it with the right button for its
//! width, double-click it for 0 dB; under the display, one row per band with its three
//! numbers in wells. The picture is drawn from the coefficients the audio thread runs
//! (`cantus_dsp::ChannelEq`), sampled once per setting.

use super::channel::Chan;
use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::kit::color::{shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Text};
use crate::kit::{cache, light, Brush, Fill, Function, Style};
use cantus_core::{
    Command, EqField, MergeKey, MixerStrip, StripEq, EQ_MAX_GAIN_DB, EQ_MAX_HZ, EQ_MAX_Q,
    EQ_MIN_HZ, EQ_MIN_Q,
};
use egui::{pos2, Id, PointerButton, Pos2, Rect, Sense, Vec2};

/// The display's height at 100 %.
pub const EQ_H: f32 = 112.0;
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
    rect.shrink2(Vec2::new(0.0, s.s(8.0)))
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
    let t = &s.t;
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
    let rect = b.snap_rect(rect);
    b.rect(
        rect,
        Fill::V(shade(t.ink, if t.dark { 0.025 } else { 0.0 }), t.ink),
    );
    let plot = plot_of(rect, s);
    let px = b.px();
    let minor = with_alpha(t.grid, if t.dark { 0.5 } else { 0.55 });
    let major = if t.dark {
        shade(t.grid, 0.07)
    } else {
        t.grid_strong
    };
    for f in [
        30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0,
        900.0, 2e3, 3e3, 4e3, 5e3, 6e3, 7e3, 8e3, 9e3,
    ] {
        b.vline(
            b.snap_mid(x_of(f, plot)),
            plot.top(),
            plot.bottom(),
            px,
            minor,
        );
    }
    for f in [100.0, 1_000.0, 10_000.0] {
        b.vline(
            b.snap_mid(x_of(f, plot)),
            rect.top(),
            rect.bottom(),
            px,
            major,
        );
    }
    let mut db = -12.0;
    while db <= 12.0 {
        let c = if db == 0.0 { shade(major, 0.05) } else { minor };
        b.hline(plot.left(), plot.right(), b.snap_mid(y_of(db, plot)), px, c);
        db += 6.0;
    }
    curve.update(eq);
    let mut pts = [Pos2::ZERO; CURVE_N];
    for (i, p) in pts.iter_mut().enumerate() {
        let x = plot.left() + plot.width() * i as f32 / (CURVE_N - 1) as f32;
        *p = pos2(x, y_of(curve.db.get(i).copied().unwrap_or(0.0), plot));
    }
    let flat = eq.is_flat();
    let hue = if flat { t.text_dim } else { s.hue };
    b.area(
        &pts,
        y_of(0.0, plot),
        with_alpha(hue, if t.dark { 0.16 } else { 0.12 }),
    );
    b.polyline(&pts, s.s(2.0), hue);
    for (i, digit) in ["1", "2", "3"].into_iter().enumerate() {
        let c = token(eq, i, plot);
        let r = s.s(if hovered == Some(i) || selected == Some(i) {
            7.0
        } else {
            6.0
        });
        if selected == Some(i) {
            light::glow(b, c, r * 2.2, s.hue, 0.8);
            b.circle(c, r, Fill::Solid(s.hue));
            let tx = Text::new(digit, Role::Micro, t.ink);
            let _ = text::paint(
                b,
                Rect::from_center_size(c, Vec2::splat(r * 2.0)),
                &tx,
                s.tk(),
            );
        } else {
            b.circle(c, r, Fill::Solid(with_alpha(t.ink, 0.85)));
            b.circle_stroke(c, r - s.s(0.75), s.s(1.5), hue);
            let tx = Text::new(digit, Role::Micro, hue);
            let _ = text::paint(
                b,
                Rect::from_center_size(c, Vec2::splat(r * 2.0)),
                &tx,
                s.tk(),
            );
        }
    }
    for (f, l) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        let r = Rect::from_center_size(
            pos2(x_of(f, plot) + s.s(12.0), rect.bottom() - s.s(7.0)),
            Vec2::new(s.s(28.0), s.s(12.0)),
        );
        let _ = text::paint(
            b,
            r,
            &Text::new(l, Role::Micro, t.text_dim).caps(false),
            s.tk(),
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
    // The numbers: frequency, gain and width of each band, in wells, under a legend
    // printed once for the three rows (the units live there, so the wells hold digits).
    let legend = {
        let w = sh.width();
        let (r, _) = sh
            .ui
            .allocate_exact_size(Vec2::new(w, s.s(12.0)), Sense::hover());
        r
    };
    let mut first = true;
    for (i, key) in BANDS.iter().enumerate() {
        let row = sh.row();
        let gap = s.s(4.0);
        // Three wells of at least 44 pt: on a narrow panel the label column gives way
        // (and the band's name shortens to "Low", "Mid", "High").
        let need = 3.0 * s.s(44.0) + 2.0 * gap;
        let (label, value) = if row.value.width() >= need {
            (row.label, row.value)
        } else {
            let vl = row.value.right() - need;
            (
                Rect::from_min_max(
                    row.label.min,
                    pos2(vl - s.s(sheet::GAP), row.label.bottom()),
                ),
                Rect::from_min_max(pos2(vl, row.value.top()), row.value.max),
            )
        };
        sh.label(label, key, ms.is_none());
        let cw = (value.width() - 2.0 * gap) / 3.0;
        let cell = |k: usize| {
            Rect::from_min_size(
                pos2(value.left() + k as f32 * (cw + gap), value.top()),
                Vec2::new(cw, value.height()),
            )
        };
        if first {
            first = false;
            for (k, lk) in [
                "inspector.eq_legend_freq",
                "inspector.eq_legend_gain",
                "inspector.eq_legend_q",
            ]
            .into_iter()
            .enumerate()
            {
                let c = cell(k);
                let r = Rect::from_min_max(
                    pos2(c.left(), legend.top()),
                    pos2(c.right(), legend.bottom()),
                );
                let tx = Text::key(
                    lk,
                    Role::Micro,
                    s.t.text_dim
                        .gamma_multiply(if ms.is_none() { 0.45 } else { 1.0 }),
                )
                .caps(false);
                let _ = text::paint(sh.b, r, &tx, s.tk());
            }
        }
        let b = eq.bands[i];
        let sid = ms.map(|m| m.id);
        let dim = ms.is_none();
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
        for (k, (field, v, range, nkey)) in fields.into_iter().enumerate() {
            let name = sheet::name(sh.fmt.tr(nkey));
            let d = sh.value(
                cell(k),
                Id::new(("insp-eqv", sid, i, k)),
                v,
                &range,
                false,
                &name,
                |v, t| match field {
                    EqField::Freq => short_hz(v, t),
                    EqField::Gain => signed(v, t),
                    EqField::Q => short_q(v, t),
                },
                Function::Tone,
                dim,
            );
            if let (Some(nv), Some(id)) = (d.value, sid) {
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

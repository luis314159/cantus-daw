//! Comparatio's screen (design sheet v1), dark glass in every theme, three pictures:
//!
//! - **Tonal balance**: your mix's long-term spectrum and the reference's, each relative to
//!   its own loudness (level never shows as balance), in their two voices, over a
//!   logarithmic axis 20 Hz – 20 kHz; under them, on its own zero line, **the difference**
//!   (mix − reference) with its uncertainty as a soft band, fading where the confidence is
//!   low; over it **the match EQ's suggestion** (phase B) — what *Send to Aequalis* would
//!   put before the unit, drawn before anything is sent — with its key.
//! - **Dynamics**: the two short-term loudness histograms mirrored on one axis (your mix
//!   above, the reference below), with the crest factor, peak to loudness and loudness range
//!   of each.
//! - **Stereo**: correlation (−1 … +1) and width for the whole, the lows, the mids and the
//!   highs, your mix's and the reference's needles paired on each scale.

use super::art::{c, voices};
use super::picture::{num, Picture};
use crate::kit::color::with_alpha;
use crate::kit::skin::comparator as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::comparatio::measure::{HIST_BINS, HIST_LO};
use cantus_engine::fx::comparatio::{R_CREST, R_PLR, R_RANGE};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// Where the screen's pictures are.
#[derive(Copy, Clone, Debug)]
pub struct Plot {
    pub glass: Rect,
    pub tonal: Rect,
    pub diff: Rect,
    pub dynamics: Rect,
    pub stereo: Rect,
}

impl Plot {
    pub fn lay(glass: Rect, k: f32) -> Plot {
        let inner = glass.shrink2(vec2(14.0 * k, 10.0 * k));
        let low_h = (inner.height() * 0.34).max(150.0 * k);
        let top = Rect::from_min_max(
            inner.min,
            pos2(inner.right(), inner.bottom() - low_h - 12.0 * k),
        );
        let tonal = Rect::from_min_max(
            pos2(top.left() + 44.0 * k, top.top() + 30.0 * k),
            pos2(top.right(), top.bottom() - top.height() * 0.30),
        );
        let diff = Rect::from_min_max(
            pos2(tonal.left(), tonal.bottom() + 24.0 * k),
            pos2(tonal.right(), top.bottom() - 18.0 * k),
        );
        let low = Rect::from_min_max(pos2(inner.left(), top.bottom() + 12.0 * k), inner.max);
        let split = low.left() + low.width() * 0.52;
        Plot {
            glass,
            tonal,
            diff,
            dynamics: Rect::from_min_max(low.min, pos2(split - 8.0 * k, low.bottom())),
            stereo: Rect::from_min_max(pos2(split + 8.0 * k, low.top()), low.max),
        }
    }
}

fn ink() -> Color32 {
    c(pal::SCREEN_INK)
}
fn dim() -> Color32 {
    c(pal::SCREEN_DIM)
}
fn grat() -> Color32 {
    with_alpha(c(pal::GRATICULE), 0.7)
}

fn put(b: &mut Brush, k: f32, at: Pos2, align: Align2, t: &Text<'_>) -> Rect {
    super::super::canvas::put(b, at, align, t, k, None)
}

fn x_of(r: Rect, hz: f32) -> f32 {
    let f = (cantus_dsp::math::log10(hz.max(20.0) / 20.0) / 3.0).clamp(0.0, 1.0);
    r.left() + f * r.width()
}

/// Where *Send to Aequalis* sits: on the difference's title row, at its right.
pub fn send_rect(ctx: &egui::Context, k: f32, p: &Plot) -> Rect {
    let t = Text::key("comparatio.d.send", Role::Micro, Color32::TRANSPARENT).caps(true);
    let w = crate::kit::text::measure(ctx, &t, k).x + 22.0 * k;
    Rect::from_min_size(
        pos2(p.diff.right() - w, p.diff.top() - 21.0 * k),
        vec2(w, 17.0 * k),
    )
}

pub fn paint(
    b: &mut Brush,
    s: &Style,
    p: &Plot,
    pic: &Picture,
    tonal_res: usize,
    send: Option<Rect>,
    send_hot: bool,
) {
    let k = s.k;
    let (mv, rv) = voices(s);
    tonal(b, s, p, pic, mv, rv, tonal_res);
    if let Some(r) = send {
        send_key(b, k, r, send_hot);
    }
    dynamics(b, s, p.dynamics, pic, mv, rv);
    stereo(b, s, p.stereo, pic, mv, rv);
    let _ = k;
    super::art::glass_sheen(b, s, p.glass);
}

fn title(b: &mut Brush, k: f32, at: Pos2, key: &str) -> Rect {
    put(
        b,
        k,
        at,
        Align2::LEFT_CENTER,
        &Text::key(key, Role::Micro, ink()),
    )
}

fn tonal(b: &mut Brush, s: &Style, p: &Plot, pic: &Picture, mv: Color32, rv: Color32, res: usize) {
    let k = s.k;
    let r = p.tonal;
    let tr = title(
        b,
        k,
        pos2(p.glass.left() + 14.0 * k, p.glass.top() + 18.0 * k),
        "comparatio.d.tonal",
    );
    // The legend: each voice named, then the resolution.
    let mut x = tr.right() + 22.0 * k;
    for (key, col) in [
        ("comparatio.d.mix", mv),
        ("comparatio.d.reference", rv),
        ("comparatio.d.difference", c(pal::DIFF)),
    ] {
        b.segment(
            pos2(x, tr.center().y),
            pos2(x + 14.0 * k, tr.center().y),
            1.6 * k,
            col,
            false,
        );
        let w = put(
            b,
            k,
            pos2(x + 18.0 * k, tr.center().y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, ink()),
        );
        x = w.right() + 16.0 * k;
    }
    let rk = if res == 0 {
        "fx.comparatio.res.0"
    } else {
        "fx.comparatio.res.1"
    };
    put(
        b,
        k,
        pos2(p.glass.right() - 14.0 * k, tr.center().y),
        Align2::RIGHT_CENTER,
        &Text::key(rk, Role::Micro, dim()).caps(false),
    );
    // The graticule: decades and octaves, the balance in dB.
    for hz in [
        31.5f32, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
    ] {
        let x = b.snap_mid(x_of(r, hz));
        b.vline(x, r.top(), r.bottom(), 0.5 * k, grat());
        b.vline(x, p.diff.top(), p.diff.bottom(), 0.5 * k, grat());
    }
    for (hz, w) in [
        (31.5f32, "31.5"),
        (125.0, "125"),
        (500.0, "500"),
        (2_000.0, "2 k"),
        (8_000.0, "8 k"),
    ] {
        put(
            b,
            k,
            pos2(x_of(r, hz), p.diff.bottom() + 9.0 * k),
            Align2::CENTER_CENTER,
            &Text::new(w, Role::Micro, dim()).caps(false),
        );
    }
    put(
        b,
        k,
        pos2(r.right(), p.diff.bottom() + 9.0 * k),
        Align2::RIGHT_CENTER,
        &Text::key("comparatio.d.hz", Role::Micro, dim()).caps(false),
    );
    let y_of = |db: f32| r.top() + (1.0 - ((db + 50.0) / 50.0).clamp(0.0, 1.0)) * r.height();
    for (db, w) in [
        (0.0f32, "0"),
        (-10.0, "-10"),
        (-20.0, "-20"),
        (-30.0, "-30"),
        (-40.0, "-40"),
    ] {
        let y = b.snap_mid(y_of(db));
        b.hline(r.left(), r.right(), y, 0.5 * k, grat());
        put(
            b,
            k,
            pos2(r.left() - 6.0 * k, y),
            Align2::RIGHT_CENTER,
            &Text::new(w, Role::Micro, dim()).caps(false),
        );
    }
    put(
        b,
        k,
        pos2(r.left() - 6.0 * k, r.top() - 10.0 * k),
        Align2::RIGHT_CENTER,
        &Text::new("dB", Role::Micro, dim()).caps(false),
    );
    let curve_pts = |cv: &cantus_dsp::comparatio::tonal::Curve| -> Vec<Pos2> {
        (0..cv.bands.count)
            .filter(|i| cv.bands.centre[*i] >= 20.0 && cv.bands.centre[*i] <= 20_000.0)
            .map(|i| pos2(x_of(r, cv.bands.centre[i]), y_of(cv.db[i] - 6.0)))
            .collect()
    };
    // Each curve as it is there: the reference's from its file at once, yours once enough
    // of it was heard.
    if let Some(rf) = &pic.ref_curve {
        let pr = curve_pts(rf);
        b.area(&pr, r.bottom(), with_alpha(rv, 0.10));
        b.glow_polyline(&pr, 1.6 * k, rv, with_alpha(rv, 0.25), 4.0 * k);
    }
    if let Some(m) = &pic.mix_curve {
        let pm = curve_pts(m);
        b.area(&pm, r.bottom(), with_alpha(mv, 0.10));
        b.glow_polyline(&pm, 1.6 * k, mv, with_alpha(mv, 0.25), 4.0 * k);
    }
    let empty = match (&pic.mix_curve, &pic.ref_curve) {
        (_, None) => Some("comparatio.d.no_reference"),
        (None, Some(_)) => Some("comparatio.d.listening"),
        _ => None,
    };
    if let Some(key) = empty {
        put(
            b,
            k,
            pos2(r.center().x, r.top() + 16.0 * k),
            Align2::CENTER_CENTER,
            &Text::key(key, Role::Label, dim()).caps(false),
        );
    }
    // The difference, on its own zero line: ±12 dB.
    let d = p.diff;
    let zy = d.center().y;
    let dt = put(
        b,
        k,
        pos2(p.glass.left() + 14.0 * k, d.top() - 11.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("comparatio.d.diff_title", Role::Micro, ink()),
    );
    // The match EQ's legend, when there is one to show.
    if pic.match_eq.is_some() {
        let x = dt.right() + 22.0 * k;
        b.segment(
            pos2(x, dt.center().y),
            pos2(x + 14.0 * k, dt.center().y),
            1.6 * k,
            c(pal::MATCH),
            false,
        );
        put(
            b,
            k,
            pos2(x + 18.0 * k, dt.center().y),
            Align2::LEFT_CENTER,
            &Text::key("comparatio.d.match", Role::Micro, ink()),
        );
    }
    for (db, w) in [(12.0f32, "+12"), (0.0, "0"), (-12.0, "-12")] {
        let y = b.snap_mid(zy - db / 12.0 * d.height() * 0.5);
        b.hline(
            d.left(),
            d.right(),
            y,
            if db == 0.0 { 0.9 * k } else { 0.5 * k },
            grat(),
        );
        put(
            b,
            k,
            pos2(d.left() - 6.0 * k, y),
            Align2::RIGHT_CENTER,
            &Text::new(w, Role::Micro, dim()).caps(false),
        );
    }
    if let Some(df) = &pic.diff {
        let dy = |v: f32| zy - (v / 12.0).clamp(-1.0, 1.0) * d.height() * 0.5;
        let idx: Vec<usize> = (0..df.bands.count)
            .filter(|i| df.bands.centre[*i] >= 20.0 && df.bands.centre[*i] <= 20_000.0)
            .collect();
        let up: Vec<Pos2> = idx
            .iter()
            .map(|i| {
                pos2(
                    x_of(d, df.bands.centre[*i]),
                    dy(df.db[*i] + df.uncertainty[*i]),
                )
            })
            .collect();
        let dn: Vec<Pos2> = idx
            .iter()
            .map(|i| {
                pos2(
                    x_of(d, df.bands.centre[*i]),
                    dy(df.db[*i] - df.uncertainty[*i]),
                )
            })
            .collect();
        b.between(&up, &dn, with_alpha(c(pal::DIFF), 0.16));
        for w in idx.windows(2) {
            let (i, j) = (w[0], w[1]);
            let conf = 0.5 * (df.confidence[i] + df.confidence[j]);
            let a = pos2(x_of(d, df.bands.centre[i]), dy(df.db[i]));
            let z = pos2(x_of(d, df.bands.centre[j]), dy(df.db[j]));
            b.segment(
                a,
                z,
                1.5 * k,
                with_alpha(c(pal::DIFF), 0.25 + 0.75 * conf),
                true,
            );
        }
        // The match EQ's suggestion: what Aequalis would do to your mix (toward the
        // reference), on the same scale.
        if let Some(m) = &pic.match_eq {
            let pts: Vec<Pos2> = m
                .points()
                .iter()
                .filter(|(hz, _)| (20.0..=20_000.0).contains(hz))
                .map(|(hz, db)| pos2(x_of(d, *hz), dy(*db)))
                .collect();
            b.glow_polyline(
                &pts,
                1.4 * k,
                c(pal::MATCH),
                with_alpha(c(pal::MATCH), 0.22),
                3.0 * k,
            );
        }
        // Which way: the words at the ends of the zero line.
        put(
            b,
            k,
            pos2(d.right() - 4.0 * k, d.top() + 7.0 * k),
            Align2::RIGHT_CENTER,
            &Text::key("comparatio.d.more_mix", Role::Micro, mv).caps(false),
        );
        put(
            b,
            k,
            pos2(d.right() - 4.0 * k, d.bottom() - 7.0 * k),
            Align2::RIGHT_CENTER,
            &Text::key("comparatio.d.more_ref", Role::Micro, rv).caps(false),
        );
    }
}

/// *Send to Aequalis*: a small key lit in the match EQ's colour, on the dark glass.
fn send_key(b: &mut Brush, k: f32, r: Rect, hot: bool) {
    b.rrect(
        r,
        3.0 * k,
        Fill::Solid(with_alpha(c(pal::MATCH), if hot { 0.30 } else { 0.16 })),
    );
    b.rrect_stroke(r, 3.0 * k, 0.8 * k, with_alpha(c(pal::MATCH), 0.85));
    put(
        b,
        k,
        r.center(),
        Align2::CENTER_CENTER,
        &Text::key("comparatio.d.send", Role::Micro, c(pal::MATCH)).caps(true),
    );
}

fn dynamics(b: &mut Brush, s: &Style, r: Rect, pic: &Picture, mv: Color32, rv: Color32) {
    let k = s.k;
    b.rrect_stroke(r, 3.0 * k, 0.6 * k, grat());
    title(
        b,
        k,
        pos2(r.left() + 8.0 * k, r.top() + 10.0 * k),
        "comparatio.d.dynamics",
    );
    let plot = Rect::from_min_max(
        pos2(r.left() + 10.0 * k, r.top() + 22.0 * k),
        pos2(r.left() + r.width() * 0.42, r.bottom() - 20.0 * k),
    );
    let mid = plot.center().y;
    b.hline(plot.left(), plot.right(), b.snap_mid(mid), 0.7 * k, grat());
    let top = |h: &[f32; HIST_BINS]| h.iter().fold(1.0f32, |m, v| m.max(*v));
    let (tm, tr) = (top(&pic.mix_hist), top(&pic.ref_hist));
    let bw = plot.width() / 40.0;
    // Bins from −50 to −10 LUFS (40 of them).
    for i in 0..40 {
        let bin = (i as f64 - 50.0 - HIST_LO) as usize;
        if bin >= HIST_BINS {
            continue;
        }
        let x = plot.left() + i as f32 * bw;
        let hm = pic.mix_hist[bin] / tm * (plot.height() * 0.5 - 2.0 * k);
        let hr = pic.ref_hist[bin] / tr * (plot.height() * 0.5 - 2.0 * k);
        if hm > 0.0 {
            b.rect(
                Rect::from_min_max(pos2(x + 0.5, mid - hm), pos2(x + bw - 0.5, mid - 0.5)),
                Fill::V(mv, with_alpha(mv, 0.4)),
            );
        }
        if hr > 0.0 {
            b.rect(
                Rect::from_min_max(pos2(x + 0.5, mid + 0.5), pos2(x + bw - 0.5, mid + hr)),
                Fill::V(with_alpha(rv, 0.4), rv),
            );
        }
    }
    for (lufs, w) in [(-40.0f32, "-40"), (-30.0, "-30"), (-20.0, "-20")] {
        let x = plot.left() + (lufs + 50.0) * bw;
        put(
            b,
            k,
            pos2(x, plot.bottom() + 9.0 * k),
            Align2::CENTER_CENTER,
            &Text::new(w, Role::Micro, dim()).caps(false),
        );
    }
    put(
        b,
        k,
        pos2(plot.right(), plot.bottom() + 9.0 * k),
        Align2::RIGHT_CENTER,
        &Text::new("LUFS", Role::Micro, dim()).caps(false),
    );
    // The numbers beside it: crest factor, peak to loudness, loudness range — mix and
    // reference side by side, each named.
    let col0 = plot.right() + 14.0 * k;
    let colm = r.right() - 70.0 * k;
    let colr = r.right() - 10.0 * k;
    let mut y = r.top() + 30.0 * k;
    put(
        b,
        k,
        pos2(colm, y),
        Align2::RIGHT_CENTER,
        &Text::key("comparatio.d.mix_short", Role::Micro, mv),
    );
    put(
        b,
        k,
        pos2(colr, y),
        Align2::RIGHT_CENTER,
        &Text::key("comparatio.d.reference_short", Role::Micro, rv),
    );
    let mut buf = String::new();
    for (key, idx) in [
        ("comparatio.d.crest", R_CREST),
        ("comparatio.d.plr", R_PLR),
        ("comparatio.d.lra", R_RANGE),
    ] {
        y += 19.0 * k;
        put(
            b,
            k,
            pos2(col0, y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, ink()).caps(false),
        );
        num(pic.mix[idx], 1, &mut buf);
        buf.push_str(if idx == R_RANGE { " LU" } else { " dB" });
        put(
            b,
            k,
            pos2(colm, y),
            Align2::RIGHT_CENTER,
            &Text::new(&buf, Role::Micro, mv).caps(false),
        );
        num(pic.reference[idx], 1, &mut buf);
        buf.push_str(if idx == R_RANGE { " LU" } else { " dB" });
        put(
            b,
            k,
            pos2(colr, y),
            Align2::RIGHT_CENTER,
            &Text::new(&buf, Role::Micro, rv).caps(false),
        );
    }
}

fn stereo(b: &mut Brush, s: &Style, r: Rect, pic: &Picture, mv: Color32, rv: Color32) {
    let k = s.k;
    b.rrect_stroke(r, 3.0 * k, 0.6 * k, grat());
    title(
        b,
        k,
        pos2(r.left() + 8.0 * k, r.top() + 10.0 * k),
        "comparatio.d.stereo",
    );
    let names = [
        "comparatio.d.whole",
        "comparatio.d.lows",
        "comparatio.d.mids",
        "comparatio.d.highs",
    ];
    let label_w = 64.0 * k;
    let scale = Rect::from_min_max(
        pos2(r.left() + 10.0 * k + label_w, r.top() + 34.0 * k),
        pos2(r.right() - 64.0 * k, r.bottom() - 20.0 * k),
    );
    // The correlation scale: −1, 0, +1.
    for (v, w) in [(-1.0f32, "-1"), (0.0, "0"), (1.0, "+1")] {
        let x = scale.left() + (v + 1.0) * 0.5 * scale.width();
        b.vline(
            b.snap_mid(x),
            scale.top() - 6.0 * k,
            scale.bottom(),
            0.5 * k,
            grat(),
        );
        put(
            b,
            k,
            pos2(x, scale.top() - 13.0 * k),
            Align2::CENTER_CENTER,
            &Text::new(w, Role::Micro, dim()).caps(false),
        );
    }
    put(
        b,
        k,
        pos2(r.right() - 10.0 * k, scale.top() - 13.0 * k),
        Align2::RIGHT_CENTER,
        &Text::key("comparatio.d.width", Role::Micro, dim()).caps(false),
    );
    let row_h = scale.height() / 4.0;
    let mut buf = String::new();
    for (i, key) in names.iter().enumerate() {
        let cy = scale.top() + (i as f32 + 0.5) * row_h;
        put(
            b,
            k,
            pos2(r.left() + 10.0 * k, cy),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, ink()).caps(false),
        );
        b.hline(
            scale.left(),
            scale.right(),
            b.snap_mid(cy),
            0.5 * k,
            with_alpha(c(pal::GRATICULE), 0.4),
        );
        for (j, (st, col)) in [(&pic.mix_stereo, mv), (&pic.ref_stereo, rv)]
            .iter()
            .enumerate()
        {
            let (corr, width) = st[i];
            let dy = if j == 0 { -3.0 * k } else { 3.0 * k };
            if corr.is_finite() {
                let x = scale.left() + (corr.clamp(-1.0, 1.0) + 1.0) * 0.5 * scale.width();
                b.segment(
                    pos2(x, cy + dy - 4.0 * k),
                    pos2(x, cy + dy + 4.0 * k),
                    2.0 * k,
                    *col,
                    true,
                );
            }
            num(width, 2, &mut buf);
            let wx = r.right() - 10.0 * k - if j == 0 { 30.0 * k } else { 0.0 };
            put(
                b,
                k,
                pos2(wx, cy),
                Align2::RIGHT_CENTER,
                &Text::new(&buf, Role::Micro, *col).caps(false),
            );
        }
    }
}

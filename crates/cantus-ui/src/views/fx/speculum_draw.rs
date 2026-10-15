//! Speculum's displays, painted with the kit from the measurements (`View::analysis`).

use super::super::canvas::{self, Ink};
use super::super::dynfx;
use super::super::frame;
use super::View;
use crate::kit::color::mix;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_dsp::speculum::{Channel, SpeculumSettings, Sync, HI_HZ, LO_HZ, SPECTRO_BANDS, SPECTRO_ROWS};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The display of view `which` (0 overview … 5 loudness) in `rect`.
pub fn view(
    ui: &mut egui::Ui,
    s: &Style,
    ink: &Ink,
    rect: Rect,
    v: &mut View,
    set: &SpeculumSettings,
    which: usize,
) {
    let k = s.k;
    if which == 2 {
        spectrogram_texture(ui.ctx(), v, ink, set);
    }
    with_brush(&ui.painter_at(rect), |b| {
        match which {
            1 => spectrum(b, rect, ink, k, v, set),
            2 => spectrogram(b, rect, ink, k, v, set),
            3 => scope(b, rect, ink, k, v, set),
            4 => vector(b, rect, ink, k, v),
            5 => loudness(b, rect, ink, k, v, set),
            _ => {
                let gap = 4.0 * k;
                let w = (rect.width() - gap) * 0.62;
                let top = (rect.height() - gap) * 0.58;
                let a = Rect::from_min_size(rect.min, vec2(w, top));
                let bb = Rect::from_min_max(pos2(a.right() + gap, rect.top()), pos2(rect.right(), a.bottom()));
                let c = Rect::from_min_max(pos2(rect.left(), a.bottom() + gap), pos2(a.right(), rect.bottom()));
                let d = Rect::from_min_max(pos2(bb.left(), c.top()), rect.max);
                spectrum(b, a, ink, k, v, set);
                scope(b, c, ink, k, v, set);
                vector(b, bb, ink, k, v);
                loudness(b, d, ink, k, v, set);
            }
        }
        if v.analysis.frames() == 0 {
            let t = Text::key("speculum.no_stream", Role::Body, ink.dim).caps(false);
            canvas::put(b, rect.center(), Align2::CENTER_CENTER, &t, k, Some(ink.plate));
        }
    });
}

/// A titled panel: its ground, its name in the corner; returns the inside.
fn panel(b: &mut Brush, r: Rect, ink: &Ink, k: f32, key: &str) -> Rect {
    canvas::ground_b(b, r, ink, None);
    canvas::edge_b(b, r, ink);
    let t = Text::key(key, Role::Micro, ink.dim).caps(true);
    canvas::put(b, pos2(r.left() + 6.0 * k, r.top() + 4.0 * k), Align2::LEFT_TOP, &t, k, None);
    r.shrink2(vec2(6.0 * k, 4.0 * k))
}

fn x_of_hz(plot: Rect, hz: f32) -> f32 {
    let t = cantus_dsp::math::ln(hz.max(LO_HZ) / LO_HZ) / cantus_dsp::math::ln(HI_HZ / LO_HZ);
    plot.left() + plot.width() * t.clamp(0.0, 1.0)
}

fn hz_of_x(plot: Rect, x: f32) -> f32 {
    let t = ((x - plot.left()) / plot.width()).clamp(0.0, 1.0);
    LO_HZ * cantus_dsp::math::powf(HI_HZ / LO_HZ, t)
}

/// The spectrum: averaged (filled) and held (a faint line), on a log axis.
fn spectrum(b: &mut Brush, r: Rect, ink: &Ink, k: f32, v: &View, set: &SpeculumSettings) {
    let inside = panel(b, r, ink, k, "speculum.card.spectrum");
    let plot = Rect::from_min_max(
        pos2(inside.left() + 30.0 * k, inside.top() + 16.0 * k),
        pos2(inside.right() - 4.0 * k, inside.bottom() - 14.0 * k),
    );
    let (top, floor) = (6.0f32, set.floor_db);
    let y_of = |db: f32| plot.bottom() - plot.height() * ((db - floor) / (top - floor)).clamp(0.0, 1.0);
    let mut db = 0.0f32;
    while db >= floor {
        let y = y_of(db);
        b.hline(plot.left(), plot.right(), y, b.px(), if db == 0.0 { ink.grid_strong } else { ink.grid });
        canvas::putf(b, pos2(plot.left() - 4.0 * k, y), Align2::RIGHT_CENTER, Role::Micro, ink.dim, k, None, format_args!("{db:.0}"));
        db -= 24.0;
    }
    for (hz, label) in [
        (50.0f32, None),
        (100.0, Some("100")),
        (200.0, None),
        (500.0, None),
        (1_000.0, Some("1k")),
        (2_000.0, None),
        (5_000.0, None),
        (10_000.0, Some("10k")),
    ] {
        let x = x_of_hz(plot, hz);
        b.vline(x, plot.top(), plot.bottom(), b.px(), ink.grid);
        if let Some(l) = label {
            let t = Text::new(l, Role::Micro, ink.dim).caps(false);
            canvas::put(b, pos2(x, plot.bottom() + 2.0 * k), Align2::CENTER_TOP, &t, k, None);
        }
    }
    let a = &v.analysis;
    if a.analyses() == 0 {
        return;
    }
    let n = ((plot.width() / (2.0 * k)) as usize).clamp(32, 480);
    let curves = set.channel.curves();
    let colours = [ink.hue, mix(ink.hue, ink.text, 0.55)];
    for which in (0..curves).rev() {
        let mut avg = [Pos2::ZERO; 480];
        let mut held = [Pos2::ZERO; 480];
        for j in 0..n {
            let x0 = plot.left() + plot.width() * j as f32 / n as f32;
            let x1 = plot.left() + plot.width() * (j + 1) as f32 / n as f32;
            let (f0, f1) = (hz_of_x(plot, x0), hz_of_x(plot, x1));
            let x = 0.5 * (x0 + x1);
            avg[j] = pos2(x, y_of(a.spectrum_span_db(which, f0, f1, false)));
            held[j] = pos2(x, y_of(a.spectrum_span_db(which, f0, f1, true)));
        }
        if set.hold {
            b.polyline(&held[..n], 1.0 * k, Ink::a(colours[which], 0.55));
        }
        if which == 0 {
            b.area(&avg[..n], plot.bottom(), ink.fill(0.5));
        }
        b.polyline(&avg[..n], 1.6 * k, colours[which]);
    }
    // The legend: what each line is.
    let mut x = plot.right();
    let legend: &[(&str, Color32)] = if set.channel == Channel::Both {
        &[("speculum.legend.right", colours[1]), ("speculum.legend.left", colours[0])]
    } else if set.hold {
        &[("speculum.legend.held", Ink::a(ink.hue, 0.55)), ("speculum.legend.average", ink.hue)]
    } else {
        &[]
    };
    for (key, c) in legend {
        let t = Text::key(key, Role::Micro, ink.dim).caps(true);
        let at = canvas::put(b, pos2(x, inside.top() + 2.0 * k), Align2::RIGHT_TOP, &t, k, None);
        b.hline(at.left() - 16.0 * k, at.left() - 4.0 * k, at.center().y, 2.0 * k, *c);
        x = at.left() - 24.0 * k;
    }
}

/// The spectrogram's picture, made into a texture when a new row came.
fn spectrogram_texture(ctx: &egui::Context, v: &mut View, ink: &Ink, set: &SpeculumSettings) {
    let rows = v.analysis.spectrogram_rows();
    if v.texture.is_some() && rows == v.tex_rows {
        return;
    }
    v.tex_rows = rows;
    let mut pixels = Vec::with_capacity(SPECTRO_ROWS * SPECTRO_BANDS);
    let floor = set.floor_db;
    for band in (0..SPECTRO_BANDS).rev() {
        for col in 0..SPECTRO_ROWS {
            let ago = SPECTRO_ROWS - 1 - col;
            let db = if (ago as u64) < rows {
                v.analysis.spectrogram_row(ago)[band]
            } else {
                -200.0
            };
            let t = ((db - floor) / (0.0 - floor)).clamp(0.0, 1.0);
            let c = if t < 0.6 {
                mix(ink.ground, ink.hue, t / 0.6)
            } else {
                mix(ink.hue, ink.text, (t - 0.6) / 0.4)
            };
            pixels.push(c);
        }
    }
    let img = egui::ColorImage::new([SPECTRO_ROWS, SPECTRO_BANDS], pixels);
    match v.texture.as_mut() {
        Some(t) => t.set(img, egui::TextureOptions::LINEAR),
        None => {
            v.texture = Some(ctx.load_texture("speculum-spectrogram", img, egui::TextureOptions::LINEAR))
        }
    }
}

/// The spectrogram: time across (the newest at the right), frequency up.
fn spectrogram(b: &mut Brush, r: Rect, ink: &Ink, k: f32, v: &View, set: &SpeculumSettings) {
    let inside = panel(b, r, ink, k, "speculum.card.spectrogram");
    let plot = Rect::from_min_max(
        pos2(inside.left() + 30.0 * k, inside.top() + 16.0 * k),
        pos2(inside.right() - 4.0 * k, inside.bottom() - 4.0 * k),
    );
    if let Some(t) = v.texture.as_ref() {
        b.foreign(t.id(), plot);
    }
    let _ = set;
    for (hz, label) in [(100.0f32, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        let t = cantus_dsp::math::ln(hz / LO_HZ) / cantus_dsp::math::ln(HI_HZ / LO_HZ);
        let y = plot.bottom() - plot.height() * t;
        let tx = Text::new(label, Role::Micro, ink.dim).caps(false);
        canvas::put(b, pos2(plot.left() - 4.0 * k, y), Align2::RIGHT_CENTER, &tx, k, None);
    }
}

/// The oscilloscope: the window's lowest and highest sample per column, around the middle.
fn scope(b: &mut Brush, r: Rect, ink: &Ink, k: f32, v: &mut View, set: &SpeculumSettings) {
    let inside = panel(b, r, ink, k, "speculum.card.scope");
    let plot = Rect::from_min_max(
        pos2(inside.left(), inside.top() + 16.0 * k),
        inside.max,
    );
    let mid = plot.center().y;
    b.hline(plot.left(), plot.right(), mid, b.px(), ink.grid_strong);
    for q in [0.25f32, 0.75] {
        b.hline(plot.left(), plot.right(), plot.top() + plot.height() * q, b.px(), ink.grid);
    }
    let cols = ((plot.width() / k) as usize).clamp(16, 1_024);
    let (lo, hi) = (&mut v.lo[..cols], &mut v.hi[..cols]);
    let ok = v.analysis.scope_columns(lo, hi);
    let half = plot.height() * 0.5;
    if ok {
        let w = plot.width() / cols as f32;
        for j in 0..cols {
            let x = plot.left() + j as f32 * w;
            let (a, c) = (mid - hi[j].clamp(-1.0, 1.0) * half, mid - lo[j].clamp(-1.0, 1.0) * half);
            b.rect(
                Rect::from_min_max(pos2(x, a), pos2(x + w.max(k), c.max(a + k))),
                Fill::Solid(ink.hue),
            );
        }
    }
    // What the window is.
    let (word, beat) = match set.sync {
        Sync::Free => ("speculum.scope.free", false),
        Sync::Trigger => ("speculum.scope.trigger", false),
        Sync::Beat => ("speculum.scope.beat", true),
    };
    if beat && !v.analysis.on_beat() {
        let t = Text::key("speculum.waiting", Role::Micro, ink.dim).caps(false);
        canvas::put(b, pos2(inside.right(), inside.top()), Align2::RIGHT_TOP, &t, k, Some(ink.plate));
    } else if beat {
        let i = cantus_core::fx::speculum::SCOPE_BEATS
            .iter()
            .position(|x| *x == set.scope_beats)
            .unwrap_or(2);
        canvas::tag(b, pos2(inside.right(), inside.top()), Align2::RIGHT_TOP, frame::tr(word), ink.text, k, Some(ink.plate), format_args!("{}", frame::tr(super::BEATS_KEYS[i])));
    } else {
        canvas::tag(b, pos2(inside.right(), inside.top()), Align2::RIGHT_TOP, frame::tr(word), ink.text, k, Some(ink.plate), format_args!("{:.0} ms", set.scope_ms));
    }
}

/// The vectorscope (mid up, side across) and the correlation under it.
fn vector(b: &mut Brush, r: Rect, ink: &Ink, k: f32, v: &View) {
    let inside = panel(b, r, ink, k, "speculum.card.vector");
    let bar_h = 22.0 * k;
    let area = Rect::from_min_max(pos2(inside.left(), inside.top() + 16.0 * k), pos2(inside.right(), inside.bottom() - bar_h));
    let side = area.width().min(area.height());
    let sq = Rect::from_center_size(area.center(), vec2(side, side));
    let c = sq.center();
    let rad = side * 0.5;
    b.circle_stroke(c, rad, b.px(), ink.grid);
    b.segment(pos2(c.x, sq.top()), pos2(c.x, sq.bottom()), b.px(), ink.grid_strong, false);
    b.segment(pos2(sq.left(), c.y), pos2(sq.right(), c.y), b.px(), ink.grid, false);
    for (dx, key) in [(-1.0f32, "speculum.legend.left"), (1.0, "speculum.legend.right")] {
        let at = pos2(c.x + dx * rad * 0.70, c.y - rad * 0.70);
        b.segment(c, at, b.px(), ink.grid, false);
        let t = Text::key(key, Role::Micro, ink.dim).caps(true);
        let align = if dx < 0.0 { Align2::RIGHT_BOTTOM } else { Align2::LEFT_BOTTOM };
        canvas::put(b, at, align, &t, k, None);
    }
    let dot = (1.4 * k).max(b.px());
    let col = Ink::a(ink.hue, 0.55);
    for (m, s) in v.analysis.vector_points() {
        let p = pos2(c.x + s.clamp(-1.0, 1.0) * rad, c.y - m.clamp(-1.0, 1.0) * rad);
        b.rect(Rect::from_center_size(p, vec2(dot, dot)), Fill::Solid(col));
    }
    // The correlation, −1 … +1.
    let bar = Rect::from_min_max(pos2(inside.left() + 70.0 * k, inside.bottom() - bar_h + 8.0 * k), pos2(inside.right() - 40.0 * k, inside.bottom() - 6.0 * k));
    let t = Text::key("speculum.m.corr", Role::Micro, ink.dim).caps(true);
    canvas::put(b, pos2(inside.left(), bar.center().y), Align2::LEFT_CENTER, &t, k, None);
    b.rrect(bar, 2.0 * k, Fill::Solid(ink.grid));
    b.vline(bar.center().x, bar.top() - 2.0 * k, bar.bottom() + 2.0 * k, b.px(), ink.grid_strong);
    match v.analysis.correlation() {
        Some(cv) => {
            let x = bar.center().x + cv * bar.width() * 0.5;
            let colour = if cv < 0.0 { ink.danger } else { ink.ok };
            b.rrect(Rect::from_min_max(pos2(x.min(bar.center().x), bar.top()), pos2(x.max(bar.center().x), bar.bottom())), 2.0 * k, Fill::Solid(colour));
            canvas::putf(b, pos2(inside.right(), bar.center().y), Align2::RIGHT_CENTER, Role::Value, ink.text, k, None, format_args!("{cv:+.2}"));
        }
        None => {
            canvas::putf(b, pos2(inside.right(), bar.center().y), Align2::RIGHT_CENTER, Role::Value, ink.dim, k, None, format_args!("\u{2014}"));
        }
    }
}

/// Loudness: momentary, short-term, integrated, range, true peak; the target; 30 s of
/// short-term; peak and RMS per channel.
fn loudness(b: &mut Brush, r: Rect, ink: &Ink, k: f32, v: &View, set: &SpeculumSettings) {
    let inside = panel(b, r, ink, k, "speculum.card.loudness");
    let l = v.analysis.loudness();
    let lv = v.analysis.levels();
    let row = 18.0 * k;
    let dash = "\u{2014}";
    let lufs = |x: f64| -> Option<f64> { (x.is_finite() && x > -70.0).then_some(x) };
    let tp = (l.true_peak_db > -150.0).then_some(f64::from(l.true_peak_db));
    let big = inside.width() > 520.0 * k && inside.height() > 260.0 * k;
    let mut y = inside.top() + 18.0 * k;
    let cols_w;
    if big {
        // A view of its own: momentary, short-term and integrated as three big readouts,
        // the one nearest the target lit; range, true peak and target under them.
        let tile_h = 58.0 * k;
        let gap = 6.0 * k;
        let w = (inside.width() - 2.0 * gap) / 3.0;
        for (i, (key, val)) in [
            ("speculum.m.momentary", lufs(l.momentary)),
            ("speculum.m.short_term", lufs(l.short_term)),
            ("speculum.m.integrated", l.integrated.and_then(lufs)),
        ]
        .into_iter()
        .enumerate()
        {
            let t = Rect::from_min_size(pos2(inside.left() + i as f32 * (w + gap), y), vec2(w, tile_h));
            b.rrect(t, 3.0 * k, Fill::Solid(ink.plate));
            let word = Text::key(key, Role::Micro, ink.dim).caps(true);
            canvas::put(b, pos2(t.left() + 8.0 * k, t.top() + 6.0 * k), Align2::LEFT_TOP, &word, k, None);
            let near = val.is_some_and(|x| (x - f64::from(set.target_lufs)).abs() <= 1.0);
            let colour = if near { ink.ok } else { ink.text };
            match val {
                Some(x) => canvas::putf(b, pos2(t.right() - 44.0 * k, t.bottom() - 8.0 * k), Align2::RIGHT_BOTTOM, Role::Display, colour, k, None, format_args!("{x:.1}")),
                None => canvas::putf(b, pos2(t.right() - 44.0 * k, t.bottom() - 8.0 * k), Align2::RIGHT_BOTTOM, Role::Display, ink.dim, k, None, format_args!("{dash}")),
            };
            let unit = Text::new("LUFS", Role::Micro, ink.dim).caps(false);
            canvas::put(b, pos2(t.right() - 8.0 * k, t.bottom() - 10.0 * k), Align2::RIGHT_BOTTOM, &unit, k, None);
        }
        y += tile_h + 8.0 * k;
        let third = (inside.width() - 2.0 * gap) / 3.0;
        for (i, (key, val, unit)) in [
            ("speculum.m.range", l.range, "LU"),
            ("speculum.m.true_peak", tp, "dBTP"),
            ("speculum.m.target", Some(f64::from(set.target_lufs)), "LUFS"),
        ]
        .into_iter()
        .enumerate()
        {
            let lr = Rect::from_min_size(pos2(inside.left() + i as f32 * (third + gap), y), vec2(third, row));
            match val {
                Some(x) => dynfx::stat(b, lr, frame::tr(key), ink.text, ink, k, format_args!("{x:.1} {unit}")),
                None => dynfx::stat(b, lr, frame::tr(key), ink.dim, ink, k, format_args!("{dash}")),
            }
        }
        y += row + 4.0 * k;
        cols_w = inside.width();
    } else {
        cols_w = inside.width().min(360.0 * k);
        let mut line = |b: &mut Brush, word: &str, val: Option<f64>, unit: &str| {
            let lr = Rect::from_min_size(pos2(inside.left(), y), vec2(cols_w, row));
            match val {
                Some(x) => dynfx::stat(b, lr, word, ink.text, ink, k, format_args!("{x:.1} {unit}")),
                None => dynfx::stat(b, lr, word, ink.dim, ink, k, format_args!("{dash}")),
            }
            y += row;
        };
        line(b, frame::tr("speculum.m.momentary"), lufs(l.momentary), "LUFS");
        line(b, frame::tr("speculum.m.short_term"), lufs(l.short_term), "LUFS");
        line(b, frame::tr("speculum.m.integrated"), l.integrated.and_then(lufs), "LUFS");
        line(b, frame::tr("speculum.m.range"), l.range, "LU");
        line(b, frame::tr("speculum.m.true_peak"), tp, "dBTP");
        line(b, frame::tr("speculum.m.target"), Some(f64::from(set.target_lufs)), "LUFS");
    }
    // Peak and RMS, left and right, as bars from −60 dBFS.
    for (c, key) in [(0usize, "speculum.legend.left"), (1, "speculum.legend.right")] {
        let lr = Rect::from_min_size(pos2(inside.left(), y + 2.0 * k), vec2(cols_w, 10.0 * k));
        let t = Text::key(key, Role::Micro, ink.dim).caps(true);
        canvas::put(b, pos2(lr.left(), lr.center().y), Align2::LEFT_CENTER, &t, k, None);
        let bar = Rect::from_min_max(pos2(lr.left() + 70.0 * k, lr.top()), lr.max);
        let frac = |db: f32| ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        dynfx::bar(b, bar, frac(lv.peak_db[c]), Ink::a(ink.hue, 0.45), ink, k);
        let rms = Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * frac(lv.rms_db[c]), bar.bottom()));
        if rms.width() > 0.0 {
            b.rrect(rms, 2.0 * k, Fill::Solid(ink.hue));
        }
        y += 14.0 * k;
    }
    // Thirty seconds of short-term against the target.
    let hist = Rect::from_min_max(pos2(inside.left(), y + 8.0 * k), inside.max);
    if hist.height() > 30.0 * k {
        let t = Text::key("speculum.m.history", Role::Micro, ink.dim).caps(true);
        canvas::put(b, pos2(hist.left(), hist.top()), Align2::LEFT_TOP, &t, k, None);
        let plot = Rect::from_min_max(pos2(hist.left(), hist.top() + 14.0 * k), hist.max);
        let (lo, hi) = (set.target_lufs - 24.0, set.target_lufs + 6.0);
        let y_of = |x: f32| plot.bottom() - plot.height() * ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
        canvas::ground_b(b, plot, ink, None);
        canvas::dashed(b, pos2(plot.left(), y_of(set.target_lufs)), pos2(plot.right(), y_of(set.target_lufs)), 1.0 * k, ink.ok, 4.0 * k, 3.0 * k);
        let n = v.history().count();
        if n > 1 {
            let mut pts = [Pos2::ZERO; super::HISTORY];
            let w = plot.width() / (super::HISTORY - 1) as f32;
            let skip = super::HISTORY - n;
            for (i, x) in v.history().enumerate() {
                pts[i] = pos2(plot.left() + (skip + i) as f32 * w, y_of(x));
            }
            // Nothing before the first reading above the gate: a start of silence is not drawn.
            let first = v.history().position(|x| x > -70.0).unwrap_or(n);
            if n - first > 1 {
                b.polyline(&pts[first..n], 1.6 * k, ink.hue);
            }
        }
        canvas::edge_b(b, plot, ink);
    }
}

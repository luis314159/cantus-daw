//! Cedo's screen, its lower half (design sheet v1):
//!
//! - **The last four seconds**, scrolling with the song: the reduction hanging from the
//!   top as a filled curtain (its decibels on the right), the hits along the foot — the
//!   cycle's starts in Tempo, the notes in MIDI, the key's hits in Sidechain, where the
//!   key's level runs over it against the threshold's dashed line.
//! - **The band strip** in Lows or Highs: the split's response (`Cedo::response_db`) at
//!   the amount now, filled, and at the full depth, dashed; the crossover marked; the split
//!   (minimum or linear phase) and the latency it costs written beside.

use super::super::canvas::{self, Ink};
use super::screen::{band as band_of, Feed};
use super::view::{Hit, FLOOR_DB, HISTORY, RATE};
use crate::kit::color::with_alpha;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Pos2, Rect};

/// The history in `r`.
pub fn history(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, feed: &Feed<'_>) {
    let k = s.k;
    let spec = feed.spec;
    let trig = super::screen::trigger(spec);
    let key = ink.look.trace2;
    // The title over it, and the hits' word.
    canvas::put(
        b,
        pos2(r.left(), r.top() - 12.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("cedo.d.history", Role::Micro, ink.text).caps(true),
        k,
        None,
    );
    let hits = match trig {
        0 => "cedo.d.hits_key",
        1 => "cedo.d.hits_cycle",
        _ => "cedo.d.hits_note",
    };
    let hw = canvas::put(
        b,
        pos2(r.right(), r.top() - 12.0 * k),
        Align2::RIGHT_CENTER,
        &Text::key(hits, Role::Micro, ink.dim),
        k,
        None,
    );
    b.rect(
        Rect::from_center_size(
            pos2(hw.left() - 8.0 * k, hw.center().y),
            vec2(2.0 * k, 9.0 * k),
        ),
        Fill::Solid(key),
    );
    let mut right_x = hw.left() - 18.0 * k;
    if trig == 0 {
        let w = canvas::put(
            b,
            pos2(right_x, r.top() - 12.0 * k),
            Align2::RIGHT_CENTER,
            &Text::key("cedo.d.keylevel", Role::Micro, ink.dim),
            k,
            None,
        );
        canvas::dashed(
            b,
            pos2(w.left() - 16.0 * k, w.center().y),
            pos2(w.left() - 4.0 * k, w.center().y),
            1.2 * k,
            key,
            3.0 * k,
            2.0 * k,
        );
        right_x = w.left() - 22.0 * k;
    }
    let _ = right_x;
    // The frame and the seconds.
    b.rect(r, Fill::Solid(with_alpha(ink.ground, 0.4)));
    let span = HISTORY as f32 / RATE;
    for sec in 1..span as i32 {
        let x = r.right() - r.width() * sec as f32 / span;
        b.vline(x, r.top(), r.bottom(), 0.5 * k, with_alpha(ink.grid, 0.8));
        canvas::putf(
            b,
            pos2(x, r.bottom() + 4.0 * k),
            Align2::CENTER_TOP,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("\u{2212}{sec} s"),
        );
    }
    b.hline(
        r.left(),
        r.right(),
        r.top(),
        0.8 * k,
        with_alpha(ink.grid_strong, 0.9),
    );
    b.hline(
        r.left(),
        r.right(),
        r.bottom(),
        0.8 * k,
        with_alpha(ink.grid_strong, 0.9),
    );
    // The reduction's scale on the right: the depth (or 24 dB for silence) at the foot.
    let depth = spec.get("depth");
    let full = if depth >= cantus_dsp::cedo::DEPTH_INF {
        48.0
    } else {
        depth.max(3.0)
    };
    let red = |gr: f32| r.top() + r.height() * (gr / full).clamp(0.0, 1.0);
    let step = super::screen::nice_step(full / 3.0);
    let mut g = step;
    while g < full - 0.3 * step && r.bottom() - red(g) > 14.0 * k {
        let y = red(g);
        b.hline(r.left(), r.right(), y, 0.4 * k, with_alpha(ink.grid, 0.7));
        canvas::putf(
            b,
            pos2(r.right() + 5.0 * k, y),
            Align2::LEFT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("\u{2212}{g:.0}"),
        );
        g += step;
    }
    canvas::putf(
        b,
        pos2(r.right() + 5.0 * k, r.bottom()),
        Align2::LEFT_CENTER,
        Role::Micro,
        ink.dim,
        k,
        None,
        format_args!("\u{2212}{full:.0}"),
    );
    // The key's level against the threshold (Sidechain), on a scale of −60 … 0 dBFS.
    let lvl = |db: f32| r.bottom() - r.height() * ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    if trig == 0 {
        let thr = spec.get("thr");
        let y = lvl(thr);
        canvas::dashed(
            b,
            pos2(r.left(), y),
            pos2(r.right(), y),
            1.0 * k,
            with_alpha(key, 0.8),
            6.0 * k,
            4.0 * k,
        );
        canvas::tag(
            b,
            pos2(r.left() + 6.0 * k, y - 3.0 * k),
            Align2::LEFT_BOTTOM,
            super::super::frame::tr("fx.cedo.thr"),
            key,
            k,
            Some(ink.plate),
            format_args!("{thr:.1} dB"),
        );
    }
    let view = feed.view;
    if view.is_empty() {
        // Nothing heard yet: what it waits for.
        let wait = match trig {
            0 => "cedo.d.wait_key",
            1 => "cedo.d.wait_song",
            _ => "cedo.d.wait_notes",
        };
        canvas::put(
            b,
            r.center(),
            Align2::CENTER_CENTER,
            &Text::key(wait, Role::Micro, ink.dim).caps(true),
            k,
            None,
        );
        return;
    }
    let n = view.len();
    let step_x = r.width() / (HISTORY - 1) as f32;
    let x0 = r.right() - step_x * (n - 1) as f32;
    let mut grs = [Pos2::ZERO; HISTORY];
    let mut keys = [Pos2::ZERO; HISTORY];
    let mut was_key = false;
    for (i, p) in view.points().enumerate() {
        let x = x0 + step_x * i as f32;
        grs[i] = pos2(x, red(p.gr));
        keys[i] = pos2(x, lvl(p.key_db.max(FLOOR_DB)));
        // A key's hit marked where it begins; a cycle's or a note's start where it is.
        let tick = match (trig, p.hit) {
            (0, Hit::Key) => !was_key,
            (1 | 2, Hit::Cycle) => true,
            _ => false,
        };
        was_key = p.hit == Hit::Key;
        if tick {
            b.rect(
                Rect::from_min_max(
                    pos2(x - 1.0 * k, r.bottom() - 9.0 * k),
                    pos2(x + 1.0 * k, r.bottom()),
                ),
                Fill::Solid(key),
            );
        }
    }
    b.area(
        &grs[..n],
        r.top(),
        with_alpha(ink.hue, if ink.dark { 0.34 } else { 0.30 }),
    );
    canvas::trace(b, &grs[..n], 1.5 * k, ink.hue, ink);
    if trig == 0 {
        b.dashed(&keys[..n], 1.0 * k, with_alpha(key, 0.9), 3.0 * k, 2.0 * k);
    }
    // The playhead's edge: now.
    b.vline(
        r.right(),
        r.top(),
        r.bottom(),
        1.2 * k,
        with_alpha(ink.text, 0.7),
    );
}

/// The band strip in `r`: the split's response now and at full depth, the crossover.
pub fn band(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, feed: &Feed<'_>) {
    let k = s.k;
    let spec = feed.spec;
    let which = band_of(spec);
    let lin = spec.get("xover") >= 0.5;
    let xhz = spec.get("xhz");
    let depth = spec.get("depth");
    let floor = if depth >= cantus_dsp::cedo::DEPTH_INF {
        36.0
    } else {
        depth.clamp(3.0, 36.0)
    };
    let (lo, hi) = (20.0f32, 20_000.0f32);
    let lx = |hz: f32| {
        r.left()
            + r.width()
                * (cantus_dsp::math::log10(hz / lo) / cantus_dsp::math::log10(hi / lo))
                    .clamp(0.0, 1.0)
    };
    let yd = |db: f32| r.top() + r.height() * (-db / floor).clamp(0.0, 1.0);
    // The title, the split and its latency.
    let band_word = super::super::frame::tr(if which == 1 {
        "fx.cedo.band.1"
    } else {
        "fx.cedo.band.2"
    });
    let split_word = super::super::frame::tr(if lin {
        "fx.cedo.xover.1"
    } else {
        "fx.cedo.xover.0"
    });
    let t = canvas::put(
        b,
        pos2(r.left(), r.top() - 12.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("cedo.d.band", Role::Micro, ink.text).caps(true),
        k,
        None,
    );
    let t2 = canvas::put(
        b,
        pos2(t.right() + 10.0 * k, t.center().y),
        Align2::LEFT_CENTER,
        &Text::new(band_word, Role::Micro, ink.hue).caps(true),
        k,
        None,
    );
    let t3 = canvas::put(
        b,
        pos2(t2.right() + 10.0 * k, t.center().y),
        Align2::LEFT_CENTER,
        &Text::new(split_word, Role::Micro, ink.dim).caps(false),
        k,
        None,
    );
    let lat_ms = if lin {
        let sr = if feed.now.seen {
            feed.now.sample_rate
        } else {
            48_000.0
        };
        cantus_dsp::cedo::band::linear_latency(sr) as f32 * 1_000.0 / sr
    } else {
        0.0
    };
    if lin {
        canvas::tag(
            b,
            pos2(t3.right() + 10.0 * k, t.center().y),
            Align2::LEFT_CENTER,
            super::super::frame::tr("cedo.d.latency"),
            ink.dim,
            k,
            None,
            format_args!("{lat_ms:.1} ms"),
        );
    } else {
        canvas::put(
            b,
            pos2(t3.right() + 10.0 * k, t.center().y),
            Align2::LEFT_CENTER,
            &Text::key("cedo.d.no_latency", Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    // The grid.
    b.rect(r, Fill::Solid(with_alpha(ink.ground, 0.4)));
    for hz in [
        50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
    ] {
        let x = lx(hz);
        b.vline(x, r.top(), r.bottom(), 0.5 * k, with_alpha(ink.grid, 0.8));
        if hz >= 1_000.0 {
            canvas::putf(
                b,
                pos2(x, r.bottom() + 3.0 * k),
                Align2::CENTER_TOP,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{:.0} kHz", hz / 1_000.0),
            );
        } else {
            canvas::putf(
                b,
                pos2(x, r.bottom() + 3.0 * k),
                Align2::CENTER_TOP,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{hz:.0} Hz"),
            );
        }
    }
    b.hline(
        r.left(),
        r.right(),
        r.top(),
        0.8 * k,
        with_alpha(ink.grid_strong, 0.9),
    );
    b.hline(
        r.left(),
        r.right(),
        r.bottom(),
        0.8 * k,
        with_alpha(ink.grid_strong, 0.9),
    );
    canvas::putf(
        b,
        pos2(r.right() + 5.0 * k, r.top()),
        Align2::LEFT_CENTER,
        Role::Micro,
        ink.dim,
        k,
        None,
        format_args!("0"),
    );
    canvas::putf(
        b,
        pos2(r.right() + 5.0 * k, r.bottom()),
        Align2::LEFT_CENTER,
        Role::Micro,
        ink.dim,
        k,
        None,
        format_args!("\u{2212}{floor:.0}"),
    );
    let Some(dsp) = feed.split else {
        return;
    };
    const M: usize = 121;
    let mut full = [Pos2::ZERO; M];
    let mut now = [Pos2::ZERO; M];
    let a_now = if feed.now.seen { feed.now.amount } else { 0.0 };
    for i in 0..M {
        let t = i as f32 / (M - 1) as f32;
        let hz = lo * cantus_dsp::math::powf(hi / lo, t);
        let x = r.left() + r.width() * t;
        full[i] = pos2(x, yd(dsp.response_db(hz, 1.0)));
        now[i] = pos2(x, yd(dsp.response_db(hz, a_now)));
    }
    b.area(
        &full,
        r.top(),
        with_alpha(ink.hue, if ink.dark { 0.08 } else { 0.07 }),
    );
    b.dashed(&full, 1.0 * k, with_alpha(ink.hue, 0.7), 4.0 * k, 3.0 * k);
    // The legend on the right of the title: the full depth dashed, now solid.
    let ly = r.top() - 12.0 * k;
    let mut lgx = r.right();
    for (key, solid) in [("cedo.d.band_now", true), ("cedo.d.band_full", false)] {
        let w = canvas::put(
            b,
            pos2(lgx, ly),
            Align2::RIGHT_CENTER,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            None,
        );
        let (a, z) = (pos2(w.left() - 18.0 * k, ly), pos2(w.left() - 5.0 * k, ly));
        if solid {
            b.segment(a, z, 1.6 * k, ink.hue, false);
        } else {
            canvas::dashed(b, a, z, 1.0 * k, ink.hue, 3.0 * k, 2.0 * k);
        }
        lgx = a.x - 14.0 * k;
    }
    b.area(
        &now,
        r.top(),
        with_alpha(ink.hue, if ink.dark { 0.36 } else { 0.30 }),
    );
    canvas::trace(b, &now, 1.6 * k, ink.hue, ink);
    // The crossover.
    let x = lx(xhz);
    let key = ink.text;
    b.vline(x, r.top(), r.bottom(), 1.0 * k, with_alpha(key, 0.85));
    let left = x > r.center().x;
    canvas::tag(
        b,
        pos2(
            if left { x - 6.0 * k } else { x + 6.0 * k },
            r.bottom() - 5.0 * k,
        ),
        if left {
            Align2::RIGHT_BOTTOM
        } else {
            Align2::LEFT_BOTTOM
        },
        super::super::frame::tr("fx.cedo.xhz"),
        key,
        k,
        Some(ink.plate),
        format_args!("{xhz:.0} Hz"),
    );
}

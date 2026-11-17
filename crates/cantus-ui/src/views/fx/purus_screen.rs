//! Purus's screen (design sheet v1): a spectrogram on dark glass in every theme — the last
//! six seconds left to right, 20 Hz to 20 kHz bottom to top on a logarithmic rail. What is
//! heard glows in a warm heat; **what was taken out** of each cell is laid over it in the
//! residue's cool colour, so the hum's harmonics, the clicks' flashes and the room's tail
//! show as what fell out of the picture. The hum's harmonics are a comb of fine needles on
//! the rail at their true frequencies (each its depth; held ones bracketed in the gentle
//! mode), carried across the picture as hairlines. Under it the clicks' lane, a tick per
//! repair; beside it the room's tail before and after.

use super::art::{c, needle, residue};
use super::picture::{frac, Picture, COLS, ROWS};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::restoration as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// Where the screen's parts are.
#[derive(Copy, Clone, Debug)]
pub struct Plot {
    pub glass: Rect,
    pub head: Rect,
    pub rail: Rect,
    pub spec: Rect,
    pub clicks: Rect,
    pub tail: Rect,
}

impl Plot {
    pub fn lay(glass: Rect, k: f32) -> Plot {
        let inner = glass.shrink2(vec2(12.0 * k, 8.0 * k));
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), 22.0 * k));
        let tail_w = (inner.width() * 0.28).clamp(240.0 * k, 270.0 * k);
        let body = Rect::from_min_max(pos2(inner.left(), head.bottom() + 6.0 * k), inner.max);
        let tail = Rect::from_min_max(pos2(body.right() - tail_w, body.top()), body.max);
        let left = Rect::from_min_max(body.min, pos2(tail.left() - 12.0 * k, body.bottom()));
        let rail = Rect::from_min_max(
            left.min,
            pos2(left.left() + 50.0 * k, left.bottom() - 52.0 * k),
        );
        let spec = Rect::from_min_max(
            pos2(rail.right(), rail.top()),
            pos2(left.right(), rail.bottom()),
        );
        let clicks = Rect::from_min_max(
            pos2(spec.left(), spec.bottom() + 20.0 * k),
            pos2(spec.right(), left.bottom() - 4.0 * k),
        );
        Plot {
            glass,
            head,
            rail,
            spec,
            clicks,
            tail,
        }
    }

    /// The y of a frequency on the spectrogram.
    pub fn y_of(&self, hz: f32) -> f32 {
        self.spec.bottom() - frac(hz) * self.spec.height()
    }

    /// The frequency at a y on the spectrogram.
    pub fn hz_at(&self, y: f32) -> f32 {
        use super::picture::{HI_HZ, LO_HZ};
        let f = ((self.spec.bottom() - y) / self.spec.height().max(1.0)).clamp(0.0, 1.0);
        LO_HZ * (HI_HZ / LO_HZ).powf(f)
    }
}

/// The screen's words (made by the editor when what they say changes).
pub struct Words<'a> {
    pub hum: &'a str,
    pub decay: &'a str,
    pub clicks: &'a str,
    pub fork: &'a str,
}

/// The settings the screen reads.
pub struct Shown {
    pub hum_on: bool,
    pub harmonics: usize,
    pub gentle: bool,
    pub click_on: bool,
    pub verb_on: bool,
    pub keep_ms: f32,
    /// The reduction set (`vamount`), dB: the handle on the tail's right edge.
    pub amount_db: f32,
    pub delta: bool,
}

/// Where the room's tail was drawn (the editor's drags read it next frame): its plot, the
/// seconds it spans, and the dB its height spans.
#[derive(Copy, Clone, Debug)]
pub struct TailGeom {
    pub plot: Rect,
    pub span_s: f32,
}

impl TailGeom {
    /// The x of a time after the sound stops, s.
    pub fn x_of(&self, t: f32) -> f32 {
        self.plot.left() + self.plot.width() * (t / self.span_s)
    }

    /// The time at an x, s.
    pub fn t_at(&self, x: f32) -> f32 {
        (x - self.plot.left()) / self.plot.width().max(1.0) * self.span_s
    }

    /// The y of a level, dB (0 at the top, −60 at the foot).
    pub fn y_of(&self, db: f32) -> f32 {
        self.plot.top() + self.plot.height() * (-db / 60.0).clamp(0.0, 1.0)
    }

    /// The level at a y, dB below the top.
    pub fn db_below_at(&self, y: f32) -> f32 {
        (y - self.plot.top()) / self.plot.height().max(1.0) * 60.0
    }
}

/// The heat's colour for a cell (0 … 255).
fn heat(v: u8) -> Color32 {
    let x = (f32::from(v) / 255.0).powf(1.7) * (pal::HEAT.len() - 1) as f32;
    let i = (x.floor() as usize).min(pal::HEAT.len() - 2);
    mix(c(pal::HEAT[i]), c(pal::HEAT[i + 1]), x - i as f32)
}

fn ink() -> Color32 {
    c(pal::SCREEN_INK)
}

fn dim() -> Color32 {
    c(pal::SCREEN_DIM)
}

fn put(b: &mut Brush, k: f32, at: Pos2, align: Align2, t: &Text<'_>) -> Rect {
    super::super::canvas::put(b, at, align, t, k, None)
}

/// Paint the screen; returns where the room's tail was drawn.
pub fn paint(
    b: &mut Brush,
    s: &Style,
    p: &Plot,
    pic: &Picture,
    sh: &Shown,
    w: &Words<'_>,
) -> TailGeom {
    let k = s.k;
    let res = residue(s);
    let ndl = needle(s);
    head(b, s, p, sh, w, res, ndl);
    // The spectrogram's ground and graticule.
    b.rect(p.spec, Fill::Solid(c(pal::HEAT[0])));
    let grat = with_alpha(c(pal::GRATICULE), 0.55);
    for hz in [
        50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
    ] {
        let y = b.snap_mid(p.y_of(hz));
        b.hline(p.spec.left(), p.spec.right(), y, 0.5 * k, grat);
    }
    for i in 1..6 {
        let x = b.snap_mid(p.spec.left() + p.spec.width() * i as f32 / 6.0);
        b.vline(x, p.spec.top(), p.spec.bottom(), 0.5 * k, grat);
    }
    // The cells: what is heard, then what was taken out over it.
    let cw = p.spec.width() / COLS as f32;
    let rh = p.spec.height() / ROWS as f32;
    let first = COLS.saturating_sub(pic.cols.len());
    for (i, col) in pic.cols.iter().enumerate() {
        let x0 = p.spec.left() + (first + i) as f32 * cw;
        for r in 0..ROWS {
            let y1 = p.spec.bottom() - r as f32 * rh;
            let cell = Rect::from_min_max(pos2(x0, y1 - rh), pos2(x0 + cw + 0.3, y1 + 0.3));
            let h = col.heard[r];
            let gone = col.removed[r];
            if sh.delta {
                // Listening to what is removed: the residue is the picture.
                if h > 10 {
                    let a = (f32::from(h) / 255.0).powf(1.4);
                    b.rect(cell, Fill::Solid(with_alpha(res, a)));
                }
                continue;
            }
            if h > 10 {
                b.rect(cell, Fill::Solid(heat(h)));
            }
            if gone > 10 {
                let a = (f32::from(gone) / 255.0 * 1.2).min(0.72);
                b.rect(cell, Fill::Solid(with_alpha(res, a)));
            }
        }
    }
    // The hum's harmonics: hairlines across the picture and needles on the rail.
    if sh.hum_on {
        for n in 0..sh.harmonics.min(pic.depth.len()) {
            let hz = pic.hum_hz * (n + 1) as f32;
            if hz > 20_000.0 {
                break;
            }
            let y = p.y_of(hz);
            b.hline(
                p.spec.left(),
                p.spec.right(),
                y,
                0.6 * k,
                with_alpha(ndl, 0.22),
            );
            let depth = (pic.depth[n] / 80.0).clamp(0.0, 1.0);
            let x1 = p.rail.right() - 4.0 * k;
            let len = 8.0 * k + depth * 22.0 * k;
            b.segment(pos2(x1 - len, y), pos2(x1, y), 1.1 * k, ndl, false);
            let lvl = ((pic.level[n] + 90.0) / 70.0).clamp(0.0, 1.0);
            b.circle(pos2(x1 - len, y), 1.4 * k + 1.6 * k * lvl, Fill::Solid(ndl));
            if sh.gentle && pic.held & (1 << n) != 0 {
                let br = Rect::from_center_size(pos2(x1 - len - 6.0 * k, y), vec2(4.0, 8.0) * k);
                b.polyline(
                    &[
                        br.right_top(),
                        br.left_top(),
                        br.left_bottom(),
                        br.right_bottom(),
                    ],
                    0.8 * k,
                    ndl,
                );
            }
        }
    }
    // The frequency rail's words.
    for (hz, word) in [
        (50.0f32, "50 Hz"),
        (100.0, "100 Hz"),
        (200.0, "200 Hz"),
        (500.0, "500 Hz"),
        (1_000.0, "1 kHz"),
        (2_000.0, "2 kHz"),
        (5_000.0, "5 kHz"),
        (10_000.0, "10 kHz"),
    ] {
        let t = Text::new(word, Role::Micro, dim()).caps(false);
        put(
            b,
            k,
            pos2(p.rail.left(), p.y_of(hz)),
            Align2::LEFT_CENTER,
            &t,
        );
    }
    // The time under the picture.
    let ty = p.spec.bottom() + 9.0 * k;
    put(
        b,
        k,
        pos2(p.spec.left(), ty),
        Align2::LEFT_CENTER,
        &Text::key("purus.d.ago", Role::Micro, dim()).caps(false),
    );
    put(
        b,
        k,
        pos2(p.spec.right(), ty),
        Align2::RIGHT_CENTER,
        &Text::key("purus.d.now", Role::Micro, dim()).caps(false),
    );
    clicks(b, s, p, pic, sh, w, res);
    let geom = tail(b, s, p, pic, sh, w, res);
    if pic.waiting() {
        let t = Text::key("purus.d.waiting", Role::Label, ink()).caps(false);
        put(b, k, p.spec.center(), Align2::CENTER_CENTER, &t);
    }
    super::art::sheen(b, s, p.glass);
    geom
}

/// The head: the legend on the left, every colour named; the hum's reading on the right.
fn head(b: &mut Brush, s: &Style, p: &Plot, sh: &Shown, w: &Words<'_>, res: Color32, ndl: Color32) {
    let k = s.k;
    let y = p.head.center().y;
    let mut x = p.head.left();
    let heard = if sh.delta {
        "purus.d.only_removed"
    } else {
        "purus.d.heard"
    };
    let items: [(&str, Color32); 3] = [
        (heard, if sh.delta { res } else { heat(200) }),
        ("purus.d.removed", res),
        ("purus.d.harmonics", ndl),
    ];
    for (i, (key, col)) in items.iter().enumerate() {
        if sh.delta && i == 1 {
            continue;
        }
        if i == 2 {
            b.segment(pos2(x, y), pos2(x + 12.0 * k, y), 1.2 * k, *col, false);
            b.circle(pos2(x, y), 2.2 * k, Fill::Solid(*col));
        } else {
            b.rrect(
                Rect::from_center_size(pos2(x + 6.0 * k, y), vec2(12.0, 7.0) * k),
                1.5 * k,
                Fill::Solid(*col),
            );
        }
        let r = put(
            b,
            k,
            pos2(x + 17.0 * k, y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, ink()),
        );
        x = r.right() + 16.0 * k;
    }
    let t = Text::new(w.hum, Role::Label, if sh.hum_on { ndl } else { dim() }).caps(false);
    put(b, k, pos2(p.head.right(), y), Align2::RIGHT_CENTER, &t);
}

/// The clicks' lane: a tick for each column that had a repair, taller for more, and the
/// count in all at its right.
fn clicks(
    b: &mut Brush,
    s: &Style,
    p: &Plot,
    pic: &Picture,
    sh: &Shown,
    w: &Words<'_>,
    res: Color32,
) {
    let k = s.k;
    let r = p.clicks;
    b.rrect(r, 3.0 * k, Fill::Solid(with_alpha(c(pal::HEAT[0]), 0.9)));
    b.rrect_stroke(r, 3.0 * k, 0.6 * k, with_alpha(c(pal::GRATICULE), 0.7));
    let base = r.bottom() - 4.0 * k;
    b.hline(
        r.left() + 4.0 * k,
        r.right() - 4.0 * k,
        base,
        0.5 * k,
        with_alpha(c(pal::GRATICULE), 0.8),
    );
    let label = Text::key(
        "purus.d.clicks",
        Role::Micro,
        if sh.click_on { ink() } else { dim() },
    );
    let lr = put(
        b,
        k,
        pos2(r.left() + 6.0 * k, r.top() + 7.0 * k),
        Align2::LEFT_CENTER,
        &label,
    );
    let count = Text::new(w.clicks, Role::Micro, if sh.click_on { res } else { dim() }).caps(false);
    put(
        b,
        k,
        pos2(r.right() - 6.0 * k, r.top() + 7.0 * k),
        Align2::RIGHT_CENTER,
        &count,
    );
    let cw = p.spec.width() / COLS as f32;
    let first = COLS.saturating_sub(pic.cols.len());
    let top = lr.bottom() + 2.0 * k;
    for (i, col) in pic.cols.iter().enumerate() {
        if col.clicks == 0 || !sh.click_on {
            continue;
        }
        let x = p.spec.left() + (first + i) as f32 * cw + cw * 0.5;
        let h = ((col.clicks as f32).min(3.0) / 3.0) * (base - top);
        b.segment(
            pos2(x, base),
            pos2(x, base - h.max(4.0 * k)),
            1.2 * k,
            res,
            false,
        );
        b.dot(
            pos2(x, base - h.max(4.0 * k)),
            1.4 * k,
            mix(res, c(pal::LAMP_HOT), 0.5),
        );
    }
}

/// The room's tail, before and after: level over time (0 to −60 dB), the room's decay as
/// heard before (a bright line, so it reads against the glass), and after — the early
/// reflections kept, then the tail taken down by the reduction now — the gap between them
/// in the residue's colour; on the right edge the reduction set, a handle a drag moves.
fn tail(
    b: &mut Brush,
    s: &Style,
    p: &Plot,
    pic: &Picture,
    sh: &Shown,
    w: &Words<'_>,
    res: Color32,
) -> TailGeom {
    let k = s.k;
    let r = p.tail;
    b.rrect(r, 4.0 * k, Fill::Solid(with_alpha(c(pal::HEAT[0]), 0.8)));
    b.rrect_stroke(r, 4.0 * k, 0.6 * k, with_alpha(c(pal::GRATICULE), 0.8));
    let title = Text::key(
        "purus.d.tail",
        Role::Micro,
        if sh.verb_on { ink() } else { dim() },
    );
    let tr = put(
        b,
        k,
        pos2(r.left() + 8.0 * k, r.top() + 9.0 * k),
        Align2::LEFT_CENTER,
        &title,
    );
    let dt = Text::new(w.decay, Role::Micro, if sh.verb_on { res } else { dim() }).caps(false);
    let dr = put(
        b,
        k,
        pos2(r.left() + 8.0 * k, tr.bottom() + 8.0 * k),
        Align2::LEFT_CENTER,
        &dt,
    );
    let plot = Rect::from_min_max(
        pos2(r.left() + 34.0 * k, dr.bottom() + 12.0 * k),
        pos2(r.right() - 22.0 * k, r.bottom() - 46.0 * k),
    );
    let grat = with_alpha(c(pal::GRATICULE), 0.6);
    for (i, db) in ["0", "-20", "-40", "-60"].iter().enumerate() {
        let y = plot.top() + plot.height() * i as f32 / 3.0;
        b.hline(plot.left(), plot.right(), b.snap_mid(y), 0.5 * k, grat);
        let t = Text::new(db, Role::Micro, dim()).caps(false);
        put(
            b,
            k,
            pos2(plot.left() - 4.0 * k, y),
            Align2::RIGHT_CENTER,
            &t,
        );
    }
    let t60 = pic.decay.max(0.1);
    let span = (t60 * 1.25).max(0.4);
    let geom = TailGeom { plot, span_s: span };
    let n = 48;
    let before: Vec<Pos2> = (0..=n)
        .map(|i| {
            let t = span * i as f32 / n as f32;
            pos2(geom.x_of(t), geom.y_of(-60.0 * t / t60))
        })
        .collect();
    let cut = if sh.verb_on {
        pic.verb_mean().clamp(0.0, 30.0)
    } else {
        0.0
    };
    let keep = sh.keep_ms / 1000.0;
    let after: Vec<Pos2> = (0..=n)
        .map(|i| {
            let t = span * i as f32 / n as f32;
            let into = ((t - keep) / (0.25 * t60).max(0.05)).clamp(0.0, 1.0);
            pos2(geom.x_of(t), geom.y_of(-60.0 * t / t60 - cut * into))
        })
        .collect();
    b.between(&before, &after, with_alpha(res, 0.34));
    // The room as it was: the screen's ink, solid and as heavy as the line after, so the
    // two read apart by colour (cream against the warm heat) and both stand off the glass.
    let was = before_ink();
    b.polyline(&before, 1.5 * k, was);
    b.polyline(&after, 1.5 * k, heat(220));
    // The early reflections kept: a marker at their end (a drag moves it).
    if sh.verb_on && keep < span {
        let x = geom.x_of(keep);
        b.vline(x, plot.top(), plot.bottom(), 0.8 * k, with_alpha(res, 0.8));
        b.circle(pos2(x, plot.bottom()), 2.6 * k, Fill::Solid(res));
    }
    // The reduction set: a handle on the right edge, 0 at the top, 30 dB half way down.
    if sh.verb_on {
        let y = geom.y_of(-sh.amount_db.clamp(0.0, 30.0));
        let x = plot.right() + 3.0 * k;
        b.polygon(
            &[
                pos2(x, y),
                pos2(x + 7.0 * k, y - 4.5 * k),
                pos2(x + 7.0 * k, y + 4.5 * k),
            ],
            res,
        );
        b.hline(
            plot.right() - 10.0 * k,
            plot.right(),
            y,
            1.0 * k,
            with_alpha(res, 0.9),
        );
    }
    // The legend under it, each line named.
    let ly = plot.bottom() + 16.0 * k;
    b.segment(
        pos2(plot.left(), ly),
        pos2(plot.left() + 14.0 * k, ly),
        1.5 * k,
        was,
        false,
    );
    let br = put(
        b,
        k,
        pos2(plot.left() + 18.0 * k, ly),
        Align2::LEFT_CENTER,
        &Text::key("purus.d.before", Role::Micro, ink()),
    );
    let ax = br.right() + 12.0 * k;
    b.segment(
        pos2(ax, ly),
        pos2(ax + 14.0 * k, ly),
        1.4 * k,
        heat(220),
        false,
    );
    put(
        b,
        k,
        pos2(ax + 18.0 * k, ly),
        Align2::LEFT_CENTER,
        &Text::key("purus.d.after", Role::Micro, ink()),
    );
    let ky = ly + 16.0 * k;
    b.vline(
        plot.left() + 7.0 * k,
        ky - 5.0 * k,
        ky + 5.0 * k,
        0.8 * k,
        with_alpha(res, 0.8),
    );
    put(
        b,
        k,
        pos2(plot.left() + 18.0 * k, ky),
        Align2::LEFT_CENTER,
        &Text::key("purus.d.kept", Role::Micro, ink()),
    );
    let _ = w.fork;
    geom
}

/// The tail before the repair: the screen's cream ink, a touch brighter than its words.
fn before_ink() -> Color32 {
    mix(ink(), c(pal::GLINT), 0.3)
}

//! Cedo's screen (design sheet v1): a phosphor glass in its riveted frame, dark in every
//! theme.
//!
//! - **The shape** (Tempo and MIDI): one cycle of the division — in milliseconds at the
//!   song's tempo — the gain falling from 0 dB at the top to the depth at the foot, the
//!   drawn shape a line with its breakpoints (rings) and each segment's bend handle
//!   (diamonds), what it takes away lightly filled; over it, **what the engine ducks now**
//!   swept in by the playhead (the shape's phase): the current cycle up to the playhead, the
//!   cycle before after it — so the drawn and the heard read as one.
//! - **The envelope** (Sidechain): the key's hit, the attack along its S, the hold, the
//!   release in its shape, each with its handle and its time; the amount now on the edge.
//! - **The last four seconds** under it ([`super::trace`]), and in Lows or Highs **the
//!   band strip** with the split's response.
//!
//! Drawing the shape is this sheet's; dragging its points and bends
//! (`Command::SetCedoShape`) is the full editor's.

use super::super::canvas::{self, Ink};
use super::trace;
use super::view::{Reading, View};
use crate::kit::color::with_alpha;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_dsp::cedo::Cedo;
use cantus_dsp::lapsus::Curve;
use cantus_engine::fx::fluctus::shape_at;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// Points along a drawn curve.
const N: usize = 241;

/// The shape's amount at `u` (0 … 1 of the cycle).
pub fn amount_at(curve: &Curve, u: f32) -> f32 {
    curve.at(u.rem_euclid(1.0) as f64, shape_at).clamp(0.0, 1.0) as f32
}

/// The gain taken away at full amount, as a share (1 for silence).
pub fn span(depth: f32) -> f32 {
    if depth >= cantus_dsp::cedo::DEPTH_INF {
        1.0
    } else {
        1.0 - cantus_dsp::math::db_to_gain(-depth.max(0.0))
    }
}

/// The amount that ducks by `db` at `depth`.
pub fn amount_of_db(db: f32, depth: f32) -> f32 {
    let sp = span(depth);
    if sp <= 1e-6 {
        return 0.0;
    }
    ((1.0 - cantus_dsp::math::db_to_gain(-db)) / sp).clamp(0.0, 1.0)
}

/// What the screen draws from.
pub struct Feed<'a> {
    pub spec: &'a EffectSpec,
    pub curve: &'a Curve,
    pub now: &'a Reading,
    pub view: &'a View,
    /// The split's unit, for its response (Lows or Highs).
    pub split: Option<&'a Cedo>,
}

/// The trigger in force: 0 Sidechain, 1 Tempo, 2 MIDI.
pub fn trigger(spec: &EffectSpec) -> usize {
    spec.get("trig").round().clamp(0.0, 2.0) as usize
}

/// The band: 0 Whole, 1 Lows, 2 Highs.
pub fn band(spec: &EffectSpec) -> usize {
    spec.get("band").round().clamp(0.0, 2.0) as usize
}

/// The cycle's length, ms, at the tempo.
pub fn cycle_ms(spec: &EffectSpec, tempo: f32) -> f32 {
    let beats = cantus_dsp::modfx_clock::division_beats(spec.get("div")) as f32;
    beats * 60_000.0 / tempo.max(1.0)
}

/// Where the screen's parts stand in its glass: what the editor's pointer reads too.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    pub inner: Rect,
    /// The shape over one cycle (Tempo, MIDI) or the envelope on a hit (Sidechain).
    pub plot: Rect,
    /// The last four seconds.
    pub hist: Rect,
    /// The band strip, in Lows or Highs.
    pub band: Option<Rect>,
}

/// The screen's layout in `glass`.
pub fn layout(glass: Rect, k: f32, banded: bool) -> Layout {
    let inner = glass.shrink2(vec2(14.0 * k, 10.0 * k));
    // Top to bottom: the header, the plot and its times, the history's title, the history
    // and its seconds, and in a band mode the strip's title, the strip and its hertz.
    let head = 18.0 * k;
    let gap = 10.0 * k;
    let labels = 18.0 * k;
    let title = 20.0 * k;
    let fixed = head + gap + labels + title + labels + if banded { title + labels } else { 0.0 };
    let avail = (inner.height() - fixed).max(60.0 * k);
    let (ph, hh) = if banded {
        (avail * 0.48, avail * 0.27)
    } else {
        (avail * 0.60, avail * 0.40)
    };
    let (x0, x1) = (inner.left() + 46.0 * k, inner.right() - 48.0 * k);
    let mut y = inner.top() + head + gap;
    let plot = Rect::from_min_max(pos2(x0, y), pos2(x1, y + ph));
    y = plot.bottom() + labels + title;
    let hist = Rect::from_min_max(pos2(x0, y), pos2(x1, y + hh));
    y = hist.bottom() + labels + title;
    let band = banded.then(|| Rect::from_min_max(pos2(x0, y), pos2(x1, inner.bottom() - labels)));
    Layout {
        inner,
        plot,
        hist,
        band,
    }
}

/// The screen in `glass` (inside its frame); where the band strip's split is written (a
/// click there switches it).
pub fn show(b: &mut Brush, s: &Style, ink: &Ink, glass: Rect, feed: &Feed<'_>) -> Option<Rect> {
    let k = s.k;
    canvas::ground_b(b, glass, ink, None);
    let Layout {
        inner,
        plot,
        hist,
        band: strip,
    } = layout(glass, k, band(feed.spec) != 0);
    header(b, s, ink, inner, feed);
    if trigger(feed.spec) == 0 {
        envelope(b, s, ink, plot, feed);
    } else {
        shape(b, s, ink, plot, feed);
    }
    trace::history(b, s, ink, hist, feed);
    strip.map(|r| trace::band(b, s, ink, r, feed))
}

/// The header: what the plot shows on the left, the legend on the right.
fn header(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, feed: &Feed<'_>) {
    let k = s.k;
    let y = r.top() + 8.0 * k;
    let trig = trigger(feed.spec);
    let title = if trig == 0 {
        "cedo.d.envelope"
    } else {
        "cedo.d.shape"
    };
    let t = canvas::put(
        b,
        pos2(r.left(), y),
        Align2::LEFT_CENTER,
        &Text::key(title, Role::Micro, ink.text).caps(true),
        k,
        None,
    );
    // The legend: the drawn line and what is ducked now.
    let hue = ink.hue;
    let entries: [(&str, Color32, bool); 2] =
        [("cedo.d.drawn", hue, false), ("cedo.d.now", hue, true)];
    let mut x = r.right();
    let shown = if trig == 0 {
        &entries[..0]
    } else {
        &entries[..]
    };
    for (key, col, filled) in shown.iter().rev() {
        let w = canvas::put(
            b,
            pos2(x, y),
            Align2::RIGHT_CENTER,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            None,
        );
        let sw = Rect::from_center_size(pos2(w.left() - 10.0 * k, y), vec2(12.0 * k, 7.0 * k));
        if *filled {
            b.rect(sw, Fill::Solid(with_alpha(*col, 0.45)));
            b.hline(sw.left(), sw.right(), sw.bottom(), 1.2 * k, *col);
        } else {
            b.hline(sw.left(), sw.right(), sw.center().y, 1.8 * k, *col);
            b.circle(sw.center(), 2.6 * k, Fill::Solid(ink.ground));
            b.circle_stroke(sw.center(), 2.6 * k, 1.1 * k, *col);
        }
        x = sw.left() - 14.0 * k;
    }
    let _ = (t, feed);
}

/// The y of an amount in `plot` (0 at the top).
fn ya(plot: Rect, a: f32) -> f32 {
    plot.top() + plot.height() * a.clamp(0.0, 1.0)
}

/// The decibel scale down the left of a plot: 0 at the top, a step the depth reads in,
/// the depth (or ∞) at the foot.
fn db_scale(b: &mut Brush, s: &Style, ink: &Ink, plot: Rect, depth: f32) {
    let k = s.k;
    let inf = depth >= cantus_dsp::cedo::DEPTH_INF;
    let shown = if inf { 36.0 } else { depth.max(0.0) };
    let step = match shown {
        d if d <= 4.0 => 1.0,
        d if d <= 9.0 => 3.0,
        d if d <= 24.0 => 6.0,
        d if d <= 48.0 => 12.0,
        _ => 24.0,
    };
    let mut last = f32::NEG_INFINITY;
    let mut d = 0.0f32;
    while d < shown - 0.25 * step {
        let y = ya(plot, amount_of_db(d, depth));
        b.hline(
            plot.left(),
            plot.right(),
            y,
            0.6 * k,
            with_alpha(ink.grid_strong, 0.7),
        );
        if y - last > 13.0 * k && plot.bottom() - y > 13.0 * k {
            canvas::putf(
                b,
                pos2(plot.left() - 10.0 * k, y),
                Align2::RIGHT_CENTER,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{}", minus(d)),
            );
            last = y;
        }
        d += step;
    }
    let y = plot.bottom();
    b.hline(
        plot.left(),
        plot.right(),
        y,
        0.8 * k,
        with_alpha(ink.grid_strong, 0.9),
    );
    if inf {
        canvas::putf(
            b,
            pos2(plot.left() - 10.0 * k, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("\u{2212}\u{221e} dB"),
        );
    } else {
        canvas::putf(
            b,
            pos2(plot.left() - 10.0 * k, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{} dB", minus(depth)),
        );
    }
}

/// A reduction as a signed figure: "0", "−6", "−9.5".
fn minus(d: f32) -> Minus {
    Minus(d)
}

struct Minus(f32);

impl std::fmt::Display for Minus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let d = self.0;
        if d.abs() < 0.05 {
            return write!(f, "0");
        }
        if (d - d.round()).abs() < 0.05 {
            write!(f, "\u{2212}{:.0}", d)
        } else {
            write!(f, "\u{2212}{:.1}", d)
        }
    }
}

/// One cycle of the drawn shape, the live duck swept over it, the playhead.
fn shape(b: &mut Brush, s: &Style, ink: &Ink, plot: Rect, feed: &Feed<'_>) {
    let k = s.k;
    let spec = feed.spec;
    let depth = spec.get("depth");
    let tempo = if feed.now.seen && feed.now.tempo > 0.0 {
        feed.now.tempo
    } else {
        120.0
    };
    let ms = cycle_ms(spec, tempo);
    db_scale(b, s, ink, plot, depth);
    // The time grid: quarters of the cycle (thirds for a triplet), fine lines between.
    let div = spec.get("div").round() as i32;
    let parts = if matches!(div, 7 | 10 | 13 | 15) {
        3
    } else {
        4
    };
    let fine = parts * 4;
    for i in 0..=fine {
        let x = plot.left() + plot.width() * i as f32 / fine as f32;
        let major = i % 4 == 0;
        b.vline(
            x,
            plot.top(),
            plot.bottom(),
            if major { 0.7 } else { 0.4 } * k,
            with_alpha(if major { ink.grid_strong } else { ink.grid }, 0.8),
        );
        if major {
            let t = ms * i as f32 / fine as f32;
            let unit = if i == fine { " ms" } else { "" };
            canvas::putf(
                b,
                pos2(x, plot.bottom() + 4.0 * k),
                Align2::CENTER_TOP,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{t:.0}{unit}"),
            );
        }
    }
    let curve = feed.curve;
    let mut pts = [Pos2::ZERO; N];
    for (i, p) in pts.iter_mut().enumerate() {
        let u = i as f32 / (N - 1) as f32;
        let a = if i == N - 1 {
            curve.left_end(shape_at) as f32
        } else {
            amount_at(curve, u)
        };
        *p = pos2(plot.left() + plot.width() * u, ya(plot, a));
    }
    // What the drawn shape takes away, lightly.
    b.area(
        &pts,
        plot.top(),
        with_alpha(ink.hue, if ink.dark { 0.10 } else { 0.08 }),
    );
    // What is ducked now, swept in by the playhead.
    sweep(b, k, ink, plot, feed);
    canvas::trace(b, &pts, 2.0 * k, ink.hue, ink);
    // The breakpoints and the bends.
    let p = curve.points();
    for (i, q) in p.iter().enumerate() {
        let c = pos2(plot.left() + plot.width() * q.x, ya(plot, q.y));
        // The bend handle halfway along the segment that starts here.
        let next = if i + 1 < p.len() {
            p[i + 1].x
        } else {
            p[0].x + 1.0
        };
        let um = 0.5 * (q.x + next);
        let m = pos2(
            plot.left() + plot.width() * um.min(1.0),
            ya(plot, amount_at(curve, um)),
        );
        if q.shape != 2 && um <= 1.0 && (next - q.x) * plot.width() > 26.0 * k {
            let d = 3.6 * k;
            let dia = [
                m + vec2(0.0, -d),
                m + vec2(d, 0.0),
                m + vec2(0.0, d),
                m + vec2(-d, 0.0),
            ];
            b.polygon(&dia, ink.ground);
            let mut ring = [Pos2::ZERO; 5];
            ring[..4].copy_from_slice(&dia);
            ring[4] = dia[0];
            b.polyline(&ring, 1.1 * k, with_alpha(ink.hue, 0.85));
        }
        b.circle(c, 5.0 * k, Fill::Solid(ink.ground));
        b.circle_stroke(c, 5.0 * k, 1.6 * k, ink.hue);
        b.circle(c, 1.8 * k, Fill::Solid(ink.hue));
    }
    // The playhead.
    let ph = feed.now.phase;
    if feed.now.seen && ph >= 0.0 {
        let x = plot.left() + plot.width() * ph.clamp(0.0, 1.0);
        b.vline(
            x,
            plot.top() - 4.0 * k,
            plot.bottom(),
            1.0 * k,
            with_alpha(ink.text, 0.85),
        );
        b.polygon(
            &[
                pos2(x - 4.5 * k, plot.top() - 9.0 * k),
                pos2(x + 4.5 * k, plot.top() - 9.0 * k),
                pos2(x, plot.top() - 3.0 * k),
            ],
            ink.text,
        );
        let y = ya(plot, amount_at(curve, ph));
        b.circle(pos2(x, y), 3.4 * k, Fill::Solid(ink.text));
    }
    // The cycle and its swing, on a plate in the empty corner.
    let swing = spec.get("swing");
    let div_word = crate::i18n::t(&format!("modfx.div.{}", div.clamp(0, 16)));
    let at = pos2(plot.right() - 6.0 * k, plot.bottom() - 6.0 * k);
    let r = canvas::tag(
        b,
        at,
        Align2::RIGHT_BOTTOM,
        super::super::frame::tr("cedo.d.cycle"),
        ink.text,
        k,
        Some(ink.plate),
        format_args!("{div_word} \u{b7} {ms:.0} ms"),
    );
    if swing > 0.005 {
        canvas::tag(
            b,
            pos2(at.x, r.top() - 5.0 * k),
            Align2::RIGHT_BOTTOM,
            super::super::frame::tr("fx.cedo.swing"),
            ink.text,
            k,
            Some(ink.plate),
            format_args!("{:.0} %", swing * 100.0),
        );
    }
}

/// The live duck over the shape: the history's points of this cycle up to the playhead,
/// and the cycle before after it, at their phases.
fn sweep(b: &mut Brush, k: f32, ink: &Ink, plot: Rect, feed: &Feed<'_>) {
    use super::view::{Point, HISTORY, RATE};
    let now = feed.now;
    if !now.seen || feed.view.is_empty() {
        return;
    }
    let view = feed.view;
    let n = view.len();
    // The phosphor's persistence: every point of the last four seconds at its phase, the
    // newest brightest — a short cycle (a sixteenth) is traced over many turns.
    for (i, p) in view.points().enumerate() {
        if p.phase < 0.0 {
            continue;
        }
        let age = (n - 1 - i) as f32 / n.max(1) as f32;
        let q = pos2(
            plot.left() + plot.width() * p.phase.clamp(0.0, 1.0),
            ya(plot, p.amount),
        );
        b.circle(
            q,
            1.5 * k,
            Fill::Solid(with_alpha(ink.hue, 0.15 + 0.55 * (1.0 - age))),
        );
    }
    // A cycle long enough to be drawn point by point is swept in: this cycle up to the
    // playhead, the cycle before after it.
    let tempo = if now.tempo > 0.0 { now.tempo } else { 120.0 };
    if cycle_ms(feed.spec, tempo) * RATE / 1_000.0 < 16.0 {
        return;
    }
    let mut back = [Point::QUIET; HISTORY];
    for (i, p) in view.points().enumerate() {
        back[n - 1 - i] = *p;
    }
    let mut cur = [Pos2::ZERO; HISTORY + 1];
    let mut old = [Pos2::ZERO; HISTORY];
    let (mut nc, mut no) = (0usize, 0usize);
    let mut stage = 0;
    let mut prev = f32::INFINITY;
    for p in &back[..n] {
        if p.phase < 0.0 {
            break;
        }
        if p.phase > prev + 0.02 {
            stage += 1;
        }
        if stage > 1 || (stage == 1 && p.phase <= now.phase) {
            break;
        }
        let q = pos2(
            plot.left() + plot.width() * p.phase.clamp(0.0, 1.0),
            ya(plot, p.amount),
        );
        if stage == 0 {
            cur[nc] = q;
            nc += 1;
        } else {
            old[no] = q;
            no += 1;
        }
        prev = p.phase;
    }
    cur[..nc].reverse();
    old[..no].reverse();
    // The cycle's first point lands on its start.
    if nc > 0 && cur[0].x > plot.left() {
        cur.copy_within(0..nc, 1);
        cur[0] = pos2(plot.left(), ya(plot, amount_at(feed.curve, 0.0)));
        nc += 1;
    }
    if no > 1 {
        b.area(
            &old[..no],
            plot.top(),
            with_alpha(ink.hue, if ink.dark { 0.14 } else { 0.12 }),
        );
    }
    if nc > 1 {
        b.area(
            &cur[..nc],
            plot.top(),
            with_alpha(ink.hue, if ink.dark { 0.40 } else { 0.34 }),
        );
        b.polyline(&cur[..nc], 1.0 * k, with_alpha(ink.hue, 0.9));
    }
}

/// The sidechain's envelope: the key's hit, attack, hold, release, with their handles.
fn envelope(b: &mut Brush, s: &Style, ink: &Ink, plot: Rect, feed: &Feed<'_>) {
    let k = s.k;
    let spec = feed.spec;
    let depth = spec.get("depth");
    db_scale(b, s, ink, plot, depth);
    let (att, hold, rel) = (
        spec.get("att").max(0.0),
        spec.get("hold").max(0.0),
        spec.get("rel").max(0.0),
    );
    let look = spec.get("look").max(0.0);
    let total = (att + hold + rel).max(1.0);
    let lead = total * 0.08;
    let span_ms = lead + total * 1.10;
    let x_of = |t: f32| plot.left() + plot.width() * ((t + lead) / span_ms).clamp(0.0, 1.0);
    // A time grid at a round step.
    let step = nice_step(span_ms / 5.0);
    let mut t = 0.0;
    while t <= span_ms - lead {
        let x = x_of(t);
        b.vline(
            x,
            plot.top(),
            plot.bottom(),
            0.5 * k,
            with_alpha(ink.grid, 0.8),
        );
        canvas::putf(
            b,
            pos2(x, plot.bottom() + 4.0 * k),
            Align2::CENTER_TOP,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{t:.0}"),
        );
        t += step;
    }
    canvas::put(
        b,
        pos2(plot.right(), plot.bottom() + 4.0 * k),
        Align2::RIGHT_TOP,
        &Text::new("ms", Role::Micro, ink.dim).caps(false),
        k,
        None,
    );
    // The curve: nothing before the hit, the S to full, the hold, the release's shape.
    let rshape = cantus_dsp::cedo::env::ReleaseShape::from_index(
        spec.get("rshape").round().max(0.0) as usize,
    );
    let a_of = |t: f32| -> f32 {
        if t < 0.0 {
            0.0
        } else if t < att {
            let p = t / att.max(1e-3);
            p * p * (3.0 - 2.0 * p)
        } else if t < att + hold {
            1.0
        } else if t < att + hold + rel {
            1.0 - rshape.at(((t - att - hold) / rel.max(1e-3)) as f64) as f32
        } else {
            0.0
        }
    };
    let mut pts = [Pos2::ZERO; N];
    for (i, p) in pts.iter_mut().enumerate() {
        let t = -lead + span_ms * i as f32 / (N - 1) as f32;
        *p = pos2(x_of(t), ya(plot, a_of(t)));
    }
    b.area(
        &pts,
        plot.top(),
        with_alpha(ink.hue, if ink.dark { 0.20 } else { 0.16 }),
    );
    canvas::trace(b, &pts, 2.0 * k, ink.hue, ink);
    // The key's hit, and the lookahead before it.
    let key = ink.look.trace2;
    let hx = x_of(0.0);
    canvas::dashed(
        b,
        pos2(hx, plot.top()),
        pos2(hx, plot.bottom()),
        1.0 * k,
        key,
        4.0 * k,
        3.0 * k,
    );
    let hit = canvas::put(
        b,
        pos2(hx + 6.0 * k, plot.top() + 8.0 * k),
        Align2::LEFT_TOP,
        &Text::key("cedo.d.hit", Role::Micro, key).caps(true),
        k,
        Some(ink.plate),
    );
    if look > 0.0 {
        canvas::tag(
            b,
            pos2(hx + 6.0 * k, hit.bottom() + 5.0 * k),
            Align2::LEFT_TOP,
            super::super::frame::tr("fx.cedo.look"),
            key,
            k,
            Some(ink.plate),
            format_args!("{look:.1} ms"),
        );
    }
    // The handles and their times on plates: the attack's and the hold's over the floor of
    // the duck, the release's under its end.
    let marks = [
        (att, 1.0, "fx.cedo.att", att),
        (att + hold, 1.0, "fx.cedo.hold", hold),
        (att + hold + rel, 0.0, "fx.cedo.rel", rel),
    ];
    let mut last_right = f32::NEG_INFINITY;
    for (i, (t, a, key_word, v)) in marks.into_iter().enumerate() {
        let c = pos2(x_of(t), ya(plot, a));
        b.circle(c, 5.0 * k, Fill::Solid(ink.ground));
        b.circle_stroke(c, 5.0 * k, 1.6 * k, ink.hue);
        b.circle(c, 1.8 * k, Fill::Solid(ink.hue));
        let word = super::super::frame::tr(key_word);
        if i == 2 {
            canvas::tag(
                b,
                pos2(c.x - 6.0 * k, c.y + 10.0 * k),
                Align2::RIGHT_TOP,
                word,
                ink.text,
                k,
                Some(ink.plate),
                format_args!("{}", Ms(v)),
            );
            continue;
        }
        let x = (c.x + 8.0 * k).max(last_right + 10.0 * k);
        let r = canvas::tag(
            b,
            pos2(x, c.y - 12.0 * k),
            Align2::LEFT_BOTTOM,
            word,
            ink.text,
            k,
            Some(ink.plate),
            format_args!("{}", Ms(v)),
        );
        last_right = r.right();
    }
    // The amount now: a pointer on the right edge.
    if feed.now.seen {
        let y = ya(plot, feed.now.amount);
        let x = plot.right() + 3.0 * k;
        b.polygon(
            &[
                pos2(x, y),
                pos2(x + 8.0 * k, y - 5.0 * k),
                pos2(x + 8.0 * k, y + 5.0 * k),
            ],
            ink.hue,
        );
    }
}

/// A time in ms, whole above ten.
struct Ms(f32);

impl std::fmt::Display for Ms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.2} s", self.0 / 1_000.0)
        } else if self.0 >= 10.0 {
            write!(f, "{:.0} ms", self.0)
        } else {
            write!(f, "{:.1} ms", self.0)
        }
    }
}

/// A round step near `x`: 1, 2 or 5 times a power of ten.
pub fn nice_step(x: f32) -> f32 {
    let x = x.max(1e-3);
    let p = cantus_dsp::math::powf(10.0f32, cantus_dsp::math::log10(x).floor());
    let m = x / p;
    p * if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    }
}

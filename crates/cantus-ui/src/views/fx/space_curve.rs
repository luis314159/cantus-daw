//! Spatium's curve display: decay time against frequency, in seconds, with the post EQ on
//! a second axis. The display is the control: drag a band's handle, wheel for its width,
//! double-click to make one (its shape follows where), Alt-click to bypass, right-click
//! for its menu, Delete to remove it. Which curve the gestures edit, and each curve's
//! bypass, sit inside the display at its top-left.
//!
//! Four things are drawn so they can be told apart without colour: the **curve** asked
//! for (the thick line, filled against the curve before the drawn bands), the curve
//! **before the bands** (dashed), what the **engine achieves** (dotted, from its fitted
//! filters), and what was **measured** on the last tail (hollow squares). Under the plot,
//! the octave table prints the three in seconds.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::super::multiband::{choice, dashed_poly};
use super::{
    achieved_t60, band_used, remove_band, skey, Curve, Hz, Label, Secs, State, View, G_CURVE,
};
use crate::i18n::{t, tf};
use crate::kit::color::on_fill;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Brush, Fill, Function, Style};
use cantus_core::fx::space::{DECAY_BANDS, POST_BANDS};
use cantus_dsp::spatium::OCTAVES;
use cantus_dsp::Aequalis;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense, Vec2};

const LO_HZ: f32 = 20.0;
const HI_HZ: f32 = 20_000.0;
const POINTS: usize = 200;
const DOT_R: f32 = 7.0;
/// The post EQ's axis, ± dB.
const POST_DB: f32 = 18.0;
/// The head, where the display's selectors sit, at 100 %.
const HEAD: f32 = 28.0;
/// A row of the octave table at 100 %.
const ROW: f32 = 15.0;

const CURVE_LABELS: [Src<'static>; 2] =
    [Src::Key("space.curve.decay"), Src::Key("space.curve.post")];
const CURVE_TIPS: [&str; 2] = ["space.curve.decay_tip", "space.curve.post_tip"];

/// A handle being dragged: which band, and where it and the pointer started.
#[derive(Copy, Clone, Debug)]
pub struct Drag {
    curve: Curve,
    band: usize,
    freq: f32,
    y0: f32,
    origin: Pos2,
}

/// The seconds axis for a decay: a tenth of it to five times it, so the curve sits in
/// the same place whatever the decay and the labels say where that is.
pub fn axis_seconds(decay: f32) -> (f32, f32) {
    ((decay / 10.0).max(0.02), (decay * 5.0).min(120.0))
}

/// A band's colour: the decay bands from the start of the rotation, the post EQ's six
/// further on, so the two curves' handles never share one.
pub fn band_color(curve: Curve, n: usize) -> Color32 {
    match curve {
        Curve::Decay => crate::views::eq_editor::band_color(n),
        Curve::Post => crate::views::eq_editor::band_color(n + DECAY_BANDS),
    }
}

/// The post EQ's colour: the tone hue as ever; in a face, a voice of its own apart from the
/// decay curve's (round 32, the review of delivery 2: both had become the accent).
pub fn post_colour(s: &Style, ink: &Ink) -> Color32 {
    ink.third(s, Function::Tone)
}

/// A band's colour as the display draws it: clamped to the theme's safe band.
fn band_ink(curve: Curve, n: usize, s: &Style) -> Color32 {
    crate::kit::color::safe_track(
        band_color(curve, n),
        s.t.dark,
        crate::kit::color::FillSize::Small,
    )
}

#[derive(Copy, Clone)]
struct Geo {
    rect: Rect,
    lo: f32,
    hi: f32,
}

impl Geo {
    fn x(&self, hz: f32) -> f32 {
        let f = (hz.max(1.0) / LO_HZ).ln() / (HI_HZ / LO_HZ).ln();
        self.rect.left() + f * self.rect.width()
    }
    fn hz(&self, x: f32) -> f32 {
        let f = (x - self.rect.left()) / self.rect.width().max(1.0);
        LO_HZ * (HI_HZ / LO_HZ).powf(f)
    }
    fn y(&self, sec: f32) -> f32 {
        let f = (sec.max(1e-3) / self.lo).ln() / (self.hi / self.lo).ln();
        self.rect.bottom() - f.clamp(0.0, 1.0) * self.rect.height()
    }
    fn sec(&self, y: f32) -> f32 {
        let f = (self.rect.bottom() - y) / self.rect.height().max(1.0);
        self.lo * (self.hi / self.lo).powf(f)
    }
    fn y_db(&self, db: f32) -> f32 {
        self.rect.center().y - (db / POST_DB).clamp(-1.0, 1.0) * self.rect.height() * 0.5
    }
    fn db(&self, y: f32) -> f32 {
        (self.rect.center().y - y) / (self.rect.height() * 0.5) * POST_DB
    }
}

/// A tick on the seconds axis: `50 ms`, `1 s`, `20 s`.
struct Tick(f32);

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 < 1.0 {
            write!(f, "{:.0} ms", self.0 * 1_000.0)
        } else {
            write!(f, "{:.0} s", self.0)
        }
    }
}

fn point_hz(i: usize) -> f32 {
    LO_HZ * (HI_HZ / LO_HZ).powf(i as f32 / (POINTS - 1) as f32)
}

/// The post EQ's summed response at `hz`, dB (without the automatic gain).
fn post_db(view: &View, hz: f32) -> f32 {
    if !view.s.post_eq_on {
        return 0.0;
    }
    view.s
        .post_bands
        .iter()
        .filter(|b| b.active)
        .map(|b| Aequalis::band_response_db(b, 48_000.0, hz))
        .sum()
}

/// Where a band's handle sits.
fn handle(host: &dyn FxHost, view: &View, geo: &Geo, curve: Curve, n: usize) -> Pos2 {
    let spec = host.spec();
    let freq = spec.get(skey(curve, n, "freq"));
    let shape = spec.get(skey(curve, n, "shape")).round() as usize;
    match curve {
        Curve::Decay => {
            let target = view.s.target_curve();
            let y = if shape == 3 {
                geo.rect.bottom() - DOT_R - 2.0
            } else {
                geo.y(target.base_t60(freq) * spec.get(skey(curve, n, "mult")))
            };
            pos2(geo.x(freq), y)
        }
        Curve::Post => {
            let gain = if shape < 3 {
                spec.get(skey(curve, n, "gain"))
            } else {
                0.0
            };
            pos2(geo.x(freq), geo.y_db(gain))
        }
    }
}

/// How many bands a curve has room for.
fn capacity(curve: Curve) -> usize {
    match curve {
        Curve::Decay => DECAY_BANDS,
        Curve::Post => POST_BANDS,
    }
}

/// Whether a curve's handles are shown (and so can be grabbed).
fn shown(view: &View, curve: Curve) -> bool {
    match curve {
        Curve::Decay => view.s.decay_eq_on,
        Curve::Post => view.s.post_eq_on,
    }
}

/// The display: its head, the plot and the octave table under it; then its gestures.
pub fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    view: &View,
    rect: Rect,
) {
    let k = s.k;
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD * k));
    let table_h = (4.0 * ROW + 6.0) * k;
    let table = Rect::from_min_max(pos2(rect.left(), rect.bottom() - table_h), rect.max);
    let plot = Rect::from_min_max(
        pos2(rect.left(), head.bottom()),
        pos2(rect.right(), table.top()),
    );
    let (lo, hi) = axis_seconds(view.s.decay_s);
    let geo = Geo { rect: plot, lo, hi };
    let resp = ui.interact(plot, ui.id().with("space-curve"), Sense::click_and_drag());
    let pointer = resp.hover_pos();
    let mut hovered: Option<(Curve, usize)> = None;
    for curve in [Curve::Post, Curve::Decay] {
        if !shown(view, curve) {
            continue;
        }
        for n in (0..capacity(curve)).filter(|n| band_used(host.spec(), curve, *n)) {
            let c = handle(host, view, &geo, curve, n);
            if pointer.is_some_and(|pt| pt.distance(c) <= (DOT_R + 3.0) * k) {
                hovered = Some((curve, n));
            }
        }
    }
    with_brush(&ui.painter_at(rect), |b| {
        paint(
            b, host, st, s, view, rect, head, &geo, table, pointer, hovered,
        );
    });
    head_controls(host, st, ui, s, view, head);
    gestures(host, st, ui, s, view, &geo, &resp, hovered);
    menu(host, st, ui, s);
    if ui.rect_contains_pointer(table) {
        let cut = st
            .capture
            .measured
            .as_ref()
            .is_some_and(|m| m.complete.iter().any(|c| !c));
        let what = if cut {
            "space.table.cut_tip"
        } else {
            "space.table.tip"
        };
        hint::set_text(
            ui.ctx(),
            frame::tr("space.table.name"),
            frame::tr(what),
            Caps::NONE,
        );
    }
}

/// The head: which curve the gestures edit, each curve's bypass, and the post EQ's gain
/// compensation with the amount it applies.
fn head_controls(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    view: &View,
    head: Rect,
) {
    // The head lies on the display: its controls in the display's style (its fills,
    // its polarity), the theme's in the plain look.
    let s = &crate::kit::skin::display_style(s);
    let k = s.k;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(head.shrink2(vec2(6.0 * k, 3.0 * k)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 4.0 * k;
    let ui = &mut child;
    let tinted = s.function(if st.mode == Curve::Post {
        Function::Tone
    } else {
        Function::Space
    });
    let now = (st.mode == Curve::Post) as usize;
    if let Some(i) = choice(
        ui,
        &tinted,
        now,
        &CURVE_LABELS,
        &CURVE_TIPS,
        frame::tr("space.curve.name"),
        196.0,
    ) {
        st.mode = if i == 1 { Curve::Post } else { Curve::Decay };
    }
    ui.add_space(6.0 * k);
    for (key, label, what, w, f) in [
        (
            "deq_on",
            "space.curve.decay_on",
            "space.curve.decay_on_tip",
            76.0,
            Function::Space,
        ),
        (
            "peq_on",
            "space.curve.post_on",
            "space.curve.post_on_tip",
            56.0,
            Function::Tone,
        ),
        (
            "peq_auto",
            "space.curve.auto",
            "space.curve.auto_tip",
            104.0,
            Function::Tone,
        ),
    ] {
        let r = controls::led_param(ui, &s.function(f), host, key, Src::Key(label), w);
        if r.hovered() {
            hint::set_text(ui.ctx(), frame::tr(label), frame::tr(what), Caps::NONE);
        }
    }
    if view.s.post_eq_on && view.s.post_auto_gain {
        let (_, comp) = view.s.wet_eq();
        if comp.abs() >= 0.05 {
            super::with_fmt(format_args!("{comp:+.1} dB"), |text| {
                controls::readout(ui, s, text, true, frame::tr("space.curve.auto"), 60.0)
            });
        }
    }
}

/// The plot and the table, through one brush: the grid, the post EQ, the decay curves, the
/// measured squares and the handles; then, last, every word.
#[allow(clippy::too_many_arguments)]
fn paint(
    b: &mut Brush,
    host: &dyn FxHost,
    st: &State,
    s: &Style,
    view: &View,
    rect: Rect,
    head: Rect,
    geo: &Geo,
    table: Rect,
    pointer: Option<Pos2>,
    hovered: Option<(Curve, usize)>,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let tone = post_colour(s, &ink);
    let plot = geo.rect;
    let px = b.px();
    canvas::ground_b(b, rect, &ink, s.hue_tok);
    b.hline(
        rect.left(),
        rect.right(),
        b.snap_mid(head.bottom()),
        px,
        ink.edge,
    );
    b.hline(
        rect.left(),
        rect.right(),
        b.snap_mid(table.top()),
        px,
        ink.edge,
    );

    // Grid: decades and their 2/5 marks; seconds on a 1-2-5 ladder.
    for decade in [10.0f32, 100.0, 1_000.0, 10_000.0] {
        for m in [1.0, 2.0, 5.0] {
            let hz = decade * m;
            if !(LO_HZ..=HI_HZ).contains(&hz) {
                continue;
            }
            let c = if m == 1.0 { ink.grid_strong } else { ink.grid };
            b.vline(b.snap_mid(geo.x(hz)), plot.top(), plot.bottom(), px, c);
        }
    }
    const LADDER: [f32; 13] = [
        0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0,
    ];
    for v in LADDER {
        if v >= geo.lo * 0.999 && v <= geo.hi * 1.001 {
            b.hline(
                plot.left(),
                plot.right(),
                b.snap_mid(geo.y(v)),
                px,
                ink.grid,
            );
        }
    }

    // The post EQ: a dashed line in the tone hue, faintly filled against 0 dB.
    let post = view.s.post_eq_on && view.s.post_bands.iter().any(|x| x.active);
    let mut pts = [Pos2::ZERO; POINTS];
    if post {
        for (i, p) in pts.iter_mut().enumerate() {
            *p = pos2(geo.x(point_hz(i)), geo.y_db(post_db(view, point_hz(i))));
        }
        b.area(
            &pts,
            geo.y_db(0.0),
            Ink::a(tone, if ink.dark { 0.10 } else { 0.08 }),
        );
        dashed_poly(b, &pts, 1.75 * k, tone, 7.0 * k, 3.0 * k);
    }

    // The decay curve: before the bands (dashed), as asked (thick, filled against the
    // former), and as achieved (dotted).
    let c = view.s.target_curve();
    let mut base = [Pos2::ZERO; POINTS];
    for (i, p) in base.iter_mut().enumerate() {
        *p = pos2(geo.x(point_hz(i)), geo.y(c.base_t60(point_hz(i))));
    }
    for (i, p) in pts.iter_mut().enumerate() {
        *p = pos2(geo.x(point_hz(i)), geo.y(c.t60(point_hz(i))));
    }
    b.between(&pts, &base, ink.fill(0.9));
    dashed_poly(b, &base, px.max(k), ink.dim, 5.0 * k, 4.0 * k);
    b.polyline(&pts, 2.5 * k, ink.hue);
    let mut n = 0;
    for i in (0..POINTS).step_by(2) {
        let hz = point_hz(i);
        if !(31.0..=16_000.0).contains(&hz) {
            continue;
        }
        if let Some(a) = achieved_t60(st, view, hz) {
            base[n] = pos2(geo.x(hz), geo.y(a));
            n += 1;
        }
    }
    if n > 1 {
        dashed_poly(b, &base[..n], 1.25 * k, ink.text, 2.0 * k, 3.0 * k);
    }
    // Measured on the last tail: hollow squares at the octaves. An octave whose tail was
    // cut before it fell far enough is extrapolated: dimmer.
    if let Some(m) = &st.capture.measured {
        for ((hz, t60), complete) in OCTAVES.iter().zip(m.t60).zip(m.complete) {
            if let Some(t60) = t60 {
                let at = pos2(geo.x(*hz), geo.y(t60));
                let color = if complete { ink.text } else { ink.dim };
                b.rrect_stroke(
                    Rect::from_center_size(at, Vec2::splat(8.0 * k)),
                    k,
                    1.5 * k,
                    color,
                );
            }
        }
    }

    // Handles: the decay bands as numbered circles, the post EQ's as lettered squares.
    let spec = host.spec();
    let selected = st.selected;
    for curve in [Curve::Post, Curve::Decay] {
        if !shown(view, curve) {
            continue;
        }
        for n in (0..capacity(curve)).filter(|n| band_used(spec, curve, *n)) {
            let at = handle(host, view, geo, curve, n);
            let color = band_ink(curve, n, s);
            let on = spec.get(skey(curve, n, "on")) >= 0.5;
            let r = DOT_R * k;
            match curve {
                Curve::Decay if on => b.circle(at, r, Fill::Solid(color)),
                Curve::Decay => b.circle_stroke(at, r, 1.5 * k, color),
                Curve::Post => {
                    let sq = Rect::from_center_size(at, Vec2::splat(2.0 * r));
                    if on {
                        b.rrect(sq, 3.0 * k, Fill::Solid(color));
                    } else {
                        b.rrect_stroke(sq, 3.0 * k, 1.5 * k, color);
                    }
                }
            }
            if selected == Some((curve, n)) || hovered == Some((curve, n)) {
                b.circle_stroke(at, r + 3.0 * k, 1.5 * k, ink.text);
            }
        }
    }
    if let Some(pt) = pointer {
        b.vline(
            b.snap_mid(pt.x),
            plot.top(),
            plot.bottom(),
            px,
            Ink::a(ink.dim, 0.5),
        );
    }

    // ---- the words, last ----
    let micro =
        |b: &mut Brush, at: Pos2, align: Align2, color: Color32, args: std::fmt::Arguments<'_>| {
            canvas::putf(b, at, align, Role::Micro, color, tk, None, args)
        };
    let mut ruler = canvas::Ruler::new();
    for decade in [10.0f32, 100.0, 1_000.0, 10_000.0] {
        for m in [1.0, 2.0, 5.0] {
            let hz = decade * m;
            if !(LO_HZ..=HI_HZ).contains(&hz) {
                continue;
            }
            let x = geo.x(hz) + 3.0 * k;
            if !ruler.fits(x, 6.0 * k) {
                continue;
            }
            let r = micro(
                b,
                pos2(x, plot.bottom() - 2.0 * k),
                Align2::LEFT_BOTTOM,
                ink.dim,
                format_args!("{}", Hz(hz)),
            );
            ruler.placed(r);
        }
    }
    // The seconds at the left, clear of the frequencies along the bottom.
    for v in LADDER {
        if v < geo.lo * 0.999 || v > geo.hi * 1.001 {
            continue;
        }
        let y = geo.y(v);
        if y > plot.bottom() - 16.0 * k || y < plot.top() + 10.0 * k {
            continue;
        }
        micro(
            b,
            pos2(plot.left() + 3.0 * k, y - 1.0),
            Align2::LEFT_BOTTOM,
            ink.dim,
            format_args!("{}", Tick(v)),
        );
    }
    if view.s.post_eq_on {
        for db in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
            let y = geo.y_db(db);
            if y > plot.bottom() - 16.0 * k {
                continue;
            }
            // On plates: the curves end under this scale.
            let at = pos2(plot.right() - 3.0 * k, y - 1.0);
            let plate = Some(ink.plate);
            if db == 0.0 {
                canvas::putf(
                    b,
                    at,
                    Align2::RIGHT_BOTTOM,
                    Role::Micro,
                    tone,
                    tk,
                    plate,
                    format_args!("0"),
                );
            } else {
                canvas::putf(
                    b,
                    at,
                    Align2::RIGHT_BOTTOM,
                    Role::Micro,
                    tone,
                    tk,
                    plate,
                    format_args!("{db:+.0}"),
                );
            }
        }
    }
    let mut any = false;
    for curve in [Curve::Post, Curve::Decay] {
        if !shown(view, curve) {
            continue;
        }
        for n in (0..capacity(curve)).filter(|n| band_used(spec, curve, *n)) {
            any = true;
            let at = handle(host, view, geo, curve, n);
            let on = spec.get(skey(curve, n, "on")) >= 0.5;
            let color = if on {
                on_fill(band_ink(curve, n, s))
            } else {
                ink.text
            };
            micro(
                b,
                at,
                Align2::CENTER_CENTER,
                color,
                format_args!("{}", Label(curve, n)),
            );
        }
    }
    // What is under the pointer: the frequency, the curve, the engine, the EQ.
    if let Some(pt) = pointer {
        let hz = geo.hz(pt.x);
        let at = pos2(plot.right() - 30.0 * k, plot.top() + 4.0 * k);
        let plate = Some(ink.plate);
        let curve_word = frame::tr("space.curve.decay");
        let engine = frame::tr("space.engine");
        match (achieved_t60(st, view, hz), post) {
            (Some(a), true) => canvas::putf(
                b,
                at,
                Align2::RIGHT_TOP,
                Role::Label,
                ink.text,
                tk,
                plate,
                format_args!(
                    "{}  \u{b7}  {curve_word} {}  \u{b7}  {engine} {}  \u{b7}  EQ {:+.1} dB",
                    Hz(hz),
                    Secs(c.t60(hz)),
                    Secs(a),
                    post_db(view, hz)
                ),
            ),
            (Some(a), false) => canvas::putf(
                b,
                at,
                Align2::RIGHT_TOP,
                Role::Label,
                ink.text,
                tk,
                plate,
                format_args!(
                    "{}  \u{b7}  {curve_word} {}  \u{b7}  {engine} {}",
                    Hz(hz),
                    Secs(c.t60(hz)),
                    Secs(a)
                ),
            ),
            (None, _) => canvas::putf(
                b,
                at,
                Align2::RIGHT_TOP,
                Role::Label,
                ink.text,
                tk,
                plate,
                format_args!("{}  \u{b7}  {curve_word} {}", Hz(hz), Secs(c.t60(hz))),
            ),
        };
    } else if !any {
        canvas::put_key(
            b,
            plot.center(),
            Align2::CENTER_CENTER,
            "space.curve.empty",
            Role::Body,
            ink.dim,
            tk,
        );
    }
    octave_table(b, st, s, view, table);
}

/// `63`, `250`, `1.0 kHz`: an octave's name in the table.
struct Octave(f32);

impl std::fmt::Display for Octave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.1} kHz", self.0 / 1_000.0)
        } else {
            write!(f, "{:.0}", self.0)
        }
    }
}

/// Decay in seconds per octave, three ways, always on screen: the curve asked for, what
/// the engine's filters achieve, and what was measured on the last tail.
fn octave_table(b: &mut Brush, st: &State, s: &Style, view: &View, rect: Rect) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let inner = rect.shrink2(vec2(6.0 * k, 3.0 * k));
    let label_w = 84.0 * k;
    let col = (inner.width() - label_w) / OCTAVES.len() as f32;
    let row = inner.height() / 4.0;
    let c = view.s.target_curve();
    let measured = st.capture.measured.as_ref();
    b.hline(
        inner.left(),
        inner.right(),
        b.snap_mid(inner.top() + row),
        b.px(),
        ink.grid_strong,
    );
    let rows: [(&str, Color32); 4] = [
        ("Hz", ink.dim),
        (frame::tr("space.table.curve"), ink.hue),
        (frame::tr("space.table.engine"), ink.text),
        (frame::tr("space.table.measured"), ink.text),
    ];
    let cell = |b: &mut Brush, x: f32, y: f32, color: Color32, args: std::fmt::Arguments<'_>| {
        canvas::putf(
            b,
            pos2(x, y),
            Align2::CENTER_CENTER,
            Role::Micro,
            color,
            tk,
            None,
            args,
        );
    };
    for (r, (label, color)) in rows.iter().enumerate() {
        let y = inner.top() + row * (r as f32 + 0.5);
        canvas::putf(
            b,
            pos2(inner.left(), y),
            Align2::LEFT_CENTER,
            Role::Micro,
            *color,
            tk,
            None,
            format_args!("{label}"),
        );
        if r == 3 && measured.is_none() {
            // Nothing measured yet: say when it will be, in the row's place.
            canvas::putf(
                b,
                pos2(inner.left() + label_w, y),
                Align2::LEFT_CENTER,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{}", frame::tr("space.table.measured_hint")),
            );
            continue;
        }
        for (i, hz) in OCTAVES.iter().enumerate() {
            let x = inner.left() + label_w + col * (i as f32 + 0.5);
            match r {
                0 => cell(b, x, y, *color, format_args!("{}", Octave(*hz))),
                1 => cell(b, x, y, *color, format_args!("{:.2}", c.t60(*hz))),
                2 => match achieved_t60(st, view, *hz) {
                    Some(v) => cell(b, x, y, *color, format_args!("{v:.2}")),
                    None => cell(b, x, y, ink.dim, format_args!("\u{2013}")),
                },
                _ => {
                    // A measured octave whose tail was cut before it fell far enough is an
                    // estimate — "≈", dimmer — or, when too little of it was heard to
                    // estimate, "cut" in words rather than a bare dash (ADR 0022).
                    let cut = measured.is_some_and(|m| !m.complete[i]);
                    match measured.and_then(|m| m.t60[i]) {
                        Some(v) if cut => cell(b, x, y, ink.dim, format_args!("\u{2248}{v:.2}")),
                        Some(v) => cell(b, x, y, *color, format_args!("{v:.2}")),
                        None if cut => cell(
                            b,
                            x,
                            y,
                            ink.dim,
                            format_args!("{}", frame::tr("space.table.cut")),
                        ),
                        None => cell(b, x, y, ink.dim, format_args!("\u{2013}")),
                    }
                }
            }
        }
    }
}

/// Make a band at the first free number on a curve, shaped by where the pointer is. One
/// step.
fn add_band(host: &mut dyn FxHost, st: &mut State, view: &View, geo: &Geo, curve: Curve, pt: Pos2) {
    let Some(n) = (0..capacity(curve)).find(|b| !band_used(host.spec(), curve, *b)) else {
        host.status(t("space.no_free_band"));
        return;
    };
    let fx = (pt.x - geo.rect.left()) / geo.rect.width();
    let fy = (pt.y - geo.rect.top()) / geo.rect.height();
    let freq = geo.hz(pt.x).clamp(20.0, 20_000.0);
    let key = |f: &str| skey(curve, n, f);
    let mut edits: [(&str, Option<f32>); 4] = [
        (key("used"), Some(1.0)),
        (key("freq"), Some(freq)),
        ("", None),
        ("", None),
    ];
    match curve {
        Curve::Decay => {
            // Edges make shelves; the floor makes a notch; anywhere else a bell.
            let shape = if fx < 0.1 {
                1.0
            } else if fx > 0.9 {
                2.0
            } else if fy > 0.92 {
                3.0
            } else {
                0.0
            };
            edits[2] = (key("shape"), Some(shape));
            edits[3] = if shape != 3.0 {
                let base = view.s.target_curve().base_t60(freq);
                (key("mult"), Some((geo.sec(pt.y) / base).clamp(0.25, 4.0)))
            } else {
                (key("q"), Some(4.0))
            };
        }
        Curve::Post => {
            let shape = if fx < 0.05 {
                3.0
            } else if fx > 0.95 {
                4.0
            } else if fx < 0.12 {
                1.0
            } else if fx > 0.88 {
                2.0
            } else {
                0.0
            };
            edits[2] = (key("shape"), Some(shape));
            if shape < 3.0 {
                edits[3] = (key("gain"), Some(geo.db(pt.y).clamp(-18.0, 18.0)));
            }
        }
    }
    let used = if edits[3].0.is_empty() { 3 } else { 4 };
    host.apply(&edits[..used], "cmd.space_edit", None);
    st.selected = Some((curve, n));
    st.mode = curve;
}

#[allow(clippy::too_many_arguments)]
fn gestures(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    view: &View,
    geo: &Geo,
    resp: &egui::Response,
    hovered: Option<(Curve, usize)>,
) {
    let fine = ui.input(|i| i.modifiers.shift);
    let alt = ui.input(|i| i.modifiers.alt);
    if resp.drag_started() {
        if let (Some((curve, n)), Some(origin)) = (hovered, resp.interact_pointer_pos()) {
            let c = handle(host, view, geo, curve, n);
            st.drag = Some(Drag {
                curve,
                band: n,
                freq: host.spec().get(skey(curve, n, "freq")),
                y0: c.y,
                origin,
            });
            st.selected = Some((curve, n));
            st.mode = curve;
        }
    }
    if let (Some(d), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let scale = if fine { 0.2 } else { 1.0 };
            let delta = (pt - d.origin) * scale;
            let freq = geo.hz(geo.x(d.freq) + delta.x).clamp(20.0, 20_000.0);
            let y = d.y0 + delta.y;
            let key = |f: &str| skey(d.curve, d.band, f);
            let shape = host.spec().get(key("shape")).round() as usize;
            let second = match d.curve {
                Curve::Decay if shape != 3 => {
                    let base = view.s.target_curve().base_t60(freq);
                    Some((key("mult"), Some((geo.sec(y) / base).clamp(0.25, 4.0))))
                }
                Curve::Post if shape < 3 => Some((key("gain"), Some(geo.db(y).clamp(-18.0, 18.0)))),
                _ => None,
            };
            let first = (key("freq"), Some(freq));
            match second {
                Some(e) => host.apply(&[first, e], "cmd.space_edit", Some(G_CURVE)),
                None => host.apply(&[first], "cmd.space_edit", Some(G_CURVE)),
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.clicked() {
        match hovered {
            Some((curve, n)) if alt => {
                let k = skey(curve, n, "on");
                let on = host.spec().get(k) >= 0.5;
                host.set(k, if on { 0.0 } else { 1.0 }, None);
            }
            Some((curve, n)) => {
                st.selected = Some((curve, n));
                st.mode = curve;
            }
            None => st.selected = None,
        }
    }
    if resp.double_clicked() && hovered.is_none() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let mode = st.mode;
            add_band(host, st, view, geo, mode, pt);
        }
    }
    // The wheel over a handle: its width (Q), or a cut's slope in 6 dB steps.
    if let Some((curve, n)) = hovered {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let shape = host.spec().get(skey(curve, n, "shape")).round() as usize;
            if curve == Curve::Post && shape >= 3 {
                let k = skey(curve, n, "slope");
                let slope = host.spec().get(k);
                let step = if scroll > 0.0 { 6.0 } else { -6.0 };
                host.set(k, (slope + step).clamp(6.0, 96.0), None);
            } else {
                let k = skey(curve, n, "q");
                let q = host.spec().get(k);
                let rate = if fine { 0.0015 } else { 0.006 };
                host.set(k, (q * (scroll * rate).exp2()).clamp(0.1, 10.0), Some(k));
            }
        }
    }
    if resp.secondary_clicked() {
        let band = hovered
            .or(st.selected)
            .filter(|(c, n)| band_used(host.spec(), *c, *n));
        if let (Some((curve, n)), Some(at)) =
            (band, resp.interact_pointer_pos().or(resp.hover_pos()))
        {
            st.menu = Some((curve, n, at));
        }
    }
    if let Some((curve, n)) = st.selected {
        if resp.hovered() && ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            remove_band(host, st, curve, n);
        }
    }
    if hovered.is_some() || st.drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr(if st.mode == Curve::Post {
                "space.curve.post"
            } else {
                "space.curve.decay"
            }),
            frame::tr("space.curve.tip"),
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let _ = s;
}

/// A band's menu, while open: its shapes, and remove it.
fn menu(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style) {
    let Some((curve, n, at)) = st.menu else {
        return;
    };
    if !band_used(host.spec(), curve, n) {
        st.menu = None;
        return;
    }
    let title = match curve {
        Curve::Decay => tf("space.decay_band_n", &[("n", &Label(curve, n).to_string())]),
        Curve::Post => tf("space.post_band_n", &[("n", &Label(curve, n).to_string())]),
    };
    let keys: &[&'static str] = match curve {
        Curve::Decay => &super::DSHAPE_KEYS,
        Curve::Post => &super::PSHAPE_KEYS,
    };
    let shape = host.spec().get(skey(curve, n, "shape")).round() as usize;
    let mut items: Vec<Item<'_>> = Vec::with_capacity(keys.len() + 3);
    items.push(Item::Band(Src::Str(&title)));
    for (i, key) in keys.iter().enumerate() {
        items.push(frame::radio(Src::Key(key), i == shape));
    }
    items.push(Item::Separator);
    items.push(frame::action(Src::Key("space.remove_band")));
    let mut close = false;
    let chosen = frame::popup_list(
        ui.ctx(),
        ui.id().with("space-band-menu"),
        at,
        Rect::from_center_size(at, Vec2::splat(2.0)),
        s,
        &items,
        200.0 * s.k,
        &mut close,
    );
    let count = items.len();
    drop(items);
    if let Some(i) = chosen {
        close = true;
        if i >= 1 && i <= keys.len() {
            host.set(skey(curve, n, "shape"), (i - 1) as f32, None);
        } else if i + 1 == count {
            remove_band(host, st, curve, n);
        }
    }
    if close {
        st.menu = None;
    }
}

#[cfg(test)]
mod face_colour_tests {
    use super::*;
    use crate::kit::color::{contrast, Hsl};

    fn gap(a: Color32, b: Color32) -> f32 {
        let (a, b) = (Hsl::of(a), Hsl::of(b));
        if a.s < 0.15 || b.s < 0.15 {
            return 180.0;
        }
        let d = (a.h - b.h).rem_euclid(360.0);
        d.min(360.0 - d)
    }

    fn face(base: crate::kit::Tokens) -> (Style, Ink) {
        let sheet = crate::kit::skin::of_effect(cantus_core::EffectKind::Space);
        let s = Style::new(base).skinned(sheet).function(Function::Space);
        let ink = Ink::of(&s);
        (s, ink)
    }

    fn pairs(base: crate::kit::Tokens) -> Vec<(&'static str, Color32, Color32)> {
        let (s, ink) = face(base);
        vec![(
            "the post EQ and the decay curve",
            post_colour(&s, &ink),
            ink.hue,
        )]
    }

    fn grounds(base: crate::kit::Tokens) -> Vec<(&'static str, Color32, Color32)> {
        let (s, ink) = face(base);
        vec![("the post EQ", post_colour(&s, &ink), ink.ground)]
    }

    /// Round 32, the review of delivery 2: colours that tell two things apart inside the
    /// display stay apart in the unit's face (at least 20° of hue) and read on the look's
    /// ground (3:1), in every preset.
    #[test]
    fn colours_with_a_meaning_stay_apart_in_the_face() {
        let mut bad = Vec::new();
        for (key, make) in crate::theme::PRESETS {
            let base = make().kit_tokens();
            for (what, a, b) in pairs(base) {
                if gap(a, b) < 20.0 {
                    bad.push(format!("{key}: {what} {:.0}° apart", gap(a, b)));
                }
            }
            for (what, c, ground) in grounds(base) {
                if contrast(c, ground) < 2.99 {
                    bad.push(format!(
                        "{key}: {what} at {:.2} on the ground",
                        contrast(c, ground)
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}

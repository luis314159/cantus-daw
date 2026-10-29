//! What the modulation family's editors share (ADR 0055): the LFO card (sync, rate or
//! note value, shape, soft edge, swing, stereo spread and start phase), the **movement
//! display** — what the LFO does over two cycles, drawn from the same settings the DSP
//! reads, with a playhead where the unit is now — and the pills every one of them prints.
//!
//! Each editor maps the LFO through its own picture (a gain, a pan, a delay, a sweep), so
//! the display takes a function from a position in cycles to the left and right values.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::FxHost;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Style};
use cantus_dsp::modfx_clock::tel;
use cantus_dsp::modfx_lfo::{self, Shape};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::fmt::Write as _;

/// A card's style in a modulation unit: the function's hue in the plain look; in a unit's
/// own face, its accent for the Mod function too — the theme's modulation hue stays for
/// modulation cues (a knob's rings, the live dot; ADR 0074), never a unit's identity.
pub fn own(s: &Style, f: crate::kit::Function) -> Style {
    if f == crate::kit::Function::Mod && s.skin.accent.is_some() {
        body(s)
    } else {
        s.function(f)
    }
}

/// A modulation unit's body style: the device frame asks for the family's function (Mod),
/// and a skinned style keeps the theme's modulation hue for it (ADR 0074); in the unit's
/// own face the body paints in its accent instead (as the tokens' tone hue holds it).
pub fn body(s: &Style) -> Style {
    if s.skin.accent.is_some() && s.hue_tok == Some(crate::kit::Tok::HueMod) {
        Style {
            hue: s.t.hue_tone,
            hue_tok: Some(crate::kit::Tok::HueTone),
            ..*s
        }
    } else {
        *s
    }
}

/// The family's characteristic displays (round 32, ADR 0074).
#[path = "modfx_face.rs"]
pub mod face;

/// The note values of the synced rates, as [`cantus_dsp::modfx_clock::DIVISIONS`] holds
/// them: the same words in every language.
pub const DIV_NAMES: [&str; 17] = [
    "8/1",
    "4/1",
    "2/1",
    "1/1",
    "1/2\u{b7}",
    "1/2",
    "1/4\u{b7}",
    "1/2T",
    "1/4",
    "1/8\u{b7}",
    "1/4T",
    "1/8",
    "1/16\u{b7}",
    "1/8T",
    "1/16",
    "1/16T",
    "1/32",
];

/// The shapes' names, by index (`modfx.shape.{i}`).
pub const SHAPE_KEYS: [&str; 6] = [
    "modfx.shape.0",
    "modfx.shape.1",
    "modfx.shape.2",
    "modfx.shape.3",
    "modfx.shape.4",
    "modfx.shape.5",
];

// ---- pills ----------------------------------------------------------------------------

pub fn div_pill(v: f32, b: &mut String) {
    let i = (v.round().max(0.0) as usize).min(DIV_NAMES.len() - 1);
    b.push_str(DIV_NAMES[i]);
}

pub fn pct_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{:.0}%", v * 100.0);
}

pub fn deg_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.0}\u{b0}");
}

pub fn db_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:+.1} dB");
}

pub fn ms_pill(v: f32, b: &mut String) {
    controls::write_readout(v, "ms", b);
}

pub fn hz_pill(v: f32, b: &mut String) {
    if v < 10.0 {
        let _ = write!(b, "{v:.2} Hz");
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

// ---- the LFO card ---------------------------------------------------------------------

/// What an editor's LFO card holds besides the rate and the shape.
#[derive(Copy, Clone, Debug, Default)]
pub struct LfoCard {
    /// The shapes it offers, in the table's order (empty: none; a vibrato offers two).
    pub shapes: &'static [Shape],
    pub edge: bool,
    pub swing: bool,
    pub spread: bool,
    pub offset: bool,
}

/// The width of an LFO card at 100 %.
pub fn lfo_card_width(c: &LfoCard) -> f32 {
    let mut sizes = vec![Size::M];
    for on in [c.edge, c.swing, c.spread, c.offset] {
        if on {
            sizes.push(Size::M);
        }
    }
    controls::cells_width(&sizes) + 4.0 + SWITCH_W + 16.0
}

/// The column of switches beside the cells: sync and the shapes.
pub const SWITCH_W: f32 = 168.0;

/// Whether the spec's LFO follows the song.
pub fn synced(host: &dyn FxHost) -> bool {
    host.spec().get("sync") >= 0.5
}

/// The LFO card: the sync switch and the shapes in a column, then rate (or note value),
/// edge, swing, spread and start phase as cells.
pub fn lfo_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, c: &LfoCard) {
    controls::card(ui, s, rect, Src::Key("modfx.card.lfo"), None, |ui, _| {
        controls::column(ui, s, SWITCH_W, |ui| {
            let labels = [Src::Key("modfx.sync.0"), Src::Key("modfx.sync.1")];
            controls::segmented_param(ui, s, host, "sync", &labels, SWITCH_W);
            if !c.shapes.is_empty() {
                shape_keys(host, ui, s, SWITCH_W, c.shapes);
            }
        });
        rate_cell(host, ui, s, Size::M);
        if c.edge {
            let stepped = Shape::from_index(host.spec().get("shape")).has_edge();
            ui.add_enabled_ui(stepped || c.shapes.is_empty(), |ui| {
                controls::param_with(ui, s, host, "edge", Size::M, Some(&pct_pill));
            });
        }
        if c.swing {
            let on = synced(host);
            ui.add_enabled_ui(on, |ui| {
                // 0 … 100 %, as the exported plug-ins and a typed value read it: 0 straight,
                // 100 % the second cycle of each pair half a cycle late (an MPC's 75 %).
                controls::param_with(ui, s, host, "swing", Size::M, Some(&pct_pill));
            });
        }
        if c.spread {
            controls::param_with(ui, s, host, "spread", Size::M, Some(&deg_pill));
        }
        if c.offset {
            controls::param_with(ui, s, host, "offset", Size::M, Some(&deg_pill));
        }
    });
}

/// The rate: a note value while synced, Hz while free — one cell that changes with the
/// switch.
pub fn rate_cell(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, size: Size) {
    if synced(host) {
        controls::param_with(ui, s, host, "div", size, Some(&div_pill));
    } else {
        controls::param_with(ui, s, host, "rate", size, Some(&hz_pill));
    }
}

/// The shapes as keys with their waves drawn on them; the lit one is the LFO's. A
/// shape's value is its index in the list.
pub fn shape_keys(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    width: f32,
    shapes: &[Shape],
) {
    let now = host.spec().get("shape").round().max(0.0) as usize;
    let (rect, _) = ui.allocate_exact_size(vec2(width * s.k, s.d(controls::ROW)), Sense::hover());
    let n = shapes.len().max(1);
    let gap = s.d(2.0);
    let w = (rect.width() - gap * (n - 1) as f32) / n as f32;
    for (i, &shape) in shapes.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rect.left() + i as f32 * (w + gap), rect.top()),
            vec2(w, rect.height()),
        );
        let resp = ui.interact(r, ui.id().with(("modfx-shape", i)), Sense::click());
        let st = crate::kit::State::of(&resp, false).lit(i == now);
        if ui.is_rect_visible(r) {
            with_brush(ui.painter(), |b| {
                controls::paint_key(b, r, s, st, Src::Str(""));
                let ink = if i == now { s.t.text } else { s.hue };
                shape_glyph(b, r.shrink2(vec2(5.0 * s.k, 5.0 * s.k)), shape, ink, s.k);
            });
        }
        // The row is the parameter's control: each button its menu and the learn mode
        // (round 29, review 2 of ADR 0062, #44).
        let name = host
            .spec()
            .kind
            .param("shape")
            .map_or("", |p| frame::tr(p.label_key));
        let learnt = controls::stepped_offer(ui, s, host, "shape", name, &resp);
        if resp.clicked() && i != now && !learnt {
            host.set("shape", i as f32, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr(SHAPE_KEYS[i]),
                frame::tr("modfx.shape_tip"),
                Caps::NONE,
            );
        }
    }
}

/// One cycle of a shape in a small box.
pub fn shape_glyph(b: &mut crate::kit::Brush, r: Rect, shape: Shape, ink: egui::Color32, k: f32) {
    const N: usize = 25;
    let mut pts = [Pos2::ZERO; N];
    for (j, p) in pts.iter_mut().enumerate() {
        let ph = j as f64 / (N - 1) as f64;
        let v = modfx_lfo::wave(shape, ph.min(0.9999), 0, 0.15, 7);
        *p = pos2(
            r.left() + r.width() * ph as f32,
            r.center().y - v * r.height() * 0.5,
        );
    }
    b.polyline(&pts, 1.4 * k, ink);
}

// ---- the movement display -------------------------------------------------------------

/// What the display draws: a value per channel for a position in cycles, both in −1 … 1
/// (the top of the display is 1).
pub type Curve<'a> = &'a dyn Fn(f64) -> (f32, f32);

/// The movement display: two cycles of `curve`, the right channel faint under the left
/// when they differ, the cycle lines, a playhead at the unit's phase (from the telemetry),
/// and the rate in words in the corner. `top`/`bottom` name the display's ends.
#[allow(clippy::too_many_arguments)]
pub fn movement(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    host: &dyn FxHost,
    tel: &[f32; 64],
    curve: Curve<'_>,
    top: &str,
    bottom: &str,
) {
    movement_with(
        ui,
        s,
        rect,
        host,
        tel,
        curve,
        top,
        bottom,
        &Extras::default(),
    );
}

/// What a display may draw beyond its left and right curves.
#[derive(Default)]
pub struct Extras<'a> {
    /// More curves, faint (a chorus's other voices).
    pub more: &'a [&'a dyn Fn(f64) -> f32],
    /// The rate in Hz when the unit's own table does not say it (a chorus model's fixed
    /// rate).
    pub hz: Option<f64>,
    /// The unit's own marks over its curves (round 32, ADR 0074).
    pub marks: face::Marks,
}

/// [`movement`] with [`Extras`].
#[allow(clippy::too_many_arguments)]
pub fn movement_with(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    host: &dyn FxHost,
    tel: &[f32; 64],
    curve: Curve<'_>,
    top: &str,
    bottom: &str,
    extras: &Extras<'_>,
) {
    let ink = Ink::of(s);
    let k = s.k;
    const N: usize = 241;
    let cycles = 2.0f64;
    let plot = rect.shrink2(vec2(10.0 * k, 14.0 * k));
    let y_of = |v: f32| plot.center().y - v.clamp(-1.0, 1.0) * plot.height() * 0.5;
    let mut left = [Pos2::ZERO; N];
    let mut right = [Pos2::ZERO; N];
    let mut differ = false;
    for j in 0..N {
        let c = cycles * j as f64 / (N - 1) as f64;
        let (a, b) = curve(c);
        let x = plot.left() + plot.width() * j as f32 / (N - 1) as f32;
        left[j] = pos2(x, y_of(a));
        right[j] = pos2(x, y_of(b));
        differ |= (a - b).abs() > 1e-4;
    }
    let spec = host.spec();
    // The unit's marks only in its own face: with the switch off, the picture as before.
    let marks = if face::own_face(s) {
        extras.marks
    } else {
        face::Marks::None
    };
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        // The middle line and the cycle lines.
        b.hline(
            plot.left(),
            plot.right(),
            plot.center().y,
            b.px(),
            ink.grid_strong,
        );
        for q in [0.25f32, 0.75] {
            let y = plot.top() + plot.height() * q;
            b.hline(plot.left(), plot.right(), y, b.px(), ink.grid);
        }
        for c in 0..=2 {
            let x = plot.left() + plot.width() * c as f32 / 2.0;
            b.vline(x, plot.top(), plot.bottom(), b.px(), ink.grid_strong);
        }
        b.area(&left, plot.bottom(), ink.fill(0.6));
        for more in extras.more {
            let mut pts = [Pos2::ZERO; N];
            for (j, p) in pts.iter_mut().enumerate() {
                let c = cycles * j as f64 / (N - 1) as f64;
                let x = plot.left() + plot.width() * j as f32 / (N - 1) as f32;
                *p = pos2(x, y_of(more(c)));
            }
            b.polyline(&pts, 1.2 * k, ink.faint);
        }
        // The right channel: the face's second voice, or the faint line.
        let (rc, rw) = face::right_line(s, &ink);
        if differ {
            canvas::trace(b, &right, rw * k, rc, &ink);
        }
        match marks {
            face::Marks::Buckets => {
                // A bucket brigade's stages: the lines thinner, beads along them.
                if differ {
                    face::beads(b, &right, 9.0 * k, 1.9 * k, rc);
                }
                canvas::trace(b, &left, 1.4 * k, ink.hue, &ink);
                face::beads(b, &left, 9.0 * k, 2.2 * k, ink.hue);
            }
            _ => canvas::trace(b, &left, 2.0 * k, ink.hue, &ink),
        }
        if let face::Marks::Crossings(v) = marks {
            face::crossings(
                b,
                &left,
                y_of(v),
                k,
                face::second_voice(s, &ink).unwrap_or(ink.text),
            );
        }
        // Where the unit is now: in the first cycle, by the telemetry's phase.
        let now = (tel[tel::BLOCKS] > 0.0).then(|| {
            let ph = tel[tel::PHASE_L].clamp(0.0, 1.0);
            let x = plot.left() + plot.width() * 0.5 * ph;
            let (a, _) = curve(f64::from(ph));
            (x, a)
        });
        if let Some((x, a)) = now {
            b.vline(x, plot.top(), plot.bottom(), 1.0 * k, ink.text);
            if marks != face::Marks::Chart {
                b.dot(pos2(x, y_of(a)), 3.5 * k, ink.text);
            }
        }
        // The ends' names, and the rate in words (their rects: the marks keep clear of them).
        let mut words = [Rect::NOTHING; 3];
        for (slot, (word, at, align)) in [
            (top, rect.top() + 3.0 * k, Align2::LEFT_TOP),
            (bottom, rect.bottom() - 3.0 * k, Align2::LEFT_BOTTOM),
        ]
        .into_iter()
        .enumerate()
        {
            // Words in capitals; a number keeps its unit as written ("6.4 ms").
            let numeric =
                word.starts_with(|c: char| c.is_ascii_digit() || "+-\u{2212}".contains(c));
            let t = Text::new(word, Role::Micro, ink.dim).caps(!numeric);
            words[slot] = canvas::put(b, pos2(rect.left() + 6.0 * k, at), align, &t, k, None);
        }
        let tempo = tel[tel::TEMPO];
        words[2] = if let Some(hz) = extras.hz {
            canvas::putf(
                b,
                pos2(rect.right() - 6.0 * k, rect.top() + 3.0 * k),
                Align2::RIGHT_TOP,
                Role::Value,
                ink.text,
                k,
                Some(ink.plate),
                format_args!("{hz:.2} Hz"),
            )
        } else if spec.get("sync") >= 0.5 {
            let i = (spec.get("div").round().max(0.0) as usize).min(DIV_NAMES.len() - 1);
            canvas::putf(
                b,
                pos2(rect.right() - 6.0 * k, rect.top() + 3.0 * k),
                Align2::RIGHT_TOP,
                Role::Value,
                ink.text,
                k,
                Some(ink.plate),
                format_args!(
                    "{} \u{b7} {:.0} {}",
                    DIV_NAMES[i],
                    if tempo > 0.0 { tempo } else { 120.0 },
                    frame::tr("modfx.bpm")
                ),
            )
        } else {
            canvas::putf(
                b,
                pos2(rect.right() - 6.0 * k, rect.top() + 3.0 * k),
                Align2::RIGHT_TOP,
                Role::Value,
                ink.text,
                k,
                Some(ink.plate),
                format_args!("{:.2} Hz", spec.get("rate")),
            )
        };
        // The rate's plate is wider than its words (`canvas::put`).
        words[2] = words[2].expand2(vec2(3.0 * k, 1.0 * k));
        match marks {
            face::Marks::Chart => {
                let pen = now.map(|(x, a)| pos2(x, y_of(a)));
                face::chart(b, plot, pen, k, &ink, ink.text, &words);
            }
            face::Marks::Tuner => {
                let below = words[2].bottom();
                face::tuner(b, plot, below, now.map(|n| n.1), k, &ink, ink.hue);
            }
            _ => {}
        }
        canvas::edge_b(b, rect, &ink);
    });
}

/// The LFO's value at `c` cycles in, left and right: the same function the DSP plays (the
/// settings, the shape, the swing, the spread) — what [`movement`] draws.
pub fn lfo_at(lfo: &cantus_dsp::modfx_lfo::LfoSettings, spread: f32, c: f64) -> (f32, f32) {
    let mut l = cantus_dsp::modfx_lfo::Lfo::new(48_000.0);
    l.set(lfo, &cantus_dsp::modfx_clock::Clock::new());
    let (p, n) = l.phase_at(c);
    let (q, m) = l.phase_at(c + f64::from(spread));
    (
        modfx_lfo::wave(lfo.shape, p, n, lfo.edge, lfo.seed),
        modfx_lfo::wave(lfo.shape, q, m, lfo.edge, lfo.seed),
    )
}

// ---- a mode card ----------------------------------------------------------------------

/// A column of modes as keys, each with a picture and its name, and what the chosen one
/// does in two lines under them. `glyph` draws mode `i` in a box.
#[allow(clippy::too_many_arguments)]
pub fn mode_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    key: &'static str,
    title: &'static str,
    names: &[&'static str],
    words: &[&'static str],
    glyph: &dyn Fn(&mut crate::kit::Brush, Rect, usize, egui::Color32),
) {
    let now = (host.spec().get(key).round().max(0.0) as usize).min(names.len() - 1);
    controls::card(ui, s, rect, Src::Key(title), None, |ui, cr| {
        let inner = cr.body;
        let k = s.k;
        let row = s.d(26.0);
        let gap = s.d(3.0);
        let mut y = inner.top();
        for (i, name) in names.iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), row));
            y += row + gap;
            if r.bottom() > inner.bottom() + 1.0 {
                break;
            }
            let resp = ui.interact(r, ui.id().with((key, i)), Sense::click());
            let st = crate::kit::State::of(&resp, false).lit(i == now);
            with_brush(ui.painter(), |b| {
                controls::paint_key(b, r, s, st, Src::Str(""));
                let g = Rect::from_min_size(
                    pos2(r.left() + 6.0 * k, r.top() + 4.0 * k),
                    vec2(r.height() - 8.0 * k, r.height() - 8.0 * k),
                );
                let ink = if i == now { s.t.text } else { s.hue };
                glyph(b, g, i, ink);
                let at = Rect::from_min_max(pos2(g.right() + 7.0 * k, r.top()), r.max);
                let t = Text::key(
                    name,
                    Role::Control,
                    if i == now { s.t.text } else { s.t.label },
                )
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, at, &t, s.tk());
            });
            let learnt = controls::stepped_offer(ui, s, host, key, frame::tr(name), &resp);
            if resp.clicked() && i != now && !learnt {
                host.set(key, i as f32, None);
            }
            if resp.hovered() {
                hint::set_text(ui.ctx(), frame::tr(name), frame::tr(words[i]), Caps::NONE);
            }
        }
        // What the chosen one does, in words.
        y += gap;
        let text = Rect::from_min_size(
            pos2(inner.left() + 2.0 * k, y),
            vec2(inner.width() - 2.0 * k, 16.0 * k),
        );
        if text.bottom() <= inner.bottom() + 1.0 {
            with_brush(ui.painter(), |b| {
                let t = Text::key(words[now], Role::Body, s.t.text_dim)
                    .caps(false)
                    .align(Align2::LEFT_TOP)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, text, &t, s.tk());
            });
        }
    });
}

/// A small meter of a level in dBFS from the telemetry: the frame's output meter.
pub fn out_meter(tel: &[f32; 64]) -> Option<frame::Levels> {
    let db = tel[tel::OUT_PEAK];
    (tel[tel::BLOCKS] > 0.0).then_some(frame::Levels {
        level: [db; 2],
        hold: [db; 2],
    })
}

/// A word in the middle of a display, on a plate: what an idle unit says ("Depth is 0 %:
/// the sound passes untouched").
pub fn idle_note(ui: &mut egui::Ui, s: &Style, rect: Rect, key: &'static str) {
    let ink = Ink::of(s);
    with_brush(&ui.painter_at(rect), |b| {
        let t = Text::key(key, Role::Body, ink.dim).caps(false);
        canvas::put(
            b,
            rect.center(),
            Align2::CENTER_CENTER,
            &t,
            s.k,
            Some(ink.plate),
        );
    });
}

/// The rate of a spec in words, for a line that is formatted without allocating: `1/8`
/// while synced, `4.00 Hz` free.
pub struct RateWords<'a>(pub &'a cantus_core::EffectSpec);

impl std::fmt::Display for RateWords<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let spec = self.0;
        if spec.kind.param("sync").is_some() && spec.get("sync") >= 0.5 {
            let i = (spec.get("div").round().max(0.0) as usize).min(DIV_NAMES.len() - 1);
            f.write_str(DIV_NAMES[i])
        } else if spec.kind.param("rate").is_some() {
            write!(f, "{:.2} Hz", spec.get("rate"))
        } else {
            Ok(())
        }
    }
}

/// The rate an inspector preview names: the spec's (its division when synced, else its
/// rate), one the unit really plays at (a Juno model's), or none (the LFO not in use).
#[derive(Copy, Clone)]
pub enum PreviewRate<'a> {
    Spec(&'a cantus_core::EffectSpec),
    Hz(f32),
    Off,
}

impl std::fmt::Display for PreviewRate<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            PreviewRate::Spec(spec) => std::fmt::Display::fmt(&RateWords(spec), f),
            PreviewRate::Hz(hz) => write!(f, "{hz:.2} Hz"),
            PreviewRate::Off => Ok(()),
        }
    }
}

/// The inspector line under a preview's picture: "mode · rate · amount", or "mode ·
/// amount" with no rate in use.
pub struct PreviewWords<'a> {
    pub mode: &'a str,
    pub rate: PreviewRate<'a>,
    pub amount: &'a dyn std::fmt::Display,
}

impl std::fmt::Display for PreviewWords<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.rate {
            PreviewRate::Off => write!(f, "{} \u{b7} {}", self.mode, self.amount),
            rate => write!(f, "{} \u{b7} {rate} \u{b7} {}", self.mode, self.amount),
        }
    }
}

/// The inspector's picture of a modulation unit: a cycle and a half of what it does in a
/// strip, and its mode, rate and amount in one line.
pub fn preview_strip(
    ui: &mut egui::Ui,
    spec: &cantus_core::EffectSpec,
    words: &PreviewWords<'_>,
    curve: &dyn Fn(f64) -> f32,
) -> egui::Response {
    use crate::design::space;
    let st = Style::current().function(frame::hue_of(spec.kind));
    let ink = Ink::of(&st);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let pic = Rect::from_min_size(rect.min, vec2(h * 2.0, h)).shrink(space::S1);
    const N: usize = 49;
    let mut pts = [Pos2::ZERO; N];
    for (j, p) in pts.iter_mut().enumerate() {
        let c = 1.5 * j as f64 / (N - 1) as f64;
        let v = curve(c).clamp(-1.0, 1.0);
        *p = pos2(
            pic.left() + pic.width() * j as f32 / (N - 1) as f32,
            pic.center().y - v * pic.height() * 0.45,
        );
    }
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        b.hline(
            pic.left(),
            pic.right(),
            pic.center().y,
            b.px(),
            ink.grid_strong,
        );
        b.polyline(&pts, 1.5, ink.hue);
        let place = Rect::from_min_max(pos2(pic.right() + space::S2, rect.top()), rect.max);
        canvas::line_in(
            b,
            place,
            Role::Body,
            ink.text,
            st.tk(),
            format_args!("{words}"),
        );
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("modfx.preview_tip"), "", Caps::NONE);
    }
    resp
}

/// The spec of the effect whose editor is open.
pub fn open_spec(app: &crate::app::CantusApp) -> Option<cantus_core::EffectSpec> {
    let (strip, effect, _) = app.fx.open?;
    app.project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .map(|e| e.spec.clone())
}

/// Something to modulate, for screenshots: a chord of saws, 128 BPM, `seconds` long, run
/// through `f` in blocks of 480 with the transport playing from the top.
pub fn demo_run(f: &mut DemoRun<'_>, seconds: f32) {
    let sr = 48_000.0f32;
    let n = (seconds * sr) as usize;
    let mut at = 0usize;
    while at < n {
        let m = 480.min(n - at);
        let mut l = vec![0.0f32; m];
        for (i, x) in l.iter_mut().enumerate() {
            let t = (at + i) as f32 / sr;
            for (h, f) in [130.8f32, 164.8, 196.0].iter().enumerate() {
                let ph = (t * f).fract();
                *x += (2.0 * ph - 1.0) * 0.12 / (h as f32 + 1.0);
            }
        }
        let mut r = l.clone();
        f(
            &mut l,
            &mut r,
            cantus_dsp::modfx_clock::Transport {
                playing: true,
                tempo_bpm: 128.0,
                samples: at as i64,
            },
        );
        at += m;
    }
}

/// A share as a percentage, for a line formatted without allocating.
pub struct Pct(pub f32);

impl std::fmt::Display for Pct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.0}%", self.0 * 100.0)
    }
}

/// A value with its unit, for a line formatted without allocating: `35 ct`, `120 Hz`.
pub struct Unit(pub f32, pub &'static str);

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.abs() < 10.0 && self.0.fract() != 0.0 {
            write!(f, "{:.1} {}", self.0, self.1)
        } else {
            write!(f, "{:.0} {}", self.0, self.1)
        }
    }
}

/// A child area at `rect`, disabled unless `enabled`: a card whose controls only matter in
/// one mode is drawn greyed rather than hidden, so the layout never jumps.
pub fn region<R>(
    ui: &mut egui::Ui,
    rect: Rect,
    enabled: bool,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.expand(4.0)));
    if !enabled {
        child.disable();
    }
    f(&mut child)
}

/// What [`demo_run`] hands the signal to: a block, and the transport at its start.
pub type DemoRun<'a> = dyn FnMut(&mut [f32], &mut [f32], cantus_dsp::modfx_clock::Transport) + 'a;

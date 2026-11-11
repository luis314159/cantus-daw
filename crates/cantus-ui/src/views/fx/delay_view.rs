//! Delay's echoes (round 39, ADR 0115): the model of what the unit plays — taken from
//! `cantus_dsp::delay` and held to it by a test that runs the real unit — the display that
//! draws them and is their control, the inspector's small strip, and the demo set-up.
//!
//! The model: the first repeat comes at the time, at the wet level, untouched by the tone;
//! each later one is `feedback` lower and has passed once more through the one-pole low-pass
//! in the feedback path. Ping-pong writes the sum of the sides into the left line and
//! crosses the feedback, so the first repeat is on the left and they alternate.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::{DawHost, FxHost};
use super::State;
use crate::app::CantusApp;
use crate::kit::color::mix;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::delay::DelaySettings;
use cantus_dsp::math;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::fmt::Write as _;

/// The frequency the tone's darkening is measured at.
pub const REF_HZ: f32 = 8_000.0;
/// The quietest repeat drawn or counted, relative to the first: −60 dB.
pub const FLOOR: f32 = 0.001;
/// At most this many repeats are drawn.
pub const MAX_ECHOES: usize = 64;
/// The sample rate the picture assumes when the host gives none.
const SR: f32 = 48_000.0;

/// Which side a repeat is heard on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Side {
    Both,
    Left,
    Right,
}

/// One repeat: when (seconds after the sound), how loud (relative to the first repeat),
/// how much of its highs are left at [`REF_HZ`] (1 = all), and where.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Echo {
    pub at: f32,
    pub level: f32,
    pub bright: f32,
    pub side: Side,
}

/// The unit's settings from its parameters, as the engine reads them
/// (`cantus_engine::effects`, `Dsp::Delay`).
pub fn settings(spec: &EffectSpec) -> DelaySettings {
    DelaySettings {
        time_ms: spec.get("time_ms"),
        feedback: spec.get("feedback"),
        tone_hz: spec.get("tone_hz"),
        ping_pong: spec.get("ping_pong") >= 0.5,
        mix: spec.get("mix"),
    }
}

/// The one-pole low-pass's gain at `f` for a tone of `tone_hz` (the unit's own filter:
/// `y += a (x − y)`, `a = 1 − e^(−2π fc / sr)`).
pub fn pass_gain(tone_hz: f32, sample_rate: f32, f: f32) -> f32 {
    let fc = tone_hz.clamp(20.0, sample_rate * 0.45);
    let a = 1.0 - math::exp(-std::f32::consts::TAU * fc / sample_rate);
    let w = std::f32::consts::TAU * f.min(sample_rate * 0.5) / sample_rate;
    let b = 1.0 - a;
    a / (1.0 - 2.0 * b * math::cos(w) + b * b).max(1e-12).sqrt()
}

/// The repeats heard within `span` seconds, loudest first, down to [`FLOOR`].
pub fn echoes(ds: &DelaySettings, sample_rate: f32, span: f32, out: &mut Vec<Echo>) {
    out.clear();
    let t = ds.time_ms.max(1.0) / 1000.0;
    let fb = ds.feedback.clamp(0.0, 0.98);
    let one = pass_gain(ds.tone_hz, sample_rate, REF_HZ);
    let (mut level, mut bright) = (1.0f32, 1.0f32);
    for n in 1..=MAX_ECHOES {
        let at = t * n as f32;
        if at > span || level < FLOOR {
            break;
        }
        let side = match (ds.ping_pong, n % 2) {
            (false, _) => Side::Both,
            (true, 1) => Side::Left,
            (true, _) => Side::Right,
        };
        out.push(Echo {
            at,
            level,
            bright,
            side,
        });
        level *= fb;
        bright *= one;
    }
}

/// How many repeats are heard before they fall under −60 dB of the first.
pub fn repeats_heard(feedback: f32) -> usize {
    let fb = feedback.clamp(0.0, 0.98);
    if fb <= 0.0 {
        return 1;
    }
    1 + (math::ln(FLOOR) / math::ln(fb)).floor() as usize
}

/// The round span of seconds the display shows for a time: about six and a half repeats.
pub fn span_for(time_ms: f32) -> f32 {
    const SPANS: [f32; 15] = [
        0.05, 0.1, 0.2, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0,
    ];
    let want = time_ms.max(1.0) / 1000.0 * 6.5;
    SPANS.into_iter().find(|s| *s >= want).unwrap_or(16.0)
}

/// The note value at `bpm` that a time of `ms` is (within half a millisecond), as its
/// index in the modulation family's divisions (`modfx.div.{i}`).
pub fn note_of(ms: f32, bpm: f64) -> Option<usize> {
    cantus_dsp::modfx_clock::DIVISIONS
        .iter()
        .position(|beats| ((beats * 60_000.0 / bpm) as f32 - ms).abs() < 0.5)
}

/// The divisions whose time at `bpm` the unit can play (1 ms … 2 s).
pub fn notes_in_range(bpm: f64) -> impl Iterator<Item = (usize, f32)> {
    cantus_dsp::modfx_clock::DIVISIONS
        .iter()
        .enumerate()
        .map(move |(i, beats)| (i, (beats * 60_000.0 / bpm) as f32))
        .filter(|(_, ms)| (1.0..=2_000.0).contains(ms))
}

/// The input's level heard lately: one peak per batch the tap delivered, stamped with the
/// sample count at its end, so a repeat's glow is the input's level its time ago.
#[derive(Debug, Default)]
pub struct Heard {
    /// (samples at the batch's end, its peak), oldest first.
    peaks: std::collections::VecDeque<(u64, f32)>,
    /// Samples taken so far.
    clock: u64,
}

impl Heard {
    /// Seconds of history kept: the longest span.
    const KEEP_S: f32 = 16.5;

    /// Take a batch of (input, output) pairs; whether any of it was above −90 dB.
    pub fn push(&mut self, pairs: &[(f32, f32)], sample_rate: f32) -> bool {
        if pairs.is_empty() {
            return false;
        }
        let peak = pairs.iter().fold(0.0f32, |m, (i, _)| m.max(i.abs()));
        self.clock += pairs.len() as u64;
        self.peaks.push_back((self.clock, peak));
        let keep = (Self::KEEP_S * sample_rate) as u64;
        while self
            .peaks
            .front()
            .is_some_and(|(at, _)| self.clock.saturating_sub(*at) > keep)
        {
            self.peaks.pop_front();
        }
        peak > 3.2e-5
    }

    /// The input's peak `ago` samples back (0 when nothing was heard then).
    pub fn at(&self, ago: u64) -> f32 {
        let Some(when) = self.clock.checked_sub(ago) else {
            return 0.0;
        };
        // The batch that holds `when`: the first that ends at or after it.
        let i = self.peaks.partition_point(|(end, _)| *end < when);
        match self.peaks.get(i) {
            Some((end, p)) if *end >= when => {
                let start = if i == 0 { 0 } else { self.peaks[i - 1].0 };
                if when > start || i == 0 {
                    *p
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    /// Forget what was heard (another effect, a reset).
    pub fn clear(&mut self) {
        self.peaks.clear();
        self.clock = 0;
    }
}

/// What a drag on the display began from: the time and feedback then, the lane it began
/// in (up is more feedback in the upper lane, down in the lower), the span it holds still.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Drag {
    from: Pos2,
    time_ms: f32,
    feedback: f32,
    lane: f32,
    span: f32,
    gesture: u64,
}

/// The display's plot inside its rect: the lanes, the time axis.
#[derive(Copy, Clone, Debug)]
pub struct Plot {
    pub rect: Rect,
    pub span: f32,
}

impl Plot {
    pub fn of(d: Rect, k: f32, span: f32) -> Plot {
        Plot {
            rect: Rect::from_min_max(
                d.min + vec2(26.0 * k, 28.0 * k),
                d.max - vec2(12.0 * k, 20.0 * k),
            ),
            span,
        }
    }

    pub fn x(&self, t: f32) -> f32 {
        self.rect.left() + self.rect.width() * (t / self.span).clamp(0.0, 1.0)
    }

    pub fn t(&self, x: f32) -> f32 {
        (x - self.rect.left()) / self.rect.width().max(1.0) * self.span
    }

    pub fn mid(&self) -> f32 {
        self.rect.center().y
    }

    pub fn half(&self) -> f32 {
        self.rect.height() * 0.5
    }

    /// A level's height from the centre: its square root, so a quiet repeat still shows.
    pub fn h(&self, level: f32) -> f32 {
        self.half() * level.clamp(0.0, 1.0).sqrt()
    }
}

/// The highest feedback the parameter takes.
const FB_MAX: f32 = 0.95;

/// The drag's values for the pointer at `p`: the time across (the first repeat follows the
/// pointer), the feedback up or down in the lane the drag began in. `fine`: a fifth.
pub fn dragged_to(d: &Drag, plot: &Plot, p: Pos2, fine: bool) -> (f32, f32) {
    let rate = if fine { 0.2 } else { 1.0 };
    let x0 = plot.x(d.time_ms / 1000.0);
    let x = x0 + (p.x - d.from.x) * rate;
    let ms = (plot.t(x) * 1000.0).clamp(1.0, 2_000.0);
    let ms = (ms * 10.0).round() / 10.0;
    let y0 = d.feedback.clamp(0.0, FB_MAX).sqrt();
    let y = y0 + d.lane * (d.from.y - p.y) / plot.half().max(1.0) * rate;
    let y = y.clamp(0.0, FB_MAX.sqrt());
    let fb = y * y;
    (ms, (fb * 1000.0).round() / 1000.0)
}

/// The display: the echoes drawn in time, and the control of the time and the feedback.
/// Returns whether anything on it moves with the sound (a repeat glowing).
pub fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    d: Rect,
) -> bool {
    let k = s.k;
    let spec = host.spec().clone();
    let ds = settings(&spec);
    let sr = host.sample_rate().max(1.0);
    let span = st.drag.map_or_else(|| span_for(ds.time_ms), |g| g.span);
    let plot = Plot::of(d, k, span);
    // The control first, so the picture shows where the drag has got to.
    let resp = ui.interact(d, ui.id().with("delay-echoes"), Sense::click_and_drag());
    if resp.drag_started() {
        let from = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos());
        if let Some(from) = from {
            st.gesture += 1;
            st.drag = Some(Drag {
                from,
                time_ms: ds.time_ms,
                feedback: ds.feedback,
                lane: if from.y <= plot.mid() { 1.0 } else { -1.0 },
                span,
                gesture: st.gesture,
            });
        }
    }
    if let (Some(g), Some(p), true) = (st.drag, resp.interact_pointer_pos(), resp.dragged()) {
        let fine = ui.input(|i| i.modifiers.shift);
        let (ms, fb) = dragged_to(&g, &plot, p, fine);
        if ms != ds.time_ms || fb != ds.feedback {
            host.apply(
                &[("time_ms", Some(ms)), ("feedback", Some(fb))],
                "cmd.delay_echoes",
                Some(g.gesture),
            );
        }
    }
    if resp.drag_stopped() && st.drag.take().is_some() {
        host.end_gesture();
    }
    if resp.double_clicked() {
        let def = |key: &str| spec.kind.param(key).map_or(0.0, |i| i.default);
        host.apply(
            &[
                ("time_ms", Some(def("time_ms"))),
                ("feedback", Some(def("feedback"))),
            ],
            "cmd.delay_echoes",
            None,
        );
    }
    if resp.hovered() || st.drag.is_some() {
        hint::set_text(
            ui.ctx(),
            frame::tr("delay.display_tip"),
            frame::tr("delay.display_how"),
            Caps::NONE,
        );
    }
    // What the drag set this frame is what is drawn.
    let ds = settings(host.spec());
    let bpm = bpm_of(host);
    paint(ui, st, s, d, &plot, &ds, sr, bpm)
}

/// The song's tempo where the host is.
pub fn bpm_of(host: &dyn FxHost) -> f64 {
    host.transport().ticks_per_second * 60.0 / cantus_core::PPQ as f64
}

/// A repeat's glow for a level heard: 0 under −48 dB, 1 at full scale.
fn glow_of(level: f32) -> f32 {
    if level <= 0.0 {
        return 0.0;
    }
    ((20.0 * math::log10(level) + 48.0) / 48.0).clamp(0.0, 1.0)
}

/// Paint the echoes. Returns whether a repeat glows with the sound now.
#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &mut egui::Ui,
    st: &mut State,
    s: &Style,
    d: Rect,
    plot: &Plot,
    ds: &DelaySettings,
    sr: f32,
    bpm: f64,
) -> bool {
    let k = s.k;
    let ink = Ink::of(s);
    let bright = ink.hue;
    // A repeat with no highs left: the second voice, a third of the way into the ground.
    let dark = mix(ink.voice(s.hue, true), ink.ground, 0.3);
    let mut list = std::mem::take(&mut st.echoes);
    echoes(ds, sr, plot.span, &mut list);
    let (mid, r) = (plot.mid(), plot.rect);
    let fb = ds.feedback.clamp(0.0, FB_MAX);
    let t = ds.time_ms.max(1.0) / 1000.0;
    let mut live = false;
    // The words on the plates, made when what they say changes.
    let key = (
        ds.time_ms.to_bits(),
        bpm.to_bits(),
        ds.ping_pong,
        crate::i18n::locale(),
    );
    if st.words.as_ref().is_none_or(|w| w.0 != key) {
        let mut time = String::new();
        super::super::controls::write_readout(ds.time_ms, "ms", &mut time);
        if let Some(i) = note_of(ds.time_ms, bpm) {
            let _ = write!(time, " \u{b7} {}", frame::tr(DIV_KEYS[i]));
        }
        let side = frame::tr(if ds.ping_pong {
            "delay.side.ping_pong"
        } else {
            "delay.side.both"
        });
        st.words = Some((key, time, side.to_string()));
    }
    with_brush(&ui.painter_at(d), |b| {
        canvas::ground_b(b, d, &ink, s.hue_tok);
        // The lanes: the centre, a quarter and the whole of each side's level.
        b.hline(
            r.left(),
            r.right(),
            b.snap_mid(mid),
            b.px(),
            ink.grid_strong,
        );
        for lvl in [0.25f32, 1.0] {
            for sg in [-1.0f32, 1.0] {
                let y = b.snap_mid(mid + sg * plot.h(lvl));
                b.hline(r.left(), r.right(), y, b.px(), ink.grid);
            }
        }
        // The time under the lanes, in sixths of the span.
        for q in 0..=6 {
            let at = plot.span * q as f32 / 6.0;
            let x = plot.x(at);
            b.segment(
                pos2(x, r.bottom()),
                pos2(x, r.bottom() + 3.0 * k),
                b.px(),
                ink.grid_strong,
                false,
            );
            let mut label = String::new();
            super::super::controls::write_readout(at, "s", &mut label);
            let tx = Text::new(&label, Role::Micro, ink.dim).caps(false);
            let align = match q {
                0 => Align2::LEFT_TOP,
                6 => Align2::RIGHT_TOP,
                _ => Align2::CENTER_TOP,
            };
            canvas::put(b, pos2(x, r.bottom() + 4.0 * k), align, &tx, k, None);
        }
        for (key, sg) in [("delay.lane.left", -1.0f32), ("delay.lane.right", 1.0)] {
            let tx = Text::key(key, Role::Micro, ink.dim).caps(true);
            let at = pos2(d.left() + 12.0 * k, mid + sg * plot.half() * 0.5);
            canvas::put(b, at, Align2::CENTER_CENTER, &tx, k, None);
        }
        // The sound itself, at 0.
        b.segment(
            pos2(r.left(), mid - plot.half()),
            pos2(r.left(), mid + plot.half()),
            1.2 * k,
            ink.dim,
            false,
        );
        // The decay through the repeats' heads, in each lane.
        if fb > 0.0 && plot.span > t {
            let n = 48;
            for sg in [-1.0f32, 1.0] {
                let pts: Vec<Pos2> = (0..=n)
                    .map(|i| {
                        let at = t + (plot.span - t) * i as f32 / n as f32;
                        let lvl = math::powf(fb, at / t - 1.0);
                        pos2(plot.x(at), mid + sg * plot.h(lvl))
                    })
                    .collect();
                b.dashed(&pts, 1.0 * k, Ink::a(bright, 0.45), 3.0 * k, 3.0 * k);
            }
        }
        // The feedback's level in each lane, from the first repeat on.
        let fy = plot.h(fb);
        for sg in [-1.0f32, 1.0] {
            let y = mid + sg * fy;
            b.dashed(
                &[pos2(plot.x(t), y), pos2(r.right(), y)],
                1.0 * k,
                Ink::a(bright, 0.8),
                6.0 * k,
                4.0 * k,
            );
        }
        // The repeats: a stem from the centre, its colour as the tone left it.
        for e in &list {
            let c = mix(dark, bright, e.bright);
            let x = plot.x(e.at);
            let ago = (e.at * sr) as u64;
            let heard = glow_of(st.heard.at(ago) * e.level * ds.mix.max(0.05));
            live |= heard > 0.0;
            for (sg, on) in [
                (-1.0f32, e.side != Side::Right),
                (1.0, e.side != Side::Left),
            ] {
                if !on {
                    continue;
                }
                let head = pos2(x, mid + sg * plot.h(e.level));
                if heard > 0.0 {
                    b.dot(head, 8.0 * k, Ink::a(c, 0.15 + 0.45 * heard));
                }
                b.segment(pos2(x, mid), head, 3.0 * k, c, false);
                // The head is the repeat's air: it shrinks as the tone takes the highs.
                b.dot(
                    head,
                    (1.8 + 2.2 * e.bright) * k,
                    mix(c, ink.text, 0.25 * e.bright),
                );
            }
        }
        // The handle: the first repeat's time at the feedback's level, in both lanes.
        let hot = st.drag.is_some();
        for sg in [-1.0f32, 1.0] {
            let at = pos2(plot.x(t), mid + sg * fy);
            if hot {
                b.dot(at, 6.5 * k, ink.plate);
            }
            b.circle_stroke(at, 6.5 * k, 1.6 * k, ink.text);
        }
        // The words: the time (and its note), the sides, the feedback by its line.
        if let Some((_, time, side)) = &st.words {
            let tx = Text::new(time, Role::Value, ink.text);
            canvas::put(
                b,
                d.min + vec2(8.0 * k, 6.0 * k),
                Align2::LEFT_TOP,
                &tx,
                k,
                Some(ink.plate),
            );
            let tx = Text::new(side, Role::Micro, ink.dim).caps(true);
            canvas::put(
                b,
                pos2(d.right() - 8.0 * k, d.top() + 7.0 * k),
                Align2::RIGHT_TOP,
                &tx,
                k,
                Some(ink.plate),
            );
        }
        let ly = mid - fy;
        let (at, align) = if ly - r.top() < 18.0 * k {
            (pos2(r.right() - 2.0 * k, ly + 4.0 * k), Align2::RIGHT_TOP)
        } else {
            (
                pos2(r.right() - 2.0 * k, ly - 4.0 * k),
                Align2::RIGHT_BOTTOM,
            )
        };
        canvas::tag(
            b,
            at,
            align,
            frame::tr("delay.feedback"),
            ink.text,
            k,
            Some(ink.plate),
            format_args!("{:.0}%", fb * 100.0),
        );
        canvas::edge_b(b, d, &ink);
    });
    st.echoes = list;
    if ds.mix <= 0.0 {
        super::super::modfx::idle_note(ui, s, d, "delay.idle");
    }
    live
}

/// The modulation family's names of the note values (`cantus_dsp::modfx_clock::DIVISIONS`).
pub const DIV_KEYS: [&str; 17] = [
    "modfx.div.0",
    "modfx.div.1",
    "modfx.div.2",
    "modfx.div.3",
    "modfx.div.4",
    "modfx.div.5",
    "modfx.div.6",
    "modfx.div.7",
    "modfx.div.8",
    "modfx.div.9",
    "modfx.div.10",
    "modfx.div.11",
    "modfx.div.12",
    "modfx.div.13",
    "modfx.div.14",
    "modfx.div.15",
    "modfx.div.16",
];

/// The inspector's picture: the repeats in a strip, with the time and the feedback.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Delay));
    let ink = Ink::of(&st);
    let k = st.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0 * k), Sense::click());
    let ds = settings(spec);
    let span = span_for(ds.time_ms);
    let mut list = Vec::with_capacity(16);
    echoes(&ds, SR, span, &mut list);
    let plot = Plot {
        rect: rect.shrink2(vec2(8.0 * k, 6.0 * k)),
        span,
    };
    let (bright, dark) = (ink.hue, ink.voice(st.hue, true));
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        let mid = plot.mid();
        b.hline(
            plot.rect.left(),
            plot.rect.right(),
            b.snap_mid(mid),
            b.px(),
            ink.grid,
        );
        for e in &list {
            let c = mix(dark, bright, e.bright);
            let x = plot.x(e.at);
            for (sg, on) in [
                (-1.0f32, e.side != Side::Right),
                (1.0, e.side != Side::Left),
            ] {
                if on {
                    b.segment(
                        pos2(x, mid),
                        pos2(x, mid + sg * plot.h(e.level)),
                        2.0 * k,
                        c,
                        false,
                    );
                }
            }
        }
        canvas::putf(
            b,
            rect.min + vec2(6.0 * k, 4.0 * k),
            Align2::LEFT_TOP,
            Role::Label,
            ink.text,
            k,
            Some(ink.plate),
            format_args!("{:.0} ms \u{b7} {:.0}%", ds.time_ms, ds.feedback * 100.0),
        );
        canvas::edge_b(b, rect, &ink);
    });
    resp
}

/// The settings `--editor delay` and the screenshot tool open it with: its *Dotted eighth*
/// preset (dotted eighths at 120 BPM, bouncing from side to side).
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let values = super::super::factory_presets(EffectKind::Delay)
        .iter()
        .find(|(k, _)| *k == "delay.preset.dotted_eighth")
        .map(|(_, v)| *v)
        .unwrap_or(&[
            ("time_ms", 375.0),
            ("feedback", 0.42),
            ("tone_hz", 4_500.0),
            ("ping_pong", 1.0),
            ("mix", 0.2),
        ]);
    DawHost::new(app, strip, effect, "delay").set_many(values, "cmd.fx_preset");
}

/// What a playing engine would have fed the editor, for screenshots with no engine: a
/// phrase's four eighth notes, the tap's batches as the window took them frame by frame,
/// the last note a moment ago — so the repeats glow as they leave.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((_, effect, _)) = app.fx.open else {
        return;
    };
    let st = &mut app.fx.delay;
    st.effect = Some(effect);
    st.heard.clear();
    let n = (SR * 1.15) as usize;
    let batch = (SR / 60.0) as usize;
    let mut pairs = Vec::with_capacity(batch);
    for i in 0..n {
        let t = i as f32 / SR;
        let note = (t / 0.25).floor();
        let x = t - note * 0.25;
        let v = if note < 4.0 {
            0.7 * math::exp(-x * 18.0) * math::sin(std::f32::consts::TAU * 330.0 * t)
        } else {
            0.0
        };
        pairs.push((v, v));
        if pairs.len() == batch {
            st.heard.push(&pairs, SR);
            pairs.clear();
        }
    }
    st.heard.push(&pairs, SR);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::{EffectId, EffectSlot, EffectSpec};

    /// The real unit's impulse response, through the engine's own wiring of the slot.
    fn impulse_response(values: &[(&str, f32)], seconds: f32) -> (Vec<f32>, Vec<f32>) {
        let mut spec = EffectSpec::new(EffectKind::Delay);
        for (k, v) in values {
            spec.set(k, *v);
        }
        let slot = EffectSlot {
            id: EffectId(1),
            bypass: false,
            spec,
            mix: 1.0,
            sidechain: None,
            midi_from: None,
        };
        let mut unit = cantus_engine::EffectUnit::new(&slot, SR);
        let n = (seconds * SR) as usize;
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        l[0] = 1.0;
        r[0] = 1.0;
        unit.process(&mut l, &mut r);
        (l, r)
    }

    /// The sum of the samples around `at` (a repeat's DC gain: the tone's low-pass passes it
    /// whole) and the magnitude of its 8 kHz component.
    fn measure(x: &[f32], at: f32, half: f32) -> (f64, f64) {
        let a = ((at - half) * SR) as usize;
        let b = (((at + half) * SR) as usize).min(x.len());
        let w = std::f64::consts::TAU * REF_HZ as f64 / SR as f64;
        let (mut dc, mut re, mut im) = (0.0f64, 0.0f64, 0.0f64);
        for (i, v) in x[a..b].iter().enumerate() {
            let v = *v as f64;
            dc += v;
            let (s, c) = math::sin_cos(w * i as f64);
            re += v * c;
            im -= v * s;
        }
        (dc, (re * re + im * im).sqrt())
    }

    /// The display draws what the unit plays: every repeat the model lists is in the real
    /// unit's output at its time, on its side, at its level (relative to the first) and
    /// with what is left of its highs at 8 kHz; nothing is on the other side.
    #[test]
    fn the_echoes_drawn_are_the_units_own() {
        for ping in [0.0f32, 1.0] {
            let values = [
                ("time_ms", 100.0),
                ("feedback", 0.5),
                ("tone_hz", 4_000.0),
                ("ping_pong", ping),
                ("mix", 1.0),
            ];
            let mut spec = EffectSpec::new(EffectKind::Delay);
            for (k, v) in values {
                spec.set(k, v);
            }
            let mut list = Vec::new();
            echoes(&settings(&spec), SR, 0.65, &mut list);
            assert_eq!(list.len(), 6, "six repeats in 0.65 s");
            let (l, r) = impulse_response(&values, 0.7);
            // Ping-pong hears the sides' mean: an impulse on both is a whole one.
            let gain = 1.0;
            let first = |x: &[f32]| measure(x, list[0].at, 0.04);
            let (f_dc, f_hi) = match list[0].side {
                Side::Right => first(&r),
                _ => first(&l),
            };
            assert!(
                (f_dc - gain).abs() < 1e-3,
                "the first repeat is the input: {f_dc}"
            );
            for e in &list {
                let (on, off) = match e.side {
                    Side::Left => (&l, Some(&r)),
                    Side::Right => (&r, Some(&l)),
                    Side::Both => (&l, None),
                };
                let (dc, hi) = measure(on, e.at, 0.04);
                assert!(
                    (dc / f_dc - e.level as f64).abs() < 1e-3,
                    "ping {ping}: the repeat at {} s is at {} of the first, drawn at {}",
                    e.at,
                    dc / f_dc,
                    e.level
                );
                let want = (e.level * e.bright) as f64;
                assert!(
                    (hi / f_hi - want).abs() < 0.02 * want.max(0.05),
                    "ping {ping}: at {} s, 8 kHz is at {} of the first's, drawn {want}",
                    e.at,
                    hi / f_hi
                );
                if let Some(off) = off {
                    let (odc, _) = measure(off, e.at, 0.04);
                    assert!(odc.abs() < 1e-6, "nothing on the other side at {} s", e.at);
                }
            }
            if ping > 0.5 {
                assert_eq!(list[0].side, Side::Left, "ping-pong starts on the left");
                assert_eq!(list[1].side, Side::Right);
            } else {
                let (dc, _) = measure(&r, list[0].at, 0.04);
                assert!((dc - f_dc).abs() < 1e-6, "both sides repeat alike");
            }
        }
    }

    #[test]
    fn the_counts_and_the_span() {
        assert_eq!(repeats_heard(0.5), 10);
        assert!(0.5f32.powi(9) >= FLOOR && 0.5f32.powi(10) < FLOOR);
        assert_eq!(repeats_heard(0.0), 1);
        assert_eq!(span_for(375.0), 3.0);
        assert_eq!(span_for(95.0), 0.75);
        assert_eq!(span_for(2_000.0), 16.0);
        // Dotted eighths at 120 BPM; nothing longer than 2 s offered.
        assert_eq!(note_of(375.0, 120.0), Some(9));
        assert_eq!(note_of(380.0, 120.0), None);
        assert!(notes_in_range(120.0).all(|(_, ms)| ms <= 2_000.0));
        assert_eq!(notes_in_range(120.0).count(), 14, "down from 1/1");
        // The tone leaves the highs at its own frequency's corner.
        assert!(pass_gain(20_000.0, SR, REF_HZ) > 0.9);
        assert!(pass_gain(1_000.0, SR, REF_HZ) < 0.2);
    }

    #[test]
    fn a_drag_sets_the_time_across_and_the_feedback_up_and_down() {
        let plot = Plot {
            rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 200.0)),
            span: 3.0,
        };
        let from = pos2(plot.x(0.375), 40.0);
        let up = Drag {
            from,
            time_ms: 375.0,
            feedback: 0.25,
            lane: 1.0,
            span: 3.0,
            gesture: 1,
        };
        // Across by a tenth of a second's width; up by a tenth of a lane.
        let (ms, fb) = dragged_to(&up, &plot, from + vec2(20.0, -10.0), false);
        assert!((ms - 475.0).abs() < 0.11, "{ms}");
        assert!((fb - 0.36).abs() < 1e-3, "{fb}: sqrt 0.5 + 0.1 squared");
        // In the lower lane, down is more.
        let down = Drag { lane: -1.0, ..up };
        let (_, fb2) = dragged_to(&down, &plot, from + vec2(0.0, 10.0), false);
        assert!((fb2 - 0.36).abs() < 1e-3, "{fb2}");
        // Shift: a fifth.
        let (ms3, _) = dragged_to(&up, &plot, from + vec2(20.0, 0.0), true);
        assert!((ms3 - 395.0).abs() < 0.11, "{ms3}");
        // Held in range.
        let (ms4, fb4) = dragged_to(&up, &plot, from + vec2(-900.0, -900.0), false);
        assert_eq!((ms4, fb4), (1.0, 0.95));
    }

    #[test]
    fn what_was_heard_is_read_back_by_its_age() {
        let mut h = Heard::default();
        assert!(!h.push(&[], SR));
        assert!(h.push(&vec![(0.5, 0.0); 800], SR));
        assert!(!h.push(&vec![(0.0, 0.0); 800], SR));
        assert_eq!(h.at(0), 0.0);
        assert_eq!(h.at(900), 0.5);
        assert_eq!(h.at(1_600), 0.5);
        assert_eq!(h.at(1_601), 0.0);
        h.clear();
        assert_eq!(h.at(10), 0.0);
    }
}

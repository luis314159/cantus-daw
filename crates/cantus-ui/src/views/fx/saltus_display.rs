//! Saltus's display (ADR 0135, provisional): **the leap**. A keyboard across a dark
//! vacuum-fluorescent screen; the note heard lit on it and named large; from it an arc to
//! each voice, up or down by its interval, as thick as the voice is loud, labelled with the
//! interval's name and the note it lands on — the landing dragged to another key to change
//! the interval. Harmony marks the scale's notes (or the chord held); Octave draws its three
//! octaves; Whammy sweeps one arc with the pedal drawn beside the keyboard. The four modes are
//! lit legends along the top. Calm at rest: the last note heard stays, dimmed, with the arcs
//! the settings would make from it.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::pedal;
use super::words;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Style};
use cantus_dsp::saltus::voice::degree_step;
use cantus_dsp::saltus::{HarmonySource, Mode, OctaveStyle, Path, SaltusSettings};
use cantus_engine::fx::saltus::Reading;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// The note a display starts from before it has heard one: A3.
const START: f32 = 57.0;
/// How long the last note heard stays lit after it ends, seconds.
const FADE_S: f32 = 1.2;

/// What the display remembers between frames: every value eased toward its target.
#[derive(Debug, Default)]
pub struct View {
    started: bool,
    /// The note the arcs leap from (MIDI), and how lit it is (0 … 1).
    pub anchor: f32,
    pub heard: f32,
    /// Each lane's interval (semitones) and how much it sounds (0 … 1).
    pub shift: [f32; 3],
    pub lit: [f32; 3],
    /// The whammy's pedal as followed.
    pub pedal: f32,
    /// The analog octave's live levels.
    pub octaves: [f32; 3],
    held: u16,
    moving: bool,
    /// A landing being dragged: the voice.
    drag: Option<usize>,
    /// The last seconds of pitch, newest last: the note heard and each lane's landing
    /// (NaN where silent), one frame every [`TRAIL_DT`] (two and a half seconds).
    trail: std::collections::VecDeque<[f32; 4]>,
    since: f32,
}

/// The trail's frames: one every this many seconds, this many kept.
const TRAIL_DT: f32 = 1.0 / 40.0;
const TRAIL: usize = 100;

/// A lane of the leap: its interval, its weight (the level set), whether it sounds now, and
/// which voice's colour it wears.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Lane {
    pub interval: f32,
    pub level: f32,
    pub second: bool,
    /// The key it edits when its landing is dragged (Shift's intervals).
    pub key: Option<&'static str>,
    /// Its name: "One down" in Octave, none for a voice (its interval names it).
    pub name: Option<&'static str>,
}

/// The lanes `ss` draws from `anchor`: the two voices, the octaves, or the whammy's one.
pub fn lanes(ss: &SaltusSettings, anchor: f32, pedal: f32, held: u16) -> Vec<Lane> {
    let voice = |i: usize, interval: f32, key| Lane {
        interval,
        level: ss.voices[i].level,
        second: i == 1,
        key,
        name: None,
    };
    let n = anchor.round() as i32;
    match ss.mode {
        Mode::Shift => (0..2)
            .map(|i| {
                let v = ss.voices[i];
                let key = cantus_engine::fx::saltus::VOICE_KEYS[i][0];
                voice(i, v.semitones + v.cents / 100.0, Some(key))
            })
            .collect(),
        Mode::Harmony => {
            let mut out = Vec::new();
            for i in 0..2 {
                let v = ss.voices[i];
                // As the engine counts: the note heard snapped to the scale (or the chord),
                // the voice that many degrees from it, moved by the semitones between.
                let t = match ss.harmony_source {
                    HarmonySource::Scale => {
                        let key = ss.key as i32 % 12;
                        let m = snap(n, key, ss.scale.degrees());
                        Some(degree_step(m, v.degree, key, ss.scale.degrees()) - m + n)
                    }
                    HarmonySource::Chord => {
                        let degrees: Vec<u8> = (0..12u8).filter(|c| held & (1 << c) != 0).collect();
                        (!degrees.is_empty()).then(|| {
                            let m = snap(n, 0, &degrees);
                            degree_step(m, v.degree, 0, &degrees) - m + n
                        })
                    }
                    // The melody's voices sing the notes held: only the engine knows which.
                    HarmonySource::Melody => None,
                };
                if let Some(t) = t {
                    out.push(voice(i, (t - n) as f32 + v.cents / 100.0, None));
                }
            }
            out
        }
        Mode::Octave => [
            (ss.sub2, -24.0, "saltus.l.down2", false),
            (ss.sub1, -12.0, "saltus.l.down1", false),
            (ss.up1, 12.0, "saltus.l.up1", true),
        ]
        .into_iter()
        .map(|(level, interval, name, second)| Lane {
            interval,
            level,
            second,
            key: None,
            name: Some(name),
        })
        .collect(),
        Mode::Whammy => vec![Lane {
            interval: cantus_dsp::saltus::whammy_interval(ss, pedal),
            level: ss.voices[0].level,
            second: false,
            key: None,
            name: None,
        }],
    }
}

/// The note of the scale `degrees` (pitch classes from `key`) nearest `n`, the lower on a
/// tie.
fn snap(n: i32, key: i32, degrees: &[u8]) -> i32 {
    (0..=6)
        .flat_map(|d| [n - d, n + d])
        .find(|m| degrees.contains(&((m - key).rem_euclid(12) as u8)))
        .unwrap_or(n)
}

/// `x` moved toward `to` by `k` of the way, landed when within `snap`; whether it moved.
fn ease(x: &mut f32, to: f32, k: f32, snap: f32) -> bool {
    if (*x - to).abs() <= snap {
        let moved = *x != to;
        *x = to;
        moved
    } else {
        *x += (to - *x) * k;
        true
    }
}

impl View {
    /// The newest telemetry into the eased values, `dt` seconds after the last frame.
    pub fn update(&mut self, r: &Reading, ss: &SaltusSettings, dt: f32) {
        if !self.started {
            self.seed(r, ss);
            return;
        }
        let k = 1.0 - (-dt / 0.06).exp();
        let mut moving = false;
        if let Some(n) = r.note {
            moving |= ease(&mut self.anchor, n, k, 0.002);
            moving |= ease(&mut self.heard, 1.0, (k * 2.0).min(1.0), 0.01);
        } else if self.heard > 0.0 {
            self.heard = (self.heard - dt / FADE_S).max(0.0);
            moving = true;
        }
        moving |= ease(&mut self.pedal, r.pedal, k, 1e-4);
        for i in 0..3 {
            let (to, on) = match r.shifts[i] {
                Some(s) => (s, 1.0),
                None => (self.shift[i], 0.0),
            };
            moving |= ease(&mut self.shift[i], to, k, 0.002);
            moving |= ease(&mut self.lit[i], on, k, 0.01);
            moving |= ease(&mut self.octaves[i], r.octaves[i].clamp(0.0, 1.0), k, 0.005);
        }
        self.held = r.held;
        let _ = ss;
        // The trail: what was heard and where each voice landed, at a fixed rate.
        self.since += dt;
        while self.since >= TRAIL_DT {
            self.since -= TRAIL_DT;
            let mut f = [f32::NAN; 4];
            if let Some(n) = r.note {
                f[0] = n;
                for i in 0..3 {
                    if let Some(sh) = r.shifts[i] {
                        f[1 + i] = n + sh;
                    }
                }
            }
            self.push(f);
            moving = true;
        }
        self.moving = moving || self.trail.iter().any(|f| !f[0].is_nan());
    }

    fn push(&mut self, f: [f32; 4]) {
        if self.trail.len() == TRAIL {
            self.trail.pop_front();
        }
        self.trail.push_back(f);
    }

    /// The values at their targets at once: a picture with no engine, a window opened.
    pub fn seed(&mut self, r: &Reading, ss: &SaltusSettings) {
        self.started = true;
        self.anchor = r.note.unwrap_or(if self.anchor > 0.0 {
            self.anchor
        } else {
            START
        });
        self.heard = if r.note.is_some() { 1.0 } else { 0.0 };
        self.pedal = r.pedal;
        for i in 0..3 {
            if let Some(s) = r.shifts[i] {
                self.shift[i] = s;
            }
            self.lit[i] = f32::from(u8::from(r.shifts[i].is_some()));
            self.octaves[i] = r.octaves[i].clamp(0.0, 1.0);
        }
        self.held = r.held;
        self.moving = false;
        // A sung phrase leading to the note heard, for a picture with no engine: the note a
        // tone below, a breath, then the note with its vibrato.
        self.trail.clear();
        if r.note.is_some() {
            let a = self.anchor;
            for i in 0..TRAIL {
                let t = i as f32 * TRAIL_DT;
                let (n, on) = if t < 0.9 {
                    (a - 2.0 + 0.1 * (t * 31.0).sin(), true)
                } else if t < 1.1 {
                    (a, false)
                } else {
                    let rise = ((t - 1.1) / 0.12).min(1.0);
                    (a - 0.6 * (1.0 - rise) + 0.08 * ((t - 1.1) * 34.5).sin() * rise, true)
                };
                let mut f = [f32::NAN; 4];
                if on {
                    f[0] = n;
                    for (j, l) in lanes(ss, n, r.pedal, r.held).iter().enumerate().take(3) {
                        if l.level > 0.0 {
                            f[1 + j] = n + l.interval;
                        }
                    }
                }
                self.push(f);
            }
        }
    }

    /// Whether anything is still easing (the window asks for another frame).
    pub fn moving(&self) -> bool {
        self.moving || self.drag.is_some()
    }

    /// The pitch classes held on MIDI.
    pub fn held(&self) -> u16 {
        self.held
    }
}

/// The display's parts: the legends' band, the leap, and the pedal beside it in Whammy.
pub struct Parts {
    pub band: Rect,
    pub leap: Rect,
    pub keys: Rect,
    pub pedal: Option<Rect>,
}

pub fn parts(r: Rect, k: f32, whammy: bool) -> Parts {
    let band = Rect::from_min_size(r.min, vec2(r.width(), 34.0 * k));
    let body = Rect::from_min_max(pos2(r.left(), band.bottom()), r.max);
    let (leap, pedal) = if whammy {
        let w = (r.width() * 0.30).clamp(260.0 * k, 380.0 * k);
        let p = Rect::from_min_max(pos2(body.right() - w, body.top()), body.max);
        let l = Rect::from_min_max(body.min, pos2(p.left(), body.bottom()));
        (l, Some(p))
    } else {
        (body, None)
    };
    let inset = 18.0 * k;
    let kh = (leap.height() * 0.24).clamp(54.0 * k, 84.0 * k);
    let keys = Rect::from_min_max(
        pos2(leap.left() + inset, leap.bottom() - kh - 30.0 * k),
        pos2(leap.right() - inset, leap.bottom() - 30.0 * k),
    );
    Parts {
        band,
        leap,
        keys,
        pedal,
    }
}

/// The keyboard's span: whole octaves (C to B) holding the note and every landing, three
/// octaves at least — it moves by octaves only, so a vibrato never shakes it.
pub fn span(anchor: f32, lanes: &[Lane]) -> (i32, i32) {
    let a = anchor.round() as i32;
    let (mut lo, mut hi) = (a, a);
    for l in lanes {
        let t = (anchor + l.interval).round() as i32;
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let mut low = (lo - 2).div_euclid(12) * 12;
    let mut high = (hi + 2).div_euclid(12) * 12 + 11;
    let mut up = true;
    while high - low + 1 < 36 {
        if up {
            high += 12;
        } else {
            low -= 12;
        }
        up = !up;
    }
    (low.max(0), high.min(127))
}

fn is_black(m: i32) -> bool {
    matches!(m.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

/// Where each key of `low ..= high` is across `r`.
pub struct Keyboard {
    pub low: i32,
    pub high: i32,
    pub r: Rect,
    white: f32,
}

impl Keyboard {
    pub fn new(r: Rect, low: i32, high: i32) -> Keyboard {
        let whites = (low..=high).filter(|m| !is_black(*m)).count().max(1);
        Keyboard {
            low,
            high,
            r,
            white: r.width() / whites as f32,
        }
    }

    /// The white keys before `m`.
    fn whites_before(&self, m: i32) -> f32 {
        (self.low..m).filter(|x| !is_black(*x)).count() as f32
    }

    /// The rect of key `m`.
    pub fn key(&self, m: i32) -> Rect {
        let x = self.r.left() + self.whites_before(m) * self.white;
        if is_black(m) {
            let w = self.white * 0.62;
            Rect::from_min_size(
                pos2(x - w * 0.5, self.r.top()),
                vec2(w, self.r.height() * 0.6),
            )
        } else {
            Rect::from_min_size(pos2(x, self.r.top()), vec2(self.white, self.r.height()))
        }
    }

    /// The centre of a (fractional) note along the keyboard's top.
    pub fn x(&self, note: f32) -> f32 {
        let m = note.floor() as i32;
        let f = note - m as f32;
        let a = self.key(m.clamp(self.low, self.high)).center().x;
        let b = self.key((m + 1).clamp(self.low, self.high)).center().x;
        a + (b - a) * f
    }

    /// The note under `x`.
    pub fn note_at(&self, p: Pos2) -> i32 {
        (self.low..=self.high)
            .filter(|m| is_black(*m))
            .find(|m| self.key(*m).contains(p))
            .or_else(|| {
                (self.low..=self.high)
                    .filter(|m| !is_black(*m))
                    .find(|m| p.x < self.key(*m).right())
            })
            .unwrap_or(self.high)
    }
}

/// The arc of a lane: from the key heard to the landing, over the keyboard, as high as the
/// interval is wide.
fn arc_points(kb: &Keyboard, anchor: f32, interval: f32, room: f32, k: f32) -> [Pos2; 33] {
    let y = kb.r.top() - 5.0 * k;
    let x0 = kb.x(anchor);
    let x1 = kb.x(anchor + interval);
    let w = interval.abs();
    let h = 16.0 * k + (w / 24.0).min(1.0).powf(0.55) * (room - 16.0 * k);
    // A unison still leaps: a small loop above its key.
    let (x0, x1) = if (x1 - x0).abs() < 6.0 * k {
        let d = 6.0 * k * if interval < 0.0 { -1.0 } else { 1.0 };
        (x0 - d, x0 + d)
    } else {
        (x0, x1)
    };
    let mut pts = [Pos2::ZERO; 33];
    for (i, p) in pts.iter_mut().enumerate() {
        let t = i as f32 / 32.0;
        let x = x0 + (x1 - x0) * t;
        *p = pos2(x, y - 4.0 * h * t * (1.0 - t));
    }
    pts
}

/// The display: its legends and status, the leap, the pedal in Whammy; its drags.
pub fn show(
    host: &mut dyn FxHost,
    v: &mut View,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    ss: &SaltusSettings,
    rd: &Reading,
) {
    let k = s.k;
    let whammy = ss.mode == Mode::Whammy;
    let p = parts(r, k, whammy);
    let ink = Ink::of(s);
    let lanes = lanes(ss, v.anchor, v.pedal, v.held);
    let (low, high) = span(v.anchor, &lanes);
    let kb = Keyboard::new(p.keys, low, high);
    legends(host, ui, s, p.band);
    leap_edit(host, v, ui, &kb, &lanes, ss, p.leap, k);
    if let Some(pr) = p.pedal {
        pedal::edit(host, ui, pr, k);
    }
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        paint_band(b, s, &ink, p.band, ss, host.sample_rate(), k);
        paint_leap(b, &ink, p.leap, &kb, v, ss, rd, &lanes, k);
        if let Some(pr) = p.pedal {
            let by_notes = ss.whammy_notes;
            pedal::paint(b, &ink, pr, v.pedal, rd.target, ss, by_notes, v.held() != 0, k);
            b.vline(
                pr.left(),
                pr.top() + 10.0 * k,
                pr.bottom() - 10.0 * k,
                0.7 * k,
                ink.grid,
            );
        }
        b.hline(
            r.left() + 10.0 * k,
            r.right() - 10.0 * k,
            p.band.bottom(),
            0.7 * k,
            ink.grid,
        );
        canvas::edge_b(b, r, &ink);
    });
}

/// The four modes along the top: a click chooses one (its menu offers automation and MIDI
/// learn of the mode).
fn legends(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, band: Rect) {
    let now = host.spec().get("mode").round().max(0.0) as usize;
    let name = frame::tr("fx.saltus.mode");
    for (i, r) in legend_rects(band, s.k).into_iter().enumerate() {
        let resp = ui.interact(r, ui.id().with(("saltus-mode", i)), Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, "mode", name, &resp);
        if resp.clicked() && !learnt && i != now {
            host.set("mode", i as f32, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr(words::MODES[i]),
                frame::tr("saltus.d.mode_tip"),
                Caps::NONE,
            );
        }
    }
}

fn legend_rects(band: Rect, k: f32) -> [Rect; 4] {
    let w = 128.0 * k;
    std::array::from_fn(|i| {
        Rect::from_min_size(
            pos2(band.left() + 12.0 * k + i as f32 * w, band.top() + 3.0 * k),
            vec2(w - 6.0 * k, band.height() - 6.0 * k),
        )
    })
}

/// Drag a voice's landing to another key: its interval in Shift.
#[allow(clippy::too_many_arguments)]
fn leap_edit(
    host: &mut dyn FxHost,
    v: &mut View,
    ui: &mut egui::Ui,
    kb: &Keyboard,
    lanes: &[Lane],
    ss: &SaltusSettings,
    leap: Rect,
    k: f32,
) {
    let g = ui.interact(leap, ui.id().with("saltus-leap"), Sense::click_and_drag());
    let pointer = g.interact_pointer_pos().or(g.hover_pos());
    if g.drag_started() {
        if let Some(pt) = pointer {
            v.drag = lanes
                .iter()
                .enumerate()
                .filter(|(_, l)| l.key.is_some())
                .map(|(i, l)| {
                    let at = pos2(kb.x(v.anchor + l.interval), kb.r.top() - 5.0 * k);
                    (i, at.distance(pt))
                })
                .filter(|(_, d)| *d < 24.0 * k)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
        }
    }
    if let (Some(i), Some(pt)) = (v.drag, pointer) {
        if g.dragged() {
            if let Some(key) = lanes.get(i).and_then(|l| l.key) {
                let to = kb.note_at(pos2(pt.x, kb.r.center().y));
                let st = (to - v.anchor.round() as i32).clamp(-24, 24) as f32;
                if st != ss.voices[i].semitones {
                    host.set(key, st, Some(key));
                }
            }
        }
    }
    if g.drag_stopped() && v.drag.take().is_some() {
        host.end_gesture();
    }
    if g.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("saltus.d.leap_tip"),
            if ss.mode == Mode::Shift {
                frame::tr("saltus.d.leap_how")
            } else {
                ""
            },
            Caps::NONE,
        );
    }
}

/// A mode's small sign beside its legend, in a box of `u` points.
fn icon(b: &mut Brush, mode: usize, c: Pos2, u: f32, col: Color32, k: f32) {
    let w = 1.1 * k;
    let h = u * 0.5;
    match mode {
        // Shift: a leap up and a leap down.
        0 => {
            let pts: [Pos2; 9] = std::array::from_fn(|i| {
                let t = i as f32 / 8.0;
                pos2(c.x - h + 2.0 * h * t, c.y + h * 0.5 - 4.0 * h * 0.8 * t * (1.0 - t))
            });
            b.polyline(&pts, w, col);
            b.dot(pts[8], 1.6 * k, col);
        }
        // Harmony: a chord of three notes stacked on a stem.
        1 => {
            for j in 0..3 {
                let y = c.y + h * 0.7 - j as f32 * h * 0.7;
                b.circle_stroke(pos2(c.x - h * 0.25, y), 2.2 * k, w, col);
            }
            b.segment(
                pos2(c.x + h * 0.25, c.y + h * 0.7),
                pos2(c.x + h * 0.25, c.y - h),
                w,
                col,
                true,
            );
        }
        // Octave: two notes an octave apart, joined.
        2 => {
            let (a, z) = (pos2(c.x - h * 0.6, c.y + h * 0.6), pos2(c.x + h * 0.6, c.y - h * 0.6));
            b.segment(a, z, w * 0.8, with_alpha(col, 0.6), false);
            b.dot(a, 2.4 * k, col);
            b.circle_stroke(z, 2.4 * k, w, col);
        }
        // Whammy: a treadle on its base.
        _ => {
            b.segment(
                pos2(c.x - h, c.y + h * 0.7),
                pos2(c.x + h, c.y + h * 0.7),
                w,
                col,
                true,
            );
            b.segment(
                pos2(c.x - h, c.y + h * 0.2),
                pos2(c.x + h, c.y - h * 0.5),
                w * 1.3,
                col,
                true,
            );
            b.dot(pos2(c.x - h * 0.3, c.y + h * 0.4), 1.4 * k, col);
        }
    }
}

/// The legends and, at the right, the engine and its latency.
fn paint_band(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    band: Rect,
    ss: &SaltusSettings,
    sr: f32,
    k: f32,
) {
    let now = ss.mode as usize;
    for (i, r) in legend_rects(band, k).into_iter().enumerate() {
        let on = i == now;
        let col = if on { ink.hue } else { ink.dim };
        icon(b, i, pos2(r.left() + 10.0 * k, r.center().y), 14.0 * k, col, k);
        let at = pos2(r.left() + 24.0 * k, r.center().y);
        let t = canvas::put(
            b,
            at,
            Align2::LEFT_CENTER,
            &Text::key(words::MODES[i], Role::Control, col),
            k,
            None,
        );
        if on {
            let y = r.bottom() - 1.0 * k;
            canvas::trace(
                b,
                &[pos2(r.left() + 4.0 * k, y), pos2(t.right() + 2.0 * k, y)],
                1.6 * k,
                ink.hue,
                ink,
            );
        }
    }
    let mut w = String::new();
    let engine = match ss.mode {
        Mode::Shift | Mode::Whammy => ["fx.saltus.engine.0", "fx.saltus.engine.1"][ss.engine as usize],
        Mode::Harmony => words::HSRC[ss.harmony_source as usize],
        Mode::Octave => ["fx.saltus.ostyle.0", "fx.saltus.ostyle.1"][ss.octave_style as usize],
    };
    w.push_str(frame::tr(engine));
    w.push_str("  \u{b7}  ");
    let lat = cantus_dsp::saltus::latency_for(ss, sr);
    if lat == 0 {
        w.push_str(frame::tr("saltus.d.no_latency"));
    } else {
        w.push_str(frame::tr("saltus.d.latency"));
        w.push(' ');
        controls::write_readout(lat as f32 / sr.max(1.0) * 1000.0, "ms", &mut w);
    }
    canvas::put(
        b,
        pos2(band.right() - 14.0 * k, band.center().y),
        Align2::RIGHT_CENTER,
        &Text::new(&w, Role::Value, ink.text).caps(false),
        k,
        None,
    );
    let _ = s;
}

/// The words above an arc's apex: its name, then its interval and its landing.
struct Label {
    name: String,
    value: String,
    col: Color32,
}

/// The leap: the note heard, the keyboard, an arc to each lane, the whammy's travel.
#[allow(clippy::too_many_arguments)]
fn paint_leap(
    b: &mut Brush,
    ink: &Ink,
    leap: Rect,
    kb: &Keyboard,
    v: &View,
    ss: &SaltusSettings,
    rd: &Reading,
    lanes: &[Lane],
    k: f32,
) {
    let poly = cantus_dsp::saltus::path_of(ss) == Path::Poly;
    let second = ink.look.trace2;
    let lit = if poly { 1.0 } else { v.heard };
    // The note heard, large, at the top left.
    let x0 = leap.left() + 20.0 * k;
    canvas::put(
        b,
        pos2(x0, leap.top() + 14.0 * k),
        Align2::LEFT_TOP,
        &Text::key("saltus.d.heard", Role::Micro, ink.dim),
        k,
        None,
    );
    let mut w = String::new();
    let mut detail = String::new();
    if poly {
        w.push_str(frame::tr("saltus.d.every_note"));
        detail.push_str(frame::tr("saltus.d.poly_about"));
    } else if v.heard > 0.0 {
        let m = v.anchor.round() as i32;
        words::note(m, &mut w);
        controls::write_readout((v.anchor - m as f32) * 100.0, "ct", &mut detail);
        detail.push_str("  \u{b7}  ");
        let hz = ss.a4 * cantus_dsp::math::exp2((v.anchor - 69.0) / 12.0);
        controls::write_readout(hz, "Hz", &mut detail);
    } else {
        w.push_str(frame::tr("saltus.d.no_note"));
    }
    let hero_col = if poly || v.heard > 0.0 {
        with_alpha(ink.hue, 0.45 + 0.55 * lit)
    } else {
        ink.dim
    };
    let role = if poly { Role::Title } else { Role::Display };
    let hr = canvas::put(
        b,
        pos2(x0, leap.top() + 28.0 * k),
        Align2::LEFT_TOP,
        &Text::new(&w, role, hero_col).caps(false),
        k,
        None,
    );
    if !detail.is_empty() {
        canvas::put(
            b,
            pos2(x0, hr.bottom() + 4.0 * k),
            Align2::LEFT_TOP,
            &Text::new(&detail, Role::Value, ink.text).caps(false),
            k,
            None,
        );
    }
    harmony_note(b, ink, leap, ss, v.held(), k);
    // The keyboard: the scale's notes marked, the chord held, the landings and the note.
    let in_scale = |m: i32| {
        ss.mode == Mode::Harmony
            && ss.harmony_source == HarmonySource::Scale
            && ss
                .scale
                .degrees()
                .contains(&((m - ss.key as i32).rem_euclid(12) as u8))
    };
    let held = |m: i32| {
        ss.mode == Mode::Harmony
            && ss.harmony_source != HarmonySource::Scale
            && v.held() & (1 << m.rem_euclid(12)) != 0
    };
    let anchor_key = v.anchor.round() as i32;
    // Poly moves every note at once: a chord (the minor triad on the note) leaps together.
    let chord = [anchor_key, anchor_key + 3, anchor_key + 7];
    let landing = |m: i32| -> Option<(Color32, f32)> {
        lanes.iter().find_map(|l| {
            let hit = |a: i32| (a as f32 + l.interval).round() as i32 == m;
            let on = if poly { chord.iter().any(|a| hit(*a)) } else { hit(anchor_key) };
            (on && l.level > 0.0).then_some((
                if l.second { second } else { ink.hue },
                l.level,
            ))
        })
    };
    for pass in [false, true] {
        for m in kb.low..=kb.high {
            if is_black(m) != pass {
                continue;
            }
            let r = kb.key(m).shrink2(vec2(1.0 * k, 0.0));
            let base = if pass {
                mix(ink.ground, ink.text, 0.03)
            } else {
                mix(ink.ground, ink.text, 0.12)
            };
            b.rrect(r, 2.0 * k, crate::kit::Fill::Solid(base));
            if held(m) {
                b.rrect(r, 2.0 * k, crate::kit::Fill::Solid(with_alpha(second, 0.28)));
            }
            // A landing: lit lightly and ringed in its voice's colour (the note heard is
            // solid).
            let land = landing(m);
            if let Some((c, lv)) = land {
                let a = 0.12 + 0.2 * lv * lit.max(0.4);
                b.rrect(r, 2.0 * k, crate::kit::Fill::Solid(with_alpha(c, a)));
            }
            let lit_key = m == anchor_key || poly && chord.contains(&m);
            if lit_key {
                let a = if poly { 0.55 } else { 0.22 + 0.6 * v.heard };
                b.rrect(r, 2.0 * k, crate::kit::Fill::Solid(with_alpha(ink.hue, a)));
            }
            b.rrect_stroke(r, 2.0 * k, 0.8 * k, if pass { ink.grid } else { ink.grid_strong });
            if let Some((c, _)) = land {
                b.rrect_stroke(r.shrink(1.0 * k), 2.0 * k, 1.6 * k, c);
            }
            if in_scale(m) {
                let c = pos2(r.center().x, r.bottom() - if pass { 7.0 } else { 22.0 } * k);
                if (m - ss.key as i32).rem_euclid(12) == 0 {
                    b.circle_stroke(c, 3.0 * k, 1.0 * k, ink.hue);
                } else {
                    b.dot(c, 1.8 * k, ink.text);
                }
            }
            if !pass && m.rem_euclid(12) == 0 {
                let mut n = String::new();
                words::note(m, &mut n);
                canvas::put(
                    b,
                    pos2(r.center().x, r.bottom() - 5.0 * k),
                    Align2::CENTER_BOTTOM,
                    &Text::new(&n, Role::Micro, if lit_key { ink.ground } else { ink.dim }).caps(false),
                    k,
                    None,
                );
            }
        }
    }
    // The arcs, then their words (on plates, never over each other).
    let room = (kb.r.top() - leap.top() - 96.0 * k).max(40.0 * k);
    let mut placed: Vec<Rect> = vec![hr.expand(6.0 * k)];
    let mut labels = Vec::new();
    trail(b, ink, leap, kb, v, k);
    for (i, l) in lanes.iter().enumerate() {
        // A voice at no level is not drawn (an octave is: its arc waits, dashed).
        if l.level <= 0.0 && l.name.is_none() {
            continue;
        }
        if poly && l.level > 0.0 {
            for a in &chord[1..] {
                let pts = arc_points(kb, *a as f32, l.interval, room * 0.82, k);
                let col = if l.second { second } else { ink.hue };
                canvas::trace(b, &pts, (0.8 + 1.0 * l.level) * k, with_alpha(col, 0.55), ink);
                b.dot(pts[32], 2.6 * k, col);
            }
        }
        let pts = arc_points(kb, v.anchor, l.interval, room, k);
        let col = if l.second { second } else { ink.hue };
        let sounding = match ss.mode {
            Mode::Octave if ss.octave_style == OctaveStyle::Analog => {
                v.octaves[[1, 0, 2][i.min(2)]].max(0.35 * lit)
            }
            _ => lit.max(v.lit.get(i).copied().unwrap_or(0.0)),
        };
        if l.level <= 0.0 {
            b.dashed(&pts, 0.9 * k, with_alpha(col, 0.4), 4.0 * k, 4.0 * k);
        } else {
            let a = 0.4 + 0.6 * sounding;
            canvas::trace(b, &pts, (1.0 + 1.8 * l.level) * k, with_alpha(col, a), ink);
            b.dot(pts[32], 3.2 * k, col);
            b.circle_stroke(pts[0], 3.0 * k, 1.0 * k, col);
        }
        let mut name = String::new();
        match l.name {
            Some(key) => name.push_str(frame::tr(key)),
            None => words::interval(l.interval.round() as i32, &mut name),
        }
        let mut value = String::new();
        if ss.mode == Mode::Whammy {
            words::fine_st(l.interval, &mut value);
        } else if l.name.is_some() {
            if l.level > 0.0 {
                controls::write_readout(l.level, "%", &mut value);
            } else {
                value.push_str(frame::tr("saltus.d.off"));
            }
        } else {
            words::semitone_pill(l.interval, &mut value);
        }
        if !poly && l.level > 0.0 {
            value.push_str(" \u{b7} ");
            words::note((v.anchor + l.interval).round() as i32, &mut value);
        }
        let col = if l.level <= 0.0 { ink.dim } else { col };
        labels.push((pts[16], Label { name, value, col }));
    }
    for (apex, l) in labels {
        let mut y = apex.y - 8.0 * k;
        let size = |t: &str, role| crate::kit::text::measure(b.ctx(), &Text::new(t, role, l.col).caps(false), k);
        let (sn, sv) = (size(&l.name, Role::Label), size(&l.value, Role::Value));
        let w = sn.x.max(sv.x) + 8.0 * k;
        let h = sn.y + sv.y + 4.0 * k;
        let mut r = Rect::from_center_size(pos2(apex.x, y - h * 0.5), vec2(w, h));
        r = r.translate(vec2(
            (leap.left() + 6.0 * k - r.left()).max(0.0) - (r.right() - leap.right() + 6.0 * k).max(0.0),
            0.0,
        ));
        while placed.iter().any(|p| p.intersects(r)) && r.top() > leap.top() {
            r = r.translate(vec2(0.0, -(h * 0.5 + 2.0 * k)));
        }
        placed.push(r);
        y = r.bottom();
        canvas::put(
            b,
            pos2(r.center().x, y),
            Align2::CENTER_BOTTOM,
            &Text::new(&l.value, Role::Value, ink.text).caps(false),
            k,
            Some(ink.plate),
        );
        canvas::put(
            b,
            pos2(r.center().x, y - sv.y - 3.0 * k),
            Align2::CENTER_BOTTOM,
            &Text::new(&l.name, Role::Label, l.col).caps(false),
            k,
            Some(ink.plate),
        );
    }
    if ss.mode == Mode::Whammy {
        travel(b, ink, kb, v.anchor, ss, k);
    }
    let _ = rd;
}

/// Harmony's words at the top right: the key and scale, or the chord or notes held.
fn harmony_note(b: &mut Brush, ink: &Ink, leap: Rect, ss: &SaltusSettings, held: u16, k: f32) {
    if ss.mode != Mode::Harmony {
        return;
    }
    let mut w = String::new();
    let (cap, col) = match ss.harmony_source {
        HarmonySource::Scale => {
            w.push_str(frame::tr(words::KEYS[ss.key as usize % 12]));
            w.push(' ');
            w.push_str(&frame::tr(words::SCALES[ss.scale as usize]).to_lowercase());
            ("saltus.l.counts", ink.text)
        }
        _ if held == 0 => {
            w.push_str(frame::tr("saltus.d.nothing_held"));
            ("saltus.d.chord_held", ink.dim)
        }
        src => {
            for c in (0..12).filter(|c| held & (1 << c) != 0) {
                if !w.is_empty() {
                    w.push(' ');
                }
                w.push_str(frame::tr(words::KEYS[c]));
            }
            let cap = if src == HarmonySource::Chord {
                "saltus.d.chord_held"
            } else {
                "saltus.d.melody_held"
            };
            (cap, ink.look.trace2)
        }
    };
    let x = leap.right() - 20.0 * k;
    canvas::put(
        b,
        pos2(x, leap.top() + 14.0 * k),
        Align2::RIGHT_TOP,
        &Text::key(cap, Role::Micro, ink.dim),
        k,
        None,
    );
    canvas::put(
        b,
        pos2(x, leap.top() + 30.0 * k),
        Align2::RIGHT_TOP,
        &Text::new(&w, Role::Title, col).caps(false),
        k,
        None,
    );
}

/// The whammy's travel under the keyboard: from the heel's landing to the toe's.
fn travel(b: &mut Brush, ink: &Ink, kb: &Keyboard, anchor: f32, ss: &SaltusSettings, k: f32) {
    let y = kb.r.bottom() + 9.0 * k;
    let (xh, xt) = (kb.x(anchor + ss.heel), kb.x(anchor + ss.toe));
    b.segment(pos2(xh, y), pos2(xt, y), 1.0 * k, with_alpha(ink.hue, 0.7), false);
    for (x, key) in [(xh, "saltus.d.heel"), (xt, "saltus.d.toe")] {
        b.segment(pos2(x, y - 4.0 * k), pos2(x, y + 4.0 * k), 1.0 * k, ink.hue, true);
        canvas::put(
            b,
            pos2(x, y + 5.0 * k),
            Align2::CENTER_TOP,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            None,
        );
    }
}

/// The last seconds of pitch rising from the keyboard: the note heard, and where each
/// voice landed, fading with age.
fn trail(b: &mut Brush, ink: &Ink, leap: Rect, kb: &Keyboard, v: &View, k: f32) {
    let n = v.trail.len();
    if n < 2 {
        return;
    }
    let (y0, y1) = (kb.r.top() - 6.0 * k, leap.top() + 84.0 * k);
    let at = |age: usize| y0 - (y0 - y1) * age as f32 / (TRAIL - 1) as f32;
    let cols = [ink.hue, ink.hue, ink.look.trace2, ink.look.trace2];
    for lane in 0..4 {
        for j in 1..n {
            let (a, c) = (v.trail[n - j][lane], v.trail[n - 1 - j][lane]);
            if a.is_nan() || c.is_nan() || (a - c).abs() > 1.5 {
                continue;
            }
            let fade = 1.0 - j as f32 / TRAIL as f32;
            let (w, alpha) = if lane == 0 {
                (1.5, 0.75 * fade)
            } else {
                (1.0, 0.4 * fade)
            };
            let (pa, pc) = (pos2(kb.x(a), at(j - 1)), pos2(kb.x(c), at(j)));
            if leap.contains(pa) && leap.contains(pc) {
                b.segment(pa, pc, w * k, with_alpha(cols[lane], alpha), false);
            }
        }
    }
}

//! Lapsus's timeline: the time pattern and the volume pattern playing, over the longer of
//! their two cycles (the shorter one repeated across it), the time curve over the cycle's
//! own input and the level curve under it, the playhead and the slip (fx side round, ADR
//! 0132 provisional).
//!
//! The time lane reads downwards: its top line is *now* (the music as it arrives), its
//! bottom one cycle back. x and y share the cycle's length, so a slope is a speed you can
//! see: flat is the music, a line down to the right at the diagonal is the tape stopped,
//! half that slope half speed, steeper reverse. Below the diagonal lies what was before
//! the cycle began, which Lapsus never reads (silence): shaded, and named. While the unit
//! plays, the playhead crosses both lanes and **the slip** runs from where the song is to
//! where the music is read from, marked on the ruler.
//!
//! Drawn fine, as the best plug-in displays are (Gross Beat's envelope, FabFilter's curves):
//! hairline grids, curves of a point and a half, hollow handles, one-pixel playhead.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::super::modfx;
use super::{Playing, State};
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::fx::lapsus::{self as model, Lane};
use cantus_dsp::lapsus::{t, Pattern};
use cantus_dsp::modfx_clock::tel;
use cantus_engine::fx::fluctus::shape_at;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};
use std::collections::VecDeque;

/// Columns of the cycle's waveform.
pub const COLS: usize = 192;

/// The input's peak level across the drawn span, column by column: what the timeline draws
/// faintly under the time curve.
#[derive(Clone, Debug)]
pub struct Wave {
    pub peak: [f32; COLS],
    /// The last pairs taken (their count and the newest's bits), so each comes once.
    sig: (usize, u64),
}

impl Default for Wave {
    fn default() -> Self {
        Wave {
            peak: [0.0; COLS],
            sig: (0, 0),
        }
    }
}

impl Wave {
    /// The analyser's newest pairs (the input first of each), the span's phase at the last
    /// of them and the span's length in samples.
    pub fn feed(&mut self, pairs: &VecDeque<(f32, f32)>, phase_end: f32, span: f32) {
        let sig = (
            pairs.len(),
            pairs.back().map_or(0, |p| {
                (u64::from(p.0.to_bits()) << 32) | u64::from(p.1.to_bits())
            }),
        );
        if pairs.is_empty() || sig == self.sig || span < 1.0 {
            return;
        }
        self.sig = sig;
        let mut fresh = [0.0f32; COLS];
        let mut hit = [false; COLS];
        for (j, (x, _)) in pairs.iter().rev().enumerate() {
            let ph = phase_end - j as f32 / span;
            let c = ((ph.rem_euclid(1.0) * COLS as f32) as usize).min(COLS - 1);
            fresh[c] = fresh[c].max(x.abs());
            hit[c] = true;
        }
        for c in 0..COLS {
            if hit[c] {
                self.peak[c] = fresh[c];
            }
        }
    }
}

/// The timeline's places: x across the span (0 … 1), the time lane's delay down, the
/// level up.
#[derive(Copy, Clone, Debug)]
struct Map {
    time: Rect,
    level: Rect,
}

impl Map {
    fn x(&self, u: f32) -> f32 {
        self.time.left() + self.time.width() * u.clamp(0.0, 1.0)
    }
    /// The time lane: 0 (now) at the top, one time cycle back at the bottom.
    fn yt(&self, d: f32) -> f32 {
        self.time.top() + self.time.height() * d.clamp(0.0, 1.0)
    }
    fn yl(&self, v: f32) -> f32 {
        self.level.bottom() - self.level.height() * v.clamp(0.0, 1.0)
    }
}

/// The two patterns drawn, and how many times each fits the span.
struct Shown {
    time: Pattern,
    vol: Pattern,
    /// Quarter notes in the span (the longer cycle).
    beats: f32,
    /// Turns of each cycle in the span.
    reps: (usize, usize),
}

impl Shown {
    fn of(host: &dyn FxHost, playing: Playing) -> Shown {
        let time = super::pattern_of(host.spec(), Lane::Time, playing.time);
        let vol = super::pattern_of(host.spec(), Lane::Vol, playing.vol);
        let beats = time.beats.max(vol.beats);
        let reps = (
            (beats / time.beats).round().max(1.0) as usize,
            (beats / vol.beats).round().max(1.0) as usize,
        );
        Shown {
            time,
            vol,
            beats: beats as f32,
            reps,
        }
    }
}

/// Whether the unit runs and its telemetry is fresh.
fn live(tel: &[f32; 64]) -> bool {
    tel[tel::BLOCKS] > 0.0 && tel[tel::PLAYING] > 0.5
}

/// Where in the span the playhead is: the longer cycle's phase.
fn span_phase(tel: &[f32; 64], sh: &Shown) -> f32 {
    if sh.reps.0 == 1 {
        tel[t::PHASE]
    } else {
        tel[t::VPHASE]
    }
}

#[allow(clippy::too_many_arguments)]
pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    playing: Playing,
) {
    let sh = Shown::of(host, playing);
    let tempo = if tel[tel::TEMPO] > 0.0 {
        tel[tel::TEMPO]
    } else {
        120.0
    };
    controls::card(
        ui,
        s,
        rect,
        Src::Key("lapsus.card.timeline"),
        None,
        |ui, cr| {
            let r = cr.body;
            let k = s.k;
            let top = r.top() + 40.0 * k;
            let bottom = r.bottom() - 10.0 * k;
            let left = r.left() + 48.0 * k;
            let right = r.right() - 12.0 * k;
            let split = top + (bottom - top) * 0.63;
            let map = Map {
                time: Rect::from_min_max(pos2(left, top), pos2(right, split - 9.0 * k)),
                level: Rect::from_min_max(pos2(left, split + 9.0 * k), pos2(right, bottom)),
            };
            st.lanes = Some((map.time, map.level));
            if live(tel) {
                let span = (f64::from(sh.beats) * 60.0 / f64::from(tempo)
                    * f64::from(host.sample_rate())) as f32;
                st.wave.feed(host.pairs(), span_phase(tel, &sh), span);
            }
            let mut hovered = false;
            for (lane, rect, reps, beats) in [
                (Lane::Time, map.time, sh.reps.0, sh.time.beats),
                (Lane::Vol, map.level, sh.reps.1, sh.vol.beats),
            ] {
                let resp = ui.interact(
                    rect,
                    ui.id().with(("lapsus-lane", lane as u8)),
                    egui::Sense::click_and_drag(),
                );
                hovered |= resp.hovered();
                let place = super::edit::Place {
                    lane,
                    rect,
                    reps,
                    beats,
                };
                super::edit::lane(host, st, ui, &resp, place, playing.of(lane), k);
            }
            // What is drawn is what is now set (an edit above may have moved it).
            let sh = Shown::of(host, playing);
            let spec = host.spec();
            paint(ui, s, r, map, st, tel, &sh, spec, playing, tempo);
            if hovered {
                hint::set_text(
                    ui.ctx(),
                    frame::tr("lapsus.timeline_tip"),
                    frame::tr("lapsus.timeline_how"),
                    Caps::NONE,
                );
            }
            if sh.time.is_through(cantus_dsp::lapsus::Lane::Time)
                && sh.vol.is_through(cantus_dsp::lapsus::Lane::Vol)
                && !live(tel)
            {
                modfx::idle_note(ui, s, map.time, "lapsus.idle");
            }
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &egui::Ui,
    s: &Style,
    r: Rect,
    map: Map,
    st: &State,
    tel: &[f32; 64],
    sh: &Shown,
    spec: &cantus_core::EffectSpec,
    playing: Playing,
    tempo: f32,
) {
    let ink = Ink::of(s);
    let k = s.k;
    let (amber, cyan) = voices(s, &ink);
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        lanes_grid(b, map, &ink, sh.beats, super::GRIDS[st.grid].1 as f32);
        for rep in 0..sh.reps.0 {
            before_the_cycle(b, map, &ink, rep, sh.reps.0, k);
        }
        wave(b, map, &st.wave, amber);
        // The curves, sampled across the span (each cycle repeated as often as it fits).
        const N: usize = 721;
        let mut time = [Pos2::ZERO; N];
        let mut level = [Pos2::ZERO; N];
        for j in 0..N {
            let u = j as f64 / (N - 1) as f64;
            let x = map.x(u as f32);
            let (rt, rv) = (sh.reps.0 as f64, sh.reps.1 as f64);
            let ut = ((u * rt) - (u * rt).floor().min(rt - 1.0)).min(0.999_999);
            let uv = ((u * rv) - (u * rv).floor().min(rv - 1.0)).min(0.999_999);
            time[j] = pos2(x, map.yt(sh.time.curve.at(ut, shape_at) as f32));
            level[j] = pos2(x, map.yl(sh.vol.curve.at(uv, shape_at) as f32));
        }
        b.area(&level, map.level.bottom(), with_alpha(cyan, 0.16));
        b.polyline(&level, 1.1 * k, cyan);
        canvas::trace(b, &time, 1.4 * k, amber, &ink);
        for rep in 0..sh.reps.0 {
            for q in sh.time.curve.points() {
                let x = (rep as f32 + q.x) / sh.reps.0 as f32;
                node(b, pos2(map.x(x), map.yt(q.y)), amber, ink.ground, k);
            }
        }
        for rep in 0..sh.reps.1 {
            for q in sh.vol.curve.points() {
                let x = (rep as f32 + q.x) / sh.reps.1 as f32;
                dot(b, pos2(map.x(x), map.yl(q.y)), cyan, ink.ground, k);
            }
        }
        ruler(b, map, &ink, sh.beats, k);
        gutter(b, map, &ink, spec, playing, k);
        header(b, r, map, &ink, (amber, cyan), spec, playing, tempo, k);
        if live(tel) {
            playhead(b, map, &ink, tel, sh, amber, cyan, k);
        }
        canvas::edge_b(b, r, &ink);
    });
}

/// A point of the time curve: a small hollow square, a sequencer's step.
fn node(b: &mut Brush, at: Pos2, c: Color32, ground: Color32, k: f32) {
    let r = b.snap_rect(Rect::from_center_size(at, vec2(6.0 * k, 6.0 * k)));
    b.rect(r, Fill::Solid(ground));
    b.rrect_stroke(r, 0.0, 1.0 * k, c);
}

/// A point of the level curve: a small hollow ring.
fn dot(b: &mut Brush, at: Pos2, c: Color32, ground: Color32, k: f32) {
    b.dot(at, 2.8 * k, ground);
    b.circle_stroke(at, 2.8 * k, 1.0 * k, c);
}

/// The beats, the grid's steps and the lanes' levels: hairlines.
fn lanes_grid(b: &mut Brush, map: Map, ink: &Ink, beats: f32, step: f32) {
    let steps = (beats / step.max(0.125)).round().max(1.0) as usize;
    let px = b.px();
    for lane in [map.time, map.level] {
        for j in 1..steps {
            let u = j as f32 / steps as f32;
            let at = u * beats;
            let c = if (at / 4.0).fract().abs() < 1e-4 {
                ink.grid_strong
            } else if at.fract().abs() < 1e-4 {
                with_alpha(ink.grid_strong, 0.7)
            } else {
                with_alpha(ink.grid, 0.7)
            };
            b.vline(b.snap_mid(map.x(u)), lane.top(), lane.bottom(), px, c);
        }
        let r = b.snap_rect(lane);
        b.rrect_stroke(r, 0.0, px, ink.grid_strong);
    }
    for q in [0.25f32, 0.5, 0.75] {
        let c = with_alpha(ink.grid, 0.7);
        b.hline(
            map.time.left(),
            map.time.right(),
            b.snap_mid(map.yt(q)),
            px,
            c,
        );
        b.hline(
            map.level.left(),
            map.level.right(),
            b.snap_mid(map.yl(q)),
            px,
            c,
        );
    }
}

/// What lies before the time cycle's start (the delay past the time already played): never
/// read — shaded, its edge the stopped tape's diagonal, and named once.
fn before_the_cycle(b: &mut Brush, map: Map, ink: &Ink, rep: usize, reps: usize, k: f32) {
    let (u0, u1) = (rep as f32 / reps as f32, (rep + 1) as f32 / reps as f32);
    let a = pos2(map.x(u0), map.yt(0.0));
    let c = pos2(map.x(u1), map.yt(1.0));
    let d = pos2(map.x(u0), map.yt(1.0));
    b.polygon(&[a, c, d], with_alpha(ink.faint, 0.12));
    canvas::dashed(
        b,
        a,
        c,
        b.px(),
        with_alpha(ink.grid_strong, 0.9),
        3.0 * k,
        3.0 * k,
    );
    if rep == 0 {
        let t = Text::key("lapsus.before", Role::Micro, ink.dim);
        canvas::put(
            b,
            pos2(d.x + 8.0 * k, d.y - 6.0 * k),
            Align2::LEFT_BOTTOM,
            &t,
            k,
            None,
        );
    }
}

/// The input along the span, faint, hanging from the *now* line.
fn wave(b: &mut Brush, map: Map, w: &Wave, c: Color32) {
    let lane = map.time;
    let top = lane.top() + 1.0;
    let deep = lane.height() * 0.26;
    let cw = lane.width() / COLS as f32;
    let col = with_alpha(c, 0.14);
    for (i, &v) in w.peak.iter().enumerate() {
        if v <= 0.002 {
            continue;
        }
        let h = v.min(1.0).sqrt() * deep;
        let x0 = lane.left() + i as f32 * cw;
        b.rect(
            Rect::from_min_max(pos2(x0 + 0.5, top), pos2(x0 + cw - 0.5, top + h)),
            Fill::Solid(col),
        );
    }
}

/// The beats' numbers above the time lane.
fn ruler(b: &mut Brush, map: Map, ink: &Ink, beats: f32, k: f32) {
    let n = beats.round().max(1.0) as usize;
    let y = map.time.top() - 6.0 * k;
    for j in 0..n {
        let u = j as f32 / beats;
        canvas::putf(
            b,
            pos2(map.x(u) + 7.0 * k, y),
            Align2::LEFT_BOTTOM,
            Role::Micro,
            if j % 4 == 0 { ink.text } else { ink.dim },
            k,
            None,
            format_args!("{}", j + 1),
        );
    }
}

/// The lanes' scales in the gutter: *now* and a time cycle back; full and silent.
fn gutter(
    b: &mut Brush,
    map: Map,
    ink: &Ink,
    spec: &cantus_core::EffectSpec,
    playing: Playing,
    k: f32,
) {
    let x = map.time.left() - 6.0 * k;
    let len = model::pattern_of(spec.lapsus.as_deref(), Lane::Time, playing.time).len;
    let back = super::LEN_KEYS[(len as usize).min(4)];
    for (key, y, align) in [
        ("lapsus.now", map.yt(0.0), Align2::RIGHT_TOP),
        (back, map.yt(1.0), Align2::RIGHT_BOTTOM),
    ] {
        let t = Text::key(key, Role::Micro, ink.dim);
        canvas::put(b, pos2(x, y), align, &t, k, None);
    }
    for (w, v, align) in [
        ("100%", 1.0f32, Align2::RIGHT_TOP),
        ("0%", 0.0, Align2::RIGHT_BOTTOM),
    ] {
        let t = Text::new(w, Role::Micro, ink.dim);
        canvas::put(b, pos2(x, map.yl(v)), align, &t, k, None);
    }
}

/// Each lane's legend and the pattern it plays (number, name, length) on the left, the
/// tempo on the right: above the lanes, where no curve runs.
#[allow(clippy::too_many_arguments)]
fn header(
    b: &mut Brush,
    r: Rect,
    map: Map,
    ink: &Ink,
    (amber, cyan): (Color32, Color32),
    spec: &cantus_core::EffectSpec,
    playing: Playing,
    tempo: f32,
    k: f32,
) {
    let y = r.top() + 12.0 * k;
    let mut x = map.time.left();
    let tr = canvas::putf(
        b,
        pos2(r.right() - 10.0 * k, y),
        Align2::RIGHT_CENTER,
        Role::Value,
        ink.dim,
        k,
        None,
        format_args!("{tempo:.0} BPM"),
    );
    for (lane, key, c) in [
        (Lane::Time, "lapsus.lane.time", amber),
        (Lane::Vol, "lapsus.lane.level", cyan),
    ] {
        b.dot(pos2(x + 3.0 * k, y), 2.5 * k, c);
        let t = Text::key(key, Role::Micro, c).caps(true);
        let w = canvas::put(b, pos2(x + 10.0 * k, y), Align2::LEFT_CENTER, &t, k, None);
        let slot = playing.of(lane);
        let pm = model::pattern_of(spec.lapsus.as_deref(), lane, slot);
        let name = pm
            .name
            .as_deref()
            .unwrap_or_else(|| super::slot_name(lane, slot));
        let room = Rect::from_min_max(
            pos2(w.right() + 6.0 * k, y - 8.0 * k),
            pos2(
                (w.right() + 250.0 * k).min(tr.left() - 12.0 * k),
                y + 8.0 * k,
            ),
        );
        canvas::line_in(
            b,
            room,
            Role::Value,
            ink.text,
            k,
            format_args!(
                "{} {}  \u{b7}  {}",
                slot + 1,
                name,
                frame::tr(super::LEN_KEYS[(pm.len as usize).min(4)]),
            ),
        );
        x = room.right() + 18.0 * k;
    }
}

/// The playhead across both lanes, the read's place on the time curve, the slip back to
/// where the music is read from, and the level now.
#[allow(clippy::too_many_arguments)]
fn playhead(
    b: &mut Brush,
    map: Map,
    ink: &Ink,
    tel: &[f32; 64],
    sh: &Shown,
    amber: Color32,
    cyan: Color32,
    k: f32,
) {
    let u = span_phase(tel, sh).clamp(0.0, 1.0);
    let d = tel[t::DELAY].clamp(0.0, 1.0);
    let v = tel[t::LEVEL].clamp(0.0, 1.0);
    let x = b.snap_mid(map.x(u));
    let px = b.px();
    let head = with_alpha(ink.text, 0.8);
    b.vline(x, map.time.top() - 12.0 * k, map.level.bottom(), px, head);
    let at = pos2(x, map.yt(d));
    // The slip, in the span's units: the time cycle's delay over its turns in the span.
    let read = (u - d / sh.reps.0 as f32).max(0.0);
    let rx = map.x(read);
    if d > 0.002 {
        canvas::dashed(
            b,
            at,
            pos2(rx, map.time.top()),
            px,
            with_alpha(amber, 0.85),
            4.0 * k,
            3.0 * k,
        );
        b.hline(
            rx,
            x,
            map.time.top() - 1.0 * k,
            1.5 * k,
            with_alpha(amber, 0.5),
        );
    }
    let tip = map.time.top();
    b.polygon(
        &[
            pos2(rx - 4.0 * k, tip - 7.0 * k),
            pos2(rx + 4.0 * k, tip - 7.0 * k),
            pos2(rx, tip - 1.0 * k),
        ],
        amber,
    );
    b.circle_stroke(at, 5.0 * k, 1.0 * k, amber);
    b.dot(at, 2.2 * k, amber);
    let lv = pos2(x, map.yl(v));
    b.circle_stroke(lv, 4.5 * k, 1.0 * k, cyan);
    b.dot(lv, 2.0 * k, cyan);
    // How far behind, and how fast the music runs there.
    canvas::tag(
        b,
        pos2(
            (x + 8.0 * k).min(map.time.right() - 4.0 * k),
            map.time.bottom() - 4.0 * k,
        ),
        if x + 140.0 * k < map.time.right() {
            Align2::LEFT_BOTTOM
        } else {
            Align2::RIGHT_BOTTOM
        },
        frame::tr("lapsus.speed"),
        ink.text,
        k,
        Some(ink.plate),
        format_args!(
            "{:.2}\u{d7}  \u{2212}{:.2} {}",
            tel[t::SPEED],
            d * tel[t::BEATS],
            frame::tr("lapsus.beats")
        ),
    );
}

/// The inspector's picture: the two playing patterns, small, with their names.
pub fn preview(ui: &mut egui::Ui, spec: &cantus_core::EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let slot = |key: &str| (spec.get(key).round().max(0.0) as usize).min(model::SLOTS - 1);
    let (ts, vs) = (slot("tslot"), slot("vslot"));
    let tp = super::pattern_of(spec, Lane::Time, ts);
    let vp = super::pattern_of(spec, Lane::Vol, vs);
    let w = ui.available_width();
    let h = 46.0 * s.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let ink = Ink::of(&s);
    let k = s.k;
    let (amber, cyan) = voices(&s, &ink);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let plot = Rect::from_min_max(
            pos2(rect.left() + 6.0 * k, rect.top() + 6.0 * k),
            pos2(rect.left() + 64.0 * k, rect.bottom() - 6.0 * k),
        );
        let half = plot.height() * 0.5;
        let top = Rect::from_min_size(plot.min, vec2(plot.width(), half - 1.0 * k));
        let bot = Rect::from_min_max(pos2(plot.left(), plot.top() + half + 1.0 * k), plot.max);
        thumb(b, top, Lane::Time, &tp, amber, k);
        thumb(b, bot, Lane::Vol, &vp, cyan, k);
        let x = plot.right() + 10.0 * k;
        for (lane, sl, y, c) in [
            (Lane::Time, ts, top.center().y, amber),
            (Lane::Vol, vs, bot.center().y, cyan),
        ] {
            canvas::putf(
                b,
                pos2(x, y),
                Align2::LEFT_CENTER,
                Role::Micro,
                c,
                k,
                None,
                format_args!("{} \u{b7} {}", sl + 1, super::slot_name(lane, sl)),
            );
        }
        canvas::edge_b(b, rect, &ink);
    });
    resp
}

/// A pattern drawn small in `r`: a time curve as a line falling from the top, a volume
/// curve as a filled level.
pub fn thumb(b: &mut Brush, r: Rect, lane: Lane, p: &Pattern, c: Color32, k: f32) {
    const N: usize = 61;
    let mut pts = [Pos2::ZERO; N];
    for (j, q) in pts.iter_mut().enumerate() {
        let u = j as f64 / (N - 1) as f64;
        let v = p.curve.at(u.min(0.999_999), shape_at).clamp(0.0, 1.0) as f32;
        let x = r.left() + r.width() * u as f32;
        *q = match lane {
            Lane::Time => pos2(x, r.top() + 1.0 * k + (r.height() - 2.0 * k) * v),
            Lane::Vol => pos2(x, r.bottom() - (r.height() - 1.0 * k) * v),
        };
    }
    if lane == Lane::Vol {
        b.area(&pts, r.bottom(), with_alpha(c, 0.22));
    }
    b.polyline(&pts, 1.0 * k, c);
}

/// What a playing engine would feed the editor (see `lapsus::demo_feed`).
pub fn demo_feed(app: &mut crate::app::CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut u = cantus_engine::fx::lapsus::Unit::new(48_000.0);
    use cantus_engine::fx::FxUnit as _;
    u.configure(&spec);
    if let Some(cantus_engine::MemoryNeed::Lapsus(n)) = u.memory_need() {
        drop(u.supply(cantus_engine::EffectMemory::build(
            cantus_engine::MemoryNeed::Lapsus(n),
            48_000.0,
        )));
    }
    u.reset();
    let tempo = 120.0f64;
    let slot = |key: &str| (spec.get(key).round().max(0.0) as usize).min(model::SLOTS - 1);
    let bank = spec.lapsus.as_deref();
    let beats = model::pattern_of(bank, Lane::Time, slot("tslot"))
        .beats()
        .max(model::pattern_of(bank, Lane::Vol, slot("vslot")).beats());
    let span = (beats * 60.0 / tempo * 48_000.0) as usize;
    // A span and most of another: the playhead past three quarters of it.
    let total = span + span * 85 / 100;
    let mut pairs: Vec<f32> = Vec::new();
    let mut wave = Wave::default();
    let mut at = 0usize;
    let mut noise = 0x2545_f491u32;
    while at < total {
        let m = 480.min(total - at);
        let mut l = vec![0.0f32; m];
        for (i, x) in l.iter_mut().enumerate() {
            *x = drum(at + i, tempo, &mut noise);
        }
        let mut r = l.clone();
        let dry = l.clone();
        u.set_position(&cantus_engine::fx::SongPosition {
            playing: true,
            tempo_bpm: tempo,
            tempo_now: tempo,
            samples: at as i64,
            beats: at as f64 / 24_000.0,
            ..Default::default()
        });
        u.process(&mut l, &mut r, None);
        for (j, x) in dry.iter().enumerate() {
            let ph = ((at + j) % span) as f32 / span as f32;
            let c = ((ph * COLS as f32) as usize).min(COLS - 1);
            wave.peak[c] = wave.peak[c].max(x.abs());
            pairs.push(*x);
            pairs.push(l[j]);
        }
        at += m;
    }
    let tel = u.telemetry();
    app.fx.injected = Some(tel);
    app.fx.feed(&pairs);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.lapsus.effect != Some(effect) {
            app.fx.lapsus = State {
                effect: Some(effect),
                ..State::default()
            };
        }
    }
    app.fx.lapsus.wave = wave;
    // The pairs are taken: the next frame's feed sees nothing new to mark.
    app.fx.lapsus.wave.sig = (0, 1);
}

/// A drum loop at `tempo`: a kick on the beats, a snare on two and four, hats on the
/// eighths — enough for the waveform to show the beat.
fn drum(n: usize, tempo: f64, noise: &mut u32) -> f32 {
    use cantus_dsp::math::{exp, sin};
    let beat = (48_000.0 * 60.0 / tempo) as usize;
    let eighth = beat / 2;
    let i = n % beat;
    let b = (n / beat) % 4;
    let tk = i as f32 / 48_000.0;
    let sweep = 50.0 * tk + 90.0 * (1.0 - exp(-tk * 30.0)) / 30.0;
    let kick = sin(std::f32::consts::TAU * sweep) * exp(-tk * 9.0) * 0.8;
    *noise ^= *noise << 13;
    *noise ^= *noise >> 17;
    *noise ^= *noise << 5;
    let w = *noise as f32 / u32::MAX as f32 - 0.5;
    let snare = if b % 2 == 1 {
        w * exp(-tk * 18.0) * 0.9
    } else {
        0.0
    };
    let th = (n % eighth) as f32 / 48_000.0;
    let hat = w * exp(-th * 90.0) * 0.25;
    kick + snare + hat
}

/// The display's two voices, (time, level): in Lapsus's face the time curve is the second
/// accent's amber and the level the face's ice blue (the accent the kit paints its controls
/// in); in the kit's plain look the theme's hue and the faint line, as before.
pub(super) fn voices(s: &Style, ink: &Ink) -> (Color32, Color32) {
    match modfx::face::second_voice(s, ink) {
        Some(amber) => (amber, ink.hue),
        None => (ink.hue, ink.faint),
    }
}

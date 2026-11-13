//! Lapsus's timeline: one cycle of the pattern playing, the time curve over the cycle's own
//! input and the level curve under it, the playhead and the slip (fx side round, ADR 0132
//! provisional).
//!
//! The time lane reads downwards: its top line is *now* (the music as it arrives), its
//! bottom one cycle back. x and y share the cycle's length, so a slope is a speed you can
//! see: flat is the music, a line down to the right at the diagonal is the tape stopped,
//! half that slope half speed, steeper reverse. Below the diagonal lies what was before
//! the cycle began, which Lapsus never reads (silence): shaded, and named. While the unit
//! plays, the playhead crosses both lanes and **the slip** runs from where the song is to
//! where the music is read from, marked on the ruler.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::super::modfx;
use super::State;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::fx::lapsus as model;
use cantus_dsp::lapsus::{t, Pattern};
use cantus_dsp::modfx_clock::tel;
use cantus_engine::fx::fluctus::shape_at;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};
use std::collections::VecDeque;

/// Columns of the cycle's waveform.
pub const COLS: usize = 160;

/// The input's peak level across the cycle, column by column: what the timeline draws
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
    /// The analyser's newest pairs (the input first of each), the cycle's phase at the
    /// last of them and the cycle's length in samples.
    pub fn feed(&mut self, pairs: &VecDeque<(f32, f32)>, phase_end: f32, cycle: f32) {
        let sig = (
            pairs.len(),
            pairs
                .back()
                .map_or(0, |p| (u64::from(p.0.to_bits()) << 32) | u64::from(p.1.to_bits())),
        );
        if pairs.is_empty() || sig == self.sig || cycle < 1.0 {
            return;
        }
        self.sig = sig;
        let mut fresh = [0.0f32; COLS];
        let mut hit = [false; COLS];
        for (j, (x, _)) in pairs.iter().rev().enumerate() {
            let ph = phase_end - j as f32 / cycle;
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

/// The timeline's places.
#[derive(Copy, Clone, Debug)]
struct Map {
    time: Rect,
    level: Rect,
}

impl Map {
    fn x(&self, u: f32) -> f32 {
        self.time.left() + self.time.width() * u.clamp(0.0, 1.0)
    }
    /// The time lane: 0 (now) at the top, one cycle back at the bottom.
    fn yt(&self, d: f32) -> f32 {
        self.time.top() + self.time.height() * d.clamp(0.0, 1.0)
    }
    fn yl(&self, v: f32) -> f32 {
        self.level.bottom() - self.level.height() * v.clamp(0.0, 1.0)
    }
}

/// Whether the unit runs and its telemetry is fresh.
fn live(tel: &[f32; 64]) -> bool {
    tel[tel::BLOCKS] > 0.0 && tel[tel::PLAYING] > 0.5
}

#[allow(clippy::too_many_arguments)]
pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    slot: usize,
) {
    let p = super::pattern_of(host.spec(), slot);
    let pm = model::pattern_of(host.spec().lapsus.as_deref(), slot);
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
                let cycle = (p.beats * 60.0 / f64::from(tempo) * f64::from(host.sample_rate())) as f32;
                st.wave.feed(host.pairs(), tel[t::PHASE], cycle);
            }
            let resp = ui.interact(
                map.time.union(map.level),
                ui.id().with("lapsus-timeline"),
                egui::Sense::click_and_drag(),
            );
            paint(ui, s, r, map, st, tel, &p, pm, slot, tempo);
            if resp.hovered() {
                hint::set_text(
                    ui.ctx(),
                    frame::tr("lapsus.timeline_tip"),
                    frame::tr("lapsus.timeline_how"),
                    Caps::NONE,
                );
            }
            if p.is_through() && !live(tel) {
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
    p: &Pattern,
    pm: &model::Pattern,
    slot: usize,
    tempo: f32,
) {
    let ink = Ink::of(s);
    let k = s.k;
    let amber = ink.hue;
    let cyan = modfx::face::second_voice(s, &ink).unwrap_or(ink.faint);
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        lanes_grid(b, map, &ink, p.beats as f32, super::GRIDS[st.grid].1 as f32, k);
        before_the_cycle(b, map, &ink, k);
        wave(b, map, &st.wave, amber);
        // The time curve: the line you edit, glowing.
        const N: usize = 481;
        let mut time = [Pos2::ZERO; N];
        let mut level = [Pos2::ZERO; N];
        for j in 0..N {
            let u = j as f64 / (N - 1) as f64;
            let uu = u.min(0.999_999);
            let x = map.x(u as f32);
            time[j] = pos2(x, map.yt(p.time.at(uu, shape_at) as f32));
            level[j] = pos2(x, map.yl(p.vol.at(uu, shape_at) as f32));
        }
        b.area(&level, map.level.bottom(), with_alpha(cyan, 0.22));
        canvas::trace(b, &level, 1.6 * k, cyan, &ink);
        canvas::trace(b, &time, 2.2 * k, amber, &ink);
        for q in p.time.points() {
            node(b, pos2(map.x(q.x), map.yt(q.y)), amber, ink.ground, k);
        }
        for q in p.vol.points() {
            b.dot(pos2(map.x(q.x), map.yl(q.y)), 3.0 * k, cyan);
        }
        ruler(b, map, &ink, p.beats as f32, k);
        gutter(b, map, &ink, pm.len as usize, k);
        header(b, r, map, &ink, amber, cyan, slot, pm, tempo, k);
        if live(tel) {
            playhead(b, map, &ink, tel, amber, cyan, p.beats as f32, k);
        }
        canvas::edge_b(b, r, &ink);
    });
}

/// A point of the time curve: a hollow square, a sequencer's step.
fn node(b: &mut Brush, at: Pos2, c: Color32, ground: Color32, k: f32) {
    b.rect(Rect::from_center_size(at, vec2(8.0 * k, 8.0 * k)), Fill::Solid(c));
    b.rect(Rect::from_center_size(at, vec2(4.0 * k, 4.0 * k)), Fill::Solid(ground));
}

/// The beats, the grid's steps and the lanes' levels.
fn lanes_grid(b: &mut Brush, map: Map, ink: &Ink, beats: f32, step: f32, k: f32) {
    let steps = (beats / step.max(0.125)).round().max(1.0) as usize;
    for lane in [map.time, map.level] {
        for j in 1..steps {
            let u = j as f32 / steps as f32;
            let on_beat = ((u * beats).fract()).abs() < 1e-4;
            let c = if on_beat { ink.grid_strong } else { ink.grid };
            b.vline(map.x(u), lane.top(), lane.bottom(), b.px(), c);
        }
        b.hline(lane.left(), lane.right(), lane.top(), b.px(), ink.grid_strong);
        b.hline(lane.left(), lane.right(), lane.bottom(), b.px(), ink.grid_strong);
        b.vline(lane.left(), lane.top(), lane.bottom(), b.px(), ink.grid_strong);
        b.vline(lane.right(), lane.top(), lane.bottom(), b.px(), ink.grid_strong);
    }
    for q in [0.25f32, 0.5, 0.75] {
        b.hline(map.time.left(), map.time.right(), map.yt(q), b.px(), ink.grid);
        b.hline(map.level.left(), map.level.right(), map.yl(q), b.px(), ink.grid);
    }
    let _ = k;
}

/// What lies before the cycle's start (the delay past the time already played): never
/// read — shaded, its edge the stopped tape's diagonal, and named.
fn before_the_cycle(b: &mut Brush, map: Map, ink: &Ink, k: f32) {
    let a = pos2(map.x(0.0), map.yt(0.0));
    let c = pos2(map.x(1.0), map.yt(1.0));
    let d = pos2(map.x(0.0), map.yt(1.0));
    b.polygon(&[a, c, d], with_alpha(ink.faint, 0.16));
    canvas::dashed(b, a, c, b.px(), ink.grid_strong, 4.0 * k, 4.0 * k);
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

/// The cycle's input, faint, hanging from the *now* line: the music along the cycle, where
/// the curve reads it from.
fn wave(b: &mut Brush, map: Map, w: &Wave, c: Color32) {
    let lane = map.time;
    let top = lane.top() + 1.0;
    let deep = lane.height() * 0.30;
    let cw = lane.width() / COLS as f32;
    let col = with_alpha(c, 0.16);
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
            if j == 0 { ink.text } else { ink.dim },
            k,
            None,
            format_args!("{}", j + 1),
        );
    }
}

/// The lanes' scales in the gutter: *now* and a length back; full and silent.
fn gutter(b: &mut Brush, map: Map, ink: &Ink, len: usize, k: f32) {
    let x = map.time.left() - 6.0 * k;
    let back = super::LEN_KEYS[len.min(4)];
    for (key, y, align) in [
        ("lapsus.now", map.yt(0.0), Align2::RIGHT_TOP),
        (back, map.yt(1.0), Align2::RIGHT_BOTTOM),
    ] {
        let t = Text::key(key, Role::Micro, ink.dim);
        canvas::put(b, pos2(x, y), align, &t, k, None);
    }
    for (w, v, align) in [("100%", 1.0f32, Align2::RIGHT_TOP), ("0%", 0.0, Align2::RIGHT_BOTTOM)] {
        let t = Text::new(w, Role::Micro, ink.dim);
        canvas::put(b, pos2(x, map.yl(v)), align, &t, k, None);
    }
}

/// The lanes' names on the left, the pattern's name, length and tempo on the right.
#[allow(clippy::too_many_arguments)]
fn header(
    b: &mut Brush,
    r: Rect,
    map: Map,
    ink: &Ink,
    amber: Color32,
    cyan: Color32,
    slot: usize,
    pm: &model::Pattern,
    tempo: f32,
    k: f32,
) {
    let y = r.top() + 12.0 * k;
    let mut x = map.time.left();
    for (key, c) in [("lapsus.lane.time", amber), ("lapsus.lane.level", cyan)] {
        b.dot(pos2(x + 4.0 * k, y), 3.5 * k, c);
        let t = Text::key(key, Role::Micro, ink.text).caps(true);
        let w = canvas::put(b, pos2(x + 12.0 * k, y), Align2::LEFT_CENTER, &t, k, None);
        x = w.right() + 16.0 * k;
    }
    let name = pm
        .name
        .as_deref()
        .unwrap_or_else(|| super::slot_name(slot));
    canvas::putf(
        b,
        pos2(r.right() - 10.0 * k, y),
        Align2::RIGHT_CENTER,
        Role::Value,
        ink.text,
        k,
        Some(ink.plate),
        format_args!(
            "{}  \u{b7}  {}  \u{b7}  {:.0} BPM",
            name,
            frame::tr(super::LEN_KEYS[(pm.len as usize).min(4)]),
            tempo
        ),
    );
}

/// The playhead across both lanes, the read's place on the time curve, the slip back to
/// where the music is read from, and the level now.
#[allow(clippy::too_many_arguments)]
fn playhead(
    b: &mut Brush,
    map: Map,
    ink: &Ink,
    tel: &[f32; 64],
    amber: Color32,
    cyan: Color32,
    beats: f32,
    k: f32,
) {
    let u = tel[t::PHASE].clamp(0.0, 1.0);
    let d = tel[t::DELAY].clamp(0.0, 1.0);
    let v = tel[t::LEVEL].clamp(0.0, 1.0);
    let x = map.x(u);
    let head = with_alpha(ink.text, 0.85);
    b.vline(x, map.time.top() - 14.0 * k, map.level.bottom(), 1.2 * k, head);
    let at = pos2(x, map.yt(d));
    let read = (u - d).max(0.0);
    let rx = map.x(read);
    if d > 0.002 {
        // The slip: from the song's place to the place read, along the stopped tape's
        // diagonal, and its span on the ruler.
        canvas::dashed(b, at, pos2(rx, map.time.top()), 1.2 * k, with_alpha(amber, 0.8), 5.0 * k, 3.0 * k);
        let y = map.time.top() - 1.5 * k;
        b.hline(rx, x, y, 2.0 * k, with_alpha(amber, 0.55));
    }
    let tip = map.time.top();
    b.polygon(
        &[
            pos2(rx - 5.0 * k, tip - 9.0 * k),
            pos2(rx + 5.0 * k, tip - 9.0 * k),
            pos2(rx, tip - 1.0 * k),
        ],
        amber,
    );
    b.circle_stroke(at, 7.0 * k, 1.6 * k, amber);
    b.dot(at, 3.5 * k, amber);
    let lv = pos2(x, map.yl(v));
    b.circle_stroke(lv, 5.5 * k, 1.4 * k, cyan);
    b.dot(lv, 2.5 * k, cyan);
    // How far behind, and how fast the music runs there.
    canvas::tag(
        b,
        pos2((x + 8.0 * k).min(map.time.right() - 4.0 * k), map.time.bottom() - 4.0 * k),
        if x + 120.0 * k < map.time.right() {
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
            d * beats,
            frame::tr("lapsus.beats")
        ),
    );
}

/// The inspector's picture: the playing pattern, small, with its name.
pub fn preview(ui: &mut egui::Ui, spec: &cantus_core::EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let slot = (spec.get("slot").round().max(0.0) as usize).min(model::SLOTS - 1);
    let p = super::pattern_of(spec, slot);
    let w = ui.available_width();
    let h = 46.0 * s.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let ink = Ink::of(&s);
    let k = s.k;
    let cyan = modfx::face::second_voice(&s, &ink).unwrap_or(ink.faint);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let plot = Rect::from_min_max(
            pos2(rect.left() + 6.0 * k, rect.top() + 6.0 * k),
            pos2(rect.left() + rect.width() * 0.45, rect.bottom() - 6.0 * k),
        );
        thumb(b, plot, &p, ink.hue, cyan, k);
        let name = super::slot_name(slot);
        canvas::putf(
            b,
            pos2(plot.right() + 10.0 * k, rect.center().y),
            Align2::LEFT_CENTER,
            Role::Value,
            ink.text,
            k,
            None,
            format_args!("{} \u{b7} {}", slot + 1, name),
        );
        canvas::edge_b(b, rect, &ink);
    });
    resp
}

/// A pattern drawn small in `r`: the time curve over the top two thirds, the level under.
pub fn thumb(b: &mut Brush, r: Rect, p: &Pattern, amber: Color32, cyan: Color32, k: f32) {
    const N: usize = 49;
    let split = r.top() + r.height() * 0.66;
    let mut tp = [Pos2::ZERO; N];
    let mut lp = [Pos2::ZERO; N];
    for j in 0..N {
        let u = j as f64 / (N - 1) as f64;
        let uu = u.min(0.999_999);
        let x = r.left() + r.width() * u as f32;
        let d = p.time.at(uu, shape_at).clamp(0.0, 1.0) as f32;
        let v = p.vol.at(uu, shape_at).clamp(0.0, 1.0) as f32;
        tp[j] = pos2(x, r.top() + (split - r.top() - 1.5 * k) * d + 1.0 * k);
        lp[j] = pos2(x, r.bottom() - (r.bottom() - split - 1.5 * k) * v);
    }
    b.area(&lp, r.bottom(), with_alpha(cyan, 0.16));
    b.polyline(&lp, 1.0 * k, cyan);
    b.polyline(&tp, 1.4 * k, amber);
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
    let slot = (spec.get("slot").round().max(0.0) as usize).min(model::SLOTS - 1);
    let beats = model::pattern_of(spec.lapsus.as_deref(), slot).beats();
    let cycle = (beats * 60.0 / tempo * 48_000.0) as usize;
    // A bar and most of another: the playhead three quarters into the cycle.
    let total = cycle + cycle * 85 / 100;
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
            let ph = ((at + j) % cycle) as f32 / cycle as f32;
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
    let beat = (48_000.0 * 60.0 / tempo) as usize;
    let eighth = beat / 2;
    let i = n % beat;
    let b = (n / beat) % 4;
    let tk = i as f32 / 48_000.0;
    let kick = cantus_dsp::math::sin(std::f32::consts::TAU * (50.0 * tk + 90.0 * (1.0 - cantus_dsp::math::exp(-tk * 30.0)) / 30.0))
        * cantus_dsp::math::exp(-tk * 9.0)
        * 0.8;
    *noise ^= *noise << 13;
    *noise ^= *noise >> 17;
    *noise ^= *noise << 5;
    let w = *noise as f32 / u32::MAX as f32 - 0.5;
    let snare = if b % 2 == 1 {
        w * cantus_dsp::math::exp(-tk * 18.0) * 0.9
    } else {
        0.0
    };
    let th = (n % eighth) as f32 / 48_000.0;
    let hat = w * cantus_dsp::math::exp(-th * 90.0) * 0.25;
    kick + snare + hat
}

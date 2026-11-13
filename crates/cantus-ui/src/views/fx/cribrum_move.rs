//! Cribrum's movement card: the three sources that move the filters — the LFO, the
//! follower and the drawn curve — each a key with its depth and a live bar; the chosen one
//! drawn in time with the song on the lane beside them, its controls and its depths onto
//! the cutoff, the resonance and the vowel after it.

use super::super::canvas::{self, Ink};
use super::super::controls::{self, Size};
use super::super::frame;
use super::super::host::FxHost;
use super::super::modfx;
use super::State;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_dsp::cribrum::modulation::{wave, Wave};
use cantus_dsp::cribrum::{t, CribrumSettings, SEED};
use cantus_dsp::fluctus::Curve;
use cantus_dsp::modfx_clock::tel;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::collections::VecDeque;

/// The follower's recent movement, one value a block.
#[derive(Debug, Default)]
pub struct Trail {
    v: VecDeque<f32>,
    last: f32,
}

const TRAIL: usize = 240;

impl Trail {
    /// Fill it from a run (the screenshots' demo).
    pub fn seed(&mut self, vals: &[f32]) {
        self.v.clear();
        for x in vals.iter().skip(vals.len().saturating_sub(TRAIL)) {
            self.v.push_back(*x);
        }
    }

    fn push(&mut self, t: &[f32; 64]) {
        let blocks = t[tel::BLOCKS];
        if blocks > 0.0 && blocks != self.last {
            self.last = blocks;
            if self.v.len() == TRAIL {
                self.v.pop_front();
            }
            self.v.push_back(t[super::t::ENV]);
        }
    }
}

/// The source keys' column, the controls' column of switches, at 100 %.
const SRC_W: f32 = 168.0;
const SW_W: f32 = 152.0;

/// The sources' names (`cribrum.source.{i}`) and their parameters' prefixes.
const SOURCES: [(&str, &str); 3] = [
    ("cribrum.source.0", "lfo"),
    ("cribrum.source.1", "env"),
    ("cribrum.source.2", "cur"),
];

/// Each source's depth keys: cutoff, resonance, vowel.
const DEPTHS: [[&str; 3]; 3] = [
    ["lfo.cut", "lfo.res", "lfo.vowel"],
    ["env.cut", "env.res", "env.vowel"],
    ["cur.cut", "cur.res", "cur.vowel"],
];

/// The depths' cell labels: their target.
const DEPTH_LABELS: [&str; 3] = [
    "cribrum.target.cut",
    "cribrum.target.res",
    "cribrum.target.vowel",
];

pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel_now: &[f32; 64],
    cs: &CribrumSettings,
) {
    st.trail.push(tel_now);
    st.source = st.source.min(2);
    controls::card(
        ui,
        s,
        rect,
        Src::Key("cribrum.card.movement"),
        None,
        |ui, cr| {
            let k = s.k;
            let body = cr.body;
            // The keys of the sources.
            let src = Rect::from_min_size(body.min, vec2(SRC_W * k, body.height()));
            ui.allocate_rect(src, Sense::hover());
            source_keys(st, ui, s, src, tel_now, cs);
            // The controls, right-aligned in two groups; the lane between.
            let pad = GROUP_PAD * k;
            let ctl_w = (SW_W + 4.0 + controls::cells_width(&[Size::M; 4])) * k + 2.0 * pad;
            let lane = Rect::from_min_max(
                pos2(src.right() + 8.0 * k, body.top()),
                pos2(body.right() - ctl_w - 8.0 * k, body.bottom()),
            );
            ui.allocate_rect(lane, Sense::hover());
            lane_paint(ui, s, lane, st, tel_now, cs);
            let ctl = Rect::from_min_max(pos2(lane.right() + 8.0 * k, body.top()), body.max);
            let gh = (ctl.height() - 4.0 * k) * 0.5;
            let g1 = Rect::from_min_size(ctl.min, vec2(ctl.width(), gh));
            let g2 = Rect::from_min_size(pos2(ctl.left(), g1.bottom() + 4.0 * k), g1.size());
            let reach = reach_words(cs, st.source);
            with_brush(&ui.painter_at(ctl.expand(2.0)), |b| {
                for (g, key) in [(g1, SOURCES[st.source].0), (g2, "cribrum.depth")] {
                    crate::kit::light::sunken(b, g, 5.0 * k, s.t.well, 0.45, s.t.depth);
                    canvas::put(
                        b,
                        pos2(g.left() + pad, g.top() + 4.0 * k),
                        Align2::LEFT_TOP,
                        &Text::key(key, Role::Micro, s.t.text_dim).caps(true),
                        k,
                        None,
                    );
                }
                if let Some((label, value)) = &reach {
                    let at = pos2(g2.right() - pad, g2.top() + 4.0 * k);
                    canvas::put(
                        b,
                        at,
                        Align2::RIGHT_TOP,
                        &Text::new(label, Role::Micro, s.t.text_dim).caps(true),
                        k,
                        None,
                    );
                    canvas::put(
                        b,
                        pos2(at.x, g2.center().y + 4.0 * k),
                        Align2::RIGHT_CENTER,
                        &Text::new(value, Role::Value, s.hue).caps(false),
                        k,
                        None,
                    );
                }
            });
            let inner = |g: Rect| {
                Rect::from_min_max(
                    pos2(g.left() + pad, g.top() + 18.0 * k),
                    pos2(g.right() - pad, g.bottom() - 2.0 * k),
                )
            };
            let mut c1 = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner(g1))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            source_controls(host, st.source, &mut c1, s, cs);
            let mut c2 = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner(g2))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            depth_controls(host, st.source, &mut c2, s);
        },
    );
}

/// The inner margin of a group of controls, points at 100 %.
const GROUP_PAD: f32 = 8.0;

/// Where the chosen source can take filter A's cutoff, as words: ("Cutoff reach",
/// "280 Hz - 2.6 kHz"); `None` when it does not move the cutoff.
fn reach_words(cs: &CribrumSettings, source: usize) -> Option<(String, String)> {
    let d = [cs.lfo_depth, cs.env_depth, cs.curve_depth][source].cut;
    if d == 0.0 {
        return None;
    }
    let (lo, hi) = match source {
        0 => (-d.abs(), d.abs()),
        _ => (d.min(0.0), d.max(0.0)),
    };
    let c = cs.a.cut;
    let mut v = String::new();
    controls::write_readout(c * 2f32.powf(lo), "Hz", &mut v);
    v.push_str(" \u{2013} ");
    controls::write_readout(c * 2f32.powf(hi), "Hz", &mut v);
    Some((frame::tr("cribrum.reach").to_string(), v))
}

/// The three sources as keys: each its name, its depth onto the cutoff, and a bar with its
/// value now while the unit plays.
fn source_keys(
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tl: &[f32; 64],
    cs: &CribrumSettings,
) {
    let k = s.k;
    let gap = 6.0 * k;
    let h = (rect.height() - 2.0 * gap) / 3.0;
    let live = super::display::live(tl);
    let depths = [cs.lfo_depth, cs.env_depth, cs.curve_depth];
    for (i, (name, _)) in SOURCES.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rect.left(), rect.top() + i as f32 * (h + gap)),
            vec2(rect.width(), h),
        );
        let resp = ui.interact(r, ui.id().with(("cribrum-src", i)), Sense::click());
        if resp.clicked() {
            st.source = i;
        }
        let lit = st.source == i;
        let state = crate::kit::State::of(&resp, false).lit(lit);
        let d = depths[i];
        let mut words = String::new();
        if d.cut != 0.0 {
            let _ = std::fmt::Write::write_fmt(&mut words, format_args!("{:+.1} oct", d.cut));
        } else if d.vowel != 0.0 {
            let _ = std::fmt::Write::write_fmt(
                &mut words,
                format_args!("{:+.1} {}", d.vowel, frame::tr("cribrum.source.vowel")),
            );
        } else if d.res != 0.0 {
            let _ = std::fmt::Write::write_fmt(
                &mut words,
                format_args!("{:+.0}% {}", d.res * 100.0, frame::tr("cribrum.source.res")),
            );
        } else {
            words.push_str(frame::tr("cribrum.source.off"));
        }
        let value = match i {
            0 => 0.5 + 0.5 * tl[tel::MOD_L],
            1 => tl[t::ENV],
            _ => tl[t::CURVE],
        };
        with_brush(ui.painter(), |b| {
            controls::paint_key(b, r, s, state, Src::Str(""));
            let ink = if lit { s.t.text } else { s.t.text_dim };
            let pad = 8.0 * k;
            canvas::put(
                b,
                pos2(r.left() + pad, r.top() + 6.0 * k),
                Align2::LEFT_TOP,
                &Text::key(name, Role::Label, ink),
                k,
                None,
            );
            let on = d.cut != 0.0 || d.res != 0.0 || d.vowel != 0.0;
            canvas::put(
                b,
                pos2(r.right() - pad, r.top() + 6.0 * k),
                Align2::RIGHT_TOP,
                &Text::new(&words, Role::Value, if on { s.hue } else { s.t.text_dim }).caps(false),
                k,
                None,
            );
            // The bar: its value now, from the source's rest (0, or the LFO's middle).
            let bar = Rect::from_min_max(
                pos2(r.left() + pad, r.bottom() - 9.0 * k),
                pos2(r.right() - pad, r.bottom() - 6.0 * k),
            );
            b.rrect(bar, 1.5 * k, Fill::Solid(with_alpha(s.t.text_dim, 0.25)));
            if live && on {
                let from: f32 = if i == 0 { 0.5 } else { 0.0 };
                let (a, z) = (from.min(value), from.max(value));
                let fill = Rect::from_min_max(
                    pos2(bar.left() + bar.width() * a, bar.top()),
                    pos2(bar.left() + bar.width() * z.max(a + 0.01), bar.bottom()),
                );
                b.rrect(fill, 1.5 * k, Fill::Solid(s.hue));
            }
        });
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr(name),
                frame::tr("cribrum.source_tip"),
                Caps::NONE,
            );
        }
    }
}

/// The chosen source drawn in time: two cycles of the LFO, the follower's last seconds, or
/// one cycle of the curve; the playhead where the song is.
fn lane_paint(
    ui: &egui::Ui,
    s: &Style,
    r: Rect,
    st: &State,
    tl: &[f32; 64],
    cs: &CribrumSettings,
) {
    let ink = Ink::of(s);
    let k = s.k;
    let live = super::display::live(tl);
    let second = ink.voice(s.hue, true);
    let plot = Rect::from_min_max(
        pos2(r.left() + 8.0 * k, r.top() + 22.0 * k),
        pos2(r.right() - 8.0 * k, r.bottom() - 18.0 * k),
    );
    let tempo = if tl[tel::TEMPO] > 0.0 {
        tl[tel::TEMPO]
    } else {
        120.0
    };
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        b.hline(plot.left(), plot.right(), plot.center().y, b.px(), ink.grid);
        b.hline(plot.left(), plot.right(), plot.top(), b.px(), ink.grid);
        b.hline(plot.left(), plot.right(), plot.bottom(), b.px(), ink.grid);
        let x = |u: f32| plot.left() + plot.width() * u;
        let y = |v: f32| plot.bottom() - plot.height() * v.clamp(0.0, 1.0);
        let mut words = String::new();
        match st.source {
            0 => {
                let l = &cs.lfo;
                const M: usize = 241;
                let off = f64::from(l.phase) / 360.0;
                let st_off = f64::from(l.stereo) / 360.0;
                let pt = |j: usize, d: f64| {
                    let u = j as f64 / (M - 1) as f64 * 2.0;
                    let c = u + off + d;
                    let v = wave(l.wave, c - c.floor(), c.floor() as i64, l.edge, SEED);
                    pos2(x(u as f32 / 2.0), y(0.5 + 0.5 * v))
                };
                if l.stereo > 0.0 {
                    let right: Vec<Pos2> = (0..M).map(|j| pt(j, st_off)).collect();
                    b.polyline(&right, 1.0 * k, with_alpha(second, 0.7));
                }
                let left: Vec<Pos2> = (0..M).map(|j| pt(j, 0.0)).collect();
                b.area(&left, plot.center().y, ink.fill(0.35));
                canvas::trace(b, &left, 2.0 * k, ink.hue, &ink);
                b.vline(x(0.5), plot.top(), plot.bottom(), b.px(), ink.grid_strong);
                if live {
                    let ph = tl[tel::PHASE_L].clamp(0.0, 1.0) * 0.5;
                    b.vline(x(ph), plot.top(), plot.bottom(), 1.2 * k, second);
                    b.dot(pos2(x(ph), y(0.5 + 0.5 * tl[tel::MOD_L])), 3.5 * k, second);
                }
                if l.synced {
                    let i = cs_div_index(cs.lfo.beats);
                    let _ = std::fmt::Write::write_fmt(
                        &mut words,
                        format_args!("{} \u{b7} {:.0} BPM", super::DIV_NAMES[i], tempo),
                    );
                } else {
                    controls::write_readout(l.rate_hz, "Hz", &mut words);
                }
            }
            1 => {
                let n = st.trail.v.len();
                if n > 1 {
                    let pts: Vec<Pos2> = st
                        .trail
                        .v
                        .iter()
                        .enumerate()
                        .map(|(i, v)| pos2(x((i + TRAIL - n) as f32 / (TRAIL - 1) as f32), y(*v)))
                        .collect();
                    b.area(&pts, plot.bottom(), ink.fill(0.45));
                    canvas::trace(b, &pts, 1.8 * k, ink.hue, &ink);
                }
                if live {
                    let db = tl[t::ENV_DB];
                    if db > -150.0 {
                        let _ = std::fmt::Write::write_fmt(
                            &mut words,
                            format_args!("{} {:+.0} dBFS", frame::tr("cribrum.env.level"), db),
                        );
                    }
                }
                if words.is_empty() {
                    words.push_str(frame::tr("cribrum.env.quiet"));
                }
            }
            _ => {
                let curve = Curve::new(&cs.curve.points, cantus_engine::fx::fluctus::shape_at);
                const M: usize = 241;
                let pts: Vec<Pos2> = (0..M)
                    .map(|j| {
                        let u = j as f32 / (M - 1) as f32;
                        pos2(x(u), y(curve.at(u.min(0.99999))))
                    })
                    .collect();
                for q in 1..4 {
                    b.vline(x(q as f32 / 4.0), plot.top(), plot.bottom(), b.px(), ink.grid);
                }
                b.area(&pts, plot.bottom(), ink.fill(0.45));
                canvas::trace(b, &pts, 2.0 * k, ink.hue, &ink);
                for p in cs.curve.points.iter().flatten() {
                    b.dot(pos2(x(p.x), y(p.y)), 3.5 * k, ink.hue);
                }
                if live {
                    let ph = tl[t::CURVE_PHASE].clamp(0.0, 1.0);
                    b.vline(x(ph), plot.top(), plot.bottom(), 1.2 * k, second);
                    b.dot(pos2(x(ph), y(tl[t::CURVE])), 3.5 * k, second);
                }
                let i = cs_div_index(cs.curve.beats);
                let _ = std::fmt::Write::write_fmt(
                    &mut words,
                    format_args!("{} \u{b7} {:.0} BPM", super::DIV_NAMES[i], tempo),
                );
            }
        }
        canvas::put(
            b,
            pos2(r.left() + 8.0 * k, r.top() + 4.0 * k),
            Align2::LEFT_TOP,
            &Text::key(SOURCES[st.source].0, Role::Micro, ink.dim),
            k,
            None,
        );
        canvas::put(
            b,
            pos2(r.right() - 8.0 * k, r.top() + 4.0 * k),
            Align2::RIGHT_TOP,
            &Text::new(&words, Role::Value, ink.text).caps(false),
            k,
            Some(ink.plate),
        );
        canvas::edge_b(b, r, &ink);
    });
}

/// The index of a synced length by its beats.
fn cs_div_index(beats: f64) -> usize {
    cantus_dsp::cribrum::modulation::DIVISIONS
        .iter()
        .position(|d| (d - beats).abs() < 1e-9)
        .unwrap_or(8)
}

/// The chosen source's own controls: a column of switches, then a row of cells.
fn source_controls(
    host: &mut dyn FxHost,
    source: usize,
    ui: &mut egui::Ui,
    s: &Style,
    cs: &CribrumSettings,
) {
    let gap = egui::Vec2::new(s.d(4.0), s.d(4.0));
    ui.spacing_mut().item_spacing = gap;
    controls::column(ui, s, SW_W, |ui| match source {
        0 => {
            let labels = [Src::Key("modfx.sync.0"), Src::Key("modfx.sync.1")];
            controls::segmented_param(ui, s, host, "lfo.sync", &labels, SW_W);
            shape_keys(host, ui, s, SW_W);
        }
        1 => {
            let labels = [Src::Key("cribrum.key.0"), Src::Key("cribrum.key.1")];
            controls::segmented_param(ui, s, host, "env.key", &labels, SW_W);
            controls::caption(ui, s, Src::Key("cribrum.env.how"), SW_W);
        }
        _ => {
            controls::caption(ui, s, Src::Key("cribrum.curve.how"), SW_W);
        }
    });
    match source {
        0 => {
            if cs.lfo.synced {
                controls::param_with(ui, s, host, "lfo.div", Size::M, Some(&super::div_pill));
            } else {
                controls::param_with(ui, s, host, "lfo.rate", Size::M, Some(&modfx::hz_pill));
            }
            controls::param_with(ui, s, host, "lfo.phase", Size::M, Some(&modfx::deg_pill));
            controls::param_with(ui, s, host, "lfo.stereo", Size::M, Some(&modfx::deg_pill));
            let stepped = matches!(
                cs.lfo.wave,
                Wave::Square | Wave::SawUp | Wave::SawDown | Wave::Hold
            );
            ui.add_enabled_ui(stepped, |ui| {
                controls::param_with(ui, s, host, "lfo.edge", Size::M, Some(&modfx::pct_pill));
            });
        }
        1 => {
            controls::param(ui, s, host, "env.att", Size::M);
            controls::param(ui, s, host, "env.rel", Size::M);
            controls::param(ui, s, host, "env.gain", Size::M);
        }
        _ => {
            controls::param_with(ui, s, host, "cur.div", Size::M, Some(&super::div_pill));
            controls::param(ui, s, host, "cur.smooth", Size::M);
        }
    }
}

/// The chosen source's depths onto the cutoff, the resonance and the vowel.
fn depth_controls(host: &mut dyn FxHost, source: usize, ui: &mut egui::Ui, s: &Style) {
    let gap = egui::Vec2::new(s.d(4.0), s.d(4.0));
    ui.spacing_mut().item_spacing = gap;
    for (key, label) in DEPTHS[source].into_iter().zip(DEPTH_LABELS) {
        super::param_labelled(ui, s, host, key, label, Size::M, None);
    }
}

/// The LFO's seven waves as keys with their shapes drawn on them.
fn shape_keys(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, width: f32) {
    let now = host.spec().get("lfo.shape").round().max(0.0) as usize;
    let (rect, _) = ui.allocate_exact_size(vec2(width * s.k, s.d(controls::ROW)), Sense::hover());
    let n = Wave::ALL.len();
    let gap = s.d(2.0);
    let w = (rect.width() - gap * (n - 1) as f32) / n as f32;
    let name = frame::tr("modfx.p.shape");
    for (i, wv) in Wave::ALL.into_iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rect.left() + i as f32 * (w + gap), rect.top()),
            vec2(w, rect.height()),
        );
        let resp = ui.interact(r, ui.id().with(("cribrum-wave", i)), Sense::click());
        let st = crate::kit::State::of(&resp, false).lit(i == now);
        if ui.is_rect_visible(r) {
            with_brush(ui.painter(), |b| {
                controls::paint_key(b, r, s, st, Src::Str(""));
                let ink = if i == now { s.t.text } else { s.hue };
                wave_glyph(b, r.shrink2(vec2(3.0 * s.k, 5.0 * s.k)), wv, ink, s.k);
            });
        }
        let learnt = controls::stepped_offer(ui, s, host, "lfo.shape", name, &resp);
        if resp.clicked() && i != now && !learnt {
            host.set("lfo.shape", i as f32, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr(WAVE_KEYS[i]),
                frame::tr("modfx.shape_tip"),
                Caps::NONE,
            );
        }
    }
}

/// The waves' names (`cribrum.shape.{i}`).
const WAVE_KEYS: [&str; 7] = [
    "cribrum.shape.0",
    "cribrum.shape.1",
    "cribrum.shape.2",
    "cribrum.shape.3",
    "cribrum.shape.4",
    "cribrum.shape.5",
    "cribrum.shape.6",
];

/// One cycle of a wave (two for the random ones) in a small box.
fn wave_glyph(b: &mut Brush, r: Rect, w: Wave, ink: egui::Color32, k: f32) {
    const M: usize = 25;
    let cycles = if matches!(w, Wave::Hold | Wave::Drift) {
        3.0
    } else {
        1.0
    };
    let pts: Vec<Pos2> = (0..M)
        .map(|j| {
            let u = j as f64 / (M - 1) as f64;
            let c = (u * cycles).min(cycles - 1e-4);
            let v = wave(w, c - c.floor(), c.floor() as i64, 0.15, 11);
            pos2(r.left() + r.width() * u as f32, r.center().y - v * r.height() * 0.5)
        })
        .collect();
    b.polyline(&pts, 1.3 * k, ink);
}

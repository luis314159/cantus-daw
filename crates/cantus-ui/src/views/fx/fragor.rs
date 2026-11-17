//! Fragor's editor: the pedalboard (ADR 0128).
//!
//! The window is the board seen from above: up to three pedals standing on it, each in
//! its own finish with its knobs as the pedal labels them, its footswitch and its LED,
//! patch cables from the track's jack through them to the output — and, over the board,
//! what the chain does to a note at the level playing: its transfer (output against
//! input over a cycle — a curve where it clips, a loop where its capacitors remember) and
//! its harmonics, odd and even. Beside the board: the gate, the input, the mix and the
//! output; the oversampling and mono.
//!
//! The pedals are drawn by `pedal_draw` (reusable by any board of effects); this file
//! knows the five pedals and their parameters, and makes the drawing interactive.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::pedal_draw::{self as pd, Enclosure, Knob};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::skin::pedals;
use crate::kit::text::Src;
use crate::kit::{with_brush, Function, Style};
use cantus_core::fx::fragor::{KNOBS, MODELS, SLOTS, SWITCHES};
use cantus_core::{EffectId, EffectKind, StripId};
use egui::{pos2, vec2, Pos2, Rect, Sense, Vec2};
use std::fmt::Write as _;

#[path = "fragor_view.rs"]
mod view;
pub use view::{demo_feed, demo_feed_at, demo_setup, preview};

pub(crate) const KIND: EffectKind = EffectKind::Pedal;

/// Factory presets: data (`presets/effects/pedal.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// Each pedal's name and the line under it, by model index.
pub const NAMES: [&str; 5] = [
    "fragor.model.1",
    "fragor.model.2",
    "fragor.model.3",
    "fragor.model.4",
    "fragor.model.5",
];
/// Each pedal's case, its natural size: the overdrive and the op-amp distortion in the
/// common box, the silicon distortion a mini, the fuzz round, the muff wide.
pub const SHAPES: [pd::Shape; 5] = [
    pd::Shape::Standard,
    pd::Shape::Standard,
    pd::Shape::Mini,
    pd::Shape::Round,
    pd::Shape::Wide,
];
/// Each pedal's paint: metal flake, crinkle, metal flake, hammertone, brushed.
pub const TEXTURES: [pd::Texture; 5] = [
    pd::Texture::Sparkle,
    pd::Texture::Crinkle,
    pd::Texture::Sparkle,
    pd::Texture::Hammer,
    pd::Texture::Brushed,
];
/// Each pedal's lettering.
pub const LETTERS: [pd::Letter; 5] = [
    pd::Letter::Deco,
    pd::Letter::Technical,
    pd::Letter::Bold,
    pd::Letter::Script,
    pd::Letter::Wide,
];
/// Each pedal's artwork.
pub const MOTIFS: [pd::Motif; 5] = [
    pd::Motif::Waves,
    pd::Motif::Circuit,
    pd::Motif::Square,
    pd::Motif::Burst,
    pd::Motif::Nested,
];
pub const SUBS: [&str; 5] = [
    "fragor.sub.od",
    "fragor.sub.op",
    "fragor.sub.si",
    "fragor.sub.ge",
    "fragor.sub.mf",
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The chain's sound at the level playing, and what it was computed for.
    curves: view::Cache,
    /// The knob or footswitch under the pointer last frame: `(slot, part)`, part 0…2 a
    /// knob, 3 the footswitch.
    hot: Option<(usize, usize)>,
    /// The LEDs' glow, smoothed (by model).
    glow: [f32; 5],
    /// The tubes' glow, 0 … 1 (0.5 at rest).
    tubes: f32,
    /// Each pedal's view size (an index of [`VIEW_SIZES`]: compact, standard, large).
    pub view_sizes: [u8; 5],
    /// The clock the cables' streaks run by, when the screenshot tool sets it (else the
    /// window's own time).
    pub anim_time: Option<f64>,
    /// Where each slot's pedal was drawn last frame (the screenshot tool crops by them).
    pub pedal_rects: [Option<Rect>; 3],
    /// How far the board is scrolled sideways when its pedals do not fit (0 … 1).
    scroll: f32,
    /// A pedal being dragged to another slot: its slot and where it was grabbed.
    drag: Option<(usize, egui::Vec2)>,
    /// The input and output meters, dBFS, falling smoothly.
    meters: Meters,
    /// A colourway chosen in the ⋯ menu this frame, for the host to keep.
    colourway: Option<&'static str>,
}

/// Two meters, dBFS; new, at the floor.
#[derive(Copy, Clone, Debug)]
struct Meters([f32; 2]);

impl Default for Meters {
    fn default() -> Self {
        Meters([-90.0; 2])
    }
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.fragor);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "fragor"), &mut st, ui);
    // The colourway chosen is the user's for every Fragor, kept in the settings.
    if let Some(id) = st.colourway.take() {
        if let Some(name) = crate::kit::skin::colourway_unit(crate::kit::skin::Unit::Effect(KIND)) {
            app.set_unit_colourway(name, id);
        }
    }
    app.fx.fragor = st;
    out
}

/// Fragor's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    use cantus_dsp::fragor::telemetry as t;
    let lat = tel[t::LATENCY];
    let chip = [chips.latency(lat, sr)];
    for (m, slot) in st.meters.0.iter_mut().zip([t::IN_PEAK, t::OUT_PEAK]) {
        let v = tel[slot].max(-90.0);
        *m = if v > *m {
            v
        } else {
            (*m - 1.5).max(v).max(-90.0)
        };
    }
    let out_db = [st.meters.0[1], st.meters.0[1]];
    // The ⋯ menu's colourways (ADR 0127 §Colourways), each row with its swatch.
    let unit = crate::kit::skin::Unit::Effect(KIND);
    let ways = crate::kit::skin::colourways(unit);
    let wearing = crate::kit::skin::colourway_of(unit).map(|w| w.id);
    let dark = Style::current().t.dark;
    let mut more: Vec<crate::kit::menu::Item<'_>> = Vec::with_capacity(ways.len() + 1);
    more.push(crate::kit::menu::Item::Band(Src::Key("fragor.colourway")));
    for w in ways {
        more.push(crate::kit::menu::Item::Action {
            label: Src::Key(w.name_key),
            check: crate::kit::menu::Check::Swatch(
                crate::kit::skin::swatch(w.sheet, dark),
                wearing == Some(w.id),
            ),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: Some(crate::kit::meter::Levels {
            level: out_db,
            hold: out_db,
        }),
        more: &more,
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel)
    });
    st.chrome = chrome;
    st.chips = chips;
    // A colourway chosen: worn at once on this thread (an exported plug-in's window keeps it
    // for its session); Cantus's window also keeps it in the settings (`framed`).
    if let Some(w) = out
        .more
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| ways.get(i))
    {
        if let Some(name) = crate::kit::skin::colourway_unit(unit) {
            crate::kit::skin::set_colourway(name, w.id);
            st.colourway = Some(w.id);
        }
    }
    out
}

/// The side column's width at 100 %.
const SIDE_W: f32 = 236.0;
/// The display's height at 100 %.
const SOUND_H: f32 = 184.0;
/// The tube strip's height at 100 %.
const STRIP_H: f32 = 84.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let total = area.width() - gap;
    let side = SIDE_W * k;
    let sound_h = (SOUND_H * k).min(area.height() * 0.4);
    let st_tone = s.function(Function::Tone);
    let rows: Vec<Rect> = g.row(sound_h, &[1.0]).collect();
    let disp = (rows[0].height() / k - crate::kit::band::TITLE - 1.0).max(20.0);
    controls::card_with(
        ui,
        &st_tone,
        rows[0],
        Src::Key("fragor.card.sound"),
        None,
        Some(disp),
        |ui, cr| {
            if let Some(d) = cr.display {
                view::display(host, st, ui, &st_tone, d, tel);
                with_brush(&ui.painter_at(d), |b| {
                    super::valve_draw::glass(b, &st_tone, d)
                });
            }
        },
    );
    // The valve amp behind the board: its tubes glow steadily, a little brighter with the
    // signal (slowly — a lamp, never a meter), still at rest.
    {
        use cantus_dsp::fragor::telemetry as t;
        let peak = tel[t::OUT_PEAK];
        let target = if peak > -60.0 {
            0.5 + 0.5 * ((peak + 60.0) / 60.0).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let d = target - st.tubes;
        if d.abs() > 1e-3 {
            st.tubes += d * 0.08;
            ui.ctx().request_repaint();
        } else {
            st.tubes = target;
        }
    }
    let strip: Vec<Rect> = g.row(STRIP_H * k, &[1.0]).collect();
    with_brush(&ui.painter_at(strip[0]), |b| {
        super::valve_draw::strip(b, s, strip[0], st.tubes, Src::Key("picker.dev.fragor.name"));
    });
    let rest = g.rest();
    let cols: Vec<Rect> = g
        .row(rest.height(), &[(total - side) / total, side / total])
        .collect();
    board_ui(host, st, ui, s, cols[0], tel);
    side_ui(host, st.meters.0, ui, s, cols[1]);
}

/// Where the board's parts go in `r`.
struct BoardGeo {
    pedals: [Rect; 3],
    jack_in: Pos2,
    jack_out: Pos2,
    /// The board's content is wider than its window: its width, and how far it is scrolled.
    overflow: Option<(f32, f32)>,
}

/// The view-size choices of a pedal (compact, standard, large): a preference of the view,
/// never heard.
pub const VIEW_SIZES: [f32; 3] = [0.85, 1.0, 1.15];

/// The board's layout in `r`: each slot's pedal at its natural size (its shape, times its
/// view size), the row fitted to the window — scaled down as far as a legible floor, then
/// scrolled sideways from `scroll` (0 … 1) — and bottom-aligned on the board.
/// Where a slot's numbered tag sits: centred above its pedal (or its empty place).
fn slot_tag_rect(pedal: Rect, k: f32) -> Rect {
    Rect::from_center_size(
        pos2(pedal.center().x, pedal.top() - 15.0 * k),
        vec2(16.0 * k, 13.0 * k),
    )
}

fn board_geo(
    r: Rect,
    k: f32,
    shapes: [Option<pd::Shape>; 3],
    views: [f32; 3],
    scroll: f32,
) -> BoardGeo {
    let margin = 44.0 * k;
    let gap = 26.0;
    let inner = Rect::from_min_max(
        pos2(r.left() + margin, r.top() + 12.0 * k),
        pos2(r.right() - margin, r.bottom() - 14.0 * k),
    );
    // Natural sizes at 100 % (an empty slot: a narrow place to add one).
    let sizes: [Vec2; 3] = std::array::from_fn(|i| match shapes[i] {
        Some(sh) => pd::size_of(sh) * views[i],
        None => Vec2::new(124.0, pd::SIZE.y),
    });
    let total_w = sizes.iter().map(|s| s.x).sum::<f32>() + 2.0 * gap;
    let max_h = sizes.iter().map(|s| s.y).fold(0.0, f32::max);
    let avail_h = inner.height() - 64.0 * k;
    // As large as the window allows; never under 55 % of the window's own scale.
    let fit = (avail_h / max_h).min(inner.width() / total_w);
    let floor = 0.55 * k;
    let scale = fit.max(floor);
    let content_w = total_w * scale;
    let overflow = (content_w > inner.width() + 0.5).then_some(content_w);
    let shift = overflow.map_or(0.0, |w| (w - inner.width()) * scroll.clamp(0.0, 1.0));
    let start = if overflow.is_some() {
        inner.left() - shift
    } else {
        inner.center().x - content_w * 0.5
    };
    let mut pedals = [Rect::NOTHING; 3];
    let mut x = start;
    for i in 0..3 {
        let sz = sizes[i] * scale;
        pedals[i] = Rect::from_min_size(pos2(x, inner.bottom() - sz.y - 4.0 * k), sz);
        x += sz.x + gap * scale;
    }
    let jy = inner.bottom() - max_h * scale - 6.0 * k;
    BoardGeo {
        pedals,
        jack_in: pos2(r.left() + 20.0 * k, jy),
        jack_out: pos2(r.right() - 20.0 * k, jy),
        overflow: overflow.map(|w| (w, shift)),
    }
}

fn board_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    tel: &[f32; 64],
) {
    use cantus_dsp::fragor::telemetry as t;
    let k = s.k;
    let spec = host.spec().clone();
    let dark = s.t.dark;
    let shapes: [Option<pd::Shape>; 3] =
        std::array::from_fn(|i| model_of(spec.get(SLOTS[i])).map(|m| SHAPES[m]));
    let views: [f32; 3] = std::array::from_fn(|i| {
        model_of(spec.get(SLOTS[i])).map_or(1.0, |m| VIEW_SIZES[st.view_sizes[m] as usize % 3])
    });
    let geo = board_geo(r, k, shapes, views, st.scroll);
    st.pedal_rects = geo.pedals.map(Some);
    let chain: [Option<usize>; 3] = std::array::from_fn(|i| model_of(spec.get(SLOTS[i])));
    // The LEDs follow each pedal's output, smoothed (a lamp, not a meter).
    for m in 0..5 {
        let on = spec.get(SWITCHES[m]) >= 0.5;
        let peak = tel[t::PEDAL_PEAK + m];
        let target = if on && peak > -60.0 {
            ((peak + 60.0) / 60.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        st.glow[m] += (target - st.glow[m]) * 0.35;
    }
    let hot = st.hot;
    let gate_on = spec.get("gate") > cantus_core::fx::fragor::GATE_OFF_DB;
    let time = st.anim_time.unwrap_or_else(|| ui.input(|i| i.time));
    let mut flowing = false;
    let mut hits: [Option<pd::Hits>; 3] = [None; 3];
    with_brush(&ui.painter_at(r), |b| {
        pd::board(b, s, r, dark);
        let ink = pd::board_ink(s);
        pd::board_jack(b, s, geo.jack_in, Src::Key("fragor.jack.in"), ink);
        pd::board_jack(b, s, geo.jack_out, Src::Key("fragor.jack.out"), ink);
        let dragging = st.drag.map(|d| d.0);
        let pointer = ui.ctx().pointer_latest_pos();
        let mut lifted = None;
        for (i, m) in chain.iter().enumerate() {
            match m {
                Some(m) if dragging == Some(i) => {
                    // Lifted off the board: drawn last, where the hand is.
                    let r = geo.pedals[i].shrink2(vec2(6.0 * k, 0.0));
                    pd::empty_slot(b, s, r, Src::Key("fragor.add"), false);
                    lifted = Some(*m);
                }
                Some(m) => {
                    let on = spec.get(SWITCHES[*m]) >= 0.5;
                    let labels = knob_labels(*m);
                    let knobs: [Knob; 3] = std::array::from_fn(|j| Knob {
                        label: Src::Key(labels[j]),
                        value: spec.get(KNOBS[*m][j]),
                        hot: hot == Some((i, j)),
                    });
                    let e = Enclosure {
                        finish: &pedals::FRAGOR[*m],
                        name: Src::Key(NAMES[*m]),
                        sub: Src::Key(SUBS[*m]),
                        motif: MOTIFS[*m],
                        letter: LETTERS[*m],
                        texture: TEXTURES[*m],
                        shape: SHAPES[*m],
                        knobs: &knobs,
                        on,
                        glow: if on { 0.25 + 0.75 * st.glow[*m] } else { 0.0 },
                        lifted: false,
                        switch_hot: hot == Some((i, 3)),
                    };
                    hits[i] = Some(pd::enclosure(b, s, geo.pedals[i], &e));
                }
                None => {
                    let r = geo.pedals[i].shrink2(vec2(6.0 * k, 0.0));
                    pd::empty_slot(b, s, r, Src::Key("fragor.add"), false);
                }
            }
        }
        // Where a dragged pedal would land, and the pedal itself in the hand.
        if let (Some(m), Some(p), Some((from, grab))) = (lifted, pointer, st.drag) {
            if let Some(to) = slot_at(&geo, p).filter(|t| *t != from) {
                let t = geo.pedals[to].expand(6.0 * k);
                pd::drop_marker(b, s, t.left(), t.top(), t.bottom());
                pd::drop_marker(b, s, t.right(), t.top(), t.bottom());
            }
            let labels = knob_labels(m);
            let knobs: [Knob; 3] = std::array::from_fn(|j| Knob {
                label: Src::Key(labels[j]),
                value: spec.get(KNOBS[m][j]),
                hot: false,
            });
            let e = Enclosure {
                finish: &pedals::FRAGOR[m],
                name: Src::Key(NAMES[m]),
                sub: Src::Key(SUBS[m]),
                motif: MOTIFS[m],
                letter: LETTERS[m],
                texture: TEXTURES[m],
                shape: SHAPES[m],
                knobs: &knobs,
                on: spec.get(SWITCHES[m]) >= 0.5,
                glow: 0.3,
                lifted: true,
                switch_hot: false,
            };
            let r = Rect::from_min_size(p - grab, geo.pedals[from].size());
            pd::enclosure(b, s, r, &e);
        }
        // The cables: the track's jack → each pedal in turn → the output, the signal
        // flowing in them as loud as it is at that point (after the gate; after each pedal
        // its output, or what passes it when its footswitch is off).
        let lv = |db: f32| ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let mut level = if tel[t::GATE] > 0.0 || !gate_on {
            lv(tel[t::IN_PEAK])
        } else {
            0.0
        };
        let mut from = geo.jack_in;
        let lift = 34.0 * k;
        for (i, h) in hits.iter().enumerate() {
            let Some(h) = h else { continue };
            pd::cable_with(
                b,
                s,
                from,
                h.jack_in,
                lift,
                Some(pd::Energy { level, time }),
            );
            if let Some(m) = chain[i] {
                if spec.get(SWITCHES[m]) >= 0.5 {
                    level = lv(tel[t::PEDAL_PEAK + m]);
                }
            }
            from = h.jack_out;
        }
        let out_level = lv(tel[t::OUT_PEAK]).min(level.max(lv(tel[t::OUT_PEAK])));
        pd::cable_with(
            b,
            s,
            from,
            geo.jack_out,
            lift,
            Some(pd::Energy {
                level: out_level,
                time,
            }),
        );
        flowing = level > 0.01 || out_level > 0.01;
        // Each slot's numbered tag above it: its place in the chain.
        for (i, n) in ["1", "2", "3"].iter().enumerate() {
            let tr = slot_tag_rect(geo.pedals[i], k);
            pd::slot_tag(b, s, tr, n, ui.rect_contains_pointer(tr));
        }
    });
    // While the signal flows the streaks move: repaint; at silence nothing does.
    if flowing && st.anim_time.is_none() {
        ui.ctx().request_repaint();
    }
    // A board wider than its window scrolls sideways: a kit scrollbar at its foot, and the
    // wheel (Shift+wheel, or a sideways wheel) over it.
    if let Some((content, _)) = geo.overflow {
        let bar = Rect::from_min_max(
            pos2(r.left() + 12.0 * k, r.bottom() - 9.0 * k),
            pos2(r.right() - 12.0 * k, r.bottom() - 3.0 * k),
        );
        let len = (r.width() - 88.0 * k) / content;
        let mut start = st.scroll * (1.0 - len).max(0.0);
        crate::kit::widgets::scrollbar(
            ui,
            bar,
            ui.id().with("fragor-board-scroll"),
            &mut start,
            len,
        );
        if ui.rect_contains_pointer(r) {
            let d = ui.input(|i| i.smooth_scroll_delta);
            let dx = if d.x != 0.0 {
                d.x
            } else if ui.input(|i| i.modifiers.shift) {
                d.y
            } else {
                0.0
            };
            start = (start - dx / content).clamp(0.0, (1.0 - len).max(0.0));
        }
        st.scroll = if len < 1.0 { start / (1.0 - len) } else { 0.0 };
    }
    // Interaction. Registered before the knobs and the footswitch so they win where they
    // overlap: each pedal's body drags it to another slot (the two swap), right-click
    // offers to replace or remove it; an empty slot clicked offers the pedals not on the
    // board.
    st.hot = None;
    let pointer = ui.ctx().pointer_latest_pos();
    for i in 0..3 {
        let id = ui.id().with(("fragor-slot", i));
        let resp = ui.interact(geo.pedals[i], id, Sense::click_and_drag());
        // The slot's tag: the slot parameter's own control (automate, modulate, learn).
        let tag = ui.interact(
            slot_tag_rect(geo.pedals[i], k),
            id.with("tag"),
            Sense::click(),
        );
        let slot_name = KIND.param(SLOTS[i]).map_or("", |p| frame::tr(p.label_key));
        controls::stepped_offer(ui, s, host, SLOTS[i], slot_name, &tag);
        if tag.hovered() {
            let what = chain[i].map_or_else(|| frame::tr("fragor.add"), |m| frame::tr(NAMES[m]));
            crate::kit::hint::set_text(ui.ctx(), slot_name, what, crate::kit::hint::Caps::NONE);
        }
        match chain[i] {
            Some(m) => {
                if resp.drag_started() {
                    if let Some(p) = pointer {
                        st.drag = Some((i, p - geo.pedals[i].min));
                    }
                }
                if resp.drag_stopped() {
                    if let (Some((from, _)), Some(p)) = (st.drag.take(), pointer) {
                        if let Some(to) = slot_at(&geo, p) {
                            if to != from {
                                let a = spec.get(SLOTS[from]);
                                let b = spec.get(SLOTS[to]);
                                host.set_many(
                                    &[(SLOTS[from], b), (SLOTS[to], a)],
                                    "fragor.cmd.move",
                                );
                            }
                        }
                    }
                }
                if let Some(v) = pedal_menu(host, &resp, i, m, &chain, st.view_sizes[m]) {
                    st.view_sizes[m] = v;
                }
            }
            None => {
                add_menu(host, &resp, i, &chain);
                if resp.hovered() {
                    crate::kit::hint::set_text(
                        ui.ctx(),
                        frame::tr("fragor.add"),
                        "",
                        crate::kit::hint::Caps::NONE,
                    );
                }
            }
        }
    }
    // The knobs drag up and down (Shift: finely), a double click resets, a right-click
    // opens the parameter's menu (automate, modulate, learn, reset); the footswitch clicks.
    for (i, h) in hits.iter().enumerate() {
        let (Some(h), Some(m)) = (h, chain[i]) else {
            continue;
        };
        for (j, (knob, key)) in h.knobs.iter().zip(KNOBS[m]).take(h.n).enumerate() {
            let id = ui.id().with(("fragor-knob", i, j));
            let resp = ui.interact(*knob, id, Sense::click_and_drag());
            if resp.hovered() || resp.dragged() {
                st.hot = Some((i, j));
            }
            if resp.dragged() {
                let dy = resp.drag_delta().y;
                let fine = if ui.input(|i| i.modifiers.shift) {
                    0.2
                } else {
                    1.0
                };
                let v = (spec.get(key) - dy / (180.0 * k) * fine).clamp(0.0, 1.0);
                host.set(key, v, Some(key));
            }
            if resp.drag_stopped() {
                host.end_gesture();
            }
            let default = KIND.param(key).map_or(0.5, |p| p.default);
            if resp.double_clicked() {
                host.set(key, default, None);
            }
            let offer = crate::kit::automate::Offer {
                automated: host.can_automate() && host.automated(key),
                typing: false,
                modulation: None,
            };
            match crate::kit::automate::menu(ui, &resp, offer) {
                Some(crate::kit::automate::Choice::Automate) => host.automate(key),
                Some(crate::kit::automate::Choice::Reset) => host.set(key, default, None),
                Some(crate::kit::automate::Choice::Type) | None => {}
                Some(c) => host.modulate(key, c, resp.rect),
            }
            if resp.hovered() || resp.dragged() {
                // The help line: the parameter's whole name (the pedal's and the knob's)
                // and its value.
                let name = KIND.param(key).map_or("", |p| frame::tr(p.label_key));
                let v = spec.get(key);
                crate::kit::hint::set(
                    ui.ctx(),
                    name,
                    crate::kit::hint::Caps::PARAM,
                    resp.dragged(),
                    |b| write!(b, "{:.0} %", v * 100.0),
                );
            }
        }
        let id = ui.id().with(("fragor-switch", i));
        let resp = ui.interact(h.switch, id, Sense::click());
        let key = SWITCHES[m];
        let name = KIND.param(key).map_or("", |p| frame::tr(p.label_key));
        let learnt = controls::stepped_offer(ui, s, host, key, name, &resp);
        let on = spec.get(key) >= 0.5;
        if resp.hovered() {
            st.hot = Some((i, 3));
            let word = frame::tr(if on {
                "fragor.onoff.1"
            } else {
                "fragor.onoff.0"
            });
            crate::kit::hint::set_text(ui.ctx(), name, word, crate::kit::hint::Caps::NONE);
        }
        if resp.clicked() && !learnt {
            host.set(key, if on { 0.0 } else { 1.0 }, None);
        }
    }
    if !ui.input(|i| i.pointer.any_down()) {
        st.drag = None;
    }
}

/// The slot under `p`, if any.
fn slot_at(geo: &BoardGeo, p: Pos2) -> Option<usize> {
    (0..3).find(|i| {
        let r = geo.pedals[*i];
        let half = r.width() * 0.5 + (r.width() * 0.3);
        (p.x - r.center().x).abs() <= half && p.y >= r.top() - r.height() * 0.3
    })
}

/// A pedal's own menu (a right-click on its body): another pedal in its place (one not
/// on the board), or off the board.
fn pedal_menu(
    host: &mut dyn FxHost,
    resp: &egui::Response,
    slot: usize,
    model: usize,
    chain: &[Option<usize>; 3],
    view: u8,
) -> Option<u8> {
    use crate::kit::menu::{Check, Item};
    let mut chosen = None;
    let mut sized = None;
    egui::Popup::context_menu(resp)
        .frame(crate::kit::automate::frame())
        .show(|ui| {
            let others: Vec<usize> = (0..MODELS.len())
                .filter(|m| *m != model && !chain.contains(&Some(*m)))
                .collect();
            let mut items: Vec<Item> = others
                .iter()
                .map(|m| Item::action(Src::Key(NAMES[*m])))
                .collect();
            items.push(Item::action(Src::Key("fragor.remove")));
            items.push(Item::Separator);
            for (v, key) in ["fragor.size.0", "fragor.size.1", "fragor.size.2"]
                .iter()
                .enumerate()
            {
                items.push(Item::Action {
                    label: Src::Key(key),
                    check: if v as u8 == view {
                        Check::Radio(true)
                    } else {
                        Check::Radio(false)
                    },
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                });
            }
            if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
                if i < others.len() {
                    chosen = Some(others[i] as f32 + 1.0);
                } else if i == others.len() {
                    chosen = Some(0.0);
                } else if i > others.len() + 1 {
                    sized = Some((i - others.len() - 2) as u8);
                }
                ui.close();
            }
        });
    if let Some(v) = chosen {
        let label = if v == 0.0 {
            "fragor.cmd.remove"
        } else {
            "fragor.cmd.add"
        };
        host.set_many(&[(SLOTS[slot], v)], label);
    }
    sized
}

/// An empty slot's menu (a click): the pedals not on the board.
fn add_menu(host: &mut dyn FxHost, resp: &egui::Response, slot: usize, chain: &[Option<usize>; 3]) {
    use crate::kit::menu::Item;
    let mut chosen = None;
    egui::Popup::menu(resp)
        .frame(crate::kit::automate::frame())
        .show(|ui| {
            let free: Vec<usize> = (0..MODELS.len())
                .filter(|m| !chain.contains(&Some(*m)))
                .collect();
            let items: Vec<Item> = free
                .iter()
                .map(|m| Item::action(Src::Key(NAMES[*m])))
                .collect();
            if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
                chosen = Some(free[i]);
                ui.close();
            }
        });
    if let Some(m) = chosen {
        host.set_many(
            &[(SLOTS[slot], m as f32 + 1.0), (SWITCHES[m], 1.0)],
            "fragor.cmd.add",
        );
    }
}

/// The knobs' names as each pedal labels them (on the face; the parameters' own names
/// carry the pedal's, for automation lanes and menus).
pub const FACE: [[&str; 3]; 5] = [
    ["fragor.knob.drive", "fragor.knob.tone", "fragor.knob.level"],
    [
        "fragor.knob.distortion",
        "fragor.knob.filter",
        "fragor.knob.volume",
    ],
    ["fragor.knob.dist", "fragor.knob.tone", "fragor.knob.level"],
    ["fragor.knob.fuzz", "fragor.knob.bias", "fragor.knob.volume"],
    [
        "fragor.knob.sustain",
        "fragor.knob.tone",
        "fragor.knob.volume",
    ],
];

pub fn knob_labels(m: usize) -> [&'static str; 3] {
    FACE[m]
}

fn side_ui(host: &mut dyn FxHost, meters: [f32; 2], ui: &mut egui::Ui, s: &Style, r: Rect) {
    let k = s.k;
    let mut g = Grid::new(r, s);
    let ch = controls::card_height(Size::M) * k;
    let q_h = (crate::kit::band::TITLE + 2.0 * controls::ROW + 34.0) * k;
    let sd = s.function(Function::Dynamics);
    let r1: Vec<Rect> = g.row(ch, &[1.0]).collect();
    controls::card(
        ui,
        &sd,
        r1[0],
        Src::Key("fragor.card.input"),
        None,
        |ui, _| {
            controls::param_with(ui, &sd, host, "gate", Size::M, Some(&gate_pill));
            controls::param(ui, &sd, host, "in", Size::M);
        },
    );
    let so = s.function(Function::Output);
    let r2: Vec<Rect> = g.row(ch, &[1.0]).collect();
    controls::card(
        ui,
        &so,
        r2[0],
        Src::Key("fragor.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "mix", Size::M, Some(&pct));
            controls::param(ui, &so, host, "out", Size::M);
        },
    );
    let r3: Vec<Rect> = g.row(q_h, &[1.0]).collect();
    controls::card(
        ui,
        &so,
        r3[0],
        Src::Key("fragor.card.quality"),
        None,
        |ui, _| {
            let w = (r3[0].width() - 24.0 * k) / k;
            controls::column(ui, &so, w, |ui| {
                controls::segmented_param(
                    ui,
                    &so,
                    host,
                    "os",
                    &[
                        Src::Key("fragor.os.0"),
                        Src::Key("fragor.os.1"),
                        Src::Key("fragor.os.2"),
                    ],
                    w,
                );
                controls::segmented_param(
                    ui,
                    &so,
                    host,
                    "mono",
                    &[Src::Key("fragor.chan.0"), Src::Key("fragor.chan.1")],
                    w,
                );
            });
        },
    );
    let rest = g.rest();
    if rest.height() > 60.0 * k {
        let r4: Vec<Rect> = g.row(rest.height(), &[1.0]).collect();
        let disp = (r4[0].height() / k - crate::kit::band::TITLE - 1.0).max(20.0);
        controls::card_with(
            ui,
            &so,
            r4[0],
            Src::Key("fragor.card.levels"),
            None,
            Some(disp),
            |ui, cr| {
                if let Some(d) = cr.display {
                    view::meters(ui, &so, d, meters);
                }
            },
        );
    }
}

fn pct(v: f32, b: &mut String) {
    super::modfx::pct_pill(v, b);
}

fn gate_pill(v: f32, b: &mut String) {
    if v <= cantus_core::fx::fragor::GATE_OFF_DB {
        b.push_str(frame::tr("fragor.onoff.0"));
    } else {
        let _ = write!(b, "{v:.0} dB");
    }
}

/// The model in a slot's parameter value (0 empty).
pub fn model_of(v: f32) -> Option<usize> {
    let v = v.round() as usize;
    (1..=MODELS.len()).contains(&v).then(|| v - 1)
}

/// Where the board's pedals stand in a board of `r` at scale `k` (for the tests and the
/// screenshot tool).
pub fn pedal_rects(r: Rect, k: f32) -> [Rect; 3] {
    board_geo(r, k, [Some(pd::Shape::Standard); 3], [1.0; 3], 0.0).pedals
}

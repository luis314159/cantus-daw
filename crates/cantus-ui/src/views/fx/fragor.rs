//! Fragor's editor: the pedalboard (ADR 0131).
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
use egui::{pos2, vec2, Pos2, Rect, Sense};
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
    /// The input and output meters, dBFS, falling smoothly.
    meters: Meters,
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
        *m = if v > *m { v } else { (*m - 1.5).max(v).max(-90.0) };
    }
    let out_db = [st.meters.0[1], st.meters.0[1]];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: Some(crate::kit::meter::Levels {
            level: out_db,
            hold: out_db,
        }),
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The side column's width at 100 %.
const SIDE_W: f32 = 236.0;
/// The display's height at 100 %.
const SOUND_H: f32 = 196.0;

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
            }
        },
    );
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
}

fn board_geo(r: Rect, k: f32) -> BoardGeo {
    let margin = 44.0 * k;
    let inner = Rect::from_min_max(
        pos2(r.left() + margin, r.top() + 12.0 * k),
        pos2(r.right() - margin, r.bottom() - 10.0 * k),
    );
    let w = inner.width() / 3.0;
    // The pedals' scale: as large as the slot allows, their proportions kept.
    let avail_h = inner.height() - 64.0 * k;
    let scale = (avail_h / pd::SIZE.y)
        .min((w - 18.0 * k) / pd::SIZE.x)
        .max(0.3 * k);
    let size = pd::SIZE * scale;
    let mut pedals = [Rect::NOTHING; 3];
    for (i, p) in pedals.iter_mut().enumerate() {
        let x = inner.left() + w * (i as f32 + 0.5);
        let c = pos2(x, inner.bottom() - size.y * 0.5 - 6.0 * k);
        *p = Rect::from_center_size(c, size);
    }
    let jy = pedals[0].top() - 2.0 * k;
    BoardGeo {
        pedals,
        jack_in: pos2(r.left() + 20.0 * k, jy),
        jack_out: pos2(r.right() - 20.0 * k, jy),
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
    let geo = board_geo(r, k);
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
    let mut hits: [Option<pd::Hits>; 3] = [None; 3];
    with_brush(&ui.painter_at(r), |b| {
        pd::board(b, s, r, dark);
        let ink = s.t.text_dim;
        pd::board_jack(b, s, geo.jack_in, Src::Key("fragor.jack.in"), ink);
        pd::board_jack(b, s, geo.jack_out, Src::Key("fragor.jack.out"), ink);
        for (i, m) in chain.iter().enumerate() {
            match m {
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
        // The cables: the track's jack → each pedal in turn → the output.
        let mut from = geo.jack_in;
        let lift = 34.0 * k;
        for h in hits.iter().flatten() {
            pd::cable(b, s, from, h.jack_in, lift);
            from = h.jack_out;
        }
        pd::cable(b, s, from, geo.jack_out, lift);
    });
    // Interaction: the knobs drag up and down (Shift: finely), a double click resets;
    // the footswitch clicks.
    st.hot = None;
    for (i, h) in hits.iter().enumerate() {
        let (Some(h), Some(m)) = (h, chain[i]) else {
            continue;
        };
        for j in 0..h.n {
            let id = ui.id().with(("fragor-knob", i, j));
            let resp = ui.interact(h.knobs[j], id, Sense::click_and_drag());
            let key = KNOBS[m][j];
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
            if resp.double_clicked() {
                let d = KIND.param(key).map_or(0.5, |p| p.default);
                host.set(key, d, None);
            }
            let mut tip = String::new();
            let _ = write!(
                tip,
                "{} {:.0} %",
                frame::tr(knob_labels(m)[j]),
                spec.get(key) * 100.0
            );
            resp.on_hover_text(tip);
        }
        let id = ui.id().with(("fragor-switch", i));
        let resp = ui.interact(h.switch, id, Sense::click());
        if resp.hovered() {
            st.hot = Some((i, 3));
        }
        if resp.clicked() {
            let key = SWITCHES[m];
            let on = spec.get(key) >= 0.5;
            host.set(key, if on { 0.0 } else { 1.0 }, None);
        }
    }
}

/// The knobs' names as each pedal labels them (on the face; the parameters' own names
/// carry the pedal's, for automation lanes and menus).
pub const FACE: [[&str; 3]; 5] = [
    ["fragor.knob.drive", "fragor.knob.tone", "fragor.knob.level"],
    ["fragor.knob.distortion", "fragor.knob.filter", "fragor.knob.volume"],
    ["fragor.knob.dist", "fragor.knob.tone", "fragor.knob.level"],
    ["fragor.knob.fuzz", "fragor.knob.bias", "fragor.knob.volume"],
    ["fragor.knob.sustain", "fragor.knob.tone", "fragor.knob.volume"],
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
    board_geo(r, k).pedals
}

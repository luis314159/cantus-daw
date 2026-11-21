//! Nitor's editor (round 41, ADR 0151; design sheet v1, for the owner's approval): the
//! one-knob enhancer. Two parts:
//!
//! - **the corona** (`nitor_corona.rs`), a dark glass across the top: the five characters as
//!   cut facets down its left (a click chooses one), the one knob — the *amount* — in its
//!   middle like a sun, the character's four bands round it, the exciter's harmonics as rays,
//!   and the low end's readout down its right (the tonic, where it comes from, the sub);
//! - **the cards** under it: the low end (the sub and what it follows, the bass's
//!   harmonics, tight, the lift, the low band), the exciter (body, presence, air, colour)
//!   and the output (the level match, the key, mix, output).
//!
//! Every number from the engine's telemetry (`cantus_engine::fx::nitor`).

use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost};
use super::{frame, FactoryPreset};
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::{Rect, Sense, Vec2};

#[path = "nitor_corona.rs"]
pub(crate) mod corona;

const KIND: EffectKind = EffectKind::Enhancer;

/// Factory presets: data (`presets/effects/enhancer.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The designed size at 100 %: the corona over the cards, every word whole in both
/// languages.
pub fn designed_size() -> Vec2 {
    Vec2::new(1240.0, 820.0)
}

/// The states the design sheet draws it in: a name and the settings over the demo (the
/// first is the starting preset as inserted).
pub const DESIGN_STATES: &[(&str, &[(&str, f32)])] = &[
    ("start", &[]),
    ("punch", &[("amount", 0.7), ("char", 1.0), ("tight", 0.4)]),
    (
        "warm",
        &[("amount", 0.55), ("char", 2.0), ("body", 0.2), ("lift", 0.4)],
    ),
    ("air", &[("amount", 0.6), ("char", 3.0), ("pres", 0.15)]),
    (
        "low",
        &[("sub", 0.6), ("submode", 1.0), ("tight", 0.3), ("harm", 0.4)],
    ),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    view: corona::View,
    /// Where the corona and its facets were drawn last frame (tests point at them).
    pub corona: Option<Rect>,
    pub facets: [Option<Rect>; 5],
}

/// Nitor's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.nitor);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "nitor"), &mut st, ui);
    app.fx.nitor = st;
    out
}

/// The output meter from Nitor's telemetry.
fn out_meter(tel: &[f32; 64]) -> Option<frame::Levels> {
    use cantus_engine::fx::nitor::{T_BLOCKS, T_OUT_PEAK};
    let db = tel[T_OUT_PEAK];
    (tel[T_BLOCKS] > 0.0).then_some(frame::Levels {
        level: [db; 2],
        hold: [db; 2],
    })
}

/// Nitor's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: out_meter(&tel),
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

const KEYS: &[&str] = &[
    "fx.nitor.key.0",
    "fx.nitor.key.1",
    "fx.nitor.key.2",
    "fx.nitor.key.3",
    "fx.nitor.key.4",
    "fx.nitor.key.5",
    "fx.nitor.key.6",
    "fx.nitor.key.7",
    "fx.nitor.key.8",
    "fx.nitor.key.9",
    "fx.nitor.key.10",
    "fx.nitor.key.11",
    "fx.nitor.key.12",
];

/// The low band's crossover in words ("100 Hz").
fn hz_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.0} Hz");
}

const LOW: &[Item] = &[
    Item::Switch(
        "submode",
        "fx.nitor.submode",
        &["nitor.s.octave", "nitor.s.tonic"],
    ),
    Item::Knob("sub", "fx.nitor.sub"),
    Item::Knob("harm", "fx.nitor.harm"),
    Item::Knob("tight", "fx.nitor.tight"),
    Item::Knob("lift", "fx.nitor.lift"),
    Item::Pill("xover", "fx.nitor.xover", hz_pill),
];
const EXCITER: &[Item] = &[
    Item::Knob("body", "fx.nitor.body"),
    Item::Knob("pres", "fx.nitor.pres"),
    Item::Knob("air", "fx.nitor.air"),
    Item::Knob("color", "fx.nitor.color"),
];
const OUTPUT: &[Item] = &[
    Item::Switch("match", "fx.nitor.match", &["fx.nitor.match.0", "fx.nitor.match.1"]),
    Item::Choice("key", "fx.nitor.key", KEYS),
    Item::Knob("mix", "fx.nitor.mix"),
    Item::Knob("out", "fx.nitor.out"),
];

/// The corona over its row (or rows) of cards.
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
    let row = super::controls::card_height(super::controls::Size::M) * k;
    let gap = s.d(4.0);
    let cards: [cells::Card<'_>; 3] = [
        ("nitor.card.low", LOW),
        ("nitor.card.exciter", EXCITER),
        ("nitor.card.output", OUTPUT),
    ];
    let mut widths = [0.0f32; 3];
    for (w, (_, items)) in widths.iter_mut().zip(&cards) {
        *w = k * cells::card_width(ui, items);
    }
    let rows = cells::breaks(&widths, 6.0 * k, area.width());
    let top = (area.height() - rows.len() as f32 * (row + gap)).max(420.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let glass = g.full(top);
    st.corona = Some(glass);
    let reading = corona::Reading::of(tel);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.view.update(&reading, dt);
    let p = corona::parts(glass, k);
    let spec = host.spec().clone();
    let chosen = spec.get("char").round().clamp(0.0, 4.0) as usize;
    let words = corona::Words {
        source: if spec.get("key").round() as i32 == 0 {
            if reading.song.is_some() {
                "nitor.d.song"
            } else {
                "nitor.d.no_key"
            }
        } else {
            "nitor.d.fixed"
        },
        sub_follows: if spec.get("submode") >= 0.5 {
            "nitor.s.tonic"
        } else {
            "nitor.s.octave"
        },
    };
    // The facets answer first, then everything is drawn with what they did.
    // Each is the `char` parameter: a click chooses it, its menu automates or learns it.
    let mut hot = None;
    let name = frame::tr("fx.nitor.char");
    for (i, r) in p.facets.iter().enumerate() {
        let resp = ui.interact(*r, ui.id().with(("nitor.facet", i)), Sense::click());
        let learnt = super::controls::stepped_offer(ui, s, host, "char", name, &resp);
        if resp.hovered() {
            hot = Some(i);
        }
        if resp.clicked() && !learnt && i != chosen {
            host.set("char", i as f32, None);
        }
        st.facets[i] = Some(*r);
    }
    let chosen = host.spec().get("char").round().clamp(0.0, 4.0) as usize;
    let amount = host.spec().get("amount");
    with_brush(&ui.painter_at(glass), |b| {
        corona::paint(b, s, &p, &st.view, &reading, amount);
        for (i, r) in p.facets.iter().enumerate() {
            corona::facet(b, s, *r, i, i == chosen, hot == Some(i));
        }
        corona::what(b, s, p.what, chosen);
        corona::horizon(b, s, &p, &st.view, &reading, &words);
    });
    // The one knob, the sun's own.
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(p.knob)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    sun_knob(host, &mut child, s);
    if reading.seen {
        ui.ctx().request_repaint();
    }
    for &(i, j) in &rows {
        let sum: f32 = widths[i..j].iter().sum::<f32>().max(1.0);
        let mut shares = [0.0f32; 3];
        for (sh, w) in shares.iter_mut().zip(&widths[i..j]) {
            *sh = w / sum;
        }
        let rects: Vec<Rect> = g.row(row, &shares[..j - i]).collect();
        for ((title, items), rect) in cards[i..j].iter().zip(rects) {
            cells::card(host, ui, s, rect, title, items);
        }
    }
}

/// The amount, an XL knob in the corona's middle.
fn sun_knob(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style) {
    use super::controls;
    use crate::kit::text::Src;
    let spec = host.spec();
    let Some(info) = spec.kind.param("amount") else {
        return;
    };
    let cell = controls::Cell::param(info, spec.get("amount"))
        .size(crate::kit::knob::KnobSize::XL)
        .label(Src::Key("fx.nitor.amount"))
        .width(112.0);
    let out = cell.show_in(ui, s, host, "amount");
    if let Some(v) = out.changed {
        host.set("amount", v, Some("amount"));
    }
    if out.done {
        host.end_gesture();
    }
}

/// The inspector's line: the character and its amount ("Clear · 35 %").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let ch = spec.get("char").round().clamp(0.0, 4.0);
    let w = format!(
        "{} \u{b7} {:.0} %",
        crate::i18n::t(&format!("fx.nitor.char.{ch}")),
        100.0 * spec.get("amount")
    );
    super::controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// `--editor nitor`: its starting preset (*Gentle shine*).
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "nitor").set_many(&[("amount", 0.35)], "cmd.fx_preset");
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings fed four seconds of a song in G (drums, a bass, a
/// chord), its telemetry as the engine sends it.
pub fn demo_feed(app: &mut CantusApp) {
    use cantus_engine::fx::FxUnit as _;
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let sr = 48_000.0;
    let mut unit = cantus_engine::fx::nitor::Unit::new(sr);
    unit.configure(&spec);
    unit.set_song_key(Some(cantus_core::scale::Key::new(
        7,
        cantus_core::scale::ScaleKind::Major,
    )));
    let n = 512;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut seed = 0x2545_f491u32;
    let mut t = 0.0f32;
    for _ in 0..(4.0 * sr) as usize / n {
        for i in 0..n {
            let (a, b) = song(t, &mut seed);
            l[i] = a;
            r[i] = b;
            t += 1.0 / sr;
        }
        unit.process(&mut l, &mut r, None);
    }
    app.fx.injected = Some(unit.telemetry());
}

/// A number in −1 … 1 from a running seed: the same every run.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The demo's song at `t` seconds, in G at 120 BPM: a kick on the beat, a snare on two and
/// four, hats, a bass on G1, a G major chord.
fn song(t: f32, seed: &mut u32) -> (f32, f32) {
    use cantus_dsp::math::{exp, sin};
    use std::f32::consts::TAU;
    let beat = 0.5;
    let tb = t % beat;
    let kick = 0.7 * exp(-tb / 0.15) * sin(TAU * (50.0 * tb + 3.0 * (1.0 - exp(-tb / 0.03))));
    let ts = (t % (2.0 * beat) - beat).max(0.0);
    let n = noise(seed);
    let snare = if ts > 0.0 { 0.3 * exp(-ts / 0.08) * n } else { 0.0 };
    let th = t % (beat * 0.5);
    let hats = 0.06 * exp(-th / 0.025) * noise(seed);
    let bass = 0.25 * exp(-tb / 0.5) * (sin(TAU * 49.0 * t) + 0.35 * sin(TAU * 98.0 * t));
    let mut chord = 0.0;
    for f in [196.0f32, 246.94, 293.66] {
        chord += 0.04 * sin(TAU * f * t) + 0.012 * sin(TAU * 2.0 * f * t);
    }
    let m = 0.45 * (kick + snare + bass);
    (m + 0.45 * (chord + hats), m + 0.45 * (chord - 0.6 * hats))
}

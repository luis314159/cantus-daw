//! Vinculum's editor (ADR 0142, provisional; design sheet v1): the bus compressor — a rack
//! unit over a row of cards. The display is the unit itself, screwed into its rack: the
//! faceplate in the colourway's finish with the maker's name and the model's own name
//! engraved, jewel lamps (the external key, the Auto release, the limit, the valve colour),
//! two backlit needle meters (the gain reduction, the output) either side of a window on
//! the model's gain element (the VCA's chip, the opto's glowing cell, the vari-mu's
//! valves), the model selector and the model's own stepped switches as real controls, and a
//! dark screen with the transfer curve and eight seconds of gain reduction. The cards hold
//! the continuous controls (threshold and make-up, the key and its high-pass, the valve
//! colour, the mix and the output). Every number from the engine's telemetry
//! (`cantus_engine::fx::vinculum::t`).

use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost, KeySource};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::Rect;

#[path = "vinculum_art.rs"]
mod art;
#[path = "vinculum_display.rs"]
pub(crate) mod display;
#[path = "vinculum_face.rs"]
mod face;
#[path = "vinculum_meter.rs"]
mod meter;
#[path = "vinculum_parts.rs"]
mod parts;
#[path = "vinculum_screen.rs"]
pub(crate) mod screen;

const KIND: EffectKind = EffectKind::BusCompressor;

/// Factory presets: a label key and the parameters it sets (the factory presets are data,
/// `crates/cantus-ui/presets/`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The window's designed size at 100 % (the frame's `designed_size`): the rack unit over
/// one row of cards, every word whole in both languages.
pub const DESIGNED_SIZE: egui::Vec2 = egui::Vec2::new(1340.0, 900.0);

/// The states the design sheets and `fxb_whole` draw it in: a name and the settings over
/// the demo (the first is the starting preset as inserted). `keyed` sets the side chain's
/// high-pass, and the demo then routes another strip as the key (`demo_feed`).
pub const DESIGN_STATES: &[(&str, &[(&str, f32)])] = &[
    ("start", &[]),
    (
        "vca",
        &[("rel", 2.0), ("ratio", 2.0), ("att", 3.0), ("thr", -22.0)],
    ),
    ("opto", &[("model", 1.0), ("char", 1.0), ("thr", -24.0)]),
    (
        "varimu",
        &[
            ("model", 2.0),
            ("colour", 0.6),
            ("time", 4.0),
            ("thr", -22.0),
        ],
    ),
    ("keyed", &[("hpf", 120.0), ("mix", 0.6), ("thr", -30.0)]),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    view: display::View,
    /// The key's source as the host gave it, until its revision moves.
    key: Option<(u64, Option<KeySource>)>,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
    /// Where the key's field was drawn last frame.
    pub key_field: Option<Rect>,
    /// The faceplate's lamps and the panel's switches as drawn last frame (the pointer
    /// tests find them here).
    lamps: [Option<face::Lamp>; 4],
    switches: [Option<face::Switch>; 4],
    /// What the lamps remember (the release Auto gives back, the key's open list).
    lamp_mem: face::LampMemory,
}

/// Vinculum's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.vinculum);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "vinculum"),
        &mut st,
        ui,
    );
    app.fx.vinculum = st;
    out
}

/// Vinculum's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
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

const COMPRESSION: &[Item] = &[
    Item::Knob("thr", "vinculum.l.threshold"),
    Item::Knob("makeup", "vinculum.l.makeup"),
];
const VALVE: &[Item] = &[Item::Knob("colour", "vinculum.l.colour")];
const OUTPUT: &[Item] = &[
    Item::Knob("mix", "vinculum.l.mix"),
    Item::Knob("out", "vinculum.l.output"),
];
const HIGHPASS: Item = Item::Pill("hpf", "vinculum.l.highpass", hpf_pill);
/// The side chain's card: the key's field and the high-pass (its width is the field's and
/// the knob's).
const SIDE: &[Item] = &[HIGHPASS];
/// The key's field, at 100 %.
const KEY_W: f32 = 176.0;

/// The high-pass in words: "Off" at its foot, "150 Hz" above it.
fn hpf_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    if v <= cantus_core::fx::vinculum::HPF_OFF_HZ {
        b.push_str(frame::tr("vinculum.s.off"));
    } else {
        let _ = write!(b, "{v:.0} Hz");
    }
}

/// Whether a strip is routed as the key.
fn routed(key: &Option<(u64, Option<KeySource>)>) -> bool {
    match key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips { current, .. }) => current.is_some(),
        Some(KeySource::Switch(on)) => *on,
        None => false,
    }
}

/// The rack unit over its row of cards.
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
    let vari = face::model(host.spec()) == 2;
    // The cards in as few rows as fit, evenly filled; the side chain's card carries the
    // key's field beside its knob.
    let mut cards: [cells::Card<'_>; 4] = [
        ("vinculum.card.compression", COMPRESSION),
        ("vinculum.card.side", SIDE),
        ("vinculum.card.output", OUTPUT),
        ("vinculum.card.output", OUTPUT),
    ];
    let n = if vari {
        cards[2] = ("vinculum.card.valve", VALVE);
        4
    } else {
        3
    };
    let mut widths = [0.0f32; 4];
    for (w, (title, items)) in widths.iter_mut().zip(&cards[..n]) {
        let extra = if *title == "vinculum.card.side" {
            KEY_W + 4.0
        } else {
            0.0
        };
        *w = k * (cells::card_width(ui, items) + extra);
    }
    let rows = cells::breaks(&widths[..n], 6.0 * k, area.width());
    let top = (area.height() - rows.len() as f32 * (row + gap)).max(460.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    // The key's source, asked again only when the host's revision moves.
    let rev = host.revision();
    if st.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.key = Some((rev, host.key_source()));
    }
    let reading = display::Reading::of(tel);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.view.update(&reading, dt);
    // The lamps answer first (the key's list, Auto, the limit), then the unit is drawn
    // with what they did.
    let strip_rect = display::parts(r1, k, 0.0).strip;
    let keyed = reading.keyed || routed(&st.key);
    let lamps = face::lamps(ui.ctx(), s, strip_rect, host.spec(), keyed);
    let key = st.key.as_ref().and_then(|(_, k)| k.as_ref());
    face::lamp_edit(host, ui, s, &lamps, key, &mut st.lamp_mem);
    let lamps = face::lamps(ui.ctx(), s, strip_rect, host.spec(), keyed);
    st.lamps = lamps;
    st.switches = display::show(host, &st.view, ui, s, r1, &reading, routed(&st.key), &lamps);
    if reading.seen {
        ui.ctx().request_repaint();
    }
    for &(i, j) in &rows {
        let sum: f32 = widths[i..j].iter().sum::<f32>().max(1.0);
        let mut shares = [0.0f32; 4];
        for (sh, w) in shares.iter_mut().zip(&widths[i..j]) {
            *sh = w / sum;
        }
        let rects: Vec<Rect> = g.row(row, &shares[..j - i]).collect();
        for ((title, items), rect) in cards[i..j].iter().zip(rects) {
            if *title == "vinculum.card.side" {
                side_card(host, st, ui, s, rect);
            } else {
                cells::card(host, ui, s, rect, title, items);
            }
        }
    }
}

/// The side chain's card: what the detector hears (its own input, or another strip) and
/// the high-pass it hears through.
fn side_card(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    use super::controls;
    controls::card(
        ui,
        s,
        rect,
        Src::Key("vinculum.card.side"),
        None,
        |ui, _| {
            controls::column(ui, s, KEY_W, |ui| {
                controls::caption(ui, s, Src::Key("vinculum.l.key"), KEY_W);
                st.key_field = super::dynfx::key_source(
                    host,
                    &mut st.key,
                    ui,
                    s,
                    KEY_W,
                    "vinculum.key_own",
                    "vinculum.key_tip",
                    ["vinculum.key.own", "vinculum.key.sidechain"],
                );
            });
            if let Item::Pill(key, label, f) = HIGHPASS {
                cells::knob(host, ui, s, key, label, Some(&f), None);
            }
        },
    );
}

/// The inspector's picture: the model and its panel in a line ("VCA · 4:1 · Auto ·
/// −18 dB").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let pick = |key: &str, n: f32| {
        crate::i18n::t(&format!(
            "fx.vinculum.{key}.{}",
            spec.get(key).round().clamp(0.0, n)
        ))
    };
    let middle = match face::model(spec) {
        0 => format!("{} \u{b7} {}", pick("ratio", 2.0), pick("rel", 4.0)),
        1 => pick("char", 1.0),
        _ => format!("{} \u{b7} {}", pick("char", 1.0), pick("time", 5.0)),
    };
    let w = format!(
        "{} \u{b7} {middle} \u{b7} {:.0} dB",
        pick("model", 2.0),
        spec.get("thr")
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

/// The starting preset (`cantus_core::starting`): Glue — the VCA at 4 : 1, 30 ms, Auto,
/// −18 dB, +2 dB of make-up, the output fitted to the dry's loudness (−1.5 dB).
pub const DEMO: &[(&str, f32)] = &[
    ("thr", -18.0),
    ("ratio", 1.0),
    ("att", 5.0),
    ("rel", 4.0),
    ("makeup", 2.0),
    ("out", -1.5),
];

/// The settings `--editor vinculum` and the screenshot tool open it with: the starting
/// preset.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "vinculum").set_many(DEMO, "cmd.fx_preset");
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings fed eight seconds of a mix bus (drums, bass and a chord
/// at 120 BPM), its telemetry sampled as the engine sends it and the display's history
/// filled from it. With the side chain's high-pass set (the sheets' *keyed* state) the
/// drums come from another strip of the song as the key, and that strip is routed.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let keyed = spec.get("hpf") > cantus_core::fx::vinculum::HPF_OFF_HZ;
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    if keyed {
        let other = app
            .project()
            .strips
            .iter()
            .map(|s| s.id)
            .find(|id| *id != strip && !app.project().routing_would_cycle(*id, Some(strip)));
        if let Some(source) = other {
            app.exec(cantus_core::Command::SetEffectSidechain {
                strip,
                effect,
                source: Some(source),
            });
        }
    }
    let (tel, points) = demo_telemetry(&spec, keyed);
    app.fx.injected = Some(tel);
    if app.fx.vinculum.effect != Some(effect) {
        app.fx.vinculum = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    app.fx
        .vinculum
        .view
        .seed(&points, &display::Reading::of(&tel));
}

/// A number in −1 … 1 from a running seed (the programme's noise): the same every run.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The programme at `t` seconds: the mix (left, right) and the drums alone (the key).
fn programme(t: f32, seed: &mut u32, hat: &mut f32) -> (f32, f32, f32) {
    use cantus_dsp::math::{exp, sin};
    use std::f32::consts::TAU;
    let beat = 0.5;
    let tb = t % beat;
    let bar = t % (4.0 * beat);
    // The kick on every beat: a falling sine.
    let phase = TAU * (48.0 * tb + 62.0 * 0.04 * (1.0 - exp(-tb / 0.04)));
    let kick = 0.80 * exp(-tb / 0.17) * sin(phase);
    // The snare on two and four: a tone and a burst of noise.
    let ts = (bar - beat).rem_euclid(2.0 * beat);
    let n = noise(seed);
    let snare = if ts < beat {
        0.38 * exp(-ts / 0.09) * n + 0.28 * exp(-ts / 0.05) * sin(TAU * 190.0 * ts)
    } else {
        0.0
    };
    // The hats on the eighths: the noise's edge.
    let th = t % (beat * 0.5);
    let edge = n - *hat;
    *hat = n;
    let hats = 0.10 * exp(-th / 0.03) * edge;
    // The bass on A1 under each beat, the chord (A minor) held.
    let bass = 0.24
        * (1.0 - exp(-tb / 0.01))
        * exp(-tb / 0.6)
        * (sin(TAU * 55.0 * t) + 0.3 * sin(TAU * 110.0 * t));
    let mut chord = 0.0;
    for f in [220.0f32, 261.63, 329.63] {
        chord += 0.045 * sin(TAU * f * t) + 0.015 * sin(TAU * 2.0 * f * t);
    }
    let swell = (t * 0.8).min(1.0);
    let drums = kick + snare;
    let level = 0.42;
    (
        level * (drums + hats * 1.2 + bass + chord * swell),
        level * (drums + hats * 0.8 + bass + chord * swell * 0.92),
        level * drums,
    )
}

/// Vinculum at `spec` fed eight seconds of the programme (its drums the key when `keyed`):
/// the last telemetry and a point of the history every thirtieth of a second.
pub(crate) fn demo_telemetry(spec: &EffectSpec, keyed: bool) -> ([f32; 64], Vec<display::Point>) {
    use cantus_engine::FxUnit as _;
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::vinculum::Unit::new(sr);
    u.configure(spec);
    let n = (sr / display::RATE) as usize;
    let secs = 8.2;
    let blocks = (secs * sr) as usize / n;
    let mut points = Vec::with_capacity(blocks);
    let (mut seed, mut hat) = (0x1234_5678u32, 0.0f32);
    let (mut l, mut r, mut key) = (vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    for blk in 0..blocks {
        for i in 0..n {
            let t = (blk * n + i) as f32 / sr;
            let (a, b, d) = programme(t, &mut seed, &mut hat);
            l[i] = a;
            r[i] = b;
            key[i] = d;
        }
        if keyed {
            u.process(&mut l, &mut r, Some((&key, &key)));
        } else {
            u.process(&mut l, &mut r, None);
        }
        let read = display::Reading::of(&u.telemetry());
        points.push(display::Point {
            gr: read.gr_peak,
            in_db: read.in_db,
            out_db: read.out_db,
            key_db: read.key_db,
        });
    }
    (u.telemetry(), points)
}

impl State {
    /// How many points the display's history holds.
    pub fn history_len(&self) -> usize {
        self.view.len()
    }

    /// Where a lamp was drawn last frame, by its word (`vinculum.d.key`, `.auto`, `.limit`,
    /// `.colour`); `None` when the model has none.
    pub fn lamp(&self, word: &str) -> Option<Rect> {
        self.lamps
            .iter()
            .flatten()
            .find(|l| l.word == word)
            .map(face::Lamp::hit)
    }

    /// Whether a lamp was lit last frame.
    pub fn lamp_lit(&self, word: &str) -> Option<bool> {
        self.lamps
            .iter()
            .flatten()
            .find(|l| l.word == word)
            .map(|l| l.on)
    }

    /// A switch of the panel as drawn last frame, by its parameter: its knob (or bat) and
    /// the engraved word of each position.
    pub fn switch(&self, key: &str) -> Option<(Rect, Vec<Rect>)> {
        self.switches
            .iter()
            .flatten()
            .find(|w| w.key == key)
            .map(|w| (w.knob, w.positions[..w.n].to_vec()))
    }

    /// The key's list while it is open under its lamp: the list's rect and its rows.
    pub fn key_menu(&self) -> Option<(Rect, usize)> {
        self.lamp_mem.menu
    }
}

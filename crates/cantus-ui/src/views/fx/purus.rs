//! Purus's editor (ADR 0145, provisional; design sheet v1): the real-time repair unit — a
//! **restoration bench**. A precision instrument in machined metal lies on a restorer's
//! self-healing cutting mat: its screen a spectrogram on dark glass (what is heard in a warm
//! heat, what is taken out in the residue's cool colour, the hum's harmonics a comb of
//! needles, the clicks' lane, the room's tail before and after); beside it three module
//! cartridges slid into the housing — Hum removal (its emblem a tuning fork engraved with
//! the fundamental it follows, ringing with the hum it hears), Click and crackle removal,
//! Room removal — each with its bat toggle, its jewel lamp and a key to hear what it takes
//! out; along the foot the maker's plate and the latency. The cards hold every knob.
//! Every number from the engine's telemetry (`cantus_engine::fx::purus::t`).

use super::controls::{self, Size};
use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense, Vec2};

#[path = "purus_art.rs"]
mod art;
#[path = "purus_demo.rs"]
mod demo;
#[path = "purus_parts.rs"]
mod parts;
#[path = "purus_picture.rs"]
pub mod picture;
#[path = "purus_screen.rs"]
pub mod screen;

const KIND: EffectKind = EffectKind::Repair;

/// Factory presets: data (`presets/effects/repair.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// Rows of cards in a language (the window grows a row for each more; the screen keeps its
/// size — the owner's rule since Loquax's v2).
pub fn card_rows(lang: crate::i18n::Locale) -> usize {
    match lang {
        crate::i18n::Locale::En => 2,
        crate::i18n::Locale::Es => 2,
    }
}

/// The designed size at 100 % in this thread's language: 1340 wide; 940 tall with two rows
/// of cards, a row taller for each more.
pub fn designed_size() -> Vec2 {
    let rows = card_rows(crate::i18n::locale()) as f32;
    let row = controls::card_height(Size::M) + 4.0;
    Vec2::new(1340.0, 940.0 + (rows - 2.0) * row)
}

/// The states the design sheets and `fxc_whole` draw it in: a name and the settings over
/// the demo (the first is the starting preset as inserted).
pub const DESIGN_STATES: &[(&str, &[(&str, f32)])] = &[
    ("start", &[]),
    (
        "all",
        &[
            ("con", 1.0),
            ("crackle", 0.5),
            ("von", 1.0),
            ("vamount", 14.0),
            ("hharm", 12.0),
        ],
    ),
    (
        "gentle",
        &[
            ("hgentle", 1.0),
            ("hmode", 2.0),
            ("con", 1.0),
            ("crackle", 0.4),
        ],
    ),
    (
        "room",
        &[
            ("hon", 0.0),
            ("von", 1.0),
            ("vdecay", 1.0),
            ("vamount", 20.0),
            ("vdirect", 0.0),
        ],
    ),
    (
        "delta",
        &[
            ("con", 1.0),
            ("von", 1.0),
            ("hdelta", 1.0),
            ("cdelta", 1.0),
            ("vdelta", 1.0),
        ],
    ),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    pub pic: Pic,
    words: Words,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
    /// How many rows the cards took last frame (tests hold [`card_rows`] to it).
    pub rows: usize,
    /// The bench's controls last frame (tests press them).
    pub bench: Option<Bench>,
}

/// The picture, boxed (it is large).
#[derive(Debug, Default)]
pub struct Pic(pub Box<picture::Picture>);

/// The screen's and the cartridges' words, made again only when what they say changes.
#[derive(Debug, Default)]
struct Words {
    key: Option<(u32, u32, u32, u32, u32, crate::i18n::Locale)>,
    hum: String,
    fork: String,
    decay: String,
    clicks: String,
    room: String,
    latency: String,
}

impl Words {
    fn update(&mut self, spec: &EffectSpec, pic: &picture::Picture) {
        let lang = crate::i18n::locale();
        let hz = (pic.hum_hz * 100.0).round() as u32;
        let mode = spec.get("hmode").round() as u32 + 10 * u32::from(spec.get("hon") >= 0.5);
        let decay =
            (pic.decay * 100.0).round() as u32 + 100_000 * u32::from(spec.get("vdecay") >= 0.5);
        let clicks = pic.clicks_total as u32;
        let lat = pic.latency.round() as u32 + 1_000_000 * (pic.verb_mean() * 10.0).round() as u32;
        let key = (hz, mode, decay, clicks, lat, lang);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let hz_s = format!("{:.2}", pic.hum_hz);
        let mode_s = frame::tr(match spec.get("hmode").round() as i32 {
            1 => "fx.purus.hmode.1",
            2 => "fx.purus.hmode.2",
            _ => "fx.purus.hmode.0",
        });
        self.hum = if spec.get("hon") >= 0.5 {
            crate::i18n::tf("purus.d.hum", &[("hz", &hz_s), ("mode", &mode_s)])
        } else {
            frame::tr("purus.d.hum_off").to_string()
        };
        self.fork = format!("{hz_s} Hz");
        let s = format!("{:.2}", pic.decay);
        self.decay = crate::i18n::tf(
            if spec.get("vdecay") >= 0.5 {
                "purus.d.decay_est"
            } else {
                "purus.d.decay_set"
            },
            &[("s", &s)],
        );
        self.clicks = crate::i18n::tf("purus.d.clicks_n", &[("n", &clicks)]);
        let cut = format!("{:.1}", pic.verb_mean());
        self.room = crate::i18n::tf("purus.d.room_cut", &[("db", &cut)]);
        let ms = format!("{:.1}", pic.latency / pic.sample_rate.max(1.0) * 1000.0);
        self.latency = crate::i18n::tf("purus.d.latency", &[("ms", &ms)]);
    }
}

/// Purus's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.purus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "purus"), &mut st, ui);
    app.fx.purus = st;
    out
}

/// Purus's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let set = cantus_engine::fx::purus::settings(host.spec());
    let latency = cantus_dsp::purus::latency_for(&set, sr.max(1.0)) as f32;
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency, sr)];
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

const MODES: &[&str] = &["fx.purus.hmode.0", "fx.purus.hmode.1", "fx.purus.hmode.2"];
const HUM: &[Item] = &[
    Item::Choice("hmode", "purus.l.mode", MODES),
    Item::Pill("hhz", "purus.l.fundamental", hz2_pill),
    Item::Knob("hharm", "purus.l.harmonics"),
    Item::Knob("hdepth", "purus.l.depth"),
    Item::Knob("hwidth", "purus.l.width"),
    Item::Knob("hmix", "purus.l.mix"),
    Item::Knob("hout", "purus.l.output"),
];
const CLICK: &[Item] = &[
    Item::Knob("csens", "purus.l.sensitivity"),
    Item::Knob("crackle", "purus.l.crackle"),
    Item::Knob("cmix", "purus.l.mix"),
    Item::Knob("cout", "purus.l.output"),
];
const ROOM: &[Item] = &[
    Item::Knob("vamount", "purus.l.reduction"),
    Item::Knob("vt60", "purus.l.decay"),
    Item::Knob("vdirect", "purus.l.direct"),
    Item::Knob("vsmooth", "purus.l.smoothing"),
    Item::Knob("vkeep", "purus.l.early"),
    Item::Knob("vmix", "purus.l.mix"),
    Item::Knob("vout", "purus.l.output"),
];
const OUTPUT: &[Item] = &[Item::Knob("out", "purus.l.output")];

/// A frequency to the hundredth: "50.02 Hz".
fn hz2_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.2} Hz");
}

/// The bench over its rows of cards.
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
    let row = controls::card_height(Size::M) * k;
    let gap = s.d(4.0);
    let cards: [cells::Card<'_>; 4] = [
        ("purus.card.hum", HUM),
        ("purus.card.click", CLICK),
        ("purus.card.room", ROOM),
        ("purus.card.output", OUTPUT),
    ];
    let widths: Vec<f32> = cards
        .iter()
        .map(|(_, it)| k * cells::card_width(ui, it))
        .collect();
    let rows = cells::breaks(&widths, 6.0 * k, area.width());
    st.rows = rows.len();
    let top = (area.height() - rows.len() as f32 * (row + gap)).max(520.0 * k);
    let r1 = g.full(top);
    st.display = Some(r1);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.pic.0.update(tel, dt);
    display(host, st, ui, s, r1);
    for &(i, j) in &rows {
        let sum: f32 = widths[i..j].iter().sum::<f32>().max(1.0);
        let shares: Vec<f32> = widths[i..j].iter().map(|w| w / sum).collect();
        let rects: Vec<Rect> = g.row(row, &shares).collect();
        for ((title, items), rect) in cards[i..j].iter().zip(rects) {
            cells::card(host, ui, s, rect, title, items);
        }
    }
}

/// What a press on the bench does.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Act {
    /// Switch a module (its toggle).
    Toggle(&'static str),
    /// Set a parameter.
    Set(&'static str, f32),
}

/// A control on the bench: where it is, what it does, its name and help (locale keys).
#[derive(Copy, Clone, Debug)]
pub struct Spot {
    pub rect: Rect,
    pub act: Act,
    pub name: &'static str,
    pub tip: &'static str,
}

/// The bench's layout.
#[derive(Clone, Debug)]
pub struct Bench {
    pub housing: Rect,
    pub plot: screen::Plot,
    pub carts: [Rect; 3],
    pub spots: Vec<Spot>,
    pub foot: Rect,
}

/// A key's width at 100 % for its words.
fn key_w(ctx: &egui::Context, key: &str, k: f32) -> f32 {
    let t = Text::key(key, Role::Micro, egui::Color32::TRANSPARENT).caps(true);
    crate::kit::text::measure(ctx, &t, k).x + 22.0 * k
}

/// Lay the bench out in `r`.
fn lay(ctx: &egui::Context, k: f32, r: Rect) -> Bench {
    let housing = Rect::from_min_max(
        pos2(r.left() + 22.0 * k, r.top() + 20.0 * k),
        pos2(r.right() - 22.0 * k, r.bottom() - 18.0 * k),
    );
    let face = housing.shrink(3.0 * k);
    let content = Rect::from_min_max(
        pos2(face.left() + 30.0 * k, face.top() + 26.0 * k),
        pos2(face.right() - 30.0 * k, face.bottom() - 18.0 * k),
    );
    let col_w = 344.0 * k;
    let col = Rect::from_min_max(pos2(content.right() - col_w, content.top()), content.max);
    let left = Rect::from_min_max(content.min, pos2(col.left() - 22.0 * k, content.bottom()));
    let foot_h = 40.0 * k;
    let bezel = Rect::from_min_max(
        left.min,
        pos2(left.right(), left.bottom() - foot_h - 10.0 * k),
    );
    let foot = Rect::from_min_max(pos2(left.left(), bezel.bottom() + 10.0 * k), left.max);
    let glass = bezel.shrink(7.0 * k);
    let plot = screen::Plot::lay(glass, k);
    let gap = 12.0 * k;
    let ch = (col.height() - 2.0 * gap) / 3.0;
    let carts = [0, 1, 2].map(|i| {
        Rect::from_min_size(
            pos2(col.left(), col.top() + i as f32 * (ch + gap)),
            vec2(col.width(), ch),
        )
    });
    let mut spots = Vec::new();
    // Each cartridge's toggle and its key to hear what it takes out.
    let mods: [(&str, &str, &str, &str); 3] = [
        ("hon", "hdelta", "fx.purus.hon", "purus.tip.hon"),
        ("con", "cdelta", "fx.purus.con", "purus.tip.con"),
        ("von", "vdelta", "fx.purus.von", "purus.tip.von"),
    ];
    for (c, (on, delta, name, tip)) in carts.iter().zip(mods) {
        let tg = toggle_at(*c, k);
        spots.push(Spot {
            rect: Rect::from_center_size(tg, vec2(28.0, 44.0) * k),
            act: Act::Toggle(on),
            name,
            tip,
        });
        let w = key_w(ctx, "purus.k.hear", k);
        let kr = Rect::from_min_size(
            pos2(c.right() - 12.0 * k - w, c.bottom() - 34.0 * k),
            vec2(w, 22.0 * k),
        );
        spots.push(Spot {
            rect: kr,
            act: Act::Toggle(delta),
            name: match delta {
                "hdelta" => "fx.purus.hdelta",
                "cdelta" => "fx.purus.cdelta",
                _ => "fx.purus.vdelta",
            },
            tip: "purus.tip.delta",
        });
    }
    // The hum's keys: 50 Hz, 60 Hz, Gentle — along its foot, left of its listening key.
    let c0 = carts[0];
    let mut x = c0.left() + 22.0 * k;
    for (key, act, name, tip) in [
        (
            "purus.k.fifty",
            Act::Set("hhz", 50.0),
            "fx.purus.hhz",
            "purus.tip.fifty",
        ),
        (
            "purus.k.sixty",
            Act::Set("hhz", 60.0),
            "fx.purus.hhz",
            "purus.tip.sixty",
        ),
        (
            "purus.k.gentle",
            Act::Toggle("hgentle"),
            "fx.purus.hgentle",
            "purus.tip.gentle",
        ),
    ] {
        let w = key_w(ctx, key, k);
        spots.push(Spot {
            rect: Rect::from_min_size(pos2(x, c0.bottom() - 34.0 * k), vec2(w, 22.0 * k)),
            act,
            name,
            tip,
        });
        x += w + 6.0 * k;
    }
    // The room's decay: set by hand or estimated.
    let c2 = carts[2];
    let mut x = c2.left() + 22.0 * k;
    for (key, v, tip) in [
        ("purus.k.set", 0.0, "purus.tip.set"),
        ("purus.k.estimated", 1.0, "purus.tip.estimated"),
    ] {
        let w = key_w(ctx, key, k);
        spots.push(Spot {
            rect: Rect::from_min_size(pos2(x, c2.bottom() - 34.0 * k), vec2(w, 22.0 * k)),
            act: Act::Set("vdecay", v),
            name: "fx.purus.vdecay",
            tip,
        });
        x += w + 6.0 * k;
    }
    Bench {
        housing,
        plot,
        carts,
        spots,
        foot,
    }
}

/// A cartridge's toggle: by its pull, at its upper third.
fn toggle_at(c: Rect, k: f32) -> Pos2 {
    pos2(c.left() + 36.0 * k, c.top() + 40.0 * k)
}

/// Whether a spot is lit by the settings.
fn lit(spec: &EffectSpec, act: Act) -> bool {
    match act {
        Act::Toggle(key) => spec.get(key) >= 0.5,
        Act::Set(key, v) => (spec.get(key) - v).abs() < 0.01,
    }
}

/// The bench in `r`, its controls answering the pointer.
fn display(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let k = s.k;
    let ctx = ui.ctx().clone();
    let bench = lay(&ctx, k, r);
    // The controls first, so this frame draws what they set.
    let mut hot = None;
    for (i, sp) in bench.spots.iter().enumerate() {
        let id = ui.id().with(("purus-spot", i));
        let resp = ui.interact(sp.rect, id, Sense::click());
        if resp.clicked() {
            let v = match sp.act {
                Act::Toggle(key) => {
                    if host.spec().get(key) >= 0.5 {
                        0.0
                    } else {
                        1.0
                    }
                }
                Act::Set(_, v) => v,
            };
            let key = match sp.act {
                Act::Toggle(key) | Act::Set(key, _) => key,
            };
            host.set(key, v, None);
        }
        if resp.hovered() {
            hot = Some(i);
            hint::set_text(ui.ctx(), frame::tr(sp.name), frame::tr(sp.tip), Caps::NONE);
        }
    }
    let spec = host.spec().clone();
    st.words.update(&spec, &st.pic.0);
    let t_now = ui.input(|i| i.time) as f32;
    with_brush(&ui.painter_at(r), |b| {
        art::mat(b, s, r);
        let face = art::housing(b, s, bench.housing);
        let _ = face;
        let glass = art::bezel(b, s, bench.plot.glass.expand(7.0 * k));
        let _ = glass;
        let shown = screen::Shown {
            hum_on: spec.get("hon") >= 0.5,
            harmonics: spec.get("hharm").round() as usize,
            gentle: spec.get("hgentle") >= 0.5,
            click_on: spec.get("con") >= 0.5,
            verb_on: spec.get("von") >= 0.5,
            keep_ms: spec.get("vkeep"),
            delta: ["hdelta", "cdelta", "vdelta"]
                .iter()
                .any(|d| spec.get(d) >= 0.5),
        };
        let w = screen::Words {
            hum: &st.words.hum,
            decay: &st.words.decay,
            clicks: &st.words.clicks,
            fork: &st.words.fork,
        };
        screen::paint(b, s, &bench.plot, &st.pic.0, &shown, &w);
        cartridges(b, s, &bench, &spec, &st.pic.0, &st.words, hot, t_now);
        foot(b, s, bench.foot, &st.words);
    });
    st.bench = Some(bench);
}

/// The three cartridges: each its toggle, lamp, name, reading, emblem and keys.
#[allow(clippy::too_many_arguments)]
fn cartridges(
    b: &mut Brush,
    s: &Style,
    bench: &Bench,
    spec: &EffectSpec,
    pic: &picture::Picture,
    words: &Words,
    hot: Option<usize>,
    t_now: f32,
) {
    let k = s.k;
    let res = art::residue(s);
    let ndl = art::needle(s);
    let names = ["purus.m.hum", "purus.m.click", "purus.m.room"];
    let ons = ["hon", "con", "von"];
    for (i, c) in bench.carts.iter().enumerate() {
        let on = spec.get(ons[i]) >= 0.5;
        let face = parts::cartridge(b, s, *c, on);
        let tg = toggle_at(*c, k);
        let lamp_col = if i == 0 { ndl } else { res };
        let activity = match i {
            0 => ((pic.level.iter().take(4).fold(-120.0f32, |m, v| m.max(*v)) + 70.0) / 40.0)
                .clamp(0.0, 1.0),
            1 => (pic
                .cols
                .iter()
                .rev()
                .take(12)
                .map(|c| c.clicks as f32)
                .sum::<f32>()
                / 3.0)
                .clamp(0.0, 1.0),
            _ => (pic.verb_mean() / 12.0).clamp(0.0, 1.0),
        };
        let lamp_at = pos2(face.right() - 18.0 * k, face.top() + 18.0 * k);
        parts::lamp(
            b,
            s,
            lamp_at,
            5.5 * k,
            if on { 0.35 + 0.65 * activity } else { 0.0 },
            lamp_col,
        );
        let name = Text::key(names[i], Role::Label, egui::Color32::TRANSPARENT).caps(true);
        let nr = art::engrave(
            b,
            s,
            pos2(tg.x + 22.0 * k, face.top() + 18.0 * k),
            Align2::LEFT_CENTER,
            &name,
            1.0,
        );
        let reading = match i {
            0 => words.hum.as_str(),
            1 => words.clicks.as_str(),
            _ => words.decay.as_str(),
        };
        let rt = Text::new(reading, Role::Micro, egui::Color32::TRANSPARENT).caps(false);
        art::print(
            b,
            s,
            pos2(nr.left(), nr.bottom() + 9.0 * k),
            Align2::LEFT_CENTER,
            &rt,
            1.0,
        );
        // The emblem's place: between the reading and the keys, right of the toggle.
        let em = Rect::from_min_max(
            pos2(tg.x + 22.0 * k, nr.bottom() + 20.0 * k),
            pos2(face.right() - 12.0 * k, face.bottom() - 42.0 * k),
        );
        match i {
            0 => {
                let ring = if on { activity } else { 0.0 };
                parts::fork(b, s, em, &words.fork, ring, t_now);
            }
            1 => loupe(b, s, em, on, res),
            _ => room_emblem(b, s, em, on, res, &words.room, spec.get("vkeep")),
        }
        parts::toggle(b, s, tg, on, false);
    }
    for (j, sp) in bench.spots.iter().enumerate() {
        if matches!(sp.act, Act::Toggle(k) if ["hon", "con", "von"].contains(&k)) {
            if hot == Some(j) {
                parts::toggle(b, s, sp.rect.center(), lit(spec, sp.act), true);
            }
            continue;
        }
        let label = match (sp.act, sp.tip) {
            (_, "purus.tip.delta") => "purus.k.hear",
            (_, "purus.tip.fifty") => "purus.k.fifty",
            (_, "purus.tip.sixty") => "purus.k.sixty",
            (_, "purus.tip.gentle") => "purus.k.gentle",
            (_, "purus.tip.set") => "purus.k.set",
            _ => "purus.k.estimated",
        };
        let col = if sp.tip == "purus.tip.fifty"
            || sp.tip == "purus.tip.sixty"
            || sp.tip == "purus.tip.gentle"
        {
            ndl
        } else {
            res
        };
        parts::key(
            b,
            s,
            sp.rect,
            frame::tr(label),
            lit(spec, sp.act),
            hot == Some(j),
            col,
        );
    }
}

/// The de-click's emblem: a jeweller's loupe over a click — the spike dashed, the line
/// rebuilt under it solid, the lens's rim and handle.
fn loupe(b: &mut Brush, s: &Style, r: Rect, on: bool, res: egui::Color32) {
    use crate::kit::color::with_alpha;
    use crate::kit::skin::restoration as pal;
    use crate::kit::Fill;
    let k = s.k;
    let rad = (r.height() * 0.5).min(30.0 * k);
    let cen = pos2(r.left() + rad + 8.0 * k, r.center().y);
    // The handle, then the rim and the lens.
    let h0 = cen + vec2(rad * 0.7, rad * 0.7);
    b.segment(
        h0 + vec2(1.0, 2.0) * k,
        h0 + vec2(rad, rad) * 0.9 + vec2(1.0, 2.0) * k,
        7.0 * k,
        with_alpha(art::shadow(), 0.4),
        true,
    );
    b.segment(
        h0,
        h0 + vec2(rad, rad) * 0.9,
        6.0 * k,
        art::c(pal::RIM),
        true,
    );
    b.segment(
        h0,
        h0 + vec2(rad, rad) * 0.9,
        1.0 * k,
        with_alpha(art::c(pal::STEEL_LIGHT), 0.35),
        true,
    );
    b.circle(
        cen,
        rad + 3.5 * k,
        Fill::V(art::c(pal::STEEL_LIGHT), art::c(pal::RIM)),
    );
    b.circle(
        cen,
        rad,
        Fill::V(art::c(pal::GLASS_TOP), art::c(pal::GLASS_FOOT)),
    );
    // Inside: a waveform with a click, magnified.
    let pts: Vec<Pos2> = (0..=40)
        .map(|i| {
            let x = -1.0 + i as f32 / 20.0;
            let y = 0.35 * cantus_dsp::math::sin(x * 5.0) * (1.0 - x * x * 0.4);
            cen + vec2(x * rad * 0.8, -y * rad)
        })
        .collect();
    b.polyline(&pts, 1.2 * k, if on { res } else { with_alpha(res, 0.4) });
    let spike = [pts[19], cen + vec2(0.02 * rad, -0.85 * rad), pts[21]];
    b.dashed(
        &spike,
        0.9 * k,
        with_alpha(art::c(pal::HEAT[3]), if on { 0.9 } else { 0.4 }),
        2.5 * k,
        2.0 * k,
    );
    b.circle_stroke(cen, rad, 0.8 * k, with_alpha(art::c(pal::SHEEN), 0.3));
    b.arc(
        cen,
        rad * 0.8,
        3.6,
        4.4,
        1.2 * k,
        with_alpha(art::c(pal::SHEEN), 0.35),
        true,
    );
    // Its words beside it.
    let t = Text::key("purus.d.loupe", Role::Micro, egui::Color32::TRANSPARENT).caps(false);
    art::print(
        b,
        s,
        pos2(cen.x + rad + 22.0 * k, cen.y),
        Align2::LEFT_CENTER,
        &t,
        1.0,
    );
}

/// The de-reverb's emblem: a voice's wave fronts spreading into a room — the early ones
/// kept, the late ones (past the early reflections kept) drawn in the residue's colour,
/// dashed, as the module takes them out — and the reduction now beside it.
fn room_emblem(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    on: bool,
    res: egui::Color32,
    words: &str,
    keep_ms: f32,
) {
    use crate::kit::color::with_alpha;
    let k = s.k;
    let f = art::finish(s);
    let src = pos2(r.left() + 10.0 * k, r.center().y);
    let ink = art::c(f.engrave);
    b.circle(src, 3.5 * k, crate::kit::Fill::Solid(ink));
    let n = 9;
    let kept = ((keep_ms / 200.0) * n as f32)
        .round()
        .clamp(1.0, n as f32 - 1.0) as usize;
    for i in 0..n {
        let rad = (9.0 + i as f32 * 7.0) * k;
        let fade = 1.0 - i as f32 / n as f32;
        if i < kept || !on {
            b.arc(
                src,
                rad,
                -0.75,
                0.75,
                1.0 * k,
                with_alpha(ink, 0.25 + 0.6 * fade),
                false,
            );
        } else {
            let pts: Vec<Pos2> = (0..=12)
                .map(|j| {
                    let a = -0.75 + 1.5 * j as f32 / 12.0;
                    src + vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a)) * rad
                })
                .collect();
            b.dashed(
                &pts,
                1.0 * k,
                with_alpha(res, 0.35 + 0.6 * fade),
                3.0 * k,
                2.5 * k,
            );
        }
    }
    // The wall the room's sound comes back from: a fine hatched edge.
    let wx = src.x + (9.0 + n as f32 * 7.0) * k;
    b.vline(
        wx,
        r.center().y - 30.0 * k,
        r.center().y + 30.0 * k,
        1.2 * k,
        ink,
    );
    for j in 0..7 {
        let y = r.center().y - 27.0 * k + j as f32 * 9.0 * k;
        b.segment(
            pos2(wx, y),
            pos2(wx + 6.0 * k, y + 5.0 * k),
            0.7 * k,
            with_alpha(ink, 0.7),
            false,
        );
    }
    let t = Text::new(words, Role::Micro, egui::Color32::TRANSPARENT).caps(false);
    art::print(
        b,
        s,
        pos2(wx + 18.0 * k, r.center().y),
        Align2::LEFT_CENTER,
        &t,
        1.0,
    );
}

/// The housing's foot: the maker's plate, the model, the latency printed at the right.
fn foot(b: &mut Brush, s: &Style, r: Rect, words: &Words) {
    let k = s.k;
    let name_t = Text::key(
        "picker.dev.purus.name",
        Role::Title,
        egui::Color32::TRANSPARENT,
    )
    .caps(true);
    let name_w = crate::kit::text::measure(b.ctx(), &name_t, k * 1.2).x;
    let plate = Rect::from_min_size(
        pos2(r.left(), r.center().y - 16.0 * k),
        vec2(name_w + 44.0 * k, 32.0 * k),
    );
    art::plate(b, s, plate);
    let model = Text::key("purus.model", Role::Label, egui::Color32::TRANSPARENT).caps(false);
    art::print(
        b,
        s,
        pos2(plate.right() + 16.0 * k, r.center().y),
        Align2::LEFT_CENTER,
        &model,
        1.0,
    );
    let lat = Text::new(&words.latency, Role::Label, egui::Color32::TRANSPARENT).caps(false);
    art::print(
        b,
        s,
        pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        &lat,
        1.0,
    );
}

/// The inspector's picture: the modules that are on, in a line ("Hum 50 Hz · Clicks ·
/// Room 12 dB").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let mut parts: Vec<String> = Vec::new();
    if spec.get("hon") >= 0.5 {
        parts.push(crate::i18n::tf(
            "purus.p.hum",
            &[("hz", &format!("{:.0}", spec.get("hhz")))],
        ));
    }
    if spec.get("con") >= 0.5 {
        parts.push(frame::tr("purus.p.clicks").to_string());
    }
    if spec.get("von") >= 0.5 {
        parts.push(crate::i18n::tf(
            "purus.p.room",
            &[("db", &format!("{:.0}", spec.get("vamount")))],
        ));
    }
    if parts.is_empty() {
        parts.push(frame::tr("purus.p.none").to_string());
    }
    let w = parts.join(" \u{b7} ");
    controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The starting preset (`cantus_core::starting`): the de-hum on at 50 Hz, followed.
pub const DEMO: &[(&str, f32)] = &[("hon", 1.0), ("hmode", 1.0)];

/// The settings `--editor purus` and the screenshot tool open it with: the starting preset.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "purus").set_many(DEMO, "cmd.fx_preset");
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings over the demo take ([`demo`]).
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let Some((_, effect, _)) = app.fx.open else {
        return;
    };
    let (tel, pic) = demo::picture(&spec);
    app.fx.injected = Some(tel);
    if app.fx.purus.effect != Some(effect) {
        app.fx.purus = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    app.fx.purus.pic = Pic(Box::new(pic));
}

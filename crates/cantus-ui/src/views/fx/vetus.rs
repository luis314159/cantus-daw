//! Vetus's editor: lo-fi (fx side round, ADR 0130).
//!
//! *Vetus* is Latin for old, and the window is a record player: at its centre **the
//! record** — the music of its last turns in its groove, worn by every module that is on
//! (stepped by the bits, cut in facets by the rate, off its spindle with the wow, dusty,
//! scratched, its label yellowed and dated by Character) — turned back by hand to age it;
//! beside it **what comes out**, the spectrum with the converter's Nyquist, the quantiser's
//! floor and the medium's band drawn on it and dragged there. Beside them **Character**,
//! the one knob that ages everything; under them the six modules in the signal's order,
//! each a card with its switch.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::modfx_clock::tel;
use cantus_dsp::vetus::{t, VetusSettings};
use egui::{pos2, vec2, Align2, Rect, Sense};

#[path = "vetus_cassette.rs"]
mod cassette;
#[path = "vetus_modules.rs"]
mod modules;

/// The unit's name for its view preference (`settings.json`'s `unit_views`).
pub const VIEW_UNIT: &str = "vetus";
/// Its two objects: the cassette is the default (the owner's choice).
pub const VIEWS: [(&str, &str); 2] = [
    ("cassette", "vetus.view.cassette"),
    ("record", "vetus.view.record"),
];

/// Whether the window shows the record (else the cassette).
pub fn shows_record() -> bool {
    super::unit_view::is(VIEW_UNIT, "record", VIEWS[0].0)
}
#[path = "vetus_record.rs"]
mod record;
#[path = "vetus_spectrum.rs"]
mod spectrum;

pub(crate) const KIND: EffectKind = EffectKind::LoFi;

/// Factory presets: data (`presets/effects/lofi.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The media's names (`vetus.medium.{i}`).
pub const MEDIUM_KEYS: [&str; 4] = [
    "vetus.medium.0",
    "vetus.medium.1",
    "vetus.medium.2",
    "vetus.medium.3",
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The groove: the output of the last turns.
    groove: record::Groove,
    /// The spectrum and its drags.
    spectrum: spectrum::View,
    /// A turn of the record being dragged: the pointer's angle then and Character now.
    turning: Option<(f32, f32)>,
    /// Where the record and the spectrum were drawn last frame (tests point at them).
    pub record: Option<Rect>,
    pub plot: Option<Rect>,
    /// A colourway chosen in the ⋯ menu this frame, for the host to keep.
    colourway: Option<&'static str>,
    /// A view chosen in the ⋯ menu this frame, for the host to keep.
    view: Option<&'static str>,
}

/// Vetus's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.vetus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "vetus"), &mut st, ui);
    // The colourway chosen is the user's for every Vetus, kept in the settings.
    if let Some(id) = st.colourway.take() {
        if let Some(name) = crate::kit::skin::colourway_unit(crate::kit::skin::Unit::Effect(KIND)) {
            app.set_unit_colourway(name, id);
        }
    }
    // The object chosen is the user's for every Vetus too.
    if let Some(id) = st.view.take() {
        app.set_unit_view(VIEW_UNIT, id);
    }
    app.fx.vetus = st;
    out
}

/// Vetus's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    // The ⋯ menu's colourways (ADR 0127 §Colourways), each row with its swatch.
    let unit = crate::kit::skin::Unit::Effect(KIND);
    let ways = crate::kit::skin::colourways(unit);
    let wearing = crate::kit::skin::colourway_of(unit).map(|w| w.id);
    let dark = Style::current().t.dark;
    let mut more: Vec<crate::kit::menu::Item<'_>> = Vec::with_capacity(ways.len() + 4);
    // The object first (record or cassette), then the colourways.
    more.push(crate::kit::menu::Item::Band(Src::Key("vetus.view")));
    let record = shows_record();
    for (id, key) in VIEWS {
        more.push(crate::kit::menu::Item::Action {
            label: Src::Key(key),
            check: crate::kit::menu::Check::Radio((id == "record") == record),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    if !ways.is_empty() {
        more.push(crate::kit::menu::Item::Band(Src::Key("vetus.colourway")));
    }
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
        meter: super::modfx::out_meter(&tel),
        more: &more,
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel)
    });
    st.chrome = chrome;
    st.chips = chips;
    // The rows: 0 the object's band, 1 … 2 the objects, 3 the colourways' band, then them.
    if let Some(i) = out
        .more
        .and_then(|i| i.checked_sub(1))
        .filter(|i| *i < VIEWS.len())
    {
        let id = VIEWS[i].0;
        super::unit_view::set_view(VIEW_UNIT, id);
        st.view = Some(id);
    }
    if let Some(w) = out
        .more
        .and_then(|i| i.checked_sub(VIEWS.len() + 2))
        .and_then(|i| ways.get(i))
    {
        if let Some(name) = crate::kit::skin::colourway_unit(unit) {
            crate::kit::skin::set_colourway(name, w.id);
            st.colourway = Some(w.id);
        }
    }
    out
}

/// The share of the display row Character's card takes, at 100 %.
const CHAR_W: f32 = 176.0;

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
    let top = (area.height() - 2.0 * row - 2.0 * gap).max(240.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let w = area.width();
    let char_share = (CHAR_W * k / w).clamp(0.1, 0.3);
    let r1: Vec<Rect> = g.row(top, &[1.0 - char_share, char_share]).collect();
    display(host, st, ui, s, r1[0], tel);
    character_card(host, ui, s, r1[1], tel);
    let cs = cantus_engine::fx::vetus::settings(host.spec());
    let r2: Vec<Rect> = g.row(row, &[0.30, 0.25, 0.45]).collect();
    modules::bits(host, ui, s, r2[0], &cs);
    modules::rate(host, ui, s, r2[1], &cs);
    modules::wobble(host, ui, s, r2[2], &cs);
    let r3: Vec<Rect> = g.row(row, &[0.18, 0.40, 0.42]).collect();
    modules::age(host, ui, s, r3[0], &cs);
    modules::noise(host, ui, s, r3[1], &cs);
    modules::tone(host, ui, s, r3[2], &cs);
}

/// Whether the unit is playing and its telemetry is fresh.
pub fn live(tel: &[f32; 64]) -> bool {
    tel[tel::BLOCKS] > 0.0 && tel[tel::PLAYING] > 0.5
}

/// The display: the record and what comes out.
fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key(if shows_record() {
            "vetus.card.record"
        } else {
            "vetus.card.cassette"
        }),
        None,
        |ui, cr| {
            let r = cr.body;
            let k = s.k;
            let rec_w = (r.height() * 1.18).min(r.width() * 0.56);
            let rec = Rect::from_min_size(r.min, vec2(rec_w, r.height()));
            let spec = Rect::from_min_max(pos2(rec.right() + 4.0 * k, r.top()), r.max);
            st.record = Some(rec);
            st.plot = Some(spectrum::map_of(spec, k).plot);
            st.groove.update(host.pairs());
            spectrum::update(&mut st.spectrum, host.pairs(), host.sample_rate());
            if shows_record() {
                record::edit(host, st, ui, rec);
            } else {
                let (c, reach) = cassette::grip(rec);
                record::edit_at(host, st, ui, c, reach);
            }
            spectrum::edit(host, &mut st.spectrum, ui, spectrum::map_of(spec, k), k);
            let cs = cantus_engine::fx::vetus::settings(host.spec());
            let look = record::Look::of(&cs, tel, live(tel));
            let sr = host.sample_rate();
            let ink = super::canvas::Ink::of(s);
            with_brush(&ui.painter_at(r), |b| {
                super::canvas::ground_b(b, r, &ink, s.hue_tok);
                if shows_record() {
                    record::paint(b, rec, s, &look, &st.groove, k);
                } else {
                    cassette::paint(b, rec, s, &look, &st.groove, k);
                }
                spectrum::paint(b, s, spec, &st.spectrum, &cs, sr, k);
                // The seam between the turntable's well and the screen: a hairline.
                b.vline(
                    rec.right() + 2.0 * k,
                    r.top(),
                    r.bottom(),
                    0.8 * k,
                    ink.edge,
                );
                words(b, s, rec, &look, k);
                super::canvas::edge_b(b, r, &ink);
            });
        },
    );
}

/// The record's corner words: the speed and its deviation; the modules sounding.
fn words(b: &mut crate::kit::Brush, s: &Style, r: Rect, look: &record::Look, k: f32) {
    let ink = super::canvas::Ink::of(s);
    let mut w = if shows_record() {
        let mut w = String::from("33\u{2153} ");
        w.push_str(frame::tr("vetus.rpm"));
        w
    } else {
        String::from(frame::tr("vetus.tape_speed"))
    };
    if look.s.wob_on && look.playing {
        w.push_str("  \u{b7}  ");
        w.push_str(frame::tr("vetus.speed"));
        w.push(' ');
        controls::write_readout(look.speed_ct, "ct", &mut w);
    }
    let t = Text::new(&w, Role::Value, ink.text).caps(false);
    super::canvas::put(
        b,
        pos2(r.left() + 8.0 * k, r.top() + 6.0 * k),
        Align2::LEFT_TOP,
        &t,
        k,
        Some(ink.plate),
    );
}

/// Character, the mix and the output: the one knob that ages everything, a size up.
fn character_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
) {
    let _ = tel;
    controls::card(
        ui,
        s,
        rect,
        Src::Key("vetus.card.character"),
        None,
        |ui, cr| {
            let w = cr.body.width() / s.k;
            controls::column(ui, s, w, |ui| {
                ui.vertical_centered(|ui| {
                    controls::param_with(
                        ui,
                        s,
                        host,
                        "char",
                        Size::XL,
                        Some(&|v: f32, out: &mut String| {
                            controls::write_readout(v, "%", out);
                        }),
                    );
                    let era = year_text(host.spec().get("char"));
                    let (er, _) = ui.allocate_exact_size(vec2(w * s.k, s.d(16.0)), Sense::hover());
                    let ink = super::canvas::Ink::of(s);
                    with_brush(ui.painter(), |b| {
                        let t = Text::new(&era, Role::Value, s.t.text_dim).caps(false);
                        super::canvas::put(b, er.center(), Align2::CENTER_CENTER, &t, s.k, None);
                        let _ = ink;
                    });
                    ui.horizontal(|ui| {
                        let pad = ((w - 2.0 * 64.0 - 4.0) * 0.5).max(0.0);
                        ui.add_space(pad * s.k);
                        controls::param(ui, s, host, "mix", Size::M);
                        controls::param(ui, s, host, "out", Size::M);
                    });
                });
            });
        },
    );
}

/// The era Character dates the label with: this year at 0, a century back at 100 %.
pub fn year_text(character: f32) -> String {
    if character <= 0.005 {
        return frame::tr("vetus.label.new").to_string();
    }
    let year = 2026 - (character.clamp(0.0, 1.0) * 100.0).round() as i32;
    year.to_string()
}

/// A bit depth as its words: "8 bits", "7.5 bits".
pub fn bits_pill(v: f32, out: &mut String) {
    let r = (v * 2.0).round() / 2.0;
    if (r - r.round()).abs() < 1e-3 {
        let _ = std::fmt::Write::write_fmt(out, format_args!("{:.0} ", r));
    } else {
        let _ = std::fmt::Write::write_fmt(out, format_args!("{:.1} ", r));
    }
    out.push_str(frame::tr(if (r - 1.0).abs() < 1e-3 {
        "vetus.bit"
    } else {
        "vetus.bits"
    }));
}

/// The inspector's picture: a small record and the modules that are on.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    modules::preview(ui, spec)
}

/// The settings `--editor vetus` and the screenshot tool open it with: an old cassette
/// sampled at 8 bits — every module on, a little Character.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "vetus").set_many(DEMO, "cmd.fx_preset");
}

/// The demo's settings.
pub const DEMO: &[(&str, f32)] = &[
    ("char", 0.35),
    ("bits.on", 1.0),
    ("bits.depth", 8.0),
    ("rate.on", 1.0),
    ("rate.hz", 11_025.0),
    ("wob.on", 1.0),
    ("wow", 18.0),
    ("flut", 5.0),
    ("age.on", 1.0),
    ("age.drop", 0.35),
    ("noise.on", 1.0),
    ("crackle", 0.45),
    ("hiss", 0.3),
    ("hum", 0.2),
    ("tone.on", 1.0),
    ("tone.lo", 60.0),
    ("tone.hi", 9_000.0),
];

/// What a playing engine would feed the editor, for screenshots with no engine: the demo
/// chord (with a hat on the eighths) through the unit, its telemetry, the analyser's pairs
/// and the groove of the last turns.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::vetus::Vetus::new(48_000.0);
    d.set(&cantus_engine::fx::vetus::settings(&spec));
    d.reset();
    let mut pairs: Vec<f32> = Vec::new();
    let mut out: Vec<f32> = Vec::new();
    let mut noise = 0x2545_f491u32;
    let mut at = 0usize;
    let eighth = (48_000.0 * 60.0 / 128.0 / 2.0) as usize;
    super::modfx::demo_run(
        &mut |l, r, pos| {
            for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
                let n = at + i;
                let env = (-((n % eighth) as f32) / 900.0).exp();
                noise ^= noise << 13;
                noise ^= noise >> 17;
                noise ^= noise << 5;
                let w = (noise as f32 / u32::MAX as f32 - 0.5) * 0.5 * env;
                // A kick on the beats.
                let beat = n % (2 * eighth);
                let kick = (beat as f32 / 48_000.0 * 55.0 * std::f32::consts::TAU).sin()
                    * (-(beat as f32) / 5_000.0).exp()
                    * 0.6;
                *a += w + kick;
                *b += w + kick;
            }
            let dry: Vec<f32> = l.to_vec();
            d.set_transport(&pos);
            d.process(l, r);
            for (x, y) in dry.iter().zip(l.iter()) {
                pairs.push(*x);
                pairs.push(*y);
                out.push(*y);
            }
            at += l.len();
        },
        5.6,
    );
    let mut tel = [0.0f32; 64];
    d.telemetry(&mut tel);
    // A tick fell at the stylus this frame.
    tel[t::CLICKS] = tel[t::CLICKS].max(2.0);
    app.fx.injected = Some(tel);
    app.fx.feed(&pairs);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.vetus.effect != Some(effect) {
            app.fx.vetus = State {
                effect: Some(effect),
                ..State::default()
            };
        }
    }
    app.fx.vetus.groove.seed(&out);
}

/// Vetus's settings for a spec, after Character: what the editor shows.
pub fn heard(spec: &EffectSpec) -> VetusSettings {
    cantus_engine::fx::vetus::settings(spec).aged()
}

//! Rota's editor: the rotary speaker (fx side round, wave 3, ADR 0136 provisional).
//!
//! *Rota* is Latin for the wheel. Its dark display is the cabinet three ways: **cut away**
//! from the front, the horn turning in the upper chamber and the drum's scoop in the lower,
//! each at the angle and speed the engine reports, the amp's valve on the shelf glowing with
//! the drive; **from above**, the horn's arm and the drum's opening sweeping past the two
//! microphones, which are dragged round the cabinet (their angle) and away from it (their
//! distance); and **the ramps**, each rotor's speed against time since the switch was
//! thrown, its dot travelling up the curve as it spins up. The speed switch — Stop, Slow,
//! Fast — is lit along the display's top. Under it the horn's and the drum's speeds, the
//! ramp and the notes that throw the switch, the microphones, the cabinet and the mix.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::Rect;

#[path = "rota_art.rs"]
pub mod art;
#[path = "rota_display.rs"]
mod display;

const KIND: EffectKind = EffectKind::Rotary;

/// Factory presets: data (`presets/effects/rotary.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    view: display::View,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
}

/// Rota's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.rota);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "rota"), &mut st, ui);
    app.fx.rota = st;
    out
}

/// Rota's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
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

/// The display over a row of cards.
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
    let top = (area.height() - row - gap).max(300.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    let reading = display::Reading::of(tel);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.view.update(&reading, dt);
    display::show(host, &mut st.view, ui, s, r1, &reading);
    if reading.seen {
        ui.ctx().request_repaint();
    }
    let w = [
        controls::cells_width(&[Size::M; 2]) + 16.0,
        controls::cells_width(&[Size::M; 2]) + 16.0,
        controls::cells_width(&[Size::M]) + COLUMN + 20.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
        controls::cells_width(&[Size::M]) + COLUMN + 20.0,
        controls::cells_width(&[Size::M; 2]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let cards: Vec<Rect> = g.row(row, &w.map(|x| x / sum)).collect();
    card2(host, ui, s, cards[0], "rota.card.horn", ["hslow", "hfast"]);
    card2(host, ui, s, cards[1], "rota.card.drum", ["dslow", "dfast"]);
    controls::card(ui, s, cards[2], Src::Key("rota.card.switch"), None, |ui, _| {
        labelled(host, ui, s, "ramp", "rota.l.ramp");
        controls::column(ui, s, COLUMN, |ui| {
            controls::caption(ui, s, Src::Key("rota.l.notes"), COLUMN);
            let labels = [Src::Key("fx.rota.notes.0"), Src::Key("fx.rota.notes.1")];
            controls::segmented_param(ui, s, host, "notes", &labels, COLUMN);
        });
    });
    controls::card(ui, s, cards[3], Src::Key("rota.card.mics"), None, |ui, _| {
        labelled_as(host, ui, s, "spread", "rota.l.angle", Some(&deg_pill));
        labelled_as(host, ui, s, "dist", "rota.l.distance", Some(&cm_pill));
        labelled(host, ui, s, "balance", "rota.l.balance");
    });
    controls::card(ui, s, cards[4], Src::Key("rota.card.cabinet"), None, |ui, _| {
        controls::column(ui, s, COLUMN, |ui| {
            controls::caption(ui, s, Src::Key("rota.l.back"), COLUMN);
            let labels = [Src::Key("fx.rota.open.0"), Src::Key("fx.rota.open.1")];
            controls::segmented_param(ui, s, host, "open", &labels, COLUMN);
        });
        labelled(host, ui, s, "drive", "rota.l.drive");
    });
    card2(host, ui, s, cards[5], "rota.card.mix", ["mix", "out"]);
}

/// A column of switches beside a knob, at 100 %.
const COLUMN: f32 = 112.0;

/// A cell on `key` with a short label of its own (the card says whose it is).
fn labelled(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, key: &'static str, label: &str) {
    labelled_as(host, ui, s, key, label, None);
}

/// [`labelled`], its value written by `pill` (whole rpm, degrees, centimetres).
fn labelled_as(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    label: &str,
    pill: Option<controls::Pill<'_>>,
) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let mut c = controls::Cell::param(info, spec.get(key))
        .size(Size::M)
        .label(Src::Key(label))
        .width(super::arca::words::cell_width(ui, label));
    if let Some(f) = pill {
        c = c.format(f);
    }
    let out = c.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// A card of two knobs: the horn's or the drum's slow and fast, or the mix and output.
fn card2(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    title: &'static str,
    keys: [&'static str; 2],
) {
    let labels = if keys[0] == "mix" {
        ["rota.l.mix", "rota.l.output"]
    } else {
        ["rota.l.slow", "rota.l.fast"]
    };
    controls::card(ui, s, rect, Src::Key(title), None, |ui, _| {
        let pill: Option<controls::Pill<'_>> = if keys[0] == "mix" {
            None
        } else {
            Some(&rpm_pill)
        };
        labelled_as(host, ui, s, keys[0], labels[0], pill);
        labelled_as(host, ui, s, keys[1], labels[1], pill);
    });
}

/// The inspector's picture: the speed and the rotors' speeds in a line ("Fast · horn 423
/// rpm, drum 357 rpm").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let (rs, _) = cantus_engine::fx::rota::settings(spec);
    let speed = (spec.get("speed").round().max(0.0) as usize).min(2);
    let (h, d) = match speed {
        2 => (rs.horn_fast_rpm, rs.drum_fast_rpm),
        1 => (rs.horn_slow_rpm, rs.drum_slow_rpm),
        _ => (0.0, 0.0),
    };
    let w = format!(
        "{} \u{b7} {} {h:.0}, {} {d:.0} rpm",
        frame::tr(display::SPEEDS[speed]),
        frame::tr("rota.d.horn"),
        frame::tr("rota.d.drum"),
    );
    controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The settings `--editor rota` and the screenshot tool open it with: the starting preset,
/// Chorale (the mix up, slow).
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "rota").set_many(DEMO, "cmd.fx_preset");
}

/// The starting preset (`cantus_core::starting`).
pub const DEMO: &[(&str, f32)] = &[("mix", 1.0)];

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings, its switch thrown from slow to the setting's speed 0.9 s
/// ago (the horn nearly there, the drum on its way), its telemetry as the engine sends it.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let tel = demo_telemetry(&spec, 0.9);
    app.fx.injected = Some(tel);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.rota.effect != Some(effect) {
            app.fx.rota = State {
                effect: Some(effect),
                ..State::default()
            };
        }
        app.fx.rota.view.seed(&display::Reading::of(&tel), &spec);
    }
}

/// The telemetry of Rota at `spec`, two seconds at slow and then `since` seconds at the
/// spec's speed, a held organ chord playing through it.
pub fn demo_telemetry(spec: &EffectSpec, since: f32) -> [f32; 64] {
    use cantus_engine::FxUnit as _;
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::rota::Unit::new(sr);
    let mut slow = spec.clone();
    slow.set("speed", 1.0);
    u.configure(&slow);
    let n = 480;
    let blocks_slow = (2.0 * sr) as usize / n;
    let blocks = blocks_slow + (since * sr) as usize / n;
    for blk in 0..blocks {
        if blk == blocks_slow {
            u.configure(spec);
        }
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            let t = (blk * n + i) as f32 / sr;
            let mut x = 0.0;
            for f in [130.81f32, 196.0, 261.63, 329.63] {
                x += 0.08 * cantus_dsp::math::sin(std::f32::consts::TAU * f * t);
            }
            *a = x;
            *b = x;
        }
        u.process(&mut l, &mut r, None);
    }
    u.telemetry()
}

/// A rotor's speed, whole: "423 rpm".
fn rpm_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.0} rpm");
}

/// The microphones' angle, whole: "120°".
fn deg_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.0}\u{b0}");
}

/// The microphones' distance, whole: "60 cm".
fn cm_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.0} cm");
}

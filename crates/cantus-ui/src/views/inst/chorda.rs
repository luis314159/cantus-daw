//! Chorda — the modelled grand piano: its editor, on the kit's device shell (ADR 0024 §8,
//! ADR 0059). Owned by this instrument's builder (ADR 0023).
//!
//! What a physically modelled piano's editor usually hides and this one shows, from the
//! model's own numbers:
//!
//! - **The piano from above**: every key's strings at their speaking length (one, two or
//!   three), the bridge, where the hammers strike, the dampers as firmly down as the pedal
//!   leaves them — the key last played lit, its hammer shifted by the soft pedal.
//! - **The half-pedal law**: how far down the dampers are at each position of the pedal,
//!   with the pedal's position on it.
//! - **The tuning's stretch** in cents across the keyboard, redrawn as the stretch and the
//!   strings' stiffness move (a new tuning is designed off the audio thread; the caption
//!   says so while it is).
//! - **The felt**, force against compression, as measured and as set; **the ring** of every
//!   key, free and damped.
//!
//! The look (ADR 0024): **Play** is the piano from above beside the pedals, and a row of
//! cells (hammer, resonance, output); **Edit** is every part as a card in its function hue
//! — strings, tuning, hammer, the strings' ring, pedals, resonance, output.

#[path = "chorda_draw.rs"]
mod draw;

use super::organ::parts::{cells, paint_card, Card, Rows};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Policy, Role, Text};
use crate::kit::{well, with_brush, Fill, Function, State as KitState, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::chorda::tables::{self, Key, KEYS};
use cantus_engine::inst::chorda::{settings_from, tel, tuning_of, values_of};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

pub use draw::Now;

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    /// Where each control of the last frame (cell) sits, by parameter key.
    pub placed: std::collections::HashMap<&'static str, egui::Rect>,
    /// The rect the cards were laid out in, and the part of it that is visible.
    pub area: Option<egui::Rect>,
    pub view: Option<egui::Rect>,
    /// The keyboard drawn, and the tuning it was computed for (a4, stretch, stiffness).
    keys: Option<([f32; 3], Box<[Key; KEYS]>)>,
}

/// The keyboard of `spec`'s tuning (kept while the tuning stays).
fn keyboard<'a>(state: &'a mut State, spec: &NativeSpec) -> &'a [Key; KEYS] {
    let want = [spec.get("a4"), spec.get("stretch"), spec.get("stiffness")];
    if state.keys.as_ref().map(|k| k.0) != Some(want) {
        let tu = tuning_of(&values_of(spec));
        state.keys = Some((
            want,
            Box::new(tables::keyboard(tu.a4, tu.stretch, tu.stiffness)),
        ));
    }
    &state.keys.as_ref().expect("just set").1
}

/// What the pictures show: the engine's report, or with no engine, middle C at rest.
fn now_of(tele: &cantus_engine::Telemetry) -> Now {
    let alive = tele[tel::ALIVE] > 0.5;
    let key = tele[tel::KEY] as u8;
    Now {
        key: if alive && (tables::LOWEST..=tables::HIGHEST).contains(&key) {
            key
        } else {
            60
        },
        playing: alive && tele[0] > 0.0,
        velocity: tele[tel::VELOCITY].clamp(0.0, 1.0),
        pedal: tele[tel::PEDAL].clamp(0.0, 1.0),
        soft: tele[tel::SOFT].clamp(0.0, 1.0),
    }
}

/// Whether the engine is still designing the tuning asked for.
fn designing(tele: &cantus_engine::Telemetry) -> bool {
    tele[tel::ALIVE] > 0.5 && tele[tel::DESIGNED] < 0.5
}

/// The height of a card with a `display` and a `body`, at 100 %.
fn card_h(display: f32, body: f32) -> f32 {
    24.0 + if display > 0.0 { 1.0 + display } else { 0.0 } + 8.0 + body + 8.0
}

/// A card: its title, a display drawn by `paint` (when `display` > 0), its cells below.
#[allow(clippy::too_many_arguments)]
fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    title: &str,
    f: Function,
    display: f32,
    keys: &[&str],
    paint: impl FnOnce(&mut crate::kit::Brush, Rect, &Style),
) {
    let sf = s.function(f);
    let mut c = Card::new(title, f);
    if display > 0.0 {
        c = c.display(display);
    }
    let cr = paint_card(ui, rect, s, &c);
    if let Some(d) = cr.display {
        with_brush(&ui.painter_at(d), |b| paint(b, d, &sf));
    }
    if !keys.is_empty() {
        cells(app, ui, cr.body, track, spec, keys, &sf, KnobSize::M);
    }
}

/// The display's height that makes a card `h` points tall (at the style's scale) with a
/// row of cells, or with none.
fn display_for(h: f32, s: &Style, with_cells: bool) -> f32 {
    let d = if with_cells {
        h / s.k - card_h(0.0, 86.0) - 1.0
    } else {
        h / s.k - 24.0 - 1.0 - 6.0
    };
    d.max(40.0)
}

/// The Play view: the piano from above beside the pedals, and the numbers a player turns.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let tele = app.inst_telemetry();
    let now = now_of(&tele);
    let (vs, _) = settings_from(&values_of(spec));
    let grip = vs.voice.damper as f32;
    let area = ui.max_rect();
    app.inst.chorda.area = Some(area);
    app.inst.chorda.view = Some(ui.clip_rect());
    let keys = *keyboard(&mut app.inst.chorda, spec);
    let gutter = s.d(4.0);
    let h2 = s.s(card_h(0.0, 86.0));
    let h1 = (area.height() - h2 - gutter).max(s.s(240.0));
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(h1, &[0.64, 0.36]);
    let r2 = rows.row(h2, &[0.36, 0.36, 0.28]);
    let none: [&str; 0] = [];
    card(
        app,
        ui,
        track,
        spec,
        r1[0],
        s,
        "chorda.strings",
        Function::Source,
        display_for(h1, s, false),
        &none,
        |b, d, sf| draw::grand(b, d, sf, &keys, &now, grip),
    );
    card(
        app,
        ui,
        track,
        spec,
        r1[1],
        s,
        "chorda.pedals",
        Function::Dynamics,
        display_for(h1, s, true),
        &["damper", "una"],
        |b, d, sf| draw::pedals(b, d, sf, &now, grip),
    );
    let rows2: [(&str, Function, &[&str]); 3] = [
        (
            "chorda.hammer",
            Function::Dynamics,
            &["hardness", "dynamics", "vel.curve"],
        ),
        (
            "chorda.resonance",
            Function::Space,
            &["sympathy", "decay", "body"],
        ),
        (
            "chorda.output",
            Function::Output,
            &["tilt", "width", "gain"],
        ),
    ];
    for (r, (title, f, keys)) in r2.iter().zip(rows2) {
        card(
            app,
            ui,
            track,
            spec,
            *r,
            s,
            title,
            f,
            0.0,
            keys,
            |_, _, _| {},
        );
    }
}

/// The Edit view: every part of the instrument, each with its picture. Taller than the
/// window, it scrolls there.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let tele = app.inst_telemetry();
    let now = now_of(&tele);
    let busy = designing(&tele);
    let (vs, hardness) = settings_from(&values_of(spec));
    let grip = vs.voice.damper as f32;
    let decay = vs.voice.decay;
    let a4 = spec.get("a4") as f64;
    let gutter = s.d(4.0);
    let small = s.s(card_h(0.0, 86.0));
    let heights = [
        s.s(card_h(170.0, 86.0)),
        s.s(card_h(110.0, 86.0)),
        small * 2.0 + gutter,
    ];
    let total = heights.iter().sum::<f32>() + gutter * (heights.len() - 1) as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total), Sense::hover());
    app.inst.chorda.area = Some(area);
    app.inst.chorda.view = Some(ui.clip_rect());
    let keys = *keyboard(&mut app.inst.chorda, spec);
    let shown = keys
        .iter()
        .find(|k| k.key == now.key)
        .copied()
        .unwrap_or(keys[39]);
    let mut rows = Rows::new(area, s);

    let r1 = rows.row(heights[0], &[0.6, 0.4]);
    card(
        app,
        ui,
        track,
        spec,
        r1[0],
        s,
        "chorda.strings",
        Function::Source,
        170.0,
        &["unison", "coupling", "phantom"],
        |b, d, sf| draw::grand(b, d, sf, &keys, &now, grip),
    );
    card(
        app,
        ui,
        track,
        spec,
        r1[1],
        s,
        "chorda.tuning",
        Function::Tone,
        170.0,
        &["a4", "stretch", "stiffness"],
        |b, d, sf| draw::stretch(b, d, sf, &keys, a4, busy),
    );

    let r2 = rows.row(heights[1], &[0.5, 0.5]);
    card(
        app,
        ui,
        track,
        spec,
        r2[0],
        s,
        "chorda.hammer",
        Function::Dynamics,
        110.0,
        &["hardness", "hysteresis", "dynamics", "vel.curve"],
        |b, d, sf| draw::felt(b, d, sf, &shown, hardness),
    );
    card(
        app,
        ui,
        track,
        spec,
        r2[1],
        s,
        "chorda.ring",
        Function::Source,
        110.0,
        &["decay"],
        |b, d, sf| draw::ring(b, d, sf, &keys, now.key, decay),
    );

    let r3 = rows.row(heights[2], &[0.5, 0.5]);
    card(
        app,
        ui,
        track,
        spec,
        r3[0],
        s,
        "chorda.pedals",
        Function::Dynamics,
        display_for(heights[2], s, true),
        &["damper", "una"],
        |b, d, sf| draw::pedals(b, d, sf, &now, grip),
    );
    let right = r3[1];
    let top = Rect::from_min_size(right.min, Vec2::new(right.width(), small));
    let bottom = Rect::from_min_size(
        Pos2::new(right.left(), right.top() + small + gutter),
        Vec2::new(right.width(), small),
    );
    let none = |_: &mut crate::kit::Brush, _: Rect, _: &Style| {};
    card(
        app,
        ui,
        track,
        spec,
        top,
        s,
        "chorda.resonance",
        Function::Space,
        0.0,
        &["sympathy", "body"],
        none,
    );
    card(
        app,
        ui,
        track,
        spec,
        bottom,
        s,
        "chorda.output",
        Function::Output,
        0.0,
        &["tilt", "width", "gain"],
        none,
    );
}

/// The inspector's small picture: three strings and a hammer, the tuning.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(Function::Source);
    let text = format!(
        "{} \u{b7} A4 {:.1} Hz \u{b7} {} {:.0} %",
        t(spec.kind.name_key()),
        spec.get("a4"),
        t("chorda.stretch"),
        spec.get("stretch")
    );
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(36.0)), Sense::hover());
    with_brush(&ui.painter_at(rect), |b| {
        well::paint(b, rect, &s, KitState::REST, None);
        let y = rect.center().y;
        let x0 = rect.left() + s.s(8.0);
        for j in 0..3 {
            let yy = y - s.s(4.0) + j as f32 * s.s(4.0);
            b.segment(
                Pos2::new(x0, yy),
                Pos2::new(x0 + s.s(40.0), yy),
                s.s(1.2),
                s.t.text_dim,
                true,
            );
        }
        b.rrect(
            Rect::from_center_size(
                Pos2::new(x0 + s.s(12.0), y + s.s(9.0)),
                Vec2::new(s.s(8.0), s.s(5.0)),
            ),
            s.r(1.5),
            Fill::Solid(s.hue),
        );
        let tr = Rect::from_min_max(
            Pos2::new(x0 + s.s(50.0), rect.top()),
            Pos2::new(rect.right() - s.s(6.0), rect.bottom()),
        );
        let _ = ktext::paint(
            b,
            tr,
            &Text::new(&text, Role::Body, s.t.text)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End)),
            s.tk(),
        );
    });
    resp
}

/// `--editor chorda`: the concert grand, an E5 sounding with the pedal partly down.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.concert")
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Chorda, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: E5 struck firmly, the pedal at 40 %.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0; cantus_engine::TELEMETRY_LEN];
    t[0] = 6.0;
    t[1] = 88.0;
    t[tel::KEY] = 76.0;
    t[tel::VELOCITY] = 0.8;
    t[tel::PEDAL] = 0.4;
    t[tel::PEAK_L] = 0.3;
    t[tel::PEAK_R] = 0.35;
    t[tel::DESIGNED] = 1.0;
    t[tel::ALIVE] = 1.0;
    app.inst.injected = Some(t);
}

// ---- factory presets --------------------------------------------------------------------

/// Four macros on every preset: 1 hardness, 2 sympathy, 3 the body, 4 the decay.
const MACROS: &[(&str, &str)] = &[
    ("macro1.name", "Hardness"),
    ("mod1.src", "macro1"),
    ("mod1.dst", "hardness"),
    ("macro2.name", "Resonance"),
    ("mod2.src", "macro2"),
    ("mod2.dst", "sympathy"),
    ("macro3.name", "Body"),
    ("mod3.src", "macro3"),
    ("mod3.dst", "body"),
    ("macro4.name", "Decay"),
    ("mod4.src", "macro4"),
    ("mod4.dst", "decay"),
];

macro_rules! preset {
    ($label:expr, $tags:expr, [$($k:expr => $v:expr),* $(,)?]) => {
        FactoryPreset {
            label_key: $label,
            values: &[
                ("mod1.amt", 0.4),
                ("mod2.amt", 0.5),
                ("mod3.amt", 0.4),
                ("mod4.amt", 0.3),
                $(($k, $v)),*
            ],
            text: MACROS,
            tags: $tags,
        }
    };
}

/// Factory presets, with tags.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    preset!(
        "chorda.preset.concert",
        &["preset.tag.keys", "chorda.tag.grand"],
        []
    ),
    preset!("chorda.preset.studio", &["preset.tag.keys", "chorda.tag.grand"], [
        "hardness" => 0.3, "tilt" => 2.5, "phantom" => 1.4, "width" => 0.8, "dynamics" => 0.9,
    ]),
    preset!("chorda.preset.ballad", &["preset.tag.keys", "chorda.tag.soft"], [
        "hardness" => -0.3, "tilt" => -3.0, "sympathy" => 1.3, "decay" => 1.2,
        "hysteresis" => 0.92,
    ]),
    preset!("chorda.preset.jazz", &["preset.tag.keys", "chorda.tag.grand"], [
        "hardness" => 0.1, "body" => 0.8, "width" => 0.55, "decay" => 0.85, "dynamics" => 1.15,
        "unison" => 1.5,
    ]),
    preset!("chorda.preset.felt", &["preset.tag.keys", "chorda.tag.soft"], [
        "hardness" => -0.85, "tilt" => -9.0, "decay" => 0.55, "sympathy" => 0.7, "body" => 0.7,
        "dynamics" => 0.7, "gain" => 3.0,
    ]),
    preset!("chorda.preset.honky", &["preset.tag.keys", "chorda.tag.character"], [
        "unison" => 7.0, "coupling" => 0.3, "hardness" => 0.4, "tilt" => 3.0, "body" => 0.8,
    ]),
    preset!("chorda.preset.baroque", &["preset.tag.keys", "chorda.tag.character"], [
        "a4" => 415.0, "stretch" => 0.7, "hardness" => 0.15,
    ]),
    preset!("chorda.preset.wash", &["preset.tag.keys", "chorda.tag.soft"], [
        "sympathy" => 2.0, "decay" => 1.8, "body" => 1.5, "width" => 0.9, "damper" => 0.6,
    ]),
    preset!("chorda.preset.pop", &["preset.tag.keys", "chorda.tag.grand"], [
        "hardness" => 0.5, "dynamics" => 0.7, "tilt" => 4.0, "phantom" => 1.6, "width" => 0.6,
    ]),
    preset!("chorda.preset.old", &["preset.tag.keys", "chorda.tag.character"], [
        "stiffness" => 1.3, "unison" => 3.5, "decay" => 0.75, "body" => 0.65, "tilt" => -2.0,
        "coupling" => 0.65,
    ]),
];

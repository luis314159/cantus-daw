//! Lamina — the modelled electric piano: its editor, on the kit's device shell (ADR 0024
//! §8). Owned by this instrument's builder (ADR 0023).
//!
//! What the benchmarks (Stage-73 V2, Wurli V3, Lounge Lizard EP-5, Keyscape;
//! `research/instruments/keys.md` §5) get wrong and what this does instead:
//!
//! - **Millimetres, not 0.444.** The pickup's gap and offset are drawn at true scale under
//!   its transfer curve, and the tine's swing is lit on the curve as it plays: you see the
//!   bark happen. Drag the pole piece to voice it.
//! - **The tine itself**, to scale for the key you last played: its length from the physics,
//!   where the hammer meets it, the tone bar, the pickup.
//! - **Keyboard curves in every tier**: hardness, decay and level across the keyboard, dragged
//!   on anchors over the keys (Pianoteq keeps this for its dearest edition; Lounge Lizard has
//!   only KEY knobs).
//! - **The pedal as the dampers see it**: its travel, the half-pedal zone shaded from the
//!   actual lift point, where it is now.
//!
//! The look (ADR 0024): **Play** is the pickup with the tine's swing beside the tine drawn
//! to scale and the pedal, the engine (Tine · Reed) in the pickup's title, and a row of
//! cells; **Edit** is every section as a card in its function hue — pickup, tine or reed,
//! hammer, preamp, dampers and pedal, tremolo, playing, and the key curves. Every drag is
//! one step of the project's undo.

#[path = "epiano_keys.rs"]
mod keys;
#[path = "epiano_pickup.rs"]
mod pickup;

use super::face;
use super::organ::parts::{self, cells, paint_card, switch, tail_rect, Card, Rows};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Fill, Function, State as KitState, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::lamina::engagement;
use cantus_dsp::lamina::geometry::Engine;
use cantus_engine::inst::epiano::{settings, tel};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

pub use keys::Curve;

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    /// Which keyboard curve the strip edits.
    pub curve: Curve,
    /// Where the last frame drew the pole piece and the C4 anchor (tests aim at them).
    pub pole: Option<egui::Rect>,
    pub anchor: Option<egui::Rect>,
    /// Where each control of the last frame (cell, switch, lamp) sits, by parameter key.
    pub placed: std::collections::HashMap<&'static str, egui::Rect>,
    /// The rect the cards were laid out in, and the part of it that is visible.
    pub area: Option<egui::Rect>,
    pub view: Option<egui::Rect>,
}

pub(super) fn set(app: &mut CantusApp, track: TrackId, key: &str, v: f32, gesture: bool) {
    app.set_inst_value(track, key, v, gesture);
}

const ENGINES: [Src<'static>; 2] = [
    Src::Key("epiano.engine_tine"),
    Src::Key("epiano.engine_reed"),
];
const RANGES: [Src<'static>; 3] = [
    Src::Key("epiano.range_73"),
    Src::Key("epiano.range_64"),
    Src::Key("epiano.range_88"),
];
const OUTPUTS: [Src<'static>; 2] = [
    Src::Key("epiano.out_direct"),
    Src::Key("epiano.out_speaker"),
];
const TREMS: [Src<'static>; 3] = [
    Src::Key("epiano.trem_auto"),
    Src::Key("epiano.trem_pan"),
    Src::Key("epiano.trem_amp"),
];

/// The height of a card with a `display` and a `body`, at 100 %.
fn card_h(display: f32, body: f32) -> f32 {
    24.0 + if display > 0.0 { 1.0 + display } else { 0.0 } + 8.0 + body + 8.0
}

/// How far the dampers rest on the tines: the engine's report, or, with no engine, the
/// pedal up.
fn engaged(s: &cantus_dsp::lamina::LaminaSettings, tele: &cantus_engine::Telemetry) -> f32 {
    if tele[tel::ALIVE] > 0.5 {
        tele[tel::ENGAGEMENT]
    } else {
        engagement(0.0, s.pedal_lift)
    }
}

/// The key the geometry shows: the last one struck, or C4.
fn shown_key(tele: &cantus_engine::Telemetry) -> u8 {
    if tele[tel::ALIVE] > 0.5 && tele[tel::KEY] > 0.0 {
        tele[tel::KEY] as u8
    } else {
        60
    }
}

/// The resonator's numbers for the engine: the tine's four, or the reed's two.
fn resonator_keys(engine: Engine) -> &'static [&'static str] {
    match engine {
        Engine::Tine => &["tine.decay", "tine.couple", "tine.bell", "tine.bell_decay"],
        Engine::Reed => &["reed.decay", "reed.overtones"],
    }
}

/// A card of cells: the title, and `keys` as cells across its body.
#[allow(clippy::too_many_arguments)]
fn cells_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    title: &str,
    f: Function,
    keys: &[&str],
) -> crate::kit::band::CardRects {
    let cr = paint_card(ui, rect, s, &Card::new(title, f));
    cells(
        app,
        ui,
        cr.body,
        track,
        spec,
        keys,
        &s.function(f),
        KnobSize::M,
    );
    cr
}

/// The pickup as a card: the engine's switch in its title, the curve and the pole as its
/// display, and in Edit the pickup's cells below.
#[allow(clippy::too_many_arguments)]
fn pickup_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    ls: &cantus_dsp::lamina::LaminaSettings,
    tele: &cantus_engine::Telemetry,
    rect: Rect,
    s: &Style,
    edit: bool,
) {
    let ss = s.function(Function::Source);
    let tail = 132.0;
    let display = if edit {
        rect.height() / s.k - card_h(0.0, 86.0) - 1.0
    } else {
        rect.height() / s.k - 24.0 - 1.0 - 6.0
    };
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("epiano.pickup", Function::Source)
            .display(display)
            .tail(tail),
    );
    switch(
        app,
        ui,
        tail_rect(&cr, s, tail),
        track,
        spec,
        "engine",
        &ENGINES,
        &ss,
    );
    if let Some(d) = cr.display {
        pickup::pickup(app, ui, track, ls, tele, d, &face::on_display(&ss));
    }
    if edit {
        let (g, a) = if ls.engine == Engine::Tine {
            ("tine.gap", "tine.align")
        } else {
            ("reed.gap", "reed.align")
        };
        let keys = [g, a, "pickup.level"];
        cells(app, ui, cr.body, track, spec, &keys, &ss, KnobSize::M);
    }
}

/// The tine (or reed) as a card: drawn to scale as its display, its numbers below.
#[allow(clippy::too_many_arguments)]
fn resonator_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    ls: &cantus_dsp::lamina::LaminaSettings,
    key: u8,
    rect: Rect,
    s: &Style,
    keys: &[&str],
) {
    let ss = s.function(Function::Source);
    let title = if ls.engine == Engine::Tine {
        "epiano.resonator_tine"
    } else {
        "epiano.resonator_reed"
    };
    let display = (rect.height() / s.k - card_h(0.0, 86.0) - 1.0).max(40.0);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new(title, Function::Source).display(display),
    );
    if let Some(d) = cr.display {
        let sd = face::on_display(&ss);
        with_brush(&ui.painter_at(d), |b| {
            pickup::geometry(b, ls, key, d, &sd);
            face::cover(b, d, &sd);
        });
    }
    cells(app, ui, cr.body, track, spec, keys, &ss, KnobSize::M);
}

/// The dampers and the pedal: the pedal's travel as the display, in Edit the dampers' cells.
#[allow(clippy::too_many_arguments)]
fn damper_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    ls: &cantus_dsp::lamina::LaminaSettings,
    tele: &cantus_engine::Telemetry,
    rect: Rect,
    s: &Style,
    edit: bool,
) {
    let sd = s.function(Function::Dynamics);
    let display = if edit {
        rect.height() / s.k - card_h(0.0, 86.0) - 1.0
    } else {
        rect.height() / s.k - 24.0 - 1.0 - 6.0
    };
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("epiano.damper", Function::Dynamics).display(display),
    );
    if let Some(d) = cr.display {
        keys::pedal(
            app,
            ui,
            track,
            ls,
            tele[tel::PEDAL],
            engaged(ls, tele),
            d,
            &face::on_display(&sd),
        );
    }
    if edit {
        let keys = [
            "damper.time",
            "damper.tone",
            "damper.noise",
            "release.noise",
            "pedal.lift",
        ];
        cells(app, ui, cr.body, track, spec, &keys, &sd, KnobSize::M);
    }
}

/// The tremolo: its mode in the title, a second of it as the display (Edit), its cells.
#[allow(clippy::too_many_arguments)]
fn tremolo_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    ls: &cantus_dsp::lamina::LaminaSettings,
    now: f32,
    rect: Rect,
    s: &Style,
    display: bool,
) {
    let sm = s.function(Function::Mod);
    let tail = 196.0;
    let mut c = Card::new("epiano.tremolo", Function::Mod).tail(tail);
    if display {
        c = c.display((rect.height() / s.k - card_h(0.0, 86.0) - 1.0).max(24.0));
    }
    let cr = paint_card(ui, rect, s, &c);
    switch(
        app,
        ui,
        tail_rect(&cr, s, tail),
        track,
        spec,
        "trem.mode",
        &TREMS,
        &sm,
    );
    if let Some(d) = cr.display {
        let sd = face::on_display(&sm);
        with_brush(&ui.painter_at(d), |b| {
            keys::tremolo(b, ls, now, d, &sd);
            face::cover(b, d, &sd);
        });
    }
    cells(
        app,
        ui,
        cr.body,
        track,
        spec,
        &["trem.rate", "trem.depth"],
        &sm,
        KnobSize::M,
    );
}

/// The Play view: the pickup with the tine's swing on it beside the tine and the pedal,
/// and the numbers a player turns.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let ls = settings(spec);
    let tele = app.inst_telemetry();
    let area = ui.max_rect();
    app.inst.epiano.area = Some(area);
    app.inst.epiano.view = Some(ui.clip_rect());
    let gutter = s.d(4.0);
    let h2 = s.s(card_h(0.0, 86.0));
    let h1 = (area.height() - h2 - gutter).max(s.s(260.0));
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(h1, &[0.57, 0.43]);
    let r2 = rows.row(h2, &[0.37, 0.38, 0.25]);
    pickup_card(app, ui, track, spec, &ls, &tele, r1[0], s, false);
    let pedal_h = s.s(24.0 + 1.0 + 70.0 + 6.0);
    let tine_h = (h1 - gutter - pedal_h).max(s.s(card_h(40.0, 86.0)));
    let col = [
        Rect::from_min_size(r1[1].min, Vec2::new(r1[1].width(), tine_h)),
        Rect::from_min_max(
            Pos2::new(r1[1].left(), r1[1].top() + tine_h + gutter),
            r1[1].max,
        ),
    ];
    let decay = if ls.engine == Engine::Tine {
        ["hard", "tine.decay", "tine.bell"]
    } else {
        ["hard", "reed.decay", "reed.overtones"]
    };
    resonator_card(
        app,
        ui,
        track,
        spec,
        &ls,
        shown_key(&tele),
        col[0],
        s,
        &decay,
    );
    damper_card(app, ui, track, spec, &ls, &tele, col[1], s, false);
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[0],
        s,
        "epiano.preamp",
        Function::Dynamics,
        &["drive", "bass", "treble"],
    );
    tremolo_card(app, ui, track, spec, &ls, tele[tel::TREM], r2[1], s, false);
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[2],
        s,
        "epiano.output",
        Function::Output,
        &["dynamics", "gain"],
    );
}

/// The Edit view: every part of the instrument, each with its picture — pickup, tine or
/// reed, hammer, preamp, dampers and pedal, tremolo, playing, and the key curves. Taller
/// than the window, it scrolls there.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let ls = settings(spec);
    let tele = app.inst_telemetry();
    let gutter = s.d(4.0);
    let heights = [
        s.s(card_h(150.0, 86.0)),
        s.s(card_h(0.0, 86.0)),
        s.s(card_h(64.0, 86.0)),
        s.s(card_h(0.0, 86.0)),
        s.s(24.0 + 1.0 + 176.0 + 6.0),
    ];
    let total = heights.iter().sum::<f32>() + gutter * (heights.len() - 1) as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total), Sense::hover());
    app.inst.epiano.area = Some(area);
    app.inst.epiano.view = Some(ui.clip_rect());
    let mut rows = Rows::new(area, s);

    let r1 = rows.row(heights[0], &[0.57, 0.43]);
    pickup_card(app, ui, track, spec, &ls, &tele, r1[0], s, true);
    resonator_card(
        app,
        ui,
        track,
        spec,
        &ls,
        shown_key(&tele),
        r1[1],
        s,
        resonator_keys(ls.engine),
    );

    let r2 = rows.row(heights[1], &[0.52, 0.48]);
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[0],
        s,
        "epiano.hammer",
        Function::Dynamics,
        &["hard", "hard.vel", "force", "strike", "hammer.noise"],
    );
    preamp_card(app, ui, track, spec, r2[1], s);

    let r3 = rows.row(heights[2], &[0.6, 0.4]);
    damper_card(app, ui, track, spec, &ls, &tele, r3[0], s, true);
    tremolo_card(app, ui, track, spec, &ls, tele[tel::TREM], r3[1], s, true);

    let r4 = rows.row(heights[3], &[1.0]);
    playing_card(app, ui, track, spec, r4[0], s);

    let r5 = rows.row(heights[4], &[1.0]);
    curves_card(app, ui, track, &ls, r5[0], s);
}

/// The preamp: drive and the two-band tone, its output (direct or the speaker) in the title.
fn preamp_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let st = s.function(Function::Tone);
    let tail = 150.0;
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("epiano.preamp", Function::Tone).tail(tail),
    );
    switch(
        app,
        ui,
        tail_rect(&cr, s, tail),
        track,
        spec,
        "output",
        &OUTPUTS,
        &st,
    );
    cells(
        app,
        ui,
        cr.body,
        track,
        spec,
        &["drive", "bass", "treble"],
        &st,
        KnobSize::M,
    );
}

/// Playing: dynamics, velocity, voices, age and tuning; the keyboard's range in the title.
fn playing_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let so = s.function(Function::Output);
    let tail = 228.0;
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("epiano.playing", Function::Output).tail(tail),
    );
    switch(
        app,
        ui,
        tail_rect(&cr, s, tail),
        track,
        spec,
        "range",
        &RANGES,
        &so,
    );
    let keys = [
        "dynamics",
        "vel.curve",
        "polyphony",
        "age",
        "spread",
        "a4",
        "gain",
    ];
    cells(app, ui, cr.body, track, spec, &keys, &so, KnobSize::M);
}

/// The key curves: which one in the title, the curve over the keys as the display.
fn curves_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    ls: &cantus_dsp::lamina::LaminaSettings,
    rect: Rect,
    s: &Style,
) {
    let sm = s.function(Function::Mod);
    let tail = 240.0;
    let display = rect.height() / s.k - 24.0 - 1.0 - 6.0;
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("epiano.key_curve", Function::Mod)
            .display(display)
            .tail(tail),
    );
    keys::curve_switch(app, ui, track, tail_rect(&cr, s, tail), &sm);
    if let Some(d) = cr.display {
        keys::key_curves(app, ui, track, ls, d, &face::on_display(&sm));
    }
}

/// The inspector's small picture: the engine and its pickup in millimetres.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(Function::Source);
    let ls = settings(spec);
    let (engine, gap, align) = match ls.engine {
        Engine::Tine => (t("epiano.engine_tine"), ls.tine_gap_mm, ls.tine_align_mm),
        Engine::Reed => (t("epiano.engine_reed"), ls.reed_gap_mm, ls.reed_align_mm),
    };
    let text = format!(
        "{} \u{b7} {engine} \u{b7} {gap:.1} mm / {align:+.1} mm",
        t(spec.kind.name_key())
    );
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(36.0)), Sense::hover());
    with_brush(&ui.painter_at(rect), |b| {
        well::paint(b, rect, &s, KitState::REST, None);
        // A tine and its pickup, in the device hue.
        let y = rect.center().y;
        let x0 = rect.left() + s.s(8.0);
        b.rect(
            b.snap_rect(Rect::from_min_max(
                Pos2::new(x0, y - s.s(6.0)),
                Pos2::new(x0 + s.s(5.0), y + s.s(6.0)),
            )),
            Fill::Solid(s.t.text_dim),
        );
        b.segment(
            Pos2::new(x0 + s.s(5.0), y),
            Pos2::new(x0 + s.s(38.0), y),
            s.s(2.0),
            s.hue,
            true,
        );
        b.rect(
            b.snap_rect(Rect::from_min_max(
                Pos2::new(x0 + s.s(42.0), y - s.s(7.0)),
                Pos2::new(x0 + s.s(47.0), y + s.s(7.0)),
            )),
            Fill::Solid(s.t.text_dim),
        );
        let tr = Rect::from_min_max(
            Pos2::new(x0 + s.s(56.0), rect.top()),
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

/// `--editor epiano`: a suitcase with its tremolo, a note mid-swing.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "epiano.preset.suitcase")
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::ElectricPiano, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    app.set_inst_value(track, "curve.hard.6", 0.15, false);
    app.set_inst_value(track, "curve.hard.7", 0.25, false);
    app.set_inst_value(track, "curve.decay.2", 1.4, false);
    // With no audio device (the screenshot tool), show what a playing engine reports.
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: a C4 struck hard, swinging across the pickup,
/// the pedal half down.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0; cantus_engine::TELEMETRY_LEN];
    t[0] = 5.0;
    t[1] = 32.0;
    t[tel::KEY] = 60.0;
    t[tel::VELOCITY] = 0.9;
    t[tel::X] = 0.7;
    t[tel::SWING_LO] = -1.5;
    t[tel::SWING_HI] = 1.7;
    t[tel::PEDAL] = 0.38;
    t[tel::ENGAGEMENT] = engagement(0.38, 0.35);
    t[tel::TREM] = 0.3;
    t[tel::PEAK_L] = 0.4;
    t[tel::PEAK_R] = 0.35;
    t[tel::ALIVE] = 1.0;
    app.inst.injected = Some(t);
}

// ---- factory presets --------------------------------------------------------------------

/// Macros for the tine presets: 1 bark (the pickup closer), 2 drive, 3 tremolo, 4 hardness.
const TINE_MACROS: &[(&str, &str)] = &[
    ("macro1.name", "Bark"),
    ("mod1.src", "macro1"),
    ("mod1.dst", "tine.gap"),
    ("macro2.name", "Drive"),
    ("mod2.src", "macro2"),
    ("mod2.dst", "drive"),
    ("macro3.name", "Tremolo"),
    ("mod3.src", "macro3"),
    ("mod3.dst", "trem.depth"),
    ("macro4.name", "Hardness"),
    ("mod4.src", "macro4"),
    ("mod4.dst", "hard"),
];
/// The same four for the reed presets (bark moves the reed's slot).
const REED_MACROS: &[(&str, &str)] = &[
    ("macro1.name", "Bark"),
    ("mod1.src", "macro1"),
    ("mod1.dst", "reed.gap"),
    ("macro2.name", "Drive"),
    ("mod2.src", "macro2"),
    ("mod2.dst", "drive"),
    ("macro3.name", "Tremolo"),
    ("mod3.src", "macro3"),
    ("mod3.dst", "trem.depth"),
    ("macro4.name", "Hardness"),
    ("mod4.src", "macro4"),
    ("mod4.dst", "hard"),
];

macro_rules! preset {
    ($label:expr, $text:expr, $tags:expr, [$($k:expr => $v:expr),* $(,)?]) => {
        FactoryPreset {
            label_key: $label,
            values: &[
                ("mod1.amt", -0.15),
                ("mod2.amt", 0.5),
                ("mod3.amt", 0.8),
                ("mod4.amt", 0.4),
                $(($k, $v)),*
            ],
            text: $text,
            tags: $tags,
        }
    };
}

pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    preset!("epiano.preset.suitcase", TINE_MACROS, &["preset.tag.keys", "epiano.tag.tine"], [
        "trem.depth" => 0.55, "trem.rate" => 4.8, "drive" => 0.25, "tine.bell" => -12.0,
    ]),
    preset!("epiano.preset.stage", TINE_MACROS, &["preset.tag.keys", "epiano.tag.tine"], [
        "tine.gap" => 1.4, "tine.align" => 0.8, "drive" => 0.3, "tine.bell" => -10.0, "hard" => 0.55,
    ]),
    preset!("epiano.preset.bark", TINE_MACROS, &["preset.tag.keys", "epiano.tag.tine"], [
        "tine.gap" => 0.9, "tine.align" => 0.5, "hard" => 0.72, "drive" => 0.5, "dynamics" => 44.0,
        "treble" => 2.0,
    ]),
    preset!("epiano.preset.ballad", TINE_MACROS, &["preset.tag.keys", "epiano.tag.soft"], [
        "hard" => 0.3, "tine.bell" => -18.0, "tine.decay" => 12.0, "trem.depth" => 0.3,
        "trem.rate" => 3.5, "drive" => 0.1, "hammer.noise" => -30.0,
    ]),
    preset!("epiano.preset.bells", TINE_MACROS, &["preset.tag.keys", "epiano.tag.tine"], [
        "tine.bell" => -4.0, "tine.bell_decay" => 0.6, "strike" => 0.05, "hard" => 0.75,
        "tine.couple" => 0.7,
    ]),
    preset!("epiano.preset.worn", TINE_MACROS, &["preset.tag.keys", "epiano.tag.lofi"], [
        "age" => 0.8, "spread" => 6.0, "drive" => 0.4, "treble" => -6.0, "trem.mode" => 2.0,
        "trem.depth" => 0.25, "release.noise" => -24.0, "hammer.noise" => -18.0,
    ]),
    preset!("epiano.preset.octave", TINE_MACROS, &["preset.tag.keys", "epiano.tag.tine"], [
        "tine.align" => 0.0, "tine.gap" => 1.2, "drive" => 0.2, "tine.bell" => -16.0,
    ]),
    preset!("epiano.preset.bright", TINE_MACROS, &["preset.tag.keys", "epiano.tag.tine"], [
        "treble" => 6.0, "bass" => -2.0, "tine.bell" => -8.0, "drive" => 0.15, "hard" => 0.62,
    ]),
    preset!("epiano.preset.reed", REED_MACROS, &["preset.tag.keys", "epiano.tag.reed"], [
        "engine" => 1.0, "range" => 1.0, "trem.depth" => 0.4, "trem.rate" => 5.8, "drive" => 0.3,
        "dynamics" => 50.0, "gain" => 5.0,
    ]),
    preset!("epiano.preset.reed_growl", REED_MACROS, &["preset.tag.keys", "epiano.tag.reed"], [
        "engine" => 1.0, "range" => 1.0, "reed.gap" => 0.7, "reed.align" => 0.4, "hard" => 0.7,
        "drive" => 0.6, "dynamics" => 50.0,
    ]),
    preset!("epiano.preset.reed_soft", REED_MACROS, &["preset.tag.keys", "epiano.tag.soft"], [
        "engine" => 1.0, "range" => 1.0, "hard" => 0.35, "reed.overtones" => -16.0, "drive" => 0.1,
        "output" => 1.0, "gain" => 5.0,
    ]),
];

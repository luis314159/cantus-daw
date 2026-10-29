//! Organum — the tonewheel organ and rotary speaker: its editor, on the kit's device shell
//! (ADR 0024 §8). Owned by this instrument's builder (ADR 0023).
//!
//! What the benchmarks (B-3 V2, NI Vintage Organs; `research/instruments/keys.md` §5) get
//! wrong and what this does instead:
//!
//! - **No photograph.** The drawbars are the display and the control: drag them, footages
//!   printed under each, the registration written in the card's title (`88 8000 000`), a
//!   step's dB in the help line.
//! - **Units everywhere**: leakage and click in dB, percussion decay in ms to −60 dB,
//!   rotor speeds in rpm, spin-up in seconds, microphones in cm and degrees — where
//!   B-3 V2 shows "Tonewheel Leakage 0.012".
//! - **The rotary is state**: the horn and drum turn at the angle the engine reports; the
//!   spin-up curve shows where the rotors are on their way to the new speed.
//! - **Splits on one track**: the keyboard map shows which keys play the pedals, the lower
//!   and the upper manual; drag a boundary.
//!
//! The look (ADR 0024): **Play** is the upper drawbars beside the cabinet turning, the
//! switches a player reaches for between songs (percussion, vibrato and chorus, the rotor
//! speed) and a row of cells; **Edit** is every section in signal order — the three sets of
//! drawbars, splits, percussion, vibrato, tone, preamp and the rotary speaker — as cards in
//! their function hues. Every drag is one step of the project's undo.

#[path = "organ_cabinet.rs"]
mod cabinet;
#[path = "organ_drawbars.rs"]
mod drawbars;
#[path = "organ_parts.rs"]
pub(super) mod parts;

use super::face;
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::band::CardRects;
use crate::kit::hint::{self, Caps};
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Fill, Function, State as KitState, Style};
use cantus_core::inst::organ::{LOWER, PEDAL, UPPER, VIB_MODES};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::organum::OrganSettings;
use cantus_engine::inst::organ::{settings, tel};
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use parts::{at, cells, lamp, paint_card, split, stack, switch, tail_rect, Card, Rows};

pub use drawbars::registration;

/// Editor state that is not part of the project: where the last frame put the controls a
/// gesture can grab and the area it laid its cards out in (tests aim at them; the layout
/// stays the view's business).
#[derive(Debug, Default)]
pub struct State {
    pub bars: std::collections::HashMap<&'static str, egui::Rect>,
    /// Where each control of the last frame (cell, switch, lamp) sits, by parameter key.
    pub placed: std::collections::HashMap<&'static str, egui::Rect>,
    /// The rect the cards were laid out in, and the part of it that is visible.
    pub area: Option<egui::Rect>,
    pub view: Option<egui::Rect>,
}

/// Set a value (a gesture merges into one undo step until `end_gesture`).
pub(super) fn set(app: &mut CantusApp, track: TrackId, key: &str, v: f32, gesture: bool) {
    app.set_inst_value(track, key, v, gesture);
}

const SPEEDS: [Src<'static>; 3] = [
    Src::Key("organ.speed_stop"),
    Src::Key("organ.speed_slow"),
    Src::Key("organ.speed_fast"),
];
const HARMS: [Src<'static>; 2] = [Src::Key("organ.perc_2nd"), Src::Key("organ.perc_3rd")];
const DECAYS: [Src<'static>; 2] = [
    Src::Key("organ.perc_fast_sw"),
    Src::Key("organ.perc_slow_sw"),
];
const VOLS: [Src<'static>; 2] = [
    Src::Key("organ.perc_normal"),
    Src::Key("organ.perc_soft_sw"),
];
const MODES: [Src<'static>; 6] = [
    Src::Str(VIB_MODES[0]),
    Src::Str(VIB_MODES[1]),
    Src::Str(VIB_MODES[2]),
    Src::Str(VIB_MODES[3]),
    Src::Str(VIB_MODES[4]),
    Src::Str(VIB_MODES[5]),
];
const TUNINGS: [Src<'static>; 2] = [
    Src::Key("organ.tuning_gear"),
    Src::Key("organ.tuning_equal"),
];
const CABS: [Src<'static>; 2] = [Src::Key("organ.cab_closed"), Src::Key("organ.cab_open")];

/// The height of a card with a `display` and a `body`, at 100 %: title, display, body with
/// its padding.
fn card_h(display: f32, body: f32) -> f32 {
    24.0 + if display > 0.0 { 1.0 + display } else { 0.0 } + 8.0 + body + 8.0
}

/// A card's power LED over the on/off parameter `key` (lit at `on`); a click flips it.
#[allow(clippy::too_many_arguments)]
fn power(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    cr: &CardRects,
    track: TrackId,
    key: &str,
    on: bool,
    lit_value: f32,
    s: &Style,
) {
    let Some(led) = cr.led else {
        return;
    };
    let resp = ui.interact(
        led.expand(s.s(4.0)),
        egui::Id::new(("organ-power", track.0, key)),
        Sense::click(),
    );
    if resp.hovered() {
        if let Some(info) = cantus_core::InstrumentKind::Organ.param(key) {
            hint::set_text(ui.ctx(), &t(info.label_key), "", Caps::PARAM);
        }
    }
    // The card's lamp is its parameter's control (round 29, review 3 of ADR 0062, #35).
    let before = app.learn_armed_at();
    let learning = crate::controllers::stepped_inst_of(
        app,
        ui,
        track,
        cantus_core::InstrumentKind::Organ,
        key,
        &resp,
    );
    // A lamp lit at 0 (the rotary's, lit while its output is not taken off) is learnt
    // the lamp's way: *On* lights it, as feedback does (review 4, #53).
    let now = app.learn_armed_at();
    if lit_value < 0.5 && now.is_some() && now != before {
        let target =
            cantus_core::controller::MappingTarget::Param(cantus_core::AutoTarget::Instrument {
                track,
                key: key.to_string(),
            });
        app.set_learn_range(&target, 1.0, 0.0);
    }
    if resp.clicked() && !learning {
        let v = if on { 1.0 - lit_value } else { lit_value };
        set(app, track, key, v, false);
    }
}

/// A card of cells: the title, and `keys` as cells of `size` across its body.
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
    size: KnobSize,
) -> CardRects {
    let cr = paint_card(ui, rect, s, &Card::new(title, f));
    cells(app, ui, cr.body, track, spec, keys, &s.function(f), size);
    cr
}

/// A set of drawbars as a card: the bars are its display, the registration its title's
/// readout.
#[allow(clippy::too_many_arguments)]
fn drawbar_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    title: &str,
    keys: &[&'static str],
    footages: &[&str],
    kinds: &[drawbars::Kind],
) {
    let st = s.function(Function::Tone);
    let mut values = [0.0f32; 9];
    for (v, k) in values.iter_mut().zip(keys) {
        *v = spec.get(k);
    }
    // The registration, with spaces that do not split it into number and unit.
    let reg = registration(&values[..keys.len()]).replace(' ', "\u{a0}");
    let tail = if keys.len() == 9 { 92.0 } else { 40.0 };
    let display = rect.height() / s.k - 24.0 - 1.0 - 6.0;
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new(title, Function::Tone).display(display).tail(tail),
    );
    let tr = tail_rect(&cr, s, tail);
    with_brush(&ui.painter_at(tr.expand(s.s(2.0))), |b| {
        well::readout(b, tr, &st, KitState::REST, &reg, true)
    });
    if let Some(d) = cr.display {
        let sd = face::on_display(&st);
        drawbars::drawbars(app, ui, track, spec, d, &sd, keys, footages, kinds);
    }
}

/// The percussion: its decay as the card's display, the switches, and in Edit its cells.
#[allow(clippy::too_many_arguments)]
fn percussion_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    o: &OrganSettings,
    env: f32,
    rect: Rect,
    s: &Style,
    edit: bool,
) {
    let sd = s.function(Function::Dynamics);
    let on = o.perc_on;
    let body = if edit { 86.0 } else { 24.0 + 8.0 + 24.0 };
    let display = (rect.height() / s.k - card_h(0.0, body) - 1.0).max(24.0);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("organ.percussion", Function::Dynamics)
            .power(on)
            .display(display),
    );
    power(app, ui, &cr, track, "perc", on, 1.0, s);
    if let Some(d) = cr.display {
        let fd = face::on_display(&sd);
        with_brush(&ui.painter_at(d), |b| {
            cabinet::percussion(b, o, env, d, &fd);
            face::cover(b, d, &fd);
        });
    }
    at(ui, cr.body, ("organ-perc", track.0), |ui| {
        if !on {
            ui.multiply_opacity(0.45);
        }
        let g = s.s(6.0);
        let (switches, knobs) = if edit {
            let cols = split(cr.body, s.s(10.0), &[0.52, 0.48]);
            (cols[0], Some(cols[1]))
        } else {
            (cr.body, None)
        };
        let rows = stack(switches, s.s(24.0), s.s(8.0), 2);
        let top = split(rows[0], g, &[0.5, 0.5]);
        let low = split(rows[1], g, &[0.5, 0.5]);
        switch(app, ui, top[0], track, spec, "perc.harm", &HARMS, &sd);
        switch(app, ui, top[1], track, spec, "perc.decay", &DECAYS, &sd);
        switch(app, ui, low[0], track, spec, "perc.vol", &VOLS, &sd);
        let led = Rect::from_min_size(low[1].min, Vec2::new(low[1].width(), s.s(20.0)))
            .translate(Vec2::new(0.0, s.s(2.0)));
        lamp(
            app,
            ui,
            led,
            track,
            spec,
            "perc.steal",
            Src::Key("organ.perc_steal"),
            &sd,
        );
        if let Some(r) = knobs {
            let keys = ["perc.fast", "perc.slow", "perc.soft", "perc.drop"];
            cells(app, ui, r, track, spec, &keys, &sd, KnobSize::M);
        }
    });
}

/// The vibrato and chorus scanner: the line and its pickup as the display, the six modes,
/// the two manuals it acts on, and in Edit its rate.
#[allow(clippy::too_many_arguments)]
fn vibrato_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    o: &OrganSettings,
    position: f32,
    rect: Rect,
    s: &Style,
    edit: bool,
) {
    let sm = s.function(Function::Mod);
    let body = if edit { 86.0 } else { 24.0 + 6.0 + 20.0 };
    let display = (rect.height() / s.k - card_h(0.0, body) - 1.0).max(24.0);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("organ.vibrato", Function::Mod).display(display),
    );
    if let Some(d) = cr.display {
        let fd = face::on_display(&sm);
        with_brush(&ui.painter_at(d), |b| {
            cabinet::scanner(b, o, position, d, &fd);
            face::cover(b, d, &fd);
        });
    }
    let (controls, rate) = if edit {
        let c = split(cr.body, s.s(8.0), &[0.72, 0.28]);
        (c[0], Some(c[1]))
    } else {
        (cr.body, None)
    };
    let top = Rect::from_min_size(controls.min, Vec2::new(controls.width(), s.s(24.0)));
    switch(app, ui, top, track, spec, "vib", &MODES, &sm);
    let leds = Rect::from_min_size(
        Pos2::new(controls.left(), top.bottom() + s.s(6.0)),
        Vec2::new(controls.width(), s.s(20.0)),
    );
    let halves = split(leds, s.s(6.0), &[0.5, 0.5]);
    for (r, key, label) in [
        (halves[0], "vib.upper", "organ.vib_upper"),
        (halves[1], "vib.lower", "organ.vib_lower"),
    ] {
        lamp(app, ui, r, track, spec, key, Src::Key(label), &sm);
    }
    if let Some(r) = rate {
        cells(app, ui, r, track, spec, &["vib.rate"], &sm, KnobSize::M);
    }
}

/// The Play view: the upper drawbars and the cabinet turning, the switches a player reaches
/// for between songs, and the few numbers that matter live.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let o = settings(spec);
    let tele = app.inst_telemetry();
    let rotors = cabinet::Rotors::of(&o.rotary, &tele);
    let area = ui.max_rect();
    app.inst.organ.area = Some(area);
    app.inst.organ.view = Some(ui.clip_rect());
    let gutter = s.d(4.0);
    let h2 = s.s(card_h(0.0, 86.0));
    let h1 = (area.height() - h2 - gutter).max(s.s(260.0));
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(h1, &[0.40, 0.31, 0.29]);
    let r2 = rows.row(h2, &[0.36, 0.32, 0.32]);
    drawbar_card(
        app,
        ui,
        track,
        spec,
        r1[0],
        s,
        "organ.upper_manual",
        &UPPER,
        &drawbars::manual_footages(),
        &drawbars::MANUAL_KINDS,
    );
    rotary_play(app, ui, track, spec, &o, &rotors, r1[1], s);
    let col = stack(r1[2], (h1 - gutter) * 0.5, gutter, 2);
    percussion_card(app, ui, track, spec, &o, tele[tel::PERC], col[0], s, false);
    vibrato_card(
        app,
        ui,
        track,
        spec,
        &o,
        tele[tel::SCANNER],
        col[1],
        s,
        false,
    );
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[0],
        s,
        "organ.preamp",
        Function::Dynamics,
        &["swell", "drive", "click"],
        KnobSize::M,
    );
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[1],
        s,
        "organ.tone",
        Function::Tone,
        &["bright", "leak", "click.rel"],
        KnobSize::M,
    );
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[2],
        s,
        "organ.output",
        Function::Output,
        &["lvl.upper", "lvl.lower", "gain"],
        KnobSize::M,
    );
}

/// Play's rotary card: the cabinet from above as its display, the speed switch below.
#[allow(clippy::too_many_arguments)]
fn rotary_play(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    o: &OrganSettings,
    rotors: &cabinet::Rotors,
    rect: Rect,
    s: &Style,
) {
    let sp = s.function(Function::Space);
    let display = rect.height() / s.k - card_h(0.0, 24.0) - 1.0;
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("organ.rotary", Function::Space)
            .power(o.rotary_on)
            .display(display),
    );
    power(app, ui, &cr, track, "rot.out", o.rotary_on, 0.0, s);
    if let Some(d) = cr.display {
        cabinet::cabinet(app, ui, track, o, rotors, d, &face::on_display(&sp));
    }
    let r = Rect::from_min_size(cr.body.min, Vec2::new(cr.body.width(), s.s(24.0)));
    switch(app, ui, r, track, spec, "rot.speed", &SPEEDS, &sp);
}

/// The Edit view: every section in signal order — the drawbars, the splits, percussion,
/// vibrato, tone, preamp, and the rotary speaker with its spin-up. Taller than the window,
/// it scrolls there.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let o = settings(spec);
    let tele = app.inst_telemetry();
    let rotors = cabinet::Rotors::of(&o.rotary, &tele);
    let gutter = s.d(4.0);
    let heights = [
        s.s(250.0),
        s.s(card_h(56.0, 86.0)),
        s.s(card_h(56.0, 86.0)),
        s.s(card_h(190.0, 24.0 + 8.0 + 86.0)),
    ];
    let total = heights.iter().sum::<f32>() + gutter * (heights.len() - 1) as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total), Sense::hover());
    app.inst.organ.area = Some(area);
    app.inst.organ.view = Some(ui.clip_rect());
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(heights[0], &[0.42, 0.42, 0.16]);
    let foot = drawbars::manual_footages();
    for (rect, title, keys) in [
        (r1[0], "organ.upper_manual", &UPPER),
        (r1[1], "organ.lower_manual", &LOWER),
    ] {
        drawbar_card(
            app,
            ui,
            track,
            spec,
            rect,
            s,
            title,
            keys,
            &foot,
            &drawbars::MANUAL_KINDS,
        );
    }
    drawbar_card(
        app,
        ui,
        track,
        spec,
        r1[2],
        s,
        "organ.pedals",
        &PEDAL,
        &drawbars::PEDAL_FOOTAGES,
        &drawbars::PEDAL_KINDS,
    );

    let r2 = rows.row(heights[1], &[0.36, 0.64]);
    splits_card(app, ui, track, spec, r2[0], s);
    percussion_card(app, ui, track, spec, &o, tele[tel::PERC], r2[1], s, true);

    let r3 = rows.row(heights[2], &[0.30, 0.44, 0.26]);
    vibrato_card(app, ui, track, spec, &o, tele[tel::SCANNER], r3[0], s, true);
    tone_card(app, ui, track, spec, r3[1], s);
    cells_card(
        app,
        ui,
        track,
        spec,
        r3[2],
        s,
        "organ.preamp",
        Function::Dynamics,
        &["swell", "drive", "gain"],
        KnobSize::M,
    );

    let r4 = rows.row(heights[3], &[1.0]);
    rotary_edit(app, ui, track, spec, &o, &rotors, r4[0], s);
}

/// The splits: the keyboard map as the display, each division's level as a cell.
fn splits_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let st = s.function(Function::Tone);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("organ.splits", Function::Tone).display(56.0),
    );
    if let Some(d) = cr.display {
        cabinet::splits(app, ui, track, spec, d, &face::on_display(&st));
    }
    let keys = ["lvl.upper", "lvl.lower", "lvl.pedal"];
    cells(app, ui, cr.body, track, spec, &keys, &st, KnobSize::M);
}

/// Tone: key click, leakage, brilliance and the tuning; the tuning's switch in the title.
fn tone_card(
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
        &Card::new("organ.tone", Function::Tone).tail(tail),
    );
    let tr = tail_rect(&cr, s, tail);
    switch(app, ui, tr, track, spec, "tuning", &TUNINGS, &st);
    let keys = ["click", "click.rel", "bright", "leak", "leak.bar", "a4"];
    cells(app, ui, cr.body, track, spec, &keys, &st, KnobSize::M);
}

/// Edit's rotary speaker: the cabinet and the spin-up side by side as its display, the
/// speed and the cabinet's back below, then every rotor number.
#[allow(clippy::too_many_arguments)]
fn rotary_edit(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    o: &OrganSettings,
    rotors: &cabinet::Rotors,
    rect: Rect,
    s: &Style,
) {
    let sp = s.function(Function::Space);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("organ.rotary", Function::Space)
            .power(o.rotary_on)
            .display(190.0),
    );
    power(app, ui, &cr, track, "rot.out", o.rotary_on, 0.0, s);
    if let Some(d) = cr.display {
        let halves = split(d, 0.0, &[0.34, 0.66]);
        let fd = face::on_display(&sp);
        cabinet::cabinet(app, ui, track, o, rotors, halves[0], &fd);
        with_brush(&ui.painter_at(halves[1]), |b| {
            let x = b.snap_mid(halves[1].left());
            b.vline(x, d.top(), d.bottom(), b.px(), fd.t.edge);
            cabinet::ramp(b, o, rotors, halves[1], &fd);
        });
    }
    let top = Rect::from_min_size(cr.body.min, Vec2::new(cr.body.width(), s.s(24.0)));
    let sw = split(top, s.s(10.0), &[0.34, 0.2, 0.46]);
    switch(app, ui, sw[0], track, spec, "rot.speed", &SPEEDS, &sp);
    switch(app, ui, sw[1], track, spec, "rot.cab", &CABS, &sp);
    let row = Rect::from_min_max(
        Pos2::new(cr.body.left(), top.bottom() + s.s(8.0)),
        cr.body.max,
    );
    let keys = [
        "rot.horn_slow",
        "rot.horn_fast",
        "rot.horn_up",
        "rot.horn_down",
        "rot.drum_slow",
        "rot.drum_fast",
        "rot.drum_up",
        "rot.drum_down",
        "rot.balance",
        "rot.xover",
        "rot.dist",
        "rot.spread",
        "rot.brake",
    ];
    cells(app, ui, row, track, spec, &keys, &sp, KnobSize::M);
}

/// The inspector's small picture: the upper registration as nine little bars, the name,
/// the registration and the rotor speed.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(Function::Tone);
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(36.0)), Sense::hover());
    let mut values = [0.0f32; 9];
    for (v, k) in values.iter_mut().zip(UPPER) {
        *v = spec.get(k);
    }
    let speed = SPEEDS[spec.get("rot.speed").round().clamp(0.0, 2.0) as usize];
    let speed = match speed {
        Src::Key(k) => t(k),
        Src::Str(x) => x.to_string(),
    };
    let text = format!(
        "{} \u{b7} {} \u{b7} {}",
        t(spec.kind.name_key()),
        registration(&values),
        speed
    );
    with_brush(&ui.painter_at(rect), |b| {
        well::paint(b, rect, &s, KitState::REST, None);
        let bars = Rect::from_min_size(
            rect.min + Vec2::new(s.s(6.0), s.s(5.0)),
            Vec2::new(s.s(9.0 * 7.0), rect.height() - s.s(10.0)),
        );
        for (i, v) in values.iter().enumerate() {
            let x = bars.left() + i as f32 * s.s(7.0);
            let h = bars.height() * (v / 8.0).clamp(0.0, 1.0);
            let r = Rect::from_min_max(
                Pos2::new(x, bars.top()),
                Pos2::new(x + s.s(5.0), bars.top() + h.max(s.s(1.0))),
            );
            b.rect(b.snap_rect(r), Fill::Solid(s.hue));
        }
        let tr = Rect::from_min_max(
            Pos2::new(bars.right() + s.s(8.0), rect.top()),
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

/// `--editor organ`: a jazz registration with percussion and a split, rotors fast.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "organ.preset.jazz_trio")
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Organ, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    app.set_inst_value(track, "rot.speed", 2.0, false);
    // With no audio device (the screenshot tool), show what a playing engine reports.
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: the rotors caught mid-turn, spinning up.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0; cantus_engine::TELEMETRY_LEN];
    t[0] = 4.0;
    t[1] = cantus_engine::inst::organ::KEYS as f32;
    t[tel::HORN_ANGLE] = 0.13;
    t[tel::DRUM_ANGLE] = 0.62;
    t[tel::HORN_RPM] = 318.0;
    t[tel::DRUM_RPM] = 131.0;
    t[tel::SCANNER] = 0.64;
    t[tel::PERC] = 0.35;
    t[tel::PEAK_L] = 0.4;
    t[tel::PEAK_R] = 0.38;
    t[tel::SPEED] = 2.0;
    t[tel::HELD] = 3.0;
    t[tel::HELD + 2] = 1.0;
    t[tel::ALIVE] = 1.0;
    app.inst.injected = Some(t);
}

// ---- factory presets --------------------------------------------------------------------

/// Macros every preset names the same way: 1 drive, 2 rotor speed (past half: fast),
/// 3 key click, 4 brilliance.
const MACROS: &[(&str, &str)] = &[
    ("macro1.name", "Drive"),
    ("mod1.src", "macro1"),
    ("mod1.dst", "drive"),
    ("macro2.name", "Fast"),
    ("mod2.src", "macro2"),
    ("mod2.dst", "rot.speed"),
    ("macro3.name", "Click"),
    ("mod3.src", "macro3"),
    ("mod3.dst", "click"),
    ("macro4.name", "Brilliance"),
    ("mod4.src", "macro4"),
    ("mod4.dst", "bright"),
];

/// A registration as values: `[16′, 5⅓′, 8′, 4′, 2⅔′, 2′, 1⅗′, 1⅓′, 1′]`.
macro_rules! upper {
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr) => {
        [
            ("u1", $a),
            ("u2", $b),
            ("u3", $c),
            ("u4", $d),
            ("u5", $e),
            ("u6", $f),
            ("u7", $g),
            ("u8", $h),
            ("u9", $i),
        ]
    };
}

const MACRO_DEPTHS: [(&str, f32); 4] = [
    ("mod1.amt", 0.5),
    ("mod2.amt", 0.25),
    ("mod3.amt", 0.3),
    ("mod4.amt", 0.5),
];

const fn concat<const A: usize, const B: usize, const C: usize>(
    a: [(&'static str, f32); A],
    b: [(&'static str, f32); B],
) -> [(&'static str, f32); C] {
    let mut out = [("", 0.0); C];
    let mut i = 0;
    while i < A {
        out[i] = a[i];
        i += 1;
    }
    let mut j = 0;
    while j < B {
        out[A + j] = b[j];
        j += 1;
    }
    out
}

const JAZZ: [(&str, f32); 21] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("perc", 1.0),
        ("perc.harm", 1.0),
        ("vib.upper", 0.0),
        ("split.pedal", 48.0),
        ("split.upper", 60.0),
        ("l3", 6.0),
        ("l4", 6.0),
        ("drive", 0.15),
    ],
);
const FULL: [(&str, f32); 17] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0),
        MACRO_DEPTHS,
    ),
    [
        ("drive", 0.6),
        ("rot.speed", 2.0),
        ("click", 0.0),
        ("lvl.upper", -4.0),
    ],
);
const GOSPEL: [(&str, f32); 17] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 8.0),
        MACRO_DEPTHS,
    ),
    [
        ("drive", 0.35),
        ("rot.speed", 2.0),
        ("click", -3.0),
        ("vib", 5.0),
    ],
);
const BALLAD: [(&str, f32); 17] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 3.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("drive", 0.05),
        ("click", -18.0),
        ("vib", 5.0),
        ("lvl.upper", 0.0),
    ],
);
const SOUL: [(&str, f32); 17] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("drive", 0.3),
        ("vib.upper", 0.0),
        ("click", 0.0),
        ("perc.vol", 0.0),
    ],
);
const CHORALE: [(&str, f32); 17] = concat(
    concat::<9, 4, 13>(
        upper!(6.0, 8.0, 8.0, 6.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("vib", 5.0),
        ("drive", 0.1),
        ("click", -24.0),
        ("rot.speed", 1.0),
    ],
);
const REGGAE: [(&str, f32); 18] = concat(
    concat::<9, 4, 13>(
        upper!(0.0, 0.0, 8.0, 0.0, 8.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("perc", 1.0),
        ("perc.harm", 0.0),
        ("perc.vol", 1.0),
        ("vib.upper", 0.0),
        ("click", 0.0),
    ],
);
const FLUTE: [(&str, f32); 16] = concat(
    concat::<9, 4, 13>(
        upper!(0.0, 0.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [("vib", 0.0), ("click", -30.0), ("drive", 0.0)],
);
const GROWL: [(&str, f32); 17] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("drive", 0.85),
        ("rot.speed", 2.0),
        ("click", 0.0),
        ("lvl.upper", -6.0),
    ],
);
const PARKED: [(&str, f32); 17] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("rot.speed", 0.0),
        ("rot.brake", 90.0),
        ("drive", 0.25),
        ("vib", 2.0),
    ],
);
const TRIO: [(&str, f32); 26] = concat(
    concat::<9, 4, 13>(
        upper!(8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        MACRO_DEPTHS,
    ),
    [
        ("perc", 1.0),
        ("split.pedal", 48.0),
        ("split.upper", 60.0),
        ("l1", 0.0),
        ("l3", 8.0),
        ("l4", 6.0),
        ("l6", 4.0),
        ("p1", 8.0),
        ("p2", 5.0),
        ("vib.lower", 1.0),
        ("lvl.lower", -6.0),
        ("lvl.pedal", -2.0),
        ("drive", 0.2),
    ],
);

pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    FactoryPreset {
        label_key: "organ.preset.jazz_trio",
        values: &JAZZ,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.jazz"],
    },
    FactoryPreset {
        label_key: "organ.preset.full",
        values: &FULL,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.rock"],
    },
    FactoryPreset {
        label_key: "organ.preset.gospel",
        values: &GOSPEL,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.gospel"],
    },
    FactoryPreset {
        label_key: "organ.preset.ballad",
        values: &BALLAD,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.soft"],
    },
    FactoryPreset {
        label_key: "organ.preset.soul",
        values: &SOUL,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.soul"],
    },
    FactoryPreset {
        label_key: "organ.preset.chorale",
        values: &CHORALE,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.soft"],
    },
    FactoryPreset {
        label_key: "organ.preset.reggae",
        values: &REGGAE,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.reggae"],
    },
    FactoryPreset {
        label_key: "organ.preset.flute",
        values: &FLUTE,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.soft"],
    },
    FactoryPreset {
        label_key: "organ.preset.growl",
        values: &GROWL,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.rock"],
    },
    FactoryPreset {
        label_key: "organ.preset.parked",
        values: &PARKED,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.soul"],
    },
    FactoryPreset {
        label_key: "organ.preset.trio",
        values: &TRIO,
        text: MACROS,
        tags: &["preset.tag.keys", "organ.tag.jazz"],
    },
];

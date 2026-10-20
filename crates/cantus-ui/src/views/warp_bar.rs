//! The warp editor's bar: the mode, the transposition (drag, semitones; Shift for cents),
//! the formants, *Follow the project tempo*, the source's tempo (drag; ½ and ×2), *Detect
//! tempo*, *Quantise audio…*, *Reset warp*, and the editor's own snap grid. What is not set
//! yet (a clip never warped) shows the value it would start from. What does nothing in
//! the clip's mode is drawn disabled, as the inspector draws it (round 29's review).

use super::mode_index;
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::{self, well, State, Style};
use cantus_core::warp::WarpMode;
use egui::{vec2, Sense};

/// The modes' words, in `WarpMode::ALL`'s order.
pub const MODES: [&str; 4] = [
    "warp.mode.complex",
    "warp.mode.rhythm",
    "warp.mode.melodic",
    "warp.mode.repitch",
];

/// A value dragged on the bar: its face, and the drag's value (`None` while still).
#[allow(clippy::too_many_arguments)]
fn drag_value(
    ui: &mut egui::Ui,
    app: &mut CantusApp,
    which: u8,
    face: &str,
    name: &str,
    now: f64,
    per_px: f64,
    width: f32,
    enabled: bool,
) -> (Option<f64>, bool) {
    let s = Style::current();
    // Its name before it: label and value (ADR 0024).
    let (lr, _) = ui.allocate_exact_size(vec2(s.s(96.0), s.d(24.0)), Sense::hover());
    let words = name;
    let tx = crate::kit::text::Text::new(words, crate::kit::text::Role::Label, s.t.text_dim)
        .align(egui::Align2::RIGHT_CENTER)
        .policy(crate::kit::text::Policy::Elide(
            crate::kit::text::Elide::End,
        ));
    kit::with_brush(ui.painter(), |b| {
        let _ = crate::kit::text::paint(b, lr, &tx, s.tk());
    });
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(s.s(width), s.d(24.0)), sense);
    let st = State {
        disabled: !enabled,
        ..State::of(&resp, false)
    };
    kit::with_brush(ui.painter(), |b| {
        well::readout(b, rect, &s, st, face, true);
    });
    if resp.hovered() || resp.dragged() {
        hint::set_text(ui.ctx(), name, face, Caps::PARAM);
    }
    if resp.drag_started() {
        app.warp
            .view_value_start(which, now, resp.interact_pointer_pos().map_or(0.0, |p| p.x));
    }
    let mut out = None;
    if resp.dragged() {
        if let (Some((w, v0, x0)), Some(p)) = (app.warp.view_value(), resp.interact_pointer_pos()) {
            if w == which {
                let fine = ui.input(|i| i.modifiers.shift);
                let k = if fine { per_px * 0.1 } else { per_px };
                out = Some(v0 + (p.x - x0) as f64 * k);
            }
        }
    }
    let ended = resp.drag_stopped();
    if ended {
        app.warp.view_value_end();
    }
    (out, ended)
}

/// A push key on the bar, drawn disabled when it would do nothing; whether it was clicked.
fn key(ui: &mut egui::Ui, label: &'static str, enabled: bool, width: f32) -> bool {
    let mut on = false;
    let r = kit::widgets::led_button_if(ui, &mut on, Src::Key(label), &t(label), width, enabled);
    r.changed()
}

/// The editor's snap grids: a quarter, an eighth, a sixteenth, a thirty-second, none.
const GRIDS: [Option<cantus_core::Ticks>; 5] = [
    Some(cantus_core::Ticks(cantus_core::PPQ)),
    Some(cantus_core::Ticks(cantus_core::PPQ / 2)),
    Some(cantus_core::Ticks(cantus_core::PPQ / 4)),
    Some(cantus_core::Ticks(cantus_core::PPQ / 8)),
    None,
];

pub fn bar(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef) {
    let s = Style::current();
    let Some(w) = app.warp_to_edit(r) else {
        return;
    };
    let warped = app.clip_warp(r).is_some_and(|w| w.follow);
    ui.allocate_ui(vec2(ui.available_width(), s.s(super::BAR_H)), |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("warp-bar")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut m = mode_index(w.mode);
                    let labels = MODES.map(Src::Key);
                    if kit::widgets::segmented(
                        ui,
                        &mut m,
                        &labels,
                        &t("warp.inspector.mode"),
                        300.0,
                    )
                    .changed()
                    {
                        // The mode alone: it does not warp the clip.
                        app.set_warp_mode(r, WarpMode::ALL[m]);
                    }
                    // The transposition (Re-pitch has none: its pitch is its speed).
                    let mut face = String::new();
                    crate::warp::write_cents(&mut face, w.cents);
                    let (v, ended) = drag_value(
                        ui,
                        app,
                        0,
                        &face,
                        &t("warp.inspector.transpose"),
                        w.cents as f64,
                        5.0,
                        90.0,
                        w.mode.transposes(),
                    );
                    if let Some(c) = v {
                        app.set_warp_cents(r, c.round() as i32, true);
                    }
                    if ended {
                        app.end_gesture();
                    }
                    let mut formants = w.formants;
                    if kit::widgets::led_button_if(
                        ui,
                        &mut formants,
                        Src::Key("warp.inspector.formants"),
                        &t("warp.inspector.formants"),
                        90.0,
                        w.mode.keeps_formants() && w.cents != 0,
                    )
                    .changed()
                    {
                        app.set_warp_formants(r, formants);
                    }
                    let mut follow = warped;
                    if kit::widgets::led_button(
                        ui,
                        &mut follow,
                        Src::Key("warp.inspector.follow_short"),
                        &t("warp.inspector.follow"),
                        90.0,
                    )
                    .clicked()
                    {
                        app.set_warp_follow(r, follow);
                    }
                    // The source's tempo.
                    let bpm = w.tempo();
                    let face = format!("{bpm:.2} BPM");
                    let (v, ended) = drag_value(
                        ui,
                        app,
                        1,
                        &face,
                        &t("warp.inspector.tempo"),
                        bpm,
                        0.1,
                        96.0,
                        true,
                    );
                    if let Some(b) = v {
                        app.set_warp_tempo(r, b.clamp(20.0, 999.0), true);
                    }
                    if ended {
                        app.end_gesture();
                    }
                    if key(ui, "warp.tempo.half", true, 46.0) {
                        app.set_warp_tempo(r, bpm / 2.0, false);
                    }
                    if key(ui, "warp.tempo.double", true, 46.0) {
                        app.set_warp_tempo(r, bpm * 2.0, false);
                    }
                    if key(ui, "warp.menu.detect", true, 140.0) {
                        app.warp_by_analysis(r);
                    }
                    if key(ui, "warp.menu.quantise_short", true, 120.0) {
                        app.open_quantise(vec![r]);
                    }
                    if key(ui, "warp.menu.reset", app.clip_warp(r).is_some(), 130.0) {
                        app.reset_warp(r);
                    }
                    // The editor's own snap grid (the arrangement's is its own).
                    let mut g = GRIDS
                        .iter()
                        .position(|x| *x == app.warp.view.snap())
                        .unwrap_or(2);
                    let labels = [
                        Src::Str("1/4"),
                        Src::Str("1/8"),
                        Src::Str("1/16"),
                        Src::Str("1/32"),
                        Src::Key("warp.grid.off"),
                    ];
                    if kit::widgets::segmented(ui, &mut g, &labels, &t("warp.grid"), 230.0)
                        .changed()
                    {
                        app.warp.view.set_snap(GRIDS[g]);
                    }
                });
            });
    });
}

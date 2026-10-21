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

/// The bar's controls, in the order they are laid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Mode,
    Grid,
    Transpose,
    Formants,
    Follow,
    Tempo,
    Half,
    Double,
    Detect,
    Quantise,
    Reset,
}

impl Item {
    pub const ALL: [Item; 11] = [
        Item::Mode,
        Item::Grid,
        Item::Transpose,
        Item::Formants,
        Item::Follow,
        Item::Tempo,
        Item::Half,
        Item::Double,
        Item::Detect,
        Item::Quantise,
        Item::Reset,
    ];

    /// Whether a label stands before it (one more gap between the two).
    fn labelled(self) -> bool {
        matches!(
            self,
            Item::Mode | Item::Grid | Item::Transpose | Item::Tempo
        )
    }

    /// Its width at 100 %, its label included.
    fn width(self) -> f32 {
        match self {
            Item::Mode => LABEL + 300.0,
            Item::Grid => LABEL + 230.0,
            Item::Transpose => 96.0 + 90.0,
            Item::Formants | Item::Follow => 90.0,
            Item::Tempo => 96.0 + 96.0,
            Item::Half | Item::Double => 46.0,
            Item::Detect => 140.0,
            Item::Quantise => 120.0,
            Item::Reset => 130.0,
        }
    }
}

/// A chip's label before it (*Mode*, *Grid*), at 100 %.
const LABEL: f32 = 64.0;

/// A label before a control, right-aligned in `width` (label and value, ADR 0024).
fn label(ui: &mut egui::Ui, words: &str, width: f32) {
    let s = Style::current();
    let (lr, _) = ui.allocate_exact_size(vec2(s.s(width), s.d(24.0)), Sense::hover());
    let tx = crate::kit::text::Text::new(words, crate::kit::text::Role::Label, s.t.text_dim)
        .align(egui::Align2::RIGHT_CENTER)
        .policy(crate::kit::text::Policy::Elide(
            crate::kit::text::Elide::End,
        ));
    kit::with_brush(ui.painter(), |b| {
        let _ = crate::kit::text::paint(b, lr, &tx, s.tk());
    });
}

/// The items laid in rows of at most `avail` points (as many rows as they need — round
/// 29's second review: one row pushed *Reset* off the screen at an ordinary width).
pub fn rows(avail: f32, gap: f32, k: f32) -> Vec<Vec<Item>> {
    let mut out: Vec<Vec<Item>> = vec![Vec::new()];
    let mut used = 0.0;
    for it in Item::ALL {
        let w = it.width() * k + if it.labelled() { gap } else { 0.0 };
        let row = out.last_mut().expect("a row");
        if !row.is_empty() && used + gap + w > avail {
            out.push(vec![it]);
            used = w;
        } else {
            used += if row.is_empty() { w } else { gap + w };
            row.push(it);
        }
    }
    out
}

pub fn bar(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef) {
    let s = Style::current();
    let Some(w) = app.warp_to_edit(r) else {
        return;
    };
    let pane = ui.available_width();
    let gap = ui.spacing().item_spacing.x;
    let k = s.s(1.0);
    // An item wider than the pane (Mode's chips, Grid's) is reached by scrolling the bar
    // sideways — never clipped out of reach (round 29's third review).
    let widest = Item::ALL
        .iter()
        .map(|it| it.width() * k + if it.labelled() { gap } else { 0.0 })
        .fold(0.0f32, f32::max);
    let avail = pane.max(widest);
    let rows = rows(avail, gap, k);
    let row_h = s.s(super::BAR_H);
    app.warp.view.bar_rects.clear();
    let lay = |app: &mut CantusApp, ui: &mut egui::Ui| {
        ui.allocate_ui(vec2(avail, row_h * rows.len() as f32), |ui| {
            ui.vertical(|ui| {
                for row in &rows {
                    ui.allocate_ui(vec2(avail, row_h), |ui| {
                        ui.horizontal(|ui| {
                            for it in row {
                                let rect = ui.scope(|ui| item(app, ui, r, &w, *it)).response.rect;
                                app.warp.view.bar_rects.push((*it, rect));
                            }
                        });
                    });
                }
            });
        });
    };
    if widest > pane {
        egui::ScrollArea::horizontal()
            .id_salt("warp-bar")
            .show(ui, |ui| lay(app, ui));
    } else {
        lay(app, ui);
    }
}

/// One item of the bar.
fn item(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef, w: &cantus_core::warp::Warp, it: Item) {
    let warped = app.clip_warp(r).is_some_and(|w| w.follow);
    match it {
        Item::Mode => {
            label(ui, &t("warp.inspector.mode"), LABEL);
            let mut m = mode_index(w.mode);
            let labels = MODES.map(Src::Key);
            if kit::widgets::segmented(ui, &mut m, &labels, &t("warp.inspector.mode"), 300.0)
                .changed()
            {
                // The mode alone: it does not warp the clip.
                app.set_warp_mode(r, WarpMode::ALL[m]);
            }
        }
        Item::Grid => {
            // The editor's own snap grid (the arrangement's is its own).
            label(ui, &t("warp.grid"), LABEL);
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
            if kit::widgets::segmented(ui, &mut g, &labels, &t("warp.grid"), 230.0).changed() {
                app.warp.view.set_snap(GRIDS[g]);
            }
        }
        Item::Transpose => {
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
        }
        Item::Formants => {
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
        }
        Item::Follow => {
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
        }
        Item::Tempo | Item::Half | Item::Double => {
            // The source's tempo: what the analysis found while the clip does not follow
            // (round 29's second review), else the tempo where it starts.
            let bpm = app.clip_source_tempo(r).unwrap_or_else(|| {
                let at = app
                    .project()
                    .track(r.0)
                    .and_then(|t| t.clip(r.1))
                    .map_or(0.0, |c| c.start.0 as f64);
                crate::views::bars::map(app.project()).bpm_at(at)
            });
            match it {
                Item::Tempo => {
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
                }
                Item::Half => {
                    if key(ui, "warp.tempo.half", true, 46.0) {
                        app.set_warp_tempo(r, bpm / 2.0, false);
                    }
                }
                _ => {
                    if key(ui, "warp.tempo.double", true, 46.0) {
                        app.set_warp_tempo(r, bpm * 2.0, false);
                    }
                }
            }
        }
        Item::Detect => {
            if key(ui, "warp.menu.detect", true, 140.0) {
                app.warp_by_analysis(r);
            }
        }
        Item::Quantise => {
            if key(ui, "warp.menu.quantise_short", true, 120.0) {
                app.open_quantise(vec![r]);
            }
        }
        Item::Reset => {
            if key(ui, "warp.menu.reset", app.clip_warp(r).is_some(), 130.0) {
                app.reset_warp(r);
            }
        }
    }
}

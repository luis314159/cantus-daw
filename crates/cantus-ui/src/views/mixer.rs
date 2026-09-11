//! Mixer: one channel strip per track plus the master. Faders and pans are the same
//! commands the arrangement headers issue, so both views always agree.

use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::{bar, fader, Bar, Fader};
use cantus_core::{linear_to_db, Command, MergeKey, PanLaw, TrackId};
use egui::{pos2, vec2, Align2, FontId, Rect, RichText, Sense, Stroke, StrokeKind};
use std::time::Instant;

/// Strip geometry, all on the 4-px grid (`design::space`).
const STRIP_W: f32 = space::GRID * 24.0;
const HEADER_H: f32 = space::ROW;
const FADER_H: f32 = space::GRID * 40.0;
const METER_W: f32 = space::S4;
const HOLD_SECONDS: f32 = 1.2;

#[derive(Debug, Default)]
pub struct MixerView {
    /// Peak-hold per strip (track index; last entry is master).
    holds: Vec<(f32, Instant)>,
}

impl MixerView {
    fn hold(&mut self, index: usize, peak: f32) -> f32 {
        while self.holds.len() <= index {
            self.holds.push((0.0, Instant::now()));
        }
        let (held, at) = &mut self.holds[index];
        if peak >= *held || at.elapsed().as_secs_f32() > HOLD_SECONDS {
            *held = peak;
            *at = Instant::now();
        }
        *held
    }
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("mixer.title"))
                .heading()
                .color(theme::p().gold),
        );
        ui.label(
            RichText::new(tf("mixer.tracks", &[("n", &app.project().tracks.len())]))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
    });
    let ids: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for (i, id) in ids.iter().enumerate() {
                strip(app, ui, *id, i);
            }
            ui.add_space(space::S2);
            master_strip(app, ui, ids.len());
        });
    });
}

fn meter(ui: &mut egui::Ui, rect: Rect, peak: f32, hold: f32) {
    let p = ui.painter();
    // A meter is a well cut into the strip, like the fader's groove beside it — the same
    // colour rule, so the two read as one control rather than two unrelated boxes. `ink`
    // was right on a dark theme and a white slab on Paper.
    p.rect_filled(rect, 2.0, theme::recess(&theme::p()));
    let frac = |v: f32| ((linear_to_db(v) + 60.0) / 60.0).clamp(0.0, 1.0);
    let h = rect.height() * frac(peak);
    let bar = Rect::from_min_max(
        pos2(rect.left() + 1.0, rect.bottom() - h),
        pos2(rect.right() - 1.0, rect.bottom()),
    );
    p.rect_filled(bar, 1.0, theme::meter_color(peak));
    let hy = rect.bottom() - rect.height() * frac(hold);
    p.hline(
        rect.x_range(),
        hy,
        Stroke::new(1.5, theme::meter_color(hold)),
    );
    // 0 dB and -12 dB ticks.
    for db in [0.0f32, -12.0, -24.0, -48.0] {
        let y = rect.bottom() - rect.height() * ((db + 60.0) / 60.0);
        p.hline(
            rect.left()..=rect.left() + 3.0,
            y,
            Stroke::new(1.0, theme::p().grid_bar),
        );
    }
    p.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, theme::p().grid_strong),
        StrokeKind::Inside,
    );
}

fn strip(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, index: usize) {
    let Some(track) = app.project().track(id).cloned() else {
        return;
    };
    let selected = app.selected_track == Some(id);
    let color = theme::track_color(track.color);
    let peak = app
        .engine
        .meters()
        .tracks
        .get(index)
        .copied()
        .unwrap_or(0.0);
    let hold = app.mixer.hold(index, peak);

    let frame = egui::Frame::NONE
        .fill(if selected {
            theme::p().panel_raised
        } else {
            theme::p().panel
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                theme::p().gold
            } else {
                theme::p().grid
            },
        ))
        .inner_margin(egui::Margin::same(space::S2 as i8))
        .corner_radius(theme::p().corner_radius);
    frame.show(ui, |ui| {
        ui.set_width(STRIP_W);
        ui.vertical_centered(|ui| {
            // Header: colour bar + name; click selects, hover brightens.
            let (rect, resp) = ui.allocate_exact_size(vec2(STRIP_W, HEADER_H), Sense::click());
            let lift = match (selected, resp.hovered()) {
                (true, _) => 0.9,
                (false, true) => 0.72,
                (false, false) => 0.55,
            };
            ui.painter()
                .rect_filled(rect, theme::p().corner_radius, color.gamma_multiply(lift));
            ui.painter().with_clip_rect(rect).text(
                rect.center(),
                Align2::CENTER_CENTER,
                &track.name,
                FontId::proportional(12.0),
                theme::p().ink,
            );
            if resp.clicked() {
                app.select_track(id);
            }
            ui.add_space(space::S1);
            // Effects count.
            ui.label(
                RichText::new(tf("mixer.effects", &[("n", &track.effects.len())]))
                    .text_style(design::caption())
                    .color(theme::p().text_dim),
            );
            // Pan, width and polarity: the three things a channel strip does to the
            // stereo field, in the order the signal meets them.
            let lane = STRIP_W - space::S4;
            let mut pan = track.pan;
            let pr = ui.add(Bar::pan(&mut pan).size(lane).name(t("track.pan")));
            if pr.changed() {
                app.exec_merging(
                    Command::SetTrackPan { id, pan },
                    MergeKey::of(&("pan", id.0)),
                );
            }
            if pr.drag_stopped() {
                app.end_gesture();
            }
            ui.label(
                RichText::new(bar::pan_text(pan))
                    .text_style(design::mono_small())
                    .color(theme::p().text_dim),
            );
            let mut width = track.width;
            let wr = ui.add(Bar::width(&mut width).size(lane).name(t("track.width")));
            if wr.changed() {
                app.exec_merging(
                    Command::SetTrackWidth { id, width },
                    MergeKey::of(&("width", id.0)),
                );
            }
            if wr.drag_stopped() {
                app.end_gesture();
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(bar::width_text(width))
                        .text_style(design::mono_small())
                        .color(theme::p().text_dim),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    phase_button(ui, track.phase_invert).clicked().then(|| {
                        app.exec(Command::SetTrackPhaseInvert {
                            id,
                            invert: !track.phase_invert,
                        })
                    });
                    // Mono is the width control's end stop, so the button is a shortcut
                    // to it rather than a second, disagreeing switch.
                    let is_mono = track.width <= 0.005;
                    if ui
                        .selectable_label(
                            is_mono,
                            RichText::new(t("track.mono")).text_style(design::caption()),
                        )
                        .on_hover_text(t("track.mono_tip"))
                        .clicked()
                    {
                        app.exec(Command::SetTrackWidth {
                            id,
                            width: if is_mono { 1.0 } else { 0.0 },
                        });
                    }
                });
            });
            ui.add_space(space::S1);
            // Fader + meter side by side.
            ui.horizontal(|ui| {
                let mut gain = track.gain_db;
                // The mixer's scroll area scrolls horizontally, so the wheel is free for
                // the faders (`Fader::wheel`).
                let g = ui.add(
                    Fader::new(&mut gain)
                        .vertical(FADER_H)
                        .wheel(true)
                        .name(&track.name),
                );
                if g.changed() {
                    app.exec_merging(
                        Command::SetTrackGain { id, gain_db: gain },
                        MergeKey::of(&("gain", id.0)),
                    );
                }
                if g.drag_stopped() {
                    app.end_gesture();
                }
                ui.add_space(space::S2);
                let (rect, _) = ui.allocate_exact_size(vec2(METER_W, FADER_H), Sense::hover());
                meter(ui, rect, peak, hold);
            });
            ui.label(
                RichText::new(fader::read_out(track.gain_db)).text_style(design::readout_small()),
            );
            ui.horizontal(|ui| {
                let mut muted = track.muted;
                if ui
                    .selectable_label(
                        muted,
                        RichText::new(t("track.mute")).color(if muted {
                            theme::p().danger
                        } else {
                            theme::p().text_dim
                        }),
                    )
                    .clicked()
                {
                    muted = !muted;
                    app.exec(Command::SetTrackMute { id, muted });
                }
                let mut solo = track.soloed;
                if ui
                    .selectable_label(
                        solo,
                        RichText::new(t("track.solo")).color(if solo {
                            theme::p().gold_bright
                        } else {
                            theme::p().text_dim
                        }),
                    )
                    .clicked()
                {
                    solo = !solo;
                    app.exec(Command::SetTrackSolo { id, soloed: solo });
                }
            });
        });
    });
}

fn master_strip(app: &mut CantusApp, ui: &mut egui::Ui, index: usize) {
    let m = app.engine.meters().master;
    let peak = m[0].max(m[1]);
    let hold = app.mixer.hold(index, peak);
    let frame = egui::Frame::NONE
        .fill(theme::p().panel_raised)
        .stroke(Stroke::new(1.0, theme::p().gold.gamma_multiply(0.6)))
        .inner_margin(egui::Margin::same(space::S2 as i8))
        .corner_radius(theme::p().corner_radius);
    frame.show(ui, |ui| {
        ui.set_width(STRIP_W);
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(STRIP_W, HEADER_H), Sense::hover());
            ui.painter()
                .rect_filled(rect, theme::p().corner_radius, theme::p().gold);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                t("mixer.master"),
                FontId::proportional(12.0),
                theme::p().ink,
            );
            ui.add_space(space::S1);
            ui.label(
                RichText::new(t("mixer.stereo"))
                    .text_style(design::caption())
                    .color(theme::p().text_dim),
            );
            // The master is a channel strip too: a mix can be off-centre, too wide, or
            // out of polarity against a reference. Its pan is a *balance* — the pan law
            // does not apply to it, because the master is already stereo (ADR 0012).
            let lane = STRIP_W - space::S4;
            let mut mpan = app.project().master.pan;
            let pr = ui.add(Bar::pan(&mut mpan).size(lane).name(t("mixer.master")));
            if pr.changed() {
                app.exec_merging(
                    Command::SetMasterPan { pan: mpan },
                    MergeKey::of(&"master-pan"),
                );
            }
            if pr.drag_stopped() {
                app.end_gesture();
            }
            ui.label(
                RichText::new(bar::pan_text(mpan))
                    .text_style(design::mono_small())
                    .color(theme::p().text_dim),
            );
            let mut mwidth = app.project().master.width;
            let wr = ui.add(Bar::width(&mut mwidth).size(lane).name(t("track.width")));
            if wr.changed() {
                app.exec_merging(
                    Command::SetMasterWidth { width: mwidth },
                    MergeKey::of(&"master-width"),
                );
            }
            if wr.drag_stopped() {
                app.end_gesture();
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(bar::width_text(mwidth))
                        .text_style(design::mono_small())
                        .color(theme::p().text_dim),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let inverted = app.project().master.phase_invert;
                    if phase_button(ui, inverted).clicked() {
                        app.exec(Command::SetMasterPhaseInvert { invert: !inverted });
                    }
                });
            });
            ui.add_space(space::S1);
            ui.horizontal(|ui| {
                let mut gain = app.project().master.gain_db;
                let g = ui.add(
                    Fader::new(&mut gain)
                        .vertical(FADER_H)
                        .wheel(true)
                        .name(t("mixer.master")),
                );
                if g.changed() {
                    app.exec_merging(
                        Command::SetMasterGain { gain_db: gain },
                        MergeKey::of(&"master"),
                    );
                }
                if g.drag_stopped() {
                    app.end_gesture();
                }
                ui.add_space(space::S2);
                let (l, _) = ui.allocate_exact_size(vec2(space::S2, FADER_H), Sense::hover());
                meter(ui, l, m[0], hold);
                let (r, _) = ui.allocate_exact_size(vec2(space::S2, FADER_H), Sense::hover());
                meter(ui, r, m[1], hold);
            });
            ui.label(
                RichText::new(fader::read_out(app.project().master.gain_db))
                    .text_style(design::readout_small()),
            );
            pan_law_picker(app, ui);
            let peak_db = linear_to_db(hold);
            ui.label(
                RichText::new(if hold <= 0.0 {
                    "-\u{221e}".to_string()
                } else {
                    format!("{peak_db:.1}")
                })
                .text_style(design::mono_small())
                .color(if hold >= 1.0 {
                    theme::p().danger
                } else {
                    theme::p().text_dim
                }),
            );
        });
    });
}

/// The polarity switch. A Ø that is *on* is a state, so it gets the accent fill — the one
/// thing the design system reserves it for.
pub fn phase_button(ui: &mut egui::Ui, inverted: bool) -> egui::Response {
    ui.selectable_label(
        inverted,
        RichText::new(t("track.phase"))
            .text_style(design::caption())
            .color(if inverted {
                theme::p().gold_bright
            } else {
                theme::p().text_dim
            }),
    )
    .on_hover_text(t("track.phase_tip"))
}

/// The pan law lives on the master strip because that is where somebody mixing looks for
/// it, and it is a *project* setting rather than a preference: it decides what the file
/// renders as (ADR 0012), so it travels with the file.
fn pan_law_picker(app: &mut CantusApp, ui: &mut egui::Ui) {
    let current = app.project().pan_law;
    ui.label(
        RichText::new(t("mixer.pan_law").to_uppercase())
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
    let mut chosen = None;
    egui::ComboBox::from_id_salt("pan-law")
        .width(STRIP_W - space::S4)
        .selected_text(RichText::new(current.id()).text_style(design::mono_small()))
        .show_ui(ui, |ui| {
            for law in PanLaw::ALL {
                if ui
                    .selectable_label(law == current, t(law.label_key()))
                    .clicked()
                    && law != current
                {
                    chosen = Some(law);
                }
            }
        })
        .response
        .on_hover_text(t("panlaw.tip"));
    if let Some(law) = chosen {
        app.exec(Command::SetPanLaw { law });
    }
}

//! Inspector: master gain, selected track settings and the Firmus parameters.

use crate::app::CantusApp;
use crate::design::{self, space, Elevation};
use crate::i18n::t;
use crate::theme;
use crate::widgets::{fader, Fader};
use cantus_core::{
    Color, Command, EffectKind, EffectSlot, FirmusParam, MergeKey, TrackId, Waveform,
};
use egui::RichText;

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(space::S1);
        heading(ui, &t("inspector.master"));
        let mut master = app.project().master.gain_db;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t("track.gain"))
                    .text_style(design::caption())
                    .color(theme::p().text_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(fader::read_out(master)).text_style(design::readout_small()),
                );
            });
        });
        // The inspector scrolls vertically, so this one does not take the wheel.
        let r = ui.add(
            Fader::new(&mut master)
                .horizontal(ui.available_width() - space::S2)
                .name(t("inspector.master")),
        );
        if r.changed() {
            app.exec_merging(
                Command::SetMasterGain { gain_db: master },
                MergeKey::of(&"master"),
            );
        }
        if r.drag_stopped() {
            app.end_gesture();
        }
        ui.separator();

        let Some(id) = app.selected_track else {
            ui.label(RichText::new(t("track.no_selection")).color(theme::p().text_dim));
            return;
        };
        let Some(track) = app.project().track(id).cloned() else {
            return;
        };

        ui.add_space(space::S1);
        ui.label(RichText::new(&track.name).heading());
        ui.horizontal(|ui| {
            ui.label(t("track.color"));
            for c in Color::PALETTE {
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(space::S4, space::S4), egui::Sense::click());
                let selected = c == track.color;
                let pal = theme::p();
                ui.painter()
                    .rect_filled(rect, pal.corner_radius, theme::track_color(c));
                // Selected reads as a ring in the text colour; hover only lifts the edge,
                // so the two states never look alike.
                let edge = if selected {
                    Some(egui::Stroke::new(2.0, pal.text))
                } else if resp.hovered() {
                    Some(egui::Stroke::new(1.0, pal.text_dim))
                } else {
                    None
                };
                if let Some(stroke) = edge {
                    ui.painter().rect_stroke(
                        rect,
                        pal.corner_radius,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }
                if resp.clicked() {
                    app.exec(Command::SetTrackColor { id, color: c });
                }
            }
        });
        ui.separator();
        heading(
            ui,
            &format!(
                "{} · {}",
                t("inspector.title"),
                t(track.instrument.name_key())
            ),
        );
        let Some(params) = track.instrument.firmus().cloned() else {
            return;
        };

        section(ui, "inspector.oscillators");
        wave_combo(app, ui, id, FirmusParam::Osc1Wave, params.osc1_wave);
        wave_combo(app, ui, id, FirmusParam::Osc2Wave, params.osc2_wave);
        for p in [
            FirmusParam::Osc2Semitones,
            FirmusParam::Osc2Detune,
            FirmusParam::OscMix,
        ] {
            param_slider(app, ui, id, p, p.get(&params));
        }
        section(ui, "inspector.filter");
        for p in [
            FirmusParam::Cutoff,
            FirmusParam::Resonance,
            FirmusParam::FilterEnvAmount,
            FirmusParam::KeyTrack,
        ] {
            param_slider(app, ui, id, p, p.get(&params));
        }
        section(ui, "inspector.amp_env");
        for p in [
            FirmusParam::AmpAttack,
            FirmusParam::AmpDecay,
            FirmusParam::AmpSustain,
            FirmusParam::AmpRelease,
        ] {
            param_slider(app, ui, id, p, p.get(&params));
        }
        section(ui, "inspector.filter_env");
        for p in [
            FirmusParam::FilterAttack,
            FirmusParam::FilterDecay,
            FirmusParam::FilterSustain,
            FirmusParam::FilterRelease,
        ] {
            param_slider(app, ui, id, p, p.get(&params));
        }
        section(ui, "inspector.output");
        param_slider(app, ui, id, FirmusParam::Gain, params.gain_db);
        ui.add_space(space::S3);
        ui.separator();
        effects(app, ui, id, &track.effects);
        ui.add_space(space::S2);
    });
}

/// A panel heading: the accent colour carries the hierarchy, the semibold face carries
/// the weight, so neither has to shout.
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).heading().color(theme::p().gold));
}

/// A group label inside a panel — one step quieter than a heading.
fn section(ui: &mut egui::Ui, key: &str) {
    ui.add_space(space::S2);
    ui.label(
        RichText::new(t(key).to_uppercase())
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
}

fn wave_combo(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    param: FirmusParam,
    current: Waveform,
) {
    ui.horizontal(|ui| {
        ui.label(t(param.info().label_key));
        egui::ComboBox::from_id_salt(("wave", id.0, param as u8))
            .selected_text(t(current.label_key()))
            .show_ui(ui, |ui| {
                for w in Waveform::ALL {
                    if ui
                        .selectable_label(w == current, t(w.label_key()))
                        .clicked()
                        && w != current
                    {
                        app.exec(Command::SetInstrumentParam {
                            track: id,
                            param,
                            value: w.index() as f32,
                        });
                    }
                }
            });
    });
}

fn param_slider(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    param: FirmusParam,
    current: f32,
) {
    let info = param.info();
    let mut v = current;
    let mut slider = egui::Slider::new(&mut v, info.min..=info.max)
        .text(t(info.label_key))
        .logarithmic(info.logarithmic);
    if !info.unit.is_empty() {
        slider = slider.suffix(format!(" {}", info.unit));
    }
    if info.stepped {
        slider = slider.step_by(1.0);
    }
    let r = ui.add(slider);
    if r.changed() {
        app.exec_merging(
            Command::SetInstrumentParam {
                track: id,
                param,
                value: v,
            },
            MergeKey::of(&("param", id.0, param as u8)),
        );
    }
    if r.drag_stopped() || r.lost_focus() {
        app.end_gesture();
    }
    if r.double_clicked() {
        app.exec(Command::SetInstrumentParam {
            track: id,
            param,
            value: info.default,
        });
    }
}

fn effects(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, slots: &[EffectSlot]) {
    ui.horizontal(|ui| {
        heading(ui, &t("inspector.effects"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button(t("effect.add"), |ui| {
                for kind in EffectKind::ALL {
                    if ui.button(t(kind.name_key())).clicked() {
                        ui.close();
                        app.add_effect(id, kind);
                    }
                }
            });
        });
    });
    let n = slots.len();
    for (i, slot) in slots.iter().enumerate() {
        Elevation::Card.frame(&theme::p()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut on = !slot.bypass;
                if ui
                    .checkbox(&mut on, "")
                    .on_hover_text(t("effect.bypass"))
                    .changed()
                {
                    app.exec(Command::SetEffectBypass {
                        track: id,
                        effect: slot.id,
                        bypass: !on,
                    });
                }
                ui.label(RichText::new(t(slot.spec.kind.name_key())).text_style(design::strong()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(t("effect.remove_short"))
                        .on_hover_text(t("effect.remove"))
                        .clicked()
                    {
                        app.remove_effect(id, slot.id);
                    }
                    if ui
                        .add_enabled(i + 1 < n, egui::Button::new(t("effect.down_short")).small())
                        .on_hover_text(t("effect.down"))
                        .clicked()
                    {
                        app.exec(Command::MoveEffect {
                            track: id,
                            effect: slot.id,
                            index: i + 1,
                        });
                    }
                    if ui
                        .add_enabled(i > 0, egui::Button::new(t("effect.up_short")).small())
                        .on_hover_text(t("effect.up"))
                        .clicked()
                    {
                        app.exec(Command::MoveEffect {
                            track: id,
                            effect: slot.id,
                            index: i - 1,
                        });
                    }
                });
            });
            for info in slot.spec.kind.params() {
                let mut v = slot.spec.get(info.key);
                let mut slider = egui::Slider::new(&mut v, info.min..=info.max)
                    .text(t(info.label_key))
                    .logarithmic(info.logarithmic);
                if !info.unit.is_empty() {
                    slider = slider.suffix(format!(" {}", info.unit));
                }
                if info.stepped {
                    slider = slider.step_by(1.0);
                }
                let r = ui.add(slider);
                if r.changed() {
                    app.exec_merging(
                        Command::SetEffectParam {
                            track: id,
                            effect: slot.id,
                            key: info.key.to_string(),
                            value: v,
                        },
                        MergeKey::of(&("fx", slot.id.0, info.key)),
                    );
                }
                if r.drag_stopped() || r.lost_focus() {
                    app.end_gesture();
                }
                if r.double_clicked() {
                    app.exec(Command::SetEffectParam {
                        track: id,
                        effect: slot.id,
                        key: info.key.to_string(),
                        value: info.default,
                    });
                }
            }
        });
    }
    if slots.is_empty() {
        ui.label(
            RichText::new(t("effect.none"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
    }
}

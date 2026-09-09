//! Inspector: master gain, selected track settings and the Firmus parameters.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::theme;
use cantus_core::{
    Color, Command, EffectKind, EffectSlot, FirmusParam, MergeKey, TrackId, Waveform,
};
use egui::RichText;

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(4.0);
        ui.label(
            RichText::new(t("inspector.master"))
                .strong()
                .color(theme::p().gold),
        );
        let mut master = app.project().master.gain_db;
        let r = ui.add(
            egui::Slider::new(&mut master, -60.0..=12.0)
                .suffix(" dB")
                .text(t("track.gain")),
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

        ui.label(RichText::new(&track.name).strong().size(15.0));
        ui.horizontal(|ui| {
            ui.label(t("track.color"));
            for c in Color::PALETTE {
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                let selected = c == track.color;
                ui.painter().rect_filled(rect, 3.0, theme::track_color(c));
                if selected {
                    ui.painter().rect_stroke(
                        rect,
                        3.0,
                        egui::Stroke::new(2.0, theme::p().text),
                        egui::StrokeKind::Inside,
                    );
                }
                if resp.clicked() {
                    app.exec(Command::SetTrackColor { id, color: c });
                }
            }
        });
        ui.separator();
        ui.label(
            RichText::new(format!(
                "{} · {}",
                t("inspector.title"),
                t(track.instrument.name_key())
            ))
            .strong()
            .color(theme::p().gold),
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
        ui.add_space(10.0);
        ui.separator();
        effects(app, ui, id, &track.effects);
        ui.add_space(8.0);
    });
}

fn section(ui: &mut egui::Ui, key: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(t(key)).small().color(theme::p().text_dim));
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
        ui.label(
            RichText::new(t("inspector.effects"))
                .strong()
                .color(theme::p().gold),
        );
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
        let frame = egui::Frame::NONE
            .fill(theme::p().panel_raised)
            .inner_margin(egui::Margin::same(6))
            .corner_radius(4);
        frame.show(ui, |ui| {
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
                ui.label(RichText::new(t(slot.spec.kind.name_key())).strong());
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
        ui.add_space(4.0);
    }
    if slots.is_empty() {
        ui.label(
            RichText::new(t("effect.none"))
                .small()
                .color(theme::p().text_dim),
        );
    }
}

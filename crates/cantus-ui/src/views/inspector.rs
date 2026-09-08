//! Inspector: master gain, selected track settings and the Firmus parameters.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::theme;
use cantus_core::{Color, Command, FirmusParam, MergeKey, TrackId, Waveform};
use egui::RichText;

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(4.0);
        ui.label(
            RichText::new(t("inspector.master"))
                .strong()
                .color(theme::GOLD),
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
            ui.label(RichText::new(t("track.no_selection")).color(theme::TEXT_DIM));
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
                        egui::Stroke::new(2.0, theme::TEXT),
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
            .color(theme::GOLD),
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
        ui.add_space(8.0);
    });
}

fn section(ui: &mut egui::Ui, key: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(t(key)).small().color(theme::TEXT_DIM));
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

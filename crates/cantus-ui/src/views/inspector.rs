//! Inspector: master gain, selected track settings and the instrument's parameters.

use crate::app::CantusApp;
use crate::design::{self, space, Elevation};
use crate::i18n::t;
use crate::theme;
use crate::widgets::{bar, fader, Bar, Fader};
use cantus_core::{
    Color, Command, DrumPad, EffectKind, EffectSlot, FirmusParam, InstrumentParam, MergeKey,
    PadField, PadVoice, TrackId, Waveform,
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
        channel(app, ui, id, &track);
        ui.separator();
        ui.horizontal(|ui| {
            heading(ui, &t("inspector.title"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                instrument_combo(app, ui, id, &track.instrument);
            });
        });
        if let Some(kit) = track.instrument.pulsus().cloned() {
            pulsus(app, ui, id, &kit.pads);
            ui.add_space(space::S3);
            ui.separator();
            effects(app, ui, id, &track.effects);
            ui.add_space(space::S2);
            return;
        }
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

/// Which instrument the track hosts. Switching keeps the clips: a note is a pitch on
/// either, and the lanes follow the instrument.
fn instrument_combo(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    current: &cantus_core::InstrumentSpec,
) {
    use cantus_core::{FirmusParams, InstrumentSpec, PulsusParams};
    let is_kit = current.pulsus().is_some();
    egui::ComboBox::from_id_salt(("instrument", id.0))
        .selected_text(t(current.name_key()))
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(!is_kit, t("instrument.firmus"))
                .clicked()
                && is_kit
            {
                app.set_instrument(id, InstrumentSpec::Firmus(FirmusParams::default()));
            }
            if ui
                .selectable_label(is_kit, t("instrument.pulsus"))
                .clicked()
                && !is_kit
            {
                app.set_instrument(id, InstrumentSpec::Pulsus(PulsusParams::default()));
            }
        });
}

/// The kit: a row of pad buttons, then the parameters of the one that is selected.
///
/// One pad at a time rather than all of them. Seven pads with nine parameters each is
/// sixty-three sliders, which is not an inspector, it is a wall — and the pad you are
/// working on is the one the channel rack already knows about, so the two panels agree
/// without either of them having to be told.
fn pulsus(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, pads: &[DrumPad]) {
    section(ui, "inspector.pads");
    ui.horizontal_wrapped(|ui| {
        let selected = app.rack.selected_pad;
        for (i, pad) in pads.iter().enumerate() {
            let is = i == selected;
            let mut text = RichText::new(&pad.name).text_style(design::caption());
            if pad.muted {
                // A muted pad is dimmed rather than hidden: it is still in the kit, and
                // hunting for a pad that vanished when you muted it is its own bug report.
                text = text.color(theme::p().text_dim);
            }
            if ui.selectable_label(is, text).clicked() {
                app.rack.selected_pad = i;
            }
        }
        // Add a pad of each shape, or a sample. This is how a kit becomes the musician's
        // rather than ours.
        ui.menu_button(t("pad.add"), |ui| {
            for voice in PadVoice::ALL {
                if ui.button(t(voice.label_key())).clicked() {
                    ui.close();
                    app.add_pad(id, voice);
                }
            }
        });
    });
    if pads.is_empty() {
        ui.label(
            RichText::new(t("pad.none_yet"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        return;
    }
    let selected = app.rack.selected_pad.min(pads.len() - 1);
    app.rack.selected_pad = selected;
    let pad = &pads[selected];
    ui.add_space(space::S1);
    heading(ui, &format!("{} · {}", pad.name, t(pad.voice.label_key())));
    ui.horizontal(|ui| {
        section(ui, "inspector.pad_sound");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button(t("pad.remove"))
                .on_hover_text(t("pad.remove_tip"))
                .clicked()
            {
                app.remove_pad(id, selected);
            }
        });
    });
    // Only the fields this voice actually has: a hi-hat has no pitch envelope, and a
    // slider that does nothing is worse than one that is missing.
    for field in PadField::for_voice(pad.voice) {
        match field {
            PadField::Voice => {
                voice_combo(app, ui, id, selected, pad.voice);
                if pad.voice == PadVoice::Sample {
                    sample_row(app, ui, id, selected, pad);
                }
            }
            PadField::Mute => {
                let mut muted = pad.muted;
                if ui.checkbox(&mut muted, t("pad.mute")).changed() {
                    set_pad(app, id, selected, field, muted as u8 as f32, false);
                }
            }
            PadField::Choke => choke_combo(app, ui, id, selected, pad.choke),
            _ => pad_slider(app, ui, id, selected, pad, field),
        }
    }
}

/// Which voice the pad is. Switching to *Sample* keeps whatever file the pad already had.
fn voice_combo(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, pad: usize, current: PadVoice) {
    ui.horizontal(|ui| {
        ui.label(t("pad.voice"));
        egui::ComboBox::from_id_salt(("voice", id.0, pad))
            .selected_text(t(current.label_key()))
            .show_ui(ui, |ui| {
                for v in PadVoice::ALL {
                    if ui
                        .selectable_label(v == current, t(v.label_key()))
                        .clicked()
                        && v != current
                    {
                        set_pad(app, id, pad, PadField::Voice, v.index() as f32, false);
                    }
                }
            });
    });
}

/// The sample a pad plays: its file name, whether it loaded, and a button to choose one.
///
/// The status comes from the engine's cache, so what is shown is what will *sound*: a path
/// the project remembers but the disk no longer has says so here, instead of a pad that is
/// mysteriously quiet.
fn sample_row(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, pad: usize, drum: &DrumPad) {
    use cantus_engine::SampleStatus;
    ui.horizontal(|ui| {
        ui.label(t("pad.sample"));
        let pal = theme::p();
        match drum.sample.as_deref() {
            None => {
                ui.label(RichText::new(t("pad.sample_none")).color(pal.text_dim));
            }
            Some(path) => {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string());
                match app.engine.samples.status(std::path::Path::new(path)) {
                    SampleStatus::Loaded {
                        rate,
                        channels,
                        frames,
                    } => {
                        let secs = frames as f32 / rate.max(1) as f32;
                        ui.label(RichText::new(name).text_style(design::strong()))
                            .on_hover_text(path);
                        ui.label(
                            RichText::new(format!(
                                "{secs:.2} s · {rate} Hz · {}",
                                if channels >= 2 { "stereo" } else { "mono" }
                            ))
                            .text_style(design::mono_small())
                            .color(pal.text_dim),
                        );
                    }
                    SampleStatus::Failed(err) => {
                        ui.label(RichText::new(name).color(pal.danger))
                            .on_hover_text(format!("{}\n{err}", t("pad.sample_missing")));
                    }
                    SampleStatus::Unknown => {
                        ui.label(RichText::new(name).color(pal.text_dim));
                    }
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(t("pad.sample_load")).clicked() {
                app.load_pad_sample(id, pad);
            }
        });
    });
}

/// The choke group, as a named list rather than a number line.
///
/// It is an *identity*, not a quantity: group 2 is not twice group 1, and a slider would
/// invite dragging through the groups looking for the right one.
fn choke_combo(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, pad: usize, current: u8) {
    ui.horizontal(|ui| {
        ui.label(t("pad.choke"));
        let label = if current == 0 {
            t("pad.none")
        } else {
            crate::i18n::tf("pad.group", &[("n", &current)])
        };
        egui::ComboBox::from_id_salt(("choke", id.0, pad))
            .selected_text(label)
            .show_ui(ui, |ui| {
                for g in 0u8..=4 {
                    let label = if g == 0 {
                        t("pad.none")
                    } else {
                        crate::i18n::tf("pad.group", &[("n", &g)])
                    };
                    if ui.selectable_label(g == current, label).clicked() && g != current {
                        set_pad(app, id, pad, PadField::Choke, g as f32, false);
                    }
                }
            });
    });
}

fn pad_slider(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    pad_index: usize,
    pad: &DrumPad,
    field: PadField,
) {
    let info = field.info(pad.voice);
    let mut v = field.get(pad);
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
        set_pad(app, id, pad_index, field, v, true);
    }
    if r.drag_stopped() || r.lost_focus() {
        app.end_gesture();
    }
    if r.double_clicked() {
        set_pad(app, id, pad_index, field, info.default, false);
    }
}

fn set_pad(
    app: &mut CantusApp,
    id: TrackId,
    pad: usize,
    field: PadField,
    value: f32,
    merging: bool,
) {
    let cmd = Command::SetInstrumentParam {
        track: id,
        param: InstrumentParam::Pulsus {
            pad: pad as u8,
            field,
        },
        value,
    };
    if merging {
        app.exec_merging(cmd, MergeKey::of(&("pad", id.0, pad, field)));
    } else {
        app.exec(cmd);
    }
}

/// The selected track's channel strip: gain, pan, width and polarity.
///
/// This is what left the arrangement's track header. A header is for *which track*; a
/// strip is for *how loud and where*, and it needs room — a scale beside the fader, a
/// readout under the pan — that a 40-px row does not have and a panel does. It is the same
/// set of controls the mixer draws, laid out for one track at a time.
fn channel(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, track: &cantus_core::Track) {
    heading(ui, &t("inspector.channel"));
    let lane = ui.available_width() - space::S2;
    let mut gain = track.gain_db;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("track.gain"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(fader::read_out(gain)).text_style(design::readout_small()));
        });
    });
    let g = ui.add(Fader::new(&mut gain).horizontal(lane).name(&track.name));
    if g.changed() {
        app.exec_merging(
            Command::SetTrackGain { id, gain_db: gain },
            MergeKey::of(&("gain", id.0)),
        );
    }
    if g.drag_stopped() {
        app.end_gesture();
    }
    ui.add_space(space::S1);
    let mut pan = track.pan;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("track.pan"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(bar::pan_text(pan)).text_style(design::readout_small()));
        });
    });
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
    ui.add_space(space::S1);
    let mut width = track.width;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("track.width"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(bar::width_text(width)).text_style(design::readout_small()));
        });
    });
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
        if crate::views::mixer::phase_button(ui, track.phase_invert).clicked() {
            app.exec(Command::SetTrackPhaseInvert {
                id,
                invert: !track.phase_invert,
            });
        }
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
                            param: param.into(),
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
                param: param.into(),
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
            param: param.into(),
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

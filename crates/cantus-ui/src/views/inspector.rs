//! Inspector: master gain, selected track settings and the instrument's parameters.

use crate::app::CantusApp;
use crate::design::{self, space, Elevation};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::{bar, fader, Bar, Fader, Knob};
use cantus_core::{
    Color, Command, DrumPad, EffectKind, EffectSlot, EqField, FirmusParam, InstrumentParam,
    MergeKey, MixerStrip, PadField, PadVoice, Send, StripEq, StripId, TrackId, Waveform,
    EQ_MAX_GAIN_DB, EQ_MAX_HZ, EQ_MAX_Q, EQ_MIN_HZ, EQ_MIN_Q, MAX_INSERTS,
};
use egui::{pos2, vec2, Align2, RichText, Sense, Stroke, StrokeKind};

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
            strip_section(app, ui);
            ui.add_space(space::S2);
            return;
        }
        let Some(params) = track.instrument.firmus().cloned() else {
            return;
        };

        section(ui, "inspector.oscillators");
        wave_combo(app, ui, id, FirmusParam::Osc1Wave, params.osc1_wave);
        wave_combo(app, ui, id, FirmusParam::Osc2Wave, params.osc2_wave);
        knob_row(ui, |ui| {
            for p in [
                FirmusParam::Osc2Semitones,
                FirmusParam::Osc2Detune,
                FirmusParam::OscMix,
            ] {
                param_slider(app, ui, id, p, p.get(&params));
            }
        });
        section(ui, "inspector.filter");
        knob_row(ui, |ui| {
            for p in [
                FirmusParam::Cutoff,
                FirmusParam::Resonance,
                FirmusParam::FilterEnvAmount,
                FirmusParam::KeyTrack,
            ] {
                param_slider(app, ui, id, p, p.get(&params));
            }
        });
        section(ui, "inspector.amp_env");
        knob_row(ui, |ui| {
            for p in [
                FirmusParam::AmpAttack,
                FirmusParam::AmpDecay,
                FirmusParam::AmpSustain,
                FirmusParam::AmpRelease,
            ] {
                param_slider(app, ui, id, p, p.get(&params));
            }
        });
        section(ui, "inspector.filter_env");
        knob_row(ui, |ui| {
            for p in [
                FirmusParam::FilterAttack,
                FirmusParam::FilterDecay,
                FirmusParam::FilterSustain,
                FirmusParam::FilterRelease,
            ] {
                param_slider(app, ui, id, p, p.get(&params));
            }
        });
        section(ui, "inspector.output");
        knob_row(ui, |ui| {
            param_slider(app, ui, id, FirmusParam::Gain, params.gain_db)
        });
        ui.add_space(space::S3);
        ui.separator();
        strip_section(app, ui);
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
    let fields: Vec<PadField> = PadField::for_voice(pad.voice).collect();
    for field in fields.iter().copied() {
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
            _ => {}
        }
    }
    knob_row(ui, |ui| {
        for field in fields.iter().copied() {
            if !matches!(field, PadField::Voice | PadField::Mute | PadField::Choke) {
                pad_slider(app, ui, id, selected, pad, field);
            }
        }
    });
}

/// A row of knobs that wraps: the house layout for a group of parameters.
fn knob_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = space::S1;
        add(ui);
    });
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
    let r = ui.add(
        Knob::new(&mut v, info.min..=info.max, info.default)
            .log(info.logarithmic)
            .stepped(info.stepped)
            .unit(info.unit)
            .label(t(info.label_key)),
    );
    if r.changed() {
        set_pad(app, id, pad_index, field, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
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

/// The track's own channel: its level and balance before the mixer, and which strip it
/// feeds. FL's channel volume and pan, and its mixer-track picker, in one place.
///
/// The fader with a scale, the width and the polarity are the *strip's*, further down:
/// they belong to the channel of the mixer, not to whichever instrument plays into it.
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
    ui.horizontal(|ui| {
        ui.label(t("track.route"));
        route_picker(app, ui, id, track.strip);
    });
}

/// Which strip a track feeds. The list is every strip by number, with its name when it
/// has one, and the master.
pub fn route_picker(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, current: Option<StripId>) {
    let label = match current.and_then(|s| app.project().strip_number(s)) {
        Some(n) => tf("track.route_strip", &[("n", &n)]),
        None => t("mixer.master"),
    };
    let mut chosen: Option<Option<StripId>> = None;
    egui::ComboBox::from_id_salt(("route", id.0))
        .selected_text(label)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), t("mixer.master"))
                .clicked()
            {
                chosen = Some(None);
            }
            // FL's `TRACK` button: the first strip nothing feeds, so a channel gets a
            // strip of its own without anyone counting.
            if let Some(free) = app.project().first_free_strip() {
                if ui.selectable_label(false, t("track.route_free")).clicked() {
                    chosen = Some(Some(free));
                }
            }
            let strips: Vec<(StripId, usize, String)> = app
                .project()
                .strips
                .iter()
                .enumerate()
                .map(|(i, s)| (s.id, i + 1, s.name.clone()))
                .collect();
            for (sid, n, name) in strips {
                let text = if name.is_empty() {
                    tf("track.route_strip", &[("n", &n)])
                } else {
                    format!("{n} · {name}")
                };
                if ui.selectable_label(current == Some(sid), text).clicked() {
                    chosen = Some(Some(sid));
                }
            }
        });
    if let Some(strip) = chosen {
        if strip != current {
            app.route_track(id, strip);
        }
    }
}

/// The mixer strip the inspector is about: its fader, pan, width, polarity, output and
/// inserts — the same controls as the mixer's, for one strip at a time with room to
/// breathe.
fn strip_section(app: &mut CantusApp, ui: &mut egui::Ui) {
    let Some(id) = app.inspected_strip() else {
        heading(ui, &t("inspector.strip"));
        ui.label(
            RichText::new(t("inspector.strip_master"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        return;
    };
    let Some(strip) = app.project().strip(id).cloned() else {
        return;
    };
    let number = app.project().strip_number(id).unwrap_or(0);
    let title = if strip.name.is_empty() {
        tf("inspector.strip_n", &[("n", &number)])
    } else {
        format!(
            "{} · {}",
            tf("inspector.strip_n", &[("n", &number)]),
            strip.name
        )
    };
    heading(ui, &title);
    let lane = ui.available_width() - space::S2;
    let mut gain = strip.gain_db;
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
    let g = ui.add(Fader::new(&mut gain).horizontal(lane).name(&title));
    if g.changed() {
        app.exec_merging(
            Command::SetStripGain { id, gain_db: gain },
            MergeKey::of(&("sgain", id.0)),
        );
    }
    if g.drag_stopped() {
        app.end_gesture();
    }
    ui.add_space(space::S1);
    let mut pan = strip.pan;
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
            Command::SetStripPan { id, pan },
            MergeKey::of(&("spans", id.0)),
        );
    }
    if pr.drag_stopped() {
        app.end_gesture();
    }
    ui.add_space(space::S1);
    let mut width = strip.width;
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
            Command::SetStripWidth { id, width },
            MergeKey::of(&("swidth", id.0)),
        );
    }
    if wr.drag_stopped() {
        app.end_gesture();
    }
    ui.horizontal(|ui| {
        if crate::views::mixer::phase_button(ui, strip.phase_invert).clicked() {
            app.exec(Command::SetStripPhaseInvert {
                id,
                invert: !strip.phase_invert,
            });
        }
        let is_mono = strip.width <= 0.005;
        if ui
            .selectable_label(
                is_mono,
                RichText::new(t("track.mono")).text_style(design::caption()),
            )
            .on_hover_text(t("track.mono_tip"))
            .clicked()
        {
            app.exec(Command::SetStripWidth {
                id,
                width: if is_mono { 1.0 } else { 0.0 },
            });
        }
        let muted = strip.muted;
        if ui
            .selectable_label(
                muted,
                RichText::new(t("track.mute"))
                    .text_style(design::caption())
                    .color(if muted {
                        theme::p().danger
                    } else {
                        theme::p().text_dim
                    }),
            )
            .on_hover_text(t("mixer.mute_strip"))
            .clicked()
        {
            app.exec(Command::SetStripMute { id, muted: !muted });
        }
        if ui
            .selectable_label(
                strip.soloed,
                RichText::new(t("track.solo"))
                    .text_style(design::caption())
                    .color(if strip.soloed {
                        theme::p().gold_bright
                    } else {
                        theme::p().text_dim
                    }),
            )
            .on_hover_text(t("mixer.solo_strip"))
            .clicked()
        {
            app.toggle_strip_solo(id);
        }
        if ui
            .selectable_label(
                strip.swap_channels,
                RichText::new(t("mixer.swap"))
                    .text_style(design::caption())
                    .color(if strip.swap_channels {
                        theme::p().gold_bright
                    } else {
                        theme::p().text_dim
                    }),
            )
            .on_hover_text(t("mixer.swap_tip"))
            .clicked()
        {
            app.exec(Command::SetStripSwapChannels {
                id,
                swap: !strip.swap_channels,
            });
        }
    });
    eq_section(app, ui, &strip);
    sends_section(app, ui, &strip);
    ui.add_space(space::S2);
    effects(app, ui, &strip);
}

/// The strip's three-band EQ: the response drawn, then each band's frequency, gain and
/// Q. The curve is what the ear will meet; the numbers are how it is set exactly.
fn eq_section(app: &mut CantusApp, ui: &mut egui::Ui, strip: &MixerStrip) {
    let id = strip.id;
    ui.horizontal(|ui| {
        section(ui, "inspector.eq");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !strip.eq.is_flat() && ui.small_button(t("inspector.eq_reset")).clicked() {
                let commands = (0..3u8)
                    .map(|band| Command::SetStripEq {
                        id,
                        band,
                        field: EqField::Gain,
                        value: 0.0,
                    })
                    .collect();
                app.exec(Command::Batch {
                    label_key: "cmd.set_strip_eq".into(),
                    commands,
                });
            }
        });
    });
    eq_curve(ui, &strip.eq);
    let names = [
        "inspector.eq_band_low",
        "inspector.eq_band_mid",
        "inspector.eq_band_high",
    ];
    for (b, band) in strip.eq.bands.iter().enumerate() {
        ui.label(
            RichText::new(t(names[b]))
                .text_style(design::caption())
                .color(if band.gain_db != 0.0 {
                    theme::p().gold_bright
                } else {
                    theme::p().text_dim
                }),
        );
        knob_row(ui, |ui| {
            eq_slider(app, ui, id, b as u8, EqField::Freq, band.freq_hz);
            eq_slider(app, ui, id, b as u8, EqField::Gain, band.gain_db);
            eq_slider(app, ui, id, b as u8, EqField::Q, band.q);
        });
    }
}

fn eq_slider(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: StripId,
    band: u8,
    field: EqField,
    current: f32,
) {
    let (min, max, label, unit, log, default) = match field {
        EqField::Freq => (
            EQ_MIN_HZ,
            EQ_MAX_HZ,
            "inspector.eq_freq",
            "Hz",
            true,
            StripEq::default().bands[band as usize].freq_hz,
        ),
        EqField::Gain => (
            -EQ_MAX_GAIN_DB,
            EQ_MAX_GAIN_DB,
            "inspector.eq_gain",
            "dB",
            false,
            0.0,
        ),
        EqField::Q => (EQ_MIN_Q, EQ_MAX_Q, "inspector.eq_q", "", true, 0.71),
    };
    let mut v = current;
    let r = ui.add(
        Knob::new(&mut v, min..=max, default)
            .log(log)
            .unit(unit)
            .label(t(label)),
    );
    if r.changed() {
        app.exec_merging(
            Command::SetStripEq {
                id,
                band,
                field,
                value: v,
            },
            MergeKey::of(&("seq", id.0, band, field as u8)),
        );
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

/// The EQ's magnitude response, 20 Hz to 20 kHz on a log axis, ±18 dB, with a datum at
/// 0 dB and a mark at each band's frequency. Drawn from the same coefficients the audio
/// thread runs (`cantus_dsp::ChannelEq`), so the picture cannot disagree with the sound.
pub fn eq_curve(ui: &mut egui::Ui, eq: &StripEq) {
    let pal = theme::p();
    let width = ui.available_width() - space::S2;
    let (rect, _) = ui.allocate_exact_size(vec2(width, space::GRID * 14.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let sr = 48_000.0;
    let mut dsp = cantus_dsp::ChannelEq::default();
    for (i, b) in eq.bands.iter().enumerate() {
        dsp.set_band(i, sr, b.freq_hz, b.gain_db, b.q);
    }
    let x_of = |hz: f32| {
        let f = (hz.max(EQ_MIN_HZ) / EQ_MIN_HZ).ln() / (EQ_MAX_HZ / EQ_MIN_HZ).ln();
        rect.left() + f * rect.width()
    };
    let y_of =
        |db: f32| rect.center().y - (db / EQ_MAX_GAIN_DB).clamp(-1.0, 1.0) * rect.height() * 0.5;
    // Datum and decade lines.
    p.hline(rect.x_range(), y_of(0.0), Stroke::new(1.0, pal.grid_strong));
    for hz in [100.0, 1_000.0, 10_000.0] {
        p.vline(x_of(hz), rect.y_range(), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(x_of(hz) + 2.0, rect.bottom() - 1.0),
            Align2::LEFT_BOTTOM,
            crate::views::mixer::hz_text(hz),
            design::paint::caption(),
            pal.text_dim,
        );
    }
    let n = 96;
    let points: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let f = i as f32 / n as f32;
            let hz = EQ_MIN_HZ * (EQ_MAX_HZ / EQ_MIN_HZ).powf(f);
            pos2(x_of(hz), y_of(dsp.magnitude_db(sr, hz)))
        })
        .collect();
    let tone = if eq.is_flat() {
        pal.text_dim
    } else {
        pal.gold_bright
    };
    p.add(egui::Shape::line(points, Stroke::new(1.5, tone)));
    for b in &eq.bands {
        let c = pos2(x_of(b.freq_hz), y_of(b.gain_db));
        p.circle_filled(c, 3.0, tone);
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
}

/// The strip's sends, with room for a level slider each; the mixer has the same rows in
/// a narrower form.
fn sends_section(app: &mut CantusApp, ui: &mut egui::Ui, strip: &MixerStrip) {
    let id = strip.id;
    ui.horizontal(|ui| {
        section(ui, "inspector.sends");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let targets: Vec<(StripId, usize, String)> = app
                .project()
                .strips
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.id != id
                        && strip.send_to(s.id).is_none()
                        && !app.project().routing_would_cycle(id, Some(s.id))
                })
                .map(|(i, s)| (s.id, i + 1, s.name.clone()))
                .collect();
            ui.menu_button(t("inspector.send_add"), |ui| {
                if targets.is_empty() {
                    ui.label(t("mixer.send_none"));
                }
                for (sid, n, name) in &targets {
                    let text = if name.is_empty() {
                        tf("mixer.send_to", &[("n", n)])
                    } else {
                        format!("{} · {name}", tf("mixer.send_to", &[("n", n)]))
                    };
                    if ui.button(text).clicked() {
                        app.set_strip_send(id, Send::new(*sid));
                        app.end_gesture();
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(t("mixer.send_add"));
        });
    });
    if strip.sends.is_empty() {
        ui.label(
            RichText::new(t("inspector.sends_none"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
    }
    for send in &strip.sends {
        let Some(n) = app.project().strip_number(send.to) else {
            continue;
        };
        let name = app
            .project()
            .strip(send.to)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        Elevation::Card.frame(&theme::p()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut on = send.enabled;
                if ui.checkbox(&mut on, "").changed() {
                    app.set_strip_send(
                        id,
                        Send {
                            enabled: on,
                            ..*send
                        },
                    );
                    app.end_gesture();
                }
                let label = if name.is_empty() {
                    tf("mixer.send_to", &[("n", &n)])
                } else {
                    format!("{} · {name}", tf("mixer.send_to", &[("n", &n)]))
                };
                ui.label(RichText::new(label).text_style(design::strong()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(t("effect.remove_short"))
                        .on_hover_text(t("mixer.send_remove"))
                        .clicked()
                    {
                        app.remove_strip_send(id, send.to);
                    }
                });
            });
            let mut level = send.level;
            let r = ui.add(
                Knob::new(&mut level, 0.0..=1.0, 1.0)
                    .unit("%")
                    .label(t("track.gain")),
            );
            if r.changed() {
                app.set_strip_send(id, Send { level, ..*send });
            }
            if r.drag_stopped() || r.double_clicked() {
                app.end_gesture();
            }
        });
    }
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
    let r = ui.add(
        Knob::new(&mut v, info.min..=info.max, info.default)
            .log(info.logarithmic)
            .stepped(info.stepped)
            .unit(info.unit)
            .label(t(info.label_key)),
    );
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
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

fn effects(app: &mut CantusApp, ui: &mut egui::Ui, strip: &MixerStrip) {
    let id = strip.id;
    let slots: &[EffectSlot] = &strip.effects;
    ui.horizontal(|ui| {
        heading(ui, &t("inspector.effects"));
        ui.label(
            RichText::new(tf(
                "inspector.effects_count",
                &[("n", &slots.len()), ("max", &MAX_INSERTS)],
            ))
            .text_style(design::caption())
            .color(theme::p().text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let full = slots.len() >= MAX_INSERTS;
            ui.add_enabled_ui(!full, |ui| {
                ui.menu_button(t("effect.add"), |ui| {
                    for kind in EffectKind::ALL {
                        if ui.button(t(kind.name_key())).clicked() {
                            ui.close();
                            app.add_effect(id, kind);
                        }
                    }
                    // Plug-ins made to the market's standards (ADR 0019).
                    crate::views::plugins::insert_menu(app, ui, id);
                })
                .response
                .on_disabled_hover_text(t("effect.full"));
            });
            if !strip.fx_enabled {
                ui.label(
                    RichText::new(t("mixer.fx_off"))
                        .text_style(design::caption())
                        .color(theme::p().danger),
                );
            }
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
                        strip: id,
                        effect: slot.id,
                        bypass: !on,
                    });
                }
                if slot.spec.kind == EffectKind::External {
                    crate::views::plugins::slot_title(app, ui, slot);
                } else {
                    ui.label(
                        RichText::new(t(slot.spec.kind.name_key())).text_style(design::strong()),
                    );
                }
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
                            strip: id,
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
                            strip: id,
                            effect: slot.id,
                            index: i - 1,
                        });
                    }
                });
            });
            // An effect with an editor of its own shows what it does, and opens the editor;
            // its parameters are not a row of knobs here.
            if slot.spec.kind.has_editor() {
                let preview = match slot.spec.kind {
                    EffectKind::Compressor => crate::views::comp_editor::mini_curve(ui, &slot.spec),
                    EffectKind::Eq => crate::views::eq_editor::mini_curve(ui, &slot.spec),
                    EffectKind::External => crate::views::plugins::preview(app, ui, slot),
                    _ => crate::views::fx::preview(ui, &slot.spec),
                };
                if preview.clicked() || ui.button(t("eq.open")).clicked() {
                    app.open_effect_editor(id, slot.id);
                }
            }
            knob_row(ui, |ui| {
                // The slot's own dry/wet, outside the effect: FL's knob beside every slot.
                let mut mix = slot.mix;
                let m = ui.add(
                    Knob::new(&mut mix, 0.0..=1.0, 1.0)
                        .unit("%")
                        .label(t("effect.mix")),
                );
                if m.changed() {
                    app.exec_merging(
                        Command::SetEffectMix {
                            strip: id,
                            effect: slot.id,
                            mix,
                        },
                        MergeKey::of(&("fxmix", slot.id.0)),
                    );
                }
                if m.drag_stopped() || m.double_clicked() {
                    app.end_gesture();
                }
                let own_editor = slot.spec.kind.has_editor();
                for info in slot.spec.kind.params().iter().filter(|_| !own_editor) {
                    let mut v = slot.spec.get(info.key);
                    let r = ui.add(
                        Knob::new(&mut v, info.min..=info.max, info.default)
                            .log(info.logarithmic)
                            .stepped(info.stepped)
                            .unit(if info.key == "mix" { "%" } else { info.unit })
                            .label(t(info.label_key)),
                    );
                    if r.changed() {
                        app.exec_merging(
                            Command::SetEffectParam {
                                strip: id,
                                effect: slot.id,
                                key: info.key.to_string(),
                                value: v,
                            },
                            MergeKey::of(&("fx", slot.id.0, info.key)),
                        );
                    }
                    if r.drag_stopped() || r.double_clicked() {
                        app.end_gesture();
                    }
                }
            });
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

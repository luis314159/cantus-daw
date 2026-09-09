//! Transport bar: menus, play/stop/loop/metronome, tempo, position, undo/redo, master meter.

use crate::app::CantusApp;
use crate::i18n::{t, tf, Locale};
use crate::icons::{self, Icon};
use crate::theme;
use cantus_core::{Command, MergeKey, Ticks};
use egui::{pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind};

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        menus(app, ui);
        ui.separator();
        buttons(app, ui);
        ui.separator();
        tempo_and_position(app, ui);
        ui.separator();
        undo_redo(app, ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            master_meter(app, ui);
            ui.separator();
            let name = app.project().name.clone();
            let dirty = if app.bus.is_dirty() { " •" } else { "" };
            ui.label(RichText::new(format!("{name}{dirty}")).color(theme::p().text_dim));
        });
    });
}

fn menus(app: &mut CantusApp, ui: &mut egui::Ui) {
    ui.menu_button(t("menu.file"), |ui| {
        if ui.button(t("menu.new")).clicked() {
            ui.close();
            app.new_project();
        }
        if ui.button(t("menu.open")).clicked() {
            ui.close();
            app.open_dialog();
        }
        ui.separator();
        if ui.button(t("menu.save")).clicked() {
            ui.close();
            app.save();
        }
        if ui.button(t("menu.save_as")).clicked() {
            ui.close();
            app.save_as();
        }
        ui.separator();
        if ui.button(t("menu.export_wav")).clicked() {
            ui.close();
            app.export_dialog();
        }
        ui.separator();
        if ui.button(t("menu.quit")).clicked() {
            ui.close();
            app.request_quit = true;
        }
    });
    ui.menu_button(t("menu.edit"), |ui| {
        let undo = app
            .bus
            .undo_label_key()
            .map(|k| tf("menu.undo_named", &[("what", &t(k))]));
        let redo = app
            .bus
            .redo_label_key()
            .map(|k| tf("menu.redo_named", &[("what", &t(k))]));
        if ui
            .add_enabled(
                undo.is_some(),
                egui::Button::new(undo.unwrap_or_else(|| t("menu.undo"))),
            )
            .clicked()
        {
            ui.close();
            app.undo();
        }
        if ui
            .add_enabled(
                redo.is_some(),
                egui::Button::new(redo.unwrap_or_else(|| t("menu.redo"))),
            )
            .clicked()
        {
            ui.close();
            app.redo();
        }
        ui.separator();
        if ui.button(t("menu.cut")).clicked() {
            ui.close();
            app.cut_selection();
        }
        if ui.button(t("menu.copy")).clicked() {
            ui.close();
            app.copy_selection();
        }
        if ui.button(t("menu.paste")).clicked() {
            ui.close();
            app.paste_clipboard();
        }
        if ui.button(t("menu.duplicate")).clicked() {
            ui.close();
            app.duplicate_selection();
        }
        ui.separator();
        if ui.button(t("menu.select_all")).clicked() {
            ui.close();
            app.select_all_notes();
        }
        if ui.button(t("menu.delete")).clicked() {
            ui.close();
            app.delete_selection();
        }
        if ui.button(t("menu.quantize")).clicked() {
            ui.close();
            app.quantize_selection();
        }
    });
    ui.menu_button(t("menu.view"), |ui| {
        ui.menu_button(t("menu.language"), |ui| {
            for l in Locale::ALL {
                if ui
                    .selectable_label(crate::i18n::locale() == l, l.native_name())
                    .clicked()
                {
                    crate::i18n::set_locale(l);
                    ui.close();
                }
            }
        });
        ui.menu_button(t("menu.theme"), |ui| {
            for (key, make) in theme::PRESETS {
                if ui.button(t(key)).clicked() {
                    ui.close();
                    app.apply_theme_preset(make());
                }
            }
            ui.separator();
            if ui.button(t("menu.open_theme_file")).clicked() {
                ui.close();
                app.reveal_theme_file();
            }
        });
        ui.checkbox(&mut app.keyboard_piano, t("menu.keyboard_piano"));
        ui.checkbox(&mut app.show_stats, t("menu.stats"));
    });
    ui.menu_button(t("menu.help"), |ui| {
        if ui.button(t("menu.shortcuts")).clicked() {
            ui.close();
            app.show_shortcuts = true;
        }
        if ui.button(t("menu.about")).clicked() {
            ui.close();
            app.show_about = true;
        }
    });
}

fn buttons(app: &mut CantusApp, ui: &mut egui::Ui) {
    if icons::button(ui, Icon::ToStart, false, t("transport.to_start")).clicked() {
        app.seek(Ticks::ZERO);
    }
    let playing = app.engine.is_playing();
    let icon = if playing { Icon::Pause } else { Icon::Play };
    if icons::button(ui, icon, playing, t("transport.play")).clicked() {
        app.toggle_play();
    }
    if icons::button(ui, Icon::Stop, false, t("transport.stop")).clicked() {
        app.stop();
    }
    let recording = app.is_recording();
    if icons::button(ui, Icon::Record, recording, t("transport.record")).clicked() {
        app.toggle_record();
    }
    let mut looping = app.project().loop_enabled;
    if ui.toggle_value(&mut looping, t("transport.loop")).changed() {
        app.exec(Command::SetLoopEnabled { enabled: looping });
    }
    let mut metro = app.project().metronome;
    if ui
        .toggle_value(&mut metro, t("transport.metronome"))
        .changed()
    {
        app.exec(Command::SetMetronome { enabled: metro });
    }
    let mut typing = app.keyboard_piano;
    if ui
        .toggle_value(&mut typing, t("transport.typing"))
        .on_hover_text(t("transport.typing_tip"))
        .changed()
    {
        app.keyboard_piano = typing;
    }
    if ui
        .small_button(t("transport.panic_short"))
        .on_hover_text(t("transport.panic"))
        .clicked()
    {
        app.engine.panic();
    }
}

fn tempo_and_position(app: &mut CantusApp, ui: &mut egui::Ui) {
    let mut bpm = app.project().tempo_bpm;
    let resp = ui.add(
        egui::DragValue::new(&mut bpm)
            .speed(0.1)
            .range(20.0..=999.0)
            .fixed_decimals(1)
            .suffix(format!(" {}", t("transport.bpm"))),
    );
    if resp.changed() {
        let key = MergeKey::of(&"tempo");
        app.exec_merging(Command::SetTempo { bpm }, key);
    }
    if resp.drag_stopped() || resp.lost_focus() {
        app.end_gesture();
    }
    let pos = app.engine.position();
    let ts = app.project().time_signature;
    let (bar, beat, tick) = ts.bbt(pos);
    let secs = pos.to_seconds(app.project().tempo_bpm);
    let text = format!(
        "{bar:>3}.{beat}.{tick:03}  {:02}:{:05.2}",
        (secs / 60.0) as i64,
        secs % 60.0
    );
    ui.label(
        RichText::new(text)
            .monospace()
            .size(15.0)
            .color(theme::p().gold_bright),
    )
    .on_hover_text(t("transport.position"));
    ui.label(RichText::new(ts.to_string()).color(theme::p().text_dim));
}

pub fn view_tabs(app: &mut CantusApp, ui: &mut egui::Ui) {
    use crate::app::CentralView;
    for (view, key) in [
        (CentralView::PianoRoll, "view.piano_roll"),
        (CentralView::Mixer, "view.mixer"),
    ] {
        if ui.selectable_label(app.central == view, t(key)).clicked() {
            app.central = view;
        }
    }
}

fn undo_redo(app: &mut CantusApp, ui: &mut egui::Ui) {
    let undo_tip = app
        .bus
        .undo_label_key()
        .map(|k| tf("menu.undo_named", &[("what", &t(k))]))
        .unwrap_or_else(|| t("menu.undo"));
    if ui
        .add_enabled(app.bus.can_undo(), egui::Button::new(t("menu.undo")))
        .on_hover_text(undo_tip)
        .clicked()
    {
        app.undo();
    }
    let redo_tip = app
        .bus
        .redo_label_key()
        .map(|k| tf("menu.redo_named", &[("what", &t(k))]))
        .unwrap_or_else(|| t("menu.redo"));
    if ui
        .add_enabled(app.bus.can_redo(), egui::Button::new(t("menu.redo")))
        .on_hover_text(redo_tip)
        .clicked()
    {
        app.redo();
    }
}

fn master_meter(app: &mut CantusApp, ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(120.0, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, theme::p().ink);
    let m = app.engine.meters().master;
    for (ch, peak) in m.iter().enumerate() {
        let h = (rect.height() - 4.0) / 2.0 - 1.0;
        let y0 = rect.top() + 2.0 + ch as f32 * (h + 2.0);
        let db = cantus_core::linear_to_db(*peak);
        let frac = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let w = (rect.width() - 4.0) * frac;
        let bar = Rect::from_min_size(pos2(rect.left() + 2.0, y0), vec2(w, h));
        p.rect_filled(bar, 1.0, theme::meter_color(*peak));
    }
    // 0 dBFS tick.
    p.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, theme::p().grid_strong),
        StrokeKind::Inside,
    );
    let peak = m[0].max(m[1]);
    if peak >= 1.0 {
        p.text(
            rect.right_center() + vec2(-4.0, 0.0),
            Align2::RIGHT_CENTER,
            "CLIP",
            FontId::proportional(9.0),
            Color32::WHITE,
        );
    }
}

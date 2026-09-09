//! Transport bar: menus, play/stop/loop/metronome, tempo, position, undo/redo, master meter.

use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::icons::{self, Icon};
use crate::theme;
use cantus_core::{Command, MergeKey, Ticks};
use egui::{pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind};

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    // Right-to-left on the outside, then left-to-right for the remainder: the meter and
    // the project name claim their width first, so a narrow window squeezes the controls
    // instead of letting them overrun the readouts.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            master_meter(app, ui);
            ui.separator();
            let name = app.project().name.clone();
            let dirty = if app.bus.is_dirty() { " •" } else { "" };
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{name}{dirty}"))
                        .text_style(design::caption())
                        .color(theme::p().text_dim),
                )
                .truncate(),
            );
            ui.separator();
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                menus(app, ui);
                ui.separator();
                buttons(app, ui);
                ui.separator();
                tempo_and_position(app, ui);
                ui.separator();
                undo_redo(app, ui);
            });
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
        // Language and theme live in Settings rather than here as well: two places to
        // change one thing is how they drift apart.
        if ui.button(t("menu.settings")).clicked() {
            ui.close();
            app.show_settings = true;
        }
        ui.separator();
        layout_menu(app, ui);
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

/// *View → Layout*: switch between saved window shapes, toggle the panels, save the
/// current shape under a name, or go back to the built-in one.
fn layout_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    ui.menu_button(t("menu.layout"), |ui| {
        let current = app.layouts.current.name.clone();
        let saved: Vec<String> = app.layouts.saved.iter().map(|l| l.name.clone()).collect();
        for name in &saved {
            if ui.selectable_label(*name == current, name).clicked() {
                ui.close();
                app.apply_layout(name);
            }
        }
        if !saved.is_empty() {
            ui.separator();
        }

        // Panel visibility is part of the layout, so toggling it here is an edit to the
        // current layout, not a separate piece of view state.
        let mut arrangement = app.layouts.current.show_arrangement;
        if ui
            .checkbox(&mut arrangement, t("layout.show_arrangement"))
            .changed()
        {
            app.layouts.current.show_arrangement = arrangement;
        }
        let mut inspector = app.layouts.current.show_inspector;
        if ui
            .checkbox(&mut inspector, t("layout.show_inspector"))
            .changed()
        {
            app.layouts.current.show_inspector = inspector;
        }
        ui.separator();

        if ui.button(t("layout.save_as")).clicked() {
            ui.close();
            // Offer the current name, so re-saving a layout is one keystroke.
            let seed = if current == crate::layout::DEFAULT_NAME {
                String::new()
            } else {
                current.clone()
            };
            app.naming_layout = Some(seed);
        }
        let is_saved = saved.contains(&current);
        if ui
            .add_enabled(is_saved, egui::Button::new(t("layout.delete")))
            .clicked()
        {
            ui.close();
            let ctx = ui.ctx().clone();
            app.delete_layout(&current, &ctx);
        }
        if ui.button(t("layout.reset")).clicked() {
            ui.close();
            app.reset_layout();
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
    let looping = app.project().loop_enabled;
    if icons::button(ui, Icon::Loop, looping, t("transport.loop")).clicked() {
        app.exec(Command::SetLoopEnabled { enabled: !looping });
    }
    let metro = app.project().metronome;
    if icons::button(ui, Icon::Metronome, metro, t("transport.metronome")).clicked() {
        app.exec(Command::SetMetronome { enabled: !metro });
    }
    let typing = app.keyboard_piano;
    if icons::button(
        ui,
        Icon::Keyboard,
        typing,
        format!("{} — {}", t("transport.typing"), t("transport.typing_tip")),
    )
    .clicked()
    {
        app.keyboard_piano = !typing;
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
    // Bars.beats.ticks is what a musician reads; the clock is the check. Both are
    // monospaced so the digits stay in their columns as the transport runs.
    let bbt = format!("{bar:>3}.{beat}.{tick:03}");
    let clock = format!("{:02}:{:05.2}", (secs / 60.0) as i64, secs % 60.0);
    ui.label(
        RichText::new(bbt)
            .text_style(design::readout())
            .color(theme::p().gold_bright),
    )
    .on_hover_text(t("transport.position"));
    ui.label(
        RichText::new(clock)
            .text_style(design::readout_small())
            .color(theme::p().text_dim),
    );
    ui.label(
        RichText::new(ts.to_string())
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
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
    let (rect, _) = ui.allocate_exact_size(vec2(space::GRID * 30.0, space::S4), Sense::hover());
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
            FontId::proportional(10.0),
            Color32::WHITE,
        );
    }
}

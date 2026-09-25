//! Hosted plug-ins in the interface (ADR 0019).
//!
//! - **The insert menu's plug-in list**: every effect the scans found, by vendor or by
//!   category, with a search field; a click puts it in the strip's chain.
//! - **The slot**: the plug-in's own name and a small format badge; *missing* in the danger
//!   colour, with the reason, when the plug-in is not on this computer.
//! - **The generic editor**, inside the shared FX frame (`views::fx`): the plug-in's
//!   parameters grouped as the plug-in groups them, the house knob for each, the plug-in's
//!   own text for its values; host-kept A/B and presets as saved states; the sidechain
//!   choice when the plug-in has a second input; and *Open plug-in window* for its own
//!   editor.
//! - **The plug-in manager** in Settings: folders, rescans, and the blocked list.

use crate::app::CantusApp;
use crate::design::{self, space as grid};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::{Command, EffectId, EffectSlot, EffectSpec, MergeKey, PluginState, StripId};
use cantus_host::{ParamInfo, PluginInfo};
use egui::{RichText, Sense};
use std::collections::{BTreeMap, HashMap};

/// The interface's own state for hosted plug-ins.
#[derive(Debug, Default)]
pub struct State {
    /// What is typed in the insert menu's search field.
    pub search: String,
    /// The insert menu groups by category rather than by vendor.
    pub by_category: bool,
    /// What is typed in the editor's parameter filter.
    pub filter: String,
    /// The other side of A/B for each hosted effect that has one — its state and values —
    /// and whether B is the side showing.
    ab: HashMap<EffectId, (PluginState, BTreeMap<u32, f64>, bool)>,
    /// The name being typed for a user preset.
    preset_name: String,
    /// A folder being typed in the manager.
    pub folder: String,
}

/// A small outlined tag: the plug-in's format.
pub fn badge(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let pal = theme::p();
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, pal.grid_strong))
        .corner_radius(pal.corner_radius)
        .inner_margin(egui::Margin::symmetric(grid::S1 as i8, 0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .text_style(design::mono_small())
                    .color(pal.text_dim),
            )
        })
        .response
}

/// The words of a plug-in's features that say what it is ("equalizer", "reverb"), without
/// the ones that only say what it plugs into.
fn category(p: &PluginInfo) -> String {
    const PLUMBING: [&str; 9] = [
        "audio-effect",
        "instrument",
        "note-effect",
        "note-detector",
        "stereo",
        "mono",
        "surround",
        "ambisonic",
        "fx",
    ];
    p.features
        .iter()
        .find(|f| !PLUMBING.contains(&f.to_lowercase().as_str()))
        .map(|f| {
            let mut c = f.replace(['-', '_'], " ");
            if let Some(first) = c.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            c
        })
        .unwrap_or_else(|| t("host.menu.other"))
}

fn plugin_row(ui: &mut egui::Ui, p: &PluginInfo) -> bool {
    ui.horizontal(|ui| {
        let clicked = ui
            .button(&p.name)
            .on_hover_text(tf(
                "host.menu.tip",
                &[("vendor", &p.vendor), ("version", &p.version)],
            ))
            .clicked();
        badge(ui, cantus_host::to_project_format(p.format).badge());
        clicked
    })
    .inner
}

/// The plug-ins part of a strip's insert menu.
pub fn insert_menu(app: &mut CantusApp, ui: &mut egui::Ui, strip: StripId) {
    let pal = theme::p();
    ui.separator();
    ui.label(
        RichText::new(t("host.menu.title"))
            .text_style(design::caption())
            .color(pal.text_dim),
    );
    let effects: Vec<PluginInfo> = app.plugins.db.effects().into_iter().cloned().collect();
    if effects.is_empty() {
        ui.label(
            RichText::new(t("host.menu.none"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        let scanning = app.plugins.scanning();
        if ui
            .add_enabled(!scanning, egui::Button::new(t("host.manager.scan")))
            .clicked()
        {
            app.plugins.start_scan(false);
        }
        return;
    }
    let state = &mut app.fx.hosted;
    ui.add(
        egui::TextEdit::singleline(&mut state.search)
            .hint_text(t("host.menu.search"))
            .desired_width(grid::GRID * 48.0),
    );
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.by_category, false, t("host.menu.by_vendor"));
        ui.selectable_value(&mut state.by_category, true, t("host.menu.by_category"));
    });
    let query = state.search.trim().to_lowercase();
    let by_category = state.by_category;
    let mut chosen: Option<PluginInfo> = None;
    if !query.is_empty() {
        let found: Vec<&PluginInfo> = effects
            .iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query)
                    || p.vendor.to_lowercase().contains(&query)
                    || p.features.iter().any(|f| f.to_lowercase().contains(&query))
            })
            .collect();
        if found.is_empty() {
            ui.label(
                RichText::new(t("host.menu.no_match"))
                    .text_style(design::caption())
                    .color(pal.text_dim),
            );
        }
        for p in found {
            if plugin_row(ui, p) {
                chosen = Some(p.clone());
            }
        }
    } else {
        let mut groups: BTreeMap<String, Vec<&PluginInfo>> = BTreeMap::new();
        for p in &effects {
            let key = if by_category {
                category(p)
            } else if p.vendor.is_empty() {
                t("host.menu.no_vendor")
            } else {
                p.vendor.clone()
            };
            groups.entry(key).or_default().push(p);
        }
        for (group, list) in groups {
            ui.menu_button(format!("{group} ({})", list.len()), |ui| {
                for p in list {
                    if plugin_row(ui, p) {
                        chosen = Some(p.clone());
                    }
                }
            });
        }
    }
    if let Some(p) = chosen {
        ui.close();
        app.add_plugin(strip, &p);
    }
}

impl CantusApp {
    /// Put a hosted plug-in at the end of a strip's chain: one command, one undo step.
    pub fn add_plugin(&mut self, strip: StripId, info: &PluginInfo) {
        match self.plugins.make_slot(&mut self.bus, strip, info) {
            Ok(slot) => {
                let index = self.project().strip(strip).map_or(0, |s| s.effects.len());
                self.exec(Command::InsertEffect {
                    strip,
                    index,
                    slot: Box::new(slot),
                });
            }
            Err(e) => self.set_error(tf(
                "host.load_failed",
                &[("name", &info.name), ("error", &e)],
            )),
        }
    }

    /// Set one parameter of a hosted plug-in; a gesture merges frame to frame into one
    /// undo step, exactly as the plug-in's own window does.
    pub fn set_plugin_param(
        &mut self,
        strip: StripId,
        effect: EffectId,
        id: u32,
        value: f64,
        gesture: bool,
    ) {
        let cmd = Command::SetPluginParam {
            strip,
            effect,
            id,
            value: Some(value),
        };
        if gesture {
            self.exec_merging(cmd, MergeKey::of(&("plugin", effect.0, id)));
        } else {
            self.exec(cmd);
        }
    }

    /// Replace a hosted plug-in's state and values, as one command.
    pub fn set_plugin_state(
        &mut self,
        strip: StripId,
        effect: EffectId,
        state: PluginState,
        params: BTreeMap<u32, f64>,
    ) {
        self.exec(Command::SetPluginState {
            strip,
            effect,
            state,
            params,
        });
    }
}

/// A slot's title in the inspector: the plug-in's name, its format, and *missing* when it
/// is not on this computer.
pub fn slot_title(app: &CantusApp, ui: &mut egui::Ui, slot: &EffectSlot) {
    let pal = theme::p();
    let Some(p) = slot.spec.external.as_deref() else {
        ui.label(RichText::new(t(slot.spec.kind.name_key())).text_style(design::strong()));
        return;
    };
    ui.label(RichText::new(&p.name).text_style(design::strong()))
        .on_hover_text(&p.vendor);
    badge(ui, p.format.badge());
    if let Some(why) = app.plugins.missing(slot.id) {
        ui.label(
            RichText::new(t("host.missing"))
                .text_style(design::caption())
                .color(pal.danger),
        )
        .on_hover_text(tf(
            "host.missing_tip",
            &[("vendor", &p.vendor), ("why", &why)],
        ));
    }
}

/// The inspector's picture of a hosted plug-in: how many values it holds, and its latency.
pub fn preview(app: &CantusApp, ui: &mut egui::Ui, slot: &EffectSlot) -> egui::Response {
    let pal = theme::p();
    let n = slot.spec.external.as_deref().map_or(0, |p| p.params.len());
    let text = match app.plugins.hosted(slot.id) {
        Some(h) => tf("host.preview", &[("n", &n), ("latency", &h.latency())]),
        None => t("host.preview_missing"),
    };
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .text_style(design::caption())
                .color(pal.text_dim),
        )
        .sense(Sense::click()),
    )
}

/// Every hosted slot's spec, for code that has only a spec (the frame's preview).
pub fn is_hosted(spec: &EffectSpec) -> bool {
    spec.external.is_some()
}

// ---- the generic editor ---------------------------------------------------------------

/// The value a parameter shows: the project's, or the plug-in's own when the project holds
/// none for it.
fn shown_value(app: &CantusApp, slot: &EffectSlot, p: &ParamInfo) -> f64 {
    slot.spec
        .external
        .as_deref()
        .and_then(|e| e.params.get(&p.id).copied())
        .or_else(|| {
            app.plugins
                .hosted(slot.id)
                .and_then(|h| h.param_value(p.id))
        })
        .unwrap_or(p.default)
}

/// One parameter as a house knob, its value in the plug-in's own words.
fn param_knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    slot: &EffectSlot,
    p: &ParamInfo,
) {
    let value = shown_value(app, slot, p);
    let (lo, hi) = (p.min.min(p.max), p.max.max(p.min));
    if hi <= lo {
        // A parameter with no range (a read-out): its name and its text.
        let text = app
            .plugins
            .hosted(slot.id)
            .and_then(|h| h.param_text(p.id, value))
            .unwrap_or_else(|| format!("{value:.2}"));
        ui.label(
            RichText::new(format!("{}: {text}", p.name))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        return;
    }
    let mut v = value as f32;
    let r = {
        let hosted = app.plugins.hosted(slot.id);
        let id = p.id;
        let knob = Knob::new(&mut v, lo as f32..=hi as f32, p.default as f32)
            .diameter(grid::GRID * 11.0)
            .stepped(p.stepped)
            .label(p.name.clone())
            .format(move |x| {
                hosted
                    .and_then(|h| h.param_text(id, f64::from(x)))
                    .unwrap_or_else(|| format!("{x:.2}"))
            });
        ui.add_enabled(!p.read_only && hi > lo, knob)
    };
    if r.changed() {
        app.set_plugin_param(strip, slot.id, p.id, f64::from(v), true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

/// The body of a hosted plug-in's editor inside the shared frame.
pub fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let pal = theme::p();
    let Some(record) = slot.spec.external.as_deref() else {
        return;
    };
    // What it is, what it costs, and its own window.
    ui.horizontal(|ui| {
        ui.label(RichText::new(&record.name).text_style(design::strong()));
        badge(ui, record.format.badge());
        ui.label(
            RichText::new(&record.vendor)
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        if let Some(h) = app.plugins.hosted(effect) {
            ui.label(
                RichText::new(tf(
                    "host.editor.latency",
                    &[
                        ("samples", &h.latency()),
                        (
                            "ms",
                            &format!(
                                "{:.1}",
                                h.latency() as f64 * 1000.0 / app.engine.sample_rate
                            ),
                        ),
                    ],
                ))
                .text_style(design::mono_small())
                .color(pal.text_dim),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (has_editor, open) = app
                .plugins
                .hosted(effect)
                .map_or((false, false), |h| (h.has_editor(), h.window_open()));
            let text = if open {
                t("host.editor.close_window")
            } else {
                t("host.editor.open_window")
            };
            let button = ui
                .add_enabled(has_editor, egui::Button::new(text).selected(open))
                .on_hover_text(t("host.editor.window_tip"))
                .on_disabled_hover_text(t("host.editor.no_window"));
            if button.clicked() {
                app.toggle_plugin_window(effect);
            }
            if record.sidechain {
                key_menu(app, ui, strip, effect, slot);
            }
        });
    });
    if let Some(why) = app.plugins.missing(effect) {
        ui.add_space(grid::S2);
        ui.label(
            RichText::new(tf(
                "host.editor.missing",
                &[("name", &record.name), ("why", &why)],
            ))
            .color(pal.danger),
        );
        ui.label(
            RichText::new(t("host.editor.missing_kept"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        return;
    }
    let params: Vec<ParamInfo> = app
        .plugins
        .hosted(effect)
        .map(|h| h.params.clone())
        .unwrap_or_default();
    ui.add_space(grid::S1);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.fx.hosted.filter)
                .hint_text(t("host.editor.filter"))
                .desired_width(grid::GRID * 48.0),
        );
        ui.label(
            RichText::new(tf("host.editor.count", &[("n", &params.len())]))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
    });
    ui.add_space(grid::S1);
    let filter = app.fx.hosted.filter.trim().to_lowercase();
    // Grouped as the plug-in groups them, in its order.
    let mut groups: Vec<(String, Vec<ParamInfo>)> = Vec::new();
    for p in params.into_iter().filter(|p| !p.hidden) {
        if !filter.is_empty() && !p.name.to_lowercase().contains(&filter) {
            continue;
        }
        match groups.iter_mut().find(|(g, _)| *g == p.group) {
            Some((_, list)) => list.push(p),
            None => groups.push((p.group.clone(), vec![p])),
        }
    }
    // Each group a card, the cards flowing left to right and wrapping; a group too large
    // for one row takes the width and wraps its own knobs.
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let full = ui.available_width() - grid::S6;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(grid::S2, grid::S2);
                for (group, list) in &groups {
                    let title = if group.is_empty() {
                        t("host.editor.ungrouped")
                    } else {
                        group.replace('/', " \u{203a} ")
                    };
                    let width = (list.len() as f32 * grid::GRID * 16.0)
                        .max(grid::GRID * 24.0)
                        .min(full);
                    design::Elevation::Card.frame(&pal).show(ui, |ui| {
                        ui.set_width(width);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(title.to_uppercase())
                                    .text_style(design::caption())
                                    .color(pal.text_dim),
                            );
                            ui.horizontal_wrapped(|ui| {
                                for p in list {
                                    param_knob(app, ui, strip, slot, p);
                                }
                            });
                        });
                    });
                }
            });
        });
}

/// Which strip keys the plug-in's sidechain input.
fn key_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let current = slot.sidechain;
    let label = match current {
        Some(k) if app.project().strip(k).is_some() => tf(
            "host.editor.key",
            &[(
                "strip",
                &app.capture_label(cantus_engine::CaptureTarget::Strip(k)),
            )],
        ),
        _ => t("host.editor.key_none"),
    };
    egui::ComboBox::from_id_salt(("host-key", effect.0))
        .selected_text(label)
        .width(grid::GRID * 40.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), t("host.editor.key_none"))
                .clicked()
            {
                app.exec(Command::SetEffectSidechain {
                    strip,
                    effect,
                    source: None,
                });
            }
            let options: Vec<StripId> = app
                .project()
                .strips
                .iter()
                .map(|x| x.id)
                .filter(|id| *id != strip && !app.project().routing_would_cycle(*id, Some(strip)))
                .collect();
            for id in options {
                let text = app.capture_label(cantus_engine::CaptureTarget::Strip(id));
                if ui.selectable_label(current == Some(id), text).clicked() {
                    app.exec(Command::SetEffectSidechain {
                        strip,
                        effect,
                        source: Some(id),
                    });
                }
            }
        })
        .response
        .on_hover_text(t("host.editor.key_tip"));
}

impl CantusApp {
    /// Open the plug-in's own window, or close it.
    pub fn toggle_plugin_window(&mut self, effect: EffectId) {
        if self.plugins.close_window(effect) {
            return;
        }
        let title = self
            .project()
            .strips
            .iter()
            .find_map(|s| {
                s.effect(effect).map(|e| {
                    let name = e
                        .spec
                        .external
                        .as_deref()
                        .map_or_else(|| t("host.plugin"), |p| p.name.clone());
                    format!(
                        "{name} \u{2014} {}",
                        self.capture_label(cantus_engine::CaptureTarget::Strip(s.id))
                    )
                })
            })
            .unwrap_or_default();
        let owner = self.plugins.owner_window;
        if let Err(e) = self.plugins.open_window(effect, &title, owner) {
            self.set_error(tf("host.editor.window_failed", &[("error", &e)]));
        }
    }
}

// ---- A/B and presets, kept by the host ------------------------------------------------

/// Switch between A and B: the showing side is kept, the other one applied — one command.
fn toggle_ab(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let Some(current) = app.plugins.capture(effect) else {
        return;
    };
    let (other_state, other_params, on_b) = app
        .fx
        .hosted
        .ab
        .remove(&effect)
        .unwrap_or_else(|| (current.0.clone(), current.1.clone(), false));
    app.set_plugin_state(strip, effect, other_state, other_params);
    app.fx
        .hosted
        .ab
        .insert(effect, (current.0, current.1, !on_b));
}

/// The folder a plug-in's presets live in: one per plug-in id.
pub fn preset_dir(plugin_id: &str) -> std::path::PathBuf {
    let clean: String = plugin_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    crate::app::data_dir()
        .join("presets")
        .join("plugins")
        .join(clean)
}

/// A saved state, as a preset file.
#[derive(serde::Serialize, serde::Deserialize)]
struct PresetFile {
    state: PluginState,
    params: BTreeMap<u32, f64>,
}

fn presets(plugin_id: &str) -> Vec<(String, std::path::PathBuf)> {
    let mut out = Vec::new();
    if let Ok(dir) = std::fs::read_dir(preset_dir(plugin_id)) {
        for e in dir.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) == Some("json") {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push((name, path));
            }
        }
    }
    out.sort();
    out
}

/// The frame's header for a hosted plug-in: presets as saved states, and A/B.
pub fn header_controls(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let Some(record) = slot.spec.external.as_deref() else {
        return;
    };
    let loaded = app.plugins.hosted(effect).is_some();
    let plugin_id = record.id.clone();
    ui.add_enabled_ui(loaded, |ui| {
        ui.menu_button(t("preset.menu"), |ui| {
            let list = presets(&plugin_id);
            if list.is_empty() {
                ui.label(
                    RichText::new(t("host.preset.none"))
                        .text_style(design::caption())
                        .color(theme::p().text_dim),
                );
            }
            for (name, path) in list {
                if ui.button(&name).clicked() {
                    let file = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|t| serde_json::from_str::<PresetFile>(&t).ok());
                    if let Some(f) = file {
                        app.set_plugin_state(strip, effect, f.state, f.params);
                    }
                    ui.close();
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.fx.hosted.preset_name)
                        .hint_text(t("preset.name"))
                        .desired_width(grid::GRID * 36.0),
                );
                if ui.button(t("preset.save")).clicked() {
                    let name = app.fx.hosted.preset_name.trim().to_string();
                    match save_preset(app, effect, &plugin_id, &name) {
                        Ok(()) => {
                            app.set_status(tf("preset.saved", &[("name", &name)]));
                            app.fx.hosted.preset_name.clear();
                            ui.close();
                        }
                        Err(e) => app.set_error(e),
                    }
                }
            });
        });
        let on_b = app.fx.hosted.ab.get(&effect).is_some_and(|x| x.2);
        if ui
            .selectable_label(!on_b, "A")
            .on_hover_text(t("ab.tip"))
            .clicked()
            && on_b
        {
            toggle_ab(app, strip, effect);
        }
        if ui
            .selectable_label(on_b, "B")
            .on_hover_text(t("ab.tip"))
            .clicked()
            && !on_b
        {
            toggle_ab(app, strip, effect);
        }
        if ui
            .small_button(t("ab.copy"))
            .on_hover_text(t("ab.copy_tip"))
            .clicked()
        {
            if let Some((state, params)) = app.plugins.capture(effect) {
                app.fx.hosted.ab.insert(effect, (state, params, on_b));
            }
        }
    });
}

fn save_preset(
    app: &mut CantusApp,
    effect: EffectId,
    plugin_id: &str,
    name: &str,
) -> Result<(), String> {
    let clean: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    if clean.trim().is_empty() {
        return Err(t("preset.empty_name"));
    }
    let (state, params) = app
        .plugins
        .capture(effect)
        .ok_or_else(|| t("host.preview_missing"))?;
    let dir = preset_dir(plugin_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text =
        serde_json::to_string_pretty(&PresetFile { state, params }).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{}.json", clean.trim())), text).map_err(|e| e.to_string())
}

// ---- the plug-in manager, in Settings -------------------------------------------------

/// Folders, rescans, what was found, and the blocked list.
pub fn manager(app: &mut CantusApp, ui: &mut egui::Ui) {
    let pal = theme::p();
    let dim = |s: String| {
        RichText::new(s)
            .text_style(design::caption())
            .color(pal.text_dim)
    };
    let found = app.plugins.db.plugins().len();
    let blocked = app.plugins.db.blocked().len();
    ui.horizontal(|ui| {
        let scanning = app.plugins.scanning();
        if ui
            .add_enabled(!scanning, egui::Button::new(t("host.manager.scan")))
            .on_hover_text(t("host.manager.scan_tip"))
            .clicked()
        {
            app.plugins.start_scan(false);
        }
        if ui
            .add_enabled(!scanning, egui::Button::new(t("host.manager.rescan_all")))
            .on_hover_text(t("host.manager.rescan_all_tip"))
            .clicked()
        {
            app.plugins.start_scan(true);
        }
        let status = if scanning {
            t("host.manager.scanning")
        } else {
            tf(
                "host.manager.summary",
                &[("found", &found), ("blocked", &blocked)],
            )
        };
        ui.label(dim(status));
    });
    ui.add_space(grid::S2);
    ui.label(RichText::new(t("host.manager.folders")).text_style(design::strong()));
    let standard: Vec<std::path::PathBuf> = cantus_host::backends()
        .iter()
        .flat_map(|b| b.search_paths())
        .collect();
    for f in &standard {
        ui.horizontal(|ui| {
            ui.label(RichText::new(f.display().to_string()).text_style(design::mono_small()));
            ui.label(dim(t("host.manager.standard")));
        });
    }
    let mut remove = None;
    for f in &app.plugins.db.folders {
        ui.horizontal(|ui| {
            ui.label(RichText::new(f.display().to_string()).text_style(design::mono_small()));
            if ui.small_button(t("host.manager.remove")).clicked() {
                remove = Some(f.clone());
            }
        });
    }
    if let Some(f) = remove {
        app.plugins.remove_folder(&f);
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.fx.hosted.folder)
                .hint_text(t("host.manager.folder_hint"))
                .desired_width(grid::GRID * 64.0),
        );
        let typed = app.fx.hosted.folder.trim().to_string();
        if ui
            .add_enabled(!typed.is_empty(), egui::Button::new(t("host.manager.add")))
            .clicked()
        {
            app.plugins.add_folder(std::path::PathBuf::from(&typed));
            app.fx.hosted.folder.clear();
        }
        if ui.button(t("host.manager.browse")).clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                app.plugins.add_folder(dir);
            }
        }
    });
    // What the scans found.
    let found_list: Vec<PluginInfo> = app.plugins.db.plugins().into_iter().cloned().collect();
    ui.add_space(grid::S2);
    egui::CollapsingHeader::new(
        RichText::new(tf("host.manager.found", &[("n", &found_list.len())]))
            .text_style(design::strong()),
    )
    .id_salt("host-found")
    .default_open(true)
    .show(ui, |ui| {
        for p in &found_list {
            ui.horizontal(|ui| {
                ui.label(&p.name);
                badge(ui, cantus_host::to_project_format(p.format).badge());
                ui.label(dim(format!("{} · {}", p.vendor, p.version)));
                if p.kind != cantus_host::PluginKind::Effect {
                    ui.label(dim(t("host.manager.not_effect")));
                }
            });
        }
    });
    let blocked: Vec<(std::path::PathBuf, String)> = app
        .plugins
        .db
        .blocked()
        .into_iter()
        .map(|e| {
            let why = match &e.status {
                cantus_host::ScanStatus::Blocked(w) => w.clone(),
                _ => String::new(),
            };
            (e.path.clone(), why)
        })
        .collect();
    ui.add_space(grid::S2);
    ui.label(RichText::new(t("host.manager.blocked")).text_style(design::strong()));
    if blocked.is_empty() {
        ui.label(dim(t("host.manager.blocked_none")));
    } else {
        ui.label(dim(t("host.manager.blocked_hint")));
    }
    let mut retry = None;
    for (path, why) in &blocked {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(path.display().to_string())
                    .text_style(design::mono_small())
                    .color(pal.danger),
            );
            ui.label(dim(why.clone()));
            if ui
                .small_button(t("host.manager.retry"))
                .on_hover_text(t("host.manager.retry_tip"))
                .clicked()
            {
                retry = Some(path.clone());
            }
        });
    }
    if let Some(path) = retry {
        app.plugins.rescan_file(&path);
    }
    let failed = app.plugins.db.failed().len();
    if failed > 0 {
        ui.label(dim(tf("host.manager.failed", &[("n", &failed)])));
    }
}

// ---- screenshots ------------------------------------------------------------------------

/// `shoot plugin` / `shoot plugins`: the plug-in at `library` (the test plug-in, built from
/// source) read in this process into the database, beside a blocked file and a folder of
/// the user's; with `open_editor`, put on the first strip with a few values moved and its
/// editor open. The database is not written anywhere.
pub fn demo_setup(app: &mut CantusApp, library: &std::path::Path, open_editor: bool) {
    use cantus_host::{Backend, DbEntry, ScanStatus};
    app.plugins.db_path = None;
    let folder = std::env::temp_dir().join("cantus-shoot-plugins");
    let _ = std::fs::create_dir_all(&folder);
    let file = folder.join("Cantus Test Gain.clap");
    let _ = std::fs::copy(library, &file);
    let plugins = cantus_host::clap::ClapBackend
        .scan(&file)
        .unwrap_or_default();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
    app.plugins.db.put(DbEntry {
        path: file.clone(),
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    app.plugins.db.put(DbEntry {
        path: folder.join("Old Reverb 2009.clap"),
        format: cantus_host::Format::Clap,
        modified: 0,
        size: 0,
        status: ScanStatus::Blocked("crashed (exit code 0xc0000409)".into()),
        plugins: Vec::new(),
    });
    app.plugins
        .db
        .folders
        .push(std::path::PathBuf::from(r"D:\Audio\Plug-ins"));
    if !open_editor {
        return;
    }
    let Some(info) = app.plugins.db.effects().first().map(|p| (*p).clone()) else {
        return;
    };
    let Some(strip) = app.project().strips.first().map(|s| s.id) else {
        return;
    };
    app.add_plugin(strip, &info);
    app.sync_engine();
    let Some(effect) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effects.iter().rev().find(|e| e.spec.external.is_some()))
        .map(|e| e.id)
    else {
        return;
    };
    app.set_plugin_param(strip, effect, 0, -4.5, false);
    app.set_plugin_param(strip, effect, 2, 1.0, false);
    if let Some(key) = app.project().strips.get(1).map(|s| s.id) {
        app.exec(Command::SetEffectSidechain {
            strip,
            effect,
            source: Some(key),
        });
    }
    app.sync_engine();
    app.open_effect_editor(strip, effect);
}

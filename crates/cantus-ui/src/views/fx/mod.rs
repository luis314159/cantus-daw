//! The plug-in family's editors, and the frame they share.
//!
//! Every plug-in's editor is a module here that owns its own display (`body`), its small
//! read-only picture for the inspector (`preview`), its factory presets, and the demo set-up
//! the screenshot tool uses. What they share is in this file, so it is written once:
//!
//! - **The frame** — one window per open editor, and a header with the strip it sits on
//!   (colour and name), the presets menu, A/B, and the bypass. `research/fabfilter/
//!   family.md` rule 1: one frame, fixed positions.
//! - **A/B** kept by the host: switching is one command, so it is one step of the project's
//!   undo, like everything else (rule 3).
//! - **Presets**: the plug-in's factory list plus the user's own, saved as JSON under the
//!   app's data folder, per kind.
//! - **A parameter knob** bound to a key, with the house gestures and one undo step per
//!   gesture.
//! - **What the engine reports** for the open editor: the analyser's (input, output) pairs
//!   and the unit's telemetry (`Meters::fx`).
//!
//! The Aequalis and Comprimo editors predate this module and keep their own windows; they
//! use the shared [`ab_bar`] and [`presets_menu`] from here.

use crate::app::CantusApp;
use crate::design::{self, space as grid};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::{Command, EffectId, EffectKind, EffectSlot, EffectSpec, MergeKey, StripId};
use egui::{vec2, RichText, Sense};
use std::collections::{BTreeMap, HashMap, VecDeque};

pub mod deesser;
pub mod gate;
/// What an editor needs from whatever runs it: Cantus, or a plug-in host (ADR 0020).
pub mod host;
pub mod limiter;
pub mod multiband;
pub mod space;

/// How many recent (input, output) pairs of the analysed effect are kept for displays.
pub const PAIRS: usize = 16_384;

/// A preset: a label key (or a plain name, for the user's own) and the parameters it sets.
/// Parameters it does not name go back to their defaults.
pub type FactoryPreset = (&'static str, &'static [(&'static str, f32)]);

/// State of the plug-in editors behind this module.
#[derive(Debug, Default)]
pub struct FxEditors {
    /// The effect whose editor is open, and its kind.
    pub open: Option<(StripId, EffectId, EffectKind)>,
    /// Recent (input, output) mono pairs of the open effect, oldest first.
    pub pairs: VecDeque<(f32, f32)>,
    /// Telemetry injected by demos and tests, used instead of the engine's while set.
    pub injected: Option<cantus_engine::Telemetry>,
    /// The other side of A/B for each effect that has one: its parameters, and whether B
    /// is the one showing.
    ab: HashMap<EffectId, (BTreeMap<String, f32>, bool)>,
    /// The name being typed for a user preset.
    preset_name: String,
    pub limiter: limiter::State,
    pub space: space::State,
    pub multiband: multiband::State,
    pub deesser: deesser::State,
    pub gate: gate::State,
    /// Hosted plug-ins' editors and menus (`views::plugins`).
    pub hosted: crate::views::plugins::State,
}

impl FxEditors {
    /// Feed (input, output) pairs directly — what the engine's analyser tap does — for
    /// screenshots and tests with no engine. Interleaved: in, out, in, out…
    pub fn feed(&mut self, interleaved: &[f32]) {
        for p in interleaved.chunks_exact(2) {
            if self.pairs.len() == PAIRS {
                self.pairs.pop_front();
            }
            self.pairs.push_back((p[0], p[1]));
        }
    }

    /// Whether B is showing for this effect.
    pub fn on_b(&self, effect: EffectId) -> bool {
        self.ab.get(&effect).is_some_and(|(_, b)| *b)
    }
}

impl CantusApp {
    /// The open editor's telemetry: injected (demos, tests) or the engine's.
    pub fn fx_telemetry(&self) -> cantus_engine::Telemetry {
        self.fx.injected.unwrap_or_else(|| self.engine.meters().fx)
    }

    pub(crate) fn open_fx_editor(&mut self, strip: StripId, effect: EffectId, kind: EffectKind) {
        self.fx.open = Some((strip, effect, kind));
        self.fx.pairs.clear();
        self.engine.set_analyser(Some(effect));
    }

    pub fn close_fx_editor(&mut self) {
        if self.fx.open.take().is_some() {
            self.engine.set_analyser(None);
        }
    }

    pub(crate) fn pump_fx_pairs(&mut self, pairs: &[f32]) {
        if self.fx.open.is_some() {
            self.fx.feed(pairs);
        }
    }

    /// Set one parameter; a gesture merges frame to frame into one undo step.
    pub fn set_fx_param(
        &mut self,
        strip: StripId,
        effect: EffectId,
        key: &str,
        value: f32,
        gesture: bool,
    ) {
        let cmd = Command::SetEffectParam {
            strip,
            effect,
            key: key.to_string(),
            value,
        };
        if gesture {
            self.exec_merging(cmd, MergeKey::of(&("fx", effect.0, key)));
        } else {
            self.exec(cmd);
        }
    }

    /// Replace an effect's parameters with `params`, as one command: keys it names are
    /// set, every other stored key is cleared back to its default.
    pub fn apply_fx_params(
        &mut self,
        strip: StripId,
        effect: EffectId,
        params: &BTreeMap<String, f32>,
        label_key: &str,
    ) {
        let Some(slot) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .cloned()
        else {
            return;
        };
        let mut commands = Vec::new();
        for info in slot.spec.kind.params() {
            match params.get(info.key) {
                Some(v) => {
                    if slot.spec.params.get(info.key) != Some(v) {
                        commands.push(Command::SetEffectParam {
                            strip,
                            effect,
                            key: info.key.to_string(),
                            value: *v,
                        });
                    }
                }
                None => {
                    if slot.spec.kind.sparse() {
                        if slot.spec.params.contains_key(info.key) {
                            commands.push(Command::ClearEffectParam {
                                strip,
                                effect,
                                key: info.key.to_string(),
                            });
                        }
                    } else if slot.spec.get(info.key) != info.default {
                        commands.push(Command::SetEffectParam {
                            strip,
                            effect,
                            key: info.key.to_string(),
                            value: info.default,
                        });
                    }
                }
            }
        }
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: label_key.into(),
                commands,
            });
        }
    }

    /// Switch between A and B: the showing side is kept, the other one applied — one
    /// command, so one step of undo.
    pub fn toggle_ab(&mut self, strip: StripId, effect: EffectId) {
        let Some(current) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.params.clone())
        else {
            return;
        };
        let (other, on_b) = self
            .fx
            .ab
            .remove(&effect)
            .unwrap_or_else(|| (current.clone(), false));
        self.apply_fx_params(strip, effect, &other, "cmd.fx_ab");
        self.fx.ab.insert(effect, (current, !on_b));
    }

    /// Copy the showing side onto the other one.
    pub fn copy_ab(&mut self, effect: EffectId, params: &BTreeMap<String, f32>) {
        let on_b = self.fx.on_b(effect);
        self.fx.ab.insert(effect, (params.clone(), on_b));
    }
}

/// The folder a kind's user presets live in.
pub fn user_preset_dir(kind: EffectKind) -> std::path::PathBuf {
    crate::app::data_dir()
        .join("presets")
        .join(format!("{kind:?}").to_lowercase())
}

/// The user's presets for a kind, by name.
pub fn user_presets(kind: EffectKind) -> Vec<(String, BTreeMap<String, f32>)> {
    let mut out = Vec::new();
    if let Ok(dir) = std::fs::read_dir(user_preset_dir(kind)) {
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Ok(params) = serde_json::from_str::<BTreeMap<String, f32>>(&text) {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push((name, params));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Save the current parameters as a user preset.
pub fn save_user_preset(
    kind: EffectKind,
    name: &str,
    params: &BTreeMap<String, f32>,
) -> Result<(), String> {
    let clean: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return Err(t("preset.empty_name"));
    }
    let dir = user_preset_dir(kind);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(params).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{clean}.json")), text).map_err(|e| e.to_string())
}

/// A kind's factory presets.
pub fn factory_presets(kind: EffectKind) -> &'static [FactoryPreset] {
    match kind {
        EffectKind::Eq => crate::views::eq_editor::FACTORY_PRESETS,
        EffectKind::Compressor => crate::views::comp_editor::FACTORY_PRESETS,
        EffectKind::Limiter => limiter::FACTORY_PRESETS,
        EffectKind::Space => space::FACTORY_PRESETS,
        EffectKind::Multiband => multiband::FACTORY_PRESETS,
        EffectKind::DeEsser => deesser::FACTORY_PRESETS,
        EffectKind::Gate => gate::FACTORY_PRESETS,
        _ => &[],
    }
}

/// The presets menu: factory presets, the user's, and "save as…".
pub fn presets_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let kind = slot.spec.kind;
    ui.menu_button(t("preset.menu"), |ui| {
        let factory = factory_presets(kind);
        if !factory.is_empty() {
            ui.label(
                RichText::new(t("preset.factory"))
                    .text_style(design::caption())
                    .color(theme::p().text_dim),
            );
        }
        for (label, params) in factory {
            if ui.button(t(label)).clicked() {
                let map = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
                app.apply_fx_params(strip, effect, &map, "cmd.fx_preset");
                ui.close();
            }
        }
        let user = user_presets(kind);
        if !user.is_empty() {
            ui.separator();
            ui.label(
                RichText::new(t("preset.user"))
                    .text_style(design::caption())
                    .color(theme::p().text_dim),
            );
        }
        for (name, params) in user {
            if ui.button(&name).clicked() {
                app.apply_fx_params(strip, effect, &params, "cmd.fx_preset");
                ui.close();
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.fx.preset_name)
                    .hint_text(t("preset.name"))
                    .desired_width(grid::GRID * 36.0),
            );
            if ui.button(t("preset.save")).clicked() {
                match save_user_preset(kind, &app.fx.preset_name, &slot.spec.params) {
                    Ok(()) => {
                        app.set_status(tf("preset.saved", &[("name", &app.fx.preset_name)]));
                        app.fx.preset_name.clear();
                        ui.close();
                    }
                    Err(e) => app.set_error(e),
                }
            }
        });
    });
}

/// A and B, and "copy to the other side". Host-kept: switching is one undo step.
pub fn ab_bar(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let on_b = app.fx.on_b(effect);
    if ui
        .selectable_label(!on_b, "A")
        .on_hover_text(t("ab.tip"))
        .clicked()
        && on_b
    {
        app.toggle_ab(strip, effect);
    }
    if ui
        .selectable_label(on_b, "B")
        .on_hover_text(t("ab.tip"))
        .clicked()
        && !on_b
    {
        app.toggle_ab(strip, effect);
    }
    if ui
        .small_button(t("ab.copy"))
        .on_hover_text(t("ab.copy_tip"))
        .clicked()
    {
        app.copy_ab(effect, &slot.spec.params);
    }
}

/// A knob bound to one parameter of an effect: range, taper, unit and label come from the
/// parameter's own description; a drag is one undo step.
pub fn param_knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    spec: &EffectSpec,
    key: &str,
) -> egui::Response {
    let Some(info) = spec.kind.param(key) else {
        return ui.label(key);
    };
    let mut v = spec.get(key);
    let r = ui.add(
        Knob::new(&mut v, info.min..=info.max, info.default)
            .diameter(grid::GRID * 11.0)
            .log(info.logarithmic)
            .stepped(info.stepped)
            .unit(info.unit)
            .label(t(info.label_key)),
    );
    if r.changed() {
        app.set_fx_param(strip, effect, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    r
}

/// The window for the open editor, drawn every frame while one is open.
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((strip, effect, kind)) = app.fx.open else {
        return;
    };
    let Some(slot) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .cloned()
    else {
        app.close_fx_editor();
        return;
    };
    let strip_name = app.capture_label(cantus_engine::CaptureTarget::Strip(strip));
    let mut open = true;
    // A hosted plug-in is called by its own name.
    let name = slot
        .spec
        .external
        .as_deref()
        .map_or_else(|| t(kind.name_key()), |p| p.name.clone());
    egui::Window::new(format!("{name} \u{2014} {strip_name}"))
        .id(egui::Id::new(("fx-editor", effect.0)))
        .open(&mut open)
        .default_size([980.0, 620.0])
        .min_size([560.0, 400.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            header(app, ui, strip, effect, &slot);
            ui.add_space(grid::S1);
            match kind {
                EffectKind::Limiter => limiter::body(app, ui, strip, effect, &slot),
                EffectKind::Space => space::body(app, ui, strip, effect, &slot),
                EffectKind::Multiband => multiband::body(app, ui, strip, effect, &slot),
                EffectKind::DeEsser => deesser::body(app, ui, strip, effect, &slot),
                EffectKind::Gate => gate::body(app, ui, strip, effect, &slot),
                EffectKind::External => crate::views::plugins::body(app, ui, strip, effect, &slot),
                _ => {}
            }
        });
    ctx.request_repaint();
    if !open {
        app.close_fx_editor();
    }
}

/// The shared header: the strip it sits on, presets, A/B, bypass.
pub fn header(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        let (chip, _) = ui.allocate_exact_size(vec2(grid::S3, grid::S3), Sense::hover());
        let color = app
            .project()
            .tracks_on_strip(strip)
            .next()
            .map(|t| theme::track_color(t.color))
            .unwrap_or(pal.gold);
        ui.painter().rect_filled(chip, pal.corner_radius, color);
        ui.label(
            RichText::new(app.capture_label(cantus_engine::CaptureTarget::Strip(strip)))
                .text_style(design::strong()),
        );
        ui.separator();
        if slot.spec.kind == EffectKind::External {
            // Presets are saved states, and A/B switches whole states (`views::plugins`).
            crate::views::plugins::header_controls(app, ui, strip, effect, slot);
        } else {
            presets_menu(app, ui, strip, effect, slot);
            ab_bar(app, ui, strip, effect, slot);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut on = !slot.bypass;
            if ui
                .toggle_value(&mut on, t("eq.active"))
                .on_hover_text(t("effect.bypass"))
                .changed()
            {
                app.exec(Command::SetEffectBypass {
                    strip,
                    effect,
                    bypass: !on,
                });
            }
        });
    });
}

/// The inspector's picture of an effect with an editor behind this module.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    match spec.kind {
        EffectKind::Limiter => limiter::preview(ui, spec),
        EffectKind::Space => space::preview(ui, spec),
        EffectKind::Multiband => multiband::preview(ui, spec),
        EffectKind::DeEsser => deesser::preview(ui, spec),
        EffectKind::Gate => gate::preview(ui, spec),
        _ => ui.label(t(spec.kind.name_key())),
    }
}

/// `--editor <kind>`: the demo settings a kind's screenshot is taken with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId, kind: EffectKind) {
    match kind {
        EffectKind::Compressor => crate::views::comp_editor::demo_setup(app, strip, effect),
        EffectKind::Limiter => limiter::demo_setup(app, strip, effect),
        EffectKind::Space => space::demo_setup(app, strip, effect),
        EffectKind::Multiband => multiband::demo_setup(app, strip, effect),
        EffectKind::DeEsser => deesser::demo_setup(app, strip, effect),
        EffectKind::Gate => gate::demo_setup(app, strip, effect),
        EffectKind::Eq => crate::views::eq_editor::demo_setup(app, strip, effect),
        _ => {}
    }
}

/// For screenshots and tests with no engine: feed the open editor what a playing engine
/// would (pairs through [`FxEditors::feed`], telemetry through `FxEditors::injected`).
pub fn demo_feed(app: &mut CantusApp, kind: EffectKind) {
    match kind {
        EffectKind::Compressor => crate::views::comp_editor::demo_feed(app),
        EffectKind::Limiter => limiter::demo_feed(app),
        EffectKind::Space => space::demo_feed(app),
        EffectKind::Multiband => multiband::demo_feed(app),
        EffectKind::DeEsser => deesser::demo_feed(app),
        EffectKind::Gate => gate::demo_feed(app),
        EffectKind::Eq => crate::views::eq_editor::demo_feed(app),
        _ => {}
    }
}

/// The generic body a scaffolded editor starts with: a knob per parameter.
pub fn knobs_for_every_param(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    ui.horizontal_wrapped(|ui| {
        for info in slot.spec.kind.params() {
            param_knob(app, ui, strip, effect, &slot.spec, info.key);
        }
    });
}

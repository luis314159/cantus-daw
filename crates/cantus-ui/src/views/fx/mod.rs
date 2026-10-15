//! The plug-in family's editors, and the frame they share (ADR 0017, ADR 0020, ADR 0024 §8).
//!
//! Every plug-in's editor is a module here (Aequalis and Comprimo are `views::eq_editor`
//! and `views::comp_editor`) that owns its display, its small read-only picture for the
//! inspector (`preview`), its factory presets, and the demo set-up the screenshot tool
//! uses. What they share is written once:
//!
//! - **The device frame** ([`frame`]): the kit's shell — the header is the title bar, with
//!   the presets, A/B, the device's undo, the bypass, ⋯ and the size; the footer is the
//!   help line with the latency chip — wired to the editor's host.
//! - **Where editors go on screen**: [`open_window`], the one function that decides it.
//! - **The controls** ([`controls`]) and **the displays' paint** ([`canvas`]), all through
//!   the kit, at the device's scale and in its hue.
//! - **The host** ([`host`]): every editor asks [`host::FxHost`] for what it needs, so the
//!   same editor runs in Cantus and in the exported plug-ins' windows ([`Editor`]).
//! - **A/B** kept by the host: switching is one command, so it is one step of the project's
//!   undo, like everything else. **Presets**: the kind's factory list plus the user's own,
//!   saved as JSON under the app's data folder, per kind.
//! - **What the engine reports** for the open editor: the analyser's (input, output) pairs
//!   and the unit's telemetry (`Meters::fx`).

use crate::app::CantusApp;
use crate::i18n::t;
use cantus_core::{Command, EffectId, EffectKind, EffectSpec, MergeKey, StripId};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// Anulus, the ring modulator (round 28).
pub mod anulus;
/// What the displays paint with: tokens, and measured words on plates.
pub mod canvas;
/// Concentus, the chorus (Juno, Dimension) (round 28).
pub mod concentus;
/// The kit's controls at the device's scale and hue, bound to the host's parameters.
pub mod controls;
pub mod deesser;
/// Discrimen, the transient splitter (round 28).
pub mod discrimen;
/// Flexus, the through-zero flanger (round 28).
pub mod flexus;
/// Fluctus, the LFO Tool-style volume shaper (round 28).
pub mod fluctus;
/// The device frame: the kit's shell wired to the host, and where editors go on screen.
pub mod frame;
pub mod gate;
/// What an editor needs from whatever runs it: Cantus, or a plug-in host (ADR 0020).
pub mod host;
/// Impetus, the transient processor (round 28).
pub mod impetus;
pub mod limiter;
pub mod maximizer;
/// What the modulation family's editors share: the LFO card, the movement display (round 28).
pub mod modfx;
pub mod multiband;
/// Phasis, the phaser (round 28).
pub mod phasis;
/// Fervor, the saturator (ADR 0026).
pub mod saturator;
pub mod space;
/// Speculum, the analyser (Wave Candy / SPAN class) (round 28).
pub mod speculum;
pub mod stereo;
/// Tonsor, the clipper, with true peak (round 28).
pub mod tonsor;
/// Tonus, the real-time auto-tuner (round 28).
pub mod tonus;
/// Translatio, the frequency shifter (round 28).
pub mod translatio;
/// Tremor, tremolo and auto-pan (round 28).
pub mod tremor;
/// Vibrans, the vibrato (round 28).
pub mod vibrans;

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
    pub limiter: limiter::State,
    pub space: space::State,
    pub multiband: multiband::State,
    pub deesser: deesser::State,
    pub gate: gate::State,
    pub stereo: stereo::State,
    pub maximizer: maximizer::State,
    pub saturator: saturator::State,
    pub concentus: concentus::State,
    pub flexus: flexus::State,
    pub phasis: phasis::State,
    pub tremor: tremor::State,
    pub vibrans: vibrans::State,
    pub anulus: anulus::State,
    pub translatio: translatio::State,
    pub fluctus: fluctus::State,
    pub tonsor: tonsor::State,
    pub impetus: impetus::State,
    pub discrimen: discrimen::State,
    pub speculum: speculum::State,
    pub tonus: tonus::State,
    /// Hosted plug-ins' editors and menus (`views::plugins`).
    pub hosted: crate::views::plugins::State,
    /// The open editor's frame: its preset, its lists.
    pub chrome: frame::Chrome,
    /// Where the floating editor sits (every effect's editor opens where the last was).
    pub pos: Option<egui::Pos2>,
    /// The footer's chips, formatted when they change.
    pub chips: frame::ChipText,
    /// The open effect's parameters as the host read them, and the project revision they
    /// were read at: an editor at rest copies nothing (`host::DawHost::new`).
    pub(crate) spec_cache: Option<(u64, EffectId, EffectSpec)>,
    /// The step a device's own undo put on the redo list, and the revision after it: its
    /// redo is offered while nothing else has changed the project.
    pub(crate) redo_mark: Option<(EffectId, u64)>,
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
        let Some(slot) = self.project().effect(strip, effect).cloned() else {
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
            .effect(strip, effect)
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
        EffectKind::Stereo => stereo::FACTORY_PRESETS,
        EffectKind::Maximizer => maximizer::FACTORY_PRESETS,
        EffectKind::Saturator => saturator::FACTORY_PRESETS,
        EffectKind::Chorus => concentus::FACTORY_PRESETS,
        EffectKind::Flanger => flexus::FACTORY_PRESETS,
        EffectKind::Phaser => phasis::FACTORY_PRESETS,
        EffectKind::Tremolo => tremor::FACTORY_PRESETS,
        EffectKind::Vibrato => vibrans::FACTORY_PRESETS,
        EffectKind::RingMod => anulus::FACTORY_PRESETS,
        EffectKind::FreqShift => translatio::FACTORY_PRESETS,
        EffectKind::VolumeShaper => fluctus::FACTORY_PRESETS,
        EffectKind::Clipper => tonsor::FACTORY_PRESETS,
        EffectKind::Transient => impetus::FACTORY_PRESETS,
        EffectKind::TransientSplit => discrimen::FACTORY_PRESETS,
        EffectKind::Analyzer => speculum::FACTORY_PRESETS,
        EffectKind::AutoTune => tonus::FACTORY_PRESETS,
        _ => &[],
    }
}

/// **Where every effect editor goes on screen** (ADR 0024 §5, §8) — the one function
/// that decides it. Today each editor is a floating window of its own
/// ([`frame::place`]), sized by its kind's design and scale, with no egui title bar;
/// when the workspace's docks exist, this is the line that makes it a dock tab, and no
/// editor changes.
pub fn open_window(
    ctx: &egui::Context,
    id: egui::Id,
    pos: &mut Option<egui::Pos2>,
    kind: EffectKind,
    add: impl FnOnce(&mut egui::Ui) -> frame::Out,
) -> bool {
    open_window_sized(ctx, id, pos, kind, frame::designed_size(kind), add)
}

/// [`open_window`] at a size of the editor's own at 100 % (a hosted plug-in's generic
/// editor fits its parameters).
pub fn open_window_sized(
    ctx: &egui::Context,
    id: egui::Id,
    pos: &mut Option<egui::Pos2>,
    kind: EffectKind,
    designed: egui::Vec2,
    add: impl FnOnce(&mut egui::Ui) -> frame::Out,
) -> bool {
    use crate::views::workspace_device as device;
    // A device of the workspace (round 20): closed from outside (Esc, the tray), kept
    // open but not drawn while minimised, over the workspace while maximised, and back
    // where it floated last.
    let key = id.value();
    let st = device::register_area(ctx, key, frame::hue_of(kind), || {
        crate::i18n::t(kind.name_key())
    });
    if st.closed {
        return false;
    }
    if st.minimised {
        return true;
    }
    if st.raise {
        // *Open the editor* on an editor already open (round 27, ADR 0051): above the
        // other windows. The area is `frame::place`'s, at the middle order.
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    }
    let size = designed * frame::shell_of(kind).scale(ctx);
    let (at, size) = match st.maximised {
        Some(area) => (area.min, area.size()),
        None => (frame::position(ctx, st.place.or(*pos), size), size),
    };
    let mut out = frame::Out::default();
    frame::place(ctx, id, at, size, |ui| out = add(ui));
    let whole = egui::Rect::from_min_size(at, size);
    let header = egui::Rect::from_min_max(at, egui::pos2(whole.right(), out.body.top()));
    device::note_shown(ctx, key, whole, header, out.window);
    if st.maximised.is_none() {
        let to = at + out.moved;
        *pos = Some(to);
        device::remember_place(ctx, key, egui::Rect::from_min_size(to, size));
    }
    // Meters and histories move with the audio: at most 60 frames a second.
    ctx.request_repaint_after(std::time::Duration::from_millis(16));
    !out.close
}

/// The device id of an effect's editor window: its key among the workspace's devices.
pub fn device_id(effect: cantus_core::EffectId) -> egui::Id {
    egui::Id::new(("fx-editor", effect.0))
}

/// One of the effect editors that run on any [`host::FxHost`], with its state — for a
/// host that shows one editor on its own: the exported plug-in's window (ADR 0020).
pub enum Editor {
    Gate(Box<gate::State>),
    Stereo(Box<stereo::State>),
    Multiband(Box<multiband::State>),
    Space(Box<space::State>),
    Limiter(Box<limiter::State>),
    DeEsser(Box<deesser::State>),
    Comp(Box<crate::views::comp_editor::State>),
    Eq(Box<crate::views::eq_editor::EqEditor>),
    Maximizer(Box<maximizer::State>),
    Saturator(Box<saturator::State>),
    Concentus(Box<concentus::State>),
    Flexus(Box<flexus::State>),
    Phasis(Box<phasis::State>),
    Tremor(Box<tremor::State>),
    Vibrans(Box<vibrans::State>),
    Anulus(Box<anulus::State>),
    Translatio(Box<translatio::State>),
    Fluctus(Box<fluctus::State>),
    Tonsor(Box<tonsor::State>),
    Impetus(Box<impetus::State>),
    Discrimen(Box<discrimen::State>),
    Speculum(Box<speculum::State>),
    Tonus(Box<tonus::State>),
}

impl Editor {
    /// The editor of `kind`, if it runs on [`host::FxHost`].
    pub fn of(kind: EffectKind) -> Option<Editor> {
        match kind {
            EffectKind::Gate => Some(Editor::Gate(Box::default())),
            EffectKind::Stereo => Some(Editor::Stereo(Box::default())),
            EffectKind::Multiband => Some(Editor::Multiband(Box::default())),
            EffectKind::Space => Some(Editor::Space(Box::default())),
            EffectKind::Limiter => Some(Editor::Limiter(Box::default())),
            EffectKind::DeEsser => Some(Editor::DeEsser(Box::default())),
            EffectKind::Compressor => Some(Editor::Comp(Box::default())),
            EffectKind::Eq => Some(Editor::Eq(Box::default())),
            EffectKind::Maximizer => Some(Editor::Maximizer(Box::default())),
            EffectKind::Saturator => Some(Editor::Saturator(Box::default())),
            EffectKind::Chorus => Some(Editor::Concentus(Box::default())),
            EffectKind::Flanger => Some(Editor::Flexus(Box::default())),
            EffectKind::Phaser => Some(Editor::Phasis(Box::default())),
            EffectKind::Tremolo => Some(Editor::Tremor(Box::default())),
            EffectKind::Vibrato => Some(Editor::Vibrans(Box::default())),
            EffectKind::RingMod => Some(Editor::Anulus(Box::default())),
            EffectKind::FreqShift => Some(Editor::Translatio(Box::default())),
            EffectKind::VolumeShaper => Some(Editor::Fluctus(Box::default())),
            EffectKind::Clipper => Some(Editor::Tonsor(Box::default())),
            EffectKind::Transient => Some(Editor::Impetus(Box::default())),
            EffectKind::TransientSplit => Some(Editor::Discrimen(Box::default())),
            EffectKind::Analyzer => Some(Editor::Speculum(Box::default())),
            EffectKind::AutoTune => Some(Editor::Tonus(Box::default())),
            _ => None,
        }
    }

    /// The editor in its device frame, filling `ui`.
    pub fn show(&mut self, host: &mut dyn host::FxHost, ui: &mut egui::Ui) -> frame::Out {
        match self {
            Editor::Gate(st) => gate::editor(host, st, ui),
            Editor::Stereo(st) => stereo::editor(host, st, ui),
            Editor::Multiband(st) => multiband::editor(host, st, ui),
            Editor::Space(st) => space::editor(host, st, ui),
            Editor::Limiter(st) => limiter::editor(host, st, ui),
            Editor::DeEsser(st) => deesser::editor(host, st, ui),
            Editor::Comp(st) => crate::views::comp_editor::editor(host, st, ui),
            Editor::Eq(st) => crate::views::eq_editor::editor(host, st, ui),
            Editor::Maximizer(st) => maximizer::editor(host, st, ui),
            Editor::Saturator(st) => saturator::editor(host, st, ui),
            Editor::Concentus(st) => concentus::editor(host, st, ui),
            Editor::Flexus(st) => flexus::editor(host, st, ui),
            Editor::Phasis(st) => phasis::editor(host, st, ui),
            Editor::Tremor(st) => tremor::editor(host, st, ui),
            Editor::Vibrans(st) => vibrans::editor(host, st, ui),
            Editor::Anulus(st) => anulus::editor(host, st, ui),
            Editor::Translatio(st) => translatio::editor(host, st, ui),
            Editor::Fluctus(st) => fluctus::editor(host, st, ui),
            Editor::Tonsor(st) => tonsor::editor(host, st, ui),
            Editor::Impetus(st) => impetus::editor(host, st, ui),
            Editor::Discrimen(st) => discrimen::editor(host, st, ui),
            Editor::Speculum(st) => speculum::editor(host, st, ui),
            Editor::Tonus(st) => tonus::editor(host, st, ui),
        }
    }
}

/// The window of the open editor (Porta, Terminus, Spatium, Multiplex, Sibilus, a hosted
/// plug-in), drawn every frame while one is open.
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((strip, effect, kind)) = app.fx.open else {
        return;
    };
    if app.project().effect(strip, effect).is_none() {
        app.close_fx_editor();
        return;
    }
    let open = match kind {
        EffectKind::Gate
        | EffectKind::Multiband
        | EffectKind::Space
        | EffectKind::Limiter
        | EffectKind::DeEsser
        | EffectKind::Stereo
        | EffectKind::Maximizer
        | EffectKind::Saturator
        | EffectKind::Chorus
        | EffectKind::Flanger
        | EffectKind::Phaser
        | EffectKind::Tremolo
        | EffectKind::Vibrato
        | EffectKind::RingMod
        | EffectKind::FreqShift
        | EffectKind::VolumeShaper
        | EffectKind::Clipper
        | EffectKind::Transient
        | EffectKind::TransientSplit
        | EffectKind::Analyzer
        | EffectKind::AutoTune
        | EffectKind::External => {
            let mut pos = app.fx.pos;
            let mut designed = frame::designed_size(kind);
            if let Some(h) = app
                .fx
                .hosted
                .editor
                .wanted
                .filter(|_| kind == EffectKind::External)
            {
                designed.y = h;
            }
            let open = open_window_sized(
                ctx,
                egui::Id::new(("fx-editor", effect.0)),
                &mut pos,
                kind,
                designed,
                |ui| match kind {
                    EffectKind::Gate => gate::framed(app, ui, strip, effect),
                    EffectKind::Multiband => multiband::framed(app, ui, strip, effect),
                    EffectKind::Space => space::framed(app, ui, strip, effect),
                    EffectKind::Limiter => limiter::framed(app, ui, strip, effect),
                    EffectKind::DeEsser => deesser::framed(app, ui, strip, effect),
                    EffectKind::Stereo => stereo::framed(app, ui, strip, effect),
                    EffectKind::Maximizer => maximizer::framed(app, ui, strip, effect),
                    EffectKind::Saturator => saturator::framed(app, ui, strip, effect),
                    EffectKind::Chorus => concentus::framed(app, ui, strip, effect),
                    EffectKind::Flanger => flexus::framed(app, ui, strip, effect),
                    EffectKind::Phaser => phasis::framed(app, ui, strip, effect),
                    EffectKind::Tremolo => tremor::framed(app, ui, strip, effect),
                    EffectKind::Vibrato => vibrans::framed(app, ui, strip, effect),
                    EffectKind::RingMod => anulus::framed(app, ui, strip, effect),
                    EffectKind::FreqShift => translatio::framed(app, ui, strip, effect),
                    EffectKind::VolumeShaper => fluctus::framed(app, ui, strip, effect),
                    EffectKind::Clipper => tonsor::framed(app, ui, strip, effect),
                    EffectKind::Transient => impetus::framed(app, ui, strip, effect),
                    EffectKind::TransientSplit => discrimen::framed(app, ui, strip, effect),
                    EffectKind::Analyzer => speculum::framed(app, ui, strip, effect),
                    EffectKind::AutoTune => tonus::framed(app, ui, strip, effect),
                    _ => crate::views::plugins::framed(app, ui, strip, effect),
                },
            );
            app.fx.pos = pos;
            open
        }
        // Every other kind has no editor of its own (Aequalis and Comprimo have their
        // windows in `views::eq_editor` and `views::comp_editor`).
        _ => false,
    };
    if !open {
        app.close_fx_editor();
    }
}

/// The inspector's picture of an effect with an editor behind this module.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    match spec.kind {
        EffectKind::Limiter => limiter::preview(ui, spec),
        EffectKind::Space => space::preview(ui, spec),
        EffectKind::Multiband => multiband::preview(ui, spec),
        EffectKind::DeEsser => deesser::preview(ui, spec),
        EffectKind::Gate => gate::preview(ui, spec),
        EffectKind::Stereo => stereo::preview(ui, spec),
        EffectKind::Maximizer => maximizer::preview(ui, spec),
        EffectKind::Saturator => saturator::preview(ui, spec),
        EffectKind::Chorus => concentus::preview(ui, spec),
        EffectKind::Flanger => flexus::preview(ui, spec),
        EffectKind::Phaser => phasis::preview(ui, spec),
        EffectKind::Tremolo => tremor::preview(ui, spec),
        EffectKind::Vibrato => vibrans::preview(ui, spec),
        EffectKind::RingMod => anulus::preview(ui, spec),
        EffectKind::FreqShift => translatio::preview(ui, spec),
        EffectKind::VolumeShaper => fluctus::preview(ui, spec),
        EffectKind::Clipper => tonsor::preview(ui, spec),
        EffectKind::Transient => impetus::preview(ui, spec),
        EffectKind::TransientSplit => discrimen::preview(ui, spec),
        EffectKind::Analyzer => speculum::preview(ui, spec),
        EffectKind::AutoTune => tonus::preview(ui, spec),
        _ => {
            let s = crate::kit::Style::current();
            let name = frame::tr(spec.kind.name_key());
            controls::readout(ui, &s, name, false, name, ui.available_width())
        }
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
        EffectKind::Stereo => stereo::demo_setup(app, strip, effect),
        EffectKind::Maximizer => maximizer::demo_setup(app, strip, effect),
        EffectKind::Saturator => saturator::demo_setup(app, strip, effect),
        EffectKind::Chorus => concentus::demo_setup(app, strip, effect),
        EffectKind::Flanger => flexus::demo_setup(app, strip, effect),
        EffectKind::Phaser => phasis::demo_setup(app, strip, effect),
        EffectKind::Tremolo => tremor::demo_setup(app, strip, effect),
        EffectKind::Vibrato => vibrans::demo_setup(app, strip, effect),
        EffectKind::RingMod => anulus::demo_setup(app, strip, effect),
        EffectKind::FreqShift => translatio::demo_setup(app, strip, effect),
        EffectKind::VolumeShaper => fluctus::demo_setup(app, strip, effect),
        EffectKind::Clipper => tonsor::demo_setup(app, strip, effect),
        EffectKind::Transient => impetus::demo_setup(app, strip, effect),
        EffectKind::TransientSplit => discrimen::demo_setup(app, strip, effect),
        EffectKind::Analyzer => speculum::demo_setup(app, strip, effect),
        EffectKind::AutoTune => tonus::demo_setup(app, strip, effect),
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
        EffectKind::Stereo => stereo::demo_feed(app),
        EffectKind::Maximizer => maximizer::demo_feed(app),
        EffectKind::Saturator => saturator::demo_feed(app),
        EffectKind::Chorus => concentus::demo_feed(app),
        EffectKind::Flanger => flexus::demo_feed(app),
        EffectKind::Phaser => phasis::demo_feed(app),
        EffectKind::Tremolo => tremor::demo_feed(app),
        EffectKind::Vibrato => vibrans::demo_feed(app),
        EffectKind::RingMod => anulus::demo_feed(app),
        EffectKind::FreqShift => translatio::demo_feed(app),
        EffectKind::VolumeShaper => fluctus::demo_feed(app),
        EffectKind::Clipper => tonsor::demo_feed(app),
        EffectKind::Transient => impetus::demo_feed(app),
        EffectKind::TransientSplit => discrimen::demo_feed(app),
        EffectKind::Analyzer => speculum::demo_feed(app),
        EffectKind::AutoTune => tonus::demo_feed(app),
        EffectKind::Eq => crate::views::eq_editor::demo_feed(app),
        _ => {}
    }
}

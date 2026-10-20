//! MIDI learn and controller mapping in the interface (round 29, ADR 0062).
//!
//! What lives here: the controller tap's interface half and its state (the per-mapping
//! takeover, the gesture, the activity), the user's **global map** (`controllers.json`
//! beside `keymap.toml`, read in the boot's Files stage), the learn mode and learning, and
//! the verbs the rest of the interface calls — [`CantusApp::learn_target`],
//! [`CantusApp::forget_target`], [`CantusApp::toggle_learn_mode`],
//! [`CantusApp::link_last_tweaked`]. Each frame [`CantusApp::controller_input`] reads what the
//! tap heard (`controllers_input.rs`), learns (`controllers_learn.rs`) and sends feedback
//! (`controllers_feedback.rs`).
//!
//! **Where a learnt mapping goes**: a target that means the same in every project (the
//! transport's actions, the tempo, the master, *the selected track*, *the focused device*)
//! to the global map, so play/stop works in every song; anything else to the project (a
//! command, one undo step). A message a project mapping takes is not looked up in the global
//! map: **the project comes first**.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::controller::{
    Control, ControllerMap, DeskParam, Focus, MapMode, Mapping, MappingId, MappingTarget,
    MidiSource, Takeover,
};
use cantus_core::{AutoTarget, Command, InstrumentSpec};
use cantus_engine::midi::{Heard, MidiOutputs, MidiTap};
use cantus_engine::MidiEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Which map a mapping is in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    /// The user's, in every project.
    Global,
    /// This project's.
    Project,
}

/// A mapping's key in the interface's state.
pub type Key = (Scope, MappingId);

/// A 14-bit pair's state on one port, channel and number: its coarse value, and whether its
/// fine half has ever come (a controller in 7-bit mode never sends it).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Wide {
    pub msb: u8,
    pub lsb_seen: bool,
}

/// A target armed to learn the next hardware control moved.
#[derive(Clone, Debug, PartialEq)]
pub struct Learning {
    pub target: MappingTarget,
    pub scope: Scope,
    /// Learnt again: the mapping whose source is replaced.
    pub replace: Option<MappingId>,
    /// What the control sent so far (its port and channel are the first message's).
    pub heard: Vec<(Control, u16)>,
    pub source: Option<(u8, u8)>,
    /// When the first message came.
    pub first: Option<Instant>,
    /// When it was armed: an armed learn that hears nothing is dropped after
    /// `controllers_learn::LEARN_TIMEOUT`, so a forgotten learn does not eat the next note.
    pub armed_at: Instant,
}

/// The controllers' state in the interface.
#[derive(Default)]
pub struct ControllerUi {
    /// The tap the MIDI ports tee into (set on the boot path; kept across rescans).
    pub tap: MidiTap,
    /// The user's global map, and its file.
    pub global: ControllerMap,
    pub global_path: Option<PathBuf>,
    /// Ableton's MIDI Map mode: every learnable control outlined, a click arms it.
    pub learn_mode: bool,
    pub learning: Option<Learning>,
    /// Per mapping: takeover and toggles (core's arithmetic state).
    pub(crate) states: HashMap<Key, cantus_core::controller::DriveState>,
    /// When each mapping last moved something: the mappings view's activity dot.
    pub activity: HashMap<Key, Instant>,
    /// The gesture going on: the targets it moved, its undo key, its last message.
    pub gesture: crate::controllers_input::Gesture,
    /// The filter last published, and what it was made from: the project's revision, the
    /// global map, the ports.
    /// The project's revision, the loaded hosted parameters' signature, the global map, the
    /// ports.
    pub(crate) published: Option<(u64, u64, ControllerMap, Vec<String>)>,
    /// MIDI out for feedback, and the value last sent per mapping.
    pub outputs: MidiOutputs,
    pub sent: HashMap<Key, u16>,
    /// Feedback just sent, by port: what comes back within a moment is its echo.
    pub(crate) echoes: Vec<(u8, [u8; 3], Instant)>,
    /// Feedback written here instead of to MIDI out (the tests' sink).
    pub feedback_log: Option<Vec<(String, Vec<u8>)>>,
    /// The coarse and fine halves of 14-bit controllers, per port, channel and number.
    pub(crate) wide: HashMap<(u8, u8, u8), Wide>,
    /// A frame's messages (kept to reuse the allocation).
    pub(crate) buf: Vec<Heard>,
    /// The mappings view's state.
    pub view: crate::views::controllers::ViewState,
}

impl std::fmt::Debug for ControllerUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControllerUi")
            .field("global", &self.global.mappings.len())
            .field("learn_mode", &self.learn_mode)
            .finish()
    }
}

impl ControllerUi {
    /// The ports were opened again (a rescan, an audio restart): their indices may have
    /// moved, the outputs are closed, the filter must be published again.
    pub fn reopened(&mut self) {
        self.published = None;
        self.outputs.reset();
        self.sent.clear();
        self.echoes.clear();
        self.wide.clear();
        self.states.clear();
    }
}

/// A tap that wakes this window when a mapped message comes (the boot path's).
pub fn waking_tap(ctx: &egui::Context) -> MidiTap {
    let ctx = ctx.clone();
    MidiTap::new(Some(std::sync::Arc::new(move || ctx.request_repaint())))
}

// ---- the global map's file ----------------------------------------------------------------

/// The global map's file: `controllers.json` beside `keymap.toml`.
pub fn global_path(dir: &Path) -> PathBuf {
    dir.join("controllers.json")
}

/// The file's format version.
pub const GLOBAL_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct GlobalFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    mappings: Vec<serde_json::Value>,
}

/// Read the global map: absent or unreadable, an empty one; a mapping that does not parse,
/// is malformed or names a target that is not portable is left out (logged), so one bad
/// line never loses the rest.
pub fn load_global(path: &Path) -> ControllerMap {
    load_global_checked(path).0
}

/// [`load_global`], and whether the file was damaged: then it is kept beside, as
/// `controllers.json.bad`, before anything can write over it (review 1, #31).
pub fn load_global_checked(path: &Path) -> (ControllerMap, bool) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (ControllerMap::default(), false);
    };
    let file: GlobalFile = match serde_json::from_str(&text) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("controllers: cannot read {}: {e}", path.display());
            let _ = std::fs::rename(path, path.with_extension("json.bad"));
            return (ControllerMap::default(), true);
        }
    };
    let mut map = ControllerMap::default();
    for v in file.mappings {
        match serde_json::from_value::<Mapping>(v) {
            Ok(m) if m.is_valid() && m.target.is_portable() && map.get(m.id).is_none() => {
                map.mappings.push(m)
            }
            Ok(m) => log::warn!("controllers: mapping {} left out", m.id.0),
            Err(e) => log::warn!("controllers: a mapping left out: {e}"),
        }
    }
    (map, false)
}

/// Write the global map.
pub fn save_global(path: &Path, map: &ControllerMap) -> std::io::Result<()> {
    let file = GlobalFile {
        version: GLOBAL_VERSION,
        mappings: map
            .mappings
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Through a file beside, flushed, then renamed: a crash mid-write leaves the old map.
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write as _;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

// ---- words --------------------------------------------------------------------------------

/// A note's name, as the piano roll writes it (C4 is 60).
pub fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[usize::from(n % 12)], i32::from(n / 12) - 1)
}

/// The control in words: "CC 74", "14-bit CC 7", "Note C2", "Pitch bend"…
pub fn control_text(c: Control) -> String {
    match c {
        Control::Cc(n) => tf("ctl.src.cc", &[("n", &n)]),
        Control::Cc14(n) => tf("ctl.src.cc14", &[("n", &n)]),
        Control::Note(n) => tf("ctl.src.note", &[("n", &note_name(n))]),
        Control::PitchBend => t("ctl.src.bend"),
        Control::Program(n) => tf("ctl.src.program", &[("n", &(u32::from(n) + 1))]),
        Control::ChannelPressure => t("ctl.src.pressure"),
        Control::PolyPressure(n) => tf("ctl.src.key_pressure", &[("n", &note_name(n))]),
    }
}

/// The channel in words: "Ch 1" or "Any channel".
pub fn channel_text(c: Option<u8>) -> String {
    match c {
        Some(c) => tf("ctl.src.channel", &[("n", &(u32::from(c) + 1))]),
        None => t("ctl.src.any_channel"),
    }
}

/// The port in words: its name (with its ordinal when two share it), or "Any port".
pub fn port_text(p: &cantus_core::controller::PortRef) -> String {
    if p.is_any() {
        t("ctl.src.any_port")
    } else if p.ordinal > 0 {
        format!("{} ({})", p.name, u32::from(p.ordinal) + 1)
    } else {
        p.name.clone()
    }
}

/// The mode in words.
pub fn mode_text(m: MapMode) -> String {
    use cantus_core::controller::Encoding as E;
    t(match m {
        MapMode::Absolute => "ctl.mode.absolute",
        MapMode::Relative(E::TwosComplement) => "ctl.mode.rel_twos",
        MapMode::Relative(E::BinaryOffset) => "ctl.mode.rel_offset",
        MapMode::Relative(E::SignBit) => "ctl.mode.rel_sign",
        MapMode::Toggle => "ctl.mode.toggle",
        MapMode::Momentary => "ctl.mode.momentary",
        MapMode::Trigger => "ctl.mode.trigger",
    })
}

/// The takeover in words.
pub fn takeover_text(k: Takeover) -> String {
    t(match k {
        Takeover::Jump => "ctl.takeover.jump",
        Takeover::Pickup => "ctl.takeover.pickup",
        Takeover::Scale => "ctl.takeover.scale",
    })
}

fn desk_text(p: DeskParam) -> String {
    t(match p {
        DeskParam::Volume => "ctl.desk.volume",
        DeskParam::Pan => "ctl.desk.pan",
        DeskParam::Mute => "ctl.desk.mute",
        DeskParam::Solo => "ctl.desk.solo",
        DeskParam::Arm => "ctl.desk.arm",
    })
}

/// The source in words, whole: "CC 74 · Ch 1 · Keystation 49".
pub fn source_text(s: &MidiSource) -> String {
    format!(
        "{} \u{b7} {} \u{b7} {}",
        control_text(s.control),
        channel_text(s.channel),
        port_text(&s.port)
    )
}

// ---- the verbs ----------------------------------------------------------------------------

impl CantusApp {
    /// A target in words, for the mappings view, the status line and the menus.
    pub fn mapping_target_name(&self, target: &MappingTarget) -> String {
        let p = self.project();
        let track = |id: cantus_core::TrackId| {
            p.track(id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| t("auto.target.missing"))
        };
        let strip = |id: cantus_core::StripId| {
            crate::automation::target_parts(p, &AutoTarget::StripGain(id)).0
        };
        let effect = |id: cantus_core::EffectId| {
            p.effect_host(id)
                .and_then(|h| p.effect(h, id))
                .map(|e| match e.spec.external.as_deref() {
                    Some(x) => x.name.clone(),
                    None => t(e.spec.kind.name_key()),
                })
                .unwrap_or_else(|| t("auto.target.missing"))
        };
        let two = |a: String, b: String| format!("{a} \u{b7} {b}");
        match target {
            MappingTarget::Param(a) => self.target_name(a),
            MappingTarget::TrackMute(id) => two(track(*id), t("ctl.desk.mute")),
            MappingTarget::TrackSolo(id) => two(track(*id), t("ctl.desk.solo")),
            MappingTarget::TrackArm(id) => two(track(*id), t("ctl.desk.arm")),
            MappingTarget::StripMute(id) => two(strip(*id), t("ctl.desk.mute")),
            MappingTarget::StripSolo(id) => two(strip(*id), t("ctl.desk.solo")),
            MappingTarget::EffectBypass(id) => two(effect(*id), t("ctl.target.bypass")),
            MappingTarget::EffectMix(id) => two(effect(*id), t("ctl.target.mix")),
            MappingTarget::Tempo => t("ctl.target.tempo"),
            MappingTarget::Action(id) => match crate::keymap::Action::from_id(id) {
                Some(a) => two(t("ctl.target.action"), t(&a.label_key())),
                None => two(t("ctl.target.action"), id.clone()),
            },
            MappingTarget::SelectedTrack(d) => two(t("ctl.target.selected_track"), desk_text(*d)),
            MappingTarget::SelectedStrip(d) => two(t("ctl.target.selected_strip"), desk_text(*d)),
            MappingTarget::DeviceParam(n) => {
                tf("ctl.target.device_param", &[("n", &(u32::from(*n) + 1))])
            }
        }
    }

    /// The map of `scope`.
    pub fn controller_map(&self, scope: Scope) -> &ControllerMap {
        match scope {
            Scope::Global => &self.controllers.global,
            Scope::Project => &self.project().controllers,
        }
    }

    /// The mappings that drive `target`, in either map (the project's first).
    pub fn mappings_of(&self, target: &MappingTarget) -> Vec<(Scope, Mapping)> {
        let mut out: Vec<(Scope, Mapping)> = self
            .project()
            .controllers
            .of_target(target)
            .map(|m| (Scope::Project, m.clone()))
            .collect();
        out.extend(
            self.controllers
                .global
                .of_target(target)
                .map(|m| (Scope::Global, m.clone())),
        );
        out
    }

    /// Whether a controller drives `target`.
    pub fn is_mapped(&self, target: &MappingTarget) -> bool {
        self.project()
            .controllers
            .of_target(target)
            .next()
            .is_some()
            || self.controllers.global.of_target(target).next().is_some()
    }

    /// What a control bound to `target` shows of MIDI learn (the kit's offer).
    pub fn learn_offer(&self, target: &MappingTarget) -> crate::kit::automate::LearnOffer {
        crate::kit::automate::LearnOffer {
            mapped: self.is_mapped(target),
            mode: self.controllers.learn_mode,
            armed: self
                .controllers
                .learning
                .as_ref()
                .is_some_and(|l| &l.target == target),
        }
    }

    /// The map a learnt `target` goes to (the module's rule).
    pub fn scope_for(target: &MappingTarget) -> Scope {
        if target.is_portable() {
            Scope::Global
        } else {
            Scope::Project
        }
    }

    /// Arm `target`: the next hardware control moved is mapped to it (learnt again when it
    /// is mapped already: its first mapping's source is replaced). Says so on the status
    /// line; Esc, or arming it again, lets go.
    pub fn learn_target(&mut self, target: MappingTarget) {
        if self
            .controllers
            .learning
            .as_ref()
            .is_some_and(|l| l.target == target)
        {
            self.cancel_learning();
            return;
        }
        // A key that is set, never automated (Chorda's tuning, ADR 0059): not learnt
        // either — a controller would record automation on it.
        if let MappingTarget::Param(AutoTarget::Instrument { track, key }) = &target {
            let kind = self
                .project()
                .track(*track)
                .and_then(|t| t.instrument.native())
                .map(|n| n.kind);
            if kind.is_some_and(|k| !k.automatable(key)) {
                self.set_error(t("chorda.not_automatable"));
                return;
            }
        }
        let scope = Self::scope_for(&target);
        let replace = self
            .controller_map(scope)
            .of_target(&target)
            .next()
            .map(|m| m.id);
        let name = self.mapping_target_name(&target);
        self.controllers.learning = Some(Learning {
            target,
            scope,
            replace,
            heard: Vec::new(),
            source: None,
            first: None,
            armed_at: Instant::now(),
        });
        self.controllers.tap.filter().set_learning(true);
        self.set_status(tf("ctl.status.armed", &[("name", &name)]));
    }

    /// Stop waiting for a control.
    pub fn cancel_learning(&mut self) {
        if self.controllers.learning.take().is_some() {
            self.set_status(t("ctl.status.cancelled"));
        }
        self.controllers.tap.filter().set_learning(false);
    }

    /// Take every mapping of `target` away (the project's: one undo step).
    pub fn forget_target(&mut self, target: &MappingTarget) {
        let name = self.mapping_target_name(target);
        self.forget_targets(std::slice::from_ref(target), &name);
    }

    /// Take every mapping of every target in `targets` away: the project's in one undo
    /// step, the global ones saved once, one status naming `what` (review 1, #51).
    pub fn forget_targets(&mut self, targets: &[MappingTarget], what: &str) {
        let project: Vec<MappingId> = self
            .project()
            .controllers
            .mappings
            .iter()
            .filter(|m| targets.contains(&m.target))
            .map(|m| m.id)
            .collect();
        let global = self
            .controllers
            .global
            .mappings
            .iter()
            .filter(|m| targets.contains(&m.target))
            .count();
        if !project.is_empty() {
            let commands: Vec<Command> = project
                .into_iter()
                .map(|id| Command::RemoveMapping { id })
                .collect();
            let cmd = if commands.len() == 1 {
                commands.into_iter().next().expect("one")
            } else {
                Command::Batch {
                    label_key: "cmd.controllers.remove".into(),
                    commands,
                }
            };
            self.exec(cmd);
        }
        if global > 0 {
            self.controllers
                .global
                .mappings
                .retain(|m| !targets.contains(&m.target));
            self.store_global_map();
        }
        self.set_status(tf("ctl.status.forgot", &[("name", &what)]));
    }

    /// The learn mode on or off (Ableton's MIDI Map mode). Off lets go of a learn in
    /// progress.
    pub fn toggle_learn_mode(&mut self) {
        self.controllers.learn_mode = !self.controllers.learn_mode;
        if self.controllers.learn_mode {
            self.set_status(t("ctl.status.mode_on"));
        } else {
            self.cancel_learning();
            self.set_status(t("ctl.status.mode_off"));
        }
    }

    /// FL's *Link to controller* for the last tweaked parameter — the way into a hosted
    /// plug-in's own window: move a knob there, then this, then the hardware.
    pub fn link_last_tweaked(&mut self) -> bool {
        match self.automation.last_tweaked.clone() {
            Some(target) => {
                self.learn_target(MappingTarget::Param(target));
                true
            }
            None => {
                self.set_status(t("auto.status.nothing_tweaked"));
                false
            }
        }
    }

    /// A new mapping id, unique in both maps.
    pub(crate) fn new_mapping_id(&mut self) -> MappingId {
        loop {
            let id = MappingId(self.bus.alloc_id());
            if self.project().controllers.get(id).is_none()
                && self.controllers.global.get(id).is_none()
            {
                return id;
            }
        }
    }

    /// Add `m` to `scope`'s map (the project's: one undo step; the global one is saved).
    pub fn add_mapping(&mut self, scope: Scope, m: Mapping) -> bool {
        match scope {
            Scope::Project => {
                let index = self.project().controllers.mappings.len();
                self.exec(Command::InsertMapping {
                    index,
                    mapping: Box::new(m),
                })
            }
            Scope::Global => {
                if !m.is_valid() || !m.target.is_portable() {
                    return false;
                }
                self.controllers.global.mappings.push(m);
                self.store_global_map();
                true
            }
        }
    }

    /// Replace the mapping of `m`'s id in `scope` (a drag in the view merges under `key`).
    pub fn set_mapping(&mut self, scope: Scope, m: Mapping, key: Option<cantus_core::MergeKey>) {
        // Its mode or range may have changed: it takes over afresh.
        self.controllers.states.remove(&(scope, m.id));
        match scope {
            Scope::Project => {
                let cmd = Command::SetMapping {
                    mapping: Box::new(m),
                };
                match key {
                    Some(k) => self.exec_merging(cmd, k),
                    None => self.exec(cmd),
                };
            }
            Scope::Global => {
                if !m.is_valid() {
                    return;
                }
                if let Some(i) = self.controllers.global.position(m.id) {
                    self.controllers.global.mappings[i] = m;
                    if key.is_none() {
                        self.store_global_map();
                    }
                }
            }
        }
    }

    /// Take the mapping `id` out of `scope`'s map.
    pub fn remove_mapping(&mut self, scope: Scope, id: MappingId) {
        match scope {
            Scope::Project => {
                self.exec(Command::RemoveMapping { id });
            }
            Scope::Global => {
                self.controllers.global.mappings.retain(|m| m.id != id);
                self.store_global_map();
            }
        }
        self.controllers.states.remove(&(scope, id));
    }

    /// Write the global map to its file (when it has one).
    pub fn store_global_map(&mut self) {
        if let Some(path) = &self.controllers.global_path {
            if let Err(e) = save_global(path, &self.controllers.global) {
                self.set_error(tf("ctl.status.save_failed", &[("error", &e.to_string())]));
            }
        }
    }

    /// The selected track and strip, and the focused device's first parameters: what the
    /// relative targets name now.
    pub fn controller_focus(
        &self,
    ) -> (
        Option<cantus_core::TrackId>,
        Option<cantus_core::StripId>,
        Vec<AutoTarget>,
    ) {
        (
            self.selected_track,
            self.selected_strip,
            self.focused_device_params(),
        )
    }

    /// The concrete target `m`'s names now.
    pub fn resolve_target(&self, target: &MappingTarget) -> Option<MappingTarget> {
        if !target.is_relative() {
            return Some(target.clone());
        }
        let (track, strip, device) = self.controller_focus();
        target.resolve(&Focus {
            track,
            strip,
            device: &device,
        })
    }

    /// The focused device's first eight parameters: the device window clicked last (an
    /// effect's or an instrument's), else the open effect editor, else the selected
    /// track's instrument.
    pub fn focused_device_params(&self) -> Vec<AutoTarget> {
        let p = self.project();
        let key = self
            .theme_ctx
            .as_ref()
            .and_then(crate::views::workspace_device::focused);
        if let Some(key) = key {
            if let Some((_, e)) = p
                .all_effects()
                .find(|(_, e)| crate::views::fx::device_id(e.id).value() == key)
            {
                return self.effect_params(e.id);
            }
            if let Some(tr) = p
                .tracks
                .iter()
                .find(|tr| crate::views::inst::editor_id(tr.id).value() == key)
            {
                return self.instrument_params(tr.id);
            }
        }
        let open = self
            .fx
            .open
            .map(|(_, e, _)| e)
            .or(self.eq_editor.open.map(|(_, e)| e))
            .or(self.comp_editor.open.map(|(_, e)| e));
        if let Some(e) = open {
            return self.effect_params(e);
        }
        match self.selected_track {
            Some(tr) => self.instrument_params(tr),
            None => Vec::new(),
        }
    }

    /// An insert's first parameters: continuous ones first, its mix last.
    fn effect_params(&self, effect: cantus_core::EffectId) -> Vec<AutoTarget> {
        let p = self.project();
        let Some(slot) = p.effect_host(effect).and_then(|h| p.effect(h, effect)) else {
            return Vec::new();
        };
        if slot.spec.external.is_some() {
            let Some(h) = self.plugins.hosted(effect) else {
                return Vec::new();
            };
            return h
                .params
                .iter()
                .filter(|x| x.automatable && !x.hidden && !x.read_only)
                .take(usize::from(cantus_core::controller::DEVICE_PARAMS))
                .map(|x| AutoTarget::Plugin { effect, id: x.id })
                .collect();
        }
        let params = slot.spec.kind.params();
        let mut keys: Vec<&str> = params
            .iter()
            .filter(|i| !i.stepped && i.key != "mix")
            .map(|i| i.key)
            .collect();
        keys.extend(params.iter().filter(|i| i.stepped).map(|i| i.key));
        keys.into_iter()
            .take(usize::from(cantus_core::controller::DEVICE_PARAMS))
            .map(|k| AutoTarget::Effect {
                effect,
                key: k.to_string(),
            })
            .collect()
    }

    /// A track's instrument's first parameters.
    fn instrument_params(&self, track: cantus_core::TrackId) -> Vec<AutoTarget> {
        let n = usize::from(cantus_core::controller::DEVICE_PARAMS);
        let Some(tr) = self.project().track(track) else {
            return Vec::new();
        };
        match &tr.instrument {
            InstrumentSpec::Native(spec) => {
                let kind = spec.kind;
                let params = kind.params();
                let mut keys: Vec<&str> = params
                    .iter()
                    .filter(|i| kind.automatable(i.key) && !i.stepped)
                    .map(|i| i.key)
                    .collect();
                keys.extend(
                    params
                        .iter()
                        .filter(|i| kind.automatable(i.key) && i.stepped)
                        .map(|i| i.key),
                );
                keys.into_iter()
                    .take(n)
                    .map(|k| AutoTarget::Instrument {
                        track,
                        key: k.to_string(),
                    })
                    .collect()
            }
            InstrumentSpec::Firmus(_) => cantus_core::FirmusParam::ALL
                .iter()
                .take(n)
                .map(|p| AutoTarget::Firmus { track, param: *p })
                .collect(),
            InstrumentSpec::External(_) => self
                .plugins
                .hosted_instrument(track)
                .map(|h| {
                    h.params
                        .iter()
                        .filter(|x| x.automatable && !x.hidden && !x.read_only)
                        .take(n)
                        .map(|x| AutoTarget::PluginInstrument { track, id: x.id })
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The keymap's controller actions (round 29): the learn mode, *Link to controller*,
    /// the mappings pane. `true` when `action` was one.
    pub fn controllers_action(&mut self, action: crate::keymap::Action) -> bool {
        use crate::keymap::Action;
        match action {
            Action::MidiLearnMode => self.toggle_learn_mode(),
            Action::LinkLastTweaked => {
                self.link_last_tweaked();
            }
            Action::ViewControllers => crate::views::controllers::toggle_pane(self),
            _ => return false,
        }
        true
    }
}

/// A switch that is not a parameter (a mute, a solo, an arm, the transport's buttons) in
/// learn mode: outlined above whatever is drawn there, and a click on it arms it. `true`
/// when it was clicked (the caller then does nothing else with the click: the button under
/// is covered anyway). Outside learn mode it does nothing.
pub fn learn_spot(
    app: &mut CantusApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    target: MappingTarget,
) -> bool {
    learn_spot_in(app, ui, rect, ui.clip_rect(), target)
}

/// [`learn_spot`] inside `clip` (a view's own visible area: the mixer's strips scrolled
/// under a dock, an arrangement header under the ruler) — nothing outlined or covered
/// outside it (review 1, #46). Outside learn mode a mapped spot wears the mapped mark.
pub fn learn_spot_in(
    app: &mut CantusApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    clip: egui::Rect,
    target: MappingTarget,
) -> bool {
    let shown = rect.intersect(clip).intersect(ui.clip_rect());
    crate::kit::automate::probe::note_learn(&format!("spot:{target:?}"), true);
    if shown.width() <= 0.0 || shown.height() <= 0.0 || !ui.is_rect_visible(shown) {
        return false;
    }
    let offer = app.learn_offer(&target);
    if !app.controllers.learn_mode {
        if offer.mapped {
            crate::kit::automate::learn_mark_over(ui, rect, shown);
        }
        return false;
    }
    let id = ui.id().with(("learn-spot", &target));
    let clicked = crate::kit::automate::learn_spot_clipped(ui, rect, shown, id, offer);
    if clicked {
        app.learn_target(target);
    }
    clicked
}

/// [`learn_spot`] for a key that runs a keymap action (the top bar's transport).
pub fn learn_action_spot(
    app: &mut CantusApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    action: crate::keymap::Action,
) -> bool {
    learn_spot(
        app,
        ui,
        rect,
        MappingTarget::Action(action.id().to_string()),
    )
}

/// The track header's menu rows for MIDI learn (round 29, ADR 0062): the track's volume,
/// pan, mute, solo and arm, each checked when mapped, and *Forget* when one is. `true` when
/// a row was chosen (the menu closes).
pub fn track_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: cantus_core::TrackId) -> bool {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    let targets = [
        (
            MappingTarget::Param(AutoTarget::TrackGain(id)),
            "ctl.menu.volume",
        ),
        (
            MappingTarget::Param(AutoTarget::TrackPan(id)),
            "ctl.menu.pan",
        ),
        (MappingTarget::TrackMute(id), "ctl.menu.mute"),
        (MappingTarget::TrackSolo(id), "ctl.menu.solo"),
        (MappingTarget::TrackArm(id), "ctl.menu.arm"),
    ];
    let mapped: Vec<bool> = targets.iter().map(|(t, _)| app.is_mapped(t)).collect();
    let any = mapped.iter().any(|m| *m);
    let mut items = vec![Item::Band(Src::Key("ctl.menu.band"))];
    for ((_, key), m) in targets.iter().zip(&mapped) {
        items.push(Item::Action {
            label: Src::Key(key),
            check: if *m { Check::On } else { Check::Off },
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    if any {
        items.push(Item::action(Src::Key("ctl.menu.forget")));
    }
    match crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
        Some(i) if (1..=targets.len()).contains(&i) => {
            app.learn_target(targets[i - 1].0.clone());
            true
        }
        Some(i) if any && i == targets.len() + 1 => {
            let all: Vec<MappingTarget> = targets.iter().map(|(t, _)| t.clone()).collect();
            let name = app
                .project()
                .track(id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            app.forget_targets(&all, &name);
            true
        }
        _ => false,
    }
}

/// What a strip's (or the master's) learn row `what` drives: volume (0), pan (1), width
/// (2), mute (3), solo (4); the master has no mute or solo.
pub fn strip_learn_target(id: cantus_core::StripId, what: u8) -> Option<MappingTarget> {
    use cantus_core::AutoTarget as A;
    let master = id.is_master();
    Some(match what {
        0 if master => MappingTarget::Param(A::MasterGain),
        1 if master => MappingTarget::Param(A::MasterPan),
        2 if master => MappingTarget::Param(A::MasterWidth),
        0 => MappingTarget::Param(A::StripGain(id)),
        1 => MappingTarget::Param(A::StripPan(id)),
        2 => MappingTarget::Param(A::StripWidth(id)),
        3 if !master => MappingTarget::StripMute(id),
        4 if !master => MappingTarget::StripSolo(id),
        _ => return None,
    })
}

/// What an insert's learn row `what` drives: its bypass (0) or its mix (1).
pub fn slot_learn_target(effect: cantus_core::EffectId, what: u8) -> MappingTarget {
    if what == 0 {
        MappingTarget::EffectBypass(effect)
    } else {
        MappingTarget::EffectMix(effect)
    }
}

/// A raw message's addresses and values as mappings read them: a 7-bit controller is also
/// its 14-bit pair's half; a note-off (or a note-on at 0) is a release.
pub(crate) fn decode(
    wide: &mut HashMap<(u8, u8, u8), Wide>,
    port: u8,
    ev: &MidiEvent,
) -> (u8, [(Control, u16, bool); 2], usize) {
    let ch = ev.status & 0x0F;
    let (d1, d2) = (ev.data1 & 0x7F, ev.data2 & 0x7F);
    let none = (Control::PitchBend, 0, false);
    match ev.status & 0xF0 {
        0xB0 => {
            let first = (Control::Cc(d1), u16::from(d2), true);
            if d1 < 32 {
                let e = wide.entry((port, ch, d1)).or_default();
                e.msb = d2;
                if e.lsb_seen {
                    // Its fine half follows: the pair is read then, never half-made
                    // (review 1, #9).
                    (ch, [first, none], 1)
                } else {
                    // A pair that never sent its fine half (7-bit mode): the coarse value
                    // over the whole 14-bit travel, so its top is the top.
                    let v = (u32::from(d2) * 16_383 / 127) as u16;
                    (ch, [first, (Control::Cc14(d1), v, true)], 2)
                }
            } else if d1 < 64 {
                let e = wide.entry((port, ch, d1 - 32)).or_default();
                e.lsb_seen = true;
                let v = u16::from(e.msb) << 7 | u16::from(d2);
                (ch, [first, (Control::Cc14(d1 - 32), v, true)], 2)
            } else {
                (ch, [first, none], 1)
            }
        }
        0x90 if d2 > 0 => (ch, [(Control::Note(d1), u16::from(d2), true), none], 1),
        0x80 | 0x90 => (ch, [(Control::Note(d1), 0, false), none], 1),
        0xA0 => (
            ch,
            [(Control::PolyPressure(d1), u16::from(d2), true), none],
            1,
        ),
        0xC0 => (ch, [(Control::Program(d1), 127, true), none], 1),
        0xD0 => (
            ch,
            [(Control::ChannelPressure, u16::from(d1), true), none],
            1,
        ),
        0xE0 => (
            ch,
            [
                (Control::PitchBend, u16::from(d2) << 7 | u16::from(d1), true),
                none,
            ],
            1,
        ),
        _ => (ch, [none, none], 0),
    }
}

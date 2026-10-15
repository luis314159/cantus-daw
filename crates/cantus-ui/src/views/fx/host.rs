//! What an effect's editor needs from whatever runs it (ADR 0020).
//!
//! The editors were written against `CantusApp`. The same editor also runs inside a
//! plug-in window in another DAW (`cantus-plugins`), so what it asks of its host is this
//! trait, and each host answers it its own way:
//!
//! | Need | Cantus ([`DawHost`]) | A plug-in in another DAW |
//! |---|---|---|
//! | the parameters | the slot's `EffectSpec` | the plug-in's values |
//! | set one, with a gesture | a command merged into one undo step | CLAP gesture begin/value/end, VST3 `beginEdit`/`performEdit`/`endEdit` — the host's undo and automation record it |
//! | telemetry, analyser pairs | `Meters::fx`, the engine's tap | the plug-in's own audio thread, through the same lock-free rings |
//! | the key (sidechain) | any strip of the desk, chosen here | the host routes it; the editor offers the plug-in's *Sidechain* switch |
//! | tempo and transport | the song | what the host said in its last block, if anything |
//! | DAW-only actions (hits → a MIDI clip) | yes | `None`: the button is not shown |
//! | the look | the kit's tokens, as everywhere | the same theme, applied to the plug-in window's context |

use cantus_core::{EffectSpec, StripId};
use cantus_engine::Telemetry;
use std::collections::VecDeque;

/// Where the song is, for displays that draw bar lines or stamp events in song time.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transport {
    /// The song position in ticks, while it plays.
    pub playing_at: Option<i64>,
    pub ticks_per_second: f64,
    pub ticks_per_bar: i64,
}

impl Default for Transport {
    /// 120 BPM in 4/4, stopped: what a display assumes when the host says nothing.
    fn default() -> Self {
        let ppq = cantus_core::PPQ as f64;
        Transport {
            playing_at: None,
            ticks_per_second: 2.0 * ppq,
            ticks_per_bar: 4 * cantus_core::PPQ,
        }
    }
}

/// Where the detector's key comes from, as far as the host lets the editor choose it.
#[derive(Clone, Debug, PartialEq)]
pub enum KeySource {
    /// Any strip of the desk (Cantus): the one chosen, its label, and the choices.
    Strips {
        current: Option<StripId>,
        label: String,
        options: Vec<(StripId, String)>,
    },
    /// The host routes a sidechain input; the editor switches the detector to it.
    Switch(bool),
}

/// A change of key source.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum KeyTarget {
    Strip(Option<StripId>),
    Switch(bool),
}

/// What an editor asks of its host.
pub trait FxHost {
    /// The effect's parameters as they are this frame.
    fn spec(&self) -> &EffectSpec;
    /// Set one parameter. `gesture` names the drag or turn it belongs to: everything one
    /// gesture sets is one step of undo (Cantus) or one begin…end gesture (a plug-in
    /// host); `None` is a change of its own.
    fn set(&mut self, key: &str, value: f32, gesture: Option<&str>);
    /// Several at once, as one step (a learned pair, a reset).
    fn set_many(&mut self, values: &[(&str, f32)], label_key: &str);
    /// The pointer was released: the gesture is over.
    fn end_gesture(&mut self);
    /// The unit's 64 numbers, refreshed each block.
    fn telemetry(&self) -> Telemetry;
    /// Recent (input, output) mono pairs from the analyser tap, oldest first.
    fn pairs(&self) -> &VecDeque<(f32, f32)>;
    fn transport(&self) -> Transport;
    /// The key's source, if the host lets the editor choose it.
    fn key_source(&self) -> Option<KeySource> {
        None
    }
    fn set_key_source(&mut self, _target: KeyTarget) {}
    /// A line for the status bar, where the host has one.
    fn status(&mut self, _text: String) {}
    /// Whether the host can automate a parameter (Cantus; ADR 0027). A plug-in host
    /// automates through its own lanes, so the editor offers nothing.
    fn can_automate(&self) -> bool {
        false
    }
    /// Whether a clip drives `key` (the cell wears the automated mark).
    fn automated(&self, _key: &str) -> bool {
        false
    }
    /// Create the automation clip of `key`, or open the one it has.
    fn automate(&mut self, _key: &str) {}
    /// Notes into a MIDI clip on a new drum track — something only a DAW can do. `notes`
    /// are (tick, velocity) at `pitch`. `None` when the host cannot; `Some(written)`.
    fn notes_to_clip(&mut self, _notes: &[(i64, u8)], _pitch: u8) -> Option<bool> {
        None
    }
    /// Whether [`FxHost::notes_to_clip`] is offered, so the editor can hide the button.
    fn makes_clips(&self) -> bool {
        false
    }
    /// The editor's window as a device of the workspace (round 20): its key, when the
    /// host draws the window's own buttons — swap, minimise, maximise, close. `None` where
    /// the window is the host's (the exported plug-ins').
    fn device_key(&self) -> Option<u64> {
        None
    }
    /// The swap button: offer the device picker for this slot, in place.
    fn swap(&mut self) {}

    // ---- editing several parameters at once ------------------------------------------

    /// Several changes as one step: `Some(v)` sets a key, `None` clears it back to its
    /// default (a sparse kind forgets it). With `gesture`, every call carrying the same
    /// number folds into one step — a drag of several bands at once.
    fn apply(&mut self, edits: &[(&str, Option<f32>)], label_key: &str, gesture: Option<u64>) {
        let _ = gesture;
        let kind = self.spec().kind;
        let values: Vec<(&str, f32)> = edits
            .iter()
            .map(|(k, v)| {
                let v = v.unwrap_or_else(|| kind.param(k).map_or(0.0, |i| i.default));
                (*k, v)
            })
            .collect();
        self.set_many(&values, label_key);
    }

    /// Replace every parameter with `params` (a preset, the other side of A/B): what it
    /// names is set, everything else goes back to its default. One step.
    fn load(&mut self, params: &std::collections::BTreeMap<String, f32>, label_key: &str) {
        let spec = self.spec();
        let kind = spec.kind;
        let mut edits: Vec<(&str, Option<f32>)> = Vec::new();
        for info in kind.params() {
            let now = spec.get(info.key);
            match params.get(info.key) {
                Some(v) if *v != now || !spec.params.contains_key(info.key) => {
                    edits.push((info.key, Some(*v)))
                }
                Some(_) => {}
                None if spec.params.contains_key(info.key) || now != info.default => {
                    edits.push((info.key, None))
                }
                None => {}
            }
        }
        if !edits.is_empty() {
            self.apply(&edits, label_key, None);
        }
    }

    // ---- the device frame (ADR 0024 §8) ----------------------------------------------

    /// Where the device sits, for the header: "Insert 3 · Keys". Empty when the host has
    /// no desk.
    fn context(&self) -> String {
        String::new()
    }
    /// `Some(true)` when the device is bypassed; `None` when the host keeps no bypass.
    fn bypassed(&self) -> Option<bool> {
        None
    }
    fn set_bypassed(&mut self, _bypassed: bool) {}
    /// The host's undo, filtered to this device: (can undo, can redo). `None`: the host
    /// keeps its own history (a DAW hosting the plug-in), so the header shows none.
    fn history(&self) -> Option<(bool, bool)> {
        None
    }
    fn undo(&mut self) {}
    fn redo(&mut self) {}
    /// A/B: whether B is the side showing; `None` when the host offers no compare.
    fn ab(&self) -> Option<bool> {
        None
    }
    /// Switch sides: the showing one is kept, the other applied — one step.
    fn toggle_ab(&mut self) {}
    /// Copy the showing side onto the other one.
    fn copy_ab(&mut self) {}
    /// Whether the user's own presets can be saved here (a folder of the DAW's).
    fn user_presets(&self) -> bool {
        false
    }
    /// The sample rate the unit runs at, for displays that design filters.
    fn sample_rate(&self) -> f32 {
        48_000.0
    }
    /// The name of a strip of the desk (a key's source), where there is a desk.
    fn strip_label(&self, _strip: StripId) -> String {
        String::new()
    }
    /// A number that changes whenever the host's state might have (the project's
    /// revision), so the frame can keep what it formats until then.
    fn revision(&self) -> u64 {
        0
    }
    /// The device's own name, when it is not its kind's (a hosted plug-in's).
    fn name(&self) -> Option<String> {
        None
    }
    /// The presets the header offers, read when its list opens: the kind's factory
    /// presets, then the user's where the host keeps a folder.
    fn presets(&self) -> Vec<super::frame::Entry> {
        super::frame::read_presets(self.spec().kind, self.user_presets())
    }
    /// Load one of [`FxHost::presets`]: one step.
    fn load_preset(&mut self, preset: &super::frame::Entry) {
        self.load(&preset.params, "cmd.fx_preset");
    }
    /// Save the values as the user's preset `name`.
    fn save_preset(&mut self, name: &str) -> Result<(), String> {
        let spec = self.spec();
        super::save_user_preset(spec.kind, name, &spec.params)
    }
    /// Cantus itself, for what only the DAW has (a track's notes playing the gate, the
    /// strips of the desk): the app, the strip and the effect. `None` in a plug-in window.
    fn daw(&mut self) -> Option<(&mut crate::app::CantusApp, StripId, cantus_core::EffectId)> {
        None
    }
    /// Take the analyser pairs that came since the last call, oldest first, into `out` —
    /// for an editor that measures the stream as it comes (Spatium's tail). Cantus hands
    /// over its tap's queue; a host with nothing to give leaves `out` as it is.
    fn take_pairs(&mut self, out: &mut Vec<(f32, f32)>) {
        if let Some((app, _, _)) = self.daw() {
            out.extend(app.fx.pairs.drain(..));
        }
    }

    // ---- what only some units report -------------------------------------------------

    /// The EQ's dynamic bands: how far each moved in the last block, dB (Cantus's engine
    /// meters them). `None` where the host has no such meter.
    fn eq_deltas(&self) -> Option<[f32; cantus_core::EQ_BANDS]> {
        None
    }

    /// Move the analyser tap's newest (input, output) pairs, interleaved, into `out`, for
    /// a display that keeps its own analyser (the EQ's). `false` when the host has no such
    /// stream: the display reads [`FxHost::pairs`] instead.
    fn drain_analyser(&mut self, _out: &mut Vec<f32>) -> bool {
        false
    }

    /// Stereo frames of this effect's own signal that came since the last call, oldest
    /// first, into `out` — every one of them, for an analyser that measures each sample
    /// (Speculum, ADR 0056). What it says of them: whether the host has such a stream at
    /// all, and the song's position of the newest frame and the tempo while the song plays.
    fn take_stereo(&mut self, _out: &mut Vec<(f32, f32)>) -> StereoTake {
        StereoTake::default()
    }
}

/// Whether a command changes the effect `effect` (the header's undo is this device's).
pub fn touches(cmd: &cantus_core::Command, effect: cantus_core::EffectId) -> bool {
    use cantus_core::Command as C;
    match cmd {
        C::SetEffectParam { effect: e, .. }
        | C::ClearEffectParam { effect: e, .. }
        | C::SetEffectBypass { effect: e, .. }
        | C::SetEffectSidechain { effect: e, .. }
        | C::SetEffectMidiSource { effect: e, .. }
        | C::SetEffectMix { effect: e, .. }
        | C::SetPluginParam { effect: e, .. }
        | C::SetPluginState { effect: e, .. } => *e == effect,
        C::Batch { commands, .. } => commands.iter().any(|c| touches(c, effect)),
        _ => false,
    }
}

/// Cantus as an editor's host: the project, the engine and the desk.
pub struct DawHost<'a> {
    pub app: &'a mut crate::app::CantusApp,
    pub strip: StripId,
    pub effect: cantus_core::EffectId,
    spec: EffectSpec,
    /// What the editor's gestures are merged under (`MergeKey`): its kind's name.
    tag: &'static str,
    /// The project's revision the spec was read at, when it came from [`DawHost::new`]:
    /// it goes back to the app's cache when the host is dropped.
    cached_at: Option<u64>,
}

impl Drop for DawHost<'_> {
    fn drop(&mut self) {
        if let Some(rev) = self.cached_at {
            // An empty stand-in: a new map allocates nothing.
            let empty = EffectSpec {
                kind: self.spec.kind,
                params: Default::default(),
                external: None,
            };
            let spec = std::mem::replace(&mut self.spec, empty);
            self.app.fx.spec_cache = Some((rev, self.effect, spec));
        }
    }
}

impl<'a> DawHost<'a> {
    /// The host for one effect, its spec read from the project (a new spec of the
    /// kind if the effect is gone). The copy is kept between frames while the project
    /// does not change, so an editor at rest copies nothing.
    pub fn new(
        app: &'a mut crate::app::CantusApp,
        strip: StripId,
        effect: cantus_core::EffectId,
        tag: &'static str,
    ) -> DawHost<'a> {
        let rev = app.bus.revision();
        let spec = match app.fx.spec_cache.take() {
            Some((r, e, spec)) if r == rev && e == effect => spec,
            _ => app
                .project()
                .effect(strip, effect)
                .map(|e| e.spec.clone())
                .unwrap_or_else(|| EffectSpec::new(cantus_core::EffectKind::Gate)),
        };
        // An automated parameter shows what its clip gives it where the song is heard
        // (ADR 0027): FL's knobs move with their automation.
        let mut spec = spec;
        let lanes = app.auto_lanes();
        let at = app.engine.position().0 as f64;
        for l in lanes.iter() {
            if let cantus_core::AutoTarget::Effect { effect: e, key } = &l.target {
                if *e == effect {
                    let v = l.value_at(at);
                    match spec.params.get_mut(key.as_str()) {
                        Some(x) => *x = v,
                        None => {
                            spec.params.insert(key.clone(), v);
                        }
                    }
                }
            }
        }
        let mut host = DawHost::with_spec(app, strip, effect, spec, tag);
        host.cached_at = Some(rev);
        host
    }

    /// The host for one effect whose spec the caller already has.
    pub fn with_spec(
        app: &'a mut crate::app::CantusApp,
        strip: StripId,
        effect: cantus_core::EffectId,
        spec: EffectSpec,
        tag: &'static str,
    ) -> DawHost<'a> {
        DawHost {
            app,
            strip,
            effect,
            spec,
            tag,
            cached_at: None,
        }
    }
}

impl FxHost for DawHost<'_> {
    fn spec(&self) -> &EffectSpec {
        &self.spec
    }

    fn can_automate(&self) -> bool {
        true
    }

    fn automated(&self, key: &str) -> bool {
        let effect = self.effect;
        self.app
            .automated_value_where(|t| {
                matches!(t, cantus_core::AutoTarget::Effect { effect: e, key: k } if *e == effect && k == key)
            })
            .is_some()
    }

    fn automate(&mut self, key: &str) {
        self.app.automate(cantus_core::AutoTarget::Effect {
            effect: self.effect,
            key: key.to_string(),
        });
    }

    fn device_key(&self) -> Option<u64> {
        Some(super::device_id(self.effect).value())
    }

    fn swap(&mut self) {
        self.app
            .open_picker(crate::picker::PickPurpose::ReplaceEffect {
                strip: self.strip,
                effect: self.effect,
            });
    }

    fn set(&mut self, key: &str, value: f32, gesture: Option<&str>) {
        let cmd = cantus_core::Command::SetEffectParam {
            strip: self.strip,
            effect: self.effect,
            key: key.to_string(),
            value,
        };
        match gesture {
            Some(g) => {
                self.app.exec_merging(
                    cmd,
                    cantus_core::MergeKey::of(&(self.tag, self.effect.0, g)),
                );
            }
            None => {
                self.app.exec(cmd);
            }
        }
    }

    fn set_many(&mut self, values: &[(&str, f32)], label_key: &str) {
        let commands = values
            .iter()
            .map(|(k, v)| cantus_core::Command::SetEffectParam {
                strip: self.strip,
                effect: self.effect,
                key: k.to_string(),
                value: *v,
            })
            .collect();
        self.app.exec(cantus_core::Command::Batch {
            label_key: label_key.into(),
            commands,
        });
    }

    fn end_gesture(&mut self) {
        self.app.end_gesture();
    }

    fn telemetry(&self) -> Telemetry {
        self.app.fx_telemetry()
    }

    fn pairs(&self) -> &VecDeque<(f32, f32)> {
        &self.app.fx.pairs
    }

    fn transport(&self) -> Transport {
        let project = self.app.project();
        Transport {
            playing_at: self
                .app
                .engine
                .is_playing()
                // The engine's position, not the heard one: an editor times what its unit
                // sees at its input (ADR 0022).
                .then(|| self.app.engine.engine_position().0),
            ticks_per_second: project.tempo_bpm / 60.0 * cantus_core::PPQ as f64,
            ticks_per_bar: project.time_signature.ticks_per_bar().0,
        }
    }

    fn key_source(&self) -> Option<KeySource> {
        let project = self.app.project();
        let current = project
            .effect(self.strip, self.effect)
            .and_then(|e| e.sidechain);
        let label = |id: StripId| {
            self.app
                .capture_label(cantus_engine::CaptureTarget::Strip(id))
        };
        let options = project
            .strips
            .iter()
            .map(|x| x.id)
            .filter(|id| *id != self.strip && !project.routing_would_cycle(*id, Some(self.strip)))
            .map(|id| (id, label(id)))
            .collect();
        Some(KeySource::Strips {
            current,
            label: match current {
                Some(k) => label(k),
                None => crate::i18n::t("gate.key_own"),
            },
            options,
        })
    }

    fn set_key_source(&mut self, target: KeyTarget) {
        if let KeyTarget::Strip(source) = target {
            self.app.exec(cantus_core::Command::SetEffectSidechain {
                strip: self.strip,
                effect: self.effect,
                source,
            });
        }
    }

    fn status(&mut self, text: String) {
        self.app.set_status(text);
    }

    fn notes_to_clip(&mut self, notes: &[(i64, u8)], pitch: u8) -> Option<bool> {
        Some(self.app.gate_notes_to_clip(self.strip, notes, pitch))
    }

    fn makes_clips(&self) -> bool {
        true
    }

    fn apply(&mut self, edits: &[(&str, Option<f32>)], label_key: &str, gesture: Option<u64>) {
        use cantus_core::Command as C;
        let (strip, effect) = (self.strip, self.effect);
        let commands: Vec<C> = edits
            .iter()
            .map(|(k, v)| match v {
                Some(value) => C::SetEffectParam {
                    strip,
                    effect,
                    key: k.to_string(),
                    value: *value,
                },
                None => C::ClearEffectParam {
                    strip,
                    effect,
                    key: k.to_string(),
                },
            })
            .collect();
        if commands.is_empty() {
            return;
        }
        let cmd = C::Batch {
            label_key: label_key.into(),
            commands,
        };
        match gesture {
            Some(g) => {
                self.app.exec_merging(
                    cmd,
                    cantus_core::MergeKey::of(&(self.tag, self.effect.0, "apply", g)),
                );
            }
            None => {
                self.app.exec(cmd);
            }
        }
    }

    fn load(&mut self, params: &std::collections::BTreeMap<String, f32>, label_key: &str) {
        self.app
            .apply_fx_params(self.strip, self.effect, params, label_key);
    }

    fn context(&self) -> String {
        let project = self.app.project();
        let slot = project
            .effects_of(self.strip)
            .and_then(|s| s.iter().position(|e| e.id == self.effect))
            .map_or(1, |i| i + 1);
        crate::i18n::tf(
            "fx.frame.context",
            &[
                ("n", &slot),
                (
                    "strip",
                    &self
                        .app
                        .capture_label(cantus_engine::CaptureTarget::Strip(self.strip)),
                ),
            ],
        )
    }

    fn bypassed(&self) -> Option<bool> {
        self.app
            .project()
            .effect(self.strip, self.effect)
            .map(|e| e.bypass)
    }

    fn set_bypassed(&mut self, bypass: bool) {
        self.app.exec(cantus_core::Command::SetEffectBypass {
            strip: self.strip,
            effect: self.effect,
            bypass,
        });
    }

    fn history(&self) -> Option<(bool, bool)> {
        let bus = &self.app.bus;
        let undo = bus
            .history()
            .last()
            .is_some_and(|e| touches(&e.command, self.effect));
        // The bus keeps no public redo list: the step on top of it is this device's while
        // nothing has changed the project since this device's own undo.
        let redo = bus.can_redo() && self.app.fx.redo_mark == Some((self.effect, bus.revision()));
        Some((undo, redo))
    }

    fn undo(&mut self) {
        if self.history().is_some_and(|h| h.0) {
            self.app.undo();
            self.app.fx.redo_mark = Some((self.effect, self.app.bus.revision()));
        }
    }

    fn redo(&mut self) {
        if self.history().is_some_and(|h| h.1) {
            self.app.redo();
            let more = self.app.bus.can_redo();
            self.app.fx.redo_mark = more.then(|| (self.effect, self.app.bus.revision()));
        }
    }

    fn ab(&self) -> Option<bool> {
        if self.spec.external.is_some() {
            // A hosted plug-in compares whole states (`views::plugins`).
            return Some(self.app.fx.hosted.ab.get(&self.effect).is_some_and(|x| x.2));
        }
        Some(self.app.fx.on_b(self.effect))
    }

    fn toggle_ab(&mut self) {
        if self.spec.external.is_some() {
            crate::views::plugins::toggle_ab(self.app, self.strip, self.effect);
            return;
        }
        self.app.toggle_ab(self.strip, self.effect);
    }

    fn copy_ab(&mut self) {
        if self.spec.external.is_some() {
            let on_b = self.ab() == Some(true);
            if let Some((state, params)) = self.app.plugins.capture(self.effect) {
                self.app
                    .fx
                    .hosted
                    .ab
                    .insert(self.effect, (state, params, on_b));
            }
            return;
        }
        let params = self.spec.params.clone();
        self.app.copy_ab(self.effect, &params);
    }

    fn name(&self) -> Option<String> {
        self.spec.external.as_deref().map(|p| p.name.clone())
    }

    fn presets(&self) -> Vec<super::frame::Entry> {
        match self.spec.external.as_deref() {
            // A hosted plug-in's presets are its saved states.
            Some(p) => crate::views::plugins::presets(&p.id)
                .into_iter()
                .map(|(name, path)| super::frame::Entry {
                    name,
                    params: Default::default(),
                    factory: false,
                    file: Some(path),
                })
                .collect(),
            None => super::frame::read_presets(self.spec.kind, true),
        }
    }

    fn load_preset(&mut self, preset: &super::frame::Entry) {
        match &preset.file {
            Some(path) => {
                let file = std::fs::read_to_string(path).ok().and_then(|t| {
                    serde_json::from_str::<crate::views::plugins::PresetFile>(&t).ok()
                });
                if let Some(f) = file {
                    self.app
                        .set_plugin_state(self.strip, self.effect, f.state, f.params);
                }
            }
            None => self.load(&preset.params, "cmd.fx_preset"),
        }
    }

    fn save_preset(&mut self, name: &str) -> Result<(), String> {
        match self.spec.external.as_deref().map(|p| p.id.clone()) {
            Some(id) => crate::views::plugins::save_preset(self.app, self.effect, &id, name),
            None => super::save_user_preset(self.spec.kind, name, &self.spec.params),
        }
    }

    fn user_presets(&self) -> bool {
        true
    }

    fn sample_rate(&self) -> f32 {
        let sr = self.app.engine.sample_rate as f32;
        if sr > 1_000.0 {
            sr
        } else {
            48_000.0
        }
    }

    fn strip_label(&self, strip: StripId) -> String {
        self.app
            .capture_label(cantus_engine::CaptureTarget::Strip(strip))
    }

    fn revision(&self) -> u64 {
        self.app.bus.revision()
    }

    fn daw(&mut self) -> Option<(&mut crate::app::CantusApp, StripId, cantus_core::EffectId)> {
        Some((&mut *self.app, self.strip, self.effect))
    }

    fn eq_deltas(&self) -> Option<[f32; cantus_core::EQ_BANDS]> {
        let d = &self.app.engine.meters().eq_deltas;
        let mut out = [0.0; cantus_core::EQ_BANDS];
        let n = out.len().min(d.len());
        out[..n].copy_from_slice(&d[..n]);
        Some(out)
    }

    fn drain_analyser(&mut self, out: &mut Vec<f32>) -> bool {
        self.app.engine.drain_analyser(out);
        true
    }

    fn take_stereo(&mut self, out: &mut Vec<(f32, f32)>) -> StereoTake {
        let effect = self.effect;
        let engine = &mut self.app.engine;
        if !engine.watch_scope(effect) {
            return StereoTake::default();
        }
        let per_frame_of = |tempo: f64, sr: f64| tempo / 60.0 / sr.max(1.0);
        let sr = engine.sample_rate;
        let mut clock = None;
        engine.drain_scope(effect, |c| {
            let n = c.frames as usize;
            out.extend(c.l[..n].iter().copied().zip(c.r[..n].iter().copied()));
            clock = c.beat.map(|b| {
                (
                    b + (n.max(1) - 1) as f64 * per_frame_of(c.tempo_bpm, sr),
                    c.tempo_bpm,
                )
            });
        });
        StereoTake {
            stream: true,
            clock,
        }
    }
}

/// What [`FxHost::take_stereo`] says of the frames it gave.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct StereoTake {
    /// The host has a stream of the effect's own frames.
    pub stream: bool,
    /// The song's position of the newest frame, quarter notes, and the tempo, while the
    /// song plays.
    pub clock: Option<(f64, f64)>,
}

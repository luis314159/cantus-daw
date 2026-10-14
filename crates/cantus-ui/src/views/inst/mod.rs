//! The native instruments' editors, and the frame they share (ADR 0023,
//! `research/instruments/family.md` §6).
//!
//! Every instrument's editor is a module here that owns its **Edit** view (`body`), its
//! **Play** view (`play`), its small picture for the inspector (`preview`), its factory
//! presets and the demo set-up the screenshot tool uses. What they share is written once,
//! here and in `frame.rs`:
//!
//! - **The frame**: one window per open editor; a header with the track it sits on, the
//!   presets, A/B and the view switch (Play · Edit · Matrix); the **macro bar**, the
//!   **modulation strip** (sources you drag onto a knob), the **keyboard** and a **footer**
//!   with the voices in use.
//! - **The host owns state**: every edit is a command, so undo, A/B and presets are the
//!   project's; A/B switching is one undo step.
//! - **Modulation** is the shared model of `cantus_core::inst::modulation`: drag a source
//!   onto a knob to connect it, see the range as a ring, edit every connection in Matrix.
//! - **What the engine reports** for the open editor: the unit's telemetry
//!   (`Meters::inst`). Indices [`TEL_VOICES`], [`TEL_POLYPHONY`] and [`TEL_STOLEN`] are the
//!   frame's; an instrument's own numbers start at [`TEL_OWN`].

use crate::app::CantusApp;
use crate::design::{self, space as grid};
use crate::i18n::{t, tf};
use crate::theme;
use cantus_core::{Command, InstrumentKind, InstrumentSpec, MergeKey, NativeSpec, TrackId};
use egui::RichText;
use std::collections::HashMap;

mod frame;
pub use frame::*;

pub mod analog;
pub mod bass;
pub mod epiano;
pub mod fm;
pub mod organ;
pub mod sampler;
pub mod wavetable;

/// Telemetry slots the frame reads (the rest are the instrument's own).
pub const TEL_VOICES: usize = 0;
pub const TEL_POLYPHONY: usize = 1;
pub const TEL_STOLEN: usize = 2;
pub const TEL_OWN: usize = 8;

/// A factory preset: a label key, the numbers and the texts it sets, and its tags
/// (`preset.tag.*` keys: bass, lead, keys, pad, pluck…). Keys it does not name go back to
/// their defaults.
#[derive(Copy, Clone, Debug)]
pub struct FactoryPreset {
    pub label_key: &'static str,
    pub values: &'static [(&'static str, f32)],
    pub text: &'static [(&'static str, &'static str)],
    pub tags: &'static [&'static str],
}

/// A pop-up the shell's header opened.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Popup {
    #[default]
    None,
    More,
    Presets,
    Sizes,
}

/// Which of the three views the editor shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Play,
    Edit,
    Matrix,
}

/// State of the instrument editors behind this module.
#[derive(Debug, Default)]
pub struct InstEditors {
    /// The track whose instrument's editor is open.
    pub open: Option<TrackId>,
    pub view: View,
    /// Telemetry injected by demos and tests, used instead of the engine's while set.
    pub injected: Option<cantus_engine::Telemetry>,
    /// The other side of A/B per track, and whether B is showing.
    ab: HashMap<TrackId, (NativeSpec, bool)>,
    /// Keys the on-screen keyboard is holding down.
    pub held: Vec<u8>,
    /// The source being dragged from the macro bar or the modulation strip.
    pub dragging: Option<&'static str>,
    /// The shell's bands (ADR 0024 §8): collapsed macro and modulation cards, and the
    /// keyboard (`None`: shown in Play only, the default).
    pub macros_closed: bool,
    pub mods_closed: bool,
    pub keyboard: Option<bool>,
    /// Which of the shell's pop-ups is open: the ⋯ menu, the preset list, the sizes.
    pub popup: Popup,
    /// The factory or user preset each track's instrument was last set from.
    pub preset: HashMap<TrackId, String>,
    pub wavetable: wavetable::State,
    pub analog: analog::State,
    pub fm: fm::State,
    pub sampler: sampler::State,
    pub bass: bass::State,
    pub epiano: epiano::State,
    pub organ: organ::State,
}

impl InstEditors {
    pub fn on_b(&self, track: TrackId) -> bool {
        self.ab.get(&track).is_some_and(|(_, b)| *b)
    }

    /// The track's instrument was swapped for another (round 20): its A/B side and its
    /// preset name belonged to the old one.
    pub fn forget(&mut self, track: TrackId) {
        self.ab.remove(&track);
        self.preset.remove(&track);
    }
}

/// `--editor <name>` for an instrument: its kind, by the module or product name.
pub fn kind_from_name(name: &str) -> Option<InstrumentKind> {
    Some(match name {
        "wavetable" | "unda" => InstrumentKind::Wavetable,
        "analog" | "calor" => InstrumentKind::Analog,
        "fm" | "modulus" => InstrumentKind::Fm,
        "sampler" | "exemplar" => InstrumentKind::Sampler,
        "bass" | "gravis" => InstrumentKind::Bass,
        "epiano" | "lamina" => InstrumentKind::ElectricPiano,
        "organ" | "organum" => InstrumentKind::Organ,
        _ => return None,
    })
}

impl CantusApp {
    /// The open editor's telemetry: injected (demos, tests) or the engine's.
    pub fn inst_telemetry(&self) -> cantus_engine::Telemetry {
        self.inst
            .injected
            .unwrap_or_else(|| self.engine.meters().inst)
    }

    pub fn open_inst_editor(&mut self, track: TrackId) {
        // A hosted instrument's editor is the plug-in's own window (ADR 0028).
        if self.is_hosted_instrument(track) {
            self.open_hosted_window(track);
            return;
        }
        // Already open (shown, behind another tab, under another window or in the tray):
        // brought back and to the front as it is (round 27, ADR 0051) — closing and
        // opening it again would leave a minimised one in the tray.
        if self.inst.open == Some(track) {
            if let Some(ctx) = &self.theme_ctx {
                crate::views::workspace_device::present(ctx, editor_id(track).value());
            }
            return;
        }
        self.close_inst_editor();
        self.inst.open = Some(track);
        self.engine.set_instrument_probe(Some(track));
    }

    pub fn close_inst_editor(&mut self) {
        if let Some(track) = self.inst.open.take() {
            for key in std::mem::take(&mut self.inst.held) {
                self.engine.note_off(track, key);
            }
            self.engine.set_instrument_probe(None);
        }
    }

    /// The native instrument a track hosts, cloned.
    pub fn native_of(&self, track: TrackId) -> Option<NativeSpec> {
        self.project()
            .track(track)
            .and_then(|t| t.instrument.native())
            .cloned()
    }

    /// Set one number; a gesture merges frame to frame into one undo step.
    pub fn set_inst_value(&mut self, track: TrackId, key: &str, value: f32, gesture: bool) {
        let cmd = Command::SetInstrumentValue {
            track,
            key: key.to_string(),
            value,
        };
        if gesture {
            self.exec_merging(cmd, MergeKey::of(&("inst", track.0, key)));
        } else {
            self.exec(cmd);
        }
    }

    pub fn set_inst_text(&mut self, track: TrackId, key: &str, text: Option<String>) {
        self.exec(Command::SetInstrumentText {
            track,
            key: key.to_string(),
            text,
        });
    }

    /// Make the track's instrument equal `target` (same kind), as one command: numbers and
    /// texts it names are set, everything else stored goes back to its default. Presets
    /// and A/B go through here.
    pub fn apply_inst_spec(&mut self, track: TrackId, target: &NativeSpec, label_key: &str) {
        let Some(now) = self.native_of(track) else {
            return;
        };
        if now.kind != target.kind {
            return;
        }
        let mut commands = Vec::new();
        for (k, v) in &target.params {
            if now.params.get(k) != Some(v) {
                commands.push(Command::SetInstrumentValue {
                    track,
                    key: k.clone(),
                    value: *v,
                });
            }
        }
        for k in now
            .params
            .keys()
            .filter(|k| !target.params.contains_key(*k))
        {
            commands.push(Command::ClearInstrumentValue {
                track,
                key: k.clone(),
            });
        }
        for (k, v) in &target.text {
            if now.text.get(k) != Some(v) {
                commands.push(Command::SetInstrumentText {
                    track,
                    key: k.clone(),
                    text: Some(v.clone()),
                });
            }
        }
        for k in now.text.keys().filter(|k| !target.text.contains_key(*k)) {
            commands.push(Command::SetInstrumentText {
                track,
                key: k.clone(),
                text: None,
            });
        }
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: label_key.into(),
                commands,
            });
        }
    }

    /// Switch between A and B: one command, so one step of undo.
    pub fn toggle_inst_ab(&mut self, track: TrackId) {
        let Some(current) = self.native_of(track) else {
            return;
        };
        let (other, on_b) = self
            .inst
            .ab
            .remove(&track)
            .unwrap_or_else(|| (current.clone(), false));
        self.apply_inst_spec(track, &other, "cmd.fx_ab");
        self.inst.ab.insert(track, (current, !on_b));
    }

    pub fn copy_inst_ab(&mut self, track: TrackId, spec: &NativeSpec) {
        let on_b = self.inst.on_b(track);
        self.inst.ab.insert(track, (spec.clone(), on_b));
    }

    /// Connect `source` to the numeric key `target` in the first free slot, at a quarter
    /// of the travel: one undo step.
    pub fn connect_modulation(&mut self, track: TrackId, source: &str, target: &str) {
        use cantus_core::inst::modulation::{free_slot, SLOTS};
        let Some(spec) = self.native_of(track) else {
            return;
        };
        let Some(n) = free_slot(&spec) else {
            self.set_error(t("mod.full"));
            return;
        };
        let k = SLOTS[n - 1];
        self.exec(Command::Batch {
            label_key: "cmd.connect_modulation".into(),
            commands: vec![
                Command::SetInstrumentText {
                    track,
                    key: k.src.into(),
                    text: Some(source.into()),
                },
                Command::SetInstrumentText {
                    track,
                    key: k.dst.into(),
                    text: Some(target.into()),
                },
                Command::SetInstrumentValue {
                    track,
                    key: k.amt.into(),
                    value: 0.25,
                },
            ],
        });
    }

    /// Empty a connection slot (1-based): one undo step.
    pub fn remove_modulation(&mut self, track: TrackId, slot: usize) {
        use cantus_core::inst::modulation::SLOTS;
        let Some(spec) = self.native_of(track) else {
            return;
        };
        let k = SLOTS[slot - 1];
        let mut commands = Vec::new();
        for key in [k.src, k.dst, k.via] {
            if spec.text(key).is_some() {
                commands.push(Command::SetInstrumentText {
                    track,
                    key: key.into(),
                    text: None,
                });
            }
        }
        for key in [k.amt, k.curve, k.on] {
            if spec.is_stored(key) {
                commands.push(Command::ClearInstrumentValue {
                    track,
                    key: key.into(),
                });
            }
        }
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.remove_modulation".into(),
                commands,
            });
        }
    }

    /// `--editor organ` and friends: the instrument on the selected track, set up to show
    /// what its editor does, with the editor open.
    pub fn demo_instrument(&mut self, kind: InstrumentKind) {
        let Some(track) = self.selected_track else {
            return;
        };
        self.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        demo_setup(self, track, kind);
        self.open_inst_editor(track);
    }
}

/// The folder a kind's user presets live in.
pub fn user_preset_dir(kind: InstrumentKind) -> std::path::PathBuf {
    crate::app::data_dir()
        .join("presets")
        .join("instruments")
        .join(format!("{kind:?}").to_lowercase())
}

/// The user's presets for a kind, by name.
pub fn user_presets(kind: InstrumentKind) -> Vec<(String, NativeSpec)> {
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
            if let Ok(spec) = serde_json::from_str::<NativeSpec>(&text) {
                let spec = preset_from_file(spec);
                if spec.kind == kind {
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    out.push((name, spec));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A user preset as written to its file: a recalibrated kind's output trim (ADR 0039) is
/// written out even at its default, so the preset reads back at the level it was saved
/// with whatever a later calibration says.
pub fn preset_to_file(spec: &NativeSpec) -> NativeSpec {
    spec.spell_out_trim()
}

/// A user preset as read from its file: one of a recalibrated kind without a trim was
/// saved before the calibration (ADR 0039), at the old level — a trim of 0 dB.
pub fn preset_from_file(spec: NativeSpec) -> NativeSpec {
    spec.read_unversioned()
}

/// Save the instrument's state as a user preset.
pub fn save_user_preset(spec: &NativeSpec, name: &str) -> Result<(), String> {
    let clean: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return Err(t("preset.empty_name"));
    }
    let dir = user_preset_dir(spec.kind);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&preset_to_file(spec)).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{clean}.json")), text).map_err(|e| e.to_string())
}

/// A factory preset as the spec it describes.
pub fn preset_spec(kind: InstrumentKind, preset: &FactoryPreset) -> NativeSpec {
    let mut s = NativeSpec::new(kind);
    for (k, v) in preset.values {
        s.set(k, *v);
    }
    for (k, v) in preset.text {
        let _ = s.set_text(k, Some((*v).to_string()));
    }
    s
}

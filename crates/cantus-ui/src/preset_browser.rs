//! The preset browser's state and what it does (round 37, ADR 0101); its window is
//! `views::preset_browser`.
//!
//! Opened on a unit — the instrument (or Pulsus kit) of a track, or an insert — it lists
//! that unit's presets (`preset_catalog`), filters them by style and role and by words
//! (`preset_query`), **auditions** one without touching the project (`preset_audition`)
//! and **applies** the one chosen as a single step of undo — the same command the unit's
//! own preset menu runs.

use crate::app::CantusApp;
use crate::preset_audition::{Audition, Target};
use crate::preset_catalog::{entries, Entry, Payload, Unit};
use crate::preset_query::Query;
use cantus_core::{InstrumentSpec, TrackId};

/// The undo label of loading a preset (the history names it).
pub const APPLY_LABEL: &str = "cmd.fx_preset";

/// What the browser keeps between frames.
#[derive(Debug, Default)]
pub struct Browser {
    pub open: bool,
    /// Where an applied preset goes.
    pub target: Target,
    /// Whose presets are listed.
    pub unit: Option<Unit>,
    pub query: Query,
    /// The unit's presets, read when the browser opens (and when a filter needs them
    /// again after a save).
    pub entries: Vec<Entry>,
    /// The preset the keyboard and the *Load* key act on.
    pub selected: Option<String>,
    /// The list's scroll, in points from its top.
    pub scroll: f32,
    /// Audition at the project's tempo (FL's *Sync BPM*), else at the phrases' 120.
    pub sync_bpm: bool,
    pub audition: Audition,
    /// The rows laid out last frame, on screen (published for tests).
    pub rows: Vec<(String, egui::Rect)>,
    /// Where the window was moved to (its top left), when it was.
    pub at: Option<egui::Pos2>,
    /// The last preset applied, by id.
    pub applied: Option<String>,
    /// Scroll the chosen row into view on the next frame (it was chosen by a key).
    pub reveal: bool,
    /// The preset ↑ / ↓ chose and when: heard once the selection rests
    /// (`views::preset_browser_keys::SETTLE`).
    pub pending: Option<(String, std::time::Instant)>,
    /// Put the keyboard in the search field on the next frame (a printable key started a
    /// search).
    pub focus_search: bool,
    /// How many presets the browser loaded into each insert (an effect's window names the
    /// loaded one again when it moves).
    pub loads: std::collections::HashMap<cantus_core::EffectId, u64>,
    /// The *Load* key's rect last frame (published for tests).
    pub load: Option<egui::Rect>,
}

impl Browser {
    /// The presets the filters leave, in their order.
    pub fn shown(&self) -> Vec<&Entry> {
        self.query
            .run(&self.entries)
            .into_iter()
            .map(|i| &self.entries[i])
            .collect()
    }

    /// The entry with this id.
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Move the selection by `by` among the shown presets (wrapping); returns the id.
    pub fn step(&mut self, by: i32) -> Option<String> {
        let shown: Vec<String> = self.shown().iter().map(|e| e.id.clone()).collect();
        if shown.is_empty() {
            return None;
        }
        let n = shown.len() as i32;
        let at = self
            .selected
            .as_ref()
            .and_then(|s| shown.iter().position(|x| x == s))
            .map(|i| i as i32)
            .unwrap_or(if by > 0 { -1 } else { 0 });
        let id = shown[(at + by).rem_euclid(n) as usize].clone();
        self.selected = Some(id.clone());
        Some(id)
    }
}

/// The unit a target loads presets into, in `app`'s project.
pub fn unit_of(app: &CantusApp, target: Target) -> Option<Unit> {
    match target {
        Target::Track(track) => match &app.project().track(track)?.instrument {
            InstrumentSpec::Native(n) => Some(Unit::Instrument(n.kind)),
            InstrumentSpec::Pulsus(_) => Some(Unit::Kit),
            _ => None,
        },
        Target::Effect { strip, effect } => {
            Some(Unit::Effect(app.project().effect(strip, effect)?.spec.kind))
        }
        Target::None => None,
    }
}

impl CantusApp {
    /// Open the browser on `target`'s presets. Nothing changes in the project.
    pub fn open_preset_browser(&mut self, target: Target) {
        let Some(unit) = unit_of(self, target) else {
            return;
        };
        self.stop_preset_audition();
        let b = &mut self.presets;
        if b.unit != Some(unit) {
            b.query = Query::default();
            b.scroll = 0.0;
        }
        b.open = true;
        b.target = target;
        b.unit = Some(unit);
        b.entries = entries(unit, true);
        let current = current_preset(self, target);
        self.presets.selected = current;
    }

    /// Close it, stopping what it plays.
    pub fn close_preset_browser(&mut self) {
        self.stop_preset_audition();
        self.presets.open = false;
    }

    /// Audition the preset `id` of the open browser (a second call on the one playing
    /// stops it).
    pub fn audition_preset_id(&mut self, id: &str) {
        let Some(entry) = self.presets.entry(id).cloned() else {
            return;
        };
        self.presets.selected = Some(entry.id.clone());
        let target = self.presets.target;
        self.audition_preset(target, &entry);
    }

    /// Apply the preset `id` to the browser's target: **one step of undo**, the unit's own
    /// preset command. Returns whether it was applied.
    pub fn apply_preset_id(&mut self, id: &str) -> bool {
        let Some(entry) = self.presets.entry(id).cloned() else {
            return false;
        };
        let target = self.presets.target;
        if unit_of(self, target) != Some(entry.unit) {
            return false;
        }
        if load_refused(self) {
            return false;
        }
        match (&entry.payload, target) {
            (Payload::Native(spec), Target::Track(track)) => {
                self.apply_inst_spec(track, spec, APPLY_LABEL);
                self.inst.preset.insert(track, entry.name.clone());
            }
            (Payload::Kit(kit), Target::Track(track)) => {
                apply_kit(self, track, &entry.id, kit.clone());
            }
            (Payload::Effect(values), Target::Effect { strip, effect }) => {
                self.apply_fx_params(strip, effect, values, APPLY_LABEL);
                *self.presets.loads.entry(effect).or_default() += 1;
            }
            _ => return false,
        }
        self.presets.selected = Some(entry.id.clone());
        self.presets.applied = Some(entry.id);
        true
    }
}

/// A kit loaded as Pulsus's own menu loads it, its name in the well.
fn apply_kit(app: &mut CantusApp, track: TrackId, id: &str, kit: cantus_core::PulsusParams) {
    if let Some(f) = cantus_core::PulsusParams::factory()
        .into_iter()
        .find(|f| f.label_key == id)
    {
        app.pulsus.kit_name.insert(track, f.label_key);
    }
    crate::views::pulsus_editor::load_kit(app, track, kit);
}

/// The preset the target was last set from, as the unit's own well names it.
fn current_preset(app: &CantusApp, target: Target) -> Option<String> {
    match target {
        Target::Track(track) => {
            if let Some(k) = app.pulsus.kit_name.get(&track) {
                return Some(k.to_string());
            }
            let name = app.inst.preset.get(&track)?;
            app.presets
                .entries
                .iter()
                .find(|e| &e.name == name)
                .map(|e| e.id.clone())
        }
        _ => None,
    }
}

impl Browser {
    /// How many presets the browser loaded into `effect`.
    pub fn loads_into(&self, effect: cantus_core::EffectId) -> u64 {
        self.loads.get(&effect).copied().unwrap_or(0)
    }
}

/// Whether loading a preset into the target would be refused because what it changes is
/// frozen (ADR 0064): the browser says *frozen — unfreeze to load* instead of offering
/// *Load*. The audition still plays the preset live.
pub fn load_refused(app: &CantusApp) -> bool {
    let p = app.project();
    let cmd = match app.presets.target {
        Target::Track(track) => match p.track(track) {
            Some(t) => cantus_core::Command::SetInstrument {
                track,
                spec: Box::new(t.instrument.clone()),
            },
            None => return false,
        },
        Target::Effect { strip, effect } => match p.effect(strip, effect) {
            Some(slot) => match slot.spec.kind.params().first() {
                Some(info) => cantus_core::Command::SetEffectParam {
                    strip,
                    effect,
                    key: info.key.to_string(),
                    value: slot.spec.get(info.key),
                },
                None => return false,
            },
            None => return false,
        },
        Target::None => return false,
    };
    cantus_core::freeze::refuses(p, &cmd).is_some()
}

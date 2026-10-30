//! What adding or swapping a device keeps (round 20, ADR 0029 §device-ux) — the work behind
//! [`CantusApp::apply_pick`](crate::app::CantusApp::apply_pick).
//!
//! - **Swapping an instrument** keeps the track: its clips and notes (a note is a pitch for
//!   a kit and a synth alike, so a kit → melodic swap keeps every note), its mixer strip and
//!   the inserts on it, its colour, and its name — unless the name was a default one ("Track
//!   3", or the old instrument's own name), which then follows the new instrument, as FL
//!   names a channel after its plug-in. Between two native instruments the **matching
//!   settings** come along when [`Keep::matching`] is on (the default): the eight macros
//!   and their names, every value whose key the new kind also has, and the modulation
//!   connections whose source and target it also has. One step of undo.
//! - **Swapping an effect** keeps the slot's **position** in the chain, its mix and bypass,
//!   and its key (sidechain) and note source where the new kind takes them — one
//!   `ReplaceEffect`, one step of undo. The editor that was open on the old one opens on
//!   the new one, where the old one was.
//! - **Adding** an instrument makes a track named after it (numbered when the name is
//!   taken) at the place asked; adding an effect puts it at the end of the chain. Either
//!   opens the new device's window, as FL does.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::picker::{PickChoice, PickPurpose};
use crate::views::workspace_device as device;
use cantus_core::inst::modulation::{self, MACRO_KEYS, MACRO_NAME_KEYS};
use cantus_core::{
    Command, EffectId, EffectSlot, InstrumentKind, InstrumentSpec, NativeSpec, PulsusParams,
    StripId, TrackId,
};

/// What a swap keeps beyond what it always keeps. Lives on the application, so the picker
/// can offer the choice ("Keep matching settings") and remember it for the session.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Keep {
    /// Between two native instruments: macros, shared values, connections that still
    /// make sense.
    pub matching: bool,
}

impl Default for Keep {
    fn default() -> Self {
        Keep { matching: true }
    }
}

/// Whether `choice` can do what `purpose` asks today: an instrument for an instrument's
/// purpose, an effect for an effect's. A hosted **instrument** cannot yet — it arrives with
/// ADR 0028 (the hosted-inst work), and the picker shows its tile disabled until then.
pub fn available(purpose: &PickPurpose, choice: &PickChoice) -> bool {
    use cantus_host::PluginKind;
    match choice {
        PickChoice::Firmus
        | PickChoice::Pulsus
        | PickChoice::Instrument(_)
        | PickChoice::Sound(_) => purpose.wants_instrument(),
        PickChoice::Effect(_) => !purpose.wants_instrument(),
        // A track template is a track of its own: added, never swapped in (ADR 0072).
        PickChoice::TrackTemplate(_) => matches!(purpose, PickPurpose::AddInstrument { .. }),
        PickChoice::Hosted(info) => match info.kind {
            // ADR 0028: `apply_pick` puts it on a track through `set_hosted_instrument`.
            PluginKind::Instrument => purpose.wants_instrument(),
            PluginKind::Effect => !purpose.wants_instrument(),
            PluginKind::NoteEffect | PluginKind::Other => false,
        },
    }
}

/// The instrument a choice stands for, as a fresh spec (a kit's pad names are put in the
/// interface's language by `set_instrument`).
pub fn spec_of(choice: &PickChoice) -> Option<InstrumentSpec> {
    match choice {
        PickChoice::Firmus => Some(InstrumentSpec::default()),
        PickChoice::Pulsus => Some(InstrumentSpec::Pulsus(PulsusParams::default())),
        // r33:mpe — a new instance comes with its MPE routes (ADR 0080)
        PickChoice::Instrument(k) => Some(InstrumentSpec::Native(
            cantus_core::model::note_expr::defaults::with_routes(NativeSpec::new(*k)),
        )),
        _ => None,
    }
}

/// An instrument's name, in the interface's language.
pub fn instrument_name(spec: &InstrumentSpec) -> String {
    match spec {
        InstrumentSpec::Native(n) => t(n.kind.name_key()),
        InstrumentSpec::Pulsus(_) => t("instrument.pulsus"),
        _ => t("instrument.firmus"),
    }
}

/// Whether two specs are the same instrument (a swap to it changes nothing).
fn same_instrument(a: &InstrumentSpec, b: &InstrumentSpec) -> bool {
    match (a, b) {
        (InstrumentSpec::Native(x), InstrumentSpec::Native(y)) => x.kind == y.kind,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

/// Whether `name` is one a track gets without anyone choosing it: empty, "Track 3" (in the
/// interface's language), or the name of the instrument it hosts.
pub fn is_default_name(name: &str, hosted: &InstrumentSpec) -> bool {
    let name = name.trim();
    if name.is_empty() || name == instrument_name(hosted) {
        return true;
    }
    // "Track {n}": the pattern with the number taken out.
    let pattern = tf("track.new_name", &[("n", &"\u{0}")]);
    match pattern.split_once('\u{0}') {
        Some((before, after)) => {
            name.len() > before.len() + after.len()
                && name.starts_with(before)
                && name.ends_with(after)
                && name[before.len()..name.len() - after.len()]
                    .chars()
                    .all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// The new native instrument `to`, with what it can take from `from`: the macros and their
/// names, every number whose key `to` also has (clamped to its range), every text it also
/// takes that is not a file, and the connections whose source it produces and whose target
/// it has. Returns the spec and how many settings came along.
pub fn carry_over(from: &NativeSpec, to: InstrumentKind) -> (NativeSpec, usize) {
    let mut out = NativeSpec::new(to);
    let mut kept = 0;
    // "mod12.amt": a connection slot's key, carried below as a connection, not as a value.
    let is_slot_key = |k: &str| {
        k.strip_prefix("mod")
            .and_then(|r| r.split_once('.'))
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    };
    for (key, v) in &from.params {
        if is_slot_key(key) {
            continue;
        }
        let shared = MACRO_KEYS.contains(&key.as_str()) || to.param(key).is_some();
        if shared && out.set(key, *v).is_some() {
            kept += 1;
        }
    }
    for (key, text) in &from.text {
        if is_slot_key(key) || to.is_asset_key(key) || from.kind.is_asset_key(key) {
            continue;
        }
        if MACRO_NAME_KEYS.contains(&key.as_str()) || to.is_text_key(key) {
            out.text.insert(key.clone(), text.clone());
            kept += 1;
        }
    }
    // Connections: compacted into the new spec's slots from the first, keeping only those
    // whose source the new kind produces and whose target it has.
    let mut slot = 0;
    for c in modulation::connections(from) {
        let src_ok = to.sources().contains(&c.src) || c.src.starts_with("macro");
        // Chorda's tuning takes no source (sound review 2).
        let dst_ok = to.param(c.dst).is_some() && to.modulatable(c.dst);
        if !(src_ok && dst_ok) || slot >= modulation::MOD_SLOTS {
            continue;
        }
        let keys = &modulation::SLOTS[slot];
        out.text.insert(keys.src.to_string(), c.src.to_string());
        out.text.insert(keys.dst.to_string(), c.dst.to_string());
        if let Some(via) = c
            .via
            .filter(|v| to.sources().contains(v) || v.starts_with("macro"))
        {
            out.text.insert(keys.via.to_string(), via.to_string());
        }
        let _ = out.set(keys.amt, c.amount);
        let _ = out.set(keys.curve, c.curve as u8 as f32);
        let _ = out.set(keys.on, if c.on { 1.0 } else { 0.0 });
        slot += 1;
        kept += 1;
    }
    (out, kept)
}

/// A track name not yet taken: `base`, else "base 2", "base 3" and so on.
fn unique_track_name(app: &CantusApp, base: &str) -> String {
    let taken = |n: &str| app.project().tracks.iter().any(|t| t.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} {i}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

/// A fresh kit's pad names in the interface's language, as `set_instrument` gives them.
fn localized(spec: InstrumentSpec) -> InstrumentSpec {
    match spec {
        InstrumentSpec::Pulsus(mut kit) => {
            for pad in &mut kit.pads {
                pad.name = crate::app::localized_pad_name(pad);
            }
            InstrumentSpec::Pulsus(kit)
        }
        other => other,
    }
}

impl CantusApp {
    /// Whether an editor is open on an effect.
    fn effect_window_open(&self, strip: StripId, effect: EffectId) -> bool {
        self.fx
            .open
            .is_some_and(|(s, e, _)| (s, e) == (strip, effect))
            || self.eq_editor.open == Some((strip, effect))
            || self.comp_editor.open == Some((strip, effect))
    }

    /// *Swap instrument*: give `track` the instrument `choice` stands for, keeping what
    /// the module docs say, as one step of undo. Returns whether anything changed.
    pub fn replace_instrument(&mut self, track: TrackId, choice: &PickChoice) -> bool {
        let Some(fresh) = spec_of(choice) else {
            return false;
        };
        let Some(tr) = self.project().track(track) else {
            return false;
        };
        let (old, name) = (tr.instrument.clone(), tr.name.clone());
        if same_instrument(&old, &fresh) {
            return false;
        }
        let (new, kept) = match (&old, fresh) {
            (InstrumentSpec::Native(from), InstrumentSpec::Native(to))
                if self.swap_keep.matching =>
            {
                let (spec, kept) = carry_over(from, to.kind);
                (InstrumentSpec::Native(spec), kept)
            }
            (_, spec) => (localized(spec), 0),
        };
        let new_name = instrument_name(&new);
        let mut commands = vec![Command::SetInstrument {
            track,
            spec: Box::new(new),
        }];
        if is_default_name(&name, &old) && name != new_name {
            commands.push(Command::SetTrackName {
                id: track,
                name: new_name.clone(),
            });
        }
        let was_open = self.instrument_window_open(track);
        let old_key = self.instrument_window_key(track);
        if !self.exec(Command::Batch {
            label_key: "cmd.replace_instrument".into(),
            commands,
        }) {
            return false;
        }
        self.rack.selected_pad = 0;
        self.inst.forget(track);
        self.pulsus.kit_name.remove(&track);
        // The window that showed the old instrument shows the new one, where it was.
        let new_key = self.instrument_window_key(track);
        if was_open && new_key != old_key {
            if let (Some(ctx), Some(a), Some(b)) = (self.theme_ctx.clone(), old_key, new_key) {
                device::inherit_place(&ctx, a, b);
            }
            self.close_instrument_window(track);
            if new_key.is_some() {
                self.open_instrument_window(track);
            }
        }
        self.set_status(if kept > 0 {
            tf(
                "swap.status.instrument_kept",
                &[("name", &new_name), ("n", &kept)],
            )
        } else {
            tf("swap.status.instrument", &[("name", &new_name)])
        });
        true
    }

    /// *Add track*: a new track hosting `choice`'s instrument, named after it, at `index`
    /// (the end when `None`), selected, its window open. One step of undo.
    pub fn add_instrument_track(&mut self, index: Option<usize>, choice: &PickChoice) -> bool {
        let Some(spec) = spec_of(choice) else {
            return false;
        };
        let spec = localized(spec);
        let name = unique_track_name(self, &instrument_name(&spec));
        let windowed = !matches!(spec, InstrumentSpec::Firmus(_));
        let mut track = self.bus.new_track(name);
        track.instrument = spec;
        let id = track.id;
        let len = self.project().tracks.len();
        let index = index.unwrap_or(len).min(len);
        if !self.exec(Command::InsertTrack {
            index,
            track: Box::new(track),
        }) {
            return false;
        }
        self.select_track(id);
        if windowed {
            self.open_instrument_window(id);
        }
        true
    }

    /// *Add effect*: `choice` at the end of `strip`'s chain, its editor open. One step.
    pub fn add_picked_effect(&mut self, strip: StripId, choice: &PickChoice) -> bool {
        let before = self.project().effects_of(strip).map_or(0, |c| c.len());
        match choice {
            PickChoice::Effect(kind) => self.add_effect(strip, *kind),
            PickChoice::Hosted(info) => self.add_plugin(strip, info),
            _ => return false,
        }
        let added = self
            .project()
            .effects_of(strip)
            .filter(|c| c.len() > before)
            .and_then(|c| c.last())
            .map(|e| e.id);
        if let Some(id) = added {
            self.open_effect_editor(strip, id);
        }
        added.is_some()
    }

    /// The commands that point the automation of `old`'s parameters at `new`, for every
    /// parameter the new kind also has (by key).
    /// The project's controller mappings on the old slot, pointed at the new one: its
    /// bypass and mix (which the swap keeps), a native parameter the new kind also has, and
    /// a hosted parameter — learnt on the old plug-in, so its row says *Other plug-in* and
    /// its control is free until learnt again (an undo of the swap brings it back).
    fn retarget_mappings(
        &self,
        old: EffectId,
        new: EffectId,
        kind: cantus_core::EffectKind,
    ) -> Vec<Command> {
        use cantus_core::controller::MappingTarget as T;
        use cantus_core::AutoTarget as A;
        let p = self.project();
        let mut out = Vec::new();
        for m in &p.controllers.mappings {
            let target = match &m.target {
                T::EffectBypass(e) if *e == old => T::EffectBypass(new),
                T::EffectMix(e) if *e == old => T::EffectMix(new),
                T::Param(A::Effect { effect, key })
                    if *effect == old && kind.param(key).is_some() =>
                {
                    T::Param(A::Effect {
                        effect: new,
                        key: key.clone(),
                    })
                }
                T::Param(A::Plugin { effect, id }) if *effect == old => T::Param(A::Plugin {
                    effect: new,
                    id: *id,
                }),
                _ => continue,
            };
            let mut moved = m.clone();
            // A hosted mapping keeps the plug-in it was learnt on.
            if moved.plugin.is_none() {
                moved.plugin = m.target.plugin_of(p);
            }
            moved.target = target;
            out.push(Command::SetMapping {
                mapping: Box::new(moved),
            });
        }
        out
    }

    fn retarget_automation(
        &self,
        old: EffectId,
        new: EffectId,
        kind: cantus_core::EffectKind,
    ) -> Vec<Command> {
        let mut out = Vec::new();
        for t in &self.project().tracks {
            for c in &t.clips {
                let Some(a) = c.automation.as_ref() else {
                    continue;
                };
                if let cantus_core::AutoTarget::Effect { effect, key } = &a.target {
                    if *effect == old && kind.param(key).is_some() {
                        let mut a = a.clone();
                        a.target = cantus_core::AutoTarget::Effect {
                            effect: new,
                            key: key.clone(),
                        };
                        out.push(Command::SetClipAutomation {
                            track: t.id,
                            clip: c.id,
                            automation: Some(a),
                        });
                    }
                }
            }
        }
        out
    }

    /// *Swap effect*: `choice` in `effect`'s place on `strip` (the same position, its mix
    /// and bypass, its key and note source where the new kind takes them) as one
    /// `ReplaceEffect`. The editor open on the old one opens on the new one, where it was.
    pub fn replace_effect_with(
        &mut self,
        strip: StripId,
        effect: EffectId,
        choice: &PickChoice,
    ) -> bool {
        let Some(old) = self.project().effect(strip, effect).cloned() else {
            return false;
        };
        let mut slot: EffectSlot = match choice {
            PickChoice::Effect(kind) => {
                if old.spec.kind == *kind && old.spec.external.is_none() {
                    return false;
                }
                // As an insert starts (ADR 0071).
                self.new_insert(*kind)
            }
            PickChoice::Hosted(info) => {
                let same = old
                    .spec
                    .external
                    .as_deref()
                    .is_some_and(|p| p.id == info.id);
                if same {
                    return false;
                }
                match self.plugins.make_slot(&mut self.bus, strip, info) {
                    Ok(s) => s,
                    Err(e) => {
                        self.set_error(tf(
                            "host.load_failed",
                            &[("name", &info.name), ("error", &e)],
                        ));
                        return false;
                    }
                }
            }
            _ => return false,
        };
        slot.mix = old.mix;
        slot.bypass = old.bypass;
        slot.sidechain = old.sidechain.filter(|_| slot.spec.takes_sidechain());
        slot.midi_from = old.midi_from.filter(|_| slot.spec.kind.takes_notes());
        let new_id = slot.id;
        let kind = slot.spec.kind;
        let was_open = self.effect_window_open(strip, effect);
        // Automation follows the slot (ADR 0027): a lane on a parameter the new kind also
        // has is pointed at the new effect in the same undo step; one on a parameter it
        // lacks keeps its clips and goes quiet.
        let mut retarget = self.retarget_automation(effect, new_id, kind);
        // The controllers follow the slot too (round 29, review 3 of ADR 0062, #54).
        retarget.extend(self.retarget_mappings(effect, new_id, kind));
        let swap = |slot: &EffectSlot| {
            let mut commands = vec![Command::ReplaceEffect {
                strip,
                effect,
                slot: Box::new(slot.clone()),
            }];
            commands.extend(retarget.iter().cloned());
            Command::Batch {
                label_key: "cmd.replace_effect".into(),
                commands,
            }
        };
        // An isolated plug-in's values, known once it loads, join this step.
        let load = cantus_host::PluginHost::load_key(new_id);
        let replaced = self.exec_merging(swap(&slot), load) || {
            // The key would now close a loop (the routes moved since): swap without it.
            slot.sidechain = None;
            slot.midi_from = None;
            self.exec_merging(swap(&slot), load)
        };
        if !replaced {
            return false;
        }
        if was_open {
            if let Some(ctx) = self.theme_ctx.clone() {
                let a = crate::views::fx::device_id(effect).value();
                let b = crate::views::fx::device_id(new_id).value();
                device::inherit_place(&ctx, a, b);
            }
            self.close_eq_editor();
            self.close_comp_editor();
            self.close_fx_editor();
            if kind.has_editor() {
                self.open_effect_editor(strip, new_id);
            }
        }
        let name = match choice {
            PickChoice::Hosted(info) => info.name.clone(),
            _ => t(kind.name_key()),
        };
        self.set_status(tf("swap.status.effect", &[("name", &name)]));
        true
    }
}

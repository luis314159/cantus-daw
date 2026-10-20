//! Modulators in the interface (ADR 0057): where they live (the Modulators section of the
//! inspector, for the track, its channel and the master — `section`), how a parameter is
//! connected to one (*Modulate…* in any control's menu — `popup`; a modulator's handle
//! dropped on a control; Bitwig's *mapping* mode), how a depth is edited (Alt+drag on the
//! control, or the route's row), what a control shows (rings, the live dot, the mark), and
//! *Bake to automation*.
//!
//! Every edit is one command — `InsertModulator`, `RemoveModulator`, or the whole
//! modulator in `SetModulator` — and a drag merges into one undo step.

#[path = "modulator_bake.rs"]
pub mod bake;
#[path = "modulator_paint.rs"]
pub mod paint;
#[path = "modulator_popup.rs"]
pub mod popup;
#[path = "modulator_record.rs"]
pub mod record;

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::automate::{Choice, ModOffer};
use crate::kit::{Function, Style};
use cantus_core::automation::range::{base_value, range_of};
use cantus_core::modulator::{ModHost, ModKind, ModRoute, Modulator};
use cantus_core::{AutoTarget, Command, MergeKey, ModulatorId, Project};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// The depth a new route starts at: a quarter of the travel.
pub const NEW_DEPTH: f32 = 0.25;
/// Rings a control draws at most.
pub const MAX_RINGS: usize = 4;

/// The interface's own modulator state.
#[derive(Default)]
pub struct ModUi {
    /// Bitwig's mapping mode: a drag on any modulatable control sets this modulator's
    /// depth there (and connects it).
    pub mapping: Option<ModulatorId>,
    /// The *Modulate…* list open for a parameter, and where its control is.
    pub popup: Option<(AutoTarget, egui::Rect)>,
    /// Modulators folded to their title row in the panel.
    pub folded: HashSet<ModulatorId>,
    /// The kinds a host's **+** offers, open under its rect.
    pub add_menu: Option<(ModHost, egui::Rect)>,
    /// A setting's list open (whose audio, whose notes), under its field.
    pub list: Option<(ModulatorId, ListKind, egui::Rect)>,
    /// A bake that waits for its offline trace (a source that follows audio).
    pub pending_bake: Option<bake::Pending>,
    /// The automation a recording pass has written so far.
    pub rec: record::AutoRec,
    /// Every route into each target, at a revision: (modulator, depth, its kind).
    cache: RefCell<Option<(u64, std::sync::Arc<Routes>)>>,
}

/// The lists a modulator's settings open.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ListKind {
    /// The strip (or the master) whose audio a follower or an envelope hears.
    Listen,
    /// The track whose notes an envelope or an LFO hears.
    Notes,
}

type Routes = HashMap<AutoTarget, Vec<(ModulatorId, f32, ModKind)>>;

impl std::fmt::Debug for ModUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModUi")
            .field("mapping", &self.mapping)
            .field("popup", &self.popup)
            .finish_non_exhaustive()
    }
}

/// The hue a kind wears: on its rings, its title, its handle.
pub fn function_of(kind: ModKind) -> Function {
    match kind {
        ModKind::Lfo => Function::Mod,
        ModKind::Envelope => Function::Dynamics,
        ModKind::Follower => Function::Output,
        ModKind::Step => Function::Source,
        ModKind::Random => Function::Space,
    }
}

/// A modulator's name: its own, else its kind and its number among its host's modulators
/// of that kind ("LFO 2").
pub fn mod_name(project: &Project, id: ModulatorId) -> Option<String> {
    let (host, i, m) = project.modulator(id)?;
    if !m.name.is_empty() {
        return Some(m.name.clone());
    }
    let kind = m.source.kind();
    let n = project.modulators(host)?[..=i]
        .iter()
        .filter(|x| x.source.kind() == kind)
        .count();
    Some(tf(
        "mods.name.numbered",
        &[("kind", &t(kind.name_key())), ("n", &n.to_string())],
    ))
}

/// Where a modulator for `target` naturally lives: the track the parameter is on, the
/// strip (an insert's, a fader's), or the master.
pub fn host_for(
    project: &Project,
    target: &AutoTarget,
    selected: Option<cantus_core::TrackId>,
) -> ModHost {
    let strip_of_effect = |e| project.effect_host(e);
    match target {
        AutoTarget::TrackGain(t)
        | AutoTarget::TrackPan(t)
        | AutoTarget::Instrument { track: t, .. }
        | AutoTarget::Firmus { track: t, .. }
        | AutoTarget::PluginInstrument { track: t, .. } => ModHost::Track(*t),
        AutoTarget::StripGain(s)
        | AutoTarget::StripPan(s)
        | AutoTarget::StripWidth(s)
        | AutoTarget::Send { strip: s, .. } => ModHost::Strip(*s),
        AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. } => {
            match strip_of_effect(*effect) {
                Some(s) if !s.is_master() => ModHost::Strip(s),
                _ => ModHost::Master,
            }
        }
        AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth => ModHost::Master,
        AutoTarget::ModAmount(m) => project
            .modulator(*m)
            .map(|(h, _, _)| h)
            .or(selected.map(ModHost::Track))
            .unwrap_or(ModHost::Master),
    }
}

impl CantusApp {
    /// Every route into each target, compiled once per project revision.
    fn mod_routes(&self) -> std::sync::Arc<Routes> {
        let rev = self.bus.revision();
        let mut cache = self.modulators.cache.borrow_mut();
        if let Some((r, routes)) = cache.as_ref() {
            if *r == rev {
                return routes.clone();
            }
        }
        let mut map: Routes = HashMap::new();
        for (_, m) in self.project().all_modulators() {
            for r in &m.routes {
                map.entry(r.target.clone())
                    .or_default()
                    .push((m.id, r.depth, m.source.kind()));
            }
        }
        let routes = std::sync::Arc::new(map);
        *cache = Some((rev, routes.clone()));
        routes
    }

    /// Whether a modulator drives `target`.
    pub fn is_modulated(&self, target: &AutoTarget) -> bool {
        self.mod_routes().contains_key(target)
    }

    /// What a control bound to `target` shows and offers of its modulation: its rings
    /// (into `rings`, returning how many), the depth a depth drag edits, whether a
    /// modulator is mapping, and where the parameter is now. `base` is the control's
    /// position (0…1) without modulation.
    pub fn mod_offer(
        &self,
        target: &AutoTarget,
        base: f32,
        s: &Style,
        rings: &mut [(f32, egui::Color32); MAX_RINGS],
    ) -> (ModOffer, usize) {
        crate::kit::automate::probe::note_offer(target);
        let routes = self.mod_routes();
        let list = routes.get(target);
        let mut n = 0;
        let mut live = base;
        let tele = self.engine.modulation();
        let muted = |id: ModulatorId| {
            self.project()
                .modulator(id)
                .is_some_and(|(_, _, m)| m.muted)
        };
        for (id, depth, kind) in list.into_iter().flatten() {
            if muted(*id) {
                continue;
            }
            if n < MAX_RINGS {
                rings[n] = (*depth, s.t.hue(function_of(*kind)));
                n += 1;
            }
            if let Some(v) = tele.value(*id) {
                live += depth * v;
            }
        }
        let mapping = self
            .modulators
            .mapping
            .filter(|m| self.project().modulator(*m).is_some());
        let depth = match mapping {
            Some(m) => Some(
                list.and_then(|l| l.iter().find(|(id, ..)| *id == m))
                    .map_or(0.0, |x| x.1),
            ),
            None => list.and_then(|l| l.first()).map(|x| x.1),
        };
        let offer = ModOffer {
            depth,
            mapping: mapping.is_some(),
            live: (n > 0).then_some(live.clamp(0.0, 1.0)),
            // MIDI learn (round 29, ADR 0062).
            learn: Some(
                self.learn_offer(&cantus_core::controller::MappingTarget::Param(
                    target.clone(),
                )),
            ),
        };
        (offer, n)
    }

    /// The position (0…1) `target` has where nothing modulates it now: its automation's
    /// where the song is, else the project's value (a hosted parameter's, normalised by
    /// the loaded plug-in's range).
    pub fn base_pos(&self, target: &AutoTarget) -> Option<f32> {
        let p = self.project();
        let range = range_of(p, target)?;
        let auto = self.automated_value(target).filter(|v| v.is_finite());
        if target.is_plugin() {
            // A hosted parameter's curve is its normalised value already.
            return auto.or_else(|| self.plugin_pos(target));
        }
        let v = auto.or_else(|| base_value(p, target))?;
        v.is_finite().then(|| range.pos(v))
    }

    /// Act on what a control bound to `target` reported of its modulation (ADR 0057):
    /// open *Modulate…* at `rect`, set a depth (a drag merges), end the drag, connect a
    /// dropped handle.
    pub fn modulation_choice(&mut self, target: &AutoTarget, choice: Choice, rect: egui::Rect) {
        // MIDI learn (round 29, ADR 0062): every parameter that offers it.
        match choice {
            Choice::Learn => {
                self.learn_target(cantus_core::controller::MappingTarget::Param(
                    target.clone(),
                ));
                return;
            }
            Choice::Forget => {
                self.forget_target(&cantus_core::controller::MappingTarget::Param(
                    target.clone(),
                ));
                return;
            }
            _ => {}
        }
        if !self.modulatable(target) {
            return;
        }
        match choice {
            Choice::Modulate => self.modulators.popup = Some((target.clone(), rect)),
            Choice::Depth(d) => {
                let id = self.modulators.mapping.or_else(|| {
                    self.mod_routes()
                        .get(target)
                        .and_then(|l| l.first())
                        .map(|x| x.0)
                });
                if let Some(id) = id {
                    self.set_route_depth(id, target, d, true);
                }
            }
            Choice::DepthDone => self.end_gesture(),
            Choice::Connect(id) => {
                self.connect(ModulatorId(id), target.clone());
            }
            Choice::Automate | Choice::Type | Choice::Reset | Choice::Learn | Choice::Forget => {}
        }
    }

    /// Whether `target` can be modulated at all: a native instrument's key that is set,
    /// never modulated (Chorda's tuning, `InstrumentKind::modulatable`) cannot — every
    /// path into a route asks here (integration review).
    pub fn modulatable(&self, target: &AutoTarget) -> bool {
        match target {
            AutoTarget::Instrument { track, key } => self
                .project()
                .track(*track)
                .and_then(|t| t.instrument.native())
                .is_none_or(|s| s.kind.modulatable(key)),
            _ => true,
        }
    }

    /// Put a new modulator of `kind` on `host` (one undo step), driving `target` when one
    /// is given; it is then the panel's newest.
    pub fn add_modulator(
        &mut self,
        host: ModHost,
        kind: ModKind,
        target: Option<AutoTarget>,
    ) -> Option<ModulatorId> {
        if target.as_ref().is_some_and(|t| !self.modulatable(t)) {
            self.set_error(t("mod.not_modulatable"));
            return None;
        }
        let index = self.project().modulators(host)?.len();
        if index >= cantus_core::modulator::MAX_PER_HOST {
            self.set_error(t("mods.status.full"));
            return None;
        }
        let id = ModulatorId(self.bus.alloc_id());
        let mut m = Modulator::new(id, kind);
        if let Some(target) = target {
            m.routes.push(ModRoute {
                target,
                depth: NEW_DEPTH,
            });
        }
        self.exec(Command::InsertModulator {
            host,
            index,
            modulator: Box::new(m),
        })
        .then_some(id)
    }

    /// Connect modulator `id` to `target` at the new-route depth; one undo step. Already
    /// connected: nothing.
    pub fn connect(&mut self, id: ModulatorId, target: AutoTarget) -> bool {
        if !self.modulatable(&target) {
            self.set_error(t("mod.not_modulatable"));
            return false;
        }
        let Some((_, _, m)) = self.project().modulator(id) else {
            return false;
        };
        if m.route(&target).is_some() || target == AutoTarget::ModAmount(id) {
            return false;
        }
        if m.routes.len() >= cantus_core::modulator::MAX_ROUTES {
            self.set_error(t("mods.status.routes_full"));
            return false;
        }
        let mut m = m.clone();
        m.routes.push(ModRoute {
            target,
            depth: NEW_DEPTH,
        });
        self.edit_modulator(m, None)
    }

    /// Take `target` out of modulator `id`'s routes; one undo step.
    pub fn disconnect(&mut self, id: ModulatorId, target: &AutoTarget) -> bool {
        let Some((_, _, m)) = self.project().modulator(id) else {
            return false;
        };
        let mut m = m.clone();
        m.routes.retain(|r| &r.target != target);
        self.edit_modulator(m, None)
    }

    /// Set the depth of modulator `id`'s route into `target` (making the route when it
    /// has none, as a mapping drag does). A drag merges into one undo step.
    pub fn set_route_depth(
        &mut self,
        id: ModulatorId,
        target: &AutoTarget,
        depth: f32,
        drag: bool,
    ) {
        let Some((_, _, m)) = self.project().modulator(id) else {
            return;
        };
        let depth = if depth.is_finite() {
            depth.clamp(-1.0, 1.0)
        } else {
            return;
        };
        let mut m = m.clone();
        let room = m.routes.len() < cantus_core::modulator::MAX_ROUTES;
        match m.routes.iter_mut().find(|r| &r.target == target) {
            Some(r) if r.depth == depth => return,
            Some(r) => r.depth = depth,
            None if target != &AutoTarget::ModAmount(id) && room => m.routes.push(ModRoute {
                target: target.clone(),
                depth,
            }),
            None => return,
        }
        let key = drag.then(|| MergeKey::of(&("mod-depth", id.0, target)));
        self.edit_modulator(m, key);
    }

    /// Replace a modulator whole (one command; merged under `key` while a drag goes on).
    pub fn edit_modulator(&mut self, m: Modulator, key: Option<MergeKey>) -> bool {
        // Only its amount moved: the amount's own command, which a pass records and *last
        // tweaked* names, as a controller's (round 29, review 2 of ADR 0062, #21).
        let amount_only = self.project().modulator(m.id).is_some_and(|(_, _, old)| {
            old.amount != m.amount && {
                let mut same = old.clone();
                same.amount = m.amount;
                same == m
            }
        });
        let cmd = if amount_only {
            Command::SetModulatorAmount {
                id: m.id,
                amount: m.amount,
            }
        } else {
            Command::SetModulator {
                modulator: Box::new(m),
            }
        };
        match key {
            Some(k) => self.exec_merging(cmd, k),
            None => self.exec(cmd),
        }
    }

    /// Take a modulator away (its routes with it); one undo step.
    pub fn remove_modulator(&mut self, id: ModulatorId) -> bool {
        if self.modulators.mapping == Some(id) {
            self.modulators.mapping = None;
        }
        self.exec(Command::RemoveModulator { id })
    }

    /// Start or stop mapping with modulator `id` (Bitwig's mode).
    pub fn toggle_mapping(&mut self, id: ModulatorId) {
        self.modulators.mapping = if self.modulators.mapping == Some(id) {
            None
        } else {
            Some(id)
        };
        if let Some(name) = self
            .modulators
            .mapping
            .and_then(|m| mod_name(self.project(), m))
        {
            self.set_status(tf("mods.status.mapping", &[("name", &name)]));
        }
    }

    /// Forget what names a modulator no longer there (an undo, another project).
    pub fn validate_modulators(&mut self) {
        let p = self.bus.project();
        if self
            .modulators
            .mapping
            .is_some_and(|m| p.modulator(m).is_none())
        {
            self.modulators.mapping = None;
        }
        if let Some((target, _)) = &self.modulators.popup {
            if range_of(p, target).is_none() {
                self.modulators.popup = None;
            }
        }
        self.modulators.folded.retain(|m| p.modulator(*m).is_some());
    }
}

impl CantusApp {
    /// A target in words, "Device · Parameter" — a hosted plug-in's parameter by its own
    /// name, from the plug-in's list.
    pub fn target_name(&self, target: &AutoTarget) -> String {
        if let Some((device, param)) = self.plugin_param(target) {
            return format!("{device} \u{b7} {}", param.name);
        }
        crate::automation::target_name(self.project(), target)
    }

    /// A hosted plug-in's name and the parameter `target` names, from the loaded plug-in.
    pub fn plugin_param(&self, target: &AutoTarget) -> Option<(String, &cantus_host::ParamInfo)> {
        match target {
            AutoTarget::Plugin { effect, id } => {
                let h = self.plugins.hosted(*effect)?;
                let p = h.params.iter().find(|p| p.id == *id)?;
                Some((h.info.name.clone(), p))
            }
            AutoTarget::PluginInstrument { track, id } => {
                let h = self.plugins.hosted_instrument(*track)?;
                let p = h.params.iter().find(|p| p.id == *id)?;
                Some((h.info.name.clone(), p))
            }
            _ => None,
        }
    }

    /// A hosted parameter's position (0…1 of its range) as the project holds it: what a
    /// modulator offsets where no clip automates it.
    pub fn plugin_pos(&self, target: &AutoTarget) -> Option<f32> {
        let (_, info) = self.plugin_param(target)?;
        let p = self.project();
        let value = match target {
            AutoTarget::Plugin { effect, id } => p
                .effect_host(*effect)
                .and_then(|h| p.effect(h, *effect))
                .and_then(|e| e.spec.external.as_deref())
                .and_then(|x| x.params.get(id).copied()),
            AutoTarget::PluginInstrument { track, id } => p
                .track(*track)
                .and_then(|t| t.instrument.external())
                .and_then(|x| x.params.get(id).copied()),
            _ => None,
        }
        .unwrap_or(info.default);
        let span = info.max - info.min;
        Some(if span.abs() > 0.0 {
            ((value - info.min) / span).clamp(0.0, 1.0) as f32
        } else {
            0.0
        })
    }

    /// Show `host`'s modulators in the inspector (a new one was just made there).
    pub fn reveal_modulators(&mut self, host: ModHost) {
        match host {
            ModHost::Track(t) => self.select_track(t),
            ModHost::Strip(s) => self.select_strip(s),
            ModHost::Master => self.select_master(),
        }
        self.inspector.collapsed = false;
    }
}

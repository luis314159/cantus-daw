//! A render's own instances of the hosted plug-ins (ADR 0019, ADR 0028), made **before**
//! the render begins — without the interface ever waiting on a helper (round 29, review
//! isolation-5). A child of `session`.
//!
//! - **Ready first.** Whatever renders (an export, a bake, a freeze, a bounce) asks
//!   [`PluginHost::render_ready`] each frame before it builds its render, as it waits for
//!   samples. The first call asks every live plug-in for its state as it is now (a render
//!   from the live states; a freeze's keyed render takes the project's); once they are in,
//!   it makes the render's instances — an isolated one loads in a helper of its own, with
//!   its state and values, while the frames go on. It answers `true` once every instance
//!   is loaded, or lost. Within Cantus all of that happens in the first call.
//! - **Units.** [`PluginHost::render_units`] and [`PluginHost::render_instruments`] then
//!   make the units from those instances; their main halves stay until
//!   [`PluginHost::finish_render`].
//! - **One at a time, its owner's.** Each preparation belongs to who asked
//!   ([`RenderFor`]: an export, a bake, a freeze or bounce) and to what it renders (every
//!   part's values, and the states when they are the project's): asked for another, it is
//!   made anew; another owner never takes it while it is in use — its owner asking each
//!   frame, or its render running — and waits (review isolation-6 #1, #4, #13, #26). An
//!   owner that gives up drops it ([`PluginHost::drop_render`]); one nobody asked about
//!   for a while goes by itself.
//! - **Waited for.** A heard plug-in not loaded live yet — let go a moment ago (a device
//!   change), its state on its way, or not synced yet — is waited for, then made a part of
//!   the render; only a frozen chain's resting ones are none of it (review final-B #1). A
//!   preparation asked for again with other contents keeps each instance whose part is the
//!   same, so a state that moves at every look in one plug-in never makes another's load
//!   again (review final-B #10).
//! - **Losses, said.** A heard plug-in that is not loaded live (it could not be loaded, or
//!   crashed loading), or whose render instance never loaded, is a loss of the render
//!   ([`RenderLoss::NeverLoaded`], review isolation-5 #27); a state that could not be had
//!   as it is now — its helper held by a window, or too slow — is used as it was last said,
//!   and the render says so ([`RenderLoss::Stale`]): never a stale state as a fresh one.

use super::iso::{Instance, RenderFailure, RenderLoss};
use super::{Parked, PluginHost};
use crate::backend::{AudioSetup, PluginInfo, StatePoll};
use crate::inst_unit::HostedInstrument;
use crate::unit::HostedUnit;
use cantus_core::{EffectId, PluginIsolation, PluginState, Project, TrackId};
use cantus_engine::{hosted_key, EffectUnit, NativeUnit, MAX_BLOCK_FRAMES};
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A preparation nobody asked about for this long is dropped (a render that was not
/// started after all): its instances end.
const FORGOTTEN_AFTER: Duration = Duration::from_secs(10);

/// A preparation its owner has not asked about for this long is no longer in use: another
/// owner may take its place (its owner asks every frame while it waits).
const ABANDONED_AFTER: Duration = Duration::from_secs(1);

/// Who a render's preparation is for (review isolation-6 #4, #13, #26).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderFor {
    Export,
    Bake,
    /// A freeze, a refresh or a bounce (one queue).
    Freeze,
    /// A render made at once, without asking first (tests, an in-process render).
    Now,
}

/// What a render's plug-in is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Target {
    Effect(EffectId),
    /// A track's instrument, with its [`hosted_key`].
    Instrument(TrackId, u64),
}

/// One plug-in of the render: what it is made from, and its instance once made.
struct Part {
    target: Target,
    info: PluginInfo,
    isolation: PluginIsolation,
    values: BTreeMap<u32, f64>,
    /// Its state: the project's, or the live one's once it is in (`None`: still asked).
    state: Option<PluginState>,
    instance: Option<Instance>,
}

/// A heard plug-in not loaded live yet (leaving, or not synced yet): waited for, then made
/// a part (review final-B #1).
struct Pending {
    target: Target,
    name: String,
    values: BTreeMap<u32, f64>,
    /// The project's state, for a render from the stored states.
    state: Option<PluginState>,
}

/// The render being prepared: for whom, for which project, rate and states, and its
/// plug-ins.
pub(crate) struct Prep {
    owner: RenderFor,
    key: (u64, bool, u64),
    parts: Vec<Part>,
    pending: Vec<Pending>,
    /// The plug-ins it has no part for on purpose: not loaded live and said as lost, a
    /// frozen chain's resting ones, unheard ones, or taken already.
    rests: HashSet<Target>,
    since: Instant,
    /// Said with the render's outcome: states used as they were last said.
    stale: Vec<RenderFailure>,
    /// Heard plug-ins it lacks from the start.
    lost: Vec<RenderFailure>,
    made: bool,
    touched: Instant,
}

impl PluginHost {
    /// Whether `owner`'s render of `project` at `sample_rate` can begin now: its plug-ins'
    /// render instances are loaded, or lost — `stored`: from the states `project` holds (a
    /// freeze's keyed render), else from the live ones as they are now. Asked each frame
    /// before a render begins; never waits. `false` too while another owner's render is
    /// prepared or running: it waits its turn (see the module documentation).
    pub fn render_ready(
        &mut self,
        owner: RenderFor,
        project: &Project,
        sample_rate: f64,
        stored: bool,
    ) -> bool {
        let key = (sample_rate.to_bits(), stored, content_of(project, stored));
        let mut before = None;
        if let Some(p) = &self.render_prep {
            if p.owner != owner {
                if !self.render_mains.is_empty() || p.touched.elapsed() < ABANDONED_AFTER {
                    return false;
                }
                self.drop_render_prep();
            } else if p.key != key {
                // Its owner asks for another render now (a state or a value changed): made
                // anew, keeping each instance whose part is the same (review final-B #10).
                before = self.render_prep.take();
                if self.render_mains.is_empty() {
                    self.render_lost.clear();
                }
            }
        }
        if self.render_prep.is_none() {
            if !self.render_mains.is_empty() {
                return false;
            }
            let mut prep = self.new_prep(owner, project, key, stored);
            if let Some(before) = before {
                prep.keep_from(before);
            }
            self.fresh_render_cancel();
            self.render_prep = Some(prep);
        }
        // A test's hold: prepared, and said not ready.
        self.advance_prep(sample_rate) && !self.test_hooks.hold_renders
    }

    /// For the interface's tests: whose render is being prepared, if any. The application
    /// never calls it.
    #[doc(hidden)]
    pub fn render_prepared_for_tests(&self) -> Option<RenderFor> {
        self.render_prep.as_ref().map(|p| p.owner)
    }

    /// Whether `owner`'s preparation is still loading — looked at, and kept alive, by its
    /// owner only (`false` when it has none).
    pub fn render_waiting(&mut self, owner: RenderFor) -> bool {
        let Some(rate) = self
            .render_prep
            .as_ref()
            .filter(|p| p.owner == owner)
            .map(|p| f64::from_bits(p.key.0))
        else {
            return false;
        };
        !self.advance_prep(rate) || self.test_hooks.hold_renders
    }

    /// `owner` gave up its render before it began (a freeze cancelled, an export stopped,
    /// a bake dropped): its preparation goes, its instances with it. A running render's
    /// are never touched.
    pub fn drop_render(&mut self, owner: RenderFor) {
        if self.render_prep.as_ref().is_some_and(|p| p.owner == owner)
            && self.render_mains.is_empty()
        {
            self.drop_render_prep();
        }
    }

    pub(crate) fn drop_render_prep(&mut self) {
        if self.render_prep.take().is_some() && self.render_mains.is_empty() {
            self.render_lost.clear();
        }
    }

    /// A preparation nobody asked about for a while is let go (`idle`).
    pub(crate) fn forget_render_prep(&mut self) {
        if self
            .render_prep
            .as_ref()
            .is_some_and(|p| p.touched.elapsed() > FORGOTTEN_AFTER)
        {
            self.drop_render_prep();
        }
    }

    fn new_prep(
        &mut self,
        owner: RenderFor,
        project: &Project,
        key: (u64, bool, u64),
        stored: bool,
    ) -> Prep {
        let mut parts = Vec::new();
        let mut lost = Vec::new();
        let mut pending = Vec::new();
        let mut rests = HashSet::new();
        for (strip, slot) in project.all_effects() {
            let Some(plugin) = slot.spec.external.as_deref() else {
                continue;
            };
            let heard = !slot.bypass && project.strip(strip).is_none_or(|s| s.fx_enabled);
            match self.slots.get_mut(&slot.id) {
                Some(h) => {
                    if !stored {
                        h.main.ask_state_now();
                    }
                    parts.push(Part {
                        target: Target::Effect(slot.id),
                        info: h.info.clone(),
                        isolation: h.isolation,
                        values: plugin.params.clone(),
                        state: stored.then(|| plugin.state.clone()),
                        instance: None,
                    });
                }
                // Not loaded live: lost when it should be heard (a frozen chain's plug-ins
                // rest — the render plays what they rendered); one let go a moment ago, or
                // not synced yet, is waited for (review final-B #1).
                None => {
                    let target = Target::Effect(slot.id);
                    if let Some(m) = self.missing.get(&slot.id) {
                        if heard {
                            lost.push(never(&plugin.name, &m.error));
                        }
                        rests.insert(target);
                    } else if heard && !self.resting.contains(&slot.id) {
                        pending.push(Pending {
                            target,
                            name: plugin.name.clone(),
                            values: plugin.params.clone(),
                            state: stored.then(|| plugin.state.clone()),
                        });
                    } else {
                        rests.insert(target);
                    }
                }
            }
        }
        for t in &project.tracks {
            let Some(plugin) = t.instrument.external() else {
                continue;
            };
            let key = hosted_key(plugin);
            match self.instruments.get_mut(&t.id).filter(|h| h.key == key) {
                Some(h) => {
                    if !stored {
                        h.main.ask_state_now();
                    }
                    parts.push(Part {
                        target: Target::Instrument(t.id, key),
                        info: h.info.clone(),
                        isolation: h.isolation,
                        values: plugin.params.clone(),
                        state: stored.then(|| plugin.state.clone()),
                        instance: None,
                    });
                }
                None => {
                    let target = Target::Instrument(t.id, key);
                    if let Some(m) = self.inst_missing.get(&t.id) {
                        lost.push(never(&plugin.name, &m.error));
                        rests.insert(target);
                    } else if !self.inst_resting.contains(&t.id) {
                        pending.push(Pending {
                            target,
                            name: plugin.name.clone(),
                            values: plugin.params.clone(),
                            state: stored.then(|| plugin.state.clone()),
                        });
                    } else {
                        rests.insert(target);
                    }
                }
            }
        }
        Prep {
            owner,
            key,
            parts,
            pending,
            rests,
            since: Instant::now(),
            stale: Vec::new(),
            lost,
            made: false,
            touched: Instant::now(),
        }
    }

    /// One look: the live states that came in, the instances made once they all did, and
    /// whether every instance is loaded or lost.
    fn advance_prep(&mut self, sample_rate: f64) -> bool {
        let Some(mut prep) = self.render_prep.take() else {
            return true;
        };
        prep.touched = Instant::now();
        self.resolve_pending(&mut prep);
        let mut waiting = !prep.pending.is_empty();
        for part in prep.parts.iter_mut().filter(|p| p.state.is_none()) {
            let live = match part.target {
                Target::Effect(e) => self.slots.get_mut(&e).map(|h| &mut h.main),
                Target::Instrument(t, _) => self.instruments.get_mut(&t).map(|h| &mut h.main),
            };
            let Some(main) = live else {
                // Gone meanwhile: what it said last was parked with it.
                part.state = Some(PluginState::default());
                continue;
            };
            match main.poll_state() {
                StatePoll::Waiting => waiting = true,
                StatePoll::Fresh(s) => part.state = Some(PluginState::new(s)),
                StatePoll::Stale(s, why) => {
                    prep.stale.push(RenderFailure {
                        name: part.info.name.clone(),
                        reason: why,
                        kind: RenderLoss::Stale,
                    });
                    part.state = Some(PluginState::new(s));
                }
                StatePoll::Failed(e) => {
                    prep.stale.push(RenderFailure {
                        name: part.info.name.clone(),
                        reason: e.to_string(),
                        kind: RenderLoss::Stale,
                    });
                    part.state = Some(PluginState::default());
                }
            }
        }
        if !waiting && !prep.made {
            prep.made = true;
            let setup = AudioSetup {
                sample_rate,
                max_block: MAX_BLOCK_FRAMES,
            };
            // A part kept from the preparation before has its instance already.
            for part in prep.parts.iter_mut().filter(|p| p.instance.is_none()) {
                let state = part.state.clone().unwrap_or_default();
                // One that cannot be made is in `render_lost` (review isolation-4 #24).
                part.instance = self
                    .instantiate(
                        &part.info,
                        Some(part.isolation),
                        &state,
                        &part.values,
                        setup,
                        true,
                    )
                    .ok();
            }
        }
        let mut ready = prep.made;
        for i in prep.parts.iter_mut().filter_map(|p| p.instance.as_mut()) {
            // Its answers taken (its load, its parameters), its load's limit judged.
            let _ = i.main.idle();
            if !i.main.loaded() && i.main.stopped().is_none() {
                ready = false;
            }
        }
        self.render_prep = Some(prep);
        ready
    }

    /// The prepared instance of `target` (`name`), loaded — or why the render lacks it.
    fn take_prepared(&mut self, target: Target, name: &str) -> Option<Instance> {
        let prep = self.render_prep.as_mut()?;
        self.render_lost.append(&mut prep.lost);
        self.render_lost.append(&mut prep.stale);
        let Some(at) = prep.parts.iter().position(|p| p.target == target) else {
            // No part for it: one it has none for on purpose is none of its; any other —
            // still on its way when the render began — is its loss, said (review final-B
            // #1).
            if prep.rests.insert(target) {
                self.render_lost
                    .push(never(name, "it was still loading when the render began"));
            }
            return None;
        };
        prep.rests.insert(target);
        let part = prep.parts.swap_remove(at);
        let Some(mut inst) = part.instance else {
            // Its instance was never made (its state still asked): a loss, said — never a
            // render silently without it (review isolation-6 #2, #11, #22).
            self.render_lost.push(never(
                &part.info.name,
                "it was still loading when the render began",
            ));
            return None;
        };
        if inst.main.loaded() {
            if let Some(audio) = inst.main.take_audio() {
                inst.audio = audio;
            }
            return Some(inst);
        }
        // Never loaded: why, said with the render (its instance ends here).
        let why = inst
            .main
            .stopped()
            .map(|(why, _)| why)
            .unwrap_or_else(|| "it was still loading when the render began".into());
        self.render_lost.push(never(&part.info.name, &why));
        None
    }

    /// The render's units for `project`'s inserts (see the module documentation).
    pub fn render_units(&mut self, project: &Project, sample_rate: f64) -> Vec<EffectUnit> {
        self.render_units_as(project, sample_rate, false)
    }

    /// [`Self::render_units`] with the states `project` holds instead of the live ones: a
    /// render kept under a key of what the project holds (a freeze's — review 2).
    pub fn render_units_stored(&mut self, project: &Project, sample_rate: f64) -> Vec<EffectUnit> {
        self.render_units_as(project, sample_rate, true)
    }

    /// Whether the preparation at hand is this render's — made now when there is none
    /// (in process it is ready at once; an isolated plug-in not asked for first, frame by
    /// frame, is then said as never loaded).
    fn prepared_for(&mut self, project: &Project, sample_rate: f64, stored: bool) -> bool {
        let key = (sample_rate.to_bits(), stored, content_of(project, stored));
        if !self.render_prep.as_ref().is_some_and(|p| p.key == key) {
            self.render_ready(RenderFor::Now, project, sample_rate, stored);
        }
        self.render_prep.as_ref().is_some_and(|p| p.key == key)
    }

    /// A plug-in the render cannot have: another render's preparation held its place.
    fn not_prepared(&mut self, name: &str) {
        self.render_lost
            .push(never(name, "another render was being prepared"));
    }

    fn render_units_as(
        &mut self,
        project: &Project,
        sample_rate: f64,
        stored: bool,
    ) -> Vec<EffectUnit> {
        let prepared = self.prepared_for(project, sample_rate, stored);
        let mut units = Vec::new();
        for (_, slot) in project.all_effects() {
            let Some(plugin) = slot.spec.external.as_deref() else {
                continue;
            };
            if !prepared {
                if self.slots.contains_key(&slot.id) {
                    self.not_prepared(&plugin.name);
                }
                continue;
            }
            let Some(inst) = self.take_prepared(Target::Effect(slot.id), &plugin.name) else {
                continue;
            };
            let main = inst.main;
            let unit = HostedUnit::new(
                inst.audio,
                &main.params(),
                |id| main.param_value(id),
                main.latency(),
                sample_rate,
                Arc::new(AtomicU64::new(0)),
            );
            units.push(EffectUnit::from_fx(slot, Box::new(unit)));
            self.render_mains.push(main);
        }
        units
    }

    /// The render's units for `project`'s hosted instruments.
    pub fn render_instruments(&mut self, project: &Project, sample_rate: f64) -> Vec<NativeUnit> {
        self.render_instruments_as(project, sample_rate, false)
    }

    /// [`Self::render_instruments`] with the states `project` holds (a freeze's render).
    pub fn render_instruments_stored(
        &mut self,
        project: &Project,
        sample_rate: f64,
    ) -> Vec<NativeUnit> {
        self.render_instruments_as(project, sample_rate, true)
    }

    fn render_instruments_as(
        &mut self,
        project: &Project,
        sample_rate: f64,
        stored: bool,
    ) -> Vec<NativeUnit> {
        let prepared = self.prepared_for(project, sample_rate, stored);
        let mut units = Vec::new();
        for t in &project.tracks {
            let Some(plugin) = t.instrument.external() else {
                continue;
            };
            let key = hosted_key(plugin);
            if !prepared {
                if self.instruments.contains_key(&t.id) {
                    self.not_prepared(&plugin.name);
                }
                continue;
            }
            let Some(inst) = self.take_prepared(Target::Instrument(t.id, key), &plugin.name) else {
                continue;
            };
            let main = inst.main;
            let unit = HostedInstrument::new(
                inst.audio,
                &main.params(),
                |id| main.param_value(id),
                main.latency(),
                sample_rate,
                Arc::new(AtomicU64::new(0)),
            )
            .supports(main.expression_support());
            units.push(NativeUnit::hosted(t.id, key, Box::new(unit)));
            self.render_mains.push(main);
        }
        units
    }
}

impl PluginHost {
    /// The pending plug-ins that are loaded live now made parts; those found missing, or
    /// resting, or waited for too long, said or let be (review final-B #1).
    fn resolve_pending(&mut self, prep: &mut Prep) {
        let stored = prep.key.1;
        let limit = self.leaving_limit() + PENDING_AFTER_LEAVING;
        for p in std::mem::take(&mut prep.pending) {
            let live = match p.target {
                Target::Effect(e) => self
                    .slots
                    .get_mut(&e)
                    .map(|h| (&mut h.main, h.info.clone(), h.isolation)),
                Target::Instrument(t, key) => self
                    .instruments
                    .get_mut(&t)
                    .filter(|h| h.key == key)
                    .map(|h| (&mut h.main, h.info.clone(), h.isolation)),
            };
            if let Some((main, info, isolation)) = live {
                if !stored {
                    main.ask_state_now();
                }
                prep.parts.push(Part {
                    target: p.target,
                    info,
                    isolation,
                    values: p.values,
                    state: p.state,
                    instance: None,
                });
                continue;
            }
            let (missing, of, resting) = match p.target {
                Target::Effect(e) => (
                    self.missing.get(&e).map(|m| m.error.clone()),
                    Parked::Effect(e),
                    self.resting.contains(&e),
                ),
                Target::Instrument(t, key) => (
                    self.inst_missing.get(&t).map(|m| m.error.clone()),
                    Parked::Instrument(t, key),
                    self.inst_resting.contains(&t),
                ),
            };
            let leaving = self.is_leaving(of);
            if let Some(why) = missing {
                prep.lost.push(never(&p.name, &why));
                prep.rests.insert(p.target);
            } else if resting && !leaving {
                prep.rests.insert(p.target);
            } else if prep.since.elapsed() > limit {
                prep.lost
                    .push(never(&p.name, "it was still loading when the render began"));
                prep.rests.insert(p.target);
            } else {
                prep.pending.push(p);
            }
        }
    }
}

/// How long past a plug-in's leaving limit a render waits for it to be loaded live again.
const PENDING_AFTER_LEAVING: Duration = Duration::from_secs(10);

impl Prep {
    /// The instances of `before` whose part is the same in this one (the same plug-in, where
    /// it runs, values and state), kept instead of made again (review final-B #10).
    fn keep_from(&mut self, before: Prep) {
        for mut old in before.parts {
            let Some(instance) = old.instance.take() else {
                continue;
            };
            if let Some(part) = self.parts.iter_mut().find(|n| {
                n.instance.is_none()
                    && n.target == old.target
                    && n.info.id == old.info.id
                    && n.info.format == old.info.format
                    && n.isolation == old.isolation
                    && n.values == old.values
                    && n.state.is_some()
                    && n.state == old.state
            }) {
                part.instance = Some(instance);
            }
        }
    }
}

fn never(name: &str, why: &str) -> RenderFailure {
    RenderFailure {
        name: name.to_string(),
        reason: why.to_string(),
        kind: RenderLoss::NeverLoaded,
    }
}

/// What a render of `project` is made of: which hosted plug-ins, where they run, the values
/// it gives them — and, `stored`, the states (a freeze's render, keyed by them): what a
/// preparation is for, so one made for other states or values is never used (review
/// isolation-6 #1, #15).
fn content_of(project: &Project, stored: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut one = |id: u64, p: &cantus_core::ExternalPlugin| {
        (id, &p.id, p.isolation).hash(&mut h);
        for (k, v) in &p.params {
            (k, v.to_bits()).hash(&mut h);
        }
        if stored {
            p.state.bytes().hash(&mut h);
        }
    };
    for (_, slot) in project.all_effects() {
        if let Some(p) = slot.spec.external.as_deref() {
            one(slot.id.0, p);
        }
    }
    for t in &project.tracks {
        if let Some(p) = t.instrument.external() {
            one(t.id.0 ^ hosted_key(p), p);
        }
    }
    h.finish()
}

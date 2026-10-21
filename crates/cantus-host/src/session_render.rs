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
//! - **Losses, said.** A heard plug-in that is not loaded live (it could not be loaded, or
//!   crashed loading), or whose render instance never loaded, is a loss of the render
//!   ([`RenderLoss::NeverLoaded`], review isolation-5 #27); a state that could not be had
//!   as it is now — its helper held by a window, or too slow — is used as it was last said,
//!   and the render says so ([`RenderLoss::Stale`]): never a stale state as a fresh one.

use super::iso::{Instance, RenderFailure, RenderLoss};
use super::PluginHost;
use crate::backend::{AudioSetup, PluginInfo, StatePoll};
use crate::inst_unit::HostedInstrument;
use crate::unit::HostedUnit;
use cantus_core::{EffectId, PluginIsolation, PluginState, Project, TrackId};
use cantus_engine::{hosted_key, EffectUnit, NativeUnit, MAX_BLOCK_FRAMES};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A preparation nobody asked about for this long is dropped (a render that was not
/// started after all): its instances end.
const FORGOTTEN_AFTER: Duration = Duration::from_secs(10);

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

/// The render being prepared: for which project, rate and states, and its plug-ins.
pub(crate) struct Prep {
    key: (u64, bool, u64),
    parts: Vec<Part>,
    /// Said with the render's outcome: states used as they were last said.
    stale: Vec<RenderFailure>,
    /// Heard plug-ins it lacks from the start.
    lost: Vec<RenderFailure>,
    made: bool,
    touched: Instant,
}

impl PluginHost {
    /// Whether the render of `project` at `sample_rate` can begin now: its plug-ins'
    /// render instances are loaded, or lost — `stored`: from the states `project` holds (a
    /// freeze's keyed render), else from the live ones as they are now. Asked each frame
    /// before a render begins; never waits (see the module documentation).
    pub fn render_ready(&mut self, project: &Project, sample_rate: f64, stored: bool) -> bool {
        let key = (sample_rate.to_bits(), stored, plugins_of(project));
        if self.render_prep.as_ref().is_some_and(|p| p.key != key) {
            self.drop_render_prep();
        }
        if self.render_prep.is_none() {
            let prep = self.new_prep(project, key, stored);
            self.fresh_render_cancel();
            self.render_prep = Some(prep);
        }
        self.advance_prep(sample_rate)
    }

    /// Whether a render's plug-ins are being made ready (a render waits for them).
    pub fn render_loading(&mut self) -> bool {
        let Some(rate) = self.render_prep.as_ref().map(|p| f64::from_bits(p.key.0)) else {
            return false;
        };
        !self.advance_prep(rate)
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

    fn new_prep(&mut self, project: &Project, key: (u64, bool, u64), stored: bool) -> Prep {
        let mut parts = Vec::new();
        let mut lost = Vec::new();
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
                // rest — the render plays what they rendered).
                None => {
                    if let Some(m) = self.missing.get(&slot.id).filter(|_| heard) {
                        lost.push(never(&plugin.name, &m.error));
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
                    if let Some(m) = self.inst_missing.get(&t.id) {
                        lost.push(never(&plugin.name, &m.error));
                    }
                }
            }
        }
        Prep {
            key,
            parts,
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
        let mut waiting = false;
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
            for part in &mut prep.parts {
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

    /// The prepared instance of `target`, loaded — or why the render lacks it.
    fn take_prepared(&mut self, target: Target) -> Option<Instance> {
        let prep = self.render_prep.as_mut()?;
        self.render_lost.append(&mut prep.lost);
        self.render_lost.append(&mut prep.stale);
        let at = prep.parts.iter().position(|p| p.target == target)?;
        let part = prep.parts.swap_remove(at);
        let mut inst = part.instance?;
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

    fn render_units_as(
        &mut self,
        project: &Project,
        sample_rate: f64,
        stored: bool,
    ) -> Vec<EffectUnit> {
        self.render_ready(project, sample_rate, stored);
        let mut units = Vec::new();
        for (_, slot) in project.all_effects() {
            let Some(inst) = self.take_prepared(Target::Effect(slot.id)) else {
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
        self.render_ready(project, sample_rate, stored);
        let mut units = Vec::new();
        for t in &project.tracks {
            let Some(plugin) = t.instrument.external() else {
                continue;
            };
            let key = hosted_key(plugin);
            let Some(inst) = self.take_prepared(Target::Instrument(t.id, key)) else {
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
            );
            units.push(NativeUnit::hosted(t.id, key, Box::new(unit)));
            self.render_mains.push(main);
        }
        units
    }
}

fn never(name: &str, why: &str) -> RenderFailure {
    RenderFailure {
        name: name.to_string(),
        reason: why.to_string(),
        kind: RenderLoss::NeverLoaded,
    }
}

/// Which hosted plug-ins `project` has, and where: what a preparation is for.
fn plugins_of(project: &Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (_, slot) in project.all_effects() {
        if let Some(p) = slot.spec.external.as_deref() {
            (slot.id, &p.id, p.isolation).hash(&mut h);
        }
    }
    for t in &project.tracks {
        if let Some(p) = t.instrument.external() {
            (t.id, hosted_key(p), p.isolation).hash(&mut h);
        }
    }
    h.finish()
}

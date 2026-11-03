//! Mixer snapshots (round 36, ADR 0093): the desk's state taken at a moment and recalled
//! — all of it, or some channels — as one undo step, and cued on the timeline.
//!
//! **What a snapshot holds** is *state*, never *structure*: for each channel the fader, pan,
//! width, polarity, mute, solo, swap, the inserts' switch, the EQ, each existing send's
//! level, switch and tap, and each insert's bypass, dry/wet and native parameters (a hosted
//! plug-in's own state is its project's business, not the desk's); the master's. A recall
//! never creates, removes or re-routes anything: an insert or a send made since is left as
//! it is, one that is gone is skipped. A channel marked *safe* is left alone by a recall of
//! everything (a recall that names it still reaches it).

use crate::automation::{AutoLane, AutoSeg, AutoTarget};
use crate::effect::{EffectKind, EffectSlot};
use crate::model::{
    EffectId, Master, MixerStrip, Project, Send, StripEq, StripId, MAX_GAIN_DB, MAX_WIDTH,
    MIN_GAIN_DB,
};
use crate::time::Ticks;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A snapshot's id, stable across saves.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MixSnapId(pub u64);

impl std::fmt::Display for MixSnapId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ms{}", self.0)
    }
}

/// A cue's id.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MixCueId(pub u64);

/// One insert's state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FxState {
    pub id: EffectId,
    pub kind: EffectKind,
    pub bypass: bool,
    pub mix: f32,
    pub params: BTreeMap<String, f32>,
}

impl FxState {
    fn of(e: &EffectSlot) -> Self {
        FxState {
            id: e.id,
            kind: e.spec.kind,
            bypass: e.bypass,
            mix: e.mix,
            params: if e.spec.external.is_some() {
                BTreeMap::new()
            } else {
                e.spec.params.clone()
            },
        }
    }

    /// Put the state on the insert: `exact` (an undo) writes back what was taken, the whole
    /// parameter map included, so no key a recall added survives its undo (review 1, item 9);
    /// a recall goes through each parameter's range.
    fn put(&self, e: &mut EffectSlot, exact: bool) {
        if e.spec.kind != self.kind {
            return;
        }
        e.bypass = self.bypass;
        if exact {
            e.mix = self.mix;
            if e.spec.external.is_none() {
                e.spec.params = self.params.clone();
            }
            return;
        }
        if self.mix.is_finite() {
            e.mix = self.mix.clamp(0.0, 1.0);
        }
        if e.spec.external.is_none() {
            if e.spec.kind.sparse() {
                e.spec.params.clear();
            }
            for (k, v) in &self.params {
                e.spec.set(k, *v);
            }
        }
    }
}

/// One channel's state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StripState {
    pub id: StripId,
    pub gain_db: f32,
    pub pan: f32,
    pub width: f32,
    pub phase_invert: bool,
    pub muted: bool,
    pub soloed: bool,
    pub swap_channels: bool,
    pub fx_enabled: bool,
    pub eq: StripEq,
    pub sends: Vec<Send>,
    pub effects: Vec<FxState>,
}

/// The master's state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MasterState {
    pub gain_db: f32,
    pub pan: f32,
    pub width: f32,
    pub phase_invert: bool,
    pub fx_enabled: bool,
    pub effects: Vec<FxState>,
}

/// The desk's state: some channels and maybe the master.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MixState {
    pub strips: Vec<StripState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master: Option<MasterState>,
}

/// A named snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixSnapshot {
    pub id: MixSnapId,
    pub name: String,
    pub state: MixState,
}

/// A recall at a place on the timeline (a scene of the mix): from `at` on, the snapshot's
/// continuous values play as automation holds, until another cue or a clip's own curve
/// takes over. `scope` empty means every channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixCue {
    pub id: MixCueId,
    pub at: Ticks,
    pub snapshot: MixSnapId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<StripId>,
}

/// The project's snapshots, cues, and the VCA given to each group.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MixBook {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snapshots: Vec<MixSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cues: Vec<MixCue>,
    /// `(group, VCA)`: the strips of the group follow the VCA.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_vcas: Vec<(crate::model::GroupId, StripId)>,
}

impl MixBook {
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty() && self.cues.is_empty() && self.group_vcas.is_empty()
    }

    pub fn snapshot(&self, id: MixSnapId) -> Option<&MixSnapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    pub fn group_vca(&self, g: crate::model::GroupId) -> Option<StripId> {
        self.group_vcas
            .iter()
            .find(|(x, _)| *x == g)
            .map(|(_, v)| *v)
    }
}

fn state_of(s: &MixerStrip) -> StripState {
    StripState {
        id: s.id,
        gain_db: s.gain_db,
        pan: s.pan,
        width: s.width,
        phase_invert: s.phase_invert,
        muted: s.muted,
        soloed: s.soloed,
        swap_channels: s.swap_channels,
        fx_enabled: s.fx_enabled,
        eq: s.eq,
        sends: s.sends.clone(),
        effects: s.effects.iter().map(FxState::of).collect(),
    }
}

fn master_of(m: &Master) -> MasterState {
    MasterState {
        gain_db: m.gain_db,
        pan: m.pan,
        width: m.width,
        phase_invert: m.phase_invert,
        fx_enabled: m.fx_enabled,
        effects: m.effects.iter().map(FxState::of).collect(),
    }
}

/// The desk's state now: `scope` empty is every channel; the master when asked.
pub fn capture(project: &Project, scope: &[StripId], master: bool) -> MixState {
    MixState {
        strips: project
            .strips
            .iter()
            .filter(|s| scope.is_empty() || scope.contains(&s.id))
            .map(state_of)
            .collect(),
        master: master.then(|| master_of(&project.master)),
    }
}

/// The desk's state taken again for a snapshot that holds `old`: the same channels (those
/// still on the desk), and the master only if it had it — an update never widens a partial
/// snapshot (review 1, item 8).
pub fn recapture(project: &Project, old: &MixState) -> MixState {
    let ids: Vec<StripId> = old.strips.iter().map(|s| s.id).collect();
    MixState {
        strips: project
            .strips
            .iter()
            .filter(|s| ids.contains(&s.id))
            .map(state_of)
            .collect(),
        master: old.master.as_ref().map(|_| master_of(&project.master)),
    }
}

fn fin(v: f32, old: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        old
    }
}

fn put_strip(s: &mut MixerStrip, st: &StripState, exact: bool) {
    if exact {
        // An undo: what was taken, as it was.
        s.gain_db = st.gain_db;
        s.pan = st.pan;
        s.width = st.width;
        s.phase_invert = st.phase_invert;
        s.muted = st.muted;
        s.soloed = st.soloed;
        s.swap_channels = st.swap_channels;
        s.fx_enabled = st.fx_enabled;
        s.eq = st.eq;
        for x in &st.sends {
            if let Some(d) = s.send_to_mut(x.to) {
                *d = *x;
            }
        }
        for fx in &st.effects {
            if let Some(e) = s.effect_mut(fx.id) {
                fx.put(e, true);
            }
        }
        return;
    }
    s.gain_db = fin(st.gain_db, s.gain_db).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    s.pan = fin(st.pan, s.pan).clamp(-1.0, 1.0);
    s.width = fin(st.width, s.width).clamp(0.0, MAX_WIDTH);
    s.phase_invert = st.phase_invert;
    s.muted = st.muted;
    s.soloed = st.soloed;
    s.swap_channels = st.swap_channels;
    s.fx_enabled = st.fx_enabled;
    if st
        .eq
        .bands
        .iter()
        .all(|b| b.freq_hz.is_finite() && b.gain_db.is_finite() && b.q.is_finite())
    {
        s.eq = st.eq;
    }
    for x in &st.sends {
        if let Some(d) = s.send_to_mut(x.to) {
            if x.level.is_finite() {
                d.level = x.level.clamp(0.0, 1.0);
            }
            d.enabled = x.enabled;
            d.tap = x.tap;
        }
    }
    for fx in &st.effects {
        if let Some(e) = s.effect_mut(fx.id) {
            fx.put(e, false);
        }
    }
}

/// Recall `state` onto the project: the channels of `scope` (empty: all), the master when
/// `master`, a *safe* channel only when `include_safe` or named by `scope`. Returns the
/// state it replaced — exactly what was changed, so applying it back (`exact`) undoes the
/// recall to the bit.
pub fn apply_state(
    project: &mut Project,
    state: &MixState,
    scope: &[StripId],
    master: bool,
    include_safe: bool,
    exact: bool,
) -> MixState {
    let mut old = MixState::default();
    for st in &state.strips {
        if !scope.is_empty() && !scope.contains(&st.id) {
            continue;
        }
        let Some(s) = project.strip_mut(st.id) else {
            continue;
        };
        if s.recall_safe && !include_safe && !scope.contains(&st.id) {
            continue;
        }
        old.strips.push(state_of(s));
        put_strip(s, st, exact);
    }
    if master {
        if let Some(m) = &state.master {
            old.master = Some(master_of(&project.master));
            let p = &mut project.master;
            if exact {
                p.gain_db = m.gain_db;
                p.pan = m.pan;
                p.width = m.width;
                p.phase_invert = m.phase_invert;
                p.fx_enabled = m.fx_enabled;
                for fx in &m.effects {
                    if let Some(e) = p.effects.iter_mut().find(|e| e.id == fx.id) {
                        fx.put(e, true);
                    }
                }
                return old;
            }
            p.gain_db = fin(m.gain_db, p.gain_db).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
            p.pan = fin(m.pan, p.pan).clamp(-1.0, 1.0);
            p.width = fin(m.width, p.width).clamp(0.0, MAX_WIDTH);
            p.phase_invert = m.phase_invert;
            p.fx_enabled = m.fx_enabled;
            for fx in &m.effects {
                if let Some(e) = p.effects.iter_mut().find(|e| e.id == fx.id) {
                    fx.put(e, false);
                }
            }
        }
    }
    old
}

/// What a recall would change, for the freeze guard: the strips whose chain changes, the
/// strips whose mix changes, and whether the master does.
pub fn changes(
    project: &Project,
    state: &MixState,
    scope: &[StripId],
    master: bool,
    include_safe: bool,
) -> (Vec<StripId>, Vec<StripId>, bool) {
    let mut after = project.clone();
    apply_state(&mut after, state, scope, master, include_safe, false);
    let (mut chain, mut mix) = (Vec::new(), Vec::new());
    for (a, b) in project.strips.iter().zip(&after.strips) {
        if a.phase_invert != b.phase_invert
            || a.swap_channels != b.swap_channels
            || a.fx_enabled != b.fx_enabled
            || a.effects != b.effects
            || a.eq != b.eq
        {
            chain.push(a.id);
        }
        if a.gain_db != b.gain_db
            || a.pan != b.pan
            || a.width != b.width
            || a.muted != b.muted
            || a.soloed != b.soloed
            || a.sends != b.sends
        {
            mix.push(a.id);
        }
    }
    let m = project.master != after.master;
    (chain, mix, m)
}

// ---- cues → automation ----------------------------------------------------------------------

/// The automatable values of an insert (a sparse kind holds only what differs from its
/// defaults: the rest read as the defaults).
///
/// `now` is the insert as the project holds it, when no lane drives any of its parameters: a
/// value equal to it is left out before any target is built (a whole desk's EQs are hundreds
/// of keys each).
fn fx_lanes(fx: &FxState, now: Option<&EffectSlot>, out: &mut Vec<(AutoTarget, f32)>) {
    let now = now.filter(|e| e.spec.kind == fx.kind && e.spec.external.is_none());
    // The whole insert unchanged: nothing to look at key by key.
    if now.is_some_and(|e| e.spec.params == fx.params) {
        return;
    }
    let same = |k: &str, v: f32| now.is_some_and(|e| e.spec.get(k) == v);
    if fx.kind.sparse() {
        for i in fx.kind.params() {
            let v = fx.params.get(i.key).copied().unwrap_or(i.default);
            if now.is_some_and(|e| e.spec.params.get(i.key).copied().unwrap_or(i.default) == v) {
                continue;
            }
            out.push((
                AutoTarget::Effect {
                    effect: fx.id,
                    key: i.key.to_string(),
                },
                v,
            ));
        }
    } else {
        for (k, x) in &fx.params {
            if same(k, *x) {
                continue;
            }
            out.push((
                AutoTarget::Effect {
                    effect: fx.id,
                    key: k.clone(),
                },
                *x,
            ));
        }
    }
}

/// Every continuous value a strip's state holds that automation can drive (`now`: an
/// insert's project state when no lane drives it, by id).
fn lane_values<'a>(
    st: &StripState,
    now: &dyn Fn(EffectId) -> Option<&'a EffectSlot>,
) -> Vec<(AutoTarget, f32)> {
    let mut v = vec![
        (AutoTarget::StripGain(st.id), st.gain_db),
        (AutoTarget::StripPan(st.id), st.pan),
        (AutoTarget::StripWidth(st.id), st.width),
    ];
    for s in &st.sends {
        v.push((
            AutoTarget::Send {
                strip: st.id,
                to: s.to,
            },
            s.level,
        ));
    }
    for fx in &st.effects {
        fx_lanes(fx, now(fx.id), &mut v);
    }
    v
}

/// Lay the cues over the lanes: at each cue's tick every value of its snapshot (for the
/// channels it names, safe channels left out unless named) holds, in the target's lane —
/// made if the target has none, holding the project's value before the first cue — **until
/// the next automation clip of that target starts** (or a later cue): a clip that was playing
/// across the cue's bar is silenced from the cue on, as a console's scene recall overrides a
/// ride until the next one is written (review 1, item 10). A value the lane already holds,
/// unmoving, over that span is left out: a cue of a whole desk costs only what it changes
/// (review 1, item 4).
pub fn cue_lanes(project: &Project, lanes: &mut Vec<AutoLane>) {
    if project.mix.cues.is_empty() {
        return;
    }
    use std::collections::HashMap;
    let mut index: HashMap<AutoTarget, usize> = lanes
        .iter()
        .enumerate()
        .map(|(i, l)| (l.target.clone(), i))
        .collect();
    // Where each target's automation clips start (what ends a cue's hold).
    let mut starts: HashMap<&AutoTarget, Vec<Ticks>> = HashMap::new();
    for t in &project.tracks {
        for c in &t.clips {
            if let Some(a) = c.automation.as_ref() {
                if !c.muted && !a.points.is_empty() && c.length > Ticks::ZERO {
                    starts.entry(&a.target).or_default().push(c.start);
                }
            }
        }
    }
    for v in starts.values_mut() {
        v.sort();
    }
    // Inserts some lane drives: their values are compared with the lane, not the project.
    let mut laned_fx: std::collections::HashSet<EffectId> = lanes
        .iter()
        .filter_map(|l| match &l.target {
            AutoTarget::Effect { effect, .. } => Some(*effect),
            _ => None,
        })
        .collect();
    let mut cues: Vec<&MixCue> = project.mix.cues.iter().collect();
    cues.sort_by_key(|c| (c.at, c.id));
    for cue in cues {
        let Some(snap) = project.mix.snapshot(cue.snapshot) else {
            continue;
        };
        let mut values: Vec<(AutoTarget, f32)> = Vec::new();
        for st in &snap.state.strips {
            if !cue.scope.is_empty() && !cue.scope.contains(&st.id) {
                continue;
            }
            let Some(s) = project.strip(st.id) else {
                continue;
            };
            if s.recall_safe && !cue.scope.contains(&st.id) {
                continue;
            }
            let now = |id: EffectId| (!laned_fx.contains(&id)).then(|| s.effect(id)).flatten();
            values.extend(lane_values(st, &now));
        }
        if cue.scope.is_empty() {
            if let Some(m) = &snap.state.master {
                values.push((AutoTarget::MasterGain, m.gain_db));
                values.push((AutoTarget::MasterPan, m.pan));
                values.push((AutoTarget::MasterWidth, m.width));
                for fx in &m.effects {
                    let now = (!laned_fx.contains(&fx.id))
                        .then(|| project.master.effects.iter().find(|e| e.id == fx.id))
                        .flatten();
                    fx_lanes(fx, now, &mut values);
                }
            }
        }
        let t = cue.at.0 as f64;
        for (target, value) in values {
            if !value.is_finite() {
                continue;
            }
            let end = starts
                .get(&target)
                .and_then(|v| v.iter().find(|s| **s > cue.at).copied());
            let inside = |s: &AutoSeg| s.at >= cue.at && end.is_none_or(|e| s.at < e);
            let li = match index.get(&target) {
                Some(&i) => {
                    // Unchanged and unmoving over the cue's span: nothing to hold.
                    let l = &lanes[i];
                    let p = l.range.pos(value);
                    let now = l.pos_at(t).unwrap_or_else(|| l.range.pos(l.base));
                    let still = l.seg_at(t).is_none_or(|k| {
                        let s = &l.segs[k];
                        s.v0 == s.v1 || s.shape == crate::automation::Shape::Hold
                    });
                    if now == p && still && !l.segs.iter().any(|s| s.at > cue.at && inside(s)) {
                        continue;
                    }
                    i
                }
                None => {
                    let (Some(range), Some(base)) = (
                        crate::automation::range::range_of(project, &target),
                        crate::automation::range::base_value(project, &target),
                    ) else {
                        continue;
                    };
                    if range.pos(value) == range.pos(base) {
                        continue;
                    }
                    if let AutoTarget::Effect { effect, .. } = &target {
                        laned_fx.insert(*effect);
                    }
                    lanes.push(AutoLane {
                        target: target.clone(),
                        range,
                        base,
                        segs: Vec::new(),
                    });
                    index.insert(target, lanes.len() - 1);
                    lanes.len() - 1
                }
            };
            let lane = &mut lanes[li];
            let p = lane.range.pos(value);
            let seg = AutoSeg {
                at: cue.at,
                t0: cue.at,
                t1: cue.at,
                v0: p,
                v1: p,
                shape: crate::automation::Shape::Hold,
                tension: 0.0,
            };
            // The pieces inside the cue's span go; the hold takes their place.
            lane.segs.retain(|s| !inside(s));
            let at = lane.segs.partition_point(|s| s.at < cue.at);
            lane.segs.insert(at, seg);
        }
    }
}

#[cfg(test)]
#[path = "mix_snap_tests.rs"]
mod tests;

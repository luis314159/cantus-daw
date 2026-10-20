//! Freezing (ADR 0064): a mixer strip — everything it gathers and its chain up to the width
//! — or a track's own source — its instrument and clips — played from a render instead of
//! computed, the CPU freed, and given back live, bit for bit, by unfreezing.
//!
//! **The boundary.** A frozen strip's render is taken **before width, pan and fader**: those,
//! mute, solo, the sends' levels and every automation of them stay live, so a frozen strip
//! is still mixed. A frozen track's render is taken before the track's own level and
//! balance, which stay live with its mute and solo.
//!
//! **What makes the sound** of a frozen target is its *subject* ([`subject`]): the frozen
//! strip's chain, every strip and track it gathers (by output or send, recursively), every
//! strip a key comes from and every track whose notes an insert hears (with what they
//! gather), the automation and the modulators that reach any of those, the audio sources
//! their clips play, the tempo map and the pan law. Its canonical form (compact JSON, the
//! fields that cannot change the sound left out) hashed with SHA-256 is the target's
//! **content key** ([`content_key`]); the render on disk is found by it.
//!
//! **An edit that would change a frozen sound is refused** ([`guard`], in `freeze_guard.rs`):
//! every [`Command`] is classified by what it touches — a `match` with no wildcard, so a new
//! command does not build until it is classified (the edit lock cannot drift out of date,
//! BACKLOG §4c) — and a touch inside a frozen region is refused with
//! [`CommandError::Frozen`], naming the target to unfreeze. Commands whose reach is
//! project-wide (the tempo, the pan law, a whole project replaced) are applied and checked:
//! when a frozen target's content key moved, they are undone and refused.
//!
//! Which nodes then rest is [`plays`]' business: given which frozen targets have their
//! render at hand, the snapshot marks every node whose sound only reaches a playing frozen
//! target as resting (its CPU freed, its hosted plug-ins unloaded), except what something
//! live still needs — a key, the notes an insert hears, the audio a modulator follows.

use crate::automation::AutoTarget;
use crate::command::{Command, CommandError};
use crate::model::{MixerStrip, ModulatorId, Project, StripId, Track, TrackId};
use crate::modulator::{EnvTrigger, Listen, ModHost, ModSource, Modulator, Retrigger};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[path = "freeze_guard.rs"]
mod guard_impl;
pub use guard_impl::{guard, touches, Touch};

/// Bumped when the subject's canonical form changes (a content key from an older build is
/// then another key: the render is made again — never a wrong one played).
pub const SUBJECT_VERSION: u32 = 2;

/// What is frozen.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FreezeTarget {
    /// A mixer strip: everything it gathers and its chain up to the width.
    Strip(StripId),
    /// A track's own source: its instrument and clips, before its level and balance.
    Track(TrackId),
}

/// A frozen target's state in the model (schema 37): the content key its render was made
/// for. Its render is found on disk by that key (with the rate and the DSP version), so the
/// project needs nothing else; a render that is not there is made again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frozen {
    /// The target's content key ([`content_key`]) when it was frozen: lower-case hex
    /// SHA-256, 64 characters.
    pub content: String,
}

impl Frozen {
    /// Valid: 64 lower-case hex digits.
    pub fn is_valid(&self) -> bool {
        self.content.len() == 64
            && self
                .content
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

impl Project {
    /// A target's frozen state, `None` when it is live (or gone).
    pub fn frozen(&self, target: FreezeTarget) -> Option<&Frozen> {
        match target {
            FreezeTarget::Strip(s) => self.strip(s)?.frozen.as_ref(),
            FreezeTarget::Track(t) => self.track(t)?.frozen.as_ref(),
        }
    }

    /// Every frozen target, strips first (in the desk's order), then tracks.
    pub fn frozen_targets(&self) -> Vec<FreezeTarget> {
        self.strips
            .iter()
            .filter(|s| s.frozen.is_some())
            .map(|s| FreezeTarget::Strip(s.id))
            .chain(
                self.tracks
                    .iter()
                    .filter(|t| t.frozen.is_some())
                    .map(|t| FreezeTarget::Track(t.id)),
            )
            .collect()
    }

    /// Whether anything is frozen.
    pub fn any_frozen(&self) -> bool {
        self.strips.iter().any(|s| s.frozen.is_some())
            || self.tracks.iter().any(|t| t.frozen.is_some())
    }

    /// What *Freeze* on a track freezes: the strip it feeds when that strip is its own (no
    /// other track feeds it) — the track and its inserts in one render — else the track's
    /// own source.
    pub fn freeze_target_of_track(&self, track: TrackId) -> Option<FreezeTarget> {
        let t = self.track(track)?;
        match t.strip {
            Some(s)
                if self.strip(s).is_some() && self.tracks_on_strip(s).all(|o| o.id == track) =>
            {
                Some(FreezeTarget::Strip(s))
            }
            _ => Some(FreezeTarget::Track(track)),
        }
    }
}

/// The nodes whose sound makes a target's: a strip's upstream (what it gathers, its keys,
/// its inserts' note sources, what its modulators listen to — recursively), or a track
/// alone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Region {
    /// The strips, the target's first when it is a strip.
    pub strips: BTreeSet<StripId>,
    pub tracks: BTreeSet<TrackId>,
    /// Some modulator in it listens to the master: the whole mix is in it.
    pub whole: bool,
    /// A modulator driving something in it listens to the target's own output — the
    /// frozen strip after its fader, or the strip a frozen track plays into: the target's
    /// own live controls (fader, pan, width, mute; a track's level and balance) shape what
    /// it hears, so they are part of the sound (locked, in the key).
    pub hears_root: bool,
    /// A frozen strip's only input — one track and no other strip's output or send: that
    /// track's mute and solo stay live (the frozen strip is silenced with it).
    pub own: Option<TrackId>,
    /// The strips the modulators driving something in it listen to.
    listened: BTreeSet<StripId>,
}

/// A target's region. A track's is the track; a strip's grows to a fixed point over the
/// routes into it, its inserts' keys and note sources, and the sources of the modulators
/// that drive anything in it.
pub fn region(project: &Project, target: FreezeTarget) -> Region {
    let mut r = Region::default();
    match target {
        FreezeTarget::Track(t) => {
            if project.track(t).is_none() {
                return r;
            }
            r.tracks.insert(t);
            // A track's source: its instrument's automation and modulators reach it, but
            // nothing else's sound does — except a modulator following some audio, and
            // then everything that audio is made of (review 1: a follower of a bus heard
            // silence in the render, and the bus's tracks were not locked).
            grow_by_modulators(project, &mut r, &|tgt| source_target_of(tgt) == Some(t));
            for s in r.strips.clone() {
                let sr = region(project, FreezeTarget::Strip(s));
                r.hears_root |= sr.tracks.contains(&t);
                r.strips.extend(sr.strips);
                r.tracks.extend(sr.tracks);
                r.whole |= sr.whole;
            }
            if r.whole {
                r.strips.extend(project.strips.iter().map(|s| s.id));
                r.tracks.extend(project.tracks.iter().map(|t| t.id));
                r.hears_root = true;
            }
        }
        FreezeTarget::Strip(s) => {
            if project.strip(s).is_none() {
                return r;
            }
            let mut stack = vec![s];
            loop {
                while let Some(id) = stack.pop() {
                    if !r.strips.insert(id) {
                        continue;
                    }
                    let Some(st) = project.strip(id) else {
                        continue;
                    };
                    for t in project.tracks_on_strip(id) {
                        r.tracks.insert(t.id);
                    }
                    for o in &project.strips {
                        if o.id != id && o.feeds().any(|to| to == id) {
                            stack.push(o.id);
                        }
                    }
                    for e in &st.effects {
                        if let Some(k) = e.sidechain {
                            stack.push(k);
                        }
                        if let Some(t) = e.midi_from {
                            if project.track(t).is_some() {
                                r.tracks.insert(t);
                            }
                        }
                    }
                }
                // Modulators driving anything inside listen to (or hear the notes of)
                // something that is then inside too.
                let before = (r.strips.len(), r.tracks.len(), r.whole);
                let inside = r.clone();
                grow_by_modulators(project, &mut r, &|tgt| {
                    target_in_strip_region(project, &inside, Some(s), None, tgt)
                });
                if r.whole {
                    for st in &project.strips {
                        r.strips.insert(st.id);
                    }
                    for t in &project.tracks {
                        r.tracks.insert(t.id);
                    }
                }
                // The strips the modulators added still need their own upstream.
                stack.extend(
                    r.strips
                        .iter()
                        .copied()
                        .filter(|x| !inside.strips.contains(x)),
                );
                for x in &stack {
                    r.strips.remove(x);
                }
                if stack.is_empty() && before == (r.strips.len(), r.tracks.len(), r.whole) {
                    break;
                }
            }
            r.hears_root = r.whole || r.listened.contains(&s);
            let mut on = project.tracks_on_strip(s);
            r.own = match (on.next(), on.next()) {
                (Some(t), None)
                    if !r.whole
                        && !project
                            .strips
                            .iter()
                            .any(|o| o.id != s && o.feeds().any(|to| to == s)) =>
                {
                    Some(t.id)
                }
                _ => None,
            };
        }
    }
    r
}

/// The track an automation target is a source parameter of (its instrument's), if any.
fn source_target_of(target: &AutoTarget) -> Option<TrackId> {
    match target {
        AutoTarget::Instrument { track, .. }
        | AutoTarget::Firmus { track, .. }
        | AutoTarget::PluginInstrument { track, .. } => Some(*track),
        _ => None,
    }
}

/// Whether `target` moves the sound of a region: any parameter of a strip or track inside
/// it, except a frozen strip `root`'s own width, pan, fader and sends and a frozen track
/// `root_track`'s own level and balance (live).
fn target_in_strip_region(
    project: &Project,
    r: &Region,
    root: Option<StripId>,
    root_track: Option<TrackId>,
    target: &AutoTarget,
) -> bool {
    match target {
        AutoTarget::TrackGain(t) | AutoTarget::TrackPan(t) => {
            Some(*t) != root_track && r.tracks.contains(t)
        }
        AutoTarget::Instrument { track, .. }
        | AutoTarget::Firmus { track, .. }
        | AutoTarget::PluginInstrument { track, .. } => r.tracks.contains(track),
        AutoTarget::StripGain(s) | AutoTarget::StripPan(s) | AutoTarget::StripWidth(s) => {
            Some(*s) != root && r.strips.contains(s)
        }
        AutoTarget::Send { strip, .. } => Some(*strip) != root && r.strips.contains(strip),
        AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. } => project
            .effect_host(*effect)
            .is_some_and(|h| r.strips.contains(&h)),
        AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth => false,
        AutoTarget::ModAmount(m) => modulator_reaches(project, *m, &|t| {
            !matches!(t, AutoTarget::ModAmount(_))
                && target_in_strip_region(project, r, root, root_track, t)
        }),
    }
}

/// Whether modulator `m` drives a target `inside` says is inside (one level of nesting).
fn modulator_reaches(
    project: &Project,
    m: ModulatorId,
    inside: &dyn Fn(&AutoTarget) -> bool,
) -> bool {
    project
        .modulator(m)
        .is_some_and(|(_, _, m)| m.routes.iter().any(|r| inside(&r.target)))
}

/// Add to `r` what the modulators driving anything `inside` listen to and hear the notes
/// of.
fn grow_by_modulators(project: &Project, r: &mut Region, inside: &dyn Fn(&AutoTarget) -> bool) {
    let drives = |m: &Modulator| {
        !m.muted
            && m.routes.iter().any(|route| {
                inside(&route.target)
                    || matches!(&route.target, AutoTarget::ModAmount(n)
                        if modulator_reaches(project, *n, inside))
            })
    };
    // A modulator whose amount another drives: the driver matters too (one level).
    let mut reaching: Vec<(ModHost, &Modulator)> = project
        .all_modulators()
        .filter(|(_, m)| drives(m))
        .collect();
    let ids: Vec<ModulatorId> = reaching.iter().map(|(_, m)| m.id).collect();
    reaching.extend(project.all_modulators().filter(|(_, m)| {
        !m.muted
            && !ids.contains(&m.id)
            && m.routes
                .iter()
                .any(|r| matches!(r.target, AutoTarget::ModAmount(n) if ids.contains(&n)))
    }));
    for (host, m) in reaching {
        for (listen, notes) in modulator_sources(project, host, m) {
            match listen {
                Some(Listen::Strip(s)) => {
                    r.strips.insert(s);
                    r.listened.insert(s);
                }
                Some(Listen::Master) => r.whole = true,
                Some(Listen::Host) => match project.host_listen_strip(host) {
                    Some(s) => {
                        r.strips.insert(s);
                        r.listened.insert(s);
                    }
                    None => r.whole = true,
                },
                None => {}
            }
            if let Some(t) = notes {
                r.tracks.insert(t);
            }
        }
    }
}

/// What a modulator listens to and whose notes it hears.
fn modulator_sources(
    project: &Project,
    host: ModHost,
    m: &Modulator,
) -> Vec<(Option<Listen>, Option<TrackId>)> {
    let host_notes = || project.host_notes_track(host);
    match &m.source {
        ModSource::Lfo(l) if l.retrigger == Retrigger::Note => {
            vec![(None, l.notes.or_else(host_notes))]
        }
        ModSource::Envelope(e) => match e.trigger {
            EnvTrigger::Notes { track } => vec![(None, track.or_else(host_notes))],
            EnvTrigger::Audio { listen, .. } => vec![(Some(listen), None)],
        },
        ModSource::Follower(f) => vec![(Some(f.listen), None)],
        _ => Vec::new(),
    }
}

/// Every frozen region, and what is locked by them.
#[derive(Clone, Debug, Default)]
pub struct Locks {
    /// Strips whose chain or what they gather a frozen sound depends on: every strip of a
    /// frozen strip's region, the frozen strip included.
    pub chain: BTreeSet<StripId>,
    /// Strips whose mix (fader, pan, width, mute, sends, where they go) a frozen sound
    /// depends on: the regions' strips but their frozen roots.
    pub mix: BTreeSet<StripId>,
    /// Tracks whose source (instrument, clips, notes, input) a frozen sound depends on.
    pub source: BTreeSet<TrackId>,
    /// Tracks whose level, balance, mute, solo and routing a frozen sound depends on.
    pub track_mix: BTreeSet<TrackId>,
    /// Of those, the tracks whose mute and solo stay live: a frozen strip's only input
    /// ([`Region::own`]) — the strip is silenced with them.
    pub own: BTreeSet<TrackId>,
    /// The frozen target each locked node belongs to (the first that holds it), for the
    /// refusal's notice.
    owner_strip: Vec<(StripId, FreezeTarget)>,
    owner_track: Vec<(TrackId, FreezeTarget)>,
}

impl Locks {
    pub fn of(project: &Project) -> Locks {
        let mut l = Locks::default();
        for target in project.frozen_targets() {
            let r = region(project, target);
            match target {
                FreezeTarget::Strip(root) => {
                    for s in &r.strips {
                        l.chain.insert(*s);
                        // The root's own mix is live, unless something in its region
                        // hears it.
                        if *s != root || r.hears_root {
                            l.mix.insert(*s);
                        }
                        l.owner_strip.push((*s, target));
                    }
                    for t in &r.tracks {
                        l.source.insert(*t);
                        l.track_mix.insert(*t);
                        l.owner_track.push((*t, target));
                    }
                    l.own.extend(r.own);
                }
                FreezeTarget::Track(root) => {
                    for t in &r.tracks {
                        l.source.insert(*t);
                        if *t != root || r.hears_root {
                            l.track_mix.insert(*t);
                        }
                        l.owner_track.push((*t, target));
                    }
                    for s in &r.strips {
                        l.chain.insert(*s);
                        l.mix.insert(*s);
                        l.owner_strip.push((*s, target));
                    }
                }
            }
        }
        // A track's mute and solo stay live only when no frozen region needs them.
        let needed: Vec<TrackId> = l
            .own
            .iter()
            .copied()
            .filter(|t| {
                l.owner_track
                    .iter()
                    .any(|(x, o)| x == t && !Locks::owns_live(project, *o, *t))
            })
            .collect();
        for t in needed {
            l.own.remove(&t);
        }
        // A root frozen inside another's region: its mix is locked by that one.
        for (s, owner) in &l.owner_strip {
            if *owner != FreezeTarget::Strip(*s) {
                l.mix.insert(*s);
            }
        }
        l
    }

    /// Whether track `t`'s mute and solo are live for frozen target `owner`.
    fn owns_live(project: &Project, owner: FreezeTarget, t: TrackId) -> bool {
        match owner {
            FreezeTarget::Strip(_) => region(project, owner).own == Some(t),
            FreezeTarget::Track(root) => root == t && !region(project, owner).hears_root,
        }
    }

    /// Whether nothing is locked.
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty() && self.source.is_empty()
    }

    /// The frozen target holding strip `s`.
    pub fn owner_of_strip(&self, s: StripId) -> Option<FreezeTarget> {
        self.owner_strip
            .iter()
            .find(|(x, _)| *x == s)
            .map(|(_, o)| *o)
    }

    /// The frozen target holding track `t`.
    pub fn owner_of_track(&self, t: TrackId) -> Option<FreezeTarget> {
        self.owner_track
            .iter()
            .find(|(x, _)| *x == t)
            .map(|(_, o)| *o)
    }
}

// ---- the subject and its key ------------------------------------------------------------

/// A JSON object without the keys in `drop` (top level), and without any `isolation` at
/// any depth (where a hosted plug-in runs never changes its sound, ADR 0063).
fn scrub(mut v: serde_json::Value, drop: &[&str]) -> serde_json::Value {
    if let serde_json::Value::Object(m) = &mut v {
        for k in drop {
            m.remove(*k);
        }
    }
    strip_key(&mut v, "isolation");
    v
}

fn strip_key(v: &mut serde_json::Value, key: &str) {
    match v {
        serde_json::Value::Object(m) => {
            m.remove(key);
            for x in m.values_mut() {
                strip_key(x, key);
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                strip_key(x, key);
            }
        }
        _ => {}
    }
}

fn value<T: Serialize>(x: &T) -> serde_json::Value {
    // Through text, so a number is what the file writes (f32s at their shortest), as the
    // project's own hash does.
    serde_json::to_string(x)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// What never changes how anything sounds: names, colours, where things are drawn.
const STRIP_COSMETIC: &[&str] = &[
    "name",
    "color",
    "dock",
    "group",
    "modulators",
    "frozen",
    "soloed",
];
/// A frozen strip's own live controls (and where it goes).
const STRIP_LIVE: &[&str] = &["gain_db", "pan", "width", "muted", "sends", "output"];
const TRACK_COSMETIC: &[&str] = &[
    "name",
    "color",
    "height",
    "icon",
    "takes_open",
    "automation_open",
    "pitch_open",
    "modulators",
    "frozen",
    "soloed",
];
/// A frozen track's own live controls (and where it goes).
const TRACK_LIVE: &[&str] = &["gain_db", "pan", "muted", "strip"];

/// A frozen strip's part: its own live controls left out — but those something in its
/// region hears (a modulator following it, or the whole mix).
fn strip_part(s: &MixerStrip, r: &Region) -> serde_json::Value {
    let v = scrub(value(s), STRIP_COSMETIC);
    if r.whole {
        v
    } else if r.hears_root {
        scrub(v, &["sends", "output"])
    } else {
        scrub(v, STRIP_LIVE)
    }
}

/// A track's part, without `live` (a frozen track's own controls, or an own track's mute).
fn track_part(t: &Track, live: &[&str]) -> serde_json::Value {
    let mut v = scrub(value(t), TRACK_COSMETIC);
    if !live.is_empty() {
        v = scrub(v, live);
    }
    // Automation clips are the targets' (below), wherever they sit; a note or audio clip's
    // name and colour are nothing to hear.
    if let Some(serde_json::Value::Array(clips)) = v.get_mut("clips") {
        clips.retain(|c| c.get("automation").is_none_or(|a| a.is_null()));
        for c in clips.iter_mut() {
            if let serde_json::Value::Object(m) = c {
                m.remove("name");
                m.remove("color");
            }
        }
    }
    v
}

/// Whether automation (or modulation) of `target` moves the sound of frozen target
/// `frozen`, whose region is `r`.
pub(crate) fn reaches(
    project: &Project,
    frozen: FreezeTarget,
    r: &Region,
    target: &AutoTarget,
) -> bool {
    match frozen {
        FreezeTarget::Strip(root) => {
            if is_root_live(target, root) {
                // The root's own controls: heard only by a modulator that follows it (its
                // sends, only by one that follows the whole mix).
                return r.whole || (r.hears_root && !matches!(target, AutoTarget::Send { .. }));
            }
            r.whole || target_in_strip_region(project, r, Some(root), None, target)
        }
        FreezeTarget::Track(t) => {
            let own = (!r.hears_root).then_some(t);
            r.whole
                || source_target_of(target) == Some(t)
                || matches!(target, AutoTarget::ModAmount(m)
                    if modulator_reaches(project, *m, &|x| source_target_of(x) == Some(t)))
                || (r.hears_root
                    && matches!(target, AutoTarget::TrackGain(x) | AutoTarget::TrackPan(x)
                        if *x == t))
                || (r.tracks.len() + r.strips.len() > 1
                    && target_in_strip_region(project, r, None, own, target))
        }
    }
}

/// A frozen strip's own live controls as automation targets.
fn is_root_live(target: &AutoTarget, root: StripId) -> bool {
    match target {
        AutoTarget::StripGain(s) | AutoTarget::StripPan(s) | AutoTarget::StripWidth(s) => {
            *s == root
        }
        AutoTarget::Send { strip, .. } => *strip == root,
        _ => false,
    }
}

/// A target's subject: the canonical form of everything that makes its sound. `None` when
/// the target is gone.
pub fn subject(project: &Project, target: FreezeTarget) -> Option<serde_json::Value> {
    use serde_json::{json, Value};
    let (kind, root_strip, source_track) = match target {
        FreezeTarget::Strip(s) => {
            project.strip(s)?;
            ("strip", Some(s), None)
        }
        FreezeTarget::Track(t) => {
            project.track(t)?;
            ("track", None, Some(t))
        }
    };
    let r = region(project, target);
    // The frozen strip on its own (where it sits on the desk is nothing to hear); the
    // others in the desk's order (their sums are made in it).
    let root: Option<Value> = root_strip
        .and_then(|s| project.strip(s))
        .map(|s| strip_part(s, &r));
    let strips: Vec<Value> = project
        .strips
        .iter()
        .filter(|s| r.strips.contains(&s.id) && Some(s.id) != root_strip)
        .map(|s| scrub(value(s), STRIP_COSMETIC))
        .collect();
    let tracks: Vec<Value> = project
        .tracks
        .iter()
        .filter(|t| r.tracks.contains(&t.id))
        .map(|t| {
            let live: &[&str] = if Some(t.id) == source_track && !r.hears_root {
                TRACK_LIVE
            } else if Some(t.id) == r.own && !r.whole {
                &["muted"]
            } else {
                &[]
            };
            track_part(t, live)
        })
        .collect();
    // Automation clips wherever they sit — in no order of tracks (moving a track is
    // nothing to hear), but which of two starting together wins is kept.
    let mut reaching: Vec<(String, crate::Ticks, Value)> = project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter_map(|c| {
            let a = c.automation.as_ref()?;
            reaches(project, target, &r, &a.target).then(|| {
                (
                    value(&a.target).to_string(),
                    c.start,
                    scrub(value(c), &["id", "name", "color", "link", "comp"]),
                )
            })
        })
        .collect();
    let mut automation: Vec<Value> = Vec::with_capacity(reaching.len());
    for i in 0..reaching.len() {
        let tie = reaching[..i]
            .iter()
            .filter(|(t, at, _)| *t == reaching[i].0 && *at == reaching[i].1)
            .count();
        let mut v = std::mem::take(&mut reaching[i].2);
        if tie > 0 {
            if let Value::Object(m) = &mut v {
                m.insert("tie".into(), json!(tie));
            }
        }
        automation.push(canonical(v));
    }
    automation.sort_by_cached_key(|v| v.to_string());
    let modulators: Vec<Value> = project
        .all_modulators()
        .filter(|(_, m)| {
            m.routes
                .iter()
                .any(|x| reaches(project, target, &r, &x.target))
        })
        .map(|(host, m)| json!({ "host": value(&host), "m": scrub(value(m), &["name"]) }))
        .collect();
    // The sources the region's clips play (and its comps keep), in the pool's order.
    let mut used = BTreeSet::new();
    for t in project.tracks.iter().filter(|t| r.tracks.contains(&t.id)) {
        for c in t.clips.iter() {
            if let Some(a) = &c.audio {
                used.insert(a.source);
            }
        }
        for w in t.comps.iter().flat_map(|c| c.windows.iter()) {
            if let Some(a) = &w.audio {
                used.insert(a.source);
            }
        }
    }
    let mut sources: Vec<&crate::audio::AudioSource> = project
        .audio_pool
        .iter()
        .filter(|s| used.contains(&s.id))
        .collect();
    sources.sort_by_key(|s| s.id);
    let sources: Vec<Value> = sources
        .into_iter()
        .map(|s| scrub(value(s), &["name", "recorded"]))
        .collect();
    let mut o = serde_json::Map::new();
    o.insert("v".into(), json!(SUBJECT_VERSION));
    o.insert("kind".into(), json!(kind));
    if let Some(root) = root {
        o.insert("root".into(), root);
    }
    o.insert("strips".into(), Value::Array(strips));
    o.insert("tracks".into(), Value::Array(tracks));
    o.insert("automation".into(), Value::Array(automation));
    o.insert("modulators".into(), Value::Array(modulators));
    o.insert("sources".into(), Value::Array(sources));
    o.insert(
        "tempo".into(),
        json!({
            "bpm": value(&project.tempo_bpm),
            "metre": value(&project.time_signature),
            "points": value(&project.tempo_points),
            "metres": value(&project.metre_changes),
        }),
    );
    // A modulator there follows the whole mix: the master shapes what it hears, and every
    // strip's place in the desk the order of its sums.
    if r.whole {
        o.insert(
            "master".into(),
            scrub(value(&project.master), &["modulators"]),
        );
        o.insert(
            "desk".into(),
            json!(project.strips.iter().map(|s| s.id.0).collect::<Vec<_>>()),
        );
    }
    // A strip's pan goes through the law: any strip of the region but a frozen root.
    if r.strips.iter().any(|s| Some(*s) != root_strip) {
        o.insert("pan_law".into(), value(&project.pan_law));
    }
    Some(Value::Object(o))
}

/// A target's content key: SHA-256 of its subject's compact JSON, lower-case hex. `None`
/// when the target is gone.
pub fn content_key(project: &Project, target: FreezeTarget) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(&canonical(subject(project, target)?)).ok()?;
    Some(hex(&Sha256::digest(bytes)))
}

/// Every object's keys in sorted order, at every depth: the subject's canonical form,
/// whatever order its fields were written or taken out in (a removed key must not reorder
/// the rest — the fuzz found it).
fn canonical(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let sorted: std::collections::BTreeMap<String, serde_json::Value> =
                m.into_iter().map(|(k, v)| (k, canonical(v))).collect();
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(canonical).collect())
        }
        v => v,
    }
}

/// SHA-256 of `bytes`, lower-case hex (the engine's render keys use it too).
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(bytes))
}

/// Lower-case hex.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Whether every frozen target's stored key is its content key now: what every command
/// keeps (the guard's invariant). The first target that is not, otherwise.
pub fn consistent(project: &Project) -> Result<(), FreezeTarget> {
    for target in project.frozen_targets() {
        let stored = project.frozen(target).map(|f| f.content.as_str());
        if content_key(project, target).as_deref() != stored {
            return Err(target);
        }
    }
    Ok(())
}

/// Re-key every frozen target whose stored key is not its content key — a file edited by
/// hand, or written by a build whose subject was another: the target stays frozen (the
/// user's choice), its render is made again for what it holds now. Returns how many.
pub fn repair(project: &mut Project) -> usize {
    let mut n = 0;
    for target in project.frozen_targets() {
        let Some(content) = content_key(project, target) else {
            continue;
        };
        let slot = match target {
            FreezeTarget::Strip(s) => project.strip_mut(s).and_then(|s| s.frozen.as_mut()),
            FreezeTarget::Track(t) => project.track_mut(t).and_then(|t| t.frozen.as_mut()),
        };
        if let Some(f) = slot {
            if f.content != content {
                f.content = content;
                n += 1;
            }
        }
    }
    n
}

/// The command that freezes `target` as the project is now (its key computed), or `None`
/// when it is gone. [`freeze_commands`] is what the interface runs.
pub fn freeze_command(project: &Project, target: FreezeTarget) -> Option<Command> {
    let content = content_key(project, target)?;
    let frozen = Some(Frozen { content });
    Some(match target {
        FreezeTarget::Strip(strip) => Command::SetStripFrozen { strip, frozen },
        FreezeTarget::Track(track) => Command::SetTrackFrozen { track, frozen },
    })
}

/// The command that unfreezes `target`.
pub fn unfreeze_command(target: FreezeTarget) -> Command {
    match target {
        FreezeTarget::Strip(strip) => Command::SetStripFrozen {
            strip,
            frozen: None,
        },
        FreezeTarget::Track(track) => Command::SetTrackFrozen {
            track,
            frozen: None,
        },
    }
}

/// Why a target cannot be frozen as the project is now.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CannotFreeze {
    Gone,
    /// It is frozen already.
    Frozen,
    /// A track of its region monitors its input always (*On*): a render cannot hold a live
    /// input. The track to switch.
    Monitoring(TrackId),
    /// A track of its region is soloed (its own excepted): a render holds the whole region
    /// (solo is no part of a frozen sound), so the solo would be lost. The track to unsolo.
    SoloedTrack(TrackId),
    /// The same for a strip of its region (the frozen strip excepted).
    SoloedStrip(StripId),
}

/// One undo step that freezes `target`: the armed tracks of its region disarmed first (a
/// take could not land on a frozen track), then the freeze with the key of the project as
/// it will be.
pub fn freeze_commands(project: &Project, target: FreezeTarget) -> Result<Command, CannotFreeze> {
    match target {
        FreezeTarget::Strip(s) if project.strip(s).is_none() => return Err(CannotFreeze::Gone),
        FreezeTarget::Track(t) if project.track(t).is_none() => return Err(CannotFreeze::Gone),
        _ => {}
    }
    if project.frozen(target).is_some() {
        return Err(CannotFreeze::Frozen);
    }
    let r = region(project, target);
    if let Some(s) = project
        .strips
        .iter()
        .find(|s| s.soloed && r.strips.contains(&s.id) && target != FreezeTarget::Strip(s.id))
    {
        return Err(CannotFreeze::SoloedStrip(s.id));
    }
    let mut after = project.clone();
    let mut commands = Vec::new();
    for t in project.tracks.iter().filter(|t| r.tracks.contains(&t.id)) {
        if t.input
            .as_ref()
            .is_some_and(|i| i.monitor == crate::audio::MonitorMode::On)
        {
            return Err(CannotFreeze::Monitoring(t.id));
        }
        let its_own = Some(t.id) == r.own || target == FreezeTarget::Track(t.id);
        if t.soloed && !its_own {
            return Err(CannotFreeze::SoloedTrack(t.id));
        }
        if t.armed {
            let c = Command::SetTrackArmed {
                id: t.id,
                armed: false,
            };
            let _ = c.clone().apply(&mut after);
            commands.push(c);
        }
    }
    commands.push(freeze_command(&after, target).ok_or(CannotFreeze::Gone)?);
    Ok(if commands.len() == 1 {
        commands.pop().expect("one")
    } else {
        Command::Batch {
            label_key: "cmd.freeze".into(),
            commands,
        }
    })
}

/// A frozen target whose key `commands` move: its old key and its new one.
pub type Rekeyed = (FreezeTarget, String, String);

/// `commands` (one undo step, `label_key`) run through the frozen targets whose key they
/// move — where a file lives, which saving changes (review 1: *Collect* moved a frozen
/// take's file, then its path change was refused): each such target unfrozen before them
/// and frozen again after with its new key. With those targets and their keys, so the
/// renders can follow (the sound is the same file's).
pub fn rekeyed(
    project: &Project,
    label_key: &str,
    commands: Vec<Command>,
) -> (Command, Vec<Rekeyed>) {
    let mut after = project.clone();
    thaw(&mut after);
    for c in &commands {
        let _ = c.clone().apply(&mut after);
    }
    let moved: Vec<Rekeyed> = project
        .frozen_targets()
        .into_iter()
        .filter_map(|t| {
            let old = project.frozen(t)?.content.clone();
            let new = content_key(&after, t)?;
            (new != old).then_some((t, old, new))
        })
        .collect();
    let mut all: Vec<Command> = moved.iter().map(|(t, ..)| unfreeze_command(*t)).collect();
    all.extend(commands);
    for (t, _, new) in &moved {
        let frozen = Some(Frozen {
            content: new.clone(),
        });
        all.push(match *t {
            FreezeTarget::Strip(strip) => Command::SetStripFrozen { strip, frozen },
            FreezeTarget::Track(track) => Command::SetTrackFrozen { track, frozen },
        });
    }
    (
        Command::Batch {
            label_key: label_key.into(),
            commands: all,
        },
        moved,
    )
}

/// Whether `cmd` would be refused by a frozen target, and which (the notice's subject):
/// for menus that grey what they cannot do.
pub fn refused(project: &Project, cmd: &Command) -> Option<FreezeTarget> {
    if !project.any_frozen() {
        return None;
    }
    let mut p = project.clone();
    match cmd.clone().apply(&mut p) {
        Err(CommandError::Frozen(t)) => Some(t),
        _ => None,
    }
}

// ---- playing frozen ---------------------------------------------------------------------

/// A frozen target's render as the engine plays it: opened for streaming from disk (ADR
/// 0050). Opaque here — `cantus-core` does no I/O — like [`crate::SampleData`]'s stream.
#[derive(Clone)]
pub struct FrozenAudio {
    /// The render key it was found by (content, rate, DSP version).
    pub key: String,
    /// Its rate and length.
    pub rate: u32,
    pub frames: u64,
    /// The engine's reader.
    pub reader: crate::StreamRef,
}

impl std::fmt::Debug for FrozenAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenAudio")
            .field("key", &self.key)
            .field("rate", &self.rate)
            .field("frames", &self.frames)
            .finish()
    }
}

impl PartialEq for FrozenAudio {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.reader == other.reader
    }
}

/// A frozen target in a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenPlay {
    /// Its content key.
    pub content: String,
    /// Its render, once the engine has attached it; `None` plays live.
    pub audio: Option<std::sync::Arc<FrozenAudio>>,
    /// A frozen strip while a track is soloed: everything its render holds is silenced by
    /// that solo (the tracks inside it cannot be soloed: they are locked), so the strip is
    /// muted while it plays its render.
    pub silenced: bool,
}

impl FrozenPlay {
    pub fn new(f: &Frozen, silenced: bool) -> Self {
        FrozenPlay {
            content: f.content.clone(),
            audio: None,
            silenced,
        }
    }

    /// Whether it plays its render.
    pub fn plays(&self) -> bool {
        self.audio.is_some()
    }
}

/// Every frozen state taken out: a project that plays live (pattern mode's audition, a
/// render that must not read the render it is making).
pub fn thaw(project: &mut Project) {
    for s in &mut project.strips {
        s.frozen = None;
    }
    for t in &mut project.tracks {
        t.frozen = None;
    }
}

/// The project a render of `target` is made from (review 1): no solo anywhere — solo is
/// no part of a frozen sound (not in its key; live, a solo elsewhere silences a playing
/// frozen strip) — and a frozen strip's only input unmuted (its mute stays live).
pub fn neutral(project: &mut Project, target: FreezeTarget) {
    unsolo(project);
    if let FreezeTarget::Strip(_) = target {
        if let Some(t) = region(project, target).own {
            if let Some(t) = project.track_mut(t) {
                t.muted = false;
            }
        }
    }
}

/// Every solo taken out (a render is of what plays unsoloed).
pub fn unsolo(project: &mut Project) {
    for t in &mut project.tracks {
        t.soloed = false;
    }
    for s in &mut project.strips {
        s.soloed = false;
    }
}

/// Whether frozen strip `strip` is silenced while it plays its render: by its only
/// input's mute or a solo it is not in, or — with no such input — by any track's solo
/// (the tracks inside it cannot be soloed: they are locked).
pub fn silenced(project: &Project, strip: StripId) -> bool {
    match region(project, FreezeTarget::Strip(strip)).own {
        Some(t) => project.track(t).is_some_and(|t| !project.track_audible(t)),
        None => project.any_solo(),
    }
}

/// What rests while frozen targets play their renders: the nodes whose sound only reaches a
/// playing frozen strip — not run, their hosted plug-ins unloaded — and the sources of the
/// playing frozen targets themselves (their inserts or instrument: not run either).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dormant {
    /// Strips that do not run at all.
    pub strips: BTreeSet<StripId>,
    /// Tracks that do not run at all.
    pub tracks: BTreeSet<TrackId>,
    /// Frozen strips playing their renders: their inserts are not run.
    pub roots: BTreeSet<StripId>,
    /// Frozen tracks playing their renders: their instruments are not run.
    pub sources: BTreeSet<TrackId>,
}

impl Dormant {
    /// Whether an insert on `strip` is not run (its hosted plug-in can be unloaded).
    pub fn effect_rests(&self, strip: StripId) -> bool {
        self.strips.contains(&strip) || self.roots.contains(&strip)
    }

    /// Whether `track`'s instrument is not run.
    pub fn instrument_rests(&self, track: TrackId) -> bool {
        self.tracks.contains(&track) || self.sources.contains(&track)
    }

    pub fn is_empty(&self) -> bool {
        self.strips.is_empty()
            && self.tracks.is_empty()
            && self.roots.is_empty()
            && self.sources.is_empty()
    }
}

/// What rests, given which frozen targets have their render at hand (`plays`). A node of a
/// playing frozen strip's region rests when its sound reaches nothing but playing frozen
/// strips (ADR 0064 §4 — review 1: a key strip that also plays to the master, a strip that
/// also sends into a frozen return, a note source that also plays through its own strip
/// went silent). It runs when it reaches something live — the master, or a strip that runs
/// and is no playing frozen strip — or when something live needs it: an insert's key or
/// note source on a strip (or the master) that runs, or what a modulator driving anything
/// that runs listens to or hears the notes of. What a running node gathers then runs too
/// (it reaches it), to a fixed point.
pub fn dormant(project: &Project, plays: &dyn Fn(FreezeTarget) -> bool) -> Dormant {
    let mut d = Dormant::default();
    if project.pattern_mode {
        return d;
    }
    for target in project.frozen_targets() {
        if !plays(target) {
            continue;
        }
        match target {
            FreezeTarget::Strip(root) => {
                d.roots.insert(root);
                let r = region(project, target);
                d.strips.extend(r.strips.iter().filter(|s| **s != root));
                d.tracks.extend(r.tracks.iter().copied());
            }
            FreezeTarget::Track(t) => {
                d.sources.insert(t);
            }
        }
    }
    // A playing root inside another's region rests with it (its render is not heard).
    if d.strips.is_empty() && d.tracks.is_empty() {
        return d;
    }
    // Whether audio routed to `to` is heard: the master (a strip that is not there plays
    // into it too), or a strip that runs and does not play a render (a playing frozen
    // strip gathers nothing).
    let heard = |d: &Dormant, to: Option<StripId>| match to {
        None => true,
        Some(s) => project.strip(s).is_none() || (!d.strips.contains(&s) && !d.roots.contains(&s)),
    };
    loop {
        let mut wake_strips: Vec<StripId> = Vec::new();
        let mut wake_tracks: Vec<TrackId> = Vec::new();
        // What reaches something live.
        for s in project.strips.iter().filter(|s| d.strips.contains(&s.id)) {
            // A send to a strip that is not there goes nowhere.
            let sends = s
                .sends
                .iter()
                .any(|x| project.strip(x.to).is_some() && heard(&d, Some(x.to)));
            if heard(&d, s.output) || sends {
                wake_strips.push(s.id);
            }
        }
        for t in project.tracks.iter().filter(|t| d.tracks.contains(&t.id)) {
            if heard(&d, t.strip) {
                wake_tracks.push(t.id);
            }
        }
        // Inserts that run: on a running strip that is no playing root, and the master's.
        let running = project
            .strips
            .iter()
            .filter(|s| !d.strips.contains(&s.id) && !d.roots.contains(&s.id))
            .flat_map(|s| s.effects.iter())
            .chain(project.master.effects.iter());
        for e in running {
            if let Some(k) = e.sidechain.filter(|k| d.strips.contains(k)) {
                wake_strips.push(k);
            }
            if let Some(t) = e.midi_from.filter(|t| d.tracks.contains(t)) {
                wake_tracks.push(t);
            }
        }
        // Modulators driving anything that runs.
        let runs = |target: &AutoTarget| -> bool {
            match target {
                AutoTarget::TrackGain(t) | AutoTarget::TrackPan(t) => !d.tracks.contains(t),
                AutoTarget::Instrument { track, .. }
                | AutoTarget::Firmus { track, .. }
                | AutoTarget::PluginInstrument { track, .. } => !d.instrument_rests(*track),
                AutoTarget::StripGain(s)
                | AutoTarget::StripPan(s)
                | AutoTarget::StripWidth(s)
                | AutoTarget::Send { strip: s, .. } => !d.strips.contains(s),
                AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. } => project
                    .effect_host(*effect)
                    .is_some_and(|h| !d.effect_rests(h)),
                _ => true,
            }
        };
        for (host, m) in project.all_modulators() {
            if m.muted || !m.routes.iter().any(|r| runs(&r.target)) {
                continue;
            }
            for (listen, notes) in modulator_sources(project, host, m) {
                let s = match listen {
                    Some(Listen::Strip(s)) => Some(s),
                    Some(Listen::Host) => project.host_listen_strip(host),
                    _ => None,
                };
                if let Some(s) = s.filter(|s| d.strips.contains(s)) {
                    wake_strips.push(s);
                }
                if let Some(t) = notes.filter(|t| d.tracks.contains(t)) {
                    wake_tracks.push(t);
                }
            }
        }
        if wake_strips.is_empty() && wake_tracks.is_empty() {
            break;
        }
        // A woken node runs; what it gathers wakes on the next round (it now reaches
        // something live) — a woken playing root plays its render and gathers nothing.
        for s in wake_strips {
            d.strips.remove(&s);
        }
        for t in wake_tracks {
            d.tracks.remove(&t);
        }
    }
    d
}

/// Settle a snapshot whose frozen renders the engine has attached (`FrozenPlay::audio`):
/// a playing frozen strip loses its chain before the width (its render holds it) and is
/// muted while its render is silenced by a solo; the nodes that rest are marked so.
pub fn settle(project: &Project, snapshot: &mut crate::Snapshot) {
    let playing_strip = |s: StripId| {
        snapshot
            .strips
            .iter()
            .find(|x| x.id == s)
            .and_then(|x| x.frozen.as_ref())
            .is_some_and(|f| f.plays())
    };
    let playing_track = |t: TrackId| {
        snapshot
            .tracks
            .iter()
            .find(|x| x.id == t)
            .and_then(|x| x.frozen.as_ref())
            .is_some_and(|f| f.plays())
    };
    let d = dormant(project, &|target| match target {
        FreezeTarget::Strip(s) => playing_strip(s),
        FreezeTarget::Track(t) => playing_track(t),
    });
    for s in &mut snapshot.strips {
        s.rest = d.strips.contains(&s.id);
        // A resting strip's hosted plug-ins are unloaded (`Dormant::effect_rests`): the
        // engine lets their units go too.
        if s.rest {
            s.effects.retain(|e| e.spec.external.is_none());
        }
        if let Some(f) = s.frozen.as_ref().filter(|f| f.plays()) {
            let silenced = f.silenced;
            s.effects.clear();
            s.eq = crate::model::StripEq::default();
            s.phase_invert = false;
            s.swap_channels = false;
            s.fx_enabled = true;
            s.muted |= silenced;
        }
    }
    for t in &mut snapshot.tracks {
        t.rest = d.tracks.contains(&t.id);
        // A hosted instrument that is not run is unloaded (`Dormant::instrument_rests`):
        // the engine lets its unit go (the slot keeps a silent built-in one, never played).
        if d.instrument_rests(t.id) && t.instrument.external().is_some() {
            t.instrument = crate::instrument::InstrumentSpec::default();
        }
    }
}

/// Mark every node of `snapshot` outside `keep` as resting: a freeze's render (or a
/// bounce's) computes only what makes its sound.
pub fn isolate(snapshot: &mut crate::Snapshot, keep: &Region) {
    if keep.whole {
        return;
    }
    for s in &mut snapshot.strips {
        if !keep.strips.contains(&s.id) {
            s.rest = true;
        }
    }
    for t in &mut snapshot.tracks {
        if !keep.tracks.contains(&t.id) {
            t.rest = true;
        }
    }
}

/// What makes the sound of one track through strip `strip`'s chain (a bounce in place
/// with the inserts, ADR 0064): the track, the strip, and what the strip's inserts and the
/// modulators driving them listen to — not the strip's other inputs.
pub fn region_through(project: &Project, track: TrackId, strip: StripId) -> Region {
    let mut r = Region::default();
    if project.track(track).is_some() {
        r.tracks.insert(track);
    }
    let Some(st) = project.strip(strip) else {
        return r;
    };
    r.strips.insert(strip);
    for e in &st.effects {
        if let Some(k) = e.sidechain {
            let kr = region(project, FreezeTarget::Strip(k));
            r.strips.extend(kr.strips);
            r.tracks.extend(kr.tracks);
            r.whole |= kr.whole;
        }
        if let Some(t) = e.midi_from {
            r.tracks.insert(t);
        }
    }
    let inside = r.clone();
    grow_by_modulators(project, &mut r, &|tgt| {
        target_in_strip_region(project, &inside, Some(strip), None, tgt)
    });
    for s in r.strips.clone() {
        if !inside.strips.contains(&s) {
            let sr = region(project, FreezeTarget::Strip(s));
            r.strips.extend(sr.strips);
            r.tracks.extend(sr.tracks);
            r.whole |= sr.whole;
        }
    }
    r
}

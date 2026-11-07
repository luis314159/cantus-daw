//! The recording mode's model (round 36, ADR 0092; BACKLOG §4c): which targets a
//! *Prepare to record* bakes, and the edit lock the mode puts on the project.
//!
//! The mode is built **on freeze** (ADR 0064): each strip that feeds the master is frozen,
//! its render taken after the inserts, sends and effect automation and before the fader and
//! pan, so the engine only reads files and sums them. What is new is *which* targets
//! ([`plan`]) and a stricter lock ([`refuses`]): while the mode is on, everything that would
//! change the sound is refused — the frozen guard already refuses it inside a baked region;
//! this lock adds the project-wide edits (tempo, structure, pattern mode, un-freezing), the
//! mix controls when the user chose to lock them, and says "Recording mode" instead of
//! offering *Unfreeze*.
//!
//! The lock is [`touches`]-based: it reuses the frozen guard's classification of every
//! [`Command`] (a `match` with no wildcard, so a new command does not build until it is
//! classified) and adds nothing of its own to maintain.

use crate::automation::AutoTarget;
use crate::command::Command;
use crate::freeze::{self, FreezeTarget, Touch};
use crate::model::{Project, StripId, TrackId};
use std::collections::BTreeSet;

/// Why a strip or track that feeds the master is not baked: it stays live.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// A track in it is armed: the musician's own path, live with the least delay.
    Armed(TrackId),
    /// A track in it monitors its input *On* (a render cannot hold a live input).
    Monitoring(TrackId),
    /// A track or strip inside it is soloed (a render holds the whole region).
    Soloed,
    /// A dancer's own track (it makes no sound and must keep running).
    Dancer,
    /// A track in it plays outboard gear (ADR 0094): an offline render cannot play it.
    External(TrackId),
    /// A mix cue (ADR 0093) solos a strip inside it: a render holds no solo.
    CueSolo(StripId),
    /// A kit whose pads play on strips of their own goes straight to the master (ADR 0100):
    /// its pads' strips are baked, its own output is not (freeze refuses the track). // r37:drums
    SplitKit(TrackId), // r37:drums
}

/// What *Prepare to record* does with the project as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// The targets to render, in the desk's order: strips (a bus is one render, with what
    /// it sums and what feeds it by send), then tracks that go straight to the master.
    pub bake: Vec<FreezeTarget>,
    /// Frozen already by the user: kept as they are (and kept when the mode is left).
    pub already: Vec<FreezeTarget>,
    /// What stays live, and why.
    pub live: Vec<(FreezeTarget, Why)>,
}

impl Plan {
    /// Whether there is nothing to bake (nothing to gain).
    pub fn is_empty(&self) -> bool {
        self.bake.is_empty()
    }
}

/// The plan for `project`: every strip whose output is the master, and every track that
/// goes straight to it, is baked — unless it holds an armed track (the musician's live
/// path), or cannot be rendered as a whole.
pub fn plan(project: &Project) -> Plan {
    let mut out = Plan::default();
    for s in project.strips.iter().filter(|s| s.output.is_none()) {
        let target = FreezeTarget::Strip(s.id);
        if s.frozen.is_some() {
            out.already.push(target);
            continue;
        }
        let r = freeze::region(project, target);
        if r.tracks.is_empty() {
            continue;
        }
        match why_live(project, &r, s.id) {
            Some(why) => out.live.push((target, why)),
            None => out.bake.push(target),
        }
    }
    for t in project.tracks.iter() {
        let straight = t.strip.is_none_or(|s| project.strip(s).is_none());
        if !straight || t.clips.is_empty() {
            continue;
        }
        let target = FreezeTarget::Track(t.id);
        if t.frozen.is_some() {
            out.already.push(target);
            continue;
        }
        if crate::inst::dancer::is_dancer(&t.instrument) {
            out.live.push((target, Why::Dancer));
            continue;
        }
        // r37:drums — a split kit straight to the master: its pads' strips are baked as
        // strips; the track's own output (one render cannot hold its buses) stays live.
        if !t.instrument.output_strips().is_empty() {
            out.live.push((target, Why::SplitKit(t.id)));
            continue;
        }
        let why = if let Some(u) = external_in(project, &[t.id]) {
            Some(Why::External(u))
        } else if t.armed {
            Some(Why::Armed(t.id))
        } else if t
            .input
            .as_ref()
            .is_some_and(|i| i.monitor == crate::audio::MonitorMode::On)
        {
            Some(Why::Monitoring(t.id))
        } else if t.soloed {
            Some(Why::Soloed)
        } else {
            None
        };
        match why {
            Some(why) => out.live.push((target, why)),
            None => out.bake.push(target),
        }
    }
    out
}

/// The first track of `tracks` that plays enabled outboard gear (ADR 0094).
fn external_in(project: &Project, tracks: &[TrackId]) -> Option<TrackId> {
    project
        .external_units
        .iter()
        .find(|u| u.enabled && tracks.contains(&u.track))
        .map(|u| u.track)
}

fn why_live(project: &Project, r: &freeze::Region, root: StripId) -> Option<Why> {
    // Outboard gear first: freeze refuses it (CannotFreeze::External).
    let tracks: Vec<TrackId> = r.tracks.iter().copied().collect();
    if let Some(t) = external_in(project, &tracks) {
        return Some(Why::External(t));
    }
    for t in project.tracks.iter().filter(|t| r.tracks.contains(&t.id)) {
        if t.armed {
            return Some(Why::Armed(t.id));
        }
        if t.input
            .as_ref()
            .is_some_and(|i| i.monitor == crate::audio::MonitorMode::On)
        {
            return Some(Why::Monitoring(t.id));
        }
        if crate::inst::dancer::is_dancer(&t.instrument) {
            return Some(Why::Dancer);
        }
        // Its own solo stays live in a freeze, but a solo inside a bus is lost in a render.
        if t.soloed && r.own != Some(t.id) {
            return Some(Why::Soloed);
        }
    }
    if project
        .strips
        .iter()
        .any(|s| s.soloed && r.strips.contains(&s.id) && s.id != root)
    {
        return Some(Why::Soloed);
    }
    freeze::cue_solo_inside(project, FreezeTarget::Strip(root)).map(Why::CueSolo)
}

/// The edit lock (the owner's choices of 2026-10-10, and what stays live).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditLock {
    /// Fader, pan, width, mute and solo of a baked strip (and their automation) act live
    /// at the sum; `false` locks them too.
    pub mix_live: bool,
    /// The master chain runs live on the sum; `false`: the master is baked (one render),
    /// and nothing of it moves (ADR 0092 §3).
    pub master_live: bool,
    /// The tracks and strips of the armed regions: the musician's, live and editable.
    /// Everything else that stays live (a strip that could not be rendered, a monitoring,
    /// soloed or dancer's region) is locked as the baked ones are: an edit there would
    /// change what plays under the take.
    pub live_tracks: BTreeSet<TrackId>,
    pub live_strips: BTreeSet<StripId>,
}

impl Default for EditLock {
    fn default() -> Self {
        EditLock {
            mix_live: true,
            master_live: true,
            live_tracks: BTreeSet::new(),
            live_strips: BTreeSet::new(),
        }
    }
}

impl EditLock {
    /// The lock with `editable` (the armed roots, and those brought live to arm a track)
    /// open for edits: their regions' tracks and strips.
    pub fn new(
        project: &Project,
        mix_live: bool,
        master_live: bool,
        editable: &[FreezeTarget],
    ) -> Self {
        let mut l = EditLock {
            // A baked master holds the whole mix: nothing of it moves.
            mix_live: mix_live && master_live,
            master_live,
            ..EditLock::default()
        };
        for t in editable {
            let r = freeze::region(project, *t);
            l.live_tracks.extend(r.tracks);
            l.live_strips.extend(r.strips);
        }
        l
    }
}

/// Whether `cmd` is one of the master's own.
fn is_master(cmd: &Command) -> bool {
    use Command::*;
    match cmd {
        SetMasterGain { .. }
        | SetMasterPan { .. }
        | SetMasterWidth { .. }
        | SetMasterPhaseInvert { .. }
        | SetMasterFxEnabled { .. } => true,
        InsertEffect { strip, .. }
        | RemoveEffect { strip, .. }
        | MoveEffect { strip, .. }
        | ReplaceEffect { strip, .. }
        | SetEffectBypass { strip, .. }
        | SetEffectParam { strip, .. }
        | ClearEffectParam { strip, .. }
        | SetEffectSidechain { strip, .. }
        | SetEffectMix { strip, .. }
        | SetPluginParam { strip, .. }
        | SetPluginState { strip, .. }
        | SetEffectMidiSource { strip, .. } => *strip == StripId::MASTER,
        SetStripEq { id, .. }
        | SetStripPhaseInvert { id, .. }
        | SetStripSwapChannels { id, .. } => *id == StripId::MASTER,
        _ => false,
    }
}

/// The mix controls of a baked root: the commands that stay live while the mix is live.
fn is_mix_control(cmd: &Command) -> bool {
    use Command::*;
    matches!(
        cmd,
        SetStripGain { .. }
            | SetStripPan { .. }
            | SetStripWidth { .. }
            | SetStripMute { .. }
            | SetStripSolo { .. }
            | SetTrackGain { .. }
            | SetTrackPan { .. }
            | SetTrackMute { .. }
            | SetTrackSolo { .. }
    )
}

fn strip_root(project: &Project, s: StripId) -> bool {
    project.strip(s).is_some_and(|x| x.frozen.is_some())
}

fn track_root(project: &Project, t: TrackId) -> bool {
    project.track(t).is_some_and(|x| x.frozen.is_some())
}

/// A frozen strip's only input: its mute and solo stay live with the strip's.
fn owned(project: &Project, t: TrackId) -> bool {
    project.frozen_targets().into_iter().any(|f| match f {
        FreezeTarget::Strip(_) => freeze::region(project, f).own == Some(t),
        FreezeTarget::Track(_) => false,
    })
}

fn dancer(project: &Project, t: TrackId) -> bool {
    project
        .track(t)
        .is_some_and(|x| crate::inst::dancer::is_dancer(&x.instrument))
}

/// A desk recall (or a solo-safe switch, ADR 0093) that changes only what a fader strip's
/// live controls are — fader, pan, width, mute, solo — and no send: a mix control, judged
/// strip by strip as the faders are.
fn mix_recall_of_levels(cmd: &Command, project: &Project) -> bool {
    use crate::mix_cmd::MixCommand as M;
    let (state, scope, master, safe) = match cmd {
        Command::Mix(M::SetSoloSafe { .. }) => return true,
        Command::Mix(M::Recall {
            id,
            scope,
            master,
            include_safe,
        }) => match project.mix.snapshot(*id) {
            Some(s) => (s.state.clone(), scope.clone(), *master, *include_safe),
            None => return false,
        },
        Command::Mix(M::Apply { state }) => ((**state).clone(), Vec::new(), true, true),
        _ => return false,
    };
    let mut after = project.clone();
    crate::mix_snap::apply_state(&mut after, &state, &scope, master, safe, false);
    project.strips.iter().zip(&after.strips).all(|(a, b)| {
        a.sends == b.sends
            && a.phase_invert == b.phase_invert
            && a.swap_channels == b.swap_channels
            && a.fx_enabled == b.fx_enabled
            && a.effects == b.effects
            && a.eq == b.eq
    })
}

/// Whether the lock lets `touch` through. `control`: the command is a mix control (or an
/// automation curve: a gain curve on one).
fn allowed(lock: &EditLock, project: &Project, touch: &Touch, control: bool, depth: u8) -> bool {
    use Touch::*;
    let mix = lock.mix_live && control;
    match touch {
        // A dancer makes no sound (ADR 0087: a silent instrument whose unit only tells the
        // screen where the song is): its settings change what is seen, never a sample.
        TrackSource(t) => lock.live_tracks.contains(t) || dancer(project, *t),
        TrackMix(t) => lock.live_tracks.contains(t) || (mix && track_root(project, *t)),
        TrackAudibility(t) => {
            lock.live_tracks.contains(t) || (mix && (track_root(project, *t) || owned(project, *t)))
        }
        StripChain(s) | StripInput(s) => lock.live_strips.contains(s),
        StripMix(s) | StripLevel(s) => {
            lock.live_strips.contains(s)
                || (mix && strip_root(project, *s))
                // A VCA's fader, mute and solo (ADR 0093) are a mix control: allowed while
                // the mix is live and every strip it reaches may move.
                || (mix
                    && project.is_vca(*s)
                    && project
                        .vca_members(*s)
                        .iter()
                        .all(|m| allowed(lock, project, &StripMix(*m), control, depth)))
        }
        Target(a) => {
            let curve = |t: Touch| allowed(lock, project, &t, true, depth);
            match a {
                AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth => {
                    lock.master_live
                }
                AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. } => {
                    match project.effect_host(*effect) {
                        Some(h) if h == StripId::MASTER => lock.master_live,
                        Some(h) => allowed(lock, project, &StripChain(h), false, depth),
                        None => false,
                    }
                }
                AutoTarget::TrackGain(t) | AutoTarget::TrackPan(t) => curve(TrackMix(*t)),
                AutoTarget::StripGain(s) | AutoTarget::StripPan(s) => curve(StripLevel(*s)),
                AutoTarget::StripWidth(s) => curve(StripMix(*s)),
                AutoTarget::Send { strip, .. } => curve(StripMix(*strip)),
                AutoTarget::Instrument { track, .. }
                | AutoTarget::Firmus { track, .. }
                | AutoTarget::PluginInstrument { track, .. }
                | AutoTarget::NoteFx { track, .. } => {
                    allowed(lock, project, &TrackSource(*track), false, depth)
                }
                // A modulator's amount scales every route it drives: each one judged.
                AutoTarget::ModAmount(m) => {
                    if depth > 0 {
                        return false;
                    }
                    match project.modulator(*m) {
                        Some((_, _, m)) => m.routes.iter().all(|r| {
                            allowed(lock, project, &Target(r.target.clone()), true, depth + 1)
                        }),
                        None => false,
                    }
                }
            }
        }
        Global => false,
    }
}

/// Whether the recording mode, with `lock`, refuses `cmd` on `project` (as it is now).
///
/// Refused: un-freezing and freezing (the mode's own business), pattern mode (it plays
/// every chain live), anything inside a baked region, every project-wide edit but the
/// master's (while it is live), anything outside the armed regions — and, when the mix
/// is locked, the baked strips' own controls. Allowed: what no sound depends on (names,
/// colours, markers, the loop, the metronome), the armed regions and their takes, a
/// dancer's settings, the master while it is live, and a baked strip's fader, pan, width,
/// mute and solo while the mix is live. Every touch is judged on its own: a command that
/// moves several strips (a group, a VCA, a snapshot recall) is refused whole when one of
/// them is locked.
pub fn refuses(lock: &EditLock, cmd: &Command, project: &Project) -> bool {
    use Command::*;
    match cmd {
        Batch { commands, .. } => return commands.iter().any(|c| refuses(lock, c, project)),
        SetStripFrozen { .. } | SetTrackFrozen { .. } | SetPatternMode { .. } => return true,
        _ => {}
    }
    // Inside a baked region the frozen guard already says no: the mode says it first.
    if freeze::refuses(project, cmd).is_some() {
        return true;
    }
    let master = is_master(cmd);
    let control = is_mix_control(cmd) || mix_recall_of_levels(cmd, project);
    freeze::touches(cmd, project)
        .iter()
        .any(|touch| match touch {
            Touch::Global => !(master && lock.master_live),
            t => !allowed(lock, project, t, control, 0),
        })
}

/// A snapshot whose master plays the mode's render (ADR 0092 §3): every node a freeze
/// holds rests (its sound is in the master's render; its hosted plug-ins are let go), and
/// the master's chain is taken out (it is in the render too). The live strips play on.
pub fn rest_for_master(project: &Project, snapshot: &mut crate::Snapshot) {
    let locks = freeze::Locks::of(project);
    for s in &mut snapshot.strips {
        if locks.chain.contains(&s.id) {
            s.rest = true;
            s.effects.retain(|e| e.spec.external.is_none());
        }
    }
    for t in &mut snapshot.tracks {
        if locks.source.contains(&t.id) {
            t.rest = true;
            t.notes_only = false;
        }
    }
    crate::padout::settle_taps(snapshot); // r37:drums — a pad's output rests with its kit
    snapshot.master_effects.clear();
}

/// Bumped when the baked master's render changes form (its key is then another).
pub const MASTER_VERSION: u32 = 1;

/// `project` with what stays live muted and no solo: what the baked master's render holds
/// (ADR 0092 §3).
pub fn muted(project: &Project, live: &[FreezeTarget]) -> Project {
    let mut p = project.clone();
    for t in live {
        match t {
            FreezeTarget::Strip(s) => {
                if let Some(s) = p.strip_mut(*s) {
                    s.muted = true;
                }
            }
            FreezeTarget::Track(t) => {
                if let Some(t) = p.track_mut(*t) {
                    t.muted = true;
                }
            }
        }
    }
    for s in &mut p.strips {
        s.soloed = false;
    }
    for t in &mut p.tracks {
        t.soloed = false;
    }
    // No cue brings a live strip back into the render, nor solos anything in it (round
    // 36, the integrated review: a cue's recall unmuted them, +6 dB, doubled).
    let silent: Vec<StripId> = live
        .iter()
        .filter_map(|t| match t {
            FreezeTarget::Strip(s) => Some(*s),
            FreezeTarget::Track(_) => None,
        })
        .collect();
    for snap in &mut p.mix.snapshots {
        snap.state.strips.retain(|st| !silent.contains(&st.id));
        for st in &mut snap.state.strips {
            st.soloed = false;
        }
    }
    p
}

/// The baked master's content key: SHA-256 of the project so muted, every byte of it
/// (anything that could change the master's sound changes the key; a rename only renders
/// it again).
pub fn master_key(project: &Project, live: &[FreezeTarget]) -> String {
    let p = muted(project, live);
    let json = serde_json::to_vec(&p).unwrap_or_default();
    let mut bytes = format!("cantus-master-bake/{MASTER_VERSION}\n").into_bytes();
    bytes.extend_from_slice(&json);
    freeze::sha256_hex(&bytes)
}

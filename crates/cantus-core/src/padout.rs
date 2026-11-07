//! Instruments with several outputs, and a kit split to strips (Pulsus III, ADR 0100).
//!
//! **The model.** An instrument may play into more than its track's strip: each pad of a
//! kit says where it is heard ([`crate::DrumPad::output`]: `None` the kit's own strip, or a
//! strip of its own). The pads that name one strip share one **bus**; bus 0 is the track's
//! own output ([`crate::InstrumentSpec::output_strips`] numbers the others).
//!
//! **The snapshot.** Each bus `k ≥ 1` of a track becomes a **tap**: one more
//! [`TrackSnapshot`] after the project's own tracks, which plays nothing of its own — only
//! what its source renders into its bus — through its own level (the source's), its own
//! line and its own strip. To the rest of the engine a tap is a track like any other, so
//! delay compensation, solo and mute, sends, VCAs, the recording mode, freeze and stems
//! reach a pad's strip by the paths every track already has. Taps come after the project's
//! tracks, so a project track's index in the snapshot is still its index in the project.
//!
//! **Split to strips** ([`split_with`]) gives every pad that sounds a strip of its own,
//! named after it, routed into the kit's strip, in pad order, with the 0 dB pan law (a
//! balance: centred, it passes the pad untouched) — so the split kit, every new strip at
//! unity, sums to the unsplit kit **to the bit** for a model 3 kit (each pad on its own
//! lane, the lanes in pad order: `cantus_dsp::pulsus::LANES_MODEL`). **Merge back**
//! ([`merge`]) returns every pad to the kit and removes the strips nothing else uses. Each
//! is one undo step.

use crate::command::Command;
use crate::model::{MixerStrip, PanLaw, Project, StripId, Track, TrackId};
use crate::pads::{DrumPad, PadVoice};
use crate::snapshot::{Snapshot, TrackSnapshot};
use crate::vca::StripKind;

/// A track that carries one of another track's outputs (see the module's docs).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tap {
    /// Index into `Snapshot::tracks` of the track whose instrument renders this bus.
    pub source: usize,
    /// The bus, 1 and up.
    pub bus: u8,
}

/// The top bit of a tap's id: no project allocates ids that high (`Project::alloc_id`
/// counts up from 1), so a tap's id is never a track's.
pub const TAP_BIT: u64 = 1 << 63;

/// The id of track `source`'s bus `bus` in the snapshot.
pub fn tap_id(source: TrackId, bus: u8) -> TrackId {
    TrackId(TAP_BIT | (source.0 << 8) | u64::from(bus))
}

/// Whether `id` names a tap rather than a track of the project.
pub fn is_tap(id: TrackId) -> bool {
    id.0 & TAP_BIT != 0
}

/// Append a tap for every bus of every track (`Snapshot::compile`): the bus's strip, or the
/// source's own when the strip named is gone (a hand-edited file); the source's level,
/// balance, mute and solo, delay and lateness.
pub(crate) fn append_taps(project: &Project, tracks: &mut Vec<TrackSnapshot>) {
    let count = tracks.len().min(project.tracks.len());
    for i in 0..count {
        let outs = project.tracks[i].instrument.output_strips();
        for (k, s) in outs.into_iter().enumerate() {
            let bus = (k + 1).min(u8::MAX as usize) as u8;
            let at = tracks.len();
            let src = &tracks[i];
            let strip = project.strip_index(s).or(src.strip);
            let name = match project.strip(s) {
                Some(st) if !st.name.is_empty() => st.name.clone(),
                _ => format!("{} {bus}", src.name),
            };
            let tap = TrackSnapshot {
                id: tap_id(src.id, bus),
                name,
                gain: src.gain,
                pan: src.pan,
                audible: src.audible,
                strip,
                instrument: crate::instrument::InstrumentSpec::default(),
                events: Vec::new(),
                controls: Vec::new(),
                control_kinds: Vec::new(),
                expr: Vec::new(),
                lane_count: 0,
                samples: Vec::new(),
                assets: Vec::new(),
                audio: Vec::new(),
                input: None,
                armed: false,
                frozen: None,
                rest: src.rest,
                notes_only: false,
                lateness: src.lateness,
                external: Vec::new(),
                note_fx: Vec::new(),
                direct: Vec::new(),
                buses: Vec::new(),
                tap: Some(Tap { source: i, bus }),
            };
            tracks.push(tap);
            tracks[i].buses.push(at);
        }
    }
}

/// Settle the taps after the tracks and strips have been settled (a freeze, the recording
/// mode's baked master, a render isolated to a region): a tap rests when its source does
/// not play its instrument (it rests, plays only its notes, or plays a render), or when the
/// strip it plays into does not run or plays a render of its own.
pub fn settle_taps(snapshot: &mut Snapshot) {
    for i in 0..snapshot.tracks.len() {
        let Some(tap) = snapshot.tracks[i].tap else {
            continue;
        };
        let source_quiet = snapshot
            .tracks
            .get(tap.source)
            .is_none_or(|s| s.rest || s.notes_only || s.frozen.as_ref().is_some_and(|f| f.plays()));
        let strip_quiet = snapshot.tracks[i]
            .strip
            .and_then(|s| snapshot.strips.get(s))
            .is_some_and(|s| s.rest || s.frozen.as_ref().is_some_and(|f| f.plays()));
        snapshot.tracks[i].rest = source_quiet || strip_quiet;
    }
}

/// Whether a pad makes a sound of its own: a synthesized voice, or a sample pad with a file
/// or a layer. A pad that does not gets no strip of its own when a kit is split.
pub fn sounds(pad: &DrumPad) -> bool {
    pad.voice != PadVoice::Sample || pad.sample.is_some() || !pad.layers.is_empty()
}

/// Why a kit cannot be split or merged, or a chain changed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The track is gone, or its instrument is not a kit.
    NotAKit,
    /// Every pad that sounds already plays on a strip of its own (to split), or none does
    /// (to merge); the pad has no such insert in its chain.
    Nothing,
    /// The pad is not in the kit.
    NoPad,
    /// The engine has no room for it: every pad's output is a track the engine plays
    /// ([`ENGINE_TRACKS`]) and a strip ([`ENGINE_STRIPS`]); past them a pad would fall
    /// silently back onto the kit's strip (round 37's review, item 6).
    NoRoom,
}

/// How many tracks the engine plays — the project's and every tap — and how many strips
/// it mixes (`cantus_engine::MAX_TRACKS`, `MAX_STRIPS`, held equal by a test there).
pub const ENGINE_TRACKS: usize = 256;
pub const ENGINE_STRIPS: usize = 256;

/// How many tracks the engine plays for `project`: its own and one tap per output of an
/// instrument besides its track's.
pub fn engine_tracks(project: &Project) -> usize {
    project.tracks.len()
        + project
            .tracks
            .iter()
            .map(|t| t.instrument.output_strips().len())
            .sum::<usize>()
}

/// `cmd` if the engine has room for what it makes ([`Refusal::NoRoom`] otherwise): no
/// more tracks and taps than [`ENGINE_TRACKS`], no more strips than [`ENGINE_STRIPS`] —
/// judged only when it adds to them.
fn with_room(project: &Project, cmd: Command) -> Result<Command, Refusal> {
    let mut after = project.clone();
    if cmd.clone().apply(&mut after).is_err() {
        return Ok(cmd); // the bus says why
    }
    let tracks = engine_tracks(&after);
    let strips = after.strips.len();
    let grows_tracks = tracks > engine_tracks(project);
    let grows_strips = strips > project.strips.len();
    if (grows_tracks && tracks > ENGINE_TRACKS) || (grows_strips && strips > ENGINE_STRIPS) {
        return Err(Refusal::NoRoom);
    }
    Ok(cmd)
}

/// Whether `s` is a pad's chain (a strip of kind `Chain`).
pub fn is_chain(project: &Project, s: StripId) -> bool {
    project.strip(s).is_some_and(|x| x.kind == StripKind::Chain)
}

/// The chain a pad plays into, if it has one.
pub fn chain_of(project: &Project, pad: &DrumPad) -> Option<StripId> {
    pad.output.filter(|s| is_chain(project, *s))
}

/// Where a pad of `t`'s kit is heard: `None` on the kit's own strip (directly, or through
/// its chain), `Some` on a strip of its own (split), through its chain or not.
pub fn heard_on(project: &Project, t: &Track, pad: &DrumPad) -> Option<StripId> {
    let o = pad.output?;
    if is_chain(project, o) {
        let out = project.strip(o).and_then(|c| c.output);
        if out != t.strip {
            out
        } else {
            None
        }
    } else {
        Some(o)
    }
}

/// Whether some pad of `track`'s kit plays on a strip of its own (it can be merged back).
pub fn is_split(project: &Project, track: TrackId) -> bool {
    project.track(track).is_some_and(|t| {
        t.instrument
            .pulsus()
            .is_some_and(|k| k.pads.iter().any(|p| heard_on(project, t, p).is_some()))
    })
}

/// *Split to strips*: one command (a batch, one undo step) that gives every pad of
/// `track`'s kit that sounds and is still heard on the kit's strip a strip of its own —
/// named after the pad, placed right after the kit's strip in pad order, routed into the
/// kit's strip (the master when the kit has none), its pan law a balance — and moves the
/// pad onto it (a pad with a chain: its chain's output). `mint` makes a strip with a fresh
/// id (`CommandBus::new_strip`).
pub fn split_with(
    project: &Project,
    track: TrackId,
    mut mint: impl FnMut(&str) -> MixerStrip,
) -> Result<Command, Refusal> {
    let t = project.track(track).ok_or(Refusal::NotAKit)?;
    let kit = t.instrument.pulsus().ok_or(Refusal::NotAKit)?;
    let pads: Vec<&DrumPad> = kit
        .pads
        .iter()
        .filter(|p| heard_on(project, t, p).is_none() && sounds(p))
        .collect();
    if pads.is_empty() {
        return Err(Refusal::Nothing);
    }
    let kit_strip = t.strip.filter(|s| project.strip(*s).is_some());
    // Right after the kit's strip (and after any strip already split from it), or at the
    // end of the desk.
    let after = kit_strip
        .and_then(|s| project.strip_index(s))
        .map(|i| {
            let mut at = i + 1;
            while project.strips.get(at).is_some_and(|x| {
                x.output == kit_strip && project.tracks_on_strip(x.id).any(|o| o.id == track)
            }) {
                at += 1;
            }
            at
        })
        .unwrap_or(project.strips.len());
    let dock = kit_strip
        .and_then(|s| project.strip(s))
        .map(|s| s.dock)
        .unwrap_or_default();
    let mut commands = Vec::with_capacity(pads.len() * 2);
    for (k, pad) in pads.iter().enumerate() {
        let mut strip = mint(&pad.name);
        strip.kind = StripKind::Pad;
        strip.output = kit_strip;
        strip.pan_law = Some(PanLaw::ZeroDb);
        strip.dock = dock;
        let id = strip.id;
        commands.push(Command::InsertStrip {
            index: after + k,
            strip: Box::new(strip),
        });
        commands.push(match chain_of(project, pad) {
            Some(chain) => Command::SetStripOutput {
                id: chain,
                output: Some(id),
            },
            None => Command::SetPadOutput {
                track,
                pad: pad.id,
                output: Some(id),
            },
        });
    }
    with_room(
        project,
        Command::Batch {
            label_key: "cmd.split_kit".into(),
            commands,
        },
    )
}

/// [`split_with`] through a command bus, which mints the strips.
pub fn split(bus: &mut crate::bus::CommandBus, track: TrackId) -> Result<Command, Refusal> {
    let project = bus.project().clone();
    split_with(&project, track, |name| bus.new_strip(name))
}

/// *Merge back*: one command that returns every pad of `track`'s kit to the kit's own
/// strip (a pad's chain kept, its output back on the kit's strip) and removes each strip a
/// pad was heard on that nothing else uses afterwards (no other track or pad, no other
/// strip routed or sending into it, no insert keyed from it, no strip following it as a
/// VCA) — what *Split to strips* made, and only that.
pub fn merge(project: &Project, track: TrackId) -> Result<Command, Refusal> {
    let t = project.track(track).ok_or(Refusal::NotAKit)?;
    let kit = t.instrument.pulsus().ok_or(Refusal::NotAKit)?;
    let mut outs: Vec<StripId> = Vec::new();
    let mut moved: Vec<StripId> = Vec::new();
    let mut commands = Vec::new();
    for p in &kit.pads {
        let Some(own) = heard_on(project, t, p) else {
            continue;
        };
        if !outs.contains(&own) {
            outs.push(own);
        }
        commands.push(match chain_of(project, p) {
            Some(chain) => {
                moved.push(chain);
                Command::SetStripOutput {
                    id: chain,
                    output: t.strip,
                }
            }
            None => Command::SetPadOutput {
                track,
                pad: p.id,
                output: None,
            },
        });
    }
    if outs.is_empty() {
        return Err(Refusal::Nothing);
    }
    for s in outs {
        let others = project
            .tracks
            .iter()
            .any(|o| o.strip == Some(s) || (o.id != track && o.feeds_through_bus(s)));
        let routed = project
            .strips
            .iter()
            .filter(|x| !moved.contains(&x.id))
            .any(|x| x.feeds().any(|to| to == s))
            || project.strips.iter().any(|x| x.keys().any(|k| k == s))
            || project.strips.iter().any(|x| x.vcas.contains(&s))
            || project
                .master
                .effects
                .iter()
                .any(|e| e.sidechain == Some(s));
        if !others && !routed && project.strip(s).is_some() {
            commands.push(Command::RemoveStrip { id: s });
        }
    }
    // And every strip made for pads that nothing uses after it and that played into the
    // kit's strip (left by a kit loaded before, a pad removed): the same step.
    let mut after = project.clone();
    let merged = Command::Batch {
        label_key: "cmd.merge_kit".into(),
        commands: commands.clone(),
    };
    if merged.apply(&mut after).is_ok() {
        for s in orphans(&after) {
            if after.strip(s).is_some_and(|x| x.output == t.strip) {
                commands.push(Command::RemoveStrip { id: s });
            }
        }
    }
    Ok(Command::Batch {
        label_key: "cmd.merge_kit".into(),
        commands,
    })
}

/// Whether `track`'s kit can be split (some pad that sounds is still heard on the kit).
pub fn can_split(project: &Project, track: TrackId) -> bool {
    project.track(track).is_some_and(|t| {
        t.instrument.pulsus().is_some_and(|k| {
            k.pads
                .iter()
                .any(|p| heard_on(project, t, p).is_none() && sounds(p))
        })
    })
}

/// Whether `track`'s kit has pads on strips of their own (it can be merged back).
pub fn can_merge(project: &Project, track: TrackId) -> bool {
    is_split(project, track)
}

/// The kit track a strip belongs to as a pad's strip or chain (its first pad's track).
pub fn kit_of_strip(project: &Project, strip: StripId) -> Option<TrackId> {
    project
        .tracks
        .iter()
        .find(|t| t.feeds_through_bus(strip) || chain_into(project, t, strip))
        .map(|t| t.id)
}

/// Whether one of `t`'s pads' chains outputs into `strip`.
fn chain_into(project: &Project, t: &Track, strip: StripId) -> bool {
    t.instrument.pulsus().is_some_and(|k| {
        k.pads.iter().any(|p| {
            chain_of(project, p)
                .and_then(|c| project.strip(c))
                .is_some_and(|c| c.output == Some(strip))
        })
    })
}

/// Whether `s` was made for a pad: a pad's own strip from *Split to strips*, or a pad's
/// chain — by its kind, never by its pan law (a strip of the user's may have its own).
pub fn pad_made(s: &MixerStrip) -> bool {
    matches!(s.kind, StripKind::Chain | StripKind::Pad)
}

/// The strips made for pads that nothing uses any more in `project`: no track or pad plays
/// into them, no other strip routes, sends or keys into them, no strip follows them as a
/// VCA, no master insert is keyed from them — a kit replaced, a pad removed, a kit's track
/// deleted. Chains first: an orphan chain leaves the pad strip it played into unused too.
pub fn orphans(project: &Project) -> Vec<StripId> {
    let mut gone: Vec<StripId> = Vec::new();
    loop {
        let unused = project.strips.iter().find(|s| {
            pad_made(s)
                && !gone.contains(&s.id)
                && project.tracks_on_strip(s.id).next().is_none()
                && !project
                    .strips
                    .iter()
                    .filter(|x| !gone.contains(&x.id))
                    .any(|x| x.feeds().any(|to| to == s.id) || x.keys().any(|k| k == s.id))
                && !project.strips.iter().any(|x| x.vcas.contains(&s.id))
                && !project
                    .master
                    .effects
                    .iter()
                    .any(|e| e.sidechain == Some(s.id))
        });
        match unused {
            Some(s) => gone.push(s.id),
            None => break,
        }
    }
    gone
}

/// Whether `cmd` can leave strips made for pads unused: a kit's instrument replaced or its
/// pads removed, a track deleted (alone or in a batch). Cheap: the bus asks it of every
/// command.
pub fn may_orphan(cmd: &Command) -> bool {
    match cmd {
        Command::SetInstrument { .. } | Command::RemoveTrack { .. } | Command::RemovePad { .. } => {
            true
        }
        Command::Batch { commands, .. } => commands.iter().any(may_orphan),
        _ => false,
    }
}

/// `cmd`, and in the same undo step the removal of every strip made for pads that it
/// leaves unused ([`orphans`]): what a kit's pads played on goes with the kit (loading
/// another kit, another instrument, the track deleted). `cmd` as it is when it leaves none,
/// or when it does not apply.
pub fn with_orphans_removed(project: &Project, cmd: Command) -> Command {
    let mut after = project.clone();
    let before = orphans(project);
    if cmd.clone().apply(&mut after).is_err() {
        return cmd;
    }
    let left: Vec<StripId> = orphans(&after)
        .into_iter()
        .filter(|s| !before.contains(s))
        .collect();
    if left.is_empty() {
        return cmd;
    }
    let label = cmd.label_key().to_string();
    let mut commands = vec![cmd];
    commands.extend(left.into_iter().map(|id| Command::RemoveStrip { id }));
    Command::Batch {
        label_key: label,
        commands,
    }
}

/// A kit loaded onto `old`'s track keeps its pads' outputs: every new pad on an old pad's
/// pitch takes that pad's output first; then each new pad still without one takes the
/// output of the first old pad of its voice not taken yet — a synthesised voice only: a
/// sample kit's pads are all `Sample`, and matching on that would hand old strips to
/// unrelated pads in order (round 37's review, item 5) — then of the old pad of its name,
/// then of the old pad in its place. So a split kit stays split and a pad's chain stays with
/// the drum that replaces it (a new rim never takes the snare's strip while the snare is
/// there to take it).
pub fn carry_outputs(old: &crate::pads::PulsusParams, new: &mut crate::pads::PulsusParams) {
    let mut taken: Vec<usize> = Vec::new();
    let mut done: Vec<bool> = vec![false; new.pads.len()];
    for (n, pad) in new.pads.iter_mut().enumerate() {
        if let Some(i) = old
            .pads
            .iter()
            .position(|o| o.pitch == pad.pitch && o.output.is_some())
        {
            taken.push(i);
            pad.output = old.pads[i].output;
            done[n] = true;
        }
    }
    // The later passes, in order: the voice (not a sample's), the name, the place.
    type Matches = fn(usize, &DrumPad, usize, &DrumPad) -> bool;
    let passes: [Matches; 3] = [
        |_, o, _, p| o.voice == p.voice && p.voice != PadVoice::Sample,
        |_, o, _, p| !p.name.is_empty() && o.name.eq_ignore_ascii_case(&p.name),
        |i, _, n, _| i == n,
    ];
    for matches in passes {
        for (n, pad) in new.pads.iter_mut().enumerate() {
            if done[n] {
                continue;
            }
            if let Some(i) = old.pads.iter().enumerate().position(|(i, o)| {
                o.output.is_some() && !taken.contains(&i) && matches(i, o, n, pad)
            }) {
                taken.push(i);
                pad.output = old.pads[i].output;
                done[n] = true;
            }
        }
    }
}

/// How many pads of `track`'s kit play through a chain of their own.
pub fn chain_count(project: &Project, track: TrackId) -> usize {
    project
        .track(track)
        .and_then(|t| t.instrument.pulsus())
        .map_or(0, |k| {
            k.pads
                .iter()
                .filter(|p| chain_of(project, p).is_some())
                .count()
        })
}

/// *Merge back* applied to `p` in place (a render's copy: a bounce of a split kit's track,
/// ADR 0100 §9): every pad heard on a strip of its own back on the kit, its chain kept.
/// Whether the kit was split.
pub fn merge_in_place(p: &mut Project, track: TrackId) -> bool {
    match merge(p, track) {
        Ok(cmd) => cmd.apply(p).is_ok(),
        Err(_) => false,
    }
}

/// In a render's copy `p`: a strip of the render's own that sums `track`'s kit — its own
/// output and every pad's chain — at unity (the 0 dB law, the track's level and balance
/// and their automation taken out: a bounce of the source keeps those on its new track),
/// before the kit's strip. Its id; `None` when `track` is not a kit.
pub fn kit_sum_strip(p: &mut Project, track: TrackId) -> Option<StripId> {
    let t = p.track(track)?;
    let kit = t.instrument.pulsus()?;
    let to = t.strip;
    let chains: Vec<StripId> = kit.pads.iter().filter_map(|x| chain_of(p, x)).collect();
    let mut sum = p.make_strip("kit");
    sum.pan_law = Some(PanLaw::ZeroDb);
    sum.output = to;
    let id = sum.id;
    p.strips.push(sum);
    for c in chains {
        if let Some(s) = p.strip_mut(c) {
            s.output = Some(id);
        }
    }
    let t = p.track_mut(track)?;
    t.strip = Some(id);
    t.gain_db = 0.0;
    t.pan = 0.0;
    use crate::automation::AutoTarget;
    for x in &mut p.tracks {
        x.clips.retain(|c| {
            !matches!(
                c.automation.as_ref().map(|a| &a.target),
                Some(AutoTarget::TrackGain(o) | AutoTarget::TrackPan(o)) if *o == track
            )
        });
    }
    Some(id)
}

#[path = "padout_chain.rs"]
mod chain;
pub use chain::{chain_add, chain_effects, chain_remove, may_reroute, with_kit_strips_following};

#[cfg(test)]
#[path = "padout_tests.rs"]
mod tests;

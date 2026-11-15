//! A pad's own mixer track, chosen per pad as in FL Studio (round 40, ADR 0117).
//!
//! FL's channel rack gives every channel a mixer-track number; Cantus's rack gives every
//! pad of a kit one too — the well left of its name. The model is round 37's
//! ([`DrumPad::output`], ADR 0100): `None` is the kit's own strip (the default, and what
//! every old song holds), a strip is the pad's own mixer track. *Split to strips* hands
//! every pad a new strip at once; the well picks **any** mixer track for **one** pad — a
//! free one, a bus, another channel's strip — and the pad leaves the kit's strip for it
//! (FL's way: its inserts and fader no longer touch that pad). A pad with a chain (ADR 0100
//! §6) keeps it: the chain plays into the track chosen.
//!
//! [`pad_track`] is what the well shows, [`set_pad_track`] the one command a choice makes
//! (one undo step, its inverse exact). The engine needs nothing new: a pad on a strip of its
//! own is a bus of its kit carried by a tap track (ADR 0100 §2), so an unset pad renders as
//! today to the bit.

use super::*;
use crate::pads::PadId;

/// The mixer track a pad of `t`'s kit is heard on, as the rack's well shows it: its own,
/// or the kit's (`t.strip`; `None` is the master).
pub fn pad_track(project: &Project, t: &Track, pad: &DrumPad) -> Option<StripId> {
    heard_on(project, t, pad).or(t.strip)
}

/// Whether a pad plays on a mixer track of its own rather than the kit's.
pub fn has_own_track(project: &Project, t: &Track, pad: &DrumPad) -> bool {
    heard_on(project, t, pad).is_some()
}

/// Whether `s` can be a pad's mixer track: an audio strip of the project — not a VCA (it
/// has no audio) nor a pad's chain (the desk does not show one).
pub fn can_carry_pad(project: &Project, s: StripId) -> bool {
    project
        .strip(s)
        .is_some_and(|x| !matches!(x.kind, StripKind::Vca | StripKind::Chain))
}

/// One command that sends pad `pad` of `track`'s kit to mixer track `to` — `None`, or the
/// kit's own strip, puts it back on the kit. A pad with a chain keeps it: the chain plays
/// into the track chosen. Refused when the track has no kit or the kit no such pad
/// ([`Refusal::NotAKit`], [`Refusal::NoPad`]), when `to` cannot carry a pad
/// ([`Refusal::Unfit`]: gone, a VCA, a chain, or a loop through the pad's chain), when the
/// pad is already there ([`Refusal::Nothing`]), and when the engine has no room for one more
/// output ([`Refusal::NoRoom`]). A free strip with no name takes the pad's. One undo step
/// (`cmd.set_pad_track`).
pub fn set_pad_track(
    project: &Project,
    track: TrackId,
    pad: PadId,
    to: Option<StripId>,
) -> Result<Command, Refusal> {
    let t = project.track(track).ok_or(Refusal::NotAKit)?;
    let kit = t.instrument.pulsus().ok_or(Refusal::NotAKit)?;
    let p = kit.by_id(pad).ok_or(Refusal::NoPad)?;
    // The kit's own strip named is the kit: `None`, as every pad on it is stored.
    let to = to.filter(|s| Some(*s) != t.strip);
    if let Some(s) = to {
        if !can_carry_pad(project, s) {
            return Err(Refusal::Unfit);
        }
    }
    if heard_on(project, t, p) == to {
        return Err(Refusal::Nothing);
    }
    let cmd = match chain_of(project, p) {
        Some(chain) => {
            // The chain outputs where the pad is heard: the track chosen, or the kit's.
            let out = to.or(t.strip);
            if project.routing_would_cycle(chain, out) {
                return Err(Refusal::Unfit);
            }
            Command::SetStripOutput {
                id: chain,
                output: out,
            }
        }
        None => Command::SetPadOutput {
            track,
            pad,
            output: to,
        },
    };
    let mut commands = vec![cmd];
    // A free strip, unnamed, takes the pad's name, as FL names the track it routes a
    // channel to: the desk then reads "Open hat", not the kit's name again.
    if let Some(s) = to {
        let unnamed = project.strip(s).is_some_and(|x| x.name.is_empty());
        if unnamed && !p.name.is_empty() && project.free_strips().any(|f| f == s) {
            commands.push(Command::SetStripName {
                id: s,
                name: p.name.clone(),
            });
        }
    }
    with_room(
        project,
        Command::Batch {
            label_key: "cmd.set_pad_track".into(),
            commands,
        },
    )
}

#[cfg(test)]
#[path = "padout_track_tests.rs"]
mod tests;

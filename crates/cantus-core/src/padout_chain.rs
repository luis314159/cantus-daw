//! A chain per pad (Pulsus III, ADR 0100 §6): the inserts on one pad of a kit.
//!
//! A pad's chain is a strip of kind [`StripKind::Chain`] that the pad plays into
//! (`DrumPad::output`) and that outputs where the pad is heard — the kit's strip, or the
//! pad's own strip when the kit is split. Being a strip, it is everything an insert chain
//! is already (native effects and hosted plug-ins, keys, compensation, freeze, automation)
//! with no second path; the desk does not show it, the pad does. A pad with no chain has no
//! strip: an empty chain is removed with its last insert, so a chain at rest costs nothing.

use super::*;
use crate::effect::EffectSlot;
use crate::model::EffectId;

/// The inserts of a pad's chain, in order (empty: it has none).
pub fn chain_effects<'a>(project: &'a Project, pad: &DrumPad) -> &'a [EffectSlot] {
    chain_of(project, pad)
        .and_then(|s| project.strip(s))
        .map(|s| s.effects.as_slice())
        .unwrap_or(&[])
}

/// One command that adds `effect` at the end of pad `pad`'s chain on `track`'s kit — the
/// chain made first when the pad has none (a strip of kind `Chain` named after the pad,
/// output where the pad is heard, the 0 dB law) and the pad moved onto it. `mint` makes a
/// strip with a fresh id. One undo step.
pub fn chain_add(
    project: &Project,
    track: TrackId,
    pad: crate::pads::PadId,
    effect: EffectSlot,
    mut mint: impl FnMut(&str) -> MixerStrip,
) -> Result<Command, Refusal> {
    let t = project.track(track).ok_or(Refusal::NotAKit)?;
    let kit = t.instrument.pulsus().ok_or(Refusal::NotAKit)?;
    let p = kit.by_id(pad).ok_or(Refusal::NoPad)?;
    let commands = match chain_of(project, p) {
        Some(chain) => vec![Command::InsertEffect {
            strip: chain,
            index: project.strip(chain).map_or(0, |s| s.effects.len()),
            slot: Box::new(effect),
        }],
        None => {
            let mut strip = mint(&p.name);
            strip.kind = StripKind::Chain;
            // Where the pad is heard now: its own strip, or the kit's.
            strip.output = p.output.or(t.strip);
            strip.pan_law = Some(PanLaw::ZeroDb);
            strip.dock = t
                .strip
                .and_then(|s| project.strip(s))
                .map(|s| s.dock)
                .unwrap_or_default();
            strip.effects.push(effect);
            let id = strip.id;
            vec![
                Command::InsertStrip {
                    index: project.strips.len(),
                    strip: Box::new(strip),
                },
                Command::SetPadOutput {
                    track,
                    pad,
                    output: Some(id),
                },
            ]
        }
    };
    with_room(
        project,
        Command::Batch {
            label_key: "cmd.pad_chain_add".into(),
            commands,
        },
    )
}

/// One command that takes `effect` out of pad `pad`'s chain; the last one takes the chain
/// with it (the pad back where the chain played it). One undo step.
pub fn chain_remove(
    project: &Project,
    track: TrackId,
    pad: crate::pads::PadId,
    effect: EffectId,
) -> Result<Command, Refusal> {
    let t = project.track(track).ok_or(Refusal::NotAKit)?;
    let kit = t.instrument.pulsus().ok_or(Refusal::NotAKit)?;
    let p = kit.by_id(pad).ok_or(Refusal::NoPad)?;
    let chain = chain_of(project, p).ok_or(Refusal::Nothing)?;
    let st = project.strip(chain).ok_or(Refusal::Nothing)?;
    if st.effect(effect).is_none() {
        return Err(Refusal::Nothing);
    }
    let commands = if st.effects.len() == 1 {
        // Back where the chain played it: its own strip, or the kit's (`None`).
        let back = st.output.filter(|o| Some(*o) != t.strip);
        vec![
            Command::SetPadOutput {
                track,
                pad,
                output: back,
            },
            Command::RemoveStrip { id: chain },
        ]
    } else {
        vec![Command::RemoveEffect {
            strip: chain,
            effect,
        }]
    };
    Ok(Command::Batch {
        label_key: "cmd.pad_chain_remove".into(),
        commands,
    })
}

/// Whether `cmd` routes a track to another strip, alone or in a batch. Cheap: the bus asks
/// it of every command.
pub fn may_reroute(cmd: &Command) -> bool {
    match cmd {
        Command::SetTrackStrip { .. } => true,
        Command::Batch { commands, .. } => commands.iter().any(may_reroute),
        _ => false,
    }
}

/// `cmd`, and in the same undo step what keeps a kit's pads with the kit when `cmd` routes
/// the kit's track to another strip (review of round 37, item 1): every strip made for the
/// kit's pads — a pad's chain, a pad's own strip from *Split to strips* — that played into
/// the kit's old strip plays into its new one. Without it a chained pad stayed on the old
/// strip through a hidden chain (and that strip looked free to the next new track). `cmd`
/// as it is when it moves no kit, or when it does not apply; a re-pointing that would not
/// apply (a loop) is left out.
pub fn with_kit_strips_following(project: &Project, cmd: Command) -> Command {
    let mut after = project.clone();
    if cmd.clone().apply(&mut after).is_err() {
        return cmd;
    }
    let mut extra: Vec<Command> = Vec::new();
    for t in &project.tracks {
        let Some(now) = after.track(t.id) else {
            continue;
        };
        if now.strip == t.strip {
            continue;
        }
        let Some(kit) = now.instrument.pulsus() else {
            continue;
        };
        let to = now.strip;
        // The kit's own strips, as `cmd` leaves them: its pads' chains and own strips, and
        // the strips its chains play into that were made for pads.
        let mut mine: Vec<StripId> = Vec::new();
        for p in &kit.pads {
            let Some(o) = p.output else { continue };
            let Some(s) = after.strip(o) else { continue };
            if pad_made(s) && !mine.contains(&o) {
                mine.push(o);
            }
            if s.kind == StripKind::Chain {
                if let Some(po) = s.output {
                    if after.strip(po).is_some_and(|x| x.kind == StripKind::Pad)
                        && !mine.contains(&po)
                    {
                        mine.push(po);
                    }
                }
            }
        }
        for s in mine {
            if Some(s) == to || after.strip(s).is_none_or(|x| x.output != t.strip) {
                continue;
            }
            let c = Command::SetStripOutput { id: s, output: to };
            if c.clone().apply(&mut after).is_ok() {
                extra.push(c);
            }
        }
    }
    if extra.is_empty() {
        return cmd;
    }
    let label = cmd.label_key().to_string();
    let mut commands = vec![cmd];
    commands.extend(extra);
    Command::Batch {
        label_key: label,
        commands,
    }
}

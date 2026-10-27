//! What the undo history panel calls each step (round 31, ADR 0072): the step's `cmd.*`
//! label, capitalised, and — where it fits — what it touched: *Move 3 notes*, *Insert
//! Concentus on Lead*, *Delete track Bass*, *Insert track template Pad*. Pure: a step and
//! the project as it is now, so names read as they read today (a strip renamed since says
//! its new name). Anything without a sentence of its own is its label.

use crate::i18n::{t, tf, tf_count};
use crate::picker::PickChoice;
use cantus_core::{Command, EffectKind, EffectSlot, Project, StripId};

/// A label capitalised for the list ("move notes" → "Move notes").
pub fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// An insert's name: a native kind's, or its hosted plug-in's.
pub fn effect_name(slot: &EffectSlot) -> String {
    if let Some(x) = &slot.spec.external {
        return x.name.clone();
    }
    kind_name(slot.spec.kind)
}

fn kind_name(kind: EffectKind) -> String {
    crate::picker_catalog::slug_of(&PickChoice::Effect(kind))
        .map(|s| t(&format!("picker.dev.{s}.name")))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn strip_word(project: &Project, strip: StripId) -> String {
    let name = crate::routing_actions::strip_name(project, strip);
    if name.is_empty() {
        t("workflow.history.gone")
    } else {
        name
    }
}

/// How many notes a step's command adds, removes or edits (a batch's, summed).
fn notes_in(c: &Command) -> usize {
    match c {
        Command::InsertNotes { notes, .. } | Command::SetNotes { notes, .. } => notes.len(),
        Command::RemoveNotes { ids, .. } => ids.len(),
        Command::Batch { commands, .. } => commands.iter().map(notes_in).sum(),
        _ => 0,
    }
}

/// The sentence a notes label takes with its count.
fn notes_key(label: &str) -> Option<&'static str> {
    Some(match label {
        "cmd.insert_notes" => "workflow.history.notes.add",
        "cmd.remove_notes" | "cmd.erase" => "workflow.history.notes.delete",
        "cmd.set_notes" => "workflow.history.notes.edit",
        "cmd.nudge" => "workflow.history.notes.move",
        "cmd.transpose" => "workflow.history.notes.transpose",
        "cmd.duplicate" => "workflow.history.notes.duplicate",
        "cmd.paste" => "workflow.history.notes.paste",
        "cmd.paint" => "workflow.history.notes.paint",
        "cmd.quantize" => "workflow.history.notes.quantize",
        "cmd.mute_notes" => "workflow.history.notes.mute",
        "cmd.slice" => "workflow.history.notes.slice",
        _ => return None,
    })
}

/// The first command of a kind inside a step (itself, or in its batch).
fn first<'a, T>(c: &'a Command, f: &impl Fn(&'a Command) -> Option<T>) -> Option<T> {
    if let Some(x) = f(c) {
        return Some(x);
    }
    match c {
        Command::Batch { commands, .. } => commands.iter().find_map(|c| first(c, f)),
        _ => None,
    }
}

/// What the panel calls a step: its label, with what it touched where that fits.
/// `command` is the step's, `inverse` what undoes it (it holds what a removal took away).
pub fn describe(label: &str, command: &Command, inverse: &Command, project: &Project) -> String {
    let plain = capitalised(&t(label));
    if let Some(key) = notes_key(label) {
        let n = notes_in(command);
        if n > 0 {
            return tf_count(key, n, &[]);
        }
        return plain;
    }
    match label {
        "cmd.insert_effect" => first(command, &|c| match c {
            Command::InsertEffect { strip, slot, .. } => Some(tf(
                "workflow.history.insert_effect",
                &[
                    ("effect", &effect_name(slot)),
                    ("strip", &strip_word(project, *strip)),
                ],
            )),
            _ => None,
        }),
        "cmd.remove_effect" => first(inverse, &|c| match c {
            Command::InsertEffect { strip, slot, .. } => Some(tf(
                "workflow.history.remove_effect",
                &[
                    ("effect", &effect_name(slot)),
                    ("strip", &strip_word(project, *strip)),
                ],
            )),
            _ => None,
        }),
        "cmd.insert_track" => first(command, &|c| match c {
            Command::InsertTrack { track, .. } => Some(tf(
                "workflow.history.insert_track",
                &[("name", &track.name)],
            )),
            _ => None,
        }),
        "cmd.remove_track" => first(inverse, &|c| match c {
            Command::InsertTrack { track, .. } => Some(tf(
                "workflow.history.remove_track",
                &[("name", &track.name)],
            )),
            _ => None,
        }),
        "cmd.set_track_name" => first(command, &|c| match c {
            Command::SetTrackName { name, .. } => {
                Some(tf("workflow.history.rename_track", &[("name", name)]))
            }
            _ => None,
        }),
        "cmd.insert_track_template" => first(command, &|c| match c {
            Command::InsertTrack { track, .. } => Some(tf(
                "workflow.history.insert_track_template",
                &[("name", &track.name)],
            )),
            _ => None,
        }),
        _ => None,
    }
    .unwrap_or(plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::{Origin, Ticks};

    #[test]
    fn a_step_says_what_it_touched_where_that_fits() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let mut p = crate::starter_project();
        let track = p.tracks[0].id;
        let strip = p.tracks[0].strip.unwrap();
        let mut clip = p.make_clip("C", Ticks::ZERO, Ticks::beats(4));
        let notes: Vec<_> = (0..3)
            .map(|i| p.make_note(Ticks::beats(i), Ticks::QUARTER, 60, 100))
            .collect();
        clip.notes = notes.clone();
        let cid = clip.id;
        p.tracks[0].clips.push(clip);
        let slot = p.make_effect(EffectKind::Compressor);
        let mut bus = cantus_core::CommandBus::new(p);
        bus.execute(
            Command::Batch {
                label_key: "cmd.nudge".into(),
                commands: vec![Command::SetNotes {
                    track,
                    clip: cid,
                    notes,
                }],
            },
            Origin::User,
        )
        .unwrap();
        bus.execute(
            Command::InsertEffect {
                strip,
                index: 0,
                slot: Box::new(slot),
            },
            Origin::User,
        )
        .unwrap();
        let h = bus.history();
        let say = |i: usize| describe(&h[i].label_key, &h[i].command, &h[i].inverse, bus.project());
        assert_eq!(say(0), "Move 3 notes");
        let name = crate::routing_actions::strip_name(bus.project(), strip);
        assert_eq!(say(1), format!("Insert Comprimo on {name}"));
        assert_eq!(capitalised("rename project"), "Rename project");
    }
}

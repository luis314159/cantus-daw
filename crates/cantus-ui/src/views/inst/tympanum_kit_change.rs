//! Another kit for a Tympanum (review of 40a27bb7, finding 1): every place that changes the
//! kit — a modelled kit's row, a recorded kit's row, *Install from a file…* — goes through
//! [`change_kit`], one undo step:
//!
//! 1. Both kits' definitions are read first (a pack's kit loads lazily off the audio
//!    thread; `cantus_engine::inst::tympanum::cache::recorded` reads and keeps it now), so
//!    `cantus_core::inst::tympanum::cmd::change_kit` carries every route and level by name,
//!    then by family, and never drops them for a kit that is merely not read yet.
//! 2. The strips follow the new kit: a strip made for a mic and named after it takes the
//!    new mic's name; a strip made for a mic the new kit has no place for, left with
//!    nothing, is removed; a kit that was *Each mic to its own strip* gives the new kit's
//!    other mics strips of their own too; the kit's sub-groups are made again by family.

use super::strips;
use crate::app::CantusApp;
use cantus_core::inst::tympanum::{cmd, KitRef};
use cantus_core::inst::tympanum_kit as kitdef;
use cantus_core::inst_out::{self, OutputPlan};
use cantus_core::{padout, Command, Project, StripId, TrackId};

/// Commands applied in turn to a copy of the song, so each next one is planned on what the
/// ones before it leave (indices, orphans, groups).
pub(super) struct Steps {
    pub view: Project,
    pub cmds: Vec<Command>,
}

impl Steps {
    pub fn new(view: Project) -> Steps {
        Steps {
            view,
            cmds: Vec::new(),
        }
    }

    /// Apply `c` to the copy; kept only when it applies.
    pub fn push(&mut self, c: Command) -> bool {
        let mut v = self.view.clone();
        if c.clone().apply(&mut v).is_ok() {
            self.view = v;
            self.cmds.push(c);
            true
        } else {
            false
        }
    }
}

/// Read `kit`'s definition now if it is a pack's not read yet (the engine's loader, which
/// keeps it for the session; off the audio thread).
pub fn know(kit: &KitRef) {
    if let KitRef::Pack { .. } = kit {
        if !kitdef::is_recorded(&kit.text()) {
            let _ = cantus_engine::inst::tympanum::cache::recorded(kit);
        }
    }
}

/// Whether every one of the kit's outputs plays on a strip of its own (what *Each mic to
/// its own strip* leaves).
fn each_own(p: &Project, track: TrackId) -> bool {
    let outs = inst_out::definition(p, track);
    let mut seen: Vec<StripId> = Vec::new();
    !outs.is_empty()
        && outs.iter().all(|o| match o.strip {
            Some(s) if !seen.contains(&s) => {
                seen.push(s);
                true
            }
            _ => false,
        })
}

/// Give `track`'s Tympanum the kit `to`: one command, one undo step. `false` when nothing
/// changed (already that kit, not a Tympanum, or the bus refused it — it says why).
pub fn change_kit(app: &mut CantusApp, track: TrackId, to: &KitRef) -> bool {
    match plan(app, track, to) {
        Some(c) => app.exec(c),
        None => false,
    }
}

/// [`change_kit`]'s command, not run (a preset naming a kit sets the rest on top of it).
/// `None`: already that kit, or not a Tympanum.
pub fn plan(app: &mut CantusApp, track: TrackId, to: &KitRef) -> Option<Command> {
    let p0 = app.project().clone();
    let n = p0.track(track).and_then(|t| t.instrument.native())?;
    let from = KitRef::of(n);
    know(&from);
    know(to);
    let change = cmd::change_kit(&p0, track, to)?;
    let was_each = each_own(&p0, track);
    let olds = strips::kit_groups(&p0, track);
    let mut st = Steps::new(p0.clone());
    for (_, members) in &olds {
        for m in members {
            st.push(Command::SetStripGroup {
                id: *m,
                group: None,
            });
        }
    }
    if !st.push(change.clone()) {
        // The bus says why when it runs.
        return Some(change);
    }
    // A strip made for one mic, named after it, takes the new mic's name.
    let old_slots = n.outputs.clone();
    let new_slots = st
        .view
        .track(track)
        .and_then(|t| t.instrument.native())
        .map(|n| n.outputs.clone())
        .unwrap_or_default();
    for s in &new_slots {
        let Some(strip) = st.view.strip(s.strip) else {
            continue;
        };
        let alone = new_slots.iter().filter(|x| x.strip == s.strip).count() == 1;
        let was = old_slots.iter().find(|x| x.strip == s.strip);
        // Named after the old mic as the song keeps it or as the interface said it; it
        // takes the new mic's name as the interface says it (a modelled kit's, translated).
        let new_name = super::def::mic_said(to, &s.name);
        if padout::pad_made(strip)
            && alone
            && was.is_some_and(|w| {
                strip.name == w.name || strip.name == super::def::mic_said(&from, &w.name)
            })
            && strip.name != new_name
        {
            let id = strip.id;
            st.push(Command::SetStripName { id, name: new_name });
        }
    }
    // A strip left with nothing goes.
    let before = padout::orphans(&p0);
    for id in padout::orphans(&st.view) {
        if !before.contains(&id) {
            st.push(Command::RemoveStrip { id });
        }
    }
    for (g, _) in &olds {
        st.push(Command::RemoveStripGroup { id: *g });
    }
    if was_each {
        let view = st.view.clone();
        if let Ok(c) = inst_out::plan_with(
            &view,
            track,
            OutputPlan::EachOwn,
            &mut |name| app.bus.new_strip(name),
            &|n: &str| super::def::mic_said(to, n),
        ) {
            st.push(c);
        }
    }
    if was_each || !olds.is_empty() {
        let view = st.view.clone();
        for c in strips::regroup(app, &view, track) {
            st.push(c);
        }
    }
    Some(Command::Batch {
        label_key: "cmd.tympanum_kit".into(),
        commands: st.cmds,
    })
}

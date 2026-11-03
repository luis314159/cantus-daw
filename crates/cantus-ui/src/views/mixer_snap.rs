//! The desk's snapshots and VCAs in its menus (round 36, ADR 0093): the title row's
//! *Snapshots* menu (take, recall all or the selected channels, update, A/B, cue, rename,
//! remove), a strip's *VCA* submenu (follow, release, a new VCA from the selection), the
//! two safe switches, a group's VCA. A child of `mixer_menu`: it builds `Entry` rows and
//! runs `Act`s of the parent's own kinds.

use super::*;
use cantus_core::mix_cmd::MixCommand;
use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
use cantus_core::vca::StripKind;

/// What a row of the snapshot and VCA menus does.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum MixAct {
    /// Take a snapshot of the whole desk (0) or of the selected strips (1).
    Take(u8),
    /// Recall: all channels (0), the selected ones (1), everything with the safe ones (2).
    Recall(MixSnapId, u8),
    /// Take the desk's state again into the snapshot.
    Update(MixSnapId),
    /// Swap the desk with the snapshot, and back (A/B).
    Compare(MixSnapId),
    Rename(MixSnapId),
    Remove(MixSnapId),
    /// A cue for the snapshot at the playhead.
    Cue(MixSnapId),
    RemoveCue(MixCueId),
    /// A new VCA after the last strip, taking the strips of the selection (the strip's, or
    /// none from the desk's menu).
    NewVca(Option<StripId>),
    /// The strips (the selection, when the strip is in it) follow, or stop following, a VCA.
    Follow(StripId, StripId),
    GroupVca(GroupId, Option<StripId>),
    RecallSafe(StripId),
    SoloSafe(StripId),
}

/// The snapshot the A/B compare holds the desk for, while nothing changed since.
fn ab_live(v: &MixerView, app: &CantusApp) -> Option<MixSnapId> {
    v.snap_ab
        .as_ref()
        .filter(|(_, _, rev)| *rev == app.bus.revision())
        .map(|(id, _, _)| *id)
}

/// The *Snapshots* menu.
pub(super) fn entries(v: &MixerView, app: &CantusApp, e: &mut Vec<Entry>) {
    let p = app.project();
    let selection = app.strip_selection();
    e.push(Entry::act(
        Label::Key("mix.snap.take"),
        Act::Mix(MixAct::Take(0)),
    ));
    e.push(
        Entry::act(
            Label::Key("mix.snap.take_selected"),
            Act::Mix(MixAct::Take(1)),
        )
        .enabled(!selection.is_empty()),
    );
    if !p.mix.snapshots.is_empty() {
        e.push(Entry::band("mix.snap.band"));
        let now = capture(p, &[], true);
        for s in &p.mix.snapshots {
            let same = s.state == now;
            let held = ab_live(v, app) == Some(s.id);
            e.push(
                Entry::sub(Label::Own(s.name.clone()), Sub::Snap(s.id)).check(if held {
                    Check::On
                } else {
                    Check::Radio(same)
                }),
            );
        }
    }
    if !p.mix.cues.is_empty() {
        e.push(Entry::band("mix.cue.band"));
        for c in &p.mix.cues {
            let name = p.mix.snapshot(c.snapshot).map_or("", |s| s.name.as_str());
            let (bar, beat) = bar_beat(p, c.at);
            e.push(Entry::act(
                Label::Own(crate::i18n::tf(
                    "mix.cue.remove",
                    &[("bar", &bar), ("beat", &beat), ("name", &name)],
                )),
                Act::Mix(MixAct::RemoveCue(c.id)),
            ));
        }
    }
}

/// Bar and beat (from 1) of a tick, in the project's metre.
fn bar_beat(p: &cantus_core::Project, at: cantus_core::Ticks) -> (i64, i64) {
    let beats_per_bar = i64::from(p.time_signature.numerator.max(1));
    let beats = at.0.div_euclid(cantus_core::PPQ);
    (
        beats.div_euclid(beats_per_bar) + 1,
        beats.rem_euclid(beats_per_bar) + 1,
    )
}

/// One snapshot's submenu.
pub(super) fn snap_entries(v: &MixerView, app: &CantusApp, id: MixSnapId, e: &mut Vec<Entry>) {
    let selection = app.strip_selection();
    let cued = app.project().mix.cues.iter().any(|c| c.snapshot == id);
    let held = ab_live(v, app) == Some(id);
    e.push(Entry::act(
        Label::Key("mix.snap.recall"),
        Act::Mix(MixAct::Recall(id, 0)),
    ));
    e.push(
        Entry::act(
            Label::Key("mix.snap.recall_selected"),
            Act::Mix(MixAct::Recall(id, 1)),
        )
        .enabled(!selection.is_empty()),
    );
    e.push(Entry::act(
        Label::Key("mix.snap.recall_all"),
        Act::Mix(MixAct::Recall(id, 2)),
    ));
    e.push(
        Entry::act(
            Label::Key("mix.snap.compare"),
            Act::Mix(MixAct::Compare(id)),
        )
        .check(on(held)),
    );
    e.push(Entry::separator());
    e.push(Entry::act(
        Label::Key("mix.snap.update"),
        Act::Mix(MixAct::Update(id)),
    ));
    e.push(Entry::act(
        Label::Key("mix.snap.rename"),
        Act::Mix(MixAct::Rename(id)),
    ));
    e.push(Entry::act(
        Label::Key("mix.snap.cue"),
        Act::Mix(MixAct::Cue(id)),
    ));
    e.push(Entry::act(Label::Key("mix.snap.remove"), Act::Mix(MixAct::Remove(id))).enabled(!cued));
}

/// A strip's *VCA* submenu: every VCA, checked where the strip follows it.
pub(super) fn vca_entries(app: &CantusApp, id: StripId, e: &mut Vec<Entry>) {
    let p = app.project();
    let follows = p.strip(id).map(|s| s.vcas.clone()).unwrap_or_default();
    for (i, s) in p.strips.iter().enumerate() {
        if s.kind != StripKind::Vca || s.id == id {
            continue;
        }
        let name = if s.name.is_empty() {
            crate::i18n::tf("mix.vca.unnamed", &[("number", &(i + 1))])
        } else {
            s.name.clone()
        };
        // A VCA that would follow itself round a loop is greyed.
        let loops = p.is_vca(id) && p.vca_governing(s.id).contains(&id);
        e.push(
            Entry::act(Label::Own(name), Act::Mix(MixAct::Follow(id, s.id)))
                .check(on(follows.contains(&s.id)))
                .enabled(!loops),
        );
    }
    if e.is_empty() {
        e.push(Entry::band("mix.vca.none"));
    }
    e.push(Entry::separator());
    e.push(Entry::act(
        Label::Key("mix.vca.new_from"),
        Act::Mix(MixAct::NewVca(Some(id))),
    ));
}

/// A group's VCA: none, or one of the VCAs.
pub(super) fn group_entries(app: &CantusApp, g: GroupId, e: &mut Vec<Entry>) {
    let p = app.project();
    let now = p.mix.group_vca(g);
    e.push(
        Entry::act(
            Label::Key("mix.vca.group_none"),
            Act::Mix(MixAct::GroupVca(g, None)),
        )
        .check(radio(now.is_none())),
    );
    for (i, s) in p.strips.iter().enumerate() {
        if s.kind != StripKind::Vca {
            continue;
        }
        let name = if s.name.is_empty() {
            crate::i18n::tf("mix.vca.unnamed", &[("number", &(i + 1))])
        } else {
            s.name.clone()
        };
        e.push(
            Entry::act(Label::Own(name), Act::Mix(MixAct::GroupVca(g, Some(s.id))))
                .check(radio(now == Some(s.id))),
        );
    }
}

/// The two safe switches and the VCA entry of a strip's menu.
pub(super) fn strip_rows(app: &CantusApp, id: StripId, e: &mut Vec<Entry>) {
    let Some(s) = app.project().strip(id) else {
        return;
    };
    e.push(Entry::band("mix.menu.band"));
    if s.kind == StripKind::Audio || s.kind == StripKind::Vca {
        e.push(Entry::sub(Label::Key("mix.menu.vca"), Sub::Vca(id)));
    }
    e.push(
        Entry::act(
            Label::Key("mix.menu.recall_safe"),
            Act::Mix(MixAct::RecallSafe(id)),
        )
        .check(on(s.recall_safe)),
    );
    e.push(
        Entry::act(
            Label::Key("mix.menu.solo_safe"),
            Act::Mix(MixAct::SoloSafe(id)),
        )
        .check(on(s.solo_safe)),
    );
}

/// Run one.
pub(super) fn apply(v: &mut MixerView, app: &mut CantusApp, act: MixAct) {
    match act {
        MixAct::Take(scope) => {
            let scope = if scope == 1 {
                app.strip_selection()
            } else {
                Vec::new()
            };
            let p = app.project();
            let state = capture(p, &scope, scope.is_empty());
            let n = p.mix.snapshots.len();
            let id = MixSnapId(app.bus.alloc_id());
            let name = crate::i18n::tf("mix.snap.default_name", &[("number", &(n + 1))]);
            app.exec(Command::Mix(MixCommand::InsertSnapshot {
                index: n,
                snapshot: Box::new(MixSnapshot { id, name, state }),
            }));
        }
        MixAct::Recall(id, how) => {
            let (scope, safe) = match how {
                1 => (app.strip_selection(), false),
                2 => (Vec::new(), true),
                _ => (Vec::new(), false),
            };
            v.snap_ab = None;
            app.exec(Command::Mix(MixCommand::Recall {
                id,
                scope,
                master: how != 1,
                include_safe: safe,
            }));
        }
        MixAct::Update(id) => {
            // Its own channels again, and the master only if it had it (review 1, item 8).
            let Some(old) = app.project().mix.snapshot(id).map(|s| s.state.clone()) else {
                return;
            };
            let state = cantus_core::mix_snap::recapture(app.project(), &old);
            app.exec(Command::Mix(MixCommand::UpdateSnapshot {
                id,
                state: Box::new(state),
            }));
        }
        MixAct::Compare(id) => {
            // A held desk counts only while nothing has changed since its recall (an undo
            // of the recall, an edit: it is dropped — review 1, item 11).
            let live = ab_live(v, app);
            let held = v.snap_ab.take().filter(|_| live.is_some());
            match held {
                // B back to A: what the desk was before the first press.
                Some((h, a, _)) if h == id => {
                    app.exec(Command::Mix(MixCommand::Apply { state: a }));
                }
                // Another snapshot as B: the same A is kept.
                other => {
                    let a = other
                        .map(|(_, a, _)| a)
                        .unwrap_or_else(|| Box::new(capture(app.project(), &[], true)));
                    if app.exec(Command::Mix(MixCommand::Recall {
                        id,
                        scope: Vec::new(),
                        master: true,
                        include_safe: true,
                    })) {
                        v.snap_ab = Some((id, a, app.bus.revision()));
                    }
                }
            }
        }
        MixAct::Rename(id) => {
            if let Some(s) = app.project().mix.snapshot(id) {
                v.rename = Some((super::super::Renaming::Snapshot(id), s.name.clone()));
            }
        }
        MixAct::Remove(id) => {
            app.exec(Command::Mix(MixCommand::RemoveSnapshot { id }));
        }
        MixAct::Cue(id) => {
            let at = app.engine.position();
            let cue = MixCue {
                id: MixCueId(app.bus.alloc_id()),
                at,
                snapshot: id,
                scope: Vec::new(),
            };
            let index = app.project().mix.cues.len();
            app.exec(Command::Mix(MixCommand::InsertCue { index, cue }));
        }
        MixAct::RemoveCue(id) => {
            app.exec(Command::Mix(MixCommand::RemoveCue { id }));
        }
        MixAct::NewVca(from) => {
            let p = app.project();
            let members: Vec<StripId> = match from {
                Some(s) if in_selection(app, s) => app.strip_selection(),
                Some(s) => vec![s],
                None => Vec::new(),
            };
            let members: Vec<StripId> = members
                .into_iter()
                .filter(|m| p.strip(*m).is_some())
                .collect();
            let n = p.strips.iter().filter(|s| s.kind == StripKind::Vca).count();
            let index = p.strips.len();
            let mut s = app.bus.new_strip(crate::i18n::tf(
                "mix.vca.default_name",
                &[("number", &(n + 1))],
            ));
            s.kind = StripKind::Vca;
            let vca = s.id;
            let mut cmds = vec![Command::InsertStrip {
                index,
                strip: Box::new(s),
            }];
            for m in members {
                let mut list = app
                    .project()
                    .strip(m)
                    .map(|x| x.vcas.clone())
                    .unwrap_or_default();
                list.push(vca);
                cmds.push(Command::Mix(MixCommand::SetStripVcas { id: m, vcas: list }));
            }
            let cmd = if cmds.len() == 1 {
                cmds.pop().expect("one")
            } else {
                Command::Batch {
                    label_key: "cmd.mix.create_vca".into(),
                    commands: cmds,
                }
            };
            app.exec(cmd);
            app.selected_strip = Some(vca);
        }
        MixAct::Follow(strip, vca) => {
            let p = app.project();
            let targets: Vec<StripId> = if in_selection(app, strip) {
                app.strip_selection()
            } else {
                vec![strip]
            };
            // One switch for all: on unless the clicked strip already follows.
            let on = !p.strip(strip).is_some_and(|s| s.vcas.contains(&vca));
            let cmds: Vec<Command> = targets
                .into_iter()
                .filter(|t| *t != vca && p.strip(*t).is_some())
                .map(|t| {
                    let mut list = p.strip(t).map(|x| x.vcas.clone()).unwrap_or_default();
                    list.retain(|x| *x != vca);
                    if on {
                        list.push(vca);
                    }
                    Command::Mix(MixCommand::SetStripVcas { id: t, vcas: list })
                })
                .collect();
            let cmd = if cmds.len() == 1 {
                cmds.into_iter().next().expect("one")
            } else if cmds.is_empty() {
                return;
            } else {
                Command::Batch {
                    label_key: "cmd.mix.set_vcas".into(),
                    commands: cmds,
                }
            };
            app.exec(cmd);
        }
        MixAct::GroupVca(id, vca) => {
            app.exec(Command::Mix(MixCommand::SetGroupVca { id, vca }));
        }
        MixAct::RecallSafe(id) => {
            let safe = !app.project().strip(id).is_some_and(|s| s.recall_safe);
            app.exec(Command::Mix(MixCommand::SetRecallSafe { id, safe }));
        }
        MixAct::SoloSafe(id) => {
            let safe = !app.project().strip(id).is_some_and(|s| s.solo_safe);
            app.exec(Command::Mix(MixCommand::SetSoloSafe { id, safe }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_engine::EngineHandle;

    fn rig() -> (MixerView, CantusApp) {
        let mut p = crate::starter_project();
        p.strips[0].gain_db = -6.0;
        let app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
        (MixerView::default(), app)
    }

    /// Review 1, item 8: *Update from the desk* keeps a partial snapshot partial and
    /// without the master. Failed before: it took the whole desk and the master.
    #[test]
    fn an_update_keeps_a_partial_snapshot_partial() {
        let (mut v, mut app) = rig();
        app.selected_strip = Some(app.project().strips[0].id);
        apply(&mut v, &mut app, MixAct::Take(1));
        let id = app.project().mix.snapshots[0].id;
        assert_eq!(app.project().mix.snapshots[0].state.strips.len(), 1);
        assert!(app.project().mix.snapshots[0].state.master.is_none());
        apply(&mut v, &mut app, MixAct::Update(id));
        let s = &app.project().mix.snapshots[0].state;
        assert_eq!(s.strips.len(), 1);
        assert!(s.master.is_none());
    }

    /// Review 1, item 11: an A/B whose recall was undone is dropped — the next press puts the
    /// snapshot on the desk again instead of "going back" to a desk that is already there.
    #[test]
    fn an_undone_ab_starts_again() {
        let (mut v, mut app) = rig();
        apply(&mut v, &mut app, MixAct::Take(0));
        let id = app.project().mix.snapshots[0].id;
        let s0 = app.project().strips[0].id;
        app.exec(Command::SetStripGain {
            id: s0,
            gain_db: 3.0,
        });
        app.end_gesture();
        apply(&mut v, &mut app, MixAct::Compare(id));
        assert_eq!(
            app.project().strip(s0).unwrap().gain_db,
            -6.0,
            "B: the snapshot"
        );
        app.undo();
        assert_eq!(
            app.project().strip(s0).unwrap().gain_db,
            3.0,
            "undone: the desk"
        );
        apply(&mut v, &mut app, MixAct::Compare(id));
        assert_eq!(
            app.project().strip(s0).unwrap().gain_db,
            -6.0,
            "B again, not a stale A"
        );
        apply(&mut v, &mut app, MixAct::Compare(id));
        assert_eq!(app.project().strip(s0).unwrap().gain_db, 3.0, "back to A");
    }
}

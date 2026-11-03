//! Mixer snapshots (ADR 0093): what a snapshot holds, a recall exact and undone exactly,
//! scope, the safe set, cues compiled into automation, and what the freeze guard sees.

use super::*;
use crate::command::Command;
use crate::effect::{EffectKind, EffectSlot, EffectSpec};
use crate::mix_cmd::MixCommand;
use crate::model::Project;

fn with_effect(p: &mut Project, strip: usize, kind: EffectKind) -> EffectId {
    let id = EffectId(p.alloc_id());
    p.strips[strip].effects.push(EffectSlot {
        id,
        bypass: false,
        spec: EffectSpec::new(kind),
        mix: 1.0,
        sidechain: None,
        midi_from: None,
    });
    id
}

/// A desk with some character: levels, a send, an insert with a changed parameter.
fn busy() -> (Project, EffectId) {
    let mut p = Project::default();
    p.strips[0].gain_db = -4.5;
    p.strips[0].pan = 0.3;
    p.strips[1].muted = true;
    p.strips[2].width = 1.6;
    let to = p.strips[5].id;
    p.strips[0].sends.push(crate::model::Send {
        to,
        level: 0.4,
        enabled: true,
        tap: crate::model::SendTap::Post,
    });
    let fx = with_effect(&mut p, 0, EffectKind::Compressor);
    let key = EffectKind::Compressor.params()[0].key;
    let info = EffectKind::Compressor.params()[0];
    p.strips[0].effects[0]
        .spec
        .set(key, (info.min + info.max) * 0.5);
    p.master.gain_db = -1.5;
    (p, fx)
}

fn take(p: &Project, id: u64) -> MixSnapshot {
    MixSnapshot {
        id: MixSnapId(id),
        name: format!("snap {id}"),
        state: capture(p, &[], true),
    }
}

fn add(p: &mut Project, id: u64) -> Command {
    let snapshot = Box::new(take(p, id));
    run(p, MixCommand::InsertSnapshot { index: 0, snapshot })
}

fn run(p: &mut Project, c: MixCommand) -> Command {
    Command::Mix(c).apply(p).unwrap()
}

/// Everything a recall may touch is state; structure (ids, routing) must be the same.
fn structure(p: &Project) -> Vec<(StripId, usize, usize)> {
    p.strips
        .iter()
        .map(|s| (s.id, s.effects.len(), s.sends.len()))
        .collect()
}

#[test]
fn a_recall_restores_the_state_exactly_and_undoes_exactly() {
    let (mut p, fx) = busy();
    let snap = take(&p, 1);
    let at_snapshot = p.clone();
    run(
        &mut p,
        MixCommand::InsertSnapshot {
            index: 0,
            snapshot: Box::new(snap),
        },
    );
    // Wreck the desk.
    p.strips[0].gain_db = 5.0;
    p.strips[0].pan = -1.0;
    p.strips[1].muted = false;
    p.strips[2].width = 0.0;
    p.strips[0].sends[0].level = 1.0;
    p.strips[0].sends[0].enabled = false;
    p.strips[0].effects[0].bypass = true;
    p.strips[0].effects[0].mix = 0.2;
    p.strips[0].effects[0].spec.set(
        EffectKind::Compressor.params()[0].key,
        EffectKind::Compressor.params()[0].max,
    );
    p.strips[3].soloed = true;
    p.master.gain_db = 4.0;
    let wrecked = p.clone();
    let inv = run(
        &mut p,
        MixCommand::Recall {
            id: MixSnapId(1),
            scope: vec![],
            master: true,
            include_safe: false,
        },
    );
    assert_eq!(p.strips, at_snapshot.strips, "the desk is as it was taken");
    assert_eq!(p.master, at_snapshot.master);
    assert!(p.strips[0].effect(fx).is_some());
    inv.apply(&mut p).unwrap();
    assert_eq!(p, wrecked, "undo puts back exactly what the recall changed");
}

#[test]
fn a_recall_never_creates_removes_or_reroutes() {
    let (mut p, _) = busy();
    let snap = take(&p, 1);
    run(
        &mut p,
        MixCommand::InsertSnapshot {
            index: 0,
            snapshot: Box::new(snap),
        },
    );
    // An insert and a send made since stay; the recall does not touch them.
    let late = with_effect(&mut p, 1, EffectKind::Delay);
    let to = p.strips[6].id;
    p.strips[1].sends.push(crate::model::Send {
        to,
        level: 0.9,
        enabled: true,
        tap: crate::model::SendTap::Pre,
    });
    p.strips[0].output = Some(p.strips[7].id);
    let shape = structure(&p);
    let out = p.strips[0].output;
    run(
        &mut p,
        MixCommand::Recall {
            id: MixSnapId(1),
            scope: vec![],
            master: false,
            include_safe: false,
        },
    );
    assert_eq!(structure(&p), shape);
    assert_eq!(p.strips[0].output, out, "routing is structure, not state");
    assert!(p.strips[1].effect(late).is_some());
    assert_eq!(p.strips[1].sends[0].level, 0.9);
}

#[test]
fn a_recall_can_name_channels_and_the_safe_ones_sit_out_the_rest() {
    let (mut p, _) = busy();
    let ids: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
    add(&mut p, 1);
    p.strips[0].gain_db = 3.0;
    p.strips[2].width = 0.1;
    // Only strip 2.
    run(
        &mut p,
        MixCommand::Recall {
            id: MixSnapId(1),
            scope: vec![ids[2]],
            master: false,
            include_safe: false,
        },
    );
    assert_eq!(p.strips[2].width, 1.6);
    assert_eq!(p.strips[0].gain_db, 3.0, "outside the scope");
    // Everything, with strip 0 safe: it stays; named, it is reached.
    run(
        &mut p,
        MixCommand::SetRecallSafe {
            id: ids[0],
            safe: true,
        },
    );
    run(
        &mut p,
        MixCommand::Recall {
            id: MixSnapId(1),
            scope: vec![],
            master: false,
            include_safe: false,
        },
    );
    assert_eq!(
        p.strips[0].gain_db, 3.0,
        "a safe strip sits out a recall of all"
    );
    run(
        &mut p,
        MixCommand::Recall {
            id: MixSnapId(1),
            scope: vec![ids[0]],
            master: false,
            include_safe: false,
        },
    );
    assert_eq!(p.strips[0].gain_db, -4.5, "named, it is recalled");
}

#[test]
fn snapshots_are_taken_renamed_updated_and_removed_with_exact_inverses() {
    let (mut p, _) = busy();
    let c0 = p.clone();
    let i1 = add(&mut p, 1);
    let c1 = p.clone();
    let i2 = run(
        &mut p,
        MixCommand::RenameSnapshot {
            id: MixSnapId(1),
            name: "verse".into(),
        },
    );
    assert_eq!(p.mix.snapshots[0].name, "verse");
    i2.apply(&mut p).unwrap();
    assert_eq!(p, c1);
    p.strips[0].gain_db = 0.0;
    let st = capture(&p, &[], true);
    let i3 = run(
        &mut p,
        MixCommand::UpdateSnapshot {
            id: MixSnapId(1),
            state: Box::new(st),
        },
    );
    i3.apply(&mut p).unwrap();
    assert_eq!(p.mix.snapshots[0].state.strips[0].gain_db, -4.5);
    p.strips[0].gain_db = -4.5;
    assert_eq!(p, c1);
    i1.apply(&mut p).unwrap();
    assert_eq!(p, c0);
    // Unknown ids are refused.
    assert!(Command::Mix(MixCommand::Recall {
        id: MixSnapId(9),
        scope: vec![],
        master: false,
        include_safe: false
    })
    .apply(&mut p)
    .is_err());
}

#[test]
fn a_cued_snapshot_cannot_be_removed() {
    let (mut p, _) = busy();
    add(&mut p, 1);
    let cue = MixCue {
        id: MixCueId(2),
        at: Ticks::beats(4),
        snapshot: MixSnapId(1),
        scope: vec![],
    };
    run(&mut p, MixCommand::InsertCue { index: 0, cue });
    assert!(
        Command::Mix(MixCommand::RemoveSnapshot { id: MixSnapId(1) })
            .apply(&mut p)
            .is_err()
    );
    run(&mut p, MixCommand::RemoveCue { id: MixCueId(2) });
    assert!(
        Command::Mix(MixCommand::RemoveSnapshot { id: MixSnapId(1) })
            .apply(&mut p)
            .is_ok()
    );
}

#[test]
fn a_cue_holds_the_snapshots_values_from_its_bar_on() {
    let (mut p, _) = busy();
    add(&mut p, 1);
    p.strips[0].gain_db = 0.0; // the live fader differs from the snapshot's
    let at = Ticks::beats(8);
    run(
        &mut p,
        MixCommand::InsertCue {
            index: 0,
            cue: MixCue {
                id: MixCueId(2),
                at,
                snapshot: MixSnapId(1),
                scope: vec![],
            },
        },
    );
    let s = crate::snapshot::Snapshot::compile(&p, 1);
    let lane = s
        .automation
        .iter()
        .find(|l| l.target == AutoTarget::StripGain(p.strips[0].id))
        .expect("the fader has a lane");
    assert_eq!(
        lane.value_at(0.0),
        0.0,
        "before the cue, the project's value"
    );
    assert!(
        (lane.value_at(at.0 as f64) - -4.5).abs() < 1e-3,
        "from the cue, the snapshot's"
    );
    assert!(
        (lane.value_at((at.0 * 3) as f64) - -4.5).abs() < 1e-3,
        "and it holds"
    );
    // The master is as the snapshot has it: nothing to hold.
    assert!(!s
        .automation
        .iter()
        .any(|l| l.target == AutoTarget::MasterGain));
    // No cue, no lane: the null.
    let mut q = p.clone();
    q.mix.cues.clear();
    assert!(crate::snapshot::Snapshot::compile(&q, 1)
        .automation
        .is_empty());
}

#[test]
fn a_clip_starting_after_the_cue_takes_over_where_it_starts() {
    let (mut p, _) = busy();
    add(&mut p, 1);
    let target = AutoTarget::StripGain(p.strips[0].id);
    let range = crate::automation::range::range_of(&p, &target).unwrap();
    let clip = p.make_automation_clip(
        "later",
        target.clone(),
        Ticks::beats(16),
        Ticks::beats(4),
        vec![crate::automation::AutoPoint::new(
            Ticks::ZERO,
            range.pos(-12.0),
        )],
    );
    let mut t = p.make_track("auto");
    t.strip = None;
    t.clips.push(clip);
    p.tracks.push(t);
    // The live fader differs from the snapshot's, so the cue holds something.
    p.strips[0].gain_db = 0.0;
    let id = p.strips[0].id;
    p.mix.cues.push(MixCue {
        id: MixCueId(2),
        at: Ticks::beats(8),
        snapshot: MixSnapId(1),
        scope: vec![id],
    });
    let s = crate::snapshot::Snapshot::compile(&p, 1);
    let l = s.automation.iter().find(|l| l.target == target).unwrap();
    assert!((l.value_at(Ticks::beats(9).0 as f64) - -4.5).abs() < 1e-3);
    assert!((l.value_at(Ticks::beats(17).0 as f64) - -12.0).abs() < 1e-3);
}

#[test]
fn the_edit_lock_sees_what_a_recall_would_change() {
    let (mut p, _) = busy();
    add(&mut p, 1);
    let recall = Command::Mix(MixCommand::Recall {
        id: MixSnapId(1),
        scope: vec![],
        master: true,
        include_safe: false,
    });
    assert!(
        crate::freeze::touches(&recall, &p).is_empty(),
        "a recall of what is already on the desk changes nothing"
    );
    p.strips[0].effects[0].bypass = true;
    p.strips[0].gain_db = 1.0;
    p.master.gain_db = 3.0;
    let t = crate::freeze::touches(&recall, &p);
    let s0 = p.strips[0].id;
    assert!(t.contains(&crate::freeze::Touch::StripChain(s0)));
    assert!(t.contains(&crate::freeze::Touch::StripMix(s0)));
    assert!(t.contains(&crate::freeze::Touch::Global));
}

#[test]
fn a_project_with_snapshots_round_trips_through_json() {
    let (mut p, _) = busy();
    let ids: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
    add(&mut p, 1);
    run(
        &mut p,
        MixCommand::InsertCue {
            index: 0,
            cue: MixCue {
                id: MixCueId(2),
                at: Ticks::beats(4),
                snapshot: MixSnapId(1),
                scope: vec![ids[0]],
            },
        },
    );
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
    // A project without any writes none of it.
    let plain = serde_json::to_string(&Project::default()).unwrap();
    assert!(
        !plain.contains("\"mix\"") && !plain.contains("recall_safe") && !plain.contains("vcas")
    );
}

// ---- review 1 ---------------------------------------------------------------------------

/// Item 9: a recall that writes a key the insert did not hold is undone to the bit. Failed
/// before: the inverse set the old keys one by one and left the new one.
#[test]
fn undoing_a_recall_takes_out_the_keys_it_added() {
    let (mut p, _) = busy();
    // A kind that stores every key (not a sparse one).
    assert!(!EffectKind::Delay.sparse());
    with_effect(&mut p, 1, EffectKind::Delay);
    let key = EffectKind::Delay.params()[1].key;
    add(&mut p, 1);
    // The insert now lacks that key (an older file, a key added since).
    p.strips[1].effects[0].spec.params.remove(key);
    let before = p.clone();
    let inv = run(
        &mut p,
        MixCommand::Recall {
            id: MixSnapId(1),
            scope: vec![],
            master: true,
            include_safe: false,
        },
    );
    assert!(p.strips[1].effects[0].spec.params.contains_key(key));
    inv.apply(&mut p).unwrap();
    assert_eq!(p, before);
}

/// Item 8: taking a partial snapshot again keeps it partial, and without the master when it
/// had none.
#[test]
fn an_update_keeps_a_partial_snapshot_partial() {
    let (p, _) = busy();
    let ids = [p.strips[0].id, p.strips[3].id];
    let old = capture(&p, &ids, false);
    let again = recapture(&p, &old);
    assert_eq!(again.strips.len(), 2);
    assert!(again.master.is_none());
    let full = recapture(&p, &capture(&p, &[], true));
    assert_eq!(full.strips.len(), p.strips.len());
    assert!(full.master.is_some());
}

fn cue(p: &mut Project, at: Ticks, snapshot: u64) {
    let id = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id,
        at,
        snapshot: MixSnapId(snapshot),
        scope: vec![],
    });
}

/// Item 4: a cue of the desk as it is makes no lane at all; one changed fader makes one.
/// Failed before: every native parameter of every strip got a lane.
#[test]
fn a_cue_makes_lanes_only_for_what_it_changes() {
    let (mut p, _) = busy();
    add(&mut p, 1);
    cue(&mut p, Ticks::beats(4), 1);
    assert!(crate::snapshot::Snapshot::compile(&p, 1)
        .automation
        .is_empty());
    p.mix.snapshots[0].state.strips[3].gain_db = -10.0;
    let s = crate::snapshot::Snapshot::compile(&p, 1);
    assert_eq!(s.automation.len(), 1);
    assert_eq!(
        s.automation[0].target,
        AutoTarget::StripGain(p.strips[3].id)
    );
}

/// Item 10: a cue inside a clip holds until the next clip starts, the clip's later points
/// silenced. Failed before: the clip's next point took over.
#[test]
fn a_cue_inside_a_clip_holds_until_the_next_clip() {
    use crate::automation::AutoPoint;
    let (mut p, _) = busy();
    add(&mut p, 1);
    let s0 = p.strips[0].id;
    let target = AutoTarget::StripGain(s0);
    let range = crate::automation::range::range_of(&p, &target).unwrap();
    // A clip from beat 0 to 16 ramping the fader down, and another from beat 24.
    let long = p.make_automation_clip(
        "ride",
        target.clone(),
        Ticks::ZERO,
        Ticks::beats(16),
        vec![
            AutoPoint::new(Ticks::ZERO, range.pos(0.0)),
            AutoPoint::new(Ticks::beats(8), range.pos(-20.0)),
            AutoPoint::new(Ticks::beats(12), range.pos(-30.0)),
        ],
    );
    let later = p.make_automation_clip(
        "later",
        target.clone(),
        Ticks::beats(24),
        Ticks::beats(4),
        vec![AutoPoint::new(Ticks::ZERO, range.pos(-40.0))],
    );
    let mut t = p.make_track("auto");
    t.strip = None;
    t.clips.push(long);
    t.clips.push(later);
    p.tracks.push(t);
    cue(&mut p, Ticks::beats(4), 1);
    let s = crate::snapshot::Snapshot::compile(&p, 1);
    let lane = s.automation.iter().find(|l| l.target == target).unwrap();
    let at = |b: i64| lane.value_at(Ticks::beats(b).0 as f64);
    assert!((at(5) - -4.5).abs() < 1e-3, "the cue holds");
    assert!(
        (at(13) - -4.5).abs() < 1e-3,
        "past the clip's later points: {}",
        at(13)
    );
    assert!((at(20) - -4.5).abs() < 1e-3, "until the next clip");
    assert!(
        (at(25) - -40.0).abs() < 1e-2,
        "which takes over: {}",
        at(25)
    );
    assert!(
        at(2) > -10.0 && at(2) < 0.0,
        "before the cue, the ride: {}",
        at(2)
    );
}

/// Item 5: the switch timeline — a cue that mutes a strip and bypasses an insert makes a
/// region where it is silent and bypassed; the project's own region comes first; a cue
/// that switches nothing compiles to nothing.
#[test]
fn the_cues_switch_timeline() {
    use crate::mix_scene::{region_at, timeline};
    let (mut p, _) = busy();
    add(&mut p, 1);
    cue(&mut p, Ticks::beats(4), 1);
    assert!(
        timeline(&p).is_empty(),
        "a cue of the desk as it is switches nothing"
    );
    p.mix.snapshots[0].state.strips[0].muted = true;
    p.mix.snapshots[0].state.strips[0].effects[0].bypass = true;
    p.mix.snapshots[0].state.strips[5].soloed = true;
    let t = timeline(&p);
    assert_eq!(t.len(), 2);
    assert!(!t[0].strips[0].silent && !t[0].strips[0].bypass[0]);
    assert!(t[1].strips[0].silent && t[1].strips[0].bypass[0]);
    // Strip 5 soloed: strip 3 (not connected) is silenced by it, from the cue on.
    assert!(!t[0].strips[3].silent && t[1].strips[3].silent);
    assert_eq!(region_at(&t, 0.0), 0);
    assert_eq!(region_at(&t, Ticks::beats(4).0 as f64), 1);
    assert_eq!(region_at(&t, -1.0e9), 0);
    // The snapshot carries it.
    assert_eq!(crate::snapshot::Snapshot::compile(&p, 1).mix_scenes, t);
}

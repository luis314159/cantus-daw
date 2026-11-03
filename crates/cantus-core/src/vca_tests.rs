//! VCA semantics (ADR 0093): who follows whom, mute and solo, the commands, the compile.

use super::*;
use crate::automation::{AutoSeg, AutoTarget, Shape};
use crate::command::Command;
use crate::mix_cmd::MixCommand;
use crate::model::{db_to_linear, Project};
use crate::snapshot::Snapshot;
use crate::time::Ticks;

fn vca(p: &mut Project, name: &str) -> StripId {
    let mut s = p.make_strip(name);
    s.kind = StripKind::Vca;
    let id = s.id;
    p.strips.push(s);
    id
}

fn follow(p: &mut Project, s: StripId, vcas: &[StripId]) {
    p.strip_mut(s).unwrap().vcas = vcas.to_vec();
}

fn desk() -> (Project, Vec<StripId>) {
    let p = Project::default();
    let ids = p.strips.iter().map(|s| s.id).collect();
    (p, ids)
}

#[test]
fn nothing_follows_nothing_by_default() {
    let (p, ids) = desk();
    assert!(!p.has_vca());
    assert!(p.vca_governing(ids[0]).is_empty());
    assert!(!p.vca_cycle());
}

#[test]
fn a_strip_hears_every_vca_that_reaches_it_once() {
    let (mut p, ids) = desk();
    let (a, b, c) = (vca(&mut p, "A"), vca(&mut p, "B"), vca(&mut p, "C"));
    // Nested: C follows B follows A; a strip follows C and A directly (A twice over).
    follow(&mut p, c, &[b]);
    follow(&mut p, b, &[a]);
    follow(&mut p, ids[0], &[c, a]);
    let g = p.vca_governing(ids[0]);
    assert_eq!(g.len(), 3, "A, B and C, A once: {g:?}");
    for v in [a, b, c] {
        assert!(g.contains(&v));
    }
    // A member in two VCAs hears both.
    follow(&mut p, ids[1], &[a, b]);
    assert_eq!(p.vca_governing(ids[1]), vec![a, b]);
    // The VCAs' members, transitively.
    let m = p.vca_members(a);
    assert!(m.contains(&b) && m.contains(&c) && m.contains(&ids[0]) && m.contains(&ids[1]));
    assert!(!m.contains(&ids[2]));
}

#[test]
fn a_group_gives_its_strips_a_vca() {
    let (mut p, ids) = desk();
    let a = vca(&mut p, "A");
    let g = crate::model::GroupId(p.alloc_id());
    p.strip_groups.push(crate::model::StripGroup {
        id: g,
        name: "drums".into(),
        color: crate::model::Color::PALETTE[0],
        collapsed: false,
    });
    p.strip_mut(ids[0]).unwrap().group = Some(g);
    p.mix.group_vcas.push((g, a));
    assert_eq!(p.vca_governing(ids[0]), vec![a]);
    assert!(p.vca_governing(ids[1]).is_empty());
}

#[test]
fn a_muted_vca_mutes_its_members_and_a_soloed_one_solos_them() {
    let (mut p, ids) = desk();
    let a = vca(&mut p, "A");
    follow(&mut p, ids[0], &[a]);
    assert!(p.strip_audibility().iter().all(|x| *x));
    p.strip_mut(a).unwrap().muted = true;
    let au = p.strip_audibility();
    assert!(!au[0], "the member is muted by its VCA");
    assert!(au[1], "a strip it does not reach plays");
    p.strip_mut(a).unwrap().muted = false;
    p.strip_mut(a).unwrap().soloed = true;
    let au = p.strip_audibility();
    assert!(au[0], "the member is soloed by its VCA");
    assert!(!au[1], "everything else is silenced by the solo");
    // The member's own mute still wins.
    p.strip_mut(ids[0]).unwrap().muted = true;
    assert!(!p.strip_audibility()[0]);
}

#[test]
fn no_vca_leaves_audibility_as_it_was() {
    let (mut p, _) = desk();
    p.strips[2].soloed = true;
    let a = p.strip_audibility();
    assert!(a[2] && !a[0]);
}

fn exec(p: &mut Project, c: Command) -> Result<Command, crate::command::CommandError> {
    c.apply(p)
}

#[test]
fn follow_lists_are_checked_and_undo_exactly() {
    let (mut p, ids) = desk();
    let (a, b) = (vca(&mut p, "A"), vca(&mut p, "B"));
    let before = p.clone();
    // Only a VCA can be followed; not oneself; no duplicates.
    for bad in [vec![ids[1]], vec![ids[0]], vec![a, a]] {
        let r = exec(
            &mut p,
            Command::Mix(MixCommand::SetStripVcas {
                id: ids[0],
                vcas: bad,
            }),
        );
        assert!(r.is_err());
    }
    assert_eq!(p, before, "a refused command changes nothing");
    let inv = exec(
        &mut p,
        Command::Mix(MixCommand::SetStripVcas {
            id: b,
            vcas: vec![a],
        }),
    )
    .unwrap();
    // A cycle is refused, and put back.
    let mid = p.clone();
    let r = exec(
        &mut p,
        Command::Mix(MixCommand::SetStripVcas {
            id: a,
            vcas: vec![b],
        }),
    );
    assert!(r.is_err());
    assert_eq!(p, mid);
    exec(&mut p, inv).unwrap();
    assert_eq!(p, before);
}

#[test]
fn a_vca_has_no_audio_and_a_followed_one_stays() {
    let (mut p, ids) = desk();
    let a = vca(&mut p, "A");
    // No track, no output, no send, no insert on a VCA.
    let t = p.make_track("t");
    let tid = t.id;
    p.tracks.push(t);
    assert!(exec(
        &mut p,
        Command::SetTrackStrip {
            id: tid,
            strip: Some(a)
        }
    )
    .is_err());
    assert!(exec(
        &mut p,
        Command::SetStripOutput {
            id: ids[0],
            output: Some(a)
        }
    )
    .is_err());
    assert!(exec(
        &mut p,
        Command::SetStripOutput {
            id: a,
            output: Some(ids[1])
        }
    )
    .is_err());
    // A followed VCA cannot be removed; once nobody follows it, it can.
    follow(&mut p, ids[0], &[a]);
    assert!(exec(&mut p, Command::RemoveStrip { id: a }).is_err());
    follow(&mut p, ids[0], &[]);
    let inv = exec(&mut p, Command::RemoveStrip { id: a }).unwrap();
    assert!(p.strip(a).is_none());
    exec(&mut p, inv).unwrap();
    assert!(p.is_vca(a));
    // A new track never lands on a VCA.
    assert_ne!(p.first_free_strip(), Some(a));
}

fn gain_of(s: &Snapshot, id: StripId) -> f32 {
    s.strips.iter().find(|x| x.id == id).unwrap().gain
}

#[test]
fn a_vca_at_unity_touches_no_gain_to_the_bit() {
    let (mut p, ids) = desk();
    p.strip_mut(ids[0]).unwrap().gain_db = -3.7;
    let plain = Snapshot::compile(&p, 1);
    let a = vca(&mut p, "A");
    follow(&mut p, ids[0], &[a]);
    let with = Snapshot::compile(&p, 1);
    for id in &ids {
        assert_eq!(
            gain_of(&plain, *id).to_bits(),
            gain_of(&with, *id).to_bits(),
            "{id}"
        );
    }
    assert!(with.vca_links.is_empty());
}

#[test]
fn a_vca_fader_scales_the_members_gain_stage_in_decibels() {
    let (mut p, ids) = desk();
    p.strip_mut(ids[0]).unwrap().gain_db = -3.0;
    let a = vca(&mut p, "A");
    let b = vca(&mut p, "B");
    follow(&mut p, ids[0], &[a, b]);
    p.strip_mut(a).unwrap().gain_db = -6.0;
    p.strip_mut(b).unwrap().gain_db = 2.0;
    let s = Snapshot::compile(&p, 1);
    let want = db_to_linear(-3.0 + -6.0 + 2.0);
    assert!((gain_of(&s, ids[0]) - want).abs() < 1e-6);
    // Its own fader value is what the project still says.
    assert_eq!(p.strip(ids[0]).unwrap().gain_db, -3.0);
    // The VCA itself is never run.
    assert!(s.strips.iter().find(|x| x.id == a).unwrap().rest);
    // A strip nobody reaches is untouched.
    assert_eq!(gain_of(&s, ids[1]), db_to_linear(0.0));
}

#[test]
fn an_automated_vca_rides_each_members_own_lane() {
    let (mut p, ids) = desk();
    let a = vca(&mut p, "A");
    follow(&mut p, ids[0], &[a]);
    follow(&mut p, ids[1], &[a]);
    let target = AutoTarget::StripGain(a);
    let mut lanes = vec![crate::automation::AutoLane {
        target: target.clone(),
        range: crate::automation::range::range_of(&p, &target).unwrap(),
        base: 0.0,
        segs: vec![AutoSeg {
            at: Ticks::ZERO,
            t0: Ticks::ZERO,
            t1: Ticks::beats(4),
            v0: 0.5,
            v1: 0.2,
            shape: Shape::Curve,
            tension: 0.0,
        }],
    }];
    let c = compile(&p, &mut lanes);
    assert_eq!(c.links.len(), 2, "one link per member");
    for l in &c.links {
        assert!(matches!(lanes[l.lane].target, AutoTarget::StripGain(_)));
        assert_eq!(l.from, vec![0]);
        assert_eq!(l.static_db, 0.0);
    }
    assert_eq!(lanes.len(), 3, "a lane made for each member");
}

#[test]
fn a_member_with_a_lane_of_its_own_gets_the_static_vcas_added_to_it() {
    let (mut p, ids) = desk();
    let a = vca(&mut p, "A");
    follow(&mut p, ids[0], &[a]);
    p.strip_mut(a).unwrap().gain_db = -6.0;
    let target = AutoTarget::StripGain(ids[0]);
    let mut lanes = vec![crate::automation::AutoLane {
        target: target.clone(),
        range: crate::automation::range::range_of(&p, &target).unwrap(),
        base: 0.0,
        segs: Vec::new(),
    }];
    let c = compile(&p, &mut lanes);
    assert_eq!(c.links.len(), 1);
    assert_eq!(c.links[0].static_db, -6.0);
    assert!(c.links[0].from.is_empty());
}

#[test]
fn a_solo_safe_strip_stays_when_another_is_soloed() {
    let (mut p, ids) = desk();
    p.strips[2].soloed = true;
    p.strips[5].solo_safe = true;
    let a = p.strip_audibility();
    assert!(a[2] && a[5], "the solo and the safe strip play");
    assert!(!a[0] && !a[3]);
    // Muted, it is still silent.
    p.strips[5].muted = true;
    assert!(!p.strip_audibility()[5]);
    // The same with a VCA on the desk.
    p.strips[5].muted = false;
    let v = vca(&mut p, "A");
    follow(&mut p, ids[2], &[v]);
    assert!(p.strip_audibility()[5]);
}

#[test]
fn the_safe_switches_undo_exactly() {
    let (mut p, ids) = desk();
    let before = p.clone();
    let i = exec(
        &mut p,
        Command::Mix(MixCommand::SetSoloSafe {
            id: ids[1],
            safe: true,
        }),
    )
    .unwrap();
    assert!(p.strips[1].solo_safe);
    exec(&mut p, i).unwrap();
    assert_eq!(p, before);
}

// ---- review 1 ---------------------------------------------------------------------------

/// Item 2: a VCA inside its own group follows nothing through it — no cycle, so later edits
/// are not refused, and undo stays exact. Failed before: `vca_cycle` saw the group edge.
#[test]
fn a_vca_in_its_own_group_is_no_cycle_and_undo_stays_exact() {
    let (mut p, ids) = desk();
    let x = vca(&mut p, "X");
    let g = crate::model::GroupId(p.alloc_id());
    p.strip_groups.push(crate::model::StripGroup {
        id: g,
        name: "g".into(),
        color: crate::model::Color::PALETTE[0],
        collapsed: false,
    });
    let start = p.clone();
    let mut undo = Vec::new();
    for c in [
        Command::Mix(MixCommand::SetGroupVca {
            id: g,
            vca: Some(x),
        }),
        Command::SetStripGroup {
            id: x,
            group: Some(g),
        },
        // A later follow edit is not refused.
        Command::Mix(MixCommand::SetStripVcas {
            id: ids[0],
            vcas: vec![x],
        }),
        Command::Mix(MixCommand::SetGroupVca { id: g, vca: None }),
    ] {
        undo.push(exec(&mut p, c).expect("applied"));
    }
    assert!(!p.vca_cycle());
    while let Some(c) = undo.pop() {
        exec(&mut p, c).expect("undone");
    }
    assert_eq!(p, start);
}

/// Item 3: the fader's floor wins over a sum — a member at the bottom under a raised VCA is
/// silent, and so is a member under a VCA at the bottom. Failed before: −96 + 6 = −90.
#[test]
fn the_floor_is_silence_whatever_the_other_fader_says() {
    use crate::model::{MAX_GAIN_DB, MIN_GAIN_DB};
    assert_eq!(sum_db(MIN_GAIN_DB, 6.0), MIN_GAIN_DB);
    assert_eq!(sum_db(6.0, MIN_GAIN_DB), MIN_GAIN_DB);
    assert_eq!(sum_db(-3.0, -6.0), -9.0);
    for (member, v) in [(MIN_GAIN_DB, MAX_GAIN_DB), (MAX_GAIN_DB, MIN_GAIN_DB)] {
        let (mut p, ids) = desk();
        let a = vca(&mut p, "A");
        follow(&mut p, ids[0], &[a]);
        p.strip_mut(ids[0]).unwrap().gain_db = member;
        p.strip_mut(a).unwrap().gain_db = v;
        let s = Snapshot::compile(&p, 1);
        assert_eq!(gain_of(&s, ids[0]), 0.0, "member {member}, VCA {v}");
    }
}

/// Item 1: a bus frozen with a VCA's member among its inputs — the VCA's fader and mute are
/// in its key, its automation reaches it, and the commands that move it touch the member.
/// Failed before: `vcas` was cosmetic and nothing touched the member.
#[test]
fn a_frozen_bus_hears_its_inputs_vcas() {
    use crate::freeze::{content_key, touches, FreezeTarget, Touch};
    let (mut p, ids) = desk();
    let (member, bus) = (ids[0], ids[1]);
    p.strip_mut(member).unwrap().output = Some(bus);
    let a = vca(&mut p, "A");
    let key = |p: &Project| content_key(p, FreezeTarget::Strip(bus)).unwrap();
    let before = key(&p);
    follow(&mut p, member, &[a]);
    let followed = key(&p);
    assert_ne!(before, followed, "following a VCA is in the key");
    p.strip_mut(a).unwrap().gain_db = -6.0;
    assert_ne!(key(&p), followed, "the VCA's fader is in the key");
    let at = key(&p);
    p.strip_mut(a).unwrap().muted = true;
    assert_ne!(key(&p), at, "its mute too");
    // The commands that move it touch the member's mix.
    let has = |t: Vec<Touch>| t.contains(&Touch::StripMix(member));
    assert!(has(touches(
        &Command::SetStripGain {
            id: a,
            gain_db: 0.0
        },
        &p
    )));
    assert!(has(touches(
        &Command::Mix(MixCommand::SetStripVcas {
            id: member,
            vcas: vec![]
        }),
        &p
    )));
    // A clip on the VCA's fader is inside the bus's region.
    let target = AutoTarget::StripGain(a);
    let r = crate::freeze::region(&p, FreezeTarget::Strip(bus));
    assert!(crate::freeze::reaches(
        &p,
        FreezeTarget::Strip(bus),
        &r,
        &target
    ));
    // A project without VCAs keeps the key it had.
    let (mut q, qids) = desk();
    q.strip_mut(qids[0]).unwrap().output = Some(qids[1]);
    assert_eq!(key(&q), before);
}

/// Item 7: a refused group VCA leaves the groups' VCAs in their order. Failed before: the
/// group's entry came back at the end.
#[test]
fn a_refused_group_vca_keeps_the_order() {
    let (mut p, _) = desk();
    let (a, b) = (vca(&mut p, "A"), vca(&mut p, "B"));
    let mut groups = Vec::new();
    for n in 0..2 {
        let g = crate::model::GroupId(p.alloc_id());
        p.strip_groups.push(crate::model::StripGroup {
            id: g,
            name: format!("{n}"),
            color: crate::model::Color::PALETTE[0],
            collapsed: false,
        });
        groups.push(g);
    }
    // A sits in group 0; both groups follow C; B follows A.
    let c = vca(&mut p, "C");
    p.strip_mut(a).unwrap().group = Some(groups[0]);
    p.mix.group_vcas = vec![(groups[0], c), (groups[1], c)];
    follow(&mut p, b, &[a]);
    assert!(!p.vca_cycle());
    let before = p.clone();
    // Group 0 (holding A) following B, which follows A: refused.
    let r = exec(
        &mut p,
        Command::Mix(MixCommand::SetGroupVca {
            id: groups[0],
            vca: Some(b),
        }),
    );
    assert!(r.is_err());
    assert_eq!(p, before);
}

/// Item 11: no track, output, send or key into a VCA, whatever command brings it.
#[test]
fn nothing_routes_audio_into_a_vca() {
    let (mut p, ids) = desk();
    let a = vca(&mut p, "A");
    let mut t = p.make_track("t");
    t.strip = Some(a);
    assert!(exec(
        &mut p,
        Command::InsertTrack {
            index: 0,
            track: Box::new(t)
        }
    )
    .is_err());
    let mut s = p.make_strip("s");
    s.output = Some(a);
    assert!(exec(
        &mut p,
        Command::InsertStrip {
            index: 0,
            strip: Box::new(s)
        }
    )
    .is_err());
    let slot = crate::effect::EffectSlot {
        id: crate::model::EffectId(p.alloc_id()),
        bypass: false,
        spec: crate::effect::EffectSpec::new(crate::effect::EffectKind::Compressor),
        mix: 1.0,
        sidechain: None,
        midi_from: None,
    };
    let fx = slot.id;
    p.strip_mut(ids[0]).unwrap().effects.push(slot);
    assert!(exec(
        &mut p,
        Command::SetEffectSidechain {
            strip: ids[0],
            effect: fx,
            source: Some(a)
        }
    )
    .is_err());
}

//! Round 25, piece A (ADR 0041): who feeds a strip, the one answer to "can this route be
//! made?", the doubled-signal mark, the edge list the cables and the matrix draw, and the
//! batches every routing gesture issues — each one step of undo, each exactly undone.

use cantus_core::routing::{self, plan, Edge, EdgeKind, End, Route, RouteKind, RouteRefusal};
use cantus_core::*;

/// A project with `n` strips beyond the default sixteen's first `n`, and their ids.
fn desk(n: usize) -> (Project, Vec<StripId>) {
    let p = Project::default();
    let ids = p.strips.iter().take(n).map(|s| s.id).collect();
    (p, ids)
}

fn apply(p: &mut Project, cmd: Command) {
    cmd.apply(p).expect("applies");
}

/// Apply through a bus, then undo: the project must be the one before, to the bit.
fn applies_and_undoes(p: &Project, cmd: Command) -> Project {
    let mut bus = CommandBus::new(p.clone());
    bus.execute(cmd, Origin::User).expect("applies");
    let after = bus.project().clone();
    bus.undo();
    assert!(bus.project() == p, "undo restores the project exactly");
    bus.redo();
    assert!(bus.project() == &after, "redo restores the result exactly");
    after
}

fn label(cmd: &Command) -> &str {
    match cmd {
        Command::Batch { label_key, .. } => label_key,
        other => panic!("not a batch: {other:?}"),
    }
}

// ---- InsertStripSend: a removed send comes back in its place ------------------------

#[test]
fn a_removed_pre_fader_send_comes_back_where_it_was_with_its_tap() {
    let (mut p, s) = desk(5);
    for (i, to) in s[1..4].iter().enumerate() {
        let mut send = Send::new(*to);
        send.level = 0.1 * (i + 1) as f32;
        if i == 1 {
            send.tap = SendTap::Pre;
            send.enabled = false;
        }
        apply(&mut p, Command::SetStripSend { id: s[0], send });
    }
    let cmd = Command::RemoveStripSend { id: s[0], to: s[2] };
    let after = applies_and_undoes(&p, cmd);
    assert_eq!(after.strip(s[0]).unwrap().sends.len(), 2);
}

#[test]
fn insert_strip_send_refuses_what_a_new_send_refuses() {
    let (mut p, s) = desk(3);
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[1],
            output: Some(s[0]),
        },
    );
    let at = |to| Command::InsertStripSend {
        id: s[0],
        index: 7,
        send: Send::new(to),
    };
    assert!(at(s[0]).apply(&mut p.clone()).is_err(), "itself");
    assert!(at(s[1]).apply(&mut p.clone()).is_err(), "a loop: 2 feeds 1");
    assert!(
        at(StripId(424_242)).apply(&mut p.clone()).is_err(),
        "unknown"
    );
    let mut q = p.clone();
    at(s[2])
        .apply(&mut q)
        .expect("a new send, its index clamped");
    assert!(at(s[2]).apply(&mut q.clone()).is_err(), "a duplicate");
    let mut full = p.clone();
    for i in 0..MAX_SENDS {
        let r = full.make_strip(format!("r{i}"));
        let id = r.id;
        full.strips.push(r);
        apply(
            &mut full,
            Command::SetStripSend {
                id: s[2],
                send: Send::new(id),
            },
        );
    }
    let more = Command::InsertStripSend {
        id: s[2],
        index: 0,
        send: Send::new(s[0]),
    };
    assert!(more.apply(&mut full).is_err(), "a 17th");
}

// ---- feeders -------------------------------------------------------------------------

#[test]
fn a_return_fed_only_by_sends_is_fed() {
    // The owner's bug: every demo's Reverb and Delay said "Nothing feeds this strip yet".
    let (mut p, s) = desk(4);
    let ret = s[3];
    let mut send = Send::new(ret);
    send.level = 0.25;
    send.tap = SendTap::Pre;
    apply(&mut p, Command::SetStripSend { id: s[0], send });
    let f = p.feeders(ret);
    assert!(!f.is_empty());
    assert!(routing::is_fed(&p, ret));
    assert!(f.tracks.is_empty() && f.outputs.is_empty() && f.keys.is_empty());
    assert_eq!(
        f.sends,
        vec![(s[0], send)],
        "the level, the switch and the tap"
    );
    assert!(p.feeders(s[2]).is_empty());
    assert!(!routing::is_fed(&p, s[2]));
}

#[test]
fn feeders_lists_tracks_outputs_sends_keys_and_notes() {
    let (mut p, s) = desk(5);
    let t = p.make_track("Kick");
    let tid = t.id;
    p.tracks.push(t);
    apply(
        &mut p,
        Command::SetTrackStrip {
            id: tid,
            strip: Some(s[4]),
        },
    );
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[1],
            output: Some(s[4]),
        },
    );
    let mut off = Send::new(s[4]);
    off.enabled = false;
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[2],
            send: off,
        },
    );
    let comp = p.make_effect(EffectKind::Compressor);
    let gate = p.make_effect(EffectKind::Gate);
    let (cid, gid) = (comp.id, gate.id);
    for slot in [comp, gate] {
        let index = p.strip(s[4]).unwrap().effects.len();
        apply(
            &mut p,
            Command::InsertEffect {
                strip: s[4],
                index,
                slot: Box::new(slot),
            },
        );
    }
    apply(
        &mut p,
        Command::SetEffectSidechain {
            strip: s[4],
            effect: cid,
            source: Some(s[3]),
        },
    );
    apply(
        &mut p,
        Command::SetEffectMidiSource {
            strip: s[4],
            effect: gid,
            source: Some(tid),
        },
    );
    let f = routing::feeders(&p, s[4]);
    assert_eq!(f.tracks, vec![tid]);
    assert_eq!(f.outputs, vec![s[1]]);
    assert_eq!(
        f.sends,
        vec![(s[2], off)],
        "a switched-off send still feeds (it is a route)"
    );
    assert_eq!(f.keys, vec![(s[3], cid)]);
    assert_eq!(f.notes, vec![(tid, gid)]);
    assert_eq!(f.len(), 5);
    // The master is fed by every strip whose output is the master.
    let m = p.feeders(StripId::MASTER);
    assert!(m.outputs.contains(&s[0]) && !m.outputs.contains(&s[1]));
    assert!(m.sends.is_empty());
}

// ---- doubled --------------------------------------------------------------------------

#[test]
fn a_send_where_the_output_already_goes_is_doubled_and_allowed() {
    let (mut p, s) = desk(4);
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[0],
            output: Some(s[1]),
        },
    );
    assert_eq!(
        p.route_check(s[0], s[1], RouteKind::Send),
        Ok(()),
        "allowed (answer 5)"
    );
    assert!(!routing::doubled(&p, s[0], s[1]), "no send yet");
    assert!(routing::output_reaches(&p, s[0], s[1]));
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[0],
            send: Send::new(s[1]),
        },
    );
    assert!(routing::doubled(&p, s[0], s[1]));
    // Through a bus: 1 → 2 → 3, and 1 also sends to 3.
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[1],
            output: Some(s[2]),
        },
    );
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[0],
            send: Send::new(s[2]),
        },
    );
    assert!(
        routing::doubled(&p, s[0], s[2]),
        "3 hears 1 twice, through 2"
    );
    assert!(!routing::doubled(&p, s[0], s[3]));
    let mut off = Send::new(s[2]);
    off.enabled = false;
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[0],
            send: off,
        },
    );
    assert!(
        !routing::doubled(&p, s[0], s[2]),
        "a switched-off send carries nothing"
    );
}

// ---- edges ----------------------------------------------------------------------------

#[test]
fn every_route_is_one_edge_in_strip_order() {
    let (mut p, s) = desk(4);
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[0],
            output: Some(s[2]),
        },
    );
    let mut off = Send::new(s[3]);
    off.enabled = false;
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[0],
            send: off,
        },
    );
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[1],
            send: Send::new(s[3]),
        },
    );
    let comp = p.make_effect(EffectKind::Compressor);
    let cid = comp.id;
    apply(
        &mut p,
        Command::InsertEffect {
            strip: StripId::MASTER,
            index: 0,
            slot: Box::new(comp),
        },
    );
    apply(
        &mut p,
        Command::SetEffectSidechain {
            strip: StripId::MASTER,
            effect: cid,
            source: Some(s[1]),
        },
    );
    let e = routing::edges(&p);
    let n = p.strips.len();
    assert_eq!(
        e.len(),
        n + 2 + 1,
        "an output per strip, two sends, one key"
    );
    assert_eq!(
        e[0],
        Edge {
            from: s[0],
            to: s[2],
            kind: EdgeKind::Output
        }
    );
    assert!(matches!(e[1].kind, EdgeKind::Send { enabled: false, .. }) && e[1].to == s[3]);
    assert!(e[2].is_master_output() && e[2].from == s[1]);
    assert!(matches!(e[3].kind, EdgeKind::Send { enabled: true, level, .. } if level == 1.0));
    let last = e.last().unwrap();
    assert_eq!(
        *last,
        Edge {
            from: s[1],
            to: StripId::MASTER,
            kind: EdgeKind::Key { effect: cid }
        }
    );
    // Moving an unrelated strip changes nothing but where its own master edge sits.
    let mut q = p.clone();
    let lone = q.strips[10].id;
    apply(
        &mut q,
        Command::MoveStrip {
            id: lone,
            index: 12,
        },
    );
    let f = routing::edges(&q);
    let routed = |v: &[Edge]| {
        v.iter()
            .filter(|x| !x.is_master_output())
            .copied()
            .collect::<Vec<_>>()
    };
    assert_eq!(routed(&e), routed(&f));
    let mut buf = Vec::with_capacity(64);
    routing::edges_into(&p, &mut buf);
    assert_eq!(buf, e);
}

// ---- route_check agrees with the commands ---------------------------------------------

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// A random graph built only through commands (refusals skipped): outputs, sends, inserts
/// of every kind on strips and the master, keys.
fn random_graph(seed: u64, strips: usize) -> Project {
    let mut rng = Lcg(seed);
    let mut p = Project::default();
    p.strips.truncate(strips);
    let ids: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
    let pick = |rng: &mut Lcg| ids[rng.below(ids.len())];
    for _ in 0..strips * 6 {
        let cmd = match rng.below(5) {
            0 => Command::SetStripOutput {
                id: pick(&mut rng),
                output: (rng.below(3) != 0).then(|| pick(&mut rng)),
            },
            1 | 2 => Command::SetStripSend {
                id: pick(&mut rng),
                send: Send::new(pick(&mut rng)),
            },
            3 => {
                let host = if rng.below(5) == 0 {
                    StripId::MASTER
                } else {
                    pick(&mut rng)
                };
                let kind = EffectKind::ALL[rng.below(EffectKind::ALL.len())];
                let slot = p.make_effect(kind);
                let index = p.effects_of(host).unwrap().len();
                Command::InsertEffect {
                    strip: host,
                    index,
                    slot: Box::new(slot),
                }
            }
            _ => {
                let host = if rng.below(5) == 0 {
                    StripId::MASTER
                } else {
                    pick(&mut rng)
                };
                let chain = p.effects_of(host).unwrap();
                let Some(e) = chain.get(rng.below(chain.len())) else {
                    continue;
                };
                Command::SetEffectSidechain {
                    strip: host,
                    effect: e.id,
                    source: Some(pick(&mut rng)),
                }
            }
        };
        let _ = cmd.apply(&mut p);
    }
    p
}

/// [`random_graph`], then crowded to the limits through commands: one strip with
/// [`MAX_SENDS`] sends (to strips added for them, so no loop refuses one), one host with
/// [`MAX_INSERTS`] inserts none of which takes a key, and one with as many whose only keyed
/// insert already listens to a strip — so the checks meet `NoRoom` and `AlreadyThere`.
fn crowded_graph(seed: u64) -> Project {
    let mut p = random_graph(seed, 3 + (seed as usize % 6));
    let add = |p: &mut Project, name: &str| {
        let strip = p.make_strip(name);
        let id = strip.id;
        let index = p.strips.len();
        apply(
            p,
            Command::InsertStrip {
                index,
                strip: Box::new(strip),
            },
        );
        id
    };
    let sender = p.strips[seed as usize % p.strips.len()].id;
    while p.strip(sender).unwrap().sends.len() < MAX_SENDS {
        let to = add(&mut p, "destination");
        let index = p.strip(sender).unwrap().sends.len();
        apply(
            &mut p,
            Command::InsertStripSend {
                id: sender,
                index,
                send: Send::new(to),
            },
        );
    }
    let fill = |p: &mut Project, host: StripId| {
        while p.effects_of(host).unwrap().len() < MAX_INSERTS {
            let slot = p.make_effect(EffectKind::Delay);
            assert!(!slot.spec.takes_sidechain());
            let index = p.effects_of(host).unwrap().len();
            apply(
                p,
                Command::InsertEffect {
                    strip: host,
                    index,
                    slot: Box::new(slot),
                },
            );
        }
    };
    let deaf = add(&mut p, "full, nothing keyable");
    fill(&mut p, deaf);
    let keyed = add(&mut p, "full, keyed already");
    let comp = p.make_effect(EffectKind::Compressor);
    let effect = comp.id;
    apply(
        &mut p,
        Command::InsertEffect {
            strip: keyed,
            index: 0,
            slot: Box::new(comp),
        },
    );
    apply(
        &mut p,
        Command::SetEffectSidechain {
            strip: keyed,
            effect,
            source: Some(sender),
        },
    );
    fill(&mut p, keyed);
    p
}

fn ok(cmd: Command, p: &Project) -> bool {
    cmd.apply(&mut p.clone()).is_ok()
}

/// Whether `p`'s routes (outputs, sends and keys: the processing order's edges) hold no
/// loop — the project's own rule, asked of every strip.
fn acyclic(p: &Project) -> bool {
    p.strips.iter().all(|s| {
        p.downstream(s.id)
            .into_iter()
            .all(|to| !p.routing_would_cycle(s.id, Some(to)))
    })
}

/// The raw commands a new keyed insert is: `slot` in at the end of `host`'s chain, then
/// keyed from `from` — on a copy, judged by whether both apply and leave a project with no
/// loop. Built without `route_check` (nor `plan::key_with`, which asks it), so it can
/// disagree with it.
fn fresh_key_applies(p: &Project, from: StripId, host: StripId, kind: EffectKind) -> bool {
    let mut q = p.clone();
    let slot = q.make_effect(kind);
    let effect = slot.id;
    let Some(index) = q.effects_of(host).map(|c| c.len()) else {
        return false;
    };
    let insert = Command::InsertEffect {
        strip: host,
        index,
        slot: Box::new(slot),
    };
    let key = Command::SetEffectSidechain {
        strip: host,
        effect,
        source: Some(from),
    };
    insert.apply(&mut q).is_ok() && key.apply(&mut q).is_ok() && acyclic(&q)
}

#[test]
fn route_check_agrees_with_the_commands_on_random_graphs() {
    let mut seen_loop = 0;
    let mut seen_ok = [0usize; 3];
    // Refusals for want of room, and for a key already there: `[send NoRoom, key-any
    // NoRoom, key-any AlreadyThere]`.
    let mut seen_full = [0usize; 3];
    let graphs = (1..=12u64)
        .map(|seed| (seed, random_graph(seed, 3 + (seed as usize % 6))))
        .chain((13..=15u64).map(|seed| (seed, crowded_graph(seed))));
    for (seed, p) in graphs {
        assert!(acyclic(&p), "seed {seed}: the commands built a loop");
        let mut ends: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
        ends.push(StripId::MASTER);
        ends.push(StripId(987_654));
        for &from in &ends {
            for &to in &ends {
                // Output: SetStripOutput applies iff Ok or AlreadyThere.
                let r = p.route_check(from, to, RouteKind::Output);
                let output = Command::SetStripOutput {
                    id: from,
                    output: (!to.is_master()).then_some(to),
                };
                assert_eq!(
                    ok(output, &p),
                    r.is_ok() || r == Err(RouteRefusal::AlreadyThere),
                    "seed {seed}: output {from:?} → {to:?}: {r:?}"
                );
                if r.is_ok() {
                    seen_ok[0] += 1;
                }
                if matches!(r, Err(RouteRefusal::Loop { .. })) {
                    seen_loop += 1;
                }
                // Send: a new one (InsertStripSend) applies iff Ok; SetStripSend also
                // applies to a send already there (it changes it).
                let r = p.route_check(from, to, RouteKind::Send);
                let insert = Command::InsertStripSend {
                    id: from,
                    index: 0,
                    send: Send::new(to),
                };
                assert_eq!(
                    ok(insert, &p),
                    r.is_ok(),
                    "seed {seed}: send {from:?} → {to:?}: {r:?}"
                );
                let set = Command::SetStripSend {
                    id: from,
                    send: Send::new(to),
                };
                assert_eq!(
                    ok(set, &p),
                    r.is_ok() || r == Err(RouteRefusal::AlreadyThere)
                );
                if r.is_ok() {
                    seen_ok[1] += 1;
                }
                if r == Err(RouteRefusal::NoRoom) {
                    seen_full[0] += 1;
                }
                // Key, per insert of the destination: SetEffectSidechain applies iff Ok or
                // AlreadyThere.
                let chain: Vec<EffectSlot> = p.effects_of(to).cloned().unwrap_or_default();
                for e in &chain {
                    let r = p.route_check(from, to, RouteKind::Key(Some(e.id)));
                    let key = Command::SetEffectSidechain {
                        strip: to,
                        effect: e.id,
                        source: Some(from),
                    };
                    assert_eq!(
                        ok(key, &p),
                        r.is_ok() || r == Err(RouteRefusal::AlreadyThere),
                        "seed {seed}: key {from:?} → {to:?}/{:?}: {r:?}",
                        e.spec.kind
                    );
                    if r.is_ok() {
                        seen_ok[2] += 1;
                    }
                }
                // Key, any insert: Ok iff some insert could be keyed anew, or a new keyed
                // compressor could go in — both judged by the raw commands alone.
                let r = p.route_check(from, to, RouteKind::Key(None));
                let anew = chain.iter().any(|e| {
                    e.sidechain != Some(from)
                        && ok(
                            Command::SetEffectSidechain {
                                strip: to,
                                effect: e.id,
                                source: Some(from),
                            },
                            &p,
                        )
                });
                let fresh = fresh_key_applies(&p, from, to, EffectKind::Compressor);
                assert_eq!(
                    fresh,
                    fresh_key_applies(&p, from, to, EffectKind::Gate),
                    "seed {seed}: a gate and a compressor go in alike"
                );
                assert_eq!(
                    r.is_ok(),
                    anew || fresh,
                    "seed {seed}: key any {from:?} → {to:?}: {r:?}"
                );
                match r {
                    Err(RouteRefusal::NoRoom) => seen_full[1] += 1,
                    Err(RouteRefusal::AlreadyThere) => seen_full[2] += 1,
                    _ => {}
                }
            }
        }
    }
    assert!(seen_loop > 0, "the graphs hold loops to refuse");
    assert!(
        seen_full.iter().all(|n| *n > 0),
        "a full strip refuses a send and a key, and a key already there is named:          {seen_full:?}"
    );
    assert!(
        seen_ok.iter().all(|n| *n > 0),
        "every kind is made somewhere: {seen_ok:?}"
    );
}

#[test]
fn a_loop_names_the_strip_that_closes_it() {
    let (mut p, s) = desk(4);
    // 1 → 2 → 3 by outputs: a route from 3 into 1 closes a loop, and the strip on the
    // path that feeds 3 is 2.
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[0],
            output: Some(s[1]),
        },
    );
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[1],
            output: Some(s[2]),
        },
    );
    assert_eq!(
        p.route_check(s[2], s[0], RouteKind::Send),
        Err(RouteRefusal::Loop { through: s[1] }),
        "1 reaches 3 through 2, the strip that feeds 3"
    );
    assert_eq!(
        p.route_check(s[1], s[0], RouteKind::Output),
        Err(RouteRefusal::Loop { through: s[0] })
    );
    assert_eq!(
        p.route_check(s[0], s[0], RouteKind::Send),
        Err(RouteRefusal::ToItself)
    );
    assert_eq!(
        p.route_check(s[0], StripId::MASTER, RouteKind::Send),
        Err(RouteRefusal::SendToMaster)
    );
    assert_eq!(
        p.route_check(StripId::MASTER, s[0], RouteKind::Send),
        Err(RouteRefusal::FromMaster)
    );
    assert_eq!(
        p.route_check(s[0], s[1], RouteKind::Output),
        Err(RouteRefusal::AlreadyThere)
    );
    assert_eq!(
        p.route_check(s[3], StripId::MASTER, RouteKind::Output),
        Err(RouteRefusal::AlreadyThere)
    );
    for r in [
        RouteRefusal::ToItself,
        RouteRefusal::Loop { through: s[0] },
        RouteRefusal::NoRoom,
        RouteRefusal::NotKeyable,
        RouteRefusal::AlreadyThere,
        RouteRefusal::FromMaster,
        RouteRefusal::SendToMaster,
        RouteRefusal::UnknownStrip(s[0]),
        RouteRefusal::UnknownEffect(EffectId(1)),
    ] {
        assert!(r.label_key().starts_with("routing.refusal."));
    }
}

#[test]
fn key_choices_are_the_inserts_that_can_be_keyed_now() {
    let (mut p, s) = desk(3);
    let comp = p.make_effect(EffectKind::Compressor);
    let delay = p.make_effect(EffectKind::Delay);
    let gate = p.make_effect(EffectKind::Gate);
    let ids = [comp.id, delay.id, gate.id];
    for (i, slot) in [comp, delay, gate].into_iter().enumerate() {
        apply(
            &mut p,
            Command::InsertEffect {
                strip: s[1],
                index: i,
                slot: Box::new(slot),
            },
        );
    }
    let gate_keys = EffectKind::Gate.takes_sidechain();
    let want: Vec<EffectId> = if gate_keys {
        vec![ids[0], ids[2]]
    } else {
        vec![ids[0]]
    };
    assert_eq!(
        routing::key_choices(&p, s[0], s[1]),
        want,
        "the delay takes no key"
    );
    apply(
        &mut p,
        Command::SetEffectSidechain {
            strip: s[1],
            effect: ids[0],
            source: Some(s[0]),
        },
    );
    assert!(
        !routing::key_choices(&p, s[0], s[1]).contains(&ids[0]),
        "keyed already"
    );
    assert!(routing::key_choices(&p, s[1], s[1]).is_empty(), "itself");
}

// ---- the batches ----------------------------------------------------------------------

#[test]
fn a_plain_gesture_adds_unity_post_sends_from_every_source() {
    let (mut p, s) = desk(5);
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[4],
            output: Some(s[0]),
        },
    );
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[1],
            send: Send::new(s[3]),
        },
    );
    // Into 4: 1 (fine), 2 (already sends there), 4 (itself), 5 (fine: its output to 1
    // has nothing to do with 4) and 1 again (once is enough).
    let cmd = plan::add_sends(&p, &[s[0], s[1], s[3], s[4], s[0]], s[3]).unwrap();
    assert_eq!(label(&cmd), "cmd.add_send");
    let after = applies_and_undoes(&p, cmd);
    for from in [s[0], s[4]] {
        assert_eq!(
            after.strip(from).unwrap().send_to(s[3]),
            Some(&Send::new(s[3]))
        );
    }
    assert_eq!(after.strip(s[1]).unwrap().sends.len(), 1);
    assert!(
        plan::add_sends(&p, &[s[3]], s[3]).is_none(),
        "nothing to do: no command"
    );
}

#[test]
fn ctrl_replaces_the_output_and_leaves_the_sends_alone() {
    let (mut p, s) = desk(4);
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[0],
            output: Some(s[1]),
        },
    );
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[0],
            send: Send::new(s[2]),
        },
    );
    let cmd = plan::replace_outputs(&p, &[s[0]], s[2]).unwrap();
    assert_eq!(label(&cmd), "cmd.replace_output");
    let after = applies_and_undoes(&p, cmd);
    let a = after.strip(s[0]).unwrap();
    assert_eq!(a.output, Some(s[2]), "the destination is the only output");
    assert_eq!(
        a.sends,
        vec![Send::new(s[2])],
        "sends untouched (now doubled, flagged)"
    );
    assert!(routing::doubled(&after, s[0], s[2]));
    let back = plan::replace_outputs(&after, &[s[0]], StripId::MASTER).unwrap();
    assert_eq!(
        applies_and_undoes(&after, back).strip(s[0]).unwrap().output,
        None
    );
    assert!(
        plan::replace_outputs(&p, &[s[0]], s[1]).is_none(),
        "already there"
    );
}

#[test]
fn alt_keys_an_insert_or_brings_a_keyed_one() {
    let (mut p, s) = desk(3);
    let comp = p.make_effect(EffectKind::Compressor);
    let cid = comp.id;
    apply(
        &mut p,
        Command::InsertEffect {
            strip: s[1],
            index: 0,
            slot: Box::new(comp),
        },
    );
    let cmd = plan::key(&p, s[0], s[1], cid).unwrap();
    assert_eq!(label(&cmd), "cmd.key_sidechain");
    let after = applies_and_undoes(&p, cmd);
    assert_eq!(after.strip(s[1]).unwrap().effects[0].sidechain, Some(s[0]));
    assert!(plan::key(&p, s[1], s[1], cid).is_none(), "itself");
    let mut q = p.clone();
    let gate = q.make_effect(EffectKind::Compressor);
    let gid = gate.id;
    let cmd = plan::key_with(&q, s[0], s[2], gate, "cmd.gate_key").unwrap();
    assert_eq!(label(&cmd), "cmd.gate_key");
    let after = applies_and_undoes(&q, cmd);
    let e = after.strip(s[2]).unwrap().effect(gid).unwrap();
    assert_eq!(e.sidechain, Some(s[0]));
    let delay = q.make_effect(EffectKind::Delay);
    assert!(
        plan::key_with(&q, s[0], s[2], delay, "x").is_none(),
        "a delay takes no key"
    );
}

#[test]
fn moving_a_sends_end_keeps_its_place_level_switch_and_tap() {
    let (mut p, s) = desk(5);
    for to in [s[1], s[2], s[3]] {
        apply(
            &mut p,
            Command::SetStripSend {
                id: s[0],
                send: Send::new(to),
            },
        );
    }
    let mut send = Send::new(s[2]);
    send.level = 0.3;
    send.enabled = false;
    send.tap = SendTap::Pre;
    apply(&mut p, Command::SetStripSend { id: s[0], send });
    let route = Route::send(s[0], s[2]);
    let cmd = plan::move_route(&p, route, End::Destination, s[4]).unwrap();
    assert_eq!(label(&cmd), "cmd.move_route");
    let after = applies_and_undoes(&p, cmd);
    let sends = &after.strip(s[0]).unwrap().sends;
    assert_eq!(
        sends.iter().map(|x| x.to).collect::<Vec<_>>(),
        vec![s[1], s[4], s[3]]
    );
    assert_eq!(sends[1], Send { to: s[4], ..send });
    assert!(
        plan::move_route(&p, route, End::Destination, s[1]).is_none(),
        "already sends there"
    );
    assert!(
        plan::move_route(&p, route, End::Destination, s[0]).is_none(),
        "itself"
    );
    // The source end: the same send, from another strip.
    let cmd = plan::move_route(&p, route, End::Source, s[4]).unwrap();
    let after = applies_and_undoes(&p, cmd);
    assert!(after.strip(s[0]).unwrap().send_to(s[2]).is_none());
    assert_eq!(after.strip(s[4]).unwrap().send_to(s[2]), Some(&send));
    // An output's end, and a key's source.
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[1],
            output: Some(s[3]),
        },
    );
    let cmd = plan::move_route(&p, Route::output(s[1], s[3]), End::Destination, s[4]).unwrap();
    assert_eq!(
        applies_and_undoes(&p, cmd).strip(s[1]).unwrap().output,
        Some(s[4])
    );
    let comp = p.make_effect(EffectKind::Compressor);
    let cid = comp.id;
    apply(
        &mut p,
        Command::InsertEffect {
            strip: s[3],
            index: 0,
            slot: Box::new(comp),
        },
    );
    apply(
        &mut p,
        Command::SetEffectSidechain {
            strip: s[3],
            effect: cid,
            source: Some(s[4]),
        },
    );
    let key = Route::key(s[4], s[3], cid);
    let cmd = plan::move_route(&p, key, End::Source, s[2]).unwrap();
    assert_eq!(
        applies_and_undoes(&p, cmd).strip(s[3]).unwrap().effects[0].sidechain,
        Some(s[2])
    );
    assert!(
        plan::move_route(&p, key, End::Destination, s[2]).is_none(),
        "a key stays on its insert"
    );
}

#[test]
fn removing_a_route_and_converting_sends_and_outputs() {
    let (mut p, s) = desk(4);
    apply(
        &mut p,
        Command::SetStripOutput {
            id: s[0],
            output: Some(s[1]),
        },
    );
    let mut send = Send::new(s[2]);
    send.level = 0.5;
    apply(&mut p, Command::SetStripSend { id: s[0], send });
    let comp = p.make_effect(EffectKind::Compressor);
    let cid = comp.id;
    apply(
        &mut p,
        Command::InsertEffect {
            strip: s[3],
            index: 0,
            slot: Box::new(comp),
        },
    );
    apply(
        &mut p,
        Command::SetEffectSidechain {
            strip: s[3],
            effect: cid,
            source: Some(s[0]),
        },
    );
    for (route, check) in [
        (Route::output(s[0], s[1]), 0),
        (Route::send(s[0], s[2]), 1),
        (Route::key(s[0], s[3], cid), 2),
    ] {
        let cmd = plan::remove_route(&p, route).unwrap();
        assert_eq!(label(&cmd), "cmd.remove_route");
        let after = applies_and_undoes(&p, cmd);
        let a = after.strip(s[0]).unwrap();
        match check {
            0 => assert_eq!(a.output, None),
            1 => assert!(a.sends.is_empty()),
            _ => assert_eq!(after.strip(s[3]).unwrap().effects[0].sidechain, None),
        }
    }
    assert!(plan::remove_route(&p, Route::output(s[2], StripId::MASTER)).is_none());
    // Send → output: the output moves there, the send goes.
    let cmd = plan::send_to_output(&p, s[0], s[2]).unwrap();
    assert_eq!(label(&cmd), "cmd.send_to_output");
    let after = applies_and_undoes(&p, cmd);
    let a = after.strip(s[0]).unwrap();
    assert_eq!((a.output, a.sends.len()), (Some(s[2]), 0));
    // Output → send: back to the master, and a unity send where the output went.
    let cmd = plan::output_to_send(&p, s[0]).unwrap();
    assert_eq!(label(&cmd), "cmd.output_to_send");
    let after = applies_and_undoes(&p, cmd);
    let a = after.strip(s[0]).unwrap();
    assert_eq!(a.output, None);
    assert_eq!(a.send_to(s[1]), Some(&Send::new(s[1])));
    assert!(
        plan::output_to_send(&after, s[0]).is_none(),
        "the master: nothing to convert"
    );
}

#[test]
fn a_new_return_with_its_effect_and_the_sends_is_one_step() {
    let (mut p, s) = desk(3);
    let mut ret = p.make_strip("Reverb");
    ret.dock = Dock::Middle;
    let rid = ret.id;
    let mut verb = p.make_effect(EffectKind::Space);
    verb.spec.set("mix", 1.0);
    let cmd = plan::new_return(&p, &[s[0], s[1], StripId::MASTER], ret, Some(verb)).unwrap();
    assert_eq!(label(&cmd), "cmd.new_return");
    let after = applies_and_undoes(&p, cmd);
    let r = after.strip(rid).unwrap();
    assert_eq!(r.dock, Dock::Right, "returns live on the right");
    assert_eq!(after.strips.last().unwrap().id, rid);
    assert_eq!(r.effects.len(), 1);
    assert_eq!(
        after.strip(s[0]).unwrap().send_to(rid),
        Some(&Send::new(rid))
    );
    assert_eq!(
        after.strip(s[1]).unwrap().send_to(rid),
        Some(&Send::new(rid))
    );
    let empty = p.make_strip("Return");
    assert!(
        plan::new_return(&p, &[], empty, None).is_some(),
        "a return alone"
    );
}

#[test]
fn tracks_route_onto_a_strip_and_a_tap_switches() {
    let (mut p, s) = desk(3);
    let a = p.make_track("A");
    let b = p.make_track("B");
    let (ta, tb) = (a.id, b.id);
    p.tracks.push(a);
    p.tracks.push(b);
    let cmd = plan::route_tracks(&p, &[ta, tb], Some(s[2])).unwrap();
    assert_eq!(label(&cmd), "cmd.route_tracks");
    let after = applies_and_undoes(&p, cmd);
    assert!(after.tracks.iter().all(|t| t.strip == Some(s[2])));
    assert!(plan::route_tracks(&after, &[ta], Some(s[2])).is_none());
    assert!(plan::route_tracks(&p, &[ta], Some(StripId(31_337))).is_none());
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[0],
            send: Send::new(s[1]),
        },
    );
    let cmd = plan::set_send_tap(&p, s[0], s[1], SendTap::Pre).unwrap();
    assert_eq!(label(&cmd), "cmd.set_send_tap");
    let after = applies_and_undoes(&p, cmd);
    assert_eq!(after.strip(s[0]).unwrap().sends[0].tap, SendTap::Pre);
    assert!(
        plan::set_send_tap(&p, s[0], s[1], SendTap::Post).is_none(),
        "already post"
    );
    assert!(
        plan::set_send_tap(&p, s[0], s[2], SendTap::Pre).is_none(),
        "no such send"
    );
}

// ---- a moved send keeps its automation (integration finding) ---------------------------

/// A project whose strip `s[0]` sends to `s[2]`, that send ridden by an automation clip on
/// the first track and by a linked copy of it.
fn ridden_send() -> (Project, Vec<StripId>, TrackId, [ClipId; 2]) {
    let (mut p, s) = desk(6);
    apply(
        &mut p,
        Command::SetStripSend {
            id: s[0],
            send: Send::new(s[2]),
        },
    );
    let t = p.make_track("Rides");
    p.tracks.push(t);
    let target = automation::AutoTarget::Send {
        strip: s[0],
        to: s[2],
    };
    let points = vec![
        automation::AutoPoint::new(Ticks(0), 0.2),
        automation::AutoPoint::new(Ticks(960), 0.9),
    ];
    let mut a = p.make_automation_clip("ride", target, Ticks(0), Ticks(1920), points);
    a.link = Some(LinkId(p.alloc_id()));
    let mut b = a.clone();
    b.id = ClipId(p.alloc_id());
    b.start = Ticks(3840);
    let track = p.tracks[0].id;
    let ids = [a.id, b.id];
    p.tracks[0].clips.push(a);
    p.tracks[0].clips.push(b);
    (p, s, track, ids)
}

fn targets(p: &Project, track: TrackId) -> Vec<automation::AutoTarget> {
    p.track(track)
        .unwrap()
        .clips
        .iter()
        .filter_map(|c| c.automation.as_ref().map(|a| a.target.clone()))
        .collect()
}

#[test]
fn a_moved_send_takes_its_automation_along() {
    let (p, s, track, _) = ridden_send();
    // Its destination end moved to s[4]: the clips ride Send { s[0] → s[4] }.
    let cmd = plan::move_route(&p, Route::send(s[0], s[2]), End::Destination, s[4]).unwrap();
    let after = applies_and_undoes(&p, cmd);
    let want = automation::AutoTarget::Send {
        strip: s[0],
        to: s[4],
    };
    assert_eq!(targets(&after, track), vec![want.clone(), want]);
    // Its source end moved to s[1]: Send { s[1] → s[2] }.
    let cmd = plan::move_route(&p, Route::send(s[0], s[2]), End::Source, s[1]).unwrap();
    let after = applies_and_undoes(&p, cmd);
    let want = automation::AutoTarget::Send {
        strip: s[1],
        to: s[2],
    };
    assert_eq!(targets(&after, track), vec![want.clone(), want]);
    assert!(links::check(&after).is_ok(), "the linked copies stay equal");
}

/// *Make this the output* takes the send away; its automation has no level to ride on an
/// output (one output, at unity), so the clip waits, unchanged, and rides again when the
/// output is made a send once more.
#[test]
fn make_this_the_output_then_a_send_again_gives_the_automation_back() {
    let (p, s, track, _) = ridden_send();
    let mut q = applies_and_undoes(&p, plan::send_to_output(&p, s[0], s[2]).unwrap());
    let target = automation::AutoTarget::Send {
        strip: s[0],
        to: s[2],
    };
    assert_eq!(targets(&q, track), vec![target.clone(), target.clone()]);
    assert!(
        automation::range::range_of(&q, &target).is_none(),
        "nothing to ride"
    );
    let back = plan::output_to_send(&q, s[0]).unwrap();
    apply(&mut q, back);
    assert!(
        automation::range::range_of(&q, &target).is_some(),
        "it rides again"
    );
}

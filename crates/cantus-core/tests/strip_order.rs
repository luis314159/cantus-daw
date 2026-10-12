//! Round 25 (the owner's answer 12): strips and groups move along the desk — a strip or
//! the selection dragged to a place, a group moved as one block, Alt+←/→ moving the
//! selection or the group — each one step of undo, docks respected, and a group's run
//! never split by accident.

use cantus_core::routing::order::{self, Spot};
use cantus_core::*;

/// The default desk: sixteen strips, the last three docked on the right; a group G of
/// strips 3–5 (indices 2..=4) and a group H of strips 9–10 (indices 8, 9).
fn desk() -> (Project, Vec<StripId>, GroupId, GroupId) {
    let mut p = Project::default();
    let s: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
    assert_eq!(p.strips[13].dock, Dock::Right, "the default right dock");
    let g = p.make_strip_group("G", Color::PALETTE[1]);
    let h = p.make_strip_group("H", Color::PALETTE[2]);
    let (gid, hid) = (g.id, h.id);
    p.strip_groups.push(g);
    p.strip_groups.push(h);
    for i in 2..=4 {
        p.strips[i].group = Some(gid);
    }
    for i in 8..=9 {
        p.strips[i].group = Some(hid);
    }
    (p, s, gid, hid)
}

/// The desk order of a dock, as strip numbers of the original desk (1-based).
fn dock(p: &Project, s: &[StripId], d: Dock) -> Vec<usize> {
    p.strips
        .iter()
        .filter(|x| x.dock == d)
        .map(|x| s.iter().position(|id| *id == x.id).unwrap() + 1)
        .collect()
}

fn run(p: &Project, cmd: Option<Command>, label: &str) -> Project {
    let cmd = cmd.expect("a command");
    match &cmd {
        Command::Batch { label_key, .. } => assert_eq!(label_key, label),
        other => panic!("not a batch: {other:?}"),
    }
    let mut bus = CommandBus::new(p.clone());
    bus.execute(cmd, Origin::User).expect("applies");
    let after = bus.project().clone();
    bus.undo();
    assert!(
        bus.project() == p,
        "one undo step restores the desk exactly"
    );
    after
}

fn group_of(p: &Project, id: StripId) -> Option<GroupId> {
    p.strip(id).unwrap().group
}

const MID: Dock = Dock::Middle;

#[test]
fn a_strip_moves_before_another_in_one_step() {
    let (p, s, ..) = desk();
    let after = run(
        &p,
        order::move_strips(&p, &[s[5]], Spot::before(MID, s[1])),
        "cmd.move_strips",
    );
    assert_eq!(dock(&after, &s, MID)[..7], [1, 6, 2, 3, 4, 5, 7]);
    assert!(
        order::move_strips(&p, &[s[5]], Spot::before(MID, s[6])).is_none(),
        "already there"
    );
}

#[test]
fn a_strip_moves_into_another_dock() {
    let (p, s, ..) = desk();
    let after = run(
        &p,
        order::move_strips(&p, &[s[1]], Spot::end(Dock::Right)),
        "cmd.move_strips",
    );
    assert_eq!(dock(&after, &s, Dock::Right), vec![14, 15, 16, 2]);
    assert_eq!(after.strip(s[1]).unwrap().dock, Dock::Right);
    let left = run(
        &p,
        order::move_strips(&p, &[s[6]], Spot::end(Dock::Left)),
        "cmd.move_strips",
    );
    assert_eq!(dock(&left, &s, Dock::Left), vec![7]);
}

#[test]
fn the_selection_moves_as_one_block_in_its_order() {
    let (p, s, ..) = desk();
    let sel = [s[6], s[0]];
    let after = run(
        &p,
        order::move_strips(&p, &sel, Spot::before(MID, s[11])),
        "cmd.move_strips",
    );
    let d = dock(&after, &s, MID);
    let at = d.iter().position(|n| *n == 1).unwrap();
    assert_eq!(
        d[at..at + 3],
        [1, 7, 12],
        "desk order kept inside the block"
    );
}

#[test]
fn a_moved_strip_joins_a_group_only_when_dropped_inside_its_run() {
    let (p, s, g, _) = desk();
    let inside = run(
        &p,
        order::move_strips(&p, &[s[7]], Spot::before(MID, s[3])),
        "cmd.move_strips",
    );
    assert_eq!(
        group_of(&inside, s[7]),
        Some(g),
        "between two members: joins"
    );
    let edge = run(
        &p,
        order::move_strips(&p, &[s[7]], Spot::before(MID, s[2])),
        "cmd.move_strips",
    );
    assert_eq!(
        group_of(&edge, s[7]),
        None,
        "at the run's edge: stays loose"
    );
    let out = run(
        &p,
        order::move_strips(&p, &[s[3]], Spot::before(MID, s[10])),
        "cmd.move_strips",
    );
    assert_eq!(
        group_of(&out, s[3]),
        None,
        "a member dragged away leaves its group"
    );
    let within = run(
        &p,
        order::move_strips(&p, &[s[4]], Spot::before(MID, s[2])),
        "cmd.move_strips",
    );
    assert_eq!(
        group_of(&within, s[4]),
        Some(g),
        "reordered inside its run: stays"
    );
    assert_eq!(dock(&within, &s, MID)[1..5], [2, 5, 3, 4]);
}

#[test]
fn a_group_moves_as_one_block_and_never_splits_another() {
    let (p, s, g, _) = desk();
    let after = run(
        &p,
        order::move_group(&p, g, Spot::before(MID, s[11])),
        "cmd.move_group",
    );
    let d = dock(&after, &s, MID);
    let at = d.iter().position(|n| *n == 3).unwrap();
    assert_eq!(d[at..at + 4], [3, 4, 5, 12]);
    assert!((2..=4).all(|i| group_of(&after, s[i]) == Some(g)));
    // Dropped inside H's run (before strip 10), it lands before H instead.
    let snapped = run(
        &p,
        order::move_group(&p, g, Spot::before(MID, s[9])),
        "cmd.move_group",
    );
    let d = dock(&snapped, &s, MID);
    let at = d.iter().position(|n| *n == 3).unwrap();
    assert_eq!(d[at..at + 5], [3, 4, 5, 9, 10]);
    // To the right dock, every member goes.
    let right = run(
        &p,
        order::move_group(&p, g, Spot::end(Dock::Right)),
        "cmd.move_group",
    );
    assert_eq!(dock(&right, &s, Dock::Right), vec![14, 15, 16, 3, 4, 5]);
    assert!(
        order::move_group(&p, g, Spot::before(MID, s[3])).is_none(),
        "onto itself"
    );
}

#[test]
fn alt_arrows_move_a_strip_one_place_within_its_dock() {
    let (p, s, ..) = desk();
    let after = run(&p, order::nudge(&p, &[s[5]], 1), "cmd.move_strips");
    assert_eq!(dock(&after, &s, MID)[5..7], [7, 6]);
    assert!(
        order::nudge(&p, &[s[0]], -1).is_none(),
        "the dock's first strip"
    );
    assert!(
        order::nudge(&p, &[s[13]], -1).is_none(),
        "docks are respected"
    );
    assert!(
        order::nudge(&p, &[s[12]], 1).is_none(),
        "the middle's last strip"
    );
}

#[test]
fn alt_arrows_hop_over_a_group_and_step_out_of_one() {
    let (p, s, g, _) = desk();
    // A loose strip beside a group hops over the whole run, staying loose.
    let hop = run(&p, order::nudge(&p, &[s[1]], 1), "cmd.move_strips");
    assert_eq!(dock(&hop, &s, MID)[1..5], [3, 4, 5, 2]);
    assert_eq!(group_of(&hop, s[1]), None);
    // A member inside its run swaps with its neighbour.
    let inside = run(&p, order::nudge(&p, &[s[2]], 1), "cmd.move_strips");
    assert_eq!(dock(&inside, &s, MID)[2..5], [4, 3, 5]);
    assert_eq!(group_of(&inside, s[2]), Some(g));
    // A member at the run's edge steps out of the group where it stands.
    let out = run(&p, order::nudge(&p, &[s[4]], 1), "cmd.move_strips");
    assert_eq!(dock(&out, &s, MID), dock(&p, &s, MID));
    assert_eq!(group_of(&out, s[4]), None);
}

#[test]
fn alt_arrows_move_a_selection_and_a_whole_group() {
    let (p, s, g, h) = desk();
    let after = run(&p, order::nudge(&p, &[s[6], s[5]], 1), "cmd.move_strips");
    assert_eq!(dock(&after, &s, MID)[5..8], [8, 6, 7]);
    // The selection is all of G: the group moves, past the loose strip after it.
    let grp = run(
        &p,
        order::nudge(&p, &[s[3], s[2], s[4]], 1),
        "cmd.move_group",
    );
    assert_eq!(dock(&grp, &s, MID)[1..6], [2, 6, 3, 4, 5]);
    assert!((2..=4).all(|i| group_of(&grp, s[i]) == Some(g)));
    // A group hops over another group as one item.
    let past = run(&p, order::nudge_group(&p, h, -1), "cmd.move_group");
    assert_eq!(dock(&past, &s, MID)[6..10], [7, 9, 10, 8]);
    let hop = run(&p, order::nudge_group(&p, g, 1), "cmd.move_group");
    assert_eq!(dock(&hop, &s, MID)[1..6], [2, 6, 3, 4, 5]);
    let mut q = p.clone();
    for i in 5..=7 {
        q.strips[i].group = Some(h);
    }
    // H is now 6–10, right after G: G hops over all of it.
    let over = run(&q, order::nudge_group(&q, g, 1), "cmd.move_group");
    assert_eq!(dock(&over, &s, MID)[1..10], [2, 6, 7, 8, 9, 10, 3, 4, 5]);
}

/// Integration finding: a selection mixing a loose strip and a member of a group, dropped
/// right after the group's other member, used to land as `[9(H), 2, 10(H), 11]` — the loose
/// strip splitting H. The block is ordered so the members that stay in the left
/// neighbour's group come first and those in the right neighbour's group last.
#[test]
fn a_mixed_selection_never_splits_the_group_it_lands_beside() {
    let (p, s, _, h) = desk();
    // Loose strip 2 and H's member 10, dropped before 11 (right after H's 9).
    let after = run(
        &p,
        order::move_strips(&p, &[s[1], s[9]], Spot::before(MID, s[10])),
        "cmd.move_strips",
    );
    let mid = dock(&after, &s, MID);
    let at9 = mid.iter().position(|n| *n == 9).unwrap();
    assert_eq!(&mid[at9..at9 + 4], &[9, 10, 2, 11], "{mid:?}");
    assert_eq!(group_of(&after, s[9]), Some(h), "10 stays beside 9 in H");
    assert_eq!(group_of(&after, s[1]), None);
    contiguous(&after);
}

/// Every group's members are one run in each dock.
fn contiguous(p: &Project) {
    for d in Dock::ALL {
        let seq: Vec<Option<GroupId>> = p
            .strips
            .iter()
            .filter(|x| x.dock == d)
            .map(|x| x.group.filter(|g| p.strip_group(*g).is_some()))
            .collect();
        let mut closed: Vec<GroupId> = Vec::new();
        let mut prev: Option<GroupId> = None;
        for g in &seq {
            if *g != prev {
                if let Some(pg) = prev {
                    closed.push(pg);
                }
                if let Some(ng) = g {
                    assert!(!closed.contains(ng), "group {ng:?} split in {d:?}: {seq:?}");
                }
            }
            prev = *g;
        }
    }
}

/// A small deterministic generator (xorshift64*), so the search is the same every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Property: on desks whose groups are each one run per dock, no strip move, selection
/// move or group move splits a group — whatever is selected and wherever it is dropped.
#[test]
fn no_move_splits_a_group_by_accident() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut moves = 0;
    for _ in 0..3000 {
        let mut p = Project::default();
        let s: Vec<StripId> = p.strips.iter().map(|x| x.id).collect();
        // Up to three groups, each a run inside one dock.
        let mut taken = vec![false; s.len()];
        let mut groups = Vec::new();
        for k in 0..1 + rng.below(3) {
            let len = 2 + rng.below(3);
            let start = rng.below(s.len() - len);
            let dock = p.strips[start].dock;
            let run: Vec<usize> = (start..start + len).collect();
            if run.iter().any(|i| taken[*i] || p.strips[*i].dock != dock) {
                continue;
            }
            let g = p.make_strip_group(format!("G{k}"), Color::PALETTE[k + 1]);
            let gid = g.id;
            p.strip_groups.push(g);
            for i in run {
                taken[i] = true;
                p.strips[i].group = Some(gid);
            }
            groups.push(gid);
        }
        contiguous(&p);
        // A selection of one to four strips anywhere, dropped anywhere in any dock.
        let n = 1 + rng.below(4);
        let sel: Vec<StripId> = (0..n).map(|_| s[rng.below(s.len())]).collect();
        let dock = Dock::ALL[rng.below(3)];
        let in_dock: Vec<StripId> = p
            .strips
            .iter()
            .filter(|x| x.dock == dock)
            .map(|x| x.id)
            .collect();
        let spot = match rng.below(in_dock.len() + 1) {
            k if k == in_dock.len() => Spot::end(dock),
            k => Spot::before(dock, in_dock[k]),
        };
        if let Some(cmd) = order::move_strips(&p, &sel, spot) {
            let after = run(&p, Some(cmd), "cmd.move_strips");
            contiguous(&after);
            moves += 1;
        }
        if let Some(g) = groups.first() {
            if let Some(cmd) = order::move_group(&p, *g, spot) {
                let after = run(&p, Some(cmd), "cmd.move_group");
                contiguous(&after);
                moves += 1;
            }
        }
        for delta in [-2, -1, 1, 2] {
            if let Some(cmd) = order::nudge(&p, &sel, delta) {
                let label = match &cmd {
                    Command::Batch { label_key, .. } => label_key.clone(),
                    _ => String::new(),
                };
                let after = run(&p, Some(cmd), &label);
                contiguous(&after);
                moves += 1;
            }
        }
    }
    assert!(moves > 4000, "the search made {moves} moves");
}

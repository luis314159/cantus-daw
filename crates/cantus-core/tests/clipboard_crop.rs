//! The time-range crop (ADR 0036): what a copy or a cut of `[from, to)` takes out of every
//! track, checked against what is *heard* — the compiled automation and the snapshot's
//! notes — so a crop that leaves a clip "the same shape" but playing something else fails.

use cantus_core::automation::compile::lane_for;
use cantus_core::clipboard::{self, ClipOrigin, Clipboard, Crop};
use cantus_core::*;
use std::collections::HashMap;

fn ids_of(p: &Project) -> impl FnMut() -> u64 {
    let mut next = p.next_id;
    move || {
        let id = next;
        next += 1;
        id
    }
}

fn normalised(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

/// A gain automation clip, 4 bars long, looping a 1-bar brace. Its curve also has a point
/// *past* the brace (beat 6), which the loop never plays: a crop that reads past the
/// brace would hear it.
fn looping_automation() -> (Project, TrackId, AutoTarget) {
    let mut p = Project::empty("auto");
    let t = p.make_track("t");
    let tid = t.id;
    p.tracks.push(t);
    let target = AutoTarget::TrackGain(tid);
    let mut c = p.make_automation_clip(
        "gain",
        target.clone(),
        Ticks::ZERO,
        Ticks::beats(16),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.0),
            AutoPoint::new(Ticks::beats(2), 1.0),
            AutoPoint::new(Ticks::beats(3), 0.25),
            AutoPoint::new(Ticks::beats(6), 0.9),
        ],
    );
    c.content_length = Some(Ticks::beats(4));
    p.tracks[0].clips.push(c);
    (p, tid, target)
}

/// The lane's value every 1/8 beat over `[0, end)`, with where it was read.
fn curve(p: &Project, target: &AutoTarget, end: Ticks) -> Vec<(i64, f32)> {
    let lane = lane_for(p, target).expect("the target is automated");
    (0..end.0)
        .step_by((PPQ / 8) as usize)
        .map(|t| (t, lane.value_at(t as f64)))
        .collect()
}

fn cut(p: &Project, from: Ticks, to: Ticks) -> (Vec<Crop>, Vec<Command>) {
    let mut ids = ids_of(p);
    let crops = clipboard::time_crops(p, from, to, &mut ids);
    let cmds = crops.iter().flat_map(|c| c.cut.clone()).collect();
    (crops, cmds)
}

fn run(bus: &mut CommandBus, label: &str, commands: Vec<Command>) {
    bus.execute(
        Command::Batch {
            label_key: label.into(),
            commands,
        },
        Origin::User,
    )
    .expect("the batch applies");
}

#[test]
fn a_time_cut_through_a_looping_automation_clip_leaves_the_curve_outside_it_unchanged() {
    let (p, _, target) = looping_automation();
    let end = Ticks::beats(16);
    let before = curve(&p, &target, end);
    // Neither edge falls on a repeat: beat 5 is one beat into the 2nd loop, 11 three
    // beats into the 3rd.
    let (from, to) = (Ticks::beats(5), Ticks::beats(11));
    let (crops, cmds) = cut(&p, from, to);
    let mut bus = CommandBus::new(p.clone());
    run(&mut bus, "cmd.cut_time", cmds);
    let outside = |t: i64| t < from.0 || t >= to.0;
    let after = curve(bus.project(), &target, end);
    for (b, a) in before.iter().zip(&after) {
        if outside(b.0) {
            assert!(
                (b.1 - a.1).abs() < 1e-6,
                "tick {}: {} before the cut, {} after",
                b.0,
                b.1,
                a.1
            );
        }
    }
    for (_, c) in bus.project().tracks[0].clips.iter().map(|c| (0, c)) {
        assert!(
            c.end() <= from || c.start >= to,
            "a clip still covers the cut range: {}..{}",
            c.start.0,
            c.end().0
        );
    }

    // The pieces that were cut out, pasted back where they were: the whole curve is back.
    let picked = crops
        .into_iter()
        .flat_map(|c| {
            let (index, track) = (c.index, c.track);
            c.pieces.into_iter().map(move |piece| (index, track, piece))
        })
        .collect();
    let board = Clipboard::new(
        ClipOrigin::default(),
        bus.project(),
        clipboard::clips_payload(picked, from, Some(to - from)),
    );
    let mut ids = ids_of(bus.project());
    let out = clipboard::paste_clips(
        bus.project(),
        &board,
        true,
        0,
        from,
        &HashMap::new(),
        false,
        &mut ids,
    );
    assert_eq!(out.skipped_automation, 0);
    run(&mut bus, "cmd.paste_clips", out.commands);
    let back = curve(bus.project(), &target, end);
    for (b, a) in before.iter().zip(&back) {
        assert!(
            (b.1 - a.1).abs() < 1e-6,
            "tick {}: {} before, {} after cut and paste back",
            b.0,
            b.1,
            a.1
        );
    }
    bus.undo();
    bus.undo();
    assert_eq!(normalised(bus.project()), normalised(&p), "undo is exact");
}

#[test]
fn a_copy_of_a_range_inside_one_loop_is_one_clip_that_plays_what_was_heard() {
    let (p, tid, target) = looping_automation();
    let (from, to) = (Ticks::beats(5), Ticks::beats(7));
    let (crops, _) = cut(&p, from, to);
    assert_eq!(crops.len(), 1);
    assert_eq!(crops[0].track, tid);
    assert_eq!(
        crops[0].pieces.len(),
        1,
        "a range inside one repeat is one piece"
    );
    let piece = &crops[0].pieces[0];
    assert_eq!((piece.start, piece.length), (from, to - from));
    // Played on its own, the piece follows the loop's curve over the range.
    let mut alone = p.clone();
    alone.tracks[0].clips = vec![piece.clone()];
    let lane = lane_for(&p, &target).unwrap();
    let solo = lane_for(&alone, &target).unwrap();
    for t in (from.0..to.0).step_by((PPQ / 8) as usize) {
        let (x, y) = (lane.value_at(t as f64), solo.value_at(t as f64));
        assert!((x - y).abs() < 1e-6, "tick {t}: loop {x}, piece {y}");
    }
}

#[test]
fn a_range_on_clip_edges_takes_whole_clips_and_leaves_neighbours_alone() {
    let mut p = Project::empty("edges");
    let mut t = p.make_track("t");
    let a = p.make_clip("a", Ticks::ZERO, Ticks::beats(4));
    let b = p.make_clip("b", Ticks::beats(4), Ticks::beats(4));
    let c = p.make_clip("c", Ticks::beats(8), Ticks::beats(4));
    let (ia, ib, ic) = (a.id, b.id, c.id);
    t.clips = vec![a, b, c];
    p.tracks.push(t);
    let (crops, cmds) = cut(&p, Ticks::beats(4), Ticks::beats(8));
    assert_eq!(crops.len(), 1, "only the clip inside the range");
    assert_eq!(crops[0].pieces[0].id, ib);
    let mut bus = CommandBus::new(p.clone());
    run(&mut bus, "cmd.cut_time", cmds);
    let left: Vec<ClipId> = bus.project().tracks[0].clips.iter().map(|c| c.id).collect();
    assert_eq!(left, vec![ia, ic]);
    assert_eq!(bus.project().tracks[0].clips[0].length, Ticks::beats(4));
    bus.undo();
    assert_eq!(normalised(bus.project()), normalised(&p));
}

//! The conductor's second round (the integrator's review of the timing round): a launch
//! pressed before a boundary it can no longer reach plays on it, caught up; the tolerance on
//! a fine grid; presses resolved in the order of their ticks; a follow time shortened below
//! what played waits for its next turn; a window's tempo given once.

use super::*;
use crate::session::edit::Performance;
use crate::session::play::{plan, plan_from, tolerance, Change, Input, Timed, LAUNCH_TOLERANCE};

fn launch(slot: SlotRef) -> Input {
    Input::Launch {
        slot,
        velocity: 1.0,
    }
}

/// The fixture's keys column: a two-bar loop (scene 0) and two one-bar loops (1, 2), and a
/// one-bar loop in the bass's first slot.
fn column() -> (CommandBus, Vec<TrackId>, Vec<SceneId>) {
    let (mut bus, tracks, scenes) = fixture();
    for slot in [
        SlotRef::new(tracks[0], scenes[1]),
        SlotRef::new(tracks[0], scenes[2]),
        SlotRef::new(tracks[1], scenes[0]),
    ] {
        let c = note_clip(&mut bus, 1);
        bus.execute(
            Command::Session(SessionCommand::InsertSlotClip {
                slot,
                clip: Box::new(c),
            }),
            Origin::User,
        )
        .unwrap();
    }
    (bus, tracks, scenes)
}

fn set_launch(bus: &mut CommandBus, slot: SlotRef, f: impl FnOnce(&mut LaunchSettings)) {
    let mut l = bus.project().slot_clip(slot).unwrap().launch;
    f(&mut l);
    bus.execute(
        Command::Session(SessionCommand::SetSlotLaunch { slot, launch: l }),
        Origin::User,
    )
    .unwrap();
}

fn set_follow(bus: &mut CommandBus, slot: SlotRef, after: FollowAfter, a: FollowAction) {
    let follow = FollowActions {
        on: true,
        a,
        b: FollowAction::None,
        chance_a: 100,
        after,
        count: 1,
    };
    bus.execute(
        Command::Session(SessionCommand::SetSlotFollow { slot, follow }),
        Origin::User,
    )
    .unwrap();
}

/// A press heard at `pressed`, taking effect no sooner than `at`.
fn heard(pressed: Ticks, at: Ticks, input: Input) -> Timed {
    Timed {
        at,
        now: false,
        input,
        pressed,
    }
}

/// N1: a launch pressed 10 ms before the bar line, with a lead longer than that, plays on
/// that bar caught up — not on the next one; and one whose compile overran (planned again
/// a beat past the bar it meant) plays on the bar it meant, caught up.
#[test]
fn a_launch_pressed_before_a_bar_it_cannot_reach_plays_on_it_caught_up() {
    let (bus, tracks, scenes) = column();
    let slot = SlotRef::new(tracks[0], scenes[1]);
    let ms = |s: f64| Ticks::from_seconds(s / 1000.0, 120.0);
    let pressed = BAR - ms(10.0);
    let at = pressed + ms(27.0);
    let p = plan(bus.project(), &[heard(pressed, at, launch(slot))], BAR * 4);
    let x = p.perf.played[0];
    assert_eq!((x.at, x.origin, x.from), (at, BAR, at - BAR), "on that bar");
    // An overrun: pressed a beat before the bar, planned again a beat after it.
    let pressed = BAR - Ticks::beats(1);
    let at = BAR + Ticks::beats(1);
    let p = plan(bus.project(), &[heard(pressed, at, launch(slot))], BAR * 4);
    let x = p.perf.played[0];
    assert_eq!((x.at, x.origin, x.from), (at, BAR, Ticks::beats(1)));
    // A stop pressed before a bar it cannot reach stops as soon as it can, not a bar on.
    let stop = Input::StopTrack { track: tracks[0] };
    let p = plan(
        bus.project(),
        &[
            Timed::at(Ticks::ZERO, launch(slot)),
            heard(BAR * 2 - ms(10.0), BAR * 2 + ms(17.0), stop),
        ],
        BAR * 4,
    );
    assert_eq!(p.perf.played.last().unwrap().until, BAR * 2 + ms(17.0));
}

/// Minor: on a launch grid of a 32nd or finer the tolerance is a quarter of the grid, so a
/// press half a step after a grid line is not caught up from it.
#[test]
fn the_tolerance_is_at_most_a_quarter_of_a_fine_grid() {
    let (mut bus, tracks, scenes) = column();
    let slot = SlotRef::new(tracks[0], scenes[1]);
    set_launch(&mut bus, slot, |l| l.quantize = Quantize::ThirtySecond);
    let step = Ticks(crate::time::PPQ / 8);
    assert_eq!(tolerance(QuantGrid::Ticks(step.0)), Ticks(step.0 / 4));
    assert_eq!(tolerance(QuantGrid::Bars(1)), LAUNCH_TOLERANCE);
    let pressed = BAR + Ticks(step.0 / 2 + 1);
    let at = pressed + Ticks(10);
    let p = plan(bus.project(), &[heard(pressed, at, launch(slot))], BAR * 4);
    let x = p.perf.played[0];
    assert_eq!((x.at, x.origin), (BAR + step, BAR + step), "the next step");
}

/// N2: a press stamped later in the list but sooner in time (a slot recording's launch is
/// laid a bar ahead, then a beat-quantised launch comes) is taken in its place: the plan is
/// the plan of the presses in tick order, every window ends after it starts.
#[test]
fn presses_are_taken_in_the_order_of_their_ticks() {
    let (mut bus, tracks, scenes) = column();
    let a = SlotRef::new(tracks[0], scenes[0]);
    let b = SlotRef::new(tracks[0], scenes[1]);
    let bass = SlotRef::new(tracks[1], scenes[0]);
    set_follow(
        &mut bus,
        a,
        FollowAfter::Time(Ticks::beats(1)),
        FollowAction::Again,
    );
    set_launch(&mut bus, b, |l| l.quantize = Quantize::Quarter);
    let pressed = [
        Timed::at(Ticks::ZERO, launch(a)),
        Timed {
            at: BAR * 2,
            now: true,
            input: launch(bass),
            pressed: BAR * 2,
        },
        Timed::at(BAR + Ticks::beats(1), launch(b)),
    ];
    let p = plan(bus.project(), &pressed, BAR * 4);
    let sorted = plan(
        bus.project(),
        &[pressed[0], pressed[2], pressed[1]],
        BAR * 4,
    );
    assert_eq!(p.perf, sorted.perf);
    assert_eq!(p.changes, sorted.changes);
    assert_eq!(
        p.effect,
        vec![sorted.effect[0], sorted.effect[2], sorted.effect[1]]
    );
    assert!(p.perf.played.iter().all(|x| x.at < x.until));
    assert!(p.changes.windows(2).all(|w| w[0].0 <= w[1].0));
    let x = p.perf.played.iter().find(|x| x.slot == b).unwrap();
    assert_eq!(x.at, BAR + Ticks::beats(1));
    assert!(p
        .perf
        .played
        .iter()
        .filter(|x| x.slot == a)
        .all(|x| x.until <= BAR + Ticks::beats(1)));
}

/// N8: a playing clip's follow time shortened below what already played follows the next
/// time it comes round, not at once mid-bar.
#[test]
fn a_follow_time_shortened_below_what_played_waits_for_its_next_turn() {
    let (mut bus, tracks, scenes) = column();
    let a = SlotRef::new(tracks[0], scenes[0]);
    set_follow(&mut bus, a, FollowAfter::Time(BAR * 8), FollowAction::Next);
    let inputs = [Timed::at(Ticks::ZERO, launch(a))];
    let before = plan(bus.project(), &inputs, BAR * 16);
    set_follow(&mut bus, a, FollowAfter::Time(BAR), FollowAction::Next);
    let cut = BAR * 3 + Ticks::beats(2);
    let after = plan_from(bus.project(), &inputs, BAR * 16, Some((&before, cut)));
    let followed = after
        .changes
        .iter()
        .find(|(t, c)| *t >= cut && matches!(c, Change::Followed { .. }))
        .map(|(t, _)| *t);
    assert_eq!(followed, Some(BAR * 4));
}

/// Minor: the tempo in force before a window and one set on its first tick are not both
/// given.
#[test]
fn a_window_gives_one_tempo_per_tick() {
    let perf = Performance {
        tempos: vec![(Ticks::ZERO, 100.0), (BAR, 140.0)],
        ..Performance::default()
    };
    let w = perf.within(BAR, BAR * 2, |_| None);
    assert_eq!(w.tempos, vec![(BAR, 140.0)]);
}

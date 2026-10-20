//! The render store's bookkeeping (round 29's second review): what a piece falls back on,
//! and what the budget keeps.

use super::*;

fn data(frames: usize) -> SampleData {
    SampleData {
        stream: None,
        rate: 48_000,
        channels: 1,
        frames: vec![0.0f32; frames].into(),
    }
}

const C: ClipId = ClipId(7);

/// Findings 20: a final asked for at an earlier drag step arriving after the bridges of
/// later steps is filed behind them — the piece falls back on the newest asked.
#[test]
fn a_late_final_never_comes_back_over_a_newer_bridge() {
    let mut r = Renders::default();
    let p = (C, Ticks::ZERO);
    // Step 1 asks K1, steps 2 and 3 ask K2 and K3 (their bridges arrive).
    for (g, k) in [(1, 1u64), (2, 2), (3, 3)] {
        r.generation = g;
        r.stamp.insert(k, g);
    }
    r.put_bridge(2, data(10));
    r.remember(p, 2);
    r.put_bridge(3, data(10));
    r.remember(p, 3);
    // K1's final lands last.
    r.put_done(1, data(10));
    r.remember(p, 1);
    assert_eq!(r.fallback(p, 48_000).map(|x| x.0), Some(3));
    assert_eq!(r.recent[&p], vec![3, 2, 1]);
}

/// Finding 20: a clip moved back to where it was edited before falls back on its newest
/// renders, not on what that place had.
#[test]
fn a_piece_moved_back_falls_back_on_its_clips_newest() {
    let mut r = Renders::default();
    let (s1, s2) = ((C, Ticks::ZERO), (C, Ticks::beats(8)));
    r.stamp.insert(1, 1);
    r.put_done(1, data(10));
    r.remember(s1, 1);
    r.stamp.insert(2, 5);
    r.put_done(2, data(10));
    r.remember(s2, 2);
    assert_eq!(r.fallback(s1, 48_000).map(|x| x.0), Some(2));
    // A loop's pieces asked together: each its own.
    r.stamp.insert(1, 5);
    assert_eq!(r.fallback(s1, 48_000).map(|x| x.0), Some(1));
    assert_eq!(r.fallback(s2, 48_000).map(|x| x.0), Some(2));
}

/// Findings 23, 42: past the budget the oldest asked go first; what plays (protected) is
/// kept whatever its age; what goes waits in the graveyard while a snapshot holds it.
#[test]
fn the_budget_keeps_what_plays_and_the_newest() {
    let mut r = Renders {
        budget: 3 * 400,
        ..Default::default()
    };
    let p = (C, Ticks::ZERO);
    for k in 1..=5u64 {
        r.stamp.insert(k, k);
        r.put_done(k, data(100));
        r.remember(p, k);
    }
    let playing = r.done[&1].clone();
    let protected: HashSet<u64> = [1].into_iter().collect();
    r.keep(&protected);
    let mut held: Vec<u64> = r.done.keys().copied().collect();
    held.sort();
    assert_eq!(held, vec![1, 3, 4, 5], "the oldest unplayed went");
    // Nothing protected now: 1 goes too, into the graveyard while the snapshot holds it.
    r.keep(&HashSet::new());
    assert!(!r.done.contains_key(&1));
    r.sweep();
    assert_eq!(
        r.graveyard.len(),
        1,
        "2 freed, 1 waits while the snapshot holds it"
    );
    drop(playing);
    r.sweep();
    assert!(r.graveyard.is_empty());
}

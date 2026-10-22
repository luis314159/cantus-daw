//! Review 2 of the tempo track (ADR 0060): a late unit's steps through a loop's wraps,
//! pass cuts only for the units that follow the song, one reach rule for a late step, a
//! watched Speculum's late frames at the tempo they were played at, the rendered range's
//! stop sample with a tail, and the transport's loop over a ramp's end.

#[path = "support/modfx.rs"]
mod modfx;
mod support;

use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::*;
use modfx::set_all;
use std::sync::Arc;
use support::{add_effect, demo_project};

const SR: f64 = 48_000.0;

fn pt(tick: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    }
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!(
            "{what}: first difference at sample {i} ({} vs {})",
            a[i], b[i]
        );
    }
}

/// Play `p` live from its start for `frames` in blocks of `block`, every effect and
/// instrument built as the application builds them (latencies compensated): the output
/// and the passes the processor ran.
fn live(p: &Project, frames: usize, block: usize) -> (Vec<f32>, u64) {
    let mut snap = Snapshot::compile(p, 1);
    snap.loop_region = if p.loop_enabled {
        p.loop_region.clone()
    } else {
        None
    };
    let snap = Arc::new(snap);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, SR as f32,
        ))))
        .ok();
    }
    for unit in cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
        .ok();
    let plan = cantus_engine::graph::compile(&snap);
    tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
    proc.settle();
    while let Ok(ev) = rx.pop() {
        match ev {
            EngineEvent::EffectNeed { effect, need } => {
                let memory = EffectMemory::build(need, SR as f32);
                tx.push(EngineMessage::EffectMemory { effect, memory }).ok();
                proc.settle();
            }
            EngineEvent::PdcNeed { count, delay } => {
                tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                    count, delay,
                )))
                .ok();
                proc.settle();
            }
            _ => {}
        }
    }
    let before = proc.passes_run();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    tx.push(EngineMessage::Play).ok();
    let mut out = Vec::with_capacity(frames * 2 + block * 2);
    let mut buf = vec![0f32; block * 2];
    while out.len() / 2 < frames {
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out.truncate(frames * 2);
    (out, proc.passes_run() - before)
}

/// The demo song, a ramp from 100 BPM at beat 4 to 140 at beat 12, a loop from beat 2 to a
/// tick in the ramp just past one of its steps, and a synced tremolo behind a limiter's
/// look-ahead: after each wrap the tremolo hears the end of the pass before.
fn looped_ramp() -> Project {
    let mut p = demo_project();
    p.tempo_bpm = 100.0;
    p.tempo_points = vec![
        pt(0, 100.0, false),
        pt(3840, 100.0, true),
        pt(11_520, 140.0, false),
    ];
    let lim = add_effect(&mut p, EffectKind::Limiter);
    set_all(&mut p, lim, &[("look", 5.0)]);
    let fx = add_effect(&mut p, EffectKind::Tremolo);
    set_all(
        &mut p,
        fx,
        &[("depth", 1.0), ("div", 8.0), ("shape", 0.0), ("edge", 0.0)],
    );
    // Steps every 60 ticks from beat 4: 6 900 is one; the loop ends 4 ticks after it.
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(2),
        end: Ticks(6_904),
    });
    p.loop_enabled = true;
    p
}

/// Findings 4 and 9: the cut at a late unit's step folds through the loop's wraps, as the
/// position it is told does — the same at every block size.
#[test]
fn a_late_unit_after_a_wrap_hears_the_steps_at_every_block_size() {
    let p = looped_ramp();
    let frames = 260_000;
    let (reference, _) = live(&p, frames, 64);
    for block in [1usize, 37, 512] {
        same_bits(
            &reference,
            &live(&p, frames, block).0,
            &format!("block {block}"),
        );
    }
}

/// Finding 10: an insert that does not follow the song (an equaliser) behind a latent one
/// adds no pass cuts at its own steps; one that follows it (a tremolo) does.
#[test]
fn only_the_units_that_follow_the_song_cut_passes_at_their_late_steps() {
    let base = || {
        let mut p = demo_project();
        p.tempo_points = vec![
            pt(0, 90.0, false),
            pt(960, 90.0, true),
            pt(15_360, 140.0, false),
        ];
        p.loop_enabled = false;
        p
    };
    let with = |latent: bool, kind: EffectKind| {
        let mut p = base();
        if latent {
            let lim = add_effect(&mut p, EffectKind::Limiter);
            set_all(&mut p, lim, &[("look", 5.0)]);
        }
        add_effect(&mut p, kind);
        live(&p, 200_000, 512).1
    };
    let plain = with(false, EffectKind::Eq);
    assert_eq!(
        with(true, EffectKind::Eq),
        plain,
        "an equaliser behind a look-ahead"
    );
    assert!(
        with(true, EffectKind::Tremolo) > with(false, EffectKind::Tremolo),
        "a tremolo behind one hears its own steps"
    );
}

/// Finding 11: the cut at a late unit's step and the step it is told agree to the sample —
/// at no lateness is the new step told a sample before the cut, at a slow tempo and a
/// high rate where a tick is a small fraction of a sample.
#[test]
fn a_late_step_is_reached_exactly_at_its_cut() {
    let sr = 192_000.0;
    let map = Arc::new(TempoMap::new(
        20.0,
        TimeSignature::default(),
        &[
            pt(0, 20.0, false),
            pt(960, 20.0, true),
            pt(9_600, 30.0, false),
        ],
        &[],
    ));
    let mut checked = 0;
    // Where a step falls between two samples depends on where the playhead was anchored:
    // many seeks give many fractions.
    for seek in (1_000..9_000).step_by(1) {
        let mut t = Transport::new(sr);
        t.set_map(map.clone());
        t.seek(Ticks(seek));
        t.play();
        for late in [700u64, 5_000] {
            let Some(k) = t.samples_until_step_late(late) else {
                continue;
            };
            if k < 2 {
                continue;
            }
            let mut a = t.clone();
            a.advance(k - 1);
            let heard = a.tick_before(a.position_f64(), late);
            let told = cantus_engine::fx::step_of(&map, heard, sr);
            let mut b = a.clone();
            b.advance(1);
            let heard_b = b.tick_before(b.position_f64(), late);
            let told_b = cantus_engine::fx::step_of(&map, heard_b, sr);
            assert!(
                told_b > told,
                "seek {seek}, late {late}: the step is reached at the cut ({k})"
            );
            checked += 1;
        }
    }
    assert!(checked > 12_000, "{checked}");
}

/// Finding 12: a watched Speculum behind a look-ahead stamps the audio it hears at the
/// tempo that audio was played at — across a point the transport has passed and the late
/// audio has not.
#[test]
fn a_late_speculum_stamps_its_frames_at_the_tempo_it_hears() {
    let mut p = modfx::lead_only();
    p.tempo_bpm = 128.0;
    p.tempo_points = vec![pt(0, 128.0, false), pt(240, 64.0, false)];
    let lim = add_effect(&mut p, EffectKind::Limiter);
    set_all(&mut p, lim, &[("look", 5.0)]);
    let id = add_effect(&mut p, EffectKind::Analyzer);
    let map = p.tempo_map();
    let (mut h, mut proc) = EngineHandle::manual(SR);
    let s = h.compile(&p, 1);
    h.set_snapshot(s);
    h.play();
    let mut buf = vec![0f32; 2 * 64];
    proc.process(&mut buf, 2);
    assert!(h.watch_scope(id));
    let mut stamps = Vec::new();
    let sizes = [256usize, 333, 1024, 96, 512, 17];
    for k in 0..40 {
        let n = sizes[k % sizes.len()];
        let mut buf = vec![0f32; 2 * n];
        proc.process(&mut buf, 2);
        h.drain_scope(id, |c: &cantus_engine::fx::speculum::ScopeChunk| {
            if let Some(b) = c.beat {
                stamps.push((b, c.tempo_bpm));
            }
        });
        h.watch_scope(id);
    }
    assert!(stamps.iter().any(|s| s.1 == 64.0), "the point was passed");
    for (beat, bpm) in &stamps {
        let want = map.bpm_at(beat * 960.0 + 1e-9);
        assert_eq!(*bpm, want, "a chunk heard at beat {beat}");
    }
}

/// Finding 28: the transport through a loop whose end lies inside a ramp — its position
/// after every wrap is the same at every block size — and a ramp in closed form: beat 6
/// of 90 BPM for a beat, a ramp to 140 over four, then 140, by hand.
#[test]
// A value by hand: std's own logarithm, apart from the code under test.
#[allow(clippy::disallowed_methods)]
fn the_transport_loops_over_a_ramps_end_and_meets_the_closed_form() {
    let map = Arc::new(TempoMap::new(
        90.0,
        TimeSignature::default(),
        &[
            pt(0, 90.0, false),
            pt(960, 90.0, true),
            pt(4_800, 140.0, false),
        ],
        &[],
    ));
    let by_hand = 60.0 / 90.0 + 60.0 * 4.0 / 50.0 * (140.0f64 / 90.0).ln() + 60.0 / 140.0;
    let mut t = Transport::new(SR);
    t.set_map(map.clone());
    t.seek(Ticks::ZERO);
    t.play();
    let want = (by_hand * SR).ceil() as u64;
    assert_eq!(t.samples_until(Ticks::beats(6).0 as f64), want);
    let run = |block: u64| {
        let mut t = Transport::new(SR);
        t.set_map(map.clone());
        t.set_loop(Some(LoopRegion {
            start: Ticks::beats(2),
            end: Ticks(4_000),
        }));
        t.seek(Ticks::ZERO);
        t.play();
        let mut ends = Vec::new();
        let mut done = 0u64;
        while done < 900_000 {
            let mut n = block.min(900_000 - done);
            if let Some(r) = t.samples_until_loop_end() {
                if r > 0 {
                    n = n.min(r);
                }
            }
            if t.advance(n) {
                ends.push((done + n, t.position_f64().to_bits()));
            }
            done += n;
        }
        (ends, t.position_f64().to_bits())
    };
    let reference = run(64);
    assert!(reference.0.len() > 10, "it wrapped");
    for block in [1u64, 37, 512] {
        assert_eq!(run(block), reference, "block {block}");
    }
}

/// ADR 0060 §8, review 2 (finding 10): the passes a ramp costs with latent inserts — the
/// demo over a 90→140 ramp, each strip behind a look-ahead limiter (its own lateness),
/// then a synced tremolo on one strip only. Counted live, 512-sample blocks, ten seconds
/// (run with `--ignored --nocapture`).
#[test]
#[ignore = "a measurement"]
fn measure_the_passes_of_latent_inserts_in_a_ramp() {
    let song = |ramp: bool, latent: bool, follower: bool| {
        let mut p = demo_project();
        p.loop_enabled = false;
        if ramp {
            p.tempo_points = vec![
                pt(0, 90.0, false),
                pt(960, 90.0, true),
                pt(30_720, 140.0, false),
            ];
        }
        if latent {
            for i in 0..p.tracks.len() {
                let Some(strip) = p.tracks[i].strip else {
                    continue;
                };
                let mut lim = p.make_effect(EffectKind::Limiter);
                lim.spec.set("look", 1.0 + i as f32).unwrap();
                let mut eq = p.make_effect(EffectKind::Eq);
                eq.spec.set("b0.used", 1.0).unwrap();
                let s = p.strip_mut(strip).unwrap();
                s.effects.insert(0, lim);
                s.effects.push(eq);
            }
        }
        if follower {
            let fx = add_effect(&mut p, EffectKind::Tremolo);
            set_all(&mut p, fx, &[("depth", 0.5), ("div", 8.0)]);
        }
        live(&p, 480_000, 512).1
    };
    let flat = song(false, true, true);
    eprintln!("one tempo, latent inserts and a tremolo: {flat} passes");
    eprintln!("ramp, nothing latent: {} passes", song(true, false, false));
    eprintln!(
        "ramp, latent inserts, no follower: {} passes",
        song(true, true, false)
    );
    eprintln!(
        "ramp, latent inserts, a tremolo: {} passes",
        song(true, true, true)
    );
}

/// Review 3 (finding 21): blocks that cross the loop's end (no cut there) carry the
/// overshoot: after each wrap the position is the map's seconds past the end, laid after
/// the loop's start — the end inside a ramp, at several block sizes.
#[test]
fn a_wrap_inside_a_ramp_carries_its_overshoot_through_the_map() {
    let map = Arc::new(TempoMap::new(
        90.0,
        TimeSignature::default(),
        &[
            pt(0, 90.0, false),
            pt(960, 90.0, true),
            pt(4_800, 140.0, false),
        ],
        &[],
    ));
    let (start, end) = (Ticks::beats(2), Ticks(4_000));
    for block in [37u64, 512, 1000] {
        let mut t = Transport::new(SR);
        t.set_map(map.clone());
        t.set_loop(Some(LoopRegion { start, end }));
        t.seek(Ticks::ZERO);
        t.play();
        let mut wraps = 0;
        for _ in 0..(30.0 * SR) as u64 / block {
            let before = t.position_f64();
            if t.advance(block) {
                let over = block as f64 / SR - map.seconds_between_f64(before, end.0 as f64);
                let want = map.tick_after_seconds(start.0 as f64, over);
                let got = t.position_f64();
                assert!((got - want).abs() < 1e-6, "block {block}: {got} vs {want}");
                wraps += 1;
            }
        }
        assert!(wraps > 10, "block {block}: {wraps} wraps");
    }
}

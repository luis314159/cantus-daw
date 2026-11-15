//! r40:pads — the application's engine starts with no compensation line (ADR 0120 §7,
//! the `memory` builder's lever L5): its handle sends the lines a snapshot can need ahead of
//! it, off the audio thread. Null tests: an engine that started with none plays a song that
//! gains latency mid-session, and one that had it from the start, **to the bit** as an
//! engine that started with the old 32; and a processor driven by hand that started with
//! none holds what a snapshot needs through `hold_parts_for`.

mod support;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;
use support::*;

const SR: f64 = 48_000.0;

/// The demo song with a compressor hearing 5 ms ahead on the lead: latency, and every
/// other path compensated for it.
fn latent(p: &Project) -> Project {
    let mut q = p.clone();
    let fx = add_effect(&mut q, EffectKind::Compressor);
    let s = strip_of(&mut q, 0).effect_mut(fx).unwrap();
    s.spec.set("look", 5.0).unwrap();
    s.spec.set("thr", -30.0).unwrap();
    q
}

/// `steps` of (project, blocks) through a handle whose processor started with `lines`.
fn session(lines: usize, steps: &[(&Project, usize)]) -> (Vec<f32>, usize) {
    let (mut h, mut proc_) = EngineHandle::manual_with_lines(SR, lines);
    assert_eq!(proc_.lines_held(), lines);
    let mut out = vec![0f32; 256 * 2];
    let mut all = Vec::new();
    for (i, (p, blocks)) in steps.iter().enumerate() {
        let snap = h.compile(p, 1);
        h.set_snapshot(snap);
        if i == 0 {
            h.play();
        }
        for _ in 0..*blocks {
            proc_.process(&mut out, 2);
            h.poll();
            all.extend_from_slice(&out);
        }
    }
    (all, proc_.lines_held())
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i}: {:e} vs {:e}", a[i], b[i]);
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

#[test]
fn a_song_gaining_latency_mid_session_plays_as_with_the_old_start_lines() {
    let p = demo_project();
    let q = latent(&p);
    let steps = [(&p, 60), (&q, 160)];
    let (eager, _) = session(pdc::START_LINES, &steps);
    let (lean, held) = session(0, &steps);
    same_bits(&lean, &eager, "gaining latency mid-session");
    assert_eq!(
        held,
        pdc::lines_needed(&Snapshot::compile(&q, 1)),
        "the lines sent ahead: what the song can need, no more"
    );
}

#[test]
fn a_song_with_latency_from_the_start_plays_as_with_the_old_start_lines() {
    let q = latent(&demo_project());
    let (eager, _) = session(pdc::START_LINES, &[(&q, 200)]);
    let (lean, _) = session(0, &[(&q, 200)]);
    same_bits(&lean, &eager, "latency from the start");
    // And as the song that had it all along, offline.
    let offline = render_with(&q, 256);
    let n = offline.len().min(lean.len());
    same_bits(&lean[..n], &offline[..n], "live = offline");
}

#[test]
fn a_song_with_no_latency_takes_no_line() {
    let mut p = Project::new("quiet");
    let t = p.make_track("Empty");
    p.tracks.push(t);
    p.tracks[0].instrument = InstrumentSpec::default();
    let snap = Snapshot::compile(&p, 1);
    let (mut h, mut proc_) = EngineHandle::manual_with_lines(SR, 0);
    let need = pdc::lines_needed(&snap);
    let snap = h.compile(&p, 1);
    h.set_snapshot(snap);
    let mut out = vec![0f32; 512];
    proc_.process(&mut out, 2);
    h.poll();
    assert_eq!(proc_.lines_held(), need);
}

#[test]
fn a_hand_driven_processor_with_no_lines_holds_what_its_snapshot_needs() {
    let q = latent(&demo_project());
    let snap = Arc::new(Snapshot::compile(&q, 1));
    let run = |lines: usize| {
        let (mut tx, rx) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (etx, _erx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let mut proc_ = Processor::without_midi_lines(SR, rx, etx, lines);
        proc_.hold_parts_for(&snap);
        assert!(proc_.lines_held() >= pdc::lines_needed(&snap).min(lines.max(1)));
        tx.push(EngineMessage::SetSnapshot(snap.clone())).unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let mut out = vec![0f32; 256 * 2];
        let mut all = Vec::new();
        for _ in 0..200 {
            proc_.process(&mut out, 2);
            all.extend_from_slice(&out);
        }
        all
    };
    same_bits(
        &run(0),
        &run(pdc::START_LINES),
        "hand-driven, held by hold_parts_for",
    );
}

//! Delay's time synced to the tempo through the engine (round 40, ADR 0119): the unit told
//! the tempo of each pass by the processor's own wiring, through a tempo track's ramp and a
//! jump; the same bits at every block size and in real time as offline; the echo where the
//! note value is at each tempo; the longer line a whole note needs asked for and answered on
//! ADR 0022's path while the song plays; and the sync off is the unit it always was.

use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::sync::Arc;

const SR: u32 = 48_000;

/// One track, a short note at `beats` each; Delay on its strip, fully wet, one repeat, both
/// sides straight; the tempo at 120 BPM, then `points`.
fn project(beats: &[i64], points: &[TempoPoint], delay: &[(&str, f32)]) -> (Project, EffectId) {
    let mut p = Project::new("delay sync");
    p.tempo_bpm = 120.0;
    if !points.is_empty() {
        p.tempo_points = points.to_vec();
    }
    let mut t = p.make_track("Keys");
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(16));
    for b in beats {
        clip.notes
            .push(p.make_note(Ticks::beats(*b), Ticks::SIXTEENTH, 72, 110));
    }
    t.clips.push(clip);
    let mut fx = p.make_effect(EffectKind::Delay);
    for (k, v) in [
        ("mix", 1.0),
        ("feedback", 0.0),
        ("ping_pong", 0.0),
        ("tone_hz", 20_000.0),
    ]
    .iter()
    .chain(delay)
    {
        fx.spec.set(k, *v).unwrap();
    }
    let id = fx.id;
    let strip = t.strip.unwrap();
    p.tracks.push(t);
    p.strip_mut(strip).unwrap().effects.push(fx);
    (p, id)
}

fn point(beat: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks::beats(beat),
        bpm,
        ramp,
    }
}

/// A ramp from 120 down to 90 over beats 2 … 6, a jump to 150 at beat 10.
fn tempo_track() -> Vec<TempoPoint> {
    vec![
        point(0, 120.0, false),
        point(2, 120.0, true),
        point(6, 90.0, false),
        point(10, 150.0, false),
    ]
}

fn render_at(p: &Project, block_frames: usize) -> Vec<f32> {
    render(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            sample_rate: SR,
            end: Some(Ticks::beats(14)),
            tail_seconds: 1.0,
            block_frames,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

/// The first frame from `from` seconds on where the left side passes `level`.
fn onset(x: &[f32], from: f64, level: f32) -> Option<f64> {
    let a = (from * f64::from(SR)) as usize;
    (a..x.len() / 2)
        .find(|i| x[i * 2].abs() > level)
        .map(|i| i as f64 / f64::from(SR))
}

#[test]
fn through_a_ramp_and_a_jump_every_block_size_is_the_same_to_the_bit() {
    let (p, _) = project(
        &[0, 3, 5, 8, 11],
        &tempo_track(),
        &[("sync", 1.0), ("note", 12.0), ("feedback", 0.55)],
    );
    let reference = render_at(&p, 512);
    assert!(reference.iter().any(|v| v.abs() > 0.05), "it sounds");
    for block in [1usize, 37, 64, 333, 4_096] {
        assert_null(
            &reference,
            &render_at(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

#[test]
fn offline_render_equals_real_time() {
    let (p, _) = project(
        &[0, 3, 5, 8, 11],
        &tempo_track(),
        &[("sync", 1.0), ("note", 10.0), ("feedback", 0.5)],
    );
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let offline = render(
        snap.clone(),
        &RenderOptions {
            sample_rate: SR,
            end: Some(Ticks::beats(14)),
            tail_seconds: 1.0,
            block_frames: 256,
            ..Default::default()
        },
    )
    .unwrap();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, SR as f32);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = offline.frames();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    assert_null(&offline.samples, &out, 0.0, "real time against offline");
}

#[test]
fn the_echo_is_the_note_value_at_each_tempo() {
    // A quarter: half a second at 120 BPM; after the jump to 90 at beat 4, two thirds of
    // one. The note at beat 6 is at 2 s + 2 × 2/3 s; its echo a quarter at 90 later.
    let points = [point(0, 120.0, false), point(4, 90.0, false)];
    let (synced, _) = project(&[0, 6], &points, &[("sync", 1.0), ("note", 12.0)]);
    let (free, _) = project(&[0, 6], &points, &[("time_ms", 500.0)]);
    let (dry, _) = project(&[0, 6], &points, &[("mix", 0.0)]);
    let (s, f, d) = (
        render_at(&synced, 512),
        render_at(&free, 512),
        render_at(&dry, 512),
    );
    let peak = d.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let level = peak * 0.05;
    let note = onset(&d, 3.0, level).expect("the second note");
    assert!(
        (note - (2.0 + 4.0 / 3.0)).abs() < 0.01,
        "the note at {note}"
    );
    let first = onset(&s, 0.0, level).unwrap();
    assert!((first - 0.5).abs() < 0.01, "a quarter at 120 BPM: {first}");
    let echo = onset(&s, 3.0, level).unwrap();
    assert!(
        (echo - note - 2.0 / 3.0).abs() < 0.005,
        "a quarter at 90 BPM after the note: {} s",
        echo - note
    );
    let free_echo = onset(&f, 3.0, level).unwrap();
    assert!(
        (free_echo - note - 0.5).abs() < 0.005,
        "the free time stays 500 ms: {} s",
        free_echo - note
    );
}

#[test]
fn the_sync_off_renders_as_a_delay_saved_before_round_40() {
    // A song saved before round 40 names no `sync` and no `note`: it reads them as their
    // defaults and renders, to the bit, as a new Delay with the sync off.
    let (p, fx) = project(
        &[0, 3, 8],
        &tempo_track(),
        &[("time_ms", 333.0), ("feedback", 0.6)],
    );
    let mut old = p.clone();
    let strip = old.tracks[0].strip.unwrap();
    let slot = old
        .strip_mut(strip)
        .unwrap()
        .effects
        .iter_mut()
        .find(|e| e.id == fx)
        .unwrap();
    slot.spec.params.remove("sync");
    slot.spec.params.remove("note");
    assert_eq!(slot.spec.get("sync"), 0.0);
    let (a, b) = (render_at(&p, 512), render_at(&old, 512));
    assert_null(&a, &b, 0.0, "the keys missing are the defaults");
    // And no note value moves a sample while the sync is off.
    let mut other = p.clone();
    other
        .strip_mut(strip)
        .unwrap()
        .effects
        .iter_mut()
        .find(|e| e.id == fx)
        .unwrap()
        .spec
        .set("note", 25.0);
    assert_null(
        &a,
        &render_at(&other, 512),
        0.0,
        "the note waits for the sync",
    );
}

#[test]
fn a_whole_note_turned_on_while_playing_asks_for_its_line_and_gets_it() {
    // Playing with the sync off on the free 2 s line; then the sync on at a whole note at
    // 60 BPM (4 s): the unit asks for a 12 s line once (ADR 0022's path), the answer comes
    // as the backend sends it, and from then on the echo is a whole note late.
    let points = [point(0, 60.0, false)];
    let (p, fx) = project(&[0, 8], &points, &[]);
    let mut on = p.clone();
    let strip = on.tracks[0].strip.unwrap();
    let slot = on.strip_mut(strip).unwrap();
    let e = slot.effects.iter_mut().find(|e| e.id == fx).unwrap();
    e.spec.set("sync", 1.0);
    e.spec.set("note", 18.0);
    let (snap, snap_on) = (
        Arc::new(Snapshot::compile(&p, 1)),
        Arc::new(Snapshot::compile(&on, 1)),
    );
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    for strip in &snap.strips {
        for slot in &strip.effects {
            tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
                slot, SR as f32,
            ))))
            .unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::new();
    let mut asked = 0;
    let mut run = |proc: &mut Processor,
                   tx: &mut rtrb::Producer<EngineMessage>,
                   frames: usize,
                   out: &mut Vec<f32>| {
        let mut done = 0;
        while done < frames {
            let mut buf = vec![0f32; 512 * 2];
            proc.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            done += 512;
            while let Ok(ev) = rx.pop() {
                if let EngineEvent::EffectNeed { effect, need } = ev {
                    assert_eq!(effect, fx);
                    asked += 1;
                    let memory = EffectMemory::build(need.need(), SR as f32);
                    assert!(
                        matches!(memory, EffectMemory::DelayLine(ref l) if l.len() == 12 * SR as usize + 2)
                    );
                    tx.push(EngineMessage::EffectMemory { effect, memory })
                        .unwrap();
                }
            }
        }
    };
    run(&mut proc, &mut tx, SR as usize, &mut out);
    tx.push(EngineMessage::SetSnapshot(snap_on)).unwrap();
    run(&mut proc, &mut tx, 13 * SR as usize, &mut out);
    assert_eq!(asked, 1, "asked once");
    // The note at beat 8 (8 s at 60 BPM): its echo a whole note (4 s) later.
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let echo = onset(&out, 8.5, peak * 0.05).expect("the echo");
    assert!(
        (echo - 12.0).abs() < 0.01,
        "a whole note at 60 BPM after beat 8: {echo}"
    );
    assert!(out.iter().all(|v| v.is_finite()));
}

// ---- fix-a: the mid-round review (A-M2, A-M3) --------------------------------------------

/// A lane on Delay `fx`'s `key` on track 0, from `beat` for eight beats, holding `pos` (the
/// parameter's position, 0 … 1).
fn lane(p: &mut Project, fx: EffectId, key: &str, beat: i64, pos: f32) {
    let c = p.make_automation_clip(
        "a",
        AutoTarget::Effect {
            effect: fx,
            key: key.into(),
        },
        Ticks::beats(beat),
        Ticks::beats(8),
        vec![AutoPoint::new(Ticks::ZERO, pos)],
    );
    p.tracks[0].clips.push(c);
}

/// The position of note value `i` on its lane (0 … 26, stepped).
fn note_pos(i: usize) -> f32 {
    i as f32 / 26.0
}

/// The live engine over `p`, its events answered after each block as the backend does
/// (varied blocks); the output, and the lengths of the Delay lines it gave back.
fn real_time(p: &Project, frames: usize) -> (Vec<f32>, Vec<usize>) {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        let unit = EffectUnit::new(slot, SR as f32);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(frames * 2);
    let mut back = Vec::new();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while let Ok(ev) = rx.pop() {
            match ev {
                EngineEvent::EffectNeed { effect, need } => {
                    let memory = EffectMemory::build(need.need(), SR as f32);
                    tx.push(EngineMessage::EffectMemory { effect, memory })
                        .unwrap();
                }
                EngineEvent::GarbageMemory(EffectMemory::DelayLine(l)) => back.push(l.len()),
                _ => {}
            }
        }
    }
    (out, back)
}

#[test]
fn a_note_automated_longer_plays_its_whole_echo_offline_and_in_real_time() {
    // At 60 BPM a quarter is 1 s (its line 3 s, a quarter at 20 BPM); from beat 4 a lane
    // makes it a whole note, 4 s. The note at beat 8 echoes at 12 s — not held at the
    // quarter's 3 s line (11 s): the line was built for the lane before the song got
    // there. Real time (the line arriving after the first block, the history moved in a
    // stretch per pass) equals offline, to the bit.
    let points = [point(0, 60.0, false)];
    let (mut p, fx) = project(&[0, 8], &points, &[("sync", 1.0), ("note", 12.0)]);
    lane(&mut p, fx, "note", 4, note_pos(18));
    let offline = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            sample_rate: SR,
            end: Some(Ticks::beats(14)),
            tail_seconds: 1.0,
            block_frames: 256,
            ..Default::default()
        },
    )
    .unwrap();
    let peak = offline.samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let echo = onset(&offline.samples, 8.5, peak * 0.05).expect("the echo");
    assert!(
        (echo - 12.0).abs() < 0.01,
        "a whole note at 60 BPM after beat 8: {echo}"
    );
    let (live, back) = real_time(&p, offline.frames());
    assert_null(&offline.samples, &live, 0.0, "real time against offline");
    // The quarter's line came back to be freed, once the 12 s line held its history.
    assert_eq!(back, vec![sync_line(3.0)]);
}

/// A line of `seconds` at the tests' rate, in cells.
fn sync_line(seconds: f32) -> usize {
    cantus_dsp::delay::sync::line_len(seconds, SR as f32)
}

#[test]
fn sync_switched_on_by_automation_behind_a_latent_insert_is_the_same_at_every_block_size() {
    // The tempo track's ramp and jump; a compressor that looks 5 ms ahead before the Delay,
    // so the Delay hears the song 240 samples late; its sync off, switched on by a lane at
    // beat 1. Its tempo steps cut passes at its own lateness from then on (the follow
    // lates re-planned when it starts following the song): every block size the same.
    let (mut p, fx) = project(
        &[0, 3, 5, 8, 11],
        &tempo_track(),
        &[("time_ms", 300.0), ("note", 12.0), ("feedback", 0.55)],
    );
    let strip = p.tracks[0].strip.unwrap();
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.spec.set("look", 5.0).unwrap();
    comp.spec.set("ratio", 1.0).unwrap();
    p.strip_mut(strip).unwrap().effects.insert(0, comp);
    lane(&mut p, fx, "sync", 1, 1.0);
    let reference = render_at(&p, 512);
    assert!(reference.iter().any(|v| v.abs() > 0.05), "it sounds");
    for block in [1usize, 37, 64, 333, 4_096] {
        assert_null(
            &reference,
            &render_at(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

// ---- fix-c (R2-A3): a modulated note reserves the line its modulator reaches -------------

/// The lengths of the Delay lines the live engine asks for while it plays `p`.
fn lines_asked(p: &Project, frames: usize) -> Vec<usize> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        let unit = EffectUnit::new(slot, SR as f32);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut asked = Vec::new();
    let mut done = 0;
    while done < frames {
        let mut buf = vec![0f32; 512 * 2];
        proc.process(&mut buf, 2);
        done += 512;
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::EffectNeed { effect, need } = ev {
                let need = need.need();
                if let MemoryNeed::DelayLine(n) = need {
                    asked.push(n);
                }
                let memory = EffectMemory::build(need, SR as f32);
                tx.push(EngineMessage::EffectMemory { effect, memory })
                    .unwrap();
            }
        }
    }
    asked
}

#[test]
fn a_modulated_note_reserves_only_the_line_its_modulator_reaches() {
    // A quarter at 60 BPM, an LFO on its note value at a small depth (a step either side):
    // the line is built for the longest note the LFO reaches — never the 48 s line every
    // note value would need (18 MB a side at 48 kHz) — and offline still equals real time.
    let points = [point(0, 60.0, false)];
    let (mut p, fx) = project(&[0, 8], &points, &[("sync", 1.0), ("note", 12.0)]);
    let host = ModHost::Track(p.tracks[0].id);
    let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    lfo.routes = vec![ModRoute {
        target: AutoTarget::Effect {
            effect: fx,
            key: "note".into(),
        },
        depth: 1.2 / 26.0,
    }];
    p.modulators_mut(host).unwrap().push(lfo);
    let reach = (11..=13)
        .map(|n| sync_line(cantus_dsp::delay::sync::line_seconds(true, n)))
        .max()
        .unwrap();
    let asked = lines_asked(&p, 6 * SR as usize);
    assert!(!asked.is_empty(), "the floor's line is asked for");
    assert!(
        asked.iter().all(|n| *n <= reach),
        "{asked:?}: no more than the notes 11 … 13 need ({reach})"
    );
    assert!(asked.iter().all(|n| *n < sync_line(48.0)));
    let offline = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            sample_rate: SR,
            end: Some(Ticks::beats(14)),
            tail_seconds: 1.0,
            block_frames: 256,
            ..Default::default()
        },
    )
    .unwrap();
    let (live, _) = real_time(&p, offline.frames());
    assert_null(&offline.samples, &live, 0.0, "real time against offline");
}

// ---- fix-c (R2-A1): a second reset before the line is cleared -----------------------------

/// The live engine over `p` in blocks of 512, `panics` panics sent before block `at` (the
/// processor takes them together, at that block's start).
fn with_panics(p: &Project, blocks: usize, at: usize, panics: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        let unit = EffectUnit::new(slot, SR as f32);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::new();
    for b in 0..blocks {
        if b == at {
            for _ in 0..panics {
                tx.push(EngineMessage::Panic).unwrap();
            }
        }
        let mut buf = vec![0f32; 512 * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

#[test]
fn two_panics_at_once_clear_a_delay_as_one_does() {
    // A note on every beat into a long-feedback Delay (free time and synced); two panics
    // in a row play on exactly as one: the line the first reset left to be cleared is
    // never read back by the second.
    for sync in [0.0, 1.0] {
        let beats: Vec<i64> = (0..16).collect();
        let (p, _) = project(
            &beats,
            &[],
            &[("feedback", 0.9), ("time_ms", 700.0), ("sync", sync)],
        );
        // At 4 s (beat 8): the 2 s and 3 s lines have wrapped, every cell holds sound.
        let at = 4 * SR as usize / 512;
        let blocks = 8 * SR as usize / 512;
        let once = with_panics(&p, blocks, at, 1);
        assert!(
            once[at * 1024..].iter().any(|v| v.abs() > 0.01),
            "sync {sync}: it sounds after the panic"
        );
        assert_null(
            &once,
            &with_panics(&p, blocks, at, 2),
            0.0,
            &format!("sync {sync}: two panics against one"),
        );
    }
}

//! Freezing (round 29, ADR 0064): a frozen strip or track plays its render, streamed from
//! disk, and the mix is the live mix **to the bit** — with its fader, pan, width, mute and
//! sends live and automated; at every block size; offline and in real time; serial and on
//! the pool; and unfreezing gives the live chain back, bit for bit. What only reaches the
//! frozen strip rests: its CPU is freed.

mod support;
use cantus_core::automation::{AutoPoint, AutoTarget};
use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::*;
use cantus_engine::freeze::render::{prepare, TapOptions};
use cantus_engine::*;
use std::path::PathBuf;
use support::*;

const SR: f64 = 48_000.0;

/// A folder of the test's own for renders.
fn folder(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("freeze")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn handle(name: &str) -> EngineHandle {
    let (mut h, _p) = EngineHandle::manual(SR);
    h.frozen.set_app_dir(folder(name));
    h
}

/// The demo song, the lead's strip with a delay (a tail that rings past the notes) and a
/// fader ride, a send from it into a reverb return; the bass's strip with a compressor.
fn song() -> Project {
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Delay);
    let lead = p.tracks[0].strip.unwrap();
    let ret = p.strips[5].id;
    let mut space = p.make_effect(EffectKind::Space);
    space.spec.set("mix", 1.0);
    p.strip_mut(ret).unwrap().effects.push(space);
    p.strip_mut(lead).unwrap().sends.push(Send {
        to: ret,
        level: 0.4,
        enabled: true,
        tap: SendTap::Post,
    });
    let comp = p.make_effect(EffectKind::Compressor);
    strip_of(&mut p, 1).effects.push(comp);
    // The lead's fader rides down and back: live while frozen.
    let ride = p.make_automation_clip(
        "ride",
        AutoTarget::StripGain(lead),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.8),
            AutoPoint::new(Ticks::beats(4), 0.3),
            AutoPoint::new(Ticks::beats(8), 0.9),
        ],
    );
    p.tracks[0].clips.push(ride);
    p
}

/// Render `target`'s freeze into the handle's folder, then freeze it in `p`.
fn freeze_now(h: &mut EngineHandle, p: &mut Project, target: FreezeTarget) {
    let job = prepare(h, p, target).expect("prepared");
    job.run(Vec::new(), Vec::new(), &RenderProgress::default())
        .expect("rendered");
    h.frozen.forget_missing();
    freeze::freeze_commands(p, target)
        .unwrap()
        .apply(p)
        .unwrap();
}

/// The mix as an export makes it: compensated, from tick 0, with a tail.
fn mix(h: &mut EngineHandle, p: &Project, block: usize) -> Vec<f32> {
    let snap = h.compile_for_render(p, 1);
    render(
        snap,
        &RenderOptions {
            block_frames: block,
            tail_seconds: 1.5,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i} differs: {} vs {}", a[i], b[i]);
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

fn lead_strip(p: &Project) -> FreezeTarget {
    FreezeTarget::Strip(p.tracks[0].strip.unwrap())
}

#[test]
fn a_frozen_strip_plays_the_live_mix_to_the_bit_with_its_mix_controls_live() {
    let mut h = handle("bits");
    let mut p = song();
    let live = mix(&mut h, &p, 512);
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let snap = h.compile_for_render(&p, 1);
    let si = p.strip_index(p.tracks[0].strip.unwrap()).unwrap();
    assert!(
        snap.strips[si].frozen.as_ref().unwrap().plays(),
        "the render is at hand"
    );
    assert!(
        snap.strips[si].effects.is_empty(),
        "its chain is in the render"
    );
    assert!(snap.tracks[0].rest, "the lead rests");
    same_bits(&mix(&mut h, &p, 512), &live, "frozen");
    // Its fader, pan, width and send level move after the freeze: still the live mix.
    let lead = p.tracks[0].strip.unwrap();
    for cmd in [
        Command::SetStripPan { id: lead, pan: 0.6 },
        Command::SetStripWidth {
            id: lead,
            width: 0.4,
        },
        Command::SetStripSend {
            id: lead,
            send: Send {
                to: p.strips[5].id,
                level: 0.9,
                enabled: true,
                tap: SendTap::Post,
            },
        },
    ] {
        let mut thawed = p.clone();
        freeze::thaw(&mut thawed);
        cmd.clone().apply(&mut thawed).unwrap();
        cmd.apply(&mut p).unwrap();
        same_bits(
            &mix(&mut h, &p, 512),
            &mix(&mut h, &thawed, 512),
            "a live control",
        );
    }
    // Muted, it is silent and the rest plays on.
    let mut muted = p.clone();
    Command::SetStripMute {
        id: lead,
        muted: true,
    }
    .apply(&mut muted)
    .unwrap();
    let mut thawed = muted.clone();
    freeze::thaw(&mut thawed);
    same_bits(
        &mix(&mut h, &muted, 512),
        &mix(&mut h, &thawed, 512),
        "muted",
    );
}

#[test]
fn unfreezing_gives_the_live_chain_back_to_the_bit() {
    let mut h = handle("unfreeze");
    let mut p = song();
    let live = mix(&mut h, &p, 512);
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    freeze::unfreeze_command(target).apply(&mut p).unwrap();
    let snap = h.compile_for_render(&p, 1);
    assert!(!snap.tracks[0].rest && !snap.strips[0].effects.is_empty());
    same_bits(&mix(&mut h, &p, 512), &live, "unfrozen");
}

#[test]
fn frozen_playback_does_not_depend_on_the_block_size() {
    let mut h = handle("blocks");
    let mut p = song();
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let reference = mix(&mut h, &p, 512);
    for block in [1, 37, 64] {
        same_bits(
            &mix(&mut h, &p, block),
            &reference,
            &format!("block {block}"),
        );
    }
}

#[test]
fn a_frozen_track_plays_its_source_and_keeps_its_level_and_balance_live() {
    let mut h = handle("track");
    let mut p = song();
    // The bass shares its strip with a second track, so Freeze takes its source alone.
    let mut pad = p.make_track("Pad");
    pad.strip = p.tracks[1].strip;
    let mut c = p.make_clip("pad", Ticks::ZERO, Ticks::beats(8));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(6), 52, 90));
    pad.clips.push(c);
    p.tracks.push(pad);
    let bass = p.tracks[1].id;
    let target = p.freeze_target_of_track(bass).unwrap();
    assert_eq!(target, FreezeTarget::Track(bass));
    let live = mix(&mut h, &p, 512);
    freeze_now(&mut h, &mut p, target);
    same_bits(&mix(&mut h, &p, 512), &live, "frozen track");
    for cmd in [
        Command::SetTrackGain {
            id: bass,
            gain_db: -7.5,
        },
        Command::SetTrackPan {
            id: bass,
            pan: -0.5,
        },
    ] {
        let mut thawed = p.clone();
        freeze::thaw(&mut thawed);
        cmd.clone().apply(&mut thawed).unwrap();
        cmd.apply(&mut p).unwrap();
        same_bits(
            &mix(&mut h, &p, 512),
            &mix(&mut h, &thawed, 512),
            "its level",
        );
    }
}

#[test]
fn a_latent_insert_frozen_away_keeps_the_mix_aligned() {
    let mut h = handle("latent");
    let mut p = song();
    // A linear-phase EQ on the lead (thousands of samples late) and another on the bass.
    let lead = p.tracks[0].strip.unwrap();
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec = support::heavy::heavy(EffectKind::Eq);
    p.strip_mut(lead).unwrap().effects.push(eq);
    let mut eq2 = p.make_effect(EffectKind::Eq);
    eq2.spec.set("mode", 2.0);
    strip_of(&mut p, 1).effects.push(eq2);
    let live = mix(&mut h, &p, 512);
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    same_bits(&mix(&mut h, &p, 512), &live, "latent frozen");
}

#[test]
fn a_frozen_strip_without_its_render_plays_live() {
    let mut h = handle("missing");
    let mut p = song();
    let live = mix(&mut h, &p, 512);
    let target = lead_strip(&p);
    freeze::freeze_commands(&p, target)
        .unwrap()
        .apply(&mut p)
        .unwrap();
    let snap = h.compile_for_render(&p, 1);
    assert!(!snap.strips[0].frozen.as_ref().unwrap().plays());
    assert!(!snap.tracks[0].rest);
    same_bits(&mix(&mut h, &p, 512), &live, "no render: live");
}

#[test]
fn a_render_is_found_by_its_key_and_streamed_not_held_whole() {
    let mut h = handle("key");
    let mut p = song();
    let target = lead_strip(&p);
    let content = freeze::content_key(&p, target).unwrap();
    freeze_now(&mut h, &mut p, target);
    let path = h.frozen.path_of(&content, 48_000).expect("written");
    assert!(path.extension().is_some_and(|x| x == "cfrz"));
    assert!(
        h.frozen.path_of(&content, 44_100).is_none(),
        "another rate is another key"
    );
    assert_ne!(
        cantus_engine::freeze::render_key(&content, 48_000),
        cantus_engine::freeze::render_key(&content, 44_100)
    );
    let a = h.frozen.find(&content, 48_000).unwrap();
    assert!(a.frames > 48_000, "the song and its tail: {}", a.frames);
    let snap = h.compile_for_render(&p, 1);
    let held = snap.strips[0]
        .frozen
        .as_ref()
        .unwrap()
        .audio
        .as_ref()
        .unwrap();
    let handle = held
        .reader
        .0
        .clone()
        .downcast::<cantus_engine::freeze::FrozenHandle>()
        .unwrap();
    assert!(handle.stream.is_some(), "a long render streams");
    assert!(
        handle.head.frames.len() / 2 < handle.frames / 4,
        "only its head is in memory: {} of {}",
        handle.head.frames.len() / 2,
        handle.frames
    );
}

#[test]
fn the_tail_is_kept_until_it_has_rung_out() {
    let mut h = handle("tail");
    let p = song();
    let target = lead_strip(&p);
    let job = prepare(&mut h, &p, target).unwrap();
    let end = job.snapshot.tempo.seconds_at(job.opts.end.0 as f64);
    let path = job
        .run(Vec::new(), Vec::new(), &RenderProgress::default())
        .unwrap();
    let a = cantus_engine::freeze::open(&path, &job.key).unwrap();
    let seconds = a.frames as f64 / SR;
    assert!(
        seconds > end,
        "the delay rings past the notes: {seconds} vs {end}"
    );
    assert!(seconds < end + 31.0);
    assert_eq!(job.opts, TapOptions::freeze(48_000, job.opts.end));
}

#[test]
fn a_cancelled_freeze_writes_nothing() {
    let mut h = handle("cancel");
    let p = song();
    let job = prepare(&mut h, &p, lead_strip(&p)).unwrap();
    let progress = RenderProgress::default();
    progress.cancel();
    assert!(matches!(
        job.run(Vec::new(), Vec::new(), &progress),
        Err(RenderError::Cancelled)
    ));
    assert!(!job.path.exists());
    assert!(std::fs::read_dir(job.path.parent().unwrap())
        .unwrap()
        .next()
        .is_none());
}

/// A processor playing `snap` from tick 0 as the application drives it, in blocks of
/// `block`, `blocks` of them; `pace` between blocks (a real-time stand-in).
fn play(
    snap: &std::sync::Arc<Snapshot>,
    workers: Workers,
    offline: bool,
    block: usize,
    blocks: usize,
    pace: std::time::Duration,
) -> (Vec<f32>, EngineStats) {
    let effects = snap.effect_slots().count();
    let instruments = cantus_engine::inst::units_for(snap, SR as f32, |_, _| false);
    let (mut tx, rx_engine) =
        rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    p.set_workers(workers);
    p.set_offline(offline);
    for slot in snap.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, SR as f32,
        ))))
        .ok();
    }
    for unit in instruments {
        tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(snap)))
        .ok();
    tx.push(EngineMessage::SetSnapshotPlanned(
        snap.clone(),
        cantus_engine::graph::compile(snap),
    ))
    .ok();
    p.settle();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                count, delay,
            )))
            .ok();
            p.settle();
        }
    }
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    // Stopped a moment first, as a musician does before pressing play: the reader reads
    // where the playhead waits.
    let mut buf = vec![0f32; block * 2];
    p.process(&mut buf, 2);
    std::thread::sleep(std::time::Duration::from_millis(50));
    tx.push(EngineMessage::Play).ok();
    let mut out = Vec::with_capacity(block * blocks * 2);
    for _ in 0..blocks {
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
        if !pace.is_zero() {
            std::thread::sleep(pace);
        }
    }
    (out, p.stats().clone())
}

#[test]
fn frozen_playback_on_the_pool_is_the_serial_engines() {
    let mut h = handle("pool");
    let mut p = song();
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let bass = p.tracks[1].id;
    freeze_now(&mut h, &mut p, FreezeTarget::Track(bass));
    let snap = h.compile_for_render(&p, 1);
    let (serial, _) = play(&snap, Workers::SERIAL, true, 256, 700, Default::default());
    for w in [1, 3] {
        let (pool, stats) = play(
            &snap,
            Workers::parallel(w),
            true,
            256,
            700,
            Default::default(),
        );
        same_bits(&pool, &serial, &format!("W = {w}"));
        assert_eq!(stats.schedule_conflicts, 0);
    }
}

#[test]
fn in_real_time_the_reader_keeps_ahead_of_the_playhead() {
    let mut h = handle("realtime");
    let mut p = song();
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let snap = h.compile_for_render(&p, 1);
    // Twice real time: 256 frames (5.3 ms) every 2.7 ms.
    let (rt, stats) = play(
        &snap,
        Workers::SERIAL,
        false,
        256,
        800,
        std::time::Duration::from_micros(2_700),
    );
    assert_eq!(stats.stream_late, 0, "no block came late");
    let (offline, _) = play(&snap, Workers::SERIAL, true, 256, 800, Default::default());
    same_bits(&rt, &offline, "real time = offline");
}

#[test]
fn what_only_reaches_a_frozen_strip_is_not_run() {
    // A heavy instrument on the lead: thirty-two voices of a sampler-sized synth.
    let mut h = handle("cpu");
    let mut p = song();
    let mut rng = support::heavy::Rng::new(7);
    let spec = support::heavy::random_instrument(InstrumentKind::Wavetable, 1.0, &mut rng);
    p.tracks[0].instrument = InstrumentSpec::Native(spec);
    let mut chord = p.make_clip("chord", Ticks::ZERO, Ticks::beats(8));
    for k in 0..32u8 {
        chord
            .notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(8), 36 + k * 2, 100));
    }
    p.tracks[0].clips.push(chord);
    let cost = |h: &mut EngineHandle, p: &Project| {
        let snap = h.compile_for_render(p, 1);
        (0..5)
            .map(|_| {
                let t = std::time::Instant::now();
                let _ = play(&snap, Workers::SERIAL, true, 512, 300, Default::default());
                t.elapsed().as_secs_f64()
            })
            .fold(f64::MAX, f64::min)
    };
    let live = cost(&mut h, &p);
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let frozen = cost(&mut h, &p);
    eprintln!(
        "live {live:.3} s, frozen {frozen:.3} s ({:.0} %)",
        100.0 * frozen / live
    );
    assert!(
        frozen < live * 0.6,
        "freezing the heavy strip frees its CPU: live {live:.3} s, frozen {frozen:.3} s"
    );
    // Its instrument plays no voice while frozen.
    let snap = h.compile_for_render(&p, 1);
    assert!(snap.tracks[0].rest);
}

#[test]
fn a_frozen_mix_is_the_live_one_before_compensation_too() {
    let mut h = handle("raw");
    let mut p = song();
    let lead = p.tracks[0].strip.unwrap();
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec = support::heavy::heavy(EffectKind::Eq);
    p.strip_mut(lead).unwrap().effects.push(eq);
    let raw = |h: &mut EngineHandle, p: &Project| {
        render(
            h.compile_for_render(p, 1),
            &RenderOptions {
                tail_seconds: 1.5,
                ..Default::default()
            },
        )
        .unwrap()
        .samples
    };
    let live = raw(&mut h, &p);
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(lead));
    same_bits(&raw(&mut h, &p), &live, "uncompensated");
}

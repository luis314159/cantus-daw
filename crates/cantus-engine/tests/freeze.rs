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
    // Its pan sweeps and its send to the return swells: live too.
    let sweep = p.make_automation_clip(
        "sweep",
        AutoTarget::StripPan(lead),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.2),
            AutoPoint::new(Ticks::beats(8), 0.8),
        ],
    );
    let swell = p.make_automation_clip(
        "swell",
        AutoTarget::Send {
            strip: lead,
            to: ret,
        },
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.1),
            AutoPoint::new(Ticks::beats(6), 0.9),
        ],
    );
    p.tracks[1].clips.push(sweep);
    p.tracks[1].clips.push(swell);
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
    (out, *p.stats())
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

#[test]
fn a_frozen_strip_with_a_warped_clip_plays_its_render_to_the_bit() {
    use cantus_core::warp::Warp;
    let mut h = handle("warp");
    let dir = folder("warp-src");
    let path = dir.join("loop.wav");
    let m = cantus_dsp::beat::testkit::drum_loop(48_000.0, 100.0, 2, 0.0, 0.0, 8);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for s in &m.audio {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
    let mut p = Project::new("warped");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Loop");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "loop".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: m.audio.len() as u64,
        sha256: None,
        recorded: false,
    });
    let warp = Warp::identity(48_000, 100.0, 0)
        .add_marker(Ticks::beats(4))
        .unwrap()
        .move_marker(1, Ticks::beats(3));
    let mut c = p.make_audio_clip("loop", id, Ticks::ZERO, Ticks::beats(8));
    c.audio.as_mut().unwrap().warp = Some(Box::new(warp));
    t.clips.push(c);
    p.tracks.push(t);
    let s = p.tracks[0].strip.unwrap();
    let comp = p.make_effect(EffectKind::Compressor);
    p.strip_mut(s).unwrap().effects.push(comp);
    let live = mix(&mut h, &p, 512);
    let target = FreezeTarget::Strip(s);
    let before = freeze::content_key(&p, target).unwrap();
    freeze_now(&mut h, &mut p, target);
    same_bits(&mix(&mut h, &p, 512), &live, "warped, frozen");
    // The warp is in what makes the sound: moving a marker moves the key.
    let mut q = p.clone();
    freeze::thaw(&mut q);
    q.tracks[0].clips[0].audio.as_mut().unwrap().warp = Some(Box::new(
        Warp::identity(48_000, 100.0, 0)
            .add_marker(Ticks::beats(4))
            .unwrap()
            .move_marker(1, Ticks::beats(2)),
    ));
    assert_ne!(freeze::content_key(&q, target).unwrap(), before);
}

// ---- review 1 ---------------------------------------------------------------------------

fn return_strip(p: &Project) -> StripId {
    p.strips[5].id
}

#[test]
fn a_frozen_return_keeps_the_dry_signal_that_feeds_it_to_the_bit() {
    let mut h = handle("return");
    let mut p = song();
    let live = mix(&mut h, &p, 512);
    let ret = return_strip(&p);
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(ret));
    let snap = h.compile_for_render(&p, 1);
    assert!(!snap.tracks[0].rest, "the lead plays to the master too");
    same_bits(&mix(&mut h, &p, 512), &live, "frozen return");
}

#[test]
fn a_frozen_keyed_bus_and_a_note_source_leave_their_sources_playing_to_the_bit() {
    let mut h = handle("keyed");
    let mut p = song();
    let lead_strip = p.tracks[0].strip.unwrap();
    let lead = p.tracks[0].id;
    // The bass's compressor keyed from the lead's strip, and an insert on the bass's
    // strip that hears the lead's notes.
    let bass_strip = p.tracks[1].strip.unwrap();
    p.strip_mut(bass_strip).unwrap().effects[0].sidechain = Some(lead_strip);
    let mut gate = p.make_effect(EffectKind::Gate);
    gate.midi_from = Some(lead);
    p.strip_mut(bass_strip).unwrap().effects.push(gate);
    let live = mix(&mut h, &p, 512);
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(bass_strip));
    let snap = h.compile_for_render(&p, 1);
    let li = p.strip_index(lead_strip).unwrap();
    assert!(!snap.strips[li].rest && !snap.tracks[0].rest);
    assert!(
        snap.tracks[1].rest,
        "the bass only reaches its frozen strip"
    );
    same_bits(&mix(&mut h, &p, 512), &live, "frozen keyed bus");
}

#[test]
fn a_solo_when_it_froze_is_not_in_the_render() {
    let mut h = handle("solo");
    let mut p = song();
    let live = mix(&mut h, &p, 512);
    let bass = p.tracks[1].id;
    Command::SetTrackSolo {
        id: bass,
        soloed: true,
    }
    .apply(&mut p)
    .unwrap();
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    Command::SetTrackSolo {
        id: bass,
        soloed: false,
    }
    .apply(&mut p)
    .unwrap();
    same_bits(
        &mix(&mut h, &p, 512),
        &live,
        "frozen under a solo, unsoloed",
    );
}

#[test]
fn a_nested_frozen_strip_renders_its_own_sound_while_the_outer_one_plays() {
    let mut h = handle("nested");
    let mut p = song();
    // The lead's strip plays into the bass's strip, a bus.
    let lead = p.tracks[0].strip.unwrap();
    let bus = p.tracks[1].strip.unwrap();
    p.strip_mut(lead).unwrap().output = Some(bus);
    // Heard only through the bus (no send elsewhere).
    p.strip_mut(lead).unwrap().sends.clear();
    for t in &mut p.tracks {
        t.clips.retain(
            |c| !matches!(&c.automation, Some(a) if matches!(a.target, AutoTarget::Send { .. })),
        );
    }
    let live = mix(&mut h, &p, 512);
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(lead));
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(bus));
    // Elsewhere only the bus's render is at hand: the lead's is made again under it.
    let mut h2 = handle("nested-2");
    let content = p.frozen(FreezeTarget::Strip(bus)).unwrap().content.clone();
    let from = h.frozen.path_of(&content, 48_000).unwrap();
    let to = h2.frozen.app_dir().unwrap().join(from.file_name().unwrap());
    std::fs::copy(&from, &to).unwrap();
    let job = prepare(&mut h2, &p, FreezeTarget::Strip(lead)).unwrap();
    job.run(Vec::new(), Vec::new(), &RenderProgress::default())
        .unwrap();
    h2.frozen.forget_missing();
    // The bus thawed: the lead's new render plays, and it is the live sound.
    freeze::unfreeze_command(FreezeTarget::Strip(bus))
        .apply(&mut p)
        .unwrap();
    let snap = h2.compile_for_render(&p, 1);
    let li = p.strip_index(lead).unwrap();
    assert!(snap.strips[li].frozen.as_ref().unwrap().plays());
    same_bits(
        &mix(&mut h2, &p, 512),
        &live,
        "the lead re-rendered under the bus",
    );
}

#[test]
fn an_offline_render_and_live_playback_read_one_render_with_their_own_cursors() {
    let mut h = handle("boards");
    let mut p = song();
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let alone = mix(&mut h, &p, 512);
    // Live playback of the same render, at twice real time, while an export runs.
    let live_snap = h.compile(&p, 1);
    // One render, one stream, a board for each reader.
    let board = |s: &Snapshot| {
        s.strips[0]
            .frozen
            .as_ref()
            .unwrap()
            .audio
            .as_ref()
            .unwrap()
            .reader
            .0
            .clone()
            .downcast::<cantus_engine::freeze::FrozenHandle>()
            .unwrap()
    };
    let (a, b) = (board(&live_snap), board(&h.compile_for_render(&p, 1)));
    assert!(std::sync::Arc::ptr_eq(&a.head, &b.head));
    assert!(!std::sync::Arc::ptr_eq(&a.board, &b.board));
    drop((a, b));
    let player = std::thread::spawn(move || {
        play(
            &live_snap,
            Workers::SERIAL,
            false,
            256,
            800,
            std::time::Duration::from_micros(2_700),
        )
    });
    let mut exported = Vec::new();
    for _ in 0..3 {
        exported = mix(&mut h, &p, 512);
    }
    let (_, stats) = player.join().unwrap();
    same_bits(&exported, &alone, "the export, beside live playback");
    assert_eq!(stats.stream_late, 0, "live playback kept its blocks");
}

#[test]
fn an_offline_render_starting_just_before_a_block_boundary_reads_its_block() {
    let mut h = handle("boundary");
    let mut p = song();
    // No automation: passes of a whole block (a curve splits them finer).
    for t in &mut p.tracks {
        t.clips.retain(|c| c.automation.is_none());
    }
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    // Frame ≈ 20 367 at 128 BPM: past the head (16 384), 113 frames before a block's end.
    let start = Ticks(869);
    let frame = p.tempo_map().seconds_at(start.0 as f64) * SR;
    let block = cantus_dsp::exemplar::stream::BLOCK as f64;
    assert!(frame > cantus_dsp::exemplar::stream::HEAD_FRAMES as f64);
    assert!(frame % block > block - 1024.0, "{frame}");
    let t = std::time::Instant::now();
    let out = render(
        h.compile_for_render(&p, 1),
        &RenderOptions {
            start,
            block_frames: 1024,
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .expect("every block read");
    assert!(out.samples.iter().any(|x| *x != 0.0));
    assert!(t.elapsed() < std::time::Duration::from_secs(5), "no stall");
}

/// A processor for `snap`'s project driven by hand: snapshots swapped while it plays.
struct Rig {
    p: Processor,
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    have: std::collections::HashSet<EffectId>,
}

impl Rig {
    fn new(snap: &std::sync::Arc<Snapshot>) -> Rig {
        let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let mut r = Rig {
            p: Processor::without_midi(SR, rx_engine, tx_engine),
            tx,
            rx,
            have: Default::default(),
        };
        r.p.set_offline(true);
        r.install(snap);
        r.tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        r.tx.push(EngineMessage::Play).ok();
        r
    }

    fn install(&mut self, snap: &std::sync::Arc<Snapshot>) {
        for slot in snap.effect_slots() {
            if self.have.insert(slot.id) {
                self.tx
                    .push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
                        slot, SR as f32,
                    ))))
                    .ok();
            }
        }
        self.tx
            .push(EngineMessage::Automation(AutomationRuntime::build(snap)))
            .ok();
        self.tx
            .push(EngineMessage::SetSnapshotPlanned(
                snap.clone(),
                cantus_engine::graph::compile(snap),
            ))
            .ok();
        self.p.settle();
        self.serve();
    }

    fn serve(&mut self) {
        while let Ok(ev) = self.rx.pop() {
            match ev {
                EngineEvent::PdcNeed { count, delay } => {
                    self.tx
                        .push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                            count, delay,
                        )))
                        .ok();
                    self.p.settle();
                }
                // A unit handed back is one the engine no longer has.
                EngineEvent::GarbageEffect(u) => {
                    self.have.remove(&u.id);
                }
                EngineEvent::EffectNeed { effect, need } => {
                    let memory = cantus_engine::effects::EffectMemory::build(need, SR as f32);
                    self.tx
                        .push(EngineMessage::EffectMemory { effect, memory })
                        .ok();
                    self.p.settle();
                }
                _ => {}
            }
        }
    }

    /// Play until the transport reaches `tick`.
    fn play_to(&mut self, map: &TempoMap, tick: Ticks) -> Vec<f32> {
        let until = (map.seconds_at(tick.0 as f64) * SR) as u64;
        let mut out = Vec::new();
        let mut buf = vec![0f32; 512];
        while self.p.transport().samples() < until {
            self.p.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            self.serve();
        }
        out
    }
}

#[test]
fn thawing_while_playing_leaves_no_voice_and_no_tail_from_before() {
    let mut h = handle("thaw");
    let p = song();
    let map = p.tempo_map();
    let target = lead_strip(&p);
    let live = h.compile_for_render(&p, 1);
    let mut frozen_p = p.clone();
    freeze_now(&mut h, &mut frozen_p, target);
    let frozen = h.compile_for_render(&frozen_p, 1);
    let mut rig = Rig::new(&live);
    // The lead's chord sounds from beat 6 to beat 10: frozen while it is held, played
    // on past its note-off, thawed at beat 12.
    rig.play_to(&map, Ticks::beats(7));
    rig.install(&frozen);
    rig.play_to(&map, Ticks::beats(12));
    rig.install(&live);
    rig.play_to(&map, Ticks::beats(12) + Ticks::QUARTER);
    assert_eq!(
        rig.p.stats().active_voices,
        0,
        "no voice of the chord came back"
    );
    // And while frozen, a note due at a loop's start is not played into the frozen track.
    let mut q = p.clone();
    let bass = q.tracks[1].id;
    let mut empty = q.make_track("Spare");
    empty.strip = q.tracks[1].strip;
    q.tracks.push(empty);
    q.tracks[0].clips.clear();
    q.loop_enabled = true;
    q.loop_region = Some(LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(4),
    });
    freeze_now(&mut h, &mut q, FreezeTarget::Track(bass));
    let looped = h.compile(&q, 2);
    let mut rig = Rig::new(&looped);
    let mut buf = vec![0f32; 512];
    let mut most = 0;
    for _ in 0..1200 {
        rig.p.process(&mut buf, 2);
        rig.serve();
        most = most.max(rig.p.stats().active_voices);
    }
    assert_eq!(most, 0, "the frozen track's instrument is never played");
    drop(p);
}

#[test]
fn a_follower_of_a_frozen_strip_bakes_what_it_heard_live() {
    let mut h = handle("bake");
    let mut p = song();
    let lead = p.tracks[0].strip.unwrap();
    let bass = p.tracks[1].strip.unwrap();
    let mut f = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Strip(lead);
    }
    f.routes.push(ModRoute {
        target: AutoTarget::StripPan(bass),
        depth: 0.5,
    });
    let id = f.id;
    p.strip_mut(bass).unwrap().modulators.push(f);
    let opts = RenderOptions::default();
    let live = cantus_engine::modulator::trace(
        h.compile_for_render(&p, 1),
        &opts,
        Vec::new(),
        Vec::new(),
        id,
    );
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(lead));
    let frozen = cantus_engine::modulator::trace(
        h.compile_for_render(&p, 1),
        &opts,
        Vec::new(),
        Vec::new(),
        id,
    );
    assert_eq!(frozen, live);
    assert!(live.iter().filter(|(_, v)| *v != 0.0).count() > 10);
}

#[test]
fn renders_no_longer_used_are_let_go() {
    let mut h = handle("letgo");
    let mut p = song();
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let _ = h.compile(&p, 1);
    assert_eq!(h.frozen.open_count(), 1);
    freeze::unfreeze_command(target).apply(&mut p).unwrap();
    let _ = h.compile(&p, 2);
    assert_eq!(h.frozen.open_count(), 0, "unfrozen, its render is let go");
}

#[test]
fn a_damaged_render_is_missing_and_made_again() {
    let mut h = handle("damaged");
    let mut p = song();
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let content = p.frozen(target).unwrap().content.clone();
    // Damaged past its head, before anything opened it.
    let path = h.frozen.path_of(&content, 48_000).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    let n = bytes.len();
    // About two seconds in (8 bytes a frame): read by the mix, far past the head.
    assert!(n > 900_000);
    for b in &mut bytes[800_000..804_096] {
        *b ^= 0x5a;
    }
    std::fs::write(&path, &bytes).unwrap();
    let snap = h.compile_for_render(&p, 1);
    let attached = snap.strips[0].frozen.as_ref().unwrap().plays();
    let failed = render(
        snap,
        &RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    );
    // Past the head, the damage is not seen when it opens: attached, then not readable
    // (review 2: the test held either way).
    assert!(attached, "opened: its head is whole");
    assert!(failed.is_err(), "the damaged blocks cannot be read");
    // Then it is missing: the strip plays live, and the interface makes it again.
    assert!(h.frozen.find(&content, 48_000).is_none());
}

// ---- review 2 ---------------------------------------------------------------------------

#[test]
fn a_passing_read_error_keeps_a_render_and_its_file() {
    let mut h = handle("io");
    let mut p = song();
    let live = mix(&mut h, &p, 512);
    let target = lead_strip(&p);
    freeze_now(&mut h, &mut p, target);
    let content = p.frozen(target).unwrap().content.clone();
    let path = h.frozen.path_of(&content, 48_000).unwrap();
    // One read of the render fails (a busy drive, an antivirus' lock).
    cantus_engine::inst::stream::inject_io_fault(&path);
    same_bits(
        &mix(&mut h, &p, 512),
        &live,
        "read again, played to the bit",
    );
    assert!(path.is_file(), "the render is kept");
    assert!(
        h.frozen.find(&content, 48_000).is_some(),
        "and still at hand"
    );
}

#[test]
fn a_missing_file_makes_no_render() {
    let mut h = handle("missing-file");
    let mut p = Project::new("missing");
    let mut t = p.make_track("Take");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "gone".into(),
        path: folder("missing-src")
            .join("not there.wav")
            .to_string_lossy()
            .into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000,
        sha256: None,
        recorded: false,
    });
    t.clips
        .push(p.make_audio_clip("gone", id, Ticks::ZERO, Ticks::beats(2)));
    p.tracks.push(t);
    let s = p.tracks[0].strip.unwrap();
    let e = prepare(&mut h, &p, FreezeTarget::Strip(s)).expect_err("refused");
    assert!(e.starts_with("missing:"), "{e}");
}

#[test]
fn a_cancelled_write_leaves_no_render() {
    let mut h = handle("cancel-write");
    let p = song();
    let job = prepare(&mut h, &p, lead_strip(&p)).unwrap();
    let raw = job.path.with_extension("raw.part");
    let t = cantus_engine::freeze::render::render_tapped(
        job.snapshot.clone(),
        job.point,
        &job.opts,
        Vec::new(),
        Vec::new(),
        &RenderProgress::default(),
        &raw,
    )
    .unwrap();
    let stop = RenderProgress::default();
    stop.cancel();
    assert!(matches!(
        cantus_engine::freeze::render::write_render(&t, &job.path, &job.key, &stop),
        Err(RenderError::Cancelled)
    ));
    assert!(!job.path.exists());
    let wav = job.path.with_extension("wav");
    assert!(matches!(
        cantus_engine::freeze::render::write_wav(&t, &wav, &stop),
        Err(RenderError::Cancelled)
    ));
    assert!(!wav.exists());
}

#[test]
fn a_solo_that_silences_a_key_plays_the_frozen_bus_live() {
    let mut h = handle("solo-key");
    let mut p = song();
    // The bass's strip keyed from the lead's strip, frozen; the bass's strip soloed: live,
    // the lead (not upstream of it) is muted and keys nothing.
    let lead_strip = p.tracks[0].strip.unwrap();
    let bass_strip = p.tracks[1].strip.unwrap();
    p.strip_mut(bass_strip).unwrap().effects[0].sidechain = Some(lead_strip);
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(bass_strip));
    Command::SetStripSolo {
        id: bass_strip,
        soloed: true,
    }
    .apply(&mut p)
    .unwrap();
    let mut thawed = p.clone();
    freeze::thaw(&mut thawed);
    same_bits(&mix(&mut h, &p, 512), &mix(&mut h, &thawed, 512), "soloed");
}

#[test]
fn a_note_source_resting_but_for_its_notes_keeps_the_mix() {
    let mut h = handle("notes-only");
    let mut p = song();
    // Tonus on the lead's strip hears the bass's notes; the bass's strip is frozen.
    let lead_strip = p.tracks[0].strip.unwrap();
    let bass = p.tracks[1].id;
    let bass_strip = p.tracks[1].strip.unwrap();
    let mut tonus = p.make_effect(EffectKind::AutoTune);
    tonus.midi_from = Some(bass);
    p.strip_mut(lead_strip).unwrap().effects.push(tonus);
    let live = mix(&mut h, &p, 512);
    freeze_now(&mut h, &mut p, FreezeTarget::Strip(bass_strip));
    let snap = h.compile_for_render(&p, 1);
    assert!(snap.tracks[1].notes_only && !snap.tracks[1].rest);
    same_bits(&mix(&mut h, &p, 512), &live, "its notes still heard");
}

#[test]
fn thawing_a_strip_while_playing_resets_its_eq_and_a_resting_strips_echoes() {
    let mut h = handle("thaw-eq");
    // The lead alone: its strip with a resonant EQ and a delay, into a bus.
    let mut p = demo_project();
    p.tracks.truncate(1);
    let lead = p.tracks[0].strip.unwrap();
    let bus = p.strips[6].id;
    let mut delay = p.make_effect(EffectKind::Delay);
    delay.spec.set("feedback", 0.9).unwrap();
    delay.spec.set("mix", 1.0).unwrap();
    p.strip_mut(lead).unwrap().effects.push(delay);
    p.strip_mut(lead).unwrap().eq.bands[0].gain_db = 12.0;
    p.strip_mut(lead).unwrap().output = Some(bus);
    let map = p.tempo_map();
    for target in [FreezeTarget::Strip(lead), FreezeTarget::Strip(bus)] {
        let live = h.compile_for_render(&p, 1);
        let mut frozen_p = p.clone();
        freeze_now(&mut h, &mut frozen_p, target);
        let frozen = h.compile_for_render(&frozen_p, 1);
        let mut rig = Rig::new(&live);
        // Frozen at beat 7, while the lead's chord sounds; thawed at beat 12.
        rig.play_to(&map, Ticks::beats(7));
        rig.install(&frozen);
        rig.play_to(&map, Ticks::beats(12));
        rig.install(&live);
        let after = rig.play_to(&map, Ticks::beats(13));
        assert!(
            after.iter().all(|x| *x == 0.0),
            "{target:?}: nothing from before the freeze sounds after the thaw"
        );
    }
}

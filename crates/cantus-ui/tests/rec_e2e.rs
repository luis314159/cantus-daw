//! A recording pass end to end (ADR 0025 §5–§7): a real pass through the engine's
//! recorder (a processor driven by hand stands in for the audio callback), finished by
//! the handle, laid out by `record_pass::build` exactly as the application does, and
//! played back — every loop pass's clip must play exactly the frames that were played
//! against that pass, to the sample, with a latency that is not a whole tick.

use cantus_core::audio::TrackInput;
use cantus_core::{Command, Project, Snapshot, Ticks, TrackId};
use cantus_engine::{render, EngineHandle, RenderOptions, SampleCache};
use cantus_ui::record_pass::{self, PassPlan};
use std::sync::Arc;

const SR: u32 = 48_000;
/// At 120 BPM a tick is exactly 25 frames.
const FPT: i64 = 25;

/// Frame `i` of the input is `i + 1` (scaled to stay exact in f32): any frame read back
/// says which input frame it was.
fn ramp(frames: usize) -> Vec<f32> {
    (0..frames).map(|i| (i + 1) as f32 / 1_048_576.0).collect()
}

fn which(v: f32) -> Option<i64> {
    (v != 0.0).then(|| (v * 1_048_576.0).round() as i64 - 1)
}

fn project() -> (Project, TrackId) {
    let mut p = Project::new("e2e");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    t.strip = None;
    let id = t.id;
    p.tracks.push(t);
    (p, id)
}

/// What each clip of the track plays from timeline tick `from`, `frames` long, alone.
fn play_alone(p: &Project, track: TrackId, clip: cantus_core::ClipId, from: Ticks) -> Vec<f32> {
    let mut q = p.clone();
    let t = q.tracks.iter_mut().find(|t| t.id == track).unwrap();
    t.clips.retain(|c| c.id == clip);
    t.clips[0].lane = 0;
    let mut s = Snapshot::compile(&q, 1);
    assert!(SampleCache::new().attach_audio(&mut s, SR).is_empty());
    let opts = RenderOptions {
        sample_rate: SR,
        start: from,
        tail_seconds: 0.0,
        block_frames: 512,
        ..Default::default()
    };
    let out = render(Arc::new(s), &opts).unwrap().samples;
    out.chunks_exact(2).map(|f| f[0]).collect()
}

fn run_pass(latency: i64, block: usize) {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, track) = project();
    // Loop bars 1–2 of 2/4… simply: beat 1 to beat 3 (48 000 frames a pass), recording
    // from the loop's start.
    let (a, b) = (Ticks::beats(1), Ticks::beats(3));
    p.loop_enabled = true;
    p.loop_region = Some(cantus_core::LoopRegion { start: a, end: b });
    let pass = ((b - a).0 * FPT) as usize;

    let (mut h, mut proc) = EngineHandle::manual(SR as f64);
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 2);
    proc.attach_input(consumer, 1);
    let snap = h.compile(&p, 1);
    h.set_snapshot(snap);
    h.set_record_offset(latency);
    h.seek(a);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    // Three and a half passes.
    let total = pass * 7 / 2;
    let x = ramp(total);
    let mut out = vec![0f32; block * 2];
    for chunk in x.chunks(block) {
        for v in chunk {
            ring.push(*v).unwrap();
        }
        proc.process(&mut out[..chunk.len() * 2], 2);
        h.poll();
    }
    h.set_recording(false);
    h.stop();
    proc.process(&mut out, 2);
    h.poll();
    let takes = h.finish_recording();
    assert_eq!(takes.len(), 1);
    assert_eq!(takes[0].frames, total as u64);

    let mut plan = PassPlan::new(a);
    plan.audio_tracks = vec![track];
    plan.loop_region = Some((a, b));
    plan.stop_at = a + Ticks((total - 3 * pass) as i64 / FPT);
    let mut next = p.next_id;
    let mut ids = || {
        next += 1;
        next - 1
    };
    let lat = h.record_latency().total();
    assert_eq!(lat, latency);
    let cmds = record_pass::build(&p, &plan, &takes, &[], lat, &mut ids).unwrap();
    p.next_id = next;
    for c in cmds {
        c.apply(&mut p).unwrap();
    }

    // The passes, oldest first: the take lanes in order, then the playing lane.
    let t = p.track(track).unwrap();
    let mut clips: Vec<_> = t.clips.iter().filter(|c| c.audio.is_some()).collect();
    clips.sort_by_key(|c| if c.lane == 0 { u8::MAX } else { c.lane });
    assert_eq!(clips.len(), 4, "a clip per pass");
    for (k, c) in clips.iter().enumerate() {
        assert_eq!(c.start, a, "pass {k} begins at the loop's start");
        let got = play_alone(&p, track, c.id, a);
        // Timeline frame `a + j` of pass `k` was rendered at engine frame `k·pass + j`
        // and heard `latency` frames later: what was played against it is the input
        // the engine took then.
        let len = if k < 3 {
            pass as i64
        } else {
            (total - 3 * pass) as i64 - latency
        };
        for j in 0..len {
            let want = k as i64 * pass as i64 + j + latency;
            let at = got.get(j as usize).copied().unwrap_or(0.0);
            assert_eq!(
                which(at),
                Some(want),
                "latency {latency}, block {block}: pass {k}, frame {j}"
            );
        }
    }
}

#[test]
fn every_pass_plays_exactly_what_was_played_against_it() {
    // 1 337 frames is 53.48 ticks: only the clip's fine offset can place it.
    for (latency, block) in [(1_337, 512), (250, 480), (7, 333), (4_801, 1024)] {
        run_pass(latency, block);
    }
}

#[test]
fn a_command_batch_from_a_real_pass_is_one_undo_step() {
    // The same pass through the bus: one undo step takes every clip and the source away.
    let (p, track) = project();
    let mut bus = cantus_core::CommandBus::new(p);
    let before = bus.project().clone();
    let take = cantus_engine::FinishedTake {
        track,
        path: std::path::PathBuf::from("C:/rec/v.wav"),
        sample_rate: SR,
        channels: 1,
        frames: 48_000,
        start_tick: -53.48,
        wraps: vec![],
        wrap_shifts: Vec::new(),
        dropped: 0,
    };
    let mut next = bus.project().next_id;
    let mut ids = || {
        next += 1;
        next - 1
    };
    let cmds = record_pass::build(
        bus.project(),
        &PassPlan::new(Ticks::ZERO),
        &[take],
        &[],
        1_337,
        &mut ids,
    )
    .unwrap();
    bus.reserve_ids(next);
    let clip = cmds.iter().find_map(|c| match c {
        Command::InsertClip { clip, .. } => Some(clip.clone()),
        _ => None,
    });
    let clip = clip.unwrap();
    // Frame 0 belongs 53.48 ticks before zero: the window opens at zero with an offset
    // of 53 ticks, and the file moves 12 frames (0.48 tick) under it.
    assert_eq!((clip.start, clip.offset), (Ticks::ZERO, Ticks(53)));
    assert_eq!(clip.audio.as_ref().unwrap().slip_frames, 12);
    bus.execute(
        Command::Batch {
            label_key: "cmd.record_take".into(),
            commands: cmds,
        },
        cantus_core::Origin::User,
    )
    .unwrap();
    bus.undo();
    assert_eq!(bus.project().tracks, before.tracks);
    assert!(bus.project().audio_pool.is_empty());
}

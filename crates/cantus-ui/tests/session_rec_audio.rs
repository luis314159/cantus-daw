//! N6 (ADR 0076 §8): an audio take recorded into a slot plays on seamlessly — what its file
//! holds while it is still being written (the writer brings the header up to date every
//! half second) is the take's beginning to the sample, placed where the whole take will
//! be: the clip laid ahead from it plays, from the recording's end, what the whole take
//! plays there. A real pass through the engine's recorder (a processor driven by hand).

use cantus_core::audio::TrackInput;
use cantus_core::{AudioRegion, AudioSourceId, Project, Snapshot, Ticks, TrackId};
use cantus_engine::{render, EngineHandle, FinishedTake, RenderOptions, SampleCache};
use cantus_ui::record_pass::{audio_windows, source_of, PassPlan};
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;

fn project() -> (Project, TrackId) {
    let mut p = Project::new("slot take");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    t.strip = None;
    let id = t.id;
    p.tracks.push(t);
    (p, id)
}

/// The first `frames` frames of the WAV at `from`, copied to `to` (what the conductor
/// does: `session_play_rec::copy_head`).
fn copy_head(from: &Path, to: &Path) -> u64 {
    let mut r = hound::WavReader::open(from).unwrap();
    let spec = r.spec();
    let frames = u64::from(r.duration());
    let mut w = hound::WavWriter::create(to, spec).unwrap();
    for x in r.samples::<f32>().take(frames as usize) {
        w.write_sample(x.unwrap()).unwrap();
    }
    w.finalize().unwrap();
    frames
}

/// The take's clip over `[from, until)` playing source `sid` (`take_clip`'s steps).
fn clip_of(
    p: &mut Project,
    take: &FinishedTake,
    from: Ticks,
    until: Ticks,
    sid: AudioSourceId,
    latency: i64,
) -> cantus_core::Clip {
    let map = p.tempo_map();
    let mut plan = PassPlan::new(from);
    plan.audio_tracks = vec![take.track];
    plan.punch = Some((from, until));
    let w = *audio_windows(take, &plan, &map, latency).last().unwrap();
    let (start, length, offset) = w.ticks().unwrap();
    let mut c = p.make_clip("take", Ticks::ZERO, length);
    let mut region = AudioRegion::of(sid);
    region.slip_frames = w.slip_frames_in(start, offset, &map, take.sample_rate);
    c.audio = Some(region);
    c.offset = offset;
    c.content_length = Some(length);
    c
}

/// What `clip` (looping, placed at `at`) plays over `[at, at + frames)`, alone.
fn play(
    p: &Project,
    track: TrackId,
    mut clip: cantus_core::Clip,
    at: Ticks,
    frames: usize,
) -> Vec<f32> {
    let mut q = p.clone();
    clip.start = at;
    q.track_mut(track).unwrap().clips = vec![clip];
    let mut s = Snapshot::compile(&q, 1);
    assert!(SampleCache::new().attach_audio(&mut s, SR).is_empty());
    let opts = RenderOptions {
        sample_rate: SR,
        start: at,
        end: Some(at + Ticks::from_seconds(frames as f64 / f64::from(SR), 120.0)),
        tail_seconds: 0.0,
        block_frames: 512,
        ..Default::default()
    };
    let out = render(Arc::new(s), &opts).unwrap().samples;
    out.chunks_exact(2).map(|f| f[0]).take(frames).collect()
}

#[test]
fn an_audio_take_laid_ahead_plays_what_the_whole_take_plays() {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, track) = project();
    let (mut h, mut proc) = EngineHandle::manual(f64::from(SR));
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 4);
    proc.attach_input(consumer, 1);
    let snap = h.compile(&p, 1);
    h.set_snapshot(snap);
    h.set_record_offset(1_337);
    h.seek(Ticks::ZERO);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    let mut out = vec![0f32; 512 * 2];
    let mut n = 0usize;
    let mut feed = |h: &mut EngineHandle, proc: &mut cantus_engine::Processor, frames: usize| {
        for _ in 0..frames / 512 {
            for _ in 0..512 {
                n += 1;
                // A signal that says which input frame it is.
                ring.push(((n * 7_919) % 1_000) as f32 / 1_000.0 - 0.5)
                    .unwrap();
            }
            proc.process(&mut out, 2);
            h.poll();
        }
    };
    // Two seconds recorded, the writer given time to bring its header up to date.
    feed(&mut h, &mut proc, 2 * SR as usize);
    std::thread::sleep(std::time::Duration::from_millis(700));
    let growing = h.growing_takes();
    assert_eq!(growing.len(), 1);
    let g = growing[0].clone();
    assert!(
        g.frames > 0 && g.frames <= 2 * u64::from(SR),
        "{}",
        g.frames
    );
    let head = dir.path().join("head.wav");
    let frames = copy_head(&g.path, &head);
    assert_eq!(frames, g.frames);
    // Two more seconds, and the pass ends.
    feed(&mut h, &mut proc, 2 * SR as usize);
    h.set_recording(false);
    h.stop();
    proc.process(&mut out, 2);
    h.poll();
    let takes = h.finish_recording();
    let whole = takes[0].clone();
    assert_eq!(whole.start_tick, g.start_tick, "placed alike");
    // The slot recorded beats 1 to 7: the clip laid ahead from the
    // head, and the whole take's.
    let (from, until) = (Ticks::beats(1), Ticks::beats(7));
    let latency = h.pass_latency().total();
    let ahead_take = FinishedTake {
        frames: g.frames + u64::from(SR) * 3600,
        ..g.clone()
    };
    let (ha, wa) = (AudioSourceId(p.alloc_id()), AudioSourceId(p.alloc_id()));
    p.audio_pool.push(source_of(
        &FinishedTake {
            path: head.clone(),
            frames,
            ..g.clone()
        },
        ha,
    ));
    p.audio_pool.push(source_of(&whole, wa));
    let ahead = clip_of(&mut p, &ahead_take, from, until, ha, latency);
    let real = clip_of(&mut p, &whole, from, until, wa, latency);
    assert_eq!(
        (
            ahead.offset,
            ahead.content_length,
            ahead.audio.as_ref().unwrap().slip_frames
        ),
        (
            real.offset,
            real.content_length,
            real.audio.as_ref().unwrap().slip_frames
        ),
        "the same window"
    );
    // Launched at the recording's end: its first 200 ms, the same samples.
    let a = play(&p, track, ahead, until, SR as usize / 5);
    let b = play(&p, track, real, until, SR as usize / 5);
    assert!(b.iter().any(|x| *x != 0.0));
    assert_eq!(a, b, "seamless: the head plays what the whole take plays");
}

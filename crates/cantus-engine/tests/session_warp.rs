//! The session's warped audio (ADR 0076 §7, N5): the warp renders a launch plays are asked
//! for when the song is compiled, before any launch — the first launch of a warped clip
//! plays its render (a 100 BPM loop in a 120 BPM song in time, at its pitch), not the
//! varispeed bridge.

use cantus_core::session::play::{plan, project_play, Input, Timed};
use cantus_core::session::*;
use cantus_core::warp::Warp;
use cantus_core::*;
use cantus_dsp::beat::testkit::drum_loop;
use cantus_engine::*;
use std::path::Path;

const BAR: Ticks = Ticks::beats(4);

fn write_wav(path: &Path, x: &[f32], rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in x {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

/// A 120 BPM song whose session holds a two-bar 100 BPM loop, warped to follow the tempo,
/// looping, in its first slot; nothing in the arrangement.
fn song(path: &Path, frames: usize) -> (Project, SlotRef) {
    let mut p = Project::new("warped session");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Loop");
    t.strip = None;
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "loop".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: frames as u64,
        sha256: None,
        recorded: false,
    });
    let mut c = p.make_audio_clip("loop", id, Ticks::ZERO, BAR * 2);
    c.audio.as_mut().unwrap().warp = Some(Box::new(Warp::identity(48_000, 100.0, 0)));
    c.content_length = Some(BAR * 2);
    let scene = SceneId(p.alloc_id());
    p.session.scenes.push(Scene::new(scene, "A"));
    t.slots.push(Slot {
        scene,
        clip: Some(SessionClip::new(c)),
        stop: true,
    });
    let slot = SlotRef::new(t.id, scene);
    p.tracks.push(t);
    (p, slot)
}

fn settle(cache: &mut SampleCache) {
    let t = std::time::Instant::now();
    while cache.loading() && t.elapsed() < std::time::Duration::from_secs(120) {
        cache.poll_loaded();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!cache.loading());
}

/// N5: compiled before any launch, the song asks for its warped session clip's render; the
/// first projection of a launch (on the third bar: where it lands does not matter) plays
/// that render — the final one, frame for frame — not the bridge.
#[test]
fn the_first_launch_of_a_warped_session_clip_plays_its_render() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop 100.wav");
    let m = drum_loop(48_000.0, 100.0, 2, 0.0, 0.0, 5);
    write_wav(&path, &m.audio, 48_000);
    let (p, slot) = song(&path, m.audio.len());
    let mut cache = SampleCache::new();
    cache.set_background(true);
    cache.set_pitch_background(true);
    // The song as compiled before any launch (`EngineHandle::compile`'s steps), and again
    // when its file has arrived (the application compiles again when a load arrives:
    // `SampleCache::poll_loaded`).
    for rev in 1..=2 {
        let mut s = Snapshot::compile(&p, rev);
        cache.attach_audio(&mut s, 48_000);
        cache.preload_session(&p, 48_000);
        cache.preload_session_warps(&p, 48_000);
        settle(&mut cache);
    }
    // Launched on bar 3: its first projection.
    let launch = Input::Launch {
        slot,
        velocity: 1.0,
    };
    let perf = plan(&p, &[Timed::at(BAR * 2, launch)], BAR * 6).perf;
    let heard = project_play(&p, &perf);
    let mut first = Snapshot::compile(&heard, 3);
    cache.attach_audio(&mut first, 48_000);
    assert!(!cache.loading(), "nothing asked for: it was made ahead");
    let got = first.tracks[0].audio[0].data.clone().expect("it plays");
    // The final render, made at once by a manual cache.
    let mut want = Snapshot::compile(&heard, 3);
    SampleCache::new().attach_audio(&mut want, 48_000);
    let want = want.tracks[0].audio[0].data.clone().unwrap();
    assert_ne!(want.len_frames(), m.audio.len(), "a render, not the source");
    assert_eq!(got.frames[..], want.frames[..], "its render, not a bridge");
}

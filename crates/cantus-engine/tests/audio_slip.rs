//! A clip's fine offset (`AudioRegion::slip_frames`, ADR 0025 §7): the content moves by
//! source frames under a window that stays in whole ticks, so a take sits where it was
//! played to the sample; and a split plays on without a seam at any tempo.

use cantus_core::*;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;

fn ramp(frames: usize) -> Vec<f32> {
    // Every frame its own value, exactly representable, so a frame's index can be read
    // back from the output.
    (0..frames).map(|i| (i + 1) as f32 / 65_536.0).collect()
}

fn write_f32(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in samples {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

fn project(bpm: f64, path: &Path, frames: u64) -> (Project, AudioSourceId) {
    let mut p = Project::new("slip");
    p.tempo_bpm = bpm;
    let mut t = p.make_track("Audio");
    t.strip = None;
    p.tracks.push(t);
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "ramp".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames,
        sha256: None,
        recorded: true,
    });
    (p, id)
}

fn left(p: &Project) -> Vec<f32> {
    let mut s = Snapshot::compile(p, 1);
    assert!(SampleCache::new().attach_audio(&mut s, SR).is_empty());
    let opts = RenderOptions {
        sample_rate: SR,
        tail_seconds: 0.0,
        block_frames: 333,
        ..Default::default()
    };
    let out = render(Arc::new(s), &opts).unwrap().samples;
    out.chunks_exact(2).map(|f| f[0]).collect()
}

/// The file frame each output frame plays (`None` for silence).
fn frames_played(out: &[f32]) -> Vec<Option<i64>> {
    out.iter()
        .map(|v| (*v != 0.0).then(|| (*v * 65_536.0).round() as i64 - 1))
        .collect()
}

#[test]
fn the_fine_offset_moves_the_content_by_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp.wav");
    let x = ramp(20_000);
    write_f32(&path, &x);
    for slip in [0i32, 7, -7, 12, -13] {
        let (mut p, id) = project(120.0, &path, x.len() as u64);
        // A window from tick 100 (frame 2 500) showing content from tick 10 (frame 250).
        let mut c = p.make_audio_clip("c", id, Ticks(100), Ticks(400));
        c.offset = Ticks(10);
        c.audio.as_mut().unwrap().slip_frames = slip;
        p.tracks[0].clips.push(c);
        let out = frames_played(&left(&p));
        assert_eq!(out[2_499], None);
        assert_eq!(out[2_500], Some(250 + slip as i64), "slip {slip}");
        assert_eq!(out[7_000], Some(250 + 4_500 + slip as i64), "slip {slip}");
        assert_eq!(out.get(12_500).copied().flatten(), None, "the window ends");
    }
}

#[test]
fn a_split_plays_on_without_a_seam_at_any_tempo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp.wav");
    let x = ramp(60_000);
    write_f32(&path, &x);
    for bpm in [120.0, 97.3, 133.0] {
        for slip in [0, 5, -9] {
            let (mut whole, id) = project(bpm, &path, x.len() as u64);
            let mut c = whole.make_audio_clip("c", id, Ticks(37), Ticks(1_500));
            c.offset = Ticks(11);
            c.audio.as_mut().unwrap().slip_frames = slip;
            let mut split = whole.clone();
            whole.tracks[0].clips.push(c.clone());
            // Split at an awkward tick, as `clips::split` does: the right half's window
            // starts later and its offset moves with it.
            let at = Ticks(37 + 641);
            let mut l = c.clone();
            l.length = at - c.start;
            let mut r = c.clone();
            r.id = ClipId(split.alloc_id());
            r.start = at;
            r.length = c.end() - at;
            r.offset = c.offset + (at - c.start);
            split.tracks[0].clips.push(l);
            split.tracks[0].clips.push(r);
            let (a, b) = (left(&whole), left(&split));
            assert_eq!(a, b, "{bpm} BPM, slip {slip}: the split is seamless");
            // And the content runs one frame per frame across it.
            let played: Vec<i64> = frames_played(&b).into_iter().flatten().collect();
            assert!(played.windows(2).all(|w| w[1] == w[0] + 1), "{bpm} {slip}");
        }
    }
}

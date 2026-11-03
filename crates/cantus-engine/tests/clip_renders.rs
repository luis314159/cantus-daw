//! Renders made outside the engine played in place of a clip's source (round 36, ADR 0097:
//! an ARA plug-in's render of a bound clip). A render equal to its source plays the source to
//! the bit — at the source's rate and at another (the same resampler converts both) — and at
//! every block size; a render that differs is what plays; a clip with none plays its source.

use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

fn signal(frames: usize, seed: u32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    (0..frames * 2)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 1.6 - 0.8
        })
        .collect()
}

fn write(path: &Path, rate: u32, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in samples {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

/// A project with one audio clip over `path` (a file at `rate`): (project, clip).
fn project(path: &Path, rate: u32, frames: u64) -> (Project, ClipId) {
    let mut p = Project::new("renders");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Audio");
    t.strip = None;
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "src".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: rate,
        channels: 2,
        frames,
        sha256: None,
        recorded: false,
    });
    let c = p.make_audio_clip("clip", id, Ticks::beats(1), Ticks::beats(2));
    let cid = c.id;
    t.clips.push(c);
    p.tracks.push(t);
    (p, cid)
}

fn render_with(p: &Project, renders: HashMap<ClipId, ClipRender>, block: usize) -> Vec<f32> {
    let mut s = Snapshot::compile(p, 1);
    let mut cache = SampleCache::new();
    cache.set_clip_renders(renders);
    let errors = cache.attach_audio(&mut s, 48_000);
    assert!(errors.is_empty(), "{errors:?}");
    let opts = RenderOptions {
        sample_rate: 48_000,
        tail_seconds: 0.1,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(s), &opts).unwrap().samples
}

#[test]
fn a_render_equal_to_its_source_plays_the_source_to_the_bit_at_any_rate_and_block() {
    for rate in [48_000u32, 44_100] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let frames = 40_000;
        write(&path, rate, &signal(frames, 3));
        let (p, clip) = project(&path, rate, frames as u64);
        let plain = render_with(&p, HashMap::new(), 512);
        let same = Arc::new(SampleCache::decode_source(&path).unwrap());
        assert_eq!(same.rate, rate, "decoded at its own rate");
        for block in [512usize, 333, 64] {
            let out = render_with(&p, HashMap::from([(clip, whole(same.clone()))]), block);
            assert_null(&plain, &out, 0.0, &format!("{rate} Hz, block {block}"));
        }
    }
}

#[test]
fn a_render_that_differs_is_what_plays_and_other_clips_keep_their_sources() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let frames = 40_000;
    write(&path, 48_000, &signal(frames, 5));
    let (p, clip) = project(&path, 48_000, frames as u64);
    let plain = render_with(&p, HashMap::new(), 512);
    let src = SampleCache::decode_source(&path).unwrap();
    let half: Vec<f32> = src.frames.iter().map(|x| x * 0.5).collect();
    let r = Arc::new(SampleData {
        frames: half.into(),
        ..src
    });
    let out = render_with(&p, HashMap::from([(clip, whole(r))]), 512);
    let want: Vec<f32> = plain.iter().map(|x| x * 0.5).collect();
    assert_null(&want, &out, 0.0, "half the source plays as half");
    // A render for a clip that is not there changes nothing.
    let other = render_with(
        &p,
        HashMap::from([(
            ClipId(u64::MAX),
            whole(Arc::new(SampleCache::decode_source(&path).unwrap())),
        )]),
        512,
    );
    assert_null(&plain, &other, 0.0, "no render: the source");
}

/// A render of the whole source, from its start.
fn whole(d: Arc<SampleData>) -> ClipRender {
    let frames = (d.frames.len() / d.channels.max(1) as usize) as i64;
    ClipRender {
        start: 0.0,
        source_frames: (0, frames),
        source_rate: d.rate,
        at_source_rate: d.frames.clone(),
        data: d,
    }
}

/// A render of only a stretch of the source (what the host makes: the clip's window and a
/// margin), placed by its start: it plays exactly what the source would there.
#[test]
fn a_render_of_a_stretch_placed_by_its_start_plays_the_source_to_the_bit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let frames = 60_000;
    write(&path, 48_000, &signal(frames, 9));
    let (p, clip) = project(&path, 48_000, frames as u64);
    let plain = render_with(&p, HashMap::new(), 512);
    let src = SampleCache::decode_source(&path).unwrap();
    // The clip (two beats at 120 BPM from the file's start) reads frames 0..48 000: a stretch
    // from 1 000 to 50 000 holds all but its first 1 000, which then read nothing.
    let (first, end) = (1_000usize, 50_000usize);
    let part: Arc<[f32]> = src.frames[first * 2..end * 2].into();
    let r = ClipRender {
        data: Arc::new(SampleData {
            rate: 48_000,
            channels: 2,
            frames: part.clone(),
            stream: None,
        }),
        start: first as f64 / 48_000.0,
        source_frames: (first as i64, end as i64),
        source_rate: 48_000,
        at_source_rate: part,
    };
    let out = render_with(&p, HashMap::from([(clip, r)]), 333);
    // The clip starts at beat 1 (24 000 frames); its first 1 000 source frames are outside.
    let at = 24_000 * 2;
    assert!(
        out[at..at + 2_000].iter().all(|x| *x == 0.0),
        "before the stretch: nothing"
    );
    assert_null(
        &plain[at + 2_000..at + 96_000],
        &out[at + 2_000..at + 96_000],
        0.0,
        "inside: the source",
    );
}

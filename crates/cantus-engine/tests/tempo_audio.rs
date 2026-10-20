//! Unwarped audio clips under a tempo map (round 29, ADR 0060 §5; review 1, finding 17):
//! the engine plays each clip at its natural speed anchored at its start, the offset read at
//! the tempo there — sample for sample what `cantus_core::audio_time` (the interface's
//! mapping) says — the same from wherever playback started, at every block size, with its
//! fades measured through the map.

use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;

fn signal(frames: usize, seed: u32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    (0..frames)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 1.6 - 0.8
        })
        .collect()
}

fn write_mono(path: &Path, samples: &[f32]) {
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

fn pt(beats: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks::beats(beats),
        bpm,
        ramp,
    }
}

/// One track straight to the master, a tempo track: 120, a ramp to 140 from beat 2 to beat
/// 6, held, then 90 from beat 8.
fn project(path: &Path, frames: usize) -> (Project, AudioSourceId) {
    let mut p = Project::new("audio");
    p.tempo_bpm = 120.0;
    p.loop_enabled = false;
    p.tempo_points = vec![
        pt(0, 120.0, false),
        pt(2, 120.0, true),
        pt(6, 140.0, false),
        pt(8, 90.0, false),
    ];
    let mut t = p.make_track("Audio");
    t.strip = None;
    p.tracks.push(t);
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "src".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames: frames as u64,
        sha256: None,
        recorded: false,
    });
    (p, id)
}

fn render_at(p: &Project, start: Ticks, block: usize) -> Vec<f32> {
    let mut s = Snapshot::compile(p, 1);
    let errors = SampleCache::new().attach_audio(&mut s, SR);
    assert!(errors.is_empty(), "{errors:?}");
    let opts = RenderOptions {
        sample_rate: SR,
        start,
        tail_seconds: 0.1,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(s), &opts)
        .unwrap()
        .samples
        .chunks_exact(2)
        .map(|f| f[0])
        .collect()
}

#[test]
fn an_unwarped_clip_plays_where_audio_time_says_through_a_ramp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let frames = 600_000;
    let file = signal(frames, 7);
    write_mono(&path, &file);
    let (mut p, src) = project(&path, frames);
    // From beat 1 (inside the held 120) across the ramp, a beat into its file.
    let mut c = p.make_audio_clip("a", src, Ticks::beats(1), Ticks::beats(9));
    c.offset = Ticks::beats(1);
    p.tracks[0].clips.push(c);
    let out = render_at(&p, Ticks::ZERO, 64);
    let map = p.tempo_map();
    let (start, offset) = (Ticks::beats(1), Ticks::beats(1));
    let first = map.sample_at(start.0 as f64, SR as f64).round() as usize;
    let last = map.sample_at(Ticks::beats(10).0 as f64, SR as f64).round() as usize;
    let mut checked = 0;
    for k in (first + 2..last - 2).step_by(97) {
        // The interface's frame at this sample's tick, and the engine's.
        let tick = map.tick_at_sample(k as f64, SR as f64);
        let ui = cantus_core::audio_time::frame_at(&map, start, offset, 0.0, SR as f64, tick);
        let lo = (ui.floor() as i64 - 1).max(0) as usize;
        let f = (lo..lo + 4)
            .filter(|f| *f < frames)
            .find(|f| {
                file[*f].to_bits() == out[k].to_bits() && (*f as f64 - ui).abs() <= 0.5 + 1e-9
            });
        assert!(
            f.is_some(),
            "sample {k}: the engine plays no frame within half of {ui}"
        );
        checked += 1;
    }
    assert!(checked > 1000, "{checked}");
}

#[test]
fn a_clip_on_a_tempo_point_plays_the_same_from_wherever_playback_started() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let frames = 300_000;
    let file = signal(frames, 9);
    write_mono(&path, &file);
    let (mut p, src) = project(&path, frames);
    // At beat 8, exactly on the step to 90, a beat into its file: 2/3 s at 90 BPM.
    let mut c = p.make_audio_clip("b", src, Ticks::beats(8), Ticks::beats(2));
    c.offset = Ticks::beats(1);
    p.tracks[0].clips.push(c);
    let want = file[32_000];
    let from_start = render_at(&p, Ticks::ZERO, 256);
    let at = p
        .tempo_map()
        .sample_at(Ticks::beats(8).0 as f64, SR as f64)
        .round() as usize;
    assert_eq!(
        from_start[at].to_bits(),
        want.to_bits(),
        "played from the start"
    );
    let from_point = render_at(&p, Ticks::beats(8), 256);
    assert_eq!(
        from_point[0].to_bits(),
        want.to_bits(),
        "played from the point"
    );
}

#[test]
fn audio_under_a_map_is_the_same_at_every_block_size_and_its_fades_are_measured_through_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let frames = 600_000;
    let file = signal(frames, 11);
    write_mono(&path, &file);
    let (mut p, src) = project(&path, frames);
    let mut c = p.make_audio_clip("c", src, Ticks::beats(1), Ticks::beats(8));
    {
        let a = c.audio.as_mut().unwrap();
        // A fade in over the ramp's start, a fade out over the step to 90.
        a.fade_in = Ticks::beats(2);
        a.fade_out = Ticks::beats(2);
    }
    p.tracks[0].clips.push(c);
    let reference = render_at(&p, Ticks::ZERO, 64);
    for block in [1usize, 37, 512] {
        let other = render_at(&p, Ticks::ZERO, block);
        assert!(
            reference
                .iter()
                .zip(&other)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "block {block}"
        );
    }
    // The fade in ends at the sample the map puts beat 3 on: from there the file plays at
    // unity, just before it does not.
    let map = p.tempo_map();
    let s1 = map.sample_at(Ticks::beats(1).0 as f64, SR as f64).round() as usize;
    let s3 = map.sample_at(Ticks::beats(3).0 as f64, SR as f64).round() as usize;
    let len = s3 - s1;
    for k in [s3 + 1, s3 + 500, s3 + 5_000] {
        assert_eq!(
            reference[k].to_bits(),
            file[k - s1].to_bits(),
            "after the fade, {k}"
        );
    }
    let k = s1 + len / 2;
    assert!(
        reference[k].abs() < file[k - s1].abs() || file[k - s1] == 0.0,
        "inside it"
    );
    // Review 2 (finding 22): the fade out begins where the map puts beat 7 — unity just
    // before, attenuated from there (the constant tempo would put it hundreds of samples
    // away).
    let s7 = map.sample_at(Ticks::beats(7).0 as f64, SR as f64);
    let first = (s7 as usize - 400..s7 as usize + 400)
        .find(|k| reference[*k].to_bits() != file[*k - s1].to_bits())
        .expect("a fade out");
    assert!(
        (first as f64 - s7).abs() <= 1.0,
        "the fade out begins at {first}, the map's beat 7 at {s7}"
    );
}

/// Review 2 (finding 22): a clip that starts inside the ramp — its offset read at the
/// ramp's tempo there, its fine offset added — plays at its first sample the frame by hand
/// (one beat at the tempo at beat 3.5, plus 37 frames), and at every sample the frame
/// `audio_time` says to within half a frame: played from the start and from a seek into
/// the ramp alike.
#[test]
fn a_clip_inside_the_ramp_plays_where_audio_time_says_from_anywhere() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let frames = 600_000;
    let file = signal(frames, 13);
    write_mono(&path, &file);
    let (mut p, src) = project(&path, frames);
    let start = Ticks(3 * 960 + 480);
    let mut c = p.make_audio_clip("r", src, start, Ticks::beats(4));
    c.offset = Ticks::beats(1);
    c.audio.as_mut().unwrap().slip_frames = 37;
    p.tracks[0].clips.push(c);
    let map = p.tempo_map();
    let sr = SR as f64;
    let at_start =
        cantus_core::audio_time::frame_at(&map, start, Ticks::beats(1), 37.0, sr, start.0 as f64);
    let by_hand = 60.0 / map.bpm_at(start.0 as f64) * sr + 37.0;
    assert!((at_start - by_hand).abs() < 1e-6, "{at_start} vs {by_hand}");
    let check = |out: &[f32], from: Ticks, what: &str| {
        let mut n = 0;
        let end = map.seconds_between(from, start + Ticks::beats(4)) * sr;
        let first = (map.seconds_between(from, start) * sr).max(0.0);
        for k in (first.ceil() as usize + 1..end as usize - 1).step_by(53) {
            let tick = map.tick_after_seconds(from.0 as f64, k as f64 / sr);
            let ui = cantus_core::audio_time::frame_at(&map, start, Ticks::beats(1), 37.0, sr, tick);
            let lo = (ui.floor() as i64 - 1).max(0) as usize;
            let f = (lo..lo + 4).find(|f| {
                file[*f].to_bits() == out[k].to_bits() && (*f as f64 - ui).abs() <= 0.5 + 1e-9
            });
            assert!(f.is_some(), "{what}, sample {k}: no frame within half of {ui}");
            n += 1;
        }
        assert!(n > 500, "{what}: {n}");
    };
    check(&render_at(&p, Ticks::ZERO, 64), Ticks::ZERO, "from the start");
    check(&render_at(&p, Ticks::beats(4), 128), Ticks::beats(4), "from beat 4");
}

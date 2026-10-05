//! Audio clips in the engine (ADR 0025 §1, §9): integer-sample playback from memory, so a
//! clip at 0 dB with no fades plays its file bit for bit; gain, fades, reverse, offsets;
//! block-size independence with loops; offline equal to real time; the cache's loading,
//! conversion and missing files.

use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;
/// At 120 BPM and 48 kHz one tick is exactly 25 frames.
const FPT: usize = 25;

/// A deterministic noise-like signal, interleaved.
fn signal(frames: usize, channels: usize, seed: u32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    (0..frames * channels)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 1.6 - 0.8
        })
        .collect()
}

fn write_f32(path: &Path, channels: u16, rate: u32, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels,
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

/// A project with one empty track (its instrument never plays) going straight to the
/// master, so what the clips play reaches the output untouched.
fn project() -> Project {
    let mut p = Project::new("audio");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Audio");
    t.strip = None;
    p.tracks.push(t);
    p
}

fn add_source(p: &mut Project, path: &Path, channels: u8, frames: u64) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "src".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels,
        frames,
        sha256: None,
        recorded: false,
    });
    id
}

fn add_clip(p: &mut Project, src: AudioSourceId, start: Ticks, length: Ticks) -> &mut Clip {
    let c = p.make_audio_clip("clip", src, start, length);
    p.tracks[0].clips.push(c);
    p.tracks[0].clips.last_mut().unwrap()
}

fn region(c: &mut Clip) -> &mut AudioRegion {
    c.audio.as_mut().unwrap()
}

fn snapshot(p: &Project) -> Arc<Snapshot> {
    let mut s = Snapshot::compile(p, 1);
    let errors = SampleCache::new().attach_audio(&mut s, SR);
    assert!(errors.is_empty(), "{errors:?}");
    Arc::new(s)
}

fn render_at(p: &Project, start: Ticks, block: usize) -> Vec<f32> {
    let opts = RenderOptions {
        sample_rate: SR,
        start,
        tail_seconds: 0.1,
        block_frames: block,
        ..Default::default()
    };
    render(snapshot(p), &opts).unwrap().samples
}

fn render_of(p: &Project) -> Vec<f32> {
    render_at(p, Ticks::ZERO, 512)
}

/// Left and right of interleaved stereo.
fn chan(v: &[f32], c: usize) -> Vec<f32> {
    v.chunks_exact(2).map(|f| f[c]).collect()
}

#[test]
fn a_clip_at_0_db_plays_its_file_bit_exact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let frames = 30_000;
    let file = signal(frames, 2, 1);
    write_f32(&path, 2, SR, &file);
    let mut p = project();
    let src = add_source(&mut p, &path, 2, frames as u64);
    // At beat 1, and longer than the file: the rest is silence.
    add_clip(&mut p, src, Ticks::beats(1), Ticks::beats(2));
    let out = render_of(&p);
    let at = 960 * FPT;
    for (k, f) in out.chunks_exact(2).enumerate() {
        let want = if k >= at && k < at + frames {
            [file[(k - at) * 2], file[(k - at) * 2 + 1]]
        } else {
            [0.0, 0.0]
        };
        assert_eq!(
            [f[0].to_bits(), f[1].to_bits()],
            [want[0].to_bits(), want[1].to_bits()],
            "frame {k}"
        );
    }
}

#[test]
fn a_clip_reaches_its_strip_bit_exact() {
    // Through a strip: the track stem (what the track hands its strip) is the file.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let file = signal(10_000, 2, 2);
    write_f32(&path, 2, SR, &file);
    let mut p = project();
    p.tracks[0].strip = Some(p.strips[0].id);
    let src = add_source(&mut p, &path, 2, 10_000);
    add_clip(&mut p, src, Ticks::ZERO, Ticks::beats(1));
    let opts = StemOptions {
        render: RenderOptions {
            sample_rate: SR,
            tail_seconds: 0.0,
            ..Default::default()
        },
        keep_silent: true,
        ..Default::default()
    };
    let spec = StemSpec {
        source: StemSource::Track(0),
        file_name: "t.wav".into(),
    };
    let stems = render_stems(snapshot(&p), &opts, &[spec]).unwrap();
    assert_eq!(&stems[0].samples[..file.len()], &file[..]);
}

#[test]
fn a_mono_file_plays_into_both_sides() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.wav");
    let file = signal(5_000, 1, 3);
    write_f32(&path, 1, SR, &file);
    let mut p = project();
    let src = add_source(&mut p, &path, 1, 5_000);
    add_clip(&mut p, src, Ticks::ZERO, Ticks::beats(1));
    let out = render_of(&p);
    assert_eq!(&chan(&out, 0)[..5_000], &file[..]);
    assert_eq!(&chan(&out, 1)[..5_000], &file[..]);
}

#[test]
fn fades_are_exactly_zero_and_one_at_their_ends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dc.wav");
    let frames = 48_000;
    write_f32(&path, 1, SR, &vec![0.5; frames]);
    for curve in FadeCurve::ALL {
        let mut p = project();
        let src = add_source(&mut p, &path, 1, frames as u64);
        // 1 s clip (1920 ticks) at beat 1, a 100-tick (2 500-frame) fade each side.
        let c = add_clip(&mut p, src, Ticks::beats(1), Ticks(1920));
        let a = region(c);
        a.fade_in = Ticks(100);
        a.fade_out = Ticks(100);
        a.fade_in_curve = curve;
        a.fade_out_curve = curve;
        let l = chan(&render_of(&p), 0);
        let (s, len, f) = (960 * FPT, 1920 * FPT, 100 * FPT);
        assert_eq!(l[s - 1], 0.0, "{curve:?}: before the clip");
        assert_eq!(l[s], 0.0, "{curve:?}: the fade in's first frame");
        assert_eq!(l[s + f], 0.5, "{curve:?}: the fade in's end");
        assert_eq!(l[s + len / 2], 0.5, "{curve:?}: the middle");
        assert_eq!(l[s + len - 1 - f], 0.5, "{curve:?}: the fade out's start");
        assert_eq!(l[s + len - 1], 0.0, "{curve:?}: the fade out's last frame");
        assert_eq!(l[s + len], 0.0, "{curve:?}: after the clip");
        // Rising and falling the whole way.
        for k in 0..f {
            assert!(l[s + k] <= l[s + k + 1], "{curve:?} in at {k}");
            assert!(
                l[s + len - 1 - k] <= l[s + len - 2 - k],
                "{curve:?} out at {k}"
            );
        }
    }
}

#[test]
fn fades_that_would_overlap_are_shortened_in_proportion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dc.wav");
    write_f32(&path, 1, SR, &vec![0.5; 20_000]);
    let mut p = project();
    let src = add_source(&mut p, &path, 1, 20_000);
    // 200 ticks long, fades of 300 and 100: they become 150 and 50.
    let c = add_clip(&mut p, src, Ticks::ZERO, Ticks(200));
    region(c).fade_in = Ticks(300);
    region(c).fade_out = Ticks(100);
    let l = chan(&render_of(&p), 0);
    let len = 200 * FPT;
    assert_eq!(l[0], 0.0);
    assert_eq!(l[len - 1], 0.0);
    let top = (0..len).max_by(|a, b| l[*a].total_cmp(&l[*b])).unwrap();
    assert!(
        (top as i64 - (150 * FPT) as i64).abs() <= 1,
        "the fades meet at {top}"
    );
}

#[test]
fn gain_offset_and_reverse() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.wav");
    let file = signal(20_000, 1, 4);
    write_f32(&path, 1, SR, &file);
    let mut p = project();
    let src = add_source(&mut p, &path, 1, 20_000);
    // 100 ticks of the file from content tick 200 (frame 5 000), at -6 dB, backwards.
    let c = add_clip(&mut p, src, Ticks(40), Ticks(100));
    c.offset = Ticks(200);
    region(c).gain_db = -6.0;
    region(c).reverse = true;
    let g = 10f32.powf(-6.0 / 20.0);
    let l = chan(&render_of(&p), 0);
    let (s, len, off) = (40 * FPT, 100 * FPT, 200 * FPT);
    for j in 0..len {
        assert_eq!(l[s + j], file[off + len - 1 - j] * g, "frame {j}");
    }
    assert_eq!(l[s + len], 0.0);
    assert_eq!(l[s - 1], 0.0);
}

#[test]
fn a_render_from_inside_a_clip_lands_mid_clip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.wav");
    let file = signal(48_000, 2, 5);
    write_f32(&path, 2, SR, &file);
    let mut p = project();
    let src = add_source(&mut p, &path, 2, 48_000);
    add_clip(&mut p, src, Ticks(100), Ticks(1800));
    let whole = render_at(&p, Ticks::ZERO, 512);
    let from = Ticks(777);
    let part = render_at(&p, from, 333);
    let skip = 777 * FPT * 2;
    let n = part.len().min(whole.len() - skip) - 2 * 2_400;
    assert_null(
        &whole[skip..skip + n],
        &part[..n],
        0.0,
        "seek into the clip",
    );
}

/// Several clips: stereo and mono, fades of every kind, gain, reverse, overlapping.
fn busy_project(dir: &Path) -> Project {
    let a = dir.join("a.wav");
    let b = dir.join("b.wav");
    write_f32(&a, 2, SR, &signal(60_000, 2, 6));
    write_f32(&b, 1, SR, &signal(40_000, 1, 7));
    let mut p = project();
    let sa = add_source(&mut p, &a, 2, 60_000);
    let sb = add_source(&mut p, &b, 1, 40_000);
    let c = add_clip(&mut p, sa, Ticks(13), Ticks(2000));
    region(c).fade_in = Ticks(77);
    region(c).fade_out = Ticks(311);
    region(c).fade_in_curve = FadeCurve::EqualPower;
    region(c).fade_out_curve = FadeCurve::SCurve;
    let c = add_clip(&mut p, sb, Ticks(500), Ticks(1200));
    c.offset = Ticks(37);
    region(c).gain_db = 3.5;
    region(c).reverse = true;
    region(c).fade_out = Ticks(90);
    region(c).fade_out_curve = FadeCurve::Exponential;
    let c = add_clip(&mut p, sa, Ticks(1700), Ticks(900));
    c.offset = Ticks(400);
    region(c).fade_in = Ticks(1000);
    region(c).fade_in_curve = FadeCurve::Logarithmic;
    p
}

#[test]
fn playback_is_block_size_independent() {
    let dir = tempfile::tempdir().unwrap();
    let p = busy_project(dir.path());
    let reference = render_at(&p, Ticks::ZERO, 512);
    assert!(reference.iter().any(|x| *x != 0.0));
    for block in [1usize, 7, 64, 480, 1000, 4096, 8192] {
        let r = render_at(&p, Ticks::ZERO, block);
        assert_null(&reference, &r, 0.0, &format!("block size {block}"));
    }
}

/// Drive a processor by hand with a loop on, at a given sequence of block sizes.
fn looped(snap: Arc<Snapshot>, sizes: &[usize], total: usize) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR as f64, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Seek(Ticks(300))).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(total * 2);
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

#[test]
fn loops_wrap_mid_clip_the_same_for_every_block_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = busy_project(dir.path());
    // A loop whose ends fall inside clips and on no sample boundary at this tempo.
    p.tempo_bpm = 131.0;
    p.loop_enabled = true;
    p.loop_region = Some(LoopRegion {
        start: Ticks(250),
        end: Ticks(1777),
    });
    let snap = snapshot(&p);
    let total = 200_000;
    let reference = looped(snap.clone(), &[512], total);
    assert!(reference.iter().any(|x| *x != 0.0));
    for sizes in [
        &[1usize][..],
        &[7],
        &[480],
        &[8192],
        &[128, 333, 17, 1024, 96],
    ] {
        let r = looped(snap.clone(), sizes, total);
        assert_null(&reference, &r, 0.0, &format!("blocks {sizes:?}"));
    }
}

#[test]
fn offline_render_equals_real_time() {
    let dir = tempfile::tempdir().unwrap();
    let p = busy_project(dir.path());
    let snap = snapshot(&p);
    let offline = render(
        snap.clone(),
        &RenderOptions {
            sample_rate: SR,
            tail_seconds: 0.2,
            block_frames: 256,
            ..Default::default()
        },
    )
    .unwrap();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR as f64, rx_engine, tx_engine);
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

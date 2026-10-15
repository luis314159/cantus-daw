//! Comp seams (ADR 0058): where two playing-lane audio windows meet and the right one
//! carries a crossfade, the engine plays an **equal-power** crossfade centred on the seam —
//! each window reading past the seam into its own source, the left fading out over exactly
//! the samples the right fades in on — the same at every block size and offline as live;
//! a seam with no crossfade is the butt join it always was, to the bit.

use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;
/// At 120 BPM and 48 kHz one tick is exactly 25 frames.
const FPT: i64 = 25;

/// A stereo file holding (l, r) in every frame.
fn constant(path: &Path, frames: usize, l: f32, r: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..frames {
        w.write_sample(l).unwrap();
        w.write_sample(r).unwrap();
    }
    w.finalize().unwrap();
}

fn source(p: &mut Project, path: &Path, frames: u64) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 2,
        frames,
        sha256: None,
        recorded: true,
    });
    id
}

/// Two takes of eight beats — the first all left, the second all right — comped: the
/// first take to beat 4, the second after, the seam at beat 4 carrying `xfade`.
fn comped(dir: &Path, xfade: Ticks) -> Project {
    let mut p = Project::new("comp");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Vox");
    t.strip = None;
    p.tracks.push(t);
    let frames = 8 * 960 * FPT as usize;
    let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
    constant(&a, frames, 1.0, 0.0);
    constant(&b, frames, 0.0, 1.0);
    let sa = source(&mut p, &a, frames as u64);
    let sb = source(&mut p, &b, frames as u64);
    let left = p.make_audio_clip("a", sa, Ticks::ZERO, Ticks::beats(4));
    let mut right = p.make_audio_clip("b", sb, Ticks::beats(4), Ticks::beats(4));
    right.offset = Ticks::beats(4);
    right.audio.as_mut().unwrap().xfade = xfade;
    p.tracks[0].clips.extend([left, right]);
    p
}

fn snapshot(p: &Project) -> Arc<Snapshot> {
    let mut s = Snapshot::compile(p, 1);
    let errors = SampleCache::new().attach_audio(&mut s, SR);
    assert!(errors.is_empty(), "{errors:?}");
    Arc::new(s)
}

fn render_with(p: &Project, block: usize) -> Vec<f32> {
    let opts = RenderOptions {
        sample_rate: SR,
        tail_seconds: 0.0,
        block_frames: block,
        ..Default::default()
    };
    render(snapshot(p), &opts).unwrap().samples
}

#[test]
fn a_seam_crossfades_equal_power_over_one_stretch_centred_on_it() {
    let dir = tempfile::tempdir().unwrap();
    let x = Ticks(20); // 500 samples, ~10 ms
    let out = render_with(&comped(dir.path(), x), 512);
    let seam = 4 * 960 * FPT;
    let (from, to) = (seam - 10 * FPT, seam + 10 * FPT);
    let at = |f: i64| (out[2 * f as usize], out[2 * f as usize + 1]);
    // Before: the first take alone; after: the second alone.
    for f in [0, from - 200, from - 1] {
        assert_eq!(at(f), (1.0, 0.0), "frame {f}");
    }
    for f in [to, to + 1, to + 5_000] {
        assert_eq!(at(f), (0.0, 1.0), "frame {f}");
    }
    // Across it: equal power at every sample, the left falling, the right rising.
    let mut last = (1.0f32, 0.0f32);
    for f in from..to {
        let (l, r) = at(f);
        assert!((l * l + r * r - 1.0).abs() < 1e-5, "frame {f}: {l} {r}");
        assert!(l <= last.0 && r >= last.1, "frame {f}: monotone");
        last = (l, r);
    }
    // At the seam itself the two are equal: the crossfade is centred on it.
    let (l, r) = at(seam);
    assert!((l - r).abs() < 0.01, "{l} {r}");
}

#[test]
fn a_seam_without_a_crossfade_is_the_butt_join_it_always_was() {
    let dir = tempfile::tempdir().unwrap();
    let out = render_with(&comped(dir.path(), Ticks::ZERO), 512);
    let seam = (4 * 960 * FPT) as usize;
    assert_eq!((out[2 * (seam - 1)], out[2 * (seam - 1) + 1]), (1.0, 0.0));
    assert_eq!((out[2 * seam], out[2 * seam + 1]), (0.0, 1.0));
    // And a window whose start meets nothing has no seam: its crossfade waits unused.
    let mut p = comped(dir.path(), Ticks(20));
    p.tracks[0].clips.remove(0);
    let alone = render_with(&p, 512);
    assert_eq!(alone[2 * seam + 1], 1.0);
    assert_eq!(alone[2 * (seam - 1) + 1], 0.0);
}

#[test]
fn seams_render_the_same_at_every_block_size_and_live() {
    let dir = tempfile::tempdir().unwrap();
    let p = comped(dir.path(), Ticks(37));
    let reference = render_with(&p, 512);
    for block in [1usize, 37, 64, 1000] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
    // The processor driven as the callback drives it, in blocks of every size.
    let s = snapshot(&p);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    let mut live = (*s).clone();
    live.loop_region = None;
    live.metronome = false;
    tx.push(EngineMessage::SetSnapshot(Arc::new(live))).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = reference.len() / 2;
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 17, 1024, 5];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    assert_null(&reference, &out, 0.0, "live vs offline");
}

/// Review 1: a window starting on its file's first frame (or one ending on its last) has
/// nothing to read past the seam — the crossfade moves to where both have audio, so it
/// never cuts in: equal power at every sample, no dip, no step.
#[test]
fn a_seam_at_a_files_edge_crossfades_where_both_sides_have_audio() {
    let dir = tempfile::tempdir().unwrap();
    let x = Ticks(20);
    let seam = 4 * 960 * FPT;
    for case in ["right starts its file", "left ends its file"] {
        let mut p = comped(dir.path(), x);
        if case == "right starts its file" {
            p.tracks[0].clips[1].offset = Ticks::ZERO;
        } else {
            p.tracks[0].clips[0].offset = Ticks::beats(4);
        }
        let out = render_with(&p, 512);
        let at = |f: i64| (out[2 * f as usize], out[2 * f as usize + 1]);
        let mut last = (1.0f32, 0.0f32);
        for f in seam - 30 * FPT..seam + 30 * FPT {
            let (l, r) = at(f);
            assert!(
                (l * l + r * r - 1.0).abs() < 1e-5,
                "{case}, frame {f}: {l} {r}"
            );
            assert!(l <= last.0 && r >= last.1, "{case}, frame {f}: monotone");
            last = (l, r);
        }
        // Still a crossfade, the whole length of it, not a butt join.
        let crossing = (seam - 30 * FPT..seam + 30 * FPT)
            .filter(|&f| at(f).0 > 0.0 && at(f).1 > 0.0)
            .count() as i64;
        assert!(crossing >= 20 * FPT - 2, "{case}: {crossing} samples cross");
    }
}

/// Integration review: a split comp window is no seam. Its two halves meet and show the
/// same file without a break, so even a crossfade length on the right half (a split that
/// carried the window's own) must not cross them: the render equals the unsplit one, to
/// the bit (before: a +3 dB swell 10 ms wide at the cut).
#[test]
fn a_split_window_is_no_seam() {
    let dir = tempfile::tempdir().unwrap();
    let x = Ticks(20);
    let whole = comped(dir.path(), x);
    let reference = render_with(&whole, 512);
    let mut split = whole.clone();
    let id = ClipId(split.alloc_id());
    let b = split.tracks[0].clips[1].clone();
    let mut right = b.clone();
    right.id = id;
    right.start = Ticks::beats(6);
    right.length = Ticks::beats(2);
    right.offset = Ticks::beats(6);
    split.tracks[0].clips[1].length = Ticks::beats(2);
    split.tracks[0].clips.push(right);
    assert_null(&reference, &render_with(&split, 512), 0.0, "a split window");
}

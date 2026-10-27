//! Crossfades between overlapping clips and clip gain envelopes in the engine (ADR 0070):
//! an overlap with no crossfade plays summed as it always did, to the bit; one with a
//! crossfade crosses over the whole overlap on its curve, the earlier clip fading out over
//! exactly the samples the later fades in on; no envelope, or a flat one, touches no
//! sample; an envelope lands where its points are, mirrored on a reversed clip, carried by
//! a slip; every block size, the live callback and the stems render alike.

use cantus_core::clip_gain::{gain_of, GainEnvelope, GainPoint};
use cantus_core::crossfade::XfadeCurve;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::stems::{render_stems, StemOptions, StemSource, StemSpec};
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;
/// At 120 BPM and 48 kHz one tick is exactly 25 frames.
const FPT: i64 = 25;
const BEAT: i64 = 960 * FPT;

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

/// Two eight-beat files — the first all left, the second all right — as two clips, the
/// first from beat 0 to 4, the second from beat 3 to 7: a beat of overlap, crossed on
/// `cross` (or summed).
fn overlapped(dir: &Path, cross: Option<XfadeCurve>) -> Project {
    let mut p = Project::new("xf");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Vox");
    t.strip = None;
    p.tracks.push(t);
    let frames = 8 * BEAT as usize;
    let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
    constant(&a, frames, 1.0, 0.0);
    constant(&b, frames, 0.0, 1.0);
    let sa = source(&mut p, &a, frames as u64);
    let sb = source(&mut p, &b, frames as u64);
    let left = p.make_audio_clip("a", sa, Ticks::ZERO, Ticks::beats(4));
    let mut right = p.make_audio_clip("b", sb, Ticks::beats(3), Ticks::beats(4));
    right.audio.as_mut().unwrap().cross = cross;
    p.tracks[0].clips.extend([left, right]);
    p
}

/// One clip of a file that is 0.5 everywhere, from beat 1 for four beats, its content
/// from beat 2 of the file.
fn one_clip(dir: &Path) -> Project {
    let mut p = Project::new("env");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Vox");
    t.strip = None;
    p.tracks.push(t);
    let frames = 8 * BEAT as usize;
    let f = dir.join("c.wav");
    constant(&f, frames, 0.5, 0.5);
    let s = source(&mut p, &f, frames as u64);
    let mut c = p.make_audio_clip("c", s, Ticks::beats(1), Ticks::beats(4));
    c.offset = Ticks::beats(2);
    p.tracks[0].clips.push(c);
    p
}

fn set_env(p: &mut Project, points: &[(i64, f32)]) {
    let a = p.tracks[0].clips[0].audio.as_mut().unwrap();
    a.gain_env = Some(Box::new(GainEnvelope::new(
        points
            .iter()
            .map(|(t, db)| GainPoint::new(Ticks(*t), *db))
            .collect(),
    )));
}

fn snapshot(p: &Project) -> Arc<Snapshot> {
    let mut s = Snapshot::compile(p, 1);
    let errors = SampleCache::new().attach_audio(&mut s, SR);
    assert!(errors.is_empty(), "{errors:?}");
    Arc::new(s)
}

fn opts(block: usize) -> RenderOptions {
    RenderOptions {
        sample_rate: SR,
        tail_seconds: 0.0,
        block_frames: block,
        ..Default::default()
    }
}

fn render_with(p: &Project, block: usize) -> Vec<f32> {
    render(snapshot(p), &opts(block)).unwrap().samples
}

/// The processor driven as the callback drives it, in blocks of uneven sizes.
fn live(p: &Project, total: usize) -> Vec<f32> {
    let s = snapshot(p);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    let mut live = (*s).clone();
    live.loop_region = None;
    live.metronome = false;
    tx.push(EngineMessage::SetSnapshot(Arc::new(live))).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
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
    out
}

fn at(out: &[f32], f: i64) -> (f32, f32) {
    (out[2 * f as usize], out[2 * f as usize + 1])
}

#[test]
fn an_overlap_without_a_crossfade_plays_summed_as_it_always_did() {
    let dir = tempfile::tempdir().unwrap();
    let out = render_with(&overlapped(dir.path(), None), 512);
    assert_eq!(at(&out, 3 * BEAT - 1), (1.0, 0.0));
    for f in [3 * BEAT, 3 * BEAT + 1_000, 4 * BEAT - 1] {
        assert_eq!(at(&out, f), (1.0, 1.0), "frame {f}: both, summed");
    }
    assert_eq!(at(&out, 4 * BEAT), (0.0, 1.0));
}

#[test]
fn an_overlap_crosses_over_its_whole_length_on_each_curve() {
    let dir = tempfile::tempdir().unwrap();
    for curve in XfadeCurve::ALL {
        let out = render_with(&overlapped(dir.path(), Some(curve)), 512);
        let (from, to) = (3 * BEAT, 4 * BEAT);
        for f in [0, from - 1] {
            assert_eq!(at(&out, f), (1.0, 0.0), "{curve:?} frame {f}");
        }
        for f in [to, to + 1, to + 5_000] {
            assert_eq!(at(&out, f), (0.0, 1.0), "{curve:?} frame {f}");
        }
        let mut last = (1.0f32, 0.0f32);
        for f in from..to {
            let (l, r) = at(&out, f);
            // The two gains are the curve's halves at the same `u`, on the same sample.
            let u = ((f - from) as f32 + 0.5) / (to - from) as f32;
            assert_eq!(
                (l, r),
                (curve.gain_out(u), curve.gain_in(u)),
                "{curve:?} {f}"
            );
            match curve {
                XfadeCurve::EqualPower => assert!((l * l + r * r - 1.0).abs() < 1e-5),
                _ => assert!((l + r - 1.0).abs() < 1e-6),
            }
            assert!(l <= last.0 && r >= last.1, "{curve:?} frame {f}: monotone");
            last = (l, r);
        }
    }
}

#[test]
fn crossfades_and_envelopes_render_alike_at_every_block_size_live_and_as_stems() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = overlapped(dir.path(), Some(XfadeCurve::SCurve));
    set_env(
        &mut p,
        &[(0, 0.0), (960, -12.0), (1_900, 3.0), (2_000, 3.0)],
    );
    let reference = render_with(&p, 512);
    for block in [1usize, 37, 64, 1000] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
    assert_null(
        &reference,
        &live(&p, reference.len() / 2),
        0.0,
        "live vs offline",
    );
    // The stems' master is the mix, to the bit.
    let stems = render_stems(
        snapshot(&p),
        &StemOptions {
            render: opts(512),
            keep_silent: true,
            ..Default::default()
        },
        &[StemSpec {
            source: StemSource::Master,
            file_name: "master.wav".into(),
        }],
    )
    .unwrap();
    let n = reference.len().min(stems[0].samples.len());
    assert_null(&reference[..n], &stems[0].samples[..n], 0.0, "stem");
}

#[test]
fn no_envelope_or_a_flat_one_touches_no_sample() {
    let dir = tempfile::tempdir().unwrap();
    let p = one_clip(dir.path());
    let reference = render_with(&p, 512);
    let mut flat = p.clone();
    set_env(&mut flat, &[(0, 0.0), (5_000, 0.0)]);
    assert_null(&reference, &render_with(&flat, 512), 0.0, "flat envelope");
    let mut empty = p.clone();
    set_env(&mut empty, &[]);
    assert_null(&reference, &render_with(&empty, 512), 0.0, "no points");
}

#[test]
fn an_envelope_lands_where_its_points_are() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = one_clip(dir.path());
    // In content ticks: the clip shows content beats 2…6 from timeline beat 1, so content
    // beat 3 is timeline beat 2, content beat 4 timeline beat 3.
    set_env(&mut p, &[(3 * 960, 0.0), (4 * 960, -12.0)]);
    let out = render_with(&p, 512);
    let half = 0.5f32;
    // Before the first point it holds 0 dB; after the last, −12 dB.
    assert_eq!(at(&out, BEAT + 10).0, half);
    assert_eq!(at(&out, 2 * BEAT).0, half);
    assert_eq!(at(&out, 3 * BEAT).0, half * gain_of(-12.0));
    assert_eq!(at(&out, 4 * BEAT + 100).0, half * gain_of(-12.0));
    // Half-way in dB: −6 dB.
    let mid = at(&out, 2 * BEAT + BEAT / 2).0;
    assert!((mid - half * gain_of(-6.0)).abs() < 1e-6, "{mid}");
    // A slip moves the content and its line with it: one beat later, the line lands a
    // beat later.
    let mut slipped = p.clone();
    slipped.tracks[0].clips[0].offset = Ticks::beats(1);
    let out = render_with(&slipped, 512);
    assert_eq!(at(&out, 3 * BEAT).0, half);
    assert_eq!(at(&out, 4 * BEAT).0, half * gain_of(-12.0));
}

#[test]
fn a_reversed_clip_plays_its_line_mirrored_with_its_audio() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = one_clip(dir.path());
    set_env(&mut p, &[(3 * 960, 0.0), (4 * 960, -12.0)]);
    let fwd = render_with(&p, 512);
    p.tracks[0].clips[0].audio.as_mut().unwrap().reverse = true;
    let rev = render_with(&p, 512);
    // The window is beats 1…5 (samples BEAT…5·BEAT): sample s reversed is s' forward,
    // s + s' = start + end − 1.
    let (start, end) = (BEAT, 5 * BEAT);
    for s in [start, start + 7, 2 * BEAT + 333, 3 * BEAT, end - 1] {
        let mirror = start + end - 1 - s;
        assert_eq!(at(&rev, s).0, at(&fwd, mirror).0, "sample {s}");
    }
}

#[test]
fn an_envelope_to_the_floor_is_silence() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = one_clip(dir.path());
    set_env(&mut p, &[(0, cantus_core::clip_gain::ENV_MIN_DB)]);
    let out = render_with(&p, 512);
    assert!(out.iter().all(|s| *s == 0.0));
}

/// Review 1 (ADR 0070): a split inside a crossed overlap changes nothing that plays —
/// the earlier clip split (its second half lies under the later one), or the later one
/// (its first half lies over the earlier one): to the bit, on every curve.
#[test]
fn a_split_inside_a_crossed_overlap_plays_as_before() {
    let dir = tempfile::tempdir().unwrap();
    for curve in XfadeCurve::ALL {
        let p = overlapped(dir.path(), Some(curve));
        let reference = render_with(&p, 512);
        let t = p.tracks[0].id;
        for which in [0usize, 1] {
            let mut q = p.clone();
            let c = q.tracks[0].clips[which].clone();
            let at = Ticks::beats(3) + Ticks(480);
            let mut next = 9_000u64;
            let mut ids = || {
                next += 1;
                next
            };
            let cmds = clipboard::split(&q, t, &c, at, &mut ids).expect("a split");
            for cmd in cmds {
                cmd.apply(&mut q).unwrap();
            }
            assert_eq!(q.tracks[0].clips.len(), 3);
            assert_null(
                &reference,
                &render_with(&q, 512),
                0.0,
                &format!("{curve:?}: clip {which} split inside the overlap"),
            );
        }
    }
}

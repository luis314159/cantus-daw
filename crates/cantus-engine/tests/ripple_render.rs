//! r40:ripple — insert time heard (ADR 0116): a bar of silence inserted into a song moves
//! the audio after it by exactly the bar's length, sample-exact — before the cut the render
//! is the same to the bit, the new bar is silent, and after it is the old render to the bit,
//! later by the bar's frames. With one tempo, and with a tempo track that changes after the
//! cut (the moved windows keep the second of their file they played). Deleting the bar
//! again renders the song as it was.

use cantus_core::ripple::{delete_time, insert_time};
use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;

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

fn write_f32(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
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

fn bar(n: i64) -> Ticks {
    Ticks(Ticks::beats(4).0 * n)
}

/// Two audio tracks: a long take across bar 2 (from beat 1, trimmed so its window starts
/// inside the file) and a short one at bar 3.
fn song(dir: &Path, tempo_track: bool) -> Project {
    let mut p = Project::new("ripple");
    p.tempo_bpm = 120.0;
    if tempo_track {
        p.tempo_points = vec![
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: false,
            },
            TempoPoint {
                tick: bar(4),
                bpm: 96.0,
                ramp: false,
            },
        ];
    }
    let frames = 48_000 * 12;
    for (i, seed) in [(0usize, 1u32), (1, 2)] {
        let path = dir.join(format!("t{i}.wav"));
        write_f32(&path, &signal(frames, seed));
        let id = AudioSourceId(p.alloc_id());
        p.audio_pool.push(AudioSource {
            id,
            name: format!("t{i}"),
            path: path.to_string_lossy().into_owned(),
            sample_rate: SR,
            channels: 2,
            frames: frames as u64,
            sha256: None,
            recorded: false,
        });
        let mut t = p.make_track(format!("T{i}"));
        t.strip = None;
        let mut c = if i == 0 {
            p.make_audio_clip("long", id, Ticks::beats(1), bar(4))
        } else {
            p.make_audio_clip("short", id, bar(3), bar(1))
        };
        c.offset = Ticks::beats(1);
        t.clips.push(c);
        p.tracks.push(t);
    }
    p
}

fn render_of(p: &Project) -> Vec<f32> {
    let mut s = Snapshot::compile(p, 1);
    let errors = SampleCache::new().attach_audio(&mut s, SR);
    assert!(errors.is_empty(), "{errors:?}");
    let opts = RenderOptions {
        sample_rate: SR,
        start: Ticks::ZERO,
        tail_seconds: 0.1,
        block_frames: 512,
        ..Default::default()
    };
    render(Arc::new(s), &opts).unwrap().samples
}

fn with_ids<R>(p: &mut Project, f: impl FnOnce(&Project, &mut dyn FnMut() -> u64) -> R) -> R {
    let mut next = p.next_id;
    let out = f(p, &mut || {
        let id = next;
        next += 1;
        id
    });
    p.next_id = next;
    out
}

fn frame_of(p: &Project, t: Ticks) -> usize {
    let s = p.tempo_map().seconds_at(t.0 as f64) * f64::from(SR);
    assert_eq!(s, s.round(), "a whole frame");
    s as usize
}

fn inserted_bar_moves_the_audio_by_its_length(tempo_track: bool) {
    let dir = tempfile::tempdir().unwrap();
    let mut p = song(dir.path(), tempo_track);
    let before = render_of(&p);
    let cmd = with_ids(&mut p, |q, ids| insert_time(q, bar(2), bar(1), ids)).unwrap();
    let mut q = p.clone();
    cmd.apply(&mut q).unwrap();
    let after = render_of(&q);
    let at = frame_of(&p, bar(2)) * 2;
    let len = (frame_of(&q, bar(3)) - frame_of(&q, bar(2))) * 2;
    assert_eq!(len, 48_000 * 2 * 2, "a bar at 120 BPM: two seconds");
    // Before the cut: the same, to the bit.
    assert!(
        before[..at]
            .iter()
            .zip(&after[..at])
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "before the cut"
    );
    // The new bar: silence.
    assert!(
        after[at..at + len].iter().all(|s| *s == 0.0),
        "the new bar is silent"
    );
    // After it: the old song, later by the bar's frames, to the bit.
    let rest = before.len() - at;
    let tail = &after[at + len..at + len + rest.min(after.len() - at - len)];
    for (i, (a, b)) in before[at..].iter().zip(tail).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "sample {} after the cut (tempo track: {tempo_track})",
            i
        );
    }
    // Deleted again: the song renders as it did.
    let cmd = with_ids(&mut q, |r, ids| delete_time(r, bar(2), bar(3), ids)).unwrap();
    let mut r = q.clone();
    cmd.apply(&mut r).unwrap();
    let again = render_of(&r);
    assert_eq!(again.len(), before.len());
    assert!(
        again
            .iter()
            .zip(&before)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "deleted again: as it was"
    );
}

#[test]
fn an_inserted_bar_of_silence_moves_the_audio_exactly_by_its_length() {
    inserted_bar_moves_the_audio_by_its_length(false);
}

#[test]
fn under_a_tempo_track_too() {
    inserted_bar_moves_the_audio_by_its_length(true);
}

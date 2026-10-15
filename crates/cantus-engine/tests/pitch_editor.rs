//! The pitch editor's engine side (round 28, ADR 0054): the analysis of a take into notes
//! and a key, the render of the edits, and the render played by the engine — the brief's
//! "done when", measured: a synthetic sung take with known errors corrected to within
//! 5 cents; formants kept at ±3 semitones (spectral envelope distance); a timing edit that
//! leaves its neighbours bit for bit; nothing asked = the take bit for bit; offline = real
//! time; the original (never silence) while a render is pending.

use cantus_core::pitch::curves::Frames;
use cantus_core::pitch::{edit, PitchEdit};
use cantus_core::*;
use cantus_dsp::math;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::pitch::testkit::{self, envelope_distance, Vowel};
use cantus_dsp::pitch::{analyse, hz_to_midi, midi_to_hz};
use cantus_engine::pitch::{analyse_take, render_edit, TakeAnalysis};
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;

/// A sung note of the synthetic take: from, to (seconds), the note meant (MIDI), how far
/// off it was sung (cents), and its vibrato (cents, peak; 5.5 Hz).
struct Sung {
    from: f64,
    to: f64,
    meant: f32,
    off: f32,
    vibrato: f32,
}

/// Four notes, sung out of tune: A3 +35, C4 −28, E4 +22 with a vibrato, D4 −40.
fn sung() -> Vec<Sung> {
    let n = |from, to, meant, off, vibrato| Sung {
        from,
        to,
        meant,
        off,
        vibrato,
    };
    vec![
        n(0.10, 0.70, 57.0, 35.0, 0.0),
        n(0.85, 1.45, 60.0, -28.0, 0.0),
        n(1.60, 2.30, 64.0, 22.0, 30.0),
        n(2.45, 3.05, 62.0, -40.0, 0.0),
    ]
}

fn voice_of(notes: &[Sung], seconds: f32) -> Vec<f32> {
    let f0 = |t: f64| -> f32 {
        for n in notes {
            if t >= n.from && t < n.to {
                let vib =
                    n.vibrato * math::sin(2.0 * std::f64::consts::PI * 5.5 * (t - n.from)) as f32;
                return midi_to_hz(n.meant + (n.off + vib) / 100.0);
            }
        }
        0.0
    };
    testkit::voice(SR as f32, seconds, Vowel::A, 0.5, f0)
}

fn take() -> (Vec<f32>, TakeAnalysis) {
    let x = voice_of(&sung(), 3.2);
    let a = analyse_take(&[&x], SR);
    (x, a)
}

/// The mean pitch of `x` over each of `edit`'s notes, measured by the offline tracker the
/// way the analysis measures a note's centre.
fn centres(x: &[f32], e: &PitchEdit) -> Vec<f32> {
    let a = analyse(x, SR as f32);
    let midi: Vec<f32> = a
        .frames
        .iter()
        .map(|f| if f.voiced { hz_to_midi(f.f0) } else { f32::NAN })
        .collect();
    let f = Frames {
        hop: a.hop,
        midi: &midi,
    };
    e.notes
        .iter()
        .map(|n| f.centre(n.start, n.end).unwrap_or(f32::NAN))
        .collect()
}

#[test]
fn the_analysis_finds_the_sung_notes_and_the_key() {
    let (_, a) = take();
    let truth = sung();
    assert_eq!(a.notes.len(), truth.len(), "{:#?}", a.notes);
    for (n, s) in a.notes.iter().zip(&truth) {
        let (from, to) = (n.src_start as f64 / SR as f64, n.src_end as f64 / SR as f64);
        assert!(
            (from - s.from).abs() < 0.03 && (to - s.to).abs() < 0.03,
            "{from}–{to}"
        );
        let err = (n.sung - (s.meant + s.off / 100.0)) * 100.0;
        assert!(err.abs() < 3.0, "centre {} off by {err} cents", n.sung);
        assert!(n.level_db < -3.0 && n.level_db > -30.0);
    }
    // A, C, E, D: C major or A minor, the two keys with these notes and nothing else.
    let k = a.key;
    assert!(
        (k.tonic, k.scale) == (0, 1) || (k.tonic, k.scale) == (9, 2),
        "{k:?}"
    );
    let e = a.edit();
    assert!(e.is_valid() && !e.edited());
    assert_eq!(e.rate, SR);
}

#[test]
fn a_take_sung_out_of_tune_is_corrected_within_5_cents() {
    let (x, a) = take();
    let mut e = a.edit();
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    edit::correct_pitch(&mut e, &ids, 1.0);
    let out = render_edit(&[&x], &a, &e);
    let got = centres(&out[0], &e);
    let before = centres(&x, &e);
    for ((g, b), s) in got.iter().zip(&before).zip(&sung()) {
        let err = (g - s.meant) * 100.0;
        eprintln!(
            "note {}: sung {:+.1} cents, corrected {err:+.2} cents",
            s.meant,
            (b - s.meant) * 100.0
        );
        assert!(err.abs() < 5.0, "{} corrected to {err} cents", s.meant);
    }
}

/// Formants kept a minor third up and down: the spectral envelope distance (dB RMS of the
/// harmonics' levels against the original's envelope, 300 Hz – 4 kHz) on a man's voice at
/// 110 and 180 Hz and a woman's at 220 and 300 Hz — small with the formants kept, large
/// when the formants are moved with the pitch (the throat at the same amount).
#[test]
fn formants_are_kept_three_semitones_up_and_down() {
    for (f0, vowel) in [
        (110.0f32, Vowel::A),
        (180.0, Vowel::A),
        (220.0, Vowel::A_FEMALE),
        (300.0, Vowel::A_FEMALE),
    ] {
        let x = testkit::voice(SR as f32, 1.4, vowel, 0.5, move |t| {
            if (0.1..1.3).contains(&t) {
                f0
            } else {
                0.0
            }
        });
        let a = analyse_take(&[&x], SR);
        assert_eq!(a.notes.len(), 1);
        let mid = |v: &[f32]| v[(0.4 * SR as f64) as usize..(1.1 * SR as f64) as usize].to_vec();
        for semis in [3.0f32, -3.0] {
            let mut kept = a.edit();
            edit::move_pitch(&mut kept, &[0], semis);
            let mut moved = kept.clone();
            edit::set_field(&mut moved, &[0], edit::Field::Formant, semis);
            let k = render_edit(&[&x], &a, &kept);
            let m = render_edit(&[&x], &a, &moved);
            let to = f0 * math::exp2(semis / 12.0);
            let d =
                |y: &[f32]| envelope_distance(&mid(&x), f0, &mid(y), to, SR as f32, 300.0, 4_000.0);
            let (d_kept, d_moved) = (d(&k[0]), d(&m[0]));
            eprintln!(
                "{f0} Hz {semis:+} st: envelope distance kept {d_kept:.2} dB, moved {d_moved:.2} dB"
            );
            assert!(d_kept < 3.0, "{f0} Hz {semis:+} st: kept {d_kept} dB");
            assert!(
                d_moved > d_kept + 3.0,
                "{f0} Hz {semis:+}: moved {d_moved}, kept {d_kept}"
            );
        }
    }
}

#[test]
fn a_timing_edit_leaves_its_neighbours_bit_for_bit() {
    let (x, a) = take();
    let mut e = a.edit();
    let id = e.notes[1].id;
    let (s, en) = (e.notes[1].start, e.notes[1].end);
    // 25 ms later, 10 % longer.
    let start = s + 1_200;
    let end = start + ((en - s) as f64 * 1.10) as u64;
    assert!(edit::set_timing(&mut e, id, start, end));
    assert_eq!(
        (e.notes[1].start, e.notes[1].end),
        (start, end),
        "not clamped"
    );
    let out = render_edit(&[&x], &a, &e);
    for n in [&e.notes[0], &e.notes[2], &e.notes[3]] {
        let (a, b) = (n.start as usize, n.end as usize);
        let same = out[0][a..b]
            .iter()
            .zip(&x[a..b])
            .all(|(p, q)| p.to_bits() == q.to_bits());
        assert!(same, "neighbour {a}..{b} touched");
    }
    // The moved note is where it was put: its own pitch, later.
    let got = centres(&out[0], &e);
    let before = sung();
    assert!(((got[1] - (before[1].meant + before[1].off / 100.0)) * 100.0).abs() < 5.0);
}

#[test]
fn nothing_asked_is_the_take_bit_for_bit_and_a_render_is_deterministic() {
    let (x, a) = take();
    let e = a.edit();
    let out = render_edit(&[&x, &x], &a, &e);
    for ch in &out {
        assert!(ch.iter().zip(&x).all(|(p, q)| p.to_bits() == q.to_bits()));
    }
    let mut moved = e.clone();
    let third = e.notes[2].id;
    edit::move_pitch(&mut moved, &[third], 2.0);
    let one = render_edit(&[&x], &a, &moved);
    let two = render_edit(&[&x], &a, &moved);
    assert_eq!(one, two);
    // And only the moved note changed (and the gaps beside it).
    let n = &moved.notes;
    for k in [0usize, 1, 3] {
        let (s, e) = (n[k].start as usize, n[k].end as usize);
        assert!(one[0][s..e]
            .iter()
            .zip(&x[s..e])
            .all(|(p, q)| p.to_bits() == q.to_bits()));
    }
}

#[test]
fn a_notes_gain_is_its_own() {
    let (x, a) = take();
    let mut e = a.edit();
    let first = e.notes[0].id;
    edit::set_field(&mut e, &[first], edit::Field::Gain, -6.0);
    let out = render_edit(&[&x], &a, &e);
    let n = &e.notes[0];
    let mid = (n.start + n.end) as usize / 2;
    let ratio = out[0][mid - 2_000..mid + 2_000]
        .iter()
        .map(|v| v * v)
        .sum::<f32>()
        / x[mid - 2_000..mid + 2_000]
            .iter()
            .map(|v| v * v)
            .sum::<f32>();
    let db = 10.0 * math::log10(ratio);
    assert!((db + 6.0).abs() < 0.01, "{db} dB");
    let (s, e2) = (e.notes[1].start as usize, e.notes[1].end as usize);
    assert!(out[0][s..e2]
        .iter()
        .zip(&x[s..e2])
        .all(|(p, q)| p.to_bits() == q.to_bits()));
}

// ---- in the engine ---------------------------------------------------------------------

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

/// A project playing the take from the start through the master, its clip carrying `e`.
fn project_with(path: &Path, x: &[f32], e: Option<PitchEdit>) -> Project {
    let mut p = Project::new("pitch");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.strip = None;
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "voice".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames: x.len() as u64,
        sha256: None,
        recorded: true,
    });
    let mut c = p.make_audio_clip("voice", id, Ticks::ZERO, Ticks::beats(8));
    c.audio.as_mut().unwrap().pitch = e.map(Box::new);
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn corrected(a: &TakeAnalysis) -> PitchEdit {
    let mut e = a.edit();
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    edit::correct_pitch(&mut e, &ids, 1.0);
    e
}

#[test]
fn the_engine_plays_the_render_and_offline_equals_real_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("voice.wav");
    let (x, a) = take();
    write_f32(&path, &x);
    let e = corrected(&a);
    let p = project_with(&path, &x, Some(e.clone()));
    let mut s = Snapshot::compile(&p, 1);
    let mut cache = SampleCache::new();
    assert!(cache.attach_audio(&mut s, SR).is_empty());
    assert!(!cache.loading(), "a manual cache renders at once");
    let want = render_edit(&[&x], &a, &e);
    let data = s.tracks[0].audio[0].data.clone().expect("attached");
    assert_eq!(&data.frames[..], &want[0][..], "the clip plays the render");
    let snap = Arc::new(s);
    let opts = RenderOptions {
        sample_rate: SR,
        tail_seconds: 0.0,
        block_frames: 256,
        ..Default::default()
    };
    let offline = render(snap.clone(), &opts).unwrap();
    // The bounce is the render, sample for sample (one channel to both sides, no strip).
    for (k, f) in offline.samples.chunks_exact(2).take(x.len()).enumerate() {
        assert_eq!(f[0].to_bits(), want[0][k].to_bits(), "frame {k}");
    }
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

#[test]
fn compared_with_the_original_or_untouched_the_clip_plays_its_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("voice.wav");
    let (x, a) = take();
    write_f32(&path, &x);
    let mut e = corrected(&a);
    e.bypass = true;
    for edit in [Some(e), Some(a.edit()), None] {
        let p = project_with(&path, &x, edit);
        let mut s = Snapshot::compile(&p, 1);
        let mut cache = SampleCache::new();
        cache.attach_audio(&mut s, SR);
        let data = s.tracks[0].audio[0].data.clone().unwrap();
        assert!(data
            .frames
            .iter()
            .zip(&x)
            .all(|(p, q)| p.to_bits() == q.to_bits()));
    }
}

#[test]
fn while_a_render_is_on_its_way_the_original_plays_never_silence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("voice.wav");
    let (x, a) = take();
    write_f32(&path, &x);
    let p = project_with(&path, &x, Some(corrected(&a)));
    let mut cache = SampleCache::new();
    cache.set_background(true);
    let wait = |cache: &mut SampleCache| {
        let t = std::time::Instant::now();
        while cache.loading() && t.elapsed() < std::time::Duration::from_secs(60) {
            cache.poll_loaded();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!cache.loading());
    };
    // The source itself loads first (silence until it does, as before).
    let mut s = Snapshot::compile(&p, 1);
    cache.attach_audio(&mut s, SR);
    wait(&mut cache);
    // Then the render is asked for: the original plays meanwhile, and export waits.
    let mut s = Snapshot::compile(&p, 2);
    cache.attach_audio(&mut s, SR);
    let now = s.tracks[0].audio[0].data.clone().expect("never silence");
    assert!(now
        .frames
        .iter()
        .zip(&x)
        .all(|(p, q)| p.to_bits() == q.to_bits()));
    assert!(
        cache.loading(),
        "a render is pending: an export waits for it"
    );
    wait(&mut cache);
    let mut s = Snapshot::compile(&p, 3);
    cache.attach_audio(&mut s, SR);
    let done = s.tracks[0].audio[0].data.clone().unwrap();
    let want = render_edit(&[&x], &a, &corrected(&a));
    assert_eq!(&done.frames[..], &want[0][..]);
}

#[test]
fn the_analysis_is_cached_on_disk_and_read_back_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("voice.wav");
    let (x, _) = take();
    write_f32(&path, &x);
    let cache_dir = dir.path().join("cache");
    let first = cantus_engine::pitch::PitchService::new();
    first.set_disk_cache(Some(cache_dir.clone()));
    let a = first.analyse_now(&path, SR).unwrap();
    assert_eq!(first.computed(), 1);
    let second = cantus_engine::pitch::PitchService::new();
    second.set_disk_cache(Some(cache_dir));
    let b = second.analyse_now(&path, SR).unwrap();
    assert_eq!(second.computed(), 0, "read from the disk");
    assert_eq!(a.analysis.frames, b.analysis.frames);
    assert_eq!(a.analysis.epochs, b.analysis.epochs);
    assert_eq!(a.notes, b.notes);
    assert_eq!(a.key, b.key);
}

/// How fast: a three-minute take analysed and rendered with every note corrected — the
/// cost of opening a take and of each edit's render (stated in docs/round28/pitch.md).
#[test]
fn a_three_minute_take_analyses_and_renders_in_seconds() {
    let line = voice_of(&sung(), 3.2);
    let mut x = Vec::with_capacity(line.len() * 57);
    while x.len() < 180 * SR as usize {
        x.extend_from_slice(&line);
    }
    let t = std::time::Instant::now();
    let a = analyse_take(&[&x], SR);
    let analysis = t.elapsed().as_secs_f64();
    let e = corrected(&a);
    let t = std::time::Instant::now();
    let out = render_edit(&[&x, &x], &a, &e);
    let render = t.elapsed().as_secs_f64();
    eprintln!(
        "3 min take: {} notes; analysis {analysis:.2} s, stereo render of every note {render:.2} s",
        a.notes.len()
    );
    assert_eq!(out[0].len(), x.len());
    assert!(a.notes.len() > 200);
    assert!(analysis < 20.0 && render < 20.0, "{analysis} s, {render} s");
}

/// A moved note never clicks: two notes sung legato (no gap), the second moved up a major
/// third — around the joint the output's second difference stays within what the steady
/// voice has.
#[test]
fn a_moved_legato_note_does_not_click() {
    let x = testkit::voice(SR as f32, 1.6, Vowel::A, 0.5, |t| {
        if !(0.1..1.5).contains(&t) {
            0.0
        } else if t < 0.8 {
            196.0
        } else {
            220.0
        }
    });
    let a = analyse_take(&[&x], SR);
    assert_eq!(a.notes.len(), 2, "{:#?}", a.notes);
    let mut e = a.edit();
    let second = e.notes[1].id;
    edit::move_pitch(&mut e, &[second], 4.0);
    let y = &render_edit(&[&x], &a, &e)[0];
    let d2 = |v: &[f32], from: f64, to: f64| {
        let (i, j) = ((from * SR as f64) as usize, (to * SR as f64) as usize);
        (i + 1..j - 1)
            .map(|k| (v[k + 1] - 2.0 * v[k] + v[k - 1]).abs())
            .fold(0.0f32, f32::max)
    };
    let steady = d2(y, 1.0, 1.4).max(d2(y, 0.3, 0.7));
    let joint = d2(y, 0.72, 0.88);
    eprintln!("second difference: steady {steady:.4}, at the joint {joint:.4}");
    assert!(joint <= steady * 1.5, "joint {joint} vs steady {steady}");
}

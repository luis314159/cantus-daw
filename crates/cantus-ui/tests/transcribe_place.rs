//! Where *Convert to MIDI…* puts what it heard (ADR 0086, round 34's review): each piece of
//! a clip through its own mapping — a loop after a tempo change, a reversed loop, a short
//! last piece, an offset, a slip, a warp — the bends of a cut or reversed note, drum hits
//! at their onsets; the kit and the sampler a new track gets; melody mode's shortest note.
//!
//! The expected ticks are worked out here from the rules the engine plays by
//! (`Snapshot::compile`'s pieces, natural speed through the tempo map), not from the code
//! under test.

use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ml::transcribe::spec::{DrumClass, DrumHit, Mode, NoteEvent, Transcription};
use cantus_ui::transcribe_ui::{self as tr, Instrument, Settings};
use cantus_ui::CantusApp;

const SR: u32 = 48_000;
const Q: f64 = Ticks::QUARTER.0 as f64;

struct Rig {
    app: CantusApp,
    _dir: tempfile::TempDir,
}

/// A 120-BPM project whose Voice track holds a 4-second take in a clip from `start` for
/// `beats`, shaped by `edit` (loop, reverse, offset…) and the project by `song`.
fn rig(
    start: f64,
    beats: f64,
    edit: impl FnOnce(&mut Clip),
    song: impl FnOnce(&mut Project),
) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..4 * SR {
        w.write_sample((i as f32 * 0.01).sin() * 0.1).unwrap();
    }
    w.finalize().unwrap();
    let mut p = Project::new("place");
    p.tempo_bpm = 120.0;
    let mut v = p.make_track("Voice");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames: 4 * SR as u64,
        sha256: None,
        recorded: true,
    });
    let mut c = p.make_audio_clip(
        "Take 1",
        id,
        Ticks((start * Q) as i64),
        Ticks((beats * Q) as i64),
    );
    edit(&mut c);
    v.clips.push(c);
    p.tracks.push(v);
    song(&mut p);
    Rig {
        app: CantusApp::from_parts(Some(p), EngineHandle::dummy(), false),
        _dir: dir,
    }
}

fn source(r: &Rig) -> tr::Source {
    let t = &r.app.project().tracks[0];
    tr::source(&r.app, t.id, t.clips[0].id).unwrap()
}

fn note(start: f64, end: f64, pitch: u8, bends: Vec<i32>) -> NoteEvent {
    NoteEvent {
        start,
        end,
        pitch,
        velocity: 90,
        amplitude: 0.7,
        bends,
        curve: Vec::new(),
    }
}

fn notes(n: Vec<NoteEvent>) -> Transcription {
    Transcription {
        mode: Mode::Polyphonic,
        duration: 4.0,
        notes: n,
        hits: Vec::new(),
    }
}

fn poly(bends: bool) -> Settings {
    Settings {
        mode: Mode::Polyphonic,
        bends,
        ..Settings::default()
    }
}

/// The placed notes' (start, end) in beats.
/// A placed note: start, end, pitch, velocity, bend points.
type Placed = (Ticks, Ticks, u8, u8, Vec<(f64, f64)>);

fn beats(placed: &[Placed]) -> Vec<(f64, f64)> {
    placed
        .iter()
        .map(|p| (p.0 .0 as f64 / Q, p.1 .0 as f64 / Q))
        .collect()
}

fn near(got: &[(f64, f64)], want: &[(f64, f64)]) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g.0 - w.0).abs() < 2.0 / Q && (g.1 - w.1).abs() < 2.0 / Q,
            "{got:?} against {want:?}"
        );
    }
}

#[test]
fn a_loop_after_a_tempo_change_places_each_repeat_through_its_own_piece() {
    // A 3-beat loop over 8 beats; from beat 4 the song is at 60 BPM. The engine plays each
    // piece from the loop's start at the take's own speed, so the second piece holds 2.5 s
    // of take (half a second at 120, two at 60) and the short last one, 2 s.
    let r = rig(
        0.0,
        8.0,
        |c| c.content_length = Some(Ticks::beats(3)),
        |p| {
            p.tempo_points = vec![TempoPoint {
                tick: Ticks::beats(4),
                bpm: 60.0,
                ramp: false,
            }]
        },
    );
    let src = source(&r);
    assert_eq!(src.pieces.len(), 3);
    assert_eq!(src.frames, (0, 5 * SR as u64 / 2));
    let t = notes(vec![
        note(0.25, 0.35, 60, vec![]),
        note(1.9, 2.1, 64, vec![0, 3]),
    ]);
    let placed = tr::place(&src, &poly(true), &t);
    near(
        &beats(&placed),
        &[(0.5, 0.7), (3.5, 3.7), (5.4, 5.6), (6.25, 6.35), (7.9, 8.0)],
    );
    // The last piece cuts the bent note in half: it keeps the first half of its bend.
    let cut = placed.last().unwrap();
    assert_eq!(cut.2, 64);
    let end = cut.4.last().unwrap();
    assert!(
        (end.0 - 1.0).abs() < 1e-9 && (end.1 - 0.5).abs() < 1e-6,
        "{:?}",
        cut.4
    );
    let whole = &placed[2];
    assert!(
        (whole.4.last().unwrap().1 - 1.0).abs() < 1e-6,
        "{:?}",
        whole.4
    );
}

#[test]
fn a_reversed_loop_mirrors_each_piece_in_its_own_window() {
    // A 3-beat (1.5 s) loop over 8 beats, played backwards: each piece mirrored in itself,
    // the short last one (1 s) too.
    let r = rig(
        0.0,
        8.0,
        |c| {
            c.content_length = Some(Ticks::beats(3));
            c.audio.as_mut().unwrap().reverse = true;
        },
        |_| {},
    );
    let src = source(&r);
    assert_eq!(src.frames, (0, 3 * SR as u64 / 2));
    let t = notes(vec![note(0.25, 0.5, 60, vec![0, 3])]);
    let placed = tr::place(&src, &poly(true), &t);
    near(&beats(&placed), &[(2.0, 2.5), (5.0, 5.5), (7.0, 7.5)]);
    for p in &placed {
        // A rise played backwards falls.
        assert_eq!(p.4.first().map(|b| b.1), Some(1.0), "{:?}", p.4);
        assert_eq!(p.4.last().map(|b| b.1), Some(0.0), "{:?}", p.4);
    }
    // A drum hit lands where it is struck — its onset, mirrored — not where it ends.
    let hits = Transcription {
        mode: Mode::Drums,
        duration: 1.5,
        notes: Vec::new(),
        hits: vec![DrumHit {
            time: 0.25,
            class: DrumClass::Kick,
            key: 36,
            velocity: 100,
        }],
    };
    let d = Settings {
        mode: Mode::Drums,
        ..Settings::default()
    };
    let placed = tr::place(&src, &d, &hits);
    near(&beats(&placed), &[(2.5, 2.625), (5.5, 5.625), (7.5, 7.625)]);
}

#[test]
fn an_offset_and_a_slip_move_the_take_under_the_clip() {
    // The clip starts at beat 2 showing the take from beat 1 (half a second) and 4 800
    // frames (0.1 s) of slip: its first frame is 0.6 s in.
    let r = rig(
        2.0,
        4.0,
        |c| {
            c.offset = Ticks::beats(1);
            c.audio.as_mut().unwrap().slip_frames = 4_800;
        },
        |_| {},
    );
    let src = source(&r);
    assert_eq!(src.frames.0, 28_800);
    let t = notes(vec![
        note(0.0, 0.25, 60, vec![]),
        note(0.5, 0.75, 62, vec![]),
    ]);
    near(
        &beats(&tr::place(&src, &poly(false), &t)),
        &[(2.0, 2.5), (3.0, 3.5)],
    );
}

#[test]
fn a_warped_clip_places_through_its_warp() {
    // A take at 60 BPM warped to follow a 120-BPM song: a second of take is one beat.
    let r = rig(
        0.0,
        8.0,
        |c| c.audio.as_mut().unwrap().warp = Some(Box::new(cantus_core::warp::Warp::new(SR, 60.0))),
        |_| {},
    );
    let src = source(&r);
    let t = notes(vec![note(1.0, 1.5, 60, vec![])]);
    near(&beats(&tr::place(&src, &poly(false), &t)), &[(1.0, 1.5)]);
}

#[test]
fn a_bend_keeps_the_part_of_the_note_that_is_kept() {
    let b = [(0.0, 0.0), (0.5, 1.0), (1.0, 1.0)];
    assert_eq!(
        tr::remap_bend(&b, (0.25, 0.75), false),
        [(0.0, 0.5), (0.5, 1.0), (1.0, 1.0)]
    );
    assert_eq!(
        tr::remap_bend(&b, (0.25, 0.75), true),
        [(0.0, 1.0), (0.5, 1.0), (1.0, 0.5)]
    );
    assert_eq!(tr::remap_bend(&b, (0.0, 1.0), false), b);
    assert!(tr::remap_bend(&[], (0.2, 0.4), true).is_empty());
}

#[test]
fn every_drum_key_a_conversion_writes_has_a_pad_in_its_kit() {
    let d = Settings {
        mode: Mode::Drums,
        ..Settings::default()
    };
    let InstrumentSpec::Pulsus(kit) = tr::instrument(&d) else {
        panic!("a drum conversion gets Pulsus")
    };
    for key in cantus_ml::transcribe::drums_model::KEYS {
        assert!(
            kit.pads.iter().any(|p| p.pitch == key),
            "key {key} has no pad in the studio kit"
        );
    }
}

#[test]
fn exemplar_gets_a_sound_not_an_empty_sampler() {
    let s = Settings {
        instrument: Instrument::Exemplar,
        ..Settings::default()
    };
    let InstrumentSpec::Native(n) = tr::instrument(&s) else {
        panic!()
    };
    assert_eq!(n.kind, cantus_core::inst::InstrumentKind::Sampler);
    assert!(!n.params.is_empty(), "a factory preset's zones");
    assert_eq!(
        n,
        cantus_ui::views::inst::preset_spec(
            cantus_core::inst::InstrumentKind::Sampler,
            &cantus_ui::views::inst::sampler::FACTORY_PRESETS[0]
        )
    );
}

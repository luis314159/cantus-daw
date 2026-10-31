//! Cantus's port of Basic Pitch's note creation held to Basic Pitch's own (ADR 0086): the
//! network's posteriorgrams for a few seconds of tones (chords, a glide, vibrato, a bass
//! line, a note at the very end), quantised to 16 bits, and the notes `basic_pitch`'s
//! `model_output_to_notes` made from exactly those values under six settings
//! (`tools/reference_basic_pitch.py fixture`). Every note equal — start and end frame,
//! pitch, mean posterior to the bit, every bend — and in Basic Pitch's own order.

use cantus_ml::transcribe::notes::{self, NoteParams, Posteriors, CONTOURS, NOTES};
use serde_json::Value;
use std::io::Read;

fn fixture() -> (Posteriors, Value) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/basic_pitch");
    let gz = std::fs::read(dir.join("posteriors.u16.gz")).unwrap();
    let mut b = Vec::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_end(&mut b)
        .unwrap();
    let frames = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
    let vals: Vec<f32> = b[4..]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]) as f32 / 65535.0f32)
        .collect();
    assert_eq!(vals.len(), frames * (2 * NOTES + CONTOURS));
    let (note, rest) = vals.split_at(frames * NOTES);
    let (onset, contour) = rest.split_at(frames * NOTES);
    let post = Posteriors {
        frames,
        note: note.to_vec(),
        onset: onset.to_vec(),
        contour: contour.to_vec(),
    };
    let json: Value =
        serde_json::from_slice(&std::fs::read(dir.join("notes.json")).unwrap()).unwrap();
    (post, json)
}

fn params(s: &Value) -> NoteParams {
    NoteParams {
        onset_threshold: s["onset_thresh"].as_f64().unwrap(),
        frame_threshold: s["frame_thresh"].as_f64().unwrap(),
        min_note_frames: s["min_note_len"].as_u64().unwrap() as usize,
        min_hz: s.get("min_freq").and_then(Value::as_f64),
        max_hz: s.get("max_freq").and_then(Value::as_f64),
        infer_onsets: s
            .get("infer_onsets")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        melodia_trick: s
            .get("melodia_trick")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    }
}

#[test]
fn every_setting_gives_basic_pitchs_own_notes() {
    let (post, json) = fixture();
    assert_eq!(json["frames"].as_u64().unwrap() as usize, post.frames);
    let settings = json["settings"].as_object().unwrap();
    assert_eq!(settings.len(), 6);
    for (name, s) in settings {
        let want = json["notes"][name].as_array().unwrap();
        let got = notes::notes(&post, &params(s));
        let bends = notes::pitch_bends(&post.contour, &got);
        assert_eq!(
            got.len(),
            want.len(),
            "{name}: {} notes, not {}",
            got.len(),
            want.len()
        );
        assert!(!got.is_empty(), "{name}");
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            let ctx = format!("{name}, note {i}: {g:?} vs {w}");
            assert_eq!(g.pitch as u64, w["pitch"].as_u64().unwrap(), "{ctx}");
            let (s, e) = (notes::frame_time(g.start), notes::frame_time(g.end));
            assert!((s - w["start"].as_f64().unwrap()).abs() < 1e-12, "{ctx}");
            assert!((e - w["end"].as_f64().unwrap()).abs() < 1e-12, "{ctx}");
            assert_eq!(
                g.amplitude,
                w["amplitude"].as_f64().unwrap() as f32,
                "{ctx}"
            );
            let wb: Vec<i32> = w["bends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect();
            assert_eq!(bends[i], wb, "{ctx}");
        }
    }
}

#[test]
fn the_fixture_exercises_what_matters() {
    // Chords (several notes at one onset), the Melodia trick (more notes with it than
    // without), bends off the pitch (the glide and the vibrato), a band limit, and a note
    // at the last frames.
    let (post, json) = fixture();
    let n = |k: &str| json["notes"][k].as_array().unwrap().len();
    assert!(n("default") > n("no_melodia"));
    assert!(n("sensitive") > n("default") && n("strict") < n("default"));
    let def = notes::notes(&post, &params(&json["settings"]["default"]));
    let bends = notes::pitch_bends(&post.contour, &def);
    assert!(
        bends.iter().any(|b| b.iter().any(|v| v.abs() >= 3)),
        "a bend of a semitone"
    );
    let starts: Vec<usize> = def.iter().map(|n| n.start).collect();
    assert!(starts
        .iter()
        .any(|s| starts.iter().filter(|t| *t == s).count() >= 3));
    assert!(def.iter().any(|n| n.end + 12 >= post.frames));
}

#[test]
fn numpys_float32_mean_is_reproduced() {
    // `np.mean` of float32 is NumPy's pairwise sum from zero over the count, in float32:
    // eight lanes up to 128 values, halves above. NumPy's own answers (2.5.4):
    // `np.sum(np.float32([2**24] + [1]*8)) - 2**24 == 8.0`, with 200 ones `188.0`.
    let big = 16_777_216.0f32;
    let mut v = vec![big];
    v.extend(std::iter::repeat_n(1.0f32, 8));
    assert_eq!(notes::pairwise_sum(&v) - big, 8.0);
    assert_eq!(
        v.iter().sum::<f32>() - big,
        0.0,
        "the naive order loses them all"
    );
    let mut v = vec![big];
    v.extend(std::iter::repeat_n(1.0f32, 200));
    assert_eq!(notes::pairwise_sum(&v) - big, 188.0);
    let v: Vec<f32> = (0..1000).map(|i| i as f32 * 0.001f32 + 1000.0f32).collect();
    assert_eq!(notes::pairwise_sum(&v), 1.000_499_5e6_f32);
}

#[test]
fn times_frames_and_windows_follow_basic_pitch() {
    assert_eq!(notes::min_note_frames(127.70), 11);
    assert_eq!(notes::frame_time(0), 0.0);
    // Within the first window frames are 256 samples apart; past it, the alignment
    // constant shifts them back.
    assert!((notes::frame_time(86) - 86.0 * 256.0 / 22050.0).abs() < 1e-12);
    let w = (256.0 / 22050.0) * (172.0 - 43844.0 / 256.0) + 0.0018;
    assert!((notes::frame_time(172) - (172.0 * 256.0 / 22050.0 - w)).abs() < 1e-12);
    assert_eq!(notes::window_hop(), 43_844 - 30 * 256);
    // A file of one sample still takes a window; one of exactly a hop less the padding,
    // one; a sample more, two.
    assert_eq!(notes::window_count(1), 1);
    assert_eq!(notes::window_count(36_164 - 3_840), 1);
    assert_eq!(notes::window_count(36_164 - 3_840 + 1), 2);
    let x: Vec<f32> = (0..50_000).map(|i| i as f32).collect();
    let mut buf = vec![0.0; notes::WINDOW];
    notes::window(&x, 0, &mut buf);
    assert_eq!((buf[3_839], buf[3_840], buf[3_841]), (0.0, 0.0, 1.0));
    notes::window(&x, 1, &mut buf);
    assert_eq!(buf[0], (36_164 - 3_840) as f32);
    // Joining keeps 142 frames a window and trims to the file: 86 frames a second.
    let wins = vec![vec![1.0f32; 172 * 2]; 3];
    assert_eq!(notes::unwrap(&wins, 2, 22_050).len(), 86 * 2);
    assert_eq!(notes::unwrap(&wins, 2, 10 * 22_050).len(), 3 * 142 * 2);
}

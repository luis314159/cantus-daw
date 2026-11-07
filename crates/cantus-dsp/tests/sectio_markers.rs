//! Sectio's markers read from files another program wrote (round 37, ADR 0099): three WAVs
//! made by **libsndfile 1.2.2** (`SFC_SET_CUE`, `SFC_SET_INSTRUMENT` — the library most
//! audio tools write cue points and sampler loops with), in `tests/data/sectio/`:
//!
//! - `libsndfile-cues-mono16.wav` — 16-bit mono, 22 050 Hz, four cue points written in the
//!   order 3, 1, 4, 2 at frames 5 512, 0, 8 268, 2 756;
//! - `libsndfile-smpl-stereo24.wav` — 24-bit stereo, 44 100 Hz, no cues, a `smpl` chunk
//!   with two forward loops starting at 2 205 and 0;
//! - `libsndfile-cues-smpl-float.wav` — 32-bit float stereo, 48 kHz, cues at 0, 1 200,
//!   2 400, 3 600 and a loop over the whole (the cues win).
//!
//! The script that wrote them (Python's ctypes on libsndfile, from the `soundfile` 0.14.0
//! wheel) is in `tests/data/sectio/README.md` with each file's SHA-256: these bytes are
//! libsndfile's, not ours.

use cantus_dsp::sectio::markers;

fn file(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sectio")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn libsndfiles_cue_points_are_read_in_order() {
    let m = markers(&file("libsndfile-cues-mono16.wav")).expect("a WAV");
    assert_eq!((m.rate, m.channels, m.frames), (22_050, 1, 11_025));
    assert_eq!(m.cues, vec![0, 2_756, 5_512, 8_268]);
    assert_eq!(m.points(), vec![0, 2_756, 5_512, 8_268]);
    assert!(m.acid.is_none());
}

#[test]
fn libsndfiles_sampler_loops_are_the_points_when_there_are_no_cues() {
    let m = markers(&file("libsndfile-smpl-stereo24.wav")).expect("a WAV");
    assert_eq!((m.rate, m.channels, m.frames), (44_100, 2, 4_410));
    assert!(m.cues.is_empty());
    assert_eq!(m.points(), vec![0, 2_205]);
}

#[test]
fn libsndfiles_cues_win_over_its_loop_in_a_float_file() {
    let m = markers(&file("libsndfile-cues-smpl-float.wav")).expect("a WAV");
    assert_eq!((m.rate, m.channels, m.frames), (48_000, 2, 4_800));
    assert_eq!(m.points(), vec![0, 1_200, 2_400, 3_600]);
    assert!(!m.loops.is_empty());
}

/// ACID chunks as Sony's Acidizer wrote them into three loops of **uttori-audio-wave**'s test
/// assets (github.com/uttori/uttori-audio-wave, MIT; `test/assets/`, fetched 2026-11-06):
/// each file's `fmt ` and `acid` chunks copied byte for byte (the audio is not ours to keep —
/// only these 56 bytes of each are), and a `data` chunk of the file's true length, silent. uttori's own decoder reads "AM - Dark
/// (808).wav"'s chunk as type 1, root 60, 12 beats, 4/4, tempo 0 (its `audio-wav.test.js`).
fn acidized(fmt: &str, acid: &str, data_len: u32) -> Vec<u8> {
    let hex = |h: &str| -> Vec<u8> {
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect()
    };
    let mut body = b"WAVE".to_vec();
    body.extend(hex(fmt));
    body.extend(hex(acid));
    body.extend(b"data");
    body.extend(data_len.to_le_bytes());
    body.extend(vec![0u8; data_len as usize]);
    let mut v = b"RIFF".to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    v
}

#[test]
fn acidizers_chunks_from_outside_files_are_read() {
    // "01 fx 01.wav" (SHA-256 1a4eb7d6…c1fbc3): 24-bit stereo, 44.1 kHz; flags 5, root 48,
    // 32 beats, 4/4, tempo 78.0 — and its length is 32 beats at 78 BPM.
    let m = markers(&acidized(
        "666d7420100000000100020044ac00009809040006001800",
        "6163696418000000050000003000008000000000200000000400040000009c42",
        6_513_234,
    ))
    .expect("a WAV");
    let a = m.acid.expect("its ACID chunk");
    assert_eq!((a.root, a.beats, a.meter), (48, 32, (4, 4)));
    assert_eq!(a.bpm, 78.0);
    assert_eq!((m.rate, m.channels, m.frames), (44_100, 2, 1_085_539));
    assert!((m.bpm().unwrap() - 78.0).abs() < 0.01, "{:?}", m.bpm());

    // "AM - Dark (808).wav" (ee79c947…81539f): float stereo; a one-shot (type 1), root 60,
    // 12 beats, tempo 0 — a hit, no tempo (12 beats over its 5.4 s would say 133 BPM).
    let m = markers(&acidized(
        "666d7420100000000300020044ac00002062050008002000",
        "6163696418000000010000003c008000000000000c0000000400040000000000",
        1_906_856,
    ))
    .expect("a WAV");
    let a = m.acid.expect("its ACID chunk");
    assert!(a.one_shot);
    assert_eq!((a.root, a.beats, a.meter, a.bpm), (60, 12, (4, 4), 0.0));
    assert_eq!(m.bpm(), None);

    // "Scream_FX_1.wav" (3da2deb9…de826fe): a loop whose chunk writes no tempo (its last four
    // bytes 00 00 ff ff: not a number) — the tempo comes from its 19 beats over 9.52 s.
    let m = markers(&acidized(
        "666d7420100000000100020044ac000010b1020004001000",
        "6163696418000000000000008000003c0000000013000000040004000000ffff",
        1_679_492,
    ))
    .expect("a WAV");
    let a = m.acid.expect("its ACID chunk");
    assert!(!a.one_shot && a.bpm.is_nan());
    let bpm = m.bpm().expect("a tempo from its beats");
    assert!(
        (bpm - 19.0 * 60.0 * 44_100.0 / 419_873.0).abs() < 0.01,
        "{bpm}"
    );
}

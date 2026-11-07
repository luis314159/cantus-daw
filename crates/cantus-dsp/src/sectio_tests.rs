//! Sectio's analysis: markers read from WAV bytes laid out as the RIFF, `cue `, `smpl` and
//! ACID specifications give them (the bytes written here by hand, field by field, after
//! the published layouts — Microsoft's RIFF/WAVE 1991 `cue ` and `smpl`, Sonic Foundry's
//! ACID `acid` chunk as libsndfile and SoX read it), transients on clicks at known places,
//! the grid.

use super::*;

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    if body.len() % 2 == 1 {
        v.push(0);
    }
    v
}

fn wav(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = b"WAVE".iter().copied().chain(chunks.concat()).collect();
    let mut v = b"RIFF".to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    v
}

fn fmt(rate: u32, channels: u16) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend(1u16.to_le_bytes()); // PCM
    f.extend(channels.to_le_bytes());
    f.extend(rate.to_le_bytes());
    f.extend((rate * channels as u32 * 2).to_le_bytes());
    f.extend((channels * 2).to_le_bytes()); // block align
    f.extend(16u16.to_le_bytes());
    chunk(b"fmt ", &f)
}

fn cue(points: &[u32]) -> Vec<u8> {
    let mut c = (points.len() as u32).to_le_bytes().to_vec();
    for (i, p) in points.iter().enumerate() {
        c.extend((i as u32 + 1).to_le_bytes()); // dwName
        c.extend(p.to_le_bytes()); // dwPosition (play order)
        c.extend(b"data"); // fccChunk
        c.extend(0u32.to_le_bytes()); // dwChunkStart
        c.extend(0u32.to_le_bytes()); // dwBlockStart
        c.extend(p.to_le_bytes()); // dwSampleOffset
    }
    chunk(b"cue ", &c)
}

fn acid(beats: u32, bpm: f32) -> Vec<u8> {
    let mut a = Vec::new();
    a.extend(0u32.to_le_bytes()); // flags: a loop
    a.extend(60u16.to_le_bytes()); // root
    a.extend(0x8000u16.to_le_bytes());
    a.extend(0f32.to_le_bytes());
    a.extend(beats.to_le_bytes());
    a.extend(4u16.to_le_bytes()); // meter denominator
    a.extend(4u16.to_le_bytes()); // meter numerator
    a.extend(bpm.to_le_bytes());
    chunk(b"acid", &a)
}

fn data(frames: u32, channels: u16) -> Vec<u8> {
    chunk(b"data", &vec![0u8; (frames * channels as u32 * 2) as usize])
}

#[test]
fn cue_points_and_an_acid_chunk_are_read() {
    let w = wav(&[
        fmt(44_100, 2),
        data(88_200, 2),
        cue(&[0, 22_050, 44_100, 66_150]),
        acid(4, 120.0),
    ]);
    let m = markers(&w).expect("a WAV");
    assert_eq!(m.rate, 44_100);
    assert_eq!(m.channels, 2);
    assert_eq!(m.frames, 88_200);
    assert_eq!(m.points(), vec![0, 22_050, 44_100, 66_150]);
    let a = m.acid.unwrap();
    assert_eq!((a.beats, a.meter, a.root), (4, (4, 4), 60));
    // Four beats in two seconds: 120 BPM exactly.
    assert!((m.bpm().unwrap() - 120.0).abs() < 1e-4);
}

#[test]
fn a_smpl_loop_start_is_a_point_when_there_are_no_cues() {
    let mut s = Vec::new();
    for v in [0u32, 0, 22_675, 60, 0, 0, 0, 1, 0] {
        s.extend(v.to_le_bytes());
    }
    for v in [0u32, 0, 11_025, 33_074, 0, 0] {
        s.extend(v.to_le_bytes());
    }
    let w = wav(&[fmt(44_100, 1), data(44_100, 1), chunk(b"smpl", &s)]);
    let m = markers(&w).unwrap();
    assert!(m.cues.is_empty());
    assert_eq!(m.points(), vec![11_025]);
    assert_eq!(m.acid, None);
}

#[test]
fn not_a_wav_reads_as_none_and_a_cut_file_does_not_panic() {
    assert_eq!(markers(b"fLaC\0\0\0\x22"), None);
    let w = wav(&[fmt(48_000, 2), data(48_000, 2), cue(&[0, 1_000])]);
    for n in 0..w.len() {
        let _ = markers(&w[..n]);
    }
}

/// Clicks on a quiet bed at known frames: every one found within 2 ms, and the
/// sensitivity drops the faint ones first.
#[test]
fn transients_land_on_clicks_and_the_sensitivity_keeps_the_strong() {
    let rate = 48_000.0f32;
    let mut x = vec![0.0f32; 96_000];
    let mut seed = 7u64;
    for v in x.iter_mut() {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        *v = ((seed >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 1e-4;
    }
    let hits: [(usize, f32); 8] = [
        (0, 0.9),
        (12_000, 0.8),
        (24_000, 0.9),
        (36_000, 0.05),
        (48_000, 0.9),
        (60_000, 0.8),
        (72_000, 0.9),
        (84_000, 0.05),
    ];
    for (at, g) in hits {
        for k in 0..2_400 {
            let env = crate::math::exp(-(k as f32) / 300.0);
            let ph = crate::math::sin(k as f32 * 0.31) + 0.5 * crate::math::sin(k as f32 * 1.7);
            x[at + k] += g * env * ph;
        }
    }
    let all = transients(&x, rate, 1.0);
    for (at, _) in hits {
        assert!(
            all.iter().any(|p| (*p as i64 - at as i64).abs() <= 96),
            "{at} not found in {all:?}"
        );
    }
    let strong = transients(&x, rate, 0.2);
    assert!(
        !strong.iter().any(|p| (*p as i64 - 36_000).abs() < 480),
        "{strong:?}"
    );
    assert!(
        strong.iter().any(|p| (*p as i64 - 48_000).abs() < 96),
        "{strong:?}"
    );
    assert_eq!(strong[0], 0);
}

#[test]
fn the_grid_steps_at_the_tempo() {
    let g = grid(96_000, 48_000, 120.0, 4, 0);
    assert_eq!(g.len(), 16);
    assert_eq!(g[1], 6_000);
    assert_eq!(g[15], 90_000);
}

/// A hostile file (review 2 #5): a `cue ` chunk of 12 bytes claiming 2³²−1 points, a `smpl`
/// claiming as many loops, and a `data` chunk claiming 0xFFFFFFFF bytes with an ACID chunk —
/// read at once, with what is there: no points, the frames the file holds, no tempo.
#[test]
fn a_hostile_count_or_length_is_read_as_what_is_there() {
    let mut cue = 0xFFFF_FFFFu32.to_le_bytes().to_vec();
    cue.extend([0u8; 8]);
    let mut smpl = vec![0u8; 28];
    smpl.extend(0xFFFF_FFFFu32.to_le_bytes());
    smpl.extend([0u8; 8]);
    let mut d = b"data".to_vec();
    d.extend(0xFFFF_FFFFu32.to_le_bytes());
    d.extend(vec![0u8; 4_800 * 2]);
    let bytes = wav(&[
        fmt(48_000, 1),
        chunk(b"cue ", &cue),
        chunk(b"smpl", &smpl),
        acid(8, 120.0),
        d,
    ]);
    let t = std::time::Instant::now();
    let m = markers(&bytes).expect("a WAV");
    assert!(t.elapsed() < std::time::Duration::from_millis(100));
    assert!(m.cues.is_empty() && m.loops.is_empty());
    assert_eq!(m.frames, 4_800);
    // 8 beats in 0.1 s would say 4 800 BPM: the tempo the chunk writes stands.
    assert_eq!(m.bpm(), Some(120.0));
}

//! `.dawproject` under round 29's third review (ADR 0061 §12): Cantus's own warp comes
//! back as it went (the warp's own markers, Follow), a `<Warps>` of Cantus's own stays
//! following, a segment across a tempo step is not the natural line, an import reads only
//! the parts it uses (an oversized one said), and audio streams past the part limit.

use cantus_core::tempo::TempoPoint;
use cantus_core::warp::{Warp, WarpMarker};
use cantus_core::*;
use cantus_project::dawproject::{self, zip};

/// A project at 120 BPM with one clip of a `seconds`-long file at `rate` (its bytes in
/// `dir`), `length` long from `start`, `offset` in.
fn project(dir: &std::path::Path, rate: u32, seconds: f64, start: Ticks, length: Ticks) -> Project {
    let mut p = Project::new("Review");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Audio");
    std::fs::write(dir.join("take.wav"), b"RIFF take").unwrap();
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: "take.wav".into(),
        sample_rate: rate,
        channels: 1,
        frames: (seconds * rate as f64) as u64,
        sha256: None,
        recorded: false,
    });
    let c = p.make_audio_clip("take", id, start, length);
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn points(p: &mut Project, pts: &[(Ticks, f64, bool)]) {
    p.tempo_points = pts
        .iter()
        .map(|&(tick, bpm, ramp)| TempoPoint { tick, bpm, ramp })
        .collect();
    p.tempo_bpm = pts[0].1;
}

/// The archive `bytes` again, as another application would have written it: its
/// application renamed, Cantus's own warps left out (`keep_side` keeps them, the file
/// Cantus's still).
fn rewritten(bytes: &[u8], foreign: bool, keep_side: bool) -> Vec<u8> {
    let mut z = zip::ZipStream::new(std::io::Cursor::new(Vec::new()));
    for (name, data) in zip::read(bytes).unwrap() {
        if name.starts_with("cantus/") && !keep_side {
            continue;
        }
        let data = if name == "project.xml" && foreign {
            String::from_utf8(data)
                .unwrap()
                .replacen("name=\"Cantus\"", "name=\"Other\"", 1)
                .into_bytes()
        } else {
            data
        };
        z.add(&name, &data, true).unwrap();
    }
    z.finish().unwrap().into_inner()
}

/// Finding 21: a following warp under a ramp (its own markers along it), one marker dragged,
/// comes back from Cantus's own file exactly: the warp's own markers still its own, Follow,
/// the transposition and the fine offset kept.
#[test]
fn a_round_trip_keeps_the_warps_own_markers() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project(dir.path(), 44_100, 40.0, Ticks::beats(4), Ticks::beats(24));
    points(
        &mut p,
        &[(Ticks::ZERO, 100.0, true), (Ticks::beats(64), 140.0, false)],
    );
    let map = p.tempo_map();
    let c = &mut p.tracks[0].clips[0];
    c.offset = Ticks::beats(2);
    let w = Warp::natural_of(&map, c.start, c.offset, c.length, 0, 44_100);
    assert!(
        w.markers.iter().any(|m| m.auto),
        "its own markers along the ramp"
    );
    let i = w
        .markers
        .iter()
        .position(|m| !m.auto && m.tick > c.offset)
        .unwrap();
    let mut w = w.move_marker(i, w.markers[i].tick + Ticks(300));
    w.cents = 150;
    let a = c.audio.as_mut().unwrap();
    a.slip_frames = 17;
    a.warp = Some(Box::new(w.clone()));
    let out = dawproject::export_with(&p, "0.1.0", Some(dir.path()));
    let back = dawproject::import(&out.bytes).unwrap();
    let b = &back.project.tracks[0].clips[0];
    let got = b.audio.as_ref().unwrap();
    assert_eq!(got.warp.as_deref(), Some(&w), "the warp as it went");
    assert_eq!(got.slip_frames, 17);
    assert_eq!(
        (b.start, b.offset),
        (p.tracks[0].clips[0].start, p.tracks[0].clips[0].offset)
    );
}

/// Finding 23: a `<Warps>` of Cantus's own on its natural line (Follow pressed, nothing
/// dragged) comes back following — without the warp beside it too (an earlier file) — and
/// another application's comes back at natural speed, said.
#[test]
fn a_cantus_warp_on_its_natural_line_comes_back_following() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project(dir.path(), 44_100, 40.0, Ticks::beats(4), Ticks::beats(16));
    points(
        &mut p,
        &[(Ticks::ZERO, 120.0, false), (Ticks::beats(12), 90.0, false)],
    );
    let map = p.tempo_map();
    let c = &mut p.tracks[0].clips[0];
    let w = Warp::natural_of(&map, c.start, c.offset, c.length, 0, 44_100);
    c.audio.as_mut().unwrap().warp = Some(Box::new(w));
    let out = dawproject::export_with(&p, "0.1.0", Some(dir.path()));
    for (foreign, follows) in [(false, true), (true, false)] {
        let bytes = rewritten(&out.bytes, foreign, false);
        let back = dawproject::import(&bytes).unwrap();
        let got = back.project.tracks[0].clips[0].audio.clone().unwrap();
        assert_eq!(
            got.warp.as_ref().is_some_and(|w| w.follow),
            follows,
            "foreign {foreign}"
        );
        let said = back
            .report
            .items
            .iter()
            .any(|i| i.key == "interop.report.d_warp_natural");
        assert_eq!(said, foreign, "foreign {foreign}: said");
    }
}

/// Finding 24: points on the curve at both ends of a segment that crosses a tempo step are
/// not the natural line: the clip keeps its warp, beat 4 heard at the file's third second.
#[test]
fn a_segment_across_a_tempo_step_is_not_the_natural_line() {
    let dir = tempfile::tempdir().unwrap();
    let rate = 48_000;
    let mut p = project(dir.path(), rate, 6.0, Ticks::ZERO, Ticks::beats(8));
    points(
        &mut p,
        &[(Ticks::ZERO, 120.0, false), (Ticks::beats(4), 60.0, false)],
    );
    // Laid at 80 BPM: content beat 8 is the file's sixth second, beat 4 its third.
    let mut w = Warp::new(rate, 80.0);
    w.markers = vec![
        WarpMarker::at(0.0, Ticks::ZERO),
        WarpMarker::at(6.0 * rate as f64, Ticks::beats(8)),
    ];
    p.tracks[0].clips[0].audio.as_mut().unwrap().warp = Some(Box::new(w));
    let out = dawproject::export_with(&p, "0.1.0", Some(dir.path()));
    let back = dawproject::import(&rewritten(&out.bytes, true, false)).unwrap();
    let c = &back.project.tracks[0].clips[0];
    let a = c.audio.as_ref().unwrap();
    let w = a.warp.as_ref().expect("a warp: its line is not the curve");
    let f = w.frame_at(Ticks::beats(4).0 as f64);
    assert!((f - 3.0 * rate as f64).abs() < 1.0, "beat 4 at {f}");
}

/// The archive of a project with a Cantus effect whose state is padded to `state` bytes, and
/// a foreign part of `foreign` bytes.
fn padded(dir: &std::path::Path, state: usize, foreign: usize) -> std::path::PathBuf {
    let mut p = project(dir, 48_000, 1.0, Ticks::ZERO, Ticks::beats(2));
    let mut fx = p.make_effect(EffectKind::Delay);
    fx.spec.set("feedback", 0.4).unwrap();
    let s = p.make_strip("Bus");
    let sid = s.id;
    p.strips.push(s);
    p.strip_mut(sid).unwrap().effects.push(fx);
    p.tracks[0].strip = Some(sid);
    let out = dawproject::export_with(&p, "0.1.0", Some(dir));
    let mut z = zip::ZipStream::new(std::io::Cursor::new(Vec::new()));
    for (name, mut data) in zip::read(&out.bytes).unwrap() {
        if name.starts_with("devices/") {
            data.resize(state.max(data.len()), b' ');
        }
        z.add(&name, &data, false).unwrap();
    }
    z.add("plugins/someone else.vstpreset", &vec![7u8; foreign], false)
        .unwrap();
    let file = dir.join("padded.dawproject");
    std::fs::write(&file, z.finish().unwrap().into_inner()).unwrap();
    file
}

/// Finding 26: an import reads only what it uses — a part no read takes is never in memory
/// (a limit below it refuses nothing) — and a state too large to read is said.
#[test]
fn an_import_reads_only_the_parts_it_uses_and_says_one_too_large() {
    let dir = tempfile::tempdir().unwrap();
    let file = padded(dir.path(), 0, 200_000);
    let xml = zip::read(&std::fs::read(&file).unwrap())
        .unwrap()
        .into_iter()
        .filter(|(n, _)| n.starts_with("devices/") || n == "project.xml")
        .map(|(_, d)| d.len())
        .max()
        .unwrap();
    // Every part it reads under the limit, the foreign one far over it.
    let media = dir.path().join("media");
    let got = dawproject::import_file_within(&file, &media, xml as u64 + 64).unwrap();
    assert!(got.too_large.is_empty());
    let fx = &got.project.strips.iter().find(|s| !s.effects.is_empty());
    assert!(fx.is_some(), "the delay restored from its state");
    // Its state past the limit: left out, and said.
    let file = padded(dir.path(), 300_000, 10);
    let got = dawproject::import_file_within(&file, &media, 100_000).unwrap();
    assert_eq!(got.too_large.len(), 1, "{:?}", got.too_large);
    assert!(got
        .report
        .items
        .iter()
        .any(|i| i.key == "interop.report.d_part_too_large"));
}

/// Finding 37: audio larger than the part limit is copied, never read as a part: an import
/// with a limit just above project.xml's size succeeds, the file on disk the source's.
#[test]
fn audio_streams_past_the_part_limit() {
    let dir = tempfile::tempdir().unwrap();
    let audio: Vec<u8> = (0..300_000u32).map(|i| (i * 13 % 251) as u8).collect();
    let mut p = project(dir.path(), 48_000, 3.0, Ticks::ZERO, Ticks::beats(4));
    std::fs::write(dir.path().join("take.wav"), &audio).unwrap();
    p.audio_pool[0].frames = 48_000;
    let file = dir.path().join("song.dawproject");
    dawproject::export_to(&p, "test", Some(dir.path()), &file).unwrap();
    let largest = zip::read(&std::fs::read(&file).unwrap())
        .unwrap()
        .into_iter()
        .filter(|(n, _)| !n.starts_with("audio/"))
        .map(|(_, d)| d.len())
        .max()
        .unwrap();
    assert!(audio.len() > largest + 1_000);
    let media = dir.path().join("media");
    let got = dawproject::import_file_within(&file, &media, largest as u64 + 16).unwrap();
    let s = &got.project.audio_pool[0];
    assert_eq!(std::fs::read(&s.path).unwrap(), audio);
}

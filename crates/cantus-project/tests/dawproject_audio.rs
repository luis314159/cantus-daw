//! Audio clips and their warps in DAWproject (round 29, ADR 0061): a clip that follows the
//! tempo written as `<Warps>` over `<Audio>`, one at its natural speed as a bare `<Audio>`
//! placed in seconds; the files in the archive (stored, once each); valid against the
//! official schema; read back with what each plays — under a tempo map too; take lanes
//! left out and said; audio tracks written as audio; external files found next to the
//! archive.

mod support;
use cantus_core::warp::{Warp, WarpMode};
use cantus_core::*;
use cantus_project::dawproject;
use support::xsd::Schema;

fn schema() -> Schema {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dawproject/");
    Schema::parse(&std::fs::read_to_string(format!("{dir}Project.xsd")).unwrap())
}

/// A project with a warped loop and a recording at natural speed (slipped), both files on
/// disk in `dir`.
fn project(dir: &std::path::Path) -> Project {
    let mut p = Project::new("Audio");
    p.tempo_bpm = 128.0;
    let mut t = p.make_track("Loops");
    for (i, name) in ["drums.wav", "take.wav"].iter().enumerate() {
        let path = dir.join(name);
        // The bytes are what travels; nothing here decodes them.
        std::fs::write(&path, format!("RIFF {name}")).unwrap();
        let id = AudioSourceId(p.alloc_id());
        p.audio_pool.push(AudioSource {
            id,
            name: name.trim_end_matches(".wav").into(),
            path: name.to_string(),
            sample_rate: 44_100,
            channels: 1,
            frames: 44_100 * 4,
            sha256: None,
            recorded: i == 1,
        });
        let mut c = p.make_audio_clip(*name, id, Ticks::beats(8 * i as i64), Ticks::beats(8));
        let a = c.audio.as_mut().unwrap();
        a.fade_in = Ticks(240);
        if i == 0 {
            let mut w = Warp::identity(44_100, 92.0, 0)
                .add_marker(Ticks::beats(4))
                .unwrap();
            w = w.move_marker(1, Ticks::beats(5));
            w.mode = WarpMode::Rhythm;
            a.warp = Some(Box::new(w));
        } else {
            a.slip_frames = 441;
        }
        t.clips.push(c);
    }
    p.tracks.push(t);
    p
}

/// The source frame a clip plays at timeline tick `t`.
fn heard(p: &Project, c: &Clip, t: Ticks) -> f64 {
    let rate = p
        .audio_source(c.audio.as_ref().unwrap().source)
        .unwrap()
        .sample_rate;
    audio_time::clip_frame_at(
        &p.tempo_map(),
        c.start,
        c.offset,
        c.audio.as_ref().unwrap(),
        rate as f64,
        t.0 as f64,
    )
}

#[test]
fn audio_clips_are_written_valid_with_their_files() {
    let dir = tempfile::tempdir().unwrap();
    let p = project(dir.path());
    let parts = dawproject::write::parts_with(&p, "0.1.0", Some(dir.path()));
    let errors = schema().validate(&parts.project_xml);
    assert!(errors.is_empty(), "project.xml:\n{}", errors.join("\n"));
    let doc = roxmltree::Document::parse(&parts.project_xml).unwrap();
    let files: Vec<&str> = doc
        .descendants()
        .filter(|n| n.has_tag_name("File"))
        .map(|n| n.attribute("path").unwrap())
        .collect();
    assert_eq!(files.len(), 2);
    for f in files {
        assert!(parts.audio.iter().any(|(p, _)| p == f), "{f} not carried");
    }
    // The warped loop follows the tempo; the take is a bare file at its natural speed.
    assert_eq!(
        doc.descendants()
            .filter(|n| n.has_tag_name("Warps"))
            .count(),
        1
    );
    assert!(parts.project_xml.contains("algorithm=\"beats\""));
    assert!(parts.project_xml.contains("contentTimeUnit=\"seconds\""));
    // A track of audio only is written as audio, with no instrument.
    let track = doc.descendants().find(|n| n.has_tag_name("Track")).unwrap();
    assert_eq!(track.attribute("contentType"), Some("audio"));
}

#[test]
fn audio_clips_come_back_with_their_warps() {
    let dir = tempfile::tempdir().unwrap();
    let before = project(dir.path());
    let out = dawproject::export_with(&before, "0.1.0", Some(dir.path()));
    let back = dawproject::import(&out.bytes).unwrap();
    assert_eq!(back.media.len(), 2, "the files come with it");
    let clips: Vec<&Clip> = back.project.tracks.iter().flat_map(|t| &t.clips).collect();
    let audio: Vec<&AudioRegion> = clips.iter().filter_map(|c| c.audio.as_ref()).collect();
    assert_eq!(audio.len(), 2);
    let was = before.tracks[0].clips[0]
        .audio
        .as_ref()
        .unwrap()
        .warp
        .clone()
        .unwrap();
    let w = audio[0].warp.as_ref().expect("the loop is warped");
    assert_eq!(w.mode, WarpMode::Rhythm);
    // Every marker of the original lands on its tick at its frame.
    for m in &was.markers {
        let f = w.frame_at(m.tick.0 as f64);
        assert!((f - m.frame).abs() < 1.0, "{m:?}: {f}");
    }
    assert_eq!(audio[0].fade_in, Ticks(240));
    // The take at its natural speed stays unwarped, and plays the same frames.
    assert!(audio[1].warp.is_none());
    let (a, b) = (&before.tracks[0].clips[1], clips[1]);
    for t in [a.start, a.start + Ticks::beats(3)] {
        let d = heard(&back.project, b, t) - heard(&before, a, t);
        assert!(d.abs() <= 1.0, "the take at {t:?}: {d:+} frames");
    }
    // A track of audio comes back able to record audio.
    assert!(back.project.tracks[0].input.is_some());
    assert!(!back.report.has("interop.report.d_audio_clips"));
}

/// Under a tempo map, a clip at natural speed stays at natural speed — the same frames
/// heard, after a tempo change — and a warp that follows comes back a warp, even on its
/// tempo's natural line.
#[test]
fn audio_clips_keep_what_they_play_under_a_tempo_map() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = project(dir.path());
    before.tempo_points = vec![
        tempo::TempoPoint {
            tick: Ticks::ZERO,
            bpm: 128.0,
            ramp: false,
        },
        tempo::TempoPoint {
            tick: Ticks::beats(6),
            bpm: 96.0,
            ramp: false,
        },
    ];
    // The loop on its own tempo's line (an identity at 128) still follows the map.
    let a = before.tracks[0].clips[0].audio.as_mut().unwrap();
    a.warp = Some(Box::new(Warp::identity(44_100, 128.0, 0)));
    let out = dawproject::export_with(&before, "0.1.0", Some(dir.path()));
    let back = dawproject::import(&out.bytes).unwrap();
    let clips: Vec<&Clip> = back.project.tracks.iter().flat_map(|t| &t.clips).collect();
    assert!(
        clips[0].audio.as_ref().unwrap().warp.is_some(),
        "the loop still follows"
    );
    let (a, b) = (&before.tracks[0].clips[1], clips[1]);
    assert!(b.audio.as_ref().unwrap().warp.is_none());
    for t in [a.start, a.start + Ticks::beats(5)] {
        let d = heard(&back.project, b, t) - heard(&before, a, t);
        assert!(d.abs() <= 1.0, "the take at {t:?}: {d:+} frames");
    }
}

/// Takes are never heard: only the playing lane is written, and the takes are said.
#[test]
fn take_lanes_are_left_out_and_said() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project(dir.path());
    let mut take = p.tracks[0].clips[1].clone();
    take.id = ClipId(p.alloc_id());
    take.lane = 1;
    p.tracks[0].clips.push(take);
    let out = dawproject::export_with(&p, "0.1.0", Some(dir.path()));
    assert!(out.report.has("interop.report.x_takes"));
    let back = dawproject::import(&out.bytes).unwrap();
    assert_eq!(back.project.tracks[0].clips.len(), 2);
}

/// The audio is stored (not deflated), each file once however many clips play it; and a
/// file written to disk streams through the same writer.
#[test]
fn audio_is_stored_once_and_streams_to_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project(dir.path());
    for k in 0..5 {
        let mut c = p.tracks[0].clips[1].clone();
        c.id = ClipId(p.alloc_id());
        c.start = Ticks::beats(20 + 8 * k);
        p.tracks[0].clips.push(c);
    }
    let parts = dawproject::write::parts_with(&p, "0.1.0", Some(dir.path()));
    assert_eq!(parts.audio.len(), 2, "each file once");
    let dest = dir.path().join("song.dawproject");
    let report = dawproject::export_to(&p, "0.1.0", Some(dir.path()), &dest).unwrap();
    assert!(!report.has("interop.report.x_audio_unread"));
    let bytes = std::fs::read(&dest).unwrap();
    assert_eq!(
        bytes,
        dawproject::export_with(&p, "0.1.0", Some(dir.path())).bytes
    );
    // Stored: the file's own bytes are in the archive as they are.
    assert!(bytes.windows(14).any(|w| w == b"RIFF drums.wav"));
}

/// A file the archive references outside itself, by a path relative to the archive, is
/// found next to it; one that is nowhere is said.
#[test]
fn an_external_file_is_found_next_to_the_archive() {
    let dir = tempfile::tempdir().unwrap();
    let p = project(dir.path());
    // Exported without a folder: the files cannot be read, so they are referenced.
    let out = dawproject::export_with(&p, "0.1.0", None);
    let next_to = tempfile::tempdir().unwrap();
    std::fs::write(next_to.path().join("drums.wav"), b"RIFF").unwrap();
    let back = dawproject::import_from(&out.bytes, Some(next_to.path())).unwrap();
    let paths: Vec<&str> = back
        .project
        .audio_pool
        .iter()
        .map(|s| s.path.as_str())
        .collect();
    assert!(paths
        .iter()
        .any(|s| std::path::Path::new(s) == next_to.path().join("drums.wav")));
    assert!(
        back.report.has("interop.report.d_audio_missing"),
        "take.wav is nowhere"
    );
}

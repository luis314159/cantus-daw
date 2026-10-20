//! Audio clips and their warps in DAWproject (round 29, ADR 0061): written as `<Warps>`
//! over `<Audio>` with the file in the archive, valid against the official schema, and
//! read back with their markers, mode, fades and natural-speed clips as they were.

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
        assert!(parts.files.iter().any(|(p, _)| p == f), "{f} not carried");
    }
    assert_eq!(
        doc.descendants()
            .filter(|n| n.has_tag_name("Warps"))
            .count(),
        2
    );
    assert!(parts.project_xml.contains("algorithm=\"beats\""));
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
    // The take at its natural speed stays unwarped, its fine offset kept.
    assert!(audio[1].warp.is_none());
    assert_eq!(audio[1].slip_frames, 441);
    // Nothing about audio clips is reported dropped any more.
    assert!(!back.report.has("interop.report.d_audio_clips"));
}

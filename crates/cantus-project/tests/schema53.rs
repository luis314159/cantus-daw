//! Schema 53 (round 36, ADR 0097): an audio clip bound to an ARA 2 plug-in — the plug-in and
//! its archive of the clip's modification — saved, loaded back, and every older file loading
//! with no clip bound.

use std::path::Path;

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn the_v53_fixture_carries_a_bound_clip_with_its_archive() {
    let p = cantus_project::load(&fixtures().join("v53.cantus"), false)
        .unwrap()
        .project;
    let (clip, ara) = p
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find_map(|c| c.audio.as_ref()?.ara.as_ref().map(|a| (c, a)))
        .expect("a bound clip");
    assert_eq!(clip.name, "ARA take");
    assert_eq!(ara.plugin.name, "Cantus Test ARA");
    assert_eq!(ara.plugin.factory_id, "org.cantus.test.ara.factory");
    let archive = ara.archive.as_ref().expect("an archive");
    assert_eq!(archive.data.bytes(), b"gain=0.5\n");
    assert_eq!(
        archive.document_archive_id,
        "org.cantus.test.ara.document.v1"
    );
    assert_eq!(
        archive.modification_id,
        cantus_core::ara::modification_persistent_id(clip.id)
    );
}

#[test]
fn a_bound_clip_survives_a_save_and_load_to_the_bit() {
    let p = cantus_project::load(&fixtures().join("v53.cantus"), false)
        .unwrap()
        .project;
    let dir = std::env::temp_dir().join(format!("cantus-schema53-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("again.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let back = cantus_project::load(&path, true).unwrap();
    assert_eq!(back.migrated_from, None);
    assert_eq!(back.project.tracks, p.tracks);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_older_fixture_loads_with_no_clip_bound() {
    let mut seen = 0;
    for entry in std::fs::read_dir(fixtures()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Some(v) = name
            .strip_prefix('v')
            .and_then(|s| s.strip_suffix(".cantus"))
        else {
            continue;
        };
        let v: u32 = v.parse().unwrap();
        if v >= 53 {
            continue;
        }
        let p = cantus_project::load(&path, false)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
            .project;
        let bound = p
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .any(|c| c.audio.as_ref().is_some_and(|a| a.ara.is_some()));
        assert!(!bound, "{name} binds a clip");
        seen += 1;
    }
    assert!(seen > 40, "only {seen} older fixtures");
}

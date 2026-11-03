//! Schema 50 (round 36, ADR 0093): VCA faders and mixer snapshots — the v50 fixture carries
//! a VCA, the strip that follows it (marked recall-safe), a snapshot and a cue, they survive a
//! save and a load, and every older fixture loads with none of them.

use cantus_core::mix_snap::MixSnapId;
use cantus_core::vca::StripKind;
use cantus_core::Ticks;
use std::path::Path;

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn check(p: &cantus_core::Project) {
    let vca = p
        .strips
        .iter()
        .find(|s| s.kind == StripKind::Vca)
        .expect("a VCA");
    assert_eq!(vca.name, "Keys VCA");
    assert_eq!(vca.gain_db, -3.0);
    let follower = p
        .strips
        .iter()
        .find(|s| s.vcas.contains(&vca.id))
        .expect("a strip that follows the VCA");
    assert_eq!(follower.name, "Keys bus");
    assert!(follower.recall_safe);
    assert_eq!(p.vca_governing(follower.id), vec![vca.id]);
    assert_eq!(p.mix.snapshots.len(), 1);
    let snap = &p.mix.snapshots[0];
    assert_eq!(snap.name, "Verse");
    assert!(snap.state.master.is_some());
    assert_eq!(snap.state.strips.len(), p.strips.len());
    assert_eq!(p.mix.cues.len(), 1);
    let cue = &p.mix.cues[0];
    assert_eq!(cue.at, Ticks::beats(8));
    assert_eq!(cue.snapshot, snap.id);
    assert!(cue.scope.is_empty());
    assert_ne!(cue.snapshot, MixSnapId(0));
}

#[test]
fn the_v50_fixture_carries_a_vca_its_follower_a_snapshot_and_a_cue() {
    let p = cantus_project::load(&fixtures().join("v50.cantus"), false)
        .unwrap()
        .project;
    check(&p);
    // Saved and loaded again, the same.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("again.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let back = cantus_project::load(&path, false).unwrap().project;
    check(&back);
    assert_eq!(back.strips, p.strips);
    assert_eq!(back.mix, p.mix);
}

#[test]
fn older_fixtures_have_no_vca_and_no_snapshot() {
    for v in [
        "v49.cantus",
        "v48.cantus",
        "v36.cantus",
        "v20.cantus",
        "v9.cantus",
    ] {
        let p = cantus_project::load(&fixtures().join(v), false)
            .unwrap()
            .project;
        assert!(!p.has_vca(), "{v}");
        assert!(p.mix.is_empty(), "{v}");
        assert!(
            p.strips
                .iter()
                .all(|s| s.vcas.is_empty() && !s.recall_safe && !s.solo_safe),
            "{v}"
        );
    }
}

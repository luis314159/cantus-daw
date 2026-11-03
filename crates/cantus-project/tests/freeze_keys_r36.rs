//! Round 36 (ADR 0092 §2): freeing a pre-fader sender's fader changed the freeze subject
//! of the regions that hold one, and only those. Every frozen target of the fixtures whose
//! region has no pre-fader-only sender and no mix cue reaching it keeps the content key it had before the change
//! (`fixtures/freeze_keys_before_r36.txt`, computed by round 35's subject from the same
//! files): no `SUBJECT_VERSION` bump, their renders are found again.

use cantus_core::freeze::{self, FreezeTarget};
use std::path::PathBuf;

#[test]
fn frozen_keys_without_a_pre_fader_sender_are_unchanged() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let before = std::fs::read_to_string(root.join("tests/fixtures/freeze_keys_before_r36.txt"))
        .unwrap()
        .replace("\r\n", "\n");
    let mut checked = 0;
    let mut freed = 0;
    for line in before.lines().filter(|l| !l.trim().is_empty()) {
        let (v, rest) = line.split_once(' ').unwrap();
        let (target_text, key) = rest.rsplit_once(' ').unwrap();
        let path = root.join("fixtures").join(format!("{v}.cantus"));
        let project = cantus_project::load(&path, false)
            .unwrap_or_else(|e| panic!("{v} loads: {e}"))
            .project;
        let target = project
            .frozen_targets()
            .into_iter()
            .find(|t| format!("{t:?}") == target_text)
            .unwrap_or_else(|| panic!("{v}: {target_text} is frozen"));
        let r = freeze::region(&project, target);
        let strip = match target {
            FreezeTarget::Strip(s) => Some(s),
            FreezeTarget::Track(_) => None,
        };
        let now = freeze::content_key(&project, target).unwrap();
        // A mix cue (ADR 0093) reaching the region is in its key since round 36 too.
        let cued = freeze::subject(&project, target).is_some_and(|v| v.get("cues").is_some());
        if freeze::pre_only_senders(&project, &r, strip).is_empty() && !cued {
            assert_eq!(now, key, "{v}: {target_text}'s key moved");
            checked += 1;
        } else {
            freed += 1;
        }
    }
    assert!(
        checked > 0,
        "some frozen target was checked (freed: {freed})"
    );
}

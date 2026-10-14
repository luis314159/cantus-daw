//! The loader's memory of the faces it has read (its own test binary: the state is per
//! process). Two of the bundled files stand in for system faces, so it runs anywhere.

use cantus_ui::font_fallback::{self, Face};
use std::path::PathBuf;

fn asset(file: &str) -> Face {
    Face {
        path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/fonts")
            .join(file),
        index: 0,
        key: format!("system:{file}#0"),
    }
}

fn cmap(f: &Face) -> std::collections::HashSet<u32> {
    font_fallback::cmap_of(&std::fs::read(&f.path).unwrap(), f.index)
}

/// A face read for one character that it lacks is remembered with what it covers, and
/// used for a later character it has; a face listed twice is read and installed once.
#[test]
fn a_face_read_once_serves_later_characters_and_appears_once() {
    let icons = asset("emoji-icon-font.ttf");
    let plex = asset("IBMPlexSans-Regular.ttf");
    let (ci, cp) = (cmap(&icons), cmap(&plex));
    let only_plex = cp
        .iter()
        .copied()
        .filter(|u| !ci.contains(u))
        .min()
        .unwrap();
    let only_icons = ci
        .iter()
        .copied()
        .filter(|u| !cp.contains(u))
        .min()
        .unwrap();
    let (only_plex, only_icons) = (
        char::from_u32(only_plex).unwrap(),
        char::from_u32(only_icons).unwrap(),
    );
    // The same face twice (as when two scripts list it), and the icons first.
    let order = [icons.clone(), plex.clone(), plex.clone(), icons.clone()];

    assert!(font_fallback::resolve_among(only_plex, &order));
    assert_eq!(
        font_fallback::faces_read(),
        vec![(icons.key.clone(), false), (plex.key.clone(), true)],
        "the icons were read, kept, not installed"
    );
    assert!(font_fallback::asked(only_plex));

    // A later character only the icons have: the face read before now serves it.
    assert!(font_fallback::resolve_among(only_icons, &order));
    assert_eq!(
        font_fallback::faces_read(),
        vec![(icons.key.clone(), true), (plex.key.clone(), true)]
    );

    // Each face once in the definitions and in every chain.
    let defs = font_fallback::definitions();
    for chain in defs.families.values() {
        for key in [&icons.key, &plex.key] {
            assert_eq!(chain.iter().filter(|k| *k == key).count(), 1, "{chain:?}");
        }
    }
    // Asked again: nothing new is read or installed.
    assert!(!font_fallback::resolve_among(only_icons, &order));
    assert_eq!(font_fallback::faces_read().len(), 2);
}

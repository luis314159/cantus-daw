//! Basic Pitch's note creation is Spotify's Apache-2.0 code ported to Rust, so its NOTICE
//! travels with Cantus (Apache-2.0 §4(d)): the hand-written part of the notices carries it
//! and the generated THIRD-PARTY-NOTICES.md does too (round 34's review).

fn read(rel: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(rel))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
        .replace("\r\n", "\n")
}

#[test]
fn spotifys_notice_is_in_the_notices() {
    for f in ["xtask/notices/head.md", "THIRD-PARTY-NOTICES.md"] {
        let s = read(f);
        let at = s
            .find("## Basic Pitch's note creation, ported to Rust")
            .unwrap_or_else(|| panic!("{f}: no section for Basic Pitch's port"));
        let section = &s[at..];
        for line in [
            "Copyright 2022 Spotify AB",
            "This product includes software developed at\nSpotify AB (http://www.spotify.com/).",
            "Apache License, Version 2.0",
            "basic_pitch/note_creation.py",
        ] {
            assert!(section.contains(line), "{f}: {line:?} is missing");
        }
    }
}

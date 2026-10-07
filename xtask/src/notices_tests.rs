//! `THIRD-PARTY-NOTICES.md` names every crate a release ships, under a licence Cantus may
//! ship — a crate added to `Cargo.lock` without regenerating the file fails here.

use crate::notices::{self, allowed, problems, Crate};
use crate::notices_render::{self, collapse_apache};
use std::path::PathBuf;

#[test]
fn licence_expressions_are_read_as_spdx_says() {
    for ok in [
        "MIT",
        "MIT OR Apache-2.0",
        "MIT/Apache-2.0",
        "Apache-2.0 OR GPL-2.0-only",
        "(MIT OR Apache-2.0) AND Unicode-3.0",
        "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0",
        "Apache-2.0 AND ISC",
        "Apache-2.0 WITH LLVM-exception",
        "0BSD OR MIT OR Apache-2.0",
    ] {
        assert!(allowed(ok), "{ok} may ship");
    }
    for bad in [
        "GPL-2.0-only",
        "MIT AND GPL-2.0-only",
        "CC-BY-NC-4.0",
        "AGPL-3.0 AND Proprietary",
        "",
        "(MIT",
        "MIT OR",
    ] {
        assert!(!allowed(bad), "{bad:?} must not pass");
    }
}

#[test]
fn the_apache_terms_are_printed_once_and_notices_around_them_are_kept() {
    let ws = crate::workspace();
    let apache = std::fs::read_to_string(ws.join("LICENSE-APACHE")).unwrap();
    let collapsed = collapse_apache(&apache);
    assert!(collapsed.starts_with("[The Apache License"), "{collapsed}");
    assert!(collapsed.len() < 200, "only the reference is left");
    // A file that carries other notices around the terms keeps them.
    let wrapped = format!("Copyright 2020 Someone\n\n{apache}\n\nNOTICE: includes X by Y.\n");
    let c = collapse_apache(&wrapped);
    assert!(c.starts_with("Copyright 2020 Someone"));
    assert!(c.contains("[The Apache License"));
    assert!(c.ends_with("NOTICE: includes X by Y."));
    assert!(!c.contains("TERMS AND CONDITIONS"));
    // Text without the terms is left as it is.
    assert_eq!(collapse_apache("MIT License\r\n\r\nx\r\n"), "MIT License\n\nx");
}

fn fake(name: &str, license: &str) -> Crate {
    Crate {
        name: name.into(),
        version: "1.0.0".into(),
        license: license.into(),
        authors: vec!["A".into()],
        repository: None,
        dir: PathBuf::from("does-not-exist"),
        build_only: false,
    }
}

#[test]
fn a_crate_without_a_notice_or_with_a_forbidden_licence_is_reported() {
    let crates = [fake("good", "MIT"), fake("new", "MIT"), fake("bad", "GPL-2.0-only")];
    let text = "| good | 1.0.0 | MIT | x |\n| bad | 1.0.0 | GPL-2.0-only | x |\n";
    let found = problems(text, &crates, text);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().any(|p| p.contains("new 1.0.0 has no notice")));
    assert!(found.iter().any(|p| p.contains("bad 1.0.0: licence")));
    // Everything named and allowed, but not what would be written now: stale.
    let crates = [fake("good", "MIT")];
    let found = problems(text, &crates, "something else");
    assert_eq!(found, vec![format!("{} is stale", notices::FILE)]);
    assert!(problems(text, &crates, text).is_empty());
}

#[test]
fn a_crate_that_ships_no_licence_file_gets_the_standard_text() {
    let ws = crate::workspace();
    let out = notices_render::render(&ws, &[fake("bare", "BSL-1.0")]);
    assert!(out.contains("| bare | 1.0.0 | BSL-1.0 | declared only"));
    assert!(out.contains("Boost Software License - Version 1.0"));
    assert!(out.contains("- bare 1.0.0 — `BSL-1.0` — Copyright (c) A"));
    let out = notices_render::render(&ws, &[fake("odd", "MIT OR Zlib")]);
    assert!(
        out.contains(notices_render::MISSING),
        "a standard text we do not have is flagged"
    );
}

/// The real thing: `cargo metadata` over `Cargo.lock`, against the committed file.
#[test]
fn the_notices_name_every_crate_a_release_ships() {
    let ws = crate::workspace();
    let meta = notices::metadata(&ws).expect("cargo metadata");
    let crates = notices::shipped(&meta);
    assert!(crates.len() > 200, "the closure was found: {}", crates.len());
    for name in ["egui", "libm", "ureq", "vst3", "clap-sys", "hound"] {
        assert!(crates.iter().any(|c| c.name == name), "{name} ships");
    }
    assert!(
        crates.iter().all(|c| c.name != "proptest" && c.name != "tempfile"),
        "dev-dependencies do not ship"
    );
    assert!(
        crates.iter().any(|c| c.name == "bindgen" && c.build_only),
        "build tools are marked"
    );
    let expected = notices_render::render(&ws, &crates);
    let text = std::fs::read_to_string(ws.join(notices::FILE)).unwrap_or_default();
    let found = problems(&text, &crates, &expected);
    assert!(
        found.is_empty(),
        "{}\nrun `cargo run -p xtask -- notices` and commit {}",
        found.join("\n"),
        notices::FILE
    );
}

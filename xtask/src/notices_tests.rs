//! `THIRD-PARTY-NOTICES.md` names every crate a release ships, under a licence Cantus may
//! ship — a crate added to `Cargo.lock` without regenerating the file fails here.

use crate::notices::{self, allowed, problems, Crate};
use crate::notices_render::{self, collapse_apache};
use cantus_content::Manifest;
use std::path::PathBuf;

/// The content manifest with CC BY packs that `cantus-content`'s tests use.
fn cc_by_fixture() -> Manifest {
    let ws = crate::workspace();
    let text = std::fs::read_to_string(
        ws.join("crates/cantus-content/tests/fixtures/manifest-cc-by.json"),
    )
    .expect("the fixture manifest");
    Manifest::parse(&text).expect("a valid fixture")
}

fn no_packs() -> Manifest {
    Manifest {
        format: 1,
        updated: "2026-10-13".into(),
        packs: vec![],
    }
}

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
    assert_eq!(
        collapse_apache("MIT License\r\n\r\nx\r\n"),
        "MIT License\n\nx"
    );
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
    let crates = [
        fake("good", "MIT"),
        fake("new", "MIT"),
        fake("bad", "GPL-2.0-only"),
    ];
    let text = "| good | 1.0.0 | MIT | x |\n| bad | 1.0.0 | GPL-2.0-only | x |\n";
    let found = problems(text, &crates, &no_packs(), text);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().any(|p| p.contains("new 1.0.0 has no notice")));
    assert!(found.iter().any(|p| p.contains("bad 1.0.0: licence")));
    // Everything named and allowed, but not what would be written now: stale.
    let crates = [fake("good", "MIT")];
    let found = problems(text, &crates, &no_packs(), "something else");
    assert_eq!(found, vec![format!("{} is stale", notices::FILE)]);
    assert!(problems(text, &crates, &no_packs(), text).is_empty());
}

#[test]
fn a_crate_that_ships_no_licence_file_gets_the_standard_text() {
    let ws = crate::workspace();
    let out = notices_render::render(&ws, &[fake("bare", "BSL-1.0")], &no_packs());
    assert!(out.contains("| bare | 1.0.0 | BSL-1.0 | declared only"));
    assert!(out.contains("Boost Software License - Version 1.0"));
    assert!(out.contains("- bare 1.0.0 — `BSL-1.0` — Copyright (c) A"));
    let out = notices_render::render(&ws, &[fake("odd", "MIT OR Unlicense")], &no_packs());
    assert!(
        out.contains(notices_render::MISSING),
        "a standard text we do not have is flagged"
    );
}

/// The real thing: `cargo metadata` over `Cargo.lock`, against the committed file.
#[test]
fn the_notices_name_every_crate_a_release_ships() {
    let ws = crate::workspace();
    let crates = notices::shipped_everywhere(&ws).expect("cargo metadata");
    assert!(
        crates.len() > 200,
        "the closure was found: {}",
        crates.len()
    );
    // Linux's own (round 30, ADR 0065): the PipeWire and JACK hosts, the X11 windows.
    for name in ["pipewire", "libspa", "jack", "x11rb", "alsa"] {
        assert!(
            crates.iter().any(|c| c.name == name && !c.build_only),
            "{name}, shipped on Linux, is named"
        );
    }
    for name in ["egui", "libm", "ureq", "vst3", "clap-sys", "hound"] {
        assert!(crates.iter().any(|c| c.name == name), "{name} ships");
    }
    assert!(
        crates.iter().all(|c| c.name != "proptest"),
        "dev-dependencies do not ship"
    );
    // tempfile is a dev-dependency here, but macOS's TLS (native-tls) ships it: look at
    // Windows alone for it.
    let windows = notices::shipped(&notices::metadata(&ws, notices::TARGETS[0]).unwrap());
    assert!(
        windows.iter().all(|c| c.name != "tempfile"),
        "dev-dependencies do not ship"
    );
    assert!(
        crates.iter().any(|c| c.name == "bindgen" && c.build_only),
        "build tools are marked"
    );
    let manifest = Manifest::bundled();
    let expected = notices_render::render(&ws, &crates, &manifest);
    let text = std::fs::read_to_string(ws.join(notices::FILE)).unwrap_or_default();
    let found = problems(&text, &crates, &manifest, &expected);
    assert!(
        found.is_empty(),
        "{}\nrun `cargo run -p xtask -- notices` and commit {}",
        found.join("\n"),
        notices::FILE
    );
}

/// ADR 0042 §3: the notices carry a *Content* section generated from the manifest — every
/// CC BY pack with its credit line word for word (and its author's words about crediting),
/// every CC0 pack credited as a courtesy.
#[test]
fn the_content_section_credits_every_pack_of_the_manifest() {
    let ws = crate::workspace();
    let m = cc_by_fixture();
    let out = notices_render::render(&ws, &[fake("good", "MIT")], &m);
    let section = &out[out.find("\n## Content\n").expect("a Content section")..];
    let section = &section[..section.find("\n## Rust crates").expect("before the crates")];
    let by = &section[..section.find("### Under CC0").expect("a CC0 part")];
    for p in m.packs.iter().filter(|p| p.needs_credit()) {
        let line = p.credit_full().unwrap();
        assert!(by.contains(&line), "{} credit: {line}", p.id);
        assert!(by.contains(&p.licence_url), "{}: the licence's link", p.id);
        assert!(
            section.contains(&format!("`{}`", p.licence_file)),
            "{}",
            p.id
        );
    }
    assert!(
        section.contains("without crediting me"),
        "the author's words"
    );
    // The CC0 kit's courtesy credit sits in the CC0 table, never under CC BY.
    assert!(!by.contains("Fixture Kit"), "{by}");
    let row = section
        .lines()
        .find(|l| l.starts_with("| Fixture Kit |"))
        .expect("the CC0 row");
    assert!(row.contains("Example Studios"), "{row}");
    assert!(section.contains("CC BY 3.0") && section.contains("CC0 1.0"));
    // The head's table of contents points at it.
    assert!(out.contains("](#content)"), "linked from the contents");
    // A manifest with no CC BY pack says so instead of an empty list.
    let cc0_only = notices_render::content(&Manifest::bundled());
    assert!(cc0_only.contains("Cantus Starter Drums"), "{cc0_only}");
}

#[test]
fn a_cc_by_credit_missing_from_the_notices_fails_the_check() {
    let ws = crate::workspace();
    let m = cc_by_fixture();
    let crates = [fake("good", "MIT")];
    let expected = notices_render::render(&ws, &crates, &m);
    assert!(problems(&expected, &crates, &m, &expected).is_empty());
    let piano = m
        .pack("fixture-grand-piano")
        .unwrap()
        .credit_line()
        .unwrap();
    let edited = expected.replace(&piano, "Fixture Grand Piano");
    let found = problems(&edited, &crates, &m, &expected);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("fixture-grand-piano") && found[0].contains("credit"),
        "{found:?}"
    );
    // Named even when the file is otherwise current (not only as "stale").
    let found = problems(&edited, &crates, &m, &edited);
    assert_eq!(found.len(), 1, "{found:?}");
}

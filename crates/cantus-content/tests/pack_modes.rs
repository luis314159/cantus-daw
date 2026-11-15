//! The pack modes' download rules (ADR 0042 amended, round 40 phase 3), against a loopback
//! server — **verified only in emulation**: no real author's site was asked.
//!
//! - *Manual*: Cantus never fetches the source; the author's page opens; the file the user
//!   brings installs through the same checks (size, SHA-256, licence).
//! - *From the author's site*: one click from the author's own URL; a changed file — a
//!   hash that is not the pinned one, a missing file, a web page instead of the archive —
//!   sends the user to the author's page, *Install from a file…* offered.
//! - The manifest refuses an author-site pack whose file is not on the author's own host.

mod support;

use cantus_content::fetch::Net;
use cantus_content::install::{InstallError, Patience};
use cantus_content::manifest::{Manifest, PackMode, AUTHOR_TERMS, FORMAT};
use cantus_content::Manager;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use support::*;

const WAIT: Duration = Duration::from_secs(30);

fn manager(root: &std::path::Path, m: Manifest) -> Manager {
    Manager::with_transport(root, m, Arc::new(Net::new()), Patience::none())
}

fn moded(mode: PackMode, url: String, bytes: &[u8], page: &str) -> cantus_content::manifest::Pack {
    let mut p = zip_pack("kit", url, bytes, "", &[], "LICENSE");
    p.mode = mode;
    p.author_page = Some(page.to_string());
    p.licence = AUTHOR_TERMS.to_string();
    p
}

#[test]
fn a_manual_pack_is_never_fetched_and_installs_from_the_users_file() {
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    let bytes = zip_bytes(&[("s.wav", b"RIFF...."), ("LICENSE", b"the author's terms")]);
    let served = bytes.clone();
    let base = serve(move |_, _| {
        h.fetch_add(1, Ordering::SeqCst);
        response("200 OK", "application/zip", &served, served.len())
    });
    let root = tempfile::tempdir().unwrap();
    let p = moded(
        PackMode::Manual,
        format!("{base}/kit.zip"),
        &bytes,
        &format!("{base}/page"),
    );
    let mut m = manager(root.path(), manifest(vec![p]));
    m.install("kit");
    match &m.wait_idle(WAIT)[0].result {
        Err(InstallError::Manual { page }) => assert!(page.ends_with("/page")),
        other => panic!("{other:?}"),
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0, "the source was never asked");
    // The user brings the file: it installs through the same checks.
    let mine = root.path().join("from the author.zip");
    std::fs::write(&mine, &bytes).unwrap();
    assert!(m.install_from_file("kit", &mine));
    assert!(m.wait_idle(WAIT)[0].result.is_ok());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn a_changed_file_on_the_authors_site_sends_the_user_to_the_authors_page() {
    let bytes = zip_bytes(&[("s.wav", b"RIFF...."), ("LICENSE", b"terms")]);
    // The author replaced the file: other bytes at the same URL.
    let other = zip_bytes(&[("s.wav", b"RIFF!!!!"), ("LICENSE", b"terms")]);
    let html = b"<!DOCTYPE html><html><body>moved</body></html>".to_vec();
    let base = serve(move |path, _| match path {
        "/changed.zip" => response("200 OK", "application/zip", &other, other.len()),
        "/page.zip" => response("200 OK", "text/html", &html, html.len()),
        _ => response("404 Not Found", "text/plain", b"no", 2),
    });
    for file in ["changed.zip", "page.zip", "gone.zip"] {
        let root = tempfile::tempdir().unwrap();
        let p = moded(
            PackMode::AuthorSite,
            format!("{base}/{file}"),
            &bytes,
            &format!("{base}/kits"),
        );
        let mut m = manager(root.path(), manifest(vec![p]));
        m.install("kit");
        match &m.wait_idle(WAIT)[0].result {
            Err(InstallError::AuthorChanged { page, .. }) => {
                assert!(page.ends_with("/kits"), "{file}")
            }
            other => panic!("{file}: {other:?}"),
        }
    }
    // The pinned file still there: one click installs it.
    let good = bytes.clone();
    let base = serve(move |_, _| response("200 OK", "application/zip", &good, good.len()));
    let root = tempfile::tempdir().unwrap();
    let p = moded(
        PackMode::AuthorSite,
        format!("{base}/kit.zip"),
        &bytes,
        &format!("{base}/kits"),
    );
    let mut m = manager(root.path(), manifest(vec![p]));
    m.install("kit");
    assert!(m.wait_idle(WAIT)[0].result.is_ok());
}

#[test]
fn an_author_site_pack_must_fetch_from_the_authors_own_host() {
    let bytes = zip_bytes(&[("LICENSE", b"terms")]);
    let mut m = manifest(vec![moded(
        PackMode::AuthorSite,
        "https://mirror.example.net/kit.zip".into(),
        &bytes,
        "https://author.example.org/kits",
    )]);
    m.format = FORMAT;
    let text = serde_json::to_string(&m).unwrap();
    assert!(Manifest::parse(&text).is_err(), "a mirror is refused");
    let ok = manifest(vec![moded(
        PackMode::AuthorSite,
        "https://files.author.example.org/kit.zip".into(),
        &bytes,
        "https://www.author.example.org/kits",
    )]);
    let text = serde_json::to_string(&ok).unwrap();
    assert!(Manifest::parse(&text).is_ok(), "the author's own host");
}

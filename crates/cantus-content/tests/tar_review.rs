//! The second review of `.tar.gz` packs (round 26): GNU headers' times are not a name
//! prefix, a pax record belongs to the entry it describes, sparse files are refused,
//! names that are one file on Windows are refused, V7 folders are folders, an archive of
//! countless empty files is stopped, a cancelled or crashed unpack leaves no staging, and
//! a receipt from a newer Cantus still names an installed pack.

mod support;

use cantus_content::fetch::{file_url, Net};
use cantus_content::install::{self, InstallError, Job, Layout, Patience};
use cantus_content::manifest::{check_path, Shelf};
use cantus_content::Manager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use support::*;

const WAIT: Duration = Duration::from_secs(30);

fn manager(root: &std::path::Path, m: cantus_content::Manifest) -> Manager {
    Manager::with_transport(root, m, Arc::new(Net::new()), Patience::none())
}

/// Install `bytes` as a tar.gz pack; the result, the pack's folder and the content root.
fn install_tar(
    bytes: &[u8],
    strip: &str,
    edit: impl Fn(&mut cantus_content::Pack),
) -> (
    Result<cantus_content::Receipt, InstallError>,
    std::path::PathBuf,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = src.path().join("pack.tar.gz");
    std::fs::write(&path, bytes).unwrap();
    let mut p = tar_pack("t", file_url(&path), bytes, strip, &[], "LICENSE");
    p.installed_size = 4096;
    edit(&mut p);
    let mut m = manager(root.path(), manifest(vec![p]));
    assert!(m.install("t"));
    let r = m.wait_idle(WAIT).remove(0).result;
    let dir = m.pack_dir("t");
    (r, dir, src, root)
}

/// One pax record, `"<len> key=value\n"`, its length counting itself.
fn pax(key: &str, value: &str) -> String {
    let body = format!(" {key}={value}\n");
    let mut len = body.len() + 1;
    while format!("{len}{body}").len() != len {
        len += 1;
    }
    format!("{len}{body}")
}

fn entry<'a>(name: &'a str, kind: u8, data: &'a [u8]) -> TarEntry<'a> {
    TarEntry {
        name,
        kind,
        data,
        claims: None,
        link: "",
    }
}

#[test]
fn gnu_headers_keep_their_names_whatever_their_times_say() {
    // GNU tar 1.35, `--format=gnu -G`: atime and ctime where ustar keeps its prefix, and
    // `D` folder entries.
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/gnu-incremental.tar.gz"),
    )
    .unwrap();
    let (r, dir, _s, _root) = install_tar(&bytes, "Kit/", |_| {});
    let r = r.expect("installed");
    assert_eq!(r.files, 2);
    assert_eq!(std::fs::read(dir.join("LICENSE")).unwrap(), b"CC BY 3.0");
    assert_eq!(
        std::fs::read(dir.join("Samples/a.flac")).unwrap(),
        b"fLaC-a"
    );
}

#[test]
fn a_pax_record_belongs_to_the_entry_it_describes() {
    let records = pax("size", "12");
    let long = format!("Kit/{}LICENSE", "deep/".repeat(20));
    let mut long_data = long.clone().into_bytes();
    long_data.push(0);
    let bytes = gzip(&tar_bytes(&[
        entry("PaxHeaders/LICENSE", b'x', records.as_bytes()),
        entry("././@LongLink", b'L', &long_data),
        TarEntry {
            name: "Kit/LICENSE",
            kind: b'0',
            data: b"licence text",
            claims: Some(0),
            link: "",
        },
    ]));
    let (r, dir, _s, _root) = install_tar(&bytes, "Kit/", |p| {
        p.licence_file = format!("{}LICENSE", "deep/".repeat(20));
    });
    r.expect("installed");
    assert_eq!(
        std::fs::read(dir.join(format!("{}LICENSE", "deep/".repeat(20)))).unwrap(),
        b"licence text",
        "the pax size is the file's, not the long name's"
    );
}

#[test]
fn sparse_files_are_refused_plainly() {
    let records = format!(
        "{}{}{}",
        pax("GNU.sparse.major", "1"),
        pax("GNU.sparse.name", "Kit/LICENSE"),
        pax("GNU.sparse.realsize", "12")
    );
    let bytes = gzip(&tar_bytes(&[
        entry("PaxHeaders/LICENSE", b'x', records.as_bytes()),
        entry("Kit/GNUSparseFile.0/LICENSE", b'0', b"1\n0\n12\n"),
    ]));
    let (r, dir, _s, _root) = install_tar(&bytes, "Kit/", |_| {});
    let e = r.expect_err("refused");
    assert!(e.to_string().contains("sparse"), "{e}");
    assert!(!dir.exists());
    // Old GNU sparse entries too.
    let bytes = gzip(&tar_bytes(&[entry("Kit/LICENSE", b'S', b"")]));
    let (r, _dir, _s, _root) = install_tar(&bytes, "Kit/", |_| {});
    assert!(r.expect_err("refused").to_string().contains("sparse"));
}

#[test]
fn names_that_are_one_file_on_windows_are_refused() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "a case variant",
            tar_bytes(&[file("LICENSE", b"licence text"), file("license", b"EVIL")]),
        ),
        (
            "a trailing dot",
            tar_bytes(&[file("LICENSE", b"licence text"), file("LICENSE.", b"EVIL")]),
        ),
        (
            "a trailing space",
            tar_bytes(&[file("LICENSE", b"licence text"), file("LICENSE ", b"EVIL")]),
        ),
        (
            "the same name twice",
            tar_bytes(&[file("LICENSE", b"licence text"), file("LICENSE", b"EVIL")]),
        ),
        (
            "a device name",
            tar_bytes(&[file("LICENSE", b"CC"), file("Samples/NUL", b"x")]),
        ),
        (
            "a device name with an extension",
            tar_bytes(&[file("LICENSE", b"CC"), file("Samples/com1.flac", b"x")]),
        ),
        (
            "a folder ending in a dot",
            tar_bytes(&[file("LICENSE", b"CC"), file("Samples./a.flac", b"x")]),
        ),
    ];
    for (why, tar) in cases {
        let (r, dir, _s, _root) = install_tar(&gzip(&tar), "", |_| {});
        assert!(matches!(r, Err(InstallError::Archive(_))), "{why}: {r:?}");
        assert!(!dir.exists(), "{why}");
    }
    // The manifest's own paths follow the same rule.
    for bad in [
        "NUL",
        "Samples/aux.wav",
        "Samples./a.wav",
        "a /b.wav",
        "LPT9",
    ] {
        assert!(check_path(bad).is_err(), "{bad}");
    }
    for good in ["Nullify.wav", "console/a.wav", "Samples/a.wav", "COM10.wav"] {
        assert_eq!(check_path(good), Ok(()), "{good}");
    }
}

#[test]
fn a_v7_folder_entry_is_a_folder() {
    let bytes = gzip(&tar_bytes(&[
        file("Kit/Samples/", b""),
        file("Kit/Samples/a.wav", b"RIFF"),
        file("Kit/LICENSE", b"CC"),
    ]));
    let (r, dir, _s, _root) = install_tar(&bytes, "Kit/", |_| {});
    let r = r.expect("installed");
    assert_eq!(r.files, 2);
    assert!(dir.join("Samples").is_dir());
    assert_eq!(std::fs::read(dir.join("Samples/a.wav")).unwrap(), b"RIFF");
}

#[test]
fn countless_empty_files_are_stopped_too() {
    let names: Vec<String> = (0..3000).map(|i| format!("e/{i}.wav")).collect();
    let mut entries: Vec<TarEntry> = names.iter().map(|n| file(n, b"")).collect();
    entries.push(file("LICENSE", b"CC"));
    let bytes = gzip(&tar_bytes(&entries));
    let (r, dir, _s, _root) = install_tar(&bytes, "", |_| {});
    let e = r.expect_err("refused");
    assert!(e.to_string().contains("more than"), "{e}");
    assert!(!dir.exists());
}

#[test]
fn a_cancelled_unpack_leaves_no_staging_and_a_crashed_one_is_cleared_at_start() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = gzip(&tar_bytes(&[
        file("a.wav", b"RIFF"),
        file("LICENSE", b"CC"),
    ]));
    let path = src.path().join("p.tar.gz");
    std::fs::write(&path, &bytes).unwrap();
    let mut p = tar_pack("t", file_url(&path), &bytes, "", &[], "LICENSE");
    p.installed_size = 4096;
    let layout = Layout::new(root.path());
    let cancel = AtomicBool::new(false);
    let job = Job {
        layout: &layout,
        transport: &Net::new(),
        cancel: &cancel,
        patience: Patience::none(),
        archive: None,
        free_space: &|_| None,
    };
    // Cancelled as the unpack begins (the download is done).
    let r = install::install(&job, &p, &mut |phase| {
        if matches!(phase, install::Phase::Verifying) {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(r.is_err());
    assert!(
        !root.path().join(".staging").join("t").exists(),
        "a half-unpacked folder never stays"
    );
    // A staging folder a crash left, with a stale file in it: cleared when the manager
    // starts, and an install starts from an empty folder anyway.
    let stale = root.path().join(".staging").join("t");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("stale.wav"), b"old").unwrap();
    let mut m = manager(root.path(), manifest(vec![p.clone()]));
    assert!(!stale.exists(), "cleared at start");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("stale.wav"), b"old").unwrap();
    m.install("t");
    m.wait_idle(WAIT).remove(0).result.expect("installed");
    assert!(
        !m.pack_dir("t").join("stale.wav").exists(),
        "nothing of the old attempt"
    );
}

#[test]
fn a_receipt_with_a_shelf_from_a_newer_cantus_still_names_an_installed_pack() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("strings");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(install::RECEIPT),
        r#"{"id":"strings","version":1,"name":{"en":"Strings"},
            "licence":"CC0-1.0",
            "licence_url":"https://creativecommons.org/publicdomain/zero/1.0/legalcode",
            "licence_file":"LICENSE","credits":[],
            "instruments":[{"id":"ensemble","name":{"en":"Ensemble"},"file":"E.sfz",
            "shelf":"orchestral"}],
            "installed_at":0,"files":1,"bytes":1}"#,
    )
    .unwrap();
    let m = manager(root.path(), manifest(vec![]));
    assert!(m.is_installed("strings"));
    let list = m.instruments();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].instrument.shelf, Shelf::Other);
}

//! `.tar.gz` packs (round 26, the Salamander Grand Piano): the same rules as a zip — the
//! archive's hash, `strip` / `include` / `exclude`, the licence file — and nothing that
//! climbs out of the pack's folder, no links, no entry larger than the manifest allows.

mod support;

use cantus_content::fetch::{file_url, Net};
use cantus_content::install::{self, InstallError, Patience};
use cantus_content::manifest::Source;
use cantus_content::Manager;
use std::sync::Arc;
use std::time::Duration;
use support::*;

const WAIT: Duration = Duration::from_secs(30);

fn manager(root: &std::path::Path, m: cantus_content::Manifest) -> Manager {
    Manager::with_transport(root, m, Arc::new(Net::new()), Patience::none())
}

/// Install `bytes` as a tar.gz pack named `id`; the result and the pack's folder.
fn install_tar(
    bytes: &[u8],
    strip: &str,
    include: &[&str],
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
    let mut p = tar_pack("t", file_url(&path), bytes, strip, include, "LICENSE");
    p.installed_size = 4096;
    edit(&mut p);
    let mut m = manager(root.path(), manifest(vec![p]));
    assert!(m.install("t"));
    let r = m.wait_idle(WAIT).remove(0).result;
    let dir = m.pack_dir("t");
    (r, dir, src, root)
}

#[test]
fn a_tar_gz_pack_keeps_what_it_includes_strips_its_folder_and_writes_a_receipt() {
    let bytes = gzip(&tar_bytes(&[
        TarEntry {
            name: "Kit/",
            kind: b'5',
            data: b"",
            claims: None,
            link: "",
        },
        file("Kit/Samples/kick.flac", b"fLaC-kick"),
        file("Kit/Samples/wet/kick.flac", b"fLaC-wet"),
        file("Kit/Kit.sfz", b"<region> sample=Samples/kick.flac"),
        file("Kit/Manual.pdf", b"%PDF"),
        file("Kit/LICENSE", b"CC BY 3.0"),
    ]));
    let (r, dir, _s, root) =
        install_tar(&bytes, "Kit/", &["Samples/**", "*.sfz", "LICENSE"], |p| {
            if let Source::TarGz(z) = &mut p.source {
                z.exclude = vec!["Samples/wet/**".into()];
            }
        });
    let r = r.expect("installed");
    assert_eq!(r.files, 3);
    assert_eq!(
        std::fs::read(dir.join("Samples/kick.flac")).unwrap(),
        b"fLaC-kick"
    );
    assert!(dir.join("Kit.sfz").is_file() && dir.join("LICENSE").is_file());
    assert!(!dir.join("Samples/wet").exists() && !dir.join("Manual.pdf").exists());
    assert!(dir.join(install::RECEIPT).is_file());
    // The downloaded archive is deleted once unpacked.
    let left = std::fs::read_dir(root.path().join(".downloads"))
        .unwrap()
        .count();
    assert_eq!(left, 0);
}

/// Archives written by another implementation (Python's `tarfile`): GNU long names
/// (`././@LongLink`) and pax headers (`path=`), both with a name over 100 bytes.
#[test]
fn gnu_long_names_and_pax_paths_are_read_as_other_tools_write_them() {
    for fixture in ["gnu.tar.gz", "pax.tar.gz"] {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture),
        )
        .unwrap();
        let (r, dir, _s, _root) = install_tar(&bytes, "Kit/", &["Samples/**", "LICENSE"], |_| {});
        let r = r.unwrap_or_else(|e| panic!("{fixture}: {e}"));
        assert_eq!(r.files, 3, "{fixture}");
        let long = format!(
            "Samples/{}/Kick hard 1.flac",
            "very-long-folder-name-".repeat(5)
        );
        assert_eq!(
            std::fs::read(dir.join(&long)).unwrap(),
            b"fLaC-long",
            "{fixture}"
        );
        assert!(!dir.join("Manual.pdf").exists());
    }
}

#[test]
fn names_that_climb_out_and_links_are_refused_and_nothing_is_installed() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "a ../ name",
            tar_bytes(&[file("../evil.flac", b"x"), file("LICENSE", b"CC")]),
        ),
        (
            "a ../ inside",
            tar_bytes(&[
                file("Samples/../../evil.flac", b"x"),
                file("LICENSE", b"CC"),
            ]),
        ),
        (
            "an absolute path",
            tar_bytes(&[file("/etc/evil.flac", b"x"), file("LICENSE", b"CC")]),
        ),
        (
            "a drive letter",
            tar_bytes(&[file("C:/evil.flac", b"x"), file("LICENSE", b"CC")]),
        ),
        (
            "a backslash",
            tar_bytes(&[
                file("Samples\\..\\..\\evil.flac", b"x"),
                file("LICENSE", b"CC"),
            ]),
        ),
        (
            "a symbolic link",
            tar_bytes(&[
                TarEntry {
                    name: "Samples/kick.flac",
                    kind: b'2',
                    data: b"",
                    claims: None,
                    link: "/etc/passwd",
                },
                file("LICENSE", b"CC"),
            ]),
        ),
        (
            "a hard link",
            tar_bytes(&[
                file("LICENSE", b"CC"),
                TarEntry {
                    name: "Samples/kick.flac",
                    kind: b'1',
                    data: b"",
                    claims: None,
                    link: "LICENSE",
                },
            ]),
        ),
    ];
    for (why, tar) in cases {
        let (r, dir, _s, root) = install_tar(&gzip(&tar), "", &[], |_| {});
        assert!(matches!(r, Err(InstallError::Archive(_))), "{why}: {r:?}");
        assert!(!dir.exists(), "{why}: nothing installed");
        assert!(
            !root.path().join("evil.flac").exists() && !root.path().join(".staging/t").exists(),
            "{why}: nothing written outside, staging cleared"
        );
    }
}

#[test]
fn an_archive_larger_than_the_manifest_says_is_refused() {
    // A gzip bomb: a few KB that unpack to 8 MiB, where the pack says 4 KB.
    let big = vec![0u8; 8 << 20];
    let bytes = gzip(&tar_bytes(&[
        file("big.flac", &big),
        file("LICENSE", b"CC"),
    ]));
    assert!(bytes.len() < 64 << 10);
    let (r, dir, _s, _root) = install_tar(&bytes, "", &[], |_| {});
    let e = r.expect_err("refused");
    assert!(e.to_string().contains("more than"), "{e}");
    assert!(!dir.exists());
    // A header that claims an enormous entry is refused before anything is read.
    let bytes = gzip(&tar_bytes(&[
        TarEntry {
            name: "huge.flac",
            kind: b'0',
            data: b"tiny",
            claims: Some(16 << 30),
            link: "",
        },
        file("LICENSE", b"CC"),
    ]));
    let (r, _dir, _s, _root) = install_tar(&bytes, "", &[], |_| {});
    assert!(matches!(r, Err(InstallError::Archive(_))), "{r:?}");
    // The same rule for a zip.
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let z = zip_bytes(&[("big.wav", &big), ("LICENSE", b"CC")]);
    let path = src.path().join("big.zip");
    std::fs::write(&path, &z).unwrap();
    let mut p = zip_pack("z", file_url(&path), &z, "", &[], "LICENSE");
    p.installed_size = 4096;
    let mut m = manager(root.path(), manifest(vec![p]));
    m.install("z");
    let r = m.wait_idle(WAIT).remove(0).result;
    assert!(matches!(r, Err(InstallError::Archive(_))), "{r:?}");
}

#[test]
fn a_damaged_header_or_a_truncated_archive_is_refused() {
    let mut tar = tar_bytes(&[file("a.flac", b"fLaC"), file("LICENSE", b"CC")]);
    tar[0] ^= 0x20; // the name changes; the checksum no longer matches
    let (r, _d, _s, _r) = install_tar(&gzip(&tar), "", &[], |_| {});
    assert!(matches!(r, Err(InstallError::Archive(_))), "{r:?}");
    let tar = tar_bytes(&[file("a.flac", &[7u8; 2000]), file("LICENSE", b"CC")]);
    let (r, _d, _s, _r) = install_tar(&gzip(&tar[..1000]), "", &[], |_| {});
    assert!(matches!(r, Err(InstallError::Archive(_))), "{r:?}");
}

#[test]
fn a_tar_gz_source_needs_manifest_format_2() {
    let p = tar_pack(
        "t",
        "https://example.org/t.tar.gz".into(),
        b"x",
        "",
        &[],
        "LICENSE",
    );
    let mut m = manifest(vec![p]);
    assert_eq!(m.validate(), Ok(()));
    m.format = 1;
    let e = m.validate().unwrap_err();
    assert!(e.to_string().contains("format 2"), "{e}");
}

/// The real Salamander archive, when it is on this machine (`CANTUS_SALAMANDER_ARCHIVE`):
/// installed from the file through the bundled manifest's entry — its hash, its strip and
/// include rules, its licence file — with the receipt naming its instruments.
#[test]
#[ignore]
fn the_real_salamander_installs_from_its_archive() {
    let Ok(archive) = std::env::var("CANTUS_SALAMANDER_ARCHIVE") else {
        eprintln!("CANTUS_SALAMANDER_ARCHIVE is not set");
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let mut m = Manager::with_transport(
        root.path(),
        cantus_content::Manifest::bundled(),
        Arc::new(Net::new()),
        Patience::none(),
    );
    let t = std::time::Instant::now();
    assert!(m.install_from_file("salamander-grand-piano", std::path::Path::new(&archive)));
    let r = m
        .wait_idle(Duration::from_secs(1800))
        .remove(0)
        .result
        .expect("installed");
    eprintln!(
        "installed in {:?}: {} files, {} bytes",
        t.elapsed(),
        r.files,
        r.bytes
    );
    let pack = m.manifest().pack("salamander-grand-piano").unwrap();
    assert_eq!(
        r.bytes, pack.installed_size,
        "the manifest's size is the archive's"
    );
    assert_eq!(r.files, 643, "639 and the four pedal noises (round 26)");
    let dir = m.pack_dir("salamander-grand-piano");
    assert!(dir.join("readme.txt").is_file());
    assert!(dir.join("SalamanderGrandPiano-V3+20200602.sfz").is_file());
    assert!(dir.join("samples/C4v16.flac").is_file());
    for f in ["pedalD1", "pedalD2", "pedalU1", "pedalU2"] {
        assert!(
            dir.join(format!("samples/{f}.flac")).is_file(),
            "the pedal noises Exemplar plays: {f}"
        );
    }
    assert_eq!(m.instruments().len(), 2);
    assert!(r.credit_line().unwrap().contains("Alexander Holm"));
}

//! The third look at pack installs (round 26): 8.3 short names, which Windows maps onto
//! long ones, are refused; two installs at once (two Cantus windows, or the same pack
//! twice) never touch each other's staging, and the crash cleanup leaves a live install
//! alone; folders count against the unpack budget like files, and a path has a sane
//! depth and length.

mod support;

use cantus_content::fetch::{file_url, Net};
use cantus_content::install::{self, InstallError, Layout, Patience};
use cantus_content::manifest::check_path;
use cantus_content::Manager;
use std::sync::Arc;
use std::time::Duration;
use support::*;

const WAIT: Duration = Duration::from_secs(60);

fn manager(root: &std::path::Path, m: cantus_content::Manifest) -> Manager {
    Manager::with_transport(root, m, Arc::new(Net::new()), Patience::none())
}

fn install_one(
    root: &std::path::Path,
    src: &std::path::Path,
    id: &str,
    bytes: &[u8],
    tar: bool,
) -> Result<cantus_content::Receipt, InstallError> {
    let path = src.join(format!("{id}.{}", if tar { "tar.gz" } else { "zip" }));
    std::fs::write(&path, bytes).unwrap();
    let mut p = if tar {
        tar_pack(id, file_url(&path), bytes, "", &[], "LICENSE")
    } else {
        zip_pack(id, file_url(&path), bytes, "", &[], "LICENSE")
    };
    p.installed_size = 4096;
    let mut m = manager(root, manifest(vec![p]));
    assert!(m.install(id));
    m.wait_idle(WAIT).remove(0).result
}

#[test]
fn eight_dot_three_short_names_are_refused_in_tar_and_zip() {
    for tar in [true, false] {
        let src = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let entries: [(&str, &[u8]); 3] = [
            ("LICENSE", b"CC"),
            ("Licence-CC-BY.txt", b"the real licence text"),
            ("LICENC~1.TXT", b"EVIL"),
        ];
        let bytes = if tar {
            gzip(&tar_bytes(
                &entries.iter().map(|(n, d)| file(n, d)).collect::<Vec<_>>(),
            ))
        } else {
            zip_bytes(&entries)
        };
        let r = install_one(root.path(), src.path(), "t", &bytes, tar);
        assert!(
            matches!(r, Err(InstallError::Archive(_))),
            "tar={tar}: {r:?}"
        );
        assert!(!root.path().join("t").exists());
    }
    for bad in [
        "LICENC~1.TXT",
        "Samples/KICKHA~2.WAV",
        "PROGRA~1/a.wav",
        "A~1",
    ] {
        assert!(check_path(bad).is_err(), "{bad}");
    }
    for good in [
        "my~file.wav",
        "Kick~soft.wav",
        "~1.wav",
        "TOOLONGNAME~1.wav",
    ] {
        assert_eq!(check_path(good), Ok(()), "{good}");
    }
}

#[test]
fn a_path_has_a_sane_depth_and_length() {
    let deep = vec!["d"; 33].join("/");
    assert!(check_path(&deep).is_err(), "33 levels");
    assert_eq!(check_path(&vec!["d"; 32].join("/")), Ok(()));
    let long = format!("{}/x.wav", "n".repeat(400));
    assert!(check_path(&long).is_err(), "over 400 bytes");
}

#[test]
fn folders_count_against_the_budget_like_files() {
    // Forty empty files, each at the end of its own 30 folders: few bytes, 1 240 things
    // to create — more than a 4 KB pack may.
    let names: Vec<String> = (0..40)
        .map(|i| {
            format!(
                "{}/f.wav",
                (0..30)
                    .map(|l| format!("{i}-{l}"))
                    .collect::<Vec<_>>()
                    .join("/")
            )
        })
        .collect();
    let mut entries: Vec<TarEntry> = Vec::new();
    for n in &names {
        entries.push(TarEntry {
            name: "././@LongLink",
            kind: b'L',
            data: n.as_bytes(),
            claims: None,
            link: "",
        });
        entries.push(file("x", b""));
    }
    entries.push(file("LICENSE", b"CC"));
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let r = install_one(
        root.path(),
        src.path(),
        "t",
        &gzip(&tar_bytes(&entries)),
        true,
    );
    let e = r.expect_err("refused");
    assert!(e.to_string().contains("more than"), "{e}");
}

#[test]
fn a_live_install_keeps_its_staging_and_a_second_install_of_it_waits_its_turn() {
    let root = tempfile::tempdir().unwrap();
    let layout = Layout::new(root.path());
    // An install in progress (another Cantus, say) holds its pack's lock.
    let live = install::Staging::begin(&layout, "t").expect("staging");
    std::fs::write(live.dir().join("half.wav"), b"RIFF").unwrap();
    // A second manager starting on the same folder leaves it alone…
    let src = tempfile::tempdir().unwrap();
    let bytes = gzip(&tar_bytes(&[
        file("a.wav", b"RIFF"),
        file("LICENSE", b"CC"),
    ]));
    let path = src.path().join("t.tar.gz");
    std::fs::write(&path, &bytes).unwrap();
    let mut p = tar_pack("t", file_url(&path), &bytes, "", &[], "LICENSE");
    p.installed_size = 4096;
    let mut m = manager(root.path(), manifest(vec![p]));
    assert!(
        live.dir().join("half.wav").is_file(),
        "a live staging stays"
    );
    // …and its own install of the same pack is refused while the other runs.
    m.install("t");
    let r = m.wait_idle(WAIT).remove(0).result;
    assert_eq!(r, Err(InstallError::Busy));
    assert!(live.dir().join("half.wav").is_file());
    // Once the other is gone, its leftovers go at the next start, and the install runs.
    let dir = live.dir().to_path_buf();
    live.abandon();
    let _ = manager(root.path(), manifest(vec![]));
    assert!(!dir.exists(), "stale staging cleared");
    m.install("t");
    m.wait_idle(WAIT).remove(0).result.expect("installed");
}

#[test]
fn two_installs_at_once_both_finish_whole() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let pack = |id: &str| {
        let names: Vec<String> = (0..200).map(|i| format!("s/{i}.wav")).collect();
        let mut entries: Vec<TarEntry> = names.iter().map(|n| file(n, b"RIFF")).collect();
        // Two different archives (one archive's download is shared by its hash).
        entries.push(file("LICENSE", id.as_bytes()));
        let bytes = gzip(&tar_bytes(&entries));
        let path = src.path().join(format!("{id}.tar.gz"));
        std::fs::write(&path, &bytes).unwrap();
        let mut p = tar_pack(id, file_url(&path), &bytes, "", &[], "LICENSE");
        p.installed_size = 1 << 20;
        p
    };
    let (a, b) = (pack("a"), pack("b"));
    let mut m1 = manager(root.path(), manifest(vec![a]));
    let mut m2 = manager(root.path(), manifest(vec![b]));
    m1.install("a");
    // A third manager starting mid-install (a new window) must not disturb either.
    let _ = manager(root.path(), manifest(vec![]));
    m2.install("b");
    let r1 = m1.wait_idle(WAIT).remove(0).result.expect("a");
    let r2 = m2.wait_idle(WAIT).remove(0).result.expect("b");
    for (r, id) in [(r1, "a"), (r2, "b")] {
        assert_eq!(r.files, 201, "{id}");
        let n = std::fs::read_dir(root.path().join(id).join("s"))
            .unwrap()
            .count();
        assert_eq!(n, 200, "{id}: every file is there");
    }
}

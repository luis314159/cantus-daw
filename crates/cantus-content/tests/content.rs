//! The content manager end to end, against `file://` fixtures and a loopback server.

mod support;

use cantus_content::fetch::{self, file_url, FetchError, Net};
use cantus_content::install::{self, InstallError, Patience};
use cantus_content::manifest::{Manifest, ManifestError, Source, FORMAT};
use cantus_content::{Manager, Status};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use support::*;

fn manager(root: &std::path::Path, m: Manifest) -> Manager {
    Manager::with_transport(root, m, Arc::new(Net::new()), Patience::none())
}

const WAIT: Duration = Duration::from_secs(30);

#[test]
fn the_bundled_manifest_is_valid_and_every_licence_accepted() {
    let m = Manifest::bundled();
    assert!(m.format <= FORMAT);
    assert!(!m.packs.is_empty());
    for p in &m.packs {
        // ADR 0042: CC0 1.0, or CC BY 3.0 / 4.0 with its credit line.
        let licence = p.licence_kind();
        assert!(licence.is_some(), "{} is {}", p.id, p.licence);
        assert!(
            !p.needs_credit() || p.credit_line().is_some(),
            "{} needs its credit line",
            p.id
        );
        assert!(
            !p.credits.is_empty()
                && p.credits
                    .iter()
                    .all(|c| c.licence_url.starts_with("https://"))
        );
        assert!(
            p.name.es.len() > 1 && p.description.es.len() > 1,
            "{} needs Spanish",
            p.id
        );
        assert!(p.installed_size > 0);
        match &p.source {
            Source::Files(f) => assert!(f.iter().all(|f| f.url.starts_with("https://"))),
            Source::Zip(z) | Source::TarGz(z) => assert!(z.url.starts_with("https://")),
        }
    }
    let starter = m.recommended().expect("a recommended pack");
    assert!(matches!(starter.source, Source::Files(_)));
    assert!(
        starter.download_size() < 60_000_000,
        "the starter kit stays small"
    );
    assert_eq!(m.packs.iter().filter(|p| p.recommended).count(), 1);
}

#[test]
fn a_manifest_from_a_newer_cantus_is_refused_by_name() {
    let text = format!(
        r#"{{"format": {}, "updated": "2030-01-01", "packs": [], "mirrors": []}}"#,
        FORMAT + 1
    );
    assert_eq!(
        Manifest::parse(&text),
        Err(ManifestError::TooNew(FORMAT + 1))
    );
    assert!(matches!(Manifest::parse("{"), Err(ManifestError::Json(_))));
}

#[test]
fn validation_catches_what_would_hurt() {
    let dir = tempfile::tempdir().unwrap();
    let good = files_pack(
        "ok",
        dir.path(),
        &[("A/kick.wav", wav(3, 64)), ("LICENSE", b"CC0".to_vec())],
    );
    assert_eq!(manifest(vec![good.clone()]).validate(), Ok(()));

    let reject = |edit: &dyn Fn(&mut cantus_content::Pack), why: &str| {
        let mut p = good.clone();
        edit(&mut p);
        assert!(manifest(vec![p]).validate().is_err(), "{why}");
    };
    reject(&|p| p.id = "Bad Id".into(), "id with spaces");
    reject(&|p| p.licence_url = "http://x".into(), "plain http licence");
    reject(&|p| p.credits.clear(), "no credit");
    reject(
        &|p| p.licence_file = "NOTICE".into(),
        "licence file not shipped",
    );
    let reject_file = |edit: &dyn Fn(&mut cantus_content::manifest::FileEntry), why: &str| {
        reject(
            &|p: &mut cantus_content::Pack| {
                if let Source::Files(f) = &mut p.source {
                    edit(&mut f[0]);
                }
            },
            why,
        )
    };
    reject_file(&|f| f.to = "../escape.wav".into(), "climbs out");
    reject_file(&|f| f.to = "C:/abs.wav".into(), "absolute");
    reject_file(&|f| f.to = r"a\b.wav".into(), "backslash");
    reject_file(&|f| f.sha256 = "ABC".into(), "short hash");
    reject_file(&|f| f.url = "http://example.org/x.wav".into(), "plain http");
    reject_file(&|f| f.to = "LICENSE".into(), "two files land in one place");
    assert!(
        manifest(vec![good.clone(), good]).validate().is_err(),
        "duplicate id"
    );
}

#[test]
fn a_files_pack_installs_with_its_licence_and_a_receipt() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let p = files_pack(
        "kit",
        src.path(),
        &[
            ("Kicks/Kick 1.wav", wav(1, 500)),
            ("Snares/Snare.wav", wav(2, 300)),
            ("Licences/LICENSE.txt", b"CC0 1.0 Universal".to_vec()),
        ],
    );
    let mut m = manager(root.path(), manifest(vec![p]));
    assert_eq!(m.status("kit"), Status::NotInstalled);
    assert!(m.install("kit"));
    assert!(
        matches!(m.status("kit"), Status::Busy(_)),
        "the install runs off the caller's thread"
    );
    let done = m.wait_idle(WAIT);
    assert_eq!(done.len(), 1);
    let receipt = done[0].result.clone().expect("installed");
    assert_eq!(
        (receipt.files, receipt.bytes),
        (3, 16 + 500 + 16 + 300 + 17)
    );
    let dir = m.pack_dir("kit");
    assert_eq!(
        std::fs::read(dir.join("Kicks/Kick 1.wav")).unwrap(),
        wav(1, 500)
    );
    assert!(dir.join("Licences/LICENSE.txt").is_file());
    assert!(dir.join(install::RECEIPT).is_file());
    assert_eq!(m.status("kit"), Status::Installed { version: 1 });
    assert!(
        !root.path().join(".staging").join("kit").exists(),
        "staging is moved, not copied"
    );

    // A fresh manager finds it again from the receipt.
    let again = manager(root.path(), m.manifest().clone());
    assert!(again.is_installed("kit"));

    // A newer version in the manifest is an update; removing takes the folder away.
    let mut newer = m.manifest().clone();
    newer.packs[0].version = 2;
    let mut m2 = manager(root.path(), newer);
    assert_eq!(m2.status("kit"), Status::Update { have: 1, new: 2 });
    m2.remove("kit").unwrap();
    assert!(!dir.exists());
    assert_eq!(m2.status("kit"), Status::NotInstalled);
}

#[test]
fn a_corrupted_download_is_refused_and_nothing_is_installed() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut p = files_pack(
        "bad",
        src.path(),
        &[("a.wav", wav(4, 100)), ("LICENSE", b"CC0".to_vec())],
    );
    // The file on the server is not the file the manifest names.
    if let Source::Files(f) = &p.source {
        std::fs::write(fetch::file_url_path(&f[0].url).unwrap(), wav(5, 100)).unwrap();
    }
    p.installed_size = 1;
    let mut m = manager(root.path(), manifest(vec![p]));
    m.install("bad");
    let done = m.wait_idle(WAIT);
    assert!(matches!(
        done[0].result,
        Err(InstallError::Fetch(FetchError::Corrupt { .. }))
    ));
    assert!(!m.pack_dir("bad").exists());
    assert_eq!(m.status("bad"), Status::NotInstalled);
    assert!(m.error("bad").is_some(), "the window can say why");
    // The bad bytes are not kept for a resume either.
    let leftovers: Vec<_> = std::fs::read_dir(root.path().join(".downloads"))
        .unwrap()
        .flatten()
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_zip_pack_keeps_only_what_it_includes_and_strips_its_folder() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(&[
        ("Kit/Samples/kick/k1.flac", b"fLaC1"),
        ("Kit/Samples/wet/k1.flac", b"fLaC2"),
        ("Kit/Programs/kit.sfz", b"<region>"),
        ("Kit/GUI/back.png", b"png"),
        ("Kit/Manual.pdf", b"pdf"),
        ("Kit/LICENSE", b"CC0"),
    ]);
    let path = src.path().join("kit.zip");
    std::fs::write(&path, &bytes).unwrap();
    let mut p = zip_pack(
        "zipkit",
        file_url(&path),
        &bytes,
        "Kit/",
        &["Samples/**", "Programs/*.sfz", "LICENSE"],
        "LICENSE",
    );
    if let Source::Zip(z) = &mut p.source {
        z.exclude = vec!["Samples/wet/**".into()];
    }
    let mut m = manager(root.path(), manifest(vec![p]));
    m.install("zipkit");
    let done = m.wait_idle(WAIT);
    let r = done[0].result.clone().expect("installed");
    assert_eq!(r.files, 3);
    let dir = m.pack_dir("zipkit");
    assert!(dir.join("Samples/kick/k1.flac").is_file());
    assert!(dir.join("Programs/kit.sfz").is_file());
    assert!(
        !dir.join("Samples/wet").exists()
            && !dir.join("GUI").exists()
            && !dir.join("Manual.pdf").exists()
    );
    // The downloaded archive is deleted once unpacked.
    assert_eq!(
        std::fs::read_dir(root.path().join(".downloads"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn a_zip_without_the_promised_licence_is_not_installed() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(&[("a.wav", b"RIFF")]);
    let path = src.path().join("x.zip");
    std::fs::write(&path, &bytes).unwrap();
    let p = zip_pack("nolic", file_url(&path), &bytes, "", &[], "LICENSE");
    let mut m = manager(root.path(), manifest(vec![p]));
    m.install("nolic");
    let done = m.wait_idle(WAIT);
    assert!(matches!(done[0].result, Err(InstallError::NoLicence(_))));
    assert!(!m.pack_dir("nolic").exists());
}

#[test]
fn an_offline_install_takes_the_users_archive_through_the_same_checks() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(&[("s.wav", b"RIFF...."), ("LICENSE", b"CC0")]);
    let mine = src.path().join("downloaded by hand.zip");
    std::fs::write(&mine, &bytes).unwrap();
    // The manifest's URL is unreachable; the user's copy is used instead.
    let p = zip_pack(
        "off",
        "https://unreachable.invalid/x.zip".into(),
        &bytes,
        "",
        &[],
        "LICENSE",
    );
    let mut m = manager(root.path(), manifest(vec![p]));
    assert!(m.install_from_file("off", &mine));
    assert!(m.wait_idle(WAIT)[0].result.is_ok());
    // A different archive is refused.
    std::fs::write(&mine, zip_bytes(&[("other.wav", b"RIFF")])).unwrap();
    m.remove("off").unwrap();
    m.install_from_file("off", &mine);
    assert!(matches!(
        m.wait_idle(WAIT)[0].result,
        Err(InstallError::Fetch(FetchError::Corrupt { .. }))
    ));
}

#[test]
fn a_proxy_page_is_recognised_and_never_written() {
    let body = b"<!DOCTYPE html><html><body>Your download is being scanned</body></html>".to_vec();
    let b2 = body.clone();
    let base = serve(move |_, _| response("200 OK", "text/html; charset=utf-8", &b2, b2.len()));
    let dir = tempfile::tempdir().unwrap();
    let (part, dest) = (dir.path().join("x.part"), dir.path().join("x.wav"));
    let r = fetch::download(
        &Net::new(),
        &format!("{base}/x.wav"),
        1000,
        &sha(&wav(1, 984)),
        &part,
        &dest,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert_eq!(r, Err(FetchError::Intercepted));
    assert!(!dest.exists());
    // And when the proxy lies about the type, the bytes give it away.
    let base = serve(move |_, _| response("200 OK", "application/octet-stream", &body, body.len()));
    let r = fetch::download(
        &Net::new(),
        &format!("{base}/x.wav"),
        1000,
        &sha(&wav(1, 984)),
        &part,
        &dest,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert_eq!(r, Err(FetchError::Intercepted));
    assert!(!part.exists() && !dest.exists());
    assert!(
        FetchError::Intercepted.retry_after(0).unwrap() >= Duration::from_secs(10),
        "a quarantine is waited out"
    );
}

#[test]
fn an_interrupted_download_resumes_where_it_stopped() {
    let file = wav(7, 4000);
    let total = file.len();
    let served = file.clone();
    let base = serve(move |_, range| match range {
        // First request: the connection drops half-way.
        None => response("200 OK", "audio/wav", &served[..total / 2], total),
        Some(from) => {
            let from = from as usize;
            let mut r = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Type: audio/wav\r\nContent-Range: bytes {from}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                total - 1,
                total - from
            )
            .into_bytes();
            r.extend_from_slice(&served[from..]);
            r
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let (part, dest) = (dir.path().join("x.part"), dir.path().join("x.wav"));
    let url = format!("{base}/x.wav");
    let first = fetch::download(
        &Net::new(),
        &url,
        total as u64,
        &sha(&file),
        &part,
        &dest,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(
        matches!(
            first,
            Err(FetchError::Incomplete { .. }) | Err(FetchError::Network(_))
        ),
        "{first:?}"
    );
    let kept = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    assert!(kept > 0, "the partial download is kept");
    assert!(first.unwrap_err().retry_after(0).is_some());
    let mut seen_from = None;
    let second = fetch::download(
        &Net::new(),
        &url,
        total as u64,
        &sha(&file),
        &part,
        &dest,
        &AtomicBool::new(false),
        &mut |n| {
            seen_from.get_or_insert(n);
        },
    );
    assert_eq!(second, Ok(()));
    assert_eq!(seen_from, Some(kept), "it went on from where it stopped");
    assert_eq!(std::fs::read(&dest).unwrap(), file);
}

#[test]
fn a_cancelled_download_stops_and_keeps_its_part() {
    let src = tempfile::tempdir().unwrap();
    let p = src.path().join("big.wav");
    std::fs::write(&p, wav(9, 200_000)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let r = fetch::download(
        &Net::new(),
        &file_url(&p),
        200_016,
        &sha(&wav(9, 200_000)),
        &dir.path().join("a.part"),
        &dir.path().join("a.wav"),
        &AtomicBool::new(true),
        &mut |_| {},
    );
    assert_eq!(r, Err(FetchError::Cancelled));
    assert!(!dir.path().join("a.wav").exists());
}

#[test]
fn a_server_error_is_retried_and_a_missing_file_is_not() {
    assert!(FetchError::Http(503).retry_after(0).is_some());
    assert!(FetchError::Http(429).retry_after(0).is_some());
    assert!(FetchError::Http(404).retry_after(0).is_none());
    assert!(FetchError::Cancelled.retry_after(0).is_none());
    // Offline: the manager reports, it does not hang or panic.
    let root = tempfile::tempdir().unwrap();
    let p = files_pack("gone", root.path(), &[("LICENSE", b"x".to_vec())]);
    let mut p = p;
    if let Source::Files(f) = &mut p.source {
        f[0].url = file_url(&root.path().join("nowhere.bin"));
    }
    let mut m = manager(&root.path().join("content"), manifest(vec![p]));
    m.install("gone");
    assert!(matches!(
        m.wait_idle(WAIT)[0].result,
        Err(InstallError::Fetch(FetchError::Http(404)))
    ));
}

#[test]
fn globs_match_like_the_manifest_says() {
    use install::glob;
    assert!(glob("Samples/**", "Samples/kick/a.wav"));
    assert!(!glob("Samples/**", "GUI/a.png"));
    assert!(glob("Programs/*.sfz", "Programs/kit.sfz"));
    assert!(!glob("Programs/*.sfz", "Programs/maps/kit.sfz"));
    assert!(glob("**/*.wav", "a/b/c.WAV"));
    assert!(glob("LICENSE", "license"));
}

/// Downloads the recommended pack for real and checks every hash. Run with
/// `CANTUS_CONTENT_NET=1 cargo test -p cantus-content -- --ignored`.
#[test]
#[ignore]
fn the_starter_pack_downloads_for_real() {
    if std::env::var("CANTUS_CONTENT_NET").is_err() {
        eprintln!("set CANTUS_CONTENT_NET=1 to run");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let mf = Manifest::bundled();
    let id = mf.recommended().unwrap().id.clone();
    let count = match &mf.recommended().unwrap().source {
        Source::Files(f) => f.len() as u64,
        Source::Zip(_) | Source::TarGz(_) => 0,
    };
    let mut m = Manager::with_transport(root.path(), mf, Arc::new(Net::new()), Patience::default());
    m.install(&id);
    let done = m.wait_idle(Duration::from_secs(30 * 60));
    let r = done[0].result.clone().expect("the starter pack installs");
    assert_eq!(r.files, count);
    let wavs = cantus_content::library::Index::build(&[m.pack_dir(&id)]);
    assert!(wavs.hits.len() as u64 >= count - 3);
}

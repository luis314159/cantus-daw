//! The update check end to end, against recorded answers and a network that is a table:
//! parse GitHub's answer, compare versions, pick the installer, verify what arrives and
//! refuse what does not match (a bad hash, a proxy's page), keep the schedule.

use cantus_update::net::{Answer, Fetch, NetError};
use cantus_update::release::{self, ParseError, Release};
use cantus_update::verify::{self, Signature, VerifyError};
use cantus_update::{Config, InstallKind, Status, Updater, Version};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const RECORDED: &str = include_str!("fixtures/github-latest-recorded.json");
const CANTUS: &str = include_str!("fixtures/cantus-latest.json");
const SUMS: &str = include_str!("fixtures/SHA256SUMS.txt");
const API: &str = "https://api.github.com/repos/luis314159/cantus/releases/latest";
const DL: &str = "https://github.com/luis314159/cantus/releases/download/v0.2.0/";
const PROXY_PAGE: &[u8] = b"<html><head><title>Internet Security by Zscaler</title></head>\
<body>The file is being scanned. Try again in a few minutes.</body></html>";

#[test]
fn a_recorded_github_answer_parses() {
    let r = Release::parse(RECORDED.as_bytes()).expect("GitHub's own answer parses");
    assert_eq!(r.version(), Version::parse("15.2.0").unwrap());
    assert!(!r.draft && !r.prerelease);
    assert!(r.assets.len() > 10);
    let a = &r.assets[0];
    assert!(a.browser_download_url.starts_with("https://github.com/"));
    assert_eq!(
        a.sha256().map(|h| h.len()),
        Some(64),
        "GitHub's digest is read"
    );
    // Another project's release has no Cantus installer.
    assert!(r.installer().is_none());
    assert!(
        r.body.as_deref().is_some_and(|b| !b.is_empty()),
        "notes are read"
    );
}

#[test]
fn a_cantus_release_names_every_file_the_package_task_makes() {
    let r = Release::parse(CANTUS.as_bytes()).unwrap();
    assert_eq!(r.version_text(), "0.2.0");
    // One Windows edition, every driver in it (round 21): no "-asio" files.
    let i = r.installer().expect("the installer");
    assert_eq!(i.name, release::installer_name("0.2.0"));
    assert_eq!(i.name, "Cantus-0.2.0-windows-x64-setup.exe");
    assert!(r.portable().is_some());
    assert!(r.assets.iter().all(|a| !a.name.contains("asio")));
    assert!(r.asset(&release::plugins_name("0.2.0")).is_some());
    // GitHub's digests agree with the sums file.
    let sums = release::parse_sums(SUMS);
    for a in r.assets.iter().filter(|a| a.name != release::SUMS_NAME) {
        assert_eq!(sums.get(&a.name), a.sha256().as_ref(), "{}", a.name);
    }
}

#[test]
fn versions_compare_and_only_newer_releases_are_offered() {
    let r = Release::parse(CANTUS.as_bytes()).unwrap();
    let v = |s| Version::parse(s).unwrap();
    assert!(r.is_update_for(&v("0.1.0")));
    assert!(r.is_update_for(&v("0.2.0-rc.1")));
    assert!(!r.is_update_for(&v("0.2.0")));
    assert!(!r.is_update_for(&v("0.10.0")));
    let mut pre = r.clone();
    pre.prerelease = true;
    assert!(
        !pre.is_update_for(&v("0.1.0")),
        "a stable build is not offered a pre-release"
    );
    assert!(
        pre.is_update_for(&v("0.2.0-beta.1")),
        "a pre-release build is"
    );
    let mut draft = r;
    draft.draft = true;
    assert!(!draft.is_update_for(&v("0.1.0")));
}

#[test]
fn what_is_not_a_release_is_refused() {
    assert_eq!(Release::parse(PROXY_PAGE), Err(ParseError::Html));
    assert_eq!(
        Release::parse(b"<!DOCTYPE html>\n<html lang=en>"),
        Err(ParseError::Html)
    );
    assert!(matches!(
        Release::parse(br#"{"message":"Not Found"}"#),
        Err(ParseError::NotARelease(_))
    ));
    let bad_tag = CANTUS.replace("\"tag_name\": \"v0.2.0\"", "\"tag_name\": \"nightly\"");
    assert_eq!(
        Release::parse(bad_tag.as_bytes()),
        Err(ParseError::BadTag("nightly".into()))
    );
}

#[test]
fn sums_files_read_both_shapes_and_skip_the_rest() {
    let h = "a".repeat(64);
    let text = format!(
        "# sums\n{h}  one.exe\n{} *two.zip\nnot a line\n{h}\n",
        "B".repeat(64)
    );
    let sums = release::parse_sums(&text);
    assert_eq!(sums.len(), 2);
    assert_eq!(sums["two.zip"], "b".repeat(64));
    let back = release::format_sums(&[("z".into(), h.clone()), ("a".into(), h.clone())]);
    assert_eq!(back, format!("{h}  a\n{h}  z\n"));
}

#[test]
fn a_bad_hash_a_wrong_size_and_a_proxy_page_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("x.exe");
    std::fs::write(&f, b"installer standard").unwrap();
    let good = verify::sha256_hex(b"installer standard");
    assert_eq!(
        verify::check_file(&f, Some(18), std::slice::from_ref(&good)),
        Ok(())
    );
    assert_eq!(
        verify::check_file(&f, Some(18), &[good.to_uppercase()]),
        Ok(()),
        "hex case does not matter"
    );
    let bad = verify::sha256_hex(b"something else");
    assert!(matches!(
        verify::check_file(&f, Some(18), &[good.clone(), bad]),
        Err(VerifyError::Hash { .. })
    ));
    assert!(matches!(
        verify::check_file(&f, Some(99), std::slice::from_ref(&good)),
        Err(VerifyError::Size {
            expected: 99,
            got: 18
        })
    ));
    assert_eq!(verify::check_file(&f, None, &[]), Err(VerifyError::NoHash));
    std::fs::write(&f, PROXY_PAGE).unwrap();
    let e = verify::check_file(&f, None, &[good]).unwrap_err();
    assert_eq!(e, VerifyError::Html);
    assert!(e.worth_retrying());
    std::fs::write(&f, b"").unwrap();
    assert_eq!(
        verify::check_file(&f, None, &["0".repeat(64)]),
        Err(VerifyError::Empty)
    );
}

#[test]
fn a_signature_must_be_valid_and_the_publisher_s() {
    let sig = |status: &str, subject: &str| Signature {
        status: status.into(),
        subject: subject.into(),
        thumbprint: "AB".into(),
        message: String::new(),
    };
    let me = "Luis Angel Almazan Lopez";
    assert_eq!(
        verify::check_signature(&sig("Valid", &format!("CN={me}, O={me}, C=MX")), me),
        Ok(())
    );
    assert_eq!(
        verify::check_signature(&sig("NotSigned", ""), me),
        Err(VerifyError::Unsigned)
    );
    assert_eq!(
        verify::check_signature(&sig("HashMismatch", &format!("CN={me}")), me),
        Err(VerifyError::Signature("HashMismatch".into()))
    );
    assert!(matches!(
        verify::check_signature(&sig("Valid", "CN=Someone Else"), me),
        Err(VerifyError::Publisher { .. })
    ));
    assert_eq!(
        verify::common_name("CN=\"Doe, Jane\", O=X"),
        Some("Doe, Jane")
    );
}

/// A network that is a table: url → answer, url → bytes for downloads.
#[derive(Default)]
struct Table {
    answers: Mutex<HashMap<String, Answer>>,
    files: Mutex<HashMap<String, Vec<Vec<u8>>>>,
    gets: AtomicUsize,
    etags_sent: Mutex<Vec<Option<String>>>,
}

impl Table {
    fn answer(&self, url: &str, status: u16, etag: Option<&str>, body: &[u8]) {
        self.answers.lock().unwrap().insert(
            url.into(),
            Answer {
                status,
                etag: etag.map(Into::into),
                rate_reset: None,
                body: body.to_vec(),
            },
        );
    }
    /// Successive downloads of `url` get these bodies in turn (the last one repeats).
    fn file(&self, url: &str, bodies: &[&[u8]]) {
        self.files
            .lock()
            .unwrap()
            .insert(url.into(), bodies.iter().map(|b| b.to_vec()).collect());
    }
}

impl Fetch for Table {
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Answer, NetError> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.etags_sent.lock().unwrap().push(etag.map(Into::into));
        let a = self.answers.lock().unwrap().get(url).cloned();
        match a {
            Some(mut a) => {
                if etag.is_some() && etag == a.etag.as_deref() && a.status == 200 {
                    a.status = 304;
                    a.body.clear();
                }
                Ok(a)
            }
            None => Err(NetError::Network("offline".into())),
        }
    }

    fn download(
        &self,
        url: &str,
        dest: &Path,
        progress: &dyn Fn(u64, Option<u64>),
        _cancel: &AtomicBool,
    ) -> Result<(), NetError> {
        let mut files = self.files.lock().unwrap();
        let list = files.get_mut(url).ok_or(NetError::Status(404))?;
        let body = if list.len() > 1 {
            list.remove(0)
        } else {
            list[0].clone()
        };
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(dest, &body).unwrap();
        progress(body.len() as u64, Some(body.len() as u64));
        Ok(())
    }
}

fn config(dir: &Path, current: &str, kind: InstallKind) -> Config {
    Config {
        repo: cantus_update::REPO.into(),
        current: Version::parse(current).unwrap(),
        kind,
        state_path: dir.join("update.json"),
        downloads: dir.join("updates"),
        publisher: None,
        held_retry: Duration::from_millis(5),
    }
}

/// Wait for the updater's thread to settle.
fn settle(u: &Updater) -> Status {
    let start = Instant::now();
    loop {
        let s = u.status();
        if !s.busy() {
            return s;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "stuck at {s:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn net_with_release() -> Arc<Table> {
    let t = Arc::new(Table::default());
    t.answer(API, 200, Some("W/\"e1\""), CANTUS.as_bytes());
    t.answer(&format!("{DL}SHA256SUMS.txt"), 200, None, SUMS.as_bytes());
    t
}

#[test]
fn a_check_offers_downloads_verifies_and_is_ready_to_install() {
    let dir = tempfile::tempdir().unwrap();
    let net = net_with_release();
    net.file(
        &format!("{DL}Cantus-0.2.0-windows-x64-setup.exe"),
        &[b"installer standard"],
    );
    let u = Updater::new(
        config(dir.path(), "0.1.0", InstallKind::Installed),
        net.clone(),
    );
    u.check_at(false, 1_000_000);
    let Status::Available(offer) = settle(&u) else {
        panic!("offered: {:?}", u.status())
    };
    assert_eq!(offer.version, "0.2.0");
    assert_eq!(offer.title, "Cantus 0.2.0");
    assert_eq!(offer.notes[0].text, "What is new");
    assert_eq!(
        offer.notes[2].text,
        "Automation clips: draw a curve on any knob."
    );
    assert!(offer.page.ends_with("/releases/tag/v0.2.0"));
    u.download();
    let Status::Ready { installer, .. } = settle(&u) else {
        panic!("ready: {:?}", u.status())
    };
    assert_eq!(std::fs::read(&installer).unwrap(), b"installer standard");

    // The next start: no check due, no request, and the verified download is reused.
    let before = net.gets.load(Ordering::SeqCst);
    let again = Updater::new(
        config(dir.path(), "0.1.0", InstallKind::Installed),
        net.clone(),
    );
    again.check_at(false, 1_000_000 + 3600);
    assert!(matches!(settle(&again), Status::Ready { .. }));
    assert_eq!(
        net.gets.load(Ordering::SeqCst),
        before,
        "at most once a day"
    );

    // A day later the check asks again, with the ETag: GitHub answers 304.
    again.check_at(false, 1_000_000 + schedule_day());
    assert!(matches!(settle(&again), Status::Ready { .. }));
    assert_eq!(
        net.etags_sent
            .lock()
            .unwrap()
            .last()
            .cloned()
            .flatten()
            .as_deref(),
        Some("W/\"e1\"")
    );
}

fn schedule_day() -> u64 {
    cantus_update::schedule::DAY
}

#[test]
fn a_download_with_a_bad_hash_is_refused_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let net = net_with_release();
    // Same size as the real one, different bytes.
    net.file(
        &format!("{DL}Cantus-0.2.0-windows-x64-setup.exe"),
        &[b"installer TAMPERED"],
    );
    let u = Updater::new(config(dir.path(), "0.1.0", InstallKind::Installed), net);
    u.check_at(true, 5);
    assert!(matches!(settle(&u), Status::Available(_)));
    u.download();
    let s = settle(&u);
    let Status::Failed { key, offer, .. } = &s else {
        panic!("refused: {s:?}")
    };
    assert_eq!(*key, "update.error.hash");
    assert!(
        offer.is_some(),
        "the offer stays, to try again or open the page"
    );
    assert!(!dir
        .path()
        .join("updates/Cantus-0.2.0-windows-x64-setup.exe")
        .exists());
}

#[test]
fn a_download_a_proxy_holds_is_tried_again_until_the_file_arrives() {
    let dir = tempfile::tempdir().unwrap();
    let net = net_with_release();
    net.file(
        &format!("{DL}Cantus-0.2.0-windows-x64-setup.exe"),
        &[PROXY_PAGE, PROXY_PAGE, b"installer standard"],
    );
    let u = Updater::new(config(dir.path(), "0.1.0", InstallKind::Installed), net);
    u.check_at(true, 5);
    settle(&u);
    u.download();
    assert!(matches!(settle(&u), Status::Ready { .. }));
}

#[test]
fn the_checks_failures_say_why() {
    let dir = tempfile::tempdir().unwrap();
    type Case<'a> = (Option<(u16, &'a [u8])>, &'a str);
    let cases: [Case; 4] = [
        (None, "update.error.offline"),
        (Some((200, PROXY_PAGE)), "update.error.held"),
        (
            Some((403, b"{\"message\":\"API rate limit exceeded\"}")),
            "update.error.rate_limited",
        ),
        (Some((500, b"")), "update.error.answer"),
    ];
    for (answer, want) in cases {
        let net = Arc::new(Table::default());
        if let Some((status, body)) = answer {
            net.answer(API, status, None, body);
        }
        let u = Updater::new(config(dir.path(), "0.1.0", InstallKind::Installed), net);
        u.check_at(true, 5);
        match settle(&u) {
            Status::Failed { key, .. } => assert_eq!(key, want),
            other => panic!("{want}: {other:?}"),
        }
    }
    // No release published yet: up to date, not an error.
    let net = Arc::new(Table::default());
    net.answer(API, 404, None, b"{\"message\":\"Not Found\"}");
    let u = Updater::new(config(dir.path(), "0.1.0", InstallKind::Installed), net);
    u.check_at(true, 5);
    assert!(matches!(settle(&u), Status::UpToDate { .. }));
}

#[test]
fn the_same_version_a_skipped_one_and_a_portable_copy() {
    let dir = tempfile::tempdir().unwrap();
    let net = net_with_release();
    let u = Updater::new(
        config(dir.path(), "0.2.0", InstallKind::Installed),
        net.clone(),
    );
    u.check_at(true, 5);
    assert!(matches!(settle(&u), Status::UpToDate { .. }));

    let u = Updater::new(
        config(dir.path(), "0.1.0", InstallKind::Installed),
        net.clone(),
    );
    u.check_at(true, 5);
    assert!(matches!(settle(&u), Status::Available(_)));
    u.skip();
    u.check_at(false, 5 + schedule_day());
    assert!(matches!(settle(&u), Status::UpToDate { .. }), "skipped");
    u.check_at(true, 6 + schedule_day());
    assert!(
        matches!(settle(&u), Status::Available(_)),
        "asked for: shown"
    );

    // A portable copy is told, with the page, but has nothing to install.
    let p = Updater::new(config(dir.path(), "0.1.0", InstallKind::Portable), net);
    p.check_at(true, 5);
    let Status::Available(o) = settle(&p) else {
        panic!()
    };
    assert!(o.installer.is_none());
}

#[cfg(windows)]
#[test]
fn windows_reads_an_unsigned_file_as_unsigned() {
    let dir = tempfile::tempdir().unwrap();
    // This test's own executable: a real program, and nobody signed it.
    let f: PathBuf = dir.path().join("unsigned.exe");
    std::fs::copy(std::env::current_exe().unwrap(), &f).unwrap();
    let sig = verify::authenticode(&f).expect("powershell answers");
    assert_eq!(sig.status, "NotSigned", "{sig:?}");
    assert_eq!(
        verify::check_signature(&sig, "Anyone"),
        Err(VerifyError::Unsigned)
    );
    assert_eq!(verify::check_publisher(&f, None), Ok(()));
}

const FLEXASIO: &str = include_str!("fixtures/flexasio-latest-recorded.json");

#[test]
fn flexasio_s_latest_release_names_its_installer() {
    use cantus_update::flexasio;
    let r = flexasio::parse(FLEXASIO.as_bytes()).expect("the recorded answer parses");
    assert!(
        r.tag_name.starts_with("flexasio-"),
        "not SemVer, still read"
    );
    let a = flexasio::installer(&r).expect("an .exe");
    assert!(a.name.starts_with("FlexASIO-") && a.name.ends_with(".exe"));
    assert!(a.size > 1_000_000);
    assert_eq!(flexasio::parse(PROXY_PAGE), Err(ParseError::Html));

    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join(&a.name);
    let mut asset = a.clone();
    // A program of the reported size passes; a proxy page, a wrong size or a non-program
    // does not, and none of them is ever run.
    let mut program = b"MZ".to_vec();
    program.resize(4096, 0);
    std::fs::write(&f, &program).unwrap();
    asset.size = 4096;
    assert_eq!(flexasio::check(&f, &asset), Ok(()));
    asset.size = 4097;
    assert!(matches!(
        flexasio::check(&f, &asset),
        Err(VerifyError::Size { .. })
    ));
    std::fs::write(&f, PROXY_PAGE).unwrap();
    assert_eq!(flexasio::check(&f, &asset), Err(VerifyError::Html));
    std::fs::write(&f, b"PK\x03\x04 a zip").unwrap();
    assert_eq!(flexasio::check(&f, &asset), Err(VerifyError::Html));
}

// ---- Pre-releases (the public beta, round 22) ----------------------------------------

/// The recent releases, as `GET /repos/{repo}/releases` lists them.
const LIST: &str = "https://api.github.com/repos/luis314159/cantus/releases?per_page=20";

/// The Cantus fixture re-tagged as `version`, a pre-release or not.
fn release_json(version: &str, prerelease: bool) -> String {
    CANTUS
        .replace("0.2.0", version)
        .replace(
            "\"prerelease\": false",
            &format!("\"prerelease\": {prerelease}"),
        )
}

#[test]
fn the_beta_sorts_before_the_next_beta_and_both_before_the_release() {
    let v = |s| Version::parse(s).unwrap();
    let chain = [
        "0.1.0-beta.1",
        "0.1.0-beta.2",
        "0.1.0-beta.10",
        "0.1.0-rc.1",
        "0.1.0",
        "0.1.1",
    ];
    for pair in chain.windows(2) {
        assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
    }
    // The version this build carries is one the updater can compare.
    assert!(
        Version::parse(env!("CARGO_PKG_VERSION")).is_some(),
        "the workspace version {} is SemVer",
        env!("CARGO_PKG_VERSION")
    );
    let beta2 = Release::parse(release_json("0.1.0-beta.2", true).as_bytes()).unwrap();
    assert!(beta2.is_update_for(&v("0.1.0-beta.1")));
    assert!(!beta2.is_update_for(&v("0.1.0-beta.2")));
    assert!(
        !beta2.is_update_for(&v("0.1.0")),
        "the release is never offered a beta"
    );
    let release = Release::parse(release_json("0.1.0", false).as_bytes()).unwrap();
    assert!(release.is_update_for(&v("0.1.0-beta.2")));
}

#[test]
fn the_newest_of_a_list_is_picked_and_drafts_and_strangers_are_left_out() {
    let list = format!(
        "[{}, {}, {}, {}]",
        release_json("0.1.0-beta.1", true),
        release_json("0.1.0-beta.3", true).replace("\"draft\": false", "\"draft\": true"),
        release_json("0.1.0-beta.2", true),
        CANTUS.replace("\"tag_name\": \"v0.2.0\"", "\"tag_name\": \"nightly\""),
    );
    let (r, json) = release::newest(list.as_bytes()).unwrap().expect("one is picked");
    assert_eq!(r.version_text(), "0.1.0-beta.2");
    assert_eq!(Release::parse(json.as_bytes()).unwrap(), r, "kept as it came");
    assert_eq!(release::newest(b"[]"), Ok(None));
    assert_eq!(release::newest(PROXY_PAGE), Err(ParseError::Html));
    assert!(matches!(
        release::newest(br#"{"message":"Not Found"}"#),
        Err(ParseError::NotARelease(_))
    ));
}

#[test]
fn a_beta_asks_for_the_list_and_is_offered_the_next_beta_then_the_release() {
    let dir = tempfile::tempdir().unwrap();
    let t = Arc::new(Table::default());
    let two = format!(
        "[{}, {}]",
        release_json("0.1.0-beta.2", true),
        release_json("0.1.0-beta.1", true)
    );
    t.answer(LIST, 200, Some("W/\"l1\""), two.as_bytes());
    let u = Updater::new(
        config(dir.path(), "0.1.0-beta.1", InstallKind::Installed),
        t.clone(),
    );
    u.check_at(true, 1_000_000);
    let Status::Available(offer) = settle(&u) else {
        panic!("offered: {:?}", u.status())
    };
    assert_eq!(offer.version, "0.1.0-beta.2");
    assert_eq!(
        offer.installer.map(|a| a.name).as_deref(),
        Some("Cantus-0.1.0-beta.2-windows-x64-setup.exe")
    );
    // GitHub's `/releases/latest` never names a pre-release, so a beta must not ask it:
    // the table has no answer there, and asking it would have failed as offline.

    // The release comes out: a beta is offered it.
    let three = format!(
        "[{}, {}, {}]",
        release_json("0.1.0", false),
        release_json("0.1.0-beta.2", true),
        release_json("0.1.0-beta.1", true)
    );
    t.answer(LIST, 200, Some("W/\"l2\""), three.as_bytes());
    u.check_at(true, 1_000_000 + 60);
    let Status::Available(offer) = settle(&u) else {
        panic!("offered: {:?}", u.status())
    };
    assert_eq!(offer.version, "0.1.0");

    // The newest beta itself is up to date; a list with nothing in it is too.
    t.answer(LIST, 200, Some("W/\"l3\""), two.as_bytes());
    let newest = Updater::new(
        config(dir.path(), "0.1.0-beta.2", InstallKind::Installed),
        t.clone(),
    );
    newest.check_at(true, 1_000_000 + 120);
    assert!(matches!(settle(&newest), Status::UpToDate { .. }));
    let dir2 = tempfile::tempdir().unwrap();
    let empty = Arc::new(Table::default());
    empty.answer(LIST, 200, None, b"[]");
    let u = Updater::new(
        config(dir2.path(), "0.1.0-beta.1", InstallKind::Installed),
        empty,
    );
    u.check_at(true, 1_000_000);
    assert!(matches!(settle(&u), Status::UpToDate { .. }));
}

#[test]
fn a_beta_s_kept_answer_is_reused_on_304() {
    let dir = tempfile::tempdir().unwrap();
    let t = Arc::new(Table::default());
    let two = format!(
        "[{}, {}]",
        release_json("0.1.0-beta.1", true),
        release_json("0.1.0-beta.2", true)
    );
    t.answer(LIST, 200, Some("W/\"l1\""), two.as_bytes());
    let u = Updater::new(
        config(dir.path(), "0.1.0-beta.1", InstallKind::Portable),
        t.clone(),
    );
    u.check_at(false, 1_000_000);
    assert!(matches!(settle(&u), Status::Available(_)));
    // A day later GitHub answers 304 to the ETag: the kept release is offered again.
    u.check_at(false, 1_000_000 + schedule_day());
    let Status::Available(offer) = settle(&u) else {
        panic!("offered: {:?}", u.status())
    };
    assert_eq!(offer.version, "0.1.0-beta.2");
    assert_eq!(
        t.etags_sent.lock().unwrap().last().cloned().flatten().as_deref(),
        Some("W/\"l1\"")
    );
}

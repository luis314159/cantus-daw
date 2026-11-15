//! *Install from a file…* (round 40 phase 3, ADR 0125 §3–4; `chorda-src`): a pack's own
//! file recognised by its pinned size and SHA-256 under any name; a bought instrument
//! referenced where it is; an archive unpacked into a user pack; user packs never credited.

mod support;

use cantus_content::manifest::{PackMode, Plays};
use cantus_content::user_pack::{self, Format, Recognised, UserPackError, USER_LICENCE};
use support::*;

#[test]
fn a_manual_packs_file_is_recognised_by_its_size_and_hash_whatever_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(&[("Piano/LICENSE", b"terms"), ("Piano/p.sfz", b"<region>")]);
    let mut p = zip_pack(
        "helsinki",
        "https://example.org/x.zip".into(),
        &bytes,
        "Piano/",
        &[],
        "LICENSE",
    );
    p.mode = PackMode::Manual;
    p.author_page = Some("https://example.org/helsinki".into());
    let m = manifest(vec![p]);
    // The user's browser saved it under another name.
    let saved = dir.path().join("Helsinki 1967 (1).zip");
    std::fs::write(&saved, &bytes).unwrap();
    assert_eq!(
        user_pack::recognise(&m, &saved),
        Ok(Recognised::Pack("helsinki".into()))
    );
    // One byte different: not that pack, an archive of the user's.
    let mut other = bytes.clone();
    *other.last_mut().unwrap() ^= 1;
    let changed = dir.path().join("changed.zip");
    std::fs::write(&changed, &other).unwrap();
    assert_eq!(user_pack::recognise(&m, &changed), Ok(Recognised::Archive));
    let junk = dir.path().join("notes.txt");
    std::fs::write(&junk, b"hello").unwrap();
    assert_eq!(user_pack::recognise(&m, &junk), Err(UserPackError::Unknown));
}

#[test]
fn a_bought_instrument_is_played_where_it_is_and_never_credited() {
    let lib = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let sfz = lib.path().join("Estate Grand IN.sfz");
    std::fs::write(&sfz, b"<region> sample=IN/C4.wav").unwrap();
    assert_eq!(
        user_pack::recognise(&manifest(vec![]), &sfz),
        Ok(Recognised::Instrument(Format::Sfz))
    );
    let r = user_pack::install_user(root.path(), &sfz, Plays::Chorda).unwrap();
    assert!(r.id.starts_with("user-estate-grand-in-sfz-"), "{}", r.id);
    assert_eq!(r.licence, USER_LICENCE);
    assert_eq!(r.instruments.len(), 1);
    assert_eq!(r.instruments[0].plays, Plays::Chorda);
    // The manager sees it, at the file's own place (not copied).
    let mut m = cantus_content::Manager::new(root.path(), manifest(vec![]));
    m.refresh();
    let i = m.instruments();
    assert_eq!(i.len(), 1);
    assert_eq!(
        std::fs::canonicalize(&i[0].path).unwrap(),
        std::fs::canonicalize(&sfz).unwrap()
    );
    assert!(!root.path().join(&r.id).join("Estate Grand IN.sfz").exists());
    // The same file again is the same pack.
    let again = user_pack::install_user(root.path(), &sfz, Plays::Chorda).unwrap();
    assert_eq!(again.id, r.id);
    // Never in the credits: it is the user's own.
    let credits = cantus_content::credits::list(&manifest(vec![]), m.installed());
    assert!(credits.is_empty());
}

#[test]
fn an_archive_is_unpacked_and_every_instrument_in_it_offered() {
    let dir = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(&[
        ("Upright/Upright.dspreset", b"<DecentSampler/>"),
        ("Upright/Upright Soft.dspreset", b"<DecentSampler/>"),
        ("Upright/Samples/C4.wav", &wav(3, 64)),
        ("__MACOSX/Upright/._Upright.dspreset", b"junk"),
        ("Upright/GM.sf2", b"RIFF"),
    ]);
    let z = dir.path().join("upright.zip");
    std::fs::write(&z, &bytes).unwrap();
    let r = user_pack::install_user(root.path(), &z, Plays::Chorda).unwrap();
    let files: Vec<_> = r.instruments.iter().map(|i| i.file.as_str()).collect();
    assert_eq!(
        files,
        [
            "Upright/GM.sf2",
            "Upright/Upright Soft.dspreset",
            "Upright/Upright.dspreset"
        ]
    );
    assert!(!root.path().join(&r.id).join("__MACOSX").exists());
    // An archive with nothing to play: refused, nothing left behind.
    let empty = dir.path().join("empty.zip");
    std::fs::write(&empty, zip_bytes(&[("a.txt", b"x")])).unwrap();
    assert_eq!(
        user_pack::install_user(root.path(), &empty, Plays::Chorda).map(|r| r.id),
        Err(UserPackError::NoInstrument)
    );
    assert!(!root.path().join(user_pack::user_id(&empty)).exists());
}

// ---- r40:fix-f (review of 40a27bb7, findings 4 and 5) ----

/// A zip of one SFZ instrument and its sample.
fn kit_zip(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let z = dir.join(name);
    std::fs::write(
        &z,
        zip_bytes(&[
            ("Kit/Kit.sfz", b"<region> sample=kick.wav key=36"),
            ("Kit/kick.wav", &wav(5, 256)),
            ("Kit/Samples/snare.wav", &wav(7, 256)),
        ]),
    )
    .unwrap();
    z
}

fn leftovers(root: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_failed_unpack_leaves_nothing_and_a_retry_installs() {
    let dir = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let z = kit_zip(dir.path(), "kit.zip");
    let need = user_pack::declared_size(&z).unwrap();
    assert_eq!(
        need,
        272 + 272 + 31,
        "the zip's entries, as it declares them"
    );
    // An older Cantus's failed unpack: a user pack's folder with no receipt.
    let stale = root.path().join(user_pack::user_id(&z));
    std::fs::create_dir_all(stale.join("Kit")).unwrap();
    std::fs::write(stale.join("Kit/half.wav"), b"half").unwrap();
    // Exactly the declared size free: the folders it makes overrun it mid-unpack.
    let tight = |_: &std::path::Path| Some(need);
    assert!(matches!(
        user_pack::install_user_with(root.path(), &z, Plays::Tympanum, &tight),
        Err(UserPackError::Io(_))
    ));
    assert!(
        leftovers(root.path()).is_empty(),
        "nothing left behind: {:?}",
        leftovers(root.path())
    );
    // The retry is a clean install.
    let roomy = |_: &std::path::Path| Some(u64::MAX / 8);
    let r = user_pack::install_user_with(root.path(), &z, Plays::Tympanum, &roomy).unwrap();
    assert_eq!(leftovers(root.path()), std::slice::from_ref(&r.id));
    assert!(root.path().join(&r.id).join("Kit/kick.wav").exists());
    // Installed again: the pack replaced, not added to.
    std::fs::write(root.path().join(&r.id).join("extra.txt"), b"x").unwrap();
    let again = user_pack::install_user_with(root.path(), &z, Plays::Tympanum, &roomy).unwrap();
    assert_eq!(again.id, r.id);
    assert!(!root.path().join(&r.id).join("extra.txt").exists());
    assert!(root
        .path()
        .join(&r.id)
        .join(cantus_content::install::RECEIPT)
        .exists());
}

#[test]
fn an_archive_bigger_than_the_free_space_is_refused_before_it_unpacks() {
    let dir = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let z = kit_zip(dir.path(), "big.zip");
    let small = |_: &std::path::Path| Some(100);
    assert_eq!(
        user_pack::install_user_with(root.path(), &z, Plays::Tympanum, &small).map(|r| r.id),
        Err(UserPackError::NoSpace {
            need: 575,
            free: 100
        })
    );
    assert!(leftovers(root.path()).is_empty());
    // At start-up, staging folders and packs without a receipt are cleaned.
    std::fs::create_dir_all(root.path().join(".user-staging-user-x-1/a")).unwrap();
    std::fs::create_dir_all(root.path().join("user-y-00000000")).unwrap();
    std::fs::create_dir_all(root.path().join("salamander")).unwrap();
    assert_eq!(user_pack::clean_orphans(root.path()), 2);
    assert_eq!(leftovers(root.path()), ["salamander"]);
}

/// DRSKit's published kit files (`fixtures/drskit_minimal`, CC BY 4.0, byte for byte) with
/// a short 13-channel 16-bit hit for every file they name — each channel its own tone, so
/// a mic's file can be told apart.
pub fn drskit_with_hits(to: &std::path::Path) -> std::path::PathBuf {
    let src =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/drskit_minimal");
    let mut stack = vec![src.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(&src).unwrap();
            if p.is_dir() {
                std::fs::create_dir_all(to.join(rel)).unwrap();
                stack.push(p);
            } else {
                std::fs::copy(&p, to.join(rel)).unwrap();
            }
        }
    }
    let kit = cantus_content::drumgizmo::read_kit(to, "DRSKit_minimal.xml").unwrap();
    for f in cantus_content::drumgizmo::files(&kit) {
        let path = cantus_content::drumgizmo::path_of(to, &f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let spec = hound::WavSpec {
            channels: 13,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..480 {
            for c in 0..13 {
                let x =
                    ((i as f32) * (0.02 + 0.01 * c as f32)).sin() * (1.0 - i as f32 / 480.0) * 0.3;
                w.write_sample((x * 32_000.0) as i16).unwrap();
            }
        }
        w.finalize().unwrap();
    }
    to.join("DRSKit_minimal.xml")
}

#[test]
fn a_users_drumgizmo_kit_is_built_for_tympanum_every_mic_its_own_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let xml = drskit_with_hits(&dir.path().join("DRSKit"));
    assert_eq!(Format::of(&xml), Some(Format::DrumGizmo));
    let r = user_pack::install_user(root.path(), &xml, Plays::Chorda).unwrap();
    assert_eq!(r.instruments.len(), 1);
    let i = &r.instruments[0];
    assert_eq!(i.plays, Plays::Tympanum, "a drum kit is Tympanum's");
    assert_eq!(i.file, user_pack::dg::SFZ);
    let pack = root.path().join(&r.id);
    let sfz = std::fs::read_to_string(pack.join(&i.file)).unwrap();
    // 13 channels, the OH and Amb pairs stereo: 11 mics, each a <master>.
    assert_eq!(
        sfz.matches("<master>").count(),
        11,
        "{}",
        &sfz[..400.min(sfz.len())]
    );
    // Its MIDI map's keys: the kick on 36, the snare on 38, the hats on 42/44 and 46 by CC4.
    for key in [
        "lokey=36",
        "lokey=38",
        "lokey=44",
        "lokey=46 hikey=46 locc4=",
    ] {
        assert!(sfz.contains(key), "{key}");
    }
    let snare = cantus_content::wav::read(&pack.join("Mics/Snare_top/Snare/01.wav")).unwrap();
    assert_eq!(snare.channels, 1);
    let oh = cantus_content::wav::read(&pack.join("Mics/OH/Snare/01.wav")).unwrap();
    assert_eq!(oh.channels, 2, "a pair is one stereo mic");
    assert!(
        pack.join("Tympanum.sfz.mics.json").exists(),
        "its main mics"
    );
}

/// The coordinator's follow-up (no doubled disk): an archive of a DrumGizmo kit, once
/// built, keeps only what plays — the SFZ, its main mics and the mics' files — and the
/// receipt; a cancelled install leaves nothing at all.
#[test]
fn a_drumgizmo_archive_keeps_only_what_plays_and_a_cancel_leaves_nothing() {
    use std::io::Write as _;
    let dir = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let kit = dir.path().join("DRSKit");
    drskit_with_hits(&kit);
    // Zipped as a user's download is: the kit in its folder.
    let z = dir.path().join("DRSKit.zip");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&z).unwrap());
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut stack = vec![kit.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir.path()).unwrap();
                w.start_file(
                    rel.to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                    opts,
                )
                .unwrap();
                w.write_all(&std::fs::read(&p).unwrap()).unwrap();
            }
        }
    }
    w.finish().unwrap();
    // Cancelled while it unpacks: nothing left behind.
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut seen = 0;
    let r = user_pack::install_user_job(
        root.path(),
        &z,
        Plays::Tympanum,
        &|_| None,
        &cancel,
        &mut |_| {
            seen += 1;
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        },
    );
    assert_eq!(r.map(|r| r.id), Err(UserPackError::Cancelled));
    assert!(seen > 0, "it reported its progress");
    assert!(
        leftovers(root.path()).is_empty(),
        "{:?}",
        leftovers(root.path())
    );
    // Installed whole: only what plays.
    let r = user_pack::install_user(root.path(), &z, Plays::Tympanum).unwrap();
    assert_eq!(r.instruments[0].file, user_pack::dg::SFZ);
    let mut kept = leftovers(&root.path().join(&r.id));
    kept.sort();
    assert_eq!(
        kept,
        [
            "Mics",
            "Tympanum.sfz",
            "Tympanum.sfz.mics.json",
            "cantus-pack.json"
        ]
    );
}

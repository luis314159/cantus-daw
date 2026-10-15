//! A pack can carry playable instruments (round 26, the Salamander Grand Piano): an SFZ
//! file inside it that the instrument picker offers once the pack is installed. The
//! manifest names them; the receipt keeps them, so an installed pack stays playable
//! whatever later manifests say.

mod support;

use cantus_content::fetch::Net;
use cantus_content::install::Patience;
use cantus_content::manifest::{PackInstrument, Shelf, Text};
use cantus_content::{Manager, Pack};
use std::sync::Arc;
use std::time::Duration;
use support::*;

fn piano() -> PackInstrument {
    PackInstrument {
        id: "grand".into(),
        name: Text {
            en: "Grand Piano".into(),
            es: "Piano de cola".into(),
        },
        file: "Piano.sfz".into(),
        shelf: Shelf::Keys,
        layers: Some(8),
        gain_db: 3.0,
        voices: None,
    }
}

fn with_piano(dir: &std::path::Path) -> Pack {
    let mut p = files_pack(
        "piano",
        dir,
        &[
            ("Piano.sfz", b"<region> sample=C4.wav".to_vec()),
            ("C4.wav", wav(1, 64)),
            ("LICENSE", b"CC BY".to_vec()),
        ],
    );
    p.instruments = vec![piano()];
    p
}

#[test]
fn a_pack_instrument_is_validated() {
    let dir = tempfile::tempdir().unwrap();
    let good = with_piano(dir.path());
    assert_eq!(manifest(vec![good.clone()]).validate(), Ok(()));
    let reject = |edit: &dyn Fn(&mut PackInstrument), why: &str| {
        let mut p = good.clone();
        edit(&mut p.instruments[0]);
        assert!(manifest(vec![p]).validate().is_err(), "{why}");
    };
    reject(&|i| i.id = "Grand Piano".into(), "an id with spaces");
    reject(&|i| i.id.clear(), "no id");
    reject(&|i| i.name.en = " ".into(), "no name");
    reject(
        &|i| i.file = "../Piano.sfz".into(),
        "a file outside the pack",
    );
    reject(&|i| i.file = "Piano.exe".into(), "not an instrument file");
    reject(&|i| i.layers = Some(0), "no layers");
    reject(&|i| i.gain_db = 40.0, "a gain no calibration needs");
    reject(&|i| i.gain_db = f32::NAN, "a gain that is not a number");
    // Two instruments with one id.
    let mut p = good.clone();
    p.instruments.push(piano());
    assert!(manifest(vec![p]).validate().is_err(), "two ids alike");
    // A files pack must carry the instrument's file.
    let mut p = good.clone();
    p.instruments[0].file = "Other.sfz".into();
    assert!(
        manifest(vec![p]).validate().is_err(),
        "a file the pack lacks"
    );
    // Format 1 builds do not know instruments.
    let mut m = manifest(vec![good]);
    m.format = 1;
    assert!(m.validate().unwrap_err().to_string().contains("format 2"));
}

#[test]
fn an_installed_pack_offers_its_instruments_from_its_receipt() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let p = with_piano(src.path());
    let mut m = Manager::with_transport(
        root.path(),
        manifest(vec![p.clone()]),
        Arc::new(Net::new()),
        Patience::none(),
    );
    assert!(m.instruments().is_empty(), "nothing before the install");
    m.install("piano");
    let r = m
        .wait_idle(Duration::from_secs(30))
        .remove(0)
        .result
        .unwrap();
    assert_eq!(r.instruments, p.instruments, "the receipt keeps them");
    let list = m.instruments();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].pack, "piano");
    assert_eq!(list[0].instrument, piano());
    assert_eq!(list[0].path, m.pack_dir("piano").join("Piano.sfz"));
    assert!(list[0].path.is_file());
    assert_eq!(list[0].key(), "pack:piano/grand");
    // Still offered when a later manifest no longer names the pack.
    let again = Manager::with_transport(
        root.path(),
        manifest(vec![]),
        Arc::new(Net::new()),
        Patience::none(),
    );
    assert_eq!(again.instruments().len(), 1);
    // And gone with the pack.
    m.remove("piano").unwrap();
    assert!(m.instruments().is_empty());
}

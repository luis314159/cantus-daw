//! ADR 0042: content under CC0 1.0 and CC BY 3.0 / 4.0 is accepted; a CC BY pack carries
//! its credit line (author, title, licence, link) and its licence file; everything else —
//! NC, ND, SA, retired or unknown licences — fails the manifest. The receipt carries the
//! credit, old receipts keep loading, and the credits list names every pack.

mod support;

use cantus_content::credits::{self, Courtesy};
use cantus_content::fetch::Net;
use cantus_content::install::{self, Patience};
use cantus_content::manifest::{Licence, Manifest, ManifestError, FORMAT};
use cantus_content::{Manager, Pack};
use std::sync::Arc;
use std::time::Duration;
use support::*;

/// The fixture manifest with CC BY packs (`tests/fixtures/manifest-cc-by.json`).
const FIXTURE: &str = include_str!("fixtures/manifest-cc-by.json");

fn base(dir: &std::path::Path) -> Pack {
    files_pack(
        "piano",
        dir,
        &[("C4.wav", wav(3, 64)), ("LICENSE", b"CC BY".to_vec())],
    )
}

fn check(p: Pack) -> Result<(), ManifestError> {
    manifest(vec![p]).validate()
}

#[test]
fn exactly_cc0_and_cc_by_3_and_4_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(check(base(dir.path())), Ok(()), "CC0 1.0");
    assert_eq!(check(cc_by(base(dir.path()), "3.0")), Ok(()), "CC BY 3.0");
    assert_eq!(check(cc_by(base(dir.path()), "4.0")), Ok(()), "CC BY 4.0");
    for refused in [
        "CC-BY-NC-4.0",
        "CC-BY-ND-4.0",
        "CC-BY-SA-4.0",
        "CC-BY-NC-SA-3.0",
        "CC-BY-NC-ND-4.0",
        "CC-BY-2.0",
        "CC-BY-2.5",
        "LicenseRef-Sampling-Plus-1.0",
        "CC-PDDC",
        "GPL-3.0-only",
        "cc-by-4.0",
        "CC BY 4.0",
        "",
    ] {
        let mut p = cc_by(base(dir.path()), "4.0");
        p.licence = refused.into();
        let e = check(p).expect_err(refused);
        assert!(
            e.to_string().contains("not accepted") || refused.is_empty(),
            "{refused}: {e}"
        );
    }
    assert_eq!(
        Licence::ACCEPTED,
        ["CC0-1.0", "CC-BY-3.0", "CC-BY-4.0"],
        "the list ADR 0042 §1 names"
    );
    for id in Licence::ACCEPTED {
        let l = Licence::from_spdx(id).expect(id);
        assert_eq!(l.spdx(), id);
        assert_eq!(l.needs_credit(), id != "CC0-1.0", "{id}");
    }
    assert_eq!(Licence::from_spdx("CC-BY-4.0").unwrap().name(), "CC BY 4.0");
    assert_eq!(Licence::from_spdx("CC0-1.0").unwrap().name(), "CC0 1.0");
}

#[test]
fn a_cc_by_pack_needs_its_credit_line_and_its_licence_file() {
    let dir = tempfile::tempdir().unwrap();
    let good = cc_by(base(dir.path()), "3.0");
    let reject = |edit: &dyn Fn(&mut Pack), why: &str| {
        let mut p = good.clone();
        edit(&mut p);
        assert!(check(p).is_err(), "{why}");
    };
    reject(&|p| p.attribution = None, "no credit line at all");
    reject(
        &|p| p.attribution.as_mut().unwrap().author = " ".into(),
        "no author",
    );
    reject(
        &|p| p.attribution.as_mut().unwrap().title.clear(),
        "no title",
    );
    reject(
        &|p| p.attribution.as_mut().unwrap().url = "http://example.org".into(),
        "a link that is not https",
    );
    reject(&|p| p.attribution.as_mut().unwrap().url.clear(), "no link");
    reject(&|p| p.licence_file.clear(), "no licence file");
    reject(
        &|p| p.licence_file = "NOTICE".into(),
        "a licence file the pack does not carry",
    );
    let e = check({
        let mut p = good.clone();
        p.attribution = None;
        p
    })
    .unwrap_err();
    assert!(
        e.to_string().contains("credit"),
        "says what is missing: {e}"
    );
    // A licensor's own wording is taken as it is, but it cannot be blank.
    reject(
        &|p| p.attribution.as_mut().unwrap().line = "   ".into(),
        "a blank licensor's line",
    );
}

#[test]
fn the_licence_link_is_the_text_of_the_licence_named() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = cc_by(base(dir.path()), "4.0");
    p.licence_url = "https://creativecommons.org/licenses/by/3.0/legalcode".into();
    assert!(check(p).is_err(), "CC BY 4.0 with the 3.0 text");
    let mut p = base(dir.path());
    p.licence_url = "https://creativecommons.org/licenses/by/4.0/".into();
    assert!(check(p).is_err(), "CC0 with a CC BY text");
    let mut p = cc_by(base(dir.path()), "4.0");
    p.licence_url = "https://creativecommons.org/licenses/by-nc/4.0/".into();
    assert!(check(p).is_err(), "an NC text under a CC BY id");
    // The deed, the legal code and their translations; nothing else under the path.
    for ok in [
        "https://creativecommons.org/licenses/by/3.0/",
        "https://creativecommons.org/licenses/by/3.0",
        "https://creativecommons.org/licenses/by/3.0/legalcode",
        "https://creativecommons.org/licenses/by/3.0/legalcode/",
        "https://creativecommons.org/licenses/by/3.0/legalcode.es",
        "https://creativecommons.org/licenses/by/3.0/deed.es",
        "https://creativecommons.org/licenses/by/3.0/deed.pt_BR",
        "https://creativecommons.org/licenses/by/3.0/deed.zh-hans",
    ] {
        let mut p = cc_by(base(dir.path()), "3.0");
        p.licence_url = ok.into();
        assert_eq!(check(p), Ok(()), "{ok}");
    }
}

#[test]
fn the_credit_line_is_the_licensors_own_or_title_author_link_and_licence() {
    let dir = tempfile::tempdir().unwrap();
    let p = cc_by(base(dir.path()), "4.0");
    let line = p.credit_line().expect("a CC BY pack has a credit line");
    for part in [
        "Upright piano",
        "Ana Example",
        "https://example.org/piano",
        "CC BY 4.0",
        "https://creativecommons.org/licenses/by/4.0/legalcode",
    ] {
        assert!(line.contains(part), "{part:?} not in {line}");
    }
    let mut asked = p.clone();
    asked.attribution.as_mut().unwrap().line = "Piano by Ana Example, CC BY 4.0".into();
    assert_eq!(
        asked.credit_line().as_deref(),
        Some("Piano by Ana Example, CC BY 4.0"),
        "exactly as the licensor asks"
    );
    assert_eq!(base(dir.path()).credit_line(), None, "CC0 asks for none");
    assert!(p.needs_credit() && !base(dir.path()).needs_credit());
}

#[test]
fn a_manifest_with_credit_lines_is_one_older_builds_refuse_by_name() {
    // Format 1 builds would read a CC BY pack's credit as an unknown field; format 2
    // says so plainly, and a format-1 manifest may not carry one.
    const { assert!(FORMAT >= 2) };
    let dir = tempfile::tempdir().unwrap();
    let mut m = manifest(vec![cc_by(base(dir.path()), "4.0")]);
    m.format = 1;
    let e = m.validate().unwrap_err();
    assert!(e.to_string().contains("format 2"), "{e}");
    // A format-1 manifest of CC0 packs (the bundled one) stays valid.
    let mut m = manifest(vec![base(dir.path())]);
    m.format = 1;
    assert_eq!(m.validate(), Ok(()));
}

#[test]
fn the_fixture_manifest_with_cc_by_packs_parses() {
    let m = Manifest::parse(FIXTURE).expect("the fixture is valid");
    let by: Vec<&Pack> = m.packs.iter().filter(|p| p.needs_credit()).collect();
    assert_eq!(by.len(), 2);
    let piano = m.pack("fixture-grand-piano").unwrap();
    assert!(piano.credit_line().unwrap().contains("Alex Example"));
    assert!(piano
        .attribution
        .as_ref()
        .unwrap()
        .note
        .contains("without crediting me"));
    let choir = m.pack("fixture-choir").unwrap();
    assert_eq!(
        choir.credit_line().as_deref(),
        Some("Fixture Choir by The Example Singers (example.org/choir), CC BY 4.0")
    );
    // The same packs with a credit line taken out fail as a whole.
    let broken = FIXTURE.replace("\"author\": \"Alex Example\",", "\"author\": \"\",");
    assert!(Manifest::parse(&broken).is_err());
}

#[test]
fn the_receipt_carries_the_credit_and_old_receipts_still_load() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let p = cc_by(base(src.path()), "3.0");
    let line = p.credit_line().unwrap();
    let mut m = Manager::with_transport(
        root.path(),
        manifest(vec![p.clone()]),
        Arc::new(Net::new()),
        Patience::none(),
    );
    m.install("piano");
    let done = m.wait_idle(Duration::from_secs(30));
    let receipt = done[0].result.clone().expect("installed");
    assert_eq!(receipt.attribution, p.attribution);
    assert_eq!(receipt.credit_line().as_deref(), Some(line.as_str()));
    // On disk, word for word, and read back.
    let json = std::fs::read_to_string(m.pack_dir("piano").join(install::RECEIPT)).unwrap();
    assert!(json.contains("Ana Example"), "{json}");
    let back = install::installed(&install::Layout::new(root.path()));
    assert_eq!(back[0].credit_line().as_deref(), Some(line.as_str()));

    // A receipt written before round 26 (no credit line) is still an installed pack.
    let old = root.path().join("old-kit");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(
        old.join(install::RECEIPT),
        r#"{"id":"old-kit","version":1,"name":{"en":"Old Kit","es":"Kit viejo"},
            "licence":"CC0-1.0",
            "licence_url":"https://creativecommons.org/publicdomain/zero/1.0/legalcode",
            "licence_file":"LICENSE","credits":[{"name":"Someone","url":"https://example.org",
            "licence_url":"https://example.org/LICENSE"}],"installed_at":0,"files":3,"bytes":9}"#,
    )
    .unwrap();
    let all = install::installed(&install::Layout::new(root.path()));
    let old = all.iter().find(|r| r.id == "old-kit").expect("old receipt");
    assert_eq!((old.attribution.clone(), old.credit_line()), (None, None));
    // And a CC0 receipt does not grow an empty credit field on disk.
    let r = serde_json::to_string(old).unwrap();
    assert!(!r.contains("attribution"), "{r}");
}

#[test]
fn the_credits_list_names_every_pack_cc_by_in_full_and_cc0_as_a_courtesy() {
    let m = Manifest::parse(FIXTURE).unwrap();
    let list = credits::list(&m, &[]);
    assert_eq!(list.len(), 3, "every pack of the manifest");
    let piano = list.iter().find(|c| c.id == "fixture-grand-piano").unwrap();
    assert_eq!(piano.courtesy, Courtesy::Required);
    assert_eq!(
        piano.line,
        m.pack(&piano.id).unwrap().credit_line().unwrap()
    );
    assert_eq!(piano.licence, "CC BY 3.0");
    let kit = list.iter().find(|c| c.id == "fixture-cc0-kit").unwrap();
    assert_eq!(kit.courtesy, Courtesy::Courtesy);
    assert!(kit.line.contains("Example Studios"), "{}", kit.line);
    assert!(
        kit.attribution.is_some(),
        "a CC0 pack's courtesy credit is kept, never required"
    );
    // An installed pack the manifest no longer lists is still credited, from its receipt.
    let src = tempfile::tempdir().unwrap();
    let gone = cc_by(base(src.path()), "4.0");
    let receipt = cantus_content::Receipt {
        id: "gone".into(),
        version: 1,
        name: gone.name.clone(),
        licence: gone.licence.clone(),
        licence_url: gone.licence_url.clone(),
        licence_file: gone.licence_file.clone(),
        credits: gone.credits.clone(),
        attribution: gone.attribution.clone(),
        instruments: vec![],
        installed_at: 0,
        files: 1,
        bytes: 1,
    };
    let list = credits::list(&m, std::slice::from_ref(&receipt));
    assert_eq!(list.len(), 4);
    let g = list.iter().find(|c| c.id == "gone").unwrap();
    assert_eq!(g.courtesy, Courtesy::Required);
    assert_eq!(Some(g.line.clone()), receipt.credit_line());
    // A receipt of a pack the manifest lists adds nothing twice.
    let mut same = receipt.clone();
    same.id = "fixture-choir".into();
    assert_eq!(credits::list(&m, &[same]).len(), 3);
}

#[test]
fn a_licence_url_that_only_starts_like_the_licence_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        "https://creativecommons.org/licenses/by/4.0/../../by-nc/4.0/legalcode",
        "https://creativecommons.org/licenses/by/4.0/legalcode?by-nc",
        "https://creativecommons.org/licenses/by/4.0/legalcode#nc",
        "https://creativecommons.org/licenses/by/4.0/%2e%2e/by-sa/4.0/",
        "https://creativecommons.org/licenses/by/4.0/anything",
        "https://creativecommons.org/licenses/by/4.0/us/legalcode",
        "https://creativecommons.org/licenses/by/4.0legalcode",
        "https://creativecommons.org/licenses/by/4.0/deed.",
        "https://creativecommons.org/licenses/by/4.0/legalcode/x",
    ] {
        let mut p = cc_by(base(dir.path()), "4.0");
        p.licence_url = bad.into();
        assert!(check(p).is_err(), "{bad}");
    }
}

#[test]
fn a_cc0_pack_may_carry_a_courtesy_credit_that_is_never_required() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = cc_by(base(dir.path()), "4.0");
    p.licence = "CC0-1.0".into();
    p.licence_url = "https://creativecommons.org/publicdomain/zero/1.0/legalcode".into();
    assert_eq!(
        check(p.clone()),
        Ok(()),
        "CC0 with the credit its author asked for"
    );
    assert!(!p.needs_credit());
    let list = credits::list(&manifest(vec![p.clone()]), &[]);
    assert_eq!(
        list[0].courtesy,
        Courtesy::Courtesy,
        "decided by the licence"
    );
    assert!(list[0].line.contains("Ana Example"), "{}", list[0].line);
    // CC0 is a dedication to the public domain, not a licence: worded as one.
    let line = p.credit_line().unwrap();
    assert!(
        line.contains("dedicated to the public domain under CC0 1.0")
            && !line.contains("licensed under"),
        "{line}"
    );
    // And from a receipt the same way; a CC BY receipt without a line is still required.
    let receipt = |licence: &str, url: &str, a: Option<cantus_content::Attribution>| {
        cantus_content::Receipt {
            id: "gone".into(),
            version: 1,
            name: p.name.clone(),
            licence: licence.into(),
            licence_url: url.into(),
            licence_file: "LICENSE".into(),
            credits: p.credits.clone(),
            attribution: a,
            instruments: vec![],
            installed_at: 0,
            files: 1,
            bytes: 1,
        }
    };
    let empty = manifest(vec![]);
    let cc0 = receipt(
        "CC0-1.0",
        "https://creativecommons.org/publicdomain/zero/1.0/legalcode",
        p.attribution.clone(),
    );
    assert_eq!(
        credits::list(&empty, &[cc0])[0].courtesy,
        Courtesy::Courtesy
    );
    let by = receipt(
        "CC-BY-4.0",
        "https://creativecommons.org/licenses/by/4.0/legalcode",
        None,
    );
    assert_eq!(credits::list(&empty, &[by])[0].courtesy, Courtesy::Required);
}

#[test]
fn the_licensors_own_line_is_given_with_the_licence_and_its_link() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = cc_by(base(dir.path()), "4.0");
    p.attribution.as_mut().unwrap().line = "Piano by Ana Example".into();
    let full = p.credit_full().unwrap();
    assert!(full.starts_with("Piano by Ana Example"), "{full}");
    assert!(
        full.contains("CC BY 4.0") && full.contains(&p.licence_url),
        "CC BY 4.0 §3(a)(1)(C): a link to the licence: {full}"
    );
    // A line that already links the licence is not doubled; a composed line has it.
    let mut linked = p.clone();
    linked.attribution.as_mut().unwrap().line = format!("Piano by Ana Example, {}", p.licence_url);
    assert_eq!(linked.credit_full(), linked.credit_line());
    let composed = cc_by(base(dir.path()), "4.0");
    assert_eq!(composed.credit_full(), composed.credit_line());
    // The credits list and the receipt give the full credit.
    let list = credits::list(&manifest(vec![p.clone()]), &[]);
    assert_eq!(list[0].line, full);
}

#[test]
fn a_receipt_with_fields_a_newer_cantus_wrote_still_loads() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("newer");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(install::RECEIPT),
        r#"{"id":"newer","version":2,"name":{"en":"Newer","es":"Más nuevo","fr":"Plus récent"},
            "licence":"CC-BY-4.0",
            "licence_url":"https://creativecommons.org/licenses/by/4.0/legalcode",
            "licence_file":"LICENSE","credits":[{"name":"Someone","url":"https://example.org",
            "licence_url":"https://example.org/LICENSE","role":"recording"}],
            "attribution":{"title":"T","author":"A","url":"https://example.org",
            "modified":"trimmed"},
            "installed_at":0,"files":3,"bytes":9,"verified_by":"cantus 2"}"#,
    )
    .unwrap();
    let all = install::installed(&install::Layout::new(root.path()));
    assert_eq!(
        all.len(),
        1,
        "an unknown field never makes a pack look uninstalled"
    );
    assert!(all[0].credit_line().unwrap().contains("by A"));
}

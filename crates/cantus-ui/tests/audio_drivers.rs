//! Getting a low-latency driver (round 20): GitHub's answer for FlexASIO's latest release
//! is read (a recorded response), a downloaded installer is verified (a proxy's HTML page
//! is refused, so is anything that is not a Windows program of the size GitHub reports),
//! and the makers' driver pages come with the detected interface's maker first.

use cantus_ui::audio_setup::drivers::{
    looks_like_html, makers_for, parse_release, verify_installer, FetchError, ASIO4ALL_URL,
    FLEXASIO_PAGE, MAKERS,
};

/// GitHub's answer for `repos/dechamps/FlexASIO/releases/latest`, as recorded on
/// 2026-10-06 (trimmed to the fields that matter; the shape is GitHub's).
const RECORDED: &str = r#"{
  "url": "https://api.github.com/repos/dechamps/FlexASIO/releases/123456",
  "html_url": "https://github.com/dechamps/FlexASIO/releases/tag/flexasio-1.10b",
  "id": 123456,
  "tag_name": "flexasio-1.10b",
  "name": "FlexASIO 1.10b",
  "draft": false,
  "prerelease": false,
  "assets": [
    {
      "name": "FlexASIO-1.10b.exe",
      "content_type": "application/x-msdownload",
      "size": 1458230,
      "browser_download_url": "https://github.com/dechamps/FlexASIO/releases/download/flexasio-1.10b/FlexASIO-1.10b.exe"
    },
    {
      "name": "FlexASIO-1.10b.exe.sigstore",
      "content_type": "application/octet-stream",
      "size": 8123,
      "browser_download_url": "https://github.com/dechamps/FlexASIO/releases/download/flexasio-1.10b/FlexASIO-1.10b.exe.sigstore"
    }
  ],
  "body": "Release notes."
}"#;

#[test]
fn githubs_answer_names_the_installer_its_size_and_its_signature() {
    let r = parse_release(RECORDED.as_bytes()).unwrap();
    assert_eq!(r.tag, "flexasio-1.10b");
    assert_eq!(r.asset, "FlexASIO-1.10b.exe");
    assert_eq!(r.size, 1_458_230);
    assert!(r.url.ends_with("/flexasio-1.10b/FlexASIO-1.10b.exe"));
    assert!(r.sigstore.as_deref().unwrap().ends_with(".exe.sigstore"));
    // A proxy's page instead of JSON, a release without an installer, junk.
    let page = b"<!DOCTYPE html><html><body>This file is being scanned</body></html>";
    assert_eq!(parse_release(page), Err(FetchError::Quarantined));
    assert!(matches!(
        parse_release(
            br#"{"tag_name":"x","assets":[{"name":"a.zip","size":1,"browser_download_url":"u"}]}"#
        ),
        Err(FetchError::Parse(_))
    ));
    assert!(matches!(
        parse_release(b"not json"),
        Err(FetchError::Parse(_))
    ));
}

#[test]
fn a_download_is_an_installer_of_the_size_github_reports_or_it_is_refused() {
    let mut exe = b"MZ\x90\x00".to_vec();
    exe.resize(4096, 0);
    assert_eq!(verify_installer(&exe, 4096), Ok(()));
    assert_eq!(
        verify_installer(&exe, 5000),
        Err(FetchError::Size {
            got: 4096,
            want: 5000
        })
    );
    let quarantine =
        b"\r\n  <html><head><title>Zscaler</title></head><body>quarantined</body></html>";
    assert_eq!(
        verify_installer(quarantine, quarantine.len() as u64),
        Err(FetchError::Quarantined)
    );
    assert!(looks_like_html(b"\xef\xbb\xbf<!doctype html>"));
    assert_eq!(
        verify_installer(b"PK\x03\x04zip", 7),
        Err(FetchError::NotInstaller)
    );
    assert!(!looks_like_html(&exe));
    // Every refusal has words, in both languages.
    for loc in cantus_ui::i18n::Locale::ALL {
        cantus_ui::i18n::default_thread_locale(loc);
        for e in [
            FetchError::Quarantined,
            FetchError::NotInstaller,
            FetchError::Size { got: 1, want: 2 },
            FetchError::Network("x".into()),
        ] {
            assert!(!e.text().starts_with("devices."), "{loc:?}: {}", e.text());
        }
    }
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn the_makers_pages_list_the_detected_maker_first() {
    let (all, none) = makers_for(&["Realtek(R) Audio"]);
    assert_eq!(none, None);
    assert_eq!(all.len(), MAKERS.len());
    let names: Vec<&str> = all.iter().map(|m| m.name).collect();
    for want in [
        "Behringer",
        "Focusrite",
        "PreSonus",
        "Steinberg",
        "MOTU",
        "Universal Audio",
        "Audient",
        "M-Audio",
        "Native Instruments",
        "RME",
        "Arturia",
        "Roland",
        "Yamaha",
    ] {
        assert!(names.iter().any(|n| n.contains(want)), "{want} missing");
    }
    for (devices, maker) in [
        (vec!["UMC204HD 192k"], "Behringer (UMC)"),
        (vec!["Realtek(R) Audio", "Focusrite USB Audio"], "Focusrite"),
        (vec!["Scarlett 2i2 USB"], "Focusrite"),
        (vec!["Babyface Pro"], "RME"),
        (vec!["MiniFuse 2"], "Arturia"),
        (vec!["Steinberg UR22C"], "Steinberg"),
    ] {
        let (list, detected) = makers_for(&devices);
        assert_eq!(detected, Some(maker), "{devices:?}");
        assert_eq!(list[0].name, maker);
        assert_eq!(list.len(), MAKERS.len(), "every maker still listed once");
    }
    for m in MAKERS {
        assert!(m.url.starts_with("https://"), "{}", m.name);
    }
    assert!(ASIO4ALL_URL.starts_with("https://asio4all.org"));
    assert!(FLEXASIO_PAGE.starts_with("https://github.com/dechamps/FlexASIO"));
}

//! What `packaging/macos` declares (ADR 0066), checked on every system: the bundle's
//! identity and minimum macOS, the microphone's reason in both languages, `.cantus` as a
//! document type Cantus owns, and the hardened runtime's exceptions plug-ins need.

use std::path::PathBuf;

fn read(rel: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    // Lines trimmed and joined: indentation and line endings (CRLF in a Windows checkout)
    // do not matter to a property list.
    text.lines().map(str::trim).collect()
}

/// The value after `<key>name</key>` (the next element's text).
fn after_key<'a>(plist: &'a str, name: &str) -> &'a str {
    let at = plist
        .find(&format!("<key>{name}</key>"))
        .unwrap_or_else(|| panic!("no {name}"));
    &plist[at..]
}

#[test]
fn the_info_plist_names_the_app_its_system_and_its_documents() {
    let plist = read("packaging/macos/Info.plist");
    assert!(
        after_key(&plist, "CFBundleIdentifier").contains("<string>com.cantusdaw.Cantus</string>")
    );
    assert!(after_key(&plist, "LSMinimumSystemVersion")
        .starts_with("<key>LSMinimumSystemVersion</key><string>11.0</string>"));
    assert!(after_key(&plist, "CFBundleExecutable").contains("<string>cantus</string>"));
    assert!(plist.contains("<key>NSMicrophoneUsageDescription</key>"));
    assert!(plist.contains("<key>NSHighResolutionCapable</key><true/>"));
    // `.cantus`: Cantus's own type, which Finder and the Dock open with it.
    let docs = after_key(&plist, "CFBundleDocumentTypes");
    assert!(docs.contains("<string>com.cantusdaw.project</string>"));
    assert!(docs.contains("<key>LSHandlerRank</key><string>Owner</string>"));
    let uti = after_key(&plist, "UTExportedTypeDeclarations");
    assert!(uti.contains("<key>public.filename-extension</key><array><string>cantus</string>"));
    for lang in ["en", "es"] {
        let strings = read(&format!("packaging/macos/{lang}.lproj/InfoPlist.strings"));
        assert!(
            strings.contains("NSMicrophoneUsageDescription = \""),
            "{lang}"
        );
        assert!(strings.contains("\"Cantus project\" = \""), "{lang}");
    }
}

#[test]
fn the_entitlements_let_plug_ins_load_and_run() {
    let e = read("packaging/macos/Cantus.entitlements");
    for key in [
        "com.apple.security.device.audio-input",
        "com.apple.security.cs.disable-library-validation",
        "com.apple.security.cs.allow-jit",
        "com.apple.security.cs.allow-unsigned-executable-memory",
    ] {
        assert!(e.contains(&format!("<key>{key}</key><true/>")), "{key}");
    }
    assert!(!e.contains("<key>com.apple.security.cs.allow-dyld-environment-variables</key>"));
}

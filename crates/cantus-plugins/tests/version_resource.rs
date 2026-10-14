//! Each plug-in library (`Cantus <Name>.clap` / `.vst3`) carries a Windows version
//! resource — product name "Cantus", the workspace's version, the plug-in's name as its
//! description — which code signing checks (ADR 0048 §9) and Explorer shows. `build.rs`
//! links the resource into every linked output of this package (the `cdylib` and the test
//! executables alike), so this test's own executable carries the same one.

#[cfg(windows)]
fn version_string(bytes: &[u8], key: &str) -> Option<String> {
    // As `xtask/src/package_meta.rs` reads it: the key in UTF-16 after `wType == 1`,
    // padded to 32 bits from the structure's start, then the value.
    let mut needle: Vec<u8> = key.encode_utf16().flat_map(u16::to_le_bytes).collect();
    needle.extend_from_slice(&[0, 0]);
    let u16_at = |i: usize| Some(u16::from_le_bytes([*bytes.get(i)?, *bytes.get(i + 1)?]));
    let mut from = 0;
    while let Some(off) = bytes[from..]
        .windows(needle.len())
        .position(|w| w == needle)
    {
        let at = from + off;
        from = at + 2;
        if at < 6 || !at.is_multiple_of(2) || u16_at(at - 2) != Some(1) {
            continue;
        }
        let start = at - 6;
        let mut v = at + needle.len();
        v += (4 - (v - start) % 4) % 4;
        let mut units = Vec::new();
        while let Some(u) = u16_at(v) {
            if u == 0 {
                break;
            }
            units.push(u);
            v += 2;
        }
        return String::from_utf16(&units).ok();
    }
    None
}

#[cfg(windows)]
#[test]
fn every_linked_output_names_its_product_and_version() {
    let me = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    assert_eq!(
        version_string(&me, "ProductName").as_deref(),
        Some("Cantus")
    );
    assert_eq!(
        version_string(&me, "ProductVersion").as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    // The test build has every plug-in compiled in; a bundled library has one, and its
    // description is that plug-in's full name.
    let d = version_string(&me, "FileDescription").unwrap_or_default();
    assert!(d.starts_with("Cantus"), "{d}");
}

/// `build.rs` names each plug-in from its cargo feature; the names are `desc.rs`'s.
#[test]
fn the_build_script_knows_every_plug_in_by_name() {
    let script = include_str!("../build.rs");
    for d in &cantus_plugins::desc::ALL {
        assert!(
            script.contains(&format!(", \"{}\")", d.full_name)),
            "build.rs does not name {}",
            d.full_name
        );
    }
    assert_eq!(
        script.matches(", \"Cantus ").count(),
        cantus_plugins::desc::ALL.len(),
        "build.rs names a plug-in desc.rs does not have"
    );
}

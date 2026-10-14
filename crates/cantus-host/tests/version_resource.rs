//! `cantus-scan.exe` carries a Windows version resource — product name "Cantus", the
//! workspace's version, its own description — which code signing checks (ADR 0048 §9) and
//! Explorer shows. `build.rs` links the resource into every executable of this package, so
//! this test's own executable carries the same one: reading it proves the build script.
#![cfg(windows)]

/// A `StringFileInfo` value: the key in UTF-16 after `wType == 1`, padded to 32 bits from
/// the structure's start, then the value (`xtask/src/package_meta.rs` reads it the same way).
fn version_string(bytes: &[u8], key: &str) -> Option<String> {
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

#[test]
fn the_scanner_names_its_product_and_version() {
    let me = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    assert_eq!(
        version_string(&me, "ProductName").as_deref(),
        Some("Cantus")
    );
    assert_eq!(
        version_string(&me, "ProductVersion").as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        version_string(&me, "FileDescription").as_deref(),
        Some("Cantus plug-in scanner")
    );
}

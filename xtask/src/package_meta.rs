//! Every file a release signs names its product and version (ADR 0048 §9): SignPath's
//! rule for signed binaries ("set all product name attributes to your project's name, all
//! product version attributes to the same value in each build"), and what Explorer's
//! *Details* tab shows. The programs and the launcher get their resource from
//! `cantus-app/icon.rs`, the scanner from `cantus-host/build.rs`, each plug-in library from
//! `cantus-plugins/build.rs`, the installer from `VIAddVersionKey` in `cantus.nsi`.
//!
//! The check reads the `StringFileInfo` entries of a Windows version resource straight
//! from the file's bytes: each is a `String` structure — `wLength`, `wValueLength`,
//! `wType` (1 = text), the key in UTF-16 with its NUL, padding to 32 bits, the value in
//! UTF-16 with its NUL.

use std::path::{Path, PathBuf};

/// The product name every signed file carries.
pub const PRODUCT: &str = "Cantus";

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

/// The value of the version-resource string `key` (`ProductName`, `ProductVersion`, …),
/// if the file has one.
pub fn version_string(bytes: &[u8], key: &str) -> Option<String> {
    let mut needle = utf16(key);
    needle.extend_from_slice(&[0, 0]);
    let mut from = 0;
    while let Some(off) = bytes[from..]
        .windows(needle.len())
        .position(|w| w == needle.as_slice())
    {
        let at = from + off;
        from = at + 2;
        // A String structure: the key starts 6 bytes in, after wType == 1 (text), and the
        // structure is 32-bit aligned.
        if at < 6 || !at.is_multiple_of(2) || u16_at(bytes, at - 2) != Some(1) {
            continue;
        }
        let start = at - 6;
        let value_len = u16_at(bytes, start + 2)? as usize; // in UTF-16 units, NUL included
        let mut v = at + needle.len();
        v += (4 - (v - start) % 4) % 4;
        let mut units = Vec::new();
        while let Some(u) = u16_at(bytes, v) {
            if u == 0 || units.len() >= value_len.max(1) {
                break;
            }
            units.push(u);
            v += 2;
        }
        return String::from_utf16(&units).ok();
    }
    None
}

/// What is wrong with the version resources of `files`, one line each; empty when every
/// file names [`PRODUCT`] and `version`.
pub fn problems(files: &[PathBuf], version: &str) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        let bytes = match std::fs::read(f) {
            Ok(b) => b,
            Err(e) => {
                out.push(format!("{}: {e}", f.display()));
                continue;
            }
        };
        for (key, want) in [("ProductName", PRODUCT), ("ProductVersion", version)] {
            match version_string(&bytes, key) {
                Some(v) if v == want => {}
                Some(v) => out.push(format!("{}: {key} is {v:?}, not {want:?}", name(f))),
                None => out.push(format!("{}: no {key}", name(f))),
            }
        }
    }
    out
}

fn name(f: &Path) -> String {
    f.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Stop the release when a file it signs lacks its product name or version.
pub fn require(files: &[PathBuf], version: &str) {
    let found = problems(files, version);
    assert!(
        found.is_empty(),
        "files the release signs without their version resource (ADR 0048 §9):\n  {}",
        found.join("\n  ")
    );
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A `String` structure as the resource compiler writes it, 32-bit aligned.
    pub fn entry(key: &str, value: &str) -> Vec<u8> {
        let mut key_bytes = utf16(key);
        key_bytes.extend_from_slice(&[0, 0]);
        let mut val = utf16(value);
        val.extend_from_slice(&[0, 0]);
        let mut b = vec![0u8; 6];
        b.extend_from_slice(&key_bytes);
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        b.extend_from_slice(&val);
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        let len = b.len() as u16;
        b[0..2].copy_from_slice(&len.to_le_bytes());
        b[2..4].copy_from_slice(&((value.encode_utf16().count() + 1) as u16).to_le_bytes());
        b[4..6].copy_from_slice(&1u16.to_le_bytes());
        b
    }

    /// Bytes standing in for a signed file with its version resource.
    pub fn with_resource(version: &str) -> Vec<u8> {
        let mut b = b"MZ\x90\0 a program".to_vec();
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        // Text that merely mentions the key, not a resource entry, must not be taken.
        b.extend_from_slice(&utf16("ProductName"));
        b.extend_from_slice(&[0, 0, 0, 0]);
        for (k, v) in [
            ("CompanyName", "Cantus"),
            ("FileDescription", "Cantus plug-in scanner"),
            ("ProductName", PRODUCT),
            ("ProductVersion", version),
        ] {
            b.extend(entry(k, v));
        }
        b
    }

    #[test]
    fn the_strings_are_read_from_a_version_resource() {
        let b = with_resource("0.1.0-beta.1");
        assert_eq!(version_string(&b, "ProductName").as_deref(), Some("Cantus"));
        assert_eq!(
            version_string(&b, "ProductVersion").as_deref(),
            Some("0.1.0-beta.1")
        );
        assert_eq!(
            version_string(&b, "FileDescription").as_deref(),
            Some("Cantus plug-in scanner")
        );
        assert_eq!(version_string(&b, "LegalTrademarks"), None);
        assert_eq!(version_string(b"MZ nothing here", "ProductName"), None);
    }

    #[test]
    fn a_file_without_its_name_or_version_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.exe");
        let bare = dir.path().join("bare.clap");
        let old = dir.path().join("old.exe");
        std::fs::write(&good, with_resource("1.2.3")).unwrap();
        std::fs::write(&bare, b"MZ no resource").unwrap();
        std::fs::write(&old, with_resource("1.2.2")).unwrap();
        assert!(problems(std::slice::from_ref(&good), "1.2.3").is_empty());
        let found = problems(&[good, bare, old], "1.2.3");
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.iter().any(|p| p == "bare.clap: no ProductName"));
        assert!(found.iter().any(|p| p == "bare.clap: no ProductVersion"));
        assert!(found
            .iter()
            .any(|p| p == "old.exe: ProductVersion is \"1.2.2\", not \"1.2.3\""));
    }
}

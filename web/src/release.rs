//! The latest release's facts — version, date, and each file's size, SHA-256 and link — read
//! at BUILD time from `assets/release.json` (`tools/release_json.py`, run by the site
//! workflow; `build.rs` turns it into constants). The visitor's browser asks no one.

/// One file of the latest release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelFile {
    pub name: &'static str,
    /// Bytes.
    pub size: u64,
    /// Lower-case hex; empty when GitHub gave none.
    pub sha256: &'static str,
    pub url: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/release.rs"));

/// The release's versioned file whose name ends in `suffix` (`windows-x64-setup.exe`…).
pub fn file(suffix: &str) -> Option<RelFile> {
    REL_FILES
        .iter()
        .copied()
        .find(|f| f.name.ends_with(suffix) && f.name.contains(REL_VERSION))
        .or_else(|| REL_FILES.iter().copied().find(|f| f.name.ends_with(suffix)))
}

/// A size as people read it: `159 MB` (decimal megabytes, as file managers show them).
pub fn megabytes(bytes: u64) -> String {
    format!("{} MB", (bytes as f64 / 1_000_000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_in_megabytes() {
        assert_eq!(megabytes(166_648_623), "167 MB");
        assert_eq!(megabytes(0), "0 MB");
    }

    #[test]
    fn the_committed_facts_name_a_windows_installer_with_its_hash() {
        let w = file("windows-x64-setup.exe").expect("a Windows installer");
        assert!(w.size > 0);
        assert_eq!(w.sha256.len(), 64);
        assert!(!REL_DATE.is_empty() && !REL_VERSION.is_empty());
    }
}

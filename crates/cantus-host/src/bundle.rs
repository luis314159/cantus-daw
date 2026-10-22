//! macOS plug-in bundles (ADR 0066): a `.clap` or `.vst3` on macOS is a folder whose
//! binary is `Contents/MacOS/<CFBundleExecutable>` — usually the bundle's own name, without
//! an extension. Pure path work, so it is tested on every system.

use std::path::{Path, PathBuf};

/// The executable a bundle names in `Contents/Info.plist` (an XML property list; a binary
/// one is not read and the fallbacks apply).
fn executable_in_plist(bundle: &Path) -> Option<String> {
    let text = std::fs::read_to_string(bundle.join("Contents").join("Info.plist")).ok()?;
    let at = text.find("<key>CFBundleExecutable</key>")?;
    let rest = &text[at + "<key>CFBundleExecutable</key>".len()..];
    let open = rest.find("<string>")? + "<string>".len();
    let close = rest[open..].find("</string>")?;
    let name = rest[open..open + close].trim();
    (!name.is_empty() && !name.contains(['/', '\\'])).then(|| name.to_string())
}

/// The binary inside a macOS bundle folder: the executable its `Info.plist` names, else a
/// file named as the bundle (without its extension), else the only file in
/// `Contents/MacOS`. `None` if none of those exists.
pub fn binary(bundle: &Path) -> Option<PathBuf> {
    let dir = bundle.join("Contents").join("MacOS");
    if let Some(name) = executable_in_plist(bundle) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(stem) = bundle.file_stem() {
        let p = dir.join(stem);
        if p.is_file() {
            return Some(p);
        }
    }
    let files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    match files.as_slice() {
        [only] => Some(only.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(dir: &Path, name: &str, files: &[&str], plist: Option<&str>) -> PathBuf {
        let b = dir.join(name);
        let macos = b.join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        for f in files {
            std::fs::write(macos.join(f), b"\xcf\xfa\xed\xfe").unwrap();
        }
        if let Some(exe) = plist {
            let text = format!(
                "<?xml version=\"1.0\"?>\n<plist version=\"1.0\"><dict>\n\
                 <key>CFBundleName</key><string>X</string>\n\
                 <key>CFBundleExecutable</key>\n\t<string>{exe}</string>\n</dict></plist>\n"
            );
            std::fs::write(b.join("Contents").join("Info.plist"), text).unwrap();
        }
        b
    }

    #[test]
    fn the_plist_names_the_binary_then_the_bundle_name_then_the_only_file() {
        let dir = tempfile::tempdir().unwrap();
        let named = bundle(dir.path(), "A.clap", &["A", "Other"], Some("Other"));
        assert_eq!(binary(&named), Some(named.join("Contents/MacOS/Other")));
        let by_stem = bundle(dir.path(), "B.vst3", &["B", "helper"], None);
        assert_eq!(binary(&by_stem), Some(by_stem.join("Contents/MacOS/B")));
        let only = bundle(dir.path(), "C.vst3", &["Renamed"], None);
        assert_eq!(binary(&only), Some(only.join("Contents/MacOS/Renamed")));
        let stale = bundle(dir.path(), "D.clap", &["D"], Some("Gone"));
        assert_eq!(binary(&stale), Some(stale.join("Contents/MacOS/D")));
        let none = bundle(dir.path(), "E.clap", &["x", "y"], None);
        assert_eq!(binary(&none), None);
        assert_eq!(binary(&dir.path().join("Missing.clap")), None);
    }
}

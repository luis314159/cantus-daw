//! Zips, written with forward-slash names under one top folder, files in a stable order,
//! so the same inputs make the same archive listing.

use std::io::Write;
use std::path::{Path, PathBuf};

/// One entry: its name inside the archive and the file it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub src: PathBuf,
}

/// Every file under `dir`, named relative to it with `/`, sorted.
pub fn walk(dir: &Path) -> Vec<Entry> {
    fn go(root: &Path, dir: &Path, out: &mut Vec<Entry>) {
        let mut items: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        items.sort();
        for p in items {
            if p.is_dir() {
                go(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).expect("under the root");
                let name = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push(Entry { name, src: p });
            }
        }
    }
    let mut out = Vec::new();
    go(dir, dir, &mut out);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Write `entries` into a new zip at `out`, each under `top/` when `top` is not empty.
pub fn zip(out: &Path, top: &str, entries: &[Entry]) {
    zip_modes(out, top, entries, &|_| None);
}

/// [`zip`], with each entry's Unix mode as `mode` says from its name inside the archive (before
/// `top/`), for the systems that read it (round 42: a Mac bundle's `Contents/MacOS/` program and
/// a Linux plug-in's library 0755); `None` leaves the zip crate's default.
pub fn zip_modes(out: &Path, top: &str, entries: &[Entry], mode: &dyn Fn(&str) -> Option<u32>) {
    use zip::write::SimpleFileOptions;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let file =
        std::fs::File::create(out).unwrap_or_else(|e| panic!("create {}: {e}", out.display()));
    let mut z = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9))
        .large_file(false);
    for e in entries {
        let name = if top.is_empty() {
            e.name.clone()
        } else {
            format!("{top}/{}", e.name)
        };
        let bytes =
            std::fs::read(&e.src).unwrap_or_else(|err| panic!("read {}: {err}", e.src.display()));
        // `None`: the zip crate's own default, so the archives made before stay as they were.
        let o = match mode(&e.name) {
            // Unix, wherever it is written: on Windows the zip crate writes DOS attributes,
            // which carry no mode, and an unzip on a Mac or on Linux would drop the bit.
            Some(m) => opts.unix_permissions(m).system(zip::System::Unix),
            None => opts,
        };
        z.start_file(name, o).expect("zip entry");
        z.write_all(&bytes).expect("zip write");
    }
    z.finish().expect("zip finish");
}

/// The names inside a zip, sorted (for the tests).
#[cfg(test)]
pub fn names(path: &Path) -> Vec<String> {
    let f = std::fs::File::open(path).unwrap();
    let z = zip::ZipArchive::new(f).expect("a zip");
    let mut v: Vec<String> = z
        .file_names()
        .map(|n| n.expect("a name in UTF-8").into_owned())
        .collect();
    v.sort();
    v
}

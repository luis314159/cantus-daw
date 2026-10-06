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
        z.start_file(name, opts).expect("zip entry");
        z.write_all(&bytes).expect("zip write");
    }
    z.finish().expect("zip finish");
}

/// The names inside a zip, sorted (for the tests and the package's own check).
pub fn names(path: &Path) -> Vec<String> {
    let f = std::fs::File::open(path).unwrap();
    let z = zip::ZipArchive::new(f).expect("a zip");
    let mut v: Vec<String> = z.file_names().map(str::to_owned).collect();
    v.sort();
    v
}

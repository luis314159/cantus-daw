//! Zip one of Cantus's own sound packs and print its manifest entry (round 37, ADR 0101):
//! the archive's size and SHA-256, the size it installs to, and the `source` that points
//! at the release asset the integrator uploads (`luis314159/cantus-daw`, tag `content-r37`).
//!
//! `cargo run -p cantus-content --example pack_folder -- <folder> [<out.zip>]`
//!
//! Every file under the folder goes in, under the folder's name stripped: the pack installs
//! its files at the top of its own folder. Files are sorted, so the same folder makes the
//! same archive.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const RELEASE: &str = "https://github.com/luis314159/cantus-daw/releases/download/content-r37";

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("the folder")
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().expect("a pack folder"));
    let name = folder
        .file_name()
        .expect("a folder name")
        .to_string_lossy()
        .into_owned();
    let out = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| folder.with_extension("zip"));
    let mut files = Vec::new();
    files_under(&folder, &mut files);
    let file = std::fs::File::create(&out).expect("the archive");
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    let mut installed = 0u64;
    for f in &files {
        let rel = f
            .strip_prefix(&folder)
            .expect("under the folder")
            .to_string_lossy()
            .replace('\\', "/");
        let mut bytes = Vec::new();
        std::fs::File::open(f)
            .and_then(|mut r| r.read_to_end(&mut bytes))
            .expect("the file");
        installed += bytes.len() as u64;
        zip.start_file(format!("{name}/{rel}"), options)
            .expect("an entry");
        zip.write_all(&bytes).expect("written");
    }
    zip.finish().expect("the archive closes");
    let mut h = Sha256::new();
    let mut r = std::fs::File::open(&out).expect("the archive");
    let mut buf = vec![0u8; 1 << 20];
    let mut size = 0u64;
    loop {
        let n = r.read(&mut buf).expect("read");
        if n == 0 {
            break;
        }
        size += n as u64;
        h.update(&buf[..n]);
    }
    let sha: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let file_name = out.file_name().unwrap().to_string_lossy();
    println!(
        "{} files, {installed} bytes installed, archive {size} bytes",
        files.len()
    );
    println!(
        "\"installed_size\": {installed},\n\"source\": {{\"zip\": {{\"url\": \"{RELEASE}/{file_name}\", \"size\": {size}, \"sha256\": \"{sha}\", \"strip\": \"{name}/\"}}}}"
    );
}

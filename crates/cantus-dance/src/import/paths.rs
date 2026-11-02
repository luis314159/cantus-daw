//! Nothing is read outside a file's own folder (ADR 0089 §5).
//!
//! A path a character file names (an atlas, an image) and a `file:` path a project names
//! are both *relative*, plain names joined by `/` or `\`: no `..`, no `.` games, no drive,
//! no root, no scheme (`http:`, `data:` is the caller's to decode), no NUL. What is left
//! is joined to the folder and, when it exists, canonicalised by the system and checked to
//! be still inside the folder — a link pointing out of it is refused.

use super::ImportError;
use std::path::{Component, Path, PathBuf};

/// Why a named path was refused, in words.
fn refused(what: &str, p: &str) -> ImportError {
    ImportError::Invalid(format!("the path \"{p}\" {what}"))
}

/// The relative parts of `p`, or why not.
pub fn relative_parts(p: &str) -> Result<Vec<String>, ImportError> {
    if p.is_empty() {
        return Err(refused("is empty", p));
    }
    if p.contains('\0') {
        return Err(refused("holds a NUL", p));
    }
    if p.len() > 1024 {
        return Err(refused("is too long", p));
    }
    // A scheme (`http://`, `file:`, `C:`): a colon anywhere is refused — no name of ours
    // needs one, and on Windows it also means a drive or an alternate stream.
    if p.contains(':') {
        return Err(refused("names a drive, a stream or an address", p));
    }
    if p.starts_with('/') || p.starts_with('\\') {
        return Err(refused("starts at a root", p));
    }
    let mut parts = Vec::new();
    for part in p.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => return Err(refused("leaves its folder", p)),
            s => parts.push(s.to_string()),
        }
    }
    if parts.is_empty() {
        return Err(refused("names no file", p));
    }
    Ok(parts)
}

/// `p` resolved inside `folder`; refused when it would leave it.
pub fn resolve(folder: &Path, p: &str) -> Result<PathBuf, ImportError> {
    let parts = relative_parts(p)?;
    let mut out = folder.to_path_buf();
    for part in &parts {
        out.push(part);
    }
    // Every component must be a plain name (a Windows prefix or root could hide in one).
    let tail = out
        .strip_prefix(folder)
        .map_err(|_| refused("leaves its folder", p))?;
    if tail
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(refused("leaves its folder", p));
    }
    // Links: what exists must still be inside the folder once the system has resolved it.
    if let (Ok(real), Ok(base)) = (out.canonicalize(), folder.canonicalize()) {
        if !real.starts_with(&base) {
            return Err(refused("points out of its folder", p));
        }
    }
    Ok(out)
}

/// Read a file named by a character file, bounded in bytes.
pub fn read_inside(folder: &Path, p: &str, max: usize) -> Result<Vec<u8>, ImportError> {
    let path = resolve(folder, p)?;
    read_bounded(&path, max)
}

/// Read `path`, refusing it before reading when it is larger than `max`.
pub fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, ImportError> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(ImportError::Invalid(format!(
            "\"{}\" is not a file",
            path.display()
        )));
    }
    if meta.len() > max as u64 {
        return Err(ImportError::TooLarge(format!(
            "{} is {} bytes, at most {max}",
            path.display(),
            meta.len()
        )));
    }
    use std::io::Read;
    let mut out = Vec::with_capacity(meta.len() as usize);
    std::fs::File::open(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut out)?;
    if out.len() > max {
        return Err(ImportError::TooLarge(format!(
            "{} grew past {max} bytes",
            path.display()
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_relative_names_resolve_inside() {
        let d = tempfile::tempdir().unwrap();
        let p = resolve(d.path(), "images/a.png").unwrap();
        assert!(p.starts_with(d.path()));
        assert_eq!(
            relative_parts("./sub\\x.png").unwrap(),
            vec!["sub".to_string(), "x.png".to_string()]
        );
    }

    #[test]
    fn escapes_are_refused() {
        let d = tempfile::tempdir().unwrap();
        for bad in [
            "../x.png",
            "a/../../x.png",
            "/etc/passwd",
            "\\\\server\\share\\x",
            "C:\\Windows\\win.ini",
            "C:x.png",
            "http://example.com/x.png",
            "file:///x.png",
            "data:image/png;base64,AAAA",
            "a\0b",
            "",
            ".",
            "x.png:stream",
        ] {
            assert!(resolve(d.path(), bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_folder_is_refused() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"no").unwrap();
        let d = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), d.path().join("s.png"))
            .unwrap();
        assert!(resolve(d.path(), "s.png").is_err());
    }

    #[test]
    fn reading_is_bounded_before_it_starts() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("big.bin"), vec![0u8; 2000]).unwrap();
        assert!(matches!(
            read_inside(d.path(), "big.bin", 1999),
            Err(ImportError::TooLarge(_))
        ));
        assert_eq!(read_inside(d.path(), "big.bin", 2000).unwrap().len(), 2000);
    }
}

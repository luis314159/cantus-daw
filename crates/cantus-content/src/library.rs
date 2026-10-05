//! What the sample browser shows: folders and audio files under its roots (installed
//! packs and the user's own folders), listed one folder at a time, and an index of every
//! file built on a worker thread for search.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// The files the browser lists and Cantus can decode.
pub const AUDIO_EXTENSIONS: [&str; 8] = ["wav", "wave", "aif", "aiff", "aifc", "flac", "ogg", "oga"];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// One row of a folder listing.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub dir: bool,
}

/// Compare names the way people count: `Kick 2` before `Kick 10`, case-insensitively.
pub fn natural(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(c) = x.peek().copied().filter(|c| c.is_ascii_digit()) {
                    n.push(c);
                    x.next();
                }
                let mut m = String::new();
                while let Some(d) = y.peek().copied().filter(|d| d.is_ascii_digit()) {
                    m.push(d);
                    y.next();
                }
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let o = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// A folder's sub-folders and audio files, folders first, in natural order. Hidden
/// entries (a leading dot) and the pack receipt are left out.
pub fn list(dir: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Entry> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let path = e.path();
            let dir = e.file_type().ok()?.is_dir() || path.is_dir();
            (dir || is_audio(&path)).then_some(Entry { name, path, dir })
        })
        .collect();
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| natural(&a.name, &b.name)));
    out
}

/// One file in the index.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Which root it is under (an index into the roots the index was built from).
    pub root: usize,
    pub path: PathBuf,
    /// The path below its root, lowercased with `/`: what a query matches.
    key: String,
}

impl Hit {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Index {
    pub hits: Vec<Hit>,
    /// The walk stopped at the cap: the index is partial.
    pub truncated: bool,
}

/// At most this many files are indexed (a whole drive added as a folder would not fit
/// a browser anyway).
pub const INDEX_CAP: usize = 200_000;

impl Index {
    /// Walk `roots` (blocking).
    pub fn build(roots: &[PathBuf]) -> Index {
        let mut idx = Index::default();
        for (ri, root) in roots.iter().enumerate() {
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                for e in list(&dir) {
                    if e.dir {
                        stack.push(e.path);
                        continue;
                    }
                    if idx.hits.len() >= INDEX_CAP {
                        idx.truncated = true;
                        return idx;
                    }
                    let rel = e.path.strip_prefix(root).unwrap_or(&e.path);
                    let key = rel.to_string_lossy().replace('\\', "/").to_lowercase();
                    idx.hits.push(Hit {
                        root: ri,
                        path: e.path,
                        key,
                    });
                }
            }
        }
        idx.hits.sort_by(|a, b| a.root.cmp(&b.root).then_with(|| natural(&a.key, &b.key)));
        idx
    }

    /// Files whose path below the root contains every word of `query` (case-insensitive),
    /// at most `max`.
    pub fn search(&self, query: &str, max: usize) -> Vec<&Hit> {
        let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.hits
            .iter()
            .filter(|h| words.iter().all(|w| h.key.contains(w.as_str())))
            .take(max)
            .collect()
    }
}

/// An index being built on a worker thread.
pub struct Indexer {
    rx: Receiver<Index>,
}

impl Indexer {
    pub fn spawn(roots: Vec<PathBuf>) -> Indexer {
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new()
            .name("cantus-sample-index".into())
            .spawn(move || {
                let _ = tx.send(Index::build(&roots));
            });
        Indexer { rx }
    }

    /// The index, once it is ready.
    pub fn poll(&self) -> Option<Index> {
        self.rx.try_recv().ok()
    }
}

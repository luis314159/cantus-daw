//! A small, strict reader of tar archives (ustar, GNU and pax), for `.tar.gz` packs
//! (round 26: the Salamander Grand Piano is published only so).
//!
//! It reads entries in order and hands each one's name, kind and data to the caller; it
//! never creates anything itself, so it cannot follow a link or write outside a folder.
//! What it understands: the ustar `prefix` + `name`, GNU long names (`L`) and pax headers
//! (`x`, the `path` and `size` records); octal and base-256 sizes. Every header's checksum
//! is verified, and an archive that ends before its data does is an error.

use std::io::Read;

const BLOCK: usize = 512;

/// What an entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
    /// A hard or symbolic link, a device, a FIFO — anything that is not a file's bytes.
    Other,
}

/// One entry's header, its data still to be read from the reader.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The entry's name as the archive gives it (`/` separators, possibly hostile).
    pub name: String,
    pub kind: Kind,
    pub size: u64,
}

fn bad(why: impl Into<String>) -> String {
    why.into()
}

fn field(h: &[u8]) -> &[u8] {
    let end = h.iter().position(|&b| b == 0).unwrap_or(h.len());
    &h[..end]
}

/// An octal number (spaces and NULs around it), or GNU's base-256 when the top bit is set.
fn number(h: &[u8]) -> Result<u64, String> {
    if h.first().is_some_and(|b| b & 0x80 != 0) {
        let mut v: u64 = (h[0] & 0x7f) as u64;
        for &b in &h[1..] {
            v = v
                .checked_mul(256)
                .and_then(|v| v.checked_add(b as u64))
                .ok_or_else(|| bad("a size too large to read"))?;
        }
        return Ok(v);
    }
    let s = std::str::from_utf8(field(h)).map_err(|_| bad("a number that is not text"))?;
    let s = s.trim_matches(|c: char| c == ' ' || c == '\0');
    if s.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(s, 8).map_err(|_| bad(format!("a number that is not octal: {s:?}")))
}

fn checksum_ok(h: &[u8; BLOCK]) -> Result<bool, String> {
    let want = number(&h[148..156])?;
    let sum: u64 = h
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            if (148..156).contains(&i) {
                32
            } else {
                b as u64
            }
        })
        .sum();
    Ok(sum == want)
}

/// Read exactly `buf.len()` bytes; `Ok(false)` at a clean end before the first byte.
fn read_block<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<bool, String> {
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..]) {
            Ok(0) if got == 0 => return Ok(false),
            Ok(0) => return Err(bad("the archive ends in the middle of a header")),
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(bad(e.to_string())),
        }
    }
    Ok(true)
}

/// Read an entry's data (`size` bytes and the padding after them) into `sink`, which
/// receives the data only.
fn pump<R: Read>(
    r: &mut R,
    size: u64,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut left = size;
    let mut buf = vec![0u8; 64 * 1024];
    while left > 0 {
        let want = left.min(buf.len() as u64) as usize;
        let n = match r.read(&mut buf[..want]) {
            Ok(0) => return Err(bad("the archive ends in the middle of an entry")),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(bad(e.to_string())),
        };
        sink(&buf[..n])?;
        left -= n as u64;
    }
    let pad = (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64;
    let mut skip = [0u8; BLOCK];
    if pad > 0 && !read_block(r, &mut skip[..pad as usize])? {
        return Err(bad("the archive ends in the middle of an entry"));
    }
    Ok(())
}

/// A whole small entry (a long name, a pax header), at most 1 MiB.
fn small<R: Read>(r: &mut R, size: u64) -> Result<Vec<u8>, String> {
    if size > 1 << 20 {
        return Err(bad("an extended header larger than 1 MiB"));
    }
    let mut v = Vec::with_capacity(size as usize);
    pump(r, size, &mut |b| {
        v.extend_from_slice(b);
        Ok(())
    })?;
    Ok(v)
}

/// What a pax header says about the entry after it.
#[derive(Default)]
struct Pax {
    path: Option<String>,
    size: Option<u64>,
    /// `GNU.sparse.*`: a sparse file, whose data is a map, not the file's bytes.
    sparse: bool,
}

/// The `path`, `size` and `GNU.sparse.*` records of a pax header (`"<len> <key>=<value>\n"`
/// each).
fn pax(data: &[u8]) -> Result<Pax, String> {
    let (mut path, mut size, mut sparse) = (None, None, false);
    let mut rest = data;
    while !rest.is_empty() {
        let sp = rest
            .iter()
            .position(|&b| b == b' ')
            .ok_or_else(|| bad("a damaged pax header"))?;
        let len: usize = std::str::from_utf8(&rest[..sp])
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&l| l > sp + 1 && l <= rest.len())
            .ok_or_else(|| bad("a damaged pax header"))?;
        let record = &rest[sp + 1..len];
        let record = record.strip_suffix(b"\n").unwrap_or(record);
        if let Some(eq) = record.iter().position(|&b| b == b'=') {
            let (k, v) = (&record[..eq], &record[eq + 1..]);
            match k {
                b"path" => {
                    path = Some(
                        String::from_utf8(v.to_vec())
                            .map_err(|_| bad("a pax path not in UTF-8"))?,
                    )
                }
                b"size" => {
                    size = std::str::from_utf8(v).ok().and_then(|s| s.parse().ok());
                    if size.is_none() {
                        return Err(bad("a damaged pax size"));
                    }
                }
                k if k.starts_with(b"GNU.sparse.") => sparse = true,
                _ => {}
            }
        }
        rest = &rest[len..];
    }
    Ok(Pax { path, size, sparse })
}

const SPARSE: &str = "a sparse file: packs hold plain files only";

/// Where an entry's data goes, a chunk at a time.
pub type Sink<'a> = dyn FnMut(&[u8]) -> Result<(), String> + 'a;
/// Reads an entry's data into a sink.
pub type ReadData<'a> = dyn FnMut(&mut Sink<'_>) -> Result<(), String> + 'a;
/// What [`walk`] calls for each entry.
pub type Each<'a> = dyn FnMut(&Entry, &mut ReadData<'_>) -> Result<(), String> + 'a;

/// Walk every entry of the tar stream `r`. For each file, folder or other entry, `each`
/// gets its header and a function that reads its data into a sink (call it at most once;
/// data not read is skipped). Returns at the end of the archive.
pub fn walk<R: Read>(r: &mut R, each: &mut Each<'_>) -> Result<(), String> {
    let mut h = [0u8; BLOCK];
    // A GNU long name and a pax header apply to the next real entry only (not to another
    // extended header in between).
    let mut long_name: Option<String> = None;
    let mut next = Pax::default();
    loop {
        if !read_block(r, &mut h)? {
            // No end-of-archive blocks: tolerated, as GNU tar does, but only between entries.
            return Ok(());
        }
        if h.iter().all(|&b| b == 0) {
            return Ok(());
        }
        if !checksum_ok(&h)? {
            return Err(bad(
                "a header's checksum does not match: the archive is damaged",
            ));
        }
        let own = number(&h[124..136])?;
        let flag = h[156];
        let size = own;
        match flag {
            b'L' => {
                let v = small(r, size)?;
                long_name = Some(
                    String::from_utf8(field(&v).to_vec())
                        .map_err(|_| bad("a long name not in UTF-8"))?,
                );
                continue;
            }
            b'x' => {
                next = pax(&small(r, size)?)?;
                continue;
            }
            b'S' => return Err(bad(SPARSE)),
            b'g' | b'K' => {
                // Global pax records and long link names change no file's name or bytes.
                small(r, size)?;
                continue;
            }
            _ => {}
        }
        let this = std::mem::take(&mut next);
        if this.sparse {
            return Err(bad(SPARSE));
        }
        let size = this.size.unwrap_or(own);
        let name = match (this.path, long_name.take()) {
            (Some(p), _) => p,
            (None, Some(l)) => l,
            (None, None) => {
                let name = String::from_utf8(field(&h[0..100]).to_vec())
                    .map_err(|_| bad("a name not in UTF-8"))?;
                // POSIX ustar (`ustar\0` `00`) keeps a name prefix at 345; GNU's
                // `ustar  \0` keeps access and change times there.
                let ustar = &h[257..263] == b"ustar\0";
                let prefix = if ustar {
                    String::from_utf8(field(&h[345..500]).to_vec())
                        .map_err(|_| bad("a name not in UTF-8"))?
                } else {
                    String::new()
                };
                if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix}/{name}")
                }
            }
        };
        let kind = match flag {
            // A V7 folder is a file entry whose name ends in `/`.
            b'0' | 0 | b'7' if name.ends_with('/') => Kind::Dir,
            b'0' | 0 | b'7' => Kind::File,
            // `D`: a GNU dump folder (its data lists the folder's names).
            b'5' | b'D' => Kind::Dir,
            _ => Kind::Other,
        };
        let entry = Entry { name, kind, size };
        let mut read = false;
        let mut pumped = |sink: &mut Sink<'_>| {
            read = true;
            pump(r, size, sink)
        };
        each(&entry, &mut pumped)?;
        if !read {
            pump(r, size, &mut |_| Ok(()))?;
        }
    }
}

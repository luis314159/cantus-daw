//! ISO base media file format (MP4, QuickTime MOV) boxes: writing them, and a small reader
//! the tests use to check what was written.
//!
//! A box is a 32-bit big-endian size (header included), a four-letter type and its body; a
//! "full" box starts its body with a version byte and 24 bits of flags.

/// A box: `name` with `body`.
pub fn bx(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(body.len() + 8);
    v.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    v.extend_from_slice(name);
    v.extend_from_slice(body);
    v
}

/// A full box: version and flags, then `body`.
pub fn full(name: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(body.len() + 4);
    b.push(version);
    b.extend_from_slice(&flags.to_be_bytes()[1..]);
    b.extend_from_slice(body);
    bx(name, &b)
}

/// 32-bit big-endian words.
pub fn u32s(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

/// The identity transformation matrix of `mvhd` and `tkhd` (16.16 and 2.30 fixed point).
pub const MATRIX: [u32; 9] = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000];

/// A chunk offset table: `stco` (32-bit) when every offset fits, else `co64`.
pub fn chunk_offsets(offsets: &[u64], wide: bool) -> Vec<u8> {
    let mut v = (offsets.len() as u32).to_be_bytes().to_vec();
    if wide {
        for o in offsets {
            v.extend_from_slice(&o.to_be_bytes());
        }
        full(b"co64", 0, 0, &v)
    } else {
        for o in offsets {
            v.extend_from_slice(&(*o as u32).to_be_bytes());
        }
        full(b"stco", 0, 0, &v)
    }
}

/// `stsc` from each chunk's sample count: one entry for each run of equal counts
/// (first chunk 1-based, samples a chunk, sample description 1).
pub fn sample_to_chunk(counts: &[u32]) -> Vec<u8> {
    let mut entries: Vec<[u32; 3]> = Vec::new();
    for (i, &c) in counts.iter().enumerate() {
        if entries.last().is_none_or(|e| e[1] != c) {
            entries.push([i as u32 + 1, c, 1]);
        }
    }
    let mut v = vec![entries.len() as u32];
    for e in entries {
        v.extend_from_slice(&e);
    }
    full(b"stsc", 0, 0, &u32s(&v))
}

/// `stsz` with a size for each sample (or one size for all: `uniform`).
pub fn sample_sizes(sizes: &[u32], uniform: Option<(u32, u32)>) -> Vec<u8> {
    match uniform {
        Some((size, count)) => full(b"stsz", 0, 0, &u32s(&[size, count])),
        None => {
            let mut v = vec![0, sizes.len() as u32];
            v.extend_from_slice(sizes);
            full(b"stsz", 0, 0, &u32s(&v))
        }
    }
}

/// One box read back: its type, where it starts in the buffer, and its body.
#[derive(Clone, Copy, Debug)]
pub struct Box<'a> {
    pub kind: [u8; 4],
    pub offset: usize,
    /// The header's length: 8, or 16 with a 64-bit size.
    pub header: usize,
    pub body: &'a [u8],
}

impl<'a> Box<'a> {
    /// The boxes inside this one (for a container box).
    pub fn children(&self) -> Vec<Box<'a>> {
        parse_at(self.body, self.offset + self.header)
    }

    /// The first child of type `kind`.
    pub fn child(&self, kind: &[u8; 4]) -> Option<Box<'a>> {
        self.children().into_iter().find(|b| &b.kind == kind)
    }

    /// The box at `path` below this one ("trak/mdia/minf").
    pub fn find(&self, path: &str) -> Option<Box<'a>> {
        let mut cur = *self;
        for name in path.split('/') {
            let k: [u8; 4] = name.as_bytes().try_into().ok()?;
            cur = cur.child(&k)?;
        }
        Some(cur)
    }

    /// The 32-bit word at `at` bytes into the body.
    pub fn u32_at(&self, at: usize) -> u32 {
        u32::from_be_bytes(self.body[at..at + 4].try_into().expect("four bytes"))
    }

    /// The 64-bit word at `at` bytes into the body.
    pub fn u64_at(&self, at: usize) -> u64 {
        u64::from_be_bytes(self.body[at..at + 8].try_into().expect("eight bytes"))
    }
}

/// The top-level boxes of a file (a 64-bit `largesize` read; a box running past the end is
/// cut at the end).
pub fn parse(data: &[u8]) -> Vec<Box<'_>> {
    parse_at(data, 0)
}

fn parse_at(data: &[u8], base: usize) -> Vec<Box<'_>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= data.len() {
        let size32 = u32::from_be_bytes(data[i..i + 4].try_into().expect("four bytes")) as u64;
        let kind: [u8; 4] = data[i + 4..i + 8].try_into().expect("four bytes");
        let (size, header) = match size32 {
            0 => ((data.len() - i) as u64, 8),
            1 if i + 16 <= data.len() => (
                u64::from_be_bytes(data[i + 8..i + 16].try_into().expect("eight bytes")),
                16,
            ),
            s => (s, 8),
        };
        if size < header as u64 {
            break;
        }
        let end = (i as u64 + size).min(data.len() as u64) as usize;
        out.push(Box {
            kind,
            offset: base + i,
            header,
            body: &data[i + header..end],
        });
        i = end;
    }
    out
}

/// The first top-level box of type `kind` in a file.
pub fn top<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<Box<'a>> {
    parse(data).into_iter().find(|b| &b.kind == kind)
}

/// A full box's table entries after its version/flags and entry count: `(count, entries)`.
pub fn table<'a>(b: &Box<'a>) -> (u32, &'a [u8]) {
    (b.u32_at(4), &b.body[8..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_written_read_back() {
        let inner = full(b"mvhd", 0, 3, &[9, 9]);
        let moov = bx(
            b"moov",
            &[inner.clone(), bx(b"trak", &bx(b"mdia", b"xy"))].concat(),
        );
        let file = [bx(b"ftyp", b"isom"), moov].concat();
        let tops = parse(&file);
        assert_eq!(tops.len(), 2);
        assert_eq!(&tops[0].kind, b"ftyp");
        assert_eq!(tops[0].body, b"isom");
        let moov = top(&file, b"moov").unwrap();
        assert_eq!(moov.offset, 12);
        let mvhd = moov.child(b"mvhd").unwrap();
        assert_eq!(mvhd.body, &[0, 0, 0, 3, 9, 9]);
        assert_eq!(moov.find("trak/mdia").unwrap().body, b"xy");
    }

    #[test]
    fn a_large_box_header_is_read() {
        let mut f = 1u32.to_be_bytes().to_vec();
        f.extend_from_slice(b"mdat");
        f.extend_from_slice(&20u64.to_be_bytes());
        f.extend_from_slice(&[7, 7, 7, 7]);
        let b = top(&f, b"mdat").unwrap();
        assert_eq!(b.body, &[7, 7, 7, 7]);
    }

    #[test]
    fn sample_to_chunk_runs() {
        let b = sample_to_chunk(&[2, 2, 3, 3, 3, 1]);
        let parsed = parse(&b);
        let (n, e) = table(&parsed[0]);
        assert_eq!(n, 3);
        assert_eq!(e, &u32s(&[1, 2, 1, 3, 3, 1, 6, 1, 1])[..]);
    }
}

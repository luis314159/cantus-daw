//! The words written into a file, in each format's own way: an ID3v2.3 tag for MP3,
//! Vorbis comments for FLAC and Ogg, a RIFF `LIST/INFO` chunk for WAV. Every file also
//! says what made it — the "made with Cantus" credit.

use crate::settings::Tags;

/// What every file says made it: `Cantus 0.1.0`.
pub fn encoder_credit() -> String {
    format!("Cantus {}", env!("CARGO_PKG_VERSION"))
}

/// The tags with the credit, as `(Vorbis key, value)` pairs, the empty ones left out.
pub fn vorbis_pairs(t: &Tags, encoder: &str) -> Vec<(&'static str, String)> {
    let mut v = Vec::new();
    for (k, s) in [
        ("TITLE", &t.title),
        ("ARTIST", &t.artist),
        ("ALBUM", &t.album),
        ("DATE", &t.year),
        ("COMMENT", &t.comment),
    ] {
        let s = s.trim();
        if !s.is_empty() {
            v.push((k, s.to_string()));
        }
    }
    v.push(("ENCODER", encoder.to_string()));
    v
}

/// A FLAC `VORBIS_COMMENT` block's body (little-endian lengths, UTF-8).
pub fn vorbis_comment_block(t: &Tags, encoder: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let vendor = encoder.as_bytes();
    out.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    out.extend_from_slice(vendor);
    let pairs = vorbis_pairs(t, encoder);
    out.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for (k, v) in pairs {
        let entry = format!("{k}={v}");
        out.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        out.extend_from_slice(entry.as_bytes());
    }
    out
}

/// A text in ID3v2.3's terms: ISO-8859-1 when every character fits, else UTF-16 with a
/// byte-order mark (v2.3 has no UTF-8, and v2.4 is still read badly by some players).
fn id3_text(s: &str) -> Vec<u8> {
    if s.chars().all(|c| (c as u32) < 0x100) {
        let mut v = vec![0u8];
        v.extend(s.chars().map(|c| c as u8));
        v
    } else {
        let mut v = vec![1u8, 0xFF, 0xFE];
        for u in s.encode_utf16() {
            v.extend_from_slice(&u.to_le_bytes());
        }
        v
    }
}

fn id3_frame(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(body);
}

/// A whole ID3v2.3 tag, to go before the first MP3 frame.
pub fn id3v2(t: &Tags, encoder: &str) -> Vec<u8> {
    let mut frames = Vec::new();
    for (id, s) in [
        (b"TIT2", &t.title),
        (b"TPE1", &t.artist),
        (b"TALB", &t.album),
        (b"TYER", &t.year),
    ] {
        let s = s.trim();
        if !s.is_empty() {
            id3_frame(&mut frames, id, &id3_text(s));
        }
    }
    let comment = t.comment.trim();
    if !comment.is_empty() {
        // COMM: encoding, language, an empty short description, the text.
        let text = id3_text(comment);
        let mut body = vec![text[0]];
        body.extend_from_slice(b"eng");
        if text[0] == 0 {
            body.push(0);
            body.extend_from_slice(&text[1..]);
        } else {
            // UTF-16: an empty description is a BOM and a two-byte terminator.
            body.extend_from_slice(&[0xFF, 0xFE, 0, 0]);
            body.extend_from_slice(&text[1..]);
        }
        id3_frame(&mut frames, b"COMM", &body);
    }
    id3_frame(&mut frames, b"TSSE", &id3_text(encoder));
    let mut out = Vec::with_capacity(10 + frames.len());
    out.extend_from_slice(b"ID3\x03\x00\x00");
    out.extend_from_slice(&syncsafe(frames.len() as u32));
    out.extend_from_slice(&frames);
    out
}

/// A 28-bit number in four bytes of seven bits (ID3's tag size).
fn syncsafe(n: u32) -> [u8; 4] {
    [
        ((n >> 21) & 0x7F) as u8,
        ((n >> 14) & 0x7F) as u8,
        ((n >> 7) & 0x7F) as u8,
        (n & 0x7F) as u8,
    ]
}

/// A RIFF `LIST` chunk of type `INFO` (title, artist, album, date, comment, software),
/// UTF-8, each string NUL-terminated and padded to an even length.
pub fn riff_info(t: &Tags, encoder: &str) -> Vec<u8> {
    let mut body = b"INFO".to_vec();
    for (id, s) in [
        (b"INAM", t.title.trim()),
        (b"IART", t.artist.trim()),
        (b"IPRD", t.album.trim()),
        (b"ICRD", t.year.trim()),
        (b"ICMT", t.comment.trim()),
        (b"ISFT", encoder),
    ] {
        if s.is_empty() {
            continue;
        }
        let mut data = s.as_bytes().to_vec();
        data.push(0);
        body.extend_from_slice(id);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(&data);
        if data.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut out = b"LIST".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags() -> Tags {
        Tags {
            title: "Canción".into(),
            artist: "Luis".into(),
            album: String::new(),
            year: "2026".into(),
            comment: "日本".into(),
        }
    }

    #[test]
    fn id3_size_is_syncsafe_and_counts_the_frames() {
        let t = id3v2(&tags(), "Cantus 0.1.0");
        let n = ((t[6] as u32) << 21) | ((t[7] as u32) << 14) | ((t[8] as u32) << 7) | t[9] as u32;
        assert_eq!(n as usize, t.len() - 10);
        assert!(t.windows(4).any(|w| w == b"TIT2"));
        assert!(
            !t.windows(4).any(|w| w == b"TALB"),
            "empty tags are left out"
        );
    }

    #[test]
    fn riff_info_chunks_are_even() {
        let c = riff_info(&tags(), "Cantus");
        assert_eq!(c.len() % 2, 0);
        let size = u32::from_le_bytes([c[4], c[5], c[6], c[7]]) as usize;
        assert_eq!(size + 8, c.len());
    }

    #[test]
    fn the_vorbis_block_lists_the_credit() {
        let b = vorbis_comment_block(&tags(), "Cantus 0.1.0");
        let n = u32::from_le_bytes([b[16], b[17], b[18], b[19]]);
        assert_eq!(n, 5, "title, artist, date, comment and the encoder");
    }
}

//! What an edit is written as (ADR 0096): a 32-bit float WAV, named by the SHA-256 of its
//! bytes, so the same edit of the same take is the same file and an edit never overwrites
//! another. Bytes only — the editor writes the file.

use super::AudioBuf;
use sha2::{Digest, Sha256};

/// `buf` as a WAV file (format 3, IEEE float, 32 bits), every byte determined by the audio.
pub fn encode(buf: &AudioBuf) -> Vec<u8> {
    let ch = buf.channels.max(1) as u32;
    let data_len = (buf.data.len() * 4) as u32;
    let mut b = Vec::with_capacity(44 + buf.data.len() * 4);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&(ch as u16).to_le_bytes());
    b.extend_from_slice(&buf.rate.to_le_bytes());
    b.extend_from_slice(&(buf.rate * ch * 4).to_le_bytes());
    b.extend_from_slice(&((ch * 4) as u16).to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for v in &buf.data {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// Read back what [`encode`] wrote (format 3, 32 bits).
pub fn parse(b: &[u8]) -> Option<AudioBuf> {
    if b.len() < 44 || &b[..4] != b"RIFF" || &b[8..16] != b"WAVEfmt " {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    if u16_at(20) != 3 || u16_at(34) != 32 || &b[36..40] != b"data" {
        return None;
    }
    let len = u32_at(40) as usize;
    let body = b.get(44..44 + len)?;
    let data = body
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    Some(AudioBuf::new(u32_at(24), u16_at(22) as u8, data))
}

/// The SHA-256 of `bytes`, lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// The file name an edit is kept under: the first 16 hex digits of its hash.
pub fn file_name(sha256_hex: &str) -> String {
    format!("edit-{}.wav", &sha256_hex[..16.min(sha256_hex.len())])
}

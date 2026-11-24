//! Reading the references: WAV (hound) and AIFF (the Iowa MIS files: big-endian PCM, the
//! rate an 80-bit extended float), mixed to mono; and Iowa's chromatic runs cut into notes.

use std::path::Path;

/// A mono signal and its rate.
pub struct Mono {
    pub x: Vec<f32>,
    pub sr: f64,
}

pub fn read(path: &Path) -> Option<Mono> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if ext == "wav" {
        read_wav(path)
    } else if ext == "aif" || ext == "aiff" {
        read_aiff(path)
    } else {
        None
    }
}

fn read_wav(path: &Path) -> Option<Mono> {
    let mut r = hound::WavReader::open(path).ok()?;
    let spec = r.spec();
    let ch = spec.channels as usize;
    let all: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / full)
                .collect()
        }
    };
    let x = all
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
    Some(Mono {
        x,
        sr: spec.sample_rate as f64,
    })
}

/// An IEEE 754 80-bit extended float (AIFF's sample rate).
fn ext80(b: &[u8]) -> f64 {
    let exp = (((b[0] as i32) & 0x7F) << 8) | b[1] as i32;
    let mut mant = 0u64;
    for v in &b[2..10] {
        mant = (mant << 8) | *v as u64;
    }
    if exp == 0 && mant == 0 {
        return 0.0;
    }
    let e = exp - 16383 - 63;
    let mut v = mant as f64;
    let mut k = e;
    while k > 0 {
        v *= 2.0;
        k -= 1;
    }
    while k < 0 {
        v *= 0.5;
        k += 1;
    }
    v
}

fn read_aiff(path: &Path) -> Option<Mono> {
    let d = std::fs::read(path).ok()?;
    if &d[0..4] != b"FORM" || (&d[8..12] != b"AIFF" && &d[8..12] != b"AIFC") {
        return None;
    }
    let (mut ch, mut bits, mut sr) = (0usize, 0usize, 0.0f64);
    let mut data: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let len = u32::from_be_bytes([d[at + 4], d[at + 5], d[at + 6], d[at + 7]]) as usize;
        let body = &d[at + 8..(at + 8 + len).min(d.len())];
        if id == b"COMM" {
            ch = u16::from_be_bytes([body[0], body[1]]) as usize;
            bits = u16::from_be_bytes([body[6], body[7]]) as usize;
            sr = ext80(&body[8..18]);
        } else if id == b"SSND" {
            let offset = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
            data = Some(&body[8 + offset..]);
        }
        at += 8 + len + (len & 1);
    }
    let data = data?;
    let bytes = bits.div_ceil(8);
    let full = (1i64 << (bits - 1)) as f32;
    let frame = bytes * ch;
    let x = data
        .chunks_exact(frame)
        .map(|f| {
            let mut s = 0.0f32;
            for c in 0..ch {
                let b = &f[c * bytes..(c + 1) * bytes];
                let mut v: i32 = (b[0] as i8) as i32;
                for byte in &b[1..] {
                    v = (v << 8) | *byte as i32;
                }
                s += v as f32 / full;
            }
            s / ch as f32
        })
        .collect();
    Some(Mono { x, sr })
}

/// The notes of a chromatic run (Iowa's files): stretches whose 10 ms level stays above
/// −45 dB under the file's loudest, at least 0.25 s long, split by at least 0.12 s under it.
pub fn segments(m: &Mono) -> Vec<(usize, usize)> {
    let hop = (0.01 * m.sr) as usize;
    let env: Vec<f32> =
        m.x.chunks(hop)
            .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt())
            .collect();
    let peak = env.iter().fold(0f32, |a, b| a.max(*b));
    let gate = peak * 0.0056; // −45 dB
    let (mut out, mut start, mut quiet) = (Vec::new(), None, 0usize);
    for (i, e) in env.iter().enumerate() {
        if *e > gate {
            if start.is_none() {
                start = Some(i);
            }
            quiet = 0;
        } else if let Some(s) = start {
            quiet += 1;
            if quiet >= 12 {
                let end = i - quiet;
                if end - s >= 25 {
                    out.push((s * hop, end * hop));
                }
                start = None;
            }
        }
    }
    if let Some(s) = start {
        if env.len() - s >= 25 {
            out.push((s * hop, m.x.len()));
        }
    }
    out
}

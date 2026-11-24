//! What every blind listening kit shares (Chorda's `chorda_abx`, Tympanum's `tympanum_abx`):
//! the clips of one item brought to one length and one loudness, the page filled, the
//! clips written. The page (`abx.html`) is one for every instrument: what it compares —
//! its texts, its items, its pairs, its trials — is the generator's subject file.

use std::fmt::Write as _;
use std::path::Path;

/// The page every kit carries, and the placeholders a generator fills.
pub const PAGE: &str = include_str!("abx.html");
pub const PLACEHOLDERS: [&str; 7] = [
    "__KIT__",
    "__PHRASES__",
    "__FILES__",
    "__TESTS__",
    "__SEED__",
    "__REVEAL__",
    "__SUBJECT__",
];

/// A JSON string literal.
pub fn js(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A subject file as one line of JSON (the page's `const SUBJECT = …;`, which the browser
/// check reads line by line); a subject that does not parse stops the kit.
pub fn subject_line(text: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(text).expect("the subject file is JSON");
    for k in [
        "id", "play", "trials", "secs", "pairs", "items", "credits", "text",
    ] {
        assert!(v.get(k).is_some(), "the subject names its {k}");
    }
    v.to_string()
}

/// The page with every placeholder filled (`fills` in `PLACEHOLDERS`' order).
pub fn fill_page(fills: [&str; 7]) -> String {
    let mut page = PAGE.to_string();
    for (p, v) in PLACEHOLDERS.iter().zip(fills) {
        page = page.replace(p, v);
    }
    assert!(
        !PLACEHOLDERS.iter().any(|p| page.contains(p)),
        "every placeholder of the page filled"
    );
    page
}

/// Integrated loudness, ITU-R BS.1770 (LUFS); −120 for silence.
pub fn lufs(l: &[f32], r: &[f32], sr: u32) -> f64 {
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(sr as f32);
    m.process(l, r);
    m.integrated().unwrap_or(-120.0)
}

pub fn db(x: f32) -> f64 {
    20.0 * cantus_dsp::math::log10(x.max(1e-9) as f64)
}

pub fn peak(l: &[f32], r: &[f32]) -> f32 {
    l.iter().chain(r).fold(0.0f32, |m, x| m.max(x.abs()))
}

fn gain(l: &mut [f32], r: &mut [f32], g_db: f64) {
    let g = cantus_dsp::math::powf(10.0f64, g_db / 20.0) as f32;
    for x in l.iter_mut().chain(r.iter_mut()) {
        *x *= g;
    }
}

/// How one item's clips were matched: the target, each clip's loudness and peak before, and
/// the gain each took.
pub struct Matched {
    pub target: f64,
    pub before: Vec<(f64, f64)>,
    pub gains: Vec<f64>,
}

/// One item's clips (stereo, the same rate) cut to the shortest, faded out over its last
/// `fade_s`, and brought to one loudness: `target` LUFS, or lower — together — if a clip's
/// peak would pass `ceiling` dBFS.
pub fn match_group(
    clips: &mut [(&mut Vec<f32>, &mut Vec<f32>)],
    sr: u32,
    fade_s: f64,
    target: f64,
    ceiling: f64,
) -> Matched {
    let n = clips.iter().map(|(l, _)| l.len()).min().unwrap_or(0);
    for (l, r) in clips.iter_mut() {
        l.truncate(n);
        r.truncate(n);
        let f = (fade_s * sr as f64) as usize;
        for i in 0..f.min(n) {
            let g = i as f32 / f as f32;
            l[n - 1 - i] *= g;
            r[n - 1 - i] *= g;
        }
    }
    let before: Vec<(f64, f64)> = clips
        .iter()
        .map(|(l, r)| (lufs(l, r, sr), db(peak(l, r))))
        .collect();
    // One target for all: lowered together if any would pass the ceiling.
    let target = before
        .iter()
        .map(|(l, p)| target.min(l + (ceiling - p)))
        .fold(target, f64::min);
    let mut gains = Vec::new();
    for ((l, r), (lu, _)) in clips.iter_mut().zip(&before) {
        let g = target - lu;
        gain(l, r, g);
        // Once more: BS.1770's absolute gate lets a quiet tail in or out as the level
        // moves, so the first gain lands within about half a LU; the second on it.
        let again = target - lufs(l, r, sr);
        gain(l, r, again);
        gains.push(g + again);
    }
    // The second gain may lift the peak that set the target past the ceiling (the first
    // draw of Chorda's kit 5 left middle C at −0.81 dBFS): all come down together.
    let over = clips
        .iter()
        .map(|(l, r)| db(peak(l, r)))
        .fold(f64::MIN, f64::max)
        - ceiling;
    if over > 0.0 {
        for ((l, r), g) in clips.iter_mut().zip(gains.iter_mut()) {
            gain(l, r, -(over + 0.005));
            *g -= over + 0.005;
        }
    }
    Matched {
        target,
        before,
        gains,
    }
}

/// 24-bit FLAC (`cantus-export`'s encoder, the export dialog's).
pub fn write_flac(path: &Path, l: &[f32], r: &[f32], sr: u32) -> Result<(), String> {
    use cantus_export::settings::{Depth, Dither, Tags};
    let samples = l.iter().zip(r).flat_map(|(a, b)| [*a, *b]).collect();
    let audio = cantus_export::Audio {
        rate: sr,
        channels: 2,
        samples,
    };
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| e.to_string())?);
    let mut progress = |_: f32| true;
    cantus_export::encode::flac(
        &mut out,
        &audio,
        Depth::Int24,
        Dither::Off,
        &Tags::default(),
        "",
        &mut progress,
    )
    .map_err(|e| format!("{e:?}"))?;
    use std::io::Write as _;
    out.flush().map_err(|e| e.to_string())
}

/// 24-bit PCM WAV (the published kit's: claude.ai serves .wav, not .flac).
pub fn write_wav(path: &Path, l: &[f32], r: &[f32], sr: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sr,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    let q = |x: f32| (x.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
    for (a, b) in l.iter().zip(r) {
        w.write_sample(q(*a)).map_err(|e| e.to_string())?;
        w.write_sample(q(*b)).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_has_every_placeholder() {
        for p in PLACEHOLDERS {
            assert!(PAGE.contains(p), "{p}");
        }
    }

    #[test]
    fn json_strings_escape() {
        assert_eq!(js("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn a_group_is_cut_faded_and_matched() {
        // Two noises 12 dB apart and of different lengths: one length, one loudness.
        let sr = 48_000;
        let mut x = 1u32;
        let mut noise = |n: usize, a: f32| -> Vec<f32> {
            (0..n)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    a * (x as f32 / u32::MAX as f32 - 0.5)
                })
                .collect()
        };
        let (mut l1, mut r1) = (noise(96_000, 0.4), noise(96_000, 0.4));
        let (mut l2, mut r2) = (noise(90_000, 0.1), noise(90_000, 0.1));
        let m = {
            let mut g = [(&mut l1, &mut r1), (&mut l2, &mut r2)];
            match_group(&mut g, sr, 0.03, -20.0, -1.0)
        };
        assert_eq!(l1.len(), 90_000);
        assert_eq!(l1[89_999], 0.0);
        assert!((m.before[0].0 - m.before[1].0 - 12.04).abs() < 0.2);
        for (l, r) in [(&l1, &r1), (&l2, &r2)] {
            assert!((lufs(l, r, sr) - m.target).abs() < 0.05);
            assert!(db(peak(l, r)) <= -1.0);
        }
    }
}

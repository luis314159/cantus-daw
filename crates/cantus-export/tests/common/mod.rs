//! What the export tests share: a test signal, the repository's decoders (and symphonia
//! for MP3), band energies, and tag readers.
#![allow(dead_code)]

use cantus_export::Audio;
use std::f64::consts::PI;
use std::path::Path;

/// Six seconds of something like music at `rate`: a chord with harmonics, a bass, a
/// noise-burst hat, panned; peaks near −6 dBFS.
pub fn music(rate: u32, seconds: f64) -> Audio {
    let n = (seconds * rate as f64) as usize;
    let sr = rate as f64;
    let mut st = 0x2545_f491_4f6c_dd1du64;
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f64 / sr;
        let mut l = 0.0;
        let mut r = 0.0;
        for (k, f0) in [261.63, 329.63, 392.0].iter().enumerate() {
            let mut s = 0.0;
            for h in 1..8 {
                s += (2.0 * PI * f0 * h as f64 * t).sin() / h as f64;
            }
            let pan = k as f64 / 2.0;
            l += s * 0.07 * (1.0 - 0.5 * pan);
            r += s * 0.07 * (0.5 + 0.5 * pan);
        }
        let bass = (2.0 * PI * 65.41 * t).sin() * 0.15;
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        let noise = (st >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
        let hat = noise * (-(t % 0.25) * 40.0).exp() * 0.08;
        out.push((l + bass + hat) as f32);
        out.push((r + bass + 0.8 * hat) as f32);
    }
    Audio {
        rate,
        channels: 2,
        samples: out,
    }
}

pub fn rms_db(x: &[f32]) -> f64 {
    let e: f64 = x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64;
    10.0 * e.max(1e-30).log10()
}

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let a = -2.0 * PI / len as f64;
        for s in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((a * k as f64).cos(), (a * k as f64).sin());
                let (ur, ui) = (re[s + k], im[s + k]);
                let (xr, xi) = (re[s + k + len / 2], im[s + k + len / 2]);
                let (vr, vi) = (xr * wr - xi * wi, xr * wi + xi * wr);
                re[s + k] = ur + vr;
                im[s + k] = ui + vi;
                re[s + k + len / 2] = ur - vr;
                im[s + k + len / 2] = ui - vi;
            }
        }
        len <<= 1;
    }
}

/// Energy in octave bands (63 Hz … 8 kHz centres) of the first channel, dB, averaged
/// over 16384-sample Hann frames: a band's power whatever the rate, the bins narrow
/// enough that a tone near a band's edge stays out of it.
pub fn octave_bands(x: &[f32], channels: usize, rate: u32) -> Vec<f64> {
    let n = 16_384;
    let centres = [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];
    let mut acc = vec![0.0f64; centres.len()];
    let frames = x.len() / channels;
    let mut at = 0;
    let mut count = 0usize;
    while at + n <= frames {
        count += 1;
        let mut re: Vec<f64> = (0..n)
            .map(|i| {
                let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos();
                x[(at + i) * channels] as f64 * w
            })
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        for (b, c) in centres.iter().enumerate() {
            let lo = (c / 2f64.sqrt() / rate as f64 * n as f64) as usize;
            let hi = (c * 2f64.sqrt() / rate as f64 * n as f64) as usize;
            acc[b] += (lo..hi.max(lo + 1))
                .map(|k| re[k] * re[k] + im[k] * im[k])
                .sum::<f64>();
        }
        at += n;
    }
    // Per frame, so files of different rates (and so frame counts) compare.
    acc.iter()
        .map(|e| 10.0 * (e / count.max(1) as f64).max(1e-30).log10())
        .collect()
}

/// An MP3 decoded by symphonia, gapless (the LAME tag's delay and padding honoured):
/// (rate, channels, interleaved samples).
pub fn decode_mp3(path: &Path) -> (u32, usize, Vec<f32>) {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let file = std::fs::File::open(path).unwrap();
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &opts, &MetadataOptions::default())
        .unwrap();
    let mut fmt = probed.format;
    let track = fmt.default_track().unwrap().clone();
    let rate = track.codec_params.sample_rate.unwrap();
    let mut dec = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .unwrap();
    let mut out = Vec::new();
    let mut ch = 0;
    while let Ok(p) = fmt.next_packet() {
        let Ok(buf) = dec.decode(&p) else { continue };
        ch = buf.spec().channels.count();
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, *buf.spec());
        sb.copy_interleaved_ref(buf);
        out.extend_from_slice(sb.samples());
    }
    (rate, ch, out)
}

/// The standard tags symphonia reads from a file (MP3's ID3v2, WAV's INFO), as
/// (key, value) with the key's debug name.
pub fn symphonia_tags(path: &Path, ext: &str) -> Vec<(String, String)> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let file = std::fs::File::open(path).unwrap();
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);
    let mut probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .unwrap();
    let mut tags = Vec::new();
    let mut take = |rev: &symphonia::core::meta::MetadataRevision| {
        for t in rev.tags() {
            if let Some(k) = t.std_key {
                tags.push((format!("{k:?}"), t.value.to_string()));
            }
        }
    };
    if let Some(m) = probed.metadata.get() {
        if let Some(rev) = m.current() {
            take(rev);
        }
    }
    if let Some(rev) = probed.format.metadata().current() {
        take(rev);
    }
    tags
}

/// An Ogg Vorbis file decoded by lewton: (rate, channels, interleaved, comments).
pub fn decode_ogg(path: &Path) -> (u32, usize, Vec<f32>, Vec<(String, String)>) {
    let f = std::fs::File::open(path).unwrap();
    let mut r = lewton::inside_ogg::OggStreamReader::new(f).unwrap();
    let rate = r.ident_hdr.audio_sample_rate;
    let ch = r.ident_hdr.audio_channels as usize;
    let comments = r.comment_hdr.comment_list.clone();
    let mut out = Vec::new();
    while let Some(p) = r.read_dec_packet_itl().unwrap() {
        out.extend(p.iter().map(|s| *s as f32 / 32768.0));
    }
    (rate, ch, out, comments)
}

/// A FLAC file decoded by claxon: (rate, channels, bits, interleaved integers, tags).
pub fn decode_flac(path: &Path) -> (u32, usize, u32, Vec<i32>, Vec<(String, String)>) {
    let mut r = claxon::FlacReader::open(path).unwrap();
    let info = r.streaminfo();
    let tags = r
        .tags()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let samples: Vec<i32> = r.samples().map(|s| s.unwrap()).collect();
    (
        info.sample_rate,
        info.channels as usize,
        info.bits_per_sample,
        samples,
        tags,
    )
}

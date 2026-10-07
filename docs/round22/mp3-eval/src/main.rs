//! Compare MP3 encoders: LAME (mp3lame-encoder), rusty_mp3, shine-rs.
use std::f64::consts::PI;
use std::time::Instant;

const SR: u32 = 44_100;

fn rng(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// 16 s of music-like material: a saw pad chord, a bass, kick/snare/hats, then a quiet
/// decaying-tone passage.
fn signal() -> Vec<f32> {
    let n = 16 * SR as usize;
    let mut out = vec![0f32; n * 2];
    let mut st = 0x1234_5678_9abc_def0u64;
    let sr = SR as f64;
    let chord = [220.0, 277.18, 329.63, 440.0];
    let mut hp = [0.0f64; 2];
    for i in 0..n {
        let t = i as f64 / sr;
        let mut l = 0.0;
        let mut r = 0.0;
        if t < 12.0 {
            for (k, f0) in chord.iter().enumerate() {
                let det = 1.0 + 0.002 * (2.0 * PI * 0.3 * t + k as f64).sin();
                let mut s = 0.0;
                let mut h = 1;
                while f0 * h as f64 * det < 16_000.0 {
                    s += (2.0 * PI * f0 * det * h as f64 * t).sin() / h as f64;
                    h += 1;
                }
                let pan = k as f64 / 3.0;
                l += s * 0.06 * (1.0 - pan * 0.6);
                r += s * 0.06 * (0.4 + pan * 0.6);
            }
            // bass
            let b = (2.0 * PI * 55.0 * t).sin() * 0.25;
            l += b;
            r += b;
            // drums every 0.5 s: kick on beats, snare on off-beats, hats every 0.125
            let beat = t % 0.5;
            if (t / 0.5) as usize % 2 == 0 {
                let f = 50.0 + 120.0 * (-beat * 30.0).exp();
                let k = (2.0 * PI * f * beat).sin() * (-beat * 9.0).exp() * 0.5;
                l += k;
                r += k;
            } else {
                let s = rng(&mut st) * (-beat * 18.0).exp() * 0.25;
                l += s;
                r += s;
            }
            let hb = t % 0.125;
            let noise = rng(&mut st);
            hp[0] = 0.5 * (noise - hp[1]);
            hp[1] = noise;
            let hat = hp[0] * (-hb * 60.0).exp() * 0.3;
            l += hat * 0.7;
            r += hat;
        } else {
            let u = t - 12.0;
            let note = u % 1.0;
            let f = [523.25, 659.25, 783.99, 1046.5][(u as usize) % 4];
            let s = ((2.0 * PI * f * note).sin() + 0.3 * (4.0 * PI * f * note).sin())
                * (-note * 4.0).exp()
                * 0.01;
            l += s;
            r += s * 0.9;
        }
        out[2 * i] = l as f32;
        out[2 * i + 1] = r as f32;
    }
    out
}

fn encode_lame(pcm: &[f32], kbps: u16, vbr: Option<u8>) -> Vec<u8> {
    use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, Quality, VbrMode};
    let mut b = Builder::new().unwrap();
    b.set_num_channels(2).unwrap();
    b.set_sample_rate(SR).unwrap();
    b.set_quality(Quality::Best).unwrap();
    match vbr {
        Some(q) => {
            b.set_vbr_mode(VbrMode::Mtrh).unwrap();
            let qq = match q {
                0 => Quality::Best,
                2 => Quality::NearBest,
                _ => Quality::Good,
            };
            b.set_vbr_quality(qq).unwrap();
        }
        None => {
            b.set_vbr_mode(VbrMode::Off).unwrap();
            let br = match kbps {
                128 => Bitrate::Kbps128,
                192 => Bitrate::Kbps192,
                _ => Bitrate::Kbps320,
            };
            b.set_brate(br).unwrap();
        }
    }
    let mut enc = b.build().unwrap();
    let mut out = Vec::new();
    for chunk in pcm.chunks(2 * 4096) {
        out.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len() / 2));
        enc.encode_to_vec(InterleavedPcm(chunk), &mut out).unwrap();
    }
    out.reserve(7200);
    enc.flush_to_vec::<FlushNoGap>(&mut out).unwrap();
    out
}

fn encode_lame_q(pcm: &[f32], _kbps: u16) -> Vec<u8> {
    use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, Quality, VbrMode};
    let mut b = Builder::new().unwrap();
    b.set_num_channels(2).unwrap();
    b.set_sample_rate(SR).unwrap();
    b.set_quality(Quality::NearBest).unwrap();
    b.set_vbr_mode(VbrMode::Off).unwrap();
    b.set_brate(Bitrate::Kbps320).unwrap();
    let mut enc = b.build().unwrap();
    let mut out = Vec::new();
    for chunk in pcm.chunks(2 * 4096) {
        out.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len() / 2));
        enc.encode_to_vec(InterleavedPcm(chunk), &mut out).unwrap();
    }
    out.reserve(7200);
    enc.flush_to_vec::<FlushNoGap>(&mut out).unwrap();
    out
}

fn encode_rusty(pcm: &[f32], kbps: u32, vbr: Option<f32>) -> Vec<u8> {
    let mut e = rusty_mp3::Mp3Encoder::new(rusty_mp3::Mp3EncoderConfig {
        bitrate_kbps: kbps,
        vbr_quality: vbr.map(rusty_mp3::vbr_quality_index),
    });
    let mut out = Vec::new();
    for chunk in pcm.chunks(2 * 4096) {
        e.push_pcm_f32(chunk, 2, SR).unwrap();
        while let Ok(p) = e.next_packet() {
            out.extend_from_slice(&p);
        }
    }
    e.finish();
    let mut tail = Vec::new();
    while let Ok(p) = e.next_packet() {
        tail.push(p);
    }
    // finish() prepends a Xing frame: it comes out first among the queue? keep order
    for p in tail {
        out.extend_from_slice(&p);
    }
    out
}

fn encode_shine(pcm: &[f32], kbps: u32) -> Vec<u8> {
    use shine_rs::mp3_encoder::{Mp3Encoder, Mp3EncoderConfig, StereoMode};
    let cfg = Mp3EncoderConfig::new()
        .sample_rate(SR)
        .bitrate(kbps)
        .channels(2)
        .stereo_mode(StereoMode::JointStereo);
    let mut e = Mp3Encoder::new(cfg).unwrap();
    let ints: Vec<i16> = pcm
        .iter()
        .map(|x| (x.clamp(-1.0, 1.0) * 32767.0).round() as i16)
        .collect();
    let spf = e.samples_per_frame() * 2;
    let mut out = Vec::new();
    for chunk in ints.chunks(spf) {
        let mut c = chunk.to_vec();
        c.resize(spf, 0);
        for f in e.encode_interleaved(&c).unwrap() {
            out.extend_from_slice(&f);
        }
    }
    if let Ok(rest) = e.finish() {
        out.extend_from_slice(&rest);
    }
    out
}

include!("measure.rs");

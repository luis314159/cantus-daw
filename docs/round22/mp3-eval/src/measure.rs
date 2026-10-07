fn decode(bytes: &[u8]) -> Vec<f32> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let cur = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cur), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .unwrap();
    let mut fmt = probed.format;
    let track = fmt.default_track().unwrap().clone();
    let mut dec = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .unwrap();
    let mut out = Vec::new();
    while let Ok(p) = fmt.next_packet() {
        let Ok(buf) = dec.decode(&p) else { continue };
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, *buf.spec());
        sb.copy_interleaved_ref(buf);
        out.extend_from_slice(sb.samples());
    }
    out
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

/// Best lag of `b` against `a` (left channel), 0..5000 frames.
fn align(a: &[f32], b: &[f32]) -> usize {
    let n = (a.len() / 2).min(SR as usize * 4);
    let mut best = (0, f64::MIN);
    for lag in 0..5000 {
        let mut c = 0.0;
        for i in (0..n).step_by(3) {
            if 2 * (i + lag) < b.len() {
                c += a[2 * i] as f64 * b[2 * (i + lag)] as f64;
            }
        }
        if c > best.1 {
            best = (lag, c);
        }
    }
    best.0
}

/// (whole SNR dB, mean band SNR dB under 16 kHz, 5th-percentile band SNR, bandwidth Hz)
fn measure(src: &[f32], dec: &[f32]) -> (f64, f64, f64, f64, usize) {
    let lag = align(src, dec);
    let n = src.len() / 2;
    let get = |i: usize, c: usize| dec.get(2 * (i + lag) + c).copied().unwrap_or(0.0) as f64;
    let (mut es, mut ee) = (0.0, 0.0);
    for i in 0..n {
        for c in 0..2 {
            let s = src[2 * i + c] as f64;
            let e = get(i, c) - s;
            es += s * s;
            ee += e * e;
        }
    }
    let snr = 10.0 * (es / ee).log10();
    // Bands: 1/3 octave from 50 Hz to 16 kHz.
    let fsz = 2048;
    let mut edges = vec![50.0f64];
    while *edges.last().unwrap() < 16_000.0 {
        let e = edges.last().unwrap() * 2f64.powf(1.0 / 3.0);
        edges.push(e.min(16_000.0));
    }
    let bin = |f: f64| (f / SR as f64 * fsz as f64).round() as usize;
    let mut snrs = Vec::new();
    let mut src_hi = vec![0.0; fsz / 2];
    let mut dec_hi = vec![0.0; fsz / 2];
    let win: Vec<f64> = (0..fsz)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / fsz as f64).cos())
        .collect();
    let mut f0 = 0;
    while f0 + fsz <= n {
        let (mut sr_, mut si) = (vec![0.0; fsz], vec![0.0; fsz]);
        let (mut er, mut ei) = (vec![0.0; fsz], vec![0.0; fsz]);
        let (mut dr, mut di) = (vec![0.0; fsz], vec![0.0; fsz]);
        for i in 0..fsz {
            let s = src[2 * (f0 + i)] as f64;
            let d = get(f0 + i, 0);
            sr_[i] = s * win[i];
            er[i] = (d - s) * win[i];
            dr[i] = d * win[i];
        }
        fft(&mut sr_, &mut si);
        fft(&mut er, &mut ei);
        fft(&mut dr, &mut di);
        for k in 0..fsz / 2 {
            src_hi[k] += sr_[k] * sr_[k] + si[k] * si[k];
            dec_hi[k] += dr[k] * dr[k] + di[k] * di[k];
        }
        let total: f64 = (0..fsz / 2).map(|k| sr_[k] * sr_[k] + si[k] * si[k]).sum();
        for w in edges.windows(2) {
            let (a, b) = (bin(w[0]), bin(w[1]).max(bin(w[0]) + 1));
            let s: f64 = (a..b).map(|k| sr_[k] * sr_[k] + si[k] * si[k]).sum();
            let e: f64 = (a..b).map(|k| er[k] * er[k] + ei[k] * ei[k]).sum();
            if s > total * 1e-6 && s > 1e-9 {
                snrs.push(10.0 * (s / e.max(1e-30)).log10());
            }
        }
        f0 += fsz;
    }
    snrs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = snrs.iter().sum::<f64>() / snrs.len() as f64;
    let p5 = snrs[snrs.len() / 20];
    // Bandwidth: the highest bin where the decoded spectrum is within 20 dB of the source's.
    let mut bw = 0.0;
    for k in (1..fsz / 2).rev() {
        if src_hi[k] > 0.0 && dec_hi[k] > src_hi[k] * 0.01 {
            bw = k as f64 * SR as f64 / fsz as f64;
            break;
        }
    }
    (snr, mean, p5, bw, lag)
}

fn report(name: &str, src: &[f32], bytes: &[u8], secs: f64) {
    let dec = decode(bytes);
    let (snr, mean, p5, bw, lag) = measure(src, &dec);
    let (n16, o16) = nmr(src, &dec, 16_000.0);
    let (nall, oall) = nmr(src, &dec, 22_050.0);
    println!("{name:<26} NMR<=16k {n16:>6.1} dB over {:>5.1} %   NMR full {nall:>6.1} dB over {:>5.1} %", o16*100.0, oall*100.0);
    let dur = src.len() as f64 / 2.0 / SR as f64;
    println!(
        "{name:<26} {:>6.1} kbps  SNR {snr:>5.1} dB  band SNR mean {mean:>5.1} p5 {p5:>5.1}  bw {:>5.0} Hz  lag {lag:>4}  len {}/{}  {:>5.0}x RT",
        bytes.len() as f64 * 8.0 / dur / 1000.0,
        bw,
        dec.len() / 2,
        src.len() / 2,
        dur / secs
    );
}

fn main() {
    let src = signal();
    let run = |name: &str, f: &dyn Fn() -> Vec<u8>| {
        let t = Instant::now();
        let b = f();
        report(name, &src, &b, t.elapsed().as_secs_f64());
    };
    run("LAME CBR 128", &|| encode_lame(&src, 128, None));
    run("rusty_mp3 CBR 128", &|| encode_rusty(&src, 128, None));
    run("shine CBR 128", &|| encode_shine(&src, 128));
    run("LAME CBR 192", &|| encode_lame(&src, 192, None));
    run("rusty_mp3 CBR 192", &|| encode_rusty(&src, 192, None));
    run("shine CBR 192", &|| encode_shine(&src, 192));
    run("LAME CBR 320", &|| encode_lame(&src, 320, None));
    run("rusty_mp3 CBR 320", &|| encode_rusty(&src, 320, None));
    run("shine CBR 320", &|| encode_shine(&src, 320));
    run("LAME VBR V0", &|| encode_lame(&src, 0, Some(0)));
    run("rusty_mp3 VBR q0", &|| encode_rusty(&src, 0, Some(0.0)));
    run("LAME CBR 320 q2", &|| encode_lame_q(&src, 320));
    run("LAME VBR V2", &|| encode_lame(&src, 0, Some(2)));
    run("rusty_mp3 VBR q2", &|| encode_rusty(&src, 0, Some(2.0)));
}

fn bark(f: f64) -> f64 {
    13.0 * (0.00076 * f).atan() + 3.5 * ((f / 7500.0) * (f / 7500.0)).atan()
}

/// Johnston-style noise-to-mask ratio: (mean NMR dB over frames and bands up to `top` Hz,
/// fraction of band-frames whose noise is over the mask).
fn nmr(src: &[f32], dec: &[f32], top: f64) -> (f64, f64) {
    let lag = align(src, dec);
    let n = src.len() / 2;
    let fsz = 2048;
    let nb = 25usize;
    let win: Vec<f64> = (0..fsz)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / fsz as f64).cos())
        .collect();
    let band_of: Vec<usize> = (0..fsz / 2)
        .map(|k| (bark(k as f64 * SR as f64 / fsz as f64) as usize).min(nb - 1))
        .collect();
    let (mut sum, mut cnt, mut over) = (0.0, 0usize, 0usize);
    let mut f0 = 0;
    while f0 + fsz <= n {
        for c in 0..2 {
            let (mut sr_, mut si) = (vec![0.0; fsz], vec![0.0; fsz]);
            let (mut er, mut ei) = (vec![0.0; fsz], vec![0.0; fsz]);
            for i in 0..fsz {
                let s = src[2 * (f0 + i) + c] as f64;
                let d = dec.get(2 * (f0 + i + lag) + c).copied().unwrap_or(0.0) as f64;
                sr_[i] = s * win[i];
                er[i] = (d - s) * win[i];
            }
            fft(&mut sr_, &mut si);
            fft(&mut er, &mut ei);
            let mut es = vec![0.0; nb];
            let mut en = vec![0.0; nb];
            let mut geo = vec![0.0; nb];
            let mut cntb = vec![0usize; nb];
            for k in 1..fsz / 2 {
                let b = band_of[k];
                let p = sr_[k] * sr_[k] + si[k] * si[k];
                es[b] += p;
                en[b] += er[k] * er[k] + ei[k] * ei[k];
                geo[b] += (p + 1e-20).ln();
                cntb[b] += 1;
            }
            for b in 0..nb {
                if cntb[b] == 0 {
                    continue;
                }
                let lo_f = (0..fsz / 2).find(|&k| band_of[k] == b).unwrap() as f64 * SR as f64
                    / fsz as f64;
                if lo_f > top {
                    continue;
                }
                // spread
                let mut spread = 0.0;
                for j in 0..nb {
                    let dz = b as f64 - j as f64;
                    let sdb = 15.81 + 7.5 * (dz + 0.474) - 17.5 * (1.0 + (dz + 0.474).powi(2)).sqrt();
                    spread += es[j] * 10f64.powf(sdb / 10.0);
                }
                let flat = (geo[b] / cntb[b] as f64).exp() / (es[b] / cntb[b] as f64 + 1e-20);
                let sfm = (10.0 * flat.max(1e-12).log10() / -60.0).min(1.0);
                let offset = sfm * (14.5 + b as f64) + (1.0 - sfm) * 5.5;
                // absolute threshold: full scale sine ~ 96 dB SPL; a 2048 Hann FFT of a
                // full-scale sine has power (fsz/4)^2 per bin pair.
                let fs_ref = (fsz as f64 / 4.0).powi(2);
                let f = lo_f.max(20.0) / 1000.0;
                let ath = 3.64 * f.powf(-0.8) - 6.5 * (-0.6 * (f - 3.3).powi(2)).exp()
                    + 1e-3 * f.powi(4);
                let ath_p = fs_ref * 10f64.powf((ath - 96.0) / 10.0) * cntb[b] as f64;
                let mask = (spread * 10f64.powf(-offset / 10.0)).max(ath_p);
                let r = 10.0 * (en[b] / mask + 1e-12).log10();
                if es[b] > ath_p || en[b] > ath_p {
                    sum += r;
                    cnt += 1;
                    if r > 0.0 {
                        over += 1;
                    }
                }
            }
        }
        f0 += fsz;
    }
    (sum / cnt.max(1) as f64, over as f64 / cnt.max(1) as f64)
}

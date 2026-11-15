//! One drum hit measured: the attack, the octave bands over time, each band's decay, the
//! low partial's glide, the level.

thread_local! {
    static RATE: std::cell::Cell<f64> = const { std::cell::Cell::new(48_000.0) };
}

/// The rate the next measurements read their hits at.
pub fn set_rate(fs: f64) {
    RATE.with(|r| r.set(fs));
}

#[allow(non_snake_case)]
fn FS() -> f64 {
    RATE.with(|r| r.get())
}
/// Octave band centres, Hz.
pub const BANDS: [f64; 10] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
/// Windows after the onset, ms.
pub const WINDOWS: [(f64, f64); 6] = [
    (0.0, 3.0),
    (3.0, 15.0),
    (15.0, 60.0),
    (60.0, 250.0),
    (250.0, 1000.0),
    (1000.0, 3000.0),
];

#[derive(Clone, Debug, Default)]
pub struct Hit {
    /// Peak (dBFS) and RMS over the first 500 ms (dBFS).
    pub peak_db: f64,
    pub rms_db: f64,
    /// Onset to peak, ms.
    pub attack_ms: f64,
    /// Spectral centroid (Hz) of the first 3 ms and of 15–60 ms.
    pub centroid_attack: f64,
    pub centroid_body: f64,
    /// Band energy per window, dB relative to the hit's loudest window-band.
    pub bands: [[f64; 10]; 6],
    /// Per band: ms from onset to its envelope's peak, then to 20 and 40 dB below it.
    pub band_peak_ms: [f64; 10],
    pub t20_ms: [f64; 10],
    pub t40_ms: [f64; 10],
    /// The strongest partial 30–700 Hz in 10–60 ms and in 150–400 ms, Hz.
    pub f_early: f64,
    pub f_late: f64,
    /// The three strongest peaks 30–1500 Hz in 20–200 ms, Hz.
    pub partials: [f64; 4],
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
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for s in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let a = s + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let n2 = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = n2;
            }
        }
        len <<= 1;
    }
}

/// Power spectrum of `x` (Hann-windowed when `hann`), zero-padded to at least `min_n`.
pub fn power(x: &[f64], min_n: usize, hann: bool) -> Vec<f64> {
    let n = x.len().max(min_n).next_power_of_two();
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    let l = x.len().max(2);
    for (i, v) in x.iter().enumerate() {
        let w = if hann {
            0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (l - 1) as f64).cos()
        } else {
            1.0
        };
        re[i] = v * w;
    }
    fft(&mut re, &mut im);
    (0..n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect()
}

fn band_of(hz: f64) -> Option<usize> {
    BANDS
        .iter()
        .position(|c| hz >= c / std::f64::consts::SQRT_2 && hz < c * std::f64::consts::SQRT_2)
}

fn db(x: f64) -> f64 {
    10.0 * x.max(1e-30).log10()
}

fn peak_in(p: &[f64], n: usize, lo: f64, hi: f64) -> f64 {
    let bin = FS() / n as f64;
    let (a, b) = ((lo / bin) as usize, ((hi / bin) as usize).min(p.len() - 2));
    let mut k = a.max(1);
    for i in a.max(1)..=b {
        if p[i] > p[k] {
            k = i;
        }
    }
    // Parabolic interpolation on the log power.
    let (l, c, r) = (db(p[k - 1]), db(p[k]), db(p[k + 1]));
    let d = 0.5 * (l - r) / (l - 2.0 * c + r).min(-1e-9);
    (k as f64 + d) * bin
}

/// The strongest `m` local peaks in `lo..hi`, at least a semitone apart.
fn peaks(p: &[f64], n: usize, lo: f64, hi: f64, m: usize) -> Vec<f64> {
    let bin = FS() / n as f64;
    let (a, b) = ((lo / bin) as usize, ((hi / bin) as usize).min(p.len() - 2));
    let mut c: Vec<(f64, f64)> = (a.max(1)..=b)
        .filter(|&i| p[i] > p[i - 1] && p[i] >= p[i + 1])
        .map(|i| (p[i], i as f64 * bin))
        .collect();
    c.sort_by(|x, y| y.0.total_cmp(&x.0));
    let mut out: Vec<f64> = Vec::new();
    for (_, f) in c {
        if out.iter().all(|g| (f / g).ln().abs() > 0.0578) {
            out.push(f);
        }
        if out.len() == m {
            break;
        }
    }
    out
}

/// Measure one hit (mono, the onset somewhere near its start).
pub fn measure(x: &[f64]) -> Hit {
    let mut h = Hit::default();
    let peak = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if peak <= 0.0 {
        return h;
    }
    let onset = x.iter().position(|v| v.abs() > 0.05 * peak).unwrap_or(0);
    // Back up to where it first left silence (−60 dB under the peak).
    let mut t0 = onset;
    while t0 > 0 && x[t0 - 1].abs() > 1e-3 * peak {
        t0 -= 1;
    }
    let x = &x[t0..];
    let ms = |m: f64| ((m * 0.001 * FS()) as usize).min(x.len());
    let pk = x.iter().position(|v| v.abs() == peak).unwrap_or(0);
    h.peak_db = 20.0 * peak.log10();
    h.attack_ms = pk as f64 * 1000.0 / FS();
    let e500 = &x[..ms(500.0)];
    h.rms_db = db(e500.iter().map(|v| v * v).sum::<f64>() / e500.len().max(1) as f64);
    // Bands per window (power density: per sample of the window).
    let mut top = 1e-30f64;
    for (w, (a, b)) in WINDOWS.iter().enumerate() {
        let seg = &x[ms(*a)..ms(*b)];
        if seg.len() < 8 {
            continue;
        }
        let n = 8192.max(seg.len());
        let p = power(seg, n, false);
        let nn = p.len() * 2;
        let mut e = [0.0f64; 10];
        let mut num = 0.0;
        let mut den = 0.0;
        for (k, v) in p.iter().enumerate() {
            let hz = k as f64 * FS() / nn as f64;
            if let Some(b) = band_of(hz) {
                e[b] += v;
            }
            if hz > 20.0 {
                num += hz * v;
                den += v;
            }
        }
        for x in e.iter_mut() {
            *x /= seg.len() as f64;
            top = top.max(*x);
        }
        h.bands[w] = e;
        if w == 0 {
            h.centroid_attack = num / den.max(1e-30);
        }
        if w == 2 {
            h.centroid_body = num / den.max(1e-30);
        }
    }
    for w in 0..6 {
        for b in 0..10 {
            h.bands[w][b] = db(h.bands[w][b] / top);
        }
    }
    // Each band's envelope: 1024-point frames every 256 samples.
    let (fl, hop) = (1024usize, 256usize);
    let frames = (x.len().saturating_sub(fl)) / hop;
    let mut env = vec![[0.0f64; 10]; frames];
    for (f, e) in env.iter_mut().enumerate() {
        let p = power(&x[f * hop..f * hop + fl], fl, true);
        for (k, v) in p.iter().enumerate() {
            if let Some(b) = band_of(k as f64 * FS() / fl as f64) {
                e[b] += v;
            }
        }
    }
    let fms = |f: usize| (f * hop + fl / 2) as f64 * 1000.0 / FS();
    for b in 0..10 {
        let (mut fm, mut m) = (0usize, 0.0f64);
        for (f, e) in env.iter().enumerate() {
            if e[b] > m {
                m = e[b];
                fm = f;
            }
        }
        h.band_peak_ms[b] = fms(fm);
        let fall = |d: f64| {
            env.iter()
                .enumerate()
                .skip(fm)
                .find(|(_, e)| db(e[b] / m) < -d)
                .map_or(f64::NAN, |(f, _)| fms(f))
        };
        h.t20_ms[b] = fall(20.0);
        h.t40_ms[b] = fall(40.0);
    }
    // The low partial, early and late.
    let n = 1 << 16;
    let early = &x[ms(10.0)..ms(60.0)];
    let late = &x[ms(150.0)..ms(400.0)];
    if early.len() > 64 {
        h.f_early = peak_in(&power(early, n, true), n, 30.0, 700.0);
    }
    if late.len() > 64 {
        h.f_late = peak_in(&power(late, n, true), n, 30.0, 700.0);
    }
    let mid = &x[ms(20.0)..ms(200.0)];
    if mid.len() > 64 {
        for (i, f) in peaks(&power(mid, n, true), n, 30.0, 1500.0, 4)
            .into_iter()
            .enumerate()
        {
            h.partials[i] = f;
        }
    }
    h
}

/// The hit as text, a few lines.
pub fn text(h: &Hit) -> String {
    let mut s = String::new();
    s += &format!(
        "  peak {:6.1} dB  rms500 {:6.1} dB  attack {:5.2} ms  centroid 0–3ms {:6.0} Hz, 15–60ms {:6.0} Hz\n",
        h.peak_db, h.rms_db, h.attack_ms, h.centroid_attack, h.centroid_body
    );
    s += &format!(
        "  low partial {:6.1} Hz early → {:6.1} Hz late ({:+.0} cents); peaks {:.0} {:.0} {:.0} {:.0}\n",
        h.f_early,
        h.f_late,
        1200.0 * (h.f_early / h.f_late.max(1e-9)).log2(),
        h.partials[0],
        h.partials[1],
        h.partials[2],
        h.partials[3]
    );
    s += "  bands Hz      31   63  125  250  500   1k   2k   4k   8k  16k\n";
    for (w, (a, b)) in WINDOWS.iter().enumerate() {
        s += &format!("  {:>4}-{:<5}", a, b);
        for v in h.bands[w] {
            s += &format!("{:5.0}", v.max(-99.0));
        }
        s += "\n";
    }
    let row = |name: &str, v: &[f64; 10]| {
        let mut r = format!("  {name:<11}");
        for x in v {
            if x.is_nan() {
                r += "    -";
            } else {
                r += &format!("{:5.0}", x);
            }
        }
        r + "\n"
    };
    s += &row("peak at ms", &h.band_peak_ms);
    s += &row("T20 ms", &h.t20_ms);
    s += &row("T40 ms", &h.t40_ms);
    s
}

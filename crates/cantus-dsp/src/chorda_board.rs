//! Chorda's post-voice stage: what every voice's bridge force does together, run once on
//! the voices' sum (in voice order) — the strings ringing in sympathy and the soundboard.
//!
//! **Sympathetic strings.** Every key's own string, at its own pitch (its speaking length:
//! never a detuned or non-speaking loop), hangs on the bridge: the bridge's motion
//! (`Y · F` for the voices' total force `F`) drives its end, and what it rings with goes to
//! the soundboard — one way, so nothing feeds back. Its damper is the key's: down unless
//! the sustain pedal lifts it (continuously, as for the voices) or the key's own voice
//! sounds (that key's strings are the voice's); the keys above E6 have none and always
//! ring along, as on a grand. A sympathetic string is one loop with a reduced design of
//! its key (at most two dispersion sections, `design::design_limited`): the first partial
//! exact and the low ones, where sympathy is heard, close.
//!
//! **The soundboard** (after Bank et al. 2003's multirate board): the low band is a bank
//! of resonant modes running at a quarter of the sample rate (60 Hz – 1.5 kHz, a soundboard's
//! modal density and its loss factor of about 3 %, Conklin 1996), the high band a small
//! feedback delay network whose decay shortens with frequency, and the bridge's force goes
//! straight out as well (the board's direct radiation, a 35 Hz high-pass and a tilt).

use super::design::LoopDesign;
use super::string::Ring;
use super::tables::Key;
use crate::math;

/// The board's low modes.
pub const MODES: usize = 40;
/// The low band's decimation.
pub const DECIMATE: u32 = 4;
const FDN: usize = 4;

/// A sympathetic string: one loop, its damper, its drive.
#[derive(Clone, Debug)]
struct SymString {
    line: Ring,
    fir: [f64; 2],
    frac: f64,
    frac_x: f64,
    frac_y: f64,
    a1: [f64; 2],
    a2: [f64; 2],
    sections: usize,
    bq: [[f64; 4]; 2],
    g: f64,
    alpha: f64,
    /// Its wave impedance and the bridge's admittance seen from it (`Y·Z`).
    z: f64,
    yz: f64,
    contact: f64,
    target: f64,
    set_contact: f64,
    /// Whether it holds anything (skipped while silent and undriven).
    live: bool,
    quiet: u32,
    peak: f64,
    key: Key,
    sounding: bool,
}

#[derive(Clone, Debug)]
pub struct Sympathetic {
    strings: Vec<SymString>,
    fs: f64,
    phase: u32,
    /// The drive's scale (the instrument's *sympathetic* setting).
    pub level: f64,
    pub decay: f64,
}

impl Sympathetic {
    /// Room for every key's loop. Allocates.
    pub fn new(capacities: &[usize], keys: &[Key], fs: f64) -> Sympathetic {
        let strings = capacities
            .iter()
            .zip(keys)
            .map(|(c, k)| SymString {
                line: Ring::new(*c),
                fir: [0.0; 2],
                frac: 0.0,
                frac_x: 0.0,
                frac_y: 0.0,
                a1: [0.0; 2],
                a2: [0.0; 2],
                sections: 0,
                bq: [[0.0; 4]; 2],
                g: 0.0,
                alpha: 0.0,
                z: k.impedance,
                yz: 0.0,
                contact: 1.0,
                target: 1.0,
                set_contact: -1.0,
                live: false,
                quiet: 0,
                peak: 0.0,
                key: *k,
                sounding: false,
            })
            .collect();
        Sympathetic {
            strings,
            fs,
            phase: 0,
            level: 1.0,
            decay: 1.0,
        }
    }

    /// Lay every string out for a keyboard's reduced designs (silent).
    pub fn tune(&mut self, keys: &[Key], designs: &[LoopDesign]) {
        for (s, (k, d)) in self.strings.iter_mut().zip(keys.iter().zip(designs)) {
            s.key = *k;
            s.z = k.impedance;
            s.line.reset(d.delay.max(2));
            s.fir = [0.0; 2];
            s.frac = d.frac;
            s.frac_x = 0.0;
            s.frac_y = 0.0;
            s.sections = d.sections.min(2);
            for i in 0..s.sections {
                let (a1, a2) = super::design::second_coefs(d.poles[i].0, d.poles[i].1);
                s.a1[i] = a1;
                s.a2[i] = a2;
            }
            s.bq = [[0.0; 4]; 2];
            s.live = false;
            s.set_contact = -1.0;
        }
    }

    /// The dampers: lifted by the pedal (0…1), and a key whose voice sounds is skipped.
    pub fn set_dampers(&mut self, pedal: f64, sounding: &[bool]) {
        let contact = super::voice::pedal_contact(pedal);
        for (s, on) in self.strings.iter_mut().zip(sounding) {
            s.sounding = *on;
            s.target = if s.key.damper { contact } else { 0.0 };
        }
    }

    /// Silence every string.
    pub fn reset(&mut self) {
        for s in self.strings.iter_mut() {
            let len = s.line.len_now();
            s.line.reset(len);
            s.fir = [0.0; 2];
            s.frac_x = 0.0;
            s.frac_y = 0.0;
            s.bq = [[0.0; 4]; 2];
            s.live = false;
            s.quiet = 0;
            s.contact = s.target;
            s.set_contact = -1.0;
        }
        self.phase = 0;
    }

    pub fn at_rest(&self) -> bool {
        !self.strings.iter().any(|s| s.live)
    }

    /// Frames skipped at rest: the control phase moves on.
    pub fn skip(&mut self, frames: usize) {
        self.phase = ((self.phase as usize + frames) % super::voice::CONTROL as usize) as u32;
    }

    /// Add to `out` what the strings ring with, driven by the voices' total force `drive`.
    pub fn process(&mut self, drive: &[f64], out: &mut [f64]) {
        let level = self.level;
        for (i, (d, o)) in drive.iter().zip(out.iter_mut()).enumerate() {
            if self.phase == 0 {
                self.control(drive, i);
            }
            let mut sum = 0.0;
            for s in self.strings.iter_mut() {
                if !s.live || s.sounding {
                    continue;
                }
                let raw = s.line.read();
                let lossy = s.g * (s.alpha * (raw + s.fir[1]) + (1.0 - 2.0 * s.alpha) * s.fir[0]);
                s.fir[1] = s.fir[0];
                s.fir[0] = raw;
                let y = s.frac * (lossy - s.frac_y) + s.frac_x;
                s.frac_x = lossy;
                s.frac_y = y;
                let mut x = y;
                for k in 0..s.sections {
                    let st = &mut s.bq[k];
                    let (a1, a2) = (s.a1[k], s.a2[k]);
                    let out = a2 * x + a1 * st[0] + st[1] - a1 * st[2] - a2 * st[3];
                    st[1] = st[0];
                    st[0] = x;
                    st[3] = st[2];
                    st[2] = out;
                    x = out;
                }
                // The bridge moves with the drive: the end's reflection, and the force.
                let v_b = level * s.yz / s.z * d;
                s.line.write(2.0 * v_b - x);
                let f = s.z * 2.0 * (x - v_b);
                s.peak = s.peak.max(f.abs());
                sum += f;
            }
            *o += sum;
            self.phase += 1;
            if self.phase == super::voice::CONTROL {
                self.phase = 0;
            }
        }
    }

    /// Every control step: the dampers glide, the loss filters follow, a quiet string is
    /// let go and a driven one woken.
    fn control(&mut self, drive: &[f64], at: usize) {
        let k = 1.0 - math::exp(-(super::voice::CONTROL as f64) / (0.008 * self.fs));
        let end = (at + super::voice::CONTROL as usize).min(drive.len());
        let driven = drive[at..end].iter().any(|d| *d != 0.0);
        for s in self.strings.iter_mut() {
            if s.sounding {
                continue;
            }
            s.contact += (s.target - s.contact) * k;
            if (s.contact - s.target).abs() < 1e-4 {
                s.contact = s.target;
            }
            if driven && !s.live && s.contact < 0.999 {
                s.live = true;
                s.quiet = 0;
            }
            if !s.live {
                continue;
            }
            if s.contact != s.set_contact {
                let grip = 6.907_755 / super::voice::damped_t60(s.key.key as f64);
                let (g, a, _) =
                    super::string::loss(&s.key, self.fs, 0.0, self.decay, s.contact * grip, 0.0);
                s.g = g;
                s.alpha = a;
                s.yz = math::tanh(0.5 * s.key.b1 / (2.0 * s.key.partial(1)));
                s.set_contact = s.contact;
            }
            if s.peak < 1.0e-10 && !driven {
                s.quiet += 1;
                if s.quiet >= 64 {
                    let len = s.line.len_now();
                    s.line.reset(len);
                    s.fir = [0.0; 2];
                    s.frac_x = 0.0;
                    s.frac_y = 0.0;
                    s.bq = [[0.0; 4]; 2];
                    s.live = false;
                    s.quiet = 0;
                }
            } else {
                s.quiet = 0;
            }
            s.peak = 0.0;
        }
    }
}

/// A small, fixed variation (−1…1) from an index: hashed, never random.
fn hash(i: u64) -> f64 {
    let mut z = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x50_0B0A_2D;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// The soundboard: direct radiation, low modes at a quarter rate, a high-band FDN.
#[derive(Clone, Debug)]
pub struct Soundboard {
    /// Low modes: per mode `b0` (input), `a1`, `a2`, and state `y1`, `y2`.
    modes: [[f64; 5]; MODES],
    /// The decimator's running sum, the low band's last two outputs (for interpolation).
    acc: f64,
    low_prev: f64,
    low_now: f64,
    /// Samples into the current quarter-rate step, counted from the stream's start.
    tick: u32,
    /// The FDN: lines, their damping one-poles, the input's high-pass.
    lines: [Ring; FDN],
    damp: [f64; FDN],
    damp_a: [f64; FDN],
    damp_g: [f64; FDN],
    hp_x: f64,
    hp_y: f64,
    hp_a: f64,
    /// The direct path's 35 Hz high-pass (per channel) and its tilt.
    dc_x: [f64; 2],
    dc_y: [f64; 2],
    dc_a: f64,
    tilt: [f64; 2],
    tilt_a: f64,
    /// The body's level (modes and FDN), and the tilt's shelf gain minus one.
    pub body: f64,
    shelf: f64,
}

impl Soundboard {
    /// Allocates.
    pub fn new(fs: f64) -> Soundboard {
        let fsd = fs / DECIMATE as f64;
        let mut modes = [[0.0; 5]; MODES];
        for (i, m) in modes.iter_mut().enumerate() {
            // Log-spaced 60 Hz … 1.5 kHz, each nudged a little (a board's modes are not a
            // grid), loss factor about 3 % (Q ≈ 33), the higher ones weaker.
            let t = i as f64 / (MODES - 1) as f64;
            let f = 60.0 * math::powf(25.0f64, t) * (1.0 + 0.04 * hash(i as u64));
            let q = 33.0 * (1.0 + 0.2 * hash(100 + i as u64));
            let w = std::f64::consts::TAU * f.min(0.45 * fsd) / fsd;
            let r = math::exp(-w / (2.0 * q));
            let gain = math::powf(f / 100.0, -0.5) * (1.0 + 0.3 * hash(200 + i as u64));
            *m = [
                gain * (1.0 - r) / (MODES as f64).sqrt(),
                -2.0 * r * math::cos(w),
                r * r,
                0.0,
                0.0,
            ];
        }
        // FDN lines 2.9 … 8.3 ms; each line's one-pole damping makes the board ring about
        // 0.25 s at 1 kHz and shorter above.
        let lens_ms = [2.9, 4.3, 6.1, 8.3];
        let mut lines: [Ring; FDN] =
            std::array::from_fn(|i| Ring::new((lens_ms[i] * 1e-3 * fs) as usize + 1));
        let mut damp_a = [0.0; FDN];
        let mut damp_g = [0.0; FDN];
        for i in 0..FDN {
            let n = (lens_ms[i] * 1e-3 * fs) as usize;
            lines[i].reset(n.max(1));
            let g = math::powf(10.0f64, -3.0 * n as f64 / (0.25 * fs));
            damp_g[i] = g;
            damp_a[i] = math::exp(-std::f64::consts::TAU * 3_000.0 / fs);
        }
        Soundboard {
            modes,
            acc: 0.0,
            low_prev: 0.0,
            low_now: 0.0,
            tick: 0,
            lines,
            damp: [0.0; FDN],
            damp_a,
            damp_g,
            hp_x: 0.0,
            hp_y: 0.0,
            hp_a: math::exp(-std::f64::consts::TAU * 800.0 / fs),
            dc_x: [0.0; 2],
            dc_y: [0.0; 2],
            dc_a: math::exp(-std::f64::consts::TAU * 35.0 / fs),
            tilt: [0.0; 2],
            tilt_a: math::exp(-std::f64::consts::TAU * 2_500.0 / fs),
            body: 1.0,
            shelf: 0.0,
        }
    }

    /// The tilt: the top raised or lowered by `db` (a shelf from about 2.5 kHz).
    pub fn set_tilt(&mut self, db: f64) {
        self.shelf = math::powf(10.0f64, db / 20.0) - 1.0;
    }

    pub fn reset(&mut self) {
        for m in self.modes.iter_mut() {
            m[3] = 0.0;
            m[4] = 0.0;
        }
        self.acc = 0.0;
        self.low_prev = 0.0;
        self.low_now = 0.0;
        self.tick = 0;
        for l in self.lines.iter_mut() {
            let n = l.len_now();
            l.reset(n);
        }
        self.damp = [0.0; FDN];
        self.hp_x = 0.0;
        self.hp_y = 0.0;
        self.dc_x = [0.0; 2];
        self.dc_y = [0.0; 2];
        self.tilt = [0.0; 2];
    }

    /// Whether every state is exactly zero (what the rest of the instrument's silence needs).
    pub fn at_rest(&self) -> bool {
        self.modes.iter().all(|m| m[3] == 0.0 && m[4] == 0.0)
            && self.acc == 0.0
            && self.low_prev == 0.0
            && self.low_now == 0.0
            && self.lines.iter().all(|l| l.peak() == 0.0)
            && self.damp.iter().all(|d| *d == 0.0)
            && self.hp_x == 0.0
            && self.hp_y == 0.0
            && self.dc_x == [0.0; 2]
            && self.dc_y == [0.0; 2]
            && self.tilt == [0.0; 2]
    }

    /// The largest state of each part (tests): modes, decimator, lines, damping, filters.
    pub fn largest(&self) -> [f64; 5] {
        let modes = self
            .modes
            .iter()
            .fold(0.0f64, |m, x| m.max(x[3].abs()).max(x[4].abs()));
        let dec = self
            .acc
            .abs()
            .max(self.low_prev.abs())
            .max(self.low_now.abs());
        let lines = self.lines.iter().fold(0.0f64, |m, l| m.max(l.peak()));
        let damp = self.damp.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        let f = [
            self.hp_x,
            self.hp_y,
            self.dc_x[0],
            self.dc_x[1],
            self.dc_y[0],
            self.dc_y[1],
            self.tilt[0],
            self.tilt[1],
        ]
        .iter()
        .fold(0.0f64, |m, x| m.max(x.abs()));
        [modes, dec, lines, damp, f]
    }

    /// Frames skipped at rest: the quarter-rate phase moves on.
    pub fn skip(&mut self, frames: usize) {
        self.tick = ((self.tick as usize + frames) % DECIMATE as usize) as u32;
    }

    /// Flush every state below `floor` to zero (exact silence after the sound has gone).
    pub fn flush(&mut self, floor: f64) {
        let f = |x: &mut f64| {
            if x.abs() < floor {
                *x = 0.0;
            }
        };
        for m in self.modes.iter_mut() {
            f(&mut m[3]);
            f(&mut m[4]);
        }
        f(&mut self.acc);
        f(&mut self.low_prev);
        f(&mut self.low_now);
        for d in self.damp.iter_mut() {
            f(d);
        }
        f(&mut self.hp_x);
        f(&mut self.hp_y);
        for c in 0..2 {
            f(&mut self.dc_x[c]);
            f(&mut self.dc_y[c]);
            f(&mut self.tilt[c]);
        }
        for l in self.lines.iter_mut() {
            if l.peak() < floor {
                let n = l.len_now();
                l.reset(n);
            }
        }
    }

    /// One sample: the voices' panned forces `(l, r)` and the board's input (their sum and
    /// the sympathetic strings') in, the radiated stereo out.
    #[inline]
    pub fn tick(&mut self, l: f64, r: f64, input: f64) -> (f64, f64) {
        // The low band, a quarter of the rate: box-decimated in, linearly interpolated out.
        self.acc += input;
        self.tick += 1;
        if self.tick == DECIMATE {
            self.tick = 0;
            let x = self.acc / DECIMATE as f64;
            self.acc = 0.0;
            let mut y = 0.0;
            for m in self.modes.iter_mut() {
                let out = m[0] * x - m[1] * m[3] - m[2] * m[4];
                m[4] = m[3];
                m[3] = out;
                y += out;
            }
            self.low_prev = self.low_now;
            self.low_now = y;
        }
        let t = self.tick as f64 / DECIMATE as f64;
        let low = self.low_prev + (self.low_now - self.low_prev) * t;
        // The high band: the input above 800 Hz into a Hadamard FDN.
        let hp = self.hp_a * (self.hp_y + input - self.hp_x);
        self.hp_x = input;
        self.hp_y = hp;
        let outs: [f64; FDN] = std::array::from_fn(|i| self.lines[i].read());
        let h = [
            outs[0] + outs[1] + outs[2] + outs[3],
            outs[0] - outs[1] + outs[2] - outs[3],
            outs[0] + outs[1] - outs[2] - outs[3],
            outs[0] - outs[1] - outs[2] + outs[3],
        ];
        for i in 0..FDN {
            let fb = 0.5 * h[i] * self.damp_g[i];
            self.damp[i] = fb + self.damp_a[i] * (self.damp[i] - fb);
            self.lines[i].write(self.damp[i] + 0.25 * hp);
        }
        let body = self.body;
        let mut out = [0.0; 2];
        for (c, x) in [l, r].into_iter().enumerate() {
            // The direct radiation: a 35 Hz high-pass and the tilt (a high shelf).
            let y = self.dc_a * (self.dc_y[c] + x - self.dc_x[c]);
            self.dc_x[c] = x;
            self.dc_y[c] = y;
            self.tilt[c] = y + self.tilt_a * (self.tilt[c] - y);
            let high = y - self.tilt[c];
            let fdn = if c == 0 {
                outs[0] + outs[2]
            } else {
                outs[1] + outs[3]
            };
            out[c] = y + self.shelf * high + body * (low + 0.5 * fdn);
        }
        (out[0], out[1])
    }
}

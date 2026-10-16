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
/// Radiation: a plate below its coincidence frequency radiates the better the higher the
/// frequency, so what reaches the listener rises against the bridge's force — modelled as a
/// first-order rise of 20 dB between 120 Hz and 1.2 kHz (unity above: `1 − 0.9·LP₁₂₀₀`).
const RADIATION_HZ: f64 = 1_200.0;
const RADIATION_CUT: f64 = 0.9;
/// The bridge's motion a sympathetic string's end follows, against the voices' own
/// coupling: a fifth (the bridge is stiffer where the struck key's strings do not cross it).
const SYMPATHY: f64 = 0.2;

/// What a sympathetic string is laid out with, copied from a keyboard's reduced design so
/// a new keyboard can wait until the string is silent.
#[derive(Clone, Copy, Debug)]
struct Layout {
    key: Key,
    delay: usize,
    frac: f64,
    sections: usize,
    poles: [(f64, f64); 2],
    a1: [f64; 2],
    a2: [f64; 2],
}

impl Layout {
    fn of(key: &Key, d: &LoopDesign) -> Layout {
        let sections = d.sections.min(2);
        let mut l = Layout {
            key: *key,
            delay: d.delay.max(2),
            frac: d.frac,
            sections,
            poles: [(0.0, 0.0); 2],
            a1: [0.0; 2],
            a2: [0.0; 2],
        };
        for i in 0..sections {
            l.poles[i] = d.poles[i];
            let (a1, a2) = super::design::second_coefs(d.poles[i].0, d.poles[i].1);
            l.a1[i] = a1;
            l.a2[i] = a2;
        }
        l
    }
}

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
    poles: [(f64, f64); 2],
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
    /// A new keyboard's layout, taken once the string is silent: a string still ringing
    /// fades out first ([`FADE`]; never cut), even while the bridge drives it (sound
    /// review 2: under continuous playing it never went quiet and kept the old tuning).
    next: Option<Layout>,
    /// Its output's gain: 1, or falling to 0 while a new layout waits.
    fade: f64,
}

/// Samples a ringing string takes to fade out before it takes a new keyboard's layout.
pub const FADE: usize = 256;

impl SymString {
    /// Its damper with the pedal up: down as firmly as the dampers grip, or none above E6.
    fn rest(&self, grip: f64) -> f64 {
        if self.key.damper {
            grip
        } else {
            0.0
        }
    }

    /// Silent, its state cleared (its layout and damper stay); a waiting layout is taken.
    fn silence(&mut self) {
        let len = self.line.len_now();
        self.line.reset(len);
        self.fir = [0.0; 2];
        self.frac_x = 0.0;
        self.frac_y = 0.0;
        self.bq = [[0.0; 4]; 2];
        self.live = false;
        self.quiet = 0;
        self.peak = 0.0;
        self.fade = 1.0;
        if let Some(l) = self.next.take() {
            self.lay(&l);
        }
    }

    fn lay(&mut self, l: &Layout) {
        self.key = l.key;
        self.z = l.key.impedance;
        self.line.reset(l.delay);
        self.frac = l.frac;
        self.sections = l.sections;
        self.poles = l.poles;
        self.a1 = l.a1;
        self.a2 = l.a2;
        self.fir = [0.0; 2];
        self.frac_x = 0.0;
        self.frac_y = 0.0;
        self.bq = [[0.0; 4]; 2];
        self.set_contact = -1.0;
    }
}

#[derive(Clone, Debug)]
pub struct Sympathetic {
    strings: Vec<SymString>,
    fs: f64,
    phase: u32,
    /// Whether the drive moved since the last control step (what wakes a string: never a
    /// look ahead, which a block's end would cut).
    drive_seen: bool,
    /// The dampers' glide per control step (8 ms).
    glide: f64,
    /// The drive's 20 Hz high-pass: a bridge does not move steadily, and a loop in tune at
    /// its partials also holds DC — no string can, so none is fed in.
    hp_x: f64,
    hp_y: f64,
    hp_a: f64,
    /// The drive's scale (the instrument's *sympathetic* setting).
    pub level: f64,
    decay: f64,
    /// How firmly the dampers grip (the instrument's *dampers* setting, as for the voices).
    grip: f64,
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
                poles: [(0.0, 0.0); 2],
                bq: [[0.0; 4]; 2],
                g: 0.0,
                alpha: 0.0,
                z: k.impedance,
                yz: 0.0,
                // Where a pedal at rest leaves it: down, or none at all above E6.
                contact: if k.damper { 1.0 } else { 0.0 },
                target: if k.damper { 1.0 } else { 0.0 },
                set_contact: -1.0,
                live: false,
                quiet: 0,
                peak: 0.0,
                key: *k,
                sounding: false,
                next: None,
                fade: 1.0,
            })
            .collect();
        Sympathetic {
            strings,
            fs,
            phase: 0,
            drive_seen: false,
            glide: 1.0 - math::exp(-(super::voice::CONTROL as f64) / (0.008 * fs)),
            hp_x: 0.0,
            hp_y: 0.0,
            hp_a: math::exp(-std::f64::consts::TAU * 20.0 / fs),
            level: 1.0,
            decay: 1.0,
            grip: 1.0,
        }
    }

    /// Lay every string out for a keyboard's reduced designs: a silent one at once, one
    /// still ringing once it has rung out.
    pub fn tune(&mut self, keys: &[Key], designs: &[LoopDesign]) {
        for (s, (k, d)) in self.strings.iter_mut().zip(keys.iter().zip(designs)) {
            let l = Layout::of(k, d);
            if s.live {
                s.next = Some(l);
            } else {
                s.next = None;
                s.lay(&l);
            }
        }
    }

    /// The strings' decay against the measured (as for the voices): a ringing string's loss
    /// follows at its next control step.
    pub fn set_decay(&mut self, decay: f64) {
        if decay != self.decay {
            self.decay = decay;
            for s in self.strings.iter_mut() {
                s.set_contact = -1.0;
            }
        }
    }

    /// How firmly the dampers grip (0…1). A string resting where the old grip left it rests
    /// where the new one does.
    pub fn set_grip(&mut self, grip: f64) {
        let grip = grip.clamp(0.0, 1.0);
        if grip == self.grip {
            return;
        }
        let old = self.grip;
        self.grip = grip;
        for s in self.strings.iter_mut() {
            if !s.live && !s.sounding && s.contact == s.rest(old) && s.target == s.rest(old) {
                s.contact = s.rest(grip);
                s.target = s.contact;
                s.set_contact = -1.0;
            }
        }
    }

    /// The dampers: lifted by the pedal (0…1), and a key whose voice sounds is skipped —
    /// silenced as its voice starts (the voice plays those strings now), and back from
    /// silence, its damper where the pedal leaves it, when the voice has ended. Called on
    /// the instrument's own clock (every chunk of the stream), never between.
    pub fn set_dampers(&mut self, pedal: f64, sounding: &[bool]) {
        let contact = super::voice::pedal_contact(pedal) * self.grip;
        for (s, on) in self.strings.iter_mut().zip(sounding) {
            s.target = if s.key.damper { contact } else { 0.0 };
            if *on != s.sounding {
                s.silence();
                s.contact = s.target;
                s.set_contact = -1.0;
            }
            s.sounding = *on;
        }
    }

    /// Silence every string.
    pub fn reset(&mut self) {
        let grip = self.grip;
        for s in self.strings.iter_mut() {
            s.silence();
            s.contact = s.rest(grip);
            s.target = s.contact;
            s.sounding = false;
            s.set_contact = -1.0;
        }
        self.phase = 0;
        self.drive_seen = false;
        self.hp_x = 0.0;
        self.hp_y = 0.0;
    }

    /// Nothing ringing, nothing driven, every damper where a new instrument has it.
    pub fn at_rest(&self) -> bool {
        let grip = self.grip;
        !self.drive_seen
            && self.hp_x == 0.0
            && self.hp_y == 0.0
            && self.strings.iter().all(|s| {
                !s.live && !s.sounding && s.contact == s.rest(grip) && s.target == s.rest(grip)
            })
    }

    /// Strings still waiting for a new keyboard's layout (tests).
    #[cfg(test)]
    pub fn waiting(&self) -> usize {
        self.strings.iter().filter(|s| s.next.is_some()).count()
    }

    /// Frames skipped at rest: the control phase moves on.
    pub fn skip(&mut self, frames: usize) {
        self.phase = ((self.phase as usize + frames) % super::voice::CONTROL as usize) as u32;
    }

    /// Add to `out` what the strings ring with, driven by the voices' total force `drive`.
    pub fn process(&mut self, drive: &[f64], out: &mut [f64]) {
        let level = self.level;
        for (d, o) in drive.iter().zip(out.iter_mut()) {
            if self.phase == 0 {
                let driven = self.drive_seen || *d != 0.0;
                self.drive_seen = false;
                self.control(driven);
            }
            self.drive_seen |= *d != 0.0;
            let hp = self.hp_a * (self.hp_y + d - self.hp_x);
            self.hp_x = *d;
            self.hp_y = hp;
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
                // The ring is the whole round trip: the nut's rigid reflection inverts the
                // wave (`arrived = −x`, as `-nut.read()` does in a voice), and the moving
                // bridge reflects it again: `2·v_b − arrived`. Two inversions: the loop is in
                // tune at the key's own partials (one would put it at (k − ½)·f₀).
                let v_b = SYMPATHY * level * s.yz / s.z * hp;
                let arrived = -x;
                s.line.write(2.0 * v_b - arrived);
                let f = s.z * 2.0 * (arrived - v_b);
                s.peak = s.peak.max(f.abs());
                if s.next.is_some() {
                    // A new keyboard waits: fade out, then take it (`control`).
                    sum += f * s.fade;
                    s.fade = (s.fade - 1.0 / FADE as f64).max(0.0);
                } else {
                    sum += f;
                }
            }
            *o += sum;
            self.phase += 1;
            if self.phase == super::voice::CONTROL {
                self.phase = 0;
            }
        }
    }

    /// Every control step: the dampers glide, the loss filters follow, a quiet string is
    /// let go (and takes a waiting layout) and a driven one woken.
    fn control(&mut self, driven: bool) {
        // The high-pass's tail, once nothing drives it, ends exactly.
        if !driven && self.hp_x == 0.0 && self.hp_y.abs() < 1.0e-15 {
            self.hp_y = 0.0;
        }
        let k = self.glide;
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
            if s.next.is_some() && s.fade <= 0.0 {
                // Faded out: silent, it takes the new layout (and wakes again if driven).
                s.silence();
                continue;
            }
            if s.contact != s.set_contact {
                // The bridge takes half the first partial's loss, as a voice's does: its
                // reflection `(1 − YZ)/(1 + YZ)` keeps the loop under unity at every
                // frequency, DC too (the loss filter alone is capped at 1 there).
                let rate = 6.907_755 / super::voice::damped_t60(s.key.key as f64);
                let fs = self.fs;
                let (delay, frac, n, poles) = (s.line.len_now(), s.frac, s.sections, s.poles);
                // The loop's own trip at each partial: its group delay there.
                let trip = |k: u32| {
                    let w = std::f64::consts::TAU * s.key.partial(k) / fs;
                    let mut d = delay as f64 + 1.0 + super::design::first_delay(w, frac);
                    for (r, t) in &poles[..n] {
                        d += super::design::second_delay(w, *r, *t);
                    }
                    d / fs
                };
                let (g, a, yz) = super::string::loss_timed(
                    &s.key,
                    fs,
                    0.5,
                    self.decay,
                    s.contact * rate,
                    0.0,
                    &trip,
                );
                s.g = g * (1.0 - yz) / (1.0 + yz);
                s.alpha = a;
                s.yz = yz;
                s.set_contact = s.contact;
            }
            if s.peak < 1.0e-10 && !driven {
                s.quiet += 1;
                if s.quiet >= 64 {
                    s.silence();
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
    let mut z = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x500B_0A2D;
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
    /// The radiation's low-pass (per channel, subtracted).
    rad: [f64; 2],
    rad_a: f64,
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
            rad: [0.0; 2],
            rad_a: math::exp(-std::f64::consts::TAU * RADIATION_HZ / fs),
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
        self.rad = [0.0; 2];
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
            && self.rad == [0.0; 2]
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
            self.rad[0],
            self.rad[1],
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
            f(&mut self.rad[c]);
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
        let gains = self.damp_g.iter().zip(&self.damp_a);
        for (((d, line), h), (g, a)) in self
            .damp
            .iter_mut()
            .zip(self.lines.iter_mut())
            .zip(h)
            .zip(gains)
        {
            let fb = 0.5 * h * g;
            *d = fb + a * (*d - fb);
            line.write(*d + 0.25 * hp);
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
            let o = y + self.shelf * high + body * (low + 0.5 * fdn);
            self.rad[c] = o + self.rad_a * (self.rad[c] - o);
            out[c] = o - RADIATION_CUT * self.rad[c];
        }
        (out[0], out[1])
    }
}

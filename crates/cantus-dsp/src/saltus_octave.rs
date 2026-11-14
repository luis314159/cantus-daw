//! Saltus's analog octaver: **a divider that follows the note**, with no latency.
//!
//! An analog octaver (Boss's OC-2, its many descendants) squares the note up with a
//! comparator and halves it with flip-flops; what it gets wrong is the comparator — a
//! strong second harmonic or a pick's noise crosses twice, and the octave *glitches*. Here:
//!
//! 1. **Conditioning.** A DC blocker, then a four-pole low-pass kept just above the note's
//!    fundamental (1.2 × f0): the pitch tracker's f0 when it hears a note, else the
//!    divider's own period once two cycles agree, else 400 Hz. The tracker only *guides*
//!    the filter — nothing waits for it.
//! 2. **The comparator** with hysteresis at a fifth of the fundamental's peak (a follower
//!    of the filtered signal), and a refractory time of 0.6 of the period guided: an edge
//!    sooner than that is the second harmonic's, ignored.
//! 3. **The sub-octaves** are sines whose phase is *locked* to the edges: at every rising
//!    edge, one octave down must have advanced half a cycle and two down a quarter, and the
//!    rate over the next period is set to land there (a phase-locked loop of one period).
//!    So each is a clean, round octave with the note's timing, never a buzzing square. Their
//!    level is the fundamental's peak over the last period, glided (3 ms up, 20 ms down).
//! 4. **The octave up** is the filtered fundamental rectified (full wave: twice the
//!    frequency), its DC taken out, as the Octavia did.
//!
//! **At rest**: every filter state flushed to +0.0 once silence has taken it under
//! 10⁻¹⁵, the follower and the level at zero, the edge count, the period and the guide
//! cleared — a new divider's state.

use crate::math;
use crate::pitch::PitchFrame;

/// States under this are flushed to +0.0 while the input is silent (−300 dB).
const FLUSH: f32 = 1e-15;
/// The low-pass sits this far above the fundamental.
const CUT_OVER_F0: f32 = 1.2;
/// The low-pass with no note known, Hz.
const CUT_DEFAULT: f32 = 400.0;
/// The low-pass's range, Hz.
const CUT_MIN: f32 = 30.0;
const CUT_MAX: f32 = 2_000.0;
/// The comparator's hysteresis, as a share of the fundamental's peak.
const HYSTERESIS: f32 = 0.2;
/// An edge sooner than this share of the period guided is a harmonic's.
const REFRACTORY: f32 = 0.6;
/// The follower's release, seconds.
const FOLLOW_S: f32 = 0.03;
/// Below this peak the comparator does not move (−80 dBFS).
const GATE: f32 = 1e-4;
/// The sub's level glides up and down over these, seconds.
const ATTACK_S: f32 = 0.003;
const RELEASE_S: f32 = 0.02;
/// The note has ended when no edge came for this many periods.
const LOST_PERIODS: f32 = 3.0;
/// A guide from the tracker is kept this many frames after it stops hearing a note.
const GUIDE_FRAMES: u32 = 100;
/// The octave up's DC blocker, Hz, and its gain (the rectified sine's second harmonic is
/// 4/3π of its peak).
const UP_HZ: f32 = 40.0;
const UP_GAIN: f32 = 2.36;

/// One sample of the octaves, mono.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Octaves {
    pub sub1: f32,
    pub sub2: f32,
    pub up1: f32,
}

/// A two-pole TPT state-variable low-pass (Zavalishin), Butterworth.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Lp2 {
    ic1: f32,
    ic2: f32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct LpCoef {
    a1: f32,
    a2: f32,
    a3: f32,
}

impl LpCoef {
    fn new(hz: f32, sr: f32) -> Self {
        let g = math::tan(core::f32::consts::PI * (hz / sr).min(0.45));
        let k = core::f32::consts::SQRT_2;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        LpCoef { a1, a2, a3: g * a2 }
    }
}

impl Lp2 {
    #[inline]
    fn tick(&mut self, x: f32, c: &LpCoef) -> f32 {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        v2
    }

    #[inline]
    fn flush(&mut self) {
        if self.ic1.abs() < FLUSH && self.ic2.abs() < FLUSH {
            self.ic1 = 0.0;
            self.ic2 = 0.0;
        }
    }
}

/// A one-pole DC blocker (`y = x − x₋₁ + R·y₋₁`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Dc {
    x1: f32,
    y1: f32,
}

impl Dc {
    #[inline]
    fn tick(&mut self, x: f32, r: f32) -> f32 {
        let y = x - self.x1 + r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    #[inline]
    fn flush(&mut self) {
        if self.x1.abs() < FLUSH && self.y1.abs() < FLUSH {
            self.x1 = 0.0;
            self.y1 = 0.0;
        }
    }
}

/// The divider (see the module docs). Holds no buffer: nothing to allocate.
#[derive(Clone, Debug)]
pub struct Divider {
    sr: f32,
    r_dc: f32,
    r_up: f32,
    rel: f32,
    k_attack: f32,
    k_release: f32,
    dc: Dc,
    lp: [Lp2; 2],
    coef: LpCoef,
    cut: f32,
    /// The follower of the filtered signal's peak.
    follow: f32,
    high: bool,
    prev: f32,
    /// Samples since the last rising edge (fractional), and the period measured (0: none).
    since: f32,
    period: f32,
    /// The tracker's period while it hears a note, and frames since it last did.
    guide: f32,
    guide_age: u32,
    edges: u32,
    phase: [f32; 2],
    rate: [f32; 2],
    /// The filtered signal's peak since the last rising edge.
    peak: f32,
    amp: f32,
    target: f32,
    up_dc: [Dc; 2],
}

impl Divider {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let tau = |s: f32| 1.0 - math::exp(-1.0 / (s * sr));
        Divider {
            sr,
            r_dc: math::exp(-2.0 * core::f32::consts::PI * 20.0 / sr),
            r_up: math::exp(-2.0 * core::f32::consts::PI * UP_HZ / sr),
            rel: math::exp(-1.0 / (FOLLOW_S * sr)),
            k_attack: tau(ATTACK_S),
            k_release: tau(RELEASE_S),
            dc: Dc::default(),
            lp: [Lp2::default(); 2],
            coef: LpCoef::new(CUT_DEFAULT, sr),
            cut: CUT_DEFAULT,
            follow: 0.0,
            high: false,
            prev: 0.0,
            since: 0.0,
            period: 0.0,
            guide: 0.0,
            guide_age: 0,
            edges: 0,
            phase: [0.0; 2],
            rate: [0.0; 2],
            peak: 0.0,
            amp: 0.0,
            target: 0.0,
            up_dc: [Dc::default(); 2],
        }
    }

    pub fn reset(&mut self) {
        *self = Divider::new(self.sr);
    }

    /// No note under way, every state a new divider's (the filter's cutoff included).
    pub fn at_rest(&self) -> bool {
        self.fresh()
            && self.guide == 0.0
            && self.guide_age == 0
            && self.cut == CUT_DEFAULT
            && self.dc == Dc::default()
            && self.lp == [Lp2::default(); 2]
            && self.up_dc == [Dc::default(); 2]
            && self.follow == 0.0
            && self.peak == 0.0
            && self.prev == 0.0
            && !self.high
    }

    /// No note being followed: the counters a new divider's.
    fn fresh(&self) -> bool {
        self.edges == 0
            && self.period == 0.0
            && self.amp == 0.0
            && self.target == 0.0
            && self.since == 0.0
    }

    /// A frame from the tracker: the note's period guides the filter.
    pub fn guide(&mut self, f: &PitchFrame) {
        if f.voiced && f.period > 0.0 {
            self.guide = f.period;
            self.guide_age = 0;
        } else if self.guide > 0.0 {
            self.guide_age += 1;
            if self.guide_age >= GUIDE_FRAMES {
                self.guide = 0.0;
                self.guide_age = 0;
            }
        }
        self.retune();
    }

    /// The filter's cutoff for what is known of the note.
    fn retune(&mut self) {
        let period = if self.guide > 0.0 {
            self.guide
        } else {
            self.period
        };
        let want = if period > 0.0 {
            (CUT_OVER_F0 * self.sr / period).clamp(CUT_MIN, CUT_MAX)
        } else {
            CUT_DEFAULT
        };
        if want != self.cut {
            // Halfway each frame: smooth enough for a filter that is never heard alone.
            let next = self.cut + 0.5 * (want - self.cut);
            self.cut = if (next - want).abs() < 0.5 {
                want
            } else {
                next
            };
            self.coef = LpCoef::new(self.cut, self.sr);
        }
    }

    /// The period the refractory time and a first sub start from.
    fn known_period(&self) -> f32 {
        if self.guide > 0.0 {
            self.guide
        } else {
            self.period
        }
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> Octaves {
        let d = self.dc.tick(x, self.r_dc);
        let c0 = self.lp[0].tick(d, &self.coef);
        let c = self.lp[1].tick(c0, &self.coef);
        let a = c.abs();
        self.follow = if a > self.follow {
            a
        } else {
            self.follow * self.rel
        };
        if a > self.peak {
            self.peak = a;
        }
        let running = !self.fresh() || self.follow > GATE;
        if running {
            self.since += 1.0;
            self.comparator(c);
            self.advance();
        }
        // The octave up: the fundamental rectified, its DC out.
        let u0 = self.up_dc[0].tick(a, self.r_up);
        let u = self.up_dc[1].tick(u0, self.r_up) * UP_GAIN;
        if x == 0.0 {
            self.flush();
        }
        self.prev = c;
        let s1 = self.amp * math::sin(2.0 * core::f32::consts::PI * self.phase[0]);
        let s2 = self.amp * math::sin(2.0 * core::f32::consts::PI * self.phase[1]);
        Octaves {
            sub1: s1,
            sub2: s2,
            up1: u,
        }
    }

    fn flush(&mut self) {
        self.dc.flush();
        self.lp[0].flush();
        self.lp[1].flush();
        self.up_dc[0].flush();
        self.up_dc[1].flush();
        if self.follow < FLUSH {
            self.follow = 0.0;
        }
        if self.prev.abs() < FLUSH {
            self.prev = 0.0;
        }
        if self.peak < FLUSH {
            self.peak = 0.0;
        }
    }

    #[inline]
    fn comparator(&mut self, c: f32) {
        let h = HYSTERESIS * self.follow;
        if self.follow <= GATE {
            return;
        }
        let p = self.known_period();
        let refractory = if p > 0.0 {
            REFRACTORY * p
        } else {
            self.sr / 2_000.0
        };
        if !self.high {
            if c > h && self.since >= refractory {
                // Where between the two samples the threshold was crossed.
                let frac = ((c - h) / (c - self.prev).max(1e-20)).clamp(0.0, 1.0);
                self.high = true;
                self.rising(frac);
            }
        } else if c < -h {
            self.high = false;
        }
    }

    /// A rising edge, `frac` of a sample ago.
    fn rising(&mut self, frac: f32) {
        let measured = self.since - frac;
        let first = self.edges == 0;
        let plausible = !first && measured > self.sr / 2_000.0 && measured < self.sr / 25.0;
        let had = self.period > 0.0;
        if plausible {
            self.period = if had && (measured / self.period - 1.0).abs() < 0.15 {
                self.period + 0.5 * (measured - self.period)
            } else {
                measured
            };
        }
        self.since = frac;
        self.edges = self.edges.wrapping_add(1) & 3;
        let p = self.known_period();
        if p > 0.0 {
            self.target = self.peak;
            for k in 0..2 {
                let div = (2 << k) as f32;
                let here = (self.edges % (2 << k)) as f32 / div;
                if !had || self.amp == 0.0 {
                    // A note starting: on the beat, at the nominal rate.
                    self.phase[k] = here;
                    self.rate[k] = 1.0 / (div * p);
                } else {
                    let next = here + 1.0 / div;
                    let mut d = next - self.phase[k];
                    // Wrapped into −½ … ½ of a cycle.
                    d -= (d + 0.5).floor();
                    let nominal = 1.0 / (div * p);
                    self.rate[k] = (d / p).clamp(0.25 * nominal, 2.0 * nominal);
                }
            }
        }
        self.peak = 0.0;
    }

    /// The sub-octaves' phases and level, one sample on.
    #[inline]
    fn advance(&mut self) {
        let p = self.known_period();
        let lost = p > 0.0 && self.since > LOST_PERIODS * p + 0.02 * self.sr;
        if lost || self.follow <= GATE || self.edges == 0 && self.period == 0.0 {
            self.target = 0.0;
        }
        for k in 0..2 {
            let v = self.phase[k] + self.rate[k];
            self.phase[k] = v - v.floor();
        }
        if self.amp != self.target {
            let k = if self.target > self.amp {
                self.k_attack
            } else {
                self.k_release
            };
            let v = self.amp + (self.target - self.amp) * k;
            self.amp = if (self.target - v).abs() < 1e-7 || v == self.amp {
                self.target
            } else {
                v
            };
        }
        if self.amp == 0.0 && self.target == 0.0 && (lost || self.follow <= GATE) {
            // The note is over: back to a new divider's counters.
            self.edges = 0;
            self.period = 0.0;
            self.since = 0.0;
            self.high = false;
            self.phase = [0.0; 2];
            self.rate = [0.0; 2];
            self.peak = 0.0;
        }
    }

    /// The note followed, MIDI against `a4`, while a sub sounds.
    pub fn detected(&self, a4: f32) -> Option<f32> {
        (self.period > 0.0 && self.amp > 0.0)
            .then(|| 69.0 + 12.0 * math::log2(self.sr / self.period / a4.max(1.0)))
    }

    /// The octaves' levels now: the subs' (one, two down) and the follower's (up).
    pub fn levels(&self) -> [f32; 3] {
        [self.amp, self.amp, self.follow]
    }
}

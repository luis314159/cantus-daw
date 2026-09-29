//! The vibrato and chorus scanner.
//!
//! On the instrument a lumped LC delay line is tapped at intervals, and a motor-driven
//! capacitive scanner sweeps its pickup across the taps, up the line and back, about seven
//! times a second. Its sound comes from that *stepped* reading — the pickup crossfades from
//! one tap to the next, it does not slide a delay smoothly — and from the line's low-pass
//! character, deeper taps being duller.
//!
//! Here: a delay line read at [`TAPS`] fixed taps [`TAP_MS`] apart; the scanner position
//! is a triangle over one revolution, spanning the first 3, 6 or all 10 taps for V1, V2 and
//! V3 (depths 0.22, 0.55 and 0.99 ms, in the ratio 1 : 2.5 : 4.5 the documentation gives
//! for the instrument); between two taps the pickup crossfades with a smoothstep, the
//! capacitive plates' overlap. A one-pole low-pass that closes as the pickup moves deeper
//! stands for the LC sections. C1–C3 mix the scanned signal with the dry one, equal
//! power. The scanner's phase is a 32-bit accumulator counted from the start of the
//! stream, so it is block-size independent.

use super::filters::OnePole;

/// Taps along the line.
pub const TAPS: usize = 10;
/// Delay between taps, ms.
pub const TAP_MS: f32 = 0.11;
/// Taps each vibrato depth spans (V1, V2, V3).
pub const SPAN: [usize; 3] = [3, 6, 10];

/// The six positions of the vibrato switch.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum VibMode {
    V1,
    V2,
    V3,
    C1,
    C2,
    #[default]
    C3,
}

impl VibMode {
    pub const ALL: [VibMode; 6] = [
        VibMode::V1,
        VibMode::V2,
        VibMode::V3,
        VibMode::C1,
        VibMode::C2,
        VibMode::C3,
    ];

    pub fn from_index(i: f32) -> VibMode {
        Self::ALL[(i.round().max(0.0) as usize).min(5)]
    }

    /// 0, 1 or 2: how deep.
    pub fn depth(self) -> usize {
        match self {
            VibMode::V1 | VibMode::C1 => 0,
            VibMode::V2 | VibMode::C2 => 1,
            VibMode::V3 | VibMode::C3 => 2,
        }
    }

    pub fn chorus(self) -> bool {
        matches!(self, VibMode::C1 | VibMode::C2 | VibMode::C3)
    }

    pub fn label(self) -> &'static str {
        ["V1", "V2", "V3", "C1", "C2", "C3"][self as usize]
    }
}

const LINE: usize = 1024;

#[derive(Clone, Debug)]
pub struct Scanner {
    line: Box<[f32; LINE]>,
    pos: usize,
    sr: f32,
    tap: [f32; TAPS],
    pub phase: u32,
    inc: u32,
    mode: VibMode,
    lp: OnePole,
    /// Where the pickup is now, 0…1 of the line (for the editor).
    pub position: f32,
}

impl Scanner {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let mut tap = [0.0; TAPS];
        for (i, t) in tap.iter_mut().enumerate() {
            *t = (i as f32 * TAP_MS * 0.001 * sr).min((LINE - 4) as f32);
        }
        let mut s = Scanner {
            line: Box::new([0.0; LINE]),
            pos: 0,
            sr,
            tap,
            phase: 0,
            inc: 0,
            mode: VibMode::C3,
            lp: OnePole::default(),
            position: 0.0,
        };
        s.set(VibMode::C3, 7.0);
        s
    }

    pub fn set(&mut self, mode: VibMode, rate_hz: f32) {
        self.mode = mode;
        self.inc = (rate_hz.max(0.0) as f64 / self.sr as f64 * 4_294_967_296.0).round() as u32;
    }

    pub fn clear(&mut self) {
        self.line.fill(0.0);
        self.lp.reset();
    }

    /// Advance the scanner `n` samples in silence.
    pub fn skip(&mut self, n: usize) {
        self.phase = self.phase.wrapping_add(self.inc.wrapping_mul(n as u32));
    }

    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let d = delay.max(0.0);
        let i = d as usize;
        let f = d - i as f32;
        let a = self.line[(self.pos + LINE - i) & (LINE - 1)];
        let b = self.line[(self.pos + LINE - i - 1) & (LINE - 1)];
        a + (b - a) * f
    }

    /// One sample in; the scanned (vibrato) or mixed (chorus) sample out.
    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        self.pos = (self.pos + 1) & (LINE - 1);
        self.line[self.pos] = x;
        self.phase = self.phase.wrapping_add(self.inc);
        // Triangle 0 → 1 → 0 over one revolution.
        let u = self.phase as f32 * (1.0 / 4_294_967_296.0);
        let tri = if u < 0.5 { 2.0 * u } else { 2.0 - 2.0 * u };
        let span = SPAN[self.mode.depth()];
        let p = tri * (span - 1) as f32;
        self.position = p / (TAPS - 1) as f32;
        let i = (p as usize).min(span - 1);
        let j = (i + 1).min(span - 1);
        let f = p - i as f32;
        let w = f * f * (3.0 - 2.0 * f);
        let scanned = self.read(self.tap[i]) * (1.0 - w) + self.read(self.tap[j]) * w;
        // Deeper in the line is duller: 9 kHz at the first tap to 5 kHz at the last.
        let hz = 9_000.0 - 4_000.0 * self.position;
        self.lp.a = super::filters::one_pole_coef(self.sr, hz);
        let wet = self.lp.run(scanned);
        if self.mode.chorus() {
            std::f32::consts::FRAC_1_SQRT_2 * (x + wet)
        } else {
            wet
        }
    }
}

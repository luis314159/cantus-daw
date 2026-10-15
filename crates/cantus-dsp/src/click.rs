//! The metronome's click: a short burst per beat, the downbeat accented.
//!
//! Three voices (schema 26, ADR 0052): **Classic** — an exponentially decaying sine, higher
//! on the downbeat, exactly as every earlier version played it (a test holds it to the
//! bit); **Wood** — two damped modes struck at once, a woodblock; **Beep** — a soft-edged
//! square tone with a flat top and a short release. The accent can be switched off: every
//! beat then sounds as an ordinary one. Fixed cost per sample, no allocation.

use crate::math;

/// The click's voice.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClickVoice {
    #[default]
    Classic,
    Wood,
    Beep,
}

impl ClickVoice {
    pub const ALL: [ClickVoice; 3] = [ClickVoice::Classic, ClickVoice::Wood, ClickVoice::Beep];
}

#[derive(Clone, Debug)]
pub struct Click {
    sample_rate: f32,
    phase: f32,
    inc: f32,
    env: f32,
    decay: f32,
    gain: f32,
    voice: ClickVoice,
    accent: bool,
    /// Wood's second mode.
    phase2: f32,
    inc2: f32,
    env2: f32,
    decay2: f32,
    /// Beep: samples since the strike, and how long the attack and the flat top last.
    age: u32,
    attack: u32,
    hold: u32,
}

impl Click {
    pub fn new(sample_rate: f32) -> Self {
        Click {
            sample_rate,
            phase: 0.0,
            inc: 0.0,
            env: 0.0,
            decay: 0.0,
            gain: 0.5,
            voice: ClickVoice::Classic,
            accent: true,
            phase2: 0.0,
            inc2: 0.0,
            env2: 0.0,
            decay2: 0.0,
            age: 0,
            attack: 1,
            hold: 0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain.clamp(0.0, 1.0);
    }

    /// The voice of the next click (one already sounding finishes in its own).
    pub fn set_voice(&mut self, voice: ClickVoice) {
        if voice != self.voice {
            // Only Wood has a second mode: another voice must not keep it "ringing".
            self.env2 = 0.0;
        }
        self.voice = voice;
    }

    /// Whether the first beat of a bar is accented.
    pub fn set_accent(&mut self, on: bool) {
        self.accent = on;
    }

    /// Start a click. `downbeat` = first beat of the bar (accented when the accent is on).
    pub fn trigger(&mut self, downbeat: bool) {
        let accent = downbeat && self.accent;
        let sr = self.sample_rate;
        self.phase = 0.0;
        self.env2 = 0.0;
        match self.voice {
            ClickVoice::Classic => {
                let hz = if accent { 1_760.0 } else { 1_174.7 };
                self.inc = hz / sr;
                self.env = if accent { 1.0 } else { 0.7 };
                // ~25 ms to −60 dB.
                self.decay = math::exp(-6.9 / (0.025 * sr));
            }
            ClickVoice::Wood => {
                // A woodblock's two strongest modes (about 1 : 1.6), higher when accented;
                // the upper one dies first, as wood's does.
                let (f1, f2) = if accent {
                    (1_850.0, 2_960.0)
                } else {
                    (1_350.0, 2_160.0)
                };
                let level = if accent { 1.0 } else { 0.7 };
                self.inc = f1 / sr;
                self.inc2 = f2 / sr;
                self.phase2 = 0.0;
                self.env = level * 0.72;
                self.env2 = level * 0.28;
                self.decay = math::exp(-6.9 / (0.014 * sr));
                self.decay2 = math::exp(-6.9 / (0.007 * sr));
            }
            ClickVoice::Beep => {
                let hz = if accent { 2_000.0 } else { 1_000.0 };
                self.inc = hz / sr;
                self.env = if accent { 1.0 } else { 0.7 };
                // 1 ms rise, 28 ms flat, then ~10 ms to −60 dB: no click of its own.
                self.attack = ((0.001 * sr) as u32).max(1);
                self.hold = (0.028 * sr) as u32;
                self.age = 0;
                self.decay = math::exp(-6.9 / (0.010 * sr));
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.env > 1e-4 || self.env2 > 1e-4
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.voice {
            ClickVoice::Classic => self.next_classic(),
            ClickVoice::Wood => self.next_wood(),
            ClickVoice::Beep => self.next_beep(),
        }
    }

    /// The classic voice, as it always was.
    #[inline]
    fn next_classic(&mut self) -> f32 {
        if self.env <= 1e-4 {
            self.env = 0.0;
            return 0.0;
        }
        let s = math::sin(self.phase * std::f32::consts::TAU) * self.env * self.gain;
        self.phase = (self.phase + self.inc).rem_euclid(1.0);
        self.env *= self.decay;
        s
    }

    #[inline]
    fn next_wood(&mut self) -> f32 {
        if self.env <= 1e-4 && self.env2 <= 1e-4 {
            self.env = 0.0;
            self.env2 = 0.0;
            return 0.0;
        }
        let tau = std::f32::consts::TAU;
        let s = (math::sin(self.phase * tau) * self.env + math::sin(self.phase2 * tau) * self.env2)
            * self.gain;
        self.phase = (self.phase + self.inc).rem_euclid(1.0);
        self.phase2 = (self.phase2 + self.inc2).rem_euclid(1.0);
        self.env *= self.decay;
        self.env2 *= self.decay2;
        s
    }

    #[inline]
    fn next_beep(&mut self) -> f32 {
        if self.env <= 1e-4 {
            self.env = 0.0;
            return 0.0;
        }
        // A soft-edged square: tanh of a driven sine, normalised to ±1.
        let drive = 3.0f32;
        let tone = math::tanh(drive * math::sin(self.phase * std::f32::consts::TAU))
            / math::tanh(drive);
        let rise = (self.age as f32 / self.attack as f32).min(1.0);
        let s = tone * self.env * rise * self.gain;
        self.phase = (self.phase + self.inc).rem_euclid(1.0);
        if self.age >= self.attack + self.hold {
            self.env *= self.decay;
        }
        self.age = self.age.saturating_add(1);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// The click as every version before schema 26 played it, kept here as the reference:
    /// the classic voice with its accent must stay this, to the bit.
    struct Legacy {
        phase: f32,
        inc: f32,
        env: f32,
        decay: f32,
        gain: f32,
    }

    impl Legacy {
        fn trigger(&mut self, accent: bool) {
            let hz = if accent { 1_760.0 } else { 1_174.7 };
            self.inc = hz / SR;
            self.phase = 0.0;
            self.env = if accent { 1.0 } else { 0.7 };
            self.decay = math::exp(-6.9 / (0.025 * SR));
        }
        fn next(&mut self) -> f32 {
            if self.env <= 1e-4 {
                self.env = 0.0;
                return 0.0;
            }
            let s = math::sin(self.phase * std::f32::consts::TAU) * self.env * self.gain;
            self.phase = (self.phase + self.inc).rem_euclid(1.0);
            self.env *= self.decay;
            s
        }
    }

    fn render(c: &mut Click, accent: bool, n: usize) -> Vec<f32> {
        c.trigger(accent);
        (0..n).map(|_| c.next()).collect()
    }

    #[test]
    fn the_classic_accented_click_is_the_old_one_to_the_bit() {
        for gain in [0.5f32, 0.5 * 0.25, 0.9] {
            let mut c = Click::new(SR);
            c.set_gain(gain);
            let mut old = Legacy {
                phase: 0.0,
                inc: 0.0,
                env: 0.0,
                decay: 0.0,
                gain,
            };
            for accent in [true, false, true] {
                old.trigger(accent);
                let want: Vec<u32> = (0..4_000).map(|_| old.next().to_bits()).collect();
                let got: Vec<u32> = render(&mut c, accent, 4_000)
                    .into_iter()
                    .map(f32::to_bits)
                    .collect();
                assert_eq!(got, want, "gain {gain}, accent {accent}");
            }
        }
    }

    #[test]
    fn without_the_accent_the_downbeat_sounds_as_any_beat() {
        for voice in ClickVoice::ALL {
            let mut c = Click::new(SR);
            c.set_voice(voice);
            c.set_accent(false);
            let down = render(&mut c, true, 3_000);
            let beat = render(&mut c, false, 3_000);
            assert_eq!(down, beat, "{voice:?}");
            c.set_accent(true);
            assert_ne!(render(&mut c, true, 3_000), beat, "{voice:?} accented");
        }
    }

    /// Every voice: bounded by its gain, silent (inactive) within 80 ms, deterministic, and
    /// each audibly its own.
    #[test]
    fn every_voice_is_short_bounded_and_its_own() {
        let mut seen: Vec<Vec<f32>> = Vec::new();
        for voice in ClickVoice::ALL {
            let mut c = Click::new(SR);
            c.set_voice(voice);
            c.set_gain(0.5);
            for accent in [true, false] {
                let a = render(&mut c, accent, 4_800);
                let b = render(&mut c, accent, 4_800);
                assert_eq!(a, b, "{voice:?}: deterministic");
                let peak = a.iter().fold(0f32, |m, x| m.max(x.abs()));
                assert!(peak > 0.05 && peak <= 0.5 + 1e-6, "{voice:?} peak {peak}");
                assert!(a.iter().all(|x| x.is_finite()));
                c.trigger(accent);
                for _ in 0..(0.08 * SR) as usize {
                    c.next();
                }
                assert!(!c.is_active(), "{voice:?} rings past 80 ms");
                if accent {
                    seen.push(a);
                }
            }
        }
        assert_ne!(seen[0], seen[1]);
        assert_ne!(seen[1], seen[2]);
        assert_ne!(seen[0], seen[2]);
    }
}

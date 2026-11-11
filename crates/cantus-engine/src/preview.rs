//! The browser's audition voice: one sample played once, straight to the output after
//! the master strip (like the click, it is for the person at the desk — the master
//! fader does not turn it down and the recorder never hears it).
//!
//! Real-time rules (ADR 0005): the sample arrives as an `Arc` in a message; a sample the
//! voice lets go of is handed back to the interface to be dropped there, never freed on
//! the audio thread. Playback reads with linear interpolation at the file's own rate —
//! an audition, not a render.

use cantus_core::SampleData;
use std::sync::Arc;

/// Frames of fade at a start and at a stop, so neither clicks.
const FADE: u32 = 64;

/// How long a new level takes to arrive while the audition plays: a straight ramp, so a
/// level dragged as the sound plays never clicks. // r39:fixes
const LEVEL_RAMP_SECONDS: f64 = 0.010;

#[derive(Clone, Debug)]
pub struct Preview {
    sample: Option<Arc<SampleData>>,
    /// Read position in the sample's frames.
    pos: f64,
    /// Sample frames per output frame.
    step: f64,
    gain: f32,
    /// The level a ramp goes to, its step per frame and the frames it has left. // r39:fixes
    target: f32,
    ramp_step: f32,
    ramp_left: u32,
    /// Frames into the fade-in (counts up to `FADE`).
    attack: u32,
    /// Frames left of the fade-out; `None` while not stopping.
    release: Option<u32>,
    out_rate: f64,
}

impl Preview {
    pub fn new(out_rate: f64) -> Preview {
        Preview {
            sample: None,
            pos: 0.0,
            step: 1.0,
            gain: 1.0,
            target: 1.0, // r39:fixes
            ramp_step: 0.0,
            ramp_left: 0,
            attack: 0,
            release: None,
            out_rate: out_rate.max(1.0),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.sample.is_some()
    }

    /// Play `s` from its start at `gain`; returns the sample it replaces, which the
    /// caller hands back as garbage.
    pub fn start(&mut self, s: Arc<SampleData>, gain: f32) -> Option<Arc<SampleData>> {
        self.step = s.rate.max(1) as f64 / self.out_rate;
        self.pos = 0.0;
        self.gain = gain;
        self.target = gain; // r39:fixes
        self.ramp_left = 0;
        self.attack = 0;
        self.release = None;
        self.sample.replace(s)
    }

    /// The audition's level from now on, without starting it again: a ramp of
    /// [`LEVEL_RAMP_SECONDS`] while it plays, at once while nothing does. The level it
    /// already has changes nothing. Real-time safe. // r39:fixes
    pub fn set_gain(&mut self, gain: f32) {
        if self.sample.is_none() {
            self.gain = gain;
            self.target = gain;
            self.ramp_left = 0;
            return;
        }
        if gain == self.target {
            return;
        }
        let frames = ((self.out_rate * LEVEL_RAMP_SECONDS) as u32).max(1);
        self.target = gain;
        self.ramp_step = (gain - self.gain) / frames as f32;
        self.ramp_left = frames;
    }

    /// The level now (the ramp's, while one runs).
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Fade out and let go.
    pub fn stop(&mut self) {
        if self.sample.is_some() && self.release.is_none() {
            self.release = Some(FADE);
        }
    }

    /// Add the voice to `frames` frames of interleaved `out`. Returns the sample when it
    /// has finished (to hand back), `None` otherwise.
    pub fn mix(
        &mut self,
        out: &mut [f32],
        channels: usize,
        frames: usize,
    ) -> Option<Arc<SampleData>> {
        let s = self.sample.as_ref()?;
        let len = s.len_frames();
        let src_ch = s.channels.max(1) as usize;
        let data = &s.frames;
        let mut finished = false;
        for i in 0..frames {
            let idx = self.pos as usize;
            if idx + 1 >= len.max(1) {
                finished = true;
                break;
            }
            // r39:fixes — a new level ramps in.
            if self.ramp_left > 0 {
                self.ramp_left -= 1;
                self.gain = if self.ramp_left == 0 {
                    self.target
                } else {
                    self.gain + self.ramp_step
                };
            }
            let mut env = self.gain;
            if self.attack < FADE {
                env *= self.attack as f32 / FADE as f32;
                self.attack += 1;
            }
            if let Some(r) = self.release.as_mut() {
                if *r == 0 {
                    finished = true;
                    break;
                }
                env *= *r as f32 / FADE as f32;
                *r -= 1;
            }
            let frac = (self.pos - idx as f64) as f32;
            let at = |f: usize, c: usize| data[f * src_ch + c.min(src_ch - 1)];
            let l = at(idx, 0) + (at(idx + 1, 0) - at(idx, 0)) * frac;
            let r = if src_ch > 1 {
                at(idx, 1) + (at(idx + 1, 1) - at(idx, 1)) * frac
            } else {
                l
            };
            let frame = &mut out[i * channels..(i + 1) * channels];
            frame[0] += l * env;
            if channels > 1 {
                frame[1] += r * env;
            }
            self.pos += self.step;
        }
        if finished {
            self.release = None;
            return self.sample.take();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, frames: usize) -> Arc<SampleData> {
        Arc::new(SampleData {
            stream: None,
            rate,
            channels: 1,
            frames: (0..frames)
                .map(|i| cantus_dsp::math::sin(i as f32 * 0.01))
                .collect::<Vec<_>>()
                .into(),
        })
    }

    #[test]
    fn plays_once_then_hands_the_sample_back() {
        let mut p = Preview::new(48_000.0);
        assert!(p.start(tone(48_000, 1000), 1.0).is_none());
        let mut out = vec![0.0f32; 2 * 512];
        assert!(p.mix(&mut out, 2, 512).is_none());
        assert!(out.iter().any(|s| s.abs() > 0.1));
        assert_eq!(out[0], 0.0, "the start fades in");
        let mut out = vec![0.0f32; 2 * 512];
        assert!(
            p.mix(&mut out, 2, 512).is_some(),
            "the finished sample comes back"
        );
        assert!(!p.is_playing());
    }

    #[test]
    fn stop_fades_and_lets_go() {
        let mut p = Preview::new(44_100.0);
        p.start(tone(44_100, 100_000), 1.0);
        let mut out = vec![0.0f32; 2 * 256];
        p.mix(&mut out, 2, 256);
        p.stop();
        let mut out = vec![0.0f32; 2 * 256];
        assert!(p.mix(&mut out, 2, 256).is_some());
        assert!(
            out[2 * 200..].iter().all(|s| *s == 0.0),
            "silent after the fade"
        );
    }

    /// r39:fixes — a new level while it plays: the sound goes on from where it was, the
    /// level ramps to the new one, and the level it had changes no sample.
    #[test]
    fn a_new_level_ramps_in_without_starting_again() {
        let s = tone(48_000, 100_000);
        let mut a = Preview::new(48_000.0);
        let mut b = Preview::new(48_000.0);
        a.start(s.clone(), 0.5);
        b.start(s, 0.5);
        let mut oa = vec![0.0f32; 2 * 256];
        let mut ob = vec![0.0f32; 2 * 256];
        a.mix(&mut oa, 2, 256);
        b.mix(&mut ob, 2, 256);
        // The same level again: not a bit moves.
        b.set_gain(0.5);
        for _ in 0..4 {
            let mut oa = vec![0.0f32; 2 * 256];
            let mut ob = vec![0.0f32; 2 * 256];
            a.mix(&mut oa, 2, 256);
            b.mix(&mut ob, 2, 256);
            assert_eq!(oa, ob);
        }
        let pos = b.pos;
        b.set_gain(0.25);
        assert_eq!(b.pos, pos, "not started again");
        let mut oa = vec![0.0f32; 2 * 1024];
        let mut ob = vec![0.0f32; 2 * 1024];
        a.mix(&mut oa, 2, 1024);
        b.mix(&mut ob, 2, 1024);
        // The first frame already on its way down, never a jump: within one step of 0.5.
        let ratio = |i: usize| ob[2 * i] / oa[2 * i];
        let step = 0.25 / 480.0;
        assert!((ratio(0) - (0.5 - step) / 0.5).abs() < 1e-4, "{}", ratio(0));
        for i in 1..1024 {
            if oa[2 * i].abs() > 1e-3 && oa[2 * i - 2].abs() > 1e-3 {
                assert!(ratio(i) <= ratio(i - 1) + 1e-4, "it only goes down, at {i}");
            }
        }
        // After the ramp, exactly half of the other's.
        for i in 480..1024 {
            assert_eq!(ob[2 * i], oa[2 * i] * 0.5, "at {i}");
        }
        assert_eq!(b.gain(), 0.25);
    }

    #[test]
    fn a_level_with_nothing_playing_is_the_next_start_s_own() {
        let mut p = Preview::new(48_000.0);
        p.set_gain(0.3);
        assert_eq!(p.gain(), 0.3);
        p.start(tone(48_000, 1000), 0.7);
        assert_eq!(p.gain(), 0.7);
    }

    #[test]
    fn a_new_audition_replaces_the_old_and_returns_it() {
        let mut p = Preview::new(48_000.0);
        p.start(tone(48_000, 10_000), 1.0);
        assert!(p.start(tone(44_100, 10_000), 1.0).is_some());
    }
}

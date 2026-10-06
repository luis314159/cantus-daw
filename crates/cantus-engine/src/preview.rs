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

#[derive(Debug)]
pub struct Preview {
    sample: Option<Arc<SampleData>>,
    /// Read position in the sample's frames.
    pos: f64,
    /// Sample frames per output frame.
    step: f64,
    gain: f32,
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
        self.attack = 0;
        self.release = None;
        self.sample.replace(s)
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
            rate,
            channels: 1,
            frames: (0..frames)
                .map(|i| (i as f32 * 0.01).sin())
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

    #[test]
    fn a_new_audition_replaces_the_old_and_returns_it() {
        let mut p = Preview::new(48_000.0);
        p.start(tone(48_000, 10_000), 1.0);
        assert!(p.start(tone(44_100, 10_000), 1.0).is_some());
    }
}

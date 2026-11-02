//! Time in a video export is counted, never measured (ADR 0088 §6).
//!
//! Video frame `n` starts at `n × TIMESCALE / fps` on a 30 000-a-second timescale (1 000
//! ticks a frame at 30 fps, 500 at 60): exact for every rate offered (24, 25, 30, 50, 60).
//! The audio belonging to frame `n` is the samples `⌊n·rate/fps⌋ … ⌊(n+1)·rate/fps⌋`: cut
//! so that the cuts never drift (44.1 kHz at 60 fps is 735 samples a frame; at 24 fps
//! 1 837.5, alternating 1 837 and 1 838).

use crate::VideoError;
use std::ops::Range;

/// The video track's timescale: ticks a second.
pub const TIMESCALE: u32 = 30_000;

/// The frame rates the export offers.
pub const FRAME_RATES: [u32; 5] = [24, 25, 30, 50, 60];

/// The sample rates the audio is written at (what AAC-LC on every platform takes).
pub const AUDIO_RATES: [u32; 2] = [44_100, 48_000];

/// The timing of one export: its frame rate, its audio's sample rate and its length.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timing {
    pub fps: u32,
    pub rate: u32,
    pub frames: u32,
}

impl Timing {
    /// A timing for `frames` frames at `fps`, the audio at `rate`; refused in words when
    /// either rate is not one the export offers.
    pub fn new(fps: u32, rate: u32, frames: u32) -> Result<Timing, VideoError> {
        if !FRAME_RATES.contains(&fps) {
            return Err(VideoError::Unsupported(format!(
                "{fps} frames a second (the export offers {FRAME_RATES:?})"
            )));
        }
        if !AUDIO_RATES.contains(&rate) {
            return Err(VideoError::Unsupported(format!(
                "audio at {rate} Hz (a video's audio is 44 100 or 48 000 Hz)"
            )));
        }
        Ok(Timing { fps, rate, frames })
    }

    /// The frames covering `seconds` (the last one partly).
    pub fn frames_for(fps: u32, seconds: f64) -> u32 {
        (seconds.max(0.0) * fps as f64).ceil() as u32
    }

    /// Ticks of the video timescale a frame lasts.
    pub fn frame_ticks(&self) -> u32 {
        TIMESCALE / self.fps
    }

    /// Where frame `n` starts, in seconds (exact as a rational; this is its nearest f64).
    pub fn frame_seconds(&self, n: u32) -> f64 {
        n as f64 / self.fps as f64
    }

    /// The first sample of frame `n`'s audio.
    pub fn frame_sample(&self, n: u32) -> u64 {
        n as u64 * self.rate as u64 / self.fps as u64
    }

    /// The samples (frames of audio) belonging to video frame `n`.
    pub fn audio_of_frame(&self, n: u32) -> Range<u64> {
        self.frame_sample(n)..self.frame_sample(n + 1)
    }

    /// The audio's whole length in samples: exactly the video's.
    pub fn audio_samples(&self) -> u64 {
        self.frame_sample(self.frames)
    }

    /// The video's length in seconds.
    pub fn seconds(&self) -> f64 {
        self.frames as f64 / self.fps as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_rate_has_whole_ticks_a_frame() {
        for fps in FRAME_RATES {
            assert_eq!(TIMESCALE % fps, 0, "{fps}");
        }
        let t = Timing::new(30, 48_000, 900).unwrap();
        assert_eq!(t.frame_ticks(), 1_000);
        assert_eq!(Timing::new(60, 48_000, 1).unwrap().frame_ticks(), 500);
    }

    #[test]
    fn the_audio_cuts_never_drift() {
        for fps in FRAME_RATES {
            for rate in AUDIO_RATES {
                let t = Timing::new(fps, rate, 10 * fps).unwrap();
                let mut next = 0;
                for n in 0..t.frames {
                    let r = t.audio_of_frame(n);
                    assert_eq!(r.start, next);
                    let len = r.end - r.start;
                    let exact = rate as f64 / fps as f64;
                    assert!((len as f64 - exact).abs() < 1.0, "{fps} {rate} {len}");
                    next = r.end;
                }
                assert_eq!(next, 10 * rate as u64);
                assert_eq!(t.audio_samples(), 10 * rate as u64);
            }
        }
    }

    #[test]
    fn rates_the_export_does_not_offer_are_refused_in_words() {
        let e = Timing::new(29, 48_000, 1).unwrap_err().to_string();
        assert!(e.contains("29 frames a second"), "{e}");
        let e = Timing::new(30, 96_000, 1).unwrap_err().to_string();
        assert!(e.contains("96000 Hz"), "{e}");
        assert_eq!(Timing::frames_for(30, 30.0), 900);
        assert_eq!(Timing::frames_for(30, 0.01), 1);
    }
}

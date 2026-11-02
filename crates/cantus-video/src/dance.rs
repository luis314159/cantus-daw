//! The song's video (ADR 0088 §6): every dancer of the project drawn at each frame's time
//! from the export's own render — the timeline its unit kept offline
//! (`cantus_engine::dance::DanceFeed::take_timeline`, ADR 0087) — through the one renderer.
//!
//! A timeline holds one [`DanceFrame`] a block, published at the block's end. The frame at
//! a video frame's time is the one of the block holding it, carried to that exact time
//! (`cantus_engine::dance::sample`) — so a pose is the pure function of the song's position
//! the screen draws (`pose_at`), and the screen and the video agree.

use crate::export::FrameSource;
use cantus_dance::render::{render_into, Stage};
use cantus_dance::{pose_at, Character, DanceFrame, Pose};
use std::sync::Arc;
use tiny_skia::Pixmap;

/// One dancer: its character and the timeline its unit kept.
#[derive(Clone)]
pub struct Dancer {
    pub character: Arc<dyn Character>,
    pub timeline: Vec<DanceFrame>,
}

/// What a frame draws besides the dancers (the visualiser, the title): called after them.
pub type Overlay = dyn Fn(u32, f64, &mut Pixmap) + Send + Sync;

/// Every frame of the song's video.
pub struct SongFrames {
    pub dancers: Vec<Dancer>,
    /// The stage at the video's size, with its background.
    pub stage: Stage,
    /// The song's seconds at video time 0 (the range's start).
    pub start_seconds: f64,
    pub fps: u32,
    pub overlay: Option<Box<Overlay>>,
}

/// The frame of `timeline` at song `seconds` (the engine's own rule: the frame of the
/// segment holding that moment, carried to it — `cantus_engine::dance::sample`); an empty
/// timeline gives a dancer standing still.
pub fn frame_at(timeline: &[DanceFrame], seconds: f64) -> DanceFrame {
    cantus_engine::dance::sample(timeline, seconds).unwrap_or(DanceFrame {
        seconds,
        ..DanceFrame::default()
    })
}

impl SongFrames {
    /// The poses of every dancer at video frame `n`.
    pub fn poses(&self, n: u32) -> Vec<Pose> {
        let t = self.start_seconds + n as f64 / self.fps as f64;
        self.dancers
            .iter()
            .map(|d| pose_at(&frame_at(&d.timeline, t), d.character.as_ref(), t))
            .collect()
    }
}

impl FrameSource for SongFrames {
    fn draw(&self, n: u32, pixmap: &mut Pixmap) {
        let poses = self.poses(n);
        let list: Vec<(&dyn Character, Pose)> = self
            .dancers
            .iter()
            .zip(poses)
            .map(|(d, p)| (d.character.as_ref(), p))
            .collect();
        render_into(pixmap, &self.stage, &list);
        if let Some(o) = &self.overlay {
            o(n, self.start_seconds + n as f64 / self.fps as f64, pixmap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(seconds: f64, bar: i32) -> DanceFrame {
        DanceFrame {
            seconds,
            beats: seconds * 2.0,
            tempo_bpm: 120.0,
            playing: true,
            bar,
            bar_start_beats: bar as f64 * 4.0,
            bar_beats: 4.0,
            ..DanceFrame::default()
        }
    }

    #[test]
    fn a_frame_is_carried_to_the_exact_time_and_over_bar_lines() {
        let tl: Vec<DanceFrame> = (1..300)
            .map(|i: i32| block(f64::from(i) * 0.01, i / 200))
            .collect();
        let f = frame_at(&tl, 1.234_5);
        assert!((f.seconds - 1.234_5).abs() < 1e-12);
        assert!((f.beats - 2.469).abs() < 1e-9);
        let f = frame_at(&tl, 2.0);
        assert_eq!((f.bar, f.bar_start_beats), (1, 4.0));
        assert_eq!(frame_at(&tl, 1.999).bar, 0);
        assert_eq!(frame_at(&[], 3.0).seconds, 3.0);
    }

    #[test]
    fn the_same_time_gives_the_same_pose_at_any_frame_rate() {
        let tl: Vec<DanceFrame> = (0..800)
            .map(|i: i32| block(f64::from(i) * 0.01, i / 200))
            .collect();
        let ch = cantus_dance::cast::default_character();
        let at = |fps: u32, n: u32| {
            SongFrames {
                dancers: vec![Dancer {
                    character: ch.clone(),
                    timeline: tl.clone(),
                }],
                stage: Stage::new(64, 64, cantus_dance::Background::Transparent),
                start_seconds: 0.0,
                fps,
                overlay: None,
            }
            .poses(n)[0]
        };
        // 2.5 s is frame 75 at 30 fps and frame 150 at 60.
        assert_eq!(format!("{:?}", at(30, 75)), format!("{:?}", at(60, 150)));
    }
}

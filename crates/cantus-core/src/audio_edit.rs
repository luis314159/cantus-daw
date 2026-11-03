//! The audio editor's edits (ADR 0096), as pure functions on a buffer: what a destructive
//! edit does to a take before it is written as a new file. Nothing here reads or writes a
//! file (`cantus-core` does no I/O) and nothing is random: the same buffer, selection and
//! edit give the same bits.
//!
//! **Identity edits touch no sample.** A gain of 0 dB, a fade of no length, a
//! reversal twice, an inversion twice, a cut put back where it was, and noise reduction at
//! an amount of 0 return the buffer's bits (tests). An edit that finds nothing to do says
//! so ([`Edited::changed`] false) and the editor writes no file.

use crate::audio::FadeCurve;
use cantus_dsp::denoise::{NoiseProfile, Params};

#[path = "audio_edit_apply.rs"]
mod apply_impl;
#[path = "audio_edit_join.rs"]
pub mod join;
#[path = "audio_edit_silence.rs"]
pub mod silence;
#[path = "audio_edit_wav.rs"]
pub mod wav;

/// Interleaved audio at the file's own rate.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioBuf {
    pub rate: u32,
    pub channels: u8,
    pub data: Vec<f32>,
}

impl AudioBuf {
    pub fn new(rate: u32, channels: u8, data: Vec<f32>) -> Self {
        AudioBuf {
            rate,
            channels: channels.max(1),
            data,
        }
    }

    pub fn frames(&self) -> usize {
        self.data.len() / self.channels.max(1) as usize
    }

    /// Channel `c`, alone.
    pub fn channel(&self, c: usize) -> Vec<f32> {
        let n = self.channels.max(1) as usize;
        self.data.iter().skip(c).step_by(n).copied().collect()
    }

    /// From one `Vec` per channel (all the same length).
    pub fn from_planar(rate: u32, planar: &[Vec<f32>]) -> Self {
        let n = planar.len().max(1);
        let frames = planar.first().map_or(0, Vec::len);
        let mut data = vec![0.0; frames * n];
        for (c, ch) in planar.iter().enumerate() {
            for (i, v) in ch.iter().enumerate() {
                data[i * n + c] = *v;
            }
        }
        AudioBuf::new(rate, n as u8, data)
    }

    /// Every channel apart.
    pub fn planar(&self) -> Vec<Vec<f32>> {
        (0..self.channels.max(1) as usize)
            .map(|c| self.channel(c))
            .collect()
    }

    /// Frames `sel` as a buffer of its own.
    pub fn slice(&self, sel: Sel) -> AudioBuf {
        let n = self.channels.max(1) as usize;
        let s = sel.clamp(self.frames());
        AudioBuf::new(
            self.rate,
            self.channels,
            self.data[s.start * n..s.end * n].to_vec(),
        )
    }

    /// The mono mix (the mean of the channels), for display and analysis.
    pub fn mono(&self) -> Vec<f32> {
        let n = self.channels.max(1) as usize;
        if n == 1 {
            return self.data.clone();
        }
        self.data
            .chunks_exact(n)
            .map(|f| f.iter().sum::<f32>() / n as f32)
            .collect()
    }

    /// The largest absolute sample in `sel`.
    pub fn peak(&self, sel: Sel) -> f32 {
        let n = self.channels.max(1) as usize;
        let s = sel.clamp(self.frames());
        self.data[s.start * n..s.end * n]
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()))
    }

    /// Root mean square over every channel and frame of `sel`, linear.
    pub fn rms(&self, sel: Sel) -> f32 {
        let n = self.channels.max(1) as usize;
        let s = sel.clamp(self.frames());
        let part = &self.data[s.start * n..s.end * n];
        if part.is_empty() {
            return 0.0;
        }
        (part.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / part.len() as f64).sqrt()
            as f32
    }

    /// Integrated loudness (BS.1770, gated) of `sel`, LUFS; `None` when it is shorter than a
    /// 400 ms block or all below the gate.
    pub fn lufs(&self, sel: Sel) -> Option<f32> {
        use cantus_dsp::comprimo::loudness as l;
        let n = self.channels.max(1) as usize;
        let s = sel.clamp(self.frames());
        let hop = (self.rate as f64 * l::HOP_S).round().max(1.0) as usize;
        let mut m = l::Meter::new(self.rate as f32);
        let mut blocks = Vec::new();
        let (mut in_hop, mut hops) = (0usize, 0usize);
        for f in s.start..s.end {
            let left = self.data[f * n];
            let right = if n > 1 { self.data[f * n + 1] } else { 0.0 };
            m.add(left, right);
            in_hop += 1;
            if in_hop == hop {
                m.end_hop(hop);
                hops += 1;
                in_hop = 0;
                if hops >= l::MOMENTARY_HOPS {
                    blocks.push(m.momentary());
                }
            }
        }
        let v = l::integrated(&blocks);
        (v > l::SILENCE).then_some(v)
    }
}

/// A selection in frames, `start..end`, in the buffer's own frames.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Sel {
    pub start: usize,
    pub end: usize,
}

impl Sel {
    pub fn new(a: usize, b: usize) -> Sel {
        Sel {
            start: a.min(b),
            end: a.max(b),
        }
    }

    pub fn whole(frames: usize) -> Sel {
        Sel {
            start: 0,
            end: frames,
        }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clamp(self, frames: usize) -> Sel {
        Sel {
            start: self.start.min(frames),
            end: self.end.min(frames).max(self.start.min(frames)),
        }
    }
}

/// What to bring the selection to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Level {
    /// The highest sample, dBFS.
    Peak,
    /// Root mean square over the selection, dBFS.
    Rms,
    /// Integrated loudness, LUFS.
    Lufs,
}

/// One destructive edit.
#[derive(Clone, Debug)]
pub enum Edit {
    /// Take the selection out; what follows moves up. (Cut is this, with the clipboard.)
    Delete,
    /// Keep the selection, drop the rest.
    Trim,
    /// Put `clip` at the selection's start, replacing the selection (nothing, when it is
    /// empty: an insertion). Same rate and channels as the take (the editor conforms it).
    Paste {
        clip: AudioBuf,
    },
    /// Insert `frames` of silence at the selection's start.
    InsertSilence {
        frames: usize,
    },
    /// Replace the selection with silence.
    Silence,
    FadeIn {
        curve: FadeCurve,
    },
    FadeOut {
        curve: FadeCurve,
    },
    /// Gain in dB over the selection.
    Gain {
        db: f32,
    },
    /// Bring the selection to `target_db` by one gain; for RMS and LUFS, never past a
    /// peak of `ceiling_db` when one is given (the gain stops there and says so).
    Normalize {
        level: Level,
        target_db: f32,
        ceiling_db: Option<f32>,
    },
    Invert,
    Reverse,
    /// Take the mean of the selection out of it, per channel.
    DcRemove,
    /// Cut the quiet stretches out of the selection (see [`silence`]).
    RemoveSilence(silence::Settings),
    /// Noise reduction against a learned profile.
    Denoise {
        profile: NoiseProfile,
        params: Params,
    },
    /// Turn a time-frequency rectangle of the selection's span up or down.
    Attenuate {
        lo_hz: f32,
        hi_hz: f32,
        gain_db: f32,
    },
    /// Find the clicks in the selection (or the take) and repair each from the audio
    /// around it; `sensitivity` 0..1.
    Declick {
        sensitivity: f32,
    },
    /// Repair the selection itself (a click, a pop: at most [`MAX_REPAIR_MS`]) from the
    /// audio around it.
    Repair,
}

/// The longest selection *Repair* rebuilds, ms.
pub const MAX_REPAIR_MS: f32 = 20.0;

impl Edit {
    /// A stable name, for undo labels and agents.
    pub fn name(&self) -> &'static str {
        match self {
            Edit::Delete => "delete",
            Edit::Trim => "trim",
            Edit::Paste { .. } => "paste",
            Edit::InsertSilence { .. } => "insert_silence",
            Edit::Silence => "silence",
            Edit::FadeIn { .. } => "fade_in",
            Edit::FadeOut { .. } => "fade_out",
            Edit::Gain { .. } => "gain",
            Edit::Normalize { .. } => "normalize",
            Edit::Invert => "invert",
            Edit::Reverse => "reverse",
            Edit::DcRemove => "dc_remove",
            Edit::RemoveSilence(_) => "remove_silence",
            Edit::Denoise { .. } => "denoise",
            Edit::Attenuate { .. } => "attenuate",
            Edit::Declick { .. } => "declick",
            Edit::Repair => "repair",
        }
    }

    /// Whether the audio's timing stays where it was, sample for sample: the pitch
    /// editor's notes and a warp's markers (both set on source frames) still fit. Gain-like
    /// edits and repairs do; a reversal, a paste and every edit that changes the length do
    /// not.
    pub fn keeps_timing(&self) -> bool {
        matches!(
            self,
            Edit::Gain { .. }
                | Edit::Normalize { .. }
                | Edit::Invert
                | Edit::DcRemove
                | Edit::FadeIn { .. }
                | Edit::FadeOut { .. }
                | Edit::Silence
                | Edit::Denoise { .. }
                | Edit::Attenuate { .. }
                | Edit::Declick { .. }
                | Edit::Repair
        )
    }
}

/// Why an edit was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum EditError {
    /// The edit needs a selection and there is none.
    NeedsSelection,
    /// Nothing would be left.
    WouldBeEmpty,
    /// Another channel count or rate.
    Mismatch,
    /// Too short to measure (loudness needs 400 ms).
    TooShort,
    /// Not enough audio around the selection to rebuild it from.
    NoContext,
    /// A selection too long for the edit (a repair).
    TooLong,
    /// A value out of range.
    Invalid(&'static str),
    Denoise(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::NeedsSelection => f.write_str("select part of the audio first"),
            EditError::WouldBeEmpty => f.write_str("nothing would be left"),
            EditError::Mismatch => f.write_str("the other audio has another rate or channels"),
            EditError::TooShort => f.write_str("too short to measure (loudness needs 400 ms)"),
            EditError::NoContext => {
                f.write_str("not enough audio around the selection to rebuild it from")
            }
            EditError::TooLong => f.write_str("a repair is for clicks: 20 ms at most"),
            EditError::Invalid(w) => write!(f, "invalid {w}"),
            EditError::Denoise(e) => f.write_str(e),
        }
    }
}

/// Something an edit wants said beside its result.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Note {
    /// The normalize stopped at the ceiling: the level it reached, dB (or LUFS).
    Ceiling { reached_db: f32 },
    /// The normalize pushed peaks over 0 dBFS: the peak, dBFS.
    Clips { peak_db: f32 },
    /// Clicks found and repaired.
    Clicks { count: usize },
}

/// What an edit made.
#[derive(Clone, Debug, PartialEq)]
pub struct Edited {
    pub buf: AudioBuf,
    /// Where the selection is now (the pasted audio, the join of a cut).
    pub sel: Sel,
    /// False when the result is the buffer it came from.
    pub changed: bool,
    /// Frames added (positive) or taken out (negative).
    pub delta_frames: i64,
    /// The selection the edit really worked on (its edges moved to zero crossings).
    pub used: Sel,
    pub note: Option<Note>,
}

pub use apply_impl::{apply, apply_with};
pub use join::Joins;

#[cfg(test)]
#[path = "audio_edit_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "audio_edit_tests2.rs"]
mod tests2;

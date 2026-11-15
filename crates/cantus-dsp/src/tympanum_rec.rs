//! A recorded kit on the audio thread (ADR 0124 §2): every strike plays **one take** —
//! drawn once ([`super::hit_draw`], [`super::take_index`]) — on every microphone, each mic's
//! file on its own [`Playhead`] (in memory or streamed from disk, ADR 0050), into that
//! mic's lane through the piece × mic cell (`b{p}.{m}`: level, polarity, power).
//!
//! **The hi-hat is continuous.** Strikes of one key recorded at several openness levels
//! (`ArtHits::openness`) are crossfaded at the hit between the two levels nearest the
//! openness controller (CC4), and the crossfade follows the controller while they ring; a
//! pedal falling below the pair chokes them, and a pedal closing fast from open plays the
//! foot's chick. A hat strike silences every hat ringing more open than itself.
//!
//! Fixed pools — voices, plays, each play's window — allocated with the engine; nothing
//! is allocated, locked or read from disk here.

use super::{hit_draw, take_index, Outputs, LANE, MAX_MICS};
use crate::exemplar::play::{Playhead, Reach};
use crate::exemplar::resample::{Quality, Reader, MAX_TAPS};
use crate::exemplar::stream::Board;
use crate::exemplar::voice::Svf;
use crate::exemplar::{Region, Source};
use std::sync::Arc;

/// Strikes sounding at once.
pub const MAX_VOICES: usize = 64;
/// Microphone plays sounding at once (a strike plays one per mic, two for a crossfaded
/// hat).
pub const MAX_PLAYS: usize = 320;
/// Pieces a kit has at most (`cantus_core::inst::tympanum::MAX_PIECES`).
pub const MAX_PIECES: usize = 32;
/// Output frames between two looks at the stream (Exemplar's control step).
const CONTROL: usize = 32;
/// A voice stolen or a play late fades over this.
const STEAL_MS: f32 = 5.0;
/// Plays kept free for stolen voices' fades (fix-e): a strike takes a play only while
/// this many stay for them — four strikes of a sixteen-mic kit stolen within one fade.
pub const RESERVE: usize = 64;
/// A stolen voice's plays read no voice's gains: these.
const ONES: [f32; LANE] = [1.0; LANE];
/// How long a hat crossfade takes to follow the pedal.
const FOLLOW_MS: f32 = 8.0;
const NONE: u16 = u16::MAX;

/// One mic's file of one take, as the engine plays it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecMicTake {
    pub mic: u8,
    pub file: u32,
    /// The take's own level (the mic's in the kit's mix, the region's), linear.
    pub gain: f32,
    /// −1 … 1: the constant-power law for a mono file, a balance for a stereo one.
    pub pan: f32,
    pub offset: u32,
    pub cents: f32,
    pub highpass: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecTake {
    pub mics: Vec<RecMicTake>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecLayer {
    pub lo: u8,
    pub hi: u8,
    /// The layer's gain at each velocity.
    pub gain: [f32; 128],
    pub takes: Vec<RecTake>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecArt {
    pub piece: u16,
    pub key: u8,
    /// A hi-hat's level, 0 closed … 1 open.
    pub openness: Option<f32>,
    pub layers: Vec<RecLayer>,
    pub chokes: Vec<u16>,
    pub choke_s: f32,
    pub polyphony: u8,
}

/// A recorded kit, ready for the audio thread (built off it from the kit's definition).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecKit {
    pub mics: usize,
    pub arts: Vec<RecArt>,
    /// The foot's chick (a pedal closing on an open hat plays it).
    pub pedal: Option<u16>,
}

impl RecKit {
    /// The hat articulations on `key`, by openness (lowest first), as indices.
    fn hats_on(&self, key: u8, out: &mut [u16; 16]) -> usize {
        let mut n = 0;
        for (i, a) in self.arts.iter().enumerate() {
            if a.key == key && a.openness.is_some() && n < out.len() {
                out[n] = i as u16;
                n += 1;
            }
        }
        // Insertion sort by openness (a handful).
        for i in 1..n {
            let mut j = i;
            while j > 0
                && self.arts[out[j - 1] as usize].openness > self.arts[out[j] as usize].openness
            {
                out.swap(j - 1, j);
                j -= 1;
            }
        }
        n
    }
}

/// What a voice is doing.
#[derive(Copy, Clone, Debug)]
struct Voice {
    active: bool,
    /// The articulation, and the more open one it crossfades into (a hat), or `NONE`.
    art: [u16; 2],
    /// The second articulation's weight now, and where the pedal wants it.
    w: f32,
    w_to: f32,
    piece: u16,
    note: u64,
    age: u64,
    /// The openness this strike sounds at (a hat), for the choke rule.
    open: f32,
    hat: bool,
    /// A choke's fade: 1 … 0, and its step per frame (0: not fading).
    fade: f32,
    fade_step: f32,
    /// The piece's damping (`p{n}.damp`): an envelope falling by `env_k` a frame.
    env: f32,
    env_k: f32,
    plays: u16,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            active: false,
            art: [NONE; 2],
            w: 0.0,
            w_to: 0.0,
            piece: 0,
            note: 0,
            age: 0,
            open: 0.0,
            hat: false,
            fade: 1.0,
            fade_step: 0.0,
            env: 1.0,
            env_k: 1.0,
            plays: 0,
        }
    }
}

/// One mic of one voice.
#[derive(Clone, Debug, Default)]
struct Play {
    active: bool,
    voice: u16,
    /// Which of the voice's two articulations (a crossfaded hat).
    side: u8,
    mic: u8,
    stereo: bool,
    head: Playhead,
    hp: Svf,
    hp_on: bool,
    gl: f32,
    gr: f32,
    ratio: f64,
    /// The play's own fade (a late block), 1 … 0, and its step.
    fade: f32,
    fade_step: f32,
    /// A stolen voice's play (fix-e): its gain frozen in `gl`/`gr`, fading on its own.
    orphan: bool,
    late: bool,
    cut: bool,
}

/// Per piece and mic: the cell's gain (level × polarity × the kit's bleed fader), 0 off.
pub type Cells = [[f32; MAX_MICS]; MAX_PIECES];

/// The recorded kit's engine.
#[derive(Debug)]
pub struct RecEngine {
    fs: f32,
    kit: Option<Arc<RecKit>>,
    reader: Reader,
    voices: Box<[Voice]>,
    plays: Box<[Play]>,
    board: Arc<Board>,
    /// Each voice's two gains per frame of the chunk (side 0, side 1).
    gains: Vec<f32>,
    pub seed: u32,
    pub gain: f32,
    /// `vary`: never the take before on the key (on), or a free draw.
    pub vary: bool,
    /// `hat.chick`: the foot's chick when the pedal closes a ringing hat.
    pub chick: bool,
    pub cells: Box<Cells>,
    /// Per piece: cents and the damping's envelope fall per frame (1: none).
    pub piece_cents: [f32; MAX_PIECES],
    pub piece_env_k: [f32; MAX_PIECES],
    /// The pedal (CC4) 0 … 1, once one has arrived.
    pedal: Option<f32>,
    counter: u64,
    chicks: u32,
    late: u32,
    cut: u32,
    wakes: u32,
}

impl RecEngine {
    /// Allocates every pool: with the unit, never on the audio thread.
    pub fn new(fs: f32) -> RecEngine {
        let fs = if fs > 0.0 { fs } else { 48_000.0 };
        let board = Arc::new(Board::new(MAX_PLAYS));
        let fade = (STEAL_MS * fs / 1000.0).ceil() as usize;
        let ahead = CONTROL + fade;
        let frames =
            (2 * ahead + 2 * MAX_TAPS + 4 * crate::exemplar::play::HB_M + 64).next_multiple_of(256);
        let mut plays = vec![Play::default(); MAX_PLAYS].into_boxed_slice();
        for (i, p) in plays.iter_mut().enumerate() {
            p.head.attach_window(frames, fade, board.clone(), i as u16);
            p.head
                .set_head_only_above(crate::exemplar::head_only_above(fs));
        }
        RecEngine {
            fs,
            kit: None,
            reader: Reader::new(Quality::Standard, fs),
            voices: vec![Voice::default(); MAX_VOICES].into_boxed_slice(),
            plays,
            board,
            gains: vec![0.0; MAX_VOICES * 2 * LANE],
            seed: 0,
            gain: 1.0,
            vary: true,
            chick: true,
            cells: Box::new([[1.0; MAX_MICS]; MAX_PIECES]),
            piece_cents: [0.0; MAX_PIECES],
            piece_env_k: [1.0; MAX_PIECES],
            pedal: None,
            counter: 0,
            chicks: 0,
            late: 0,
            cut: 0,
            wakes: 0,
        }
    }

    /// The board its plays publish their reads on (the stream reader watches it).
    pub fn board(&self) -> &Arc<Board> {
        &self.board
    }

    /// The kit (an `Arc` clone: no allocation). A different kit silences the old one's
    /// voices at once (the unit keeps playing the old kit until the new one is loaded).
    pub fn set_kit(&mut self, kit: Option<&Arc<RecKit>>) {
        let same = match (&self.kit, kit) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.silence();
            self.kit = kit.cloned();
        }
    }

    pub fn kit(&self) -> Option<&Arc<RecKit>> {
        self.kit.as_ref()
    }

    pub fn set_blocking(&mut self, on: bool) {
        for p in self.plays.iter_mut() {
            p.head.set_blocking(on);
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    pub fn is_quiet(&self) -> bool {
        !self.voices.iter().any(|v| v.active) && !self.plays.iter().any(|p| p.active && p.orphan)
    }

    /// Every voice gone at once (a reset, a kit changed).
    pub fn silence(&mut self) {
        for v in self.voices.iter_mut() {
            *v = Voice::default();
        }
        for p in self.plays.iter_mut() {
            p.active = false;
            p.head.let_go();
        }
    }

    pub fn reset(&mut self) {
        self.silence();
        self.pedal = None;
        self.counter = 0;
        self.chicks = 0;
    }

    pub fn take_late(&mut self) -> u32 {
        std::mem::take(&mut self.late)
    }

    pub fn take_cut(&mut self) -> u32 {
        std::mem::take(&mut self.cut)
    }

    pub fn take_wakes(&mut self) -> u32 {
        std::mem::take(&mut self.wakes)
    }
}

#[path = "tympanum_rec_play.rs"]
mod play;

#[cfg(test)]
#[path = "tympanum_rec_tests.rs"]
mod tests;

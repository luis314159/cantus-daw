//! A modelled kit (ADR 0124 §2): Pulsus's modal voices strike the pieces, and **virtual
//! microphones** hear them where they stand. Each piece renders once, dry and mono, into
//! a ring; each microphone (each capsule of a pair) reads every piece through its paths —
//! the direct sound and the room's first reflections (image sources on the room's walls,
//! floor and ceiling) — each path a fractional delay (distance / c), a level (1/r, the
//! mic's pattern at the angle it arrives from, the piece's radiation towards it, its
//! polarity: a mic under a head hears it pushed the other way) and a colour (off-axis
//! highs fall, a directional mic close by gains bass). The room's late field
//! ([`super::room::Room`]) joins every mic by how far into the room it listens. Moving a
//! mic (`m{n}.*`) moves its paths: their delays and levels glide, sample by sample.
//!
//! Every path renders sample by sample from the ring, so any chunking gives the same bits.

use super::room::Room;
use super::{pan_law, Outputs, LANE, MAX_MICS};
use crate::math;
use crate::pulsus::{PadSettings, Pulsus, MAX_PADS};

/// Pieces a modelled kit has at most.
pub const MAX_PIECES: usize = 32;
/// Paths at most (pieces × capsules × direct and reflections).
pub const MAX_PATHS: usize = 1536;
/// The speed of sound, m/s.
pub const C: f32 = 343.0;
/// A path's delay and level glide over this when a mic moves.
const GLIDE_MS: f32 = 30.0;
/// Each piece's ring carries a copy of its first samples after its end (drums-gen 3, the
/// cost): a settled path reads a chunk's window in one run, never wrapping.
const MIRROR: usize = LANE + 4;
/// Groups (piece × mic × channel) whose colours run side by side (their filters' chains
/// overlap: each alone waited on its own previous sample).
const BATCH: usize = 8;

/// Where a piece stands and how it radiates, metres in the kit's frame (x to the
/// audience's right, y towards the audience, z up).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PieceGeom {
    pub pos: [f32; 3],
    /// The batter head's normal, towards the side it is struck from.
    pub normal: [f32; 3],
    /// How much of its sound is a dipole along the normal (heads, cymbals) rather than
    /// the same all round (0 … 1).
    pub dipole: f32,
    /// An extended plate (a cymbal: 0 a point source): a stereo pair hears its two sides
    /// apart — the left capsule the main less `width` of the side, the right the main
    /// plus it (the voice's side signal, uncorrelated with the main: a model 5 cymbal's;
    /// a mono voice has none). Every piece's direct sound reaches a pair from the pair's
    /// centre.
    pub width: f32,
    /// A drum head's radius, m (0: the kind's own, a 22″ kick's or a 14″ drum's): its
    /// highs leave along its normal, the narrower the larger the head (`head_beam`).
    pub radius: f32,
}

/// A microphone: where it stands and aims, its pattern, a pair's spacing; and what the
/// kit's own mix does with it.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct MicGeom {
    pub pos: [f32; 3],
    pub aim: [f32; 3],
    /// 0 omni, 1 cardioid, 2 supercardioid, 3 hypercardioid, 4 figure-of-eight.
    pub pattern: u8,
    /// A stereo pair's spacing, m (0: mono).
    pub spread: f32,
    /// The kit's own pan for a mono mic, and its level (linear).
    pub pan: f32,
    pub level: f32,
    /// How much of the room's late field it hears (0 … 1): rooms most, close mics least.
    pub room: f32,
    /// What the level is relative to: the gain of its own piece's direct sound (or the
    /// kit's centre's, for an overhead or a room) at its default place, so moving the mic
    /// away makes it quieter, as it would.
    pub norm: f32,
    /// Which reflections it hears: 0 the floor only (a close mic), 1 floor and ceiling,
    /// 2 every wall.
    pub reflections: u8,
}

/// The room, metres: width (x), depth (y), height (z); where the wall behind the kit is;
/// how much each surface reflects; its reverberation time.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RoomGeom {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    pub back: f32,
    pub reflect: f32,
    pub floor_reflect: f32,
    pub t60: f32,
}

impl Default for RoomGeom {
    fn default() -> Self {
        RoomGeom {
            width: 6.0,
            depth: 8.0,
            height: 3.4,
            back: 2.7,
            reflect: 0.7,
            floor_reflect: 0.75,
            t60: 0.45,
        }
    }
}

/// The pattern's gain at `cos` of the angle off its axis.
#[inline]
pub fn pattern_gain(pattern: u8, cos: f32) -> f32 {
    let a = match pattern {
        0 => 1.0,
        1 => 0.5,
        2 => 0.37,
        3 => 0.25,
        _ => 0.0,
    };
    a + (1.0 - a) * cos
}

#[inline]
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// One path from a piece to a capsule.
#[derive(Copy, Clone, Debug, Default)]
struct Path {
    piece: u8,
    mic: u8,
    /// 0 left, 1 right, 2 both (a mono mic).
    ch: u8,
    /// How much of the piece's side it hears (− left, + right; 0 none).
    side: f32,
    /// How much of its main (1; 0: a measured cymbal in a mono mic hears its side alone).
    main: f32,
    /// Delay in samples and level, now and where they glide to.
    delay: f32,
    gain: f32,
    delay_to: f32,
    gain_to: f32,
}

/// One piece × capsule's colour: an off-axis low-pass (resting on `floor` of its input: a
/// head's beam heard in its near field) and a proximity low shelf.
#[derive(Copy, Clone, Debug, Default)]
struct Colour {
    lp_a: f32,
    lp: f32,
    floor: f32,
    shelf_a: f32,
    shelf_k: f32,
    shelf: f32,
}

/// The modelled kit's engine.
#[derive(Clone, Debug)]
pub struct ModelEngine {
    fs: f32,
    pub pulsus: Pulsus,
    pieces: usize,
    mics: usize,
    geom_p: [PieceGeom; MAX_PIECES],
    geom_m: [MicGeom; MAX_MICS],
    room_geom: RoomGeom,
    /// Per piece × mic: the bleed cell's gain (0: off).
    pub cells: Box<[[f32; MAX_MICS]; MAX_PIECES]>,
    /// The chunk's dry pieces, `LANE` each, and their sides.
    src: Vec<f32>,
    src_s: Vec<f32>,
    touched: [bool; MAX_PIECES],
    ring: Vec<f32>,
    ring_s: Vec<f32>,
    ring_len: usize,
    ring_pos: usize,
    paths: Vec<Path>,
    n_paths: usize,
    /// Per piece × mic × channel (L, R): its colour.
    colour: Vec<Colour>,
    scratch: Vec<f32>,
    room: Room,
    room_send: [f32; MAX_PIECES],
    /// Pieces a measured voice plays (model 5): their tables hold the recording's own room.
    measured: [bool; MAX_PIECES],
    /// Frames since the last piece sounded (the paths still deliver for a while).
    quiet: usize,
    /// Per piece: frames since its last sample that was not zero (its ring all zeros past
    /// `ring_len`: its paths give nothing and are skipped).
    pquiet: [usize; MAX_PIECES],
    glide: f32,
    /// What each pad was last set to (`set_pads` sets only what changed).
    pads_set: Box<[Option<(u8, PadSettings)>]>,
    /// The hybrid voice (r40:drums-hyb): a recorded residual added to each piece's dry
    /// source, empty — the model alone, to the bit — unless the kit's `voice` is hybrid.
    pub hybrid: crate::tympanum_residual::Player,
}

impl ModelEngine {
    /// Allocates everything: with the unit, never on the audio thread.
    pub fn new(fs: f32) -> ModelEngine {
        let fs = if fs > 0.0 { fs } else { 48_000.0 };
        // Long enough for a reflection across a hall: 120 ms.
        let ring_len = ((0.12 * fs) as usize).next_power_of_two();
        ModelEngine {
            fs,
            pulsus: {
                let mut p = Pulsus::new(fs);
                p.enable_acoustic();
                p.enable_measured();
                p
            },
            pieces: 0,
            mics: 0,
            geom_p: [PieceGeom::default(); MAX_PIECES],
            geom_m: [MicGeom::default(); MAX_MICS],
            room_geom: RoomGeom::default(),
            cells: Box::new([[1.0; MAX_MICS]; MAX_PIECES]),
            src: vec![0.0; MAX_PIECES * LANE],
            src_s: vec![0.0; MAX_PIECES * LANE],
            touched: [false; MAX_PIECES],
            ring: vec![0.0; MAX_PIECES * (ring_len + MIRROR)],
            ring_s: vec![0.0; MAX_PIECES * (ring_len + MIRROR)],
            ring_len,
            ring_pos: 0,
            paths: vec![Path::default(); MAX_PATHS],
            n_paths: 0,
            colour: vec![Colour::default(); MAX_PIECES * MAX_MICS * 2],
            scratch: vec![0.0; BATCH * LANE],
            room: Room::new(fs),
            room_send: [0.0; MAX_PIECES],
            measured: [false; MAX_PIECES],
            quiet: usize::MAX / 2,
            pquiet: [usize::MAX / 2; MAX_PIECES],
            glide: 1.0 - math::exp(-1.0 / (GLIDE_MS * 0.001 * fs)),
            pads_set: vec![None; MAX_PADS].into_boxed_slice(),
            hybrid: crate::tympanum_residual::Player::new(fs),
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.fs
    }

    /// The pads (one per articulation, `bus` its piece): Pulsus's own call — only for a
    /// pad that changed (fix-e). Pulsus moves every ringing voice whose pad id matches to
    /// the pad set, and a kit's pads all carry id 0, so setting an unchanged pad again
    /// (each configure: a snapshot, a lane's control point) moved every ringing voice to
    /// the last pad's piece; now a configure with nothing changed changes no sample.
    pub fn set_pads(&mut self, pads: &[(u8, PadSettings)]) {
        let n = pads.len().min(MAX_PADS);
        self.pulsus.set_pad_count(n);
        self.measured = [false; MAX_PIECES];
        for (i, p) in pads.iter().take(n).enumerate() {
            if self.pads_set[i].as_ref() != Some(p) {
                self.pulsus.set_pad(i, p.0, &p.1);
                self.pads_set[i] = Some(*p);
            }
            if p.1.model == 5 {
                self.measured[(p.1.bus as usize).min(MAX_PIECES - 1)] = true;
            }
        }
        for p in 0..MAX_PIECES {
            self.room_send[p] = room_send(self.measured[p]);
        }
    }
}

/// How much of a piece the room's late field hears: a modal voice 0.6; a measured one
/// (its recording's room already in it) 0.15.
fn room_send(measured: bool) -> f32 {
    if measured {
        0.15
    } else {
        0.6
    }
}

#[path = "tympanum_model_paths.rs"]
mod paths;

impl ModelEngine {
    /// drums-gen 3, the cost: the paths (all, not culled) and the groups (piece × mic ×
    /// channel) they make.
    #[doc(hidden)]
    pub fn path_stats(&self) -> [usize; 3] {
        let p = &self.paths[..self.n_paths];
        let live = p.iter().filter(|q| q.gain_to != 0.0).count();
        let groups = (0..p.len())
            .filter(|&i| {
                i == 0
                    || (p[i].piece, p[i].mic, p[i].ch)
                        != (p[i - 1].piece, p[i - 1].mic, p[i - 1].ch)
            })
            .count();
        [p.len(), live, groups]
    }

    /// drums-gen 3: a piece's paths — (mic, channel, delay in samples, gain, main, side).
    #[doc(hidden)]
    pub fn paths_of(&self, piece: u8) -> Vec<(u8, u8, f32, f32, f32, f32)> {
        self.paths[..self.n_paths]
            .iter()
            .filter(|q| q.piece == piece)
            .map(|q| (q.mic, q.ch, q.delay_to, q.gain_to, q.main, q.side))
            .collect()
    }
}

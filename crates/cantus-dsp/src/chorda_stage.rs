//! Chorda's second model's perspective (round 37, voicing pass two — ADR 0098 §5): where
//! the microphones are, how wide the pair, the lid, the room's size. A modelled piano's
//! perspective is a control, not a recording: the body's direct path and its resonance, the
//! room's send, the lid's and the air's shelves and the pair's width are each a gain that
//! glides (20 ms, sample by sample: no zipper), so every setting moves smoothly and
//! deterministically. At the defaults every gain is exactly 1 and every shelf off: the
//! stage passes the voiced chain bit for bit.
//!
//! Perspective `p` (0 over the hammers … [`PERSP`] the player … 1 the audience): the direct
//! path rises toward the hammers ([`DIRECT_DB`] per unit) and the board's ring and the room
//! toward the audience; the air takes the top away with distance and a close pair hears it
//! brighter (a shelf from 2.5 kHz). The lid (0 closed … 1 open): a shelf from 1.2 kHz down
//! to −12 dB closed, the
//! strings' direct sound covered. The width (0…1, [`WIDTH`] as voiced): the board's and
//! the room's side signal scaled — a mid/side gain, so the mono sum never changes and no
//! setting can comb-filter it.

use crate::math;

/// The defaults: the player's perspective, the pair as voiced, the lid open, the room's
/// size as voiced.
pub const PERSP: f64 = 0.35;
pub const WIDTH: f64 = 0.7;
pub const LID: f64 = 1.0;
pub const SIZE: f64 = 0.4;

/// Per unit of perspective (dB): the direct path (toward the hammers), the board's ring
/// and the room (toward the audience), the action's noises (toward the hammers).
pub const DIRECT_DB: f64 = 24.0;
pub const BOARD_DB: f64 = 3.0;
pub const ROOM_DB: f64 = 20.0;
pub const NOISE_DB: f64 = 8.0;
/// The air: the top shelf at the audience (dB, at 1) and over the hammers (at 0).
pub const AIR_FAR_DB: f64 = -10.0;
pub const AIR_NEAR_DB: f64 = 2.5;
/// The air's shelf corner (Hz): a first-order shelf reaches most of its depth an octave
/// or two above it (−10 dB asked: about −6 dB at 5–10 kHz).
pub const AIR_HZ: f64 = 2_500.0;
/// The lid closed: its shelf (dB), the direct path's and the board's levels (dB).
pub const LID_SHELF_DB: f64 = -12.0;
pub const LID_DIRECT_DB: f64 = -6.0;
pub const LID_BOARD_DB: f64 = -1.5;
/// The perspective's level held about even (measured on the ballad and middle C: the
/// audience heard +3 dB louder, the hammers −2 dB, before it): the whole stage's gain at
/// the far end and at the near end (dB), linear in between from the player's.
pub const LEVEL_FAR_DB: f64 = -3.0;
pub const LEVEL_NEAR_DB: f64 = 1.8;
/// The room's size: its decay times scaled by 2^(3 (size − SIZE)) — 0.44× … 3.5×.
pub const SIZE_OCTAVES: f64 = 3.0;

/// `x`, or `d` when it is within a parameter's single-precision step of it (a stored 0.35
/// reads back as 0.349999994…: the default must still be exactly the default).
pub fn at(x: f64, d: f64) -> f64 {
    if (x - d).abs() < 1.0e-6 {
        d
    } else {
        x
    }
}

fn db(x: f64) -> f64 {
    math::powf(10.0f64, x / 20.0)
}

/// The action's noises' scale at perspective `p` (exactly 1 at [`PERSP`]).
pub fn noise_scale(p: f64) -> f64 {
    let p = at(p, PERSP);
    if p == PERSP {
        1.0
    } else {
        db(NOISE_DB * (PERSP - p))
    }
}

/// The room's decay scale at size `z` (exactly 1 at [`SIZE`]).
pub fn size_scale(z: f64) -> f64 {
    let z = at(z, SIZE);
    if z == SIZE {
        1.0
    } else {
        math::powf(2.0f64, SIZE_OCTAVES * (z - SIZE))
    }
}

/// A gain that glides to its target, one pole per sample; exactly its target once there.
#[derive(Clone, Copy, Debug)]
struct Glide {
    cur: f64,
    to: f64,
}

impl Glide {
    fn new(x: f64) -> Glide {
        Glide { cur: x, to: x }
    }
    #[inline]
    fn next(&mut self, k: f64) -> f64 {
        if self.cur != self.to {
            self.cur += k * (self.to - self.cur);
            if (self.to - self.cur).abs() < 1.0e-9 {
                self.cur = self.to;
            }
        }
        self.cur
    }
    fn snap(&mut self) {
        self.cur = self.to;
    }
}

/// A first-order high shelf: `x + g (x − lowpass(x))` per channel, off when its gain is 0
/// and its states are.
#[derive(Clone, Copy, Debug)]
struct Shelf {
    a: f64,
    y: [f64; 2],
}

impl Shelf {
    fn new(fs: f64, hz: f64) -> Shelf {
        Shelf {
            a: math::exp(-std::f64::consts::TAU * hz / fs),
            y: [0.0; 2],
        }
    }
    #[inline]
    fn tick(&mut self, g: f64, o: &mut [f64; 2]) {
        if g == 0.0 && self.y == [0.0; 2] {
            return;
        }
        for (c, x) in o.iter_mut().enumerate() {
            self.y[c] = *x + self.a * (self.y[c] - *x);
            *x += g * (*x - self.y[c]);
        }
    }
    fn flush(&mut self, floor: f64) {
        for y in self.y.iter_mut() {
            if y.abs() < floor {
                *y = 0.0;
            }
        }
    }
}

/// The stage: gains and shelves between the body and the output.
#[derive(Clone, Debug)]
pub struct Stage {
    k: f64,
    direct: Glide,
    board: Glide,
    room: Glide,
    side: Glide,
    lid: Glide,
    air: Glide,
    lid_shelf: Shelf,
    air_shelf: Shelf,
}

impl Stage {
    pub fn new(fs: f64) -> Stage {
        Stage {
            k: 1.0 - math::exp(-1.0 / (0.02 * fs)),
            direct: Glide::new(1.0),
            board: Glide::new(1.0),
            room: Glide::new(1.0),
            side: Glide::new(1.0),
            lid: Glide::new(0.0),
            air: Glide::new(0.0),
            lid_shelf: Shelf::new(fs, 1_200.0),
            air_shelf: Shelf::new(fs, AIR_HZ),
        }
    }

    /// The targets for perspective `p`, width `w` and lid `l` (each 0…1).
    pub fn set(&mut self, p: f64, w: f64, l: f64) {
        let p = at(p, PERSP).clamp(0.0, 1.0);
        let w = at(w, WIDTH).clamp(0.0, 1.0);
        let l = at(l, LID).clamp(0.0, 1.0);
        let closed = 1.0 - l;
        let at = |x: f64| if x == 0.0 { 1.0 } else { db(x) };
        let level = if p > PERSP {
            LEVEL_FAR_DB * (p - PERSP) / (1.0 - PERSP)
        } else {
            LEVEL_NEAR_DB * (PERSP - p) / PERSP
        };
        self.direct.to = at(DIRECT_DB * (PERSP - p) + LID_DIRECT_DB * closed + level);
        self.board.to = at(BOARD_DB * (p - PERSP) + LID_BOARD_DB * closed + level);
        self.room.to = at(ROOM_DB * (p - PERSP) + level);
        self.side.to = if w == WIDTH { 1.0 } else { w / WIDTH };
        self.lid.to = if closed == 0.0 {
            0.0
        } else {
            db(LID_SHELF_DB * closed) - 1.0
        };
        self.air.to = if p == PERSP {
            0.0
        } else if p > PERSP {
            db(AIR_FAR_DB * (p - PERSP) / (1.0 - PERSP)) - 1.0
        } else {
            db(AIR_NEAR_DB * (PERSP - p) / PERSP) - 1.0
        };
    }

    /// Every gain at its target now (a unit at rest takes its settings at once).
    pub fn snap(&mut self) {
        for g in [
            &mut self.direct,
            &mut self.board,
            &mut self.room,
            &mut self.side,
            &mut self.lid,
            &mut self.air,
        ] {
            g.snap();
        }
    }

    pub fn at_rest(&self) -> bool {
        self.lid_shelf.y == [0.0; 2] && self.air_shelf.y == [0.0; 2]
    }

    pub fn reset(&mut self) {
        self.lid_shelf.y = [0.0; 2];
        self.air_shelf.y = [0.0; 2];
        self.snap();
    }

    pub fn flush(&mut self, floor: f64) {
        self.lid_shelf.flush(floor);
        self.air_shelf.flush(floor);
    }

    /// Mid/side with the side scaled by `s` (exactly the input at 1).
    #[inline]
    fn width(s: f64, o: [f64; 2]) -> [f64; 2] {
        if s == 1.0 {
            return o;
        }
        let m = 0.5 * (o[0] + o[1]);
        let d = 0.5 * s * (o[0] - o[1]);
        [m + d, m - d]
    }

    /// The dry sound at the microphones: the direct path and the board's ring (each
    /// stereo) at the perspective's levels, the board's with the pair's width.
    #[inline]
    pub fn mix(&mut self, d: [f64; 2], b: [f64; 2]) -> [f64; 2] {
        let k = self.k;
        let (gd, gb, s) = (self.direct.next(k), self.board.next(k), self.side.next(k));
        let b = Self::width(s, b);
        [gd * d[0] + gb * b[0], gd * d[1] + gb * b[1]]
    }

    /// The lid's shelf on the dry sound (what the room hears and what goes on).
    #[inline]
    pub fn lid(&mut self, o: &mut [f64; 2]) {
        let g = self.lid.next(self.k);
        self.lid_shelf.tick(g, o);
    }

    /// The room's return (its level `amount`) added to the dry sound, its width, the air.
    #[inline]
    pub fn wet(&mut self, dry: [f64; 2], wet: [f64; 2], amount: f64) -> [f64; 2] {
        let k = self.k;
        let (gr, s) = (self.room.next(k), self.side.cur);
        let w = Self::width(s, wet);
        let g = amount * gr;
        let mut o = [dry[0] + g * w[0], dry[1] + g * w[1]];
        let air = self.air.next(k);
        self.air_shelf.tick(air, &mut o);
        o
    }
}

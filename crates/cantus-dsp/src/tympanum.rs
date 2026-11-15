//! Tympanum's DSP (round 40 phase 3, ADR 0124): the acoustic drummer, grown from Pulsus's
//! engine — modal voices for a modelled kit ([`model`]), sample voices for a recorded one
//! ([`rec`]), each microphone a lane, the piece × mic bleed matrix, continuous hi-hat
//! openness, virtual microphones (position, distance, aim and pattern → delay, level,
//! polarity, colour) and the room ([`room`]). Owned by `tymp-engine`.
//!
//! **One draw per hit, shared by every mic.** SFZ's `lorand`/`hirand` draws a number per
//! region, so the close mic and the overhead of one hit may play different takes and phase
//! against each other (sfzformat.com, `seq_length`'s warning). Tympanum draws **once per
//! hit** — from the kit's seed, the hit's key and its ordinal on the track (how many hits of
//! its key come before it, `NoteEvent::ordinal`), so a render from any start draws the same
//! — and every mic's region of that hit reads the same draw ([`take_index`]).
//!
//! **Lanes, to the bit** (ADR 0123 §3). Each microphone renders into its own lane, which
//! starts at +0.0 every [`LANE`] frames; its mix ([`crate::outmix::OutMix`], the delay of
//! [`OutDelay`]) runs on the lane; the lanes then go to their buses **in output order**
//! ([`Outputs::emit`]). A kit with every mic on Main and a kit with every mic on its own
//! strip receive the very same lanes in the very same order, so the split sums to the
//! unsplit to the bit.

use crate::outmix::OutMix;

#[path = "tympanum_delay.rs"]
mod delay;
#[path = "tympanum_model.rs"]
pub mod model;
#[path = "tympanum_rec.rs"]
pub mod rec;
#[path = "tympanum_room.rs"]
pub mod room;
pub use delay::{OutDelay, DELAY_GLIDE_MS, MAX_DELAY_MS};

/// Microphones (named outputs) a kit has at most: `cantus_core::inst_out::MAX_OUTPUTS`.
pub const MAX_MICS: usize = 32;
/// The length of a mic's lane: a call is cut into chunks of it, which changes no sample.
pub const LANE: usize = 256;

/// The draw of one hit, in `[0, 1)`: a pure function of the kit's seed, the key and the
/// hit's ordinal (no state, no allocation), the same for every microphone of the hit.
#[inline]
pub fn hit_draw(seed: u32, key: u8, ordinal: u32) -> f32 {
    // SplitMix32-style mixing of the three numbers: integer only, deterministic everywhere.
    let mut x = seed
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(u32::from(key).wrapping_mul(0x85EB_CA6B))
        .wrapping_add(ordinal.wrapping_mul(0xC2B2_AE35));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    // 24 bits: exact in an f32, strictly below 1.
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Which of `n` takes the hit with this `draw` and `ordinal` plays — **never the take the
/// hit before it on the same key played** (roll smoothing) and still a pure function, so a
/// render from any start chooses the same: hits alternate between the even and the odd
/// takes (ordinal parity), and draw freely inside their half. Two takes alternate; one
/// repeats (it must).
#[inline]
pub fn take_index(draw: f32, n: usize, ordinal: u32) -> usize {
    match n {
        0 | 1 => 0,
        _ => {
            let parity = (ordinal & 1) as usize;
            // Takes of this parity: parity, parity + 2, … < n.
            let count = (n - parity).div_ceil(2);
            let k = ((draw * count as f32) as usize).min(count - 1);
            parity + 2 * k
        }
    }
}

/// The constant-power law for a mono source placed at `pan` (−1 … 1): `(left, right)`,
/// −3 dB each at the centre (SFZ players' `pan`).
#[inline]
pub fn pan_law(pan: f32) -> (f32, f32) {
    let t = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let (s, c) = crate::math::sin_cos(t);
    (c, s)
}

/// What the instrument's outputs are this snapshot: each mic's mix, bus and power, and
/// the lanes of the chunk being rendered.
#[derive(Clone, Debug)]
pub struct Outputs {
    pub count: usize,
    /// Each mic's mix; a mic muted by another's solo arrives here muted.
    pub mix: [OutMix; MAX_MICS],
    /// 0 Main, k ≥ 1 the k-th distinct strip (`cantus_core::inst_out::bus_of`).
    pub bus: [u8; MAX_MICS],
    /// `o{n}.on`: off renders nothing (its files are not loaded).
    pub on: [bool; MAX_MICS],
    /// Each mic's output id (`o{n}`): which line delays it.
    pub out_id: [u8; MAX_MICS],
    /// By output id: the delay lines the unit was built with (`None`: no line).
    pub delay: Vec<Option<OutDelay>>,
    lane_l: Vec<f32>,
    lane_r: Vec<f32>,
    /// Which lanes something wrote into this chunk.
    touched: [bool; MAX_MICS],
    /// Each mic's peak (linear, after its mix) since [`Self::reset_peaks`]: the meters.
    pub peak: [f32; MAX_MICS],
}

impl Outputs {
    /// Allocates every lane, and a delay line for each output in `delays` (a bit per
    /// output id, `o{n}` → bit n: ADR 0123 §3's rule, an output whose delay is set or
    /// automated): with the unit, never on the audio thread. `Outputs::new(fs, 0)` owns
    /// no line.
    pub fn new(fs: f32, delays: u32) -> Outputs {
        Outputs {
            count: 0,
            mix: [OutMix::default(); MAX_MICS],
            bus: [0; MAX_MICS],
            on: [true; MAX_MICS],
            out_id: std::array::from_fn(|m| m as u8),
            delay: (0..MAX_MICS)
                .map(|n| (delays & (1 << n) != 0).then(|| OutDelay::new(fs)))
                .collect(),
            lane_l: vec![0.0; MAX_MICS * LANE],
            lane_r: vec![0.0; MAX_MICS * LANE],
            touched: [false; MAX_MICS],
            peak: [0.0; MAX_MICS],
        }
    }

    /// The delay lines this was built with, a bit per output id.
    pub fn delay_mask(&self) -> u32 {
        self.delay
            .iter()
            .enumerate()
            .filter(|(_, d)| d.is_some())
            .fold(0, |m, (n, _)| m | 1 << n)
    }

    /// Mic `m`'s delay line, if its output has one.
    pub fn line(&mut self, m: usize) -> Option<&mut OutDelay> {
        let id = usize::from(*self.out_id.get(m)?);
        self.delay.get_mut(id)?.as_mut()
    }

    /// Start a chunk of `c ≤ LANE` frames: every lane at +0.0, nothing touched.
    pub fn begin(&mut self, c: usize) {
        for m in 0..self.count.min(MAX_MICS) {
            self.lane_l[m * LANE..m * LANE + c].fill(0.0);
            self.lane_r[m * LANE..m * LANE + c].fill(0.0);
            self.touched[m] = false;
        }
    }

    /// Mic `m`'s lane, `c` frames, to add into.
    #[inline]
    pub fn lane(&mut self, m: usize, c: usize) -> (&mut [f32], &mut [f32]) {
        self.touched[m] = true;
        (
            &mut self.lane_l[m * LANE..m * LANE + c],
            &mut self.lane_r[m * LANE..m * LANE + c],
        )
    }

    /// Mix every lane and hand it on, in output order: bus 0 added into `main` at `at`,
    /// the others to `out(bus, at, l, r)`. Returns whether anything was given.
    pub fn emit(
        &mut self,
        c: usize,
        at: usize,
        main_l: &mut [f32],
        main_r: &mut [f32],
        out: &mut crate::pulsus::BusOut<'_>,
    ) -> bool {
        let n = self.count.min(MAX_MICS);
        let mut any = false;
        for m in 0..n {
            let id = usize::from(self.out_id[m]);
            let line = self.delay.get_mut(id).and_then(Option::as_mut);
            if !self.on[m] {
                if let Some(d) = line {
                    d.skip(c);
                }
                continue;
            }
            let had = self.touched[m];
            if !had && line.as_ref().is_none_or(|d| !d.ringing()) {
                // Nothing in, nothing to give: the line moves on as if fed silence.
                if let Some(d) = line {
                    d.skip(c);
                }
                continue;
            }
            let (l, r) = (
                &mut self.lane_l[m * LANE..m * LANE + c],
                &mut self.lane_r[m * LANE..m * LANE + c],
            );
            if let Some(d) = line {
                d.run(l, r);
            }
            self.mix[m].apply(l, r);
            let pk = l
                .iter()
                .chain(r.iter())
                .fold(self.peak[m], |a, x| a.max(x.abs()));
            self.peak[m] = pk;
            any = true;
            if self.bus[m] == 0 {
                for (o, x) in main_l[at..at + c].iter_mut().zip(l.iter()) {
                    *o += *x;
                }
                for (o, x) in main_r[at..at + c].iter_mut().zip(r.iter()) {
                    *o += *x;
                }
            } else {
                out(self.bus[m], at, l, r);
            }
        }
        any
    }

    /// One of Cantus's rooms on a recorded kit's room mics (`room` 1–3, ADR 0124 §2): the
    /// kit's other mics, summed, go into `room`, and its tail replaces what the room mics
    /// recorded (their own lanes are cleared first). `send` scales what goes in.
    pub fn room_from(
        &mut self,
        room: &mut room::Room,
        rooms: &[bool; MAX_MICS],
        c: usize,
        send: f32,
    ) {
        let n = self.count.min(MAX_MICS);
        let rooms = &rooms[..n];
        if !rooms.iter().any(|r| *r) {
            return;
        }
        for (m, _) in rooms.iter().enumerate().filter(|(_, r)| **r) {
            self.lane_l[m * LANE..m * LANE + c].fill(0.0);
            self.lane_r[m * LANE..m * LANE + c].fill(0.0);
        }
        for t in 0..c {
            let mut x = 0.0;
            for (m, _) in rooms
                .iter()
                .enumerate()
                .filter(|(m, r)| !**r && self.touched[*m])
            {
                x += self.lane_l[m * LANE + t] + self.lane_r[m * LANE + t];
            }
            let (l, r) = room.tick(0.5 * send * x);
            for (m, _) in rooms.iter().enumerate().filter(|(_, r)| **r) {
                self.lane_l[m * LANE + t] = l;
                self.lane_r[m * LANE + t] = r;
            }
        }
        let ringing = !room.at_rest();
        for (m, _) in rooms.iter().enumerate().filter(|(_, r)| **r) {
            self.touched[m] |= ringing;
        }
    }

    /// What a recorded kit's other mics send to a room (`room` 1–3, through a convolution
    /// the caller runs): their lanes summed into `l`/`r`, `send` scaled; the room mics' own
    /// lanes cleared.
    pub fn room_send(
        &mut self,
        rooms: &[bool; MAX_MICS],
        c: usize,
        send: f32,
        l: &mut [f32],
        r: &mut [f32],
    ) {
        let n = self.count.min(MAX_MICS);
        l[..c].fill(0.0);
        r[..c].fill(0.0);
        for (m, is_room) in rooms.iter().enumerate().take(n) {
            if *is_room {
                self.lane_l[m * LANE..m * LANE + c].fill(0.0);
                self.lane_r[m * LANE..m * LANE + c].fill(0.0);
            } else if self.touched[m] {
                for t in 0..c {
                    l[t] += send * self.lane_l[m * LANE + t];
                    r[t] += send * self.lane_r[m * LANE + t];
                }
            }
        }
    }

    /// The room's return on the room mics' lanes (after [`Self::room_send`]).
    pub fn room_return(
        &mut self,
        rooms: &[bool; MAX_MICS],
        c: usize,
        l: &[f32],
        r: &[f32],
        ringing: bool,
    ) {
        let n = self.count.min(MAX_MICS);
        for (m, _) in rooms.iter().enumerate().take(n).filter(|(_, x)| **x) {
            self.lane_l[m * LANE..m * LANE + c].copy_from_slice(&l[..c]);
            self.lane_r[m * LANE..m * LANE + c].copy_from_slice(&r[..c]);
            self.touched[m] |= ringing;
        }
    }

    /// Start the meters again (each block, before it renders).
    pub fn reset_peaks(&mut self) {
        self.peak = [0.0; MAX_MICS];
    }

    /// Whether some mic's delay line still holds sound.
    pub fn ringing(&self) -> bool {
        (0..self.count.min(MAX_MICS)).any(|m| {
            self.delay
                .get(usize::from(self.out_id[m]))
                .and_then(Option::as_ref)
                .is_some_and(OutDelay::ringing)
        })
    }

    /// Forget every line's tail (a reset): only the lines the unit was built with.
    pub fn clear(&mut self) {
        for d in self.delay.iter_mut().flatten() {
            d.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hits_draw_is_pure_and_inside_the_unit_interval() {
        let mut seen = std::collections::HashSet::new();
        for ordinal in 0..1000 {
            let d = hit_draw(7, 38, ordinal);
            assert!((0.0..1.0).contains(&d));
            assert_eq!(d, hit_draw(7, 38, ordinal));
            seen.insert(d.to_bits());
        }
        assert!(seen.len() > 990, "draws repeat: {}", seen.len());
        assert_ne!(hit_draw(7, 38, 0), hit_draw(8, 38, 0));
        assert_ne!(hit_draw(7, 38, 0), hit_draw(7, 36, 0));
    }

    #[test]
    fn a_take_never_repeats_the_one_before_and_every_take_is_heard() {
        for n in 2..9usize {
            let mut count = vec![0usize; n];
            let mut last = usize::MAX;
            for ordinal in 0..4000u32 {
                let t = take_index(hit_draw(3, 38, ordinal), n, ordinal);
                assert!(t < n);
                assert_ne!(t, last, "n {n} ordinal {ordinal}");
                last = t;
                count[t] += 1;
            }
            assert!(count.iter().all(|c| *c > 0), "n {n}: {count:?}");
        }
        assert_eq!(take_index(0.99, 1, 5), 0);
    }

    #[test]
    fn a_delay_at_zero_passes_through_and_a_delay_delays_whole_samples() {
        let mut d = OutDelay::new(48_000.0);
        let mut l = [0.25f32, -0.5, 0.75];
        let mut r = l;
        d.run(&mut l, &mut r);
        assert_eq!(l, [0.25, -0.5, 0.75]);
        let mut d = OutDelay::new(48_000.0);
        let (mut l, mut r) = ([0.25f32, -0.5, 0.75], [0.25f32, -0.5, 0.75]);
        d.set_ms(1.0 / 48.0, 48_000.0);
        assert_eq!(d.frames(), 1);
        d.run(&mut l, &mut r);
        assert_eq!(l, [0.0, 0.25, -0.5]);
        let (a, b) = pan_law(0.0);
        assert!((a - b).abs() < 1e-7 && (a - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }
}

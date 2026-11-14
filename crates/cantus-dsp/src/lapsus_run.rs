//! Lapsus running: the two cycles on the song, the notes at their samples, and each
//! sample's read, joins and level (ADR 0132, provisional). See `lapsus.rs` for the rules.

use super::{Cycle, Ghost, Lane, Lapsus, Note, NoteMode, JUMP, SLOTS, SNAP};
use crate::modfx_clock::{peak_db, peak_of, tel};

/// The equal-gain raised cosine: 0 at 0, 1 at 1.
#[inline]
fn rc(x: f64) -> f32 {
    (0.5 - 0.5 * crate::math::cos(std::f64::consts::PI * x.clamp(0.0, 1.0))) as f32
}

/// Where cycle `k` of `beats` quarter notes starts, in song samples (fractional), counted
/// from `origin`: the engine's own expression for a beat's sample, so a bar's line is the
/// sample the engine puts it on.
#[inline]
fn bound(k: i64, beats: f64, tempo: f64, sr: f64, origin: i64) -> f64 {
    origin as f64 + (k as f64 * beats) * 60.0 / tempo * sr
}

impl Lapsus {
    /// The cycle that holds output song sample `so` for `lane`'s slot `slot` from
    /// `origin`. Pure.
    fn cycle_of(&self, lane: Lane, slot: usize, origin: Option<i64>, so: i64) -> Cycle {
        let p = &self.banks[lane as usize][slot];
        let (tempo, sr) = (self.clock.tempo(), f64::from(self.sr));
        let o = origin.unwrap_or(0);
        let len = p.beats * 60.0 / tempo * sr;
        let at = |k: i64| bound(k, p.beats, tempo, sr, o);
        let rb = |k: i64| at(k).round() as i64;
        let mut k = ((so - o) as f64 / len).floor() as i64;
        while rb(k) > so {
            k -= 1;
        }
        while rb(k + 1) <= so {
            k += 1;
        }
        let (e0, e1) = (at(k), at(k + 1));
        let (b0, b1) = (rb(k), rb(k + 1));
        let u_first = (b1 as f64 - e1) / len;
        let (first, wrap_jump) = match lane {
            Lane::Time => {
                let ahead = self.ahead as f64;
                let d_first = (ahead + p.curve.at(u_first, self.shape) * len).max(0.0);
                let d_end = (ahead + p.curve.left_end(self.shape) * len).max(0.0);
                (d_first, (d_end - d_first).abs() > JUMP)
            }
            Lane::Vol => (p.curve.at(u_first, self.shape), false),
        };
        Cycle {
            valid: true,
            b0,
            b1,
            e0,
            len,
            first,
            wrap_jump,
        }
    }

    /// `lane`'s cycle for its slot playing, cached.
    #[inline]
    fn cycle_at(&mut self, lane: Lane, so: i64) -> Cycle {
        let l = &self.lanes[lane as usize];
        let c = l.cyc;
        if c.valid && so >= c.b0 && so < c.b1 {
            return c;
        }
        let c = self.cycle_of(lane, l.active, l.origin, so);
        self.lanes[lane as usize].cyc = c;
        if lane == Lane::Time {
            self.dc = [(i64::MIN, 0.0); 4];
        }
        c
    }

    /// The delay (samples, lookahead included, never into the future) at `so` of time
    /// slot `slot` within cycle `c`. Pure.
    #[inline]
    fn delay_in(&self, slot: usize, c: &Cycle, so: i64) -> f64 {
        let u = (so as f64 - c.e0) / c.len;
        (self.ahead as f64 + self.banks[0][slot].curve.at(u, self.shape) * c.len).max(0.0)
    }

    /// The delay at `so` for the time slot playing, through the cache.
    #[inline]
    fn delay_cached(&mut self, c: &Cycle, so: i64) -> f64 {
        for &(s, d) in &self.dc {
            if s == so {
                return d;
            }
        }
        let d = self.delay_in(self.lanes[0].active, c, so);
        self.dc = [self.dc[1], self.dc[2], self.dc[3], (so, d)];
        d
    }

    /// Whether the delay jumps at `so` (sample `t` of time cycle `c`): its step there leaves
    /// the steps on both sides — the line the last two samples drew and the one the next
    /// draws — by more than [`JUMP`]. A function of the pattern alone (the next sample's
    /// delay is the curve's, known ahead), so a skip finds the same jumps a run does. The
    /// read that would have gone on: its delay here and its step.
    fn jump_at(&mut self, c: &Cycle, so: i64, t: i64) -> Option<(f64, f64)> {
        if t < 2 || self.xf == 0 {
            return None;
        }
        let d = self.delay_cached(c, so);
        let d1 = self.delay_cached(c, so - 1);
        let d2 = self.delay_cached(c, so - 2);
        let (e0, e1) = (d - d1, d1 - d2);
        if (e0 - e1).abs() <= JUMP {
            return None;
        }
        if so + 1 < c.b1 {
            let e2 = self.delay_cached(c, so + 1) - d;
            if (e2 - e0).abs() <= JUMP {
                return None;
            }
        }
        Some((d1 + e1, e1))
    }

    /// What the time lane does at every sample before it reads, run or skipped: its cycle
    /// (whose first sample starts the joins again), a jump's join. The cycle, the place in
    /// it, the delay and the one before.
    #[inline]
    fn advance_time(&mut self, so: i64) -> (Cycle, i64, f64, f64, f64) {
        let c = self.cycle_at(Lane::Time, so);
        let t = so - c.b0;
        let u = (so as f64 - c.e0) / c.len;
        if t == 0 {
            self.ghost.on = false;
            if !self.lanes[0].overridden() {
                self.lanes[0].dirty = false;
            }
        }
        if let Some((d, slope)) = self.jump_at(&c, so, t) {
            self.ghost = Ghost {
                on: true,
                d,
                slope,
                left: self.xf,
                len: self.xf,
            };
        }
        let d = self.delay_cached(&c, so);
        let d1 = if t >= 1 {
            self.delay_cached(&c, so - 1)
        } else {
            d
        };
        (c, t, u, d, d1)
    }

    /// What the volume lane does at every sample: its cycle (whose first sample starts the
    /// glide again), the glide. The cycle and the place in it.
    #[inline]
    fn advance_vol(&mut self, so: i64) -> (Cycle, f64) {
        let c = self.cycle_at(Lane::Vol, so);
        let u = (so as f64 - c.e0) / c.len;
        let target = self.banks[1][self.lanes[1].active].curve.at(u, self.shape) as f32;
        if so == c.b0 {
            self.glide = target;
            if !self.lanes[1].overridden() {
                self.lanes[1].dirty = false;
            }
        } else {
            self.glide_to(target);
        }
        (c, u)
    }

    /// The fading read moves on a sample.
    #[inline]
    fn ghost_on(&mut self) {
        if self.ghost.on {
            self.ghost.d += self.ghost.slope;
            self.ghost.left -= 1;
            self.ghost.on = self.ghost.left > 0;
        }
    }

    /// The volume curve's level at the output's song sample now (no glide).
    pub(super) fn level_now(&self) -> f32 {
        let so = self.so_next();
        let l = &self.lanes[1];
        let c = self.cycle_of(Lane::Vol, l.active, l.origin, so);
        let u = (so as f64 - c.e0) / c.len;
        self.banks[1][l.active].curve.at(u, self.shape) as f32
    }

    /// The time read playing goes on, fading out from output song sample `so` (the sample
    /// about to be processed): where it would be there, at its speed.
    pub(super) fn start_ghost_at(&mut self, so: i64) {
        if self.xf == 0 {
            return;
        }
        let l = self.lanes[0];
        let c1 = self.cycle_of(Lane::Time, l.active, l.origin, so - 1);
        let c2 = self.cycle_of(Lane::Time, l.active, l.origin, so - 2);
        let d1 = self.delay_in(l.active, &c1, so - 1);
        let d2 = self.delay_in(l.active, &c2, so - 2);
        let slope = if c1.b0 == c2.b0 { d1 - d2 } else { 0.0 };
        self.ghost = Ghost {
            on: true,
            d: d1 + slope,
            slope,
            left: self.xf,
            len: self.xf,
        };
    }

    /// One note acting now (`so`: the output's song sample), on each bank whose range
    /// holds it.
    pub(super) fn apply(&mut self, note: Note, so: i64) {
        for lane in Lane::ALL {
            self.apply_lane(lane, note, so);
        }
    }

    fn apply_lane(&mut self, lane: Lane, note: Note, so: i64) {
        let mode = self.set.notes;
        let key = i32::from(self.set.key(lane));
        let li = lane as usize;
        let before = self.lanes[li].origin;
        let slot = |pitch: u8| {
            let k = i32::from(pitch) - key;
            (0..SLOTS as i32).contains(&k).then_some(k as u8)
        };
        match note {
            Note::On(pitch) => {
                let Some(k) = slot(pitch) else { return };
                if mode == NoteMode::Off {
                    return;
                }
                let l = &mut self.lanes[li];
                match mode {
                    NoteMode::Switch => l.latched = Some(k),
                    _ => {
                        unhold(l, k);
                        l.held[l.held_n] = k;
                        l.held_n += 1;
                    }
                }
                if self.set.retrig {
                    l.origin = Some(so);
                }
            }
            Note::Off(pitch) => {
                let Some(k) = slot(pitch) else { return };
                let l = &mut self.lanes[li];
                if mode == NoteMode::Hold {
                    unhold(l, k);
                    if l.held_n == 0 {
                        l.origin = None;
                    }
                }
            }
            Note::AllOff => {
                let l = &mut self.lanes[li];
                l.held_n = 0;
                l.origin = None;
            }
        }
        if self.lanes[li].origin != before {
            // The pattern starts again here (or back on the song's grid): fade into it.
            if lane == Lane::Time {
                self.start_ghost_at(so);
            }
            self.invalidate(lane);
            self.lanes[li].dirty = true;
        }
        self.choose_at(lane, so);
    }

    /// The notes due at stream sample `n`, in the order they came.
    #[inline]
    fn apply_due(&mut self, n: i64, so: i64) {
        while self.note_n > 0 && self.notes[0].0 <= n {
            let (_, note) = self.notes[0];
            self.notes.copy_within(1..self.note_n, 0);
            self.note_n -= 1;
            self.apply(note, so);
        }
    }

    /// The level's glide one step towards `target`.
    #[inline]
    fn glide_to(&mut self, target: f32) {
        let next = self.glide + (target - self.glide) * self.kv;
        self.glide = if next == self.glide || (next - target).abs() < SNAP {
            target
        } else {
            next
        };
    }

    /// Whether a block may pass untouched: both patterns change nothing, nothing fades, no
    /// lookahead, mix and output at unity, no note waiting.
    fn passes(&self) -> bool {
        self.ahead == 0
            && !self.ghost.on
            && self.note_n == 0
            && self.mix.still()
            && self.mix.value == 1.0
            && self.out.still()
            && self.out.value == 1.0
            && self.glide == 1.0
            && self.banks[0][self.lanes[0].active].is_through(Lane::Time)
            && self.banks[1][self.lanes[1].active].is_through(Lane::Vol)
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        if self.passes() {
            for i in 0..n {
                self.hist.write(self.clock.stream(i), l[i], r[i]);
            }
            self.rest_cycles(n);
        } else {
            for i in 0..n {
                let (a, b) = self.tick(i, l[i], r[i]);
                l[i] = a;
                r[i] = b;
            }
        }
        self.clock.advance(n);
        self.telemetry_after(n, in_peak, peak_of(&l[..n], &r[..n]));
    }

    /// A block that ran without ticks (passing, or skipped): a lane whose cycle began in it
    /// clears what was not its pattern's own.
    fn rest_cycles(&mut self, frames: usize) {
        if frames == 0 {
            return;
        }
        let so = self.so_next();
        for lane in Lane::ALL {
            let li = lane as usize;
            if self.lanes[li].dirty && !self.lanes[li].overridden() {
                let c = self.cycle_at(lane, so);
                if so == c.b0 || so + frames as i64 > c.b1 {
                    self.lanes[li].dirty = false;
                }
            }
        }
    }

    /// One sample.
    #[inline]
    fn tick(&mut self, i: usize, xl: f32, xr: f32) -> (f32, f32) {
        let n = self.clock.stream(i);
        let so = self.clock.song(i) - self.ahead as i64;
        self.apply_due(n, so);
        self.hist.write(n, xl, xr);
        let (c, t, u, d, d1) = self.advance_time(so);
        let (cv, uv) = self.advance_vol(so);
        // The level, towards the next volume cycle's first in its last moments (no longer
        // than a time join, so a level rising on a bar's line rises with the read that
        // joins there, not before it).
        let mut v = self.glide;
        let to_end_v = cv.b1 - so;
        let fv = if self.xf > 0 {
            self.fv.min(self.xf)
        } else {
            self.fv
        };
        if fv > 0 && to_end_v <= i64::from(fv) && v != cv.first as f32 {
            let f = f64::from(fv);
            v += (cv.first as f32 - v) * rc((f - to_end_v as f64 + 1.0) / (f + 1.0));
        }
        // The read: the main one, the join fading out, the next cycle's first read fading
        // in before the line.
        let lo = n - self.ahead as i64 - t;
        let speed = (1.0 - (d - d1)) as f32;
        let (mut yl, mut yr) = self.hist.read(n, d, lo, speed);
        if self.ghost.on {
            let g = self.ghost;
            let (gl, gr) = self.hist.read(n, g.d.max(0.0), lo, (1.0 - g.slope) as f32);
            let len = f64::from(g.len);
            let w = rc((len - f64::from(g.left) + 1.0) / (len + 1.0));
            yl = gl * (1.0 - w) + yl * w;
            yr = gr * (1.0 - w) + yr * w;
        }
        self.ghost_on();
        let to_end = c.b1 - so;
        if c.wrap_jump && self.xf > 0 && to_end <= i64::from(self.xf) {
            let f = f64::from(self.xf);
            let w = rc((f - to_end as f64 + 1.0) / (f + 1.0));
            let (nl, nr) = self.hist.read(n, c.first, lo, 1.0);
            yl = yl * (1.0 - w) + nl * w;
            yr = yr * (1.0 - w) + nr * w;
        }
        if v != 1.0 {
            yl *= v;
            yr *= v;
        }
        let (dl, dr) = if self.ahead == 0 {
            (xl, xr)
        } else {
            self.hist.at(n, self.ahead as i64, i64::MIN)
        };
        let m = self.mix.next();
        let (mut ol, mut or) = if m == 1.0 {
            (yl, yr)
        } else if m == 0.0 {
            (dl, dr)
        } else {
            (dl + (yl - dl) * m, dr + (yr - dr) * m)
        };
        let g = self.out.next();
        if g != 1.0 {
            ol *= g;
            or *= g;
        }
        self.last = [
            u as f32,
            ((d - self.ahead as f64) / c.len) as f32,
            v,
            speed,
            uv as f32,
        ];
        (ol, or)
    }

    /// `frames` of silence were not processed (at rest): the level's glide and the time
    /// lane's joins run as they would have.
    pub fn skip(&mut self, frames: usize) {
        // Where the level is flat and landed, only the last joins' worth of samples can
        // leave anything behind (a join lasts `xfade`); elsewhere every sample glides.
        let vflat = self.banks[1][self.lanes[1].active].curve.constant() == Some(self.glide);
        let w = (self.xf as usize + 1).min(frames);
        if frames > w {
            self.ghost.on = false;
        }
        let from = if vflat { frames - w } else { 0 };
        for i in from..frames {
            let so = self.clock.song(i) - self.ahead as i64;
            if i >= frames - w {
                self.advance_time(so);
                self.ghost_on();
            }
            if !vflat {
                self.advance_vol(so);
            }
        }
        self.hist.skip(frames);
        self.clock.skip(frames);
    }

    fn telemetry_after(&mut self, n: usize, in_peak: f32, out_peak: f32) {
        self.blocks = (self.blocks + 1) % (1 << 24);
        let tbeats = self.banks[0][self.lanes[0].active].beats;
        let vbeats = self.banks[1][self.lanes[1].active].beats;
        let held = self.lanes.iter().any(|l| l.overridden());
        let mut t = [0.0f32; 24];
        t[tel::SAMPLE_RATE] = self.sr;
        t[tel::TEMPO] = self.clock.tempo() as f32;
        t[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        let [phase, delay, level, speed, vphase] = if n == 0 || self.passes() {
            let (a, b) = self.phases_now();
            [a, 0.0, 1.0, 1.0, b]
        } else {
            self.last
        };
        t[tel::PHASE_L] = phase;
        t[tel::PHASE_R] = vphase;
        t[tel::MOD_L] = delay;
        t[tel::MOD_R] = level;
        t[tel::IN_PEAK] = peak_db(in_peak);
        t[tel::OUT_PEAK] = peak_db(out_peak);
        t[tel::BLOCKS] = self.blocks as f32;
        t[super::t::SLOT] = self.lanes[0].active as f32;
        t[super::t::PHASE] = phase;
        t[super::t::DELAY] = delay;
        t[super::t::LEVEL] = level;
        t[super::t::SPEED] = speed;
        t[super::t::BEATS] = tbeats as f32;
        t[super::t::NOTE] = if held { 1.0 } else { 0.0 };
        t[super::t::KNOB] = self.set.tslot as f32;
        t[super::t::HELD_S] = self.hist.frames() as f32 / self.sr;
        t[super::t::JOIN] = if self.ghost.on { 1.0 } else { 0.0 };
        t[super::t::VSLOT] = self.lanes[1].active as f32;
        t[super::t::VPHASE] = vphase;
        t[super::t::VBEATS] = vbeats as f32;
        t[super::t::VKNOB] = self.set.vslot as f32;
        self.tel = t;
    }

    /// Where in each cycle the last sample was (0 … 1), without a tick.
    fn phases_now(&mut self) -> (f32, f32) {
        let so = self.so_next() - 1;
        let ct = self.cycle_at(Lane::Time, so);
        let cv = self.cycle_at(Lane::Vol, so);
        (
            ((so as f64 - ct.e0) / ct.len) as f32,
            ((so as f64 - cv.e0) / cv.len) as f32,
        )
    }

    /// The telemetry: the family's header and Lapsus's own ([`super::t`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..24].copy_from_slice(&self.tel);
    }
}

fn unhold(l: &mut super::LaneState, k: u8) {
    if let Some(i) = l.held[..l.held_n].iter().position(|&h| h == k) {
        l.held.copy_within(i + 1..l.held_n, i);
        l.held_n -= 1;
    }
}

#[cfg(test)]
impl Lapsus {
    /// [`Lapsus::process`] sample by sample even where a block could pass untouched: the
    /// null of the full path.
    pub(super) fn process_by_ticks(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        for i in 0..n {
            let (a, b) = self.tick(i, l[i], r[i]);
            l[i] = a;
            r[i] = b;
        }
        self.clock.advance(n);
    }
}

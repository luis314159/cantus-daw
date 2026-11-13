//! Lapsus running: the cycle on the song, the notes at their samples, and each sample's
//! read, joins and level (ADR 0132, provisional). See `lapsus.rs` for the rules.

use super::{Cycle, Ghost, Lapsus, Note, NoteMode, JUMP, SLOTS, SNAP};
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
    /// The cycle that holds output song sample `so` for slot `slot` from `origin`. Pure.
    fn cycle_of(&self, slot: usize, origin: Option<i64>, so: i64) -> Cycle {
        let p = &self.bank[slot];
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
        let ahead = self.ahead as f64;
        let u_first = (b1 as f64 - e1) / len;
        let d_first = (ahead + p.time.at(u_first, self.shape) * len).max(0.0);
        let d_end = (ahead + p.time.left_end(self.shape) * len).max(0.0);
        Cycle {
            valid: true,
            b0,
            b1,
            e0,
            len,
            d_first,
            v_first: p.vol.at(u_first, self.shape) as f32,
            wrap_jump: (d_end - d_first).abs() > JUMP,
        }
    }

    /// The cycle of the slot playing, cached.
    #[inline]
    fn cycle_at(&mut self, so: i64) -> Cycle {
        let c = self.cyc;
        if c.valid && so >= c.b0 && so < c.b1 {
            return c;
        }
        self.cyc = self.cycle_of(self.active, self.origin, so);
        self.dc = [(i64::MIN, 0.0); 2];
        self.cyc
    }

    /// The delay (samples, lookahead included, never into the future) at `so` of `slot`
    /// within cycle `c`. Pure.
    #[inline]
    fn delay_in(&self, slot: usize, c: &Cycle, so: i64) -> f64 {
        let u = (so as f64 - c.e0) / c.len;
        (self.ahead as f64 + self.bank[slot].time.at(u, self.shape) * c.len).max(0.0)
    }

    /// The delay at `so` for the slot playing, through the two-sample cache.
    #[inline]
    fn delay_cached(&mut self, c: &Cycle, so: i64) -> f64 {
        for &(s, d) in &self.dc {
            if s == so {
                return d;
            }
        }
        let d = self.delay_in(self.active, c, so);
        self.dc = [self.dc[1], (so, d)];
        d
    }

    /// The volume curve's level at the output's song sample now (no glide).
    pub(super) fn level_now(&self) -> f32 {
        let so = self.clock.song(0) - self.ahead as i64;
        let c = self.cycle_of(self.active, self.origin, so);
        let u = (so as f64 - c.e0) / c.len;
        self.bank[self.active].vol.at(u, self.shape) as f32
    }

    /// The read playing goes on, fading out from output song sample `so` (the sample about
    /// to be processed): where it would be there, at its speed, at its level.
    pub(super) fn start_ghost_at(&mut self, so: i64) {
        if self.xf == 0 {
            return;
        }
        let c1 = self.cycle_of(self.active, self.origin, so - 1);
        let c2 = self.cycle_of(self.active, self.origin, so - 2);
        let d1 = self.delay_in(self.active, &c1, so - 1);
        let d2 = self.delay_in(self.active, &c2, so - 2);
        let slope = if c1.b0 == c2.b0 { d1 - d2 } else { 0.0 };
        self.ghost = Ghost {
            on: true,
            d: d1 + slope,
            slope,
            vol: self.glide,
            left: self.xf,
            len: self.xf,
        };
    }

    /// One note acting now (`so`: the output's song sample).
    pub(super) fn apply(&mut self, note: Note, so: i64) {
        let mode = self.set.notes;
        let before = self.origin;
        match note {
            Note::On(pitch) => {
                let k = i32::from(pitch) - i32::from(self.set.key);
                if mode == NoteMode::Off || !(0..SLOTS as i32).contains(&k) {
                    return;
                }
                let k = k as u8;
                match mode {
                    NoteMode::Switch => self.latched = Some(k),
                    _ => {
                        self.unhold(k);
                        self.held[self.held_n] = k;
                        self.held_n += 1;
                    }
                }
                if self.set.retrig {
                    self.origin = Some(so);
                }
            }
            Note::Off(pitch) => {
                let k = i32::from(pitch) - i32::from(self.set.key);
                if mode == NoteMode::Hold && (0..SLOTS as i32).contains(&k) {
                    self.unhold(k as u8);
                    if self.held_n == 0 {
                        self.origin = None;
                    }
                }
            }
            Note::AllOff => {
                self.held_n = 0;
                self.origin = None;
            }
        }
        if self.origin != before {
            // The pattern starts again here (or back on the song's grid): fade into it.
            self.start_ghost_at(so);
            self.invalidate();
            self.dirty = true;
        }
        self.choose_at(so);
    }

    fn unhold(&mut self, k: u8) {
        if let Some(i) = self.held[..self.held_n].iter().position(|&h| h == k) {
            self.held.copy_within(i + 1..self.held_n, i);
            self.held_n -= 1;
        }
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

    /// Whether a block may pass untouched: the pattern changes nothing, nothing fades, no
    /// lookahead, mix and output at unity, no note waiting.
    fn passes(&self) -> bool {
        self.ahead == 0
            && !self.ghost.on
            && self.note_n == 0
            && self.mix.still()
            && self.mix.value == 1.0
            && self.out.still()
            && self.out.value == 1.0
            && self.bank[self.active].is_through()
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        if self.passes() {
            for i in 0..n {
                self.hist.write(self.clock.stream(i), l[i], r[i]);
            }
            self.glide = 1.0;
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

    /// A block that ran without ticks (passing, or skipped): a cycle that began in it
    /// clears what was not the pattern's own.
    fn rest_cycles(&mut self, frames: usize) {
        if self.dirty && !self.overridden() && frames > 0 {
            let so = self.clock.song(0) - self.ahead as i64;
            let c = self.cycle_at(so);
            if so == c.b0 || so + frames as i64 > c.b1 {
                self.dirty = false;
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
        let c = self.cycle_at(so);
        let t = so - c.b0;
        let u = (so as f64 - c.e0) / c.len;
        let slot = self.active;
        if t == 0 {
            // The cycle begins again: only the pattern counts from here.
            self.glide = self.bank[slot].vol.at(u, self.shape) as f32;
            self.ghost.on = false;
            if !self.overridden() {
                self.dirty = false;
            }
        }
        let d = self.delay_cached(&c, so);
        let d1 = if t >= 1 { self.delay_cached(&c, so - 1) } else { d };
        // A jump: the delay leaves the line its last two samples drew (the sample after a
        // jump is measured across it, and is not one).
        if t >= 2 && self.xf > 0 && self.jumped != so - 1 {
            let d2 = self.delay_in(slot, &c, so - 2);
            let pred = 2.0 * d1 - d2;
            if (d - pred).abs() > JUMP {
                self.jumped = so;
                self.ghost = Ghost {
                    on: true,
                    d: pred,
                    slope: d1 - d2,
                    vol: self.glide,
                    left: self.xf,
                    len: self.xf,
                };
            }
        }
        let target = self.bank[slot].vol.at(u, self.shape) as f32;
        self.glide_to(target);
        let to_end = c.b1 - so;
        let mut v = self.glide;
        // Towards the next cycle's level where the time does not jump there (where it does,
        // the next read comes in at its own level, below).
        let joins = c.wrap_jump && self.xf > 0;
        if !joins && self.fv > 0 && to_end <= i64::from(self.fv) && v != c.v_first {
            let f = f64::from(self.fv);
            v += (c.v_first - v) * rc((f - to_end as f64 + 1.0) / (f + 1.0));
        }
        let lo = n - self.ahead as i64 - t;
        let speed = (1.0 - (d - d1)) as f32;
        let (ml, mr) = self.hist.read(n, d, lo, speed);
        let (mut yl, mut yr) = if v == 1.0 { (ml, mr) } else { (ml * v, mr * v) };
        if self.ghost.on {
            let g = self.ghost;
            let (gl, gr) = self.hist.read(n, g.d.max(0.0), lo, (1.0 - g.slope) as f32);
            let len = f64::from(g.len);
            let w = rc((len - f64::from(g.left) + 1.0) / (len + 1.0));
            yl = gl * g.vol * (1.0 - w) + yl * w;
            yr = gr * g.vol * (1.0 - w) + yr * w;
            self.ghost.d += g.slope;
            self.ghost.left -= 1;
            self.ghost.on = self.ghost.left > 0;
        }
        if joins && to_end <= i64::from(self.xf) {
            let f = f64::from(self.xf);
            let w = rc((f - to_end as f64 + 1.0) / (f + 1.0));
            let (nl, nr) = self.hist.read(n, c.d_first, lo, 1.0);
            yl = yl * (1.0 - w) + nl * c.v_first * w;
            yr = yr * (1.0 - w) + nr * c.v_first * w;
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
        ];
        (ol, or)
    }

    /// `frames` of silence were not processed (at rest): the level's glide and the cycle
    /// run as they would have.
    pub fn skip(&mut self, frames: usize) {
        let flat = self.bank[self.active].vol.constant();
        if flat == Some(self.glide) {
            self.rest_cycles(frames);
        } else {
            for i in 0..frames {
                let so = self.clock.song(i) - self.ahead as i64;
                let c = self.cycle_at(so);
                let u = (so as f64 - c.e0) / c.len;
                let target = self.bank[self.active].vol.at(u, self.shape) as f32;
                if so == c.b0 {
                    self.glide = target;
                    if !self.overridden() {
                        self.dirty = false;
                    }
                } else {
                    self.glide_to(target);
                }
            }
        }
        self.hist.skip(frames);
        self.clock.skip(frames);
    }

    fn telemetry_after(&mut self, n: usize, in_peak: f32, out_peak: f32) {
        self.blocks = (self.blocks + 1) % (1 << 24);
        let beats = self.bank[self.active].beats;
        let held = self.overridden();
        let mut t = [0.0f32; 24];
        t[tel::SAMPLE_RATE] = self.sr;
        t[tel::TEMPO] = self.clock.tempo() as f32;
        t[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        let [phase, delay, level, speed] = if n == 0 || self.passes() {
            [self.phase_now(), 0.0, 1.0, 1.0]
        } else {
            self.last
        };
        t[tel::PHASE_L] = phase;
        t[tel::PHASE_R] = phase;
        t[tel::MOD_L] = delay;
        t[tel::MOD_R] = level;
        t[tel::IN_PEAK] = peak_db(in_peak);
        t[tel::OUT_PEAK] = peak_db(out_peak);
        t[tel::BLOCKS] = self.blocks as f32;
        t[super::t::SLOT] = self.active as f32;
        t[super::t::PHASE] = phase;
        t[super::t::DELAY] = delay;
        t[super::t::LEVEL] = level;
        t[super::t::SPEED] = speed;
        t[super::t::BEATS] = beats as f32;
        t[super::t::NOTE] = if held { 1.0 } else { 0.0 };
        t[super::t::KNOB] = self.set.slot as f32;
        t[super::t::HELD_S] = self.hist.frames() as f32 / self.sr;
        t[super::t::JOIN] = if self.ghost.on { 1.0 } else { 0.0 };
        self.tel = t;
    }

    /// Where in the cycle the next sample is (0 … 1), without a tick.
    fn phase_now(&mut self) -> f32 {
        let so = self.clock.song(0) - self.ahead as i64 - 1;
        let c = self.cycle_at(so);
        ((so as f64 - c.e0) / c.len) as f32
    }

    /// The telemetry: the family's header and Lapsus's own ([`super::t`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..24].copy_from_slice(&self.tel);
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

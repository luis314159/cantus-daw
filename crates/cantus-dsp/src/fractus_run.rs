//! Fractus running: the notes at their samples, each sample's events, the read with its
//! joins, the processors, the cloud, the mix; the skip at rest; the telemetry.

use super::fx::env;
use super::read::{self, rc, Ctx, Id, Rd};
use super::seq::Grid;
use super::{t, Ev, Fractus, Module, Note, LANES};
use crate::modfx_clock::{peak_db, peak_of, tel};

impl Fractus {
    fn grid(&self) -> Grid {
        Grid::of(&self.set, self.clock.tempo(), f64::from(self.sr))
    }

    fn ctx(&self, g: &Grid) -> Ctx {
        Ctx {
            spb: g.spb(),
            step: g.step * g.spb(),
            xf: self.xf,
            seed: self.set.seed,
        }
    }

    /// One note acting at song sample `so`.
    pub(super) fn apply_note(&mut self, note: Note, so: i64) {
        let g = self.grid();
        let key = i32::from(self.set.key);
        let q = self.set.quantise;
        let lane = |p: u8| {
            let k = i32::from(p) - key;
            (0..LANES as i32).contains(&k).then_some(k as usize)
        };
        match note {
            Note::On(p, v) => {
                if let Some(l) = lane(p) {
                    if v == 0 {
                        self.seq.note_off(l, so, self.xf, q, &g);
                    } else {
                        self.seq.note_on(l, so, v, q, &g);
                    }
                }
            }
            Note::Off(p) => {
                if let Some(l) = lane(p) {
                    self.seq.note_off(l, so, self.xf, q, &g);
                }
            }
            Note::AllOff => self.seq.all_off(so, self.xf),
        }
    }

    /// The notes due at stream sample `n`, in the order they came.
    #[inline]
    fn apply_due(&mut self, n: i64, so: i64) {
        while self.note_n > 0 && self.notes[0].0 <= n {
            let (_, note) = self.notes[0];
            self.notes.copy_within(1..self.note_n, 0);
            self.note_n -= 1;
            self.apply_note(note, so);
        }
    }

    /// Whether a block passes untouched: nothing can play, nothing fades or rings, the
    /// cloud is off and the glides are still at unity.
    fn passes(&self) -> bool {
        let no_steps = !self.set.trigger.steps() || self.pat.is_empty();
        no_steps
            && self.note_n == 0
            && !self.seq.notes_held()
            && !self.reader.fading()
            && self.reader.prev.0 == Id::DRY
            && !self.cloud.enabled()
            && self.fx.silent_tail()
            && self.mix.still()
            && self.out.still()
            && self.out.value == 1.0
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let g = self.grid();
        if self.passes() {
            for i in 0..n {
                let (k, so) = (self.clock.stream(i), self.clock.song(i));
                self.hist.write(k, l[i], r[i]);
                self.cap.write(k, l[i], r[i]);
                self.cap_head = k;
                self.fx.feed([l[i], r[i]]);
                self.cloud.tick([l[i], r[i]], so);
            }
            self.act = 0;
            self.read_lane = 0;
        } else {
            let c = self.ctx(&g);
            for i in 0..n {
                let [a, b] = self.tick(i, [l[i], r[i]], &g, &c);
                l[i] = a;
                r[i] = b;
            }
        }
        self.clock.advance(n);
        self.telemetry_after(n, in_peak, peak_of(&l[..n], &r[..n]), &g);
    }

    /// The events of every lane at `so`.
    #[inline]
    fn events(&mut self, so: i64, g: &Grid) -> [Option<Ev>; LANES] {
        self.seq.update(so, &self.set, &self.pat, g);
        let trig = self.set.trigger;
        let mut evs = [None; LANES];
        for (lane, e) in evs.iter_mut().enumerate() {
            *e = self.seq.event(lane, so, trig);
        }
        evs
    }

    /// One sample.
    #[inline]
    fn tick(&mut self, i: usize, x: [f32; 2], g: &Grid, c: &Ctx) -> [f32; 2] {
        let n = self.clock.stream(i);
        let so = self.clock.song(i);
        self.apply_due(n, so);
        let evs = self.events(so, g);
        self.hist.write(n, x[0], x[1]);
        let r0 = read::repeat_slice(&self.set.repeat, c.spb);
        let holding = evs[Module::Repeat.lane()].is_some_and(|e| so - e.start >= r0);
        if !holding {
            self.cap.write(n, x[0], x[1]);
            self.cap_head = n;
        }
        let y = self.read_stage(&evs, so, n, x, c);
        let mut w = self.fx.run(y, &evs, so, g, self.xf);
        let cl = self.cloud.tick(x, so);
        if let Some(e) = evs[Module::Cloud.lane()] {
            let k = env(&e, so, self.xf);
            w = [w[0] + (cl[0] - w[0]) * k, w[1] + (cl[1] - w[1]) * k];
        }
        let lv = self.set.cloud.level;
        if lv > 0.0 {
            w = [w[0] + cl[0] * lv, w[1] + cl[1] * lv];
        }
        let mut act = 0u32;
        for (lane, e) in evs.iter().enumerate() {
            if e.is_some() {
                act |= 1 << lane;
            }
        }
        self.act = act;
        let m = self.mix.next();
        let mut o = if m == 1.0 {
            w
        } else if m == 0.0 {
            x
        } else {
            [x[0] + (w[0] - x[0]) * m, x[1] + (w[1] - x[1]) * m]
        };
        let gain = self.out.next();
        if gain != 1.0 {
            o = [o[0] * gain, o[1] * gain];
        }
        o
    }

    /// The read module of `lane` for `e` at `so`.
    #[inline]
    fn module_read(&self, lane: usize, e: &Ev, so: i64, c: &Ctx) -> Rd {
        match lane {
            0 => read::repeat(e, so, &self.set.repeat, c),
            1 => read::reverse(e, so, &self.set.reverse, c),
            2 => read::tape(e, so, &self.set.tape, c),
            _ => read::shuffle(e, so, &self.set.shuffle, c),
        }
    }

    /// The read playing at `so` — the newest read event's (ties: the lower lane), the live
    /// signal without one — and, in its last `xfade` before its end, the read after it.
    #[inline]
    fn choose(&self, evs: &[Option<Ev>; LANES], so: i64, c: &Ctx) -> ((Id, Rd), Option<(Id, Rd, f32)>) {
        let mut win: Option<(usize, Ev)> = None;
        let mut run: Option<(usize, Ev)> = None;
        for m in Module::READS {
            let l = m.lane();
            if let Some(e) = evs[l] {
                if win.is_none_or(|(_, w)| e.start > w.start) {
                    run = win;
                    win = Some((l, e));
                } else if run.is_none_or(|(_, r)| e.start > r.start) {
                    run = Some((l, e));
                }
            }
        }
        let read_of = |p: Option<(usize, Ev)>| match p {
            Some((l, e)) => (
                Id {
                    lane: l as u8,
                    start: e.start,
                },
                self.module_read(l, &e, so, c),
            ),
            None => (Id::DRY, Rd::DRY),
        };
        let main = read_of(win);
        let fade = win.and_then(|(_, e)| {
            if e.end == i64::MAX {
                return None;
            }
            let x = i64::from(c.xf).min((e.end - e.start) / 2).max(1);
            let to_end = e.end - so;
            (to_end <= x).then(|| {
                let w = rc((x - to_end + 1) as f64 / (x + 1) as f64);
                let (fid, frd) = read_of(run);
                (fid, frd, w)
            })
        });
        (main, fade)
    }

    fn read_stage(&mut self, evs: &[Option<Ev>; LANES], so: i64, n: i64, x: [f32; 2], c: &Ctx) -> [f32; 2] {
        let ((id, rd), fade) = self.choose(evs, so, c);
        self.read_lane = if id == Id::DRY { 0 } else { id.lane + 1 };
        self.reader.join(id, &rd, fade, c.xf);
        let mut v = self.value(&rd, n, x);
        if let Some((_, frd, w)) = fade {
            let f = self.value(&frd, n, x);
            v = [v[0] + (f[0] - v[0]) * w, v[1] + (f[1] - v[1]) * w];
        }
        if self.reader.ghost.on {
            let gw = self.reader.ghost_weight();
            let gv = self.value(&self.reader.ghost.rd, n, x);
            v = [v[0] + (gv[0] - v[0]) * gw, v[1] + (gv[1] - v[1]) * gw];
        }
        self.reader.ghost_on();
        v
    }

    /// What `rd` reads at stream sample `n`, mixed with the live `x` by its amount.
    #[inline]
    fn value(&self, rd: &Rd, n: i64, x: [f32; 2]) -> [f32; 2] {
        if rd.is_dry() {
            return x;
        }
        let s = if rd.gain == 0.0 {
            [0.0; 2]
        } else {
            let sp = rd.speed as f32;
            let (a, b) = if rd.cap {
                let d = (self.cap_head - n) as f64 + rd.d;
                self.cap.read(self.cap_head, d, i64::MIN, sp)
            } else {
                self.hist.read(n, rd.d, i64::MIN, sp)
            };
            if rd.gain == 1.0 {
                [a, b]
            } else {
                [a * rd.gain, b * rd.gain]
            }
        };
        if rd.amt >= 1.0 {
            s
        } else {
            [x[0] + (s[0] - x[0]) * rd.amt, x[1] + (s[1] - x[1]) * rd.amt]
        }
    }

    /// `frames` of silence were not processed (at rest): the capture's head, the joins,
    /// the processors' holds and the cloud's grains as processing silence leaves them.
    pub fn skip(&mut self, frames: usize) {
        if frames == 0 {
            return;
        }
        let g = self.grid();
        let c = self.ctx(&g);
        let (n0, so0) = (self.clock.stream(0), self.clock.song(0));
        let last = so0 + frames as i64 - 1;
        let evs = self.events(last, &g);
        // The capture is written until a repeat holds its slice.
        let r0 = read::repeat_slice(&self.set.repeat, c.spb);
        match evs[Module::Repeat.lane()] {
            Some(e) if last - e.start >= r0 => {
                let held_from = e.start + r0;
                if held_from > so0 {
                    self.cap_head = n0 + (held_from - 1 - so0);
                }
            }
            _ => self.cap_head = n0 + frames as i64 - 1,
        }
        self.hist.skip(frames);
        self.cap.skip(frames);
        // The joins: only the last few samples can leave one running.
        let w = (2 * self.xf as usize + 2).min(frames);
        if frames > w {
            self.reader.ghost.on = false;
        }
        for i in frames - w..frames {
            let so = so0 + i as i64;
            let evs = self.events(so, &g);
            let ((id, rd), fade) = self.choose(&evs, so, &c);
            self.reader.join(id, &rd, fade, c.xf);
            self.reader.ghost_on();
        }
        self.fx.skip(frames);
        self.fx.skip_crush_to(evs[Module::Crush.lane()], last);
        self.fx.skip_filter_to(evs[Module::Filter.lane()]);
        self.cloud.skip(frames, so0);
        self.clock.skip(frames);
    }

    fn telemetry_after(&mut self, n: usize, in_peak: f32, out_peak: f32, g: &Grid) {
        self.blocks = (self.blocks + 1) % (1 << 24);
        let so = self.clock.song(0) - 1;
        let steps = g.steps as i64;
        let k = g.step_of(so);
        let cyc = k.div_euclid(steps) * steps;
        let (c0, c1) = (g.line(cyc), g.line(cyc + steps));
        let phase = ((so - c0) as f64 / (c1 - c0).max(1) as f64) as f32;
        let mut tl = [0.0f32; 64];
        tl[tel::SAMPLE_RATE] = self.sr;
        tl[tel::TEMPO] = self.clock.tempo() as f32;
        tl[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        tl[tel::PHASE_L] = phase;
        tl[tel::PHASE_R] = phase;
        tl[tel::IN_PEAK] = peak_db(in_peak);
        tl[tel::OUT_PEAK] = peak_db(out_peak);
        tl[tel::BLOCKS] = self.blocks as f32;
        tl[t::STEP] = k.rem_euclid(steps) as f32;
        tl[t::STEPS] = steps as f32;
        tl[t::PHASE] = phase;
        tl[t::ACTIVE] = if n == 0 { 0.0 } else { self.act as f32 };
        tl[t::NOTES] = self.seq.note_bits(so) as f32;
        tl[t::GRAINS] = self.cloud.live() as f32;
        tl[t::FROZEN] = if self.cloud.frozen() { 1.0 } else { 0.0 };
        tl[t::READ] = f32::from(self.read_lane);
        let mut shown = [(0.0f32, 0.0f32); t::SHOWN];
        let m = self.cloud.shown(&mut shown);
        for (j, (p, s)) in shown.iter().take(m).enumerate() {
            tl[t::GRAIN0 + 2 * j] = *p;
            tl[t::GRAIN0 + 2 * j + 1] = *s;
        }
        self.tel = tl;
    }
}

#[cfg(test)]
impl Fractus {
    /// [`Fractus::process`] sample by sample even where a block could pass untouched: the
    /// null of the full path.
    pub(super) fn process_by_ticks(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let g = self.grid();
        let c = self.ctx(&g);
        for i in 0..n {
            let [a, b] = self.tick(i, [l[i], r[i]], &g, &c);
            l[i] = a;
            r[i] = b;
        }
        self.clock.advance(n);
    }
}

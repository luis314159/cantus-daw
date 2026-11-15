//! Cedo running: each sample's amount, the delayed audio through the band split, the
//! meter; rest, skip and reset. See `cedo.rs` for the rules.

use super::{Cedo, CedoMeter, Listen, Trigger};
use crate::cedo::env::Envelope;
use crate::modfx_clock::peak_db;

impl Cedo {
    /// Process in place. `key` is the sidechain from another strip, if one is routed
    /// (Sidechain trigger only).
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len());
        let mut m = CedoMeter::default();
        let mut g_min = [1.0f32; 2];
        let mut in_pk = 0.0f32;
        let mut out_pk = 0.0f32;
        let mut key_pk = 0.0f32;
        let mut notes_done = 0;
        let mut last_g = [1.0f32; 2];
        let mut last_a = 0.0f32;
        for i in 0..n {
            let t = self.clock.stream(i);
            while notes_done < self.note_n && self.notes[notes_done] <= t {
                self.pump.fire(&self.clock, self.notes[notes_done]);
                notes_done += 1;
            }
            let a = match self.s.trigger {
                Trigger::Sidechain => {
                    let k = match key {
                        Some((kl, kr)) => [
                            kl.get(i).copied().unwrap_or(0.0),
                            kr.get(i).copied().unwrap_or(0.0),
                        ],
                        None => [0.0; 2],
                    };
                    let kf = self.key_f.tick(k);
                    self.key_line.push(kf);
                    let pk = kf[0].abs().max(kf[1].abs());
                    key_pk = key_pk.max(pk);
                    let above = [kf[0].abs() >= self.thr, kf[1].abs() >= self.thr];
                    m.triggered |= above[0] || above[1];
                    let mut now = [
                        self.env[0].tick(above[0], &self.timing),
                        self.env[1].tick(above[1], &self.timing),
                    ];
                    let link = self.s.link.clamp(0.0, 1.0);
                    if link > 0.0 && now[0] != now[1] {
                        let loud = now[0].max(now[1]);
                        for v in &mut now {
                            *v = if link == 1.0 {
                                loud
                            } else {
                                *v + (loud - *v) * link
                            };
                        }
                    }
                    self.amt_line.push(now);
                    self.amt_line.tap(self.split.latency())
                }
                Trigger::Tempo | Trigger::Midi => {
                    let v = self.pump.next(&self.clock, t - self.lat as i64);
                    [v, v]
                }
            };
            let wv = self.wgt.next();
            let mut g = [1.0f32; 2];
            for ch in 0..2 {
                if a[ch] != 0.0 && wv != 0.0 {
                    g[ch] = (1.0 - a[ch] * wv).max(0.0);
                }
                g_min[ch] = g_min[ch].min(g[ch]);
            }
            last_g = g;
            last_a = a[0].max(a[1]);

            let x = [left[i], right[i]];
            in_pk = in_pk.max(x[0].abs()).max(x[1].abs());
            self.dry.push(x);
            let xd = self.dry.tap(self.lat);
            let kin = self.dry.tap(self.look + self.split.kernel_offset());
            let y = self.split.tick(xd, kin, g);
            let og = self.out.next();
            let mut o = y;
            if og != 1.0 {
                o[0] *= og;
                o[1] *= og;
            }
            let heard = match self.s.listen {
                Listen::Output => o,
                Listen::Delta => [xd[0] - y[0], xd[1] - y[1]],
                Listen::Key => {
                    if self.s.trigger == Trigger::Sidechain {
                        self.key_line.tap(self.lat)
                    } else {
                        [0.0; 2]
                    }
                }
            };
            out_pk = out_pk.max(heard[0].abs()).max(heard[1].abs());
            left[i] = heard[0];
            right[i] = heard[1];
        }
        if notes_done > 0 {
            self.notes.copy_within(notes_done..self.note_n, 0);
            self.note_n -= notes_done;
        }
        let tau = self.clock.stream(n) - 1 - self.lat as i64;
        m.phase = match self.s.trigger {
            Trigger::Sidechain => -1.0,
            _ => self.pump.phase_of(&self.clock, tau),
        };
        self.clock.advance(n);
        let gr = |g: f32| {
            if g >= 1.0 {
                0.0
            } else {
                -peak_db(g).max(-200.0)
            }
        };
        m.gr_db = gr(g_min[0].min(g_min[1]));
        m.gr_now = [gr(last_g[0]), gr(last_g[1])];
        m.amount = last_a;
        m.key_db = peak_db(key_pk);
        m.in_db = peak_db(in_pk);
        m.out_db = peak_db(out_pk);
        self.meter = m;
    }

    /// Samples of silence skipped while at rest: the clock moves (the shape is a function
    /// of the song, so its window is made afresh), notes due are placed.
    pub fn skip(&mut self, frames: usize) {
        let end = self.clock.stream(frames);
        let mut done = 0;
        while done < self.note_n && self.notes[done] < end {
            self.pump.fire(&self.clock, self.notes[done]);
            done += 1;
        }
        if done > 0 {
            self.notes.copy_within(done..self.note_n, 0);
            self.note_n -= done;
        }
        self.clock.skip(frames);
        self.pump.invalidate();
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// The first thing that keeps it from resting, by name.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.wgt.still() || !self.out.still() {
            Some("a glide is moving")
        } else if !self.env.iter().all(Envelope::is_idle) {
            Some("the envelope is ducking")
        } else if self.note_n > 0 || !self.pump.quiet() {
            Some("a note's shape is playing")
        } else if !self.key_f.is_clear() {
            Some("the key filter rings")
        } else if !self.split.is_clear() {
            Some("the band split rings")
        } else if !self.dry.is_clear() {
            Some("the audio line holds sound")
        } else if !self.key_line.is_clear() || !self.amt_line.is_clear() {
            Some("the key line holds sound")
        } else {
            None
        }
    }

    /// A new unit's state, the settings and the shape kept.
    pub fn reset(&mut self) {
        self.clock.reset();
        self.pump.reset();
        self.key_f.reset();
        self.env = [Envelope::default(); 2];
        self.amt_line.clear();
        self.dry.clear();
        self.key_line.clear();
        self.split.reset();
        self.wgt.jump(self.wgt.target);
        self.out.jump(self.out.target);
        self.note_n = 0;
        self.meter = CedoMeter::default();
    }
}

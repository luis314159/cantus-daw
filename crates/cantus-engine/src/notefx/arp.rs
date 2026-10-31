//! The arpeggiator's state: the notes held, the step the pattern is on, the notes it is
//! sounding and when each one ends.
//!
//! Its first step plays where the first note of a chord arrives (on the grid for a clip's
//! chord on the grid), and every step after it one rate later — every second one later by
//! the swing. A step's index chooses its note (up, down, up-down, …, or a hash of the seed
//! and the index for *Random*), so the pattern is the same at any block size and offline.

use super::{derive_tag, Due, Msg, Now};
use cantus_core::notefx::{Arp, ArpMode};

/// Notes it holds, notes it sounds, outputs one call may leave.
pub(super) const HELD: usize = 32;
pub(super) const SOUND: usize = 64;
pub(super) const MAX_OUT: usize = 64;

#[derive(Copy, Clone, Debug)]
struct Held {
    pitch: u8,
    velocity: u8,
    tag: u64,
    live: bool,
    /// Still down (with *hold* on, a released note keeps playing until a new chord).
    down: bool,
    /// The order it was played in.
    order: u64,
    release: u8,
}

#[derive(Copy, Clone, Debug)]
struct Sound {
    pitch: u8,
    tag: u64,
    off: f64,
    release: u8,
    live: bool,
}

const NO_HELD: Held = Held {
    pitch: 0,
    velocity: 0,
    tag: 0,
    live: false,
    down: false,
    order: 0,
    release: 0,
};

const NO_SOUND: Sound = Sound {
    pitch: 0,
    tag: 0,
    off: 0.0,
    release: 0,
    live: false,
};

#[derive(Clone, Debug)]
pub(super) struct ArpState {
    held: [Held; HELD],
    n_held: usize,
    order: u64,
    sound: [Sound; SOUND],
    n_sound: usize,
    /// The domain its times are in (ticks rolling, samples stopped).
    rolling: bool,
    /// A step is scheduled at `next`.
    running: bool,
    /// The next step's time before its swing, and with it.
    base: f64,
    next: f64,
    /// The next step's index from the anchor.
    k: u64,
}

impl ArpState {
    pub fn new() -> ArpState {
        ArpState {
            held: [NO_HELD; HELD],
            n_held: 0,
            order: 0,
            sound: [NO_SOUND; SOUND],
            n_sound: 0,
            rolling: false,
            running: false,
            base: 0.0,
            next: 0.0,
            k: 0,
        }
    }

    pub fn reset(&mut self) {
        self.n_held = 0;
        self.n_sound = 0;
        self.running = false;
        self.k = 0;
        self.order = 0;
    }

    pub fn sounding(&self) -> bool {
        self.n_sound > 0
    }

    fn anchor(&mut self, at: Due) {
        self.rolling = at.rolling;
        self.base = at.pos;
        self.next = at.pos;
        self.k = 0;
        self.running = true;
    }

    /// The transport started or stopped: what sounds ends now, a clip's held notes are let
    /// go (only live keys keep an arpeggio running while stopped), and what is still held
    /// starts again from here.
    fn switch(&mut self, now: Now, out: &mut [Msg; MAX_OUT], n: &mut usize) {
        for s in &self.sound[..self.n_sound] {
            push(out, n, off_of(s));
        }
        self.n_sound = 0;
        if !now.rolling {
            let mut j = 0;
            for i in 0..self.n_held {
                if self.held[i].live {
                    self.held[j] = self.held[i];
                    j += 1;
                }
            }
            self.n_held = j;
        }
        self.rolling = now.rolling;
        if self.n_held > 0 {
            self.anchor(Due {
                rolling: now.rolling,
                pos: now.pos,
            });
        } else {
            self.running = false;
        }
    }

    /// Move to the domain of `now` (the transport started or stopped): what it left, in
    /// `out`.
    pub fn sync(&mut self, now: Now, out: &mut [Msg; MAX_OUT]) -> usize {
        let mut n = 0;
        if (self.running || self.n_sound > 0) && now.rolling != self.rolling {
            self.switch(now, out, &mut n);
        }
        n
    }

    /// A note into the arpeggiator at `at`: held, let go, or — an off it never held —
    /// passed on. Returns how many outputs it left in `out`.
    pub fn input(&mut self, p: &Arp, now: Now, at: Due, m: Msg, out: &mut [Msg; MAX_OUT]) -> usize {
        let mut n = 0;
        if (self.running || self.n_sound > 0) && now.rolling != self.rolling {
            self.switch(now, out, &mut n);
        }
        // Hold switched off since (a click, automation, a modulator): the keys already up
        // are let go.
        if !p.hold {
            self.let_go_released();
        }
        if m.on {
            // Latched and every key up: a new chord replaces the old one.
            if p.hold && self.n_held > 0 && self.held[..self.n_held].iter().all(|h| !h.down) {
                self.n_held = 0;
                self.running = false;
            }
            let was_empty = self.n_held == 0;
            self.order += 1;
            let h = Held {
                pitch: m.pitch,
                velocity: m.velocity,
                tag: m.tag,
                live: m.live,
                down: true,
                order: self.order,
                release: 0,
            };
            match self.held[..self.n_held]
                .iter()
                .position(|x| x.tag == m.tag && x.pitch == m.pitch)
            {
                Some(i) => self.held[i] = h,
                None if self.n_held < HELD => {
                    self.held[self.n_held] = h;
                    self.n_held += 1;
                }
                None => return n,
            }
            if was_empty || !self.running {
                self.anchor(at);
            }
        } else {
            match self.held[..self.n_held]
                .iter()
                .position(|x| x.tag == m.tag && x.pitch == m.pitch)
            {
                Some(i) if p.hold => {
                    self.held[i].down = false;
                    self.held[i].release = m.release;
                }
                Some(i) => {
                    self.held.copy_within(i + 1..self.n_held, i);
                    self.n_held -= 1;
                    if self.n_held == 0 {
                        self.running = false;
                    }
                }
                None => push(out, &mut n, m),
            }
        }
        n
    }

    /// The earliest of the next step and the next note-off.
    pub fn next_due(&self) -> Option<Due> {
        let mut t = if self.running {
            self.next
        } else {
            f64::INFINITY
        };
        for s in &self.sound[..self.n_sound] {
            if s.off < t {
                t = s.off;
            }
        }
        t.is_finite().then_some(Due {
            rolling: self.rolling,
            pos: t,
        })
    }

    /// Fire the earliest scheduled event: a note-off (first, at a tie), or the next step.
    pub fn fire(&mut self, p: &Arp, now: Now, stage: usize, out: &mut [Msg; MAX_OUT]) -> usize {
        let mut n = 0;
        // The earliest note-off, if it is not after the step.
        let mut first: Option<usize> = None;
        for (i, s) in self.sound[..self.n_sound].iter().enumerate() {
            if first.is_none_or(|f| s.off < self.sound[f].off) {
                first = Some(i);
            }
        }
        if let Some(i) = first {
            if !self.running || self.sound[i].off <= self.next {
                push(out, &mut n, off_of(&self.sound[i]));
                self.sound.copy_within(i + 1..self.n_sound, i);
                self.n_sound -= 1;
                return n;
            }
        }
        if !p.hold {
            self.let_go_released();
        }
        if !self.running || self.n_held == 0 {
            self.running = false;
            return n;
        }
        // A step's length in this domain: ticks rolling, samples at the tempo stopped.
        let mut r = p.rate_ticks() as f64;
        if !self.rolling {
            r /= now.tps.max(1e-9);
        }
        let at = self.next;
        // A note's length, in whole ticks (or samples), never under one.
        let gate = (p.gate.clamp(0.05, 2.0) as f64 * r).round().max(1.0);
        let k = self.k;
        let mut sorted = [NO_HELD; HELD];
        let count = self.n_held;
        sorted[..count].copy_from_slice(&self.held[..count]);
        let octaves = p.octaves.clamp(1, 4) as usize;
        let len = count * octaves;
        match p.mode {
            ArpMode::Played => sorted[..count].sort_by_key(|h| h.order),
            _ => sorted[..count].sort_by_key(|h| (h.pitch, h.tag)),
        }
        let pick = |idx: usize| -> (Held, u8) {
            let h = sorted[idx % count];
            (h, (idx / count) as u8 * 12)
        };
        let mut play = |h: Held, up: u8, j: u64, out: &mut [Msg; MAX_OUT], n: &mut usize| {
            let pitch = h.pitch as u16 + up as u16;
            if pitch > 127 || self.n_sound >= SOUND {
                return;
            }
            let tag = derive_tag(
                h.tag ^ ((h.pitch as u64) << 48),
                stage,
                k.wrapping_mul(64).wrapping_add(j),
            );
            let velocity = if p.fixed_velocity {
                p.velocity.clamp(1, 127)
            } else {
                h.velocity
            };
            push(
                out,
                n,
                Msg {
                    on: true,
                    pitch: pitch as u8,
                    velocity,
                    release: 0,
                    tag,
                    ordinal: k as u32,
                    live: h.live,
                },
            );
            self.sound[self.n_sound] = Sound {
                pitch: pitch as u8,
                tag,
                off: at + gate,
                release: if h.down { 0 } else { h.release },
                live: h.live,
            };
            self.n_sound += 1;
        };
        let step = k as usize;
        match p.mode {
            ArpMode::Chord => {
                let o = (step % octaves) as u8 * 12;
                for (j, h) in sorted[..count].iter().enumerate() {
                    play(*h, o, j as u64, out, &mut n);
                }
            }
            mode => {
                let idx = match mode {
                    ArpMode::Up | ArpMode::Played => step % len,
                    ArpMode::Down => len - 1 - step % len,
                    ArpMode::UpDown | ArpMode::DownUp => {
                        let i = if len == 1 {
                            0
                        } else {
                            let period = 2 * len - 2;
                            let j = step % period;
                            if j < len {
                                j
                            } else {
                                period - j
                            }
                        };
                        if mode == ArpMode::DownUp {
                            len - 1 - i
                        } else {
                            i
                        }
                    }
                    _ => (mix(p.seed as u64, k) % len as u64) as usize,
                };
                let (h, up) = pick(idx);
                play(h, up, 0, out, &mut n);
            }
        }
        self.k += 1;
        self.base += r;
        // Whole ticks (whole samples stopped): every time the arpeggio keeps is exact, so
        // a bake writes it to the tick and a render hears it on the same sample.
        let swing = ((p.swing.clamp(0.5, 0.75) as f64 - 0.5) * 2.0 * r).round();
        self.next = self.base + if self.k % 2 == 1 { swing } else { 0.0 };
        n
    }
}

fn off_of(s: &Sound) -> Msg {
    Msg {
        on: false,
        pitch: s.pitch,
        velocity: 0,
        release: s.release,
        tag: s.tag,
        ordinal: 0,
        live: s.live,
    }
}

#[inline]
fn push(out: &mut [Msg; MAX_OUT], n: &mut usize, m: Msg) {
    if *n < MAX_OUT {
        out[*n] = m;
        *n += 1;
    }
}

/// SplitMix64 of the seed and the step: *Random*'s draw.
#[inline]
fn mix(seed: u64, k: u64) -> u64 {
    let mut z = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(k)
        .wrapping_add(0x6A09_E667_F3BC_C909);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl ArpState {
    /// Forget the held notes whose keys are up (what *Hold* kept): once it is off they
    /// play no more; with none left down, the arpeggio stops.
    fn let_go_released(&mut self) {
        if self.held[..self.n_held].iter().all(|h| h.down) {
            return;
        }
        let mut j = 0;
        for i in 0..self.n_held {
            if self.held[i].down {
                self.held[j] = self.held[i];
                j += 1;
            }
        }
        self.n_held = j;
        if j == 0 {
            self.running = false;
        }
    }
}

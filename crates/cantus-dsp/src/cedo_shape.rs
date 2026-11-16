//! Cedo's pump: a drawn ducking shape, fired on every beat division of the song (with
//! swing) or by the notes of another track — the classic sidechain "pump" with no audio
//! needed.
//!
//! - **The shape** is Lapsus's curve ([`crate::lapsus::Curve`], Fluctus's points with the
//!   automation editor's segment shapes and tensions): points across one cycle, `y` the
//!   duck's amount (0 none … 1 the full depth).
//! - **On the song** (`Tempo`): cycle `j` of a division of `b` quarter notes starts on the
//!   song sample `round(j·b·60/tempo·fs)` — the engine's own expression for a beat's sample
//!   (Lapsus's) — and with swing `σ` every second cycle starts `σ/2` of a cycle late (the
//!   modulation family's convention: σ = 1 is MPC's 75 %). Within a cycle the phase is
//!   `(s − start)/(next start − start)`, so the shape's first point lands **on the grid's
//!   sample**. The amount is a function of the song sample alone: a render from bar 5 hears
//!   what a whole render hears from bar 5, and offline equals real time. It plays while the
//!   transport plays; on stop it lets go through its edge.
//! - **On notes** (`Midi`): a note plays the shape once from its sample, a cycle of the
//!   division long; the next note starts it again; after the cycle nothing is ducked.
//! - **The edge**: the shape is smoothed by a Hann kernel `edge` wide **centred** on the
//!   sample — the song's future is known, so a vertical edge becomes a raised-cosine ramp
//!   whose middle is the edge itself, with no latency (a note's ramp is centred too when
//!   the unit's latency covers half the edge; otherwise it starts at the note). Where the
//!   whole kernel sees one value, that value is passed exactly (no sum), so a flat stretch
//!   at 0 ducks nothing, to the bit.
//!
//! The ring of values the kernel reads is a function of the song (or of the notes' samples)
//! too: it is filled afresh from the shape at every start, seek, wrap or tempo change, and
//! after a `skip`, so it never depends on what was processed before.

use crate::fluctus::{CurvePoint, ShapeFn};
use crate::lapsus::Curve;
use crate::modfx_clock::{Clock, DIVISIONS};

/// The longest edge, ms.
pub const MAX_EDGE_MS: f32 = 10.0;
/// A start of playback or a seek that lands inside a cycle (not on a cycle's start nor on
/// a bar line) fades the shape in over this long, ms: a raised cosine from nothing.
pub const START_FADE_MS: f32 = 10.0;
/// Notes remembered (only the newest that has begun matters; the rest wait for their
/// sample).
const MAX_ORIGINS: usize = 32;

/// What fires the shape.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Tempo,
    Midi,
}

/// The pump.
#[derive(Clone, Debug)]
pub struct Pump {
    sr: f64,
    shape: ShapeFn,
    curve: Curve,
    beats: f64,
    /// The swung cycle's lateness, cycles (0 … 0.5).
    sw: f64,
    h: usize,
    w: Vec<f32>,
    /// The kernel's values, doubled so the window is one slice.
    ring: Vec<f32>,
    cap: usize,
    wpos: usize,
    head: i64,
    valid: bool,
    run: usize,
    last: f32,
    seen_jumps: u32,
    seen_playing: bool,
    src: Source,
    origins: [i64; MAX_ORIGINS],
    /// Each note's amount (its velocity's, 1 without).
    scales: [f32; MAX_ORIGINS],
    on: usize,
    seen_tempo: f64,
    /// Where the last start or seek inside a cycle landed (stream sample of its audio):
    /// the shape fades in from there. A function of where it landed, so a skip or a refill
    /// gives the same values.
    fade_from: Option<i64>,
    fade_len: f64,
    /// The cycle holding the last song sample asked: (start, next start, number).
    cyc: Option<(i64, i64, i64)>,
}

impl Pump {
    /// Allocates the kernel and its ring for the longest edge at `sample_rate`.
    pub fn new(sample_rate: f32, shape: ShapeFn) -> Pump {
        let sr = sample_rate as f64;
        let max_taps = 2 * half_width(MAX_EDGE_MS, sr) + 1;
        let cap = max_taps.next_power_of_two();
        let mut p = Pump {
            sr,
            shape,
            curve: Curve::flat(0.0),
            beats: 1.0,
            sw: 0.0,
            h: 0,
            w: vec![0.0; max_taps],
            ring: vec![0.0; 2 * cap],
            cap,
            wpos: 0,
            head: 0,
            valid: false,
            run: 0,
            last: 0.0,
            seen_jumps: 0,
            seen_playing: false,
            src: Source::Tempo,
            origins: [0; MAX_ORIGINS],
            scales: [1.0; MAX_ORIGINS],
            on: 0,
            seen_tempo: 0.0,
            fade_from: None,
            fade_len: (START_FADE_MS as f64 * 0.001 * sr).round().max(1.0),
            cyc: None,
        };
        p.set_edge(1.0);
        p
    }

    /// The shape's points (`y` the amount, 0 … 1), at most [`crate::lapsus::MAX_POINTS`].
    /// No allocation.
    pub fn set_shape(&mut self, points: &[CurvePoint]) {
        if !self.curve.same_as(points, 0.0, 1.0) {
            self.curve.set(points, 0.0, 1.0);
            self.valid = false;
        }
    }

    pub fn points(&self) -> &[CurvePoint] {
        self.curve.points()
    }

    /// The division (an index into the family's [`DIVISIONS`]), the swing (0 … 1) and the
    /// source.
    pub fn set_grid(&mut self, division: usize, swing: f32, src: Source) {
        let beats = DIVISIONS[division.min(DIVISIONS.len() - 1)];
        let sw = if swing.is_finite() {
            swing.clamp(0.0, 1.0) as f64 * 0.5
        } else {
            0.0
        };
        if beats != self.beats || sw != self.sw || src != self.src {
            if src != self.src {
                self.on = 0;
            }
            self.beats = beats;
            self.sw = sw;
            self.src = src;
            self.valid = false;
            self.cyc = None;
        }
    }

    /// The edge's width, ms (0 … [`MAX_EDGE_MS`]).
    pub fn set_edge(&mut self, ms: f32) {
        let h = half_width(ms, self.sr);
        if h == self.h && self.w[0] != 0.0 {
            return;
        }
        self.h = h;
        let taps = 2 * h + 1;
        let mut sum = 0.0f64;
        for k in 0..taps {
            let s = crate::math::sin(std::f64::consts::PI * (k + 1) as f64 / (taps + 1) as f64);
            sum += s * s;
        }
        for k in 0..taps {
            let s = crate::math::sin(std::f64::consts::PI * (k + 1) as f64 / (taps + 1) as f64);
            self.w[k] = (s * s / sum) as f32;
        }
        self.valid = false;
    }

    /// The ring must be filled afresh before its next value (a skip, a reset, a latency
    /// change).
    pub fn invalidate(&mut self) {
        self.valid = false;
        self.cyc = None;
    }

    /// Back to a new pump's state (the shape and settings kept).
    pub fn reset(&mut self) {
        self.on = 0;
        self.invalidate();
        self.seen_jumps = 0;
        self.seen_playing = false;
        self.seen_tempo = 0.0;
        self.fade_from = None;
    }

    /// Samples in one cycle at the clock's tempo.
    fn len(&self, clock: &Clock) -> f64 {
        self.beats * 60.0 / clock.tempo() * self.sr
    }

    /// The start of cycle `j` on the song, samples.
    fn bound(&self, j: i64, len: f64) -> i64 {
        let pair = j.div_euclid(2);
        let x = if j.rem_euclid(2) == 0 {
            (2 * pair) as f64
        } else {
            (2 * pair + 1) as f64 + self.sw
        };
        (x * len).round() as i64
    }

    /// The phase (0 … 1) of song sample `s`, and its cycle's number.
    fn phase(&mut self, s: i64, len: f64) -> (f64, i64) {
        let (b0, b1, j) = match self.cyc {
            Some(c) if s >= c.0 && s < c.1 => c,
            _ => {
                let mut j = (s as f64 / len).floor() as i64;
                while self.bound(j, len) > s {
                    j -= 1;
                }
                while self.bound(j + 1, len) <= s {
                    j += 1;
                }
                let c = (self.bound(j, len), self.bound(j + 1, len), j);
                self.cyc = Some(c);
                c
            }
        };
        ((s - b0) as f64 / (b1 - b0).max(1) as f64, j)
    }

    /// The shape's amount for the audio of stream sample `tau` (no edge).
    fn value(&mut self, clock: &Clock, tau: i64) -> f32 {
        match self.src {
            Source::Tempo => {
                if !clock.playing() {
                    return 0.0;
                }
                let s = clock.song(0) + (tau - clock.stream(0));
                let len = self.len(clock);
                let (u, _) = self.phase(s, len);
                let v = self.curve.at(u, self.shape) as f32;
                match self.fade_from {
                    Some(f) if tau < f => 0.0,
                    Some(f) if ((tau - f) as f64) < self.fade_len => {
                        let r = (tau - f) as f64 / self.fade_len;
                        v * (0.5 - 0.5 * crate::math::cos(std::f64::consts::PI * r)) as f32
                    }
                    _ => v,
                }
            }
            Source::Midi => {
                let Some(i) = self.origins[..self.on].iter().rposition(|o| *o <= tau) else {
                    return 0.0;
                };
                let d = (tau - self.origins[i]) as f64;
                let len = self.len(clock);
                if d < len {
                    self.curve.at(d / len, self.shape) as f32 * self.scales[i]
                } else {
                    0.0
                }
            }
        }
    }

    /// Where in its cycle the audio of stream sample `tau` is (0 … 1), or −1 when no
    /// cycle runs there: the display's playhead.
    pub fn phase_of(&mut self, clock: &Clock, tau: i64) -> f32 {
        let len = self.len(clock);
        match self.src {
            Source::Tempo => {
                if !clock.playing() {
                    return -1.0;
                }
                let s = clock.song(0) + (tau - clock.stream(0));
                self.phase(s, len).0 as f32
            }
            Source::Midi => match self.origins[..self.on].iter().rev().find(|o| **o <= tau) {
                Some(o) if ((tau - o) as f64) < len => ((tau - o) as f64 / len) as f32,
                _ => -1.0,
            },
        }
    }

    #[inline]
    fn push(&mut self, v: f32) {
        self.ring[self.wpos] = v;
        self.ring[self.wpos + self.cap] = v;
        self.wpos = (self.wpos + 1) & (self.cap - 1);
        if v.to_bits() == self.last.to_bits() {
            self.run = (self.run + 1).min(self.cap);
        } else {
            self.run = 1;
            self.last = v;
        }
    }

    /// The window: the newest `2h + 1` values, oldest first.
    #[inline]
    fn window(&self) -> &[f32] {
        let taps = 2 * self.h + 1;
        let end = self.wpos + self.cap;
        &self.ring[end - taps..end]
    }

    fn recount(&mut self) {
        let win = self.window();
        let newest = win[win.len() - 1];
        let run = win
            .iter()
            .rev()
            .take_while(|v| v.to_bits() == newest.to_bits())
            .count();
        self.last = newest;
        self.run = run;
    }

    /// Fill the window afresh around `tc`.
    fn prefill(&mut self, clock: &Clock, tc: i64) {
        let h = self.h as i64;
        self.cyc = None;
        for tau in tc - h..=tc + h {
            let v = self.value(clock, tau);
            self.push(v);
        }
        self.recount();
        self.head = tc + h;
        self.valid = true;
        self.seen_jumps = clock.jumps();
        self.seen_playing = clock.playing();
        self.seen_tempo = clock.tempo();
    }

    /// Whether the transport landed (this call's first sample) on a cycle's start or a bar
    /// line (4/4 from the song's start): a start there is heard as a whole render hears it.
    fn on_boundary(&mut self, clock: &Clock) -> bool {
        let s = clock.song(0);
        let len = self.len(clock);
        if self.phase(s, len).0 == 0.0 {
            return true;
        }
        let bar = 4.0 * 60.0 / clock.tempo() * self.sr;
        let k = (s as f64 / bar).round();
        (k * bar).round() as i64 == s
    }

    /// A note at stream sample `at`: the shape plays once from there, its amount scaled
    /// by `scale` (0 … 1). Two notes on one sample: the louder.
    pub fn fire(&mut self, clock: &Clock, at: i64, scale: f32) {
        if self.src != Source::Midi {
            return;
        }
        let scale = if scale.is_finite() {
            scale.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if self.on > 0 && self.origins[self.on - 1] == at {
            if scale <= self.scales[self.on - 1] {
                return;
            }
            self.on -= 1;
        }
        if self.on == MAX_ORIGINS {
            self.origins.copy_within(1.., 0);
            self.scales.copy_within(1.., 0);
            self.on -= 1;
        }
        self.origins[self.on] = at;
        self.scales[self.on] = scale;
        self.on += 1;
        if self.valid && at <= self.head {
            // The window already holds values from `at` on, made before the note was
            // known: make them again.
            let taps = 2 * self.h + 1;
            let from = at.max(self.head - (taps as i64 - 1));
            for tau in from..=self.head {
                let back = (self.head - tau) as usize;
                let v = self.value(clock, tau);
                let i = (self.wpos + self.cap - 1 - back) & (self.cap - 1);
                self.ring[i] = v;
                self.ring[i + self.cap] = v;
            }
            self.recount();
        }
    }

    /// The smoothed amount for the audio of stream sample `tc`; called once a sample, `tc`
    /// rising by one each time.
    #[inline]
    pub fn next(&mut self, clock: &Clock, tc: i64) -> f32 {
        let started = clock.playing() && !self.seen_playing;
        let jumped = clock.jumps() != self.seen_jumps;
        if self.src == Source::Tempo
            && clock.playing()
            && (started || (jumped && clock.tempo() == self.seen_tempo))
        {
            // The audio from the landing on fades in; the audio before it is not ducked by
            // the place the song has jumped to.
            self.fade_from = if self.on_boundary(clock) {
                None
            } else {
                Some(clock.stream(0))
            };
        }
        if !self.valid
            || clock.jumps() != self.seen_jumps
            || started
            || self.head != tc - 1 + self.h as i64
        {
            self.prefill(clock, tc);
        } else {
            let v = self.value(clock, tc + self.h as i64);
            self.push(v);
            self.head += 1;
            self.seen_playing = clock.playing();
        }
        self.prune(clock);
        if self.run > 2 * self.h {
            return self.last;
        }
        let win = self.window();
        let mut acc = 0.0f32;
        for (w, v) in self.w.iter().zip(win) {
            acc += w * v;
        }
        acc.clamp(0.0, 1.0)
    }

    /// Notes no longer heard anywhere in the window: before a newer one that has begun, or
    /// played out.
    fn prune(&mut self, clock: &Clock) {
        if self.on == 0 {
            return;
        }
        let tail = self.head - 2 * self.h as i64;
        let len = self.len(clock);
        let mut keep_from = 0;
        for i in 0..self.on {
            let later_begun = i + 1 < self.on && self.origins[i + 1] <= tail;
            let ended = (self.origins[i] as f64 + len) <= tail as f64;
            if later_begun || ended {
                keep_from = i + 1;
            } else {
                break;
            }
        }
        if keep_from > 0 {
            self.origins.copy_within(keep_from..self.on, 0);
            self.scales.copy_within(keep_from..self.on, 0);
            self.on -= keep_from;
        }
    }

    /// Nothing to come from the notes, and the window all zeros: what a new pump gives.
    pub fn quiet(&self) -> bool {
        self.on == 0 && (!self.valid || (self.run > 2 * self.h && self.last.to_bits() == 0))
    }

    /// Notes waiting or playing.
    pub fn notes(&self) -> usize {
        self.on
    }
}

fn half_width(ms: f32, sr: f64) -> usize {
    let ms = if ms.is_finite() {
        ms.clamp(0.0, MAX_EDGE_MS)
    } else {
        0.0
    };
    (ms as f64 * 0.0005 * sr).round() as usize
}

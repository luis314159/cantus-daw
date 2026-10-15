//! **Tonus**, the real-time auto-tuner (round 28, ADR 0053): the pitch base's streaming
//! tracker ([`crate::pitch::PitchTracker`]) hears the voice, a target note is chosen — from
//! a key and scale, a set of notes, or the notes played on MIDI — and the streaming
//! shifter ([`crate::psola::Shifter`]) moves the voice there, keeping its formants.
//!
//! ## The correction, sample by sample
//!
//! The tracker gives the sung pitch `p` (MIDI units against the reference A4) every hop
//! (2.7 ms). The **target** `T` is the allowed note nearest `p` (with a little hysteresis,
//! [`scale::HYSTERESIS`]); a *bypass* note, or no note at all, asks for no correction.
//! The wanted correction `T − p` (zero inside the *flex* tolerance) is followed by a
//! one-pole glide whose time is the **retune speed** (0 ms: at once — the hard effect);
//! *humanise* slows it as a note is held, so long notes keep their movement. The output
//! pitch is `p + amount·c`, plus the singer's own vibrato scaled by *vibrato* (as much of
//! it as the speed lets through), plus a created vibrato, plus the transposition — glided
//! over 2 ms so the hop's steps never show. The shifter's synthesis period is the output
//! pitch's period exactly, so at speed 0 the output **is** the target: no warble, however
//! the voice moves.
//!
//! ## The null
//!
//! At **amount 0** (with no vibrato added or scaled, no transposition, no throat), or with
//! every note on *bypass*, or with a MIDI source and no note held, the shifter is never
//! engaged and the output is the input delayed by [`Tonus::latency`], bit for bit.

use crate::math;
use crate::pitch::{PitchFrame, PitchTracker};
use crate::psola::{Control, Shifter};

#[path = "tonus_scale.rs"]
pub mod scale;
#[cfg(test)]
#[path = "tonus_tests.rs"]
mod tests;

pub use scale::{Allowed, Held, NoteState, Scale};

/// Where the targets come from (the parameter `src`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// The key, the scale and the per-note states.
    #[default]
    Scale,
    /// The pitch classes held on MIDI (any octave): the nearest one is the target.
    Midi,
    /// The newest note held on MIDI, exactly: the voice sings the keyboard's melody.
    Melody,
}

impl Source {
    pub fn from_index(i: usize) -> Source {
        match i {
            1 => Source::Midi,
            2 => Source::Melody,
            _ => Source::Scale,
        }
    }
}

/// Tonus's settings, as the DSP wants them. The defaults are a chromatic corrector at a
/// quick, natural speed.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TonusSettings {
    pub source: Source,
    /// The key, 0 = C … 11 = B.
    pub key: u8,
    pub scale: Scale,
    /// Per pitch class, C … B.
    pub notes: [NoteState; 12],
    /// The retune speed, ms (0: the hard effect).
    pub speed_ms: f32,
    /// How much of the correction is applied, 0 … 1.
    pub amount: f32,
    /// How much longer notes keep of their movement, 0 … 1.
    pub humanize: f32,
    /// Deviations smaller than this are left alone, cents.
    pub flex_cents: f32,
    /// The singer's own vibrato: −1 removes what the speed lets through, +1 doubles it.
    pub vibrato: f32,
    /// A created vibrato's depth, cents (0: none), and rate, Hz.
    pub vib_depth: f32,
    pub vib_rate: f32,
    /// Keep the formants where they are (the default) or let them move with the pitch.
    pub formants: bool,
    /// The formant shift — the throat — semitones.
    pub throat: f32,
    /// A transposition, semitones.
    pub transpose: f32,
    /// The reference pitch of A4, Hz.
    pub a4: f32,
}

impl Default for TonusSettings {
    fn default() -> Self {
        TonusSettings {
            source: Source::Scale,
            key: 0,
            scale: Scale::Chromatic,
            notes: [NoteState::Scale; 12],
            speed_ms: 20.0,
            amount: 1.0,
            humanize: 0.0,
            flex_cents: 0.0,
            vibrato: 0.0,
            vib_depth: 0.0,
            vib_rate: 5.0,
            formants: true,
            throat: 0.0,
            transpose: 0.0,
            a4: 440.0,
        }
    }
}

/// A note from the slot's MIDI source.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoteEvent {
    On(u8),
    Off(u8),
    AllOff,
}

/// A telemetry column: what was sung and what came out, MIDI (0: unvoiced).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Column {
    pub input: f32,
    pub output: f32,
}

/// Columns of history kept for the editor.
pub const HISTORY: usize = 24;
/// The humanise glide: notes held longer than `HUMAN_START` s slow toward
/// `speed + humanize · HUMAN_MS` over `HUMAN_RAMP` s.
const HUMAN_START: f32 = 0.12;
const HUMAN_RAMP: f32 = 0.3;
const HUMAN_MS: f32 = 400.0;
/// The centre of the sung pitch (vibrato removed): a one-pole at this cutoff, Hz.
const CENTRE_HZ: f32 = 2.0;
/// The vibrato the singer's own is judged at, Hz.
const VIBRATO_HZ: f32 = 5.5;
/// The output pitch's glide, seconds.
const OUT_GLIDE_S: f32 = 0.002;
/// A created vibrato fades in over this long after a note starts, seconds.
const VIB_ONSET_S: f32 = 0.25;
/// Below this level (dBFS) the unvoiced input is silence: nothing to move formants on.
const SILENCE_DB: f32 = -90.0;
const MAX_EVENTS: usize = 128;

/// The real-time auto-tuner. Allocates in `new` only.
#[derive(Clone, Debug)]
pub struct Tonus {
    sample_rate: f32,
    tracker: PitchTracker,
    shifter: Shifter,
    s: TonusSettings,
    allowed: [Allowed; 12],
    held: Held,
    events: [(usize, NoteEvent); MAX_EVENTS],
    n_events: usize,
    // The voice, from the newest frame.
    voiced: bool,
    p: f32,
    p_c: f32,
    period: f32,
    level_db: f32,
    // The correction.
    target: Option<i32>,
    bypass: bool,
    c_target: f32,
    c: f32,
    k: f32,
    vib_term: f32,
    note_age: u32,
    lfo: f32,
    p_out: f32,
    ctl: Control,
    // Coefficients.
    k_centre: f32,
    k_out: f32,
    // Telemetry.
    col_len: usize,
    col_pos: usize,
    seq: u64,
    cols: [Column; HISTORY],
}

impl Tonus {
    pub fn new(sample_rate: f32) -> Self {
        let tracker = PitchTracker::new(sample_rate);
        let shifter = Shifter::new(sample_rate);
        let sr = shifter.sample_rate();
        let hop = tracker.hop() as f32;
        let s = TonusSettings::default();
        let mut t = Tonus {
            sample_rate: sr,
            col_len: tracker.hop() * 2,
            tracker,
            shifter,
            allowed: [Allowed::Target; 12],
            held: Held::default(),
            events: [(0, NoteEvent::AllOff); MAX_EVENTS],
            n_events: 0,
            voiced: false,
            p: 0.0,
            p_c: 0.0,
            period: 0.0,
            level_db: -120.0,
            target: None,
            bypass: false,
            c_target: 0.0,
            c: 0.0,
            k: 1.0,
            vib_term: 0.0,
            note_age: 0,
            lfo: 0.0,
            p_out: 0.0,
            ctl: Control::default(),
            k_centre: 1.0 - math::exp(-2.0 * core::f32::consts::PI * CENTRE_HZ * hop / sr),
            k_out: 1.0 - math::exp(-1.0 / (OUT_GLIDE_S * sr)),
            col_pos: 0,
            seq: 0,
            cols: [Column::default(); HISTORY],
            s,
        };
        t.set(&s);
        t
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The fixed delay, samples (the shifter's).
    pub fn latency(&self) -> usize {
        self.shifter.latency()
    }

    pub fn settings(&self) -> &TonusSettings {
        &self.s
    }

    pub fn set(&mut self, s: &TonusSettings) {
        self.s = *s;
        self.allowed = scale::resolve(s.scale, s.key, &s.notes);
        if self.voiced {
            self.decide();
            self.retune_coefficient();
        }
    }

    pub fn reset(&mut self) {
        self.tracker.reset();
        self.shifter.reset();
        self.held.clear();
        self.n_events = 0;
        self.unvoice();
        self.level_db = -120.0;
        self.ctl = Control::default();
        self.col_pos = 0;
        self.seq = 0;
        self.cols = [Column::default(); HISTORY];
    }

    /// Exactly as new: the tracker and the shifter at rest, no note held or pending, no
    /// correction under way.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.tracker.at_rest() {
            Some("the tracker hears sound")
        } else if !self.shifter.at_rest() {
            Some("the shifter is still sounding")
        } else if !self.held.is_empty() || self.n_events > 0 {
            Some("a note is held")
        } else if self.voiced || self.ctl.engage {
            Some("a voice is being corrected")
        } else {
            None
        }
    }

    /// `frames` samples of silence went by unprocessed, at rest.
    pub fn skip(&mut self, frames: usize) {
        self.tracker.skip(frames);
        self.shifter.skip(frames);
        let total = self.col_pos + frames;
        let done = (total / self.col_len) as u64;
        // The columns silence would have closed: unvoiced, each.
        let fill = done.min(HISTORY as u64);
        for k in 0..fill {
            let idx = self.seq + done - fill + k;
            self.cols[(idx % HISTORY as u64) as usize] = Column::default();
        }
        self.seq += done;
        self.col_pos = total % self.col_len;
    }

    /// A note from the slot's MIDI source, `offset` samples into the next `process`.
    pub fn note(&mut self, offset: usize, e: NoteEvent) {
        if self.n_events < MAX_EVENTS {
            self.events[self.n_events] = (offset, e);
            self.n_events += 1;
        } else if let NoteEvent::AllOff = e {
            // Never lose the one that frees everything.
            self.events[MAX_EVENTS - 1] = (offset, e);
        }
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut ev = 0;
        for i in 0..n {
            while ev < self.n_events && self.events[ev].0 <= i {
                let e = self.events[ev].1;
                self.apply(e);
                ev += 1;
            }
            let mono = 0.5 * (l[i] + r[i]);
            if let Some(f) = self.tracker.push(mono) {
                self.frame(f);
            }
            self.control();
            let (a, b) = self.shifter.tick(l[i], r[i], &self.ctl);
            l[i] = a;
            r[i] = b;
            self.column();
        }
        // Events past the block (the host promises none): applied now.
        while ev < self.n_events {
            let e = self.events[ev].1;
            self.apply(e);
            ev += 1;
        }
        self.n_events = 0;
    }

    fn apply(&mut self, e: NoteEvent) {
        match e {
            NoteEvent::On(p) => self.held.on(p),
            NoteEvent::Off(p) => self.held.off(p),
            NoteEvent::AllOff => self.held.clear(),
        }
        if self.voiced {
            self.decide();
        }
    }

    /// A frame from the tracker.
    fn frame(&mut self, f: PitchFrame) {
        self.level_db = f.level_db;
        if !f.voiced || f.f0 <= 0.0 {
            if self.voiced {
                self.unvoice();
            }
            return;
        }
        let p = 69.0 + 12.0 * math::log2(f.f0 / self.s.a4);
        if self.voiced {
            self.p_c += (p - self.p_c) * self.k_centre;
        } else {
            self.voiced = true;
            self.p_c = p;
        }
        self.p = p;
        self.period = f.period;
        self.decide();
        self.retune_coefficient();
    }

    /// Back to "no voice": every piece of the correction as new.
    fn unvoice(&mut self) {
        self.voiced = false;
        self.p = 0.0;
        self.p_c = 0.0;
        self.period = 0.0;
        self.target = None;
        self.bypass = false;
        self.c_target = 0.0;
        self.c = 0.0;
        self.k = 1.0;
        self.vib_term = 0.0;
        self.note_age = 0;
        self.lfo = 0.0;
        self.p_out = 0.0;
    }

    /// Choose the target for the current pitch and the correction it asks for.
    fn decide(&mut self) {
        let s = &self.s;
        let pick = match s.source {
            Source::Scale => scale::choose(self.p, &self.allowed, self.target),
            Source::Midi if self.held.is_empty() => None,
            Source::Midi => scale::choose(self.p, &self.held.allowed(), self.target),
            Source::Melody => self.held.last().map(|n| (n as i32, false)),
        };
        let t = pick.map(|x| x.0);
        if t != self.target {
            self.note_age = 0;
            self.lfo = 0.0;
        }
        self.target = t;
        self.bypass = pick.is_some_and(|x| x.1);
        self.c_target = match pick {
            Some((n, false)) => {
                let want = n as f32 - self.p;
                want * flex_gain(((n as f32 - self.p_c) * 100.0).abs(), s.flex_cents)
            }
            _ => 0.0,
        };
    }

    /// The glide coefficient of the correction: the speed, slowed by humanise as the note
    /// is held; and the share of the singer's vibrato that speed lets through.
    fn retune_coefficient(&mut self) {
        let s = &self.s;
        let age = self.note_age as f32 / self.sample_rate;
        let ramp = ((age - HUMAN_START) / HUMAN_RAMP).clamp(0.0, 1.0);
        let speed = s.speed_ms + s.humanize * HUMAN_MS * ramp;
        self.k = if speed <= 0.0 {
            1.0
        } else {
            1.0 - math::exp(-3.0 / (speed * 0.001 * self.sample_rate))
        };
        // A one-pole of time constant speed/3 corrects a vibrato at VIBRATO_HZ partly:
        // what is left is |jωτ / (1 + jωτ)|.
        let wt = 2.0 * core::f32::consts::PI * VIBRATO_HZ * speed * 0.001 / 3.0;
        let through = wt / (1.0 + wt * wt).sqrt();
        self.vib_term = if s.vibrato == 0.0 {
            0.0
        } else {
            s.vibrato * through * (self.p - self.p_c)
        };
    }

    /// The shifter's control for this sample.
    fn control(&mut self) {
        let s = self.s;
        let throat = if s.throat == 0.0 {
            1.0
        } else {
            math::exp2(s.throat / 12.0)
        };
        if !self.voiced {
            self.ctl = Control {
                period: 0.0,
                ratio: 1.0,
                formant: throat,
                engage: throat != 1.0 && self.level_db > SILENCE_DB,
            };
            return;
        }
        self.note_age = self.note_age.saturating_add(1);
        self.c = if self.k >= 1.0 {
            self.c_target
        } else {
            self.c + (self.c_target - self.c) * self.k
        };
        let mut created = 0.0;
        if s.vib_depth > 0.0 {
            let age = self.note_age as f32 / self.sample_rate;
            let onset = (age / VIB_ONSET_S).min(1.0);
            created = s.vib_depth * 0.01 * onset * math::sin(2.0 * core::f32::consts::PI * self.lfo);
            self.lfo += s.vib_rate / self.sample_rate;
            if self.lfo >= 1.0 {
                self.lfo -= 1.0;
            }
        }
        let shift = s.amount * self.c + self.vib_term + created + s.transpose;
        if shift == 0.0 && throat == 1.0 {
            self.p_out = self.p;
            self.ctl = Control {
                period: self.period,
                ratio: 1.0,
                formant: 1.0,
                engage: false,
            };
            return;
        }
        let want = self.p + shift;
        self.p_out = if self.p_out == 0.0 {
            want
        } else {
            self.p_out + (want - self.p_out) * self.k_out
        };
        let ratio = math::exp2((self.p_out - self.p) / 12.0);
        self.ctl = Control {
            period: self.period,
            ratio,
            formant: if s.formants { throat } else { throat * ratio },
            engage: true,
        };
    }

    /// Close a telemetry column when one is complete.
    fn column(&mut self) {
        self.col_pos += 1;
        if self.col_pos < self.col_len {
            return;
        }
        self.col_pos = 0;
        let c = if self.voiced {
            Column {
                input: self.p,
                output: if self.ctl.engage { self.p_out } else { self.p },
            }
        } else {
            Column::default()
        };
        self.cols[(self.seq % HISTORY as u64) as usize] = c;
        self.seq += 1;
    }

    // ---- What the editor reads ----------------------------------------------------

    /// Columns completed since reset.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Samples per telemetry column.
    pub fn column_len(&self) -> usize {
        self.col_len
    }

    /// Column `index`, if still held (the last [`HISTORY`]).
    pub fn column_at(&self, index: u64) -> Option<Column> {
        (index < self.seq && self.seq - index <= HISTORY as u64)
            .then(|| self.cols[(index % HISTORY as u64) as usize])
    }

    /// The sung pitch now (MIDI against the reference), when voiced.
    pub fn detected(&self) -> Option<f32> {
        self.voiced.then_some(self.p)
    }

    /// The target note now, when there is one.
    pub fn target(&self) -> Option<i32> {
        self.target
    }

    /// Whether the voice is on a bypass note.
    pub fn on_bypass(&self) -> bool {
        self.bypass
    }

    /// The shift applied now, semitones (0 when not engaged).
    pub fn shift(&self) -> f32 {
        if self.voiced && self.ctl.engage {
            self.p_out - self.p
        } else {
            0.0
        }
    }

    /// The pitch classes held on MIDI (bit 0 = C).
    pub fn held_classes(&self) -> u16 {
        self.held.classes()
    }

    /// The pitch classes that are targets now (bit 0 = C), and those on bypass.
    pub fn target_classes(&self) -> (u16, u16) {
        let a = match self.s.source {
            Source::Scale => self.allowed,
            _ => self.held.allowed(),
        };
        let mut on = 0u16;
        let mut by = 0u16;
        for (pc, x) in a.iter().enumerate() {
            match x {
                Allowed::Target => on |= 1 << pc,
                Allowed::Bypass => by |= 1 << pc,
                Allowed::Removed => {}
            }
        }
        (on, by)
    }

    /// The input level of the newest frame, dBFS.
    pub fn level_db(&self) -> f32 {
        self.level_db
    }
}

/// The share of a correction applied at a deviation of `cents`, with a tolerance of
/// `flex` cents: none below half the tolerance, all of it beyond the tolerance, a smooth
/// step between.
pub fn flex_gain(cents: f32, flex: f32) -> f32 {
    if flex <= 0.0 {
        return 1.0;
    }
    let x = ((cents - flex * 0.5) / (flex * 0.5)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

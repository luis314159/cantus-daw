//! Stereo delay with feedback tone control and ping-pong mode.
//!
//! The delay line is allocated once (2 s at the given sample rate); time changes glide
//! through a smoother and a fractional read head, so sweeping the time bends pitch like a
//! tape echo instead of clicking.
//!
//! Round 40 (ADR 0119): the time can follow the tempo — a note value, 1/64 to 4/1, straight,
//! dotted or triplet ([`sync`]). Off by default, and while it is off the unit runs exactly
//! the instructions it ran before (the null, to the bit). With it on, the line grows with
//! what the note needs ([`StereoDelay::need`], [`StereoDelay::supply`]) and the read head
//! follows each tempo the engine tells ([`StereoDelay::set_tempo`], [`sync::Head`]).
//!
//! A long line is never copied or cleared whole in one call (fix-a, A-M3): its history
//! moves into a new line, and a reset clears it, a bounded stretch per call
//! ([`line`]: `MAINTAIN_CELLS` cells a side beyond the call's frames).

use crate::math;
use crate::rest::ZeroRun;
use crate::smoother::Smoother;

#[path = "delay_sync.rs"]
pub mod sync;

#[path = "delay_line.rs"]
mod line;

pub use line::MAINTAIN_CELLS;
pub use sync::Lines;

pub const MAX_DELAY_SECONDS: f32 = 2.0;

/// The time smoother's glide, seconds (to 99 %).
const GLIDE_S: f32 = 0.05;

#[derive(Clone, Debug, PartialEq)]
pub struct DelaySettings {
    pub time_ms: f32,
    pub feedback: f32,
    pub tone_hz: f32,
    pub ping_pong: bool,
    pub mix: f32,
    /// The time follows the tempo: `note` at the tempo the engine tells; `time_ms` waits.
    pub sync: bool,
    /// The note value while synced ([`sync::beats`]).
    pub note: usize,
}

impl Default for DelaySettings {
    fn default() -> Self {
        DelaySettings {
            time_ms: 375.0,
            feedback: 0.4,
            tone_hz: 6_000.0,
            ping_pong: true,
            mix: 0.3,
            sync: false,
            note: sync::DEFAULT_NOTE,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StereoDelay {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
    time: Smoother,
    mix: Smoother,
    lp_l: f32,
    lp_r: f32,
    lp_coef: f32,
    settings: DelaySettings,
    /// Zeros written into each line in a row, for [`StereoDelay::at_rest`].
    quiet: [ZeroRun; 2],
    /// Samples processed or skipped since reset (round 40): the write position is this
    /// modulo the line's length, and a new line takes its position from it.
    n: u64,
    /// The tempo the engine told last (120 until it does).
    tempo: f64,
    /// The synced read head, in samples (round 40).
    synced: sync::Head,
    /// The synced head's glide and crossfade, in samples (the time smoother's 50 ms).
    fade: u32,
    /// The sync was just switched on: the head goes to the note's time at the tempo of the
    /// next pass, told before it ([`Self::set_tempo`]) — never from a tempo gone stale while
    /// the sync was off, when nobody told the unit one (fix-a, A-L1).
    arm: bool,
    /// A line being given the history, a stretch per call ([`line`]).
    moving: Option<line::Move>,
    /// The line the unit no longer uses, to be freed off the audio thread
    /// ([`Self::take_spent`]).
    spent: Option<Box<Lines>>,
    /// After a reset of a line that held sound: the cells from the write position up to
    /// this one still hold it, and are read as the zeros they stand for until cleared (0:
    /// none).
    stale: usize,
    /// Cells copied or cleared by the last call (both sides).
    work: usize,
    /// The note whose synced line the unit holds at least ([`Self::set_line_floor`]).
    floor: Option<usize>,
}

impl StereoDelay {
    /// fx/final — the time and the mix at their targets (`FxUnit::snap`: a start, a seek, a
    /// loop's wrap); the lines keep what they hold.
    pub fn snap(&mut self) {
        self.time.land();
        self.mix.land();
    }

    pub fn new(sample_rate: f32) -> Self {
        let len = (MAX_DELAY_SECONDS * sample_rate) as usize + 2;
        let s = DelaySettings::default();
        let time = Smoother::new(s.time_ms * 0.001 * sample_rate, GLIDE_S, sample_rate);
        let mut d = StereoDelay {
            sample_rate,
            buf_l: vec![0.0; len],
            buf_r: vec![0.0; len],
            write: 0,
            time,
            mix: Smoother::new(s.mix, 0.02, sample_rate),
            lp_l: 0.0,
            lp_r: 0.0,
            lp_coef: 1.0,
            settings: s.clone(),
            quiet: [ZeroRun::FULL; 2],
            n: 0,
            tempo: sync::tempo_or_default(f64::NAN),
            synced: sync::Head::new(1.0),
            fade: (GLIDE_S * sample_rate).round().max(1.0) as u32,
            arm: false,
            moving: None,
            spent: None,
            stale: 0,
            work: 0,
            floor: None,
        };
        d.set(&s);
        d
    }

    pub fn settings(&self) -> &DelaySettings {
        &self.settings
    }

    /// The tempo the unit plays its synced time at.
    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    /// The longest delay the line holds now, in samples.
    fn max_samples(&self) -> f32 {
        (self.buf_l.len() - 2) as f32
    }

    /// The synced time at the current tempo, in samples, held to the line.
    fn synced_target(&self, note: usize) -> f64 {
        let max = f64::from(self.max_samples());
        sync::time_samples(note, self.tempo, self.sample_rate).clamp(1.0, max)
    }

    pub fn set(&mut self, s: &DelaySettings) {
        let max_samples = self.max_samples();
        let samples = (s.time_ms.max(0.0) * 0.001 * self.sample_rate).clamp(1.0, max_samples);
        if s.sync {
            if !self.settings.sync {
                // From the free time to the synced one: the head goes on from where it is,
                // to the note's time at the tempo the next pass is told (A-L1).
                self.synced.snap(f64::from(self.time.value()));
                self.arm = true;
            } else if !self.arm {
                let target = self.synced_target(s.note);
                if target != self.synced.goal() {
                    self.synced.go(target, None, self.fade);
                }
            }
            // The free time waits where it would be, settled, for the sync to go off.
            self.time.snap(samples);
        } else {
            if self.settings.sync {
                // Back to the free time: on from where the synced read head is.
                self.time.snap(self.synced.current as f32);
            }
            self.arm = false;
            self.time.set_target(samples);
        }
        self.mix.set_target(s.mix.clamp(0.0, 1.0));
        let fc = s.tone_hz.clamp(20.0, self.sample_rate * 0.45);
        self.lp_coef = 1.0 - math::exp(-std::f32::consts::TAU * fc / self.sample_rate);
        self.settings = s.clone();
    }

    /// The tempo of the next pass (the engine's `SongPosition::tempo_bpm`), and whether it
    /// is a step of a ramp (`tempo_inc` is not 0). Only the synced time hears it: inside a
    /// ramp the head goes straight to the step's time over the step (a sixteenth of a
    /// beat); otherwise it glides a small move and crossfades a jump ([`sync::Head`]); while
    /// the lines hold only silence, where nothing can be heard moving, it is there at once.
    /// The first tempo after the sync is switched on sends the head to its note (A-L1).
    /// No allocation.
    pub fn set_tempo(&mut self, bpm: f64, ramp: bool) {
        let bpm = sync::tempo_or_default(bpm);
        if self.arm {
            self.tempo = bpm;
            self.go_armed();
            return;
        }
        if bpm == self.tempo {
            return;
        }
        self.tempo = bpm;
        if !self.settings.sync {
            return;
        }
        let target = self.synced_target(self.settings.note);
        if self.silent() {
            self.synced.snap(target);
        } else {
            let step = (60.0 / bpm / 16.0 * f64::from(self.sample_rate))
                .round()
                .max(1.0);
            self.synced
                .go(target, ramp.then_some(step as u32), self.fade);
        }
    }

    /// The sync switched on: the head to the note's time at the tempo it has now (a jump,
    /// crossfaded; at once over silence).
    fn go_armed(&mut self) {
        self.arm = false;
        let target = self.synced_target(self.settings.note);
        if self.silent() {
            self.synced.snap(target);
        } else if target != self.synced.goal() {
            self.synced.go(target, None, self.fade);
        }
    }

    /// Both lines and the tone filter hold only zeros.
    fn silent(&self) -> bool {
        let len = self.buf_l.len();
        crate::rest::z(self.lp_l)
            && crate::rest::z(self.lp_r)
            && self.quiet[0].covers(len)
            && self.quiet[1].covers(len)
    }

    /// Clear the line and jump smoothers to their targets. A line that held sound is not
    /// written whole here (a 48 s line is 18 MB): what it holds is read as zeros and
    /// cleared a stretch per call after (A-M3), so the output is a cleared line's, to the
    /// bit. A line on its way is taken now ([`line`]).
    pub fn reset(&mut self) {
        let loud = match self.finish_move_for_reset() {
            // A line on its way, taken now: it holds what it was given, or nothing yet.
            Some(copied) => copied,
            // A line an earlier reset left stale still holds its sound under the mask
            // (`quiet` says silent only because the reads see zeros): it stays masked —
            // a second reset before the clearing ends never brings it back (fix-c, R2-A1).
            None => self.stale > 0 || !self.silent_lines(),
        };
        self.write = 0;
        self.n = 0;
        self.stale = if loud { self.buf_l.len() } else { 0 };
        self.lp_l = 0.0;
        self.lp_r = 0.0;
        let max_samples = self.max_samples();
        self.time
            .snap((self.settings.time_ms * 0.001 * self.sample_rate).clamp(1.0, max_samples));
        if self.settings.sync {
            let t = self.synced_target(self.settings.note);
            self.synced.snap(t);
        }
        self.mix.snap(self.settings.mix.clamp(0.0, 1.0));
        self.quiet = [ZeroRun::FULL; 2];
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): the write position moves
    /// as processing would have moved it. It is not idle bookkeeping — the read head's
    /// fraction is worked out from the absolute position, so it is in the output's bits.
    pub fn skip(&mut self, frames: usize) {
        self.maintain(frames);
        self.skip_stale(frames);
        let len = self.buf_l.len();
        self.write = (self.write + frames % len) % len;
        self.n = self.n.wrapping_add(frames as u64);
    }

    /// At rest (ADR 0031 §5.1): both lines hold only zeros, the feedback tone filter is
    /// empty and the time and mix glides have landed — what [`Self::reset`] leaves, with
    /// the write position that [`Self::skip`] keeps — on the line its settings want, none
    /// on its way.
    pub fn at_rest(&self) -> bool {
        let glide = if self.settings.sync {
            self.synced.is_settled() && !self.arm
        } else {
            self.time.is_settled()
        };
        self.silent()
            && glide
            && self.mix.is_settled()
            && self.need().is_none()
            && self.moving.is_none()
    }

    /// The cells of line the settings want (ADR 0119): the free line's with the sync off —
    /// or, when the engine set a floor, at least the line of that note synced.
    pub fn wanted_len(&self) -> usize {
        let s = &self.settings;
        let own = sync::line_seconds(s.sync, s.note);
        let floor = self
            .floor
            .map_or(own, |n| own.max(sync::line_seconds(true, n)));
        sync::line_len(floor, self.sample_rate)
    }

    /// The longest note the song's automation or modulation of `note` and `sync` can reach
    /// (fix-a, A-M2): the line is built for it before the song gets there — so a note
    /// automated longer never waits for its line, and offline equals real time — and kept
    /// with the sync off too (switching the sync by automation asks for nothing). `None`
    /// (every unit nobody automates): the line its settings want.
    pub fn set_line_floor(&mut self, note: Option<usize>) {
        self.floor = note.map(|n| n.min(sync::NOTES - 1));
    }

    /// The line to build off the audio thread, in cells, when the settings want another
    /// length than the unit holds (or is being given); `None` when it has the right one.
    pub fn need(&self) -> Option<usize> {
        let want = self.wanted_len();
        let have = self
            .moving
            .as_ref()
            .map_or(self.buf_l.len(), |m| m.lines.len());
        (want != have).then_some(want)
    }

    /// The free line's cells (what [`Self::new`] builds): a line longer than this was
    /// built for a synced note or a floor.
    fn free_len(&self) -> usize {
        (MAX_DELAY_SECONDS * self.sample_rate) as usize + 2
    }

    /// Both lines hold only zeros.
    fn silent_lines(&self) -> bool {
        let len = self.buf_l.len();
        self.quiet[0].covers(len) && self.quiet[1].covers(len)
    }

    /// The free time's read: the sample `delay` before the write position, between the two
    /// around it, worked out from the position as it always was (in `f32`: exact enough on
    /// the free 2 s line, a quarter sample off 2 M cells along — a longer line reads by
    /// whole and fraction, [`sync::read_masked`]).
    #[cfg(test)]
    #[inline]
    fn read(buf: &[f32], write: usize, delay: f32) -> f32 {
        Self::read_free::<false>(buf, write, delay, 0)
    }

    /// [`Self::read`], the cells a reset left (from `write` up to `stale`) read as zeros
    /// when `MASK` — a copy of its own, so the unmasked one is the instructions it was.
    #[inline(always)]
    fn read_free<const MASK: bool>(buf: &[f32], write: usize, delay: f32, stale: usize) -> f32 {
        let len = buf.len();
        let pos = write as f32 - delay;
        let pos = if pos < 0.0 { pos + len as f32 } else { pos };
        let i0 = pos.floor() as usize % len;
        let i1 = (i0 + 1) % len;
        let frac = pos - pos.floor();
        let a = sync::cell::<MASK>(buf, i0, write, stale);
        let b = sync::cell::<MASK>(buf, i1, write, stale);
        a + (b - a) * frac
    }
}

impl StereoDelay {
    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        self.maintain(n);
        if self.arm {
            // Switched on and never told a tempo since: the one it has.
            self.go_armed();
        }
        // The free time on a line longer than the free one (a floor kept it long) is read by
        // its whole and fraction (fix-c, R2-A3); on the free line, as it always was.
        let wide = self.buf_l.len() > self.free_len();
        match (self.settings.sync, self.stale > 0, wide) {
            (false, false, false) => self.process_free::<false, false>(&mut l[..n], &mut r[..n]),
            (false, true, false) => self.process_free::<true, false>(&mut l[..n], &mut r[..n]),
            (false, false, true) => self.process_free::<false, true>(&mut l[..n], &mut r[..n]),
            (false, true, true) => self.process_free::<true, true>(&mut l[..n], &mut r[..n]),
            (true, false, _) => self.process_synced::<false>(&mut l[..n], &mut r[..n]),
            (true, true, _) => self.process_synced::<true>(&mut l[..n], &mut r[..n]),
        }
        self.quiet[0].wrote(&self.buf_l, self.write, n);
        self.quiet[1].wrote(&self.buf_r, self.write, n);
        self.n = self.n.wrapping_add(n as u64);
        self.end_stale();
    }

    /// The free time: the unit as it always was, instruction for instruction — or, `WIDE`,
    /// on a line kept longer than the free one, read as the synced head reads.
    fn process_free<const MASK: bool, const WIDE: bool>(&mut self, l: &mut [f32], r: &mut [f32]) {
        let fb = self.settings.feedback.clamp(0.0, 0.98);
        let ping_pong = self.settings.ping_pong;
        let len = self.buf_l.len();
        let stale = self.stale;
        for i in 0..l.len() {
            let d = self.time.next();
            let mix = self.mix.next();
            let (rd_l, rd_r) = if WIDE {
                let d = f64::from(d);
                (
                    sync::read_masked::<MASK>(&self.buf_l, self.write, d, stale),
                    sync::read_masked::<MASK>(&self.buf_r, self.write, d, stale),
                )
            } else {
                (
                    Self::read_free::<MASK>(&self.buf_l, self.write, d, stale),
                    Self::read_free::<MASK>(&self.buf_r, self.write, d, stale),
                )
            };
            let (ol, or) = self.pass(l[i], r[i], rd_l, rd_r, mix, fb, ping_pong, len);
            l[i] = ol;
            r[i] = or;
        }
    }

    /// The synced time: the read head at the `f64` glide, read by its whole and fraction
    /// ([`sync::read`]), so a line that grows mid-song changes no sample.
    fn process_synced<const MASK: bool>(&mut self, l: &mut [f32], r: &mut [f32]) {
        let fb = self.settings.feedback.clamp(0.0, 0.98);
        let ping_pong = self.settings.ping_pong;
        let len = self.buf_l.len();
        let max = f64::from(self.max_samples());
        let fade = self.fade;
        let stale = self.stale;
        for i in 0..l.len() {
            let (a, b, g) = self.synced.next(fade);
            let mix = self.mix.next();
            let b = b.clamp(1.0, max);
            let mut rd_l = sync::read_masked::<MASK>(&self.buf_l, self.write, b, stale);
            let mut rd_r = sync::read_masked::<MASK>(&self.buf_r, self.write, b, stale);
            if g < 1.0 {
                // A crossfade from the old head.
                let a = a.clamp(1.0, max);
                let (ol, or) = (
                    sync::read_masked::<MASK>(&self.buf_l, self.write, a, stale),
                    sync::read_masked::<MASK>(&self.buf_r, self.write, a, stale),
                );
                rd_l = ol + (rd_l - ol) * g;
                rd_r = or + (rd_r - or) * g;
            }
            let (ol, or) = self.pass(l[i], r[i], rd_l, rd_r, mix, fb, ping_pong, len);
            l[i] = ol;
            r[i] = or;
        }
    }

    /// One sample through the feedback path, given what the heads read.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn pass(
        &mut self,
        in_l: f32,
        in_r: f32,
        rd_l: f32,
        rd_r: f32,
        mix: f32,
        fb: f32,
        ping_pong: bool,
        len: usize,
    ) -> (f32, f32) {
        // Tone: one-pole low-pass in the feedback path.
        self.lp_l += self.lp_coef * (rd_l - self.lp_l);
        self.lp_r += self.lp_coef * (rd_r - self.lp_r);
        flush(&mut self.lp_l);
        flush(&mut self.lp_r);
        let (wr_l, wr_r) = if ping_pong {
            ((in_l + in_r) * 0.5 + fb * self.lp_r, fb * self.lp_l)
        } else {
            (in_l + fb * self.lp_l, in_r + fb * self.lp_r)
        };
        self.buf_l[self.write] = wr_l;
        self.buf_r[self.write] = wr_r;
        self.write = (self.write + 1) % len;
        (in_l + (rd_l - in_l) * mix, in_r + (rd_r - in_r) * mix)
    }
}

#[inline]
fn flush(x: &mut f32) {
    if x.abs() < 1e-20 {
        *x = 0.0;
    }
}

#[cfg(test)]
#[path = "delay_tests.rs"]
mod tests;

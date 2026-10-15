//! The modulation family's clock (ADR 0055): where a modulation effect's LFO reads time.
//!
//! Two times run beside each other, both integers so no block split can round them:
//!
//! - **The song sample** — the transport's position in samples at the current tempo
//!   ([`Transport::samples`]). A *synced* LFO is a function of it alone, so its phase is a
//!   function of the song position: a render from bar 5 hears what a full render hears from
//!   bar 5, offline equals real time, and a loop that wraps puts the LFO back where it was.
//! - **The free sample** — samples since playback last started (or since the unit was made,
//!   while nothing has played). A *free* LFO runs on it, so it is deterministic from the
//!   start of playback.
//!
//! The transport arrives once per pass ([`Clock::set`]); between passes the clock counts
//! on its own. It takes a new anchor only when the song **jumps** — a seek, a loop's wrap,
//! a tempo change, the start of playback — never because a pass split the stream, so the
//! times are the same at every block size. While the transport is stopped the song sample
//! keeps counting from where it was (a synced chorus does not freeze on stop).
//!
//! Nothing here is state that holds sound: a unit built on this clock is at rest when its
//! audio state is, and [`Clock::skip`] advances it exactly as processing silence would.

/// The transport as the modulation family reads it (the engine's `SongPosition`, reduced).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transport {
    pub playing: bool,
    /// Beats per minute; 0 or less means "not known" (120 is assumed).
    pub tempo_bpm: f64,
    /// Samples from the song start at the current tempo.
    pub samples: i64,
}

/// The tempo a synced LFO assumes until a host says otherwise.
pub const DEFAULT_TEMPO: f64 = 120.0;

/// How far the transport may disagree with the clock's own count, in samples, before it
/// counts as a jump: the transport's position is a float accumulated over passes, and its
/// rounding to samples may wobble by one.
const SLACK: i64 = 2;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Clock {
    /// Samples processed (or skipped) since reset: the stream clock.
    n: i64,
    /// The song sample at stream sample `anchor_n`.
    anchor_song: i64,
    anchor_n: i64,
    /// The stream sample at which playback last started.
    play_n: i64,
    tempo: f64,
    playing: bool,
    /// Bumped at every start of playback: an LFO that runs free starts its cycle again.
    epoch: u32,
    /// Bumped at every new anchor of the song sample (a seek, a wrap, a tempo change).
    jumps: u32,
}

impl Default for Clock {
    fn default() -> Self {
        Clock::new()
    }
}

impl Clock {
    pub const fn new() -> Clock {
        Clock {
            n: 0,
            anchor_song: 0,
            anchor_n: 0,
            play_n: 0,
            tempo: DEFAULT_TEMPO,
            playing: false,
            epoch: 0,
            jumps: 0,
        }
    }

    /// Back to the start of a stream: no transport heard yet.
    pub fn reset(&mut self) {
        *self = Clock::new();
    }

    /// The transport at the start of the next `process` call.
    pub fn set(&mut self, t: &Transport) {
        let tempo = if t.tempo_bpm.is_finite() && t.tempo_bpm > 0.0 {
            t.tempo_bpm
        } else {
            DEFAULT_TEMPO
        };
        let started = t.playing && !self.playing;
        let tempo_moved = tempo != self.tempo;
        let jumped = t.playing && (t.samples - self.song(0)).abs() > SLACK;
        if started {
            self.play_n = self.n;
            self.epoch = self.epoch.wrapping_add(1);
        }
        if started || tempo_moved || jumped {
            self.anchor_song = t.samples;
            self.anchor_n = self.n;
            self.jumps = self.jumps.wrapping_add(1);
        }
        self.tempo = tempo;
        self.playing = t.playing;
    }

    /// The song sample `i` samples into the next call.
    #[inline]
    pub fn song(&self, i: usize) -> i64 {
        self.anchor_song + (self.n - self.anchor_n) + i as i64
    }

    /// The free sample `i` samples into the next call: samples since playback started.
    #[inline]
    pub fn free(&self, i: usize) -> i64 {
        self.n - self.play_n + i as i64
    }

    /// The stream sample `i` samples into the next call: samples since reset.
    #[inline]
    pub fn stream(&self, i: usize) -> i64 {
        self.n + i as i64
    }

    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    /// Changes at every start of playback.
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Changes at every new anchor of the song sample.
    pub fn jumps(&self) -> u32 {
        self.jumps
    }

    /// `frames` were processed.
    #[inline]
    pub fn advance(&mut self, frames: usize) {
        self.n += frames as i64;
    }

    /// `frames` of silence were skipped: exactly [`Clock::advance`].
    #[inline]
    pub fn skip(&mut self, frames: usize) {
        self.advance(frames);
    }
}

/// Beats in a cycle of each synced rate, as the modulation family offers them
/// (`modfx.div.{i}` in the locale files): whole notes down to 1/32, dotted (·) and
/// triplet (T) values between.
pub const DIVISIONS: [f64; 17] = [
    32.0,      // 8/1
    16.0,      // 4/1
    8.0,       // 2/1
    4.0,       // 1/1
    3.0,       // 1/2·
    2.0,       // 1/2
    1.5,       // 1/4·
    4.0 / 3.0, // 1/2T
    1.0,       // 1/4
    0.75,      // 1/8·
    2.0 / 3.0, // 1/4T
    0.5,       // 1/8
    0.375,     // 1/16·
    1.0 / 3.0, // 1/8T
    0.25,      // 1/16
    1.0 / 6.0, // 1/16T
    0.125,     // 1/32
];

/// The index of 1/4 in [`DIVISIONS`].
pub const DIV_QUARTER: usize = 8;
/// The index of 1/8 in [`DIVISIONS`].
pub const DIV_EIGHTH: usize = 11;
/// The index of 1/1 (a bar of 4/4) in [`DIVISIONS`].
pub const DIV_WHOLE: usize = 3;

/// The beats of division `index` (clamped into the table).
pub fn division_beats(index: f32) -> f64 {
    let i = if index.is_finite() {
        index.round().clamp(0.0, (DIVISIONS.len() - 1) as f32) as usize
    } else {
        DIV_QUARTER
    };
    DIVISIONS[i]
}

/// Cycles per sample of a synced rate: `beats` per cycle at `tempo` and `sample_rate`.
#[inline]
pub fn synced_inc(beats: f64, tempo: f64, sample_rate: f64) -> f64 {
    tempo / (60.0 * sample_rate * beats.max(1e-9))
}

/// The telemetry every unit of the family starts with (64 floats, ADR 0018): what its
/// editor draws the LFO's playhead and the meters from. A unit's own numbers start at
/// [`tel::OWN`].
pub mod tel {
    /// Sample rate, Hz.
    pub const SAMPLE_RATE: usize = 0;
    /// The tempo the clock runs at, BPM.
    pub const TEMPO: usize = 1;
    /// 1 while the transport plays.
    pub const PLAYING: usize = 2;
    /// The LFO's phase (0 … 1) at the end of the last block, left and right.
    pub const PHASE_L: usize = 3;
    pub const PHASE_R: usize = 4;
    /// The modulation's value (−1 … 1) at the end of the last block, left and right.
    pub const MOD_L: usize = 5;
    pub const MOD_R: usize = 6;
    /// Peaks of the last block, dBFS (−200 for silence).
    pub const IN_PEAK: usize = 7;
    pub const OUT_PEAK: usize = 8;
    /// Blocks processed, modulo 2²⁴: an editor tells new telemetry from old.
    pub const BLOCKS: usize = 9;
    /// Where a unit's own numbers start.
    pub const OWN: usize = 10;
}

/// A peak as dBFS for the telemetry: −200 for silence.
#[inline]
pub fn peak_db(peak: f32) -> f32 {
    if peak > 1e-10 {
        20.0 * crate::math::log10(peak)
    } else {
        -200.0
    }
}

/// The largest magnitude in two channels.
#[inline]
pub fn peak_of(l: &[f32], r: &[f32]) -> f32 {
    l.iter().chain(r).fold(0.0f32, |m, x| m.max(x.abs()))
}

/// A 64-bit hash (SplitMix64's finaliser): every random choice in the family is one of
/// these of a seed and a count.
#[inline]
pub fn hash64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A value in −1 … 1 from a hash of `seed` and `n`.
#[inline]
pub fn random_bipolar(seed: u64, n: i64) -> f32 {
    let h = hash64(seed ^ hash64(n as u64));
    ((h >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(playing: bool, samples: i64) -> Transport {
        Transport {
            playing,
            tempo_bpm: 120.0,
            samples,
        }
    }

    #[test]
    fn a_continuing_transport_never_moves_the_anchor() {
        let mut c = Clock::new();
        c.set(&t(true, 1_000));
        let j = c.jumps();
        for _ in 0..100 {
            c.advance(37);
            // The transport's rounding may wobble by a sample.
            let s = c.song(0) + if c.n % 2 == 0 { 1 } else { -1 };
            c.set(&t(true, s));
        }
        assert_eq!(c.jumps(), j);
        assert_eq!(c.song(0), 1_000 + 3_700);
    }

    #[test]
    fn a_seek_and_a_tempo_change_take_a_new_anchor() {
        let mut c = Clock::new();
        c.set(&t(true, 0));
        c.advance(512);
        c.set(&t(true, 96_000));
        assert_eq!(c.song(5), 96_005);
        c.advance(10);
        c.set(&Transport {
            playing: true,
            tempo_bpm: 60.0,
            samples: 192_020,
        });
        assert_eq!(c.song(0), 192_020);
        assert_eq!(c.tempo(), 60.0);
    }

    #[test]
    fn stopped_it_keeps_counting_and_play_restarts_the_free_time() {
        let mut c = Clock::new();
        c.set(&t(false, 0));
        c.advance(100);
        c.set(&t(false, 0));
        assert_eq!(
            c.song(0),
            100,
            "a stopped transport does not freeze the song time"
        );
        assert_eq!(c.free(0), 100);
        let e = c.epoch();
        c.set(&t(true, 48_000));
        assert_ne!(c.epoch(), e);
        assert_eq!(c.free(0), 0);
        assert_eq!(c.song(0), 48_000);
    }

    #[test]
    fn skip_is_advance() {
        let mut a = Clock::new();
        let mut b = Clock::new();
        a.advance(1234);
        b.skip(1234);
        assert_eq!(a, b);
    }

    #[test]
    fn the_divisions_are_note_values() {
        assert_eq!(DIVISIONS[DIV_QUARTER], 1.0);
        assert_eq!(DIVISIONS[DIV_EIGHTH], 0.5);
        assert_eq!(DIVISIONS[DIV_WHOLE], 4.0);
        assert!(DIVISIONS.windows(2).all(|w| w[0] > w[1]));
        assert_eq!(division_beats(f32::NAN), 1.0);
        assert_eq!(division_beats(99.0), 0.125);
    }

    #[test]
    fn random_values_are_a_function_of_seed_and_count() {
        assert_eq!(random_bipolar(7, 3), random_bipolar(7, 3));
        assert_ne!(random_bipolar(7, 3), random_bipolar(7, 4));
        let v: Vec<f32> = (0..1000).map(|n| random_bipolar(1, n)).collect();
        assert!(v.iter().all(|x| (-1.0..=1.0).contains(x)));
        let mean = v.iter().sum::<f32>() / 1000.0;
        assert!(mean.abs() < 0.1);
    }
}

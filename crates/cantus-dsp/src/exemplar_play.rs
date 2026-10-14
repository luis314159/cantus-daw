//! Exemplar's playhead: where a voice is in its sample, the loop it follows, and the
//! resampler state that reads it (`exemplar_resample`).
//!
//! **Positions are unwrapped.** A voice's position only ever grows; a loop is a mapping
//! from that position to the file (lap *j* of a forward loop reads `i − j·len`). Releasing
//! a sustain loop just says from which lap on the mapping stops wrapping, so audio the
//! reader has already fetched never changes under it and the tail follows seamlessly.
//! Crossfades are applied to the mapped signal tap by tap, so the interpolator sees one
//! continuous looped signal.

use super::resample::{Kernel, Quality, Reader, MAX_HB, MAX_LEVELS, MAX_TAPS};
use super::stream::{Board, Cursor, Stream, BLOCK};
use std::sync::Arc;

// Fetching taps through the loop, interpolating, decimating.
#[path = "exemplar_read.rs"]
mod read;

// A streamed voice's window (ADR 0050).
#[path = "exemplar_window.rs"]
mod window;

pub use window::Reach;

/// A sample as a voice reads it: its mip-map levels (level 0 is the file), interleaved.
#[derive(Clone, Debug, Default)]
pub struct Source {
    pub levels: [Option<Arc<[f32]>>; MAX_LEVELS],
    /// 1 or 2.
    pub channels: usize,
    pub rate: f32,
    /// Frames at level 0.
    pub frames: usize,
    /// Cents the sample is sharp of its nominal root (built-in sounds): subtracted.
    pub tune_ct: f32,
    /// Streamed from disk (ADR 0050): `levels` then hold only each level's head.
    pub stream: Option<Arc<Stream>>,
}

impl Source {
    /// A streamed sample as a voice reads it: the heads in memory, the rest from the pool.
    pub fn from_stream(st: &Arc<Stream>) -> Source {
        let mut levels: [Option<Arc<[f32]>>; MAX_LEVELS] = Default::default();
        for (l, h) in levels.iter_mut().zip(st.heads.iter()) {
            l.clone_from(h);
        }
        Source {
            levels,
            channels: st.channels,
            rate: st.rate,
            frames: st.frames[0],
            tune_ct: 0.0,
            stream: Some(st.clone()),
        }
    }

    /// Keep levels `0..n` only — a zone's own mip-maps out of a stream that holds every
    /// level its file has, so it plays exactly as its in-memory sample would.
    pub fn limit_levels(&mut self, n: usize) {
        for l in self.levels.iter_mut().skip(n.max(1)) {
            *l = None;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.levels[0].is_none() || self.frames == 0
    }
    /// The highest level present.
    pub fn top_level(&self) -> usize {
        (0..MAX_LEVELS)
            .rev()
            .find(|k| self.levels[*k].is_some())
            .unwrap_or(0)
    }
    /// Bytes of audio held, all levels.
    pub fn bytes(&self) -> usize {
        self.levels.iter().flatten().map(|l| l.len() * 4).sum()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LoopDir {
    #[default]
    Forward,
    PingPong,
    Backward,
}

/// What part of the file plays and how it loops, in level-0 frames.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Region {
    pub start: i64,
    /// Exclusive.
    pub end: i64,
    pub looping: bool,
    /// Loop start and end, both inclusive (SFZ and the WAV `smpl` chunk agree).
    pub ls: i64,
    pub le: i64,
    pub dir: LoopDir,
    /// Crossfade length in frames, before the loop end.
    pub xf: i64,
    /// Equal-power rather than linear crossfade.
    pub xf_power: bool,
    pub reverse: bool,
}

impl Region {
    /// Clamp everything into a file of `frames` frames.
    pub fn sanitized(mut self, frames: i64) -> Region {
        let frames = frames.max(1);
        self.end = if self.end <= 0 || self.end > frames {
            frames
        } else {
            self.end
        };
        self.start = self.start.clamp(0, self.end - 1);
        if self.looping {
            if self.le <= 0 || self.le >= self.end {
                self.le = self.end - 1;
            }
            self.ls = self.ls.clamp(0, self.le);
            if self.le - self.ls < 1 || self.reverse {
                self.looping = false;
            }
            let len = self.le - self.ls + 1;
            self.xf = self.xf.clamp(0, self.ls.min(len - 1));
            if self.dir != LoopDir::Forward {
                self.xf = 0;
            }
        } else {
            self.xf = 0;
        }
        self
    }
    /// Frames in the loop.
    pub fn loop_len(&self) -> i64 {
        self.le - self.ls + 1
    }
}

/// A region scaled to one mip level.
#[derive(Copy, Clone, Debug, Default)]
struct Lvl {
    start: i64,
    end: i64,
    ls: i64,
    le: i64,
    len: i64,
    xf: i64,
    frames: i64,
}

fn scale(r: &Region, frames0: usize, k: usize) -> Lvl {
    let f = (1i64 << k) as f64;
    let d = |x: i64| (x as f64 / f).round() as i64;
    let len = if r.looping { d(r.loop_len()).max(2) } else { 0 };
    let ls = d(r.ls);
    Lvl {
        start: d(r.start),
        end: (r.end as f64 / f).ceil() as i64,
        ls,
        le: ls + len - 1,
        len,
        xf: d(r.xf).min((len - 1).max(0)).min(ls),
        frames: (frames0 as i64 + (1 << k) - 1) >> k,
    }
}

/// How the playhead is reading right now.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mode {
    /// Ratio exactly 1 at an integral position: the samples themselves, bit for bit.
    Exact,
    /// Draft quality: linear interpolation on the file.
    Linear,
    /// Mip-map level, sinc at 2×, half-band decimation.
    Resampled,
}

/// A voice's reader.
#[derive(Clone, Debug)]
pub struct Playhead {
    src: Source,
    region: Region,
    lvl: Lvl,
    level: usize,
    mode: Mode,
    /// Unwrapped position of the current output frame, in level-0 frames.
    pos: f64,
    /// From this lap on the loop no longer wraps (sustain loop released); `i64::MAX` while
    /// it loops.
    release_lap: i64,
    /// The furthest level-0 index fetched, so a release never rewrites audio already read.
    furthest: i64,
    /// The decimator's history, split by phase (a polyphase half-band): the first sample
    /// of each output pair feeds only the centre tap (`u1`, the last M of them), the
    /// second feeds every other tap (`u2`, the last 2M). Doubled rings, so a window is
    /// one contiguous slice.
    u1: [[f32; 2 * HB_M]; 2],
    u2: [[f32; 4 * HB_M]; 2],
    p1: usize,
    p2: usize,
    done: bool,
    /// A streamed source's window (ADR 0050): frames `win_base..win_base + win_len` of
    /// level `win_level`, interleaved, in a buffer allocated once ([`Self::attach_window`]).
    win: Vec<f32>,
    win_cap: usize,
    win_base: i64,
    win_len: usize,
    win_level: usize,
    /// Output frames the window keeps beyond what is rendered (a fade's worth).
    fade: usize,
    /// A read missed a block (counted as late by the engine).
    missed: std::cell::Cell<bool>,
    /// Where it says what it reads: its instrument's board and its cell.
    board: Option<(Arc<Board>, u16)>,
    published: u64,
    /// Offline: wait for a block rather than fade (ADR 0050 §6).
    blocking: bool,
    /// Decided once, at the voice's first read (second review #17): a control step reads
    /// more than the stream reader keeps ready ahead — faster than it serves in real time —
    /// so the voice plays its head and ends there, alike in real time and offline. `None`:
    /// not decided.
    pub(super) head_only: Option<bool>,
    /// The local ratio above which it is decided so ([`Self::set_head_only_above`]).
    pub(super) head_only_above: f64,
    /// Times it woke the stream reader since the last [`Self::take_wakes`] (fourth review
    /// #1: a counted system call on the audio thread, only when the reader is needed).
    pub(super) wakes: u32,
}

/// The most odd taps a runtime decimator has (`MAX_HB / 4`).
pub const HB_M: usize = MAX_HB / 4;

impl Default for Playhead {
    fn default() -> Self {
        Playhead {
            src: Source::default(),
            region: Region::default(),
            lvl: Lvl::default(),
            level: 0,
            mode: Mode::Exact,
            pos: 0.0,
            release_lap: i64::MAX,
            furthest: 0,
            u1: [[0.0; 2 * HB_M]; 2],
            u2: [[0.0; 4 * HB_M]; 2],
            p1: 0,
            p2: 0,
            done: true,
            win: Vec::new(),
            win_cap: 0,
            win_base: 0,
            win_len: 0,
            win_level: 0,
            fade: 0,
            missed: std::cell::Cell::new(false),
            board: None,
            published: 0,
            blocking: false,
            head_only: None,
            head_only_above: 64.0,
            wakes: 0,
        }
    }
}

impl Playhead {
    /// Start reading `src` over `region` at a ratio of `ratio` source frames per output
    /// frame. `offset` frames past the region's start (a velocity-driven start).
    pub fn start(&mut self, src: Source, region: Region, ratio: f64, reader: &Reader, offset: i64) {
        let mut region = region.sanitized(src.frames as i64);
        if src.stream.is_some() {
            // Never asked for (looped and reversed samples stay in memory), so never heard:
            // a streamed sample plays forwards.
            region.looping = false;
            region.reverse = false;
        }
        self.win_len = 0;
        self.head_only = None;
        self.src = src;
        self.region = region;
        self.pos = (region.start + offset.max(0)).min(region.end - 1) as f64;
        self.release_lap = i64::MAX;
        self.furthest = self.pos as i64;
        self.done = self.src.is_empty();
        self.level = 0;
        self.lvl = scale(&self.region, self.src.frames, 0);
        self.mode = if ratio == 1.0 {
            Mode::Exact
        } else if reader.quality == Quality::Draft {
            Mode::Linear
        } else {
            Mode::Resampled
        };
        if self.mode == Mode::Resampled {
            self.choose_level(ratio, true);
            self.ensure(ratio, reader, super::voice::CONTROL as usize, false);
            self.prime(ratio, reader);
        }
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn source(&self) -> &Source {
        &self.src
    }

    /// Position in the file (level-0 frames, the loop folded in), for the editor.
    pub fn file_position(&self) -> f64 {
        let i = self.pos.floor() as i64;
        let frac = self.pos - i as f64;
        let r = &self.region;
        let j = if r.reverse {
            r.end - 1 - (i - r.start)
        } else if !r.looping || i <= r.le {
            i
        } else {
            let o = i - r.le - 1;
            let len = r.loop_len();
            match r.dir {
                LoopDir::Forward => {
                    let lap = o / len + 1;
                    if lap >= self.release_lap {
                        i - self.release_lap * len
                    } else {
                        i - lap * len
                    }
                }
                LoopDir::Backward => r.le - o % len,
                LoopDir::PingPong => {
                    let lap = o / (len - 1).max(1) + 1;
                    let t = o % (len - 1).max(1);
                    if lap % 2 == 1 {
                        r.le - 1 - t
                    } else {
                        r.ls + 1 + t
                    }
                }
            }
        };
        j as f64 + frac
    }

    /// Mip level `k` has local ratio r/2^k; keep it in [0.9, 2) (hysteresis, so vibrato
    /// around an octave boundary does not flap).
    fn choose_level(&mut self, ratio: f64, fresh: bool) {
        let top = self.src.top_level();
        let mut k = self.level;
        if fresh {
            k = 0;
            while k < top && ratio / (1u64 << k) as f64 >= 2.0 {
                k += 1;
            }
        } else {
            while k < top && ratio / (1u64 << k) as f64 >= 2.0 {
                k += 1;
            }
            while k > 0 && ratio / ((1u64 << k) as f64) < 0.9 {
                k -= 1;
            }
        }
        if k != self.level || fresh {
            self.level = k;
            self.lvl = scale(&self.region, self.src.frames, k);
        }
    }

    /// A pitch change (modulation, bend): pick the level again, and leave the exact path
    /// for the resampler when the ratio is no longer 1.
    pub fn retune(&mut self, ratio: f64, reader: &Reader) {
        match self.mode {
            Mode::Exact if ratio != 1.0 => {
                if reader.quality == Quality::Draft {
                    self.mode = Mode::Linear;
                } else {
                    self.mode = Mode::Resampled;
                    self.choose_level(ratio, true);
                    self.ensure(ratio, reader, super::voice::CONTROL as usize, false);
                    self.prime(ratio, reader);
                }
            }
            Mode::Resampled => self.choose_level(ratio, false),
            _ => {}
        }
    }

    /// Fill the decimator's history as if the voice had been reading all along: the
    /// 2M − 1 output steps before this one, each a pair of oversampled values.
    fn prime(&mut self, ratio: f64, reader: &Reader) {
        let m = reader.dec.odd.len();
        let d2 = (reader.dec.len / 2) as f64;
        let scale = (1u64 << self.level) as f64;
        let s = ratio / scale / 2.0;
        let pk = self.pos / scale;
        self.p1 = 0;
        self.p2 = 0;
        let mut coefs = [0.0f32; MAX_TAPS];
        for n in -(2 * m as i64 - 1)..=-1 {
            let q1 = pk + (2.0 * n as f64 - 1.0 + d2) * s;
            let v1 = self.interp(q1, &reader.kernel, &mut coefs);
            let v2 = self.interp(q1 + s, &reader.kernel, &mut coefs);
            self.push_pair(v1, v2, m);
        }
    }

    /// Push one output step's two oversampled values into the phase rings.
    #[inline]
    pub(super) fn push_pair(&mut self, a: [f32; 2], b: [f32; 2], m: usize) {
        for c in 0..2 {
            self.u1[c][self.p1] = a[c];
            self.u1[c][self.p1 + m] = a[c];
            self.u2[c][self.p2] = b[c];
            self.u2[c][self.p2 + 2 * m] = b[c];
        }
        self.p1 = (self.p1 + 1) % m;
        self.p2 = (self.p2 + 1) % (2 * m);
    }

    /// Release a sustain loop: the lap being read finishes and the tail follows.
    pub fn release_loop(&mut self) {
        let r = &self.region;
        if !r.looping || r.dir != LoopDir::Forward || self.release_lap != i64::MAX {
            return;
        }
        let i = self.furthest;
        let lap = if i <= r.le {
            0
        } else {
            (i - r.le - 1) / r.loop_len() + 1
        };
        // If the reader is already inside this lap's crossfade, finish one more lap.
        let base = i - lap * r.loop_len();
        let in_xf = r.xf > 0 && base > r.le - r.xf;
        self.release_lap = lap + i64::from(in_xf);
    }

    /// Whether a sustain loop has been let go (or there is none).
    pub fn released(&self) -> bool {
        self.release_lap != i64::MAX
    }
}

//! The processor's audio input, monitoring, recording and audio-clip playback (ADR 0025).
//!
//! A child module of `processor`, so it reaches the processor's own fields; everything
//! here runs on the audio thread and follows ADR 0005 — every buffer is allocated in
//! [`Live::new`], before the processor moves to the audio thread.
//!
//! - **Input** ([`Processor::read_input`]): at the top of each block one block of the
//!   input ring is popped into `Live::input`, deinterleaved, and metered. Input and output
//!   may run on two clocks, so the ring is kept near one block of fill: more than that
//!   plus a block and the surplus is dropped (a frame per block while it is small, all of
//!   it when it is more than a block); less than a block and what is there is used and
//!   the last frame held for the rest; nothing at all and the block is silence and the
//!   ring primes again. Both are counted (`EngineStats::{input_dropped, input_repeated}`).
//! - **Monitoring** ([`Live::add_track_audio`]): a monitored track's input × trim joins
//!   its signal where the instrument's output is, before the channel's level and balance,
//!   so its strip's inserts apply. Off never; On always; Auto while armed and (stopped,
//!   or recording).
//! - **Recording** ([`Processor::record_segment`]): while the transport plays with
//!   recording on and rings are attached, each armed track's input × trim — what is heard,
//!   before any effect — goes into its ring, whole frames only. A full ring drops the
//!   segment for that track and reports it; it never waits.
//! - **Audio clips** ([`add_clips`]): integer-sample playback from memory, no
//!   interpolation (the cache converted the file to the engine's rate on load), so a clip
//!   at 0 dB with no fades plays its file bit for bit.

use super::Processor;
use crate::messages::{EngineEvent, MAX_BLOCK_FRAMES, MAX_TRACKS};
use crate::recorder::{RecordMark, MAX_INPUTS};
use crate::transport::Transport;
use cantus_core::{AudioClipSnapshot, ClickMode, MonitorMode, Snapshot, TrackId};

/// A track's input as the audio thread reads it, by track slot.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct TrackInputSlot {
    /// The track has an input at all.
    pub has: bool,
    pub first: usize,
    pub stereo: bool,
    pub monitor: MonitorMode,
    pub trim: f32,
    pub armed: bool,
}

/// The block's input, deinterleaved — what monitoring tracks read, on whichever thread
/// runs them (it lives in a cell of the graph, `graph/exec.rs`).
pub(crate) struct InputBlock {
    /// Channel `c` is `input[c * MAX_BLOCK_FRAMES..]`.
    input: Vec<f32>,
    /// Channels in `input` this block (0: no input device).
    channels: usize,
}

impl InputBlock {
    pub fn new() -> Self {
        InputBlock {
            input: vec![0.0; MAX_BLOCK_FRAMES * MAX_INPUTS],
            channels: 0,
        }
    }

    pub fn prefault(&mut self) {
        super::exec::touch(&mut self.input);
    }

    /// Input channel `c`, frames `[from, from + n)` of this block; `None` past the
    /// device's channels.
    #[inline]
    fn channel(&self, c: usize, from: usize, n: usize) -> Option<&[f32]> {
        (c < self.channels).then(|| &self.input[c * MAX_BLOCK_FRAMES + from..][..n])
    }
}

/// Input, monitoring and recording state, preallocated.
pub(super) struct Live {
    /// The ring has held a whole block since it last ran dry.
    primed: bool,
    /// The last frame popped, held when the ring runs short.
    last: [f32; MAX_INPUTS],
    /// `EngineStats::input_queue` has had its first reading.
    queue_seeded: bool,
    /// Each track slot's input.
    pub tracks: [TrackInputSlot; MAX_TRACKS],
    /// A pass is being written (the `Started` mark has gone out).
    active: bool,
    /// Frames written since the rings were attached.
    frames: u64,
    /// How far the heard latency moved since the pass started, not yet taken into the
    /// file (review 2): positive, frames still to drop; negative, silence still to write.
    /// The take is placed by the latency at its start; what is played after the backing is
    /// heard later (or sooner) moves in the file by as much, so it lands where it was
    /// played.
    pub shift: i64,
    /// Moves not due yet (review 3): the input of the next round trip was played against
    /// the backing as it was. (Input frame it is due at, by how much), oldest first.
    due: [(u64, i64); DUE],
    due_n: usize,
    /// Input frames the pass has taken, written or dropped.
    taken: u64,
    /// The moves taken into the file so far (frames dropped less silence written): a
    /// wrap carries it (review 3: later loop passes were placed by the start's latency).
    applied: i64,
    /// The devices' round trip, in frames (`EngineMessage::SetDeviceRoundTrip`).
    pub round_trip: u32,
    /// Frames each ring lost since the last `Overflow` mark, and whose ring it is.
    dropped: [(Option<TrackId>, u64); MAX_TRACKS],
    /// The count-in's end, in ticks (`EngineMessage::SetRecordStart`).
    pub record_start: Option<f64>,
    /// The click's mode, from the snapshot.
    pub click: ClickMode,
}

impl Live {
    pub fn new() -> Self {
        Live {
            primed: false,
            last: [0.0; MAX_INPUTS],
            queue_seeded: false,
            tracks: [TrackInputSlot::default(); MAX_TRACKS],
            active: false,
            frames: 0,
            shift: 0,
            due: [(0, 0); DUE],
            due_n: 0,
            taken: 0,
            applied: 0,
            round_trip: 0,
            dropped: [(None, 0); MAX_TRACKS],
            record_start: None,
            click: ClickMode::Always,
        }
    }

    /// New rings: a new pass counts its frames from zero.
    pub fn new_pass(&mut self) {
        self.frames = 0;
        self.active = false;
        self.dropped = [(None, 0); MAX_TRACKS];
    }
}

/// Heard-latency moves a pass keeps waiting at once; more are folded into the last.
const DUE: usize = 8;

impl Live {
    /// The heard latency moved by `by` frames: due once the input the musician played
    /// after hearing it arrives — `wait` frames from now.
    fn schedule(&mut self, by: i64, wait: u64) {
        let at = self.taken + wait;
        if self.due_n < DUE {
            self.due[self.due_n] = (at, by);
            self.due_n += 1;
        } else {
            self.due[DUE - 1].1 += by;
        }
    }

    /// Take the moves due by input frame `self.taken` into `shift`.
    fn take_due(&mut self) {
        while self.due_n > 0 && self.due[0].0 <= self.taken {
            self.shift += self.due[0].1;
            self.due.copy_within(1..self.due_n, 0);
            self.due_n -= 1;
        }
    }

    /// Input frames from now to the next move that is not due yet (`room` at most).
    fn until_due(&self, room: u64) -> u64 {
        if self.due_n > 0 {
            self.due[0].0.saturating_sub(self.taken).min(room)
        } else {
            room
        }
    }
}

/// Add a track's audio clips and its monitored input (`t`, read from `input`) to its
/// signal `l`/`r` for the segment `[done, done + n)`. `has_signal` says whether the
/// instrument wrote `l`/`r` (otherwise they hold another track's leftovers and are cleared
/// before anything is added). Returns whether the track now has a signal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_track_audio(
    input: Option<&InputBlock>,
    t: &TrackInputSlot,
    track: Option<&cantus_core::TrackSnapshot>,
    mut has_signal: bool,
    done: usize,
    l: &mut [f32],
    r: &mut [f32],
    transport: &Transport,
    recording: bool,
) -> bool {
    let n = l.len();
    let clear = |has: &mut bool, l: &mut [f32], r: &mut [f32]| {
        if !*has {
            l.iter_mut().for_each(|x| *x = 0.0);
            r.iter_mut().for_each(|x| *x = 0.0);
            *has = true;
        }
    };
    if let Some(t) = track {
        if transport.is_playing() && t.audio.iter().any(|c| c.data.is_some()) {
            let mut zeroed = has_signal;
            if add_clips(&t.audio, transport, l, r, &mut |l, r| {
                clear(&mut zeroed, l, r)
            }) {
                has_signal = true;
            }
        }
    }
    let Some(input) = input.filter(|i| t.has && i.channels > 0) else {
        return has_signal;
    };
    let playing = transport.is_playing();
    let monitoring = match t.monitor {
        MonitorMode::Off => false,
        MonitorMode::On => true,
        MonitorMode::Auto => t.armed && (!playing || recording),
    };
    if !monitoring {
        return has_signal;
    }
    let left = input.channel(t.first, done, n);
    let right = if t.stereo {
        input.channel(t.first + 1, done, n)
    } else {
        left
    };
    if left.is_none() && right.is_none() {
        return has_signal;
    }
    // A monitoring track is never idle, even over digital silence: its strip's
    // inserts and its meters follow the input.
    clear(&mut has_signal, l, r);
    let g = t.trim;
    if let Some(a) = left {
        for (o, x) in l.iter_mut().zip(a) {
            *o += x * g;
        }
    }
    if let Some(b) = right {
        for (o, x) in r.iter_mut().zip(b) {
            *o += x * g;
        }
    }
    has_signal
}

/// The transport sample (counted from its origin) a timeline tick falls on, to the
/// nearest sample — through the tempo map (ADR 0060), the constant arithmetic where the
/// tempo holds.
#[inline]
fn sample_of(tick: f64, transport: &Transport) -> i64 {
    transport.sample_of_f64(tick).round() as i64
}

/// Add every audio clip sounding in this segment into `l`/`r` (`l.len()` frames from the
/// transport's current sample). `clear` is called once, before the first sample is
/// added, to clear buffers that hold nothing of this track yet. Returns whether a clip
/// sounded.
///
/// Frame arithmetic, so it is the same for every block size: a clip starts at the
/// transport sample nearest its start tick, the file frame under transport sample `s` is
/// `s − zero`, where `zero` is the sample nearest the window's content origin
/// (`start − offset`) less the region's fine offset (`slip`), and the
/// fades are counted in frames from the clip's edges — a fade in is exactly 0 on the
/// clip's first frame and 1 from its length on; a fade out mirrors it onto the last
/// frame. Fades that would overlap are shortened in proportion.
pub(super) fn add_clips(
    clips: &[AudioClipSnapshot],
    transport: &Transport,
    l: &mut [f32],
    r: &mut [f32],
    clear: &mut dyn FnMut(&mut [f32], &mut [f32]),
) -> bool {
    let n = l.len() as i64;
    if n == 0 {
        return false;
    }
    let s0 = transport.samples() as i64;
    let s1 = s0 + n;
    let mut sounded = false;
    for c in clips {
        let start = sample_of(c.start.0 as f64, transport);
        if start >= s1 {
            break; // Sorted by start: nothing later begins in this segment.
        }
        let end = sample_of(c.end.0 as f64, transport);
        let len = end - start;
        if end <= s0 || len <= 0 {
            continue;
        }
        let Some(data) = c.data.as_deref() else {
            continue;
        };
        let stereo = data.channels >= 2;
        let frames = data.len_frames() as i64;
        // The transport sample the file's frame 0 lands on, rounded once: the window's
        // content origin (`start − offset`, whole ticks) less the region's fine offset.
        // One rounding keeps the halves of a split seamless, and the fine offset puts a
        // take where it was played to the sample (ADR 0025 §7).
        // Unwarped (ADR 0060 §4, `cantus_core::audio_time`): natural speed from the content
        // origin through the tempo map (a tempo edit re-anchors the window, `tempo::keep`,
        // so what plays at its start never moves).
        // A warp's render starts on the window's first sample, its own frame 0 (round 29's
        // second review: never by cancelling the formula above).
        let zero = if c.zero_at_start {
            start
        } else {
            transport.clip_zero(c.start, c.offset, c.slip)
        };
        // The file frame under the window's first sample.
        let offset = start - zero;
        // What the piece played before this snapshot swapped its data, laid as it was: it
        // fades out over the first samples the snapshot plays while the new fades in.
        let swap = c.fade_from.as_ref().map(|f| {
            let z = if f.zero_at_start {
                start
            } else {
                transport.clip_zero(c.start, c.offset, f.slip)
            };
            (f.data.as_ref(), start - z)
        });
        let (s_tick, e_tick) = (c.start.0 as f64, c.end.0 as f64);
        let mut fade_in = transport
            .samples_between(s_tick, s_tick + c.fade_in.0.max(0) as f64)
            .round() as i64;
        let mut fade_out = transport
            .samples_between(e_tick - c.fade_out.0.max(0) as f64, e_tick)
            .round() as i64;
        if fade_in + fade_out > len {
            let fi = (fade_in as i128 * len as i128 / (fade_in + fade_out) as i128) as i64;
            fade_out = len - fi;
            fade_in = fi;
        }
        let from = start.max(s0);
        let to = end.min(s1);
        // The first and last file frames the window reads in this segment.
        let first = if c.reverse {
            offset + (len - 1 - (from - start))
        } else {
            offset + (from - start)
        };
        let last = if c.reverse {
            offset + (len - 1 - (to - 1 - start))
        } else {
            offset + (to - 1 - start)
        };
        if first.max(last) < 0 || first.min(last) >= frames {
            continue; // The window shows no part of the file here.
        }
        if !sounded {
            clear(l, r);
            sounded = true;
        }
        let gain = c.gain;
        let faded = fade_in > 0 || fade_out > 0;
        // A comp seam (ADR 0058): the stretch crossed over, in samples, rounded from its
        // ticks the same way for both windows, so one fades out over exactly the samples
        // the other fades in on — equal power at every one of them.
        let stretch = |x: Option<(cantus_core::Ticks, cantus_core::Ticks)>| {
            x.map(|(a, b)| {
                let (a, b) = (
                    sample_of(a.0 as f64, transport),
                    sample_of(b.0 as f64, transport),
                );
                (a, (b - a).max(1))
            })
        };
        let (xin, xout) = (stretch(c.xin), stretch(c.xout));
        for s in from..to {
            let j = s - start;
            let src = if c.reverse {
                offset + (len - 1 - j)
            } else {
                offset + j
            };
            if src < 0 || src >= frames {
                continue;
            }
            let (mut a, mut b) = if stereo {
                let k = src as usize * 2;
                (data.frames[k], data.frames[k + 1])
            } else {
                let x = data.frames[src as usize];
                (x, x)
            };
            if let Some((old, off)) = swap {
                if let Some(u) = transport.swap_fade(s) {
                    let at = if c.reverse {
                        off + (len - 1 - j)
                    } else {
                        off + j
                    };
                    let (oa, ob) = frame_of(old, at);
                    let q = std::f32::consts::FRAC_PI_2 * u;
                    let (gi, go) = (cantus_dsp::math::sin(q), cantus_dsp::math::cos(q));
                    a = a * gi + oa * go;
                    b = b * gi + ob * go;
                }
            }
            let mut g = gain;
            if let Some((a, n)) = xin {
                if s < a + n {
                    let u = ((s - a).max(0) as f32 + 0.5) / n as f32;
                    g *= cantus_dsp::math::sin(std::f32::consts::FRAC_PI_2 * u);
                }
            }
            if let Some((a, n)) = xout {
                if s >= a {
                    let u = ((s - a) as f32 + 0.5) / n as f32;
                    g *= cantus_dsp::math::cos(std::f32::consts::FRAC_PI_2 * u.min(1.0));
                }
            }
            if faded {
                if j < fade_in {
                    g *= c.fade_in_curve.gain(j as f32 / fade_in as f32);
                }
                let k = len - 1 - j;
                if k < fade_out {
                    g *= c.fade_out_curve.gain(k as f32 / fade_out as f32);
                }
            }
            let i = (s - s0) as usize;
            l[i] += a * g;
            r[i] += b * g;
        }
    }
    sounded
}

/// The frame `at` of `d` as a pair (silence outside it).
#[inline]
fn frame_of(d: &cantus_core::SampleData, at: i64) -> (f32, f32) {
    let frames = d.len_frames() as i64;
    if at < 0 || at >= frames {
        return (0.0, 0.0);
    }
    if d.channels >= 2 {
        let k = at as usize * 2;
        (d.frames[k], d.frames[k + 1])
    } else {
        let x = d.frames[at as usize];
        (x, x)
    }
}

impl Processor {
    /// Copy what the snapshot says about inputs and the click into the audio thread's
    /// tables. After `track_to_slot` is built; allocates nothing.
    pub(super) fn live_install(&mut self, snapshot: &Snapshot) {
        self.live.tracks = [TrackInputSlot::default(); MAX_TRACKS];
        for (ti, t) in snapshot
            .tracks
            .iter()
            .enumerate()
            .take(self.track_to_slot.len())
        {
            let slot = self.track_to_slot[ti];
            if let Some(i) = &t.input {
                self.live.tracks[slot] = TrackInputSlot {
                    has: true,
                    first: i.first as usize,
                    stereo: i.stereo,
                    monitor: i.monitor,
                    // 0 dB is exactly 1.0: a trim at its default touches no sample.
                    trim: if i.trim_db == 0.0 {
                        1.0
                    } else {
                        cantus_dsp::math::powf(10f32, i.trim_db / 20.0)
                    },
                    armed: t.armed,
                };
            } else if t.armed {
                // Armed with no audio input: its notes are monitored (low latency, ADR
                // 0064 §4).
                self.live.tracks[slot].armed = true;
            }
        }
        // And each track slot's own copy, which its node reads on whichever thread runs it.
        let nodes = self.nodes.clone();
        for slot in 0..MAX_TRACKS {
            if self.track_ids[slot].is_none() && !self.live.tracks[slot].has {
                continue;
            }
            if let Some(mut s) = nodes.track(slot) {
                s.input = self.live.tracks[slot];
            }
        }
        self.live.click = snapshot.recording.click;
        // The click's voice and accent (schema 26, ADR 0052); the classic accented one is
        // what every older project played.
        self.click.set_voice(match snapshot.recording.click_sound {
            cantus_core::ClickSound::Classic => cantus_dsp::click::ClickVoice::Classic,
            cantus_core::ClickSound::Wood => cantus_dsp::click::ClickVoice::Wood,
            cantus_core::ClickSound::Beep => cantus_dsp::click::ClickVoice::Beep,
        });
        self.click.set_accent(snapshot.recording.click_accent);
        // The click's built-in level is 0.5; 0 dB keeps it exactly (10^0 is 1).
        let db = snapshot.recording.click_db;
        self.click.set_gain(
            0.5 * if db == 0.0 {
                1.0
            } else {
                cantus_dsp::math::powf(10f32, db / 20.0)
            },
        );
    }

    /// Whether a beat of the metronome sounds now (`ClickMode`). `Always` is what the
    /// metronome switch always meant.
    pub(super) fn click_allowed(&self) -> bool {
        self.click_allowed_at(self.transport.samples())
    }

    /// Whether the click runs at all: the metronome switch, or a count-in — which clicks
    /// whatever the switch says (round 22, the soak builder's E6).
    pub(super) fn click_on(&self) -> bool {
        self.metronome || self.live.record_start.is_some()
    }

    /// Whether the transport sample `samples` is inside the count-in (before the
    /// recording's start).
    fn counting_in_at(&self, samples: u64) -> bool {
        self.live
            .record_start
            .is_some_and(|at| self.transport.position_at(samples) < at - 1e-9)
    }

    /// [`Self::click_allowed`] for a beat at the transport's sample `samples` (inside the
    /// current pass): the switch's mode, or the count-in.
    pub(super) fn click_allowed_at(&self, samples: u64) -> bool {
        if self.counting_in_at(samples) {
            return true;
        }
        if !self.metronome {
            return false;
        }
        match self.live.click {
            ClickMode::Always => true,
            ClickMode::Recording => self.recording,
            ClickMode::CountInOnly => self
                .live
                .record_start
                .is_some_and(|at| self.transport.position_at(samples) < at - 1e-9),
        }
    }

    /// Pop one block of input (see the module's docs), deinterleave it and meter it.
    pub(super) fn read_input(&mut self, frames: usize) {
        let stride = self.input.channels;
        let channels = stride.min(MAX_INPUTS);
        let live = &mut self.live;
        let Some(mut block) = self.nodes.input.try_borrow_mut() else {
            self.nodes.conflict();
            return;
        };
        let Some(ring) = self.input.ring.as_mut().filter(|_| stride > 0) else {
            block.channels = 0;
            self.meters.input_count = 0;
            self.stats.input_queue = 0.0;
            return;
        };
        block.channels = channels;
        let mut avail = ring.slots() / stride;
        if !live.primed && avail >= frames {
            live.primed = true;
        }
        let mut take = 0;
        if live.primed {
            // Too much waiting: the input's clock is ahead. A little is shed a frame a
            // block (inaudible); a backlog of more than a block goes at once.
            let target = frames;
            if avail > target + frames {
                let surplus = avail - target - frames;
                let shed = if surplus > frames { surplus } else { 1 };
                if let Ok(chunk) = ring.read_chunk(shed * stride) {
                    chunk.commit_all();
                    avail -= shed;
                    self.stats.input_dropped += shed as u64;
                }
            }
            take = avail.min(frames);
            if take > 0 {
                // How long what is popped now has waited: part of a take's latency.
                let clock = self
                    .input
                    .clock
                    .as_ref()
                    .and_then(|c| c.last_buffer(self.sample_rate));
                let q = crate::devices::queue_wait(avail as f64, frames as f64, clock);
                let s = &mut self.stats.input_queue;
                *s = if live.queue_seeded {
                    *s + (q - *s) * 0.05
                } else {
                    q
                };
                live.queue_seeded = true;
            }
            if let Ok(chunk) = ring.read_chunk(take * stride) {
                let (a, b) = chunk.as_slices();
                for (i, frame) in a
                    .chunks_exact(stride)
                    .chain(b.chunks_exact(stride))
                    .enumerate()
                {
                    for (c, x) in frame.iter().take(channels).enumerate() {
                        block.input[c * MAX_BLOCK_FRAMES + i] = *x;
                    }
                }
                chunk.commit_all();
            } else {
                take = 0;
            }
            if take == 0 {
                // Dry: silence, and wait for a whole block again.
                live.primed = false;
                live.last = [0.0; MAX_INPUTS];
            } else {
                for c in 0..channels {
                    live.last[c] = block.input[c * MAX_BLOCK_FRAMES + take - 1];
                }
            }
            if take < frames {
                self.stats.input_repeated += (frames - take) as u64;
            }
        }
        for c in 0..channels {
            let held = live.last[c];
            let ch = &mut block.input[c * MAX_BLOCK_FRAMES..c * MAX_BLOCK_FRAMES + frames];
            ch[take..].iter_mut().for_each(|x| *x = held);
            self.meters.inputs[c] = ch.iter().fold(0f32, |m, x| m.max(x.abs()));
        }
        self.meters.input_count = channels;
    }

    fn should_record(&self) -> bool {
        self.recording && self.transport.is_playing() && self.record_rings.is_some()
    }

    fn mark(&mut self, mark: RecordMark) {
        if self.tx.push(EngineEvent::Record(mark)).is_err() {
            self.stats.dropped_events += 1;
        }
    }

    /// At the top of a block, after the messages: report what the rings lost, and end
    /// the pass if the transport stopped, recording went off or the rings were taken.
    pub(super) fn record_block_start(&mut self) {
        for k in 0..MAX_TRACKS {
            if let (Some(track), frames) = self.live.dropped[k] {
                if frames > 0 {
                    self.live.dropped[k].1 = 0;
                    self.mark(RecordMark::Overflow { track, frames });
                }
            }
        }
        if self.live.active && !self.should_record() {
            self.flush_record_gaps();
            self.live.active = false;
            let frames = self.live.frames;
            self.mark(RecordMark::Stopped { frames });
        }
    }

    /// Write the segment `[done, done + n)` of every armed track's input into its ring —
    /// moved, where the heard latency moved, by as much (review 2), from the frame played
    /// once the move was heard (review 3).
    pub(super) fn record_segment(&mut self, done: usize, n: usize) {
        if !self.should_record() {
            return;
        }
        if !self.live.active {
            self.live.active = true;
            self.live.shift = 0;
            self.live.due_n = 0;
            self.live.taken = 0;
            self.live.applied = 0;
            let tick = self.transport.position_f64();
            let plugin = self.pdc.total.min(u32::MAX as usize) as u32;
            let queue = self.stats.input_queue.round() as i64;
            self.mark(RecordMark::Started {
                tick,
                plugin,
                queue,
            });
        }
        let mut a = 0usize;
        while a < n {
            // The heard latency moved while recording: playing from now on is moved by as
            // much — dropped, or silence written before it.
            self.live.take_due();
            if self.live.shift < 0 {
                let pad = self.live.shift.unsigned_abs();
                self.live.shift = 0;
                if let Some(rings) = self.record_rings.as_mut() {
                    for ring in rings.tracks.iter_mut() {
                        // The writer pads a gap with silence (the file keeps its timing).
                        ring.pending += pad;
                    }
                }
                self.live.frames += pad;
                self.live.applied -= pad as i64;
            }
            let until = self.live.until_due((n - a) as u64) as usize;
            let skip = if self.live.shift > 0 {
                (self.live.shift as usize).min(until)
            } else {
                0
            };
            self.live.shift -= skip as i64;
            self.live.applied += skip as i64;
            self.live.taken += skip as u64;
            let w = until - skip;
            if w > 0 {
                self.record_input(done + a + skip, w);
                self.live.taken += w as u64;
            }
            a += until;
        }
    }

    /// Write `[done, done + n)` of every armed track's input into its ring.
    fn record_input(&mut self, done: usize, n: usize) {
        let Some(rings) = self.record_rings.as_mut() else {
            return;
        };
        let nodes = self.nodes.clone();
        let Some(input) = nodes.read(&nodes.input) else {
            return;
        };
        for (k, ring) in rings.tracks.iter_mut().enumerate().take(MAX_TRACKS) {
            let ch = ring.channels.clamp(1, 2);
            let slot = self.track_ids.iter().position(|s| *s == Some(ring.track));
            let t = slot.map(|s| self.live.tracks[s]).unwrap_or_default();
            // A ring without room, or a gap the writer cannot be told about yet: lose
            // this segment for this track, say so, never wait. The writer pads the gap
            // with silence, so the file keeps its length and its timing.
            if ring.producer.slots() < n * ch || !ring.flush_gap() {
                ring.pending += n as u64;
                self.live.dropped[k] = (Some(ring.track), self.live.dropped[k].1 + n as u64);
                self.stats.record_overflow += n as u64;
                continue;
            }
            let Ok(mut chunk) = ring.producer.write_chunk(n * ch) else {
                continue;
            };
            ring.at += n as u64;
            let (a, b) = chunk.as_mut_slices();
            let left = input.channel(t.first, done, n).filter(|_| t.has);
            let right = input.channel(t.first + 1, done, n).filter(|_| t.has);
            let g = t.trim;
            for (i, frame) in a
                .chunks_exact_mut(ch)
                .chain(b.chunks_exact_mut(ch))
                .enumerate()
            {
                frame[0] = left.map_or(0.0, |x| x[i] * g);
                if ch == 2 {
                    frame[1] = right.map_or(0.0, |x| x[i] * g);
                }
            }
            chunk.commit_all();
        }
        self.live.frames += n as u64;
    }

    /// Announce every ring's last gap (the pass is ending, or the rings are handed back),
    /// so the writer pads a take that lost its end too.
    pub(super) fn flush_record_gaps(&mut self) {
        if let Some(rings) = self.record_rings.as_mut() {
            for ring in rings.tracks.iter_mut() {
                let _ = ring.flush_gap();
            }
        }
    }

    /// Whether a pass is being written.
    pub(super) fn live_recording(&self) -> bool {
        self.live.active
    }

    /// The heard latency moved by `by` frames while a pass records: due a round trip
    /// later — the devices' and the input ring's wait (review 3: applied at once, it cut or
    /// misplaced the playing in flight).
    pub(super) fn live_shift(&mut self, by: i64) {
        let wait = self.live.round_trip as u64 + self.stats.input_queue.max(0.0).round() as u64;
        self.live.schedule(by, wait);
    }

    /// The loop wrapped: the pass's takes split here, with the moves taken so far.
    pub(super) fn record_wrapped(&mut self) {
        if self.live.active {
            let frames = self.live.frames;
            let shift = self.live.applied;
            self.mark(RecordMark::Wrapped { frames, shift });
        }
    }
}

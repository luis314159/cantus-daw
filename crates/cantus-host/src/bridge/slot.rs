//! One isolated instance's shared memory (ADR 0063): the lock-step exchange, then one block
//! of work — the audio in and out, the sidechain, the parameter changes, the notes, the
//! transport — and what the plug-in reported back. Fixed size, made when the instance is
//! loaded: nothing here allocates once it exists.
//!
//! Both processes run the same build, so the `#[repr(C)]` layout is the protocol; the
//! handshake ([`super::proto::PROTOCOL`]) refuses a helper from another build.
//!
//! Who touches what, and when, is the exchange's rule: between [`Caller::submit`] and the
//! job's completion the job area belongs to the helper; otherwise to Cantus. A *late* job
//! keeps it on the helper's side until it finishes ([`Caller::finished`]).
//!
//! [`Caller::submit`]: cantus_rt::ipc::Caller::submit
//! [`Caller::finished`]: cantus_rt::ipc::Caller::finished

use crate::backend::{
    NoteEvent, NoteExpression, NoteKind, ParamChange, ParamEvent, ParamOut, TransportInfo,
};
use cantus_rt::ipc::Exchange;
use cantus_rt::shm::{unique_name, SharedMemory};
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

/// Parameter changes one job carries; more are counted as lost (`ParamOut::lost_in`).
pub const MAX_PARAMS: usize = 2048;
/// Notes one job carries (as many as a hosted instrument queues per call).
pub const MAX_NOTES: usize = crate::inst_unit::NOTE_CAPACITY;
/// What the plug-in reports per job, as `ParamOut` holds it.
pub const MAX_OUT: usize = ParamOut::CAPACITY;

/// What a job asks.
pub mod op {
    pub const PROCESS: u32 = 1;
    pub const RESET: u32 = 2;
    pub const STOP: u32 = 3;
}

/// Job flags.
pub mod flag {
    /// A sidechain pair is in the key buffers.
    pub const KEY: u32 = 1;
    /// The transport is valid.
    pub const TRANSPORT: u32 = 2;
    /// Flush denormals for the call (the caller's thread does: `cantus_rt::denormal`).
    pub const FLUSH: u32 = 4;
}

/// What the helper says about a job it ran.
pub mod status {
    pub const OK: u32 = 0;
    /// No audio half is installed (a restart under way): nothing was processed.
    pub const NO_AUDIO: u32 = 1;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct WireTransport {
    pub playing: u32,
    pub recording: u32,
    pub has_loop: u32,
    pub sig_num: u16,
    pub sig_den: u16,
    pub tempo: f64,
    pub beats: f64,
    pub samples: i64,
    pub bar_start: f64,
    pub loop_a: f64,
    pub loop_b: f64,
    /// The tempo map's fields (round 29, ADR 0060): how the tempo moves inside a ramp, the
    /// real song time, the loop in seconds and the bar counted through metre changes.
    pub tempo_inc: f64,
    pub seconds: f64,
    pub loop_sa: f64,
    pub loop_sb: f64,
    pub bar_number: i32,
    pub has_loop_seconds: u32,
    /// VST3's sample clock (review 2): the real samples, rounded once by the engine.
    pub project_samples: i64,
}

impl WireTransport {
    fn of(t: &TransportInfo) -> Self {
        let (has_loop, (loop_a, loop_b)) = match t.loop_beats {
            Some(l) => (1, l),
            None => (0, (0.0, 0.0)),
        };
        let (has_loop_seconds, (loop_sa, loop_sb)) = match t.loop_seconds {
            Some(l) => (1, l),
            None => (0, (0.0, 0.0)),
        };
        WireTransport {
            playing: u32::from(t.playing),
            recording: u32::from(t.recording),
            has_loop,
            sig_num: t.time_sig_num,
            sig_den: t.time_sig_den,
            tempo: t.tempo_bpm,
            beats: t.position_beats,
            samples: t.position_samples,
            bar_start: t.bar_start_beats,
            loop_a,
            loop_b,
            tempo_inc: t.tempo_inc,
            seconds: t.position_seconds,
            loop_sa,
            loop_sb,
            bar_number: t.bar_number,
            has_loop_seconds,
            project_samples: t.project_samples,
        }
    }

    pub fn info(&self) -> TransportInfo {
        TransportInfo {
            playing: self.playing != 0,
            recording: self.recording != 0,
            tempo_bpm: self.tempo,
            position_beats: self.beats,
            position_samples: self.samples,
            bar_start_beats: self.bar_start,
            time_sig_num: self.sig_num,
            time_sig_den: self.sig_den,
            loop_beats: (self.has_loop != 0).then_some((self.loop_a, self.loop_b)),
            tempo_inc: self.tempo_inc,
            position_seconds: self.seconds,
            bar_number: self.bar_number,
            loop_seconds: (self.has_loop_seconds != 0).then_some((self.loop_sa, self.loop_sb)),
            project_samples: self.project_samples,
        }
    }
}

/// A job's description, and what the helper reports about it.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Header {
    pub op: u32,
    pub frames: u32,
    pub flags: u32,
    pub params: u32,
    pub notes: u32,
    /// Written by the helper: events in the out area, those that found no room.
    pub out: u32,
    pub out_dropped: u32,
    pub status: u32,
    /// Written by the helper: the host's changes the plug-in's lists had no room for.
    pub out_lost_in: u64,
    pub transport: WireTransport,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct WireParam {
    pub offset: u32,
    pub id: u32,
    pub value: f64,
}

/// A note as the job carries it: `kind` 0 on, 1 off, 2 choke, 3 expression, 4 MIDI.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct WireNote {
    pub offset: u32,
    pub note_id: i32,
    pub channel: u8,
    pub key: u8,
    pub kind: u8,
    pub expression: u8,
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
    pub _pad: u8,
    pub value: f64,
}

const EXPRESSIONS: [NoteExpression; 7] = [
    NoteExpression::Tuning,
    NoteExpression::Pressure,
    NoteExpression::Brightness,
    NoteExpression::Pan,
    NoteExpression::Volume,
    NoteExpression::Vibrato,
    NoteExpression::Expression,
];

impl WireNote {
    fn of(n: &NoteEvent) -> Self {
        let mut w = WireNote {
            offset: n.offset,
            note_id: n.note_id,
            channel: n.channel,
            key: n.key,
            ..WireNote::default()
        };
        match n.kind {
            NoteKind::On { velocity } => w.value = f64::from(velocity),
            NoteKind::Off { velocity } => {
                w.kind = 1;
                w.value = f64::from(velocity);
            }
            NoteKind::Choke => w.kind = 2,
            NoteKind::Expression { expression, value } => {
                w.kind = 3;
                w.expression = EXPRESSIONS
                    .iter()
                    .position(|e| *e == expression)
                    .unwrap_or(0) as u8;
                w.value = value;
            }
            NoteKind::Midi {
                status,
                data1,
                data2,
            } => {
                w.kind = 4;
                w.status = status;
                w.data1 = data1;
                w.data2 = data2;
            }
        }
        w
    }

    pub fn event(&self) -> NoteEvent {
        let kind = match self.kind {
            1 => NoteKind::Off {
                velocity: self.value as f32,
            },
            2 => NoteKind::Choke,
            3 => NoteKind::Expression {
                expression: EXPRESSIONS[usize::from(self.expression).min(6)],
                value: self.value,
            },
            4 => NoteKind::Midi {
                status: self.status,
                data1: self.data1,
                data2: self.data2,
            },
            _ => NoteKind::On {
                velocity: self.value as f32,
            },
        };
        NoteEvent {
            offset: self.offset,
            note_id: self.note_id,
            channel: self.channel,
            key: self.key,
            kind,
        }
    }
}

/// A `ParamEvent` as the job carries it: `kind` 0 begin, 1 value, 2 end.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct WireEvent {
    pub kind: u32,
    pub id: u32,
    pub value: f64,
}

impl WireEvent {
    pub fn of(e: &ParamEvent) -> Self {
        match *e {
            ParamEvent::Begin(id) => WireEvent {
                kind: 0,
                id,
                value: 0.0,
            },
            ParamEvent::Value(id, value) => WireEvent { kind: 1, id, value },
            ParamEvent::End(id) => WireEvent {
                kind: 2,
                id,
                value: 0.0,
            },
        }
    }

    pub fn event(&self) -> ParamEvent {
        match self.kind {
            0 => ParamEvent::Begin(self.id),
            1 => ParamEvent::Value(self.id, self.value),
            _ => ParamEvent::End(self.id),
        }
    }
}

/// What the helper measures, read by Cantus at any time (relaxed atomics).
#[repr(C)]
#[derive(Debug, Default)]
pub struct Stats {
    /// Nanoseconds the plug-in spent in its jobs, and the frames they covered.
    pub nanos: AtomicU64,
    pub frames: AtomicU64,
    pub jobs: AtomicU64,
}

const EXCHANGE: usize = 0;
const HEADER: usize = 256;
const STATS: usize = 512;
const AUDIO: usize = 640;

/// Byte offsets of the areas for a largest block of `max` frames.
#[derive(Copy, Clone, Debug)]
struct Layout {
    max: usize,
    params: usize,
    notes: usize,
    out: usize,
    len: usize,
}

fn round_up(n: usize) -> usize {
    n.div_ceil(64) * 64
}

impl Layout {
    fn of(max: usize) -> Layout {
        let params = round_up(AUDIO + 4 * max * 4);
        let notes = round_up(params + MAX_PARAMS * size_of::<WireParam>());
        let out = round_up(notes + MAX_NOTES * size_of::<WireNote>());
        let len = round_up(out + MAX_OUT * size_of::<WireEvent>());
        Layout {
            max,
            params,
            notes,
            out,
            len,
        }
    }
}

/// One instance's mapping.
#[derive(Debug)]
pub struct Slot {
    mem: SharedMemory,
    name: String,
    at: Layout,
}

impl Slot {
    /// Makes a fresh region for blocks of up to `max_block` frames (Cantus's side).
    pub fn create(max_block: usize) -> io::Result<Slot> {
        let name = unique_name("cantus-plugin");
        let at = Layout::of(max_block.max(1));
        let mem = SharedMemory::create(&name, at.len)?;
        Ok(Slot { mem, name, at })
    }

    /// Maps the region Cantus made (the helper's side).
    pub fn open(name: &str, max_block: usize) -> io::Result<Slot> {
        let at = Layout::of(max_block.max(1));
        let mem = SharedMemory::open(name, at.len)?;
        Ok(Slot {
            mem,
            name: name.to_string(),
            at,
        })
    }

    /// The helper has mapped it: its name is removed (Unix; review #43).
    pub fn unlink_name(&self) {
        self.mem.unlink_name();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn max_block(&self) -> usize {
        self.at.max
    }

    pub fn exchange(&self) -> *const Exchange {
        // SAFETY: offset 0 of a page-aligned region at least `HEADER` bytes long.
        unsafe { self.mem.as_ptr().add(EXCHANGE) as *const Exchange }
    }

    pub fn stats(&self) -> &Stats {
        // SAFETY: an aligned, zero-initialised `Stats` (atomics) inside the live region.
        unsafe { &*(self.mem.as_ptr().add(STATS) as *const Stats) }
    }

    fn header(&self) -> *mut Header {
        // SAFETY: inside the region; aligned to 256.
        unsafe { self.mem.as_ptr().add(HEADER) as *mut Header }
    }

    /// Channel `c` (0 left, 1 right, 2 key left, 3 key right) of the audio area.
    fn channel(&self, c: usize) -> *mut f32 {
        // SAFETY: four channels of `max` floats from `AUDIO`, inside the region.
        unsafe { (self.mem.as_ptr().add(AUDIO) as *mut f32).add(c * self.at.max) }
    }

    fn area<T>(&self, offset: usize) -> *mut T {
        // SAFETY: each area's offset is inside the region and 64-byte aligned.
        unsafe { self.mem.as_ptr().add(offset) as *mut T }
    }

    /// Cantus's side: writes a job. Returns how many parameter changes found no room.
    ///
    /// # Safety
    /// The job area is Cantus's now (no job in flight), and `left`, `right` and the key
    /// hold at most [`Self::max_block`] frames.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn put_job(
        &self,
        op: u32,
        left: &[f32],
        right: &[f32],
        key: Option<(&[f32], &[f32])>,
        transport: Option<&TransportInfo>,
        params: &[ParamChange],
        notes: &[NoteEvent],
        flush: bool,
    ) -> u64 {
        let frames = left.len().min(right.len()).min(self.at.max);
        // SAFETY: the caller's contract: the area is ours, the lengths fit.
        unsafe {
            std::ptr::copy_nonoverlapping(left.as_ptr(), self.channel(0), frames);
            std::ptr::copy_nonoverlapping(right.as_ptr(), self.channel(1), frames);
            let mut flags = 0;
            if let Some((kl, kr)) = key {
                let n = frames.min(kl.len()).min(kr.len());
                std::ptr::copy_nonoverlapping(kl.as_ptr(), self.channel(2), n);
                std::ptr::copy_nonoverlapping(kr.as_ptr(), self.channel(3), n);
                flags |= flag::KEY;
            }
            let h = &mut *self.header();
            h.transport = match transport {
                Some(t) => {
                    flags |= flag::TRANSPORT;
                    WireTransport::of(t)
                }
                None => WireTransport::default(),
            };
            if flush {
                flags |= flag::FLUSH;
            }
            let n_params = params.len().min(MAX_PARAMS);
            let p = self.area::<WireParam>(self.at.params);
            for (i, c) in params[..n_params].iter().enumerate() {
                p.add(i).write(WireParam {
                    offset: c.offset,
                    id: c.id,
                    value: c.value,
                });
            }
            let n_notes = notes.len().min(MAX_NOTES);
            let w = self.area::<WireNote>(self.at.notes);
            for (i, n) in notes[..n_notes].iter().enumerate() {
                w.add(i).write(WireNote::of(n));
            }
            h.op = op;
            h.frames = frames as u32;
            h.flags = flags;
            h.params = n_params as u32;
            h.notes = n_notes as u32;
            h.out = 0;
            h.out_dropped = 0;
            h.out_lost_in = 0;
            h.status = status::OK;
            (params.len() - n_params + notes.len() - n_notes) as u64
        }
    }

    /// Cantus's side, after a job finished: the processed audio into `left`/`right`, the
    /// plug-in's events into `out`. Returns the helper's status.
    ///
    /// # Safety
    /// The job finished (its area is Cantus's again).
    pub unsafe fn take_result(
        &self,
        left: &mut [f32],
        right: &mut [f32],
        out: &mut ParamOut,
    ) -> u32 {
        // SAFETY: the caller's contract: the job finished, the area is ours.
        unsafe {
            let h = &*self.header();
            if h.status != status::OK {
                return h.status;
            }
            let frames = (h.frames as usize).min(left.len()).min(right.len());
            std::ptr::copy_nonoverlapping(self.channel(0), left.as_mut_ptr(), frames);
            std::ptr::copy_nonoverlapping(self.channel(1), right.as_mut_ptr(), frames);
            let ev = self.area::<WireEvent>(self.at.out);
            for i in 0..(h.out as usize).min(MAX_OUT) {
                out.push((*ev.add(i)).event());
            }
            out.dropped += h.out_dropped as usize;
            out.lost_in += h.out_lost_in;
            status::OK
        }
    }

    /// The helper's side: the job as the plug-in's audio half takes it. `params` and
    /// `notes` are refilled in place (their capacity is kept: no allocation).
    ///
    /// # Safety
    /// A job was published and not yet completed (the area is the helper's).
    pub unsafe fn job(&self, params: &mut Vec<ParamChange>, notes: &mut Vec<NoteEvent>) -> Job<'_> {
        // SAFETY: the caller's contract; the counts were written by Cantus within bounds.
        unsafe {
            let h = &*self.header();
            let frames = (h.frames as usize).min(self.at.max);
            params.clear();
            let p = self.area::<WireParam>(self.at.params);
            for i in 0..(h.params as usize).min(MAX_PARAMS) {
                let w = *p.add(i);
                if params.len() < params.capacity() {
                    params.push(ParamChange {
                        offset: w.offset,
                        id: w.id,
                        value: w.value,
                    });
                }
            }
            notes.clear();
            let w = self.area::<WireNote>(self.at.notes);
            for i in 0..(h.notes as usize).min(MAX_NOTES) {
                if notes.len() < notes.capacity() {
                    notes.push((*w.add(i)).event());
                }
            }
            let key = (h.flags & flag::KEY != 0).then(|| {
                (
                    std::slice::from_raw_parts(self.channel(2) as *const f32, frames),
                    std::slice::from_raw_parts(self.channel(3) as *const f32, frames),
                )
            });
            Job {
                op: h.op,
                flush: h.flags & flag::FLUSH != 0,
                transport: (h.flags & flag::TRANSPORT != 0).then(|| h.transport.info()),
                left: std::slice::from_raw_parts_mut(self.channel(0), frames),
                right: std::slice::from_raw_parts_mut(self.channel(1), frames),
                key,
            }
        }
    }

    /// The helper's side, after the plug-in ran: what it reported, and the status.
    ///
    /// # Safety
    /// As [`Self::job`].
    pub unsafe fn put_result(&self, out: &ParamOut, status: u32) {
        // SAFETY: the caller's contract.
        unsafe {
            let h = &mut *self.header();
            let ev = self.area::<WireEvent>(self.at.out);
            let n = out.events().len().min(MAX_OUT);
            for (i, e) in out.events()[..n].iter().enumerate() {
                ev.add(i).write(WireEvent::of(e));
            }
            h.out = n as u32;
            h.out_dropped = (out.dropped + out.events().len() - n) as u32;
            h.out_lost_in = out.lost_in;
            h.status = status;
        }
    }

    /// The helper measured a job: `nanos` for `frames`.
    pub fn account(&self, nanos: u64, frames: u64) {
        let s = self.stats();
        s.nanos.fetch_add(nanos, Ordering::Relaxed);
        s.frames.fetch_add(frames, Ordering::Relaxed);
        s.jobs.fetch_add(1, Ordering::Relaxed);
    }
}

/// One job, as the helper sees it: buffers inside the shared region.
#[derive(Debug)]
pub struct Job<'a> {
    pub op: u32,
    pub flush: bool,
    pub transport: Option<TransportInfo>,
    pub left: &'a mut [f32],
    pub right: &'a mut [f32],
    pub key: Option<(&'a [f32], &'a [f32])>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_and_its_result_travel_through_the_slot_intact() {
        let host = Slot::create(64).unwrap();
        let child = Slot::open(host.name(), 64).unwrap();
        let left: Vec<f32> = (0..64).map(|i| i as f32 * 0.25).collect();
        let right: Vec<f32> = (0..64).map(|i| -(i as f32)).collect();
        let key = vec![0.5f32; 64];
        let transport = TransportInfo {
            playing: true,
            recording: false,
            tempo_bpm: 123.5,
            tempo_inc: 0.000_25,
            position_beats: 7.25,
            position_samples: 123_456,
            position_seconds: 3.5,
            project_samples: 168_000,
            bar_start_beats: 4.0,
            bar_number: 2,
            time_sig_num: 7,
            time_sig_den: 8,
            loop_beats: Some((1.0, 9.0)),
            loop_seconds: Some((0.5, 4.5)),
        };
        let params = [ParamChange {
            offset: 3,
            id: 9,
            value: 0.1 + 0.2,
        }];
        let notes = [
            NoteEvent {
                offset: 5,
                note_id: 42,
                channel: 3,
                key: 60,
                kind: NoteKind::On { velocity: 0.75 },
            },
            NoteEvent {
                offset: 6,
                note_id: 42,
                channel: 3,
                key: 60,
                kind: NoteKind::Expression {
                    expression: NoteExpression::Pan,
                    value: 0.3,
                },
            },
            NoteEvent {
                offset: 7,
                note_id: -1,
                channel: 0,
                key: 0,
                kind: NoteKind::Midi {
                    status: 0xB0,
                    data1: 64,
                    data2: 127,
                },
            },
        ];
        // SAFETY: no job in flight; one thread plays both sides in turn.
        unsafe {
            host.put_job(
                op::PROCESS,
                &left,
                &right,
                Some((&key, &key)),
                Some(&transport),
                &params,
                &notes,
                true,
            );
            let mut p = Vec::with_capacity(MAX_PARAMS);
            let mut n = Vec::with_capacity(MAX_NOTES);
            let job = child.job(&mut p, &mut n);
            assert_eq!(job.op, op::PROCESS);
            assert!(job.flush);
            assert_eq!(job.transport, Some(transport));
            assert_eq!(job.left, &left[..]);
            assert_eq!(job.key.unwrap().0, &key[..]);
            assert_eq!(p, params);
            assert_eq!(n, notes);
            for x in job.left.iter_mut() {
                *x *= 2.0;
            }
            let mut out = ParamOut::new();
            out.push(ParamEvent::Begin(1));
            out.push(ParamEvent::Value(1, -3.5));
            out.push(ParamEvent::End(1));
            out.lost_in = 2;
            child.put_result(&out, status::OK);
            let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
            let mut back = ParamOut::new();
            assert_eq!(host.take_result(&mut l, &mut r, &mut back), status::OK);
            assert_eq!(l[10].to_bits(), (2.5f32 * 2.0).to_bits());
            assert_eq!(r, right);
            assert_eq!(back.events(), out.events());
            assert_eq!(back.lost_in, 2);
        }
    }
}

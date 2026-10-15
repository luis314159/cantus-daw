//! The audio-thread half of a CLAP plug-in: `start_processing` (lazily, on the first block),
//! `process`, `reset`, `stop_processing`. Everything it touches was allocated on the main
//! thread when it was made; a call allocates, locks and logs nothing (ADR 0005).

use super::events::{InEvents, OutEvents};
use super::host::AudioScope;
use super::instance::{owner, Instance, Ports};
use crate::backend::{Audio, AudioThread, NoteEvent, ParamChange, ParamOut, TransportInfo};
use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::events::{
    clap_event_header, clap_event_transport, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_TRANSPORT,
    CLAP_TRANSPORT_HAS_BEATS_TIMELINE, CLAP_TRANSPORT_HAS_SECONDS_TIMELINE,
    CLAP_TRANSPORT_HAS_TEMPO, CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE,
    CLAP_TRANSPORT_IS_PLAYING, CLAP_TRANSPORT_IS_RECORDING,
};
use clap_sys::fixedpoint::{CLAP_BEATTIME_FACTOR, CLAP_SECTIME_FACTOR};
use clap_sys::process::{clap_process, CLAP_PROCESS_ERROR};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Buffers for one side (inputs or outputs) of the plug-in's audio ports: a channel of
/// `max_block` samples for every channel of every port, the pointer arrays CLAP reads, and
/// the port structs pointing at them. Made once; the pointers never move.
struct Side {
    #[allow(dead_code)] // owns the memory the pointers point into
    channels: Vec<Vec<f32>>,
    /// Per port, the pointers to its channels.
    ptrs: Vec<Vec<*mut f32>>,
    ports: Vec<clap_audio_buffer>,
}

impl Side {
    fn new(ports: &[super::instance::Port], max_block: usize) -> Side {
        let mut channels: Vec<Vec<f32>> = Vec::new();
        let mut ptrs: Vec<Vec<*mut f32>> = Vec::new();
        for p in ports {
            let mut port_ptrs = Vec::new();
            for _ in 0..p.channels.min(64) {
                channels.push(vec![0.0; max_block]);
                let last = channels.last_mut().expect("just pushed");
                port_ptrs.push(last.as_mut_ptr());
            }
            ptrs.push(port_ptrs);
        }
        let ports = ptrs
            .iter_mut()
            .map(|p| clap_audio_buffer {
                data32: p.as_mut_ptr(),
                data64: std::ptr::null_mut(),
                channel_count: p.len() as u32,
                latency: 0,
                constant_mask: 0,
            })
            .collect();
        Side {
            channels,
            ptrs,
            ports,
        }
    }

    /// Channel `c` of port `port`, `n` frames.
    fn channel(&mut self, port: usize, c: usize, n: usize) -> &mut [f32] {
        let ptr = self.ptrs[port][c];
        // SAFETY: every pointer points at the start of a `max_block` buffer owned by
        // `channels`, and `n <= max_block`.
        unsafe { std::slice::from_raw_parts_mut(ptr, n) }
    }

    fn channels_of(&self, port: usize) -> usize {
        self.ptrs.get(port).map_or(0, Vec::len)
    }
}

pub struct ClapAudio {
    inst: Arc<Instance>,
    ports: Ports,
    inputs: Side,
    outputs: Side,
    events: Box<InEvents>,
    out_events: Box<OutEvents>,
    transport: clap_event_transport,
    max_block: usize,
    /// Where the piece being run starts in the host's block: its events' offsets are
    /// counted from here.
    base: u32,
    steady_time: i64,
    processing: bool,
    stopped: bool,
}

// The raw pointers point into memory this struct owns; it moves to the audio thread whole.
unsafe impl Send for ClapAudio {}

impl ClapAudio {
    pub(crate) fn new(inst: Arc<Instance>, ports: Ports) -> ClapAudio {
        let max_block = inst.setup.max_block.max(1);
        inst.audio_alive.store(true, Ordering::Release);
        ClapAudio {
            inputs: Side::new(&ports.inputs, max_block),
            outputs: Side::new(&ports.outputs, max_block),
            ports,
            inst,
            events: InEvents::new(),
            out_events: OutEvents::new(),
            // SAFETY: a zeroed transport event is valid plain data; it is filled before use.
            transport: unsafe { std::mem::zeroed() },
            max_block,
            base: 0,
            steady_time: 0,
            processing: false,
            stopped: false,
        }
    }

    /// Take the processing side for the audio thread and start processing. `false` when
    /// the main thread is flushing parameters this very moment (then this block is a
    /// wire, and the next one tries again) or the plug-in refused.
    fn start(&mut self) -> bool {
        if self.processing {
            return true;
        }
        if !self.inst.active.load(Ordering::Acquire) {
            return false;
        }
        if self
            .inst
            .owner
            .compare_exchange(
                owner::NONE,
                owner::AUDIO,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false;
        }
        let plugin = self.inst.plugin();
        // SAFETY: audio thread (the scope says so to the plug-in), active, not processing.
        let ok = unsafe { (*plugin).start_processing.is_none_or(|f| f(plugin)) };
        if !ok {
            self.inst.owner.store(owner::NONE, Ordering::Release);
            return false;
        }
        self.processing = true;
        true
    }

    fn fill_transport(&mut self, t: &TransportInfo) {
        let beats = |b: f64| (b * CLAP_BEATTIME_FACTOR as f64).round() as i64;
        let seconds = |s: f64| (s * CLAP_SECTIME_FACTOR as f64).round() as i64;
        let mut flags = CLAP_TRANSPORT_HAS_TEMPO
            | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
            | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE
            | CLAP_TRANSPORT_HAS_TIME_SIGNATURE;
        if t.playing {
            flags |= CLAP_TRANSPORT_IS_PLAYING;
        }
        if t.recording {
            flags |= CLAP_TRANSPORT_IS_RECORDING;
        }
        let (loop_start, loop_end) = t.loop_beats.unwrap_or((0.0, 0.0));
        if t.loop_beats.is_some() {
            flags |= CLAP_TRANSPORT_IS_LOOP_ACTIVE;
        }
        let spb = if t.tempo_bpm > 0.0 {
            60.0 / t.tempo_bpm
        } else {
            0.0
        };
        let beats_per_bar =
            f64::from(t.time_sig_num.max(1)) * 4.0 / f64::from(t.time_sig_den.max(1));
        self.transport = clap_event_transport {
            header: clap_event_header {
                size: size_of::<clap_event_transport>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_TRANSPORT,
                flags: 0,
            },
            flags,
            song_pos_beats: beats(t.position_beats),
            song_pos_seconds: seconds(t.position_beats * spb),
            tempo: t.tempo_bpm,
            tempo_inc: 0.0,
            loop_start_beats: beats(loop_start),
            loop_end_beats: beats(loop_end),
            loop_start_seconds: seconds(loop_start * spb),
            loop_end_seconds: seconds(loop_end * spb),
            bar_start: beats(t.bar_start_beats),
            bar_number: (t.bar_start_beats / beats_per_bar).round() as i32,
            tsig_num: t.time_sig_num,
            tsig_denom: t.time_sig_den,
        };
    }

    /// One call of at most `max_block` frames.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        transport: bool,
        params: &[ParamChange],
        notes: &[NoteEvent],
        out: &mut ParamOut,
    ) {
        let n = left.len().min(right.len()).min(self.max_block);
        // Inputs: the strip into the main port, the key into the sidechain port (silence
        // when nothing is routed), silence anywhere else.
        for port in 0..self.inputs.ports.len() {
            let chans = self.inputs.channels_of(port);
            for c in 0..chans {
                let dst = self.inputs.channel(port, c, n);
                if Some(port) == self.ports.main_in {
                    match (chans, c) {
                        (1, _) => {
                            for (d, (l, r)) in dst.iter_mut().zip(left.iter().zip(right.iter())) {
                                *d = (l + r) * 0.5;
                            }
                        }
                        (_, 0) => dst.copy_from_slice(&left[..n]),
                        (_, 1) => dst.copy_from_slice(&right[..n]),
                        _ => dst.fill(0.0),
                    }
                } else if Some(port) == self.ports.side_in {
                    match (key, chans, c) {
                        (Some((kl, kr)), 1, _) => {
                            for (d, (l, r)) in dst.iter_mut().zip(kl.iter().zip(kr.iter())) {
                                *d = (l + r) * 0.5;
                            }
                        }
                        (Some((kl, _)), _, 0) => dst.copy_from_slice(&kl[..n]),
                        (Some((_, kr)), _, 1) => dst.copy_from_slice(&kr[..n]),
                        _ => dst.fill(0.0),
                    }
                } else {
                    dst.fill(0.0);
                }
            }
        }
        self.events
            .fill(params, notes, n as u32, self.ports.note_dialects, self.base);
        out.lost_in += std::mem::take(&mut self.events.lost);
        self.out_events.bind(out);
        let process = clap_process {
            steady_time: self.steady_time,
            frames_count: n as u32,
            transport: if transport {
                &self.transport
            } else {
                std::ptr::null()
            },
            audio_inputs: self.inputs.ports.as_ptr(),
            audio_outputs: self.outputs.ports.as_mut_ptr(),
            audio_inputs_count: self.inputs.ports.len() as u32,
            audio_outputs_count: self.outputs.ports.len() as u32,
            in_events: self.events.raw(),
            out_events: self.out_events.raw(),
        };
        let plugin = self.inst.plugin();
        // SAFETY: audio thread, between start and stop processing; every pointer in
        // `process` points into memory this struct owns for the whole call.
        let status = unsafe {
            (*plugin)
                .process
                .map_or(CLAP_PROCESS_ERROR, |f| f(plugin, &process))
        };
        self.out_events.unbind();
        self.steady_time += n as i64;
        if status == CLAP_PROCESS_ERROR {
            return; // the plug-in failed this block: the strip passes through untouched
        }
        if let Some(port) = self.ports.main_out {
            let chans = self.outputs.channels_of(port);
            match chans {
                0 => {}
                1 => {
                    let src = self.outputs.channel(port, 0, n);
                    left[..n].copy_from_slice(src);
                    right[..n].copy_from_slice(src);
                }
                _ => {
                    left[..n].copy_from_slice(self.outputs.channel(port, 0, n));
                    right[..n].copy_from_slice(self.outputs.channel(port, 1, n));
                }
            }
        }
    }
}

impl AudioThread for ClapAudio {
    fn process(
        &mut self,
        audio: Audio<'_>,
        params: &[ParamChange],
        notes: &[NoteEvent],
        out: &mut ParamOut,
    ) {
        if self.stopped {
            return;
        }
        let _scope = AudioScope::enter();
        if !self.start() {
            return; // a wire this block
        }
        let transport = match audio.transport {
            Some(t) => {
                self.fill_transport(t);
                true
            }
            None => false,
        };
        let frames = audio.left.len().min(audio.right.len());
        let mut done = 0;
        let mut first = true;
        // A block longer than the plug-in was activated for goes in pieces: each piece
        // hears the parameter changes that fall in it, at their offsets in it (automation
        // is timed, ADR 0057); the notes are all at the first piece's start.
        while done < frames {
            let n = (frames - done).min(self.max_block);
            let key = audio
                .key
                .map(|(l, r)| (&l[done..done + n], &r[done..done + n]));
            let lo = params.partition_point(|p| (p.offset as usize) < done);
            let hi = if done + n >= frames {
                params.len()
            } else {
                params.partition_point(|p| (p.offset as usize) < done + n)
            };
            self.base = done as u32;
            let (p, e): (&[ParamChange], &[NoteEvent]) = if first {
                (&params[lo..hi], notes)
            } else {
                (&params[lo..hi], &[])
            };
            self.run(
                &mut audio.left[done..done + n],
                &mut audio.right[done..done + n],
                key,
                transport && first,
                p,
                e,
                out,
            );
            first = false;
            done += n;
        }
    }

    fn reset(&mut self) {
        if !self.processing {
            return;
        }
        let _scope = AudioScope::enter();
        let plugin = self.inst.plugin();
        // SAFETY: audio thread, active.
        unsafe {
            if let Some(f) = (*plugin).reset {
                f(plugin);
            }
        }
    }

    fn stop(&mut self) {
        self.stopped = true;
        if !self.processing {
            return;
        }
        let _scope = AudioScope::enter();
        let plugin = self.inst.plugin();
        // SAFETY: audio thread, processing.
        unsafe {
            if let Some(f) = (*plugin).stop_processing {
                f(plugin);
            }
        }
        self.processing = false;
        self.inst.owner.store(owner::NONE, Ordering::Release);
    }
}

impl Drop for ClapAudio {
    fn drop(&mut self) {
        // Normally `stop` ran on the audio thread already. If this half is dropped without
        // it (a unit that never reached the engine's chain), stop here: whatever thread
        // this is, it is the only one calling the plug-in's processing side now.
        if self.processing {
            self.stop();
        }
        self.inst.audio_alive.store(false, Ordering::Release);
    }
}

//! MIDI clock and time code out (ADR 0079 §1, §3): stamped on the sample they belong to.
//!
//! - **Clock.** One `F8` on every 24th of a beat the transport crosses, at the sample it
//!   crosses it (`Transport::samples_until`, the arithmetic notes are due by — through the
//!   tempo map and its ramps, or the followed tempo). Starting at the song's start sends
//!   Start (`FA`); anywhere else the song position pointer (`F2`) and then Continue
//!   (`FB`); stopping sends Stop (`FC`); a locate while stopped sends the pointer; a jump
//!   while running (a loop's wrap, a seek) sends Stop, the pointer and Continue — the order
//!   receivers expect, the pointer never while they run.
//! - **Time code.** Quarter frames at the song's time plus the start offset (through the
//!   project's tempo map), a full frame on every start and locate.
//!
//! Every stamp is the sample **heard**: the engine's sample plus the delay compensation it
//! renders ahead by. The out thread adds the device's latency and the port's offset.

use super::*;

/// Ticks in a sixteenth: the song position pointer's unit.
const SIXTEENTH: f64 = PPQ as f64 / 4.0;

/// The first sixteenth at or after `tick` (within a millionth of a tick).
fn next_sixteenth(tick: f64) -> f64 {
    (tick / SIXTEENTH - 1e-6).ceil().max(0.0) * SIXTEENTH
}

/// The pointer naming the sixteenth at `tick`.
fn spp_at(tick: f64) -> [u8; 3] {
    clock::spp((tick / SIXTEENTH).round() / 4.0)
}

impl Processor {
    /// The block's starts, stops and locates.
    pub(super) fn out_block(&mut self, run: &mut SyncRun) {
        let cfg = &run.port.shared.out;
        let (clk, tc) = (
            cfg.clock.load(Ordering::Relaxed),
            cfg.mtc.load(Ordering::Relaxed),
        );
        let playing = self.transport.is_playing();
        let pos = self.transport.position_f64();
        let at = self.clock + self.pdc.total as u64;
        if !(clk || tc) {
            run.out_was_playing = playing;
            run.out_pos = pos;
            run.next_clock = f64::NAN;
            run.qf = None;
            return;
        }
        let rate = mtc::Rate::from_code(cfg.mtc_rate.load(Ordering::Relaxed));
        let off = cfg.offset_s();
        if playing && !run.out_was_playing {
            // From the song's start — or a start before it (a count-in, a follower's start
            // inside the block): Start, the first clock on tick 0. Elsewhere: the pointer of
            // the next sixteenth and Continue, the first clock on that sixteenth, as a
            // receiver counts (review r33 #4).
            let first = if pos < 0.5 { 0.0 } else { next_sixteenth(pos) };
            if clk {
                if first == 0.0 {
                    run.push(Stamped::new(at, OutKind::Clock, &[clock::START]));
                } else {
                    run.push(Stamped::new(at, OutKind::Clock, &spp_at(first)));
                    run.push(Stamped::new(at, OutKind::Clock, &[clock::CONTINUE]));
                }
            }
            self.out_locate(run, at, pos, first);
        } else if !playing && run.out_was_playing {
            if clk {
                run.push(Stamped::new(at, OutKind::Clock, &[clock::STOP]));
            }
            run.qf = None;
        } else if !playing && (pos - run.out_pos).abs() > 0.5 {
            if clk {
                let first = next_sixteenth(pos.max(0.0));
                run.push(Stamped::new(at, OutKind::Clock, &spp_at(first)));
            }
            if tc {
                let secs = self.transport.map().seconds_at(pos) + off;
                let full = mtc::Timecode::at_seconds(secs.max(0.0), rate).full_frame();
                run.push(Stamped::new(at, OutKind::Mtc, &full));
            }
        }
        run.out_was_playing = playing;
        run.out_pos = pos;
    }

    /// Start counting quarter frames from `pos` and clocks from `first` (the tick the first
    /// clock is: a start's 0, a pointer's sixteenth).
    fn out_locate(&mut self, run: &mut SyncRun, at: u64, pos: f64, first: f64) {
        let cfg = &run.port.shared.out;
        run.next_clock = first;
        if cfg.mtc.load(Ordering::Relaxed) {
            let rate = mtc::Rate::from_code(cfg.mtc_rate.load(Ordering::Relaxed));
            let secs = self.transport.map().seconds_at(pos) + cfg.offset_s();
            let full = mtc::Timecode::at_seconds(secs.max(0.0), rate).full_frame();
            run.push(Stamped::new(at, OutKind::Mtc, &full));
            run.qf = Some(mtc::QfWriter::at(secs.max(0.0), rate));
        }
        run.expect = pos;
    }

    /// The pass `[p0, p0 + n)` of a playing transport: its clocks and quarter frames.
    pub(in crate::processor) fn sync_pass(&mut self, p0: usize, n: usize) {
        let Some(mut run) = self.sync.take() else {
            return;
        };
        if self.transport.is_playing() {
            self.out_pass(&mut run, p0, n);
        }
        self.sync = Some(run);
    }

    fn out_pass(&mut self, run: &mut SyncRun, p0: usize, n: usize) {
        let cfg = &run.port.shared.out;
        let (clk, tc) = (
            cfg.clock.load(Ordering::Relaxed),
            cfg.mtc.load(Ordering::Relaxed),
        );
        if !(clk || tc) {
            return;
        }
        let at = self.clock + p0 as u64 + self.pdc.total as u64;
        let pos = self.transport.position_f64();
        if run.next_clock.is_nan() {
            // Turned on while playing: clocks from the next 24th (a receiver started by hand).
            let first = (pos / CLOCK_TICKS - 1e-9).ceil() * CLOCK_TICKS;
            self.out_locate(run, at, pos, first);
        } else if (pos - run.expect).abs() > 0.5 {
            // A jump while running: Stop, the pointer of the next sixteenth, Continue.
            let first = next_sixteenth(pos.max(0.0));
            if clk {
                run.push(Stamped::new(at, OutKind::Clock, &[clock::STOP]));
                run.push(Stamped::new(at, OutKind::Clock, &spp_at(first)));
                run.push(Stamped::new(at, OutKind::Clock, &[clock::CONTINUE]));
            }
            self.out_locate(run, at, pos, first);
        }
        let base = self.clock + p0 as u64 + self.pdc.total as u64;
        if clk {
            loop {
                let k = self.transport.samples_until(run.next_clock);
                if k as usize >= n {
                    break;
                }
                run.push(Stamped::new(base + k, OutKind::Clock, &[clock::CLOCK]));
                run.next_clock += CLOCK_TICKS;
            }
        }
        let cfg = &run.port.shared.out;
        if tc {
            let rate = mtc::Rate::from_code(cfg.mtc_rate.load(Ordering::Relaxed));
            let off = cfg.offset_s();
            if run.qf.is_none() {
                let secs = self.transport.map().seconds_at(pos) + off;
                run.qf = Some(mtc::QfWriter::at(secs.max(0.0), rate));
            }
            for _ in 0..64 {
                let Some(mut w) = run.qf else { break };
                let tick = self.transport.map().tick_at_seconds(w.due(rate) - off);
                let k = self.transport.samples_until(tick);
                if k as usize >= n {
                    break;
                }
                let msg = w.take(rate);
                run.qf = Some(w);
                run.push(Stamped::new(base + k, OutKind::Mtc, &msg));
            }
        }
        run.expect = self
            .transport
            .position_at(self.transport.samples() + n as u64);
    }
}

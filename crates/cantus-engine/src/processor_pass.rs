//! The block: cut into passes, each run by the pool (or the callback alone), then the
//! master on the callback; and after the block, what the nodes left for the callback —
//! their gestures, meters, the profilers' time and the blame.

use super::ctx::Job;
use super::exec::Outgoing;
use super::*;
use crate::profile::Node;
use std::sync::atomic::Ordering::Relaxed;

impl Processor {
    /// The block, as passes of the graph (`crate::graph`): cut only where everything must
    /// stop at once — a loop wrap, an automation control point, every `PASS_FRAMES` —
    /// and in each pass every node runs once at its own segments.
    pub(super) fn render(&mut self, output: &mut [f32], channels: usize, frames: usize) {
        let nodes = self.nodes.clone();
        if let Some(mut tap) = nodes.tap.try_borrow_mut() {
            if let Some(tap) = tap.as_mut() {
                tap.begin(frames);
            }
        }
        // Peaks are the block's: what no pass writes stays silent.
        for &slot in &self.track_to_slot {
            nodes.track_peak[slot].store(0, Relaxed);
        }
        for &slot in &self.strip_to_slot {
            nodes.strip_peak[slot].store(0, Relaxed);
            nodes.strip_gr[slot].store(0, Relaxed);
        }
        let mut master_peak = [0f32; 2];
        let (master_gain, master_width, master_pan, master_invert, law) = self
            .snapshot
            .as_ref()
            .map(|s| {
                (
                    s.master_gain,
                    s.master_width,
                    s.master_pan,
                    s.master_phase_invert,
                    s.pan_law,
                )
            })
            .unwrap_or((1.0, 1.0, 0.0, false, PanLaw::default()));
        // The master's pan is a *balance*, not a pan pot, so it does not take the
        // project's pan law. A pan law compensates for placing a **mono source** in a
        // stereo field; the master carries a mix that is already stereo, and running it
        // through, say, the −6 dB law would take 6 dB off every centred mix for nothing.
        // `balance_gains` is the balance formulation — the `ZeroDb` law's taper on the far
        // channel — and returns *exactly* unity at centre and on the near side. (The taper
        // alone, `cos(π/4)·√2`, is 0.99999994 in f32: a master at its default once touched
        // every sample by one rounding step — see the capture null test. Since
        // `DSP_VERSION` 4 the `ZeroDb` law *is* this balance.)
        let (master_l, master_r) = balance_gains(master_pan);
        let master = MasterStrip {
            gain: master_gain,
            width: master_width,
            l: master_l,
            r: master_r,
            sign: if master_invert { -1.0f32 } else { 1.0 },
        };
        let snap = self.snapshot.clone();

        let mut done = 0usize;
        let mut guard = 0usize;
        while done < frames {
            self.prof.enter(Node::Engine);
            guard += 1;
            if guard > MAX_BLOCK_FRAMES * 2 {
                break; // Impossible unless the loop region is degenerate; never spin.
            }
            // A control point due now runs first; the pass ends at the next one.
            let mut end = self.auto_point(done, frames).min(done + PASS_FRAMES);
            let mut wrap = false;
            if self.transport.is_playing() && snap.is_some() {
                let off = self.transport.samples_until_loop_end();
                // After a wrap, each late insert's own wrap — the sample from which the
                // audio reaching it is the loop's start again — ends a pass too, so it
                // hears the jump there whatever the blocks (review of fix3); only the
                // wraps played, at their real samples (review of fix4: the same `Wraps` as
                // `SongPosition::earlier` folds by), whatever the loop is now (review of
                // fix6: turned off or moved ahead just after a wrap, the audio in flight
                // still wraps).
                if let Some(w) = self.transport.wraps_played() {
                    for &l in &self.lates[..self.late_n] {
                        if let Some(r) = w.before_wrap(l as u64) {
                            if off.is_none_or(|o| r < o) {
                                end = end.min(done + r as usize);
                            }
                        }
                    }
                }
                if let Some(off) = off {
                    let off = off as usize;
                    if done + off <= end {
                        end = done + off;
                        wrap = true;
                    }
                }
            }
            if end <= done {
                // Only possible for a loop boundary that is due right now: what is due at
                // this sample fires first, as it always did, and then the loop wraps.
                if wrap {
                    if let Some(s) = snap.as_deref() {
                        self.fire_due_now(s, done);
                    }
                    self.note_at = self.clock + done as u64;
                    self.wrap_loop();
                }
                continue;
            }
            let n = end - done;
            // A hosted plug-in's automation inside the pass, at its offsets (ADR 0057).
            self.auto_hosted(done, n);
            self.run_pass(
                snap.as_deref(),
                &mut output[done * channels..end * channels],
                channels,
                done,
                n,
                law,
                master,
                &mut master_peak,
            );
            done = end;
            if self.transport.is_playing() {
                self.transport.advance(n as u64);
                self.played = self.played.saturating_add(n as u64);
                if wrap {
                    self.note_at = self.clock + done as u64;
                    self.wrap_loop();
                }
            }
        }
        self.clock += done as u64;
        // Notes for effects that did not run, or are gone, never arrive: what is past is
        // dropped.
        self.prune_notes();

        self.meters.master = master_peak;
        self.meters.master_gr = self.master_gr();
        self.meters.master_loudness = self.master_loud.take();
        self.meters.track_count = self.track_to_slot.len();
        for (ti, slot) in self.track_to_slot.iter().enumerate() {
            self.meters.tracks[ti] = f32::from_bits(nodes.track_peak[*slot].load(Relaxed));
        }
        self.meters.strip_count = self.strip_to_slot.len();
        for (si, slot) in self.strip_to_slot.iter().enumerate() {
            self.meters.strips[si] = f32::from_bits(nodes.strip_peak[*slot].load(Relaxed));
            // The desk's reduction lane: the deepest of the strip's working compressors.
            self.meters.strip_gr[si] = f32::from_bits(nodes.strip_gr[*slot].load(Relaxed));
        }
    }

    /// One pass `[p0, p0 + n)` of the block: every node once — the tracks and strips by
    /// the pool (or the callback alone), then the master. `out` is the pass's stretch of
    /// the device's buffer.
    #[allow(clippy::too_many_arguments)]
    fn run_pass(
        &mut self,
        snap: Option<&Snapshot>,
        out: &mut [f32],
        channels: usize,
        p0: usize,
        n: usize,
        law: PanLaw,
        master: MasterStrip,
        peaks: &mut [f32; 2],
    ) {
        // A latency the pass's automation moved is compensated before the pass runs.
        self.replan_if_latency_moved();
        // Armed tracks' input into their recording rings (ADR 0025).
        self.record_segment(p0, n);
        // Where the song is at the pass's first sample, for the strips' and the master's
        // units that follow it (hosted plug-ins): a pass never crosses a loop wrap.
        let position = self.song_position();
        self.pass += 1;
        if self.map_dirty {
            self.rebuild_map();
        }
        let nodes = self.nodes.clone();
        let tap = nodes.tap.try_borrow_mut().is_some_and(|t| t.is_some());
        match nodes.frame.try_borrow_mut() {
            Some(mut f) => {
                let c = &mut f.ctx;
                c.transport.clone_from(&self.transport);
                c.clock = self.clock;
                c.p0 = p0;
                c.n = n;
                c.position = position;
                c.law = law;
                c.recording = self.recording;
                c.probed = self.probed;
                c.analysed = self.analysed;
                c.scoped = self.scoped;
                c.scoped_delay = self.scoped_delay;
                c.capture_slot = self.capture_slot;
                c.capture_line = self.capture_line;
                c.auto_at = self.auto.base();
                c.pass = self.pass;
                c.block = self.block;
                c.profiling = self.prof.is_on();
                c.tap = tap;
                c.split_voices = self.split_voices;
            }
            None => nodes.conflict(),
        }
        if let Some(mut cap) = nodes.capture.try_borrow_mut() {
            cap.ran = false;
        }
        // ---- the tracks and the strips: the pool's part of the pass ----------------------
        self.prof.enter(Node::Engine);
        self.last_parallel = false;
        // With a stem tap the nodes run on the callback in the plan's order: the tap's
        // direct sum is taken in track order.
        let ran = match self.pool.as_mut().filter(|_| !tap) {
            Some(pool) => {
                let r = pool.run_pass();
                self.last_parallel = !r.serial && !r.too_big;
                self.block_work += r.work_ns;
                !r.too_big
            }
            None => false,
        };
        if !ran {
            self.run_nodes_serially();
        }
        let slot0 = nodes.scratch[0]
            .try_borrow_mut()
            .map_or(0, |mut sc| sc.prof.take_charged());
        if self.last_parallel {
            // The callback's wait for the critical path is nobody's work: the nodes it
            // waited on are charged on the workers that ran them (§2.9).
            self.prof.restart(Node::Engine);
        } else {
            self.prof.enter_except(Node::Engine, slot0);
        }
        // A captured strip that did not run this pass — nothing came in, nothing rings —
        // still records its silence: a take must keep time.
        let captured = nodes.capture.try_borrow_mut().is_some_and(|c| c.ran);
        if matches!(self.capture_target, CaptureTarget::Strip(_)) && !captured {
            self.capture_silence(n);
        }
        // ---- the master: gather → chain → strip → take → click ---------------------------
        self.run_master_node(snap, out, channels, p0, n, &position, master, peaks);
    }

    /// Every node but the master on the callback, in the plan's order: the serial engine
    /// (`W = 0`), and the fallback if the pool cannot take the plan.
    fn run_nodes_serially(&mut self) {
        let nodes = &*self.nodes;
        let Some(frame) = nodes.read(&nodes.frame) else {
            return;
        };
        let Some(mut sc) = nodes.scratch[0].try_borrow_mut() else {
            nodes.conflict();
            return;
        };
        sc.prof.set_on(frame.ctx.profiling);
        let started = Instant::now();
        for (n, job) in frame.map.jobs.iter().enumerate() {
            if *job != Job::Master {
                nodes.run_node(&frame, n, &mut sc);
            }
        }
        self.block_work += started.elapsed().as_nanos() as u64;
    }

    /// The captured strip's silence (through its line, which may still hold what it
    /// played before).
    fn capture_silence(&mut self, n: usize) {
        let nodes = self.nodes.clone();
        let Some(mut cap) = nodes.capture.try_borrow_mut() else {
            nodes.conflict();
            return;
        };
        match self.capture_line {
            Some(li) => {
                self.edge_l[..n].iter_mut().for_each(|x| *x = 0.0);
                self.edge_r[..n].iter_mut().for_each(|x| *x = 0.0);
                if let Some(mut line) = nodes.line(li) {
                    line.run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
                }
                for i in 0..n {
                    cap.frame(self.edge_l[i], self.edge_r[i]);
                }
            }
            None => {
                for _ in 0..n {
                    cap.frame(0.0, 0.0);
                }
            }
        }
    }

    /// At a loop boundary due right now (the playhead sits on the loop's end): whatever is
    /// due at this sample fires before the wrap releases it, as the serial engine did at
    /// the top of every segment.
    fn fire_due_now(&mut self, snap: &Snapshot, done: usize) {
        let nodes = self.nodes.clone();
        for ti in 0..self.track_to_slot.len() {
            let slot = self.track_to_slot[ti];
            let t = &snap.tracks[ti];
            let Some(mut s) = nodes.track(slot) else {
                continue;
            };
            while let Some(ev) = t.events.get(s.next_event) {
                if self.transport.samples_until(ev.tick.0 as f64) != 0 {
                    break;
                }
                fire(&mut s, ev);
                s.next_event += 1;
                if self.listened[slot] {
                    let pushed = nodes.tracks[slot]
                        .outbox
                        .try_borrow_mut()
                        .is_some_and(|mut o| o.push(self.clock + done as u64, fx_note(ev)));
                    if !pushed {
                        self.stats.dropped_events += 1;
                    }
                }
            }
        }
        if self.click_on() && self.transport.samples_until(self.next_beat_tick) == 0 {
            let beat_len = snap.time_signature.ticks_per_beat().0 as f64;
            let bar_len = snap.time_signature.ticks_per_bar().0 as f64;
            let in_bar = self.next_beat_tick.rem_euclid(bar_len);
            if self.click_allowed() {
                self.click.trigger(in_bar.abs() < 0.5);
            }
            self.next_beat_tick += beat_len;
        }
    }

    fn wrap_loop(&mut self) {
        // `Transport::advance` already moved the playhead to loop start when it detected
        // the wrap; if it has not (boundary exactly at segment start), force it.
        if let Some(l) = self.transport.loop_region().cloned() {
            if self.transport.samples_until_loop_end() == Some(0) {
                self.transport.wrap_to(l.start);
            }
        }
        self.release_all();
        self.reseek_events();
        self.record_wrapped();
    }

    /// Forward what the nodes left (gestures, readings), fold the slots' profilers, judge
    /// the block (overrun, blame, the adaptive rule) and fill the stats.
    pub(super) fn after_block(&mut self, started: Instant, frames: usize) {
        let nodes = self.nodes.clone();
        for cell in nodes.scratch.iter() {
            let Some(mut sc) = cell.try_borrow_mut() else {
                nodes.conflict();
                continue;
            };
            for o in sc.events.drain(..) {
                let ev = match o {
                    Outgoing::Inst { track, event } => EngineEvent::InstParam { track, event },
                    Outgoing::Fx { effect, event } => EngineEvent::FxParam { effect, event },
                };
                if self.tx.push(ev).is_err() {
                    self.stats.dropped_events += 1;
                }
            }
            self.prof.fold(&mut sc.prof);
        }
        if let Some(mut p) = nodes.probe.try_borrow_mut() {
            if let Some(t) = p.take() {
                self.meters.inst = t;
            }
        }
        if let Some(mut a) = nodes.analysis.try_borrow_mut() {
            if let Some(d) = a.eq_deltas.take() {
                self.meters.eq_deltas = d;
            }
            if let Some(c) = a.comp.take() {
                self.meters.comp = c;
            }
            if let Some(t) = a.fx.take() {
                self.meters.fx = t;
            }
        }
        if let Some(mut c) = nodes.capture.try_borrow_mut() {
            self.stats.capture_overruns += std::mem::take(&mut c.overruns);
        }
        self.stats.dropped_events += nodes.dropped.swap(0, Relaxed);
        self.stats.stream_late += nodes.stream_late.swap(0, Relaxed);
        self.stats.stream_cut += nodes.stream_cut.swap(0, Relaxed);
        self.stats.stream_wakes += nodes.stream_wakes.swap(0, Relaxed);
        let budget = frames as f64 / self.sample_rate * 1e6;
        if let Some(pool) = self.pool.as_mut() {
            let s = pool.stats();
            self.stats.wakes += s.wakes - self.pool_wakes;
            self.pool_wakes = s.wakes;
            self.stats.workers = s.available;
            pool.adapt(self.block_work, (budget * 1e3) as u64);
        }
        self.stats.parallel = self.last_parallel;
        self.block_work = 0;
        self.measure_costs();

        // Instrumentation.
        let ended = Instant::now();
        let elapsed = ended.saturating_duration_since(started).as_secs_f64() * 1e6;
        let overran = elapsed > budget && budget > 0.0;
        let blamed = if overran && self.last_parallel {
            let path = self.critical_path();
            self.prof.end_among(ended, true, |n| path.holds(n))
        } else {
            self.prof.end(ended, overran)
        };
        if let Some((node, ns)) = blamed {
            self.stats.last_dropout = Some(crate::profile::Dropout {
                culprit: self.culprit(node),
                culprit_us: ns as f32 / 1e3,
                block_us: elapsed as f32,
                budget_us: budget as f32,
                block: self.stats.blocks_processed,
            });
        }
        self.stats.histogram.record(elapsed);
        self.stats.block_frames = frames;
        self.stats.last_block_us = elapsed;
        self.stats.max_block_us = self.stats.max_block_us.max(elapsed);
        self.stats.load = if budget > 0.0 {
            (elapsed / budget) as f32
        } else {
            0.0
        };
        if overran {
            self.stats.overruns += 1;
        }
        self.stats.blocks_processed += 1;
        // The slots a track holds: a freed slot is never rendered again.
        self.stats.active_voices = self
            .track_to_slot
            .iter()
            .map(|s| nodes.track_voices[*s].load(Relaxed) as usize)
            .sum();
        self.stats.schedule_conflicts = nodes.conflicts.load(Relaxed);
        (self.stats.voice_jobs_helped, self.stats.voice_forks) = nodes.forks.counts();
    }
}

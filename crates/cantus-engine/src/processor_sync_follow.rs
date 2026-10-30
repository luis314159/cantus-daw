//! Following: Link (phase within a quantum) and the absolute sources — MIDI clock (beats
//! from the song position pointer) and time code (seconds, through the project's map).

use super::*;

/// The first beat at or after `x` congruent to `target` modulo `q`: `x` plus the least
/// non-negative residue of `target − x` (modular arithmetic; Euclidean remainders).
pub(super) fn next_congruent(x: f64, target: f64, q: f64) -> f64 {
    if q <= 0.0 {
        return x;
    }
    let want = target.rem_euclid(q);
    let have = x.rem_euclid(q);
    x + (want - have).rem_euclid(q)
}

/// The tempo's pull for a phase error (µs): none within the deadband, then proportional.
fn pull_of(err_us: f64) -> f64 {
    if err_us.abs() <= DEADBAND_US {
        return 0.0;
    }
    (-(err_us - DEADBAND_US * err_us.signum()) / (PULL_S * 1e6)).clamp(-PULL_MAX, PULL_MAX)
}

/// `e` folded into `[-q/2, q/2)`: a phase error within the quantum.
pub(super) fn wrap_phase(e: f64, q: f64) -> f64 {
    if q <= 0.0 {
        return e;
    }
    (e + q / 2.0).rem_euclid(q) - q / 2.0
}

impl Processor {
    /// The user's own play and stop while a source is followed: their moment comes from
    /// the source (a play waits for the quantum or the master; Link hears of it).
    fn sync_intent(&mut self, run: &mut SyncRun, tl: &Timeline, heard: i64) {
        let playing = self.transport.is_playing();
        if playing && !run.was_playing {
            // Pressed play: nothing has rendered yet this block, so stopping is unheard.
            self.transport.stop();
            run.queued = true;
            run.hold = false;
            if tl.kind == SourceKind::Link && tl.start_stop {
                let _ = run.port.proposals.push(Proposal::Play { at: heard });
            }
        } else if !playing && run.was_playing && run.locked.is_some() {
            // Pressed stop.
            run.locked = None;
            run.queued = false;
            run.hold = tl.playing;
            if tl.kind == SourceKind::Link && tl.start_stop {
                let _ = run.port.proposals.push(Proposal::Stop { at: heard });
            }
        }
        if !tl.playing {
            run.hold = false;
        }
    }

    pub(super) fn follow_link(
        &mut self,
        run: &mut SyncRun,
        tl: &Timeline,
        heard: i64,
        frames: usize,
        drift: f64,
    ) {
        self.sync_intent(run, tl, heard);
        let q = if tl.quantum > 0.0 { tl.quantum } else { 4.0 };
        let want = if tl.start_stop {
            (tl.playing && !run.hold) || (run.queued && tl.playing)
        } else {
            run.queued || run.locked.is_some()
        };
        let sr = self.sample_rate;
        if run.locked.is_some() && !want {
            self.transport.stop();
            run.locked = None;
            run.port.shared.set_lock(Lock::Idle);
            return;
        }
        if run.locked.is_none() {
            if !want {
                let l = if run.queued { Lock::Queued } else { Lock::Idle };
                run.port.shared.set_lock(l);
                return;
            }
            // Start at the next session beat in the phase of where the playhead is.
            let p = self.transport.position_f64() / PPQ as f64;
            let b = tl.beat_at(heard);
            let at = next_congruent(b, p, q);
            let k = (tl.time_at(at) - heard) as f64 * sr / 1e6;
            if k < frames as f64 {
                let bpm = tl.tempo * drift;
                self.sync_start(p * PPQ as f64, k.max(0.0), bpm);
                run.locked = Some(SourceKind::Link);
                run.offset = p - at;
                run.epoch = tl.epoch;
                run.queued = false;
                run.base_bpm = tl.tempo;
                run.retuned = heard;
                run.port.shared.set_lock(Lock::Locked);
            } else {
                run.port.shared.set_lock(Lock::Queued);
            }
            return;
        }
        // Locked: the phase error, heard against heard.
        let ours = self.transport.position_f64() / PPQ as f64;
        let target = tl.beat_at(heard) + run.offset;
        let err_beats = wrap_phase(ours - target, q);
        let err_us = err_beats * 60e6 / tl.tempo.max(1e-9);
        if err_us.abs() > RELOCK_US {
            // A jump in the session (another joined, a beat forced): the nearest beat in
            // phase, at once.
            let to = ours - err_beats;
            self.sync_jump(to * PPQ as f64);
            run.offset = to - tl.beat_at(heard);
            run.epoch = tl.epoch;
            Self::report(run, err_us);
            return;
        }
        if (ours - target - err_beats).abs() > 1e-6 {
            // The session's numbering moved by whole quanta (a join): keep our place.
            run.offset += ours - target - err_beats;
        }
        let pull = pull_of(err_us);
        let bpm = tl.tempo * drift * (1.0 + pull);
        Self::sync_retune(run, &mut self.transport, tl.tempo, bpm, heard);
        Self::report(run, err_us);
    }

    /// MIDI clock and time code: an absolute place; the master starts and stops.
    pub(super) fn follow_absolute(
        &mut self,
        run: &mut SyncRun,
        tl: &Timeline,
        heard: i64,
        drift: f64,
    ) {
        self.sync_intent(run, tl, heard);
        let want = tl.playing && !run.hold;
        let sr = self.sample_rate;
        // Where the master is, in our ticks, and the tempo that keeps us there.
        let (target, base) = match tl.kind {
            SourceKind::Mtc => {
                let off = run.port.shared.out.offset_s();
                let map = self.transport.map();
                let t = map.tick_at_seconds(tl.seconds_at(heard) - off);
                (t, map.bpm_at(t.max(0.0)) * tl.rate)
            }
            _ => (tl.beat_at(heard) * PPQ as f64, tl.tempo),
        };
        if base <= 0.0 || !base.is_finite() {
            return;
        }
        if run.locked.is_some() && !want {
            self.transport.stop();
            run.locked = None;
            run.queued = false;
            run.port.shared.set_lock(Lock::Idle);
            return;
        }
        if run.locked.is_none() {
            if want {
                self.sync_start(target, 0.0, base * drift);
                run.locked = Some(tl.kind);
                run.epoch = tl.epoch;
                run.queued = false;
                run.base_bpm = base;
                run.retuned = heard;
                run.port.shared.set_lock(Lock::Locked);
            } else {
                // Stopped: the playhead stands where the master located.
                let l = if run.queued { Lock::Queued } else { Lock::Idle };
                run.port.shared.set_lock(l);
                if tl.epoch != run.epoch && !self.transport.is_playing() {
                    self.release_all();
                    self.transport.seek_f64(target);
                    self.reseek_events();
                    run.epoch = tl.epoch;
                }
            }
            return;
        }
        let tps = base * PPQ as f64 / (60.0 * sr);
        let err_us = (self.transport.position_f64() - target) / tps / sr * 1e6;
        if tl.epoch != run.epoch || err_us.abs() > RELOCK_US {
            self.sync_jump(target);
            run.epoch = tl.epoch;
            Self::report(run, err_us);
            return;
        }
        let pull = pull_of(err_us);
        Self::sync_retune(
            run,
            &mut self.transport,
            base,
            base * drift * (1.0 + pull),
            heard,
        );
        Self::report(run, err_us);
    }
}

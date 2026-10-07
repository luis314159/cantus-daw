//! The master node's pass (`master.rs`): gather, chain, strip, take, click — on the
//! callback, after the pool's part of the pass, reading what the other nodes left in
//! their cells.

use super::strip_run::{analyse, read_meters, SILENCE};
use super::*;
use crate::graph::gather::add_interleaved;
use crate::profile::Node;

impl Processor {
    /// Run the master over the pass `[p0, p0 + n)`: `out` is the pass's stretch of the
    /// device's interleaved buffer, zeroed at the top of the block.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_master_node(
        &mut self,
        snap: Option<&Snapshot>,
        out: &mut [f32],
        channels: usize,
        p0: usize,
        n: usize,
        position: &crate::fx::SongPosition,
        strip: MasterStrip,
        peaks: &mut [f32; 2],
    ) {
        self.prof.enter(Node::Engine);
        // ---- gather, in canonical order: the tracks routed here, then each strip's
        // output edge in the order --------------------------------------------------------
        {
            let nodes = &*self.nodes;
            let master = self.plan.master();
            if let Some(frame) = nodes.read(&nodes.frame) {
                for i in frame.map.inputs(master) {
                    strip::with_input(nodes, *i, n, &mut |c| add_interleaved(out, channels, c));
                }
            }
        }
        // The master's insert chain, on the summed mix, before its fader (ADR 0026).
        self.run_master(out, channels, n, p0, position);
        // Master strip: width, then pan and gain, then polarity last of all. Automation
        // (ADR 0027) overrides the snapshot's values while it drives them.
        let master_width = self.auto.master[2].unwrap_or(strip.width);
        if channels > 1 && master_width != 1.0 {
            for frame in out.chunks_exact_mut(channels) {
                let (mut l, mut r) = (frame[0], frame[1]);
                let (mid, side) = ((l + r) * 0.5, (l - r) * 0.5 * master_width.clamp(0.0, 2.0));
                l = mid + side;
                r = mid - side;
                frame[0] = l;
                frame[1] = r;
            }
        }
        let master_ramp = self
            .auto
            .k0(self.transport.samples())
            .filter(|_| self.auto.master_gain.is_some() || self.auto.master_pan.is_some());
        let master_gain = self.auto.master[0].unwrap_or(strip.gain);
        let (master_l, master_r) = match self.auto.master[1] {
            Some(p) => balance_gains(p),
            None => (strip.l, strip.r),
        };
        for (i, frame) in out.chunks_exact_mut(channels).enumerate() {
            let (g, ml, mr) = match master_ramp {
                Some(k0) => {
                    let k = k0 + i as u64;
                    let g = self.auto.master_gain.map_or(master_gain, |r| r.at(k));
                    let (l, r) = match self.auto.master_pan {
                        Some(p) => balance_gains(p.at(k)),
                        None => (master_l, master_r),
                    };
                    (g, l, r)
                }
                None => (master_gain, master_l, master_r),
            };
            for (ch, s) in frame.iter_mut().enumerate() {
                let pan = match ch {
                    0 => ml,
                    1 => mr,
                    _ => 1.0,
                };
                *s *= g * pan * strip.sign;
                if ch < 2 {
                    peaks[ch] = peaks[ch].max(s.abs());
                }
            }
        }
        // The recorder, on the master: the mix as it leaves the master strip.
        if self.capture_target == CaptureTarget::Master {
            match self.nodes.capture.try_borrow_mut() {
                Some(mut cap) => {
                    for frame in out.chunks_exact(channels) {
                        let (l, r) = (frame[0], if channels > 1 { frame[1] } else { frame[0] });
                        cap.frame(l, r);
                    }
                }
                None => self.nodes.conflict(),
            }
        }
        self.run_click(snap, out, channels, n);
    }

    /// The click, after the master strip: it is for the player, so the master fader does
    /// not turn it down and the recorder never hears it. It splits the pass at its own
    /// beats only: a beat due at a stretch's first sample sounds there, and the stretch
    /// ends at the next one.
    fn run_click(&mut self, snap: Option<&Snapshot>, out: &mut [f32], channels: usize, n: usize) {
        let beats = match snap {
            Some(s) if self.click_on() && self.transport.is_playing() => Some((
                s.time_signature.ticks_per_beat().0 as f64,
                s.time_signature.ticks_per_bar().0 as f64,
            )),
            _ => None,
        };
        let mut c0 = 0usize;
        while c0 < n {
            let mut c1 = n;
            if let Some((beat_len, bar_len)) = beats {
                let until = |p: &Self| {
                    (p.transport.samples_until(p.next_beat_tick) as usize).saturating_sub(c0)
                };
                if until(self) == 0 {
                    let in_bar = self.next_beat_tick.rem_euclid(bar_len);
                    if self.click_allowed_at(self.transport.samples() + c0 as u64) {
                        self.click.trigger(in_bar.abs() < 0.5);
                    }
                    self.next_beat_tick += beat_len;
                }
                c1 = n.min(c0 + until(self).max(1));
            }
            self.click_stretch(&mut out[c0 * channels..c1 * channels], channels, c1 - c0);
            c0 = c1;
        }
    }

    /// One stretch of the click into the output. With latency it goes through a line of
    /// the output's latency, so it comes out with the beat it marks (ADR 0022).
    fn click_stretch(&mut self, out: &mut [f32], channels: usize, n: usize) {
        let click_line = self
            .click_line
            .filter(|li| self.click.is_active() || self.nodes.line_ringing(Some(*li)));
        if let Some(li) = click_line {
            let active = self.click.is_active();
            for i in 0..n {
                let c = if active { self.click.next() } else { 0.0 };
                self.edge_l[i] = c;
                self.edge_r[i] = c;
            }
            if let Some(mut line) = self.nodes.line(li) {
                line.run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
            }
            for (i, frame) in out.chunks_exact_mut(channels).enumerate() {
                frame[0] += self.edge_l[i];
                if channels > 1 {
                    frame[1] += self.edge_r[i];
                }
            }
        } else if self.click.is_active() && self.click_line.is_none() {
            for frame in out.chunks_exact_mut(channels) {
                let c = self.click.next();
                frame[0] += c;
                if channels > 1 {
                    frame[1] += c;
                }
            }
        }
    }

    /// Run the master chain on the summed mix in `out` (interleaved, `n` frames, the pass
    /// starting `p0` frames into the block), in place. Called after the master's gather
    /// and before its width, gain, balance and polarity.
    ///
    /// A mono output carries the mix as `l + r`; the chain hears half of it on each side
    /// and gives back their sum, which at unity is the input to the bit.
    pub(super) fn run_master(
        &mut self,
        out: &mut [f32],
        channels: usize,
        n: usize,
        p0: usize,
        position: &crate::fx::SongPosition,
    ) {
        if self.master.effects.is_empty() {
            self.master.gr = 0.0;
            return;
        }
        // The deepest reduction of this block: the first pass starts it afresh.
        if p0 == 0 {
            self.master.gr = 0.0;
        }
        if !self.master.runs() {
            return;
        }
        let start = self.clock + p0 as u64;
        let pass = self.pass;
        let nodes = self.nodes.clone();
        let m = &mut self.master;
        let l = &mut m.buf_l[..n];
        let r = &mut m.buf_r[..n];
        for i in 0..n {
            let frame = &out[i * channels..(i + 1) * channels];
            if channels > 1 {
                l[i] = frame[0];
                r[i] = frame[1];
            } else {
                l[i] = frame[0] * 0.5;
                r[i] = frame[0] * 0.5;
            }
        }
        let dry_l = &mut m.dry_l[..n];
        let dry_r = &mut m.dry_r[..n];
        for (i, e) in m.effects.iter_mut().enumerate() {
            self.prof.enter(Node::MasterInsert(i));
            let looked_at = self.analysed == Some(e.id);
            let mut ana = if looked_at {
                let a = nodes.analysis.try_borrow_mut();
                if a.is_none() {
                    nodes.conflict();
                }
                a
            } else {
                None
            };
            if let Some(a) = ana.as_mut() {
                for k in 0..n {
                    a.pre[k] = (l[k] + r[k]) * 0.5;
                }
            }
            // Notes from the track it listens to, due in this pass (ADR 0022).
            if let Some(li) = m.listener[i] {
                if let Some(mut lis) = nodes.listener(li as usize) {
                    if let Some(outbox) = nodes.read(&nodes.tracks[lis.track].outbox) {
                        lis.deliver(&outbox, start, start + n as u64, |at, note| e.note(at, note));
                    }
                }
            }
            // Bypassed or switched off: a wire when it has no latency, a soft bypass (its
            // dry line, the effect still running) when it has.
            let off = m.bypass[i] || !m.fx_enabled;
            let latent = e.latency() > 0;
            if off && !latent {
                if let Some(a) = ana.as_mut() {
                    analyse(a, l, r);
                }
                continue;
            }
            let mix = m.mix[i];
            // The master runs after every strip: a key written this pass is heard.
            let kbuf = m.key[i].and_then(|k| nodes.read(&nodes.strips[k].key));
            let key = m.key[i].map(|_| match kbuf.as_deref() {
                Some(b) if b.pass == pass => (&b.l[..n], &b.r[..n]),
                _ => (&SILENCE[..n], &SILENCE[..n]),
            });
            e.set_position(position);
            if mix >= 1.0 && !latent {
                e.process_keyed(l, r, key);
            } else {
                dry_l.copy_from_slice(l);
                dry_r.copy_from_slice(r);
                if let Some(li) = m.dry_line[i] {
                    if let Some(mut line) = nodes.line(li) {
                        line.run(dry_l, dry_r);
                    }
                }
                e.process_keyed(l, r, key);
                if off {
                    l.copy_from_slice(dry_l);
                    r.copy_from_slice(dry_r);
                } else if mix < 1.0 {
                    let dry = 1.0 - mix;
                    for k in 0..n {
                        l[k] = l[k] * mix + dry_l[k] * dry;
                        r[k] = r[k] * mix + dry_r[k] * dry;
                    }
                }
            }
            drop(kbuf);
            let effect = e.id;
            e.drain_param_events(|event| {
                if self
                    .tx
                    .push(EngineEvent::FxParam { effect, event })
                    .is_err()
                {
                    self.stats.dropped_events += 1;
                }
            });
            if let Some(a) = ana.as_mut() {
                analyse(a, l, r);
            }
            if off {
                continue;
            }
            if let Some(gr) = e.gain_reduction_db() {
                m.gr = m.gr.max(gr);
            }
            if let Some(a) = ana.as_mut() {
                read_meters(a, e);
            }
        }
        self.prof.enter(Node::Engine);
        for i in 0..n {
            let frame = &mut out[i * channels..(i + 1) * channels];
            if channels > 1 {
                frame[0] = l[i];
                frame[1] = r[i];
            } else {
                frame[0] = l[i] + r[i];
            }
        }
    }
}

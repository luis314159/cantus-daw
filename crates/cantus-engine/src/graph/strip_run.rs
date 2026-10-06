//! A strip node's work after its gather (`strip.rs`): the chain, then what it leaves for
//! its consumers. The arithmetic is the serial engine's, expression for expression; only
//! the span changed — a whole pass instead of a segment.

use super::*;
use crate::profile::Node;

impl Processor {
    /// Polarity → swap → inserts → EQ → width → pan law → fader → mute → automation, in
    /// place in the scratch pair. Returns the fader's gains (`(1, 1)` when a ramp already
    /// scaled the pair) and whether the strip is muted.
    pub(super) fn strip_chain(
        &mut self,
        slot: usize,
        p0: usize,
        n: usize,
        position: &crate::fx::SongPosition,
        law: PanLaw,
    ) -> (f32, f32, bool) {
        let l = &mut self.buf_l[..n];
        let r = &mut self.buf_r[..n];
        let dry_l = &mut self.dry_l[..n];
        let dry_r = &mut self.dry_r[..n];
        let start = self.clock + p0 as u64;
        let (gl, gr, muted) = {
            let st = &mut self.strips[slot];
            // Polarity first: the console's Ø is an input-stage switch, so the whole
            // channel — inserts included — comes out inverted.
            if st.phase_invert {
                for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                    *a = -*a;
                    *b = -*b;
                }
            }
            // Swap is the other input-stage switch: the channel arrives mirrored, and a
            // ping-pong delay behind it starts on the other side.
            if st.swap {
                for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                    std::mem::swap(a, b);
                }
            }
            for (i, e) in st.effects.iter_mut().enumerate() {
                self.prof.enter(Node::Insert(slot, i));
                let looked_at = self.analysed == Some(e.id);
                if looked_at {
                    for k in 0..n {
                        self.ana_pre[k] = (l[k] + r[k]) * 0.5;
                    }
                }
                // Notes from the track it listens to, due in this pass, at their sample in
                // it (ADR 0022) — delivered even to a bypassed effect, so it knows what is
                // held when it is back.
                if let Some(li) = st.listener[i] {
                    let lis = &mut self.listeners[li as usize];
                    let outbox = &self.slots[lis.track].outbox;
                    lis.deliver(outbox, start, start + n as u64, |at, note| e.note(at, note));
                }
                // Bypassed, or the whole chain switched off. An insert with no latency is
                // then a wire and does not run; a latent one is a *soft* bypass (ADR
                // 0022): it keeps running, so its state follows the signal, and what
                // leaves the slot is its dry line — the input delayed by exactly its
                // latency.
                let off = st.bypass[i] || !st.fx_enabled;
                let latent = e.latency() > 0;
                if off && !latent {
                    if looked_at {
                        Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
                    }
                    continue;
                }
                let mix = st.mix[i];
                // A keyed effect hears the key strip's post-fader output — or silence, if
                // that strip made none this pass.
                let key = st.key[i].map(|k| {
                    if self.key_valid[k] {
                        (&self.key_bufs[k].0[..n], &self.key_bufs[k].1[..n])
                    } else {
                        (&self.silence[..n], &self.silence[..n])
                    }
                });
                // A unit that follows the song (a hosted plug-in) hears where it is before
                // each call.
                e.set_position(position);
                if mix >= 1.0 && !latent {
                    e.process_keyed(l, r, key);
                } else {
                    // The slot's dry/wet. The effect runs even at 0 %, so its delay line or
                    // tail is where it would be when the knob comes back up; a latent
                    // effect's dry line runs at every setting, so it is in step whenever
                    // it is needed.
                    dry_l.copy_from_slice(l);
                    dry_r.copy_from_slice(r);
                    if let Some(li) = self.dry_line[slot][i] {
                        self.pdc.lines[li].run(dry_l, dry_r);
                    }
                    e.process_keyed(l, r, key);
                    if off {
                        l.copy_from_slice(dry_l);
                        r.copy_from_slice(dry_r);
                    } else if mix < 1.0 {
                        let dry = 1.0 - mix;
                        for i in 0..n {
                            l[i] = l[i] * mix + dry_l[i] * dry;
                            r[i] = r[i] * mix + dry_r[i] * dry;
                        }
                    }
                }
                // A hosted plug-in's own window: its gestures go to the host.
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
                if off {
                    if looked_at {
                        Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
                    }
                    continue;
                }
                if looked_at {
                    Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
                    if let Some(d) = e.eq_deltas() {
                        self.meters.eq_deltas = d;
                    }
                    if let Some(c) = e.comp_meter() {
                        self.meters.comp = [c.input, c.output, c.gr_db];
                    }
                    if let Some(t) = e.telemetry() {
                        self.meters.fx = t;
                    }
                }
            }
            self.prof.enter(Node::Strip(slot));
            // The strip's own EQ, after the inserts and before the channel is placed —
            // where a console's channel EQ sits.
            if !st.eq_flat {
                st.eq_l.process(l);
                st.eq_r.process(r);
            }
            // Width, then pan: narrowing a stereo reverb has to happen before the channel
            // is placed, or the placement is what gets narrowed.
            apply_width(l, r, st.width);
            let (pl, pr) = law.gains(st.pan);
            (st.gain * pl, st.gain * pr, st.muted)
        };
        let (gl, gr) = if muted { (0.0, 0.0) } else { (gl, gr) };
        // An automated fader or pan ramping (ADR 0027): applied to the buffer, so every tap
        // after it — keys, the recorder, outputs, sends — is post-fader.
        match self.auto.k0(self.transport.samples()) {
            Some(k0)
                if !muted
                    && auto::ramp_strip(&self.auto, slot, &self.strips[slot], law, k0, l, r) =>
            {
                (1.0, 1.0, muted)
            }
            _ => (gl, gr, muted),
        }
    }

    /// What leaves the strip, from the scratch pair after its chain: the stem, the key,
    /// the take, the output edge, the delayed sends' lines, and the post-chain pair for the
    /// undelayed sends its consumers scale themselves.
    pub(super) fn strip_outputs(
        &mut self,
        slot: usize,
        p0: usize,
        n: usize,
        gl: f32,
        gr: f32,
        muted: bool,
    ) {
        let l = &self.buf_l[..n];
        let r = &self.buf_r[..n];
        if let Some(tap) = self.stem_tap.as_mut() {
            tap.strip(slot, p0, l, r, gl, gr);
        }
        // A strip something is keyed from keeps what leaves its fader.
        if self.strips[slot].is_key {
            let (kl, kr) = &mut self.key_bufs[slot];
            for i in 0..n {
                kl[i] = l[i] * gl;
                kr[i] = r[i] * gr;
            }
            self.key_valid[slot] = true;
        }
        // The recorder, on a strip: exactly what leaves the fader — delayed, in a project
        // with latency, by the strip's path to the master, so the take is in time with what
        // is heard (ADR 0022).
        if self.capture_slot == Some(slot) {
            self.strip_captured = true;
            if let Some(li) = self.capture_line {
                for i in 0..n {
                    self.edge_l[i] = l[i] * gl;
                    self.edge_r[i] = r[i] * gr;
                }
                self.pdc.lines[li].run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
                for i in 0..n {
                    Self::capture_frame(
                        &mut self.capture,
                        &mut self.stats,
                        self.edge_l[i],
                        self.edge_r[i],
                    );
                }
            } else {
                for i in 0..n {
                    Self::capture_frame(&mut self.capture, &mut self.stats, l[i] * gl, r[i] * gr);
                }
            }
        }
        // The output edge, as its destination adds it: through its line first and at unity
        // after, or at the fader's gains.
        let out = &mut self.strip_out[slot];
        let mut peak = 0f32;
        match self.out_line[slot] {
            Some(li) => {
                for i in 0..n {
                    out.out_l[i] = l[i] * gl;
                    out.out_r[i] = r[i] * gr;
                }
                self.pdc.lines[li].run(&mut out.out_l[..n], &mut out.out_r[..n]);
                for i in 0..n {
                    let sl = out.out_l[i] * 1.0;
                    let sr = out.out_r[i] * 1.0;
                    peak = peak.max(sl.abs()).max(sr.abs());
                }
            }
            None => {
                for i in 0..n {
                    let sl = l[i] * gl;
                    let sr = r[i] * gr;
                    peak = peak.max(sl.abs()).max(sr.abs());
                    out.out_l[i] = sl;
                    out.out_r[i] = sr;
                }
            }
        }
        out.ran = true;
        out.muted = muted;
        out.gl = gl;
        out.gr = gr;
        let st = &mut self.strips[slot];
        st.peak = st.peak.max(peak);
        // Post-fader sends: the same signal that just left the fader, at each send's level.
        // A delayed one goes through its line here (a muted strip feeds its line silence,
        // so the line drains); an undelayed one its consumer scales from the pair kept
        // below. A muted strip without lines sends nothing.
        let (send_count, sends) = (st.send_count, st.sends);
        let mut undelayed = false;
        if !muted || self.send_line[slot].iter().any(|x| x.is_some()) {
            for (k, (_, level)) in sends.iter().take(send_count).enumerate() {
                let Some(li) = self.send_line[slot][k] else {
                    undelayed |= !muted;
                    continue;
                };
                let (sl, sr) = (gl * level, gr * level);
                let [el, er] = &mut self.edge_out[li];
                for i in 0..n {
                    el[i] = l[i] * sl;
                    er[i] = r[i] * sr;
                }
                self.pdc.lines[li].run(&mut el[..n], &mut er[..n]);
            }
        }
        // The pair itself, for the undelayed sends: handed over, not copied — the scratch
        // pair and the strip's own swap places.
        if undelayed {
            let out = &mut self.strip_out[slot];
            std::mem::swap(&mut self.buf_l, &mut out.post_l);
            std::mem::swap(&mut self.buf_r, &mut out.post_r);
        }
    }
}

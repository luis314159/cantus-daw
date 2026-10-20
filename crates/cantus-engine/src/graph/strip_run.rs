//! A strip node's work after its gather (`strip.rs`): the chain, then what it leaves for
//! its consumers. The arithmetic is the serial engine's, expression for expression; only
//! the span changed — a whole pass instead of a segment — and the thread.

use super::ctx::PassCtx;
use super::exec::{Analysis, Nodes, Outgoing, PushEvent, Scratch};
use super::strip::{StripOut, StripSlot};
use super::*;
use crate::profile::Node;

/// Silence, for a key whose strip made none this pass.
pub(crate) static SILENCE: [f32; PASS_FRAMES] = [0.0; PASS_FRAMES];

/// Input/output pairs into the analyser's ring. The analyser is a picture, so a full ring
/// simply drops the rest of the pass.
#[inline]
pub(crate) fn analyse(a: &mut Analysis, l: &[f32], r: &[f32]) {
    let Analysis { ring, pre, .. } = a;
    if let Some(ring) = ring.as_mut() {
        for k in 0..l.len() {
            if ring.slots() < 2 {
                break;
            }
            let _ = ring.push(pre[k]);
            let _ = ring.push((l[k] + r[k]) * 0.5);
        }
    }
}

/// A watched Speculum's signal into its scope ring (ADR 0056). Frames fill a chunk of
/// [`CHUNK`](crate::fx::speculum::CHUNK) across passes — so the ring's capacity is in
/// frames, however short the passes — and a chunk goes in when it is full or when the song
/// jumps (a seek, a loop, a tempo change). Each chunk carries its owner, the song's
/// position at its first frame **as heard at this slot** (the transport's, less the
/// compensation the slot's signal waits: `delay`), and the frames a full ring lost before
/// it. Nothing for an effect nobody watches (eight comparisons). Allocation-free.
#[inline]
#[allow(clippy::too_many_arguments)]
pub(crate) fn scope(
    nodes: &Nodes,
    scoped: &[Option<EffectId>],
    delays: &[usize],
    position: &crate::fx::SongPosition,
    map: &cantus_core::TempoMap,
    sample_rate: f64,
    id: EffectId,
    l: &[f32],
    r: &[f32],
) {
    use crate::fx::speculum::{ScopeChunk, CHUNK};
    let Some(i) = scoped.iter().position(|s| *s == Some(id)) else {
        return;
    };
    let Some(mut tap) = nodes.scopes[i].try_borrow_mut() else {
        nodes.conflict();
        return;
    };
    let tap = &mut *tap;
    if tap.owner != Some(id) {
        // A ring given to another Speculum: nothing of the last one's goes on.
        tap.owner = Some(id);
        tap.pending = ScopeChunk::EMPTY;
        tap.lost = 0;
    }
    let per_frame = position.tempo_bpm / 60.0 / sample_rate.max(1.0);
    let late = delays.get(i).copied().unwrap_or(0) as f64;
    // Where the audio reaching the scope is: through the tempo map when it moves (review
    // 1: across a point the constant arithmetic misplaced late audio).
    let first = position.playing.then(|| {
        if map.is_constant() {
            position.beats - late * per_frame
        } else {
            position.earlier_in(late as usize, sample_rate, map).beats
        }
    });
    // Does this pass carry on from the chunk being filled?
    let p = &tap.pending;
    if p.frames > 0 {
        let on = match (p.beat, first) {
            (Some(b), Some(f)) => {
                // Inside a ramp's step (ADR 0060) the beats bend away from the step's
                // straight line by far less than a frame: carrying on is no jump.
                let tol = if position.tempo_inc != 0.0 {
                    1e-3
                } else {
                    1e-6
                };
                p.tempo_bpm == position.tempo_bpm
                    && (b + f64::from(p.frames) * per_frame - f).abs() < tol
            }
            (None, None) => true,
            _ => false,
        };
        if !on {
            flush(tap);
        }
    }
    let n = l.len().min(r.len());
    for k in 0..n {
        let p = &mut tap.pending;
        if p.frames == 0 {
            p.owner = id;
            p.beat = first.map(|b| b + k as f64 * per_frame);
            p.tempo_bpm = position.tempo_bpm;
        }
        let at = p.frames as usize;
        p.l[at] = l[k];
        p.r[at] = r[k];
        p.frames += 1;
        if p.frames as usize == CHUNK {
            flush(tap);
        }
    }
}

/// The chunk being filled into the ring, or its frames counted lost when the ring is full.
#[inline]
fn flush(tap: &mut super::exec::ScopeTap) {
    let frames = u64::from(tap.pending.frames);
    if frames == 0 {
        return;
    }
    match tap.ring.as_mut() {
        Some(ring) if ring.slots() > 0 => {
            tap.pending.lost = tap.lost.min(u64::from(u32::MAX)) as u32;
            let _ = ring.push(tap.pending);
            tap.lost = 0;
        }
        Some(_) => tap.lost += frames,
        None => {}
    }
    tap.pending.frames = 0;
    tap.pending.lost = 0;
}

/// The analysed unit's readings, for the meters.
#[inline]
pub(crate) fn read_meters(a: &mut Analysis, e: &EffectUnit) {
    if let Some(d) = e.eq_deltas() {
        a.eq_deltas = Some(d);
    }
    if let Some(c) = e.comp_meter() {
        a.comp = Some([c.input, c.output, c.gr_db]);
    }
    if let Some(t) = e.telemetry() {
        a.fx = Some(t);
    }
}

/// What [`strip_chain`] leaves for [`strip_outputs`] besides the scratch pair.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Fader {
    /// The fader's gains after the pan law and mute (`(1, 1)` when a ramp already scaled
    /// the pair).
    pub gl: f32,
    pub gr: f32,
    pub muted: bool,
    /// A ramp scaled the pair and the strip has a pre-fader send, so the pair as it was
    /// before the fader is in the scratch's `dry_l`/`dry_r`. Otherwise the scratch pair
    /// itself is pre-fader (the fader is still in `gl`/`gr`).
    pub pre_apart: bool,
}

/// Polarity → swap → inserts → EQ → width → pan law → fader → mute → automation, in
/// place in the slot's scratch pair. Returns the fader's gains, whether the strip is muted
/// and where its pre-fader pair is.
pub(crate) fn strip_chain(
    nodes: &Nodes,
    ctx: &PassCtx,
    slot: usize,
    st: &mut StripSlot,
    sc: &mut Scratch,
) -> Fader {
    let n = ctx.n;
    let Scratch {
        buf_l,
        buf_r,
        dry_l,
        dry_r,
        events,
        prof,
        ..
    } = sc;
    let l = &mut buf_l[..n];
    let r = &mut buf_r[..n];
    let dry_l = &mut dry_l[..n];
    let dry_r = &mut dry_r[..n];
    let start = ctx.clock + ctx.p0 as u64;
    // Polarity first: the console's Ø is an input-stage switch, so the whole channel —
    // inserts included — comes out inverted.
    if st.phase_invert {
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            *a = -*a;
            *b = -*b;
        }
    }
    // Swap is the other input-stage switch: the channel arrives mirrored, and a ping-pong
    // delay behind it starts on the other side.
    if st.swap {
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            std::mem::swap(a, b);
        }
    }
    let StripSlot {
        effects,
        bypass,
        mix,
        fx_enabled,
        key,
        key_before,
        listener,
        dry_line,
        ..
    } = st;
    for (i, e) in effects.iter_mut().enumerate() {
        prof.enter(Node::Insert(slot, i));
        let looked_at = ctx.analysed == Some(e.id);
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
        // Notes from the track it listens to, due in this pass, at their sample in it
        // (ADR 0022) — delivered even to a bypassed effect, so it knows what is held when
        // it is back.
        if let Some(li) = listener[i] {
            if let Some(mut lis) = nodes.listener(li as usize) {
                if let Some(outbox) = nodes.read(&nodes.tracks[lis.track].outbox) {
                    lis.deliver(&outbox, start, start + n as u64, |at, note| {
                        e.note(at, note)
                    });
                }
            }
        }
        // Bypassed, or the whole chain switched off. An insert with no latency is then a
        // wire and does not run; a latent one is a *soft* bypass (ADR 0022): it keeps
        // running, so its state follows the signal, and what leaves the slot is its dry
        // line — the input delayed by exactly its latency.
        let off = bypass[i] || !*fx_enabled;
        let latent = e.latency() > 0;
        if off && !latent {
            if let Some(a) = ana.as_mut() {
                analyse(a, l, r);
            }
            scope(
                nodes,
                &ctx.scoped,
                &ctx.scoped_delay,
                &ctx.position,
                ctx.transport.map(),
                ctx.sample_rate,
                e.id,
                l,
                r,
            );
            continue;
        }
        let wet = mix[i];
        // A keyed effect hears the key strip's post-fader output — or silence, if that
        // strip made none this pass (or runs after this one: no dependency, silence).
        let kbuf = match key[i] {
            Some(k) if key_before[i] => nodes.read(&nodes.strips[k].key),
            _ => None,
        };
        let keyed = key[i].map(|_| match kbuf.as_deref() {
            Some(b) if b.pass == ctx.pass => (&b.l[..n], &b.r[..n]),
            _ => (&SILENCE[..n], &SILENCE[..n]),
        });
        // A unit that follows the song (a hosted plug-in) hears where it is before each
        // call.
        e.set_position(
            &ctx.position
                .earlier_in(e.late, ctx.sample_rate, ctx.transport.map()),
        );
        if wet >= 1.0 && !latent {
            e.process_keyed(l, r, keyed);
        } else {
            // The slot's dry/wet. The effect runs even at 0 %, so its delay line or tail is
            // where it would be when the knob comes back up; a latent effect's dry line
            // runs at every setting, so it is in step whenever it is needed.
            dry_l.copy_from_slice(l);
            dry_r.copy_from_slice(r);
            if let Some(li) = dry_line[i] {
                if let Some(mut line) = nodes.line(li) {
                    line.run(dry_l, dry_r);
                }
            }
            e.process_keyed(l, r, keyed);
            if off {
                l.copy_from_slice(dry_l);
                r.copy_from_slice(dry_r);
            } else if wet < 1.0 {
                let dry = 1.0 - wet;
                for i in 0..n {
                    l[i] = l[i] * wet + dry_l[i] * dry;
                    r[i] = r[i] * wet + dry_r[i] * dry;
                }
            }
        }
        drop(kbuf);
        // A hosted plug-in's own window: its gestures go to the host, after the pass.
        let effect = e.id;
        e.drain_param_events(|event| {
            if !events.push_event(Outgoing::Fx { effect, event }) {
                nodes.drop_event();
            }
        });
        // What found no room on its way into it, automated or not (review 2).
        let lost = e.take_dropped();
        if lost > 0 {
            nodes
                .dropped
                .fetch_add(lost, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some(a) = ana.as_mut() {
            analyse(a, l, r);
            if !off {
                read_meters(a, e);
            }
        }
        // A watched Speculum: its signal to its scope ring (ADR 0056).
        scope(
            nodes,
            &ctx.scoped,
            &ctx.scoped_delay,
            &ctx.position,
            ctx.transport.map(),
            ctx.sample_rate,
            e.id,
            l,
            r,
        );
    }
    prof.enter(Node::Strip(slot));
    // The strip's own EQ, after the inserts and before the channel is placed — where a
    // console's channel EQ sits.
    if !st.eq_flat {
        st.eq_l.process(l);
        st.eq_r.process(r);
    }
    // A freeze's or a bounce's render is taken here, before the width (ADR 0064): what
    // follows stays live on a frozen strip.
    if ctx.tap {
        match nodes.tap.try_borrow_mut() {
            Some(mut tap) => {
                if let Some(tap) = tap.as_mut() {
                    tap.strip_before_width(slot, ctx.p0, l, r);
                }
            }
            None => nodes.conflict(),
        }
    }
    // Width, then pan: narrowing a stereo reverb has to happen before the channel is
    // placed, or the placement is what gets narrowed.
    apply_width(l, r, st.width);
    let (pl, pr) = ctx.law.gains(st.pan);
    let (gl, gr, muted) = (st.gain * pl, st.gain * pr, st.muted);
    let (gl, gr) = if muted { (0.0, 0.0) } else { (gl, gr) };
    // An automated fader or pan ramping (ADR 0027): applied to the buffer, so every tap
    // after it — keys, the recorder, outputs, post-fader sends — is post-fader. A
    // pre-fader send needs the pair before it: kept apart first, only when there is one.
    let riding = st.gain_ramp.is_some() || st.pan_ramp.is_some();
    match ctx.k0(ctx.transport.samples()) {
        Some(k0) if !muted && riding => {
            let pre_apart = st.has_pre;
            if pre_apart {
                dry_l.copy_from_slice(l);
                dry_r.copy_from_slice(r);
            }
            auto::ramp_strip(st, ctx.law, k0, l, r);
            Fader {
                gl: 1.0,
                gr: 1.0,
                muted,
                pre_apart,
            }
        }
        _ => Fader {
            gl,
            gr,
            muted,
            pre_apart: false,
        },
    }
}

/// What leaves the strip, from the scratch pair after its chain: the stem, the key, the
/// take, the output edge, the delayed sends' lines, and the post-chain pair (and, under a
/// fader ride, the pre-fader one) for the undelayed sends its consumers scale themselves.
pub(crate) fn strip_outputs(
    nodes: &Nodes,
    ctx: &PassCtx,
    slot: usize,
    st: &mut StripSlot,
    out: &mut StripOut,
    sc: &mut Scratch,
    fader: Fader,
) {
    let n = ctx.n;
    let Fader {
        gl,
        gr,
        muted,
        pre_apart,
    } = fader;
    let Scratch {
        buf_l,
        buf_r,
        dry_l,
        dry_r,
        edge_l,
        edge_r,
        ..
    } = sc;
    let undelayed_pre = {
        let l = &buf_l[..n];
        let r = &buf_r[..n];
        if ctx.tap {
            match nodes.tap.try_borrow_mut() {
                Some(mut tap) => {
                    if let Some(tap) = tap.as_mut() {
                        tap.strip(slot, ctx.p0, l, r, gl, gr);
                    }
                }
                None => nodes.conflict(),
            }
        }
        // A strip something is keyed from keeps what leaves its fader.
        if st.is_key {
            match nodes.strips[slot].key.try_borrow_mut() {
                Some(mut k) => {
                    for i in 0..n {
                        k.l[i] = l[i] * gl;
                        k.r[i] = r[i] * gr;
                    }
                    k.pass = ctx.pass;
                }
                None => nodes.conflict(),
            }
        }
        // The recorder, on a strip: exactly what leaves the fader — delayed, in a project
        // with latency, by the strip's path to the master, so the take is in time with
        // what is heard (ADR 0022).
        if ctx.capture_slot == Some(slot) {
            match nodes.capture.try_borrow_mut() {
                Some(mut cap) => {
                    cap.ran = true;
                    match ctx.capture_line {
                        Some(li) => {
                            for i in 0..n {
                                edge_l[i] = l[i] * gl;
                                edge_r[i] = r[i] * gr;
                            }
                            if let Some(mut line) = nodes.line(li) {
                                line.run(&mut edge_l[..n], &mut edge_r[..n]);
                            }
                            for i in 0..n {
                                cap.frame(edge_l[i], edge_r[i]);
                            }
                        }
                        None => {
                            for i in 0..n {
                                cap.frame(l[i] * gl, r[i] * gr);
                            }
                        }
                    }
                }
                None => nodes.conflict(),
            }
        }
        // The output edge, as its destination adds it: through its line first and at
        // unity after, or at the fader's gains.
        let mut peak = 0f32;
        match st.out_line {
            Some(li) => {
                for i in 0..n {
                    out.out_l[i] = l[i] * gl;
                    out.out_r[i] = r[i] * gr;
                }
                if let Some(mut line) = nodes.line(li) {
                    line.run(&mut out.out_l[..n], &mut out.out_r[..n]);
                }
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
        st.peak = st.peak.max(peak);
        st.follow_peak = st.follow_peak.max(peak);
        // Sends: a post-fader one is the same signal that just left the fader, a
        // pre-fader one the pair before the fader and the pan law (kept apart under a
        // ride), each at its level. A delayed one goes through its line here (a muted strip
        // feeds its line silence, so the line drains); an undelayed one its consumer scales
        // from the pairs kept below. A muted strip without lines sends nothing.
        let (send_count, sends, taps) = (st.send_count, st.sends, st.send_tap);
        for (k, (_, level)) in sends.iter().take(send_count).enumerate() {
            out.send_level[k] = *level;
            out.send_tap[k] = taps[k];
        }
        out.pre_apart = pre_apart;
        let (pre_l, pre_r) = if pre_apart {
            (&dry_l[..n], &dry_r[..n])
        } else {
            (l, r)
        };
        let mut undelayed = false;
        let mut undelayed_pre = false;
        if !muted || st.send_line.iter().any(|x| x.is_some()) {
            for (k, (_, level)) in sends.iter().take(send_count).enumerate() {
                let Some(li) = st.send_line[k] else {
                    undelayed |= !muted;
                    undelayed_pre |= !muted && !taps[k].is_post();
                    continue;
                };
                // Mute silences either tap: its gains are then 0, and the line drains.
                let (src_l, src_r, sl, sr) = match taps[k] {
                    SendTap::Post => (l, r, gl * level, gr * level),
                    SendTap::Pre if muted => (pre_l, pre_r, 0.0, 0.0),
                    SendTap::Pre => (pre_l, pre_r, *level, *level),
                };
                match nodes.edges[li].try_borrow_mut() {
                    Some(mut e) => {
                        let [el, er] = &mut *e;
                        for i in 0..n {
                            el[i] = src_l[i] * sl;
                            er[i] = src_r[i] * sr;
                        }
                        if let Some(mut line) = nodes.line(li) {
                            line.run(&mut el[..n], &mut er[..n]);
                        }
                    }
                    None => nodes.conflict(),
                }
            }
        }
        if !undelayed {
            return;
        }
        undelayed_pre
    };
    // The pair itself, for the undelayed sends: handed over, not copied — the scratch pair
    // and the strip's own swap places.
    std::mem::swap(buf_l, &mut out.post_l);
    std::mem::swap(buf_r, &mut out.post_r);
    // Under a ride the pre-fader pair goes the same way, when an undelayed send reads it.
    if pre_apart && undelayed_pre {
        std::mem::swap(dry_l, &mut out.pre_l);
        std::mem::swap(dry_r, &mut out.pre_r);
    }
}

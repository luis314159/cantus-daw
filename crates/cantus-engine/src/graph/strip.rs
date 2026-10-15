//! A **strip node** (ADR 0031 §1.1): it **gathers** its inputs in canonical order
//! (`graph::gather`), then polarity → swap → inserts → EQ → width → pan law → fader, and
//! leaves for its consumers its **post-chain pair** and gains (what an undelayed send
//! scales), its **output edge** (what its destination adds as it is), each delayed send's
//! line output, its key (`l · gl`, for the inserts keyed from it) and its capture frames.
//!
//! A strip runs **whole passes** — usually the whole block — where the serial engine called
//! its chain once per segment, cut at every event of every track. It runs on whichever
//! thread of the pool takes it: everything it writes is its own cells, and everything it
//! reads from others is a producer's output the plan makes it wait for.

use super::ctx::{Frame, In};
use super::exec::{Nodes, Scratch};
use super::*;
use crate::automation::Ramp;
use crate::graph::gather::{add_stereo, Contribution};
use crate::profile::Node;
use std::sync::atomic::Ordering::Relaxed;

/// One mixer strip on the audio thread: its inserts and its controls. What it leaves for
/// its consumers each pass is its [`StripOut`].
///
/// Everything is allocated once for every possible strip, so routing can change with a
/// snapshot and nothing is allocated for it (ADR 0005).
#[allow(clippy::vec_box)]
pub(crate) struct StripSlot {
    pub id: Option<StripId>,
    /// Insert chain in processing order; capacity fixed at construction.
    pub effects: Vec<Box<EffectUnit>>,
    pub bypass: [bool; MAX_EFFECTS_PER_TRACK],
    /// Each slot's dry/wet; 1.0 is skipped entirely.
    pub mix: [f32; MAX_EFFECTS_PER_TRACK],
    /// The whole chain in or out.
    pub fx_enabled: bool,
    /// The strip's own EQ, one per channel. `eq_flat` says whether it runs at all.
    pub eq_l: ChannelEq,
    pub eq_r: ChannelEq,
    pub eq_flat: bool,
    pub gain: f32,
    pub pan: f32,
    pub width: f32,
    pub phase_invert: bool,
    pub swap: bool,
    pub muted: bool,
    /// The strip slot this one feeds; `None` is the master.
    pub output: Option<usize>,
    /// Sends as `(strip slot, level)`; the first `send_count` are live.
    pub sends: [(usize, f32); MAX_SENDS_PER_STRIP],
    /// Where each send takes its copy (aligned with `sends`): after the fader, or before
    /// it and the pan law (round 25, ADR 0041 §2).
    pub send_tap: [SendTap; MAX_SENDS_PER_STRIP],
    pub send_count: usize,
    /// A live send is pre-fader: under a fader ride, the pair before the ramp is kept.
    pub has_pre: bool,
    /// Per insert (aligned with `effects`): the strip it is keyed from, by id as the
    /// snapshot says and by slot once resolved — and whether that strip runs before this
    /// one (a key from a strip processed later is silence, as the serial engine heard it).
    pub key_id: [Option<StripId>; MAX_EFFECTS_PER_TRACK],
    pub key: [Option<usize>; MAX_EFFECTS_PER_TRACK],
    pub key_before: [bool; MAX_EFFECTS_PER_TRACK],
    /// Per insert: the track whose notes it hears (ADR 0022), and its place among the
    /// processor's listeners once resolved.
    pub midi_id: [Option<TrackId>; MAX_EFFECTS_PER_TRACK],
    pub listener: [Option<u16>; MAX_EFFECTS_PER_TRACK],
    /// Something's detector listens to this strip, so its post-fader output is kept.
    pub is_key: bool,
    /// Something came in, the last pass.
    pub active: bool,
    /// The block's peak, and the block it belongs to.
    pub peak: f32,
    pub peak_block: u64,
    /// Its compensation lines: the output edge, each send, each latent insert's dry path.
    pub out_line: Option<usize>,
    pub send_line: [Option<usize>; MAX_SENDS_PER_STRIP],
    pub dry_line: [Option<usize>; MAX_EFFECTS_PER_TRACK],
    /// Automated fader and pan ramping (ADR 0027).
    pub gain_ramp: Option<Ramp>,
    pub pan_ramp: Option<Ramp>,
    /// The output's peak since the last control point, for a modulator that follows the
    /// strip (ADR 0057); the control phase reads and empties it.
    pub follow_peak: f32,
}

impl StripSlot {
    pub fn new() -> Self {
        StripSlot {
            id: None,
            effects: Vec::with_capacity(MAX_EFFECTS_PER_TRACK),
            bypass: [false; MAX_EFFECTS_PER_TRACK],
            mix: [1.0; MAX_EFFECTS_PER_TRACK],
            fx_enabled: true,
            eq_l: ChannelEq::default(),
            eq_r: ChannelEq::default(),
            eq_flat: true,
            gain: 1.0,
            pan: 0.0,
            width: 1.0,
            phase_invert: false,
            swap: false,
            muted: false,
            output: None,
            sends: [(0, 0.0); MAX_SENDS_PER_STRIP],
            send_tap: [SendTap::Post; MAX_SENDS_PER_STRIP],
            send_count: 0,
            has_pre: false,
            key_id: [None; MAX_EFFECTS_PER_TRACK],
            key: [None; MAX_EFFECTS_PER_TRACK],
            key_before: [false; MAX_EFFECTS_PER_TRACK],
            midi_id: [None; MAX_EFFECTS_PER_TRACK],
            listener: [None; MAX_EFFECTS_PER_TRACK],
            is_key: false,
            active: false,
            peak: 0.0,
            peak_block: 0,
            out_line: None,
            send_line: [None; MAX_SENDS_PER_STRIP],
            dry_line: [None; MAX_EFFECTS_PER_TRACK],
            gain_ramp: None,
            pan_ramp: None,
            follow_peak: 0.0,
        }
    }

    /// The deepest reduction of the strip's working compressors, for the desk's lane.
    pub fn gr(&self) -> f32 {
        let mut gr = 0.0f32;
        if self.fx_enabled && self.active {
            for (i, e) in self.effects.iter().enumerate() {
                if let (false, Some(m)) = (self.bypass[i], e.comp_meter()) {
                    gr = gr.max(m.gr_db);
                }
            }
        }
        gr
    }
}

/// What a strip slot left for its consumers in the current pass.
pub(crate) struct StripOut {
    /// It ran this pass: everything below is this pass's.
    pub ran: bool,
    pub muted: bool,
    /// The fader's gains after mute and automation (`(1, 1)` when a ramp already scaled
    /// the pair).
    pub gl: f32,
    pub gr: f32,
    /// Each send's level and tap this pass, for the consumers that scale the post-chain
    /// pair.
    pub send_level: [f32; MAX_SENDS_PER_STRIP],
    pub send_tap: [SendTap; MAX_SENDS_PER_STRIP],
    /// After the chain and the fader's ramp, before the fader's constant gains.
    pub post_l: Vec<f32>,
    pub post_r: Vec<f32>,
    /// `pre_l`/`pre_r` hold this pass's pre-fader pair: a fader ride scaled `post_l` in
    /// place, so the pair before the ramp was kept apart. Otherwise the pre-fader pair *is*
    /// `post_l`/`post_r` (the fader is still in `gl`/`gr`).
    pub pre_apart: bool,
    pub pre_l: Vec<f32>,
    pub pre_r: Vec<f32>,
    /// The output edge, as its destination adds it.
    pub out_l: Vec<f32>,
    pub out_r: Vec<f32>,
}

impl StripOut {
    pub fn new() -> Self {
        StripOut {
            ran: false,
            muted: false,
            gl: 1.0,
            gr: 1.0,
            send_level: [0.0; MAX_SENDS_PER_STRIP],
            send_tap: [SendTap::Post; MAX_SENDS_PER_STRIP],
            post_l: vec![0.0; PASS_FRAMES],
            post_r: vec![0.0; PASS_FRAMES],
            pre_apart: false,
            pre_l: vec![0.0; PASS_FRAMES],
            pre_r: vec![0.0; PASS_FRAMES],
            out_l: vec![0.0; PASS_FRAMES],
            out_r: vec![0.0; PASS_FRAMES],
        }
    }
}

/// Add one input, as its producer left it for this pass of `n` frames, through `add` —
/// or nothing (the producer was idle, did not run, or — a muted strip's undelayed send —
/// sends nothing). Returns whether it added. The producer's output is borrowed shared for
/// the call: the plan made this node wait for it.
#[inline]
pub(crate) fn with_input(
    nodes: &Nodes,
    input: In,
    n: usize,
    add: &mut dyn FnMut(Contribution<'_>),
) -> bool {
    match input {
        In::Track(slot) => {
            let Some(o) = nodes.read(&nodes.tracks[slot as usize].out) else {
                return false;
            };
            let o: &TrackOut = &o;
            if !o.active {
                return false;
            }
            add(Contribution::Plain(&o.l[..n], &o.r[..n]));
        }
        In::Out(slot) => {
            let Some(o) = nodes.read(&nodes.strips[slot as usize].out) else {
                return false;
            };
            let o: &StripOut = &o;
            if !o.ran {
                return false;
            }
            add(Contribution::Plain(&o.out_l[..n], &o.out_r[..n]));
        }
        In::Line(slot, li) => {
            let ran = nodes
                .read(&nodes.strips[slot as usize].out)
                .is_some_and(|o| {
                    let o: &StripOut = &o;
                    o.ran
                });
            if !ran {
                return false;
            }
            let Some(e) = nodes.read(&nodes.edges[li as usize]) else {
                return false;
            };
            let [l, r] = &*e;
            add(Contribution::Plain(&l[..n], &r[..n]));
        }
        In::Send(slot, k) => {
            let Some(o) = nodes.read(&nodes.strips[slot as usize].out) else {
                return false;
            };
            let o: &StripOut = &o;
            if !o.ran || o.muted {
                return false;
            }
            let level = o.send_level[k as usize];
            match o.send_tap[k as usize] {
                SendTap::Post => {
                    let (sl, sr) = (o.gl * level, o.gr * level);
                    add(Contribution::Scaled(&o.post_l[..n], &o.post_r[..n], sl, sr));
                }
                // Before the fader and the pan law: the level alone. (Mute and solo
                // silenced it above.)
                SendTap::Pre if o.pre_apart => {
                    add(Contribution::Scaled(
                        &o.pre_l[..n],
                        &o.pre_r[..n],
                        level,
                        level,
                    ));
                }
                SendTap::Pre => {
                    add(Contribution::Scaled(
                        &o.post_l[..n],
                        &o.post_r[..n],
                        level,
                        level,
                    ));
                }
            }
        }
    }
    true
}

/// Gather `inputs` in canonical order into a stereo accumulator; `false` when none came
/// (the accumulator is then untouched).
fn gather(nodes: &Nodes, inputs: &[In], n: usize, acc_l: &mut [f32], acc_r: &mut [f32]) -> bool {
    let mut first = true;
    for &i in inputs {
        with_input(nodes, i, n, &mut |c| {
            add_stereo(&mut acc_l[..n], &mut acc_r[..n], c, first);
            first = false;
        });
    }
    !first
}

/// Whether a strip's outgoing delay lines still hold signal, so the strip must run this
/// pass even though nothing new came in.
fn lines_ringing(nodes: &Nodes, st: &StripSlot) -> bool {
    nodes.line_ringing(st.out_line) || st.send_line.iter().any(|l| nodes.line_ringing(*l))
}

/// Run plan node `node`, the strip in slot `slot`, over the pass. It rests when nothing
/// came in and nothing in it rings.
pub(crate) fn run_strip(nodes: &Nodes, frame: &Frame, node: usize, slot: usize, sc: &mut Scratch) {
    sc.prof.enter(Node::Strip(slot));
    let ctx = &frame.ctx;
    let n = ctx.n;
    let Some(mut st) = nodes.strip(slot) else {
        return;
    };
    let Some(mut out) = nodes.strips[slot].out.try_borrow_mut() else {
        nodes.conflict();
        return;
    };
    if st.peak_block != ctx.block {
        st.peak = 0.0;
        st.peak_block = ctx.block;
    }
    // ---- gather, in canonical order, into the scratch pair ------------------------------
    let any = gather(
        nodes,
        frame.map.inputs(node),
        n,
        &mut sc.buf_l,
        &mut sc.buf_r,
    );
    // An EQ with signal still in it has to keep running over silence, or its state
    // would freeze at a different point for every block size and the next note would
    // meet a different filter. Quiet sections are exactly zero (denormals are
    // flushed), so skipping them changes nothing.
    let eq_ringing = !st.eq_flat && !(st.eq_l.is_quiet() && st.eq_r.is_quiet());
    let ringing = !st.effects.is_empty() || eq_ringing;
    st.active = any;
    if !any && !ringing && !lines_ringing(nodes, &st) {
        out.ran = false; // Nothing came in and nothing is ringing.
    } else {
        if !any {
            sc.buf_l[..n].iter_mut().for_each(|x| *x = 0.0);
            sc.buf_r[..n].iter_mut().for_each(|x| *x = 0.0);
        }
        let fader = super::strip_run::strip_chain(nodes, ctx, slot, &mut st, sc);
        super::strip_run::strip_outputs(nodes, ctx, slot, &mut st, &mut out, sc, fader);
    }
    nodes.strip_peak[slot].store(st.peak.to_bits(), Relaxed);
    nodes.strip_gr[slot].store(st.gr().to_bits(), Relaxed);
}

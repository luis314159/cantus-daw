//! A **strip node** (ADR 0031 §1.1): it **gathers** its inputs in canonical order
//! (`graph::gather`), then polarity → swap → inserts → EQ → width → pan law → fader, and
//! leaves for its consumers its **post-chain pair** and gains (what an undelayed send
//! scales), its **output edge** (what its destination adds as it is), each delayed send's
//! line output, its key (`l · gl`, for the inserts keyed from it) and its capture frames.
//!
//! A strip runs **whole passes** — usually the whole block — where the serial engine called
//! its chain once per segment, cut at every event of every track.
//!
//! A child of `processor` (it owns the strip's state through the processor today).

use super::*;
use crate::graph::gather::{gather_stereo, Contribution};
use crate::graph::plan::Input;
use crate::profile::Node;

/// What a strip slot left for its consumers in the current pass.
pub(super) struct StripOut {
    /// It ran this pass: everything below is this pass's.
    pub ran: bool,
    pub muted: bool,
    /// The fader's gains after mute and automation (`(1, 1)` when a ramp already scaled
    /// the pair).
    pub gl: f32,
    pub gr: f32,
    /// After the chain and the fader's ramp, before the fader's constant gains.
    pub post_l: Vec<f32>,
    pub post_r: Vec<f32>,
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
            post_l: vec![0.0; PASS_FRAMES],
            post_r: vec![0.0; PASS_FRAMES],
            out_l: vec![0.0; PASS_FRAMES],
            out_r: vec![0.0; PASS_FRAMES],
        }
    }
}

/// What the producers left, read by a node's gather. Borrowed field by field from the
/// processor so the gathering node's own buffers can be written at the same time.
pub(super) struct Produced<'a> {
    pub track_to_slot: &'a [usize],
    pub strip_to_slot: &'a [usize],
    pub track_out: &'a [crate::processor::track::TrackOut],
    pub strip_out: &'a [StripOut],
    pub edge_out: &'a [[Vec<f32>; 2]],
    pub send_line: &'a [[Option<usize>; MAX_SENDS_PER_STRIP]],
    pub strips: &'a [StripSlot],
}

impl Produced<'_> {
    /// One input as its producer left it for this pass of `n` frames, or nothing (the
    /// producer was idle, did not run, or — a muted strip's undelayed send — sends
    /// nothing).
    #[inline]
    pub fn contribution(&self, input: Input, n: usize) -> Option<Contribution<'_>> {
        match input {
            Input::Track(ti) => {
                let o = &self.track_out[*self.track_to_slot.get(ti as usize)?];
                o.active.then(|| Contribution::Plain(&o.l[..n], &o.r[..n]))
            }
            Input::Out(si) => {
                let o = &self.strip_out[*self.strip_to_slot.get(si as usize)?];
                o.ran
                    .then(|| Contribution::Plain(&o.out_l[..n], &o.out_r[..n]))
            }
            Input::Send(si, k) => {
                let slot = *self.strip_to_slot.get(si as usize)?;
                let o = &self.strip_out[slot];
                if !o.ran {
                    return None;
                }
                let k = k as usize;
                if let Some(li) = self.send_line[slot][k] {
                    let [l, r] = &self.edge_out[li];
                    return Some(Contribution::Plain(&l[..n], &r[..n]));
                }
                if o.muted {
                    return None;
                }
                let level = self.strips[slot].sends[k].1;
                let (sl, sr) = (o.gl * level, o.gr * level);
                Some(Contribution::Scaled(&o.post_l[..n], &o.post_r[..n], sl, sr))
            }
        }
    }
}

/// The producers' outputs, for a gather: the processor's fields borrowed one by one, so
/// the gathering node may write its own at the same time.
macro_rules! produced {
    ($p:expr) => {
        $crate::processor::strip::Produced {
            track_to_slot: &$p.track_to_slot,
            strip_to_slot: &$p.strip_to_slot,
            track_out: &$p.track_out,
            strip_out: &$p.strip_out,
            edge_out: &$p.edge_out,
            send_line: &$p.send_line,
            strips: &$p.strips,
        }
    };
}
pub(super) use produced;

impl Processor {
    /// Run the strip at place `place` of the order over the pass `[p0, p0 + n)`. Returns
    /// whether it ran (it rests when nothing came in and nothing in it rings).
    pub(super) fn run_strip(
        &mut self,
        place: usize,
        p0: usize,
        n: usize,
        position: &crate::fx::SongPosition,
        law: PanLaw,
    ) -> bool {
        let slot = self.strip_play_order[place];
        let node = self.plan.strip_node(place);
        // ---- gather, in canonical order, into the scratch pair --------------------------
        let any = {
            let produced = produced!(self);
            let inputs = self.plan.inputs(node);
            gather_stereo(
                &mut self.buf_l[..n],
                &mut self.buf_r[..n],
                inputs.iter().filter_map(|i| produced.contribution(*i, n)),
            )
        };
        let st = &self.strips[slot];
        // An EQ with signal still in it has to keep running over silence, or its state
        // would freeze at a different point for every block size and the next note would
        // meet a different filter. Quiet sections are exactly zero (denormals are
        // flushed), so skipping them changes nothing.
        let eq_ringing = !st.eq_flat && !(st.eq_l.is_quiet() && st.eq_r.is_quiet());
        let ringing = !st.effects.is_empty() || eq_ringing;
        self.strips[slot].active = any;
        if !any && !ringing && !self.strip_lines_ringing(slot) {
            self.strip_out[slot].ran = false;
            return false; // Nothing came in and nothing is ringing.
        }
        if !any {
            self.buf_l[..n].iter_mut().for_each(|x| *x = 0.0);
            self.buf_r[..n].iter_mut().for_each(|x| *x = 0.0);
        }
        self.prof.enter(Node::Strip(slot));
        let (gl, gr, muted) = self.strip_chain(slot, p0, n, position, law);
        self.strip_outputs(slot, p0, n, gl, gr, muted);
        true
    }
}

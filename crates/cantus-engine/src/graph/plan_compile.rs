//! [`GraphPlan::compile_into`]: the snapshot's routing, resolved into nodes, canonical
//! input lists and dependency edges, exactly as the serial engine routed it.

use super::*;

impl GraphPlan {
    /// Compile `snap` into this plan, reusing its memory: allocation-free when the plan was
    /// made by [`GraphPlan::new`] (every array was given the largest size it can need).
    pub fn compile_into(&mut self, snap: &Snapshot) {
        self.key = topology_key(snap);
        let tracks = snap.tracks.len().min(MAX_TRACKS);
        let strips = snap.strips.len().min(MAX_STRIPS);
        self.tracks = tracks;
        self.strips = strips;
        self.refused_tracks = snap.tracks.len() - tracks;
        self.refused_strips = snap.strips.len() - strips;
        let sc = &mut self.scratch;

        // The order: each strip the engine holds, once.
        sc.pos.clear();
        sc.pos.resize(strips, NONE);
        self.order.clear();
        for &si in &snap.strip_order {
            if si < strips && sc.pos[si] == NONE {
                sc.pos[si] = self.order.len() as u16;
                self.order.push(si as u16);
            }
        }
        // Nodes: the tracks, the strips in order, the master.
        self.nodes.clear();
        self.nodes
            .extend((0..tracks).map(|t| NodeRef::Track(t as u16)));
        self.nodes
            .extend(self.order.iter().map(|s| NodeRef::Strip(*s)));
        self.nodes.push(NodeRef::Master);
        let count = self.nodes.len();
        let master = (count - 1) as u16;
        let pos = &sc.pos;
        // A strip's node, or NONE when it is not in the order (never processed).
        let node_of = |si: usize| -> u16 {
            match pos.get(si) {
                Some(&p) if p != NONE => tracks as u16 + p,
                _ => NONE,
            }
        };

        // Every audio source in canonical order, and the node it reaches (NONE: lost).
        sc.src.clear();
        sc.dest.clear();
        for (ti, t) in snap.tracks.iter().take(tracks).enumerate() {
            // A strip the engine does not hold resolves to none: the master, as before.
            let d = match t.strip {
                Some(si) if si < strips => node_of(si),
                _ => master,
            };
            sc.src.push(Input::Track(ti as u16));
            sc.dest.push(d);
        }
        for (place, &si) in self.order.iter().enumerate() {
            let si = si as usize;
            let st = &snap.strips[si];
            let me = tracks as u16 + place as u16;
            // Into a strip processed after this one, or lost.
            let later = |d: u16| if d != NONE && d > me { d } else { NONE };
            let out = match st.output {
                Some(o) if o < strips => later(node_of(o)),
                _ => master,
            };
            sc.src.push(Input::Out(si as u16));
            sc.dest.push(out);
            let mut k = 0usize;
            for (to, _) in &st.sends {
                if k >= MAX_SENDS_PER_STRIP {
                    break;
                }
                if *to < strips && *to != si {
                    sc.src.push(Input::Send(si as u16, k as u8));
                    sc.dest.push(later(node_of(*to)));
                    k += 1;
                }
            }
        }
        // A stable counting sort by destination: every node's inputs keep the canonical
        // order of the sources.
        self.input_start.clear();
        self.input_start.resize(count + 1, 0);
        for &d in &sc.dest {
            if d != NONE {
                self.input_start[d as usize + 1] += 1;
            }
        }
        for n in 0..count {
            self.input_start[n + 1] += self.input_start[n];
        }
        sc.fill.clear();
        sc.fill.extend_from_slice(&self.input_start[..count]);
        self.inputs.clear();
        self.inputs
            .resize(self.input_start[count] as usize, Input::Track(0));
        for (s, &d) in sc.src.iter().zip(&sc.dest) {
            if d != NONE {
                let at = &mut sc.fill[d as usize];
                self.inputs[*at as usize] = *s;
                *at += 1;
            }
        }

        // Dependencies: every input, plus each insert's key strip and note track.
        sc.strip_ids.clear();
        for (si, st) in snap.strips.iter().take(strips).enumerate() {
            sc.strip_ids.insert(st.id.0, si as u16);
        }
        sc.track_ids.clear();
        for (ti, t) in snap.tracks.iter().take(tracks).enumerate() {
            sc.track_ids.insert(t.id.0, ti as u16);
        }
        sc.edges.clear();
        sc.stamp.clear();
        sc.stamp.resize(count, NONE);
        for n in 0..count {
            let to = n as u16;
            let edge = |from: u16, stamp: &mut Vec<u16>, edges: &mut Vec<(u16, u16)>| {
                if from != NONE && from < to && stamp[from as usize] != to {
                    stamp[from as usize] = to;
                    edges.push((from, to));
                }
            };
            for i in &self.inputs[self.input_start[n] as usize..self.input_start[n + 1] as usize]
            {
                let from = match *i {
                    Input::Track(t) => t,
                    Input::Out(s) | Input::Send(s, _) => node_of(s as usize),
                };
                edge(from, &mut sc.stamp, &mut sc.edges);
            }
            let effects = match self.nodes[n] {
                NodeRef::Track(_) => continue,
                NodeRef::Strip(s) => &snap.strips[s as usize].effects,
                NodeRef::Master => &snap.master_effects,
            };
            for e in effects.iter().take(MAX_EFFECTS_PER_TRACK) {
                // A key from a strip processed later is silence, not a dependency.
                if let Some(k) = e.sidechain.and_then(|id| sc.strip_ids.get(id.0)) {
                    edge(node_of(k as usize), &mut sc.stamp, &mut sc.edges);
                }
                if let Some(t) = e.midi_from.and_then(|id| sc.track_ids.get(id.0)) {
                    edge(t, &mut sc.stamp, &mut sc.edges);
                }
            }
        }
        // Successor lists by a counting sort on the source; predecessor counts.
        self.succ_start.clear();
        self.succ_start.resize(count + 1, 0);
        self.preds.clear();
        self.preds.resize(count, 0);
        for &(from, to) in &sc.edges {
            self.succ_start[from as usize + 1] += 1;
            self.preds[to as usize] += 1;
        }
        for n in 0..count {
            self.succ_start[n + 1] += self.succ_start[n];
        }
        sc.fill.clear();
        sc.fill.extend_from_slice(&self.succ_start[..count]);
        self.succs.clear();
        self.succs.resize(sc.edges.len(), 0);
        for &(from, to) in &sc.edges {
            let at = &mut sc.fill[from as usize];
            self.succs[*at as usize] = to as u32;
            *at += 1;
        }
    }
}

#![allow(clippy::type_complexity, clippy::needless_range_loop)] // a reference model, written for clarity
//! The canonical gather (ADR 0031 §1.3) against the serial engine's scatter, bit for bit,
//! on random topologies.
//!
//! The serial engine before the graph added each source into its destination's buffer as
//! the source finished — tracks in snapshot order, then strips in `strip_order`, each its
//! output and then its sends — and a strip read its buffer when its turn came. The
//! reference below is that scatter, rule for rule (a strip past the limit resolved to the
//! master, a send into itself or past the sixteenth accepted dropped, a contribution into a
//! strip already mixed lost, a muted strip's undelayed sends silent, a delayed edge's line
//! added at unity). The graph instead compiles a plan (`graph::compile`) and has each node
//! *gather* its inputs in the plan's order (`graph::gather`). On random topologies —
//! tracks, strips, outputs, sends, keys, `midi_from`, mutes, delayed edges, strips that
//! rest, a mono device — every node's gathered input must equal what the scatter left in
//! its buffer, to the bit (`to_bits`, so −0.0 and +0.0 differ too).
//!
//! Random but reproducible: each case is a seed; a failure prints it.

use cantus_core::*;
use cantus_engine::graph::gather::{gather_interleaved, gather_stereo, Contribution};
use cantus_engine::graph::{compile, topology_key, GraphPlan, Input, NodeRef};
use cantus_engine::{MAX_SENDS_PER_STRIP, MAX_STRIPS};

/// SplitMix64: small, seedable, the same everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    /// A sample: mostly ordinary values, sometimes the ones that test a sum's sign and
    /// rounding — ±0, tiny, huge, subnormal.
    fn sample(&mut self) -> f32 {
        match self.below(12) {
            0 => 0.0,
            1 => -0.0,
            2 => f32::from_bits(1 + self.below(1000) as u32), // subnormal
            3 => -1.0e-30,
            4 => 3.0e30,
            _ => (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0,
        }
    }

    fn buffer(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.sample()).collect()
    }

    fn gain(&mut self) -> f32 {
        match self.below(6) {
            0 => 0.0,
            1 => 1.0,
            _ => (self.next() >> 40) as f32 / (1u64 << 24) as f32 * 1.7,
        }
    }
}

/// A random snapshot: `t` tracks and `s` strips, routed at random — including routes the
/// application never makes (into a strip past the end, into itself, into a strip earlier
/// in the order, an order that leaves strips out).
fn topology(rng: &mut Rng, t: usize, s: usize) -> Snapshot {
    let mut p = Project::new("graph order");
    let gate = p.make_effect(EffectKind::Gate);
    let track = p.make_track("t");
    p.tracks.push(track);
    let mut snap = Snapshot::compile(&p, 1);
    let strip = snap.strips[0].clone();
    let proto = snap.tracks[0].clone();
    let dest = |rng: &mut Rng| -> Option<usize> {
        match rng.below(10) {
            0 => None,
            1 => Some(s + rng.below(3)), // past the end: the master, as the engine did
            _ => Some(rng.below(s.max(1))),
        }
    };
    snap.tracks = (0..t)
        .map(|i| {
            let mut x = proto.clone();
            x.id = TrackId(10_000 + i as u64);
            x.strip = if s == 0 { None } else { dest(rng) };
            x
        })
        .collect();
    snap.strips = (0..s)
        .map(|i| {
            let mut x = strip.clone();
            x.id = StripId(20_000 + i as u64);
            x.muted = rng.chance(20);
            x.output = dest(rng);
            x.sends = (0..rng.below(4) + if rng.chance(3) { 18 } else { 0 })
                .map(|_| (rng.below(s + 1), rng.gain(), SendTap::Post))
                .collect();
            x.effects = (0..rng.below(3))
                .map(|_| {
                    let mut e = gate.clone();
                    e.sidechain = rng
                        .chance(40)
                        .then(|| StripId(20_000 + rng.below(s + 1) as u64));
                    e.midi_from = rng
                        .chance(40)
                        .then(|| TrackId(10_000 + rng.below(t + 1) as u64));
                    e
                })
                .collect();
            x
        })
        .collect();
    // The order: a random permutation, sometimes missing a strip.
    let mut order: Vec<usize> = (0..s).collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    if rng.chance(20) && !order.is_empty() {
        order.remove(rng.below(order.len()));
    }
    snap.strip_order = order;
    snap.master_effects = (0..rng.below(2))
        .map(|_| {
            let mut e = gate.clone();
            e.sidechain = rng
                .chance(50)
                .then(|| StripId(20_000 + rng.below(s + 1) as u64));
            e
        })
        .collect();
    snap
}

/// What one strip left this pass, by snapshot index.
struct StripOut {
    ran: bool,
    muted: bool,
    gl: f32,
    gr: f32,
    post: [Vec<f32>; 2],
    /// The output edge as its destination adds it (post times the gains, or its line).
    out: [Vec<f32>; 2],
    out_line: bool,
    /// Per accepted send: its target, level and (when delayed) its line's output.
    sends: Vec<(usize, f32, Option<[Vec<f32>; 2]>)>,
}

/// Every producer's output for one pass of `n` frames.
struct Produced {
    tracks: Vec<Option<[Vec<f32>; 2]>>,
    strips: Vec<StripOut>,
}

fn produce(rng: &mut Rng, snap: &Snapshot, n: usize) -> Produced {
    let s = snap.strips.len().min(MAX_STRIPS);
    let tracks = snap
        .tracks
        .iter()
        .map(|_| (!rng.chance(25)).then(|| [rng.buffer(n), rng.buffer(n)]))
        .collect();
    let strips = snap
        .strips
        .iter()
        .enumerate()
        .map(|(si, st)| {
            let (gl, gr) = if st.muted {
                (0.0, 0.0)
            } else {
                (rng.gain(), rng.gain())
            };
            let post = [rng.buffer(n), rng.buffer(n)];
            let out_line = rng.chance(25);
            let out = if out_line {
                [rng.buffer(n), rng.buffer(n)]
            } else {
                [
                    post[0].iter().map(|x| x * gl).collect(),
                    post[1].iter().map(|x| x * gr).collect(),
                ]
            };
            let sends = st
                .sends
                .iter()
                .filter(|(to, _, _)| *to < s && *to != si)
                .take(MAX_SENDS_PER_STRIP)
                .map(|(to, level, _)| {
                    (
                        *to,
                        *level,
                        rng.chance(25).then(|| [rng.buffer(n), rng.buffer(n)]),
                    )
                })
                .collect();
            StripOut {
                ran: !rng.chance(15),
                muted: st.muted,
                gl,
                gr,
                post,
                out,
                out_line,
                sends,
            }
        })
        .collect();
    Produced { tracks, strips }
}

fn add(buf: &mut [Vec<f32>; 2], l: &[f32], r: &[f32]) {
    for i in 0..l.len() {
        buf[0][i] += l[i];
        buf[1][i] += r[i];
    }
}

fn add_master(master: &mut [f32], channels: usize, l: &[f32], r: &[f32]) {
    for i in 0..l.len() {
        let frame = &mut master[i * channels..(i + 1) * channels];
        frame[0] += l[i];
        if channels > 1 {
            frame[1] += r[i];
        } else {
            frame[0] += r[i];
        }
    }
}

/// The serial engine's scatter, rule for rule: what each strip in the order found in its
/// buffer when its turn came, and what the master received.
fn scatter(
    snap: &Snapshot,
    p: &Produced,
    n: usize,
    channels: usize,
) -> (Vec<Option<[Vec<f32>; 2]>>, Vec<f32>) {
    let s = snap.strips.len().min(MAX_STRIPS);
    let t = snap.tracks.len().min(cantus_engine::MAX_TRACKS);
    let mut bufs: Vec<[Vec<f32>; 2]> = (0..s).map(|_| [vec![0.0; n], vec![0.0; n]]).collect();
    let mut seen: Vec<Option<[Vec<f32>; 2]>> = (0..s).map(|_| None).collect();
    let mut master = vec![0.0f32; n * channels];
    for (ti, track) in snap.tracks.iter().take(t).enumerate() {
        let Some([l, r]) = &p.tracks[ti] else {
            continue;
        };
        match track.strip {
            Some(si) if si < s => add(&mut bufs[si], l, r),
            _ => add_master(&mut master, channels, l, r),
        }
    }
    for &si in snap.strip_order.iter().filter(|si| **si < s) {
        seen[si] = Some(bufs[si].clone());
        let o = &p.strips[si];
        if !o.ran {
            continue;
        }
        // The output edge: through its line at unity, or the post-chain pair at the gains.
        let (src, gl, gr) = if o.out_line {
            (&o.out, 1.0f32, 1.0f32)
        } else {
            (&o.post, o.gl, o.gr)
        };
        let sl: Vec<f32> = src[0].iter().map(|x| x * gl).collect();
        let sr: Vec<f32> = src[1].iter().map(|x| x * gr).collect();
        match snap.strips[si].output {
            Some(d) if d < s => add(&mut bufs[d], &sl, &sr),
            _ => add_master(&mut master, channels, &sl, &sr),
        }
        // Post-fader sends: a delayed one through its line, added at unity; a muted strip's
        // undelayed sends nothing.
        if !o.muted || o.sends.iter().any(|x| x.2.is_some()) {
            for (to, level, line) in &o.sends {
                if let Some([l, r]) = line {
                    add(&mut bufs[*to], l, r);
                    continue;
                }
                if o.muted {
                    continue;
                }
                let (gl, gr) = (o.gl * level, o.gr * level);
                for i in 0..n {
                    bufs[*to][0][i] += o.post[0][i] * gl;
                    bufs[*to][1][i] += o.post[1][i] * gr;
                }
            }
        }
    }
    (seen, master)
}

/// The graph's way: one input as its producer left it.
fn contribution<'a>(p: &'a Produced, i: Input) -> Option<Contribution<'a>> {
    match i {
        Input::Track(t) => p.tracks[t as usize]
            .as_ref()
            .map(|[l, r]| Contribution::Plain(l, r)),
        Input::Out(s) => {
            let o = &p.strips[s as usize];
            o.ran.then(|| Contribution::Plain(&o.out[0], &o.out[1]))
        }
        Input::Send(s, k) => {
            let o = &p.strips[s as usize];
            if !o.ran {
                return None;
            }
            let (_, level, line) = &o.sends[k as usize];
            if let Some([l, r]) = line {
                return Some(Contribution::Plain(l, r));
            }
            if o.muted {
                return None;
            }
            Some(Contribution::Scaled(
                &o.post[0],
                &o.post[1],
                o.gl * level,
                o.gr * level,
            ))
        }
    }
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// One random case: every strip's gather equals what the scatter left in its buffer, and
/// the master's equals what the scatter added to the device's buffer.
fn check(seed: u64) {
    let mut rng = Rng(seed);
    let t = rng.below(12);
    let s = rng.below(12);
    let n = 1 + rng.below(40);
    let channels = if rng.chance(25) { 1 } else { 2 + rng.below(2) };
    let snap = topology(&mut rng, t, s);
    let plan = compile(&snap);
    let produced = produce(&mut rng, &snap, n);
    let (seen, master) = scatter(&snap, &produced, n, channels);

    for (node, r) in plan.nodes().iter().enumerate() {
        match *r {
            NodeRef::Track(_) => assert!(plan.inputs(node).is_empty(), "seed {seed}"),
            NodeRef::Strip(si) => {
                let mut acc = [vec![f32::NAN; n], vec![f32::NAN; n]];
                let [al, ar] = &mut acc;
                let any = gather_stereo(
                    al,
                    ar,
                    plan.inputs(node)
                        .iter()
                        .filter_map(|i| contribution(&produced, *i)),
                );
                if !any {
                    acc = [vec![0.0; n], vec![0.0; n]];
                }
                let want = seen[si as usize].as_ref().expect("a strip in the order");
                assert_eq!(
                    bits(&acc[0]),
                    bits(&want[0]),
                    "seed {seed}: strip {si}, left"
                );
                assert_eq!(
                    bits(&acc[1]),
                    bits(&want[1]),
                    "seed {seed}: strip {si}, right"
                );
            }
            NodeRef::Master => {
                let mut out = vec![0.0f32; n * channels];
                gather_interleaved(
                    &mut out,
                    channels,
                    plan.inputs(node)
                        .iter()
                        .filter_map(|i| contribution(&produced, *i)),
                );
                assert_eq!(
                    bits(&out),
                    bits(&master),
                    "seed {seed}: master, {channels} ch"
                );
            }
        }
    }
    check_dag(&plan, seed);
}

/// The plan's dependencies: every edge runs forward in the serial order, the predecessor
/// counts match the successor lists, every audio input's producer is a predecessor.
fn check_dag(plan: &GraphPlan, seed: u64) {
    let count = plan.nodes().len();
    let mut preds = vec![0u32; count];
    for n in 0..count {
        for &s in plan.successors(n) {
            assert!(
                (s as usize) > n,
                "seed {seed}: edge {n} → {s} runs backwards"
            );
            preds[s as usize] += 1;
        }
    }
    for (n, p) in preds.iter().enumerate() {
        assert_eq!(
            *p,
            plan.predecessors(n),
            "seed {seed}: node {n}'s predecessors"
        );
    }
    let node_of = |r: NodeRef| plan.nodes().iter().position(|x| *x == r).unwrap();
    for n in 0..count {
        for i in plan.inputs(n) {
            let from = match *i {
                Input::Track(t) => node_of(NodeRef::Track(t)),
                Input::Out(s) | Input::Send(s, _) => node_of(NodeRef::Strip(s)),
            };
            assert!(
                plan.successors(from).contains(&(n as u32)),
                "seed {seed}: input {i:?} of node {n} is not a dependency"
            );
        }
    }
    assert_eq!(plan.nodes()[plan.master()], NodeRef::Master);
}

#[test]
fn the_gather_equals_the_serial_scatter_on_random_topologies() {
    for seed in 0..3000u64 {
        check(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0xC0FFEE);
    }
}

/// Larger graphs, fewer of them: up to a hundred tracks and sixty strips.
#[test]
fn the_gather_equals_the_serial_scatter_on_large_topologies() {
    for seed in 0..40u64 {
        let mut rng = Rng(seed ^ 0xBADC0DE);
        let (t, s) = (1 + rng.below(100), 1 + rng.below(60));
        let snap = topology(&mut rng, t, s);
        let plan = compile(&snap);
        let n = 16;
        let produced = produce(&mut rng, &snap, n);
        let (seen, master) = scatter(&snap, &produced, n, 2);
        for (node, r) in plan.nodes().iter().enumerate() {
            let inputs = plan
                .inputs(node)
                .iter()
                .filter_map(|i| contribution(&produced, *i));
            match *r {
                NodeRef::Strip(si) => {
                    let mut acc = [vec![0.0; n], vec![0.0; n]];
                    let [al, ar] = &mut acc;
                    gather_stereo(al, ar, inputs);
                    let want = seen[si as usize].as_ref().unwrap();
                    assert_eq!(bits(&acc[0]), bits(&want[0]), "seed {seed}: strip {si}");
                    assert_eq!(bits(&acc[1]), bits(&want[1]), "seed {seed}: strip {si}");
                }
                NodeRef::Master => {
                    let mut out = vec![0.0f32; n * 2];
                    gather_interleaved(&mut out, 2, inputs);
                    assert_eq!(bits(&out), bits(&master), "seed {seed}: master");
                }
                NodeRef::Track(_) => {}
            }
        }
        check_dag(&plan, seed);
    }
}

/// A plan compiled into another plan's memory is the plan compiled afresh; the key moves
/// with the routing and not with a level.
#[test]
fn compiling_in_place_gives_the_same_plan_and_the_key_is_the_routing() {
    let mut rng = Rng(7);
    let mut reused = compile(&topology(&mut rng, 9, 7));
    for seed in 0..200u64 {
        let mut rng = Rng(seed);
        let (t, s) = (rng.below(10), rng.below(10));
        let snap = topology(&mut rng, t, s);
        let fresh = compile(&snap);
        reused.compile_into(&snap);
        assert_eq!(fresh.key(), reused.key());
        assert_eq!(fresh.nodes(), reused.nodes(), "seed {seed}");
        for n in 0..fresh.nodes().len() {
            assert_eq!(fresh.inputs(n), reused.inputs(n), "seed {seed}");
            assert_eq!(fresh.successors(n), reused.successors(n), "seed {seed}");
        }
        assert_eq!(fresh.key(), topology_key(&snap));
        // Levels, gains, mutes: not the routing.
        let mut louder = snap.clone();
        for st in &mut louder.strips {
            st.gain *= 0.5;
            st.muted = !st.muted;
            for s in &mut st.sends {
                s.1 *= 0.25;
            }
        }
        assert_eq!(topology_key(&louder), topology_key(&snap), "seed {seed}");
        // A route: yes.
        if let Some(t) = louder.tracks.first_mut() {
            t.strip = match t.strip {
                Some(_) => None,
                None => Some(0),
            };
            assert_ne!(topology_key(&louder), topology_key(&snap), "seed {seed}");
        }
    }
}

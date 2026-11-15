//! Each amp's preamp against an independent circuit analysis: the whole schematic — no
//! split, every plate loaded by what it really drives, supplies ideal — typed again from
//! the model's component table and solved as SPICE does: modified nodal analysis, the DC
//! operating point by Newton with the sources stepped up, then the AC response of the
//! circuit linearised there, in complex arithmetic at each frequency. Nothing is shared
//! with the DSP but the device equations (Koren's triode, Dempwolf's grid) and the values.
//!
//! The DSP is measured from its impulse response (10 µV: far below anything that clips)
//! at the default oversampled rate, 192 kHz.

#![allow(clippy::needless_range_loop)]

use super::models::AmpModel;
use super::net::{Block, Knob, Knobs, Preamp, Taper, Tone, C_GK, C_GP};
use super::tests::{gain_db, preamp_ir};
use super::SOURCE_OHMS;
use crate::fragor::circuits::POT_MIN;
use crate::fragor::dk::Koren;

#[derive(Copy, Clone, Debug)]
struct Cx(f64, f64);

impl Cx {
    fn add(self, o: Cx) -> Cx {
        Cx(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: Cx) -> Cx {
        Cx(self.0 - o.0, self.1 - o.1)
    }
    fn mul(self, o: Cx) -> Cx {
        Cx(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn div(self, o: Cx) -> Cx {
        let d = o.0 * o.0 + o.1 * o.1;
        Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn abs(self) -> f64 {
        (self.0 * self.0 + self.1 * self.1).sqrt()
    }
}

enum E {
    R(usize, usize, f64),
    C(usize, usize, f64),
    /// An ideal source `p − 0 = dc` (+ `ac` in the AC analysis).
    V(usize, f64, f64),
    /// A triode: plate, grid, cathode.
    T(usize, usize, usize, Koren),
}

struct Net {
    nodes: usize,
    el: Vec<E>,
    out: usize,
}

impl Net {
    fn node(&mut self) -> usize {
        self.nodes += 1;
        self.nodes
    }
    fn sources(&self) -> usize {
        self.el.iter().filter(|e| matches!(e, E::V(..))).count()
    }
}

fn pot_r(r: f64, taper: Taper, pos: f64) -> (f64, f64) {
    let a = taper.fraction(pos);
    (((1.0 - a) * r).max(POT_MIN), (a * r).max(POT_MIN))
}

/// The schematic, typed again from the table: one circuit, left to right.
fn netlist(blocks: &[Block], k: &Knobs) -> Net {
    let mut n = Net {
        nodes: 0,
        el: vec![],
        out: 0,
    };
    let jack = n.node();
    n.el.push(E::V(jack, 0.0, 1.0));
    let first = n.node();
    n.el.push(E::R(jack, first, SOURCE_OHMS));
    let mut cur = first;
    let mut supply = 0;
    let mut tube = None;
    for b in blocks {
        match *b {
            Block::Couple(c) => {
                let m = n.node();
                n.el.push(E::C(cur, m, c));
                cur = m;
            }
            Block::Series(r, c) => {
                let m = n.node();
                n.el.push(E::R(cur, m, r));
                if let Some(c) = c {
                    n.el.push(E::C(cur, m, c));
                }
                cur = m;
            }
            Block::Leak(r) => n.el.push(E::R(cur, 0, r)),
            Block::Pot {
                r,
                taper,
                knob,
                bright,
            } => {
                let w = n.node();
                let (up, lo) = pot_r(r, taper, k.pos(knob));
                n.el.push(E::R(cur, w, up));
                n.el.push(E::R(w, 0, lo));
                if let Some(c) = bright {
                    n.el.push(E::C(cur, w, c));
                }
                cur = w;
            }
            Block::Tone(t) => cur = tone(&mut n, cur, &t, k),
            Block::Triode(t) => {
                let (kk, p, b) = (n.node(), n.node(), n.node());
                n.el.push(E::V(b, t.bplus, 0.0));
                n.el.push(E::R(p, b, t.rp));
                n.el.push(E::R(kk, 0, t.rk));
                if let Some(c) = t.ck {
                    n.el.push(E::C(kk, 0, c));
                }
                if let Some(c) = t.cp {
                    n.el.push(E::C(p, b, c));
                }
                n.el.push(E::C(cur, kk, C_GK));
                n.el.push(E::C(cur, p, C_GP));
                n.el.push(E::T(p, cur, kk, t.tube));
                supply = b;
                tube = Some(t.tube);
                cur = p;
            }
            Block::Follower(rk) => {
                let kk = n.node();
                n.el.push(E::R(kk, 0, rk));
                n.el.push(E::C(cur, kk, C_GK));
                n.el.push(E::T(supply, cur, kk, tube.unwrap()));
                cur = kk;
            }
        }
    }
    n.out = cur;
    n
}

fn tone(n: &mut Net, i: usize, t: &Tone, k: &Knobs) -> usize {
    match *t {
        Tone::Tmb {
            slope,
            c1,
            c2,
            c3,
            treble,
            bass,
            mid,
        } => {
            let (tn, out, l, s, p, q) = (n.node(), n.node(), n.node(), n.node(), n.node(), n.node());
            let (tu, tl) = pot_r(treble.0, treble.1, k.pos(Knob::Treble));
            let (_, bl) = pot_r(bass.0, bass.1, k.pos(Knob::Bass));
            let (mu, ml) = pot_r(mid.0, mid.1, k.pos(Knob::Mid));
            n.el.push(E::C(i, tn, c1));
            n.el.push(E::R(tn, out, tu));
            n.el.push(E::R(out, l, tl));
            n.el.push(E::R(i, s, slope));
            n.el.push(E::C(s, l, c2));
            n.el.push(E::C(s, p, c3));
            n.el.push(E::R(l, q, bl));
            n.el.push(E::R(q, p, mu));
            n.el.push(E::R(p, 0, ml));
            out
        }
        Tone::Vox {
            r1,
            r2,
            c1,
            c2,
            c3,
            treble,
            bass,
        } => {
            let (a, out, x, b, y) = (n.node(), n.node(), n.node(), n.node(), n.node());
            let (tu, tl) = pot_r(treble, Taper::Log, k.pos(Knob::Treble));
            let (bu, bl) = pot_r(bass, Taper::Log, k.pos(Knob::Bass));
            n.el.push(E::C(i, a, c1));
            n.el.push(E::R(a, out, tu));
            n.el.push(E::R(out, x, tl));
            n.el.push(E::R(i, b, r1));
            n.el.push(E::C(b, x, c2));
            n.el.push(E::C(b, y, c3));
            // Turned up, the wiper leaves ground for X.
            n.el.push(E::R(y, 0, bu));
            n.el.push(E::R(x, y, bl));
            n.el.push(E::R(y, 0, r2));
            out
        }
    }
}

/// The linear stamps: conductances (and, at ω, admittances) into `m`, the sources into
/// `rhs` (their DC times `scale`, or their AC).
fn stamp(net: &Net, w: Option<f64>, m: &mut [Vec<Cx>], rhs: &mut [Cx], ac: bool, scale: f64) {
    let put = |m: &mut [Vec<Cx>], r: usize, c: usize, g: Cx| {
        if r > 0 && c > 0 {
            m[r - 1][c - 1] = m[r - 1][c - 1].add(g);
        }
    };
    let two = |m: &mut [Vec<Cx>], a: usize, b: usize, g: Cx| {
        put(m, a, a, g);
        put(m, b, b, g);
        put(m, a, b, Cx(-g.0, -g.1));
        put(m, b, a, Cx(-g.0, -g.1));
    };
    for i in 0..net.nodes {
        m[i][i] = m[i][i].add(Cx(1e-12, 0.0));
    }
    let mut br = net.nodes;
    for e in &net.el {
        match *e {
            E::R(a, b, r) => two(m, a, b, Cx(1.0 / r, 0.0)),
            E::C(a, b, c) => {
                if let Some(w) = w {
                    two(m, a, b, Cx(0.0, w * c));
                }
            }
            E::V(p, dc, acv) => {
                put(m, p, br + 1, Cx(1.0, 0.0));
                put(m, br + 1, p, Cx(1.0, 0.0));
                rhs[br] = if ac { Cx(acv, 0.0) } else { Cx(dc * scale, 0.0) };
                br += 1;
            }
            E::T(..) => {}
        }
    }
}

/// The triodes' currents leaving each node, and their conductances, at `x`.
fn triodes(net: &Net, x: &[f64], f: &mut [f64], jac: &mut [Vec<f64>]) {
    let v = |n: usize| if n == 0 { 0.0 } else { x[n - 1] };
    for e in &net.el {
        if let E::T(p, g, k, t) = *e {
            let (vgk, vpk) = (v(g) - v(k), v(p) - v(k));
            let (ig, dig, ip, gm, gp) = t.currents(vgk, vpk);
            let mut add = |r: usize, val: f64| {
                if r > 0 {
                    f[r - 1] += val;
                }
            };
            add(g, ig);
            add(p, ip);
            add(k, -(ig + ip));
            // ∂/∂(v_g, v_p, v_k) of each current.
            let dg = [(g, dig), (k, -dig)];
            let dp = [(g, gm), (p, gp), (k, -gm - gp)];
            for (row, sign, d) in [(g, 1.0, &dg[..]), (k, -1.0, &dg[..]), (p, 1.0, &dp[..]), (k, -1.0, &dp[..])] {
                for &(col, val) in d {
                    if row > 0 && col > 0 {
                        jac[row - 1][col - 1] += sign * val;
                    }
                }
            }
        }
    }
}

fn solve(m: &mut [Vec<Cx>], b: &mut [Cx]) {
    let n = b.len();
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))
            .unwrap();
        m.swap(col, piv);
        b.swap(col, piv);
        for r in col + 1..n {
            let f = m[r][col].div(m[col][col]);
            if f.abs() == 0.0 {
                continue;
            }
            for c in col..n {
                let t = f.mul(m[col][c]);
                m[r][c] = m[r][c].sub(t);
            }
            let t = f.mul(b[col]);
            b[r] = b[r].sub(t);
        }
    }
    for col in (0..n).rev() {
        let mut s = b[col];
        for c in col + 1..n {
            s = s.sub(m[col][c].mul(b[c]));
        }
        b[col] = s.div(m[col][col]);
    }
}

/// The operating point: Newton, the supplies stepped up in 40 steps.
fn dc(net: &Net) -> Vec<f64> {
    let size = net.nodes + net.sources();
    let mut x = vec![0.0; size];
    for step in 1..=40 {
        let scale = step as f64 / 40.0;
        for _ in 0..300 {
            let mut m = vec![vec![Cx(0.0, 0.0); size]; size];
            let mut rhs = vec![Cx(0.0, 0.0); size];
            stamp(net, None, &mut m, &mut rhs, false, scale);
            let mut f = vec![0.0; size];
            for r in 0..size {
                let mut s = -rhs[r].0;
                for c in 0..size {
                    s += m[r][c].0 * x[c];
                }
                f[r] = s;
            }
            let mut jac: Vec<Vec<f64>> = (0..size).map(|r| m[r].iter().map(|c| c.0).collect()).collect();
            triodes(net, &x, &mut f, &mut jac);
            let mut jm: Vec<Vec<Cx>> = jac
                .iter()
                .map(|r| r.iter().map(|g| Cx(*g, 0.0)).collect())
                .collect();
            let mut d: Vec<Cx> = f.iter().map(|v| Cx(-v, 0.0)).collect();
            solve(&mut jm, &mut d);
            let big = d.iter().take(net.nodes).map(|c| c.0.abs()).fold(0.0, f64::max);
            let k = if big > 5.0 { 5.0 / big } else { 1.0 };
            for (xi, di) in x.iter_mut().zip(&d) {
                *xi += di.0 * k;
            }
            if big < 1e-10 {
                break;
            }
        }
    }
    x
}

/// |H(j2πf)| from the input to the output, linearised at `op`, in dB.
fn ac_db(net: &Net, op: &[f64], hz: f64) -> f64 {
    let size = net.nodes + net.sources();
    let mut m = vec![vec![Cx(0.0, 0.0); size]; size];
    let mut rhs = vec![Cx(0.0, 0.0); size];
    stamp(net, Some(std::f64::consts::TAU * hz), &mut m, &mut rhs, true, 1.0);
    let mut f = vec![0.0; size];
    let mut jac = vec![vec![0.0; size]; size];
    triodes(net, op, &mut f, &mut jac);
    for r in 0..size {
        for c in 0..size {
            m[r][c] = m[r][c].add(Cx(jac[r][c], 0.0));
        }
    }
    solve(&mut m, &mut rhs);
    20.0 * crate::math::log10(rhs[net.out - 1].abs())
}

const FREQS: [f64; 8] = [80.0, 200.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 6_000.0, 8_000.0];

/// One model at some knobs: the DSP's small-signal response against the schematic's —
/// the worst difference to 4 kHz, and at 6 and 8 kHz (where the split's one
/// approximation shows: a plate's Miller current through its grid–plate capacitance is
/// computed with the plate unloaded by the next circuit).
fn compare(m: AmpModel, k: Knobs) -> (f64, f64) {
    let rate = 192_000.0;
    let net = netlist(m.preamp(), &k);
    let op = dc(&net);
    let mut p = Preamp::new(m.preamp(), SOURCE_OHMS, rate, &k);
    let (h, amp) = preamp_ir(&mut p, rate as usize);
    let (mut low, mut high) = (0.0f64, 0.0f64);
    let mut line = String::new();
    for hz in FREQS {
        let ours = gain_db(&h, amp, rate, hz);
        let theirs = ac_db(&net, &op, hz);
        let d = (ours - theirs).abs();
        if hz <= 4_000.0 {
            low = low.max(d);
        } else {
            high = high.max(d);
        }
        line += &format!(" {hz}: {ours:.2}/{theirs:.2}");
    }
    println!("{m:?} {k:?}: worst {low:.3} dB to 4 kHz, {high:.3} dB above —{line}");
    (low, high)
}

#[test]
fn every_preamp_matches_its_schematic() {
    for m in AmpModel::ALL {
        for k in [
            Knobs::default(),
            Knobs {
                gain: 0.9,
                bass: 0.2,
                mid: 0.8,
                treble: 0.7,
            },
        ] {
            let (low, high) = compare(m, k);
            assert!(low < 0.25 && high < 1.0, "{m:?}: {low:.2} dB, {high:.2} dB");
        }
    }
}

/// The operating points: every plate of the DSP's circuits against the schematic's.
#[test]
fn the_operating_points_match_the_schematic() {
    for m in AmpModel::ALL {
        let k = Knobs::default();
        let net = netlist(m.preamp(), &k);
        let op = dc(&net);
        let mut theirs = vec![];
        for e in &net.el {
            if let E::T(p, _, kk, _) = *e {
                theirs.push(op[p - 1] - op[kk - 1]);
            }
        }
        let pre = Preamp::new(m.preamp(), SOURCE_OHMS, 192_000.0, &k);
        let mut ours = vec![];
        for s in pre.stages() {
            for pair in s.c.ports().chunks(2) {
                ours.push(pair[1]);
            }
        }
        println!("{m:?}: v_pk ours {ours:.1?}, schematic {theirs:.1?}");
        assert_eq!(ours.len(), theirs.len());
        for (a, b) in ours.iter().zip(&theirs) {
            assert!((a - b).abs() < 0.05, "{m:?}: {a} against {b}");
        }
    }
}

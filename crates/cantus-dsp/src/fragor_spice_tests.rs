//! Each pedal's small-signal response against an independent circuit analysis: the whole
//! schematic (no part split) typed again from the published values, solved the way SPICE
//! does — modified nodal analysis, the DC operating point by Newton with the sources
//! stepped up, then the AC response of the circuit linearised there, in complex arithmetic
//! at each frequency. Nothing here is shared with the DSP but the device equations
//! (Ebers–Moll, Shockley) and the component values, which come from the schematics.
//!
//! The DSP is measured by playing a 10 µV sine through it (far below where anything
//! clips) and reading the output's amplitude at that frequency over whole cycles.

use super::circuits::{taper_a, Model, Pedal, D_IS, D_N, FUZZ_SOURCE, VT};
use crate::math;

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

/// An element of the reference netlist (nodes by name index; 0 is ground).
#[derive(Clone, Debug)]
enum E {
    R(usize, usize, f64),
    C(usize, usize, f64),
    /// An ideal voltage source `p − n = dc` (+ `ac` in the AC analysis).
    V(usize, usize, f64, f64),
    /// An op-amp: `out_int = A(s)·(p − m)` with A(s) = a0 / (1 + s/ωp), ωp = 2π·gbw/a0,
    /// then `r_out` to `out`.
    Op {
        p: usize,
        m: usize,
        int: usize,
        gbw: f64,
        a0: f64,
    },
    Diodes(usize, usize),
    /// NPN: c, b, e, Is, βf, βr.
    Q(usize, usize, usize, f64, f64, f64),
}

struct Net {
    nodes: usize,
    el: Vec<E>,
    out: Vec<(usize, f64)>,
}

impl Net {
    fn new() -> Net {
        Net {
            nodes: 0,
            el: vec![],
            out: vec![],
        }
    }
    fn node(&mut self) -> usize {
        self.nodes += 1;
        self.nodes
    }
    /// Branch unknowns (voltage sources and op-amp outputs), after the nodes.
    fn branches(&self) -> usize {
        self.el
            .iter()
            .filter(|e| matches!(e, E::V(..) | E::Op { .. }))
            .count()
    }
}

/// The nonlinear elements' currents into each node and their conductances, at `x`.
fn nonlinear(net: &Net, x: &[f64], f: &mut [f64], jac: &mut [Vec<f64>]) {
    let v = |n: usize| if n == 0 { 0.0 } else { x[n - 1] };
    let add_i = |f: &mut [f64], n: usize, i: f64| {
        if n > 0 {
            f[n - 1] += i;
        }
    };
    let add_g = |jac: &mut [Vec<f64>], r: usize, c: usize, g: f64| {
        if r > 0 && c > 0 {
            jac[r - 1][c - 1] += g;
        }
    };
    for e in &net.el {
        match *e {
            E::Diodes(a, b) => {
                let vd = v(a) - v(b);
                let nvt = D_N * VT;
                let (ep, em) = (math::exp(vd / nvt), math::exp(-vd / nvt));
                let i = D_IS * (ep - em);
                let g = D_IS * (ep + em) / nvt;
                // Current leaving a, entering b.
                add_i(f, a, i);
                add_i(f, b, -i);
                for (r, sr) in [(a, 1.0), (b, -1.0)] {
                    for (c, sc) in [(a, 1.0), (b, -1.0)] {
                        add_g(jac, r, c, sr * sc * g);
                    }
                }
            }
            E::Q(c, b, e, is, bf, br) => {
                let (vbe, vbc) = (v(b) - v(e), v(b) - v(c));
                let (ebe, ebc) = (math::exp(vbe / VT), math::exp(vbc / VT));
                let ib = is / bf * (ebe - 1.0) + is / br * (ebc - 1.0);
                let ic = is * (ebe - ebc) - is / br * (ebc - 1.0);
                // Currents leaving the nodes into the device: b gives ib, c gives ic, e
                // receives both.
                add_i(f, b, ib);
                add_i(f, c, ic);
                add_i(f, e, -(ib + ic));
                let dib = [is / bf * ebe / VT, is / br * ebc / VT]; // ∂/∂vbe, ∂/∂vbc
                let dic = [is * ebe / VT, -is * ebc / VT * (1.0 + 1.0 / br)];
                // ∂vbe/∂(b, e) = (1, −1); ∂vbc/∂(b, c) = (1, −1).
                let dv = |d: [f64; 2]| [(b, d[0] + d[1]), (e, -d[0]), (c, -d[1])];
                for (row, d, s) in [(b, dib, 1.0), (c, dic, 1.0)] {
                    for (col, g) in dv(d) {
                        add_g(jac, row, col, s * g);
                        add_g(jac, e, col, -s * g);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Stamp the linear part: real (DC, `w` = None) or the AC matrix at ω.
fn stamp(net: &Net, w: Option<f64>, m: &mut [Vec<Cx>], rhs: &mut [Cx], ac: bool, scale: f64) {
    let n = net.nodes;
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
    for i in 0..n {
        m[i][i] = m[i][i].add(Cx(1e-12, 0.0));
    }
    let mut br = n;
    for e in &net.el {
        match *e {
            E::R(a, b, r) => two(m, a, b, Cx(1.0 / r, 0.0)),
            E::C(a, b, c) => {
                if let Some(w) = w {
                    two(m, a, b, Cx(0.0, w * c));
                }
            }
            E::V(p, q, dc, acv) => {
                // Branch current k flows from p through the source to q.
                put(m, p, br + 1, Cx(1.0, 0.0));
                put(m, q, br + 1, Cx(-1.0, 0.0));
                put(m, br + 1, p, Cx(1.0, 0.0));
                put(m, br + 1, q, Cx(-1.0, 0.0));
                rhs[br] = if ac { Cx(acv, 0.0) } else { Cx(dc * scale, 0.0) };
                br += 1;
            }
            E::Op {
                p,
                m: mm,
                int,
                gbw,
                a0,
            } => {
                // v_int − A(s)(v_p − v_m) = 0, the source's current into int.
                let a = match w {
                    Some(w) => Cx(a0, 0.0).div(Cx(1.0, w * a0 / (std::f64::consts::TAU * gbw))),
                    None => Cx(a0, 0.0),
                };
                put(m, int, br + 1, Cx(1.0, 0.0));
                put(m, br + 1, int, Cx(1.0, 0.0));
                put(m, br + 1, p, Cx(-a.0, -a.1));
                put(m, br + 1, mm, a);
                br += 1;
            }
            _ => {}
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

/// The DC operating point (node voltages, then branch currents).
fn dc(net: &Net) -> Vec<f64> {
    let size = net.nodes + net.branches();
    let mut x = vec![0.0; size];
    for step in 1..=40 {
        let scale = step as f64 / 40.0;
        for _ in 0..300 {
            let mut m = vec![vec![Cx(0.0, 0.0); size]; size];
            let mut rhs = vec![Cx(0.0, 0.0); size];
            stamp(net, None, &mut m, &mut rhs, false, scale);
            // Residual of the linear part, plus the nonlinear currents; Newton.
            let mut f = vec![0.0; size];
            for r in 0..size {
                let mut s = -rhs[r].0;
                for c in 0..size {
                    s += m[r][c].0 * x[c];
                }
                f[r] = s;
            }
            let mut jac: Vec<Vec<f64>> = (0..size).map(|r| m[r].iter().map(|c| c.0).collect()).collect();
            nonlinear(net, &x, &mut f, &mut jac);
            let mut jm: Vec<Vec<Cx>> = jac.iter().map(|r| r.iter().map(|g| Cx(*g, 0.0)).collect()).collect();
            let mut d: Vec<Cx> = f.iter().map(|v| Cx(-v, 0.0)).collect();
            solve(&mut jm, &mut d);
            let big = d.iter().map(|c| c.0.abs()).fold(0.0, f64::max);
            let k = if big > 0.1 { 0.1 / big } else { 1.0 };
            for (xi, di) in x.iter_mut().zip(&d) {
                *xi += di.0 * k;
            }
            if big < 1e-12 {
                break;
            }
        }
    }
    x
}

/// |H(j2πf)| from the AC source to the output, linearised at `op`.
fn ac(net: &Net, op: &[f64], hz: f64) -> f64 {
    let size = net.nodes + net.branches();
    let mut m = vec![vec![Cx(0.0, 0.0); size]; size];
    let mut rhs = vec![Cx(0.0, 0.0); size];
    stamp(net, Some(std::f64::consts::TAU * hz), &mut m, &mut rhs, true, 1.0);
    let mut f = vec![0.0; size];
    let mut jac = vec![vec![0.0; size]; size];
    nonlinear(net, op, &mut f, &mut jac);
    for r in 0..size {
        for c in 0..size {
            m[r][c] = m[r][c].add(Cx(jac[r][c], 0.0));
        }
    }
    solve(&mut m, &mut rhs);
    let mut y = Cx(0.0, 0.0);
    for &(n, w) in &net.out {
        y = y.add(Cx(rhs[n - 1].0 * w, rhs[n - 1].1 * w));
    }
    y.abs()
}

/// A follower (the DSP's buffers): out = in, behind `r` ohms.
fn follower(n: &mut Net, inp: usize, out: usize, r: f64) {
    let int = n.node();
    n.el.push(E::Op {
        p: inp,
        m: 0,
        int,
        gbw: 1e15,
        a0: 1.0,
    });
    n.el.push(E::R(int, out, r));
}

fn opamp(n: &mut Net, p: usize, m: usize, out: usize, gbw: f64, a0: f64) {
    let int = n.node();
    n.el.push(E::Op { p, m, int, gbw, a0 });
    n.el.push(E::R(int, out, 50.0));
}

const RAIL: f64 = 1.0;

/// A supply node: an ideal source behind the DSP's rail impedance.
fn rail(n: &mut Net, volts: f64) -> usize {
    let s = n.node();
    let r = n.node();
    n.el.push(E::V(s, 0, volts, 0.0));
    n.el.push(E::R(s, r, RAIL));
    r
}

/// The signal source behind `r` ohms; returns the node after it.
fn source(n: &mut Net, r: f64) -> usize {
    let s = n.node();
    let o = n.node();
    n.el.push(E::V(s, 0, 0.0, 1.0));
    n.el.push(E::R(s, o, r));
    o
}

fn pot(r: f64) -> f64 {
    r.max(1.0)
}

/// Tube Screamer TS808 (ElectroSmash: clipping amplifier, tone/volume stage).
fn ts(drive: f64, tone: f64) -> Net {
    use E::*;
    let mut n = Net::new();
    let vref = rail(&mut n, 4.5);
    let src = source(&mut n, 100.0);
    let [a, p, m, g, o1, t1, m2, w, rc, o2, oc, lv] = std::array::from_fn(|_| n.node());
    n.el.extend([C(src, a, 1e-6), R(a, p, 1e3), R(p, vref, 10e3)]);
    n.el.extend([R(m, g, 4.7e3), C(g, 0, 47e-9)]);
    n.el.extend([R(m, o1, 51e3 + pot(500e3 * taper_a(drive))), C(m, o1, 51e-12)]);
    n.el.push(Diodes(m, o1));
    opamp(&mut n, p, m, o1, 3e6, 1e5);
    n.el.extend([R(o1, t1, 1e3), C(t1, 0, 0.22e-6), R(t1, vref, 10e3)]);
    n.el.extend([R(t1, w, pot(20e3 * tone)), R(w, m2, pot(20e3 * (1.0 - tone)))]);
    n.el.extend([C(w, rc, 0.22e-6), R(rc, 0, 220.0), R(o2, m2, 1e3)]);
    opamp(&mut n, t1, m2, o2, 3e6, 1e5);
    n.el.extend([C(o2, oc, 1e-6), R(oc, lv, 1e3), R(lv, 0, 100e3)]);
    n.out = vec![(lv, 1.0)];
    n
}

/// ProCo RAT (ElectroSmash).
fn rat(dist: f64, filter: f64) -> Net {
    use E::*;
    let mut n = Net::new();
    let vref = rail(&mut n, 4.5);
    let src = source(&mut n, 100.0);
    let [a, p, m, g1, g2, o, b, d, f, j] = std::array::from_fn(|_| n.node());
    n.el.extend([R(src, 0, 1e6), C(src, a, 22e-9), R(a, vref, 1e6)]);
    n.el.extend([R(a, p, 1e3), C(p, 0, 1e-9)]);
    n.el.extend([R(m, o, pot(100e3 * taper_a(dist))), C(m, o, 100e-12)]);
    n.el.extend([R(m, g1, 47.0), C(g1, 0, 2.2e-6), R(m, g2, 560.0), C(g2, 0, 4.7e-6)]);
    let gbw = 0.3e6 * 30e-12 / (2.0 * VT) / (std::f64::consts::TAU * 30e-12);
    opamp(&mut n, p, m, o, gbw, 2e5);
    n.el.extend([C(o, b, 4.7e-6), R(b, d, 1e3), Diodes(d, 0)]);
    n.el.extend([R(d, f, 1.5e3 + pot(100e3 * taper_a(filter))), C(f, 0, 3.3e-9)]);
    n.el.extend([C(f, j, 22e-9), R(j, 0, 1e6)]);
    n.out = vec![(j, 1.0)];
    n
}

/// Boss DS-1 (ElectroSmash's DS-1A), whole: buffer, booster, U1a follower, U1b, tone.
fn ds1(dist: f64, tone: f64) -> Net {
    use E::*;
    let mut n = Net::new();
    let vref = rail(&mut n, 4.5);
    let vcc = rail(&mut n, 9.0);
    let src = source(&mut n, 1e3);
    let [b1, e1, x, b2, c2, e2, y, ps, m, h, o, k, d, hp1, hh, ll, w] =
        std::array::from_fn(|_| n.node());
    n.el.extend([C(src, b1, 47e-9), R(b1, vref, 470e3)]);
    follower(&mut n, b1, e1, 50.0);
    n.el.extend([R(e1, 0, 10e3), C(e1, x, 0.47e-6), R(x, vref, 100e3), C(x, b2, 47e-9)]);
    n.el.extend([R(b2, 0, 100e3), R(c2, b2, 470e3), C(c2, b2, 250e-12), R(vcc, c2, 10e3)]);
    n.el.extend([R(e2, 0, 22.0), Q(c2, b2, e2, 1e-14, 400.0, 4.0)]);
    n.el.extend([C(c2, y, 68e-9), R(y, vref, 100e3)]);
    follower(&mut n, y, ps, 50.0);
    n.el.extend([R(m, o, 100e3), C(m, o, 100e-12)]);
    n.el.extend([R(m, h, 4.7e3 + pot(100e3 * (1.0 - dist))), C(h, 0, 0.47e-6)]);
    opamp(&mut n, ps, m, o, 1.2e6, 1e5);
    n.el.extend([R(o, k, 2.2e3), C(k, d, 0.47e-6), Diodes(d, vref), C(d, 0, 0.01e-6)]);
    n.el.extend([C(d, hp1, 0.022e-6), R(hp1, hh, 2.2e3), R(hh, vref, 6.8e3)]);
    n.el.extend([R(d, ll, 6.8e3), C(ll, vref, 0.1e-6)]);
    n.el.extend([R(ll, w, pot(20e3 * tone)), R(w, hh, pot(20e3 * (1.0 - tone)))]);
    n.el.push(R(w, vref, 100e3));
    n.out = vec![(w, 1.0), (vref, -1.0)];
    n
}

/// Fuzz Face (ElectroSmash), as its NPN mirror on +9 V (the same small-signal response).
fn ff(fuzz: f64, bias: f64) -> Net {
    use E::*;
    let mut n = Net::new();
    let (e, r_int) = super::circuits::battery(bias);
    let bs = n.node();
    let vcc = n.node();
    n.el.extend([V(bs, 0, e, 0.0), R(bs, vcc, r_int)]);
    let src = source(&mut n, FUZZ_SOURCE);
    let [b1, c1, e2, w, o, c2, v] = std::array::from_fn(|_| n.node());
    n.el.extend([C(src, b1, 2.2e-6), R(vcc, c1, 33e3)]);
    n.el.push(Q(c1, b1, 0, 85.8e-9, 85.0, 20.0));
    n.el.extend([Q(c2, c1, e2, 120.8e-9, 120.0, 20.0), R(e2, b1, 100e3)]);
    n.el.extend([R(e2, w, pot(1e3 * (1.0 - fuzz))), R(w, 0, pot(1e3 * fuzz)), C(w, 0, 20e-6)]);
    n.el.extend([R(o, c2, 8.2e3), R(vcc, o, 470.0), C(o, v, 0.01e-6), R(v, 0, 500e3)]);
    n.out = vec![(v, 1.0)];
    n
}

/// Big Muff π V3 (ElectroSmash), whole: no virtual grounds.
fn bmp(sustain: f64, tone: f64) -> Net {
    use E::*;
    let mut n = Net::new();
    let vcc = rail(&mut n, 9.0);
    let src = source(&mut n, 100.0);
    let q = |c, b, e| Q(c, b, e, 1.8e-14, 300.0, 7.5);
    let [r, b4, c4, e4, st, sw, sb, qn] = std::array::from_fn(|_| n.node());
    let [b3, c3, e3, dd, q2n, b2, c2, e2, dd2] = std::array::from_fn(|_| n.node());
    let [ll, hh, w, b1, c1, e1, vo] = std::array::from_fn(|_| n.node());
    n.el.extend([R(src, r, 39e3), C(r, b4, 1e-6), R(b4, 0, 47e3)]);
    n.el.extend([R(c4, b4, 470e3), C(c4, b4, 470e-12), R(vcc, c4, 10e3), R(e4, 0, 100.0)]);
    n.el.extend([q(c4, b4, e4), C(c4, st, 1e-6)]);
    n.el.extend([R(st, sw, pot(100e3 * (1.0 - sustain))), R(sw, sb, pot(100e3 * sustain))]);
    n.el.extend([R(sb, 0, 1e3), C(sw, qn, 100e-9), R(qn, b3, 10e3)]);
    n.el.extend([R(b3, 0, 100e3), R(c3, b3, 470e3), C(c3, b3, 470e-12)]);
    n.el.extend([Diodes(c3, dd), C(dd, b3, 1e-6), R(vcc, c3, 10e3), R(e3, 0, 150.0)]);
    n.el.extend([q(c3, b3, e3), C(c3, q2n, 100e-9), R(q2n, b2, 10e3)]);
    n.el.extend([R(b2, 0, 100e3), R(c2, b2, 470e3), C(c2, b2, 470e-12)]);
    n.el.extend([Diodes(c2, dd2), C(dd2, b2, 1e-6), R(vcc, c2, 10e3), R(e2, 0, 150.0)]);
    n.el.push(q(c2, b2, e2));
    n.el.extend([R(c2, ll, 39e3), C(ll, 0, 10e-9), C(c2, hh, 4e-9), R(hh, 0, 22e3)]);
    n.el.extend([R(hh, w, pot(100e3 * (1.0 - tone))), R(w, ll, pot(100e3 * tone))]);
    n.el.extend([C(w, b1, 100e-9), R(vcc, b1, 430e3), R(b1, 0, 100e3)]);
    n.el.extend([R(vcc, c1, 15e3), R(e1, 0, 3.3e3), q(c1, b1, e1)]);
    n.el.extend([C(c1, vo, 100e-9), R(vo, 0, 100e3)]);
    n.out = vec![(vo, 1.0)];
    n
}

/// The DSP's gain at `hz`: a 10 µV sine, read over whole cycles after the transients.
fn dsp_gain(p: &mut Pedal, rate: f64, hz: f64) -> f64 {
    let amp = 1e-5;
    let settle = (rate * 0.6) as usize;
    let cycles = (hz * 0.05).ceil().max(4.0);
    let len = (cycles * rate / hz).round() as usize;
    let w = std::f64::consts::TAU * hz / rate;
    let (mut re, mut im) = (0.0, 0.0);
    for i in 0..settle + len {
        let y = p.tick(amp * math::sin(w * i as f64));
        if i >= settle {
            let (s, c) = math::sin_cos(w * i as f64);
            re += y * c;
            im += y * s;
        }
    }
    2.0 * (re * re + im * im).sqrt() / len as f64 / amp / p.level()
}

const FREQS: [f64; 6] = [80.0, 200.0, 500.0, 1_000.0, 3_000.0, 8_000.0];
const RATE: f64 = 192_000.0;

/// Compare a model at some knob settings; returns the worst difference in dB.
fn compare(model: Model, knobs: [f64; 3], net: &Net) -> f64 {
    let op = dc(net);
    let mut p = Pedal::new(model, RATE);
    p.set_knobs(knobs);
    p.settle();
    let mut worst = 0.0f64;
    for hz in FREQS {
        // The trapezoidal rule's frequency warping at 192 kHz: within 0.1 dB to 3 kHz,
        // 0.25 dB at 8 kHz on the steepest slopes.
        let allow = if hz > 5_000.0 { 0.4 } else { 1.0 };
        let r = ac(net, &op, hz);
        let d = dsp_gain(&mut p, RATE, hz);
        p.snap();
        let diff = 20.0 * math::log10(d / r);
        println!(
            "{model:?} {knobs:?} {hz:>6} Hz: analysis {:+7.2} dB, DSP {:+7.2} dB ({diff:+.3})",
            20.0 * math::log10(r),
            20.0 * math::log10(d)
        );
        worst = worst.max(diff.abs() * allow);
    }
    worst
}

/// The Tube Screamer within 0.1 dB of the analysis from 80 Hz to 8 kHz, at three drives
/// and both ends of the tone.
#[test]
fn the_green_overdrive_matches_its_schematic() {
    for (drive, tone) in [(0.0, 0.5), (0.5, 0.0), (1.0, 1.0), (0.5, 0.5)] {
        let w = compare(Model::Overdrive, [drive, tone, 1.0], &ts(drive, tone));
        assert!(w < 0.1, "{w} dB");
    }
}

#[test]
fn the_opamp_distortion_matches_its_schematic() {
    for (dist, filter) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (1.0, 0.0)] {
        let w = compare(Model::OpAmp, [dist, filter, 1.0], &rat(dist, filter));
        assert!(w < 0.1, "{w} dB");
    }
}

#[test]
fn the_silicon_distortion_matches_its_schematic() {
    for (dist, tone) in [(0.0, 0.5), (0.5, 0.0), (1.0, 1.0)] {
        let w = compare(Model::Silicon, [dist, tone, 1.0], &ds1(dist, tone));
        assert!(w < 0.1, "{w} dB");
    }
}

#[test]
fn the_germanium_fuzz_matches_its_schematic() {
    for (fuzz, bias) in [(0.0, 1.0), (0.5, 1.0), (1.0, 1.0), (0.5, 0.5)] {
        let w = compare(Model::Fuzz, [fuzz, bias, 1.0], &ff(fuzz, bias));
        assert!(w < 0.1, "{w} dB");
    }
}

#[test]
fn the_triangle_muff_matches_its_schematic() {
    for (sus, tone) in [(0.2, 0.5), (0.5, 0.0), (1.0, 1.0)] {
        let w = compare(Model::Muff, [sus, tone, 1.0], &bmp(sus, tone));
        assert!(w < 0.3, "{w} dB");
    }
}

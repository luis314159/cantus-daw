//! Fragor's circuit solver: a nodal network discretised by the trapezoidal rule and reduced
//! to its nonlinear ports — the **DK method** (Yeh, Abel & Smith, "Automated physical
//! modeling of nonlinear audio circuits for real-time audio effects", IEEE TASLP 2010;
//! Holters & Zölzer, DAFx-11), solved by Newton's method each sample.
//!
//! ## What a circuit is here
//!
//! Nodes `1..=n` (0 is ground); resistors, capacitors, voltage-controlled current sources
//! (the op-amps' gain and output stages), inputs (a voltage behind a series resistance — a
//! Norton source — or a pure current), and nonlinear elements: a pair of antiparallel
//! diodes, an NPN transistor (Ebers–Moll), and an op-amp input stage whose current
//! saturates (its slew rate). Each nonlinear element has *ports*: a controlling voltage and
//! the current it draws.
//!
//! ## The method
//!
//! A capacitor becomes, per sample, a conductance `2C/T` beside a current source carrying
//! its history (the trapezoidal companion). Then the network is linear but for the ports:
//!
//! ```text
//! G x = s + Nᵢ i(v),   v = Nᵥ x          (x the node voltages, s inputs and history)
//! v = Nᵥ G⁻¹ s + Nᵥ G⁻¹ Nᵢ i(v) = p + K i(v)
//! ```
//!
//! so each sample solves only `p + K i(v) − v = 0` — one to five unknowns — by Newton,
//! starting from the last sample's answer, and then reads the node voltages it needs.
//! `G⁻¹`, `K` and the rest change only when a knob moves a resistor, on the board's update
//! grid. Everything is `f64`, in a fixed order, with [`crate::math`]'s functions: the same
//! bits on every machine. No allocation anywhere (fixed-size arrays).
//!
//! ## Rest
//!
//! [`Circuit::solve_dc`] finds the operating point with the capacitors open (Newton with
//! the supplies ramped up — source stepping — and a tiny conductance to ground on every
//! node, as SPICE does) and charges every capacitor to it: the state a freshly powered
//! pedal settles into. The board returns a pedal to that exact state after a stretch of
//! digital silence (`fragor.rs`).

use crate::math;

/// Nodes at most, ground excluded.
pub const MAX_N: usize = 16;
/// Nonlinear port dimensions at most.
pub const MAX_NL: usize = 5;
/// Capacitors at most.
pub const MAX_C: usize = 12;
/// Resistors at most.
pub const MAX_R: usize = 28;
/// Inputs at most.
pub const MAX_IN: usize = 3;
/// Controlled sources at most.
pub const MAX_G: usize = 6;
/// Nonlinear elements at most.
pub const MAX_E: usize = 4;
/// Outputs (linear readings of the node voltages) at most.
pub const MAX_OUT: usize = 2;
/// Entries of `q`: the inputs and the capacitors' history sources.
const MAX_Q: usize = MAX_IN + MAX_C;
/// Rows of the fused per-sample matrix: the ports (0 …), the capacitors' voltages (from
/// [`CAP_ROW`]), the outputs (from [`OUT_ROW`]); padded to a multiple of four so the
/// columns vectorise.
const ROWS: usize = 20;
const CAP_ROW: usize = MAX_NL;
const OUT_ROW: usize = MAX_NL + MAX_C;

/// SPICE's minimum conductance to ground, on every node, so a node joined to the rest only
/// through capacitors still has a voltage at DC.
const GMIN: f64 = 1e-12;
/// Newton's iterations per sample at most, and its tolerance (volts).
const ITER: usize = 40;
/// Newton stops once a step is below this: the next would be about its square (quadratic
/// convergence), so the answer is good to ~10⁻⁹ V; the currents are then taken to first
/// order at the new voltages.
const STOP: f64 = 1e-5;
/// The largest step a junction voltage takes in one iteration (volts): keeps Newton on the
/// exponential's good side.
const JUNCTION_STEP: f64 = 0.1;
/// The largest step an op-amp's differential input takes in one iteration.
const SLEW_STEP: f64 = 0.5;
/// Exponents are capped here (e^80 ≈ 5.5·10³⁴): a diverging iterate never overflows.
const EXP_CAP: f64 = 80.0;

/// A nonlinear element.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Nl {
    /// Two identical diodes antiparallel between `a` and `b`: `i = 2·Is·sinh(v/(n·Vt))`,
    /// flowing from `a` to `b`. One port.
    DiodePair { a: u8, b: u8, is: f64, nvt: f64 },
    /// An NPN transistor, Ebers–Moll transport model. Two ports: `(v_be, i_b)` and
    /// `(v_bc, i_c)`.
    Npn {
        c: u8,
        b: u8,
        e: u8,
        is: f64,
        bf: f64,
        br: f64,
        vt: f64,
    },
    /// An op-amp's input stage: `i = Imax·tanh(gm·(v₊ − v₋)/Imax)` into node `to` — its
    /// slew rate is `Imax / C` for the compensation capacitor `C` on that node. One port.
    Slew {
        p: u8,
        m: u8,
        to: u8,
        gm: f64,
        imax: f64,
    },
}

impl Nl {
    fn dims(&self) -> usize {
        match self {
            Nl::Npn { .. } => 2,
            _ => 1,
        }
    }
}

/// A voltage-controlled current source: `gm·(v[cp] − v[cm])` into `op`, out of `om`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Vccs {
    op: u8,
    om: u8,
    cp: u8,
    cm: u8,
    gm: f64,
}

/// A circuit and its state.
#[derive(Clone, Debug)]
pub struct Circuit {
    n: usize,
    dt: f64,
    r: [(u8, u8, f64); MAX_R],
    nr: usize,
    c: [(u8, u8, f64); MAX_C],
    nc: usize,
    g: [Vccs; MAX_G],
    ng: usize,
    /// Inputs: the node and the series conductance (0: a pure current into the node).
    inputs: [(u8, f64); MAX_IN],
    ni: usize,
    e: [Nl; MAX_E],
    ne: usize,
    dims: usize,
    /// Each port's block of ports (an NPN's two share one): `[lo, hi)`.
    block: [(u8, u8); MAX_NL],
    outs: [[f64; MAX_N]; MAX_OUT],
    nout: usize,
    // ---- derived from the values (rebuilt when one moves) ----
    ginv: [[f64; MAX_N]; MAX_N],
    /// G⁻¹ S: each node's share of each entry of `q = [inputs, capacitor histories]`.
    tq: [[f64; MAX_Q]; MAX_N],
    /// G⁻¹ Nᵢ: each node's share of each port current.
    gn: [[f64; MAX_NL]; MAX_N],
    /// Nᵥ G⁻¹ Nᵢ.
    k: [[f64; MAX_NL]; MAX_NL],
    /// Nᵥ G⁻¹ S: the ports' voltages before their own currents.
    pq: [[f64; MAX_Q]; MAX_NL],
    /// Each capacitor's voltage, from `q` and from the port currents.
    vq: [[f64; MAX_Q]; MAX_C],
    vi: [[f64; MAX_NL]; MAX_C],
    /// Each output, likewise.
    oq: [[f64; MAX_Q]; MAX_OUT],
    oi: [[f64; MAX_NL]; MAX_OUT],
    /// The same, column by column, fused: `aq[j]` is entry `j` of `q`'s share of every
    /// port, capacitor and output row; `ai[d]` port current `d`'s. Each sample is two
    /// passes of independent multiply-adds over whole columns (no chain of dependent
    /// additions: that latency, not the arithmetic, was the cost).
    aq: [[f64; ROWS]; MAX_Q],
    ai: [[f64; ROWS]; MAX_NL],
    /// Capacitor companions' conductances, 2C/T (0 when solving DC).
    gc: [f64; MAX_C],
    // ---- state ----
    /// Each capacitor's history source, (2C/T)·v_c + i_c of the last sample.
    j: [f64; MAX_C],
    /// The ports' voltages at the last two samples.
    v: [f64; MAX_NL],
    v1: [f64; MAX_NL],
    /// The last sample's `q` and port currents (what the node voltages are read from).
    q_last: [f64; MAX_Q],
    i_last: [f64; MAX_NL],
    /// The outputs at the operating point (subtracted from what [`Self::step`] returns).
    y0: [f64; MAX_OUT],
}

impl Circuit {
    /// An empty circuit of `n` nodes sampled every `dt` seconds.
    pub fn new(n: usize, dt: f64) -> Circuit {
        assert!(n <= MAX_N);
        Circuit {
            n,
            dt,
            r: [(0, 0, 1.0); MAX_R],
            nr: 0,
            c: [(0, 0, 0.0); MAX_C],
            nc: 0,
            g: [Vccs::default(); MAX_G],
            ng: 0,
            inputs: [(0, 0.0); MAX_IN],
            ni: 0,
            e: [Nl::DiodePair {
                a: 0,
                b: 0,
                is: 0.0,
                nvt: 1.0,
            }; MAX_E],
            ne: 0,
            dims: 0,
            block: [(0, 0); MAX_NL],
            outs: [[0.0; MAX_N]; MAX_OUT],
            nout: 0,
            ginv: [[0.0; MAX_N]; MAX_N],
            tq: [[0.0; MAX_Q]; MAX_N],
            gn: [[0.0; MAX_NL]; MAX_N],
            k: [[0.0; MAX_NL]; MAX_NL],
            pq: [[0.0; MAX_Q]; MAX_NL],
            vq: [[0.0; MAX_Q]; MAX_C],
            vi: [[0.0; MAX_NL]; MAX_C],
            oq: [[0.0; MAX_Q]; MAX_OUT],
            oi: [[0.0; MAX_NL]; MAX_OUT],
            aq: [[0.0; ROWS]; MAX_Q],
            ai: [[0.0; ROWS]; MAX_NL],
            gc: [0.0; MAX_C],
            j: [0.0; MAX_C],
            v: [0.0; MAX_NL],
            v1: [0.0; MAX_NL],
            q_last: [0.0; MAX_Q],
            i_last: [0.0; MAX_NL],
            y0: [0.0; MAX_OUT],
        }
    }

    /// A resistor of `ohms` between `a` and `b`; returns its index (for [`Self::set_r`]).
    pub fn r(&mut self, a: u8, b: u8, ohms: f64) -> usize {
        self.r[self.nr] = (a, b, ohms);
        self.nr += 1;
        self.nr - 1
    }

    /// A capacitor of `farads` between `a` and `b`.
    pub fn c(&mut self, a: u8, b: u8, farads: f64) {
        self.c[self.nc] = (a, b, farads);
        self.nc += 1;
    }

    /// A transconductance `gm·(v[cp] − v[cm])` into `op`, out of `om`.
    pub fn vccs(&mut self, op: u8, om: u8, cp: u8, cm: u8, gm: f64) {
        self.g[self.ng] = Vccs { op, om, cp, cm, gm };
        self.ng += 1;
    }

    /// A voltage source behind `ohms` into `node` (`ohms` = ∞: a pure current input).
    /// Returns the input's index.
    pub fn input(&mut self, node: u8, ohms: f64) -> usize {
        let g = if ohms.is_finite() { 1.0 / ohms } else { 0.0 };
        self.inputs[self.ni] = (node, g);
        self.ni += 1;
        self.ni - 1
    }

    /// An op-amp as a single-pole macromodel: a transconductance from `(p, m)` into an
    /// internal node `int` loaded by `cc` and by `gm / a0` (open-loop gain `a0`, unity-gain
    /// frequency `gm / 2πcc`), and an output stage of `r_out` ohms following that node into
    /// `out`. With `imax` set, the transconductance saturates (slew rate `imax / cc`) and
    /// becomes a nonlinear port.
    #[allow(clippy::too_many_arguments)]
    pub fn opamp(
        &mut self,
        p: u8,
        m: u8,
        int: u8,
        out: u8,
        gbw_hz: f64,
        a0: f64,
        cc: f64,
        r_out: f64,
        imax: Option<f64>,
    ) {
        let gm = std::f64::consts::TAU * gbw_hz * cc;
        match imax {
            Some(imax) => self.nl(Nl::Slew {
                p,
                m,
                to: int,
                gm,
                imax,
            }),
            None => self.vccs(int, 0, p, m, gm),
        }
        self.c(int, 0, cc);
        self.r(int, 0, a0 / gm);
        // The output stage: a source following `int` behind `r_out`.
        self.vccs(out, 0, int, 0, 1.0 / r_out);
        self.r(out, 0, r_out);
    }

    /// A unity buffer: `out` follows `inp` behind `r_out` ohms.
    pub fn buffer(&mut self, inp: u8, out: u8, r_out: f64) {
        self.vccs(out, 0, inp, 0, 1.0 / r_out);
        self.r(out, 0, r_out);
    }

    /// A nonlinear element.
    pub fn nl(&mut self, e: Nl) {
        let (lo, hi) = (self.dims as u8, (self.dims + e.dims()) as u8);
        for b in &mut self.block[lo as usize..hi as usize] {
            *b = (lo, hi);
        }
        self.dims += e.dims();
        assert!(self.dims <= MAX_NL);
        self.e[self.ne] = e;
        self.ne += 1;
    }

    /// An output: the sum of `weights` times node voltages, `(node, weight)`.
    pub fn out(&mut self, weights: &[(u8, f64)]) -> usize {
        for &(node, w) in weights {
            if node > 0 {
                self.outs[self.nout][node as usize - 1] += w;
            }
        }
        self.nout += 1;
        self.nout - 1
    }

    /// Resistor `i`'s value (a knob moved). Call [`Self::rebuild`] after.
    pub fn set_r(&mut self, i: usize, ohms: f64) {
        self.r[i].2 = ohms;
    }

    /// Input `i`'s series resistance (a battery running down). Call [`Self::rebuild`] after.
    pub fn set_input_r(&mut self, i: usize, ohms: f64) {
        self.inputs[i].1 = 1.0 / ohms;
    }

    pub fn resistance(&self, i: usize) -> f64 {
        self.r[i].2
    }

    /// The sampling interval.
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Recompute the matrices from the values, for the transient (`dc` false) or the
    /// operating point (`dc` true: capacitors open).
    pub fn rebuild_for(&mut self, dc: bool) {
        let n = self.n;
        let mut a = [[0.0f64; MAX_N]; MAX_N];
        let mut stamp = |a: &mut [[f64; MAX_N]; MAX_N], p: u8, q: u8, g: f64| {
            let (p, q) = (p as usize, q as usize);
            if p > 0 {
                a[p - 1][p - 1] += g;
            }
            if q > 0 {
                a[q - 1][q - 1] += g;
            }
            if p > 0 && q > 0 {
                a[p - 1][q - 1] -= g;
                a[q - 1][p - 1] -= g;
            }
        };
        for row in a.iter_mut().take(n).enumerate() {
            row.1[row.0] += GMIN;
        }
        for &(p, q, ohms) in &self.r[..self.nr] {
            stamp(&mut a, p, q, 1.0 / ohms);
        }
        for (k, &(p, q, f)) in self.c[..self.nc].iter().enumerate() {
            self.gc[k] = if dc { 0.0 } else { 2.0 * f / self.dt };
            stamp(&mut a, p, q, self.gc[k]);
        }
        for &(node, g) in &self.inputs[..self.ni] {
            if g > 0.0 {
                stamp(&mut a, node, 0, g);
            }
        }
        for s in &self.g[..self.ng] {
            // Injected current gm·(v_cp − v_cm) into op: moved to the left-hand side.
            for (row, sign) in [(s.op, 1.0), (s.om, -1.0)] {
                if row == 0 {
                    continue;
                }
                let r = row as usize - 1;
                if s.cp > 0 {
                    a[r][s.cp as usize - 1] -= sign * s.gm;
                }
                if s.cm > 0 {
                    a[r][s.cm as usize - 1] += sign * s.gm;
                }
            }
        }
        invert(&mut a, &mut self.ginv, n);
        // Nᵥ and Nᵢ, column by column.
        let mut nv = [[0.0f64; MAX_N]; MAX_NL];
        let mut ni = [[0.0f64; MAX_NL]; MAX_N];
        let put_v = |nv: &mut [[f64; MAX_N]; MAX_NL], d: usize, node: u8, w: f64| {
            if node > 0 {
                nv[d][node as usize - 1] += w;
            }
        };
        let put_i = |ni: &mut [[f64; MAX_NL]; MAX_N], d: usize, node: u8, w: f64| {
            if node > 0 {
                ni[node as usize - 1][d] += w;
            }
        };
        let mut d = 0;
        for e in &self.e[..self.ne] {
            match *e {
                Nl::DiodePair { a: p, b: q, .. } => {
                    put_v(&mut nv, d, p, 1.0);
                    put_v(&mut nv, d, q, -1.0);
                    put_i(&mut ni, d, p, -1.0);
                    put_i(&mut ni, d, q, 1.0);
                    d += 1;
                }
                Nl::Npn { c, b, e, .. } => {
                    put_v(&mut nv, d, b, 1.0);
                    put_v(&mut nv, d, e, -1.0);
                    put_v(&mut nv, d + 1, b, 1.0);
                    put_v(&mut nv, d + 1, c, -1.0);
                    put_i(&mut ni, d, b, -1.0);
                    put_i(&mut ni, d, e, 1.0);
                    put_i(&mut ni, d + 1, c, -1.0);
                    put_i(&mut ni, d + 1, e, 1.0);
                    d += 2;
                }
                Nl::Slew { p, m, to, .. } => {
                    put_v(&mut nv, d, p, 1.0);
                    put_v(&mut nv, d, m, -1.0);
                    put_i(&mut ni, d, to, 1.0);
                    d += 1;
                }
            }
        }
        let (dims, m) = (self.dims, self.ni + self.nc);
        // T = G⁻¹ S: the nodes' share of each input and each capacitor's history source.
        let ginv = &self.ginv;
        for (r, row) in self.tq.iter_mut().enumerate().take(n) {
            *row = [0.0; MAX_Q];
            for (k, &(node, g)) in self.inputs[..self.ni].iter().enumerate() {
                if node > 0 {
                    row[k] = ginv[r][node as usize - 1] * if g > 0.0 { g } else { 1.0 };
                }
            }
            for (k, &(p, q, _)) in self.c[..self.nc].iter().enumerate() {
                let at = |node: u8| {
                    if node > 0 {
                        ginv[r][node as usize - 1]
                    } else {
                        0.0
                    }
                };
                row[self.ni + k] = at(p) - at(q);
            }
        }
        for r in 0..n {
            for j in 0..dims {
                let mut s = 0.0;
                for (l, row) in ni.iter().enumerate().take(n) {
                    s += ginv[r][l] * row[j];
                }
                self.gn[r][j] = s;
            }
        }
        for i in 0..dims {
            for j in 0..dims {
                let mut s = 0.0;
                for (l, w) in nv[i].iter().enumerate().take(n) {
                    s += w * self.gn[l][j];
                }
                self.k[i][j] = s;
            }
            for j in 0..m {
                let mut s = 0.0;
                for (l, w) in nv[i].iter().enumerate().take(n) {
                    s += w * self.tq[l][j];
                }
                self.pq[i][j] = s;
            }
        }
        let node_row = |rows: &[[f64; MAX_Q]; MAX_N], node: u8| {
            if node > 0 {
                rows[node as usize - 1]
            } else {
                [0.0; MAX_Q]
            }
        };
        let node_gn = |rows: &[[f64; MAX_NL]; MAX_N], node: u8| {
            if node > 0 {
                rows[node as usize - 1]
            } else {
                [0.0; MAX_NL]
            }
        };
        for (k, &(p, q, _)) in self.c[..self.nc].iter().enumerate() {
            let (ta, tb) = (node_row(&self.tq, p), node_row(&self.tq, q));
            let (ga, gb) = (node_gn(&self.gn, p), node_gn(&self.gn, q));
            for j in 0..MAX_Q {
                self.vq[k][j] = ta[j] - tb[j];
            }
            for j in 0..MAX_NL {
                self.vi[k][j] = ga[j] - gb[j];
            }
        }
        for o in 0..self.nout {
            self.oq[o] = [0.0; MAX_Q];
            self.oi[o] = [0.0; MAX_NL];
            for l in 0..n {
                let w = self.outs[o][l];
                if w == 0.0 {
                    continue;
                }
                for j in 0..MAX_Q {
                    self.oq[o][j] += w * self.tq[l][j];
                }
                for j in 0..MAX_NL {
                    self.oi[o][j] += w * self.gn[l][j];
                }
            }
        }
        self.aq = [[0.0; ROWS]; MAX_Q];
        self.ai = [[0.0; ROWS]; MAX_NL];
        for j in 0..m {
            for r in 0..dims {
                self.aq[j][r] = self.pq[r][j];
            }
            for k in 0..self.nc {
                self.aq[j][CAP_ROW + k] = self.vq[k][j];
            }
            for o in 0..self.nout {
                self.aq[j][OUT_ROW + o] = self.oq[o][j];
            }
        }
        for d in 0..dims {
            for k in 0..self.nc {
                self.ai[d][CAP_ROW + k] = self.vi[k][d];
            }
            for o in 0..self.nout {
                self.ai[d][OUT_ROW + o] = self.oi[o][d];
            }
        }
    }

    /// Recompute for the transient.
    pub fn rebuild(&mut self) {
        self.rebuild_for(false);
    }
}

/// Gauss–Jordan inversion with partial pivoting of the leading `n × n` block.
fn invert(a: &mut [[f64; MAX_N]; MAX_N], out: &mut [[f64; MAX_N]; MAX_N], n: usize) {
    for (i, row) in out.iter_mut().enumerate() {
        *row = [0.0; MAX_N];
        if i < n {
            row[i] = 1.0;
        }
    }
    for col in 0..n {
        let mut piv = col;
        for r in col + 1..n {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        a.swap(col, piv);
        out.swap(col, piv);
        let d = a[col][col];
        let inv = if d != 0.0 { 1.0 / d } else { 0.0 };
        for j in 0..n {
            a[col][j] *= inv;
            out[col][j] *= inv;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = a[r][col];
            if f == 0.0 {
                continue;
            }
            for j in 0..n {
                a[r][j] -= f * a[col][j];
                out[r][j] -= f * out[col][j];
            }
        }
    }
}

/// Solve the `d × d` system `m · x = b` in place (`b` becomes `x`), partial pivoting.
fn solve_small(m: &mut [[f64; MAX_NL]; MAX_NL], b: &mut [f64; MAX_NL], d: usize) {
    for col in 0..d {
        let mut piv = col;
        for r in col + 1..d {
            if m[r][col].abs() > m[piv][col].abs() {
                piv = r;
            }
        }
        m.swap(col, piv);
        b.swap(col, piv);
        let p = m[col][col];
        let inv = if p != 0.0 { 1.0 / p } else { 0.0 };
        for r in col + 1..d {
            let f = m[r][col] * inv;
            if f == 0.0 {
                continue;
            }
            for j in col..d {
                m[r][j] -= f * m[col][j];
            }
            b[r] -= f * b[col];
        }
    }
    for col in (0..d).rev() {
        let mut s = b[col];
        for j in col + 1..d {
            s -= m[col][j] * b[j];
        }
        let p = m[col][col];
        b[col] = if p != 0.0 { s / p } else { 0.0 };
    }
}

/// Newton iterations counted, for the tests.
#[cfg(test)]
pub static ITERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `e^x` for the junctions, our own and fast: `2^k · e^r` with `|r| ≤ ln2/2` and a
/// degree-10 Taylor polynomial (relative error below 10⁻¹¹ — far under what the device
/// models know), ordinary arithmetic in a fixed order, so the same bits everywhere. The
/// argument is capped at [`EXP_CAP`] above and −700 below.
#[inline]
pub fn fast_exp(x: f64) -> f64 {
    const LOG2E: f64 = std::f64::consts::LOG2_E;
    const LN2_HI: f64 = 6.931_471_803_691_238_2e-1;
    const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;
    let x = x.clamp(-700.0, EXP_CAP);
    // Round to nearest by truncation of x·log₂e ± ½ (a plain conversion: `round` is a
    // library call on baseline x86-64).
    let t = x * LOG2E;
    let ki = (t + if t >= 0.0 { 0.5 } else { -0.5 }) as i64;
    let k = ki as f64;
    let r = (x - k * LN2_HI) - k * LN2_LO;
    let mut p = 1.0 / 3_628_800.0;
    p = p * r + 1.0 / 362_880.0;
    p = p * r + 1.0 / 40_320.0;
    p = p * r + 1.0 / 5_040.0;
    p = p * r + 1.0 / 720.0;
    p = p * r + 1.0 / 120.0;
    p = p * r + 1.0 / 24.0;
    p = p * r + 1.0 / 6.0;
    p = p * r + 0.5;
    p = p * r + 1.0;
    p = p * r + 1.0;
    p * f64::from_bits(((ki + 1023) as u64) << 52)
}

#[inline]
fn cap_exp(x: f64) -> f64 {
    fast_exp(x)
}

/// Port currents `i` and their derivatives `∂i/∂v` (block diagonal: an NPN couples its two
/// ports) at port voltages `v`.
#[inline]
fn currents(
    e: &[Nl],
    v: &[f64; MAX_NL],
    i: &mut [f64; MAX_NL],
    di: &mut [[f64; MAX_NL]; MAX_NL],
) {
    let mut d = 0;
    for el in e {
        match *el {
            Nl::DiodePair { is, nvt, .. } => {
                let ep = cap_exp(v[d] / nvt);
                let em = 1.0 / ep;
                i[d] = is * (ep - em);
                di[d][d] = is * (ep + em) / nvt;
                d += 1;
            }
            Nl::Npn { is, bf, br, vt, .. } => {
                let ebe = cap_exp(v[d] / vt);
                let ebc = cap_exp(v[d + 1] / vt);
                let (ef, er) = (ebe - 1.0, ebc - 1.0);
                // Port d: the base current; port d + 1: the collector current.
                i[d] = is / bf * ef + is / br * er;
                i[d + 1] = is * (ef - er) - is / br * er;
                di[d][d] = is / bf * ebe / vt;
                di[d][d + 1] = is / br * ebc / vt;
                di[d + 1][d] = is * ebe / vt;
                di[d + 1][d + 1] = -is * ebc / vt * (1.0 + 1.0 / br);
                d += 2;
            }
            Nl::Slew { gm, imax, .. } => {
                let t = 1.0 - 2.0 / (cap_exp(2.0 * gm * v[d] / imax) + 1.0);
                i[d] = imax * t;
                di[d][d] = gm * (1.0 - t * t);
                d += 1;
            }
        }
    }
}

impl Circuit {
    /// The largest step a port takes in one Newton iteration.
    fn step_limit(&self) -> f64 {
        let mut lim = f64::INFINITY;
        for e in &self.e[..self.ne] {
            lim = lim.min(match e {
                Nl::Slew { .. } => SLEW_STEP,
                _ => JUNCTION_STEP,
            });
        }
        lim
    }

    /// Solve `p + K i(v) − v = 0` from the current `self.v`; returns the port currents.
    #[inline]
    fn newton(&mut self, p: &[f64; MAX_NL], iters: usize) -> [f64; MAX_NL] {
        let d = self.dims;
        let mut i = [0.0; MAX_NL];
        if d == 0 {
            return i;
        }
        let lim = self.step_limit();
        let mut di = [[0.0; MAX_NL]; MAX_NL];
        for _ in 0..iters {
            #[cfg(test)]
            ITERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            currents(&self.e[..self.ne], &self.v, &mut i, &mut di);
            // F = p + K i − v;  J = K ∂i/∂v − I;  Δ = −J⁻¹ F.
            let mut f = [0.0; MAX_NL];
            let mut j = [[0.0; MAX_NL]; MAX_NL];
            for r in 0..d {
                let mut s = p[r] - self.v[r];
                for c in 0..d {
                    s += self.k[r][c] * i[c];
                    // ∂i/∂v is block diagonal: column c has entries only in its block.
                    let (lo, hi) = self.block[c];
                    let mut t = 0.0;
                    for l in lo as usize..hi as usize {
                        t += self.k[r][l] * di[l][c];
                    }
                    j[r][c] = t;
                }
                j[r][r] -= 1.0;
                f[r] = -s;
            }
            if d == 1 {
                f[0] /= j[0][0];
            } else {
                solve_small(&mut j, &mut f, d);
            }
            let mut big = 0.0f64;
            for x in f.iter().take(d) {
                big = big.max(x.abs());
            }
            let scale = if big > lim { lim / big } else { 1.0 };
            for (v, x) in self.v.iter_mut().zip(&f).take(d) {
                *v += x * scale;
            }
            if big < STOP {
                // Converged: the currents at the new voltages, to first order.
                let mut out = i;
                for (r, o) in out.iter_mut().enumerate().take(d) {
                    for (c, x) in f.iter().enumerate().take(d) {
                        *o += di[r][c] * x;
                    }
                }
                return out;
            }
        }
        currents(&self.e[..self.ne], &self.v, &mut i, &mut di);
        i
    }

    /// `q = [inputs, capacitor histories]`.
    #[inline]
    fn q(&self, u: &[f64], scale: f64) -> [f64; MAX_Q] {
        let mut q = [0.0; MAX_Q];
        for (k, qk) in q.iter_mut().enumerate().take(self.ni) {
            *qk = u.get(k).copied().unwrap_or(0.0) * scale;
        }
        q[self.ni..self.ni + self.nc].copy_from_slice(&self.j[..self.nc]);
        q
    }

    #[inline]
    fn ports_p(&self, q: &[f64; MAX_Q]) -> [f64; MAX_NL] {
        let m = self.ni + self.nc;
        let mut p = [0.0; MAX_NL];
        for (pr, row) in p.iter_mut().zip(&self.pq).take(self.dims) {
            let mut acc = 0.0;
            for (a, b) in row.iter().zip(q).take(m) {
                acc += a * b;
            }
            *pr = acc;
        }
        p
    }

    #[inline]
    fn lin(&self, rq: &[f64; MAX_Q], ri: &[f64; MAX_NL], q: &[f64; MAX_Q], i: &[f64; MAX_NL]) -> f64 {
        let m = self.ni + self.nc;
        let mut acc = 0.0;
        for (a, b) in rq.iter().zip(q).take(m) {
            acc += a * b;
        }
        for (a, b) in ri.iter().zip(i).take(self.dims) {
            acc += a * b;
        }
        acc
    }

    fn read_out(&self, o: usize) -> f64 {
        self.lin(&self.oq[o], &self.oi[o], &self.q_last, &self.i_last)
    }

    /// One sample: inputs `u` in, outputs (relative to the operating point) into `y`.
    #[inline]
    pub fn step(&mut self, u: &[f64], y: &mut [f64; MAX_OUT]) {
        let q = self.q(u, 1.0);
        let m = self.ni + self.nc;
        let mut acc = [0.0f64; ROWS];
        for (col, qj) in self.aq.iter().zip(&q).take(m) {
            for (a, c) in acc.iter_mut().zip(col) {
                *a += c * qj;
            }
        }
        let mut p = [0.0; MAX_NL];
        p.copy_from_slice(&acc[..MAX_NL]);
        // Start Newton from the line through the last two answers.
        let v1 = self.v;
        for k in 0..self.dims {
            self.v[k] = 2.0 * self.v[k] - self.v1[k];
        }
        self.v1 = v1;
        let i = self.newton(&p, ITER);
        for (col, id) in self.ai.iter().zip(&i).take(self.dims) {
            for (a, c) in acc.iter_mut().zip(col) {
                *a += c * id;
            }
        }
        for k in 0..self.nc {
            self.j[k] = 2.0 * self.gc[k] * acc[CAP_ROW + k] - self.j[k];
        }
        self.q_last = q;
        self.i_last = i;
        for (o, yo) in y.iter_mut().enumerate().take(self.nout) {
            *yo = acc[OUT_ROW + o] - self.y0[o];
        }
    }

    /// The operating point for constant inputs `u` (supplies, and the signal at rest):
    /// capacitors open, the sources ramped up in steps so Newton follows the circuit as it
    /// powers on. Charges every capacitor to it and leaves the matrices built for the
    /// transient, the outputs there taken as their zero. Allocation-free; a few hundred
    /// small solves.
    pub fn solve_dc(&mut self, u: &[f64]) {
        self.rebuild_for(true);
        self.v = [0.0; MAX_NL];
        self.j = [0.0; MAX_C];
        const STEPS: usize = 24;
        let mut q = [0.0; MAX_Q];
        let mut i = [0.0; MAX_NL];
        for step in 1..=STEPS {
            q = self.q(u, step as f64 / STEPS as f64);
            let p = self.ports_p(&q);
            i = self.newton(&p, 400);
        }
        // Each capacitor holds its DC voltage and carries no current: its history source
        // is (2C/T)·v_c − i_c = (2C/T)·v_c.
        let mut vc = [0.0; MAX_C];
        for (k, v) in vc.iter_mut().enumerate().take(self.nc) {
            *v = self.lin(&self.vq[k], &self.vi[k], &q, &i);
        }
        self.rebuild_for(false);
        for k in 0..self.nc {
            self.j[k] = self.gc[k] * vc[k];
        }
        // The transient's own answer at that point is the zero of the outputs.
        let q = self.q(u, 1.0);
        let p = self.ports_p(&q);
        let i = self.newton(&p, 400);
        self.v1 = self.v;
        self.q_last = q;
        self.i_last = i;
        self.y0 = [0.0; MAX_OUT];
        for o in 0..self.nout {
            self.y0[o] = self.read_out(o);
        }
    }

    /// A node's voltage after the last step (absolute).
    pub fn node(&self, node: u8) -> f64 {
        if node == 0 {
            return 0.0;
        }
        let r = node as usize - 1;
        self.lin(&self.tq[r], &self.gn[r], &self.q_last, &self.i_last)
    }

    /// An output's absolute value after the last step.
    pub fn output_abs(&self, o: usize) -> f64 {
        self.read_out(o)
    }

    /// The port voltages (for tests: a transistor's `v_be` and `v_bc`, …).
    pub fn ports(&self) -> &[f64] {
        &self.v[..self.dims]
    }

    /// The state: capacitor histories, ports, the last solution, the outputs' zero.
    pub fn state(&self) -> State {
        State {
            j: self.j,
            v: self.v,
            v1: self.v1,
            q: self.q_last,
            i: self.i_last,
            y0: self.y0,
        }
    }

    /// Back to a saved state (the snapshot a pedal returns to after silence).
    pub fn set_state(&mut self, s: &State) {
        self.j = s.j;
        self.v = s.v;
        self.v1 = s.v1;
        self.q_last = s.q;
        self.i_last = s.i;
        self.y0 = s.y0;
    }
}

/// A circuit's state, saved.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct State {
    j: [f64; MAX_C],
    v: [f64; MAX_NL],
    v1: [f64; MAX_NL],
    q: [f64; MAX_Q],
    i: [f64; MAX_NL],
    y0: [f64; MAX_OUT],
}

impl Default for State {
    fn default() -> Self {
        State {
            j: [0.0; MAX_C],
            v: [0.0; MAX_NL],
            v1: [0.0; MAX_NL],
            q: [0.0; MAX_Q],
            i: [0.0; MAX_NL],
            y0: [0.0; MAX_OUT],
        }
    }
}

//! Arca's preamp as circuits: a model's preamp is a sequence of [`Block`]s — the published
//! schematic read left to right — that this module turns into circuits for
//! [`crate::fragor::dk`].
//!
//! ## Where the preamp is split
//!
//! Each circuit holds one gain stage (and the cathode follower DC-coupled to its plate,
//! when there is one): two triodes and four ports at most, so Newton stays small. A
//! circuit ends at a plate, unloaded; the next begins with that plate's voltage behind the
//! plate's own output resistance `R_p ‖ (r_p + (μ + 1)·R_k′)` (its Thévenin equivalent at
//! the operating point; `R_k′` the cathode resistor when it is not bypassed) and carries
//! everything the plate drives — the coupling capacitor, the tone stack, the volume pot,
//! the grid leak — so the next valve's **grid current flows through its own coupling
//! capacitor**, and the bias shift it charges there (the "blocking" of an overdriven
//! stage) is in the model. At small signals the split is exact but for the plate's Miller
//! capacitance seen through `r_p`; `arca_spice_tests` measures it against the whole
//! schematic solved in one piece.
//!
//! The same tables feed that independent analysis (the component values are shared; the
//! topology there is the schematic's, unsplit).

use super::tube::triode_small_signal;
use crate::fragor::circuits::{taper_a, POT_MIN};
use crate::fragor::dk::{Circuit, Koren, Nl, State};

/// The 12AX7's interelectrode capacitances (datasheet): grid–cathode and grid–plate.
pub const C_GK: f64 = 1.6e-12;
pub const C_GP: f64 = 1.7e-12;
/// What a supply looks like at audio frequencies behind its filter capacitor.
const R_RAIL: f64 = 1.0;

/// A potentiometer's taper.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Taper {
    Lin,
    /// Audio ("A"): 10 % of the track at half rotation.
    Log,
}

impl Taper {
    pub fn fraction(self, pos: f64) -> f64 {
        match self {
            Taper::Lin => pos.clamp(0.0, 1.0),
            Taper::Log => taper_a(pos),
        }
    }
}

/// Which front-panel control turns a pot.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Knob {
    Gain,
    Bass,
    Mid,
    Treble,
    /// Fixed at this rotation (a pot the model has no knob for).
    Fixed(f64),
}

/// A triode gain stage: plate resistor, cathode resistor (and its bypass capacitor), a
/// capacitor across the plate resistor, the plate supply.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Triode {
    pub tube: Koren,
    pub rp: f64,
    pub rk: f64,
    pub ck: Option<f64>,
    pub cp: Option<f64>,
    pub bplus: f64,
}

/// A tone stack, its component values.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Tone {
    /// The treble–middle–bass stack (Fender, Marshall; Yeh & Smith's analysis):
    /// `slope` R4, `c1` treble, `c2` bass, `c3` middle; pots treble R1 (a divider: its
    /// wiper is the output), bass R2 (a variable resistor), middle R3 (a divider whose
    /// wiper takes C3); `mid` the pot or `None` for a fixed resistor of that value.
    Tmb {
        slope: f64,
        c1: f64,
        c2: f64,
        c3: f64,
        treble: (f64, Taper),
        bass: (f64, Taper),
        mid: (f64, Taper),
    },
    /// The top-boost network (two knobs, the Duncan calculator's "Vox"): C1 from the input
    /// to the treble pot's top, its wiper the output, its bottom node X; R1 from the input
    /// to B; C2 B–X; C3 B–Y; the bass pot from X to ground, wiper Y; R2 Y–ground.
    Vox {
        r1: f64,
        r2: f64,
        c1: f64,
        c2: f64,
        c3: f64,
        treble: f64,
        bass: f64,
    },
}

/// One element of a preamp, left to right.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Block {
    /// A series capacitor (coupling).
    Couple(f64),
    /// A series resistor, with a capacitor across it.
    Series(f64, Option<f64>),
    /// A resistor to ground.
    Leak(f64),
    /// A pot to ground whose wiper goes on (volume, gain), with a bright capacitor from
    /// its top to the wiper.
    Pot {
        r: f64,
        taper: Taper,
        knob: Knob,
        bright: Option<f64>,
    },
    /// The tone stack; its output is the treble pot's wiper.
    Tone(Tone),
    /// A gain stage; its grid is the node so far, its plate the next.
    Triode(Triode),
    /// A cathode follower DC-coupled to the plate before it: plate at that stage's supply,
    /// this cathode resistor to ground; its cathode is the next node.
    Follower(f64),
}

/// A pot's resistors in a circuit: `(upper, lower)` indices (either may be absent: a
/// variable resistor), its value and taper, and the knob that turns it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PotRef {
    pub upper: Option<usize>,
    pub lower: Option<usize>,
    pub r: f64,
    pub taper: Taper,
    pub knob: Knob,
}

impl PotRef {
    /// Set its resistors for rotation `pos`.
    pub fn apply(&self, c: &mut Circuit, pos: f64) {
        let a = self.taper.fraction(pos);
        if let Some(i) = self.upper {
            c.set_r(i, ((1.0 - a) * self.r).max(POT_MIN));
        }
        if let Some(i) = self.lower {
            c.set_r(i, (a * self.r).max(POT_MIN));
        }
    }
}

pub const MAX_POTS: usize = 4;

/// The netlist being written: a circuit's elements before it is sized (fixed arrays: a
/// model is built in place on the audio thread when it is chosen, allocation-free).
struct Net {
    nodes: u8,
    r: [(u8, u8, f64); 28],
    nr: usize,
    c: [(u8, u8, f64); 12],
    nc: usize,
    inputs: [(u8, f64); 3],
    ni: usize,
    tubes: [Option<Nl>; 2],
    nt: usize,
    pots: [Option<PotRef>; MAX_POTS],
    np: usize,
    supplies: [f64; 2],
    ns: usize,
}

impl Net {
    fn new() -> Net {
        Net {
            nodes: 0,
            r: [(0, 0, 1.0); 28],
            nr: 0,
            c: [(0, 0, 0.0); 12],
            nc: 0,
            inputs: [(0, 0.0); 3],
            ni: 0,
            tubes: [None; 2],
            nt: 0,
            pots: [None; MAX_POTS],
            np: 0,
            supplies: [0.0; 2],
            ns: 0,
        }
    }
    fn node(&mut self) -> u8 {
        self.nodes += 1;
        self.nodes
    }
    fn r(&mut self, a: u8, b: u8, ohms: f64) -> usize {
        self.r[self.nr] = (a, b, ohms);
        self.nr += 1;
        self.nr - 1
    }
    fn c(&mut self, a: u8, b: u8, f: f64) {
        self.c[self.nc] = (a, b, f);
        self.nc += 1;
    }
    fn input(&mut self, n: u8, r: f64) {
        self.inputs[self.ni] = (n, r);
        self.ni += 1;
    }
    fn supply(&mut self, volts: f64) -> u8 {
        let n = self.node();
        self.input(n, R_RAIL);
        self.supplies[self.ns] = volts;
        self.ns += 1;
        n
    }
    fn tube(&mut self, t: Nl) {
        self.tubes[self.nt] = Some(t);
        self.nt += 1;
    }
    fn pot(
        &mut self,
        upper: Option<usize>,
        lower: Option<usize>,
        r: f64,
        taper: Taper,
        knob: Knob,
    ) {
        self.pots[self.np] = Some(PotRef {
            upper,
            lower,
            r,
            taper,
            knob,
        });
        self.np += 1;
    }
}

/// The knobs' rotations, 0 … 1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Knobs {
    pub gain: f64,
    pub bass: f64,
    pub mid: f64,
    pub treble: f64,
}

impl Default for Knobs {
    fn default() -> Self {
        Knobs {
            gain: 0.5,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
        }
    }
}

impl Knobs {
    pub fn pos(&self, k: Knob) -> f64 {
        match k {
            Knob::Gain => self.gain,
            Knob::Bass => self.bass,
            Knob::Mid => self.mid,
            Knob::Treble => self.treble,
            Knob::Fixed(p) => p,
        }
    }
}

fn write_tone(net: &mut Net, i: u8, t: &Tone) -> u8 {
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
            let (tn, out, l, s, p, q) = (
                net.node(),
                net.node(),
                net.node(),
                net.node(),
                net.node(),
                net.node(),
            );
            net.c(i, tn, c1);
            let tu = net.r(tn, out, treble.0);
            let tl = net.r(out, l, treble.0);
            net.r(i, s, slope);
            net.c(s, l, c2);
            net.c(s, p, c3);
            let bl = net.r(l, q, bass.0);
            let mu = net.r(q, p, mid.0);
            let ml = net.r(p, 0, mid.0);
            net.pot(Some(tu), Some(tl), treble.0, treble.1, Knob::Treble);
            net.pot(None, Some(bl), bass.0, bass.1, Knob::Bass);
            net.pot(Some(mu), Some(ml), mid.0, mid.1, Knob::Mid);
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
            let (a, out, x, b, y) = (net.node(), net.node(), net.node(), net.node(), net.node());
            net.c(i, a, c1);
            let tu = net.r(a, out, treble);
            let tl = net.r(out, x, treble);
            net.r(i, b, r1);
            net.c(b, x, c2);
            net.c(b, y, c3);
            // Turned up, the wiper leaves ground for X: the bass's path grows.
            let bu = net.r(y, 0, bass);
            let bl = net.r(x, y, bass);
            net.r(y, 0, r2);
            net.pot(Some(tu), Some(tl), treble, Taper::Log, Knob::Treble);
            net.pot(Some(bu), Some(bl), bass, Taper::Log, Knob::Bass);
            out
        }
    }
}

/// What drives a circuit: a resistance (the guitar's lead), or the plate of the circuit
/// before — its open-circuit voltage behind the plate's output impedance at the operating
/// point, `R_p ‖ (r_p + (μ + 1)·Z_k)`, `Z_k` the cathode's resistor and bypass capacitor
/// (`(μ + 1)·Z_k` is a resistor `(μ + 1)·R_k` beside a capacitor `C_k/(μ + 1)`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Source {
    R(f64),
    Plate {
        rp_load: f64,
        rp: f64,
        rk: f64,
        ck: Option<f64>,
    },
}

/// Write `blocks` after their source; returns the output node and the stage whose plate
/// it is, if the circuit ends at a plate.
fn write(net: &mut Net, blocks: &[Block], src: Source) -> (u8, Option<Triode>) {
    let mut cur = net.node();
    match src {
        Source::R(r) => net.input(cur, r),
        Source::Plate {
            rp_load,
            rp,
            rk,
            ck,
        } => {
            net.input(cur, R_RAIL);
            let (m, n) = (net.node(), net.node());
            net.r(cur, n, rp_load);
            net.r(cur, m, rp);
            net.r(m, n, rk.max(POT_MIN));
            if let Some(c) = ck {
                net.c(m, n, c);
            }
            cur = n;
        }
    }
    let mut last: Option<(Triode, u8)> = None;
    let mut ends_at_plate = None;
    for b in blocks {
        ends_at_plate = None;
        match *b {
            Block::Couple(f) => {
                let n = net.node();
                net.c(cur, n, f);
                cur = n;
            }
            Block::Series(r, cap) => {
                let n = net.node();
                net.r(cur, n, r);
                if let Some(f) = cap {
                    net.c(cur, n, f);
                }
                cur = n;
            }
            Block::Leak(r) => {
                net.r(cur, 0, r);
            }
            Block::Pot {
                r,
                taper,
                knob,
                bright,
            } => {
                let w = net.node();
                let up = net.r(cur, w, r);
                let lo = net.r(w, 0, r);
                if let Some(f) = bright {
                    net.c(cur, w, f);
                }
                net.pot(Some(up), Some(lo), r, taper, knob);
                cur = w;
            }
            Block::Tone(t) => cur = write_tone(net, cur, &t),
            Block::Triode(t) => {
                let (k, p) = (net.node(), net.node());
                let bp = net.supply(t.bplus);
                net.r(p, bp, t.rp);
                net.r(k, 0, t.rk);
                if let Some(f) = t.ck {
                    net.c(k, 0, f);
                }
                if let Some(f) = t.cp {
                    net.c(p, bp, f);
                }
                net.c(cur, k, C_GK);
                net.c(cur, p, C_GP);
                net.tube(Nl::Triode {
                    p,
                    g: cur,
                    k,
                    tube: t.tube,
                });
                last = Some((t, bp));
                cur = p;
                ends_at_plate = Some(t);
            }
            Block::Follower(rk) => {
                let (t, bp) = last.expect("a follower follows a gain stage");
                let k = net.node();
                net.r(k, 0, rk);
                net.c(cur, k, C_GK);
                net.tube(Nl::Triode {
                    p: bp,
                    g: cur,
                    k,
                    tube: t.tube,
                });
                cur = k;
            }
        }
    }
    (cur, ends_at_plate)
}

/// Circuits a preamp has at most.
pub const MAX_STAGES: usize = 5;

/// Where a model's blocks are cut into circuits — after every plate that no follower
/// takes: `(start, end)` ranges, and how many.
pub fn split(blocks: &[Block]) -> ([(usize, usize); MAX_STAGES], usize) {
    let mut out = [(0, 0); MAX_STAGES];
    let mut n = 0;
    let mut start = 0;
    for i in 0..blocks.len() {
        let plate = matches!(blocks[i], Block::Triode(_));
        let follower_next = matches!(blocks.get(i + 1), Some(Block::Follower(_)));
        if plate && !follower_next && i + 1 < blocks.len() {
            out[n] = (start, i + 1);
            n += 1;
            start = i + 1;
        }
    }
    out[n] = (start, blocks.len());
    (out, n + 1)
}

/// One circuit of a preamp: solved, at its operating point, its pots and its rest.
#[derive(Clone, Debug)]
pub struct Stage {
    pub c: Circuit,
    pots: [Option<PotRef>; MAX_POTS],
    /// The input vector: the signal, then the supplies.
    u: [f64; 3],
    rest: State,
    /// What its output looks like to the next circuit.
    pub out_z: Source,
}

impl Stage {
    fn empty(rate: f64) -> Stage {
        Stage {
            c: Circuit::new(1, 1.0 / rate),
            pots: [None; MAX_POTS],
            u: [0.0; 3],
            rest: State::default(),
            out_z: Source::R(1.0),
        }
    }

    /// Write, size and solve `blocks` at `rate`, driven through `src_r`, knobs at `k`.
    /// Allocation-free.
    pub fn new(blocks: &[Block], src: Source, rate: f64, k: &Knobs) -> Stage {
        let mut net = Net::new();
        let (out, plate) = write(&mut net, blocks, src);
        let mut c = Circuit::new(net.nodes as usize, 1.0 / rate);
        for &(a, b, r) in &net.r[..net.nr] {
            c.r(a, b, r);
        }
        for &(a, b, f) in &net.c[..net.nc] {
            c.c(a, b, f);
        }
        for &(n, r) in &net.inputs[..net.ni] {
            c.input(n, r);
        }
        for t in net.tubes.iter().flatten() {
            c.nl(*t);
        }
        c.out(&[(out, 1.0)]);
        let mut u = [0.0; 3];
        u[1..1 + net.ns].copy_from_slice(&net.supplies[..net.ns]);
        let mut s = Stage {
            c,
            pots: net.pots,
            u,
            rest: State::default(),
            out_z: Source::R(1.0),
        };
        s.set_knobs(k);
        s.c.solve_dc(&s.u);
        s.rest = s.c.state();
        s.out_z = match plate {
            Some(t) => {
                let ports = s.c.ports();
                let (vgk, vpk) = (ports[0], ports[1]);
                let (rp, gm) = triode_small_signal(&t.tube, vgk, vpk);
                let mu1 = gm * rp + 1.0;
                Source::Plate {
                    rp_load: t.rp,
                    rp,
                    rk: mu1 * t.rk,
                    ck: t.ck.map(|c| c / mu1),
                }
            }
            None => Source::R(1.0),
        };
        s
    }

    /// Turn the pots (rebuilds the matrices; the operating point does not move: no
    /// direct current flows through a pot or a tone stack).
    pub fn set_knobs(&mut self, k: &Knobs) {
        for p in self.pots.iter().flatten() {
            p.apply(&mut self.c, k.pos(p.knob));
        }
        self.c.rebuild();
    }

    /// Whether any of its pots answers to `knob`.
    pub fn has(&self, knob: Knob) -> bool {
        self.pots.iter().flatten().any(|p| p.knob == knob)
    }

    /// One sample: the input (volts, relative to rest) in, the output (likewise) out.
    #[inline]
    pub fn step(&mut self, x: f64) -> f64 {
        self.u[0] = x;
        let mut y = [0.0; crate::fragor::dk::MAX_OUT];
        self.c.step(&self.u, &mut y);
        y[0]
    }

    /// Back to the operating point.
    pub fn rest(&mut self) {
        self.c.set_state(&self.rest);
    }

    /// The supplies' voltages.
    pub fn supplies(&self) -> &[f64] {
        &self.u[1..]
    }
}

/// A whole preamp: its circuits in order.
#[derive(Clone, Debug)]
pub struct Preamp {
    stages: [Stage; MAX_STAGES],
    n: usize,
}

impl Preamp {
    /// Build `blocks` at `rate`, driven from a source of `src_r` ohms (the guitar's lead
    /// into the jack). Allocation-free (the circuits are written in place).
    pub fn new(blocks: &[Block], src_r: f64, rate: f64, k: &Knobs) -> Preamp {
        let mut p = Preamp {
            stages: std::array::from_fn(|_| Stage::empty(rate)),
            n: 0,
        };
        p.build(blocks, src_r, rate, k);
        p
    }

    /// Rebuild in place for other blocks or another rate.
    pub fn build(&mut self, blocks: &[Block], src_r: f64, rate: f64, k: &Knobs) {
        let (parts, n) = split(blocks);
        let mut src = Source::R(src_r);
        for (i, &(a, b)) in parts[..n].iter().enumerate() {
            self.stages[i] = Stage::new(&blocks[a..b], src, rate, k);
            src = self.stages[i].out_z;
        }
        self.n = n;
    }

    pub fn stages(&self) -> &[Stage] {
        &self.stages[..self.n]
    }

    pub fn set_knobs(&mut self, k: &Knobs) {
        for s in &mut self.stages[..self.n] {
            s.set_knobs(k);
        }
    }

    #[inline]
    pub fn step(&mut self, x: f64) -> f64 {
        let mut v = x;
        for s in &mut self.stages[..self.n] {
            v = s.step(v);
        }
        v
    }

    pub fn rest(&mut self) {
        for s in &mut self.stages[..self.n] {
            s.rest();
        }
    }
}

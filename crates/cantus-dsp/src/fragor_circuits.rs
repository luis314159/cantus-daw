//! Fragor's five pedals as circuits: each netlist transcribed from the published schematic
//! (the component values are in the code, beside each element, and in ADR 0131), solved by
//! [`super::dk`].
//!
//! | Model | Reference circuit | Stages solved | Ports |
//! |---|---|---|---|
//! | Green overdrive | Ibanez TS808 Tube Screamer | clipping amp (diodes in the feedback) + active tone + level | 1 |
//! | Op-amp distortion | ProCo RAT | LM308 gain stage with its slew rate, diodes to ground, Filter, volume | 2 |
//! | Silicon distortion | Boss DS-1 | transistor booster ‖ op-amp gain (slew) + diodes to ground + tone/level | 2 ‖ 2 |
//! | Germanium fuzz | Dallas Arbiter Fuzz Face (AC128) | the whole circuit, battery included | 4 |
//! | Triangle muff | Electro-Harmonix Big Muff π (V3) | input booster ‖ clipping stage ‖ clipping stage + tone + output booster | 2 ‖ 3 ‖ 5 |
//!
//! `‖` is where a pedal is split into separately solved parts (a dimension of Newton is
//! paid per port, so the parts stay small):
//! - the DS-1's booster drives the op-amp's buffered input (an op-amp follower, U1a): the
//!   split is exact;
//! - the Big Muff's parts are joined at the series resistors into the clipping stages'
//!   bases (R19, R12): the part before drives the resistor against the base's voltage at
//!   the previous sample and hands the next part the current that flows. The loop this
//!   closes (base voltage → current → base voltage) has a gain of about 0.2, so a delay of
//!   one oversampled sample in it costs a few hundredths of a dB (measured against the
//!   whole circuit's analysis in `fragor_spice_tests.rs`; treating the bases as virtual
//!   grounds instead was 6 dB off).
//!
//! Signals are volts: 1.0 at the board's input is one volt at the jack.

use super::dk::{Circuit, Nl, State, MAX_OUT};

/// The thermal voltage at 27 °C (SPICE's default temperature).
pub const VT: f64 = 0.025_852;
/// 1N914 / 1N4148 (the RAT's, the DS-1's, the Big Muff's clipping diodes; the TS808's MA150
/// is a silicon switching diode of the same class): saturation current and emission
/// coefficient from the standard SPICE model.
pub const D_IS: f64 = 2.52e-9;
pub const D_N: f64 = 1.752;
/// The supply.
pub const VCC: f64 = 9.0;
pub const VREF: f64 = 4.5;
/// A supply's or a reference's AC impedance (filtered by large capacitors): ohms.
const R_RAIL: f64 = 1.0;
/// What drives an input that the reference circuit drives from a buffer or an interface.
const R_SOURCE: f64 = 100.0;
/// A potentiometer's track never quite reaches zero ohms.
pub const POT_MIN: f64 = 1.0;

/// The pedals.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    /// Green overdrive (TS808).
    Overdrive,
    /// Op-amp distortion (RAT).
    OpAmp,
    /// Silicon distortion (DS-1).
    Silicon,
    /// Germanium fuzz (Fuzz Face).
    Fuzz,
    /// Triangle muff (Big Muff π).
    Muff,
}

impl Model {
    pub const ALL: [Model; 5] = [
        Model::Overdrive,
        Model::OpAmp,
        Model::Silicon,
        Model::Fuzz,
        Model::Muff,
    ];

    pub fn index(self) -> usize {
        Model::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Model {
        Model::ALL[i.min(4)]
    }
}

/// An audio ("A", logarithmic) taper: 10 % of the track at half rotation.
pub fn taper_a(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    (crate::math::powf(81.0f64, x) - 1.0) / 80.0
}

fn pot(r: f64) -> f64 {
    r.max(POT_MIN)
}

/// One pedal: its circuit (in up to three parts), the indices of the resistors its knobs
/// move, and the state it rests in.
#[derive(Clone, Debug)]
pub struct Pedal {
    pub model: Model,
    parts: [Circuit; 3],
    nparts: usize,
    /// Resistor indices the knobs set, `(part, index)`.
    knob_r: [(usize, usize); 6],
    /// The knobs (0 … 1), as last set.
    knobs: [f64; 3],
    /// The output level (the pedal's volume pot's share, times its calibration).
    level: f64,
    /// The battery (the fuzz's bias): volts and internal resistance.
    battery: (f64, f64),
    rest: [State; 3],
    /// The Big Muff's coupling: the bases of the two clipping stages (Q3, Q2) at the last
    /// sample — what the series resistor before each (R19, R12) sees at its far end — and
    /// their values at rest.
    bases: [f64; 2],
    rest_bases: [f64; 2],
    /// Whether `rest` is the operating point for the knobs as they are.
    rest_valid: bool,
}

/// What a pedal's output is scaled by at full volume, so that at the knobs' defaults
/// and half volume a guitar-level note (−18 dBFS RMS) comes out about as loud as it went in.
/// Measured by `fragor_tests::calibration`.
pub const CAL_DB: [f64; 5] = [14.2, 6.4, 3.8, 15.9, -1.7];

impl Pedal {
    /// A pedal sampled at `rate` (the oversampled rate), knobs at their defaults, resting.
    pub fn new(model: Model, rate: f64) -> Pedal {
        let dt = 1.0 / rate;
        let mut p = Pedal {
            model,
            parts: [
                Circuit::new(1, dt),
                Circuit::new(1, dt),
                Circuit::new(1, dt),
            ],
            nparts: 1,
            knob_r: [(0, 0); 6],
            knobs: [f64::NAN; 3],
            level: 1.0,
            battery: (VCC, R_RAIL),
            rest: [State::default(); 3],
            bases: [0.0; 2],
            rest_bases: [0.0; 2],
            rest_valid: false,
        };
        match model {
            Model::Overdrive => p.build_ts(dt),
            Model::OpAmp => p.build_rat(dt),
            Model::Silicon => p.build_ds1(dt),
            Model::Fuzz => p.build_ff(dt),
            Model::Muff => p.build_bmp(dt),
        }
        p.set_knobs(default_knobs(model));
        p.settle();
        p
    }

    /// The Tube Screamer (ElectroSmash's TS808 clipping amplifier and tone/volume stage).
    fn build_ts(&mut self, dt: f64) {
        let mut c = Circuit::new(16, dt);
        let (src, a, p, vref, m, g, int1, o1) = (1, 2, 3, 4, 5, 6, 7, 8);
        let (t1, m2, w, rc, int2, o2, oc, lv) = (9, 10, 11, 12, 13, 14, 15, 16);
        c.input(src, R_SOURCE); // the input buffer's output (the signal)
        c.input(vref, R_RAIL); // 4.5 V
        c.c(src, a, 1e-6); // C2 1 µF
        c.r(a, p, 1e3); // RA 1 kΩ
        c.r(p, vref, 10e3); // R5 10 kΩ
        c.r(m, g, 4.7e3); // R4 4.7 kΩ
        c.c(g, 0, 0.047e-6); // C3 47 nF
        let drive = c.r(m, o1, 51e3); // R6 51 kΩ + P1 500 kΩ (A) Drive
        c.c(m, o1, 51e-12); // C4 51 pF
        c.nl(Nl::DiodePair {
            a: m,
            b: o1,
            is: D_IS,
            nvt: D_N * VT,
        }); // D1, D2 (MA150)
        c.opamp(p, m, int1, o1, 3e6, 1e5, 30e-12, 50.0, None); // JRC4558
        c.r(o1, t1, 1e3); // R7 1 kΩ
        c.c(t1, 0, 0.22e-6); // C5 0.22 µF
        c.r(t1, vref, 10e3); // R9 10 kΩ
        let tone_a = c.r(t1, w, 10e3); // P2 20 kΩ Tone, + input end
        let tone_b = c.r(w, m2, 10e3); // P2, − input end
        c.c(w, rc, 0.22e-6); // C6 0.22 µF
        c.r(rc, 0, 220.0); // R8 220 Ω
        c.r(o2, m2, 1e3); // R11 1 kΩ
        c.opamp(t1, m2, int2, o2, 3e6, 1e5, 30e-12, 50.0, None);
        c.c(o2, oc, 1e-6); // C7 1 µF
        c.r(oc, lv, 1e3); // R11 1 kΩ
        c.r(lv, 0, 100e3); // P3 100 kΩ (A) Level, its whole track
        c.out(&[(lv, 1.0)]);
        self.parts[0] = c;
        self.nparts = 1;
        self.knob_r = [(0, drive), (0, tone_a), (0, tone_b), (0, 0), (0, 0), (0, 0)];
    }

    /// The RAT (ElectroSmash's schematic: clipping stage, tone control; the JFET follower
    /// is taken as unity).
    fn build_rat(&mut self, dt: f64) {
        let mut c = Circuit::new(13, dt);
        let (src, a, vref, p, m, g1, g2) = (1, 2, 3, 4, 5, 6, 7);
        let (int, o, b, d, f, j) = (8, 9, 10, 11, 12, 13);
        c.input(src, R_SOURCE);
        c.input(vref, R_RAIL);
        c.r(src, 0, 1e6); // R1 1 MΩ
        c.c(src, a, 22e-9); // C1 22 nF
        c.r(a, vref, 1e6); // R2 1 MΩ
        c.r(a, p, 1e3); // R3 1 kΩ
        c.c(p, 0, 1e-9); // C2 1 nF
        let dist = c.r(m, o, 100e3); // Distortion 100 kΩ (A)
        c.c(m, o, 100e-12); // C4 100 pF
        c.r(m, g1, 47.0); // R4 47 Ω
        c.c(g1, 0, 2.2e-6); // C5 2.2 µF
        c.r(m, g2, 560.0); // R5 560 Ω
        c.c(g2, 0, 4.7e-6); // C6 4.7 µF
                            // LM308 with C3 = 30 pF: slew rate 0.3 V/µs (ElectroSmash, from the datasheet), so
                            // Imax = 9 µA, and gm = Imax / 2Vt of its bipolar input pair: unity gain at ~1 MHz.
        let imax = 0.3e6 * 30e-12;
        let gbw = imax / (2.0 * VT) / (std::f64::consts::TAU * 30e-12);
        c.opamp(p, m, int, o, gbw, 2e5, 30e-12, 50.0, Some(imax));
        c.c(o, b, 4.7e-6); // C7 4.7 µF
        c.r(b, d, 1e3); // R6 1 kΩ
        c.nl(Nl::DiodePair {
            a: d,
            b: 0,
            is: D_IS,
            nvt: D_N * VT,
        }); // D1, D2 1N914
        let filter = c.r(d, f, 1.5e3); // Filter 100 kΩ (A) + R7 1.5 kΩ
        c.c(f, 0, 3.3e-9); // C8 3.3 nF
        c.c(f, j, 22e-9); // C9 22 nF
        c.r(j, 0, 1e6); // R8 1 MΩ
        c.out(&[(j, 1.0)]);
        self.parts[0] = c;
        self.nparts = 1;
        self.knob_r = [(0, dist), (0, filter), (0, 0), (0, 0), (0, 0), (0, 0)];
    }
}

/// The knobs' defaults (noon, the fuzz's battery fresh). The knobs are, in order: the gain
/// (Drive, Distortion, Dist, Fuzz, Sustain), the tone (Tone, Filter, Tone, **Bias**, Tone)
/// and the level (Level, Volume, Level, Volume, Volume).
pub fn default_knobs(model: Model) -> [f64; 3] {
    match model {
        Model::Fuzz => [0.5, 1.0, 0.5],
        _ => [0.5, 0.5, 0.5],
    }
}

/// Outputs scratch.
pub(crate) type Out = [f64; MAX_OUT];

/// 2SC2240 (the DS-1's): a low-noise, high-gain silicon NPN.
const Q_2SC2240: (f64, f64, f64) = (1e-14, 400.0, 4.0);
/// BC239 (the Big Muff V3's).
const Q_BC239: (f64, f64, f64) = (1.8e-14, 300.0, 7.5);
/// AC128, germanium PNP, from ElectroSmash's PSpice models: Q1 the low-gain one (β 85),
/// Q2 the high-gain one (β 120), as the Fuzz Face wants them.
const Q_AC128_LO: (f64, f64, f64) = (85.8e-9, 85.0, 20.0);
const Q_AC128_HI: (f64, f64, f64) = (120.8e-9, 120.0, 20.0);
/// The pickup the fuzz sees, as a resistance (its inductance is left out).
pub const FUZZ_SOURCE: f64 = 4.7e3;

fn npn(c: u8, b: u8, e: u8, q: (f64, f64, f64)) -> Nl {
    Nl::Npn {
        c,
        b,
        e,
        is: q.0,
        bf: q.1,
        br: q.2,
        vt: VT,
    }
}

fn diodes(a: u8, b: u8) -> Nl {
    Nl::DiodePair {
        a,
        b,
        is: D_IS,
        nvt: D_N * VT,
    }
}

impl Pedal {
    /// The DS-1 (ElectroSmash's DS-1A schematic). Part 0: the input buffer (taken as a
    /// follower) and the transistor booster, loaded by C5 and R10; part 1: U1b's gain stage
    /// fed by U1a's follower, the diodes, the tone and level network.
    fn build_ds1(&mut self, dt: f64) {
        let mut c = Circuit::new(10, dt);
        let (src, b1, vref, e1, x, b2, c2, e2, vcc, y) = (1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
        c.input(src, 1e3); // R1 1 kΩ
        c.input(vref, R_RAIL);
        c.input(vcc, R_RAIL);
        c.c(src, b1, 47e-9); // C1 47 nF
        c.r(b1, vref, 470e3); // R2 470 kΩ
        c.buffer(b1, e1, 50.0); // Q1, emitter follower
        c.r(e1, 0, 10e3); // R3 10 kΩ
        c.c(e1, x, 0.47e-6); // C2 0.47 µF
        c.r(x, vref, 100e3); // R4 100 kΩ
        c.c(x, b2, 47e-9); // C3 47 nF
        c.r(b2, 0, 100e3); // R5 100 kΩ
        c.r(c2, b2, 470e3); // R7 470 kΩ
        c.c(c2, b2, 250e-12); // C4 250 pF
        c.r(vcc, c2, 10e3); // R8 10 kΩ
        c.r(e2, 0, 22.0); // R9 22 Ω
        c.nl(npn(c2, b2, e2, Q_2SC2240)); // Q2
        c.c(c2, y, 68e-9); // C5 68 nF
        c.r(y, vref, 100e3); // R10 100 kΩ
        c.out(&[(y, 1.0)]);
        self.parts[0] = c;

        let mut c = Circuit::new(12, dt);
        let (ps, vref, m, h, int, o, k, d, hp1, hh, ll, w) =
            (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);
        c.input(ps, 50.0); // U1a, a follower of the booster's output (through R39)
        c.input(vref, R_RAIL);
        c.r(m, o, 100e3); // R11 100 kΩ
        c.c(m, o, 100e-12); // C7 100 pF
        let dist = c.r(m, h, 4.7e3); // VR1 100 kΩ (B) Distortion + R13 4.7 kΩ
        c.c(h, 0, 0.47e-6); // C8 0.47 µF
                            // NJM3404A-class: about 1.2 MHz, 0.6 V/µs.
        c.opamp(
            ps,
            m,
            int,
            o,
            1.2e6,
            1e5,
            30e-12,
            50.0,
            Some(0.6e6 * 30e-12),
        );
        c.r(o, k, 2.2e3); // R14 2.2 kΩ
        c.c(k, d, 0.47e-6); // C9 0.47 µF
        c.nl(diodes(d, vref)); // D4, D5 1N4148, to the 4.5 V reference
        c.c(d, 0, 0.01e-6); // C10 10 nF
        c.c(d, hp1, 0.022e-6); // C11 22 nF
        c.r(hp1, hh, 2.2e3); // R15 2.2 kΩ
        c.r(hh, vref, 6.8e3); // R17 6.8 kΩ
        c.r(d, ll, 6.8e3); // R16 6.8 kΩ
        c.c(ll, vref, 0.1e-6); // C12 0.1 µF
        let tone_l = c.r(ll, w, 10e3); // VR3 20 kΩ (B) Tone, low-pass end
        let tone_h = c.r(w, hh, 10e3); // VR3, high-pass end
        c.r(w, vref, 100e3); // VR2 100 kΩ (B) Level, its whole track
        c.out(&[(w, 1.0), (vref, -1.0)]);
        self.parts[1] = c;
        self.nparts = 2;
        self.knob_r = [(1, dist), (1, tone_l), (1, tone_h), (0, 0), (0, 0), (0, 0)];
    }

    /// The Fuzz Face (ElectroSmash's schematic of the first germanium model), the whole
    /// circuit in one: the PNP original on −9 V is this NPN circuit on +9 V with every
    /// voltage negated, so [`Self::tick`] negates the signal on the way in and out.
    fn build_ff(&mut self, dt: f64) {
        let mut c = Circuit::new(9, dt);
        let (src, b1, c1, e2, w, o, c2, vcc, v) = (1, 2, 3, 4, 5, 6, 7, 8, 9);
        c.input(src, FUZZ_SOURCE);
        c.input(vcc, R_RAIL); // the battery, behind its internal resistance
        c.c(src, b1, 2.2e-6); // C1 2.2 µF
        c.r(vcc, c1, 33e3); // R1 33 kΩ
        c.nl(npn(c1, b1, 0, Q_AC128_LO)); // Q1
        c.nl(npn(c2, c1, e2, Q_AC128_HI)); // Q2
        c.r(e2, b1, 100e3); // R4 100 kΩ
        let fuzz_a = c.r(e2, w, 500.0); // Fuzz 1 kΩ (B), emitter end
        let fuzz_b = c.r(w, 0, 500.0); // Fuzz, ground end
        c.c(w, 0, 20e-6); // C2 20 µF
        c.r(o, c2, 8.2e3); // R3 8.2 kΩ
        c.r(vcc, o, 470.0); // R2 470 Ω
        c.c(o, v, 0.01e-6); // C3 10 nF
        c.r(v, 0, 500e3); // Volume 500 kΩ (A), its whole track
        c.out(&[(v, 1.0)]);
        self.parts[0] = c;
        self.nparts = 1;
        self.knob_r = [(0, fuzz_a), (0, fuzz_b), (0, 0), (0, 0), (0, 0), (0, 0)];
    }

    /// The Big Muff π V3 (ElectroSmash's schematic), in three parts joined by the current
    /// into each clipping stage's base.
    fn build_bmp(&mut self, dt: f64) {
        let mut c = Circuit::new(10, dt);
        let (src, r, b4, c4, e4, vcc, st, sw, sb, q) = (1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
        c.input(src, R_SOURCE);
        c.input(vcc, R_RAIL);
        c.r(src, r, 39e3); // R2 39 kΩ
        c.c(r, b4, 1e-6); // C1 1 µF
        c.r(b4, 0, 47e3); // R14 47 kΩ
        c.r(c4, b4, 470e3); // R9 470 kΩ
        c.c(c4, b4, 470e-12); // C10 470 pF
        c.r(vcc, c4, 10e3); // R13 10 kΩ
        c.r(e4, 0, 100.0); // R22 100 Ω
        c.nl(npn(c4, b4, e4, Q_BC239)); // Q4
        c.c(c4, st, 1e-6); // C4 1 µF
        let sus_a = c.r(st, sw, 50e3); // Sustain 100 kΩ (B), top
        let sus_b = c.r(sw, sb, 50e3); // Sustain, bottom
        c.r(sb, 0, 1e3); // R23 1 kΩ
        c.c(sw, q, 100e-9); // C5 100 nF
        c.input(q, 10e3); // R19 10 kΩ, against Q3's base
        c.out(&[(q, 1.0)]);
        self.parts[0] = c;

        let mut c = Circuit::new(6, dt);
        let (b3, c3, e3, dd, vcc, q2) = (1, 2, 3, 4, 5, 6);
        c.input(b3, f64::INFINITY); // the current through R19
        c.input(vcc, R_RAIL);
        c.r(b3, 0, 100e3); // R20 100 kΩ
        c.r(c3, b3, 470e3); // R17 470 kΩ
        c.c(c3, b3, 470e-12); // C12 470 pF
        c.nl(diodes(c3, dd)); // D3, D4 1N914
        c.c(dd, b3, 1e-6); // C6 1 µF
        c.r(vcc, c3, 10e3); // R18 10 kΩ
        c.r(e3, 0, 150.0); // R21 150 Ω
        c.nl(npn(c3, b3, e3, Q_BC239)); // Q3
        c.c(c3, q2, 100e-9); // C13 100 nF
        c.input(q2, 10e3); // R12 10 kΩ, against Q2's base
        c.out(&[(q2, 1.0)]);
        c.out(&[(b3, 1.0)]);
        self.parts[1] = c;

        let mut c = Circuit::new(12, dt);
        let (b2, c2, e2, dd2, vcc, ll, hh, w, b1, c1, e1, vo) =
            (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);
        c.input(b2, f64::INFINITY); // the current through R12
        c.input(vcc, R_RAIL);
        c.r(b2, 0, 100e3); // R16 100 kΩ
        c.r(c2, b2, 470e3); // R15 470 kΩ
        c.c(c2, b2, 470e-12); // C11 470 pF
        c.nl(diodes(c2, dd2)); // D1, D2 1N914
        c.c(dd2, b2, 1e-6); // C7 1 µF
        c.r(vcc, c2, 10e3); // R11 10 kΩ
        c.r(e2, 0, 150.0); // R10 150 Ω
        c.nl(npn(c2, b2, e2, Q_BC239)); // Q2
        c.r(c2, ll, 39e3); // R8 39 kΩ
        c.c(ll, 0, 10e-9); // C8 10 nF
        c.c(c2, hh, 4e-9); // C9 4 nF
        c.r(hh, 0, 22e3); // R5 22 kΩ
        let tone_h = c.r(hh, w, 50e3); // Tone 100 kΩ (B), high-pass end
        let tone_l = c.r(w, ll, 50e3); // Tone, low-pass end
        c.c(w, b1, 100e-9); // C3 100 nF
        c.r(vcc, b1, 430e3); // R7 430 kΩ
        c.r(b1, 0, 100e3); // R3 100 kΩ
        c.r(vcc, c1, 15e3); // R6 15 kΩ
        c.r(e1, 0, 3.3e3); // R4 3.3 kΩ
        c.nl(npn(c1, b1, e1, Q_BC239)); // Q1
        c.c(c1, vo, 100e-9); // C2 100 nF
        c.r(vo, 0, 100e3); // Volume 100 kΩ (B), its whole track
        c.out(&[(vo, 1.0)]);
        c.out(&[(b2, 1.0)]);
        self.parts[2] = c;
        self.nparts = 3;
        self.knob_r = [
            (0, sus_a),
            (0, sus_b),
            (2, tone_h),
            (2, tone_l),
            (0, 0),
            (0, 0),
        ];
    }
}

impl Pedal {
    /// Set the knobs (0 … 1 each, in [`default_knobs`]'s order). Rebuilds the matrices of
    /// the parts whose values moved; the resting state is found again when next needed.
    pub fn set_knobs(&mut self, k: [f64; 3]) {
        if k == self.knobs {
            return;
        }
        let old = self.knobs;
        self.knobs = k;
        let [g, t, l] = k.map(|v| v.clamp(0.0, 1.0));
        let mut vals = [f64::NAN; 6];
        let cal = crate::math::powf(10.0f64, CAL_DB[self.model.index()] / 20.0);
        match self.model {
            Model::Overdrive => {
                vals[0] = 51e3 + pot(500e3 * taper_a(g));
                vals[1] = pot(20e3 * t);
                vals[2] = pot(20e3 * (1.0 - t));
                self.level = taper_a(l) * cal;
            }
            Model::OpAmp => {
                vals[0] = pot(100e3 * taper_a(g));
                vals[1] = 1.5e3 + pot(100e3 * taper_a(t));
                self.level = taper_a(l) * cal;
            }
            Model::Silicon => {
                vals[0] = 4.7e3 + pot(100e3 * (1.0 - g));
                vals[1] = pot(20e3 * t);
                vals[2] = pot(20e3 * (1.0 - t));
                self.level = l * cal;
            }
            Model::Fuzz => {
                vals[0] = pot(1e3 * (1.0 - g));
                vals[1] = pot(1e3 * g);
                self.battery = battery(t);
                self.parts[0].set_input_r(1, self.battery.1);
                self.level = taper_a(l) * cal;
            }
            Model::Muff => {
                vals[0] = pot(100e3 * (1.0 - g));
                vals[1] = pot(100e3 * g);
                vals[2] = pot(100e3 * (1.0 - t));
                vals[3] = pot(100e3 * t);
                self.level = l * cal;
            }
        }
        let mut dirty = [old[1] != k[1] && self.model == Model::Fuzz, false, false];
        for (i, v) in vals.iter().enumerate() {
            if v.is_nan() {
                continue;
            }
            let (part, r) = self.knob_r[i];
            if self.parts[part].resistance(r) != *v {
                self.parts[part].set_r(r, *v);
                dirty[part] = true;
            }
        }
        for (part, d) in dirty.iter().enumerate().take(self.nparts) {
            if *d {
                self.parts[part].rebuild();
            }
        }
        self.rest_valid = false;
    }

    pub fn knobs(&self) -> [f64; 3] {
        self.knobs
    }

    /// The constant inputs of part `i` at rest (the signal at zero), given the part before.
    fn dc_inputs(&self, i: usize) -> [f64; 3] {
        let prev = if i > 0 {
            self.parts[i - 1].output_abs(0)
        } else {
            0.0
        };
        match (self.model, i) {
            (Model::Overdrive | Model::OpAmp, _) => [0.0, VREF, 0.0],
            (Model::Silicon, 0) => [0.0, VREF, VCC],
            (Model::Silicon, _) => [prev, VREF, 0.0],
            (Model::Fuzz, _) => [0.0, self.battery.0, 0.0],
            // At rest no current crosses C5 or C13: each part sees the next one's base.
            (Model::Muff, 0) => [0.0, VCC, self.parts[1].output_abs(1)],
            (Model::Muff, 1) => [0.0, VCC, self.parts[2].output_abs(1)],
            (Model::Muff, _) => [0.0, VCC, 0.0],
        }
    }

    /// Find the operating point for the knobs as they are and rest there (the state a
    /// pedal powered on and left alone settles into).
    pub fn settle(&mut self) {
        // The Big Muff's parts depend on the next one's base: solve them last to first.
        let backwards = self.model == Model::Muff;
        for k in 0..self.nparts {
            let i = if backwards { self.nparts - 1 - k } else { k };
            let u = self.dc_inputs(i);
            self.parts[i].solve_dc(&u);
            self.rest[i] = self.parts[i].state();
        }
        if self.model == Model::Muff {
            self.bases = [self.parts[1].output_abs(1), self.parts[2].output_abs(1)];
        }
        self.rest_bases = self.bases;
        self.rest_valid = true;
    }

    /// Back to rest: the saved operating point (found again if a knob moved since).
    pub fn snap(&mut self) {
        if !self.rest_valid {
            self.settle();
            return;
        }
        for i in 0..self.nparts {
            self.parts[i].set_state(&self.rest[i]);
        }
        self.bases = self.rest_bases;
    }

    /// One sample (at the oversampled rate), volts in, volts out.
    #[inline]
    pub fn tick(&mut self, x: f64) -> f64 {
        let mut y: Out = [0.0; MAX_OUT];
        match self.model {
            Model::Overdrive | Model::OpAmp => {
                self.parts[0].step(&[x, VREF], &mut y);
                y[0] * self.level
            }
            Model::Silicon => {
                self.parts[0].step(&[x, VREF, VCC], &mut y);
                let mid = self.parts[0].output_abs(0);
                self.parts[1].step(&[mid, VREF], &mut y);
                y[0] * self.level
            }
            Model::Fuzz => {
                self.parts[0].step(&[-x, self.battery.0], &mut y);
                -y[0] * self.level
            }
            Model::Muff => {
                let [b3, b2] = self.bases;
                self.parts[0].step(&[x, VCC, b3], &mut y);
                let i1 = (self.parts[0].output_abs(0) - b3) / 10e3;
                self.parts[1].step(&[i1, VCC, b2], &mut y);
                let i2 = (self.parts[1].output_abs(0) - b2) / 10e3;
                self.bases[0] = self.parts[1].output_abs(1);
                self.parts[2].step(&[i2, VCC], &mut y);
                self.bases[1] = self.parts[2].output_abs(1);
                y[0] * self.level
            }
        }
    }

    /// What the output is scaled by now: the level knob's share of its pot, times the
    /// model's calibration.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// The parts, for the tests (a transistor's bias, a node's voltage).
    pub fn part(&self, i: usize) -> &Circuit {
        &self.parts[i]
    }

    /// The battery the fuzz runs on now: volts and internal resistance.
    pub fn battery(&self) -> (f64, f64) {
        self.battery
    }
}

/// The fuzz's Bias knob as a battery: 1 is a fresh 9 V cell (about 2 Ω inside); turning it
/// down drains it to 1.2 V — the voltage falls and the internal resistance climbs to 1 kΩ,
/// as a cell's does near the end of its life — which starves the transistors into the
/// sputtering, gated fuzz of a dying battery (below about 3 V a decaying note collapses
/// instead of fading: `fragor_tests::the_dying_battery_gates_a_decaying_note`).
pub fn battery(bias: f64) -> (f64, f64) {
    let b = bias.clamp(0.0, 1.0);
    // Squared, so the knob spends its travel where the sound changes most (1.2 – 5 V).
    (1.2 + 7.8 * b * b, 2.0 + 1000.0 * (1.0 - b) * (1.0 - b))
}

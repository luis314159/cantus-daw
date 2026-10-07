//! Modulus — the FM synth: its parameter model (ADR 0023, research
//! `research/instruments/fm-additive-granular.md` §8.5). Owned by this instrument's builder.
//!
//! Eight operators, A…H, **never renumbered**. Every operator's output can modulate the
//! phase of any other, and its own (feedback): a free matrix whose cells are ordinary
//! numbers, so every cell is automatable, modulatable and undoable. Keys:
//!
//! | Key | What | Unit |
//! |---|---|---|
//! | `op.<x>.<field>` | the operator's oscillator: ratio or fixed frequency, detune, level, velocity, phase… | |
//! | `m.<s>.<t>` | modulation index from operator *s* into operator *t* (*s* ≠ *t*) | radians of peak phase deviation |
//! | `fb.<x>` | self-feedback, through the DX7's two-sample average | radians |
//! | `out.<x>`, `pan.<x>` | what the operator sends to the output | dB (−60 = off), % |
//! | `env.<x>.<field>` | the operator's envelope: time/level breakpoints, or DX rates and levels | ms, %, 0…99 |
//! | `ks.<x>.<field>` | keyboard level and rate scaling (the DX break point, depths, curves) | note, dB, 0…7 |
//! | globals | voices, glide, unison, quality, alias guard, pitch envelope, LFOs, easy macros | |
//!
//! Stored sparse, like every native instrument: a fresh patch is operator A alone, a sine
//! at the output. Keys are forever once shipped.

use cantus_dsp::math;

#[path = "fm_algorithms.rs"]
mod algorithms;
pub use algorithms::{dx_algorithm_of, DxAlgorithm, DX_ALGORITHMS};

use super::InstParamInfo;
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.fm";
pub const ICON: TrackIcon = TrackIcon::Synth;
/// Offered in the instrument picker once it is built.
pub const READY: bool = true;

/// Operators per voice.
pub const OPS: usize = 8;
/// Breakpoints of a time-mode envelope.
pub const ENV_POINTS: usize = 8;
/// The letters in keys, and as the editor prints them.
pub const LETTERS: [&str; OPS] = ["a", "b", "c", "d", "e", "f", "g", "h"];
pub const NAMES: [&str; OPS] = ["A", "B", "C", "D", "E", "F", "G", "H"];
/// An output level at or below this is off: the operator is not heard.
pub const OUT_OFF: f32 = -60.0;

/// The modulation sources it produces (tags from `modulation::SOURCES`).
pub const SOURCES: &[&str] = &[
    "env1",
    "lfo1",
    "lfo2",
    "vel",
    "relvel",
    "key",
    "rnd",
    "mpe.bend",
    "mpe.press",
    "mpe.slide",
    "macro1",
    "macro2",
    "macro3",
    "macro4",
    "macro5",
    "macro6",
    "macro7",
    "macro8",
    "mw",
    "at",
    "pb",
    "expr",
    "sus",
];

/// The keys of one operator, as constants (the engine reads them without formatting).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OpKeys {
    pub on: &'static str,
    pub wave: &'static str,
    pub freq_mode: &'static str,
    pub ratio: &'static str,
    pub detune: &'static str,
    pub offset: &'static str,
    pub fixed: &'static str,
    pub level: &'static str,
    pub vel: &'static str,
    pub phase: &'static str,
    pub sync: &'static str,
    pub penv: &'static str,
    pub ams: &'static str,
    pub out: &'static str,
    pub pan: &'static str,
    pub fb: &'static str,
    pub env_mode: &'static str,
    pub env_n: &'static str,
    pub env_sus: &'static str,
    pub env_loop: &'static str,
    pub env_l0: &'static str,
    pub env_t: [&'static str; ENV_POINTS],
    pub env_l: [&'static str; ENV_POINTS],
    pub env_c: [&'static str; ENV_POINTS],
    pub key_time: &'static str,
    pub vel_time: &'static str,
    pub dx_r: [&'static str; 4],
    pub dx_l: [&'static str; 4],
    pub ks_break: &'static str,
    pub ks_ldepth: &'static str,
    pub ks_rdepth: &'static str,
    pub ks_lcurve: &'static str,
    pub ks_rcurve: &'static str,
    pub ks_rate: &'static str,
}

macro_rules! op_keys {
    ($x:literal) => {
        OpKeys {
            on: concat!("op.", $x, ".on"),
            wave: concat!("op.", $x, ".wave"),
            freq_mode: concat!("op.", $x, ".freq_mode"),
            ratio: concat!("op.", $x, ".ratio"),
            detune: concat!("op.", $x, ".detune"),
            offset: concat!("op.", $x, ".offset"),
            fixed: concat!("op.", $x, ".fixed"),
            level: concat!("op.", $x, ".level"),
            vel: concat!("op.", $x, ".vel"),
            phase: concat!("op.", $x, ".phase"),
            sync: concat!("op.", $x, ".sync"),
            penv: concat!("op.", $x, ".penv"),
            ams: concat!("op.", $x, ".ams"),
            out: concat!("out.", $x),
            pan: concat!("pan.", $x),
            fb: concat!("fb.", $x),
            env_mode: concat!("env.", $x, ".mode"),
            env_n: concat!("env.", $x, ".n"),
            env_sus: concat!("env.", $x, ".sus"),
            env_loop: concat!("env.", $x, ".loop"),
            env_l0: concat!("env.", $x, ".l0"),
            env_t: [
                concat!("env.", $x, ".t1"),
                concat!("env.", $x, ".t2"),
                concat!("env.", $x, ".t3"),
                concat!("env.", $x, ".t4"),
                concat!("env.", $x, ".t5"),
                concat!("env.", $x, ".t6"),
                concat!("env.", $x, ".t7"),
                concat!("env.", $x, ".t8"),
            ],
            env_l: [
                concat!("env.", $x, ".l1"),
                concat!("env.", $x, ".l2"),
                concat!("env.", $x, ".l3"),
                concat!("env.", $x, ".l4"),
                concat!("env.", $x, ".l5"),
                concat!("env.", $x, ".l6"),
                concat!("env.", $x, ".l7"),
                concat!("env.", $x, ".l8"),
            ],
            env_c: [
                concat!("env.", $x, ".c1"),
                concat!("env.", $x, ".c2"),
                concat!("env.", $x, ".c3"),
                concat!("env.", $x, ".c4"),
                concat!("env.", $x, ".c5"),
                concat!("env.", $x, ".c6"),
                concat!("env.", $x, ".c7"),
                concat!("env.", $x, ".c8"),
            ],
            key_time: concat!("env.", $x, ".key_time"),
            vel_time: concat!("env.", $x, ".vel_time"),
            dx_r: [
                concat!("env.", $x, ".dr1"),
                concat!("env.", $x, ".dr2"),
                concat!("env.", $x, ".dr3"),
                concat!("env.", $x, ".dr4"),
            ],
            dx_l: [
                concat!("env.", $x, ".dl1"),
                concat!("env.", $x, ".dl2"),
                concat!("env.", $x, ".dl3"),
                concat!("env.", $x, ".dl4"),
            ],
            ks_break: concat!("ks.", $x, ".break"),
            ks_ldepth: concat!("ks.", $x, ".ldepth"),
            ks_rdepth: concat!("ks.", $x, ".rdepth"),
            ks_lcurve: concat!("ks.", $x, ".lcurve"),
            ks_rcurve: concat!("ks.", $x, ".rcurve"),
            ks_rate: concat!("ks.", $x, ".rate"),
        }
    };
}

/// `OP_KEYS[i]` are operator *i*'s keys (0 = A).
pub const OP_KEYS: [OpKeys; OPS] = [
    op_keys!("a"),
    op_keys!("b"),
    op_keys!("c"),
    op_keys!("d"),
    op_keys!("e"),
    op_keys!("f"),
    op_keys!("g"),
    op_keys!("h"),
];

macro_rules! m_row {
    ($s:literal) => {
        [
            concat!("m.", $s, ".a"),
            concat!("m.", $s, ".b"),
            concat!("m.", $s, ".c"),
            concat!("m.", $s, ".d"),
            concat!("m.", $s, ".e"),
            concat!("m.", $s, ".f"),
            concat!("m.", $s, ".g"),
            concat!("m.", $s, ".h"),
        ]
    };
}

/// `M_KEYS[s][t]`: the index from operator *s* into *t*. The diagonal is not a key (an
/// operator into itself is `fb.<x>`, [`matrix_key`] answers it).
pub const M_KEYS: [[&str; OPS]; OPS] = [
    m_row!("a"),
    m_row!("b"),
    m_row!("c"),
    m_row!("d"),
    m_row!("e"),
    m_row!("f"),
    m_row!("g"),
    m_row!("h"),
];

/// The key of a matrix cell, the diagonal being the operator's feedback.
pub fn matrix_key(s: usize, t: usize) -> &'static str {
    if s == t {
        OP_KEYS[s].fb
    } else {
        M_KEYS[s][t]
    }
}

/// The operator a letter names (`"a"` → 0).
pub fn op_index(letter: &str) -> Option<usize> {
    LETTERS.iter().position(|l| *l == letter)
}

// ---- the table -----------------------------------------------------------------------------

/// Play modes (`mode`), as the shared allocator reads them.
pub const MODES: [&str; 3] = ["fm.mode.poly", "fm.mode.mono", "fm.mode.legato"];
/// `quality`: the internal rate. Auto picks each voice's factor at its note-on from the
/// bandwidth the patch can reach there.
pub const QUALITIES: [&str; 5] = [
    "fm.quality.eco",
    "fm.quality.auto",
    "fm.quality.x2",
    "fm.quality.x4",
    "fm.quality.x8",
];
/// `guard`: what the alias guard does with the bandwidth it computes.
pub const GUARDS: [&str; 3] = ["fm.guard.off", "fm.guard.show", "fm.guard.tame"];
/// `op.<x>.wave`.
pub const WAVES: [&str; 4] = [
    "fm.wave.sine",
    "fm.wave.tri",
    "fm.wave.saw",
    "fm.wave.square",
];
/// `lfo<n>.wave`.
pub const LFO_WAVES: [&str; 6] = [
    "fm.lfo.sine",
    "fm.lfo.tri",
    "fm.lfo.saw_down",
    "fm.lfo.saw_up",
    "fm.lfo.square",
    "fm.lfo.sh",
];
/// `ks.<x>.lcurve` / `rcurve`, in the DX7's order.
pub const KS_CURVES: [&str; 4] = [
    "fm.ks.neg_lin",
    "fm.ks.neg_exp",
    "fm.ks.pos_exp",
    "fm.ks.pos_lin",
];

const GLOBAL: &[InstParamInfo] = &[
    p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
    p("voices", "fm.voices", 1.0, 64.0, 16.0, "", false, true),
    p("mode", "fm.mode", 0.0, 2.0, 0.0, "", false, true),
    p("priority", "fm.priority", 0.0, 2.0, 0.0, "", false, true),
    p("glide", "fm.glide", 0.0, 5000.0, 0.0, "ms", false, false),
    p("unison", "fm.unison", 1.0, 8.0, 1.0, "", false, true),
    p(
        "uni.detune",
        "fm.uni_detune",
        0.0,
        100.0,
        10.0,
        "ct",
        false,
        false,
    ),
    p(
        "uni.spread",
        "fm.uni_spread",
        0.0,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    p(
        "transpose",
        "fm.transpose",
        -48.0,
        48.0,
        0.0,
        "st",
        false,
        true,
    ),
    p("tune", "fm.tune", -100.0, 100.0, 0.0, "ct", false, false),
    p("pb_range", "fm.pb_range", 0.0, 48.0, 2.0, "st", false, true),
    p("quality", "fm.quality", 0.0, 4.0, 1.0, "", false, true),
    p("guard", "fm.guard", 0.0, 2.0, 1.0, "", false, true),
    p("fb_avg", "fm.fb_avg", 0.0, 1.0, 1.0, "", false, true),
    p("drift", "fm.drift", 0.0, 20.0, 0.0, "ct", false, false),
    p(
        "penv.l0",
        "fm.penv.l0",
        -48.0,
        48.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "penv.t1",
        "fm.penv.t1",
        0.0,
        10000.0,
        100.0,
        "ms",
        false,
        false,
    ),
    p(
        "penv.l1",
        "fm.penv.l1",
        -48.0,
        48.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "penv.t2",
        "fm.penv.t2",
        0.0,
        10000.0,
        100.0,
        "ms",
        false,
        false,
    ),
    p(
        "penv.l2",
        "fm.penv.l2",
        -48.0,
        48.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "penv.t3",
        "fm.penv.t3",
        0.0,
        10000.0,
        100.0,
        "ms",
        false,
        false,
    ),
    p(
        "penv.l3",
        "fm.penv.l3",
        -48.0,
        48.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "lfo1.rate",
        "fm.lfo.rate",
        0.01,
        50.0,
        5.0,
        "Hz",
        true,
        false,
    ),
    p("lfo1.wave", "fm.lfo.wave", 0.0, 5.0, 0.0, "", false, true),
    p(
        "lfo1.delay",
        "fm.lfo.delay",
        0.0,
        5000.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p("lfo1.sync", "fm.lfo.sync", 0.0, 1.0, 1.0, "", false, true),
    p(
        "lfo1.pitch",
        "fm.lfo.pitch",
        0.0,
        1200.0,
        0.0,
        "ct",
        false,
        false,
    ),
    p("lfo1.amp", "fm.lfo.amp", 0.0, 48.0, 0.0, "dB", false, false),
    p(
        "lfo2.rate",
        "fm.lfo.rate",
        0.01,
        50.0,
        2.0,
        "Hz",
        true,
        false,
    ),
    p("lfo2.wave", "fm.lfo.wave", 0.0, 5.0, 1.0, "", false, true),
    p(
        "lfo2.delay",
        "fm.lfo.delay",
        0.0,
        5000.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p("lfo2.sync", "fm.lfo.sync", 0.0, 1.0, 1.0, "", false, true),
    p(
        "menv.a",
        "fm.menv.a",
        0.0,
        10000.0,
        10.0,
        "ms",
        false,
        false,
    ),
    p(
        "menv.d",
        "fm.menv.d",
        0.0,
        20000.0,
        500.0,
        "ms",
        false,
        false,
    ),
    p("menv.s", "fm.menv.s", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "menv.r",
        "fm.menv.r",
        0.0,
        20000.0,
        300.0,
        "ms",
        false,
        false,
    ),
    p(
        "macro.bright",
        "fm.macro.bright",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "macro.harm",
        "fm.macro.harm",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "macro.detune",
        "fm.macro.detune",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p("macro.a", "fm.macro.a", -1.0, 1.0, 0.0, "%", false, false),
    p("macro.d", "fm.macro.d", -1.0, 1.0, 0.0, "%", false, false),
    p("macro.s", "fm.macro.s", -1.0, 1.0, 0.0, "%", false, false),
    p("macro.r", "fm.macro.r", -1.0, 1.0, 0.0, "%", false, false),
    p("morph.x", "fm.morph.x", 0.0, 1.0, 0.0, "%", false, false),
    p("morph.y", "fm.morph.y", 0.0, 1.0, 0.0, "%", false, false),
];
const GLOBAL_LEN: usize = GLOBAL.len();

/// Numbers per operator in the table, in the order [`op_block`] writes them.
pub const OP_BLOCK: usize = 21 + 3 * ENV_POINTS + 2 + 8 + 6;
const MATRIX_LEN: usize = OPS * (OPS - 1);
const TOTAL: usize = GLOBAL_LEN + OPS * OP_BLOCK + MATRIX_LEN;

/// A fresh time-mode envelope is an ADSR: 5 ms up, 300 ms down to 70 %, 250 ms release.
pub const ENV_T_DEFAULT: [f32; ENV_POINTS] = [5.0, 300.0, 250.0, 100.0, 100.0, 100.0, 100.0, 100.0];
pub const ENV_L_DEFAULT: [f32; ENV_POINTS] = [1.0, 0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
/// DX levels of a fresh DX-mode envelope (the DX7's init voice).
pub const DX_L_DEFAULT: [f32; 4] = [99.0, 99.0, 99.0, 0.0];

const BLANK: InstParamInfo = p("", "", 0.0, 0.0, 0.0, "", false, false);

const fn op_block(k: &OpKeys, out_default: f32) -> [InstParamInfo; OP_BLOCK] {
    let mut b = [BLANK; OP_BLOCK];
    b[0] = p(k.on, "fm.op.on", 0.0, 1.0, 1.0, "", false, true);
    b[1] = p(k.wave, "fm.op.wave", 0.0, 3.0, 0.0, "", false, true);
    b[2] = p(
        k.freq_mode,
        "fm.op.freq_mode",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    );
    b[3] = p(k.ratio, "fm.op.ratio", 0.0625, 64.0, 1.0, "", true, false);
    b[4] = p(
        k.detune,
        "fm.op.detune",
        -100.0,
        100.0,
        0.0,
        "ct",
        false,
        false,
    );
    b[5] = p(
        k.offset,
        "fm.op.offset",
        -100.0,
        100.0,
        0.0,
        "Hz",
        false,
        false,
    );
    b[6] = p(
        k.fixed,
        "fm.op.fixed",
        0.1,
        20000.0,
        440.0,
        "Hz",
        true,
        false,
    );
    b[7] = p(k.level, "fm.op.level", -96.0, 0.0, 0.0, "dB", false, false);
    b[8] = p(k.vel, "fm.op.vel", 0.0, 1.0, 0.0, "%", false, false);
    b[9] = p(k.phase, "fm.op.phase", 0.0, 360.0, 0.0, "°", false, false);
    b[10] = p(k.sync, "fm.op.sync", 0.0, 1.0, 1.0, "", false, true);
    b[11] = p(k.penv, "fm.op.penv", 0.0, 1.0, 1.0, "", false, true);
    b[12] = p(k.ams, "fm.op.ams", 0.0, 3.0, 0.0, "", false, true);
    b[13] = p(
        k.out,
        "fm.out",
        OUT_OFF,
        6.0,
        out_default,
        "dB",
        false,
        false,
    );
    b[14] = p(k.pan, "fm.pan", -1.0, 1.0, 0.0, "%", false, false);
    b[15] = p(k.fb, "fm.fb", 0.0, 4.0, 0.0, "rad", false, false);
    b[16] = p(k.env_mode, "fm.env.mode", 0.0, 1.0, 0.0, "", false, true);
    b[17] = p(
        k.env_n,
        "fm.env.n",
        1.0,
        ENV_POINTS as f32,
        3.0,
        "",
        false,
        true,
    );
    b[18] = p(
        k.env_sus,
        "fm.env.sus",
        0.0,
        ENV_POINTS as f32,
        2.0,
        "",
        false,
        true,
    );
    b[19] = p(
        k.env_loop,
        "fm.env.loop",
        0.0,
        ENV_POINTS as f32,
        0.0,
        "",
        false,
        true,
    );
    b[20] = p(k.env_l0, "fm.env.l0", 0.0, 1.0, 0.0, "%", false, false);
    let mut i = 0;
    while i < ENV_POINTS {
        b[21 + i] = p(
            k.env_t[i],
            "fm.env.time",
            0.0,
            30000.0,
            ENV_T_DEFAULT[i],
            "ms",
            false,
            false,
        );
        b[21 + ENV_POINTS + i] = p(
            k.env_l[i],
            "fm.env.level",
            0.0,
            1.0,
            ENV_L_DEFAULT[i],
            "%",
            false,
            false,
        );
        b[21 + 2 * ENV_POINTS + i] = p(
            k.env_c[i],
            "fm.env.curve",
            -1.0,
            1.0,
            0.0,
            "%",
            false,
            false,
        );
        i += 1;
    }
    let n = 21 + 3 * ENV_POINTS;
    b[n] = p(
        k.key_time,
        "fm.env.key_time",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    );
    b[n + 1] = p(
        k.vel_time,
        "fm.env.vel_time",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    );
    let mut j = 0;
    while j < 4 {
        b[n + 2 + j] = p(
            k.dx_r[j],
            "fm.env.dx_rate",
            0.0,
            99.0,
            99.0,
            "",
            false,
            true,
        );
        b[n + 6 + j] = p(
            k.dx_l[j],
            "fm.env.dx_level",
            0.0,
            99.0,
            DX_L_DEFAULT[j],
            "",
            false,
            true,
        );
        j += 1;
    }
    let n = n + 10;
    b[n] = p(k.ks_break, "fm.ks.break", 0.0, 127.0, 60.0, "", false, true);
    b[n + 1] = p(
        k.ks_ldepth,
        "fm.ks.ldepth",
        0.0,
        48.0,
        0.0,
        "dB",
        false,
        false,
    );
    b[n + 2] = p(
        k.ks_rdepth,
        "fm.ks.rdepth",
        0.0,
        48.0,
        0.0,
        "dB",
        false,
        false,
    );
    b[n + 3] = p(k.ks_lcurve, "fm.ks.lcurve", 0.0, 3.0, 0.0, "", false, true);
    b[n + 4] = p(k.ks_rcurve, "fm.ks.rcurve", 0.0, 3.0, 0.0, "", false, true);
    b[n + 5] = p(k.ks_rate, "fm.ks.rate", 0.0, 7.0, 0.0, "", false, true);
    b
}

const fn build() -> [InstParamInfo; TOTAL] {
    let mut out = [BLANK; TOTAL];
    let mut n = 0;
    while n < GLOBAL_LEN {
        out[n] = GLOBAL[n];
        n += 1;
    }
    let mut o = 0;
    while o < OPS {
        let b = op_block(&OP_KEYS[o], if o == 0 { 0.0 } else { OUT_OFF });
        let mut j = 0;
        while j < OP_BLOCK {
            out[n] = b[j];
            n += 1;
            j += 1;
        }
        o += 1;
    }
    let mut s = 0;
    while s < OPS {
        let mut t = 0;
        while t < OPS {
            if s != t {
                out[n] = p(
                    M_KEYS[s][t],
                    "fm.index",
                    -16.0,
                    16.0,
                    0.0,
                    "rad",
                    false,
                    false,
                );
                n += 1;
            }
            t += 1;
        }
        s += 1;
    }
    out
}

/// Every number of the instrument: the globals, then operator A's block … H's, then the
/// matrix. [`param`] finds a key by searching only its block.
pub static PARAMS: [InstParamInfo; TOTAL] = build();

pub fn params() -> &'static [InstParamInfo] {
    &PARAMS
}

/// The block of the table an operator's keys live in.
pub fn op_params(op: usize) -> &'static [InstParamInfo] {
    let start = GLOBAL_LEN + op * OP_BLOCK;
    &PARAMS[start..start + OP_BLOCK]
}

pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    let mut parts = key.split('.');
    let head = parts.next()?;
    let list: &'static [InstParamInfo] = match head {
        "op" | "out" | "pan" | "fb" | "env" | "ks" => op_params(parts.next().and_then(op_index)?),
        "m" => &PARAMS[TOTAL - MATRIX_LEN..],
        _ => &PARAMS[..GLOBAL_LEN],
    };
    list.iter()
        .find(|i| i.key == key)
        .or_else(|| super::modulation::param(key))
}

/// The patch's name (a DX voice's, or the user's), shown in the editor.
pub const NAME_TEXT: &str = "name";

/// The morph pad's four corners (text): snapshots of the morphable values as
/// `key=value;…`, a key left out being at its default. Corner 1 is at `morph.x = 0,
/// morph.y = 0`, 2 at (1, 0), 3 at (0, 1), 4 at (1, 1); a corner never stored stands for
/// the patch as it is.
pub const MORPH_CORNERS: [&str; 4] = ["morph.c1", "morph.c2", "morph.c3", "morph.c4"];

/// Values the morph pad moves: each operator's ratio, fixed frequency, detune, level,
/// output and feedback, every matrix cell, and three easy macros (FM8's Morph Square moves
/// the matrix and the operators, not the envelopes).
pub const MORPH_FIELDS: usize = OPS * 6 + OPS * (OPS - 1) + 3;

const fn morph_list() -> [&'static str; MORPH_FIELDS] {
    let mut out = [""; MORPH_FIELDS];
    let mut n = 0;
    let mut o = 0;
    while o < OPS {
        let k = &OP_KEYS[o];
        let fields = [k.ratio, k.fixed, k.detune, k.level, k.out, k.fb];
        let mut j = 0;
        while j < 6 {
            out[n] = fields[j];
            n += 1;
            j += 1;
        }
        o += 1;
    }
    let mut s = 0;
    while s < OPS {
        let mut t = 0;
        while t < OPS {
            if s != t {
                out[n] = M_KEYS[s][t];
                n += 1;
            }
            t += 1;
        }
        s += 1;
    }
    out[n] = "macro.bright";
    out[n + 1] = "macro.harm";
    out[n + 2] = "macro.detune";
    out
}

pub static MORPH_KEYS: [&str; MORPH_FIELDS] = morph_list();

pub fn morph_slot(key: &str) -> Option<usize> {
    MORPH_KEYS.iter().position(|k| *k == key)
}

/// A corner's snapshot of the patch: every morphable value away from its default.
pub fn morph_snapshot(spec: &super::NativeSpec) -> String {
    let mut out = String::new();
    for k in MORPH_KEYS.iter() {
        let v = spec.get(k);
        if param(k).is_some_and(|i| i.default != v) {
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str(&format!("{k}={v}"));
        }
    }
    out
}

/// The `(key, value)` pairs of a corner's text, skipping anything malformed. No allocation.
pub fn morph_pairs(text: &str) -> impl Iterator<Item = (&str, f32)> {
    text.split(';').filter_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        Some((k.trim(), v.trim().parse::<f32>().ok()?))
    })
}

/// The corners' weights at a pad position (bilinear), corner 1…4.
pub fn morph_weights(x: f32, y: f32) -> [f32; 4] {
    let (x, y) = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
    [(1.0 - x) * (1.0 - y), x * (1.0 - y), (1.0 - x) * y, x * y]
}

pub fn is_text_key(key: &str) -> bool {
    key == NAME_TEXT || MORPH_CORNERS.contains(&key) || super::modulation::is_text_key(key)
}

pub fn is_asset_key(key: &str) -> bool {
    let _ = key;
    false
}
/// The output level each of `n` carriers gets in a template: their powers add up to one
/// carrier at 0 dB, so an algorithm change keeps the loudness and the headroom.
pub fn carrier_db(n: usize) -> f32 {
    (-100.0 * math::log10(n.max(1) as f32)).round() / 10.0
}

/// The whole topology a DX7 algorithm stands for, as `(key, value)`: every matrix cell,
/// every feedback and every output of the eight operators. Edges get `index` radians,
/// the feedback `feedback` radians, carriers [`carrier_db`] and the rest [`OUT_OFF`]. `alg` is 1…32.
pub fn dx_template(alg: usize, index: f32, feedback: f32) -> Vec<(&'static str, f32)> {
    let a = &DX_ALGORITHMS[alg.clamp(1, 32) - 1];
    let mut out = Vec::with_capacity(OPS * OPS + OPS);
    for s in 0..OPS {
        for t in 0..OPS {
            let (ds, dt) = (s as u8 + 1, t as u8 + 1);
            let v = if s == t {
                if a.feedback == (ds, dt) {
                    feedback
                } else {
                    0.0
                }
            } else if a.feedback == (ds, dt) {
                feedback // a loop back into the top of a stack (algorithms 4 and 6)
            } else if a.edges.contains(&(ds, dt)) {
                index
            } else {
                0.0
            };
            out.push((matrix_key(s, t), v));
        }
    }
    for (o, k) in OP_KEYS.iter().enumerate() {
        let carrier = a.carriers.contains(&(o as u8 + 1));
        out.push((
            k.out,
            if carrier {
                carrier_db(a.carriers.len())
            } else {
                OUT_OFF
            },
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inst::{InstrumentKind, NativeSpec};

    #[test]
    fn the_table_is_laid_out_in_blocks_param_can_search() {
        assert_eq!(PARAMS.len(), TOTAL);
        assert!(PARAMS.iter().all(|i| !i.key.is_empty()), "a blank entry");
        for (o, k) in OP_KEYS.iter().enumerate() {
            let block = op_params(o);
            assert_eq!(block[0].key, k.on);
            assert_eq!(block[OP_BLOCK - 1].key, k.ks_rate);
            for info in block {
                assert!(
                    info.key.contains(&format!(".{}", LETTERS[o])),
                    "{}",
                    info.key
                );
            }
        }
        for info in PARAMS.iter() {
            assert_eq!(param(info.key), Some(info), "{}", info.key);
        }
        assert!(param("m.a.a").is_none(), "the diagonal is fb.a");
        assert_eq!(param("fb.c").map(|i| i.max), Some(4.0));
        assert!(param("op.z.ratio").is_none() && param("nothing").is_none());
    }

    #[test]
    fn a_fresh_patch_is_operator_a_alone_at_the_output() {
        let s = NativeSpec::new(InstrumentKind::Fm);
        assert_eq!(s.get("out.a"), 0.0);
        for k in &OP_KEYS[1..] {
            assert_eq!(s.get(k.out), OUT_OFF);
        }
        assert_eq!(s.get("m.b.a"), 0.0);
        assert_eq!(s.get("quality"), 1.0, "Auto");
    }

    #[test]
    fn the_32_algorithms_are_well_formed() {
        for (i, a) in DX_ALGORITHMS.iter().enumerate() {
            let n = i + 1;
            assert!(!a.carriers.is_empty(), "{n}");
            for &(s, t) in a.edges {
                assert!(
                    s != t && (1..=6).contains(&s) && (1..=6).contains(&t),
                    "{n}"
                );
                assert!(
                    !a.carriers.contains(&s),
                    "{n}: operator {s} both modulates and sounds"
                );
            }
            // Every operator is used: it sounds or it modulates.
            for op in 1..=6u8 {
                assert!(
                    a.carriers.contains(&op) || a.edges.iter().any(|e| e.0 == op),
                    "{n}: operator {op} does nothing"
                );
            }
            assert_eq!(
                dx_algorithm_of(a.edges, a.carriers, Some(a.feedback)),
                Some(n)
            );
        }
        // The chart's landmarks.
        assert_eq!(DX_ALGORITHMS[31].carriers.len(), 6);
        assert_eq!(DX_ALGORITHMS[3].feedback, (4, 6));
        assert_eq!(DX_ALGORITHMS[5].feedback, (5, 6));
        assert_eq!(DX_ALGORITHMS[15].carriers, &[1]);
    }

    #[test]
    fn a_template_writes_the_whole_topology() {
        let t = dx_template(5, 2.0, 1.5);
        let get = |k: &str| t.iter().find(|(key, _)| *key == k).map(|(_, v)| *v);
        assert_eq!(get("m.b.a"), Some(2.0));
        assert_eq!(get("m.a.b"), Some(0.0));
        assert_eq!(get("fb.f"), Some(1.5));
        assert_eq!(get("out.e"), Some(carrier_db(3)));
        assert_eq!(carrier_db(1), 0.0);
        assert_eq!(carrier_db(6), -7.8);
        assert_eq!(get("out.b"), Some(OUT_OFF));
        assert_eq!(get("out.g"), Some(OUT_OFF));
        let loop4 = dx_template(4, 2.0, 1.5);
        let get4 = |k: &str| loop4.iter().find(|(key, _)| *key == k).map(|(_, v)| *v);
        assert_eq!(get4("m.d.f"), Some(1.5), "algorithm 4 feeds 4 back into 6");
        assert_eq!(get4("fb.f"), Some(0.0));
        for (k, _) in &t {
            assert!(param(k).is_some(), "{k}");
        }
    }
}

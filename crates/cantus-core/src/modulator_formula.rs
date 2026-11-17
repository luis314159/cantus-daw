//! The formula controller's language (ADR 0148): a small, safe, deterministic expression
//! language in the syntax of FL Studio's *Fruity Formula Controller*, parsed and compiled
//! off the audio thread into a flat program the audio thread evaluates without allocating.
//!
//! **What it is.** One expression, case-insensitive, of numbers, the inputs and the
//! song's time, operators and functions — no assignments, no loops, no recursion: a
//! program is at most [`MAX_OPS`] steps run once each, on a stack of at most
//! [`MAX_STACK`] numbers checked when it is compiled. Every transcendental is
//! `cantus_dsp::math`'s (the same number on every machine). The value is computed in
//! `f64` (a song's time in beats keeps its precision) and the controller's output is the
//! value clamped to 0…1, as FL's is; anything not finite is 0.
//!
//! **Names** ([`Var`]): `a`, `b`, `c` — the controller's three knobs, 0…1; `SongTime` and
//! `beat` — the song position in quarters (beats); `t` — seconds since playback started;
//! `Time` — milliseconds since playback started (FL's is the system clock: a render
//! would never repeat); `GetTempo`/`tempo` — beats per minute there; `Pi`; `e`; `Rand` —
//! a random number 0…1 that is a hash of the song position (a render repeats). A name may
//! be written as a call with no arguments (`SongTime()`), as FL's manual writes them.
//! `MouseX`, `MouseY` and `Date` are refused: they would make a render depend on the
//! moment it was made. `Tension` and `Warp` are refused too (their published definition
//! does not say what they compute).
//!
//! **Operators**, loosest first: comparisons `=` (`in`), `<>`, `<`, `<=`, `>`, `>=` (1 or
//! 0); `+`, `-`, `or`, `xor`; `*`, `/`, `div`, `mod`, `fmod`, `shl`, `shr`, `and`; the
//! prefixes `-`, `+`, `not`; `^` (power, right to left); the suffixes `%` (a percentage:
//! `50%` is 0.5) and `!` (factorial of the whole part). Logic reads above 0.5 as true, as
//! FL's does.
//!
//! **Functions** ([`Func`]): FL's — `Sin`, `Cos`, `Tan`/`Tg`, `Sec`, `CoSec`, `Ctg`,
//! `ArcSin`, `ArcCos`, `ArcTg`/`ArcTan`, `ATan2`, `Sinh`, `Cosh`, `Tanh`, `ArcCosh`,
//! `ArcTanh`, `DegToRad`, `RadToDeg`, `Int`, `Trunc`, `Floor`, `Ceil`, `Round` (half to
//! even, as Delphi's), `Frac`, `Sqrt`, `Sqr` (the square), `Exp`, `Ln`, `Log10`, `Log2`,
//! `LogN`, `Power`/`Pow`, `IntPower`, `Max`, `Min`, `Sum`, `Abs`, `Neg`, `Case`, `If`,
//! `IfE`, `IfG`, `IfGE`, `IfL`, `IfLE`, `Inter`, `InterNoSwap`, `Map`, `Fold`,
//! `SeededRand` — and Cantus's own cycle waves, each 0…1 over one cycle of its argument:
//! `sine`, `saw`, `tri`, `square`, `pulse(x, width)`, and `clamp(x, lo, hi)`.

#[path = "modulator_formula_parse.rs"]
mod parse;

/// The longest formula text accepted.
pub const MAX_TEXT: usize = 512;
/// The most steps a program runs.
pub const MAX_OPS: usize = 256;
/// The deepest the evaluation stack goes.
pub const MAX_STACK: usize = 32;
/// The deepest the parentheses and calls nest.
pub const MAX_NESTING: usize = 48;

/// A name a formula reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Var {
    A,
    B,
    C,
    /// The song position in beats (`SongTime`, `beat`).
    Beats,
    /// Seconds since playback started (`t`).
    Seconds,
    /// Milliseconds since playback started (`Time`).
    Millis,
    /// Beats per minute (`GetTempo`, `tempo`).
    Tempo,
    Pi,
    E,
    /// A hash of the song position, 0…1 (`Rand`).
    Rand,
}

/// A function of one, two or three numbers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Sec,
    CoSec,
    Ctg,
    ArcSin,
    ArcCos,
    ArcTan,
    Sinh,
    Cosh,
    Tanh,
    ArcCosh,
    ArcTanh,
    DegToRad,
    RadToDeg,
    Int,
    Floor,
    Ceil,
    Round,
    Frac,
    Sqrt,
    Sqr,
    Exp,
    Ln,
    Log10,
    Log2,
    Abs,
    Neg,
    Fold,
    SeededRand,
    Sine,
    Saw,
    Tri,
    Square,
    // Two arguments.
    ATan2,
    LogN,
    Power,
    IntPower,
    Max,
    Min,
    Sum,
    IfE,
    IfG,
    IfGE,
    IfL,
    IfLE,
    Pulse,
    // Three arguments.
    Case,
    If,
    Inter,
    InterNoSwap,
    Map,
    Clamp,
}

/// One step of a program (postfix: each pops its operands and pushes its value).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Op {
    Num(f64),
    Var(Var),
    Neg,
    Not,
    Percent,
    Factorial,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    IDiv,
    Mod,
    FMod,
    Shl,
    Shr,
    And,
    Or,
    Xor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Call(Func),
}

/// What a formula's time and inputs are at one moment.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Env {
    /// The knobs, 0…1.
    pub a: f64,
    pub b: f64,
    pub c: f64,
    /// The song position in beats.
    pub beats: f64,
    /// Seconds since playback started.
    pub seconds: f64,
    /// Beats per minute.
    pub tempo: f64,
}

/// Why a formula was refused, and where (a byte offset into its text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormulaError {
    pub at: usize,
    pub kind: ErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Empty,
    TooLong,
    /// A character the language has no use for.
    Unexpected(char),
    /// The text ended where more was needed.
    UnexpectedEnd,
    /// Something follows a whole formula (`a b`).
    Trailing,
    /// A `(` never closed, or a `)` with no `(`.
    Unclosed,
    UnknownName(String),
    /// A name FL has whose value depends on the moment (`MouseX`, `Date`).
    NotDeterministic(String),
    /// A name FL has whose definition is not published (`Tension`, `Warp`).
    NotSupported(String),
    /// A function called with the wrong number of arguments.
    Arguments { name: String, wants: usize },
    BadNumber,
    /// More than [`MAX_OPS`] steps, [`MAX_STACK`] deep or [`MAX_NESTING`] nested.
    TooComplex,
}

impl ErrorKind {
    /// The i18n key of its message (the editor shows it under the display).
    pub fn key(&self) -> &'static str {
        match self {
            ErrorKind::Empty => "fxd.formula.err.empty",
            ErrorKind::TooLong => "fxd.formula.err.too_long",
            ErrorKind::Unexpected(_) => "fxd.formula.err.unexpected",
            ErrorKind::UnexpectedEnd => "fxd.formula.err.end",
            ErrorKind::Trailing => "fxd.formula.err.trailing",
            ErrorKind::Unclosed => "fxd.formula.err.unclosed",
            ErrorKind::UnknownName(_) => "fxd.formula.err.unknown",
            ErrorKind::NotDeterministic(_) => "fxd.formula.err.moment",
            ErrorKind::NotSupported(_) => "fxd.formula.err.unsupported",
            ErrorKind::Arguments { .. } => "fxd.formula.err.arguments",
            ErrorKind::BadNumber => "fxd.formula.err.number",
            ErrorKind::TooComplex => "fxd.formula.err.complex",
        }
    }
}

/// What a program reads, so the engine and the editor know what moves it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Reads {
    /// `a`, `b`, `c`.
    pub inputs: [bool; 3],
    /// The song's time, the time since playback started or the tempo.
    pub time: bool,
}

/// A compiled formula: built off the audio thread, evaluated on it without allocating.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    ops: Vec<Op>,
    reads: Reads,
}

impl Program {
    /// Compile `text`, or say why not (and where).
    pub fn compile(text: &str) -> Result<Program, FormulaError> {
        if text.len() > MAX_TEXT {
            return Err(FormulaError {
                at: MAX_TEXT,
                kind: ErrorKind::TooLong,
            });
        }
        let ops = parse::parse(text)?;
        let mut reads = Reads::default();
        for op in &ops {
            match op {
                Op::Var(Var::A) => reads.inputs[0] = true,
                Op::Var(Var::B) => reads.inputs[1] = true,
                Op::Var(Var::C) => reads.inputs[2] = true,
                Op::Var(Var::Beats | Var::Seconds | Var::Millis | Var::Tempo | Var::Rand) => {
                    reads.time = true
                }
                _ => {}
            }
        }
        Ok(Program { ops, reads })
    }

    /// The steps, in order.
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    pub fn reads(&self) -> Reads {
        self.reads
    }

    /// The formula's value at `env` (any number, NaN included). Never allocates: the
    /// stack is a fixed array its compilation proved deep enough.
    pub fn value(&self, env: &Env) -> f64 {
        let mut stack = [0.0f64; MAX_STACK];
        let mut n = 0usize;
        for op in &self.ops {
            match *op {
                Op::Num(x) => {
                    stack[n] = x;
                    n += 1;
                }
                Op::Var(v) => {
                    stack[n] = var(v, env);
                    n += 1;
                }
                Op::Neg | Op::Not | Op::Percent | Op::Factorial => {
                    let x = &mut stack[n - 1];
                    *x = unary(*op, *x);
                }
                Op::Call(f) => {
                    let k = arity(f);
                    let args = &stack[n - k..n];
                    let v = call(f, args);
                    n -= k;
                    stack[n] = v;
                    n += 1;
                }
                _ => {
                    let (a, b) = (stack[n - 2], stack[n - 1]);
                    n -= 1;
                    stack[n - 1] = binary(*op, a, b);
                }
            }
        }
        if n == 1 {
            stack[0]
        } else {
            0.0
        }
    }

    /// The controller's output at `env`: the value clamped to 0…1 (FL's range), 0 where
    /// it is not a number.
    pub fn output(&self, env: &Env) -> f32 {
        let v = self.value(env);
        if v.is_finite() {
            v.clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }
}

/// How many numbers a function takes.
pub fn arity(f: Func) -> usize {
    use Func::*;
    match f {
        ATan2 | LogN | Power | IntPower | Max | Min | Sum | IfE | IfG | IfGE | IfL | IfLE
        | Pulse => 2,
        Case | If | Inter | InterNoSwap | Map | Clamp => 3,
        _ => 1,
    }
}

#[path = "modulator_formula_eval.rs"]
mod ops;
use ops::{binary, call, unary, var};

#[cfg(test)]
#[path = "modulator_formula_tests.rs"]
mod tests;

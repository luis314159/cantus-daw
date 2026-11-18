//! The formula's parser (ADR 0145): recursive descent with a nesting limit, emitting the
//! postfix program and proving its stack depth as it goes.

use super::{arity, ErrorKind, FormulaError, Func, Op, Var, MAX_NESTING, MAX_OPS, MAX_STACK};

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Name(String),
    Sym(&'static str),
}

struct Lexed {
    toks: Vec<(Tok, usize)>,
    end: usize,
}

fn lex(text: &str) -> Result<Lexed, FormulaError> {
    let b = text.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let at = i;
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            // An exponent: 1e3, 2.5E-2.
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_digit() {
                    i = j;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let v: f64 = text[at..i].parse().map_err(|_| FormulaError {
                at,
                kind: ErrorKind::BadNumber,
            })?;
            toks.push((Tok::Num(v), at));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            toks.push((Tok::Name(text[at..i].to_ascii_lowercase()), at));
            continue;
        }
        let two = text.get(i..i + 2).unwrap_or("");
        let sym: &'static str = match two {
            "<=" => "<=",
            ">=" => ">=",
            "<>" => "<>",
            _ => match c {
                b'+' => "+",
                b'-' => "-",
                b'*' => "*",
                b'/' => "/",
                b'^' => "^",
                b'%' => "%",
                b'!' => "!",
                b'(' => "(",
                b')' => ")",
                b',' => ",",
                b'=' => "=",
                b'<' => "<",
                b'>' => ">",
                _ => {
                    let ch = text[i..].chars().next().unwrap_or('?');
                    return Err(FormulaError {
                        at,
                        kind: ErrorKind::Unexpected(ch),
                    });
                }
            },
        };
        i += sym.len();
        toks.push((Tok::Sym(sym), at));
    }
    Ok(Lexed {
        toks,
        end: text.len(),
    })
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    end: usize,
    pos: usize,
    ops: Vec<Op>,
    depth: usize,
    max_depth: usize,
    nesting: usize,
}

fn name_var(n: &str) -> Option<Var> {
    Some(match n {
        "a" => Var::A,
        "b" => Var::B,
        "c" => Var::C,
        "songtime" | "beat" | "beats" => Var::Beats,
        "t" | "seconds" => Var::Seconds,
        "time" => Var::Millis,
        "gettempo" | "tempo" => Var::Tempo,
        "pi" => Var::Pi,
        "e" => Var::E,
        "rand" => Var::Rand,
        _ => return None,
    })
}

fn name_func(n: &str) -> Option<Func> {
    use Func::*;
    Some(match n {
        "sin" => Sin,
        "cos" => Cos,
        "tan" | "tg" => Tan,
        "sec" => Sec,
        "cosec" => CoSec,
        "ctg" | "cot" => Ctg,
        "arcsin" => ArcSin,
        "arccos" => ArcCos,
        "arctg" | "arctan" => ArcTan,
        "sinh" => Sinh,
        "cosh" => Cosh,
        "tanh" => Tanh,
        "arccosh" => ArcCosh,
        "arctanh" => ArcTanh,
        "degtorad" => DegToRad,
        "radtodeg" => RadToDeg,
        "int" | "trunc" => Int,
        "floor" => Floor,
        "ceil" => Ceil,
        "round" => Round,
        "frac" => Frac,
        "sqrt" => Sqrt,
        "sqr" => Sqr,
        "exp" => Exp,
        "ln" => Ln,
        "log10" => Log10,
        "log2" => Log2,
        "abs" => Abs,
        "neg" => Neg,
        "fold" => Fold,
        "seededrand" => SeededRand,
        "sine" => Sine,
        "saw" => Saw,
        "tri" => Tri,
        "square" => Square,
        "atan2" => ATan2,
        "logn" => LogN,
        "power" | "pow" => Power,
        "intpower" => IntPower,
        "max" => Max,
        "min" => Min,
        "sum" => Sum,
        "ife" => IfE,
        "ifg" => IfG,
        "ifge" => IfGE,
        "ifl" => IfL,
        "ifle" => IfLE,
        "pulse" => Pulse,
        "case" => Case,
        "if" => If,
        "inter" => Inter,
        "internoswap" => InterNoSwap,
        "map" => Map,
        "clamp" => Clamp,
        _ => return None,
    })
}

const NOT_DETERMINISTIC: [&str; 3] = ["mousex", "mousey", "date"];
const NOT_SUPPORTED: [&str; 2] = ["tension", "warp"];

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|(t, _)| t)
    }

    fn at(&self) -> usize {
        self.toks.get(self.pos).map_or(self.end, |(_, a)| *a)
    }

    fn err(&self, kind: ErrorKind) -> FormulaError {
        FormulaError {
            at: self.at(),
            kind,
        }
    }

    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if *x == s)
    }

    fn is_name(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Name(x)) if x == s)
    }

    /// Emit a step, tracking the stack: `pops` taken, one pushed.
    fn emit(&mut self, op: Op, pops: usize) -> Result<(), FormulaError> {
        if self.ops.len() >= MAX_OPS {
            return Err(self.err(ErrorKind::TooComplex));
        }
        self.ops.push(op);
        self.depth = self.depth + 1 - pops;
        self.max_depth = self.max_depth.max(self.depth);
        if self.max_depth > MAX_STACK {
            return Err(self.err(ErrorKind::TooComplex));
        }
        Ok(())
    }

    fn nest(&mut self) -> Result<(), FormulaError> {
        self.nesting += 1;
        if self.nesting > MAX_NESTING {
            return Err(self.err(ErrorKind::TooComplex));
        }
        Ok(())
    }

    fn expr(&mut self) -> Result<(), FormulaError> {
        self.nest()?;
        self.additive()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Sym("=")) => Op::Eq,
                Some(Tok::Name(n)) if n == "in" => Op::Eq,
                Some(Tok::Sym("<>")) => Op::Ne,
                Some(Tok::Sym("<")) => Op::Lt,
                Some(Tok::Sym("<=")) => Op::Le,
                Some(Tok::Sym(">")) => Op::Gt,
                Some(Tok::Sym(">=")) => Op::Ge,
                _ => break,
            };
            self.pos += 1;
            self.additive()?;
            self.emit(op, 2)?;
        }
        self.nesting -= 1;
        Ok(())
    }

    fn additive(&mut self) -> Result<(), FormulaError> {
        self.term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Sym("+")) => Op::Add,
                Some(Tok::Sym("-")) => Op::Sub,
                Some(Tok::Name(n)) if n == "or" => Op::Or,
                Some(Tok::Name(n)) if n == "xor" => Op::Xor,
                _ => break,
            };
            self.pos += 1;
            self.term()?;
            self.emit(op, 2)?;
        }
        Ok(())
    }

    fn term(&mut self) -> Result<(), FormulaError> {
        self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Sym("*")) => Op::Mul,
                Some(Tok::Sym("/")) => Op::Div,
                Some(Tok::Name(n)) => match n.as_str() {
                    "div" => Op::IDiv,
                    "mod" => Op::Mod,
                    "fmod" => Op::FMod,
                    "shl" => Op::Shl,
                    "shr" => Op::Shr,
                    "and" => Op::And,
                    _ => break,
                },
                _ => break,
            };
            self.pos += 1;
            self.unary()?;
            self.emit(op, 2)?;
        }
        Ok(())
    }

    fn unary(&mut self) -> Result<(), FormulaError> {
        self.nest()?;
        if self.is_sym("-") {
            self.pos += 1;
            self.unary()?;
            self.emit(Op::Neg, 1)?;
        } else if self.is_sym("+") {
            self.pos += 1;
            self.unary()?;
        } else if self.is_name("not") {
            self.pos += 1;
            self.unary()?;
            self.emit(Op::Not, 1)?;
        } else {
            self.power()?;
        }
        self.nesting -= 1;
        Ok(())
    }

    fn power(&mut self) -> Result<(), FormulaError> {
        self.postfix()?;
        if self.is_sym("^") {
            self.pos += 1;
            // Right to left, and a sign after it: 2^-1, 2^3^2 = 2^9.
            self.unary()?;
            self.emit(Op::Pow, 2)?;
        }
        Ok(())
    }

    fn postfix(&mut self) -> Result<(), FormulaError> {
        self.primary()?;
        loop {
            if self.is_sym("%") {
                self.pos += 1;
                self.emit(Op::Percent, 1)?;
            } else if self.is_sym("!") {
                self.pos += 1;
                self.emit(Op::Factorial, 1)?;
            } else {
                return Ok(());
            }
        }
    }

    fn primary(&mut self) -> Result<(), FormulaError> {
        let at = self.at();
        match self.peek().cloned() {
            None => Err(self.err(ErrorKind::UnexpectedEnd)),
            Some(Tok::Num(v)) => {
                self.pos += 1;
                self.emit(Op::Num(v), 0)
            }
            Some(Tok::Sym("(")) => {
                self.pos += 1;
                self.expr()?;
                if !self.is_sym(")") {
                    return Err(FormulaError {
                        at,
                        kind: ErrorKind::Unclosed,
                    });
                }
                self.pos += 1;
                Ok(())
            }
            Some(Tok::Sym(s)) => {
                Err(self.err(ErrorKind::Unexpected(s.chars().next().unwrap_or('?'))))
            }
            Some(Tok::Name(n)) => {
                self.pos += 1;
                self.name(n, at)
            }
        }
    }

    fn name(&mut self, n: String, at: usize) -> Result<(), FormulaError> {
        let fail = |kind| Err(FormulaError { at, kind });
        if NOT_DETERMINISTIC.contains(&n.as_str()) {
            return fail(ErrorKind::NotDeterministic(n));
        }
        if NOT_SUPPORTED.contains(&n.as_str()) {
            return fail(ErrorKind::NotSupported(n));
        }
        if let Some(v) = name_var(&n) {
            // `SongTime()`: a name written as a call of nothing.
            if self.is_sym("(") && matches!(self.toks.get(self.pos + 1), Some((Tok::Sym(")"), _))) {
                self.pos += 2;
            }
            return self.emit(Op::Var(v), 0);
        }
        let Some(f) = name_func(&n) else {
            return fail(ErrorKind::UnknownName(n));
        };
        let wants = arity(f);
        if !self.is_sym("(") {
            return fail(ErrorKind::Arguments { name: n, wants });
        }
        // The parenthesis, where an unclosed call points.
        let open = self.at();
        self.pos += 1;
        self.nest()?;
        let mut got = 0;
        loop {
            self.expr()?;
            got += 1;
            if self.is_sym(",") {
                self.pos += 1;
                continue;
            }
            break;
        }
        if !self.is_sym(")") {
            return match self.peek() {
                None => Err(FormulaError {
                    at: open,
                    kind: ErrorKind::Unclosed,
                }),
                Some(_) => Err(self.err(ErrorKind::Trailing)),
            };
        }
        self.pos += 1;
        self.nesting -= 1;
        if got != wants {
            return fail(ErrorKind::Arguments { name: n, wants });
        }
        self.emit(Op::Call(f), wants)
    }
}

/// The program for `text`.
pub(super) fn parse(text: &str) -> Result<Vec<Op>, FormulaError> {
    let lexed = lex(text)?;
    if lexed.toks.is_empty() {
        return Err(FormulaError {
            at: 0,
            kind: ErrorKind::Empty,
        });
    }
    let mut p = Parser {
        toks: lexed.toks,
        end: lexed.end,
        pos: 0,
        ops: Vec::new(),
        depth: 0,
        max_depth: 0,
        nesting: 0,
    };
    p.expr()?;
    if p.pos < p.toks.len() {
        return Err(if p.is_sym(")") {
            p.err(ErrorKind::Unclosed)
        } else {
            p.err(ErrorKind::Trailing)
        });
    }
    debug_assert_eq!(p.depth, 1);
    Ok(p.ops)
}

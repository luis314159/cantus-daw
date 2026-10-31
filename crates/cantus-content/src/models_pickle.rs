//! Just enough of Python's pickle to read the tensors a PyTorch file names (`torch.save`,
//! protocol 2 to 4): a stack machine over the opcodes such a file uses, building plain
//! values. Nothing is ever imported or called — a `GLOBAL` is only a name, and a "call"
//! (`REDUCE`, `NEWOBJ`) only records its callable and arguments — so a hostile file can
//! make this fail, never run anything.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Tuple(Vec<Val>),
    List(Rc<RefCell<Vec<Val>>>),
    Dict(Rc<RefCell<Vec<(Val, Val)>>>),
    /// `module.name`, never resolved.
    Global(String, String),
    /// What calling `func` with `args` would make (`REDUCE`, `NEWOBJ`).
    Call(Box<Val>, Box<Val>),
    /// A persistent id (`BINPERSID`): in PyTorch, a tensor's storage.
    Persistent(Box<Val>),
    /// An object given its state (`BUILD`).
    Built(Box<Val>, Box<Val>),
}

impl Val {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Val::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Val::Int(i) => Some(*i),
            Val::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }

    /// A dict's entries (an `OrderedDict` made by `REDUCE` is one too).
    pub fn entries(&self) -> Option<Vec<(Val, Val)>> {
        match self {
            Val::Dict(d) => Some(d.borrow().clone()),
            _ => None,
        }
    }

    /// The value at string key `k` of a dict.
    pub fn get(&self, k: &str) -> Option<Val> {
        self.entries()?
            .into_iter()
            .find(|(key, _)| key.as_str() == Some(k))
            .map(|(_, v)| v)
    }

    pub fn items(&self) -> Option<Vec<Val>> {
        match self {
            Val::Tuple(t) => Some(t.clone()),
            Val::List(l) => Some(l.borrow().clone()),
            _ => None,
        }
    }
}

const LIMIT: usize = 1 << 20;

/// The deepest nesting a file may make (a PyTorch state is a few levels deep): deeper is
/// refused, so no value is so deep that dropping it overflows the stack (review 1 #7).
pub const MAX_DEPTH: u32 = 64;

/// The most values a file may make, copies counted (`DUP`, a memo read): a chain of
/// copies cannot grow a file of kilobytes into gigabytes (review 1 #7).
pub const MAX_NODES: u64 = 1 << 20;

/// A value on the stack or in the memo, with its depth and its size in values.
#[derive(Clone)]
struct Node {
    v: Val,
    depth: u32,
    size: u64,
}

struct Machine {
    stack: Vec<Node>,
    marks: Vec<usize>,
    memo: Vec<Option<Node>>,
    made: u64,
}

impl Machine {
    fn charge(&mut self, n: u64) -> Result<(), String> {
        self.made = self.made.saturating_add(n);
        if self.made > MAX_NODES {
            return Err("the pickle makes too many values".into());
        }
        Ok(())
    }

    fn push(&mut self, v: Val) -> Result<(), String> {
        self.charge(1)?;
        if self.stack.len() >= LIMIT {
            return Err("the pickle is too large".into());
        }
        self.stack.push(Node {
            v,
            depth: 1,
            size: 1,
        });
        Ok(())
    }

    /// A value made of parts of these depths and sizes, one deeper than the deepest.
    fn push_of(&mut self, parts: &[(u32, u64)], v: Val) -> Result<(), String> {
        let depth = 1 + parts.iter().map(|p| p.0).max().unwrap_or(0);
        if depth > MAX_DEPTH {
            return Err("the pickle nests too deep".into());
        }
        let size = 1 + parts.iter().map(|p| p.1).sum::<u64>();
        self.charge(1)?;
        self.stack.push(Node { v, depth, size });
        Ok(())
    }

    fn pop(&mut self) -> Result<Node, String> {
        self.stack
            .pop()
            .ok_or_else(|| "the pickle's stack is empty".to_string())
    }

    fn since_mark(&mut self) -> Result<Vec<Node>, String> {
        let m = self.marks.pop().ok_or("no mark")?;
        if m > self.stack.len() {
            return Err("a mark past the stack".into());
        }
        Ok(self.stack.split_off(m))
    }

    /// Grow the container on top by `items` (its depth and size follow them).
    fn grow(&mut self, items: &[Node]) -> Result<(), String> {
        let top = self.stack.last_mut().ok_or("nothing to add to")?;
        for i in items {
            top.depth = top.depth.max(i.depth + 1);
            top.size = top.size.saturating_add(i.size);
        }
        if top.depth > MAX_DEPTH {
            return Err("the pickle nests too deep".into());
        }
        Ok(())
    }

    /// A copy of `n` pushed: it costs its size.
    fn push_copy(&mut self, n: Node) -> Result<(), String> {
        self.charge(n.size)?;
        if self.stack.len() >= LIMIT {
            return Err("the pickle is too large".into());
        }
        self.stack.push(n);
        Ok(())
    }
}

fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |acc, &x| (acc << 8) | x as u64)
}

/// Read a pickle.
pub fn load(data: &[u8]) -> Result<Val, String> {
    let mut m = Machine {
        stack: Vec::new(),
        marks: Vec::new(),
        memo: Vec::new(),
        made: 0,
    };
    let mut at = 0usize;
    let take = |at: &mut usize, n: usize| -> Result<&[u8], String> {
        let end = at
            .checked_add(n)
            .filter(|&e| e <= data.len())
            .ok_or("the pickle ends early")?;
        let s = &data[*at..end];
        *at = end;
        Ok(s)
    };
    loop {
        let op = *take(&mut at, 1)?.first().expect("one byte");
        match op {
            0x80 => {
                take(&mut at, 1)?; // PROTO
            }
            0x95 => {
                take(&mut at, 8)?; // FRAME: a length, nothing to do
            }
            b'.' => return m.pop().map(|n| n.v), // STOP
            b'(' => m.marks.push(m.stack.len()),
            b'N' => m.push(Val::None)?,
            0x88 => m.push(Val::Bool(true))?,
            0x89 => m.push(Val::Bool(false))?,
            b'K' => m.push(Val::Int(take(&mut at, 1)?[0] as i64))?,
            b'M' => m.push(Val::Int(le(take(&mut at, 2)?) as i64))?,
            b'J' => m.push(Val::Int(le(take(&mut at, 4)?) as u32 as i32 as i64))?,
            0x8a => {
                // LONG1: little-endian two's complement, up to 8 bytes here.
                let n = take(&mut at, 1)?[0] as usize;
                let b = take(&mut at, n)?;
                if n > 8 {
                    return Err("an integer too large".into());
                }
                let mut v = le(b) as i64;
                if n > 0 && n < 8 && b[n - 1] & 0x80 != 0 {
                    v -= 1i64 << (8 * n);
                }
                m.push(Val::Int(v))?;
            }
            b'G' => {
                let b = take(&mut at, 8)?;
                m.push(Val::Float(f64::from_be_bytes(
                    b.try_into().expect("8 bytes"),
                )))?;
            }
            b'X' | 0x8c | 0x8d => {
                // BINUNICODE (u32 length), SHORT_BINUNICODE (u8), BINUNICODE8 (u64).
                let n = match op {
                    b'X' => le(take(&mut at, 4)?) as usize,
                    0x8c => take(&mut at, 1)?[0] as usize,
                    _ => le(take(&mut at, 8)?) as usize,
                };
                let s = std::str::from_utf8(take(&mut at, n)?).map_err(|_| "not UTF-8")?;
                m.push(Val::Str(s.to_string()))?;
            }
            b'U' | b'T' | b'C' | b'B' => {
                // SHORT_BINSTRING, BINSTRING, SHORT_BINBYTES, BINBYTES.
                let n = if op == b'U' || op == b'C' {
                    take(&mut at, 1)?[0] as usize
                } else {
                    le(take(&mut at, 4)?) as usize
                };
                m.push(Val::Bytes(take(&mut at, n)?.to_vec()))?;
            }
            b'c' => {
                // GLOBAL: "module\nname\n".
                let line = |at: &mut usize| -> Result<String, String> {
                    let rest = &data[*at..];
                    let end = rest
                        .iter()
                        .position(|&b| b == b'\n')
                        .ok_or("a name without its end")?;
                    let s = std::str::from_utf8(&rest[..end])
                        .map_err(|_| "not UTF-8")?
                        .to_string();
                    *at += end + 1;
                    Ok(s)
                };
                let module = line(&mut at)?;
                let name = line(&mut at)?;
                m.push(Val::Global(module, name))?;
            }
            0x93 => {
                // STACK_GLOBAL
                let name = m.pop()?;
                let module = m.pop()?;
                match (module.v, name.v) {
                    (Val::Str(a), Val::Str(b)) => m.push(Val::Global(a, b))?,
                    _ => return Err("a global that is not two names".into()),
                }
            }
            b'}' => m.push(Val::Dict(Rc::default()))?,
            b']' => m.push(Val::List(Rc::default()))?,
            b')' => m.push(Val::Tuple(Vec::new()))?,
            b't' => {
                let items = m.since_mark()?;
                let meta: Vec<_> = items.iter().map(|n| (n.depth, n.size)).collect();
                m.push_of(&meta, Val::Tuple(items.into_iter().map(|n| n.v).collect()))?;
            }
            0x85..=0x87 => {
                let n = (op - 0x84) as usize;
                if m.stack.len() < n {
                    return Err("a tuple of more than the stack".into());
                }
                let items = m.stack.split_off(m.stack.len() - n);
                let meta: Vec<_> = items.iter().map(|n| (n.depth, n.size)).collect();
                m.push_of(&meta, Val::Tuple(items.into_iter().map(|n| n.v).collect()))?;
            }
            b'a' | b'e' => {
                let items = if op == b'a' {
                    vec![m.pop()?]
                } else {
                    m.since_mark()?
                };
                m.grow(&items)?;
                match m.stack.last().map(|n| &n.v) {
                    Some(Val::List(l)) => l.borrow_mut().extend(items.into_iter().map(|n| n.v)),
                    _ => return Err("APPEND(S) to no list".into()),
                }
            }
            b's' | b'u' => {
                let items = if op == b's' {
                    let v = m.pop()?;
                    let k = m.pop()?;
                    vec![k, v]
                } else {
                    m.since_mark()?
                };
                if items.len() % 2 != 0 {
                    return Err("a dict's items are not in pairs".into());
                }
                m.grow(&items)?;
                match m.stack.last().map(|n| &n.v) {
                    Some(Val::Dict(d)) => {
                        let mut d = d.borrow_mut();
                        let mut it = items.into_iter().map(|n| n.v);
                        while let (Some(k), Some(v)) = (it.next(), it.next()) {
                            d.push((k, v));
                        }
                    }
                    _ => return Err("SETITEM(S) on no dict".into()),
                }
            }
            b'q' | b'r' | 0x94 => {
                let i = match op {
                    b'q' => take(&mut at, 1)?[0] as usize,
                    b'r' => le(take(&mut at, 4)?) as usize,
                    _ => m.memo.len(), // MEMOIZE
                };
                if i >= LIMIT {
                    return Err("a memo index too large".into());
                }
                let n = m.stack.last().cloned().ok_or("nothing to memoise")?;
                if m.memo.len() <= i {
                    m.memo.resize(i + 1, None);
                }
                m.memo[i] = Some(n);
            }
            b'h' | b'j' => {
                let i = if op == b'h' {
                    take(&mut at, 1)?[0] as usize
                } else {
                    le(take(&mut at, 4)?) as usize
                };
                let n = m
                    .memo
                    .get(i)
                    .cloned()
                    .flatten()
                    .ok_or("a memo entry that is not there")?;
                m.push_copy(n)?;
            }
            b'R' | 0x81 => {
                // REDUCE, NEWOBJ: an OrderedDict (or dict) becomes a dict, so the
                // SETITEMS that follow fill it; anything else is only recorded.
                let args = m.pop()?;
                let func = m.pop()?;
                match &func.v {
                    Val::Global(a, b) if a == "collections" && b == "OrderedDict" => {
                        m.push(Val::Dict(Rc::default()))?
                    }
                    _ => {
                        let meta = [(func.depth, func.size), (args.depth, args.size)];
                        m.push_of(&meta, Val::Call(Box::new(func.v), Box::new(args.v)))?;
                    }
                }
            }
            b'Q' => {
                let pid = m.pop()?;
                m.push_of(&[(pid.depth, pid.size)], Val::Persistent(Box::new(pid.v)))?;
            }
            b'b' => {
                let state = m.pop()?;
                let obj = m.pop()?;
                // A dict given a state keeps its entries (and is the same object).
                match obj.v {
                    Val::Dict(_) => m.stack.push(obj),
                    o => {
                        let meta = [(obj.depth, obj.size), (state.depth, state.size)];
                        m.push_of(&meta, Val::Built(Box::new(o), Box::new(state.v)))?;
                    }
                }
            }
            b'0' => {
                m.pop()?; // POP
            }
            b'2' => {
                let n = m.stack.last().cloned().ok_or("DUP of nothing")?;
                m.push_copy(n)?;
            }
            _ => {
                return Err(format!(
                    "pickle opcode 0x{op:02x} is not one a PyTorch file uses"
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nesting past the limit is refused, not built until dropping it overflows the stack.
    #[test]
    fn a_nesting_bomb_is_refused() {
        let mut data = vec![0x80, 2, b'N'];
        data.extend(std::iter::repeat_n(0x85u8, 100_000)); // TUPLE1, over and over
        data.push(b'.');
        let e = load(&data).unwrap_err();
        assert!(e.contains("deep"), "{e}");
        // Within the limit, fine.
        let mut ok = vec![0x80, 2, b'N'];
        ok.extend(std::iter::repeat_n(0x85u8, (MAX_DEPTH - 1) as usize));
        ok.push(b'.');
        assert!(load(&ok).is_ok());
    }

    /// Copies doubling a value (DUP, TUPLE2) are charged by their size: refused long before
    /// they could fill the memory.
    #[test]
    fn a_copying_bomb_is_refused() {
        let mut data = vec![0x80, 2, b'N'];
        for _ in 0..60 {
            data.extend([b'2', 0x86]); // DUP, TUPLE2: twice the size each time
        }
        data.push(b'.');
        let e = load(&data).unwrap_err();
        assert!(e.contains("too many") || e.contains("deep"), "{e}");
        // Memo reads cost what they copy too.
        let mut data = vec![0x80, 2, b']'];
        for i in 0..40u8 {
            data.extend([b'q', i, b'h', i, 0x86]);
        }
        data.push(b'.');
        assert!(load(&data).is_err());
    }

    /// `pickle.dumps({'a': [1, -2, 3.5], 'b': ('x', None, True)}, protocol=2)`.
    #[test]
    fn reads_what_python_writes() {
        let data = b"\x80\x02}q\x00(X\x01\x00\x00\x00aq\x01]q\x02(K\x01J\xfe\xff\xff\xffG@\x0c\x00\x00\x00\x00\x00\x00eX\x01\x00\x00\x00bq\x03X\x01\x00\x00\x00xq\x04N\x88\x87q\x05u.";
        let v = load(data).unwrap();
        let a = v.get("a").unwrap().items().unwrap();
        assert_eq!(a, vec![Val::Int(1), Val::Int(-2), Val::Float(3.5)]);
        let b = v.get("b").unwrap();
        assert_eq!(
            b,
            Val::Tuple(vec![Val::Str("x".into()), Val::None, Val::Bool(true)])
        );
    }

    #[test]
    fn hostile_input_fails_and_never_panics() {
        for data in [
            &b""[..],
            b"\x80\x02",
            b"h\x05.",
            b"e.",
            b"\x80\x02X\xff\xff\xff\xff",
            b"\x80\x02\xff",
            b"(u.",
            b"]K\x01s.",
        ] {
            assert!(load(data).is_err(), "{data:?}");
        }
    }
}

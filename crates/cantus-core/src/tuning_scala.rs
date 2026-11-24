//! Scala `.scl` scales and `.kbm` keyboard maps, read and written (round 42, ADR 0155).
//!
//! The grammar and every quirk are those of `research/instruments/microtuning-verified.md`
//! §1, from the Scala format page and help and the 5 401 files of the archive (version 94):
//! Latin-1 text (strict UTF-8 when Ableton's `! @ABL` lines are present), CRLF, LF or a lone
//! CR, comments anywhere, a blank description, a count of zero, text after a value, tabs,
//! bare integers as ratios, cents with a trailing period, negative cents, ratios below 1/1,
//! degrees in any order, any period, ratio terms past 64 bits (kept as their exact cents).
//! The must-fail cases are those of Surge's MIT `tuning-library` (`tests/data/bad`, committed
//! under `crates/cantus-core/tests/data/tuning-library`): a value that is not one, a blank line
//! or a missing line among the pitches, a blank or garbage mapping entry, a map shorter than
//! its size, a missing header field.
//!
//! Writing gives what Scala writes: CRLF, Latin-1, a name comment, a period on every cents
//! value (so it reads back as cents), the ratio as it was.

use crate::tuning::{KeyboardMap, ScalaScale, ScaleDegree};
use crate::tuning_cmd::MAX_DEGREES;
use std::fmt;

/// Why a file was refused — each a stated reason, with the line (1-based) where it applies.
#[derive(Clone, Debug, PartialEq)]
pub enum ScalaError {
    /// An `@ABL` file that is not UTF-8 (Ableton's rule).
    NotUtf8,
    /// The file ended before `what` (`line` is the last line read).
    Missing { what: &'static str, line: usize },
    /// A line that should hold `what` holds something else.
    Bad {
        what: &'static str,
        line: usize,
        text: String,
    },
    /// More degrees (or map entries) than Cantus keeps ([`MAX_DEGREES`]).
    TooMany { count: usize },
    /// Fewer pitches than the count says.
    FewerPitches { count: usize, found: usize },
    /// A map with fewer entries than its size: Scala lets the last unmapped keys be left out,
    /// but a missing entry in the middle shifts every key after it and cannot be told apart
    /// (Surge's `tuning-library` refuses it too) — write the trailing keys as `x`.
    ShortMap { size: usize, found: usize },
    /// A map that leaves its own reference key unmapped (the Scala help: an error).
    ReferenceUnmapped,
}

impl fmt::Display for ScalaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScalaError::NotUtf8 => write!(f, "a file with Ableton (@ABL) lines must be UTF-8"),
            ScalaError::Missing { what, line } => {
                write!(f, "the file ends after line {line}, before the {what}")
            }
            ScalaError::Bad { what, line, text } => {
                write!(f, "line {line} should hold the {what}: {text:?}")
            }
            ScalaError::TooMany { count } => {
                write!(
                    f,
                    "{count} degrees: more than the {MAX_DEGREES} Cantus keeps"
                )
            }
            ScalaError::FewerPitches { count, found } => {
                write!(f, "the count says {count} pitches, the file has {found}")
            }
            ScalaError::ShortMap { size, found } => write!(
                f,
                "the map's size is {size} but it has {found} entries (write unmapped keys as x)"
            ),
            ScalaError::ReferenceUnmapped => {
                write!(f, "the map leaves its own reference key unmapped")
            }
        }
    }
}

impl std::error::Error for ScalaError {}

/// The UTF-8 byte order mark an editor (Notepad, a spreadsheet's export) may put first.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// The text of a file: strict UTF-8 when it carries Ableton's extensions, Latin-1 otherwise
/// (the Scala page: "ISO 8859-1"; 125 archive files hold bytes ≥ 0x80). A leading UTF-8 byte
/// order mark is dropped and says the file is UTF-8 (r42 fix2-model, review 4); a file that
/// has one but is not UTF-8 after it is read as if it had none.
pub fn decode(bytes: &[u8]) -> Result<String, ScalaError> {
    if let Some(rest) = bytes.strip_prefix(&BOM[..]) {
        if let Ok(text) = std::str::from_utf8(rest) {
            return Ok(text.to_string());
        }
        return decode(rest);
    }
    let abl = bytes.windows(4).any(|w| w == b"@ABL");
    if abl {
        return String::from_utf8(bytes.to_vec()).map_err(|_| ScalaError::NotUtf8);
    }
    Ok(bytes.iter().map(|&b| char::from(b)).collect())
}

/// The lines of a text, with CRLF, LF or a lone CR as ends (no empty line after the last
/// end); numbered from 1.
fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let text = text.strip_suffix('\r').unwrap_or(text);
    text.lines()
        .flat_map(|l| l.split('\r'))
        .enumerate()
        .map(|(i, l)| (i + 1, l))
}

/// Space and tab — the only blanks the format knows.
fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn trim(l: &str) -> &str {
    l.trim_matches(is_blank)
}

/// The leading run of ASCII digits of `s` and what follows.
fn digits(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(end)
}

/// One pitch value at the start of `line` (after blanks); anything after it is ignored.
pub fn parse_degree(line: &str) -> Option<ScaleDegree> {
    let s = line.trim_start_matches(is_blank);
    let (neg, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (int, rest) = digits(body);
    if let Some(frac) = rest.strip_prefix('.') {
        // A period: cents (`408.`, `.5` and `-5.0` all are).
        let (fd, _) = digits(frac);
        if int.is_empty() && fd.is_empty() {
            return None;
        }
        let text = format!("{}{}.{}", if neg { "-" } else { "" }, int, fd);
        let c: f64 = text.parse().ok()?;
        return c.is_finite().then_some(ScaleDegree::Cents(c));
    }
    if int.is_empty() || neg {
        // Nothing numeric, or a negative ratio ("meaningless", a read error).
        return None;
    }
    let n: u128 = int.parse().ok()?;
    let d: u128 = match rest.strip_prefix('/') {
        Some(den) => {
            let (dd, _) = digits(den);
            if dd.is_empty() {
                return None;
            }
            dd.parse().ok()?
        }
        None => 1,
    };
    if n == 0 || d == 0 {
        return None;
    }
    Some(ratio(n, d))
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// A ratio as written; reduced only when its terms pass 64 bits, and kept as its exact
/// cents when even the reduced terms do (8 lines of the archive, `atomschis.scl`).
fn ratio(n: u128, d: u128) -> ScaleDegree {
    if let (Ok(a), Ok(b)) = (u64::try_from(n), u64::try_from(d)) {
        return ScaleDegree::Ratio(a, b);
    }
    let g = gcd(n, d);
    let (n, d) = (n / g, d / g);
    if let (Ok(a), Ok(b)) = (u64::try_from(n), u64::try_from(d)) {
        return ScaleDegree::Ratio(a, b);
    }
    ScaleDegree::Cents(1200.0 * (log2_u128(n) - log2_u128(d)))
}

/// `log2(x)` for a 128-bit integer to f64 precision: its top 64 bits and the shift.
fn log2_u128(x: u128) -> f64 {
    let bits = 128 - x.leading_zeros();
    let shift = bits.saturating_sub(64);
    let top = (x >> shift) as u64;
    cantus_dsp::math::log2(top as f64) + f64::from(shift)
}

/// Read a `.scl` file's bytes.
pub fn read_scl(bytes: &[u8]) -> Result<ScalaScale, ScalaError> {
    let text = decode(bytes)?;
    let mut description: Option<String> = None;
    let mut count: Option<usize> = None;
    let mut degrees = Vec::new();
    let mut last = 0;
    for (n, line) in lines(&text) {
        last = n;
        if line.starts_with('!') {
            continue;
        }
        if description.is_none() {
            // The first non-comment line, possibly empty, is the description; a blank before
            // a `!` is the escape [`write_scl`] writes, and is dropped (r42 fix2-model).
            let escaped = line.starts_with(is_blank)
                && line.trim_start_matches(is_blank).starts_with('!');
            description = Some(if escaped { &line[1..] } else { line }.to_string());
            continue;
        }
        let Some(c) = count else {
            let (d, _) = digits(line.trim_start_matches(is_blank));
            let c: usize = d.parse().map_err(|_| ScalaError::Bad {
                what: "number of notes",
                line: n,
                text: line.to_string(),
            })?;
            if c > MAX_DEGREES {
                return Err(ScalaError::TooMany { count: c });
            }
            count = Some(c);
            degrees.reserve(c);
            continue;
        };
        if degrees.len() == c {
            // Trailing data after the pitches is ignored (the archive's comments; Surge's
            // `bad/extraline.scl`).
            break;
        }
        if trim(line).is_empty() {
            return Err(ScalaError::Bad {
                what: "pitch value (a blank line)",
                line: n,
                text: String::new(),
            });
        }
        let d = parse_degree(line).ok_or_else(|| ScalaError::Bad {
            what: "pitch value",
            line: n,
            text: line.to_string(),
        })?;
        degrees.push(d);
    }
    let Some(c) = count else {
        return Err(ScalaError::Missing {
            what: if description.is_none() {
                "description"
            } else {
                "number of notes"
            },
            line: last,
        });
    };
    if degrees.len() < c {
        return Err(ScalaError::FewerPitches {
            count: c,
            found: degrees.len(),
        });
    }
    Ok(ScalaScale {
        description: description.unwrap_or_default(),
        degrees,
    })
}

/// A cents value as Scala writes one: always with a period, so it reads back as cents.
pub fn format_cents(c: f64) -> String {
    let s = format!("{c}");
    if s.contains('.') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// A written file's text as the bytes to save: Latin-1, as the Scala page asks (a character
/// past U+00FF becomes `?`).
pub fn latin1(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

/// Write a `.scl` (CRLF line ends, as Scala writes them); save it with [`latin1`].
pub fn write_scl(scale: &ScalaScale) -> String {
    let one_line = |s: &str| s.replace(['\r', '\n'], " ");
    let mut t = String::new();
    t.push_str("! Written by Cantus\r\n!\r\n");
    // A description that starts with `!` (after blanks) would read as a comment: a blank
    // before it keeps it the description, and the reader drops that blank (r42 fix2-model,
    // review 5).
    let description = one_line(&scale.description);
    if description.trim_start_matches(is_blank).starts_with('!') {
        t.push(' ');
    }
    t.push_str(&description);
    t.push_str("\r\n");
    t.push_str(&format!(" {}\r\n!\r\n", scale.degrees.len()));
    for d in &scale.degrees {
        match *d {
            ScaleDegree::Cents(c) => t.push_str(&format!(" {}\r\n", format_cents(c))),
            ScaleDegree::Ratio(n, d) => t.push_str(&format!(" {n}/{d}\r\n")),
        }
    }
    t
}

/// Read a `.kbm` file's bytes.
pub fn read_kbm(bytes: &[u8]) -> Result<KeyboardMap, ScalaError> {
    const FIELDS: [&str; 7] = [
        "map size",
        "first key",
        "last key",
        "middle key",
        "reference key",
        "reference frequency",
        "formal octave degree",
    ];
    let text = decode(bytes)?;
    let mut header: Vec<f64> = Vec::with_capacity(7);
    let mut mapping: Vec<Option<u32>> = Vec::new();
    let mut size = 0usize;
    let mut last = 0;
    for (n, line) in lines(&text) {
        last = n;
        if line.starts_with('!') {
            continue;
        }
        let t = trim(line);
        if header.len() < 7 {
            let what = FIELDS[header.len()];
            let bad = || ScalaError::Bad {
                what,
                line: n,
                text: line.to_string(),
            };
            let token = t.split(is_blank).next().unwrap_or("");
            let v = if header.len() == 5 {
                let f: f64 = token.parse().map_err(|_| bad())?;
                if !(f.is_finite() && f > 0.0) {
                    return Err(bad());
                }
                f
            } else {
                let (d, _) = digits(token);
                let v: u32 = d.parse().map_err(|_| bad())?;
                let key_field = (1..=4).contains(&header.len());
                if key_field && v > 127 {
                    return Err(bad());
                }
                f64::from(v)
            };
            header.push(v);
            if header.len() == 1 {
                size = v as usize;
                if size > MAX_DEGREES {
                    return Err(ScalaError::TooMany { count: size });
                }
            }
            continue;
        }
        if mapping.len() == size {
            // Trailing lines after the map (blank or not) are ignored.
            break;
        }
        let entry = t.split(is_blank).next().unwrap_or("");
        let bad = || ScalaError::Bad {
            what: "mapping entry (a degree or x)",
            line: n,
            text: line.to_string(),
        };
        if entry.eq_ignore_ascii_case("x") {
            mapping.push(None);
        } else {
            let (d, rest) = digits(entry);
            if d.is_empty() || !rest.is_empty() {
                return Err(bad());
            }
            mapping.push(Some(d.parse().map_err(|_| bad())?));
        }
    }
    if header.len() < 7 {
        return Err(ScalaError::Missing {
            what: FIELDS[header.len()],
            line: last,
        });
    }
    if mapping.len() < size {
        return Err(ScalaError::ShortMap {
            size,
            found: mapping.len(),
        });
    }
    let key = |i: usize| header[i] as u8;
    let map = KeyboardMap {
        size: size as u32,
        first_key: key(1),
        last_key: key(2),
        middle_key: key(3),
        reference_key: key(4),
        reference_hz: header[5],
        octave_degree: header[6] as u32,
        mapping,
    };
    if map.size > 0 && map.degree_of(map.reference_key, 1).is_none() {
        return Err(ScalaError::ReferenceUnmapped);
    }
    Ok(map)
}

/// Write a `.kbm` (CRLF), with Scala's template comments; save it with [`latin1`].
pub fn write_kbm(map: &KeyboardMap) -> String {
    let mut t = String::new();
    let mut field = |comment: &str, value: String| {
        t.push_str(&format!("! {comment}\r\n{value}\r\n"));
    };
    field("Size of map:", map.size.to_string());
    field(
        "First MIDI note number to retune:",
        map.first_key.to_string(),
    );
    field("Last MIDI note number to retune:", map.last_key.to_string());
    field(
        "Middle note where the first entry of the mapping is mapped to:",
        map.middle_key.to_string(),
    );
    field(
        "Reference note for which frequency is given:",
        map.reference_key.to_string(),
    );
    field(
        "Frequency to tune the above note to (floating point e.g. 440.0):",
        format_cents(map.reference_hz),
    );
    field(
        "Scale degree to consider as formal octave:",
        map.octave_degree.to_string(),
    );
    t.push_str("! Mapping.\r\n");
    for e in &map.mapping {
        match e {
            Some(d) => t.push_str(&format!("{d}\r\n")),
            None => t.push_str("x\r\n"),
        }
    }
    t
}

/// Ableton's `! @ABL REFERENCE_PITCH <octave> <index> <Hz>` line of an `.ascl` file (Ableton's
/// published `.ascl` specification): scale degree `index` sounds at `Hz`. Read as Surge's MIT
/// `tuning-library` reads it (`readASCLStream`, tested there against Ableton-generated `.kbm`
/// files): the scale position nearest middle C of 12-TET (261.6256 Hz) goes on key 60, the
/// map is linear over every key with the scale's own period, and the reference degree's key
/// sounds at `Hz` — the octave number adds nothing the frequency does not say. `None`: no
/// such line (a plain `.scl`, or an `.ascl` without one). A line that is not three numbers,
/// an octave past 32 bits, an index outside the scale, a frequency that is not positive or a
/// scale whose period is not a positive interval is refused. r42 fix2-model (review 2).
pub fn read_ascl_map(
    bytes: &[u8],
    scale: &ScalaScale,
) -> Result<Option<KeyboardMap>, ScalaError> {
    let text = decode(bytes)?;
    let mut found = None;
    for (n, line) in lines(&text) {
        let Some(rest) = line.strip_prefix('!') else {
            continue;
        };
        let mut words = rest.split(is_blank).filter(|w| !w.is_empty());
        if words.next() != Some("@ABL") || words.next() != Some("REFERENCE_PITCH") {
            continue;
        }
        let bad = || ScalaError::Bad {
            what: "reference pitch (@ABL REFERENCE_PITCH)",
            line: n,
            text: line.to_string(),
        };
        let args: Vec<&str> = words.collect();
        if args.len() != 3 {
            return Err(bad());
        }
        let _octave: i32 = args[0].parse().map_err(|_| bad())?;
        let index: usize = args[1].parse().map_err(|_| bad())?;
        let hz: f64 = args[2].parse().map_err(|_| bad())?;
        let cents: Vec<f64> = scale.degrees.iter().map(|d| d.cents()).collect();
        let period = cents.last().copied().unwrap_or(0.0);
        if !(hz.is_finite() && hz > 0.0)
            || index >= cents.len()
            || !(period.is_finite() && period > 0.0)
        {
            return Err(bad());
        }
        found = Some((index as i64, hz, cents, period));
    }
    let Some((index, hz, cents, period)) = found else {
        return Ok(None);
    };
    let n = cents.len() as i64;
    let at = |p: i64| {
        let (q, r) = (p.div_euclid(n), p.rem_euclid(n));
        q as f64 * period + if r == 0 { 0.0 } else { cents[(r - 1) as usize] }
    };
    // Middle C in cents above degree 0, given the reference; the position nearest it.
    let target =
        at(index) + 1200.0 * cantus_dsp::math::log2(crate::tuning::C4_DEFAULT_HZ / hz);
    let q0 = (target / period).floor() as i64;
    let mut middle = 0i64;
    let mut best = f64::INFINITY;
    for p in (q0 - 1) * n..(q0 + 2) * n {
        let d = (at(p) - target).abs();
        if d < best {
            (best, middle) = (d, p);
        }
    }
    let key = |p: i64| (60 + p - middle).clamp(0, 127) as u8;
    Ok(Some(KeyboardMap {
        size: n as u32,
        first_key: 0,
        last_key: 127,
        middle_key: key(0),
        reference_key: key(index),
        reference_hz: hz,
        octave_degree: n as u32,
        mapping: (0..n as u32).map(Some).collect(),
    }))
}

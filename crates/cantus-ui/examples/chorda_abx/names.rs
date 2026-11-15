//! The kit's blinding: random names, and the answer table kept out of plain sight.
//!
//! Kit 4 named each file by a salted FNV of its phrase and piano: the salt was shared and the
//! inputs differed by a byte, so the files of one phrase shared 15 of 16 hex digits and a look
//! at the URLs grouped them and showed the odd one out. Kit 5 draws every name and every token
//! from a keyed PRF (SipHash under the standard library's per-process random key, applied to a
//! counter): no name carries anything of what it names, and `names_look_random` checks it.

use std::hash::{BuildHasher, Hasher};

/// Random 64-bit words: SipHash under a key the OS drew for this process, over a counter.
pub struct Rng {
    key: std::collections::hash_map::RandomState,
    n: u64,
}

impl Rng {
    pub fn new() -> Self {
        Rng {
            key: std::collections::hash_map::RandomState::new(),
            n: 0,
        }
    }
    pub fn next(&mut self) -> u64 {
        let mut h = self.key.build_hasher();
        h.write_u64(self.n);
        self.n += 1;
        h.finish()
    }
    /// A uniform integer below `n` (n small: the modulo bias is under 2^-50).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    /// 16 hex digits: a file's name.
    pub fn name(&mut self) -> String {
        format!("{:016x}", self.next())
    }
    /// 8 hex digits: a piano's token in the page.
    pub fn token(&mut self) -> String {
        format!("{:08x}", self.next() >> 32)
    }
}

/// Whether names (equal-length hex strings) carry structure: any two sharing a prefix of more
/// than `PREFIX` digits, or agreeing in more than `SAME` of their positions. For 16 random hex
/// digits a pair shares one position on average; across the ~250 pairs of a kit the chance of
/// any pair reaching 10 is ~10⁻⁶, a 7-digit prefix ~10⁻⁶ (a kit redraws if it happens) —
/// kit 4's pairs shared 15.
pub const PREFIX: usize = 6;
pub const SAME: usize = 9;

pub fn names_look_random(names: &[String]) -> Result<(), String> {
    for (i, a) in names.iter().enumerate() {
        for b in &names[i + 1..] {
            if a == b {
                return Err(format!("{a} twice"));
            }
            let prefix = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
            let same = a.bytes().zip(b.bytes()).filter(|(x, y)| x == y).count();
            if prefix > PREFIX || same > SAME {
                return Err(format!(
                    "{a} and {b} share {same} digits ({prefix} leading)"
                ));
            }
        }
    }
    Ok(())
}

/// The page's answer table: `text`'s bytes XOR a xorshift32 stream from `seed`, in base64 —
/// not secret (the page decodes it), only out of a glance at the source. The page's `unveil`
/// is the inverse.
pub fn veil(text: &str, seed: u32) -> String {
    let mut x = seed.max(1);
    let bytes: Vec<u8> = text
        .bytes()
        .map(|b| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            b ^ (x >> 24) as u8
        })
        .collect();
    base64(&bytes)
}

fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                o.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                o.push('=');
            }
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawn_names_look_random() {
        // Many kits' worth: a kit's 23 names, a hundred times (all pass but once in ~10⁴ runs).
        let mut r = Rng::new();
        for _ in 0..100 {
            let names: Vec<String> = (0..23).map(|_| r.name()).collect();
            names_look_random(&names).unwrap();
        }
    }

    #[test]
    fn kit_four_names_are_caught() {
        // Kit 4's 21 names, as written: caught.
        let kit4 = "11e73706bbfbd9ca 2c0dba9592e70815 2c0dbb9592e702c2 34dc5adcf31ef7f6             34dc5bdcf31efc25 3910eabe1ac487e0 3910ebbe1ac48e2f 6068de972b108ac1 64902536394bf816             6a0c41056e21bbfd 6a0c42056e21a54e 721371f9b80be8bd 935713bde173fbfd bcbdb888aa0a7fd2             bcbdb988aa0a7101 c6f5666ed294a56c c743c32c9b02ad05 c743c42c9b029756 d20dac92f1875a54             e1c33811d665b63e e1c33911d665b08d";
        let names: Vec<String> = kit4.split_whitespace().map(String::from).collect();
        assert!(names_look_random(&names).is_err());
        // And each of its phrase pairs on its own.
        assert!(names_look_random(&names[1..3]).is_err());
        assert!(names_look_random(&names[19..21]).is_err());
    }

    #[test]
    fn veil_is_base64_of_the_stream() {
        // RFC 4648's vectors; the veil is not the plain text's base64.
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"f"), "Zg==");
        assert_ne!(veil("{\"a\":1}", 7), base64(b"{\"a\":1}"));
    }
}

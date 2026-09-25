//! Small conversions shared by the VST3 backend: strings in the SDK's fixed arrays, class
//! ids as text, and result codes.

use ::vst3::Steinberg::{char16, char8, kResultOk, tresult, TUID};

/// A NUL-terminated `char8` array (ASCII or UTF-8) as a Rust string.
pub fn from_char8(chars: &[char8]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A NUL-terminated UTF-16 array as a Rust string.
pub fn from_char16(chars: &[char16]) -> String {
    let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..end])
}

/// Write `s` into a UTF-16 array, truncated and always NUL-terminated.
pub fn to_char16(s: &str, dst: &mut [char16]) {
    if dst.is_empty() {
        return;
    }
    let mut n = 0;
    for unit in s.encode_utf16() {
        if n + 1 >= dst.len() {
            break;
        }
        dst[n] = unit;
        n += 1;
    }
    dst[n] = 0;
}

/// Length of a NUL-terminated UTF-16 string.
///
/// # Safety
/// `s` must point to a NUL-terminated UTF-16 string.
pub unsafe fn char16_len(s: *const char16) -> usize {
    let mut n = 0;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}

/// Whether a result code means success (`kResultOk`, which is also `kResultTrue`).
pub fn ok(r: tresult) -> bool {
    r == kResultOk
}

/// A class id as the SDK prints it (`FUID::toString`): 32 upper-case hex digits of the
/// four 32-bit words given to `DECLARE_UID`/`uid()`. The text is the same on every
/// platform although the bytes of a `TUID` are not (Windows stores them COM-style), so a
/// project saved on one system finds the plug-in on another, and it matches the `CID`
/// strings of `moduleinfo.json`.
pub fn tuid_to_string(tuid: &TUID) -> String {
    let b: Vec<u8> = tuid.iter().map(|&c| c as u8).collect();
    let words: [u32; 4] = if cfg!(target_os = "windows") {
        [
            u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            (u32::from(b[5]) << 24)
                | (u32::from(b[4]) << 16)
                | (u32::from(b[7]) << 8)
                | u32::from(b[6]),
            u32::from_be_bytes([b[8], b[9], b[10], b[11]]),
            u32::from_be_bytes([b[12], b[13], b[14], b[15]]),
        ]
    } else {
        [
            u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            u32::from_be_bytes([b[4], b[5], b[6], b[7]]),
            u32::from_be_bytes([b[8], b[9], b[10], b[11]]),
            u32::from_be_bytes([b[12], b[13], b[14], b[15]]),
        ]
    };
    format!(
        "{:08X}{:08X}{:08X}{:08X}",
        words[0], words[1], words[2], words[3]
    )
}

/// The inverse of [`tuid_to_string`]; `None` unless the text is exactly 32 hex digits.
pub fn tuid_from_string(s: &str) -> Option<TUID> {
    let s = s.trim();
    if s.len() != 32 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let word = |i: usize| u32::from_str_radix(&s[i * 8..i * 8 + 8], 16).ok();
    Some(::vst3::uid(word(0)?, word(1)?, word(2)?, word(3)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_ids_print_as_the_sdk_declares_them() {
        let tuid = ::vst3::uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF318797D);
        let text = tuid_to_string(&tuid);
        assert_eq!(text, "6E33225254224A00AA69301AF318797D");
        assert_eq!(tuid_from_string(&text), Some(tuid));
        assert_eq!(
            tuid_from_string(&text.to_lowercase()),
            Some(tuid),
            "hex digits in either case"
        );
        assert_eq!(tuid_from_string("6E33"), None);
    }

    #[test]
    fn utf16_round_trip_truncates_and_terminates() {
        let mut buf = [0u16; 6];
        to_char16("Gain dB", &mut buf);
        assert_eq!(from_char16(&buf), "Gain ");
        assert_eq!(buf[5], 0);
        let chars: Vec<char8> = b"Fx|EQ\0junk".iter().map(|&b| b as char8).collect();
        assert_eq!(from_char8(&chars), "Fx|EQ");
    }
}

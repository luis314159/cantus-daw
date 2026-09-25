//! What a musician types into the EQ's boxes, understood: `2k`, `2.5 kHz`, `440 Hz`, `A4`,
//! `C#2+13` (a note and cents) for a frequency; `-3`, `+4.5 dB`, `2x` (a gain ratio) for a
//! gain; a plain number for Q. Middle C is C4 (MIDI 60), as the piano roll names it.

/// The frequency of MIDI note `n` plus `cents`.
pub fn note_hz(midi: f32, cents: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0 + cents / 100.0) / 12.0)
}

/// The nearest note to `hz` and how far off it is, in cents.
pub fn note_of(hz: f32) -> (i32, i32) {
    let midi = 69.0 + 12.0 * (hz.max(1.0) / 440.0).log2();
    let n = midi.round();
    (n as i32, ((midi - n) * 100.0).round() as i32)
}

/// `A4`, `A4 +3`.
pub fn note_label(hz: f32) -> String {
    let (n, cents) = note_of(hz);
    let name = cantus_core::pitch_name(n.clamp(0, 127) as u8);
    if cents == 0 {
        name
    } else {
        format!("{name} {cents:+}")
    }
}

/// A note name with an optional octave (default 4) and cents: `A4`, `c#2+13`, `Bb3 -20`,
/// `C-1`. The letter must come first.
pub fn parse_note(s: &str) -> Option<f32> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut chars = s.chars().peekable();
    let letter = chars.next()?.to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut semis: i32 = base;
    match chars.peek() {
        Some('#') | Some('♯') => {
            semis += 1;
            chars.next();
        }
        Some('b') | Some('♭') => {
            semis -= 1;
            chars.next();
        }
        _ => {}
    }
    let rest: String = chars.collect();
    // The octave is the integer before any cents: "-1", "4", "4+13", "-1-20".
    let (octave, cents) = split_octave_cents(&rest)?;
    let midi = (octave + 1) * 12 + semis;
    Some(note_hz(midi as f32, cents))
}

fn split_octave_cents(rest: &str) -> Option<(i32, f32)> {
    if rest.is_empty() {
        return Some((4, 0.0));
    }
    let bytes = rest.as_bytes();
    // A leading '-' belongs to the octave only when a digit follows it.
    let mut i = 0;
    if bytes[0] == b'-' && bytes.len() > 1 && bytes[1].is_ascii_digit() {
        i = 1;
    }
    let start_digits = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let octave = if i > start_digits {
        rest[..i].parse::<i32>().ok()?
    } else {
        4
    };
    let tail = &rest[i..];
    let tail = tail.trim_end_matches("ct").trim_end_matches('c');
    let cents = if tail.is_empty() {
        0.0
    } else {
        tail.parse::<f32>().ok()?
    };
    if !(-1..=9).contains(&octave) || cents.abs() > 100.0 {
        return None;
    }
    Some((octave, cents))
}

/// A frequency: a number with an optional `k` (or `kHz`, `Hz`), or a note.
pub fn parse_freq(s: &str) -> Option<f32> {
    let t = s.trim().to_lowercase().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    let t = t.trim_end_matches("hz").trim();
    let (num, mul) = if let Some(n) = t.strip_suffix('k') {
        (n.trim(), 1_000.0)
    } else {
        (t, 1.0)
    };
    if let Ok(v) = num.parse::<f32>() {
        return (v > 0.0 && v.is_finite()).then_some(v * mul);
    }
    parse_note(s.trim())
}

/// A gain in dB: `-3`, `+4.5`, `4.5 dB`, or a ratio — `2x` is +6.02 dB, `0.5x` −6.02.
pub fn parse_gain(s: &str) -> Option<f32> {
    let t = s.trim().to_lowercase().replace(',', ".");
    let t = t.trim_end_matches("db").trim();
    if let Some(r) = t.strip_suffix('x').or_else(|| t.strip_suffix('×')) {
        let r: f32 = r.trim().parse().ok()?;
        return (r > 0.0).then(|| 20.0 * r.log10());
    }
    let v: f32 = t.parse().ok()?;
    v.is_finite().then_some(v)
}

/// A Q: a positive number.
pub fn parse_q(s: &str) -> Option<f32> {
    let t = s.trim().replace(',', ".");
    let t = t.trim_start_matches(['q', 'Q']).trim();
    let v: f32 = t.parse().ok()?;
    (v > 0.0 && v.is_finite()).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Option<f32>, b: f32) -> bool {
        a.is_some_and(|a| (a - b).abs() <= b.abs() * 1e-4 + 1e-4)
    }

    #[test]
    fn frequencies_are_read_the_way_they_are_said() {
        assert!(close(parse_freq("2k"), 2_000.0));
        assert!(close(parse_freq("2.5 kHz"), 2_500.0));
        assert!(close(parse_freq("440"), 440.0));
        assert!(close(parse_freq("440Hz"), 440.0));
        assert!(close(parse_freq("1,2k"), 1_200.0));
        assert!(close(parse_freq("A4"), 440.0));
        assert!(close(parse_freq("a4"), 440.0));
        assert!(close(parse_freq("C4"), 261.6256));
        assert!(close(
            parse_freq("C#2+13"),
            69.2957 * 2f32.powf(13.0 / 1200.0)
        ));
        assert!(close(
            parse_freq("Bb3 -20"),
            233.0819 * 2f32.powf(-20.0 / 1200.0)
        ));
        assert!(close(parse_freq("C-1"), 8.1758));
        assert!(close(parse_freq("E"), 329.6276));
        assert_eq!(parse_freq("H4"), None);
        assert_eq!(parse_freq(""), None);
        assert_eq!(parse_freq("-5"), None);
        assert_eq!(parse_freq("A4+300"), None);
    }

    #[test]
    fn gains_and_q_are_read_too() {
        assert!(close(parse_gain("-3"), -3.0));
        assert!(close(parse_gain("+4.5 dB"), 4.5));
        assert!(close(parse_gain("2x"), 6.0206));
        assert!(close(parse_gain("0.5x"), -6.0206));
        assert_eq!(parse_gain("x"), None);
        assert!(close(parse_q("0.71"), 0.71));
        assert!(close(parse_q("Q 4"), 4.0));
        assert_eq!(parse_q("0"), None);
    }

    #[test]
    fn notes_round_trip() {
        for hz in [27.5f32, 110.0, 261.6256, 1_000.0, 4_186.0] {
            let (n, c) = note_of(hz);
            assert!((note_hz(n as f32, c as f32) - hz).abs() / hz < 0.001);
        }
        assert_eq!(note_label(440.0), "A4");
        assert_eq!(note_label(445.0), "A4 +20");
    }
}

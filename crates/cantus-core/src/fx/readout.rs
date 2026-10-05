//! How a parameter's value reads, and how a typed one is understood — one place, so the
//! DAW's knobs and the exported plug-ins (ADR 0020) print and parse the same text.

/// The readout of a number: no more digits than a musician reads. `1.2 kHz`, `350 Hz`,
/// `0.71`, `-3.5 dB`, `12 ms`.
pub fn readout(v: f32, unit: &str) -> String {
    match unit {
        "Hz" if v >= 1_000.0 => format!("{:.1} kHz", v / 1_000.0),
        "Hz" => format!("{v:.0} Hz"),
        "ms" if v >= 1_000.0 => format!("{:.2} s", v / 1_000.0),
        "ms" if v >= 100.0 => format!("{v:.0} ms"),
        "ms" => format!("{v:.1} ms"),
        "s" if v < 1.0 => format!("{:.0} ms", v * 1_000.0),
        "s" => format!("{v:.2} s"),
        "dB" => format!("{v:+.1} dB"),
        "st" => format!("{v:+.1} st"),
        "ct" => format!("{v:+.0} ct"),
        "%" => format!("{:.0}%", v * 100.0),
        "\u{b0}" => format!("{v:+.1}\u{b0}"),
        "" if v.abs() >= 100.0 => format!("{v:.0}"),
        "" => format!("{v:.2}"),
        u => format!("{v:.2} {u}"),
    }
}

/// Read a typed value in a parameter's unit: what [`readout`] prints reads back, and so do
/// the short forms people type — `2k` or `2 kHz` for 2000 Hz, `1.5 s` for 1500 ms, `50%`
/// or `50` for 0.5 of a `%` parameter, `-6` for −6 dB. `None` when it is not a number.
pub fn parse_readout(text: &str, unit: &str) -> Option<f32> {
    let lower = text.trim().to_lowercase();
    let mut s = lower.as_str();
    let mut scale = 1.0f32;
    // The most specific suffixes first: `khz` before `hz`, `db/oct` before `ct`, `ms` and
    // `lufs` before `s`.
    for suffix in [
        "khz", "hz", "db/oct", "ms", "db", "lufs", "st", "ct", ":1", "%", "\u{b0}", "x", "s", "k",
    ] {
        if let Some(rest) = s.strip_suffix(suffix) {
            s = rest.trim_end();
            scale = match suffix {
                "khz" | "k" => 1_000.0,
                // Seconds typed into a parameter in milliseconds, and the other way round.
                "s" if unit == "ms" => 1_000.0,
                "ms" if unit == "s" => 0.001,
                _ => 1.0,
            };
            break;
        }
    }
    let s = s.trim().replace(',', ".").replace('\u{2212}', "-");
    let v: f32 = s.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    let v = v * scale;
    // A `%` parameter stores 0…1 and prints 0…100 %: a typed number is read as printed.
    Some(if unit == "%" { v / 100.0 } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_printed_reads_back() {
        for (v, unit) in [
            (1_200.0, "Hz"),
            (350.0, "Hz"),
            (-3.5, "dB"),
            (12.0, "ms"),
            (1_500.0, "ms"),
            (0.5, "%"),
            (4.0, ":1"),
            (-14.0, "LUFS"),
            (-22.5, "\u{b0}"),
        ] {
            let text = readout(v, unit);
            let back = parse_readout(&text, unit).unwrap();
            assert!(
                (back - v).abs() <= v.abs() * 0.01 + 1e-6,
                "{text} read {back}"
            );
        }
    }

    #[test]
    fn the_short_forms_people_type() {
        assert_eq!(parse_readout("2k", "Hz"), Some(2_000.0));
        assert_eq!(parse_readout("2 kHz", "Hz"), Some(2_000.0));
        assert_eq!(parse_readout("1.5 s", "ms"), Some(1_500.0));
        assert_eq!(parse_readout("-6", "dB"), Some(-6.0));
        assert_eq!(parse_readout("50%", "%"), Some(0.5));
        assert_eq!(parse_readout("50", "%"), Some(0.5));
        assert_eq!(parse_readout("0,5", ""), Some(0.5));
        assert_eq!(parse_readout("loud", "dB"), None);
    }
}

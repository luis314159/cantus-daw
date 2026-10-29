//! MusicXML 4.0, partwise: the logical score as MuseScore, Dorico, Sibelius and Finale
//! read it — every value, tie, tuplet, beam, stem, accidental, clef, key, metre, tempo,
//! octave line and dynamic as the engraver wrote it, and each note's velocity in the
//! note's `dynamics` attribute (a percentage of MIDI forte, 90) so it reads back exactly.

use crate::model::{BeamMark, Dur, Kind, Part, Score, Stem};
use std::fmt::Write;

/// What the file says about itself.
#[derive(Clone, Debug, Default)]
pub struct XmlOptions {
    /// `<software>`: e.g. "Cantus 0.1.0".
    pub software: String,
    /// `<encoding-date>` (YYYY-MM-DD); none for a file that must not change by the day.
    pub date: Option<String>,
}

/// Escape text for XML.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a.abs()
    } else {
        gcd(b, a % b)
    }
}

/// Divisions per quarter: the coarsest that writes every length and start as a whole
/// number.
fn divisions(score: &Score) -> i64 {
    let q = cantus_core::time::PPQ;
    let mut g = q;
    for b in &score.bars {
        g = gcd(g, b.len);
    }
    for p in &score.parts {
        for st in &p.staves {
            for bar in &st.bars {
                for v in &bar.voices {
                    for e in &v.events {
                        g = gcd(g, e.len);
                        g = gcd(g, e.start);
                    }
                }
            }
        }
    }
    q / g.max(1)
}

fn accidental_name(alter: i8) -> &'static str {
    match alter {
        -2 => "flat-flat",
        -1 => "flat",
        1 => "sharp",
        2 => "double-sharp",
        _ => "natural",
    }
}

/// Semitones to diatonic steps, for `<transpose>`.
fn diatonic(semitones: i32) -> i32 {
    const D: [i32; 12] = [0, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6];
    let o = semitones.div_euclid(12);
    o * 7 + D[semitones.rem_euclid(12) as usize]
}

/// The score as a MusicXML 4.0 partwise document.
pub fn write(score: &Score, opts: &XmlOptions) -> String {
    let mut x = String::with_capacity(64 * 1024);
    let div = divisions(score);
    let q = cantus_core::time::PPQ;
    let d = |ticks: i64| ticks * div / q;
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    x.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    x.push_str("<score-partwise version=\"4.0\">\n");
    if !score.title.is_empty() {
        let _ = writeln!(
            x,
            "  <work>\n    <work-title>{}</work-title>\n  </work>",
            escape(&score.title)
        );
    }
    x.push_str("  <identification>\n");
    if !score.composer.is_empty() {
        let _ = writeln!(
            x,
            "    <creator type=\"composer\">{}</creator>",
            escape(&score.composer)
        );
    }
    x.push_str("    <encoding>\n");
    if !opts.software.is_empty() {
        let _ = writeln!(x, "      <software>{}</software>", escape(&opts.software));
    }
    if let Some(date) = &opts.date {
        let _ = writeln!(x, "      <encoding-date>{}</encoding-date>", escape(date));
    }
    for e in ["accidental", "beam", "stem"] {
        let _ = writeln!(x, "      <supports element=\"{e}\" type=\"yes\"/>");
    }
    x.push_str("    </encoding>\n  </identification>\n");
    x.push_str("  <part-list>\n");
    for (i, p) in score.parts.iter().enumerate() {
        let _ = writeln!(
            x,
            "    <score-part id=\"P{}\">\n      <part-name>{}</part-name>\n    </score-part>",
            i + 1,
            escape(&p.name)
        );
    }
    x.push_str("  </part-list>\n");
    for (pi, part) in score.parts.iter().enumerate() {
        let _ = writeln!(x, "  <part id=\"P{}\">", pi + 1);
        write_part(&mut x, score, part, div, &d);
        x.push_str("  </part>\n");
    }
    x.push_str("</score-partwise>\n");
    x
}

fn write_part(x: &mut String, score: &Score, part: &Part, div: i64, d: &dyn Fn(i64) -> i64) {
    let staves = part.staves.len();
    // Octave lines: where each starts and stops, by (staff, bar, voice, event).
    let mut ottava_start = std::collections::HashMap::new();
    let mut ottava_stop = std::collections::HashMap::new();
    for (si, st) in part.staves.iter().enumerate() {
        for (n, o) in st.ottavas.iter().enumerate() {
            ottava_start.insert((si, o.from.0, o.from.1, o.from.2), (o.shift, n + 1));
            ottava_stop.insert((si, o.to.0, o.to.1, o.to.2), (o.shift, n + 1));
        }
    }
    let last = score.bars.len().saturating_sub(1);
    for (bi, bar) in score.bars.iter().enumerate() {
        let _ = writeln!(x, "    <measure number=\"{}\">", bar.number);
        if bi == 0 || bar.show_sig {
            x.push_str("      <attributes>\n");
            if bi == 0 {
                let _ = writeln!(x, "        <divisions>{div}</divisions>");
                let _ = writeln!(
                    x,
                    "        <key>\n          <fifths>{}</fifths>\n          <mode>{}</mode>\n        </key>",
                    part.key.fifths,
                    part.key.mode.musicxml()
                );
            }
            if bar.show_sig {
                let _ = writeln!(
                    x,
                    "        <time>\n          <beats>{}</beats>\n          <beat-type>{}</beat-type>\n        </time>",
                    bar.sig.numerator, bar.sig.denominator
                );
            }
            if bi == 0 {
                if staves > 1 {
                    let _ = writeln!(x, "        <staves>{staves}</staves>");
                }
                for (si, st) in part.staves.iter().enumerate() {
                    let (sign, line, oct) = st.clef.musicxml();
                    let num = if staves > 1 {
                        format!(" number=\"{}\"", si + 1)
                    } else {
                        String::new()
                    };
                    let _ = write!(x, "        <clef{num}>\n          <sign>{sign}</sign>\n          <line>{line}</line>\n");
                    if oct != 0 {
                        let _ = writeln!(
                            x,
                            "          <clef-octave-change>{oct}</clef-octave-change>"
                        );
                    }
                    x.push_str("        </clef>\n");
                }
                if part.transpose != 0 {
                    let t = -(part.transpose as i32);
                    let _ = writeln!(
                        x,
                        "        <transpose>\n          <diatonic>{}</diatonic>\n          <chromatic>{}</chromatic>\n        </transpose>",
                        diatonic(t),
                        t
                    );
                }
            }
            x.push_str("      </attributes>\n");
        }
        if let Some(t) = &bar.tempo {
            x.push_str("      <direction placement=\"above\">\n");
            if let Some(up) = t.ramp {
                let _ = writeln!(
                    x,
                    "        <direction-type>\n          <words font-style=\"italic\">{}</words>\n        </direction-type>",
                    if up { "accel." } else { "rit." }
                );
            }
            let pm = if t.per_minute.fract() == 0.0 {
                format!("{}", t.per_minute as i64)
            } else {
                format!("{:.1}", t.per_minute)
            };
            let _ = writeln!(
                x,
                "        <direction-type>\n          <metronome parentheses=\"no\">\n            <beat-unit>{}</beat-unit>{}\n            <per-minute>{pm}</per-minute>\n          </metronome>\n        </direction-type>",
                t.unit.type_name(),
                if t.unit.dots > 0 { "\n            <beat-unit-dot/>" } else { "" }
            );
            if staves > 1 {
                x.push_str("        <staff>1</staff>\n");
            }
            let _ = writeln!(x, "        <sound tempo=\"{}\"/>", fmt_num(t.quarter_bpm));
            x.push_str("      </direction>\n");
        }
        let mut first_stream = true;
        for (si, st) in part.staves.iter().enumerate() {
            let sb = &st.bars[bi];
            for (vi, v) in sb.voices.iter().enumerate() {
                if !first_stream {
                    let _ = writeln!(
                        x,
                        "      <backup>\n        <duration>{}</duration>\n      </backup>",
                        d(bar.len)
                    );
                }
                first_stream = false;
                let voice_no = si * 4 + vi + 1;
                let mut at = 0i64;
                for (ei, e) in v.events.iter().enumerate() {
                    if e.start > at {
                        let _ = writeln!(
                            x,
                            "      <forward>\n        <duration>{}</duration>\n      </forward>",
                            d(e.start - at)
                        );
                    }
                    if let Some(&(shift, n)) = ottava_start.get(&(si, bi, vi, ei)) {
                        let _ = writeln!(
                            x,
                            "      <direction placement=\"{}\">\n        <direction-type>\n          <octave-shift type=\"{}\" size=\"8\" number=\"{n}\"/>\n        </direction-type>{}\n      </direction>",
                            if shift > 0 { "above" } else { "below" },
                            if shift > 0 { "down" } else { "up" },
                            staff_tag(staves, si)
                        );
                    }
                    if let Some(dy) = e.dynamic {
                        let _ = writeln!(
                            x,
                            "      <direction placement=\"below\">\n        <direction-type>\n          <dynamics>\n            <{}/>\n          </dynamics>\n        </direction-type>{}\n      </direction>",
                            dy.name(),
                            staff_tag(staves, si)
                        );
                    }
                    write_event(x, e, v, voice_no, staves, si, bar.len, d);
                    if let Some(&(_, n)) = ottava_stop.get(&(si, bi, vi, ei)) {
                        let _ = writeln!(
                            x,
                            "      <direction>\n        <direction-type>\n          <octave-shift type=\"stop\" size=\"8\" number=\"{n}\"/>\n        </direction-type>{}\n      </direction>",
                            staff_tag(staves, si)
                        );
                    }
                    at = e.end();
                }
                if at < bar.len {
                    let _ = writeln!(
                        x,
                        "      <forward>\n        <duration>{}</duration>\n      </forward>",
                        d(bar.len - at)
                    );
                }
            }
        }
        if bi == last {
            x.push_str("      <barline location=\"right\">\n        <bar-style>light-heavy</bar-style>\n      </barline>\n");
        }
        x.push_str("    </measure>\n");
    }
}

fn staff_tag(staves: usize, si: usize) -> String {
    if staves > 1 {
        format!("\n        <staff>{}</staff>", si + 1)
    } else {
        String::new()
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

#[allow(clippy::too_many_arguments)]
fn write_event(
    x: &mut String,
    e: &crate::model::Event,
    v: &crate::model::Voice,
    voice_no: usize,
    staves: usize,
    si: usize,
    bar_len: i64,
    d: &dyn Fn(i64) -> i64,
) {
    let tuplet = e.tuplet.map(|t| v.tuplets[t]);
    let tuplet_mark = |idx: usize| -> Option<&'static str> {
        let t = tuplet?;
        if t.first == idx && t.last == idx {
            None
        } else if t.first == idx {
            Some("start")
        } else if t.last == idx {
            Some("stop")
        } else {
            None
        }
    };
    let idx = v
        .events
        .iter()
        .position(|o| std::ptr::eq(o, e))
        .unwrap_or(0);
    match &e.kind {
        Kind::Rest { whole_bar } => {
            x.push_str("      <note>\n");
            if *whole_bar {
                x.push_str("        <rest measure=\"yes\"/>\n");
                let _ = writeln!(x, "        <duration>{}</duration>", d(bar_len));
            } else {
                x.push_str("        <rest/>\n");
                let _ = writeln!(x, "        <duration>{}</duration>", d(e.len));
            }
            let _ = writeln!(x, "        <voice>{voice_no}</voice>");
            if !*whole_bar {
                write_type(x, e.dur);
                write_time_modification(x, e.dur, tuplet);
            }
            if staves > 1 {
                let _ = writeln!(x, "        <staff>{}</staff>", si + 1);
            }
            if let Some(m) = tuplet_mark(idx) {
                let t = tuplet.unwrap();
                let _ = writeln!(
                    x,
                    "        <notations>\n          <tuplet type=\"{m}\" bracket=\"{}\"/>\n        </notations>",
                    if t.bracket { "yes" } else { "no" }
                );
            }
            x.push_str("      </note>\n");
        }
        Kind::Chord(heads) => {
            // Highest first, as MuseScore writes a chord.
            for (k, h) in heads.iter().rev().enumerate() {
                let _ = writeln!(
                    x,
                    "      <note dynamics=\"{:.2}\">",
                    h.velocity as f64 / 0.9
                );
                if k > 0 {
                    x.push_str("        <chord/>\n");
                }
                let sp = h.spelled;
                let _ = write!(
                    x,
                    "        <pitch>\n          <step>{}</step>\n",
                    sp.letter_name()
                );
                if sp.alter != 0 {
                    let _ = writeln!(x, "          <alter>{}</alter>", sp.alter);
                }
                let _ = writeln!(
                    x,
                    "          <octave>{}</octave>\n        </pitch>",
                    sp.octave
                );
                let _ = writeln!(x, "        <duration>{}</duration>", d(e.len));
                if h.tied_from {
                    x.push_str("        <tie type=\"stop\"/>\n");
                }
                if h.tie {
                    x.push_str("        <tie type=\"start\"/>\n");
                }
                let _ = writeln!(x, "        <voice>{voice_no}</voice>");
                write_type(x, e.dur);
                if let Some(a) = h.accidental {
                    let paren = if a.courtesy {
                        " cautionary=\"yes\" parentheses=\"yes\""
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        x,
                        "        <accidental{paren}>{}</accidental>",
                        accidental_name(a.alter)
                    );
                }
                write_time_modification(x, e.dur, tuplet);
                if e.dur.log >= 1 {
                    let _ = writeln!(
                        x,
                        "        <stem>{}</stem>",
                        if e.stem == Stem::Up { "up" } else { "down" }
                    );
                }
                if staves > 1 {
                    let _ = writeln!(x, "        <staff>{}</staff>", si + 1);
                }
                if k == 0 {
                    for (level, b) in e.beams.iter().enumerate() {
                        let s = match b {
                            BeamMark::Begin => "begin",
                            BeamMark::Continue => "continue",
                            BeamMark::End => "end",
                            BeamMark::HookForward => "forward hook",
                            BeamMark::HookBackward => "backward hook",
                        };
                        let _ = writeln!(x, "        <beam number=\"{}\">{s}</beam>", level + 1);
                    }
                }
                let tm = if k == 0 { tuplet_mark(idx) } else { None };
                if h.tie || h.tied_from || tm.is_some() {
                    x.push_str("        <notations>\n");
                    if h.tied_from {
                        x.push_str("          <tied type=\"stop\"/>\n");
                    }
                    if h.tie {
                        x.push_str("          <tied type=\"start\"/>\n");
                    }
                    if let Some(m) = tm {
                        let t = tuplet.unwrap();
                        let _ = writeln!(
                            x,
                            "          <tuplet type=\"{m}\" bracket=\"{}\"/>",
                            if t.bracket { "yes" } else { "no" }
                        );
                    }
                    x.push_str("        </notations>\n");
                }
                x.push_str("      </note>\n");
            }
        }
    }
}

fn write_time_modification(x: &mut String, dur: Dur, tuplet: Option<crate::model::Tuplet>) {
    let Some(t) = tuplet else {
        return;
    };
    x.push_str("        <time-modification>\n");
    let _ = writeln!(x, "          <actual-notes>{}</actual-notes>", t.actual);
    let _ = writeln!(x, "          <normal-notes>{}</normal-notes>", t.normal);
    if t.unit != dur {
        let _ = writeln!(
            x,
            "          <normal-type>{}</normal-type>",
            t.unit.type_name()
        );
    }
    x.push_str("        </time-modification>\n");
}

fn write_type(x: &mut String, dur: Dur) {
    let _ = writeln!(x, "        <type>{}</type>", dur.type_name());
    for _ in 0..dur.dots {
        x.push_str("        <dot/>\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(escape("A & <B>"), "A &amp; &lt;B&gt;");
        assert_eq!(diatonic(-2), -1);
        assert_eq!(diatonic(-7), -4);
        assert_eq!(diatonic(-12), -7);
        assert_eq!(fmt_num(120.0), "120");
        assert_eq!(fmt_num(96.5), "96.5");
    }
}

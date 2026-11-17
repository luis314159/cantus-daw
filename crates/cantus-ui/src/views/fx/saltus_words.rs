//! Saltus's words (ADR 0132): the names of the modes, the keys and scales, an
//! interval's name in semitones or in degrees, a note's name, and the value pills — shared by
//! the window, its display, the inspector's line and the pedal on Tabula's board.

use super::super::frame;
use cantus_dsp::saltus::{Mode, SaltusSettings};
use std::fmt::Write as _;

pub const MODES: [&str; 4] = [
    "fx.saltus.mode.0",
    "fx.saltus.mode.1",
    "fx.saltus.mode.2",
    "fx.saltus.mode.3",
];

pub const HSRC: [&str; 3] = ["fx.saltus.hsrc.0", "fx.saltus.hsrc.1", "fx.saltus.hsrc.2"];

pub const KEYS: [&str; 12] = [
    "fx.saltus.key.0",
    "fx.saltus.key.1",
    "fx.saltus.key.2",
    "fx.saltus.key.3",
    "fx.saltus.key.4",
    "fx.saltus.key.5",
    "fx.saltus.key.6",
    "fx.saltus.key.7",
    "fx.saltus.key.8",
    "fx.saltus.key.9",
    "fx.saltus.key.10",
    "fx.saltus.key.11",
];

pub const SCALES: [&str; 13] = [
    "fx.saltus.scale.0",
    "fx.saltus.scale.1",
    "fx.saltus.scale.2",
    "fx.saltus.scale.3",
    "fx.saltus.scale.4",
    "fx.saltus.scale.5",
    "fx.saltus.scale.6",
    "fx.saltus.scale.7",
    "fx.saltus.scale.8",
    "fx.saltus.scale.9",
    "fx.saltus.scale.10",
    "fx.saltus.scale.11",
    "fx.saltus.scale.12",
];

/// The intervals within an octave, by semitones (0 … 12).
const IV: [&str; 13] = [
    "saltus.iv.0",
    "saltus.iv.1",
    "saltus.iv.2",
    "saltus.iv.3",
    "saltus.iv.4",
    "saltus.iv.5",
    "saltus.iv.6",
    "saltus.iv.7",
    "saltus.iv.8",
    "saltus.iv.9",
    "saltus.iv.10",
    "saltus.iv.11",
    "saltus.iv.12",
];

/// The intervals in degrees of a seven-note scale (0 … 14).
const DEG: [&str; 15] = [
    "saltus.deg.0",
    "saltus.deg.1",
    "saltus.deg.2",
    "saltus.deg.3",
    "saltus.deg.4",
    "saltus.deg.5",
    "saltus.deg.6",
    "saltus.deg.7",
    "saltus.deg.8",
    "saltus.deg.9",
    "saltus.deg.10",
    "saltus.deg.11",
    "saltus.deg.12",
    "saltus.deg.13",
    "saltus.deg.14",
];

/// Up or down, after the interval's name (none for a unison).
fn direction(n: i32, out: &mut String) {
    if n != 0 {
        out.push(' ');
        out.push_str(frame::tr(if n > 0 { "saltus.up" } else { "saltus.down" }));
    }
}

/// An interval of `st` semitones in words: "Fifth up", "Octave and fifth down",
/// "Two octaves down", "Unison".
pub fn interval(st: i32, out: &mut String) {
    let a = st.unsigned_abs() as usize;
    match a {
        0..=12 => out.push_str(frame::tr(IV[a])),
        24 => out.push_str(frame::tr("saltus.iv.24")),
        13..=23 => {
            out.push_str(frame::tr("saltus.iv.octave_and"));
            out.push(' ');
            out.push_str(&frame::tr(IV[a - 12]).to_lowercase());
        }
        _ => {
            let _ = write!(out, "{a} st");
        }
    }
    direction(st, out);
}

/// An interval of `d` degrees of the scale in words: "Third up", "Fourth down".
pub fn degrees(d: i32, out: &mut String) {
    let a = (d.unsigned_abs() as usize).min(DEG.len() - 1);
    out.push_str(frame::tr(DEG[a]));
    direction(d, out);
}

/// A note's name and octave in the interface's language: "A3", "La3".
pub fn note(midi: i32, out: &mut String) {
    out.push_str(frame::tr(KEYS[midi.rem_euclid(12) as usize]));
    let _ = write!(out, "{}", midi.div_euclid(12) - 1);
}

/// A signed whole number with the true minus: "+12", "−5", "0".
pub fn signed(v: i32, out: &mut String) {
    match v.cmp(&0) {
        std::cmp::Ordering::Greater => {
            let _ = write!(out, "+{v}");
        }
        std::cmp::Ordering::Less => {
            let _ = write!(out, "\u{2212}{}", -v);
        }
        std::cmp::Ordering::Equal => out.push('0'),
    }
}

/// An interval to a tenth with the true minus: "+7.0 st", "−12.0 st".
pub fn fine_st(v: f32, out: &mut String) {
    let t = (v * 10.0).round() / 10.0;
    if t > 0.0 {
        out.push('+');
    } else if t < 0.0 {
        out.push('\u{2212}');
    }
    let _ = write!(out, "{:.1} st", t.abs());
}

/// An interval's pill: "+12 st".
pub fn semitone_pill(v: f32, out: &mut String) {
    signed(v.round() as i32, out);
    out.push_str(" st");
}

/// A number of degrees' pill: "+2".
pub fn degree_pill(v: f32, out: &mut String) {
    signed(v.round() as i32, out);
}

/// The balance in its words: "C", "L 30", "R 100".
pub fn pan_pill(v: f32, out: &mut String) {
    let p = (v * 100.0).round() as i32;
    if p == 0 {
        out.push('C');
    } else if p < 0 {
        let _ = write!(out, "L {}", -p);
    } else {
        let _ = write!(out, "R {p}");
    }
}

/// What a setting does after its mode's name, for the inspector's line and a pedal's
/// label: " · +12 st, −5 st", " · C major", " · 0 → +12 st".
pub fn summary(ss: &SaltusSettings, out: &mut String) {
    out.push_str(" \u{b7} ");
    let voices = || ss.voices.iter().filter(|v| v.level > 0.0);
    match ss.mode {
        Mode::Shift => {
            let mut first = true;
            for v in voices() {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                semitone_pill(v.semitones, out);
            }
            if first {
                out.push_str(frame::tr("saltus.d.silent"));
            }
        }
        Mode::Harmony => {
            let mut first = true;
            for v in voices() {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                degrees(v.degree, out);
            }
            if first {
                out.push_str(frame::tr("saltus.d.silent"));
            }
        }
        Mode::Octave => {
            let mut first = true;
            for (lvl, st) in [(ss.sub2, -24), (ss.sub1, -12), (ss.up1, 12)] {
                if lvl > 0.0 {
                    if !first {
                        out.push_str(", ");
                    }
                    first = false;
                    semitone_pill(st as f32, out);
                }
            }
            if first {
                out.push_str(frame::tr("saltus.d.silent"));
            }
        }
        Mode::Whammy => {
            semitone_pill(ss.heel, out);
            out.push_str(" \u{2192} ");
            semitone_pill(ss.toe, out);
        }
    }
}

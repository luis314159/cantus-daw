//! The Tuning window's pictures (ADR 0155 §5): the **circle of fifths** coloured by how fast
//! each fifth and each major third beats — how temperaments are compared (a pure interval
//! does not beat; equal temperament's thirds beat about ten times a second at middle C) —
//! and the **degrees**: each in cents above C, from equal temperament, and its fifth's and
//! third's beat rate. A scale that is not 12 to the octave gets its degrees as written, in
//! cents and from its own equal division. Everything is read from the table every unit
//! plays (`Tuning::offsets`), so the picture is what sounds. r42:tuning-face

use super::words::{degree_cents, degree_written, key_name, signed_cents};
use crate::i18n::{t, tf};
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{self, Policy, Role, Text};
use crate::kit::{Brush, Style};
use cantus_core::tuning::{ScalaScale, TuningTable};
use egui::{Align2, Color32, Pos2, Rect, Vec2};

/// The keys the circle measures: C4 … B4 and the notes a fifth and a third above them.
const C4: u8 = 60;

/// A key's frequency in `table`.
pub fn hz(table: &TuningTable, key: u8) -> f64 {
    let off = f64::from(table.at(key));
    440.0 * cantus_dsp::math::exp2((f64::from(key) - 69.0 + off) / 12.0)
}

/// How fast the fifth on pitch class `pc` (in the octave of middle C) beats, per second:
/// `|3·f − 2·f₅|`, the third harmonic of the lower note against the second of the upper.
pub fn fifth_beats(table: &TuningTable, pc: u8) -> f64 {
    let k = C4 + pc % 12;
    (3.0 * hz(table, k) - 2.0 * hz(table, k + 7)).abs()
}

/// How fast the major third on `pc` beats: `|5·f − 4·f₃|`.
pub fn third_beats(table: &TuningTable, pc: u8) -> f64 {
    let k = C4 + pc % 12;
    (5.0 * hz(table, k) - 4.0 * hz(table, k + 4)).abs()
}

/// The fifth's size from pure, in cents (negative: narrowed, as every temperament does).
pub fn fifth_from_pure(table: &TuningTable, pc: u8) -> f64 {
    let k = C4 + pc % 12;
    100.0 * (7.0 + f64::from(table.at(k + 7)) - f64::from(table.at(k))) - 701.955_000_865_4
}

/// The beat rate's colour: the meters' own scale, from their low colour (pure) through
/// their middle to their clip colour (beating fast). `x` = 0 … 1.
pub fn beat_colour(s: &Style, x: f64) -> Color32 {
    let x = x.clamp(0.0, 1.0) as f32;
    if x < 0.5 {
        mix(s.t.meter_low, s.t.meter_mid, x * 2.0)
    } else {
        mix(s.t.meter_mid, s.t.meter_clip, x * 2.0 - 1.0)
    }
}

/// Where a fifth's beat rate reaches the scale's top, per second (Pythagorean's wolf and
/// meantone's beat far faster; equal temperament's fifth beats under one per second).
pub const FIFTH_TOP: f64 = 8.0;
/// Where a major third's reaches it (equal temperament's beats about 10, Pythagorean's 16).
pub const THIRD_TOP: f64 = 16.0;

/// The circle's order: C, G, D … F — each a fifth above the one before.
pub fn fifths_order() -> [u8; 12] {
    std::array::from_fn(|i| ((i * 7) % 12) as u8)
}

/// Whether the table plays 12 notes to the octave (the circle compares such tunings).
pub fn twelve_to_the_octave(table: &TuningTable) -> bool {
    (0..12u8).all(|p| table.sounds(C4 + p) && table.sounds(C4 + p + 7) && table.sounds(C4 + p + 4))
}

/// The circle of fifths in `rect`: the fifths as the outer ring's arcs (each from its note
/// to the next), the major thirds as the inner ring's (centred on their lower note), each
/// note's name outside, the tonic ringed. Returns each note's centre (for hovering).
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, table: &TuningTable, tonic: u8) -> [Pos2; 12] {
    let k = s.k;
    let side = rect.width().min(rect.height());
    let c = rect.center();
    let r_label = side * 0.5 - 12.0 * k;
    let r_out = r_label - 18.0 * k;
    let r_in = r_out - 22.0 * k;
    let w = 10.0 * k;
    let step = std::f32::consts::TAU / 12.0;
    let at = |i: usize| -std::f32::consts::FRAC_PI_2 + step * i as f32;
    let gap = 0.035;
    // The rings' grooves first, so a pure (lightest) arc still reads as part of a ring.
    b.circle_stroke(c, r_out, w + 2.0 * k, with_alpha(s.t.well, 0.9));
    b.circle_stroke(c, r_in, w + 2.0 * k, with_alpha(s.t.well, 0.9));
    let order = fifths_order();
    let mut centres = [Pos2::ZERO; 12];
    for (i, &pc) in order.iter().enumerate() {
        let a0 = at(i);
        let fifth = beat_colour(s, fifth_beats(table, pc) / FIFTH_TOP);
        b.arc(c, r_out, a0 + gap, a0 + step - gap, w, fifth, false);
        let third = beat_colour(s, third_beats(table, pc) / THIRD_TOP);
        b.arc(
            c,
            r_in,
            a0 - step * 0.5 + gap,
            a0 + step * 0.5 - gap,
            w,
            third,
            false,
        );
        let p = c + Vec2::angled(a0) * r_label;
        centres[usize::from(pc)] = p;
        let on = pc == tonic % 12;
        if on {
            b.circle(p, 13.0 * k, crate::kit::Fill::Solid(s.t.highlight));
        }
        let ink = if on {
            crate::kit::color::on_fill(s.t.highlight)
        } else {
            s.t.text
        };
        let lr = Rect::from_center_size(p, Vec2::new(30.0, 20.0) * k);
        let _ = text::paint(
            b,
            lr,
            &Text::new(key_name(pc), Role::Control, ink).align(Align2::CENTER_CENTER),
            s.tk(),
        );
        // A tick where each fifth ends, between two arcs.
        let tick = Vec2::angled(a0);
        b.segment(
            c + tick * (r_out - w),
            c + tick * (r_out + w),
            1.0 * k,
            s.t.edge,
            false,
        );
    }
    centres
}

/// One row of the degree table, as words.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub cents: String,
    pub from_equal: String,
    /// The fifth's and the major third's beats per second (12-note tunings).
    pub fifth: String,
    pub third: String,
    /// Their colours' places on the scale, 0 … 1.
    pub fifth_x: f64,
    pub third_x: f64,
}

/// The twelve notes of a 12-to-the-octave table: each in cents above C, from equal
/// temperament at the chosen diapason (A-anchored), and its fifth's and third's beats.
pub fn twelve_rows(table: &TuningTable, a4_hz: f64) -> Vec<Row> {
    let shift = f64::from(cantus_core::tuning::diapason_semitones(a4_hz));
    let c = f64::from(table.at(C4));
    (0..12u8)
        .map(|pc| {
            let off = f64::from(table.at(C4 + pc));
            let (f, th) = (fifth_beats(table, pc), third_beats(table, pc));
            Row {
                name: key_name(pc).to_string(),
                cents: if off.is_nan() {
                    t("tuning.table.unmapped")
                } else {
                    format!("{:.2}", 100.0 * (f64::from(pc) + off - c))
                },
                from_equal: if off.is_nan() {
                    String::new()
                } else {
                    signed_cents(100.0 * (off - shift))
                },
                fifth: format!("{f:.2}"),
                third: format!("{th:.2}"),
                fifth_x: f / FIFTH_TOP,
                third_x: th / THIRD_TOP,
            }
        })
        .collect()
}

/// The nearest 12-TET semitone to `cents` above degree 0 (which key 60, C, plays) and the
/// distance from it: "C +38.71", "C# −22.58".
pub fn from_twelve(cents: f64) -> String {
    let n = (cents / 100.0).round();
    let pc = (n as i64).rem_euclid(12) as u8;
    format!("{} {}", key_name(pc), signed_cents(cents - 100.0 * n))
}

/// The degrees of a scale that is not 12 to the octave: as written, in cents, and from the
/// nearest 12-TET semitone (its name: a 31-note scale's degrees against the keyboard's).
pub fn scale_rows(s: &ScalaScale) -> Vec<Row> {
    let mut out = vec![Row {
        name: "0".into(),
        cents: "0.00".into(),
        from_equal: from_twelve(0.0),
        fifth: "1/1".into(),
        third: String::new(),
        fifth_x: 0.0,
        third_x: 0.0,
    }];
    for (i, d) in s.degrees.iter().enumerate() {
        let c = degree_cents(d);
        out.push(Row {
            name: (i + 1).to_string(),
            cents: format!("{c:.2}"),
            from_equal: from_twelve(c),
            // In a scale's table the fourth column is the degree as written.
            fifth: degree_written(d),
            third: String::new(),
            fifth_x: 0.0,
            third_x: 0.0,
        });
    }
    out
}

/// What hovering note `pc` says: its fifth's and its third's beats and sizes.
pub fn hover_words(table: &TuningTable, pc: u8) -> String {
    tf(
        "tuning.circle.hover",
        &[
            ("note", &key_name(pc)),
            ("fifth", &key_name(pc + 7)),
            ("fbeats", &format!("{:.2}", fifth_beats(table, pc))),
            ("fcents", &signed_cents(fifth_from_pure(table, pc))),
            ("third", &key_name(pc + 4)),
            ("tbeats", &format!("{:.2}", third_beats(table, pc))),
        ],
    )
}

/// The legend under the circle: the two rings, and the colours' scale from pure to fast.
pub fn legend(b: &mut Brush, rect: Rect, s: &Style) {
    let k = s.k;
    let row = 18.0 * k;
    let mut y = rect.top();
    for (key, top) in [
        ("tuning.circle.outer", FIFTH_TOP),
        ("tuning.circle.inner", THIRD_TOP),
    ] {
        let line = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), row));
        let bar = Rect::from_min_size(
            Pos2::new(line.right() - 96.0 * k, line.center().y - 3.0 * k),
            Vec2::new(96.0 * k, 6.0 * k),
        );
        let n = 24;
        for i in 0..n {
            let x0 = bar.left() + bar.width() * i as f32 / n as f32;
            let x1 = bar.left() + bar.width() * (i + 1) as f32 / n as f32;
            let seg = Rect::from_min_max(Pos2::new(x0, bar.top()), Pos2::new(x1, bar.bottom()));
            b.rect(
                seg,
                crate::kit::Fill::Solid(beat_colour(s, i as f64 / (n - 1) as f64)),
            );
        }
        let words = tf(key, &[("top", &format!("{top:.0}"))]);
        let _ = text::paint(
            b,
            Rect::from_min_max(line.min, Pos2::new(bar.left() - 8.0 * k, line.bottom())),
            &Text::new(&words, Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Abbreviate),
            s.tk(),
        );
        y += row;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_temperaments_fifths_and_thirds_beat_as_the_books_say() {
        let t = TuningTable::EQUAL;
        // At middle C: the fifth C–G beats about 0.89 per second, the third C–E about 10.4.
        assert!(
            (fifth_beats(&t, 0) - 0.889).abs() < 0.01,
            "{}",
            fifth_beats(&t, 0)
        );
        assert!(
            (third_beats(&t, 0) - 10.38).abs() < 0.05,
            "{}",
            third_beats(&t, 0)
        );
        assert!((fifth_from_pure(&t, 0) + 1.955).abs() < 1e-3);
        assert!(twelve_to_the_octave(&t));
        let rows = twelve_rows(&t, 440.0);
        assert_eq!(rows[9].name, "A");
        assert_eq!(rows[9].cents, "900.00");
        assert_eq!(rows[9].from_equal, "0.00");
    }

    #[test]
    fn a_pure_fifth_does_not_beat() {
        let mut t = TuningTable::EQUAL;
        // G4 raised to a pure fifth over C4.
        t.0[67] = (1.955_000_865 / 100.0) as f32;
        assert!(fifth_beats(&t, 0) < 0.01, "{}", fifth_beats(&t, 0));
    }

    #[test]
    fn a_degree_is_named_by_its_nearest_semitone() {
        assert_eq!(from_twelve(38.709_677), "C +38.71");
        assert_eq!(from_twelve(77.419), "C# \u{2212}22.58");
        assert_eq!(from_twelve(1200.0), "C 0.00");
        assert_eq!(from_twelve(701.955), "G +1.96");
    }

    #[test]
    fn the_circle_goes_round_by_fifths() {
        assert_eq!(fifths_order(), [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5]);
    }
}

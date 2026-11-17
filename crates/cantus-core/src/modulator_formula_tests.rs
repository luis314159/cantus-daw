//! The formula language against FL Studio's own manual (the outside source, ADR 0148):
//! every example formula the *Fruity Formula Controller* page publishes, with what the
//! page says it draws checked against a reference written from that description.
//!
//! The references call std's functions on purpose: the platform's libm, independent of
//! `cantus_dsp::math`, is the outside check (to 1e-6, never bit for bit).
#![allow(clippy::disallowed_methods)]

use super::*;
use std::f64::consts::PI;

fn env(a: f64, b: f64, c: f64, beats: f64) -> Env {
    Env {
        a,
        b,
        c,
        beats,
        seconds: beats * 0.5,
        tempo: 120.0,
    }
}

fn val(text: &str, e: &Env) -> f64 {
    Program::compile(text)
        .unwrap_or_else(|err| panic!("{text}: {err:?}"))
        .value(e)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 || (a.is_nan() && b.is_nan())
}

/// Every example formula on FL's manual page, verbatim (its capitals and spacing), each
/// with the shape the page describes, sampled over two beats at several knob settings.
#[test]
fn fl_manual_examples_compute_what_the_page_says() {
    type Ref = fn(f64, f64, f64, f64) -> f64;
    let cases: &[(&str, Ref)] = &[
        ("0.5*Sin(SongTime*Pi)+0.5", |_, _, _, s| 0.5 * (s * PI).sin() + 0.5),
        ("a*Sin(SongTime*Pi*b)+c", |a, b, c, s| a * (s * PI * b).sin() + c),
        ("a*Sin(SongTime*Pi*(Int(b*10)))+c", |a, b, c, s| {
            a * (s * PI * (b * 10.0).trunc()).sin() + c
        }),
        ("a*Sin(SongTime*Pi+b*Pi)+c", |a, b, c, s| a * (s * PI + b * PI).sin() + c),
        ("0.319*ArcCos(Cos(SongTime*Pi))", |_, _, _, s| 0.319 * (s * PI).cos().acos()),
        ("a*ArcCos(Cos(SongTime*Pi*2*b))+c", |a, b, c, s| {
            a * (s * PI * 2.0 * b).cos().acos() + c
        }),
        ("a*ArcCos(Cos(SongTime*Pi*(Int(b*10))))+c", |a, b, c, s| {
            a * (s * PI * (b * 10.0).trunc()).cos().acos() + c
        }),
        ("a*ArcCos(Cos(SongTime*Pi+b*Pi))+c", |a, b, c, s| a * (s * PI + b * PI).cos().acos() + c),
        ("a*Frac(SongTime*0.5*Int(b*10))+c", |a, b, c, s| {
            let f = s * 0.5 * (b * 10.0).trunc();
            a * (f - f.trunc()) + c
        }),
        ("a*Round(0.5*Sin(SongTime*Pi*2*b)+0.5)+c", |a, b, c, s| {
            a * (0.5 * (s * PI * 2.0 * b).sin() + 0.5).round_ties_even() + c
        }),
        ("a*Round(0.5*Sin(SongTime*Pi*(Int(b*10)))+0.5)+c", |a, b, c, s| {
            a * (0.5 * (s * PI * (b * 10.0).trunc()).sin() + 0.5).round_ties_even() + c
        }),
        ("Sin(SongTime*Pi*b)", |_, b, _, s| (s * PI * b).sin()),
        ("a*ArcTg(cos(SongTime*Pi*b)-Sec(SongTime*Pi*b))^2+c", |a, b, c, s| {
            let x = s * PI * b;
            a * (x.cos() - 1.0 / x.cos()).atan().powi(2) + c
        }),
        ("Frac(SongTime)", |_, _, _, s| s - s.trunc()),
        ("a*Frac(SongTime*b*2)+c", |a, b, c, s| a * ((s * b * 2.0) - (s * b * 2.0).trunc()) + c),
        ("Abs(Round(a)-Frac(SongTime))", |a, _, _, s| (a.round_ties_even() - (s - s.trunc())).abs()),
        ("Round(Frac(SongTime))", |_, _, _, s| (s - s.trunc()).round_ties_even()),
        ("Round(0.5*Sin(SongTime*Pi*2)+0.5)", |_, _, _, s| {
            (0.5 * (s * PI * 2.0).sin() + 0.5).round_ties_even()
        }),
        ("Round(0.5*Sin(SongTime*Pi)+a)", |a, _, _, s| (0.5 * (s * PI).sin() + a).round_ties_even()),
        ("a*Round(Frac(SongTime*2*b)+0.5-c)", |a, b, c, s| {
            let f = s * 2.0 * b;
            a * (f - f.trunc() + 0.5 - c).round_ties_even()
        }),
        ("Pi/5*ArcTg(Sin(SongTime*2*Pi*b))+0.5", |_, b, _, s| {
            PI / 5.0 * (s * 2.0 * PI * b).sin().atan() + 0.5
        }),
        ("a*Abs(Sin(SongTime*Pi*b))", |a, b, _, s| a * (s * PI * b).sin().abs()),
        ("a*(0.5+0.5*Cos(SongTime*Pi*b+Pi))^(1/5)+c", |a, b, c, s| {
            a * (0.5 + 0.5 * (s * PI * b + PI).cos()).powf(0.2) + c
        }),
    ];
    for (text, reference) in cases {
        let p = Program::compile(text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        for &(a, b, c) in &[(1.0, 1.0, 0.0), (0.5, 0.3, 0.25), (0.25, 0.9, 0.5)] {
            for k in 0..=96 {
                let s = f64::from(k) / 48.0 + 0.003;
                let got = p.value(&env(a, b, c, s));
                let want = reference(a, b, c, s);
                assert!(
                    (got - want).abs() < 1e-6,
                    "{text} at a={a} b={b} c={c} beat {s}: {got} vs {want}"
                );
            }
        }
    }
}

/// The page's own descriptions: the basic waves are a sine, a triangle, a saw and a
/// square of the song's time, in 0…1.
#[test]
fn the_basic_waves_have_the_shapes_the_page_names() {
    let sine = Program::compile("0.5*Sin(SongTime*Pi)+0.5").unwrap();
    // A two-beat period: 0.5 at beat 0, 1 at beat 0.5, 0 at beat 1.5.
    assert!(close(sine.value(&env(0., 0., 0., 0.0)), 0.5));
    assert!((sine.value(&env(0., 0., 0., 0.5)) - 1.0).abs() < 1e-12);
    assert!(sine.value(&env(0., 0., 0., 1.5)).abs() < 1e-12);
    let tri = Program::compile("0.319*ArcCos(Cos(SongTime*Pi))").unwrap();
    // 0 at beat 0, ~1 (0.319·π = 1.002) at beat 1, linear between.
    assert!(tri.value(&env(0., 0., 0., 0.0)).abs() < 1e-6);
    assert!((tri.value(&env(0., 0., 0., 1.0)) - 0.319 * PI).abs() < 1e-6);
    assert!((tri.value(&env(0., 0., 0., 0.5)) - 0.5 * 0.319 * PI).abs() < 1e-6);
    let square = Program::compile("Round(Frac(SongTime))").unwrap();
    assert_eq!(square.value(&env(0., 0., 0., 0.25)), 0.0);
    assert_eq!(square.value(&env(0., 0., 0., 0.75)), 1.0);
    // Output is clamped: 0.319·π is cut at 1.
    assert_eq!(tri.output(&env(0., 0., 0., 1.0)), 1.0);
}

/// The page's boolean table, for 1/0 inputs.
#[test]
fn the_boolean_gates_of_the_page() {
    let gates: &[(&str, fn(bool, bool) -> bool)] = &[
        ("a", |a, _| a),
        ("1-a", |a, _| !a),
        ("a*b", |a, b| a && b),
        ("a+b", |a, b| a || b),
        ("(a+b)=1", |a, b| a != b),
        ("1-(a*b)", |a, b| !(a && b)),
        ("(a+b)=0", |a, b| !(a || b)),
        ("a=b", |a, b| a == b),
    ];
    for (text, f) in gates {
        let p = Program::compile(text).unwrap();
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            let e = env(f64::from(u8::from(a)), f64::from(u8::from(b)), 0.0, 0.0);
            assert_eq!(p.output(&e), if f(a, b) { 1.0 } else { 0.0 }, "{text} {a} {b}");
        }
    }
}

/// The sample and hold of the page: a held random per step of `a`'s rate, repeatable.
#[test]
fn seeded_rand_holds_and_repeats() {
    let p = Program::compile("b*SeededRand(Int(SongTime*a))+c").unwrap();
    let at = |s| p.value(&env(4.0, 1.0, 0.0, s));
    assert_eq!(at(0.01), at(0.24));
    assert_ne!(at(0.01), at(0.26));
    assert_eq!(at(0.3), at(0.3));
    assert!((0.0..=1.0).contains(&at(0.6)));
}

#[test]
fn operators_precedence_and_fl_words() {
    let e = env(0.25, 0.75, 0.5, 3.0);
    for (text, want) in [
        ("1+2*3", 7.0),
        ("2^3^2", 512.0),
        ("-2^2", -4.0),
        ("2^-1", 0.5),
        ("50%", 0.5),
        ("4!", 24.0),
        ("7 div 2", 3.0),
        ("-7 mod 3", -1.0),
        ("7.5 fmod 2", 1.5),
        ("3 shl 2", 12.0),
        ("12 shr 2", 3.0),
        ("a and b", 0.0),
        ("a or b", 1.0),
        ("a xor b", 1.0),
        ("not a", 1.0),
        ("1 <> 2", 1.0),
        ("2 in 2", 1.0),
        ("a < b", 1.0),
        ("Max(a,b)", 0.75),
        ("If(b, 3, 4)", 3.0),
        ("Case(1, 3, 4)", 3.0),
        ("Inter(0.5, 1, 0)", 1.0),
        ("InterNoSwap(0.5, 1, 0)", 0.0),
        ("Map(0.5, 2, 4)", 3.0),
        ("Round(2.5)", 2.0),
        ("Round(3.5)", 4.0),
        ("Frac(-1.25)", -0.25),
        ("Int(-1.75)", -1.0),
        ("Sqr(3)", 9.0),
        ("LogN(2, 8)", 3.0),
        ("IntPower(2, 10)", 1024.0),
        ("Fold(1.25)", 0.75),
        ("saw(2.25)", 0.25),
        ("tri(0.25)", 0.5),
        ("square(0.75)", 0.0),
        ("pulse(0.1, 0.2)", 1.0),
        ("clamp(5, 0, 1)", 1.0),
        ("SongTime()", 3.0),
        ("beat", 3.0),
        ("t", 1.5),
        ("Time", 1500.0),
        ("GetTempo", 120.0),
        ("SIN(0)", 0.0),
        ("1e3", 1000.0),
        ("1/0 > 5", 1.0),
    ] {
        let got = val(text, &e);
        assert!(close(got, want), "{text}: {got} vs {want}");
    }
}

#[test]
fn what_is_refused_and_where() {
    let err = |t: &str| Program::compile(t).unwrap_err();
    assert_eq!(err("").kind, ErrorKind::Empty);
    assert_eq!(err("   ").kind, ErrorKind::Empty);
    assert_eq!(err("1 +").kind, ErrorKind::UnexpectedEnd);
    assert_eq!(err("(1+2").kind, ErrorKind::Unclosed);
    assert_eq!(err("1+2)").kind, ErrorKind::Unclosed);
    assert_eq!(err("a b").kind, ErrorKind::Trailing);
    assert_eq!(err("a ; b").kind, ErrorKind::Unexpected(';'));
    assert_eq!(err("a ; b").at, 2);
    assert_eq!(err("foo(1)").kind, ErrorKind::UnknownName("foo".into()));
    assert_eq!(err("2*MouseX").kind, ErrorKind::NotDeterministic("mousex".into()));
    assert_eq!(err("2*MouseX").at, 2);
    assert_eq!(err("Date()").kind, ErrorKind::NotDeterministic("date".into()));
    assert_eq!(err("Tension(a, b)").kind, ErrorKind::NotSupported("tension".into()));
    assert!(matches!(err("Max(1)").kind, ErrorKind::Arguments { wants: 2, .. }));
    assert!(matches!(err("Sin").kind, ErrorKind::Arguments { wants: 1, .. }));
    assert_eq!(err("1.2.3").kind, ErrorKind::BadNumber);
    assert_eq!(err(&"a".repeat(600)).kind, ErrorKind::TooLong);
    // Too deep, too long, too many steps: refused, never a stack overflow.
    assert_eq!(err(&format!("{}1{}", "(".repeat(200), ")".repeat(200))).kind, ErrorKind::TooComplex);
    assert_eq!(err(&format!("{}1", "-".repeat(200))).kind, ErrorKind::TooComplex);
    assert_eq!(err(&vec!["1"; 200].join("+")).kind, ErrorKind::TooComplex);
    // A right-leaning chain needs a deep stack: proven at compile time, refused.
    let deep = format!("{}1{}", "1+(".repeat(40), ")".repeat(40));
    assert_eq!(err(&deep).kind, ErrorKind::TooComplex);
    for e in [ErrorKind::Empty, ErrorKind::TooComplex, ErrorKind::BadNumber] {
        assert!(e.key().starts_with("fxd.formula.err."));
    }
}

/// Whatever a valid formula computes, the output is a number in 0…1: NaN, infinities and
/// huge values never leave the controller.
#[test]
fn the_output_is_always_in_range() {
    for text in ["0/0", "1/0", "-1/0", "Ln(-1)", "Sqrt(-1)", "1e308*10", "170!", "200!"] {
        let p = Program::compile(text).unwrap();
        let o = p.output(&env(0.5, 0.5, 0.5, 1.0));
        assert!((0.0..=1.0).contains(&o), "{text}: {o}");
    }
}

#[test]
fn reads_says_what_moves_it() {
    let r = Program::compile("a*Sin(SongTime*Pi)+c").unwrap().reads();
    assert_eq!(r.inputs, [true, false, true]);
    assert!(r.time);
    let r = Program::compile("0.5*b").unwrap().reads();
    assert_eq!(r.inputs, [false, true, false]);
    assert!(!r.time);
}

/// The same formula, the same moment: the same bits (no state, no clock, no allocation).
#[test]
fn evaluation_repeats_to_the_bit() {
    let p = Program::compile("Rand*a + 0.3*Sin(beat*Pi*b) + SeededRand(beat*8)*c").unwrap();
    for k in 0..200 {
        let e = env(0.3, 0.7, 0.2, f64::from(k) * 0.0137);
        assert_eq!(p.value(&e).to_bits(), p.value(&e).to_bits());
    }
}

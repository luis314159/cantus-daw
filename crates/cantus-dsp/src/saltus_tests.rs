//! Saltus's contract: the null at the defaults to the bit, the latency each mode reports,
//! block-size independence at tolerance 0, exact rest (four twins), finite at extremes,
//! and the MIDI notes at their sample.

use super::*;
use crate::pitch::testkit::{self, Noise, Vowel};

pub(super) const SR: f32 = 48_000.0;

/// A plucked string (Karplus–Strong, a little low-passed): `hz`, `secs`, peak `amp`.
pub(super) fn pluck(hz: f32, secs: f32, amp: f32, seed: u64) -> Vec<f32> {
    pluck_at(hz, secs, amp, seed, 0.2, 0.05)
}

/// A plucked string, plucked at `pos` of its length (small: near the bridge, bright),
/// with `noise` of the pick.
pub(super) fn pluck_at(hz: f32, secs: f32, amp: f32, seed: u64, pos: f32, noise: f32) -> Vec<f32> {
    let n = (secs * SR) as usize;
    let len = (SR / hz).round() as usize;
    let mut ns = Noise::new(seed);
    // The string's shape where it is plucked (a triangle: harmonics falling as 1/n², the
    // fundamental strongest unless plucked near the bridge), with the pick's noise.
    let peak_at = (len as f32 * pos).max(1.0);
    let mut line: Vec<f32> = (0..len)
        .map(|i| {
            let x = i as f32;
            let tri = if x < peak_at {
                x / peak_at
            } else {
                (len as f32 - x) / (len as f32 - peak_at)
            };
            tri - 0.5 + noise * ns.next()
        })
        .collect();
    let mut out = Vec::with_capacity(n);
    let mut p = 0;
    let mut z;
    for _ in 0..n {
        let a = line[p];
        let b = line[(p + 1) % len];
        z = 0.5 * (a + b) * 0.996;
        line[p] = z;
        out.push(a);
        p = (p + 1) % len;
    }
    testkit::normalise(&mut out, amp);
    out
}

/// Sung, then a breath, then plucked: something for every engine to work on.
pub(super) fn material(seed: u64) -> (Vec<f32>, Vec<f32>) {
    let mut x = testkit::voice(SR, 0.6, Vowel::A, 0.5, testkit::vibrato(196.0, 30.0, 5.5));
    x.extend(testkit::breath(SR, 0.1, 0.05, seed));
    x.extend(testkit::voice(
        SR,
        0.4,
        Vowel::I,
        0.4,
        testkit::glide(220.0, 330.0, 0.3),
    ));
    x.extend(pluck(110.0, 0.5, 0.6, seed + 1));
    let mut ns = Noise::new(seed + 2);
    let r: Vec<f32> = x.iter().map(|v| 0.8 * v + 0.01 * ns.next()).collect();
    (x, r)
}

/// Run `s` over `(l, r)` in blocks of `block` (0: a varying split).
pub(super) fn run(s: &SaltusSettings, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let mut u = Saltus::new(SR);
    u.set(s);
    u.reset();
    run_on(&mut u, l, r, block)
}

pub(super) fn run_on(u: &mut Saltus, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    let mut at = 0;
    let mut k = 0usize;
    while at < a.len() {
        let n = if block == 0 {
            [1, 7, 64, 300, 13, 512][k % 6]
        } else {
            block
        }
        .min(a.len() - at);
        u.process(&mut a[at..at + n], &mut b[at..at + n]);
        at += n;
        k += 1;
    }
    (a, b)
}

/// Every mode and engine, at musical settings.
pub(super) fn every_setting() -> Vec<(&'static str, SaltusSettings)> {
    let d = SaltusSettings::default();
    let mut v = Vec::new();
    let mut shift = d;
    shift.voices[0].semitones = 7.0;
    shift.voices[1] = VoiceSettings {
        semitones: -5.0,
        cents: 12.0,
        degree: 0,
        level: 0.6,
        pan: 0.5,
    };
    shift.dry = 0.7;
    v.push(("shift", shift));
    let mut throat = d;
    throat.throat = -3.0;
    throat.formants = false;
    throat.voices[0].semitones = -12.0;
    v.push(("shift, formants moved", throat));
    let mut poly = shift;
    poly.engine = Engine::Poly;
    v.push(("shift poly", poly));
    let mut harm = d;
    harm.mode = Mode::Harmony;
    harm.dry = 1.0;
    harm.voices[0].level = 0.7;
    harm.voices[1].level = 0.5;
    harm.voices[1].pan = -0.4;
    v.push(("harmony", harm));
    let mut chord = harm;
    chord.harmony_source = HarmonySource::Chord;
    v.push(("harmony chord", chord));
    let mut melody = harm;
    melody.harmony_source = HarmonySource::Melody;
    v.push(("harmony melody", melody));
    let mut oct = d;
    oct.mode = Mode::Octave;
    oct.dry = 1.0;
    oct.sub1 = 0.8;
    oct.sub2 = 0.5;
    oct.up1 = 0.4;
    v.push(("octave", oct));
    let mut clean = oct;
    clean.octave_style = OctaveStyle::Clean;
    v.push(("octave clean", clean));
    let mut wh = d;
    wh.mode = Mode::Whammy;
    wh.pedal = 0.7;
    v.push(("whammy", wh));
    let mut whp = wh;
    whp.engine = Engine::Poly;
    whp.dry = 0.3;
    v.push(("whammy poly", whp));
    v
}

fn assert_same(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>), what: &str) {
    for (ch, (x, y)) in [(&a.0, &b.0), (&a.1, &b.1)].into_iter().enumerate() {
        if let Some(i) = x
            .iter()
            .zip(y)
            .position(|(p, q)| p.to_bits() != q.to_bits())
        {
            panic!("{what}: channel {ch} differs at {i}: {} vs {}", x[i], y[i]);
        }
    }
}

/// The null: at the defaults the input `latency` samples later, bit for bit — noise (with
/// −0.0 and denormals in it), a voice and silence.
#[test]
fn at_its_defaults_it_is_the_input_delayed_bit_for_bit() {
    let mut ns = Noise::new(9);
    let mut l: Vec<f32> = (0..30_000).map(|_| ns.next()).collect();
    let mut r: Vec<f32> = (0..30_000).map(|_| ns.next() * 0.25).collect();
    l[100] = -0.0;
    r[200] = 1e-40;
    let (v, _) = material(4);
    l.extend(&v);
    r.extend(v.iter().map(|x| -x));
    l.extend(std::iter::repeat_n(0.0, 5_000));
    r.extend(std::iter::repeat_n(0.0, 5_000));
    let lat = Saltus::new(SR).latency();
    assert_eq!(lat, 128);
    let (a, b) = run(&SaltusSettings::default(), &l, &r, 0);
    for i in 0..l.len() {
        let (wl, wr) = if i >= lat {
            (l[i - lat], r[i - lat])
        } else {
            (0.0, 0.0)
        };
        assert_eq!(a[i].to_bits(), wl.to_bits(), "left at {i}");
        assert_eq!(b[i].to_bits(), wr.to_bits(), "right at {i}");
    }
}

/// Every mode's other controls at their defaults are unheard in Shift: the null holds
/// whatever key, scale, pedal or octave levels say.
#[test]
fn the_other_modes_controls_do_not_touch_the_null() {
    let mut s = SaltusSettings::default();
    s.key = 7;
    s.scale = Scale::Blues;
    s.pedal = 1.0;
    s.toe = -24.0;
    s.sub2 = 1.0;
    s.up1 = 1.0;
    s.harmony_source = HarmonySource::Melody;
    s.octave_style = OctaveStyle::Clean;
    s.voices[0].degree = -3;
    s.voices[1].semitones = 12.0;
    s.a4 = 432.0;
    let (l, r) = material(5);
    let a = run(&s, &l, &r, 64);
    let b = run(&SaltusSettings::default(), &l, &r, 64);
    assert_same(&a, &b, "other modes' controls");
}

/// The latency is a function of the mode, the engine and the octave's style — the same
/// time at every rate.
#[test]
fn each_mode_reports_its_fixed_latency() {
    let d = SaltusSettings::default();
    let with = |f: &dyn Fn(&mut SaltusSettings)| {
        let mut s = d;
        f(&mut s);
        s
    };
    let table: [(SaltusSettings, [usize; 3]); 7] = [
        (d, [118, 128, 256]),
        (with(&|s| s.engine = Engine::Poly), [2_047, 2_047, 4_095]),
        (with(&|s| s.mode = Mode::Harmony), [118, 128, 256]),
        (with(&|s| s.mode = Mode::Octave), [0, 0, 0]),
        (
            with(&|s| {
                s.mode = Mode::Octave;
                s.octave_style = OctaveStyle::Clean;
            }),
            [118, 128, 256],
        ),
        (with(&|s| s.mode = Mode::Whammy), [118, 128, 256]),
        (
            with(&|s| {
                s.mode = Mode::Whammy;
                s.engine = Engine::Poly;
            }),
            [2_047, 2_047, 4_095],
        ),
    ];
    for (s, want) in table {
        for (sr, w) in [44_100.0f32, 48_000.0, 96_000.0].into_iter().zip(want) {
            let mut u = Saltus::new(sr);
            u.set(&s);
            assert_eq!(u.latency(), w, "{:?} {:?} at {sr}", s.mode, s.engine);
            assert_eq!(latency_for(&s, sr), w);
        }
    }
}

/// The latency is what the output shows: an impulse through each engine at unity comes
/// out where the latency says.
#[test]
fn the_reported_latency_is_where_the_dry_comes_out() {
    for (name, mut s) in every_setting() {
        s.dry = 1.0;
        for v in &mut s.voices {
            v.level = 0.0;
        }
        s.sub1 = 0.0;
        s.sub2 = 0.0;
        s.up1 = 0.0;
        let mut l = vec![0.0f32; 6_000];
        l[1_000] = 1.0;
        let (a, _) = run(&s, &l, &l, 64);
        let lat = latency_for(&s, SR);
        let at = a.iter().position(|v| *v != 0.0).expect(name);
        assert_eq!(at, 1_000 + lat, "{name}");
        assert_eq!(a[at], 1.0, "{name}");
    }
}

/// The same bits at every block size, in every mode.
#[test]
fn every_mode_is_block_size_independent() {
    let (l, r) = material(11);
    for (name, s) in every_setting() {
        let reference = run(&s, &l, &r, 1);
        for block in [37, 64, 512, 0] {
            let got = run(&s, &l, &r, block);
            assert_same(&reference, &got, &format!("{name}, blocks of {block}"));
        }
    }
}

/// A note's timing is its offset, whatever the blocks: the melody voice moves on the
/// sample the note says.
#[test]
fn notes_act_at_their_sample_at_any_block_size() {
    let (l, r) = material(13);
    let mut s = SaltusSettings::default();
    s.mode = Mode::Harmony;
    s.harmony_source = HarmonySource::Melody;
    s.voices[1].level = 0.6;
    let events = [
        (2_000usize, NoteEvent::On(64)),
        (9_000, NoteEvent::On(67)),
        (20_001, NoteEvent::Off(64)),
        (31_000, NoteEvent::On(60)),
        (40_000, NoteEvent::AllOff),
    ];
    let render = |block: usize| {
        let mut u = Saltus::new(SR);
        u.set(&s);
        let (mut a, mut b) = (l.clone(), r.clone());
        let mut at = 0;
        while at < a.len() {
            let n = block.min(a.len() - at);
            for (t, e) in events {
                if t >= at && t < at + n {
                    u.note(t - at, e);
                }
            }
            u.process(&mut a[at..at + n], &mut b[at..at + n]);
            at += n;
        }
        (a, b)
    };
    let reference = render(1);
    for block in [37, 64, 512] {
        assert_same(
            &reference,
            &render(block),
            &format!("notes, blocks of {block}"),
        );
    }
}

/// The four twins of the engine's contract: after sound and then silence, one keeps
/// processing silence (exact zeros, still at rest), one is skipped, one is reset and one is
/// new (both skipped as far): all four the same bits when sound returns.
#[test]
fn at_rest_is_exact_in_every_mode() {
    let (l, r) = material(17);
    let (l2, r2) = material(23);
    for (name, s) in every_setting() {
        let mut a = Saltus::new(SR);
        a.set(&s);
        if s.harmony_source != HarmonySource::Scale {
            a.note(0, NoteEvent::On(64));
        }
        run_on(&mut a, &l, &r, 0);
        a.note(0, NoteEvent::AllOff);
        let mut silent = 0usize;
        let z = vec![0.0f32; 480];
        while !a.at_rest() {
            assert!(
                silent < 10 * 48_000,
                "{name}: never at rest — {:?}",
                a.rest_blocker()
            );
            run_on(&mut a, &z, &z, 480);
            silent += 480;
        }
        let mut d = a.clone();
        let mut b = a.clone();
        // a: silence in, silence out, at rest throughout.
        let k = 7_777;
        let zz = vec![0.0f32; k];
        let out = run_on(&mut a, &zz, &zz, 0);
        assert!(
            out.0.iter().chain(&out.1).all(|v| v.to_bits() == 0),
            "{name}"
        );
        assert!(a.at_rest(), "{name}: silence left it not at rest");
        let total = l.len() + silent + k;
        d.skip(k);
        assert!(d.at_rest(), "{name}");
        b.reset();
        b.skip(total);
        let mut c = Saltus::new(SR);
        c.set(&s);
        c.skip(total);
        assert!(
            b.at_rest() && c.at_rest(),
            "{name}: reset or new not at rest"
        );
        let want = run_on(&mut a, &l2, &r2, 0);
        for (twin, u) in [("skipped", &mut d), ("reset", &mut b), ("new", &mut c)] {
            let got = run_on(u, &l2, &r2, 0);
            assert_same(&want, &got, &format!("{name}: the {twin} twin"));
        }
        eprintln!(
            "{name}: at rest after {:.2} s of silence",
            silent as f32 / SR
        );
    }
}

/// Every control at its extremes, on loud material: finite, bounded.
#[test]
fn extremes_stay_finite_and_bounded() {
    let (l, r) = material(29);
    let l: Vec<f32> = l.iter().map(|v| v * 1.9).collect();
    for (name, mut s) in every_setting() {
        for (st, deg, throat, form) in [(24.0, 14, 12.0, false), (-24.0, -14, -12.0, true)] {
            for v in &mut s.voices {
                v.semitones = st;
                v.cents = 100.0;
                v.degree = deg;
                v.level = 1.0;
                v.pan = 1.0;
            }
            s.throat = throat;
            s.formants = form;
            s.heel = -st;
            s.toe = st;
            s.pedal = 1.0;
            s.glide_ms = 0.0;
            s.sub1 = 1.0;
            s.sub2 = 1.0;
            s.up1 = 1.0;
            s.dry = 1.0;
            s.out_db = 24.0;
            let (a, b) = run(&s, &l, &r, 0);
            let peak = a.iter().chain(&b).fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(
                peak.is_finite() && peak < 2_000.0,
                "{name} {st}: peak {peak}"
            );
        }
    }
}

/// Switching the engine mid-stream starts the new one as a new unit would (brought to the
/// count): the same bits as a unit set that way from the start, once its latency is past.
#[test]
fn a_new_engine_starts_as_a_new_units() {
    let (l, r) = material(31);
    let half = l.len() / 2;
    for (name, s) in every_setting() {
        let mut a = Saltus::new(SR);
        // The same settings on another engine (the pedal and the levels as they are).
        let mut other = s;
        if path_of(&s) == Path::Poly {
            other.engine = Engine::Mono;
        } else {
            other.mode = Mode::Shift;
            other.engine = Engine::Poly;
        }
        a.set(&other);
        run_on(&mut a, &l[..half], &r[..half], 64);
        a.set(&s);
        // A unit set to `s` from the start, fed silence up to `half`: the same engine state
        // as a fresh entry at `half` would be, but for the input it heard.
        let zl = vec![0.0f32; half];
        let mut b = Saltus::new(SR);
        b.set(&s);
        run_on(&mut b, &zl, &zl, 64);
        // Entering, the engine forgets the input before `half`: so does `b`'s (silence).
        let (x, _) = run_on(&mut a, &l[half..], &r[half..], 64);
        let (y, _) = run_on(&mut b, &l[half..], &r[half..], 64);
        let lat = latency_for(&s, SR);
        // The dry line is not reset (it is continuous), and it is heard in `a` only for the
        // latency's span after the switch: compare from there on.
        let from = lat + 1;
        let differs = x[from..]
            .iter()
            .zip(&y[from..])
            .position(|(p, q)| p.to_bits() != q.to_bits());
        assert_eq!(differs, None, "{name}");
    }
}

/// A copy (the engine forks units) is the unit: the same bits from there on, in every
/// mode, mid-sound — what a clone of a list with reserved room would not keep.
#[test]
fn a_copy_is_the_unit() {
    let (l, r) = material(17);
    let (l2, r2) = material(23);
    for (name, s) in every_setting() {
        let mut a = Saltus::new(SR);
        a.set(&s);
        a.note(0, NoteEvent::On(67));
        run_on(&mut a, &l, &r, 0);
        let mut d = a.clone();
        let x = run_on(&mut a, &l2, &r2, 0);
        let y = run_on(&mut d, &l2, &r2, 0);
        assert_same(&x, &y, name);
    }
}

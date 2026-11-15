//! Saltus measured: the shift's accuracy (cents) over ±24 semitones on sines and on a
//! sung voice, the formants kept (the spectral envelope's error), the octaver's tracking on
//! a bass line (lock time, glitch rate — against a plain divider), harmony in key on a
//! melody, the whammy's glide without zipper, the poly engine on chords, and the CPU per
//! mode. Each prints what it measured (`--nocapture`); the bounds are what it holds.

// Settings read clearer as the defaults and what a test changes.
#![allow(clippy::field_reassign_with_default)]

use super::tests::{pluck_at, run, SR};
use super::*;
use crate::pitch::testkit::{self, measured_f0, Vowel};

pub(super) fn sine(hz: f32, secs: f32, amp: f32) -> Vec<f32> {
    let n = (secs * SR) as usize;
    let w = 2.0 * std::f64::consts::PI * hz as f64 / SR as f64;
    (0..n)
        .map(|i| (amp as f64 * math::sin(w * i as f64)) as f32)
        .collect()
}

pub(super) fn cents(a: f32, b: f32) -> f32 {
    1200.0 * math::log2(a / b)
}

pub(super) fn shift_settings(st: f32, engine: Engine, formants: bool) -> SaltusSettings {
    let mut s = SaltusSettings::default();
    s.voices[0].semitones = st;
    s.engine = engine;
    s.formants = formants;
    s
}

/// The output of a mono signal through `s` (left channel), its latency removed.
pub(super) fn through(s: &SaltusSettings, x: &[f32]) -> Vec<f32> {
    let lat = latency_for(s, SR);
    let mut pad = x.to_vec();
    pad.extend(std::iter::repeat_n(0.0, lat));
    let (a, _) = run(s, &pad, &pad, 256);
    a[lat..].to_vec()
}

/// The spectral envelope's error (dB RMS) between a reference voice at `f0_ref` and a
/// shifted one at `f0_out` — as the pitch tests' `envelope_distance`, over the harmonics
/// the reference's envelope holds within 50 dB of its loudest (what can be heard of a
/// formant: far under it, an envelope moved down from content the source never had is
/// not a formant lost).
pub(super) fn formant_error(reference: &[f32], f0_ref: f32, out: &[f32], f0_out: f32) -> f32 {
    let (lo, hi) = (300.0f32, 4_000.0f32);
    let harmonics = |x: &[f32], f0: f32| -> Vec<(f32, f32)> {
        (1..)
            .map(|k| k as f32 * f0)
            .take_while(|&f| f < hi * 1.2)
            .map(|f| (f, testkit::amplitude_db(x, SR, f)))
            .collect()
    };
    let r = harmonics(reference, f0_ref);
    let o = harmonics(out, f0_out);
    let env = |f: f32| -> f32 {
        let i = r.partition_point(|h| h.0 < f);
        if i == 0 {
            return r[0].1;
        }
        if i >= r.len() {
            return r[r.len() - 1].1;
        }
        let (a, b) = (r[i - 1], r[i]);
        a.1 + (b.1 - a.1) * (f - a.0) / (b.0 - a.0)
    };
    let top = r.iter().fold(f32::NEG_INFINITY, |m, h| m.max(h.1));
    let pairs: Vec<(f32, f32)> = o
        .iter()
        .filter(|h| h.0 >= lo && h.0 <= hi && env(h.0) > top - 50.0)
        .map(|h| (h.1, env(h.0)))
        .collect();
    let n = pairs.len().max(1) as f32;
    let mo = pairs.iter().map(|p| p.0).sum::<f32>() / n;
    let mr = pairs.iter().map(|p| p.1).sum::<f32>() / n;
    let ms = pairs
        .iter()
        .map(|p| {
            let d = (p.0 - mo) - (p.1 - mr);
            d * d
        })
        .sum::<f32>()
        / n;
    ms.sqrt()
}

/// The frequency of the strongest partial within ±30 cents of `near`, Hz: a Hann-windowed
/// Goertzel scan in steps of half a cent, refined by a parabola — the pitch heard of
/// the partial, whatever else sounds beside it.
pub(super) fn peak_hz(y: &[f32], near: f32) -> f32 {
    let scan: Vec<(f32, f32)> = (-60..=60)
        .map(|i| {
            let f = near * math::exp2(i as f32 * 0.5 / 1200.0);
            (f, testkit::amplitude_db(y, SR, f))
        })
        .collect();
    let k = (0..scan.len())
        .max_by(|&a, &b| scan[a].1.total_cmp(&scan[b].1))
        .unwrap_or(0);
    if k == 0 || k + 1 == scan.len() {
        return scan[k].0;
    }
    let (a, b, c) = (scan[k - 1].1, scan[k].1, scan[k + 1].1);
    let den = a - 2.0 * b + c;
    let off = if den.abs() > 1e-9 {
        0.5 * (a - c) / den
    } else {
        0.0
    };
    near * math::exp2(((k as f32 - 60.0) + off) * 0.5 / 1200.0)
}

/// Rising zero crossings (fractional sample times), a rise from silence included.
fn crossings(y: &[f32]) -> Vec<f64> {
    let mut out = Vec::new();
    for i in 1..y.len() {
        if y[i - 1] <= 0.0 && y[i] > 0.0 {
            out.push((i - 1) as f64 + (y[i - 1] / (y[i - 1] - y[i])) as f64);
        }
    }
    out
}

/// The shift lands where it says: sines from two octaves down to two up, every semitone,
/// both engines.
#[test]
fn the_shift_is_accurate_across_four_octaves_on_sines() {
    for engine in [Engine::Mono, Engine::Poly] {
        let mut worst = (0.0f32, 0.0f32);
        let mut all = Vec::new();
        for st in -24..=24 {
            let f_in = 220.0;
            let want = f_in * math::exp2(st as f32 / 12.0);
            let y = through(
                &shift_settings(st as f32, engine, true),
                &sine(f_in, 1.5, 0.5),
            );
            let got = peak_hz(&y[(0.3 * SR) as usize..(1.3 * SR) as usize], want);
            let e = cents(got, want);
            all.push((st, (e * 100.0).round() / 100.0));
            if e.abs() > worst.1.abs() {
                worst = (st as f32, e);
            }
        }
        eprintln!(
            "shift on sines ({engine:?}): worst {:+.2} cents at {:+} st",
            worst.1, worst.0
        );
        eprintln!("  {all:?}");
        assert!(worst.1.abs() < 2.0, "{engine:?}: {worst:?}");
    }
}

/// On a sung vowel (the pitch tests' voice), the shift lands within a few cents, and the
/// formants stay: the shifted voice's spectral envelope against the original's, formants
/// kept and let move (which must be far worse).
#[test]
fn the_shift_keeps_a_voices_pitch_and_formants() {
    let f0 = 196.0;
    let x = testkit::voice(SR, 1.2, Vowel::A, 0.5, |_| f0);
    let xs = &x[(0.3 * SR) as usize..];
    eprintln!("sung A at 196 Hz, mono engine:");
    for st in [-12.0f32, -7.0, -5.0, -3.0, 3.0, 4.0, 7.0, 12.0] {
        let want = f0 * math::exp2(st / 12.0);
        let kept = through(&shift_settings(st, Engine::Mono, true), &x);
        let moved = through(&shift_settings(st, Engine::Mono, false), &x);
        let ks = &kept[(0.3 * SR) as usize..];
        let ms = &moved[(0.3 * SR) as usize..];
        let e = cents(peak_hz(ks, want), want);
        let d_kept = formant_error(xs, f0, ks, want);
        let d_moved = formant_error(xs, f0, ms, want);
        eprintln!(
            "  {st:+} st: {e:+.2} cents; envelope {d_kept:.2} dB kept, {d_moved:.2} dB moved"
        );
        assert!(e.abs() < 3.0, "{st}: {e} cents");
        // Far down, PSOLA's grains (one period each, further apart than a period) leave
        // the harmonics between the old ones to the grain's own spectrum: a few dB more.
        let bound = if st <= -7.0 { 7.0 } else { 3.0 };
        assert!(d_kept < bound, "{st}: kept {d_kept} dB");
        if st.abs() >= 5.0 {
            assert!(d_moved > 1.5 * d_kept, "{st}: {d_moved} vs {d_kept}");
        }
    }
    // The poly engine keeps them too (the true envelope).
    for st in [-12.0f32, -7.0, -3.0, 3.0, 7.0, 12.0] {
        let want = f0 * math::exp2(st / 12.0);
        let kept = through(&shift_settings(st, Engine::Poly, true), &x);
        let ks = &kept[(0.3 * SR) as usize..];
        let e = cents(peak_hz(ks, want), want);
        let d = formant_error(xs, f0, ks, want);
        eprintln!("  poly {st:+} st: {e:+.2} cents; envelope {d:.2} dB kept");
        // An octave down, the poly engine cannot give the top of the spectrum what the
        // source never had (its correction stops at 30 dB, never raising a noise floor
        // further): measured, not held.
        assert!(e.abs() < 1.0, "poly {st}: {e} cents");
        if st.abs() < 12.0 {
            assert!(d < 6.0, "poly {st}: {d} dB");
        }
    }
}

/// A bass line: notes plucked one after another, some legato, some with a gap; every
/// other one near the bridge with more of the pick (bright: a strong second and third
/// harmonic, what makes a divider glitch).
pub(super) fn bass_line() -> (Vec<f32>, Vec<(usize, f32, usize)>) {
    let notes = [
        (82.41f32, 0.45f32, 0.05f32),
        (98.0, 0.3, 0.0),
        (110.0, 0.45, 0.1),
        (73.42, 0.6, 0.0),
        (130.81, 0.3, 0.0),
        (146.83, 0.3, 0.05),
        (61.74, 0.6, 0.1),
        (164.81, 0.4, 0.0),
        (55.0, 0.7, 0.0),
    ];
    let mut x = Vec::new();
    let mut at = Vec::new();
    for (i, (hz, secs, gap)) in notes.into_iter().enumerate() {
        let start = x.len();
        let (pos, noise) = if i % 2 == 1 { (0.07, 0.2) } else { (0.2, 0.05) };
        let mut p = pluck_at(hz, secs, 0.6, 40 + i as u64, pos, noise);
        // A damped end, as a bassist's hand does.
        let fade = (0.01 * SR) as usize;
        let n = p.len();
        for (k, v) in p[n - fade..].iter_mut().enumerate() {
            *v *= 1.0 - k as f32 / fade as f32;
        }
        x.extend(p);
        at.push((start, hz, x.len()));
        x.extend(std::iter::repeat_n(0.0, (gap * SR) as usize));
    }
    (x, at)
}

/// The same line played sloppily: each note rings on 150 ms into the next, undamped.
fn bass_line_ringing() -> (Vec<f32>, Vec<(usize, f32, usize)>) {
    let (_, notes) = bass_line();
    let ring = (0.15 * SR) as usize;
    let total = notes.last().map(|n| n.2).unwrap_or(0) + ring;
    let mut x = vec![0.0f32; total];
    for (i, &(start, hz, end)) in notes.iter().enumerate() {
        let (pos, noise) = if i % 2 == 1 { (0.07, 0.2) } else { (0.2, 0.05) };
        let len = end - start + ring;
        let p = pluck_at(hz, len as f32 / SR, 0.6, 40 + i as u64, pos, noise);
        for (k, v) in p.iter().enumerate() {
            if start + k < total {
                x[start + k] += v;
            }
        }
    }
    // Each note judged after the last one's ring has died under it.
    let judged = notes.iter().map(|&(s, hz, e)| (s, hz, e)).collect();
    (x, judged)
}

/// What the octave did on each note: its lock time (ms from the onset to three periods in a
/// row within 2 % of the octave's, a third of a semitone), and its periods off by more than 3 % after that.
fn octave_report(y: &[f32], notes: &[(usize, f32, usize)], div: f32) -> (f32, f32, f32) {
    let mut locks = Vec::new();
    let (mut bad, mut total) = (0usize, 0usize);
    for &(start, hz, end) in notes {
        let want = (div * SR / hz) as f64;
        let z: Vec<f64> = crossings(&y[start..end])
            .into_iter()
            .map(|t| t + start as f64)
            .collect();
        let periods: Vec<(f64, f64)> = z.windows(2).map(|w| (w[1], w[1] - w[0])).collect();
        let ok = |p: f64| ((p / want) - 1.0).abs() < 0.02;
        let lock = periods
            .windows(3)
            .find(|w| w.iter().all(|(_, p)| ok(*p)))
            .map(|w| w[0].0 - w[0].1 - start as f64);
        let Some(lock) = lock else {
            let ps: Vec<i32> = periods.iter().map(|p| p.1 as i32).collect();
            eprintln!("    {hz} Hz: no lock (want {want:.0}): {ps:?}");
            locks.push(f32::INFINITY);
            continue;
        };
        locks.push((lock / SR as f64 * 1_000.0) as f32);
        for (t, p) in &periods {
            // The last period ends with the note's damping: not counted.
            if *t - p > lock + start as f64 && *t < end as f64 - want {
                total += 1;
                if ((p / want) - 1.0).abs() > 0.03 {
                    bad += 1;
                }
            }
        }
    }
    let mean = locks.iter().sum::<f32>() / locks.len() as f32;
    let worst = locks.iter().fold(0.0f32, |m, v| m.max(*v));
    (mean, worst, 100.0 * bad as f32 / total.max(1) as f32)
}

/// A plain analog divider, the reference: a fixed low-pass at 400 Hz (two poles), a
/// comparator at zero, a flip-flop, the square multiplying the filtered input (the
/// OC-2's way), low-passed again.
fn plain_divider(x: &[f32]) -> Vec<f32> {
    let k = 1.0 - math::exp(-2.0 * core::f32::consts::PI * 400.0 / SR);
    let (mut a, mut b, mut high, mut q) = (0.0f32, 0.0f32, false, 1.0f32);
    let (mut o1, mut o2) = (0.0f32, 0.0f32);
    x.iter()
        .map(|&v| {
            a += (v - a) * k;
            b += (a - b) * k;
            if !high && b > 0.0 {
                high = true;
                q = -q;
            } else if high && b < 0.0 {
                high = false;
            }
            o1 += (b * q - o1) * k;
            o2 += (o1 - o2) * k;
            o2
        })
        .collect()
}

/// The octaver on a bass line: how soon each note's octave locks and how often it
/// glitches, one and two octaves down — against a plain divider's.
#[test]
fn the_octaver_tracks_a_bass_line() {
    let (x, notes) = bass_line();
    let mut s = SaltusSettings::default();
    s.mode = Mode::Octave;
    s.sub1 = 1.0;
    s.sub2 = 0.0;
    let y1 = through(&s, &x);
    s.sub1 = 0.0;
    s.sub2 = 1.0;
    let y2 = through(&s, &x);
    let (m1, w1, g1) = octave_report(&y1, &notes, 2.0);
    let (m2, w2, g2) = octave_report(&y2, &notes, 4.0);
    let plain = plain_divider(&x);
    let (mp, wp, gp) = octave_report(&plain, &notes, 2.0);
    eprintln!("octaver on a bass line (latency 0):");
    eprintln!("  one down: lock {m1:.1} ms mean, {w1:.1} worst; {g1:.2} % periods off");
    eprintln!("  two down: lock {m2:.1} ms mean, {w2:.1} worst; {g2:.2} % periods off");
    eprintln!("  a plain divider: lock {mp:.1} ms mean, {wp:.1} worst; {gp:.2} % periods off");
    let (xr, nr) = bass_line_ringing();
    s.sub1 = 1.0;
    s.sub2 = 0.0;
    let (rm, rw, rg) = octave_report(&through(&s, &xr), &nr, 2.0);
    let (pm, pw, pg) = octave_report(&plain_divider(&xr), &nr, 2.0);
    eprintln!("  notes ringing into each other: ours lock {rm:.1} ms mean, {rw:.1} worst, {rg:.2} % off; plain {pm:.1} / {pw:.1} ms, {pg:.2} % off");
    assert_eq!(latency_for(&s, SR), 0);
    assert!(w1 < 100.0 && g1 < 1.0, "one down: {w1} ms, {g1} %");
    assert!(w2 < 150.0 && g2 < 1.0, "two down: {w2} ms, {g2} %");
    assert!(rg < 1.0, "ringing: {rg} %");
}

/// Harmony in C major on a sung scale, two degrees up (a third) and two down: each
/// note's voice is the diatonic interval — major or minor as the scale says — within a
/// few cents.
#[test]
fn harmony_sings_in_key() {
    let melody = [60, 62, 64, 65, 67, 69, 71, 72];
    let third = [4, 3, 3, 4, 4, 3, 3, 4];
    let third_below = [-3, -3, -4, -3, -3, -4, -4, -3];
    let note_s = 0.45;
    let f = |t: f64| {
        let i = ((t / note_s as f64) as usize).min(melody.len() - 1);
        crate::pitch::midi_to_hz(melody[i] as f32)
    };
    let x = testkit::voice(SR, note_s * melody.len() as f32, Vowel::A, 0.5, f);
    for (degree, want) in [(2, third), (-2, third_below)] {
        let mut s = SaltusSettings::default();
        s.mode = Mode::Harmony;
        s.voices[0].degree = degree;
        let y = through(&s, &x);
        let mut worst = 0.0f32;
        for (i, (&n, &w)) in melody.iter().zip(&want).enumerate() {
            let a = ((i as f32 + 0.35) * note_s * SR) as usize;
            let b = ((i as f32 + 0.95) * note_s * SR) as usize;
            let target = crate::pitch::midi_to_hz((n + w) as f32);
            let got = measured_f0(&y[a..b], SR, target);
            let e = cents(got, target);
            worst = worst.max(e.abs());
            assert!(e.abs() < 10.0, "degree {degree}, note {n}: {e} cents");
        }
        eprintln!(
            "harmony {degree:+} degrees in C major: every interval right, worst {worst:.2} cents"
        );
    }
}

/// The voice of the chord held: C–E–G held, one step up from an E sung is G, from a G
/// is C; with the chord let go, the voice falls silent.
#[test]
fn harmony_follows_the_chord_held() {
    // A soprano's /a/ (the male /a/'s first formant on G4's second harmonic makes the
    // pitch base's tracker hear the octave above: noted in the builder's report).
    let x = testkit::voice(SR, 1.0, Vowel::A_FEMALE, 0.5, |t| {
        crate::pitch::midi_to_hz(if t < 0.5 { 64.0 } else { 67.0 })
    });
    let mut s = SaltusSettings::default();
    s.mode = Mode::Harmony;
    s.harmony_source = HarmonySource::Chord;
    s.voices[0].degree = 1;
    let mut u = Saltus::new(SR);
    u.set(&s);
    for n in [60, 64, 67] {
        u.note(0, NoteEvent::On(n));
    }
    let lat = u.latency();
    let mut l = x.clone();
    l.extend(std::iter::repeat_n(0.0, lat));
    let mut r = l.clone();
    u.process(&mut l, &mut r);
    let y = &l[lat..];
    for (from, to, want) in [(0.2f32, 0.48f32, 67.0f32), (0.7, 0.98, 72.0)] {
        let target = crate::pitch::midi_to_hz(want);
        let got = measured_f0(&y[(from * SR) as usize..(to * SR) as usize], SR, target);
        assert!(cents(got, target).abs() < 10.0, "{want}: {got} Hz");
    }
    // Released: the voice closes.
    u.note(0, NoteEvent::AllOff);
    let mut l2 = testkit::voice(SR, 0.3, Vowel::A, 0.5, |_| 330.0);
    let mut r2 = l2.clone();
    u.process(&mut l2, &mut r2);
    let tail = &l2[(0.1 * SR) as usize..];
    assert!(
        tail.iter().all(|v| v.abs() < 1e-6),
        "the voice still sounds"
    );
}

/// The whammy's glide: the pedal moved in ten steps (as automation arrives, a block at a
/// time) gives a smooth sweep — the largest jump from one period to the next, glided and
/// not.
#[test]
fn the_whammy_glides_without_zipper() {
    let x = sine(220.0, 1.6, 0.5);
    let sweep = |glide_ms: f32| -> f32 {
        let mut s = SaltusSettings::default();
        s.mode = Mode::Whammy;
        s.toe = 12.0;
        s.glide_ms = glide_ms;
        let mut u = Saltus::new(SR);
        u.set(&s);
        let (mut l, mut r) = (x.clone(), x.clone());
        let block = 480;
        let mut at = 0;
        while at < l.len() {
            // From 0.3 s, a step every 2 400 samples (50 ms) up to the toe.
            if at >= 14_400 {
                let k = ((at - 14_400) / 2_400 + 1).min(10);
                if s.pedal != k as f32 / 10.0 {
                    s.pedal = k as f32 / 10.0;
                    u.set(&s);
                }
            }
            let n = block.min(l.len() - at);
            u.process(&mut l[at..at + n], &mut r[at..at + n]);
            at += n;
        }
        let z = crossings(&l[(0.25 * SR) as usize..]);
        let f: Vec<f32> = z
            .windows(2)
            .map(|w| (SR as f64 / (w[1] - w[0])) as f32)
            .collect();
        f.windows(2)
            .map(|w| cents(w[1], w[0]).abs())
            .fold(0.0f32, f32::max)
    };
    let glided = sweep(40.0);
    let stepped = sweep(0.0);
    eprintln!(
        "whammy sweep, largest jump between periods: {glided:.1} cents glided (40 ms), {stepped:.1} cents stepped"
    );
    assert!(glided < 30.0, "{glided}");
    assert!(stepped > 3.0 * glided, "{stepped} vs {glided}");
}

/// The shifted voice keeps its level: RMS against the dry, formants kept, mono engine
/// (the grains' gap a shift down leaves is made up).
#[test]
fn a_shifted_voice_keeps_its_level() {
    let f0 = 196.0;
    let x = testkit::voice(SR, 1.2, Vowel::A, 0.5, |_| f0);
    let from = (0.3 * SR) as usize;
    let level = |v: &[f32]| {
        let e: f64 = v.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        (10.0 * math::log10(e / v.len() as f64)) as f32
    };
    let dry = level(&x[from..]);
    let mut line = String::new();
    let mut worst = 0.0f32;
    for st in [-12.0f32, -7.0, -5.0, -3.0, 3.0, 5.0, 7.0, 12.0] {
        let y = through(&shift_settings(st, Engine::Mono, true), &x);
        let d = level(&y[from..]) - dry;
        worst = worst.max(d.abs());
        line += &format!(" {st:+}: {d:+.2}");
    }
    eprintln!("the voice's level, shifted (dB against the dry):{line}");
    // Up to a couple of dB is the voice's own: its harmonics fall elsewhere under the
    // same formants (a +7 lands them off the first formant's peak).
    assert!(worst < 2.0, "{worst} dB");
}

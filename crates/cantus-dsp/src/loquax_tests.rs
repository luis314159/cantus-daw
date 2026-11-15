//! Loquax: the null, the banks' response, block-size independence, rest.

use super::bank::{design, layout, Bank, BandDesign, MAX_BANDS};
use super::measure::{phrase, response_db};
use super::*;

pub(super) const SR: f32 = 48_000.0;

/// A deterministic stereo signal, `n` frames.
pub(super) fn noise(n: usize, seed: u64, gain: f32) -> (Vec<f32>, Vec<f32>) {
    let mut a = crate::pitch::testkit::Noise::new(seed);
    let l: Vec<f32> = (0..n).map(|_| a.next() * gain).collect();
    let r: Vec<f32> = (0..n).map(|_| a.next() * gain).collect();
    (l, r)
}

/// Keys pressed at frame 0.
pub(super) fn chord(keys: &[u8]) -> Vec<NoteEvent> {
    keys.iter()
        .map(|&key| NoteEvent {
            frame: 0,
            kind: NoteKind::On { key, velocity: 0.8 },
        })
        .collect()
}

/// `l`/`r` through `q` in blocks of `block`, with the sidechain and the notes (frames
/// counted from the start) given to each block rebased.
pub(super) fn render(
    q: &mut Loquax,
    l: &mut [f32],
    r: &mut [f32],
    side: Option<(&[f32], &[f32])>,
    notes: &[NoteEvent],
    block: usize,
) {
    let n = l.len();
    let mut at = 0usize;
    let mut ev: Vec<NoteEvent> = Vec::new();
    while at < n {
        let len = block.min(n - at);
        ev.clear();
        for e in notes {
            let f = e.frame as usize;
            if f >= at && f < at + len {
                ev.push(NoteEvent {
                    frame: (f - at) as u32,
                    kind: e.kind,
                });
            }
        }
        let sc = side.map(|(a, b)| (&a[at..at + len], &b[at..at + len]));
        q.process(&mut l[at..at + len], &mut r[at..at + len], sc, &ev);
        at += len;
    }
}

/// Settings with everything that moves the sound away from its default.
pub(super) fn busy() -> LoquaxSettings {
    let mut s = LoquaxSettings {
        mix: 1.0,
        out_db: -2.0,
        source: Source::Both,
        bands: 24,
        lo_hz: 120.0,
        hi_hz: 9_000.0,
        width: 1.3,
        order: 8,
        attack_ms: 1.0,
        release_ms: 60.0,
        formant: 3.5,
        band_shift: 0.75,
        tilt: 1.5,
        flatten: 0.6,
        sibilance: 0.7,
        unvoiced: 0.8,
        spread: 0.6,
        carrier_pass: 0.1,
        robot: true,
        robot_note: 50.0,
        ..LoquaxSettings::default()
    };
    s.band_db[3] = -6.0;
    s.band_db[10] = 4.0;
    s.synth.pulse = 0.4;
    s.synth.unison = 5;
    s
}

pub(super) fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn the_name_is_in_one_place() {
    assert_eq!(NAME, "Loquax");
}

#[test]
fn the_defaults_touch_no_sample() {
    let (l0, r0) = noise(20_000, 3, 0.5);
    let (sl, sr) = noise(20_000, 4, 0.5);
    let mut q = Loquax::new(SR);
    let (mut l, mut r) = (l0.clone(), r0.clone());
    let notes = chord(&[48, 52, 55]);
    render(&mut q, &mut l, &mut r, Some((&sl, &sr)), &notes, 256);
    assert_eq!(bits(&l), bits(&l0));
    assert_eq!(bits(&r), bits(&r0));
    // A −0.0 passes as −0.0.
    let (mut l, mut r) = (vec![-0.0f32; 64], vec![-0.0f32; 64]);
    q.process(&mut l, &mut r, None, &[]);
    assert!(l.iter().chain(&r).all(|x| x.to_bits() == (-0.0f32).to_bits()));
    assert_eq!(q.latency(), 0);
}

#[test]
fn at_mix_zero_every_other_control_moved_is_still_a_null() {
    let (l0, r0) = noise(12_000, 5, 0.5);
    let (sl, sr) = noise(12_000, 6, 0.5);
    let mut q = Loquax::new(SR);
    let mut s = busy();
    s.mix = 0.0;
    s.out_db = 0.0;
    q.set(&s);
    q.reset();
    let (mut l, mut r) = (l0.clone(), r0.clone());
    render(&mut q, &mut l, &mut r, Some((&sl, &sr)), &chord(&[60]), 100);
    assert_eq!(bits(&l), bits(&l0));
    assert_eq!(bits(&r), bits(&r0));
}

/// The analytic magnitude of the Butterworth band-pass of order `2m` whose −3 dB edges are
/// `lo` and `hi` (warped), at `hz`: `1 / √(1 + Ω^(2m))`, `Ω = (w/w0 − w0/w) / B`.
fn butterworth_db(lo: f32, hi: f32, m: usize, hz: f32, sr: f32) -> f32 {
    let wa = |f: f32| math::tan(core::f64::consts::PI * f64::from(f) / f64::from(sr));
    let (wl, wh, w) = (wa(lo), wa(hi), wa(hz));
    let w0 = (wl * wh).sqrt();
    let b = (wh - wl) / w0;
    let om = (w / w0 - w0 / w) / b;
    (-10.0 * math::log10(1.0 + math::powi(om * om, m as i32))) as f32
}

pub(super) fn band_ir(d: &BandDesign, m: usize, len: usize) -> Vec<f32> {
    let mut bank = Bank::default();
    bank.tune(core::slice::from_ref(d), m);
    let mut out = [0.0f32; MAX_BANDS];
    (0..len)
        .map(|i| {
            bank.run(if i == 0 { 1.0 } else { 0.0 }, &mut out);
            out[0]
        })
        .collect()
}

#[test]
fn every_band_is_the_butterworth_band_pass_it_was_designed_as() {
    let (mut worst, mut edge) = (0.0f32, 0.0f32);
    for &sr in &[44_100.0f32, 48_000.0, 96_000.0] {
        for &order in &ORDERS {
            let m = order / 2;
            for &n in &[8usize, 16, 32, 64] {
                for i in (0..n).step_by((n / 8).max(1)) {
                    let (c, lo, hi) = layout(i, n, 100.0, 8_000.0, 1.0);
                    let d = design(lo, hi, m, sr);
                    // Long enough for the narrowest band (64 bands, order 8, 96 kHz) to ring out.
                    let ir = band_ir(&d, m, 1 << 18);
                    let centre = response_db(&ir, d.centre, sr);
                    assert!(centre.abs() < 0.02, "{sr} order {order} n {n} band {i}: {centre}");
                    assert!((d.hi / d.lo - hi / lo).abs() < 1e-4 * hi / lo);
                    assert!(d.lo <= lo * 1.0001 && d.lo > lo * 0.97, "{} {lo}", d.lo);
                    for &f in &[d.lo, d.hi] {
                        let e = response_db(&ir, f, sr);
                        edge = edge.max((e + 3.0103).abs());
                        assert!((e + 3.0103).abs() < 0.02, "edge {f}: {e}");
                    }
                    // Across the skirts, the measured response is the analytic one.
                    for k in [-3.0f32, -1.5, -0.7, 0.3, 0.8, 1.6, 2.5] {
                        let f = c * math::exp2(k);
                        let want = butterworth_db(d.lo, d.hi, m, f, sr);
                        // Under −100 dB is the f32 states' own floor.
                        if f > 0.45 * sr || f < 10.0 || want < -100.0 {
                            continue;
                        }
                        let got = response_db(&ir, f, sr);
                        // Within 0.05 dB down to −60 dB, 1 dB below.
                        let tol = if want > -60.0 { 0.05 } else { 1.0 };
                        if want > -60.0 {
                            worst = worst.max((got - want).abs());
                        }
                        assert!((got - want).abs() < tol, "{sr} o{order} n{n} b{i} {f} Hz: {got} vs {want}");
                    }
                }
            }
        }
    }
    eprintln!(
        "band response: worst deviation from the analytic Butterworth {worst:.4} dB (to −60 dB); the edges' worst distance from −3.01 dB {edge:.4} dB"
    );
}

#[test]
fn the_slopes_are_their_order() {
    // An octave above the upper edge of a 16-band bank's band, the attenuation grows with
    // the order: 4th, 6th, 8th.
    let (c, lo, hi) = layout(6, 16, 100.0, 8_000.0, 1.0);
    let mut last = 0.0f32;
    for &order in &ORDERS {
        let m = order / 2;
        let d = design(lo, hi, m, SR);
        let ir = band_ir(&d, m, 1 << 16);
        let at = response_db(&ir, hi * 2.0, SR);
        eprintln!("order {order}: centre {c:.0} Hz, an octave above the edge {at:.1} dB");
        assert!(at < last - 6.0, "order {order}: {at} dB");
        last = at;
    }
    assert!(last < -40.0);
}

#[test]
fn the_bank_sums_flat_at_width_one() {
    for &n in &[16usize, 32] {
        let m = 3;
        let irs: Vec<Vec<f32>> = (0..n)
            .map(|i| {
                let (_, lo, hi) = layout(i, n, 100.0, 8_000.0, 1.0);
                band_ir(&design(lo, hi, m, SR), m, 1 << 15)
            })
            .collect();
        let (mut min, mut max) = (f32::MAX, f32::MIN);
        let mut f = 150.0f32;
        while f < 6_000.0 {
            let p: f32 = irs
                .iter()
                .map(|ir| math::db_to_gain(response_db(ir, f, SR) * 0.5))
                .map(|g| g * g)
                .sum();
            let db = 10.0 * math::log10(p);
            min = min.min(db);
            max = max.max(db);
            f *= 1.03;
        }
        eprintln!("{n} bands: summed power {min:.2} … {max:.2} dB");
        assert!(max - min < 1.5, "{n}: ripple {}", max - min);
    }
}

#[test]
fn block_size_does_not_change_a_sample() {
    let n = 30_000;
    let voice = phrase(SR, n as f32 / SR, 0.5);
    let (sl, sr) = noise(n, 9, 0.2);
    let mut notes = chord(&[48, 55, 60, 64]);
    notes.push(NoteEvent {
        frame: 11_111,
        kind: NoteKind::Off { key: 55 },
    });
    notes.push(NoteEvent {
        frame: 17_003,
        kind: NoteKind::On {
            key: 67,
            velocity: 1.0,
        },
    });
    notes.push(NoteEvent {
        frame: 25_000,
        kind: NoteKind::AllOff,
    });
    let mut reference: Option<(Vec<u32>, Vec<u32>)> = None;
    for &block in &[1usize, 37, 64, 512] {
        let mut q = Loquax::new(SR);
        q.set(&busy());
        q.reset();
        // Targets moved after the reset: the glides and the retuning run during the render.
        let mut s = busy();
        s.formant = -5.0;
        s.band_shift = -1.25;
        s.lo_hz = 90.0;
        s.tilt = -2.0;
        s.spread = 1.0;
        s.band_db[7] = -12.0;
        q.set(&s);
        let (mut l, mut r) = (voice.clone(), voice.clone());
        render(&mut q, &mut l, &mut r, Some((&sl, &sr)), &notes, block);
        let got = (bits(&l), bits(&r));
        match &reference {
            None => reference = Some(got),
            Some(want) => assert!(*want == got, "block {block} differs"),
        }
    }
}

#[test]
fn it_comes_to_rest_and_its_rest_is_a_new_unit() {
    let s = busy();
    let voice = phrase(SR, 1.0, 0.5);
    let mut q = Loquax::new(SR);
    q.set(&s);
    q.reset();
    let (mut l, mut r) = (voice.clone(), voice.clone());
    let mut notes = chord(&[48, 52]);
    notes.push(NoteEvent {
        frame: 30_000,
        kind: NoteKind::AllOff,
    });
    render(&mut q, &mut l, &mut r, None, &notes, 128);
    assert!(!q.at_rest());
    let mut secs = 0.0f32;
    let block = 480;
    while !q.at_rest() {
        let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
        q.process(&mut l, &mut r, None, &[]);
        secs += block as f32 / SR;
        assert!(secs < 15.0, "not at rest: {:?}", q.rest_blocker());
    }
    eprintln!("at rest {secs:.2} s into the silence");
    q.skip(48_000);
    // The twin: a new unit with the same settings.
    let mut twin = Loquax::new(SR);
    twin.set(&s);
    twin.reset();
    let next = phrase(SR, 0.5, 0.4);
    let notes = chord(&[57]);
    let (mut a, mut b) = (next.clone(), next.clone());
    render(&mut q, &mut a, &mut b, None, &notes, 128);
    let (mut c, mut d) = (next.clone(), next.clone());
    render(&mut twin, &mut c, &mut d, None, &notes, 128);
    assert_eq!(bits(&a), bits(&c));
    assert_eq!(bits(&b), bits(&d));
}

#[test]
fn a_held_key_keeps_it_from_rest_and_starts_when_engaged() {
    let mut q = Loquax::new(SR);
    let (mut l, mut r) = (vec![0.0f32; 64], vec![0.0f32; 64]);
    q.process(&mut l, &mut r, None, &chord(&[60]));
    assert_eq!(q.rest_blocker(), Some("a key is held"));
    let mut s = LoquaxSettings {
        mix: 1.0,
        ..LoquaxSettings::default()
    };
    s.synth.noise = 0.0;
    q.set(&s);
    // The voice says "aa": the held key's carrier is heard.
    let voice = crate::pitch::testkit::voice(
        SR,
        0.5,
        crate::pitch::testkit::Vowel::A,
        0.5,
        |_| 120.0,
    );
    let (mut l, mut r) = (voice.clone(), voice.clone());
    render(&mut q, &mut l, &mut r, None, &[], 256);
    assert_eq!(q.tel[t::VOICES], 1.0);
    let tail = &l[12_000..];
    assert!(measure::rms_db(tail) > -40.0, "{}", measure::rms_db(tail));
}

#[test]
fn a_value_that_is_not_a_number_is_silence() {
    let mut q = Loquax::new(SR);
    q.set(&busy());
    let mut l = vec![0.1f32; 512];
    l[10] = f32::NAN;
    l[20] = f32::INFINITY;
    let mut r = l.clone();
    let side = (l.clone(), r.clone());
    q.process(&mut l, &mut r, Some((&side.0, &side.1)), &chord(&[60]));
    assert!(l[21..].iter().chain(&r[21..]).all(|x| x.is_finite()));
}

#[test]
fn a_new_band_count_relayouts_and_the_view_names_every_band() {
    let mut q = Loquax::new(SR);
    for &n in &[8usize, 16, 33, 64, 100] {
        let s = LoquaxSettings {
            mix: 1.0,
            bands: n,
            ..LoquaxSettings::default()
        };
        q.set(&s);
        let mut v = BandView::default();
        q.bands(&mut v);
        let want = n.clamp(8, 64);
        assert_eq!(v.bands, want);
        assert!((v.analysis_hz[0] - 100.0).abs() < 1.0, "{}", v.analysis_hz[0]);
        let top = v.analysis_hz[want - 1];
        assert!((top - 8_000.0).abs() < 1.0, "{top}");
        assert!(v.analysis_hz[..want].windows(2).all(|w| w[1] > w[0]));
    }
}

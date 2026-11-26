//! r42 fiddle — one bow on two strings, the open strings in sympathy, the grit and the steel
//! set (ADR 0164): their null (every one at its default — the golden renders and
//! `LOWER_HASHES` hold them), the same bits at every block size, rest, a double stop as one
//! voice, a crossing's ring and the sympathetic ring.

use super::voice::NoteTech;
use crate::math;
use super::*;

const SR: f32 = 48_000.0;

/// The fiddle's settings: the violin, one bow on two strings, the sympathy, the grit, the
/// steel set and the just double stops all on.
pub(crate) fn fiddle() -> ArcusSettings {
    ArcusSettings {
        instrument: Instrument::Violin,
        mode: PlayMode::Legato,
        pair: 1.0,
        sym: 1.0,
        grit: 1.0,
        steel: 1.0,
        just: 0.6,
        ..ArcusSettings::default()
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum Ev {
    On(u8, f32, u64),
    Off(u8, u64),
    How(u64, Art, BowDir),
}

/// A fiddle line: a double stop struck from silence (A4 open + F#5), a fast run crossing
/// strings, a drone (the open D under notes on the A string), a slur across strings, the
/// last note let go and rung out.
pub(crate) fn line() -> Vec<(f64, Ev)> {
    let mut e = Vec::new();
    let mut id = 100u64;
    let mut note = |e: &mut Vec<(f64, Ev)>, t: f64, len: f64, k: u8, v: f32, art: Art, dir| {
        id += 1;
        e.push((t, Ev::How(id, art, dir)));
        e.push((t, Ev::On(k, v, id)));
        e.push((t + len, Ev::Off(k, id)));
    };
    use BowDir::{Down, Up};
    // The double stop.
    note(&mut e, 0.0, 0.6, 69, 0.6, Art::Sustain, Down);
    note(&mut e, 0.0, 0.6, 78, 0.6, Art::Sustain, Down);
    // The run: E5 B4 A4 E5 C#5 E5 A5 E5 (eighths at 160 BPM, each its own bow).
    let mut t = 0.65;
    let mut dir = Up;
    for k in [76u8, 71, 69, 76, 73, 76, 81, 76] {
        note(&mut e, t, 0.18, k, 0.7, Art::Sustain, dir);
        dir = if dir == Up { Down } else { Up };
        t += 0.1875;
    }
    // The drone: the open D held under A-string notes.
    note(&mut e, t, 1.2, 62, 0.6, Art::Sustain, Down);
    for (i, k) in [69u8, 71, 73, 74].iter().enumerate() {
        let art = if i == 0 { Art::Sustain } else { Art::Legato };
        note(&mut e, t + 0.3 * i as f64, 0.3, *k, 0.7, art, Down);
    }
    t += 1.3;
    // A slur across strings: D5 (A string) to G5 (E string), let go and rung out.
    note(&mut e, t, 0.4, 74, 0.6, Art::Sustain, Up);
    note(&mut e, t + 0.4, 0.5, 79, 0.6, Art::Legato, Up);
    e.sort_by(|a, b| a.0.total_cmp(&b.0));
    e
}

/// Render `score` for `secs` in blocks of `bs` (split at each event), with `s`.
pub(crate) fn render(
    s: &ArcusSettings,
    score: &[(f64, Ev)],
    secs: f64,
    bs: usize,
) -> (Arcus, Vec<f32>) {
    let mut a = Arcus::new(SR);
    a.set(s);
    let frames = (secs * SR as f64) as usize;
    let ev: Vec<(usize, Ev)> = score
        .iter()
        .map(|(t, e)| ((t * SR as f64).round() as usize, *e))
        .collect();
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    let (mut at, mut i) = (0usize, 0usize);
    while at < frames {
        while i < ev.len() && ev[i].0 <= at {
            match ev[i].1 {
                Ev::On(k, v, id) => a.note_on(k, v, id),
                Ev::Off(k, id) => a.note_off(k, id),
                Ev::How(id, art, dir) => a.note_art(id, Some(art), Some(dir)),
            }
            i += 1;
        }
        let next = ev.get(i).map_or(frames, |e| e.0.min(frames));
        let n = bs.min(next.max(at + 1) - at).min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (a, l)
}

fn out(s: &ArcusSettings, bs: usize) -> Vec<f32> {
    render(s, &line(), 5.5, bs).1
}

#[test]
fn the_fiddle_renders_the_same_bits_at_every_block_size() {
    let s = fiddle();
    let a = out(&s, 256);
    assert!(a.iter().any(|x| *x != 0.0));
    for bs in [1usize, 31, 64, 1000] {
        let b = out(&s, bs);
        let diff = a.iter().zip(&b).position(|(x, y)| x.to_bits() != y.to_bits());
        assert_eq!(diff, None, "block size {bs}: the first sample that differs");
    }
}

#[test]
fn the_fiddle_comes_to_rest() {
    let s = fiddle();
    let (a, l) = render(&s, &line(), 16.0, 256);
    assert!(a.at_rest(), "every voice, the sympathetic strings and the body still");
    assert!(l[l.len() - 4800..].iter().all(|x| *x == 0.0));
}

#[test]
fn a_double_stop_is_one_voice_under_one_bow() {
    let s = fiddle();
    let mut a = Arcus::new(SR);
    a.set(&s);
    a.note_art(1, Some(Art::Sustain), Some(BowDir::Down));
    a.note_on(69, 0.6, 1);
    a.note_on(78, 0.6, 2);
    let (mut l, mut r) = (vec![0.0f32; 4800], vec![0.0f32; 4800]);
    a.process(&mut l, &mut r);
    assert_eq!(a.active_voices(), 1, "the two notes are one voice");
    let v = a.voices.iter().find(|v| v.active()).unwrap();
    assert_eq!(v.technique(), NoteTech::Arco);
    let (f1, f2) = (v.f0(), v.second_f0().expect("a second string"));
    let a4 = 440.0;
    let fs5 = 440.0 * math::exp2(9.0f64 / 12.0);
    // A4 open; F#5 stopped, tuned 0.6 of the way to the major sixth's 5/3 (−15.6 cents).
    assert!((f1 - a4).abs() / a4 < 1e-3, "the open A: {f1}");
    let ct = 1200.0 * math::log2(f2 / fs5);
    assert!((ct + 0.6 * 15.64).abs() < 1.5, "F#5 {ct:.2} cents from tempered");
    // Its key let go: the bow leans off it; the voice plays on.
    a.note_off(78, 2);
    a.process(&mut l, &mut r);
    assert_eq!(a.active_voices(), 1);
    // The other let go: both ring out, and the instance rests.
    a.note_off(69, 1);
    for _ in 0..100 {
        a.process(&mut l, &mut r);
    }
    assert!(a.at_rest());
}

#[test]
fn without_a_pair_two_notes_at_once_slur_as_before() {
    let s = ArcusSettings {
        pair: 0.0,
        ..fiddle()
    };
    let mut a = Arcus::new(SR);
    a.set(&s);
    a.note_on(69, 0.6, 1);
    a.note_on(78, 0.6, 2);
    let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
    a.process(&mut l, &mut r);
    let v = a.voices.iter().find(|v| v.active()).unwrap();
    assert_eq!(v.second_f0(), None);
}

#[test]
fn a_string_the_bow_leaves_rings_on() {
    let s = fiddle();
    let mut a = Arcus::new(SR);
    a.set(&s);
    let (mut l, mut r) = (vec![0.0f32; 9600], vec![0.0f32; 9600]);
    a.note_on(76, 0.7, 1); // the open E
    a.process(&mut l, &mut r);
    a.note_off(76, 1);
    a.note_on(71, 0.7, 2); // B4 on the A string, re-bowed
    let (mut l2, mut r2) = (vec![0.0f32; 480], vec![0.0f32; 480]);
    a.process(&mut l2, &mut r2);
    let v = a.voices.iter().find(|v| v.active()).unwrap();
    assert_eq!(v.string_index(), 2, "B4 on the A string");
    assert!(
        v.second_f0().is_some_and(|f| (f - 659.26).abs() < 2.0),
        "the E string rings under it"
    );
}

#[test]
fn the_open_strings_ring_in_sympathy_after_the_bow_lifts() {
    // D5 on the A string: the open D's second partial and the open G's third.
    let tail = |sym: f32| {
        let s = ArcusSettings { sym, ..fiddle() };
        let mut a = Arcus::new(SR);
        a.set(&s);
        a.note_on(74, 0.7, 1);
        let mut l = vec![0.0f32; 48_000];
        let mut r = vec![0.0f32; 48_000];
        a.process(&mut l, &mut r);
        a.note_off(74, 1);
        let mut l = vec![0.0f32; 72_000];
        let mut r = vec![0.0f32; 72_000];
        a.process(&mut l, &mut r);
        // 0.6–1.4 s after the key.
        l[28_800..67_200].iter().map(|x| (x * x) as f64).sum::<f64>()
    };
    let (dry, rung) = (tail(0.0), tail(1.0));
    let db = 10.0 * (rung / dry.max(1e-30)).log10();
    assert!(db > 6.0, "the ring after the bow lifts: {db:.1} dB over none");
}

/// The double stop's level against its two notes alone (`--ignored --nocapture`).
#[test]
#[ignore = "probe: prints the levels"]
fn probe_double_stop_levels() {
    let lvl = |notes: &[u8], tilt: f32, pair: f32| {
        let s = ArcusSettings {
            tilt,
            pair,
            sym: 0.0,
            ..fiddle()
        };
        let mut a = Arcus::new(SR);
        a.set(&s);
        for (i, k) in notes.iter().enumerate() {
            a.note_art(i as u64 + 1, Some(Art::Sustain), Some(BowDir::Down));
            a.note_on(*k, 0.7, i as u64 + 1);
        }
        let mut l = vec![0.0f32; 48_000];
        let mut r = vec![0.0f32; 48_000];
        a.process(&mut l, &mut r);
        let e: f64 = l[19_200..48_000].iter().map(|x| (x * x) as f64).sum::<f64>() / 28_800.0;
        10.0 * e.max(1e-30).log10()
    };
    for (lo, hi) in [(69u8, 78u8), (69, 76), (62, 69), (69, 73), (64, 73)] {
        println!(
            "{lo}+{hi}: lo alone {:.1}, hi alone {:.1}, both {:.1}, tilt 1 (lo only) {:.1}, tilt 0 (hi only) {:.1}",
            lvl(&[lo], 0.5, 1.0),
            lvl(&[hi], 0.5, 1.0),
            lvl(&[hi, lo], 0.5, 1.0),
            lvl(&[hi, lo], 1.0, 1.0),
            lvl(&[hi, lo], 0.0, 1.0)
        );
    }
}

/// The energy of `x` in `lo … hi` Hz over the whole slice (a plain DFT on a 1 Hz-ish grid
/// would be slow: a band by first differences is enough for a ratio), as a share of all.
fn high_share(x: &[f32]) -> f64 {
    // A first difference tilts +6 dB an octave: its energy over the signal's is the
    // spectrum's weight above ~sr/(2π).
    let d: f64 = x.windows(2).map(|w| ((w[1] - w[0]) as f64).powi(2)).sum();
    let e: f64 = x.iter().map(|v| (*v as f64).powi(2)).sum();
    d / e.max(1e-30)
}

#[test]
fn a_chop_scratches_where_a_staccato_speaks() {
    let stroke = |art: Art| {
        let mut a = Arcus::new(SR);
        a.set(&fiddle());
        a.note_art(1, Some(art), Some(BowDir::Down));
        a.note_on(62, 1.0, 1);
        let (mut l, mut r) = (vec![0.0f32; 2880], vec![0.0f32; 2880]);
        a.process(&mut l, &mut r);
        a.note_off(62, 1);
        // the stroke's 60 ms
        high_share(&l)
    };
    let (chop, stac) = (stroke(Art::Chop), stroke(Art::Staccato));
    let db = 10.0 * (chop / stac).log10();
    assert!(db > 3.0, "the chop's top over the staccato's: {db:.1} dB");
}

#[test]
fn a_cross_tuning_moves_the_open_strings() {
    use super::upper::tables::CROSS;
    for (c, d) in CROSS.iter().enumerate() {
        let s = ArcusSettings {
            cross: c as u8,
            ..fiddle()
        };
        let spec = Instrument::Violin.spec_crossed(c as u8);
        for i in 0..4 {
            let key = (Instrument::Violin.spec().strings[i].open as i16 + d[i] as i16) as u8;
            assert_eq!(spec.strings[i].open, key);
            let mut a = Arcus::new(SR);
            a.set(&s);
            a.note_on(key, 0.6, 1);
            let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
            a.process(&mut l, &mut r);
            let v = a.voices.iter().find(|v| v.active()).unwrap();
            assert_eq!(v.string_index(), i, "tuning {c}: key {key} on its string");
            assert!(v.is_open(), "tuning {c}: key {key} open");
        }
    }
    // Standard is the violin's own table.
    assert!(std::ptr::eq(
        Instrument::Violin.spec_crossed(0),
        Instrument::Violin.spec()
    ));
}

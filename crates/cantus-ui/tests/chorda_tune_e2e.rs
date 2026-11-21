//! Chorda's tuning end to end (round 40, the technician's pass, `DSP_VERSION` 10, ADR 0098's
//! fourth amendment): a Chorda track added the way the instrument picker adds one, on the
//! boot path, plays the keys the owner heard out of tune (Bb2, B2, C3) and the worst of
//! candidate 16 (B0, 18 cents sharp) on the curve its keyboard is designed to — measured on
//! the real engine's offline render, each partial against the smooth stretch (the keyboard
//! without its technician's table). Its own test binary: it points `CANTUS_DATA_DIR` at a
//! temporary folder.

use cantus_core::*;
use cantus_dsp::chorda::tables;
use cantus_dsp::chorda::voicing::Voicing;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use std::f64::consts::TAU;
use std::sync::Arc;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// Partial `hz`'s power-weighted frequency (Hz) over `x`'s samples `[a, b)`: the signal
/// heterodyned to `hz`, averaged over frames `len` long every `hop`, the phase advance of
/// the lag-one product (a beating pair's mean, as the ear's), and its mean power.
fn partial(x: &[f64], fs: f64, hz: f64, a: usize, b: usize, len: usize) -> (f64, f64) {
    let hop = len / 4;
    let w = TAU * hz / fs;
    let frame = |s: usize| {
        let (mut re, mut im) = (0.0, 0.0);
        for i in 0..len {
            let hann = 0.5 - 0.5 * (TAU * (i as f64 + 0.5) / len as f64).cos();
            let ph = w * (s + i) as f64;
            re += x[s + i] * hann * ph.cos();
            im -= x[s + i] * hann * ph.sin();
        }
        (re, im)
    };
    let (mut ar, mut ai, mut p) = (0.0, 0.0, 0.0);
    let mut s = a;
    let mut prev = frame(s);
    while s + hop + len <= b {
        let next = frame(s + hop);
        ar += next.0 * prev.0 + next.1 * prev.1;
        ai += next.1 * prev.0 - next.0 * prev.1;
        p += prev.0 * prev.0 + prev.1 * prev.1;
        prev = next;
        s += hop;
    }
    (hz + ai.atan2(ar) * fs / (TAU * hop as f64), p)
}

#[test]
fn a_new_chorda_plays_the_keys_the_owner_heard_on_their_curve() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    // The keyboard a new Chorda plays carries the technician's table.
    let reference = cantus_engine::inst::chorda::reference();
    assert_eq!(reference.tune, Voicing::default().tune);
    assert_eq!(reference.tune, cantus_dsp::chorda::voicing::unison::TUNE);
    cantus_engine::inst::chorda::ensure(reference, 48_000.0);

    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let track = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the new Chorda track");
    let keys = [46u8, 47, 48, 23];
    let mut clip = app.bus.new_clip("Keys", Ticks::ZERO, Ticks::beats(32));
    for (i, k) in keys.iter().enumerate() {
        let n = app
            .bus
            .new_note(Ticks::beats(8 * i as i64), Ticks::beats(6), *k, 75);
        clip.notes.push(n);
    }
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    }));
    let others: Vec<TrackId> = app
        .project()
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|t| *t != track)
        .collect();
    for id in others {
        assert!(app.exec(Command::RemoveTrack { id }));
    }
    let p = app.project().clone();
    assert!(p.tempo_points.is_empty(), "one tempo");
    let beat = 60.0 / p.tempo_bpm;
    let r = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .unwrap();
    let fs = 48_000.0;
    let mono: Vec<f64> = r
        .samples
        .chunks(2)
        .map(|c| c[0] as f64 + c[1] as f64)
        .collect();

    // The curve: the keyboard's keys without the table.
    let mut curve = reference;
    curve.tune = [0.0; tables::KEYS];
    let curve = tables::keyboard_for(&curve);
    for (i, k) in keys.iter().enumerate() {
        let key = curve[(*k - tables::LOWEST) as usize];
        let at = (8.0 * i as f64 * beat * fs) as usize;
        let (a, b) = (at + (0.1 * fs) as usize, at + (1.1 * fs) as usize);
        let len = ((4.0 * fs / key.f0()) as usize).max(64);
        // The ear's pitch: partials 1–8 below 5 kHz by their level and the dominance region.
        let (mut sw, mut sc) = (0.0, 0.0);
        let mut line = format!("key {k}:");
        for n in 1..=8u32 {
            let want = key.partial(n);
            if want > 5_000.0 {
                break;
            }
            let (got, pw) = partial(&mono, fs, want, a, b, len);
            let ct = 1200.0 * (got / want).log2();
            let o = (want / 700.0).log2() / 1.5;
            let w = pw.sqrt() * (0.1 + 0.9 * (-0.5 * o * o).exp());
            sw += w;
            sc += w * ct;
            line += &format!(" p{n} {ct:+.1}");
        }
        let pitch = sc / sw;
        eprintln!("{line}; pitch {pitch:+.2} cents");
        assert!(
            pitch.abs() <= 1.5,
            "key {k} sounds {pitch:+.2} cents off its curve through the app"
        );
    }
}

/// A partial's beat as the level shows it (dB): the 3–97 % swing of its power, frames `len`
/// long every 10 ms over `[a, b)`, about a least-squares quadratic in time — a beat faster
/// than the window's few seconds swings it, a slow change of level (the two stages of a
/// unison's decay) the quadratic takes.
fn swing(x: &[f64], fs: f64, hz: f64, a: usize, b: usize, len: usize) -> f64 {
    let hop = (0.01 * fs) as usize;
    let w = TAU * hz / fs;
    let mut pts = Vec::new();
    let mut s = a;
    while s + len <= b {
        let (mut re, mut im) = (0.0, 0.0);
        for i in 0..len {
            let hann = 0.5 - 0.5 * (TAU * (i as f64 + 0.5) / len as f64).cos();
            let ph = w * (s + i) as f64;
            re += x[s + i] * hann * ph.cos();
            im -= x[s + i] * hann * ph.sin();
        }
        pts.push((
            (s - a) as f64 / fs,
            10.0 * (re * re + im * im + 1e-30).log10(),
        ));
        s += hop;
    }
    let n = pts.len() as f64;
    let tm = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let mut m = [[0.0f64; 3]; 3];
    let mut y = [0.0f64; 3];
    for (t, d) in &pts {
        let u = [1.0, t - tm, (t - tm) * (t - tm)];
        for r in 0..3 {
            for c in 0..3 {
                m[r][c] += u[r] * u[c];
            }
            y[r] += u[r] * d;
        }
    }
    // Gauss–Jordan (3 × 3).
    for c in 0..3 {
        for r in 0..3 {
            if r != c {
                let f = m[r][c] / m[c][c];
                for k in 0..3 {
                    m[r][k] -= f * m[c][k];
                }
                y[r] -= f * y[c];
            }
        }
    }
    let k = [y[0] / m[0][0], y[1] / m[1][1], y[2] / m[2][2]];
    let mut res: Vec<f64> = pts
        .iter()
        .map(|(t, d)| d - (k[0] + k[1] * (t - tm) + k[2] * (t - tm) * (t - tm)))
        .collect();
    res.sort_by(f64::total_cmp);
    let q = |f: f64| res[((res.len() - 1) as f64 * f).round() as usize];
    q(0.97) - q(0.03)
}

/// The deepest swing of partials 1–3 of each of `keys`, struck one every `gap` seconds in
/// `mono` (48 kHz), over 0.3–3.3 s of each.
fn swings(mono: &[f64], keys: &[u8], gap: f64, curve: &[tables::Key]) -> Vec<f64> {
    let fs = 48_000.0;
    keys.iter()
        .enumerate()
        .map(|(i, k)| {
            let key = curve[(*k - tables::LOWEST) as usize];
            let at = (gap * i as f64 * fs) as usize;
            let (a, b) = (at + (0.3 * fs) as usize, at + (3.3 * fs) as usize);
            let len = ((4.0 * fs / key.f0()) as usize).max(64);
            (1..=3u32)
                .map(|n| swing(mono, fs, key.partial(n), a, b, len))
                .fold(0.0f64, f64::max)
        })
        .collect()
}

/// The six keys the owner named (beta.15, 2026-11-20), through the app as a new Chorda plays
/// them, swing on partials 1–3 no more than this (dB); on `DSP_VERSION` 10's voicing (the
/// technician's pass, rendered here by the DSP itself) they swung more — C#3's and D3's third
/// partials beat 5.7 and 3.9 Hz, E4's 5.1.
const SWING_DB: f64 = 3.0;

#[test]
fn a_new_chorda_plays_the_six_keys_the_owner_named_without_a_beat() {
    use cantus_dsp::chorda::design::{Keyboard, Tuning};
    use cantus_dsp::chorda::{Chorda, ChordaSettings};
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    let reference = cantus_engine::inst::chorda::reference();
    cantus_engine::inst::chorda::ensure(reference, 48_000.0);
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let track = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the new Chorda track");
    // E4, C#3, D3, C3, B2, E5 — one every 4 s or so, at the lab's mezzo-forte.
    let keys = [64u8, 49, 50, 48, 47, 76];
    let beat = 60.0 / app.project().tempo_bpm;
    let gap_beats = (4.0 / beat).ceil() as i64;
    let gap = gap_beats as f64 * beat;
    let mut clip = app.bus.new_clip(
        "Six",
        Ticks::ZERO,
        Ticks::beats(gap_beats * keys.len() as i64),
    );
    for (i, k) in keys.iter().enumerate() {
        let n = app.bus.new_note(
            Ticks::beats(gap_beats * i as i64),
            Ticks::beats(gap_beats - 1),
            *k,
            75,
        );
        clip.notes.push(n);
    }
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    }));
    let others: Vec<TrackId> = app
        .project()
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|t| *t != track)
        .collect();
    for id in others {
        assert!(app.exec(Command::RemoveTrack { id }));
    }
    let p = app.project().clone();
    assert!(p.tempo_points.is_empty(), "one tempo");
    let r = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .unwrap();
    let mono: Vec<f64> = r
        .samples
        .chunks(2)
        .map(|c| c[0] as f64 + c[1] as f64)
        .collect();
    let mut curve_t = reference;
    curve_t.tune = [0.0; tables::KEYS];
    let curve = tables::keyboard_for(&curve_t);
    let now = swings(&mono, &keys, gap, &curve);

    // The same keys on `DSP_VERSION` 10's voicing, by the DSP itself.
    let was_v = Voicing::technician();
    let kb = Arc::new(Keyboard::compute(
        was_v.tuning(Tuning::default()),
        48_000.0,
        8,
    ));
    let mut c = Chorda::new(48_000.0);
    c.set_keyboard(kb);
    c.set(
        &ChordaSettings {
            model: 2,
            ..ChordaSettings::default()
        },
        0.0,
    );
    c.set_voicing(&was_v);
    let fs = 48_000.0;
    let total = (gap * keys.len() as f64 * fs) as usize;
    let (mut l, mut rr) = (vec![0.0f32; total], vec![0.0f32; total]);
    let mut at = 0;
    for (i, k) in keys.iter().enumerate() {
        let on = (gap * i as f64 * fs) as usize;
        while at < on {
            let end = (at + 512).min(on);
            c.process(&mut l[at..end], &mut rr[at..end]);
            at = end;
        }
        c.note_on(*k, 75.0 / 127.0, i as u64 + 1);
    }
    while at < total {
        let end = (at + 512).min(total);
        c.process(&mut l[at..end], &mut rr[at..end]);
        at = end;
    }
    let was_mono: Vec<f64> = l.iter().zip(&rr).map(|(a, b)| (*a + *b) as f64).collect();
    let was = swings(&was_mono, &keys, gap, &curve);
    for ((k, n), w) in keys.iter().zip(&now).zip(&was) {
        eprintln!(
            "key {k}: swing on partials 1-3 {n:.2} dB through the app (DSP_VERSION 10: {w:.2} dB)"
        );
        assert!(
            *n <= SWING_DB,
            "key {k} swings {n:.2} dB through the app: a beat"
        );
    }
    let worse = was.iter().zip(&now).filter(|(w, n)| *w > *n).count();
    assert!(
        worse >= 5,
        "DSP_VERSION 10 swung more on most of the six: {was:?} against {now:?}"
    );
}

//! Warping's bench (round 29, ADR 0061): the stretcher measured — transient sharpness and
//! pre-echo, marker timing, pitch accuracy of shifted tones, phasiness, CPU per second of
//! audio per mode — and the tempo detector's accuracy over a corpus; and the A/B listening
//! set (`cantus-bench listen <dir>`): each mode × ratios 0.5, 0.8, 1.25, 2 × material.

use crate::report::{Better, Metric};
use crate::stats::Summary;
use crate::Plan;
use cantus_dsp::beat::testkit::{click_track, drum_loop};
use cantus_dsp::math;
use cantus_dsp::pitch::testkit::{self as pk, Noise, Vowel};
use cantus_dsp::stretch::{self, measure as m, Map, Mode, Request, Tail};
use std::path::Path;
use std::time::Instant;

pub const SR: f32 = 48_000.0;
/// The ratios of the listening set and of the measurements (output length / input).
pub const RATIOS: [f64; 4] = [0.5, 0.8, 1.25, 2.0];

/// One piece of material: its name, its channels, its transients (source frames).
pub struct Material {
    pub name: &'static str,
    pub channels: Vec<Vec<f32>>,
    pub onsets: Vec<f64>,
}

fn stereo(x: Vec<f32>) -> Vec<Vec<f32>> {
    vec![x.clone(), x]
}

/// Drums: a swung loop built from the testkit's kick, snare and hats, 100 BPM, 4 bars.
pub fn drums() -> Material {
    let d = drum_loop(SR, 100.0, 4, 0.3, 0.05, 11);
    Material {
        name: "drums",
        channels: stereo(d.audio),
        onsets: d.onsets,
    }
}

/// A full mix: one of the demo songs rendered offline (its first 8 seconds, stereo).
pub fn mix() -> Material {
    let songs = crate::paced::songs();
    let (_, p) = songs.first().expect("a demo song");
    let mut s = cantus_core::Snapshot::compile(p, 1);
    let mut cache = cantus_engine::SampleCache::new();
    let _ = cache.attach(&mut s);
    let _ = cache.attach_audio(&mut s, SR as u32);
    let opts = cantus_engine::RenderOptions {
        sample_rate: SR as u32,
        tail_seconds: 0.0,
        block_frames: 256,
        end: Some(cantus_core::Ticks::from_seconds(8.0, p.tempo_bpm)),
        ..Default::default()
    };
    let r = cantus_engine::render(std::sync::Arc::new(s), &opts).expect("the song renders");
    let (l, rr): (Vec<f32>, Vec<f32>) = r.samples.chunks_exact(2).map(|f| (f[0], f[1])).unzip();
    let onsets = cantus_dsp::onset::detect(&l, SR).frames;
    Material {
        name: "mix",
        channels: vec![l, rr],
        onsets,
    }
}

/// A sung phrase: the testkit's voice, a melody with vibrato.
pub fn voice() -> Material {
    let notes = [
        (0.0, 57.0),
        (0.6, 59.0),
        (1.2, 60.0),
        (2.0, 64.0),
        (2.8, 62.0),
    ];
    let f0 = move |t: f64| {
        let n = notes
            .iter()
            .rev()
            .find(|(s, _)| t >= *s)
            .map_or(57.0, |n| n.1);
        let vib = 30.0 * math::sin(2.0 * std::f64::consts::PI * 5.5 * t) as f32;
        cantus_dsp::pitch::midi_to_hz(n + vib / 100.0)
    };
    let x = pk::voice(SR, 3.6, Vowel::A, 0.5, f0);
    let onsets = cantus_dsp::onset::detect(&x, SR).frames;
    Material {
        name: "voice",
        channels: stereo(x),
        onsets,
    }
}

/// A solo instrument: a flute-like line (breathy, harmonics falling off, legato).
pub fn solo() -> Material {
    let notes = [
        (0.0, 72.0),
        (0.5, 74.0),
        (1.0, 76.0),
        (1.5, 79.0),
        (2.25, 77.0),
        (3.0, 76.0),
    ];
    let mut noise = Noise::new(5);
    let mut phase = 0.0f64;
    let n = (3.6 * SR) as usize;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let m0 = notes
                .iter()
                .rev()
                .find(|(s, _)| t >= *s)
                .map_or(72.0, |n| n.1);
            let f = cantus_dsp::pitch::midi_to_hz(m0) as f64
                * (1.0 + 0.004 * math::sin(2.0 * std::f64::consts::PI * 5.0 * t));
            phase += 2.0 * std::f64::consts::PI * f / SR as f64;
            let env = (t / 0.04).min(1.0) * ((3.6 - t) / 0.1).min(1.0);
            let tone =
                math::sin(phase) + 0.3 * math::sin(2.0 * phase) + 0.1 * math::sin(3.0 * phase);
            (0.35 * env * tone) as f32 + 0.01 * noise.next()
        })
        .collect();
    let onsets = cantus_dsp::onset::detect(&x, SR).frames;
    Material {
        name: "solo",
        channels: stereo(x),
        onsets,
    }
}

/// A chordal pad: detuned saws of a I–vi–IV–V progression, a slow attack, spread wide.
pub fn pad() -> Material {
    let chords = [
        [48, 52, 55, 60],
        [45, 52, 57, 60],
        [41, 48, 53, 57],
        [43, 50, 55, 59],
    ];
    let n = (4.0 * SR) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for (c, chord) in chords.iter().enumerate() {
        let from = c * n / 4;
        for (v, &note) in chord.iter().enumerate() {
            for (d, detune) in [-0.08f64, 0.08].iter().enumerate() {
                let f = cantus_dsp::pitch::midi_to_hz(note as f32) as f64
                    * (1.0 + detune / 100.0 * 12.0);
                let mut ph = (v * 7 + d * 3) as f64 * 0.37;
                for i in from..from + n / 4 {
                    let t = (i - from) as f64 / SR as f64;
                    ph += f / SR as f64;
                    let saw = 2.0 * (ph - ph.floor()) - 1.0;
                    let env = (t / 0.3).min(1.0) * ((1.0 - t) / 0.05).clamp(0.0, 1.0);
                    let s = (0.05 * env * saw) as f32;
                    if d == 0 {
                        l[i] += s;
                    } else {
                        r[i] += s;
                    }
                }
            }
        }
    }
    Material {
        name: "pad",
        channels: vec![l, r],
        onsets: Vec::new(),
    }
}

/// Every material of the set.
pub fn set() -> Vec<Material> {
    vec![drums(), mix(), voice(), solo(), pad()]
}

/// Render `mat` stretched to `ratio` times its length in `mode`, transposed `semis`.
pub fn render(mat: &Material, mode: Mode, ratio: f64, semis: f64) -> Vec<Vec<f32>> {
    let len_in = mat.channels[0].len();
    let len = (len_in as f64 * ratio).round() as usize;
    let map = Map::new(vec![(0.0, 0.0), (len as f64, len_in as f64)]).expect("a map");
    let refs: Vec<&[f32]> = mat.channels.iter().map(Vec::as_slice).collect();
    let pitch = (mode == Mode::Melodic).then(|| cantus_dsp::pitch::analyse(refs[0], SR));
    stretch::render(
        &refs,
        SR,
        &Request {
            mode,
            map: &map,
            len,
            semitones: semis,
            formants: true,
            onsets: &mat.onsets,
            pitch: pitch.as_ref(),
            tail: Tail::Fade,
            origin: 0,
        },
    )
}

/// A hit every half second (the transient measurements): a 0.2 ms rise into noise and a
/// thump, nothing ringing from one into the next.
fn hits() -> Material {
    let n = (6.0 * SR) as usize;
    let mut x = vec![0.0f32; n];
    let mut g = Noise::new(7);
    let mut at = Vec::new();
    let mut s = (0.25 * SR) as usize;
    while s + ((0.2 * SR) as usize) < n {
        at.push(s as f64);
        for k in 0..(0.15 * SR) as usize {
            let t = k as f32 / SR;
            let env = math::exp(-t / 0.025) * (k as f32 / (0.0002 * SR)).min(1.0);
            let thump = math::sin(2.0 * std::f32::consts::PI * 70.0 * t) * math::exp(-t / 0.05);
            x[s + k] += 0.6 * env * g.next() + 0.4 * thump * (k as f32 / 10.0).min(1.0);
        }
        s += (0.5 * SR) as usize;
    }
    Material {
        name: "hits",
        channels: stereo(x),
        onsets: at,
    }
}

fn metric(id: &str, unit: &str, better: Better, v: &[f64], note: &str) -> Metric {
    Metric::new(id, unit, better, Summary::of(v))
        .ungated()
        .note(note)
}

/// The suite's metrics.
pub fn suite(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    // Transients: attack (frames, 10–90 %), pre-echo (dB), timing (frames) per mode and
    // ratio.
    let h = hits();
    for mode in [Mode::Complex, Mode::Rhythm] {
        for ratio in RATIOS {
            let y = render(&h, mode, ratio, 0.0);
            let (mut att, mut pre, mut err) = (Vec::new(), Vec::new(), Vec::new());
            for &s in &h.onsets[1..h.onsets.len() - 1] {
                let o = (s * ratio).round() as usize;
                if let Some(a) = m::attack_frames(&y[0], SR, o.saturating_sub(240), o + 960) {
                    att.push(a as f64);
                }
                pre.push(m::pre_echo_db(&y[0], o, 1_200, 48) as f64);
                let si = s as usize;
                if let (Some(r), Some(r0)) = (
                    m::rise_at(&y[0], o.saturating_sub(400), o + 400, 0.25),
                    m::rise_at(&h.channels[0], si.saturating_sub(400), si + 400, 0.25),
                ) {
                    err.push(((r as f64 - o as f64) - (r0 as f64 - s)).abs());
                }
            }
            let id = format!("warp.{}.x{ratio}", mode_id(mode));
            out.push(metric(
                &format!("{id}.attack"),
                "frames",
                Better::Lower,
                &att,
                "10-90 % of a hit's envelope",
            ));
            out.push(metric(
                &format!("{id}.pre_echo"),
                "dB",
                Better::Lower,
                &pre,
                "the 25 ms before a hit against the hit",
            ));
            out.push(metric(
                &format!("{id}.timing"),
                "frames",
                Better::Lower,
                &err,
                "a hit's rise from where the map puts it, less the source's own",
            ));
        }
    }
    // Pitch: a harmonic tone shifted, measured by its fundamental (cents).
    let tone: Vec<f32> = (0..(3.0 * SR) as usize)
        .map(|i| {
            let w = 2.0 * std::f64::consts::PI * 220.0 * i as f64 / SR as f64;
            (0.4 * math::sin(w) + 0.2 * math::sin(2.0 * w) + 0.1 * math::sin(3.0 * w)) as f32
        })
        .collect();
    let tm = Material {
        name: "tone",
        channels: stereo(tone),
        onsets: Vec::new(),
    };
    for semis in [-12.0, -3.0, 7.0, 12.0] {
        let mut cents = Vec::new();
        for ratio in [0.8, 1.0, 1.25] {
            let y = render(&tm, Mode::Complex, ratio, semis);
            let want = 220.0 * math::exp2(semis as f32 / 12.0);
            let n = y[0].len();
            let f = pk::measured_f0(&y[0][n / 4..n * 3 / 4], SR, want);
            cents.push(m::cents(f, want).abs() as f64);
        }
        out.push(metric(
            &format!("warp.complex.pitch{semis:+}"),
            "cents",
            Better::Lower,
            &cents,
            "a 220 Hz tone shifted, stretched x0.8/1/1.25",
        ));
    }
    // Phasiness: a held chord's level wander.
    let p = pad();
    for ratio in [0.8, 1.25] {
        let y = render(&p, Mode::Complex, ratio, 0.0);
        let n = y[0].len();
        // What the stretch adds to the chord's own beating (its detuned voices wander too).
        let n0 = p.channels[0].len();
        let own = m::level_wander_db(&p.channels[0], SR, 261.6, n0 / 16, n0 / 4 - n0 / 32) as f64;
        let w = m::level_wander_db(&y[0], SR, 261.6, n / 16, n / 4 - n / 32) as f64 - own;
        out.push(metric(
            &format!("warp.complex.x{ratio}.wander"),
            "dB",
            Better::Lower,
            &[w],
            "added to a held chord's own beating: its C, peak to peak, 40 ms windows",
        ));
    }
    // CPU: milliseconds per second of audio, per mode, at x1.25 and -3 st.
    let mats = if plan.quick {
        vec![drums(), voice()]
    } else {
        vec![drums(), mix(), voice()]
    };
    for mode in Mode::ALL {
        let mut ms = Vec::new();
        for mat in &mats {
            let secs = mat.channels[0].len() as f64 / SR as f64;
            let t = Instant::now();
            let _ = render(mat, mode, 1.25, -3.0);
            ms.push(t.elapsed().as_secs_f64() * 1e3 / secs);
        }
        out.push(metric(
            &format!("warp.{}.cpu", mode_id(mode)),
            "ms/s",
            Better::Lower,
            &ms,
            "one core, stereo, x1.25 and -3 st, offline",
        ));
    }
    // The tempo detector's corpus (60–200 BPM, clicks and drums with swing).
    let (mut inside, mut ok, mut octave) = (0usize, 0usize, 0usize);
    let mut bpm = 60.0;
    while bpm <= 200.0 {
        for kind in 0..3 {
            let mat = match kind {
                0 => click_track(44_100.0, bpm, 8, 0.1),
                1 => drum_loop(44_100.0, bpm, 6, 0.0, 0.2, bpm as u64),
                _ => drum_loop(48_000.0, bpm, 6, 0.6, 0.05, bpm as u64 + 1),
            };
            let o = cantus_dsp::onset::detect(&mat.audio, mat.rate);
            let g = cantus_dsp::beat::analyse(&o, mat.rate, mat.audio.len(), 4);
            let near = (g.bpm - bpm).abs() <= 1.0;
            if (70.0..=180.0).contains(&bpm) {
                inside += 1;
                ok += near as usize;
            }
            if !near && [0.5, 2.0].iter().any(|k| (g.bpm - bpm * k).abs() <= 1.0) {
                octave += 1;
            }
        }
        bpm += 5.0;
    }
    out.push(metric(
        "warp.tempo.within_1bpm",
        "%",
        Better::Higher,
        &[ok as f64 * 100.0 / inside.max(1) as f64],
        "generated clicks and drums, 70-180 BPM",
    ));
    out.push(metric(
        "warp.tempo.octave",
        "files",
        Better::Lower,
        &[octave as f64],
        "60-200 BPM: those outside 70-180 are put inside by design",
    ));
    suite_review(plan, &mut out);
    out
}

fn mode_id(m: Mode) -> &'static str {
    match m {
        Mode::Complex => "complex",
        Mode::Rhythm => "rhythm",
        Mode::Melodic => "melodic",
        Mode::Repitch => "repitch",
    }
}

fn write(path: &Path, ch: &[Vec<f32>]) -> std::io::Result<()> {
    let spec = hound::WavSpec {
        channels: ch.len() as u16,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(std::io::Error::other)?;
    for i in 0..ch[0].len() {
        for c in ch {
            let v = (c[i].clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
            w.write_sample(v).map_err(std::io::Error::other)?;
        }
    }
    w.finalize().map_err(std::io::Error::other)
}

/// The listening set: every material as it is, then each mode x ratio (and a few
/// transpositions) — WAVs and a README for the owner. Returns how many files.
pub fn listen(dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    let mut n = 0;
    for mat in set() {
        write(&dir.join(format!("{}-source.wav", mat.name)), &mat.channels)?;
        n += 1;
        for mode in Mode::ALL {
            for ratio in RATIOS {
                let y = render(&mat, mode, ratio, 0.0);
                let name = format!("{}-{}-x{ratio}.wav", mat.name, mode_id(mode));
                write(&dir.join(name), &y)?;
                n += 1;
            }
        }
        for (mode, semis) in [
            (Mode::Complex, -3.0),
            (Mode::Complex, 7.0),
            (Mode::Melodic, -3.0),
            (Mode::Rhythm, 5.0),
        ] {
            let y = render(&mat, mode, 1.0, semis);
            let name = format!("{}-{}-transposed{semis:+}st.wav", mat.name, mode_id(mode));
            write(&dir.join(name), &y)?;
            n += 1;
        }
        // The brief's case: slowed by 25 % and down three semitones.
        let y = render(&mat, Mode::Melodic, 1.25, -3.0);
        write(&dir.join(format!("{}-melodic-x1.25-3st.wav", mat.name)), &y)?;
        n += 1;
    }
    std::fs::write(dir.join("README.txt"), README)?;
    Ok(n)
}

const README: &str = "Cantus round 29 - the warp listening set (ADR 0061)

Each material as it is (<material>-source.wav), then stretched by each mode:
  <material>-<mode>-x<ratio>.wav   ratio = output length / input length
  x0.5 twice as fast, x0.8 a little faster, x1.25 a little slower, x2 half speed
and transposed at its own length:
  <material>-<mode>-transposed<semitones>st.wav  (Complex -3 and +7, Melodic -3, Rhythm +5)
and the brief's case, slowed 25 % and down three semitones:
  <material>-melodic-x1.25-3st.wav

Materials: drums (a swung loop, 100 BPM), mix (the startup demo song's first 8 s),
voice (a sung phrase with vibrato), solo (a flute-like line), pad (held chords, wide).
Modes: complex (mixes, anything polyphonic: phase-locked STFT, transients copied),
rhythm (drums: slices at the transients), melodic (a voice: PSOLA, formants kept),
repitch (tape: speed and pitch together; it does not transpose).

What to listen for: hits as sharp as the source (no flam, no smear before them); no
phasiness or swirl on the mix and the pad at x0.8 and x1.25; a natural voice at x1.25
-3 st with its own vowel (not a chipmunk, not a giant); the pad's stereo image kept.
Compare with elastique (Ableton Complex Pro, FL Studio) or Rubber Band if at hand.
";

/// Render `mat` through `map` (output `len` frames) in `mode`, transposed `semis`, with
/// `onsets` as its transients.
fn render_map(
    mat: &Material,
    mode: Mode,
    map: &Map,
    len: usize,
    semis: f64,
    onsets: &[f64],
) -> Vec<Vec<f32>> {
    let refs: Vec<&[f32]> = mat.channels.iter().map(Vec::as_slice).collect();
    let pitch = (mode == Mode::Melodic).then(|| cantus_dsp::pitch::analyse(refs[0], SR));
    stretch::render(
        &refs,
        SR,
        &Request {
            mode,
            map,
            len,
            semitones: semis,
            formants: false,
            onsets,
            pitch: pitch.as_ref(),
            tail: Tail::Fade,
            origin: 0,
        },
    )
}

/// A tempo ramp under the whole material (speed `k0` to `k1`), sampled as the engine
/// samples one: soft points every 256 frames.
fn ramp_map(len_in: usize, k0: f64, k1: f64) -> (Map, usize) {
    let len = (len_in as f64 / ((k0 + k1) / 2.0)) as usize;
    let mut pts = Vec::new();
    let mut soft = Vec::new();
    let mut o = 0usize;
    loop {
        let t = o as f64;
        pts.push((t, k0 * t + (k1 - k0) * t * t / (2.0 * len as f64)));
        soft.push(o != 0 && o != len);
        if o == len {
            break;
        }
        o = (o + 256).min(len);
    }
    (Map::with_soft(pts, soft).expect("a ramp"), len)
}

/// A hit's attack (added to the source's), pre-echo (the 25 ms before it against it) and
/// timing (its place, less the source's own rise) in `y` against `x`, at every onset
/// `map` places that stands alone (no other within 50 ms before or 25 ms after: on dense
/// material the windows would measure the neighbours).
fn transients(x: &[f32], y: &[f32], map: &Map, onsets: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (mut att, mut pre, mut err) = (Vec::new(), Vec::new(), Vec::new());
    for (i, &s) in onsets.iter().enumerate() {
        let alone = (i == 0 || s - onsets[i - 1] > 2_400.0)
            && onsets.get(i + 1).is_none_or(|n| n - s > 1_200.0);
        let o = map.output(s).round() as usize;
        if !alone || o < 2_000 || o + 2_000 > y.len() || (s as usize) < 2_000 {
            continue;
        }
        let si = s as usize;
        // From just before the hit: what precedes it (a slowed slice's silence, the
        // source's decay) is not the hit.
        if let (Some(a), Some(a0)) = (
            m::attack_frames(y, SR, o - 48, o + 960),
            m::attack_frames(x, SR, si - 48, si + 960),
        ) {
            att.push(a as f64 - a0 as f64);
        }
        pre.push(m::pre_echo_db(y, o, 1_200, 48) as f64);
        if let (Some(r), Some(r0)) = (
            m::rise_at(y, o - 48, o + 400, 0.25),
            m::rise_at(x, si - 48, si + 400, 0.25),
        ) {
            err.push(((r as f64 - o as f64) - (r0 as f64 - s)).abs());
        }
    }
    (att, pre, err)
}

/// Round 29's review: the measurements on the conditions that make the guarantees hard.
fn suite_review(plan: &Plan, out: &mut Vec<Metric>) {
    // Transients on real material, from the detector's own onsets, ×0.8 and ×1.25 — and
    // the hits under a tempo ramp.
    let mats = if plan.quick {
        vec![drums()]
    } else {
        vec![drums(), mix()]
    };
    for mat in &mats {
        let found = cantus_dsp::onset::detect(&mat.channels[0], SR).frames;
        // The source's own, for reference: what a hit there has before it.
        let own: Vec<f64> =
            transients(&mat.channels[0], &mat.channels[0], &Map::identity(), &found).1;
        out.push(metric(
            &format!("warp.source.{}.pre_echo", mat.name),
            "dB",
            Better::Lower,
            &own,
            "the source itself: the 25 ms before a lone hit",
        ));
        for mode in [Mode::Complex, Mode::Rhythm] {
            let (mut att, mut pre, mut err) = (Vec::new(), Vec::new(), Vec::new());
            for ratio in [0.8, 1.25] {
                let len_in = mat.channels[0].len();
                let len = (len_in as f64 * ratio) as usize;
                let map = Map::new(vec![(0.0, 0.0), (len as f64, len_in as f64)]).expect("a map");
                let y = render_map(mat, mode, &map, len, 0.0, &found);
                let (a, p, e) = transients(&mat.channels[0], &y[0], &map, &found);
                att.extend(a);
                pre.extend(p);
                err.extend(e);
            }
            let id = format!("warp.{}.{}.detected", mode_id(mode), mat.name);
            out.push(metric(
                &format!("{id}.pre_echo"),
                "dB",
                Better::Lower,
                &pre,
                "the 25 ms before a lone hit against it, detected onsets",
            ));
            // A full mix has no hit's rise to time: its pre-echo only.
            if mat.name != "mix" {
                out.push(metric(
                    &format!("{id}.attack"),
                    "frames",
                    Better::Lower,
                    &att,
                    "added attack, detected onsets, x0.8/x1.25",
                ));
                out.push(metric(
                    &format!("{id}.timing"),
                    "frames",
                    Better::Lower,
                    &err,
                    "a hit's place, detected onsets",
                ));
            }
        }
    }
    let h = hits();
    for (k0, k1) in [(0.8, 1.25), (1.25, 0.8)] {
        let (map, len) = ramp_map(h.channels[0].len(), k0, k1);
        for mode in [Mode::Complex, Mode::Rhythm] {
            let y = render_map(&h, mode, &map, len, 0.0, &h.onsets);
            let (a, p, e) = transients(&h.channels[0], &y[0], &map, &h.onsets);
            let id = format!("warp.{}.ramp{k0}-{k1}", mode_id(mode));
            out.push(metric(
                &format!("{id}.attack"),
                "frames",
                Better::Lower,
                &a,
                "added attack under a tempo ramp",
            ));
            out.push(metric(
                &format!("{id}.pre_echo"),
                "dB",
                Better::Lower,
                &p,
                "the 25 ms before a hit against it, under a tempo ramp",
            ));
            out.push(metric(
                &format!("{id}.timing"),
                "frames",
                Better::Lower,
                &e,
                "a hit's place under a tempo ramp",
            ));
        }
    }
    // A held tone under hits: its level just before each hit (a reset must not dip it).
    let tone: Vec<f32> = (0..h.channels[0].len())
        .map(|i| 0.3 * math::sin(2.0 * std::f64::consts::PI * 440.0 * i as f64 / SR as f64) as f32)
        .collect();
    let both = Material {
        name: "tone+hits",
        channels: stereo(
            h.channels[0]
                .iter()
                .zip(&tone)
                .map(|(a, b)| a + b)
                .collect(),
        ),
        onsets: h.onsets.clone(),
    };
    let mut dips = Vec::new();
    for ratio in [0.8, 1.25] {
        let y = render(&both, Mode::Complex, ratio, 0.0);
        for &s in &both.onsets[2..both.onsets.len() - 1] {
            let o = (s * ratio).round() as usize;
            let a = m::amplitude(&y[0][o - 2_640..o - 240], SR, 440.0);
            dips.push((20.0 * math::log10(a / 0.3)).abs() as f64);
        }
    }
    out.push(metric(
        "warp.complex.held_under_hits",
        "dB",
        Better::Lower,
        &dips,
        "a 440 Hz tone's level before each hit, x0.8/x1.25",
    ));
    // The level of a transposed render: a harmonic tone and a dense multi-sine.
    let harmonic: Vec<f32> = (0..(3.0 * SR) as usize)
        .map(|i| {
            let w = 2.0 * std::f64::consts::PI * 220.0 * i as f64 / SR as f64;
            (0.4 * math::sin(w) + 0.2 * math::sin(2.0 * w) + 0.1 * math::sin(3.0 * w)) as f32
        })
        .collect();
    let mut g = Noise::new(31);
    let parts: Vec<(f64, f64)> = (0..200)
        .map(|_| {
            (
                100.0 + 4_900.0 * (0.5 + 0.5 * g.next() as f64),
                std::f64::consts::PI * g.next() as f64,
            )
        })
        .collect();
    let dense: Vec<f32> = (0..(3.0 * SR) as usize)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let v: f64 = parts
                .iter()
                .map(|(hz, ph)| math::sin(2.0 * std::f64::consts::PI * hz * t + ph))
                .sum();
            (0.02 * v) as f32
        })
        .collect();
    for (name, x) in [("tone", harmonic), ("noise", dense)] {
        let mat = Material {
            name: "level",
            channels: stereo(x),
            onsets: Vec::new(),
        };
        let n = mat.channels[0].len();
        let want = m::rms_db(&mat.channels[0][n / 4..n * 3 / 4]);
        let mut d = Vec::new();
        for semis in [-12.0, -5.0, 7.0, 12.0] {
            let y = render_map(&mat, Mode::Complex, &Map::identity(), n, semis, &[]);
            d.push((m::rms_db(&y[0][n / 4..n * 3 / 4]) - want).abs() as f64);
        }
        out.push(metric(
            &format!("warp.complex.level.{name}"),
            "dB",
            Better::Lower,
            &d,
            "a transposed render's level, -12/-5/+7/+12 st, formants not kept",
        ));
    }
    // Phasiness on each voice's own frequency, both channels, after the attack.
    let p = pad();
    let first = [48, 52, 55, 60];
    for ratio in [0.8, 1.25] {
        let y = render(&p, Mode::Complex, ratio, 0.0);
        let mut worst = 0.0f64;
        for (ch, detune) in [(0usize, -0.08f64), (1, 0.08)] {
            for note in first {
                let hz = (cantus_dsp::pitch::midi_to_hz(note as f32) as f64
                    * (1.0 + detune / 100.0 * 12.0)) as f32;
                let n0 = p.channels[ch].len();
                let n = y[ch].len();
                let from = |len: usize| (0.35 * SR as f64 * len as f64 / n0 as f64) as usize;
                let own =
                    m::level_wander_db_in(&p.channels[ch], SR, hz, from(n0), n0 / 4 - n0 / 32, 0.1)
                        as f64;
                let w = m::level_wander_db_in(&y[ch], SR, hz, from(n), n / 4 - n / 32, 0.1) as f64;
                worst = worst.max(w - own);
            }
        }
        out.push(metric(&format!("warp.complex.x{ratio}.wander_voices"), "dB", Better::Lower, &[worst], "the most any voice of a held chord wanders beyond its own, both channels, 100 ms windows"));
    }
    // A short window of a long file: the cost follows the window, not the file.
    let d = drums();
    let long: Vec<f32> = d.channels[0]
        .iter()
        .copied()
        .cycle()
        .take((60.0 * SR) as usize)
        .collect();
    let mat = Material {
        name: "long",
        channels: stereo(long),
        onsets: Vec::new(),
    };
    let len = (2.0 * SR) as usize;
    let map = Map::new(vec![
        (0.0, 30.0 * SR as f64),
        (len as f64, 31.6 * SR as f64),
    ])
    .expect("a map");
    let t = Instant::now();
    let _ = render_map(&mat, Mode::Complex, &map, len, -3.0, &[]);
    let ms = t.elapsed().as_secs_f64() * 1e3 / 2.0;
    out.push(metric(
        "warp.complex.cpu_window_of_long_file",
        "ms/s",
        Better::Lower,
        &[ms],
        "two seconds of output from a one-minute file",
    ));
    // The tempo corpus, played (humanised timing, drift, a pad under it, 3/4, 6/8).
    use cantus_dsp::beat::testkit::{groove, Feel, Metre};
    let (mut inside, mut ok, mut octave) = (0usize, 0usize, 0usize);
    let mut bpm = 60.0;
    while bpm <= 200.0 {
        for (metre, jitter) in [
            (Metre::FourFour, 8.0),
            (Metre::FourFour, 15.0),
            (Metre::ThreeFour, 8.0),
        ] {
            let feel = Feel {
                metre,
                jitter_ms: jitter,
                drift: 0.01,
                pad_db: Some(-20.0),
                pickup: 0,
            };
            let mat = groove(48_000.0, bpm, 6, 0.1, feel, bpm as u64);
            let o = cantus_dsp::onset::detect(&mat.audio, mat.rate);
            let g = cantus_dsp::beat::analyse(&o, mat.rate, mat.audio.len(), 4);
            let near = (g.bpm - bpm).abs() <= 1.0;
            if (70.0..=180.0).contains(&bpm) {
                inside += 1;
                ok += near as usize;
            }
            if !near && [0.5, 2.0].iter().any(|k| (g.bpm - bpm * k).abs() <= 1.0) {
                octave += 1;
            }
        }
        bpm += 5.0;
    }
    out.push(metric(
        "warp.tempo.played.within_1bpm",
        "%",
        Better::Higher,
        &[ok as f64 * 100.0 / inside.max(1) as f64],
        "humanised grooves (jitter, drift, pad, 3/4), 70-180 BPM",
    ));
    out.push(metric(
        "warp.tempo.played.octave",
        "files",
        Better::Lower,
        &[octave as f64],
        "played grooves answered by an octave",
    ));
}

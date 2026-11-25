//! Pass 5's probes and gates (round 42, `docs/round42/arcus-match.md` *Pass 5*): the faults a
//! musical demo found by measurement — the tremolo's pitch, the violin's top octave at ff,
//! the bow change's tick, the spiccato that never left the string, the plain re-bow's noise.

use super::tables::Instrument;
use super::tests::{apply, cents, id_of, measure_hz, Ev, SR};
use super::*;
use crate::math;

/// Play `score` (times in seconds) with `s`, in blocks of 256, cutting at every event: the
/// left channel.
pub(super) fn play(s: &ArcusSettings, score: &[(f64, Ev)], seconds: f64) -> Vec<f32> {
    let mut a = Arcus::new(SR);
    a.set(s);
    let frames = (seconds * SR as f64) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    let mut ev = 0;
    let mut at = 0;
    while at < frames {
        while ev < score.len() && (score[ev].0 * SR as f64) as usize <= at {
            apply(&mut a, score[ev].1);
            ev += 1;
        }
        let next = score
            .get(ev)
            .map_or(frames, |e| ((e.0 * SR as f64) as usize).max(at + 1));
        let n = 256.min(next - at).min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    l
}

/// The pitch of `x` near `want` Hz over `from…to` s, blind to the waveform's sign: a bow
/// reversal turns a Helmholtz sawtooth over (the fundamental's phase jumps by half a turn),
/// which an ear does not hear as pitch — so the fundamental is read from the square's second
/// harmonic, whose phase a sign does not move.
pub(super) fn pitch_hz(x: &[f32], want: f64, from: f64, to: f64) -> f64 {
    let sq: Vec<f32> = x.iter().map(|v| v * v).collect();
    0.5 * measure_hz(&sq, 2.0 * want, from, to)
}

/// The short-time pitch of `x` near `want` Hz over `from…to` s, as a tracker hears it: in
/// frames of four periods (at least 20 ms), hopped by half a frame, the fundamental's
/// frequency from its phase between the frame's two halves (each Hann-weighted). A bow's
/// reversal spoils the frames across it — the median over the frames sets them aside. The
/// cents of every frame, in time order.
pub(super) fn frame_cents(x: &[f32], want: f64, from: f64, to: f64) -> Vec<f64> {
    let sr = SR as f64;
    // What lies an octave under the note (a bow's stroke-rate push on the bridge) is no
    // part of its pitch: a fourth-order high-pass at half the fundamental first.
    let x = &highpass(&highpass(x, 0.5 * want), 0.5 * want);
    let half = ((2.0 * sr / want).max(0.01 * sr)).round() as usize;
    let w = std::f64::consts::TAU * want / sr;
    let (a, b) = ((from * sr) as usize, ((to * sr) as usize).min(x.len()));
    let sum = |at: usize| -> (f64, f64) {
        let (mut re, mut im) = (0.0, 0.0);
        for j in 0..half {
            let h = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * (j as f64 + 0.5) / half as f64);
            let n = at + j;
            let (sn, cs) = math::sin_cos(w * n as f64);
            re += h * x[n] as f64 * cs;
            im -= h * x[n] as f64 * sn;
        }
        (re, im)
    };
    let mut out = Vec::new();
    let mut at = a;
    while at + 2 * half <= b {
        let (r0, i0) = sum(at);
        let (r1, i1) = sum(at + half);
        // The phase advance between the halves past the reference's.
        let d = math::atan2(i1 * r0 - r1 * i0, r1 * r0 + i1 * i0);
        let f = want + d / (std::f64::consts::TAU * half as f64 / sr);
        if f > 0.0 {
            out.push(cents(f, want));
        }
        at += half;
    }
    out
}

/// The study's tracker (`tools/arcus/study/bowlib_core.py`'s `yin_track`: de Cheveigné &
/// Kawahara's difference function over three of the note's periods, hopped every `hop_s`,
/// the dip refined by a parabola) searched within ±12 % of the note's period — a note's
/// pitch, never an octave of it: each frame's cents from `want` and its aperiodicity (the
/// dip of the difference normalised by the frames' power, 0 for a periodic signal).
pub(super) fn yin_frames(x: &[f32], want: f64, from: f64, to: f64, hop_s: f64) -> Vec<(f64, f64)> {
    yin_frames_p(x, want, from, to, hop_s, 3.0)
}

/// [`yin_frames`] over `periods` of the note (the study's `_track`: two).
pub(super) fn yin_frames_p(
    x: &[f32],
    want: f64,
    from: f64,
    to: f64,
    hop_s: f64,
    periods: f64,
) -> Vec<(f64, f64)> {
    let sr = SR as f64;
    let p = sr / want;
    let w = (periods * p).ceil() as usize;
    let (t_lo, t_hi) = ((p / 1.12).floor() as usize, (p * 1.12).ceil() as usize);
    let hop = ((hop_s * sr) as usize).max(1);
    let (a, b) = ((from * sr) as usize, ((to * sr) as usize).min(x.len()));
    let mut out = Vec::new();
    let mut at = a;
    while at + w + t_hi + 1 < b {
        let e1: f64 = x[at..at + w].iter().map(|v| (*v as f64).powi(2)).sum();
        let mut d = Vec::with_capacity(t_hi - t_lo + 3);
        for tau in t_lo - 1..=t_hi + 1 {
            let (mut e2, mut acf) = (0.0f64, 0.0f64);
            for j in 0..w {
                let (u, v) = (x[at + j] as f64, x[at + j + tau] as f64);
                e2 += v * v;
                acf += u * v;
            }
            d.push((e1 + e2 - 2.0 * acf) / (e1 + e2).max(1e-30));
        }
        let (mut i, mut best) = (1usize, f64::MAX);
        for k in 1..d.len() - 1 {
            if d[k] < best {
                best = d[k];
                i = k;
            }
        }
        let (l, c, r) = (d[i - 1], d[i], d[i + 1]);
        let den = l - 2.0 * c + r;
        let off = if den.abs() > 1e-12 {
            (0.5 * (l - r) / den).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let tau = (t_lo - 1 + i) as f64 + off;
        out.push((cents(sr / tau, want), best));
        at += hop;
    }
    out
}

/// The note's pitch as the study's tracker hears it: the median cents over the periodic
/// frames (aperiodicity under 0.35), their p10 and p90, and the periodic frames' share.
pub(super) fn yin_cents(x: &[f32], want: f64, from: f64, to: f64) -> (f64, f64, f64, f64) {
    let fr = yin_frames(x, want, from, to, 0.005);
    let ok: Vec<f64> = fr.iter().filter(|f| f.1 < 0.35).map(|f| f.0).collect();
    let (m, lo, hi) = spread(&ok);
    (m, lo, hi, ok.len() as f64 / fr.len().max(1) as f64)
}

/// A second-order Butterworth high-pass at `fc` Hz.
pub(super) fn highpass(x: &[f32], fc: f64) -> Vec<f32> {
    let w = std::f64::consts::TAU * fc / SR as f64;
    let (sn, cs) = math::sin_cos(w);
    let alpha = sn / std::f64::consts::SQRT_2;
    let a0 = 1.0 + alpha;
    let (b0, b1, b2) = (
        (1.0 + cs) / 2.0 / a0,
        -(1.0 + cs) / a0,
        (1.0 + cs) / 2.0 / a0,
    );
    let (a1, a2) = (-2.0 * cs / a0, (1.0 - alpha) / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    x.iter()
        .map(|v| {
            let v = *v as f64;
            let y = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            (x2, x1, y2, y1) = (x1, v, y1, y);
            y as f32
        })
        .collect()
}

/// The median and the 10th and 90th percentiles of `v`.
pub(super) fn spread(v: &[f64]) -> (f64, f64, f64) {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    if s.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let at = |q: f64| s[((q * (s.len() - 1) as f64).round() as usize).min(s.len() - 1)];
    (at(0.5), at(0.1), at(0.9))
}

pub(super) fn hz(key: f64) -> f64 {
    440.0 * math::exp2((key - 69.0) / 12.0)
}

/// The settings a probe plays with: the instrument's defaults, the vibrato off, the dynamic
/// from the velocity.
pub(super) fn plain(inst: Instrument) -> ArcusSettings {
    ArcusSettings {
        instrument: inst,
        vib_depth: 0.0,
        dyn_src: DynSource::Velocity,
        ..ArcusSettings::default()
    }
}

/// A tremolo on `key`: alone (`slurred` false) or slurred into from two keys under it (the
/// two notes overlapping 30 ms, as the demo writes them). The cents it sounds from its key
/// over its held part, in windows: the whole, and each quarter.
pub(super) fn tremolo_cents(
    s: &ArcusSettings,
    key: u8,
    velocity: f32,
    slurred: bool,
) -> (f64, [f64; 7]) {
    let t0 = if slurred { 0.6 } else { 0.0 };
    let mut score = Vec::new();
    if slurred {
        let from = key.saturating_sub(2);
        score.push((0.01, Ev::Art(from, Some(Art::Tremolo), None)));
        score.push((0.01, Ev::On(from, velocity)));
        score.push((t0 + 0.03, Ev::Off(from)));
        score.push((t0, Ev::Art(key, Some(Art::Tremolo), None)));
    } else {
        score.push((t0 + 0.01, Ev::Art(key, Some(Art::Tremolo), None)));
    }
    score.push((t0 + 0.01, Ev::On(key, velocity)));
    score.push((t0 + 2.0, Ev::Off(key)));
    let x = play(s, &score, t0 + 2.0);
    let want = hz(key as f64);
    let all = cents(pitch_hz(&x, want, t0 + 0.25, t0 + 1.95), want);
    let (med, p10, p90, share) = yin_cents(&x, want, t0 + 0.25, t0 + 1.95);
    let mut q = [0.0; 4];
    for (i, c) in q.iter_mut().enumerate() {
        let a = t0 + 0.25 + 0.425 * i as f64;
        *c = yin_cents(&x, want, a, a + 0.425).0;
    }
    let _ = all;
    (med, [q[0], q[1], q[2], q[3], p10, p90, share])
}

/// A probe (ignored): the tremolo's pitch on every instrument's strings, open and stopped,
/// alone and slurred — `cargo test -p cantus-dsp --lib arcus::tests_p5::probe_tremolo --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn probe_tremolo() {
    for inst in Instrument::ALL {
        let spec = inst.spec();
        for st in spec.strings {
            for key in [st.open, st.open + 4, st.open + 7] {
                let x = play(
                    &plain(inst),
                    &[(0.01, Ev::On(key, 0.6)), (2.0, Ev::Off(key))],
                    2.0,
                );
                let want = hz(key as f64);
                println!(
                    "{inst:?} key {key} arco v 0.6: {:+.2} ct (sign-blind), {:+.2} ct (phase), {:+.2} (frames)",
                    cents(pitch_hz(&x, want, 0.6, 1.95), want),
                    cents(measure_hz(&x, want, 0.6, 1.95), want),
                    spread(&frame_cents(&x, want, 0.6, 1.95)).0
                );
                for slurred in [false, true] {
                    for vel in [0.4f32, 0.8] {
                        let (all, q) = tremolo_cents(&plain(inst), key, vel, slurred);
                        println!(
                            "{inst:?} key {key} {} v {vel}: {all:+.1} ct (quarters {:+.1} {:+.1} {:+.1} {:+.1}; p10 {:+.1} p90 {:+.1}; periodic {:.2})",
                            if slurred { "slurred" } else { "alone  " },
                            q[0],
                            q[1],
                            q[2],
                            q[3],
                            q[4],
                            q[5],
                            q[6]
                        );
                    }
                }
            }
        }
    }
}

/// A probe (ignored): one tremolo's bow, every 10 ms — its speed and force against the
/// region's limits, the slips' motion over the last 20 ms, the trim and the frames' pitch.
/// `ARCUS_TREM=<inst index>:<key>:<velocity>` (default the cello's open A).
#[test]
#[ignore]
fn probe_tremolo_trace() {
    use super::tests_gate::{classify, Bowed};
    let (inst, key, vel) = std::env::var("ARCUS_TREM")
        .ok()
        .and_then(|v| {
            let p: Vec<&str> = v.split(':').collect();
            Some((
                Instrument::ALL[p.first()?.parse::<usize>().ok()?],
                p.get(1)?.parse::<u8>().ok()?,
                p.get(2)?.parse::<f32>().ok()?,
            ))
        })
        .unwrap_or((Instrument::Cello, 57, 0.6));
    let mut a = Arcus::new(SR);
    a.set(&plain(inst));
    a.note_art(id_of(key), Some(Art::Tremolo), None);
    a.note_on(key, vel, id_of(key));
    let v = a.voices.iter().position(|v| v.active()).expect("a voice");
    a.voices[v].record = true;
    let frames = (1.0 * SR) as usize;
    let hop = (0.01 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    let f = hz(key as f64);
    let os = super::upper::factor(inst) as f64;
    for at in (0..frames).step_by(hop) {
        let n = hop.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        let vc = &a.voices[v];
        let (lo, hi) = vc.limits();
        let slips = &vc.slips;
        let from = slips.len().saturating_sub((0.02 * SR as f64 * os) as usize);
        let b = Bowed {
            slips: slips[from..].to_vec(),
            out: Vec::new(),
            period: os * SR as f64 / f,
            beta: vc.beta() as f64,
        };
        let m = if b.slips.len() as f64 > 3.0 * b.period {
            format!("{:?}", classify(&b, 0))
        } else {
            "-".to_string()
        };
        let e = l[at..at + n].iter().map(|x| (x * x) as f64).sum::<f64>() / n as f64;
        println!(
            "{:.2}s {:+6.1} dB v {:+.3} F {:.3} [{:.3} {:.3}] {:?} trim {:+.2} P/P0 {:.4} {m}",
            (at + n) as f64 / SR as f64,
            10.0 * math::log10(e + 1e-20),
            vc.bow_velocity(),
            vc.bow_force(),
            lo,
            hi,
            vc.phase(),
            vc.trim,
            vc.string.period() / (os * SR as f64 / f)
        );
    }
    let fc = frame_cents(&l, f, 0.2, 0.98);
    println!(
        "frames: {:?}",
        fc.iter().map(|c| c.round() as i64).collect::<Vec<_>>()
    );
}

/// A probe (ignored): the tremolo's stroke shapes scanned — `ARCUS_KNOBS=peak:force:floor:knee;…`.
#[test]
#[ignore]
fn probe_tremolo_scan() {
    let knobs: Vec<(f32, f32, f32, f32)> = std::env::var("ARCUS_KNOBS")
        .unwrap_or_else(|_| "1.5:1.35:0.1:1".into())
        .split(';')
        .filter_map(|k| {
            let p: Vec<f32> = k.split(':').filter_map(|x| x.parse().ok()).collect();
            (p.len() == 4).then(|| (p[0], p[1], p[2], p[3]))
        })
        .collect();
    let cases = [
        (Instrument::Cello, 57u8),
        (Instrument::Cello, 47),
        (Instrument::Cello, 40),
        (Instrument::Bass, 40),
        (Instrument::Bass, 28),
        (Instrument::Bass, 33),
        (Instrument::Bass, 47),
        (Instrument::Viola, 55),
        (Instrument::Violin, 69),
        (Instrument::Violin, 83),
    ];
    for k in knobs {
        voice::TREM_KNOBS.with(|c| c.set(k));
        let mut worst = 0f64;
        let mut line = String::new();
        for (inst, key) in cases {
            let mut s = plain(inst);
            if std::env::var_os("ARCUS_NOBODY").is_some() {
                s.body_mix = 0.0;
            }
            let vel = std::env::var("ARCUS_VEL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.6);
            let (med, q) = tremolo_cents(&s, key, vel, false);
            worst = worst.max(med.abs());
            line += &format!(
                " {inst:?}{key} {med:+.1}[{:+.0},{:+.0} {:.2}]",
                q[4], q[5], q[6]
            );
        }
        println!("{k:?}: worst {worst:.0} |{line}");
    }
}

/// A probe (ignored): raw f32 renders for a look outside — `ARCUS_DUMP=<folder>`; the
/// cases are `ARCUS_TREM`'s form, `;`-separated, `t` after the velocity for a tremolo.
#[test]
#[ignore]
fn probe_dump() {
    let dir = std::path::PathBuf::from(std::env::var("ARCUS_DUMP").expect("ARCUS_DUMP"));
    let cases = std::env::var("ARCUS_CASES").unwrap_or_else(|_| "0:47:0.6:t;0:47:0.6:a".into());
    for c in cases.split(';') {
        let p: Vec<&str> = c.split(':').collect();
        let inst = Instrument::ALL[p[0].parse::<usize>().unwrap()];
        let key: u8 = p[1].parse().unwrap();
        let vel: f32 = p[2].parse().unwrap();
        let mut score = Vec::new();
        if p.get(3) == Some(&"t") {
            score.push((0.01, Ev::Art(key, Some(Art::Tremolo), None)));
        }
        score.push((0.01, Ev::On(key, vel)));
        score.push((2.0, Ev::Off(key)));
        let mut s = plain(inst);
        if p.get(4) == Some(&"n") {
            s.body_mix = 0.0;
        }
        if let Some(k) = std::env::var("ARCUS_KNOB").ok().and_then(|k| {
            let p: Vec<f32> = k.split(':').filter_map(|x| x.parse().ok()).collect();
            (p.len() == 4).then(|| (p[0], p[1], p[2], p[3]))
        }) {
            voice::TREM_KNOBS.with(|c| c.set(k));
        }
        let x = play(&s, &score, 2.5);
        let bytes: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(dir.join(format!("{}.f32", c.replace(':', "_"))), bytes).unwrap();
    }
}

/// The string alone bowed by `v(t)` (m/s, signed) and `force(t)` (N) at `beta`: the slips
/// and the output (the bridge's wave), as `tests_gate::bow_gesture` bows it.
pub(super) fn bow_profile(
    inst: Instrument,
    key: f64,
    beta: f64,
    v: &dyn Fn(f64) -> f32,
    force: &dyn Fn(f64) -> f32,
    seconds: f64,
) -> super::tests_gate::Bowed {
    use super::string::{Bow, BowedString, Friction, StringStep};
    let spec = inst.spec();
    let st = spec.strings[spec.string_for(key as f32)];
    let f = hz(key);
    let stopped = (st.open_hz() / f).clamp(0.1, 1.0);
    let mut s = BowedString::new(SR);
    let step = StringStep {
        f0: f,
        t60: st.t60_s * (0.55 + 0.45 * stopped),
        p: st.p,
        beta,
        dc: false,
        corner_hz: spec.corner_hz,
    };
    s.start(st.z0() as f32, &step, false);
    let n = (seconds * SR as f64) as usize;
    let mut out = vec![0.0f32; n];
    let mut slips = vec![0.0f32; n];
    let zeros = [0.0f32; 32];
    let (mut vb, mut fb) = ([0.0f32; 32], [0.0f32; 32]);
    for at in (0..n).step_by(32) {
        let k = 32.min(n - at);
        for j in 0..k {
            let t = (at + j) as f64 / SR as f64;
            vb[j] = v(t);
            fb[j] = force(t);
        }
        let bow = Bow {
            v: &vb[..k],
            force: &fb[..k],
            rough: &zeros[..k],
            friction: Friction::Hyperbolic,
        };
        s.render(
            &zeros[..k],
            Some(bow),
            1.0,
            &mut out[at..at + k],
            Some(&mut slips[at..at + k]),
            false,
        );
    }
    super::tests_gate::Bowed {
        slips,
        out,
        period: SR as f64 / f,
        beta,
    }
}

/// A probe (ignored): one bow change on a string in Helmholtz motion — how long the new
/// stroke takes to settle, by the reversal's time and the force through it.
#[test]
#[ignore]
fn probe_reversal() {
    use super::tests_gate::{attack_time, Bowed};
    for (inst, key) in [
        (Instrument::Cello, 47.0),
        (Instrument::Cello, 57.0),
        (Instrument::Cello, 40.0),
        (Instrument::Bass, 47.0),
        (Instrument::Bass, 33.0),
        (Instrument::Viola, 55.0),
    ] {
        let spec = inst.spec();
        let st = spec.strings[spec.string_for(key as f32)];
        let period = SR as f64 / hz(key);
        let bw = voice::bowing(spec, 0.6, 0.0, 0.0, 1.0, st.z0(), period);
        let (v0, f0) = (bw.speed as f32, bw.force as f32);
        let mut line = String::new();
        for rev_ms in [2.0f64, 8.0, 25.0] {
            for dip in [1.0f32, 0.5, 0.2] {
                let t_r = 0.4;
                let half = 0.0005 * rev_ms;
                let v = move |t: f64| -> f32 {
                    if t < t_r - half {
                        v0
                    } else if t > t_r + half {
                        -v0
                    } else {
                        v0 * (-(t - t_r) / half) as f32
                    }
                };
                let fo = move |t: f64| -> f32 {
                    let x = ((t - t_r).abs() / (half + 0.01)).min(1.0) as f32;
                    f0 * (dip + (1.0 - dip) * x)
                };
                let b = bow_profile(inst, key, bw.beta, &v, &fo, 0.8);
                let from = ((t_r + half) * SR as f64) as usize;
                let after = Bowed {
                    slips: b.slips[from..].to_vec(),
                    out: Vec::new(),
                    period: b.period,
                    beta: b.beta,
                };
                let before = Bowed {
                    slips: b.slips[(0.2 * SR as f64) as usize..from].to_vec(),
                    out: Vec::new(),
                    period: b.period,
                    beta: b.beta,
                };
                let settled = |x: &Bowed| attack_time(x).map_or(-1.0, |t| 1000.0 * t);
                line += &format!(
                    " {rev_ms}ms×{dip}: {:.0} ({:.0})",
                    settled(&after),
                    settled(&before)
                );
            }
        }
        println!("{inst:?} {key} v {v0:.3} F {f0:.2} β {:.3}:{line}", bw.beta);
    }
}

/// The cents a tremolo at `key` and dynamic `d` sounds from its key with `table` (the study's
/// tracker, the strokes' held part: 0.25–1.95 s).
fn trem_offset(inst: Instrument, key: u8, d: f64, table: &'static pitch::PitchTable) -> f64 {
    let mut a = Arcus::new(SR);
    a.set(&plain(inst));
    a.set_pitch_table(Some(table));
    a.note_art(id_of(key), Some(Art::Tremolo), None);
    a.note_on(key, (d as f32).max(0.01), id_of(key));
    let frames = (2.0 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    let want = hz(key as f64);
    yin_cents(&l, want, 0.25, 1.95).0
}

/// The generator (ignored): every instrument's tremolo measured at each key and each of
/// `TREM_DYN`, twice (the offset, then what is left with it applied), written to
/// `arcus_trem_pitch_data.rs` — `bash tools/arcus/trem-table.sh`.
#[test]
#[ignore]
fn write_tremolo_table() {
    use pitch::{TremTable, TABLE, TREM_DYN};
    use std::fmt::Write;
    let names = ["CELLO", "BASS", "VIOLIN", "VIOLA"];
    let measure = |t: &'static pitch::PitchTable| -> Vec<Vec<f32>> {
        Instrument::ALL
            .iter()
            .map(|inst| {
                let spec = inst.spec();
                let mut v = Vec::new();
                for key in spec.bottom()..=spec.top {
                    for d in TREM_DYN {
                        let c = trem_offset(*inst, key, d, t);
                        v.push(if c.is_finite() { c as f32 } else { 0.0 });
                    }
                }
                v
            })
            .collect()
    };
    let with = |cents: &[Vec<f32>]| -> &'static pitch::PitchTable {
        let mut t = TABLE;
        for (j, inst) in Instrument::ALL.iter().enumerate() {
            let spec = inst.spec();
            t.trem[*inst as usize] = TremTable {
                lo: spec.bottom(),
                keys: (spec.top - spec.bottom() + 1) as usize,
                cents: Box::leak(cents[j].clone().into_boxed_slice()),
            };
        }
        Box::leak(Box::new(t))
    };
    let mut empty = TABLE;
    for t in empty.trem.iter_mut() {
        t.keys = 0;
    }
    let first = measure(Box::leak(Box::new(empty)));
    let left = measure(with(&first));
    let fin: Vec<Vec<f32>> = first
        .iter()
        .zip(&left)
        .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x + y).collect())
        .collect();
    let check = measure(with(&fin));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "//! Arcus's tremolo pitch table — **generated** by `tools/arcus/trem-table.sh`\n\
         //! (`arcus_tests_p5.rs`'s `write_tremolo_table`); do not edit by hand. Cents each\n\
         //! tremolo key sounds from its loop at each dynamic ({TREM_DYN:?}), by the study's\n\
         //! tracker (the strokes' median pitch, 0.25–1.95 s, the instrument's body on, no\n\
         //! vibrato), measured twice (the offset, then what was left with it applied).\n\n\
         #![allow(clippy::approx_constant)] // measured cents that happen to read like π or √2\n"
    );
    for (j, inst) in Instrument::ALL.iter().enumerate() {
        let spec = inst.spec();
        let worst = check[j].iter().fold(0f32, |m, x| m.max(x.abs()));
        let span = first[j]
            .iter()
            .fold((0f32, 0f32), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
        eprintln!(
            "{}: offsets {:+.2} … {:+.2} ct; left with the table at most {worst:.2} ct",
            names[j], span.0, span.1
        );
        let name = names[*inst as usize];
        let _ = writeln!(out, "pub const {name}_LO: u8 = {};", spec.bottom());
        let _ = writeln!(
            out,
            "pub const {name}_KEYS: usize = {};",
            spec.top - spec.bottom() + 1
        );
        let _ = writeln!(out, "pub const {name}: [f32; {}] = [", fin[j].len());
        for row in fin[j].chunks(TREM_DYN.len()) {
            let cells: Vec<String> = row.iter().map(|c| format!("{c:.3}")).collect();
            let _ = writeln!(out, "    {},", cells.join(", "));
        }
        let _ = writeln!(out, "];");
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/arcus_trem_pitch_data.rs");
    std::fs::write(&path, out).expect("the table written");
    eprintln!("written {}", path.display());
}

/// The violin's cantabile preset as the engine reads it (`bow.pos` 1.09 → −0.25, the
/// vibrato 1.2 × from 160 ms over 320 ms, `bow.attack` 1.4), the dynamic from CC1.
pub(super) fn violin_cantabile(vib: bool) -> ArcusSettings {
    ArcusSettings {
        instrument: Instrument::Violin,
        vib_depth: if vib { 1.2 } else { 0.0 },
        vib_delay_ms: 160.0,
        vib_attack_ms: 320.0,
        bow_pos: -0.249,
        bow_attack: 1.4,
        body_ring: 1.15,
        ..ArcusSettings::default()
    }
}

/// A note at `key` held `len` s at CC1 `cc` (velocity 0.8): its YIN cents over its body.
pub(super) fn held_cents(s: &ArcusSettings, key: u8, cc: f32, len: f64) -> (f64, f64, f64, f64) {
    let score = [
        (0.0, Ev::Cc(0, cc)),
        (0.01, Ev::On(key, 0.8)),
        (0.01 + len, Ev::Off(key)),
    ];
    let x = play(s, &score, len + 0.1);
    yin_cents(&x, hz(key as f64), 0.15, len)
}

/// A probe (ignored): the violin's top octave and a half at ff and mf, short and long,
/// vibrato on and off.
#[test]
#[ignore]
fn probe_violin_top() {
    for key in 74u8..=93 {
        let mut line = String::new();
        for (cc, vib, len) in [
            (1.0, false, 1.5),
            (1.0, true, 1.5),
            (1.0, true, 0.5),
            (0.6, true, 0.5),
            (0.85, false, 0.5),
        ] {
            let (m, lo, hi, share) = held_cents(&violin_cantabile(vib), key, cc, len);
            line += &format!(
                " cc{cc} v{} {len}s: {m:+.1} [{lo:+.0},{hi:+.0} {share:.2}] |",
                vib as u8
            );
        }
        println!("violin {key}:{line}");
    }
}

/// The demo's climax for the violin (`examples/arcus_demo/parts.rs`, section V): each note
/// `(key, beats, slurred, velocity)` at 74 BPM, CC1 along its points, an off and the next
/// note's on at the same sample.
pub(super) const CLIMAX: [(u8, f64, bool, f32); 21] = [
    (78, 1.5, false, 0.66),
    (76, 0.5, true, 0.63),
    (78, 1.0, true, 0.63),
    (81, 1.0, true, 0.63),
    (83, 1.5, false, 0.66),
    (81, 0.5, true, 0.63),
    (79, 1.0, true, 0.63),
    (76, 1.0, true, 0.63),
    (79, 1.0, false, 0.66),
    (76, 1.0, true, 0.63),
    (81, 1.0, false, 0.66),
    (85, 1.0, true, 0.63),
    (86, 2.0, false, 0.77),
    (83, 1.5, false, 0.66),
    (79, 0.5, true, 0.63),
    (82, 1.0, false, 0.66),
    (81, 1.0, true, 0.63),
    (78, 1.0, false, 0.66),
    (76, 1.0, true, 0.63),
    (74, 3.0, false, 0.66),
    (74, 0.0, false, 0.0),
];

/// A phrase's score: `notes` at `bpm`, CC1 from `cc1` (beats, value) every 50 ms; and each
/// note's (key, start s, end s).
pub(super) fn phrase_score(
    notes: &[(u8, f64, bool, f32)],
    cc1: &[(f64, f32)],
    bpm: f64,
) -> (Vec<(f64, Ev)>, Vec<(u8, f64, f64)>) {
    let beat = 60.0 / bpm;
    let mut score = Vec::new();
    let mut spans = Vec::new();
    let total: f64 = notes.iter().map(|n| n.1).sum();
    let mut t = 0.0;
    while t <= total * beat {
        let b = t / beat;
        let i = cc1.iter().rposition(|p| p.0 <= b).unwrap_or(0);
        let v = match cc1.get(i + 1) {
            Some(n) => cc1[i].1 + (n.1 - cc1[i].1) * ((b - cc1[i].0) / (n.0 - cc1[i].0)) as f32,
            None => cc1[i].1,
        };
        score.push((t, Ev::Cc(0, v)));
        t += 0.05;
    }
    let mut at = 0.01;
    let mut last: Option<u8> = None;
    for &(key, beats, slurred, vel) in notes {
        if beats == 0.0 {
            continue;
        }
        if let Some(k) = last {
            score.push((at, Ev::Off(k)));
        }
        if slurred {
            score.push((at, Ev::Art(key, Some(Art::Legato), None)));
        }
        score.push((at, Ev::On(key, vel)));
        spans.push((key, at, at + beats * beat));
        last = Some(key);
        at += beats * beat;
    }
    if let Some(k) = last {
        score.push((at, Ev::Off(k)));
    }
    score.sort_by(|a, b| a.0.total_cmp(&b.0));
    (score, spans)
}

/// A probe (ignored): the demo's climax on the violin — each note's pitch.
#[test]
#[ignore]
fn probe_violin_climax() {
    let cc1 = [
        (0.0, 0.64),
        (8.0, 0.76),
        (12.0, 0.9),
        (14.0, 0.92),
        (18.0, 0.74),
        (20.0, 0.62),
        (24.0, 0.48),
    ];
    for vib in [false, true] {
        let (score, spans) = phrase_score(&CLIMAX, &cc1, 74.0);
        let end = spans.last().unwrap().2 + 0.5;
        let x = play(&violin_cantabile(vib), &score, end);
        let mut line = String::new();
        for (key, a, b) in spans {
            let (m, _, _, _) = yin_cents(&x, hz(key as f64), a + 0.12, b - 0.03);
            line += &format!(" {key}:{m:+.1}");
        }
        println!("vibrato {vib}:{line}");
    }
}

/// The phrases the bow-change and spiccato measures read (`tools/arcus/pass5/strokes.py`):
/// for `inst`, a détaché scale down and up at 120 BPM eighths (each note its own bow, the
/// last descending change onto the lowest), the same pitch re-bowed eight times, and
/// spiccato eighths at 74 BPM — as (name, score, seconds).
pub(super) fn stroke_phrases(inst: Instrument) -> Vec<(&'static str, Vec<(f64, Ev)>, f64)> {
    let spec = inst.spec();
    let low = spec.strings[0].open;
    let scale = [7u8, 5, 4, 2, 0, 2, 4, 5, 7, 12, 7, 0];
    let mut out = Vec::new();
    let eighth = |bpm: f64| 30.0 / bpm;
    let mut det = vec![(0.0, Ev::Cc(0, 0.6))];
    let mut t = 0.05;
    let d = eighth(120.0);
    for (i, s) in scale.iter().enumerate() {
        let k = low + 12 + s;
        if i > 0 {
            det.push((t, Ev::Off(low + 12 + scale[i - 1])));
        }
        det.push((t, Ev::On(k, 0.7)));
        t += d;
    }
    det.push((t, Ev::Off(low + 12 + scale[scale.len() - 1])));
    out.push(("detache", det, t + 0.4));
    let mut re = vec![(0.0, Ev::Cc(0, 0.6))];
    let k = low + 19;
    let mut t = 0.05;
    for i in 0..8 {
        if i > 0 {
            re.push((t, Ev::Off(k)));
        }
        re.push((t, Ev::On(k, 0.7)));
        t += eighth(100.0);
    }
    re.push((t, Ev::Off(k)));
    out.push(("rebow", re, t + 0.4));
    let mut sp = vec![(0.0, Ev::Cc(0, 0.6))];
    let mut t = 0.05;
    let d = eighth(74.0);
    for s in [0u8, 4, 7, 12, 7, 4, 0, 4] {
        let k = low + 12 + s;
        sp.push((t, Ev::Art(k, Some(Art::Spiccato), None)));
        sp.push((t, Ev::On(k, 0.75)));
        sp.push((t + d, Ev::Off(k)));
        t += d;
    }
    sp.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.push(("spiccato", sp, t + 0.6));
    out
}

/// A probe (ignored): the stroke phrases of every instrument written raw for
/// `tools/arcus/pass5/strokes.py` — `ARCUS_DUMP=<folder>`.
#[test]
#[ignore]
fn probe_dump_strokes() {
    let dir = std::path::PathBuf::from(std::env::var("ARCUS_DUMP").expect("ARCUS_DUMP"));
    for inst in Instrument::ALL {
        for (name, score, secs) in stroke_phrases(inst) {
            let x = play(
                &ArcusSettings {
                    instrument: inst,
                    ..ArcusSettings::default()
                },
                &score,
                secs,
            );
            let bytes: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(dir.join(format!("{inst:?}-{name}.f32")), bytes).unwrap();
        }
    }
}

/// A second-order Butterworth section, low- or high-pass at `fc` with quality `q`.
fn biquad(x: &[f64], fc: f64, q: f64, high: bool) -> Vec<f64> {
    let w = std::f64::consts::TAU * fc / SR as f64;
    let (sn, cs) = math::sin_cos(w);
    let alpha = sn / (2.0 * q);
    let a0 = 1.0 + alpha;
    let (b0, b1, b2) = if high {
        ((1.0 + cs) / 2.0, -(1.0 + cs), (1.0 + cs) / 2.0)
    } else {
        ((1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0)
    };
    let (a1, a2) = (-2.0 * cs / a0, (1.0 - alpha) / a0);
    let (b0, b1, b2) = (b0 / a0, b1 / a0, b2 / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    x.iter()
        .map(|&v| {
            let y = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            (x2, x1, y2, y1) = (x1, v, y1, y);
            y
        })
        .collect()
}

/// The high band `strokes.py` reads (4–12 kHz, fourth order at each edge).
pub(super) fn high_band(x: &[f32]) -> Vec<f64> {
    let mut y: Vec<f64> = x.iter().map(|v| *v as f64).collect();
    for (q, high, fc) in [
        (0.5412, true, 4000.0),
        (1.3066, true, 4000.0),
        (0.5412, false, 12_000.0),
        (1.3066, false, 12_000.0),
    ] {
        y = biquad(&y, fc, q, high);
    }
    y
}

/// RMS in dB over `win` s frames hopped by `hop` s: (centre s, dB).
pub(super) fn level_db(x: &[f64], win: f64, hop: f64) -> Vec<(f64, f64)> {
    let sr = SR as f64;
    let (w, h) = ((win * sr) as usize, ((hop * sr) as usize).max(1));
    let mut c = vec![0.0f64; x.len() + 1];
    for (i, v) in x.iter().enumerate() {
        c[i + 1] = c[i] + v * v;
    }
    let mut out = Vec::new();
    let mut a = 0;
    while a + w <= x.len() {
        let e = (c[a + w] - c[a]) / w as f64;
        out.push((
            (a as f64 + w as f64 / 2.0) / sr,
            10.0 * math::log10(e + 1e-20),
        ));
        a += h;
    }
    out
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        f64::NAN
    } else {
        v[v.len() / 2]
    }
}

/// `strokes.py`'s tick at a note boundary `tb` (s): the high band's loudest 3 ms within
/// 15 ms over the louder of its medians 60–20 ms before and 25–65 ms after, dB.
pub(super) fn tick_at(hb: &[(f64, f64)], tb: f64) -> f64 {
    let pick = |a: f64, b: f64| -> Vec<f64> {
        hb.iter()
            .filter(|f| f.0 >= tb + a && f.0 <= tb + b)
            .map(|f| f.1)
            .collect()
    };
    let near = pick(-0.015, 0.015).into_iter().fold(f64::MIN, f64::max);
    near - median(pick(-0.060, -0.020)).max(median(pick(0.025, 0.065)))
}

/// `strokes.py`'s `tick_parts` at a boundary `tb`: the high band's loudest 3 ms within 15 ms
/// over its median 60–20 ms before and 25–65 ms after (dB), and how long it stays within
/// 6 dB of that frame (ms: a click is a few, a bite tens).
pub(super) fn tick_parts(hb: &[(f64, f64)], tb: f64) -> (f64, f64, f64) {
    let pick = |a: f64, b: f64| -> Vec<f64> {
        hb.iter()
            .filter(|f| f.0 >= tb + a && f.0 <= tb + b)
            .map(|f| f.1)
            .collect()
    };
    let (mut k, mut p) = (0usize, f64::MIN);
    for (i, f) in hb.iter().enumerate() {
        if f.0 >= tb - 0.015 && f.0 <= tb + 0.015 && f.1 > p {
            (k, p) = (i, f.1);
        }
    }
    let (mut a, mut b) = (k, k);
    while a > 0 && hb[a - 1].1 > p - 6.0 {
        a -= 1;
    }
    while b + 1 < hb.len() && hb[b + 1].1 > p - 6.0 {
        b += 1;
    }
    (
        p - median(pick(-0.060, -0.020)),
        p - median(pick(0.025, 0.065)),
        1000.0 * (hb[b].0 - hb[a].0) + 1.0,
    )
}

/// `strokes.py`'s bounce for a stroke starting at `a` s and lasting to `b`: its fall from
/// its peak (the loudest 5 ms frame within 60 ms of `a`) at 40, 80, 150 and 250 ms, and the
/// gap (the peak over the least level before `b`), dB.
pub(super) fn bounce_at(lv: &[(f64, f64)], a: f64, b: f64) -> ([f64; 4], f64) {
    let span: Vec<&(f64, f64)> = lv.iter().filter(|f| f.0 >= a && f.0 < b).collect();
    let (mut k, mut p) = (0usize, f64::MIN);
    for (i, f) in span.iter().enumerate() {
        if f.0 <= a + 0.06 && f.1 > p {
            (k, p) = (i, f.1);
        }
    }
    let tk = span.get(k).map_or(a, |f| f.0);
    let at = |ms: f64| -> f64 {
        span.iter()
            .find(|f| f.0 >= tk + 0.001 * ms)
            .map_or(f64::NAN, |f| p - f.1)
    };
    let gap = p - span[k..].iter().map(|f| f.1).fold(f64::MAX, f64::min);
    ([at(40.0), at(80.0), at(150.0), at(250.0)], gap)
}

/// The bow changes' ticks of `score`'s notes (every note-on after the first), dB.
pub(super) fn ticks_of(x: &[f32], score: &[(f64, Ev)]) -> Vec<f64> {
    parts_of(x, score).iter().map(|p| p.1).collect()
}

/// The bow changes' `tick_parts` of `score`'s notes (every note-on after the first).
pub(super) fn parts_of(x: &[f32], score: &[(f64, Ev)]) -> Vec<(f64, f64, f64)> {
    let hb = level_db(&high_band(x), 0.003, 0.001);
    score
        .iter()
        .filter(|e| matches!(e.1, Ev::On(..)))
        .skip(1)
        .map(|e| tick_parts(&hb, e.0))
        .collect()
}

/// A probe (ignored): every instrument's bow-change ticks and spiccato bounce, as
/// `strokes.py` measures them, at the notes' own times.
#[test]
#[ignore]
fn probe_strokes() {
    for inst in Instrument::ALL {
        let mut line = format!("{inst:?}:");
        for (name, score, secs) in stroke_phrases(inst) {
            let s = ArcusSettings {
                instrument: inst,
                ..ArcusSettings::default()
            };
            let x = play(&s, &score, secs);
            if name == "spiccato" {
                let lv = level_db(
                    &x.iter().map(|v| *v as f64).collect::<Vec<_>>(),
                    0.005,
                    0.001,
                );
                let ons: Vec<f64> = score
                    .iter()
                    .filter(|e| matches!(e.1, Ev::On(..)))
                    .map(|e| e.0)
                    .collect();
                let mut falls = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                let mut gaps = Vec::new();
                for (i, a) in ons.iter().enumerate() {
                    let b = ons.get(i + 1).copied().unwrap_or(a + 0.405);
                    let (f, g) = bounce_at(&lv, *a, b);
                    for j in 0..4 {
                        falls[j].push(f[j]);
                    }
                    gaps.push(g);
                }
                line += &format!(
                    " spiccato fall {:.1}/{:.1}/{:.1}/{:.1} gap {:.1} dB |",
                    median(falls[0].clone()),
                    median(falls[1].clone()),
                    median(falls[2].clone()),
                    median(falls[3].clone()),
                    median(gaps)
                );
            } else {
                let t = ticks_of(&x, &score);
                let mut s = t.clone();
                s.sort_by(f64::total_cmp);
                line += &format!(
                    " {name} ticks median {:+.1} p90 {:+.1} max {:+.1} |",
                    median(t),
                    s[(0.9 * (s.len() - 1) as f64).round() as usize],
                    s[s.len() - 1]
                );
            }
        }
        println!("{line}");
    }
}

/// One instrument's strokes as `strokes.py` reads them: the détaché's and the re-bow's ticks
/// (dB) and the spiccato's falls at 40/80/150/250 ms and gaps (dB), per stroke.
pub(super) struct Strokes {
    pub detache: Vec<f64>,
    /// The détaché's and the re-bow's ticks' widths, ms.
    pub widths: Vec<f64>,
    pub rebow: Vec<f64>,
    pub falls: [Vec<f64>; 4],
    pub gaps: Vec<f64>,
}

pub(super) fn strokes_of(s: &ArcusSettings) -> Strokes {
    let mut out = Strokes {
        detache: Vec::new(),
        widths: Vec::new(),
        rebow: Vec::new(),
        falls: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        gaps: Vec::new(),
    };
    for (name, score, secs) in stroke_phrases(s.instrument) {
        let x = play(s, &score, secs);
        match name {
            "spiccato" => {
                let lv = level_db(
                    &x.iter().map(|v| *v as f64).collect::<Vec<_>>(),
                    0.005,
                    0.001,
                );
                let ons: Vec<f64> = score
                    .iter()
                    .filter(|e| matches!(e.1, Ev::On(..)))
                    .map(|e| e.0)
                    .collect();
                for (i, a) in ons.iter().enumerate() {
                    let b = ons.get(i + 1).copied().unwrap_or(a + 0.405);
                    let (f, g) = bounce_at(&lv, *a, b);
                    for j in 0..4 {
                        out.falls[j].push(f[j]);
                    }
                    out.gaps.push(g);
                }
            }
            "detache" => {
                let p = parts_of(&x, &score);
                out.widths.extend(p.iter().map(|p| p.2));
                out.detache = p.iter().map(|p| p.1).collect();
            }
            _ => {
                let p = parts_of(&x, &score);
                out.widths.extend(p.iter().map(|p| p.2));
                out.rebow = p.iter().map(|p| p.1).collect();
            }
        }
    }
    out
}

/// A probe (ignored): every fault of pass 5 measured in one run (the before and after of
/// `docs/round42/arcus-match.md`, *Pass 5*): the tremolo's pitch over every string, the
/// violin's top at ff, the bow changes' ticks, the spiccato's bounce.
#[test]
#[ignore]
fn probe_report() {
    for inst in Instrument::ALL {
        let spec = inst.spec();
        let mut all = Vec::new();
        let mut worst = (0f64, String::new());
        for st in spec.strings {
            for key in [st.open, st.open + 5] {
                for slurred in [false, true] {
                    let (c, _) = tremolo_cents(&plain(inst), key, 0.6, slurred);
                    all.push(c.abs());
                    if c.abs() > worst.0 || c.is_nan() {
                        worst = (
                            c.abs(),
                            format!("{key}{} {c:+.1}", if slurred { "s" } else { "" }),
                        );
                    }
                }
            }
        }
        println!(
            "{inst:?} tremolo: mean |c| {:.1}, worst {} ct",
            all.iter().sum::<f64>() / all.len() as f64,
            worst.1
        );
        let st = strokes_of(&ArcusSettings {
            instrument: inst,
            ..ArcusSettings::default()
        });
        let q = |v: &Vec<f64>| {
            let (m, _, hi) = spread(v);
            let mx = v.iter().cloned().fold(f64::MIN, f64::max);
            format!("median {m:+.1} p90 {hi:+.1} max {mx:+.1}")
        };
        println!(
            "{inst:?} détaché ticks {} | re-bow ticks {}",
            q(&st.detache),
            q(&st.rebow)
        );
        println!(
            "{inst:?} spiccato falls {:.1}/{:.1}/{:.1}/{:.1} dB, gap {:.1} dB",
            spread(&st.falls[0]).0,
            spread(&st.falls[1]).0,
            spread(&st.falls[2]).0,
            spread(&st.falls[3]).0,
            spread(&st.gaps).0
        );
    }
    let mut worst = (0f64, 0u8);
    for key in 76u8..=93 {
        for len in [0.5, 1.5] {
            let (m, ..) = held_cents(&violin_cantabile(true), key, 1.0, len);
            if m.abs() > worst.0 {
                worst = (m.abs(), key);
            }
        }
    }
    println!(
        "violin E5–A6 at ff: worst {:.1} ct (key {})",
        worst.0, worst.1
    );
}

/// A probe (ignored): the bow change scanned — `ARCUS_CHANGE=floor:accel×;…` (the force's
/// share left at the reversal, the re-bow's acceleration over the attack's).
#[test]
#[ignore]
fn probe_change_scan() {
    let knobs: Vec<(f32, f32)> = std::env::var("ARCUS_CHANGE")
        .unwrap_or_else(|_| "0.5:2".into())
        .split(';')
        .filter_map(|k| {
            let p: Vec<f32> = k.split(':').filter_map(|x| x.parse().ok()).collect();
            (p.len() == 2).then(|| (p[0], p[1]))
        })
        .collect();
    if let Some(f) = std::env::var("ARCUS_FINGER")
        .ok()
        .and_then(|f| f.parse().ok())
    {
        voice::REBOW_FINGER.with(|c| c.set(f));
    }
    for k in knobs {
        voice::CHANGE_KNOBS.with(|c| c.set(k));
        let mut line = format!("{k:?}:");
        for inst in Instrument::ALL {
            let st = strokes_of(&ArcusSettings {
                instrument: inst,
                oversample: std::env::var("ARCUS_OS").ok().and_then(|o| o.parse().ok()),
                body_mix: if std::env::var_os("ARCUS_NOBODY").is_some() {
                    0.0
                } else {
                    1.0
                },
                bow_noise_db: if std::env::var_os("ARCUS_NONOISE").is_some() {
                    -60.0
                } else {
                    0.0
                },
                ..ArcusSettings::default()
            });
            let w: Vec<f64> = st.widths.clone();
            let (m, _, hi) = spread(&st.detache);
            let (rm, _, rhi) = spread(&st.rebow);
            line += &format!(
                " {inst:?} det {m:+.1}/{hi:+.1} re {rm:+.1}/{rhi:+.1} w {:.0} |",
                spread(&w).0
            );
        }
        println!("{line}");
    }
}

/// A probe (ignored): one plain re-bow on the cello's G3, ms by ms around it — the high
/// band's level, the bow's speed and force and its phase. `ARCUS_TREM`'s form picks another.
#[test]
#[ignore]
fn probe_rebow_trace() {
    let (inst, key, vel) = std::env::var("ARCUS_TREM")
        .ok()
        .and_then(|v| {
            let p: Vec<&str> = v.split(':').collect();
            Some((
                Instrument::ALL[p.first()?.parse::<usize>().ok()?],
                p.get(1)?.parse::<u8>().ok()?,
                p.get(2)?.parse::<f32>().ok()?,
            ))
        })
        .unwrap_or((Instrument::Cello, 55, 0.7));
    let mut a = Arcus::new(SR);
    let to: u8 = std::env::var("ARCUS_TO")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(key);
    if inst == Instrument::Bass && std::env::var_os("ARCUS_CANT").is_some() {
        a.set(&bass_cantabile());
    } else {
        a.set(&ArcusSettings {
            instrument: inst,
            ..ArcusSettings::default()
        });
    }
    a.controller(0, 0.45);
    a.note_on(key, vel, 1);
    let frames = (0.8 * SR) as usize;
    let t_re = (0.5 * SR) as usize;
    let hop = (0.001 * SR) as usize;
    let mut out = vec![0.0f32; frames];
    let mut rows = Vec::new();
    let mut at = 0;
    let mut r = vec![0.0f32; hop];
    while at < frames {
        if at == t_re {
            a.note_off(key, 1);
            a.note_on(to, vel, 2);
        }
        a.process(&mut out[at..at + hop], &mut r);
        r.fill(0.0);
        let v = &a.voices[a.voices.iter().position(|v| v.active()).unwrap_or(0)];
        let (lo, hi) = v.limits();
        rows.push((
            at,
            v.bow_velocity(),
            v.bow_force() / lo.max(1e-6),
            v.bow_force() / hi.max(1e-6),
            v.phase(),
        ));
        at += hop;
    }
    let hb = high_band(&out);
    for (i, (at, vb, f, fh, ph)) in rows.iter().enumerate() {
        if *at + 10 * hop < t_re || *at > t_re + 40 * hop {
            continue;
        }
        let e: f64 = hb[*at..*at + hop].iter().map(|x| x * x).sum::<f64>() / hop as f64;
        let l: f64 = out[*at..*at + hop]
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            / hop as f64;
        println!(
            "{:+3} ms  hf {:6.1} dB  all {:6.1} dB  v {:+.3}  F/lo {:.2} F/hi {:.2}  {:?}",
            i as i64 - (t_re / hop) as i64,
            10.0 * math::log10(e + 1e-20),
            10.0 * math::log10(l + 1e-20),
            vb,
            f,
            fh,
            ph
        );
    }
}

/// The bass's cantabile preset as the engine reads it (`bow.pos` 1.07 → −0.195, the vibrato
/// 1.25 × from 200 ms, `bow.attack` 1.4).
pub(super) fn bass_cantabile() -> ArcusSettings {
    ArcusSettings {
        instrument: Instrument::Bass,
        vib_depth: 1.25,
        vib_delay_ms: 200.0,
        bow_pos: -0.195,
        bow_attack: 1.4,
        ..ArcusSettings::default()
    }
}

/// The demo's bass in section III (`parts.rs`): D2 G2 F#2 E2 A1 A1 D2 C#2 B1 A1 G1 A1, each
/// its own bow, CC1 0.38 → 0.48.
pub(super) const BASS_III: [(u8, f64, bool, f32); 12] = [
    (38, 4.0, false, 0.55),
    (43, 3.0, false, 0.55),
    (42, 1.0, false, 0.55),
    (40, 2.0, false, 0.55),
    (33, 2.0, false, 0.55),
    (33, 4.0, false, 0.55),
    (38, 2.0, false, 0.55),
    (37, 2.0, false, 0.55),
    (35, 2.0, false, 0.55),
    (33, 2.0, false, 0.55),
    (31, 2.0, false, 0.55),
    (33, 2.0, false, 0.55),
];

/// A probe (ignored): the demo's bass line (section III) — each bow change's tick.
#[test]
#[ignore]
fn probe_bass_line() {
    if let Some(f) = std::env::var("ARCUS_FINGER")
        .ok()
        .and_then(|f| f.parse().ok())
    {
        voice::REBOW_FINGER.with(|c| c.set(f));
    }
    if let Some(k) = std::env::var("ARCUS_CHANGE").ok().and_then(|k| {
        let p: Vec<f32> = k.split(':').filter_map(|x| x.parse().ok()).collect();
        (p.len() == 2).then(|| (p[0], p[1]))
    }) {
        voice::CHANGE_KNOBS.with(|c| c.set(k));
    }
    let (score, spans) = phrase_score(&BASS_III, &[(0.0, 0.38), (22.0, 0.48)], 74.0);
    let end = spans.last().unwrap().2 + 0.5;
    let x = play(&bass_cantabile(), &score, end);
    let p = parts_of(&x, &score);
    for (s, q) in spans.iter().skip(1).zip(&p) {
        println!(
            "key {} at {:.2}s: over pre {:+.1} over post {:+.1} width {:.0}",
            s.0, s.1, q.0, q.1, q.2
        );
    }
    let post: Vec<f64> = p.iter().map(|q| q.1).collect();
    println!("median over post {:+.1}", spread(&post).0);
    if let Ok(d) = std::env::var("ARCUS_DUMP") {
        let bytes: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(std::path::Path::new(&d).join("bass_line.f32"), bytes).unwrap();
    }
}

/// A probe (ignored): the spiccato scanned — `ARCUS_SPIC=stroke s:ring ×:force ×;…`.
#[test]
#[ignore]
fn probe_spiccato_scan() {
    let knobs: Vec<(f32, f32, f32)> = std::env::var("ARCUS_SPIC")
        .unwrap_or_else(|_| "0.07:1:1.3".into())
        .split(';')
        .filter_map(|k| {
            let p: Vec<f32> = k.split(':').filter_map(|x| x.parse().ok()).collect();
            (p.len() == 3).then(|| (p[0], p[1], p[2]))
        })
        .collect();
    for k in knobs {
        voice::SPIC_KNOBS.with(|c| c.set(k));
        let mut line = format!("{k:?}:");
        for inst in Instrument::ALL {
            let st = strokes_of(&ArcusSettings {
                instrument: inst,
                ..ArcusSettings::default()
            });
            line += &format!(
                " {inst:?} {:.1}/{:.1}/{:.1}/{:.1} gap {:.1} |",
                spread(&st.falls[0]).0,
                spread(&st.falls[1]).0,
                spread(&st.falls[2]).0,
                spread(&st.falls[3]).0,
                spread(&st.gaps).0
            );
        }
        println!("{line}");
    }
}

/// A probe (ignored): each instrument's tremolo against its sustained note at the same
/// velocity, by RMS over 0.4–1.9 s (dB), at three registers.
#[test]
#[ignore]
fn probe_tremolo_level() {
    let rms = |x: &[f32]| {
        let a = (0.4 * SR) as usize;
        let b = (1.9 * SR) as usize;
        let e: f64 = x[a..b].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / (b - a) as f64;
        10.0 * math::log10(e + 1e-20)
    };
    if let Some(k) = std::env::var("ARCUS_KNOB").ok().and_then(|k| {
        let p: Vec<f32> = k.split(':').filter_map(|x| x.parse().ok()).collect();
        (p.len() == 4).then(|| (p[0], p[1], p[2], p[3]))
    }) {
        voice::TREM_KNOBS.with(|c| c.set(k));
    }
    for inst in Instrument::ALL {
        let low = inst.spec().bottom();
        let mut d = Vec::new();
        for k in [low + 7, low + 19, low + 31] {
            for vel in [0.4f32, 0.8] {
                let s = plain(inst);
                let arco = play(&s, &[(0.01, Ev::On(k, vel)), (2.0, Ev::Off(k))], 2.0);
                let trem = play(
                    &s,
                    &[
                        (0.01, Ev::Art(k, Some(Art::Tremolo), None)),
                        (0.01, Ev::On(k, vel)),
                        (2.0, Ev::Off(k)),
                    ],
                    2.0,
                );
                d.push(rms(&trem) - rms(&arco));
            }
        }
        println!(
            "{inst:?}: tremolo − arco {:?} dB, median {:.1}",
            d.iter()
                .map(|x| (x * 10.0).round() / 10.0)
                .collect::<Vec<_>>(),
            spread(&d).0
        );
    }
}

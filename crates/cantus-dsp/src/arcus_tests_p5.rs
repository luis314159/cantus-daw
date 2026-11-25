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
    let sr = SR as f64;
    let p = sr / want;
    let w = (3.0 * p).ceil() as usize;
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
            let (med, q) = tremolo_cents(&s, key, 0.6, false);
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

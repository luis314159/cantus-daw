//! [`apply`]: what each edit does to a buffer (ADR 0096 §3), with joins made by
//! [`super::join`].

use super::join::{at, blend, snap, Joins};
use super::{silence, AudioBuf, Edit, EditError, Edited, Level, Note, Sel, MAX_REPAIR_MS};
use cantus_dsp::denoise::{self, declick, Region};
use cantus_dsp::math;

/// Apply `edit` to `sel` of `buf` with the default joins (zero crossings, 3 ms).
pub fn apply(buf: &AudioBuf, sel: Sel, edit: &Edit) -> Result<Edited, EditError> {
    apply_with(buf, sel, edit, &Joins::default())
}

fn need(sel: Sel) -> Result<(), EditError> {
    if sel.is_empty() {
        Err(EditError::NeedsSelection)
    } else {
        Ok(())
    }
}

/// The result, refused when nothing would be left.
fn finish(
    buf: &AudioBuf,
    out: AudioBuf,
    sel: Sel,
    used: Sel,
    note: Option<Note>,
) -> Result<Edited, EditError> {
    if out.frames() == 0 {
        return Err(EditError::WouldBeEmpty);
    }
    Ok(Edited {
        changed: out != *buf,
        delta_frames: out.frames() as i64 - buf.frames() as i64,
        buf: out,
        sel,
        used,
        note,
    })
}

/// Apply `edit` to `sel` of `buf`. `sel` is clamped; an empty one is refused where the edit
/// needs a span; `joins` says how pieces meet.
pub fn apply_with(
    buf: &AudioBuf,
    sel: Sel,
    edit: &Edit,
    joins: &Joins,
) -> Result<Edited, EditError> {
    let frames = buf.frames();
    let sel = sel.clamp(frames);
    let n = buf.channels.max(1) as usize;
    let xf = joins.frames(buf.rate);
    let zero = |_: usize, _: usize| 0.0f32;
    // A span's edges at zero crossings (a span that would close up keeps its own).
    let snapped = |s: Sel| -> Sel {
        let t = Sel::new(snap(buf, s.start, joins.snap), snap(buf, s.end, joins.snap));
        if t.is_empty() {
            s
        } else {
            t
        }
    };
    let in_place = |s: Sel, f: &mut dyn FnMut(&mut [f32])| -> Result<Edited, EditError> {
        need(s)?;
        let mut out = buf.clone();
        f(&mut out.data[s.start * n..s.end * n]);
        finish(buf, out, s, s, None)
    };
    match edit {
        Edit::Delete => {
            need(sel)?;
            let s = snapped(sel);
            if s.len() >= frames {
                return Err(EditError::WouldBeEmpty);
            }
            let (a, b) = (s.start, s.end);
            let mut data = buf.data[..a * n].to_vec();
            data.extend_from_slice(&buf.data[b * n..]);
            if a > 0 && b < frames {
                let len = xf.min(frames - b);
                blend(
                    &mut data,
                    n,
                    a,
                    len,
                    |i, c| at(buf, a + i, c),
                    |i, c| at(buf, b + i, c),
                );
            } else if a == 0 {
                let len = xf.min(frames - b);
                blend(&mut data, n, 0, len, zero, |i, c| at(buf, b + i, c));
            } else {
                let len = xf.min(a);
                blend(
                    &mut data,
                    n,
                    a - len,
                    len,
                    |i, c| at(buf, a - len + i, c),
                    zero,
                );
            }
            let out = AudioBuf::new(buf.rate, buf.channels, data);
            finish(buf, out, Sel::new(a, a), s, None)
        }
        Edit::Trim => {
            need(sel)?;
            let s = snapped(sel);
            let mut out = buf.slice(s);
            let len = xf.min(s.len() / 2);
            if s.start > 0 {
                blend(&mut out.data, n, 0, len, zero, |i, c| {
                    at(buf, s.start + i, c)
                });
            }
            if s.end < frames {
                let from = s.len() - len;
                blend(
                    &mut out.data,
                    n,
                    from,
                    len,
                    |i, c| at(buf, s.end - len + i, c),
                    zero,
                );
            }
            finish(buf, out, Sel::whole(s.len()), s, None)
        }
        Edit::Paste { clip } => {
            if clip.rate != buf.rate || clip.channels != buf.channels {
                return Err(EditError::Mismatch);
            }
            let m = clip.frames();
            if m == 0 {
                return Err(EditError::Invalid("clipboard"));
            }
            let s = if sel.is_empty() {
                let p = snap(buf, sel.start, joins.snap);
                Sel::new(p, p)
            } else {
                snapped(sel)
            };
            let (a, b) = (s.start, s.end);
            let mut data = buf.data[..a * n].to_vec();
            data.extend_from_slice(&clip.data);
            data.extend_from_slice(&buf.data[b * n..]);
            if a > 0 {
                let len = xf.min(m / 2).min(frames - a);
                blend(
                    &mut data,
                    n,
                    a,
                    len,
                    |i, c| at(buf, a + i, c),
                    |i, c| at(clip, i, c),
                );
            }
            if b < frames {
                let len = xf.min(m / 2).min(b);
                blend(
                    &mut data,
                    n,
                    a + m - len,
                    len,
                    |i, c| at(clip, m - len + i, c),
                    |i, c| at(buf, b - len + i, c),
                );
            }
            let out = AudioBuf::new(buf.rate, buf.channels, data);
            finish(buf, out, Sel::new(a, a + m), s, None)
        }
        Edit::InsertSilence { frames: k } => {
            let k = *k;
            if k == 0 {
                return Err(EditError::Invalid("length"));
            }
            let p = snap(buf, sel.start, joins.snap);
            let mut data = buf.data[..p * n].to_vec();
            data.resize(data.len() + k * n, 0.0);
            data.extend_from_slice(&buf.data[p * n..]);
            if p > 0 {
                let len = xf.min(k / 2).min(frames - p);
                blend(&mut data, n, p, len, |i, c| at(buf, p + i, c), zero);
            }
            if p < frames {
                let len = xf.min(k / 2).min(p);
                blend(&mut data, n, p + k - len, len, zero, |i, c| {
                    at(buf, p - len + i, c)
                });
            }
            let out = AudioBuf::new(buf.rate, buf.channels, data);
            finish(buf, out, Sel::new(p, p + k), Sel::new(p, p), None)
        }
        Edit::Silence => {
            need(sel)?;
            let s = snapped(sel);
            let mut out = buf.clone();
            out.data[s.start * n..s.end * n].fill(0.0);
            let len = xf.min(s.len() / 2);
            if s.start > 0 {
                blend(
                    &mut out.data,
                    n,
                    s.start,
                    len,
                    |i, c| at(buf, s.start + i, c),
                    zero,
                );
            }
            if s.end < frames {
                let from = s.end - len;
                blend(&mut out.data, n, from, len, zero, |i, c| {
                    at(buf, from + i, c)
                });
            }
            finish(buf, out, s, s, None)
        }
        Edit::FadeIn { curve } => {
            let s = snapped(sel);
            let len = s.len();
            in_place(s, &mut |d| {
                for (i, f) in d.chunks_exact_mut(n).enumerate() {
                    let g = curve.gain(i as f32 / len as f32);
                    f.iter_mut().for_each(|v| *v *= g);
                }
            })
        }
        Edit::FadeOut { curve } => {
            let s = snapped(sel);
            let len = s.len();
            in_place(s, &mut |d| {
                for (i, f) in d.chunks_exact_mut(n).enumerate() {
                    let g = curve.gain((len - 1 - i) as f32 / len as f32);
                    f.iter_mut().for_each(|v| *v *= g);
                }
            })
        }
        Edit::Gain { db } => {
            if !db.is_finite() || db.abs() > 96.0 {
                return Err(EditError::Invalid("gain"));
            }
            need(sel)?;
            if *db == 0.0 {
                return finish(buf, buf.clone(), sel, sel, None);
            }
            let g = math::db_to_gain(*db);
            in_place(snapped(sel), &mut |d| d.iter_mut().for_each(|v| *v *= g))
        }
        Edit::Normalize {
            level,
            target_db,
            ceiling_db,
        } => normalize(buf, snapped(sel), *level, *target_db, *ceiling_db),
        Edit::Invert => in_place(snapped(sel), &mut |d| d.iter_mut().for_each(|v| *v = -*v)),
        Edit::Reverse => {
            need(sel)?;
            let s = snapped(sel);
            let mut out = buf.clone();
            {
                let d = &mut out.data[s.start * n..s.end * n];
                let k = d.len() / n;
                for i in 0..k / 2 {
                    for c in 0..n {
                        d.swap(i * n + c, (k - 1 - i) * n + c);
                    }
                }
            }
            // The reversed audio meets what is around it through a crossfade too.
            let rev = out.clone();
            let len = xf.min(s.len() / 4);
            if s.start > 0 {
                blend(
                    &mut out.data,
                    n,
                    s.start,
                    len,
                    |i, c| at(buf, s.start + i, c),
                    |i, c| at(&rev, s.start + i, c),
                );
            }
            if s.end < frames {
                let from = s.end - len;
                blend(
                    &mut out.data,
                    n,
                    from,
                    len,
                    |i, c| at(&rev, from + i, c),
                    |i, c| at(buf, from + i, c),
                );
            }
            finish(buf, out, s, s, None)
        }
        Edit::DcRemove => in_place(sel, &mut |d| {
            let k = (d.len() / n) as f64;
            for c in 0..n {
                let mean = d.iter().skip(c).step_by(n).map(|v| *v as f64).sum::<f64>() / k;
                let mean = mean as f32;
                d.iter_mut().skip(c).step_by(n).for_each(|v| *v -= mean);
            }
        }),
        Edit::RemoveSilence(settings) => {
            let s = if sel.is_empty() {
                Sel::whole(frames)
            } else {
                sel
            };
            let out = silence::remove(buf, s, settings, xf).map_err(EditError::Invalid)?;
            finish(buf, out, Sel::new(s.start, s.start), s, None)
        }
        Edit::Denoise { profile, params } => {
            let s = if sel.is_empty() {
                Sel::whole(frames)
            } else {
                sel
            };
            if profile.power.len() != n || profile.rate != buf.rate {
                return Err(EditError::Mismatch);
            }
            params
                .check()
                .map_err(|e| EditError::Denoise(e.to_string()))?;
            if params.amount <= 0.0 {
                return finish(buf, buf.clone(), s, s, None);
            }
            denoise_part(buf, s, profile, params)
        }
        Edit::Attenuate {
            lo_hz,
            hi_hz,
            gain_db,
        } => {
            need(sel)?;
            let nyq = buf.rate as f32 / 2.0;
            if !(lo_hz.is_finite() && hi_hz.is_finite() && gain_db.is_finite())
                || lo_hz >= hi_hz
                || *lo_hz < 0.0
                || *hi_hz > nyq
                || gain_db.abs() > 96.0
            {
                return Err(EditError::Invalid("region"));
            }
            if *gain_db == 0.0 {
                return finish(buf, buf.clone(), sel, sel, None);
            }
            attenuate(buf, sel, *lo_hz, *hi_hz, *gain_db)
        }
        Edit::Declick { sensitivity } => {
            if !(sensitivity.is_finite() && (0.0..=1.0).contains(sensitivity)) {
                return Err(EditError::Invalid("sensitivity"));
            }
            let s = if sel.is_empty() {
                Sel::whole(frames)
            } else {
                sel
            };
            let mut planar = buf.planar();
            let mut count = 0;
            for ch in &mut planar {
                count = count.max(declick::declick(ch, s.start, s.end, *sensitivity));
            }
            let out = AudioBuf::from_planar(buf.rate, &planar);
            finish(buf, out, s, s, Some(Note::Clicks { count }))
        }
        Edit::Repair => {
            need(sel)?;
            let max = (buf.rate as f32 * MAX_REPAIR_MS / 1000.0) as usize;
            if sel.len() > max {
                return Err(EditError::TooLong);
            }
            let mut planar = buf.planar();
            for ch in &mut planar {
                declick::interpolate(ch, sel.start, sel.end).map_err(|_| EditError::NoContext)?;
            }
            let out = AudioBuf::from_planar(buf.rate, &planar);
            finish(buf, out, sel, sel, None)
        }
    }
}

fn normalize(
    buf: &AudioBuf,
    s: Sel,
    level: Level,
    target_db: f32,
    ceiling_db: Option<f32>,
) -> Result<Edited, EditError> {
    need(s)?;
    let n = buf.channels.max(1) as usize;
    if !target_db.is_finite() || target_db > 24.0 || ceiling_db.is_some_and(|c| !c.is_finite()) {
        return Err(EditError::Invalid("target"));
    }
    let db_of = |x: f32| (x > 0.0).then(|| 20.0 * math::log10(x));
    let now = match level {
        Level::Peak => db_of(buf.peak(s)),
        Level::Rms => db_of(buf.rms(s)),
        Level::Lufs => {
            let v = buf.lufs(s);
            if v.is_none() && (s.len() as f32) < buf.rate as f32 * 0.4 {
                return Err(EditError::TooShort);
            }
            v
        }
    };
    // Silence has no level to bring anywhere.
    let Some(now) = now else {
        return finish(buf, buf.clone(), s, s, None);
    };
    let mut db = target_db - now;
    let mut note = None;
    let peak = buf.peak(s);
    if level != Level::Peak && peak > 0.0 {
        let peak_db = 20.0 * math::log10(peak);
        match ceiling_db {
            Some(c) if peak_db + db > c => {
                db = c - peak_db;
                note = Some(Note::Ceiling {
                    reached_db: now + db,
                });
            }
            None if peak_db + db > 0.0 => {
                note = Some(Note::Clips {
                    peak_db: peak_db + db,
                });
            }
            _ => {}
        }
    }
    if db.abs() < 1e-4 {
        return finish(buf, buf.clone(), s, s, note);
    }
    let g = math::db_to_gain(db);
    let mut out = buf.clone();
    out.data[s.start * n..s.end * n]
        .iter_mut()
        .for_each(|v| *v *= g);
    finish(buf, out, s, s, note)
}

/// Noise reduction of `s`: the transform runs over the selection and some audio either side
/// (so its frames at the edges see real audio, not silence), and the cleaned selection
/// meets the untouched audio through 10 ms linear crossfades inside it — linear, because
/// the two sides are the same audio less its noise (correlated), where linear keeps the
/// level.
fn denoise_part(
    buf: &AudioBuf,
    s: Sel,
    profile: &cantus_dsp::denoise::NoiseProfile,
    params: &cantus_dsp::denoise::Params,
) -> Result<Edited, EditError> {
    use denoise::stft::N;
    let frames = buf.frames();
    let n = buf.channels.max(1) as usize;
    let (lo, hi) = (s.start.saturating_sub(2 * N), (s.end + 2 * N).min(frames));
    let part = buf.slice(Sel::new(lo, hi));
    let cleaned = denoise::denoise(&part.planar(), profile, params)
        .map_err(|e| EditError::Denoise(e.to_string()))?;
    let cleaned = AudioBuf::from_planar(buf.rate, &cleaned);
    let x = ((buf.rate as f32 * 0.010) as usize).min(s.len() / 2).max(1);
    let mut out = buf.clone();
    for f in s.start..s.end {
        let w = if s.start > 0 && f - s.start < x {
            (f - s.start) as f32 / x as f32
        } else if s.end < frames && s.end - f <= x {
            (s.end - f - 1) as f32 / x as f32
        } else {
            1.0
        };
        for c in 0..n {
            let o = at(buf, f, c);
            let d = at(&cleaned, f - lo, c);
            out.data[f * n + c] = o + (d - o) * w;
        }
    }
    finish(buf, out, s, s, None)
}

/// A rectangle of the spectrogram turned up or down. Only the samples the region can reach
/// — frames centred up to one hop past it (its soft edge), each `N/2` either side of its
/// centre — are rewritten; every other sample keeps its bits.
fn attenuate(
    buf: &AudioBuf,
    s: Sel,
    lo_hz: f32,
    hi_hz: f32,
    gain_db: f32,
) -> Result<Edited, EditError> {
    use denoise::stft::{HOP, N};
    let frames = buf.frames();
    let n = buf.channels.max(1) as usize;
    let lo = s.start.saturating_sub(2 * N);
    let hi = (s.end + 2 * N).min(frames);
    let region = Region {
        start: s.start - lo,
        end: s.end - lo,
        lo_hz,
        hi_hz,
    };
    let reach = N / 2 + HOP;
    let (from, to) = (s.start.saturating_sub(reach), (s.end + reach).min(frames));
    let mut out = buf.clone();
    for c in 0..n {
        let ch = buf.channel(c);
        let done = denoise::attenuate_region(&ch[lo..hi], buf.rate, &region, gain_db);
        for f in from..to {
            out.data[f * n + c] = done[f - lo];
        }
    }
    finish(buf, out, s, s, None)
}

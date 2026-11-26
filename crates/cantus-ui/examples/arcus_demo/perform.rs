//! The player (r42 arcus-expr, stage 1): a part as written turned into a part as performed,
//! with only what a user can enter in Cantus — each note's articulation and bow, its
//! velocity (the attack), its start and length, a per-note bend (MPE's: the intonation),
//! and the clip's controller lanes: CC1 the dynamic (the phrase), CC11 the expression (each
//! bow's shape: swells, fp, sforzando, portato, the release), CC21 the vibrato's depth (its
//! onset, its width by place and role) and CC65 the slides. The tempo track carries the
//! rubato (`rubato`).
//!
//! What it does and the measures behind it (`docs/round42/arcus-expr.md`, the study):
//!
//! - **Bowing.** One bow per slur; a phrase starts down-bow on a strong beat and up-bow on a
//!   pickup, then the bows alternate; a written `d`/`u` wins.
//! - **Attacks by context.** A phrase's first note starts from the string, softly (a low
//!   velocity: a slow, light attack), unless it is marked; a bow change inside a phrase is a
//!   détaché by its beat's weight; `>` an accent, `m` martelé (a staccato's bite), `z`
//!   sforzando and `f` fp (a hard attack, then the expression falls away).
//! - **Dynamics.** The written CC1 plus each phrase's arc (rising to its highest note,
//!   relaxing to its cadence) and the part's place in the quartet (the melody over the inner
//!   parts: the HSD quartet's first violin sits 2–4 dB over the second); CC11 shapes each bow
//!   — a long one swells to its middle and eases (the HSD stems: long notes peak at 0.55 of
//!   their length, 12 dB over their start and 14 over their end), a phrase's last bow dies
//!   away rather than stopping, a portato pulses.
//! - **Vibrato.** Continuous through a legato line (the HSD stems: its onset inside a phrase is
//!   0 s at the median), delayed on a long note that starts a phrase (the single notes: a
//!   quarter in at 0.2–0.25 s), none on short détaché and spiccato notes or open strings, wider
//!   at the climax and on the melody, narrower in the coda, narrowing as a release dies.
//! - **Timing.** Every start a few ms off the grid, seeded per player (never sloppy: under 10
//!   ms; the HSD stems, over a guide track, differ more); legato changes tighter.
//! - **Intonation.** A note leading up a semitone a little high (+6 cents), one falling a
//!   semitone a little low (−5), on the melodic parts only; slides (`^`) on chosen shifts.

use super::score::{Part, Who, Written, BAR};
use cantus_core::note_art::{Articulation, Bow, NoteArt};

/// CC11's resting value: under 1 so an accent and a swell's top have room.
pub const CC11_REST: f32 = 0.85;

/// A part's place in the texture.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Melody,
    Counter,
    Inner,
    Bass,
}

impl Role {
    /// CC1 over the written dynamic.
    fn lift(self) -> f32 {
        match self {
            Role::Melody => 0.06,
            Role::Counter => 0.0,
            Role::Inner => -0.03,
            Role::Bass => -0.02,
        }
    }

    /// The phrase's arc, ± on CC1.
    fn arc(self) -> f32 {
        match self {
            Role::Melody | Role::Counter => 0.05,
            Role::Inner | Role::Bass => 0.025,
        }
    }

    /// The vibrato's depth over the preset's (CC21 0.5).
    fn vib(self) -> f32 {
        match self {
            Role::Melody => 1.1,
            Role::Counter => 1.0,
            Role::Inner => 0.85,
            Role::Bass => 0.9,
        }
    }

    /// The timing's spread, ms (σ).
    fn sigma_ms(self) -> f64 {
        match self {
            Role::Melody => 3.0,
            _ => 4.5,
        }
    }
}

/// What the section asks of everyone.
#[derive(Copy, Clone, Debug)]
pub struct Ctx {
    /// The section's tempo, for milliseconds in beats.
    pub bpm: f64,
    /// The climax: wider vibrato, firmer bows.
    pub climax: bool,
    /// The coda: narrower vibrato.
    pub intimate: bool,
    pub seed: u64,
}

/// A note as performed (beats from the clip's start).
#[derive(Clone, Debug)]
pub struct PNote {
    pub at: f64,
    pub len: f64,
    pub key: u8,
    pub vel: u8,
    pub art: Option<NoteArt>,
    /// The intonation, semitones (0: none).
    pub bend: f64,
}

pub struct Performance {
    pub notes: Vec<PNote>,
    pub cc1: Vec<(f64, f32)>,
    pub cc11: Vec<(f64, f32)>,
    pub cc21: Vec<(f64, f32)>,
    /// Empty when nothing slides.
    pub cc65: Vec<(f64, f32)>,
}

/// The open strings' keys.
fn open(who: Who) -> [u8; 4] {
    match who {
        Who::Violin => [55, 62, 69, 76],
        Who::Viola => [48, 55, 62, 69],
        Who::Cello => [36, 43, 50, 57],
        Who::Bass => [28, 33, 38, 43],
    }
}

/// A seeded number in [−1, 1) (splitmix64).
fn unit(seed: u64, i: u64) -> f64 {
    let mut z = seed
        .wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// A seeded normal number (the sum of four uniforms, scaled to unit variance).
fn normal(seed: u64, i: u64) -> f64 {
    let s: f64 = (0..4).map(|k| unit(seed, 4 * i + k)).sum();
    s * (3.0f64 / 4.0).sqrt()
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A beat's weight in 4/4: 1 the downbeat, 0.6 the third beat, 0.3 the others, 0 between.
fn weight(at: f64) -> f64 {
    let b = at.rem_euclid(BAR);
    let r = b.round();
    if (b - r).abs() > 1e-6 {
        0.0
    } else if r == 0.0 || r == BAR {
        1.0
    } else if r == 2.0 {
        0.6
    } else {
        0.3
    }
}

fn interp(points: &[(f64, f32)], t: f64) -> f32 {
    match points {
        [] => 0.5,
        [(_, v)] => *v,
        _ => {
            if t <= points[0].0 {
                return points[0].1;
            }
            for w in points.windows(2) {
                let ((a, va), (b, vb)) = (w[0], w[1]);
                if t <= b {
                    let u = if b > a { (t - a) / (b - a) } else { 1.0 };
                    return va + (vb - va) * u as f32;
                }
            }
            points[points.len() - 1].1
        }
    }
}

/// A phrase: its notes `[first, last]`, its peak note.
#[derive(Copy, Clone, Debug)]
pub struct Phrase {
    pub first: usize,
    pub last: usize,
    pub peak: usize,
}

/// The phrases of written notes: a rest of a quarter beat or more, or a breath, ends one.
pub fn phrases(w: &[Written]) -> Vec<Phrase> {
    let mut out = Vec::new();
    let mut first = 0;
    for i in 0..w.len() {
        let end =
            i + 1 == w.len() || w[i].breath || w[i + 1].at - (w[i].at + w[i].len) >= 0.25 - 1e-9;
        if end {
            let peak = (first..=i)
                .max_by(|&a, &b| {
                    (w[a].key, (w[a].len * 1000.0) as i64, std::cmp::Reverse(a)).cmp(&(
                        w[b].key,
                        (w[b].len * 1000.0) as i64,
                        std::cmp::Reverse(b),
                    ))
                })
                .unwrap_or(first);
            out.push(Phrase {
                first,
                last: i,
                peak,
            });
            first = i + 1;
        }
    }
    out
}

/// The bows: `(first, last)` note of each — a slur's notes, or one note.
fn bows(w: &[Written]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in 0..w.len() {
        match out.last_mut() {
            Some(g) if w[i].slur != 0 && w[g.0].slur == w[i].slur => g.1 = i,
            _ => out.push((i, i)),
        }
    }
    out
}

fn written_art(w: &Written) -> Option<Articulation> {
    w.art.map(|a| a.articulation)
}

fn is_short_stroke(a: Option<Articulation>) -> bool {
    matches!(
        a,
        Some(Articulation::Staccato | Articulation::Spiccato | Articulation::Pizzicato)
    )
}

/// The part performed. `written` is `score::parse`'s, `beats` the clip's length.
pub fn perform(part: &Part, written: &[Written], beats: f64, role: Role, ctx: &Ctx) -> Performance {
    let w = written;
    let n = w.len();
    let sec = |b: f64| b * 60.0 / ctx.bpm;
    let beat = |s: f64| s * ctx.bpm / 60.0;
    let ph = phrases(w);
    let phrase_of: Vec<usize> = {
        let mut v = vec![0; n];
        for (k, p) in ph.iter().enumerate() {
            for x in v.iter_mut().take(p.last + 1).skip(p.first) {
                *x = k;
            }
        }
        v
    };
    let gs = bows(w);
    let bow_of: Vec<usize> = {
        let mut v = vec![0; n];
        for (k, g) in gs.iter().enumerate() {
            for x in v.iter_mut().take(g.1 + 1).skip(g.0) {
                *x = k;
            }
        }
        v
    };

    // ---- the bows' directions ----
    let mut dir = vec![None; gs.len()];
    let mut next = Bow::Down;
    for (k, g) in gs.iter().enumerate() {
        let f = &w[g.0];
        if written_art(f) == Some(Articulation::Pizzicato) {
            continue;
        }
        let starts_phrase = ph[phrase_of[g.0]].first == g.0;
        if starts_phrase {
            next = if weight(f.at) >= 0.6 {
                Bow::Down
            } else {
                Bow::Up
            };
        }
        let d = if f.flags.contains('d') {
            Bow::Down
        } else if f.flags.contains('u') {
            Bow::Up
        } else {
            next
        };
        dir[k] = Some(d);
        next = match d {
            Bow::Down => Bow::Up,
            Bow::Up => Bow::Down,
        };
    }

    // ---- the dynamic the player is at, before CC11 (for the attacks and the vibrato) ----
    let arc = |t: f64| -> f32 {
        let Some(p) = ph
            .iter()
            .find(|p| t >= w[p.first].at - 1e-9 && t <= w[p.last].at + w[p.last].len + 1e-9)
        else {
            return 0.0;
        };
        let (s, e) = (w[p.first].at, w[p.last].at + w[p.last].len);
        let k = w[p.peak].at + 0.4 * w[p.peak].len;
        let a = role.arc() as f64;
        if e - s < 1.5 {
            return 0.0;
        }
        let v = if t <= k {
            -a + 2.0 * a * smooth((t - s) / (k - s).max(1e-6))
        } else {
            a - 2.2 * a * smooth((t - k) / (e - k).max(1e-6))
        };
        v as f32
    };
    let lift = role.lift() + if ctx.climax { 0.03 } else { 0.0 };
    let dyn_at = |t: f64| (interp(part.cc1, t) + lift + arc(t)).clamp(0.04, 1.0);

    // ---- the notes: timing, attack, articulation, intonation ----
    let seed = ctx.seed;
    let mut notes: Vec<PNote> = Vec::with_capacity(n);
    for i in 0..n {
        let x = &w[i];
        let g = gs[bow_of[i]];
        let slurred = g.0 != i;
        let a = written_art(x);
        // Timing: a few ms, tighter on a finger change inside a slur.
        let sigma = if slurred { 2.0 } else { role.sigma_ms() };
        let off_ms = (normal(seed, i as u64) * sigma).clamp(-2.5 * sigma, 2.5 * sigma);
        let mut at = (x.at + beat(off_ms * 0.001)).max(0.0);
        if let Some(prev) = notes.last() {
            at = at.max(prev.at + 0.05);
        }
        // The attack.
        let p = ph[phrase_of[i]];
        let from_silence = p.first == i;
        let d = dyn_at(x.at);
        let wgt = weight(x.at);
        let mut vel = if a == Some(Articulation::Pizzicato) {
            f64::from(x.vel) + 8.0 * wgt - 3.0
        } else if matches!(a, Some(Articulation::Spiccato | Articulation::Staccato)) {
            58.0 + 14.0 * wgt + 0.5 * (f64::from(x.vel) - 70.0)
        } else if a == Some(Articulation::Tremolo) {
            f64::from(x.vel)
        } else if slurred {
            50.0
        } else if from_silence {
            40.0 + 14.0 * f64::from(d)
        } else {
            58.0 + 10.0 * wgt + 10.0 * f64::from(d)
        };
        if x.flags.contains('>') {
            vel = vel.max(96.0);
        }
        if x.flags.contains('m') {
            vel = vel.max(108.0);
        }
        if x.flags.contains('f') {
            vel = vel.max(108.0);
        }
        if x.flags.contains('z') {
            vel = vel.max(116.0);
        }
        // The articulation and the bow.
        let bow = dir[bow_of[i]];
        let art = match a {
            Some(Articulation::Pizzicato) => Some(NoteArt {
                articulation: Articulation::Pizzicato,
                bow: None,
                start: None,
                part: None,
                string: None,
            }),
            _ if x.flags.contains('m') && !slurred => Some(NoteArt {
                articulation: Articulation::Staccato,
                bow,
                start: None,
                part: None,
                string: None,
            }),
            Some(other) => Some(NoteArt {
                articulation: other,
                bow,
                start: None,
                part: None,
                string: None,
            }),
            None if slurred => Some(NoteArt {
                articulation: Articulation::Legato,
                bow,
                start: None,
                part: None,
                string: None,
            }),
            None => Some(NoteArt {
                articulation: Articulation::Sustain,
                bow,
                start: None,
                part: None,
                string: None,
            }),
        };
        // Intonation: tendency tones on the melodic parts.
        let mut bend = 0.0;
        if matches!(role, Role::Melody | Role::Counter)
            && !is_short_stroke(a)
            && x.len >= 0.25
            && i + 1 < n
            && phrase_of[i + 1] == phrase_of[i]
        {
            match i32::from(w[i + 1].key) - i32::from(x.key) {
                1 => bend = 0.06,
                -1 => bend = -0.05,
                _ => {}
            }
        }
        notes.push(PNote {
            at,
            len: x.len,
            key: x.key,
            vel: vel.round().clamp(20.0, 127.0) as u8,
            art,
            bend,
        });
    }
    // Lengths: a slur's notes meet; a bow change meets the next note (never overlapping it,
    // which would slur); a phrase's last note lifts a little early when a rest follows.
    for i in 0..n {
        let end = w[i].at + w[i].len;
        let mut e = end;
        if i + 1 < n {
            let nx = notes[i + 1].at;
            let same_bow = bow_of[i + 1] == bow_of[i];
            if same_bow || (w[i + 1].at - end).abs() < 1e-6 {
                e = nx;
            } else {
                e = e.min(nx);
            }
        }
        let last_of_phrase = ph[phrase_of[i]].last == i;
        let rest_after = i + 1 == n || w[i + 1].at - end >= 0.25 - 1e-9;
        if last_of_phrase && rest_after && !is_short_stroke(written_art(&w[i])) {
            e -= (0.06 * w[i].len).min(0.2);
        }
        notes[i].len = (e - notes[i].at).max(0.05);
    }

    // ---- CC1: the written dynamic, the phrase, the part's place ----
    let mut times: Vec<f64> = Vec::new();
    let mut t = 0.0;
    while t <= beats + 1e-9 {
        times.push(t);
        t += 0.5;
    }
    for p in &ph {
        times.push(w[p.first].at);
        times.push(w[p.peak].at + 0.4 * w[p.peak].len);
        times.push(w[p.last].at + w[p.last].len);
    }
    times.extend(part.cc1.iter().map(|(b, _)| *b));
    let cc1 = sample(&mut times.clone(), beats, |t| {
        (dyn_at(t) / CC11_REST).clamp(0.04, 1.0)
    });

    // ---- CC11: each bow's shape ----
    let bow_shape = |t: f64| -> f32 {
        let Some(k) = gs
            .iter()
            .position(|g| t >= notes[g.0].at - 1e-9 && t < notes[g.1].at + notes[g.1].len)
        else {
            return CC11_REST;
        };
        let g = gs[k];
        let (s, e) = (notes[g.0].at, notes[g.1].at + notes[g.1].len);
        let a = written_art(&w[g.0]);
        let f = &w[g.0];
        let u = t - s;
        let len_s = sec(e - s);
        let mut v = CC11_REST as f64;
        if a == Some(Articulation::Pizzicato)
            || (!f.flags.contains('f') && !f.flags.contains('z'))
                && (a == Some(Articulation::Tremolo) || is_short_stroke(a))
        {
            return CC11_REST;
        }
        if f.flags.contains('f') {
            // fp: the attack at its force, then piano at once.
            let k = (sec(u) / 0.14).clamp(0.0, 1.0);
            return (1.0 - (1.0 - 0.55) * smooth(k)) as f32;
        }
        if f.flags.contains('z') {
            let su = sec(u);
            return if su < 0.15 {
                1.0
            } else {
                (1.0 - (1.0 - CC11_REST as f64) * smooth((su - 0.15) / 0.35)) as f32
            };
        }
        // A long bow swells to its middle and eases (messa di voce).
        if len_s >= 0.9 {
            // Measured against the HSD stems (`arcus-expr.md` §2): at 0.24 / 0.15 the long
            // notes started 4 dB and ended 3 under their peak (the players 12 and 14).
            let m = match role {
                Role::Melody | Role::Counter => 0.45,
                _ => 0.30,
            } * (len_s / 2.5).min(1.0);
            let x = u / (e - s);
            let peak = 0.55;
            v = if x < peak {
                CC11_REST as f64 * (1.0 - m + (m + 0.6 * m) * smooth(x / peak))
            } else {
                CC11_REST as f64 * (1.0 + 0.6 * m - 1.4 * m * smooth((x - peak) / (1.0 - peak)))
            };
        }
        // A phrase's last bow dies away.
        let last = gs[k].1;
        if ph[phrase_of[last]].last == last {
            let lx = notes[last].at;
            let ll = notes[last].len;
            let k0 = lx + 0.45 * ll;
            if t > k0 {
                v *= 1.0 - 0.6 * smooth((t - k0) / (lx + ll - k0));
            }
        }
        // Portato: a pulse at the note's start inside the slur.
        for i in g.0..=g.1 {
            if i != g.0 && w[i].flags.contains('-') {
                let su = sec(t - notes[i].at);
                if (-0.03..0.12).contains(&su) {
                    let dip = if su < 0.0 {
                        1.0 - 0.28 * smooth((su + 0.03) / 0.03)
                    } else {
                        1.0 - 0.28 * (1.0 - smooth(su / 0.12))
                    };
                    v *= dip;
                }
            }
        }
        v.min(1.0) as f32
    };
    let mut times11: Vec<f64> = Vec::new();
    let mut t = 0.0;
    while t <= beats + 1e-9 {
        times11.push(t);
        t += beat(0.05);
    }
    for (i, x) in notes.iter().enumerate() {
        times11.extend([x.at - 0.002, x.at, x.at + x.len - 0.002]);
        if w[i].flags.contains('-') || w[i].flags.contains('f') || w[i].flags.contains('z') {
            for ms in [
                -30.0, 0.0, 20.0, 40.0, 60.0, 90.0, 120.0, 150.0, 200.0, 300.0, 500.0,
            ] {
                times11.push(x.at + beat(ms * 0.001));
            }
        }
    }
    let cc11 = sample(&mut times11, beats, bow_shape);

    // ---- CC21: the vibrato ----
    let opens = open(part.who);
    let target: Vec<f64> = (0..n)
        .map(|i| {
            let x = &w[i];
            let a = written_art(x);
            let len_s = sec(x.len);
            let g = gs[bow_of[i]];
            let in_slur = g.0 != g.1;
            let mut v = 0.5 * role.vib() as f64;
            if ctx.climax {
                v *= 1.2;
            }
            if ctx.intimate {
                v *= 0.85;
            }
            if x.flags.contains('q') {
                return 0.0;
            }
            match a {
                Some(Articulation::Pizzicato) => {
                    return if len_s >= 0.7 { 0.25 * v } else { 0.0 };
                }
                Some(Articulation::Spiccato | Articulation::Staccato) => return 0.0,
                Some(Articulation::Tremolo) => return 0.6 * v,
                _ => {}
            }
            // Short notes: carried through a slur, none in a détaché passage.
            if len_s < 0.25 {
                v *= if in_slur { 0.7 } else { 0.0 };
            } else if len_s < 0.5 {
                v *= 0.85;
            }
            // An open string has none (a long expressive note is stopped, a fourth finger).
            if opens.contains(&x.key) && len_s < 0.9 {
                v = 0.0;
            }
            v
        })
        .collect();
    let vib = |t: f64| -> f32 {
        let Some(i) = (0..n).rev().find(|&i| t >= notes[i].at - 1e-9) else {
            return 0.0;
        };
        let x = &notes[i];
        let su = sec(t - x.at);
        let v = target[i];
        let p = ph[phrase_of[i]];
        let mut out = if p.first == i && sec(x.len) >= 0.6 {
            // A long first note: plain, then the vibrato grows in.
            let delay = 0.12 + if dyn_at(w[i].at) < 0.35 { 0.1 } else { 0.0 };
            v * (0.2 + 0.8 * smooth((su - delay) / 0.3))
        } else if i > 0 && phrase_of[i - 1] == phrase_of[i] {
            // Continuous: from the last note's to this one's over 60 ms.
            let k = smooth(su / 0.06);
            target[i - 1] + (v - target[i - 1]) * k
        } else {
            v
        };
        // After the note (a rest): it falls away.
        if t > x.at + x.len {
            out *= 1.0 - smooth(sec(t - x.at - x.len) / 0.2);
        } else if p.last == i && sec(x.len) >= 0.6 {
            // A dying release narrows.
            let k0 = 0.5 * x.len;
            if t - x.at > k0 {
                out *= 1.0 - 0.35 * smooth((t - x.at - k0) / (x.len - k0));
            }
        }
        out.clamp(0.0, 1.0) as f32
    };
    let mut times21: Vec<f64> = Vec::new();
    let mut t = 0.0;
    while t <= beats + 1e-9 {
        times21.push(t);
        t += beat(0.05);
    }
    for x in &notes {
        for ms in [-5.0, 0.0, 30.0, 60.0] {
            times21.push(x.at + beat(ms * 0.001));
        }
        times21.push(x.at + x.len);
    }
    let cc21 = sample(&mut times21, beats, vib);

    // ---- CC65: the slides ----
    let mut cc65 = Vec::new();
    let step = 10.0 / 960.0;
    for (i, x) in notes.iter().enumerate() {
        if w[i].flags.contains('^') && i > 0 && bow_of[i] == bow_of[i - 1] {
            if cc65.is_empty() {
                cc65.push((0.0, 0.0));
            }
            cc65.extend([
                (x.at - 4.0 * step, 0.0),
                (x.at - 3.0 * step, 1.0),
                (x.at + 2.0 * step, 1.0),
                (x.at + 3.0 * step, 0.0),
            ]);
        }
    }

    Performance {
        notes,
        cc1,
        cc11,
        cc21,
        cc65,
    }
}

/// `f` at the times (sorted, 1/96 beat apart at least, inside the clip).
fn sample(times: &mut Vec<f64>, beats: f64, f: impl Fn(f64) -> f32) -> Vec<(f64, f32)> {
    times.retain(|t| (0.0..=beats).contains(t));
    times.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, f32)> = Vec::new();
    for &t in times.iter() {
        // On the lane's tick grid (the build rounds to ticks): one point per tick.
        let t = (t * 960.0).round() / 960.0;
        if out.last().is_some_and(|(p, _)| t - p < 1.0 / 96.0 - 1e-9) {
            continue;
        }
        out.push((t, f(t)));
    }
    out
}

/// The rubato a melody's phrases ask of the tempo track: a peak note held a little (−3.5 %
/// over its first half, back by its end) and each phrase's end eased (−6 % over its last
/// beat and a half, a tempo at the next start). `phrases` are `(start, peak start, peak
/// end, end, next start)` in beats from the song's start; `base` the tempo there; `keep`
/// the spans (beats) the written tempo track already shapes, left alone.
pub fn rubato(
    phrases: &[(f64, f64, f64, f64, f64)],
    base: impl Fn(f64) -> f64,
    keep: &[(f64, f64)],
) -> Vec<(f64, f64, bool)> {
    let free = |a: f64, b: f64| keep.iter().all(|(x, y)| b <= *x || a >= *y);
    let mut out: Vec<(f64, f64, bool)> = Vec::new();
    for &(s, pk_s, pk_e, e, next) in phrases {
        if e - s < 3.0 {
            continue;
        }
        let ends_at = e - 1.5;
        if pk_e < ends_at - 0.5 && pk_s > s + 0.5 && free(pk_s - 0.5, pk_e) {
            let b = base(pk_s);
            out.push((pk_s - 0.5, b, true));
            out.push((pk_s + 0.5 * (pk_e - pk_s), b * 0.965, true));
            out.push((pk_e, b, false));
        }
        let back = next.max(e + 0.25);
        if free(ends_at, back) {
            let b = base(ends_at);
            out.push((ends_at, b, true));
            out.push((e, b * 0.94, false));
            out.push((back, b, false));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut kept: Vec<(f64, f64, bool)> = Vec::new();
    for p in out {
        if kept.last().is_some_and(|l| p.0 - l.0 < 0.25) {
            continue;
        }
        kept.push(p);
    }
    kept
}

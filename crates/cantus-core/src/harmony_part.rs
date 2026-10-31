//! Parts laid over a chart: block chords, pulses, arpeggios and bass lines, on a rate or a
//! step pattern, with swing and a seeded humanisation — the comping, bass and arpeggio
//! writers the demo songs used, as one function.
//!
//! **No note rings into the next chord** (round 28's review 1): every note ends where its
//! chord ends, unless the next chord holds the same pitch and `tie_common` asks the block
//! pattern to tie it over (a common tone held, as a pianist would).

use super::chart::Chart;
use crate::tempo::TempoMap;
use crate::time::Ticks;

/// How the voices are played.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// The whole voicing, held for each chord (the rhythm is ignored).
    Block,
    /// The whole voicing on every hit of the rhythm.
    Pulse,
    /// One voice per hit, cycling through the voicing.
    Arpeggio {
        direction: ArpDirection,
        octaves: u8,
    },
    /// The voicing's lowest pitch (its bass) on each hit, in a style.
    Bass(BassStyle),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArpDirection {
    Up,
    Down,
    UpDown,
    /// Drawn from the seed: the same seed plays the same order.
    Random,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BassStyle {
    /// The bass note on every hit.
    Root,
    /// Root, fifth, root, fifth…
    RootFifth,
    /// Root and its octave above, alternating.
    Octave,
    /// Root, fifth, octave, fifth.
    RootFifthOctave,
}

/// One step of a step pattern.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// A hit; `true` is accented.
    Hit(bool),
    /// The note before goes on through this step.
    Tie,
    Rest,
}

/// When the part plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rhythm {
    /// A hit every so many ticks on the song's grid, and on every chord's start.
    Every(Ticks),
    /// A step pattern (`x.x-X...`), each step `step` long, repeated without a break from
    /// the chart's start — a pattern longer than a bar runs on across the bar lines.
    Steps { steps: Vec<Step>, step: Ticks },
}

impl Rhythm {
    /// Read a step pattern: `x` hit, `X` accent, `-` or `_` tie, `.` rest; spaces and `|`
    /// are ignored. `None` for another character, an empty pattern or one of rests only.
    pub fn parse_steps(text: &str, step: Ticks) -> Option<Rhythm> {
        let mut steps = Vec::new();
        for c in text.chars() {
            steps.push(match c {
                'x' | 'o' => Step::Hit(false),
                'X' => Step::Hit(true),
                '-' | '_' => Step::Tie,
                '.' => Step::Rest,
                ' ' | '|' => continue,
                _ => return None,
            });
        }
        if steps.iter().any(|s| matches!(s, Step::Hit(_))) && step.0 > 0 && steps.len() <= 256 {
            Some(Rhythm::Steps { steps, step })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PartSpec {
    pub pattern: Pattern,
    pub rhythm: Rhythm,
    /// The share of each hit's time a note sounds, 0.05…1 (1 is legato).
    pub gate: f32,
    pub velocity: u8,
    /// Added to accented hits and to each bar's downbeat.
    pub accent: u8,
    /// 0 straight … 1 a full triplet swing, of the off-beats of `swing_grid`.
    pub swing: f32,
    pub swing_grid: Ticks,
    /// Timing and velocity drawn within ± these, from `seed` (0: none).
    pub humanize_ticks: i64,
    pub humanize_velocity: u8,
    pub seed: u64,
    /// The block pattern ties a pitch the next chord's voicing also holds.
    pub tie_common: bool,
}

/// One note of a laid part.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PartNote {
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: u8,
    pub velocity: u8,
}

/// A small, fixed hash: the humanisation's and the random arpeggio's numbers.
fn hash(seed: u64, n: u64) -> u64 {
    let mut x = seed ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A number in `-r..=r` from the hash.
fn spread(seed: u64, n: u64, r: i64) -> i64 {
    if r <= 0 {
        return 0;
    }
    (hash(seed, n) % (2 * r as u64 + 1)) as i64 - r
}

/// The hits inside `[from, to)`: start, how long until the next hit (ties included), and
/// whether accented.
fn hits(rhythm: &Rhythm, origin: Ticks, from: Ticks, to: Ticks) -> Vec<(Ticks, Ticks, bool)> {
    let mut out: Vec<(Ticks, Ticks, bool)> = Vec::new();
    match rhythm {
        Rhythm::Every(step) => {
            let s = step.0.max(1);
            let mut t = from.0.div_euclid(s) * s;
            if t < from.0 {
                t += s;
            }
            if t != from.0 {
                out.push((from, Ticks((t - from.0).min(to.0 - from.0)), false));
            }
            while t < to.0 {
                out.push((Ticks(t), Ticks(s.min(to.0 - t)), false));
                t += s;
            }
        }
        Rhythm::Steps { steps, step } => {
            // Counted on from the chart's start without a break (review #9): a pattern
            // longer than a bar runs across the bar line as written.
            let s = step.0.max(1);
            let mut k = (from.0 - origin.0).div_euclid(s).max(0) as usize;
            loop {
                let t = origin.0 + s * k as i64;
                if t >= to.0 {
                    break;
                }
                if t >= from.0 {
                    match steps[k % steps.len()] {
                        Step::Hit(acc) => out.push((Ticks(t), Ticks(s), acc)),
                        Step::Tie => {
                            if let Some(last) = out.last_mut() {
                                last.1 = Ticks(t + s - last.0 .0);
                            }
                        }
                        Step::Rest => {}
                    }
                }
                k += 1;
            }
            for h in &mut out {
                h.1 = Ticks(h.1 .0.min(to.0 - h.0 .0));
            }
        }
    }
    out
}

/// Lay a part: `voicings[i]` (ascending pitches) plays over `chart.chords[i]`.
pub fn lay_part(
    chart: &Chart,
    voicings: &[Vec<u8>],
    map: &TempoMap,
    spec: &PartSpec,
) -> Vec<PartNote> {
    let mut out: Vec<PartNote> = Vec::new();
    let gate = if spec.gate.is_finite() {
        spec.gate.clamp(0.05, 1.0)
    } else {
        1.0
    };
    let mut counter: u64 = 0;
    let mut arp_index: usize = 0;
    let mut tied: Vec<(u8, usize)> = Vec::new(); // (pitch, index in `out`) held over
    let origin = chart.span().map_or(Ticks::ZERO, |(s, _)| s);
    for (ci, c) in chart.chords.iter().enumerate() {
        let voicing = voicings.get(ci).map(Vec::as_slice).unwrap_or(&[]);
        if voicing.is_empty() || c.chord.is_none() {
            tied.clear();
            continue;
        }
        if spec.pattern == Pattern::Block {
            let next = voicings.get(ci + 1).filter(|_| {
                chart
                    .chords
                    .get(ci + 1)
                    .is_some_and(|n| n.start == c.end && n.chord.is_some())
            });
            let mut still: Vec<(u8, usize)> = Vec::new();
            for &p in voicing {
                let i = match tied.iter().find(|(tp, _)| *tp == p) {
                    Some(&(_, i)) => {
                        out[i].length = c.end - out[i].start;
                        i
                    }
                    None => {
                        let len =
                            Ticks(((c.end - c.start).0 as f32 * gate).round() as i64).max(Ticks(1));
                        out.push(PartNote {
                            start: c.start,
                            length: len,
                            pitch: p,
                            velocity: spec.velocity,
                        });
                        out.len() - 1
                    }
                };
                if spec.tie_common && next.is_some_and(|n| n.contains(&p)) {
                    out[i].length = c.end - out[i].start;
                    still.push((p, i));
                }
            }
            tied = still;
            continue;
        }
        tied.clear();
        for (start, span, accented) in hits(&spec.rhythm, origin, c.start, c.end) {
            let downbeat = map.bar_start(start) == start;
            let pitches: Vec<u8> = match spec.pattern {
                Pattern::Block => unreachable!(),
                Pattern::Pulse => voicing.to_vec(),
                Pattern::Arpeggio { direction, octaves } => {
                    let mut pool: Vec<u8> = Vec::new();
                    for o in 0..octaves.clamp(1, 4) {
                        pool.extend(
                            voicing
                                .iter()
                                .filter_map(|p| p.checked_add(12 * o))
                                .filter(|p| *p <= 127),
                        );
                    }
                    let n = pool.len();
                    let i = match direction {
                        ArpDirection::Up => arp_index % n,
                        ArpDirection::Down => n - 1 - arp_index % n,
                        ArpDirection::UpDown => {
                            let period = (2 * n).saturating_sub(2).max(1);
                            let k = arp_index % period;
                            if k < n {
                                k
                            } else {
                                period - k
                            }
                        }
                        ArpDirection::Random => {
                            (hash(spec.seed ^ 0xA5, arp_index as u64) % n as u64) as usize
                        }
                    };
                    arp_index += 1;
                    vec![pool[i]]
                }
                Pattern::Bass(style) => {
                    // The bass voice (the slash's note or the root), and above it the
                    // chord's own fifth — measured from the chord's root, so a ø7's is
                    // flat and an aug's sharp; a chord with none takes its root (or the
                    // octave when the bass is the root).
                    let root = voicing[0];
                    let chord = c.chord.as_ref().expect("a chord");
                    let above = |pc: u8| -> Option<u8> {
                        (root + 1..=root.saturating_add(12)).find(|p| *p <= 127 && p % 12 == pc)
                    };
                    let octave = if root <= 115 { root + 12 } else { root };
                    let fifth = match chord.fifth() {
                        Some(pc) if pc != root % 12 => above(pc),
                        _ if chord.root != root % 12 => above(chord.root),
                        _ => None,
                    }
                    .unwrap_or(octave);
                    let seq: &[u8] = match style {
                        BassStyle::Root => &[root],
                        BassStyle::RootFifth => &[root, fifth],
                        BassStyle::Octave => &[root, octave],
                        BassStyle::RootFifthOctave => &[root, fifth, octave, fifth],
                    };
                    let p = seq[arp_index % seq.len()];
                    arp_index += 1;
                    vec![p]
                }
            };
            // Swing the off-beats of the swing grid; the note still ends by its span.
            let g = spec.swing_grid.0.max(1);
            let mut at = start;
            if spec.swing > 0.0 && start.0 % g == 0 && (start.0 / g) % 2 == 1 {
                let push = (g as f32 * spec.swing.clamp(0.0, 1.0) / 3.0).round() as i64;
                at = Ticks(start.0 + push);
            }
            let end = start + span;
            for p in pitches {
                counter += 1;
                let jitter = spread(spec.seed, counter * 2, spec.humanize_ticks.max(0));
                let s = Ticks((at.0 + jitter).max(c.start.0).min(end.0 - 1));
                let len = Ticks((((end - s).0 as f32) * gate).round() as i64)
                    .max(Ticks(1))
                    .min(c.end - s);
                let mut v = spec.velocity as i64;
                if accented || downbeat {
                    v += spec.accent as i64;
                }
                v += spread(spec.seed, counter * 2 + 1, spec.humanize_velocity as i64);
                out.push(PartNote {
                    start: s,
                    length: len,
                    pitch: p,
                    velocity: v.clamp(1, 127) as u8,
                });
            }
        }
    }
    // A pitch never doubles itself: humanised timing can pull a hit under the note
    // before it, which then ends where the new one starts.
    out.sort_by_key(|n| (n.pitch, n.start));
    let mut kept: Vec<PartNote> = Vec::with_capacity(out.len());
    for n in out {
        if let Some(last) = kept.last_mut() {
            if last.pitch == n.pitch && last.start + last.length > n.start {
                if last.start == n.start {
                    continue;
                }
                last.length = n.start - last.start;
            }
        }
        kept.push(n);
    }
    kept.sort_by_key(|n| (n.start, n.pitch));
    kept
}

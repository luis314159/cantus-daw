//! A chart: chords over the song's bars, in the lead sheet's shorthand.
//!
//! **The bar form** (what a model writes): bars separated by `|`, the chords of a bar by
//! spaces, each taking an equal share of it. `.` holds the chord before it for one more
//! share, `%` repeats the whole bar before, `N.C.` (or `NC`) is a silence of harmony.
//!
//! ```text
//! Fm9 | Dbmaj7 | Abadd9 | Eb Eb/G        four bars, the last split in halves
//! C . . G7 | %                           C for three beats, G7 for one; then again
//! ```
//!
//! Bars are counted through every metre change of the [`TempoMap`]: in 3/4 a bar of three
//! chords is a chord a beat.

use super::chord::{parse_chord, Chord, ChordError};
use crate::tempo::TempoMap;
use crate::time::Ticks;
use std::fmt;

/// The most bars a chart may span, and the most chords it may hold.
pub const MAX_BARS: usize = 1024;
pub const MAX_CHORDS: usize = 4096;

/// One chord of a chart and the span it sounds over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartChord {
    pub start: Ticks,
    pub end: Ticks,
    /// `None`: no chord (`N.C.`).
    pub chord: Option<Chord>,
}

/// Chords in time order, never overlapping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chart {
    pub chords: Vec<ChartChord>,
}

/// Why a chart cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChartError {
    Chord {
        bar: usize,
        error: ChordError,
    },
    Empty,
    /// `.` or `%` with nothing before it to hold or repeat.
    NothingToRepeat {
        bar: usize,
    },
    TooLong,
    /// Explicit chords out of order, overlapping or empty.
    Order {
        index: usize,
    },
}

impl fmt::Display for ChartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChartError::Chord { bar, error } => write!(f, "bar {bar} of the chart: {error}"),
            ChartError::Empty => write!(f, "the chart has no chords"),
            ChartError::NothingToRepeat { bar } => {
                write!(
                    f,
                    "bar {bar} of the chart: '.' or '%' with nothing before it"
                )
            }
            ChartError::TooLong => write!(
                f,
                "the chart is too long (at most {MAX_BARS} bars and {MAX_CHORDS} chords)"
            ),
            ChartError::Order { index } => write!(
                f,
                "chord {index} of the chart starts before the one before it ends, or is empty"
            ),
        }
    }
}

impl std::error::Error for ChartError {}

fn is_no_chord(t: &str) -> bool {
    matches!(
        t.to_ascii_uppercase().as_str(),
        "N.C." | "NC" | "N.C" | "NC."
    )
}

impl Chart {
    /// Read the bar form, its first bar at the 1-based bar `first_bar` of `map`.
    pub fn parse_bars(text: &str, map: &TempoMap, first_bar: i64) -> Result<Chart, ChartError> {
        let bars: Vec<&str> = text
            .split('|')
            .map(str::trim)
            .enumerate()
            // An empty first or last segment is the `|` that opens or closes the chart.
            .filter(|(i, b)| !(b.is_empty() && (*i == 0 || *i == text.split('|').count() - 1)))
            .map(|(_, b)| b)
            .collect();
        if bars.is_empty() || bars.iter().all(|b| b.is_empty()) {
            return Err(ChartError::Empty);
        }
        if bars.len() > MAX_BARS {
            return Err(ChartError::TooLong);
        }
        let mut chords: Vec<ChartChord> = Vec::new();
        let mut previous_bar: Option<Vec<&str>> = None;
        for (i, bar) in bars.iter().enumerate() {
            let number = i + 1;
            let mut tokens: Vec<&str> = bar.split_whitespace().collect();
            if tokens == ["%"] {
                tokens = previous_bar
                    .clone()
                    .ok_or(ChartError::NothingToRepeat { bar: number })?;
            }
            if tokens.is_empty() {
                // An empty bar holds what was sounding.
                tokens = vec!["."];
            }
            let start = map.bar_tick(first_bar + i as i64);
            let end = map.bar_tick(first_bar + i as i64 + 1);
            let share = (end.0 - start.0) / tokens.len() as i64;
            for (k, tok) in tokens.iter().enumerate() {
                let at = Ticks(start.0 + share * k as i64);
                let to = if k + 1 == tokens.len() {
                    end
                } else {
                    Ticks(at.0 + share)
                };
                if *tok == "." || *tok == "%" {
                    let last = chords
                        .last_mut()
                        .ok_or(ChartError::NothingToRepeat { bar: number })?;
                    last.end = to;
                    continue;
                }
                let chord = if is_no_chord(tok) {
                    None
                } else {
                    Some(
                        parse_chord(tok)
                            .map_err(|error| ChartError::Chord { bar: number, error })?,
                    )
                };
                chords.push(ChartChord {
                    start: at,
                    end: to,
                    chord,
                });
                if chords.len() > MAX_CHORDS {
                    return Err(ChartError::TooLong);
                }
            }
            previous_bar = Some(tokens);
        }
        Ok(Chart::merged(chords))
    }

    /// A chart from chords already placed (`start`, `end`, symbol or `None` for no chord).
    pub fn from_spans(spans: Vec<(Ticks, Ticks, Option<&str>)>) -> Result<Chart, ChartError> {
        if spans.is_empty() {
            return Err(ChartError::Empty);
        }
        if spans.len() > MAX_CHORDS {
            return Err(ChartError::TooLong);
        }
        let mut chords = Vec::with_capacity(spans.len());
        for (index, (start, end, sym)) in spans.into_iter().enumerate() {
            if end <= start || chords.last().is_some_and(|c: &ChartChord| c.end > start) {
                return Err(ChartError::Order { index });
            }
            let chord = match sym {
                Some(s) if !is_no_chord(s) => {
                    Some(parse_chord(s).map_err(|error| ChartError::Chord {
                        bar: index + 1,
                        error,
                    })?)
                }
                _ => None,
            };
            chords.push(ChartChord { start, end, chord });
        }
        Ok(Chart { chords })
    }

    /// Neighbours that are the same chord become one (`C C` in a bar is a bar of C).
    fn merged(chords: Vec<ChartChord>) -> Chart {
        let mut out: Vec<ChartChord> = Vec::with_capacity(chords.len());
        for c in chords {
            match out.last_mut() {
                Some(last) if last.end == c.start && last.chord == c.chord => last.end = c.end,
                _ => out.push(c),
            }
        }
        Chart { chords: out }
    }

    /// The chord sounding at `tick` (`None` outside the chart or under `N.C.`).
    pub fn at(&self, tick: Ticks) -> Option<&ChartChord> {
        let i = self.chords.partition_point(|c| c.start <= tick);
        let c = self.chords.get(i.checked_sub(1)?)?;
        (tick < c.end).then_some(c)
    }

    /// Where the chart starts and ends.
    pub fn span(&self) -> Option<(Ticks, Ticks)> {
        Some((self.chords.first()?.start, self.chords.last()?.end))
    }

    /// Every change of chord: the start of each chord after the first, where the chord
    /// before ends there (a gap is not a change).
    pub fn changes(&self) -> impl Iterator<Item = (usize, Ticks)> + '_ {
        self.chords
            .windows(2)
            .enumerate()
            .filter(|(_, w)| w[0].end == w[1].start)
            .map(|(i, w)| (i + 1, w[1].start))
    }
}

//! The desk's geometry (ADR 0024 §6): the width modes, the rows every strip shares so
//! they line up across the desk, and **fit first, then scroll**.
//!
//! Pure arithmetic, no egui: given the height the desk has, which rows are on and how the
//! meter and the fader split the flexible height, [`fit`] says where each row goes. When
//! the desk is short, rows drop in the ADR's order (the EQ bars, the width knob, the
//! phase and swap switches, the readout, then the name shrinks and the insert list
//! shortens); when even the minimum does not fit, the content is taller than the desk
//! and the desk scrolls vertically.

use serde::{Deserialize, Serialize};

/// How wide every strip is (FL's layouts, reduced to three). Ctrl+wheel over the desk
/// steps through them; the choice is kept per window layout.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WidthMode {
    /// 24 pt: FL's Compact (Plex Condensed needs a little more than FL's 20). Names run
    /// up the strip.
    #[default]
    Compact,
    /// 48 pt: names across, the fader's value under it.
    Wide,
    /// 76 pt: the insert list inside the strip, as FL's Extra large.
    Xl,
}

impl WidthMode {
    pub const ALL: [WidthMode; 3] = [WidthMode::Compact, WidthMode::Wide, WidthMode::Xl];

    /// The strip's pitch in points at 100 %: its body and the 1-pt gap after it.
    pub fn pitch(self) -> f32 {
        match self {
            WidthMode::Compact => 24.0,
            WidthMode::Wide => 48.0,
            WidthMode::Xl => 76.0,
        }
    }

    /// Its name (an i18n key with a `.short` form).
    pub fn key(self) -> &'static str {
        match self {
            WidthMode::Compact => "kit.mixer.width.compact",
            WidthMode::Wide => "kit.mixer.width.wide",
            WidthMode::Xl => "kit.mixer.width.xl",
        }
    }

    pub fn index(self) -> usize {
        match self {
            WidthMode::Compact => 0,
            WidthMode::Wide => 1,
            WidthMode::Xl => 2,
        }
    }

    pub fn from_index(i: usize) -> WidthMode {
        WidthMode::ALL[i.min(2)]
    }

    /// One step wider (`+1`) or narrower (`-1`), stopping at the ends as a zoom does.
    pub fn step(self, dir: i32) -> WidthMode {
        let i = self.index() as i32 + dir.signum();
        WidthMode::from_index(i.clamp(0, 2) as usize)
    }

    /// Whether a strip this wide sets its name on its side (ADR 0024 §3: below 36 pt).
    pub fn rotated_names(self) -> bool {
        self.pitch() < 36.0
    }
}

/// FL's *Colourful mixer*: how strongly a whole strip takes its track's colour.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Colourful {
    #[default]
    Off,
    Low,
    Medium,
    High,
}

impl Colourful {
    pub const ALL: [Colourful; 4] = [
        Colourful::Off,
        Colourful::Low,
        Colourful::Medium,
        Colourful::High,
    ];

    /// The tint's alpha at the top of the strip (`fl-studio.md` §12.5).
    pub fn alpha(self) -> f32 {
        match self {
            Colourful::Off => 0.0,
            Colourful::Low => 0.10,
            Colourful::Medium => 0.20,
            Colourful::High => 0.35,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Colourful::Off => "mixer.colourful.off",
            Colourful::Low => "mixer.colourful.low",
            Colourful::Medium => "mixer.colourful.medium",
            Colourful::High => "mixer.colourful.high",
        }
    }
}

/// A row of the strip, top to bottom. Every strip has the same rows at the same height,
/// so a row reads straight across the desk and the legend names it once.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Row {
    Number,
    Name,
    Meter,
    MuteSolo,
    Pan,
    Fader,
    /// The fader's value in dB (Wide and Extra large).
    Readout,
    PhaseSwap,
    Width,
    /// The strip's own three-band EQ, as three bipolar bars.
    Eq,
    FxRec,
    Sends,
    /// The insert slots (Extra large).
    Inserts,
}

/// How many rows there are.
pub const ROWS: usize = 13;

impl Row {
    pub const ALL: [Row; ROWS] = [
        Row::Number,
        Row::Name,
        Row::Meter,
        Row::MuteSolo,
        Row::Pan,
        Row::Fader,
        Row::Readout,
        Row::PhaseSwap,
        Row::Width,
        Row::Eq,
        Row::FxRec,
        Row::Sends,
        Row::Inserts,
    ];

    /// The families the desk's ▾ menu can switch off. The number, the meter and the
    /// fader are the strip; they stay.
    pub const TOGGLES: [Row; 10] = [
        Row::Name,
        Row::MuteSolo,
        Row::Pan,
        Row::Readout,
        Row::PhaseSwap,
        Row::Width,
        Row::Eq,
        Row::FxRec,
        Row::Sends,
        Row::Inserts,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// What the legend column calls it (an i18n key).
    pub fn legend_key(self) -> &'static str {
        match self {
            Row::Number => "mixer.legend.number",
            Row::Name => "mixer.legend.name",
            Row::Meter => "mixer.legend.meter",
            Row::MuteSolo => "mixer.legend.mute",
            Row::Pan => "mixer.legend.pan",
            Row::Fader => "mixer.legend.fader",
            Row::Readout => "mixer.legend.readout",
            Row::PhaseSwap => "mixer.legend.phase",
            Row::Width => "mixer.legend.width",
            Row::Eq => "mixer.legend.eq",
            Row::FxRec => "mixer.legend.fx",
            Row::Sends => "mixer.legend.sends",
            Row::Inserts => "mixer.legend.inserts",
        }
    }

    /// The bit a [`RowSet`] keeps it in.
    fn bit(self) -> u16 {
        1 << self.index()
    }
}

/// Which row families are switched on (the desk's ▾ menu), as bits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RowSet(pub u16);

impl Default for RowSet {
    fn default() -> Self {
        RowSet(u16::MAX)
    }
}

impl RowSet {
    pub fn has(self, r: Row) -> bool {
        self.0 & r.bit() != 0
    }

    pub fn set(&mut self, r: Row, on: bool) {
        if on {
            self.0 |= r.bit();
        } else {
            self.0 &= !r.bit();
        }
    }

    pub fn toggle(&mut self, r: Row) {
        let on = !self.has(r);
        self.set(r, on);
    }
}

/// The gap between rows, in points at 100 %.
pub const GAP: f32 = 3.0;
/// The space below the last row.
pub const FOOT: f32 = 3.0;
/// The meter's shortest; the fader's shortest and tallest (a fader longer than this
/// is more travel than a hand wants: the rest goes to the meter).
pub const METER_MIN: f32 = 48.0;
pub const FADER_MIN: f32 = 80.0;
pub const FADER_MAX: f32 = 440.0;
/// The meter's share of the flexible height by default (the proof's 38 : 62).
pub const DEFAULT_RATIO: f32 = 0.38;
/// Insert slots: one row's height, and the fewest the list shows.
pub const SLOT: f32 = 16.0;
pub const MIN_SLOTS: usize = 3;

/// A rotated name's height, and what it shrinks to when the desk is short.
const NAME_TALL: f32 = 64.0;
const NAME_SHORT: f32 = 40.0;
/// A name set across the strip.
const NAME_ACROSS: f32 = 18.0;

/// The fixed height of a row (the meter and the fader are flexible: 0).
fn fixed(r: Row, rotated: bool, name_short: bool, slots: usize) -> f32 {
    match r {
        Row::Number => 16.0,
        Row::Name if !rotated => NAME_ACROSS,
        Row::Name if name_short => NAME_SHORT,
        Row::Name => NAME_TALL,
        Row::Meter | Row::Fader => 0.0,
        Row::MuteSolo => 14.0,
        Row::Pan => 22.0,
        Row::Readout => 16.0,
        Row::PhaseSwap => 16.0,
        Row::Width => 22.0,
        Row::Eq => 26.0,
        Row::FxRec => 14.0,
        Row::Sends => 18.0,
        Row::Inserts => slots as f32 * SLOT + 4.0,
    }
}

/// Where every row of a strip goes.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Fit {
    /// Each row's `(top, bottom)` from the top of the strips, in points at 100 %; `None`
    /// for a row that is off or dropped.
    pub rows: [Option<(f32, f32)>; ROWS],
    /// The strips' whole height: the desk's height when everything fits, more when the
    /// desk has to scroll.
    pub height: f32,
    /// How many of the drop steps were taken.
    pub dropped: usize,
    /// The desk is shorter than the minimum: it scrolls vertically.
    pub scrolls: bool,
    /// Names run up the strip.
    pub rotated: bool,
    /// Insert slots shown (Extra large).
    pub slots: usize,
}

impl Fit {
    pub fn row(&self, r: Row) -> Option<(f32, f32)> {
        self.rows[r.index()]
    }

    /// The centre line of a row.
    pub fn mid(&self, r: Row) -> Option<f32> {
        self.row(r).map(|(a, b)| (a + b) * 0.5)
    }
}

/// The order rows give way in when the desk is short (ADR 0024 §6): the EQ bars, the
/// width knob, phase and swap, the readout; then the name shrinks and the insert list
/// shortens.
const DROPS: [Drop; 6] = [
    Drop::Off(Row::Eq),
    Drop::Off(Row::Width),
    Drop::Off(Row::PhaseSwap),
    Drop::Off(Row::Readout),
    Drop::NameShort,
    Drop::FewerSlots,
];

#[derive(Copy, Clone)]
enum Drop {
    Off(Row),
    NameShort,
    FewerSlots,
}

/// Lay the rows out in `avail` points of height.
///
/// `mode` decides the rows that exist (the readout from Wide, the inserts at Extra
/// large); `shown` is the ▾ menu's switches; `ratio` is the meter's share of the flexible
/// height (the 3-dot handle); `slots` is how many insert slots the Extra large list wants.
pub fn fit(avail: f32, mode: WidthMode, shown: RowSet, ratio: f32, slots: usize) -> Fit {
    let rotated = mode.rotated_names();
    let mut on = shown;
    // Rows the width mode does not have.
    if mode == WidthMode::Compact {
        on.set(Row::Readout, false);
    }
    if mode != WidthMode::Xl {
        on.set(Row::Inserts, false);
    }
    on.set(Row::Number, true);
    on.set(Row::Meter, true);
    on.set(Row::Fader, true);
    let mut name_short = false;
    let mut slots = slots.max(MIN_SLOTS);
    // Called once a frame: counts in place, allocates nothing.
    let need = |on: RowSet, name_short: bool, slots: usize| -> f32 {
        let (mut sum, mut n) = (0.0f32, 0usize);
        for r in Row::ALL.iter().filter(|r| on.has(**r)) {
            sum += fixed(*r, rotated, name_short, slots);
            n += 1;
        }
        sum + GAP * n.saturating_sub(1) as f32 + FOOT + METER_MIN + FADER_MIN
    };
    let mut dropped = 0;
    while need(on, name_short, slots) > avail && dropped < DROPS.len() {
        match DROPS[dropped] {
            Drop::Off(r) => on.set(r, false),
            Drop::NameShort => name_short = rotated,
            Drop::FewerSlots => slots = MIN_SLOTS,
        }
        dropped += 1;
    }
    let minimum = need(on, name_short, slots);
    let scrolls = minimum > avail + 0.5;
    let height = avail.max(minimum);
    // The flexible height, split: the fader takes its share up to its tallest, the meter
    // the rest.
    let flex = height - (minimum - METER_MIN - FADER_MIN);
    let ratio = ratio.clamp(0.15, 0.75);
    let fader = (flex * (1.0 - ratio)).round().clamp(FADER_MIN, FADER_MAX);
    let fader = fader.min(flex - METER_MIN).max(FADER_MIN);
    let meter = (flex - fader).max(METER_MIN);

    let mut rows = [None; ROWS];
    let mut y = 0.0f32;
    let mut first = true;
    for r in Row::ALL {
        if !on.has(r) {
            continue;
        }
        if !first {
            y += GAP;
        }
        first = false;
        let h = match r {
            Row::Meter => meter,
            Row::Fader => fader,
            _ => fixed(r, rotated, name_short, slots),
        };
        rows[r.index()] = Some((y, y + h));
        y += h;
    }
    Fit {
        rows,
        height: height.max(y + FOOT),
        dropped,
        scrolls,
        rotated,
        slots: if on.has(Row::Inserts) { slots } else { 0 },
    }
}

/// The ratio a drag of the 3-dot handle to `meter_h` points of meter asks for, given the
/// flexible height `flex`.
pub fn ratio_for(meter_h: f32, flex: f32) -> f32 {
    (meter_h / flex.max(1.0)).clamp(0.15, 0.75)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(f: &Fit, r: Row) -> f32 {
        f.row(r).map(|(a, b)| b - a).unwrap_or(0.0)
    }

    #[test]
    fn a_tall_desk_shows_every_row_and_splits_the_rest() {
        let f = fit(
            900.0,
            WidthMode::Compact,
            RowSet::default(),
            DEFAULT_RATIO,
            0,
        );
        assert_eq!(f.dropped, 0);
        assert!(!f.scrolls);
        for r in [Row::Eq, Row::Width, Row::PhaseSwap, Row::Sends, Row::FxRec] {
            assert!(f.row(r).is_some(), "{r:?}");
        }
        // Compact has no readout and no insert list.
        assert!(f.row(Row::Readout).is_none() && f.row(Row::Inserts).is_none());
        assert!(f.rotated);
        assert!(span(&f, Row::Fader) >= FADER_MIN && span(&f, Row::Meter) >= METER_MIN);
        // Rows are in order and never overlap.
        let mut last = -1.0;
        for r in Row::ALL {
            if let Some((a, b)) = f.row(r) {
                assert!(a >= last && b > a, "{r:?}");
                last = b;
            }
        }
        assert!(last <= 900.0);
    }

    #[test]
    fn a_short_desk_drops_rows_in_order_then_scrolls() {
        let full = fit(
            900.0,
            WidthMode::Compact,
            RowSet::default(),
            DEFAULT_RATIO,
            0,
        );
        let need =
            full.height - span(&full, Row::Meter) - span(&full, Row::Fader) + METER_MIN + FADER_MIN;
        // Just short of everything: the EQ goes first.
        let f = fit(
            need - 1.0,
            WidthMode::Compact,
            RowSet::default(),
            DEFAULT_RATIO,
            0,
        );
        assert_eq!(f.dropped, 1);
        assert!(f.row(Row::Eq).is_none() && f.row(Row::Width).is_some());
        assert!(!f.scrolls);
        // Much shorter: every drop taken, the name shortened, then it scrolls.
        let f = fit(
            200.0,
            WidthMode::Compact,
            RowSet::default(),
            DEFAULT_RATIO,
            0,
        );
        assert_eq!(f.dropped, DROPS.len());
        assert!(f.row(Row::Eq).is_none() && f.row(Row::Width).is_none());
        assert!(f.row(Row::PhaseSwap).is_none());
        assert_eq!(span(&f, Row::Name), NAME_SHORT);
        assert!(f.scrolls && f.height > 200.0);
        // The sends and the switches are still there to scroll to.
        assert!(f.row(Row::Sends).is_some() && f.row(Row::FxRec).is_some());
    }

    #[test]
    fn the_ratio_moves_height_between_meter_and_fader() {
        let a = fit(700.0, WidthMode::Wide, RowSet::default(), 0.25, 0);
        let b = fit(700.0, WidthMode::Wide, RowSet::default(), 0.5, 0);
        assert!(span(&b, Row::Meter) > span(&a, Row::Meter));
        assert!(span(&b, Row::Fader) < span(&a, Row::Fader));
        let total = |f: &Fit| span(f, Row::Meter) + span(f, Row::Fader);
        assert!((total(&a) - total(&b)).abs() < 1.0);
        // A very tall desk gives the fader its tallest and the meter the rest.
        let t = fit(2000.0, WidthMode::Wide, RowSet::default(), 0.2, 0);
        assert_eq!(span(&t, Row::Fader), FADER_MAX);
    }

    #[test]
    fn switched_off_rows_are_gone_and_wide_names_lie_flat() {
        let mut shown = RowSet::default();
        shown.toggle(Row::Sends);
        shown.set(Row::Pan, false);
        let f = fit(900.0, WidthMode::Xl, shown, DEFAULT_RATIO, 5);
        assert!(f.row(Row::Sends).is_none() && f.row(Row::Pan).is_none());
        assert!(!f.rotated);
        assert_eq!(span(&f, Row::Name), NAME_ACROSS);
        assert_eq!(f.slots, 5);
        assert!(f.row(Row::Readout).is_some() && f.row(Row::Inserts).is_some());
        // The meter and the fader cannot be switched off.
        let f = fit(900.0, WidthMode::Wide, RowSet(0), DEFAULT_RATIO, 0);
        assert!(f.row(Row::Meter).is_some() && f.row(Row::Fader).is_some());
        assert!(f.row(Row::Number).is_some());
    }

    #[test]
    fn width_modes_step_and_stop_at_the_ends() {
        assert_eq!(WidthMode::Compact.step(1), WidthMode::Wide);
        assert_eq!(WidthMode::Xl.step(1), WidthMode::Xl);
        assert_eq!(WidthMode::Compact.step(-1), WidthMode::Compact);
        assert!(WidthMode::Compact.rotated_names() && !WidthMode::Wide.rotated_names());
    }
}

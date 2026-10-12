//! The desk's geometry (ADR 0024 §6, ADR 0041): the width modes, the rows every strip
//! shares so they line up across the desk, and **fit first, then scroll**.
//!
//! Pure arithmetic, no egui: given the height the desk has, which rows are on and how the
//! meter and the fader split the flexible height, [`fit`] says where each row goes. When
//! the desk is short, rows drop in the ADR's order (the EQ bars, the width knob, the
//! phase and swap switches, the readout, then the name shrinks, the send list shortens to
//! two rows, the cable band to its least and the output chip goes); when even the minimum
//! does not fit, the content is taller than the desk and the desk scrolls vertically.

use serde::{Deserialize, Serialize};

/// How wide every strip is (FL's layouts, reduced to three). Ctrl+wheel over the desk
/// steps through them; the choice is kept per window layout.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WidthMode {
    /// 24 pt: FL's Compact (Plex Condensed needs a little more than FL's 20). Names run
    /// up the strip.
    #[default]
    Compact,
    /// 48 pt: names across, the fader's value under it, the output chip.
    Wide,
    /// 76 pt: level, pan, sends and routing (round 25, ADR 0041 §9) — the strip's own
    /// send rows, its output and its chain in one line; the inserts live in the inspector.
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
    /// The insert chain in one line, "EQ · Comp · +1" (Extra large; round 25, the owner's
    /// answer 7): a click opens the inspector, where the inserts live.
    Chain,
    /// Where the strip's output goes, as a chip: "→ 12" (Wide and Extra large).
    Output,
    /// The strip's own sends, one row each with its switch, destination, tap and level
    /// (Extra large).
    SendList,
    /// FL's row relative to the selected strip: "can the selected strip go here?"
    Sends,
    /// The cable band (round 25): every strip's port and the routes as cables. On while
    /// the desk's cable view is not *Off*.
    Cables,
}

/// How many rows there are.
pub const ROWS: usize = 16;

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
        Row::Chain,
        Row::Output,
        Row::SendList,
        Row::Sends,
        Row::Cables,
    ];

    /// The families the desk's ▾ menu can switch off. The number, the meter and the
    /// fader are the strip; they stay. The cable band follows the cable view instead.
    pub const TOGGLES: [Row; 12] = [
        Row::Name,
        Row::MuteSolo,
        Row::Pan,
        Row::Readout,
        Row::PhaseSwap,
        Row::Width,
        Row::Eq,
        Row::FxRec,
        Row::Chain,
        Row::Output,
        Row::SendList,
        Row::Sends,
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
            Row::Chain => "mixer.legend.chain",
            Row::Output => "mixer.legend.output",
            Row::SendList => "mixer.legend.sends",
            Row::Sends => "mixer.legend.from_selected",
            Row::Cables => "cables.legend",
        }
    }

    /// The bit a [`RowSet`] keeps it in. Fixed, not the row's place, so a desk saved
    /// before a row was added reads the same: the round-25 rows took the insert slots'
    /// old bit (12, now the chain that replaced them) and the free ones after it.
    fn bit(self) -> u16 {
        let b = match self {
            Row::Number => 0,
            Row::Name => 1,
            Row::Meter => 2,
            Row::MuteSolo => 3,
            Row::Pan => 4,
            Row::Fader => 5,
            Row::Readout => 6,
            Row::PhaseSwap => 7,
            Row::Width => 8,
            Row::Eq => 9,
            Row::FxRec => 10,
            Row::Sends => 11,
            Row::Chain => 12,
            Row::Output => 13,
            Row::SendList => 14,
            Row::Cables => 15,
        };
        1 << b
    }
}

/// Which cables the band draws (round 25, the owner's answer 1): none, the selection's
/// (the default), or every one — those away from the selection faint. A desk
/// preference, kept per window layout, never project data.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CableView {
    Off,
    #[default]
    Selected,
    All,
}

impl CableView {
    pub const ALL: [CableView; 3] = [CableView::Off, CableView::Selected, CableView::All];

    /// Its name (an i18n key with a `.short` form).
    pub fn key(self) -> &'static str {
        match self {
            CableView::Off => "cables.view.off",
            CableView::Selected => "cables.view.selected",
            CableView::All => "cables.view.all",
        }
    }

    pub fn index(self) -> usize {
        match self {
            CableView::Off => 0,
            CableView::Selected => 1,
            CableView::All => 2,
        }
    }

    pub fn from_index(i: usize) -> CableView {
        CableView::ALL[i.min(2)]
    }
}

/// Which of the selection's cables count as its own (round 25, the owner's request of
/// 2026-10-11): what goes *out* of the selected strips (their output, sends and keys), what
/// comes *in* (their feeders' outputs, sends and keys arriving at them), or both — today's
/// behaviour and the default. In *Selected* it decides what is drawn; in *All* what is lit.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CableScope {
    Out,
    In,
    #[default]
    Both,
}

impl CableScope {
    pub const ALL: [CableScope; 3] = [CableScope::Out, CableScope::In, CableScope::Both];

    /// Its menu row (an i18n key).
    pub fn key(self) -> &'static str {
        match self {
            CableScope::Out => "cables.scope.out",
            CableScope::In => "cables.scope.in",
            CableScope::Both => "cables.scope.both",
        }
    }

    /// Whether a cable belongs to the selection, given whether its source (`from`) and
    /// its destination (`to`) are selected.
    pub fn takes(self, from: bool, to: bool) -> bool {
        match self {
            CableScope::Out => from,
            CableScope::In => to,
            CableScope::Both => from || to,
        }
    }
}

/// A kind of cable the band can hide (the options menu's switches).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CableKindSel {
    Outputs,
    Sends,
    Keys,
}

impl CableKindSel {
    pub const ALL: [CableKindSel; 3] = [CableKindSel::Outputs, CableKindSel::Sends, CableKindSel::Keys];

    /// Its menu row (an i18n key).
    pub fn key(self) -> &'static str {
        match self {
            CableKindSel::Outputs => "cables.kinds.outputs",
            CableKindSel::Sends => "cables.kinds.sends",
            CableKindSel::Keys => "cables.kinds.keys",
        }
    }
}

/// A one-click set-up of the options menu ("Quick views").
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CablePreset {
    /// Every kind, both ways, every detail: the default.
    Everything,
    /// Only what the selection sends (the owner's "only the current channel's sends").
    Sends,
    /// Only what arrives at the selection: its feeders.
    Feeders,
}

impl CablePreset {
    pub const ALL: [CablePreset; 3] = [
        CablePreset::Everything,
        CablePreset::Sends,
        CablePreset::Feeders,
    ];

    /// Its menu row (an i18n key).
    pub fn key(self) -> &'static str {
        match self {
            CablePreset::Everything => "cables.preset.everything",
            CablePreset::Sends => "cables.preset.sends",
            CablePreset::Feeders => "cables.preset.feeders",
        }
    }

    /// The options it sets.
    pub fn opts(self) -> CableOpts {
        let all = CableOpts::default();
        match self {
            CablePreset::Everything => all,
            CablePreset::Sends => CableOpts {
                scope: CableScope::Out,
                outputs: false,
                keys: false,
                ..all
            },
            CablePreset::Feeders => CableOpts {
                scope: CableScope::In,
                ..all
            },
        }
    }
}

/// The cable band's view options beside the *Off · Selected · All* switch (the options
/// menu): whose cables, which kinds, how much detail. A desk preference kept with the
/// rest of [`super::DeskPrefs`]; a desk saved before them reads as the default, which is
/// the band as round 25 drew it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CableOpts {
    pub scope: CableScope,
    /// The kinds drawn: a hidden kind is neither drawn nor hit-tested (nor removed by
    /// Delete, which only removes a cable the band drew).
    pub outputs: bool,
    pub sends: bool,
    pub keys: bool,
    /// The level pills on the sends (with their *PRE* tag and, when selected, the tap
    /// chip beside them).
    pub levels: bool,
    /// The selection's outputs into the master (one per selected strip, all converging).
    pub master: bool,
}

impl Default for CableOpts {
    fn default() -> Self {
        CableOpts {
            scope: CableScope::Both,
            outputs: true,
            sends: true,
            keys: true,
            levels: true,
            master: true,
        }
    }
}

impl CableOpts {
    /// Whether a kind is drawn.
    pub fn shows(&self, k: CableKindSel) -> bool {
        match k {
            CableKindSel::Outputs => self.outputs,
            CableKindSel::Sends => self.sends,
            CableKindSel::Keys => self.keys,
        }
    }

    /// Show or hide a kind.
    pub fn toggle(&mut self, k: CableKindSel) {
        let on = !self.shows(k);
        match k {
            CableKindSel::Outputs => self.outputs = on,
            CableKindSel::Sends => self.sends = on,
            CableKindSel::Keys => self.keys = on,
        }
    }

    /// Whether anything differs from the default (the title row's options key is lit).
    pub fn filtered(&self) -> bool {
        *self != CableOpts::default()
    }

    /// The quick view these options are, if any.
    pub fn preset(&self) -> Option<CablePreset> {
        CablePreset::ALL.into_iter().find(|p| p.opts() == *self)
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
/// A send row of Extra large: its height, and the fewest and most the list shows.
pub const SEND_ROW: f32 = 15.0;
pub const MIN_SEND_ROWS: usize = 2;
pub const MAX_SEND_ROWS: usize = 6;
/// The cable band: its height by default, and the least and most its handle gives it.
pub const BAND: f32 = 40.0;
pub const BAND_MIN: f32 = 20.0;
pub const BAND_MAX: f32 = 64.0;

/// A rotated name's height, and what it shrinks to when the desk is short.
const NAME_TALL: f32 = 64.0;
const NAME_SHORT: f32 = 40.0;
/// A name set across the strip.
const NAME_ACROSS: f32 = 18.0;

/// What the rows that size themselves ask for.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Wants {
    /// Send rows the Extra large list wants (the strip with the most sends, and the
    /// "+ Send" row), clamped to [`MIN_SEND_ROWS`]…[`MAX_SEND_ROWS`].
    pub send_rows: usize,
    /// The cable band's height (the legend's handle), clamped to
    /// [`BAND_MIN`]…[`BAND_MAX`].
    pub band: f32,
}

impl Default for Wants {
    fn default() -> Self {
        Wants {
            send_rows: MIN_SEND_ROWS,
            band: BAND,
        }
    }
}

/// The send rows a desk whose busiest strip has `most` sends wants: one more for the
/// "+ Send" row, within the list's limits.
pub fn send_rows_for(most: usize) -> usize {
    (most + 1).clamp(MIN_SEND_ROWS, MAX_SEND_ROWS)
}

/// The fixed height of a row (the meter and the fader are flexible: 0).
fn fixed(r: Row, rotated: bool, name_short: bool, w: Wants) -> f32 {
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
        Row::Chain => 16.0,
        Row::Output => 16.0,
        Row::SendList => w.send_rows as f32 * SEND_ROW + 2.0,
        Row::Sends => 18.0,
        Row::Cables => w.band,
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
    /// Send rows shown (Extra large; 0 without the list).
    pub send_rows: usize,
    /// The cable band's height (0 without it).
    pub band: f32,
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

/// The order rows give way in when the desk is short (ADR 0024 §6, ADR 0041 §9): the EQ
/// bars, the width knob, phase and swap, the readout; then the name shrinks, the send
/// list shortens to two rows, the cable band to its least, and the output chip goes. The
/// band never goes while the cable view is on: it is the routing.
const DROPS: [Drop; 8] = [
    Drop::Off(Row::Eq),
    Drop::Off(Row::Width),
    Drop::Off(Row::PhaseSwap),
    Drop::Off(Row::Readout),
    Drop::NameShort,
    Drop::FewerSends,
    Drop::ShorterBand,
    Drop::Off(Row::Output),
];

#[derive(Copy, Clone)]
enum Drop {
    Off(Row),
    NameShort,
    FewerSends,
    ShorterBand,
}

/// Lay the rows out in `avail` points of height.
///
/// `mode` decides the rows that exist (the readout and the output chip from Wide, the
/// chain and the send list at Extra large); `shown` is the ▾ menu's switches and the
/// cable view (the band's bit); `ratio` is the meter's share of the flexible height (the
/// 3-dot handle); `wants` is what the send list and the band ask for.
pub fn fit(avail: f32, mode: WidthMode, shown: RowSet, ratio: f32, wants: Wants) -> Fit {
    let rotated = mode.rotated_names();
    let mut on = shown;
    // Rows the width mode does not have.
    if mode == WidthMode::Compact {
        on.set(Row::Readout, false);
        on.set(Row::Output, false);
    }
    if mode != WidthMode::Xl {
        on.set(Row::Chain, false);
        on.set(Row::SendList, false);
    }
    on.set(Row::Number, true);
    on.set(Row::Meter, true);
    on.set(Row::Fader, true);
    let mut name_short = false;
    let mut w = Wants {
        send_rows: wants.send_rows.clamp(MIN_SEND_ROWS, MAX_SEND_ROWS),
        band: wants.band.clamp(BAND_MIN, BAND_MAX),
    };
    // Called once a frame: counts in place, allocates nothing.
    let need = |on: RowSet, name_short: bool, w: Wants| -> f32 {
        let (mut sum, mut n) = (0.0f32, 0usize);
        for r in Row::ALL.iter().filter(|r| on.has(**r)) {
            sum += fixed(*r, rotated, name_short, w);
            n += 1;
        }
        sum + GAP * n.saturating_sub(1) as f32 + FOOT + METER_MIN + FADER_MIN
    };
    let mut dropped = 0;
    while need(on, name_short, w) > avail && dropped < DROPS.len() {
        match DROPS[dropped] {
            Drop::Off(r) => on.set(r, false),
            Drop::NameShort => name_short = rotated,
            Drop::FewerSends => w.send_rows = MIN_SEND_ROWS,
            Drop::ShorterBand => w.band = BAND_MIN,
        }
        dropped += 1;
    }
    let minimum = need(on, name_short, w);
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
            _ => fixed(r, rotated, name_short, w),
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
        send_rows: if on.has(Row::SendList) {
            w.send_rows
        } else {
            0
        },
        band: if on.has(Row::Cables) { w.band } else { 0.0 },
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

    fn all() -> Wants {
        Wants::default()
    }

    #[test]
    fn a_tall_desk_shows_every_row_and_splits_the_rest() {
        let f = fit(
            900.0,
            WidthMode::Compact,
            RowSet::default(),
            DEFAULT_RATIO,
            all(),
        );
        assert_eq!(f.dropped, 0);
        assert!(!f.scrolls);
        for r in [
            Row::Eq,
            Row::Width,
            Row::PhaseSwap,
            Row::Sends,
            Row::FxRec,
            Row::Cables,
        ] {
            assert!(f.row(r).is_some(), "{r:?}");
        }
        // Compact has no readout, output chip, chain or send list.
        for r in [Row::Readout, Row::Output, Row::Chain, Row::SendList] {
            assert!(f.row(r).is_none(), "{r:?}");
        }
        assert!(f.rotated);
        assert!(span(&f, Row::Fader) >= FADER_MIN && span(&f, Row::Meter) >= METER_MIN);
        assert_eq!(span(&f, Row::Cables), BAND);
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

    /// Design §4.5 and test 6: no insert slots in any mode; the send list and the chain
    /// only at Extra large, the output chip from Wide, the band in every mode while the
    /// cable view is on; Extra large's order is the owner's list.
    #[test]
    fn extra_large_shows_sends_and_routing_not_inserts() {
        let wants = Wants {
            send_rows: 4,
            band: 40.0,
        };
        let c = fit(1000.0, WidthMode::Compact, RowSet::default(), 0.38, wants);
        let w = fit(1000.0, WidthMode::Wide, RowSet::default(), 0.38, wants);
        let x = fit(1000.0, WidthMode::Xl, RowSet::default(), 0.38, wants);
        assert!(c.row(Row::Output).is_none() && w.row(Row::Output).is_some());
        assert!(w.row(Row::SendList).is_none() && w.row(Row::Chain).is_none());
        assert!(x.row(Row::SendList).is_some() && x.row(Row::Chain).is_some());
        assert_eq!(x.send_rows, 4);
        assert_eq!(span(&x, Row::SendList), 4.0 * SEND_ROW + 2.0);
        for f in [&c, &w, &x] {
            assert_eq!(span(f, Row::Cables), 40.0);
            assert!(f.row(Row::Sends).is_some(), "FL's row stays in every mode");
        }
        let order = [
            Row::Fader,
            Row::Readout,
            Row::Eq,
            Row::FxRec,
            Row::Chain,
            Row::Output,
            Row::SendList,
            Row::Sends,
            Row::Cables,
        ];
        for pair in order.windows(2) {
            let (a, b) = (x.row(pair[0]).unwrap(), x.row(pair[1]).unwrap());
            assert!(a.1 <= b.0, "{:?} before {:?}", pair[0], pair[1]);
        }
        // The cable view off: no band, in any mode.
        let mut shown = RowSet::default();
        shown.set(Row::Cables, false);
        let off = fit(1000.0, WidthMode::Xl, shown, 0.38, wants);
        assert!(off.row(Row::Cables).is_none() && off.band == 0.0);
        // The send list's rows: as the busiest strip wants, within its limits.
        assert_eq!(send_rows_for(0), MIN_SEND_ROWS);
        assert_eq!(send_rows_for(3), 4);
        assert_eq!(send_rows_for(16), MAX_SEND_ROWS);
    }

    #[test]
    fn a_short_desk_drops_rows_in_order_then_scrolls() {
        let full = fit(
            900.0,
            WidthMode::Compact,
            RowSet::default(),
            DEFAULT_RATIO,
            all(),
        );
        let need =
            full.height - span(&full, Row::Meter) - span(&full, Row::Fader) + METER_MIN + FADER_MIN;
        // Just short of everything: the EQ goes first.
        let f = fit(
            need - 1.0,
            WidthMode::Compact,
            RowSet::default(),
            DEFAULT_RATIO,
            all(),
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
            all(),
        );
        assert_eq!(f.dropped, DROPS.len());
        assert!(f.row(Row::Eq).is_none() && f.row(Row::Width).is_none());
        assert!(f.row(Row::PhaseSwap).is_none());
        assert_eq!(span(&f, Row::Name), NAME_SHORT);
        assert!(f.scrolls && f.height > 200.0);
        // The sends, the switches and the band are still there to scroll to.
        assert!(f.row(Row::Sends).is_some() && f.row(Row::FxRec).is_some());
        assert_eq!(
            span(&f, Row::Cables),
            BAND_MIN,
            "the band shrinks, never goes"
        );
    }

    /// The round-25 drop order, Extra large: the send list shortens before the band, the
    /// band before the output chip.
    #[test]
    fn extra_large_gives_way_send_list_then_band_then_output() {
        let wants = Wants {
            send_rows: 6,
            band: 48.0,
        };
        let full = fit(2000.0, WidthMode::Xl, RowSet::default(), 0.38, wants);
        let need =
            |f: &Fit| f.height - span(f, Row::Meter) - span(f, Row::Fader) + METER_MIN + FADER_MIN;
        let base = need(&full);
        // Everything up to the name shrinking (Extra large names lie flat: no change).
        let lost = 26.0 + 22.0 + 16.0 + 16.0 + 4.0 * GAP;
        let f = fit(
            base - lost - 1.0,
            WidthMode::Xl,
            RowSet::default(),
            0.38,
            wants,
        );
        assert_eq!(f.send_rows, MIN_SEND_ROWS, "the send list shortens first");
        assert_eq!(span(&f, Row::Cables), 48.0);
        assert!(f.row(Row::Output).is_some());
        let lost2 = lost + 4.0 * SEND_ROW;
        let f = fit(
            base - lost2 - 1.0,
            WidthMode::Xl,
            RowSet::default(),
            0.38,
            wants,
        );
        assert_eq!(span(&f, Row::Cables), BAND_MIN, "then the band");
        assert!(f.row(Row::Output).is_some());
        let lost3 = lost2 + (48.0 - BAND_MIN);
        let f = fit(
            base - lost3 - 1.0,
            WidthMode::Xl,
            RowSet::default(),
            0.38,
            wants,
        );
        assert!(f.row(Row::Output).is_none(), "then the output chip");
        assert!(f.row(Row::Chain).is_some() && f.row(Row::SendList).is_some());
    }

    #[test]
    fn the_ratio_moves_height_between_meter_and_fader() {
        let a = fit(700.0, WidthMode::Wide, RowSet::default(), 0.25, all());
        let b = fit(700.0, WidthMode::Wide, RowSet::default(), 0.5, all());
        assert!(span(&b, Row::Meter) > span(&a, Row::Meter));
        assert!(span(&b, Row::Fader) < span(&a, Row::Fader));
        let total = |f: &Fit| span(f, Row::Meter) + span(f, Row::Fader);
        assert!((total(&a) - total(&b)).abs() < 1.0);
        // A very tall desk gives the fader its tallest and the meter the rest.
        let t = fit(2000.0, WidthMode::Wide, RowSet::default(), 0.2, all());
        assert_eq!(span(&t, Row::Fader), FADER_MAX);
    }

    #[test]
    fn switched_off_rows_are_gone_and_wide_names_lie_flat() {
        let mut shown = RowSet::default();
        shown.toggle(Row::Sends);
        shown.set(Row::Pan, false);
        let f = fit(900.0, WidthMode::Xl, shown, DEFAULT_RATIO, all());
        assert!(f.row(Row::Sends).is_none() && f.row(Row::Pan).is_none());
        assert!(!f.rotated);
        assert_eq!(span(&f, Row::Name), NAME_ACROSS);
        assert!(f.row(Row::Readout).is_some() && f.row(Row::SendList).is_some());
        // The meter and the fader cannot be switched off.
        let f = fit(900.0, WidthMode::Wide, RowSet(0), DEFAULT_RATIO, all());
        assert!(f.row(Row::Meter).is_some() && f.row(Row::Fader).is_some());
        assert!(f.row(Row::Number).is_some());
    }

    /// A desk saved before round 25 reads the same: the old rows keep their bits, and a
    /// desk that had switched the insert slots off has the chain that replaced them off.
    #[test]
    fn saved_row_switches_keep_their_meaning() {
        // Round 24's bits: the EQ was 9, FL's send row 11, the insert slots 12.
        let old = RowSet(!(1 << 11) & !(1 << 12) & !(1 << 9));
        assert!(!old.has(Row::Sends) && !old.has(Row::Chain) && !old.has(Row::Eq));
        assert!(old.has(Row::Output) && old.has(Row::SendList) && old.has(Row::Cables));
        let bits: std::collections::BTreeSet<u16> = Row::ALL.iter().map(|r| r.bit()).collect();
        assert_eq!(bits.len(), ROWS, "every row has its own bit");
        assert!(
            !Row::TOGGLES.contains(&Row::Cables),
            "the cable view drives the band"
        );
    }

    #[test]
    fn width_modes_step_and_stop_at_the_ends() {
        assert_eq!(WidthMode::Compact.step(1), WidthMode::Wide);
        assert_eq!(WidthMode::Xl.step(1), WidthMode::Xl);
        assert_eq!(WidthMode::Compact.step(-1), WidthMode::Compact);
        assert!(WidthMode::Compact.rotated_names() && !WidthMode::Wide.rotated_names());
        assert_eq!(
            CableView::default(),
            CableView::Selected,
            "the owner's answer 1"
        );
        assert_eq!(CableView::from_index(2), CableView::All);
    }

    /// The options default to the band as round 25 drew it; the quick views are what
    /// they say, and only the default is unfiltered.
    #[test]
    fn cable_options_default_to_everything_and_presets_say_what_they_are() {
        let d = CableOpts::default();
        assert_eq!(d.scope, CableScope::Both);
        assert!(d.outputs && d.sends && d.keys && d.levels && d.master);
        assert!(!d.filtered());
        assert_eq!(d.preset(), Some(CablePreset::Everything));
        let s = CablePreset::Sends.opts();
        assert!(s.sends && !s.outputs && !s.keys && s.scope == CableScope::Out);
        assert!(s.filtered());
        assert_eq!(s.preset(), Some(CablePreset::Sends));
        let f = CablePreset::Feeders.opts();
        assert_eq!(f.scope, CableScope::In);
        assert!(f.outputs && f.sends && f.keys);
        let mut o = d;
        o.toggle(CableKindSel::Keys);
        assert!(!o.keys && o.filtered() && o.preset().is_none());
        o.toggle(CableKindSel::Keys);
        assert_eq!(o, d);
        // Scope: out takes the source's side, in the destination's, both either.
        assert!(CableScope::Out.takes(true, false) && !CableScope::Out.takes(false, true));
        assert!(CableScope::In.takes(false, true) && !CableScope::In.takes(true, false));
        assert!(CableScope::Both.takes(true, false) && CableScope::Both.takes(false, true));
    }
}

//! The desk's geometry (ADR 0024 §6, ADR 0041): the width modes, the rows every strip
//! shares so they line up across the desk, and **fit first, then scroll**.
//!
//! Pure arithmetic, no egui: given the height the desk has and which rows are on, [`fit`]
//! says where each row goes — des-mixer's rows (round 39), in signal order: the sound
//! (head, inserts, EQ, polarity · swap · width), the mix (pan, M · S · arm, the fader with
//! its meter beside it, the level), the route (sends, the output, the socket). When the
//! desk is short, rows leave in des-mixer's order (the EQ, the polarity row, the inserts
//! fold to one line, the sends, the socket, the level, the output) and the fader keeps the
//! rest; when even the minimum does not fit, the desk scrolls vertically.

use serde::{Deserialize, Serialize};

/// How wide every strip is (FL's layouts, reduced to three). Ctrl+wheel over the desk
/// steps through them; the choice is kept per window layout.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WidthMode {
    /// 30 pt (des-mixer): a 12-px name runs up the strip, two keys fit.
    #[default]
    Compact,
    /// 60 pt: names across, four insert rows, the EQ curve, the level, two sends.
    Wide,
    /// 94 pt ("Large"): unit names, the EQ's bands, pan and send values, the scale.
    Xl,
}

impl WidthMode {
    pub const ALL: [WidthMode; 3] = [WidthMode::Compact, WidthMode::Wide, WidthMode::Xl];

    /// The strip's pitch in points at 100 %: its body (des-mixer: 30 / 60 / 94) and the 2 pt
    /// of the desk after it.
    pub fn pitch(self) -> f32 {
        match self {
            WidthMode::Compact => 32.0,
            WidthMode::Wide => 62.0,
            WidthMode::Xl => 96.0,
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
    Off,
    Low,
    /// The default since round 37 (finish): the whole strip in its colour. // r37:finish
    #[default]
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
            Colourful::Low => 0.12,    // r37:finish
            Colourful::Medium => 0.24, // r37:finish
            Colourful::High => 0.40,   // r37:finish
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
    pub const TOGGLES: [Row; 9] = [
        Row::Chain,
        Row::Eq,
        Row::PhaseSwap,
        Row::Pan,
        Row::MuteSolo,
        Row::Readout,
        Row::SendList,
        Row::Output,
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
    pub(crate) fn bit(self) -> u16 {
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

/// Which cables the band draws: none (the default since round 39, the owner's option C:
/// a strip says where it goes, its destinations light), the selection's (the header's
/// Cables key, round 25's answer 1), or every one — those away from the selection faint
/// ("Show all"). A desk preference, kept per window layout, never project data.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CableView {
    #[default]
    Off,
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
    pub const ALL: [CableKindSel; 3] = [
        CableKindSel::Outputs,
        CableKindSel::Sends,
        CableKindSel::Keys,
    ];

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
            levels: false, // r39:el-mixer — des-mixer: the band is the sockets' row
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
    /// Every row but FL's "from the selected strip" row (des-mixer: the cables and the
    /// sends list carry the routing; the Rows menu brings FL's row back).
    fn default() -> Self {
        RowSet(!Row::Sends.bit())
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

/// The gap between rows, in points at 100 % (Large: [`GAP_LARGE`]).
pub const GAP: f32 = 4.0;
pub const GAP_LARGE: f32 = 6.0;
/// The space below the last row.
pub const FOOT: f32 = 0.0;
/// The fader's least travel per width mode (des-mixer: 120 / 112 / 150), and the cap's
/// overhang above and below the travel (10 each).
pub fn fader_min(mode: WidthMode) -> f32 {
    match mode {
        WidthMode::Compact => 120.0,
        WidthMode::Wide => 112.0,
        WidthMode::Xl => 150.0,
    }
}
pub const OVERHANG: f32 = 10.0;
/// The longest travel a hand wants; the desk's height beyond it stays below the strips.
pub const FADER_MAX: f32 = 520.0;
/// Kept for the saved desks' `ratio` (the meter now stands beside the fader: no split).
pub const DEFAULT_RATIO: f32 = 0.30;
/// A send row: Wide 14, Large 20 (des-mixer), and how many each shows.
pub fn send_row(mode: WidthMode) -> f32 {
    if mode == WidthMode::Xl {
        20.0
    } else {
        14.0
    }
}
pub fn send_rows(mode: WidthMode) -> usize {
    match mode {
        WidthMode::Compact => 3,
        WidthMode::Wide => 2,
        WidthMode::Xl => 3,
    }
}
/// The socket row, where the cable band hangs (des-mixer: 18).
pub const BAND: f32 = 18.0;
pub const BAND_MIN: f32 = 18.0;
pub const BAND_MAX: f32 = 18.0;
/// The deepest the cable band grows (r39:el-cables: Subway's lanes on a dense desk).
pub const BAND_DEEPEST: f32 = 160.0;
/// The insert list folded to one summary line ("Comp · EQ").
pub const INSERTS_FOLDED: f32 = 14.0;
/// The least a strip's click target is, in points at 100 % (round 37).
pub const TARGET: f32 = 24.0;

/// What the rows that size themselves ask for: the band's depth (des-mixer's 18, or 40
/// while the level pills show).
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Wants {
    pub send_rows: usize,
    pub band: f32,
}

/// The height of a row in `mode` (des-mixer's table, README "Rows"), 0 for a row the mode
/// does not have. The fader is flexible (0 here).
fn fixed(r: Row, mode: WidthMode, folded: bool, band: f32) -> f32 {
    use WidthMode::*;
    match (r, mode) {
        (Row::Number, Compact) => 112.0,
        (Row::Number, Wide) => 36.0,
        (Row::Number, Xl) => 44.0,
        (Row::Chain, Compact) => 10.0,
        (Row::Chain, _) if folded => INSERTS_FOLDED,
        (Row::Chain, Wide) => 68.0,
        (Row::Chain, Xl) => 94.0,
        (Row::Eq, Wide) => 22.0,
        (Row::Eq, Xl) => 36.0,
        (Row::PhaseSwap, Wide) => 24.0,
        (Row::PhaseSwap, Xl) => 24.0,
        (Row::Pan, Compact) => 22.0,
        (Row::Pan, Wide) => 28.0,
        (Row::Pan, Xl) => 40.0,
        (Row::MuteSolo, Compact) => 48.0,
        (Row::MuteSolo, Wide) => 24.0,
        (Row::MuteSolo, Xl) => 24.0,
        (Row::Readout, Wide) => 16.0,
        (Row::Readout, Xl) => 18.0,
        (Row::SendList, Compact) => 12.0,
        (Row::SendList, Wide) => 28.0,
        (Row::SendList, Xl) => 60.0,
        (Row::Output, Compact) => 12.0,
        (Row::Output, Wide) => 14.0,
        (Row::Output, Xl) => 18.0,
        (Row::Sends, _) => TARGET + 2.0,
        (Row::Cables, _) => band,
        _ => 0.0,
    }
}

/// The rows a width mode has, top to bottom, in signal order: the sound (head, inserts,
/// EQ, polarity · swap · width), the mix (pan, M · S · arm, the fader with its meter, the
/// level), the route (sends, the output, FL's row when switched on, the socket).
pub const ORDER: [Row; 12] = [
    Row::Number,
    Row::Chain,
    Row::Eq,
    Row::PhaseSwap,
    Row::Pan,
    Row::MuteSolo,
    Row::Fader,
    Row::Readout,
    Row::SendList,
    Row::Output,
    Row::Sends,
    Row::Cables,
];

/// Where every row of a strip goes.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Fit {
    /// Each row's `(top, bottom)` from the top of the strips, in points at 100 %; `None`
    /// for a row that is off, dropped, or not in this width mode. The fader's span holds
    /// the cap's overhang: its travel is [`Fit::travel`].
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
    /// Send rows shown (0 without the list).
    pub send_rows: usize,
    /// The cable band's height (0 without it).
    pub band: f32,
    /// The insert list is folded to one summary line.
    pub folded: bool,
}

impl Fit {
    pub fn row(&self, r: Row) -> Option<(f32, f32)> {
        self.rows[r.index()]
    }

    /// The centre line of a row.
    pub fn mid(&self, r: Row) -> Option<f32> {
        self.row(r).map(|(a, b)| (a + b) * 0.5)
    }

    /// The fader's travel: its row less the cap's overhang at each end.
    pub fn travel(&self) -> Option<(f32, f32)> {
        self.row(Row::Fader)
            .map(|(a, z)| (a + OVERHANG, (z - OVERHANG).max(a + OVERHANG + 1.0)))
    }
}

/// The order rows give way in when the desk is short (des-mixer, "When the desk is
/// short"): the EQ, the polarity row, the inserts fold to one line, the sends, the level,
/// the output. Then the desk scrolls. The cable band stays: since round 39 it shows only
/// when asked for (the header's Cables key), and then it is what was asked for.
const DROPS: [Drop; 6] = [
    Drop::Off(Row::Eq),
    Drop::Off(Row::PhaseSwap),
    Drop::Fold,
    Drop::Off(Row::SendList),
    Drop::Off(Row::Readout),
    Drop::Off(Row::Output),
];

#[derive(Copy, Clone)]
enum Drop {
    Off(Row),
    Fold,
}

/// Lay the rows out in `avail` points of height. `shown` is the Rows menu's switches and
/// the cable view (the band's bit); `_ratio` and `_wants` are kept for the callers.
pub fn fit(avail: f32, mode: WidthMode, shown: RowSet, _ratio: f32, wants: Wants) -> Fit {
    let band = wants.band.clamp(BAND, BAND_DEEPEST);
    let mut on = shown;
    for r in Row::ALL {
        if fixed(r, mode, false, band) == 0.0 && r != Row::Fader {
            on.set(r, false);
        }
    }
    on.set(Row::Number, true);
    on.set(Row::Fader, true);
    let gap = if mode == WidthMode::Xl {
        GAP_LARGE
    } else {
        GAP
    };
    let mut folded = false;
    let need = |on: RowSet, folded: bool| -> f32 {
        let (mut sum, mut n) = (0.0f32, 0usize);
        for r in ORDER.iter().filter(|r| on.has(**r)) {
            sum += fixed(*r, mode, folded, band);
            n += 1;
        }
        sum + gap * n.saturating_sub(1) as f32 + 2.0 * OVERHANG + fader_min(mode) + FOOT
    };
    let mut dropped = 0;
    while need(on, folded) > avail && dropped < DROPS.len() {
        match DROPS[dropped] {
            Drop::Off(r) => on.set(r, false),
            Drop::Fold => folded = mode != WidthMode::Compact && on.has(Row::Chain),
        }
        dropped += 1;
    }
    let minimum = need(on, folded);
    let scrolls = minimum > avail + 0.5;
    let height = avail.max(minimum);
    let travel = (fader_min(mode) + height - minimum).min(FADER_MAX.max(fader_min(mode)));
    let mut rows = [None; ROWS];
    let mut y = 0.0f32;
    let mut first = true;
    for r in ORDER {
        if !on.has(r) {
            continue;
        }
        if !first {
            y += gap;
        }
        first = false;
        let h = match r {
            Row::Fader => travel + 2.0 * OVERHANG,
            _ => fixed(r, mode, folded, band),
        };
        rows[r.index()] = Some((y, y + h));
        y += h;
    }
    Fit {
        rows,
        height: height.max(y + FOOT),
        dropped,
        scrolls,
        rotated: mode.rotated_names(),
        send_rows: if on.has(Row::SendList) {
            send_rows(mode)
        } else {
            0
        },
        band: if on.has(Row::Cables) { band } else { 0.0 },
        folded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(f: &Fit, r: Row) -> f32 {
        f.row(r).map(|(a, b)| b - a).unwrap_or(0.0)
    }

    fn fit_at(h: f32, mode: WidthMode) -> Fit {
        fit(h, mode, RowSet::default(), DEFAULT_RATIO, Wants::default())
    }

    /// des-mixer's redline at 616 px of strip: every row of each density at its measured
    /// height, in signal order, 4 apart (Large 6); the fader takes the rest.
    #[test]
    fn the_rows_are_the_redlines_heights_in_signal_order() {
        let w = fit_at(616.0, WidthMode::Wide);
        assert_eq!(w.dropped, 0);
        for (r, h) in [
            (Row::Number, 36.0),
            (Row::Chain, 68.0),
            (Row::Eq, 22.0),
            (Row::PhaseSwap, 24.0),
            (Row::Pan, 28.0),
            (Row::MuteSolo, 24.0),
            (Row::Readout, 16.0),
            (Row::SendList, 28.0),
            (Row::Output, 14.0),
            (Row::Cables, 18.0),
        ] {
            assert_eq!(span(&w, r), h, "{r:?}");
        }
        assert_eq!(w.row(Row::Number), Some((0.0, 36.0)));
        assert_eq!(w.row(Row::Chain), Some((40.0, 108.0)));
        let (a, z) = w.travel().unwrap();
        assert!(z - a >= fader_min(WidthMode::Wide));
        assert!((w.height - 616.0).abs() < 0.5 && !w.scrolls);
        let mut last = -1.0;
        for r in ORDER {
            if let Some((a, b)) = w.row(r) {
                assert!(a >= last && b > a, "{r:?}");
                last = b;
            }
        }
        // No label column's rows, no separate meter, no FL row by default.
        for r in [Row::Name, Row::Meter, Row::Width, Row::FxRec, Row::Sends] {
            assert!(w.row(r).is_none(), "{r:?}");
        }
        let c = fit_at(616.0, WidthMode::Compact);
        assert_eq!(span(&c, Row::Number), 112.0);
        assert_eq!(span(&c, Row::Chain), 10.0);
        assert_eq!(
            span(&c, Row::MuteSolo),
            48.0,
            "M over S, 24 each (option C)"
        );
        assert_eq!(
            span(&c, Row::SendList),
            12.0,
            "a dot per send (the owner's choice 3)"
        );
        for r in [Row::Eq, Row::PhaseSwap, Row::Readout] {
            assert!(c.row(r).is_none(), "Compact has no {r:?}");
        }
        assert!(c.rotated);
        let l = fit_at(616.0, WidthMode::Xl);
        assert_eq!(span(&l, Row::Chain), 94.0);
        assert_eq!(span(&l, Row::SendList), 60.0);
        assert_eq!(l.row(Row::Chain).unwrap().0, 44.0 + GAP_LARGE);
        assert_eq!(l.send_rows, 3);
    }

    /// The short desk (`strips-*`: Wide at 440 keeps everything, at 288 the EQ, polarity,
    /// sends and socket leave and the inserts fold), then it scrolls.
    #[test]
    fn a_short_desk_drops_rows_in_des_mixers_order_then_scrolls() {
        // Option C's 24-pt keys: Wide holds every row from 460 (was 440).
        let tall = fit_at(460.0, WidthMode::Wide);
        assert_eq!(tall.dropped, 0, "Wide at 460: nothing leaves");
        // At rest the cable band is off (the owner's option C).
        let mut rest = RowSet::default();
        rest.set(Row::Cables, false);
        let docked = fit(
            288.0,
            WidthMode::Wide,
            rest,
            DEFAULT_RATIO,
            Wants::default(),
        );
        assert!(docked.row(Row::Eq).is_none() && docked.row(Row::PhaseSwap).is_none());
        assert!(docked.folded && span(&docked, Row::Chain) == INSERTS_FOLDED);
        assert!(docked.row(Row::SendList).is_none() && docked.row(Row::Cables).is_none());
        assert!(docked.row(Row::Readout).is_some() && docked.row(Row::Output).is_some());
        // Asked for, the band stays: the rows above it give way.
        let asked = fit_at(288.0, WidthMode::Wide);
        assert!(
            asked.row(Row::Cables).is_some(),
            "the band, when asked for, stays"
        );
        assert!(!docked.scrolls);
        let tiny = fit_at(150.0, WidthMode::Wide);
        assert_eq!(tiny.dropped, DROPS.len());
        assert!(tiny.row(Row::Output).is_none() && tiny.row(Row::Readout).is_none());
        assert!(tiny.scrolls && tiny.height > 150.0);
        assert!(tiny.row(Row::Pan).is_some() && tiny.row(Row::MuteSolo).is_some());
    }

    #[test]
    fn switched_off_rows_are_gone_and_fl_s_row_comes_back_on_request() {
        let mut shown = RowSet::default();
        shown.set(Row::Pan, false);
        shown.set(Row::Sends, true);
        let f = fit(900.0, WidthMode::Xl, shown, DEFAULT_RATIO, Wants::default());
        assert!(f.row(Row::Pan).is_none());
        let (out, fl, band) = (
            f.row(Row::Output).unwrap(),
            f.row(Row::Sends).unwrap(),
            f.row(Row::Cables).unwrap(),
        );
        assert!(
            out.1 <= fl.0 && fl.1 <= band.0,
            "FL's row between the output and the socket"
        );
        // The head and the fader cannot be switched off.
        let f = fit(
            900.0,
            WidthMode::Wide,
            RowSet(0),
            DEFAULT_RATIO,
            Wants::default(),
        );
        assert!(f.row(Row::Fader).is_some() && f.row(Row::Number).is_some());
        // The cable view off: no band.
        let mut shown = RowSet::default();
        shown.set(Row::Cables, false);
        let off = fit(
            900.0,
            WidthMode::Wide,
            shown,
            DEFAULT_RATIO,
            Wants::default(),
        );
        assert!(off.row(Row::Cables).is_none() && off.band == 0.0);
    }

    /// A desk saved before reads the same: the old rows keep their bits.
    #[test]
    fn saved_row_switches_keep_their_meaning() {
        let old = RowSet(!(1 << 11) & !(1 << 12) & !(1 << 9));
        assert!(!old.has(Row::Sends) && !old.has(Row::Chain) && !old.has(Row::Eq));
        assert!(old.has(Row::Output) && old.has(Row::SendList) && old.has(Row::Cables));
        let bits: std::collections::BTreeSet<u16> = Row::ALL.iter().map(|r| r.bit()).collect();
        assert_eq!(bits.len(), ROWS, "every row has its own bit");
        assert!(!Row::TOGGLES.contains(&Row::Cables));
        assert!(
            !RowSet::default().has(Row::Sends),
            "FL's row is off by default"
        );
    }

    #[test]
    fn width_modes_step_and_stop_at_the_ends() {
        assert_eq!(WidthMode::Compact.step(1), WidthMode::Wide);
        assert_eq!(WidthMode::Xl.step(1), WidthMode::Xl);
        assert_eq!(WidthMode::Compact.step(-1), WidthMode::Compact);
        assert!(WidthMode::Compact.rotated_names() && !WidthMode::Wide.rotated_names());
        assert_eq!(WidthMode::Wide.pitch(), 62.0);
        assert_eq!(CableView::default(), CableView::Off, "the owner's option C");
        assert_eq!(CableView::from_index(2), CableView::All);
    }

    /// The options default to the band as round 25 drew it, levels left out (des-mixer:
    /// the band is the sockets' row); the quick views are what they say.
    #[test]
    fn cable_options_default_to_everything_and_presets_say_what_they_are() {
        let d = CableOpts::default();
        assert_eq!(d.scope, CableScope::Both);
        assert!(d.outputs && d.sends && d.keys && d.master);
        let s = CablePreset::Sends.opts();
        assert!(s.sends && !s.outputs && !s.keys && s.scope == CableScope::Out);
        assert!(s.filtered());
        assert_eq!(s.preset(), Some(CablePreset::Sends));
        let f = CablePreset::Feeders.opts();
        assert_eq!(f.scope, CableScope::In);
        let mut o = d;
        o.toggle(CableKindSel::Keys);
        assert!(!o.keys && o.filtered() && o.preset().is_none());
        o.toggle(CableKindSel::Keys);
        assert_eq!(o, d);
        assert!(CableScope::Out.takes(true, false) && !CableScope::Out.takes(false, true));
        assert!(CableScope::In.takes(false, true) && !CableScope::In.takes(true, false));
        assert!(CableScope::Both.takes(true, false) && CableScope::Both.takes(false, true));
    }
}

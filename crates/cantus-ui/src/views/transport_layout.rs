//! The top bar's groups and where they go (ADR 0052, BACKLOG §6e; round 39, `dir-a`'s
//! phase B sheets) — a pure function of the bar's width, each group's widths and the
//! owner's choices, so a property test can sweep every width, zoom, language and set of
//! groups.
//!
//! - **One row.** The bar is 44 high; every group sits on its centre line, 10 apart, 12 from
//!   the edges. The **left cluster** — the mark with the menus and the project's name, then
//!   the groups up to the first one that belongs on the right (`Group::right`) — packs from
//!   the left edge; the **right cluster** — those groups, then Settings and **More** —
//!   packs from the right edge. At least 24 lie between the two.
//! - **Forms.** A group is drawn full, or compact where it has a compact form (the tempo
//!   without its metre, the clock without its time, the grid as its magnet, the meter
//!   without its label, the device as its speaker); the menus block is full, compact (no
//!   summary), minimal (the mark, a ≡ key and the name) or bare (the mark and ≡).
//! - **Folding** follows one fixed order ([`STEPS`], the sheet's `FOLD`): while the bar does
//!   not fit, the next step makes a group compact or folds it into the **More** panel,
//!   where it stays usable. A group is on the bar in one of its forms or in the panel:
//!   nothing is ever clipped, nothing overlaps.

/// One group of the bar: one function, separated from the next by the kit's divider.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    /// FL's PAT / SONG switch.
    Mode,
    /// To start, play, stop, record (and its options), loop.
    Transport,
    /// The tempo, tap tempo, the time signature.
    Tempo,
    /// The metronome, its count-in, its level and its options.
    Metronome,
    /// The song position in bars.beats.ticks and in time.
    Position,
    /// The focused editor's grid and snap.
    Snap,
    /// Undo and redo.
    Undo,
    /// The areas' keys: browser, arrangement, rack, piano roll, mixer, inspector.
    Windows,
    /// CPU, RAM, voices, dropouts.
    Performance,
    /// The master meter, peak or loudness.
    Meter,
    /// The typing keyboard, MIDI input, armed tracks.
    Inputs,
    /// The audio device's chip.
    Device,
    /// The element picker, the appearance page, the interface's zoom.
    Tools,
    /// Ableton Link (on/off, its peers, the quantum) and the sync source with its tempo
    /// and lock light (round 33, ADR 0078–0079).
    Sync,
    /// The agent chip while an agent is connected (round 34, ADR 0081). // r34:mcp
    Ai, // r34:mcp
}

/// How many groups there are.
pub const COUNT: usize = 15; // r34:mcp (15)

impl Group {
    /// Every group, in the default order (round 39, `dir-a`): on the left PAT / SONG, the
    /// transport, the metronome, the tempo, the position, the grid, undo and the windows;
    /// on the right the agent chip and sync when shown, the system meters, the inputs, the
    /// master meter, the audio device and the tools.
    pub const ALL: [Group; COUNT] = [
        Group::Mode,
        Group::Transport,
        Group::Metronome,
        Group::Tempo,
        Group::Position,
        Group::Snap,
        Group::Undo,
        Group::Windows,
        Group::Ai, // r34:mcp
        Group::Sync,
        Group::Performance,
        Group::Inputs,
        Group::Meter,
        Group::Device,
        Group::Tools,
    ];

    /// The stable id kept in `settings.json`.
    pub fn id(self) -> &'static str {
        match self {
            Group::Mode => "mode",
            Group::Transport => "transport",
            Group::Tempo => "tempo",
            Group::Metronome => "metronome",
            Group::Position => "position",
            Group::Snap => "snap",
            Group::Undo => "undo",
            Group::Windows => "windows",
            Group::Performance => "performance",
            Group::Meter => "meter",
            Group::Inputs => "inputs",
            Group::Device => "device",
            Group::Tools => "tools",
            Group::Sync => "sync",
            Group::Ai => "ai", // r34:mcp
        }
    }

    pub fn from_id(id: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|g| g.id() == id)
    }

    /// Its name, in the customise menu and the overflow menu.
    pub fn name_key(self) -> &'static str {
        match self {
            Group::Mode => "topbar.group.mode",
            Group::Transport => "topbar.group.transport",
            Group::Tempo => "topbar.group.tempo",
            Group::Metronome => "topbar.group.metronome",
            Group::Position => "topbar.group.position",
            Group::Snap => "topbar.group.snap",
            Group::Undo => "topbar.group.undo",
            Group::Windows => "topbar.group.windows",
            Group::Performance => "topbar.group.performance",
            Group::Meter => "topbar.group.meter",
            Group::Inputs => "topbar.group.inputs",
            Group::Device => "topbar.group.device",
            Group::Tools => "topbar.group.tools",
            Group::Sync => "sync.group",
            Group::Ai => "ai.topbar.group", // r34:mcp
        }
    }

    /// Whether the default bar leaves it out (round 33: Sync, as Live hides its Link toggle
    /// until asked) — shown when the owner shows it, or while it is in use.
    pub fn hidden_by_default(self) -> bool {
        matches!(self, Group::Sync | Group::Ai) // r34:mcp (Ai)
    }

    /// Whether the bar shows it: not hidden by the owner, and — hidden by default — shown
    /// by the owner or `in_use`.
    pub fn is_shown(self, hidden: &[String], shown: &[String], in_use: bool) -> bool {
        if hidden.iter().any(|h| h == self.id()) {
            return false;
        }
        !self.hidden_by_default() || in_use || shown.iter().any(|h| h == self.id())
    }

    /// Whether it belongs to the bar's right cluster (round 39): the readings of the
    /// session — the system meters, the inputs, the master meter, the device — and the
    /// tools. The right cluster starts at the first such group in the owner's order.
    pub fn right(self) -> bool {
        matches!(
            self,
            Group::Ai
                | Group::Sync
                | Group::Performance
                | Group::Inputs
                | Group::Meter
                | Group::Device
                | Group::Tools
        )
    }
}

/// The owner's groups in their order, from the ids kept in the settings: unknown ids are
/// dropped, repeats kept once, and a group the list does not name (one newer than the
/// file) goes in after its default predecessor. Into a fixed array: no allocation (the
/// bar resolves it every frame).
pub fn resolve_into(order: &[String], out: &mut [Option<Group>; COUNT]) -> usize {
    *out = [None; COUNT];
    let mut n = 0;
    let has = |out: &[Option<Group>; COUNT], n: usize, g: Group| out[..n].contains(&Some(g));
    for id in order {
        if let Some(g) = Group::from_id(id) {
            if !has(out, n, g) {
                out[n] = Some(g);
                n += 1;
            }
        }
    }
    for (i, g) in Group::ALL.iter().enumerate() {
        if has(out, n, *g) {
            continue;
        }
        let at = Group::ALL[..i]
            .iter()
            .rev()
            .find_map(|p| out[..n].iter().position(|x| *x == Some(*p)))
            .map_or(0, |p| p + 1);
        out.copy_within(at..n, at + 1);
        out[at] = Some(*g);
        n += 1;
    }
    n
}

/// [`resolve_into`] as a list (the customise panel, tests).
pub fn resolve(order: &[String]) -> Vec<Group> {
    let mut out = [None; COUNT];
    let n = resolve_into(order, &mut out);
    out[..n].iter().flatten().copied().collect()
}

/// One group to place: its width compact (`min`) and full (`max`, the same when it has no
/// compact form). Every group takes the bar's height (`tall`, kept for the tests' sake:
/// round 39's bar is one row).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Want {
    pub group: Group,
    pub min: f32,
    pub max: f32,
    pub tall: bool,
}

/// The menus block's forms, widest first.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Brand {
    /// The mark, the menus over the project's name and its summary.
    #[default]
    Full,
    /// Without the summary.
    Compact,
    /// The mark, a ≡ key holding the menus, the name.
    Minimal,
    /// The mark and the ≡ key.
    Bare,
}

/// What the layout is given, in points.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Room {
    /// The bar's width.
    pub width: f32,
    /// The margin at each end (12 at 100 %).
    pub pad: f32,
    /// The space between two groups (10).
    pub gap: f32,
    /// The least space between the two clusters (24).
    pub split: f32,
    /// The menus block in each of its forms ([`Brand`], widest first): `left_max` is the
    /// full form, `left_min` the bare one.
    pub left_min: f32,
    pub left_max: f32,
    pub left_compact: f32,
    pub left_minimal: f32,
    /// The More key's width (30) and the Settings key's (28).
    pub more: f32,
    pub gear: f32,
}

impl Room {
    /// The menus block's width in `form`.
    pub fn brand(&self, form: Brand) -> f32 {
        match form {
            Brand::Full => self.left_max,
            Brand::Compact => self.left_compact,
            Brand::Minimal => self.left_minimal,
            Brand::Bare => self.left_min,
        }
    }
}

/// A span of the bar, `[x0, x1)` from its left edge.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub x0: f32,
    pub x1: f32,
}

impl Span {
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }
}

/// Where in the bar's height a group sits: round 39's bar is one row, so always the whole
/// of it (kept so the tests' overlap check reads as before).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Slot {
    #[default]
    Full,
    Top,
    Bottom,
}

/// A group placed on the bar.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Placed {
    pub group: Group,
    pub span: Span,
    pub slot: Slot,
    /// Drawn in its compact form.
    pub compact: bool,
}

/// One step of the fold order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// The group leaves the bar for the More panel.
    Fold(Group),
    /// The group takes its compact form (a step that changes nothing when it has none).
    Compact(Group),
    /// The menus block takes this form.
    Brand(Brand),
}

/// The fold order (`dir-a`'s `topbar2.FOLD`, approved 2026-11-09), then the steps a bar
/// narrower than the sheet's 800 px needs (a large interface size): the transport is the
/// last thing to leave. Sync, shown only when asked for or in use, goes first.
pub const STEPS: [Step; 23] = [
    Step::Fold(Group::Sync),
    Step::Fold(Group::Tools),
    Step::Fold(Group::Inputs),
    Step::Fold(Group::Performance),
    Step::Fold(Group::Windows),
    Step::Compact(Group::Device),
    Step::Fold(Group::Metronome),
    Step::Compact(Group::Meter),
    Step::Compact(Group::Tempo),
    Step::Compact(Group::Position),
    Step::Brand(Brand::Compact),
    Step::Fold(Group::Mode),
    Step::Compact(Group::Snap),
    Step::Brand(Brand::Minimal),
    Step::Fold(Group::Undo),
    Step::Fold(Group::Meter),
    // Past the sheet: a bar narrower than 800 px at 100 %.
    Step::Fold(Group::Ai),
    Step::Fold(Group::Device),
    Step::Fold(Group::Snap),
    Step::Brand(Brand::Bare),
    Step::Fold(Group::Tempo),
    Step::Fold(Group::Position),
    Step::Fold(Group::Transport),
];

/// Where everything went. Fixed arrays: laying the bar out allocates nothing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    /// The menus block, and its form.
    pub left: Span,
    pub brand: Brand,
    /// The groups on the bar, in the owner's order; the first `placed_len` are used.
    pub placed: [Placed; COUNT],
    pub placed_len: usize,
    /// The groups in the More panel, in the owner's order; the first `folded_len` are used.
    pub folded: [Group; COUNT],
    pub folded_len: usize,
    /// The Settings key and the More key, at the bar's right end.
    pub gear: Span,
    pub more: Span,
}

impl Layout {
    pub fn placed(&self) -> &[Placed] {
        &self.placed[..self.placed_len]
    }

    pub fn folded(&self) -> &[Group] {
        &self.folded[..self.folded_len]
    }

    /// The groups on the bar in their compact form.
    pub fn compact(&self) -> impl Iterator<Item = Group> + '_ {
        self.placed().iter().filter(|p| p.compact).map(|p| p.group)
    }

    pub fn span_of(&self, g: Group) -> Option<Span> {
        self.placed().iter().find(|p| p.group == g).map(|p| p.span)
    }

    pub fn slot_of(&self, g: Group) -> Option<Slot> {
        self.placed().iter().find(|p| p.group == g).map(|p| p.slot)
    }

    pub fn is_compact(&self, g: Group) -> bool {
        self.placed().iter().any(|p| p.group == g && p.compact)
    }
}

/// The width the bar needs: the menus block in `brand`, each group that is `on` in its
/// form, the gaps, the space between the clusters, the Settings and More keys.
fn need_of(
    room: &Room,
    wants: &[Want],
    on: &[bool; COUNT],
    compact: &[bool; COUNT],
    brand: Brand,
) -> f32 {
    let split = split_at(wants);
    let (mut sum, mut left, mut right) = (room.brand(brand), 1usize, 2usize);
    for (i, w) in wants.iter().enumerate() {
        if !on[i] {
            continue;
        }
        sum += if compact[i] { w.min } else { w.max };
        if i < split {
            left += 1;
        } else {
            right += 1;
        }
    }
    room.pad * 2.0
        + sum
        + room.more
        + room.gear
        + room.gap * ((left - 1) + (right - 1)) as f32
        + room.split
}

/// Where the right cluster starts: the first group of the owner's order that belongs on
/// the right.
fn split_at(wants: &[Want]) -> usize {
    wants
        .iter()
        .position(|w| w.group.right())
        .unwrap_or(wants.len())
}

/// Lay the bar out: `wants` are the shown groups in the owner's order. The steps of
/// [`STEPS`] are taken in turn while the bar does not fit.
pub fn layout(room: &Room, wants: &[Want]) -> Layout {
    let n = wants.len().min(COUNT);
    let wants = &wants[..n];
    let mut on = [true; COUNT];
    let mut compact = [false; COUNT];
    let mut brand = Brand::Full;
    let index = |g: Group| wants.iter().position(|w| w.group == g);
    for step in STEPS {
        if need_of(room, wants, &on, &compact, brand) <= room.width {
            break;
        }
        match step {
            Step::Fold(g) => {
                if let Some(i) = index(g) {
                    on[i] = false;
                }
            }
            Step::Compact(g) => {
                if let Some(i) = index(g) {
                    compact[i] = wants[i].min < wants[i].max;
                }
            }
            Step::Brand(b) => brand = brand.max(b),
        }
    }
    let width_of = |i: usize| {
        if compact[i] {
            wants[i].min
        } else {
            wants[i].max
        }
    };
    let split = split_at(wants);
    let empty = Placed {
        group: Group::Transport,
        span: Span::default(),
        slot: Slot::Full,
        compact: false,
    };
    let mut out = Layout {
        left: Span {
            x0: room.pad,
            x1: room.pad + room.brand(brand),
        },
        brand,
        placed: [empty; COUNT],
        placed_len: 0,
        folded: [Group::Transport; COUNT],
        folded_len: 0,
        gear: Span::default(),
        more: Span {
            x0: room.width - room.pad - room.more,
            x1: room.width - room.pad,
        },
    };
    out.gear = Span {
        x0: out.more.x0 - room.gap - room.gear,
        x1: out.more.x0 - room.gap,
    };
    // The right cluster from the right edge: its spans, before they are put in order.
    let mut right = [Span::default(); COUNT];
    let mut x = out.gear.x0;
    for i in (split..n).rev() {
        if on[i] {
            let w = width_of(i);
            right[i] = Span {
                x0: x - room.gap - w,
                x1: x - room.gap,
            };
            x = right[i].x0;
        }
    }
    let mut x = out.left.x1;
    for i in 0..n {
        if !on[i] {
            out.folded[out.folded_len] = wants[i].group;
            out.folded_len += 1;
            continue;
        }
        let span = if i < split {
            let w = width_of(i);
            let s = Span {
                x0: x + room.gap,
                x1: x + room.gap + w,
            };
            x = s.x1;
            s
        } else {
            right[i]
        };
        out.placed[out.placed_len] = Placed {
            group: wants[i].group,
            span,
            slot: Slot::Full,
            compact: compact[i],
        };
        out.placed_len += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sheet's measures (`b-topbar-groups-*.png`): the menus block 238 / 238 / 124.
    fn room(width: f32) -> Room {
        Room {
            width,
            pad: 12.0,
            gap: 10.0,
            split: 24.0,
            left_min: 60.0,
            left_max: 238.0,
            left_compact: 238.0,
            left_minimal: 124.0,
            more: 30.0,
            gear: 28.0,
        }
    }

    /// The sheet's groups (no agent, no sync), full and compact.
    fn wants() -> Vec<Want> {
        let w = |g: Group| -> (f32, f32) {
            match g {
                Group::Mode => (74.0, 74.0),
                Group::Transport => (160.0, 160.0),
                Group::Metronome => (46.0, 46.0),
                Group::Tempo => (78.0, 116.0),
                Group::Position => (73.0, 124.0),
                Group::Snap => (40.0, 80.0),
                Group::Undo => (56.0, 56.0),
                Group::Windows => (196.0, 196.0),
                Group::Performance => (142.0, 142.0),
                Group::Inputs => (112.0, 112.0),
                Group::Meter => (92.0, 132.0),
                Group::Device => (30.0, 104.0),
                Group::Tools => (84.0, 84.0),
                _ => (0.0, 0.0),
            }
        };
        Group::ALL
            .iter()
            .filter(|g| !g.hidden_by_default())
            .map(|&group| {
                let (min, max) = w(group);
                Want {
                    group,
                    min,
                    max,
                    tall: true,
                }
            })
            .collect()
    }

    /// Every width the sheet draws (`b-topbar-widths-*.png`): what folded, what went compact.
    #[test]
    fn the_bar_folds_as_the_sheet_draws_it() {
        use Group::*;
        let cases: [(f32, &[Group], &[Group], Brand); 8] = [
            (1920.0, &[], &[], Brand::Full),
            (1600.0, &[Performance, Inputs, Tools], &[], Brand::Full),
            (
                1440.0,
                &[Windows, Performance, Inputs, Tools],
                &[],
                Brand::Full,
            ),
            (
                1280.0,
                &[Windows, Performance, Inputs, Tools],
                &[Device],
                Brand::Full,
            ),
            (
                1152.0,
                &[Metronome, Windows, Performance, Inputs, Tools],
                &[Tempo, Meter, Device],
                Brand::Full,
            ),
            (
                1024.0,
                &[Mode, Metronome, Windows, Performance, Inputs, Tools],
                &[Tempo, Position, Meter, Device],
                Brand::Compact,
            ),
            (
                900.0,
                &[Mode, Metronome, Windows, Performance, Inputs, Tools],
                &[Tempo, Position, Snap, Meter, Device],
                Brand::Minimal,
            ),
            (
                800.0,
                &[Mode, Metronome, Undo, Windows, Performance, Inputs, Tools],
                &[Tempo, Position, Snap, Meter, Device],
                Brand::Minimal,
            ),
        ];
        for (width, folded, compact, brand) in cases {
            let l = layout(&room(width), &wants());
            let mut f = l.folded().to_vec();
            f.sort();
            let mut want = folded.to_vec();
            want.sort();
            assert_eq!(f, want, "{width}: folded");
            let mut c: Vec<Group> = l.compact().collect();
            c.sort();
            let mut want = compact.to_vec();
            want.sort();
            assert_eq!(c, want, "{width}: compact");
            assert_eq!(l.brand, brand, "{width}: the menus");
        }
    }

    /// The clusters: the left packs from the left edge 10 apart, the right from the right
    /// edge, Settings and More last; at least 24 between them; nothing overlaps.
    #[test]
    fn the_clusters_pack_from_their_edges() {
        for width in [800.0, 1024.0, 1280.0, 1600.0, 1920.0, 2560.0] {
            let l = layout(&room(width), &wants());
            assert_eq!(l.left.x0, 12.0);
            assert_eq!(l.more.x1, width - 12.0);
            assert_eq!(l.gear.x1 + 10.0, l.more.x0);
            let mut spans: Vec<Span> = vec![l.left];
            spans.extend(l.placed().iter().map(|p| p.span));
            spans.push(l.gear);
            spans.push(l.more);
            for w in spans.windows(2) {
                assert!(w[0].x1 <= w[1].x0 + 1e-3, "{width}: {spans:?}");
            }
            let last_left = l
                .placed()
                .iter()
                .filter(|p| !p.group.right())
                .map(|p| p.span.x1)
                .fold(l.left.x1, f32::max);
            let first_right = l
                .placed()
                .iter()
                .filter(|p| p.group.right())
                .map(|p| p.span.x0)
                .fold(l.gear.x0, f32::min);
            assert!(first_right - last_left >= 24.0 - 1e-3, "{width}: {l:?}");
        }
    }

    #[test]
    fn a_very_narrow_bar_keeps_the_transport_last() {
        let l = layout(&room(360.0), &wants());
        assert!(l.span_of(Group::Transport).is_some(), "{l:?}");
        assert_eq!(l.brand, Brand::Bare);
        assert_eq!(l.placed().len(), 1);
    }

    #[test]
    fn an_old_or_odd_order_resolves_to_every_group_once() {
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(resolve(&[]), Group::ALL.to_vec());
        let r = resolve(&ids(&["meter", "nonsense", "transport", "meter"]));
        assert_eq!(r.len(), COUNT);
        let mut sorted = r.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), COUNT, "each group once: {r:?}");
        let at = |g: Group| r.iter().position(|x| *x == g).unwrap();
        assert!(
            at(Group::Meter) < at(Group::Transport),
            "the owner's order kept"
        );
        // A group the file does not name goes after its default predecessor: the
        // metronome after the transport, the tempo after it (round 39's order).
        assert_eq!(at(Group::Metronome), at(Group::Transport) + 1);
        assert_eq!(at(Group::Tempo), at(Group::Metronome) + 1);
    }
}

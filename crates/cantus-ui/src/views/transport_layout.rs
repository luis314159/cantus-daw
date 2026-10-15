//! The top bar's groups and where they go (ADR 0052, BACKLOG §6e) — a pure function of
//! the bar's width, each group's width and the owner's choices, so a property test can
//! sweep every width, zoom, language and set of groups.
//!
//! - **The left block** — the logo, the menus over the project's name and the hint panel —
//!   is always there; it takes what room is left, between its least and its most.
//! - **The groups** follow in the owner's order (`TopBarSettings`), each at its own width;
//!   a hidden one is left out. FL's strip has two rows: so has this one — a group with
//!   two lines of its own (PAT/SONG, the metronome, the clock, CPU/RAM, the meter) takes
//!   the bar's height; the others are one row high and stack two to a column (the
//!   transport over the tempo, the snap over undo, the windows over the inputs, the audio
//!   chip over the tools).
//! - **Folding.** When the groups do not fit, the least important leaves the bar first and
//!   goes into the **⋯ overflow menu** at the bar's right end, where it stays usable (it is
//!   drawn there by the same code at the same size); any folded group that fits again once
//!   others have folded comes back. Folding never shrinks a group: a group
//!   is either on the bar whole or in the menu, so nothing is ever clipped and nothing
//!   overlaps.
//! - **Room to spare** goes to the left block (up to its most), then to the groups that can
//!   grow — the master meter, then the audio chip (its column widens with it) — and what is
//!   still left stays at the bar's right end.

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
}

/// How many groups there are.
pub const COUNT: usize = 13;

impl Group {
    /// Every group, in the default order (FL's: PAT / SONG before play, the tempo beside
    /// the transport, the position after the recording toggles, the snap and the windows
    /// after it, the meters at the end).
    pub const ALL: [Group; COUNT] = [
        Group::Mode,
        Group::Transport,
        Group::Tempo,
        Group::Metronome,
        Group::Position,
        Group::Snap,
        Group::Undo,
        Group::Windows,
        Group::Inputs,
        Group::Performance,
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
        }
    }

    /// Whether it takes the bar's whole height (else one of its two rows, shared with the
    /// next short group in the order).
    pub fn tall(self) -> bool {
        matches!(
            self,
            Group::Mode | Group::Metronome | Group::Position | Group::Performance | Group::Meter
        )
    }

    /// How much it matters on the bar: 0 is kept the longest. A narrow window folds the
    /// highest first.
    pub fn importance(self) -> u8 {
        match self {
            Group::Transport => 0,
            Group::Position => 1,
            Group::Tempo => 2,
            Group::Mode => 3,
            Group::Metronome => 4,
            Group::Device => 5,
            Group::Meter => 6,
            Group::Windows => 7,
            Group::Performance => 8,
            Group::Snap => 9,
            Group::Undo => 10,
            Group::Inputs => 11,
            Group::Tools => 12,
        }
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

/// One group to place: its width at least, and at most (the same unless it can grow), and
/// whether it takes the bar's whole height.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Want {
    pub group: Group,
    pub min: f32,
    pub max: f32,
    pub tall: bool,
}

/// What the layout is given, in points.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Room {
    /// The bar's width.
    pub width: f32,
    /// The margin at each end.
    pub pad: f32,
    /// The space between two columns (a divider sits in its middle).
    pub gap: f32,
    /// The left block's least (menus over the hint panel, the hint panel at its least) and
    /// most.
    pub left_min: f32,
    pub left_max: f32,
    /// The ⋯ key's width.
    pub more: f32,
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

/// Where in the bar's height a group sits: the whole of it, or one of its two rows.
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
}

/// Where everything went. Fixed arrays: laying the bar out allocates nothing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    pub left: Span,
    /// The groups on the bar, left to right; the first `placed_len` are used.
    pub placed: [Placed; COUNT],
    pub placed_len: usize,
    /// Where each column of groups starts (a divider goes before it); the first
    /// `columns_len` are used.
    pub columns: [f32; COUNT],
    pub columns_len: usize,
    /// The groups in the ⋯ menu, in the bar's order; the first `folded_len` are used.
    pub folded: [Group; COUNT],
    pub folded_len: usize,
    pub more: Span,
}

impl Layout {
    pub fn placed(&self) -> &[Placed] {
        &self.placed[..self.placed_len]
    }

    pub fn folded(&self) -> &[Group] {
        &self.folded[..self.folded_len]
    }

    pub fn columns(&self) -> &[f32] {
        &self.columns[..self.columns_len]
    }

    pub fn span_of(&self, g: Group) -> Option<Span> {
        self.placed().iter().find(|p| p.group == g).map(|p| p.span)
    }

    pub fn slot_of(&self, g: Group) -> Option<Slot> {
        self.placed().iter().find(|p| p.group == g).map(|p| p.slot)
    }
}

/// The columns the groups that are `on` make, in order: a tall group is a column of its
/// own; a short one goes under the short group before it when that one is alone in its
/// column, else it starts a column. Calls `f(want index, column index, slot)` per group
/// and returns each column's least width (and how many columns).
fn columns(
    wants: &[Want],
    on: &[bool; COUNT],
    mut f: impl FnMut(usize, usize, Slot),
) -> ([f32; COUNT], usize) {
    let mut w = [0f32; COUNT];
    let mut n = 0usize;
    let mut open_top = false;
    for (i, want) in wants.iter().enumerate() {
        if !on[i] {
            continue;
        }
        if want.tall {
            w[n] = want.min;
            f(i, n, Slot::Full);
            n += 1;
            open_top = false;
        } else if open_top {
            w[n - 1] = w[n - 1].max(want.min);
            f(i, n - 1, Slot::Bottom);
            open_top = false;
        } else {
            w[n] = want.min;
            f(i, n, Slot::Top);
            n += 1;
            open_top = true;
        }
    }
    (w, n)
}

/// Lay the bar out: `wants` are the shown groups in the owner's order.
///
/// The least important group folds into ⋯ until the rest fits; then, most important first,
/// any folded group that fits again comes back (a fold can free more room than the next
/// group needs). Room to spare goes to the left block, then to the groups that can grow.
/// The width the bar needs with the groups that are `on`, everything at its least.
fn need_of(room: &Room, wants: &[Want], on: &[bool; COUNT]) -> f32 {
    let (w, cols) = columns(wants, on, |_, _, _| {});
    room.pad * 2.0
        + room.left_min
        + room.gap
        + room.more
        + w[..cols].iter().map(|c| c + room.gap).sum::<f32>()
}

pub fn layout(room: &Room, wants: &[Want]) -> Layout {
    let n = wants.len().min(COUNT);
    let wants = &wants[..n];
    let mut on = [true; COUNT];
    let need = |on: &[bool; COUNT]| need_of(room, wants, on);
    while need(&on) > room.width {
        let Some(i) = (0..n)
            .filter(|&i| on[i])
            .max_by_key(|&i| wants[i].group.importance())
        else {
            break;
        };
        on[i] = false;
    }
    let mut back: [usize; COUNT] = [0; COUNT];
    let mut nb = 0;
    for i in (0..n).filter(|&i| !on[i]) {
        back[nb] = i;
        nb += 1;
    }
    back[..nb].sort_by_key(|&i| wants[i].group.importance());
    for &i in &back[..nb] {
        on[i] = true;
        if need(&on) > room.width {
            on[i] = false;
        }
    }
    let mut slack = (room.width - need(&on)).max(0.0);
    let left_w = room.left_min + slack.min((room.left_max - room.left_min).max(0.0));
    slack -= left_w - room.left_min;
    let mut out = Layout {
        left: Span {
            x0: room.pad,
            x1: room.pad + left_w,
        },
        placed: [Placed {
            group: Group::Transport,
            span: Span::default(),
            slot: Slot::Full,
        }; COUNT],
        placed_len: 0,
        columns: [0.0; COUNT],
        columns_len: 0,
        folded: [Group::Transport; COUNT],
        folded_len: 0,
        more: Span {
            x0: room.width - room.pad - room.more,
            x1: room.width - room.pad,
        },
    };
    let mut slots = [(0usize, Slot::Full); COUNT];
    let (mut widths, cols) = columns(wants, &on, |i, c, s| slots[i] = (c, s));
    // A tall group that can grow takes what is spare.
    for (i, w) in wants.iter().enumerate() {
        if on[i] && w.tall && slots[i].1 == Slot::Full {
            let grow = slack.min((w.max - w.min).max(0.0));
            slack -= grow;
            widths[slots[i].0] += grow;
        }
    }
    // Then a short one that can (the audio chip), widening its column up to its most.
    for (i, w) in wants.iter().enumerate() {
        if on[i] && !w.tall && w.max > w.min {
            let c = slots[i].0;
            let grow = slack.min((w.max - widths[c]).max(0.0));
            slack -= grow;
            widths[c] += grow;
        }
    }
    let mut x0 = [0f32; COUNT];
    let mut x = out.left.x1 + room.gap;
    for c in 0..cols {
        x0[c] = x;
        out.columns[c] = x;
        x += widths[c] + room.gap;
    }
    out.columns_len = cols;
    for (i, w) in wants.iter().enumerate() {
        if !on[i] {
            out.folded[out.folded_len] = w.group;
            out.folded_len += 1;
            continue;
        }
        let (c, slot) = slots[i];
        let width = if slot == Slot::Full {
            widths[c]
        } else {
            w.min.max(widths[c].min(w.max))
        };
        out.placed[out.placed_len] = Placed {
            group: w.group,
            span: Span {
                x0: x0[c],
                x1: x0[c] + width,
            },
            slot,
        };
        out.placed_len += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(width: f32) -> Room {
        Room {
            width,
            pad: 8.0,
            gap: 10.0,
            left_min: 200.0,
            left_max: 400.0,
            more: 24.0,
        }
    }

    fn wants() -> Vec<Want> {
        Group::ALL
            .iter()
            .map(|&group| Want {
                group,
                min: 60.0,
                max: if group == Group::Meter { 160.0 } else { 60.0 },
                tall: group.tall(),
            })
            .collect()
    }

    #[test]
    fn short_groups_share_a_column_two_high() {
        let l = layout(&room(4000.0), &wants());
        assert_eq!(l.slot_of(Group::Transport), Some(Slot::Top));
        assert_eq!(l.slot_of(Group::Tempo), Some(Slot::Bottom));
        assert_eq!(
            l.span_of(Group::Transport).unwrap().x0,
            l.span_of(Group::Tempo).unwrap().x0
        );
        assert_eq!(l.slot_of(Group::Metronome), Some(Slot::Full));
        // 13 groups, 5 tall, 8 short in 4 pairs: 9 columns.
        assert_eq!(l.columns().len(), 9);
    }

    #[test]
    fn a_narrow_bar_folds_the_least_important_first() {
        // Every group a column of its own, all as wide: each return costs the same, so the
        // folded are exactly the least important.
        let tall: Vec<Want> = wants()
            .into_iter()
            .map(|w| Want { tall: true, ..w })
            .collect();
        let l = layout(&room(700.0), &tall);
        assert!(!l.placed().is_empty() && !l.folded().is_empty(), "{l:?}");
        for g in l.folded() {
            for p in l.placed() {
                assert!(
                    g.importance() > p.group.importance(),
                    "{g:?} folded while {:?} stays",
                    p.group
                );
            }
        }
        // The default bar, two rows: a less important group may come back where it costs
        // nothing (Tools under the chip), but no folded group would fit if put back, and
        // the most important stay.
        let w = wants();
        let l = layout(&room(700.0), &w);
        let mut on = [false; COUNT];
        for p in l.placed() {
            on[w.iter().position(|x| x.group == p.group).unwrap()] = true;
        }
        for g in l.folded() {
            let mut back = on;
            back[w.iter().position(|x| x.group == *g).unwrap()] = true;
            assert!(
                need_of(&room(700.0), &w, &back) > 700.0,
                "{g:?} would fit: {l:?}"
            );
        }
        for g in [Group::Transport, Group::Position, Group::Tempo, Group::Mode] {
            assert!(l.span_of(g).is_some(), "{g:?} folded: {l:?}");
        }
        assert_eq!(l.placed().len() + l.folded().len(), COUNT);
    }

    /// The audio chip (a short group that can grow) widens its column with what is spare,
    /// after the left block and the meter; its partner keeps its own width.
    #[test]
    fn a_short_group_that_can_grow_widens_its_column() {
        let mut w = wants();
        w.iter_mut().find(|x| x.group == Group::Device).unwrap().max = 200.0;
        let l = layout(&room(4000.0), &w);
        assert_eq!(l.span_of(Group::Device).unwrap().width(), 200.0);
        assert_eq!(l.span_of(Group::Tools).unwrap().width(), 60.0);
        // Short of room: at its least.
        let all = need_of(&room(0.0), &w, &[true; COUNT]);
        let l = layout(&room(all + 200.0 + 100.0 + 30.0), &w);
        assert_eq!(l.left.width(), 400.0);
        assert_eq!(l.span_of(Group::Meter).unwrap().width(), 160.0);
        assert_eq!(l.span_of(Group::Device).unwrap().width(), 90.0);
    }

    /// A fold that frees more room than the next group needs lets a folded group back.
    #[test]
    fn a_folded_group_comes_back_when_room_frees() {
        let mut w = wants();
        // A wide, unimportant tall group (Meter is 6; make Tools tall-ish and wide).
        w.iter_mut()
            .find(|x| x.group == Group::Performance)
            .unwrap()
            .min = 400.0;
        let l = layout(&room(1100.0), &w);
        // Performance (wide) folds; Undo and Inputs (narrow, less important) come back.
        assert!(l.folded().contains(&Group::Performance), "{l:?}");
        assert!(l.span_of(Group::Undo).is_some(), "{l:?}");
    }

    #[test]
    fn room_to_spare_goes_to_the_left_block_then_the_meter() {
        let l = layout(&room(4000.0), &wants());
        assert_eq!(l.folded().len(), 0);
        assert_eq!(l.left.width(), 400.0);
        assert_eq!(l.span_of(Group::Meter).unwrap().width(), 160.0);
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
        // A group the file does not name goes after its default predecessor: Tempo after
        // Transport, Mode (which has none) first.
        assert_eq!(at(Group::Tempo), at(Group::Transport) + 1);
        assert_eq!(r[0], Group::Mode);
    }
}

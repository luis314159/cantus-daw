//! The top bar's groups and where they go (ADR 0052, BACKLOG §6e) — a pure function of
//! the bar's width, each group's width and the owner's choices, so a property test can
//! sweep every width, zoom, language and set of groups.
//!
//! - **The left block** — the logo, the menus over the project's name and the hint panel —
//!   is always there; it takes what room is left, between its least and its most.
//! - **The groups** follow in the owner's order (`TopBarSettings`), each at its own width;
//!   a hidden one is left out.
//! - **Folding.** When the groups do not fit, the least important leaves the bar first and
//!   goes into the **⋯ overflow menu** at the bar's right end, where it stays usable (it is
//!   drawn there by the same code at the same size). Folding never shrinks a group: a group
//!   is either on the bar whole or in the menu, so nothing is ever clipped and nothing
//!   overlaps.
//! - **Room to spare** goes to the left block (up to its most), then to the groups that can
//!   grow (the master meter), and what is still left stays at the bar's right end.

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

/// Groups more important than this stay on the bar while the menus fold into one key.
pub const MENUS_IMPORTANCE: u8 = 3;

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
        Group::Performance,
        Group::Meter,
        Group::Inputs,
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
            Group::Performance => 7,
            Group::Snap => 8,
            Group::Undo => 9,
            Group::Windows => 10,
            Group::Inputs => 11,
            Group::Tools => 12,
        }
    }
}

/// The owner's groups in their order, from the ids kept in the settings: unknown ids are
/// dropped, repeats kept once, and a group the list does not name (one newer than the
/// file) goes in after its default predecessor.
pub fn resolve(order: &[String]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::with_capacity(COUNT);
    for id in order {
        if let Some(g) = Group::from_id(id) {
            if !out.contains(&g) {
                out.push(g);
            }
        }
    }
    for (i, g) in Group::ALL.iter().enumerate() {
        if out.contains(g) {
            continue;
        }
        let after = Group::ALL[..i]
            .iter()
            .rev()
            .find_map(|p| out.iter().position(|x| x == p));
        out.insert(after.map_or(0, |p| p + 1), *g);
    }
    out
}

/// One group to place: its width at least, and at most (the same unless it can grow).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Want {
    pub group: Group,
    pub min: f32,
    pub max: f32,
}

/// What the layout is given, in points.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Room {
    /// The bar's width.
    pub width: f32,
    /// The margin at each end.
    pub pad: f32,
    /// The space between two groups (a divider sits in its middle).
    pub gap: f32,
    /// The left block's least (menus over the hint panel) and most, and its width with the
    /// menus folded into one key.
    pub left_min: f32,
    pub left_max: f32,
    pub left_compact: f32,
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

/// Where everything went. Fixed arrays: laying the bar out allocates nothing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    pub left: Span,
    /// The left block has its menus (else they are folded into one key).
    pub left_full: bool,
    /// The groups on the bar, left to right; the first `placed_len` are used.
    pub placed: [(Group, Span); COUNT],
    pub placed_len: usize,
    /// The groups in the ⋯ menu, in the bar's order; the first `folded_len` are used.
    pub folded: [Group; COUNT],
    pub folded_len: usize,
    pub more: Span,
}

impl Layout {
    pub fn placed(&self) -> &[(Group, Span)] {
        &self.placed[..self.placed_len]
    }

    pub fn folded(&self) -> &[Group] {
        &self.folded[..self.folded_len]
    }

    pub fn span_of(&self, g: Group) -> Option<Span> {
        self.placed().iter().find(|(x, _)| *x == g).map(|(_, s)| *s)
    }
}

/// Lay the bar out: `wants` are the shown groups in the owner's order.
///
/// Groups less important than the menus ([`MENUS_IMPORTANCE`]) fold first; then the menus
/// fold into one key (the compact left block, `room.left_compact`); then the rest.
pub fn layout(room: &Room, wants: &[Want]) -> Layout {
    let n = wants.len().min(COUNT);
    let wants = &wants[..n];
    let mut on = [true; COUNT];
    let mut left_min = room.left_min;
    let need = |on: &[bool; COUNT], left: f32| {
        room.pad * 2.0
            + left
            + room.gap
            + room.more
            + wants
                .iter()
                .zip(on.iter())
                .filter(|(_, o)| **o)
                .map(|(w, _)| w.min + room.gap)
                .sum::<f32>()
    };
    while need(&on, left_min) > room.width {
        // The least important group still on the bar leaves it — or, once only groups
        // that matter more than the menus are left, the menus fold into their key.
        let least = (0..n)
            .filter(|&i| on[i])
            .max_by_key(|&i| wants[i].group.importance());
        let full = left_min > room.left_compact;
        match least {
            Some(i) if !full || wants[i].group.importance() > MENUS_IMPORTANCE => on[i] = false,
            _ if full => left_min = room.left_compact,
            _ => break,
        }
    }
    let mut slack = (room.width - need(&on, left_min)).max(0.0);
    let left_w = left_min + slack.min((room.left_max - left_min).max(0.0));
    slack -= left_w - left_min;
    let mut out = Layout {
        left: Span {
            x0: room.pad,
            x1: room.pad + left_w,
        },
        left_full: left_w >= room.left_min,
        placed: [(Group::Transport, Span::default()); COUNT],
        placed_len: 0,
        folded: [Group::Transport; COUNT],
        folded_len: 0,
        more: Span {
            x0: room.width - room.pad - room.more,
            x1: room.width - room.pad,
        },
    };
    let mut x = out.left.x1 + room.gap;
    for (i, w) in wants.iter().enumerate() {
        if !on[i] {
            out.folded[out.folded_len] = w.group;
            out.folded_len += 1;
            continue;
        }
        let grow = slack.min((w.max - w.min).max(0.0));
        slack -= grow;
        let width = w.min + grow;
        out.placed[out.placed_len] = (
            w.group,
            Span {
                x0: x,
                x1: x + width,
            },
        );
        out.placed_len += 1;
        x += width + room.gap;
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
            left_compact: 200.0,
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
            })
            .collect()
    }

    /// When only the groups that matter more than the menus are left, the menus fold into
    /// one key (the compact left block) before those groups leave the bar.
    #[test]
    fn the_menus_fold_before_the_transport_does() {
        let mut r = room(500.0);
        r.left_compact = 60.0;
        let l = layout(&r, &wants());
        assert!(!l.left_full, "{l:?}");
        assert!(l.left.width() >= 60.0 && l.left.width() < 200.0);
        assert!(l.span_of(Group::Transport).is_some(), "{l:?}");
        // With room for the full left block, it is full.
        let l = layout(&room(1400.0), &wants());
        assert!(l.left_full);
    }
    #[test]
    fn a_narrow_bar_folds_the_least_important_first() {
        let l = layout(&room(700.0), &wants());
        let on: Vec<Group> = l.placed().iter().map(|(g, _)| *g).collect();
        // 700 − 16 − 200 − 10 − 24 = 450 → six groups of 70.
        assert_eq!(on.len(), 6, "{on:?}");
        for g in l.folded() {
            for (p, _) in l.placed() {
                assert!(g.importance() > p.importance(), "{g:?} folded before {p:?}");
            }
        }
        assert_eq!(on.len() + l.folded().len(), COUNT);
    }

    #[test]
    fn room_to_spare_goes_to_the_left_block_then_the_meter() {
        let l = layout(&room(4000.0), &wants());
        assert_eq!(l.folded().len(), 0);
        assert_eq!(l.left.width(), 200.0 + 200.0);
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

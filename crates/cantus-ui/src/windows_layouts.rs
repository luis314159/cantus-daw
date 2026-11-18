//! The built-in layouts for several screens and wide screens (ADR 0151 §7), the View menu's
//! actions on windows, and the layouts' keys (Alt+1 … Alt+9).
//!
//! - *Wide screen: arrange beside mix* — one window, the mixer right of the arrangement, on
//!   a main screen of at least 21 : 9.
//! - *Two screens: arrange | mix* — the mixer with Routing on the other screen.
//! - *Two screens: arrange | edit* — the piano roll (the score beside it) over the channel
//!   rack on the other screen, taller on a portrait screen.
//! - *Three screens: edit | arrange | mix* — the arrangement in the middle.
//!
//! "The other screen" is the largest the main window is not on; with three, left and right
//! follow the screens' places on the desktop.

use crate::app::CantusApp;
use crate::layout::{dock_to_value, Layout, DEFAULT_NAME};
use crate::screens::{self, ScreenKey};
use crate::views::workspace::{Pane, INSPECTOR_W};
use crate::windows::{Dock, Target, WindowShape};
use egui_dock::{DockState, NodeIndex};

/// A built-in layout.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Builtin {
    Wide,
    TwoMix,
    TwoEdit,
    Three,
}

impl Builtin {
    pub const ALL: [Builtin; 4] = [Builtin::Wide, Builtin::TwoMix, Builtin::TwoEdit, Builtin::Three];

    /// The name a layouts file keeps (never shown: see [`Builtin::label_key`]).
    pub fn name(self) -> &'static str {
        match self {
            Builtin::Wide => "@wide-arrange-mix",
            Builtin::TwoMix => "@two-arrange-mix",
            Builtin::TwoEdit => "@two-arrange-edit",
            Builtin::Three => "@three-edit-arrange-mix",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Builtin::Wide => "screens.layout.wide",
            Builtin::TwoMix => "screens.layout.two_mix",
            Builtin::TwoEdit => "screens.layout.two_edit",
            Builtin::Three => "screens.layout.three",
        }
    }

    pub fn of_name(name: &str) -> Option<Builtin> {
        Builtin::ALL.into_iter().find(|b| b.name() == name)
    }

    /// How many screens it needs.
    pub fn needs(self) -> usize {
        match self {
            Builtin::Wide => 1,
            Builtin::TwoMix | Builtin::TwoEdit => 2,
            Builtin::Three => 3,
        }
    }

    /// Whether it can be laid on these screens, the main window on `main`.
    pub fn fits(self, now: &[ScreenKey], main: Option<usize>) -> bool {
        match self {
            Builtin::Wide => main.and_then(|m| now.get(m)).is_some_and(ScreenKey::is_wide),
            b => now.len() >= b.needs(),
        }
    }

    /// The layout on these screens.
    pub fn layout(self, now: &[ScreenKey], main: Option<usize>) -> Layout {
        let mut lay = Layout::default_layout();
        lay.name = self.name().to_string();
        lay.inspector = INSPECTOR_W;
        lay.arrangement = 0.0;
        let window = |dock: DockState<Pane>, screen: Option<usize>, key: u32| WindowShape {
            key,
            name: String::new(),
            dock: dock_to_value(&dock),
            place: screen.and_then(|s| screens::fresh_place(now, s)),
            bar: true,
            on_top: false,
            scale: None,
        };
        let other = screens::other_screen(now, main);
        match self {
            Builtin::Wide => {
                lay.dock = dock_to_value(&wide_dock());
            }
            Builtin::TwoMix => {
                lay.dock = dock_to_value(&main_dock(Some(&[Pane::PianoRoll, Pane::ChannelRack])));
                lay.windows = vec![window(mix_dock(), other, 1)];
            }
            Builtin::TwoEdit => {
                lay.dock = dock_to_value(&main_dock(Some(&[Pane::Mixer, Pane::Routing])));
                let portrait = other.and_then(|o| now.get(o)).is_some_and(ScreenKey::is_portrait);
                lay.windows = vec![window(edit_dock(portrait), other, 1)];
            }
            Builtin::Three => {
                lay.dock = dock_to_value(&main_dock(None));
                let (left, right) = main.map_or((None, None), |m| screens::sides(now, m));
                let (left, right) = match (left, right) {
                    (Some(l), Some(r)) => (Some(l), Some(r)),
                    // Both on one side: the nearer one edits, the farther one mixes.
                    _ => {
                        let mut rest: Vec<usize> =
                            (0..now.len()).filter(|i| Some(*i) != main).collect();
                        rest.sort_by_key(|i| now[*i].pos[0]);
                        (rest.first().copied(), rest.get(1).copied())
                    }
                };
                let portrait = left.and_then(|o| now.get(o)).is_some_and(ScreenKey::is_portrait);
                lay.windows = vec![window(edit_dock(portrait), left, 1), window(mix_dock(), right, 2)];
            }
        }
        lay
    }
}

/// The main window's dock for the layouts with windows: the arrangement, the inspector at
/// its right, and `lower` under the arrangement.
fn main_dock(lower: Option<&[Pane]>) -> DockState<Pane> {
    let mut d = DockState::new(vec![Pane::Arrangement]);
    let s = d.main_surface_mut();
    let [centre, _] = s.split_right(NodeIndex::root(), 0.80, vec![Pane::Inspector]);
    if let Some(lower) = lower {
        s.split_below(centre, 0.55, lower.to_vec());
    }
    d
}

/// The mixer's window: the mixer, Routing behind it.
fn mix_dock() -> DockState<Pane> {
    DockState::new(vec![Pane::Mixer, Pane::Routing])
}

/// The editors' window: the piano roll (the score beside it) over the channel rack.
fn edit_dock(portrait: bool) -> DockState<Pane> {
    let mut d = DockState::new(vec![Pane::PianoRoll, Pane::Score]);
    let s = d.main_surface_mut();
    s.split_below(NodeIndex::root(), if portrait { 0.7 } else { 0.62 }, vec![Pane::ChannelRack]);
    d
}

/// One wide window: the arrangement over the piano roll and rack, the mixer at its right,
/// the inspector at the far right.
fn wide_dock() -> DockState<Pane> {
    let mut d = DockState::new(vec![Pane::Arrangement]);
    let s = d.main_surface_mut();
    let [centre, _] = s.split_right(NodeIndex::root(), 0.86, vec![Pane::Inspector]);
    let [arrange, _] = s.split_right(centre, 0.56, vec![Pane::Mixer, Pane::Routing]);
    s.split_below(arrange, 0.55, vec![Pane::PianoRoll, Pane::ChannelRack]);
    d
}

/// One row of the View menu's layouts, in the order Alt+1 … Alt+9 recall them.
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutRow {
    Default,
    Builtin(Builtin),
    Saved(String),
}

impl CantusApp {
    /// The layouts the View menu lists, in order: *Default*, the built-ins, the user's.
    pub fn layout_rows(&self) -> Vec<LayoutRow> {
        let mut rows = vec![LayoutRow::Default];
        rows.extend(Builtin::ALL.into_iter().map(LayoutRow::Builtin));
        rows.extend(
            self.layouts
                .saved
                .iter()
                .filter(|l| l.name != DEFAULT_NAME && Builtin::of_name(&l.name).is_none())
                .map(|l| LayoutRow::Saved(l.name.clone())),
        );
        rows
    }

    /// Whether a layout row can be applied on the screens there are.
    pub fn layout_row_fits(&self, row: &LayoutRow) -> bool {
        match row {
            LayoutRow::Builtin(b) => {
                let m = self.windows.main_screen(self.windows.main.outer.as_ref());
                b.fits(&self.windows.screens, m)
            }
            _ => true,
        }
    }

    /// Apply the View menu's layout row `row`.
    pub fn apply_layout_row(&mut self, row: &LayoutRow) {
        match row {
            LayoutRow::Default => self.reset_layout(),
            LayoutRow::Saved(name) => self.apply_layout(name),
            LayoutRow::Builtin(b) => {
                if !self.layout_row_fits(row) {
                    let n = b.needs();
                    self.set_status(crate::i18n::tf("screens.layout.needs", &[("n", &n)]));
                    return;
                }
                let m = self.windows.main_screen(self.windows.main.outer.as_ref());
                let lay = b.layout(&self.windows.screens, m);
                self.layouts.current = lay.clone();
                self.apply_layout_value(&lay);
                let label = crate::i18n::t(b.label_key());
                self.set_status(crate::i18n::tf("layout.applied", &[("name", &label)]));
            }
        }
    }

    /// Alt+`n` (1 … 9): the View menu's `n`th layout.
    pub fn apply_layout_number(&mut self, n: usize) {
        if let Some(row) = self.layout_rows().get(n.saturating_sub(1)).cloned() {
            self.apply_layout_row(&row);
        }
    }

    /// View ▸ *Gather every window on this screen*: each window on the main window's screen,
    /// restored and stepped down and right — the rescue for a screen the system still lists
    /// but nobody can see.
    pub fn gather_windows(&mut self) {
        let m = self.windows.main_screen(self.windows.main.outer.as_ref());
        let Some(m) = m else {
            return;
        };
        for (n, w) in self.windows.list.iter_mut().enumerate() {
            if let Some(mut p) = screens::fresh_place(&self.windows.screens, m) {
                p.maximised = false;
                p.rect.w = p.rect.w * 2 / 3;
                p.rect.h = p.rect.h * 2 / 3;
                let step = (32.0 * p.screen.scale) as i32 * n as i32;
                p.rect.x += step;
                p.rect.y += step;
                w.place = Some(p);
                w.shown = false;
            }
        }
    }

    /// View ▸ *Put every area back in the main window*.
    pub fn dock_every_window(&mut self) {
        let Some(ctx) = self.theme_ctx.clone() else {
            return;
        };
        for i in 0..self.windows.list.len() {
            for p in self.dock(Dock::Window(i)).panes() {
                self.move_pane(&ctx, p, Target::Main);
            }
        }
        self.windows.parked.clear();
    }

    /// *Move to a new window* for the area being worked in (the palette, its key).
    pub fn move_focused_to_new_window(&mut self) {
        let Some(ctx) = self.theme_ctx.clone() else {
            return;
        };
        let pane = self
            .workspace
            .focused_pane()
            .or_else(|| self.workspace.panes().first().copied());
        if let Some(p) = pane {
            self.move_pane(&ctx, p, Target::NewWindow { screen: None });
        }
    }
}

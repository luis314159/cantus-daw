//! The command palette's index (round 31, ADR 0072): every thing the palette can do — each
//! [`Action`] with its shortcut, each area and window, each Settings page, every effect and
//! instrument of the device catalogue, the recent projects and the templates — as entries
//! with their names in the interface's language **and** in English, found by a fuzzy match
//! and ranked by how often and how lately each was used. Pure data and pure functions: the
//! panel (`views::palette`) draws what [`rank`] returns, and the tests need no window.
//!
//! **The match.** The query is folded (lower case, no accents) and split into words; every
//! word must be found in the name as a subsequence. A word scores per letter, more at the
//! start of a word of the name and for letters that follow each other, most for a name that
//! starts with it (VS Code's and Bitwig's *Commander* order: "mx" finds *Mixer*, "spl"
//! *Split clip*, "nuevo" *Nuevo proyecto* and "new" the same entry in Spanish). The English
//! name is matched too, a little below the interface's.
//!
//! **The order.** With nothing typed: what was used last first, then the rest in the
//! index's order (actions by category). With words: the score, plus a bonus that grows with
//! the uses (a logarithm, so a favourite never buries a better match) and with recency.

use crate::keymap::Action;
use crate::picker_catalog::fold;
use crate::views::workspace::Pane;
use crate::views::workspace_settings::SettingsPage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A window the palette opens (not an area of the workspace, not an action).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Window {
    /// The keyboard shortcuts.
    Shortcuts,
    /// The appearance editor (themes, colours).
    Appearance,
    /// Which groups the top bar shows.
    TopBar,
    /// The start page: templates and recent projects.
    Start,
    /// About Cantus.
    About,
}

impl Window {
    pub const ALL: [Window; 5] = [
        Window::Shortcuts,
        Window::Appearance,
        Window::TopBar,
        Window::Start,
        Window::About,
    ];

    /// The i18n key of its name in the palette.
    pub fn key(self) -> &'static str {
        match self {
            Window::Shortcuts => "workflow.window.shortcuts",
            Window::Appearance => "workflow.window.appearance",
            Window::TopBar => "workflow.window.topbar",
            Window::Start => "workflow.window.start",
            Window::About => "workflow.window.about",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Window::Shortcuts => "shortcuts",
            Window::Appearance => "appearance",
            Window::TopBar => "topbar",
            Window::Start => "start",
            Window::About => "about",
        }
    }
}

/// What choosing an entry does.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Action(Action),
    /// An area of the workspace that has no action of its own: shown (or brought forward).
    Pane(Pane),
    Window(Window),
    Settings(SettingsPage),
    /// A device of the catalogue, by its key: an effect inserted on the selected strip, an
    /// instrument on a new track after the selected one.
    Device(String),
    /// A recent project, opened (asking first about unsaved changes).
    Recent(PathBuf),
    /// A project template: a new project made from it.
    Template(crate::templates::TemplateRef),
    /// A track template: inserted after the selected track.
    TrackTemplate(PathBuf),
    /// A note effect added at the end of a track's chain (round 31, ADR 0069).
    NoteFx(cantus_core::TrackId, cantus_core::notefx::NoteFxType),
}

/// What an entry is, as the palette's right-hand word says.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Action,
    View,
    Window,
    Settings,
    Effect,
    Instrument,
    Recent,
    Template,
    TrackTemplate,
    NoteEffect,
}

impl Kind {
    /// The i18n key of its word.
    pub fn key(self) -> &'static str {
        match self {
            Kind::Action => "workflow.kind.action",
            Kind::View => "workflow.kind.view",
            Kind::Window => "workflow.kind.window",
            Kind::Settings => "workflow.kind.settings",
            Kind::Effect => "workflow.kind.effect",
            Kind::Instrument => "workflow.kind.instrument",
            Kind::Recent => "workflow.kind.recent",
            Kind::Template => "workflow.kind.template",
            Kind::TrackTemplate => "workflow.kind.track_template",
            Kind::NoteEffect => "notefx.palette.kind",
        }
    }
}

/// One thing the palette offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub target: Target,
    pub kind: Kind,
    /// In the interface's language.
    pub name: String,
    /// In English (the same text when the interface is English).
    pub english: String,
    /// What the row says at its right: the shortcut, the folder, the device's kind.
    pub detail: String,
    /// Stable across languages and renames: what its uses are counted by.
    pub key: String,
    folded: String,
    folded_en: String,
}

impl Entry {
    pub fn new(
        target: Target,
        kind: Kind,
        name: String,
        english: String,
        detail: String,
        key: String,
    ) -> Entry {
        let folded = fold(&name);
        let folded_en = fold(&english);
        Entry {
            target,
            kind,
            name,
            english,
            detail,
            key,
            folded,
            folded_en,
        }
    }

    /// The stable key an action's entry is counted by.
    pub fn action_key(a: Action) -> String {
        format!("action:{}", a.id())
    }

    pub fn window_key(w: Window) -> String {
        format!("window:{}", w.id())
    }

    pub fn settings_key(p: SettingsPage) -> String {
        format!("settings:{}", p.key())
    }
}

/// How well one folded `word` is found in a folded `hay`, as a subsequence: `None` when a
/// letter is missing. Letters at the start of a word of `hay` and letters that follow the
/// one before score more; a `hay` that starts with the word scores most.
pub fn word_score(hay: &str, word: &str) -> Option<i32> {
    if word.is_empty() {
        return Some(0);
    }
    let h: Vec<char> = hay.chars().collect();
    let w: Vec<char> = word.chars().collect();
    let starts = |i: usize| i == 0 || !h[i - 1].is_alphanumeric();
    // Prefer an occurrence that begins at the start of a word: try each start of the first
    // letter, keep the best.
    let mut best: Option<i32> = None;
    for first in (0..h.len()).filter(|&i| h[i] == w[0]) {
        let mut score = 10 + if starts(first) { 16 } else { 0 };
        let mut prev = first;
        let mut ok = true;
        for &c in &w[1..] {
            // The next letter: at once, else at the next start of a word, else anywhere.
            let next = if prev + 1 < h.len() && h[prev + 1] == c {
                Some(prev + 1)
            } else {
                let rest = prev + 1..h.len();
                rest.clone()
                    .find(|&i| h[i] == c && starts(i))
                    .or_else(|| rest.clone().find(|&i| h[i] == c))
            };
            let Some(i) = next else {
                ok = false;
                break;
            };
            score += 10;
            if i == prev + 1 {
                score += 8;
            } else if starts(i) {
                score += 12;
            } else {
                score -= ((i - prev - 1) as i32).min(6);
            }
            prev = i;
        }
        if !ok {
            continue;
        }
        if first == 0 {
            score += 30;
        }
        best = Some(best.map_or(score, |b| b.max(score)));
    }
    best
}

/// How well a folded query (words) is found in an entry: every word must be found in its
/// name, or every word in its English name (a little lower).
pub fn score(entry: &Entry, query: &str) -> Option<i32> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let all = |hay: &str| -> Option<i32> {
        words
            .iter()
            .map(|w| word_score(hay, w))
            .try_fold(0, |acc, s| s.map(|s| acc + s))
    };
    let own = all(&entry.folded);
    let en = if entry.folded_en != entry.folded {
        all(&entry.folded_en).map(|s| s - 4)
    } else {
        None
    };
    match (own, en) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// One entry's use: how many times, and when last (the uses' own clock).
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Use {
    pub count: u32,
    pub last: u64,
}

/// How the palette has been used, kept in `palette.toml` beside the keymap.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Uses {
    #[serde(default)]
    clock: u64,
    #[serde(default)]
    uses: BTreeMap<String, Use>,
}

/// How many entries the uses remember (the least recent go first).
const USES_KEPT: usize = 400;

impl Uses {
    /// `key` was chosen.
    pub fn used(&mut self, key: &str) {
        self.clock += 1;
        let u = self.uses.entry(key.to_string()).or_default();
        u.count = u.count.saturating_add(1);
        u.last = self.clock;
        if self.uses.len() > USES_KEPT {
            if let Some(oldest) = self
                .uses
                .iter()
                .min_by_key(|(_, u)| u.last)
                .map(|(k, _)| k.clone())
            {
                self.uses.remove(&oldest);
            }
        }
    }

    pub fn of(&self, key: &str) -> Option<Use> {
        self.uses.get(key).copied()
    }

    /// The bonus a match gets for its uses: grows with their number as a logarithm, and
    /// with how lately (the last 20 choices).
    fn bonus(&self, key: &str) -> i32 {
        let Some(u) = self.of(key) else {
            return 0;
        };
        let count = (12.0 * (1.0 + u.count as f32).ln()).min(36.0) as i32;
        let age = self.clock.saturating_sub(u.last);
        let recent = if age < 20 {
            12 - (age as i32 * 12 / 20)
        } else {
            0
        };
        count + recent
    }

    pub fn load(path: &Path) -> Uses {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        // Through a temporary file: a crash never leaves half of it (review 8).
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)
            .and_then(|_| std::fs::rename(&tmp, path))
            .map_err(|e| e.to_string())
    }
}

/// Where the uses are kept.
pub fn uses_path(config: &Path) -> PathBuf {
    config.join("palette.toml")
}

/// The entries to show for `query` (typed, not yet folded), best first, as indices into
/// `entries`. Nothing typed: the used ones by recency, then the rest in their order.
pub fn rank(entries: &[Entry], query: &str, uses: &Uses) -> Vec<usize> {
    let q = fold(query.trim());
    if q.is_empty() {
        let mut used: Vec<(u64, usize)> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| uses.of(&e.key).map(|u| (u.last, i)))
            .collect();
        used.sort_by_key(|u| std::cmp::Reverse(u.0));
        let mut out: Vec<usize> = used.iter().map(|(_, i)| *i).collect();
        let mut taken = vec![false; entries.len()];
        for &i in &out {
            taken[i] = true;
        }
        out.extend((0..entries.len()).filter(|&i| !taken[i]));
        return out;
    }
    let mut hits: Vec<(i32, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| score(e, &q).map(|s| (s + uses.bonus(&e.key), i)))
        .collect();
    // Best first; equal scores keep the index's order (stable).
    hits.sort_by_key(|h| std::cmp::Reverse(h.0));
    hits.into_iter().map(|(_, i)| i).collect()
}

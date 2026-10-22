//! Linked clips in the application (ADR 0037): *Make unique*, *Link to…*, *Select
//! linked*, the setting that decides whether a copy is linked, and what the arrangement
//! needs to mark a shared clip.
//!
//! The rules live in `cantus_core::links` (the content shared by a group, the commands
//! that keep it shared); this module adds the selection, the status line and the setting.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};
use cantus_core::{links, Clip, LinkId, Project};
use std::collections::HashMap;

impl CantusApp {
    /// Take every selected clip out of its group (*Make unique*): from now on its content
    /// is its own. One undo step; the selection stays.
    pub fn make_unique_clips(&mut self) {
        let sel = self.clip_selection();
        let commands = links::make_unique(self.project(), &sel);
        if commands.is_empty() {
            self.set_status(t("links.status.not_linked"));
            return;
        }
        let n = commands.len();
        self.exec_clip_batch("cmd.make_unique", commands, sel);
        self.set_status(if n == 1 {
            t("links.status.made_unique_one")
        } else {
            tf("links.status.made_unique", &[("n", &n)])
        });
    }

    /// Link the selected clips to `target` (*Link to…*): each takes its content and joins
    /// its group. With no target, the clip being edited (the selection's own clip, the one
    /// the piano roll shows) is the one the others become.
    pub fn link_clips_to(&mut self, target: Option<ClipRef>) {
        let sel = self.clip_selection();
        let Some(target) = target.or_else(|| self.primary_clip(&sel)) else {
            self.set_status(t("links.status.select_two"));
            return;
        };
        if sel.iter().all(|r| *r == target) {
            self.set_status(t("links.status.select_two"));
            return;
        }
        let commands = self.with_ids(|project, ids| links::link_to(project, &sel, target, ids));
        if commands.is_empty() {
            self.set_status(t("links.status.nothing_to_link"));
            return;
        }
        let mut select = sel;
        if !select.contains(&target) {
            select.push(target);
        }
        self.exec_clip_batch("cmd.link_clips", commands, select);
        self.set_status(t("links.status.linked"));
    }

    /// Add every sibling of the selected clips to the selection.
    pub fn select_linked(&mut self) {
        let sel = self.clip_selection();
        let mut out = sel.clone();
        for r in &sel {
            for s in links::siblings(self.project(), *r) {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        if out.len() == sel.len() {
            self.set_status(t("links.status.not_linked"));
            return;
        }
        self.set_clip_selection(out);
        self.focus_arrangement();
    }

    /// Make `r` the edited clip (the one the piano roll shows and *Link to* without a
    /// target links to) while keeping the selection: what a right-click on a clip that is
    /// already selected does, so the menu's "this clip" and the keys mean the same clip.
    pub fn edit_clip_keeping_selection(&mut self, r: ClipRef) {
        let mut sel = self.clip_selection();
        if !sel.contains(&r) || self.selected_track.zip(self.selected_clip) == Some(r) {
            return;
        }
        sel.retain(|x| *x != r);
        sel.insert(0, r);
        self.set_clip_selection(sel);
        self.inspector
            .follow(crate::views::inspector::Subject::Clip(r.0, r.1));
    }

    /// Switch whether copies (paste, Duplicate, Alt-drag) are linked, and remember it.
    pub fn toggle_link_copies(&mut self) {
        self.settings.link_copies = !self.settings.link_copies;
        self.store_settings();
        self.set_status(t(if self.settings.link_copies {
            "links.status.copies_linked"
        } else {
            "links.status.copies_unique"
        }));
    }

    /// The clip a *Link to* without a target links to: the clip being edited when it is
    /// selected, else the first selected.
    fn primary_clip(&self, sel: &[ClipRef]) -> Option<ClipRef> {
        let current = self.selected_track.zip(self.selected_clip);
        current
            .filter(|c| sel.contains(c))
            .or_else(|| sel.first().copied())
    }
}

/// What the status line says when a project opened with clips taken out of link groups
/// they did not match (`cantus_core::links::repair`, a file edited by hand).
pub fn repaired_text(n: usize) -> String {
    if n == 1 {
        t("links.status.repaired_one")
    } else {
        tf("links.status.repaired", &[("n", &n)])
    }
}

/// Whether any clip of the selection is in a group of two or more: what lights the
/// toolbar's link key and offers *Make unique* in the menus.
pub fn selection_is_linked(project: &Project, sel: &[ClipRef]) -> bool {
    sel.iter().any(|r| !links::siblings(project, *r).is_empty())
}

/// What the arrangement needs, once a frame, to mark shared clips: the size of every
/// group, and the groups of the selected clips (whose other members are highlighted).
#[derive(Debug, Default)]
pub struct LinkMarks {
    sizes: HashMap<LinkId, usize>,
    selected: Vec<LinkId>,
}

impl LinkMarks {
    pub fn of(project: &Project, sel: &[ClipRef]) -> Self {
        // A project with no linked clip — every project before schema 22 — pays one scan.
        let any = project
            .tracks
            .iter()
            .any(|t| t.clips.iter().any(|c| c.link.is_some()));
        if !any {
            return LinkMarks::default();
        }
        let selected = sel
            .iter()
            .filter_map(|(t, c)| project.track(*t)?.clip(*c)?.link)
            .collect();
        LinkMarks {
            sizes: links::sizes(project),
            selected,
        }
    }

    /// How many clips `clip` is shared by, when it is shared at all (two or more).
    pub fn count(&self, clip: &Clip) -> Option<usize> {
        let n = *self.sizes.get(&clip.link?)?;
        (n > 1).then_some(n)
    }

    /// Whether `clip` is a sibling of a selected clip (and not selected itself).
    pub fn is_sibling(&self, clip: &Clip, selected: bool) -> bool {
        !selected
            && self.count(clip).is_some()
            && clip.link.is_some_and(|l| self.selected.contains(&l))
    }
}

// ---- linking by name (round 26) ----------------------------------------------------------

impl CantusApp {
    /// Open the *Link to…* picker for the selected clips: every link group and every
    /// clip of its own by name, to choose the clip the selection becomes. `anchor` is
    /// where it hangs from (an inspector row); without one it opens at the pointer.
    pub fn open_link_picker(&mut self, anchor: Option<egui::Pos2>) {
        let sel = self.clip_selection();
        if sel.is_empty() {
            self.set_status(t("links.status.select_first"));
            return;
        }
        self.link_picker.open(sel, anchor);
    }
}

/// One row of the *Link to…* picker: an entry of `cantus_core::links::groups` with what
/// it shows, made once when the list is built, and whether choosing it would link.
#[derive(Clone, Debug, PartialEq)]
pub struct PickRow {
    pub group: links::Group,
    /// The name as shown: the clip's, or "Untitled clip" for one with none.
    pub name: String,
    /// "×3" for a group; empty for a clip of its own.
    pub count: String,
    /// "Drums · 5.1": the first member's track, and the bar and beat it starts on — what
    /// tells two entries of one name apart.
    pub hint: String,
    /// Why choosing it would link nothing (the row is dimmed and says so instead of the
    /// hint); `None` when it links.
    pub refusal: Option<links::Refusal>,
    /// The name and the track's name as the filter compares them ([`fold`]).
    found_by: (String, String),
}

/// Every row for linking one selection in one project, built once: `groups` once, the
/// selection summarised once (`links::Selection`), each row answered from the summary.
/// Typing in the filter only narrows it ([`PickList::matching`]).
#[derive(Clone, Debug, Default)]
pub struct PickList {
    rows: Vec<PickRow>,
}

impl PickList {
    pub fn build(project: &Project, sel: &[ClipRef]) -> PickList {
        let summary = links::Selection::of(project, sel);
        let untitled = t("links.picker.unnamed");
        let rows = links::groups(project)
            .into_iter()
            .filter_map(|g| {
                let track = project.tracks.get(g.track_index)?;
                let (bar, beat, _) = crate::views::bars::bbt(project, g.start);
                let name = if g.name.is_empty() {
                    untitled.clone()
                } else {
                    g.name.clone()
                };
                let count = if g.count > 1 {
                    tf("links.count", &[("n", &g.count)])
                } else {
                    String::new()
                };
                Some(PickRow {
                    hint: format!("{} \u{b7} {bar}.{beat}", track.name),
                    refusal: summary.would_link(&g).err(),
                    found_by: (fold(&g.name), fold(&track.name)),
                    name,
                    count,
                    group: g,
                })
            })
            .collect();
        PickList { rows }
    }

    pub fn rows(&self) -> &[PickRow] {
        &self.rows
    }

    /// The rows whose name — or whose first member's track name — holds `filter`
    /// (ignoring case, accents and the spaces around it), only the link groups when
    /// `groups_only`; their indices, in the list's order.
    pub fn matching(&self, filter: &str, groups_only: bool) -> Vec<usize> {
        let want = fold(filter.trim());
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !groups_only || r.group.count > 1)
            .filter(|(_, r)| {
                want.is_empty() || r.found_by.0.contains(&want) || r.found_by.1.contains(&want)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// The picker's rows for linking `sel`, narrowed by `filter` (see [`PickList`]).
pub fn picker_rows(project: &Project, sel: &[ClipRef], filter: &str) -> Vec<PickRow> {
    let list = PickList::build(project, sel);
    list.matching(filter, false)
        .into_iter()
        .map(|i| list.rows[i].clone())
        .collect()
}

/// Text as the filter compares it: lower case, without accents — precomposed ("í") or
/// decomposed ("i" and a combining mark, as macOS stores file names) alike.
pub fn fold(s: &str) -> String {
    crate::picker_catalog::fold(s)
        .chars()
        // The combining diacritical marks, U+0300 to U+036F, by number: they are what is
        // dropped here, never drawn (so the font-coverage scan has no character to check).
        .filter(|c| !(0x300..=0x36F).contains(&u32::from(*c)))
        .collect()
}

/// The word for a clip's kind.
pub fn kind_key(k: links::Kind) -> &'static str {
    match k {
        links::Kind::Notes => "links.kind.notes",
        links::Kind::Audio => "links.kind.audio",
        links::Kind::Automation => "links.kind.automation",
    }
}

/// What a refused row says.
pub fn refusal_key(r: links::Refusal) -> &'static str {
    match r {
        links::Refusal::Kind => "links.picker.why_kind",
        links::Refusal::Target => "links.picker.why_target",
        links::Refusal::Already | links::Refusal::Gone => "links.picker.why_already",
    }
}

// ---- the clip browser's drop (round 26) --------------------------------------------------

impl CantusApp {
    /// Place a linked copy of `source` on `track` at `start` — what a tile dragged out of
    /// the clip browser does where it is dropped: one undo step (`cmd.place_linked_copy`),
    /// through `Linker::copy` (a clip of its own goes into a new group with its copy). An
    /// automation clip stays under the track it automates, as its Alt-drag copy does.
    /// The copy is selected; returns it.
    pub fn place_linked_copy(
        &mut self,
        source: ClipRef,
        track: cantus_core::TrackId,
        start: cantus_core::Ticks,
    ) -> Option<ClipRef> {
        let clip = crate::clips::clip_of(self.project(), source)?.clone();
        self.project().track(track)?;
        let to = if clip.automation.is_some() {
            source.0
        } else {
            track
        };
        let start = start.max(cantus_core::Ticks::ZERO);
        let (mut commands, copy) =
            self.with_ids(|_, ids| links::Linker::new().copy(source.0, &clip, start, ids));
        let placed = (to, copy.id);
        commands.push(cantus_core::Command::InsertClip {
            track: to,
            clip: Box::new(copy),
        });
        let rev = self.bus.revision();
        self.exec_clip_batch("cmd.place_linked_copy", commands, vec![placed]);
        if self.bus.revision() == rev {
            return None;
        }
        let name = if clip.name.is_empty() {
            t("links.picker.unnamed")
        } else {
            clip.name
        };
        self.set_status(tf("links.status.placed", &[("name", &name)]));
        Some(placed)
    }
}

// ---- painting with the clip browser's chosen tile (round 26) ------------------------------

impl CantusApp {
    /// The arrangement's paint tool on or off (B with the arrangement focused, or its
    /// toolbar key). Its source is the clip browser's chosen tile; with none chosen, the
    /// clip being edited becomes the source. With neither, the clip browser is shown and
    /// the status line says what to do.
    pub fn toggle_arrangement_paint(&mut self) {
        if self.clip_browser.painting() {
            self.clip_browser.set_painting(false);
            self.set_status(t("links.paint.off"));
            return;
        }
        let edited = self.selected_track.zip(self.selected_clip);
        let source = self
            .clip_browser
            .brush()
            .filter(|b| crate::clips::clip_of(self.project(), *b).is_some())
            .or(edited.filter(|e| crate::clips::clip_of(self.project(), *e).is_some()));
        let Some(source) = source else {
            self.show_pane(crate::views::workspace::Pane::Clips);
            self.set_status(t("links.paint.choose"));
            return;
        };
        self.clip_browser.set_brush(Some(source));
        self.clip_browser.set_painting(true);
        self.set_status(t("links.paint.on"));
    }

    /// Paint linked copies of `source` on `track` at `starts` — one stroke of the paint
    /// tool, one undo step (`cmd.paint_linked_copies`). A start whose copy would overlap a
    /// copy of the same clip on that track (or another of the stroke) is skipped. Returns how many were placed; they
    /// become the selection.
    pub fn paint_linked_copies(
        &mut self,
        source: ClipRef,
        track: cantus_core::TrackId,
        starts: &[cantus_core::Ticks],
    ) -> usize {
        let Some(clip) = crate::clips::clip_of(self.project(), source).cloned() else {
            return 0;
        };
        let Some((to, wanted)) = paintable(self.project(), source, track, starts) else {
            return 0;
        };
        if wanted.is_empty() {
            return 0;
        }
        let (commands, placed) = self.with_ids(|_, ids| {
            let mut linker = links::Linker::new();
            let mut commands = Vec::new();
            let mut placed = Vec::new();
            for &start in &wanted {
                let (pre, copy) = linker.copy(source.0, &clip, start, ids);
                commands.extend(pre);
                placed.push((to, copy.id));
                commands.push(cantus_core::Command::InsertClip {
                    track: to,
                    clip: Box::new(copy),
                });
            }
            (commands, placed)
        });
        let n = placed.len();
        self.exec_clip_batch("cmd.paint_linked_copies", commands, placed);
        n
    }

    /// Erase the copies of `source` (its group, or the clip alone) on `track` that a stroke
    /// over `from..to` touched — one undo step (`cmd.erase_linked_copies`); never another
    /// clip. When the source itself goes, another copy of its group takes its place as the
    /// paint source; with none left, the tool is put away. Returns how many were erased.
    pub fn erase_linked_copies(
        &mut self,
        source: ClipRef,
        track: cantus_core::TrackId,
        from: cantus_core::Ticks,
        to: cantus_core::Ticks,
    ) -> usize {
        let Some(clip) = crate::clips::clip_of(self.project(), source) else {
            return 0;
        };
        let link = clip.link;
        let id = clip.id;
        let (lo, hi) = (from.min(to), from.max(to));
        let Some(on) = self.project().track(track) else {
            return 0;
        };
        let commands: Vec<cantus_core::Command> = on
            .clips
            .iter()
            .filter(|c| c.id == id || (link.is_some() && c.link == link))
            .filter(|c| c.start <= hi && c.end() > lo)
            .map(|c| cantus_core::Command::RemoveClip { track, clip: c.id })
            .collect();
        let n = commands.len();
        self.exec_clip_batch("cmd.erase_linked_copies", commands, Vec::new());
        if crate::clips::clip_of(self.project(), source).is_none() {
            let next = link.and_then(|l| links::members(self.project(), l).first().copied());
            self.clip_browser.set_brush(next);
            if next.is_none() {
                self.clip_browser.set_painting(false);
                self.set_status(t("links.paint.gone"));
            }
        }
        n
    }
}

impl CantusApp {
    /// Put the arrangement's paint tool away and let its source go: Esc, when the
    /// arrangement has the edit focus (the first claim on Esc while the tool is out).
    pub fn put_paint_away(&mut self) {
        self.clip_browser.set_painting(false);
        self.clip_browser.set_brush(None);
        self.set_status(t("links.paint.off"));
    }
}

/// Where a paint stroke of `source` over `starts` (aimed at `track`) would place copies:
/// the track they land on — the automated track for an automation clip — and the starts,
/// in order, whose span overlaps no copy of the same clip already on that track nor
/// another copy of the stroke (painting never stacks a copy on a copy: the notes would
/// sound twice). The paint tool's ghosts show exactly these.
pub fn paintable(
    project: &Project,
    source: ClipRef,
    track: cantus_core::TrackId,
    starts: &[cantus_core::Ticks],
) -> Option<(cantus_core::TrackId, Vec<cantus_core::Ticks>)> {
    let clip = crate::clips::clip_of(project, source)?;
    let to = if clip.automation.is_some() {
        source.0
    } else {
        track
    };
    let mut taken: Vec<(cantus_core::Ticks, cantus_core::Ticks)> = project
        .track(to)?
        .clips
        .iter()
        .filter(|c| c.id == clip.id || (clip.link.is_some() && c.link == clip.link))
        .map(|c| (c.start, c.end()))
        .collect();
    let mut wanted = Vec::new();
    for &s in starts {
        let span = (s, s + clip.length);
        let free = !s.is_negative() && taken.iter().all(|&(a, b)| span.1 <= a || span.0 >= b);
        if free {
            taken.push(span);
            wanted.push(s);
        }
    }
    wanted.sort();
    Some((to, wanted))
}

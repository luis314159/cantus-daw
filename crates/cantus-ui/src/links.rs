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

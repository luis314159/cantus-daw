//! What the desk shows, worked out once per project revision rather than once a frame:
//! the items of each dock in desk order, where each middle item sits, what each strip is
//! called and coloured, and — per selected strip — which strips it could send to without
//! closing a loop.
//!
//! This is what makes the 64-strip desk cheap (ADR 0024 §13): the old desk walked the
//! routing graph for every strip every frame to build a menu nobody had open (O(N³),
//! 2.8 ms and 8,064 allocations at 64 strips). Here the graph is walked once when the
//! project or the selection changes, into buffers that keep their capacity.

use crate::app::CantusApp;
use cantus_core::{Color, Dock, GroupId, StripId};

/// What the desk draws, left to right, in one dock: strips and the separators of the
/// groups they sit in. A collapsed group is its separator alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    Separator(GroupId),
    /// A strip, and its index in the project's strips.
    Strip(StripId, usize),
}

/// Where a strip's name comes from, so it is borrowed at paint time, never copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    /// The strip's own name.
    Own,
    /// The name of the track at this index (the first one feeding the strip).
    Track(usize),
    /// Nothing: the number says it all.
    None,
}

/// What a strip shows that is derived from the rest of the project.
#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub label: Label,
    /// Its own colour, else its first track's, else its group's.
    pub color: Option<Color>,
    /// It carries a working compressor: its gain-reduction lane shows.
    pub compressor: bool,
}

/// The width of a group separator, in points at 100 %.
pub const SEPARATOR: f32 = 20.0;
/// The `+` after the last strip of the middle.
pub const ADD: f32 = 24.0;

#[derive(Debug, Default)]
pub struct Desk {
    revision: Option<u64>,
    strips: usize,
    pub left: Vec<Item>,
    pub middle: Vec<Item>,
    pub right: Vec<Item>,
    /// Per project strip.
    pub info: Vec<Info>,
    /// The most inserts any strip has: the Extra large list shows one more.
    pub most_inserts: usize,
    pub any_solo: bool,
    pub any_muted: bool,
    /// Where the send arrows were worked out for: (revision, selected strip).
    routes_for: Option<(u64, Option<StripId>)>,
    /// Per project strip: routing the selected strip into it would close a loop.
    pub would_cycle: Vec<bool>,
    stack: Vec<usize>,
}

impl Desk {
    /// Bring the cache up to date with the project; a no-op while nothing changed.
    pub fn refresh(&mut self, app: &CantusApp) {
        let rev = app.bus.revision();
        let p = app.project();
        if self.revision != Some(rev) || self.strips != p.strips.len() {
            self.rebuild(app);
            self.revision = Some(rev);
            self.strips = p.strips.len();
        }
        let key = (rev, app.selected_strip);
        if self.routes_for != Some(key) {
            self.routes(app);
            self.routes_for = Some(key);
        }
    }

    fn rebuild(&mut self, app: &CantusApp) {
        let p = app.project();
        for (dock, items) in [
            (Dock::Left, &mut self.left),
            (Dock::Middle, &mut self.middle),
            (Dock::Right, &mut self.right),
        ] {
            items.clear();
            let mut run: Option<GroupId> = None;
            for (i, s) in p.strips.iter().enumerate().filter(|(_, s)| s.dock == dock) {
                let group = s.group.filter(|g| p.strip_group(*g).is_some());
                if group != run {
                    if let Some(g) = group {
                        items.push(Item::Separator(g));
                    }
                    run = group;
                }
                let collapsed = group
                    .and_then(|g| p.strip_group(g))
                    .is_some_and(|g| g.collapsed);
                if !collapsed {
                    items.push(Item::Strip(s.id, i));
                }
            }
        }
        self.info.clear();
        self.most_inserts = 0;
        self.any_solo = false;
        self.any_muted = false;
        for s in &p.strips {
            let first = p.tracks.iter().position(|t| t.strip == Some(s.id));
            let label = if !s.name.is_empty() {
                Label::Own
            } else if let Some(i) = first {
                Label::Track(i)
            } else {
                Label::None
            };
            let color = s
                .color
                .or_else(|| first.map(|i| p.tracks[i].color))
                .or_else(|| s.group.and_then(|g| p.strip_group(g)).map(|g| g.color));
            let compressor = s
                .effects
                .iter()
                .any(|e| e.spec.kind == cantus_core::EffectKind::Compressor && !e.bypass);
            self.info.push(Info {
                label,
                color,
                compressor,
            });
            self.most_inserts = self.most_inserts.max(s.effects.len());
            self.any_solo |= s.soloed;
            self.any_muted |= s.muted;
        }
    }

    /// Which strips the selected strip could not route into without a loop: every strip
    /// that reaches it (through outputs, sends and sidechain keys), and itself. One walk
    /// upstream from the selection, instead of one walk per strip per frame.
    fn routes(&mut self, app: &CantusApp) {
        let p = app.project();
        let n = p.strips.len();
        self.would_cycle.clear();
        self.would_cycle.resize(n, false);
        let Some(sel) = app.selected_strip.and_then(|s| p.strip_index(s)) else {
            return;
        };
        // An edge a → b means "a must be processed before b" (a feeds b, or b's
        // effects are keyed from a). Routing sel → x closes a loop iff x reaches sel.
        self.would_cycle[sel] = true;
        self.stack.clear();
        self.stack.push(sel);
        while let Some(to) = self.stack.pop() {
            let to_id = p.strips[to].id;
            for (from, s) in p.strips.iter().enumerate() {
                if self.would_cycle[from] {
                    continue;
                }
                let feeds = s.feeds().any(|t| t == to_id);
                let keys = p.strips[to].keys().any(|k| k == s.id);
                if feeds || keys {
                    self.would_cycle[from] = true;
                    self.stack.push(from);
                }
            }
        }
    }

    /// A strip's name as the desk shows it, borrowed from the project.
    pub fn name<'a>(&self, app: &'a CantusApp, index: usize) -> &'a str {
        let p = app.project();
        match self.info.get(index).map(|i| i.label) {
            Some(Label::Own) => p.strips.get(index).map_or("", |s| s.name.as_str()),
            Some(Label::Track(t)) => p.tracks.get(t).map_or("", |t| t.name.as_str()),
            _ => "",
        }
    }

    pub fn info(&self, index: usize) -> Option<&Info> {
        self.info.get(index)
    }
}

/// The width of an item at a strip pitch of `pitch` points.
pub fn item_width(item: &Item, pitch: f32) -> f32 {
    match item {
        Item::Separator(_) => SEPARATOR,
        Item::Strip(..) => pitch,
    }
}

/// The width a run of items takes.
pub fn items_width(items: &[Item], pitch: f32) -> f32 {
    items.iter().map(|i| item_width(i, pitch)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::Send;
    use cantus_engine::EngineHandle;

    #[test]
    fn loops_are_found_by_one_walk_upstream() {
        let mut a = CantusApp::from_parts(None, EngineHandle::dummy(), false);
        let ids: Vec<StripId> = a.project().strips.iter().map(|s| s.id).collect();
        // 1 → 2 (output), 2 sends to 3; select 3: routing 3 into 1, 2 or itself loops.
        a.exec(cantus_core::Command::SetStripOutput {
            id: ids[0],
            output: Some(ids[1]),
        });
        a.set_strip_send(ids[1], Send::new(ids[2]));
        a.end_gesture();
        a.selected_strip = Some(ids[2]);
        let mut d = Desk::default();
        d.refresh(&a);
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(
                d.would_cycle[i],
                a.project().routing_would_cycle(ids[2], Some(*id)) || i == 2,
                "strip {}",
                i + 1
            );
        }
        assert!(d.would_cycle[0] && d.would_cycle[1] && d.would_cycle[2]);
        assert!(!d.would_cycle[3]);
    }

    #[test]
    fn the_desk_is_rebuilt_only_when_the_project_changes() {
        let mut a = CantusApp::from_parts(None, EngineHandle::dummy(), false);
        let mut d = Desk::default();
        d.refresh(&a);
        let n = d.middle.len() + d.left.len() + d.right.len();
        assert_eq!(n, a.project().strips.len());
        let rev = d.revision;
        d.refresh(&a);
        assert_eq!(d.revision, rev);
        a.add_strip();
        d.refresh(&a);
        assert_eq!(
            d.middle.len() + d.left.len() + d.right.len(),
            a.project().strips.len()
        );
    }
}

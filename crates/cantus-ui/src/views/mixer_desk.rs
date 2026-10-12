//! What the desk shows, worked out once per project revision rather than once a frame:
//! the items of each dock in desk order, where each strip sits, what each strip is called
//! and coloured, the routes the cable band draws, each strip's chain in one line, and —
//! per selected strip — which strips it could send to without closing a loop.
//!
//! This is what makes the 64-strip desk cheap (ADR 0024 §13): the old desk walked the
//! routing graph for every strip every frame to build a menu nobody had open (O(N³),
//! 2.8 ms and 8,064 allocations at 64 strips). Here the graph is walked once when the
//! project or the selection changes, into buffers that keep their capacity; the cable
//! band reads `edges` and `place` and allocates nothing a frame (ADR 0041 §12).

use crate::app::CantusApp;
use cantus_core::routing::{self, Edge, EdgeKind};
use cantus_core::{Color, Dock, EffectSlot, GroupId, StripId};

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
    /// It goes somewhere other than the master (an output to a strip, or a send): its
    /// port is filled.
    pub goes: bool,
}

/// Where a strip's port is, in any width mode: its dock, and how many strips and group
/// separators come before it there. A strip folded into a collapsed group sits at its
/// separator (`folded`), where its cables gather.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub dock: Dock,
    pub strips: u16,
    pub seps: u16,
    pub folded: bool,
}

impl Place {
    /// The port's x from the dock's first item, for a strip pitch of `pitch` points.
    pub fn x(self, pitch: f32) -> f32 {
        let before = self.strips as f32 * pitch + self.seps as f32 * SEPARATOR;
        if self.folded {
            before + SEPARATOR * 0.5
        } else {
            before + (pitch - 1.0) * 0.5
        }
    }
}

/// An edge end that is the master, in [`Desk::ends`].
pub const MASTER_END: u32 = u32::MAX;

/// The width of a group separator, in points at 100 %.
pub const SEPARATOR: f32 = 20.0;
/// The `+` after the last strip of the middle.
pub const ADD: f32 = 24.0;

#[derive(Debug, Default)]
pub struct Desk {
    revision: Option<u64>,
    locale: Option<crate::i18n::Locale>,
    strips: usize,
    pub left: Vec<Item>,
    pub middle: Vec<Item>,
    pub right: Vec<Item>,
    /// Per project strip.
    pub info: Vec<Info>,
    /// Per project strip: where its port is.
    pub place: Vec<Place>,
    /// Every route once (`routing::edges`), in the project's order.
    pub edges: Vec<Edge>,
    /// Per edge: the project indices of its source and destination (`MASTER_END` for
    /// the master), so a frame never searches the strips.
    pub ends: Vec<(u32, u32)>,
    /// Per project strip: the index in `edges` of its output; its sends follow in order.
    pub first_edge: Vec<u32>,
    /// Per edge: a sounding send to where the output already reaches (answer 5).
    pub doubled: Vec<bool>,
    /// Per project strip, and the master's: the chain in one line ("EQ · Comp · +1"),
    /// empty without inserts.
    pub chain: Vec<String>,
    pub master_chain: String,
    /// The most sends any strip has: the Extra large send list's rows follow it.
    pub most_sends: usize,
    pub any_solo: bool,
    pub any_muted: bool,
    /// Per project strip: anything feeds it (`routing::is_fed`) — no Route here plate.
    pub fed: Vec<bool>,
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
        let locale = crate::i18n::locale();
        if self.revision != Some(rev)
            || self.strips != p.strips.len()
            || self.locale != Some(locale)
        {
            self.rebuild(app);
            self.revision = Some(rev);
            self.strips = p.strips.len();
            self.locale = Some(locale);
        }
        let key = (rev, app.selected_strip);
        if self.routes_for != Some(key) {
            self.routes(app);
            self.routes_for = Some(key);
        }
    }

    fn rebuild(&mut self, app: &CantusApp) {
        let p = app.project();
        self.place.clear();
        self.place.resize(
            p.strips.len(),
            Place {
                dock: Dock::Middle,
                strips: 0,
                seps: 0,
                folded: false,
            },
        );
        for (dock, items) in [
            (Dock::Left, &mut self.left),
            (Dock::Middle, &mut self.middle),
            (Dock::Right, &mut self.right),
        ] {
            items.clear();
            let mut run: Option<GroupId> = None;
            let (mut strips, mut seps) = (0u16, 0u16);
            // Where the current group's separator sits, for its folded members.
            let mut sep_at = (0u16, 0u16);
            for (i, s) in p.strips.iter().enumerate().filter(|(_, s)| s.dock == dock) {
                let group = s.group.filter(|g| p.strip_group(*g).is_some());
                if group != run {
                    if let Some(g) = group {
                        items.push(Item::Separator(g));
                        sep_at = (strips, seps);
                        seps += 1;
                    }
                    run = group;
                }
                let collapsed = group
                    .and_then(|g| p.strip_group(g))
                    .is_some_and(|g| g.collapsed);
                if collapsed {
                    self.place[i] = Place {
                        dock,
                        strips: sep_at.0,
                        seps: sep_at.1,
                        folded: true,
                    };
                } else {
                    items.push(Item::Strip(s.id, i));
                    self.place[i] = Place {
                        dock,
                        strips,
                        seps,
                        folded: false,
                    };
                    strips += 1;
                }
            }
        }
        self.info.clear();
        self.fed.clear();
        self.fed
            .extend(p.strips.iter().map(|s| routing::is_fed(p, s.id)));
        routing::edges_into(p, &mut self.edges);
        let index = |id: StripId| p.strip_index(id).map_or(MASTER_END, |i| i as u32);
        self.ends.clear();
        self.ends
            .extend(self.edges.iter().map(|e| (index(e.from), index(e.to))));
        self.first_edge.clear();
        self.first_edge.extend(
            self.edges
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == EdgeKind::Output)
                .map(|(i, _)| i as u32),
        );
        self.doubled.clear();
        self.doubled.extend(
            self.edges.iter().map(|e| {
                matches!(e.kind, EdgeKind::Send { .. }) && routing::doubled(p, e.from, e.to)
            }),
        );
        self.chain.clear();
        self.chain
            .extend(p.strips.iter().map(|s| chain_text(&s.effects)));
        self.master_chain = chain_text(&p.master.effects);
        self.most_sends = 0;
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
                goes: s.output.is_some() || !s.sends.is_empty(),
            });
            self.most_sends = self.most_sends.max(s.sends.len());
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

    /// Whether anything feeds the strip at `index` (fed when unknown: no plate).
    pub fn fed(&self, index: usize) -> bool {
        self.fed.get(index).copied().unwrap_or(true)
    }

    /// Whether send `k` of the strip at `index` doubles its signal (answer 5).
    pub fn send_doubled(&self, index: usize, k: usize) -> bool {
        self.first_edge
            .get(index)
            .and_then(|e| self.doubled.get(*e as usize + 1 + k))
            .copied()
            .unwrap_or(false)
    }

    /// The chain line of a strip (`None`: the master's).
    pub fn chain(&self, index: Option<usize>) -> &str {
        match index {
            Some(i) => self.chain.get(i).map_or("", String::as_str),
            None => &self.master_chain,
        }
    }
}

/// The short name an insert goes by in a chain line: "EQ", "Comp", a plug-in's own name.
pub fn short_name(e: &EffectSlot) -> String {
    use cantus_core::EffectKind as K;
    let key = match e.spec.kind {
        K::External => {
            return e
                .spec
                .external
                .as_ref()
                .map(|x| x.name.clone())
                .unwrap_or_default()
        }
        K::Eq => "mixer.chain.eq",
        K::Compressor => "mixer.chain.compressor",
        K::Limiter => "mixer.chain.limiter",
        K::Space => "mixer.chain.space",
        K::Multiband => "mixer.chain.multiband",
        K::DeEsser => "mixer.chain.deesser",
        K::Gate => "mixer.chain.gate",
        K::Saturator => "mixer.chain.saturator",
        K::Stereo => "mixer.chain.stereo",
        K::Maximizer => "mixer.chain.maximizer",
        K::Delay => "mixer.chain.delay",
        K::Reverb => "mixer.chain.reverb",
    };
    crate::i18n::t(key)
}

/// What separates the names in a chain line: a middle dot between thin spaces, so
/// "EQ · Comp · +1" fits a 76-pt strip.
pub const CHAIN_SEP: &str = "\u{2009}\u{b7}\u{2009}";
/// The chain line's text size, of the label size.
pub const CHAIN_TEXT: f32 = 0.88;

/// A chain in one line (the owner's answer 7): the first two inserts by their short
/// names, then how many more — "EQ · Comp · +1"; empty without inserts.
pub fn chain_text(effects: &[EffectSlot]) -> String {
    let mut out = String::new();
    for e in effects.iter().take(2) {
        if !out.is_empty() {
            out.push_str(CHAIN_SEP);
        }
        out.push_str(&short_name(e));
    }
    if effects.len() > 2 {
        out.push_str(CHAIN_SEP);
        out.push_str(&format!("+{}", effects.len() - 2));
    }
    out
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
    use cantus_core::{Command, EffectKind, Send};
    use cantus_engine::EngineHandle;

    #[test]
    fn loops_are_found_by_one_walk_upstream() {
        let mut a = CantusApp::from_parts(None, EngineHandle::dummy(), false);
        let ids: Vec<StripId> = a.project().strips.iter().map(|s| s.id).collect();
        // 1 → 2 (output), 2 sends to 3; select 3: routing 3 into 1, 2 or itself loops.
        a.exec(Command::SetStripOutput {
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

    /// The band's inputs, per revision: every route once with the doubled flag, each
    /// strip's place in its dock (a folded member at its separator), the port fill, the
    /// chain line and the send list's depth.
    #[test]
    fn edges_places_and_chains_follow_the_project() {
        crate::i18n::default_thread_locale(crate::i18n::Locale::En);
        let mut a = CantusApp::from_parts(None, EngineHandle::dummy(), false);
        let ids: Vec<StripId> = a.project().strips.iter().map(|s| s.id).collect();
        // 1 → 2 by its output, and a send 1 → 3 (2 does not reach 3: not doubled);
        // 2 sends to 3 and 1's output reaches 3 through 2... only once 2 sends there:
        // then 1's send to 3 is doubled.
        a.exec(Command::SetStripOutput {
            id: ids[0],
            output: Some(ids[1]),
        });
        a.set_strip_send(ids[0], Send::new(ids[2]));
        a.end_gesture();
        let mut d = Desk::default();
        d.refresh(&a);
        assert_eq!(d.edges, routing::edges(a.project()));
        let send = d
            .edges
            .iter()
            .position(|e| e.from == ids[0] && e.to == ids[2])
            .unwrap();
        assert!(!d.doubled[send]);
        a.set_strip_send(ids[1], Send::new(ids[2]));
        a.end_gesture();
        d.refresh(&a);
        let send = d
            .edges
            .iter()
            .position(|e| e.from == ids[0] && e.to == ids[2])
            .unwrap();
        assert!(d.doubled[send], "the return hears strip 1 twice");
        assert!(d.info[0].goes && d.info[1].goes && !d.info[3].goes);
        assert_eq!(d.most_sends, 1);
        // Places: strip 4 has three strips before it in the middle.
        let p4 = d.place[3];
        assert_eq!(
            (p4.dock, p4.strips, p4.seps, p4.folded),
            (Dock::Middle, 3, 0, false)
        );
        assert_eq!(p4.x(24.0), 3.0 * 24.0 + 11.5);
        // A chain line.
        a.add_effect(ids[0], EffectKind::Eq);
        a.add_effect(ids[0], EffectKind::Compressor);
        a.add_effect(ids[0], EffectKind::Limiter);
        d.refresh(&a);
        assert_eq!(
            d.chain(Some(0)),
            format!("EQ{CHAIN_SEP}Comp{CHAIN_SEP}+1"),
            "the owner's example"
        );
        assert_eq!(d.chain(Some(1)), "");
    }
}

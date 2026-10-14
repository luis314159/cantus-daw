//! Linked clips (ADR 0037): copies of a clip that are **the same clip** in another place —
//! FL Studio's pattern clips, Live's linked clips, Logic's aliases.
//!
//! A link group is a [`LinkId`] stored on every member ([`Clip::link`]). The members share
//! their [`ClipContent`] — the notes, the audio settings (gain, fades and their curves,
//! reverse, the slip, the file), the automation curve, the name and the colour — and each
//! keeps where it sits: its window (`start`, `length`, `offset`), its repeat
//! (`content_length`), its mute and its lane. The content is **replicated** on every
//! member rather than kept once in a pool, so everything that reads a clip — the snapshot,
//! the views, the exporters, the clipboard — reads it exactly as it always did, and an old
//! project is a project in which every clip is in no group.
//!
//! Keeping the copies equal is `Command::apply`'s job, not the caller's: every command that
//! changes a shared field (`InsertNotes`, `RemoveNotes`, `SetNotes`, `SetClipAudio`,
//! `SetClipAutomation`, `SetClipName`, `SetClipColor`, `SetClipContent`) changes it on the
//! clip it names and then on every sibling, and a command that would put a clip into a
//! group it does not match (`InsertClip`, `SetClipLink`) is refused. So undo stays exact —
//! an inverse names one clip and reaches the rest the same way — and no view can forget a
//! sibling. [`check`] states the invariant (the undo fuzz runs it after every step) and
//! [`repair`] restores it in a file edited by hand.
//!
//! The builders here return plain commands for one `Command::Batch`: [`Linker::copy`] (a
//! linked copy, for paste, duplicate and the Alt-drag), [`make_unique`] and [`link_to`].
//! For a list to pick a target from (round 26), [`groups`] names every group once and
//! every clip of its own, in a stable order, and [`would_link`] says beforehand what
//! [`link_to`] would do with a target — or why it would do nothing.

use crate::{
    AudioRegion, AutoTarget, Automation, Clip, ClipId, Color, Command, LinkId, Note, Project,
    Ticks, TrackId,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A clip, identified the way the selection identifies it.
pub type ClipRef = (TrackId, ClipId);

/// What the members of a link group share: everything a clip *is*, as opposed to where it
/// *is*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipContent {
    pub name: String,
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioRegion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation: Option<Automation>,
}

impl ClipContent {
    /// The shared part of `clip`.
    pub fn of(clip: &Clip) -> Self {
        ClipContent {
            name: clip.name.clone(),
            color: clip.color,
            notes: clip.notes.clone(),
            audio: clip.audio.clone(),
            automation: clip.automation.clone(),
        }
    }

    /// Put this content on `clip`, returning what it had.
    pub fn swap_into(self, clip: &mut Clip) -> ClipContent {
        ClipContent {
            name: std::mem::replace(&mut clip.name, self.name),
            color: std::mem::replace(&mut clip.color, self.color),
            notes: std::mem::replace(&mut clip.notes, self.notes),
            audio: std::mem::replace(&mut clip.audio, self.audio),
            automation: std::mem::replace(&mut clip.automation, self.automation),
        }
    }
}

/// Whether two clips share every shared field, to the bit (notes in the same order).
pub fn same_content(a: &Clip, b: &Clip) -> bool {
    a.name == b.name
        && a.color == b.color
        && a.notes == b.notes
        && a.audio == b.audio
        && a.automation == b.automation
}

/// Copy `src`'s shared fields onto `dst`.
pub fn adopt(dst: &mut Clip, src: &Clip) {
    dst.name.clone_from(&src.name);
    dst.color = src.color;
    dst.notes.clone_from(&src.notes);
    dst.audio.clone_from(&src.audio);
    dst.automation.clone_from(&src.automation);
}

/// Every member of `link`, in track order then clip order.
pub fn members(project: &Project, link: LinkId) -> Vec<ClipRef> {
    project
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(move |c| c.link == Some(link))
                .map(move |c| (t.id, c.id))
        })
        .collect()
}

/// How many clips are in `link`.
pub fn count(project: &Project, link: LinkId) -> usize {
    project
        .tracks
        .iter()
        .map(|t| t.clips.iter().filter(|c| c.link == Some(link)).count())
        .sum()
}

/// The other members of `r`'s group (none for a clip of its own, or the last of a group).
pub fn siblings(project: &Project, r: ClipRef) -> Vec<ClipRef> {
    let Some(link) = project
        .track(r.0)
        .and_then(|t| t.clip(r.1))
        .and_then(|c| c.link)
    else {
        return Vec::new();
    };
    members(project, link)
        .into_iter()
        .filter(|m| *m != r)
        .collect()
}

/// The size of every group, for a view that marks shared clips ("×3") once a frame.
pub fn sizes(project: &Project) -> HashMap<LinkId, usize> {
    let mut out = HashMap::new();
    for t in &project.tracks {
        for c in &t.clips {
            if let Some(l) = c.link {
                *out.entry(l).or_insert(0) += 1;
            }
        }
    }
    out
}

/// The first member of `link`, in track order.
pub fn first_member(project: &Project, link: LinkId) -> Option<&Clip> {
    project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find(|c| c.link == Some(link))
}

/// The first member of `link` other than `except`, if any.
pub fn first_other(project: &Project, link: LinkId, except: ClipId) -> Option<&Clip> {
    project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find(|c| c.link == Some(link) && c.id != except)
}

/// The invariant: every group's members share their content. `Err` names the first group
/// that does not.
pub fn check(project: &Project) -> Result<(), String> {
    let mut first: HashMap<LinkId, &Clip> = HashMap::new();
    for t in &project.tracks {
        for c in &t.clips {
            let Some(l) = c.link else { continue };
            match first.get(&l) {
                None => {
                    first.insert(l, c);
                }
                Some(f) if !same_content(f, c) => {
                    return Err(format!("link {l}: {} and {} differ", f.id, c.id));
                }
                Some(_) => {}
            }
        }
    }
    Ok(())
}

/// Restore the invariant in a project read from a file (edited by hand, or written by a
/// build with a bug): a member whose content differs from its group's first member (in
/// track order) leaves the group and keeps its content. Returns how many left. Not a
/// command: it runs on load, before the project has a history.
pub fn repair(project: &mut Project) -> usize {
    let mut first: HashMap<LinkId, Clip> = HashMap::new();
    let mut left = 0;
    for t in &mut project.tracks {
        for c in &mut t.clips {
            let Some(l) = c.link else { continue };
            match first.get(&l) {
                None => {
                    first.insert(l, c.clone());
                }
                Some(f) if !same_content(f, c) => {
                    c.link = None;
                    left += 1;
                }
                Some(_) => {}
            }
        }
    }
    left
}

/// Linked copies within one batch. A clip copied twice in the same paste or duplicate
/// must land in one group, not in two, so the group a clip of its own is given is
/// remembered here.
#[derive(Debug, Default)]
pub struct Linker {
    made: HashMap<ClipId, LinkId>,
}

impl Linker {
    pub fn new() -> Self {
        Linker::default()
    }

    /// The group a linked copy of clip `clip` (on `track`, in group `link`) joins: its own,
    /// or — for a clip of its own — a new one, with the command (to run **before** the
    /// copy's `InsertClip`) that puts the clip in it.
    pub fn group_of(
        &mut self,
        track: TrackId,
        clip: ClipId,
        link: Option<LinkId>,
        ids: &mut dyn FnMut() -> u64,
    ) -> (Vec<Command>, LinkId) {
        if let Some(l) = link.or_else(|| self.made.get(&clip).copied()) {
            return (Vec::new(), l);
        }
        let l = LinkId(ids());
        self.made.insert(clip, l);
        let pre = vec![Command::SetClipLink {
            track,
            clip,
            link: Some(l),
        }];
        (pre, l)
    }

    /// A linked copy of `clip` (on `track`) starting at `start`: a fresh clip id, the same
    /// content — the same note ids too, as the two halves of a split have — and the same
    /// group (see [`Self::group_of`] for the commands returned).
    pub fn copy(
        &mut self,
        track: TrackId,
        clip: &Clip,
        start: Ticks,
        ids: &mut dyn FnMut() -> u64,
    ) -> (Vec<Command>, Clip) {
        let (pre, link) = self.group_of(track, clip.id, clip.link, ids);
        let copy = Clip {
            id: ClipId(ids()),
            start,
            link: Some(link),
            ..clip.clone()
        };
        (pre, copy)
    }
}

/// Take each clip of `sel` out of its group. It keeps its content, which from now on is
/// its own: what changes in the others no longer reaches it.
pub fn make_unique(project: &Project, sel: &[ClipRef]) -> Vec<Command> {
    let mut seen: Vec<ClipRef> = Vec::new();
    let mut out = Vec::new();
    for r in sel {
        if seen.contains(r) {
            continue;
        }
        seen.push(*r);
        let Some(c) = project.track(r.0).and_then(|t| t.clip(r.1)) else {
            continue;
        };
        if c.link.is_some() {
            out.push(Command::SetClipLink {
                track: r.0,
                clip: r.1,
                link: None,
            });
        }
    }
    out
}

/// Where a clip sits in its content: what a copy shows, as opposed to where it shows it.
fn shown(c: &Clip) -> (Ticks, Ticks, Option<Ticks>) {
    (c.offset, c.length, c.content_length)
}

/// The siblings that follow a grown copy (ADR 0037 §3): `commands` edit the content of
/// `edited` and also change its window or loop so that what they add is heard (a paste or
/// a duplicate of notes past its end, an overdub). Every sibling that showed exactly what
/// `edited` showed — the same offset, length and loop, which is every copy nobody trimmed
/// — gets the same change, moved to where it sits; a sibling trimmed by hand keeps its
/// window, as FL keeps a resized pattern clip. Returns the commands to add to the batch
/// (none when `edited` is in no group or its window does not change). A sibling the change
/// would push before the start of the song is left alone.
pub fn follow_window(project: &Project, edited: ClipRef, commands: &[Command]) -> Vec<Command> {
    let Some(before) = project.track(edited.0).and_then(|t| t.clip(edited.1)) else {
        return Vec::new();
    };
    if before.link.is_none() {
        return Vec::new();
    }
    let mut after = before.clone();
    for cmd in commands {
        match cmd {
            Command::SetClipBounds {
                track,
                clip,
                start,
                length,
            } if (*track, *clip) == edited => {
                after.start = *start;
                after.length = *length;
            }
            Command::SetClipOffset {
                track,
                clip,
                offset,
            } if (*track, *clip) == edited => after.offset = *offset,
            Command::SetClipLoop {
                track,
                clip,
                content_length,
            } if (*track, *clip) == edited => after.content_length = *content_length,
            _ => {}
        }
    }
    if shown(&after) == shown(before) && after.start == before.start {
        return Vec::new();
    }
    let delta = after.start - before.start;
    let mut out = Vec::new();
    for (track, id) in siblings(project, edited) {
        let Some(s) = project.track(track).and_then(|t| t.clip(id)) else {
            continue;
        };
        let start = s.start + delta;
        if shown(s) != shown(before) || start.is_negative() {
            continue;
        }
        if start != s.start || after.length != s.length {
            out.push(Command::SetClipBounds {
                track,
                clip: id,
                start,
                length: after.length,
            });
        }
        if after.offset != s.offset {
            out.push(Command::SetClipOffset {
                track,
                clip: id,
                offset: after.offset,
            });
        }
        if after.content_length != s.content_length {
            out.push(Command::SetClipLoop {
                track,
                clip: id,
                content_length: after.content_length,
            });
        }
    }
    out
}

/// Whether `a` can be linked to `b`: the same kind of clip (notes, audio, or automation
/// of the same target — a curve moved onto another target would change lanes).
pub fn compatible(a: &Clip, b: &Clip) -> bool {
    match (&a.audio, &b.audio, &a.automation, &b.automation) {
        (None, None, None, None) => true,
        (Some(_), Some(_), None, None) => true,
        (None, None, Some(x), Some(y)) => x.target == y.target,
        _ => false,
    }
}

/// Link every clip of `sel` to `target`: each takes `target`'s content and joins its group
/// (a new one when `target` is in none). A clip that is already a sibling is left alone,
/// and so is one of another kind ([`compatible`]). A clip leaves its old group *before*
/// its content changes, so its old siblings keep theirs.
pub fn link_to(
    project: &Project,
    sel: &[ClipRef],
    target: ClipRef,
    ids: &mut dyn FnMut() -> u64,
) -> Vec<Command> {
    let Some(t) = project.track(target.0).and_then(|t| t.clip(target.1)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut link = t.link;
    let content = ClipContent::of(t);
    let mut seen: Vec<ClipRef> = vec![target];
    for r in sel {
        if seen.contains(r) {
            continue;
        }
        seen.push(*r);
        let Some(c) = project.track(r.0).and_then(|t| t.clip(r.1)) else {
            continue;
        };
        if !compatible(c, t) || (link.is_some() && c.link == link) {
            continue;
        }
        let l = match link {
            Some(l) => l,
            None => {
                let l = LinkId(ids());
                out.push(Command::SetClipLink {
                    track: target.0,
                    clip: target.1,
                    link: Some(l),
                });
                link = Some(l);
                l
            }
        };
        if c.link.is_some() {
            out.push(Command::SetClipLink {
                track: r.0,
                clip: r.1,
                link: None,
            });
        }
        if !same_content(c, t) {
            out.push(Command::SetClipContent {
                track: r.0,
                clip: r.1,
                content: Box::new(content.clone()),
            });
        }
        out.push(Command::SetClipLink {
            track: r.0,
            clip: r.1,
            link: Some(l),
        });
    }
    out
}

// ---- the "Link to…" list (round 26) ------------------------------------------------------

/// What a clip holds, as [`compatible`] compares it: a link joins clips of one kind only.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Notes,
    Audio,
    Automation,
}

impl Kind {
    pub fn of(c: &Clip) -> Kind {
        if c.audio.is_some() {
            Kind::Audio
        } else if c.automation.is_some() {
            Kind::Automation
        } else {
            Kind::Notes
        }
    }
}

/// One entry of the "Link to…" list: a link group of two or more clips, or a clip of its
/// own (a clip in no group, or the last member left in one).
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The group, when the clip is in one (a group of one keeps its id).
    pub link: Option<LinkId>,
    /// The member a link to this entry takes its content from: the first in track order,
    /// then by start, then by id. Its track and start tell two entries of one name apart.
    pub first: ClipRef,
    /// The first member's track, by its place in the project.
    pub track_index: usize,
    /// Where the first member starts.
    pub start: Ticks,
    /// The shared name (`ClipContent::name`).
    pub name: String,
    pub kind: Kind,
    /// What an automation entry drives (only clips of the same target link to it).
    pub target: Option<AutoTarget>,
    /// How many clips share it (one for a clip of its own).
    pub count: usize,
}

/// Every link group once and every clip of its own, for a list to pick from: the groups
/// of two or more first, then the clips of their own; each part by name — ignoring case,
/// numbers compared as numbers ("Groove 2" before "Groove 10") — then exactly, then by
/// where its first member sits. The order depends only on what the project holds, never
/// on the order its clips are stored in.
pub fn groups(project: &Project) -> Vec<Group> {
    let mut by_link: HashMap<LinkId, usize> = HashMap::new();
    let mut out: Vec<Group> = Vec::new();
    for (ti, t) in project.tracks.iter().enumerate() {
        for c in &t.clips {
            let entry = Group {
                link: c.link,
                first: (t.id, c.id),
                track_index: ti,
                start: c.start,
                name: c.name.clone(),
                kind: Kind::of(c),
                target: c.automation.as_ref().map(|a| a.target.clone()),
                count: 1,
            };
            let Some(l) = c.link else {
                out.push(entry);
                continue;
            };
            match by_link.get(&l) {
                None => {
                    by_link.insert(l, out.len());
                    out.push(entry);
                }
                Some(&i) => {
                    let g = &mut out[i];
                    g.count += 1;
                    if place(&entry) < place(g) {
                        g.first = entry.first;
                        g.track_index = entry.track_index;
                        g.start = entry.start;
                    }
                }
            }
        }
    }
    out.sort_by_cached_key(|g| (g.count < 2, natural(&g.name), g.name.clone(), place(g)));
    out
}

/// Where an entry's first member sits, as the list orders ties.
fn place(g: &Group) -> (usize, Ticks, ClipId) {
    (g.track_index, g.start, g.first.1)
}

/// A name as a list of runs, for a natural order: text in lower case, and runs of digits
/// compared as numbers (by their length without leading zeros, then their digits), a
/// number before text.
fn natural(name: &str) -> Vec<(bool, usize, String)> {
    let mut out: Vec<(bool, usize, String)> = Vec::new();
    for ch in name.chars().flat_map(char::to_lowercase) {
        let digit = ch.is_ascii_digit();
        match out.last_mut() {
            Some((text, _, run)) if *text != digit => run.push(ch),
            _ => out.push((!digit, 0, ch.to_string())),
        }
    }
    for (text, len, run) in &mut out {
        if !*text {
            let trimmed = run.trim_start_matches('0');
            *run = if trimmed.is_empty() { "0" } else { trimmed }.to_string();
            *len = run.len();
        }
    }
    out
}

/// Why linking a selection to a target would do nothing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No selected clip holds what the target holds (notes, audio, automation).
    Kind,
    /// The selection holds automation of another parameter than the target's.
    Target,
    /// A selected clip already is the target or one of its group (or none is selected).
    Already,
    /// The target is not in the project.
    Gone,
}

/// The answer from what was counted: `n` would join; `already`, `other_target` and
/// `kind` say what kept the rest out. *Already* comes first — it names the clip the user
/// can see is the same — then another control, then another kind.
fn verdict(n: usize, already: bool, other_target: bool, kind: bool) -> Result<usize, Refusal> {
    match n {
        0 if already => Err(Refusal::Already),
        0 if other_target => Err(Refusal::Target),
        0 if kind => Err(Refusal::Kind),
        0 => Err(Refusal::Already),
        n => Ok(n),
    }
}

/// What [`link_to`] would do with `sel` and `target`, without doing it: how many selected
/// clips would join the target's group, or why none would. A list shows a refused target
/// dimmed, with its reason. (For many targets at once, [`Selection`] answers each in
/// constant time.)
pub fn would_link(project: &Project, sel: &[ClipRef], target: ClipRef) -> Result<usize, Refusal> {
    let Some(t) = project.track(target.0).and_then(|x| x.clip(target.1)) else {
        return Err(Refusal::Gone);
    };
    let mut seen: Vec<ClipRef> = vec![target];
    let (mut n, mut already, mut kind, mut other_target) = (0, false, false, false);
    if sel.contains(&target) {
        already = true;
    }
    for r in sel {
        if seen.contains(r) {
            continue;
        }
        seen.push(*r);
        let Some(c) = project.track(r.0).and_then(|x| x.clip(r.1)) else {
            continue;
        };
        if !compatible(c, t) {
            if Kind::of(c) == Kind::of(t) {
                other_target = true;
            } else {
                kind = true;
            }
        } else if t.link.is_none() || c.link != t.link {
            n += 1;
        } else {
            already = true;
        }
    }
    verdict(n, already, other_target, kind)
}

/// A selection counted once, so that every entry of the list is answered in constant time
/// ([`Selection::would_link`]) — what [`would_link`] answers for one target, without going
/// through the selection again. Built once per change of the project or the selection.
#[derive(Debug, Default)]
pub struct Selection {
    /// Every selected clip that exists, once.
    clips: HashSet<ClipRef>,
    /// How many of them are notes, audio, automation.
    notes: usize,
    audio: usize,
    automation: usize,
    /// Per kind (automation per target), how many are in each group.
    in_group: HashMap<(Kind, Option<AutoTarget>, LinkId), usize>,
    /// Automation clips per target.
    per_target: HashMap<AutoTarget, usize>,
}

impl Selection {
    pub fn of(project: &Project, sel: &[ClipRef]) -> Selection {
        let mut s = Selection::default();
        let wanted: HashSet<ClipRef> = sel.iter().copied().collect();
        for t in &project.tracks {
            for c in &t.clips {
                if !wanted.contains(&(t.id, c.id)) {
                    continue;
                }
                s.clips.insert((t.id, c.id));
                let kind = Kind::of(c);
                let target = c.automation.as_ref().map(|a| a.target.clone());
                match kind {
                    Kind::Notes => s.notes += 1,
                    Kind::Audio => s.audio += 1,
                    Kind::Automation => {
                        s.automation += 1;
                        if let Some(at) = &target {
                            *s.per_target.entry(at.clone()).or_insert(0) += 1;
                        }
                    }
                }
                if let Some(l) = c.link {
                    *s.in_group.entry((kind, target, l)).or_insert(0) += 1;
                }
            }
        }
        s
    }

    /// How many clips the selection holds (existing ones, each once).
    pub fn len(&self) -> usize {
        self.clips.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }

    /// What linking the selection to entry `g` would do: exactly [`would_link`]'s answer
    /// for `g.first`, in constant time.
    pub fn would_link(&self, g: &Group) -> Result<usize, Refusal> {
        let target_in = self.clips.contains(&g.first);
        let compatible = match g.kind {
            Kind::Notes => self.notes,
            Kind::Audio => self.audio,
            Kind::Automation => g
                .target
                .as_ref()
                .and_then(|t| self.per_target.get(t))
                .copied()
                .unwrap_or(0),
        };
        let of_kind = match g.kind {
            Kind::Notes => self.notes,
            Kind::Audio => self.audio,
            Kind::Automation => self.automation,
        };
        // The members of the target's group among the compatible clips (the target itself
        // included when it is selected: a group's first member is in its group).
        let same = g
            .link
            .and_then(|l| self.in_group.get(&(g.kind, g.target.clone(), l)))
            .copied()
            .unwrap_or(0);
        let others_same = if target_in && g.link.is_some() {
            same - 1
        } else {
            same
        };
        let n = compatible - target_in as usize - others_same;
        let already = target_in || others_same > 0;
        let other_target = of_kind > compatible;
        let kind = self.clips.len() > of_kind;
        verdict(n, already, other_target, kind)
    }
}

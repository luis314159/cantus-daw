//! Clip editing: turning a selection of clips into commands.
//!
//! Everything here is a free function over a [`Project`] that returns the commands to run,
//! so the whole of split / glue / duplicate / trim can be tested without a window and
//! without an engine. [`crate::app::CantusApp`] adds the selection bookkeeping on top.
//!
//! Three rules run through all of it, and they are what make the operations reversible:
//!
//! 1. **A clip is a window onto its content** ([`Clip`]). Trimming the left edge moves
//!    `start` and `offset` by the same amount, so the notes do not slide; splitting moves
//!    neither, so the halves are the original note list twice over.
//! 2. **Split shares note ids; a copy of its own allocates new ones.** Split has to share
//!    them — that is what makes gluing the halves back together give the original clip,
//!    note for note. A linked copy (ADR 0037) shares them too: it *is* the same clip.
//!    Glue tells a note it already has from the same note heard in another place by where
//!    it lands, and gives the second a fresh id.
//! 3. **Nothing is destroyed to make an edit.** Notes outside a window are kept, so every
//!    operation here is undone by the inverse the command bus already produces.

use cantus_core::{Clip, ClipId, Color, Command, Lanes, Note, Project, Ticks, TrackId, PPQ};

/// A clip, identified the way the selection identifies it.
pub type ClipRef = (TrackId, ClipId);

/// Fresh ids from the project's monotonic source (`CommandBus::alloc_id`).
pub type Ids<'a> = &'a mut dyn FnMut() -> u64;

pub fn clip_of(project: &Project, r: ClipRef) -> Option<&Clip> {
    project.track(r.0)?.clip(r.1)
}

/// Move or resize a clip's window while its content stays put on the timeline.
///
/// This is the one operation the content offset exists for. Dragging the left edge to the
/// right hides the first bar; dragging it back out reveals it again, and in neither case
/// does a note move. `start` moving by `d` means `offset` moves by `d` too.
pub fn set_window(track: TrackId, clip: &Clip, start: Ticks, length: Ticks) -> Vec<Command> {
    let start = start.max(Ticks::ZERO);
    let length = length.max(Ticks::SIXTEENTH);
    let mut out = Vec::with_capacity(2);
    if start != clip.start || length != clip.length {
        out.push(Command::SetClipBounds {
            track,
            clip: clip.id,
            start,
            length,
        });
    }
    let offset = clip.offset + (start - clip.start);
    if offset != clip.offset {
        out.push(Command::SetClipOffset {
            track,
            clip: clip.id,
            offset,
        });
    }
    out
}

/// Move the clip along the timeline, **taking its content with it**.
///
/// This is emphatically not [`set_window`] with the same length. `set_window` is for
/// *trimming*: it moves `offset` along with `start` so the content stays anchored where it
/// sounds, which is what makes dragging an edge hide and reveal notes without moving them.
/// A move wants the opposite — the window and the content travel together — so it changes
/// `start` and leaves `offset` alone. Getting this wrong makes a clip slide out from under
/// its own notes, which is what it did until somebody dragged one and looked.
///
/// An unwarped audio clip moved where the tempo under it (or its hidden part) differs
/// plays the same seconds of its file: offset, length, fades and fine offset re-derived
/// through the map (`audio_time::relocate`), as a paste to the same place gives (review
/// 2). A linked clip's fades and fine offset are its group's: only its window moves.
pub fn move_to(project: &Project, track: TrackId, clip: &Clip, start: Ticks) -> Vec<Command> {
    let start = start.max(Ticks::ZERO);
    if start == clip.start {
        return Vec::new();
    }
    let mut moved = clip.clone();
    relocate(project, clip, &mut moved, start);
    moved_to(track, clip, &moved)
}

/// The commands that turn `clip` into `moved` (its bounds, offset and region).
fn moved_to(track: TrackId, clip: &Clip, moved: &Clip) -> Vec<Command> {
    if moved.start == clip.start
        && moved.length == clip.length
        && moved.offset == clip.offset
        && moved.audio == clip.audio
    {
        return Vec::new();
    }
    let mut out = vec![Command::SetClipBounds {
        track,
        clip: clip.id,
        start: moved.start,
        length: moved.length,
    }];
    if moved.offset != clip.offset {
        out.push(Command::SetClipOffset {
            track,
            clip: clip.id,
            offset: moved.offset,
        });
    }
    if moved.audio != clip.audio {
        out.push(Command::SetClipAudio {
            track,
            clip: clip.id,
            audio: moved.audio.clone(),
        });
    }
    out
}

/// `copy` (a copy of `clip`, or `clip` itself) laid where its chain puts it
/// (`audio_time::lay_chains`): its start, window and offset; a free copy its fades and fine
/// offset too (a linked one shares its group's).
fn lay(copy: &mut Clip, l: &cantus_core::audio_time::Laid) {
    copy.start = l.start;
    copy.length = l.length;
    copy.offset = l.offset;
    if copy.link.is_none() {
        if let Some(a) = copy.audio.as_mut() {
            a.fade_in = l.fade_in;
            a.fade_out = l.fade_out;
            a.slip_frames = l.slip_frames;
        }
    }
}

/// `copy` (a copy of `clip`, or `clip` itself) laid at `start` through the project's tempo
/// map: see [`move_to`]. Its own start is set too.
pub fn relocate(project: &Project, clip: &Clip, copy: &mut Clip, start: Ticks) {
    copy.start = start;
    if project.tempo_points.is_empty() {
        return;
    }
    let Some(a) = clip.audio.as_ref() else {
        return;
    };
    let map = crate::views::bars::map(project);
    let rate = project
        .audio_source(a.source)
        .map_or(48_000.0, |s| f64::from(s.sample_rate.max(1)));
    if let Some(m) = cantus_core::audio_time::relocate(&map, clip.start, clip, &map, start, rate) {
        copy.offset = m.offset;
        copy.length = m.length;
        if copy.link.is_none() {
            if let Some(r) = copy.audio.as_mut() {
                r.fade_in = m.fade_in;
                r.fade_out = m.fade_out;
                r.slip_frames = m.slip_frames;
            }
        }
    }
}

/// Slip: move the content under a window that stays where it is. An unwarped audio window
/// under a tempo track moves its source by the seconds `by` lasts at its start, so the
/// content follows the pointer (`audio_time::slipped`, review 3).
pub fn slip(project: &Project, track: TrackId, clip: &Clip, by: Ticks) -> Vec<Command> {
    if by == Ticks::ZERO {
        return Vec::new();
    }
    let (offset, slip) = slipped(project, clip, by);
    let mut out = vec![Command::SetClipOffset {
        track,
        clip: clip.id,
        offset,
    }];
    if let (Some(a), Some(s)) = (clip.audio.as_ref(), slip) {
        out.push(Command::SetClipAudio {
            track,
            clip: clip.id,
            audio: Some(cantus_core::AudioRegion {
                slip_frames: s,
                ..a.clone()
            }),
        });
    }
    out
}

/// A clip's content slipped by `by`: its new offset, and its new fine offset when that
/// moves too (an unwarped, unlinked audio window under a tempo track).
pub fn slipped(project: &Project, clip: &Clip, by: Ticks) -> (Ticks, Option<i32>) {
    let audio = clip
        .audio
        .as_ref()
        .filter(|a| !a.warp.as_ref().is_some_and(|w| w.follow));
    match audio {
        Some(a) if !project.tempo_points.is_empty() => {
            let map = crate::views::bars::map(project);
            let rate = project
                .audio_source(a.source)
                .map_or(48_000.0, |s| f64::from(s.sample_rate.max(1)));
            let (o, s) = cantus_core::audio_time::slipped(
                &map,
                clip.start,
                clip.offset,
                a.slip_frames,
                rate,
                by,
            );
            let s = (s != a.slip_frames && clip.link.is_none()).then_some(s);
            (o, s)
        }
        _ => (clip.offset + by, None),
    }
}

/// The commands a finished drag of `anchor` makes (`sel` the selection): what
/// [`crate::app::CantusApp::commit_clip_drag`] runs, and what the arrangement draws while
/// the drag goes on (review 3: the preview is the drop).
pub fn drag_commands(
    project: &Project,
    sel: &[ClipRef],
    anchor: ClipRef,
    result: crate::views::arrangement::ClipDragResult,
    link: bool,
    ids: Ids,
) -> (Vec<Command>, Vec<ClipRef>) {
    use crate::views::arrangement::ClipDragResult;
    let Some(clip) = clip_of(project, anchor) else {
        return (Vec::new(), Vec::new());
    };
    // A reversed audio clip maps its window backwards (ADR 0025).
    if let Some((edit, delta)) = crate::audio_clips::reversed_drag(clip, result) {
        return (
            crate::audio_clips::reversed_edit(project, anchor.0, clip, edit, delta),
            Vec::new(),
        );
    }
    // A comp window's edge at a seam moves the seam: both sides (ADR 0058).
    let seam = match result {
        ClipDragResult::TrimStart { delta } => Some((true, delta)),
        ClipDragResult::TrimEnd { delta } => Some((false, delta)),
        _ => None,
    }
    .and_then(|(start, d)| crate::comping::seam_trim(project, anchor.0, clip, start, d));
    if let Some(cmds) = seam.filter(|c| !c.is_empty()) {
        return (cmds, Vec::new());
    }
    match result {
        ClipDragResult::Move { delta, rows, copy } => {
            transpose_selection(project, sel, delta, rows, copy, link, ids)
        }
        ClipDragResult::TrimStart { delta } => (
            set_window(anchor.0, clip, clip.start + delta, clip.length - delta),
            Vec::new(),
        ),
        ClipDragResult::TrimEnd { delta } => (
            set_window(anchor.0, clip, clip.start, clip.length + delta),
            Vec::new(),
        ),
        // Dragging the content to the right moves the music later, which means the window
        // starts *earlier* in the content.
        ClipDragResult::Slip { delta } => (slip(project, anchor.0, clip, -delta), Vec::new()),
    }
}

/// The unwarped audio clips a drag would leave (`drag_commands` without copies, applied to
/// copies of the clips it touches), by clip: what the arrangement draws mid-drag — every
/// other clip lands at its start plus the drag's delta, as drawn without this. Built from
/// the audio clips of the selection alone (review 4: MIDI clips were cloned for nothing);
/// one pass, every lookup by id.
pub fn drag_preview(
    project: &Project,
    sel: &[ClipRef],
    anchor: ClipRef,
    result: crate::views::arrangement::ClipDragResult,
) -> std::collections::HashMap<ClipRef, Clip> {
    use crate::views::arrangement::ClipDragResult;
    use std::collections::HashMap;
    let mut out: HashMap<ClipRef, Clip> = HashMap::new();
    let unwarped = |c: &Clip| {
        c.audio
            .as_ref()
            .is_some_and(|a| !a.warp.as_ref().is_some_and(|w| w.follow))
    };
    let mut by_id: HashMap<ClipId, (TrackId, &Clip)> = HashMap::new();
    for t in &project.tracks {
        for c in &t.clips {
            by_id.insert(c.id, (t.id, c));
        }
    }
    let commands = match result {
        ClipDragResult::Move { delta, rows, .. } => {
            // The move's own clamps over the whole selection, then its audio alone.
            let all: Vec<(usize, Ticks)> = sel
                .iter()
                .filter_map(|r| Some((project.track_index(r.0)?, by_id.get(&r.1)?.1.start)))
                .collect();
            let Some(earliest) = all.iter().map(|(_, s)| *s).min() else {
                return out;
            };
            let delta = delta.max(-earliest);
            let top = all.iter().map(|(i, _)| *i as i32).min().unwrap_or(0);
            let bottom = all.iter().map(|(i, _)| *i as i32).max().unwrap_or(0);
            let last = project.tracks.len() as i32 - 1;
            let rows = rows.clamp(-top, last - bottom);
            let audio: Vec<ClipRef> = sel
                .iter()
                .copied()
                .filter(|r| by_id.get(&r.1).is_some_and(|(_, c)| unwarped(c)))
                .collect();
            if audio.is_empty() {
                return out;
            }
            let mut none = || 0u64;
            transpose_selection(project, &audio, delta, rows, false, false, &mut none).0
        }
        other => {
            let mut none = || 0u64;
            drag_commands(project, sel, anchor, other, false, &mut none).0
        }
    };
    for cmd in commands {
        let id = match &cmd {
            Command::SetClipBounds { clip, .. }
            | Command::SetClipOffset { clip, .. }
            | Command::SetClipAudio { clip, .. } => *clip,
            _ => continue,
        };
        let Some(&(track, clip)) = by_id.get(&id) else {
            continue;
        };
        if !unwarped(clip) {
            continue;
        }
        let c = out.entry((track, id)).or_insert_with(|| clip.clone());
        match cmd {
            Command::SetClipBounds { start, length, .. } => {
                c.start = start;
                c.length = length;
            }
            Command::SetClipOffset { offset, .. } => c.offset = offset,
            Command::SetClipAudio { audio, .. } => c.audio = audio,
            _ => {}
        }
    }
    out
}

/// Split a clip at a timeline tick (`None` when the tick is not strictly inside it). It
/// lives in `cantus_core::clipboard` because the time-range crop is built on it; both
/// halves carry the whole note list, which is why [`glue`] can put them back exactly.
pub use cantus_core::clipboard::split;

/// Glue clips on one track into a single clip spanning all of them.
///
/// The earliest clip keeps its id, its offset and its repeat; the others' notes are
/// translated into its content frame and appended — only the notes each one plays, so
/// what the glued clip plays over the later clips is what they played. A note whose id is
/// already there, at the same place, is dropped, which is what makes glueing the two
/// halves of a split give back exactly the clip that was split — they hold the same notes
/// under the same ids. When the first clip holds that note only hidden and the later clip
/// plays a different version of it (an edited split), the version heard wins; when both
/// play it (linked copies, ADR 0037), the later one comes in under a fresh id.
pub fn glue(track: TrackId, clips: &[&Clip], ids: Ids) -> Option<Vec<Command>> {
    glue_with(None, track, clips, ids)
}

/// [`glue`] under `project`'s tempo map (a reversed audio clip's windows need it:
/// `audio_clips::glue_in`).
pub fn glue_in(
    project: &Project,
    track: TrackId,
    clips: &[&Clip],
    ids: Ids,
) -> Option<Vec<Command>> {
    glue_with(Some(project), track, clips, ids)
}

fn glue_with(
    project: Option<&Project>,
    track: TrackId,
    clips: &[&Clip],
    ids: Ids,
) -> Option<Vec<Command>> {
    let mut commands = glue_any(project, track, clips, ids)?;
    // The glued clip has content of its own now (ADR 0037): when the glue changes what the
    // kept clip holds, it leaves its group first, so its siblings keep theirs. Halves of a
    // split glued back change nothing and stay linked.
    let kept: Vec<&Clip> = clips
        .iter()
        .copied()
        .filter(|c| c.link.is_some() && commands.iter().any(|cmd| changes_content(cmd, c.id)))
        .collect();
    for c in kept {
        commands.insert(
            0,
            Command::SetClipLink {
                track,
                clip: c.id,
                link: None,
            },
        );
    }
    // A comp window glued with what follows it is still the comp's (ADR 0058, schema 32):
    // the glued clip — the earliest, which the glue keeps — is a window when any piece was.
    let first = clips.iter().min_by_key(|c| (c.start, c.id.0))?;
    if !first.comp && clips.iter().any(|c| c.comp && c.lane == 0) {
        commands.push(Command::SetClipComp {
            track,
            clip: first.id,
            comp: true,
        });
    }
    Some(commands)
}

/// Whether `cmd` changes what clip `id` shares with its link group (ADR 0037).
pub fn changes_content(cmd: &Command, id: ClipId) -> bool {
    match cmd {
        Command::InsertNotes { clip, .. }
        | Command::RemoveNotes { clip, .. }
        | Command::SetNotes { clip, .. }
        | Command::SetClipAudio { clip, .. }
        | Command::SetClipAutomation { clip, .. }
        | Command::SetClipPitch { clip, .. }
        | Command::SetClipWarp { clip, .. }
        | Command::SetClipName { clip, .. }
        | Command::SetClipColor { clip, .. }
        | Command::SetClipContent { clip, .. } => *clip == id,
        _ => false,
    }
}

fn glue_any(
    project: Option<&Project>,
    track: TrackId,
    clips: &[&Clip],
    ids: Ids,
) -> Option<Vec<Command>> {
    if clips.len() < 2 {
        return None;
    }
    // Audio glues only contiguous windows on one file (ADR 0025).
    if clips.iter().any(|c| c.audio.is_some()) {
        return match project {
            Some(p) => crate::audio_clips::glue_in(p, track, clips),
            None => crate::audio_clips::glue(track, clips),
        };
    }
    // Automation glues clips of one target into one curve (ADR 0027).
    if clips.iter().any(|c| c.automation.is_some()) {
        return glue_automation(track, clips);
    }
    let mut ordered: Vec<&Clip> = clips.to_vec();
    ordered.sort_by_key(|c| (c.start, c.id.0));
    let first = ordered[0];
    let start = first.start;
    let end = ordered.iter().map(|c| c.end()).max()?;

    let mut notes: Vec<Note> = first.notes.clone();
    let mut at = Vec::new();
    // Whether `clip` plays a note stored at content tick `start`.
    let mut sounds = |clip: &Clip, start: Ticks| {
        clip.timeline_positions(start, &mut at);
        !at.is_empty()
    };
    for c in &ordered[1..] {
        // The same content tick, expressed in the first clip's frame.
        let shift = first.content_of(c.start) - c.offset;
        for n in &c.notes {
            // Only what the clip plays comes in: a note hidden by its window (outside it,
            // or past its loop) would land on music that was never there — the hidden
            // notes of a copy trimmed from the left fall on the first clip's own.
            if !sounds(c, n.start) {
                continue;
            }
            let moved = Note {
                start: n.start + shift,
                ..n.clone()
            };
            match notes.iter().position(|m| m.id == n.id) {
                // The same note seen through two windows (the halves of a split).
                Some(i) if notes[i] == moved => continue,
                // The first clip holds this note but does not play it (the hidden half of
                // an edited split): the version that was heard wins.
                Some(i) if i < first.notes.len() && !sounds(first, notes[i].start) => {
                    notes[i] = moved;
                }
                // The same note heard in another place (a linked copy, ADR 0037): a note
                // of its own in the glued clip.
                Some(_) => notes.push(Note {
                    id: cantus_core::NoteId(ids()),
                    ..moved
                }),
                None => notes.push(moved),
            }
        }
    }
    notes.sort_by_key(|n| (n.start, n.pitch));

    let mut commands = Vec::with_capacity(ordered.len() + 2);
    for c in &ordered[1..] {
        commands.push(Command::RemoveClip { track, clip: c.id });
    }
    commands.push(Command::SetClipBounds {
        track,
        clip: first.id,
        start,
        length: end - start,
    });
    // The repeat period belongs to the *content*, and gluing windows together does not
    // change the content — so it is carried over rather than cleared. It is also what
    // makes splitting a clip and gluing the halves give back the clip, exactly.
    let changed: Vec<Note> = notes
        .iter()
        .filter(|n| first.note(n.id).is_some_and(|m| m != *n))
        .cloned()
        .collect();
    if !changed.is_empty() {
        commands.push(Command::SetNotes {
            track,
            clip: first.id,
            notes: changed,
        });
    }
    let added: Vec<Note> = notes
        .into_iter()
        .filter(|n| first.note(n.id).is_none())
        .collect();
    if !added.is_empty() {
        commands.push(Command::InsertNotes {
            track,
            clip: first.id,
            notes: added,
        });
    }
    Some(commands)
}

/// Glue automation clips of one target: the windows become one, and the curve is what
/// they played — each clip's points inside its window, in the first clip's content frame,
/// with a point at each seam where the curve would otherwise change. Halves of a split
/// (one curve, one frame) glue back to exactly the clip that was split.
fn glue_automation(track: TrackId, clips: &[&Clip]) -> Option<Vec<Command>> {
    let target = &clips[0].automation.as_ref()?.target;
    if clips
        .iter()
        .any(|c| c.automation.as_ref().is_none_or(|a| &a.target != target))
    {
        return None;
    }
    let mut ordered: Vec<&Clip> = clips.to_vec();
    ordered.sort_by_key(|c| (c.start, c.id.0));
    let first = ordered[0];
    let end = ordered.iter().map(|c| c.end()).max()?;
    let curve = first.automation.as_ref()?;
    let same = ordered.iter().all(|c| {
        c.automation.as_ref().map(|a| &a.points) == Some(&curve.points)
            && c.offset - c.start == first.offset - first.start
            && c.content_length.is_none()
    });
    let mut commands: Vec<Command> = ordered[1..]
        .iter()
        .map(|c| Command::RemoveClip { track, clip: c.id })
        .collect();
    commands.push(Command::SetClipBounds {
        track,
        clip: first.id,
        start: first.start,
        length: end - first.start,
    });
    if same {
        return Some(commands);
    }
    let mut points = Vec::new();
    for c in &ordered {
        let a = c.automation.as_ref()?;
        let shift = first.content_of(c.start) - c.offset;
        let (lo, hi) = (c.offset, c.offset + c.length);
        for edge in [lo, hi] {
            if let Some(v) = a.value_at(edge.0 as f64) {
                if !a.points.iter().any(|p| p.tick == edge) {
                    points.push(cantus_core::AutoPoint::new(edge + shift, v));
                }
            }
        }
        for p in a.points.iter().filter(|p| p.tick >= lo && p.tick <= hi) {
            let mut q = *p;
            q.tick = p.tick + shift;
            points.push(q);
        }
    }
    cantus_core::automation::edit::tidy(&mut points);
    points.dedup_by(|a, b| a.tick == b.tick && a.value == b.value);
    commands.push(Command::SetClipAutomation {
        track,
        clip: first.id,
        automation: Some(cantus_core::Automation::new(target.clone(), points)),
    });
    commands.push(Command::SetClipLoop {
        track,
        clip: first.id,
        content_length: None,
    });
    Some(commands)
}

/// An independent copy of a clip: a fresh clip id **and** fresh note ids. It lives with
/// the clipboard's builders now (ADR 0036), which paste nothing but copies.
pub use cantus_core::clipboard::copy_of;

/// Duplicate each clip immediately after itself, the way `Ctrl+B` does in FL and `Ctrl+D`
/// in Live. Returns the commands and the refs of the copies, so the caller can select them
/// — pressing the shortcut twice should tile, not stack.
///
/// With `link`, each copy is a linked copy — the same clip again (ADR 0037) — otherwise a
/// copy of its own.
pub fn duplicate(
    project: &Project,
    sel: &[ClipRef],
    link: bool,
    ids: Ids,
) -> (Vec<Command>, Vec<ClipRef>) {
    let mut commands = Vec::new();
    let mut made = Vec::new();
    let mut linker = cantus_core::links::Linker::new();
    for r in sel {
        let Some(clip) = clip_of(project, *r) else {
            continue;
        };
        let mut copy = if link {
            let (pre, copy) = linker.copy(r.0, clip, clip.end(), ids);
            commands.extend(pre);
            copy
        } else {
            copy_of(clip, clip.end(), ids)
        };
        relocate(project, clip, &mut copy, clip.end());
        made.push((r.0, copy.id));
        commands.push(Command::InsertClip {
            track: r.0,
            clip: Box::new(copy),
        });
    }
    (commands, made)
}

/// Set the same field on every selected clip, skipping the ones that already have it.
pub fn set_mute(project: &Project, sel: &[ClipRef], muted: bool) -> Vec<Command> {
    sel.iter()
        .filter_map(|r| clip_of(project, *r).map(|c| (r, c)))
        .filter(|(_, c)| c.muted != muted)
        .map(|(r, c)| Command::SetClipMute {
            track: r.0,
            clip: c.id,
            muted,
        })
        .collect()
}

pub fn set_color(project: &Project, sel: &[ClipRef], color: Option<Color>) -> Vec<Command> {
    sel.iter()
        .filter_map(|r| clip_of(project, *r).map(|c| (r, c)))
        .filter(|(_, c)| c.color != color)
        .map(|(r, c)| Command::SetClipColor {
            track: r.0,
            clip: c.id,
            color,
        })
        .collect()
}

/// Turn a clip's repeat on or off. Turning it on with no length given uses the window the
/// clip has now, which is what the user is looking at when they ask for it.
pub fn set_loop(project: &Project, sel: &[ClipRef], on: bool) -> Vec<Command> {
    sel.iter()
        .filter_map(|r| clip_of(project, *r).map(|c| (r, c)))
        // An audio clip plays its file once (ADR 0025): it has no repeat to switch.
        .filter(|(_, c)| c.audio.is_none() || !on)
        .filter_map(|(r, c)| {
            let want = if on { Some(c.length) } else { None };
            (c.content_length.is_some() != on).then_some(Command::SetClipLoop {
                track: r.0,
                clip: c.id,
                content_length: want,
            })
        })
        .collect()
}

/// Move the selected clips by `delta` ticks and `rows` tracks, or copy them there (linked
/// copies with `link`, ADR 0037).
///
/// The whole selection is clamped as one: a group dragged to the left stops when its
/// *earliest* clip reaches zero, so the clips keep their spacing instead of piling up at
/// the start. The same for the top and bottom track.
pub fn transpose_selection(
    project: &Project,
    sel: &[ClipRef],
    delta: Ticks,
    rows: i32,
    copy: bool,
    link: bool,
    ids: Ids,
) -> (Vec<Command>, Vec<ClipRef>) {
    let mut commands = Vec::new();
    let mut made = Vec::new();
    let mut linker = cantus_core::links::Linker::new();
    let clips: Vec<(usize, &Clip)> = sel
        .iter()
        .filter_map(|r| Some((project.track_index(r.0)?, clip_of(project, *r)?)))
        .collect();
    if clips.is_empty() {
        return (commands, made);
    }
    let earliest = clips
        .iter()
        .map(|(_, c)| c.start)
        .min()
        .unwrap_or(Ticks::ZERO);
    let delta = delta.max(-earliest);
    let top = clips.iter().map(|(i, _)| *i as i32).min().unwrap_or(0);
    let bottom = clips.iter().map(|(i, _)| *i as i32).max().unwrap_or(0);
    let last = project.tracks.len() as i32 - 1;
    let rows = rows.clamp(-top, last - bottom);
    if delta == Ticks::ZERO && rows == 0 && !copy {
        return (commands, made);
    }
    // Every clip moves by `delta` (its bars kept) and is relocated on its own — but the
    // windows of one take moved together keep their shape (reviews 3–4,
    // `audio_time::lay_chains`): its first window by `delta`, the others the same seconds
    // after it, so windows that met still meet and a take keeps one origin.
    let laid = if project.tempo_points.is_empty() {
        vec![None; clips.len()]
    } else {
        let map = crate::views::bars::map(project);
        let members: Vec<cantus_core::audio_time::Member> = clips
            .iter()
            .filter_map(|(index, clip)| {
                let a = clip.audio.as_ref()?;
                Some(cantus_core::audio_time::Member {
                    clip: *clip,
                    row: *index as u64,
                    from: clip.start,
                    rate: project
                        .audio_source(a.source)
                        .map_or(48_000.0, |s| f64::from(s.sample_rate.max(1))),
                })
            })
            .collect();
        let mut laid_audio =
            cantus_core::audio_time::lay_chains(&map, &map, delta, &members).into_iter();
        clips
            .iter()
            .map(|(_, c)| c.audio.as_ref().and_then(|_| laid_audio.next().flatten()))
            .collect()
    };
    for ((index, clip), laid) in clips.into_iter().zip(laid) {
        // An automation clip stays in its lane under its track (ADR 0027): its target does
        // not depend on where it sits, and a lane is not a row to drop it on.
        let to = if clip.automation.is_some() {
            project.tracks[index].id
        } else {
            project.tracks[(index as i32 + rows) as usize].id
        };
        let from = project.tracks[index].id;
        if copy {
            let at = laid.map_or(clip.start + delta, |l| l.start);
            let mut copy = if link {
                let (pre, copy) = linker.copy(from, clip, at, ids);
                commands.extend(pre);
                copy
            } else {
                copy_of(clip, at, ids)
            };
            match &laid {
                Some(l) => lay(&mut copy, l),
                None => relocate(project, clip, &mut copy, at),
            }
            made.push((to, copy.id));
            commands.push(Command::InsertClip {
                track: to,
                clip: Box::new(copy),
            });
        } else {
            made.push((to, clip.id));
            if to != from {
                commands.push(Command::MoveClipToTrack {
                    from,
                    to,
                    clip: clip.id,
                });
                // A comp window moved to another track leaves its comp (ADR 0058).
                if clip.comp {
                    commands.push(Command::SetClipComp {
                        track: to,
                        clip: clip.id,
                        comp: false,
                    });
                }
            }
            match &laid {
                Some(l) => {
                    let mut moved = clip.clone();
                    lay(&mut moved, l);
                    commands.extend(moved_to(to, clip, &moved));
                }
                None => commands.extend(move_to(project, to, clip, clip.start + delta)),
            }
        }
    }
    (commands, made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::{CommandBus, Origin, Snapshot};

    /// One track, one four-bar clip, one note per bar.
    fn fixture() -> (CommandBus, TrackId, ClipId) {
        let mut bus = CommandBus::default();
        let track = bus.new_track("t");
        let tid = track.id;
        bus.execute(
            Command::InsertTrack {
                index: 0,
                track: Box::new(track),
            },
            Origin::User,
        )
        .unwrap();
        let mut clip = bus.new_clip("c", Ticks::beats(4), Ticks::beats(16));
        let cid = clip.id;
        for i in 0..4 {
            clip.notes
                .push(bus.new_note(Ticks::beats(i * 4), Ticks::QUARTER, 60 + i as u8, 100));
        }
        bus.execute(
            Command::InsertClip {
                track: tid,
                clip: Box::new(clip),
            },
            Origin::User,
        )
        .unwrap();
        (bus, tid, cid)
    }

    fn run(bus: &mut CommandBus, commands: Vec<Command>) {
        bus.execute(
            Command::Batch {
                label_key: "test".into(),
                commands,
            },
            Origin::User,
        )
        .unwrap();
    }

    /// Every note-on in the project, as `(tick, pitch)` — what the *engine* will play, so
    /// a test asserting on this cannot be fooled by the model merely looking right.
    fn heard(bus: &CommandBus) -> Vec<(i64, u8)> {
        let s = Snapshot::compile(bus.project(), 0);
        let mut v: Vec<(i64, u8)> = s
            .tracks
            .iter()
            .flat_map(|t| t.events.iter())
            .filter(|e| e.is_on())
            .map(|e| (e.tick.0, e.pitch))
            .collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn trimming_the_left_edge_does_not_move_a_single_note() {
        let (mut bus, tid, cid) = fixture();
        let before = heard(&bus);
        let clip = bus.project().track(tid).unwrap().clip(cid).unwrap().clone();
        // Hide the first bar.
        run(
            &mut bus,
            set_window(tid, &clip, Ticks::beats(8), Ticks::beats(12)),
        );
        assert_eq!(
            heard(&bus),
            before[1..].to_vec(),
            "trimming hid the wrong notes, or moved the rest"
        );
        // Give it back: the hidden note returns exactly where it was.
        let clip = bus.project().track(tid).unwrap().clip(cid).unwrap().clone();
        run(
            &mut bus,
            set_window(tid, &clip, Ticks::beats(4), Ticks::beats(16)),
        );
        assert_eq!(heard(&bus), before, "trimming is not lossless");
    }

    /// Moving a clip has to take its music with it, and trimming has to leave it where it
    /// sounds. They are opposite operations on the same two fields, which is exactly why
    /// this is asserted on the events the *engine* would play rather than on `start`: a
    /// clip that had slid out from under its own notes still had the `start` you asked
    /// for, and every test that only looked at `start` passed.
    #[test]
    fn moving_a_clip_takes_its_music_with_it_and_trimming_does_not() {
        let (mut bus, tid, cid) = fixture();
        let before = heard(&bus);
        let clip = bus.project().track(tid).unwrap().clip(cid).unwrap().clone();

        run(
            &mut bus,
            move_to(&Project::new("m"), tid, &clip, clip.start + Ticks::beats(8)),
        );
        let after = heard(&bus);
        assert_eq!(
            after,
            before
                .iter()
                .map(|(t, p)| (t + Ticks::beats(8).0, *p))
                .collect::<Vec<_>>(),
            "the clip moved but its notes stayed where they were"
        );
        let moved = bus.project().track(tid).unwrap().clip(cid).unwrap();
        assert_eq!(
            moved.offset, clip.offset,
            "a move must not slip the content"
        );
        assert_eq!(moved.length, clip.length, "a move must not resize the clip");

        // And the other direction still behaves: trimming the front leaves the rest of
        // the music exactly where it was.
        let clip = moved.clone();
        run(
            &mut bus,
            set_window(
                tid,
                &clip,
                clip.start + Ticks::beats(4),
                clip.length - Ticks::beats(4),
            ),
        );
        assert_eq!(
            heard(&bus),
            after[1..].to_vec(),
            "trimming moved the notes it kept"
        );
    }

    #[test]
    fn splitting_then_gluing_gives_back_the_clip() {
        let (mut bus, tid, cid) = fixture();
        let original = bus.project().track(tid).unwrap().clip(cid).unwrap().clone();
        let heard_before = heard(&bus);
        let mut next = 9_000u64;
        let commands = split(bus.project(), tid, &original, Ticks::beats(12), &mut || {
            next += 1;
            next
        })
        .expect("splits");
        run(&mut bus, commands);
        assert_eq!(bus.project().track(tid).unwrap().clips.len(), 2);
        // A split is a change of bookkeeping, not of music.
        assert_eq!(
            heard(&bus),
            heard_before,
            "the split changed what is played"
        );

        let halves: Vec<&Clip> = bus.project().track(tid).unwrap().clips.iter().collect();
        let commands = glue(tid, &halves, &mut || 0).expect("glues");
        run(&mut bus, commands);
        let after = bus.project().track(tid).unwrap();
        assert_eq!(after.clips.len(), 1);
        assert_eq!(heard(&bus), heard_before);
        let glued = &after.clips[0];
        assert_eq!(glued.start, original.start);
        assert_eq!(glued.length, original.length);
        assert_eq!(glued.offset, original.offset);
        assert_eq!(glued.notes, original.notes, "glue lost or duplicated notes");
    }

    #[test]
    fn a_split_at_the_edge_is_not_a_split() {
        let (bus, tid, cid) = fixture();
        let clip = bus.project().track(tid).unwrap().clip(cid).unwrap();
        let p = bus.project();
        assert!(split(p, tid, clip, clip.start, &mut || 1).is_none());
        assert!(split(p, tid, clip, clip.end(), &mut || 1).is_none());
        assert!(split(p, tid, clip, Ticks::ZERO, &mut || 1).is_none());
    }

    #[test]
    fn slipping_moves_the_music_and_leaves_the_window() {
        let (mut bus, tid, cid) = fixture();
        let clip = bus.project().track(tid).unwrap().clip(cid).unwrap().clone();
        run(
            &mut bus,
            slip(&Project::new("s"), tid, &clip, Ticks::beats(4)),
        );
        let after = bus.project().track(tid).unwrap().clip(cid).unwrap();
        assert_eq!(after.start, clip.start, "slip moved the window");
        assert_eq!(after.length, clip.length);
        // The content moved a bar earlier under the window: the first note has slipped
        // out of view and the rest have moved a bar towards the clip start.
        assert_eq!(
            heard(&bus),
            vec![
                (Ticks::beats(4).0, 61),
                (Ticks::beats(8).0, 62),
                (Ticks::beats(12).0, 63)
            ]
        );
    }

    #[test]
    fn a_looping_clip_repeats_its_content_and_nothing_else() {
        let (mut bus, tid, cid) = fixture();
        // One bar of content under four bars of window.
        run(
            &mut bus,
            vec![Command::SetClipLoop {
                track: tid,
                clip: cid,
                content_length: Some(Ticks::beats(4)),
            }],
        );
        assert_eq!(
            heard(&bus),
            vec![
                (Ticks::beats(4).0, 60),
                (Ticks::beats(8).0, 60),
                (Ticks::beats(12).0, 60),
                (Ticks::beats(16).0, 60),
            ],
            "a looping clip must repeat the first period, and only that"
        );
        run(
            &mut bus,
            vec![Command::SetClipLoop {
                track: tid,
                clip: cid,
                content_length: None,
            }],
        );
        assert_eq!(
            heard(&bus).len(),
            4,
            "turning the repeat off restores the clip"
        );
    }

    #[test]
    fn a_muted_clip_keeps_its_notes_and_makes_no_sound() {
        let (mut bus, tid, cid) = fixture();
        run(
            &mut bus,
            vec![Command::SetClipMute {
                track: tid,
                clip: cid,
                muted: true,
            }],
        );
        assert!(heard(&bus).is_empty());
        assert_eq!(
            bus.project()
                .track(tid)
                .unwrap()
                .clip(cid)
                .unwrap()
                .notes
                .len(),
            4
        );
    }

    #[test]
    fn a_copy_shares_no_ids_with_its_original() {
        let (bus, tid, cid) = fixture();
        let clip = bus.project().track(tid).unwrap().clip(cid).unwrap();
        let mut n = 9_000u64;
        let copy = copy_of(clip, Ticks::ZERO, &mut || {
            n += 1;
            n
        });
        assert_ne!(copy.id, clip.id);
        for (a, b) in copy.notes.iter().zip(&clip.notes) {
            assert_ne!(
                a.id, b.id,
                "a duplicate that shares note ids cannot be glued"
            );
            assert_eq!((a.start, a.pitch), (b.start, b.pitch));
        }
    }

    #[test]
    fn dragging_a_group_left_stops_at_zero_without_squashing_it() {
        let (mut bus, tid, _) = fixture();
        let mut second = bus.new_clip("d", Ticks::beats(24), Ticks::beats(4));
        second
            .notes
            .push(bus.new_note(Ticks::ZERO, Ticks::QUARTER, 72, 100));
        let sid = second.id;
        bus.execute(
            Command::InsertClip {
                track: tid,
                clip: Box::new(second),
            },
            Origin::User,
        )
        .unwrap();
        let sel: Vec<ClipRef> = bus
            .project()
            .track(tid)
            .unwrap()
            .clips
            .iter()
            .map(|c| (tid, c.id))
            .collect();
        let (commands, _) = transpose_selection(
            bus.project(),
            &sel,
            Ticks::beats(-100),
            0,
            false,
            false,
            &mut || 0,
        );
        run(&mut bus, commands);
        let clips = &bus.project().track(tid).unwrap().clips;
        assert_eq!(clips[0].start, Ticks::ZERO, "the group did not reach zero");
        let far = clips.iter().find(|c| c.id == sid).unwrap();
        assert_eq!(
            far.start,
            Ticks::beats(20),
            "the group lost its spacing at the wall"
        );
    }
}

/// The editor a double-click on a clip opens (round 27, ADR 0051).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ClipEditor {
    /// The piano roll, on the clip: the default for notes.
    PianoRoll,
    /// The channel rack (FL's step sequencer), on the clip's pattern.
    ChannelRack,
    /// The automation editor (ADR 0027).
    Automation,
    /// The pitch editor (round 28, ADR 0054): an audio clip's notes, in the piano roll's
    /// place.
    Pitch,
}

/// Which editor `clip` opens in, on a track whose instrument has `lanes`, derived from what
/// it holds — no hint is stored (ADR 0051 §4). Automation opens its editor and audio the
/// pitch editor (ADR 0054, which amends §4: there is an audio clip editor now). Notes open in the **channel rack only when it shows and edits them with
/// nothing lost** ([`rack_grid`]); anything else — a melody, a held note, a note off every
/// step grid, an empty clip — opens in the piano roll, which shows every note as it is.
pub fn editor_for(clip: &Clip, lanes: Lanes<'_>, current: Ticks) -> ClipEditor {
    if clip.automation.is_some() {
        return ClipEditor::Automation;
    }
    if clip.audio.is_some() {
        return ClipEditor::Pitch;
    }
    if rack_grid(clip, lanes, current).is_some() {
        ClipEditor::ChannelRack
    } else {
        ClipEditor::PianoRoll
    }
}

/// The rack's grids a step pattern is tried at, after the rack's current one: sixteenths
/// first (the rack's own default, FL's), then eighths, thirty-seconds, the triplets and
/// quarters.
const RACK_GRIDS: [Ticks; 6] = [
    Ticks(PPQ / 4),
    Ticks(PPQ / 2),
    Ticks(PPQ / 8),
    Ticks(PPQ / 6),
    Ticks(PPQ / 3),
    Ticks(PPQ),
];

/// The step grid at which the channel rack shows `clip` **with nothing lost**, if any —
/// the rack's `current` grid when it does, else the first of [`RACK_GRIDS`] that does, so
/// the clip's own grid decides and not whatever the rack was last set to. Nothing lost
/// means, at that grid `step`:
///
/// - **every note is a step**: it starts on a step of the clip's own grid (content tick
///   minus the offset, a multiple of `step`, where the rack's steps fall) and lasts no
///   longer than one (a hit, which is what a step the rack switches on is);
/// - **every note has a row of its own**: a channel's row plays one pitch, as FL's does —
///   a melodic instrument's single row adds any pitch but shows only on and off, so on a
///   chromatic instrument every note must be the **same pitch** (a melody opens the piano
///   roll, as in FL); a kit shows a row per pad and per stray pitch, so any pitch is shown;
/// - **no two notes are one step**: the same pitch on the same step twice would show as
///   one.
pub fn rack_grid(clip: &Clip, lanes: Lanes<'_>, current: Ticks) -> Option<Ticks> {
    let first = clip.notes.first()?;
    if lanes.is_chromatic() && clip.notes.iter().any(|n| n.pitch != first.pitch) {
        return None;
    }
    let mut seen: Vec<(Ticks, u8)> = clip.notes.iter().map(|n| (n.start, n.pitch)).collect();
    seen.sort_unstable();
    if seen.windows(2).any(|w| w[0] == w[1]) {
        return None;
    }
    let fits = |step: Ticks| {
        step.0 > 0
            && clip.notes.iter().all(|n| {
                (n.start - clip.offset).0.rem_euclid(step.0) == 0
                    && n.length.0 > 0
                    && n.length <= step
            })
    };
    std::iter::once(current)
        .chain(RACK_GRIDS)
        .find(|g| fits(*g))
}

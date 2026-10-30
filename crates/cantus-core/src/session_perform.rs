//! The session meets the arrangement (ADR 0076): what a session clip plays as arrangement
//! windows ([`pieces`]), a stretch of a track's arrangement cleared ([`clear_range`]), a
//! slot's clip put into the arrangement, a column or a stretch of time consolidated, and a
//! recorded performance committed as one undo step ([`commit_performance`]).

use super::{batch, fresh_clip, session, Ids};
use crate::automation::AutoTarget;
use crate::command::Command;
use crate::model::{Clip, Project, TrackId};
use crate::session::{SceneId, SessionClip, SessionCommand, Slot, SlotRef};
use crate::tempo::TempoPoint;
use crate::time::{Ticks, TimeSignature};

/// An arrangement window: where it sits and how it shows its content (`Clip`'s fields).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub start: Ticks,
    pub length: Ticks,
    pub offset: Ticks,
    pub period: Option<Ticks>,
}

/// The windows that play, from timeline tick `at` for `len` ticks, content that starts at
/// `from`, runs to `end`, and then — with a `period` — repeats `[offset, offset + period)`
/// (a session clip from its start marker, or an arrangement clip from inside its repeat).
/// One window when it starts on its loop's start; two when it starts elsewhere (the run
/// to the loop's end, then the loop); none past the end of a clip that does not repeat.
fn play_pieces(
    from: Ticks,
    end: Ticks,
    offset: Ticks,
    period: Option<Ticks>,
    at: Ticks,
    len: Ticks,
) -> Vec<Piece> {
    if len <= Ticks::ZERO {
        return Vec::new();
    }
    if let Some(p) = period {
        if from == offset {
            return vec![Piece {
                start: at,
                length: len,
                offset,
                period: Some(p),
            }];
        }
    }
    let first = (end - from).max(Ticks::ZERO);
    let mut out = Vec::with_capacity(2);
    let l1 = first.min(len);
    if l1 > Ticks::ZERO {
        out.push(Piece {
            start: at,
            length: l1,
            offset: from,
            period: None,
        });
    }
    if let Some(p) = period {
        if len > first {
            out.push(Piece {
                start: at + first,
                length: len - first,
                offset,
                period: Some(p),
            });
        }
    }
    out
}

/// What session clip `c` plays launched at timeline tick `at` from content tick `from`
/// (its start marker, or where legato took over), for `len` ticks.
pub fn pieces(c: &SessionClip, from: Ticks, at: Ticks, len: Ticks) -> Vec<Piece> {
    play_pieces(from, c.end(), c.clip.offset, c.clip.period(), at, len)
}

/// What arrangement clip `c` plays over `[a, b)` (inside it), as windows.
fn arrangement_pieces(c: &Clip, a: Ticks, b: Ticks) -> Vec<Piece> {
    let len = b - a;
    match c.period() {
        None => vec![Piece {
            start: a,
            length: len,
            offset: c.content_of(a),
            period: None,
        }],
        Some(p) => {
            let r = Ticks((a - c.start).0.rem_euclid(p.0));
            play_pieces(c.offset + r, c.offset + p, c.offset, Some(p), a, len)
        }
    }
}

/// Where a placed window's id comes from: fresh ids (an edit: its notes re-id'd too), or
/// **keyed** (a performance): the same window — the same clip at the same place — always
/// gets the same id and keeps its notes' ids, so the live projection of a performance and
/// its commit are the same clips, and play the same voice tags and round-robin turns.
pub(crate) enum IdSrc<'a> {
    Fresh(Ids<'a>),
    Keyed,
}

/// A keyed id: high in the id space (`Project::alloc_id` counts up from 1 and never gets
/// there), mixed from what the window is.
pub(crate) fn keyed_id(a: u64, b: Ticks, k: u64) -> u64 {
    let mix = |mut z: u64| {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    (mix(a ^ mix(b.0 as u64 ^ mix(k))) & ((1 << 62) - 1)) | (1 << 62)
}

impl IdSrc<'_> {
    fn place(&mut self, base: &Clip, p: Piece, k: u64) -> Clip {
        let mut c = match self {
            IdSrc::Fresh(ids) => fresh_clip(base, ids),
            IdSrc::Keyed => {
                let mut c = base.clone();
                c.id = crate::model::ClipId(keyed_id(base.id.0, p.start, k));
                c.link = None;
                c.comp = false;
                c.lane = 0;
                c
            }
        };
        c.start = p.start;
        c.length = p.length;
        c.offset = p.offset;
        c.content_length = p.period;
        c
    }
}

/// The fade a cut gives an audio window where the session started or stopped it inside its
/// content (the click-free edge): a 128th note, never more than the window.
pub const CUT_FADE: Ticks = Ticks(crate::time::PPQ / 32);

/// The edges of an audio window made by a cut: its own fade where it starts (or ends) its
/// content's run, the click-free [`CUT_FADE`] where a cut made the edge.
fn cut_edges(c: &mut Clip, own_in: bool, own_out: bool) {
    let len = c.length;
    if let Some(a) = c.audio.as_mut() {
        if !own_in {
            a.fade_in = CUT_FADE.min(len);
        }
        if !own_out {
            a.fade_out = CUT_FADE.min(len);
        }
    }
}

/// Clear `[from, to)` of what `track` plays — or, with `targets`, of the automation clips
/// of those targets (on any track): every window there taken out, the ones crossing an
/// edge cut at it (the part outside kept as windows that play exactly what it played, an
/// audio window's cut edge with the click-free fade).
pub fn clear_range(
    project: &Project,
    track: Option<TrackId>,
    targets: &[AutoTarget],
    from: Ticks,
    to: Ticks,
    ids: Ids,
) -> Vec<Command> {
    clear_range_with(project, track, targets, from, to, &mut IdSrc::Fresh(ids))
}

fn clear_range_with(
    project: &Project,
    track: Option<TrackId>,
    targets: &[AutoTarget],
    from: Ticks,
    to: Ticks,
    ids: &mut IdSrc,
) -> Vec<Command> {
    let mut out = Vec::new();
    if to <= from {
        return out;
    }
    for t in &project.tracks {
        for c in &t.clips {
            let hit = match &c.automation {
                None => Some(t.id) == track && c.lane == 0,
                Some(a) => targets.contains(&a.target),
            };
            if !hit || c.end() <= from || c.start >= to {
                continue;
            }
            if c.start < from {
                out.push(Command::SetClipBounds {
                    track: t.id,
                    clip: c.id,
                    start: c.start,
                    length: from - c.start,
                });
                if let Some(a) = &c.audio {
                    let mut a = a.clone();
                    a.fade_out = CUT_FADE.min(from - c.start);
                    a.fade_in = a.fade_in.min(from - c.start);
                    out.push(Command::SetClipAudio {
                        track: t.id,
                        clip: c.id,
                        audio: Some(a),
                    });
                }
            } else {
                out.push(Command::RemoveClip {
                    track: t.id,
                    clip: c.id,
                });
            }
            if c.end() > to {
                let ps = arrangement_pieces(c, to, c.end());
                for (i, p) in ps.iter().enumerate() {
                    let mut k = ids.place(c, *p, 1 + i as u64);
                    cut_edges(&mut k, false, i + 1 == ps.len());
                    out.push(Command::InsertClip {
                        track: t.id,
                        clip: Box::new(k),
                    });
                }
            }
        }
    }
    out
}

/// One clip heard in a performance: the slot, the timeline tick it started at and the
/// content tick it started from, the tick it stopped (or the recording ended) at, and the
/// gain its launch velocity gave it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Played {
    pub slot: SlotRef,
    pub at: Ticks,
    pub from: Ticks,
    pub until: Ticks,
    pub gain: f32,
    /// The launch's velocity (what a follow action carries on).
    pub velocity: f32,
    /// The tick the clip would have been launched at to be where it is (its own launch;
    /// for a legato or caught-up start, the launch it keeps in step with).
    pub origin: Ticks,
}

/// A performance: what the session played, ready to be heard as arrangement (the live
/// projection, `session::play`) or written into it (arrangement record).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Performance {
    /// Per track, the stretches the session held it — from a launch or a stop until it went
    /// back to the arrangement or the recording ended: its arrangement there is replaced.
    pub held: Vec<(TrackId, Ticks, Ticks)>,
    /// Every clip heard, in the order heard.
    pub played: Vec<Played>,
    /// The tempos scenes set, at the ticks they took effect.
    pub tempos: Vec<(Ticks, f64)>,
    /// The metres scenes set, at the bar lines they took effect.
    pub metres: Vec<(Ticks, TimeSignature)>,
}

impl Performance {
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
            && self.played.is_empty()
            && self.tempos.is_empty()
            && self.metres.is_empty()
    }

    /// Only what lies in `[from, to)`: each stretch and clip cut to it — what arrangement
    /// record keeps of a performance that began before it was switched on (a clip that
    /// began before `from` is heard there from where it had got to).
    pub fn within(
        &self,
        from: Ticks,
        to: Ticks,
        clip_of: impl Fn(SlotRef) -> Option<SessionClip>,
    ) -> Performance {
        let held = self
            .held
            .iter()
            .map(|&(t, a, b)| (t, a.max(from), b.min(to)))
            .filter(|(_, a, b)| a < b)
            .collect();
        let mut played = Vec::new();
        for p in &self.played {
            let (a, b) = (p.at.max(from), p.until.min(to));
            if a >= b {
                continue;
            }
            let mut q = Played { until: b, ..*p };
            if a > p.at {
                let Some(c) = clip_of(p.slot) else {
                    continue;
                };
                let Some(at) = c.content_from(p.from, a - p.at) else {
                    continue;
                };
                q.at = a;
                q.from = at;
            }
            played.push(q);
        }
        // The tempo and metre in force at `from` come with it (a scene's, set before the
        // recording began): the tempo where it begins, the metre on the bar line it set.
        let mut tempos: Vec<(Ticks, f64)> = Vec::new();
        if let Some(&(_, bpm)) = self.tempos.iter().rfind(|(t, _)| *t < from) {
            tempos.push((from, bpm));
        }
        tempos.extend(
            self.tempos
                .iter()
                .copied()
                .filter(|(t, _)| *t >= from && *t < to),
        );
        let mut metres: Vec<(Ticks, TimeSignature)> = Vec::new();
        if let Some(&m) = self.metres.iter().rfind(|(t, _)| *t < from) {
            metres.push(m);
        }
        metres.extend(
            self.metres
                .iter()
                .copied()
                .filter(|(t, _)| *t >= from && *t < to),
        );
        Performance {
            held,
            played,
            tempos: last_per_tick(tempos),
            metres: last_per_tick(metres),
        }
    }
}

/// Points in tick order, one per tick: the last set there (what is in force) — the one in
/// force before a window and one set on its first tick are not both kept.
fn last_per_tick<T>(points: Vec<(Ticks, T)>) -> Vec<(Ticks, T)> {
    let mut out: Vec<(Ticks, T)> = Vec::with_capacity(points.len());
    for (t, v) in points {
        match out.last_mut() {
            Some(last) if last.0 == t => last.1 = v,
            _ => out.push((t, v)),
        }
    }
    out
}

/// Clip `c` (a slot's) placed as windows heard from `at` (content `from`) for `len`, its
/// envelopes as automation clips under it, at `gain`.
fn heard(
    c: &SessionClip,
    from: Ticks,
    at: Ticks,
    len: Ticks,
    gain: f32,
    ids: &mut IdSrc,
) -> Vec<Clip> {
    let ps = pieces(c, from, at, len);
    let ended = !c.looping() && len >= c.end() - from;
    let mut base = c.clip.clone();
    if gain != 1.0 {
        for n in &mut base.notes {
            n.velocity = ((f32::from(n.velocity) * gain).round() as u8).clamp(1, 127);
        }
        if let Some(a) = base.audio.as_mut() {
            a.gain_db += crate::model::linear_to_db(gain);
        }
    }
    let mut out = Vec::new();
    for (i, p) in ps.iter().enumerate() {
        let mut k = ids.place(&base, *p, 2 + i as u64);
        cut_edges(
            &mut k,
            i == 0 && from == c.play_from(),
            ended && i + 1 == ps.len(),
        );
        out.push(k);
    }
    for (e_i, e) in c.envelopes.iter().enumerate() {
        for (i, p) in ps.iter().enumerate() {
            let mut k = ids.place(&base, *p, 16 + 4 * e_i as u64 + i as u64);
            k.notes.clear();
            k.audio = None;
            k.lanes.clear();
            k.groove = None;
            k.automation = Some(e.clone());
            out.push(k);
        }
    }
    out
}

/// Put a slot's clip into the arrangement at `at` on its track, playing `len` (one pass
/// from its start marker when `None`), what was there replaced.
pub fn slot_to_arrangement(
    project: &Project,
    slot: SlotRef,
    at: Ticks,
    len: Option<Ticks>,
    ids: Ids,
) -> Option<Command> {
    let c = project.slot_clip(slot)?;
    let len = len.unwrap_or_else(|| c.first_pass());
    if at.is_negative() || len <= Ticks::ZERO {
        return None;
    }
    let targets: Vec<AutoTarget> = c.envelopes.iter().map(|e| e.target.clone()).collect();
    let mut src = IdSrc::Fresh(ids);
    let mut out = clear_range_with(project, Some(slot.track), &targets, at, at + len, &mut src);
    for k in heard(c, c.play_from(), at, len, 1.0, &mut src) {
        out.push(Command::InsertClip {
            track: slot.track,
            clip: Box::new(k),
        });
    }
    batch("cmd.session.to_arrangement", out)
}

/// Applies `cmds` to `scratch`, keeping them (a builder reading what the earlier steps
/// left). A clip whose keyed id the track already holds (a performance committed over an
/// earlier one) takes the next key; any other command the project refuses is dropped.
/// Applies `cmds` to `scratch` (see the doc above); with `keep` false the commands are applied and dropped — the projection needs
/// the project, not them, and is spared a copy of every clip it lays.
fn run_as(scratch: &mut Project, cmds: Vec<Command>, out: &mut Vec<Command>, keep: bool) {
    for mut c in cmds {
        // A keyed id the track holds already takes the next key.
        for _ in 0..8 {
            let taken = match &c {
                Command::InsertClip { track, clip } => scratch
                    .track(*track)
                    .is_some_and(|t| t.clip(clip.id).is_some()),
                _ => false,
            };
            if !taken {
                break;
            }
            if let Command::InsertClip { clip, .. } = &mut c {
                clip.id = crate::model::ClipId(keyed_id(clip.id.0, Ticks::ZERO, 99));
            }
        }
        if keep {
            if c.clone().apply(scratch).is_ok() {
                out.push(c);
            }
        } else {
            let _ = c.apply(scratch);
        }
    }
}

/// The commands that write `perf` into `project`'s arrangement, and the project they make:
/// each track's held stretches cleared, every clip heard written where it was heard (its
/// envelopes as automation clips, the automation of their targets cleared under them), the
/// scenes' tempos and metres written into the tempo track. Keyed ids: the same performance
/// makes the same clips — what the live projection plays is what a commit writes.
pub fn perform(project: &Project, perf: &Performance) -> (Project, Vec<Command>) {
    perform_as(project, perf, true)
}

/// [`perform`]; `keep` false gives the project alone (the projection's).
pub fn perform_as(project: &Project, perf: &Performance, keep: bool) -> (Project, Vec<Command>) {
    let mut scratch = project.clone();
    let mut out = Vec::new();
    let run = |scratch: &mut Project, cmds: Vec<Command>, out: &mut Vec<Command>| {
        run_as(scratch, cmds, out, keep)
    };
    let mut ids = IdSrc::Keyed;
    for &(t, a, b) in &perf.held {
        let cmds = clear_range_with(&scratch, Some(t), &[], a, b, &mut ids);
        run(&mut scratch, cmds, &mut out);
    }
    for p in &perf.played {
        let Some(c) = project.slot_clip(p.slot) else {
            continue;
        };
        let len = p.until - p.at;
        if len <= Ticks::ZERO {
            continue;
        }
        let targets: Vec<AutoTarget> = c.envelopes.iter().map(|e| e.target.clone()).collect();
        if !targets.is_empty() {
            let cmds = clear_range_with(&scratch, None, &targets, p.at, p.until, &mut ids);
            run(&mut scratch, cmds, &mut out);
        }
        let cmds = heard(c, p.from, p.at, len, p.gain, &mut ids)
            .into_iter()
            .map(|k| Command::InsertClip {
                track: p.slot.track,
                clip: Box::new(k),
            })
            .collect();
        run(&mut scratch, cmds, &mut out);
    }
    if !perf.tempos.is_empty() {
        let mut points: Vec<TempoPoint> = if scratch.tempo_points.is_empty() {
            vec![TempoPoint {
                tick: Ticks::ZERO,
                bpm: scratch.tempo_bpm,
                ramp: false,
            }]
        } else {
            scratch.tempo_points.clone()
        };
        for &(tick, bpm) in &perf.tempos {
            points.retain(|p| p.tick != tick);
            let i = points.partition_point(|p| p.tick < tick);
            points.insert(
                i,
                TempoPoint {
                    tick,
                    bpm,
                    ramp: false,
                },
            );
        }
        let bpm = points[0].bpm;
        // Unwarped audio keeps its place in its file through the new tempo, as the bus
        // keeps it for every tempo edit (`tempo::keep`): the projection heard and the
        // commit made agree.
        let cmd = Command::SetTempoTrack { bpm, points };
        let kept = crate::tempo::keep::before(&scratch, &cmd);
        run(&mut scratch, vec![cmd], &mut out);
        if let Some(k) = kept {
            let mut fix = crate::tempo::keep::after(&k, &scratch);
            // The session's own clips are the song's, not the performance's.
            fix.retain(|c| !matches!(c, Command::Session(_)));
            run(&mut scratch, fix, &mut out);
        }
    }
    for &(tick, signature) in &perf.metres {
        run(
            &mut scratch,
            vec![Command::SetMetreAt { tick, signature }],
            &mut out,
        );
    }
    (scratch, out)
}

/// **Commit a performance to the arrangement** (arrangement record) as one undo step: what
/// [`perform`] writes, as one `Batch` — the arrangement then plays exactly what was heard.
pub fn commit_performance(project: &Project, perf: &Performance) -> Option<Command> {
    batch("cmd.session.commit_performance", perform(project, perf).1)
}

// Consolidate a column: the note clips of `track` in `scenes` (in that order) joined into
/// one looping clip — each one's first pass after the other — in the first scene's slot,
/// the others emptied. `None` for fewer than two clips or an audio one (joining audio is a
/// render: bounce them instead).
pub fn consolidate_column(
    project: &Project,
    track: TrackId,
    scenes: &[SceneId],
    ids: Ids,
) -> Option<Command> {
    let t = project.track(track)?;
    let clips: Vec<(SceneId, &SessionClip)> = scenes
        .iter()
        .filter_map(|s| t.slot_clip(*s).map(|c| (*s, c)))
        .collect();
    // A clip with envelopes of its own, or a groove or a pass by the note effects that
    // the others do not share, would not play as it did (review of delivery 1, #8).
    let differs = |c: &SessionClip| {
        !c.envelopes.is_empty()
            || c.clip.groove != clips[0].1.clip.groove
            || c.clip.skip_note_fx != clips[0].1.clip.skip_note_fx
    };
    if clips.len() < 2
        || clips
            .iter()
            .any(|(_, c)| c.clip.audio.is_some() || differs(c))
    {
        return None;
    }
    let first = clips[0].1;
    let mut joined = fresh_clip(&first.clip, ids);
    joined.notes.clear();
    joined.lanes.clear();
    joined.offset = Ticks::ZERO;
    let mut at = Ticks::ZERO;
    for (_, c) in &clips {
        let from = c.play_from();
        let len = c.first_pass();
        for n in c
            .clip
            .notes
            .iter()
            .filter(|n| n.start >= from && n.start < from + len)
        {
            let mut k = n.clone();
            k.id = crate::model::NoteId(ids());
            k.start = at + (n.start - from);
            joined.notes.push(k);
        }
        for lane in &c.clip.lanes {
            let i = match joined.lanes.iter().position(|l| l.kind == lane.kind) {
                Some(i) => i,
                None => {
                    joined
                        .lanes
                        .push(crate::clip_lanes::ClipLane::new(lane.kind, Vec::new()));
                    joined.lanes.len() - 1
                }
            };
            let pts = &mut joined.lanes[i].points;
            if let Some(v) = super::curve_at(&lane.points, from) {
                pts.push(crate::automation::AutoPoint::new(at, v));
            }
            for p in lane
                .points
                .iter()
                .filter(|p| p.tick > from && p.tick < from + len)
            {
                let mut q = *p;
                q.tick = at + (p.tick - from);
                pts.push(q);
            }
        }
        at += len;
    }
    joined.length = at;
    joined.content_length = Some(at);
    let mut sc = SessionClip::new(joined);
    sc.launch = first.launch;
    sc.follow = first.follow;
    let mut out = Vec::new();
    for (s, _) in &clips {
        out.push(session(SessionCommand::RemoveSlotClip {
            slot: SlotRef::new(track, *s),
        }));
    }
    out.push(session(SessionCommand::InsertSlotClip {
        slot: SlotRef::new(track, clips[0].0),
        clip: Box::new(sc),
    }));
    batch("cmd.session.consolidate_column", out)
}

/// Live's *Consolidate Time to New Scene*: `[from, to)` of every track's note clips as a new
/// looping clip in a new scene at `index` (the notes heard there, the window's length; a
/// stretch one unlooped audio window covers becomes that audio's window).
pub fn consolidate_time(
    project: &Project,
    from: Ticks,
    to: Ticks,
    index: usize,
    name: &str,
    ids: Ids,
) -> Option<Command> {
    if to <= from || index > project.session.scenes.len() {
        return None;
    }
    let len = to - from;
    let scene = crate::session::Scene::new(SceneId(ids()), name);
    let mut slots = Vec::new();
    let mut at = Vec::new();
    for t in &project.tracks {
        let playing: Vec<&Clip> = t
            .clips
            .iter()
            .filter(|c| c.lane == 0 && c.automation.is_none() && !c.muted)
            .filter(|c| c.start < to && c.end() > from)
            .collect();
        if playing.is_empty() {
            continue;
        }
        let mut clip = if let [a] = playing[..] {
            if a.audio.is_some() && a.period().is_none() && a.start <= from && a.end() >= to {
                let mut k = fresh_clip(a, ids);
                k.start = Ticks::ZERO;
                k.offset = a.content_of(from);
                k.length = len;
                k.content_length = Some(len);
                cut_edges(&mut k, false, false);
                Some(k)
            } else {
                None
            }
        } else {
            None
        };
        if clip.is_none() {
            let notes_of = playing.iter().find(|c| c.audio.is_none());
            let Some(base) = notes_of else {
                continue;
            };
            let mut k = fresh_clip(base, ids);
            k.notes.clear();
            k.lanes.clear();
            k.start = Ticks::ZERO;
            k.offset = Ticks::ZERO;
            k.length = len;
            k.content_length = Some(len);
            for c in playing.iter().filter(|c| c.audio.is_none()) {
                for n in &c.notes {
                    c.timeline_positions(n.start, &mut at);
                    for &pos in &at {
                        if pos >= from && pos < to {
                            let mut m = n.clone();
                            m.id = crate::model::NoteId(ids());
                            m.start = pos - from;
                            k.notes.push(m);
                        }
                    }
                }
            }
            k.notes.sort_by_key(|n| (n.start, n.pitch, n.id));
            clip = Some(k);
        }
        if let Some(k) = clip {
            slots.push((
                t.id,
                Slot {
                    scene: scene.id,
                    clip: Some(SessionClip::new(k)),
                    stop: true,
                },
            ));
        }
    }
    batch(
        "cmd.session.consolidate_time",
        vec![session(SessionCommand::InsertScene {
            index,
            scene: Box::new(scene),
            slots,
        })],
    )
}

// r33:session
/// The session's warped audio clips laid as a launch lays them — their first pass and one
/// more, keyed windows — on scratch songs at the tempos they are launched at (the song's,
/// and each scene's own for its row): what the engine attaches ahead so the warp renders a
/// launch plays are made before it (ADR 0076 §7, N5). The render of a window inside one
/// held tempo does not depend on where it lies, so these are the renders a launch on a
/// boundary plays; a launch caught up, or under another scene's tempo, plays the bridge
/// until its own arrives, as an edit does. Empty without warped session audio.
pub fn warp_ahead(project: &Project) -> Vec<Project> {
    let warped = |c: &SessionClip| c.clip.audio.as_ref().is_some_and(|a| a.warp.is_some());
    let mut tempos: Vec<(f64, Option<SceneId>)> = vec![(project.tempo_bpm, None)];
    for sc in &project.session.scenes {
        if let Some(bpm) = sc.tempo {
            if bpm != project.tempo_bpm {
                tempos.push((bpm, Some(sc.id)));
            }
        }
    }
    let mut out = Vec::new();
    for (bpm, scene) in tempos {
        let mut q = Project::new("ahead");
        q.tempo_bpm = bpm;
        q.time_signature = project.time_signature;
        for t in &project.tracks {
            for s in t
                .slots
                .iter()
                .filter(|s| scene.is_none_or(|id| s.scene == id))
            {
                let Some(c) = s.clip.as_ref().filter(|c| warped(c)) else {
                    continue;
                };
                let len = if c.looping() {
                    c.first_pass() + c.pass()
                } else {
                    c.end() - c.play_from()
                };
                if len <= Ticks::ZERO {
                    continue;
                }
                let mut k = q.make_track(&t.name);
                k.clips = heard(c, c.play_from(), Ticks::ZERO, len, 1.0, &mut IdSrc::Keyed);
                k.clips.retain(|x| x.audio.is_some());
                q.tracks.push(k);
            }
        }
        if q.tracks.is_empty() {
            continue;
        }
        q.audio_pool = project
            .audio_pool
            .iter()
            .filter(|src| {
                q.tracks
                    .iter()
                    .flat_map(|t| &t.clips)
                    .any(|c| c.audio.as_ref().is_some_and(|a| a.source == src.id))
            })
            .cloned()
            .collect();
        out.push(q);
    }
    out
}

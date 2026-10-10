//! Cut, copy and paste (ADR 0036): the system clipboard, what each view copies, and where
//! a paste lands.
//!
//! The commands are built by `cantus_core::clipboard`; this module adds what needs the
//! application: the selection and the focus, the playhead and the grid, the files behind
//! audio clips, and the system clipboard.
//!
//! **The system clipboard is the clipboard.** A copy writes the [`Clipboard`] as JSON text
//! (`ctx.copy_text`), so it survives closing Cantus and reaches another project or another
//! Cantus window; the app also keeps it in memory, which is what a paste falls back on when
//! the system clipboard holds something else (text from another program, a failed write,
//! a headless test).
//!
//! **The keys arrive as events.** On every desktop egui-winit turns Ctrl+C, Ctrl+X and
//! Ctrl+V (and Ctrl+Ins, Shift+Del, Shift+Ins) into `Event::Copy`, `Event::Cut` and
//! `Event::Paste(text)` — never into key presses — so `Keymap::pressed` reads them as the
//! chords they came from, and the paste text is taken from the event. A paste from a menu
//! has no event: it asks the window for the clipboard (`ViewportCommand::RequestPaste`) and
//! pastes when the answer arrives — two frames later in eframe — or from memory when none
//! comes in time ([`ANSWER_FRAMES`], [`ANSWER_SECS`]); an answer that comes after that is
//! dropped, not pasted again. When the system clipboard holds no text at all (an image),
//! egui-winit sends nothing for Ctrl+V — no event and no key — so Ctrl+V cannot be seen;
//! Edit ▸ Paste still pastes what Cantus copied last.

use crate::app::{CantusApp, EditFocus};
use crate::clips::{self, ClipRef};
use crate::i18n::{t, tf};
use cantus_core::clipboard::{self as cb, ClipOrigin, Clipboard, Payload, SourcePlan};
use cantus_core::{
    AudioSourceId, Clip, ClipId, Command, Note, NoteId, Project, Ticks, TrackId, PPQ,
};
use cantus_project::SCHEMA_VERSION;
use std::collections::HashMap;
use std::path::Path;

/// Which arrangement selection was touched last: it decides what an arrangement copy or
/// cut takes, so a time range left lying around never takes over a clip copy.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SelKind {
    #[default]
    Clips,
    Time,
}

/// A paste from a menu, waiting for the system clipboard's answer: when it was asked.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PendingPaste {
    /// `ctx.cumulative_frame_nr()` of the frame that asked.
    pub frame: u64,
    /// Input time (seconds) of that frame.
    pub time: f64,
}

/// How long a menu paste waits before it gives up on the system clipboard and pastes what
/// Cantus copied last. eframe carries out a `RequestPaste` sent in frame N during frame
/// N+1's paint and delivers the answer in frame N+2 — never sooner — so the wait is at
/// least [`ANSWER_FRAMES`] frames **and** [`ANSWER_SECS`] seconds. When the clipboard
/// holds no text (an image), egui-winit sends no answer at all.
pub const ANSWER_FRAMES: u64 = 3;
pub const ANSWER_SECS: f64 = 0.1;
/// After a menu paste gave up, an `Event::Paste` arriving with no Ctrl held within this
/// long is that request's late answer, and is dropped rather than pasted a second time.
pub const LATE_SECS: f64 = 1.0;

/// The application's side of the clipboard.
#[derive(Debug)]
pub struct ClipboardState {
    /// What was copied last, here or (read from the system clipboard) anywhere.
    pub board: Option<Clipboard>,
    /// This frame's `Event::Paste` text, while the shortcuts run.
    pub paste_text: Option<String>,
    /// A paste waiting for the system clipboard.
    pub menu_paste: Option<PendingPaste>,
    /// Until this input time, a Ctrl-less `Event::Paste` is a given-up request's late
    /// answer ([`LATE_SECS`]).
    pub late_until: Option<f64>,
    /// Where the next paste lands instead of the playhead: the point the empty-timeline
    /// menu was opened on (raw tick, and the track under it). Used once.
    pub anchor: Option<(Ticks, Option<TrackId>)>,
    /// Drawn once per run of the app: with the project's epoch, the [`ClipOrigin`].
    pub session: u64,
}

impl Default for ClipboardState {
    fn default() -> Self {
        ClipboardState {
            board: None,
            paste_text: None,
            menu_paste: None,
            late_until: None,
            anchor: None,
            session: nonce(),
        }
    }
}

/// A number no other run of the app is likely to draw.
fn nonce() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (u64::from(std::process::id())).rotate_left(32)
}

// ---- the text form -------------------------------------------------------------------

/// The clipboard as the text the system clipboard holds: compact JSON, `format` first.
pub fn to_text(board: &Clipboard) -> String {
    serde_json::to_string(board).unwrap_or_default()
}

/// What a piece of system-clipboard text turned out to be.
#[derive(Debug, PartialEq)]
pub enum Decoded {
    Ours(Box<Clipboard>),
    /// Our family, a newer format or project schema: refused with a word, not misread.
    Newer,
    /// Ours, from an older build whose clips this one cannot read: refused with a word.
    Older,
    /// Anything else: text from another program.
    Foreign,
}

/// Read system-clipboard text. Only text that looks like ours is parsed at all.
///
/// The payload is the live model (`Clip`, `Note`, `AudioSource`), so the project schema it
/// was written in decides: a newer one is refused — serde would drop the fields this
/// build does not know and paste something that plays differently — and an older one is
/// read when it still parses (new fields take their defaults), refused otherwise.
pub fn from_text(text: &str) -> Decoded {
    let s = text.trim_start();
    if !s.starts_with('{') || !s.contains(cb::FORMAT_FAMILY) {
        return Decoded::Foreign;
    }
    #[derive(serde::Deserialize)]
    struct Probe {
        format: String,
        #[serde(default)]
        schema: u32,
    }
    let Ok(probe) = serde_json::from_str::<Probe>(s) else {
        return Decoded::Foreign;
    };
    if probe.format == cb::FORMAT {
        if probe.schema > SCHEMA_VERSION {
            return Decoded::Newer;
        }
        match serde_json::from_str::<Clipboard>(s) {
            Ok(b) if b.ppq == PPQ => Decoded::Ours(Box::new(b)),
            Ok(_) => Decoded::Newer,
            // 0: the text does not say (none ever shipped without it): just not ours.
            Err(_) if (1..SCHEMA_VERSION).contains(&probe.schema) => Decoded::Older,
            Err(_) => Decoded::Foreign,
        }
    } else if probe.format.starts_with(cb::FORMAT_FAMILY) {
        Decoded::Newer
    } else {
        Decoded::Foreign
    }
}

// ---- audio files ------------------------------------------------------------------------

/// Whether two paths name the same file (canonically when both exist).
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// How each copied file is found in `project`: the very source when the copy came from
/// this project in this run; a source with the same hash or the same file; otherwise a
/// new source (fresh id, **not** recorded — a save must never move another project's
/// file into this one's folder).
fn plan_sources(
    project: &Project,
    board: &Clipboard,
    same_origin: bool,
    base: Option<&Path>,
    ids: clips::Ids,
) -> HashMap<AudioSourceId, SourcePlan> {
    let mut plans = HashMap::new();
    for s in &board.sources {
        if same_origin && project.audio_source(s.id).is_some() {
            plans.insert(s.id, SourcePlan::Reuse(s.id));
            continue;
        }
        let file = Path::new(&s.path);
        let found = project.audio_pool.iter().find(|p| {
            (p.sha256.is_some() && p.sha256 == s.sha256)
                || same_file(&crate::waveform::resolve(base, &p.path), file)
        });
        let plan = match found {
            Some(p) => SourcePlan::Reuse(p.id),
            None => SourcePlan::Insert(cantus_core::AudioSource {
                id: AudioSourceId(ids()),
                path: crate::audio_files::stored_path(base, file),
                recorded: false,
                ..s.clone()
            }),
        };
        plans.insert(s.id, plan);
    }
    plans
}

/// What an arrangement copy took, so a cut removes the same thing.
enum Copied {
    Clips(Vec<ClipRef>),
    Time(Ticks, Ticks),
}

impl CantusApp {
    /// Which project, in which run, a copy made now comes from.
    pub fn clipboard_origin(&self) -> ClipOrigin {
        ClipOrigin {
            session: self.clipboard.session,
            epoch: self.epoch,
        }
    }

    /// Keep `board`, and put it on the system clipboard.
    fn publish(&mut self, mut board: Clipboard) {
        board.schema = SCHEMA_VERSION;
        if let Some(ctx) = &self.theme_ctx {
            ctx.copy_text(to_text(&board));
        }
        self.clipboard.board = Some(board);
    }

    /// *Copy*: notes in the piano roll, clips or a time range in the arrangement.
    pub fn clipboard_copy(&mut self) {
        match self.focus {
            EditFocus::PianoRoll => {
                self.copy_notes();
            }
            EditFocus::Arrangement => {
                self.copy_arrangement();
            }
            EditFocus::Mixer => {}
        }
    }

    /// *Cut*: a copy, then one undo step that removes what was copied.
    pub fn clipboard_cut(&mut self) {
        match self.focus {
            EditFocus::PianoRoll => {
                if let Some((track, clip, ids)) = self.copy_notes() {
                    self.selected_notes.clear();
                    self.exec(Command::Batch {
                        label_key: "cmd.cut_notes".into(),
                        commands: vec![Command::RemoveNotes { track, clip, ids }],
                    });
                }
            }
            EditFocus::Arrangement => {
                let commands = match self.copy_arrangement() {
                    Some(Copied::Clips(sel)) => {
                        ("cmd.cut_clips", cb::cut_clips(self.project(), &sel))
                    }
                    Some(Copied::Time(from, to)) => (
                        "cmd.cut_time",
                        self.with_ids(|p, ids| {
                            cb::time_crops(p, from, to, ids)
                                .into_iter()
                                .flat_map(|c| c.cut)
                                .collect()
                        }),
                    ),
                    None => return,
                };
                self.selected_clips.clear();
                self.exec_clip_batch(commands.0, commands.1, Vec::new());
            }
            EditFocus::Mixer => {}
        }
    }

    /// The note clip the piano roll is editing, if it is one.
    fn note_clip(&self) -> Option<(TrackId, Clip)> {
        let (t, c) = (self.selected_track?, self.selected_clip?);
        let clip = self.project().track(t)?.clip(c)?;
        (clip.audio.is_none() && clip.automation.is_none()).then(|| (t, clip.clone()))
    }

    /// Copy the selected notes or, with none selected, every note the clip shows (FL).
    /// Returns what was copied, for a cut.
    fn copy_notes(&mut self) -> Option<(TrackId, ClipId, Vec<NoteId>)> {
        let Some((track, clip)) = self.note_clip() else {
            self.set_status(t("clipboard.status.nothing"));
            return None;
        };
        let selected: Vec<Note> = clip
            .notes
            .iter()
            .filter(|n| self.selected_notes.contains(&n.id))
            .cloned()
            .collect();
        let (notes, origin) = if selected.is_empty() {
            let end = clip.offset + clip.period().unwrap_or(clip.length);
            let shown: Vec<Note> = clip
                .notes
                .iter()
                .filter(|n| n.start >= clip.offset && n.start < end)
                .cloned()
                .collect();
            (shown, clip.content_start())
        } else {
            let first = selected
                .iter()
                .map(|n| n.start)
                .min()
                .unwrap_or(Ticks::ZERO);
            (selected, first)
        };
        if notes.is_empty() {
            self.set_status(t("clipboard.status.nothing"));
            return None;
        }
        let ids: Vec<NoteId> = notes.iter().map(|n| n.id).collect();
        let board = Clipboard::new(
            self.clipboard_origin(),
            self.project(),
            cb::copy_notes(&notes, origin),
        );
        self.publish(board);
        let n = ids.len();
        self.set_status(tf("clipboard.status.copied_notes", &[("n", &n)]));
        Some((track, clip.id, ids))
    }

    /// Copy the clip selection, or the time range when it was touched last (or when no
    /// clip is selected).
    fn copy_arrangement(&mut self) -> Option<Copied> {
        let sel = self.clip_selection();
        let range = self
            .time_selection
            .filter(|_| self.last_selection == SelKind::Time || sel.is_empty());
        let (payload, copied, key) = if let Some((from, to)) = range {
            let mut n = u64::MAX / 2;
            let mut ids = || {
                n += 1;
                n
            };
            let crops = cb::time_crops(self.project(), from, to, &mut ids);
            let picked = crops
                .into_iter()
                .flat_map(|c| {
                    let (index, track) = (c.index, c.track);
                    c.pieces.into_iter().map(move |p| (index, track, p))
                })
                .collect();
            (
                cb::clips_payload(picked, from, Some(to - from)),
                Copied::Time(from, to),
                "clipboard.status.copied_time",
            )
        } else {
            (
                cb::copy_clips(self.project(), &sel),
                Copied::Clips(sel),
                "clipboard.status.copied_clips",
            )
        };
        let mut board = Clipboard::new(self.clipboard_origin(), self.project(), payload);
        if board.is_empty() {
            self.set_status(t("clipboard.status.nothing"));
            return None;
        }
        board.sources = board
            .audio_source_ids()
            .into_iter()
            .filter_map(|id| self.project().audio_source(id))
            .map(|s| {
                let file = self.audio_file(s);
                let file = std::path::absolute(&file).unwrap_or(file);
                cantus_core::AudioSource {
                    path: file.to_string_lossy().into_owned(),
                    ..s.clone()
                }
            })
            .collect();
        let n = board.len();
        self.publish(board);
        self.set_status(tf(key, &[("n", &n)]));
        Some(copied)
    }
}

impl CantusApp {
    /// *Paste*. With the text of this frame's `Event::Paste` in hand (Ctrl+V) it pastes
    /// now; from a menu it asks the window for the system clipboard first and pastes when
    /// the answer comes ([`Self::settle_paste`]). Without a window (headless) it pastes
    /// what the app copied last.
    pub fn clipboard_paste(&mut self) {
        if self.clipboard.paste_text.is_none() {
            if let Some(ctx) = self.theme_ctx.clone() {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                self.clipboard.menu_paste = Some(PendingPaste {
                    frame: ctx.cumulative_frame_nr(),
                    time: ctx.input(|i| i.time),
                });
                ctx.request_repaint();
                return;
            }
        }
        self.paste_now();
    }

    /// This frame's `Event::Paste` text, before the shortcuts read the input. It is taken
    /// out of the input (so it fires no binding and reaches no text field) when it answers
    /// a menu paste still waiting, or when it is the late answer to one that gave up: no
    /// modifier held (the hand is on the mouse) within [`LATE_SECS`]. Ctrl+V, Ctrl held, is
    /// always the user asking again.
    pub(crate) fn take_paste_event(&mut self, ctx: &egui::Context) {
        let (now, bare) = ctx.input(|i| (i.time, i.modifiers.is_none()));
        if self.clipboard.late_until.is_some_and(|t| now > t) {
            self.clipboard.late_until = None;
        }
        let text = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| match e {
                egui::Event::Paste(s) => Some(s.clone()),
                _ => None,
            })
        });
        let waiting = self.clipboard.menu_paste.is_some();
        let late = !waiting && bare && self.clipboard.late_until.is_some();
        if text.is_some() && (waiting || late) {
            ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
        }
        if late && text.is_some() {
            self.clipboard.late_until = None;
            self.clipboard.paste_text = None;
        } else {
            self.clipboard.paste_text = text;
        }
    }

    /// A paste waiting on the system clipboard: the answer arrived (`paste_text`), or it
    /// has waited [`ANSWER_FRAMES`] frames and [`ANSWER_SECS`] without one (the clipboard
    /// holds no text) and what the app copied last is pasted. Either way exactly once.
    /// Runs after the shortcuts, which is also where this frame's paste text is let go of.
    pub(crate) fn settle_paste(&mut self, ctx: &egui::Context) {
        if let Some(asked) = self.clipboard.menu_paste {
            if self.clipboard.paste_text.is_some() {
                self.paste_now();
            } else {
                let now = ctx.input(|i| i.time);
                let frames = ctx.cumulative_frame_nr().saturating_sub(asked.frame);
                if frames >= ANSWER_FRAMES && now - asked.time >= ANSWER_SECS {
                    self.clipboard.late_until = Some(now + LATE_SECS);
                    self.paste_now();
                } else {
                    ctx.request_repaint();
                }
            }
        }
        self.clipboard.paste_text = None;
    }

    /// Paste now: the system clipboard's text if it is ours, else what the app copied.
    pub fn paste_now(&mut self) {
        self.clipboard.menu_paste = None;
        let anchor = self.clipboard.anchor.take();
        if let Some(text) = self.clipboard.paste_text.take() {
            match from_text(&text) {
                Decoded::Ours(board) => self.clipboard.board = Some(*board),
                Decoded::Newer => {
                    self.set_error(t("clipboard.status.newer"));
                    return;
                }
                Decoded::Older => {
                    self.set_error(t("clipboard.status.older"));
                    return;
                }
                Decoded::Foreign => {}
            }
        }
        let Some(board) = self.clipboard.board.clone() else {
            self.set_status(t("clipboard.status.empty"));
            return;
        };
        match (self.focus, &board.payload) {
            (EditFocus::PianoRoll, Payload::Notes { notes }) => self.paste_notes(notes),
            (EditFocus::PianoRoll, Payload::Clips { .. }) => {
                self.set_status(t("clipboard.status.clips_in_piano_roll"))
            }
            (EditFocus::Arrangement, _) => self.paste_arrangement(&board, anchor),
            (EditFocus::Mixer, _) => {}
        }
    }

    /// Notes into the clip the piano roll is editing: at the time selection's start or the
    /// playhead when either is inside the clip, else at its left edge; snapped down to the
    /// grid when snap is on. The clip grows to hold them.
    fn paste_notes(&mut self, notes: &[Note]) {
        let Some((track, clip)) = self.note_clip() else {
            self.set_status(t("clipboard.status.no_clip"));
            return;
        };
        let inside = |x: Ticks| x >= clip.start && x < clip.end();
        let playhead = self.engine.position();
        let at = self
            .time_selection
            .map(|s| s.0)
            .filter(|x| inside(*x))
            .or(Some(playhead).filter(|x| inside(*x)))
            .and_then(|x| clip.content_shown_at(x));
        let at = match at {
            Some(c) if self.piano_roll.snap => c
                .snap_floor(self.piano_roll.grid.max(Ticks(1)))
                .max(clip.content_start()),
            Some(c) => c,
            None => clip.content_start(),
        };
        let (commands, made) =
            self.with_ids(|_, ids| cb::paste_notes(track, &clip, notes, at, ids));
        if commands.is_empty() {
            return;
        }
        if self.exec(Command::Batch {
            label_key: "cmd.paste".into(),
            commands,
        }) {
            self.selected_notes = made;
        }
    }

    /// Clips (or notes, as a new clip) into the arrangement: at the menu's point, the time
    /// selection's start or the playhead — snapped down to the grid when snap is on — the
    /// topmost on the menu's track, the selected track or the first.
    fn paste_arrangement(&mut self, board: &Clipboard, anchor: Option<(Ticks, Option<TrackId>)>) {
        let (raw, menu_track) = match anchor {
            Some((at, track)) => (at, track),
            None => (
                self.time_selection
                    .map(|s| s.0)
                    .unwrap_or_else(|| self.engine.position()),
                None,
            ),
        };
        let at = if self.arrangement.snap {
            raw.snap_floor(self.arrangement.grid.max(Ticks(1)))
        } else {
            raw
        }
        .max(Ticks::ZERO);
        let track = menu_track
            .or(self.selected_track)
            .filter(|t| self.project().track(*t).is_some())
            .or_else(|| self.project().tracks.first().map(|t| t.id));
        let Some(track) = track else {
            return;
        };
        match &board.payload {
            Payload::Clips { .. } => {
                let row = self.project().track_index(track).unwrap_or(0);
                let same = board.origin == self.clipboard_origin();
                let base = self.audio_base().map(Path::to_path_buf);
                let out = self.with_ids(|project, ids| {
                    let plans = plan_sources(project, board, same, base.as_deref(), ids);
                    cb::paste_clips(project, board, same, row, at, &plans, ids)
                });
                self.exec_clip_batch("cmd.paste_clips", out.commands, out.made);
                if out.skipped_automation > 0 {
                    let n = out.skipped_automation;
                    self.set_status(tf("clipboard.status.automation_skipped", &[("n", &n)]));
                }
            }
            Payload::Notes { notes } => {
                let bar = self.project().time_signature.ticks_per_bar();
                let name = t("clip.new");
                let clip = self.with_ids(|_, ids| cb::notes_clip(&name, notes, at, bar, ids));
                let made = vec![(track, clip.id)];
                self.exec_clip_batch(
                    "cmd.paste_clips",
                    vec![Command::InsertClip {
                        track,
                        clip: Box::new(clip),
                    }],
                    made,
                );
            }
        }
        self.focus_arrangement();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Clipboard {
        let p = crate::app::starter_project();
        let notes = p.tracks[0].clips[0].notes.clone();
        Clipboard::new(
            ClipOrigin {
                session: 7,
                epoch: 2,
            },
            &p,
            cb::copy_notes(&notes, Ticks::ZERO),
        )
    }

    #[test]
    fn our_text_reads_back_as_what_was_written() {
        let b = board();
        let text = to_text(&b);
        assert!(text.starts_with("{\"format\":\"cantus-clip/1\""));
        assert_eq!(from_text(&text), Decoded::Ours(Box::new(b)));
        // Leading whitespace (some clipboards add it) is fine.
        assert!(matches!(
            from_text(&format!("\n  {text}")),
            Decoded::Ours(_)
        ));
    }

    #[test]
    fn a_newer_version_is_refused_and_foreign_text_is_ignored() {
        let text = to_text(&board());
        let newer = text.replacen("cantus-clip/1", "cantus-clip/2", 1);
        assert_eq!(from_text(&newer), Decoded::Newer);
        let mut other_ppq = board();
        other_ppq.ppq = PPQ * 2;
        assert_eq!(from_text(&to_text(&other_ppq)), Decoded::Newer);
        for foreign in [
            "",
            "hello",
            "{\"format\":\"something-else\"}",
            "{ not json cantus-clip/1",
            "{\"format\":\"cantus-clip/1\",\"payload\":7}",
        ] {
            assert_eq!(from_text(foreign), Decoded::Foreign, "{foreign:?}");
        }
    }

    /// The payload is the live model, so the schema it was written in decides: a newer
    /// one is refused (its unknown fields would be dropped without a word), an older one
    /// is read while it parses and refused with a word when it does not.
    #[test]
    fn the_project_schema_decides_whether_a_clipboard_is_read() {
        let mut b = board();
        b.schema = SCHEMA_VERSION;
        assert_eq!(from_text(&to_text(&b)), Decoded::Ours(Box::new(b.clone())));
        let mut newer = b.clone();
        newer.schema = SCHEMA_VERSION + 1;
        assert_eq!(from_text(&to_text(&newer)), Decoded::Newer);
        let mut older = b.clone();
        older.schema = SCHEMA_VERSION - 1;
        assert!(matches!(from_text(&to_text(&older)), Decoded::Ours(_)));
        // An older build's clip that no longer parses (a field since made required).
        let broken = to_text(&older).replace("\"payload\":", "\"payload_was\":");
        assert_eq!(from_text(&broken), Decoded::Older);
        // The same breakage at today's schema is not ours to explain: ignored.
        let broken_now = to_text(&b).replace("\"payload\":", "\"payload_was\":");
        assert_eq!(from_text(&broken_now), Decoded::Foreign);
    }

    #[test]
    fn a_session_number_is_drawn() {
        assert_ne!(ClipboardState::default().session, 0);
    }
}

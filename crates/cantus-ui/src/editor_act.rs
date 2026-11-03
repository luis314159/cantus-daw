//! What the audio editor's window asks (ADR 0096): opening and closing, the edits, the
//! clipboard, the audition, the file's marks, files in and out, and the keys it keeps.

use super::jobs::Job;
use super::{commit, dispatch, load, say, Session, View, MAX_SECONDS};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::audio_edit::{Edit, Sel};
use cantus_core::{ClipId, Command, SourceMarker, SourceMarks, SourceRegion, TrackId};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// What the window asks of the application.
#[derive(Debug)]
pub enum Act {
    Close,
    Edit(Edit),
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Play,
    Stop,
    Learn,
    AddMarker,
    AddRegion,
    SetLoop,
    ClearMarks,
    FitAll,
    FitSelection,
    /// *Open file…*: a file onto a new track, opened here.
    OpenFile,
    /// *Paste from file…*: a file at the cursor, conformed to the take.
    PasteFile,
    /// The selection as a clip of its own, on a new track under it.
    ToNewClip,
    /// *Remove unused audio…*: the dialog.
    Cleanup,
}

/// *Edit in audio editor*: open the clip's take.
pub fn open(app: &mut CantusApp, track: TrackId, clip: ClipId) {
    let Some(c) = app.project().track(track).and_then(|t| t.clip(clip)) else {
        return;
    };
    let Some(a) = c.audio.as_ref() else {
        return;
    };
    // A clip an ARA plug-in edits plays its rendering, not the file (ADR 0097).
    if a.ara.is_some() {
        app.set_error(t("editor.ara_bound"));
        return;
    }
    let name = c.name.clone();
    let source = a.source;
    app.select_clip(track, clip);
    let secs = app
        .project()
        .audio_source(source)
        .map_or(0.0, |s| s.frames as f64 / s.sample_rate.max(1) as f64);
    // Half-written files a cut session left: no edit is being written now.
    if !app.editor.busy() {
        commit::tidy(app);
    }
    let gen = app.editor.next_gen();
    app.editor.session = Some(Session {
        track,
        clip,
        source,
        name,
        epoch: app.epoch,
        buf: None,
        peaks: None,
        spec: None,
        detail: None,
        detail_want: None,
        detail_asked: None,
        sel: None,
        band: None,
        marks: app
            .project()
            .source_marks
            .get(&source)
            .cloned()
            .unwrap_or_default(),
        view: View {
            start: 0.0,
            per: 1.0,
        },
        fit: true,
        profile: None,
        play: None,
        busy: None,
        status: None,
        dragged_out: false,
        gen,
    });
    if secs > MAX_SECONDS {
        let min = format!("{:.0}", MAX_SECONDS / 60.0);
        say(app, tf("editor.too_long", &[("min", &min)]), true);
        return;
    }
    load(app);
}

/// Leave the editor; the clip plays what it was left as. A result still on its way is
/// dropped (and its file with it, when no one else has it).
pub fn close(app: &mut CantusApp) {
    stop(app);
    app.editor.session = None;
    app.editor.next_gen();
}

/// The selection an edit works on: the one drawn, or the whole take for edits that make
/// sense on it.
fn selection_for(s: &Session, edit: &Edit, frames: usize) -> Result<Sel, String> {
    let sel = s.sel.map(|x| x.clamp(frames)).filter(|x| !x.is_empty());
    let whole_ok = !matches!(
        edit,
        Edit::Delete | Edit::Trim | Edit::Silence | Edit::Attenuate { .. } | Edit::Repair
    );
    match sel {
        Some(x) => Ok(x),
        None if whole_ok => Ok(Sel::whole(frames)),
        None => Err(t("editor.need_selection")),
    }
}

/// Whether the thread is busy — and if so, says so. Every act that would start work or
/// touch the clipboard asks first.
fn held(app: &mut CantusApp) -> bool {
    if app.editor.busy() {
        say(app, t("editor.busy_wait"), true);
        return true;
    }
    // The recording mode refuses edits that change the sound (ADR 0092): asked before the
    // work starts, said in the mode's own words.
    if let Some(probe) = probe(app) {
        if app.bus.locked_against(&probe) {
            let key = crate::record_mode::notice_key(&probe);
            say(app, t(key), true);
            app.record_mode_notice_for(key);
            return true;
        }
    }
    false
}

/// The command an edit of the open take ends in, as the project is now (what a lock is asked
/// about before any work is done).
fn probe(app: &CantusApp) -> Option<Command> {
    let s = app.editor.session.as_ref()?;
    let audio = app.project().track(s.track)?.clip(s.clip)?.audio.clone()?;
    Some(Command::SetClipAudio {
        track: s.track,
        clip: s.clip,
        audio: Some(audio),
    })
}

/// Run an edit on the session's take (on the thread; `frame` takes the result).
pub fn run_edit(app: &mut CantusApp, edit: Edit) {
    if held(app) {
        return;
    }
    let Some(s) = app.editor.session.as_ref() else {
        return;
    };
    let Some(buf) = s.buf.clone() else {
        return;
    };
    let mut edit = edit;
    // A paste replaces the selection, or goes in at the cursor (the take's start without
    // one).
    let sel = match &edit {
        Edit::Paste { .. } => Ok(s.sel.map_or(Sel::new(0, 0), |x| x.clamp(buf.frames()))),
        Edit::Attenuate { .. } if s.band.is_none() => Err(t("editor.need_region")),
        _ => selection_for(s, &edit, buf.frames()),
    };
    let sel = match sel {
        Ok(x) => x,
        Err(m) => return say(app, m, true),
    };
    if let (Edit::Attenuate { lo_hz, hi_hz, .. }, Some(b)) = (&mut edit, s.band) {
        *lo_hz = b.lo_hz;
        *hi_hz = b.hi_hz;
    }
    if let (Edit::Denoise { profile, .. }, Some(p)) = (&mut edit, s.profile.as_ref()) {
        *profile = p.clone();
    } else if matches!(edit, Edit::Denoise { .. }) {
        return say(app, t("editor.need_profile"), true);
    }
    let joins = app.editor.tools.joins();
    let dir = commit::edit_dir(app);
    let busy = match edit {
        Edit::Denoise { .. } => "editor.busy.denoise",
        Edit::RemoveSilence(_) => "editor.busy.silence",
        _ => "editor.busy.edit",
    };
    dispatch(
        app,
        Some(busy),
        Job::Apply {
            buf,
            sel,
            edit: Box::new(edit),
            joins,
            dir,
        },
    );
}

/// Audition the selection (or the whole take).
pub fn play(app: &mut CantusApp) {
    let Some(s) = app.editor.session.as_ref() else {
        return;
    };
    let Some(buf) = s.buf.clone() else {
        return;
    };
    let sel = s
        .sel
        .map(|x| x.clamp(buf.frames()))
        .filter(|x| !x.is_empty())
        .unwrap_or(Sel::whole(buf.frames()));
    let part = buf.slice(sel);
    let data = cantus_core::SampleData {
        rate: part.rate,
        channels: part.channels.min(2),
        frames: part.data.into(),
        stream: None,
    };
    app.engine
        .preview(Some((Arc::new(data), crate::samples::AUDITION_GAIN)));
    if let Some(s) = app.editor.session.as_mut() {
        s.play = Some(Play {
            at: Instant::now(),
            from: sel.start,
            to: sel.end,
        });
    }
}

use super::Play;

pub fn stop(app: &mut CantusApp) {
    if app
        .editor
        .session
        .as_mut()
        .is_some_and(|s| s.play.take().is_some())
    {
        app.engine.preview(None);
    }
}

/// The keymap's edits while the window is open: it keeps Cut, Copy, Paste, Delete, Select
/// all, Escape, Space and Stop (so the arrangement behind it does not take them). Returns
/// whether it did.
pub fn action(app: &mut CantusApp, action: crate::keymap::Action) -> bool {
    use crate::keymap::Action;
    if !app.editor.is_open() {
        return false;
    }
    match action {
        Action::Cut => act(app, Act::Cut),
        Action::Copy => act(app, Act::Copy),
        Action::Paste => act(app, Act::Paste),
        Action::Delete => act(app, Act::Edit(Edit::Delete)),
        Action::SelectAll => {
            if let Some(s) = app.editor.session.as_mut() {
                if let Some(b) = s.buf.as_ref() {
                    s.sel = Some(Sel::whole(b.frames()));
                    s.band = None;
                }
            }
        }
        Action::ClearSelection => {
            if let Some(s) = app.editor.session.as_mut() {
                s.sel = None;
                s.band = None;
            }
        }
        Action::PlayPause => {
            let playing = app
                .editor
                .session
                .as_ref()
                .is_some_and(|s| s.play.is_some());
            act(app, if playing { Act::Stop } else { Act::Play });
        }
        Action::Stop => act(app, Act::Stop),
        _ => return false,
    }
    true
}

/// Change the open file's marks: one undo step.
fn set_marks(app: &mut CantusApp, f: impl FnOnce(&mut SourceMarks, Sel)) {
    let Some(s) = app.editor.session.as_ref() else {
        return;
    };
    let frames = s.buf.as_ref().map_or(0, |b| b.frames());
    let sel = s.sel.unwrap_or(Sel::new(
        s.view.start.max(0.0) as usize,
        s.view.start.max(0.0) as usize,
    ));
    // The project's marks, not the window's mirror (which catches up next frame).
    let mut marks = app
        .project()
        .source_marks
        .get(&s.source)
        .cloned()
        .unwrap_or_default();
    f(&mut marks, sel.clamp(frames));
    marks.markers.sort_by_key(|m| m.frame);
    marks.markers.dedup_by_key(|m| m.frame);
    let id = s.source;
    let cmd = Command::SetAudioSourceMarks {
        id,
        marks: (!marks.is_empty()).then(|| Box::new(marks)),
    };
    if !app.exec(cmd) {
        say(app, t("editor.refused"), true);
    }
}

/// Do what the window asked.
pub fn act(app: &mut CantusApp, a: Act) {
    match a {
        Act::Close => close(app),
        Act::Edit(e) => run_edit(app, e),
        Act::Copy | Act::Cut => {
            if matches!(a, Act::Cut) && held(app) {
                return;
            }
            let clip = app.editor.session.as_ref().and_then(|s| {
                let b = s.buf.as_ref()?;
                let sel = s.sel.filter(|x| !x.is_empty())?;
                Some(b.slice(sel))
            });
            match clip {
                Some(c) => {
                    app.editor.clipboard = Some(c);
                    if matches!(a, Act::Cut) {
                        run_edit(app, Edit::Delete);
                    } else {
                        say(app, t("editor.copied"), false);
                    }
                }
                None => say(app, t("editor.need_selection"), true),
            }
        }
        Act::Paste => match app.editor.clipboard.clone() {
            Some(clip) => run_edit(app, Edit::Paste { clip }),
            None => say(app, t("editor.clipboard_empty"), true),
        },
        Act::Undo => app.undo(),
        Act::Redo => app.redo(),
        Act::Play => play(app),
        Act::Stop => stop(app),
        Act::Learn => {
            // Learning changes nothing: only the thread being busy holds it.
            if app.editor.busy() {
                return say(app, t("editor.busy_wait"), true);
            }
            let Some(s) = app.editor.session.as_ref() else {
                return;
            };
            let (Some(buf), Some(sel)) = (s.buf.clone(), s.sel.filter(|x| !x.is_empty())) else {
                return say(app, t("editor.need_selection"), true);
            };
            dispatch(app, Some("editor.busy.learn"), Job::Learn { buf, sel });
        }
        Act::AddMarker => set_marks(app, |m, sel| {
            let n = m.markers.len() + 1;
            m.markers.push(SourceMarker {
                frame: sel.start as u64,
                name: tf("editor.marker_name", &[("n", &n.to_string())]),
            });
        }),
        Act::AddRegion => {
            if app
                .editor
                .session
                .as_ref()
                .is_some_and(|s| s.sel.is_none_or(|x| x.is_empty()))
            {
                return say(app, t("editor.need_selection"), true);
            }
            set_marks(app, |m, sel| {
                let n = m.regions.len() + 1;
                m.regions.push(SourceRegion {
                    start: sel.start as u64,
                    end: sel.end as u64,
                    name: tf("editor.region_name", &[("n", &n.to_string())]),
                });
            })
        }
        Act::SetLoop => {
            if app
                .editor
                .session
                .as_ref()
                .is_some_and(|s| s.sel.is_none_or(|x| x.is_empty()))
            {
                return say(app, t("editor.need_selection"), true);
            }
            set_marks(app, |m, sel| {
                m.loop_points = Some((sel.start as u64, sel.end as u64));
            })
        }
        Act::ClearMarks => set_marks(app, |m, _| *m = SourceMarks::default()),
        Act::FitAll => {
            if let Some(s) = app.editor.session.as_mut() {
                s.fit = true;
            }
        }
        Act::FitSelection => {
            if let Some(s) = app.editor.session.as_mut() {
                if let Some(sel) = s.sel.filter(|x| !x.is_empty()) {
                    // `per` 0 asks the view to fit the selection's span in its width.
                    s.view = View {
                        start: sel.start as f64 - sel.len() as f64 * 0.05,
                        per: 0.0,
                    };
                    s.fit = false;
                }
            }
        }
        Act::OpenFile => {
            let pick = rfd::FileDialog::new()
                .add_filter("Audio", &["wav", "flac", "mp3", "ogg", "aif", "aiff"])
                .pick_file();
            if let Some(p) = pick {
                open_file(app, &p);
            }
        }
        Act::PasteFile => {
            let pick = rfd::FileDialog::new()
                .add_filter("Audio", &["wav", "flac", "mp3", "ogg", "aif", "aiff"])
                .pick_file();
            if let Some(p) = pick {
                paste_file(app, &p);
            }
        }
        Act::ToNewClip => to_new_clip(app),
        Act::Cleanup => commit::open_cleanup(app),
    }
}

/// *Open file…*: `path` onto a new track at the playhead, and that clip opened here.
pub fn open_file(app: &mut CantusApp, path: &Path) {
    let before: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    let at = app.engine.position();
    if app.import_audio(&[path.to_path_buf()], at, None) == 0 {
        return;
    }
    let made = app
        .project()
        .tracks
        .iter()
        .find(|t| !before.contains(&t.id))
        .and_then(|t| t.clips.first().map(|c| (t.id, c.id)));
    if let Some((t, c)) = made {
        close(app);
        open(app, t, c);
    }
}

/// *Paste from file…*: `path` read, conformed to the take (rate, channels) and pasted at
/// the selection — one undo step, like any paste.
pub fn paste_file(app: &mut CantusApp, path: &Path) {
    if held(app) {
        return;
    }
    let Some(s) = app.editor.session.as_ref() else {
        return;
    };
    let Some(buf) = s.buf.clone() else {
        return;
    };
    let sel = s.sel.map_or(Sel::new(0, 0), |x| x.clamp(buf.frames()));
    let joins = app.editor.tools.joins();
    let dir = commit::edit_dir(app);
    dispatch(
        app,
        Some("editor.busy.edit"),
        Job::PasteFile {
            buf,
            sel,
            path: path.to_path_buf(),
            joins,
            dir,
        },
    );
}

/// The selection as a clip of its own: written as a file (named by its content), then a new
/// track under the clip's with it where the selection plays now (`commit::place_new_clip`).
pub fn to_new_clip(app: &mut CantusApp) {
    if held(app) {
        return;
    }
    let Some(s) = app.editor.session.as_ref() else {
        return;
    };
    let (Some(buf), Some(sel)) = (s.buf.clone(), s.sel.filter(|x| !x.is_empty())) else {
        return say(app, t("editor.need_selection"), true);
    };
    let dir = commit::edit_dir(app);
    dispatch(
        app,
        Some("editor.busy.edit"),
        Job::Export {
            part: buf.slice(sel),
            dir,
            from: sel.start,
        },
    );
}

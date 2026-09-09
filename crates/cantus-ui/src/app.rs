//! The application: owns the command bus, the engine handle, selection and view state.
//! Every mutation goes through [`CantusApp::exec`] (ADR 0004); the engine only ever sees
//! compiled snapshots.

use crate::design::{self, space, Elevation};
use crate::i18n::{t, tf};
use crate::keyboard::{KeyEvent, KeyboardPiano};
use crate::layout::{self, Layouts};
use crate::recording::{RecordedNote, Recorder};
use crate::theme;
use crate::views::{
    self, arrangement::ArrangementView, mixer::MixerView, piano_roll::PianoRollView,
};
use cantus_core::{
    ClipId, Command, CommandBus, EffectId, EffectKind, MergeKey, Origin, Project, Snapshot, Ticks,
    TrackId,
};
use cantus_engine::{AudioConfig, EngineHandle, MidiInputs, NoteInput, RenderOptions, Rendered};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

/// What the central area shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CentralView {
    PianoRoll,
    Mixer,
}

impl From<layout::Central> for CentralView {
    fn from(c: layout::Central) -> Self {
        match c {
            layout::Central::PianoRoll => CentralView::PianoRoll,
            layout::Central::Mixer => CentralView::Mixer,
        }
    }
}

impl From<CentralView> for layout::Central {
    fn from(c: CentralView) -> Self {
        match c {
            CentralView::PianoRoll => layout::Central::PianoRoll,
            CentralView::Mixer => layout::Central::Mixer,
        }
    }
}

#[derive(Debug)]
pub struct Status {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

pub struct CantusApp {
    pub bus: CommandBus,
    pub engine: EngineHandle,
    pub path: Option<PathBuf>,
    pub selected_track: Option<TrackId>,
    pub selected_clip: Option<ClipId>,
    pub selected_notes: Vec<cantus_core::NoteId>,
    pub arrangement: ArrangementView,
    pub piano_roll: PianoRollView,
    pub mixer: MixerView,
    pub central: CentralView,
    pub recorder: Recorder,
    pub midi_port_names: Vec<String>,
    midi_inputs: Option<MidiInputs>,
    live_track_sent: Option<Option<TrackId>>,
    theme_watcher: Option<theme::ThemeWatcher>,
    theme_path: PathBuf,
    /// Named window layouts, and the one in use (`crate::layout`).
    pub layouts: Layouts,
    /// Where [`Self::store_layouts`] writes. Public so tests can redirect it away from the
    /// user's real configuration directory.
    pub layouts_path: PathBuf,
    /// Open when the user is naming a layout; holds the text being typed.
    pub naming_layout: Option<String>,
    theme_ctx: Option<egui::Context>,
    pub piano_roll_notes_drawn: usize,
    pub keyboard: KeyboardPiano,
    /// Typing keyboard plays the selected track (Ctrl+T toggles; off frees more keys).
    pub keyboard_piano: bool,
    /// Note clipboard, relative to the earliest copied note.
    pub clipboard: Vec<cantus_core::Note>,
    pub status: Status,
    pub show_stats: bool,
    pub show_about: bool,
    pub show_shortcuts: bool,
    pub request_quit: bool,
    synced_revision: u64,
    last_autosave: Instant,
    autosaved_revision: u64,
    autosave_dir: PathBuf,
    export: Option<mpsc::Receiver<Result<(PathBuf, Rendered), String>>>,
    auditions: Vec<(TrackId, u8, Instant)>,
    frame_times: Vec<f32>,
}

impl CantusApp {
    pub fn new(cc: &eframe::CreationContext<'_>, project: Option<Project>, stress: bool) -> Self {
        // The theme lives in a user-editable file; create it from the default on first run.
        let theme_path = directories_fallback().join("theme.json");
        let loaded = match theme::load_file(&theme_path) {
            Ok(mut t) => {
                // A file from an older release is missing whatever tokens have been added
                // since. Write them back so they are there to be edited.
                if t.fill_missing() {
                    if let Err(e) = theme::save_file(&t, &theme_path) {
                        log::warn!("cannot upgrade theme file: {e}");
                    }
                }
                t
            }
            Err(_) => {
                let t = theme::ink_and_gold();
                if let Err(e) = theme::save_file(&t, &theme_path) {
                    log::warn!("cannot write theme file: {e}");
                }
                t
            }
        };
        design::install_fonts(&cc.egui_ctx);
        design::install_style(&cc.egui_ctx);
        let layouts_path = layout::layouts_path(&directories_fallback());
        let mut layouts = Layouts::load(&layouts_path);
        layouts.force();
        theme::apply(&loaded, &cc.egui_ctx);
        let engine = match EngineHandle::start(&AudioConfig::default()) {
            Ok(e) => e,
            Err(e) => {
                log::error!("audio: {e}");
                EngineHandle::dummy()
            }
        };
        let mut app = Self::from_parts(project, engine, stress);
        app.central = layouts.current.central.into();
        app.layouts = layouts;
        app.layouts_path = layouts_path;
        app.theme_watcher = Some(theme::ThemeWatcher::new(theme_path.clone()));
        app.theme_path = theme_path;
        app.theme_ctx = Some(cc.egui_ctx.clone());
        if app.engine.is_live() {
            let inputs = MidiInputs::open_all(app.engine.midi_sender());
            app.midi_port_names = inputs.names.clone();
            app.midi_inputs = Some(inputs);
        }
        app
    }

    /// Build the application around an existing engine handle (headless tests use
    /// `EngineHandle::dummy()`).
    pub fn from_parts(project: Option<Project>, engine: EngineHandle, stress: bool) -> Self {
        let audio_error = if engine.is_live() {
            None
        } else {
            Some("no device".to_string())
        };
        let autosave_dir = directories_fallback().join("autosave");
        let project = project.unwrap_or_else(|| {
            if stress {
                stress_project()
            } else {
                starter_project()
            }
        });
        let mut app = CantusApp {
            bus: CommandBus::new(project),
            engine,
            path: None,
            selected_track: None,
            selected_clip: None,
            selected_notes: Vec::new(),
            arrangement: ArrangementView::default(),
            piano_roll: PianoRollView::default(),
            mixer: MixerView::default(),
            central: CentralView::PianoRoll,
            recorder: Recorder::new(),
            midi_port_names: Vec::new(),
            midi_inputs: None,
            live_track_sent: None,
            theme_watcher: None,
            theme_path: directories_fallback().join("theme.json"),
            theme_ctx: None,
            layouts: Layouts::default(),
            layouts_path: layout::layouts_path(&directories_fallback()),
            naming_layout: None,
            piano_roll_notes_drawn: 0,
            keyboard: KeyboardPiano::default(),
            keyboard_piano: false,
            clipboard: Vec::new(),
            status: Status {
                text: t("status.ready"),
                error: false,
                at: Instant::now(),
            },
            show_stats: stress,
            show_about: false,
            show_shortcuts: false,
            request_quit: false,
            synced_revision: u64::MAX,
            last_autosave: Instant::now(),
            autosaved_revision: 0,
            autosave_dir,
            export: None,
            auditions: Vec::new(),
            frame_times: Vec::with_capacity(120),
        };
        app.select_first();
        app.sync_live_track();
        if let Some(e) = audio_error {
            app.set_error(tf("status.audio_failed", &[("error", &e)]));
        }
        app.sync_engine();
        app
    }

    // ---- project access -------------------------------------------------------------

    pub fn project(&self) -> &Project {
        self.bus.project()
    }

    /// Execute a command from the user. Returns `true` on success.
    pub fn exec(&mut self, cmd: Command) -> bool {
        match self.bus.execute(cmd, Origin::User) {
            Ok(()) => true,
            Err(e) => {
                self.set_error(e.to_string());
                false
            }
        }
    }

    pub fn exec_merging(&mut self, cmd: Command, key: MergeKey) -> bool {
        match self.bus.execute_merging(cmd, Origin::User, key) {
            Ok(()) => true,
            Err(e) => {
                self.set_error(e.to_string());
                false
            }
        }
    }

    pub fn end_gesture(&mut self) {
        self.bus.end_gesture();
    }

    pub fn undo(&mut self) {
        self.bus.undo();
        self.validate_selection();
    }

    pub fn redo(&mut self) {
        self.bus.redo();
        self.validate_selection();
    }

    fn validate_selection(&mut self) {
        let p = self.bus.project();
        if let Some(t) = self.selected_track {
            if p.track(t).is_none() {
                self.selected_track = None;
                self.selected_clip = None;
            }
        }
        if let (Some(t), Some(c)) = (self.selected_track, self.selected_clip) {
            match p.track(t).and_then(|t| t.clip(c)) {
                Some(clip) => self.selected_notes.retain(|n| clip.note(*n).is_some()),
                None => {
                    self.selected_clip = None;
                    self.selected_notes.clear();
                }
            }
        }
        if self.selected_track.is_none() {
            self.select_first();
        }
    }

    fn select_first(&mut self) {
        if let Some(t) = self.bus.project().tracks.first() {
            self.selected_track = Some(t.id);
            self.selected_clip = t.clips.first().map(|c| c.id);
        }
    }

    /// Fold the live panel sizes and the current view into the layout, then write the
    /// layouts file. Called whenever a layout changes and once on exit, so what is on disk
    /// is what the user was last looking at.
    pub fn store_layouts(&mut self, ctx: &egui::Context) {
        self.layouts.sync(ctx);
        self.layouts.current.central = self.central.into();
        if let Err(e) = self.layouts.save(&self.layouts_path) {
            log::warn!("cannot write layouts file: {e}");
        }
    }

    pub fn save_layout_as(&mut self, name: &str, ctx: &egui::Context) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.layouts.current.central = self.central.into();
        self.layouts.save_as(name, ctx);
        self.store_layouts(ctx);
        self.set_status(tf("layout.saved", &[("name", &name)]));
    }

    pub fn apply_layout(&mut self, name: &str) {
        if !self.layouts.apply_named(name) {
            return;
        }
        self.central = self.layouts.current.central.into();
        let label = self.layouts.current.display_name();
        // `store` would sync the live sizes back over the ones we just applied, so the
        // file is written from the layout itself.
        if let Err(e) = self.layouts.save(&self.layouts_path) {
            log::warn!("cannot write layouts file: {e}");
        }
        self.set_status(tf("layout.applied", &[("name", &label)]));
    }

    pub fn delete_layout(&mut self, name: &str, ctx: &egui::Context) {
        self.layouts.delete(name);
        self.store_layouts(ctx);
    }

    pub fn reset_layout(&mut self) {
        self.layouts.reset();
        self.central = self.layouts.current.central.into();
        if let Err(e) = self.layouts.save(&self.layouts_path) {
            log::warn!("cannot write layouts file: {e}");
        }
        self.set_status(t("layout.was_reset"));
    }

    pub fn set_status(&mut self, text: String) {
        self.status = Status {
            text,
            error: false,
            at: Instant::now(),
        };
    }

    pub fn set_error(&mut self, text: String) {
        log::warn!("{text}");
        self.status = Status {
            text,
            error: true,
            at: Instant::now(),
        };
    }

    // ---- selection ------------------------------------------------------------------

    pub fn select_track(&mut self, id: TrackId) {
        if self.selected_track != Some(id) {
            for ev in self.keyboard.release_all() {
                self.apply_key_event(ev);
            }
            self.selected_track = Some(id);
            self.selected_clip = self
                .project()
                .track(id)
                .and_then(|t| t.clips.first().map(|c| c.id));
            self.selected_notes.clear();
        }
    }

    pub fn select_clip(&mut self, track: TrackId, clip: ClipId) {
        self.select_track(track);
        if self.selected_clip != Some(clip) {
            self.selected_clip = Some(clip);
            self.selected_notes.clear();
        }
        if let Some(c) = self.project().track(track).and_then(|t| t.clip(clip)) {
            // Centre the view on the clip's notes.
            if let Some((lo, hi)) = c.notes.iter().fold(None, |acc: Option<(u8, u8)>, n| {
                Some(acc.map_or((n.pitch, n.pitch), |(l, h)| {
                    (l.min(n.pitch), h.max(n.pitch))
                }))
            }) {
                self.piano_roll.top_pitch =
                    ((lo as f32 + hi as f32) / 2.0 + 14.0).clamp(24.0, 127.0);
            }
            self.piano_roll.scroll_beats = 0.0;
        }
    }

    pub fn select_all_notes(&mut self) {
        if let (Some(t), Some(c)) = (self.selected_track, self.selected_clip) {
            if let Some(clip) = self.project().track(t).and_then(|t| t.clip(c)) {
                self.selected_notes = clip.notes.iter().map(|n| n.id).collect();
            }
        }
    }

    pub fn delete_selection(&mut self) {
        if let (Some(t), Some(c)) = (self.selected_track, self.selected_clip) {
            if !self.selected_notes.is_empty() {
                let ids = std::mem::take(&mut self.selected_notes);
                self.exec(Command::RemoveNotes {
                    track: t,
                    clip: c,
                    ids,
                });
            }
        }
    }

    pub fn quantize_selection(&mut self) {
        let (Some(t), Some(c)) = (self.selected_track, self.selected_clip) else {
            return;
        };
        let Some(clip) = self.project().track(t).and_then(|t| t.clip(c)) else {
            return;
        };
        let grid = self.piano_roll.grid;
        let targets: Vec<cantus_core::NoteId> = if self.selected_notes.is_empty() {
            clip.notes.iter().map(|n| n.id).collect()
        } else {
            self.selected_notes.clone()
        };
        let notes: Vec<_> = clip
            .notes
            .iter()
            .filter(|n| targets.contains(&n.id))
            .map(|n| cantus_core::Note {
                start: n.start.snap(grid),
                ..n.clone()
            })
            .filter(|n| clip.note(n.id).map(|o| o.start != n.start).unwrap_or(false))
            .collect();
        if !notes.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.quantize".into(),
                commands: vec![Command::SetNotes {
                    track: t,
                    clip: c,
                    notes,
                }],
            });
        }
    }

    // ---- tracks & clips -------------------------------------------------------------

    pub fn add_track(&mut self) {
        let n = self.project().tracks.len() + 1;
        let track = self.bus.new_track(tf("track.new_name", &[("n", &n)]));
        let id = track.id;
        let index = self.project().tracks.len();
        if self.exec(Command::InsertTrack {
            index,
            track: Box::new(track),
        }) {
            self.select_track(id);
        }
    }

    pub fn delete_track(&mut self, id: TrackId) {
        if self.exec(Command::RemoveTrack { id }) {
            self.validate_selection();
        }
    }

    pub fn create_clip(&mut self, track: TrackId, start: Ticks, length: Ticks) {
        let clip = self.bus.new_clip(t("clip.new"), start, length);
        let id = clip.id;
        if self.exec(Command::InsertClip {
            track,
            clip: Box::new(clip),
        }) {
            self.select_clip(track, id);
        }
    }

    pub fn delete_clip(&mut self, track: TrackId, clip: ClipId) {
        if self.exec(Command::RemoveClip { track, clip }) {
            self.validate_selection();
        }
    }

    // ---- note editing (shared by tools, menus and shortcuts) ------------------------

    fn selected_clip_notes(&self) -> Option<(TrackId, ClipId, Vec<cantus_core::Note>)> {
        let (t, c) = (self.selected_track?, self.selected_clip?);
        let clip = self.project().track(t)?.clip(c)?;
        Some((t, c, clip.notes.clone()))
    }

    /// Notes to operate on: the selection, or nothing.
    fn selected_note_values(&self) -> Option<(TrackId, ClipId, Vec<cantus_core::Note>)> {
        let (t, c, notes) = self.selected_clip_notes()?;
        let sel: Vec<_> = notes
            .into_iter()
            .filter(|n| self.selected_notes.contains(&n.id))
            .collect();
        if sel.is_empty() {
            None
        } else {
            Some((t, c, sel))
        }
    }

    /// Remove notes; with a merge key consecutive erases form one undo step (eraser drag).
    pub fn erase_notes(&mut self, ids: Vec<cantus_core::NoteId>, merge: Option<MergeKey>) {
        let (Some(t), Some(c)) = (self.selected_track, self.selected_clip) else {
            return;
        };
        let ids: Vec<_> = {
            let Some(clip) = self.project().track(t).and_then(|tr| tr.clip(c)) else {
                return;
            };
            ids.into_iter()
                .filter(|id| clip.note(*id).is_some())
                .collect()
        };
        if ids.is_empty() {
            return;
        }
        self.selected_notes.retain(|n| !ids.contains(n));
        let cmd = Command::Batch {
            label_key: "cmd.erase".into(),
            commands: vec![Command::RemoveNotes {
                track: t,
                clip: c,
                ids,
            }],
        };
        match merge {
            Some(key) => {
                self.exec_merging(cmd, key);
            }
            None => {
                self.exec(cmd);
            }
        }
    }

    pub fn toggle_mute_notes(&mut self, ids: Vec<cantus_core::NoteId>) {
        let Some((t, c, notes)) = self.selected_clip_notes() else {
            return;
        };
        let targets: Vec<_> = notes.into_iter().filter(|n| ids.contains(&n.id)).collect();
        if targets.is_empty() {
            return;
        }
        // If any is unmuted, mute all; otherwise unmute all.
        let mute = targets.iter().any(|n| !n.muted);
        let notes = targets
            .into_iter()
            .map(|n| cantus_core::Note { muted: mute, ..n })
            .collect();
        self.exec(Command::Batch {
            label_key: "cmd.mute_notes".into(),
            commands: vec![Command::SetNotes {
                track: t,
                clip: c,
                notes,
            }],
        });
    }

    /// Split every note that spans `tick` (clip-relative) into two. Returns how many.
    pub fn slice_at(&mut self, tick: Ticks) -> usize {
        let Some((t, c, notes)) = self.selected_clip_notes() else {
            return 0;
        };
        let only_selected = !self.selected_notes.is_empty()
            && notes
                .iter()
                .any(|n| self.selected_notes.contains(&n.id) && n.start < tick && n.end() > tick);
        let mut shortened = Vec::new();
        let mut tails = Vec::new();
        for n in notes {
            if n.start < tick
                && n.end() > tick
                && (!only_selected || self.selected_notes.contains(&n.id))
            {
                let tail = self.bus.new_note(tick, n.end() - tick, n.pitch, n.velocity);
                tails.push(cantus_core::Note {
                    muted: n.muted,
                    ..tail
                });
                shortened.push(cantus_core::Note {
                    length: tick - n.start,
                    ..n
                });
            }
        }
        let count = tails.len();
        if count == 0 {
            return 0;
        }
        self.exec(Command::Batch {
            label_key: "cmd.slice".into(),
            commands: vec![
                Command::SetNotes {
                    track: t,
                    clip: c,
                    notes: shortened,
                },
                Command::InsertNotes {
                    track: t,
                    clip: c,
                    notes: tails,
                },
            ],
        });
        count
    }

    /// Move the selection in time (negative = earlier). Never before the clip start.
    pub fn nudge_selection(&mut self, delta: Ticks) {
        let Some((t, c, sel)) = self.selected_note_values() else {
            return;
        };
        let min_start = sel.iter().map(|n| n.start).min().unwrap_or(Ticks::ZERO);
        let delta = delta.max(-min_start);
        if delta == Ticks::ZERO {
            return;
        }
        let notes = sel
            .into_iter()
            .map(|n| cantus_core::Note {
                start: n.start + delta,
                ..n
            })
            .collect();
        self.exec(Command::Batch {
            label_key: "cmd.nudge".into(),
            commands: vec![Command::SetNotes {
                track: t,
                clip: c,
                notes,
            }],
        });
    }

    pub fn transpose_selection(&mut self, semitones: i32) {
        let Some((t, c, sel)) = self.selected_note_values() else {
            return;
        };
        if sel
            .iter()
            .any(|n| !(0..=127).contains(&(n.pitch as i32 + semitones)))
        {
            return;
        }
        let notes: Vec<_> = sel
            .into_iter()
            .map(|n| cantus_core::Note {
                pitch: (n.pitch as i32 + semitones) as u8,
                ..n
            })
            .collect();
        if let Some(first) = notes.first() {
            self.audition(t, first.pitch);
        }
        self.exec(Command::Batch {
            label_key: "cmd.transpose".into(),
            commands: vec![Command::SetNotes {
                track: t,
                clip: c,
                notes,
            }],
        });
    }

    /// Copy the selection right after itself (FL Ctrl+B / Live Ctrl+D). The offset is the
    /// selection's span rounded up to the grid, so repeated presses tile cleanly.
    pub fn duplicate_selection(&mut self) {
        let Some((t, c, sel)) = self.selected_note_values() else {
            return;
        };
        let start = sel.iter().map(|n| n.start).min().unwrap_or(Ticks::ZERO);
        let end = sel.iter().map(|n| n.end()).max().unwrap_or(Ticks::ZERO);
        let grid = self.piano_roll.grid;
        let span = end - start;
        let offset = Ticks(((span.0 + grid.0 - 1) / grid.0).max(1) * grid.0);
        let copies: Vec<_> = sel
            .iter()
            .map(|n| {
                let mut m = self
                    .bus
                    .new_note(n.start + offset, n.length, n.pitch, n.velocity);
                m.muted = n.muted;
                m
            })
            .collect();
        let ids = copies.iter().map(|n| n.id).collect();
        let mut commands = vec![Command::InsertNotes {
            track: t,
            clip: c,
            notes: copies,
        }];
        // Grow the clip if the copies run past its end.
        if let Some(clip) = self.project().track(t).and_then(|tr| tr.clip(c)) {
            if clip.length < end + offset {
                commands.insert(
                    0,
                    Command::SetClipBounds {
                        track: t,
                        clip: c,
                        start: clip.start,
                        length: end + offset,
                    },
                );
            }
        }
        if self.exec(Command::Batch {
            label_key: "cmd.duplicate".into(),
            commands,
        }) {
            self.selected_notes = ids;
        }
    }

    pub fn copy_selection(&mut self) {
        let Some((_, _, sel)) = self.selected_note_values() else {
            return;
        };
        let start = sel.iter().map(|n| n.start).min().unwrap_or(Ticks::ZERO);
        self.clipboard = sel
            .into_iter()
            .map(|n| cantus_core::Note {
                start: n.start - start,
                ..n
            })
            .collect();
    }

    pub fn cut_selection(&mut self) {
        self.copy_selection();
        self.delete_selection();
    }

    /// Paste at the playhead (clip-relative, snapped to the grid), or at 0 if the playhead
    /// is outside the clip.
    pub fn paste_clipboard(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let (Some(t), Some(c)) = (self.selected_track, self.selected_clip) else {
            return;
        };
        let Some(clip) = self.project().track(t).and_then(|tr| tr.clip(c)).cloned() else {
            return;
        };
        let ph = self.engine.position() - clip.start;
        let at = if ph >= Ticks::ZERO && ph < clip.length {
            ph.snap_floor(self.piano_roll.grid)
        } else {
            Ticks::ZERO
        };
        let notes: Vec<_> = self
            .clipboard
            .clone()
            .into_iter()
            .map(|n| {
                let mut m = self
                    .bus
                    .new_note(n.start + at, n.length, n.pitch, n.velocity);
                m.muted = n.muted;
                m
            })
            .collect();
        let end = notes.iter().map(|n| n.end()).max().unwrap_or(Ticks::ZERO);
        let ids = notes.iter().map(|n| n.id).collect();
        let mut commands = vec![Command::InsertNotes {
            track: t,
            clip: c,
            notes,
        }];
        if clip.length < end {
            commands.insert(
                0,
                Command::SetClipBounds {
                    track: t,
                    clip: c,
                    start: clip.start,
                    length: end,
                },
            );
        }
        if self.exec(Command::Batch {
            label_key: "cmd.paste".into(),
            commands,
        }) {
            self.selected_notes = ids;
        }
    }

    // ---- theme ----------------------------------------------------------------------

    /// Write a preset to the theme file and apply it.
    pub fn apply_theme_preset(&mut self, theme: theme::Theme) {
        if let Err(e) = theme::save_file(&theme, &self.theme_path) {
            self.set_error(tf("status.theme_error", &[("error", &e)]));
            return;
        }
        if let Some(w) = self.theme_watcher.as_mut() {
            *w = theme::ThemeWatcher::new(self.theme_path.clone());
        }
        self.apply_theme(&theme);
    }

    fn apply_theme(&mut self, theme: &theme::Theme) {
        if let Some(ctx) = &self.theme_ctx {
            theme::apply(theme, ctx);
        }
        self.set_status(tf("status.theme_applied", &[("name", &theme.name)]));
    }

    /// Show the theme file in the OS file manager so the user can edit it.
    pub fn reveal_theme_file(&mut self) {
        let path = self.theme_path.clone();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn();
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let result = std::process::Command::new("xdg-open")
            .arg(path.parent().unwrap_or(&path))
            .spawn();
        match result {
            Ok(_) => self.set_status(tf("status.theme_file", &[("path", &path.display())])),
            Err(e) => self.set_error(e.to_string()),
        }
    }

    fn poll_theme(&mut self) {
        let Some(result) = self.theme_watcher.as_mut().and_then(|w| w.poll()) else {
            return;
        };
        match result {
            Ok(theme) => self.apply_theme(&theme),
            Err(e) => self.set_error(tf("status.theme_error", &[("error", &e)])),
        }
    }

    // ---- effects --------------------------------------------------------------------

    pub fn add_effect(&mut self, track: TrackId, kind: EffectKind) {
        let slot = self.bus.new_effect(kind);
        let index = self
            .project()
            .track(track)
            .map(|t| t.effects.len())
            .unwrap_or(0);
        self.exec(Command::InsertEffect {
            track,
            index,
            slot: Box::new(slot),
        });
    }

    pub fn remove_effect(&mut self, track: TrackId, effect: EffectId) {
        self.exec(Command::RemoveEffect { track, effect });
    }

    // ---- recording ------------------------------------------------------------------

    pub fn is_recording(&self) -> bool {
        self.recorder.is_recording()
    }

    /// Start recording onto the selected track (creating a clip at the playhead if the
    /// track has none) and roll the transport.
    pub fn start_recording(&mut self) {
        let Some(track) = self.selected_track else {
            return;
        };
        if self.selected_clip.is_none() {
            let start = self
                .engine
                .position()
                .snap_floor(self.project().time_signature.ticks_per_bar());
            let len = self.project().time_signature.ticks_per_bar() * 4;
            self.create_clip(track, start, len);
        }
        self.recorder.start();
        self.engine.set_recording(true);
        if !self.engine.is_playing() {
            self.engine.play();
        }
        self.set_status(t("status.recording"));
    }

    pub fn stop_recording(&mut self) {
        if !self.recorder.is_recording() {
            return;
        }
        let at = self.engine.position();
        let notes = self.recorder.stop(at);
        self.engine.set_recording(false);
        self.commit_recorded(notes);
        self.end_gesture();
        self.set_status(t("status.ready"));
    }

    pub fn toggle_record(&mut self) {
        if self.is_recording() {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    /// Feed live-note events (normally from the engine) into the current take.
    pub fn ingest_note_inputs(&mut self, inputs: Vec<NoteInput>) {
        if inputs.is_empty() {
            return;
        }
        let loop_region = if self.project().loop_enabled {
            self.project().loop_region.clone()
        } else {
            None
        };
        let notes = self.recorder.ingest(&inputs, loop_region.as_ref());
        self.commit_recorded(notes);
    }

    fn commit_recorded(&mut self, notes: Vec<RecordedNote>) {
        if notes.is_empty() {
            return;
        }
        let (Some(track), Some(clip_id)) = (self.selected_track, self.selected_clip) else {
            return;
        };
        let Some(clip) = self
            .project()
            .track(track)
            .and_then(|t| t.clip(clip_id))
            .cloned()
        else {
            return;
        };
        let mut commands = Vec::new();
        // Grow the clip so every recorded note lies inside it.
        let mut start = clip.start;
        let mut end = clip.end();
        for n in &notes {
            start = start.min(n.start);
            end = end.max(n.start + n.length);
        }
        if start != clip.start || end != clip.end() {
            commands.push(Command::SetClipBounds {
                track,
                clip: clip_id,
                start,
                length: end - start,
            });
            if start != clip.start {
                // Shift existing notes so they stay where they sound.
                let shift = clip.start - start;
                let moved: Vec<_> = clip
                    .notes
                    .iter()
                    .map(|n| cantus_core::Note {
                        start: n.start + shift,
                        ..n.clone()
                    })
                    .collect();
                if !moved.is_empty() {
                    commands.push(Command::SetNotes {
                        track,
                        clip: clip_id,
                        notes: moved,
                    });
                }
            }
        }
        let new_notes: Vec<_> = notes
            .iter()
            .map(|n| {
                self.bus
                    .new_note(n.start - start, n.length, n.pitch, n.velocity)
            })
            .collect();
        commands.push(Command::InsertNotes {
            track,
            clip: clip_id,
            notes: new_notes,
        });
        let key = MergeKey::of(&("take", self.recorder.take()));
        self.exec_merging(
            Command::Batch {
                label_key: "cmd.record_take".into(),
                commands,
            },
            key,
        );
    }

    fn poll_recording(&mut self) {
        let inputs = self.engine.take_note_inputs();
        if self.recorder.is_recording() {
            self.ingest_note_inputs(inputs);
            if !self.engine.is_playing() && self.engine.stats().blocks_processed > 0 {
                // Transport stopped (space, end of loop-less content): close the take.
                self.stop_recording();
            }
        }
    }

    fn sync_live_track(&mut self) {
        if self.live_track_sent != Some(self.selected_track) {
            self.live_track_sent = Some(self.selected_track);
            self.engine.set_live_track(self.selected_track);
        }
    }

    // ---- transport ------------------------------------------------------------------

    pub fn toggle_play(&mut self) {
        if self.engine.is_playing() {
            if self.is_recording() {
                self.stop_recording();
            }
            self.engine.stop();
        } else {
            self.engine.play();
        }
    }

    pub fn stop(&mut self) {
        if self.is_recording() {
            self.stop_recording();
        }
        if self.engine.is_playing() {
            self.engine.stop();
        } else {
            self.seek(Ticks::ZERO);
        }
    }

    pub fn seek(&mut self, tick: Ticks) {
        self.engine.seek(tick.max(Ticks::ZERO));
    }

    /// Short preview of a pitch on a track (note drawn / moved).
    pub fn audition(&mut self, track: TrackId, pitch: u8) {
        self.engine.note_on(track, pitch, 96);
        self.auditions.push((track, pitch, Instant::now()));
    }

    fn apply_key_event(&mut self, ev: KeyEvent) {
        let Some(track) = self.selected_track else {
            return;
        };
        match ev {
            KeyEvent::On(p) => self.engine.note_on(track, p, self.keyboard.velocity),
            KeyEvent::Off(p) => self.engine.note_off(track, p),
        }
    }

    // ---- files ----------------------------------------------------------------------

    pub fn new_project(&mut self) {
        self.engine.stop();
        self.bus.reset(starter_project());
        self.path = None;
        self.selected_track = None;
        self.selected_clip = None;
        self.selected_notes.clear();
        self.select_first();
        self.seek(Ticks::ZERO);
    }

    pub fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Cantus project", &[cantus_project::FILE_EXTENSION])
            .pick_file()
        {
            self.open(&path);
        }
    }

    pub fn open(&mut self, path: &Path) {
        let loaded = match cantus_project::load(path, true) {
            Ok(l) => l,
            Err(cantus_project::StoreError::Format(cantus_project::FormatError::Integrity {
                ..
            })) => match cantus_project::load(path, false) {
                Ok(l) => {
                    self.set_error(t("status.integrity"));
                    l
                }
                Err(e) => {
                    self.set_error(tf("status.open_failed", &[("error", &e)]));
                    return;
                }
            },
            Err(e) => {
                self.set_error(tf("status.open_failed", &[("error", &e)]));
                return;
            }
        };
        self.engine.stop();
        self.bus.reset(loaded.project);
        self.path = Some(path.to_path_buf());
        self.selected_track = None;
        self.selected_clip = None;
        self.selected_notes.clear();
        self.select_first();
        self.seek(Ticks::ZERO);
        if let Some(from) = loaded.migrated_from {
            self.set_status(tf("status.migrated", &[("from", &from)]));
        } else if !self.status.error {
            self.set_status(tf("status.opened", &[("path", &path.display())]));
        }
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(p) => self.save_to_path(&p),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) {
        let name = format!("{}.{}", self.project().name, cantus_project::FILE_EXTENSION);
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Cantus project", &[cantus_project::FILE_EXTENSION])
            .set_file_name(name)
            .save_file()
        {
            self.save_to_path(&path);
        }
    }

    pub fn save_to_path(&mut self, path: &Path) {
        match cantus_project::save(self.project(), path, APP_VERSION) {
            Ok(r) => {
                self.path = Some(path.to_path_buf());
                self.bus.mark_saved();
                self.autosaved_revision = self.bus.revision();
                self.set_status(tf("status.saved", &[("path", &r.path.display())]));
            }
            Err(e) => self.set_error(tf("status.save_failed", &[("error", &e)])),
        }
    }

    fn maybe_autosave(&mut self) {
        if self.last_autosave.elapsed() < AUTOSAVE_INTERVAL {
            return;
        }
        self.last_autosave = Instant::now();
        if !self.bus.is_dirty() || self.bus.revision() == self.autosaved_revision {
            return;
        }
        match cantus_project::autosave(
            self.project(),
            self.path.as_deref(),
            &self.autosave_dir,
            APP_VERSION,
        ) {
            Ok(_) => {
                self.autosaved_revision = self.bus.revision();
                if !self.status.error {
                    self.set_status(t("status.autosaved"));
                }
            }
            Err(e) => log::warn!("autosave failed: {e}"),
        }
    }

    pub fn export_dialog(&mut self) {
        if self.export.is_some() {
            return;
        }
        let name = format!("{}.wav", self.project().name);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV", &["wav"])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        self.export_to(path);
    }

    /// Render the project to a WAV file on a background thread; poll with
    /// [`CantusApp::poll_export`] (called every frame) or [`CantusApp::export_in_progress`].
    pub fn export_to(&mut self, path: PathBuf) {
        if self.export.is_some() {
            return;
        }
        let snapshot = Arc::new(Snapshot::compile(self.project(), self.bus.revision()));
        let opts = RenderOptions {
            sample_rate: self.engine.sample_rate.round() as u32,
            ..Default::default()
        };
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("cantus-export".into())
            .spawn(move || {
                let result = cantus_engine::render_to_wav(snapshot, &opts, &path)
                    .map(|r| (path, r))
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
            })
            .expect("spawn export thread");
        self.export = Some(rx);
        self.set_status(t("status.exporting"));
    }

    pub fn export_in_progress(&self) -> bool {
        self.export.is_some()
    }

    pub fn poll_export(&mut self) {
        let Some(rx) = &self.export else { return };
        match rx.try_recv() {
            Ok(Ok((path, r))) => {
                self.export = None;
                let peak_db = cantus_core::linear_to_db(r.peak);
                self.set_status(tf(
                    "status.exported",
                    &[
                        ("path", &path.display()),
                        ("seconds", &format!("{:.1}", r.duration_seconds())),
                        ("peak", &format!("{peak_db:.1}")),
                    ],
                ));
            }
            Ok(Err(e)) => {
                self.export = None;
                self.set_error(tf("status.export_failed", &[("error", &e)]));
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.export = None;
                self.set_error(tf("status.export_failed", &[("error", &"thread died")]));
            }
        }
    }

    // ---- per-frame ------------------------------------------------------------------

    pub fn sync_engine(&mut self) {
        let rev = self.bus.revision();
        if rev != self.synced_revision {
            self.synced_revision = rev;
            self.engine
                .set_snapshot(Arc::new(Snapshot::compile(self.project(), rev)));
        }
    }

    fn global_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (space, home, cmd, z, y, s, o, n, e, a, del, q, l, m, shift) = ctx.input(|i| {
            use egui::Key;
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::Home),
                i.modifiers.command,
                i.key_pressed(Key::Z),
                i.key_pressed(Key::Y),
                i.key_pressed(Key::S),
                i.key_pressed(Key::O),
                i.key_pressed(Key::N),
                i.key_pressed(Key::E),
                i.key_pressed(Key::A),
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.key_pressed(Key::Q),
                i.key_pressed(Key::L),
                i.key_pressed(Key::M),
                i.modifiers.shift,
            )
        });
        let r = ctx.input(|i| i.key_pressed(egui::Key::R));
        if r && !cmd {
            self.toggle_record();
        }
        // Tools: number keys (Bitwig convention; letters are taken by the typing piano).
        let digit = ctx.input(|i| {
            use egui::Key;
            [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
            ]
            .iter()
            .position(|k| i.key_pressed(*k))
        });
        if let (Some(d), false) = (digit, cmd) {
            if let Some(tool) = views::piano_roll::Tool::ALL.get(d) {
                self.piano_roll.tool = *tool;
            }
        }
        // FL letters, only while the typing piano is not claiming the letter keys.
        if !cmd && !self.keyboard_piano {
            let picked = ctx.input(|i| {
                views::piano_roll::Tool::ALL
                    .iter()
                    .copied()
                    .find(|t| i.key_pressed(t.key()))
            });
            if let Some(tool) = picked {
                self.piano_roll.tool = tool;
            }
        }
        let (b, d, c, v, x, tkey, esc, left, right, up, down) = ctx.input(|i| {
            use egui::Key;
            (
                i.key_pressed(Key::B),
                i.key_pressed(Key::D),
                i.key_pressed(Key::C),
                i.key_pressed(Key::V),
                i.key_pressed(Key::X),
                i.key_pressed(Key::T),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
            )
        });
        if cmd && (b || d) {
            self.duplicate_selection();
        }
        if cmd && c {
            self.copy_selection();
        }
        if cmd && x {
            self.cut_selection();
        }
        if cmd && v {
            self.paste_clipboard();
        }
        if cmd && tkey {
            self.keyboard_piano = !self.keyboard_piano;
            for ev in self.keyboard.release_all() {
                self.apply_key_event(ev);
            }
        }
        if esc {
            self.selected_notes.clear();
        }
        let grid = self.piano_roll.grid;
        let step = if shift {
            self.project().time_signature.ticks_per_bar()
        } else {
            grid
        };
        if left {
            self.nudge_selection(-step);
        }
        if right {
            self.nudge_selection(step);
        }
        if up {
            self.transpose_selection(if shift { 12 } else { 1 });
        }
        if down {
            self.transpose_selection(if shift { -12 } else { -1 });
        }
        if space {
            self.toggle_play();
        }
        if home {
            self.seek(Ticks::ZERO);
        }
        if cmd && z && shift || cmd && y {
            self.redo();
        } else if cmd && z {
            self.undo();
        }
        if cmd && s {
            if shift {
                self.save_as();
            } else {
                self.save();
            }
        }
        if cmd && o {
            self.open_dialog();
        }
        if cmd && n {
            self.new_project();
        }
        if cmd && e {
            self.export_dialog();
        }
        if cmd && a {
            self.select_all_notes();
        }
        if cmd && l {
            let enabled = !self.project().loop_enabled;
            self.exec(Command::SetLoopEnabled { enabled });
        }
        if cmd && m {
            let enabled = !self.project().metronome;
            self.exec(Command::SetMetronome { enabled });
        }
        if del {
            self.delete_selection();
        }
        if q && !cmd {
            self.quantize_selection();
        }
        if !cmd && self.keyboard_piano {
            for ev in self.keyboard.poll(ctx) {
                self.apply_key_event(ev);
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        // The engine and MIDI readouts on the right are the ones that must always be
        // legible, so they claim their width first and the tool hint takes what is left.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let s = self.engine.stats();
                let engine = if self.engine.is_live() {
                    tf(
                        "status.engine",
                        &[
                            ("device", &self.engine.device_name),
                            ("rate", &(self.engine.sample_rate as u32)),
                            ("load", &format!("{:.0}", s.load * 100.0)),
                            ("overruns", &s.overruns),
                            ("voices", &s.active_voices),
                        ],
                    )
                } else {
                    t("status.no_audio")
                };
                ui.label(
                    egui::RichText::new(engine)
                        .text_style(design::caption())
                        .color(if s.overruns > 0 {
                            theme::p().danger
                        } else {
                            theme::p().text_dim
                        }),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(tf("status.midi", &[("n", &self.midi_port_names.len())]))
                        .text_style(design::caption())
                        .color(theme::p().text_dim),
                );
                ui.separator();
                let kb = if self.keyboard_piano {
                    tf("status.keyboard", &[("octave", &self.keyboard.octave)])
                } else {
                    t("status.keyboard_off")
                };
                ui.label(
                    egui::RichText::new(kb)
                        .text_style(design::caption())
                        .color(theme::p().text_dim),
                );
                if self.show_stats {
                    ui.separator();
                    let avg = if self.frame_times.is_empty() {
                        0.0
                    } else {
                        self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "ui {:.1} ms · notes drawn {} · block {} · max {:.0} µs",
                            avg * 1000.0,
                            self.piano_roll_notes_drawn,
                            s.block_frames,
                            s.max_block_us
                        ))
                        .text_style(design::mono_small())
                        .color(theme::p().text_dim),
                    );
                }
                ui.separator();
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let color = if self.status.error {
                        theme::p().danger
                    } else {
                        theme::p().text_dim
                    };
                    ui.label(
                        egui::RichText::new(&self.status.text)
                            .text_style(design::caption())
                            .color(color),
                    );
                    if self.central == CentralView::PianoRoll {
                        ui.separator();
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(t(self.piano_roll.tool.hint_key()))
                                    .text_style(design::caption())
                                    .color(theme::p().text_dim),
                            )
                            .truncate(),
                        );
                    }
                });
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.show_about {
            egui::Window::new(t("menu.about"))
                .collapsible(false)
                .resizable(false)
                .open(&mut self.show_about)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("Cantus")
                            .size(24.0)
                            .family(design::family_strong())
                            .color(theme::p().gold),
                    );
                    ui.label(
                        egui::RichText::new(format!("v{APP_VERSION}"))
                            .text_style(design::mono_small())
                            .color(theme::p().text_dim),
                    );
                    ui.add_space(space::S2);
                    ui.label(t("about.text"));
                });
        }
        if let Some(name) = self.naming_layout.clone() {
            let mut name = name;
            let mut close = false;
            let mut save = false;
            egui::Window::new(t("layout.save_as"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(t("layout.name"));
                    let edit =
                        ui.add(egui::TextEdit::singleline(&mut name).hint_text(t("layout.name")));
                    // Focus the field when the dialog opens, but only then: re-requesting
                    // every frame would trap focus and stop the user tabbing to the
                    // buttons.
                    if ui.ctx().memory(|m| m.focused()).is_none() {
                        edit.request_focus();
                    }
                    save = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(space::S2);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !name.trim().is_empty(),
                                egui::Button::new(t("layout.save")),
                            )
                            .clicked()
                        {
                            save = true;
                        }
                        if ui.button(t("layout.cancel")).clicked() {
                            close = true;
                        }
                    });
                });
            if save && !name.trim().is_empty() {
                self.save_layout_as(&name, ctx);
                close = true;
            }
            self.naming_layout = if close { None } else { Some(name) };
        }
        if self.show_shortcuts {
            egui::Window::new(t("menu.shortcuts"))
                .collapsible(false)
                .open(&mut self.show_shortcuts)
                .show(ctx, |ui| {
                    ui.set_max_width(space::GRID * 130.0);
                    for line in t("shortcuts.text").split(" · ") {
                        ui.label(line);
                    }
                });
        }
    }
}

impl eframe::App for CantusApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        let frame_start = Instant::now();
        self.engine.poll();
        self.poll_export();
        self.poll_recording();
        self.poll_theme();
        self.global_shortcuts(ctx);
        self.sync_live_track();

        // Release auditions after 250 ms.
        let now = Instant::now();
        let mut i = 0;
        while i < self.auditions.len() {
            if now.duration_since(self.auditions[i].2) > Duration::from_millis(250) {
                let (track, pitch, _) = self.auditions.remove(i);
                self.engine.note_off(track, pitch);
            } else {
                i += 1;
            }
        }

        // Chrome panels sit one step above the canvas and are separated from it by a
        // hairline on the edge they meet it at (design::Elevation).
        let pal = theme::p();
        let panel = Elevation::Panel.frame(&pal);
        let hairline = design::hairline(&pal);
        egui::Panel::top(egui::Id::new("transport"))
            .frame(panel.stroke(hairline).outer_margin(egui::Margin {
                bottom: -1,
                ..Default::default()
            }))
            .show(root, |ui| views::transport::show(self, ui));
        egui::Panel::bottom(egui::Id::new("status"))
            .frame(panel.stroke(hairline).outer_margin(egui::Margin {
                top: -1,
                ..Default::default()
            }))
            .show(root, |ui| self.status_bar(ui));
        // A layout being applied pins the panel for one frame (`Layouts::forced`);
        // otherwise the panel is resizable and the user's drag is what counts.
        let forced_inspector = self.layouts.forced(layout::INSPECTOR);
        let forced_arrangement = self.layouts.forced(layout::ARRANGEMENT);
        if self.layouts.current.show_inspector {
            let p = egui::Panel::right(layout::panel_id(layout::INSPECTOR));
            let p = match forced_inspector {
                Some(w) => p.exact_size(w),
                None => p
                    .resizable(true)
                    .default_size(space::GRID * 76.0)
                    .min_size(space::GRID * 60.0),
            };
            p.frame(
                panel
                    .inner_margin(egui::Margin::same(space::S3 as i8))
                    .stroke(hairline)
                    .outer_margin(egui::Margin {
                        left: -1,
                        ..Default::default()
                    }),
            )
            .show(root, |ui| views::inspector::show(self, ui));
        }
        if self.layouts.current.show_arrangement {
            let p = egui::Panel::top(layout::panel_id(layout::ARRANGEMENT));
            let p = match forced_arrangement {
                Some(h) => p.exact_size(h),
                None => p
                    .resizable(true)
                    .default_size(space::GRID * 65.0)
                    .min_size(space::GRID * 30.0),
            };
            p.frame(Elevation::Canvas.frame(&pal))
                .show(root, |ui| views::arrangement::show(self, ui));
        }
        egui::CentralPanel::default()
            .frame(
                Elevation::Canvas
                    .frame(&pal)
                    .inner_margin(egui::Margin::same(space::S1 as i8)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| views::transport::view_tabs(self, ui));
                match self.central {
                    CentralView::PianoRoll => views::piano_roll::show(self, ui),
                    CentralView::Mixer => views::mixer::show(self, ui),
                }
            });
        self.dialogs(ctx);
        design::paint_focus_ring(ctx, &pal);
        self.layouts.end_frame();

        self.sync_engine();
        self.maybe_autosave();

        if self.request_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.store_layouts(ctx);
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.bus.is_dirty() {
            // Last line of defence: never lose work on close.
            let _ = cantus_project::autosave(
                self.project(),
                self.path.as_deref(),
                &self.autosave_dir,
                APP_VERSION,
            );
        }

        let dt = frame_start.elapsed().as_secs_f32();
        if self.frame_times.len() >= 120 {
            self.frame_times.remove(0);
        }
        self.frame_times.push(dt);
        ctx.request_repaint_after(Duration::from_millis(
            if self.engine.is_playing() || !self.auditions.is_empty() || self.is_recording() {
                16
            } else {
                100
            },
        ));
    }
}

fn directories_fallback() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".local/share");
                p.into_os_string()
            })
        })
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Cantus")
}

/// A new project that already makes a sound: one track, one 4-bar clip, a small motif.
pub fn starter_project() -> Project {
    let mut p = Project::new(t("app.untitled"));
    let mut track = p.make_track(tf("track.new_name", &[("n", &1)]));
    let mut clip = p.make_clip(t("clip.new"), Ticks::ZERO, Ticks::beats(16));
    let motif = [(0, 60), (1, 64), (2, 67), (3, 72)];
    for (beat, pitch) in motif {
        let n = p.make_note(Ticks::beats(beat), Ticks::QUARTER - Ticks(60), pitch, 100);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    p.tracks.push(track);
    p
}

/// ADR 0003 guardrail: 20 000 notes in one clip, 8 tracks.
pub fn stress_project() -> Project {
    let mut p = Project::new("Stress");
    for t in 0..8 {
        let mut track = p.make_track(format!("Stress {}", t + 1));
        let mut clip = p.make_clip("Dense", Ticks::ZERO, Ticks::beats(640));
        let mut x = 12345u64 + t as u64;
        for i in 0..20_000 {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let pitch = 36 + ((x >> 33) % 60) as u8;
            let vel = 40 + ((x >> 40) % 87) as u8;
            let n = p.make_note(
                Ticks::SIXTEENTH * (i as i64 % 10_240),
                Ticks::SIXTEENTH,
                pitch,
                vel,
            );
            clip.notes.push(n);
        }
        clip.notes.sort_by_key(|n| (n.start, n.pitch));
        track.clips.push(clip);
        p.tracks.push(track);
    }
    p
}

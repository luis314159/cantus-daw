//! The application: owns the command bus, the engine handle, selection and view state.
//! Every mutation goes through [`CantusApp::exec`] (ADR 0004); the engine only ever sees
//! compiled snapshots.

use crate::i18n::{t, tf};
use crate::keyboard::{KeyEvent, KeyboardPiano};
use crate::theme;
use crate::views::{self, arrangement::ArrangementView, piano_roll::PianoRollView};
use cantus_core::{
    ClipId, Command, CommandBus, MergeKey, Origin, Project, Snapshot, Ticks, TrackId,
};
use cantus_engine::{AudioConfig, EngineHandle, RenderOptions, Rendered};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

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
    pub piano_roll_notes_drawn: usize,
    pub keyboard: KeyboardPiano,
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
        // Cantus has one look. Pin the theme so a light system preference (which egui
        // follows by default) cannot swap in light widget colours under our dark surfaces.
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx
            .set_visuals_of(egui::Theme::Dark, theme::visuals());
        cc.egui_ctx
            .set_visuals_of(egui::Theme::Light, theme::visuals());
        let engine = match EngineHandle::start(&AudioConfig::default()) {
            Ok(e) => e,
            Err(e) => {
                log::error!("audio: {e}");
                EngineHandle::dummy()
            }
        };
        Self::from_parts(project, engine, stress)
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
            piano_roll_notes_drawn: 0,
            keyboard: KeyboardPiano::default(),
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

    // ---- transport ------------------------------------------------------------------

    pub fn toggle_play(&mut self) {
        if self.engine.is_playing() {
            self.engine.stop();
        } else {
            self.engine.play();
        }
    }

    pub fn stop(&mut self) {
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

    fn sync_engine(&mut self) {
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
        if !cmd {
            for ev in self.keyboard.poll(ctx) {
                self.apply_key_event(ev);
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let color = if self.status.error {
                theme::DANGER
            } else {
                theme::TEXT_DIM
            };
            ui.label(egui::RichText::new(&self.status.text).color(color));
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
                        .small()
                        .color(if s.overruns > 0 {
                            theme::DANGER
                        } else {
                            theme::TEXT_DIM
                        }),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(tf(
                        "status.keyboard",
                        &[("octave", &self.keyboard.octave)],
                    ))
                    .small()
                    .color(theme::TEXT_DIM),
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
                        .small()
                        .monospace()
                        .color(theme::TEXT_DIM),
                    );
                }
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
                    ui.label(egui::RichText::new("Cantus").size(22.0).color(theme::GOLD));
                    ui.label(format!("v{APP_VERSION}"));
                    ui.add_space(6.0);
                    ui.label(t("about.text"));
                });
        }
        if self.show_shortcuts {
            egui::Window::new(t("menu.shortcuts"))
                .collapsible(false)
                .open(&mut self.show_shortcuts)
                .show(ctx, |ui| {
                    ui.set_max_width(520.0);
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
        self.global_shortcuts(ctx);

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

        egui::Panel::top(egui::Id::new("transport"))
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(root, |ui| views::transport::show(self, ui));
        egui::Panel::bottom(egui::Id::new("status"))
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(root, |ui| self.status_bar(ui));
        egui::Panel::right(egui::Id::new("inspector"))
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(root, |ui| views::inspector::show(self, ui));
        egui::Panel::top(egui::Id::new("arrangement"))
            .resizable(true)
            .default_size(260.0)
            .min_size(120.0)
            .frame(egui::Frame::NONE)
            .show(root, |ui| views::arrangement::show(self, ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(theme::INK)
                    .inner_margin(egui::Margin::same(4)),
            )
            .show(root, |ui| views::piano_roll::show(self, ui));
        self.dialogs(ctx);

        self.sync_engine();
        self.maybe_autosave();

        if self.request_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
            if self.engine.is_playing() || !self.auditions.is_empty() {
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

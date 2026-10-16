//! Audio files in the application (ADR 0025): importing them (dropped from the system or
//! picked from *File ▸ Import audio…*), collecting them into the project's folder when it
//! is saved, finding a missing one again, and showing one in the file manager.
//!
//! The file work of saving is `cantus_project::collect`; this is where it meets the
//! command bus (one undoable `SetAudioSourcePath` batch) and the peak store (whose cache
//! follows a file that moved, so nothing is read twice).

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};
use cantus_core::{
    AudioSource, AudioSourceId, Clip, Color, Command, Project, SampleData, Ticks, Track, TrackId,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the import reads, by extension (the engine's decoders: WAV, AIFF, FLAC, Ogg).
pub const EXTENSIONS: [&str; 8] = ["wav", "wave", "aif", "aiff", "aifc", "flac", "ogg", "oga"];

/// Whether a file is one the import takes, by its extension.
pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// A file read for import: where it is, what to call it, and its samples (kept so the
/// peaks are computed from them rather than from a second read).
#[derive(Clone, Debug)]
pub struct ImportFile {
    pub path: PathBuf,
    pub name: String,
    pub data: Arc<SampleData>,
}

/// Decode a file for import.
pub fn read(path: &Path) -> Result<ImportFile, String> {
    if !is_audio_path(path) {
        return Err(t("audioclips.error.format"));
    }
    let data = cantus_engine::SampleCache::new().load(path)?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| t("audioclips.default_name"));
    Ok(ImportFile {
        path: path.to_path_buf(),
        name,
        data,
    })
}

/// The timeline length of `frames` at `rate` played from `at` at natural speed: at the
/// project's tempo, through the tempo map when it changes (ADR 0060).
pub fn length_ticks(project: &Project, at: Ticks, frames: u64, rate: u32) -> Ticks {
    let seconds = frames as f64 / rate.max(1) as f64;
    let map = crate::views::bars::map(project);
    if map.is_constant() {
        return Ticks::from_seconds(seconds, project.tempo_bpm);
    }
    cantus_core::audio_time::length_ticks(&map, at, frames as f64, rate.max(1) as f64)
}

/// The project file's folder: what relative source paths are relative to.
pub fn base_of(path: Option<&Path>) -> Option<&Path> {
    path.and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
}

/// How a path is stored: relative (`/`-separated) when it is inside the project's folder,
/// absolute otherwise.
pub fn stored_path(base: Option<&Path>, file: &Path) -> String {
    if let Some(b) = base {
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        if let Ok(rest) = canon(file).strip_prefix(canon(b)) {
            let parts: Vec<String> = rest
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            if !parts.is_empty() {
                return parts.join("/");
            }
        }
    }
    file.to_string_lossy().into_owned()
}

/// The strips not yet taken by a track, in the order `Project::first_free_strip` would
/// hand them out — so several tracks made in one batch do not share one.
pub(crate) fn free_strips(project: &Project) -> Vec<cantus_core::StripId> {
    let taken = |id| project.tracks.iter().any(|t| t.strip == Some(id));
    let mut out: Vec<_> = project
        .strips
        .iter()
        .filter(|s| s.dock == cantus_core::Dock::Middle && !taken(s.id))
        .map(|s| s.id)
        .collect();
    out.extend(
        project
            .strips
            .iter()
            .filter(|s| s.dock != cantus_core::Dock::Middle && !taken(s.id))
            .map(|s| s.id),
    );
    out
}

/// Every relative source path made absolute against `base`: an autosave may be opened
/// from a folder that is not the project's (the fallback autosave folder, a recovery), and
/// its audio must still be found.
pub fn absolute_audio_paths(project: &mut cantus_core::Project, base: Option<&Path>) {
    let Some(base) = base else {
        return;
    };
    for s in &mut project.audio_pool {
        if Path::new(&s.path).is_relative() {
            s.path = base.join(&s.path).to_string_lossy().into_owned();
        }
    }
}

impl CantusApp {
    /// What an autosave writes: the project as a save would (hosted plug-ins' state as it
    /// is now), with every audio path absolute so a recovered autosave finds its audio
    /// wherever it is opened from.
    pub fn autosave_copy(&mut self) -> cantus_core::Project {
        let mut p = self.plugins.project_for_save(self.bus.project());
        absolute_audio_paths(&mut p, self.audio_base());
        p
    }

    /// The folder the project's relative audio paths resolve against.
    pub fn audio_base(&self) -> Option<&Path> {
        base_of(self.path.as_deref())
    }

    /// Where a source's file is.
    pub fn audio_file(&self, source: &AudioSource) -> PathBuf {
        crate::waveform::resolve(self.audio_base(), &source.path)
    }

    /// *File ▸ Import audio…*: pick files; each lands on a new track at the playhead's bar.
    pub fn import_audio_dialog(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter(t("audioclips.filter"), &EXTENSIONS)
            .pick_files()
        else {
            return;
        };
        let at = if self.project().content_end() == Ticks::ZERO {
            Ticks::ZERO
        } else {
            crate::views::bars::bar_start(self.project(), self.engine.position()).max(Ticks::ZERO)
        };
        self.import_audio(&paths, at, None);
    }

    /// Import `paths` at `at`: the first onto track `onto` (a row index; `None` or past
    /// the last track makes new tracks), each next one onto the track below. Every file
    /// becomes a source (not recorded) and a clip as long as the file at the project's
    /// tempo; the whole import is one undo step. Returns how many files were imported.
    pub fn import_audio(&mut self, paths: &[PathBuf], at: Ticks, onto: Option<usize>) -> usize {
        let mut files = Vec::new();
        for p in paths {
            match read(p) {
                Ok(f) => files.push(f),
                Err(e) => {
                    let file = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.set_error(tf(
                        "audioclips.status.import_failed",
                        &[("file", &file), ("error", &e)],
                    ));
                }
            }
        }
        if files.is_empty() {
            return 0;
        }
        let commands = self.import_commands(&files, at, onto);
        let made = commands.1;
        if !self.exec(Command::Batch {
            label_key: "cmd.import_audio".into(),
            commands: commands.0,
        }) {
            return 0;
        }
        for f in &files {
            self.peaks.insert_data(&f.path, f.data.clone());
        }
        self.set_clip_selection(made.clone());
        if let Some(&(t, c)) = made.first() {
            self.select_clip(t, c);
            self.set_clip_selection(made.clone());
        }
        self.focus_arrangement();
        let n = files.len();
        self.set_status(tf("audioclips.status.imported", &[("n", &n)]));
        n
    }

    /// The commands of an import, and the clips it makes.
    fn import_commands(
        &mut self,
        files: &[ImportFile],
        at: Ticks,
        onto: Option<usize>,
    ) -> (Vec<Command>, Vec<ClipRef>) {
        let base = self.audio_base().map(Path::to_path_buf);
        let mut commands = Vec::new();
        let mut made = Vec::new();
        let mut strips = free_strips(self.project());
        let existing = self.project().tracks.len();
        let mut new_tracks = 0usize;
        for (i, f) in files.iter().enumerate() {
            let id = AudioSourceId(self.bus.alloc_id());
            let frames = f.data.len_frames() as u64;
            let source = AudioSource {
                id,
                name: f.name.clone(),
                path: stored_path(base.as_deref(), &f.path),
                sample_rate: f.data.rate,
                channels: f.data.channels,
                frames,
                sha256: None,
                recorded: false,
            };
            let length = length_ticks(self.project(), at, frames, f.data.rate).max(Ticks(1));
            let row = onto.map(|r| r + i).filter(|r| *r < existing);
            let track: TrackId = match row {
                Some(r) => self.project().tracks[r].id,
                None => {
                    let mut track: Track = self.bus.new_track(f.name.clone());
                    let n = existing + new_tracks;
                    track.color = Color::PALETTE[n % Color::PALETTE.len()];
                    track.strip = if strips.is_empty() {
                        None
                    } else {
                        Some(strips.remove(0))
                    };
                    let tid = track.id;
                    commands.push(Command::InsertTrack {
                        index: n,
                        track: Box::new(track),
                    });
                    new_tracks += 1;
                    tid
                }
            };
            let clip: Clip = {
                let mut c = self.bus.new_clip(f.name.clone(), at, length);
                c.audio = Some(cantus_core::AudioRegion::of(id));
                c
            };
            made.push((track, clip.id));
            commands.push(Command::InsertAudioSource { source });
            commands.push(Command::InsertClip {
                track,
                clip: Box::new(clip),
            });
        }
        (commands, made)
    }

    /// Before writing the project to `file`: bring every audio file into
    /// `<folder>/Audio/` and store the paths relative (`cantus_project::collect`). One
    /// undo step; the peak cache follows each file that moved. Errors go to the status.
    pub fn collect_audio(&mut self, file: &Path) {
        if self.project().audio_pool.is_empty() {
            return;
        }
        let old_base = self.audio_base().map(Path::to_path_buf);
        let plan = cantus_project::collect::plan(self.project(), old_base.as_deref(), file);
        let out = cantus_project::collect::execute(self.project(), &plan);
        let new_base = base_of(Some(file)).map(Path::to_path_buf);
        let mut follow = Vec::new();
        let commands: Vec<Command> = out
            .paths
            .iter()
            .map(|(id, path)| {
                if let Some(s) = self.project().audio_source(*id) {
                    let before = crate::waveform::resolve(old_base.as_deref(), &s.path);
                    let after = crate::waveform::resolve(new_base.as_deref(), path);
                    follow.push((before, after));
                }
                Command::SetAudioSourcePath {
                    id: *id,
                    path: path.clone(),
                }
            })
            .collect();
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.collect_audio".into(),
                commands,
            });
        }
        for (before, after) in follow {
            self.peaks.follow(&before, &after);
        }
        if let Some((file, e)) = out.errors.first() {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.set_error(tf(
                "audioclips.status.collect_failed",
                &[("file", &name), ("error", e)],
            ));
        }
    }

    /// *Locate…* for a missing file: pick it, and every clip of the source follows.
    pub fn relink_audio_dialog(&mut self, id: AudioSourceId) {
        let Some(s) = self.project().audio_source(id) else {
            return;
        };
        let wanted = Path::new(&s.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(p) = rfd::FileDialog::new()
            .add_filter(t("audioclips.filter"), &EXTENSIONS)
            .set_file_name(wanted)
            .pick_file()
        {
            self.relink_audio(id, &p);
        }
    }

    /// Point a source at `file` (one undo step). The file must decode.
    pub fn relink_audio(&mut self, id: AudioSourceId, file: &Path) -> bool {
        let Some(old) = self.project().audio_source(id).map(|s| self.audio_file(s)) else {
            return false;
        };
        let data = match read(file) {
            Ok(f) => f.data,
            Err(e) => {
                self.set_error(tf(
                    "audioclips.status.import_failed",
                    &[("file", &file.display()), ("error", &e)],
                ));
                return false;
            }
        };
        let path = stored_path(self.audio_base(), file);
        if !self.exec(Command::SetAudioSourcePath { id, path }) {
            return false;
        }
        self.peaks.forget(&old);
        self.peaks.insert_data(file, data);
        self.set_status(tf(
            "audioclips.status.relinked",
            &[("file", &file.display())],
        ));
        true
    }

    /// *Show in file browser*.
    pub fn reveal_audio(&mut self, id: AudioSourceId) {
        let Some(path) = self.project().audio_source(id).map(|s| self.audio_file(s)) else {
            return;
        };
        if let Err(e) = crate::app::reveal(&path) {
            self.set_error(e.to_string());
        }
    }
}

/// Audio files dropped on the window at `pos`: on a track's row they land on that track
/// (and the ones below) at the tick under the pointer, snapped as the arrangement snaps;
/// below the last track, or off the arrangement, on new tracks.
pub fn drop_audio(app: &mut CantusApp, paths: &[PathBuf], pos: Option<egui::Pos2>) -> usize {
    let geo = app.arrangement.geometry();
    let (at, row) = match (geo, pos) {
        (Some(g), Some(p)) if p.x >= g.left && p.x <= g.left + g.width && p.y >= g.ruler_top => {
            let mut at = crate::views::beats_to_ticks(g.beats(p.x)).max(Ticks::ZERO);
            if app.arrangement.snap {
                at = crate::views::bars::snap_floor(app.project(), at, app.arrangement.grid);
            }
            (at, g.row_at(p.y))
        }
        _ => {
            let at = if app.project().content_end() == Ticks::ZERO {
                Ticks::ZERO
            } else {
                crate::views::bars::bar_start(app.project(), app.engine.position()).max(Ticks::ZERO)
            };
            (at, None)
        }
    };
    app.import_audio(paths, at, row)
}

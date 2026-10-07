//! Export audio (ADR 0033): the dialog's state, the export running on a thread of its
//! own with its progress in the dialog and the status bar, and what the dialog remembers.
//!
//! The dialog itself is drawn in `export_dialog.rs`; the work is `cantus_export`'s. The
//! last choices live in `Settings::export` — the title is the song's and is not kept.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_engine::{RenderOptions, StemOptions};
use cantus_export::{Control, Done, ExportSettings, Format, MixJob, Scope, StemsJob};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

/// The dialog's lists, one open at a time.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum List {
    Bitrate,
    Rate,
    Target,
    Ceiling,
    Tail,
}

/// The export dialog and the running export.
#[derive(Default)]
pub struct ExportUi {
    /// The dialog, while it is open: the choices being made.
    pub dialog: Option<ExportSettings>,
    pub list: Option<List>,
    job: Option<Job>,
    /// How the last export ended, shown in the dialog's footer until it closes.
    pub last: Option<Result<String, String>>,
}

struct Job {
    ctl: Arc<Control>,
    rx: mpsc::Receiver<Result<Done, String>>,
    target: PathBuf,
    /// A mix used the hosted plug-ins' render instances; they go when it ends.
    mix: bool,
}

impl ExportUi {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The running export's phase (a locale key) and how far it has got, 0…1; the
    /// fraction is `None` while the render cannot say.
    pub fn progress(&self) -> Option<(&'static str, Option<f32>)> {
        let j = self.job.as_ref()?;
        let pulse = j
            .ctl
            .render_is_indeterminate
            .load(std::sync::atomic::Ordering::Relaxed);
        Some((j.ctl.phase().key(), (!pulse).then(|| j.ctl.fraction())))
    }
}

/// Open the dialog with the last choices; `scope` overrides what it exports (the File
/// menu's *Export stems…*).
pub fn open(app: &mut CantusApp, scope: Option<Scope>) {
    let mut d = app.settings.export.clone();
    d.tags.title = app.project().name.clone();
    if let Some(s) = scope {
        d.scope = s;
    }
    if d.scope == Scope::Selection && app.time_selection.is_none() {
        d.scope = Scope::Song;
    }
    app.export.dialog = Some(d);
    app.export.list = None;
    app.export.last = None;
}

/// Close the dialog; the choices made in it are kept for next time.
pub fn close(app: &mut CantusApp) {
    if let Some(d) = app.export.dialog.take() {
        remember(app, &d);
    }
    app.export.list = None;
}

/// Keep `d` as the last choices (all but the song's title), in the settings file.
pub fn remember(app: &mut CantusApp, d: &ExportSettings) {
    let mut keep = d.clone();
    keep.tags.title.clear();
    if app.settings.export != keep {
        app.settings.export = keep;
        app.store_settings();
    }
}

/// The file name the save dialog proposes: the song's name in the chosen format.
pub fn file_name(app: &CantusApp, d: &ExportSettings) -> String {
    let name = safe_name(&app.project().name);
    format!("{name}.{}", d.format.extension())
}

/// Ask where, then start. Stems ask for a folder; a mix for a file.
pub fn choose_and_start(app: &mut CantusApp) {
    let Some(d) = app.export.dialog.clone() else {
        return;
    };
    let mut picker = rfd::FileDialog::new();
    if let Some(dir) = d.folder.as_ref().filter(|f| f.is_dir()) {
        picker = picker.set_directory(dir);
    }
    let target = if d.scope == Scope::Stems {
        picker.pick_folder()
    } else {
        picker
            .add_filter(d.format.label(), &[d.format.extension()])
            .set_file_name(file_name(app, &d))
            .save_file()
    };
    if let Some(path) = target {
        start(app, &path);
    }
}

/// Start exporting with the dialog's choices (or the last ones) to `target` — a file,
/// or for stems a folder. Returns whether it started.
pub fn start(app: &mut CantusApp, target: &Path) -> bool {
    if app.export.busy() {
        return false;
    }
    let mut d = match app.export.dialog.clone() {
        Some(d) => d,
        None => {
            let mut d = app.settings.export.clone();
            d.tags.title = app.project().name.clone();
            d
        }
    };
    if d.scope == Scope::Selection && app.time_selection.is_none() {
        d.scope = Scope::Song;
    }
    d.folder = if d.scope == Scope::Stems {
        Some(target.to_path_buf())
    } else {
        target.parent().map(Path::to_path_buf)
    };
    remember(app, &d);
    if let Some(open) = app.export.dialog.as_mut() {
        open.folder = d.folder.clone();
    }
    let rate = app.engine.sample_rate.round() as u32;
    let snapshot = app.engine.compile(app.bus.project(), app.bus.revision());
    let mut render = RenderOptions {
        sample_rate: rate,
        tail_seconds: d.tail_seconds.max(0.0) as f64,
        ..Default::default()
    };
    if let (Scope::Selection, Some((from, to))) = (d.scope, app.time_selection) {
        render.start = from;
        render.end = Some(to);
    }
    let ctl = Control::new();
    let (tx, rx) = mpsc::channel();
    let tags = d.tags.clone();
    let stems = d.scope == Scope::Stems;
    let spawned = if stems {
        let specs =
            crate::views::interop::stem_specs(app.project(), d.stems.tracks, d.stems.master);
        let job = StemsJob {
            snapshot,
            options: StemOptions {
                render,
                keep_silent: d.stems.keep_silent,
                through_master: d.stems.through_master,
            },
            specs,
        };
        let (c, dir) = (ctl.clone(), target.to_path_buf());
        std::thread::Builder::new()
            .name("cantus-export".into())
            .spawn(move || {
                let r = cantus_export::export_stems(job, &d, &tags, &dir, &c);
                let _ = tx.send(r.map_err(error_text));
            })
    } else {
        // Hosted plug-ins render through instances of their own, loaded here on the main
        // thread with the live ones' state (ADR 0019).
        let units = app.plugins.render_units(app.bus.project(), f64::from(rate));
        let instruments = app
            .plugins
            .render_instruments(app.bus.project(), f64::from(rate));
        let job = MixJob {
            snapshot,
            render,
            units,
            instruments,
        };
        let (c, path) = (ctl.clone(), target.to_path_buf());
        std::thread::Builder::new()
            .name("cantus-export".into())
            .spawn(move || {
                let r = cantus_export::export_mix(job, &d, &tags, &path, &c);
                let _ = tx.send(r.map_err(error_text));
            })
    };
    if let Err(e) = spawned {
        app.set_error(tf("status.export_failed", &[("error", &e)]));
        return false;
    }
    app.export.job = Some(Job {
        ctl,
        rx,
        target: target.to_path_buf(),
        mix: !stems,
    });
    app.export.last = None;
    app.set_status(t("status.exporting"));
    true
}

/// An error in words; a stop is the empty string.
fn error_text(e: cantus_export::ExportError) -> String {
    match e {
        cantus_export::ExportError::Cancelled => String::new(),
        cantus_export::ExportError::Empty => t("export_audio.error.empty"),
        e => e.to_string(),
    }
}

/// Ask the running export to stop.
pub fn stop(app: &mut CantusApp) {
    if let Some(j) = &app.export.job {
        j.ctl.cancel();
    }
}

/// See whether the export has finished (every frame).
pub fn poll(app: &mut CantusApp) {
    let Some(j) = &app.export.job else { return };
    let result = match j.rx.try_recv() {
        Ok(r) => r,
        Err(mpsc::TryRecvError::Empty) => return,
        Err(mpsc::TryRecvError::Disconnected) => Err("thread died".to_string()),
    };
    finish(app, result);
}

/// Wait for the running export (tests; the interface polls instead).
pub fn wait(app: &mut CantusApp) {
    if let Some(j) = &app.export.job {
        let r = j.rx.recv().unwrap_or_else(|_| Err("thread died".into()));
        finish(app, r);
    }
}

fn finish(app: &mut CantusApp, result: Result<Done, String>) {
    let Some(j) = app.export.job.take() else {
        return;
    };
    if j.mix {
        // The render is over: the plug-in instances it used go, on this thread.
        app.plugins.finish_render();
    }
    match result {
        Ok(done) => {
            let text = summary(&done, &j.target);
            app.export.last = Some(Ok(text.clone()));
            if done.unaligned {
                app.set_error(format!("{text} {}", t("interop.stems.unaligned")));
            } else {
                app.set_status(text);
            }
        }
        Err(e) if e.is_empty() => {
            app.export.last = Some(Err(t("export_audio.stopped")));
            app.set_status(t("export_audio.stopped"));
        }
        Err(e) => {
            let text = tf("status.export_failed", &[("error", &e)]);
            app.export.last = Some(Err(text.clone()));
            app.set_error(text);
        }
    }
}

/// What a finished export says: the file (or how many stems), its length and its level.
pub fn summary(done: &Done, target: &Path) -> String {
    let peak = if done.peak_db.is_finite() {
        format!("{:.1}", done.peak_db)
    } else {
        "−∞".to_string()
    };
    let seconds = format!("{:.1}", done.seconds);
    let loud = done
        .loudness
        .as_ref()
        .and_then(|n| n.lufs)
        .map(|l| format!("{l:.1}"));
    match (done.files.as_slice(), loud) {
        ([one], Some(lufs)) => tf(
            "export_audio.done_loud",
            &[
                ("path", &one.display()),
                ("seconds", &seconds),
                ("lufs", &lufs),
                ("peak", &peak),
            ],
        ),
        ([one], None) => tf(
            "status.exported",
            &[
                ("path", &one.display()),
                ("seconds", &seconds),
                ("peak", &peak),
            ],
        ),
        (many, _) => tf(
            "export_audio.done_stems",
            &[
                ("n", &many.len()),
                ("folder", &target.display()),
                ("silent", &done.silent.len()),
            ],
        ),
    }
}

/// A song's name as a file name: the characters no file system takes replaced.
pub fn safe_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let clean = clean.trim().trim_end_matches('.').trim();
    if clean.is_empty() {
        t("app.untitled")
    } else {
        clean.to_string()
    }
}

/// The format a path's extension names (`export_to` keeps working for any of them).
pub fn format_of(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    Format::ALL.into_iter().find(|f| f.extension() == ext)
}

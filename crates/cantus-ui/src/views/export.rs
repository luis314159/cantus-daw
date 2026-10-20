//! Export audio (ADR 0033): the dialog's state, the export running on a thread of its
//! own with its progress in the dialog and the status bar, and what the dialog remembers.
//!
//! The dialog itself is drawn in `export_dialog.rs`; the work is `cantus_export`'s. The
//! last choices live in `Settings::export` — the title is the song's and is not kept.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::Ticks;
use cantus_engine::{RenderOptions, StemOptions};
use cantus_export::{Control, Done, ExportSettings, Format, MixJob, Scope, StemsJob};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

/// The export dialog and the running export.
#[derive(Default)]
pub struct ExportUi {
    /// The dialog, while it is open: the choices being made.
    pub dialog: Option<ExportSettings>,
    /// A range asked for that the project does not have (a selection or a loop brace):
    /// its segment stays unlit and the dialog says why until it closes.
    pub asked: Option<Scope>,
    /// The constant bitrate and the VBR level last chosen, so switching the MP3 mode
    /// back and forth keeps each.
    pub mp3_cbr: Option<u16>,
    pub mp3_vbr: Option<u8>,
    /// The dialog has just opened: menus left open behind it are closed on its first frame.
    pub fresh: bool,
    /// How many times Enter asked to export (the tests count it; the save dialog it opens
    /// cannot run headless).
    pub enter_pressed: u32,
    job: Option<Job>,
    /// An export asked for while instrument samples still load (ADR 0050 §7, review 16):
    /// it starts when they are in, the window free meanwhile; the dialog shows how far the
    /// loading has got, and *Stop* drops it.
    waiting: Option<Waiting>,
    /// How the last export ended, shown in the dialog's footer until it closes.
    pub last: Option<Result<String, String>>,
}

struct Waiting {
    target: PathBuf,
    /// The choices it was asked with (the dialog's, or `export_to`'s): the export starts
    /// with them, the dialog left as it is — open or not (second review #14).
    settings: ExportSettings,
    /// Files loaded of the files asked for, last seen, and last said on the status line.
    progress: (usize, usize),
    shown: Option<(usize, usize)>,
}

struct Job {
    ctl: Arc<Control>,
    rx: mpsc::Receiver<Result<Done, String>>,
    target: PathBuf,
    /// What was asked, for the summary (stems, the loudness target).
    settings: ExportSettings,
    /// The percentage last shown in the status line.
    shown: Option<u32>,
}

impl ExportUi {
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.waiting.is_some()
    }

    /// Whether the export waits for instrument samples to load before it renders.
    pub fn waiting(&self) -> bool {
        self.waiting.is_some()
    }

    /// The running export's phase (a locale key) and how far it has got, 0…1 (every
    /// phase reports it, the render included; `None` is kept for a phase that cannot).
    pub fn progress(&self) -> Option<(&'static str, Option<f32>)> {
        if let Some(w) = &self.waiting {
            let (done, total) = w.progress;
            // Nothing counted (yet): a bar without a fraction, never "0 %".
            return Some((
                "smp.export_waiting",
                (total > 0).then(|| done as f32 / total as f32),
            ));
        }
        let j = self.job.as_ref()?;
        Some((j.ctl.phase().key(), Some(j.ctl.fraction())))
    }
}

/// The range a scope exports, if the project has it: the time selection, or the loop
/// brace; `None` for the song and the stems (and for a range that is not there).
pub fn range(app: &CantusApp, scope: Scope) -> Option<(Ticks, Ticks)> {
    match scope {
        Scope::Selection => app.time_selection,
        Scope::Loop => app
            .project()
            .loop_region
            .as_ref()
            .filter(|l| l.end > l.start)
            .map(|l| (l.start, l.end)),
        Scope::Markers { from, to } => app.span_between(
            cantus_core::tempo::MarkerId(from),
            cantus_core::tempo::MarkerId(to),
        ),
        Scope::Song | Scope::Stems => None,
    }
}

/// Whether `scope` can be exported now (a range needs to be there).
pub fn available(app: &CantusApp, scope: Scope) -> bool {
    match scope {
        Scope::Selection | Scope::Loop | Scope::Markers { .. } => range(app, scope).is_some(),
        Scope::Song | Scope::Stems => true,
    }
}

/// Open the dialog with the last choices; `scope` overrides what it exports (the File
/// menu's *Export stems…*). *Export audio…* opens on a mix: after an *Export stems…*
/// it does not open on the stems again.
pub fn open(app: &mut CantusApp, scope: Option<Scope>) {
    let mut d = app.settings.export.clone();
    d.tags.title = app.project().name.clone();
    match scope {
        Some(s) => d.scope = s,
        None if d.scope == Scope::Stems => d.scope = Scope::Song,
        None => {}
    }
    if !available(app, d.scope) {
        d.scope = Scope::Song;
    }
    app.export.dialog = Some(d);
    app.export.asked = None;
    app.export.last = None;
    app.export.fresh = true;
    // A note held on the typing piano would ring until the dialog closes: the window's
    // keys stand aside while it is open.
    for ev in app.keyboard.release_all() {
        app.apply_key_event(ev);
    }
}

/// Close the dialog; the choices made in it are kept for next time.
pub fn close(app: &mut CantusApp) {
    if let Some(d) = app.export.dialog.take() {
        remember(app, &d);
    }
    app.export.asked = None;
}

/// Keep `d` as the last choices (all but the song's title), in the settings file. The
/// loop range is kept as the song: builds out before round 23 cannot read it, and one
/// started on the same settings file would lose every setting.
pub fn remember(app: &mut CantusApp, d: &ExportSettings) {
    let mut keep = d.clone();
    keep.tags.title.clear();
    // The project's markers are the project's: kept as the song (round 29).
    if matches!(keep.scope, Scope::Loop | Scope::Markers { .. }) {
        keep.scope = Scope::Song;
    }
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

/// `path` with the extension of `format`: a name typed in the save dialog with another
/// audio format's extension gets this one's in its place, a name with none (or with a
/// dot of its own, `Take 2.final`) gets it added — the contents and the name agree.
pub fn with_format_extension(path: &Path, format: Format) -> PathBuf {
    match format_of(path) {
        Some(f) if f == format => path.to_path_buf(),
        Some(_) => path.with_extension(format.extension()),
        None => {
            let mut name = path.file_name().unwrap_or_default().to_os_string();
            name.push(".");
            name.push(format.extension());
            path.with_file_name(name)
        }
    }
}

/// The files in `folder` a stems export of `d` would replace.
fn stems_in_the_way(app: &CantusApp, d: &ExportSettings, folder: &Path) -> Vec<String> {
    let specs = crate::views::interop::stem_specs(app.project(), d.stems.tracks, d.stems.master);
    cantus_export::stems::output_names(&specs, d)
        .into_iter()
        .filter(|n| folder.join(n).exists())
        .collect()
}

/// Say why a range cannot be exported, in the dialog's note and on the status line.
fn refuse(app: &mut CantusApp, scope: Scope) {
    let key = match scope {
        Scope::Selection => "export_audio.no_selection",
        Scope::Loop => "export_audio.no_loop",
        _ if app.project().markers.len() < 2 => "tempo.export.no_markers",
        _ => "tempo.export.two_markers",
    };
    app.export.asked = Some(scope);
    app.set_error(t(key));
}

/// Ask where, then start. Stems ask for a folder; a mix for a file.
pub fn choose_and_start(app: &mut CantusApp) {
    let Some(d) = app.export.dialog.clone() else {
        return;
    };
    // Nothing to ask a file name for when the range is not there.
    if !available(app, d.scope) {
        refuse(app, d.scope);
        return;
    }
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
    let Some(path) = target else {
        return;
    };
    // The save dialog asked about the name it returned; a name whose extension is put
    // right here is another file, and is asked about here.
    let path = if d.scope == Scope::Stems {
        path
    } else {
        let right = with_format_extension(&path, d.format);
        if right != path && right.exists() {
            let name = right.file_name().unwrap_or_default().to_string_lossy();
            let ok = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title(t("export_audio.replace_file.title"))
                .set_description(tf("export_audio.replace_file.text", &[("file", &name)]))
                .set_buttons(rfd::MessageButtons::OkCancel)
                .show();
            if ok != rfd::MessageDialogResult::Ok {
                return;
            }
        }
        right
    };
    if d.scope == Scope::Stems {
        let there = stems_in_the_way(app, &d, &path);
        if !there.is_empty() {
            let names = there.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
            let ok = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title(t("export_audio.replace.title"))
                .set_description(tf(
                    "export_audio.replace.text",
                    &[("n", &there.len()), ("names", &names)],
                ))
                .set_buttons(rfd::MessageButtons::OkCancel)
                .show();
            if ok != rfd::MessageDialogResult::Ok {
                return;
            }
        }
    }
    start(app, &path);
}

/// Start exporting with the dialog's choices (or the last ones) to `target` — a file,
/// or for stems a folder. Returns whether it started.
pub fn start(app: &mut CantusApp, target: &Path) -> bool {
    start_with(app, target, None)
}

/// [`start`] with these choices (`None`: the dialog's, or the last ones) — the dialog left
/// as it is.
pub fn start_with(app: &mut CantusApp, target: &Path, choices: Option<ExportSettings>) -> bool {
    if app.export.busy() {
        return false;
    }
    // A freeze or a bounce rendering holds the hosted plug-ins' render instances (round 29,
    // ADR 0064): one set at a time — the export waits for it. A background refresh gives
    // way (review 1: it blocked every export); it is made again after.
    if !crate::freeze::yield_refresh(app) {
        app.set_error(crate::i18n::t("freeze.export_waits"));
        return false;
    }
    // A modulator's bake renders through the same plug-in instances: one render at a time
    // (round 29, review isolation-3 #5, #10, #13, #48).
    if app.modulators.pending_bake.is_some() {
        let job = crate::i18n::t("isolation.render.bake");
        app.set_error(crate::i18n::tf("isolation.render.busy", &[("job", &job)]));
        return false;
    }
    let from_dialog = choices.is_none();
    let mut d = match choices.or_else(|| app.export.dialog.clone()) {
        Some(d) => d,
        None => {
            let mut d = app.settings.export.clone();
            d.tags.title = app.project().name.clone();
            d
        }
    };
    // A range the project does not have is refused with a word, never turned into the
    // whole song under a name meant for a part of it (review 2).
    if !available(app, d.scope) {
        refuse(app, d.scope);
        return false;
    }
    let stems = d.scope == Scope::Stems;
    let target = if stems {
        target.to_path_buf()
    } else {
        with_format_extension(target, d.format)
    };
    let target = target.as_path();
    d.folder = if d.scope == Scope::Stems {
        Some(target.to_path_buf())
    } else {
        target.parent().map(Path::to_path_buf)
    };
    remember(app, &d);
    if let Some(open) = app.export.dialog.as_mut().filter(|_| from_dialog) {
        open.folder = d.folder.clone();
    }
    // Instrument samples still loading (ADR 0050 §7): the render waits for them — the
    // file holds what playing will, never a zone still on its way — and the window does
    // not (review 16).
    if app.engine.samples.loading() {
        let progress = app.engine.samples.progress().unwrap_or((0, 0));
        app.export.waiting = Some(Waiting {
            target: target.to_path_buf(),
            settings: d,
            progress,
            shown: None,
        });
        app.export.last = None;
        return true;
    }
    let rate = app.engine.sample_rate.round() as u32;
    let snapshot = app
        .engine
        .compile_for_render(app.bus.project(), app.bus.revision());
    let mut render = RenderOptions {
        sample_rate: rate,
        tail_seconds: d.tail_seconds.max(0.0) as f64,
        ..Default::default()
    };
    if let Some((from, to)) = range(app, d.scope) {
        render.start = from;
        render.end = Some(to);
    }
    let ctl = Control::new();
    let (tx, rx) = mpsc::channel();
    let tags = d.tags.clone();
    let asked = d.clone();
    // Hosted plug-ins and instruments render through instances of their own, loaded here
    // on the main thread with the live ones' state (ADR 0019, ADR 0028) — for the stems
    // as for the mix.
    let units = app.plugins.render_units(app.bus.project(), f64::from(rate));
    let instruments = app
        .plugins
        .render_instruments(app.bus.project(), f64::from(rate));
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
            units,
            instruments,
        };
        let (c, dir) = (ctl.clone(), target.to_path_buf());
        std::thread::Builder::new()
            .name("cantus-export".into())
            .spawn(move || {
                let r = cantus_export::export_stems(job, &d, &tags, &dir, &c);
                let _ = tx.send(r.map_err(error_text));
            })
    } else {
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
        app.plugins.finish_render();
        app.set_error(tf("status.export_failed", &[("error", &e)]));
        return false;
    }
    app.export.job = Some(Job {
        ctl,
        rx,
        target: target.to_path_buf(),
        settings: asked,
        shown: None,
    });
    app.export.last = None;
    app.set_status(t("status.exporting"));
    true
}

/// An error in the interface's words (the system's own reason inside); a stop is the
/// empty string.
fn error_text(e: cantus_export::ExportError) -> String {
    use cantus_export::ExportError as E;
    match e {
        E::Cancelled => String::new(),
        E::Empty => t("export_audio.error.empty"),
        E::Io(e) => tf("export_audio.error.io", &[("error", &e)]),
        E::Render(e) => tf("export_audio.error.render", &[("error", &e)]),
        E::Encode(e) => tf("export_audio.error.encode", &[("error", &e)]),
        E::TooLong => t("export_audio.error.too_long"),
    }
}

/// Ask the running export to stop.
pub fn stop(app: &mut CantusApp) {
    if app.export.waiting.take().is_some() {
        app.export.last = Some(Err(t("export_audio.stopped")));
        app.set_status(t("export_audio.stopped"));
    }
    if let Some(j) = &app.export.job {
        j.ctl.cancel();
        // An isolated plug-in hung in the render is not waited for (review #1).
        app.plugins.cancel_render();
    }
}

/// An export waiting for samples: how far they are, and the render once they are in.
fn poll_waiting(app: &mut CantusApp) {
    if app.export.waiting.is_none() {
        return;
    }
    // Take in what arrived (and send the live engine its snapshot with it).
    app.sync_engine();
    let Some(w) = app.export.waiting.as_mut() else {
        return;
    };
    if app.engine.samples.loading() {
        let now = app.engine.samples.progress().unwrap_or(w.progress);
        w.progress = now;
        if w.shown != Some(now) {
            w.shown = Some(now);
            app.set_status(if now.1 > 0 {
                tf(
                    "smp.export_waiting_status",
                    &[("done", &now.0), ("total", &now.1)],
                )
            } else {
                t("smp.export_waiting_status_plain")
            });
        }
        return;
    }
    let Some(w) = app.export.waiting.take() else {
        return;
    };
    // The samples are in (the live engine has them too): the export starts with the
    // choices it was asked with; the dialog stays as it is (second review #14).
    start_with(app, &w.target, Some(w.settings));
}

/// See whether the export has finished (every frame); while it runs, the status line
/// says how far it has got.
pub fn poll(app: &mut CantusApp) {
    poll_waiting(app);
    let Some(j) = &mut app.export.job else { return };
    let result = match j.rx.try_recv() {
        Ok(r) => r,
        Err(mpsc::TryRecvError::Empty) => {
            let pct = (j.ctl.fraction() * 100.0).floor() as u32;
            // An error said elsewhere (a plug-in that failed) stays readable for a while
            // before the progress takes the line back.
            let quiet = !app.status.error || app.status.at.elapsed().as_secs() >= 6;
            if j.shown != Some(pct) && quiet {
                j.shown = Some(pct);
                let phase = t(j.ctl.phase().key());
                let text = tf("export_audio.progress", &[("phase", &phase), ("pct", &pct)]);
                app.set_status(text);
            }
            return;
        }
        Err(mpsc::TryRecvError::Disconnected) => Err(t("export_audio.error.died")),
    };
    finish(app, result);
}

/// Wait for the running export (tests; the interface polls instead).
pub fn wait(app: &mut CantusApp) {
    if let Some(j) = &app.export.job {
        let r =
            j.rx.recv()
                .unwrap_or_else(|_| Err(t("export_audio.error.died")));
        finish(app, r);
    }
}

fn finish(app: &mut CantusApp, result: Result<Done, String>) {
    let Some(j) = app.export.job.take() else {
        return;
    };
    // A plug-in that stopped working during the render (round 29, review #6, #47): the
    // file lacks it from that point, so the export is not reported done.
    let lost = app.plugins.render_failures();
    // The render is over: the plug-in instances it used go, on this thread.
    app.plugins.finish_render();
    // One never loaded for it lacks it throughout; one begun from its last-saved state is
    // said, not failed (review isolation-4 #10, #41).
    let (losses, warnings) = super::plugin_notice::render_words(&lost, "export", "");
    match result {
        Ok(_) if losses.is_some() => {
            let text = losses.unwrap_or_default();
            app.export.last = Some(Err(text.clone()));
            app.set_error(text);
        }
        Ok(done) => {
            let mut text = summary(&done, &j.target, &j.settings);
            if let Some(w) = &warnings {
                text = format!("{text} {w}");
            }
            app.export.last = Some(Ok(text.clone()));
            if done.unaligned {
                app.set_error(format!("{text} {}", t("interop.stems.unaligned")));
            } else if warnings.is_some() {
                app.set_error(text);
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

/// What a finished export says: the file (or how many stems), its length and its level,
/// and what did not go as asked — a loudness target missed (and why), samples clipped.
pub fn summary(done: &Done, target: &Path, asked: &ExportSettings) -> String {
    let mut text = written(done, target, asked);
    let mut add = |line: String| {
        text.push(' ');
        text.push_str(&line);
    };
    if let (true, Some(goal)) = (asked.normalises(), asked.loudness.target_lufs) {
        let goal_text = format!("{goal:.0}");
        match &done.loudness {
            Some(n) if n.measured_lufs.is_none() => add(t("export_audio.loudness.unmeasured")),
            Some(n) if n.lufs.is_some_and(|l| (l - goal as f64).abs() > 0.5) => {
                let key = if n.gain_db >= cantus_export::process::MAX_GAIN_DB - 0.01 {
                    "export_audio.loudness.missed_cap"
                } else if !asked.loudness.limit {
                    "export_audio.loudness.missed_ceiling"
                } else {
                    "export_audio.loudness.missed"
                };
                add(tf(key, &[("target", &goal_text)]));
            }
            _ => {}
        }
    }
    if done.clipped > 0 {
        add(tf("export_audio.clipped", &[("n", &done.clipped)]));
    }
    if done.cut_notes > 0 {
        add(crate::i18n::tf_count(
            "smp.cut_notes_export",
            done.cut_notes as usize,
            &[],
        ));
    }
    text
}

/// The first line of the summary: what was written.
fn written(done: &Done, target: &Path, asked: &ExportSettings) -> String {
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
    if asked.scope == Scope::Stems {
        let key = if done.files.len() == 1 {
            "export_audio.done_stem_one"
        } else {
            "export_audio.done_stems"
        };
        return tf(
            key,
            &[
                ("n", &done.files.len()),
                ("folder", &target.display()),
                ("silent", &done.silent.len()),
            ],
        );
    }
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

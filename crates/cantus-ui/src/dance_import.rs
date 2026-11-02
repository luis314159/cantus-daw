//! *Import character…* (round 35, ADR 0089): a file is checked off the interface's thread,
//! a card shows it dancing with its licence and the mapping of our sixteen moves onto its
//! animations, and *Use* copies it into the project's folder and makes it the dancer's
//! character in one undo step. *Edit moves…* reopens the card for a character already
//! imported; what it saves applies to every dancer that uses that character.
//!
//! Called from elsewhere (each by one marked line): [`frame`] once a frame (`app.rs`),
//! [`open`] from the dancer's picker, [`edit_moves`] from the dancer's window,
//! [`collect_on_save`] before a save writes (`save_job.rs`), [`project_dir`] wherever a
//! `file:` character is loaded (`cantus_dance::import::load(project_dir, rel)`).

#[path = "views/dance_import.rs"]
pub mod view;

use crate::app::CantusApp;
use crate::dance_texture::DanceTexture;
use crate::i18n::{t, tf};
use cantus_core::inst::dancer::CHARACTER_KEY;
use cantus_core::{Command, TrackId};
use cantus_dance::import::{self, package, Found, ImportError, Imported, Note, Settings};
use cantus_dance::{Character, Licence};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

/// The file dialog's extensions.
pub const EXTENSIONS: [&str; 2] = ["json", "lottie"];

/// What the import is doing.
#[derive(Default)]
pub struct DanceImport {
    checking: Option<Checking>,
    pub card: Option<Card>,
    pub failed: Option<Failed>,
}

/// A copy being made off this thread (*Use*): the card waits, saying so. Tied to the
/// project it was asked in (its epoch) and the folder it copies into: a copy that can no
/// longer be used is removed (review 2).
struct Using {
    rx: Receiver<Result<(String, bool), ImportError>>,
    epoch: u64,
    dir: PathBuf,
}

impl Using {
    /// Give the copy up: once it lands, a folder it made is removed (on its own thread, so
    /// nothing here waits for it).
    fn abandon(self) {
        let Using { rx, dir, .. } = self;
        let _ = std::thread::Builder::new()
            .name("cantus-dance-import-abandon".into())
            .spawn(move || {
                if let Ok(Ok((rel, true))) = rx.recv() {
                    remove_copy(&dir, &rel);
                }
            });
    }
}

/// Remove the folder a copy made (`rel`'s, under `dir`), never anything outside it.
fn remove_copy(dir: &Path, rel: &str) {
    if let Some(folder) = package::folder_of(rel) {
        if let Ok(p) = cantus_dance::import::paths::resolve(dir, &folder) {
            let _ = std::fs::remove_dir_all(p);
        }
    }
}

/// Whether `track` is a dancer of the project now.
fn is_dancer(app: &CantusApp, track: TrackId) -> bool {
    app.project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .is_some_and(|n| n.kind == cantus_core::InstrumentKind::Dancer)
}

struct Checking {
    epoch: u64,
    track: TrackId,
    path: PathBuf,
    edit: Option<String>,
    rx: Receiver<Result<Found, ImportError>>,
}

/// A file that could not be read: said in words, with what to do.
pub struct Failed {
    pub track: TrackId,
    pub path: PathBuf,
    pub key: &'static str,
    pub detail: String,
}

/// The card: the file read, the choices made on it.
pub struct Card {
    pub track: TrackId,
    pub path: PathBuf,
    /// `Some(key path)` when editing the moves of a character already in the project.
    pub edit: Option<String>,
    pub format: &'static str,
    pub name: String,
    pub author: String,
    /// What the file states (`Unknown`: the person chooses below).
    pub stated: Licence,
    /// The person's choice from `package::CHOICES`, and the words of *other*.
    pub choice: Option<usize>,
    pub other: String,
    pub notes: Vec<Note>,
    pub facts: Vec<(&'static str, String)>,
    pub files: Vec<String>,
    pub clips: Vec<import::Clip>,
    pub settings: Settings,
    /// The character as chosen, dancing in the preview.
    pub preview: Imported,
    pub cost_ms: Option<f64>,
    pub texture: DanceTexture,
    pub opened: Instant,
    /// The move shown in the preview (the row under the pointer), else the routine.
    pub showing: Option<cantus_dance::Move>,
    /// *Use* in progress: the copy is being made.
    using: Option<Using>,
    /// The project it was opened in: another one coming in closes it.
    epoch: u64,
    /// The cost measured off this thread (a few frames of its routine).
    cost_rx: Option<Receiver<f64>>,
}

impl Card {
    /// The licence kept with the copy: what the file states, else the person's choice.
    pub fn licence(&self) -> Licence {
        if self.stated != Licence::Unknown {
            return self.stated.clone();
        }
        match self.choice.and_then(|i| package::CHOICES.get(i)) {
            Some(("other", _)) if !self.other.trim().is_empty() => Licence::Stated {
                text: self.other.trim().chars().take(200).collect(),
                source: package::CHOSEN.into(),
            },
            Some(("other", _)) | None => Licence::Unknown,
            Some((id, _)) => Licence::Stated {
                text: (*id).to_string(),
                source: package::CHOSEN.into(),
            },
        }
    }

    /// Change the choices: the preview follows.
    pub fn set_settings(&mut self, s: Settings) {
        self.settings = s.clone();
        self.preview.set_settings(s);
    }

    /// Whether *Use* is copying the file now.
    pub fn busy(&self) -> bool {
        self.using.is_some()
    }

    /// The cost, once its measure is in (polled each frame).
    pub fn poll_cost(&mut self) {
        if let Some(rx) = &self.cost_rx {
            match rx.try_recv() {
                Ok(ms) => {
                    self.cost_ms = Some(ms);
                    self.cost_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.cost_rx = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }
}

/// The folder `file:` characters resolve against: the project's, or — untitled — the
/// folder Cantus keeps them in until the first save carries them beside the project.
pub fn project_dir(app: &CantusApp) -> PathBuf {
    match app.audio_base() {
        Some(p) => p.to_path_buf(),
        None => import::untitled_dir(&crate::app::data_dir()),
    }
}

/// *Import character…* for the dancer on `track`: choose a file, then the card.
pub fn open(app: &mut CantusApp, track: TrackId) {
    let picked = rfd::FileDialog::new()
        .set_title(t("dance_import.dialog.title"))
        .add_filter(t("dance_import.dialog.filter"), &EXTENSIONS)
        .pick_file();
    if let Some(path) = picked {
        begin(app, track, path, None);
    }
}

/// Check `path` off this thread; the card opens when it is read (or says why not).
pub fn begin(app: &mut CantusApp, track: TrackId, path: PathBuf, edit: Option<String>) {
    let (tx, rx) = mpsc::channel();
    let p = path.clone();
    let spawned = std::thread::Builder::new()
        .name("cantus-dance-import".into())
        .spawn(move || {
            let _ = tx.send(import::inspect(&p));
        });
    if spawned.is_err() {
        app.set_error(t("dance_import.status.busy"));
        return;
    }
    // A card being replaced is closed properly: a copy in flight is abandoned and its
    // folder removed (review 3).
    close(app);
    app.dance_import.checking = Some(Checking {
        epoch: app.epoch,
        track,
        path,
        edit,
        rx,
    });
}

/// The `file:` key path of `track`'s character, if it is an imported one.
pub fn imported_key(app: &CantusApp, track: TrackId) -> Option<String> {
    let n = app.project().track(track)?.instrument.native()?;
    n.text(CHARACTER_KEY)?
        .strip_prefix("file:")
        .map(str::to_string)
}

/// *Edit moves…*: the card again for `track`'s imported character, its choices loaded.
pub fn edit_moves(app: &mut CantusApp, track: TrackId) {
    let Some(rel) = imported_key(app, track) else {
        app.set_status(t("dance_import.status.not_imported"));
        return;
    };
    match cantus_dance::import::paths::resolve(&project_dir(app), &rel) {
        Ok(path) => begin(app, track, path, Some(rel)),
        Err(e) => app.set_error(tf("dance_import.status.failed", &[("error", &e.detail())])),
    }
}

/// Whether the import is waiting for a file to be read.
pub fn checking(app: &CantusApp) -> bool {
    app.dance_import.checking.is_some()
}

/// A finished check becomes the card (or the words of why not). Each frame; also by
/// tests that wait for it.
pub fn poll(app: &mut CantusApp, wait: bool) {
    // Another project came in (New, Open, a recovery, an agent's load): the card, a check
    // and a copy of the one before go — a copy made for it is removed.
    if app
        .dance_import
        .card
        .as_ref()
        .is_some_and(|c| c.epoch != app.epoch)
    {
        close(app);
    }
    if app
        .dance_import
        .checking
        .as_ref()
        .is_some_and(|c| c.epoch != app.epoch)
    {
        app.dance_import.checking = None;
    }
    finish_use(app, wait);
    if let Some(card) = app.dance_import.card.as_mut() {
        card.poll_cost();
    }
    let Some(c) = app.dance_import.checking.take() else {
        return;
    };
    let got = if wait {
        c.rx.recv().ok()
    } else {
        match c.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => {
                app.dance_import.checking = Some(c);
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => None,
        }
    };
    let result = got.unwrap_or(Err(ImportError::Unsupported));
    match result {
        Ok(found) => {
            let card = card_of(app, c.track, c.path, c.edit, found);
            app.dance_import.card = Some(card);
        }
        Err(e) => {
            app.dance_import.failed = Some(Failed {
                track: c.track,
                path: c.path,
                key: e.label_key(),
                detail: e.detail(),
            })
        }
    }
}

fn card_of(
    app: &CantusApp,
    track: TrackId,
    path: PathBuf,
    edit: Option<String>,
    found: Found,
) -> Card {
    // Editing: the choices kept in `character.json` beside the copy.
    let kept = edit.as_ref().and_then(|_| {
        let dir = path.parent()?;
        package::read_package(dir).ok()
    });
    let settings = kept
        .as_ref()
        .map(|p| p.settings.clone())
        .unwrap_or_else(|| import::guessed(&found));
    let stated = match &kept {
        Some(p) => p.licence.licence(),
        None => found.licence.clone(),
    };
    let name = kept
        .as_ref()
        .map(|p| p.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| found.name.clone());
    let id = match &edit {
        Some(rel) => format!("file:{rel}"),
        None => format!("file:{}", path.display()),
    };
    let format = found.source.format();
    let clips = found.source.clips().to_vec();
    let (author, notes, facts, files) = (
        found.author.clone(),
        found.notes.clone(),
        found.facts.clone(),
        found.files.clone(),
    );
    let (path_for_cost, settings_for_cost) = (path.clone(), settings.clone());
    let preview = import::character(found, id, settings.clone());
    Card {
        track,
        path,
        edit,
        format,
        name,
        author,
        stated,
        choice: None,
        other: String::new(),
        notes,
        facts,
        files,
        clips,
        settings,
        preview,
        cost_ms: None,
        texture: DanceTexture::new("cantus-dance-import-preview"),
        opened: Instant::now(),
        showing: None,
        using: None,
        epoch: app.epoch,
        cost_rx: measure_off_thread(&path_for_cost, settings_for_cost),
    }
}

/// What the character costs to draw, measured on a thread of its own (the file read again
/// there with the card's choices; the preview stays the interface's).
fn measure_off_thread(path: &Path, settings: Settings) -> Option<Receiver<f64>> {
    let (tx, rx) = mpsc::channel();
    let p = path.to_path_buf();
    std::thread::Builder::new()
        .name("cantus-dance-import-cost".into())
        .spawn(move || {
            if let Ok(found) = import::inspect(&p) {
                let ch = import::character(found, "file:cost".into(), settings);
                let _ = tx.send(import::cost_ms(&ch, 24));
            }
        })
        .ok()?;
    Some(rx)
}

/// *Use* (or, editing, *Save*). Editing writes the moves for every dancer using the
/// character (a small file) and closes the card. Importing reads the file again and copies
/// it on a thread — the card says so and waits — then [`poll`] sets the key in one undo step
/// and closes it. `true` when it was done or started.
pub fn use_card(app: &mut CantusApp) -> bool {
    let dir = project_dir(app);
    let Some(card) = app.dance_import.card.as_mut() else {
        return false;
    };
    if card.using.is_some() {
        return true;
    }
    if let Some(rel) = card.edit.clone() {
        let name = card.name.clone();
        return match package::with_settings(&dir, &rel, &card.settings) {
            Ok(new_rel) => {
                app.dance_import.card = None;
                rekey(app, &[(rel, new_rel)], "cmd.edit_character_moves");
                app.set_status(tf("dance_import.status.moves_saved", &[("name", &name)]));
                true
            }
            Err(e) => {
                app.set_error(tf("dance_import.status.failed", &[("error", &e.detail())]));
                false
            }
        };
    }
    let epoch = app.epoch;
    let (path, name, settings, licence) = (
        card.path.clone(),
        card.name.clone(),
        card.settings.clone(),
        card.licence(),
    );
    let (tx, rx) = mpsc::channel();
    let into = dir.clone();
    let spawned = std::thread::Builder::new()
        .name("cantus-dance-import-copy".into())
        .spawn(move || {
            let result = import::inspect(&path).and_then(|f| {
                let found = Found { name, ..f };
                std::fs::create_dir_all(&into)?;
                package::copy_into_new(&into, &path, &found, &settings, &licence)
            });
            let _ = tx.send(result);
        });
    if spawned.is_err() {
        app.set_error(t("dance_import.status.busy"));
        return false;
    }
    let Some(card) = app.dance_import.card.as_mut() else {
        return false;
    };
    card.using = Some(Using { rx, epoch, dir });
    true
}

/// A finished copy: the key set in one undo step, the card closed (or the error said).
/// For a project that is no longer the one it was asked in, or a track that is no longer a
/// dancer, the copy is removed (when it made its folder) and nothing is set.
fn finish_use(app: &mut CantusApp, wait: bool) {
    let Some(card) = app.dance_import.card.as_mut() else {
        return;
    };
    let Some(using) = card.using.as_ref() else {
        return;
    };
    let got = if wait {
        using.rx.recv().ok()
    } else {
        match using.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => None,
        }
    };
    let Some(using) = card.using.take() else {
        return;
    };
    let (track, name) = (card.track, card.name.clone());
    let (rel, created) = match got.unwrap_or(Err(ImportError::Unsupported)) {
        Ok(r) => r,
        Err(e) => {
            app.set_error(tf("dance_import.status.failed", &[("error", &e.detail())]));
            return;
        }
    };
    if using.epoch != app.epoch || !is_dancer(app, track) {
        if created {
            remove_copy(&using.dir, &rel);
        }
        app.dance_import.card = None;
        app.set_error(t("dance_import.status.not_a_dancer"));
        return;
    }
    // The project moved folders while it copied (*Save As*, an untitled project's first
    // save): the copy is carried to where the project is now before its key is set.
    let now = project_dir(app);
    let rel = if now == using.dir {
        rel
    } else {
        match package::carry(&using.dir, &now, std::slice::from_ref(&rel)) {
            Ok(c) if c.missing.is_empty() => c
                .renamed
                .into_iter()
                .find(|(old, _)| *old == rel)
                .map(|(_, new)| new)
                .unwrap_or(rel),
            Ok(_) => {
                app.set_error(tf("dance_import.status.failed", &[("error", &rel)]));
                return;
            }
            Err(e) => {
                app.set_error(tf("dance_import.status.failed", &[("error", &e.detail())]));
                return;
            }
        }
    };
    let done = app.exec(Command::Batch {
        label_key: "cmd.use_character".into(),
        commands: vec![Command::SetInstrumentText {
            track,
            key: CHARACTER_KEY.into(),
            text: Some(format!("file:{rel}")),
        }],
    });
    if done {
        app.dance_import.card = None;
        app.set_status(tf("dance_import.status.used", &[("name", &name)]));
    } else {
        if created {
            remove_copy(&using.dir, &rel);
        }
        app.set_error(t("dance_import.status.not_a_dancer"));
    }
}

/// Close the card (or the words of a failure) without doing anything.
pub fn close(app: &mut CantusApp) {
    if let Some(using) = app.dance_import.card.take().and_then(|c| c.using) {
        using.abandon();
    }
    app.dance_import.failed = None;
}

/// The `file:` key paths every dancer of the project uses.
pub fn keys_in_use(app: &CantusApp) -> Vec<String> {
    let mut out: Vec<String> = app
        .project()
        .tracks
        .iter()
        .filter_map(|tr| tr.instrument.native())
        .filter_map(|n| n.text(CHARACTER_KEY))
        .filter_map(|k| k.strip_prefix("file:"))
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Before a save writes the project to `file`: every imported character its dancers use
/// is carried into the folder it is saved in (an untitled project's first save, *Save As*
/// into another folder) — the keys never change. `false` when the carry was refused (that
/// folder holds another version of one of them — review 2) or failed: the save stops and
/// says why, and what waited on it (a *Save changes?* errand, a queued save) is dropped.
pub fn collect_on_save(app: &mut CantusApp, file: &Path) -> bool {
    let keys = keys_in_use(app);
    if keys.is_empty() {
        return true;
    }
    let from = project_dir(app);
    let Some(to) = file.parent().map(Path::to_path_buf) else {
        return true;
    };
    match package::carry(&from, &to, &keys) {
        Ok(carried) => {
            if !carried.missing.is_empty() {
                app.set_error(tf(
                    "dance_import.status.missing",
                    &[("folders", &carried.missing.join(", "))],
                ));
            }
            // A folder of that name there held other content (a hand edited it): ours went
            // to a fresh name and its dancers follow, in one step.
            rekey(app, &carried.renamed, "cmd.carry_characters");
            true
        }
        Err(e) => {
            app.saving.then = None;
            let dropped = app.saving.queued.take();
            let words = match dropped {
                Some(q) => tf(
                    "dance_import.status.carry_failed_queued",
                    &[("error", &e.detail()), ("file", &q.path.display())],
                ),
                None => tf(
                    "dance_import.status.carry_failed",
                    &[("error", &e.detail())],
                ),
            };
            app.set_error(words);
            false
        }
    }
}

/// Point this project's dancers wearing each old key path at its new one, in one undo step
/// (`label`); its inverse points them back — the old folder still exists, never edited.
pub fn rekey(app: &mut CantusApp, moves: &[(String, String)], label: &str) {
    let commands: Vec<Command> = app
        .project()
        .tracks
        .iter()
        .filter_map(|tr| {
            let k = tr
                .instrument
                .native()?
                .text(CHARACTER_KEY)?
                .strip_prefix("file:")?;
            let (_, new) = moves.iter().find(|(old, new)| old == k && old != new)?;
            Some(Command::SetInstrumentText {
                track: tr.id,
                key: CHARACTER_KEY.into(),
                text: Some(format!("file:{new}")),
            })
        })
        .collect();
    if !commands.is_empty() {
        app.exec(Command::Batch {
            label_key: label.into(),
            commands,
        });
    }
}

/// One character imported in the project, for the dancer's picker.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    /// Its key path (`Characters/<name>/<file>`): the dancer's `character` is `file:` + it.
    pub rel: String,
    pub name: String,
    pub format: String,
    pub licence: Licence,
}

/// Every character in the project's `Characters/` folder.
pub fn imported_in_project(app: &CantusApp) -> Vec<Listed> {
    listed_in(&project_dir(app))
}

/// The characters in `<dir>/Characters/*/character.json`, by name.
pub fn listed_in(dir: &Path) -> Vec<Listed> {
    let Ok(entries) = std::fs::read_dir(dir.join(package::FOLDER)) else {
        return Vec::new();
    };
    let mut out: Vec<Listed> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let p = package::read_package(&e.path()).ok()?;
            let folder = e.file_name().into_string().ok()?;
            Some(Listed {
                rel: format!("{}/{folder}/{}", package::FOLDER, p.file),
                name: p.name,
                format: p.format,
                licence: p.licence.licence(),
            })
        })
        .collect();
    out.sort_by_key(|l| l.name.to_lowercase());
    out
}

/// Once a frame: the check's answer, then the card.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    poll(app, false);
    let busy = app
        .dance_import
        .card
        .as_ref()
        .is_some_and(|c| c.busy() || c.cost_rx.is_some());
    if app.dance_import.checking.is_some() || busy {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    view::show(app, ctx);
}

/// The character the card shows, for the shoots and tests.
pub fn preview(app: &CantusApp) -> Option<&dyn Character> {
    app.dance_import
        .card
        .as_ref()
        .map(|c| &c.preview as &dyn Character)
}

/// The dancer a menu's *Import character…* is for: the selected track, if it is a dancer.
pub fn selected_dancer(app: &CantusApp) -> Option<TrackId> {
    let track = app.selected_track?;
    let n = app.project().track(track)?.instrument.native()?;
    (n.kind == cantus_core::InstrumentKind::Dancer).then_some(track)
}

/// *File ▸ Import character…*: for the selected dancer (dimmed, with a word why, for
/// any other track).
pub fn file_menu_item(app: &mut CantusApp, ui: &mut egui::Ui) {
    let dancer = selected_dancer(app);
    let r = ui.add_enabled(dancer.is_some(), egui::Button::new(t("dance_import.menu")));
    let r = if dancer.is_none() {
        r.on_disabled_hover_text(t("dance_import.menu.needs_dancer"))
    } else {
        r
    };
    if r.clicked() {
        ui.close();
        menu_pick(app);
    }
}

/// *Import character…* chosen from a menu (the window's File menu, macOS's menu bar): for
/// the selected dancer; with none, the status says what to do (AppKit's items cannot be
/// dimmed per frame, so the Mac's says it in words).
pub fn menu_pick(app: &mut CantusApp) {
    match selected_dancer(app) {
        Some(track) => open(app, track),
        None => app.set_status(t("dance_import.menu.needs_dancer")),
    }
}

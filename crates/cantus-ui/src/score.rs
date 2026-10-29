//! The score (round 32, ADR 0075): its view state, the engraving cached per edit and
//! size, the track's settings as commands, MusicXML and PDF out, MusicXML in as a clip,
//! printing, and the edits made on the staff — every one a `Command` on the notes.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::notation::ScoreSettings;
use cantus_core::{ClipId, Command, Note, NoteId, Ticks, TrackId};
use cantus_score::build::{build, PartSource, Song};
use cantus_score::layout::{engrave, Engraved, Format, LayoutOptions, Paper};
use cantus_score::text::TextMeasure;
use cantus_score::{model::Dur, Score};
use std::path::{Path, PathBuf};

/// What a score shows: a clip's part, or a whole track's.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Clip(TrackId, ClipId),
    Track(TrackId),
    /// Every note track, one part each (the song's score: MusicXML and PDF only).
    Song,
}

/// An engraving kept until something it depends on changes.
pub struct Cached {
    key: (u64, Scope, u32, u32, String),
    pub score: Score,
    pub engraved: Engraved,
}

/// Entering notes on the staff, MuseScore's way: a value chosen by number, a note by its
/// letter, the cursor moving on.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Entry {
    /// Where the next note goes (timeline tick).
    pub at: i64,
    pub dur: Dur,
    /// The last pitch entered: a letter takes the octave nearest it.
    pub last: u8,
}

/// The score's view state.
pub struct ScoreUi {
    /// The staff under the piano roll is shown.
    pub split_open: bool,
    /// Its height in points at 100 %.
    pub split_h: f32,
    /// Points per staff space at 100 %: the split's and the pane's.
    pub split_space: f32,
    pub pane_space: f32,
    /// The split scrolls sideways (one long system), the pane down (wrapped systems).
    pub split_scroll: f32,
    pub pane_scroll: f32,
    /// Keep the playhead in view while playing.
    pub follow: bool,
    /// The last place the score was clicked: it has the keys (arrows, letters, Delete).
    pub focused: bool,
    pub entry: Option<Entry>,
    pub split_cache: Option<Cached>,
    pub pane_cache: Option<Cached>,
    /// The last file exported (said in the status line).
    pub last_file: Option<PathBuf>,
    /// The pass a score was last drawn in: its keys work only while it is on screen.
    pub drawn_pass: u64,
    /// The pointer is over a score's toolbar (a press there keeps the keys).
    pub bar_hovered: bool,
    /// The part's name while it is being typed.
    pub name_edit: Option<String>,
    /// A print on its way: what it did comes back here (`poll_print`).
    pub printing: Option<std::sync::mpsc::Receiver<&'static str>>,
}

impl Default for ScoreUi {
    fn default() -> Self {
        ScoreUi {
            split_open: false,
            split_h: 210.0,
            split_space: 7.0,
            pane_space: 7.0,
            split_scroll: 0.0,
            pane_scroll: 0.0,
            follow: true,
            focused: false,
            entry: None,
            split_cache: None,
            pane_cache: None,
            last_file: None,
            drawn_pass: 0,
            bar_hovered: false,
            name_edit: None,
            printing: None,
        }
    }
}

/// The smallest and largest staff space, points.
pub const SPACE_RANGE: (f32, f32) = (3.0, 20.0);

/// The text faces the score's words are measured and set in.
pub fn measure() -> TextMeasure {
    TextMeasure::new(Some(crate::design::score_faces()))
}

/// The parts a scope writes.
pub fn parts(app: &CantusApp, scope: Scope) -> Vec<PartSource> {
    let p = app.project();
    match scope {
        Scope::Clip(t, c) => p
            .track(t)
            .and_then(|tr| {
                tr.clip(c)
                    .map(|cl| vec![cantus_score::source::clip_part(tr, cl)])
            })
            .unwrap_or_default(),
        Scope::Track(t) => p
            .track(t)
            .map(|tr| vec![cantus_score::source::track_part(tr)])
            .unwrap_or_default(),
        Scope::Song => p
            .tracks
            .iter()
            .filter(|tr| !tr.clips.iter().all(|c| c.notes.is_empty()))
            .map(cantus_score::source::track_part)
            .collect(),
    }
}

/// The range a clip's score covers (its window, to whole bars); a track's and the song's
/// cover their notes.
fn range(app: &CantusApp, scope: Scope) -> Option<(i64, i64)> {
    match scope {
        Scope::Clip(t, c) => {
            let clip = app.project().track(t)?.clip(c)?;
            Some((clip.start.0, clip.end().0))
        }
        _ => None,
    }
}

/// The logical score of a scope.
pub fn score_of(app: &CantusApp, scope: Scope) -> Score {
    let p = app.project();
    let tempo = p.tempo_map();
    let song = Song {
        title: p.name.clone(),
        composer: app.settings.export.tags.artist.clone(),
        tempo: &tempo,
        key: p.key,
    };
    build(&song, &parts(app, scope), range(app, scope))
}

/// The engraving of `scope` in `format`, from `cache` when nothing changed.
pub fn engraved<'a>(
    app: &CantusApp,
    cache: &'a mut Option<Cached>,
    scope: Scope,
    format: Format,
) -> &'a Cached {
    let (a, b) = match format {
        Format::Continuous => (0, 0),
        Format::Wrap { width } => ((width * 4.0).round() as u32, 1),
        Format::Pages(p) => ((p.width_mm * 10.0) as u32, 2),
    };
    let key = (
        app.bus.revision(),
        scope,
        a,
        b,
        app.settings.export.tags.artist.clone(),
    );
    if cache.as_ref().is_none_or(|c| c.key != key) {
        let score = score_of(app, scope);
        let engraved = engrave(
            &score,
            &LayoutOptions {
                format,
                text: measure(),
                header: matches!(format, Format::Pages(_)),
            },
        );
        *cache = Some(Cached {
            key,
            score,
            engraved,
        });
    }
    cache.as_ref().expect("filled above")
}

/// A file name from a scope's name.
fn file_name(app: &CantusApp, scope: Scope, ext: &str) -> String {
    let p = app.project();
    let base = match scope {
        Scope::Clip(t, c) => p.track(t).and_then(|tr| tr.clip(c)).map(|c| c.name.clone()),
        Scope::Track(t) => p.track(t).map(|tr| tr.name.clone()),
        Scope::Song => None,
    }
    .filter(|n| !n.trim().is_empty())
    .unwrap_or_else(|| p.name.clone());
    let clean: String = base
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    format!("{clean}.{ext}")
}

/// MusicXML of a scope.
pub fn musicxml(app: &CantusApp, scope: Scope) -> String {
    let score = score_of(app, scope);
    cantus_score::musicxml::write(
        &score,
        &cantus_score::musicxml::XmlOptions {
            software: format!("Cantus {}", env!("CARGO_PKG_VERSION")),
            date: None,
        },
    )
}

/// PDF of a scope on `paper`.
pub fn pdf(app: &CantusApp, scope: Scope, paper: Paper) -> Result<Vec<u8>, String> {
    let score = score_of(app, scope);
    let e = engrave(
        &score,
        &LayoutOptions {
            format: Format::Pages(paper),
            text: measure(),
            header: true,
        },
    );
    cantus_score::pdf::write(
        &e,
        &paper,
        crate::design::score_faces(),
        &score.title,
        &score.composer,
    )
    .map_err(|e| e.to_string())
}

/// The song's key a MusicXML key names: its fifths (concert) and mode.
fn key_of(fifths: i8, mode: cantus_score::model::KeyMode) -> Option<cantus_core::Key> {
    use cantus_core::ScaleKind;
    use cantus_score::model::KeyMode as M;
    let major = (fifths as i32 * 7).rem_euclid(12);
    let (offset, scale) = match mode {
        M::Major => (0, ScaleKind::Major),
        M::Minor => (9, ScaleKind::Minor),
        M::Dorian => (2, ScaleKind::Dorian),
        M::Phrygian => (4, ScaleKind::Phrygian),
        M::Lydian => (5, ScaleKind::Lydian),
        M::Mixolydian => (7, ScaleKind::Mixolydian),
        M::Locrian => (11, ScaleKind::Locrian),
        M::None => return None,
    };
    Some(cantus_core::Key::new(((major + offset) % 12) as u8, scale))
}

/// The paper a region prints on: Letter in the United States and Canada and the
/// American countries that follow them (and the Philippines), A4 everywhere else. Takes a
/// locale as Windows (`en-US`) or POSIX (`es_MX.UTF-8`) writes it.
pub fn paper_for_region(locale: &str) -> Paper {
    let region = locale
        .split(['.', '@'])
        .next()
        .unwrap_or("")
        .rsplit(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    const LETTER: [&str; 11] = [
        "US", "CA", "MX", "PH", "CL", "CO", "VE", "GT", "PR", "PA", "CR",
    ];
    if locale.contains(['-', '_']) && LETTER.contains(&region.as_str()) {
        Paper::LETTER
    } else {
        Paper::A4
    }
}

/// The locale in `reg query "HKCU\Control Panel\International" /v LocaleName`'s output.
pub fn region_of_reg_output(out: &str) -> Option<String> {
    out.lines()
        .find(|l| l.trim_start().starts_with("LocaleName"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string)
}

/// The user's locale for paper: Windows' *Region* setting (its environment has no `LANG`),
/// read once; elsewhere `LC_PAPER`, `LC_ALL` or `LANG` (review, finding 9).
fn user_locale() -> String {
    static L: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    L.get_or_init(|| {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            std::process::Command::new("reg")
                .args([
                    "query",
                    r"HKCU\Control Panel\International",
                    "/v",
                    "LocaleName",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok()
                .and_then(|o| region_of_reg_output(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or_default()
        }
        #[cfg(not(windows))]
        {
            ["LC_PAPER", "LC_ALL", "LANG"]
                .iter()
                .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
                .unwrap_or_default()
        }
    })
    .clone()
}

/// The paper the user's region prints on.
pub fn default_paper() -> Paper {
    paper_for_region(&user_locale())
}

impl CantusApp {
    /// A track's score settings, as one undo step each (review, finding 5: a merge key
    /// left open collapsed separate changes into one; the name is set once, on Enter).
    pub fn set_track_score(&mut self, track: TrackId, settings: ScoreSettings) {
        self.exec(Command::SetTrackScore { track, settings });
    }

    /// *Export MusicXML…*: the file dialog, then the file.
    pub fn export_musicxml(&mut self, scope: Scope) {
        let name = file_name(self, scope, "musicxml");
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MusicXML", &["musicxml", "xml"])
            .set_file_name(name)
            .save_file()
        {
            self.export_musicxml_to(scope, &path);
        }
    }

    pub fn export_musicxml_to(&mut self, scope: Scope, path: &Path) -> bool {
        let xml = musicxml(self, scope);
        self.finish_export(path, std::fs::write(path, xml).map_err(|e| e.to_string()))
    }

    /// *Export PDF…*: the file dialog, then the file.
    pub fn export_pdf(&mut self, scope: Scope, paper: Paper) {
        let name = file_name(self, scope, "pdf");
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name(name)
            .save_file()
        {
            self.export_pdf_to(scope, paper, &path);
        }
    }

    pub fn export_pdf_to(&mut self, scope: Scope, paper: Paper, path: &Path) -> bool {
        let r = pdf(self, scope, paper)
            .and_then(|b| std::fs::write(path, b).map_err(|e| e.to_string()));
        self.finish_export(path, r)
    }

    fn finish_export(&mut self, path: &Path, r: Result<(), String>) -> bool {
        match r {
            Ok(()) => {
                self.set_status(tf(
                    "score.status.exported",
                    &[("file", &path.display().to_string())],
                ));
                self.score.last_file = Some(path.to_path_buf());
                true
            }
            Err(e) => {
                self.set_error(tf("score.status.export_failed", &[("error", &e)]));
                false
            }
        }
    }

    /// *Print…*: the score as a PDF in the system's temporary folder, handed to the system's
    /// printing for PDFs (`crate::score::print_file` says how on each system).
    pub fn print_score(&mut self, scope: Scope) {
        let name = file_name(self, scope, "pdf");
        let path = std::env::temp_dir().join(print_name(name.trim_end_matches(".pdf")));
        let r = pdf(self, scope, default_paper())
            .and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
        match r.and_then(|()| print_file(&path)) {
            // Said as it happens: printed, or opened to print from the viewer.
            Ok(rx) => {
                self.score.printing = Some(rx);
                self.set_status(t("score.status.print_sending"));
            }
            Err(e) => self.set_error(tf("score.status.print_failed", &[("error", &e)])),
        }
    }

    /// *Import MusicXML…*: each part becomes a clip on a new track at the song's start, as
    /// one undo step. Into a song with no notes yet, the file's tempo, metres and key come
    /// with it (in the same step); a song with notes keeps its own (review, finding 9).
    pub fn import_musicxml(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MusicXML", &["musicxml", "xml"])
            .pick_file()
        {
            self.import_musicxml_from(&path);
        }
    }

    pub fn import_musicxml_from(&mut self, path: &Path) -> bool {
        let text = match std::fs::read(path) {
            Ok(b) if b.starts_with(b"PK") => {
                self.set_error(t("score.import.compressed"));
                return false;
            }
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                self.set_error(tf("score.import.failed", &[("error", &e.to_string())]));
                return false;
            }
        };
        let imported = match cantus_score::musicxml_in::read(&text) {
            Ok(i) => i,
            Err(e) => {
                self.set_error(tf("score.import.failed", &[("error", &e.to_string())]));
                return false;
            }
        };
        // A song with nothing in it takes the file's tempo, metres and key; each only where
        // the song would accept it (a metre change off a bar line is left out, not forced).
        let empty = self.project().tracks.iter().all(|t| {
            t.clips
                .iter()
                .all(|c| c.notes.is_empty() && c.audio.is_none())
        });
        let mut commands = Vec::new();
        let mut probe = self.project().clone();
        if empty {
            let mut song: Vec<Command> = Vec::new();
            match imported.tempo.as_slice() {
                [] => {}
                [(_, bpm)] => song.push(Command::SetTempo { bpm: *bpm }),
                many => song.push(Command::SetTempoTrack {
                    bpm: many[0].1,
                    points: many
                        .iter()
                        .map(|(tick, bpm)| cantus_core::TempoPoint {
                            tick: Ticks(*tick),
                            bpm: *bpm,
                            ramp: false,
                        })
                        .collect(),
                }),
            }
            match imported.metres.as_slice() {
                [] => {}
                [(_, sig)] => song.push(Command::SetTimeSignature {
                    time_signature: *sig,
                }),
                many => song.push(Command::SetMetreTrack {
                    signature: many[0].1,
                    metres: many
                        .iter()
                        .map(|(tick, sig)| cantus_core::MetreChange {
                            tick: Ticks(*tick),
                            signature: *sig,
                        })
                        .collect(),
                }),
            }
            if let Some((fifths, mode)) = imported.key {
                song.push(Command::SetProjectKey {
                    key: key_of(fifths, mode),
                });
            }
            for c in song {
                if c.clone().apply(&mut probe).is_ok() {
                    commands.push(c);
                }
            }
        }
        let map = probe.tempo_map();
        let base = self.project().tracks.len();
        let mut made = 0usize;
        for part in &imported.parts {
            if part.notes.is_empty() {
                continue;
            }
            let end = part
                .notes
                .iter()
                .map(|n| n.start + n.len)
                .max()
                .unwrap_or(0);
            let bar = map.bar_end(Ticks(end.max(1) - 1)).0;
            let mut track = self.bus.new_track(part.name.clone());
            let mut clip = self
                .bus
                .new_clip(part.name.clone(), Ticks::ZERO, Ticks(bar.max(1)));
            for n in &part.notes {
                let note = self
                    .bus
                    .new_note(Ticks(n.start), Ticks(n.len), n.pitch, n.velocity);
                clip.notes.push(note);
            }
            track.clips.push(clip);
            commands.push(Command::InsertTrack {
                index: base + made,
                track: Box::new(track),
            });
            made += 1;
        }
        if made == 0 {
            self.set_error(t("score.import.empty"));
            return false;
        }
        let ok = self.exec(Command::Batch {
            label_key: "cmd.score.import".into(),
            commands,
        });
        if ok {
            let key = if empty {
                "score.import.done"
            } else {
                "score.import.done_kept"
            };
            self.set_status(tf(key, &[("n", &made.to_string())]));
        }
        ok
    }

    /// The notes of the selected clip that are selected.
    fn score_selection(&self) -> Option<(TrackId, ClipId, Vec<Note>)> {
        let (t, c) = self.selected_track.zip(self.selected_clip)?;
        let clip = self.project().track(t)?.clip(c)?;
        let sel: Vec<Note> = clip
            .notes
            .iter()
            .filter(|n| self.selected_notes.contains(&n.id))
            .cloned()
            .collect();
        (!sel.is_empty()).then_some((t, c, sel))
    }

    /// Move the selected notes a step on the staff (a degree of the song's key, or a
    /// semitone without one), or an octave.
    pub fn score_step(&mut self, steps: i32, octave: bool) {
        let Some((t, c, sel)) = self.score_selection() else {
            return;
        };
        let key = self.project().key;
        let moved: Option<Vec<Note>> = sel
            .iter()
            .map(|n| {
                let p = if octave {
                    let p = n.pitch as i32 + 12 * steps;
                    (0..=127).contains(&p).then_some(p as u8)
                } else {
                    match key {
                        Some(k) => k.step(k.snap(n.pitch), steps).filter(|p| *p != n.pitch),
                        None => {
                            let p = n.pitch as i32 + steps;
                            (0..=127).contains(&p).then_some(p as u8)
                        }
                    }
                }?;
                Some(Note {
                    pitch: p,
                    ..n.clone()
                })
            })
            .collect();
        let Some(notes) = moved else { return };
        if let Some(first) = notes.first() {
            self.audition(t, first.pitch);
        }
        self.exec(Command::Batch {
            label_key: "cmd.score.move".into(),
            commands: vec![Command::SetNotes {
                track: t,
                clip: c,
                notes,
            }],
        });
    }

    /// Delete the selected notes.
    pub fn score_delete(&mut self) {
        let Some((t, c, sel)) = self.score_selection() else {
            return;
        };
        let ids: Vec<NoteId> = sel.iter().map(|n| n.id).collect();
        if self.exec(Command::Batch {
            label_key: "cmd.score.delete".into(),
            commands: vec![Command::RemoveNotes {
                track: t,
                clip: c,
                ids,
            }],
        }) {
            self.selected_notes.clear();
        }
    }

    /// Enter a note by its letter (0 = C … 6 = B) at the entry cursor, in the selected
    /// clip: the key's alteration for that letter, the octave nearest the last note, the
    /// cursor's value; the cursor moves on past it.
    pub fn score_enter(&mut self, letter: u8) -> bool {
        let Some(mut e) = self.score.entry else {
            return false;
        };
        let Some((t, c)) = self.selected_track.zip(self.selected_clip) else {
            return false;
        };
        let Some(clip) = self.project().track(t).and_then(|tr| tr.clip(c)) else {
            return false;
        };
        // Only where the clip shows its content (review, finding 7): past its window a note
        // would pile up unseen — note input stops there.
        let Some(content) = clip.content_shown_at(Ticks(e.at)) else {
            self.score.entry = None;
            self.set_status(crate::i18n::t("score.entry.outside"));
            return false;
        };
        // The letter is the written one: a transposed part's key and pitch (a B-flat part's
        // D is a concert C).
        let transpose = self
            .project()
            .track(t)
            .map_or(0, |tr| tr.score.transpose as i32);
        let (_, sig) = cantus_score::spell::Speller::transposed(self.project().key, transpose);
        let alter = cantus_score::spell::key_alters(sig)[letter as usize % 7] as i32;
        const PC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
        let pc = (PC[letter as usize % 7] + alter).rem_euclid(12);
        // The octave nearest the last note entered (as written).
        let last = e.last as i32 + transpose;
        let mut written = 60;
        let mut dist = i32::MAX;
        for o in 0..11 {
            let p = o * 12 + pc;
            if (0..=127).contains(&p) && (p - last).abs() < dist {
                dist = (p - last).abs();
                written = p;
            }
        }
        let best = (written - transpose).clamp(0, 127);
        let len = Ticks(e.dur.ticks());
        let note = self.bus.new_note(content, len, best as u8, 90);
        let id = note.id;
        let ok = self.exec(Command::Batch {
            label_key: "cmd.score.enter".into(),
            commands: vec![Command::InsertNotes {
                track: t,
                clip: c,
                notes: vec![note],
            }],
        });
        if ok {
            self.audition(t, best as u8);
            self.selected_notes = vec![id];
            e.at += len.0;
            e.last = best as u8;
            self.score.entry = Some(e);
        }
        ok
    }
}

/// The name the printed PDF is written under in the temporary folder (review, finding
/// 2): only `[A-Za-z0-9 _.-]` from the scope's name (anything else an underscore, no
/// `..`), behind `cantus-score-<process id>-`, so nothing a shell or a program's command
/// line parses ever reaches one.
pub fn print_name(name: &str) -> String {
    let mut safe: String = name
        .chars()
        .take(60)
        .map(|c| {
            if c.is_ascii_alphanumeric() || " _-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    safe = safe.trim().to_string();
    if safe.is_empty() {
        safe.push_str("score");
    }
    format!("cantus-score-{}-{safe}.pdf", std::process::id())
}

/// Hand a PDF to the system's printing, on a thread of its own (the interface never waits
/// for it); what happened — printed, opened to print from the viewer, or neither — comes
/// back on the channel as a `score.print.*` key (`poll_print` says it):
///
/// - **Windows**: PowerShell's `Start-Process -Verb Print` — the shell's *print* verb, so
///   the default PDF application prints it (Edge, Acrobat, SumatraPDF…) — started with no
///   console window; with no application able to print PDFs, `explorer.exe` opens the
///   file instead, to be printed from its viewer. `cantus-ui` forbids `unsafe`, so the
///   shell's API is reached through those two programs rather than `ShellExecuteW`; the
///   file's name is [`print_name`]'s, never a track's raw name, and `cmd` is never used.
/// - **Linux**: `lp` (CUPS) sends it to the default printer; without CUPS, it opens in the
///   desktop's PDF viewer (`xdg-open`).
/// - **macOS**: `lp` as well (CUPS is the Mac's printing system); else it opens in Preview.
pub fn print_file(path: &Path) -> Result<std::sync::mpsc::Receiver<&'static str>, String> {
    if !path.is_file() {
        return Err(format!("{} is not there", path.display()));
    }
    let path = path.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("score-print".into())
        .spawn(move || {
            let how = run_print(&path);
            let _ = tx.send(how);
        })
        .map_err(|e| e.to_string())?;
    Ok(rx)
}

/// The command Windows prints a PDF with: PowerShell's `Start-Process -Verb Print` (the
/// shell's print verb), the file's path handed over as data in the environment
/// (`CANTUS_PRINT`) and never spliced into the command PowerShell parses (review 2,
/// finding 2: PowerShell takes ’ ‘ ‚ ‛ as quotes too, so a folder named `O’Brien` broke a
/// quoted path).
pub struct PrintCommand {
    pub program: &'static str,
    pub args: Vec<String>,
    pub env: Vec<(String, std::ffi::OsString)>,
}

pub fn print_command(path: &Path) -> PrintCommand {
    PrintCommand {
        program: "powershell",
        args: [
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            "Start-Process -FilePath $env:CANTUS_PRINT -Verb Print",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        env: vec![("CANTUS_PRINT".into(), path.as_os_str().to_os_string())],
    }
}

/// Print or open `path`, and say which happened (a `score.print.*` key).
fn run_print(path: &Path) -> &'static str {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let c = print_command(path);
        let printed = std::process::Command::new(c.program)
            .args(&c.args)
            .envs(c.env.iter().map(|(k, v)| (k.as_str(), v.as_os_str())))
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if printed {
            return "score.print.windows";
        }
        match std::process::Command::new("explorer.exe").arg(path).spawn() {
            Ok(_) => "score.print.opened",
            Err(_) => "score.print.failed",
        }
    }
    #[cfg(not(windows))]
    {
        let lp = std::process::Command::new("lp").arg(path).status();
        if lp.map(|s| s.success()).unwrap_or(false) {
            return "score.print.cups";
        }
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(opener).arg(path).spawn() {
            Ok(_) => "score.print.opened",
            Err(_) => "score.print.failed",
        }
    }
}

/// Say how a print went, once its thread has told (polled every frame).
pub fn poll_print(app: &mut CantusApp) {
    let Some(rx) = app.score.printing.as_ref() else {
        return;
    };
    match rx.try_recv() {
        Ok(how) => {
            app.score.printing = None;
            let text = tf("score.status.printing", &[("how", &t(how))]);
            if how == "score.print.failed" {
                app.set_error(text);
            } else {
                app.set_status(text);
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => app.score.printing = None,
    }
}

/// The score's keys while it has them (it was clicked last and is on screen) — taken
/// before the keymap's, so they edit the staff rather than whatever the keys do elsewhere:
/// ↑ ↓ a step (Ctrl an octave), Delete, N for note input; in note input 3–7 the value
/// (MuseScore's: a sixteenth to a whole note), `.` a dot, A–G a note, Esc out.
pub fn keys(app: &mut CantusApp, ctx: &egui::Context) {
    use egui::{Key, Modifiers};
    poll_print(app);
    let pass = ctx.cumulative_pass_nr();
    if !app.score.focused || app.score.drawn_pass + 1 < pass {
        return;
    }
    // Exactly these modifiers (review, finding 4): egui's `consume_key` matches logically
    // and would take Shift+↑ (transpose an octave) or Alt+C (count-in) as well.
    let take = |k: Key, m: Modifiers| ctx.input_mut(|i| take_exact(i, k, m));
    // Esc leaves note input; outside it, it is the keymap's (clear the selection).
    if app.score.entry.is_some() && take(Key::Escape, Modifiers::NONE) {
        app.score.entry = None;
        return;
    }
    for (k, steps) in [(Key::ArrowUp, 1), (Key::ArrowDown, -1)] {
        if take(k, Modifiers::COMMAND) {
            app.score_step(steps, true);
        } else if take(k, Modifiers::NONE) {
            app.score_step(steps, false);
        }
    }
    if take(Key::Delete, Modifiers::NONE) || take(Key::Backspace, Modifiers::NONE) {
        app.score_delete();
    }
    if take(Key::N, Modifiers::NONE) {
        if let Some((t, c)) = app.selected_track.zip(app.selected_clip) {
            crate::views::score_bar::toggle_entry(app, t, c);
        }
        return;
    }
    let Some(mut e) = app.score.entry else { return };
    for (k, log) in [
        (Key::Num3, 4),
        (Key::Num4, 3),
        (Key::Num5, 2),
        (Key::Num6, 1),
        (Key::Num7, 0),
    ] {
        if take(k, Modifiers::NONE) {
            e.dur = Dur::new(log, 0);
        }
    }
    if take(Key::Period, Modifiers::NONE) {
        e.dur.dots = if e.dur.dots == 0 { 1 } else { 0 };
    }
    app.score.entry = Some(e);
    for (k, letter) in [
        (Key::C, 0u8),
        (Key::D, 1),
        (Key::E, 2),
        (Key::F, 3),
        (Key::G, 4),
        (Key::A, 5),
        (Key::B, 6),
    ] {
        if take(k, Modifiers::NONE) {
            app.score_enter(letter);
        }
    }
}

/// The score's actions; `true` when `action` was one.
pub fn action(app: &mut CantusApp, action: crate::keymap::Action) -> bool {
    use crate::keymap::Action;
    match action {
        Action::ShowScore => {
            app.score.split_open = !app.score.split_open;
            if app.score.split_open {
                app.show_pane(crate::views::workspace::Pane::PianoRoll);
            }
        }
        Action::ViewScore => app.toggle_pane(crate::views::workspace::Pane::Score),
        _ => return false,
    }
    true
}

/// Take a press of `key` held with exactly `mods` (Ctrl and ⌘ as egui's `command`).
fn take_exact(i: &mut egui::InputState, key: egui::Key, mods: egui::Modifiers) -> bool {
    let at = i.events.iter().position(|e| {
        matches!(e, egui::Event::Key { key: k, pressed: true, modifiers: m, .. }
            if *k == key
                && m.alt == mods.alt
                && m.shift == mods.shift
                && m.command == mods.command
                && (m.ctrl || m.mac_cmd) == (mods.ctrl || mods.mac_cmd || mods.command))
    });
    match at {
        Some(p) => {
            i.events.remove(p);
            true
        }
        None => false,
    }
}

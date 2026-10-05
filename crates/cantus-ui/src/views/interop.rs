//! Moving work between Cantus and the other DAWs (ADR 0021): the File menu's entries,
//! their dialogs, a MIDI file dropped on the arrangement, stems exported in the
//! background with their progress in the status bar, and a report of whatever an import
//! or an export could not carry.
//!
//! Every import is one step of the undo history: a MIDI file is one `Batch` of new tracks,
//! a DAWproject one `ReplaceProject`.

use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};

use cantus_core::{Command, InstrumentSpec, Project, Ticks};
use cantus_engine::{stem_file_name, RenderOptions, StemOptions, StemSource, StemSpec, StemsDone};
use cantus_project::{dawproject, midi, Report, Severity};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

/// The dialogs, the report and the running stems export.
#[derive(Default)]
pub struct Interop {
    pub midi_export: Option<MidiExportDialog>,
    pub midi_import: Option<MidiImportDialog>,
    pub stems: Option<StemsDialog>,
    pub report: Option<ReportWindow>,
    job: Option<StemsJob>,
}

impl Interop {
    /// A stems export is running.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
}

/// What "Export MIDI…" writes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MidiScope {
    Song,
    Track,
    Clip,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MidiExportDialog {
    pub scope: MidiScope,
    pub ppq: u16,
}

/// The resolutions offered, in ticks per quarter note.
pub const PPQ_CHOICES: [u16; 5] = [96, 192, 480, 960, 1920];

/// The tails a stem export offers after the music ends, in seconds.
const TAILS: [f32; 8] = [0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 15.0, 30.0];

#[derive(Clone, Debug)]
pub struct MidiImportDialog {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    /// Read by beats, for the summary.
    pub preview: midi::MidiImport,
    pub keep_time: bool,
    pub apply_tempo: bool,
    pub at: Ticks,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StemsDialog {
    /// Tracks rather than strips.
    pub tracks: bool,
    pub tail_seconds: f32,
    pub keep_silent: bool,
    pub master: bool,
    /// Each stem through its own copy of the master chain (ADR 0026).
    pub through_master: bool,
    pub folder: Option<PathBuf>,
}

impl Default for StemsDialog {
    fn default() -> Self {
        StemsDialog {
            tracks: false,
            tail_seconds: 2.0,
            keep_silent: false,
            master: false,
            through_master: false,
            folder: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReportWindow {
    pub title: String,
    pub report: Report,
}

struct StemsJob {
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<StemsDone, String>>,
    folder: PathBuf,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// One report line in the interface's language.
pub fn report_line(item: &cantus_project::ReportItem) -> String {
    let args: Vec<(&str, &dyn std::fmt::Display)> = item
        .args
        .iter()
        .map(|(k, v)| (*k, v as &dyn std::fmt::Display))
        .collect();
    tf(item.key, &args)
}

// ---- the File menu ------------------------------------------------------------------

/// The interchange entries of the File menu, after "Export WAV…".
pub fn file_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    ui.separator();
    if ui.button(t("interop.menu.import_midi")).clicked() {
        ui.close();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MIDI", &midi::EXTENSIONS)
            .pick_file()
        {
            open_midi_import(app, &path);
        }
    }
    if ui.button(t("interop.menu.export_midi")).clicked() {
        ui.close();
        let scope = if app.selected_clip.is_some() {
            MidiScope::Clip
        } else {
            MidiScope::Song
        };
        app.interop.midi_export = Some(MidiExportDialog {
            scope,
            ppq: midi::DEFAULT_PPQ,
        });
    }
    let busy = app.interop.busy();
    if ui
        .add_enabled(!busy, egui::Button::new(t("interop.menu.export_stems")))
        .clicked()
    {
        ui.close();
        app.interop.stems.get_or_insert_with(StemsDialog::default);
    }
    ui.separator();
    if ui.button(t("interop.menu.import_dawproject")).clicked() {
        ui.close();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DAWproject", &[dawproject::EXTENSION])
            .pick_file()
        {
            import_dawproject_from(app, &path);
        }
    }
    if ui.button(t("interop.menu.export_dawproject")).clicked() {
        ui.close();
        let name = format!("{}.{}", app.project().name, dawproject::EXTENSION);
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DAWproject", &[dawproject::EXTENSION])
            .set_file_name(name)
            .save_file()
        {
            export_dawproject_to(app, &path);
        }
    }
}

// ---- every frame --------------------------------------------------------------------

/// Poll the stems export, take dropped files, and show whichever dialog is open.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    poll_stems(app, ctx);
    dropped_files(app, ctx);
    midi_export_window(app, ctx);
    midi_import_window(app, ctx);
    stems_window(app, ctx);
    report_window(app, ctx);
}

// ---- MIDI ---------------------------------------------------------------------------

/// A track name for what the file says about the track, in the interface's language.
pub fn track_name(label: &midi::TrackLabel) -> String {
    match label {
        midi::TrackLabel::Named(n) => n.clone(),
        midi::TrackLabel::NamedChannel(n, c) => {
            tf("interop.track.named_channel", &[("name", n), ("n", c)])
        }
        midi::TrackLabel::Program(p) => p.to_string(),
        midi::TrackLabel::Drums => t("interop.track.drums"),
        midi::TrackLabel::Channel(c) => tf("interop.track.channel", &[("n", c)]),
    }
}

/// Where an import lands by default: the bar the playhead is in, or the start of an empty
/// project.
fn default_at(app: &CantusApp) -> Ticks {
    if app.project().content_end() == Ticks::ZERO {
        return Ticks::ZERO;
    }
    let bar = app.project().time_signature.ticks_per_bar().max(Ticks(1));
    app.engine.position().snap_floor(bar).max(Ticks::ZERO)
}

/// Read a file and open the import dialog on it.
pub fn open_midi_import(app: &mut CantusApp, path: &Path) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            app.set_error(tf(
                "interop.status.import_failed",
                &[("file", &file_name(path)), ("error", &e)],
            ));
            return;
        }
    };
    match midi::read(&bytes, &midi::ImportOptions::default()) {
        Ok(preview) => {
            let empty = app.project().content_end() == Ticks::ZERO;
            app.interop.midi_import = Some(MidiImportDialog {
                path: path.to_path_buf(),
                bytes,
                preview,
                keep_time: false,
                apply_tempo: empty,
                at: default_at(app),
            });
        }
        Err(e) => app.set_error(tf(
            "interop.status.import_failed",
            &[("file", &file_name(path)), ("error", &e)],
        )),
    }
}

/// Add a MIDI file's tracks to the project as one undo step, at `at`, before track
/// `index` (or after the last). Returns whether anything was imported.
pub fn import_midi(
    app: &mut CantusApp,
    bytes: &[u8],
    name: &str,
    keep_time: bool,
    apply_tempo: bool,
    at: Ticks,
    index: Option<usize>,
) -> bool {
    let target_bpm = (!apply_tempo).then(|| app.project().tempo_bpm);
    let opts = midi::ImportOptions {
        tempo_mode: if keep_time {
            midi::TempoMode::Seconds
        } else {
            midi::TempoMode::Beats
        },
        target_bpm,
    };
    let import = match midi::read(bytes, &opts) {
        Ok(i) => i,
        Err(e) => {
            app.set_error(tf(
                "interop.status.import_failed",
                &[("file", &name), ("error", &e)],
            ));
            return false;
        }
    };
    let place = midi::Placement {
        at,
        index,
        apply_tempo,
        max_tracks: cantus_engine::MAX_TRACKS,
    };
    let mut next = app.bus.project().next_id;
    let mut built = {
        let mut ids = || {
            let id = next;
            next += 1;
            id
        };
        midi::import_commands(app.bus.project(), &import, &place, &mut ids, &track_name)
    };
    app.bus.reserve_ids(next);
    // The shipped pads, in the interface's language, as a kit made in the app has them.
    for c in &mut built.commands {
        if let Command::InsertTrack { track, .. } = c {
            if let InstrumentSpec::Pulsus(kit) = &mut track.instrument {
                for pad in &mut kit.pads {
                    pad.name = crate::app::localized_pad_name(pad);
                }
            }
        }
    }
    if built.tracks.is_empty() {
        app.set_error(tf("interop.status.midi_empty", &[("file", &name)]));
        return false;
    }
    let ok = app.exec(Command::Batch {
        label_key: "cmd.import_midi".into(),
        commands: built.commands,
    });
    if !ok {
        return false;
    }
    if let Some((track, clip)) = built.first_clip {
        app.select_clip(track, clip);
    }
    let mut report = import.report;
    report.items.extend(built.report.items);
    app.set_status(tf(
        "interop.status.midi_imported",
        &[
            ("tracks", &built.tracks.len()),
            (
                "notes",
                &import.tracks.iter().map(|t| t.notes.len()).sum::<usize>(),
            ),
            ("file", &name),
        ],
    ));
    if !report.is_lossless() {
        app.interop.report = Some(ReportWindow {
            title: tf("interop.report.title_import", &[("file", &name)]),
            report,
        });
    }
    true
}

/// Write the chosen part of the project as a MIDI file.
pub fn export_midi_to(app: &mut CantusApp, path: &Path, scope: MidiScope, ppq: u16) -> bool {
    let scope = match (scope, app.selected_track, app.selected_clip) {
        (MidiScope::Clip, Some(track), Some(clip)) => midi::Scope::Clip { track, clip },
        (MidiScope::Track | MidiScope::Clip, Some(track), _) => midi::Scope::Track(track),
        _ => midi::Scope::Song,
    };
    let result = midi::export(app.project(), &midi::ExportOptions { scope, ppq })
        .map_err(|e| e.to_string())
        .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
    match result {
        Ok(()) => {
            app.set_status(tf("interop.status.exported", &[("file", &file_name(path))]));
            true
        }
        Err(e) => {
            app.set_error(tf(
                "interop.status.export_failed",
                &[("file", &file_name(path)), ("error", &e)],
            ));
            false
        }
    }
}

fn dialog(title: String) -> egui::Window<'static> {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
}

/// A dialog's explanation: long-form text, wrapped, in the dim text colour — the one place
/// a sentence belongs (ADR 0024 §3: Plex Sans for dialogs and help).
fn hint(ui: &mut egui::Ui, text: String) {
    ui.label(
        egui::RichText::new(text)
            .text_style(design::caption())
            .color(crate::kit::Style::current().t.text_dim),
    );
}

/// A paragraph of a report: wrapped, in the text colour or dimmed.
fn para(ui: &mut egui::Ui, text: String, dim: bool) {
    let t = crate::kit::Style::current().t;
    ui.label(
        egui::RichText::new(text)
            .text_style(egui::TextStyle::Body)
            .color(if dim { t.text_dim } else { t.text }),
    );
}

/// The dialogs' parts, drawn with the kit: a band over each group of rows, choices and
/// switches as lamps beside their words, fields as wells, buttons as caps.
pub(crate) mod form {
    use crate::kit::light::{self, RaisedOpts};
    use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
    use crate::kit::{band, led, well, with_brush, State, Style, Tok};
    use egui::{Align2, Rect, Response, Sense, Ui, Vec2};

    /// The narrowest a dialog's rows are.
    const WIDTH: f32 = 300.0;

    fn row(ui: &mut Ui, h: f32, sense: Sense) -> (Rect, Response, Style) {
        let s = Style::current();
        let w = ui.available_width().max(WIDTH);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(h)), sense);
        (r, resp, s)
    }

    /// A band naming the group of rows under it.
    pub fn band(ui: &mut Ui, key: &str) {
        let (r, _, s) = row(ui, band::BAND, Sense::hover());
        with_brush(ui.painter(), |b| {
            band::section(b, r, &s, State::REST, Src::Key(key), None, false)
        });
    }

    /// One choice of several, or a switch: a lamp beside its words, lit when `on`.
    /// Returns whether it was clicked.
    pub fn choice(ui: &mut Ui, on: bool, enabled: bool, label: &str) -> bool {
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (r, resp, s) = row(ui, 22.0, sense);
        let hovered = enabled && resp.hovered();
        with_brush(ui.painter(), |b| {
            b.register(
                r,
                "choice",
                &[Tok::Led, Tok::Text, Tok::TextDim, Tok::Panel],
                &[],
            );
            if hovered {
                b.rrect(
                    r,
                    s.r(2.0),
                    crate::kit::Fill::Solid(crate::kit::color::shade(
                        s.t.panel,
                        if s.t.dark { 0.04 } else { -0.04 },
                    )),
                );
            }
            let c = egui::pos2(r.left() + s.s(12.0), r.center().y);
            led::paint(
                b,
                c,
                s.s(11.0),
                on,
                &s,
                Tok::Led,
                Tok::Panel,
                if enabled { 1.0 } else { 0.45 },
            );
            let ink = if !enabled {
                s.t.placeholder
            } else if on {
                s.t.text
            } else {
                s.t.text_dim
            };
            let tr = Rect::from_min_max(egui::pos2(c.x + s.s(12.0), r.top()), r.max);
            let t = Text::new(label, Role::Body, ink)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle));
            let _ = text::paint(b, tr, &t, s.tk());
        });
        enabled && resp.clicked()
    }

    /// A caption in its cell and a value in a well beside it; `field` adds ▾ (it opens a
    /// list). Returns the well's response.
    pub fn value(ui: &mut Ui, caption: &str, value: &str, field: bool) -> Response {
        let (r, _, s) = row(ui, 26.0, Sense::hover());
        let cap = Rect::from_min_size(r.min, Vec2::new(r.width() * 0.4, r.height()));
        let well_r = Rect::from_min_max(
            egui::pos2(cap.right() + s.s(6.0), r.top() + s.s(2.0)),
            r.max - Vec2::new(0.0, s.s(2.0)),
        );
        let resp = ui.interact(
            well_r,
            ui.id().with(("form-value", caption)),
            if field {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let st = State::of(&resp, false);
        with_brush(ui.painter(), |b| {
            b.register(cap, "caption", &[Tok::TextDim], &[]);
            let t = Text::new(caption, Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, cap, &t, s.tk());
            if field {
                well::menu_field(b, well_r, &s, st, value, false);
            } else {
                well::readout(b, well_r, &s, st, value, false);
            }
        });
        resp
    }

    /// A push button: a cap with its word; `primary` wears the highlight.
    pub fn button(ui: &mut Ui, label: &str, primary: bool, enabled: bool) -> bool {
        let s = Style::current();
        let t = Text::new(label, Role::Control, s.t.text);
        let w = text::measure(ui.ctx(), &t, s.tk()).x + s.s(28.0);
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w.max(s.s(76.0)), s.s(26.0)), sense);
        let mut st = State::of(&resp, false);
        st.disabled = !enabled;
        with_brush(ui.painter(), |b| {
            b.register(
                r,
                "button",
                &[
                    Tok::PanelRaised,
                    Tok::Highlight,
                    Tok::HighlightGlyph,
                    Tok::Text,
                ],
                &[],
            );
            let base = if primary {
                s.t.highlight
            } else {
                s.t.panel_raised
            };
            let base = if st.active {
                crate::kit::color::shade(base, -0.03)
            } else if st.hovered {
                crate::kit::color::shade(base, 0.04)
            } else {
                base
            };
            light::raised(
                b,
                r,
                s.r(3.0),
                base,
                RaisedOpts {
                    drop: !st.active,
                    k: s.k,
                    alpha: st.alpha(),
                    depth: s.t.depth,
                    ..Default::default()
                },
            );
            let ink = if primary {
                s.t.highlight_glyph
            } else {
                s.t.text
            };
            let t = Text::new(label, Role::Control, ink.gamma_multiply(st.alpha()))
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, r.shrink2(Vec2::new(s.s(8.0), 0.0)), &t, s.tk());
        });
        enabled && resp.clicked()
    }

    /// How far a job has got: a well filling with the selection colour, the percentage in it.
    pub fn progress(ui: &mut Ui, f: f32) {
        let (r, _, s) = row(ui, 22.0, Sense::hover());
        let mut pct = String::with_capacity(6);
        {
            use std::fmt::Write as _;
            let _ = write!(pct, "{:.0} %", f.clamp(0.0, 1.0) * 100.0);
        }
        with_brush(ui.painter(), |b| {
            well::paint(b, r, &s, State::REST, None);
            let fill =
                Rect::from_min_size(r.min, Vec2::new(r.width() * f.clamp(0.0, 1.0), r.height()))
                    .shrink(s.s(2.0));
            b.register(r, "progress", &[Tok::Well, Tok::Selection, Tok::Text], &[]);
            b.rrect(
                fill,
                s.r(1.5),
                crate::kit::Fill::Solid(crate::kit::color::with_alpha(s.t.selection, 0.55)),
            );
            let t = Text::new(&pct, Role::Value, s.t.text);
            let _ = text::paint(b, r, &t, s.tk());
        });
    }

    /// A row of buttons, right-aligned, the way a dialog ends.
    pub fn buttons<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add)
            .inner
    }
}

fn midi_export_window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(mut d) = app.interop.midi_export.clone() else {
        return;
    };
    let track_name = app
        .selected_track
        .and_then(|id| app.project().track(id))
        .map(|t| t.name.clone());
    let clip_name = app
        .selected_track
        .zip(app.selected_clip)
        .and_then(|(t, c)| app.project().track(t)?.clip(c))
        .map(|c| c.name.clone());
    let (mut export, mut close) = (false, false);
    dialog(t("interop.midi_export.title")).show(ctx, |ui| {
        form::band(ui, "interop.midi_export.what");
        if form::choice(
            ui,
            d.scope == MidiScope::Song,
            true,
            &t("interop.midi_export.song"),
        ) {
            d.scope = MidiScope::Song;
        }
        let name = track_name.clone().unwrap_or_default();
        if form::choice(
            ui,
            d.scope == MidiScope::Track,
            track_name.is_some(),
            &tf("interop.midi_export.track", &[("name", &name)]),
        ) {
            d.scope = MidiScope::Track;
        }
        let name = clip_name.clone().unwrap_or_default();
        if form::choice(
            ui,
            d.scope == MidiScope::Clip,
            clip_name.is_some(),
            &tf("interop.midi_export.clip", &[("name", &name)]),
        ) {
            d.scope = MidiScope::Clip;
        }
        ui.add_space(space::S2);
        let field = form::value(ui, &t("interop.midi_export.ppq"), &d.ppq.to_string(), true);
        egui::Popup::menu(&field)
            .frame(egui::Frame::NONE)
            .show(|ui| {
                use crate::kit::menu::{Check, Item};
                use crate::kit::text::Src;
                let labels = PPQ_CHOICES.map(|p| p.to_string());
                let items: [Item<'_>; 5] = std::array::from_fn(|i| Item::Action {
                    label: Src::Str(&labels[i]),
                    check: Check::Radio(PPQ_CHOICES[i] == d.ppq),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                });
                if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 160.0).1.clicked {
                    d.ppq = PPQ_CHOICES[i];
                    ui.close();
                }
            });
        hint(ui, t("interop.midi_export.ppq_hint"));
        hint(ui, t("interop.midi_export.note"));
        form::buttons(ui, |ui| {
            close = form::button(ui, &t("interop.cancel"), false, true);
            export = form::button(ui, &t("interop.export"), true, true);
        });
    });
    app.interop.midi_export = (!close && !export).then_some(d.clone());
    if export {
        let base = match d.scope {
            MidiScope::Song => app.project().name.clone(),
            MidiScope::Track => track_name.unwrap_or_default(),
            MidiScope::Clip => clip_name.unwrap_or_default(),
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MIDI", &midi::EXTENSIONS)
            .set_file_name(format!("{base}.mid"))
            .save_file()
        {
            export_midi_to(app, &path, d.scope, d.ppq);
        }
    }
}

fn midi_import_window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(mut d) = app.interop.midi_import.take() else {
        return;
    };
    let (mut import, mut close) = (false, false);
    let p = &d.preview;
    let changes = p
        .report
        .get("interop.report.tempo_changes")
        .and_then(|i| i.arg("n"))
        .map(str::to_string);
    let sig = p
        .time_signature
        .unwrap_or(app.project().time_signature)
        .to_string();
    let bar = app.project().time_signature.bbt(d.at).0;
    dialog(t("interop.midi_import.title")).show(ctx, |ui| {
        ui.label(tf(
            "interop.midi_import.summary",
            &[
                ("file", &file_name(&d.path)),
                ("format", &p.format),
                ("tracks", &p.tracks.len()),
                ("notes", &p.note_count()),
                ("bpm", &p.tempo_bpm),
            ],
        ));
        ui.add_space(space::S2);
        if let Some(n) = &changes {
            hint(ui, tf("interop.midi_import.tempo_changes", &[("n", n)]));
            if form::choice(ui, !d.keep_time, true, &t("interop.midi_import.keep_bars")) {
                d.keep_time = false;
            }
            if form::choice(ui, d.keep_time, true, &t("interop.midi_import.keep_time")) {
                d.keep_time = true;
            }
            ui.add_space(space::S2);
        }
        let apply = tf(
            "interop.midi_import.apply_tempo",
            &[("bpm", &p.tempo_bpm), ("sig", &sig)],
        );
        if form::choice(ui, d.apply_tempo, true, &apply) {
            d.apply_tempo = !d.apply_tempo;
        }
        hint(ui, tf("interop.midi_import.at", &[("bar", &bar)]));
        form::buttons(ui, |ui| {
            close = form::button(ui, &t("interop.cancel"), false, true);
            import = form::button(ui, &t("interop.import"), true, true);
        });
    });
    if import {
        let name = file_name(&d.path);
        import_midi(app, &d.bytes, &name, d.keep_time, d.apply_tempo, d.at, None);
    } else if !close {
        app.interop.midi_import = Some(d);
    }
}

// ---- stems --------------------------------------------------------------------------

/// The stems an export writes, named `NN Name.wav`: every strip by its number and the
/// name the desk shows (plus, when some track goes straight to the master, `00` for those
/// tracks), or every track by its number; and the master mix as `00` when asked.
pub fn stem_specs(project: &Project, tracks: bool, master: bool) -> Vec<StemSpec> {
    let mut specs = Vec::new();
    if master {
        specs.push(StemSpec {
            source: StemSource::Master,
            file_name: stem_file_name(0, &t("interop.stems.master_name")),
        });
    }
    if tracks {
        for (i, tr) in project
            .tracks
            .iter()
            .enumerate()
            .take(cantus_engine::MAX_TRACKS)
        {
            specs.push(StemSpec {
                source: StemSource::Track(i),
                file_name: stem_file_name(i + 1, &tr.name),
            });
        }
    } else {
        if project.tracks.iter().any(|t| t.strip.is_none()) {
            specs.push(StemSpec {
                source: StemSource::Direct,
                file_name: stem_file_name(0, &t("interop.stems.direct")),
            });
        }
        for (i, s) in project
            .strips
            .iter()
            .enumerate()
            .take(cantus_engine::MAX_STRIPS)
        {
            let name = if s.name.is_empty() {
                project
                    .tracks_on_strip(s.id)
                    .next()
                    .map(|t| t.name.clone())
                    .unwrap_or_default()
            } else {
                s.name.clone()
            };
            specs.push(StemSpec {
                source: StemSource::Strip(i),
                file_name: stem_file_name(i + 1, &name),
            });
        }
    }
    specs
}

/// Start exporting stems into `folder` on a thread of its own; the status bar shows how
/// far it has got.
pub fn export_stems_to(app: &mut CantusApp, folder: &Path, d: &StemsDialog) {
    if app.interop.job.is_some() {
        return;
    }
    let snapshot = app.engine.compile(app.bus.project(), app.bus.revision());
    let specs = stem_specs(app.project(), d.tracks, d.master);
    let opts = StemOptions {
        render: RenderOptions {
            sample_rate: app.engine.sample_rate.round() as u32,
            tail_seconds: d.tail_seconds.max(0.0) as f64,
            ..Default::default()
        },
        keep_silent: d.keep_silent,
        through_master: d.through_master,
    };
    let progress = Arc::new(AtomicU32::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let (p, c, dir) = (progress.clone(), cancel.clone(), folder.to_path_buf());
    let spawned = std::thread::Builder::new()
        .name("cantus-stems".into())
        .spawn(move || {
            let result =
                cantus_engine::render_stems_to_folder(snapshot, &opts, &specs, &dir, &mut |f| {
                    p.store((f * 1000.0) as u32, Ordering::Relaxed);
                    !c.load(Ordering::Relaxed)
                })
                .map_err(|e| match e {
                    cantus_engine::StemsError::Cancelled => String::new(),
                    e => e.to_string(),
                });
            let _ = tx.send(result);
        });
    if let Err(e) = spawned {
        app.set_error(tf(
            "interop.status.export_failed",
            &[("file", &file_name(folder)), ("error", &e)],
        ));
        return;
    }
    app.interop.job = Some(StemsJob {
        progress,
        cancel,
        rx,
        folder: folder.to_path_buf(),
    });
    app.set_status(tf("interop.stems.progress", &[("pct", &0)]));
}

/// Ask a running stems export to stop.
pub fn stop_stems(app: &mut CantusApp) {
    if let Some(job) = &app.interop.job {
        job.cancel.store(true, Ordering::Relaxed);
    }
}

/// Wait for the running stems export to finish (tests; the interface polls instead).
pub fn wait_for_stems(app: &mut CantusApp) {
    if let Some(job) = app.interop.job.take() {
        let result = job.rx.recv().unwrap_or_else(|_| Err("thread died".into()));
        finish_stems(app, &job.folder, result);
    }
}

fn finish_stems(app: &mut CantusApp, folder: &Path, result: Result<StemsDone, String>) {
    match result {
        Ok(done) => {
            let text = tf(
                "interop.stems.done",
                &[
                    ("n", &done.written.len()),
                    ("folder", &folder.display()),
                    ("silent", &done.silent.len()),
                ],
            );
            if done.unaligned {
                app.set_error(format!("{text} {}", t("interop.stems.unaligned")));
            } else {
                app.set_status(text);
            }
        }
        Err(e) if e.is_empty() => app.set_status(t("interop.stems.stopped")),
        Err(e) => app.set_error(tf(
            "interop.status.export_failed",
            &[("file", &folder.display()), ("error", &e)],
        )),
    }
}

fn poll_stems(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(job) = &app.interop.job else {
        return;
    };
    match job.rx.try_recv() {
        Ok(result) => {
            let folder = job.folder.clone();
            app.interop.job = None;
            finish_stems(app, &folder, result);
        }
        Err(mpsc::TryRecvError::Empty) => {
            let pct = job.progress.load(Ordering::Relaxed) / 10;
            app.set_status(tf("interop.stems.progress", &[("pct", &pct)]));
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        Err(mpsc::TryRecvError::Disconnected) => {
            let folder = job.folder.clone();
            app.interop.job = None;
            finish_stems(app, &folder, Err("thread died".into()));
        }
    }
}

fn stems_window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(mut d) = app.interop.stems.clone() else {
        return;
    };
    let busy = app.interop.busy();
    let pct = app
        .interop
        .job
        .as_ref()
        .map(|j| j.progress.load(Ordering::Relaxed) as f32 / 1000.0);
    let (mut start, mut stop, mut close, mut choose) = (false, false, false, false);
    dialog(t("interop.stems.title")).show(ctx, |ui| {
        let on = !busy;
        form::band(ui, "interop.stems.source");
        if form::choice(ui, !d.tracks, on, &t("interop.stems.strips")) {
            d.tracks = false;
        }
        if form::choice(ui, d.tracks, on, &t("interop.stems.tracks")) {
            d.tracks = true;
        }
        ui.add_space(space::S2);
        let tail = form::value(
            ui,
            &t("interop.stems.tail"),
            &format!("{:.1} s", d.tail_seconds),
            on,
        );
        if on {
            egui::Popup::menu(&tail)
                .frame(egui::Frame::NONE)
                .show(|ui| {
                    use crate::kit::menu::{Check, Item};
                    use crate::kit::text::Src;
                    let labels = TAILS.map(|s| format!("{s:.1} s"));
                    let items: [Item<'_>; TAILS.len()] = std::array::from_fn(|i| Item::Action {
                        label: Src::Str(&labels[i]),
                        check: Check::Radio((TAILS[i] - d.tail_seconds).abs() < 1e-3),
                        shortcut: None,
                        submenu: false,
                        enabled: true,
                    });
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 160.0).1.clicked {
                        d.tail_seconds = TAILS[i];
                        ui.close();
                    }
                });
        }
        if form::choice(ui, d.keep_silent, on, &t("interop.stems.keep_silent")) {
            d.keep_silent = !d.keep_silent;
        }
        if form::choice(ui, d.master, on, &t("interop.stems.master")) {
            d.master = !d.master;
        }
        // Only when the master has a chain to pass through (ADR 0026).
        if !app.project().master.effects.is_empty()
            && form::choice(ui, d.through_master, on, &t("master.stems.through"))
        {
            d.through_master = !d.through_master;
        }
        ui.add_space(space::S2);
        let shown = d
            .folder
            .as_ref()
            .map(|f| f.display().to_string())
            .unwrap_or_else(|| t("interop.stems.no_folder"));
        form::value(ui, &t("interop.stems.folder"), &shown, false);
        hint(ui, t("interop.stems.hint"));
        if let Some(f) = pct {
            ui.add_space(space::S2);
            form::progress(ui, f);
        }
        form::buttons(ui, |ui| {
            if busy {
                close = form::button(ui, &t("interop.close"), false, true);
                stop = form::button(ui, &t("interop.stems.stop"), true, true);
            } else {
                close = form::button(ui, &t("interop.cancel"), false, true);
                start = form::button(ui, &t("interop.stems.start"), true, d.folder.is_some());
                choose = form::button(ui, &t("interop.stems.choose"), false, true);
            }
        });
    });
    if choose {
        if let Some(f) = rfd::FileDialog::new().pick_folder() {
            d.folder = Some(f);
        }
    }
    if stop {
        stop_stems(app);
    }
    if start {
        if let Some(folder) = d.folder.clone() {
            export_stems_to(app, &folder, &d);
        }
    }
    app.interop.stems = (!close).then_some(d);
}

// ---- DAWproject ---------------------------------------------------------------------

pub fn export_dawproject_to(app: &mut CantusApp, path: &Path) -> bool {
    let out = dawproject::export(app.project(), crate::app::APP_VERSION);
    match std::fs::write(path, &out.bytes) {
        Ok(()) => {
            app.set_status(tf("interop.status.exported", &[("file", &file_name(path))]));
            if !out.report.is_lossless() {
                app.interop.report = Some(ReportWindow {
                    title: tf("interop.report.title_export", &[("file", &file_name(path))]),
                    report: out.report,
                });
            }
            true
        }
        Err(e) => {
            app.set_error(tf(
                "interop.status.export_failed",
                &[("file", &file_name(path)), ("error", &e)],
            ));
            false
        }
    }
}

pub fn import_dawproject_from(app: &mut CantusApp, path: &Path) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => import_dawproject(app, &bytes, &file_name(path)),
        Err(e) => {
            app.set_error(tf(
                "interop.status.import_failed",
                &[("file", &file_name(path)), ("error", &e)],
            ));
            false
        }
    }
}

/// Replace the project with a `.dawproject`'s, as one undo step.
pub fn import_dawproject(app: &mut CantusApp, bytes: &[u8], name: &str) -> bool {
    let imported = match dawproject::import(bytes) {
        Ok(i) => i,
        Err(e) => {
            app.set_error(tf(
                "interop.status.import_failed",
                &[("file", &name), ("error", &e)],
            ));
            return false;
        }
    };
    let mut project = imported.project;
    for track in &mut project.tracks {
        if let InstrumentSpec::Pulsus(kit) = &mut track.instrument {
            for pad in &mut kit.pads {
                pad.name = crate::app::localized_pad_name(pad);
            }
        }
    }
    let first = project.tracks.first().map(|t| t.id);
    app.engine.stop();
    if !app.exec(Command::ReplaceProject {
        project: Box::new(project),
    }) {
        return false;
    }
    app.path = None;
    app.selected_track = None;
    app.selected_clip = None;
    app.selected_notes.clear();
    app.selected_clips.clear();
    app.selected_strips.clear();
    app.selected_strip = None;
    if let Some(id) = first {
        app.select_track(id);
    }
    app.seek(Ticks::ZERO);
    app.set_status(tf("interop.status.dawproject_imported", &[("file", &name)]));
    app.interop.report = Some(ReportWindow {
        title: tf("interop.report.title_import", &[("file", &name)]),
        report: imported.report,
    });
    true
}

// ---- the report ---------------------------------------------------------------------

fn report_window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(w) = &app.interop.report else {
        return;
    };
    let mut close = false;
    egui::Window::new(w.title.clone())
        .collapsible(false)
        .resizable(true)
        .default_width(space::GRID * 110.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let dropped: Vec<_> = w.report.dropped_items().collect();
            let kept: Vec<_> = w
                .report
                .items
                .iter()
                .filter(|i| i.severity == Severity::Info)
                .collect();
            egui::ScrollArea::vertical()
                .max_height(space::GRID * 90.0)
                .show(ui, |ui| {
                    if dropped.is_empty() {
                        para(ui, t("interop.report.lossless"), false);
                    } else {
                        form::band(ui, "interop.report.dropped");
                        for item in dropped {
                            para(ui, format!("\u{2022} {}", report_line(item)), false);
                        }
                    }
                    if !kept.is_empty() {
                        ui.add_space(space::S3);
                        form::band(ui, "interop.report.kept");
                        for item in kept {
                            para(ui, format!("\u{2022} {}", report_line(item)), true);
                        }
                    }
                });
            let clicked = form::buttons(ui, |ui| form::button(ui, &t("interop.close"), true, true));
            close = clicked || ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    if close {
        app.interop.report = None;
    }
}

// ---- dropping files -----------------------------------------------------------------

/// Where a MIDI file dropped at `pos` lands: the tick under the pointer, snapped as the
/// arrangement snaps, and before the track below the one it was dropped on — or, off the
/// arrangement, the default place.
pub fn drop_target(app: &CantusApp, pos: Option<egui::Pos2>) -> (Ticks, Option<usize>) {
    let geo = app.arrangement.geometry();
    let inside = |g: &crate::views::arrangement::TimelineGeometry, p: egui::Pos2| {
        p.x >= g.left && p.x <= g.left + g.width && p.y >= g.ruler_top
    };
    match (geo, pos) {
        (Some(g), Some(p)) if inside(&g, p) => {
            let mut at = crate::views::beats_to_ticks(g.beats(p.x)).max(Ticks::ZERO);
            if app.arrangement.snap {
                at = at.snap_floor(app.arrangement.grid.max(Ticks(1)));
            }
            let index = g.row_at(p.y).map(|r| r + 1);
            (at, index)
        }
        _ => (default_at(app), None),
    }
}

fn dropped_files(app: &mut CantusApp, ctx: &egui::Context) {
    let (files, hovering, pos) = ctx.input(|i| {
        (
            i.raw.dropped_files.clone(),
            !i.raw.hovered_files.is_empty(),
            i.pointer.latest_pos(),
        )
    });
    if hovering {
        paint_drop_hint(app, ctx);
    }
    // Audio files become clips where they were dropped (ADR 0025), all in one undo step.
    let audio: Vec<PathBuf> = files
        .iter()
        .map(|f| f.path().to_path_buf())
        .filter(|p| crate::audio_files::is_audio_path(p))
        .collect();
    if !audio.is_empty() {
        crate::audio_files::drop_audio(app, &audio, pos);
    }
    for f in files {
        if crate::audio_files::is_audio_path(f.path()) {
            continue;
        }
        let name = file_name(f.path());
        let bytes = match f.bytes() {
            Ok(b) => b,
            Err(e) => {
                app.set_error(tf(
                    "interop.status.import_failed",
                    &[("file", &name), ("error", &e)],
                ));
                continue;
            }
        };
        let lower = name.to_lowercase();
        if midi::is_midi_path(Path::new(&lower)) {
            let (at, index) = drop_target(app, pos);
            let empty = app.project().content_end() == Ticks::ZERO;
            import_midi(app, &bytes, &name, false, empty, at, index);
        } else if lower.ends_with(&format!(".{}", dawproject::EXTENSION)) {
            import_dawproject(app, &bytes, &name);
        } else {
            app.set_error(tf("audioclips.drop.unsupported", &[("file", &name)]));
        }
    }
}

/// While a file is dragged over the window: say what dropping it does.
fn paint_drop_hint(app: &CantusApp, ctx: &egui::Context) {
    use crate::kit::text::{self, Elide, Policy, Role, Text};
    use crate::kit::{color::with_alpha, light, Fill, Style, Tok};
    let rect = app
        .arrangement
        .geometry()
        .map(|g| {
            egui::Rect::from_min_max(
                egui::pos2(g.left, g.ruler_top),
                egui::pos2(
                    g.left + g.width,
                    g.bottom().max(g.body_top + space::S8 * 2.0),
                ),
            )
        })
        .unwrap_or_else(|| ctx.content_rect());
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("interop.drop"),
    ));
    // Where it lands, in the selection colour (ADR 0024 §10: a drag shows where it
    // lands), and what dropping does on a plate of its own over it.
    let s = Style::current();
    crate::kit::with_brush(&painter, |b| {
        b.register(
            rect,
            "drop_target",
            &[Tok::Selection, Tok::Panel, Tok::Text],
            &[],
        );
        let r = rect.shrink(space::S1);
        b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(s.t.selection, 0.10)));
        b.rrect_stroke(r, s.r(3.0), 2.0, s.t.selection);
        let tx = Text::key("audioclips.drop.hint", Role::Title, s.t.text)
            .policy(Policy::Elide(Elide::End));
        let w = text::measure(b.ctx(), &tx, s.tk()).x.min(r.width() - 40.0);
        let plate = egui::Rect::from_center_size(r.center(), egui::vec2(w + 32.0, s.s(34.0)));
        light::raised(b, plate, s.r(3.0), s.t.panel, light::RaisedOpts::default());
        let _ = text::paint(b, plate.shrink2(egui::vec2(16.0, 0.0)), &tx, s.tk());
    });
}

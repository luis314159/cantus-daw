//! The application: owns the command bus, the engine handle, selection and view state.
//! Every mutation goes through [`CantusApp::exec`] (ADR 0004); the engine only ever sees
//! compiled snapshots.

use crate::clips::{self, ClipRef};
use crate::design::{self, space};
use crate::i18n::{self, t, tf};
use crate::keyboard::{KeyEvent, KeyboardPiano};
use crate::keymap::{Action, Keymap};
use crate::layout::{self, Layouts};
use crate::recording::Recorder;
use crate::settings::{self, Settings};
use crate::theme;
use crate::views::workspace::{Pane, Workspace};
use crate::views::workspace_ticker::{self as ticker, Reason};
use crate::views::{
    self, arrangement::ArrangementView, arrangement::ClipDragResult, channel_rack::ChannelRackView,
    mixer::MixerView, piano_roll::PianoRollView,
};
use cantus_core::{
    Clip, ClipId, Color, Command, CommandBus, Dock, EffectId, EffectKind, GroupId, InstrumentSpec,
    MergeKey, Origin, Project, Send, StripId, Ticks, TrackId, MAX_GAIN_DB, MIN_GAIN_DB,
};
use cantus_engine::{CaptureTarget, EngineHandle, MidiInputs};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// What the central area shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CentralView {
    PianoRoll,
    /// The channel rack. Beside the piano roll rather than beside the arrangement,
    /// because it is a *second editor for the same clip* and not a second timeline.
    ChannelRack,
    Mixer,
}

impl From<layout::Central> for CentralView {
    fn from(c: layout::Central) -> Self {
        match c {
            layout::Central::PianoRoll => CentralView::PianoRoll,
            layout::Central::ChannelRack => CentralView::ChannelRack,
            layout::Central::Mixer => CentralView::Mixer,
        }
    }
}

impl From<CentralView> for layout::Central {
    fn from(c: CentralView) -> Self {
        match c {
            CentralView::PianoRoll => layout::Central::PianoRoll,
            CentralView::ChannelRack => layout::Central::ChannelRack,
            CentralView::Mixer => layout::Central::Mixer,
        }
    }
}

/// Which view an editing command applies to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EditFocus {
    Arrangement,
    PianoRoll,
    /// The mixer: the arrows walk the strips, M and S act on the selected one (§10).
    Mixer,
}

#[derive(Debug)]
pub struct Status {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

/// The label of `Command::ReplaceProject` (a DAWproject import, a recovery): undoing or
/// redoing it swaps the whole project, which bumps `CantusApp::epoch`.
const REPLACE_PROJECT: &str = "cmd.replace_project";

pub struct CantusApp {
    pub bus: CommandBus,
    pub engine: EngineHandle,
    pub path: Option<PathBuf>,
    pub selected_track: Option<TrackId>,
    /// The clip the piano roll is editing. Always one of [`Self::selected_clips`] when
    /// that is non-empty: the arrangement can have many clips selected, but only one of
    /// them is the one whose notes are on screen.
    pub selected_clip: Option<ClipId>,
    /// The arrangement's selection: every clip an arrangement-wide operation acts on.
    pub selected_clips: Vec<ClipRef>,
    /// A range of *time*, across every track, `(from, to)` with `from < to`.
    ///
    /// It lives here rather than in the arrangement's view state because it is a
    /// selection, like [`selected_clips`](Self::selected_clips), and because everything
    /// that will hang off it — insert and delete space, ripple, quantize starts,
    /// render-selection, consolidate — is an *application* operation reached from a menu
    /// or a key, not something the view does to itself.
    ///
    /// Not in the project: a selection is not part of the piece, any more than which clip
    /// is highlighted is.
    pub time_selection: Option<(Ticks, Ticks)>,
    /// The mixer strip the mixer and the inspector are showing. Follows the selected
    /// track's strip when a track is selected, and can be chosen on its own for a strip
    /// nothing feeds yet.
    pub selected_strip: Option<StripId>,
    /// The mixer's multi-selection (FL's Ctrl+Shift+click): the strips a ganged fader
    /// move, a *route selected here* or a submix act on. Meaningful only while it holds
    /// `selected_strip`; see [`Self::strip_selection`].
    pub selected_strips: Vec<StripId>,
    pub selected_notes: Vec<cantus_core::NoteId>,
    /// Which half of the window the last edit gesture happened in. Cut, copy, paste,
    /// duplicate, delete and the arrow keys all mean different things depending on it, and
    /// "whatever you touched last" is the rule a musician can predict without being told.
    pub focus: EditFocus,
    pub arrangement: ArrangementView,
    pub piano_roll: PianoRollView,
    pub rack: ChannelRackView,
    pub mixer: MixerView,
    /// The Routing tab's matrix (round 25, `views::routing`).
    pub routing_view: views::routing::RoutingView,
    pub inspector: views::inspector::Inspector,
    /// The mix recorder: takes of the master or of a strip, written to WAV (`takes`).
    pub mix_recorder: crate::takes::MixRecorder,
    /// The Aequalis editor window (`views::eq_editor`).
    pub eq_editor: views::eq_editor::EqEditor,
    /// The Comprimo editor window (`views::comp_editor`).
    pub comp_editor: views::comp_editor::CompEditor,
    /// Every other plug-in's editor, and the frame they share (`views::fx`).
    pub fx: views::fx::FxEditors,
    /// Moving work between Cantus and other DAWs: MIDI files, stems, DAWproject.
    pub interop: views::interop::Interop,
    /// File → Demo songs: the picker (round 19).
    pub demos: views::demos::Picker,
    /// The sample browser and the content manager (ADR 0030).
    pub samples: crate::samples::Samples,
    /// Updates from GitHub Releases (ADR 0032): the check, the notice, the window.
    pub update: views::update::UpdateUi,
    /// Plug-ins made to the market's standards, hosted (ADR 0019): what the scans found,
    /// and the open project's instances.
    pub plugins: cantus_host::PluginHost,
    /// Hosted plug-ins that stopped working, waiting for an answer on their card (round
    /// 29, ADR 0063, `views::plugin_notice`).
    pub plugin_crashes: views::plugin_notice::Crashes,
    /// The native instruments' editors (ADR 0023).
    pub inst: views::inst::InstEditors,
    /// Pulsus II's editor (round 17).
    pub pulsus: views::pulsus_editor::State,
    /// What swapping a device keeps beyond what it always keeps (round 20, `crate::swap`).
    pub swap_keep: crate::swap::Keep,
    /// Where the recorder's samples are drained into each frame; reused, never shrunk.
    capture_scratch: Vec<f32>,
    pub central: CentralView,
    /// The window's areas as tabs that dock, split, stack and float (ADR 0024 §5).
    pub workspace: Workspace,
    pub recorder: Recorder,
    /// Every audio source's waveform peaks, computed off this thread (ADR 0025).
    pub peaks: crate::waveform::PeakStore,
    /// Audio recording: the pass in progress and what draws it (`record_session`).
    pub rec: crate::record_session::RecSession,
    /// The device picker, when open, and what it was opened for (`crate::picker`).
    pub picker: Option<crate::picker::PickPurpose>,
    /// The device picker's view: the catalogue, the search, favourites, thumbnails.
    pub picker_view: views::picker::PickerView,
    /// The *Link to…* picker (round 26, ADR 0037).
    pub link_picker: views::link_picker::LinkPicker,
    /// The clip browser's filter and drag (round 26, ADR 0037).
    pub clip_browser: views::clip_browser::ClipBrowser,
    /// The audio chip, its popover, the first-run setup, plug-and-play (round 20).
    pub audio_setup: crate::audio_setup::AudioSetup,
    pub midi_port_names: Vec<String>,
    pub(crate) midi_inputs: Option<MidiInputs>,
    live_track_sent: Option<Option<TrackId>>,
    /// The look as the user set it: light and dark, a base theme for each, and every
    /// change on top (`kit::appearance`). Kept in `appearance.json`; an older `theme.json`
    /// is migrated into it.
    pub appearance: crate::kit::appearance::Appearance,
    /// Where [`Self::store_appearance`] writes. Public so tests can redirect it.
    pub appearance_path: PathBuf,
    appearance_watch: views::appearance_file::Watch,
    /// The appearance page's own state: search, undo, the element picked.
    pub appearance_page: views::appearance::Page,
    /// The window's and the taskbar's icon, in the colour the appearance names (ADR 0046).
    pub window_icon: crate::window_icon::WindowIcon,
    /// Whether the taskbar is light: what the automatic icon colour follows.
    pub taskbar: crate::window_icon::Taskbar,
    /// User preferences, from `settings.json` (`crate::settings`).
    pub settings: Settings,
    /// Where [`Self::store_settings`] writes. Public so tests can redirect it.
    pub settings_path: PathBuf,
    pub show_settings: bool,
    /// The settings window's own state (a pending audio choice, an open list).
    pub settings_page: views::workspace_settings::Page,
    /// True while the appearance page (the colour editor) is open.
    pub show_theme_editor: bool,
    /// Set while the audio device is being reopened, to report the outcome once.
    audio_error: Option<String>,
    /// Every shortcut, from `keymap.toml` (`crate::keymap`).
    pub keymap: Keymap,
    /// Where [`Self::store_keymap`] writes. Public so tests can redirect it.
    pub keymap_path: PathBuf,
    /// Open while the shortcuts dialog is showing; holds the action being rebound.
    pub rebinding: Option<Action>,
    pub show_keymap: bool,
    /// Named window layouts, and the one in use (`crate::layout`).
    pub layouts: Layouts,
    /// Where [`Self::store_layouts`] writes. Public so tests can redirect it away from the
    /// user's real configuration directory.
    pub layouts_path: PathBuf,
    /// Open when the user is naming a layout; holds the text being typed.
    pub naming_layout: Option<String>,
    /// Areas to bring to the front once the dock is drawn (`show_pane`): a view drawn
    /// inside a tab cannot change the dock it is drawn in.
    pub(crate) panes_wanted: Vec<crate::views::workspace::Pane>,
    /// The egui pass whose keys were dispatched (`dispatch_keys` runs once a pass).
    keys_pass: Option<u64>,
    pub(crate) theme_ctx: Option<egui::Context>,
    pub piano_roll_notes_drawn: usize,
    pub keyboard: KeyboardPiano,
    /// Typing keyboard plays the selected track (Ctrl+T toggles; off frees more keys).
    pub keyboard_piano: bool,
    /// Cut, copy and paste: what was copied, and a paste waiting on the system clipboard
    /// (ADR 0036, `crate::clipboard`).
    pub clipboard: crate::clipboard::ClipboardState,
    /// Which arrangement selection — clips or the time range — was touched last; an
    /// arrangement copy takes that one.
    pub last_selection: crate::clipboard::SelKind,
    pub status: Status,
    pub show_stats: bool,
    /// The top bar's own state: tap tempo, pattern mode's place, the RAM reading, the
    /// customise panel (round 28, `views::transport`).
    pub topbar: views::transport::TopBar,
    pub show_about: bool,
    /// The verse at start, while it shows (BACKLOG §6c; `views::verse`).
    pub verse: Option<views::verse::Card>,
    pub show_shortcuts: bool,
    pub request_quit: bool,
    synced_revision: u64,
    /// The effects of the last snapshot sent (`sync_engine` checks the engine has them).
    sent_effects: Vec<cantus_core::EffectId>,
    pub(crate) last_autosave: Instant,
    pub(crate) autosaved_revision: u64,
    /// Where unsaved projects autosave (and saved ones whose folder cannot be written).
    /// Public so tests and the soak harness can redirect it.
    pub autosave_dir: PathBuf,
    /// Autosave this often instead of the settings' interval (tests, the soak harness).
    pub autosave_every: Option<Duration>,
    /// An autosave problem was reported; it is not repeated every interval.
    pub(crate) autosave_warned: bool,
    /// The autosave being written on its thread (`autosave`).
    pub(crate) autosave_job: Option<crate::autosave::Job>,
    /// *Save* written on its thread (`save_job`, round 24): the save in flight, one queued
    /// behind it, and what the *Save changes?* question waits to do.
    pub saving: crate::save_job::Saving,
    /// Bumped whenever the project is replaced (open, new, a demo, a recovery), so a
    /// result about the one before is recognised.
    pub(crate) epoch: u64,
    /// This session's journal (`recovery`): what a crash leaves for the next start.
    pub session: Option<crate::recovery::Session>,
    /// What a session that is gone left, offered at start (`views::recovery`).
    pub recovery: Option<crate::recovery_restore::Offer>,
    /// *Save changes?* asked before New or Open replace unsaved work (`views::unsaved`).
    pub unsaved_prompt: Option<views::unsaved::Then>,
    /// The export dialog and a running export (ADR 0033).
    pub export: views::export::ExportUi,
    auditions: Vec<(TrackId, u8, Instant)>,
    frame_times: Vec<f32>,
    /// Automation (ADR 0027): the last tweaked parameter, the lanes as the knobs read
    /// them, the large editor's state.
    pub automation: crate::automation::AutoUi,
    /// Modulators in the interface (ADR 0057): mapping, the open lists, folded panels.
    pub modulators: views::modulator::ModUi,
    /// A comp being flattened and merged (ADR 0058, review 1).
    pub merge_job: Option<crate::record_session::MergeJob>,
    /// The pitch editor's state (round 28, ADR 0054).
    pub pitch: crate::pitch::PitchUi,
    /// Freezing and bouncing in place (round 29, ADR 0064): the renders, their queue.
    pub freeze: crate::freeze::FreezeState,
    /// Warping (round 29, ADR 0061): imports waiting for their analysis, the selected
    /// marker, the quantise dialog, the warp editor, a bounce on its way.
    pub warp: crate::warp::WarpUi,
    /// MIDI learn and controller mapping (round 29, ADR 0062): the tap, the global map,
    /// the learn mode.
    pub controllers: crate::controllers::ControllerUi,
}

impl CantusApp {
    /// `locale_from_flag` is whatever `--locale` asked for. It is passed in rather than
    /// read here because an explicit flag must win over the stored preference, and only
    /// the caller knows whether one was given.
    ///
    /// This opens everything on the calling thread; the desktop app goes through the
    /// splash instead (`crate::boot`), which reads the files and scans MIDI on a thread.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        project: Option<Project>,
        stress: bool,
        locale_from_flag: Option<i18n::Locale>,
    ) -> Self {
        let files = Files::read(&directories_fallback(), locale_from_flag);
        // The real window shows names in any script: the system's fonts after ours, read
        // when first needed (round 26). Tests and shots never arm it.
        crate::font_fallback::install_for_window(&cc.egui_ctx);
        design::install_style(&cc.egui_ctx);
        let engine = start_engine(&files.settings);
        let midi = engine
            .is_live()
            .then(|| MidiInputs::open_all(engine.midi_sender()));
        let owner = {
            use raw_window_handle::HasWindowHandle;
            cc.window_handle().ok().map(|h| h.as_raw())
        };
        let db = cantus_host::PluginDb::load(&data_dir().join("plugins.json"));
        let mut app = Self::assemble(
            &cc.egui_ctx,
            files,
            project,
            engine,
            midi,
            Some(db),
            owner,
            stress,
        );
        app.start_session(&data_dir());
        app
    }

    /// With or without a device, instruments load beside the window and Exemplar streams
    /// (ADR 0050, review 18), within the limit Settings chose: what the real window does
    /// at start (tests choose).
    pub fn load_beside_the_window(&mut self) {
        Self::load_beside(&mut self.engine, &self.settings);
    }

    fn load_beside(engine: &mut EngineHandle, settings: &Settings) {
        engine.samples.set_background(true);
        engine.samples.set_streaming(true);
        settings.apply_sample_cache_limit();
    }

    /// What the real window starts beside itself, before its first snapshot (second
    /// review #4, #11, #12): the sample cache in the data folder with the limit Settings
    /// chose and its start sweep, instruments loading and streaming beside the window
    /// with or without a device, Exemplar's built-in sounds computed on a thread. Only
    /// [`Self::assemble`] calls it — the boot's path and `new`'s.
    fn start_window_services(engine: &mut EngineHandle, settings: &Settings) {
        Self::load_beside(engine, settings);
        // Exemplar's streamed samples are decoded once into this cache (ADR 0050); setting
        // it starts the sweep.
        cantus_engine::inst::stream::hub().set_cache_dir(data_dir().join("cache").join("samples"));
        // Exemplar's built-in sounds take a few hundred ms to compute: here, beside the
        // window, not in the frame that first makes an Exemplar.
        let _ = std::thread::Builder::new()
            .name("cantus-warm-builtins".into())
            .spawn(|| {
                let _ = cantus_dsp::exemplar::builtin::builtins();
            });
    }

    /// The sample cache's limit, its disk and what it holds — read at most every two
    /// seconds (Settings shows it).
    pub fn sample_cache_room(&mut self) -> cantus_engine::inst::stream::CacheRoom {
        let fresh = self
            .inst
            .sampler
            .cache_size
            .filter(|(at, _)| at.elapsed() < Duration::from_secs(2));
        if let Some((_, room)) = fresh {
            return room;
        }
        let room = cantus_engine::inst::stream::hub().cache_room();
        self.inst.sampler.cache_size = Some((std::time::Instant::now(), room));
        room
    }

    /// Tidy the sample cache beside the window: entries whose source is gone (a pack
    /// removed) or changed, then the least recently used above the limit.
    pub fn sweep_sample_cache(&mut self) {
        let _ = std::thread::Builder::new()
            .name("cantus-cache-sweep".into())
            .spawn(|| {
                let _ = cantus_engine::inst::stream::hub().sweep();
            });
        self.inst.sampler.cache_size = None;
    }

    /// A pack's folder was deleted: its decoded samples leave the cache now, beside the
    /// window (a source whose drive is only away stays — second review #7).
    pub fn sweep_sample_cache_removed(&mut self, dir: PathBuf) {
        let _ = std::thread::Builder::new()
            .name("cantus-cache-sweep".into())
            .spawn(move || {
                let _ = cantus_engine::inst::stream::hub().sweep_removed(dir);
            });
        self.inst.sampler.cache_size = None;
    }

    /// *Clear cache*: every decoded file nothing plays now goes (beside the window); they
    /// are decoded again when needed.
    pub fn clear_sample_cache(&mut self) {
        let _ = std::thread::Builder::new()
            .name("cantus-cache-clear".into())
            .spawn(|| {
                let _ = cantus_engine::inst::stream::hub().clear_cache();
            });
        self.inst.sampler.cache_size = None;
        self.set_status(t("smp.cache.cleared"));
    }

    /// Put the application together from what the boot gathered: the files read, the
    /// engine opened, the MIDI ports scanned, the plug-in database loaded.
    #[allow(clippy::too_many_arguments)]
    pub fn assemble(
        ctx: &egui::Context,
        files: Files,
        project: Option<Project>,
        engine: EngineHandle,
        midi: Option<MidiInputs>,
        plugin_db: Option<cantus_host::PluginDb>,
        owner_window: Option<raw_window_handle::RawWindowHandle>,
        stress: bool,
    ) -> Self {
        let Files {
            settings,
            settings_path,
            appearance,
            appearance_path,
            layouts,
            layouts_path,
            keymap,
            keymap_path,
            controllers,
            controllers_path,
        } = files;
        views::workspace_style::install(ctx);
        ctx.set_zoom_factor(settings.ui_scale());
        crate::kit::logo::set_disk_cache(Some(directories_fallback().join("cache")));
        let peak_cache = directories_fallback().join("cache").join("peaks");
        let mut engine = engine;
        Self::start_window_services(&mut engine, &settings);
        // Chorda's strings for this sample rate, designed beside the window (round 28,
        // ADR 0059): ready before anybody loads the piano.
        cantus_engine::inst::chorda::warm(engine.sample_rate as f32);
        let mut app = Self::from_parts(project, engine, stress);
        app.peaks.set_disk_cache(Some(peak_cache));
        // The pitch editor's analyses, beside the peaks (round 28, ADR 0054).
        app.engine
            .samples
            .pitch()
            .set_disk_cache(Some(directories_fallback().join("cache").join("pitch")));
        // Warping's analyses (transients, tempo, grid) beside them (round 29, ADR 0061).
        app.engine
            .samples
            .warp()
            .set_disk_cache(Some(directories_fallback().join("cache").join("warp")));
        app.mixer.load_prefs(&layouts_path);
        app.settings = settings;
        app.settings_path = settings_path;
        // Where third-party plug-ins run unless the project says (round 29, ADR 0063).
        views::plugin_notice::apply_default(&mut app);
        // Frozen renders' folders and low-latency monitoring (round 29, ADR 0064).
        crate::freeze::wire(&mut app);
        app.appearance = appearance;
        app.appearance_path = appearance_path.clone();
        app.appearance_watch = views::appearance_file::Watch::new(appearance_path);
        app.theme_ctx = Some(ctx.clone());
        app.apply_appearance();
        app.layouts = layouts;
        app.layouts_path = layouts_path;
        // The device picker: the user's favourites and recents, and its pictures' cache.
        app.picker_view
            .load_prefs(crate::picker_prefs::default_path());
        app.picker_view
            .thumbs
            .enable(Some(directories_fallback().join("cache").join("thumbs")));
        app.central = app.layouts.current.central.into();
        app.workspace.load(&app.layouts.current);
        app.keymap = keymap;
        app.keymap_path = keymap_path;
        if let Some(db) = plugin_db {
            app.plugins.db = db;
            app.plugins.db_path = Some(data_dir().join("plugins.json"));
        }
        // Plug-in windows are owned by the main window, so they stay above it.
        app.plugins.owner_window = owner_window;
        // MIDI learn (round 29, ADR 0062): the ports' tap (the boot opened them with one
        // that wakes this window; without ports, one is made here), the global map.
        app.controllers.tap = match &midi {
            Some(inputs) => inputs.tap().clone(),
            None => crate::controllers::waking_tap(ctx),
        };
        app.controllers.global = controllers;
        app.controllers.global_path = Some(controllers_path);
        if let Some(inputs) = midi {
            app.midi_port_names = inputs.names.clone();
            app.midi_inputs = Some(inputs);
        }
        crate::audio_setup::arm_first_run(&mut app);
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
        let plugins =
            cantus_host::PluginHost::new(Some(data_dir().join("plugins.json")), engine.sample_rate);
        let project = project.unwrap_or_else(|| {
            if stress {
                stress_project()
            } else {
                welcome_project()
            }
        });
        let mut app = CantusApp {
            bus: CommandBus::new(project),
            engine,
            path: None,
            selected_track: None,
            selected_clip: None,
            selected_clips: Vec::new(),
            time_selection: None,
            selected_strip: None,
            selected_strips: Vec::new(),
            selected_notes: Vec::new(),
            focus: EditFocus::PianoRoll,
            arrangement: ArrangementView::default(),
            piano_roll: PianoRollView::default(),
            rack: ChannelRackView::default(),
            mixer: MixerView::default(),
            routing_view: Default::default(),
            inspector: Default::default(),
            mix_recorder: crate::takes::MixRecorder::default(),
            eq_editor: views::eq_editor::EqEditor::default(),
            comp_editor: views::comp_editor::CompEditor::default(),
            fx: views::fx::FxEditors::default(),
            interop: Default::default(),
            demos: Default::default(),
            samples: Default::default(),
            update: Default::default(),
            plugins,
            plugin_crashes: Default::default(),
            inst: views::inst::InstEditors::default(),
            pulsus: views::pulsus_editor::State::default(),
            swap_keep: Default::default(),
            capture_scratch: Vec::new(),
            central: CentralView::Mixer,
            workspace: Workspace::default(),
            recorder: Recorder::new(),
            peaks: Default::default(),
            rec: Default::default(),
            picker: None,
            picker_view: Default::default(),
            link_picker: Default::default(),
            clip_browser: Default::default(),
            audio_setup: Default::default(),
            midi_port_names: Vec::new(),
            midi_inputs: None,
            live_track_sent: None,
            appearance: Default::default(),
            appearance_path: views::appearance_file::path(&directories_fallback()),
            appearance_watch: Default::default(),
            appearance_page: Default::default(),
            window_icon: Default::default(),
            taskbar: Default::default(),
            theme_ctx: None,
            settings: Settings::default(),
            settings_path: settings::settings_path(&directories_fallback()),
            show_settings: false,
            settings_page: Default::default(),
            show_theme_editor: false,
            audio_error: None,
            keymap: Keymap::default(),
            keymap_path: crate::keymap::keymap_path(&directories_fallback()),
            rebinding: None,
            show_keymap: false,
            layouts: Layouts::default(),
            layouts_path: layout::layouts_path(&directories_fallback()),
            naming_layout: None,
            panes_wanted: Vec::new(),
            keys_pass: None,
            piano_roll_notes_drawn: 0,
            keyboard: KeyboardPiano::default(),
            keyboard_piano: false,
            clipboard: Default::default(),
            last_selection: Default::default(),
            status: Status {
                text: t("status.ready"),
                error: false,
                at: Instant::now(),
            },
            show_stats: stress,
            topbar: Default::default(),
            show_about: false,
            verse: None,
            show_shortcuts: false,
            request_quit: false,
            synced_revision: u64::MAX,
            sent_effects: Vec::new(),
            last_autosave: Instant::now(),
            autosaved_revision: 0,
            autosave_dir,
            autosave_every: None,
            autosave_warned: false,
            autosave_job: None,
            saving: Default::default(),
            epoch: 0,
            session: None,
            recovery: None,
            unsaved_prompt: None,
            export: Default::default(),
            auditions: Vec::new(),
            frame_times: Vec::with_capacity(120),
            automation: Default::default(),
            modulators: Default::default(),
            merge_job: None,
            pitch: Default::default(),
            freeze: Default::default(),
            warp: Default::default(),
            controllers: Default::default(),
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
        self.note_tweak(&cmd);
        match self.bus.execute(cmd, Origin::User) {
            Ok(()) => true,
            // A frozen sound refused it (ADR 0064): which, and how to give it back.
            Err(cantus_core::CommandError::Frozen(target)) => {
                crate::freeze::refused(self, target);
                false
            }
            Err(e) => {
                self.set_error(e.to_string());
                false
            }
        }
    }

    pub fn exec_merging(&mut self, cmd: Command, key: MergeKey) -> bool {
        self.note_tweak(&cmd);
        match self.bus.execute_merging(cmd, Origin::User, key) {
            Ok(()) => true,
            Err(cantus_core::CommandError::Frozen(target)) => {
                crate::freeze::refused(self, target);
                false
            }
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
        let replaced = self.bus.undo_label_key() == Some(REPLACE_PROJECT);
        self.bus.undo();
        if replaced {
            // Another project is back: ids copied from this one mean other things there.
            self.epoch += 1;
        }
        self.validate_selection();
    }

    pub fn redo(&mut self) {
        let replaced = self.bus.redo_label_key() == Some(REPLACE_PROJECT);
        self.bus.redo();
        if replaced {
            self.epoch += 1;
        }
        self.validate_selection();
    }

    pub(crate) fn validate_selection(&mut self) {
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
        let p = self.bus.project();
        self.selected_clips
            .retain(|(t, c)| p.track(*t).and_then(|t| t.clip(*c)).is_some());
        self.selected_strips.retain(|s| p.strip(*s).is_some());
        if self.selected_strip.is_some_and(|s| p.strip(s).is_none()) {
            self.selected_strip = None;
        }
        if self.selected_track.is_none() {
            self.select_first();
        }
    }

    pub(crate) fn select_first(&mut self) {
        if let Some(t) = self.bus.project().tracks.first() {
            self.selected_track = Some(t.id);
            self.selected_clip = t.clips.first().map(|c| c.id);
        }
    }

    /// Fold the workspace's shape and the current view into the layout, then write the
    /// layouts file. Called whenever a layout changes and once on exit, so what is on disk
    /// is what the user was last looking at.
    pub fn store_layouts(&mut self, _ctx: &egui::Context) {
        self.layouts.sync(&self.workspace, self.central.into());
        if let Err(e) = self.layouts.save(&self.layouts_path) {
            log::warn!("cannot write layouts file: {e}");
        }
    }

    pub fn save_layout_as(&mut self, name: &str, ctx: &egui::Context) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.layouts
            .save_as(name, &self.workspace, self.central.into());
        self.store_layouts(ctx);
        self.set_status(tf("layout.saved", &[("name", &name)]));
    }

    pub fn apply_layout(&mut self, name: &str) {
        if !self.layouts.apply_named(name) {
            return;
        }
        self.central = self.layouts.current.central.into();
        self.workspace.load(&self.layouts.current);
        let label = self.layouts.current.display_name();
        // `store` would sync the live shape back over the one just applied, so the file is
        // written from the layout itself.
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
        self.workspace.load(&self.layouts.current);
        if let Err(e) = self.layouts.save(&self.layouts_path) {
            log::warn!("cannot write layouts file: {e}");
        }
        self.set_status(t("layout.was_reset"));
    }

    /// Bring a view to the front of the lower zone (the `--mixer` and `--rack` flags, the
    /// tests): it becomes the central view and its tab shows.
    pub fn show_central(&mut self, view: CentralView) {
        self.central = view;
        self.workspace.reveal(Pane::of_central(view));
    }

    /// Show the look the appearance describes, now: the mode's theme with the user's
    /// changes on top, installed for every view and every kit painter.
    pub fn apply_appearance(&mut self) {
        let Some(ctx) = self.theme_ctx.clone() else {
            return;
        };
        let dark = views::appearance_file::system_dark(&ctx);
        let t = self.appearance.resolve(dark);
        // The icon's colours first: the *Icon* logo tint reads them as the theme installs.
        let mark = self
            .appearance
            .icon
            .mark_on(&t.kit_tokens(), self.taskbar.light());
        crate::kit::logo::set_icon_colours(mark);
        self.window_icon.want(mark);
        theme::apply(&t, &ctx);
        self.appearance_page.system_dark = dark;
    }

    /// Write the appearance file (it is also what a hand edit or another window changes;
    /// the app picks those up while it runs), and leave the chosen logo where the next
    /// launch's splash finds it.
    pub fn store_appearance(&mut self) {
        if let Err(e) = views::appearance_file::save(&self.appearance, &self.appearance_path) {
            log::warn!("cannot write appearance file: {e}");
        }
        self.appearance_watch.saw_write(&self.appearance_path);
        let logo = self.appearance.logo;
        let themed = logo.tint != crate::kit::logo::Tint::Original;
        let key = (
            logo,
            if themed {
                crate::kit::tokens::generation()
            } else {
                0
            },
        );
        if self.appearance_page.remembered != Some(key) {
            self.appearance_page.remembered = Some(key);
            crate::boot::remember_logo(logo, &crate::kit::tokens::current());
        }
    }

    /// Give the application its egui context (the window's, or a test's): the workspace's
    /// style and the look are installed at once.
    pub fn attach(&mut self, ctx: &egui::Context) {
        self.theme_ctx = Some(ctx.clone());
        views::workspace_style::install(ctx);
        self.apply_appearance();
    }

    /// The scale everything is drawn at. egui applies it to fonts and to the spacing grid
    /// together, so the layout keeps its proportions instead of only the text growing.
    pub fn apply_ui_scale(&mut self, ctx: &egui::Context, scale: f32) {
        let scale = scale.clamp(settings::MIN_UI_SCALE, settings::MAX_UI_SCALE);
        ctx.set_zoom_factor(scale);
        self.settings.ui_scale = scale;
        self.store_settings();
        self.set_status(tf(
            "workspace.zoom_now",
            &[("pct", &format!("{:.0}", scale * 100.0))],
        ));
    }

    /// Ctrl + / Ctrl −: the next step of the global zoom (ADR 0024 §11).
    pub fn step_ui_scale(&mut self, ctx: &egui::Context, up: bool) {
        let now = self.settings.ui_scale();
        let next = settings::ui_scale_step(now, up);
        self.apply_ui_scale(ctx, next);
    }

    pub fn store_settings(&mut self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            log::warn!("cannot write settings file: {e}");
        }
    }

    /// Reopen the audio device with the current settings.
    ///
    /// Two things keep a settings change from leaving the user in silence, which is the
    /// worst thing a settings dialog in an audio application can do. `EngineHandle::start`
    /// falls back to the system default when the named device is not there — unplugged
    /// since the settings were saved, say — and if it cannot open anything at all, the
    /// engine that is already running is left alone. Either way the status line names the
    /// device actually in use, so a fallback is visible rather than mysterious.
    pub fn restart_audio(&mut self) {
        // The running device is closed before the new one opens (round 21): ASIO loads one
        // driver at a time, and ASIO4ALL cannot open a device whose endpoint Cantus still
        // holds through WASAPI — opening the new engine first left it silent.
        // The loaded samples outlive the device (ADR 0050 §7): a new device does not decode
        // an instrument again.
        let samples = std::mem::take(&mut self.engine.samples);
        self.engine = EngineHandle::dummy();
        self.engine.samples = samples;
        match EngineHandle::start(&self.settings.audio.to_config()) {
            Ok(engine) => {
                let samples = std::mem::take(&mut self.engine.samples);
                self.engine = engine;
                self.engine.samples = samples;
                self.engine
                    .set_record_offset(self.settings.audio.record_offset_frames);
                // Hosted plug-ins load again, for the new engine and its rate.
                self.plugins.reload_all(self.engine.sample_rate);
                // Chorda's strings for the new rate (a rate it has met before is kept).
                cantus_engine::inst::chorda::warm(self.engine.sample_rate as f32);
                self.midi_inputs = None;
                self.controllers.reopened();
                if self.engine.is_live() {
                    let inputs = MidiInputs::open_with(
                        self.engine.midi_sender(),
                        self.controllers.tap.clone(),
                    );
                    self.midi_port_names = inputs.names.clone();
                    self.midi_inputs = Some(inputs);
                }
                // The new engine has never seen this project.
                self.synced_revision = u64::MAX;
                self.live_track_sent = None;
                // Nor its frozen renders' folders, nor low-latency monitoring (ADR 0064).
                crate::freeze::engine_replaced(self);
                self.sync_engine();
                self.sync_live_track();
                crate::audio_setup::apply_routing(self);
                self.audio_error = None;
                self.set_status(tf(
                    "settings.audio_opened",
                    &[
                        ("device", &self.engine.device_name),
                        ("rate", &(self.engine.sample_rate as u32)),
                    ],
                ));
            }
            Err(e) => {
                self.audio_error = Some(e.to_string());
                self.set_error(tf("settings.audio_failed", &[("error", &e.to_string())]));
            }
        }
        self.store_settings();
    }

    /// Rescan MIDI inputs — what a user reaches for after plugging a keyboard in.
    pub fn rescan_midi(&mut self) {
        if !self.engine.is_live() {
            return;
        }
        self.midi_inputs = None;
        self.controllers.reopened();
        let inputs = MidiInputs::open_with(self.engine.midi_sender(), self.controllers.tap.clone());
        self.midi_port_names = inputs.names.clone();
        self.midi_inputs = Some(inputs);
        self.set_status(tf(
            "settings.midi_rescanned",
            &[("n", &self.midi_port_names.len())],
        ));
    }

    pub fn set_locale(&mut self, locale: i18n::Locale) {
        i18n::set_locale(locale);
        self.settings.locale = Some(locale.code().to_string());
        self.store_settings();
    }

    pub fn store_keymap(&mut self) {
        if let Err(e) = self.keymap.save(&self.keymap_path) {
            log::warn!("cannot write keymap file: {e}");
        }
    }

    pub fn apply_keymap_preset(&mut self, preset: crate::keymap::Preset) {
        self.keymap = preset.keymap();
        self.store_keymap();
        self.set_status(t(preset.label_key()));
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

    /// Show a strip in the mixer and the inspector. If a track feeds it, that track is
    /// selected too, so the inspector shows the instrument playing into the strip.
    pub fn select_strip(&mut self, id: StripId) {
        self.selected_strip = Some(id);
        self.selected_strips.clear();
        let track = self.project().tracks_on_strip(id).next().map(|t| t.id);
        if let Some(t) = track {
            self.select_track(t);
            self.selected_strip = Some(id);
        }
        self.inspector.follow(views::inspector::Subject::Strip(id));
    }

    /// A click on the master on the desk: the inspector shows the master's chain (ADR
    /// 0026). The master is not a strip, so the strip selection is let go.
    pub fn select_master(&mut self) {
        self.selected_strip = None;
        self.selected_strips.clear();
        self.inspector.follow(views::inspector::Subject::Master);
    }

    /// The strips a mixer operation acts on: the multi-selection while the selected strip
    /// is part of it, else the selected strip alone.
    pub fn strip_selection(&self) -> Vec<StripId> {
        match self.selected_strip {
            Some(s) if self.selected_strips.contains(&s) => self.selected_strips.clone(),
            Some(s) => vec![s],
            None => Vec::new(),
        }
    }

    /// Ctrl+click on a strip: add it to the multi-selection, or take it out again.
    pub fn toggle_strip_selected(&mut self, id: StripId) {
        let mut multi = self.strip_selection();
        match multi.iter().position(|s| *s == id) {
            Some(i) => {
                multi.remove(i);
                if self.selected_strip == Some(id) {
                    if let Some(first) = multi.first().copied() {
                        self.select_strip(first);
                    }
                }
            }
            None => {
                multi.push(id);
                self.select_strip(id);
            }
        }
        self.selected_strips = multi;
    }

    // ---- mixer: faders in a group, routing, sends, submixes -----------------------

    /// A fader move from the mixer. With several strips selected, every one of them
    /// moves by the same amount — FL's ganged faders — as one gesture and one undo step.
    pub fn set_strip_gain(&mut self, id: StripId, gain_db: f32) {
        let selection = self.strip_selection();
        let Some(old) = self.project().strip(id).map(|s| s.gain_db) else {
            return;
        };
        if selection.len() > 1 && selection.contains(&id) {
            let delta = gain_db - old;
            let commands: Vec<Command> = selection
                .iter()
                .filter_map(|s| {
                    let current = self.project().strip(*s)?.gain_db;
                    let target = if *s == id { gain_db } else { current + delta };
                    Some(Command::SetStripGain {
                        id: *s,
                        gain_db: target.clamp(MIN_GAIN_DB, MAX_GAIN_DB),
                    })
                })
                .collect();
            self.exec_merging(
                Command::Batch {
                    label_key: "cmd.set_strip_gain".into(),
                    commands,
                },
                MergeKey::of(&("sgain-gang", id.0)),
            );
        } else {
            self.exec_merging(
                Command::SetStripGain { id, gain_db },
                MergeKey::of(&("sgain", id.0)),
            );
        }
    }

    /// Route every strip in `strips` into `target` (or the master), skipping the target
    /// itself and any route that would loop. One undo step (`routing_actions`: the
    /// destination becomes each strip's only output).
    pub fn route_strips_to(&mut self, strips: &[StripId], target: Option<StripId>) -> bool {
        self.replace_output(strips, target.unwrap_or(StripId::MASTER))
    }

    /// Add a unity send from every strip in `strips` into `target`, skipping the target,
    /// sends that already exist and routes that would loop. One undo step
    /// (`routing_actions`).
    pub fn send_strips_to(&mut self, strips: &[StripId], target: StripId) -> bool {
        self.add_sends(strips, target)
    }

    pub fn set_strip_send(&mut self, id: StripId, send: Send) {
        self.exec_merging(
            Command::SetStripSend { id, send },
            MergeKey::of(&("send", id.0, send.to.0)),
        );
    }

    pub fn remove_strip_send(&mut self, id: StripId, to: StripId) {
        self.exec(Command::RemoveStripSend { id, to });
    }

    /// FL's *create submix*: a new strip right after the last of `strips`, fed by all of
    /// them and going to the master — a group, in the one tree the mixer has (ADR 0014).
    /// Selected afterwards, so the next thing typed is its name.
    pub fn create_submix(&mut self, strips: &[StripId]) -> Option<StripId> {
        let members: Vec<StripId> = strips
            .iter()
            .copied()
            .filter(|s| self.project().strip(*s).is_some())
            .collect();
        if members.is_empty() {
            return None;
        }
        let index = members
            .iter()
            .filter_map(|s| self.project().strip_index(*s))
            .max()
            .unwrap_or(0)
            + 1;
        let strip = self.bus.new_strip(t("mixer.submix_name"));
        let id = strip.id;
        let mut commands = vec![Command::InsertStrip {
            index,
            strip: Box::new(strip),
        }];
        commands.extend(members.iter().map(|m| Command::SetStripOutput {
            id: *m,
            output: Some(id),
        }));
        if self.exec(Command::Batch {
            label_key: "cmd.create_submix".into(),
            commands,
        }) {
            self.select_strip(id);
            Some(id)
        } else {
            None
        }
    }

    /// Move one strip `delta` places along its dock (the strip menu, Shift+wheel): the
    /// rules of `routing::order::nudge` — docks respected, a group hopped over, a member
    /// at its group's edge stepping out.
    pub fn move_strip(&mut self, id: StripId, delta: i32) {
        let cmd = cantus_core::routing::order::nudge(self.project(), &[id], delta);
        if let Some(cmd) = cmd {
            self.exec(cmd);
        }
    }

    pub fn remove_strip(&mut self, id: StripId) {
        if self.exec(Command::RemoveStrip { id }) {
            if self.selected_strip == Some(id) {
                self.selected_strip = None;
            }
            self.selected_strips.retain(|s| *s != id);
        }
    }

    pub fn unmute_all_strips(&mut self) {
        let commands: Vec<Command> = self
            .project()
            .strips
            .iter()
            .filter(|s| s.muted)
            .map(|s| Command::SetStripMute {
                id: s.id,
                muted: false,
            })
            .collect();
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.unmute_strips".into(),
                commands,
            });
        }
    }

    pub fn clear_strip_solos(&mut self) {
        let commands: Vec<Command> = self
            .project()
            .strips
            .iter()
            .filter(|s| s.soloed)
            .map(|s| Command::SetStripSolo {
                id: s.id,
                soloed: false,
            })
            .collect();
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.clear_strip_solos".into(),
                commands,
            });
        }
    }

    // ---- mixer: docks and groups ---------------------------------------------------

    /// Pin strips to a dock, or let them scroll. One undo step.
    pub fn set_strips_dock(&mut self, strips: &[StripId], dock: Dock) {
        let commands: Vec<Command> = strips
            .iter()
            .filter(|s| self.project().strip(**s).is_some_and(|st| st.dock != dock))
            .map(|s| Command::SetStripDock { id: *s, dock })
            .collect();
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.set_strip_dock".into(),
                commands,
            });
        }
    }

    /// Commands that bring `members` together in desk order right after the first of
    /// them, so a group reads as one run behind one separator, and into the first one's
    /// dock. Moves are applied in order, each to the slot after the last, which keeps
    /// every later index valid.
    fn gather_commands(&self, members: &[StripId]) -> Vec<Command> {
        let p = self.project();
        let mut ordered: Vec<(usize, StripId)> = members
            .iter()
            .filter_map(|s| p.strip_index(*s).map(|i| (i, *s)))
            .collect();
        ordered.sort_unstable();
        ordered.dedup();
        let Some(&(first, head)) = ordered.first() else {
            return Vec::new();
        };
        let dock = p.strip(head).map(|s| s.dock).unwrap_or_default();
        let mut commands = Vec::new();
        for (k, (_, id)) in ordered.iter().enumerate() {
            commands.push(Command::MoveStrip {
                id: *id,
                index: first + k,
            });
            if p.strip(*id).is_some_and(|s| s.dock != dock) {
                commands.push(Command::SetStripDock { id: *id, dock });
            }
        }
        commands
    }

    /// A new group from `strips`: named, coloured with the next palette colour, members
    /// gathered into one run. One undo step. Returns the group.
    pub fn create_strip_group(&mut self, strips: &[StripId]) -> Option<GroupId> {
        if strips.is_empty() {
            return None;
        }
        let n = self.project().strip_groups.len();
        let color = Color::PALETTE[(n + 3) % Color::PALETTE.len()];
        let name = tf("mixer.group.default_name", &[("n", &(n + 1))]);
        let id = GroupId(self.bus.alloc_id());
        let group = cantus_core::StripGroup {
            id,
            name,
            color,
            collapsed: false,
        };
        let mut commands = vec![Command::InsertStripGroup {
            index: n,
            group: Box::new(group),
        }];
        commands.extend(self.gather_commands(strips));
        commands.extend(strips.iter().map(|s| Command::SetStripGroup {
            id: *s,
            group: Some(id),
        }));
        self.exec(Command::Batch {
            label_key: "cmd.insert_strip_group".into(),
            commands,
        })
        .then_some(id)
    }

    /// Put strips into an existing group, gathered beside its other members.
    pub fn add_strips_to_group(&mut self, strips: &[StripId], group: GroupId) {
        let mut members: Vec<StripId> = self
            .project()
            .strips_in_group(group)
            .map(|s| s.id)
            .collect();
        members.extend(
            strips
                .iter()
                .filter(|s| !members.contains(s))
                .copied()
                .collect::<Vec<_>>(),
        );
        let mut commands = self.gather_commands(&members);
        commands.extend(strips.iter().map(|s| Command::SetStripGroup {
            id: *s,
            group: Some(group),
        }));
        self.exec(Command::Batch {
            label_key: "cmd.set_strip_group".into(),
            commands,
        });
    }

    pub fn remove_strips_from_group(&mut self, strips: &[StripId]) {
        let commands: Vec<Command> = strips
            .iter()
            .map(|s| Command::SetStripGroup {
                id: *s,
                group: None,
            })
            .collect();
        self.exec(Command::Batch {
            label_key: "cmd.set_strip_group".into(),
            commands,
        });
    }

    /// Take every member out and remove the group: the strips stay where they are.
    pub fn dissolve_strip_group(&mut self, group: GroupId) {
        let mut commands: Vec<Command> = self
            .project()
            .strips_in_group(group)
            .map(|s| Command::SetStripGroup {
                id: s.id,
                group: None,
            })
            .collect();
        commands.push(Command::RemoveStripGroup { id: group });
        self.exec(Command::Batch {
            label_key: "cmd.remove_strip_group".into(),
            commands,
        });
    }

    /// `--editor eq|comp`: an effect on the selected track's strip, set up to show what its
    /// editor does, with the editor open.
    pub fn demo_editor(&mut self, kind: &str) {
        if kind.starts_with("pulsus") {
            views::pulsus_editor::demo(self, kind);
            return;
        }
        if let Some(kind) = views::inst::kind_from_name(kind) {
            self.demo_instrument(kind);
            return;
        }
        let Some(strip) = self
            .selected_track
            .and_then(|t| self.project().track(t))
            .and_then(|t| t.strip)
        else {
            return;
        };
        let kind = match kind {
            "comp" | "compressor" => EffectKind::Compressor,
            "limiter" => EffectKind::Limiter,
            "space" | "reverb" => EffectKind::Space,
            "multiband" => EffectKind::Multiband,
            "deesser" => EffectKind::DeEsser,
            "gate" => EffectKind::Gate,
            "stereo" | "latitudo" => EffectKind::Stereo,
            "maximizer" | "culmen" => EffectKind::Maximizer,
            "saturator" | "fervor" => EffectKind::Saturator,
            "concentus" => EffectKind::Chorus,
            "flexus" => EffectKind::Flanger,
            "phasis" => EffectKind::Phaser,
            "tremor" => EffectKind::Tremolo,
            "vibrans" => EffectKind::Vibrato,
            "anulus" => EffectKind::RingMod,
            "translatio" => EffectKind::FreqShift,
            "fluctus" => EffectKind::VolumeShaper,
            "tonsor" => EffectKind::Clipper,
            "impetus" => EffectKind::Transient,
            "discrimen" => EffectKind::TransientSplit,
            "speculum" => EffectKind::Analyzer,
            "tonus" => EffectKind::AutoTune,
            _ => EffectKind::Eq,
        };
        self.add_effect(strip, kind);
        let Some(effect) = self
            .project()
            .effects_of(strip)
            .and_then(|e| e.last())
            .map(|e| e.id)
        else {
            return;
        };
        if kind == EffectKind::Eq {
            use cantus_dsp::EqShape;
            self.add_eq_band(strip, effect, EqShape::LowCut, 60.0, 0.0);
            self.add_eq_band(strip, effect, EqShape::Bell, 320.0, -4.0);
            self.add_eq_band(strip, effect, EqShape::Bell, 2_800.0, 5.0);
            self.add_eq_band(strip, effect, EqShape::HighShelf, 9_000.0, 3.0);
        } else if kind.param("thr").is_some() {
            self.exec(Command::SetEffectParam {
                strip,
                effect,
                key: "thr".into(),
                value: -30.0,
            });
        }
        views::fx::demo_setup(self, strip, effect, kind);
        self.open_effect_editor(strip, effect);
    }

    // ---- the mix recorder ----------------------------------------------------------

    /// Where takes go: beside the project when it has been saved, else the app's data
    /// folder.
    pub fn recordings_dir(&self) -> PathBuf {
        match &self.path {
            Some(p) => {
                let stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Cantus".into());
                p.with_file_name(format!("{stem} {}", t("take.folder")))
            }
            None => directories_fallback().join("recordings"),
        }
    }

    /// What a take of `target` is called.
    pub fn capture_label(&self, target: CaptureTarget) -> String {
        match target {
            CaptureTarget::Off => String::new(),
            CaptureTarget::Master => t("mixer.master"),
            // An effect's host named as a strip: the master's chain (ADR 0026).
            CaptureTarget::Strip(id) if id.is_master() => t("mixer.master"),
            CaptureTarget::Strip(id) => {
                let n = self.project().strip_number(id).unwrap_or(0);
                let name = self.project().strip(id).map(|s| {
                    if s.name.is_empty() {
                        self.project()
                            .tracks_on_strip(id)
                            .next()
                            .map(|t| t.name.clone())
                            .unwrap_or_default()
                    } else {
                        s.name.clone()
                    }
                });
                match name.filter(|n| !n.is_empty()) {
                    Some(name) => format!("{} · {name}", tf("inspector.strip_n", &[("n", &n)])),
                    None => tf("inspector.strip_n", &[("n", &n)]),
                }
            }
        }
    }

    /// Whether the recorder is taking `target` right now.
    pub fn capturing(&self, target: CaptureTarget) -> bool {
        self.mix_recorder.is_recording() && self.engine.capture_target() == target
    }

    /// The ● on a strip or the master: start a take of it, or stop the one running. A
    /// take of another target is stopped first — one take at a time.
    pub fn toggle_capture(&mut self, target: CaptureTarget) {
        let same = self.capturing(target);
        self.stop_capture();
        if same {
            return;
        }
        if !self.engine.can_capture() {
            self.set_error(t("take.no_device"));
            return;
        }
        let label = self.capture_label(target);
        let dir = self.recordings_dir();
        match self
            .mix_recorder
            .start(&label, &dir, self.engine.sample_rate.round() as u32)
        {
            Ok(_) => {
                self.engine.set_capture(target);
                self.set_status(tf("take.started", &[("what", &label)]));
            }
            Err(e) => self.set_error(tf("take.failed", &[("error", &e)])),
        }
    }

    pub fn stop_capture(&mut self) {
        if !self.mix_recorder.is_recording() {
            return;
        }
        self.engine.set_capture(CaptureTarget::Off);
        self.pump_capture();
        match self.mix_recorder.stop() {
            Some(Ok(take)) => self.set_status(tf(
                "take.saved",
                &[
                    ("what", &take.label),
                    ("seconds", &format!("{:.1}", take.seconds)),
                ],
            )),
            Some(Err(e)) => self.set_error(tf("take.failed", &[("error", &e)])),
            None => {}
        }
    }

    /// Move whatever the engine recorded since the last frame to the take's writer.
    fn pump_capture(&mut self) {
        self.capture_scratch.clear();
        if self.engine.drain_capture(&mut self.capture_scratch) > 0 {
            self.mix_recorder.pump(&self.capture_scratch);
        }
    }

    pub fn toggle_strip_solo(&mut self, id: StripId) {
        if let Some(on) = self.project().strip(id).map(|s| s.soloed) {
            self.exec(Command::SetStripSolo { id, soloed: !on });
        }
    }

    pub fn toggle_strip_mute(&mut self, id: StripId) {
        if let Some(on) = self.project().strip(id).map(|s| s.muted) {
            self.exec(Command::SetStripMute { id, muted: !on });
        }
    }

    /// The strip the inspector's mixer section is about: the selected strip, else the
    /// selected track's.
    pub fn inspected_strip(&self) -> Option<StripId> {
        self.selected_strip.or_else(|| {
            self.selected_track
                .and_then(|t| self.project().track(t))
                .and_then(|t| t.strip)
        })
    }

    /// Add a strip at the end of the desk and select it.
    pub fn add_strip(&mut self) {
        let strip = self.bus.new_strip("");
        let id = strip.id;
        let index = self.project().strips.len();
        if self.exec(Command::InsertStrip {
            index,
            strip: Box::new(strip),
        }) {
            self.selected_strip = Some(id);
        }
    }

    /// Route a track to a strip (or to the master), and follow it in the mixer.
    pub fn route_track(&mut self, track: TrackId, strip: Option<StripId>) {
        if self.exec(Command::SetTrackStrip { id: track, strip }) {
            self.selected_strip = strip;
        }
    }

    pub fn select_track(&mut self, id: TrackId) {
        self.inspector.follow(views::inspector::Subject::Track(id));
        if self.selected_track != Some(id) {
            for ev in self.keyboard.release_all() {
                self.apply_key_event(ev);
            }
            self.selected_track = Some(id);
            self.selected_strip = self.project().track(id).and_then(|t| t.strip);
            // In pattern mode, the track's clip in the pattern playing, or none: choosing a
            // channel never changes the pattern (FL keeps it until another is chosen; review
            // 2: its first clip elsewhere made the playhead jump there).
            let in_pattern = self.topbar.pattern.region().map(|r| r.clip_of(id));
            self.selected_clip = match in_pattern {
                Some(clip) => clip,
                None => self
                    .project()
                    .track(id)
                    .and_then(|t| t.clips.first().map(|c| c.id)),
            };
            self.selected_clips = match (self.selected_track, self.selected_clip) {
                (Some(t), Some(c)) => vec![(t, c)],
                _ => Vec::new(),
            };
            self.selected_notes.clear();
        }
    }

    pub fn select_clip(&mut self, track: TrackId, clip: ClipId) {
        self.select_track(track);
        self.inspector
            .follow(views::inspector::Subject::Clip(track, clip));
        self.selected_clips = vec![(track, clip)];
        self.last_selection = crate::clipboard::SelKind::Clips;
        if self.selected_clip != Some(clip) {
            self.selected_clip = Some(clip);
            self.selected_notes.clear();
        }
        let view = self
            .project()
            .track(track)
            .and_then(|t| t.clip(clip))
            .map(|c| {
                // Centre the view on the clip's notes, and open on the *window* rather than on
                // content tick zero: a clip that has been trimmed or slipped has its music
                // somewhere else in the content, and empty grid would look like it had gone.
                let pitches = c.notes.iter().fold(None, |acc: Option<(u8, u8)>, n| {
                    Some(acc.map_or((n.pitch, n.pitch), |(l, h)| {
                        (l.min(n.pitch), h.max(n.pitch))
                    }))
                });
                (pitches, c.content_start().as_beats_f64() as f32)
            });
        if let Some((pitches, scroll)) = view {
            // Centre the *rows*, not the pitches: on a kit the two are different numbers,
            // and a kit with twelve pads scrolled to "pitch 60" would show empty grid.
            let axis = self.note_axis();
            if let Some((lo, hi)) = pitches {
                let rows = axis.len() as f32;
                let mid = match (axis.lane_of(lo), axis.lane_of(hi)) {
                    (Some(a), Some(b)) => (a + b) as f32 / 2.0,
                    _ => rows * 0.5,
                };
                let half = if axis.pads() { rows * 0.5 } else { 14.0 };
                self.piano_roll.top_lane = (mid + half + 1.0).clamp(1.0, rows);
            }
            self.piano_roll.scroll_beats = scroll;
        }
    }

    /// The vertical axis of the clip the piano roll is editing.
    ///
    /// Rebuilt on demand rather than cached: it is a handful of entries, and a cached copy
    /// would be one more thing that can be stale the frame after a pad is renamed. Every
    /// vertical operation asks for it, so there is exactly one answer to "what is above
    /// this note".
    pub fn note_axis(&self) -> views::piano_roll::Axis {
        let Some((t, c)) = self.selected_track.zip(self.selected_clip) else {
            return views::piano_roll::Axis::default();
        };
        match self
            .project()
            .track(t)
            .and_then(|track| track.clip(c).map(|clip| (track, clip)))
        {
            Some((track, clip)) => views::piano_roll::Axis::build(&track.instrument, clip),
            None => views::piano_roll::Axis::default(),
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
                start: crate::views::bars::snap_content(
                    self.project(),
                    n.start,
                    grid,
                    clip.start - clip.offset,
                ),
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

    // ---- clip selection and editing --------------------------------------------------

    /// The clips an arrangement-wide operation acts on.
    pub fn clip_selection(&self) -> Vec<ClipRef> {
        if !self.selected_clips.is_empty() {
            return self.selected_clips.clone();
        }
        match (self.selected_track, self.selected_clip) {
            (Some(t), Some(c)) => vec![(t, c)],
            _ => Vec::new(),
        }
    }

    /// Ctrl+click: add the clip to the selection, or take it out again.
    pub fn toggle_clip_selection(&mut self, track: TrackId, clip: ClipId) {
        self.last_selection = crate::clipboard::SelKind::Clips;
        if let Some(i) = self.selected_clips.iter().position(|r| *r == (track, clip)) {
            self.selected_clips.remove(i);
            // The piano roll cannot edit a clip that is no longer selected; hand it
            // whatever is still selected rather than leaving it showing a ghost.
            if self.selected_clip == Some(clip) {
                if let Some((t, c)) = self.selected_clips.first().copied() {
                    self.selected_track = Some(t);
                    self.selected_clip = Some(c);
                }
                self.selected_notes.clear();
            }
        } else {
            self.selected_clips.push((track, clip));
            self.selected_track = Some(track);
            self.selected_clip = Some(clip);
            self.selected_notes.clear();
        }
    }

    /// Replace the whole clip selection — what a marquee ends with.
    pub fn set_clip_selection(&mut self, clips: Vec<ClipRef>) {
        self.selected_clips = clips;
        self.last_selection = crate::clipboard::SelKind::Clips;
        if let Some((t, c)) = self.selected_clips.first().copied() {
            self.selected_track = Some(t);
            if self.selected_clip != Some(c) {
                self.selected_clip = Some(c);
                self.selected_notes.clear();
            }
        }
    }

    /// Set the time selection, normalising the ends and dropping an empty range.
    ///
    /// An empty selection is `None`, never `Some(t, t)`: every operation that hangs off
    /// this has to answer "is there a selection", and a zero-length one that answers yes
    /// is a bug in every one of them at once.
    pub fn set_time_selection(&mut self, from: Ticks, to: Ticks) {
        let (a, b) = if from <= to { (from, to) } else { (to, from) };
        let a = a.max(Ticks::ZERO);
        self.time_selection = (b > a).then_some((a, b));
        self.last_selection = crate::clipboard::SelKind::Time;
    }

    pub fn clear_time_selection(&mut self) {
        self.time_selection = None;
        self.last_selection = crate::clipboard::SelKind::Clips;
    }

    /// Put the loop brace on the time selection and switch looping on.
    ///
    /// Both, not one: a loop region you have to remember to enable separately is the sort
    /// of two-step that gets blamed on the transport.
    pub fn loop_to_selection(&mut self) {
        let Some((from, to)) = self.time_selection else {
            return;
        };
        self.exec(Command::Batch {
            label_key: "cmd.loop_to_selection".into(),
            commands: vec![
                Command::SetLoopRegion {
                    region: Some(cantus_core::LoopRegion {
                        start: from,
                        end: to,
                    }),
                },
                Command::SetLoopEnabled { enabled: true },
            ],
        });
    }

    /// Fit the time selection to the arrangement's width.
    ///
    /// A view change, so it is not a command and does not undo — nothing about the piece
    /// changed. A little margin on each side, because a selection whose edges are exactly
    /// on the edges of the window cannot be grabbed.
    pub fn zoom_to_selection(&mut self) {
        let Some((from, to)) = self.time_selection else {
            return;
        };
        let Some(geo) = self.arrangement.geometry() else {
            return;
        };
        let width = geo.width;
        let beats = (to - from).as_beats_f64() as f32;
        if beats <= 0.0 || width <= 0.0 {
            return;
        }
        let margin = 0.92;
        self.arrangement.px_per_beat = (width * margin / beats).clamp(2.0, 400.0);
        let visible = width / self.arrangement.px_per_beat;
        let centre = (from.as_beats_f64() as f32 + to.as_beats_f64() as f32) * 0.5;
        self.arrangement.scroll_beats = (centre - visible * 0.5).max(0.0);
    }

    /// Give every track the same height. The divider is for aiming; this is for agreeing.
    pub fn set_all_track_heights(&mut self, height: f32) {
        let commands: Vec<Command> = self
            .project()
            .tracks
            .iter()
            .filter(|t| (t.height() - height).abs() > 0.001)
            .map(|t| Command::SetTrackHeight { id: t.id, height })
            .collect();
        if commands.is_empty() {
            return;
        }
        self.exec(Command::Batch {
            label_key: "cmd.set_track_height".into(),
            commands,
        });
    }

    /// Move the selection one track up or down the list.
    pub fn step_track(&mut self, delta: i32) {
        let tracks = self.project().tracks.len();
        if tracks == 0 {
            return;
        }
        let current = self
            .selected_track
            .and_then(|id| self.project().track_index(id))
            .map(|i| i as i32)
            .unwrap_or(if delta > 0 { -1 } else { tracks as i32 });
        let next = (current + delta).clamp(0, tracks as i32 - 1) as usize;
        let id = self.project().tracks[next].id;
        self.select_track(id);
    }

    pub fn clear_clip_selection(&mut self) {
        self.selected_clips.clear();
    }

    /// Move the mixer's selection one strip left or right along the desk.
    pub fn step_strip(&mut self, delta: i32) {
        let strips: Vec<StripId> = self.project().strips.iter().map(|s| s.id).collect();
        if strips.is_empty() {
            return;
        }
        let current = self
            .selected_strip
            .and_then(|id| strips.iter().position(|s| *s == id))
            .map(|i| i as i32)
            .unwrap_or(if delta > 0 { -1 } else { strips.len() as i32 });
        let next = (current + delta).clamp(0, strips.len() as i32 - 1) as usize;
        self.select_strip(strips[next]);
        self.focus = EditFocus::Mixer;
    }

    /// Show or hide an area of the workspace (F5, F6, F7, F9, I).
    pub fn toggle_pane(&mut self, pane: Pane) {
        self.workspace.toggle(pane);
        if let Some(c) = pane.central() {
            if self.workspace.is_visible(pane) {
                self.central = c;
            }
        }
    }

    /// Bring an area to the front if it is not on screen. Safe from inside a tab (the
    /// dock is checked out while its tabs draw): the request is kept and applied once the
    /// dock is drawn (`views::workspace_frame::show`).
    pub fn show_pane(&mut self, pane: Pane) {
        if !self.panes_wanted.contains(&pane) {
            self.panes_wanted.push(pane);
        }
    }

    /// Apply [`Self::show_pane`]'s requests (after the dock is drawn).
    pub(crate) fn apply_wanted_panes(&mut self) {
        for pane in std::mem::take(&mut self.panes_wanted) {
            if !self.workspace.is_visible(pane) {
                self.toggle_pane(pane);
            }
        }
    }

    /// "Pick an element": turn the eyedropper on or off (ADR 0024 §2). While it is on,
    /// clicking anything selects it and opens its colours in the appearance page.
    pub fn toggle_pick(&mut self) {
        self.appearance_page.picking = !self.appearance_page.picking;
        if let Some(ctx) = &self.theme_ctx {
            crate::kit::inspect::set_enabled(ctx, self.appearance_page.picking);
        }
    }

    pub fn focus_arrangement(&mut self) {
        self.focus = EditFocus::Arrangement;
    }

    pub fn focus_piano_roll(&mut self) {
        self.focus = EditFocus::PianoRoll;
    }

    /// Whether an edit command should act on clips rather than on notes.
    ///
    /// Editing in the arrangement and then pressing Delete must delete the clip; editing
    /// in the piano roll and pressing Delete must delete the notes. Nothing else about
    /// the two selections tells them apart — both are usually non-empty — so the rule is
    /// simply which one the user touched last.
    pub fn editing_clips(&self) -> bool {
        self.focus == EditFocus::Arrangement && !self.clip_selection().is_empty()
    }

    pub fn select_all_clips(&mut self) {
        let all: Vec<ClipRef> = self
            .project()
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.id)))
            .collect();
        self.set_clip_selection(all);
    }

    /// One arrow-key press: clips or notes, by the grid or by the bar.
    fn nudge(&mut self, direction: i64, by_bar: bool) {
        // A bar of the metre in force at the playhead (ADR 0060).
        let bar = crate::views::bars::bar_len(self.project(), self.engine.position());
        if self.editing_clips() {
            let step = if by_bar { bar } else { self.arrangement.grid };
            self.nudge_clips(Ticks(step.0 * direction));
        } else {
            let step = if by_bar { bar } else { self.piano_roll.grid };
            self.nudge_selection(Ticks(step.0 * direction));
        }
    }

    /// Run a batch, and select what it produced. Nothing happens if the batch is empty:
    /// an edit that changes nothing must not land in the undo history.
    pub(crate) fn exec_clip_batch(
        &mut self,
        label: &str,
        commands: Vec<Command>,
        select: Vec<ClipRef>,
    ) {
        if commands.is_empty() {
            return;
        }
        if self.exec(Command::Batch {
            label_key: label.to_string(),
            commands,
        }) && !select.is_empty()
        {
            self.set_clip_selection(select);
        }
        self.validate_selection();
    }

    /// Run `f` with the project and a source of fresh ids.
    ///
    /// A clip operation has to read the project *while* it allocates ids for the clips and
    /// notes it is about to build, and `CommandBus::alloc_id` borrows the bus. So the ids
    /// are taken from the project's counter directly and the bus is told afterwards
    /// ([`CommandBus::reserve_ids`]).
    pub(crate) fn with_ids<R>(&mut self, f: impl FnOnce(&Project, clips::Ids) -> R) -> R {
        let mut next = self.bus.project().next_id;
        let out = {
            let project = self.bus.project();
            let mut alloc = || {
                let id = next;
                next += 1;
                id
            };
            f(project, &mut alloc)
        };
        self.bus.reserve_ids(next);
        out
    }

    /// Split every selected clip the playhead runs through. Clips it misses are left
    /// alone rather than refused: cutting a whole track at a bar line is one gesture.
    pub fn split_clips_at_playhead(&mut self) {
        let at = self.engine.position();
        let sel = self.clip_selection();
        let (commands, select) = self.with_ids(|project, ids| {
            let mut commands = Vec::new();
            let mut select = Vec::new();
            for (track, id) in &sel {
                let Some(clip) = clips::clip_of(project, (*track, *id)) else {
                    continue;
                };
                select.push((*track, *id));
                let Some(cmds) = clips::split(*track, clip, at, ids) else {
                    continue;
                };
                for c in &cmds {
                    if let Command::InsertClip { clip, .. } = c {
                        select.push((*track, clip.id));
                    }
                }
                commands.extend(cmds);
            }
            (commands, select)
        });
        self.exec_clip_batch("cmd.split_clip", commands, select);
    }

    /// Glue the selected clips together, one track at a time. Clips on different tracks
    /// are separate pieces of music and are glued separately, not merged into one.
    pub fn glue_clips(&mut self) {
        let sel = self.clip_selection();
        let mut tracks: Vec<TrackId> = sel.iter().map(|r| r.0).collect();
        tracks.sort_by_key(|t| t.0);
        tracks.dedup();
        let (commands, select) = self.with_ids(|project, ids| {
            let mut commands = Vec::new();
            let mut select = Vec::new();
            for track in tracks {
                let Some(t) = project.track(track) else {
                    continue;
                };
                let on_track: Vec<&Clip> = t
                    .clips
                    .iter()
                    .filter(|c| sel.contains(&(track, c.id)))
                    .collect();
                let keep = on_track
                    .iter()
                    .min_by_key(|c| (c.start, c.id.0))
                    .map(|c| c.id);
                if let Some(cmds) = clips::glue(track, &on_track, ids) {
                    if let Some(k) = keep {
                        select.push((track, k));
                    }
                    commands.extend(cmds);
                }
            }
            (commands, select)
        });
        self.exec_clip_batch("cmd.glue_clips", commands, select);
    }

    pub fn duplicate_clips(&mut self) {
        let sel = self.clip_selection();
        let link = self.settings.link_copies;
        let (commands, made) =
            self.with_ids(|project, ids| clips::duplicate(project, &sel, link, ids));
        self.exec_clip_batch("cmd.duplicate_clips", commands, made);
    }

    pub fn delete_clip_selection(&mut self) {
        let sel = self.clip_selection();
        let commands: Vec<Command> = sel
            .iter()
            .filter(|r| clips::clip_of(self.bus.project(), **r).is_some())
            .map(|r| Command::RemoveClip {
                track: r.0,
                clip: r.1,
            })
            .collect();
        if commands.is_empty() {
            return;
        }
        self.selected_clips.clear();
        self.exec(Command::Batch {
            label_key: "cmd.remove_clip".into(),
            commands,
        });
        self.validate_selection();
    }

    /// Mute or unmute the selection. A mixed selection is muted, not toggled one by one:
    /// one press should leave the selection in a state you can see, not in two.
    pub fn toggle_clip_mute(&mut self) {
        let sel = self.clip_selection();
        let all_muted = !sel.is_empty()
            && sel
                .iter()
                .all(|r| clips::clip_of(self.bus.project(), *r).is_some_and(|c| c.muted));
        let commands = clips::set_mute(self.bus.project(), &sel, !all_muted);
        self.exec_clip_batch("cmd.set_clip_mute", commands, Vec::new());
    }

    pub fn toggle_clip_loop(&mut self) {
        let sel = self.clip_selection();
        let all_looping = !sel.is_empty()
            && sel.iter().all(|r| {
                clips::clip_of(self.bus.project(), *r).is_some_and(|c| c.content_length.is_some())
            });
        let commands = clips::set_loop(self.bus.project(), &sel, !all_looping);
        self.exec_clip_batch("cmd.set_clip_loop", commands, Vec::new());
    }

    /// Step the selection through the track palette, and off the end back to the track's
    /// own colour — so one key both colours a clip and clears it, and there is no state
    /// you can reach and not leave.
    pub fn cycle_clip_color(&mut self) {
        let sel = self.clip_selection();
        let current = sel
            .first()
            .and_then(|r| clips::clip_of(self.bus.project(), *r))
            .and_then(|c| c.color);
        let next = match current {
            None => Some(Color::PALETTE[0]),
            Some(c) => Color::PALETTE
                .iter()
                .position(|p| *p == c)
                .and_then(|i| Color::PALETTE.get(i + 1).copied()),
        };
        self.set_clip_color(next);
    }

    /// Start renaming the first selected clip. The arrangement draws the editor over the
    /// clip itself; this only says which one and with what text.
    pub fn begin_rename_clip(&mut self) {
        if let Some(r) = self.clip_selection().first().copied() {
            if let Some(clip) = clips::clip_of(self.bus.project(), r) {
                let name = clip.name.clone();
                self.arrangement.begin_rename_clip(r, name);
                self.focus_arrangement();
            }
        }
    }

    /// Commit an in-place rename. An empty name means the edit was abandoned (Escape) or
    /// emptied, and a clip with no name at all is worse than the one it had.
    pub fn rename_clip(&mut self, clip: ClipRef, name: String) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let unchanged = clips::clip_of(self.bus.project(), clip).is_some_and(|c| c.name == name);
        if unchanged {
            return;
        }
        self.exec(Command::SetClipName {
            track: clip.0,
            clip: clip.1,
            name,
        });
    }

    pub fn set_clip_color(&mut self, color: Option<Color>) {
        let sel = self.clip_selection();
        let commands = clips::set_color(self.bus.project(), &sel, color);
        self.exec_clip_batch("cmd.set_clip_color", commands, Vec::new());
    }

    /// Move the selected clips along the timeline by `delta`, snapped by the caller.
    pub fn nudge_clips(&mut self, delta: Ticks) {
        let sel = self.clip_selection();
        let (commands, made) = self.with_ids(|project, ids| {
            clips::transpose_selection(project, &sel, delta, 0, false, false, ids)
        });
        self.exec_clip_batch("cmd.nudge", commands, made);
    }

    /// Apply a finished drag from the arrangement. One batch, one undo step, whatever the
    /// pointer did on the way there.
    pub fn commit_clip_drag(&mut self, anchor: ClipRef, result: ClipDragResult) {
        // Alt-drag of an audio clip's right edge stretches it (round 29, ADR 0061).
        if self.warp_stretch_drag(anchor, result) {
            return;
        }
        let sel = self.clip_selection();
        let link = self.settings.link_copies;
        let (commands, made) = self.with_ids(|project, ids| {
            let Some(clip) = clips::clip_of(project, anchor) else {
                return (Vec::new(), Vec::new());
            };
            // A reversed audio clip maps its window backwards (ADR 0025).
            if let Some((edit, delta)) = crate::audio_clips::reversed_drag(clip, result) {
                return (
                    crate::audio_clips::reversed_edit(anchor.0, clip, edit, delta),
                    Vec::new(),
                );
            }
            // A comp window's edge at a seam moves the seam: both sides (ADR 0058).
            let seam = match result {
                ClipDragResult::TrimStart { delta } => Some((true, delta)),
                ClipDragResult::TrimEnd { delta } => Some((false, delta)),
                _ => None,
            }
            .and_then(|(start, d)| crate::comping::seam_trim(project, anchor.0, clip, start, d));
            if let Some(cmds) = seam.filter(|c| !c.is_empty()) {
                return (cmds, Vec::new());
            }
            match result {
                ClipDragResult::Move { delta, rows, copy } => {
                    clips::transpose_selection(project, &sel, delta, rows, copy, link, ids)
                }
                ClipDragResult::TrimStart { delta } => (
                    clips::set_window(anchor.0, clip, clip.start + delta, clip.length - delta),
                    Vec::new(),
                ),
                ClipDragResult::TrimEnd { delta } => (
                    clips::set_window(anchor.0, clip, clip.start, clip.length + delta),
                    Vec::new(),
                ),
                // Dragging the content to the right moves the music later, which means
                // the window starts *earlier* in the content.
                ClipDragResult::Slip { delta } => (clips::slip(anchor.0, clip, -delta), Vec::new()),
            }
        });
        let label = match result {
            ClipDragResult::Move { copy: true, .. } => "cmd.duplicate_clips",
            ClipDragResult::Move { .. } => "cmd.move_clip",
            ClipDragResult::Slip { .. } => "cmd.set_clip_offset",
            _ => "cmd.set_clip_bounds",
        };
        self.exec_clip_batch(label, commands, made);
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

    /// Move the selection up or down the editor's vertical axis.
    ///
    /// `semitones` is what it says on a keyboard. On a **kit** it is rows: ±1 is the next
    /// pad and ±12 — the octave keys — is four pads, which is a row of a 4×4 pad grid.
    /// Neither "a semitone" nor "an octave" names anything on a drum kit, and a key that
    /// silently did nothing would be worse than one that moves by the unit the layout
    /// actually has.
    pub fn transpose_selection(&mut self, semitones: i32) {
        let Some((t, c, sel)) = self.selected_note_values() else {
            return;
        };
        let axis = self.note_axis();
        if axis.pads() {
            let rows = if semitones.abs() >= 12 {
                semitones.signum() * 4
            } else {
                semitones
            };
            // Refuse the whole move rather than piling the selection up against an end
            // stop: a transpose that squashed three pads onto one would not be undoable
            // by transposing back.
            if sel
                .iter()
                .any(|n| axis.moved(n.pitch, rows) == n.pitch && rows != 0)
            {
                return;
            }
            let notes: Vec<_> = sel
                .into_iter()
                .map(|n| cantus_core::Note {
                    pitch: axis.moved(n.pitch, rows),
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
            return;
        }
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
            if clip.content_end() < end + offset {
                commands.insert(
                    0,
                    Command::SetClipBounds {
                        track: t,
                        clip: c,
                        start: clip.start,
                        length: end + offset - clip.offset,
                    },
                );
            }
        }
        // ...and the copies of a linked clip that showed what it showed (ADR 0037).
        let follow = cantus_core::links::follow_window(self.project(), (t, c), &commands);
        commands.extend(follow);
        if self.exec(Command::Batch {
            label_key: "cmd.duplicate".into(),
            commands,
        }) {
            self.selected_notes = ids;
        }
    }

    // ---- appearance -----------------------------------------------------------------

    /// Show the appearance file in the OS file manager so the user can edit it.
    pub fn reveal_theme_file(&mut self) {
        let path = self.appearance_path.clone();
        if !path.exists() {
            self.store_appearance();
        }
        match reveal(&path) {
            Ok(_) => self.set_status(tf("status.theme_file", &[("path", &path.display())])),
            Err(e) => self.set_error(e.to_string()),
        }
    }

    pub fn reveal_keymap_file(&mut self) {
        let path = self.keymap_path.clone();
        match reveal(&path) {
            Ok(_) => self.set_status(tf("status.theme_file", &[("path", &path.display())])),
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// The appearance file changed on disk, or the system went dark or light: show it.
    fn poll_appearance(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.appearance_watch.poll() {
            match result {
                Ok(a) => {
                    self.appearance = a;
                    self.apply_appearance();
                }
                Err(e) => self.set_error(tf("status.theme_error", &[("error", &e)])),
            }
        }
        if self.appearance.mode == crate::kit::appearance::Mode::System
            && views::appearance_file::system_dark(ctx) != self.appearance_page.system_dark
        {
            self.apply_appearance();
        }
    }

    // ---- effects --------------------------------------------------------------------

    /// Add an insert to the end of a strip's chain.
    pub fn add_effect(&mut self, strip: StripId, kind: EffectKind) {
        let slot = self.bus.new_effect(kind);
        let index = self
            .project()
            .effects_of(strip)
            .map(|e| e.len())
            .unwrap_or(0);
        self.exec(Command::InsertEffect {
            strip,
            index,
            slot: Box::new(slot),
        });
    }

    pub fn remove_effect(&mut self, strip: StripId, effect: EffectId) {
        self.exec(Command::RemoveEffect { strip, effect });
    }

    // ---- recording ------------------------------------------------------------------
    // Arming, inputs, the record options and a pass from start to end are in
    // `record_session` (ADR 0025).

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

    pub(crate) fn apply_key_event(&mut self, ev: KeyEvent) {
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
        self.replace_project(welcome_project());
    }

    /// Put `project` in the place of the open one, as a new, unsaved session: what *New*
    /// and the demo picker do.
    pub fn replace_project(&mut self, project: Project) {
        // A save in flight writes the project being replaced: it lands first.
        self.wait_for_save();
        // A pass in progress belongs to the project it was recorded into.
        if self.is_recording() {
            self.stop_recording();
        }
        self.engine.stop();
        self.bus.reset(project);
        self.epoch += 1;
        self.path = None;
        self.sync_audio_base();
        self.selected_track = None;
        self.selected_clip = None;
        self.selected_notes.clear();
        self.select_first();
        self.seek(Ticks::ZERO);
    }

    /// Give the track a different instrument. A kit arrives with its pad names in the
    /// interface language, because "Closed hat" on a Spanish screen is a bug however good
    /// the hat sounds.
    pub fn set_instrument(&mut self, track: TrackId, spec: InstrumentSpec) {
        let spec = match spec {
            InstrumentSpec::Pulsus(mut kit) => {
                for pad in &mut kit.pads {
                    pad.name = localized_pad_name(pad);
                }
                InstrumentSpec::Pulsus(kit)
            }
            other => other,
        };
        self.exec(Command::SetInstrument {
            track,
            spec: Box::new(spec),
        });
        self.rack.selected_pad = 0;
    }

    /// Add a pad of the given voice to the track's kit, on the next free pitch above its
    /// highest — so a new pad never shadows one already there — and select it.
    pub fn add_pad(&mut self, track: TrackId, voice: cantus_core::PadVoice) {
        use cantus_core::{DrumPad, PadVoice};
        let Some(kit) = self
            .project()
            .track(track)
            .and_then(|t| t.instrument.pulsus())
        else {
            return;
        };
        let pitch = kit
            .pads
            .iter()
            .map(|p| p.pitch)
            .max()
            .map(|m| m.saturating_add(1))
            .unwrap_or(36)
            .min(127);
        if kit.index_of_pitch(pitch).is_some() {
            return; // Every pitch above the top is taken: the kit is as big as it can be.
        }
        let mut pad = match voice {
            PadVoice::Kick => DrumPad::kick(),
            PadVoice::Snare => DrumPad::snare(),
            PadVoice::HiHat => DrumPad::closed_hat(),
            PadVoice::Clap => DrumPad::clap(),
            PadVoice::Tom => DrumPad::tom(pitch, "Tom", 0.0),
            PadVoice::Sample => {
                let mut p = DrumPad::snare();
                p.voice = PadVoice::Sample;
                p.decay = cantus_core::PadField::Decay.info(PadVoice::Sample).default;
                p
            }
        };
        pad.pitch = pitch;
        pad.choke = 0;
        pad.name = if voice == PadVoice::Sample {
            t("pad.voice.sample")
        } else {
            localized_pad_name(&pad)
        };
        pad.id = kit.next_id();
        let index = kit.pads.partition_point(|p| p.pitch < pitch);
        if self.exec(Command::InsertPad {
            track,
            pad: Box::new(pad),
        }) {
            self.rack.selected_pad = index;
        }
    }

    pub fn remove_pad(&mut self, track: TrackId, pad: usize) {
        let Some(id) = self
            .project()
            .track(track)
            .and_then(|t| t.instrument.pulsus())
            .and_then(|k| k.pad(pad))
            .map(|p| p.id)
        else {
            return;
        };
        if self.exec(Command::RemovePad { track, pad: id }) {
            self.rack.selected_pad = pad.saturating_sub(1);
        }
    }

    /// Choose a WAV for a pad. The dialog is the only part that cannot be tested headless;
    /// [`set_pad_sample`](Self::set_pad_sample) is the part that can.
    pub fn load_pad_sample(&mut self, track: TrackId, pad: usize) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV", &["wav", "WAV"])
            .pick_file()
        {
            self.set_pad_sample(track, pad, Some(path.to_string_lossy().to_string()));
        }
    }

    /// Point a pad at a file and make it a sample pad, in one undo step. A pad with a file
    /// but a synthesized voice would be a pad that looks loaded and sounds like a drum
    /// machine, which is a support question waiting to be asked.
    pub fn set_pad_sample(&mut self, track: TrackId, pad: usize, path: Option<String>) {
        if let Some(p) = path.as_deref() {
            self.engine.samples.retry(std::path::Path::new(p));
        }
        let Some((id, is_sample)) = self
            .project()
            .track(track)
            .and_then(|t| t.instrument.pulsus())
            .and_then(|k| k.pad(pad))
            .map(|p| (p.id, p.voice == cantus_core::PadVoice::Sample))
        else {
            return;
        };
        let mut commands = vec![Command::SetPadSample {
            track,
            pad: id,
            path,
        }];
        if !is_sample {
            commands.push(Command::SetInstrumentParam {
                track,
                param: cantus_core::InstrumentParam::Pulsus {
                    pad: id,
                    field: cantus_core::PadField::Voice,
                },
                value: cantus_core::PadVoice::Sample.index() as f32,
            });
        }
        self.exec(Command::Batch {
            label_key: "cmd.set_pad_sample".into(),
            commands,
        });
    }

    pub fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Cantus project", &[cantus_project::FILE_EXTENSION])
            .pick_file()
        {
            self.open(&path);
        }
    }

    /// Open the project at `path`. A file that cannot be opened is refused with what is
    /// wrong with it in words, and the open project stays as it was (round 22).
    pub fn open(&mut self, path: &Path) {
        // A save in flight lands before anything is read (it may be this very file).
        self.wait_for_save();
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
                    self.set_error(crate::recovery_text::open_failed(path, &e));
                    return;
                }
            },
            Err(e) => {
                self.set_error(crate::recovery_text::open_failed(path, &e));
                return;
            }
        };
        // A pass in progress belongs to the project it was recorded into.
        if self.is_recording() {
            self.stop_recording();
        }
        self.engine.stop();
        self.bus.reset(loaded.project);
        self.epoch += 1;
        self.path = Some(path.to_path_buf());
        self.sync_audio_base();
        self.selected_track = None;
        self.selected_clip = None;
        self.selected_notes.clear();
        self.select_first();
        self.seek(Ticks::ZERO);
        if loaded.links_repaired > 0 {
            self.set_error(crate::links::repaired_text(loaded.links_repaired));
        } else if loaded.warps_repaired > 0 {
            self.set_error(crate::warp::repaired_text(loaded.warps_repaired));
        } else if let Some(from) = loaded.migrated_from {
            self.set_status(tf("status.migrated", &[("from", &from)]));
        } else if !self.status.error {
            self.set_status(tf("status.opened", &[("path", &path.display())]));
        }
    }

    /// *File ▸ Save* (Ctrl+S): written on a thread (`save_job`); the window goes on.
    pub fn save(&mut self) {
        // A save in flight already knows where it goes: an untitled project is not asked
        // for a name again (`save_job`).
        match self.save_target() {
            Some(p) => {
                self.start_save(&p);
            }
            None => self.save_as(),
        }
    }

    /// *File ▸ Save As…*: the file dialog, then a save on a thread (`save_job`).
    pub fn save_as(&mut self) {
        let name = format!("{}.{}", self.project().name, cantus_project::FILE_EXTENSION);
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Cantus project", &[cantus_project::FILE_EXTENSION])
            .set_file_name(name)
            .save_file()
        {
            self.start_save(&path);
        }
    }

    /// Save to `path` and wait until it has landed (or failed): what scripts, the soak and
    /// tests use. The interface uses [`save`](Self::save) and
    /// [`start_save`](Self::start_save), which do not wait.
    pub fn save_to_path(&mut self, path: &Path) {
        self.start_save(path);
        self.wait_for_save();
    }

    /// The autosave interval: the settings', or the override tests and the soak use.
    pub fn autosave_interval(&self) -> Duration {
        self.autosave_every
            .unwrap_or_else(|| self.settings.autosave_interval())
    }

    /// File ▸ Export… (Ctrl+E): the export dialog (ADR 0033).
    pub fn export_dialog(&mut self) {
        // Already open: the choices being made stay.
        if !self.export.busy() && self.export.dialog.is_none() {
            views::export::open(self, None);
        }
    }

    /// Export the song to `path` on a background thread, in the format its extension
    /// names (WAV, FLAC, MP3, Ogg) with the remembered choices; poll with
    /// [`CantusApp::poll_export`] (called every frame) or [`CantusApp::export_in_progress`].
    pub fn export_to(&mut self, path: PathBuf) {
        if self.export.busy() {
            return;
        }
        let mut d = self.export.dialog.clone().unwrap_or_else(|| {
            let mut d = self.settings.export.clone();
            d.tags.title = self.project().name.clone();
            d
        });
        if let Some(f) = views::export::format_of(&path) {
            d.format = f;
        }
        if d.scope == cantus_export::Scope::Stems {
            d.scope = cantus_export::Scope::Song;
        }
        views::export::start_with(self, &path, Some(d));
    }

    pub fn export_in_progress(&self) -> bool {
        self.export.busy()
    }

    pub fn poll_export(&mut self) {
        views::export::poll(self);
    }

    // ---- per-frame ------------------------------------------------------------------

    /// Relative audio paths resolve against the project's folder (ADR 0025 §2): when it
    /// changes (open, save, save as, new, a demo) the engine's cache is told and the audio
    /// is attached again with the next snapshot.
    pub(crate) fn sync_audio_base(&mut self) {
        let base = self.audio_base().map(Path::to_path_buf);
        if self.engine.samples.project_dir() != base.as_deref() {
            self.engine.samples.set_project_dir(base);
            self.synced_revision = u64::MAX;
        }
    }

    /// Send the engine a fresh snapshot on the next `sync_engine` even if the project did
    /// not change (what it is compiled from did: a count-in's click).
    pub(crate) fn force_resync(&mut self) {
        self.synced_revision = u64::MAX;
    }

    /// Send the project to the engine again at the next sync (a sample file came back).
    pub(crate) fn resync_engine(&mut self) {
        self.synced_revision = u64::MAX;
    }

    pub fn sync_engine(&mut self) {
        self.sync_audio_base();
        // A snapshot held back for its zones' starts: sent once they are read (third
        // review #6) — never a wait on this thread.
        self.engine.flush_pending();
        // A source the loader thread finished: attach it (ADR 0025 §9).
        if self.engine.samples.poll_loaded() {
            self.synced_revision = u64::MAX;
        }
        self.load_notes();
        // Notes pitched too high to stream played their start only: said once a session
        // (third review #9); the status bar's chip keeps the count.
        if self.engine.stats().stream_cut > 0 && !self.inst.sampler.cut_told {
            self.inst.sampler.cut_told = true;
            self.set_status(t("smp.cut_notes"));
        }
        // An effect the last snapshot holds that the engine has no unit for: a remove and
        // its undo both went out before the engine handed the removed unit back, so the
        // undo's snapshot found it missing (round 22, soak). Send again: the unit goes too.
        if self
            .sent_effects
            .iter()
            .any(|id| !self.engine.knows_effect(*id))
        {
            self.synced_revision = u64::MAX;
        }
        let rev = self.bus.revision();
        if rev != self.synced_revision {
            self.synced_revision = rev;
            let mut snapshot = if self.rec.count_in_click {
                // A count-in with the metronome off: the click sounds for the count-in
                // only (`ClickMode::CountInOnly`), and the project is not touched.
                let mut p = self.bus.project().clone();
                p.metronome = true;
                p.recording.click = cantus_core::ClickMode::CountInOnly;
                self.engine.compile(&p, rev)
            } else if let Some(p) = views::transport::pattern::project_to_play(self) {
                // Pattern mode (ADR 0052): the current pattern alone, looped.
                self.engine.compile(&p, rev)
            } else {
                self.engine.compile(self.bus.project(), rev)
            };
            // A parameter a recording pass has moved follows its controller (ADR 0062).
            self.latch_recorded(&mut snapshot);
            self.sent_effects.clear();
            self.sent_effects
                .extend(snapshot.effect_slots().map(|s| s.id));
            // Hosted plug-ins' units reach the engine before the snapshot that holds them.
            let notices = self.plugins.sync(self.bus.project(), &mut self.engine);
            self.host_notices(notices);
            self.engine.set_snapshot(snapshot);
        }
    }

    /// What sample loads had to say (ADR 0050, review 8): once per kind and batch, on the
    /// status line.
    fn load_notes(&mut self) {
        use cantus_engine::inst::stream::LoadNote;
        let notes = self.engine.samples.take_notes();
        if notes.is_empty() {
            return;
        }
        let unwritable: Vec<&String> = notes
            .iter()
            .filter_map(|n| match n {
                LoadNote::CacheUnwritable(e) => Some(e),
                _ => None,
            })
            .collect();
        let changed = notes
            .iter()
            .filter(|n| matches!(n, LoadNote::Changed(_)))
            .count();
        let damaged = notes
            .iter()
            .filter(|n| matches!(n, LoadNote::Damaged(_)))
            .count();
        let rechecked = notes
            .iter()
            .filter(|n| matches!(n, LoadNote::ChangedSinceCached(_)))
            .count();
        // Second review #5, #15: a cache file that could not be read is said once; a sample
        // that cannot be read at all, by name.
        if let Some((file, error)) = notes.iter().find_map(|n| match n {
            LoadNote::Unreadable(p, e) => Some((p, e)),
            _ => None,
        }) {
            self.set_error(tf(
                "smp.sample_unreadable",
                &[("file", &file.display()), ("error", error)],
            ));
        } else if let Some(e) = unwritable.first() {
            self.set_error(crate::i18n::tf_count(
                "smp.cache_unwritable",
                unwritable.len(),
                &[("error", e)],
            ));
        } else if changed > 0 {
            self.set_status(crate::i18n::tf_count("smp.sample_changed", changed, &[]));
        } else if damaged > 0 {
            self.set_status(crate::i18n::tf_count("smp.cache_damaged", damaged, &[]));
        } else if rechecked > 0 {
            self.set_status(crate::i18n::tf_count(
                "smp.sample_rechecked",
                rechecked,
                &[],
            ));
        }
    }

    /// What the plug-in host has to say, on the status line.
    fn host_notices(&mut self, notices: Vec<cantus_host::HostNotice>) {
        use cantus_host::HostNotice;
        for n in notices {
            match n {
                HostNotice::LoadFailed { name, error, .. } => self.set_error(tf(
                    "host.load_failed",
                    &[("name", &name), ("error", &error)],
                )),
                HostNotice::RestartFailed { error, .. } => {
                    self.set_error(tf("host.restart_failed", &[("error", &error)]))
                }
                HostNotice::ScanDone(r) => {
                    // Plug-ins that were missing may be there now.
                    self.synced_revision = u64::MAX;
                    self.set_status(tf(
                        "host.scan_done",
                        &[
                            ("found", &r.plugins),
                            ("scanned", &r.scanned),
                            ("blocked", &r.blocked),
                        ],
                    ));
                }
                HostNotice::WindowClosed(_) | HostNotice::InstrumentWindowClosed(_) => {}
                // A hosted instrument that is not here (ADR 0028): the track keeps its
                // data and plays silence, and the status line says so.
                HostNotice::InstrumentLoadFailed { name, error, .. } => self.set_error(tf(
                    "hosted.missing_status",
                    &[("name", &name), ("why", &error)],
                )),
                HostNotice::InstrumentRestartFailed { error, .. } => {
                    self.set_error(tf("host.restart_failed", &[("error", &error)]))
                }
                // A plug-in that stopped working, and isolation without its helper (round
                // 29, ADR 0063): the crash card and the status line.
                HostNotice::PluginCrashed { .. }
                | HostNotice::InstrumentCrashed { .. }
                | HostNotice::IsolationUnavailable => views::plugin_notice::on_notice(self, n),
            }
        }
    }

    /// Turn this frame's key presses into actions, then run them.
    ///
    /// Nothing here knows which key does what — that is `keymap.toml` (ADR 0010). The one
    /// piece of policy left is the typing piano: while it is on it owns the bare letter
    /// keys, so those bindings are suppressed and the tools stay reachable on the numbers.
    pub fn global_shortcuts(&mut self, ctx: &egui::Context) {
        // The device picker reads every key while it is open (type to search).
        // A kit text field holds the keyboard — or just let it go with this very key
        // (the Esc or Enter that closed it): its keys are its own (round 26).
        if ctx.egui_wants_keyboard_input()
            || crate::kit::field::active(ctx)
            || self.rebinding.is_some()
            || self.picker.is_some()
        {
            return;
        }
        // The export dialog is modal: its keys are its own (Esc, Enter), and nothing
        // reaches the project behind it (round 23).
        if self.export.dialog.is_some() {
            return;
        }
        // A focused fader moves with the arrow keys. This dispatch runs before any panel
        // is laid out, so without the claim the same press would nudge the note selection
        // as well (`crate::widgets`).
        let arrows_taken = crate::widgets::arrow_keys_claimed(ctx);
        self.dispatch_keys(ctx, arrows_taken, &|_, _| false);
    }

    /// One pass's keys, whoever holds the focus: the keymap's actions (but those `taken`
    /// says a focused view keeps for itself), the bare M / S rule, the paste that rides
    /// with Ctrl+V, and the typing piano — so a view that holds egui's focus (the Routing
    /// matrix) and forwards the rest here neither silences the piano nor leaves a note
    /// hanging. Runs once a pass, whichever caller comes first.
    pub(crate) fn dispatch_keys(
        &mut self,
        ctx: &egui::Context,
        arrows_taken: bool,
        taken: &dyn Fn(&crate::keymap::Keymap, Action) -> bool,
    ) {
        let pass = ctx.cumulative_pass_nr();
        if self.keys_pass == Some(pass) {
            return;
        }
        self.keys_pass = Some(pass);
        self.mixer.keys_on_pass(pass);
        // Ctrl+V's text comes with its event; a menu's paste waits for it (ADR 0036).
        self.take_paste_event(ctx);
        for action in self.keymap.pressed(ctx, self.keyboard_piano, arrows_taken) {
            if taken(&self.keymap, action) {
                continue;
            }
            // A bare M or S mutes or solos the selected strip only while the mixer is what
            // is being edited; elsewhere the letters stay out of the way.
            let strip_key = matches!(action, Action::ToggleStripMute | Action::ToggleStripSolo);
            if strip_key && self.focus != EditFocus::Mixer && self.bare_binding_pressed(ctx, action)
            {
                continue;
            }
            self.run_action(action);
        }
        self.settle_paste(ctx);
        if self.keyboard_piano {
            for ev in self.keyboard.poll(ctx) {
                self.apply_key_event(ev);
            }
        }
    }

    /// Whether `action` fired this frame through a binding with no modifier.
    fn bare_binding_pressed(&self, ctx: &egui::Context, action: Action) -> bool {
        ctx.input(|i| {
            self.keymap
                .bindings_for(action)
                .iter()
                .any(|b| b.is_bare() && i.key_pressed(b.key))
        })
    }

    /// Do one thing. Every path into this — a shortcut, a menu item, a future script or
    /// MCP call — goes through the same match, so a binding can never do something a menu
    /// item cannot.
    pub fn run_action(&mut self, action: Action) {
        use Action::*;
        // Round 28 (ADR 0054): the pitch editor's keys while it has the focus.
        if self.pitch_action(action) {
            return;
        }
        // A selected warp marker takes Del (round 29, ADR 0061).
        if self.warp_action(action) {
            return;
        }
        // Round 29 (ADR 0060): markers and the tempo track.
        if self.tempo_action(action) {
            return;
        }
        // Round 29 (ADR 0064): freezing, bouncing in place, low-latency monitoring.
        if views::freeze_view::action(self, action) {
            return;
        }
        // Round 29 (ADR 0062): MIDI learn, *Link to controller*, the mappings pane.
        if self.controllers_action(action) {
            return;
        }
        match action {
            // Handled by `controllers_action` above.
            MidiLearnMode | LinkLastTweaked | ViewControllers => {}
            // Handled by `tempo_action` above.
            AddMarker | PreviousMarker | NextMarker | LoopBetweenMarkers | SelectBetweenMarkers
            | ToggleTempoTrack => {}
            // Handled by `freeze_view::action` above.
            FreezeTrack | BounceInPlace | BounceSource | ToggleLowLatency => {}
            PlayPause => self.toggle_play(),
            Stop => self.stop(),
            Record => {
                // Pattern mode records nothing: the take belongs to the song (ADR 0052).
                if !views::transport::pattern::refuse_record(self) {
                    self.toggle_record();
                }
            }
            TapTempo => views::transport::tap(self),
            TogglePatternMode => views::transport::pattern::toggle(self),
            ArmTrack => self.arm_selected(),
            AddAudioTrack => self.add_audio_track(),
            ToggleCountIn => self.toggle_count_in(),
            TogglePunch => self.toggle_punch(),
            ShowTakes => self.toggle_selected_takes(),
            ToStart => self.seek(Ticks::ZERO),
            ToggleLoop => {
                let enabled = !self.project().loop_enabled;
                self.exec(Command::SetLoopEnabled { enabled });
            }
            ToggleMetronome => {
                let enabled = !self.project().metronome;
                self.exec(Command::SetMetronome { enabled });
            }
            Panic => self.engine.panic(),
            NewProject => self.request_new(),
            DemoSongs => views::demos::show(self),
            Open => self.request_open(),
            Save => self.save(),
            SaveAs => self.save_as(),
            ExportWav => self.export_dialog(),
            Undo => self.undo(),
            Redo => self.redo(),
            // Cut, copy, paste, duplicate, delete and the arrows all exist twice over:
            // once for notes and once for clips. Which one runs is decided by the half of
            // the window the user last edited in (`EditFocus`) — not by which is
            // non-empty, because both usually are, and not by a separate shortcut, because
            // then everyone would have to learn two. The clipboard goes by the focus alone:
            // a paste needs no selection, and a copy with none takes the whole clip or the
            // time range (ADR 0036, `crate::clipboard`).
            Cut => self.clipboard_cut(),
            Copy => self.clipboard_copy(),
            Paste => self.clipboard_paste(),
            Duplicate => {
                if self.editing_clips() {
                    self.duplicate_clips();
                } else {
                    self.duplicate_selection();
                }
            }
            SelectAll => {
                if self.editing_clips() {
                    self.select_all_clips();
                } else {
                    self.select_all_notes();
                }
            }
            Delete => {
                // A cable selected on the focused desk is what Delete means there — never
                // the notes still selected in the piano roll (round 25).
                let cable = (self.focus == EditFocus::Mixer)
                    .then(|| self.mixer.deletable_cable())
                    .flatten();
                if let Some(route) = cable {
                    self.remove_route(route);
                    self.mixer.clear_cable_selection();
                } else if self.editing_clips() {
                    self.delete_clip_selection();
                } else {
                    self.delete_selection();
                }
            }
            Quantize => self.quantize_selection(),
            ClearSelection => {
                // A drag on the desk (or a route armed by a click) and a selected cable
                // take Esc for themselves — it cancels the drag, lets the cable go — and
                // nothing else is cleared with it (round 25).
                if self.mixer.claims_escape() {
                    return;
                }
                // Esc closes the device window clicked last (round 20), or a floating one
                // under the pointer (FL); anywhere else it clears the selection.
                let closed = self.close_focused_device()
                    || self
                        .theme_ctx
                        .clone()
                        .is_some_and(|ctx| self.workspace.close_hovered_floating_device(&ctx));
                if closed {
                    // The window was the thing to clear.
                } else if self.appearance_page.picking {
                    self.toggle_pick();
                } else if self.focus == EditFocus::Arrangement && self.clip_browser.painting() {
                    // The arrangement's paint tool takes Esc before the selection does, when
                    // nothing above claimed it: it is put away (round 26, ADR 0037).
                    self.put_paint_away();
                } else if self.editing_clips() {
                    self.clear_clip_selection();
                    // Escape in the arrangement clears *both* selections. Two presses to
                    // clear what reads as one selected thing is the kind of detail that
                    // makes an editor feel unfinished.
                    self.clear_time_selection();
                } else {
                    self.selected_notes.clear();
                }
            }
            LoopToSelection => self.loop_to_selection(),
            ZoomToSelection => self.zoom_to_selection(),
            SelectPrevTrack => self.step_track(-1),
            SelectNextTrack => self.step_track(1),
            SplitClip => self.split_clips_at_playhead(),
            GlueClips => self.glue_clips(),
            ToggleClipMute => self.toggle_clip_mute(),
            ToggleClipLoop => self.toggle_clip_loop(),
            ClipColorNext => self.cycle_clip_color(),
            RenameClip => self.begin_rename_clip(),
            MakeUnique => self.make_unique_clips(),
            LinkClips => self.link_clips_to(None),
            SelectLinked => self.select_linked(),
            LinkTo => self.open_link_picker(None),
            // In the mixer the arrows walk the strips (§10).
            NudgeLeft if self.focus == EditFocus::Mixer => self.step_strip(-1),
            NudgeRight if self.focus == EditFocus::Mixer => self.step_strip(1),
            NudgeLeft => self.nudge(-1, false),
            NudgeRight => self.nudge(1, false),
            NudgeLeftBar => self.nudge(-1, true),
            NudgeRightBar => self.nudge(1, true),
            TransposeUp => self.transpose_selection(1),
            TransposeDown => self.transpose_selection(-1),
            TransposeOctaveUp => self.transpose_selection(12),
            TransposeOctaveDown => self.transpose_selection(-12),
            AutomateLastTweaked => {
                self.automate_last_tweaked();
            }
            ToolDraw => self.piano_roll.tool = views::piano_roll::Tool::Draw,
            // B in the arrangement: its paint tool, painting the clip browser's chosen tile
            // (round 26, ADR 0037), as B is the paint tool in FL's playlist.
            ToolPaint if self.focus == EditFocus::Arrangement => self.toggle_arrangement_paint(),
            ToolPaint => self.piano_roll.tool = views::piano_roll::Tool::Paint,
            ToolSelect => self.piano_roll.tool = views::piano_roll::Tool::Select,
            ToolDelete => self.piano_roll.tool = views::piano_roll::Tool::Delete,
            ToolMute => self.piano_roll.tool = views::piano_roll::Tool::Mute,
            ToolSlice => self.piano_roll.tool = views::piano_roll::Tool::Slice,
            ToolPlayback => self.piano_roll.tool = views::piano_roll::Tool::Playback,
            ToggleTypingPiano => {
                self.keyboard_piano = !self.keyboard_piano;
                for ev in self.keyboard.release_all() {
                    self.apply_key_event(ev);
                }
            }
            // FL's window keys toggle their area (ADR 0024 §5): hide it when it shows,
            // bring it to the front when it is behind a tab, put it back when it is gone.
            ViewPianoRoll => self.toggle_pane(Pane::PianoRoll),
            ViewChannelRack => self.toggle_pane(Pane::ChannelRack),
            ViewMixer => self.toggle_pane(Pane::Mixer),
            ViewRouting => self.toggle_pane(Pane::Routing),
            ViewArrangement => self.toggle_pane(Pane::Arrangement),
            ToggleInspector => self.toggle_pane(Pane::Inspector),
            ToggleBrowser => self.toggle_pane(Pane::Browser),
            ToggleClips => self.toggle_pane(Pane::Clips),
            CloseFloating => {
                if let Some(ctx) = self.theme_ctx.clone() {
                    self.workspace.close_floating_devices(&ctx);
                }
            }
            ZoomIn | ZoomOut | ZoomReset => {
                if let Some(ctx) = self.theme_ctx.clone() {
                    match action {
                        ZoomIn => self.step_ui_scale(&ctx, true),
                        ZoomOut => self.step_ui_scale(&ctx, false),
                        _ => self.apply_ui_scale(&ctx, settings::DEFAULT_UI_SCALE),
                    }
                }
            }
            PickElement => self.toggle_pick(),
            ToggleInstrumentWindow => self.toggle_instrument_window(),
            ToggleDeviceKeyboard => {
                if let Some(ctx) = &self.theme_ctx {
                    views::workspace_device::request_keyboard_toggle(ctx);
                }
            }
            // The selection — or the group, when the selection is a whole group — one
            // place along its dock (round 25, the owner's answer 12).
            MoveStripLeft => {
                self.nudge_strips(-1);
            }
            MoveStripRight => {
                self.nudge_strips(1);
            }
            ToggleStripSolo => {
                if let Some(id) = self.selected_strip {
                    self.toggle_strip_solo(id);
                }
            }
            ToggleStripMute => {
                if let Some(id) = self.selected_strip {
                    self.toggle_strip_mute(id);
                }
            }
            ToggleMeterWave => {
                self.settings.mixer_meter_wave = !self.settings.mixer_meter_wave;
                self.store_settings();
            }
            // While the mixer is what is being edited: the selected strip's route menu,
            // opened by the desk where its ▼ chip is on its next frame.
            RouteMenu => {
                if self.focus == EditFocus::Mixer
                    && self.selected_strip.is_some_and(|s| !s.is_master())
                {
                    self.mixer.want_route_menu();
                }
            }
        }
    }

    /// The last audio device error, while it stands (the settings window shows it).
    pub fn audio_error(&self) -> Option<&str> {
        self.audio_error.as_deref()
    }

    /// The shortcuts editor: every action, what it is bound to, and a click to rebind.
    fn keymap_dialog(&mut self, ctx: &egui::Context) {
        use crate::keymap::{Category, Preset};

        // A rebind in progress swallows the next key press, so read it before the widgets
        // get a chance to act on it.
        if let Some(action) = self.rebinding {
            if let Some(binding) = captured_binding(ctx) {
                self.keymap.bind(action, binding);
                self.store_keymap();
                self.rebinding = None;
            } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.rebinding = None;
            }
        }

        let mut open = self.show_shortcuts;
        let mut open_file = false;
        let mut apply_preset: Option<Preset> = None;
        let mut rebind: Option<Action> = None;
        let mut unbind: Option<(Action, crate::keymap::Binding)> = None;
        let conflicts = self.keymap.conflicts();

        egui::Window::new(t("keymap.title"))
            .collapsible(false)
            .open(&mut open)
            .default_size([space::GRID * 150.0, space::GRID * 140.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t("keymap.preset"));
                    for preset in Preset::ALL {
                        let selected = self.keymap.preset == preset.id();
                        if ui
                            .selectable_label(selected, t(preset.label_key()))
                            .clicked()
                        {
                            apply_preset = Some(preset);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        open_file = ui.button(t("keymap.open_file")).clicked();
                    });
                });
                ui.label(
                    egui::RichText::new(t("keymap.approximate"))
                        .text_style(design::caption())
                        .color(theme::p().text_dim),
                );
                if !conflicts.is_empty() {
                    ui.label(
                        egui::RichText::new(tf("keymap.conflicts", &[("n", &conflicts.len())]))
                            .text_style(design::strong())
                            .color(theme::p().danger),
                    );
                }
                ui.separator();

                // The list scrolls; the mouse help below it does not, because it is the
                // part a newcomer needs and would never scroll far enough to find.
                egui::ScrollArea::vertical()
                    .max_height(space::GRID * 88.0)
                    .show(ui, |ui| {
                        for category in Category::ALL {
                            ui.add_space(space::S2);
                            ui.label(
                                egui::RichText::new(t(category.label_key()).to_uppercase())
                                    .text_style(design::caption())
                                    .color(theme::p().text_dim),
                            );
                            // A grid, not a row of fixed widths: it sizes the name column
                            // to the longest name in the category, so the shortcuts line
                            // up however long the labels are in the current language.
                            egui::Grid::new(category.label_key())
                                .num_columns(2)
                                .spacing([space::S3, space::S1])
                                .min_row_height(space::ROW)
                                .show(ui, |ui| {
                                    for action in
                                        Action::ALL.into_iter().filter(|a| a.category() == category)
                                    {
                                        ui.label(t(&action.label_key()));
                                        ui.horizontal(|ui| {
                                            let bindings =
                                                self.keymap.bindings_for(action).to_vec();
                                            for b in &bindings {
                                                let clashes =
                                                    conflicts.iter().any(|c| c.binding == *b);
                                                let mut text = egui::RichText::new(b.display())
                                                    .text_style(design::mono_small());
                                                if clashes {
                                                    text = text.color(theme::p().danger);
                                                }
                                                if ui
                                                    .button(text)
                                                    .on_hover_text(t("keymap.clear"))
                                                    .clicked()
                                                {
                                                    unbind = Some((action, *b));
                                                }
                                            }
                                            if bindings.is_empty() {
                                                ui.label(
                                                    egui::RichText::new(t("keymap.unbound"))
                                                        .color(theme::p().text_dim),
                                                );
                                            }
                                            let listening = self.rebinding == Some(action);
                                            let label = if listening {
                                                t("keymap.press")
                                            } else {
                                                t("keymap.add")
                                            };
                                            if ui.selectable_label(listening, label).clicked() {
                                                rebind =
                                                    if listening { None } else { Some(action) };
                                            }
                                        });
                                        ui.end_row();
                                    }
                                });
                        }
                    });
                // The things a keymap cannot express: what the mouse and wheel do.
                ui.separator();
                ui.label(
                    egui::RichText::new(t("keymap.mouse").to_uppercase())
                        .text_style(design::caption())
                        .color(theme::p().text_dim),
                );
                for line in t("shortcuts.text").split(" · ") {
                    ui.label(egui::RichText::new(line).text_style(design::caption()));
                }
            });

        self.show_shortcuts = open;
        if !open {
            self.rebinding = None;
        }
        if open_file {
            self.reveal_keymap_file();
        }
        if let Some(preset) = apply_preset {
            self.apply_keymap_preset(preset);
        }
        if let Some(action) = rebind {
            self.rebinding = Some(action);
        }
        if let Some((action, binding)) = unbind {
            self.keymap.unbind(action, binding);
            self.store_keymap();
        }
    }

    /// Every window that floats over the layout: About, the shortcuts editor, the layout
    /// naming prompt. Public so headless tests can render them.
    pub fn dialogs(&mut self, ctx: &egui::Context) {
        // About (round 22): the kit's window, with the notices, privacy and credits.
        views::about::frame(self, ctx);
        if let Some(name) = self.naming_layout.clone() {
            let (name, save, close) = layout_name_dialog(ctx, name);
            if save && !name.trim().is_empty() {
                self.save_layout_as(&name, ctx);
            }
            self.naming_layout = if save || close { None } else { Some(name) };
        }
        views::eq_editor::window(self, ctx);
        views::comp_editor::window(self, ctx);
        views::fx::window(self, ctx);
        views::inst::window(self, ctx);
        views::pulsus_editor::window(self, ctx);
        if self.show_settings {
            views::workspace_settings::window(self, ctx);
        }
        if self.show_theme_editor {
            views::appearance::window(self, ctx);
        }
        if self.show_shortcuts {
            self.keymap_dialog(ctx);
        }
    }
}

impl CantusApp {
    /// One frame of the whole window: the toolbar, the workspace, the status bar, the
    /// floating windows. `eframe::App::ui` calls it; so do the splash (`crate::boot`) once
    /// the application is up, and the whole-window screenshots.
    pub fn frame(&mut self, root: &mut egui::Ui) {
        crate::font_fallback::note_input(root.ctx());
        crate::font_fallback::poll(root.ctx());
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        if self.theme_ctx.is_none() {
            self.theme_ctx = Some(ctx.clone());
        }
        let frame_start = Instant::now();
        self.engine.poll();
        self.poll_samples(ctx);
        crate::audio_setup::tick(self);
        // Hosted plug-ins: their requests, restarts, and what their windows did.
        let notices = self.plugins.idle(&mut self.bus, &mut self.engine);
        // What their windows moved: last tweaked, and recorded automation (ADR 0057).
        self.host_tweaks();
        // MIDI controllers (round 29, ADR 0062): what the tap heard since the last frame.
        self.controller_input();
        self.host_notices(notices);
        self.pump_capture();
        self.pump_analyser();
        self.poll_export();
        self.poll_recording();
        self.poll_appearance(ctx);
        self.global_shortcuts(ctx);
        views::transport::title::sync(self, ctx);
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

        // The toolbar on top, the status bar below, the workspace between: every area of
        // the window a tab of the dock (ADR 0024 §5).
        let s = crate::kit::Style::current();
        let toolbar = (views::transport::HEIGHT * s.t.density).round();
        let status = (views::workspace_status::HEIGHT * s.t.density).round();
        egui::Panel::top(egui::Id::new("toolbar"))
            .frame(egui::Frame::NONE)
            .exact_size(toolbar)
            .resizable(false)
            .show(root, |ui| views::transport::show(self, ui));
        egui::Panel::bottom(egui::Id::new("status"))
            .frame(egui::Frame::NONE)
            .exact_size(status)
            .resizable(false)
            .show(root, |ui| views::workspace_status::show(self, ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(s.t.panel))
            .show(root, |ui| views::workspace::show(self, ui));
        self.dialogs(ctx);
        views::interop::frame(self, ctx);
        views::export_dialog::show(self, ctx);
        views::demos::frame(self, ctx);
        views::recovery::frame(self, ctx);
        views::unsaved::frame(self, ctx);
        views::verse::frame(self, ctx);
        views::picker::frame(self, ctx);
        views::link_picker::frame(self, ctx);
        views::warp_quantise::frame(self, ctx);
        views::modulator::popup::frame(self, ctx);
        views::samples_browser::frame(self, ctx);
        views::audio_setup::frame(self, ctx);
        views::update::frame(self, ctx);
        views::appearance::pick(self, ctx);
        views::hint::value_pill(ctx);
        design::paint_focus_ring(ctx, &theme::p());
        self.layouts.end_frame();

        self.sync_engine();
        self.poll_save(false);
        self.maybe_autosave();
        self.journal_tick();

        if self.request_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.store_layouts(ctx);
            views::update::on_quit(self);
            self.close_session();
        }

        let dt = frame_start.elapsed().as_secs_f32();
        if self.frame_times.len() >= 120 {
            self.frame_times.remove(0);
        }
        self.frame_times.push(dt);
        self.tick(ctx);
    }

    /// The repaint policy (ADR 0024 §13): say what moves, and let the ticker ask for one
    /// frame at ≤ 60 Hz — or none at all when nothing does.
    fn tick(&mut self, ctx: &egui::Context) {
        if self.engine.is_playing() {
            ticker::moving(ctx, Reason::Playback);
        }
        if self.is_recording() {
            ticker::moving(ctx, Reason::Recording);
        }
        if !self.auditions.is_empty() {
            ticker::moving(ctx, Reason::Audition);
        }
        if self.export_in_progress() || self.plugins.scanning() {
            ticker::moving(ctx, Reason::Background);
        }
        // A hosted plug-in expects its requests served soon (CLAP `request_callback`): the
        // frame loop is the main thread's pump, so it keeps turning while one is loaded.
        if self.plugins.loaded_count() > 0 {
            ticker::moving(ctx, Reason::Plugins);
        }
        let m = self.engine.meters().master;
        if m[0].max(m[1]) > 1e-5 {
            ticker::moving(ctx, Reason::Meters);
        }
        // Samples loading in the background (ADR 0050 §7): collect them and show how far.
        if self.engine.samples.loading() {
            ticker::wake_after(ctx, Duration::from_millis(50));
        }
        // A snapshot waiting for its zones' starts (at most 250 ms): look again soon.
        if self.engine.snapshot_pending() {
            ticker::wake_after(ctx, Duration::from_millis(5));
        }
        // A bake waiting for its trace (ADR 0057), a comp being merged (ADR 0058): carry
        // them on, and collect them when their worker is done, even with nothing else
        // moving.
        self.poll_merge();
        // Freezes and bounces rendering (round 29, ADR 0064).
        crate::freeze::tick(self, ctx);
        // Imports waiting for their analysis, a warped bounce being written (ADR 0061).
        if self.poll_warp() {
            ticker::wake_after(ctx, Duration::from_millis(50));
        }
        if self.modulators.pending_bake.is_some() || self.merge_job.is_some() {
            ticker::wake_after(ctx, Duration::from_millis(50));
        }
        // A save or an autosave being written: look again shortly to collect it.
        if self.autosave_job.is_some() || self.save_in_flight() {
            ticker::wake_after(ctx, Duration::from_millis(100));
        }
        // An autosave that falls due: one wake-up at that moment, not a steady tick.
        if self.bus.is_dirty() && self.bus.revision() != self.autosaved_revision {
            let left = self
                .autosave_interval()
                .saturating_sub(self.last_autosave.elapsed());
            ticker::wake_after(ctx, left.max(Duration::from_millis(250)));
        }
        ticker::finish(ctx);
    }

    /// The last two seconds' average frame time, in milliseconds (the stats readout).
    pub fn average_frame_ms(&self) -> f32 {
        if self.frame_times.is_empty() {
            0.0
        } else {
            self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32 * 1000.0
        }
    }
}

impl eframe::App for CantusApp {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.frame(root);
        // The taskbar going light or dark changes the automatic icon (read here, by the
        // real window only, at most every two seconds).
        if self.taskbar.poll() {
            self.apply_appearance();
        }
        self.window_icon.sync(root.ctx(), frame);
    }
}

/// The layout's name dialog (final review): the kit's dialog in the middle of the window —
/// its panel and title bar, the name's field (it opens with the name selected; Enter
/// saves, Esc leaves), the kit's buttons, Save lit. Returns the words, whether to save and
/// whether to close.
fn layout_name_dialog(ctx: &egui::Context, mut name: String) -> (String, bool, bool) {
    use crate::kit::text::{self as ktext, Elide, Policy, Role, Text};
    use egui::{pos2, vec2, Align2, Rect};
    let s = crate::kit::Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = vec2(360.0 * k, 150.0 * k).min(screen.size() - vec2(16.0, 16.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let (mut save, mut close) = (false, false);
    egui::Area::new(egui::Id::new("cantus-layout-name"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            views::audio_parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), 32.0 * k));
            let glyph = crate::kit::glyph::Glyph::Inspector;
            if views::workspace_settings::title(ui, head, &s, glyph, "layout.dialog_title") {
                close = true;
            }
            let x0 = rect.left() + 20.0 * k;
            let x1 = rect.right() - 20.0 * k;
            let label = Rect::from_min_max(
                pos2(x0, head.bottom() + 12.0 * k),
                pos2(x1, head.bottom() + 30.0 * k),
            );
            crate::kit::with_brush(ui.painter(), |b| {
                let _ = ktext::paint(
                    b,
                    label,
                    &Text::new(&t("layout.name"), Role::Body, s.t.text_dim)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            });
            let well = Rect::from_min_max(
                pos2(x0, label.bottom() + 4.0 * k),
                pos2(x1, label.bottom() + 4.0 * k + s.d(24.0)),
            );
            let hint = t("layout.name");
            let out = crate::kit::field::Field::new(egui::Id::new("layout-name"))
                .hint(&hint)
                .opens_when_shown()
                .show(ui, well, &mut name);
            match out.event {
                Some(crate::kit::field::Event::Commit) => save = out.enter,
                Some(crate::kit::field::Event::Cancel) => close = true,
                None => {}
            }
            let buttons = Rect::from_min_max(
                pos2(x0, rect.bottom() - 44.0 * k),
                pos2(x1, rect.bottom() - 14.0 * k),
            );
            let mut row = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(buttons)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            let ok = !name.trim().is_empty();
            if views::audio_parts::button(&mut row, &t("layout.save"), true, ok, 90.0).clicked() {
                save = true;
            }
            row.add_space(8.0 * k);
            if views::audio_parts::button(&mut row, &t("layout.cancel"), false, true, 90.0)
                .clicked()
            {
                close = true;
            }
        });
    // Esc with the field already let go leaves the dialog too.
    if !crate::kit::field::active(ctx) && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    (name, save, close)
}

/// What the application reads from disk before it can open — plain data, so the splash
/// reads it on a thread (ADR 0024 §12): the settings, the look, the layouts, the keymap.
pub struct Files {
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub appearance: crate::kit::appearance::Appearance,
    pub appearance_path: PathBuf,
    pub layouts: Layouts,
    pub layouts_path: PathBuf,
    pub keymap: Keymap,
    pub keymap_path: PathBuf,
    /// The global controller map (round 29, ADR 0062), beside the keymap.
    pub controllers: cantus_core::controller::ControllerMap,
    pub controllers_path: PathBuf,
}

impl Files {
    /// Read everything from `dir`, writing the files a fresh install should find there
    /// (the keymap, to be discovered and edited). Sets the interface language: an explicit
    /// `--locale` wins over the stored preference.
    pub fn read(dir: &Path, locale_from_flag: Option<i18n::Locale>) -> Files {
        let settings_path = settings::settings_path(dir);
        let mut settings = Settings::load(&settings_path);
        // The installer's privacy page: its update-check choice, applied once (round 26).
        settings::apply_installer_choices(dir, &mut settings);
        match locale_from_flag {
            Some(l) => i18n::set_locale(l),
            None => {
                if let Some(l) = settings.locale.as_deref().and_then(i18n::Locale::from_code) {
                    i18n::set_locale(l);
                }
            }
        }
        let (appearance, appearance_path) = views::appearance_file::load(dir);
        let layouts_path = layout::layouts_path(dir);
        let mut layouts = Layouts::load(&layouts_path);
        layouts.force();
        let keymap_path = crate::keymap::keymap_path(dir);
        let keymap = Keymap::load(&keymap_path);
        if !keymap_path.exists() {
            if let Err(e) = keymap.save(&keymap_path) {
                log::warn!("cannot write keymap file: {e}");
            }
        }
        let controllers_path = crate::controllers::global_path(dir);
        let controllers = crate::controllers::load_global(&controllers_path);
        Files {
            settings,
            settings_path,
            appearance,
            appearance_path,
            layouts,
            layouts_path,
            keymap,
            keymap_path,
            controllers,
            controllers_path,
        }
    }
}

/// Open the sound card the settings name — or the system's default when that one is gone,
/// or a silent engine when there is none at all.
pub fn start_engine(settings: &Settings) -> EngineHandle {
    match EngineHandle::start(&settings.audio.to_config()) {
        Ok(mut e) => {
            e.set_record_offset(settings.audio.record_offset_frames);
            e
        }
        Err(e) => {
            log::error!("audio: {e}");
            EngineHandle::dummy()
        }
    }
}

/// Where the application keeps the user's files: settings, look, layouts, keymap.
pub fn config_dir() -> PathBuf {
    directories_fallback()
}

/// Show a file in the platform's file manager, selecting it where that is supported.
pub(crate) fn reveal(path: &Path) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "windows")]
    return std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(target_os = "macos")]
    return std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    return std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
}

/// The first real key pressed this frame, with its modifiers — or `None` while only
/// modifiers are held, so "Ctrl+…" can be typed without the Ctrl itself being captured.
fn captured_binding(ctx: &egui::Context) -> Option<crate::keymap::Binding> {
    ctx.input(|i| {
        let m = i.modifiers;
        i.events.iter().find_map(|e| match e {
            egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                ..
            } if *key != egui::Key::Escape => Some(crate::keymap::Binding {
                key: *key,
                command: m.command,
                shift: m.shift,
                alt: m.alt,
            }),
            // Ctrl+C / X / V arrive as clipboard events, never as keys (ADR 0036).
            e => crate::keymap::captured_clipboard_binding(e, m),
        })
    })
}

/// Where the app keeps what is the user's and not a project's: presets, recordings with no
/// project, autosaves.
pub fn data_dir() -> PathBuf {
    directories_fallback()
}

fn directories_fallback() -> PathBuf {
    // `CANTUS_DATA_DIR` puts everything elsewhere: the soak harness and the crash tests
    // run whole sessions without touching the user's own folder.
    if let Some(dir) = std::env::var_os("CANTUS_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    // A portable copy (ADR 0032) keeps everything beside its executable.
    if let Some(dir) = cantus_update::install::portable_data_dir() {
        return dir;
    }
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
/// What a new session opens with: keys and a kit, already playing a groove.
///
/// A DAW that opens on silence asks the musician to build a drum machine before they can
/// hear anything; this one opens on four bars of kick, snare and hats with a motif over
/// them, looping, so the first thing that happens is music. Each drum is **its own
/// track** with a one-pad kit — its own mixer strip, its own inserts, its own row in the
/// channel rack — which is the arrangement a musician who wants to treat the kick and the
/// snare differently needs, and the one FL's channel rack is built around.
///
/// [`starter_project`] is the smaller fixture the tests use; keeping the two apart means
/// a change to the groove cannot break a test about undo.
/// The shipped pads carry English names in `cantus-core`, which does no i18n. This maps
/// those onto the interface language; a name the user typed is left alone.
pub fn localized_pad_name(pad: &cantus_core::DrumPad) -> String {
    let key = match pad.name.as_str() {
        "Kick" => "pad.name.kick",
        "Rim" => "pad.name.rim",
        "Snare" => "pad.name.snare",
        "Closed hat" => "pad.name.closed_hat",
        "Open hat" => "pad.name.open_hat",
        "Low tom" => "pad.name.low_tom",
        "High tom" => "pad.name.high_tom",
        other => {
            return views::pulsus_editor::localized_name(other).unwrap_or_else(|| pad.name.clone())
        }
    };
    t(key)
}

pub fn welcome_project() -> Project {
    use cantus_core::{DrumPad, PulsusParams};
    let mut p = Project::new(t("app.untitled"));
    let bars2 = Ticks::beats(8);
    let length = Ticks::beats(16);
    p.loop_region = Some(cantus_core::LoopRegion {
        start: Ticks::ZERO,
        end: length,
    });
    p.loop_enabled = true;

    // Keys: a two-bar motif, repeating.
    let mut keys = p.make_track(t("track.keys"));
    let mut clip = p.make_clip(t("clip.motif"), Ticks::ZERO, length);
    clip.content_length = Some(bars2);
    let motif = [
        (0, 60),
        (1, 64),
        (2, 67),
        (3, 72),
        (4, 71),
        (5, 67),
        (6, 64),
        (7, 60),
    ];
    for (beat, pitch) in motif {
        let n = p.make_note(Ticks::beats(beat), Ticks::QUARTER - Ticks(60), pitch, 96);
        clip.notes.push(n);
    }
    keys.clips.push(clip);
    keys.gain_db = -6.0;
    p.tracks.push(keys);

    // The kit, one drum per track. Steps are 1/16ths over two bars.
    let step = Ticks::SIXTEENTH;
    let drum = |p: &mut Project, name: String, pads: Vec<DrumPad>, hits: &[(i64, u8, u8)]| {
        let mut t = p.make_track(name.clone());
        let pads = pads
            .into_iter()
            .map(|mut pad| {
                pad.name = localized_pad_name(&pad);
                pad
            })
            .collect();
        t.instrument = InstrumentSpec::Pulsus(PulsusParams::of(pads));
        let mut clip = p.make_clip(name, Ticks::ZERO, length);
        clip.content_length = Some(bars2);
        for (at, pitch, velocity) in hits {
            let n = p.make_note(step * *at, step, *pitch, *velocity);
            clip.notes.push(n);
        }
        t.clips.push(clip);
        p.tracks.push(t);
    };
    drum(
        &mut p,
        t("track.kick"),
        vec![DrumPad::kick()],
        &[(0, 36, 118), (10, 36, 100), (16, 36, 118), (26, 36, 104)],
    );
    drum(
        &mut p,
        t("track.snare"),
        vec![DrumPad::snare()],
        &[
            (4, 38, 110),
            (12, 38, 112),
            (20, 38, 110),
            (28, 38, 114),
            (30, 38, 46),
        ],
    );
    let mut hats = Vec::new();
    for i in (0..32).step_by(2) {
        let open = i == 14 || i == 30;
        let velocity = if i % 4 == 0 { 96 } else { 70 };
        hats.push((i, if open { 46 } else { 42 }, velocity));
    }
    drum(
        &mut p,
        t("track.hihat"),
        vec![DrumPad::closed_hat(), DrumPad::open_hat()],
        &hats,
    );
    p
}

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

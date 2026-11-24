//! Project ▸ Tuning (round 42, ADR 0155's surface and ADR 0156): where a professional sets
//! the song's tuning — the system (equal temperament, a historical temperament in a key, a
//! Scala scale with its keyboard map, the MTS-ESP master's), the diapason and its presets,
//! *follow the song's key*, Scala import and export, the circle of fifths coloured by each
//! fifth's and major third's beat rate, the degrees in cents and from equal, an audition —
//! and, opened from a track's inspector, the same window for that track's own tuning.
//!
//! Every change is a command with its exact inverse (`SetProjectTuning`, `SetTrackTuning`):
//! the window never writes the project. A Scala import is one `SetProjectTuning` holding the
//! parsed scale (the song keeps the scale, never a path). The window's state here is only
//! what is not the song's: which tuning it edits, the diapason being typed, the audition,
//! the last import's or export's words, MTS-ESP's link. r42:tuning-face
//!
//! Its parts: `tuning_window.rs` (the frame and the layout), `tuning_rows.rs` (the
//! controls), `tuning_circle.rs` (the circle and the degrees), `tuning_io.rs` (Scala files),
//! `tuning_words.rs` (the summary line and the names).

#[path = "tuning_chorda.rs"]
pub mod chorda;
#[path = "tuning_circle.rs"]
pub mod circle;
#[path = "tuning_io.rs"]
pub mod io;
#[path = "tuning_rows.rs"]
mod rows;
#[path = "tuning_window.rs"]
mod window;
#[path = "tuning_words.rs"]
pub mod words;

pub use window::{geo, geo_names, show, SIZE};

use crate::app::CantusApp;
use cantus_core::scale::Key;
use cantus_core::tuning::{TrackTuning, Tuning, TuningSystem, TuningTable};
use cantus_core::{Command, MergeKey, TrackId};
use cantus_engine::mts::MtsLink;

/// Which tuning the window edits.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Project,
    /// A track's own (opened from its inspector's *Its own…*).
    Track(TrackId),
}

/// The window's state that is not the song's.
#[derive(Debug, Default)]
pub struct TuningUi {
    /// The window, when open, and which tuning it edits.
    pub open: Option<Target>,
    /// The frame it opened (menus behind it close).
    pub(crate) fresh: bool,
    /// The cadence playing, if one is.
    pub audition: Option<Audition>,
    /// What the last import or export said: done (its words) or refused (why).
    pub last: Option<Result<String, String>>,
    /// MTS-ESP: the library when installed (loaded at start-up, on the main thread), or
    /// why not. `None` until the boot loads it (tests: absent).
    pub mts: Option<MtsLink>,
    /// Why *Be the MTS-ESP master* was refused, until the next attempt.
    pub master_refused: Option<cantus_rt::mts::Refusal>,
    /// The key Chorda's presets rotate to, once chosen in its card.
    pub chorda_key: Option<u8>,
    /// The project's table last published as master, with the history's revision it was
    /// made from.
    published: Option<(u64, TuningTable)>,
    /// r42 fix-engine — whether the song plays the MTS-ESP master's tuning anywhere, at the
    /// history's revision it was looked at, and the master's table last seen (`None`: no
    /// master) — a change recompiles ([`master_moved`]).
    follows_at: Option<(u64, bool)>,
    master_seen: Option<Option<TuningTable>>,
    /// When a master's change last recompiled: a master that retunes continuously (dynamic
    /// tuning) recompiles at most every [`MASTER_RECOMPILE`] (the live engine follows it
    /// each block meanwhile).
    master_compiled: Option<std::time::Instant>,
}

/// r42 fix-engine — the least time between two recompiles a master's changes cause.
pub const MASTER_RECOMPILE: std::time::Duration = std::time::Duration::from_millis(200);

/// A cadence (I–IV–V–I, or i–iv–V–i in a minor song) on one track's instrument.
#[derive(Clone, Debug)]
pub struct Audition {
    pub track: TrackId,
    /// When it started (the interface's clock, seconds).
    pub started: f64,
    /// The chord sounding now (its step) and its keys.
    pub step: Option<usize>,
    pub sounding: Vec<u8>,
    /// The tonic (pitch class) and whether minor.
    pub tonic: u8,
    pub minor: bool,
}

/// How long each chord of the audition lasts, seconds.
pub const CHORD_SECONDS: f64 = 0.75;
/// The audition's chords.
pub const STEPS: usize = 4;

/// Open the window on `target` (*Project ▸ Tuning…*, `Ctrl+Alt+T`, or a track's *Its own…*).
pub fn open(app: &mut CantusApp, target: Target) {
    app.tuning.open = Some(target);
    app.tuning.fresh = true;
    app.tuning.last = None;
    app.tuning.master_refused = None;
}

/// Close it; a cadence playing stops.
pub fn close(app: &mut CantusApp) {
    stop_audition(app);
    app.tuning.open = None;
}

/// `t` as it sounds in a song in `key`: *follow the song's key* rotates a temperament to
/// the song's tonic (ADR 0155 §5): `Tuning::resolved_for_key`, as the engine's snapshot
/// resolves it.
pub fn effective(t: &Tuning, key: Option<Key>) -> Tuning {
    t.resolved_for_key(key.map(|k| k.root))
}

/// The tuning `target` edits now (a track that follows the project, or plays equal, shows
/// what it plays: the project's, or 12-TET at 440).
pub fn tuning_of(app: &CantusApp, target: Target) -> Tuning {
    let p = app.project();
    match target {
        Target::Project => p.tuning.clone(),
        Target::Track(id) => p
            .track(id)
            .map(|t| t.tuning.resolve(&p.tuning).into_owned())
            .unwrap_or_default(),
    }
}

/// What a track's override is (`None` for the project, or a track that is gone).
pub fn track_mode(app: &CantusApp, target: Target) -> Option<TrackTuning> {
    match target {
        Target::Project => None,
        Target::Track(id) => app.project().track(id).map(|t| t.tuning.clone()),
    }
}

/// The command that makes `target`'s tuning `t` (a track's becomes its own).
pub fn command(target: Target, t: Tuning) -> Command {
    match target {
        Target::Project => Command::SetProjectTuning {
            tuning: Box::new(t),
        },
        Target::Track(track) => Command::SetTrackTuning {
            track,
            tuning: TrackTuning::Own { tuning: t },
        },
    }
}

/// Make `target`'s tuning `t`, one undo step; nothing when it already is.
pub fn set(app: &mut CantusApp, target: Target, t: Tuning) -> bool {
    if own_already(app, target, &t) {
        return false;
    }
    app.exec(command(target, t))
}

/// [`set`], merged into the step before it while the same control moves (a diapason typed
/// or stepped key by key is one undo step).
pub fn set_merging(app: &mut CantusApp, target: Target, t: Tuning, what: &'static str) -> bool {
    if own_already(app, target, &t) {
        return false;
    }
    let key = MergeKey::of(&("tuning", what, target_salt(target)));
    app.exec_merging(command(target, t), key)
}

fn target_salt(target: Target) -> u64 {
    match target {
        Target::Project => u64::MAX,
        Target::Track(id) => id.0,
    }
}

fn own_already(app: &CantusApp, target: Target, t: &Tuning) -> bool {
    match target {
        Target::Project => app.project().tuning == *t,
        Target::Track(id) => app
            .project()
            .track(id)
            .is_some_and(|tr| matches!(&tr.tuning, TrackTuning::Own { tuning } if tuning == t)),
    }
}

/// A track's override: *Follow the project*, *Equal temperament* or its own (a track that
/// had none starts its own from the project's).
pub fn set_track_mode(app: &mut CantusApp, track: TrackId, mode: TrackTuning) -> bool {
    if app.project().track(track).is_none_or(|t| t.tuning == mode) {
        return false;
    }
    app.exec(Command::SetTrackTuning {
        track,
        tuning: mode,
    })
}

/// The table `target` plays, as every unit reads it: the effective tuning's offsets, the
/// MTS-ESP master's table for `External` (equal at the diapason without a master).
pub fn table_of(app: &CantusApp, target: Target) -> TuningTable {
    let t = effective(&tuning_of(app, target), app.project().key);
    if matches!(t.system, TuningSystem::External) {
        let mut ext = TuningTable::EQUAL;
        let read = app
            .tuning
            .mts
            .as_ref()
            .is_some_and(|m| cantus_engine::tuning::ExternalTuning::read(m, &mut ext));
        return t.offsets_with(read.then_some(&ext));
    }
    t.offsets()
}

/// Every pass, whether the window is open or not: the cadence's next chord, and — when
/// Cantus is the MTS-ESP master — the project's table published when it changed (ADR 0156).
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    if app.tuning.audition.is_some() {
        let now = ctx.input(|i| i.time);
        tick_audition(app, now);
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }
    publish_if_changed(app);
}

fn publish_if_changed(app: &mut CantusApp) {
    let Some(link) = app.tuning.mts.clone() else {
        return;
    };
    if !link.is_master() {
        app.tuning.published = None;
        return;
    }
    let rev = app.bus.revision();
    if app
        .tuning
        .published
        .as_ref()
        .is_some_and(|(r, _)| *r == rev)
    {
        return;
    }
    let table = table_of(app, Target::Project);
    let changed = app
        .tuning
        .published
        .as_ref()
        .is_none_or(|(_, t)| *t != table);
    if changed {
        let name = words::summary_of(&app.project().tuning, app.project().key);
        link.publish_all(&table, &name);
    }
    app.tuning.published = Some((rev, table));
}

/// *Be the MTS-ESP master* switched (ADR 0156): registered and the project's table published,
/// or refused with the reason kept for the window.
pub fn set_master(app: &mut CantusApp, on: bool, reinitialize: bool) {
    let Some(link) = app.tuning.mts.clone() else {
        return;
    };
    if !on {
        link.resign_master();
        app.tuning.master_refused = None;
        return;
    }
    let table = table_of(app, Target::Project);
    let name = words::summary_of(&app.project().tuning, app.project().key);
    match link.become_master(reinitialize, &table, &name) {
        Ok(()) => {
            app.tuning.master_refused = None;
            app.tuning.published = Some((app.bus.revision(), table));
        }
        Err(r) => app.tuning.master_refused = Some(r),
    }
}

/// r42 fix-engine — whether the song plays the MTS-ESP master's tuning anywhere: the
/// project's (its strips, the tracks that follow it) or a track's own.
pub fn follows_master(p: &cantus_core::Project) -> bool {
    let ext = |t: &Tuning| matches!(t.system, TuningSystem::External);
    ext(&p.tuning)
        || p.tracks
            .iter()
            .any(|t| matches!(&t.tuning, TrackTuning::Own { tuning } if ext(tuning)))
}

/// r42 fix-engine — each sync, for a song that plays the MTS-ESP master's tuning: the
/// master's table read now, and `true` (the engine needs a fresh snapshot) when it is not
/// the one seen last, or the master came or went (review correctness 2, 7, 8; reach 5). The
/// live engine follows the master each block on its own; a snapshot is what Chorda's design
/// (made off the audio thread), the pitched effects' tables, a track left by its master and
/// every offline copy read.
pub fn master_moved(app: &mut CantusApp) -> bool {
    let rev = app.bus.revision();
    let follows = match app.tuning.follows_at {
        Some((r, f)) if r == rev => f,
        _ => {
            let f = follows_master(app.project());
            app.tuning.follows_at = Some((rev, f));
            f
        }
    };
    let Some(link) = app.tuning.mts.as_ref().filter(|_| follows) else {
        app.tuning.master_seen = None;
        return false;
    };
    let mut table = TuningTable::EQUAL;
    let now = cantus_engine::tuning::ExternalTuning::read(link, &mut table).then_some(table);
    if app.tuning.master_seen == Some(now) {
        return false;
    }
    if app
        .tuning
        .master_compiled
        .is_some_and(|t| t.elapsed() < MASTER_RECOMPILE)
    {
        return false; // seen again at a later sync
    }
    app.tuning.master_seen = Some(now);
    app.tuning.master_compiled = Some(std::time::Instant::now());
    true
}

/// r42 fix-engine — a new engine (an audio restart) follows the same MTS-ESP link (review
/// correctness 3, reach 3): every export and live block of it reads the master.
pub fn engine_replaced(app: &mut CantusApp) {
    if let Some(link) = app.tuning.mts.clone() {
        app.engine.set_external_tuning(std::sync::Arc::new(link));
    }
}

/// Load MTS-ESP from its standard place (the boot, on the main thread): a machine without
/// it gets the reason the window shows.
pub fn load_mts(app: &mut CantusApp) {
    if app.tuning.mts.is_none() {
        install_mts(app, MtsLink::standard());
    }
}

/// Give the app (and its engine) `link`: the boot's standard library, or a test's fake.
pub fn install_mts(app: &mut CantusApp, link: MtsLink) {
    // The engine reads the MTS-ESP master's table through the same link (ADR 0156).
    app.engine
        .set_external_tuning(std::sync::Arc::new(link.clone()));
    app.tuning.mts = Some(link);
    // r42 fix-engine — the boot's first snapshot was compiled before the link (review
    // correctness 2): compiled again with it.
    app.tuning.master_seen = None;
    app.force_resync();
}

/// The track the audition plays on: the one the window edits, else the selected track,
/// else the first — whichever plays an instrument.
pub fn audition_track(app: &CantusApp, target: Target) -> Option<TrackId> {
    let p = app.project();
    let plays = |id: TrackId| {
        p.track(id)
            .is_some_and(|t| !crate::record_session::is_audio_track(t))
    };
    match target {
        Target::Track(id) if plays(id) => Some(id),
        _ => app
            .selected_track
            .filter(|t| plays(*t))
            .or_else(|| p.tracks.iter().map(|t| t.id).find(|t| plays(*t))),
    }
}

/// The cadence's tonic: the temperament's key (or the song's, when it follows it), else the
/// song's key, else C; minor when the song is.
pub fn audition_key(app: &CantusApp, target: Target) -> (u8, bool) {
    let key = app.project().key;
    let tu = effective(&tuning_of(app, target), key);
    let minor =
        key.is_some_and(|k| k.scale.intervals().contains(&3) && !k.scale.intervals().contains(&4));
    let tonic = match tu.system {
        TuningSystem::Temperament { key, .. } => key,
        _ => key.map_or(0, |k| k.root),
    };
    (tonic % 12, minor)
}

/// The keys of the cadence's chord `step` (I, IV, V, I — i, iv, V, i in minor) on `tonic`:
/// a bass and three voices around middle C.
pub fn chord(tonic: u8, minor: bool, step: usize) -> [u8; 4] {
    let third = if minor { 3 } else { 4 };
    let (root, quality) = match step {
        1 => (5, third),
        2 => (7, 4),
        _ => (0, third),
    };
    let r = 48 + (tonic + root) % 12;
    [r, r + 12, r + 12 + quality, r + 19]
}

/// Play the cadence (again from its start).
pub fn start_audition(app: &mut CantusApp, target: Target, now: f64) {
    stop_audition(app);
    let Some(track) = audition_track(app, target) else {
        return;
    };
    let (tonic, minor) = audition_key(app, target);
    app.tuning.audition = Some(Audition {
        track,
        started: now,
        step: None,
        sounding: Vec::new(),
        tonic,
        minor,
    });
    tick_audition(app, now);
}

/// The cadence's next chord when its time comes; the last one released at the end.
pub fn tick_audition(app: &mut CantusApp, now: f64) {
    let Some(mut a) = app.tuning.audition.take() else {
        return;
    };
    let step = ((now - a.started) / CHORD_SECONDS).max(0.0) as usize;
    if a.step != Some(step) {
        for key in a.sounding.drain(..) {
            app.engine.note_off(a.track, key);
        }
        if step >= STEPS {
            return;
        }
        let keys = chord(a.tonic, a.minor, step);
        for key in keys {
            app.engine.note_on(a.track, key, 84);
        }
        a.sounding.extend_from_slice(&keys);
        a.step = Some(step);
    }
    app.tuning.audition = Some(a);
}

/// Stop the cadence: every key it holds released.
pub fn stop_audition(app: &mut CantusApp) {
    if let Some(a) = app.tuning.audition.take() {
        for key in a.sounding {
            app.engine.note_off(a.track, key);
        }
    }
}

/// Chorda's Edit ▸ Tuning (ADR 0155 §5): the line under its stretch saying which tuning the
/// piano plays — the project's, equal temperament, or the track's own — and a click on it
/// opens the Tuning window there. Chorda's stretch, stiffness and unisons stay its own.
pub fn chorda_line(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    line: egui::Rect,
    s: &crate::kit::Style,
) {
    use crate::kit::text::{self, Elide, Policy, Role, Text};
    let Some(t) = app.project().track(track) else {
        return;
    };
    let key = app.project().key;
    let plays = match &t.tuning {
        TrackTuning::Follow => match words::project_summary(app.project()) {
            Some(p) => crate::i18n::tf("tuning.chorda.project", &[("tuning", &p)]),
            None => crate::i18n::t("tuning.chorda.project_equal"),
        },
        mode => crate::i18n::tf(
            "tuning.chorda.own",
            &[("tuning", &words::track_mode_text(mode, key))],
        ),
    };
    let target = match t.tuning {
        TrackTuning::Follow => Target::Project,
        _ => Target::Track(track),
    };
    let resp = ui.interact(
        line,
        egui::Id::new(("chorda-tuning-line", track.0)),
        egui::Sense::click(),
    );
    let sd = crate::views::inst::face::on_display(&s.function(crate::kit::Function::Tone));
    crate::kit::with_brush(ui.painter(), |b| {
        let ink = if resp.hovered() {
            sd.t.text
        } else {
            sd.t.text_dim
        };
        let _ = text::paint(
            b,
            line.shrink2(egui::vec2(8.0 * s.k, 0.0)),
            &Text::new(&plays, Role::Label, ink)
                .align(egui::Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            sd.tk(),
        );
    });
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &plays,
            &crate::i18n::t("tuning.chorda.hint"),
            crate::kit::hint::Caps::NONE,
        );
    }
    if resp.clicked() {
        open(app, target);
    }
}

//! Bounce in place (round 29, ADR 0064): a track — whole, its selected clips, or the time
//! selection — rendered to an audio file and laid back **on a new track right under it**,
//! at the same place, the original muted or removed (the user's choice, remembered).
//!
//! **Source only** renders the track before its own level and balance (its instrument and
//! clips): the new audio track takes the original's level, balance and strip, so the
//! song sounds as before — to the bit, through the same chain. **With the inserts**
//! renders this track alone through its strip's chain up to the width (the strip's other
//! inputs left out): the new track feeds a new strip that takes the old one's width, pan,
//! fader, mute, solo, output and sends. Hosted plug-ins render through `render_units` /
//! `render_instruments`, as an export does; the render is one offline pass at the
//! engine's rate (`cantus_engine::freeze::render`), its tail kept until it rings out, the
//! latency trimmed so the clip's first frame is the range's first tick.
//!
//! **What goes with it** (review 1): the automation and the modulators of what the bounce
//! takes the place of — the original's level and balance (source), the old strip's fader,
//! pan, width and sends (with the inserts, a whole strip) — drive the new track or strip
//! too; the keys and followers of a strip bounced whole hear the new one; a solo moves with
//! it; the original's other automation is kept when it is removed, and an original whose
//! notes something still hears is muted, never removed. A time selection takes in the notes
//! held across its edges (its edges move out to them: nothing is cut off mid-note).
//!
//! **The project it was made from.** The render is of the project as it was when it
//! started; if anything changed meanwhile it is made again (review 1: the cut put an edit
//! back, or an out-of-date render replaced edited clips).

use crate::app::CantusApp;
use crate::freeze::Want;
use crate::i18n::{t, tf};
use cantus_core::automation::AutoTarget;
use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::modulator::{EnvTrigger, Listen, ModHost, ModSource, Modulator};
use cantus_core::{
    AudioSource, AudioSourceId, ClipId, Command, ModulatorId, Project, StripId, Ticks, Track,
    TrackId,
};
use cantus_engine::freeze::render::{render_tapped, TapOptions, TapPoint};
use cantus_engine::render::RenderProgress;
use std::path::PathBuf;
use std::sync::Arc;

/// Through what a bounce is taken.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Through {
    /// The instrument and the clips, before the track's level (its strip's chain stays).
    Source,
    /// This track through its strip's chain, up to the width.
    Inserts,
    /// A whole strip (the strip menu's): everything it gathers through its chain, up to
    /// the width. The plan's track is where the new track goes (after it).
    Strip(StripId),
}

/// What a bounce takes of the track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Range {
    /// Every clip on it.
    Track,
    /// These clips.
    Clips(Vec<ClipId>),
    /// What plays between two ticks (the clips there are cut at its edges).
    Time(Ticks, Ticks),
}

/// A bounce asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub track: TrackId,
    pub through: Through,
    pub range: Range,
}

/// A bounce's render on its way, and what the commit needs.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub plan: Plan,
    pub start: Ticks,
    /// A time selection's edges, moved out to the notes held across them.
    edges: Option<(Ticks, Ticks)>,
    /// The file being written.
    pub wav: PathBuf,
    pub rate: u32,
    /// The original's clips the bounce replaces (a track's or chosen clips').
    replaced: Vec<ClipId>,
    /// A key of what the render depends on (`freeze::region_key` of its region): an edit
    /// of it while it renders makes it again, one elsewhere does not (review 2).
    depends: String,
    /// The latency its file keeps (frames at `rate`), set by the worker: the new track
    /// reports it (`Track::lateness`, review 2).
    latency: Arc<std::sync::atomic::AtomicU64>,
}

/// What a bounce's render depends on: the region it hears (`depends`).
fn region_of(p: &Project, plan: &Plan) -> Option<freeze::Region> {
    let t = p.track(plan.track)?;
    Some(match (plan.through, t.strip) {
        (Through::Strip(s), _) => freeze::region(p, FreezeTarget::Strip(s)),
        (Through::Inserts, Some(s)) => freeze::region_through(p, plan.track, s),
        _ => freeze::region(p, FreezeTarget::Track(plan.track)),
    })
}

/// The key of what `plan` renders, as `p` holds it.
fn depends(p: &Project, plan: &Plan) -> Option<String> {
    region_of(p, plan).map(|r| freeze::region_key(p, &r))
}

/// The strips whose sound reaches strip `s` (itself, and what plays into it by outputs and
/// sends, recursively — not its keys).
fn audio_upstream(p: &Project, s: StripId) -> std::collections::BTreeSet<StripId> {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![s];
    while let Some(x) = stack.pop() {
        if seen.insert(x) {
            stack.extend(
                p.strips
                    .iter()
                    .filter(|o| o.feeds().any(|f| f == x))
                    .map(|o| o.id),
            );
        }
    }
    seen
}

/// A track that plays a live input — armed, or monitoring its input always — cannot be
/// bounced (review 2: freezing refused it, bouncing muted the input the user heard).
fn live_input(t: &Track) -> bool {
    t.armed
        || t.input
            .as_ref()
            .is_some_and(|i| i.monitor == cantus_core::MonitorMode::On)
}

/// What the original becomes (Settings: `bounce_removes`).
fn removes(app: &CantusApp) -> bool {
    app.settings.bounce_removes
}

/// The sounding clips of a track: playing lane, not muted, notes or audio.
fn sounding(t: &Track) -> impl Iterator<Item = &cantus_core::Clip> {
    t.clips
        .iter()
        .filter(|c| c.lane == 0 && !c.muted && c.automation.is_none())
}

/// Whether `track` can be bounced: there, not held by a freeze (unfreeze it first), not
/// muted (a bounce already replaced it — review 1: a second one doubled it), with
/// something to bounce. The reason, as a status line, otherwise.
pub fn refusal(app: &CantusApp, track: TrackId) -> Option<String> {
    let p = app.project();
    let Some(t) = p.track(track) else {
        return Some(t("bounce.gone"));
    };
    if let Some(owner) = app.freeze_holding_track(track) {
        return Some(tf(
            "bounce.frozen",
            &[("name", &crate::freeze::name_of(p, owner))],
        ));
    }
    if t.muted {
        return Some(tf("bounce.muted", &[("name", &t.name)]));
    }
    if live_input(t) {
        return Some(tf("bounce.live_input", &[("name", &t.name)]));
    }
    if sounding(t).next().is_none() {
        return Some(tf("bounce.nothing", &[("name", &t.name)]));
    }
    None
}

/// The strip menu's bounce of a whole strip: refused while a freeze holds it, while it is
/// muted, or when nothing it gathers has a clip. The plan otherwise (placed after its first
/// track).
pub fn strip_plan(app: &CantusApp, strip: StripId) -> Result<Plan, String> {
    let p = app.project();
    let name = crate::freeze::name_of(p, FreezeTarget::Strip(strip));
    if let Some(owner) = app.freeze_holding_strip(strip) {
        return Err(tf(
            "bounce.frozen",
            &[("name", &crate::freeze::name_of(p, owner))],
        ));
    }
    if p.strip(strip).is_some_and(|s| s.muted) {
        return Err(tf("bounce.muted", &[("name", &name)]));
    }
    let r = freeze::region(p, FreezeTarget::Strip(strip));
    // A solo inside what it bounces would be lost: the render holds all of it (review 2).
    if let Some(t) = p
        .tracks
        .iter()
        .find(|t| t.soloed && r.tracks.contains(&t.id))
    {
        return Err(tf("bounce.soloed", &[("what", &t.name)]));
    }
    if let Some(s) = p
        .strips
        .iter()
        .find(|s| s.soloed && s.id != strip && r.strips.contains(&s.id))
    {
        return Err(tf(
            "bounce.soloed",
            &[(
                "what",
                &crate::freeze::name_of(p, FreezeTarget::Strip(s.id)),
            )],
        ));
    }
    // The tracks whose sound reaches the strip (not its keys' nor its note sources').
    let upstream = audio_upstream(p, strip);
    let reaching = |t: &&Track| t.strip.is_some_and(|s| upstream.contains(&s));
    if let Some(t) = p.tracks.iter().filter(reaching).find(|t| live_input(t)) {
        return Err(tf("bounce.live_input", &[("name", &t.name)]));
    }
    // Placed after the first of them that sounds.
    let track = p
        .tracks
        .iter()
        .filter(reaching)
        .find(|t| sounding(t).next().is_some())
        .map(|t| t.id)
        .ok_or_else(|| tf("bounce.nothing", &[("name", &name)]))?;
    Ok(Plan {
        track,
        through: Through::Strip(strip),
        range: Range::Track,
    })
}

/// A bounce with the inserts mutes this track's sound in its strip: refused when the strip
/// is something else's key or a modulator follows it, unless the track is all the strip
/// gathers and the whole track is bounced — then they follow the new strip (review 1).
fn key_refusal(p: &Project, plan: &Plan) -> Option<String> {
    let Through::Inserts = plan.through else {
        return None;
    };
    let s = p.track(plan.track)?.strip?;
    if !heard_elsewhere(p, s) {
        return None;
    }
    let alone = p.tracks_on_strip(s).all(|t| t.id == plan.track)
        && !p
            .strips
            .iter()
            .any(|o| o.id != s && o.feeds().any(|to| to == s));
    (!(alone && plan.range == Range::Track)).then(|| {
        tf(
            "bounce.key",
            &[("name", &crate::freeze::name_of(p, FreezeTarget::Strip(s)))],
        )
    })
}

/// Whether strip `s` keys an insert or is followed by a modulator.
fn heard_elsewhere(p: &Project, s: StripId) -> bool {
    p.all_effects().any(|(_, e)| e.sidechain == Some(s))
        || p.all_modulators()
            .any(|(host, m)| listens(p, host, m) == Some(Listen::Strip(s)))
}

/// What a modulator listens to, its host's resolved (`None`: nothing).
fn listens(p: &Project, host: ModHost, m: &Modulator) -> Option<Listen> {
    let l = match &m.source {
        ModSource::Follower(f) => f.listen,
        ModSource::Envelope(e) => match e.trigger {
            EnvTrigger::Audio { listen, .. } => listen,
            EnvTrigger::Notes { .. } => return None,
        },
        _ => return None,
    };
    Some(match l {
        Listen::Host => p
            .host_listen_strip(host)
            .map_or(Listen::Master, Listen::Strip),
        l => l,
    })
}

/// A clips or time bounce mutes clips: refused while something hears the track's notes
/// (a muted clip sends none — review 2: a gate on them shut); the whole track can be.
fn notes_refusal(p: &Project, plan: &Plan) -> Option<String> {
    if plan.range == Range::Track || !notes_heard(p, plan.track) {
        return None;
    }
    let name = p.track(plan.track)?.name.clone();
    Some(tf("bounce.notes_heard", &[("name", &name)]))
}

/// Why `plan` cannot be bounced now, if it cannot.
fn plan_refusal(app: &CantusApp, plan: &Plan) -> Option<String> {
    match plan.through {
        Through::Strip(s) => strip_plan(app, s).err(),
        _ => refusal(app, plan.track)
            .or_else(|| key_refusal(app.project(), plan))
            .or_else(|| notes_refusal(app.project(), plan)),
    }
}

impl CantusApp {
    /// Bounce in place (the track menu, the clip menu, the strip menu): rendered on a
    /// worker, laid back in one undo step.
    pub fn bounce(&mut self, plan: Plan) {
        let refused = plan_refusal(self, &plan);
        if let Some(why) = refused {
            self.set_error(why);
            return;
        }
        crate::freeze::queue(self, Want::Bounce(plan));
    }

    /// The strip menu's *Bounce in place*: the whole strip.
    pub fn bounce_strip(&mut self, strip: StripId) {
        match strip_plan(self, strip) {
            Ok(plan) => self.bounce(plan),
            Err(why) => self.set_error(why),
        }
    }
}

/// The clips of `track` a range takes (playing lane, sounding: notes and audio).
fn clips_in(t: &Track, range: &Range) -> Vec<ClipId> {
    match range {
        Range::Track => sounding(t).map(|c| c.id).collect(),
        Range::Clips(ids) => sounding(t)
            .filter(|c| ids.contains(&c.id))
            .map(|c| c.id)
            .collect(),
        Range::Time(a, b) => sounding(t)
            .filter(|c| c.start < *b && c.end() > *a)
            .map(|c| c.id)
            .collect(),
    }
}

/// A time selection's edges moved out to the notes of `track` held across them (to a
/// fixed point): cut there, nothing is cut off mid-note (review 1: a chord held across the
/// start went silent in the bounce).
fn held_edges(p: &Project, track: TrackId, a: Ticks, b: Ticks) -> (Ticks, Ticks) {
    let Some(i) = p.track_index(track) else {
        return (a, b);
    };
    let snap = cantus_core::Snapshot::compile(p, 0);
    let events = &snap.tracks[i].events;
    // Each note's span, a note-on to its note-off.
    let mut spans: Vec<(Ticks, Ticks)> = Vec::new();
    for (k, on) in events.iter().enumerate().filter(|(_, e)| e.is_on()) {
        let off = events[k + 1..]
            .iter()
            .find(|e| !e.is_on() && e.note == on.note && e.pitch == on.pitch)
            .map_or(on.tick, |e| e.tick);
        spans.push((on.tick, off));
    }
    let (mut a, mut b) = (a, b);
    loop {
        let (na, nb) = spans.iter().fold((a, b), |(x, y), (on, off)| {
            (
                if *on < x && *off > x { x.min(*on) } else { x },
                if *on < y && *off > y { y.max(*off) } else { y },
            )
        });
        if (na, nb) == (a, b) {
            return (a, b);
        }
        (a, b) = (na, nb);
    }
}

/// Begin a bounce's render (on a worker).
pub fn start(app: &mut CantusApp, plan: Plan) -> Result<Option<crate::freeze::Job>, String> {
    if let Some(why) = plan_refusal(app, &plan) {
        return Err(why);
    }
    let project = app.project().clone();
    let orig = project
        .track(plan.track)
        .ok_or_else(|| t("bounce.gone"))?
        .clone();
    // A strip's bounce takes every clip of every track it gathers.
    let whole = match plan.through {
        Through::Strip(s) => Some(freeze::region(&project, FreezeTarget::Strip(s))),
        _ => None,
    };
    let taken: Vec<ClipId> = match &whole {
        Some(r) => project
            .tracks
            .iter()
            .filter(|x| r.tracks.contains(&x.id))
            .flat_map(|x| clips_in(x, &plan.range))
            .collect(),
        None => clips_in(&orig, &plan.range),
    };
    if taken.is_empty() {
        return Err(tf("bounce.nothing", &[("name", &orig.name)]));
    }
    let (start, end, edges) = match &plan.range {
        Range::Time(a, b) => {
            let (a, b) = held_edges(&project, plan.track, *a, *b);
            (a, b, Some((a, b)))
        }
        _ => {
            let cs = project
                .tracks
                .iter()
                .flat_map(|x| x.clips.iter())
                .filter(|c| taken.contains(&c.id));
            let a = cs.clone().map(|c| c.start).min().unwrap_or(Ticks::ZERO);
            let b = cs.map(|c| c.end()).max().unwrap_or(a);
            (a, b, None)
        }
    };
    // The render's project: the track plays what the range takes (a time range, all of
    // it: the transport stops at the end), the rest of the song resting, no solo (review
    // 1: a solo elsewhere rendered silence, and the original was then muted). Frozen
    // strips the bounce hears (a key) play their renders, as in the session.
    let mut alone = project.clone();
    freeze::unsolo(&mut alone);
    match edges {
        // A time range: the track cut at its edges and only what is inside played; the
        // song plays on through its tail (review 2: stopped, the tail met still lanes).
        Some((a, b)) => cut_inside(&mut alone, plan.track, a, b),
        None if whole.is_none() => {
            if let Some(tr) = alone.track_mut(plan.track) {
                tr.clips
                    .retain(|c| c.automation.is_some() || taken.contains(&c.id));
            }
        }
        None => {}
    }
    let (keep, point) = match (plan.through, orig.strip) {
        (Through::Strip(s), _) => (
            whole.clone().unwrap_or_default(),
            TapPoint::Strip(alone.strip_index(s).ok_or_else(|| t("bounce.gone"))?),
        ),
        (Through::Inserts, Some(s)) => {
            let keep = freeze::region_through(&alone, plan.track, s);
            // What runs only for a key or the notes an insert hears is not this track's
            // sound: it does not reach the tap (review 1: it leaked into the bounce).
            for x in &mut alone.tracks {
                if x.id != plan.track && x.strip == Some(s) {
                    x.strip = None;
                }
            }
            for o in &mut alone.strips {
                if o.id != s {
                    if o.output == Some(s) {
                        o.output = None;
                    }
                    o.sends.retain(|x| x.to != s);
                }
            }
            (
                keep,
                TapPoint::Strip(alone.strip_index(s).ok_or_else(|| t("bounce.gone"))?),
            )
        }
        // Its source, with what its modulators follow (review 1).
        _ => (
            freeze::region(&alone, FreezeTarget::Track(plan.track)),
            TapPoint::Track(
                alone
                    .track_index(plan.track)
                    .ok_or_else(|| t("bounce.gone"))?,
            ),
        ),
    };
    // A file it plays not there: no bounce of silence (review 2); one back, tried again.
    if let Some(file) = crate::freeze::missing_file(app, &keep) {
        return Err(tf("freeze.file_missing", &[("file", &file)]));
    }
    crate::freeze::retry_back(app, &keep);
    let compiled = app.engine.compile_for_render(&alone, 0);
    let mut snap = (*compiled).clone();
    freeze::isolate(&mut snap, &keep);
    let rate = app.engine.sample_rate.round() as u32;
    // Played from tick 0, as the song plays (review 2: a free LFO's phase).
    let opts = TapOptions::bounce(rate, start, end);
    // How late what it replaces is heard now — its strip's fader, or its track's level —
    // which the new track keeps (review 2): its rides and keys meet the audio as before.
    let meters = app.engine.meters();
    let live_late = match (plan.through, orig.strip) {
        (Through::Strip(s), _) | (Through::Inserts, Some(s)) => project
            .strip_index(s)
            .map_or(0, |i| meters.strip_late.get(i).copied().unwrap_or(0)),
        _ => project
            .track_index(plan.track)
            .map_or(0, |i| meters.track_late.get(i).copied().unwrap_or(0)),
    } as usize;

    let dir = app.audio_record_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let label = match plan.through {
        Through::Strip(s) => crate::freeze::name_of(&project, FreezeTarget::Strip(s)),
        _ => orig.name.clone(),
    };
    let (units, insts) = crate::freeze::units_for(app, &alone, &snap, keep.whole, false)?;
    let wav = unique(&dir, &label);
    // Written under a name of its own and renamed when whole: a render cut short leaves
    // no file that looks like a bounce (review 1).
    let part = wav.with_extension("wav.part");
    let raw = wav.with_extension("raw.part");
    let progress = Arc::new(RenderProgress::default());
    let p = progress.clone();
    let out = wav.clone();
    let snap = Arc::new(snap);
    let latency = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let late = latency.clone();
    let worker = std::thread::Builder::new()
        .name("cantus-bounce".into())
        .spawn(move || {
            let tapped = render_tapped(snap, point, &opts, units, insts, &p, &raw)?;
            if tapped.frames == 0 {
                return Err(cantus_engine::render::RenderError::Empty);
            }
            // Its file arrives as late as what it replaces is heard live (silence in
            // front when the live plan waits for more than the render's did).
            let pad = live_late.saturating_sub(tapped.latency);
            late.store(
                (tapped.latency + pad) as u64,
                std::sync::atomic::Ordering::Release,
            );
            cantus_engine::freeze::render::write_wav_padded(&tapped, &part, pad, &p)?;
            std::fs::rename(&part, &out).map_err(|e| {
                let _ = std::fs::remove_file(&part);
                cantus_engine::render::RenderError::Wav(hound::Error::IoError(e))
            })?;
            Ok(out)
        })
        .map_err(|e| e.to_string())?;
    let prepared = Prepared {
        plan: plan.clone(),
        start,
        edges,
        wav,
        rate,
        replaced: taken,
        depends: depends(&project, &plan).unwrap_or_default(),
        latency,
    };
    Ok(Some(crate::freeze::bounce_job(
        app,
        Want::Bounce(plan),
        tf("bounce.job", &[("name", &label)]),
        progress,
        worker,
        prepared,
    )))
}

/// `track` in `p` cut at `a` and `b`, only its sounding clips inside them kept (a time
/// range's render).
fn cut_inside(p: &mut Project, track: TrackId, a: Ticks, b: Ticks) {
    let mut next = p.next_id;
    for at in [a, b] {
        let crossing: Vec<cantus_core::Clip> = p
            .track(track)
            .map(|t| {
                sounding(t)
                    .filter(|c| c.start < at && c.end() > at)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        for c in crossing {
            let mut ids = || {
                let id = next;
                next += 1;
                id
            };
            if let Some(cmds) = cantus_core::clipboard::split(p, track, &c, at, &mut ids) {
                for cmd in cmds {
                    let _ = cmd.apply(p);
                }
            }
        }
    }
    p.next_id = next;
    if let Some(t) = p.track_mut(track) {
        t.clips.retain(|c| {
            c.automation.is_some() || c.lane != 0 || c.muted || (c.start >= a && c.end() <= b)
        });
    }
}

/// A free file name in `dir` for a bounce of `name`.
fn unique(dir: &std::path::Path, name: &str) -> PathBuf {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let stem = format!("{} bounce", clean.trim());
    (1..)
        .map(|n| {
            if n == 1 {
                dir.join(format!("{stem}.wav"))
            } else {
                dir.join(format!("{stem} {n}.wav"))
            }
        })
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}.wav")))
}

/// The commands that cut every sounding clip of `track` that `a` or `b` crosses (review
/// 1: the right half of a clip cut at `a` was not cut again at `b`), from the project as
/// it is now.
fn cuts(app: &mut CantusApp, track: TrackId, a: Ticks, b: Ticks) -> Vec<Command> {
    app.with_ids(|project, ids| {
        let mut scratch = project.clone();
        let mut out = Vec::new();
        for at in [a, b] {
            let crossing: Vec<cantus_core::Clip> = scratch
                .track(track)
                .map(|t| {
                    sounding(t)
                        .filter(|c| c.start < at && c.end() > at)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            for c in crossing {
                if let Some(cmds) = cantus_core::clipboard::split(&scratch, track, &c, at, ids) {
                    for cmd in &cmds {
                        let _ = cmd.clone().apply(&mut scratch);
                    }
                    out.extend(cmds);
                }
            }
        }
        out
    })
}

/// What the bounce takes the place of, as automation targets, and what drives the new
/// track or strip instead: the original's level and balance (`track`), the old strip's
/// fader, pan, width and sends (`strip`).
struct Swap {
    track: Option<(TrackId, TrackId)>,
    strip: Option<(StripId, StripId)>,
}

impl Swap {
    /// `target` moved to the new track or strip, when it is one the bounce replaces.
    fn map(&self, target: &AutoTarget) -> Option<AutoTarget> {
        match (target, self.track, self.strip) {
            (AutoTarget::TrackGain(t), Some((o, n)), _) if *t == o => {
                Some(AutoTarget::TrackGain(n))
            }
            (AutoTarget::TrackPan(t), Some((o, n)), _) if *t == o => Some(AutoTarget::TrackPan(n)),
            (AutoTarget::StripGain(s), _, Some((o, n))) if *s == o => {
                Some(AutoTarget::StripGain(n))
            }
            (AutoTarget::StripPan(s), _, Some((o, n))) if *s == o => Some(AutoTarget::StripPan(n)),
            (AutoTarget::StripWidth(s), _, Some((o, n))) if *s == o => {
                Some(AutoTarget::StripWidth(n))
            }
            (AutoTarget::Send { strip, to }, _, Some((o, n))) if *strip == o => {
                Some(AutoTarget::Send { strip: n, to: *to })
            }
            _ => None,
        }
    }
}

/// A modulator's copy for `host`, driving only `routes`, its sources pinned to what the
/// original heard (its host's notes and audio, resolved where it was).
fn carried(
    p: &Project,
    from: ModHost,
    m: &Modulator,
    id: ModulatorId,
    routes: Vec<cantus_core::ModRoute>,
    strip: Option<(StripId, StripId)>,
) -> Modulator {
    let mut c = m.clone();
    c.id = id;
    c.routes = routes;
    let notes = p.host_notes_track(from);
    let heard = |l: Listen| {
        let l = match l {
            Listen::Host => p
                .host_listen_strip(from)
                .map_or(Listen::Master, Listen::Strip),
            l => l,
        };
        match (l, strip) {
            (Listen::Strip(s), Some((o, n))) if s == o => Listen::Strip(n),
            (l, _) => l,
        }
    };
    match &mut c.source {
        ModSource::Lfo(l) => l.notes = l.notes.or(notes),
        ModSource::Envelope(e) => match &mut e.trigger {
            EnvTrigger::Notes { track } => *track = track.or(notes),
            EnvTrigger::Audio { listen, .. } => *listen = heard(*listen),
        },
        ModSource::Follower(f) => f.listen = heard(f.listen),
        _ => {}
    }
    c
}

/// The commands that carry automation and modulators to the new track or strip (review
/// 1: a fader ride stayed on the muted original, or went with a removed one).
fn carry(
    app: &mut CantusApp,
    p: &Project,
    swap: &Swap,
    onto: TrackId,
    host: ModHost,
) -> Vec<Command> {
    let mut out = Vec::new();
    for t in &p.tracks {
        for c in &t.clips {
            let Some(a) = &c.automation else {
                continue;
            };
            if let Some(target) = swap.map(&a.target) {
                let mut copy = c.clone();
                copy.id = ClipId(app.bus.alloc_id());
                copy.link = None;
                if let Some(a) = copy.automation.as_mut() {
                    a.target = target;
                }
                out.push(Command::InsertClip {
                    track: onto,
                    clip: Box::new(copy),
                });
            }
        }
    }
    for (from, m) in p.all_modulators() {
        let routes: Vec<_> = m
            .routes
            .iter()
            .filter_map(|r| {
                swap.map(&r.target).map(|target| cantus_core::ModRoute {
                    target,
                    ..r.clone()
                })
            })
            .collect();
        if routes.is_empty() {
            continue;
        }
        let id = ModulatorId(app.bus.alloc_id());
        out.push(Command::InsertModulator {
            host,
            index: 0,
            modulator: Box::new(carried(p, from, m, id, routes, swap.strip)),
        });
    }
    out
}

/// Whether something still hears `track`'s notes (an insert's note source, a modulator's
/// notes): it is then muted, never removed (review 1).
fn notes_heard(p: &Project, track: TrackId) -> bool {
    p.all_effects().any(|(_, e)| e.midi_from == Some(track))
        || p.all_modulators().any(|(host, m)| {
            let own = || p.host_notes_track(host) == Some(track);
            match &m.source {
                ModSource::Lfo(l) => {
                    l.retrigger == cantus_core::modulator::Retrigger::Note
                        && l.notes.map_or_else(own, |t| t == track)
                }
                ModSource::Envelope(e) => match e.trigger {
                    EnvTrigger::Notes { track: t } => t.map_or_else(own, |t| t == track),
                    EnvTrigger::Audio { .. } => false,
                },
                _ => false,
            }
        })
}

/// What a removed original keeps (review 1): its automation of anything but its own source
/// and its level and balance moves to the new track, and its modulators driving anything
/// else go there too.
fn rescue(app: &mut CantusApp, p: &Project, orig: TrackId, onto: TrackId) -> Vec<Command> {
    let own = |target: &AutoTarget| match target {
        AutoTarget::TrackGain(t) | AutoTarget::TrackPan(t) => *t == orig,
        AutoTarget::Instrument { track, .. }
        | AutoTarget::Firmus { track, .. }
        | AutoTarget::PluginInstrument { track, .. } => *track == orig,
        _ => false,
    };
    let mut out = Vec::new();
    if let Some(t) = p.track(orig) {
        for c in &t.clips {
            if c.automation.as_ref().is_some_and(|a| !own(&a.target)) {
                out.push(Command::MoveClipToTrack {
                    from: orig,
                    to: onto,
                    clip: c.id,
                });
            }
        }
    }
    for (from, m) in p.all_modulators() {
        if from != ModHost::Track(orig) {
            continue;
        }
        let routes: Vec<_> = m
            .routes
            .iter()
            .filter(|r| !own(&r.target))
            .cloned()
            .collect();
        if routes.is_empty() {
            continue;
        }
        let id = ModulatorId(app.bus.alloc_id());
        out.push(Command::InsertModulator {
            host: ModHost::Track(onto),
            index: 0,
            modulator: Box::new(carried(p, from, m, id, routes, None)),
        });
    }
    out
}

/// Lay the bounce back: one undo step — when the project is still the one it was made
/// from; made again otherwise (review 1).
pub fn commit(app: &mut CantusApp, prepared: Prepared, wav: PathBuf) {
    let plan = prepared.plan.clone();
    // Made again only when what it renders changed meanwhile (review 2: any edit anywhere
    // did, and a long bounce never landed while the user worked).
    if depends(app.project(), &plan).as_deref() != Some(prepared.depends.as_str()) {
        let _ = std::fs::remove_file(&wav);
        if app.project().track(plan.track).is_some() {
            app.set_status(t("bounce.again"));
            crate::freeze::queue(app, Want::Bounce(plan));
        } else {
            app.set_error(t("bounce.gone"));
        }
        return;
    }
    let Ok(reader) = hound::WavReader::open(&wav) else {
        app.set_error(tf("bounce.unreadable", &[("file", &wav.display())]));
        let _ = std::fs::remove_file(&wav);
        return;
    };
    let frames = reader.duration() as u64;
    drop(reader);
    let project = app.project().clone();
    let Some(orig) = project.track(plan.track).cloned() else {
        let _ = std::fs::remove_file(&wav);
        return;
    };
    // The cuts, from the project as it is now; after them, the sounding clips inside the
    // range are what the bounce replaces (a muted one was not rendered: kept as it is).
    let mut commands = match prepared.edges {
        Some((a, b)) => cuts(app, plan.track, a, b),
        None => Vec::new(),
    };
    let mut scratch = project.clone();
    for c in &commands {
        let _ = c.clone().apply(&mut scratch);
    }
    let replaced: Vec<ClipId> = match prepared.edges {
        Some((a, b)) => scratch
            .track(plan.track)
            .map(|t| {
                sounding(t)
                    .filter(|c| c.start >= a && c.end() <= b)
                    .map(|c| c.id)
                    .collect()
            })
            .unwrap_or_default(),
        None => prepared.replaced.clone(),
    };
    let source_id = AudioSourceId(app.bus.alloc_id());
    let base = app.audio_base().map(std::path::Path::to_path_buf);
    let whole = match plan.through {
        Through::Strip(s) => Some(s),
        _ => None,
    };
    let label = match whole {
        Some(s) => crate::freeze::name_of(&project, FreezeTarget::Strip(s)),
        None => orig.name.clone(),
    };
    let name = tf("bounce.track_name", &[("name", &label)]);
    commands.push(Command::InsertAudioSource {
        source: AudioSource {
            id: source_id,
            name: name.clone(),
            path: crate::audio_files::stored_path(base.as_deref(), &wav),
            sample_rate: prepared.rate,
            channels: 2,
            frames,
            sha256: None,
            recorded: false,
        },
    });
    // The new track, right under the original — soloed when the original is (review 1:
    // under the original's solo, it was silent).
    let mut track = app.bus.new_track(name.clone());
    track.color = orig.color;
    track.icon = orig.icon;
    track.height = orig.height;
    // A track's solo goes with it; a whole strip's bounce takes only the strip's own (no
    // track inside it can be soloed: refused — review 2).
    track.soloed = orig.soloed && whole.is_none();
    // It keeps the latency what it replaces had (review 2).
    let late = prepared.latency.load(std::sync::atomic::Ordering::Acquire);
    if late > 0 {
        track.lateness = Some(cantus_core::Lateness {
            frames: late,
            rate: prepared.rate,
        });
    }
    let through_strip = match (plan.through, orig.strip) {
        (Through::Strip(s), _) => Some(s),
        (Through::Inserts, s) => s,
        _ => None,
    };
    let new_strip: Option<cantus_core::MixerStrip> = match through_strip {
        Some(s) => project.strip(s).map(|old| {
            let mut st = app.bus.new_strip(name.clone());
            st.gain_db = old.gain_db;
            st.pan = old.pan;
            st.width = old.width;
            st.muted = old.muted;
            st.soloed = old.soloed;
            st.output = old.output;
            st.sends = old.sends.clone();
            st.color = old.color;
            st.dock = old.dock;
            st.group = old.group;
            st
        }),
        _ => None,
    };
    match &new_strip {
        Some(st) => track.strip = Some(st.id),
        None => {
            track.strip = orig.strip;
            track.gain_db = orig.gain_db;
            track.pan = orig.pan;
        }
    }
    let new_strip_id = new_strip.as_ref().map(|s| s.id);
    if let Some(st) = new_strip {
        let index = through_strip
            .and_then(|s| project.strip_index(s))
            .map_or(project.strips.len(), |i| i + 1);
        commands.push(Command::InsertStrip {
            index,
            strip: Box::new(st),
        });
    }
    let index = project.track_index(plan.track).map_or(0, |i| i + 1);
    let tid = track.id;
    commands.push(Command::InsertTrack {
        index,
        track: Box::new(track),
    });
    // Rounded up: the clip must not end before the file's last frame (a tick past it
    // plays silence; a tick short would cut the tail).
    let length = crate::audio_files::length_ticks(&project, prepared.start, frames, prepared.rate)
        .max(Ticks(1))
        + Ticks(1);
    let mut clip = app.bus.new_clip(name, prepared.start, length);
    clip.audio = Some(cantus_core::AudioRegion::of(source_id));
    commands.push(Command::InsertClip {
        track: tid,
        clip: Box::new(clip),
    });
    // What drives what the bounce replaces drives the new track or strip too.
    let swap = Swap {
        track: new_strip_id.is_none().then_some((plan.track, tid)),
        strip: through_strip.zip(new_strip_id),
    };
    let host = match new_strip_id {
        Some(s) => ModHost::Strip(s),
        None => ModHost::Track(tid),
    };
    commands.extend(carry(app, &project, &swap, tid, host));
    // A strip bounced whole (or a track that was all its strip gathered): its keys and
    // followers hear the new strip (review 1: the muted old one keyed nothing).
    if let (Some((old, new)), true) = (
        swap.strip,
        whole.is_some() || plan.through == Through::Inserts,
    ) {
        for (strip, e) in project.all_effects() {
            if e.sidechain == Some(old) {
                commands.push(Command::SetEffectSidechain {
                    strip,
                    effect: e.id,
                    source: Some(new),
                });
            }
        }
        for (host, m) in project.all_modulators() {
            if listens(&project, host, m) == Some(Listen::Strip(old)) {
                let mut m = m.clone();
                match &mut m.source {
                    ModSource::Follower(f) => f.listen = Listen::Strip(new),
                    ModSource::Envelope(e) => {
                        if let EnvTrigger::Audio { listen, .. } = &mut e.trigger {
                            *listen = Listen::Strip(new);
                        }
                    }
                    _ => {}
                }
                commands.push(Command::SetModulator {
                    modulator: Box::new(m),
                });
            }
        }
    }
    // The original: muted or removed (the user's choice); a whole strip, muted. Its solo
    // moved to what replaces it whole.
    let mut remove = removes(app);
    let mut kept = false;
    if remove && notes_heard(&project, plan.track) {
        remove = false;
        kept = true;
    }
    match plan.range {
        _ if whole.is_some() => {
            if let Some(s) = whole {
                commands.push(Command::SetStripMute { id: s, muted: true });
                if project.strip(s).is_some_and(|s| s.soloed) {
                    commands.push(Command::SetStripSolo {
                        id: s,
                        soloed: false,
                    });
                }
            }
        }
        Range::Track if remove => {
            commands.extend(rescue(app, &project, plan.track, tid));
            commands.push(Command::RemoveTrack { id: plan.track });
        }
        Range::Track => {
            commands.push(Command::SetTrackMute {
                id: plan.track,
                muted: true,
            });
            if orig.soloed {
                commands.push(Command::SetTrackSolo {
                    id: plan.track,
                    soloed: false,
                });
            }
        }
        _ => {
            for c in replaced {
                commands.push(if remove {
                    Command::RemoveClip {
                        track: plan.track,
                        clip: c,
                    }
                } else {
                    Command::SetClipMute {
                        track: plan.track,
                        clip: c,
                        muted: true,
                    }
                });
            }
        }
    }
    if app.exec(Command::Batch {
        label_key: "cmd.bounce_in_place".into(),
        commands,
    }) {
        app.select_track(tid);
        // Why "remove" was not done, said with it (review 2: it was overwritten).
        if kept {
            app.set_status(tf(
                "bounce.done_kept",
                &[("name", &label), ("track", &orig.name)],
            ));
        } else {
            app.set_status(tf("bounce.done", &[("name", &label)]));
        }
    } else {
        let _ = std::fs::remove_file(&wav);
    }
}

//! Bounce in place (round 29, ADR 0064): a track — whole, its selected clips, or the time
//! selection — rendered to an audio file and laid back **on a new track right under it**,
//! at the same place, the original muted or removed (the user's choice, remembered).
//!
//! **Source only** renders the track before its own level and balance (its instrument and
//! clips): the new audio track takes the original's level, balance and strip, so the
//! song sounds as before — to the bit, through the same chain. **With the inserts**
//! renders this track alone through its strip's chain up to the width (the strip's other
//! inputs left out): the new track feeds a new strip that takes the old one's width, pan,
//! fader, mute, output and sends. Hosted plug-ins render through `render_units` /
//! `render_instruments`, as an export does; the render is one offline pass at the
//! engine's rate (`cantus_engine::freeze::render`), its tail kept until it rings out, the
//! latency trimmed so the clip's first frame is the range's first tick.

use crate::app::CantusApp;
use crate::freeze::Want;
use crate::i18n::tf;
use cantus_core::freeze::{self, FreezeTarget, Region};
use cantus_core::{AudioSource, AudioSourceId, ClipId, Command, StripId, Ticks, Track, TrackId};
use cantus_engine::freeze::render::{render_tapped, write_wav, TapOptions, TapPoint};
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
    /// The file being written.
    pub wav: PathBuf,
    pub rate: u32,
    /// The original's clips the bounce replaces (muted or removed after), and the
    /// commands that cut the ones the time selection crosses.
    replaced: Vec<ClipId>,
    cuts: Vec<Command>,
}

/// What the original becomes (Settings: `bounce_removes`).
fn removes(app: &CantusApp) -> bool {
    app.settings.bounce_removes
}

/// Whether `track` can be bounced: there, not held by a freeze (unfreeze it first), with
/// something to bounce. The reason, as a status line, otherwise.
pub fn refusal(app: &CantusApp, track: TrackId) -> Option<String> {
    let p = app.project();
    let t = p.track(track)?;
    if let Some(owner) = app.freeze_holding_track(track) {
        return Some(tf(
            "bounce.frozen",
            &[("name", &crate::freeze::name_of(p, owner))],
        ));
    }
    if !t
        .clips
        .iter()
        .any(|c| c.lane == 0 && !c.muted && c.automation.is_none())
    {
        return Some(tf("bounce.nothing", &[("name", &t.name)]));
    }
    None
}

/// The strip menu's bounce of a whole strip: refused while a freeze holds it, or when
/// nothing it gathers has a clip. The plan otherwise (placed after its first track).
pub fn strip_plan(app: &CantusApp, strip: StripId) -> Result<Plan, String> {
    let p = app.project();
    let name = crate::freeze::name_of(p, FreezeTarget::Strip(strip));
    if let Some(owner) = app.freeze_holding_strip(strip) {
        return Err(tf(
            "bounce.frozen",
            &[("name", &crate::freeze::name_of(p, owner))],
        ));
    }
    let r = freeze::region(p, FreezeTarget::Strip(strip));
    let track = p
        .tracks
        .iter()
        .filter(|t| r.tracks.contains(&t.id))
        .find(|t| {
            t.clips
                .iter()
                .any(|c| c.lane == 0 && !c.muted && c.automation.is_none())
        })
        .map(|t| t.id)
        .ok_or_else(|| tf("bounce.nothing", &[("name", &name)]))?;
    Ok(Plan {
        track,
        through: Through::Strip(strip),
        range: Range::Track,
    })
}

impl CantusApp {
    /// Bounce in place (the track menu, the clip menu, the strip menu): rendered on a
    /// worker, laid back in one undo step.
    pub fn bounce(&mut self, plan: Plan) {
        let refused = match plan.through {
            Through::Strip(s) => strip_plan(self, s).err(),
            _ => refusal(self, plan.track),
        };
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
    let sounding = t
        .clips
        .iter()
        .filter(|c| c.lane == 0 && !c.muted && c.automation.is_none());
    match range {
        Range::Track => sounding.map(|c| c.id).collect(),
        Range::Clips(ids) => sounding
            .filter(|c| ids.contains(&c.id))
            .map(|c| c.id)
            .collect(),
        Range::Time(a, b) => sounding
            .filter(|c| c.start < *b && c.end() > *a)
            .map(|c| c.id)
            .collect(),
    }
}

/// Begin a bounce's render (on a worker).
pub fn start(app: &mut CantusApp, plan: Plan) -> Result<crate::freeze::Job, String> {
    let refused = match plan.through {
        Through::Strip(s) => strip_plan(app, s).err(),
        _ => refusal(app, plan.track),
    };
    if let Some(why) = refused {
        return Err(why);
    }
    let project = app.project().clone();
    let t = project.track(plan.track).ok_or("gone")?.clone();
    // A strip's bounce takes every clip of every track it gathers.
    let whole = match plan.through {
        Through::Strip(s) => Some(freeze::region(&project, FreezeTarget::Strip(s))),
        _ => None,
    };
    let taken = match &whole {
        Some(r) => project
            .tracks
            .iter()
            .filter(|x| r.tracks.contains(&x.id))
            .flat_map(|x| clips_in(x, &plan.range))
            .collect(),
        None => clips_in(&t, &plan.range),
    };
    if taken.is_empty() {
        return Err(tf("bounce.nothing", &[("name", &t.name)]));
    }
    let (start, end, stop) = match &plan.range {
        Range::Time(a, b) => (*a, *b, true),
        _ => {
            let cs = project
                .tracks
                .iter()
                .flat_map(|x| x.clips.iter())
                .filter(|c| taken.contains(&c.id));
            let a = cs.clone().map(|c| c.start).min().unwrap_or(Ticks::ZERO);
            let b = cs.map(|c| c.end()).max().unwrap_or(a);
            (a, b, false)
        }
    };
    // The render's project: the track plays what the range takes (a time range, all of
    // it: the transport stops at the end), the rest of the song resting. Frozen strips the
    // bounce hears (a key) play their renders, as in the session: their plug-ins are
    // unloaded there too.
    let mut alone = project.clone();
    if !matches!(plan.range, Range::Time(..)) && whole.is_none() {
        if let Some(tr) = alone.track_mut(plan.track) {
            tr.clips
                .retain(|c| c.automation.is_some() || taken.contains(&c.id));
        }
    }
    let (keep, point) = match (plan.through, t.strip) {
        (Through::Strip(s), _) => (
            whole.clone().unwrap_or_default(),
            TapPoint::Strip(alone.strip_index(s).ok_or("gone")?),
        ),
        (Through::Inserts, Some(s)) => (
            freeze::region_through(&alone, plan.track, s),
            TapPoint::Strip(alone.strip_index(s).ok_or("gone")?),
        ),
        _ => (
            Region {
                tracks: [plan.track].into_iter().collect(),
                ..Region::default()
            },
            TapPoint::Track(alone.track_index(plan.track).ok_or("gone")?),
        ),
    };
    let compiled = app.engine.compile_for_render(&alone, 0);
    let mut snap = (*compiled).clone();
    freeze::isolate(&mut snap, &keep);
    let rate = app.engine.sample_rate.round() as u32;
    let opts = TapOptions::bounce(rate, start, end, stop);
    let dir = app.audio_record_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let label = match plan.through {
        Through::Strip(s) => crate::freeze::name_of(&project, FreezeTarget::Strip(s)),
        _ => t.name.clone(),
    };
    let wav = unique(&dir, &label);
    let raw = wav.with_extension("raw.part");
    let units = app.plugins.render_units(&alone, app.engine.sample_rate);
    let insts = app
        .plugins
        .render_instruments(&alone, app.engine.sample_rate);
    let progress = Arc::new(RenderProgress::default());
    let p = progress.clone();
    let out = wav.clone();
    let snap = Arc::new(snap);
    let worker = std::thread::Builder::new()
        .name("cantus-bounce".into())
        .spawn(move || {
            let tapped = render_tapped(snap, point, &opts, units, insts, &p, &raw)?;
            if tapped.frames == 0 {
                return Err(cantus_engine::render::RenderError::Empty);
            }
            write_wav(&tapped, &out)?;
            Ok(out)
        })
        .map_err(|e| e.to_string())?;
    // The clips the time selection crosses are cut at its edges first.
    let cuts = match plan.range {
        Range::Time(a, b) => cuts(app, plan.track, &taken, a, b),
        _ => Vec::new(),
    };
    let prepared = Prepared {
        plan: plan.clone(),
        start,
        wav,
        rate,
        replaced: taken,
        cuts,
    };
    Ok(crate::freeze::bounce_job(
        Want::Bounce(plan),
        tf("bounce.job", &[("name", &label)]),
        progress,
        worker,
        prepared,
    ))
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

/// The commands that cut the clips of `taken` the range `a..b` crosses at its edges.
fn cuts(app: &mut CantusApp, track: TrackId, taken: &[ClipId], a: Ticks, b: Ticks) -> Vec<Command> {
    let taken = taken.to_vec();
    app.with_ids(|project, ids| {
        let mut scratch = project.clone();
        let mut out = Vec::new();
        for at in [a, b] {
            let crossing: Vec<cantus_core::Clip> = scratch
                .track(track)
                .map(|t| {
                    t.clips
                        .iter()
                        .filter(|c| taken.contains(&c.id) || c.start >= a && c.end() <= b)
                        .filter(|c| c.start < at && c.end() > at)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            for c in crossing {
                if let Some(cmds) = cantus_core::clipboard::split(track, &c, at, ids) {
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

/// Lay the bounce back: one undo step.
pub fn commit(app: &mut CantusApp, prepared: Prepared, wav: PathBuf) {
    let Ok(reader) = hound::WavReader::open(&wav) else {
        app.set_error(tf("bounce.unreadable", &[("file", &wav.display())]));
        return;
    };
    let frames = reader.duration() as u64;
    drop(reader);
    let plan = &prepared.plan;
    let project = app.project().clone();
    let Some(orig) = project.track(plan.track).cloned() else {
        return;
    };
    let mut commands = prepared.cuts.clone();
    // After the cuts, the clips inside the range are what the bounce replaces.
    let mut scratch = project.clone();
    for c in &commands {
        let _ = c.clone().apply(&mut scratch);
    }
    let replaced: Vec<ClipId> = match plan.range {
        Range::Time(a, b) => scratch
            .track(plan.track)
            .map(|t| {
                t.clips
                    .iter()
                    .filter(|c| c.lane == 0 && c.automation.is_none())
                    .filter(|c| c.start >= a && c.end() <= b)
                    .map(|c| c.id)
                    .collect()
            })
            .unwrap_or_default(),
        _ => prepared.replaced.clone(),
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
    // The new track, right under the original.
    let mut track = app.bus.new_track(name.clone());
    track.color = orig.color;
    track.icon = orig.icon;
    track.height = orig.height;
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
            st.output = old.output;
            st.sends = old.sends.clone();
            st.color = old.color;
            st.dock = old.dock;
            st.group = old.group;
            st
        }),
        _ => None,
    };
    match (&new_strip, plan.through) {
        (Some(st), _) => track.strip = Some(st.id),
        (None, _) => {
            track.strip = orig.strip;
            track.gain_db = orig.gain_db;
            track.pan = orig.pan;
        }
    }
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
    // The original: muted or removed (the user's choice); a whole strip, muted.
    let remove = removes(app);
    match plan.range {
        _ if whole.is_some() => {
            if let Some(s) = whole {
                commands.push(Command::SetStripMute { id: s, muted: true });
            }
        }
        Range::Track if remove => commands.push(Command::RemoveTrack { id: plan.track }),
        Range::Track => commands.push(Command::SetTrackMute {
            id: plan.track,
            muted: true,
        }),
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
        app.set_status(tf("bounce.done", &[("name", &label)]));
    } else {
        let _ = std::fs::remove_file(&wav);
    }
}

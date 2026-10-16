//! Playback snapshots: an immutable, pre-sorted view of the project that the audio thread
//! can read without locks (ADR 0004/0005).
//!
//! The UI compiles a new snapshot after every command and hands an `Arc<Snapshot>` to the
//! engine. The engine never mutates it.

use crate::effect::EffectSlot;
use crate::instrument::InstrumentSpec;
use crate::model::*;
use crate::time::{Ticks, TimeSignature};

/// A note-on (`velocity > 0`) or note-off (`velocity == 0`) at an absolute tick.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NoteEvent {
    pub tick: Ticks,
    pub pitch: Pitch,
    pub velocity: u8,
    /// The voice tag: what pairs a note-off with its note-on, and what a seeded instrument
    /// draws a note's randomness from. It is the note's own id, except where another clip
    /// on the track already plays a note of that id (a linked copy, or a copy made unique —
    /// ADR 0037): there it is the id mixed with the clip's ([`voice_tag`]), so the copies
    /// are told apart. It never depends on whether a clip is linked.
    pub note: NoteId,
    /// For a note-on, how many note-ons of this pitch come before it on the track, from
    /// the top of the piece — for an Exemplar playing its round robins in order, of its
    /// round-robin lane (`sampler::RrLanes`: the notes reaching the same samples). An
    /// instrument that cycles (a drum's round robin) reads this instead of counting hits,
    /// so a render that starts mid-song cycles the same way.
    pub ordinal: u32,
}

impl NoteEvent {
    pub fn is_on(&self) -> bool {
        self.velocity > 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackSnapshot {
    pub id: TrackId,
    pub name: String,
    /// The channel's own linear gain, before its strip.
    pub gain: f32,
    /// The channel's own balance, before its strip. Not through the pan law.
    pub pan: f32,
    /// Mute/solo already resolved.
    pub audible: bool,
    /// Index into [`Snapshot::strips`] of the strip this track feeds; `None` is master.
    pub strip: Option<usize>,
    pub instrument: InstrumentSpec,
    /// Sorted by `(tick, off-before-on)`.
    pub events: Vec<NoteEvent>,
    /// How many lanes the instrument offers. Carried so the meter and the stats can say
    /// something about a kit without the audio thread reasoning about pads.
    pub lane_count: usize,
    /// Decoded audio for each pad of a kit, aligned with its pads; `None` for a pad that
    /// is not a sample or whose file could not be read.
    ///
    /// **Empty after [`Snapshot::compile`].** This crate does no I/O, so the loader in the
    /// engine fills it in before the snapshot is sent — the same snapshot then serves
    /// real-time playback and offline render, which is what keeps the two bit-identical.
    pub samples: Vec<Option<std::sync::Arc<crate::pads::SampleData>>>,
    /// Decoded audio for a native instrument's asset keys (`NativeSpec::assets`), as
    /// `(key, audio)` in key order; `None` for a file that could not be read. Empty after
    /// [`Snapshot::compile`] and filled by the engine's loader, like `samples`.
    pub assets: Vec<(String, Option<std::sync::Arc<crate::pads::SampleData>>)>,
    /// The audio clips on the playing lane (ADR 0025), sorted by `start`, unmuted only.
    /// `data` is filled by the engine's cache (`SampleCache::attach`), like `samples`.
    pub audio: Vec<AudioClipSnapshot>,
    /// The track's input, and whether it is armed: what the engine monitors and records.
    pub input: Option<crate::audio::TrackInput>,
    pub armed: bool,
}

impl TrackSnapshot {
    /// Index of the first event at or after `tick` (binary search).
    pub fn first_event_at_or_after(&self, tick: Ticks) -> usize {
        self.events.partition_point(|e| e.tick < tick)
    }
}

/// One audio clip as the engine plays it: the window in timeline ticks, the source and how
/// to play it. Ticks, not samples: the snapshot does not know the device's rate, and the
/// transport converts exactly as it does for notes.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipSnapshot {
    pub clip: crate::model::ClipId,
    pub source: crate::model::AudioSourceId,
    /// The source's path as the project stores it; the cache resolves it.
    pub path: String,
    pub start: Ticks,
    pub end: Ticks,
    /// The content tick at `start` (the clip's `offset`): content tick 0 is the file's
    /// first frame.
    pub offset: Ticks,
    pub gain: f32,
    pub fade_in: Ticks,
    pub fade_out: Ticks,
    pub fade_in_curve: crate::audio::FadeCurve,
    pub fade_out_curve: crate::audio::FadeCurve,
    pub reverse: bool,
    /// The region's fine offset (`AudioRegion::slip_frames`) in seconds: the engine adds
    /// it, at its own rate, to the content position.
    pub slip: f64,
    /// The decoded file at the engine's rate, once attached; `None` plays silence. For a
    /// clip with pitch edits, the render of the edits once it is made (ADR 0054).
    pub data: Option<std::sync::Arc<crate::pads::SampleData>>,
    /// The clip's pitch edits (ADR 0054): the sample cache attaches their render in
    /// place of the source.
    pub pitch: Option<std::sync::Arc<crate::pitch::PitchEdit>>,
    /// A comp seam's crossfade into this window (ADR 0058): the stretch of the timeline,
    /// `(from, to)`, over which it fades in on the equal-power curve while the window
    /// before it fades out over the very same samples.
    pub xin: Option<(Ticks, Ticks)>,
    /// The seam's crossfade out of this window into the next.
    pub xout: Option<(Ticks, Ticks)>,
    /// The clip's warp (ADR 0061), unless it plays as the source: the sample cache
    /// attaches its render — laid out so frame `k` is content time `k / rate` at the
    /// project's tempo from the window's content origin, its fine offset inside — and sets
    /// `slip` to 0.
    pub warp: Option<std::sync::Arc<crate::warp::Warp>>,
    /// The source file's own rate (the warp's frames count at it).
    pub source_rate: u32,
}

/// One mixer strip, resolved for the audio thread.
#[derive(Clone, Debug, PartialEq)]
pub struct StripSnapshot {
    pub id: StripId,
    pub gain: f32,
    pub pan: f32,
    pub width: f32,
    pub phase_invert: bool,
    pub swap_channels: bool,
    /// Mute **and solo** already resolved: `true` means the strip is silent this
    /// snapshot, whichever of the two made it so (`Project::strip_audibility`).
    pub muted: bool,
    /// The whole insert chain in or out.
    pub fx_enabled: bool,
    /// Insert chain in processing order (bypassed slots included, flagged).
    pub effects: Vec<EffectSlot>,
    /// The strip's own EQ, after the inserts.
    pub eq: StripEq,
    /// Index into [`Snapshot::strips`] of the strip this one feeds; `None` is master.
    pub output: Option<usize>,
    /// Sends as `(strip index, linear level, tap)`, enabled ones only — a send that is
    /// switched off, or at zero, is not carried at all.
    ///
    /// The tap says where the engine takes the copy ([`crate::SendTap`], schema 23): `Post`
    /// after the fader and the pan law — bit-identical to every send before the tap
    /// existed — or `Pre`, after the inserts, the EQ, width and swap and before the fader
    /// and the pan law. Mute and solo (`muted`) silence both.
    pub sends: Vec<(usize, f32, crate::SendTap)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub revision: u64,
    pub tempo_bpm: f64,
    pub time_signature: TimeSignature,
    /// The tempo map (ADR 0060): the tempo and metre along the song, compiled here once;
    /// the engine's transport plays through it. A project without a tempo track has the
    /// constant map of `tempo_bpm` and `time_signature`.
    pub tempo: std::sync::Arc<crate::tempo::TempoMap>,
    pub loop_region: Option<LoopRegion>,
    pub loop_enabled: bool,
    pub metronome: bool,
    pub master_gain: f32,
    pub master_pan: f32,
    pub master_width: f32,
    pub master_phase_invert: bool,
    /// Resolved once here so the audio thread never has to know which law is in force —
    /// it reads two multipliers, the same as it always did.
    pub pan_law: PanLaw,
    pub tracks: Vec<TrackSnapshot>,
    pub strips: Vec<StripSnapshot>,
    /// Indices into `strips` in the order they must be processed: every strip after all
    /// the strips that feed it (`Project::strip_order`). Computed here, once, so the audio
    /// thread never sorts anything.
    pub strip_order: Vec<usize>,
    pub content_end: Ticks,
    /// The master's insert chain (ADR 0026) and its switch.
    pub master_effects: Vec<EffectSlot>,
    pub master_fx_enabled: bool,
    /// Recording behaviour the engine needs (the click's mode and level).
    pub recording: crate::audio::RecordSettings,
    /// Every automated target, one function of time each (ADR 0027), in target order.
    /// Empty — every project without automation clips — and the engine does nothing new.
    pub automation: Vec<crate::automation::AutoLane>,
    /// Every modulator that drives something (ADR 0057), in host order; their targets
    /// have lanes in `automation`. Empty — every project without modulators.
    pub modulators: Vec<crate::modulator::compile::ModPlan>,
    /// The pitch edits of the windows alternate comps keep (ADR 0058), with their source
    /// files: nothing plays them, but the sample cache keeps their renders, so a switch
    /// back plays the edits at once (integration review).
    pub kept_pitch: Vec<(String, std::sync::Arc<crate::pitch::PitchEdit>)>,
}

impl Snapshot {
    pub fn compile(project: &Project, revision: u64) -> Snapshot {
        let mut automation = crate::automation::compile::compile(project);
        let tracks = project
            .tracks
            .iter()
            .map(|t| TrackSnapshot {
                id: t.id,
                name: t.name.clone(),
                gain: db_to_linear(t.gain_db),
                pan: t.pan,
                audible: project.track_audible(t),
                strip: t.strip.and_then(|s| project.strip_index(s)),
                instrument: t.instrument.clone(),
                events: compile_events(t),
                lane_count: t.instrument.lanes().count(),
                samples: Vec::new(),
                assets: Vec::new(),
                audio: compile_audio(project, t),
                input: t.input.clone(),
                armed: t.armed,
            })
            .collect::<Vec<_>>();
        // Modulators (ADR 0057): a lane for every modulated target, and their plans.
        let modulators = crate::modulator::compile::compile(project, &tracks, &mut automation);
        Snapshot {
            revision,
            tempo_bpm: project.tempo_bpm,
            time_signature: project.time_signature,
            tempo: std::sync::Arc::new(project.tempo_map()),
            loop_region: if project.loop_enabled {
                project.loop_region.clone()
            } else {
                None
            },
            loop_enabled: project.loop_enabled,
            metronome: project.metronome,
            master_gain: db_to_linear(project.master.gain_db),
            master_pan: project.master.pan,
            master_width: project.master.width,
            master_phase_invert: project.master.phase_invert,
            pan_law: project.pan_law,
            tracks,
            strips: {
                let audible = project.strip_audibility();
                project
                    .strips
                    .iter()
                    .zip(audible)
                    .map(|(s, audible)| StripSnapshot {
                        id: s.id,
                        gain: db_to_linear(s.gain_db),
                        pan: s.pan,
                        width: s.width,
                        phase_invert: s.phase_invert,
                        swap_channels: s.swap_channels,
                        muted: !audible,
                        fx_enabled: s.fx_enabled,
                        effects: s.effects.clone(),
                        eq: s.eq,
                        output: s.output.and_then(|o| project.strip_index(o)),
                        sends: s
                            .sends
                            .iter()
                            // An automated send is carried even at zero: the curve may
                            // bring it up.
                            .filter(|x| {
                                x.effective_level() > 0.0
                                    || (x.enabled && automated_send(&automation, s.id, x.to))
                            })
                            .filter_map(|x| {
                                project
                                    .strip_index(x.to)
                                    .map(|i| (i, x.effective_level(), x.tap))
                            })
                            .collect(),
                    })
                    .collect()
            },
            strip_order: project.strip_order(),
            content_end: project.content_end(),
            master_effects: project.master.effects.clone(),
            master_fx_enabled: project.master.fx_enabled,
            recording: project.recording.clone(),
            automation,
            modulators,
            kept_pitch: kept_pitch(project),
        }
    }

    pub fn track(&self, id: TrackId) -> Option<&TrackSnapshot> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Every insert the snapshot holds — the strips' in order, then the master's (ADR
    /// 0026) — for whoever builds the engine's units for them.
    pub fn effect_slots(&self) -> impl Iterator<Item = &EffectSlot> {
        self.strips
            .iter()
            .flat_map(|s| s.effects.iter())
            .chain(self.master_effects.iter())
    }
}

/// The edited windows alternate comps keep, with their sources' paths.
fn kept_pitch(project: &Project) -> Vec<(String, std::sync::Arc<crate::pitch::PitchEdit>)> {
    project
        .tracks
        .iter()
        .flat_map(|t| t.comps.iter().flat_map(|c| c.windows.iter()))
        .filter_map(|w| {
            let a = w.audio.as_ref()?;
            let e = a.pitch.as_ref().filter(|e| e.edited())?;
            let src = project.audio_source(a.source)?;
            Some((src.path.clone(), std::sync::Arc::new((**e).clone())))
        })
        .collect()
}

/// Whether a lane drives the send from `strip` into `to`.
fn automated_send(lanes: &[crate::automation::AutoLane], strip: StripId, to: StripId) -> bool {
    lanes
        .iter()
        .any(|l| l.target == crate::automation::AutoTarget::Send { strip, to })
}

/// Most repetitions a looping audio clip is expanded into; a hand-edited file with a tiny
/// period must not make the snapshot huge.
const MAX_AUDIO_REPEATS: i64 = 4096;

/// The audio clips that sound: lane 0, not muted, with a source in the pool, sorted by
/// start (ADR 0025 §1).
///
/// A **looping** clip (`content_length` set) is handed to the engine as one window per
/// repetition, each showing the content from `offset` for one period, so the engine plays
/// every piece the way it plays a plain clip. The clip's fade in belongs to its first
/// piece and its fade out to its last (each limited to its piece); the pieces between are
/// unfaded, so a loop sounds continuous.
fn compile_audio(project: &Project, track: &Track) -> Vec<AudioClipSnapshot> {
    let mut out: Vec<AudioClipSnapshot> = Vec::new();
    for c in track.clips.iter().filter(|c| !c.muted && c.lane == 0) {
        let Some(a) = c.audio.as_ref() else {
            continue;
        };
        let Some(src) = project.audio_source(a.source) else {
            continue;
        };
        if c.length <= Ticks::ZERO {
            continue;
        }
        // A piece of a loop keeps its fade inside itself; a whole clip hands the engine its
        // fades as they are (the engine shortens overlapping ones in proportion).
        let looped = c.period().is_some_and(|p| p < c.length);
        let fit = |f: Ticks, len: Ticks| if looped { f.min(len) } else { f };
        let piece = |start: Ticks, end: Ticks, first: bool, last: bool| AudioClipSnapshot {
            clip: c.id,
            source: a.source,
            path: src.path.clone(),
            start,
            end,
            offset: c.offset,
            gain: db_to_linear(a.gain_db),
            fade_in: if first {
                fit(a.fade_in, end - start)
            } else {
                Ticks::ZERO
            },
            fade_out: if last {
                fit(a.fade_out, end - start)
            } else {
                Ticks::ZERO
            },
            fade_in_curve: a.fade_in_curve,
            fade_out_curve: a.fade_out_curve,
            reverse: a.reverse,
            slip: if src.sample_rate > 0 {
                a.slip_frames as f64 / src.sample_rate as f64
            } else {
                0.0
            },
            data: None,
            pitch: a.pitch.as_ref().map(|p| std::sync::Arc::new((**p).clone())),
            xin: None,
            xout: None,
            // A warp that plays as the source (ADR 0061) is no warp: the bits of today.
            warp: a
                .warp
                .as_ref()
                .filter(|w| !w.is_identity(project.tempo_bpm, a.slip_frames))
                .map(|w| std::sync::Arc::new((**w).clone())),
            source_rate: src.sample_rate,
        };
        match c.period() {
            Some(period) if period < c.length => {
                let count = ((c.length.0 + period.0 - 1) / period.0).min(MAX_AUDIO_REPEATS);
                for k in 0..count {
                    let start = c.start + Ticks(k * period.0);
                    let end = (start + period).min(c.end());
                    out.push(piece(start, end, k == 0, k == count - 1));
                }
            }
            _ => out.push(piece(c.start, c.end(), true, true)),
        }
    }
    seams(project, track, &mut out);
    out.sort_by_key(|c| c.start);
    out
}

/// Comp seams (ADR 0058): where a playing-lane audio window carrying a crossfade starts
/// exactly where another ends, the two overlap by the crossfade, centred on the seam — the
/// left reads on past it, the right starts before it (each into its own source) — and are
/// told the stretch they cross over. A looped or reversed window takes no seam.
fn seams(project: &Project, track: &Track, out: &mut [AudioClipSnapshot]) {
    let plays = |c: &&Clip| !c.muted && crate::comp::seam_window(c);
    for right in track.clips.iter().filter(plays) {
        let x = right
            .audio
            .as_ref()
            .map_or(Ticks::ZERO, |a| a.xfade.min(crate::audio::MAX_XFADE));
        if x <= Ticks::ZERO {
            continue;
        }
        let Some(left) = track
            .clips
            .iter()
            .filter(plays)
            .find(|l| crate::comp::is_seam(l, right))
        else {
            continue;
        };
        let s = right.start;
        // Centred on the seam where both sides have audio to read past it; a window at
        // its file's first (or last) frame has none, and the stretch moves over to the
        // side that has (review 1): a crossfade never cuts in.
        let before = audio_room(project, right).0;
        let after = audio_room(project, left).1;
        let mut h = Ticks(x.0 / 2).min(before);
        let tail = (x - h).min(after);
        h = (x - tail).min(before);
        if h <= Ticks::ZERO && tail <= Ticks::ZERO {
            continue;
        }
        let stretch = Some((s - h, s + tail));
        if let Some(l) = out.iter_mut().find(|p| p.clip == left.id) {
            l.end = s + tail;
            l.xout = stretch;
            l.fade_out = Ticks::ZERO;
        }
        if let Some(r) = out.iter_mut().find(|p| p.clip == right.id) {
            r.start = s - h;
            r.offset -= h;
            r.xin = stretch;
            r.fade_in = Ticks::ZERO;
        }
    }
}

/// The audio a window's source holds before its content starts and after it ends, in
/// whole ticks (rounded down): how far a seam's crossfade can read past each edge.
fn audio_room(project: &Project, c: &Clip) -> (Ticks, Ticks) {
    let Some((a, src)) = c
        .audio
        .as_ref()
        .and_then(|a| project.audio_source(a.source).map(|s| (a, s)))
    else {
        return (Ticks::ZERO, Ticks::ZERO);
    };
    let rate = f64::from(src.sample_rate.max(1));
    let map = project.tempo_map();
    if map.is_constant() {
        let bpm = project.tempo_bpm.max(1e-9);
        let frame = |t: Ticks| t.to_seconds(bpm) * rate + f64::from(a.slip_frames);
        let ticks = |frames: f64| {
            Ticks(((frames.max(0.0) / rate) * bpm / 60.0 * crate::time::PPQ as f64).floor() as i64)
        };
        return (
            ticks(frame(c.offset)),
            ticks(src.frames as f64 - frame(c.offset + c.length)),
        );
    }
    // Through the tempo map (ADR 0060, `crate::audio_time`): the ticks from the source's
    // first frame to the window's start, and from its end to the source's last frame.
    let slip = f64::from(a.slip_frames);
    let at =
        |frame: f64| crate::audio_time::tick_at_frame(&map, c.start, c.offset, slip, rate, frame);
    let (start, end) = (c.start.0 as f64, (c.start + c.length).0 as f64);
    (
        Ticks((start - at(0.0)).max(0.0).floor() as i64),
        Ticks((at(src.frames as f64) - end).max(0.0).floor() as i64),
    )
}

fn compile_events(track: &Track) -> Vec<NoteEvent> {
    let mut events = Vec::new();
    // Reused across every note so compiling a project of 20 000 notes allocates once.
    let mut at = Vec::new();
    // A muted *pad* is resolved here, exactly as a muted track is: the audio thread is
    // handed a snapshot with the events already gone, rather than a kit it has to consult
    // per note. One place decides what sounds.
    let muted_pads = track.instrument.pulsus().map(|k| {
        k.pads
            .iter()
            .filter(|p| p.muted)
            .map(|p| p.pitch)
            .collect::<Vec<_>>()
    });
    // Take lanes are never heard, and audio and automation clips have no notes to fire.
    let heard = |clip: &&Clip| {
        !(clip.muted || clip.lane != 0 || clip.audio.is_some() || clip.automation.is_some())
    };
    // Which clip each event came from — needed only where two clips could share note ids.
    let mut from: Option<Vec<u64>> = (track.clips.iter().filter(heard).count() > 1).then(Vec::new);
    for clip in track.clips.iter().filter(heard) {
        for n in &clip.notes {
            if n.muted {
                continue;
            }
            let tag = n.id;
            if muted_pads
                .as_ref()
                .is_some_and(|pads| pads.contains(&n.pitch))
            {
                continue;
            }
            // Where the note is *heard*, which is not where it is stored: a clip is a
            // window onto its content, and a looping clip shows the same note more than
            // once (`Clip::timeline_positions`).
            clip.timeline_positions(n.start, &mut at);
            for on in at.iter().copied() {
                let mut off = on + n.length;
                // A note never sounds past the clip, nor past the start of the next
                // repetition — otherwise a looped clip would stack its own tails.
                off = off.min(clip.end());
                if let Some(period) = clip.period() {
                    off = off.min(on + period);
                }
                if off <= on {
                    continue;
                }
                events.push(NoteEvent {
                    tick: on,
                    pitch: n.pitch,
                    velocity: n.velocity.max(1),
                    note: tag,
                    ordinal: 0,
                });
                events.push(NoteEvent {
                    tick: off,
                    pitch: n.pitch,
                    velocity: 0,
                    note: tag,
                    ordinal: 0,
                });
                if let Some(from) = from.as_mut() {
                    from.extend([clip.id.0, clip.id.0]);
                }
            }
        }
    }
    if let Some(from) = from {
        tell_copies_apart(&mut events, &from);
    }
    // Offs before ons at the same tick so a retriggered pitch restarts cleanly.
    events.sort_by_key(|e| (e.tick, e.is_on(), e.pitch, e.note));
    // An Exemplar playing its round robins in order counts turns per lane (the notes
    // reaching the same samples), everything else per pitch.
    match track
        .instrument
        .native()
        .and_then(crate::inst::sampler::RrLanes::of)
    {
        Some(lanes) => {
            let mut lane_of = std::collections::HashMap::new();
            let mut count: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
            for e in events.iter_mut().filter(|e| e.is_on()) {
                let key = (e.pitch.min(127), e.velocity.min(127));
                let lane = *lane_of
                    .entry(key)
                    .or_insert_with(|| lanes.lane(key.0, key.1));
                let c = count.entry(lane).or_insert(0);
                e.ordinal = *c;
                *c += 1;
            }
        }
        None => {
            let mut count = [0u32; 128];
            for e in events.iter_mut().filter(|e| e.is_on()) {
                let c = &mut count[e.pitch.min(127) as usize];
                e.ordinal = *c;
                *c += 1;
            }
        }
    }
    events
}

/// Tell apart the notes of clips that share note ids on one track (ADR 0037).
///
/// Linked copies hold the same notes under the same ids, and a copy made unique keeps
/// them; two such clips can overlap, and the engine pairs a note-off with its voice by the
/// tag — the first copy's note-off would cut the second's note. For each note id played by
/// more than one clip, the clip with the lowest id (the one that held the notes first: ids
/// only grow) keeps the note's own id, and every other gets [`voice_tag`]. Whether a clip
/// is linked plays no part, so linking, unlinking or copying a clip changes no event of the
/// clip it was copied from. `from[i]` is the clip id of `events[i]`.
fn tell_copies_apart(events: &mut [NoteEvent], from: &[u64]) {
    let mut pairs: Vec<(u64, u64)> = events
        .iter()
        .zip(from)
        .filter(|(e, _)| e.is_on())
        .map(|(e, c)| (e.note.0, *c))
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    // Sorted by note id, then clip id: the first clip of each run owns the id.
    let later: Vec<(u64, u64)> = pairs
        .windows(2)
        .filter(|w| w[0].0 == w[1].0)
        .map(|w| w[1])
        .collect();
    if later.is_empty() {
        return;
    }
    for (e, c) in events.iter_mut().zip(from) {
        if later.binary_search(&(e.note.0, *c)).is_ok() {
            e.note = NoteId(voice_tag(e.note.0, *c));
        }
    }
}

/// The voice tag of note `note` played by a clip `clip` that does not own the id: the id
/// mixed with a SplitMix64 hash of the clip's. Bit 30 and bit 63 are set, so the tag
/// never equals a note's own id (ids are counted up from 1) — neither as a 64-bit tag
/// for Cantus's instruments nor cut to the 31 bits a CLAP or VST3 note id keeps — and two
/// clips' tags differ in every part of the word.
pub fn voice_tag(note: u64, clip: u64) -> u64 {
    let mut z = clip.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (note ^ z) | (1 << 30) | (1 << 63)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_sorted_offs_first_and_clamped_to_clip() {
        let mut p = Project::default();
        let mut t = p.make_track("t");
        let mut c = p.make_clip("c", Ticks::beats(4), Ticks::beats(2));
        // Note that overhangs the clip end.
        c.notes
            .push(p.make_note(Ticks::beats(1), Ticks::beats(4), 60, 100));
        // Note starting exactly where the previous one ends... but the previous is clamped.
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(1), 60, 90));
        // Note outside the clip: ignored.
        c.notes
            .push(p.make_note(Ticks::beats(3), Ticks::beats(1), 62, 90));
        t.clips.push(c);
        p.tracks.push(t);

        let s = Snapshot::compile(&p, 7);
        let ev = &s.tracks[0].events;
        assert_eq!(ev.len(), 4);
        assert_eq!((ev[0].tick, ev[0].velocity), (Ticks::beats(4), 90));
        // At beat 5: off of first note, then on of second.
        assert_eq!((ev[1].tick, ev[1].velocity), (Ticks::beats(5), 0));
        assert_eq!((ev[2].tick, ev[2].velocity), (Ticks::beats(5), 100));
        // Clamped to clip end (beat 6), not beat 9.
        assert_eq!((ev[3].tick, ev[3].velocity), (Ticks::beats(6), 0));
        assert_eq!(s.tracks[0].first_event_at_or_after(Ticks::beats(5)), 1);
        assert_eq!(s.content_end, Ticks::beats(6));
        assert_eq!(s.revision, 7);
    }

    /// A strip's solo reaches the snapshot as a mute on everything unconnected, and a
    /// disabled send is not carried at all; an enabled one carries its tap.
    #[test]
    fn strip_solo_and_sends_are_resolved_for_the_audio_thread() {
        let mut p = Project::default();
        let (s3, s5, s6) = (p.strips[2].id, p.strips[4].id, p.strips[5].id);
        p.strips[0].output = Some(s3);
        p.strips[0].soloed = true;
        p.strips[0].sends.push(Send {
            to: s5,
            level: 0.5,
            enabled: false,
            tap: crate::model::SendTap::Post,
        });
        p.strips[0].sends.push(Send {
            to: s6,
            level: 0.25,
            enabled: true,
            tap: crate::model::SendTap::Pre,
        });
        let s = Snapshot::compile(&p, 0);
        assert!(
            !s.strips[0].muted && !s.strips[2].muted,
            "soloed, and its bus"
        );
        assert!(s.strips[1].muted, "an unconnected strip is silenced");
        // The tap travels with the send (round 25's pre-fader engine).
        assert_eq!(s.strips[0].sends, vec![(5, 0.25, SendTap::Pre)]);
    }

    #[test]
    fn audio_clips_compile_to_windows_and_loops_to_pieces() {
        let mut p = Project::default();
        let src = AudioSourceId(p.alloc_id());
        p.audio_pool.push(crate::audio::AudioSource {
            id: src,
            name: "s".into(),
            path: "Audio/s.wav".into(),
            sample_rate: 48_000,
            channels: 2,
            frames: 1000,
            sha256: None,
            recorded: false,
        });
        let mut t = p.make_track("t");
        let mut plain = p.make_audio_clip("plain", src, Ticks(100), Ticks(50));
        plain.audio.as_mut().unwrap().gain_db = -6.0;
        let mut take = p.make_audio_clip("take", src, Ticks(0), Ticks(50));
        take.lane = 1;
        let mut muted = p.make_audio_clip("muted", src, Ticks(0), Ticks(50));
        muted.muted = true;
        let orphan = p.make_audio_clip("orphan", AudioSourceId(999_999), Ticks(0), Ticks(50));
        let mut looped = p.make_audio_clip("loop", src, Ticks(1000), Ticks(250));
        looped.offset = Ticks(7);
        looped.content_length = Some(Ticks(100));
        {
            let a = looped.audio.as_mut().unwrap();
            a.fade_in = Ticks(30);
            a.fade_out = Ticks(80);
        }
        t.clips = vec![looped, plain, take, muted, orphan];
        p.tracks.push(t);
        let s = Snapshot::compile(&p, 0);
        let a = &s.tracks[0].audio;
        // The plain clip, then the loop's three pieces; takes, muted and orphans never.
        assert_eq!(a.len(), 4);
        assert_eq!(
            (a[0].start, a[0].end, a[0].path.as_str()),
            (Ticks(100), Ticks(150), "Audio/s.wav")
        );
        assert!((a[0].gain - 0.501_187).abs() < 1e-5);
        let pieces: Vec<_> = a[1..]
            .iter()
            .map(|c| (c.start.0, c.end.0, c.offset.0, c.fade_in.0, c.fade_out.0))
            .collect();
        assert_eq!(
            pieces,
            vec![
                (1000, 1100, 7, 30, 0),
                (1100, 1200, 7, 0, 0),
                (1200, 1250, 7, 0, 50)
            ]
        );
        // Note clips' events never come from audio clips.
        assert!(s.tracks[0].events.is_empty());
        // The fine offset goes to the engine in seconds of the source.
        assert_eq!(a[0].slip, 0.0);
        let c = p.tracks[0]
            .clips
            .iter_mut()
            .find(|c| c.name == "plain")
            .unwrap();
        c.audio.as_mut().unwrap().slip_frames = -12;
        let s = Snapshot::compile(&p, 0);
        assert_eq!(s.tracks[0].audio[0].slip, -12.0 / 48_000.0);
    }

    #[test]
    fn solo_resolves_audibility() {
        let mut p = Project::default();
        let a = p.make_track("a");
        let mut b = p.make_track("b");
        b.soloed = true;
        p.tracks.push(a);
        p.tracks.push(b);
        let s = Snapshot::compile(&p, 0);
        assert!(!s.tracks[0].audible);
        assert!(s.tracks[1].audible);
    }
}

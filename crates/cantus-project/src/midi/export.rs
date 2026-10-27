//! A project, a track or a clip as a type 1 Standard MIDI File.
//!
//! What is written is what the engine would play: the events come from
//! [`Snapshot::compile`], the one place that decides which notes sound — so a muted note,
//! a muted clip or a muted pad is not in the file, a looping clip is written as the notes
//! it plays, and a note that runs past its clip stops where the clip does.

use super::gm;
use super::smf::{Division, Event, MidiError, Smf, TimedEvent};
use cantus_core::{ClipId, Project, Snapshot, Ticks, TimeSignature, Track, TrackId, PPQ};
use std::collections::HashMap;

/// Cantus's own resolution. A file at this PPQ holds every position exactly.
pub const DEFAULT_PPQ: u16 = PPQ as u16;

/// What to write.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every track, on the arrangement's timeline.
    Song,
    /// One track, on the arrangement's timeline (so it lines up with the song elsewhere).
    Track(TrackId),
    /// One clip, from its own start: the file begins where the clip does.
    Clip { track: TrackId, clip: ClipId },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    pub scope: Scope,
    /// Ticks per quarter note in the file. Positions that do not fall on the file's grid
    /// are rounded to the nearest of its ticks; at [`DEFAULT_PPQ`] or any multiple of it
    /// nothing is rounded.
    pub ppq: u16,
    /// The notes as played (grooves and note effects in them) or as written (round 31,
    /// ADR 0069).
    pub notes: Notes,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            scope: Scope::Song,
            ppq: DEFAULT_PPQ,
            notes: Notes::Played,
        }
    }
}

/// Which notes a file holds (round 31, ADR 0069).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Notes {
    /// What is heard: each clip's groove applied and, through [`export_with`]'s hook,
    /// every track's note effects (an arpeggio written out note by note).
    #[default]
    Played,
    /// The notes as the clips hold them: no groove, no note effects.
    Written,
}

/// What a track's note effects play from its compiled events, until `end`, in `key`: each
/// note as `(start, length, pitch, velocity, release)` on the timeline. The engine owns the effects
/// (`cantus_engine::notefx::bake`); the application hands it in.
pub type NoteFxPlay<'a> = &'a dyn Fn(
    &cantus_core::TrackSnapshot,
    Option<cantus_core::Key>,
    Ticks,
) -> Vec<(Ticks, Ticks, u8, u8, u8)>;

/// The MIDI channel for percussion (channel 10, counted from 1).
pub const DRUM_CHANNEL: u8 = 9;

/// Cantus ticks → file ticks, rounded to the nearest (halves away from zero). Exact when
/// the file's PPQ is a multiple of Cantus's, or the position is on the file's grid.
pub fn to_file_ticks(t: Ticks, ppq: u16) -> u64 {
    let num = t.0.max(0) as i128 * ppq as i128;
    let den = PPQ as i128;
    ((num * 2 + den) / (den * 2)) as u64
}

/// Write the file.
pub fn export(project: &Project, opts: &ExportOptions) -> Result<Vec<u8>, MidiError> {
    export_with(project, opts, None)
}

/// [`export`] with the note effects' hook: the file of what is played writes each track's
/// arpeggios, chords and scales out as notes (ADR 0069).
pub fn export_with(
    project: &Project,
    opts: &ExportOptions,
    fx: Option<NoteFxPlay<'_>>,
) -> Result<Vec<u8>, MidiError> {
    Ok(super::smf::write(&to_smf_with(project, opts, fx)?))
}

/// The file as events, before it is turned into bytes.
pub fn to_smf(project: &Project, opts: &ExportOptions) -> Result<Smf, MidiError> {
    to_smf_with(project, opts, None)
}

/// [`to_smf`] with the note effects' hook ([`NoteFxPlay`]), for a file of what is played.
pub fn to_smf_with(
    project: &Project,
    opts: &ExportOptions,
    fx: Option<NoteFxPlay<'_>>,
) -> Result<Smf, MidiError> {
    // As written: the clips without their grooves, and no note effects (ADR 0069).
    let written;
    let (project, fx) = match opts.notes {
        Notes::Played => (project, fx),
        Notes::Written => {
            let mut p = project.clone();
            for c in p.tracks.iter_mut().flat_map(|t| t.clips.iter_mut()) {
                c.groove = None;
            }
            written = p;
            (&written, None)
        }
    };
    let ppq = opts.ppq.clamp(1, 0x7FFF);
    // A marker at or after the music's end (an "End", an "Outro") is the song's too: the
    // song's file runs to it (review 2, finding 65).
    // So do the tempo map's last point and last metre change (review 3: a map with no
    // clips yet — a click map for a drummer — exported as one tempo).
    let last_marker = project
        .markers
        .iter()
        .map(|m| m.tick)
        .chain(project.tempo_points.iter().map(|p| p.tick))
        .chain(project.metre_changes.iter().map(|m| m.tick))
        .max()
        .unwrap_or(Ticks::ZERO);
    // Which tracks, how far each is shifted, and where the file ends.
    let (compiled, shift, end, file_name) = match opts.scope {
        Scope::Song => {
            let snap = Snapshot::compile(project, 0);
            let tracks: Vec<usize> = (0..project.tracks.len()).collect();
            let end = project.content_end().max(last_marker);
            (
                Compiled::new(project, &snap, &tracks, fx, end),
                Ticks::ZERO,
                end,
                project.name.clone(),
            )
        }
        Scope::Track(id) => {
            let i = project.track_index(id).ok_or(MidiError::Empty)?;
            let snap = Snapshot::compile(project, 0);
            let end = project.tracks[i]
                .clips
                .iter()
                .map(|c| c.end())
                .max()
                .unwrap_or(Ticks::ZERO)
                .max(project.content_end())
                .max(last_marker);
            (
                Compiled::new(project, &snap, &[i], fx, end),
                Ticks::ZERO,
                end,
                project.tracks[i].name.clone(),
            )
        }
        Scope::Clip { track, clip } => {
            let t = project.track(track).ok_or(MidiError::Empty)?;
            let c = t.clip(clip).ok_or(MidiError::Empty)?;
            // The track alone with this one clip: the compiler then says exactly what the
            // clip plays, windows and repeats and mutes included.
            let mut alone = Project::empty(project.name.clone());
            alone.tempo_bpm = project.tempo_bpm;
            alone.time_signature = project.time_signature;
            alone.tempo_points = project.tempo_points.clone();
            alone.metre_changes = project.metre_changes.clone();
            // The pool its groove may name, and the key its note effects read (ADR 0069).
            alone.grooves = project.grooves.clone();
            alone.key = project.key;
            alone.tracks.push(Track {
                clips: vec![c.clone()],
                ..t.clone()
            });
            let snap = Snapshot::compile(&alone, 0);
            (
                Compiled::new(&alone, &snap, &[0], fx, c.end()),
                c.start,
                c.end(),
                c.name.clone(),
            )
        }
    };

    let end_tick = to_file_ticks(end - shift, ppq);
    let mut tracks = Vec::with_capacity(compiled.tracks.len() + 1);
    tracks.push(conductor(
        project,
        &file_name,
        shift,
        end - shift,
        ppq,
        end_tick,
    ));
    let mut melodic = (0u8..16).filter(|c| *c != DRUM_CHANNEL).cycle();
    for ct in &compiled.tracks {
        let channel = if ct.drums {
            DRUM_CHANNEL
        } else {
            melodic.next().unwrap_or(0)
        };
        let mut events = vec![TimedEvent {
            tick: 0,
            event: Event::TrackName(ct.name.clone()),
        }];
        let mut notes: Vec<(u64, bool, u8, u8)> = Vec::with_capacity(ct.spans.len() * 2);
        for s in &ct.spans {
            let on = to_file_ticks(s.start - shift, ppq);
            // A note is never rounded away: at least one of the file's ticks long.
            let off = to_file_ticks(s.end - shift, ppq).max(on + 1);
            notes.push((on, true, s.pitch, s.velocity));
            notes.push((off, false, s.pitch, s.release)); // ADR 0068
        }
        // By time; at one tick the offs first, so a note that ends where the next of the
        // same pitch begins does not cut it off in a reader that pairs by pitch.
        notes.sort_by_key(|(t, on, pitch, _)| (*t, *on, *pitch));
        let mut last = 0;
        for (tick, on, key, velocity) in notes {
            last = last.max(tick);
            events.push(TimedEvent {
                tick,
                event: if on {
                    Event::NoteOn {
                        channel,
                        key,
                        velocity,
                    }
                } else {
                    Event::NoteOff {
                        channel,
                        key,
                        // The release velocity when the note has one (ADR 0068), else the
                        // standard's default.
                        velocity: if velocity > 0 { velocity.min(127) } else { 64 },
                    }
                },
            });
        }
        // The controller lanes (ADR 0068): each kind's changes as MIDI carries them, at one
        // tick after the offs and before the ons (the engine's order).
        let mut sent: HashMap<cantus_core::LaneKind, u16> = HashMap::new();
        for c in ct
            .controls
            .iter()
            .filter(|c| c.tick >= shift && c.tick <= end)
        {
            let v = c.kind.midi(c.value);
            if sent.insert(c.kind, v) == Some(v) {
                continue;
            }
            let event = match c.kind {
                cantus_core::LaneKind::PitchBend => Event::PitchBend { channel, value: v },
                cantus_core::LaneKind::Pressure => Event::ChannelPressure {
                    channel,
                    value: v as u8,
                },
                cantus_core::LaneKind::Cc(n) => Event::Controller {
                    channel,
                    controller: n,
                    value: v as u8,
                },
            };
            let tick = to_file_ticks(c.tick - shift, ppq);
            last = last.max(tick);
            events.push(TimedEvent { tick, event });
        }
        let class = |e: &Event| match e {
            Event::NoteOff { .. } => 1,
            Event::NoteOn { .. } => 3,
            Event::TrackName(_) => 0,
            _ => 2,
        };
        events.sort_by_key(|e| (e.tick, class(&e.event)));
        events.push(TimedEvent {
            tick: end_tick.max(last),
            event: Event::EndOfTrack,
        });
        tracks.push(events);
    }
    Ok(Smf {
        format: 1,
        division: Division::Ppq(ppq),
        tracks,
    })
}

/// How finely a ramp is written: SMF has no ramps, so a ramp becomes a tempo event every
/// sixteenth note, each the average tempo over its sixteenth — so every sixteenth (and the
/// whole ramp) lasts exactly as long as it plays (ADR 0060).
pub const RAMP_EXPORT_STEP: i64 = PPQ / 4;

/// The most tempo events one ramp is written as (review 5).
const MAX_RAMP_STEPS: i64 = 4_096;

/// Track 0 of a type 1 file: the name of the piece, and its whole tempo map from `shift`
/// (the file's tick 0) for `len` ticks — every metre change, every tempo point (a ramp in
/// steps of [`RAMP_EXPORT_STEP`]) and every marker as a marker meta event (0x06).
fn conductor(
    project: &Project,
    name: &str,
    shift: Ticks,
    len: Ticks,
    ppq: u16,
    end: u64,
) -> Vec<TimedEvent> {
    let map = project.tempo_map();
    let at = |t: Ticks| to_file_ticks((t - shift).max(Ticks::ZERO), ppq);
    let mut ev = vec![TimedEvent {
        tick: 0,
        event: Event::TrackName(name.to_string()),
    }];
    let metre = |ev: &mut Vec<TimedEvent>, tick: u64, ts: TimeSignature| {
        let denominator_pow = (ts.denominator.max(1) as f64).log2().round() as u8;
        ev.push(TimedEvent {
            tick,
            event: Event::TimeSignature {
                numerator: ts.numerator.max(1),
                denominator_pow,
                // One metronome click per beat of the signature, in MIDI clocks (24 a quarter).
                clocks: (96 / ts.denominator.max(1) as u32).clamp(1, 255) as u8,
                n32: 8,
            },
        });
    };
    // The metre in force at the file's start, then each change inside it.
    metre(&mut ev, 0, map.signature_at(shift));
    for m in map
        .metres()
        .iter()
        .filter(|m| m.tick > shift && m.tick <= shift + len)
    {
        metre(&mut ev, at(m.tick), m.signature);
    }
    // The tempo: at the file's start, then at each point and each step of a ramp.
    let stop = shift + len;
    let tempo_at = |ev: &mut Vec<TimedEvent>, from: Ticks, to: Ticks| {
        let bpm = if map.in_ramp(from.0 as f64) && to > from {
            // The average over the step: it lasts exactly as long as the ramp's stretch.
            (to - from).as_beats_f64() * 60.0 / map.seconds_between(from, to)
        } else {
            map.bpm_at(from.0 as f64)
        };
        ev.push(TimedEvent {
            tick: at(from),
            event: Event::Tempo(tempo_us(bpm)),
        });
    };
    let mut t = shift;
    loop {
        let next_point = map
            .next_change_after(t.0 as f64)
            .map(|x| Ticks(x as i64))
            .unwrap_or(stop)
            .min(stop);
        if map.in_ramp(t.0 as f64) {
            // From the ramp's start: steps on the ramp's own sixteenths — or, past
            // `MAX_RAMP_STEPS` of them, on whole runs of sixteenths, each still its run's
            // average tempo (review 5: a ramp a million beats long looped for hours).
            let ramp_start = map.point_at_or_before(t.0 as f64) as i64;
            let ramp_len = next_point.0 - ramp_start;
            let unit =
                RAMP_EXPORT_STEP * (ramp_len / (RAMP_EXPORT_STEP * MAX_RAMP_STEPS) + 1).max(1);
            let k = (t.0 - ramp_start).div_euclid(unit) + 1;
            let step = Ticks(ramp_start + k * unit).min(next_point);
            tempo_at(&mut ev, t, step);
            t = step;
        } else {
            tempo_at(&mut ev, t, next_point);
            t = next_point;
        }
        if t >= stop {
            break;
        }
    }
    // A tempo point exactly at the end is the map's too (review 3).
    if stop > shift && project.tempo_points.iter().any(|p| p.tick == stop) {
        tempo_at(&mut ev, stop, stop);
    }
    for m in &project.markers {
        if m.tick >= shift && m.tick <= stop {
            ev.push(TimedEvent {
                tick: at(m.tick),
                event: Event::Text {
                    kind: 0x06,
                    text: m.name.clone(),
                },
            });
        }
    }
    ev.push(TimedEvent {
        tick: end,
        event: Event::EndOfTrack,
    });
    ev.sort_by_key(|e| e.tick);
    ev
}

/// Microseconds per quarter note for a tempo, as the tempo meta event carries it.
pub fn tempo_us(bpm: f64) -> u32 {
    let bpm = if bpm.is_finite() && bpm > 0.0 {
        bpm
    } else {
        120.0
    };
    (60_000_000.0 / bpm).round().clamp(1.0, 0xFF_FFFF as f64) as u32
}

/// One sounding note: where it starts and stops on the timeline.
struct Span {
    start: Ticks,
    end: Ticks,
    pitch: u8,
    velocity: u8,
    /// Its release velocity (0 none given, ADR 0068).
    release: u8,
}

struct CompiledTrack {
    name: String,
    drums: bool,
    spans: Vec<Span>,
    /// The clips' controller lanes as the engine plays them (ADR 0068).
    controls: Vec<cantus_core::CtlEvent>,
}

struct Compiled {
    tracks: Vec<CompiledTrack>,
}

impl Compiled {
    /// Pair each compiled note-on with its note-off (they share the note's id; a looping
    /// clip plays one id several times, in order) and map a kit's pads to the GM keys.
    fn new(
        project: &Project,
        snap: &Snapshot,
        which: &[usize],
        fx: Option<NoteFxPlay<'_>>,
        end: Ticks,
    ) -> Compiled {
        let mut tracks = Vec::with_capacity(which.len());
        for &i in which {
            let (Some(track), Some(st)) = (project.tracks.get(i), snap.tracks.get(i)) else {
                continue;
            };
            let kit = track.instrument.pulsus();
            let mut open: HashMap<(u64, u8), Vec<(Ticks, u8)>> = HashMap::new();
            let mut spans = Vec::new();
            // Through its note effects (round 31, ADR 0069), when the file is what is
            // played and the track has any on: what the hook says they play.
            let fx_on = st.note_fx.iter().any(|e| e.enabled);
            if let Some(play) = fx.filter(|_| fx_on) {
                for (start, length, pitch, velocity, release) in play(st, snap.key, end) {
                    let pitch = match kit {
                        Some(k) => gm::export_key(k, pitch),
                        None => pitch,
                    };
                    spans.push(Span {
                        start,
                        end: start + length,
                        pitch,
                        velocity: velocity.clamp(1, 127),
                        release,
                    });
                }
                tracks.push(CompiledTrack {
                    name: track.name.clone(),
                    drums: kit.is_some(),
                    spans,
                    controls: st.controls.clone(),
                });
                continue;
            }
            for ev in &st.events {
                let key = (ev.note.0, ev.pitch);
                if ev.is_on() {
                    open.entry(key).or_default().push((ev.tick, ev.velocity));
                } else if let Some(list) = open.get_mut(&key) {
                    if !list.is_empty() {
                        let (start, velocity) = list.remove(0);
                        let pitch = match kit {
                            Some(k) => gm::export_key(k, ev.pitch),
                            None => ev.pitch,
                        };
                        spans.push(Span {
                            start,
                            end: ev.tick,
                            pitch,
                            velocity: velocity.clamp(1, 127),
                            release: ev.release,
                        });
                    }
                }
            }
            tracks.push(CompiledTrack {
                name: track.name.clone(),
                drums: kit.is_some(),
                spans,
                controls: st.controls.clone(),
            });
        }
        Compiled { tracks }
    }
}

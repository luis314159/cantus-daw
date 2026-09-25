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
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            scope: Scope::Song,
            ppq: DEFAULT_PPQ,
        }
    }
}

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
    Ok(super::smf::write(&to_smf(project, opts)?))
}

/// The file as events, before it is turned into bytes.
pub fn to_smf(project: &Project, opts: &ExportOptions) -> Result<Smf, MidiError> {
    let ppq = opts.ppq.clamp(1, 0x7FFF);
    // Which tracks, how far each is shifted, and where the file ends.
    let (compiled, shift, end, file_name) = match opts.scope {
        Scope::Song => {
            let snap = Snapshot::compile(project, 0);
            let tracks: Vec<usize> = (0..project.tracks.len()).collect();
            let end = project.content_end();
            (
                Compiled::new(project, &snap, &tracks),
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
                .max(project.content_end());
            (
                Compiled::new(project, &snap, &[i]),
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
            alone.tracks.push(Track {
                clips: vec![c.clone()],
                ..t.clone()
            });
            let snap = Snapshot::compile(&alone, 0);
            (
                Compiled::new(&alone, &snap, &[0]),
                c.start,
                c.end(),
                c.name.clone(),
            )
        }
    };

    let end_tick = to_file_ticks(end - shift, ppq);
    let mut tracks = Vec::with_capacity(compiled.tracks.len() + 1);
    tracks.push(conductor(
        &file_name,
        project.tempo_bpm,
        project.time_signature,
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
            notes.push((off, false, s.pitch, 0));
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
                        velocity: 64,
                    }
                },
            });
        }
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

/// Track 0 of a type 1 file: the name of the piece, its metre and its tempo.
fn conductor(name: &str, bpm: f64, ts: TimeSignature, end: u64) -> Vec<TimedEvent> {
    let at0 = |event| TimedEvent { tick: 0, event };
    let denominator_pow = (ts.denominator.max(1) as f64).log2().round() as u8;
    vec![
        at0(Event::TrackName(name.to_string())),
        at0(Event::TimeSignature {
            numerator: ts.numerator.max(1),
            denominator_pow,
            // One metronome click per beat of the signature, in MIDI clocks (24 a quarter).
            clocks: (96 / ts.denominator.max(1) as u32).clamp(1, 255) as u8,
            n32: 8,
        }),
        at0(Event::Tempo(tempo_us(bpm))),
        TimedEvent {
            tick: end,
            event: Event::EndOfTrack,
        },
    ]
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
}

struct CompiledTrack {
    name: String,
    drums: bool,
    spans: Vec<Span>,
}

struct Compiled {
    tracks: Vec<CompiledTrack>,
}

impl Compiled {
    /// Pair each compiled note-on with its note-off (they share the note's id; a looping
    /// clip plays one id several times, in order) and map a kit's pads to the GM keys.
    fn new(project: &Project, snap: &Snapshot, which: &[usize]) -> Compiled {
        let mut tracks = Vec::with_capacity(which.len());
        for &i in which {
            let (Some(track), Some(st)) = (project.tracks.get(i), snap.tracks.get(i)) else {
                continue;
            };
            let kit = track.instrument.pulsus();
            let mut open: HashMap<(u64, u8), Vec<(Ticks, u8)>> = HashMap::new();
            let mut spans = Vec::new();
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
                        });
                    }
                }
            }
            tracks.push(CompiledTrack {
                name: track.name.clone(),
                drums: kit.is_some(),
                spans,
            });
        }
        Compiled { tracks }
    }
}

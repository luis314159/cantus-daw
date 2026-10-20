//! A Standard MIDI File (type 0, 1 or 2) as new tracks and clips.
//!
//! Reading is separate from placing: [`read`] turns bytes into tracks of notes and a
//! report, with no project in sight; [`import_commands`] turns that into commands for a
//! given project — one `Batch`, so the whole import is one step of the undo history.

use super::gm;
use super::smf::{self, Division, Event, MidiError};
use crate::report::Report;
use cantus_core::{
    Clip, ClipId, Color, Command, Dock, InstrumentSpec, Note, NoteId, Pitch, Project, PulsusParams,
    StripId, Ticks, TimeSignature, Track, TrackId, PPQ,
};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// What a file's tempo changes become.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TempoMode {
    /// Keep the file's tempo map (round 29, ADR 0060): every note on the beat it was
    /// written on, and the file's tempo changes, metre changes and markers in the project's
    /// tempo track, metre and marker lane — when the import sets the project's tempo; the
    /// piece then plays exactly as the file does.
    #[default]
    Map,
    /// Keep the bars: every note stays on the beat it was written on, and the piece plays
    /// at one tempo, the file's first. What a sequenced file wants.
    Beats,
    /// Keep the time: every note sounds at the second it did in the file, at the project's
    /// tempo, off the grid if it has to be. What a recorded performance with a tempo map
    /// wants.
    Seconds,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ImportOptions {
    pub tempo_mode: TempoMode,
    /// The tempo the notes will play at, for [`TempoMode::Seconds`] and SMPTE files:
    /// `None` is the file's own first tempo (when the import sets the project's tempo),
    /// `Some` the project's (when it does not).
    pub target_bpm: Option<f64>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions {
            tempo_mode: TempoMode::Map,
            target_bpm: None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ImportedNote {
    /// From the start of the file, in Cantus ticks.
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: Pitch,
    pub velocity: u8,
}

/// Where a track's name comes from, so the interface can say it in its own language when
/// the file does not name the track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackLabel {
    /// The file named it (meta event 3).
    Named(String),
    /// The file named the track, which played on several channels: this is one of them.
    NamedChannel(String, u8),
    /// No name, but a GM program: "Fingered Bass".
    Program(&'static str),
    /// Channel 10, unnamed.
    Drums,
    /// Nothing to go on: the channel, counted from 1.
    Channel(u8),
}

impl TrackLabel {
    /// The name in English, for callers with no locale.
    pub fn english(&self) -> String {
        match self {
            TrackLabel::Named(n) => n.clone(),
            TrackLabel::NamedChannel(n, c) => format!("{n} (ch {c})"),
            TrackLabel::Program(p) => p.to_string(),
            TrackLabel::Drums => "Drums".to_string(),
            TrackLabel::Channel(c) => format!("Channel {c}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedTrack {
    pub label: TrackLabel,
    /// 0–15.
    pub channel: u8,
    /// Channel 10: becomes a kit.
    pub drums: bool,
    /// Sorted by start, then pitch.
    pub notes: Vec<ImportedNote>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MidiImport {
    pub format: u16,
    /// The file's tempo at its start (120 when it says nothing, as the standard has it).
    pub tempo_bpm: f64,
    /// The metre at the start, if the file gives one.
    pub time_signature: Option<TimeSignature>,
    /// The name of the piece (type 1: the first track's name; type 0: the track's).
    pub name: Option<String>,
    pub tracks: Vec<ImportedTrack>,
    /// Where the file ends, in Cantus ticks: the last note-off or end-of-track.
    pub end: Ticks,
    pub report: Report,
    /// How the file's tempo was read.
    pub tempo_mode: TempoMode,
    /// The file's tempo changes, in Cantus ticks (the first at 0): what [`TempoMode::Map`]
    /// puts in the tempo track (round 29, ADR 0060).
    pub tempo_points: Vec<cantus_core::TempoPoint>,
    /// The file's metre changes (the first at 0, when the file gives one).
    pub metre_changes: Vec<cantus_core::MetreChange>,
    /// The file's markers (meta event 6): where and what.
    pub markers: Vec<(Ticks, String)>,
}

impl MidiImport {
    pub fn note_count(&self) -> usize {
        self.tracks.iter().map(|t| t.notes.len()).sum()
    }
}

/// A 64th note: what a note with no length in the file becomes, so it can be heard.
pub const MIN_NOTE: Ticks = Ticks(PPQ / 16);

/// One tempo segment: from `tick` (file ticks) on, `us` microseconds per quarter.
#[derive(Copy, Clone, Debug)]
struct TempoPoint {
    tick: u64,
    us: u32,
    /// Seconds at `tick`.
    seconds: f64,
}

/// The file's clock: file ticks → Cantus ticks, by beats or by seconds.
struct Clock {
    division: Division,
    mode: TempoMode,
    map: Vec<TempoPoint>,
    target_bpm: f64,
}

impl Clock {
    fn seconds(&self, t: u64) -> f64 {
        match self.division {
            Division::Smpte { .. } => {
                t as f64 / self.division.smpte_ticks_per_second().unwrap_or(1.0)
            }
            Division::Ppq(ppq) => {
                let i = self.map.partition_point(|p| p.tick <= t).saturating_sub(1);
                let p = self.map[i];
                p.seconds + (t - p.tick) as f64 * p.us as f64 / 1e6 / ppq as f64
            }
        }
    }

    fn ticks(&self, t: u64) -> Ticks {
        match (self.division, self.mode) {
            (Division::Ppq(ppq), TempoMode::Beats | TempoMode::Map) => {
                let num = t as i128 * PPQ as i128;
                let den = ppq as i128;
                Ticks(((num * 2 + den) / (den * 2)) as i64)
            }
            _ => {
                let beats = self.seconds(t) * self.target_bpm / 60.0;
                Ticks((beats * PPQ as f64).round() as i64)
            }
        }
    }
}

/// Read a file into tracks of notes, and say what was left behind.
pub fn read(bytes: &[u8], opts: &ImportOptions) -> Result<MidiImport, MidiError> {
    let file = smf::read(bytes)?;
    let mut report = Report::default();

    // The tempo map, from every track: a type 1 file keeps it in the first, but not every
    // writer knows that.
    let mut tempos: BTreeMap<u64, u32> = BTreeMap::new();
    let mut metres: BTreeMap<u64, (u8, u8)> = BTreeMap::new();
    let mut marks: Vec<(u64, String)> = Vec::new();
    for track in &file.tracks {
        for te in track {
            if let Event::Text { kind: 0x06, text } = &te.event {
                if !text.trim().is_empty() {
                    marks.push((te.tick, text.trim().to_string()));
                }
            }
            match te.event {
                Event::Tempo(us) if us > 0 => {
                    tempos.insert(te.tick, us);
                }
                Event::TimeSignature {
                    numerator,
                    denominator_pow,
                    ..
                } if numerator > 0 && denominator_pow <= 7 => {
                    metres.insert(te.tick, (numerator, 1u8 << denominator_pow));
                }
                _ => {}
            }
        }
    }
    let first_us = tempos.get(&0).copied().unwrap_or(500_000);
    let tempo_bpm = round_bpm(60_000_000.0 / first_us as f64);
    let ppq = match file.division {
        Division::Ppq(p) => p as u64,
        Division::Smpte { .. } => 1,
    };
    let mut map = vec![TempoPoint {
        tick: 0,
        us: first_us,
        seconds: 0.0,
    }];
    for (&tick, &us) in tempos.range(1..) {
        let last = *map.last().expect("the map starts with one point");
        let seconds = last.seconds + (tick - last.tick) as f64 * last.us as f64 / 1e6 / ppq as f64;
        map.push(TempoPoint { tick, us, seconds });
    }
    let changes = map.windows(2).filter(|w| w[0].us != w[1].us).count();
    let smpte = matches!(file.division, Division::Smpte { .. });
    let clock = Clock {
        division: file.division,
        mode: if smpte {
            TempoMode::Seconds
        } else {
            opts.tempo_mode
        },
        map,
        target_bpm: opts.target_bpm.unwrap_or(tempo_bpm),
    };
    if smpte {
        report.info(
            "interop.report.smpte",
            vec![("bpm", format!("{:.2}", clock.target_bpm))],
        );
    }
    // The map in Cantus ticks (by beats), for TempoMode::Map.
    let beats_tick = |t: u64| match file.division {
        Division::Ppq(p) => {
            let (num, den) = (t as i128 * PPQ as i128, p as i128);
            Ticks(((num * 2 + den) / (den * 2)) as i64)
        }
        Division::Smpte { .. } => Ticks::ZERO,
    };
    let keep_map = clock.mode == TempoMode::Map;
    let mut tempo_points: Vec<cantus_core::TempoPoint> = Vec::new();
    for p in &clock.map {
        let bpm = cantus_core::time::clamp_bpm(round_bpm(60_000_000.0 / p.us as f64));
        let tick = beats_tick(p.tick);
        if tempo_points.last().is_some_and(|l| l.bpm == bpm) {
            continue;
        }
        if tempo_points.last().is_some_and(|l| l.tick == tick) {
            tempo_points.pop();
        }
        tempo_points.push(cantus_core::TempoPoint {
            tick,
            bpm,
            ramp: false,
        });
    }
    if tempo_points.len() < 2 {
        tempo_points.clear();
    }
    let mut metre_list: Vec<cantus_core::MetreChange> = metres
        .iter()
        .map(|(t, (n, d))| cantus_core::MetreChange {
            tick: beats_tick(*t),
            signature: TimeSignature::new(*n, *d),
        })
        .collect();
    metre_list.dedup_by(|b, a| a.signature == b.signature);
    if metre_list.first().is_some_and(|m| m.tick != Ticks::ZERO) {
        metre_list.insert(
            0,
            cantus_core::MetreChange {
                tick: Ticks::ZERO,
                signature: TimeSignature::default(),
            },
        );
    }
    if metre_list.len() < 2 {
        metre_list.clear();
    }
    let markers: Vec<(Ticks, String)> =
        marks.into_iter().map(|(t, s)| (beats_tick(t), s)).collect();
    if keep_map && changes > 0 {
        report.info("tempo.report.tempo_kept", vec![("n", changes.to_string())]);
    } else if changes > 0 {
        if clock.mode == TempoMode::Seconds {
            report.info(
                "interop.report.tempo_retimed",
                vec![("n", changes.to_string())],
            );
        } else {
            report.dropped(
                "interop.report.tempo_changes",
                vec![("n", changes.to_string())],
            );
        }
    }
    let time_signature = metres.get(&0).map(|(n, d)| TimeSignature::new(*n, *d));
    let metre_changes = metres
        .iter()
        .filter(|(t, _)| **t > 0)
        .filter(|(_, m)| Some(TimeSignature::new(m.0, m.1)) != time_signature)
        .count();
    if keep_map {
        if metre_changes > 0 {
            report.info(
                "tempo.report.metres_kept",
                vec![("n", metre_changes.to_string())],
            );
        }
        if !markers.is_empty() {
            report.info(
                "tempo.report.markers_kept",
                vec![("n", markers.len().to_string())],
            );
        }
    } else {
        report.count_dropped("interop.report.metre_changes", metre_changes);
    }
    let tempo_mode = clock.mode;

    let mut counts = Dropped::default();
    let mut tracks = Vec::new();
    let mut name = None;
    let mut end = Ticks::ZERO;
    for (ti, track) in file.tracks.iter().enumerate() {
        let track_name = track.iter().find_map(|te| match &te.event {
            Event::TrackName(n) if !n.trim().is_empty() => Some(n.trim().to_string()),
            _ => None,
        });
        // A type 1 file's first track is the conductor: its name is the piece's.
        if ti == 0 && name.is_none() {
            name = track_name.clone();
        }
        let mut channels: BTreeMap<u8, Building> = BTreeMap::new();
        let mut last_tick = 0u64;
        for te in track {
            last_tick = last_tick.max(te.tick);
            counts.count(&te.event);
            match te.event {
                Event::NoteOn {
                    channel,
                    key,
                    velocity,
                } if velocity > 0 => {
                    channels
                        .entry(channel)
                        .or_default()
                        .open
                        .entry(key)
                        .or_default()
                        .push_back((te.tick, velocity));
                }
                Event::NoteOn { channel, key, .. } | Event::NoteOff { channel, key, .. } => {
                    // First on, first off: the convention every sequencer follows for a
                    // pitch struck again before it was released.
                    if let Some(b) = channels.get_mut(&channel) {
                        if let Some((on, velocity)) =
                            b.open.get_mut(&key).and_then(VecDeque::pop_front)
                        {
                            b.notes.push((on, te.tick, key, velocity));
                        }
                    }
                }
                Event::Program { channel, program } => {
                    channels
                        .entry(channel)
                        .or_default()
                        .program
                        .get_or_insert(program);
                }
                _ => {}
            }
        }
        let several = channels.values().filter(|b| b.has_notes()).count() > 1;
        for (channel, mut b) in channels {
            // A note still held at the end of its track ends there.
            for (key, open) in std::mem::take(&mut b.open) {
                for (on, velocity) in open {
                    counts.unterminated += 1;
                    b.notes.push((on, last_tick.max(on), key, velocity));
                }
            }
            if b.notes.is_empty() {
                continue;
            }
            let drums = channel == super::export::DRUM_CHANNEL;
            // In a type 0 file the one track's name is the piece's, and each channel is
            // better called by what it plays; in a type 1 file the conductor's name is
            // the piece's too.
            let own_name = if file.format == 0 || (ti == 0 && file.tracks.len() > 1) {
                None
            } else {
                track_name.as_ref()
            };
            let label = match (own_name, several) {
                (Some(n), false) => TrackLabel::Named(n.clone()),
                (Some(n), true) => TrackLabel::NamedChannel(n.clone(), channel + 1),
                _ if drums => TrackLabel::Drums,
                _ => match b.program {
                    Some(p) => TrackLabel::Program(gm::program_name(p)),
                    None => TrackLabel::Channel(channel + 1),
                },
            };
            let mut notes: Vec<ImportedNote> = b
                .notes
                .iter()
                .map(|&(on, off, key, velocity)| {
                    let start = clock.ticks(on);
                    let mut length = clock.ticks(off) - start;
                    if length <= Ticks::ZERO {
                        counts.zero_length += 1;
                        length = MIN_NOTE;
                    }
                    ImportedNote {
                        start,
                        length,
                        pitch: key.min(127),
                        velocity: velocity.clamp(1, 127),
                    }
                })
                .collect();
            notes.sort_by_key(|n| (n.start, n.pitch));
            if let Some(last) = notes.iter().map(|n| n.start + n.length).max() {
                end = end.max(last);
            }
            tracks.push(ImportedTrack {
                label,
                channel,
                drums,
                notes,
            });
        }
        end = end.max(clock.ticks(last_tick));
    }
    counts.report(&mut report);
    if file.format == 2 {
        report.info("interop.report.format2", vec![]);
    }
    report.info(
        "interop.report.midi_read",
        vec![
            ("format", file.format.to_string()),
            ("tracks", tracks.len().to_string()),
            (
                "notes",
                tracks
                    .iter()
                    .map(|t| t.notes.len())
                    .sum::<usize>()
                    .to_string(),
            ),
        ],
    );
    Ok(MidiImport {
        format: file.format,
        tempo_bpm,
        time_signature,
        name,
        tracks,
        end,
        report,
        tempo_mode,
        tempo_points,
        metre_changes: metre_list,
        markers,
    })
}

/// A tempo from microseconds, rounded to a thousandth of a BPM: 140 BPM is 428 571 µs in
/// the file, which is 140.00002 read back, and nobody set that.
fn round_bpm(bpm: f64) -> f64 {
    (bpm * 1000.0).round() / 1000.0
}

#[derive(Default)]
struct Building {
    open: HashMap<u8, VecDeque<(u64, u8)>>,
    /// `(on, off, key, velocity)` in file ticks.
    notes: Vec<(u64, u64, u8, u8)>,
    program: Option<u8>,
}

impl Building {
    fn has_notes(&self) -> bool {
        !self.notes.is_empty() || self.open.values().any(|q| !q.is_empty())
    }
}

/// What a file holds that a Cantus clip does not, counted so the report can say so.
#[derive(Default)]
struct Dropped {
    controllers: usize,
    pitch_bends: usize,
    pressure: usize,
    programs: usize,
    sysex: usize,
    texts: usize,
    keys: usize,
    unterminated: usize,
    zero_length: usize,
}

impl Dropped {
    fn count(&mut self, e: &Event) {
        match e {
            Event::Controller { .. } => self.controllers += 1,
            Event::PitchBend { .. } => self.pitch_bends += 1,
            Event::PolyPressure { .. } | Event::ChannelPressure { .. } => self.pressure += 1,
            Event::Program { .. } => self.programs += 1,
            Event::SysEx(_) => self.sysex += 1,
            Event::Text { .. } => self.texts += 1,
            Event::KeySignature { .. } => self.keys += 1,
            _ => {}
        }
    }

    fn report(&self, r: &mut Report) {
        r.count_dropped("interop.report.controllers", self.controllers);
        r.count_dropped("interop.report.pitch_bends", self.pitch_bends);
        r.count_dropped("interop.report.pressure", self.pressure);
        r.count_dropped("interop.report.programs", self.programs);
        r.count_dropped("interop.report.sysex", self.sysex);
        r.count_dropped("interop.report.texts", self.texts);
        r.count_dropped("interop.report.key_signatures", self.keys);
        if self.unterminated > 0 {
            r.info(
                "interop.report.unterminated",
                vec![("n", self.unterminated.to_string())],
            );
        }
        if self.zero_length > 0 {
            r.info(
                "interop.report.zero_length",
                vec![("n", self.zero_length.to_string())],
            );
        }
    }
}

/// Where an import lands in a project.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Placement {
    /// Where the file's start goes on the arrangement.
    pub at: Ticks,
    /// Where the new tracks go in the track list; `None` is after the last.
    pub index: Option<usize>,
    /// Set the project's tempo and metre from the file. Right for an empty project, wrong
    /// for one with a song in it (the song would change speed), which is why it is asked.
    pub apply_tempo: bool,
    /// The engine's limit on tracks (`cantus_engine::MAX_TRACKS`); tracks past it are not
    /// imported, and the report says how many.
    pub max_tracks: usize,
}

impl Default for Placement {
    fn default() -> Self {
        Placement {
            at: Ticks::ZERO,
            index: None,
            apply_tempo: false,
            max_tracks: 256,
        }
    }
}

/// How many pads a kit can hold (`cantus_dsp::MAX_PADS`, which this crate cannot see).
pub const MAX_KIT_PADS: usize = 32;

/// What [`import_commands`] built.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    /// One `Batch` of these is the import: one undo step.
    pub commands: Vec<Command>,
    pub tracks: Vec<TrackId>,
    /// The first imported clip, to select.
    pub first_clip: Option<(TrackId, ClipId)>,
    pub report: Report,
}

/// The commands that add an import to a project: a track per imported track, each with one
/// clip that spans its notes from the bar before the first to the bar after the last, fed
/// into a free mixer strip — and the tempo and metre, when asked.
///
/// `ids` hands out fresh ids (`CommandBus::alloc_id`, or the project's counter);
/// `name` says a [`TrackLabel`] in the interface's language.
pub fn import_commands(
    project: &Project,
    import: &MidiImport,
    place: &Placement,
    ids: &mut dyn FnMut() -> u64,
    name: &dyn Fn(&TrackLabel) -> String,
) -> Built {
    let mut report = Report::default();
    let mut commands = Vec::new();
    let mut ts = project.time_signature;
    // The file's whole tempo map (ADR 0060): set with the tempo, from where the file goes.
    let keep_map = place.apply_tempo && import.tempo_mode == TempoMode::Map;
    if keep_map {
        let at = place.at.max(Ticks::ZERO);
        if !import.tempo_points.is_empty() {
            let bpm = cantus_core::time::clamp_bpm(import.tempo_bpm);
            let mut points = vec![cantus_core::TempoPoint {
                tick: Ticks::ZERO,
                bpm,
                ramp: false,
            }];
            points.extend(
                import
                    .tempo_points
                    .iter()
                    .skip(1)
                    .map(|p| cantus_core::TempoPoint {
                        tick: p.tick + at,
                        ..*p
                    }),
            );
            commands.push(Command::SetTempoTrack { bpm, points });
        }
        if !import.metre_changes.is_empty() {
            let signature = import.metre_changes[0].signature;
            let mut metres = vec![cantus_core::MetreChange {
                tick: Ticks::ZERO,
                signature,
            }];
            metres.extend(
                import
                    .metre_changes
                    .iter()
                    .skip(1)
                    .map(|m| cantus_core::MetreChange {
                        tick: m.tick + at,
                        ..*m
                    }),
            );
            commands.push(Command::SetMetreTrack { signature, metres });
        }
        for (tick, name) in &import.markers {
            commands.push(Command::AddMarker {
                marker: cantus_core::Marker {
                    id: cantus_core::MarkerId(ids()),
                    tick: *tick + at,
                    name: name.clone(),
                    color: None,
                },
            });
        }
    }
    if place.apply_tempo {
        if (import.tempo_bpm - project.tempo_bpm).abs() > 1e-9 {
            commands.push(Command::SetTempo {
                bpm: cantus_core::time::clamp_bpm(import.tempo_bpm),
            });
        }
        if let Some(sig) = import.time_signature {
            ts = sig;
            if sig != project.time_signature {
                commands.push(Command::SetTimeSignature {
                    time_signature: sig,
                });
            }
        }
        report.info(
            "interop.report.tempo_set",
            vec![
                (
                    "bpm",
                    format!("{}", cantus_core::time::clamp_bpm(import.tempo_bpm)),
                ),
                ("sig", ts.to_string()),
            ],
        );
    } else if (import.tempo_bpm - project.tempo_bpm).abs() > 1e-6 {
        report.info(
            "interop.report.tempo_kept",
            vec![
                ("file", format!("{}", import.tempo_bpm)),
                ("bpm", format!("{}", project.tempo_bpm)),
            ],
        );
    }

    let room = place.max_tracks.saturating_sub(project.tracks.len());
    if import.tracks.len() > room {
        report.dropped(
            "interop.report.too_many_tracks",
            vec![
                ("n", (import.tracks.len() - room).to_string()),
                ("max", place.max_tracks.to_string()),
            ],
        );
    }
    let bar = ts.ticks_per_bar().max(Ticks(1));
    let at = place.at.max(Ticks::ZERO);
    let base = place
        .index
        .unwrap_or(project.tracks.len())
        .min(project.tracks.len());
    let mut strips = FreeStrips::new(project);
    let mut built_tracks = Vec::new();
    let mut first_clip = None;
    for (i, it) in import.tracks.iter().take(room).enumerate() {
        if it.notes.is_empty() {
            continue;
        }
        let first = it
            .notes
            .iter()
            .map(|n| n.start)
            .min()
            .unwrap_or(Ticks::ZERO);
        let last = it
            .notes
            .iter()
            .map(|n| n.start + n.length)
            .max()
            .unwrap_or(bar);
        let clip_from = first.snap_floor(bar).max(Ticks::ZERO);
        let clip_to = Ticks((last.0 + bar.0 - 1).div_euclid(bar.0) * bar.0).max(clip_from + bar);
        let track_name = name(&it.label);
        let notes: Vec<Note> = it
            .notes
            .iter()
            .map(|n| Note {
                id: NoteId(ids()),
                start: n.start - clip_from,
                length: n.length,
                pitch: n.pitch,
                velocity: n.velocity,
                muted: false,
            })
            .collect();
        let clip = Clip {
            id: ClipId(ids()),
            name: track_name.clone(),
            start: at + clip_from,
            length: clip_to - clip_from,
            offset: Ticks::ZERO,
            content_length: None,
            muted: false,
            color: None,
            notes,
            audio: None,
            lane: 0,
            automation: None,
            link: None,
            comp: false,
        };
        let instrument = if it.drums {
            InstrumentSpec::Pulsus(kit_for(it, &mut report))
        } else {
            InstrumentSpec::default()
        };
        let id = TrackId(ids());
        first_clip.get_or_insert((id, clip.id));
        let track = Track {
            id,
            name: track_name,
            color: Color::PALETTE[(project.tracks.len() + i) % Color::PALETTE.len()],
            height: 1.0,
            icon: None,
            instrument,
            clips: vec![clip],
            gain_db: 0.0,
            pan: 0.0,
            strip: strips.take(),
            muted: false,
            soloed: false,
            input: None,
            armed: false,
            takes_open: false,
            automation_open: true,
            pitch_open: false,
            modulators: Vec::new(),
            comps: Vec::new(),
            active_comp: 0,
            frozen: None,
        };
        commands.push(Command::InsertTrack {
            index: base + built_tracks.len(),
            track: Box::new(track),
        });
        built_tracks.push(id);
    }
    Built {
        commands,
        tracks: built_tracks,
        first_clip,
        report,
    }
}

/// The shipped kit, and a pad for every GM key the part plays that the kit lacks.
fn kit_for(track: &ImportedTrack, report: &mut Report) -> PulsusParams {
    kit_for_keys(track.notes.iter().map(|n| n.pitch), report)
}

/// The shipped kit, and a pad for each of `keys` it lacks, named after General MIDI's
/// drum map, up to what a kit holds; the report says how many did not fit.
pub(crate) fn kit_for_keys(keys: impl Iterator<Item = Pitch>, report: &mut Report) -> PulsusParams {
    let mut kit = PulsusParams::default();
    let mut keys: Vec<Pitch> = keys.collect();
    keys.sort_unstable();
    keys.dedup();
    let mut full = 0;
    for key in keys {
        if kit.index_of_pitch(key).is_some() {
            continue;
        }
        if kit.pads.len() >= MAX_KIT_PADS {
            full += 1;
            continue;
        }
        kit.pads.push(gm::pad_for(key));
    }
    report.count_dropped("interop.report.pads_full", full);
    PulsusParams::of(kit.pads)
}

/// Mixer strips no track feeds, handed out the way `Project::first_free_strip` does:
/// the middle of the desk first, then the docks, then the master.
struct FreeStrips {
    free: Vec<StripId>,
}

impl FreeStrips {
    fn new(project: &Project) -> Self {
        let used = |id: StripId| project.tracks.iter().any(|t| t.strip == Some(id));
        let middle = project
            .strips
            .iter()
            .filter(|s| s.dock == Dock::Middle && !used(s.id));
        let docked = project
            .strips
            .iter()
            .filter(|s| s.dock != Dock::Middle && !used(s.id));
        let mut free: Vec<StripId> = middle.chain(docked).map(|s| s.id).collect();
        free.reverse();
        FreeStrips { free }
    }

    fn take(&mut self) -> Option<StripId> {
        self.free.pop()
    }
}

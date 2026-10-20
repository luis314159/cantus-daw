//! A Cantus project as DAWproject XML.
//!
//! **The mixer.** A Cantus track is FL's channel: an instrument with a level and a balance
//! of its own, feeding a mixer strip. DAWproject's track *is* a channel. So:
//!
//! - A strip fed by one track and nothing else, where that track's own level, balance,
//!   mute and solo are at rest — the common case — is written *as* that track's channel.
//! - Otherwise the track's channel carries the track's own settings and names the strip's
//!   channel as its destination, and the strip is a track of its own: `submix` when tracks
//!   or strips feed it, `effect` when only sends do.
//! - A strip nothing feeds is not written: a desk opens with sixteen, and another
//!   application would show every empty one as a track.
//! - The master is the `master` channel, last.
//!
//! **Devices.** An instrument or an insert is a `<Device>` with Cantus's name for it, an
//! id of `cantus.<kind>`, its parameters listed for anyone reading, and its settings as a
//! JSON state file in the archive, from which Cantus restores it exactly. When the
//! effects ship as CLAP plug-ins, the same slot becomes a `<ClapPlugin>` with the plug-in's
//! own state, and every CLAP host can load it (ADR 0021).

use super::xml::{a, num, XmlWriter};
use super::{
    effect_device_id, effect_device_name, Parts, APPLICATION, DEVICE_PREFIX, FORMAT_VERSION,
    TRACK_CHANNEL,
};
use crate::report::Report;
use cantus_core::{
    db_to_linear, Clip, Color, EffectSlot, InstrumentSpec, MixerStrip, Project, StripId, Ticks,
    Track, MAX_GAIN_DB, PPQ,
};
use std::collections::HashMap;

/// A tick as beats, the timeline unit written.
pub fn beats(t: Ticks) -> String {
    num(t.0 as f64 / PPQ as f64)
}

fn color(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Cantus's −1…1 as DAWproject's normalized 0…1, 0.5 the centre.
fn pan(p: f32) -> String {
    num(((p.clamp(-1.0, 1.0) as f64) + 1.0) / 2.0)
}

fn volume_linear(db: f32) -> String {
    num(db_to_linear(db) as f64)
}

struct Ids(usize);

impl Ids {
    fn next(&mut self) -> String {
        self.0 += 1;
        format!("id{}", self.0 - 1)
    }
}

/// What each strip becomes.
#[derive(Clone, Debug)]
struct StripPlan {
    channel_id: String,
    /// Written as this track's channel (the track's index).
    merged_into: Option<usize>,
    /// Anything feeds it.
    used: bool,
    /// Only sends feed it.
    return_only: bool,
}

/// The XML, the metadata and the device states of a project.
pub fn parts(project: &Project, app_version: &str) -> Parts {
    parts_with(project, app_version, None)
}

/// [`parts`], the audio clips' files read from `base` (the project's folder: a stored path
/// is relative to it) and carried in the archive (round 29, ADR 0061).
pub fn parts_with(project: &Project, app_version: &str, base: Option<&std::path::Path>) -> Parts {
    let mut report = Report::default();
    let mut ids = Ids(0);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    // The audio files the archive carries: (path in the archive, file on disk), each once.
    let mut audio: Vec<(String, std::path::PathBuf)> = Vec::new();

    // Plan the mixer first: which strips are written, and which fold into their track.
    let mut plans: HashMap<StripId, StripPlan> = HashMap::new();
    for s in &project.strips {
        let tracks: Vec<usize> = project
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.strip == Some(s.id))
            .map(|(i, _)| i)
            .collect();
        let from_outputs = project.strips.iter().any(|o| o.output == Some(s.id));
        let from_sends = project.senders_to(s.id).next().is_some();
        let merged_into = match tracks.as_slice() {
            [only] if !from_outputs && !from_sends && track_at_rest(&project.tracks[*only]) => {
                Some(*only)
            }
            _ => None,
        };
        plans.insert(
            s.id,
            StripPlan {
                channel_id: ids.next(),
                merged_into,
                used: !tracks.is_empty() || from_outputs || from_sends,
                return_only: tracks.is_empty() && !from_outputs && from_sends,
            },
        );
    }
    let master_id = ids.next();
    let empty = plans.values().filter(|p| !p.used).count();
    if empty > 0 {
        report.info(
            "interop.report.x_empty_strips",
            vec![("n", empty.to_string())],
        );
    }
    let dest = |to: Option<StripId>| -> String {
        to.and_then(|id| plans.get(&id))
            .filter(|p| p.used)
            .map(|p| p.channel_id.clone())
            .unwrap_or_else(|| master_id.clone())
    };

    let mut w = XmlWriter::new();
    w.open("Project", &[("version", a(FORMAT_VERSION))]);
    w.empty(
        "Application",
        &[("name", a(APPLICATION)), ("version", a(app_version))],
    );
    w.open("Transport", &[]);
    let tempo_id = ids.next();
    let metre_id = ids.next();
    w.empty(
        "Tempo",
        &[
            ("max", a(num(cantus_core::time::MAX_BPM))),
            ("min", a(num(cantus_core::time::MIN_BPM))),
            ("unit", a("bpm")),
            ("value", a(num(project.tempo_bpm))),
            ("id", a(tempo_id.clone())),
            ("name", a("Tempo")),
        ],
    );
    w.empty(
        "TimeSignature",
        &[
            ("denominator", a(project.time_signature.denominator)),
            ("numerator", a(project.time_signature.numerator)),
            ("id", a(metre_id.clone())),
        ],
    );
    w.close();

    w.open("Structure", &[]);
    let mut track_ids = Vec::with_capacity(project.tracks.len());
    let mut devices = 0usize;
    for (ti, t) in project.tracks.iter().enumerate() {
        let track_id = ids.next();
        track_ids.push(track_id.clone());
        let merged = t
            .strip
            .and_then(|s| plans.get(&s).map(|p| (s, p)))
            .filter(|(_, p)| p.merged_into == Some(ti));
        // What the track holds (round 29's review): audio clips, notes, or both — a track
        // of audio only is no instrument's.
        let heard = || {
            t.clips
                .iter()
                .filter(|c| c.lane == 0 && c.automation.is_none())
        };
        let has_audio = heard().any(|c| c.audio.is_some());
        let has_notes = heard().any(|c| c.audio.is_none());
        let content = match (has_audio, has_notes) {
            (true, false) => "audio",
            (true, true) => "audio notes",
            _ => "notes",
        };
        if t.input.is_some() {
            report.info(
                "interop.report.x_track_input",
                vec![("track", t.name.clone())],
            );
        }
        w.open(
            "Track",
            &[
                ("contentType", a(content)),
                ("loaded", a("true")),
                ("id", a(&track_id)),
                ("name", a(&t.name)),
                ("color", a(color(t.color))),
            ],
        );
        match merged {
            Some((sid, plan)) => {
                let strip = project.strip(sid).expect("planned from the project");
                let inst = (content != "audio").then(|| instrument_device(&mut ids, &mut files, t));
                devices += strip.effects.len();
                channel(
                    &mut w,
                    &mut ids,
                    &mut files,
                    &mut report,
                    ChannelSpec {
                        id: &plan.channel_id,
                        role: "regular",
                        name: strip_name(strip),
                        comment: None,
                        color: strip.color,
                        destination: dest(strip.output),
                        volume_db: strip.gain_db,
                        pan: strip.pan,
                        mute: strip.muted,
                        solo: strip.soloed,
                        instrument: inst,
                        strip: Some(strip),
                    },
                    &plans,
                );
            }
            None => {
                let inst = (content != "audio").then(|| instrument_device(&mut ids, &mut files, t));
                let own = ids.next();
                channel(
                    &mut w,
                    &mut ids,
                    &mut files,
                    &mut report,
                    ChannelSpec {
                        id: &own,
                        role: "regular",
                        name: None,
                        comment: Some(TRACK_CHANNEL),
                        color: None,
                        destination: dest(t.strip),
                        volume_db: t.gain_db,
                        pan: t.pan,
                        mute: t.muted,
                        solo: t.soloed,
                        instrument: inst,
                        strip: None,
                    },
                    &plans,
                );
            }
        }
        w.close();
    }
    // The strips that are channels of their own, in desk order.
    for (si, s) in project.strips.iter().enumerate() {
        let plan = &plans[&s.id];
        if !plan.used || plan.merged_into.is_some() {
            continue;
        }
        devices += s.effects.len();
        let display = strip_name(s).unwrap_or_else(|| default_strip_name(project, si));
        w.open(
            "Track",
            &[
                ("contentType", a("audio")),
                ("loaded", a("true")),
                ("id", a(ids.next())),
                ("name", a(&display)),
                ("color", s.color.map(color)),
            ],
        );
        channel(
            &mut w,
            &mut ids,
            &mut files,
            &mut report,
            ChannelSpec {
                id: &plan.channel_id,
                role: if plan.return_only { "effect" } else { "submix" },
                name: strip_name(s),
                comment: None,
                color: s.color,
                destination: dest(s.output),
                volume_db: s.gain_db,
                pan: s.pan,
                mute: s.muted,
                solo: s.soloed,
                instrument: None,
                strip: Some(s),
            },
            &plans,
        );
        w.close();
    }
    // The master.
    w.open(
        "Track",
        &[
            ("contentType", a("audio notes")),
            ("loaded", a("true")),
            ("id", a(ids.next())),
            ("name", a("Master")),
        ],
    );
    w.open(
        "Channel",
        &[
            ("audioChannels", a(2)),
            ("role", a("master")),
            ("solo", a("false")),
            ("id", a(&master_id)),
        ],
    );
    w.empty(
        "Mute",
        &[
            ("value", a("false")),
            ("id", a(ids.next())),
            ("name", a("Mute")),
        ],
    );
    pan_param(&mut w, &mut ids, project.master.pan);
    volume_param(&mut w, &mut ids, project.master.gain_db);
    w.close();
    w.close();
    if project.master.width != 1.0 {
        report.dropped("interop.report.x_master_width", vec![]);
    }
    if project.master.phase_invert {
        report.dropped("interop.report.x_master_polarity", vec![]);
    }
    w.close(); // Structure

    // The timeline.
    w.open("Arrangement", &[("id", a(ids.next()))]);
    w.open("Lanes", &[("timeUnit", a("beats")), ("id", a(ids.next()))]);
    let mut muted_notes = 0usize;
    for (t, track_id) in project.tracks.iter().zip(&track_ids) {
        w.open("Lanes", &[("track", a(track_id)), ("id", a(ids.next()))]);
        w.open("Clips", &[("id", a(ids.next()))]);
        // The playing lane only: a take lane is never heard (ADR 0058), and DAWproject's
        // one lane per track would play every take at once (round 29's review).
        for c in t.clips.iter().filter(|c| c.lane == 0) {
            muted_notes += clip(&mut w, &mut ids, c, project, base, &mut audio, &mut report);
        }
        let takes = t.clips.iter().filter(|c| c.lane != 0).count();
        if takes > 0 {
            report.dropped(
                "interop.report.x_takes",
                vec![("track", t.name.clone()), ("n", takes.to_string())],
            );
        }
        w.close();
        w.close();
    }
    w.close();
    tempo_map(&mut w, &mut ids, project, &tempo_id, &metre_id);
    w.close();
    w.open("Scenes", &[]);
    w.close();
    w.close(); // Project
    report.count_dropped("interop.report.x_muted_notes", muted_notes);
    if devices > 0 {
        report.info("interop.report.x_devices", vec![("n", devices.to_string())]);
    }
    report.info("interop.report.x_pan", vec![]);

    let mut m = XmlWriter::new();
    m.open("MetaData", &[]);
    m.text("Title", &project.name);
    m.text("Comment", &format!("{APPLICATION} {app_version}"));
    m.close();

    Parts {
        project_xml: w.finish(),
        metadata_xml: m.finish(),
        files,
        audio,
        report,
    }
}

/// A track whose own controls do nothing: it can share its strip's channel.
fn track_at_rest(t: &Track) -> bool {
    t.gain_db == 0.0 && t.pan == 0.0 && !t.muted && !t.soloed
}

fn strip_name(s: &MixerStrip) -> Option<String> {
    (!s.name.trim().is_empty()).then(|| s.name.clone())
}

/// What the mixer calls an unnamed strip: the first track feeding it, or its number.
fn default_strip_name(project: &Project, index: usize) -> String {
    let id = project.strips[index].id;
    project
        .tracks_on_strip(id)
        .next()
        .map(|t| t.name.clone())
        .unwrap_or_else(|| format!("Strip {}", index + 1))
}

struct ChannelSpec<'a> {
    id: &'a str,
    role: &'static str,
    name: Option<String>,
    /// [`TRACK_CHANNEL`] on a track's own channel, which feeds its strip's.
    comment: Option<&'static str>,
    color: Option<Color>,
    destination: String,
    volume_db: f32,
    pan: f32,
    mute: bool,
    solo: bool,
    /// The instrument's device, already written to a string of attributes.
    instrument: Option<Device>,
    strip: Option<&'a MixerStrip>,
}

/// A device, planned: its element attributes and its state file.
struct Device {
    role: &'static str,
    name: String,
    id: String,
    device_id: String,
    enabled: bool,
    state: String,
    params: Vec<(String, f32, &'static str)>,
}

fn pan_param(w: &mut XmlWriter, ids: &mut Ids, p: f32) {
    w.empty(
        "Pan",
        &[
            ("max", a("1.0")),
            ("min", a("0.0")),
            ("unit", a("normalized")),
            ("value", a(pan(p))),
            ("id", a(ids.next())),
            ("name", a("Pan")),
        ],
    );
}

fn volume_param(w: &mut XmlWriter, ids: &mut Ids, db: f32) {
    w.empty(
        "Volume",
        &[
            ("max", a(num(db_to_linear(MAX_GAIN_DB) as f64))),
            ("min", a("0.0")),
            ("unit", a("linear")),
            ("value", a(volume_linear(db))),
            ("id", a(ids.next())),
            ("name", a("Volume")),
        ],
    );
}

#[allow(clippy::too_many_arguments)]
fn channel(
    w: &mut XmlWriter,
    ids: &mut Ids,
    files: &mut Vec<(String, Vec<u8>)>,
    report: &mut Report,
    c: ChannelSpec,
    plans: &HashMap<StripId, StripPlan>,
) {
    w.open(
        "Channel",
        &[
            ("audioChannels", a(2)),
            ("destination", a(&c.destination)),
            ("role", a(c.role)),
            ("solo", a(c.solo)),
            ("id", a(c.id)),
            ("name", c.name.clone()),
            ("color", c.color.map(color)),
            ("comment", c.comment.map(str::to_string)),
        ],
    );
    let mut list: Vec<Device> = c.instrument.into_iter().collect();
    if let Some(s) = c.strip {
        let label = c.name.clone().unwrap_or_else(|| c.id.to_string());
        for slot in &s.effects {
            list.push(effect_device(ids, files, slot, plans));
        }
        strip_extras(report, s, &label);
    }
    let eq = c.strip.map(|s| s.eq).filter(|e| !e.is_flat());
    if !list.is_empty() || eq.is_some() {
        w.open("Devices", &[]);
        for d in &list {
            write_device(w, ids, d);
        }
        if let Some(eq) = eq {
            channel_eq(w, ids, &eq);
        }
        w.close();
    }
    w.empty(
        "Mute",
        &[
            ("value", a(c.mute)),
            ("id", a(ids.next())),
            ("name", a("Mute")),
        ],
    );
    pan_param(w, ids, c.pan);
    if let Some(s) = c.strip.filter(|s| !s.sends.is_empty()) {
        w.open("Sends", &[]);
        for send in &s.sends {
            let Some(to) = plans.get(&send.to) else {
                continue;
            };
            w.open(
                "Send",
                &[
                    ("destination", a(&to.channel_id)),
                    ("type", a(send_type(send.tap))),
                    ("id", a(ids.next())),
                    ("name", a("Send")),
                ],
            );
            w.empty(
                "Enable",
                &[
                    ("value", a(send.enabled)),
                    ("id", a(ids.next())),
                    ("name", a("Enable")),
                ],
            );
            w.empty(
                "Volume",
                &[
                    ("max", a("1.0")),
                    ("min", a("0.0")),
                    ("unit", a("linear")),
                    ("value", a(num(send.level as f64))),
                    ("id", a(ids.next())),
                    ("name", a("Send level")),
                ],
            );
            w.close();
        }
        w.close();
    }
    volume_param(w, ids, c.volume_db);
    w.close();
}

/// DAWproject's name for a send's tap.
fn send_type(tap: cantus_core::SendTap) -> &'static str {
    match tap {
        cantus_core::SendTap::Post => "post",
        cantus_core::SendTap::Pre => "pre",
    }
}

/// The strip controls DAWproject has no place for.
fn strip_extras(report: &mut Report, s: &MixerStrip, label: &str) {
    let name = || vec![("name", label.to_string())];
    if s.width != 1.0 {
        report.dropped("interop.report.x_width", name());
    }
    if s.phase_invert {
        report.dropped("interop.report.x_polarity", name());
    }
    if s.swap_channels {
        report.dropped("interop.report.x_swap", name());
    }
    if !s.fx_enabled {
        report.dropped("interop.report.x_fx_off", name());
    }
    if s.effects.iter().any(|e| e.mix != 1.0) {
        report.dropped("interop.report.x_slot_mix", name());
    }
    if s.effects.iter().any(|e| e.sidechain.is_some()) {
        report.info("interop.report.x_sidechain", name());
    }
}

fn write_device(w: &mut XmlWriter, ids: &mut Ids, d: &Device) {
    w.open(
        "Device",
        &[
            ("deviceID", a(&d.device_id)),
            ("deviceName", a(&d.name)),
            ("deviceRole", a(d.role)),
            ("deviceVendor", a(APPLICATION)),
            ("id", a(&d.id)),
            ("name", a(&d.name)),
        ],
    );
    if !d.params.is_empty() {
        w.open("Parameters", &[]);
        for (i, (name, value, unit)) in d.params.iter().enumerate() {
            w.empty(
                "RealParameter",
                &[
                    ("unit", a(*unit)),
                    ("value", a(num(*value as f64))),
                    ("parameterID", a(i)),
                    ("id", a(ids.next())),
                    ("name", a(name)),
                ],
            );
        }
        w.close();
    }
    w.empty(
        "Enabled",
        &[
            ("value", a(d.enabled)),
            ("id", a(ids.next())),
            ("name", a("On/Off")),
        ],
    );
    w.empty("State", &[("path", a(&d.state))]);
    w.close();
}

/// A DAWproject unit for one of our parameter units.
fn unit_of(u: &str) -> &'static str {
    match u {
        "Hz" | "kHz" => "hertz",
        "s" | "ms" => "seconds",
        "dB" => "decibel",
        "st" => "semitones",
        "%" => "percent",
        _ => "linear",
    }
}

fn instrument_device(ids: &mut Ids, files: &mut Vec<(String, Vec<u8>)>, t: &Track) -> Device {
    let (kind, params) = match &t.instrument {
        InstrumentSpec::Firmus(p) => (
            "firmus",
            cantus_core::FirmusParam::ALL
                .iter()
                .map(|f| {
                    let info = f.info();
                    (info.label_key.to_string(), f.get(p), unit_of(info.unit))
                })
                .collect(),
        ),
        InstrumentSpec::Pulsus(_) => ("pulsus", Vec::new()),
        #[allow(unreachable_patterns)]
        _ => ("instrument", Vec::new()),
    };
    let id = ids.next();
    let state = format!("devices/{kind}-{id}.json");
    // A device state has no schema version of its own: a recalibrated instrument's trim
    // is written out, so a later calibration reads it at this level (ADR 0039).
    let spec = match &t.instrument {
        InstrumentSpec::Native(n) => InstrumentSpec::Native(n.spell_out_trim()),
        other => other.clone(),
    };
    files.push((
        state.clone(),
        serde_json::to_vec_pretty(&spec).unwrap_or_default(),
    ));
    let mut name = kind.to_string();
    name[..1].make_ascii_uppercase();
    Device {
        role: "instrument",
        name,
        id,
        device_id: format!("{DEVICE_PREFIX}{kind}"),
        enabled: true,
        state,
        params,
    }
}

/// What an insert's state file holds: its settings, its slot's mix, and the channel its
/// detector listens to (by the XML id, since strip ids do not survive the trip).
#[derive(serde::Serialize, serde::Deserialize)]
pub struct EffectState {
    pub spec: cantus_core::EffectSpec,
    pub mix: f32,
    pub sidechain: Option<String>,
}

fn effect_device(
    ids: &mut Ids,
    files: &mut Vec<(String, Vec<u8>)>,
    slot: &EffectSlot,
    plans: &HashMap<StripId, StripPlan>,
) -> Device {
    let kind = slot.spec.kind;
    let id = ids.next();
    let device_id = effect_device_id(kind);
    let state = format!("devices/{}-{id}.json", &device_id[DEVICE_PREFIX.len()..]);
    let st = EffectState {
        spec: slot.spec.clone(),
        mix: slot.mix,
        sidechain: slot
            .sidechain
            .and_then(|s| plans.get(&s))
            .map(|p| p.channel_id.clone()),
    };
    files.push((
        state.clone(),
        serde_json::to_vec_pretty(&st).unwrap_or_default(),
    ));
    let params = kind
        .params()
        .iter()
        .filter(|p| !kind.sparse() || slot.spec.params.contains_key(p.key))
        .map(|p| (p.key.to_string(), slot.spec.get(p.key), unit_of(p.unit)))
        .collect();
    Device {
        role: "audioFX",
        name: effect_device_name(kind),
        id,
        device_id,
        enabled: !slot.bypass,
        state,
        params,
    }
}

/// The strip's own three-band EQ as DAWproject's built-in equaliser, which the other
/// applications map to theirs: a low shelf, a bell and a high shelf.
fn channel_eq(w: &mut XmlWriter, ids: &mut Ids, eq: &cantus_core::StripEq) {
    w.open(
        "Equalizer",
        &[
            ("deviceName", a("Channel EQ")),
            ("deviceRole", a("audioFX")),
            ("deviceVendor", a(APPLICATION)),
            ("id", a(ids.next())),
            ("name", a("Channel EQ")),
        ],
    );
    w.empty("Enabled", &[("value", a(true)), ("id", a(ids.next()))]);
    for (i, (band, kind)) in eq
        .bands
        .iter()
        .zip(["lowShelf", "bell", "highShelf"])
        .enumerate()
    {
        w.open("Band", &[("type", a(kind)), ("order", a(i))]);
        w.empty(
            "Freq",
            &[
                ("unit", a("hertz")),
                ("value", a(num(band.freq_hz as f64))),
                ("id", a(ids.next())),
            ],
        );
        w.empty(
            "Gain",
            &[
                ("unit", a("decibel")),
                ("value", a(num(band.gain_db as f64))),
                ("id", a(ids.next())),
            ],
        );
        w.empty(
            "Q",
            &[
                ("unit", a("linear")),
                ("value", a(num(band.q as f64))),
                ("id", a(ids.next())),
            ],
        );
        w.close();
    }
    w.close();
}

/// One clip with its notes. Returns how many muted notes were left out (DAWproject's
/// notes cannot be muted).
#[allow(clippy::too_many_arguments)]
fn clip(
    w: &mut XmlWriter,
    ids: &mut Ids,
    c: &Clip,
    project: &Project,
    base: Option<&std::path::Path>,
    audio: &mut Vec<(String, std::path::PathBuf)>,
    report: &mut Report,
) -> usize {
    let period = c.period();
    let fades = c
        .audio
        .as_ref()
        .filter(|a| a.fade_in > Ticks::ZERO || a.fade_out > Ticks::ZERO);
    // A clip at its natural speed counts its content in seconds of its file (round 29's
    // review): where it starts in the file, and where its loop ends.
    let natural = c
        .audio
        .as_ref()
        .filter(|a| super::audio::natural(a))
        .and_then(|a| Some((a, project.audio_source(a.source)?.sample_rate)))
        .map(|(a, rate)| super::audio::seconds_of(project, c, a, rate));
    let (play_start, loop_start, loop_end) = match natural {
        Some((s, end)) => (num(s), end.map(|_| num(s)), end.map(num)),
        None => (
            beats(c.offset),
            period.map(|_| beats(c.offset)),
            period.map(|p| beats(c.offset + p)),
        ),
    };
    w.open(
        "Clip",
        &[
            ("time", a(beats(c.start))),
            ("duration", a(beats(c.length))),
            ("contentTimeUnit", natural.map(|_| "seconds".to_string())),
            ("playStart", a(play_start)),
            ("loopStart", loop_start),
            ("loopEnd", loop_end),
            ("fadeTimeUnit", fades.map(|_| "beats".to_string())),
            ("fadeInTime", fades.map(|a| beats(a.fade_in))),
            ("fadeOutTime", fades.map(|a| beats(a.fade_out))),
            ("enable", c.muted.then(|| "false".to_string())),
            ("name", a(&c.name)),
            ("color", c.color.map(color)),
        ],
    );
    // An audio clip (round 29, ADR 0061): its file and its warp.
    if let Some(au) = &c.audio {
        super::audio::write(w, &mut || ids.next(), project, c, au, base, audio, report);
        w.close();
        return 0;
    }
    w.open("Notes", &[("id", a(ids.next()))]);
    let mut muted = 0;
    let mut notes: Vec<_> = c.notes.iter().collect();
    notes.sort_by_key(|n| (n.start, n.pitch));
    for n in notes {
        if n.muted {
            muted += 1;
            continue;
        }
        w.empty(
            "Note",
            &[
                ("time", a(beats(n.start))),
                ("duration", a(beats(n.length))),
                ("channel", a(0)),
                ("key", a(n.pitch)),
                ("vel", a(num(n.velocity as f64 / 127.0))),
            ],
        );
    }
    w.close();
    w.close();
    muted
}

/// The arrangement's markers, tempo automation and time-signature automation (round 29,
/// ADR 0060), in beats: every marker with its name and colour; every tempo point, a ramp
/// as a linear point (the tempo moves linearly in beats to the next, as DAWproject's
/// `linear` interpolation draws it), a held one as `hold`; every metre change.
fn tempo_map(w: &mut XmlWriter, ids: &mut Ids, project: &Project, tempo_id: &str, metre_id: &str) {
    let beats = |t: cantus_core::Ticks| num(t.as_beats_f64());
    if !project.markers.is_empty() {
        w.open(
            "Markers",
            &[("timeUnit", a("beats")), ("id", a(ids.next()))],
        );
        for m in &project.markers {
            let mut at = vec![("time", a(beats(m.tick))), ("name", a(m.name.clone()))];
            if let Some(c) = m.color {
                at.push(("color", a(color(c))));
            }
            w.empty("Marker", &at);
        }
        w.close();
    }
    if !project.tempo_points.is_empty() {
        w.open(
            "TempoAutomation",
            &[
                ("unit", a("bpm")),
                ("timeUnit", a("beats")),
                ("id", a(ids.next())),
            ],
        );
        w.empty("Target", &[("parameter", a(tempo_id))]);
        for p in &project.tempo_points {
            w.empty(
                "RealPoint",
                &[
                    ("time", a(beats(p.tick))),
                    ("value", a(num(p.bpm))),
                    ("interpolation", a(if p.ramp { "linear" } else { "hold" })),
                ],
            );
        }
        w.close();
    }
    if !project.metre_changes.is_empty() {
        w.open(
            "TimeSignatureAutomation",
            &[("timeUnit", a("beats")), ("id", a(ids.next()))],
        );
        w.empty("Target", &[("parameter", a(metre_id))]);
        for m in &project.metre_changes {
            w.empty(
                "TimeSignaturePoint",
                &[
                    ("time", a(beats(m.tick))),
                    ("numerator", a(m.signature.numerator)),
                    ("denominator", a(m.signature.denominator)),
                ],
            );
        }
        w.close();
    }
}

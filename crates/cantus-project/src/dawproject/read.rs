//! Any application's `.dawproject` as a new Cantus project.
//!
//! **The mixer.** Every channel becomes a mixer strip — its volume, pan, mute, solo, sends,
//! devices and destination — except the master's, which becomes the master, and a channel
//! marked as a Cantus track's own ([`super::TRACK_CHANNEL`]), which becomes that track's
//! level and balance. Every track with notes becomes a Cantus track feeding the strip made
//! from its channel. Group tracks are buses: their channel is a strip their children feed.
//!
//! **Devices.** A Cantus device (`cantus.<kind>` with its state file) comes back exactly.
//! The built-in equaliser becomes the strip's own EQ when it fits in three bands. Anything
//! else — a plug-in, another application's device — is named in the report, and a track
//! whose instrument could not come plays Firmus (or Pulsus, when its notes are all on
//! channel 10).
//!
//! What is in the file and not in the project is listed in the report, never dropped in
//! silence.

use super::read_timeline::{self, Timeline};
use super::write::EffectState;
use super::{effect_kind_of, DawProjectError, Imported, APPLICATION, DEVICE_PREFIX, TRACK_CHANNEL};
use crate::report::Report;
use cantus_core::{
    linear_to_db, Color, Dock, EffectId, EffectSlot, InstrumentSpec, Project, Send, SendTap,
    StripId, Ticks, TimeSignature, DEFAULT_STRIPS, MAX_GAIN_DB, MAX_INSERTS, MAX_SENDS,
    MIN_GAIN_DB,
};
use roxmltree::Node;
use std::collections::HashMap;

/// The engine's limits (`cantus_engine::MAX_TRACKS`, `MAX_STRIPS`), which this crate cannot
/// see. Past them the report says what was left out.
pub const MAX_TRACKS: usize = 256;
pub const MAX_STRIPS: usize = 256;

pub(crate) fn attr<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    n.attribute(name)
}

pub(crate) fn attr_f64(n: Node, name: &str) -> Option<f64> {
    n.attribute(name)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn attr_bool(n: Node, name: &str) -> Option<bool> {
    match n.attribute(name)?.trim() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

pub(crate) fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

pub(crate) fn parse_color(s: Option<&str>) -> Option<Color> {
    let h = s?.trim().strip_prefix('#')?;
    if h.len() < 6 {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some(Color::rgb(v(0)?, v(2)?, v(4)?))
}

/// A `RealParameter` as a value between its own min and max, normalised to 0…1.
fn normalized(n: Node) -> Option<f64> {
    let v = attr_f64(n, "value")?;
    let (min, max) = (
        attr_f64(n, "min").unwrap_or(0.0),
        attr_f64(n, "max").unwrap_or(1.0),
    );
    if max > min && attr(n, "unit") != Some("normalized") {
        Some(((v - min) / (max - min)).clamp(0.0, 1.0))
    } else {
        Some(v.clamp(0.0, 1.0))
    }
}

/// A volume parameter in decibels, whatever unit it was written in.
fn volume_db(n: Node) -> Option<f32> {
    let v = attr_f64(n, "value")?;
    let db = match attr(n, "unit") {
        Some("decibel") => v as f32,
        Some("normalized") | Some("percent") => {
            let scale = if attr(n, "unit") == Some("percent") {
                100.0
            } else {
                1.0
            };
            linear_to_db((v / scale) as f32)
        }
        _ => linear_to_db(v as f32),
    };
    Some(db.clamp(MIN_GAIN_DB, MAX_GAIN_DB))
}

/// One device on a channel, as far as import cares.
#[derive(Clone, Debug)]
struct DeviceInfo<'a, 'i> {
    node: Node<'a, 'i>,
    element: String,
    device_id: String,
    name: String,
    role: String,
    enabled: bool,
    state: Option<String>,
}

fn devices<'a, 'i>(channel: Node<'a, 'i>) -> Vec<DeviceInfo<'a, 'i>> {
    let Some(list) = child(channel, "Devices") else {
        return Vec::new();
    };
    list.children()
        .filter(|c| c.is_element())
        .map(|d| DeviceInfo {
            node: d,
            element: d.tag_name().name().to_string(),
            device_id: attr(d, "deviceID").unwrap_or("").to_string(),
            name: attr(d, "deviceName")
                .or(attr(d, "name"))
                .unwrap_or("?")
                .to_string(),
            role: attr(d, "deviceRole").unwrap_or("audioFX").to_string(),
            enabled: child(d, "Enabled")
                .and_then(|e| attr_bool(e, "value"))
                .unwrap_or(true),
            state: child(d, "State")
                .and_then(|s| attr(s, "path"))
                .map(str::to_string),
        })
        .collect()
}

/// A channel, read.
#[derive(Clone, Debug)]
struct ChannelInfo<'a, 'i> {
    node: Node<'a, 'i>,
    id: Option<String>,
    role: String,
    destination: Option<String>,
    /// The owning track's name, for a strip's display and for the report.
    owner: String,
    /// The group this channel's track sits in: its destination when it names none.
    parent: Option<String>,
    track_channel: bool,
}

/// A track, read.
#[derive(Clone, Debug)]
struct TrackInfo {
    id: Option<String>,
    name: String,
    color: Option<Color>,
    content: Vec<String>,
    /// Index into the channels.
    channel: Option<usize>,
}

/// The tracks and channels under `n`: the structure itself, or a group track (whose own
/// channel is already read, so only its child tracks are walked).
fn walk_structure<'a, 'i>(
    n: Node<'a, 'i>,
    parent_channel: Option<String>,
    tracks: &mut Vec<TrackInfo>,
    channels: &mut Vec<ChannelInfo<'a, 'i>>,
) {
    for c in n.children().filter(|c| c.is_element()) {
        match c.tag_name().name() {
            "Track" => {
                let name = attr(c, "name").unwrap_or("").to_string();
                let channel = child(c, "Channel").map(|ch| {
                    channels.push(channel_info(ch, &name, parent_channel.clone()));
                    channels.len() - 1
                });
                tracks.push(TrackInfo {
                    id: attr(c, "id").map(str::to_string),
                    name,
                    color: parse_color(attr(c, "color")),
                    content: attr(c, "contentType")
                        .unwrap_or("")
                        .split_whitespace()
                        .map(str::to_string)
                        .collect(),
                    channel,
                });
                // Nested tracks are a group's: they feed its channel unless they say.
                let group = channel
                    .and_then(|i| channels[i].id.clone())
                    .or(parent_channel.clone());
                walk_structure(c, group, tracks, channels);
            }
            // A channel on its own, at the top of the structure; a track's channel is its.
            "Channel" if n.has_tag_name("Structure") => {
                channels.push(channel_info(c, "", parent_channel.clone()))
            }
            _ => {}
        }
    }
}

fn channel_info<'a, 'i>(
    ch: Node<'a, 'i>,
    owner: &str,
    parent: Option<String>,
) -> ChannelInfo<'a, 'i> {
    ChannelInfo {
        node: ch,
        id: attr(ch, "id").map(str::to_string),
        role: attr(ch, "role").unwrap_or("regular").to_string(),
        destination: attr(ch, "destination").map(str::to_string),
        owner: attr(ch, "name").unwrap_or(owner).to_string(),
        parent,
        track_channel: attr(ch, "comment") == Some(TRACK_CHANNEL),
    }
}

/// Read the archive's files into a project.
pub fn project(files: &[(String, Vec<u8>)]) -> Result<Imported, DawProjectError> {
    let file = |name: &str| {
        files
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, d)| d.as_slice())
    };
    let xml = file("project.xml").ok_or(DawProjectError::NoProject)?;
    let text = String::from_utf8_lossy(xml);
    let doc = roxmltree::Document::parse(&text).map_err(|e| DawProjectError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "Project" {
        return Err(DawProjectError::NotDawProject(
            root.tag_name().name().to_string(),
        ));
    }
    let mut report = Report::default();
    let app = child(root, "Application")
        .map(|a| {
            format!(
                "{} {}",
                attr(a, "name").unwrap_or("?"),
                attr(a, "version").unwrap_or("")
            )
        })
        .unwrap_or_else(|| "?".to_string());
    let ours = child(root, "Application").and_then(|a| attr(a, "name")) == Some(APPLICATION);

    // The title, from metadata.xml.
    let title = file("metadata.xml")
        .and_then(|m| {
            let t = String::from_utf8_lossy(m).into_owned();
            let d = roxmltree::Document::parse(&t).ok()?;
            let title = child(d.root_element(), "Title")?.text()?.trim().to_string();
            (!title.is_empty()).then_some(title)
        })
        .unwrap_or_else(|| "Imported".to_string());
    let mut project = Project::empty(title);

    if let Some(tr) = child(root, "Transport") {
        if let Some(t) = child(tr, "Tempo").and_then(|t| attr_f64(t, "value")) {
            project.tempo_bpm = cantus_core::time::clamp_bpm(t);
        }
        if let Some(ts) = child(tr, "TimeSignature") {
            let n = attr_f64(ts, "numerator").unwrap_or(4.0) as u8;
            let d = attr_f64(ts, "denominator").unwrap_or(4.0) as u8;
            if n > 0 && d > 0 {
                project.time_signature = TimeSignature::new(n, d);
            }
        }
    }

    let mut tracks = Vec::new();
    let mut channels = Vec::new();
    if let Some(s) = child(root, "Structure") {
        walk_structure(s, None, &mut tracks, &mut channels);
    }
    let master = channels
        .iter()
        .position(|c| c.role == "master")
        .and_then(|i| channels[i].id.clone());
    let timeline = child(root, "Arrangement")
        .map(|arr| read_timeline::read(arr, &project, &mut report))
        .unwrap_or_default();
    tempo_map(&mut project, &timeline, &mut report);
    if let Some(scenes) = child(root, "Scenes") {
        let n = scenes.children().filter(|c| c.is_element()).count();
        report.count_dropped("interop.report.d_scenes", n);
    }

    let built = build(
        &mut project,
        &tracks,
        &channels,
        master.as_deref(),
        &timeline,
        files,
        &mut report,
    );
    report.info(
        "interop.report.d_read",
        vec![
            ("app", app),
            ("tracks", built.0.to_string()),
            ("clips", built.1.to_string()),
            ("notes", built.2.to_string()),
        ],
    );
    if !ours {
        report.info("interop.report.d_foreign", vec![]);
    }
    // The audio files the archive carries, for the caller to put on disk (round 29).
    let media: Vec<(String, Vec<u8>)> = files
        .iter()
        .filter(|(p, _)| {
            let p = p.trim_start_matches("./");
            project.audio_pool.iter().any(|s| s.path == p)
        })
        .map(|(p, d)| (p.trim_start_matches("./").to_string(), d.clone()))
        .collect();
    Ok(Imported {
        project,
        report,
        media,
    })
}

/// A file of the archive, by the path the XML gives.
fn state_file<'f>(files: &'f [(String, Vec<u8>)], path: &str) -> Option<&'f [u8]> {
    let path = path.trim_start_matches("./");
    files
        .iter()
        .find(|(n, _)| n.trim_start_matches("./") == path)
        .map(|(_, d)| d.as_slice())
}

/// Make the strips and the tracks. Returns how many tracks, clips and notes came in.
fn build(
    project: &mut Project,
    tracks: &[TrackInfo],
    channels: &[ChannelInfo],
    master: Option<&str>,
    timeline: &Timeline,
    files: &[(String, Vec<u8>)],
    report: &mut Report,
) -> (usize, usize, usize) {
    let owner_of = |ci: usize| tracks.iter().find(|t| t.channel == Some(ci));
    let is_notes = |t: &TrackInfo| {
        let master_track = master.is_some()
            && t.channel
                .is_some_and(|ci| channels[ci].id.as_deref() == master);
        !master_track
            && (t.content.iter().any(|c| c == "notes")
                || t.id
                    .as_ref()
                    .is_some_and(|id| timeline.clips.contains_key(id)))
    };

    // The strips, one per channel that is not the master's or a track's own.
    let mut strip_of: HashMap<String, StripId> = HashMap::new();
    let mut strip_for_channel: Vec<Option<StripId>> = vec![None; channels.len()];
    let mut too_many = 0;
    for (ci, ch) in channels.iter().enumerate() {
        let n = ch.node;
        if master.is_some() && ch.id.as_deref() == master {
            if let Some(v) = child(n, "Volume").and_then(volume_db) {
                project.master.gain_db = v;
            }
            if let Some(p) = child(n, "Pan").and_then(normalized) {
                project.master.pan = (p * 2.0 - 1.0) as f32;
            }
            for d in devices(n) {
                report.dropped("interop.report.d_master_device", vec![("name", d.name)]);
            }
            continue;
        }
        if ch.track_channel {
            continue;
        }
        if ch.role == "vca" {
            report.dropped("interop.report.d_vca", vec![("name", ch.owner.clone())]);
            continue;
        }
        if project.strips.len() >= MAX_STRIPS {
            too_many += 1;
            continue;
        }
        let owner = owner_of(ci);
        let notes_owner = owner.is_some_and(is_notes);
        // A notes track's strip is shown under the track's name; a bus or a return is
        // named after its track.
        let name = attr(n, "name").map(str::to_string).unwrap_or_else(|| {
            if notes_owner {
                String::new()
            } else {
                ch.owner.clone()
            }
        });
        let mut s = project.make_strip(name);
        if let Some(v) = child(n, "Volume").and_then(volume_db) {
            s.gain_db = v;
        }
        if let Some(p) = child(n, "Pan").and_then(normalized) {
            s.pan = (p * 2.0 - 1.0) as f32;
        }
        s.muted = child(n, "Mute")
            .and_then(|m| attr_bool(m, "value"))
            .unwrap_or(false);
        s.soloed = attr_bool(n, "solo").unwrap_or(false);
        s.color = parse_color(attr(n, "color")).or_else(|| {
            if notes_owner {
                None
            } else {
                owner.and_then(|t| t.color)
            }
        });
        if ch.role == "effect" {
            s.dock = Dock::Right;
        }
        if let Some(id) = &ch.id {
            strip_of.insert(id.clone(), s.id);
        }
        strip_for_channel[ci] = Some(s.id);
        project.strips.push(s);
    }
    if too_many > 0 {
        report.dropped(
            "interop.report.d_too_many_strips",
            vec![("n", too_many.to_string()), ("max", MAX_STRIPS.to_string())],
        );
    }

    // Where each channel goes: its destination, or its group's, or the master.
    let target = |ch: &ChannelInfo| -> Option<StripId> {
        let dest = ch.destination.as_deref().or(ch.parent.as_deref())?;
        if Some(dest) == master {
            return None;
        }
        strip_of.get(dest).copied()
    };

    // Routing, sends and devices, now that every strip exists.
    for (ci, ch) in channels.iter().enumerate() {
        let Some(sid) = strip_for_channel[ci] else {
            continue;
        };
        let out = target(ch);
        if out.is_some() && !project.routing_would_cycle(sid, out) {
            project.strip_mut(sid).expect("made above").output = out;
        } else if out.is_some() {
            report.dropped("interop.report.d_cycle", vec![("name", ch.owner.clone())]);
        }
        let sends = child(ch.node, "Sends")
            .into_iter()
            .flat_map(|s| s.children().filter(|c| c.has_tag_name("Send")));
        for send in sends {
            let Some(to) = attr(send, "destination")
                .and_then(|d| strip_of.get(d))
                .copied()
            else {
                report.dropped("interop.report.d_send", vec![("name", ch.owner.clone())]);
                continue;
            };
            // DAWproject's send types are `pre` and `post` (schema 23 carries both).
            let tap = if attr(send, "type") == Some("pre") {
                SendTap::Pre
            } else {
                SendTap::Post
            };
            let level = child(send, "Volume")
                .map(|v| {
                    let value = attr_f64(v, "value").unwrap_or(1.0);
                    match attr(v, "unit") {
                        Some("decibel") => cantus_core::db_to_linear(value as f32),
                        _ => value as f32,
                    }
                })
                .unwrap_or(1.0)
                .clamp(0.0, 1.0);
            let enabled = child(send, "Enable")
                .and_then(|e| attr_bool(e, "value"))
                .unwrap_or(true);
            let full = project
                .strip(sid)
                .is_some_and(|s| s.sends.len() >= MAX_SENDS);
            if to == sid || full || project.routing_would_cycle(sid, Some(to)) {
                report.dropped("interop.report.d_send", vec![("name", ch.owner.clone())]);
                continue;
            }
            let strip = project.strip_mut(sid).expect("made above");
            strip.sends.retain(|s| s.to != to);
            strip.sends.push(Send {
                to,
                level,
                enabled,
                tap,
            });
        }
        strip_devices(project, sid, ch, &strip_of, files, report);
    }

    // The tracks.
    let (mut n_tracks, mut n_clips, mut n_notes) = (0, 0, 0);
    let mut skipped = 0;
    for t in tracks.iter().filter(|t| is_notes(t)) {
        if project.tracks.len() >= MAX_TRACKS {
            skipped += 1;
            continue;
        }
        let ch = t.channel.map(|ci| &channels[ci]);
        let mut track = project.make_track(if t.name.is_empty() { "Track" } else { &t.name });
        if let Some(c) = t.color {
            track.color = c;
        }
        let key = t.id.clone().unwrap_or_default();
        let clips = timeline.clips.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        let drums = timeline.drums.contains(&key);
        track.instrument = instrument(ch, &t.name, drums, clips, files, report);
        // A track of audio records audio (round 29's review): an input, as a new audio
        // track has.
        if t.content.iter().any(|c| c == "audio") {
            track.input = Some(cantus_core::audio::TrackInput {
                first: 0,
                stereo: true,
                monitor: Default::default(),
                trim_db: 0.0,
            });
        }
        match ch {
            Some(c) if c.track_channel => {
                track.strip = target(c);
                let n = c.node;
                if let Some(v) = child(n, "Volume").and_then(volume_db) {
                    track.gain_db = v;
                }
                if let Some(p) = child(n, "Pan").and_then(normalized) {
                    track.pan = (p * 2.0 - 1.0) as f32;
                }
                track.muted = child(n, "Mute")
                    .and_then(|m| attr_bool(m, "value"))
                    .unwrap_or(false);
                track.soloed = attr_bool(n, "solo").unwrap_or(false);
            }
            Some(_) => track.strip = t.channel.and_then(|ci| strip_for_channel[ci]),
            None => track.strip = None,
        }
        for c in clips {
            let mut clip = project.make_clip(c.name.clone(), c.start, c.length);
            clip.offset = c.offset;
            clip.content_length = c.period;
            clip.muted = c.muted;
            clip.color = c.color;
            // An audio clip (round 29, ADR 0061): its file in the pool (once), its warp.
            if let Some(au) = &c.audio {
                let id = match project.audio_pool.iter().find(|s| s.path == au.path) {
                    Some(s) => s.id,
                    None => {
                        let id = cantus_core::AudioSourceId(project.alloc_id());
                        let name = std::path::Path::new(&au.path)
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| c.name.clone());
                        project.audio_pool.push(cantus_core::AudioSource {
                            id,
                            name,
                            path: au.path.clone(),
                            sample_rate: au.rate,
                            channels: au.channels,
                            frames: (au.seconds * au.rate as f64).round() as u64,
                            sha256: None,
                            recorded: false,
                        });
                        id
                    }
                };
                let mut region = super::audio::region(id, au, &project.tempo_map(), c.origin);
                region.slip_frames += c.slip;
                region.fade_in = c.fades.0;
                region.fade_out = c.fades.1;
                clip.audio = Some(region);
            }
            for n in &c.notes {
                let note = project.make_note(n.start, n.length, n.pitch, n.velocity);
                clip.notes.push(note);
            }
            n_notes += clip.notes.len();
            n_clips += 1;
            track.clips.push(clip);
        }
        project.tracks.push(track);
        n_tracks += 1;
    }
    if skipped > 0 {
        report.dropped(
            "interop.report.d_too_many_tracks",
            vec![("n", skipped.to_string()), ("max", MAX_TRACKS.to_string())],
        );
    }
    // A strip named after the first track feeding it is what the desk shows for an
    // unnamed one anyway: leave it unnamed, so renaming the track renames it.
    for i in 0..project.strips.len() {
        let id = project.strips[i].id;
        let first = project.tracks_on_strip(id).next().map(|t| t.name.clone());
        if first.as_deref() == Some(project.strips[i].name.as_str()) {
            project.strips[i].name.clear();
        }
    }
    // A desk as full as a new project's, so there is somewhere to route to.
    while project.strips.len() < DEFAULT_STRIPS {
        let s = project.make_strip("");
        project.strips.push(s);
    }
    (n_tracks, n_clips, n_notes)
}

/// A track's instrument: Cantus's own, restored from its state; otherwise a kit for a part
/// on channel 10 and Firmus for anything else, with the report naming what was replaced.
fn instrument(
    ch: Option<&ChannelInfo>,
    track: &str,
    drums: bool,
    clips: &[read_timeline::ClipData],
    files: &[(String, Vec<u8>)],
    report: &mut Report,
) -> InstrumentSpec {
    let found = ch
        .map(|c| devices(c.node))
        .unwrap_or_default()
        .into_iter()
        .find(|d| d.role == "instrument");
    if let Some(d) = &found {
        if d.device_id.starts_with(DEVICE_PREFIX) {
            let restored = d
                .state
                .as_deref()
                .and_then(|p| state_file(files, p))
                .and_then(|b| serde_json::from_slice::<InstrumentSpec>(b).ok());
            if let Some(spec) = restored {
                // Written before a calibration, a native instrument has no trim: it
                // comes back at the level it was exported with (ADR 0039).
                return match spec {
                    InstrumentSpec::Native(n) => InstrumentSpec::Native(n.read_unversioned()),
                    other => other,
                };
            }
        }
        report.dropped(
            "interop.report.d_instrument",
            vec![("name", d.name.clone()), ("track", track.to_string())],
        );
    }
    if drums {
        let keys = clips.iter().flat_map(|c| c.notes.iter().map(|n| n.pitch));
        InstrumentSpec::Pulsus(crate::midi::import::kit_for_keys(keys, report))
    } else {
        InstrumentSpec::default()
    }
}

/// A channel's audio devices onto its strip: Cantus's own restored, the built-in
/// equaliser as the strip's EQ, everything else named in the report.
fn strip_devices(
    project: &mut Project,
    sid: StripId,
    ch: &ChannelInfo,
    strip_of: &HashMap<String, StripId>,
    files: &[(String, Vec<u8>)],
    report: &mut Report,
) {
    for d in devices(ch.node) {
        if d.role == "instrument" {
            continue; // The track's: handled with it.
        }
        let full = project
            .strip(sid)
            .is_some_and(|s| s.effects.len() >= MAX_INSERTS);
        if let Some(kind) = effect_kind_of(&d.device_id) {
            let st = d
                .state
                .as_deref()
                .and_then(|p| state_file(files, p))
                .and_then(|b| serde_json::from_slice::<EffectState>(b).ok())
                .filter(|st| st.spec.kind == kind);
            if let (Some(st), false) = (st, full) {
                // A key that would close a loop, or that did not come, is left off.
                let key = st
                    .sidechain
                    .and_then(|c| strip_of.get(&c).copied())
                    .filter(|k| *k != sid && !project.routing_would_cycle(*k, Some(sid)));
                let slot = EffectSlot {
                    id: EffectId(project.alloc_id()),
                    bypass: !d.enabled,
                    mix: st.mix.clamp(0.0, 1.0),
                    sidechain: key,
                    spec: st.spec,
                    // DAWproject has no notes-into-an-effect routing to carry.
                    midi_from: None,
                };
                project.strip_mut(sid).expect("made").effects.push(slot);
                continue;
            }
        }
        if d.element == "Equalizer" && channel_eq(project, sid, d.node) {
            continue;
        }
        let key = match d.element.as_str() {
            "Vst2Plugin" | "Vst3Plugin" | "ClapPlugin" | "AuPlugin" => "interop.report.d_plugin",
            _ => "interop.report.d_device",
        };
        report.dropped(
            key,
            vec![("name", d.name.clone()), ("track", ch.owner.clone())],
        );
    }
}

/// The built-in equaliser as the strip's three bands: a low shelf, a bell and a high
/// shelf, each at most once. Anything else does not fit, and the caller reports it.
fn channel_eq(project: &mut Project, sid: StripId, eq: Node) -> bool {
    let mut out = cantus_core::StripEq::default();
    let mut taken = [false; 3];
    for b in eq.children().filter(|c| c.has_tag_name("Band")) {
        let i = match attr(b, "type") {
            Some("lowShelf") => 0,
            Some("bell") => 1,
            Some("highShelf") => 2,
            _ => return false,
        };
        if taken[i] {
            return false;
        }
        taken[i] = true;
        let enabled = child(b, "Enabled")
            .and_then(|e| attr_bool(e, "value"))
            .unwrap_or(true);
        let get = |name: &str| child(b, name).and_then(|p| attr_f64(p, "value"));
        use cantus_core::EqField;
        if let Some(f) = get("Freq") {
            out.set(i, EqField::Freq, f as f32);
        }
        if let Some(g) = get("Gain").filter(|_| enabled) {
            out.set(i, EqField::Gain, g as f32);
        }
        if let Some(q) = get("Q") {
            out.set(i, EqField::Q, q as f32);
        }
    }
    project.strip_mut(sid).expect("made").eq = out;
    true
}

/// The file's metre points as the song's metre changes: valid signatures only, two at one
/// tick the later (review 2, finding 58), the transport's metre at tick 0 when the first is
/// later. Empty when the file has none.
pub(crate) fn metre_list(
    points: &[(Ticks, u8, u8)],
    transport: TimeSignature,
) -> Vec<cantus_core::tempo::MetreChange> {
    use cantus_core::tempo::{valid_signature, MetreChange};
    let mut ms: Vec<MetreChange> = points
        .iter()
        .map(|(tick, n, d)| MetreChange {
            tick: *tick,
            signature: TimeSignature::new(*n, *d),
        })
        .filter(|m| m.tick.0 >= 0 && valid_signature(m.signature))
        .collect();
    ms.sort_by_key(|m| m.tick);
    ms.reverse();
    ms.dedup_by_key(|m| m.tick);
    ms.reverse();
    if ms.first().is_some_and(|m| m.tick != Ticks::ZERO) {
        ms.insert(
            0,
            MetreChange {
                tick: Ticks::ZERO,
                signature: transport,
            },
        );
    }
    ms
}

/// The tempo track, the metre changes and the markers from the file's arrangement (round
/// 29, ADR 0060): the first tempo and metre at tick 0 become the song's, the transport's
/// standing in where the automation starts later.
fn tempo_map(project: &mut Project, t: &Timeline, report: &mut Report) {
    use cantus_core::tempo::{Marker, MarkerId};
    // The tempo track as the timeline read it (its points normalised there).
    let pts = t.tempo.clone();
    if !pts.is_empty() {
        project.tempo_bpm = pts[0].bpm;
        if pts.len() > 1 {
            report.info(
                "tempo.report.tempo_kept",
                vec![("n", (pts.len() - 1).to_string())],
            );
            project.tempo_points = pts;
        }
    }
    let ms = metre_list(&t.metres, project.time_signature);
    if !ms.is_empty() {
        project.time_signature = ms[0].signature;
        if ms.len() > 1 {
            report.info(
                "tempo.report.metres_kept",
                vec![("n", (ms.len() - 1).to_string())],
            );
            project.metre_changes = ms;
        }
    }
    for (tick, name, color) in &t.markers {
        let id = MarkerId(project.alloc_id());
        project.markers.push(Marker {
            id,
            tick: (*tick).max(Ticks::ZERO),
            name: name.clone(),
            color: *color,
        });
    }
    project.markers.sort_by_key(|m| (m.tick, m.id));
    if !t.markers.is_empty() {
        report.info(
            "tempo.report.markers_kept",
            vec![("n", t.markers.len().to_string())],
        );
    }
}

//! On-disk representation and schema migrations (ADR 0006).

use cantus_core::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const FORMAT_ID: &str = "cantus-project";
pub const SCHEMA_VERSION: u32 = 27;
pub const FILE_EXTENSION: &str = "cantus";

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("not a Cantus project (format id {0:?})")]
    WrongFormat(String),
    #[error(
        "project was saved by a newer Cantus (schema {found}, this build reads up to {supported})"
    )]
    NewerSchema { found: u32, supported: u32 },
    #[error("migration {from}→{to} failed: {reason}")]
    Migration { from: u32, to: u32, reason: String },
    #[error("integrity check failed: stored {stored}, computed {computed}")]
    Integrity { stored: String, computed: String },
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// The document as written to disk. `project` is the payload; everything else is envelope.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format: String,
    pub schema_version: u32,
    pub app_version: String,
    /// Seconds since the Unix epoch.
    pub saved_at: u64,
    pub project: Project,
    /// SHA-256 (hex) of the canonical JSON of `project`.
    pub integrity: Option<String>,
}

impl ProjectFile {
    pub fn wrap(project: Project, app_version: &str) -> Result<Self, FormatError> {
        let integrity = Some(hash_project(&project)?);
        Ok(ProjectFile {
            format: FORMAT_ID.to_string(),
            schema_version: SCHEMA_VERSION,
            app_version: app_version.to_string(),
            saved_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            project,
            integrity,
        })
    }

    /// Serialize for disk. The `project` payload is embedded as the *same* JSON value that
    /// was hashed, so what is written is exactly what `parse` will verify.
    pub fn to_json(&self) -> Result<String, FormatError> {
        let mut envelope = serde_json::Map::new();
        envelope.insert("format".into(), Value::from(self.format.as_str()));
        envelope.insert("schema_version".into(), Value::from(self.schema_version));
        envelope.insert("app_version".into(), Value::from(self.app_version.as_str()));
        envelope.insert("saved_at".into(), Value::from(self.saved_at));
        envelope.insert("project".into(), project_value(&self.project)?);
        envelope.insert(
            "integrity".into(),
            self.integrity
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        );
        Ok(serde_json::to_string_pretty(&Value::Object(envelope))?)
    }
}

/// The project as a JSON value with numbers formatted the way they will be written
/// (f32 fields print as their shortest representation, e.g. `0.005`, not the f64
/// expansion). Going through text guarantees hash and file agree.
fn project_value(project: &Project) -> Result<Value, FormatError> {
    Ok(serde_json::from_str(&serde_json::to_string(project)?)?)
}

/// Canonical hash: compact JSON of the project value.
pub fn hash_project(project: &Project) -> Result<String, FormatError> {
    Ok(hash_value(&project_value(project)?))
}

fn hash_value(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

pub struct LoadReport {
    pub file: ProjectFile,
    pub migrated_from: Option<u32>,
    pub integrity_verified: bool,
    /// Clips taken out of a link group whose other members they did not match (ADR 0037):
    /// a file edited by hand. Zero for every file Cantus wrote.
    pub links_repaired: usize,
}

/// Parse JSON text into a project, migrating older schemas. `verify_integrity = false`
/// lets a user open a file whose hash does not match (after being warned).
pub fn parse(text: &str, verify_integrity: bool) -> Result<LoadReport, FormatError> {
    let mut value: Value = serde_json::from_str(text)?;
    let format = value
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if format != FORMAT_ID {
        return Err(FormatError::WrongFormat(format));
    }
    let found = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    if found > SCHEMA_VERSION {
        return Err(FormatError::NewerSchema {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    let migrated_from = if found < SCHEMA_VERSION {
        Some(found)
    } else {
        None
    };
    let mut version = found;
    while version < SCHEMA_VERSION {
        let step = MIGRATIONS
            .get(version as usize)
            .ok_or_else(|| FormatError::Migration {
                from: version,
                to: version + 1,
                reason: "no migration registered".into(),
            })?;
        step(&mut value).map_err(|reason| FormatError::Migration {
            from: version,
            to: version + 1,
            reason,
        })?;
        version += 1;
        value["schema_version"] = Value::from(version);
    }

    let mut integrity_verified = false;
    if let Some(stored) = value.get("integrity").and_then(Value::as_str) {
        if migrated_from.is_none() {
            let computed = value.get("project").map(hash_value).unwrap_or_default();
            if computed != stored {
                if verify_integrity {
                    return Err(FormatError::Integrity {
                        stored: stored.to_string(),
                        computed,
                    });
                }
            } else {
                integrity_verified = true;
            }
        }
    }

    let mut file: ProjectFile = serde_json::from_value(value)?;
    let links_repaired = cantus_core::links::repair(&mut file.project);
    Ok(LoadReport {
        file,
        migrated_from,
        integrity_verified,
        links_repaired,
    })
}

/// Migration `i` converts schema `i` → `i + 1`. Never remove or reorder entries: a file
/// saved today must open in five years (idea.md §5.15).
type Migration = fn(&mut Value) -> Result<(), String>;
static MIGRATIONS: &[Migration] = &[
    migrate_0_to_1,
    migrate_1_to_2,
    migrate_2_to_3,
    migrate_3_to_4,
    migrate_4_to_5,
    migrate_5_to_6,
    migrate_6_to_7,
    migrate_7_to_8,
    migrate_8_to_9,
    migrate_9_to_10,
    migrate_10_to_11,
    migrate_11_to_12,
    migrate_12_to_13,
    migrate_13_to_14,
    migrate_14_to_15,
    migrate_15_to_16,
    migrate_16_to_17,
    migrate_17_to_18,
    migrate_18_to_19,
    migrate_19_to_20,
    migrate_20_to_21,
    migrate_21_to_22,
    migrate_22_to_23,
    migrate_23_to_24,
    migrate_24_to_25,
    migrate_25_to_26,
    migrate_26_to_27,
];

/// Schema 0 never shipped; this exists so the chain is exercised by tests from day one.
fn migrate_0_to_1(value: &mut Value) -> Result<(), String> {
    let project = value.get_mut("project").ok_or("missing project")?;
    if project.get("loop_enabled").is_none() {
        project["loop_enabled"] = Value::Bool(false);
    }
    if project.get("metronome").is_none() {
        project["metronome"] = Value::Bool(false);
    }
    Ok(())
}

/// Schema 2 added per-track insert effects.
fn migrate_1_to_2(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        if t.get("effects").is_none() {
            t["effects"] = Value::Array(Vec::new());
        }
    }
    Ok(())
}

/// Schema 3 added per-note mute.
fn migrate_2_to_3(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        let Some(clips) = t.get_mut("clips").and_then(Value::as_array_mut) else {
            continue;
        };
        for c in clips {
            let Some(notes) = c.get_mut("notes").and_then(Value::as_array_mut) else {
                continue;
            };
            for n in notes {
                if n.get("muted").is_none() {
                    n["muted"] = Value::Bool(false);
                }
            }
        }
    }
    Ok(())
}

/// Schema 4 turned a clip into a *window onto its content*: it gained a content offset,
/// an optional repeat period, a mute and a colour override.
///
/// The migration writes the values that mean "behave exactly as before" — offset zero and
/// no repeat — rather than trying to be clever. A schema-3 clip played its notes once,
/// relative to its own start, and that is what `offset: 0, content_length: null` is. In
/// particular the repeat is *not* seeded from the clip's length: that would silently turn
/// every old clip into a looping one the first time somebody dragged its right edge.
fn migrate_3_to_4(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        let Some(clips) = t.get_mut("clips").and_then(Value::as_array_mut) else {
            continue;
        };
        for c in clips {
            if c.get("offset").is_none() {
                c["offset"] = Value::from(0);
            }
            if c.get("content_length").is_none() {
                c["content_length"] = Value::Null;
            }
            if c.get("muted").is_none() {
                c["muted"] = Value::Bool(false);
            }
            if c.get("color").is_none() {
                c["color"] = Value::Null;
            }
        }
    }
    Ok(())
}

/// Schema 5 added the pan law, stereo width, and polarity inversion (ADR 0012).
///
/// **The pan law is the reason this migration matters.** New projects default to −3 dB,
/// but a project written before the setting existed was mixed by ear against the law
/// Cantus actually had, so it migrates to `"0"` — which is that behaviour exactly, to the
/// sample (`PanLaw::the_zero_db_law_is_bit_identical_to_what_cantus_did_before`). A
/// migration that silently re-balanced every panned track in somebody's finished mix
/// would be the worst kind of bug: one you only hear.
fn migrate_4_to_5(value: &mut Value) -> Result<(), String> {
    let project = value.get_mut("project").ok_or("missing project")?;
    if project.get("pan_law").is_none() {
        project["pan_law"] = Value::from("0");
    }
    if let Some(master) = project.get_mut("master") {
        if master.get("pan").is_none() {
            master["pan"] = Value::from(0.0);
        }
        if master.get("width").is_none() {
            master["width"] = Value::from(1.0);
        }
        if master.get("phase_invert").is_none() {
            master["phase_invert"] = Value::Bool(false);
        }
    }
    let tracks = project
        .get_mut("tracks")
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        if t.get("width").is_none() {
            t["width"] = Value::from(1.0);
        }
        if t.get("phase_invert").is_none() {
            t["phase_invert"] = Value::Bool(false);
        }
    }
    Ok(())
}

/// Schema 6 is **Pulsus**: `InstrumentSpec` gained a second variant.
///
/// Nothing in a schema-5 file changes shape, so this migration writes nothing — and that
/// is the honest implementation rather than a placeholder. The version number is doing
/// the work: a schema-6 project may contain a track whose instrument is
/// `{"type": "Pulsus", ...}`, and a build that predates the kit cannot render it. Bumping
/// the number is what makes that build say *"saved by a newer Cantus"* and stop, instead
/// of failing somewhere further in with a message about an unknown enum variant.
///
/// The one thing it does check is that every track still has an instrument tag, because a
/// file that lost one would otherwise fail deserialization with no indication of which
/// track was at fault.
fn migrate_5_to_6(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for (i, t) in tracks.iter().enumerate() {
        if t.get("instrument").and_then(|i| i.get("type")).is_none() {
            return Err(format!("track {i} has no instrument"));
        }
    }
    Ok(())
}

/// Schema 7 gave a track a **height** and an **icon**.
///
/// Both migrate to the values that mean "as before": height 1, which is the base row every
/// schema-6 track was drawn at, and no icon, which is *derive one from the instrument*
/// rather than *have none*. Writing a derived icon into the file here would have frozen it
/// the first time somebody changed the track's instrument.
fn migrate_6_to_7(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        if t.get("height").is_none() {
            t["height"] = Value::from(1.0);
        }
        if t.get("icon").is_none() {
            t["icon"] = Value::Null;
        }
    }
    Ok(())
}

/// Schema 8: a pad can be a **sample**. `DrumPad` gained `sample` (a file path) and
/// `PadVoice` gained `Sample`.
///
/// Every schema-7 pad was synthesized, so the migration writes `sample: null` and leaves
/// the voice alone. The path is stored as given rather than resolved: a project points at
/// the musician's library, and a file that has moved makes a pad silent — never a project
/// that will not open.
fn migrate_7_to_8(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        let Some(pads) = t
            .get_mut("instrument")
            .and_then(|i| i.get_mut("pads"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for pad in pads {
            if pad.get("sample").is_none() {
                pad["sample"] = Value::Null;
            }
        }
    }
    Ok(())
}

/// Schema 9 gave the mixer **strips of its own**. Before it, a track *was* a mixer
/// channel: its inserts, width and polarity lived on the track. Now a track feeds a strip
/// and the strip holds those; several tracks can feed one strip, and a strip can feed
/// another.
///
/// The migration makes one strip per track, in track order, and **moves** the track's
/// fader, pan, width, polarity and inserts onto it, leaving the track's own gain and pan at
/// unity. That is what keeps the render bit-identical: the signal meets exactly the same
/// operations in exactly the same order, with two multiplications by 1.0 in front of them
/// (`a_pre_strip_project_renders_exactly_as_it_did` holds the hash). Then it pads the
/// mixer to its default sixteen strips, so an old project opens with the same desk a new
/// one does.
fn migrate_8_to_9(value: &mut Value) -> Result<(), String> {
    let project = value.get_mut("project").ok_or("missing project")?;
    let mut next_id = project
        .get("next_id")
        .and_then(Value::as_u64)
        .ok_or("missing next_id")?;
    let mut strips: Vec<Value> = Vec::new();
    let tracks = project
        .get_mut("tracks")
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks.iter_mut() {
        let obj = t.as_object_mut().ok_or("track is not an object")?;
        let id = next_id;
        next_id += 1;
        let take = |obj: &mut serde_json::Map<String, Value>, key: &str, default: Value| {
            obj.remove(key).unwrap_or(default)
        };
        let gain = take(obj, "gain_db", Value::from(0.0));
        let pan = take(obj, "pan", Value::from(0.0));
        let width = take(obj, "width", Value::from(1.0));
        let phase = take(obj, "phase_invert", Value::Bool(false));
        let effects = take(obj, "effects", Value::Array(Vec::new()));
        obj.insert("gain_db".into(), Value::from(0.0));
        obj.insert("pan".into(), Value::from(0.0));
        obj.insert("strip".into(), Value::from(id));
        strips.push(serde_json::json!({
            "id": id,
            "name": "",
            "gain_db": gain,
            "pan": pan,
            "width": width,
            "phase_invert": phase,
            "muted": false,
            "effects": effects,
            "output": Value::Null,
            "color": Value::Null,
        }));
    }
    while strips.len() < cantus_core::DEFAULT_STRIPS {
        let id = next_id;
        next_id += 1;
        strips.push(serde_json::json!({
            "id": id,
            "name": "",
            "gain_db": 0.0,
            "pan": 0.0,
            "width": 1.0,
            "phase_invert": false,
            "muted": false,
            "effects": [],
            "output": Value::Null,
            "color": Value::Null,
        }));
    }
    project["strips"] = Value::Array(strips);
    project["next_id"] = Value::from(next_id);
    Ok(())
}

/// Schema 10: the strip grew sends, solo, swap, an FX switch and a three-band EQ, and an
/// insert slot grew its own dry/wet. Every one of them has a serde default that is the
/// identity — no sends, not soloed, not swapped, FX on, EQ flat, mix at unity — so the
/// migration writes nothing, and the null tests hold that a strip with all of them at
/// their defaults renders exactly as a schema-9 strip did.
fn migrate_9_to_10(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 11: strips gained a dock and a group, and the project a list of groups. An old
/// desk opens exactly as it was laid out — every strip in the scrolling middle, no groups
/// — so the migration writes nothing. (A *new* project docks three strips on the right;
/// doing that to an old one would move strips the user placed.)
fn migrate_10_to_11(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 12: Aequalis and Comprimo arrived, a slot can be keyed from another strip
/// (`sidechain`), and a sparse effect stores only the parameters that moved. Nothing old
/// has any of these — no EQ or compressor existed, and a missing `sidechain` is "its own
/// input" — so the migration writes nothing.
fn migrate_11_to_12(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 13: five effect kinds arrived — the limiter, the new reverb, the multiband, the
/// de-esser and the gate. No old file holds one, so the migration writes nothing.
fn migrate_12_to_13(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 14: an insert slot can hold a hosted plug-in (ADR 0019) — `EffectKind::External`
/// with an `external` record: format, id, name, vendor, whether it has a sidechain input,
/// its state (base64) and its parameter values by the plug-in's ids. A native effect's
/// spec writes no `external` at all, so no old file changes shape and the migration writes
/// nothing; the number is what makes an older build refuse a file that holds one.
fn migrate_13_to_14(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 15: an insert slot can hear a track's notes (`midi_from`, ADR 0022). A missing
/// `midi_from` is "no track" — the effect hears nothing and renders as before — so the
/// migration writes nothing.
fn migrate_14_to_15(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 16: native instruments (ADR 0023) — `InstrumentSpec::Native`, a kind with its
/// numbers and texts by key. No old file holds one, so the migration writes nothing.
fn migrate_15_to_16(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 17: Pulsus II — a kit's `version` (absent means model 1, which renders as it
/// always did), stable pad ids, sample layers with round robin, per-pad envelopes, filter,
/// drive and chokes, the hi-hat pedal, and a pad's own `output`. Every field reads from an
/// old file as the value that changes no sample, so the migration writes nothing; the
/// number is what makes an older build refuse a kit it would silently strip.
fn migrate_16_to_17(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 18 (ADR 0025, 0026): audio clips and the audio pool, a track's input and arm,
/// take lanes, recording settings, and the master's insert chain. Every field has a serde
/// default that is exactly the old behaviour, so the step writes nothing; the number is
/// what makes an older build refuse a project with audio in it instead of dropping it.
fn migrate_17_to_18(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 19 (ADR 0027): automation clips (`Clip::automation`) and a track's
/// `automation_open`. A clip without the field is a note or audio clip, and a track
/// without the flag shows its lanes — exactly the old project — so the step writes
/// nothing; the number is what makes an older build refuse a project whose automation it
/// would silently drop.
fn migrate_18_to_19(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 20 (ADR 0028): a track's instrument may be a hosted CLAP/VST3 plug-in
/// (`InstrumentSpec::External`, the same record an insert keeps). No older project has
/// one, so the step writes nothing; the number is what makes an older build refuse a
/// project with a hosted instrument instead of failing on an instrument it cannot read.
/// (Built as schema 19 on its branch; renumbered at the round-20 merge, after automation.)
fn migrate_19_to_20(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 21 (ADR 0039, round 24): Lamina and Exemplar were recalibrated — a new Lamina
/// is 5 dB louder, a new Exemplar's placeholder sine 8 dB — through a hidden output trim
/// (`cantus_core::inst::TRIM_KEY`) whose default is the calibration. Every Lamina and
/// Exemplar saved before gets its trim written as 0 dB, the level it had, so a project
/// renders to the same bits it did; a trim already there is kept.
fn migrate_20_to_21(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        let Some(inst) = t.get_mut("instrument").and_then(Value::as_object_mut) else {
            continue;
        };
        let native = inst.get("type").and_then(Value::as_str) == Some("Native");
        let kind = inst
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !native || !matches!(kind.as_str(), "ElectricPiano" | "Sampler") {
            continue;
        }
        let params = inst
            .entry("params")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(params) = params.as_object_mut() else {
            return Err(format!("a {kind}'s params are not a map"));
        };
        params
            .entry(cantus_core::inst::TRIM_KEY)
            .or_insert_with(|| Value::from(0.0));
    }
    Ok(())
}

/// Schema 22 (ADR 0037): linked clips — a clip may name a link group (`Clip::link`), and
/// every member of a group holds the same content. A clip without the field is a clip of
/// its own, which is what every clip of an older project was, so the step writes nothing;
/// the number is what makes an older build refuse a project whose copies it would
/// silently cut apart.
fn migrate_21_to_22(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 23 (ADR 0041): a send says where it taps its strip (`Send::tap`, post-fader or
/// pre-fader). A send without the field is post-fader, which is what every send of an
/// older project was, so the step writes nothing; the number is what makes an older build
/// refuse a project whose pre-fader sends it would silently turn post-fader.
fn migrate_22_to_23(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 24 (round 26, ADR 0023's amendment): an Exemplar zone may be started by a
/// controller (`z{n}.trig` = 5, with `z{n}.cc`, `z{n}.cclo`, `z{n}.cchi`: a piano's pedal
/// noises). No older zone has the trigger or the keys — and the keys' defaults change no
/// sample — so the step writes nothing; the number is what makes an older build refuse a
/// project whose pedal noises it would play on the keys.
fn migrate_23_to_24(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 25 (round 28): thirteen effect kinds and the Chorda instrument arrived; an
/// older build must refuse them, not misread them. No older project holds one, so the
/// step writes nothing.
fn migrate_24_to_25(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 26 (round 28, ADR 0052): the metronome's voice and accent
/// (`RecordSettings::click_sound`, `click_accent`) and FL's PAT / SONG switch
/// (`Project::pattern_mode`). A file without them has the classic accented click and plays
/// the song — what every older version did — so the step writes nothing; the number is what
/// makes an older build refuse a project whose woodblock or pattern mode it would ignore.
fn migrate_25_to_26(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 27 (round 28, ADR 0054): an audio clip may carry the pitch editor's notes and
/// edits (`AudioRegion::pitch`). An older build must refuse such a file, not play the
/// clip unedited; no older project holds one, so the step writes nothing.
fn migrate_26_to_27(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::Ticks;

    #[test]
    fn schema_21_writes_the_old_level_of_lamina_and_exemplar_down() {
        let mut v = serde_json::json!({"project": {"tracks": [
            {"instrument": {"type": "Native", "kind": "ElectricPiano"}},
            {"instrument": {"type": "Native", "kind": "Sampler", "params": {"z0.gen": 3.0}}},
            {"instrument": {"type": "Native", "kind": "ElectricPiano", "params": {"trim": 2.5}}},
            {"instrument": {"type": "Native", "kind": "Organ"}},
            {"instrument": {"type": "Firmus"}},
        ]}});
        migrate_20_to_21(&mut v).unwrap();
        let t = &v["project"]["tracks"];
        assert_eq!(t[0]["instrument"]["params"]["trim"], 0.0);
        assert_eq!(t[1]["instrument"]["params"]["trim"], 0.0);
        assert_eq!(t[1]["instrument"]["params"]["z0.gen"], 3.0);
        assert_eq!(
            t[2]["instrument"]["params"]["trim"], 2.5,
            "a trim already there stays"
        );
        assert!(
            t[3]["instrument"].get("params").is_none(),
            "other kinds untouched"
        );
        assert!(t[4]["instrument"].get("params").is_none());
        let mut bad = serde_json::json!({"project": {"tracks": [
            {"instrument": {"type": "Native", "kind": "Sampler", "params": 3}}
        ]}});
        assert!(migrate_20_to_21(&mut bad).is_err());
    }

    /// Schema 24: the fixture's pedal-noise zone reads back as a controller trigger, and an
    /// older file (23) opens with no zone a controller starts.
    #[test]
    fn schema_24_keeps_a_zone_the_sustain_pedal_starts() {
        use cantus_core::inst::sampler::{zf, Trigger, Zone};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v24.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let t = p
            .tracks
            .iter()
            .find(|t| t.name == "Exemplar pedal")
            .unwrap();
        let z = Zone::of(t.instrument.native().unwrap(), 1).unwrap();
        assert_eq!(Trigger::from_index(z.get(zf::TRIG)), Trigger::Controller);
        assert_eq!(
            (z.get(zf::CC), z.get(zf::CCLO), z.get(zf::CCHI)),
            (64.0, 126.0, 127.0)
        );
        let old = std::fs::read_to_string(dir.join("v23.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p.tracks.iter().all(|t| t.name != "Exemplar pedal"));
    }

    /// Schema 25 (round 28): the fixture holds one insert of each of the thirteen new
    /// kinds — the transient shaper keyed, the tuner hearing notes — and a Chorda track;
    /// an older file (24) opens with none of them.
    #[test]
    fn schema_25_keeps_the_round_28_kinds() {
        use cantus_core::{EffectKind as K, InstrumentKind};
        let new_kinds = &K::ALL[12..];
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v25.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let slots: Vec<_> = p
            .strips
            .iter()
            .flat_map(|s| s.effects.iter())
            .filter(|e| new_kinds.contains(&e.spec.kind))
            .collect();
        let kinds: Vec<K> = slots.iter().map(|e| e.spec.kind).collect();
        assert_eq!(kinds, new_kinds.to_vec());
        let of = |k: K| slots.iter().find(|e| e.spec.kind == k).unwrap();
        assert!(of(K::Transient).sidechain.is_some());
        assert!(of(K::AutoTune).midi_from.is_some());
        assert_eq!(of(K::Chorus).spec.get("out"), -3.0);
        assert!(p
            .tracks
            .iter()
            .any(|t| t.instrument.native().map(|n| n.kind) == Some(InstrumentKind::Chorda)));
        let old = std::fs::read_to_string(dir.join("v24.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .strips
            .iter()
            .flat_map(|s| s.effects.iter())
            .all(|e| !new_kinds.contains(&e.spec.kind)));
    }

    /// Schema 26 (round 28, ADR 0052): the fixture's metronome has the woodblock voice and
    /// no accent, and the song is saved in pattern mode; an older file (25) opens with the
    /// classic accented click in song mode — what it always played.
    #[test]
    fn schema_26_keeps_the_metronome_voice_and_the_pattern_mode() {
        use cantus_core::ClickSound;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v26.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        assert_eq!(p.recording.click_sound, ClickSound::Wood);
        assert!(!p.recording.click_accent);
        assert!(p.pattern_mode);
        let old = std::fs::read_to_string(dir.join("v25.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert_eq!(p.recording.click_sound, ClickSound::Classic);
        assert!(p.recording.click_accent);
        assert!(!p.pattern_mode);
    }

    /// Schema 27 (round 28, ADR 0054): the fixture's vocal take carries pitch edits — a
    /// note moved and pulled onto its centre, one stretched, the key, the A/B switch — and
    /// they come back as they were; an older file (25) opens with none.
    #[test]
    fn schema_27_keeps_a_takes_pitch_edits() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v27.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let edits: Vec<_> = p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter_map(|c| c.audio.as_ref()?.pitch.as_deref())
            .collect();
        assert_eq!(edits.len(), 1);
        let e = edits[0];
        assert!(e.is_valid() && e.edited());
        assert_eq!((e.rate, e.key.tonic, e.key.scale), (48_000, 9, 2));
        assert_eq!(e.notes.len(), 3);
        assert_eq!((e.notes[0].pitch, e.notes[0].centre), (69.0, 1.0));
        assert!(e.notes[1].moved());
        assert_eq!(e.notes[2].formant, -2.0);
        assert!(!e.bypass);
        let old = std::fs::read_to_string(dir.join("v25.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .all(|c| c.audio.as_ref().is_none_or(|a| a.pitch.is_none())));
    }

    /// Every historical fixture in `fixtures/` must open, whatever its schema version.
    #[test]
    fn all_fixtures_load_and_migrate() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("fixtures dir") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("cantus") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let r = parse(&text, true).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(r.file.schema_version, SCHEMA_VERSION, "{}", path.display());
            assert_eq!(r.file.project.tracks[0].name, "Keys");
            assert_eq!(r.file.project.tracks[0].clips[0].notes.len(), 4);
            // Re-saving a migrated project must round-trip at the current schema.
            let again = ProjectFile::wrap(r.file.project.clone(), "test")
                .unwrap()
                .to_json()
                .unwrap();
            let back = parse(&again, true).unwrap();
            assert_eq!(back.file.project, r.file.project);
            assert!(back.integrity_verified);
            seen += 1;
        }
        assert!(seen >= 1, "no fixtures found in {}", dir.display());
    }

    fn demo() -> Project {
        let mut p = Project::new("Demo");
        let mut t = p.make_track("Lead");
        let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100));
        t.clips.push(c);
        p.tracks.push(t);
        p
    }

    #[test]
    fn roundtrip_with_integrity() {
        let p = demo();
        let text = ProjectFile::wrap(p.clone(), "test")
            .unwrap()
            .to_json()
            .unwrap();
        let r = parse(&text, true).unwrap();
        assert_eq!(r.file.project, p);
        assert!(r.integrity_verified);
        assert_eq!(r.migrated_from, None);
    }

    #[test]
    fn detects_tampering() {
        let text = ProjectFile::wrap(demo(), "test")
            .unwrap()
            .to_json()
            .unwrap();
        let tampered = text.replace("\"tempo_bpm\": 120.0", "\"tempo_bpm\": 121.0");
        assert_ne!(text, tampered);
        assert!(matches!(
            parse(&tampered, true),
            Err(FormatError::Integrity { .. })
        ));
        let r = parse(&tampered, false).unwrap();
        assert!(!r.integrity_verified);
        assert_eq!(r.file.project.tempo_bpm, 121.0);
    }

    #[test]
    fn migrates_schema_0() {
        let mut v = serde_json::to_value(ProjectFile::wrap(demo(), "test").unwrap()).unwrap();
        v["schema_version"] = Value::from(0);
        v["project"].as_object_mut().unwrap().remove("loop_enabled");
        v["project"].as_object_mut().unwrap().remove("metronome");
        let r = parse(&v.to_string(), true).unwrap();
        assert_eq!(r.migrated_from, Some(0));
        assert!(!r.file.project.loop_enabled);
        assert_eq!(r.file.schema_version, SCHEMA_VERSION);
    }

    /// The promise the pan-law migration makes, checked on the real historical files
    /// rather than on a hand-built value.
    #[test]
    fn old_projects_keep_the_pan_law_they_were_mixed_with() {
        use cantus_core::PanLaw;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        for entry in std::fs::read_dir(&dir).expect("fixtures dir") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("cantus") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let r = parse(&text, true).unwrap();
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // Only files written *before* the pan law existed are making a promise. From
            // schema 5 on the field is really in the file, and the fixtures deliberately
            // set it — which is what proves it survives a round trip. Keying this on the
            // stored version rather than on "did it migrate at all" is the difference
            // between a test that stays true and one that breaks at the next schema bump,
            // which is exactly what it did.
            const PAN_LAW_SCHEMA: u32 = 5;
            if r.migrated_from.unwrap_or(SCHEMA_VERSION) >= PAN_LAW_SCHEMA {
                continue;
            }
            for t in &r.file.project.tracks {
                assert_eq!(t.height, 1.0, "{name}: height must migrate to the base row");
                assert_eq!(t.icon, None, "{name}: the icon must stay derived");
            }
            assert_eq!(
                r.file.project.pan_law,
                PanLaw::ZeroDb,
                "{name} migrated to the wrong pan law — its panned tracks would move"
            );
            for t in &r.file.project.tracks {
                let strip = t
                    .strip
                    .and_then(|s| r.file.project.strip(s))
                    .expect("every migrated track feeds a strip");
                assert_eq!(strip.width, 1.0, "{name}: width must migrate to unity");
                assert!(
                    !strip.phase_invert,
                    "{name}: polarity must migrate to normal"
                );
            }
            assert_eq!(r.file.project.master.pan, 0.0, "{name}: master pan");
            assert_eq!(r.file.project.master.width, 1.0, "{name}: master width");
        }
    }

    #[test]
    fn refuses_newer_and_foreign_files() {
        let mut v = serde_json::to_value(ProjectFile::wrap(demo(), "test").unwrap()).unwrap();
        v["schema_version"] = Value::from(999);
        assert!(matches!(
            parse(&v.to_string(), true),
            Err(FormatError::NewerSchema { .. })
        ));
        assert!(matches!(
            parse(r#"{"format":"other"}"#, true),
            Err(FormatError::WrongFormat(_))
        ));
    }
}

#[cfg(test)]
mod strip_migration_tests {
    use super::*;

    /// The strips migration moves a track's mixer controls onto its strip and leaves the
    /// track at unity — checked on every fixture older than schema 9.
    #[test]
    fn old_tracks_hand_their_mixer_controls_to_a_strip() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("fixtures dir") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("cantus") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let r = parse(&text, true).unwrap();
            if r.migrated_from.unwrap_or(SCHEMA_VERSION) >= 9 {
                continue;
            }
            let p = &r.file.project;
            assert!(p.strips.len() >= cantus_core::DEFAULT_STRIPS);
            // The Keys track was written at −4.5 dB, pan 0.25, with a delay.
            let keys = &p.tracks[0];
            assert_eq!(
                keys.gain_db, 0.0,
                "the track's own gain is unity after migration"
            );
            assert_eq!(keys.pan, 0.0);
            let strip = p.strip(keys.strip.unwrap()).unwrap();
            assert_eq!(strip.gain_db, -4.5);
            assert_eq!(strip.pan, 0.25);
            // Inserts arrived in schema 2, so the oldest fixture has none to move.
            let expected = if r.migrated_from.unwrap_or(0) >= 2 {
                1
            } else {
                0
            };
            assert_eq!(
                strip.effects.len(),
                expected,
                "the delay moved to the strip"
            );
            assert_eq!(p.strip_number(strip.id), Some(1), "track 1 feeds strip 1");
            // Ids stay unique: every strip id is above every id the file already used.
            let mut ids: Vec<u64> = p.strips.iter().map(|s| s.id.0).collect();
            ids.extend(p.tracks.iter().map(|t| t.id.0));
            let n = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), n, "{}: an id was reused", path.display());
            checked += 1;
        }
        assert!(checked >= 1);
    }
}

#[cfg(test)]
mod hosted_plugin_tests {
    use super::*;

    /// The schema-14 fixture holds a hosted plug-in that is installed nowhere: the project
    /// opens, and the slot's record — state bytes, values to the bit — is what was written.
    #[test]
    fn a_missing_hosted_plugin_opens_with_its_data_intact() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("v14.cantus");
        let text = std::fs::read_to_string(&path).expect("the v14 fixture");
        let r = parse(&text, true).expect("opens");
        let slot = r
            .file
            .project
            .strips
            .iter()
            .flat_map(|s| s.effects.iter())
            .find(|e| e.spec.kind == cantus_core::EffectKind::External)
            .expect("a hosted slot");
        let plugin = slot.spec.external.as_deref().expect("its record");
        assert_eq!(plugin.id, "org.cantus.fixture.absent");
        assert_eq!(plugin.format, cantus_core::PluginFormat::Clap);
        assert!(plugin.sidechain);
        assert!(slot.sidechain.is_some());
        assert_eq!(slot.mix, 0.75);
        assert_eq!(plugin.state.bytes(), b"CTG1 opaque state \x00\x01\xfe\xff");
        assert_eq!(plugin.params[&0], -6.25);
        assert_eq!(plugin.params[&1024].to_bits(), (0.1f64 + 0.2).to_bits());
        // Written back, it is the same file content.
        let again = ProjectFile::wrap(r.file.project.clone(), "test")
            .unwrap()
            .to_json()
            .unwrap();
        assert_eq!(parse(&again, true).unwrap().file.project, r.file.project);
    }
}

#[cfg(test)]
mod hosted_instrument_tests {
    use super::*;

    /// The schema-20 fixture holds a hosted instrument installed nowhere (ADR 0028): the
    /// project opens, and the track's record — state bytes, values to the bit — is intact.
    #[test]
    fn a_missing_hosted_instrument_opens_with_its_data_intact() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("v20.cantus");
        let text = std::fs::read_to_string(&path).expect("the v20 fixture");
        let r = parse(&text, true).expect("opens");
        let track = r
            .file
            .project
            .tracks
            .iter()
            .find(|t| t.instrument.external().is_some())
            .expect("a track with a hosted instrument");
        assert_eq!(track.name, "Synth");
        let plugin = track.instrument.external().unwrap();
        assert_eq!(plugin.id, "org.cantus.test.synth");
        assert_eq!(plugin.format, cantus_core::PluginFormat::Clap);
        assert_eq!(plugin.vendor, "Cantus Tests");
        assert_eq!(plugin.state.bytes(), b"CTS1 opaque instrument state");
        assert_eq!(plugin.params[&0], -4.5);
        assert_eq!(plugin.params[&7].to_bits(), (0.1f64 + 0.2).to_bits());
        assert_eq!(track.clips[0].notes.len(), 1);
        let again = ProjectFile::wrap(r.file.project.clone(), "test")
            .unwrap()
            .to_json()
            .unwrap();
        assert_eq!(parse(&again, true).unwrap().file.project, r.file.project);
    }
}

#[cfg(test)]
mod linked_clip_tests {
    use super::*;

    fn fixture(n: u32) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(format!("v{n}.cantus"));
        std::fs::read_to_string(&path).expect("fixture")
    }

    /// Schema 22 (ADR 0037): every clip of an older project opens as a clip of its own.
    #[test]
    fn every_clip_of_an_older_project_stays_unique() {
        for n in 1..22 {
            let Ok(text) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join(format!("v{n}.cantus"))
                .canonicalize()
                .and_then(std::fs::read_to_string)
            else {
                continue;
            };
            let r = parse(&text, true).unwrap();
            assert_eq!(r.migrated_from, Some(n));
            assert_eq!(r.links_repaired, 0);
            for t in &r.file.project.tracks {
                assert!(t.clips.iter().all(|c| c.link.is_none()), "v{n}: {}", t.name);
            }
        }
    }

    /// The schema-22 fixture holds a note pattern linked across two tracks (three copies,
    /// one trimmed and slipped) and two linked audio clips; the groups survive a round trip.
    #[test]
    fn the_v22_fixture_keeps_its_groups() {
        let r = parse(&fixture(22), true).expect("opens");
        let p = &r.file.project;
        cantus_core::links::check(p).unwrap();
        let sizes = cantus_core::links::sizes(p);
        let mut counts: Vec<usize> = sizes.values().copied().collect();
        counts.sort();
        assert_eq!(counts, vec![2, 3]);
        let audio_group = p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter(|c| c.audio.is_some() && c.link.is_some())
            .count();
        assert_eq!(audio_group, 2);
        let again = ProjectFile::wrap(p.clone(), "test")
            .unwrap()
            .to_json()
            .unwrap();
        assert_eq!(&parse(&again, true).unwrap().file.project, p);
    }

    /// A file whose group members differ (edited by hand) opens with the odd one out of
    /// the group, its own content kept.
    #[test]
    fn a_broken_group_is_repaired_on_load() {
        let mut v: Value = serde_json::from_str(&fixture(22)).unwrap();
        let clips = v["project"]["tracks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .flat_map(|t| t["clips"].as_array_mut().unwrap().iter_mut())
            .filter(|c| c.get("link").is_some() && c.get("audio").is_none());
        let mut n = 0;
        for c in clips {
            n += 1;
            if n == 2 {
                c["name"] = Value::from("edited by hand");
            }
        }
        assert!(n >= 2);
        let r = parse(&v.to_string(), false).expect("opens");
        assert_eq!(r.links_repaired, 1);
        cantus_core::links::check(&r.file.project).unwrap();
        let odd = r
            .file
            .project
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.name == "edited by hand")
            .unwrap();
        assert_eq!(odd.link, None);
    }
}

#[cfg(test)]
mod send_tap_tests {
    use super::*;
    use cantus_core::SendTap;

    fn read(n: u32) -> Option<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(format!("v{n}.cantus"));
        std::fs::read_to_string(path).ok()
    }

    /// Schema 23 (ADR 0041): every send of an older project opens post-fader, which is
    /// what every send was before the tap existed.
    #[test]
    fn every_send_of_an_older_project_stays_post_fader() {
        let mut sends = 0;
        for n in 1..23 {
            let Some(text) = read(n) else {
                continue;
            };
            let r = parse(&text, true).unwrap();
            assert_eq!(r.migrated_from, Some(n));
            for s in &r.file.project.strips {
                for send in &s.sends {
                    assert_eq!(send.tap, SendTap::Post, "v{n}: {}", s.name);
                    sends += 1;
                }
            }
        }
        assert!(sends > 0, "the old fixtures carry sends");
    }

    /// The schema-23 fixture holds a pre-fader send; it survives a round trip, and a
    /// post-fader send writes no tap at all (the file of a project without a pre-fader
    /// send is the file it was before).
    #[test]
    fn the_v23_fixture_keeps_its_pre_fader_send() {
        let r = parse(&read(23).expect("the v23 fixture"), true).expect("opens");
        let p = &r.file.project;
        let pre: Vec<_> = p
            .strips
            .iter()
            .flat_map(|s| s.sends.iter())
            .filter(|x| x.tap == SendTap::Pre)
            .collect();
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0].level, 0.5);
        let again = ProjectFile::wrap(p.clone(), "test")
            .unwrap()
            .to_json()
            .unwrap();
        assert_eq!(&parse(&again, true).unwrap().file.project, p);
        assert_eq!(
            again.matches("\"tap\"").count(),
            1,
            "only the pre send writes it"
        );
        assert!(again.contains("\"tap\": \"Pre\""));
    }
}

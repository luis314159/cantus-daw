//! On-disk representation and schema migrations (ADR 0006).

use cantus_core::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const FORMAT_ID: &str = "cantus-project";
pub const SCHEMA_VERSION: u32 = 87; // r42: 87 Arcus's fiddler as its own switch (`player.fiddle`, ADR 0166, arcus-classical), 86 Tympanum's model 6 (another branch: its step here is a placeholder the integrator replaces), 85 Arcus's one bow on two strings, sympathetic strings, grit, steel and just double stops (ADR 0164, fiddle), 84 Chorda's colour (`colour`, ADR 0098's sixth amendment, chorda-c17), 83 a bowed note's bow start, bow part and string (ADR 0162, arcus-expr), 82 Exemplar's articulations, bows, controller crossfades and scripted legato (ADR 0158, strings-rec), 81 Arcus (the bowed string) and the note's articulation (ADRs 0157, 0158), 80 the project's and the tracks' tuning, `ExternalUnit::mts` and `ExternalPlugin::tuning_by` (ADRs 0155, 0156); r41: 79 Fides's input as a sidechain from a strip (ADR 0154), 78 Fides (the resonator), Nitor (the enhancer) and Fragor's five circuits as board-only pedals (ADRs 0150–0152); fx (the effects side round, merged after round 40): 72 the side round's five kinds, 73 Tabula's board and three kinds, 74 Fractus's lanes and three kinds, 75 wave B's Cedo shape and three kinds (Vinculum, Lenis, Cedo), 76 wave C's two kinds and Comparatio's references, 77 wave D's X-Y pad and formula controller; r40: 71 Tympanum's voice (generated or hybrid, drums-hyb), 70 a strip added to an instrument's family (tymp-ui), 62 pads (a track's record tap), 63 and 64 unused (reserved for ripple and delay, which needed none), 65 cast (the default dancer: Compás), 66 an instrument's named outputs, 67 Tympanum, 68 Chorda's recorded source, 69 the scene's room look (phase 3); r36: 50 mix, 51 external, 52 editor, 53 ara; r37: 54 piano (chorda), 55 piano (sectio, to come), 56 drums, 57 drums (kit gain), 58 scene (chorda's look), 59 chars (the default character); r39: 60 conv (Aula's response from a file), 61 arrange (sections, folders, lane modes)
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
    /// Warps dropped because they were not valid (ADR 0061: a hand-edited or damaged
    /// file); their clips play at their natural speed. Zero for every file Cantus wrote.
    pub warps_repaired: usize,
    /// Track delays dropped because a bounce could not have kept them (ADR 0064, review 3:
    /// a hand-edited or damaged file). Zero for every file Cantus wrote.
    pub lateness_repaired: usize,
    /// Clip gain envelopes repaired (ADR 0070: points out of range or order in a
    /// hand-edited or damaged file). Zero for every file Cantus wrote.
    pub gain_envs_repaired: usize,
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
    // The session a command would refuse (a hand edit): slots of scenes not there, a
    // session clip out of its form, settings out of range (ADR 0076). First: the
    // frozen keys below read the tracks' columns.
    let session_repaired = file.project.repair_session();
    if session_repaired > 0 {
        log::warn!("{session_repaired} malformed session entr(ies) repaired");
    }
    // Warps a command would refuse (a hand edit) go first (round 29's second review): the
    // links and the frozen keys below are checked against what each clip then holds.
    let warps_repaired = cantus_core::warp::repair(&mut file.project);
    // Gain envelopes a command would refuse (ADR 0070, review 1): never to the audio thread.
    let gain_envs_repaired = cantus_core::clip_gain::repair_project(&mut file.project);
    // Per-note expression a command would refuse (a hand edit): normalised (ADR 0080).
    cantus_core::model::note_expr::repair_project(&mut file.project); // r33:mpe
                                                                      // A track delay out of range never reaches the plan (review 3); before the frozen
                                                                      // keys, which hold it.
    let lateness_repaired = file.project.repair_lateness();
    // fx3:contract — boards a command would refuse (a hand edit): a nested board, a hosted
    // pedal, a duplicate id, a map to nothing (ADR 0131). Never to the audio thread.
    let boards_repaired = cantus_core::fx::tabula::repair_project(&mut file.project);
    if boards_repaired > 0 {
        log::warn!("{boards_repaired} malformed board entr(ies) repaired");
    }
    // fxa:contract — Fractus's lanes a command would refuse (a hand edit): overlapping or
    // past the cycle, or on a slot that is not a Fractus (ADR 0137).
    let lanes_repaired = cantus_core::fx::fractus::repair_project(&mut file.project);
    if lanes_repaired > 0 {
        log::warn!("{lanes_repaired} malformed glitch pattern(s) repaired");
    }
    // fxa:caelum — Caelum's two infinite switches of wave A's first build (never released)
    // made today's Sustain (ADR 0138's amendment): `ishim` to `isus`, `ilevel` dropped.
    let caelum_repaired = cantus_core::fx::caelum::repair_project(&mut file.project);
    if caelum_repaired > 0 {
        log::warn!("{caelum_repaired} Caelum infinite setting(s) made Sustain");
    }
    // fxb:contract — Cedo's shapes a command would refuse (a hand edit): points out of
    // range or order, or on a slot that is not a Cedo (ADR 0141).
    let shapes_repaired = cantus_core::fx::cedo::repair_project(&mut file.project);
    if shapes_repaired > 0 {
        log::warn!("{shapes_repaired} malformed ducking shape(s) repaired");
    }
    // fxc:comparatio — Comparatio's references a command would refuse (a hand edit): a path
    // twice or empty, too many, the one compared missing from the list (ADR 0143).
    let refs_repaired = cantus_core::fx::comparatio::repair_project(&mut file.project);
    if refs_repaired > 0 {
        log::warn!("{refs_repaired} malformed reference list(s) repaired");
    }
    // Provenance of a file not in the pool (a hand edit) names nothing: dropped. // r34:stems
    let pool: std::collections::BTreeSet<_> =
        file.project.audio_pool.iter().map(|s| s.id).collect(); // r34:stems
    file.project.provenance.retain(|id, _| pool.contains(id)); // r34:stems
                                                               // Marks of a file not in the pool, or out of its range (a hand edit), are dropped. // r36:editor
    let frames: std::collections::BTreeMap<_, _> = file
        .project
        .audio_pool
        .iter()
        .map(|s| (s.id, s.frames))
        .collect(); // r36:editor
    file.project.source_marks.retain(|id, m| {
        frames
            .get(id)
            .is_some_and(|f| m.is_valid(*f) && !m.is_empty())
    }); // r36:editor
    let links_repaired = cantus_core::links::repair(&mut file.project);
    // A frozen target whose key is not what it holds (a hand edit): re-keyed, its render
    // made again (ADR 0064). After the links' repair, which may change what one holds.
    cantus_core::freeze::repair(&mut file.project);
    // Controller mappings a command would refuse (a hand edit): dropped, so every edit of
    // the map has an exact inverse (ADR 0062, review 1).
    let dropped = file.project.controllers.repair();
    // Score settings a command would refuse (a hand edit): the nearest valid (ADR 0075).
    for t in file.project.tracks.iter_mut() {
        if !t.score.is_valid() {
            t.score = t.score.repaired();
        }
    }
    if dropped > 0 {
        log::warn!("{dropped} malformed controller mapping(s) dropped");
    }
    Ok(LoadReport {
        file,
        migrated_from,
        integrity_verified,
        links_repaired,
        warps_repaired,
        lateness_repaired,
        gain_envs_repaired,
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
    migrate_27_to_28,
    migrate_28_to_29,
    migrate_29_to_30,
    migrate_30_to_31,
    migrate_31_to_32,
    migrate_32_to_33,
    migrate_33_to_34,
    migrate_34_to_35,
    migrate_35_to_36,
    migrate_36_to_37,
    migrate_37_to_38,
    migrate_38_to_39,
    migrate_39_to_40,
    migrate_40_to_41,
    migrate_41_to_42,
    migrate_42_to_43,
    migrate_43_to_44,
    migrate_44_to_45,
    migrate_45_to_46,
    migrate_46_to_47,
    migrate_47_to_48, // r34:stems
    migrate_48_to_49, // r35:dancer
    migrate_49_to_50, // r36:mix
    migrate_50_to_51, // r36:external
    migrate_51_to_52, // r36:editor
    migrate_52_to_53, // r36:ara
    migrate_53_to_54, // r37:piano
    migrate_54_to_55, // r37:piano (Sectio's, to come: an empty step until then)
    migrate_55_to_56, // r37:drums
    migrate_56_to_57, // r37:drums — a kit's own gain
    migrate_57_to_58, // r37:scene — Chorda's look
    migrate_58_to_59, // r37:chars — the songbird written into dancers that named no character
    migrate_59_to_60, // r39:conv — Aula's response from a file
    migrate_60_to_61, // r39:arrange — sections, folders, lane modes (ADR 0113)
    migrate_61_to_62, // r40:pads — a track's record tap (ADR 0118)
    migrate_62_to_63, // r40 — reserved for ripple, which needed no model change
    migrate_63_to_64, // r40 — reserved for delay, whose new parameters live in the unit's state
    migrate_64_to_65, // r40:cast — Fuga written into dancers that named no character
    migrate_65_to_66, // r40:p3 — an instrument's named outputs (ADR 0123)
    migrate_66_to_67, // r40:p3 — Tympanum, the acoustic drummer (ADR 0124)
    migrate_67_to_68, // r40:p3 — Chorda's recorded source (ADR 0125)
    migrate_68_to_69, // r40:scene — the scene's room, floor, walls and rug (ADR 0126)
    migrate_69_to_70, // r40:tymp-ui — a strip added to an instrument's family (ADR 0123)
    migrate_70_to_71, // r40:drums-hyb — a modelled kit's voice, generated or hybrid (ADR 0124)
    migrate_71_to_72, // fx — the side round's five kinds (ADRs 0127–0130)
    migrate_72_to_73, // fx3 — Tabula's board and three kinds (ADRs 0131–0134)
    migrate_73_to_74, // fxa — Fractus's lanes and three kinds (ADRs 0136–0138)
    migrate_74_to_75, // fxb — Cedo's drawn shape and three kinds (ADRs 0139–0141)
    migrate_75_to_76, // fxc — Purus, Comparatio and its references (ADRs 0142–0143)
    migrate_76_to_77, // fxd — the X-Y pad and the formula controller (ADRs 0144–0145)
    migrate_77_to_78, // r41:contract — Fides, Nitor, Fragor's five pedals (ADRs 0150–0152)
    migrate_78_to_79, // r41:fides-input — an instrument's sidechain (ADR 0154)
    migrate_79_to_80, // r42:contract — the project's and the tracks' tuning (ADR 0155)
    migrate_80_to_81, // r42:contract — Arcus and the note's articulation (ADRs 0157, 0158)
    migrate_81_to_82, // r42:strings-rec — Exemplar's articulations and legato (ADR 0158)
    migrate_82_to_83, // r42 arcus-expr — a note's bow start, bow part and string (ADR 0162)
    migrate_83_to_84, // r42:chorda-c17 — Chorda's colour (ADR 0098, sixth amendment)
    migrate_84_to_85, // r42 fiddle — Arcus's strings under one bow (ADR 0164)
    migrate_85_to_86, // r42 — Tympanum's model 6 (its own branch; a placeholder here)
    migrate_86_to_87, // r42 arcus-classical — Arcus's fiddler as its own switch (ADR 0166)
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

/// Schema 28 (round 28, ADR 0057): modulators on tracks, strips and the master
/// (`Track::modulators`, `MixerStrip::modulators`, `Master::modulators`) and automation of a
/// hosted plug-in's parameters (`AutoTarget::Plugin`, `PluginInstrument`, `ModAmount`). An
/// older build must refuse such a file rather than play it unmodulated; no older project
/// holds one, so the step writes nothing.
fn migrate_27_to_28(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 29 (round 28, ADR 0058): comp seams' crossfades (`AudioRegion::xfade`),
/// alternate comps (`Track::comps`, `active_comp`), the record options' seam default and
/// automation switch (`RecordSettings::comp_xfade_ms`, `record_automation`). A file without
/// them butt-joins its comp windows, has the one comp lane 0 holds, a 10 ms default and
/// records automation — what every older version would do — so the step writes nothing.
fn migrate_28_to_29(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 30 (round 28, sound, ADR 0042/0023): Exemplar's round robin plays **in order**
/// (`rr.order` = 1, the default: a
/// clip's note takes its turn by its ordinal on the track, so a roll never repeats a sample
/// and a render from any point cycles as the whole one does). Every Exemplar saved before
/// drew its turns from the notes' ids: its `rr.order` is written as 0, so it renders to the
/// same bits it did; a value already there is kept. The same step brings Exemplar's `kit`
/// (a drum kit's rows in the piano roll and the channel rack): absent, an Exemplar is a
/// keyboard, as every older one was, so it writes nothing for it.
fn migrate_29_to_30(value: &mut Value) -> Result<(), String> {
    write_sampler_default(value, "rr.order", 0.0)
}

/// Write `key` = `v` into every Exemplar's parameters that lacks it.
fn write_sampler_default(value: &mut Value, key: &str, v: f64) -> Result<(), String> {
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
        if !native || inst.get("kind").and_then(Value::as_str) != Some("Sampler") {
            continue;
        }
        let params = inst
            .entry("params")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(params) = params.as_object_mut() else {
            return Err("an Exemplar's params are not a map".into());
        };
        params.entry(key).or_insert_with(|| Value::from(v));
    }
    Ok(())
}

/// Schema 31 (round 28, review 2 of ADR 0054): a pitch note says whether its pitch is
/// *Correct pitch*'s (`PitchNote::corrected`), so the macro re-targets it after a key change
/// and never takes back a hand move. A file without it has every pitch as the hand's — an
/// older build would have guessed from the key — and no project with pitch edits has
/// shipped, so the step writes nothing; the number makes an older build refuse a file
/// whose macro notes it would misread.
fn migrate_30_to_31(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 33 (round 29, tempo — ADR 0060): the tempo track (`Project::tempo_points`, each
/// point holding or ramping), the metre changes (`metre_changes`) and the named markers
/// (`markers`). A file without them has one tempo, one metre and no markers — what every
/// older file meant — so the step writes nothing; the number makes an older build refuse a
/// file whose tempo changes it would play at one tempo.
fn migrate_32_to_33(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 32 (round 28, fix4 of ADR 0058): a comp window says it is one (`Clip::comp`)
/// instead of being recognised by where it lies — a copy pasted over its takes was taken
/// for a window. The step writes the flag on every clip schema 31 counted as a comp
/// window, so an old project comps exactly as before: a playing-lane clip that shows a
/// take's content (its audio source, or its first note's id) and lies over that take on
/// the timeline — where the take put it (the same content origin, `start − offset`), or
/// slipped or nudged (an origin no take of that content has) — and every window an
/// alternate comp keeps. Raw JSON, so the step reads a v31 file whatever the model
/// becomes.
fn migrate_31_to_32(value: &mut Value) -> Result<(), String> {
    let tracks = value
        .get_mut("project")
        .and_then(|p| p.get_mut("tracks"))
        .and_then(Value::as_array_mut)
        .ok_or("missing tracks")?;
    for t in tracks {
        if let Some(clips) = t.get_mut("clips").and_then(Value::as_array_mut) {
            let keys: Vec<Option<WindowKey>> = clips.iter().map(WindowKey::of).collect();
            let windows: Vec<bool> = (0..clips.len())
                .map(|i| was_comp_window(&keys, i))
                .collect();
            for (c, w) in clips.iter_mut().zip(windows) {
                if let (true, Some(o)) = (w, c.as_object_mut()) {
                    o.insert("comp".into(), Value::Bool(true));
                }
            }
        }
        if let Some(comps) = t.get_mut("comps").and_then(Value::as_array_mut) {
            for comp in comps {
                let kept = comp.get_mut("windows").and_then(Value::as_array_mut);
                for w in kept.into_iter().flatten() {
                    if let Some(o) = w.as_object_mut() {
                        o.insert("comp".into(), Value::Bool(true));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Schema 34 (round 29, ADR 0061): an audio clip may carry a warp (`AudioRegion::warp`:
/// its markers, mode, transposition and tempo). An older build must refuse such a file,
/// not play the clip at its natural speed; no older project holds one, so the step writes
/// nothing — a clip without a warp plays as it always did.
fn migrate_33_to_34(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 35 (round 29, controllers — ADR 0062): the project keeps its MIDI controller
/// mappings (`Project::controllers`: each a port by name and ordinal, a channel, a
/// control; a target; a mode, a range, a bend, a takeover, feedback). A file without them
/// had none, so the step writes nothing; the number makes an older build refuse a file
/// whose knobs it would silently forget.
fn migrate_34_to_35(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 36 (round 29, ADR 0063): a hosted plug-in records where it runs —
/// `ExternalPlugin::isolation`, *Within Cantus*, *Together*, *By vendor* or
/// *Individually*, absent while it follows the machine's default. Nothing to write: a v35
/// plug-in follows the default, as it did. An older build must refuse a v36 file, or a
/// plug-in the user moved out of process would load back into Cantus's own.
fn migrate_35_to_36(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 37 (round 29, ADR 0064): a mixer strip or a track may be frozen —
/// `MixerStrip::frozen` / `Track::frozen`, the content key its render was made for, absent
/// while it plays live. Nothing to write: a v36 strip or track plays live, as it did. An
/// older build must refuse a v37 file, or it would play a frozen strip's chain live and let
/// edits through that the render no longer matches.
fn migrate_36_to_37(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 38 (round 29, ADR 0064 §6, review 2): a track may carry the latency its audio
/// was rendered with — `Track::lateness`, set by a bounce in place, which keeps its source's
/// or its chain's latency in its file so the level, the fader, the sends and the keys meet
/// the audio as late as before. Nothing to write: a v37 track's audio is on time. An older
/// build must refuse a v38 file, or it would play a bounced track early.
fn migrate_37_to_38(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 39 (round 29, warp's second review — ADR 0061 §11): a warp marker may be the
/// warp's own (`WarpMarker::auto`: a sample of the tempo curve a first warp edit keeps the
/// clip on — never drawn or grabbed, soft for the stretcher). Nothing to write: every v38
/// marker is the user's, as it was. An older build must refuse a v39 file, or it would show
/// a ramp's samples as markers to drag and pin the stretcher to them.
fn migrate_38_to_39(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 40 (round 29, ADR 0062 review 3): a controller mapping may drive a strip's
/// effects switch (`MappingTarget::StripFx`) or its record arm (`StripArm`) — the mixer's FX
/// bulb and record dot. Nothing to write: a v39 map has neither. An older build must
/// refuse a v40 file, or it would drop those mappings as unreadable. (Numbered 39 on the
/// controllers branch; renumbered at the merge after warp's 39.)
fn migrate_39_to_40(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 41 (round 29, ADR 0062 review 4, #47): a hosted mapping's plug-in carries the
/// names it had when learnt (`PluginRef::name`, `param`), so a row whose plug-in was
/// swapped out or is not scanned here reads as the plug-in and parameter it drove. Nothing
/// to write: a v40 mapping has none, and its row is named as before (by the catalogue).
fn migrate_40_to_41(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 42 (round 31, ADR 0068): a note may carry a release velocity (`Note::release`), a
/// note clip controller lanes (`Clip::lanes`: pitch bend, pressure, CCs), and a track and
/// the master an automation mode (`auto_mode`: Read, Write, Touch, Latch). Nothing to write:
/// a v41 note has no release (0, what its note-off carried), a clip no lanes, and every
/// mode is Read — the project plays and writes exactly as before. An older build must
/// refuse a v42 file, or it would drop the lanes and the releases on its next save.
fn migrate_41_to_42(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 43 (round 31, ADR 0069): note effects on a track (`Track::note_fx`), the song's
/// key and the groove pool (`Project::key`, `grooves`), a clip's groove and whether it
/// plays past the note effects (`Clip::groove`, `skip_note_fx`). Nothing to write: every
/// field defaults to what an older file meant — no effects, no key, no groove.
fn migrate_42_to_43(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 44 (round 31, ADR 0070): an audio clip's crossfade over an overlap
/// (`AudioRegion::cross`) and its gain envelope (`AudioRegion::gain_env`). Nothing to
/// write: an older file has neither — its overlaps play summed, its clips at their gain.
fn migrate_43_to_44(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 45 (round 32, ADR 0075): a track's score settings (`Track::score`: clef,
/// staves, the hands' split, transposition, dynamics, reading grid, the part's name).
/// Nothing to write: a v44 track has none — everything automatic, as the score view first
/// shows it. An older build must refuse a v45 file, or it would drop them on its next save.
fn migrate_44_to_45(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 46 (round 33, ADR 0076): the session view — the scenes and the session's
/// settings (`Project::session`) and each track's column of slots (`Track::slots`). Nothing
/// to write: a v45 project has no session, and its tracks no slots. An older build must
/// refuse a v46 file, or it would drop the session on its next save.
fn migrate_45_to_46(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 47 (round 33, ADR 0080): per-note expression (`Note::expr`: bend, pressure and
/// timbre curves), a hosted plug-in's expression route (`ExternalPlugin::expression`).
/// Nothing to write: a v46 note has none and plays as it did; a plug-in's route is
/// automatic. An older build must refuse a v47 file, or it would drop the curves on its
/// next save.
fn migrate_46_to_47(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 48 (round 34, ADR 0085): `provenance`, beside the audio pool — what made a file
/// a model made. Absent in every older file: nothing to write.
fn migrate_47_to_48(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 49 (round 35, ADR 0087): the dancer — `InstrumentKind::Dancer` (a native
/// instrument that makes no sound; its numbers, its `character` text key) and the
/// `TrackIcon::Dance` picture. Nothing to write: no older file holds either. An older build
/// must refuse a v49 file, which it could not read.
fn migrate_48_to_49(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 50 (round 36, ADR 0093): VCA faders and mixer snapshots — `MixerStrip::kind`,
/// `vcas` and `recall_safe`, and `Project::mix` (snapshots, cues, the VCA of each group).
/// Nothing to write: no older file holds any of them, and every one is skipped when it is
/// the default. An older build must refuse a v50 file, which it could not read.
fn migrate_49_to_50(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 51 (round 36, ADR 0094): `external_units` — the hardware that is part of a track
/// (an external instrument's MIDI out and audio return, an external effect's send and
/// return, the measured latency). Absent in every older file: nothing to write. An older
/// build must refuse a v51 file, or it would drop the routing and the measurement on its
/// next save. r36:external
fn migrate_50_to_51(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 52 (round 36, ADR 0096 §6): the audio editor's markers, regions and loop points,
/// `Project::source_marks`. Nothing to write: no older file holds any. An older build must
/// refuse a v52 file, which it could not read. // r36:editor
fn migrate_51_to_52(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 53 (round 36, ADR 0097): an audio clip bound to an ARA 2 plug-in —
/// `AudioRegion::ara` (the plug-in, and its archive of the clip's modification). Nothing to
/// write: no older file binds a clip. An older build must refuse a v53 file, which it could
/// not play as saved. // r36:ara
fn migrate_52_to_53(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 54 (round 37, ADR 0098): Chorda's second model. A new instance plays model 2;
/// a spec that says no model meant the first one, so every Chorda saved before gets
/// `model` = 1 written into its parameters — it renders to the same bits it did — and a
/// value already there is kept. // r37:piano
fn migrate_53_to_54(value: &mut Value) -> Result<(), String> {
    write_native_default(value, "Chorda", "model", 1.0)
}

/// Schema 59 (round 37, ADR 0105): the cast's default character is now Fuga the ninja. A
/// dancer that named no character showed the default — the songbird, before — so every
/// dancer saved without one gets `character` = `builtin:songbird` written into its text,
/// and a song keeps the dancer it had; a character already named is kept. // r37:chars
/// Schema 60 (round 39, ADR 0112): Aula, the convolution reverb — a new effect kind, and an
/// effect's optional `ir` (a response from a file). Nothing older has either, so nothing is
/// written: an older file reads as it did. // r39:conv
fn migrate_59_to_60(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 61 (round 39, ADR 0113): the project gains `sections` and `folders`, a track its
/// `folder` and its lanes' own modes (`lane_modes`), every one defaulting to none — what an
/// older project means. Nothing to write. // r39:arrange
fn migrate_60_to_61(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 62 (round 40, ADR 0118): a track's `record_tap`, where its recording takes its
/// signal — before its strip's inserts (the default, what every older track did) or after
/// them. Nothing older has it: nothing to write. // r40:pads
fn migrate_61_to_62(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schemas 63 and 64 were reserved for round 40's `ripple` and `delay`; neither changed the
/// model (ADRs 0116, 0119), so both steps write nothing.
fn migrate_62_to_63(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

fn migrate_63_to_64(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 65 (round 40, ADR 0122): the cast's default character is now Compás, the Slate
/// pictogram. A dancer that named no character showed the default — Fuga the ninja, since
/// schema 59 — so every dancer saved without one gets `character` = `builtin:ninja` written
/// into its text, and a song keeps the dancer it had; a character already named is kept.
/// // r40:cast
fn migrate_64_to_65(value: &mut Value) -> Result<(), String> {
    write_default_character(value, "builtin:ninja")
}

/// Schema 66 (round 40 phase 3, ADR 0123): a native instrument's named outputs — its
/// `outputs` (the ones that play somewhere other than Main) and its internal mixer's
/// `o{n}.` keys — and the `SetInstrumentOutput` command. Nothing older has either: every
/// output plays on Main, every key reads its null default. Nothing to write. // r40:p3
/// (`tymp-engine` owns this step: if it must write, say so to the integrator.)
fn migrate_65_to_66(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 67 (round 40 phase 3, ADR 0124): Tympanum, `InstrumentKind::AcousticDrums` —
/// its kit (`kit`), its pieces', mics' and bleed cells' keys. A schema-66 build cannot
/// read a song that hosts one, and the version says so. Nothing older has one: nothing to
/// write. // r40:p3 (`tymp-engine`)
fn migrate_66_to_67(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 68 (round 40 phase 3, ADR 0125): Chorda's source (`source`; absent, the model)
/// and a recorded piano's zones under `rec.`. Every older Chorda plays its model: nothing
/// to write. // r40:p3 (`chorda-src`)
fn migrate_67_to_68(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 69 (round 40 phase 3, ADR 0126 amended): the room a scene stands in and its
/// floor, walls and rug (`look.room`, `look.floor`, `look.walls`, `look.rug`), text keys
/// kept with the instrument. An older song has none: its Chorda keeps the room its Room
/// size drew and the room's own surfaces — nothing to write. // r40:scene
fn migrate_68_to_69(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 70 (round 40 phase 3, ADR 0123 and ADR 0016 amended): a strip may be added to an
/// instrument's family on the desk (`MixerStrip::attached`, the instrument's track): a
/// strip fed only by its strips, drawn at the end of its band. An older song has none —
/// nothing to write. // r40:tymp-ui
fn migrate_69_to_70(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 71 (round 40, ADR 0124's third amendment): a Tympanum's `voice` text key —
/// `hybrid` plays a recorded residual under a modelled kit; absent, generated. Every
/// older Tympanum is generated: nothing to write. // r40:drums-hyb
fn migrate_70_to_71(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 72 (the effects side round, ADRs 0127–0130; provisional 62 on its branch): five new effect kinds — Cribrum, Fragor,
/// Lapsus, Vetus and Libra. Nothing older has any of them, so nothing is written.
// fx
fn migrate_71_to_72(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 73 (the effects side round's wave 3, ADRs 0131–0134; provisional 63): Tabula's board
/// (`EffectSpec::board`, its pedals and macros) and three new effect kinds — Saltus, Rota and
/// Arca. Nothing older has any of them, so nothing is written.
// fx3
fn migrate_72_to_73(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 74 (the effects side round's wave A, ADRs 0136–0138; provisional 64): Fractus's painted lanes
/// (`EffectSpec::fractus`) and three new effect kinds — Loquax, Fractus and Caelum. Nothing
/// older has any of them, so nothing is written.
// fxa
fn migrate_73_to_74(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 75 (the effects side round's wave B, ADRs 0139–0141; provisional 65): Cedo's drawn
/// shape (`EffectSpec::cedo`) and three new effect kinds — Vinculum, Lenis and Cedo. Nothing
/// older has any of them, so nothing is written.
// fxb
fn migrate_74_to_75(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 76 (the effects side round's wave C, ADRs 0142–0143; provisional 66): two new effect
/// kinds — Purus (repair) and Comparatio (the reference comparator, whose reference track is a
/// file by path in `EffectSpec::ir`, the field Aula's response has used since schema 60, and
/// its list of references in `EffectSpec::refs`, each with the size and hash it was loaded
/// with and its own playback). Nothing older has either, so nothing is written.
// fxc
fn migrate_75_to_76(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 77 (the effects side round's wave D, ADRs 0144–0145; provisional 67): two modulator sources — the
/// X-Y pad (its puck, inertia, path and the links of its routes) and the formula controller
/// (its text, knobs and links) — and the automation target of their inputs
/// (`AutoTarget::ModInput`). Nothing older has any of them, so nothing is written.
// fxd
fn migrate_76_to_77(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 78 (round 41, ADRs 0150–0152): three new kinds' names — the instrument
/// `Resonator` (Fides), the effect `Enhancer` (Nitor) and Fragor's five circuits as
/// board-only pedals (`FragorOverdrive` … `FragorMuff`), which an older build cannot read.
/// Nothing older holds any of them, so nothing is written.
// r41:contract
fn migrate_77_to_78(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 79 (round 41, ADR 0154): a track's `sidechain`, the strip its instrument hears as
/// its input (Fides's *Input* exciter). Absent — every older track — is none, which is what
/// they heard, so nothing is written; an older build would drop the field and silence it.
// r41:fides-input
fn migrate_78_to_79(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 80 (round 42, ADRs 0155, 0156): the project's `tuning` and each track's, an
/// external unit's `mts` (MTS SysEx sent to it, `ExternalUnit::mts`) and a hosted plug-in's
/// `tuning_by` (how it is told the tuning, `ExternalPlugin::tuning_by`). Absent — every older
/// song — is 12-TET at 440 Hz, followed by every track, no MTS SysEx and MTS-ESP as the route,
/// which is what they played, so nothing is written; an older build would drop them and play
/// the song in 12-TET.
// r42:contract
fn migrate_79_to_80(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 81 (round 42, ADRs 0157, 0158): the instrument `BowedString` (Arcus) and a note's
/// `art` (its articulation and bow). Nothing older holds either, so nothing is written.
// r42:contract
fn migrate_80_to_81(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 82 (round 42, ADR 0158): an Exemplar zone's articulation, bow and controller
/// crossfade (`z{n}.art`, `.bow`, `.xfcc`, `.xfinlo` … `.xfcurve`) and the instrument's scripted
/// legato (`legato`, `legato.xf`, `legato.glide`, `legato.skip`). Absent — every older song —
/// is what they played (any articulation, any bow, no crossfade, no legato), so nothing is
/// written; an older build would drop them and play every zone of a key-switched strings
/// instrument by its keyswitch alone.
// r42:strings-rec
fn migrate_81_to_82(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 83 (round 42, ADR 0162): a note's `art` may say where the bow comes from
/// (`start`), which part of it plays (`part`) and the string (`string`). Absent — every older
/// note — lets the instrument's player decide, so nothing is written; an older build would
/// drop them and play the note as its player chooses.
// r42 arcus-expr
fn migrate_82_to_83(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 84 (round 42, ADR 0098's sixth amendment): Chorda's `colour` (−1 … 1, model 2's
/// colour toward a recorded grand's middle). Absent — every older song — reads 0, Chorda's own
/// voicing, so nothing is written; an older build drops it and plays the song uncoloured.
// r42:chorda-c17
fn migrate_83_to_84(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 85 (round 42, ADR 0164): an Arcus may bow two strings with one bow (`bow.pair`,
/// `bow.tilt`, `dstop.just`), its open strings ring in sympathy (`sym`), its bow has grit
/// (`bow.grit`), its strings may be steel (`steel`) and cross-tuned (`tuning.strings`), and
/// its body heard by one close microphone (`body.mic`) through a fiddle's bridge
/// (`body.bridge`). Absent — every older song — is each
/// one's default, which plays what it played (one string a voice, no sympathy, no grit, the
/// measured set), so nothing is written; an older build would drop them and play the song as
/// before them.
// r42 fiddle
fn migrate_84_to_85(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 86 (round 42): Tympanum's model 6 — the step is that branch's; on this branch a
/// placeholder that writes nothing (the integrator keeps the real one).
fn migrate_85_to_86(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 87 (round 42, ADR 0166): an Arcus's player is a fiddler by its own switch
/// (`player.fiddle`, 0 the classical player … 1 a fiddler); before it a steel set (`steel`)
/// stood for it. Every Arcus whose `steel` is above 0 and that names no `player.fiddle` gets
/// `player.fiddle` = its `steel`, so it plays as it did; one without steel reads 0, the
/// classical player it had. An older build drops the key and reads the steel set as before.
// r42 arcus-classical
fn migrate_86_to_87(value: &mut Value) -> Result<(), String> {
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
        if !native || inst.get("kind").and_then(Value::as_str) != Some("BowedString") {
            continue;
        }
        let Some(params) = inst.get_mut("params").and_then(Value::as_object_mut) else {
            continue;
        };
        let steel = params.get("steel").and_then(Value::as_f64).unwrap_or(0.0);
        if steel > 0.0 && !params.contains_key("player.fiddle") {
            params.insert("player.fiddle".into(), Value::from(steel.min(1.0)));
        }
    }
    Ok(())
}

/// Write `character` = `id` into every dancer's text that names none (schemas 59 and 65:
/// the default character changed, an old song keeps the one it showed).
fn write_default_character(value: &mut Value, id: &str) -> Result<(), String> {
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
        if !native || inst.get("kind").and_then(Value::as_str) != Some("Dancer") {
            continue;
        }
        let text = inst
            .entry("text")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(text) = text.as_object_mut() else {
            return Err("a dancer's text is not a map".into());
        };
        text.entry("character").or_insert_with(|| Value::from(id));
    }
    Ok(())
}

fn migrate_58_to_59(value: &mut Value) -> Result<(), String> {
    write_default_character(value, "builtin:songbird")
}

/// Write `key` = `v` into the parameters of every native instrument of `kind` that lacks
/// it. // r37:piano
fn write_native_default(value: &mut Value, kind: &str, key: &str, v: f64) -> Result<(), String> {
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
        if !native || inst.get("kind").and_then(Value::as_str) != Some(kind) {
            continue;
        }
        let params = inst
            .entry("params")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(params) = params.as_object_mut() else {
            return Err(format!("a {kind}'s params are not a map"));
        };
        params.entry(key).or_insert_with(|| Value::from(v));
    }
    Ok(())
}

/// Schema 55 (round 37, ADR 0099): Sectio, Exemplar's slicer — a sliced loop's points and
/// settings among an Exemplar's parameters (`sl.n`, `sl.p{i}`, `sl.len`, `sl.rate`,
/// `sl.bpm`, `sl.mode`, `sl.sens`, `sl.grid`) and a zone's own low-pass (`z{n}.lpf`). Nothing
/// to write: no older file holds any of them, and an Exemplar without them is not sliced and
/// plays as it did. An older build must refuse a v55 file, which would lose the slices and
/// the slices' filters on its next save. // r37:piano
fn migrate_54_to_55(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 56 (round 37, ADR 0100): Pulsus III — a pad's `output` is heard on its strip (it
/// was stored and ignored), a strip's own `pan_law`, kit model 3. Nothing to write: every
/// older kit keeps its model (1 or 2) and sums in voice order, bit for bit as it did; no
/// older file sets a strip's law. An older build must refuse a v56 file: it would play a
/// split kit's pads on the kit's strip and a pad strip through the project's law. // r37:drums
fn migrate_55_to_56(_value: &mut Value) -> Result<(), String> {
    Ok(())
}

/// Schema 57 (round 37, ADR 0100 §8): a kit's own gain (`KitSettings::gain_db`), the stage
/// the factory kits are calibrated with. Nothing to write: an older kit has none and plays at
/// 0 dB, exactly as it did. An older build must refuse a v57 file: it would play a
/// calibrated kit at 0 dB. // r37:drums
fn migrate_56_to_57(value: &mut Value) -> Result<(), String> {
    // A strip a pad of a kit plays on, made by schema 56's *Split to strips* (its own pan
    // law, audio): a pad strip now (`StripKind::Pad`), so it goes with its kit.
    let project = value
        .get_mut("project")
        .ok_or_else(|| "no project".to_string())?;
    let mut pad_strips: Vec<u64> = Vec::new();
    if let Some(tracks) = project.get("tracks").and_then(Value::as_array) {
        for t in tracks {
            let pads = t
                .get("instrument")
                .filter(|i| i.get("type").and_then(Value::as_str) == Some("Pulsus"))
                .and_then(|i| i.get("pads"))
                .and_then(Value::as_array);
            for pad in pads.into_iter().flatten() {
                if let Some(o) = pad.get("output").and_then(Value::as_u64) {
                    pad_strips.push(o);
                }
            }
        }
    }
    if let Some(strips) = project.get_mut("strips").and_then(Value::as_array_mut) {
        for s in strips {
            let id = s.get("id").and_then(Value::as_u64);
            let audio = s.get("kind").is_none_or(|k| k == "Audio");
            let own_law = s.get("pan_law").is_some();
            if id.is_some_and(|i| pad_strips.contains(&i)) && audio && own_law {
                s["kind"] = Value::from("Pad");
            }
        }
    }
    Ok(())
}

/// Schema 58 (round 37, ADR 0104): Chorda's look — the piano its scene draws, two text
/// keys (`look.body`, `look.finish`) that no sample reads. Every Chorda saved before gets the
/// look it was always shown with (a black concert grand) written in, so a later default never
/// changes the picture of a song made before it; a look already there is kept. // r37:scene
fn migrate_57_to_58(value: &mut Value) -> Result<(), String> {
    write_native_text_default(value, "Chorda", "look.body", "concert")?;
    write_native_text_default(value, "Chorda", "look.finish", "black")
}

/// Write the text `key` = `v` into every native instrument of `kind` that lacks it.
/// // r37:scene
fn write_native_text_default(
    value: &mut Value,
    kind: &str,
    key: &str,
    v: &str,
) -> Result<(), String> {
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
        if !native || inst.get("kind").and_then(Value::as_str) != Some(kind) {
            continue;
        }
        let text = inst
            .entry("text")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(text) = text.as_object_mut() else {
            return Err(format!("a {kind}'s text is not a map"));
        };
        text.entry(key).or_insert_with(|| Value::from(v));
    }
    Ok(())
}

/// What schema 31 read of a clip to tell a comp window: its lane, its stretch of the
/// timeline, what it shows and that content's origin on the timeline.
#[derive(Copy, Clone, PartialEq)]
struct WindowKey {
    lane: u64,
    start: i64,
    end: i64,
    content: u64,
    audio: bool,
    origin: i64,
}

impl WindowKey {
    fn of(c: &Value) -> Option<WindowKey> {
        let int = |k: &str| c.get(k).and_then(Value::as_i64).unwrap_or(0);
        let (start, length, offset) = (int("start"), int("length"), int("offset"));
        let (content, audio) = match c.get("audio").filter(|a| !a.is_null()) {
            Some(a) => (a.get("source").and_then(Value::as_u64)?, true),
            None => (
                c.get("notes")?.as_array()?.first()?.get("id")?.as_u64()?,
                false,
            ),
        };
        Some(WindowKey {
            lane: c.get("lane").and_then(Value::as_u64).unwrap_or(0),
            start,
            end: start + length,
            content,
            audio,
            origin: start - offset,
        })
    }

    fn shows(&self) -> (u64, bool, i64) {
        (self.content, self.audio, self.origin)
    }
}

/// Schema 31's rule (`comping::shows_take`, fix3): clip `i` is on the playing lane and a
/// window onto some take of its content it lies over.
fn was_comp_window(keys: &[Option<WindowKey>], i: usize) -> bool {
    let Some(c) = keys[i] else {
        return false;
    };
    if c.lane != 0 {
        return false;
    }
    let takes = || keys.iter().flatten().filter(|t| t.lane > 0);
    let at_a_takes_origin = takes().any(|t| t.shows() == c.shows());
    takes().any(|t| {
        (t.content, t.audio) == (c.content, c.audio)
            && c.start < t.end
            && c.end > t.start
            && (t.origin == c.origin || !at_a_takes_origin)
    })
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

    // r37:drums
    /// Schema 57 (round 37, ADR 0100 §8): the fixture's Drums kit keeps its own gain; an
    /// older file (56) opens with none (0 dB).
    #[test]
    fn schema_57_keeps_a_kits_own_gain() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let gain = |f: &str| {
            let text = std::fs::read_to_string(dir.join(f)).unwrap();
            let p = parse(&text, true).unwrap().file.project;
            let t = p.tracks.iter().find(|t| t.name == "Drums").unwrap();
            t.instrument.pulsus().unwrap().kit.gain_db
        };
        assert_eq!(gain("v57.cantus"), 3.5);
        assert_eq!(gain("v56.cantus"), 0.0);
        // A v56 file's pad strips are pad strips after the migration.
        let text = std::fs::read_to_string(dir.join("v56.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let t = p.tracks.iter().find(|t| t.name == "Drums").unwrap();
        for s in t.instrument.output_strips() {
            assert_eq!(p.strip(s).unwrap().kind, cantus_core::vca::StripKind::Pad);
        }
    }

    // r37:drums
    /// Schema 56 (round 37, ADR 0100): the fixture's Drums kit comes back split — each pad
    /// on its own strip, into the kit's, with the 0 dB law, one fader down — and an older
    /// file (53) opens with its kit whole, its model as it was, no strip with a law.
    #[test]
    fn schema_56_keeps_a_kit_split_to_strips() {
        use cantus_core::PanLaw;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v56.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let t = p.tracks.iter().find(|t| t.name == "Drums").unwrap();
        let outs = t.instrument.output_strips();
        assert!(outs.len() >= 5, "the kit is split: {outs:?}");
        for s in &outs {
            let st = p.strip(*s).expect("its strip");
            assert_eq!(st.output, t.strip);
            assert_eq!(st.pan_law, Some(PanLaw::ZeroDb));
        }
        assert_eq!(p.strip(outs[0]).unwrap().gain_db, -3.0);
        let old = std::fs::read_to_string(dir.join("v53.cantus")).unwrap();
        let q = parse(&old, true).unwrap().file.project;
        let t = q.tracks.iter().find(|t| t.name == "Drums").unwrap();
        assert!(t.instrument.output_strips().is_empty());
        assert!(t.instrument.pulsus().unwrap().version < cantus_core::PULSUS_MODEL);
        assert!(q.strips.iter().all(|s| s.pan_law.is_none()));
    }

    /// Schema 25 (round 28): the fixture holds one insert of each of the thirteen new
    /// kinds — the transient shaper keyed, the tuner hearing notes — and a Chorda track;
    /// an older file (24) opens with none of them.
    #[test]
    fn schema_25_keeps_the_round_28_kinds() {
        use cantus_core::{EffectKind as K, InstrumentKind};
        let new_kinds = &K::ALL[12..25]; // r39:conv: round 28's kinds (Aula came later)
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

    /// fx:contract — Schema 72 (the effects side round): the fixture holds one insert
    /// of each of the five new kinds, in order, on the second strip; every older file
    /// (71) opens with none of them.
    #[test]
    fn schema_72_keeps_the_fx_side_rounds_kinds() {
        use cantus_core::EffectKind as K;
        let new_kinds = &K::ALL[26..31];
        assert_eq!(
            new_kinds,
            [K::AutoFilter, K::Pedal, K::TimeShaper, K::LoFi, K::Utility]
        );
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v72.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let slots: Vec<_> = p.strips[1]
            .effects
            .iter()
            .filter(|e| new_kinds.contains(&e.spec.kind))
            .collect();
        let kinds: Vec<K> = slots.iter().map(|e| e.spec.kind).collect();
        assert_eq!(kinds, new_kinds.to_vec());
        assert_eq!(slots[0].spec.get("out"), -3.0);
        assert!(slots[1..].iter().all(|e| e.spec.get("out") == 0.0));
        let old = std::fs::read_to_string(dir.join("v71.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .strips
            .iter()
            .flat_map(|s| s.effects.iter())
            .all(|e| !new_kinds.contains(&e.spec.kind)));
    }

    /// fx3:contract — Schema 73 (the effects side round's wave 3): the fixture holds
    /// a board — Fragor on A and Delay on B (a split), a macro named Drive moving Fragor's
    /// drive — and one insert of each of the three new kinds; every older file (72) opens
    /// with none of them and no board.
    #[test]
    fn schema_73_keeps_a_board_and_wave_3s_kinds() {
        use cantus_core::fx::tabula::{Path, PedalId};
        use cantus_core::EffectKind as K;
        let new_kinds = &K::ALL[31..35];
        assert_eq!(new_kinds, [K::Board, K::PitchShift, K::Rotary, K::Amp]);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v73.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // fxa:contract — no longer the newest: migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(73));
        let p = report.file.project;
        let slots: Vec<_> = p.strips[1]
            .effects
            .iter()
            .filter(|e| new_kinds.contains(&e.spec.kind))
            .collect();
        let kinds: Vec<K> = slots.iter().map(|e| e.spec.kind).collect();
        assert_eq!(kinds, new_kinds.to_vec());
        let board = slots[0].spec.board.as_deref().expect("a board");
        let pedals: Vec<(K, Path)> = board.pedals.iter().map(|x| (x.spec.kind, x.path)).collect();
        assert_eq!(pedals, [(K::Pedal, Path::A), (K::Delay, Path::B)]);
        assert!(board.has_split());
        assert_eq!(board.macros[0].name.as_deref(), Some("Drive"));
        assert_eq!(board.macro_of(PedalId(0), "od.drive"), Some(0));
        assert_eq!(slots[0].spec.get("m1"), 0.25);
        assert_eq!(
            slots[0].spec.get("p1.time_ms"),
            250.0,
            "a pedal's key, routed"
        );
        assert!(slots[1..].iter().all(|e| e.spec.get("out") == 0.0));
        let old = std::fs::read_to_string(dir.join("v72.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .all_effects()
            .all(|(_, e)| !new_kinds.contains(&e.spec.kind) && e.spec.board.is_none()));
    }

    /// r41:contract — Schema 78 (round 41): the fixture holds a Fides track, a Nitor with its
    /// key fixed and its output set, and a board of Fragor's five circuits as pedals; every
    /// older file (77) opens with none of them, and the new names round-trip.
    #[test]
    fn schema_78_keeps_fides_nitor_and_fragor_s_five_pedals() {
        use cantus_core::EffectKind as K;
        use cantus_core::InstrumentKind;
        let new_kinds = [
            K::Enhancer,
            K::FragorOverdrive,
            K::FragorOpAmp,
            K::FragorSilicon,
            K::FragorFuzz,
            K::FragorMuff,
        ];
        assert_eq!(&K::ALL[K::ALL.len() - 6..], new_kinds);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v78.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // r41:fides-input — no longer the newest (79): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(78));
        let p = report.file.project;
        let fides = p
            .tracks
            .iter()
            .find(|t| t.name == "Fides")
            .and_then(|t| t.instrument.native())
            .expect("the Fides track");
        assert_eq!(fides.kind, InstrumentKind::Resonator);
        assert_eq!(fides.get("gain"), -3.0);
        let nitor = p
            .all_effects()
            .find(|(_, e)| e.spec.kind == K::Enhancer)
            .map(|(_, e)| e)
            .expect("the Nitor");
        assert_eq!((nitor.spec.get("key"), nitor.spec.get("out")), (8.0, -1.5));
        let board = p
            .all_effects()
            .filter_map(|(_, e)| e.spec.board.as_deref())
            .find(|b| b.pedals.iter().any(|q| q.spec.kind == K::FragorOverdrive))
            .expect("the board of five");
        let kinds: Vec<K> = board.pedals.iter().map(|q| q.spec.kind).collect();
        assert_eq!(kinds, new_kinds[1..].to_vec());
        let old = std::fs::read_to_string(dir.join("v77.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .all_effects()
            .all(|(_, e)| !new_kinds.contains(&e.spec.kind)));
        assert!(p.tracks.iter().all(|t| t
            .instrument
            .native()
            .is_none_or(|n| n.kind != InstrumentKind::Resonator)));
    }

    /// r42:contract — Schema 80 (ADR 0155): the fixture's project plays in Werckmeister III in
    /// E flat at 415 Hz, its first track in equal temperament, its second in its own 31-EDO;
    /// it round-trips; every older file (79) opens in 12-TET at 440, every track following.
    #[test]
    fn schema_80_keeps_the_tunings() {
        use cantus_core::tuning::{Temperament, TrackTuning, TuningSystem};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v80.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // No longer the newest (81): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(80));
        let p = report.file.project;
        assert_eq!(p.tuning.a4_hz, 415.0);
        assert!(matches!(
            p.tuning.system,
            TuningSystem::Temperament {
                id: Temperament::WerckmeisterIii,
                key: 3,
                ..
            }
        ));
        assert_eq!(p.tracks[0].tuning, TrackTuning::Equal);
        let TrackTuning::Own { tuning } = &p.tracks[1].tuning else {
            panic!("the second track's own tuning")
        };
        let TuningSystem::Scale(s) = &tuning.system else {
            panic!("a scale")
        };
        assert_eq!(s.degrees.len(), 31);
        let old = std::fs::read_to_string(dir.join("v79.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p.tuning.is_default());
        assert!(p.tracks.iter().all(|t| t.tuning.is_follow()));
    }

    /// r42:chorda-c17 — Schema 84 (ADR 0098's sixth amendment): the fixture's Chorda keeps its
    /// colour; every older file (83) has none and reads Chorda's own (0).
    #[test]
    fn schema_84_keeps_chordas_colour() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let chorda = |name: &str| {
            let text = std::fs::read_to_string(dir.join(name)).unwrap();
            let report = parse(&text, true).unwrap();
            let p = report.file.project;
            let t = p
                .tracks
                .iter()
                .find(|t| t.name == "Chorda")
                .expect("Chorda");
            (
                report.integrity_verified,
                t.instrument
                    .native()
                    .expect("a native Chorda")
                    .get("colour"),
            )
        };
        // r42 fiddle — no longer the newest (85): migrated, its integrity is not checked;
        // the newest keeps the colour too.
        assert_eq!(chorda("v84.cantus").1, 0.6);
        // r42 arcus-classical — no longer the newest (87) either.
        assert_eq!(chorda("v85.cantus").1, 0.6);
        assert_eq!(chorda("v87.cantus"), (true, 0.6));
        assert_eq!(chorda("v83.cantus").1, 0.0);
    }

    /// r42 fiddle — Schema 85 (ADR 0164): the fixture's Arcus keeps its strings under one
    /// bow, its sympathy, its grit, its steel set and its just double stops; every older file
    /// (83) has none of them, and its Arcus reads each at the default that plays as before.
    #[test]
    fn schema_85_keeps_arcus_s_strings_under_one_bow() {
        use cantus_core::InstrumentSpec;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let arcus = |file: &str| {
            let text = std::fs::read_to_string(dir.join(file)).unwrap();
            let report = parse(&text, true).unwrap();
            let p = report.file.project;
            let spec = p
                .tracks
                .iter()
                .find_map(|t| match &t.instrument {
                    InstrumentSpec::Native(s)
                        if s.kind == cantus_core::InstrumentKind::BowedString =>
                    {
                        Some(s.clone())
                    }
                    _ => None,
                })
                .expect("the Arcus track");
            (report.integrity_verified, spec)
        };
        // (r42 arcus-classical: the newest, 87, keeps them with its integrity verified)
        let (ok, s) = arcus("v87.cantus");
        assert!(ok);
        let keys = [
            "bow.pair",
            "bow.tilt",
            "dstop.just",
            "bow.grit",
            "bow.bite",
            "sym",
            "steel",
            "tuning.strings",
            "body.mic",
            "body.bridge",
        ];
        let got: Vec<f32> = keys.iter().map(|k| s.get(k)).collect();
        assert_eq!(got, vec![1.0, 0.6, 0.6, 1.0, 0.7, 1.2, 1.0, 1.0, 1.0, 1.0]);
        let (_, old) = arcus("v83.cantus");
        let got: Vec<f32> = keys.iter().map(|k| old.get(k)).collect();
        assert_eq!(got, vec![0.0, 0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    /// r42 arcus-classical — Schema 87 (ADR 0166): the fixture's Arcus keeps its fiddler's
    /// switch; a file from before it (85, its steel set 1) is migrated to a fiddler, as it
    /// played, and one with no steel (83) stays the classical player.
    #[test]
    fn schema_87_keeps_arcus_s_fiddler_and_migrates_the_steel_set() {
        use cantus_core::InstrumentSpec;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let fiddle = |file: &str| {
            let text = std::fs::read_to_string(dir.join(file)).unwrap();
            let report = parse(&text, true).unwrap();
            let p = report.file.project;
            p.tracks
                .iter()
                .find_map(|t| match &t.instrument {
                    InstrumentSpec::Native(s)
                        if s.kind == cantus_core::InstrumentKind::BowedString =>
                    {
                        Some((s.get("player.fiddle"), s.get("steel")))
                    }
                    _ => None,
                })
                .expect("the Arcus track")
        };
        assert_eq!(fiddle("v87.cantus"), (0.8, 1.0));
        assert_eq!(fiddle("v85.cantus"), (1.0, 1.0));
        assert_eq!(fiddle("v83.cantus"), (0.0, 0.0));
    }

    /// r42 arcus-expr — Schema 83 (ADR 0162): the fixture's second note keeps its bow start,
    /// bow part and string; every older file (82) has none of them.
    #[test]
    fn schema_83_keeps_the_note_s_bow_start_part_and_string() {
        use cantus_core::note_art::{Articulation, Bow, BowPart, BowStart};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v83.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // r42 fiddle — no longer the newest (85): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(83));
        let p = report.file.project;
        let b = p.tracks[0].clips[0].notes[1]
            .art
            .expect("the second note's");
        assert_eq!(
            (b.articulation, b.bow, b.start, b.part, b.string),
            (
                Articulation::Legato,
                Some(Bow::Down),
                Some(BowStart::Air),
                Some(BowPart::Tip),
                Some(1)
            )
        );
        let old = std::fs::read_to_string(dir.join("v82.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .flat_map(|c| c.notes.iter())
            .filter_map(|n| n.art)
            .all(|a| a.start.is_none() && a.part.is_none() && a.string.is_none()));
    }

    /// r42:strings-rec — Schema 82 (ADR 0158): the fixture's Strings track keeps its scripted
    /// legato and its zones' articulations, bows and mod-wheel crossfade; every older file
    /// (81) has none of them.
    #[test]
    fn schema_82_keeps_exemplar_s_articulations_and_legato() {
        use cantus_core::inst::sampler::zf;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v82.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // r42 arcus-expr — no longer the newest (83): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(82));
        let p = report.file.project;
        let t = p
            .tracks
            .iter()
            .find(|t| t.name == "Strings")
            .expect("Strings");
        let spec = t.instrument.native().expect("Exemplar");
        assert_eq!(
            ["legato", "legato.xf", "legato.glide", "legato.skip"].map(|k| spec.get(k)),
            [1.0, 90.0, 40.0, 120.0]
        );
        let zones = cantus_core::inst::sampler::zones(spec);
        let f = |n: usize, i: usize| zones[n].1.get(i);
        assert_eq!(
            (f(0, zf::ART), f(0, zf::BOW), f(0, zf::XFCC)),
            (1.0, 1.0, 1.0)
        );
        assert_eq!((f(0, zf::XFOUTLO), f(0, zf::XFOUTHI)), (0.0, 127.0));
        assert_eq!(
            (f(1, zf::BOW), f(1, zf::XFINHI), f(1, zf::XFCURVE)),
            (2.0, 127.0, 1.0)
        );
        assert_eq!(f(2, zf::ART), 5.0);
        // r42 fix2-model (review 14): the project's keyboard map, its black keys unmapped.
        let map = p.tuning.map.as_ref().expect("a keyboard map");
        assert_eq!(
            (map.size, map.octave_degree, map.reference_key),
            (12, 7, 69)
        );
        assert_eq!(map.mapping.iter().filter(|e| e.is_none()).count(), 5);
        assert_eq!(map.mapping[1], None);
        assert_eq!(p.tuning.a4_hz, 415.0);
        let table = p.tuning.offsets();
        assert!(
            !table.sounds(61) && table.sounds(60),
            "an x key sounds nothing"
        );
        let old = std::fs::read_to_string(dir.join("v81.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p.tracks.iter().all(|t| t.name != "Strings"));
        assert!(p
            .tracks
            .iter()
            .filter_map(|t| t.instrument.native())
            .all(|n| n.get("legato") == 0.0));
    }

    /// r42:contract — Schema 81 (ADRs 0157, 0158): the fixture holds an Arcus track, and the
    /// first clip's first two notes carry an articulation (pizzicato; legato with a down bow);
    /// every older file (80) has neither.
    #[test]
    fn schema_81_keeps_arcus_and_the_notes_articulation() {
        use cantus_core::inst::InstrumentKind;
        use cantus_core::note_art::{Articulation, Bow};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v81.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // r42:strings-rec — no longer the newest (82): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(81));
        let p = report.file.project;
        assert!(p.tracks.iter().any(|t| t
            .instrument
            .native()
            .is_some_and(|n| n.kind == InstrumentKind::BowedString)));
        let notes = &p.tracks[0].clips[0].notes;
        let a = notes[0].art.expect("the first note's articulation");
        assert_eq!(a.articulation, Articulation::Pizzicato);
        let b = notes[1].art.expect("the second note's");
        assert_eq!(
            (b.articulation, b.bow),
            (Articulation::Legato, Some(Bow::Down))
        );
        let old = std::fs::read_to_string(dir.join("v80.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .flat_map(|c| c.notes.iter())
            .all(|n| n.art.is_none()));
    }

    /// r41:fides-input — Schema 79 (ADR 0154): the fixture's Fides track hears the third
    /// strip as its input, and it round-trips; every older file (78) opens with no track
    /// keyed.
    #[test]
    fn schema_79_keeps_fides_s_sidechain() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v79.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // r42:contract — no longer the newest (81): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(79));
        let p = report.file.project;
        let fides = p
            .tracks
            .iter()
            .find(|t| t.name == "Fides")
            .expect("the Fides track");
        let third = p.strips[2].id;
        assert_eq!(fides.sidechain, Some(third));
        assert_eq!(fides.key(), Some(third), "an instrument that takes it");
        let order = p.strip_order();
        let at = |id| order.iter().position(|i| p.strips[*i].id == id);
        let dests: Vec<_> = fides.destinations().into_iter().flatten().collect();
        assert!(dests.iter().all(|d| at(third) < at(*d)), "the key first");
        let old = std::fs::read_to_string(dir.join("v78.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p.tracks.iter().all(|t| t.sidechain.is_none()));
    }

    /// fxd:contract — Schema 77 (the effects side round's wave D): the fixture holds
    /// an X-Y pad (links, a range, inertia, a playing path) and a formula whose knob b a
    /// clip automates; every older file (74) opens with no controller and no input lane.
    #[test]
    fn schema_77_keeps_the_pad_the_formula_and_their_input_lanes() {
        use cantus_core::automation::AutoTarget;
        use cantus_core::modulator::{ModKind, ModSource, RouteRange};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v77.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // r41:contract — no longer the newest (78): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(77));
        let p = report.file.project;
        let kinds: Vec<ModKind> = p
            .master
            .modulators
            .iter()
            .map(|m| m.source.kind())
            .collect();
        assert!(
            kinds.ends_with(&[ModKind::XyPad, ModKind::Formula]),
            "{kinds:?}"
        );
        let n = p.master.modulators.len();
        let pad = &p.master.modulators[n - 2];
        assert_eq!(pad.name, "Sweep");
        let ModSource::XyPad(x) = &pad.source else {
            unreachable!()
        };
        assert_eq!(
            (x.x, x.y, x.inertia_ms, x.play_path),
            (0.3, 0.7, 250.0, true)
        );
        assert_eq!(x.path.as_ref().unwrap().points.len(), 4);
        assert_eq!(
            pad.source.link(&AutoTarget::MasterWidth),
            (
                1,
                Some(RouteRange {
                    from: 0.0,
                    to: 0.6,
                    curve: 0.4
                })
            )
        );
        assert_eq!(pad.source.link(&AutoTarget::MasterPan), (0, None));
        let f = &p.master.modulators[n - 1];
        let ModSource::Formula(src) = &f.source else {
            unreachable!()
        };
        assert_eq!(src.text, "a*Sin(SongTime*Pi*b)+c");
        assert_eq!(src.inputs, [0.5, 0.25, 0.5]);
        let lane = AutoTarget::ModInput {
            modulator: f.id,
            input: 1,
        };
        assert!(p
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .any(|c| c.automation.as_ref().is_some_and(|a| a.target == lane)));
        // The engine's plan: the pad's two routes linked, the formula compiled, knob b's lane.
        let s = cantus_core::Snapshot::compile(&p, 1);
        let fp = s.modulators.iter().find(|m| m.id == f.id).unwrap();
        assert!(fp.program.is_some() && fp.input_lanes[1].is_some());
        let old = std::fs::read_to_string(dir.join("v74.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .all_modulators()
            .all(|(_, m)| m.source.kind().inputs() == 0));
    }

    /// fxa:contract — Schema 74 (the effects side round's wave A): the fixture holds
    /// one insert of each of the three new kinds, the Fractus with two painted lanes; every
    /// older file (73) opens with none of them and no pattern. A hand-edited pattern is
    /// repaired: overlapping blocks cut, a pattern on another kind taken off.
    #[test]
    fn schema_74_keeps_wave_a_s_kinds_and_fractus_s_lanes() {
        use cantus_core::fx::fractus::Block;
        use cantus_core::EffectKind as K;
        let new_kinds = &K::ALL[35..38];
        assert_eq!(new_kinds, [K::Vocoder, K::Glitch, K::Shimmer]);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v74.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // fxd:contract — no longer the newest: migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(74));
        let p = report.file.project;
        let slots: Vec<_> = p.strips[1]
            .effects
            .iter()
            .filter(|e| new_kinds.contains(&e.spec.kind))
            .collect();
        let kinds: Vec<K> = slots.iter().map(|e| e.spec.kind).collect();
        assert_eq!(kinds, new_kinds.to_vec());
        let pat = slots[1].spec.fractus.as_deref().expect("lanes");
        assert_eq!(pat.lane(0), [Block::new(12, 4)]);
        let mut half = Block::new(8, 2);
        half.amount = 0.5;
        half.prob = 0.75;
        assert_eq!(pat.lane(7), [half]);
        assert_eq!(slots[0].spec.get("mix"), 1.0);
        assert_eq!(slots[2].spec.get("shim"), 0.55);
        // fxa:caelum — the fixture's Caelum was written with the two infinite switches of
        // wave A's first build (`ishim` 0, Fades; `ilevel` 0): it opens as Sustain 0 %,
        // the switches gone (ADR 0138's amendment).
        assert_eq!(slots[2].spec.stored("isus"), Some(0.0));
        assert!(
            slots[2].spec.stored("ishim").is_none() && slots[2].spec.stored("ilevel").is_none()
        );
        let old = std::fs::read_to_string(dir.join("v73.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .all_effects()
            .all(|(_, e)| !new_kinds.contains(&e.spec.kind) && e.spec.fractus.is_none()));
        // A hand edit: an overlapping block, and a pattern on Caelum.
        let mut v: Value = serde_json::from_str(&text).unwrap();
        v.as_object_mut().unwrap().remove("integrity");
        let fx = v["project"]["strips"][1]["effects"].as_array_mut().unwrap();
        let at = fx
            .iter()
            .position(|e| e["spec"]["kind"] == "Glitch")
            .unwrap();
        let lane = fx[at]["spec"]["fractus"]["lanes"]["0"]
            .as_array_mut()
            .unwrap();
        lane.push(serde_json::json!({"at": 14, "len": 4}));
        let pattern = fx[at]["spec"]["fractus"].clone();
        let other = fx
            .iter()
            .position(|e| e["spec"]["kind"] == "Shimmer")
            .unwrap();
        fx[other]["spec"]["fractus"] = pattern;
        let p = parse(&v.to_string(), true).unwrap().file.project;
        let all: Vec<_> = p.all_effects().map(|(_, e)| e).collect();
        let g = all.iter().find(|e| e.spec.kind == K::Glitch).unwrap();
        assert_eq!(
            g.spec.fractus.as_ref().unwrap().lane(0),
            [Block::new(12, 4), Block::new(16, 2)]
        );
        assert!(all
            .iter()
            .filter(|e| e.spec.kind != K::Glitch)
            .all(|e| e.spec.fractus.is_none()));
    }

    /// fxb:contract — Schema 75 (the effects side round's wave B): the fixture holds
    /// one insert of each of the three new kinds, the Cedo with a drawn shape; every older
    /// file (74) opens with none of them and no shape. A hand-edited shape is repaired:
    /// points out of range sanitized, a shape on another kind taken off.
    #[test]
    fn schema_75_keeps_wave_b_s_kinds_and_cedo_s_shape() {
        use cantus_core::fx::lapsus::Point;
        use cantus_core::EffectKind as K;
        let new_kinds = &K::ALL[38..41];
        assert_eq!(
            new_kinds,
            [K::BusCompressor, K::ResonanceSuppressor, K::Ducker]
        );
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v75.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // fxd — no longer the newest (77): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(75));
        let p = report.file.project;
        let slots: Vec<_> = p.strips[1]
            .effects
            .iter()
            .filter(|e| new_kinds.contains(&e.spec.kind))
            .collect();
        let kinds: Vec<K> = slots.iter().map(|e| e.spec.kind).collect();
        assert_eq!(kinds, new_kinds.to_vec());
        assert_eq!(slots[0].spec.get("model"), 1.0);
        assert_eq!(slots[0].spec.get("thr"), -20.0);
        assert_eq!(slots[1].spec.get("b2on"), 1.0);
        assert_eq!(slots[2].spec.get("depth"), 9.0);
        let shape = slots[2].spec.cedo.as_deref().expect("a shape");
        assert_eq!(shape.points[1], Point::new(0.4, 0.3).bend(0.5));
        let old = std::fs::read_to_string(dir.join("v74.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .all_effects()
            .all(|(_, e)| !new_kinds.contains(&e.spec.kind) && e.spec.cedo.is_none()));
        // A hand edit: a point past the cycle, and a shape on Vinculum.
        let mut v: Value = serde_json::from_str(&text).unwrap();
        v.as_object_mut().unwrap().remove("integrity");
        let fx = v["project"]["strips"][1]["effects"].as_array_mut().unwrap();
        let at = fx
            .iter()
            .position(|e| e["spec"]["kind"] == "Ducker")
            .unwrap();
        fx[at]["spec"]["cedo"]["points"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"x": 1.5, "y": 2.0}));
        let shape = fx[at]["spec"]["cedo"].clone();
        let other = fx
            .iter()
            .position(|e| e["spec"]["kind"] == "BusCompressor")
            .unwrap();
        fx[other]["spec"]["cedo"] = shape;
        let p = parse(&v.to_string(), true).unwrap().file.project;
        let all: Vec<_> = p.all_effects().map(|(_, e)| e).collect();
        let c = all.iter().find(|e| e.spec.kind == K::Ducker).unwrap();
        assert_eq!(
            c.spec.cedo.as_ref().unwrap().points.last(),
            Some(&Point::new(1.0, 1.0))
        );
        assert!(all
            .iter()
            .filter(|e| e.spec.kind != K::Ducker)
            .all(|e| e.spec.cedo.is_none()));
    }

    /// fxc:contract — Schema 76 (the effects side round's wave C): the fixture holds
    /// a Purus with its de-hum and de-reverb on and a Comparatio with a reference track by
    /// path; every older file (75) opens with neither.
    #[test]
    fn schema_76_keeps_wave_c_s_kinds_and_the_reference_by_path() {
        use cantus_core::EffectKind as K;
        let new_kinds = [K::Repair, K::Reference];
        // r41:contract — at their places in `ALL` (41, 42), which never move; round 41 appended.
        assert_eq!(&K::ALL[41..43], new_kinds);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v76.cantus")).unwrap();
        let report = parse(&text, true).unwrap();
        // fxd — no longer the newest (77): migrated, its integrity is not checked.
        assert_eq!(report.migrated_from, Some(76));
        let p = report.file.project;
        let slots: Vec<_> = p.strips[1]
            .effects
            .iter()
            .filter(|e| new_kinds.contains(&e.spec.kind))
            .collect();
        let kinds: Vec<K> = slots.iter().map(|e| e.spec.kind).collect();
        assert_eq!(kinds, new_kinds.to_vec());
        assert_eq!(slots[0].spec.get("hon"), 1.0);
        assert_eq!(slots[0].spec.get("von"), 1.0);
        assert_eq!(slots[0].spec.get("vdirect"), 0.0);
        let r = slots[1].spec.ir.as_deref().expect("the reference");
        assert_eq!(r.path, "references/Reference master.flac");
        assert_eq!(slots[1].spec.get("match"), 1.0);
        // Phase B: the list of references, the one compared among them with its print.
        let refs = slots[1].spec.refs.as_deref().expect("the list");
        assert_eq!(refs.list.len(), 3);
        let at = refs.find(&r.path).expect("the one compared is listed");
        assert_eq!(refs.list[at].bytes, 31_457_280);
        assert_eq!(refs.list[at].hash, "9e3779b97f4a7c15");
        assert_eq!(slots[1].spec.get("eqamt"), 0.75);
        // Each reference its own playback (the owner, 2026-11-17).
        let other = refs.list[refs.find("references/Other master.wav").unwrap()].playback;
        assert_eq!(
            other.map(|p| (p.sync, p.lstart, p.lend)),
            Some((2.0, 45.0, 75.0))
        );
        let old = std::fs::read_to_string(dir.join("v75.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .all_effects()
            .all(|(_, e)| !new_kinds.contains(&e.spec.kind)));
    }

    /// fx3:contract — a hand-edited board is repaired on load: a nested board and a second
    /// pedal with the same id are dropped, and a board on a slot that is not one is taken off.
    #[test]
    fn a_hand_edited_board_is_repaired_on_load() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v73.cantus")).unwrap();
        let mut v: Value = serde_json::from_str(&text).unwrap();
        v.as_object_mut().unwrap().remove("integrity");
        let fx = v["project"]["strips"][1]["effects"].as_array_mut().unwrap();
        let at = fx
            .iter()
            .position(|e| e["spec"]["kind"] == "Board")
            .unwrap();
        let pedals = fx[at]["spec"]["board"]["pedals"].as_array_mut().unwrap();
        let mut twin = pedals[0].clone();
        twin["spec"] = serde_json::json!({"kind": "Board", "params": {}});
        pedals.push(twin);
        let board = fx[at]["spec"]["board"].clone();
        let other = fx
            .iter()
            .position(|e| e["spec"]["kind"] == "Rotary")
            .unwrap();
        fx[other]["spec"]["board"] = board;
        let p = parse(&v.to_string(), true).unwrap().file.project;
        let all: Vec<_> = p.all_effects().map(|(_, e)| e).collect();
        let b = all
            .iter()
            .find(|e| e.spec.kind == cantus_core::EffectKind::Board)
            .unwrap();
        assert_eq!(b.spec.board.as_ref().unwrap().pedals.len(), 2);
        assert!(all
            .iter()
            .filter(|e| e.spec.kind != cantus_core::EffectKind::Board)
            .all(|e| e.spec.board.is_none()));
    }

    /// fx:lapsus — schema 72's Lapsus carries a bank of its own (a volume pattern in slot 5,
    /// named), read back as written; the others hold none.
    #[test]
    fn schema_72_carries_lapsus_s_bank() {
        use cantus_core::fx::lapsus::Lane;
        use cantus_core::EffectKind as K;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v72.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let all: Vec<_> = p.strips.iter().flat_map(|s| s.effects.iter()).collect();
        let lapsus = all.iter().find(|e| e.spec.kind == K::TimeShaper).unwrap();
        let bank = lapsus.spec.lapsus.as_deref().expect("a bank");
        assert!(bank.time.is_empty());
        let mine = bank.pattern(Lane::Vol, 4);
        assert_eq!((mine.len, mine.points.len()), (1, 2));
        assert_eq!(mine.name.as_deref(), Some("Half gate"));
        assert!(all
            .iter()
            .filter(|e| e.spec.kind != K::TimeShaper)
            .all(|e| e.spec.lapsus.is_none()));
    }

    /// Schema 26 (round 28, ADR 0052): the fixture's metronome has the woodblock voice and
    /// no accent, and the song is saved in pattern mode; an older file (25) opens with the
    /// classic accented click in song mode — what it always played.
    /// Schema 33 (round 29, ADR 0060): the fixture's tempo track (a ramp from the song's
    /// tempo to 140 over four bars, then 100), its metre changes (a bar of 7/8 at bar 3,
    /// 3/4 from bar 4) and its markers come back as they were; an older file (32) opens
    /// with one tempo, one metre and no markers, and its map is the constant one.
    #[test]
    fn schema_33_keeps_the_tempo_track_metres_and_markers() {
        use cantus_core::{Ticks, TimeSignature};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v33.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        assert_eq!(p.tempo_points.len(), 3);
        assert!(p.tempo_points[0].ramp);
        assert_eq!(p.tempo_points[0].bpm, p.tempo_bpm);
        assert_eq!(p.tempo_points[1].bpm, 140.0);
        let map = p.tempo_map();
        assert!(!map.is_constant());
        assert_eq!(map.signature_at(Ticks(7680)), TimeSignature::new(7, 8));
        assert_eq!(map.signature_at(Ticks(11_040)), TimeSignature::new(3, 4));
        let names: Vec<&str> = p.markers.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Intro", "Verse", "Chorus"]);
        let old = std::fs::read_to_string(dir.join("v32.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p.tempo_points.is_empty() && p.metre_changes.is_empty() && p.markers.is_empty());
        assert!(p.tempo_map().is_constant() && p.tempo_map().is_one_metre());
    }

    /// Schemas 66–68 (round 40 phase 3, ADRs 0123–0125): the newest fixture keeps a Chorda's
    /// routed perspective and its mixer key, a Tympanum's kit, keys and routed mic, and the
    /// Chorda's recorded source; a schema-65 file has none and plays as it did. // r40:p3
    #[test]
    fn schemas_66_to_68_keep_outputs_a_tympanum_and_a_chordas_source() {
        use cantus_core::inst::chorda_source::{ChordaSource, RecordedPiano};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let newest = format!("v{SCHEMA_VERSION}.cantus");
        let p = parse(&std::fs::read_to_string(dir.join(newest)).unwrap(), true)
            .unwrap()
            .file
            .project;
        let chorda = p.tracks.iter().find(|t| t.name == "Chorda").unwrap();
        let n = chorda.instrument.native().unwrap();
        assert_eq!(n.outputs.len(), 1);
        assert_eq!(n.outputs[0].id, cantus_core::OutputId(3));
        assert_eq!(n.get("o3.level"), -3.0);
        assert_eq!(chorda.instrument.output_strips(), vec![n.outputs[0].strip]);
        assert_eq!(
            ChordaSource::of(n),
            ChordaSource::Recorded(RecordedPiano {
                pack: "salamander-grand-piano".into(),
                instrument: "grand".into(),
            })
        );
        assert_eq!(n.get("rec.z0.lo"), 60.0);
        let drums = p.tracks.iter().find(|t| t.name == "Tympanum").unwrap();
        let d = drums.instrument.native().unwrap();
        assert_eq!(d.kind, cantus_core::InstrumentKind::AcousticDrums);
        assert_eq!(d.get("p1.tune"), 2.0);
        assert_eq!(d.get("b1.3.db"), -6.0);
        assert_eq!(d.get("m2.az"), 10.0);
        assert_eq!(d.outputs.len(), 1);
        // Schema 65 had none of it.
        let old = std::fs::read_to_string(dir.join("v65.cantus")).unwrap();
        let old = parse(&old, true).unwrap().file.project;
        for t in &old.tracks {
            if let Some(n) = t.instrument.native() {
                assert!(n.outputs.is_empty(), "{}", t.name);
                assert_eq!(ChordaSource::of(n), ChordaSource::Model, "{}", t.name);
                assert_ne!(n.kind, cantus_core::InstrumentKind::AcousticDrums);
            }
        }
    }

    /// Schema 70 (round 40 phase 3, ADR 0123 amended): the strip added to the Tympanum's
    /// family comes back added; schema 69's strips have none. // r40:tymp-ui
    #[test]
    fn schema_70_keeps_a_strip_added_to_a_family() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let load = |name: &str| {
            parse(&std::fs::read_to_string(dir.join(name)).unwrap(), true)
                .unwrap()
                .file
                .project
        };
        let p = load("v70.cantus");
        let kit = p.tracks.iter().find(|t| t.name == "Tympanum").unwrap().id;
        let added: Vec<_> = p.strips.iter().filter(|s| s.attached.is_some()).collect();
        assert_eq!(added.len(), 1, "one strip added to the kit's family");
        assert_eq!(added[0].attached, Some(kit));
        let old = load("v69.cantus");
        assert!(old.strips.iter().all(|s| s.attached.is_none()));
    }

    /// Schema 69 (round 40 phase 3, ADR 0126 amended): the Chorda's room look comes back
    /// whole; schema 68's Chorda has none (its room is its Room size's, as before).
    /// // r40:scene
    #[test]
    fn schema_69_keeps_the_scenes_room_look() {
        use cantus_core::inst::room_look::{self, Floor, RoomKind, RoomLook, Rug, Walls};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let load = |name: &str| {
            parse(&std::fs::read_to_string(dir.join(name)).unwrap(), true)
                .unwrap()
                .file
                .project
        };
        let p = load("v69.cantus");
        let chorda = p.tracks.iter().find(|t| t.name == "Chorda").unwrap();
        assert_eq!(
            room_look::of(chorda.instrument.native().unwrap()),
            RoomLook {
                room: Some(RoomKind::Hall),
                floor: Some(Floor::EbonyStage),
                walls: Some(Walls::Brick),
                rug: Some(Rug::Blue),
            }
        );
        let old = load("v68.cantus");
        let chorda = old.tracks.iter().find(|t| t.name == "Chorda").unwrap();
        assert_eq!(
            room_look::of(chorda.instrument.native().unwrap()),
            RoomLook::default()
        );
    }

    /// Schema 71 (round 40, ADR 0124's third amendment): the hybrid jazz kit comes back
    /// hybrid; schema 70's Tympanum is generated. // r40:drums-hyb
    #[test]
    fn schema_71_keeps_a_tympanums_voice() {
        use cantus_core::inst::tympanum::Voice;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let load = |name: &str| {
            parse(&std::fs::read_to_string(dir.join(name)).unwrap(), true)
                .unwrap()
                .file
                .project
        };
        let p = load("v71.cantus");
        let t = p
            .tracks
            .iter()
            .find(|t| t.name == "Tympanum hybrid")
            .unwrap();
        assert_eq!(Voice::of(t.instrument.native().unwrap()), Voice::Hybrid);
        let old = load("v70.cantus");
        assert!(old.tracks.iter().all(|t| t.name != "Tympanum hybrid"));
        for t in &old.tracks {
            if let Some(n) = t.instrument.native() {
                assert_eq!(Voice::of(n), Voice::Generated, "{}", t.name);
            }
        }
    }

    /// Schema 35 (round 29, ADR 0062): the fixture's controller mappings come back whole —
    /// a knob on a filter with its range and bend, a 14-bit fader picking up with
    /// feedback, an encoder in sign-bit on the tempo, a pad toggling a mute, a button
    /// firing *play*, one whose target is gone (kept dormant); an older file (33) has none.
    /// (The tree's newest fixture holds them: the integrator regenerates `v35.cantus` once
    /// the round's schemas are chained.)
    #[test]
    fn schema_35_keeps_the_controller_mappings() {
        use cantus_core::controller::*;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let newest = format!("v{SCHEMA_VERSION}.cantus");
        let text = std::fs::read_to_string(dir.join(newest)).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let maps = &p.controllers.mappings;
        // Seven since schema 35 (later schemas add their own after them).
        assert!(maps.len() >= 7, "{}", maps.len());
        let knob = &maps[0];
        assert_eq!(knob.source.port.name, "Keystation 49");
        assert_eq!(knob.source.control, Control::Cc(74));
        assert_eq!((knob.min, knob.max, knob.curve), (0.1, 0.9, 0.4));
        assert!(matches!(
            knob.target,
            MappingTarget::Param(cantus_core::AutoTarget::Effect { .. })
        ));
        assert!(knob.target.exists(&p));
        let fader = &maps[1];
        assert_eq!(fader.source.control, Control::Cc14(7));
        assert!(fader.feedback);
        assert_eq!(fader.takeover, Takeover::Pickup);
        assert_eq!(maps[2].mode, MapMode::Relative(Encoding::SignBit));
        assert_eq!(maps[2].target, MappingTarget::Tempo);
        assert_eq!(maps[2].sensitivity, 0.5);
        assert_eq!(maps[3].mode, MapMode::Toggle);
        assert_eq!(maps[3].source.port.ordinal, 1);
        assert_eq!(maps[4].target, MappingTarget::Action("play-pause".into()));
        assert!(!maps[5].target.exists(&p), "the dormant one stays");
        // Review 2, #42: a hosted mapping keeps the plug-in it was learnt on, and the
        // schema-35 file itself reads the same mappings.
        assert!(maps[6].plugin.is_some(), "{:?}", maps[6]);
        assert!(maps[6].fits(&p));
        let v35 = std::fs::read_to_string(dir.join("v35.cantus")).unwrap();
        let p35 = parse(&v35, true).unwrap().file.project;
        assert_eq!(p35.controllers.mappings[..], p.controllers.mappings[..7]);
        let old = std::fs::read_to_string(dir.join("v33.cantus")).unwrap();
        assert!(parse(&old, true)
            .unwrap()
            .file
            .project
            .controllers
            .is_empty());
    }

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
    /// note moved to another semitone and pulled onto its centre, one stretched, the key,
    /// the A/B switch on — and they come back as they were; an older file (25) opens with
    /// none.
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
        assert_eq!((e.notes[0].pitch, e.notes[0].centre), (67.0, 1.0));
        assert_ne!(e.notes[0].pitch, e.notes[0].home(), "moved to another note");
        assert!(e.notes[1].moved());
        assert_eq!(e.notes[2].formant, -2.0);
        assert!(e.bypass, "the A/B switch, written and read");
        assert!(
            p.tracks.iter().any(|t| t.pitch_open),
            "a track's pitch lane shown"
        );
        let old = std::fs::read_to_string(dir.join("v25.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .all(|c| c.audio.as_ref().is_none_or(|a| a.pitch.is_none())));
    }

    /// Schema 28 (round 28, ADR 0057): the fixture's modulators come back as they were —
    /// an LFO on the Keys track driving the delay's mix and the hosted plug-in's
    /// parameter, an envelope on the Keys strip ducking its fader, a follower on the master
    /// listening to the Drums strip, a step sequence on the hosted synth and a random
    /// source nested into the LFO's amount — and an automation clip on a hosted parameter;
    /// an older file (27) opens with none.
    #[test]
    fn schema_28_keeps_modulators_and_hosted_automation() {
        use cantus_core::modulator::{ModKind, ModSource};
        use cantus_core::AutoTarget;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v28.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let mods: Vec<_> = p.all_modulators().map(|(_, m)| m).collect();
        let kinds: Vec<ModKind> = mods.iter().map(|m| m.source.kind()).collect();
        // fxd: the controllers arrived with schema 77 (its own test).
        for k in ModKind::ALL
            .iter()
            .filter(|k| !ModKind::CONTROLLERS.contains(k))
        {
            assert!(kinds.contains(k), "{k:?} in the fixture");
        }
        assert!(mods.iter().all(|m| m.is_valid()));
        let lfo = mods
            .iter()
            .find(|m| m.source.kind() == ModKind::Lfo)
            .unwrap();
        assert_eq!(lfo.routes.len(), 2);
        assert!(lfo
            .routes
            .iter()
            .any(|r| r.target.is_plugin() && r.depth == -0.4));
        assert!(mods.iter().any(|m| m
            .routes
            .iter()
            .any(|r| r.target == AutoTarget::ModAmount(lfo.id))));
        assert!(matches!(
            &mods.iter().find(|m| m.source.kind() == ModKind::Step).unwrap().source,
            ModSource::Step(s) if s.steps.len() == 8 && s.glide == 0.25
        ));
        assert!(!p.master.modulators.is_empty());
        assert!(p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter_map(|c| c.automation.as_ref())
            .any(|a| matches!(a.target, AutoTarget::Plugin { id: 1024, .. })));
        let old = std::fs::read_to_string(dir.join("v27.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert_eq!(p.all_modulators().count(), 0);
    }

    /// Schema 29 (round 28, ADR 0058): the fixture's vocal comp has a seam with a crossfade
    /// (the right window's `xfade`), two alternate comps — A playing, B keeping a window
    /// whole, pitch edits and all — and the record options' seam default and automation
    /// switch; an older file (28) opens with none, its defaults those of every earlier
    /// version.
    #[test]
    fn schema_29_keeps_comp_seams_and_alternate_comps() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v29.cantus")).unwrap();
        let p = parse(&text, true).unwrap().file.project;
        let vocal = p.tracks.iter().find(|t| t.name == "Vocal").unwrap();
        let seam = vocal
            .clips
            .iter()
            .filter(|c| c.lane == 0)
            .find_map(|c| c.audio.as_ref().map(|a| a.xfade))
            .into_iter()
            .chain(
                vocal
                    .clips
                    .iter()
                    .filter_map(|c| c.audio.as_ref().map(|a| a.xfade)),
            )
            .max()
            .unwrap();
        assert_eq!(seam, cantus_core::Ticks(19));
        assert_eq!(vocal.comps.len(), 2);
        assert_eq!((vocal.comps[0].name.as_str(), vocal.active_comp), ("A", 0));
        assert!(vocal.comps[0].windows.is_empty());
        let kept = &vocal.comps[1].windows;
        assert_eq!(kept.len(), 1);
        assert!(kept[0].audio.as_ref().is_some_and(|a| a.pitch.is_some()));
        assert_eq!(p.recording.comp_xfade_ms, 15.0);
        assert!(!p.recording.record_automation);
        let old = std::fs::read_to_string(dir.join("v28.cantus")).unwrap();
        let p = parse(&old, true).unwrap().file.project;
        assert!(p.tracks.iter().all(|t| t.comps.is_empty()));
        assert_eq!(p.recording.comp_xfade_ms, 10.0);
        assert!(p.recording.record_automation);
    }

    /// Schema 30 (round 28, sound): every Exemplar saved before keeps its seeded round
    /// robin (`rr.order` written as 0); one already carrying the key keeps it; other kinds
    /// are untouched. The v30 fixture holds an Exemplar made now (in order, not stored).
    #[test]
    fn schema_30_keeps_an_old_exemplars_round_robin_seeded() {
        let mut v = serde_json::json!({"project": {"tracks": [
            {"instrument": {"type": "Native", "kind": "Sampler"}},
            {"instrument": {"type": "Native", "kind": "Sampler", "params": {"z0.gen": 3.0}}},
            {"instrument": {"type": "Native", "kind": "Sampler", "params": {"rr.order": 1.0}}},
            {"instrument": {"type": "Native", "kind": "Organ"}},
            {"instrument": {"type": "Firmus"}},
        ]}});
        migrate_29_to_30(&mut v).unwrap();
        let t = &v["project"]["tracks"];
        assert_eq!(t[0]["instrument"]["params"]["rr.order"], 0.0);
        assert_eq!(t[1]["instrument"]["params"]["rr.order"], 0.0);
        assert_eq!(t[1]["instrument"]["params"]["z0.gen"], 3.0);
        assert_eq!(t[2]["instrument"]["params"]["rr.order"], 1.0);
        assert!(t[3]["instrument"].get("params").is_none());
        assert!(t[4]["instrument"].get("params").is_none());
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let samplers = |file: &str| -> Vec<(String, f32)> {
            let text = std::fs::read_to_string(dir.join(file)).unwrap();
            let p = parse(&text, true).unwrap().file.project;
            p.tracks
                .iter()
                .filter_map(|t| {
                    let n = t.instrument.native()?;
                    (n.kind == cantus_core::InstrumentKind::Sampler)
                        .then(|| (t.name.clone(), n.get("rr.order")))
                })
                .collect()
        };
        let old = samplers("v29.cantus");
        assert!(
            !old.is_empty() && old.iter().all(|(_, o)| *o == 0.0),
            "{old:?}"
        );
        let new = samplers("v30.cantus");
        assert!(
            new.contains(&("Exemplar in order".to_string(), 1.0)),
            "{new:?}"
        );
    }

    /// Schema 34 (round 29, ADR 0061): the vocal's window keeps its warp — markers, mode,
    /// transposition, what was detected — and an older file's clips have none.
    #[test]
    fn schema_34_keeps_the_warp() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let warp = |name: &str| {
            let text = std::fs::read_to_string(dir.join(name)).unwrap();
            let p = parse(&text, true).unwrap().file.project;
            let vocal = p.tracks.iter().find(|t| t.name == "Vocal").unwrap().clone();
            vocal
                .clips
                .iter()
                .filter(|c| c.lane == 0)
                .find_map(|c| c.audio.as_ref().and_then(|a| a.warp.clone()))
        };
        let w = warp("v34.cantus").expect("a warped window");
        assert!(w.is_valid());
        assert_eq!(w.mode, cantus_core::warp::WarpMode::Melodic);
        assert_eq!(w.cents, -300);
        assert_eq!(w.markers.len(), 2);
        assert_eq!(w.markers[1].tick, Ticks::beats(2) + Ticks(120));
        assert!(w.detected.is_some() && w.formants && w.follow);
        assert!(warp("v32.cantus").is_none());
    }

    /// Round 29's review: a file whose warp is not valid (a hand edit) opens with the warp
    /// dropped — its clip at natural speed — and says how many.
    #[test]
    fn a_file_with_an_invalid_warp_opens_without_it() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let text = std::fs::read_to_string(dir.join("v34.cantus")).unwrap();
        let mut p = parse(&text, true).unwrap().file.project;
        let mut broken = 0;
        for t in &mut p.tracks {
            for c in &mut t.clips {
                if let Some(w) = c.audio.as_mut().and_then(|a| a.warp.as_mut()) {
                    w.markers.reverse();
                    broken += 1;
                }
            }
        }
        assert!(broken > 0);
        let text = ProjectFile::wrap(p, "test").unwrap().to_json().unwrap();
        let r = parse(&text, true).unwrap();
        assert_eq!(r.warps_repaired, broken);
        let left = r
            .file
            .project
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .filter(|c| c.audio.as_ref().is_some_and(|a| a.warp.is_some()))
            .count();
        assert_eq!(left, 0);
    }

    /// Schema 31 (round 28, review 2): the vocal's second note was set by *Correct pitch*
    /// and says so; an older file's notes do not.
    #[test]
    fn schema_31_keeps_which_pitch_is_the_macros() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let notes = |name: &str| {
            let text = std::fs::read_to_string(dir.join(name)).unwrap();
            let p = parse(&text, true).unwrap().file.project;
            let vocal = p.tracks.iter().find(|t| t.name == "Vocal").unwrap().clone();
            vocal
                .clips
                .iter()
                .filter(|c| c.lane == 0)
                .find_map(|c| c.audio.as_ref().and_then(|a| a.pitch.clone()))
                .unwrap()
                .notes
        };
        let now = notes("v31.cantus");
        assert!(now.iter().any(|n| n.id == 1 && n.corrected), "{now:?}");
        assert!(now.iter().any(|n| !n.corrected));
        assert!(notes("v30.cantus").iter().all(|n| !n.corrected));
    }

    /// Schema 32 (fix4 of ADR 0058): the migration flags exactly the clips schema 31 took
    /// for comp windows — a window where its take put it, one slipped or nudged over its
    /// take, a MIDI window, every window an alternate comp keeps — and nothing else: the
    /// takes, a clip of another file, a copy away from its takes, a window over no take.
    #[test]
    fn schema_32_flags_the_windows_schema_31_counted() {
        let clip = |id: u64, lane: u64, start: i64, len: i64, off: i64, src: u64| {
            serde_json::json!({"id": id, "lane": lane, "start": start, "length": len,
                "offset": off, "notes": [], "audio": {"source": src}})
        };
        let midi = |id: u64, lane: u64, start: i64, len: i64, off: i64, note: u64| {
            serde_json::json!({"id": id, "lane": lane, "start": start, "length": len,
                "offset": off, "notes": [{"id": note}]})
        };
        let mut v = serde_json::json!({"project": {"tracks": [
            {"clips": [
                clip(1, 1, 0, 8_000, 0, 7),       // take A
                clip(2, 2, 0, 8_000, 0, 8),       // take B
                clip(3, 0, 0, 2_000, 0, 7),       // a window on A where A put it
                clip(4, 0, 2_000, 2_000, 2_100, 8), // slipped over B
                clip(5, 0, 4_100, 2_000, 4_000, 7), // nudged over A
                clip(6, 0, 30_000, 2_000, 0, 7),  // a copy far from its takes
                clip(7, 0, 0, 2_000, 0, 9),       // another file
            ],
             "comps": [{"name": "A"}, {"name": "B", "windows": [clip(10, 0, 0, 100, 0, 7)]}]},
            {"clips": [
                midi(20, 1, 0, 960, 0, 500),
                midi(21, 0, 0, 480, 0, 500),
                midi(22, 0, 9_000, 480, 0, 501),
            ]},
            {"clips": [clip(30, 0, 0, 100, 0, 7)]},
        ]}});
        migrate_31_to_32(&mut v).unwrap();
        let flagged = |t: usize| -> Vec<u64> {
            v["project"]["tracks"][t]["clips"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["comp"] == true)
                .map(|c| c["id"].as_u64().unwrap())
                .collect()
        };
        assert_eq!(flagged(0), vec![3, 4, 5]);
        assert_eq!(flagged(1), vec![21]);
        assert!(flagged(2).is_empty(), "a track without takes has no comp");
        assert_eq!(
            v["project"]["tracks"][0]["comps"][1]["windows"][0]["comp"],
            true
        );
    }

    /// Schema 32 on the fixtures: every v29–v31 file opens with its comp windows flagged
    /// as schema 31 counted them (the vocal's two windows, B's kept one, nothing else),
    /// and the v32 file keeps a copy pasted right over its take out of the comp.
    #[test]
    fn schema_32_keeps_old_comps_exactly_and_a_pasted_copy_out() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let load = |name: &str| {
            let text = std::fs::read_to_string(dir.join(name)).unwrap();
            parse(&text, true).unwrap().file.project
        };
        // Schema 31's rule, on the typed clips: a playing-lane clip of a take's content
        // over that take, where it put it or at an origin no take has.
        let rule = |t: &cantus_core::Track, c: &cantus_core::Clip| {
            let key = |c: &cantus_core::Clip| {
                let content = match &c.audio {
                    Some(a) => (a.source.0, true),
                    None => (c.notes.first().map(|n| n.id.0)?, false),
                };
                Some((content, (c.start - c.offset).0))
            };
            let Some(k) = key(c) else { return false };
            let takes = || t.clips.iter().filter(|x| x.lane > 0);
            let exact = takes().any(|x| key(x) == Some(k));
            c.lane == 0
                && takes().any(|x| {
                    key(x).is_some_and(|(xc, xo)| {
                        xc == k.0 && c.start < x.end() && c.end() > x.start && (xo == k.1 || !exact)
                    })
                })
        };
        for name in ["v28.cantus", "v29.cantus", "v30.cantus", "v31.cantus"] {
            let p = load(name);
            let mut windows = 0;
            for t in &p.tracks {
                for c in &t.clips {
                    assert_eq!(c.comp, rule(t, c), "{name}: {} clip {:?}", t.name, c.id);
                    windows += usize::from(c.comp);
                }
                assert!(t.comps.iter().flat_map(|c| &c.windows).all(|w| w.comp));
            }
            // v28's vocal plays its take over the older one: schema 31 counted it too.
            let want = if name == "v28.cantus" { 1 } else { 2 };
            assert_eq!(windows, want, "{name}: the vocal's windows");
        }
        let p = load("v32.cantus");
        let vocal = p.tracks.iter().find(|t| t.name == "Vocal").unwrap();
        let lane0: Vec<(i64, bool)> = vocal
            .clips
            .iter()
            .filter(|c| c.lane == 0 && c.start < Ticks::beats(16))
            .map(|c| (c.start.0 / cantus_core::PPQ, c.comp))
            .collect();
        assert_eq!(lane0, vec![(4, true), (8, true), (12, false)]);
        let pasted = vocal
            .clips
            .iter()
            .find(|c| c.lane == 0 && c.start == Ticks::beats(12));
        assert!(
            pasted.is_some_and(|c| rule(vocal, c)),
            "schema 31 would have taken the paste for a window"
        );
        assert!(vocal.comps[1].windows.iter().all(|w| w.comp));
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

#[cfg(test)]
mod isolation_tests {
    use super::*;

    fn hosted(
        file: &str,
    ) -> (
        Option<cantus_core::PluginIsolation>,
        Option<cantus_core::PluginIsolation>,
    ) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(file);
        let text = std::fs::read_to_string(&path).expect("the fixture");
        let p = parse(&text, true).expect("opens").file.project;
        let insert = p
            .all_effects()
            .find_map(|(_, e)| e.spec.external.as_deref())
            .expect("a hosted insert")
            .isolation;
        let inst = p
            .tracks
            .iter()
            .find_map(|t| t.instrument.external())
            .expect("a hosted instrument")
            .isolation;
        (insert, inst)
    }

    /// Schema 36 (round 29, ADR 0063): the fixture's hosted insert runs *Individually* and
    /// its hosted instrument *Within Cantus*, as chosen; the v32 file's follow the default.
    #[test]
    fn schema_36_keeps_where_a_hosted_plugin_runs() {
        use cantus_core::PluginIsolation::*;
        assert_eq!(hosted("v36.cantus"), (Some(Individually), Some(Within)));
        assert_eq!(hosted("v32.cantus"), (None, None));
    }
}

#[cfg(test)]
mod freeze_tests {
    use super::*;
    use cantus_core::freeze::{self, FreezeTarget};

    fn fixture(file: &str) -> Project {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(file);
        let text = std::fs::read_to_string(&path).expect("the fixture");
        parse(&text, true).expect("opens").file.project
    }

    /// Schema 37 (round 29, ADR 0064): the fixture's first strip is frozen and so is one
    /// track's source, each with the key of what it holds; a v36 file has nothing frozen.
    #[test]
    fn schema_37_keeps_frozen_strips_and_tracks() {
        let p = fixture("v37.cantus");
        let targets = p.frozen_targets();
        assert_eq!(targets.len(), 2, "{targets:?}");
        assert!(matches!(targets[0], FreezeTarget::Strip(_)));
        assert!(matches!(targets[1], FreezeTarget::Track(_)));
        // Re-keyed on load when its subject was an older one's: what it holds now.
        assert!(freeze::consistent(&p).is_ok(), "each key is what it holds");
        assert!(!fixture("v36.cantus").any_frozen());
    }

    /// Schema 44 (round 31, ADR 0070): two clips crossing an overlap on an S-curve, the
    /// first with its gain envelope; a v43 file has neither.
    #[test]
    fn schema_44_keeps_crossfades_and_clip_envelopes() {
        use cantus_core::crossfade::XfadeCurve;
        let p = fixture("v44.cantus");
        let t = p
            .tracks
            .iter()
            .find(|t| t.name == "Doubles")
            .expect("the track");
        let env = t.clips[0]
            .audio
            .as_ref()
            .and_then(|a| a.gain_env.as_deref())
            .expect("an envelope");
        assert_eq!(env.points.len(), 4);
        assert_eq!(env.points[1].shape, cantus_core::Shape::Hold);
        assert!(env.is_valid());
        let b = t.clips[1].audio.as_ref().unwrap();
        assert_eq!(b.cross, Some(XfadeCurve::SCurve));
        assert_eq!(
            cantus_core::crossfade::overlap_into(t, t.clips[1].id).map(|o| o.left_last),
            Some(t.clips[0].id)
        );
        assert!(fixture("v43.cantus").tracks.iter().all(|t| t
            .clips
            .iter()
            .filter_map(|c| c.audio.as_ref())
            .all(|a| a.cross.is_none() && a.gain_env.is_none())));
    }

    /// Schema 46 (round 33, ADR 0076): the session — three scenes (one with its tempo,
    /// metre, colour and follow action), a column holding a gate-launched MIDI clip with its
    /// start marker, follow actions and an envelope, a looping audio clip and an empty slot
    /// without its stop button, and the session's settings; a v45 file has none; slots of a
    /// scene that is not there and a session clip out of its form (a hand edit) are repaired.
    #[test]
    fn schema_46_keeps_the_session() {
        use cantus_core::session::*;
        let p = fixture("v46.cantus");
        let s = &p.session;
        assert_eq!(s.scenes.len(), 3);
        assert_eq!(s.scenes[1].tempo, Some(128.0));
        assert_eq!(s.scenes[1].follow.b, FollowAction::Jump(s.scenes[0].id));
        assert_eq!(s.settings.launch_quantize, Quantize::Bars2);
        assert_eq!(s.settings.fixed_bars, Some(4));
        let t = p
            .tracks
            .iter()
            .find(|t| !t.slots.is_empty())
            .expect("a column");
        assert_eq!(t.slots.len(), 3);
        let c = t.slot_clip(s.scenes[0].id).expect("the riff");
        assert_eq!(c.launch.mode, LaunchMode::Gate);
        assert_eq!(c.start, Some(cantus_core::Ticks::beats(2)));
        assert_eq!(c.follow.count, 4);
        assert_eq!(c.envelopes.len(), 1);
        assert!(t.clip(c.id()).is_some(), "found as any clip is");
        assert!(t.slot_clip(s.scenes[1].id).unwrap().clip.audio.is_some());
        assert!(!t.slot(s.scenes[2].id).unwrap().stop);
        assert!(!fixture("v45.cantus").has_session());
        assert!(fixture("v45.cantus")
            .tracks
            .iter()
            .all(|t| t.slots.is_empty()));
        // A hand edit: a slot of a scene that is gone, a clip in a link group on a take
        // lane — repaired on load.
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("v46.cantus"),
        )
        .unwrap();
        let mut v: Value = serde_json::from_str(&text).unwrap();
        let tracks = v["project"]["tracks"].as_array_mut().unwrap();
        let t = tracks
            .iter_mut()
            .find(|t| t.get("slots").is_some())
            .expect("the column");
        let slots = t["slots"].as_array_mut().unwrap();
        slots[0]["clip"]["clip"]["link"] = serde_json::json!(4242);
        slots[0]["clip"]["clip"]["lane"] = serde_json::json!(3);
        let mut ghost = slots[2].clone();
        ghost["scene"] = serde_json::json!(999_999_999u64);
        slots.push(ghost);
        let r = parse(&serde_json::to_string(&v).unwrap(), false).expect("opens");
        let t = r
            .file
            .project
            .tracks
            .iter()
            .find(|t| !t.slots.is_empty())
            .unwrap();
        assert_eq!(t.slots.len(), 3);
        let c = &t.slots[0].clip.as_ref().unwrap().clip;
        assert_eq!((c.link, c.lane), (None, 0));
    }

    /// Schema 47 (round 33, ADR 0080): per-note expression and a hosted instrument's route
    /// survive a save; a v45 file has none; a hand-edited curve out of order is repaired on
    /// load, never handed to a command or the audio thread as it was.
    #[test]
    fn schema_47_keeps_per_note_expression() {
        use cantus_core::model::note_expr::ExprKind;
        let p = fixture("v47.cantus");
        let n = p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().flat_map(|c| c.notes.iter()))
            .find(|n| n.expr.is_some())
            .expect("a note with expression");
        let e = n.expr.as_deref().unwrap();
        assert_eq!(e.value_at(ExprKind::Bend, 240), Some(2.0));
        assert_eq!(e.pressure.len(), 2);
        assert!(p.tracks.iter().any(|t| t
            .instrument
            .external()
            .is_some_and(|x| x.expression == Some(cantus_core::external::ExprRoute::MpeMidi))));
        let old = fixture("v45.cantus");
        assert!(old
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().flat_map(|c| c.notes.iter()))
            .all(|n| n.expr.is_none()));
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("v47.cantus"),
        )
        .unwrap();
        let mut v: Value = serde_json::from_str(&text).unwrap();
        let note = v["project"]["tracks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .flat_map(|t| t["clips"].as_array_mut().unwrap().iter_mut())
            .flat_map(|c| c["notes"].as_array_mut().unwrap().iter_mut())
            .find(|n| n.get("expr").is_some())
            .unwrap();
        note["expr"]["bend"] = serde_json::json!([[480, 0], [0, 99]]);
        let r = parse(&serde_json::to_string(&v).unwrap(), false).expect("opens");
        let e = r
            .file
            .project
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().flat_map(|c| c.notes.iter()))
            .find_map(|n| n.expr.as_deref())
            .unwrap();
        assert!(e.is_valid());
        assert_eq!(
            e.bend,
            vec![
                cantus_core::model::note_expr::ExprPoint(0, 99),
                cantus_core::model::note_expr::ExprPoint(480, 0)
            ]
        );
    }

    /// Schema 45 (round 32, ADR 0075): a track's score settings — a grand staff split at
    /// a fixed pitch, a transposition, dynamics from velocity, a reading grid and a name;
    /// a v44 file has none, and a hand-edited transposition out of range is repaired.
    #[test]
    fn schema_45_keeps_score_settings() {
        use cantus_core::notation::{ReadingGrid, StaffChoice, MAX_TRANSPOSE};
        let p = fixture("v45.cantus");
        let t = p
            .tracks
            .iter()
            .find(|t| !t.score.is_default())
            .expect("a track with settings");
        assert_eq!(t.score.staves, StaffChoice::Grand);
        assert_eq!(t.score.split, Some(62));
        assert_eq!(t.score.transpose, 2);
        assert!(t.score.dynamics);
        assert_eq!(t.score.grid, ReadingGrid::EighthTriplet);
        assert_eq!(t.score.name.as_deref(), Some("Piano"));
        assert!(fixture("v44.cantus")
            .tracks
            .iter()
            .all(|t| t.score.is_default()));
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("v45.cantus"),
        )
        .unwrap();
        let mut v: Value = serde_json::from_str(&text).unwrap();
        let tracks = v["project"]["tracks"].as_array_mut().unwrap();
        let t = tracks
            .iter_mut()
            .find(|t| t.get("score").is_some())
            .expect("the track");
        t["score"]["transpose"] = serde_json::json!(120);
        let r = parse(&serde_json::to_string(&v).unwrap(), false).expect("opens");
        let t = r
            .file
            .project
            .tracks
            .iter()
            .find(|t| !t.score.is_default())
            .unwrap();
        assert_eq!(t.score.transpose, MAX_TRANSPOSE);
    }

    /// Review 1 of ADR 0070: a damaged file's gain envelope (a value out of range, points
    /// out of order, a tension past its end) is repaired on load and counted — it never
    /// reaches the audio thread as it was.
    #[test]
    fn a_damaged_gain_envelope_is_repaired_on_load() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("v44.cantus"),
        )
        .unwrap();
        let mut v: Value = serde_json::from_str(&text).unwrap();
        let tracks = v["project"]["tracks"].as_array_mut().unwrap();
        let doubles = tracks
            .iter_mut()
            .find(|t| t["name"] == "Doubles")
            .expect("the track");
        let pts = doubles["clips"][0]["audio"]["gain_env"]["points"]
            .as_array_mut()
            .unwrap();
        pts[0]["db"] = serde_json::json!(1.0e39);
        pts[1]["tension"] = serde_json::json!(7.5);
        pts.reverse();
        let damaged = serde_json::to_string(&v).unwrap();
        let r = parse(&damaged, false).expect("opens");
        assert_eq!(r.gain_envs_repaired, 1);
        let t = r
            .file
            .project
            .tracks
            .iter()
            .find(|t| t.name == "Doubles")
            .unwrap();
        let e = t.clips[0]
            .audio
            .as_ref()
            .unwrap()
            .gain_env
            .as_deref()
            .unwrap();
        assert!(e.is_valid(), "{e:?}");
        assert!(e.points.iter().all(|p| p.db.is_finite()));
        // A file Cantus wrote needs none.
        assert_eq!(parse(&text, true).unwrap().gain_envs_repaired, 0);
    }

    /// Schema 41 (round 29, ADR 0062 review 4): a hosted mapping keeps the plug-in's and
    /// the parameter's names it was learnt with; a v40 mapping has none.
    #[test]
    fn schema_41_keeps_a_hosted_mappings_names() {
        let p = fixture("v41.cantus");
        let named = p
            .controllers
            .mappings
            .iter()
            .filter_map(|m| m.plugin.as_ref())
            .find(|r| !r.param.is_empty())
            .expect("a hosted mapping with its names");
        assert_eq!(
            (named.name.as_str(), named.param.as_str()),
            ("Cantus Test Synth", "Cutoff")
        );
        assert!(fixture("v40.cantus")
            .controllers
            .mappings
            .iter()
            .filter_map(|m| m.plugin.as_ref())
            .all(|r| r.name.is_empty() && r.param.is_empty()));
    }

    /// Schema 40 (round 29, ADR 0062 review 3): the strip's effects switch and record arm
    /// are mapped; a v39 map has neither.
    #[test]
    fn schema_40_keeps_the_strip_switch_mappings() {
        use cantus_core::controller::MappingTarget;
        let p = fixture("v40.cantus");
        let targets: Vec<&MappingTarget> =
            p.controllers.mappings.iter().map(|m| &m.target).collect();
        assert!(targets
            .iter()
            .any(|t| matches!(t, MappingTarget::StripFx(_))));
        assert!(targets
            .iter()
            .any(|t| matches!(t, MappingTarget::StripArm(_))));
        assert!(!fixture("v39.cantus")
            .controllers
            .mappings
            .iter()
            .any(|m| matches!(
                m.target,
                MappingTarget::StripFx(_) | MappingTarget::StripArm(_)
            )));
    }

    /// Schema 38 (round 29, review 2): a bounced track keeps its lateness; a v37 track has
    /// none. And the current fixture's frozen keys, as written, need no repair (review 1:
    /// a check after loading holds after any repair) — the subject did not move under it.
    #[test]
    fn schema_38_keeps_a_tracks_lateness_and_its_keys_as_written() {
        let p = fixture("v38.cantus");
        let late: Vec<_> = p.tracks.iter().filter_map(|t| t.lateness).collect();
        assert_eq!(
            late,
            vec![cantus_core::Lateness {
                frames: 4_608,
                rate: 48_000
            }]
        );
        assert!(fixture("v37.cantus")
            .tracks
            .iter()
            .all(|t| t.lateness.is_none()));
        let targets = p.frozen_targets();
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v38.cantus"),
        )
        .unwrap();
        let raw: serde_json::Value = serde_json::from_str(&text).unwrap();
        let stored: Vec<String> = ["strips", "tracks"]
            .iter()
            .flat_map(|k| raw["project"][k].as_array().cloned().unwrap_or_default())
            .filter_map(|x| x["frozen"]["content"].as_str().map(str::to_string))
            .collect();
        let keys: Vec<String> = targets
            .iter()
            .map(|t| freeze::content_key(&p, *t).unwrap())
            .collect();
        assert_eq!(stored, keys, "the fixture's keys need no repair");
    }

    /// A frozen key that is not what the target holds (a hand edit) is re-keyed on load.
    #[test]
    fn a_hand_edited_frozen_key_is_repaired_on_load() {
        let mut p = fixture("v37.cantus");
        let FreezeTarget::Strip(s) = p.frozen_targets()[0] else {
            panic!("a strip first");
        };
        p.strip_mut(s).unwrap().frozen.as_mut().unwrap().content = "a".repeat(64);
        let text = ProjectFile::wrap(p, "test").unwrap().to_json().unwrap();
        let back = parse(&text, false).unwrap().file.project;
        assert!(freeze::consistent(&back).is_ok());
        assert!(back.strip(s).unwrap().frozen.is_some(), "still frozen");
    }

    /// Round 29's third review (finding 46): a track delay a bounce could not have kept (a
    /// hand edit) is dropped on load and counted, never planned with.
    #[test]
    fn an_out_of_range_track_delay_is_dropped_on_load() {
        let mut p = fixture("v37.cantus");
        p.tracks[0].lateness = Some(cantus_core::Lateness {
            frames: 1_000_000_000_000_000,
            rate: 1,
        });
        p.tracks[1].lateness = Some(cantus_core::Lateness {
            frames: 4_608,
            rate: 48_000,
        });
        let text = ProjectFile::wrap(p, "test").unwrap().to_json().unwrap();
        let r = parse(&text, false).unwrap();
        assert_eq!(r.lateness_repaired, 1);
        assert_eq!(r.file.project.tracks[0].lateness, None);
        assert!(r.file.project.tracks[1].lateness.is_some());
        assert!(freeze::consistent(&r.file.project).is_ok());
    }

    /// Round 29's second review (finding 33): a warp dropped on load goes before the links
    /// and the frozen keys are checked — a frozen target holding it keeps a key that is
    /// what it holds, and a linked copy whose warp alone was bad stays linked. Kept comp
    /// windows are repaired too (finding 40).
    #[test]
    fn an_invalid_warp_is_dropped_before_links_and_frozen_keys_are_checked() {
        use cantus_core::comp::Comp;
        let mut p = fixture("v37.cantus");
        let mut bad = cantus_core::warp::Warp::new(48_000, 120.0);
        bad.rate = 0;
        let mut broken = 0;
        for t in &mut p.tracks {
            for c in t.clips.iter_mut().filter(|c| c.link.is_none()) {
                if let Some(a) = c.audio.as_mut() {
                    a.warp = Some(Box::new(bad.clone()));
                    broken += 1;
                }
            }
        }
        // A linked pair whose second copy alone carries the bad warp.
        let (ti, first) = p
            .tracks
            .iter()
            .enumerate()
            .find_map(|(i, t)| {
                t.clips
                    .iter()
                    .find(|c| c.audio.is_some() && c.lane == 0)
                    .map(|c| (i, c.clone()))
            })
            .expect("an audio clip");
        let link = cantus_core::LinkId(p.next_id + 900);
        let mut a = first.clone();
        a.id = cantus_core::ClipId(p.next_id + 901);
        a.start = first.start + cantus_core::Ticks::beats(400);
        a.link = Some(link);
        a.audio.as_mut().unwrap().warp = None;
        let mut b = a.clone();
        b.id = cantus_core::ClipId(p.next_id + 902);
        b.start = a.start + cantus_core::Ticks::beats(40);
        b.audio.as_mut().unwrap().warp = Some(Box::new(bad.clone()));
        broken += 1;
        // A kept window of a second comp with one too.
        let mut kept = a.clone();
        kept.id = cantus_core::ClipId(p.next_id + 903);
        kept.link = None;
        kept.comp = true;
        kept.audio.as_mut().unwrap().warp = Some(Box::new(bad.clone()));
        broken += 1;
        p.next_id += 1_000;
        let t = &mut p.tracks[ti];
        t.clips.push(a.clone());
        t.clips.push(b.clone());
        t.clips.sort_by_key(|c| (c.start, c.id));
        if t.comps.is_empty() {
            t.comps.push(Comp {
                name: "A".into(),
                windows: Vec::new(),
            });
            t.active_comp = 0;
        }
        t.comps.push(Comp {
            name: "B".into(),
            windows: vec![kept],
        });
        let text = ProjectFile::wrap(p, "test").unwrap().to_json().unwrap();
        let r = parse(&text, false).unwrap();
        assert_eq!(r.warps_repaired, broken);
        let back = r.file.project;
        assert!(
            freeze::consistent(&back).is_ok(),
            "each frozen key is what it holds"
        );
        let t = &back.tracks[ti];
        assert_eq!(t.clip(a.id).unwrap().link, Some(link));
        assert_eq!(t.clip(b.id).unwrap().link, Some(link), "still linked");
        assert!(t
            .comps
            .iter()
            .flat_map(|c| &c.windows)
            .all(|w| w.audio.as_ref().is_none_or(|a| a.warp.is_none())));
    }

    /// Schema 39 (round 29, warp's second review): a warp that started from the natural
    /// warp under the tempo ramp keeps its own markers (the ramp's samples) apart from the
    /// user's; a v38 file's markers are all the user's.
    #[test]
    fn schema_39_keeps_the_warps_own_markers() {
        let markers = |name: &str| -> Vec<cantus_core::warp::WarpMarker> {
            fixture(name)
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .filter_map(|c| c.audio.as_ref()?.warp.as_ref())
                .flat_map(|w| w.markers.clone())
                .collect()
        };
        let now = markers("v39.cantus");
        assert!(now.iter().any(|m| m.auto) && now.iter().filter(|m| !m.auto).count() >= 4);
        assert!(markers("v38.cantus").iter().all(|m| !m.auto));
        let p = fixture("v39.cantus");
        assert!(p
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .filter_map(|c| c.audio.as_ref()?.warp.as_ref())
            .all(|w| w.is_valid()));
    }
}

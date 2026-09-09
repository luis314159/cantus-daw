//! On-disk representation and schema migrations (ADR 0006).

use cantus_core::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const FORMAT_ID: &str = "cantus-project";
pub const SCHEMA_VERSION: u32 = 3;
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

    let file: ProjectFile = serde_json::from_value(value)?;
    Ok(LoadReport {
        file,
        migrated_from,
        integrity_verified,
    })
}

/// Migration `i` converts schema `i` → `i + 1`. Never remove or reorder entries: a file
/// saved today must open in five years (idea.md §5.15).
type Migration = fn(&mut Value) -> Result<(), String>;
static MIGRATIONS: &[Migration] = &[migrate_0_to_1, migrate_1_to_2, migrate_2_to_3];

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

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::Ticks;

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
            assert_eq!(r.file.project.tracks.len(), 2, "{}", path.display());
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

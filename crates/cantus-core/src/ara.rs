//! An audio clip bound to an ARA 2 plug-in (schema 53, ADR 0097): which plug-in edits it,
//! and the plug-in's own state for it, as it gave it when the project was last saved.
//!
//! Plain data: the ARA SDK, the plug-in and everything unsafe live in the GPL `cantus-ara`
//! and `cantus-host` crates. A bound clip plays the plug-in's render of its window in place
//! of its source while the plug-in is there, and its source (said on screen) when it is not.

use serde::{Deserialize, Serialize};

use crate::external::PluginState;

/// The plug-in a clip is bound to and its state for that clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AraClip {
    pub plugin: AraPlugin,
    /// Which binding this is: a fresh bind gets a new number and starts clean in the plug-in;
    /// an undo or redo brings the same number back, and with it the plug-in's edits.
    #[serde(default)]
    pub binding: u64,
    /// The plug-in's archive of this clip's audio modification (ARA's partial persistency).
    /// `None` until the project is first saved with the clip bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<AraArchive>,
}

/// Enough of a plug-in to find it again: a VST3 class (32 hex digits) in a bundle, and the
/// ARA factory's own id (versioned by the plug-in).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AraPlugin {
    /// The VST3 class id of the audio processor.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    /// The bundle it was found in (a hint: the plug-in database finds it by id).
    #[serde(default)]
    pub path: String,
    /// `ARAFactory::factoryID`.
    #[serde(default)]
    pub factory_id: String,
}

/// A partial ARA archive: the bytes, and the ids they were written under — the document
/// archive id (`ARAFactory::documentArchiveID`, which the plug-in checks on restore) and the
/// persistent ids of the audio source and modification, mapped to the current ones when the
/// clip is restored under another id (a copy, an import).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AraArchive {
    pub document_archive_id: String,
    pub source_id: String,
    pub modification_id: String,
    pub data: PluginState,
}

impl AraClip {
    pub fn new(plugin: AraPlugin) -> Self {
        AraClip {
            plugin,
            binding: 0,
            archive: None,
        }
    }
}

/// The persistent id Cantus gives an audio source in an ARA document.
pub fn source_persistent_id(source: crate::model::AudioSourceId) -> String {
    format!("cantus.source.{}", source.0)
}

/// The persistent id Cantus gives a clip's audio modification in an ARA document.
pub fn modification_persistent_id(clip: crate::model::ClipId) -> String {
    format!("cantus.clip.{}", clip.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bound_clip_round_trips_through_json_with_its_archive_as_base64() {
        let mut a = AraClip::new(AraPlugin {
            id: "0123456789ABCDEF0123456789ABCDEF".into(),
            name: "Test".into(),
            vendor: "V".into(),
            path: "x.vst3".into(),
            factory_id: "com.test.ara".into(),
        });
        a.archive = Some(AraArchive {
            document_archive_id: "doc.v1".into(),
            source_id: "s".into(),
            modification_id: "m".into(),
            data: PluginState::new(vec![0, 1, 2, 255]),
        });
        let text = serde_json::to_string(&a).unwrap();
        assert!(text.contains("\"data\":\"AAEC/w==\""), "{text}");
        assert_eq!(serde_json::from_str::<AraClip>(&text).unwrap(), a);
        let bare = serde_json::to_string(&AraClip::new(a.plugin.clone())).unwrap();
        assert!(!bare.contains("archive"));
    }

    #[test]
    fn persistent_ids_are_distinct_per_object() {
        use crate::model::{AudioSourceId, ClipId};
        assert_ne!(
            source_persistent_id(AudioSourceId(1)),
            modification_persistent_id(ClipId(1))
        );
    }
}

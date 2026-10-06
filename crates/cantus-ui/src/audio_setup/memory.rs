//! What Cantus remembers about each interface (round 20): its driver, rate and buffer, the
//! record offset measured on it, the names of its channels, and where the master and the
//! headphone cue go — so plugging the same interface in again brings all of it back, and
//! the laptop's own audio keeps its own.
//!
//! Kept in `settings.json` under `audio_setup`, keyed by the interface's adapter name
//! (`interfaces::Interface::key`). Every field has a default, so a file from before this
//! round reads as "nothing remembered, setup not done".

use crate::settings::AudioSettings;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One interface's settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub audio: AudioSettings,
    /// The inputs' names, by channel (0-based); an empty name is unnamed.
    pub inputs: Vec<String>,
    /// The outputs' names, by channel.
    pub outputs: Vec<String>,
    /// The first channel of the pair the master plays on (0 = 1–2).
    pub main: u16,
    /// The first channel of the headphone cue's pair, when there is one.
    pub cue: Option<u16>,
    /// The cue's level, in dB.
    pub cue_db: f32,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            audio: AudioSettings::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            main: 0,
            cue: None,
            cue_db: 0.0,
        }
    }
}

impl Profile {
    /// Channel `ch`'s name, when it has one.
    pub fn input_name(&self, ch: usize) -> Option<&str> {
        self.inputs
            .get(ch)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn output_name(&self, ch: usize) -> Option<&str> {
        self.outputs
            .get(ch)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    /// Name channel `ch` (growing the list as needed; trailing blanks are dropped).
    pub fn set_name(list: &mut Vec<String>, ch: usize, name: &str) {
        if list.len() <= ch {
            list.resize(ch + 1, String::new());
        }
        list[ch] = name.trim().to_string();
        while list.last().is_some_and(|s| s.is_empty()) {
            list.pop();
        }
    }

    /// The cue's gain, linear.
    pub fn cue_gain(&self) -> f32 {
        10f32.powf(self.cue_db.clamp(-60.0, 12.0) / 20.0)
    }
}

/// Everything remembered, and whether the first-run setup has been done.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMemory {
    /// The first-run setup finished (or was skipped): it does not open by itself again.
    pub setup_done: bool,
    pub profiles: BTreeMap<String, Profile>,
    /// The interface in use when the settings were last stored: when it disappears,
    /// this is the one to come back to.
    pub last: Option<String>,
}

impl AudioMemory {
    /// Store `audio` as `key`'s settings, keeping its names and routing.
    pub fn remember(&mut self, key: &str, audio: &AudioSettings) {
        let p = self.profiles.entry(key.to_string()).or_default();
        p.audio = audio.clone();
        self.last = Some(key.to_string());
    }

    /// `key`'s stored settings, if it has been used before.
    pub fn recall(&self, key: &str) -> Option<&AudioSettings> {
        self.profiles.get(key).map(|p| &p.audio)
    }

    pub fn profile(&self, key: &str) -> Option<&Profile> {
        self.profiles.get(key)
    }

    pub fn profile_mut(&mut self, key: &str) -> &mut Profile {
        self.profiles.entry(key.to_string()).or_default()
    }
}

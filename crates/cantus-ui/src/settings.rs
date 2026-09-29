//! User preferences: the choices that belong to the person, not to the project.
//!
//! A `.cantus` file holds the music. This holds everything about *how* the person likes to
//! work — which sound card, which language, how often to autosave — in `settings.json`,
//! next to `theme.json`, `layouts.json` and `keymap.toml`.
//!
//! The theme is deliberately **not** here: it is a whole document of its own that the user
//! edits by hand and the app reloads live, so it stays in its own file. What lives here is
//! the settings a dialog can hold in a handful of rows.
//!
//! Every field is optional or has a serde default, so a settings file written by an older
//! version keeps working and a newer field simply takes its default.

use cantus_engine::AudioConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How often the project is written to the autosave directory, in seconds.
pub const DEFAULT_AUTOSAVE_SECONDS: u64 = 60;
/// The shortest autosave interval the dialog offers. Below this the writes start to be
/// noticeable on a large project, and autosave is a safety net, not a version history.
pub const MIN_AUTOSAVE_SECONDS: u64 = 10;
pub const MAX_AUTOSAVE_SECONDS: u64 = 600;

/// How much bigger or smaller everything is drawn. egui scales fonts and the spacing grid
/// together, so the layout keeps its proportions rather than only the text growing.
pub const MIN_UI_SCALE: f32 = 0.75;
pub const MAX_UI_SCALE: f32 = 2.0;
pub const DEFAULT_UI_SCALE: f32 = 1.0;
/// The steps Ctrl + and Ctrl − walk through (ADR 0024 §11).
pub const UI_SCALES: [f32; 9] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 1.9, 2.0];

/// The zoom step after `now`, going up or down; a scale between steps goes to the nearest
/// step in that direction.
pub fn ui_scale_step(now: f32, up: bool) -> f32 {
    let eps = 1e-3;
    if up {
        UI_SCALES
            .iter()
            .copied()
            .find(|s| *s > now + eps)
            .unwrap_or(MAX_UI_SCALE)
    } else {
        UI_SCALES
            .iter()
            .rev()
            .copied()
            .find(|s| *s < now - eps)
            .unwrap_or(MIN_UI_SCALE)
    }
}

/// Sample rates worth offering. `None` means "whatever the device prefers", which is the
/// right default: it is the one setting guaranteed to work.
pub const SAMPLE_RATES: [u32; 5] = [44_100, 48_000, 88_200, 96_000, 192_000];

/// Buffer sizes in frames, powers of two because that is what every driver expects.
pub const BUFFER_SIZES: [u32; 7] = [64, 128, 256, 512, 1024, 2048, 4096];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// `None` = the system's default output device.
    pub device: Option<String>,
    /// `None` = the device's preferred rate.
    pub sample_rate: Option<u32>,
    /// `None` = the device's preferred buffer size.
    pub buffer_frames: Option<u32>,
}

impl AudioSettings {
    pub fn to_config(&self) -> AudioConfig {
        AudioConfig {
            device_name: self.device.clone(),
            sample_rate: self.sample_rate,
            buffer_frames: self.buffer_frames,
        }
    }

    /// The latency one block costs at a given rate, which is the number a musician
    /// actually cares about when choosing a buffer size.
    pub fn block_latency_ms(&self, actual_rate: f64) -> Option<f32> {
        let frames = self.buffer_frames? as f64;
        (actual_rate > 0.0).then(|| (frames / actual_rate * 1000.0) as f32)
    }
}

/// How much of each mixer strip is shown — FL's layout sizes, reduced to the three that
/// differ in *what* is shown rather than by a few pixels. A preference, not project data:
/// the same desk is compact on a laptop and wide on a studio screen.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixerDensity {
    /// Narrow strips: pan, switches, fader, meter, route. No width bar, EQ bars or sends.
    Compact,
    #[default]
    Normal,
    /// Wider strips, so every bar has more travel.
    Wide,
}

impl MixerDensity {
    pub fn label_key(self) -> &'static str {
        match self {
            MixerDensity::Compact => "mixer.density.compact",
            MixerDensity::Normal => "mixer.density.normal",
            MixerDensity::Wide => "mixer.density.wide",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub audio: AudioSettings,
    /// `None` = follow the `--locale` flag, then the built-in default.
    pub locale: Option<String>,
    pub autosave_seconds: u64,
    pub ui_scale: f32,
    pub mixer_density: MixerDensity,
    /// Show each strip's recent level as a scrolling envelope (FL's meter-wave view).
    pub mixer_meter_wave: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            audio: AudioSettings::default(),
            locale: None,
            autosave_seconds: DEFAULT_AUTOSAVE_SECONDS,
            ui_scale: DEFAULT_UI_SCALE,
            mixer_density: MixerDensity::Normal,
            mixer_meter_wave: false,
        }
    }
}

impl Settings {
    /// The UI scale, clamped: a settings file hand-edited to 0 would draw nothing at all,
    /// and one edited to 50 would draw a single letter.
    pub fn ui_scale(&self) -> f32 {
        if self.ui_scale.is_finite() {
            self.ui_scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE)
        } else {
            DEFAULT_UI_SCALE
        }
    }

    /// The autosave interval, clamped to something sane. A settings file edited by hand to
    /// say `0` must not turn autosave into a busy loop that writes on every frame.
    pub fn autosave_interval(&self) -> Duration {
        Duration::from_secs(
            self.autosave_seconds
                .clamp(MIN_AUTOSAVE_SECONDS, MAX_AUTOSAVE_SECONDS),
        )
    }

    /// A missing or unreadable file is not an error: it is a fresh install, or a file the
    /// user broke while editing. Either way the defaults are correct and the app opens.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("settings.json is not usable ({e}); using the defaults");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

/// Where the settings live, next to the theme, the layouts and the keymap.
pub fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_a_missing_file_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path(dir.path());
        assert_eq!(Settings::load(&path), Settings::default());

        let settings = Settings {
            audio: AudioSettings {
                device: Some("Speakers".into()),
                sample_rate: Some(48_000),
                buffer_frames: Some(256),
            },
            locale: Some("es".into()),
            autosave_seconds: 30,
            ui_scale: 1.25,
            mixer_density: MixerDensity::Compact,
            mixer_meter_wave: true,
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
    }

    #[test]
    fn a_broken_or_partial_file_still_opens_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path(dir.path());

        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());

        // A file from an older version has only some of the fields; the rest default.
        std::fs::write(&path, r#"{"locale":"es"}"#).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.locale.as_deref(), Some("es"));
        assert_eq!(loaded.autosave_seconds, DEFAULT_AUTOSAVE_SECONDS);
        assert_eq!(loaded.audio, AudioSettings::default());
    }

    #[test]
    fn the_autosave_interval_can_never_become_a_busy_loop() {
        let mut s = Settings::default();
        assert_eq!(s.autosave_interval(), Duration::from_secs(60));

        // Hand-edited nonsense must be clamped, not obeyed: autosave runs every frame
        // otherwise, and a large project would be written continuously.
        s.autosave_seconds = 0;
        assert_eq!(
            s.autosave_interval(),
            Duration::from_secs(MIN_AUTOSAVE_SECONDS)
        );
        s.autosave_seconds = u64::MAX;
        assert_eq!(
            s.autosave_interval(),
            Duration::from_secs(MAX_AUTOSAVE_SECONDS)
        );
    }

    #[test]
    fn the_ui_scale_is_clamped_to_something_you_can_still_read() {
        let mut s = Settings::default();
        assert_eq!(s.ui_scale(), DEFAULT_UI_SCALE);
        // A hand-edited 0 would draw nothing at all; a 50 would draw one letter.
        s.ui_scale = 0.0;
        assert_eq!(s.ui_scale(), MIN_UI_SCALE);
        s.ui_scale = 50.0;
        assert_eq!(s.ui_scale(), MAX_UI_SCALE);
        s.ui_scale = f32::NAN;
        assert_eq!(
            s.ui_scale(),
            DEFAULT_UI_SCALE,
            "NaN must not escape the clamp"
        );
    }

    #[test]
    fn the_zoom_steps_walk_seventy_five_to_two_hundred_percent() {
        assert_eq!(ui_scale_step(1.0, true), 1.1);
        assert_eq!(ui_scale_step(1.0, false), 0.9);
        assert_eq!(
            ui_scale_step(1.3, false),
            1.25,
            "between steps: the nearest below"
        );
        assert_eq!(
            ui_scale_step(2.0, true),
            MAX_UI_SCALE,
            "the top stays the top"
        );
        assert_eq!(ui_scale_step(0.75, false), MIN_UI_SCALE);
        let mut s = 1.0;
        for _ in 0..20 {
            s = ui_scale_step(s, true);
        }
        assert_eq!(s, 2.0);
    }

    #[test]
    fn audio_settings_become_an_engine_config() {
        let none = AudioSettings::default();
        let config = none.to_config();
        assert_eq!(config, AudioConfig::default());
        assert_eq!(
            none.block_latency_ms(48_000.0),
            None,
            "with no buffer size chosen there is no figure to quote"
        );

        let chosen = AudioSettings {
            device: Some("Speakers".into()),
            sample_rate: Some(48_000),
            buffer_frames: Some(480),
        };
        let config = chosen.to_config();
        assert_eq!(config.device_name.as_deref(), Some("Speakers"));
        assert_eq!(config.sample_rate, Some(48_000));
        assert_eq!(config.buffer_frames, Some(480));
        // 480 frames at 48 kHz is exactly 10 ms.
        assert_eq!(chosen.block_latency_ms(48_000.0), Some(10.0));
    }
}

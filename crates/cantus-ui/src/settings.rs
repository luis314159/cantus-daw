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
    /// The driver type ("WASAPI", "ASIO"); `None` = the system's default (ADR 0025).
    pub driver: Option<String>,
    /// The device recorded from; `None` = the default input, `Some("")` = no input.
    pub input_device: Option<String>,
    /// Recording latency correction in frames, measured by the loopback test or typed.
    pub record_offset_frames: i64,
    /// Cores helping the audio thread (ADR 0031 §2.1): `None` = automatic (performance
    /// cores minus two), `Some(0)` = the audio thread alone.
    pub workers: Option<u32>,
}

impl AudioSettings {
    pub fn to_config(&self) -> AudioConfig {
        AudioConfig {
            driver: self.driver.clone(),
            input_device_name: self.input_device.clone(),
            device_name: self.device.clone(),
            sample_rate: self.sample_rate,
            buffer_frames: self.buffer_frames,
            workers: self.workers.map(|n| n as usize),
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
    /// The user's own sample folders, listed in the sample browser under the packs.
    pub sample_folders: Vec<std::path::PathBuf>,
    /// Play a sample when it is clicked in the browser.
    pub sample_audition: bool,
    /// The verse at start (BACKLOG §6c): on by default, and which Spanish translation.
    pub verse: crate::verse::VerseSettings,
    /// What is remembered per interface, and whether the first-run setup was done
    /// (round 20, `audio_setup::memory`).
    pub audio_setup: crate::audio_setup::memory::AudioMemory,
    /// Checking GitHub for a newer Cantus (ADR 0032).
    pub updates: UpdateSettings,
}

/// The updater's two preferences. The schedule itself (when it last asked) is in
/// `update.json`, not here: it is the program's bookkeeping, not a choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// Ask once a day, at start. On by default; off sends nothing, ever.
    pub check_daily: bool,
    /// A proxy typed by the user, for a network whose proxy Windows does not set.
    pub proxy: Option<String>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings {
            check_daily: true,
            proxy: None,
        }
    }
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
            sample_folders: Vec::new(),
            sample_audition: true,
            verse: crate::verse::VerseSettings::default(),
            audio_setup: Default::default(),
            updates: UpdateSettings::default(),
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
                ..Default::default()
            },
            locale: Some("es".into()),
            autosave_seconds: 30,
            ui_scale: 1.25,
            mixer_density: MixerDensity::Compact,
            mixer_meter_wave: true,
            updates: UpdateSettings {
                check_daily: false,
                proxy: Some("http://proxy.example:8080".into()),
            },
            sample_folders: vec!["D:/Samples".into()],
            sample_audition: false,
            verse: crate::verse::VerseSettings {
                show: false,
                spanish: crate::verse::Spanish::Rv1909,
            },
            ..Default::default()
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
            ..Default::default()
        };
        let config = chosen.to_config();
        assert_eq!(config.device_name.as_deref(), Some("Speakers"));
        assert_eq!(config.sample_rate, Some(48_000));
        assert_eq!(config.buffer_frames, Some(480));
        // 480 frames at 48 kHz is exactly 10 ms.
        assert_eq!(chosen.block_latency_ms(48_000.0), Some(10.0));
    }

    #[test]
    fn the_recording_devices_round_trip_through_the_file() {
        let mut s = Settings {
            audio: AudioSettings {
                driver: Some("ASIO".into()),
                device: Some("Interface".into()),
                input_device: Some("Interface".into()),
                sample_rate: Some(96_000),
                buffer_frames: Some(128),
                record_offset_frames: -42,
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        let config = back.audio.to_config();
        assert_eq!(config.driver.as_deref(), Some("ASIO"));
        assert_eq!(config.input_device_name.as_deref(), Some("Interface"));
        // "No input" is an empty name, and survives the file as one.
        s.audio.input_device = Some(String::new());
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(
            back.audio.to_config().input_device_name.as_deref(),
            Some("")
        );
        // A file from before round 19 has none of them: the defaults.
        let old: Settings = serde_json::from_str(r#"{"audio":{"device":"Speakers"}}"#).unwrap();
        assert_eq!(old.audio.driver, None);
        assert_eq!(old.audio.input_device, None);
        assert_eq!(old.audio.record_offset_frames, 0);
    }
}

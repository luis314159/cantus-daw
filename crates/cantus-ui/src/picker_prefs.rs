//! What the picker remembers for the user (ADR 0029 §5): favourites, the devices used
//! last, how the gallery is sorted and how large its tiles are. Kept in the user's data
//! folder (`devices.json`), never in a project: a favourite is the musician's, not the
//! song's. Devices are named by their stable key (`picker_catalog::Device::key`).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How many recently chosen devices are kept.
pub const RECENT_MAX: usize = 16;

/// How the gallery is ordered.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sort {
    /// Ours in their designed order, by shelf, then hosted plug-ins by name.
    #[default]
    Curated,
    Name,
    Vendor,
    /// The most recently chosen first, the rest by name.
    Recent,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::Curated, Sort::Name, Sort::Vendor, Sort::Recent];

    pub fn key(self) -> &'static str {
        match self {
            Sort::Curated => "picker.sort.curated",
            Sort::Name => "picker.sort.name",
            Sort::Vendor => "picker.sort.vendor",
            Sort::Recent => "picker.sort.recent",
        }
    }
}

/// Tile sizes: small fits more on screen, large shows the device's face better.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl TileSize {
    pub const ALL: [TileSize; 3] = [TileSize::Small, TileSize::Medium, TileSize::Large];

    /// A tile's width at 100 %, in points (its height follows the thumbnail's 16:10).
    pub fn width(self) -> f32 {
        match self {
            TileSize::Small => 168.0,
            TileSize::Medium => 212.0,
            TileSize::Large => 276.0,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            TileSize::Small => "picker.size.small",
            TileSize::Medium => "picker.size.medium",
            TileSize::Large => "picker.size.large",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub favourites: Vec<String>,
    /// Most recent first.
    pub recent: Vec<String>,
    pub sort: Sort,
    pub tile: TileSize,
    /// The details pane shows (it can be folded away for a wider grid).
    pub details: bool,
}

impl Prefs {
    pub fn new() -> Prefs {
        Prefs {
            details: true,
            ..Default::default()
        }
    }

    pub fn is_favourite(&self, key: &str) -> bool {
        self.favourites.iter().any(|k| k == key)
    }

    /// Star or unstar `key`; returns whether it is a favourite now.
    pub fn toggle_favourite(&mut self, key: &str) -> bool {
        if let Some(i) = self.favourites.iter().position(|k| k == key) {
            self.favourites.remove(i);
            false
        } else {
            self.favourites.push(key.to_string());
            true
        }
    }

    /// `key` was just chosen: it goes to the front of the recent list.
    pub fn used(&mut self, key: &str) {
        self.recent.retain(|k| k != key);
        self.recent.insert(0, key.to_string());
        self.recent.truncate(RECENT_MAX);
    }

    /// Its place in the recent list (0 = the last chosen).
    pub fn recency(&self, key: &str) -> Option<usize> {
        self.recent.iter().position(|k| k == key)
    }

    /// Read the file; an absent or unreadable one is a fresh start.
    pub fn load(path: &Path) -> Prefs {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(Prefs::new)
    }

    /// Write the file (through a temporary one, so a crash never leaves half of it).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

/// Where the desktop app keeps them.
pub fn default_path() -> PathBuf {
    crate::app::data_dir().join("devices.json")
}

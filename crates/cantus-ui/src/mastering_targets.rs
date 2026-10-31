//! What a master aims at (round 34, ADR 0084): each platform's published loudness and
//! peak numbers with where they come from, and the tonal balance curves of a few genres.
//!
//! **The platforms.** A streaming service turns every track to its own reference loudness
//! (normalisation); a master louder than that is turned down and gains nothing, one far
//! quieter is turned up only as far as its peaks allow. The ceiling leaves room for the
//! lossy encode's overshoot. Each row says whether the platform publishes the number
//! itself or it is the measured, widely documented one, and when it was checked: these
//! change, so they live in this one table.
//!
//! **The curves.** A long-term octave spectrum relative to the 1 kHz octave, the shape
//! commercial masters of a genre share — ours, drawn after the averages measured over
//! thousands of commercial recordings by Pestana, Ma, Reiss and Barbosa ("Spectral
//! characteristics of popular commercial recordings 1950–2010", AES 135th Convention, paper
//! 8960, 2013) and the round-28 demo's pop reference; a guide within a couple of dB, never a
//! rule. A user's own reference track replaces them.

use cantus_dsp::speculum::ear::OCTAVES;

/// A platform's target.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Platform {
    /// Stable id (`spotify`, `apple_music`…): what an agent and a project store.
    pub id: &'static str,
    /// The name on screen (a proper name, the same in every language).
    pub name: &'static str,
    /// Integrated loudness to aim at, LUFS.
    pub lufs: f64,
    /// The true-peak ceiling, dBTP.
    pub ceiling_dbtp: f32,
    /// Whether the platform publishes the loudness itself (`false`: measured and
    /// documented by others).
    pub published: bool,
    /// Where the numbers come from.
    pub source: &'static str,
    /// When they were checked.
    pub checked: &'static str,
}

/// The platforms, in the order the assistant lists them.
pub const PLATFORMS: &[Platform] = &[
    Platform {
        id: "spotify",
        name: "Spotify",
        lufs: -14.0,
        ceiling_dbtp: -1.0,
        published: true,
        source: "Spotify for Artists, \"Loudness normalization\": −14 LUFS; true peak under −1 dBTP (−2 dBTP for masters louder than −14 LUFS)",
        checked: "2026-10",
    },
    Platform {
        id: "apple_music",
        name: "Apple Music",
        lufs: -16.0,
        ceiling_dbtp: -1.0,
        published: false,
        source: "Sound Check plays near −16 LUFS (AES TD1008, 2021); Apple Digital Masters: true peak at or under −1 dBTP",
        checked: "2026-10",
    },
    Platform {
        id: "youtube",
        name: "YouTube",
        lufs: -14.0,
        ceiling_dbtp: -1.0,
        published: false,
        source: "YouTube does not publish its reference; −14 LUFS is the measured one (AES TD1008 survey)",
        checked: "2026-10",
    },
    Platform {
        id: "tidal",
        name: "TIDAL",
        lufs: -14.0,
        ceiling_dbtp: -1.0,
        published: true,
        source: "TIDAL support, loudness normalisation: −14 LUFS",
        checked: "2026-10",
    },
    Platform {
        id: "amazon_music",
        name: "Amazon Music",
        lufs: -14.0,
        ceiling_dbtp: -2.0,
        published: false,
        source: "Amazon Music normalises near −14 LUFS (measured, AES TD1008 survey); −2 dBTP leaves room for its lossy streams",
        checked: "2026-10",
    },
    Platform {
        id: "soundcloud",
        name: "SoundCloud",
        lufs: -14.0,
        ceiling_dbtp: -1.0,
        published: false,
        source: "SoundCloud does not normalise; it transcodes to lossy formats, so a streaming level with −1 dBTP of room is the usual advice",
        checked: "2026-10",
    },
    Platform {
        id: "cd",
        name: "CD",
        lufs: -10.0,
        ceiling_dbtp: -0.3,
        published: false,
        source: "No loudness standard (Red Book, IEC 60908, only bounds full scale); −10 LUFS is a current commercial CD master, −0.3 dBTP keeps a margin under full scale",
        checked: "2026-10",
    },
    Platform {
        id: "club",
        name: "Club",
        lufs: -8.0,
        ceiling_dbtp: -0.3,
        published: false,
        source: "No standard: dance masters for DJs and club systems sit near −8 LUFS (−6 … −9); a DJ's gain does the rest",
        checked: "2026-10",
    },
    Platform {
        id: "broadcast",
        name: "EBU R128",
        lufs: -23.0,
        ceiling_dbtp: -1.0,
        published: true,
        source: "EBU R 128 (2020): −23 LUFS ±0.5, maximum true peak −1 dBTP",
        checked: "2026-10",
    },
];

/// The platform called `id`.
pub fn platform(id: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

/// A genre's tonal balance.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Curve {
    pub id: &'static str,
    /// Its name's key in the `listen` area.
    pub name_key: &'static str,
    /// dB per octave band (31.5 Hz … 16 kHz) against the 1 kHz octave.
    pub octaves: [f32; OCTAVES],
    /// The loudness range a master of the genre usually keeps, LU (what the glue aims at).
    pub lra: (f32, f32),
    /// Side against mid, dB, of a typical master (how wide it is).
    pub width_db: f32,
}

/// The curves, the first the default.
pub const CURVES: &[Curve] = &[
    Curve {
        id: "pop",
        name_key: "listen.curve.pop",
        octaves: [1.0, 5.0, 4.0, 2.0, 1.0, 0.0, -2.5, -5.0, -8.0, -16.0],
        lra: (4.0, 8.0),
        width_db: -9.0,
    },
    Curve {
        id: "rock",
        name_key: "listen.curve.rock",
        octaves: [-2.0, 3.0, 3.0, 2.0, 1.0, 0.0, -1.5, -3.5, -7.0, -14.0],
        lra: (4.0, 9.0),
        width_db: -10.0,
    },
    Curve {
        id: "electronic",
        name_key: "listen.curve.electronic",
        octaves: [4.0, 8.0, 5.0, 2.0, 0.5, 0.0, -2.0, -4.5, -7.0, -13.0],
        lra: (3.0, 7.0),
        width_db: -8.0,
    },
    Curve {
        id: "hiphop",
        name_key: "listen.curve.hiphop",
        octaves: [6.0, 9.0, 5.0, 2.0, 1.0, 0.0, -2.5, -5.0, -8.0, -15.0],
        lra: (3.0, 7.0),
        width_db: -10.0,
    },
    Curve {
        id: "acoustic",
        name_key: "listen.curve.acoustic",
        octaves: [-6.0, -1.0, 2.0, 2.0, 1.0, 0.0, -2.0, -4.0, -7.0, -14.0],
        lra: (6.0, 12.0),
        width_db: -9.0,
    },
    Curve {
        id: "classical",
        name_key: "listen.curve.classical",
        octaves: [-6.0, -2.0, 1.5, 2.0, 1.5, 0.0, -2.5, -5.5, -9.0, -18.0],
        lra: (8.0, 18.0),
        width_db: -7.0,
    },
];

/// The curve called `id`.
pub fn curve(id: &str) -> Option<&'static Curve> {
    CURVES.iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_is_sane_and_sourced() {
        let mut ids = std::collections::BTreeSet::new();
        for p in PLATFORMS {
            assert!(ids.insert(p.id), "{} twice", p.id);
            assert!((-30.0..=-5.0).contains(&p.lufs), "{}", p.id);
            assert!((-3.0..=0.0).contains(&p.ceiling_dbtp), "{}", p.id);
            assert!(!p.source.is_empty() && !p.checked.is_empty(), "{}", p.id);
        }
        for want in [
            "spotify",
            "apple_music",
            "youtube",
            "tidal",
            "amazon_music",
            "soundcloud",
            "cd",
            "club",
        ] {
            assert!(platform(want).is_some(), "{want}");
        }
        for c in CURVES {
            assert_eq!(c.octaves[5], 0.0, "{}: relative to 1 kHz", c.id);
            assert!(c.lra.0 < c.lra.1, "{}", c.id);
        }
    }
}

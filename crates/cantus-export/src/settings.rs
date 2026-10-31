//! What an export is asked to write: the format and its quality, the rate, the channels,
//! dither, loudness, the tags and what part of the song — and the four presets.
//!
//! Everything here is plain data with serde defaults, so the application keeps the last
//! choices in its settings file and an older file still opens.

use serde::{Deserialize, Serialize};

/// The container and codec.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Format {
    Wav,
    Flac,
    Mp3,
    Ogg,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Wav, Format::Flac, Format::Mp3, Format::Ogg];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Wav => "wav",
            Format::Flac => "flac",
            Format::Mp3 => "mp3",
            Format::Ogg => "ogg",
        }
    }

    /// The name shown on the format's segment (the same in every language).
    pub fn label(self) -> &'static str {
        match self {
            Format::Wav => "WAV",
            Format::Flac => "FLAC",
            Format::Mp3 => "MP3",
            Format::Ogg => "Ogg",
        }
    }

    /// Lossless formats store samples at a bit depth; the others a bitrate.
    pub fn lossless(self) -> bool {
        matches!(self, Format::Wav | Format::Flac)
    }

    /// The highest rate the format is written at, or `None` for no limit. FLAC's encoder
    /// (flacenc) takes 96 kHz at most; a higher rate is written at its family's rate
    /// under it (192 kHz → 96 kHz, 176.4 kHz → 88.2 kHz).
    pub fn max_rate(self) -> Option<u32> {
        match self {
            Format::Flac => Some(96_000),
            _ => None,
        }
    }

    /// Whether a file of this format can be written at `hz`.
    pub fn carries(self, hz: u32) -> bool {
        self.rates().is_none_or(|ok| ok.contains(&hz)) && self.max_rate().is_none_or(|m| hz <= m)
    }

    /// The sample rates the format can carry, or `None` for any.
    pub fn rates(self) -> Option<&'static [u32]> {
        match self {
            // MPEG-1 Layer III: the three rates every player plays.
            Format::Mp3 => Some(&[32_000, 44_100, 48_000]),
            _ => None,
        }
    }
}

/// A lossless format's sample format.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Depth {
    Int16,
    Int24,
    /// 32-bit float (WAV only): the engine's own samples, untouched.
    Float32,
}

impl Depth {
    pub fn bits(self) -> u16 {
        match self {
            Depth::Int16 => 16,
            Depth::Int24 => 24,
            Depth::Float32 => 32,
        }
    }

    pub fn is_int(self) -> bool {
        !matches!(self, Depth::Float32)
    }
}

/// How an MP3 spends its bits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mp3Mode {
    /// Constant bitrate, kbps (one of [`MP3_BITRATES`]).
    Cbr(u16),
    /// LAME's variable bitrate, V0 (best) … V9.
    Vbr(u8),
}

/// The constant bitrates offered, kbps.
pub const MP3_BITRATES: [u16; 6] = [128, 160, 192, 224, 256, 320];
/// The VBR levels offered (LAME's V0, V2, V4, V6).
pub const MP3_VBR: [u8; 4] = [0, 2, 4, 6];
/// The Ogg Vorbis qualities offered (`-q`, 0–10).
pub const OGG_QUALITIES: [u8; 6] = [3, 4, 5, 6, 8, 10];
/// The sample rates offered beside the session's.
pub const RATES: [u32; 5] = [44_100, 48_000, 88_200, 96_000, 192_000];

/// The sample rate of the file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rate {
    /// The engine's own rate: nothing is converted.
    Session,
    Hz(u32),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channels {
    Stereo,
    /// The two sides summed at half each: a centred sound keeps its level.
    Mono,
}

/// Dither when samples are cut to 16 or 24 bits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dither {
    Off,
    /// Flat TPDF: the error made noise, uncorrelated with the music.
    Tpdf,
    /// TPDF with Lipshitz's noise shaping: the noise moved where the ear is least
    /// sensitive (Terminus's *Optimized*).
    Shaped,
}

/// Loudness normalisation (ITU-R BS.1770-4, EBU R128 gating).
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Loudness {
    /// The integrated loudness to land on, LUFS; `None` leaves the level as mixed.
    pub target_lufs: Option<f32>,
    /// The highest true peak allowed, dBTP.
    pub ceiling_dbtp: f32,
    /// When the gain would push a peak over the ceiling: a transparent true-peak limiter
    /// (Terminus) holds it there and the loudness still lands; off, the gain is lowered
    /// instead and the file comes out quieter than asked.
    pub limit: bool,
}

impl Default for Loudness {
    fn default() -> Self {
        Loudness {
            target_lufs: None,
            ceiling_dbtp: -1.0,
            limit: true,
        }
    }
}

/// The loudness targets offered, LUFS.
pub const LUFS_TARGETS: [f32; 6] = [-9.0, -11.0, -14.0, -16.0, -19.0, -23.0];
/// The ceilings offered, dBTP.
pub const CEILINGS: [f32; 4] = [-0.1, -0.3, -1.0, -2.0];

/// What part of the piece is written.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// The whole song, to its end plus the tail.
    Song,
    /// The arrangement's time selection.
    Selection,
    /// The loop brace (round 23): FL's pattern loop, Ableton's and Logic's cycle.
    Loop,
    /// Every strip (or track) as a file of its own (ADR 0021 §3).
    Stems,
    /// Between two markers (round 29, ADR 0060), by their ids (`cantus_core::MarkerId`):
    /// from the earlier to the later.
    Markers { from: u64, to: u64 },
}

/// What a stems export takes (ADR 0021 §3, ADR 0026); everything off by default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StemChoice {
    /// Tracks rather than strips.
    pub tracks: bool,
    /// The master mix as `00` too.
    pub master: bool,
    pub keep_silent: bool,
    pub through_master: bool,
}

/// The text written into the file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: String,
    pub comment: String,
}

/// The presets: each sets every choice it names, and the dialog shows which one the
/// current choices still match.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Preset {
    /// For Spotify, Apple Music, YouTube, SoundCloud: −14 LUFS, −1 dBTP, MP3 320 kbps.
    Streaming,
    /// Red Book: 16-bit, 44.1 kHz WAV, shaped dither, the level as mastered.
    Cd,
    /// For a mastering engineer: every strip as a 24-bit WAV at the session's rate,
    /// nothing normalised, nothing through the master chain.
    MasteringStems,
    /// Spoken word: −16 LUFS, −1 dBTP, mono MP3 at 128 kbps, 44.1 kHz.
    Podcast,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::Streaming,
        Preset::Cd,
        Preset::MasteringStems,
        Preset::Podcast,
    ];

    /// The preset's name and line, as locale keys.
    pub fn keys(self) -> (&'static str, &'static str) {
        match self {
            Preset::Streaming => (
                "export_audio.preset.streaming",
                "export_audio.preset.streaming_hint",
            ),
            Preset::Cd => ("export_audio.preset.cd", "export_audio.preset.cd_hint"),
            Preset::MasteringStems => (
                "export_audio.preset.stems",
                "export_audio.preset.stems_hint",
            ),
            Preset::Podcast => (
                "export_audio.preset.podcast",
                "export_audio.preset.podcast_hint",
            ),
        }
    }

    /// `s` with this preset's choices; the tags, the folder and the tail are kept.
    pub fn apply(self, s: &ExportSettings) -> ExportSettings {
        let mut n = s.clone();
        n.dither = Dither::Tpdf;
        n.channels = Channels::Stereo;
        n.loudness = Loudness::default();
        match self {
            Preset::Streaming => {
                if n.scope == Scope::Stems {
                    n.scope = Scope::Song;
                }
                n.format = Format::Mp3;
                n.mp3 = Mp3Mode::Cbr(320);
                n.rate = Rate::Hz(44_100);
                n.loudness.target_lufs = Some(-14.0);
                n.loudness.ceiling_dbtp = -1.0;
            }
            Preset::Cd => {
                if n.scope == Scope::Stems {
                    n.scope = Scope::Song;
                }
                n.format = Format::Wav;
                n.depth = Depth::Int16;
                n.rate = Rate::Hz(44_100);
                n.dither = Dither::Shaped;
            }
            Preset::MasteringStems => {
                n.scope = Scope::Stems;
                n.format = Format::Wav;
                n.depth = Depth::Int24;
                n.rate = Rate::Session;
                n.stems.through_master = false;
                n.stems.tracks = false;
            }
            Preset::Podcast => {
                if n.scope == Scope::Stems {
                    n.scope = Scope::Song;
                }
                n.format = Format::Mp3;
                n.mp3 = Mp3Mode::Cbr(128);
                n.rate = Rate::Hz(44_100);
                n.channels = Channels::Mono;
                n.loudness.target_lufs = Some(-16.0);
                n.loudness.ceiling_dbtp = -1.0;
            }
        }
        n
    }

    /// Whether `s` still has every choice this preset makes (the ones the dialog shows).
    pub fn matches(self, s: &ExportSettings) -> bool {
        let applied = self.apply(s);
        let same_scope = match self {
            Preset::MasteringStems => s.scope == Scope::Stems,
            _ => s.scope != Scope::Stems,
        };
        // What a stems export takes is part of the stems preset (strips, not through the
        // master chain).
        let same_stems = s.scope != Scope::Stems
            || (applied.stems.tracks == s.stems.tracks
                && applied.stems.through_master == s.stems.through_master);
        same_scope && same_stems && applied.audio_eq(s)
    }
}

/// Every choice of the export dialog: what the application remembers between exports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportSettings {
    pub scope: Scope,
    pub format: Format,
    /// WAV's sample format; FLAC takes the integer ones (32-bit float becomes 24).
    pub depth: Depth,
    pub mp3: Mp3Mode,
    /// Ogg Vorbis quality, 0–10.
    pub ogg_quality: u8,
    pub rate: Rate,
    pub channels: Channels,
    pub dither: Dither,
    pub loudness: Loudness,
    /// Seconds after the music so releases and reverbs ring out.
    pub tail_seconds: f32,
    pub stems: StemChoice,
    /// The artist, album, year and comment are remembered; the title is the song's.
    pub tags: Tags,
    /// Where the last file went.
    pub folder: Option<std::path::PathBuf>,
    /// Never replace a file: each output is put in place by a link that fails if anything
    /// is there (an agent's export without leave to overwrite, round 34). Not remembered.
    #[serde(skip)]
    pub no_replace: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        ExportSettings {
            scope: Scope::Song,
            format: Format::Mp3,
            depth: Depth::Int24,
            mp3: Mp3Mode::Cbr(320),
            ogg_quality: 6,
            rate: Rate::Session,
            channels: Channels::Stereo,
            dither: Dither::Tpdf,
            loudness: Loudness::default(),
            tail_seconds: 2.0,
            stems: StemChoice::default(),
            tags: Tags::default(),
            folder: None,
            no_replace: false,
        }
    }
}

impl ExportSettings {
    /// The same audio choices (format, quality, rate, channels, dither, loudness),
    /// whatever the scope, tags and folder. A choice the dialog hides is not compared:
    /// dither when nothing is cut to integers, the loudness of an export that is not
    /// normalised (stems, or the target off — its ceiling is hidden with it).
    pub fn audio_eq(&self, o: &ExportSettings) -> bool {
        let quality = match self.format {
            Format::Wav => self.depth == o.depth,
            Format::Flac => self.flac_depth() == o.flac_depth(),
            Format::Mp3 => self.mp3 == o.mp3,
            Format::Ogg => self.ogg_quality == o.ogg_quality,
        };
        let dither = !self.dithers() || self.dither == o.dither;
        let loudness = self.normalises() == o.normalises()
            && (!self.normalises() || self.loudness == o.loudness);
        self.format == o.format
            && quality
            && self.rate == o.rate
            && self.channels == o.channels
            && dither
            && loudness
    }

    /// The first preset whose choices these are.
    pub fn preset(&self) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.matches(self))
    }

    /// FLAC's integer depth.
    pub fn flac_depth(&self) -> Depth {
        match self.depth {
            Depth::Int16 => Depth::Int16,
            _ => Depth::Int24,
        }
    }

    /// The sample format written, for a lossless format.
    pub fn int_bits(&self) -> Option<u16> {
        match self.format {
            Format::Wav => self.depth.is_int().then(|| self.depth.bits()),
            Format::Flac => Some(self.flac_depth().bits()),
            _ => None,
        }
    }

    /// Whether the samples are cut to integers here (and so whether dither applies).
    pub fn dithers(&self) -> bool {
        self.int_bits().is_some()
    }

    /// The rate the file is written at, for a session running at `session` Hz: the one
    /// asked for, or — for a format that cannot carry it — the nearest it can, preferring
    /// one above.
    pub fn file_rate(&self, session: u32) -> u32 {
        let mut want = match self.rate {
            Rate::Session => session,
            Rate::Hz(hz) => hz,
        }
        .max(1);
        if let Some(max) = self.format.max_rate() {
            // Halved until it fits: the same family, the highest rate under the limit.
            while want > max {
                want = want.div_ceil(2);
            }
        }
        match self.format.rates() {
            None => want,
            Some(list) if list.contains(&want) => want,
            Some(list) => {
                // The same family (44.1 or 48 kHz) when the rate is a multiple of one.
                let family = if want.is_multiple_of(11_025) {
                    44_100
                } else {
                    48_000
                };
                if list.contains(&family) && want > family {
                    family
                } else {
                    *list
                        .iter()
                        .min_by_key(|r| (**r as i64 - want as i64).abs())
                        .unwrap_or(&48_000)
                }
            }
        }
    }

    pub fn channel_count(&self) -> u16 {
        match self.channels {
            Channels::Stereo => 2,
            Channels::Mono => 1,
        }
    }

    /// Loudness applies to a mix, never to stems: stems normalised one by one would no
    /// longer add up to the mix.
    pub fn normalises(&self) -> bool {
        self.scope != Scope::Stems && self.loudness.target_lufs.is_some()
    }
}

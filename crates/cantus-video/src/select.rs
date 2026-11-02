//! Which encoders an MP4 uses on this system (ADR 0088 §1), said before the export starts
//! (the dialog shows it) and used by it:
//!
//! | | H.264 | AAC |
//! |---|---|---|
//! | Windows | Media Foundation | Media Foundation |
//! | Windows N | Cisco's OpenH264, if downloaded | the system's FFmpeg/GStreamer, else MP3 |
//! | Linux | Cisco's OpenH264, if downloaded | the system's FFmpeg/GStreamer, else MP3 |
//! | macOS | VideoToolbox | AudioToolbox |

use crate::audio::AudioEncoder;
use crate::audio_alt::{self, Tool};
use crate::h264::H264Encoder;
use crate::VideoError;
use std::path::PathBuf;

/// What the person allowed, and what is installed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Codecs {
    /// Cisco's OpenH264 library, downloaded and switched on (`None`: not offered).
    pub openh264: Option<PathBuf>,
    /// Use an AAC encoder the system has (FFmpeg, GStreamer) where the OS has none.
    pub system_aac: bool,
}

/// Where the H.264 comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoPath {
    MediaFoundation,
    OpenH264(PathBuf),
    VideoToolbox,
}

/// Where the audio comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioPath {
    MediaFoundation,
    AudioToolbox,
    System(Tool),
    /// MP3 in the MP4: plays everywhere tested, but Instagram takes AAC only.
    Mp3,
}

/// What an MP4 export would use here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// `None`: no H.264 encoder (Linux or Windows N before OpenH264 is downloaded).
    pub video: Option<VideoPath>,
    pub audio: AudioPath,
}

/// Something the person should know about a finished export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// The MP4's audio is MP3 (no AAC encoder on this system).
    Mp3Audio,
}

/// The system's own encoders, as far as they exist.
fn os_paths() -> (Option<VideoPath>, Option<AudioPath>) {
    #[cfg(windows)]
    {
        let (v, a) = crate::mf::available();
        (
            v.then_some(VideoPath::MediaFoundation),
            a.then_some(AudioPath::MediaFoundation),
        )
    }
    #[cfg(target_os = "macos")]
    {
        (Some(VideoPath::VideoToolbox), Some(AudioPath::AudioToolbox))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        (None, None)
    }
}

/// What an MP4 export would use with `codecs`.
pub fn plan(codecs: &Codecs) -> Plan {
    let (v, a) = os_paths();
    let video = v.or_else(|| codecs.openh264.clone().map(VideoPath::OpenH264));
    let audio = a.unwrap_or_else(|| {
        codecs
            .system_aac
            .then(audio_alt::find_aac)
            .flatten()
            .map_or(AudioPath::Mp3, AudioPath::System)
    });
    Plan { video, audio }
}

/// The encoders `plan` names, made for one export.
pub struct Picked {
    pub video: Box<dyn H264Encoder>,
    pub audio: Box<dyn AudioEncoder>,
    pub warnings: Vec<Warning>,
}

/// Make the encoders for a `width` × `height` MP4 at `fps`, `bitrate`, audio at `rate`.
pub fn make(
    plan: &Plan,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    rate: u32,
) -> Result<Picked, VideoError> {
    let video: Box<dyn H264Encoder> = match &plan.video {
        None => return Err(VideoError::NoEncoder("H.264")),
        Some(VideoPath::OpenH264(lib)) => {
            Box::new(crate::openh264::OpenH264::new(lib, fps, bitrate)?)
        }
        #[cfg(windows)]
        Some(VideoPath::MediaFoundation) => {
            Box::new(crate::mf::MfH264::new(width, height, fps, bitrate)?)
        }
        #[cfg(target_os = "macos")]
        Some(VideoPath::VideoToolbox) => {
            Box::new(crate::apple::VtH264::new(width, height, fps, bitrate)?)
        }
        #[allow(unreachable_patterns)]
        Some(other) => {
            return Err(VideoError::Unsupported(format!("{other:?} on this system")));
        }
    };
    let mut warnings = Vec::new();
    let audio: Box<dyn AudioEncoder> = match &plan.audio {
        AudioPath::System(tool) => Box::new(audio_alt::PipeAac::new(tool.clone(), rate, 2)?),
        AudioPath::Mp3 => {
            warnings.push(Warning::Mp3Audio);
            Box::new(audio_alt::Mp3::new(rate)?)
        }
        #[cfg(windows)]
        AudioPath::MediaFoundation => Box::new(crate::mf::MfAac::new(rate, 2)?),
        #[cfg(target_os = "macos")]
        AudioPath::AudioToolbox => Box::new(crate::apple::AtAac::new(rate, 2)?),
        #[allow(unreachable_patterns)]
        other => return Err(VideoError::Unsupported(format!("{other:?} on this system"))),
    };
    let _ = (width, height);
    Ok(Picked {
        video,
        audio,
        warnings,
    })
}

//! Cisco's OpenH264, downloaded on demand (ADR 0088 §1b): Linux's H.264, and a choice on
//! Windows N.
//!
//! Cisco pays the H.264 royalty for **its own binary** when each user downloads it from
//! Cisco; a build of the same source by Cantus would owe it. So the library is never in a
//! package: the content manager fetches the `.bz2` from `ciscobinary.openh264.org` (https,
//! the operating system's certificates, a proxy's HTML page refused — ADR 0030), checks
//! the download's SHA-256 against the value pinned here, unpacks it with a size bound,
//! checks the library's SHA-256 against `openh264-sys2`'s list of Cisco's builds, and only
//! then loads it. The person can switch it off (the dialog), and Cisco's words —
//! [`CREDIT`] — are shown wherever it is offered.
//!
//! Cisco's binary refuses a non-real-time usage at start-up (measured by the probe), so the
//! encoder runs as `CameraVideoRealTime`, with frame skipping off: one access unit a frame.

use crate::h264::H264Encoder;
use crate::yuv::Nv12;
use crate::VideoError;
use cantus_content::fetch::{self, Transport};
use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, Profile,
    RateControlMode, UsageType, VuiConfig,
};
use openh264::formats::YUVSource;
use openh264::OpenH264API;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// How many times a library has been hashed (the tests count it).
pub static HASHED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Cisco's words, shown with the switch and in the about box (its licence asks for them).
pub const CREDIT: &str = "OpenH264 Video Codec provided by Cisco Systems, Inc.";

/// Cisco's licence for the binary, linked from the dialog.
pub const LICENCE_URL: &str = "https://www.openh264.org/BINARY_LICENSE.txt";

/// The version downloaded (the one `openh264-sys2` 0.9 drives).
pub const VERSION: &str = "2.6.0";

/// One system's binary: the `.bz2` Cisco serves and the library inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blob {
    /// `std::env::consts::{OS}-{ARCH}`.
    pub platform: &'static str,
    pub file: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    /// The unpacked library's name and SHA-256 (Cisco's build, as `openh264-sys2` lists it).
    pub library: &'static str,
    pub library_sha256: &'static str,
}

/// Cisco's 2.6.0 builds Cantus may fetch, measured 2026-10-31 (downloaded over https).
pub const BLOBS: [Blob; 3] = [
    Blob {
        platform: "linux-x86_64",
        file: "libopenh264-2.6.0-linux64.8.so.bz2",
        size: 634_264,
        sha256: "27ab53323c110b76214c1c72222f459d17febbcd1e252136cadc292b0308d75b",
        library: "libopenh264-2.6.0-linux64.8.so",
        library_sha256: "2f0cde7c6a6abcf5cae76942894ea42897fa677bce4ed6c91a24dd1b041d5f04",
    },
    Blob {
        platform: "linux-aarch64",
        file: "libopenh264-2.6.0-linux-arm64.8.so.bz2",
        size: 619_371,
        sha256: "a78aea7970150f46bcd3bb7994c9e6dd90bd7a9ea785920f5a73f6964e3fcda7",
        library: "libopenh264-2.6.0-linux-arm64.8.so",
        library_sha256: "12e7b33623667cdab0e575170c147b1b36eadb77d0d2aa7ceb5afd3e58902140",
    },
    Blob {
        platform: "windows-x86_64",
        file: "openh264-2.6.0-win64.dll.bz2",
        size: 452_053,
        sha256: "dab5f2a872777f9a58b69bfa9fbcf20d9f82f2d6ec91383fd70bff49bd34ac9f",
        library: "openh264-2.6.0-win64.dll",
        library_sha256: "2076cb5675ec6c1a4c70e7a2a322552f547b6eeed649d6dfcd9e02a543b24691",
    },
];

/// The largest library unpacked (Cisco's are ~1 MB): a bound before memory is given.
const MAX_LIBRARY: u64 = 16 << 20;

/// This system's binary, if Cisco builds one for it.
pub fn blob() -> Option<&'static Blob> {
    blob_for(&format!(
        "{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ))
}

pub fn blob_for(platform: &str) -> Option<&'static Blob> {
    BLOBS.iter().find(|b| b.platform == platform)
}

/// Where Cisco serves `b`.
pub fn url(b: &Blob) -> String {
    format!("https://ciscobinary.openh264.org/{}", b.file)
}

/// The codec's folder in the person's data (`<data>/Codecs/OpenH264/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub dir: PathBuf,
    pub blob: Blob,
}

impl Install {
    /// This system's install under `dir`; `None` where Cisco has no binary (macOS uses
    /// VideoToolbox).
    pub fn new(dir: impl Into<PathBuf>) -> Option<Install> {
        Some(Install {
            dir: dir.into(),
            blob: *blob()?,
        })
    }

    /// The library, if it is there and is Cisco's. Its hash is checked once for each
    /// size and modification time the file has (asked every repaint by the dialog: a
    /// megabyte hashed there each frame would block it — review 1 #4); the loader checks
    /// it again as it loads.
    pub fn library(&self) -> Option<PathBuf> {
        use std::collections::HashMap;
        use std::sync::Mutex;
        type Seen = HashMap<PathBuf, (u64, std::time::SystemTime, bool)>;
        static SEEN: Mutex<Option<Seen>> = Mutex::new(None);
        let p = self.dir.join(self.blob.library);
        let m = std::fs::metadata(&p).ok()?;
        let stamp = (m.len(), m.modified().ok()?);
        let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        let seen = seen.get_or_insert_with(HashMap::new);
        let ok = match seen.get(&p) {
            Some(&(len, at, ok)) if (len, at) == stamp => ok,
            _ => {
                HASHED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let ok = fetch::sha256_file(&p).is_ok_and(|h| h == self.blob.library_sha256);
                seen.insert(p.clone(), (stamp.0, stamp.1, ok));
                ok
            }
        };
        ok.then_some(p)
    }

    /// Fetch, check and unpack the library from `url` (Cisco's, or a `file://` copy in the
    /// tests); `progress(bytes, of)` as it arrives.
    pub fn install(
        &self,
        t: &dyn Transport,
        url: &str,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<PathBuf, VideoError> {
        let b = &self.blob;
        std::fs::create_dir_all(&self.dir)?;
        let packed = self.dir.join(b.file);
        let mut part = packed.clone().into_os_string();
        part.push(".part");
        let mut each = |n| progress(n, b.size);
        fetch::download(
            t,
            url,
            b.size,
            b.sha256,
            Path::new(&part),
            &packed,
            cancel,
            &mut each,
        )
        .map_err(|e| match e {
            fetch::FetchError::Cancelled => VideoError::Cancelled,
            e => VideoError::encoder("OpenH264", format!("the download: {e}")),
        })?;
        let mut library = Vec::new();
        let read = bzip2_rs::DecoderReader::new(std::fs::File::open(&packed)?)
            .take(MAX_LIBRARY + 1)
            .read_to_end(&mut library);
        let _ = std::fs::remove_file(&packed);
        read.map_err(|e| VideoError::encoder("OpenH264", format!("unpacking: {e}")))?;
        if library.len() as u64 > MAX_LIBRARY {
            return Err(VideoError::encoder("OpenH264", "the library is too large"));
        }
        let got = fetch::hex(&sha2_of(&library));
        if got != b.library_sha256 {
            return Err(VideoError::encoder(
                "OpenH264",
                format!("the library is not Cisco's (SHA-256 {got})"),
            ));
        }
        let dest = self.dir.join(b.library);
        let mut tmp = dest.clone().into_os_string();
        tmp.push(".part");
        std::fs::write(&tmp, &library)?;
        std::fs::rename(&tmp, &dest)?;
        Ok(dest)
    }

    /// Cisco's words while its library is here (the About box shows them).
    pub fn credit(&self) -> Option<&'static str> {
        self.library().map(|_| CREDIT)
    }

    /// Remove the library (the switch off, or a fresh start).
    pub fn remove(&self) -> std::io::Result<()> {
        match std::fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}

fn sha2_of(bytes: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).to_vec()
}

/// NV12 as the planar 4:2:0 OpenH264 takes.
struct Planar {
    w: usize,
    h: usize,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl Planar {
    fn fill(&mut self, f: &Nv12) {
        self.w = f.width as usize;
        self.h = f.height as usize;
        self.y.clear();
        self.y.extend_from_slice(f.y());
        self.u.clear();
        self.v.clear();
        for uv in f.uv().chunks_exact(2) {
            self.u.push(uv[0]);
            self.v.push(uv[1]);
        }
    }
}

impl YUVSource for Planar {
    fn dimensions(&self) -> (usize, usize) {
        (self.w, self.h)
    }
    fn strides(&self) -> (usize, usize, usize) {
        (self.w, self.w / 2, self.w / 2)
    }
    fn y(&self) -> &[u8] {
        &self.y
    }
    fn u(&self) -> &[u8] {
        &self.u
    }
    fn v(&self) -> &[u8] {
        &self.v
    }
}

/// Cisco's OpenH264 as the export's H.264 encoder.
pub struct OpenH264 {
    enc: Encoder,
    planar: Planar,
}

impl OpenH264 {
    /// An encoder from the library at `library` (its hash checked again as it loads).
    pub fn new(library: &Path, fps: u32, bitrate: u32) -> Result<OpenH264, VideoError> {
        let api = OpenH264API::from_blob_path(library)
            .map_err(|e| VideoError::encoder("OpenH264", format!("loading: {e}")))?;
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(8);
        let cfg = EncoderConfig::new()
            .bitrate(BitRate::from_bps(bitrate))
            .max_frame_rate(FrameRate::from_hz(fps as f32))
            .usage_type(UsageType::CameraVideoRealTime)
            .rate_control_mode(RateControlMode::Bitrate)
            .profile(Profile::High)
            .complexity(Complexity::High)
            .skip_frames(false)
            .intra_frame_period(IntraFramePeriod::from_num_frames(fps * 2))
            .num_threads(threads as u16)
            .vui(VuiConfig::bt709());
        let enc =
            Encoder::with_api_config(api, cfg).map_err(|e| VideoError::encoder("OpenH264", e))?;
        Ok(OpenH264 {
            enc,
            planar: Planar {
                w: 0,
                h: 0,
                y: Vec::new(),
                u: Vec::new(),
                v: Vec::new(),
            },
        })
    }
}

impl H264Encoder for OpenH264 {
    fn name(&self) -> String {
        format!("OpenH264 {VERSION} (Cisco)")
    }

    fn encode(&mut self, frame: &Nv12, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.planar.fill(frame);
        let bits = self
            .enc
            .encode(&self.planar)
            .map_err(|e| VideoError::encoder("OpenH264", e))?;
        let au = bits.to_vec();
        if au.is_empty() {
            return Err(VideoError::encoder("OpenH264", "a frame was skipped"));
        }
        out.push(au);
        Ok(())
    }

    fn finish(&mut self, _out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        // Real-time usage keeps nothing back: every frame came out as it went in.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_blob_is_https_and_its_library_is_one_openh264_sys2_knows() {
        let known = include_str!("../tests/data/openh264-sys2-hashes.txt");
        for b in BLOBS {
            assert!(url(&b).starts_with("https://ciscobinary.openh264.org/"));
            assert!(b.file.starts_with(b.library) && b.file.ends_with(".bz2"));
            assert!(known.contains(b.library_sha256), "{}", b.library);
            assert_eq!(b.sha256.len(), 64);
        }
        assert_eq!(blob_for("macos-aarch64"), None);
    }

    #[test]
    fn a_library_is_hashed_once_until_it_changes_and_removing_it_is_clean() {
        let dir = tempfile::tempdir().unwrap();
        let Some(inst) = Install::new(dir.path().join("OpenH264")) else {
            return;
        };
        std::fs::create_dir_all(&inst.dir).unwrap();
        let lib = inst.dir.join(inst.blob.library);
        std::fs::write(&lib, b"not Cisco's").unwrap();
        let before = HASHED.load(std::sync::atomic::Ordering::Relaxed);
        for _ in 0..5 {
            assert_eq!(inst.library(), None);
        }
        let after = HASHED.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(after - before, 1, "hashed once for five asks");
        // Another size: checked again.
        std::fs::write(&lib, b"still not Cisco's, longer").unwrap();
        assert_eq!(inst.library(), None);
        assert_eq!(HASHED.load(std::sync::atomic::Ordering::Relaxed) - after, 1);
        inst.remove().unwrap();
        assert!(!inst.dir.exists());
        inst.remove().unwrap();
    }
}

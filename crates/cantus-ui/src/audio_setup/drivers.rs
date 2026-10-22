//! Getting a low-latency driver (round 20, the owner's request of 2026-10-06).
//!
//! Three ways, offered when no ASIO driver is installed and always under *Get a
//! low-latency driver*:
//!
//! 1. **FlexASIO** (MIT, github.com/dechamps/FlexASIO) — its latest release page, opened
//!    in the browser. The parsing of GitHub's release answer and the verification of a
//!    downloaded installer (a Windows program — "MZ" — of exactly the size GitHub
//!    reports; an HTML page, which a proxy serves while it holds a download for
//!    inspection, refused) are here and tested, for an in-app install once it is
//!    approved; Cantus does not download or run installers by itself today.
//! 2. **ASIO4ALL** — a link only: its licence does not let Cantus redistribute it.
//! 3. **The interface's own driver** — the makers' driver pages, the detected maker first.

use serde::Deserialize;

/// Where GitHub says what FlexASIO's latest release is.
pub const FLEXASIO_API: &str = "https://api.github.com/repos/dechamps/FlexASIO/releases/latest";
pub const FLEXASIO_PAGE: &str = "https://github.com/dechamps/FlexASIO/releases/latest";
pub const ASIO4ALL_URL: &str = "https://asio4all.org";

/// FlexASIO's latest release: its tag and the installer to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub asset: String,
    pub url: String,
    pub size: u64,
    /// The `.sigstore` bundle published next to the installer, when there is one.
    pub sigstore: Option<String>,
}

/// What can go wrong getting it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchError {
    /// An HTML page came back instead (the proxy holding the file for inspection).
    Quarantined,
    /// What came back is not a Windows program.
    NotInstaller,
    /// The file is not the size GitHub reports.
    Size { got: u64, want: u64 },
    /// GitHub's answer could not be read, or names no installer.
    Parse(String),
    /// The network failed.
    Network(String),
}

impl FetchError {
    /// Its sentence, for the status line under the button.
    pub fn text(&self) -> String {
        use crate::i18n::{t, tf};
        match self {
            FetchError::Quarantined => t("devices.drivers.quarantined"),
            FetchError::NotInstaller => t("devices.drivers.not_installer"),
            FetchError::Size { got, want } => {
                tf("devices.drivers.size", &[("got", got), ("want", want)])
            }
            FetchError::Parse(e) | FetchError::Network(e) => {
                tf("devices.drivers.network", &[("error", e)])
            }
        }
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Whether bytes are an HTML page (what a proxy or a captive portal answers with).
pub fn looks_like_html(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(512)];
    let text = String::from_utf8_lossy(head).to_lowercase();
    let t = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    t.starts_with('<') || text.contains("<html") || text.contains("<!doctype")
}

/// Read GitHub's answer for the latest release: the `.exe` installer and its size.
pub fn parse_release(body: &[u8]) -> Result<Release, FetchError> {
    if looks_like_html(body) {
        return Err(FetchError::Quarantined);
    }
    let r: ApiRelease =
        serde_json::from_slice(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    let exe = r
        .assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".exe"))
        .ok_or_else(|| FetchError::Parse(format!("{}: no installer", r.tag_name)))?;
    let sigstore = r
        .assets
        .iter()
        .find(|a| a.name.starts_with(&exe.name) && a.name.ends_with(".sigstore"))
        .map(|a| a.browser_download_url.clone());
    Ok(Release {
        tag: r.tag_name.clone(),
        asset: exe.name.clone(),
        url: exe.browser_download_url.clone(),
        size: exe.size,
        sigstore,
    })
}

/// Check a downloaded installer: not a page, a Windows program, the size GitHub said.
pub fn verify_installer(bytes: &[u8], size: u64) -> Result<(), FetchError> {
    if looks_like_html(bytes) {
        return Err(FetchError::Quarantined);
    }
    if !bytes.starts_with(b"MZ") {
        return Err(FetchError::NotInstaller);
    }
    if bytes.len() as u64 != size {
        return Err(FetchError::Size {
            got: bytes.len() as u64,
            want: size,
        });
    }
    Ok(())
}

/// A maker of interfaces and where its drivers are.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Maker {
    pub name: &'static str,
    pub url: &'static str,
    /// Words in a device's name that say it is theirs.
    pub words: &'static [&'static str],
}

pub const MAKERS: [Maker; 13] = [
    Maker {
        name: "Behringer (UMC)",
        url: "https://www.behringer.com/downloads.html",
        words: &["behringer", "umc", "uphoria"],
    },
    Maker {
        name: "Focusrite",
        url: "https://downloads.focusrite.com",
        words: &["focusrite", "scarlett", "clarett", "vocaster"],
    },
    Maker {
        name: "PreSonus",
        url: "https://www.presonus.com/pages/downloads",
        words: &[
            "presonus",
            "audiobox",
            "studio 24c",
            "studio 26c",
            "quantum",
        ],
    },
    Maker {
        name: "Steinberg",
        url: "https://www.steinberg.net/downloads/",
        words: &["steinberg", "ur22", "ur12", "ur44", "ixo"],
    },
    Maker {
        name: "MOTU",
        url: "https://motu.com/download",
        words: &["motu", "m2", "m4", "ultralite"],
    },
    Maker {
        name: "Universal Audio",
        url: "https://www.uaudio.com/downloads",
        words: &["universal audio", "apollo", "volt", "uad"],
    },
    Maker {
        name: "Audient",
        url: "https://audient.com/products/downloads/",
        words: &["audient", "id4", "id14", "id22", "evo"],
    },
    Maker {
        name: "M-Audio",
        url: "https://m-audio.com/support/drivers",
        words: &["m-audio", "m-track", "air 192"],
    },
    Maker {
        name: "Native Instruments",
        url: "https://www.native-instruments.com/en/support/downloads/drivers-other-files/",
        words: &["native instruments", "komplete audio", "traktor audio"],
    },
    Maker {
        name: "RME",
        url: "https://www.rme-audio.de/downloads.html",
        words: &["rme", "babyface", "fireface", "madiface"],
    },
    Maker {
        name: "Arturia",
        url: "https://www.arturia.com/support/downloads-manuals",
        words: &["arturia", "minifuse", "audiofuse"],
    },
    Maker {
        name: "Roland / BOSS",
        url: "https://www.roland.com/global/support/",
        words: &["roland", "boss", "rubix", "katana", "gt-1000"],
    },
    Maker {
        name: "Yamaha",
        url: "https://usa.yamaha.com/support/updates/",
        words: &["yamaha", "steinberg usb", "ag03", "ag06", "urx"],
    },
];

/// The makers, the one a device name points at first (`detected` says which, if any).
pub fn makers_for(device_names: &[&str]) -> (Vec<&'static Maker>, Option<&'static str>) {
    let names: Vec<String> = device_names.iter().map(|n| n.to_lowercase()).collect();
    let words: Vec<Vec<&str>> = names
        .iter()
        .map(|n| {
            n.split(|c: char| !c.is_alphanumeric() && c != '-')
                .filter(|w| !w.is_empty())
                .collect()
        })
        .collect();
    let says = |w: &str| {
        if w.contains(' ') {
            names.iter().any(|n| n.contains(w))
        } else {
            words
                .iter()
                .flatten()
                .any(|x| *x == w || (w.len() >= 3 && x.starts_with(w)))
        }
    };
    let hit = MAKERS.iter().find(|m| m.words.iter().any(|w| says(w)));
    let mut v: Vec<&'static Maker> = Vec::with_capacity(MAKERS.len());
    if let Some(m) = hit {
        v.push(m);
    }
    v.extend(MAKERS.iter().filter(|m| Some(*m) != hit));
    (v, hit.map(|m| m.name))
}

/// Open a web page in the user's browser.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        log::warn!("cannot open {url}: {e}");
    }
}

/// What `cantus --audio-drivers` prints: the driver types this build offers and whether
/// the ASIO SDK is linked in (`xtask package` refuses a release without ASIO by reading it).
pub fn report() -> String {
    let names: Vec<String> = cantus_engine::devices::drivers()
        .into_iter()
        .map(|d| d.name)
        .collect();
    // ASIO is Windows's alone: elsewhere the drivers are all there is to say.
    if !cfg!(windows) {
        return format!("audio drivers: {}", names.join(", "));
    }
    format!(
        "audio drivers: {}\nasio sdk: {}\nasio drivers installed: {}",
        names.join(", "),
        if cantus_engine::devices::asio_built() {
            "linked"
        } else {
            "not linked"
        },
        cantus_engine::devices::installed_asio_drivers().join(", ")
    )
}

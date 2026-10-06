//! What is plugged in, as a musician names it: *interfaces*, not endpoints.
//!
//! Windows lists every jack of an interface as its own endpoint — "Speakers (UMC204HD
//! 192k)", "Line (UMC204HD 192k)" — and the interface's ASIO driver under yet another
//! name ("UMC ASIO Driver"). The setup shows one card per interface: the adapter's name
//! (what is inside the parentheses), its output and input endpoints, and its ASIO driver
//! when one matches by name. What kind of thing it is (an interface, the computer's own
//! audio, headphones, a screen, a virtual cable) picks the picture on its card and the
//! suggestion for its driver and buffer.

use crate::settings::AudioSettings;

/// What kind of device an interface is, for its picture and the suggestion.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A recording interface (USB, Thunderbolt…).
    Interface,
    /// The computer's own audio (Realtek, "High Definition Audio").
    BuiltIn,
    /// Headphones or a headset (Bluetooth, USB headsets).
    Headphones,
    /// A screen's speakers (HDMI, DisplayPort).
    Display,
    /// Software: a virtual cable, a mixer, a wrapper driver (ASIO4ALL, FlexASIO).
    Virtual,
}

/// One interface: its name, kind, and how each driver reaches it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interface {
    /// The adapter's name ("UMC204HD 192k"): the key its settings are remembered by.
    pub key: String,
    pub kind: Kind,
    /// Its output and input endpoints on the system's driver (WASAPI), best first.
    pub outputs: Vec<String>,
    pub inputs: Vec<String>,
    /// Its ASIO driver, when one is installed and matches it.
    pub asio: Option<String>,
}

impl Interface {
    pub fn has_asio(&self) -> bool {
        self.asio.is_some()
    }
}

/// The adapter inside an endpoint's name: "Speakers (UMC204HD 192k)" → "UMC204HD 192k",
/// "Line (2- Focusrite USB Audio)" → "Focusrite USB Audio" (Windows numbers a second
/// device of the same model "2- "). A name without parentheses is its own adapter.
pub fn adapter_of(endpoint: &str) -> String {
    let e = endpoint.trim();
    let inner = match (e.find('('), e.rfind(')')) {
        (Some(a), Some(b)) if b > a + 1 && e.ends_with(')') => {
            // The outermost pair that closes at the end: "Speakers (Realtek(R) Audio)".
            let mut depth = 0i32;
            let mut start = None;
            for (i, c) in e.char_indices().rev() {
                match c {
                    ')' => depth += 1,
                    '(' => {
                        depth -= 1;
                        if depth == 0 {
                            start = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            match start {
                Some(s) if s > 0 => &e[s + 1..e.len() - 1],
                _ => e,
            }
        }
        _ => e,
    };
    let inner = inner.trim();
    // "2- Focusrite USB Audio": a number and a dash in front.
    let digits = inner.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && inner[digits..].starts_with("- ") {
        inner[digits + 2..].trim().to_string()
    } else {
        inner.to_string()
    }
}

/// Lower-case words of a name, split at anything that is not a letter or a digit.
fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Words that say nothing about which interface a driver is for.
const GENERIC: &[&str] = &[
    "asio", "driver", "usb", "audio", "the", "x64", "64", "32", "bit", "device", "pro",
    "hd", "2", "0", "v2", "r",
];

fn any_word(name: &str, list: &[&str]) -> bool {
    let w = words(name);
    w.iter()
        .any(|x| list.iter().any(|l| x == l || x.starts_with(l)))
}

/// What kind of thing a device name describes.
pub fn kind_of(name: &str) -> Kind {
    let n = name.to_lowercase();
    if any_word(name, &["asio4all", "flexasio", "voicemeeter", "vb", "cable", "virtual"])
        || n.contains("steam streaming")
        || n.contains("vb-audio")
    {
        Kind::Virtual
    } else if any_word(name, &["hdmi", "displayport", "nvidia", "display"])
        || n.contains("intel(r) display")
        || n.contains("amd high definition")
    {
        Kind::Display
    } else if any_word(
        name,
        &["bluetooth", "headset", "headphones", "airpods", "hands", "buds", "wh"],
    ) {
        Kind::Headphones
    } else if any_word(name, &["realtek", "conexant", "cirrus", "sst", "smart"])
        || n.contains("high definition audio")
        || n.contains("intel")
    {
        Kind::BuiltIn
    } else {
        Kind::Interface
    }
}

/// Whether an ASIO driver's name points at this adapter: a distinctive word of the
/// driver's name ("umc", "focusrite", "realtek") begins a word of the adapter's.
pub fn asio_matches(driver: &str, adapter: &str) -> bool {
    let a = words(adapter);
    words(driver)
        .into_iter()
        .filter(|w| w.len() >= 3 && !GENERIC.contains(&w.as_str()))
        .any(|w| a.iter().any(|x| x.starts_with(&w) || w.starts_with(x.as_str()) && x.len() >= 3))
}

/// An endpoint's rank among its interface's: the main jacks first.
fn rank(endpoint: &str, output: bool) -> u8 {
    let n = endpoint.to_lowercase();
    let first: &[&str] = if output {
        &["speakers", "line out", "out", "monitor", "main"]
    } else {
        &["line", "input", "in ", "mic", "instrument"]
    };
    first
        .iter()
        .position(|w| n.starts_with(w))
        .map_or(9, |i| i as u8)
}

/// Group the system driver's endpoints into interfaces and match ASIO drivers to them.
/// An ASIO driver no endpoint matches is an interface of its own (a wrapper driver, or an
/// interface whose WASAPI side is disabled). Ordered: interfaces first, then the
/// computer's own, then the rest; by name within each.
pub fn group(outputs: &[String], inputs: &[String], asio: &[String]) -> Vec<Interface> {
    let mut v: Vec<Interface> = Vec::new();
    let mut add = |endpoint: &String, output: bool, v: &mut Vec<Interface>| {
        let key = adapter_of(endpoint);
        let i = match v.iter().position(|x| x.key == key) {
            Some(i) => i,
            None => {
                v.push(Interface {
                    kind: kind_of(&key),
                    key,
                    outputs: Vec::new(),
                    inputs: Vec::new(),
                    asio: None,
                });
                v.len() - 1
            }
        };
        let list = if output {
            &mut v[i].outputs
        } else {
            &mut v[i].inputs
        };
        if !list.contains(endpoint) {
            list.push(endpoint.clone());
        }
    };
    for o in outputs {
        add(o, true, &mut v);
    }
    for i in inputs {
        add(i, false, &mut v);
    }
    for x in &mut v {
        x.outputs.sort_by_key(|e| (rank(e, true), e.clone()));
        x.inputs.sort_by_key(|e| (rank(e, false), e.clone()));
        // The endpoint's own words can say more than the adapter's ("Headphones (…)").
        if x.kind == Kind::Interface && x.outputs.iter().all(|o| kind_of(o) == Kind::Headphones)
            && !x.outputs.is_empty()
        {
            x.kind = Kind::Headphones;
        }
    }
    for d in asio {
        match v.iter_mut().find(|x| x.asio.is_none() && asio_matches(d, &x.key)) {
            Some(x) => x.asio = Some(d.clone()),
            None => v.push(Interface {
                key: d.clone(),
                kind: kind_of(d),
                outputs: Vec::new(),
                inputs: Vec::new(),
                asio: Some(d.clone()),
            }),
        }
    }
    let order = |k: Kind| match k {
        Kind::Interface => 0,
        Kind::BuiltIn => 1,
        Kind::Headphones => 2,
        Kind::Display => 3,
        Kind::Virtual => 4,
    };
    v.sort_by(|a, b| {
        (order(a.kind), a.key.to_lowercase()).cmp(&(order(b.kind), b.key.to_lowercase()))
    });
    v
}

/// The interface the settings (or, for "the default", the running device) point at.
pub fn current<'a>(
    list: &'a [Interface],
    settings: &AudioSettings,
    running_device: &str,
) -> Option<&'a Interface> {
    let asio = settings
        .driver
        .as_deref()
        .is_some_and(|d| d.eq_ignore_ascii_case("ASIO"));
    let device = settings.device.as_deref().unwrap_or(running_device);
    if asio {
        list.iter().find(|i| i.asio.as_deref() == Some(device))
    } else {
        let key = adapter_of(device);
        list.iter().find(|i| i.key == key)
    }
}

/// The settings a first choice of `iface` should start from: ASIO when this build has it
/// and the interface has a driver (not the computer's own, whose ASIO drivers rarely run
/// both directions — round 19 saw Realtek's stop), at 128 samples and 48 kHz; otherwise
/// the system's driver at 256 samples and the device's own rate. Inputs and outputs are
/// the interface's own main jacks.
pub fn suggest(iface: &Interface, asio_built: bool) -> AudioSettings {
    let asio = asio_built && iface.asio.is_some() && iface.kind == Kind::Interface;
    if asio {
        AudioSettings {
            driver: Some("ASIO".into()),
            device: iface.asio.clone(),
            input_device: None,
            sample_rate: Some(48_000),
            buffer_frames: Some(128),
            record_offset_frames: 0,
        }
    } else {
        AudioSettings {
            driver: None,
            device: iface.outputs.first().cloned(),
            // An interface with no input of its own records nothing rather than from the
            // laptop's microphone by surprise.
            input_device: Some(iface.inputs.first().cloned().unwrap_or_default()),
            sample_rate: None,
            buffer_frames: Some(256),
            record_offset_frames: 0,
        }
    }
}

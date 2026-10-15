//! Reading a DrumGizmo kit (round 28): the drum kit file (`<drumkit>`: its channels and
//! instruments, each with a hi-hat-style `group`), and each instrument's file
//! (`<instrument>`: its samples, each with a `power` and one `audiofile` per channel —
//! which file, and which channel of it, since a sample's microphones may share one
//! multi-channel file). Paths are relative to the file that names them.
//!
//! Only what a stereo build needs is read; anything else in the files is ignored. A kit
//! that names a channel it does not declare, a file outside its folder, or no samples, is
//! refused with the reason.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Kit {
    pub name: String,
    /// The microphones, as the kit declares them.
    pub channels: Vec<String>,
    pub instruments: Vec<Instrument>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instrument {
    pub name: String,
    /// Instruments of one group silence each other (DrumGizmo's hi-hat).
    pub group: Option<String>,
    pub samples: Vec<Sample>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub name: String,
    /// DrumGizmo's strength of the hit (what it chooses samples by).
    pub power: f32,
    pub audio: Vec<AudioRef>,
}

/// One microphone of a sample: where its audio is.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioRef {
    pub channel: String,
    /// Relative to the kit's folder (forward slashes).
    pub file: String,
    /// 0-based channel of `file`.
    pub file_channel: usize,
}

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// `rel` joined to `base` (a folder inside the kit), normalised, refused if it climbs out
/// of the kit or is absolute.
fn inside(base: &str, rel: &str) -> Result<String, String> {
    let rel = rel.replace('\\', "/");
    if rel.starts_with('/') || rel.contains(':') {
        return Err(format!("{rel}: an absolute path"));
    }
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for c in rel.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!("{rel}: outside the kit's folder"));
                }
            }
            c => parts.push(c),
        }
    }
    Ok(parts.join("/"))
}

/// Read the kit described by `kit_file` (relative to `root`, the kit's folder).
pub fn read_kit(root: &Path, kit_file: &str) -> Result<Kit, String> {
    let text = read_text(&root.join(kit_file))?;
    let doc = roxmltree::Document::parse(&text).map_err(|e| format!("{kit_file}: {e}"))?;
    let top = doc.root_element();
    if top.tag_name().name() != "drumkit" {
        return Err(format!("{kit_file}: not a DrumGizmo kit"));
    }
    let kit_dir = kit_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let channels: Vec<String> = top
        .descendants()
        .filter(|n| n.has_tag_name("channel"))
        .filter_map(|n| n.attribute("name").map(str::to_string))
        .collect();
    let mut instruments = Vec::new();
    for node in top
        .children()
        .filter(|n| n.has_tag_name("instruments"))
        .flat_map(|n| n.children())
        .filter(|n| n.has_tag_name("instrument"))
    {
        let name = node
            .attribute("name")
            .ok_or("an instrument without a name")?;
        let file = node
            .attribute("file")
            .ok_or_else(|| format!("instrument {name}: no file"))?;
        let rel = inside(kit_dir, file)?;
        let group = node
            .attribute("group")
            .filter(|g| !g.trim().is_empty())
            .map(str::to_string);
        let mut inst = read_instrument(root, &rel)?;
        inst.name = name.to_string();
        inst.group = group;
        for s in &inst.samples {
            if let Some(a) = s.audio.iter().find(|a| !channels.contains(&a.channel)) {
                return Err(format!(
                    "instrument {name}, sample {}: channel {} is not one of the kit's",
                    s.name, a.channel
                ));
            }
        }
        instruments.push(inst);
    }
    if instruments.is_empty() {
        return Err(format!("{kit_file}: no instruments"));
    }
    Ok(Kit {
        name: top.attribute("name").unwrap_or("").to_string(),
        channels,
        instruments,
    })
}

/// Read one instrument's file (relative to `root`).
pub fn read_instrument(root: &Path, rel: &str) -> Result<Instrument, String> {
    let text = read_text(&root.join(rel))?;
    let doc = roxmltree::Document::parse(&text).map_err(|e| format!("{rel}: {e}"))?;
    let top = doc.root_element();
    if top.tag_name().name() != "instrument" {
        return Err(format!("{rel}: not a DrumGizmo instrument"));
    }
    let dir = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut samples = Vec::new();
    for s in top.descendants().filter(|n| n.has_tag_name("sample")) {
        let name = s.attribute("name").unwrap_or("").to_string();
        let power = s
            .attribute("power")
            .and_then(|p| p.trim().parse::<f32>().ok())
            .filter(|p| p.is_finite() && *p >= 0.0)
            .ok_or_else(|| format!("{rel}, sample {name}: no power"))?;
        let mut audio = Vec::new();
        for a in s.children().filter(|n| n.has_tag_name("audiofile")) {
            let channel = a
                .attribute("channel")
                .ok_or_else(|| format!("{rel}, sample {name}: an audio file without a channel"))?;
            let file = a
                .attribute("file")
                .ok_or_else(|| format!("{rel}, sample {name}: an audio file without a name"))?;
            // DrumGizmo counts file channels from 1; a mono file may leave it out.
            let fc = a
                .attribute("filechannel")
                .map(|c| c.trim().parse::<usize>())
                .transpose()
                .map_err(|_| format!("{rel}, sample {name}: a bad file channel"))?
                .unwrap_or(1);
            if fc == 0 {
                return Err(format!("{rel}, sample {name}: file channels count from 1"));
            }
            audio.push(AudioRef {
                channel: channel.to_string(),
                file: inside(dir, file)?,
                file_channel: fc - 1,
            });
        }
        if audio.is_empty() {
            return Err(format!("{rel}, sample {name}: no audio"));
        }
        samples.push(Sample { name, power, audio });
    }
    if samples.is_empty() {
        return Err(format!("{rel}: no samples"));
    }
    Ok(Instrument {
        name: top.attribute("name").unwrap_or("").to_string(),
        group: None,
        samples,
    })
}

/// Every audio file the kit plays, relative to its folder, each once.
pub fn files(kit: &Kit) -> Vec<String> {
    let mut v: Vec<String> = kit
        .instruments
        .iter()
        .flat_map(|i| i.samples.iter())
        .flat_map(|s| s.audio.iter().map(|a| a.file.clone()))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// `root/rel` as a path.
pub fn path_of(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, c| p.join(c))
}

//! A user's DrumGizmo kit made playable (review of 40a27bb7, finding 4): *Install from a
//! file…* of a `drumkit.xml` (or an archive holding one) builds it for Tympanum at install,
//! with the very build Cantus's own DrumGizmo packs take (`kitbuild::mics::render_mics`,
//! ADR 0124 §3) — every microphone kept as its own mono or stereo file, an SFZ multi-mic kit
//! naming them, the main mics beside it — so it plays through Tympanum's SFZ reader as the
//! DRSKit pack does. What the manifest says of a pack of ours is read from the kit itself:
//!
//! - **Keys**: the kit's MIDI map (`Midimap*.xml` beside it — the one sharing the kit
//!   file's suffix first, `DRSKit_minimal.xml` → `Midimap_minimal.xml`), else General MIDI
//!   by the instruments' names (kick 36, snare 38, toms 48/45/43, hi-hat 42/46/44, crash 49,
//!   ride 51…).
//! - **Hi-hat states**: closed, half-open and open by name, laid on key 46 by openness on
//!   CC4 (the pedal stays on its key), as the packs' manifests say them.
//! - **Chokes**: the kit's `group`s (DrumGizmo's hi-hat group).
//! - **Mics**: every channel at its recorded level, a `…L`/`…R` pair one stereo mic.

use crate::drumgizmo::{self, Kit};
use crate::install::Phase;
use crate::kitbuild::mics::{mic_outs, render_mics};
use crate::manifest::{Build, Choke, KeyMap, Mic};
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// The SFZ a user's DrumGizmo build writes, relative to its pack folder.
pub const SFZ: &str = "Tympanum.sfz";
/// The folder its mics' files go into.
pub const FOLDER: &str = "Mics";

/// A General MIDI key for an instrument by its name (lowercase), and how open a hi-hat
/// state is. `None`: no key known.
fn gm(name: &str, nth: &mut [u8; 4]) -> Option<(u8, Option<f32>)> {
    let n = name.to_ascii_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| n.contains(x));
    if has(&["hihat", "hi-hat", "hi_hat", "hat"]) {
        return Some(hat(&n));
    }
    if has(&["kick", "kdrum", "bass drum", "bassdrum", "bd"]) {
        return Some((if nth[0] == 0 { 36 } else { 35 }, None)).inspect(|_| nth[0] += 1);
    }
    if has(&["rim", "stick", "xstick"]) {
        return Some((37, None));
    }
    if has(&["snare", "sd"]) {
        return Some((38, None));
    }
    if has(&["china"]) {
        return Some((52, None));
    }
    if has(&["splash"]) {
        return Some((55, None));
    }
    if has(&["bell"]) {
        return Some((53, None));
    }
    if has(&["ride"]) {
        return Some((51, None));
    }
    if has(&["crash"]) {
        let k = [49, 57][usize::from(nth[1].min(1))];
        nth[1] += 1;
        return Some((k, None));
    }
    if has(&["tom", "ftom", "floor"]) {
        const TOMS: [u8; 6] = [48, 47, 45, 43, 41, 50];
        let k = TOMS[usize::from(nth[2]).min(TOMS.len() - 1)];
        nth[2] += 1;
        return Some((k, None));
    }
    None
}

/// A hi-hat state's key and openness by its name: the pedal on 44 (no openness), closed
/// 42, half 46 at 0.5, open 46 at 1.
fn hat(n: &str) -> (u8, Option<f32>) {
    let has = |w: &[&str]| w.iter().any(|x| n.contains(x));
    if has(&["foot", "pedal", "chick", "stomp"]) {
        (44, None)
    } else if has(&["semi", "half", "mid"]) {
        (46, Some(0.5))
    } else if has(&["open"]) {
        (46, Some(1.0))
    } else {
        (42, Some(0.0))
    }
}

/// The MIDI map beside `kit_file` in `src`, if the kit ships one.
fn midimap(src: &Path, kit_file: &str) -> Option<Vec<(u8, String)>> {
    let stem = Path::new(kit_file)
        .file_stem()?
        .to_string_lossy()
        .to_ascii_lowercase();
    let suffix = stem.split_once('_').map(|(_, s)| s.to_string());
    let mut maps: Vec<String> = std::fs::read_dir(src)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            l.starts_with("midimap") && l.ends_with(".xml")
        })
        .collect();
    maps.sort();
    let pick = suffix
        .as_ref()
        .and_then(|s| {
            maps.iter()
                .find(|m| m.to_ascii_lowercase() == format!("midimap_{s}.xml"))
        })
        .or_else(|| maps.first())?;
    drumgizmo::read_midimap(src, pick).ok()
}

/// What the manifest would say of `kit` to build it for Tympanum.
pub fn build_of(kit: &Kit, src: &Path, kit_file: &str) -> Build {
    let map = midimap(src, kit_file);
    let mut nth = [0u8; 4];
    let mut keys: Vec<KeyMap> = Vec::new();
    for i in &kit.instruments {
        let hatlike = i.group.as_deref().is_some_and(|g| g.contains("hat"))
            || i.name.to_ascii_lowercase().contains("hat");
        let (ks, openness) = match &map {
            Some(m) => {
                let ks: Vec<u8> = m
                    .iter()
                    .filter(|(_, n)| *n == i.name)
                    .map(|(k, _)| *k)
                    .collect();
                let open = if hatlike {
                    hat(&i.name.to_ascii_lowercase()).1
                } else {
                    None
                };
                (ks, open)
            }
            None => match gm(&i.name, &mut nth) {
                Some((k, o)) => (vec![k], if hatlike { o } else { None }),
                None => (Vec::new(), None),
            },
        };
        if ks.is_empty() {
            continue;
        }
        keys.push(KeyMap {
            instrument: i.name.clone(),
            keys: ks,
            gain_db: 0.0,
            label: String::new(),
            openness,
        });
    }
    // The kit's groups choke within themselves.
    let mut chokes: Vec<Choke> = Vec::new();
    for i in kit
        .instruments
        .iter()
        .filter(|i| keys.iter().any(|k| k.instrument == i.name))
    {
        let Some(g) = &i.group else {
            continue;
        };
        let members: Vec<String> = kit
            .instruments
            .iter()
            .filter(|x| x.group.as_ref() == Some(g) && keys.iter().any(|k| k.instrument == x.name))
            .map(|x| x.name.clone())
            .collect();
        if members.len() > 1 && !chokes.iter().any(|c| c.instruments == members) {
            chokes.push(Choke {
                instruments: members,
                release_ms: 60.0,
            });
        }
    }
    // Every channel at its recorded level; a pair panned apart, so it stays one stereo mic.
    let pairs = mic_outs(&kit.channels);
    let mics = kit
        .channels
        .iter()
        .map(|c| {
            let pan = pairs
                .iter()
                .find(|m| m.channels.len() == 2 && m.channels.contains(c))
                .map_or(0.0, |m| if m.channels[0] == *c { -100.0 } else { 100.0 });
            Mic {
                channel: c.clone(),
                gain_db: 0.0,
                pan,
                invert: false,
                own: Vec::new(),
                bleed_db: 0.0,
            }
        })
        .collect();
    Build {
        kit: kit_file.to_string(),
        unpacked_size: 0,
        sfz: SFZ.into(),
        folder: FOLDER.into(),
        gain_db: 0.0,
        mics,
        keys,
        chokes,
        round_robin: 4,
        range_db: 48.0,
        keep: Vec::new(),
        multichannel: true,
    }
}

/// The main mics' sidecar the build writes beside [`SFZ`].
pub fn sidecar() -> String {
    crate::kitbuild::mics::sidecar_path(SFZ)
}

/// Remove the kit `xml`'s own source once built: its audio files, its instruments' files,
/// its MIDI maps and the kit file, then the folders they leave empty.
pub fn remove_source(xml: &Path) {
    let Some(src) = xml.parent() else {
        return;
    };
    let Some(kit_file) = xml.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return;
    };
    let mut gone: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(kit) = drumgizmo::read_kit(src, &kit_file) {
        gone.extend(
            drumgizmo::files(&kit)
                .iter()
                .map(|f| drumgizmo::path_of(src, f)),
        );
    }
    if let Ok(text) = std::fs::read_to_string(xml) {
        if let Ok(doc) = roxmltree::Document::parse(&text) {
            gone.extend(
                doc.descendants()
                    .filter(|n| n.has_tag_name("instrument"))
                    .filter_map(|n| n.attribute("file"))
                    .map(|f| drumgizmo::path_of(src, f)),
            );
        }
    }
    gone.push(xml.to_path_buf());
    for f in std::fs::read_dir(src).into_iter().flatten().flatten() {
        let n = f.file_name().to_string_lossy().to_ascii_lowercase();
        if n.starts_with("midimap") && n.ends_with(".xml") {
            gone.push(f.path());
        }
    }
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    for f in gone {
        let _ = std::fs::remove_file(&f);
        let mut d = f.parent().map(Path::to_path_buf);
        while let Some(x) = d {
            if !x.starts_with(src) || dirs.contains(&x) {
                break;
            }
            dirs.push(x.clone());
            d = x.parent().map(Path::to_path_buf);
        }
    }
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in dirs {
        let _ = std::fs::remove_dir(&d);
    }
}

/// Build the DrumGizmo kit `xml` into the folder `out` for Tympanum: [`SFZ`] and its
/// mics, `report`ing how many hits are written. Returns the kit's name.
pub fn build(
    xml: &Path,
    out: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<String, String> {
    let src = xml.parent().ok_or("the kit file has no folder")?;
    let kit_file = xml
        .file_name()
        .ok_or("the kit file has no name")?
        .to_string_lossy()
        .into_owned();
    let kit = drumgizmo::read_kit(src, &kit_file)?;
    let b = build_of(&kit, src, &kit_file);
    if b.keys.is_empty() {
        return Err(format!(
            "{kit_file}: none of its instruments could be put on a key (no MIDI map, and no name General MIDI knows)"
        ));
    }
    let built = render_mics(
        &b,
        src,
        out,
        &format!("{} (a DrumGizmo kit)", kit.name),
        cancel,
        &mut |done, total| report(Phase::Building { done, total }),
    )?;
    if built.files == 0 {
        return Err(format!("{kit_file}: no hit could be written"));
    }
    Ok(kit.name)
}

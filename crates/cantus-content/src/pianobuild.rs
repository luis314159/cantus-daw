//! A recorded piano written as an SFZ at install from its samples' names (round 40 phase
//! 3, ADR 0125; `chorda-src`) — the Salamander naming that bitKlavier shares
//! (research-p3 `formats.md` §6, `packs.md` P3):
//!
//! | Name | What | How it plays |
//! |---|---|---|
//! | `D#5v13.wav` | note D#5 (C4 = middle C), velocity layer 13 | the keys halfway to its neighbours, layer *k* of *N* on velocities `128·(k−1)/N … 128·k/N − 1` (bitKlavier's own rule) |
//! | `harmC2v2.wav` | release resonance of C2, layer 2 | on the key's release, its layers on 0–39 / 40–84 / 85–127 (bitKlavier's) |
//! | `rel23.wav` | the hammer and key falling back on key 23 (A0 = 1) | on the release of that key alone, not transposed |
//! | `pedalD1.wav`, `pedalU2.wav` | the sustain pedal going down, coming up | on controller 64 crossing, alternating |
//!
//! Each perspective (a folder of such files: one bitKlavier mic image) becomes its notes'
//! and resonances' groups, labelled with its name and sent to its own output
//! (`output=`, ADR 0123); the hammer and pedal noises go to the piano's *Releases* output
//! ([`NOISE_OUTPUT`]) so the mixer gives them a fader of their own. The file is plain SFZ:
//! any SFZ player plays it, and Exemplar's importer reads it as it reads any other.

use crate::manifest::{PianoBuild, PianoNaming};
use std::fmt::Write as _;
use std::path::Path;

/// The output the hammer and pedal noises play on (ADR 0125 §3): after every perspective
/// a pack can have (bitKlavier's nine), fixed so a song's routes and levels keep meaning
/// the same strip whichever images are on disk.
pub const NOISE_OUTPUT: u16 = 16;

/// What one file of the naming is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A note (its root key) at velocity layer `layer` (1-based).
    Note { root: u8, layer: u8 },
    /// The release resonance of a note, at layer `layer`.
    Harm { root: u8, layer: u8 },
    /// The hammer falling back on one key.
    Hammer { key: u8 },
    /// The sustain pedal going down (`true`) or coming up, take `n`.
    Pedal { down: bool, n: u8 },
}

/// A note's name (`C4`, `D#5`, `Bb2`) as a MIDI key, C4 = 60 (the Salamander's and
/// bitKlavier's convention).
pub fn note_key(name: &str) -> Option<u8> {
    let b = name.as_bytes();
    let step = match b.first()? {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let (shift, rest) = match b.get(1) {
        Some(b'#') => (1, &name[2..]),
        Some(b'b') => (-1, &name[2..]),
        _ => (0, &name[1..]),
    };
    let octave: i32 = rest.parse().ok()?;
    let key = 12 * (octave + 1) + step + shift;
    u8::try_from(key).ok().filter(|k| *k < 128)
}

/// What a file's name (without its extension) says, or `None` for any other file.
pub fn parse(stem: &str) -> Option<Kind> {
    if let Some(rest) = stem.strip_prefix("harm") {
        let (note, layer) = rest.split_once('v')?;
        return Some(Kind::Harm {
            root: note_key(note)?,
            layer: layer.parse().ok().filter(|l| *l > 0)?,
        });
    }
    if let Some(n) = stem.strip_prefix("rel") {
        let n: u8 = n.parse().ok().filter(|n| (1..=88).contains(n))?;
        return Some(Kind::Hammer { key: 20 + n });
    }
    if let Some(rest) = stem.strip_prefix("pedal") {
        let down = match rest.as_bytes().first()? {
            b'D' => true,
            b'U' => false,
            _ => return None,
        };
        return Some(Kind::Pedal {
            down,
            n: rest[1..].parse().ok()?,
        });
    }
    let (note, layer) = stem.split_once('v')?;
    Some(Kind::Note {
        root: note_key(note)?,
        layer: layer.parse().ok().filter(|l| *l > 0)?,
    })
}

/// The files of one perspective, sorted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    /// (root, layer, file name).
    pub notes: Vec<(u8, u8, String)>,
    pub harm: Vec<(u8, u8, String)>,
    /// (key, file name).
    pub hammers: Vec<(u8, String)>,
    /// (down, take, file name).
    pub pedals: Vec<(bool, u8, String)>,
    /// Audio files whose names the naming does not know (logged, left out).
    pub unknown: Vec<String>,
}

const AUDIO: [&str; 3] = [".wav", ".flac", ".ogg"];

impl Scan {
    /// Sort file names into the naming's kinds; names that are not audio are ignored.
    pub fn of_names<I: IntoIterator<Item = String>>(names: I) -> Scan {
        let mut s = Scan::default();
        for name in names {
            let lower = name.to_ascii_lowercase();
            let Some(ext) = AUDIO.iter().find(|e| lower.ends_with(*e)) else {
                continue;
            };
            if name.starts_with("._") {
                continue; // a Mac's resource fork beside the file
            }
            let stem = &name[..name.len() - ext.len()];
            match parse(stem) {
                Some(Kind::Note { root, layer }) => s.notes.push((root, layer, name)),
                Some(Kind::Harm { root, layer }) => s.harm.push((root, layer, name)),
                Some(Kind::Hammer { key }) => s.hammers.push((key, name)),
                Some(Kind::Pedal { down, n }) => s.pedals.push((down, n, name)),
                None => s.unknown.push(name),
            }
        }
        s.notes.sort();
        s.harm.sort();
        s.hammers.sort();
        s.pedals.sort();
        s.unknown.sort();
        s
    }

    /// The files of a folder.
    pub fn of_dir(dir: &Path) -> std::io::Result<Scan> {
        let mut names = Vec::new();
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            if e.file_type()?.is_file() {
                names.push(e.file_name().to_string_lossy().into_owned());
            }
        }
        Ok(Scan::of_names(names))
    }

    /// Velocity layers of the notes (the most any root has).
    pub fn layers(&self) -> u8 {
        self.notes.iter().map(|n| n.1).max().unwrap_or(0)
    }

    fn roots(list: &[(u8, u8, String)]) -> Vec<u8> {
        let mut r: Vec<u8> = list.iter().map(|n| n.0).collect();
        r.dedup();
        r
    }
}

/// Each root's keys: halfway to its neighbours (bitKlavier's rule); the lowest and the
/// highest reach as far beyond as their spacing.
pub fn key_ranges(roots: &[u8]) -> Vec<(u8, u8, u8)> {
    let mut out = Vec::with_capacity(roots.len());
    for (i, &r) in roots.iter().enumerate() {
        let below = (i > 0).then(|| roots[i - 1]);
        let above = roots.get(i + 1).copied();
        let lo = match below {
            Some(b) => (u16::from(b) + u16::from(r)) / 2 + 1,
            None => u16::from(r).saturating_sub(above.map_or(1, |a| u16::from(a - r) / 2)),
        };
        let hi = match above {
            Some(a) => (u16::from(r) + u16::from(a)) / 2,
            None => (u16::from(r) + below.map_or(1, |b| u16::from(r - b) / 2)).min(127),
        };
        out.push((r, lo as u8, hi as u8));
    }
    out
}

/// Layer `k` (1-based) of `n` evenly over the velocities (bitKlavier's rule).
pub fn velocities(k: u8, n: u8) -> (u8, u8) {
    let (k, n) = (u32::from(k), u32::from(n.max(1)));
    let lo = (128 * (k - 1) / n).max(1);
    let hi = (128 * k / n).saturating_sub(1).min(127);
    (lo as u8, hi as u8)
}

/// A resonance's layer: bitKlavier's own thresholds for three, evenly otherwise.
pub fn harm_velocities(k: u8, n: u8) -> (u8, u8) {
    if n == 3 {
        const T: [u8; 4] = [0, 40, 85, 128];
        let k = usize::from(k.clamp(1, 3));
        return (T[k - 1].max(1), T[k] - 1);
    }
    velocities(k, n)
}

/// The roots at or above which a grand's strings have no dampers (F#6 up): they ring on
/// after the key is released, as the Salamander's own SFZ plays them.
pub const UNDAMPED_FROM: u8 = 90;

/// One perspective as the SFZ writes it.
pub struct Perspective<'a> {
    /// The folder its files are in, relative to the SFZ (forward slashes, `""` beside it).
    pub folder: &'a str,
    /// Its name as the pack says it (`Stage Lid ORTF`): the groups' label, the strip's.
    pub name: &'a str,
    /// Its output (ADR 0123): forever for the piano.
    pub output: u16,
    pub scan: &'a Scan,
}

fn path(folder: &str, file: &str) -> String {
    if folder.is_empty() {
        file.to_string()
    } else {
        format!("{}/{file}", folder.trim_end_matches('/'))
    }
}

/// The SFZ of a piano: each perspective's notes and resonances on its output, the hammer
/// and pedal noises of the first perspective that has them on [`NOISE_OUTPUT`].
pub fn write_sfz(title: &str, persps: &[Perspective<'_>]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "// {title}");
    let _ = writeln!(
        s,
        "// Written by Cantus at install from the samples' names (the Salamander/bitKlavier\n\
         // naming: C4 = middle C, layers evenly over the velocities, harm… on release,\n\
         // rel1–88 the hammers, pedalD/U the pedal). Plain SFZ: any SFZ player reads it.\n"
    );
    for p in persps {
        let label = p.name.replace(['\r', '\n'], " ");
        let layers = p.scan.layers();
        let ranges = key_ranges(&Scan::roots(&p.scan.notes));
        for undamped in [false, true] {
            let notes: Vec<_> = p
                .scan
                .notes
                .iter()
                .filter(|n| (n.0 >= UNDAMPED_FROM) == undamped)
                .collect();
            if notes.is_empty() {
                continue;
            }
            let release = if undamped { 5 } else { 1 };
            let _ = writeln!(
                s,
                "\n// {label}: notes{}\n<group> group_label={label} output={} amp_veltrack=0 ampeg_release={release}",
                if undamped { " without dampers" } else { "" },
                p.output
            );
            for (root, layer, file) in notes {
                let Some(&(_, lo, hi)) = ranges.iter().find(|r| r.0 == *root) else {
                    continue;
                };
                let (vlo, vhi) = velocities(*layer, layers);
                let _ = writeln!(
                    s,
                    "<region> sample={} lokey={lo} hikey={hi} pitch_keycenter={root} lovel={vlo} hivel={vhi}",
                    path(p.folder, file)
                );
            }
        }
        if !p.scan.harm.is_empty() {
            let n = p.scan.harm.iter().map(|h| h.1).max().unwrap_or(1);
            let ranges = key_ranges(&Scan::roots(&p.scan.harm));
            let _ = writeln!(
                s,
                "\n// {label}: the strings' resonance on release\n<group> group_label={label} output={} trigger=release volume=-6 amp_veltrack=0 rt_decay=6",
                p.output
            );
            for (root, layer, file) in &p.scan.harm {
                let Some(&(_, lo, hi)) = ranges.iter().find(|r| r.0 == *root) else {
                    continue;
                };
                let (vlo, vhi) = harm_velocities(*layer, n);
                let _ = writeln!(
                    s,
                    "<region> sample={} lokey={lo} hikey={hi} pitch_keycenter={root} lovel={vlo} hivel={vhi}",
                    path(p.folder, file)
                );
            }
        }
    }
    // The noises, once: the first perspective that has them.
    if let Some(p) = persps.iter().find(|p| !p.scan.hammers.is_empty()) {
        let _ = writeln!(
            s,
            "\n// Releases: the hammers falling back ({})\n<group> group_label=Releases output={NOISE_OUTPUT} trigger=release pitch_keytrack=0 volume=-6 amp_veltrack=50 rt_decay=2",
            p.name
        );
        for (key, file) in &p.scan.hammers {
            let _ = writeln!(
                s,
                "<region> sample={} lokey={key} hikey={key} pitch_keycenter={key}",
                path(p.folder, file)
            );
        }
    }
    if let Some(p) = persps.iter().find(|p| !p.scan.pedals.is_empty()) {
        for down in [true, false] {
            let takes: Vec<_> = p.scan.pedals.iter().filter(|x| x.0 == down).collect();
            if takes.is_empty() {
                continue;
            }
            let (lo, hi, me, other) = if down { (64, 127, 1, 2) } else { (0, 63, 2, 1) };
            let _ = writeln!(
                s,
                "\n// Releases: the pedal {}\n<group> group_label=Releases output={NOISE_OUTPUT} group={me} off_by={other} lokey=-1 hikey=-1 on_locc64={lo} on_hicc64={hi}",
                if down { "going down" } else { "coming up" }
            );
            let n = takes.len() as f32;
            for (i, (_, _, file)) in takes.iter().enumerate() {
                let _ = writeln!(
                    s,
                    "<region> sample={} lorand={} hirand={}",
                    path(p.folder, file),
                    i as f32 / n,
                    (i + 1) as f32 / n
                );
            }
        }
    }
    s
}

/// What a build made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    pub regions: usize,
    /// Audio files the naming did not know, left out (for the log).
    pub unknown: Vec<String>,
    /// WAV files kept as FLAC, checked sample for sample.
    pub flac: usize,
    /// Files kept as they came, and why (for the log).
    pub kept: Vec<String>,
}

/// Write the SFZ `piano` describes into the pack's folder `dir`: what the install calls
/// once the archive is unpacked (`install.rs`, before the receipt). Integer PCM WAV files
/// are kept as FLAC first (`pianobuild_flac`), each checked against its WAV; `progress`
/// hears (done, total) files, and `cancel` stops between files.
pub fn build(
    piano: &PianoBuild,
    dir: &Path,
    title: &str,
    cancel: &std::sync::atomic::AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Built, String> {
    match piano.naming {
        PianoNaming::Salamander | PianoNaming::BitKlavier => {}
    }
    crate::manifest::check_path(&piano.sfz).map_err(|e| format!("sfz: {e}"))?;
    let mut built = Built::default();
    let mut scans = Vec::new();
    for p in &piano.perspectives {
        if !p.folder.is_empty() {
            crate::manifest::check_path(&p.folder).map_err(|e| format!("folder: {e}"))?;
        }
        let folder = dir.join(&p.folder);
        let scan = Scan::of_dir(&folder).map_err(|e| format!("{}: {e}", p.folder))?;
        if scan.notes.is_empty() {
            return Err(format!("{}: no note is named as the naming says", p.folder));
        }
        scans.push((folder, scan));
    }
    // Every WAV the naming knows, kept as FLAC.
    let total: u64 = scans.iter().map(|(_, s)| files_of(s).len() as u64).sum();
    let mut done = 0u64;
    for (folder, scan) in scans.iter_mut() {
        let mut renamed = Vec::new();
        for name in files_of(scan) {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            if name.to_ascii_lowercase().ends_with(".wav") {
                let (out, d) = crate::pianobuild_flac::transcode(folder, &name);
                match d {
                    crate::pianobuild_flac::Done::Flac => built.flac += 1,
                    crate::pianobuild_flac::Done::Kept(why) => {
                        built.kept.push(format!("{name}: {why}"))
                    }
                }
                renamed.push(out);
            } else {
                renamed.push(name);
            }
            done += 1;
            progress(done, total);
        }
        let unknown = std::mem::take(&mut scan.unknown);
        *scan = Scan::of_names(renamed);
        scan.unknown = unknown;
    }
    let persps: Vec<Perspective<'_>> = piano
        .perspectives
        .iter()
        .zip(&scans)
        .enumerate()
        .map(|(i, (p, (_, scan)))| Perspective {
            folder: &p.folder,
            name: &p.name.en,
            output: p.output.unwrap_or(i as u16),
            scan,
        })
        .collect();
    let text = write_sfz(title, &persps);
    built.regions = text.matches("<region>").count();
    std::fs::write(dir.join(&piano.sfz), text).map_err(|e| e.to_string())?;
    built.unknown = scans.into_iter().flat_map(|(_, s)| s.unknown).collect();
    Ok(built)
}

/// Every file of a scan the naming knows.
fn files_of(s: &Scan) -> Vec<String> {
    s.notes
        .iter()
        .map(|n| n.2.clone())
        .chain(s.harm.iter().map(|n| n.2.clone()))
        .chain(s.hammers.iter().map(|n| n.1.clone()))
        .chain(s.pedals.iter().map(|n| n.2.clone()))
        .collect()
}

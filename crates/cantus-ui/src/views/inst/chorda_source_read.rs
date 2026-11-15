//! Reading a recorded piano for Chorda (round 40 phase 3, ADR 0125): the installed pianos
//! the source list offers, and the zones a switch writes — read off the interface thread's
//! files as the instrument picker reads them (`samples_sounds::import_of`), never by the
//! snapshot.
//!
//! - **One piano, several packs.** bitKlavier Grand is one pack per microphone image (the
//!   manifest's `piano.set`): the list shows it once; its zones are the images that are on
//!   and on disk, each on its own output, the hammer and pedal noises once (the first
//!   image's), within Exemplar's [`MAX_ZONES`].
//! - **A piano that names no outputs** (the Salamander: one stereo pair) gets two: the
//!   pair, and its hammer and pedal noises on *Releases* ([`NOISE_OUTPUT`]) — so its noises
//!   have a fader of their own, as the owner asked.

use crate::views::samples_content::sounds::{import_of, spec_of, SoundChoice};
use cantus_content::pianobuild::NOISE_OUTPUT;
use cantus_content::{InstalledInstrument, Manager};
use cantus_core::inst::chorda_source::{RecordedPiano, DEFAULT_RECORDED};
use cantus_core::inst::sampler::{gf, zf, Trigger, MAX_GROUPS, MAX_ZONES};
use cantus_core::inst_out::OutputId;
use cantus_core::NativeSpec;

/// The name a piano's single perspective gets when its pack names none.
pub const STEREO: &str = "Stereo";
/// The name of the noises' output.
pub const RELEASES: &str = "Releases";

/// One image of a piano: its pack's instrument and the output it plays on.
#[derive(Clone, Debug)]
pub struct Image {
    pub output: OutputId,
    pub name: String,
    /// Installed: its instrument. Not: what downloading it weighs.
    pub installed: Option<InstalledInstrument>,
    pub pack: String,
    pub download: u64,
}

/// One entry of the source list's *Recorded* section.
#[derive(Clone, Debug)]
pub struct PianoEntry {
    pub source: RecordedPiano,
    /// In the interface's language.
    pub name: String,
    pub author: String,
    pub licence: String,
    /// A set's images, installed or not; a plain pack's one.
    pub images: Vec<Image>,
}

impl PianoEntry {
    pub fn installed(&self) -> bool {
        self.images.iter().any(|i| i.installed.is_some())
    }
}

/// Every recorded piano the list offers: installed pack instruments that play in Chorda
/// (and the Salamander, an Exemplar pack that predates `plays`), and the manifest's curated
/// pianos not installed yet — the Salamander first, then by name.
pub fn pianos(m: &Manager) -> Vec<PianoEntry> {
    let lang = crate::i18n::locale().code();
    let mut out: Vec<PianoEntry> = Vec::new();
    let installed = m.instruments();
    for p in &m.manifest().packs {
        let Some(inst) = p.instruments.iter().find(|i| {
            i.plays == cantus_content::manifest::Plays::Chorda
                || (p.id == DEFAULT_RECORDED.0 && i.id == DEFAULT_RECORDED.1)
        }) else {
            continue;
        };
        let have = installed
            .iter()
            .find(|x| x.pack == p.id && x.instrument.id == inst.id)
            .cloned();
        let set = p.piano.as_ref().and_then(|b| b.set.clone());
        let output = p
            .piano
            .as_ref()
            .and_then(|b| b.perspectives.first())
            .and_then(|q| q.output)
            .unwrap_or(0);
        let image_name = p
            .piano
            .as_ref()
            .and_then(|b| b.perspectives.first())
            .map(|q| q.name.get(lang).to_string())
            .unwrap_or_else(|| STEREO.to_string());
        let image = Image {
            output: OutputId(output),
            name: image_name,
            installed: have,
            pack: p.id.clone(),
            download: p.download_size(),
        };
        let key = set.clone().unwrap_or_else(|| p.id.clone());
        if let Some(e) = out.iter_mut().find(|e| e.source.pack == key) {
            e.images.push(image);
            continue;
        }
        let name = match &set {
            // A set is named by its piano, not by an image.
            Some(_) => p
                .name
                .get(lang)
                .split(" · ")
                .next()
                .unwrap_or_default()
                .to_string(),
            None => inst.name.get(lang).to_string(),
        };
        out.push(PianoEntry {
            source: RecordedPiano {
                pack: key,
                instrument: inst.id.clone(),
            },
            name,
            author: p
                .attribution
                .as_ref()
                .map(|a| a.author.clone())
                .or_else(|| p.credits.first().map(|c| c.name.clone()))
                .unwrap_or_default(),
            licence: p.licence.clone(),
            images: vec![image],
        });
    }
    // Pianos installed from a file (ADR 0125 §3): user packs whose instruments play in
    // Chorda, after the curated ones.
    for i in installed.iter().filter(|i| {
        i.instrument.plays == cantus_content::manifest::Plays::Chorda
            && m.manifest().pack(&i.pack).is_none()
    }) {
        out.push(PianoEntry {
            source: RecordedPiano {
                pack: i.pack.clone(),
                instrument: i.instrument.id.clone(),
            },
            name: i.instrument.name.get(lang).to_string(),
            author: i.author.clone(),
            licence: i.licence.clone(),
            images: vec![Image {
                output: OutputId(0),
                name: STEREO.into(),
                installed: Some(i.clone()),
                pack: i.pack.clone(),
                download: 0,
            }],
        });
    }
    for e in out.iter_mut() {
        e.images.sort_by_key(|i| i.output);
    }
    out.sort_by_key(|e| (e.source.pack != DEFAULT_RECORDED.0, e.name.to_lowercase()));
    out
}

/// The entry of `source`, if the list has it.
pub fn entry(m: &Manager, source: &RecordedPiano) -> Option<PianoEntry> {
    pianos(m).into_iter().find(|e| e.source == *source)
}

/// A piano whose groups name no outputs: its noises (controller-triggered zones, release
/// zones that do not follow the key — the hammers) on [`RELEASES`], the rest named
/// [`STEREO`] — unless the pack named its groups' outputs itself.
pub fn route_noises(imp: &mut crate::views::inst::sampler::import::Import) {
    if imp
        .groups
        .iter()
        .any(|(_, g)| g.values[gf::OUT].is_some_and(|o| o != 0.0))
    {
        return;
    }
    let noise = |z: &cantus_core::inst::sampler::Zone| {
        let t = Trigger::from_index(z.get(zf::TRIG));
        t == Trigger::Controller || (t == Trigger::Release && z.get(zf::KEYTRACK) == 0.0)
    };
    for m in 0..MAX_GROUPS {
        let zones: Vec<_> = imp
            .zones
            .iter()
            .filter(|z| z.get(zf::GROUP) as usize == m)
            .collect();
        if zones.is_empty() {
            continue;
        }
        let is_noise = zones.iter().all(|z| noise(z));
        let g = match imp.groups.iter_mut().find(|g| g.0 == m) {
            Some(g) => &mut g.1,
            None => {
                imp.groups.push((m, Default::default()));
                &mut imp.groups.last_mut().expect("pushed").1
            }
        };
        if is_noise {
            g.values[gf::OUT] = Some(f32::from(NOISE_OUTPUT));
            g.name = Some(RELEASES.into());
        } else if g.name.is_none() {
            g.name = Some(STEREO.into());
        }
    }
}

/// The zones `entry` plays, as the Exemplar spec a switch writes under `rec.`: its images
/// that are on (`on(output)`) and installed, the preferred first, each image's zones and
/// groups added while they fit in [`MAX_ZONES`], the noises from the first image alone.
/// Returns the spec and the images left out because they did not fit.
pub fn zones(
    entry: &PianoEntry,
    preferred: Option<OutputId>,
    on: &dyn Fn(OutputId) -> bool,
) -> Result<(NativeSpec, Vec<OutputId>), String> {
    let mut images: Vec<&Image> = entry
        .images
        .iter()
        .filter(|i| i.installed.is_some() && on(i.output))
        .collect();
    images.sort_by_key(|i| (Some(i.output) != preferred, i.output));
    let mut merged = crate::views::inst::sampler::import::Import::default();
    let mut left_out = Vec::new();
    let mut noises_from: Option<OutputId> = None;
    for img in images {
        let inst = img.installed.as_ref().expect("installed");
        let mut imp = import_of(&SoundChoice(inst.clone()))?;
        name_outputs(
            &mut imp,
            &inst.path,
            (entry.images.len() > 1).then_some(img.output),
        );
        route_noises(&mut imp);
        // Which of its groups are noises (kept from one image only).
        let noise_groups: Vec<usize> = imp
            .groups
            .iter()
            .filter(|(_, g)| g.values[gf::OUT] == Some(f32::from(NOISE_OUTPUT)))
            .map(|(m, _)| *m)
            .collect();
        let keep_noise = noises_from.is_none();
        let zones: Vec<_> = imp
            .zones
            .iter()
            .filter(|z| keep_noise || !noise_groups.contains(&(z.get(zf::GROUP) as usize)))
            .cloned()
            .collect();
        let base = merged.groups.iter().map(|g| g.0 + 1).max().unwrap_or(0);
        let groups_needed = imp.groups.iter().map(|g| g.0 + 1).max().unwrap_or(1);
        if merged.zones.len() + zones.len() > MAX_ZONES || base + groups_needed > MAX_GROUPS {
            left_out.push(img.output);
            continue;
        }
        if keep_noise && !noise_groups.is_empty() {
            noises_from = Some(img.output);
        }
        for (m, g) in imp.groups {
            if !keep_noise && noise_groups.contains(&m) {
                continue;
            }
            merged.groups.push((base + m, g));
        }
        for mut z in zones {
            z.set(zf::GROUP, (base + z.get(zf::GROUP) as usize) as f32);
            merged.zones.push(z);
        }
        if merged.values.is_empty() {
            merged.values = imp.values;
            merged.name = imp.name;
        }
    }
    if merged.zones.is_empty() {
        return Err(if left_out.is_empty() {
            "no image of it is on disk and on".into()
        } else {
            "its images do not fit in one instrument".into()
        });
    }
    Ok((spec_of(&merged), left_out))
}

/// Each group's output when the importer left them all on 0: the SFZ's own `output=` on a
/// labelled `<group>` (the piano build writes `group_label=… output=…`), and — an image of
/// a set — the image's output for every group that is not a noise. The SFZ mic rule
/// (`tymp-engine`'s importer) reads `output=` itself; this only fills what it did not.
pub fn name_outputs(
    imp: &mut crate::views::inst::sampler::import::Import,
    sfz: &std::path::Path,
    image: Option<OutputId>,
) {
    if imp
        .groups
        .iter()
        .any(|(_, g)| g.values[gf::OUT].is_some_and(|o| o != 0.0))
    {
        return;
    }
    let text = std::fs::read_to_string(sfz).unwrap_or_default();
    let mut labelled: Vec<(String, u16)> = Vec::new();
    for line in text
        .lines()
        .filter(|l| l.trim_start().starts_with("<group>"))
    {
        let label = line
            .split_once("group_label=")
            .map(|(_, r)| r.split(" output=").next().unwrap_or("").trim().to_string());
        let out = line
            .split_once("output=")
            .and_then(|(_, r)| r.split_whitespace().next())
            .and_then(|n| n.parse::<u16>().ok());
        if let (Some(l), Some(o)) = (label, out) {
            if !labelled.iter().any(|x| x.0 == l) {
                labelled.push((l, o));
            }
        }
    }
    for (_, g) in imp.groups.iter_mut() {
        let by_label = g
            .name
            .as_ref()
            .and_then(|n| labelled.iter().find(|x| &x.0 == n))
            .map(|x| x.1);
        let out = by_label.or_else(|| {
            image
                .filter(|_| g.name.as_deref() != Some(RELEASES))
                .map(|o| o.0)
        });
        if let Some(o) = out.filter(|o| *o != 0) {
            g.values[gf::OUT] = Some(f32::from(o));
        }
    }
}

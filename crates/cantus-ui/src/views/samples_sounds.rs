//! A pack's instruments (round 26, the Salamander Grand Piano): an installed pack that
//! carries an SFZ is offered in the instrument picker (`PickChoice::Sound`), and choosing
//! it gives a track an Exemplar playing the pack's samples in one undo step — on a new
//! track, or in place of a track's instrument. The pack can ask for fewer velocity layers
//! (Exemplar holds every sample in memory) and for a gain that brings it to the level the
//! native instruments are calibrated to (ADR 0039).

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::picker::PickPurpose;
use crate::views::inst::sampler::{self as exemplar, import::Import};
use cantus_content::InstalledInstrument;
use cantus_core::inst::sampler::{zf, Trigger};
use cantus_core::{Command, InstrumentKind, InstrumentSpec, NativeSpec};

/// What the picker offers for one pack instrument.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundChoice(pub InstalledInstrument);

impl SoundChoice {
    /// Its name in the interface's language.
    pub fn name(&self) -> String {
        let lang = crate::i18n::locale().code();
        self.0.instrument.name.get(lang).to_string()
    }

    /// What the instrument's preset field reads once it is loaded: its name, and the
    /// velocity layers it keeps when the pack thins them (so a preset saved from there
    /// starts from it).
    pub fn preset_name(&self) -> String {
        match self.0.instrument.layers {
            Some(n) => tf(
                "samples.sound.preset_layers",
                &[("name", &self.name()), ("n", &n)],
            ),
            None => self.name(),
        }
    }
}

/// Keep at most `keep` velocity layers in each key range of `imp`'s note zones (trigger
/// *attack*), evenly spread from the softest to the loudest; each kept layer takes the
/// velocities of the ones dropped below it, so every velocity still plays exactly one.
/// Ranges with velocity crossfades are left whole (thinning them would open holes in the
/// fades). Release and legato zones are not layers and stay.
pub fn thin_layers(imp: &mut Import, keep: usize) {
    if keep == 0 {
        return;
    }
    use std::collections::BTreeMap;
    let mut ranges: BTreeMap<[u32; 7], Vec<usize>> = BTreeMap::new();
    for (i, z) in imp.zones.iter().enumerate() {
        if Trigger::from_index(z.get(zf::TRIG)) != Trigger::Attack {
            continue;
        }
        let k = [
            z.get(zf::GROUP).to_bits(),
            z.get(zf::LO).to_bits(),
            z.get(zf::HI).to_bits(),
            z.get(zf::SEQ).to_bits(),
            z.get(zf::RLO).to_bits(),
            z.get(zf::RHI).to_bits(),
            z.get(zf::SW).to_bits(),
        ];
        ranges.entry(k).or_default().push(i);
    }
    let mut drop = vec![false; imp.zones.len()];
    let mut most = 0usize;
    for idx in ranges.values_mut() {
        if idx.len() <= keep {
            continue;
        }
        let fades = idx.iter().any(|&i| {
            let z = &imp.zones[i];
            z.get(zf::VFIN) > 0.0 || z.get(zf::VFOUT) > 0.0
        });
        if fades {
            continue;
        }
        idx.sort_by_key(|&i| imp.zones[i].vlo());
        let n = idx.len();
        most = most.max(n);
        let kept: Vec<usize> = if keep == 1 {
            vec![idx[n - 1]]
        } else {
            (0..keep)
                .map(|j| idx[(j * (n - 1) + (keep - 1) / 2) / (keep - 1)])
                .collect()
        };
        let mut from = imp.zones[idx[0]].vlo();
        for &i in &kept {
            let hi = imp.zones[i].vhi();
            imp.zones[i].set(zf::VLO, from as f32);
            from = hi.saturating_add(1);
        }
        for &i in idx.iter() {
            if !kept.contains(&i) {
                drop[i] = true;
            }
        }
    }
    if most > 0 {
        let mut i = 0;
        imp.zones.retain(|_| {
            let keep_it = !drop[i];
            i += 1;
            keep_it
        });
        imp.log.push(format!(
            "velocity layers: {keep} of {most} kept, as the pack asks, so the instrument needs less memory"
        ));
    }
}

/// The pack instrument read from its file: thinned as the pack asks, at its gain, named.
pub fn import_of(sound: &SoundChoice) -> Result<Import, String> {
    let mut imp = exemplar::read_instrument(&sound.0.path)?;
    if let Some(n) = sound.0.instrument.layers {
        thin_layers(&mut imp, n as usize);
    }
    imp.values.retain(|(k, _)| k != "gain");
    if sound.0.instrument.gain_db != 0.0 {
        imp.values.push(("gain".into(), sound.0.instrument.gain_db));
    }
    // A pack's drum kit shows a row per drum in the piano roll and the channel rack.
    if sound.0.instrument.shelf == cantus_content::manifest::Shelf::Drums {
        imp.values.retain(|(k, _)| k != "kit");
        imp.values.push(("kit".into(), 1.0));
    }
    // The voices the pack asks for (round 28: a drum kit's cymbals ring on under the hits).
    if let Some(v) = sound.0.instrument.voices {
        imp.values.retain(|(k, _)| k != "voices");
        imp.values.push(("voices".into(), v.clamp(1, 64) as f32));
    }
    imp.name = Some(sound.name());
    Ok(imp)
}

/// A fresh Exemplar holding `imp`.
pub fn spec_of(imp: &Import) -> NativeSpec {
    let mut spec = NativeSpec::new(InstrumentKind::Sampler);
    let mut values: Vec<(String, f32)> = Vec::new();
    for (m, g) in &imp.groups {
        values.extend(g.values_for(*m));
    }
    for (n, z) in imp.zones.iter().enumerate() {
        values.extend(z.values_for(n));
    }
    values.extend(imp.values.iter().cloned());
    for (k, v) in values {
        spec.set(&k, v);
    }
    for (m, g) in &imp.groups {
        if let Some(name) = &g.name {
            let _ = spec.set_text(&format!("g{m}.name"), Some(name.clone()));
        }
    }
    for (n, z) in imp.zones.iter().enumerate() {
        for (k, text) in z.texts_for(n) {
            let _ = spec.set_text(&k, Some(text));
        }
    }
    spec
}

/// `base`, or `base 2`, `base 3`… — the first name no track has.
fn unique_name(app: &CantusApp, base: &str) -> String {
    let taken = |n: &str| app.project().tracks.iter().any(|t| t.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} {i}"))
        .find(|n| !taken(n))
        .expect("a free name")
}

impl CantusApp {
    /// Put a pack instrument where `purpose` says, as one undo step: a new track named
    /// after it (selected, its window open), or in place of a track's instrument (its
    /// clips and strip kept; its name too unless it was a default one).
    pub fn apply_sound(&mut self, purpose: PickPurpose, sound: &SoundChoice) -> bool {
        let imp = match import_of(sound) {
            Ok(imp) => imp,
            Err(e) => {
                self.set_error(tf(
                    "samples.sound.failed",
                    &[("name", &sound.name()), ("why", &e)],
                ));
                return false;
            }
        };
        let spec = InstrumentSpec::Native(spec_of(&imp));
        let name = sound.name();
        let track = match purpose {
            PickPurpose::AddInstrument { index } => {
                let mut tr = self.bus.new_track(unique_name(self, &name));
                tr.instrument = spec;
                let id = tr.id;
                let len = self.project().tracks.len();
                let index = index.unwrap_or(len).min(len);
                if !self.exec(Command::InsertTrack {
                    index,
                    track: Box::new(tr),
                }) {
                    return false;
                }
                self.select_track(id);
                id
            }
            PickPurpose::ReplaceInstrument { track } => {
                let Some(tr) = self.project().track(track) else {
                    return false;
                };
                let rename = crate::swap::is_default_name(&tr.name, &tr.instrument);
                let mut commands = vec![Command::SetInstrument {
                    track,
                    spec: Box::new(spec),
                }];
                if rename {
                    commands.push(Command::SetTrackName {
                        id: track,
                        name: name.clone(),
                    });
                }
                let was_open = self.instrument_window_open(track);
                if !self.exec(Command::Batch {
                    label_key: "cmd.replace_instrument".into(),
                    commands,
                }) {
                    return false;
                }
                self.inst.forget(track);
                if was_open {
                    self.close_instrument_window(track);
                }
                track
            }
            _ => return false,
        };
        self.inst.preset.insert(track, sound.preset_name());
        self.open_instrument_window(track);
        self.inst.sampler.selected = None;
        self.inst.sampler.log = imp.log.clone();
        self.set_status(tf(
            "samples.sound.added",
            &[("name", &name), ("n", &imp.zones.len().to_string())],
        ));
        true
    }

    /// The instruments the installed packs offer, as picker choices.
    pub fn sound_choices(&mut self) -> Vec<SoundChoice> {
        self.samples
            .content()
            .instruments()
            .into_iter()
            .map(SoundChoice)
            .collect()
    }
}

/// What a pack instrument is, in a few words, for the picker.
pub fn what() -> String {
    t("samples.sound.what")
}

//! The device catalogue (ADR 0029 §1): every instrument and effect Cantus can put on a
//! track or in a slot — its own and the hosted CLAP/VST3 plug-ins the scans found — as
//! one list the picker filters, searches and sorts. Pure data: no drawing, no egui, so it
//! is tested without a window.
//!
//! A [`Device`] carries what the picker shows (name, what it is, vendor, format), where it
//! is filed ([`Category`], first the primary), the [`PickChoice`] that choosing it makes,
//! and a **stable key** (`cantus:unda`, `clap:<id>`, `vst3:<class id>`) that favourites and
//! recents are kept by, so a device keeps its star when it is renamed or translated.

use crate::i18n::t;
use crate::kit::Function;
use crate::picker::{PickChoice, PickPurpose};
use cantus_core::{EffectKind, InstrumentKind};
use cantus_host::{Format, PluginInfo, PluginKind};

/// Instruments or effects.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Instrument,
    Effect,
}

impl Role {
    pub fn of(purpose: &PickPurpose) -> Role {
        if purpose.wants_instrument() {
            Role::Instrument
        } else {
            Role::Effect
        }
    }
}

/// Where a device comes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Native,
    Clap,
    Vst3,
}

impl Source {
    pub fn key(self) -> &'static str {
        match self {
            Source::Native => "picker.source.native",
            Source::Clap => "picker.source.clap",
            Source::Vst3 => "picker.source.vst3",
        }
    }

    /// The badge on a tile: `CLAP`, `VST3`; ours carry none.
    pub fn badge(self) -> Option<&'static str> {
        match self {
            Source::Native => None,
            Source::Clap => Some("CLAP"),
            Source::Vst3 => Some("VST3"),
        }
    }
}

/// The shelves a device is filed on, Ableton's and Bitwig's categories in FL's words.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    // Instruments.
    Synths,
    Keys,
    Bass,
    Drums,
    Samplers,
    OtherInstruments,
    // Effects.
    Eq,
    Dynamics,
    Space,
    Distortion,
    Modulation,
    Stereo,
    Mastering,
    Utility,
}

impl Category {
    pub const INSTRUMENTS: [Category; 6] = [
        Category::Synths,
        Category::Keys,
        Category::Bass,
        Category::Drums,
        Category::Samplers,
        Category::OtherInstruments,
    ];
    pub const EFFECTS: [Category; 8] = [
        Category::Eq,
        Category::Dynamics,
        Category::Space,
        Category::Distortion,
        Category::Modulation,
        Category::Stereo,
        Category::Mastering,
        Category::Utility,
    ];

    pub fn role(self) -> Role {
        if Category::INSTRUMENTS.contains(&self) {
            Role::Instrument
        } else {
            Role::Effect
        }
    }

    /// The i18n key of its name.
    pub fn key(self) -> &'static str {
        match self {
            Category::Synths => "picker.cat.synths",
            Category::Keys => "picker.cat.keys",
            Category::Bass => "picker.cat.bass",
            Category::Drums => "picker.cat.drums",
            Category::Samplers => "picker.cat.samplers",
            Category::OtherInstruments => "picker.cat.other_instruments",
            Category::Eq => "picker.cat.eq",
            Category::Dynamics => "picker.cat.dynamics",
            Category::Space => "picker.cat.space",
            Category::Distortion => "picker.cat.distortion",
            Category::Modulation => "picker.cat.modulation",
            Category::Stereo => "picker.cat.stereo",
            Category::Mastering => "picker.cat.mastering",
            Category::Utility => "picker.cat.utility",
        }
    }

    /// Words that find it whatever the interface's language (English, Spanish).
    fn words(self) -> &'static str {
        // Only the shelf's own names: a synonym that names another kind of device
        // ("reverb" on Space) would find the delay when someone looks for a reverb.
        match self {
            Category::Synths => "synth synths synthesizer sintetizador sintetizadores",
            Category::Keys => "keys teclados",
            Category::Bass => "bass bajo bajos",
            Category::Drums => "drums drum bateria baterias percusion",
            Category::Samplers => "sampler samplers",
            Category::OtherInstruments => "instrument instrumento",
            Category::Eq => "eq equalizer ecualizador",
            Category::Dynamics => "dynamics dinamica",
            Category::Space => "space espacio",
            Category::Distortion => "distortion saturation distorsion saturacion",
            Category::Modulation => "modulation modulacion",
            Category::Stereo => "stereo estereo",
            Category::Mastering => "mastering masterizacion",
            Category::Utility => "utility utilidad utilidades",
        }
    }

    /// The function hue a card of this category is drawn in (ADR 0024 §2).
    pub fn hue(self) -> Function {
        match self {
            Category::Synths | Category::Samplers | Category::OtherInstruments => Function::Source,
            Category::Keys | Category::Bass | Category::Eq | Category::Distortion => Function::Tone,
            Category::Drums | Category::Dynamics | Category::Mastering => Function::Dynamics,
            Category::Modulation => Function::Mod,
            Category::Space | Category::Stereo => Function::Space,
            Category::Utility => Function::Output,
        }
    }
}

/// One device the picker can offer.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    /// Stable across languages, renames and rescans: what favourites and recents keep.
    pub key: String,
    pub choice: PickChoice,
    pub role: Role,
    pub source: Source,
    /// Its name ("Unda", "Pro-Q 4").
    pub name: String,
    /// What it is, in a few words ("Wavetable synth").
    pub what: String,
    pub vendor: String,
    /// First the primary shelf, then any other it also belongs on.
    pub categories: Vec<Category>,
    /// Ours: the slug of its locale keys (`picker.dev.<slug>.*`).
    pub slug: Option<&'static str>,
    /// The curated order of the native catalogue (hosted ones follow, by name).
    pub order: u16,
    /// Lower-case, accent-free words the search matches: name, what, vendor, categories,
    /// format, the plug-in's own features.
    pub haystack: String,
}

impl Device {
    pub fn primary(&self) -> Category {
        self.categories[0]
    }

    pub fn hue(&self) -> Function {
        match self.choice {
            PickChoice::Instrument(k) => crate::views::inst::hue(k),
            PickChoice::Effect(k) => crate::views::fx::frame::hue_of(k),
            _ => self.primary().hue(),
        }
    }

    pub fn in_category(&self, c: Category) -> bool {
        self.categories.contains(&c)
    }
}

/// Lower case without accents, so *síntesis* finds *sintesis* and the reverse.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// The native catalogue: (slug, choice, categories), in the order it is shown.
fn natives() -> Vec<(&'static str, PickChoice, Vec<Category>)> {
    use Category as C;
    use EffectKind as E;
    use InstrumentKind as I;
    let mut v = vec![
        (
            "unda",
            PickChoice::Instrument(I::Wavetable),
            vec![C::Synths],
        ),
        ("calor", PickChoice::Instrument(I::Analog), vec![C::Synths]),
        (
            "modulus",
            PickChoice::Instrument(I::Fm),
            vec![C::Synths, C::Keys],
        ),
        ("firmus", PickChoice::Firmus, vec![C::Synths]),
        (
            "lamina",
            PickChoice::Instrument(I::ElectricPiano),
            vec![C::Keys],
        ),
        ("organum", PickChoice::Instrument(I::Organ), vec![C::Keys]),
        ("gravis", PickChoice::Instrument(I::Bass), vec![C::Bass]),
        ("pulsus", PickChoice::Pulsus, vec![C::Drums]),
        (
            "exemplar",
            PickChoice::Instrument(I::Sampler),
            vec![C::Samplers],
        ),
        (
            "aequalis",
            PickChoice::Effect(E::Eq),
            vec![C::Eq, C::Mastering],
        ),
        (
            "comprimo",
            PickChoice::Effect(E::Compressor),
            vec![C::Dynamics],
        ),
        ("porta", PickChoice::Effect(E::Gate), vec![C::Dynamics]),
        ("sibilus", PickChoice::Effect(E::DeEsser), vec![C::Dynamics]),
        (
            "multiplex",
            PickChoice::Effect(E::Multiband),
            vec![C::Dynamics, C::Mastering],
        ),
        (
            "terminus",
            PickChoice::Effect(E::Limiter),
            vec![C::Dynamics, C::Mastering],
        ),
        (
            "culmen",
            PickChoice::Effect(E::Maximizer),
            vec![C::Mastering, C::Dynamics],
        ),
        ("spatium", PickChoice::Effect(E::Space), vec![C::Space]),
        ("delay", PickChoice::Effect(E::Delay), vec![C::Space]),
        ("reverb", PickChoice::Effect(E::Reverb), vec![C::Space]),
        (
            "fervor",
            PickChoice::Effect(E::Saturator),
            vec![C::Distortion, C::Mastering],
        ),
        (
            "latitudo",
            PickChoice::Effect(E::Stereo),
            vec![C::Stereo, C::Mastering],
        ),
    ];
    // A scaffold instrument stays out of the picker until it is ready (`ready_kinds`).
    v.retain(|(_, c, _)| match c {
        PickChoice::Instrument(k) => k.ready(),
        _ => true,
    });
    v
}

/// The slug of a native choice, if it is one.
pub fn slug_of(choice: &PickChoice) -> Option<&'static str> {
    natives()
        .into_iter()
        .find(|(_, c, _)| c == choice)
        .map(|(s, _, _)| s)
}

fn native(order: usize, slug: &'static str, choice: PickChoice, cats: Vec<Category>) -> Device {
    let role = match choice {
        PickChoice::Effect(_) => Role::Effect,
        _ => Role::Instrument,
    };
    let name = t(&format!("picker.dev.{slug}.name"));
    let what = t(&format!("picker.dev.{slug}.what"));
    let vendor = "Cantus".to_string();
    let mut hay = format!("{name} {what} {vendor} native nativo cantus");
    for c in &cats {
        hay.push(' ');
        hay.push_str(&t(c.key()));
        hay.push(' ');
        hay.push_str(c.words());
    }
    for p in presets(&choice) {
        hay.push(' ');
        hay.push_str(&p);
    }
    Device {
        key: format!("cantus:{slug}"),
        choice,
        role,
        source: Source::Native,
        name,
        what,
        vendor,
        categories: cats,
        slug: Some(slug),
        order: order as u16,
        haystack: fold(&hay),
    }
}

/// The factory presets of a native device, by name in the interface's language.
pub fn presets(choice: &PickChoice) -> Vec<String> {
    match choice {
        PickChoice::Instrument(k) => crate::views::inst::factory_presets(*k)
            .iter()
            .map(|p| t(p.label_key))
            .collect(),
        PickChoice::Effect(k) => crate::views::fx::factory_presets(*k)
            .iter()
            .map(|p| t(p.0))
            .collect(),
        PickChoice::Pulsus => cantus_core::PulsusParams::factory()
            .iter()
            .map(|f| t(f.label_key))
            .collect(),
        _ => Vec::new(),
    }
}

/// The shelves a hosted plug-in goes on, from the words its format gives it (CLAP
/// features, VST3 subcategories) and, failing those, its name.
pub fn hosted_categories(info: &PluginInfo) -> Vec<Category> {
    use Category as C;
    let role = hosted_role(info);
    let mut words = String::new();
    for f in &info.features {
        words.push(' ');
        words.push_str(&fold(f).replace(['|', '-', '_'], " "));
    }
    let words = format!(" {words} {} ", fold(&info.name));
    let has = |w: &[&str]| w.iter().any(|w| words.contains(&format!(" {w} ")));
    let mut out = Vec::new();
    fn push(out: &mut Vec<Category>, c: Category) {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    match role {
        Role::Instrument => {
            if has(&["drum", "drums", "drum machine", "machine", "percussion"]) {
                push(&mut out, C::Drums);
            }
            if has(&["piano", "keys", "organ", "e piano", "epiano"]) {
                push(&mut out, C::Keys);
            }
            if has(&["bass"]) {
                push(&mut out, C::Bass);
            }
            if has(&["sampler", "sample", "rompler", "soundfont"]) {
                push(&mut out, C::Samplers);
            }
            if has(&["synth", "synthesizer", "synthesiser"]) {
                push(&mut out, C::Synths);
            }
            if out.is_empty() {
                push(&mut out, C::OtherInstruments);
            }
        }
        Role::Effect => {
            if has(&["eq", "equalizer", "equaliser", "filter"]) {
                push(&mut out, C::Eq);
            }
            if has(&[
                "dynamics",
                "compressor",
                "limiter",
                "gate",
                "expander",
                "transient",
            ]) {
                push(&mut out, C::Dynamics);
            }
            if has(&["reverb", "delay", "echo", "spatial", "room"]) {
                push(&mut out, C::Space);
            }
            if has(&[
                "distortion",
                "saturation",
                "saturator",
                "overdrive",
                "amp",
                "bitcrusher",
            ]) {
                push(&mut out, C::Distortion);
            }
            if has(&[
                "modulation",
                "chorus",
                "flanger",
                "phaser",
                "tremolo",
                "vibrato",
            ]) {
                push(&mut out, C::Modulation);
            }
            if has(&["stereo", "surround", "imager", "panner", "width"]) {
                push(&mut out, C::Stereo);
            }
            if has(&["mastering", "maximizer", "loudness"]) {
                push(&mut out, C::Mastering);
            }
            if has(&[
                "utility",
                "tools",
                "analyzer",
                "analyser",
                "meter",
                "gain",
                "restoration",
            ]) {
                push(&mut out, C::Utility);
            }
            if out.is_empty() {
                push(&mut out, C::Utility);
            }
        }
    }
    out
}

fn hosted_role(info: &PluginInfo) -> Role {
    if info.kind == PluginKind::Instrument {
        Role::Instrument
    } else {
        Role::Effect
    }
}

/// Whether a scanned plug-in belongs in the picker: instruments, and effects that put out
/// audio (what an insert slot can hold, `PluginDb::effects`).
pub fn offered(info: &PluginInfo) -> bool {
    match info.kind {
        PluginKind::Instrument => info.main_outputs > 0,
        PluginKind::Effect => info.main_outputs > 0,
        _ => false,
    }
}

fn hosted(info: &PluginInfo) -> Device {
    let source = match info.format {
        Format::Clap => Source::Clap,
        Format::Vst3 => Source::Vst3,
    };
    let role = hosted_role(info);
    let categories = hosted_categories(info);
    let what = t(categories[0].key());
    let mut hay = format!(
        "{} {} {} {} {}",
        info.name,
        info.vendor,
        what,
        source.badge().unwrap_or_default(),
        info.features.join(" ")
    );
    for c in &categories {
        hay.push(' ');
        hay.push_str(c.words());
    }
    let prefix = match info.format {
        Format::Clap => "clap",
        Format::Vst3 => "vst3",
    };
    Device {
        key: format!("{prefix}:{}", info.id),
        choice: PickChoice::Hosted(Box::new(info.clone())),
        role,
        source,
        name: info.name.clone(),
        what,
        vendor: info.vendor.clone(),
        categories,
        slug: None,
        order: u16::MAX,
        haystack: fold(&hay),
    }
}

/// Every device: ours in the curated order, then every offered plug-in of `plugins`.
pub fn build<'a>(plugins: impl IntoIterator<Item = &'a PluginInfo>) -> Vec<Device> {
    build_with(plugins, &[])
}

/// A pack's instrument (round 26): filed on its shelf and with the samplers, by the
/// pack's author, found by its name, its pack's and what plays it.
fn sound(order: usize, s: &cantus_content::InstalledInstrument) -> Device {
    use cantus_content::manifest::Shelf;
    let choice = crate::views::samples_content::sounds::SoundChoice(s.clone());
    let name = choice.name();
    let lang = crate::i18n::locale().code();
    let pack = s.pack_name.get(lang).to_string();
    let what = crate::views::samples_content::sounds::what();
    let shelf = match s.instrument.shelf {
        Shelf::Keys => Category::Keys,
        Shelf::Drums => Category::Drums,
        Shelf::Bass => Category::Bass,
        Shelf::Synths => Category::Synths,
        Shelf::Samplers => Category::Samplers,
        Shelf::Other => Category::OtherInstruments,
    };
    let mut categories = vec![shelf];
    if shelf != Category::Samplers {
        categories.push(Category::Samplers);
    }
    let mut hay = format!(
        "{name} {pack} {} {what} exemplar sampler {} {}",
        s.author, s.instrument.name.en, s.instrument.name.es
    );
    for c in &categories {
        hay.push(' ');
        hay.push_str(&t(c.key()));
        hay.push(' ');
        hay.push_str(c.words());
    }
    Device {
        key: s.key(),
        choice: PickChoice::Sound(Box::new(choice)),
        role: Role::Instrument,
        source: Source::Native,
        name,
        what,
        vendor: s.author.clone(),
        categories,
        slug: None,
        order: order as u16,
        haystack: fold(&hay),
    }
}

/// [`build`], with the instruments the installed sound packs offer after ours.
pub fn build_with<'a>(
    plugins: impl IntoIterator<Item = &'a PluginInfo>,
    sounds: &[cantus_content::InstalledInstrument],
) -> Vec<Device> {
    let mut out: Vec<Device> = natives()
        .into_iter()
        .enumerate()
        .map(|(i, (slug, choice, cats))| native(i, slug, choice, cats))
        .collect();
    let first = out.len();
    out.extend(sounds.iter().enumerate().map(|(i, s)| sound(first + i, s)));
    let mut seen = std::collections::HashSet::new();
    for info in plugins {
        if offered(info) {
            let d = hosted(info);
            // A plug-in installed twice (two folders) is offered once.
            if seen.insert(d.key.clone()) {
                out.push(d);
            }
        }
    }
    out
}

/// The stable key a device of `choice` has, to find it in a catalogue.
pub fn key_of(choice: &PickChoice) -> Option<String> {
    match choice {
        PickChoice::Hosted(info) => Some(match info.format {
            Format::Clap => format!("clap:{}", info.id),
            Format::Vst3 => format!("vst3:{}", info.id),
        }),
        PickChoice::Sound(s) => Some(s.0.key()),
        c => slug_of(c).map(|s| format!("cantus:{s}")),
    }
}

/// How well `device` matches a folded query: `None` when a word of the query is missing,
/// else higher is better — a name that starts with the word beats one that contains it,
/// which beats a match anywhere else (vendor, category, a preset's name).
pub fn score(device: &Device, query: &str) -> Option<u32> {
    let name = fold(&device.name);
    let mut total = 0;
    for word in query.split_whitespace() {
        let s = if name.starts_with(word) {
            100
        } else if name.split_whitespace().any(|w| w.starts_with(word)) {
            80
        } else if name.contains(word) {
            60
        } else if device
            .haystack
            .split_whitespace()
            .any(|w| w.starts_with(word))
        {
            30
        } else if device.haystack.contains(word) {
            10
        } else {
            return None;
        };
        total += s;
    }
    Some(total)
}

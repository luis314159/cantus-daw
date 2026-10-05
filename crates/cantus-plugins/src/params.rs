//! A plug-in's parameter list as the standards want it, made from Cantus's own tables
//! (`cantus_core::fx::*`): the same keys, names, ranges, defaults, units and value text.
//!
//! **Ids are forever.** A host stores a parameter's id in every automation lane and every
//! project, so an id is derived from the parameter's key — which is itself forever (ADR
//! 0017) — by a fixed rule, never assigned by position: FNV-1a (32 bit) of the key's
//! UTF-8 bytes, masked to 31 bits (VST3 leaves the upper half of the range to hosts). The
//! golden test (`tests/ids.rs`) fails if any id changes, and a test per kind proves there
//! is no collision.
//!
//! Two parameters exist only in the exported plug-ins, after the table's:
//! [`SIDECHAIN_KEY`] (whether the detector listens to the sidechain input — a host
//! always passes that input, connected or not, so the choice must be the user's) and
//! [`BYPASS_KEY`] (the standards' bypass flag, so a host's bypass button drives it; the
//! bypass keeps the latency, ADR 0020).

use crate::text::{self, Lang};
use cantus_core::fx::readout::{parse_readout, readout};
use cantus_core::{EffectKind, EffectParamInfo};
use std::sync::OnceLock;

/// The plug-in's own switch for the sidechain input.
pub const SIDECHAIN_KEY: &str = "cantus.sidechain";
/// The standards' bypass.
pub const BYPASS_KEY: &str = "cantus.bypass";

/// The id of a parameter key: FNV-1a 32 of its bytes, top bit cleared. Forever.
pub const fn param_id(key: &str) -> u32 {
    let bytes = key.as_bytes();
    let mut hash: u32 = 0x811C_9DC5;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1;
    }
    hash & 0x7FFF_FFFF
}

/// What a parameter drives.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// One of the effect's own (`EffectSpec` key).
    Spec,
    Sidechain,
    Bypass,
}

/// One parameter, ready for either standard.
#[derive(Clone, Debug)]
pub struct Param {
    pub index: usize,
    pub id: u32,
    pub key: &'static str,
    pub role: Role,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub logarithmic: bool,
    pub stepped: bool,
    /// What a host lists: `Band 4 Gain`.
    pub name: String,
    /// The table's own label: `Gain`.
    pub short_name: String,
    /// The group it sits in (`Band 4`), empty at the top.
    pub group: String,
    /// A stepped parameter's choices by step, when it has names for them.
    pub choices: Vec<String>,
}

impl Param {
    /// A value as the effect keeps it: clamped to the range, whole for a stepped
    /// parameter, the default for anything that is not a number. The same rule as
    /// `EffectSpec::set`, so the plug-in and the DAW agree on every stored value.
    pub fn clamp(&self, value: f64) -> f32 {
        let v = value as f32;
        let v = if v.is_finite() {
            v.clamp(self.min, self.max)
        } else {
            self.default
        };
        if self.stepped {
            v.round()
        } else {
            v
        }
    }

    /// Steps between min and max for a stepped parameter, 0 for a continuous one.
    pub fn step_count(&self) -> i32 {
        if self.stepped {
            (self.max - self.min).round() as i32
        } else {
            0
        }
    }

    /// The knob's position, 0…1, through the DAW's taper (`widgets::knob`): logarithmic
    /// for frequencies, times and ratios, linear otherwise.
    pub fn to_normalized(&self, plain: f32) -> f64 {
        let (min, max, v) = (self.min as f64, self.max as f64, plain as f64);
        if max <= min {
            return 0.0;
        }
        let n = if self.logarithmic && min > 0.0 {
            (v.max(min) / min).ln() / (max / min).ln()
        } else {
            (v - min) / (max - min)
        };
        n.clamp(0.0, 1.0)
    }

    /// The value at a knob position (the inverse of [`Param::to_normalized`]), clamped.
    /// (A method: the position is this parameter's; VST3 names it `normalizedParamToPlain`.)
    #[allow(clippy::wrong_self_convention)]
    pub fn from_normalized(&self, n: f64) -> f32 {
        let n = if n.is_finite() {
            n.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (min, max) = (self.min as f64, self.max as f64);
        let v = if self.stepped {
            min + (n * (max - min)).round()
        } else if self.logarithmic && min > 0.0 {
            min * (max / min).powf(n)
        } else {
            min + n * (max - min)
        };
        self.clamp(v)
    }

    /// Whether CLAP sees this parameter as its knob position (0…1) rather than its value.
    /// CLAP has no taper: a host maps the plain range linearly onto its lanes, and a
    /// 20 Hz–20 kHz lane in hertz would put nearly all of the audible range in its bottom
    /// tenth. So a logarithmic parameter is exposed through the taper, as the knob moves;
    /// its text still reads `1.2 kHz`.
    pub fn clap_is_position(&self) -> bool {
        self.logarithmic && !self.stepped && self.min > 0.0
    }

    /// A value as CLAP carries it.
    pub fn to_clap(&self, plain: f32) -> f64 {
        if self.clap_is_position() {
            self.to_normalized(plain)
        } else {
            plain as f64
        }
    }

    /// A value from CLAP, clamped.
    #[allow(clippy::wrong_self_convention)]
    pub fn from_clap(&self, v: f64) -> f32 {
        if self.clap_is_position() {
            self.from_normalized(v)
        } else {
            self.clamp(v)
        }
    }

    /// CLAP's (min, max, default).
    pub fn clap_range(&self) -> (f64, f64, f64) {
        if self.clap_is_position() {
            (0.0, 1.0, self.to_normalized(self.default))
        } else {
            (self.min as f64, self.max as f64, self.default as f64)
        }
    }

    /// The value's text: the choice's name, or the number in its unit as the DAW prints
    /// it.
    pub fn text(&self, plain: f32) -> String {
        if self.stepped && !self.choices.is_empty() {
            let i = (plain - self.min).round().max(0.0) as usize;
            if let Some(c) = self.choices.get(i) {
                return c.clone();
            }
        }
        readout(plain, self.unit)
    }

    /// A typed value: a choice's name (any case), or a number in the unit.
    pub fn parse(&self, text: &str) -> Option<f32> {
        let wanted = text.trim().to_lowercase();
        if let Some(i) = self.choices.iter().position(|c| c.to_lowercase() == wanted) {
            return Some(self.clamp(self.min as f64 + i as f64));
        }
        parse_readout(text, self.unit).map(|v| self.clamp(v as f64))
    }
}

/// A plug-in's whole list.
#[derive(Debug)]
pub struct Table {
    pub kind: EffectKind,
    pub params: Vec<Param>,
    /// How many of `params` are the effect's own (`Role::Spec`); they come first, in the
    /// table's order.
    pub spec_count: usize,
    pub sidechain: usize,
    pub bypass: usize,
    /// `(id, index)`, sorted by id.
    by_id: Vec<(u32, usize)>,
    /// The spec's keys in `BTreeMap` order, as indices into `params`: lets the audio
    /// thread write every value into an `EffectSpec` in one pass, with no lookup.
    pub spec_order: Vec<usize>,
}

impl Table {
    pub fn by_id(&self, id: u32) -> Option<&Param> {
        self.index_of(id).map(|i| &self.params[i])
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.by_id
            .binary_search_by_key(&id, |(i, _)| *i)
            .ok()
            .map(|at| self.by_id[at].1)
    }

    pub fn by_key(&self, key: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.key == key)
    }

    pub fn defaults(&self) -> Vec<f32> {
        self.params.iter().map(|p| p.default).collect()
    }

    /// Every group, in the order its first parameter appears (VST3 units).
    pub fn groups(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for p in &self.params {
            if !p.group.is_empty() && !out.contains(&p.group.as_str()) {
                out.push(&p.group);
            }
        }
        out
    }
}

/// The group of a banded key (`b3.gain` → band 3), and the field.
fn band_of(kind: EffectKind, key: &str) -> Option<(&'static str, usize, String)> {
    let (head, field) = key.split_once('.')?;
    if kind == EffectKind::Maximizer {
        // Culmen's bands are named, not numbered: `lo.thr` is in *Low band*.
        cantus_core::fx::maximizer::parse_key(key)?;
        let group = match head {
            "lo" => "max.group.lo",
            "mid" => "max.group.mid",
            "hi" => "max.group.hi",
            _ => "max.group.all",
        };
        return Some((group, 0, format!("b.{field}")));
    }
    let digits = head.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let prefix = &head[..head.len() - digits.len()];
    let n: usize = digits.parse().ok()?;
    let group = match (kind, prefix) {
        (EffectKind::Eq, "b") | (EffectKind::Multiband, "b") => "export.group.band",
        (EffectKind::Compressor, "sc") => "export.group.key_band",
        (EffectKind::Space, "d") => "export.group.decay_band",
        (EffectKind::Space, "e") => "export.group.post_band",
        (EffectKind::Saturator, "p") => "sat.group.point",
        _ => return None,
    };
    Some((group, n, format!("{prefix}.{field}")))
}

/// The locale prefix of a stepped parameter's choices (`{prefix}.{i}`), by kind and
/// field (`b.shape` for a banded field).
fn choice_prefix(kind: EffectKind, field: &str) -> Option<&'static str> {
    use EffectKind::*;
    Some(match (kind, field) {
        (Eq, "b.shape") | (Compressor, "sc.shape") => "eq.shape",
        (Eq, "b.place") => "eq.place",
        (Eq, "mode") => "eq.mode",
        (Eq, "lp_res") => "eq.res",
        (Compressor, "style") => "comp.style",
        (Compressor, "listen") => "comp.listen",
        (Compressor, "os") => "comp.os",
        (Limiter, "style") => "lim.style",
        (Limiter, "monitor") => "lim.monitor",
        (Limiter, "shape") => "lim.shape",
        (Space, "style") => "space.style",
        (Space, "sync") => "space.sync",
        (Space, "d.shape") => "space.dshape",
        (Space, "e.shape") => "space.pshape",
        (Multiband, "mode") => "mb.mode",
        (Multiband, "listen") => "mb.listen",
        (Multiband, "b.place") => "mb.place",
        (DeEsser, "mode") => "ds.mode",
        (DeEsser, "proc") => "ds.proc",
        (DeEsser, "stereo") => "ds.stereo",
        (DeEsser, "listen") => "ds.listen",
        (Gate, "style") => "gate.style",
        (Gate, "listen") => "gate.hear",
        (Stereo, "listen") => "st.listen",
        (Stereo, "mono_slope") | (Stereo, "tone_slope") => "st.slope",
        (Stereo, "haas_mode") => "st.haas_mode",
        (Maximizer, "style") => "lim.style",
        (Maximizer, "shape") => "lim.shape",
        (Maximizer, "monitor") => "max.monitor",
        (Maximizer, "xmode") => "max.xmode",
        (Maximizer, "b.place") => "mb.place",
        (Saturator, "model") => "sat.model",
        (Saturator, "os") => "sat.os",
        (Saturator, "phase") => "sat.phase",
        (Saturator, "chan") => "sat.chan",
        _ => return None,
    })
}

/// The locale key of one choice: `{prefix}.{i}`, except where an editor names it
/// otherwise (Comprimo's fifth listen mode is its *Compare* button).
fn choice_key(kind: EffectKind, prefix: &str, i: i32) -> String {
    match (kind, prefix, i) {
        (EffectKind::Compressor, "comp.listen", 4) => "comp.compare".to_string(),
        _ => format!("{prefix}.{i}"),
    }
}

/// Every locale key a kind's choices use (the i18n test checks both languages have them).
pub fn choice_keys(kind: EffectKind) -> Vec<String> {
    let mut out = Vec::new();
    for info in kind.params() {
        let field = band_of(kind, info.key)
            .map(|(_, _, f)| f)
            .unwrap_or_else(|| info.key.to_string());
        if let Some(prefix) = choice_prefix(kind, &field) {
            for i in info.min.round() as i32..=info.max.round() as i32 {
                let k = choice_key(kind, prefix, i);
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
    }
    out
}

fn choices(kind: EffectKind, field: &str, info: &EffectParamInfo, lang: Lang) -> Vec<String> {
    if !info.stepped {
        return Vec::new();
    }
    let (lo, hi) = (info.min.round() as i32, info.max.round() as i32);
    if let Some(prefix) = choice_prefix(kind, field) {
        return (lo..=hi)
            .map(|i| text::t_in(lang, &choice_key(kind, prefix, i)))
            .collect();
    }
    match (kind, field) {
        (EffectKind::Limiter | EffectKind::Maximizer, "dither") => {
            let mut v = vec![text::t_in(lang, "export.off")];
            for bits in [24, 22, 20, 18, 16] {
                v.push(text::tf_in(lang, "export.bits", &[("n", &bits)]));
            }
            v
        }
        (EffectKind::Eq, "solo") => {
            let mut v = vec![text::t_in(lang, "export.off")];
            for n in 1..=hi {
                v.push(text::tf_in(lang, "export.group.band", &[("n", &n)]));
            }
            v
        }
        _ if lo == 0 && hi == 1 && info.unit.is_empty() => {
            vec![
                text::t_in(lang, "export.off"),
                text::t_in(lang, "export.on"),
            ]
        }
        _ => Vec::new(),
    }
}

fn build(kind: EffectKind, lang: Lang) -> Table {
    let mut params = Vec::new();
    for info in kind.params() {
        let label = text::t_in(lang, info.label_key);
        let (name, group, field) = match band_of(kind, info.key) {
            Some((group_key, n, field)) => {
                // Bands are numbered from 1 on screen; keys count from 0 (ADR 0017).
                let group = text::tf_in(lang, group_key, &[("n", &(n + 1))]);
                let name = text::tf_in(
                    lang,
                    "export.param.in_group",
                    &[("group", &group), ("name", &label)],
                );
                (name, group, field)
            }
            None => (label.clone(), String::new(), info.key.to_string()),
        };
        params.push(Param {
            index: params.len(),
            id: param_id(info.key),
            key: info.key,
            role: Role::Spec,
            min: info.min,
            max: info.max,
            default: info.default,
            unit: info.unit,
            logarithmic: info.logarithmic,
            stepped: info.stepped,
            name,
            short_name: label,
            group,
            choices: choices(kind, &field, info, lang),
        });
    }
    let spec_count = params.len();
    let switch = |index: usize, key: &'static str, role: Role, name_key: &str, choices| Param {
        index,
        id: param_id(key),
        key,
        role,
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        logarithmic: false,
        stepped: true,
        name: text::t_in(lang, name_key),
        short_name: text::t_in(lang, name_key),
        group: String::new(),
        choices,
    };
    let sidechain = params.len();
    params.push(switch(
        sidechain,
        SIDECHAIN_KEY,
        Role::Sidechain,
        "export.sidechain",
        vec![
            text::t_in(lang, "export.sidechain.own"),
            text::t_in(lang, "export.sidechain.external"),
        ],
    ));
    let bypass = params.len();
    params.push(switch(
        bypass,
        BYPASS_KEY,
        Role::Bypass,
        "export.bypass",
        vec![
            text::t_in(lang, "export.off"),
            text::t_in(lang, "export.on"),
        ],
    ));
    let mut by_id: Vec<(u32, usize)> = params.iter().map(|p| (p.id, p.index)).collect();
    by_id.sort_unstable();
    let mut spec_order: Vec<usize> = (0..spec_count).collect();
    spec_order.sort_by(|a, b| params[*a].key.cmp(params[*b].key));
    Table {
        kind,
        params,
        spec_count,
        sidechain,
        bypass,
        by_id,
        spec_order,
    }
}

/// A kind's table in the process's language, built once.
pub fn table(kind: EffectKind) -> &'static Table {
    static TABLES: [OnceLock<Table>; EffectKind::ALL.len()] =
        [const { OnceLock::new() }; EffectKind::ALL.len()];
    let at = EffectKind::ALL
        .iter()
        .position(|k| *k == kind)
        .expect("every kind is in ALL");
    TABLES[at].get_or_init(|| build(kind, text::lang()))
}

/// A kind's table in a given language (tests; the ids never depend on it).
pub fn table_in(kind: EffectKind, lang: Lang) -> Table {
    build(kind, lang)
}

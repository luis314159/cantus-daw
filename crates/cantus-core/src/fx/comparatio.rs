//! Comparatio, the reference comparator (ADR 0146, provisional): its parameter model. Owned
//! by the wave-C builder; the DSP is `cantus_dsp::comparatio`, whose report
//! (`docs/round-fx/comparatio.md`) says why it is built as it is.
//!
//! **A monitoring unit.** You load a reference track (a file, stored by path in
//! [`crate::EffectSpec::ir`] and set with `Command::SetEffectIr`, as Aula's response and
//! Arca's cabinet are — never copied into the project) and switch between your mix and the
//! reference **at the same loudness**. Its place is the master's last insert; it may sit on
//! any strip. **An export, a freeze, a stem or any other render hears the mix, never the
//! reference**: the engine takes the reference out of every snapshot it renders from
//! (`cantus_engine::fx::comparatio_store::strip_for_render`).
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the
//! bit**: the mix heard. With no reference loaded the mix is heard whatever is set.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `listen` | what is heard: the mix, the reference, X (one of them, hidden), the blind preference's One and Two | 0 … 4 |
//! | `match` | how the reference's level follows the mix's: integrated, short-term, by hand | 0 … 2 |
//! | `offset` | added to the match in every mode | −24 … +24 dB |
//! | `refl` | the integrated match reads the reference over what has played, or over its whole file | 0 … 1 |
//! | `fade` | the switch's crossfade | 1 … 500 ms |
//! | `sync` | where the reference plays: its own playhead while the song plays, locked to the song, a loop | 0 … 2 |
//! | `soff` | locked: the reference ahead of the song by | −600 … +600 s |
//! | `lstart`, `lend` | the loop's region in the reference (an end at or before the start: to the reference's end) | 0 … 3 600 s |
//! | `seed`, `trial` | the blind test's order and the trial under way | 0 … 65 535, 0 … 999 |
//! | `res` | the tonal picture in thirds or sixths of an octave (the display's; no sample changes) | 0 … 1 |
//! | `eqamt`, `eqmax` | the match EQ's suggestion: how far toward the reference, and the most it boosts or cuts (the display's and Send to Aequalis's; no sample changes) | 0 … 100 %, 1 … 12 dB |
//!
//! **The references** (phase B, ADR 0146): a Comparatio holds a list of reference tracks
//! ([`References`], `EffectSpec::refs`), each a file by path with its size and a hash taken
//! when it was loaded (so a file that went missing or changed is reported, never skipped
//! silently); the one compared is `EffectSpec::ir`, always one of the list.
//! `Command::SetReferences` sets the list and the one compared together, as one step whose
//! inverse is the pair it replaced. The list also names the Aequalis the match EQ was last
//! sent to (`eq`), so sending again updates that one instead of adding another.

use crate::effect::{p, EffectParamInfo};
use crate::fx::aula::IrFile;
use crate::EffectId;
use serde::{Deserialize, Serialize};

/// The most references one Comparatio holds.
pub const MAX_REFERENCES: usize = 8;

/// A reference track: its path as the project keeps it (relative to the project's folder
/// when it is inside it, absolute otherwise), and what the file was when it was loaded.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub path: String,
    /// The file's size in bytes when it was loaded (0: not known).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bytes: u64,
    /// A 64-bit hash of the file's head, tail and places between when it was loaded, in
    /// hexadecimal (empty: not known). `cantus_engine::fx::comparatio_store::fingerprint`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

impl Reference {
    pub fn new(path: impl Into<String>) -> Reference {
        Reference {
            path: path.into(),
            ..Reference::default()
        }
    }

    /// The name shown: the file's name without its folder or extension.
    pub fn name(&self) -> String {
        IrFile::new(self.path.clone()).name().to_string()
    }

    /// The file the engine reads for it (`EffectSpec::ir`).
    pub fn file(&self) -> IrFile {
        IrFile::new(self.path.clone())
    }
}

/// A Comparatio's references (`EffectSpec::refs`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct References {
    #[serde(default)]
    pub list: Vec<Reference>,
    /// The Aequalis (on the same strip) the match EQ was last sent to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eq: Option<EffectId>,
}

impl References {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty() && self.eq.is_none()
    }

    /// The entry for `path`, if the list holds it.
    pub fn find(&self, path: &str) -> Option<usize> {
        self.list.iter().position(|r| r.path == path)
    }

    /// Whether the list is one the model accepts: at most [`MAX_REFERENCES`], no empty path,
    /// no path twice.
    pub fn valid(&self) -> Result<(), &'static str> {
        if self.list.len() > MAX_REFERENCES {
            return Err("more references than a Comparatio holds");
        }
        for (i, r) in self.list.iter().enumerate() {
            if r.path.trim().is_empty() {
                return Err("an empty reference path");
            }
            if self.list[..i].iter().any(|o| o.path == r.path) {
                return Err("a reference listed twice");
            }
        }
        Ok(())
    }
}

/// The references a spec shows: its list, with the one compared added when an older file
/// (or a command that set only `ir`) left it out.
pub fn listed(refs: Option<&References>, ir: Option<&IrFile>) -> Vec<Reference> {
    let mut out: Vec<Reference> = refs.map(|r| r.list.clone()).unwrap_or_default();
    if let Some(f) = ir {
        if !out.iter().any(|r| r.path == f.path) {
            out.push(Reference::new(f.path.clone()));
        }
    }
    out
}

pub const NAME_KEY: &str = "effect.comparatio";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
pub const TAKES_NOTES: bool = false;

/// `listen`'s values (the DSP's `Listen`, in this order).
pub const LISTEN_MIX: f32 = 0.0;
pub const LISTEN_REFERENCE: f32 = 1.0;
pub const LISTEN_X: f32 = 2.0;
pub const LISTEN_ONE: f32 = 3.0;
pub const LISTEN_TWO: f32 = 4.0;

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("listen", "fx.comparatio.listen"),
    ("match", "fx.comparatio.match"),
    ("refl", "fx.comparatio.refl"),
    ("sync", "fx.comparatio.sync"),
    ("res", "fx.comparatio.res"),
];

const fn switch(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn seconds(key: &'static str, label: &'static str, lo: f32, hi: f32) -> EffectParamInfo {
    p(key, label, lo, hi, 0.0, "s", false, false)
}

pub const PARAMS: &[EffectParamInfo] = &[
    switch("listen", "fx.comparatio.listen", 4.0, LISTEN_MIX),
    switch("match", "fx.comparatio.match", 2.0, 0.0),
    p(
        "offset",
        "fx.comparatio.offset",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    switch("refl", "fx.comparatio.refl", 1.0, 0.0),
    p(
        "fade",
        "fx.comparatio.fade",
        1.0,
        500.0,
        30.0,
        "ms",
        true,
        false,
    ),
    switch("sync", "fx.comparatio.sync", 2.0, 0.0),
    seconds("soff", "fx.comparatio.soff", -600.0, 600.0),
    seconds("lstart", "fx.comparatio.lstart", 0.0, 3_600.0),
    seconds("lend", "fx.comparatio.lend", 0.0, 3_600.0),
    p(
        "seed",
        "fx.comparatio.seed",
        0.0,
        65_535.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "trial",
        "fx.comparatio.trial",
        0.0,
        999.0,
        0.0,
        "",
        false,
        true,
    ),
    switch("res", "fx.comparatio.res", 1.0, 0.0),
    // Phase B: the match EQ's suggestion (the display's and Send to Aequalis's).
    p(
        "eqamt",
        "fx.comparatio.eqamt",
        0.0,
        100.0,
        EQ_AMOUNT_DEFAULT,
        "%",
        false,
        false,
    ),
    p(
        "eqmax",
        "fx.comparatio.eqmax",
        1.0,
        12.0,
        EQ_MAX_DEFAULT,
        "dB",
        false,
        false,
    ),
];

/// The match EQ's defaults: the DSP's (`cantus_dsp::comparatio::match_eq::MatchOptions`).
pub const EQ_AMOUNT_DEFAULT: f32 = 50.0;
pub const EQ_MAX_DEFAULT: f32 = 6.0;

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// References a command would refuse (a hand edit), repaired on load: a list on a slot that
/// is not a Comparatio taken off, empty and repeated paths dropped, the list cut to
/// [`MAX_REFERENCES`] (the one compared kept), the one compared added when the list lacks
/// it, an empty list `None`. How many slots were changed. Never to the audio thread.
pub fn repair_project(project: &mut crate::model::Project) -> usize {
    let mut changed = 0;
    for e in project.all_effects_mut() {
        if e.spec.kind != crate::EffectKind::Reference {
            if e.spec.refs.take().is_some() {
                changed += 1;
            }
            continue;
        }
        let Some(r) = e.spec.refs.as_deref() else {
            continue;
        };
        let mut clean = References {
            list: Vec::new(),
            eq: r.eq,
        };
        for x in &r.list {
            if !x.path.trim().is_empty() && clean.find(&x.path).is_none() {
                clean.list.push(x.clone());
            }
        }
        if let Some(f) = e.spec.ir.as_deref() {
            if clean.find(&f.path).is_none() {
                clean.list.insert(0, Reference::new(f.path.clone()));
            }
        }
        if clean.list.len() > MAX_REFERENCES {
            let kept = e.spec.ir.as_deref().and_then(|f| clean.find(&f.path));
            let kept = kept.map(|k| clean.list[k].clone());
            clean.list.truncate(MAX_REFERENCES);
            if let Some(k) = kept {
                if clean.find(&k.path).is_none() {
                    clean.list[MAX_REFERENCES - 1] = k;
                }
            }
        }
        if clean != *r {
            changed += 1;
            e.spec.refs = (!clean.is_empty()).then(|| Box::new(clean));
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_defaults_in_range_and_choices_name_stepped_keys() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        for (k, _) in CHOICES {
            let info = PARAMS.iter().find(|p| p.key == *k).expect(k);
            assert!(info.stepped, "{k}");
        }
        let listen = PARAMS.iter().find(|p| p.key == "listen").unwrap();
        assert_eq!(listen.default, LISTEN_MIX, "the null: the mix heard");
        assert_eq!(listen.max, LISTEN_TWO);
    }

    #[test]
    fn a_list_is_valid_only_whole_unique_and_within_its_size() {
        let mut r = References::default();
        assert!(r.valid().is_ok() && r.is_empty());
        r.list.push(Reference::new("a.wav"));
        r.list.push(Reference::new("b.flac"));
        assert!(r.valid().is_ok());
        assert_eq!(r.find("b.flac"), Some(1));
        assert_eq!(r.list[1].name(), "b");
        r.list.push(Reference::new("a.wav"));
        assert!(r.valid().is_err(), "twice");
        r.list.pop();
        r.list.push(Reference::new("  "));
        assert!(r.valid().is_err(), "empty");
        r.list.pop();
        for i in 0..MAX_REFERENCES {
            r.list.push(Reference::new(format!("{i}.wav")));
        }
        assert!(r.valid().is_err(), "too many");
    }

    #[test]
    fn the_list_shown_holds_the_one_compared() {
        let ir = IrFile::new("x/old.wav");
        let l = listed(None, Some(&ir));
        assert_eq!(l, vec![Reference::new("x/old.wav")]);
        let refs = References {
            list: vec![Reference::new("a.wav"), Reference::new("x/old.wav")],
            eq: None,
        };
        assert_eq!(listed(Some(&refs), Some(&ir)).len(), 2);
        assert!(listed(None, None).is_empty());
    }

    #[test]
    fn a_hand_edited_list_is_repaired() {
        let mut p = crate::model::Project::default();
        let s = p.strips[0].id;
        let mut e = p.make_effect(crate::EffectKind::Reference);
        let mut list: Vec<Reference> = (0..12)
            .map(|i| Reference::new(format!("{i}.wav")))
            .collect();
        list.insert(1, Reference::new("0.wav"));
        list.insert(2, Reference::new(""));
        e.spec.refs = Some(Box::new(References { list, eq: None }));
        e.spec.ir = Some(Box::new(IrFile::new("11.wav")));
        let mut other = p.make_effect(crate::EffectKind::Reverb);
        other.spec.refs = Some(Box::new(References::default()));
        p.strip_mut(s).unwrap().effects.push(e);
        p.strip_mut(s).unwrap().effects.push(other);
        assert_eq!(repair_project(&mut p), 2);
        let fx = &p.strip(s).unwrap().effects;
        let r = fx[fx.len() - 2].spec.refs.as_deref().unwrap();
        assert!(r.valid().is_ok());
        assert_eq!(r.list.len(), MAX_REFERENCES);
        assert!(r.find("11.wav").is_some(), "the one compared kept");
        assert!(fx[fx.len() - 1].spec.refs.is_none());
        assert_eq!(repair_project(&mut p), 0, "repaired once");
    }

    #[test]
    fn a_reference_without_a_fingerprint_writes_only_its_path() {
        let j = serde_json::to_string(&Reference::new("a.wav")).unwrap();
        assert_eq!(j, r#"{"path":"a.wav"}"#);
        let back: Reference =
            serde_json::from_str(r#"{"path":"a.wav","bytes":12,"hash":"ff"}"#).unwrap();
        assert_eq!((back.bytes, back.hash.as_str()), (12, "ff"));
    }
}

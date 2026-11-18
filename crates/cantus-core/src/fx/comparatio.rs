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
//! | `eqamt`, `eqmax` | the match EQ's suggestion: how far toward the reference, and the most it boosts or cuts (the display's and Send to Aequalis's; no sample changes) | 0 … 1 (shown in %), 1 … 12 dB |
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub path: String,
    /// The file's size in bytes when it was loaded (0: not known).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bytes: u64,
    /// A 64-bit hash of the file's head, tail and places between when it was loaded, in
    /// hexadecimal (empty: not known). `cantus_engine::fx::comparatio_store::fingerprint`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
    /// Where it plays (the owner, 2026-11-17: each reference keeps its own): kept here while
    /// another is compared, brought back into the unit's keys when it is compared again.
    /// `None`: never compared yet (its playhead free, no loop). While it is the one
    /// compared, the unit's keys are the truth and this may be stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback: Option<Playback>,
}

/// A reference's own playback: the unit's [`PLAYBACK_KEYS`] (`sync`, `soff`, `lstart`,
/// `lend`). The loudness match (`match`, `offset`, `refl`), the crossfade and the display
/// stay the unit's: they are how you compare, the same for every reference, while where a
/// reference plays is the reference's own (a chorus at 0:45 in one, 1:10 in another).
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    pub sync: f32,
    #[serde(default)]
    pub soff: f32,
    #[serde(default)]
    pub lstart: f32,
    #[serde(default)]
    pub lend: f32,
}

/// The keys a reference keeps as its own.
pub const PLAYBACK_KEYS: [&str; 4] = ["sync", "soff", "lstart", "lend"];

impl Default for Playback {
    /// Its own playhead, no offset, no loop: the keys' defaults.
    fn default() -> Self {
        Playback {
            sync: 0.0,
            soff: 0.0,
            lstart: 0.0,
            lend: 0.0,
        }
    }
}

impl Playback {
    /// The unit's keys now.
    pub fn of(spec: &crate::EffectSpec) -> Playback {
        Playback {
            sync: spec.get("sync"),
            soff: spec.get("soff"),
            lstart: spec.get("lstart"),
            lend: spec.get("lend"),
        }
    }

    pub fn values(&self) -> [(&'static str, f32); 4] {
        [
            ("sync", self.sync),
            ("soff", self.soff),
            ("lstart", self.lstart),
            ("lend", self.lend),
        ]
    }

    /// Every value inside its key's range (a stepped key whole).
    pub fn valid(&self) -> bool {
        self.values().iter().all(|(k, v)| {
            PARAMS.iter().find(|i| i.key == *k).is_some_and(|i| {
                v.is_finite() && *v >= i.min && *v <= i.max && (!i.stepped || v.fract() == 0.0)
            })
        })
    }

    /// Each value clamped into its key's range (a hand edit repaired).
    pub fn clamped(&self) -> Playback {
        let c = |k: &str, v: f32| {
            let i = PARAMS.iter().find(|i| i.key == k).expect("a playback key");
            let v = if v.is_finite() { v } else { i.default };
            let v = v.clamp(i.min, i.max);
            if i.stepped {
                v.round()
            } else {
                v
            }
        };
        Playback {
            sync: c("sync", self.sync),
            soff: c("soff", self.soff),
            lstart: c("lstart", self.lstart),
            lend: c("lend", self.lend),
        }
    }
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
            if r.playback.is_some_and(|p| !p.valid()) {
                return Err("a reference's playback out of its range");
            }
        }
        Ok(())
    }
}

/// Compare `selected` from `refs` (the owner, 2026-11-17: each reference keeps its own
/// playback): the one compared until now keeps the unit's playback keys in its entry, and
/// the unit's keys take the newcomer's own (its defaults when it has none yet). Returns the
/// list to set and the keys to write — together one step (`Command::SetReferences` and
/// `SetEffectParam`s in a `Batch`, [`switch_command`]; the plug-in its state and values).
pub fn switch_to(
    spec: &crate::EffectSpec,
    mut refs: References,
    selected: Option<&IrFile>,
) -> (References, Vec<(&'static str, f32)>) {
    let now = Playback::of(spec);
    let old = spec.ir.as_deref().map(|f| f.path.as_str());
    let new = selected.map(|f| f.path.as_str());
    if old == new {
        return (refs, Vec::new());
    }
    if let Some(i) = old.and_then(|p| refs.find(p)) {
        refs.list[i].playback = Some(now);
    }
    let want = new
        .and_then(|p| refs.find(p))
        .and_then(|i| refs.list[i].playback)
        .unwrap_or_default();
    let keys = if new.is_some() {
        want.values()
            .into_iter()
            .zip(now.values())
            .filter(|((_, a), (_, b))| a != b)
            .map(|(w, _)| w)
            .collect()
    } else {
        Vec::new()
    };
    (refs, keys)
}

/// [`switch_to`] as one step of a project: `SetReferences` and the keys, a `Batch`.
pub fn switch_command(
    spec: &crate::EffectSpec,
    strip: crate::StripId,
    effect: EffectId,
    refs: References,
    selected: Option<IrFile>,
) -> crate::Command {
    let (refs, keys) = switch_to(spec, refs, selected.as_ref());
    let mut commands = vec![crate::Command::SetReferences {
        strip,
        effect,
        refs: (!refs.is_empty()).then(|| Box::new(refs)),
        selected: selected.map(Box::new),
    }];
    commands.extend(
        keys.into_iter()
            .map(|(k, v)| crate::Command::SetEffectParam {
                strip,
                effect,
                key: k.to_string(),
                value: v,
            }),
    );
    crate::Command::Batch {
        label_key: "cmd.comparatio_refs".into(),
        commands,
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
        1.0,
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
pub const EQ_AMOUNT_DEFAULT: f32 = 0.5;
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
                let mut x = x.clone();
                x.playback = x.playback.map(|p| p.clamped());
                clean.list.push(x);
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

    /// Never a Tabula pedal (ADR 0146 phase B): a reference is the master's monitoring, and
    /// the render guarantee strips Comparatio slots, not pedals inside a board — so a board
    /// may never hold one (a hand-edited board is repaired the same way).
    /// The owner, 2026-11-17: each reference keeps its own playback; switching saves the
    /// outgoing one's and brings back the incoming one's (its defaults the first time).
    #[test]
    fn switching_keeps_each_reference_s_own_playback() {
        let mut spec = crate::EffectSpec::new(crate::EffectKind::Reference);
        let refs = References {
            list: vec![Reference::new("a.wav"), Reference::new("b.wav")],
            eq: None,
        };
        spec.ir = Some(Box::new(IrFile::new("a.wav")));
        // `a` looped 45 … 75 s.
        for (k, v) in [("sync", 2.0), ("lstart", 45.0), ("lend", 75.0)] {
            spec.set(k, v).unwrap();
        }
        let (r, keys) = switch_to(&spec, refs.clone(), Some(&IrFile::new("b.wav")));
        let a = r.list[0].playback.expect("a keeps its own");
        assert_eq!((a.sync, a.lstart, a.lend), (2.0, 45.0, 75.0));
        assert_eq!(
            keys,
            vec![("sync", 0.0), ("lstart", 0.0), ("lend", 0.0)],
            "b: its defaults"
        );
        // `b` locked 3 s ahead; back to `a`: its loop again.
        let mut spec2 = spec.clone();
        spec2.ir = Some(Box::new(IrFile::new("b.wav")));
        spec2.refs = Some(Box::new(r.clone()));
        for (k, v) in keys.iter().copied().chain([("sync", 1.0), ("soff", 3.0)]) {
            spec2.set(k, v).unwrap();
        }
        let (r2, keys2) = switch_to(&spec2, r, Some(&IrFile::new("a.wav")));
        assert_eq!(r2.list[1].playback.unwrap().soff, 3.0, "b keeps its own");
        assert_eq!(
            keys2,
            vec![
                ("sync", 2.0),
                ("soff", 0.0),
                ("lstart", 45.0),
                ("lend", 75.0)
            ]
        );
        // The same one again: nothing moves.
        assert!(switch_to(&spec, refs, Some(&IrFile::new("a.wav")))
            .1
            .is_empty());
        // Out of range is refused, and repaired on load.
        let bad = Playback {
            sync: 7.0,
            ..Playback::default()
        };
        assert!(!bad.valid());
        assert_eq!(bad.clamped().sync, 2.0);
    }

    #[test]
    fn it_is_never_a_pedal() {
        assert!(!crate::fx::tabula::pedal_allowed(
            crate::EffectKind::Reference
        ));
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

//! What Tympanum's views draw a kit from: its definition (ADR 0124 `KitDef`) and the words
//! the views say of it — a piece's or a mic's name, a key, a size in inches, a mic's kind,
//! the room's measures. One definition per kit, built off the frame and kept while the kit
//! stays (a kit's definition is data, never in the song).
//!
//! **The engine's definitions.** A modelled kit is `tympanum_kit::modelled_kit`'s table, a
//! recorded kit the one its loader registered (`tympanum_kit::recorded`); a pack not read
//! yet (or a kit this build does not know) is drawn as the engine's studio kit under its
//! own name until it is (the integrator, round 40 phase 3: the views' stand-in studio kit
//! removed with the engine's table merged).

use cantus_core::inst::tympanum::{KitRef, MicPattern, RoomChoice};
use cantus_core::inst::tympanum_kit::{
    is_recorded, modelled_kit, recorded, KitDef, MicDef, PieceDef, PieceKind, Place,
};
use cantus_core::{NativeSpec, OutputFamily};
use std::sync::Arc;

/// The definition `spec`'s kit is drawn from: the engine's (a modelled kit's table, an
/// installed pack's read), else the studio kit. Cached per kit (one build per kit change,
/// and one more when a pack drawn as the stand-in has been read since).
pub fn def_of(spec: &NativeSpec) -> Arc<KitDef> {
    let kit = KitRef::of(spec);
    thread_local! {
        static LAST: std::cell::RefCell<Option<(KitRef, bool, Arc<KitDef>)>> =
            const { std::cell::RefCell::new(None) };
    }
    LAST.with(|l| {
        let mut l = l.borrow_mut();
        if let Some((k, stand_in, d)) = l.as_ref() {
            if *k == kit && !(*stand_in && is_recorded(&kit.text())) {
                return d.clone();
            }
        }
        let (d, stand_in) = build(&kit);
        let d = Arc::new(d);
        *l = Some((kit, stand_in, d.clone()));
        d
    })
}

/// The engine's studio kit, the stand-in for a kit not read yet.
fn studio() -> KitDef {
    modelled_kit("studio").expect("the engine's studio kit")
}

/// `kit`'s definition, and whether it is the stand-in.
fn build(kit: &KitRef) -> (KitDef, bool) {
    let found = match kit {
        KitRef::Modelled(id) => modelled_kit(id),
        KitRef::Pack { .. } => recorded(&kit.text()).map(|k| k.def.clone()),
    };
    let stand_in = found.is_none();
    let d = found.unwrap_or_else(|| {
        let mut d = studio();
        d.kit = kit.clone();
        if let KitRef::Pack { kit, .. } = kit {
            d.name = kit.clone();
            d.modelled = false;
            d.default_room = RoomChoice::Own;
            for m in &mut d.mics {
                m.movable = false;
            }
        }
        d
    });
    (d, stand_in)
}

/// The kit's name as the list, the header and the picture's tag say it: a modelled kit's
/// from its locale key (*Studio Maple*), a recorded kit's as its pack names it.
pub fn kit_name(def: &KitDef) -> String {
    if let KitRef::Modelled(id) = &def.kit {
        if let Some(m) = cantus_core::inst::tympanum_kit::modelled_kits()
            .iter()
            .find(|m| m.id == id)
        {
            return crate::i18n::t(m.label_key);
        }
    }
    said(&def.name)
}

/// The words under a modelled kit's name in its tag (*Maple shells, coated heads*), if any.
pub fn kit_tag(def: &KitDef) -> Option<String> {
    let KitRef::Modelled(id) = &def.kit else {
        return None;
    };
    let m = cantus_core::inst::tympanum_kit::modelled_kits()
        .iter()
        .find(|m| m.id == id)?;
    let key = format!("{}.tag", m.label_key);
    let t = crate::i18n::t(&key);
    (t != key).then_some(t)
}

/// A piece's name: its locale key's words when it has one (a modelled kit's), else the
/// name its pack gives it.
pub fn piece_name(p: &PieceDef) -> String {
    if p.label.is_empty() {
        said(&p.name)
    } else {
        crate::i18n::t(&p.label)
    }
}

/// A name the definition gives: a locale key (`tympanum.…`) said in the interface's
/// language, a pack's own name as it is.
pub fn said(name: &str) -> String {
    if name.starts_with("tympanum.") || name.starts_with("inst_mixer.") {
        crate::i18n::t(name)
    } else {
        name.to_string()
    }
}

/// r40:fix-f — a mic's name in the interface's language: a modelled kit's mics are
/// Cantus's own (`tympanum.mic_name.<name>`, the engineers' words a Spanish studio uses:
/// *Bombo fuera*, *Aéreos*, *Sala cerca*), a recorded kit's as its pack names them.
pub fn mic_name(def: &KitDef, m: &MicDef) -> String {
    mic_said(&def.kit, &m.name)
}

/// [`mic_name`] from a kit and a mic's name as the definition (and the song's slot) keeps
/// it.
pub fn mic_said(kit: &KitRef, name: &str) -> String {
    if matches!(kit, KitRef::Modelled(_)) {
        let key = format!("tympanum.mic_name.{}", mic_slug(name));
        if crate::i18n::has(crate::i18n::Locale::En, &key) {
            return crate::i18n::t(&key);
        }
    }
    said(name)
}

/// `Kick Out` → `kick_out`, `Hi-hat` → `hi_hat`.
pub fn mic_slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// A key as the piano roll names it, and its number: "D2 · 38".
pub fn key_text(key: u8) -> String {
    format!("{} \u{b7} {key}", crate::controllers::note_name(key))
}

/// A drum's size in inches, as a drummer says it ("14 × 6.5″"), a cymbal's diameter alone.
pub fn size_text(p: &PieceDef) -> String {
    let inch = |cm: f32| {
        let v = (cm / 2.54 * 2.0).round() / 2.0;
        if (v - v.round()).abs() < 0.01 {
            format!("{v:.0}")
        } else {
            format!("{v:.1}")
        }
    };
    if is_cymbal(p.kind) || p.depth <= 0.0 {
        crate::i18n::tf("tympanum.size_cym", &[("d", &inch(p.diameter))])
    } else {
        crate::i18n::tf(
            "tympanum.size_in",
            &[("d", &inch(p.diameter)), ("depth", &inch(p.depth))],
        )
    }
}

pub fn is_cymbal(k: PieceKind) -> bool {
    matches!(
        k,
        PieceKind::Crash
            | PieceKind::Ride
            | PieceKind::Splash
            | PieceKind::China
            | PieceKind::Stack
            | PieceKind::HiHat
    )
}

/// A mic's kind in a word, from its pattern, family and place: "dynamic", "condenser",
/// "spaced pair", "pair · 2 m".
pub fn mic_kind(def: &KitDef, m: &MicDef) -> String {
    use crate::i18n::{t, tf};
    if m.channels >= 2 {
        if m.family == OutputFamily::Rooms {
            let d = distance_to_kit(def, m) / 100.0;
            return tf("tympanum.mic.kind.room_pair", &[("m", &format!("{d:.0}"))]);
        }
        return t("tympanum.mic.kind.pair");
    }
    let lower = m.name.to_lowercase();
    let key = if lower.contains("sub") {
        "tympanum.mic.kind.sub"
    } else if m.family == OutputFamily::Rooms {
        "tympanum.mic.kind.ribbon"
    } else if m.family == OutputFamily::Toms {
        "tympanum.mic.kind.clip"
    } else if matches!(m.pattern, MicPattern::Cardioid | MicPattern::Supercardioid)
        && matches!(m.family, OutputFamily::Kick | OutputFamily::Snare)
        && !lower.contains("out")
    {
        "tympanum.mic.kind.dynamic"
    } else {
        "tympanum.mic.kind.condenser"
    };
    t(key)
}

/// A pattern's word.
pub fn pattern_key(p: MicPattern) -> &'static str {
    match p {
        MicPattern::Omni => "tympanum.mic.omni",
        MicPattern::Cardioid => "tympanum.mic.cardioid",
        MicPattern::Supercardioid => "tympanum.mic.super",
        MicPattern::Hypercardioid => "tympanum.mic.hyper",
        MicPattern::Figure8 => "tympanum.mic.figure8",
    }
}

/// How far a mic stands from the kit's middle (the kick), cm.
pub fn distance_to_kit(def: &KitDef, m: &MicDef) -> f32 {
    let c = def
        .pieces
        .iter()
        .find(|p| p.kind == PieceKind::Kick)
        .map(|p| p.place)
        .unwrap_or_default();
    dist(&m.place, &c)
}

pub fn dist(a: &Place, b: &Place) -> f32 {
    let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// The pieces a mic is the main mic of, else none (it hears everything as its own).
pub fn main_of(def: &KitDef, piece: u16) -> Option<&MicDef> {
    def.mics.iter().find(|m| m.main_for.contains(&piece))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_studio_kit_is_whole_and_its_keys_are_distinct() {
        let d = studio();
        assert!(d.pieces.len() >= 10, "{} pieces", d.pieces.len());
        assert!(d.mics.len() >= 10, "{} mics", d.mics.len());
        let mut keys: Vec<u8> = d
            .pieces
            .iter()
            .flat_map(|p| p.articulations.iter().map(|a| a.key))
            .collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "two articulations on one key");
        for (i, p) in d.pieces.iter().enumerate() {
            assert_eq!(usize::from(p.index), i);
        }
        for (i, m) in d.mics.iter().enumerate() {
            assert_eq!(usize::from(m.output.0), i);
        }
    }

    #[test]
    fn sizes_read_as_a_drummer_says_them() {
        let d = studio();
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let snare = d
            .pieces
            .iter()
            .find(|p| p.kind == PieceKind::Snare)
            .unwrap();
        assert!(
            size_text(snare).starts_with("14 × "),
            "{}",
            size_text(snare)
        );
        let ride = d.pieces.iter().find(|p| p.kind == PieceKind::Ride).unwrap();
        assert_eq!(size_text(ride), "20″");
        assert_eq!(key_text(38), "D2 · 38");
    }
}
